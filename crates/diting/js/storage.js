// Storage family, split out of bootstrap.js wholesale (god-file ratchet,
// pure move; JS-SURFACE-PLAN step 3): the document.cookie access gate,
// StorageEvent, the WHATWG Storage legacy-platform-object + local/session
// stores. Top level only declares bindings; every bootstrap helper named
// here (_OPS, _markNative, _domParse, location, Event) is touched at call
// time — except _markNative/Event, which exist because bootstrap ran first.
// The Storage native marks used to ride bootstrap's big _markNative list;
// they live here now because that list runs at bootstrap load time, before
// Storage exists.
// __cookieGate is a const rather than a function declaration, so it mints no
// window property at all (interface_sweep.js's final refresh would also have
// hidden it, since this file loads before the sweep).

// document.cookie access gate: origins with no cookie jar (about:blank
// initial documents, data:) throw SecurityError in Chrome on BOTH read and
// write — with different prepositions — they never silently no-op (#274).
// The ops resolve against the session URL, which is also how same-origin
// and srcdoc iframes inherit the parent origin below.
const __cookieGate = function (setting) {
  var ok = false;
  try {
    var proto = new URL(_domParse("document_url")).protocol;
    ok = proto === 'http:' || proto === 'https:';
  } catch (e) {}
  if (!ok) {
    throw new DOMException(
      "Failed to " +
        (setting ? "set the 'cookie' property on 'Document'" : "read the 'cookie' property from 'Document'") +
        ": Access is denied for this document.",
      "SecurityError");
  }
};

globalThis.StorageEvent = class StorageEvent extends Event {
  constructor(type, init = {}) {
    super(type, init);
    this.key = init.key !== undefined ? init.key : null;
    this.oldValue = init.oldValue !== undefined ? init.oldValue : null;
    this.newValue = init.newValue !== undefined ? init.newValue : null;
    this.url = init.url || "";
    this.storageArea = init.storageArea || null;
  }
  initStorageEvent(type, bubbles, cancelable, key, oldValue, newValue, url, storageArea) {
    this.initEvent(type, bubbles, cancelable);
    this.key = key !== undefined ? key : null;
    this.oldValue = oldValue !== undefined ? oldValue : null;
    this.newValue = newValue !== undefined ? newValue : null;
    this.url = url || "";
    this.storageArea = storageArea || null;
  }
};
_markNative(globalThis.StorageEvent);

// WHATWG Storage as a legacy platform object: a Proxy routes property access
// (localStorage.foo, localStorage["foo"], delete, `in`, Object.keys) through
// the named getter/setter so length/key()/iteration stay in sync with the
// backing map. Plain prototype methods alone could not intercept direct
// property access, so `localStorage.foo = x` never updated length before.
globalThis.Storage = function Storage() {};
Storage.prototype.getItem = function(k) { k = String(k); return Object.prototype.hasOwnProperty.call(this._data, k) ? this._data[k] : null; };
Storage.prototype.setItem = function(k, v) { this._data[String(k)] = String(v); };
Storage.prototype.removeItem = function(k) { delete this._data[String(k)]; };
Storage.prototype.clear = function() { const d = this._data; for (const k in d) delete d[k]; };
Storage.prototype.key = function(i) { const ks = Object.keys(this._data); i = i >>> 0; return i < ks.length ? ks[i] : null; };
Object.defineProperty(Storage.prototype, 'length', { get: function() { return Object.keys(this._data).length; }, configurable: true });

const _mkStore = (persistent) => {
  const target = Object.create(Storage.prototype);
  Object.defineProperty(target, '_data', { value: Object.create(null), writable: true, enumerable: false, configurable: true });
  // localStorage loads its persisted entries on first access (one JSON file
  // per origin, mirrored through the Rust-side global map — see
  // op_storage_read) and flushes writes debounced; sessionStorage stays
  // memory-only, per-tab lifetime is the spec'd behavior. Opaque origins
  // (data: pages report "null") stay memory-only too. Loading must be lazy:
  // the bootstrap runs before the document URL lands, so an eager read at
  // construction time would pin origin to about:blank.
  let flushPending = false;
  const scheduleFlush = () => {
    if (flushPending) return;
    flushPending = true;
    setTimeout(() => {
      flushPending = false;
      try {
        const origin = location.origin;
        if (!origin || origin === "null") return;
        _OPS.op_storage_write(origin, JSON.stringify(target._data));
      } catch { }
    }, 100);
  };
  let loadPending = persistent;
  const ensureLoaded = () => {
    if (!loadPending) return;
    loadPending = false;
    try {
      const origin = location.origin;
      if (!origin || origin === "null") return;
      const entries = JSON.parse(_OPS.op_storage_read(origin));
      for (const k in entries) target._data[k] = entries[k];
    } catch { }
  };
  if (persistent) {
    // Instance overrides: setItem/getItem et al. touch `this._data` directly
    // and so bypass the proxy traps (`localStorage.foo = x` does not); all of
    // them need the load hook for the same reason.
    target.getItem = function (k) { ensureLoaded(); return Storage.prototype.getItem.call(target, k); };
    target.setItem = function (k, v) { ensureLoaded(); Storage.prototype.setItem.call(target, k, v); scheduleFlush(); };
    target.removeItem = function (k) { ensureLoaded(); Storage.prototype.removeItem.call(target, k); scheduleFlush(); };
    target.clear = function () { ensureLoaded(); Storage.prototype.clear.call(target); scheduleFlush(); };
    target.key = function (i) { ensureLoaded(); return Storage.prototype.key.call(target, i); };
    Object.defineProperty(target, 'length', { get: function () { ensureLoaded(); return Object.keys(target._data).length; }, configurable: true });
  }
  const isReal = (p) => p === '_data' || p === 'constructor' || (p in Storage.prototype);
  return new Proxy(target, {
    get(t, p, recv) { if (typeof p === 'symbol' || isReal(p)) return Reflect.get(t, p, recv); ensureLoaded(); const v = t.getItem(p); return v === null ? undefined : v; },
    set(t, p, v, recv) { if (typeof p === 'symbol' || isReal(p)) return Reflect.set(t, p, v, recv); t.setItem(p, v); return true; },
    has(t, p) { if (typeof p === 'symbol' || isReal(p)) return true; ensureLoaded(); return Object.prototype.hasOwnProperty.call(t._data, p); },
    deleteProperty(t, p) { if (typeof p === 'symbol' || isReal(p)) return Reflect.deleteProperty(t, p); t.removeItem(p); return true; },
    ownKeys(t) { ensureLoaded(); return Object.keys(t._data); },
    getOwnPropertyDescriptor(t, p) {
      if (typeof p !== 'symbol') ensureLoaded();
      if (Object.prototype.hasOwnProperty.call(t._data, p))
        return { value: t._data[p], writable: true, enumerable: true, configurable: true };
      return Reflect.getOwnPropertyDescriptor(t, p);
    },
  });
};
globalThis.localStorage = _mkStore(true);
globalThis.sessionStorage = _mkStore(false);

[
  Storage, Storage.prototype.getItem, Storage.prototype.setItem,
  Storage.prototype.removeItem, Storage.prototype.clear, Storage.prototype.key,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });
