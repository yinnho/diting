(function () { // @@SCOPE
// god-file split (刀24, fetch helper belt): the byte codecs (base64 both
// ways, Latin-1 binary string, concat, ArrayBuffer-from-view) + the wasm
// instantiateStreaming fallback (+ its load-time install) + the WHATWG
// data: URL processor with percent-decode + the AbortSignal race +
// __recordFetchTiming + the guarded Headers polyfill. The two CDP fetch
// intercept flags stay globalThis member installs. Every cross-file
// consumer (fetch.js / xhr.js / websocket.js / filereader.js / bootstrap)
// calls at runtime — order-free; _percentDecodeToBytes is file-private.
globalThis.__fetchInterceptEnabled = false;
globalThis.__fetchInterceptCallback = null; // Set by CDP to handle paused requests

function _base64ToUint8Array(b64) {
  const clean = String(b64 || '').replace(/[\r\n\s]/g, '');
  if (!clean) return new Uint8Array();
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const padding = clean.endsWith('==') ? 2 : (clean.endsWith('=') ? 1 : 0);
  const bytes = new Uint8Array((clean.length * 3 >> 2) - padding);
  let out = 0;
  for (let i = 0; i < clean.length; i += 4) {
    const a = alphabet.indexOf(clean[i]);
    const b = alphabet.indexOf(clean[i + 1]);
    const c = clean[i + 2] === '=' ? 0 : alphabet.indexOf(clean[i + 2]);
    const d = clean[i + 3] === '=' ? 0 : alphabet.indexOf(clean[i + 3]);
    const n = (a << 18) | (b << 12) | (c << 6) | d;
    if (out < bytes.length) bytes[out++] = (n >> 16) & 0xff;
    if (out < bytes.length) bytes[out++] = (n >> 8) & 0xff;
    if (out < bytes.length) bytes[out++] = n & 0xff;
  }
  return bytes;
}

function _bodyToUint8Array(body) {
  if (body == null) return new Uint8Array();
  if (body instanceof Uint8Array) return body;
  if (body instanceof ArrayBuffer) return new Uint8Array(body);
  if (ArrayBuffer.isView(body)) return new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
  // obscura's Blob materializes its data into _bytes in the constructor.
  if (body._bytes instanceof Uint8Array) return body._bytes;
  return new TextEncoder().encode(String(body));
}

// Latin-1 binary string: one char per byte (kept for callers that need the
// legacy form).
function _bytesToBinaryString(bytes) { let s = ""; for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]); return s; }

// Base64 of raw bytes — the wire form for request bodies. The old Latin-1
// binary string corrupted non-UTF-8 byte sequences on the deno #[string]
// boundary ([0,128,255] became [0,194,128,195,191], upstream obscura #716);
// base64 is byte-exact across that boundary.
function _bytesToBase64(bytes) {
  const c="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let r="";
  for(let i=0;i<bytes.length;i+=3){
    const a=bytes[i],b=bytes[i+1],d=bytes[i+2];
    r+=c[a>>2]+c[((a&3)<<4)|((b??0)>>4)]+(i+1<bytes.length?c[((b&15)<<2)|((d??0)>>6)]:"=")+(i+2<bytes.length?c[d&63]:"=");
  }
  return r;
}

function _concatBytes(arrays) {
  let total = 0;
  for (let i = 0; i < arrays.length; i++) total += arrays[i].length;
  const out = new Uint8Array(total);
  let off = 0;
  for (let i = 0; i < arrays.length; i++) { out.set(arrays[i], off); off += arrays[i].length; }
  return out;
}

function _arrayBufferFromBytes(bytes) {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

function _installWasmStreamingFallback() {
  if (typeof WebAssembly === 'undefined') return;
  if (WebAssembly.instantiateStreaming && WebAssembly.instantiateStreaming.__ditingFallback) return;
  const nativeInstantiateStreaming = WebAssembly.instantiateStreaming;
  const fallback = async function instantiateStreaming(source, imports) {
    const response = await source;
    if (response && typeof response.arrayBuffer === 'function') {
      return WebAssembly.instantiate(await response.arrayBuffer(), imports);
    }
    if (typeof nativeInstantiateStreaming === 'function') {
      return nativeInstantiateStreaming.call(WebAssembly, response, imports);
    }
    return WebAssembly.instantiate(response, imports);
  };
  fallback.__ditingFallback = true;
  WebAssembly.instantiateStreaming = fallback;
}
_installWasmStreamingFallback();

// WHATWG data: URL processor, byte-level. fetch()/XHR take this branch
// because the HTTP client behind op_fetch_url cannot fetch the data: scheme
// (scripts and images already had their own decoders). Returns
// { bytes, mime }; throws TypeError on malformed input like Chrome does.
function _dataUrlBytes(url) {
  const comma = url.indexOf(',');
  if (comma < 0) throw new TypeError('Invalid data: URL');
  let parts = url.slice(5, comma).toLowerCase().split(';');
  const isBase64 = parts[parts.length - 1] === 'base64';
  if (isBase64) parts = parts.slice(0, -1);
  let mime = parts.join(';');
  if (!mime) mime = 'text/plain;charset=us-ascii';
  const payload = url.slice(comma + 1);
  let bytes;
  if (isBase64) {
    const encoded = payload.replace(/[\r\n\t\f ]/g, '');
    const rem = encoded.length % 4;
    if (rem === 1 || !/^[A-Za-z0-9+/]*={0,2}$/.test(encoded)) {
      throw new TypeError('Invalid data: URL base64 payload');
    }
    bytes = _base64ToUint8Array(rem ? encoded + '='.repeat(4 - rem) : encoded);
  } else {
    bytes = _percentDecodeToBytes(payload);
  }
  return { bytes, mime };
}

// Percent-decode a data: payload to raw bytes: %XX escapes become single
// bytes; every other character contributes its UTF-8 encoding (browsers run
// the URL through UTF-8 first, so unescaped non-ASCII lands here too).
function _percentDecodeToBytes(s) {
  const bytes = [];
  let buf = '';
  const flush = () => {
    if (buf) {
      const enc = new TextEncoder().encode(buf);
      for (let i = 0; i < enc.length; i++) bytes.push(enc[i]);
      buf = '';
    }
  };
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '%' && /^[0-9a-fA-F]{2}$/.test(s.slice(i + 1, i + 3))) {
      flush();
      bytes.push(parseInt(s.slice(i + 1, i + 3), 16));
      i += 2;
    } else {
      buf += s[i];
    }
  }
  flush();
  return new Uint8Array(bytes);
}

