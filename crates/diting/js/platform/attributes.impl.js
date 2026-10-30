//! module: attributes
//! layer: L2
//! exposes: NamedNodeMap
//! requires-load: -
//! requires-call: legacy:Element, legacy:Attr, legacy:DOMException, legacy:__diting_mkAttr

// NamedNodeMap + live Attr nodes (M4 knife 44). `element.attributes` built a fresh plain ARRAY of plain objects on every access
// (no NamedNodeMap, no identity, `el.attributes !== el.attributes`, `getNamedItem` closures as own array props), and
// getAttributeNode returned a new detached-looking object each call whose value never followed the element. Chrome: one
// NamedNodeMap per element (legacy platform object: indexed + named properties), and one Attr node per attribute that stays the
// same object while the attribute exists, tracks its value both ways, and is detached (ownerElement null, last value kept) when
// the attribute is removed - re-adding the name makes a NEW Attr. Pinned by chrome/nnm-truth-145.json (nnm-probe.js, 37 keys)
// and diffed by namednodemap_matches_chrome_truth.
//
// Model: per element a Map name -> Attr (only attributes somebody asked for), a recorded namespace table for setAttributeNS, and
// removal interception (removeAttribute / removeAttributeNS / toggleAttribute) that detaches the cached Attr. Attribute VALUES stay
// in the DOM; the Attr reads and writes them through its ownerElement (bootstrap.js Attr.value).

const XHTML = 'http://www.w3.org/1999/xhtml';
const nodes = new WeakMap();   // element -> Map(qualified name -> Attr)
const nsInfo = new WeakMap();  // element -> Map(qualified name -> namespaceURI)
const maps = new WeakMap();    // element -> NamedNodeMap proxy
const owners = new WeakMap();  // NamedNodeMap proxy -> element
const EP = globalThis.Element.prototype;
const origRemove = EP.removeAttribute, origRemoveNS = EP.removeAttributeNS, origToggle = EP.toggleAttribute, origSetNS = EP.setAttributeNS;
const origGetNS = EP.getAttributeNS, origHasNS = EP.hasAttributeNS;
const origSet = EP.setAttribute, origGet = EP.getAttribute, origNames = EP.getAttributeNames;

function isHtml(el) { return el.namespaceURI === XHTML && el.ownerDocument && el.ownerDocument.contentType === 'text/html'; }
function canon(el, name) { return isHtml(el) ? String(name).toLowerCase() : String(name); }
function names(el) { return origNames.call(el); }
function has(el, name) { return origGet.call(el, name) !== null; }
function cache(el) { let m = nodes.get(el); if (m === undefined) { m = new Map(); nodes.set(el, m); } return m; }
function split(q) { const i = q.indexOf(':'); return i < 0 ? { prefix: null, local: q } : { prefix: q.slice(0, i), local: q.slice(i + 1) }; }

// the (single) Attr for an existing attribute
function attrOf(el, q) {
  const m = cache(el);
  let a = m.get(q);
  if (a !== undefined && a.ownerElement === el) return a;
  const ns = (nsInfo.get(el) || new Map()).get(q) || null;
  const { prefix, local } = split(q);
  a = __diting_mkAttr(q, '', ns, ns === null ? null : prefix);
  if (ns !== null) a.localName = local;
  a.ownerElement = el;
  m.set(q, a);
  return a;
}
function detach(el, q) {
  const m = nodes.get(el);
  if (m === undefined) return;
  const a = m.get(q);
  if (a === undefined) return;
  m.delete(q);
  a.ownerElement = null;
}
// capture the live value into the Attr, then detach it (called BEFORE the DOM attribute is removed)
function detachWithValue(el, q) {
  const m = nodes.get(el);
  if (m === undefined) return;
  const a = m.get(q);
  if (a === undefined) return;
  const v = origGet.call(el, q);
  if (v !== null) a._v = v;
  detach(el, q);
}

// ---------------------------------------------------------------- NamedNodeMap
function NamedNodeMap() { P.fail(NamedNodeMap, 'Illegal constructor', "Failed to construct 'NamedNodeMap'"); }
const NP = P.iface(NamedNodeMap, 'NamedNodeMap', 0);
function elOf(o, fn) { const el = o !== null && typeof o === 'object' ? owners.get(o) : undefined; if (el === undefined) P.illegal(fn); return el; }
const isIndex = (k) => typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k) && Number(k) < 4294967295;
const handler = {
  get(t, k, r) {
    if (typeof k === 'string') {
      const el = owners.get(r) || owners.get(t);
      if (isIndex(k)) { if (el === undefined) return undefined; const ns = names(el); const i = Number(k); return i < ns.length ? attrOf(el, ns[i]) : undefined; }
      if (!(k in t) && el !== undefined) { const q = names(el).indexOf(k) >= 0 ? k : undefined; if (q !== undefined) return attrOf(el, q); }
    }
    return Reflect.get(t, k, r);
  },
  has(t, k) {
    if (typeof k === 'string') {
      const el = owners.get(t);
      if (el !== undefined) { if (isIndex(k)) return Number(k) < names(el).length; if (names(el).indexOf(k) >= 0) return true; }
    }
    return Reflect.has(t, k);
  },
  ownKeys(t) {
    const el = owners.get(t);
    const ns = el === undefined ? [] : names(el);
    const keys = ns.map((_, i) => String(i));
    for (const n of ns) if (keys.indexOf(n) < 0) keys.push(n);
    return keys.concat(Reflect.ownKeys(t));
  },
  getOwnPropertyDescriptor(t, k) {
    if (typeof k === 'string') {
      const el = owners.get(t);
      if (el !== undefined) {
        const ns = names(el);
        if (isIndex(k) && Number(k) < ns.length) return { value: attrOf(el, ns[Number(k)]), writable: false, enumerable: true, configurable: true };
        if (!isIndex(k) && ns.indexOf(k) >= 0) return { value: attrOf(el, k), writable: false, enumerable: false, configurable: true };
      }
    }
    return Reflect.getOwnPropertyDescriptor(t, k);
  },
};
function mapOf(el) {
  let p = maps.get(el);
  if (p === undefined) {
    const t = Object.create(NP);
    p = new Proxy(t, handler);
    owners.set(t, el); owners.set(p, el);
    maps.set(el, p);
  }
  return p;
}
P.getter(NP, 'length', function __f() { return names(elOf(this, __f)).length; });
function find(el, name) {
  const q = canon(el, name);
  const ns = names(el);
  return ns.indexOf(q) >= 0 ? q : (ns.indexOf(String(name)) >= 0 ? String(name) : null);
}
function findNS(el, ns, local) {
  const info = nsInfo.get(el) || new Map();
  for (const q of names(el)) {
    const { local: l } = split(q);
    const recorded = info.get(q) || null;
    if (l === local && recorded === (ns === null || ns === undefined || ns === '' ? null : String(ns))) return q;
    const a = nodes.get(el) && nodes.get(el).get(q);
    if (a && a.localName === local && (a.namespaceURI || null) === (ns === null || ns === undefined || ns === '' ? null : String(ns))) return q;
  }
  return null;
}
const pre = (cls, m) => "Failed to execute '" + m + "' on '" + cls + "'";
P.method(NP, 'getNamedItem', 1, function __f(name) {
  const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'getNamedItem'), arguments.length, 1, __f);
  const q = find(el, P.conv.DOMString(name, pre('NamedNodeMap', 'getNamedItem'), __f));
  return q === null ? null : attrOf(el, q);
});
P.method(NP, 'getNamedItemNS', 2, function __f(ns, local) {
  const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'getNamedItemNS'), arguments.length, 2, __f);
  const q = findNS(el, ns === null ? null : P.conv.DOMString(ns, pre('NamedNodeMap', 'getNamedItemNS'), __f), P.conv.DOMString(local, pre('NamedNodeMap', 'getNamedItemNS'), __f));
  return q === null ? null : attrOf(el, q);
});
P.method(NP, 'item', 1, function __f(i) {
  const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'item'), arguments.length, 1, __f);
  const n = P.conv.unsignedLong(i, pre('NamedNodeMap', 'item'), __f);
  const ns = names(el);
  return n < ns.length ? attrOf(el, ns[n]) : null;
});
function removeByName(el, q, cls, method, label) {
  if (q === null) throw new DOMException(pre(cls, method) + ": No item with name '" + label + "' was found.", 'NotFoundError');
  const a = attrOf(el, q);
  detachWithValue(el, q);
  origRemove.call(el, q);
  return a;
}
P.method(NP, 'removeNamedItem', 1, function __f(name) {
  const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'removeNamedItem'), arguments.length, 1, __f);
  const label = P.conv.DOMString(name, pre('NamedNodeMap', 'removeNamedItem'), __f);
  return removeByName(el, find(el, label), 'NamedNodeMap', 'removeNamedItem', label);
});
P.method(NP, 'removeNamedItemNS', 2, function __f(ns, local) {
  const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'removeNamedItemNS'), arguments.length, 2, __f);
  const n = ns === null || ns === undefined ? null : P.conv.DOMString(ns, pre('NamedNodeMap', 'removeNamedItemNS'), __f);
  const l = P.conv.DOMString(local, pre('NamedNodeMap', 'removeNamedItemNS'), __f);
  return removeByName(el, findNS(el, n, l), 'NamedNodeMap', 'removeNamedItemNS', (n === null ? '' : n) + '::' + l);
});
function isAttr(a) { return a !== null && typeof a === 'object' && a instanceof globalThis.Attr; }
function put(el, attr, cls, method, fn) {
  const p = pre(cls, method);
  if (!isAttr(attr)) P.fail(fn, "parameter 1 is not of type 'Attr'.", p);
  const owner = attr.ownerElement;
  if (owner === el) return attr;
  if (owner) throw new DOMException(p + ': The node provided is an attribute node that is already an attribute of another Element; attribute nodes must be explicitly cloned.', 'InUseAttributeError');
  const q = attr.namespaceURI ? attr.name : canon(el, attr.name);
  const m = cache(el);
  let old = null;
  if (has(el, q)) { old = attrOf(el, q); detachWithValue(el, q); }
  const v = attr.value;
  if (attr.namespaceURI) origSetNS.call(el, attr.namespaceURI, q, v); else origSet.call(el, q, v);
  if (attr.namespaceURI) { let info = nsInfo.get(el); if (!info) { info = new Map(); nsInfo.set(el, info); } info.set(q, attr.namespaceURI); }
  attr.ownerElement = el;
  m.set(q, attr);
  return old;
}
P.method(NP, 'setNamedItem', 1, function __f(a) { const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'setNamedItem'), arguments.length, 1, __f); return put(el, a, 'NamedNodeMap', 'setNamedItem', __f); });
P.method(NP, 'setNamedItemNS', 1, function __f(a) { const el = elOf(this, __f); P.args(pre('NamedNodeMap', 'setNamedItemNS'), arguments.length, 1, __f); return put(el, a, 'NamedNodeMap', 'setNamedItemNS', __f); });
Object.defineProperty(NP, Symbol.iterator, { value: Array.prototype.values, writable: true, enumerable: false, configurable: true });
P.seal(NamedNodeMap);

