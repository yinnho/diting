//! module: xpath
//! layer: L2
//! exposes: XPathResult, XPathExpression, XPathEvaluator
//! requires-load: legacy:Document, legacy:Node, legacy:DOMException, legacy:MutationObserver
//! requires-call: legacy:document, legacy:Element, legacy:Attr, legacy:Text

// XPath 1.0 (M4 knife 21). The bootstrap carried a "compact subset" (child/descendant
// steps, @attr, a few predicate shapes, no unions, no axes, no functions beyond
// contains()/not(), no operators, no namespaces) behind document.evaluate and an
// XPathResult that coerced everything through the first node. Chrome implements the
// whole XPath 1.0 recommendation with WebKit's quirks; pages and scraping libraries
// use far more than the subset (axes, count(), normalize-space(), unions, arithmetic).
// This module is a full evaluator: lexer with the spec's operator/name disambiguation,
// recursive-descent parser, all thirteen axes (namespace:: yields nothing, as in
// Chrome), the core function library with arity checks, node-set / number / string /
// boolean conversions and the comparison rules, namespace resolution, and the
// XPathResult / XPathExpression / XPathEvaluator interfaces. Pinned by
// chrome/xpath-truth-145.json (xpath-probe.js, 552 keys: ~120 paths over every axis,
// ~330 scalar expressions and functions, result types, ~130 malformed expressions,
// namespaces, order) and diffed by xpath_matches_chrome_truth.
//
// Quirks reproduced from the corpus: unordered result types keep evaluation order
// (a union is concatenated, not sorted) while ordered types sort into document order;
// `$x` (variables are unsupported) evaluates to the empty string; count() of a
// non-node-set throws; unprefixed name tests match no-namespace elements and, in
// HTML documents, XHTML-namespace elements case-insensitively (SVG `circle` needs a
// resolved prefix); unresolvable prefixes throw NamespaceError at parse time; a
// root for a detached tree is its top ancestor; iterator results become invalid on
// ANY document mutation (observed through one shared MutationObserver).

const XHTML = 'http://www.w3.org/1999/xhtml';
const XMLNS = 'http://www.w3.org/2000/xmlns/';
const XML = 'http://www.w3.org/XML/1998/namespace';

// ---------------------------------------------------------------- errors
function syntaxErr(pre, expr) { return new DOMException(pre + ": The string '" + expr + "' is not a valid XPath expression.", 'SyntaxError'); }
function nsErr(pre, expr) { return new DOMException(pre + ": The string '" + expr + "' contains unresolvable namespaces.", 'NamespaceError'); }
function convErr(pre) { return new DOMException(pre + ': Type conversion failed while evaluating the expression.', 'SyntaxError'); }

// ---------------------------------------------------------------- lexer
const NAME_START = /[\p{L}_]/u;
const NAME_PART = /[\p{L}\p{Nd}._\-·\p{M}]/u;
const AXES = new Set(['ancestor', 'ancestor-or-self', 'attribute', 'child', 'descendant', 'descendant-or-self', 'following', 'following-sibling', 'namespace', 'parent', 'preceding', 'preceding-sibling', 'self']);
const NODE_TYPES = new Set(['node', 'text', 'comment', 'processing-instruction']);
const OP_NAMES = new Set(['and', 'or', 'mod', 'div']);
const isOpTok = (t) => t.k === 'op' || t.k === 'opname' || t.k === 'mul';

function lex(src) {
  const toks = [];
  let i = 0;
  const n = src.length;
  const bad = () => { throw { syntax: true }; };
  const prevAllowsOperand = () => {
    if (toks.length === 0) return true;
    const p = toks[toks.length - 1];
    if (p.k === 'axis') return true;
    if (p.k === 'punct' && ['@', '::', '(', '[', ','].indexOf(p.v) >= 0) return true;
    return isOpTok(p);
  };
  const readName = () => {
    const s = i;
    const cp = src.codePointAt(i);
    const ch = String.fromCodePoint(cp);
    if (!NAME_START.test(ch)) bad();
    i += ch.length;
    while (i < n) {
      const c = String.fromCodePoint(src.codePointAt(i));
      if (!NAME_PART.test(c)) break;
      i += c.length;
    }
    return src.slice(s, i);
  };
  while (i < n) {
    const c = src[i];
    if (c === ' ' || c === '\t' || c === '\n' || c === '\r') { i++; continue; }
    if (c === '"' || c === "'") {
      const j = src.indexOf(c, i + 1);
      if (j < 0) bad();
      toks.push({ k: 'str', v: src.slice(i + 1, j) });
      i = j + 1;
      continue;
    }
    if ((c >= '0' && c <= '9') || (c === '.' && src[i + 1] >= '0' && src[i + 1] <= '9')) {
      const s = i;
      while (i < n && src[i] >= '0' && src[i] <= '9') i++;
      if (src[i] === '.') { i++; while (i < n && src[i] >= '0' && src[i] <= '9') i++; }
      toks.push({ k: 'num', v: Number(src.slice(s, i)) });
      continue;
    }
    if (c === '.' ) {
      if (src[i + 1] === '.') { toks.push({ k: 'punct', v: '..' }); i += 2; } else { toks.push({ k: 'punct', v: '.' }); i++; }
      continue;
    }
    if (c === '/') { if (src[i + 1] === '/') { toks.push({ k: 'op', v: '//' }); i += 2; } else { toks.push({ k: 'op', v: '/' }); i++; } continue; }
    if (c === '!' ) { if (src[i + 1] === '=') { toks.push({ k: 'op', v: '!=' }); i += 2; continue; } bad(); }
    if (c === '<' || c === '>') { if (src[i + 1] === '=') { toks.push({ k: 'op', v: c + '=' }); i += 2; } else { toks.push({ k: 'op', v: c }); i++; } continue; }
    if (c === '=' || c === '|' || c === '+' || c === '-') { toks.push({ k: 'op', v: c }); i++; continue; }
    if (c === '(' || c === ')' || c === '[' || c === ']' || c === '@' || c === ',') { toks.push({ k: 'punct', v: c }); i++; continue; }
    if (c === ':') { if (src[i + 1] === ':') { toks.push({ k: 'punct', v: '::' }); i += 2; continue; } bad(); }
    if (c === '$') {
      i++;
      let name = readName();
      if (src[i] === ':' && src[i + 1] !== ':') { i++; name += ':' + readName(); }
      toks.push({ k: 'var', v: name });
      continue;
    }
    if (c === '*') {
      if (!prevAllowsOperand()) { toks.push({ k: 'mul', v: '*' }); i++; continue; }
      i++;
      toks.push({ k: 'name', prefix: null, local: '*', star: true });
      continue;
    }
    // name / qname / function / axis / node type / operator name
    const first = readName();
    if (!prevAllowsOperand()) {
      if (OP_NAMES.has(first)) { toks.push({ k: 'opname', v: first }); continue; }
      bad();
    }
    // axis?
    let j = i;
    while (j < n && (src[j] === ' ' || src[j] === '\t' || src[j] === '\n' || src[j] === '\r')) j++;
    if (src[j] === ':' && src[j + 1] === ':') { toks.push({ k: 'axis', v: first }); i = j + 2; continue; }
    if (src[i] === ':' && src[i + 1] !== ':') {
      // prefix:local | prefix:*
      i++;
      if (src[i] === '*') { i++; toks.push({ k: 'name', prefix: first, local: '*', star: true }); continue; }
      const local = readName();
      toks.push({ k: 'name', prefix: first, local, star: false, call: nextIsParen() });
      continue;
    }
    toks.push({ k: 'name', prefix: null, local: first, star: false, call: nextIsParen() });
    continue;
  }
  function nextIsParen() { let j = i; while (j < n && (src[j] === ' ' || src[j] === '\t' || src[j] === '\n' || src[j] === '\r')) j++; return src[j] === '('; }
  return toks;
}

// ---------------------------------------------------------------- parser
const ARITY = {
  last: [0, 0], position: [0, 0], count: [1, 1], id: [1, 1], 'local-name': [0, 1], 'namespace-uri': [0, 1], name: [0, 1], string: [0, 1],
  concat: [2, Infinity], 'starts-with': [2, 2], contains: [2, 2], 'substring-before': [2, 2], 'substring-after': [2, 2], substring: [2, 3],
  'string-length': [0, 1], 'normalize-space': [0, 1], translate: [3, 3], boolean: [1, 1], not: [1, 1], true: [0, 0], false: [0, 0], lang: [1, 1],
  number: [0, 1], sum: [1, 1], floor: [1, 1], ceiling: [1, 1], round: [1, 1],
};

function parse(src) {
  const toks = lex(src);
  let p = 0;
  const bad = () => { throw { syntax: true }; };
  const peek = () => toks[p];
  const isP = (v) => { const t = toks[p]; return t !== undefined && t.k === 'punct' && t.v === v; };
  const isOp = (v) => { const t = toks[p]; return t !== undefined && (t.k === 'op' || t.k === 'mul') && t.v === v; };
  const isOpName = (v) => { const t = toks[p]; return t !== undefined && t.k === 'opname' && t.v === v; };
  const expectP = (v) => { if (!isP(v)) bad(); p++; };
  const prefixes = [];

  function parseOr() { let l = parseAnd(); while (isOpName('or')) { p++; l = { t: 'or', a: l, b: parseAnd() }; } return l; }
  function parseAnd() { let l = parseEq(); while (isOpName('and')) { p++; l = { t: 'and', a: l, b: parseEq() }; } return l; }
  function parseEq() { let l = parseRel(); while (isOp('=') || isOp('!=')) { const o = toks[p++].v; l = { t: 'cmp', o, a: l, b: parseRel() }; } return l; }
  function parseRel() { let l = parseAdd(); while (isOp('<') || isOp('>') || isOp('<=') || isOp('>=')) { const o = toks[p++].v; l = { t: 'cmp', o, a: l, b: parseAdd() }; } return l; }
  function parseAdd() { let l = parseMul(); while (isOp('+') || isOp('-')) { const o = toks[p++].v; l = { t: 'arith', o, a: l, b: parseMul() }; } return l; }
  function parseMul() { let l = parseUnary(); while (isOp('*') || isOpName('div') || isOpName('mod')) { const o = toks[p++].v; l = { t: 'arith', o, a: l, b: parseUnary() }; } return l; }
  function parseUnary() { if (isOp('-')) { p++; return { t: 'neg', a: parseUnary() }; } return parseUnion(); }
  function parseUnion() { let l = parsePath(); while (isOp('|')) { p++; l = { t: 'union', a: l, b: parsePath() }; } return l; }

  function startsStep(t) { return t !== undefined && (t.k === 'name' || t.k === 'axis' || (t.k === 'punct' && (t.v === '@' || t.v === '.' || t.v === '..'))); }
  function parsePath() {
    const t = peek();
    if (t === undefined) bad();
    if (t.k === 'op' && (t.v === '/' || t.v === '//')) {
      p++;
      const steps = [];
      if (t.v === '//') { steps.push(DESC_SELF); steps.push(parseStep()); }
      else if (startsStep(peek())) steps.push(parseStep());
      else return { t: 'path', abs: true, steps };
      parseMoreSteps(steps);
      return { t: 'path', abs: true, steps };
    }
    // FilterExpr or relative location path
    if (t.k === 'str' || t.k === 'num' || t.k === 'var' || (t.k === 'punct' && t.v === '(') || (t.k === 'name' && t.call && !NODE_TYPES.has(t.local))) {
      let e = parsePrimary();
      const preds = [];
      while (isP('[')) preds.push(parsePred());
      const steps = [];
      if (isOp('/') || isOp('//')) {
        const o = toks[p++].v;
        if (o === '//') steps.push(DESC_SELF);
        steps.push(parseStep());
        parseMoreSteps(steps);
      }
      if (preds.length === 0 && steps.length === 0) return e;
      return { t: 'path', filter: e, preds, steps };
    }
    if (!startsStep(t)) bad();
    const steps = [parseStep()];
    parseMoreSteps(steps);
    return { t: 'path', abs: false, steps };
  }
  function parseMoreSteps(steps) {
    while (isOp('/') || isOp('//')) {
      const o = toks[p++].v;
      if (o === '//') steps.push(DESC_SELF);
      steps.push(parseStep());
    }
  }
  function parsePrimary() {
    const t = toks[p++];
    if (t.k === 'str') return { t: 'str', v: t.v };
    if (t.k === 'num') return { t: 'num', v: t.v };
    if (t.k === 'var') return { t: 'var', v: t.v };
    if (t.k === 'punct' && t.v === '(') { const e = parseOr(); expectP(')'); return e; }
    if (t.k === 'name') {
      const name = t.prefix ? t.prefix + ':' + t.local : t.local;
      const ar = ARITY[name];
      if (ar === undefined) bad();
      expectP('(');
      const args = [];
      if (!isP(')')) {
        args.push(parseOr());
        while (isP(',')) { p++; args.push(parseOr()); }
      }
      expectP(')');
      if (args.length < ar[0] || args.length > ar[1]) bad();
      return { t: 'fn', name, args };
    }
    bad();
  }
  function parsePred() { expectP('['); const e = parseOr(); expectP(']'); return e; }
  function parseTest(axis) {
    const t = toks[p];
    if (t === undefined || t.k !== 'name') bad();
    p++;
    if (t.call && !t.prefix && NODE_TYPES.has(t.local)) {
      expectP('(');
      let target = null;
      if (t.local === 'processing-instruction' && peek() && peek().k === 'str') { target = toks[p++].v; }
      expectP(')');
      return { type: t.local === 'processing-instruction' ? 'pi' : t.local, target };
    }
    if (t.prefix !== null) { prefixes.push(t.prefix); }
    return { type: 'name', prefix: t.prefix, local: t.local, star: t.star };
  }
  function parseStep() {
    const t = peek();
    if (t === undefined) bad();
    if (t.k === 'punct' && t.v === '.') { p++; return { axis: 'self', test: { type: 'node' }, preds: [] }; }
    if (t.k === 'punct' && t.v === '..') { p++; return { axis: 'parent', test: { type: 'node' }, preds: [] }; }
    let axis = 'child';
    if (t.k === 'axis') { if (!AXES.has(t.v)) bad(); axis = t.v; p++; }
    else if (t.k === 'punct' && t.v === '@') { axis = 'attribute'; p++; }
    const test = parseTest(axis);
    const preds = [];
    while (isP('[')) preds.push(parsePred());
    return { axis, test, preds };
  }
  const DESC_SELF = { axis: 'descendant-or-self', test: { type: 'node' }, preds: [] };
  const ast = parseOr();
  if (p < toks.length) bad();
  return { ast, prefixes };
}

