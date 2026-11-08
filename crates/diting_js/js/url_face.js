(function () { // @@SCOPE
// god-file split (刀57, URL face): the WHATWG URL delegation trio
// (_urlParseOp/_urlSetOp/_urlResolveOp — full component set as JSON from
// the Rust url crate, null on invalid) plus the URL constructor itself
// (_URLw: cached component reads, searchParams write-back through the
// url_search_params.js class, length=1 per Chrome). Moved verbatim from
// bootstrap; _markNative aliased at load for the constructor marking.
// Consumers (bootstrap _resolveUrl/__locParts/location, domcore anchors,
// fetch/XHR/websocket origins) are all runtime reads.
const _markNative = globalThis.__diting_markNative;
// WHATWG URL parsing/serialization is delegated to the Rust `url` crate via
// op_url_parse / op_url_set. The op returns the full component set as JSON; the
// constructor caches it so getters are plain field reads (no per-access op) and
// the hot paths (navigation, fetch, _resolveUrl) stay cheap. Returns null when
// the input is not a valid URL.
function _urlParseOp(url, base) {
  try {
    const s = _OPS.op_url_parse(String(url), (base === undefined || base === null) ? "" : String(base));
    const c = JSON.parse(s);
    return (c && c.ok) ? c : null;
  } catch (e) { return null; }
}
function _urlSetOp(href, part, value) {
  try {
    const s = _OPS.op_url_set(String(href), part, String(value));
    const c = JSON.parse(s);
    return (c && c.ok) ? c : null;
  } catch (e) { return null; }
}
// Returns just the resolved absolute URL string (no component JSON), or null on
// failure. Cheaper than _urlParseOp for callers that only need the href.
function _urlResolveOp(href, base) {
  try {
    const r = _OPS.op_url_resolve(String(href), (base === undefined || base === null) ? "" : String(base));
    return r ? r : null;
  } catch (e) { return null; }
}
if (typeof URL === 'undefined' || !URL.prototype || !URL.__diting) {
  const _URL = class URL {
    constructor(url, base) {
      // 刀87: Chrome 145 messages (k87-ctor-e/f) — argc first, then an
      // unparseable base is its own error (even when url itself is absolute).
      if (arguments.length < 1) throw new TypeError("Failed to construct 'URL': 1 argument required, but only 0 present.");
      if (base !== undefined && base !== null && !_urlParseOp(String(base), undefined)) throw new TypeError("Failed to construct 'URL': Invalid base URL");
      const c = _urlParseOp(url, base);
      if (!c) throw new TypeError("Failed to construct 'URL': Invalid URL");
      this._c = c;
      this._sp = null;
      __hideOwn(this);
    }
    get href() { return this._c.href; }
    set href(v) { const c = _urlParseOp(v, undefined); if (!c) throw new TypeError("Failed to set the 'href' property on 'URL': Invalid URL"); this._c = c; this._refreshSP(); }
    get protocol() { return this._c.protocol; }
    set protocol(v) { this._set('protocol', v); }
    get username() { return this._c.username; }
    set username(v) { this._set('username', v); }
    get password() { return this._c.password; }
    set password(v) { this._set('password', v); }
    get host() { return this._c.host; }
    set host(v) { this._set('host', v); }
    get hostname() { return this._c.hostname; }
    set hostname(v) { this._set('hostname', v); }
    get port() { return this._c.port; }
    set port(v) { this._set('port', v); }
    get pathname() { return this._c.pathname; }
    set pathname(v) { this._set('pathname', v); }
    get search() { return this._c.search; }
    set search(v) { this._set('search', v); this._refreshSP(); }
    get hash() { return this._c.hash; }
    set hash(v) { this._set('hash', v); }
    get origin() { return this._c.origin; }
    get searchParams() {
      if (!this._sp) { __def(this, '_sp', new URLSearchParams(this._c.search)); this._sp._url = this; }
      return this._sp;
    }
    _set(part, value) { const c = _urlSetOp(this._c.href, part, value); if (c) this._c = c; }
    // search changed on the URL side: refresh the bound searchParams contents.
    _refreshSP() { if (this._sp && this._sp._setFromString) this._sp._setFromString(this._c.search); }
    // searchParams mutated: write the serialized query back without re-refreshing.
    _updateSearch(qs) { this._set('search', qs ? ('?' + qs) : ''); }
    toString() { return this._c.href; }
    toJSON() { return this._c.href; }
    static createObjectURL() { return 'blob:null/fake-' + Math.random().toString(36).slice(2); }
    static revokeObjectURL() {}
    // WHATWG URL.parse: like the constructor but returns null instead of throwing.
    static parse(url, base) { const c = _urlParseOp(url, base); if (!c) return null; const u = Object.create(_URL.prototype); u._c = c; u._sp = null; return u; }
    static canParse(url, base) { return _urlParseOp(url, base) !== null; }
  };
  _URL.__diting = true;
  // (刀96) Chrome's URL is a legacy DOM ctor: calling it without new throws
  // the new-check message, NOT V8's natural class message (k96 probe). Expose
  // a function ctor wrapping the class — same prototype object, statics
  // copied in Chrome's SORDER, new.target routed through Reflect.construct
  // so subclasses keep their own prototype.
  const _URLw = function URL(...args) {
    if (!new.target) throw new TypeError("Failed to construct 'URL': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    // rest → spread keeps the REAL argument count: the class's argc ladder
    // ("1 argument required, but only 0 present.") reads arguments.length.
    return Reflect.construct(_URL, args, new.target);
  };
  for (const s of ['canParse', 'parse', 'createObjectURL', 'revokeObjectURL']) _URLw[s] = _URL[s];
  _URLw.prototype = _URL.prototype;
  _URL.prototype.constructor = _URLw;
  _URLw.__diting = true;
  Object.defineProperty(_URLw, 'length', { value: 1, configurable: true });
  _markNative(_URLw);
  globalThis.URL = _URLw;
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_urlParseOp', _urlParseOp);
  d('_urlResolveOp', _urlResolveOp);
  d('_urlSetOp', _urlSetOp);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
