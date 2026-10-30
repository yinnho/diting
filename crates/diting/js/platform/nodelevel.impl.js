//! module: nodelevel
//! layer: L2
//! exposes: -
//! requires-load: legacy:Node, legacy:Text, legacy:Comment, legacy:CDATASection, legacy:ProcessingInstruction, legacy:Document, legacy:DocumentFragment, legacy:DocumentType, legacy:ShadowRoot
//! requires-call: -

// Node-level placement (M4 knife 23). Chrome defines nodeType / nodeName / nodeValue /
// cloneNode ONLY on Node.prototype; every node interface inherits them. The bootstrap
// classes each carried their own copy as an own accessor/method (Text, Comment,
// CDATASection, ProcessingInstruction, Document, DocumentFragment, DocumentType,
// ShadowRoot) - 4 to 6 extra own members per prototype that Chrome's
// getOwnPropertyNames never lists (the shape audit's "extra" class).
//
// The per-type behaviour is kept, but moved: each class's own descriptors are lifted into
// a table keyed by the owning prototype and deleted from the class; Node.prototype's
// members become dispatchers that look the receiver's prototype chain up in that table
// (cached per prototype) and fall back to the original Node implementation. Elements
// (the hot path) miss the table once per prototype and then take the base getter.

const NP = Node.prototype;
const NAMES = ['nodeType', 'nodeName', 'nodeValue', 'cloneNode'];
const base = {};
const table = {};
const caches = {};
for (const n of NAMES) { base[n] = Object.getOwnPropertyDescriptor(NP, n); table[n] = new Map(); caches[n] = new WeakMap(); }
for (const C of [Text, Comment, CDATASection, ProcessingInstruction, Document, DocumentFragment, DocumentType, ShadowRoot]) {
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
for (const n of ['nodeType', 'nodeName']) {
  P.getter(NP, n, function __f() {
    const d = find(n, Object.getPrototypeOf(this));
    return (d ? d.get : base[n].get).call(this);
  });
}
P.accessor(NP, 'nodeValue', function __f() {
  const d = find('nodeValue', Object.getPrototypeOf(this));
  return (d ? d.get : base.nodeValue.get).call(this);
}, function __f(v) {
  const d = find('nodeValue', Object.getPrototypeOf(this));
  const s = d ? d.set : base.nodeValue.set;
  if (s) s.call(this, v);
});
P.method(NP, 'cloneNode', 0, function __f() {
  const d = find('cloneNode', Object.getPrototypeOf(this));
  return (d ? d.value : base.cloneNode.value).apply(this, arguments);
});