// Race a fetch op promise against an AbortSignal. Rejects with the signal's
// reason the moment abort fires; the op's own settlement is swallowed so an
// aborted-then-failing walk never surfaces as an unhandled rejection.
function _raceAbortSignal(signal, opCall) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const onAbort = () => {
      if (settled) return;
      settled = true;
      signal.removeEventListener("abort", onAbort);
      reject(signal.reason);
      opCall.then(() => {}, () => {});
    };
    if (signal.aborted) { onAbort(); return; }
    signal.addEventListener("abort", onAbort);
    opCall.then(
      (v) => { if (!settled) { settled = true; signal.removeEventListener("abort", onAbort); resolve(v); } },
      (e) => { if (!settled) { settled = true; signal.removeEventListener("abort", onAbort); reject(e); } }
    );
  });
}

// #126: record one real fetch/XHR sample into the resource buffer. `t0`
// brackets the op call in the resource clock; status and body sizes come
// from the op's parsed reply. Failure shapes (blocked/CORS/abort) record
// duration with status 0 and no sizes — Chrome keeps those entries too.
// `opts` carries per-site extras (e.g. renderBlocking for stylesheets).
function __recordFetchTiming(t0, url, initiator, parsed, opts) {
  const p = globalThis.performance;
  if (!p || typeof p._recordResource !== 'function') return;
  const o = {
    name: String(url || ''),
    startTime: t0,
    duration: Math.max(0, performance.now() - t0),
    initiatorType: initiator || 'fetch',
    responseStatus: parsed && parsed.status ? parsed.status : 0,
    encodedBodySize: 0, decodedBodySize: 0, transferSize: 0,
  };
  if (parsed && parsed.bodyBase64) {
    const bytes = parsed.bodyBase64.length ? _base64ToUint8Array(parsed.bodyBase64) : [];
    o.encodedBodySize = o.decodedBodySize = bytes.length;
  } else if (parsed && typeof parsed.body === 'string') {
    o.encodedBodySize = o.decodedBodySize = parsed.body.length;
  }
  if (o.encodedBodySize) {
    // Response headers are real wire cost: approximate with the
    // serialized `name: value\r\n` pairs.
    let hb = 0;
    if (parsed && parsed.headers && typeof parsed.headers === 'object') {
      for (const [k, v] of Object.entries(parsed.headers)) hb += String(k).length + String(v).length + 4;
    }
    o.transferSize = o.encodedBodySize + hb;
  }
  if (opts) for (const k of Object.keys(opts)) o[k] = opts[k];
  p._recordResource(o);
}

if (typeof Headers === "undefined") {
  globalThis.Headers = class Headers {
    constructor(init={}) { this._h={}; if(init) { if(init instanceof Headers) { init.forEach((v,k)=>{this._h[k]=v;}); } else if(typeof init==="object") { for(const[k,v]of Object.entries(init)) this._h[k.toLowerCase()]=String(v); } }  __hideOwn(this);}
    get(n) { return this._h[n.toLowerCase()]??null; } set(n,v) { this._h[n.toLowerCase()]=String(v); }
    has(n) { return n.toLowerCase() in this._h; } delete(n) { delete this._h[n.toLowerCase()]; }
    append(n,v) { this._h[n.toLowerCase()]=String(v); }
    forEach(cb) { for(const[k,v] of Object.entries(this._h)) cb(v,k,this); }
    entries() { return Object.entries(this._h)[Symbol.iterator](); }
    keys() { return Object.keys(this._h)[Symbol.iterator](); }
    values() { return Object.values(this._h)[Symbol.iterator](); }
    [Symbol.iterator]() { return this.entries(); }
  };
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__recordFetchTiming', __recordFetchTiming);
  d('_arrayBufferFromBytes', _arrayBufferFromBytes);
  d('_base64ToUint8Array', _base64ToUint8Array);
  d('_bodyToUint8Array', _bodyToUint8Array);
  d('_bytesToBase64', _bytesToBase64);
  d('_bytesToBinaryString', _bytesToBinaryString);
  d('_concatBytes', _concatBytes);
  d('_dataUrlBytes', _dataUrlBytes);
  d('_installWasmStreamingFallback', _installWasmStreamingFallback);
  d('_raceAbortSignal', _raceAbortSignal);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