// ---------------------------------------------------------------- node-sets and DOM access
class NodeSet {
  constructor(nodes, sorted) { this.nodes = nodes; this.sorted = sorted === true; }
}
const isNS = (v) => v instanceof NodeSet;

const attrCache = new WeakMap(); // element -> Map(name -> Attr)
function canonAttr(el, a) {
  let m = attrCache.get(el);
  if (m === undefined) { m = new Map(); attrCache.set(el, m); }
  let c = m.get(a.name);
  if (c === undefined) { c = a; m.set(a.name, c); }
  return c;
}
function attrsOf(el) {
  const out = [];
  const as = el.attributes;
  if (!as) return out;
  for (let i = 0; i < as.length; i++) {
    const a = as[i];
    const nm = a.name;
    if (nm === 'xmlns' || nm.startsWith('xmlns:')) continue;
    out.push(canonAttr(el, a));
  }
  return out;
}
function docOf(n) { return n.nodeType === 9 ? n : n.ownerDocument; }
function isHtmlDoc(d) { return d !== null && d !== undefined && d.contentType === 'text/html'; }
function parentOf(n) { return n.nodeType === 2 ? n.ownerElement : n.parentNode; }
function rootOf(n) { let c = n.nodeType === 2 ? n.ownerElement : n; if (c === null || c === undefined) return n; while (c.parentNode) c = c.parentNode; return c; }
function lookupNs(el, prefix) {
  for (let e = el; e && e.nodeType === 1; e = e.parentNode) {
    const as = e.attributes;
    const want = prefix === null ? 'xmlns' : 'xmlns:' + prefix;
    for (let i = 0; i < as.length; i++) if (as[i].name === want) return as[i].value;
  }
  return null;
}
function attrNames(a) {
  const nm = a.name;
  const i = nm.indexOf(':');
  if (i < 0) return { local: nm, ns: null, prefix: null };
  const prefix = nm.slice(0, i);
  const local = nm.slice(i + 1);
  if (prefix === 'xml') return { local, ns: XML, prefix };
  const el = a.ownerElement;
  const ns = el ? lookupNs(el, prefix) : null;
  return { local, ns, prefix };
}
function nameOfNode(n) { // expanded name for name()/local-name()/namespace-uri()
  if (n.nodeType === 1) return { qname: n.namespaceURI === XHTML && isHtmlDoc(n.ownerDocument) ? n.localName : n.nodeName, local: n.localName, ns: n.namespaceURI };
  if (n.nodeType === 2) { const x = attrNames(n); return { qname: n.name, local: x.local, ns: x.ns }; }
  if (n.nodeType === 7) return { qname: n.target, local: n.target, ns: null };
  return null;
}
function stringValue(n) {
  switch (n.nodeType) {
    case 1: return n.textContent === null ? '' : n.textContent;
    case 9: return n.documentElement ? n.documentElement.textContent : '';
    case 2: return n.value;
    case 11: return n.textContent === null ? '' : n.textContent;
    default: return n.data !== undefined ? n.data : (n.nodeValue === null || n.nodeValue === undefined ? '' : n.nodeValue);
  }
}