// ---------------------------------------------------------------- Element faces
const elOk = (o, fn) => { if (o === null || typeof o !== 'object' || !(o instanceof globalThis.Element)) P.illegal(fn); return o; };
P.getter(EP, 'attributes', function __f() { return mapOf(elOk(this, __f)); });
P.method(EP, 'getAttributeNode', 1, function __f(name) {
  const el = elOk(this, __f); P.args(pre('Element', 'getAttributeNode'), arguments.length, 1, __f);
  const q = find(el, P.conv.DOMString(name, pre('Element', 'getAttributeNode'), __f));
  return q === null ? null : attrOf(el, q);
});
P.method(EP, 'getAttributeNodeNS', 2, function __f(ns, local) {
  const el = elOk(this, __f); P.args(pre('Element', 'getAttributeNodeNS'), arguments.length, 2, __f);
  const q = findNS(el, ns === null ? null : String(ns), String(local));
  return q === null ? null : attrOf(el, q);
});
P.method(EP, 'setAttributeNode', 1, function __f(a) { const el = elOk(this, __f); P.args(pre('Element', 'setAttributeNode'), arguments.length, 1, __f); return put(el, a, 'Element', 'setAttributeNode', __f); });
P.method(EP, 'setAttributeNodeNS', 1, function __f(a) { const el = elOk(this, __f); P.args(pre('Element', 'setAttributeNodeNS'), arguments.length, 1, __f); return put(el, a, 'Element', 'setAttributeNodeNS', __f); });
P.method(EP, 'removeAttributeNode', 1, function __f(a) {
  const el = elOk(this, __f); P.args(pre('Element', 'removeAttributeNode'), arguments.length, 1, __f);
  if (!isAttr(a)) P.fail(__f, "parameter 1 is not of type 'Attr'.", pre('Element', 'removeAttributeNode'));
  if (a.ownerElement !== el) throw new DOMException(pre('Element', 'removeAttributeNode') + ': The node provided is owned by another element.', 'NotFoundError');
  detachWithValue(el, a.name);
  origRemove.call(el, a.name);
  return a;
});
// removal paths that must detach the cached Attr
P.method(EP, 'removeAttribute', 1, function __f(name) {
  const el = elOk(this, __f); P.args(pre('Element', 'removeAttribute'), arguments.length, 1, __f);
  const q = canon(el, P.conv.DOMString(name, pre('Element', 'removeAttribute'), __f));
  detachWithValue(el, q);
  origRemove.call(el, q);
});
P.method(EP, 'removeAttributeNS', 2, function __f(ns, local) {
  const el = elOk(this, __f); P.args(pre('Element', 'removeAttributeNS'), arguments.length, 2, __f);
  const q = findNS(el, ns === null ? null : String(ns), String(local));
  if (q !== null) detachWithValue(el, q);
  return origRemoveNS.apply(el, arguments);
});
P.method(EP, 'toggleAttribute', 1, function __f(name) {
  const el = elOk(this, __f); P.args(pre('Element', 'toggleAttribute'), arguments.length, 1, __f);
  const q = canon(el, String(name));
  if (arguments.length < 2 && has(el, q)) detachWithValue(el, q);
  else if (arguments.length >= 2 && !arguments[1] && has(el, q)) detachWithValue(el, q);
  return origToggle.apply(el, arguments);
});
// remember the namespace of attributes created through setAttributeNS
P.method(EP, 'setAttributeNS', 3, function __f(ns, qname, value) {
  const el = elOk(this, __f);
  const r = origSetNS.apply(el, arguments);
  if (ns !== null && ns !== undefined && ns !== '') { let info = nsInfo.get(el); if (!info) { info = new Map(); nsInfo.set(el, info); } info.set(String(qname), String(ns)); }
  return r;
});

// namespace-aware reads must find attributes by (namespace, localName) the way Chrome does
P.method(EP, 'getAttributeNS', 2, function __f(ns, local) {
  const el = elOk(this, __f); P.args(pre('Element', 'getAttributeNS'), arguments.length, 2, __f);
  const q = findNS(el, ns === null ? null : String(ns), String(local));
  return q === null ? null : origGet.call(el, q);
});
P.method(EP, 'hasAttributeNS', 2, function __f(ns, local) {
  const el = elOk(this, __f); P.args(pre('Element', 'hasAttributeNS'), arguments.length, 2, __f);
  return findNS(el, ns === null ? null : String(ns), String(local)) !== null;
});

P.expose('NamedNodeMap', NamedNodeMap);
