//! module: treewalker
//! layer: L2
//! exposes: TreeWalker, NodeIterator, NodeFilter
//! requires-load: legacy:Document, legacy:Node, legacy:_markNative
//! requires-call: legacy:_dom, legacy:_wrap

// TreeWalker / NodeIterator (M4 knife 38). document.createTreeWalker / createNodeIterator returned plain OBJECT LITERALS
// (own enumerable root/currentNode/whatToShow/filter plus own `_filter`, `_accept`, `_traverse*` and every mover as an own
// method): not instances of TreeWalker/NodeIterator, `[object Object]`, thirteen leaked own keys. Chrome 145: illegal constructors,
// prototype accessors (root/whatToShow/filter read-only, currentNode read-write) over private state, prototype movers.
// Pinned by chrome/treewalker-truth-145.json (treewalker-probe.js) and diffed by treewalker_matches_chrome_truth. The traversal
// algorithms are the engine's existing ones (issues #432 #461 #462 #467 #469 #475: DOM 6.1/6.2 "traverse children/siblings",
// REJECT pruning in both directions, iterator pointer semantics) moved verbatim onto the prototypes; what changes is the face.

const walkers = new WeakMap();
const iters = new WeakMap();
const SHOW_ALL = 0xFFFFFFFF;

function ushort(v) { const n = Number(v); return isFinite(n) ? ((Math.trunc(n) % 65536) + 65536) % 65536 : 0; }
// three-valued verdict: 1 accept, 2 reject, 3 skip (any other filter result is a skip)
function verdict(s, node) {
  if (!((s.whatToShow >>> (node.nodeType - 1)) & 1)) return 3;
  const f = s.filter;
  if (f === null) return 1;
  let r;
  if (typeof f === 'function') r = f.call(undefined, node);
  else if (typeof f.acceptNode === 'function') r = f.acceptNode(node);
  else return 1;
  const u = ushort(r);
  return u === 1 ? 1 : u === 2 ? 2 : 3;
}

// ---------------------------------------------------------------- TreeWalker
function TreeWalker() { P.fail(TreeWalker, 'Illegal constructor', "Failed to construct 'TreeWalker'"); }
const TP = P.iface(TreeWalker, 'TreeWalker', 0);
function wslot(o, fn) { const s = o !== null && typeof o === 'object' ? walkers.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
P.getter(TP, 'root', function __f() { return wslot(this, __f).root; });
P.getter(TP, 'whatToShow', function __f() { return wslot(this, __f).whatToShow; });
P.getter(TP, 'filter', function __f() { return wslot(this, __f).filter; });
P.accessor(TP, 'currentNode', function __f() { return wslot(this, __f).current; }, function __f(v) {
  const s = wslot(this, __f);
  if (!(v instanceof Node)) P.fail(__f, "Failed to convert value to 'Node'.", "Failed to set the 'currentNode' property on 'TreeWalker'");
  s.current = v;
});
function traverseChildren(s, edge, step) {
  let node = s.current[edge];
  while (node) {
    const v = verdict(s, node);
    if (v === 1) { s.current = node; return node; }
    // only SKIP leaves the children eligible; REJECT prunes the subtree
    if (v === 3) { const child = node[edge]; if (child) { node = child; continue; } }
    while (node) {
      const sibling = node[step];
      if (sibling) { node = sibling; break; }
      const parent = node.parentNode;
      if (!parent || parent === s.root || parent === s.current) return null;
      node = parent;
    }
  }
  return null;
}
function traverseSiblings(s, edge, step) {
  let node = s.current;
  if (node === s.root) return null;
  for (;;) {
    let sibling = node[step];
    while (sibling) {
      node = sibling;
      const v = verdict(s, node);
      if (v === 1) { s.current = node; return node; }
      sibling = node[edge];
      if (v === 2 || !sibling) sibling = node[step];
    }
    node = node.parentNode;
    if (!node || node === s.root) return null;
    if (verdict(s, node) === 1) return null;
  }
}
P.method(TP, 'parentNode', 0, function __f() {
  const s = wslot(this, __f);
  let node = s.current;
  while (node && node !== s.root) {
    node = node.parentNode;
    if (node && verdict(s, node) === 1) { s.current = node; return node; }
  }
  return null;
});
P.method(TP, 'firstChild', 0, function __f() { return traverseChildren(wslot(this, __f), 'firstChild', 'nextSibling'); });
P.method(TP, 'lastChild', 0, function __f() { return traverseChildren(wslot(this, __f), 'lastChild', 'previousSibling'); });
P.method(TP, 'previousSibling', 0, function __f() { return traverseSiblings(wslot(this, __f), 'lastChild', 'previousSibling'); });
P.method(TP, 'nextSibling', 0, function __f() { return traverseSiblings(wslot(this, __f), 'firstChild', 'nextSibling'); });
P.method(TP, 'previousNode', 0, function __f() {
  const s = wslot(this, __f);
  let node = s.current;
  while (node !== s.root) {
    let sibling = node.previousSibling;
    while (sibling) {
      node = sibling;
      let v = verdict(s, node);
      while (v !== 2 && node.lastChild) { node = node.lastChild; v = verdict(s, node); }
      if (v === 1) { s.current = node; return node; }
      sibling = node.previousSibling;
    }
    if (node === s.root || !node.parentNode) return null;
    node = node.parentNode;
    // (no root check here: DOM 6.1 returns the parent when it is the root and accepted - Chrome does too)
    if (verdict(s, node) === 1) { s.current = node; return node; }
  }
  return null;
});
P.method(TP, 'nextNode', 0, function __f() {
  const s = wslot(this, __f);
  let node = _wrap(+_dom('next_in_subtree', s.root._nid, s.current._nid));
  while (node) {
    const v = verdict(s, node);
    if (v === 1) { s.current = node; return node; }
    node = _wrap(+_dom(v === 2 ? 'next_after_subtree' : 'next_in_subtree', s.root._nid, node._nid));
  }
  return null;
});
P.seal(TreeWalker);

// ---------------------------------------------------------------- NodeIterator
function NodeIterator() { P.fail(NodeIterator, 'Illegal constructor', "Failed to construct 'NodeIterator'"); }
const NP = P.iface(NodeIterator, 'NodeIterator', 0);
function islot(o, fn) { const s = o !== null && typeof o === 'object' ? iters.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
P.getter(NP, 'root', function __f() { return islot(this, __f).root; });
P.getter(NP, 'referenceNode', function __f() { return islot(this, __f).ref; });
P.getter(NP, 'pointerBeforeReferenceNode', function __f() { return islot(this, __f).before; });
P.getter(NP, 'whatToShow', function __f() { return islot(this, __f).whatToShow; });
P.getter(NP, 'filter', function __f() { return islot(this, __f).filter; });
P.method(NP, 'detach', 0, function __f() { islot(this, __f); });
function iterTraverse(s, forward) {
  let node = s.ref, before = s.before;
  for (;;) {
    if (forward === before) before = !before;
    else {
      const next = _wrap(+_dom(forward ? 'next_in_subtree' : 'prev_in_subtree', s.root._nid, node._nid));
      if (!next) return null;
      node = next;
    }
    if (verdict(s, node) === 1) break;
  }
  s.ref = node; s.before = before;
  return node;
}
P.method(NP, 'nextNode', 0, function __f() { return iterTraverse(islot(this, __f), true); });
P.method(NP, 'previousNode', 0, function __f() { return iterTraverse(islot(this, __f), false); });
P.seal(NodeIterator);

// ---------------------------------------------------------------- factories
function args(fnName, fn, a) {
  const pre = "Failed to execute '" + fnName + "' on 'Document'";
  P.args(pre, a.length, 1, fn);
  const root = a[0];
  if (!(root instanceof Node)) P.fail(fn, "parameter 1 is not of type 'Node'.", pre);
  const whatToShow = a[1] === undefined ? SHOW_ALL : P.conv.unsignedLong(a[1], pre, fn);
  let filter = a[2];
  if (filter === undefined || filter === null) filter = null;
  else if (typeof filter !== 'object' && typeof filter !== 'function') P.fail(fn, "parameter 3 is not of type 'Object'.", pre);
  return { root, whatToShow, filter };
}
P.method(Document.prototype, 'createTreeWalker', 1, function __f() {
  const a = args('createTreeWalker', __f, arguments);
  const w = Object.create(TP);
  walkers.set(w, { root: a.root, current: a.root, whatToShow: a.whatToShow, filter: a.filter });
  return w;
});
P.method(Document.prototype, 'createNodeIterator', 1, function __f() {
  const a = args('createNodeIterator', __f, arguments);
  const it = Object.create(NP);
  iters.set(it, { root: a.root, ref: a.root, before: true, whatToShow: a.whatToShow, filter: a.filter });
  return it;
});

// NodeFilter is a callback-interface OBJECT in the engine bootstrap (`typeof NodeFilter === 'object'`); in Chrome it is an interface
// function with an illegal constructor carrying the constants (own, enumerable, non-writable, non-configurable).
// A callback interface has no constructor and no prototype property: a method-shorthand function is callable-typed, has no
// `prototype`, and `new NodeFilter()` fails with "is not a constructor" like Chrome's.
const NodeFilter = { NodeFilter() {} }.NodeFilter;
if (typeof _markNative === 'function') _markNative(NodeFilter);
for (const k of ['FILTER_ACCEPT', 'FILTER_REJECT', 'FILTER_SKIP', 'SHOW_ALL', 'SHOW_ELEMENT', 'SHOW_ATTRIBUTE', 'SHOW_TEXT', 'SHOW_CDATA_SECTION', 'SHOW_ENTITY_REFERENCE',
  'SHOW_ENTITY', 'SHOW_PROCESSING_INSTRUCTION', 'SHOW_COMMENT', 'SHOW_DOCUMENT', 'SHOW_DOCUMENT_TYPE', 'SHOW_DOCUMENT_FRAGMENT', 'SHOW_NOTATION']) {
  const v = globalThis.NodeFilter && globalThis.NodeFilter[k];
  if (typeof v === 'number') Object.defineProperty(NodeFilter, k, { value: v, writable: false, enumerable: true, configurable: false });
}

P.expose('TreeWalker', TreeWalker);
P.expose('NodeFilter', NodeFilter);
P.expose('NodeIterator', NodeIterator);