// document order
class Order {
  constructor() { this.maps = new Map(); }
  indexMap(root) {
    let m = this.maps.get(root);
    if (m !== undefined) return m;
    m = new Map();
    let i = 0;
    const stack = [root];
    while (stack.length) {
      const n = stack.pop();
      m.set(n, i++);
      const kids = [];
      for (let c = n.firstChild; c; c = c.nextSibling) kids.push(c);
      for (let k = kids.length - 1; k >= 0; k--) stack.push(kids[k]);
    }
    this.maps.set(root, m);
    return m;
  }
  key(n) {
    if (n.nodeType === 2) {
      const el = n.ownerElement;
      const r = rootOf(el);
      const idx = this.indexMap(r).get(el);
      let sub = 1;
      const as = el.attributes;
      for (let i = 0; i < as.length; i++) if (as[i].name === n.name) { sub = i + 1; break; }
      return { r, idx, sub };
    }
    const r = rootOf(n);
    return { r, idx: this.indexMap(r).get(n), sub: 0 };
  }
  sort(nodes) {
    if (nodes.length < 2) return nodes;
    const keyed = nodes.map((n) => ({ n, k: this.key(n) }));
    const rootId = new Map();
    keyed.sort((a, b) => {
      if (a.k.r !== b.k.r) {
        if (!rootId.has(a.k.r)) rootId.set(a.k.r, rootId.size);
        if (!rootId.has(b.k.r)) rootId.set(b.k.r, rootId.size);
        return rootId.get(a.k.r) - rootId.get(b.k.r);
      }
      return a.k.idx !== b.k.idx ? a.k.idx - b.k.idx : a.k.sub - b.k.sub;
    });
    return keyed.map((x) => x.n);
  }
}

// ---------------------------------------------------------------- conversions
// WebKit/Blink print XPath numbers with 6 significant digits: fixed notation for
// decimal exponents -6..5, otherwise d.ddddde+X with the mantissa kept at five
// decimals (1000000 -> "1.00000e+6", 0.0000001 -> "1.00000e-7").
function numToStr(n) {
  if (n !== n) return 'NaN';
  if (n === Infinity) return 'Infinity';
  if (n === -Infinity) return '-Infinity';
  if (n === 0) return '0';
  const e = n.toExponential(5);
  const x = Number(/e([+-]\d+)$/.exec(e)[1]);
  if (x >= 6 || x < -6) return e;
  let s = n.toPrecision(6);
  if (s.indexOf('.') >= 0) s = s.replace(/0+$/, '').replace(/\.$/, '');
  return s;
}
function strToNum(s) {
  const t = s.replace(/^[ \t\r\n]+|[ \t\r\n]+$/g, '');
  return /^-?(\d+(\.\d*)?|\.\d+)$/.test(t) ? Number(t) : NaN;
}
function firstOf(ns, order) { const nodes = ns.sorted ? ns.nodes : order.sort(ns.nodes); return nodes.length ? nodes[0] : null; }
function toStr(v, order) {
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return numToStr(v);
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  const f = firstOf(v, order);
  return f === null ? '' : stringValue(f);
}
function toNum(v, order) { return typeof v === 'number' ? v : typeof v === 'boolean' ? (v ? 1 : 0) : strToNum(toStr(v, order)); }
function toBool(v) { return typeof v === 'boolean' ? v : typeof v === 'number' ? !(v === 0 || v !== v) : typeof v === 'string' ? v.length > 0 : v.nodes.length > 0; }

function compareAtoms(op, a, b, order) {
  // a, b are non-node-set atoms
  if (op === '=' || op === '!=') {
    let r;
    if (typeof a === 'boolean' || typeof b === 'boolean') r = toBool(a) === toBool(b);
    else if (typeof a === 'number' || typeof b === 'number') r = toNum(a) === toNum(b);
    else r = toStr(a) === toStr(b);
    return op === '=' ? r : !r;
  }
  const x = toNum(a), y = toNum(b);
  return op === '<' ? x < y : op === '>' ? x > y : op === '<=' ? x <= y : x >= y;
}
function compare(op, a, b, order) {
  const an = isNS(a), bn = isNS(b);
  if (an && bn) {
    const bs = b.nodes.map(stringValue);
    for (const x of a.nodes) { const sx = stringValue(x); for (const sy of bs) if (compareAtoms(op, sx, sy, order)) return true; }
    return false;
  }
  if (an || bn) {
    const ns = an ? a : b, other = an ? b : a;
    if (typeof other === 'boolean') { const nb = toBool(ns); return an ? compareAtoms(op, nb, other, order) : compareAtoms(op, other, nb, order); }
    for (const x of ns.nodes) {
      const sx = stringValue(x);
      const atom = typeof other === 'number' ? sx : sx;
      if (an ? compareAtoms(op, typeof other === 'number' ? strToNum(atom) : atom, other, order) : compareAtoms(op, other, typeof other === 'number' ? strToNum(atom) : atom, order)) return true;
    }
    return false;
  }
  return compareAtoms(op, a, b, order);
}

