//! module: xpath
//! layer: L2
//! exposes: XPathResult, XPathExpression
//! requires-load: legacy:Document, legacy:Node, legacy:DOMException, legacy:MutationObserver
//! requires-call: legacy:document, legacy:Element, legacy:Attr, legacy:Text, legacy:_OPS, legacy:__diting_liftSlot

// XPath 1.0 — thin face over the native engine. The lexer/parser/evaluator
// moved to Rust (crates/diting_xpath, vendored from moli-xpath, driven
// through op_xpath_prefixes/op_xpath_eval over a snapshot of the live
// DomTree); what stays here is everything whose correctness depends on JS
// object identity: the XPathResult/XPathExpression interfaces, result type
// coercion, node wrapping through _wrap/__diting_mkAttr, iterator
// invalidation via MutationObserver, and document-order sorting. Pinned by
// chrome/xpath-truth-157.json (583 keys) and diffed by
// xpath_matches_chrome_truth.

// ---------------------------------------------------------------- errors
function syntaxErr(pre, expr) { return new DOMException(pre + ": The string '" + expr + "' is not a valid XPath expression.", 'SyntaxError'); }
function nsErr(pre, expr) { return new DOMException(pre + ": The string '" + expr + "' contains unresolvable namespaces.", 'NamespaceError'); }
function convErr(pre) { return new DOMException(pre + ': Type conversion failed while evaluating the expression.', 'SyntaxError'); }

const XHTML = 'http://www.w3.org/1999/xhtml';

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

// ---------------------------------------------------------------- document order
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
      if (!el) return { root: null, idx: -1 };
      let r = el;
      while (r.parentNode) r = r.parentNode;
      return { root: r, idx: this.indexMap(r).get(el) * 64 + 32 };
    }
    let r = n;
    while (r.parentNode) r = r.parentNode;
    return { root: r, idx: this.indexMap(r).get(n) * 64 };
  }
  sort(nodes) {
    const keyed = nodes.map((n) => ({ n, k: this.key(n) }));
    keyed.sort((a, b) => (a.k.root !== b.k.root ? 0 : a.k.idx - b.k.idx));
    return keyed.map((x) => x.n);
  }
}

// ---------------------------------------------------------------- result coercion
function makeResult(out, want, pre) {
  let type = want;
  if (type > 9) type = 0;
  if (type === 0) type = out.kind === 'number' ? 1 : out.kind === 'string' ? 2 : out.kind === 'boolean' ? 3 : 4;
  const r = Object.create(RP);
  const s = { type, value: null, nodes: [], idx: 0, invalid: false, version: 0 };
  if (type === 1 || type === 2 || type === 3) {
    // scalar coercion across kinds follows the XPath conversions
    if (type === 1) s.value = toNumber(out);
    else if (type === 2) s.value = toStr(out);
    else s.value = toBool(out);
  } else {
    if (out.kind !== 'nodes') throw new TypeError(pre + ': The result is not a node set, and therefore cannot be converted to the desired type.');
    const ordered = type === 5 || type === 7 || type === 9;
    let nodes = out.nodes;
    if (ordered) nodes = new Order().sort(nodes);
    s.nodes = nodes;
    if (type === 8 || type === 9) s.nodes = s.nodes.slice(0, 1);
    if (isIter(type)) s.version = syncVersion();
  }
  resSlots.set(r, s);
  return r;
}
function nodeString(n) {
  // XPath string-value of a node: element => concatenated descendant text,
  // text/comment/attr => the data, document => descendant text.
  if (n.nodeType === 2) return n.value;
  if (n.nodeType === 3 || n.nodeType === 8) return n.data || '';
  let out = '';
  const walk = (x) => { for (let c = x.firstChild; c; c = c.nextSibling) { if (c.nodeType === 3) out += c.data || ''; else if (c.nodeType === 1) walk(c); } };
  walk(n);
  return out;
}
function toNumber(out) {
  if (out.kind === 'number') return out.v;
  if (out.kind === 'boolean') return out.v ? 1 : 0;
  const s = toStr(out).trim();
  const n = Number(s);
  return s === '' || isNaN(n) ? NaN : n;
}
function toStr(out) {
  if (out.kind === 'string') return out.v;
  if (out.kind === 'number') {
    const n = out.v;
    if (Number.isNaN(n)) return 'NaN';
    if (n === 0) return '0';
    if (!Number.isFinite(n)) return n > 0 ? 'Infinity' : '-Infinity';
    return String(n);
  }
  if (out.kind === 'boolean') return out.v ? 'true' : 'false';
  return out.nodes.length ? nodeString(out.nodes[0]) : '';
}
function toBool(out) {
  if (out.kind === 'boolean') return out.v;
  if (out.kind === 'number') return out.v !== 0 && !Number.isNaN(out.v);
  if (out.kind === 'string') return out.v.length > 0;
  return out.nodes.length > 0;
}

// ---------------------------------------------------------------- native bridge
function wrapNodes(list) {
  const out = [];
  for (const it of list) {
    if (it.nid !== undefined) {
      const n = globalThis._wrap(it.nid);
      if (n) out.push(n);
    } else if (it.attr !== undefined) {
      const owner = globalThis._wrap(it.attr);
      if (!owner) continue;
      const val = owner.getAttribute(it.name);
      if (val === null) continue;
      const a = globalThis.__diting_mkAttr(it.name, val, null, null);
      __diting_liftSlot(a).ownerElement = owner;
      out.push(a);
    }
  }
  return out;
}

function resolveMap(src, resolver, pre) {
  const scanned = JSON.parse(_OPS.op_xpath_prefixes(src));
  if (scanned.err) throw syntaxErr(pre, src);
  const prefixes = scanned.ok || [];
  const map = {};
  for (const pf of prefixes) {
    let uri;
    if (typeof resolver === 'function') uri = resolver(pf);
    else if (resolver !== null && resolver !== undefined && typeof resolver.lookupNamespaceURI === 'function') uri = resolver.lookupNamespaceURI(pf);
    if (uri === null || uri === undefined || uri === '') {
      if (typeof resolver === 'function' || (resolver !== null && resolver !== undefined)) throw nsErr(pre, src);
    } else map[pf] = String(uri);
  }
  return map;
}

function runNative(src, node, type, pre, resolver) {
  const map = resolveMap(src, resolver, pre);
  // attribute context: pass owner nid + qualified name — Attr wrappers carry no _nid
  let cnid = node._nid, aname = '';
  if (cnid === undefined || cnid === null) {
    if (node.nodeType === 2 && node.ownerElement) { cnid = node.ownerElement._nid; aname = node.name; }
    else cnid = -1;
  }
  const out = JSON.parse(_OPS.op_xpath_eval(src, cnid, JSON.stringify(map), aname));
  if (out.err) {
    if (out.err === 'namespace') throw nsErr(pre, src);
    if (out.err === 'typeconv') throw convErr(pre);
    throw syntaxErr(pre, src);
  }
  const ok = out.ok;
  if (ok.kind === 'number' && typeof ok.v === 'string') ok.v = ok.v === '#NaN' ? NaN : ok.v === '#Infinity' ? Infinity : -Infinity;
  if (ok.kind === 'nodes') ok.nodes = wrapNodes(ok.nodes);
  return makeResult(ok, type, pre);
}

function compile(src, resolver, pre) {
  // eager error parity: invalid syntax/unresolvable prefixes throw here,
  // not at first evaluate (Chrome compiles in createExpression). resolveMap's
  // prefix scan is itself a full parse, so syntax errors surface here too.
  resolveMap(src, resolver, pre);
  return src;
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
function runExpr(src, node, type, pre, resolver) {
  return runNative(src, node, type, pre, resolver === undefined ? null : resolver);
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
  return runExpr(s.src, node, type, pre, s.resolver);
});
P.seal(XPathExpression);

function mkExpression(src, resolver, pre) {
  compile(src, resolver, pre);
  const e = Object.create(XEP);
  exprSlots.set(e, { src, resolver });
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
  compile(src, resolver, pre);
  return runExpr(src, node, type, pre, resolver);
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

// XPathEvaluator's shape is generated (gen/xpath). The three operations are
// the XPathEvaluatorBase mixin, flattened into the slice. The brand and the
// evaluator stay here.
const evalSlots = new WeakSet();
exports.ctor = function ctor(iface, target) {
  evalSlots.add(target);
};
exports.call = function call(iface, member, target, ctx, args) {
  if (target === null || typeof target !== 'object' || !evalSlots.has(target)) P.illegal(call);
  if (member === 'createExpression') return createExpression(true, args, ctx, call);
  if (member === 'createNSResolver') return createNSResolver(args, ctx, call);
  return evaluate(args, ctx, call);
};

P.expose('XPathResult', XPathResult);
P.expose('XPathExpression', XPathExpression);
