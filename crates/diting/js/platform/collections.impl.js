//! module: collections
//! layer: L2
//! exposes: HTMLCollection, HTMLOptionsCollection, HTMLFormControlsCollection, RadioNodeList, __diting_mkCollection
//! requires-load: legacy:NodeList
//! requires-call: legacy:Element, legacy:DOMException

// HTMLCollection family (M4 knife 51). The engine's HTMLCollection was `class extends Array` plus a Proxy: Array.isArray was true, every
// Array method existed on it, `length` was an own non-configurable property, and `HTMLCollection._from` was a public static. Chrome's is
// a legacy platform object: indexed properties (enumerable, read-only), non-enumerable named properties (id / HTML `name`), `length`
// on the prototype, and only item / namedItem / @@iterator. HTMLOptionsCollection adds add / remove / selectedIndex / writable length;
// HTMLFormControlsCollection is the form.elements type. Instances are Proxies over `Object.create(prototype)` backed by a snapshot array
// in a WeakMap (same snapshot semantics as before); the factory is the hidden `__diting_mkCollection(items, kind, owner)`.

const store = new WeakMap(); // proxy | target -> { arr, owner }
const isHTMLEl = (el) => !!el && (el.namespaceURI === undefined || el.namespaceURI === 'http://www.w3.org/1999/xhtml');
const isIndex = (k) => typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k) && Number(k) < 4294967295;

function stOf(o, fn) { const s = o !== null && typeof o === 'object' ? store.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
function lookup(arr, name) {
  if (name === '') return null;
  for (let i = 0; i < arr.length; i++) {
    const el = arr[i];
    if (!el) continue;
    if (el.id === name) return el;
    if (isHTMLEl(el) && typeof el.getAttribute === 'function' && el.getAttribute('name') === name) return el;
  }
  return null;
}
function supportedNames(arr) {
  const out = [], seen = new Set();
  const add = (n) => { if (n && !seen.has(n)) { seen.add(n); out.push(n); } };
  for (let i = 0; i < arr.length; i++) {
    const el = arr[i];
    if (!el) continue;
    add(el.id);
    if (isHTMLEl(el) && typeof el.getAttribute === 'function') add(el.getAttribute('name'));
  }
  return out;
}

// ---------------------------------------------------------------- HTMLCollection
function HTMLCollection() { P.fail(HTMLCollection, 'Illegal constructor', "Failed to construct 'HTMLCollection'"); }
const HP = P.iface(HTMLCollection, 'HTMLCollection', 0);
P.getter(HP, 'length', function __f() { return stOf(this, __f).arr.length; });
P.method(HP, 'item', 1, function __f(i) {
  const s = stOf(this, __f); P.args("Failed to execute 'item' on 'HTMLCollection'", arguments.length, 1, __f);
  const v = s.arr[P.conv.unsignedLong(i, "Failed to execute 'item' on 'HTMLCollection'", __f)];
  return v === undefined || v === null ? null : v;
});
P.method(HP, 'namedItem', 1, function __f(name) {
  const s = stOf(this, __f); P.args("Failed to execute 'namedItem' on 'HTMLCollection'", arguments.length, 1, __f);
  return lookup(s.arr, P.conv.DOMString(name, "Failed to execute 'namedItem' on 'HTMLCollection'", __f));
});
Object.defineProperty(HP, Symbol.iterator, { value: Array.prototype.values, writable: true, enumerable: false, configurable: true });
P.seal(HTMLCollection);

// ---------------------------------------------------------------- HTMLOptionsCollection
function HTMLOptionsCollection() { P.fail(HTMLOptionsCollection, 'Illegal constructor', "Failed to construct 'HTMLOptionsCollection'"); }
const OP = P.iface(HTMLOptionsCollection, 'HTMLOptionsCollection', 0, HTMLCollection);
P.accessor(OP, 'length', function __f() { return stOf(this, __f).arr.length; }, function __f(v) {
  const s = stOf(this, __f);
  const n = P.conv.unsignedLong(v, "Failed to set the 'length' property on 'HTMLOptionsCollection'", __f);
  const sel = s.owner;
  if (n > 100000) return;
  if (n < s.arr.length) {
    for (let i = s.arr.length - 1; i >= n; i--) { const o = s.arr[i]; if (o && o.parentNode) o.parentNode.removeChild(o); }
    s.arr.length = n;
  } else if (n > s.arr.length) {
    for (let i = s.arr.length; i < n; i++) { const o = sel.ownerDocument.createElement('option'); sel.appendChild(o); s.arr.push(o); }
  }
});
P.accessor(OP, 'selectedIndex', function __f() { return stOf(this, __f).owner.selectedIndex; }, function __f(v) { stOf(this, __f).owner.selectedIndex = v; });
P.method(OP, 'add', 1, function __f(element, before) {
  const s = stOf(this, __f); P.args("Failed to execute 'add' on 'HTMLOptionsCollection'", arguments.length, 1, __f);
  s.owner.add(element, before === undefined ? null : before);
});
P.method(OP, 'remove', 1, function __f(index) {
  const s = stOf(this, __f); P.args("Failed to execute 'remove' on 'HTMLOptionsCollection'", arguments.length, 1, __f);
  const n = Number(index) | 0;
  const o = s.arr[n];
  if (n >= 0 && o && o.parentNode) o.parentNode.removeChild(o);
});
P.seal(HTMLOptionsCollection);

// ---------------------------------------------------------------- RadioNodeList (form.elements[name] with several matches)
function RadioNodeList() { P.fail(RadioNodeList, 'Illegal constructor', "Failed to construct 'RadioNodeList'"); }
const RP = P.iface(RadioNodeList, 'RadioNodeList', 0, NodeList);
const isRadio = (el) => el && el.localName === 'input' && String(el.type).toLowerCase() === 'radio';
P.accessor(RP, 'value', function __f() {
  if (this === null || typeof this !== 'object' || !radios.has(this)) P.illegal(__f);
  for (const el of radios.get(this)) if (isRadio(el) && el.checked) return el.value;
  return '';
}, function __f(v) {
  if (this === null || typeof this !== 'object' || !radios.has(this)) P.illegal(__f);
  const want = P.conv.DOMString(v, "Failed to set the 'value' property on 'RadioNodeList'", __f);
  for (const el of radios.get(this)) if (isRadio(el) && el.value === want) { el.checked = true; return; }
});
P.seal(RadioNodeList);
const radios = new WeakMap();
function makeRadioList(items) {
  const o = Object.create(RP);
  Object.defineProperty(o, '_len', { value: items.length, writable: true, configurable: true });
  for (let i = 0; i < items.length; i++) o[i] = items[i];
  radios.set(o, items.slice());
  return o;
}
// form.elements[name] / namedItem: one match -> the element, several -> a RadioNodeList, none -> null.
function controlsLookup(arr, name) {
  if (name === '') return null;
  const hits = [];
  for (let i = 0; i < arr.length; i++) {
    const el = arr[i];
    if (!el) continue;
    if (el.id === name || (isHTMLEl(el) && typeof el.getAttribute === 'function' && el.getAttribute('name') === name)) hits.push(el);
  }
  return hits.length === 0 ? null : hits.length === 1 ? hits[0] : makeRadioList(hits);
}

// ---------------------------------------------------------------- HTMLFormControlsCollection
function HTMLFormControlsCollection() { P.fail(HTMLFormControlsCollection, 'Illegal constructor', "Failed to construct 'HTMLFormControlsCollection'"); }
const FP = P.iface(HTMLFormControlsCollection, 'HTMLFormControlsCollection', 0, HTMLCollection);
P.method(FP, 'namedItem', 1, function __f(name) {
  const s = stOf(this, __f); P.args("Failed to execute 'namedItem' on 'HTMLFormControlsCollection'", arguments.length, 1, __f);
  return controlsLookup(s.arr, P.conv.DOMString(name, "Failed to execute 'namedItem' on 'HTMLFormControlsCollection'", __f));
});
P.seal(HTMLFormControlsCollection);

// ---------------------------------------------------------------- the legacy-platform-object proxy
const find = (t, k) => { const s = store.get(t); return s.kind === 'controls' ? controlsLookup(s.arr, k) : lookup(s.arr, k); };
const handler = {
  get(t, k, r) {
    if (typeof k === 'string') {
      if (isIndex(k)) { const v = store.get(t).arr[Number(k)]; return v === undefined ? undefined : v; }
      if (!(k in t)) { const v = find(t, k); if (v !== null) return v; }
    }
    return Reflect.get(t, k, r === undefined ? t : r);
  },
  has(t, k) {
    if (typeof k === 'string') {
      if (isIndex(k)) return Number(k) < store.get(t).arr.length;
      if (k in t) return true;
      return find(t, k) !== null;
    }
    return Reflect.has(t, k);
  },
  ownKeys(t) {
    const arr = store.get(t).arr, keys = [];
    for (let i = 0; i < arr.length; i++) keys.push(String(i));
    for (const n of supportedNames(arr)) keys.push(n);
    return keys.concat(Reflect.ownKeys(t));
  },
  getOwnPropertyDescriptor(t, k) {
    if (typeof k === 'string') {
      const arr = store.get(t).arr;
      if (isIndex(k)) return Number(k) < arr.length ? { value: arr[Number(k)], writable: false, enumerable: true, configurable: true } : undefined;
      const own = Reflect.getOwnPropertyDescriptor(t, k);
      if (own) return own;
      const v = find(t, k);
      return v !== null ? { value: v, writable: false, enumerable: false, configurable: true } : undefined;
    }
    return Reflect.getOwnPropertyDescriptor(t, k);
  },
  set(t, k, v, r) {
    if (typeof k === 'string' && (isIndex(k) || (!(k in t) && find(t, k) !== null))) return false;
    return Reflect.set(t, k, v, r === undefined ? t : t);
  },
  defineProperty(t, k, d) {
    if (typeof k === 'string' && isIndex(k)) {
      const iface = Object.getPrototypeOf(t) === HP ? 'HTMLCollection' : Object.getPrototypeOf(t) === OP ? 'HTMLOptionsCollection' : 'HTMLFormControlsCollection';
      throw new TypeError("Failed to set an indexed property [" + k + "] on '" + iface + "': Index property setter is not supported.");
    }
    return Reflect.defineProperty(t, k, d);
  },
  deleteProperty(t, k) {
    if (typeof k === 'string' && isIndex(k)) return Number(k) >= store.get(t).arr.length;
    return Reflect.deleteProperty(t, k);
  },
};
const PROTOS = { collection: HP, options: OP, controls: FP };
function make(items, kind, owner) {
  const t = Object.create(PROTOS[kind || 'collection']);
  const s = { arr: Array.prototype.slice.call(items), owner: owner || null, kind: kind || 'collection' };
  store.set(t, s);
  const p = new Proxy(t, handler);
  store.set(p, s);
  return p;
}
P.exposeValue('__diting_mkCollection', make);

P.expose('HTMLCollection', HTMLCollection);
P.expose('HTMLOptionsCollection', HTMLOptionsCollection);
P.expose('HTMLFormControlsCollection', HTMLFormControlsCollection);
P.expose('RadioNodeList', RadioNodeList);