// ---------------------------------------------------------------- axes
function childrenOf(n) { const out = []; for (let c = n.firstChild; c; c = c.nextSibling) out.push(c); return out; }
function descendants(n, out) {
  const stack = [];
  for (let c = n.lastChild; c; c = c.previousSibling) stack.push(c);
  while (stack.length) {
    const x = stack.pop();
    out.push(x);
    for (let c = x.lastChild; c; c = c.previousSibling) stack.push(c);
  }
  return out;
}
function axisNodes(axis, n) {
  const t = n.nodeType;
  switch (axis) {
    case 'child': return t === 2 ? [] : childrenOf(n);
    case 'descendant': return t === 2 ? [] : descendants(n, []);
    case 'descendant-or-self': return t === 2 ? [n] : descendants(n, [n]);
    case 'self': return [n];
    case 'parent': { const q = parentOf(n); return q ? [q] : []; }
    case 'ancestor': case 'ancestor-or-self': {
      const out = axis === 'ancestor-or-self' ? [n] : [];
      for (let q = parentOf(n); q; q = q.parentNode) out.push(q);
      return out;
    }
    case 'attribute': return t === 1 ? attrsOf(n) : [];
    case 'following-sibling': { if (t === 2) return []; const out = []; for (let s = n.nextSibling; s; s = s.nextSibling) out.push(s); return out; }
    case 'preceding-sibling': { if (t === 2) return []; const out = []; for (let s = n.previousSibling; s; s = s.previousSibling) out.push(s); return out; }
    case 'following': {
      const out = [];
      let cur = t === 2 ? n.ownerElement : n;
      if (t === 2) descendants(cur, out);
      for (; cur; cur = cur.parentNode) {
        for (let s = cur.nextSibling; s; s = s.nextSibling) { out.push(s); descendants(s, out); }
      }
      return out;
    }
    case 'preceding': {
      const out = [];
      const base = t === 2 ? n.ownerElement : n;
      for (let cur = base; cur; cur = cur.parentNode) {
        for (let s = cur.previousSibling; s; s = s.previousSibling) {
          const sub = descendants(s, []);
          for (let k = sub.length - 1; k >= 0; k--) out.push(sub[k]);
          out.push(s);
        }
      }
      return out;
    }
    case 'namespace': return [];
  }
  return [];
}

// ---------------------------------------------------------------- evaluation
function testNode(test, n, axis, ctx) {
  const t = n.nodeType;
  switch (test.type) {
    case 'node': return true;
    case 'text': return t === 3 || t === 4;
    case 'comment': return t === 8;
    case 'pi': return t === 7 && (test.target === null || n.target === test.target);
    case 'name': {
      const principal = axis === 'attribute' ? 2 : 1;
      if (t !== principal) return false;
      if (test.star && test.prefix === null) return true;
      let local, ns, htmlEl = false;
      if (t === 1) { local = n.localName; ns = n.namespaceURI; htmlEl = ctx.html && ns === XHTML; }
      else { const x = attrNames(n); local = x.local; ns = x.ns; htmlEl = ctx.html && n.ownerElement && n.ownerElement.namespaceURI === XHTML; }
      if (test.prefix !== null) { if (ns !== test.ns) return false; if (test.star) return true; return htmlEl ? local === test.local.toLowerCase() : local === test.local; }
      if (!(ns === null || ns === undefined || (t === 1 && ctx.html && ns === XHTML))) return false;
      return htmlEl ? local === test.local.toLowerCase() : local === test.local;
    }
  }
  return false;
}
function applyPreds(nodes, preds, ctx) {
  let cur = nodes;
  for (const pr of preds) {
    const size = cur.length;
    const next = [];
    for (let i = 0; i < size; i++) {
      const v = ev(pr, { node: cur[i], pos: i + 1, size }, ctx);
      if (typeof v === 'number' ? v === i + 1 : toBool(v)) next.push(cur[i]);
    }
    cur = next;
  }
  return cur;
}
function dedupeAdd(out, seen, nodes) { for (const x of nodes) if (!seen.has(x)) { seen.add(x); out.push(x); } }
function evalSteps(start, steps, ctx) {
  let cur = start;
  for (const st of steps) {
    const out = [];
    const seen = new Set();
    for (const n of cur) {
      let cand = axisNodes(st.axis, n).filter((x) => testNode(st.test, x, st.axis, ctx));
      if (st.preds.length) cand = applyPreds(cand, st.preds, ctx);
      dedupeAdd(out, seen, cand);
    }
    cur = cur.length > 1 || st.axis === 'ancestor' || st.axis === 'ancestor-or-self' || st.axis === 'preceding' || st.axis === 'preceding-sibling' ? ctx.order.sort(out) : out;
  }
  return cur;
}
function lang(node, want) {
  want = want.toLowerCase();
  for (let e = node.nodeType === 2 ? node.ownerElement : node; e && e.nodeType === 1; e = e.parentNode) {
    const v0 = e.getAttribute('xml:lang');
    let v = v0;
    if (v !== null) { v = v.toLowerCase(); return v === want || v.startsWith(want + '-'); }
  }
  return false;
}
function round(x) { return x !== x || x === Infinity || x === -Infinity ? x : (x === 0 ? x : (x < 0 && x >= -0.5 ? -0 : Math.floor(x + 0.5))); }
function needNS(v, c) { if (!isNS(v)) throw convErr(c.pre); return v; }

