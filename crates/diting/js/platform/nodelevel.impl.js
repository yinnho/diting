//! module: nodelevel
//! layer: L2
//! exposes: -
//! requires-load: legacy:Node, legacy:Text, legacy:Comment, legacy:CDATASection, legacy:ProcessingInstruction, legacy:Document, legacy:DocumentFragment, legacy:DocumentType, legacy:ShadowRoot, legacy:Attr
//! requires-call: -

// Node-level placement (M4 knife 23, widened knife 56). Chrome defines the
// node-reflecting members ONLY on Node.prototype; every node interface
// inherits them. The bootstrap classes each carried their own copy as an
// own accessor/method — 4 to 6 extra own members per prototype that
// Chrome's getOwnPropertyNames never lists (the shape audit's "extra"
// class).
//
// The per-type behaviour is kept, but moved: each class's own descriptors are lifted into
// a table keyed by the owning prototype and deleted from the class; Node.prototype's
// members become dispatchers that look the receiver's prototype chain up in that table
// (cached per prototype) and fall back to the original Node implementation. Elements
// (the hot path) miss the table once per prototype and then take the base getter.
//
// 刀56 widened the table from [nodeType, nodeName, nodeValue, cloneNode] to
// the Attr leftovers too: Attr chained under Node.prototype when it became a
// Node, but its id-less nature means Node's base members would throw for an
// attribute — so the class body carried 11 hand-rolled members (parentNode
// = null, childNodes = [], textContent = value, …) that Chrome does NOT
// list on Attr.prototype (it answers through its own C++ core). The same
// lift covers them: hoisted into the table, dispatched per receiver, and
// Attr.prototype owns exactly Chrome's 8 members again.

const NP = Node.prototype;
// Attr is a Node in Chrome (`attr instanceof Node`). bootstrap.js links Attr.prototype under Node.prototype (same-file link: a
// cross-file prototype mutation would raise the dependency ratchet); its type-specific members are hoisted
// below like every other node class's.
const GETTERS = ['nodeType', 'nodeName', 'parentNode', 'parentElement', 'firstChild', 'lastChild', 'previousSibling', 'nextSibling', 'childNodes', 'ownerDocument', 'isConnected'];
const ACCESSORS = ['nodeValue', 'textContent'];
const METHODS = ['cloneNode', 'hasChildNodes'];
const NAMES = [...GETTERS, ...ACCESSORS, ...METHODS];
const base = {};
const table = {};
const caches = {};
for (const n of NAMES) { base[n] = Object.getOwnPropertyDescriptor(NP, n); table[n] = new Map(); caches[n] = new WeakMap(); }
for (const C of [Text, Comment, CDATASection, ProcessingInstruction, Document, DocumentFragment, DocumentType, ShadowRoot, Attr]) {
  if (typeof C !== 'function') continue;
  for (const n of NAMES) {
    const d = Object.getOwnPropertyDescriptor(C.prototype, n);
    if (d) { table[n].set(C.prototype, d); delete C.prototype[n]; }
  }
}
function find(n, proto) {
  const c = caches[n];
  let hit = c.get(proto);
  if (hit !== undefined) return hit;
  hit = null;
  for (let p = proto; p && p !== NP; p = Object.getPrototypeOf(p)) {
    const d = table[n].get(p);
    if (d) { hit = d; break; }
  }
  c.set(proto, hit);
  return hit;
}
for (const n of GETTERS) {
  P.getter(NP, n, function __f() {
    const d = find(n, Object.getPrototypeOf(this));
    return (d ? d.get : base[n].get).call(this);
  });
}
for (const n of ACCESSORS) {
  P.accessor(NP, n, function __f() {
    const d = find(n, Object.getPrototypeOf(this));
    return (d ? d.get : base[n].get).call(this);
  }, function __f(v) {
    const d = find(n, Object.getPrototypeOf(this));
    const s = d ? d.set : base[n].set;
    if (s) s.call(this, v);
  });
}
for (const n of METHODS) {
  P.method(NP, n, 0, function __f() {
    const d = find(n, Object.getPrototypeOf(this));
    return (d ? d.value : base[n].value).apply(this, arguments);
  });
}
