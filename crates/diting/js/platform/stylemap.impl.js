//! module: stylemap
//! layer: L2
//! exposes: StylePropertyMap, StylePropertyMapReadOnly
//! requires-load: -
//! requires-call: legacy:document, legacy:getComputedStyle

// Typed OM read faces for Element staged members (M4 knife 4):
// element.attributeStyleMap (StylePropertyMap) and
// element.computedStyleMap() (StylePropertyMapReadOnly). The window classes
// were bare ctor-throw stubs from global_surface.js; this module owns them
// for real (P.iface brands + member order from Chrome truth
// element-staged-truth-145.json keys spm.classes/spm.symbols/spm.ctor - the
// global_surface THROW install SKIPs on our non-enumerable expose, the same
// handover pattern Performance took at knife 3). The ctor keeps throwing
// Chrome's exact "Failed to construct ... Illegal constructor" message.
//
// Behavior pinned by truth: attributeStyleMap is a cached ro accessor whose
// map parses the style attribute (withstyle: 'color: red' -> size 1,
// get('color') is a CSSStyleValue, String 'red'); computedStyleMap() caches a
// read-only map over getComputedStyle(el) whose size === gcs(el).length
// (Chrome 413; the engine's computed-property list is its own count - that
// list's parity is the CSSStyleDeclaration family, not this one). Instances
// of CSSStyleValue are privately minted: the window CSSStyleValue face stays
// the global_surface stub, untouched.

// ---- value face: constructor.name 'CSSStyleValue', String(v) = the text
// (truth asm.withstyle). Private class - nothing here touches the window
// CSSStyleValue stub.
const valSlots = new WeakMap();
function CSSStyleValue(v) { valSlots.set(this, v); }
Object.defineProperty(CSSStyleValue.prototype, 'toString', {
  value: function () { return String(valSlots.get(this)); },
  writable: true, enumerable: false, configurable: true,
});
function mkVal(s) { return new CSSStyleValue(s); }

// ---- interfaces (own-member order = spm.classes truth; @@iterator =
// entries, spm.symbols truth)
function StylePropertyMapReadOnly() {
  P.fail(StylePropertyMapReadOnly, 'Illegal constructor', "Failed to construct 'StylePropertyMapReadOnly'");
}
function StylePropertyMap() {
  P.fail(StylePropertyMap, 'Illegal constructor', "Failed to construct 'StylePropertyMap'");
}
const RO = P.iface(StylePropertyMapReadOnly, 'StylePropertyMapReadOnly', 0);
const MP = P.iface(StylePropertyMap, 'StylePropertyMap', 0, StylePropertyMapReadOnly);

// instance state: { el | null, map: Map<lowercased name, value text> }
const instData = new WeakMap();
function data(o, fn) {
  const d = instData.get(o);
  if (d === undefined) P.illegal(fn);
  return d;
}
function norm(map, k) {
  const s = String(k);
  return map.has(s) ? s : s.toLowerCase();
}

// StylePropertyMapReadOnly.prototype: size get getAll has entries forEach
// keys values (Chrome own order), then constructor.
P.getter(RO, 'size', function __f() { return data(this, __f).map.size; });
P.method(RO, 'get', 1, function __f(k) { const d = data(this, __f); const v = d.map.get(norm(d.map, k)); return v === undefined ? null : mkVal(v); });
P.method(RO, 'getAll', 1, function __f(k) { const d = data(this, __f); const v = d.map.get(norm(d.map, k)); return v === undefined ? null : [mkVal(v)]; });
P.method(RO, 'has', 1, function __f(k) { const d = data(this, __f); return d.map.has(norm(d.map, k)); });
function smEntries(o, fn) { const d = data(o, fn); const out = []; d.map.forEach(function (v, k) { out.push([k, mkVal(v)]); }); return out; }
P.method(RO, 'entries', 0, function __f() { return smEntries(this, __f); });
P.method(RO, 'forEach', 1, function __f(cb, thisArg) { const d = data(this, __f); d.map.forEach(function (v, k) { cb.call(thisArg, mkVal(v), k, d.map.size); }); });
P.method(RO, 'keys', 0, function __f() { return Array.from(data(this, __f).map.keys()); });
P.method(RO, 'values', 0, function __f() { const out = []; data(this, __f).map.forEach(function (v) { out.push(mkVal(v)); }); return out; });
Object.defineProperty(RO, Symbol.iterator, { value: RO.entries, writable: true, enumerable: false, configurable: true });
P.seal(StylePropertyMapReadOnly);

// StylePropertyMap.prototype: append clear delete set (Chrome own order),
// then constructor. Mutators update the style attribute in place.
function writeAttr(o, fn) {
  const d = data(o, fn);
  if (d.el) {
    const parts = [];
    d.map.forEach(function (v, k) { parts.push(k + ': ' + v); });
    d.el.setAttribute('style', parts.join('; '));
  }
}
P.method(MP, 'append', 2, function __f(k, v) { const d = data(this, __f); d.map.set(norm(d.map, k), String(v)); writeAttr(this, __f); });
P.method(MP, 'clear', 0, function __f() { data(this, __f).map.clear(); writeAttr(this, __f); });
P.method(MP, 'delete', 1, function __f(k) { const d = data(this, __f); const hit = d.map.delete(norm(d.map, k)); writeAttr(this, __f); return hit; });
P.method(MP, 'set', 2, function __f(k, v) { const d = data(this, __f); d.map.set(norm(d.map, k), String(v)); writeAttr(this, __f); });
P.seal(StylePropertyMap);

// ---- instance mints (cached per element: truth asm.identity/csm.identity)
function parseDecls(text) {
  const map = new Map();
  String(text == null ? '' : text).split(';').forEach(function (part) {
    const i = part.indexOf(':');
    if (i < 0) return;
    const k = part.slice(0, i).trim();
    const v = part.slice(i + 1).trim();
    if (k) map.set(k.toLowerCase(), v);
  });
  return map;
}

const attrCache = new WeakMap();
exports.forElement = function (el) {
  let m = attrCache.get(el);
  if (!m) {
    m = Object.create(StylePropertyMap.prototype);
    instData.set(m, { el: el, map: parseDecls(el.getAttribute('style')) });
    attrCache.set(el, m);
  }
  return m;
};

const compCache = new WeakMap();
exports.computedFor = function (el) {
  let m = compCache.get(el);
  if (!m) {
    const cs = getComputedStyle(el);
    const map = new Map();
    for (let i = 0; i < cs.length; i++) map.set(cs[i], cs.getPropertyValue(cs[i]));
    m = Object.create(StylePropertyMapReadOnly.prototype);
    instData.set(m, { el: null, map: map });
    compCache.set(el, m);
  }
  return m;
};

P.expose('StylePropertyMapReadOnly', StylePropertyMapReadOnly);
P.expose('StylePropertyMap', StylePropertyMap);