function callFn(name, args, cx, ctx) {
  const o = ctx.order;
  const a = (i) => ev(args[i], cx, ctx);
  const s = (i) => toStr(a(i), o);
  const num = (i) => toNum(a(i), o);
  const ctxNode = () => new NodeSet([cx.node], true);
  switch (name) {
    case 'last': return cx.size;
    case 'position': return cx.pos;
    case 'count': return needNS(a(0), ctx).nodes.length;
    case 'id': {
      const v = a(0);
      const ids = [];
      if (isNS(v)) for (const x of v.nodes) ids.push(...stringValue(x).split(/[ \t\r\n]+/)); else ids.push(...toStr(v, o).split(/[ \t\r\n]+/));
      const d = docOf(cx.node);
      const out = [], seen = new Set();
      for (const id of ids) { if (!id) continue; const el = d.getElementById(id); if (el && !seen.has(el)) { seen.add(el); out.push(el); } }
      return new NodeSet(out, false);
    }
    case 'local-name': case 'name': case 'namespace-uri': {
      let n;
      if (args.length === 0) n = cx.node;
      else { const v = a(0); if (!isNS(v)) return ''; n = firstOf(v, o); }
      if (n === null) return '';
      const x = nameOfNode(n);
      if (x === null) return '';
      return name === 'local-name' ? x.local : name === 'name' ? x.qname : (x.ns === null || x.ns === undefined ? '' : x.ns);
    }
    case 'string': return args.length ? s(0) : stringValue(cx.node);
    case 'concat': { let r = ''; for (let i = 0; i < args.length; i++) r += s(i); return r; }
    case 'starts-with': return s(0).startsWith(s(1));
    case 'contains': return s(0).indexOf(s(1)) >= 0;
    case 'substring-before': { const x = s(0), y = s(1); const i = x.indexOf(y); return i < 0 ? '' : x.slice(0, i); }
    case 'substring-after': { const x = s(0), y = s(1); const i = x.indexOf(y); return i < 0 ? '' : x.slice(i + y.length); }
    case 'substring': {
      const str = s(0);
      const start = round(num(1));
      const len = args.length > 2 ? round(num(2)) : Infinity;
      if (start !== start || len !== len) return '';
      const end = start + len;
      let out = '';
      const chars = Array.from(str);
      for (let i = 0; i < chars.length; i++) { const p = i + 1; if (p >= start && p < end) out += chars[i]; }
      return out;
    }
    case 'string-length': return Array.from(args.length ? s(0) : stringValue(cx.node)).length;
    case 'normalize-space': return (args.length ? s(0) : stringValue(cx.node)).replace(/[ \t\r\n]+/g, ' ').replace(/^ | $/g, '');
    case 'translate': {
      const x = Array.from(s(0)), from = Array.from(s(1)), to = Array.from(s(2));
      const map = new Map();
      from.forEach((c, i) => { if (!map.has(c)) map.set(c, i < to.length ? to[i] : null); });
      let r = '';
      for (const c of x) { if (map.has(c)) { const m = map.get(c); if (m !== null) r += m; } else r += c; }
      return r;
    }
    case 'boolean': return toBool(a(0));
    case 'not': return !toBool(a(0));
    case 'true': return true;
    case 'false': return false;
    case 'lang': return lang(cx.node, s(0));
    case 'number': return args.length ? num(0) : strToNum(stringValue(cx.node));
    case 'sum': { const v = a(0); if (!isNS(v)) return 0; let t = 0; for (const n of v.nodes) t += strToNum(stringValue(n)); return t; }
    case 'floor': return Math.floor(num(0));
    case 'ceiling': return Math.ceil(num(0));
    case 'round': return round(num(0));
  }
  throw convErr(ctx.pre);
}

function ev(e, cx, ctx) {
  switch (e.t) {
    case 'num': return e.v;
    case 'str': return e.v;
    case 'var': return '';
    case 'or': return toBool(ev(e.a, cx, ctx)) || toBool(ev(e.b, cx, ctx));
    case 'and': return toBool(ev(e.a, cx, ctx)) && toBool(ev(e.b, cx, ctx));
    case 'cmp': return compare(e.o, ev(e.a, cx, ctx), ev(e.b, cx, ctx), ctx.order);
    case 'arith': {
      const x = toNum(ev(e.a, cx, ctx), ctx.order), y = toNum(ev(e.b, cx, ctx), ctx.order);
      switch (e.o) { case '+': return x + y; case '-': return x - y; case '*': return x * y; case 'div': return x / y; default: return x % y; }
    }
    case 'neg': return -toNum(ev(e.a, cx, ctx), ctx.order);
    case 'union': {
      const l = needNS(ev(e.a, cx, ctx), ctx), r = needNS(ev(e.b, cx, ctx), ctx);
      const out = [], seen = new Set();
      dedupeAdd(out, seen, l.nodes); dedupeAdd(out, seen, r.nodes);
      return new NodeSet(out, false);
    }
    case 'fn': return callFn(e.name, e.args, cx, ctx);
    case 'path': {
      let start;
      if (e.filter !== undefined) {
        const v = ev(e.filter, cx, ctx);
        if (!isNS(v)) { if (e.preds.length === 0 && e.steps.length === 0) return v; throw convErr(ctx.pre); }
        let nodes = v.sorted ? v.nodes : ctx.order.sort(v.nodes);
        if (e.preds.length) nodes = applyPreds(nodes, e.preds, ctx);
        start = nodes;
        if (e.steps.length === 0) return new NodeSet(nodes, true);
      } else if (e.abs) {
        start = [rootOf(cx.node)];
        if (e.steps.length === 0) return new NodeSet(start, true);
      } else start = [cx.node];
      return new NodeSet(evalSteps(start, e.steps, ctx), false);
    }
  }
  throw convErr(ctx.pre);
}

// compile: parse + resolve prefixes
function compile(src, resolver, pre) {
  let parsed;
  try { parsed = parse(src); } catch (x) { if (x && x.syntax) throw syntaxErr(pre, src); throw x; }
  const map = new Map();
  for (const pf of parsed.prefixes) {
    if (map.has(pf)) continue;
    let uri = null;
    if (typeof resolver === 'function') uri = resolver(pf);
    else if (resolver !== null && resolver !== undefined && typeof resolver.lookupNamespaceURI === 'function') uri = resolver.lookupNamespaceURI(pf);
    if (uri === null || uri === undefined || uri === '') throw nsErr(pre, src);
    map.set(pf, String(uri));
  }
  // bind prefixes into name tests
  (function bind(n) {
    if (n === null || typeof n !== 'object') return;
    if (Array.isArray(n)) { n.forEach(bind); return; }
    if (n.type === 'name' && n.prefix !== null) n.ns = map.get(n.prefix);
    for (const k of Object.keys(n)) bind(n[k]);
  })(parsed.ast);
  return parsed.ast;
}

// ---------------------------------------------------------------- mutation tracking (iterator invalidation)
let version = 0;
let mo = null;
function syncVersion() {
  if (mo === null) {
    try {
      mo = new MutationObserver(function () {});
      mo.observe(document, { childList: true, subtree: true, attributes: true, characterData: true });
    } catch (e) { mo = false; }
  }
  if (mo) { const rs = mo.takeRecords(); if (rs.length) version++; }
  return version;
}

// ---------------------------------------------------------------- XPathResult
const resSlots = new WeakMap();
function XPathResult() { P.fail(XPathResult, 'Illegal constructor', "Failed to construct 'XPathResult'"); }
const RP = P.iface(XPathResult, 'XPathResult', 0);
function rslot(o, fn) { const s = o !== null && typeof o === 'object' ? resSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
const rpre = (k) => "Failed to read the '" + k + "' property from 'XPathResult'";
const isIter = (t) => t === 4 || t === 5;
const isSnap = (t) => t === 6 || t === 7;
const isOne = (t) => t === 8 || t === 9;
P.getter(RP, 'resultType', function __f() { return rslot(this, __f).type; });
P.getter(RP, 'numberValue', function __f() { const s = rslot(this, __f); if (s.type !== 1) P.fail(__f, 'The result type is not a number.', rpre('numberValue')); return s.value; });
P.getter(RP, 'stringValue', function __f() { const s = rslot(this, __f); if (s.type !== 2) P.fail(__f, 'The result type is not a string.', rpre('stringValue')); return s.value; });
P.getter(RP, 'booleanValue', function __f() { const s = rslot(this, __f); if (s.type !== 3) P.fail(__f, 'The result type is not a boolean.', rpre('booleanValue')); return s.value; });
P.getter(RP, 'singleNodeValue', function __f() { const s = rslot(this, __f); if (!isOne(s.type)) P.fail(__f, 'The result type is not a single node.', rpre('singleNodeValue')); return s.nodes.length ? s.nodes[0] : null; });
P.getter(RP, 'invalidIteratorState', function __f() { const s = rslot(this, __f); return s.invalid === true; });
P.getter(RP, 'snapshotLength', function __f() { const s = rslot(this, __f); if (!isSnap(s.type)) P.fail(__f, 'The result type is not a snapshot.', rpre('snapshotLength')); return s.nodes.length; });
const CONSTS = { ANY_TYPE: 0, NUMBER_TYPE: 1, STRING_TYPE: 2, BOOLEAN_TYPE: 3, UNORDERED_NODE_ITERATOR_TYPE: 4, ORDERED_NODE_ITERATOR_TYPE: 5, UNORDERED_NODE_SNAPSHOT_TYPE: 6, ORDERED_NODE_SNAPSHOT_TYPE: 7, ANY_UNORDERED_NODE_TYPE: 8, FIRST_ORDERED_NODE_TYPE: 9 };
for (const [k, v] of Object.entries(CONSTS)) {
  Object.defineProperty(XPathResult, k, { value: v, writable: false, enumerable: true, configurable: false });
  Object.defineProperty(RP, k, { value: v, writable: false, enumerable: true, configurable: false });
}
P.method(RP, 'iterateNext', 0, function __f() {
  const s = rslot(this, __f);
  if (!isIter(s.type)) P.fail(__f, 'The result type is not an iterator.', "Failed to execute 'iterateNext' on 'XPathResult'");
  if (s.invalid || syncVersion() !== s.version) { s.invalid = true; throw new DOMException("Failed to execute 'iterateNext' on 'XPathResult': The document has mutated since the result was returned.", 'InvalidStateError'); }
  return s.idx < s.nodes.length ? s.nodes[s.idx++] : null;
});
P.method(RP, 'snapshotItem', 1, function __f() {
  const s = rslot(this, __f);
  P.args("Failed to execute 'snapshotItem' on 'XPathResult'", arguments.length, 1, __f);
  if (!isSnap(s.type)) P.fail(__f, 'The result type is not a snapshot.', "Failed to execute 'snapshotItem' on 'XPathResult'");
  const i = P.conv.unsignedLong(arguments[0], "Failed to execute 'snapshotItem' on 'XPathResult'", __f);
  return i < s.nodes.length ? s.nodes[i] : null;
});
P.seal(XPathResult);

function makeResult(value, want, ctx, pre) {
  let type = want;
  if (type > 9) type = 0;
  if (type === 0) type = typeof value === 'number' ? 1 : typeof value === 'string' ? 2 : typeof value === 'boolean' ? 3 : 4;
  const r = Object.create(RP);
  const s = { type, value: null, nodes: [], idx: 0, invalid: false, version: 0 };
  if (type === 1) s.value = toNum(value, ctx.order);
  else if (type === 2) s.value = toStr(value, ctx.order);
  else if (type === 3) s.value = toBool(value);
  else {
    if (!isNS(value)) throw new TypeError(pre + ': The result is not a node set, and therefore cannot be converted to the desired type.');
    const ordered = type === 5 || type === 7 || type === 9;
    s.nodes = ordered && !value.sorted ? ctx.order.sort(value.nodes) : value.nodes.slice();
    if (type === 8 || type === 9) s.nodes = s.nodes.slice(0, 1);
    if (isIter(type)) s.version = syncVersion();
  }
  resSlots.set(r, s);
  return r;
}

// ---------------------------------------------------------------- shared evaluate
function checkContext(node, pre) {
  if (node === null || typeof node !== 'object' || typeof node.nodeType !== 'number') return false;
  const t = node.nodeType;
  if (t === 11 || t === 10) throw new DOMException(pre + ": The node provided is '" + (t === 11 ? '#document-fragment' : node.nodeName) + "', which is not a valid context node type.", 'NotSupportedError');
  return true;
}
function checkResolver(r, idx, pre, fn) {
  if (r === null || r === undefined) return null;
  if (typeof r !== 'object' && typeof r !== 'function') P.fail(fn, 'parameter ' + idx + " is not of type 'Object'.", pre);
  return r;
}
function runExpr(ast, node, type, pre) {
  const ctx = { order: new Order(), html: isHtmlDoc(docOf(node)), pre };
  const v = ev(ast, { node, pos: 1, size: 1 }, ctx);
  return makeResult(v, type, ctx, pre);
}
// ---------------------------------------------------------------- XPathExpression / XPathEvaluator
const exprSlots = new WeakMap();
function XPathExpression() { P.fail(XPathExpression, 'Illegal constructor', "Failed to construct 'XPathExpression'"); }
const XEP = P.iface(XPathExpression, 'XPathExpression', 0);
P.method(XEP, 'evaluate', 1, function __f() {
  const s = this !== null && typeof this === 'object' ? exprSlots.get(this) : undefined;
  if (s === undefined) P.illegal(__f);
  const pre = "Failed to execute 'evaluate' on 'XPathExpression'";
  P.args(pre, arguments.length, 1, __f);
  const node = arguments[0];
  if (node === null || typeof node !== 'object' || typeof node.nodeType !== 'number') P.fail(__f, "parameter 1 is not of type 'Node'.", pre);
  checkContext(node, pre);
  const type = arguments[1] === undefined ? 0 : P.conv.unsignedLong(arguments[1], pre, __f) & 0xffff;
  return runExpr(s.ast, node, type, pre);
});
P.seal(XPathExpression);

function mkExpression(src, resolver, pre) {
  const ast = compile(src, resolver, pre);
  const e = Object.create(XEP);
  exprSlots.set(e, { ast });
  return e;
}
function createExpression(thisBrandOk, args, pre, fn) {
  P.args(pre, args.length, 1, fn);
  const src = P.conv.DOMString(args[0], pre, fn);
  const resolver = checkResolver(args[1], 2, pre, fn);
  return mkExpression(src, resolver, pre);
}
function evaluate(args, pre, fn) {
  P.args(pre, args.length, 2, fn);
  const node = args[1];
  if (node === null || typeof node !== 'object' || typeof node.nodeType !== 'number') P.fail(fn, "parameter 2 is not of type 'Node'.", pre);
  const resolver = checkResolver(args[2], 3, pre, fn);
  checkContext(node, pre);
  const type = args[3] === undefined ? 0 : P.conv.unsignedLong(args[3], pre, fn) & 0xffff;
  const src = P.conv.DOMString(args[0], pre, fn);
  const ast = compile(src, resolver, pre);
  return runExpr(ast, node, type, pre);
}
function createNSResolver(args, pre, fn) {
  P.args(pre, args.length, 1, fn);
  const n = args[0];
  if (n === null || typeof n !== 'object' || typeof n.nodeType !== 'number') P.fail(fn, "parameter 1 is not of type 'Node'.", pre);
  return n;
}

const D = Document.prototype;
const isDoc = (o) => o !== null && typeof o === 'object' && o instanceof Document;
P.method(D, 'createExpression', 1, function __f() { if (!isDoc(this)) P.illegal(__f); return createExpression(true, arguments, "Failed to execute 'createExpression' on 'Document'", __f); });
P.method(D, 'createNSResolver', 1, function __f() { if (!isDoc(this)) P.illegal(__f); return createNSResolver(arguments, "Failed to execute 'createNSResolver' on 'Document'", __f); });
P.method(D, 'evaluate', 2, function __f() { if (!isDoc(this)) P.illegal(__f); return evaluate(arguments, "Failed to execute 'evaluate' on 'Document'", __f); });

const evalSlots = new WeakSet();
function XPathEvaluator() {
  if (new.target === undefined) P.fail(XPathEvaluator, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'XPathEvaluator'");
  const self = Reflect.construct(Object, [], new.target);
  evalSlots.add(self);
  return self;
}
const XVP = P.iface(XPathEvaluator, 'XPathEvaluator', 0);
const ebrand = (o, fn) => { if (o === null || typeof o !== 'object' || !evalSlots.has(o)) P.illegal(fn); };
P.method(XVP, 'createExpression', 1, function __f() { ebrand(this, __f); return createExpression(true, arguments, "Failed to execute 'createExpression' on 'XPathEvaluator'", __f); });
P.method(XVP, 'createNSResolver', 1, function __f() { ebrand(this, __f); return createNSResolver(arguments, "Failed to execute 'createNSResolver' on 'XPathEvaluator'", __f); });
P.method(XVP, 'evaluate', 2, function __f() { ebrand(this, __f); return evaluate(arguments, "Failed to execute 'evaluate' on 'XPathEvaluator'", __f); });
P.seal(XPathEvaluator);

P.expose('XPathResult', XPathResult);
P.expose('XPathExpression', XPathExpression);
P.expose('XPathEvaluator', XPathEvaluator);
