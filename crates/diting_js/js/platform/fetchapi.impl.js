//! module: fetchapi
//! layer: L2
//! exposes: Request, Response, __diting_mkResponse, __diting_reqTake
//! requires-load: headers, legacy:TextEncoder, legacy:TextDecoder
//! requires-call: legacy:ReadableStream, legacy:Blob, legacy:File, legacy:FormData, legacy:URLSearchParams, legacy:AbortController, legacy:AbortSignal, legacy:URL, legacy:document, legacy:location

// Fetch-standard Request / Response (M4 knife 16). The bootstrap classes kept
// every field as an own property (headers, url, method ... were enumerable own
// data), validated almost nothing, never tracked bodyUsed, left Response.status
// unchecked, had no clone for streams, no static-method checks, no guard on
// Request/Response headers and decoded text() by the Content-Type charset
// (Chrome always decodes UTF-8). Pinned by chrome/reqres-truth-145.json
// (reqres-probe.js, 96 keys: shape, construction validation and wording,
// defaults, body-derived content-type, statics, clone, bodyUsed, streams)
// and diffed by reqres_match_chrome_truth.
//
// Body model: a body is null, {bytes} or {stream}. Byte bodies are extracted
// once at construction (string -> UTF-8, Blob/typed array -> bytes, FormData ->
// multipart, URLSearchParams -> form-urlencoded); a ReadableStream body is read
// on consumption. `used` is the disturbed bit: set by any consume or by the first
// pull of the lazily-built `body` stream.
//
// fetch.js talks to this module through two hidden hooks: __diting_mkResponse
// (builds a Response with the network-only fields type/url/redirected, which the
// public constructor ignores) and __diting_reqTake (hands a Request's body bytes
// to the transport, marking it used).

const H = require('headers');
const reqSlots = new WeakMap();
const resSlots = new WeakMap();

const TOKEN = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;
const ENUMS = {
  cache: ['default', 'no-store', 'reload', 'no-cache', 'force-cache', 'only-if-cached'],
  credentials: ['omit', 'same-origin', 'include'],
  duplex: ['half'],
  mode: ['navigate', 'same-origin', 'no-cors', 'cors'],
  priority: ['high', 'low', 'auto'],
  redirect: ['follow', 'error', 'manual'],
  referrerPolicy: ['', 'no-referrer', 'no-referrer-when-downgrade', 'same-origin', 'origin', 'strict-origin', 'origin-when-cross-origin', 'strict-origin-when-cross-origin', 'unsafe-url'],
};
const ENUM_TYPE = { cache: 'RequestCache', credentials: 'RequestCredentials', duplex: 'RequestDuplex', mode: 'RequestMode', priority: 'FetchPriority', redirect: 'RequestRedirect', referrerPolicy: 'ReferrerPolicy' };

function rangeFail(message, prefix) {
  const e = new RangeError(message);
  Object.defineProperty(e, 'message', { value: prefix + ': ' + message, writable: true, enumerable: true, configurable: true });
  throw e;
}
function docBase() { try { return document.baseURI; } catch (e) { return 'about:blank'; } }
function parseUrl(s, fn, pre) {
  try { return new URL(s, docBase()); } catch (e) { P.fail(fn, 'Failed to parse URL from ' + s, pre); }
}
const enc = new TextEncoder();
const utf8 = new TextDecoder();

// ---------------------------------------------------------------- body extraction
function toBytes(v) {
  if (v instanceof ArrayBuffer) return new Uint8Array(v.slice(0));
  return new Uint8Array(v.buffer.slice(v.byteOffset, v.byteOffset + v.byteLength));
}
function formEncode(fd) {
  const boundary = '----ditingFormBoundary' + Math.random().toString(16).slice(2) + Date.now().toString(16);
  const chunks = [];
  const str = (s) => enc.encode(s);
  for (const [k, v] of fd.entries()) {
    const name = String(k).replace(/"/g, '%22');
    if (v !== null && typeof v === 'object' && v._bytes instanceof Uint8Array) {
      chunks.push(str('--' + boundary + '\r\nContent-Disposition: form-data; name="' + name + '"; filename="' + String(v.name || 'blob').replace(/"/g, '%22') + '"\r\nContent-Type: ' + (v.type || 'application/octet-stream') + '\r\n\r\n'));
      chunks.push(v._bytes);
      chunks.push(str('\r\n'));
    } else chunks.push(str('--' + boundary + '\r\nContent-Disposition: form-data; name="' + name + '"\r\n\r\n' + String(v) + '\r\n'));
  }
  chunks.push(str('--' + boundary + '--\r\n'));
  let n = 0;
  for (const c of chunks) n += c.length;
  const out = new Uint8Array(n);
  let o = 0;
  for (const c of chunks) { out.set(c, o); o += c.length; }
  return { bytes: out, type: 'multipart/form-data; boundary=' + boundary };
}
// BodyInit -> {body, type}. `type` is the Content-Type the body implies (or null).
function extract(v, pre, fn) {
  if (v === undefined || v === null) return { body: null, type: null };
  if (typeof ReadableStream === 'function' && v instanceof ReadableStream) {
    if (v.locked) P.fail(fn, 'Cannot construct a body from a locked ReadableStream', pre);
    return { body: { stream: v }, type: null };
  }
  if (typeof Blob === 'function' && v instanceof Blob) return { body: { bytes: new Uint8Array(v._bytes || 0) }, type: v.type || null };
  if (v instanceof ArrayBuffer || ArrayBuffer.isView(v)) return { body: { bytes: toBytes(v) }, type: null };
  if (typeof FormData === 'function' && v instanceof FormData) { const f = formEncode(v); return { body: { bytes: f.bytes }, type: f.type }; }
  if (typeof URLSearchParams === 'function' && v instanceof URLSearchParams) return { body: { bytes: enc.encode(v.toString()) }, type: 'application/x-www-form-urlencoded;charset=UTF-8' };
  return { body: { bytes: enc.encode(P.conv.DOMString(v, pre, fn)) }, type: 'text/plain;charset=UTF-8' };
}

// ---------------------------------------------------------------- body consumption
function disturbed(rec) { return rec.used === true; }
function locked(rec) { return rec.body !== null && rec.body.stream !== undefined && rec.body.stream.locked === true; }
function ownStream(rec) {
  if (rec.body === null) return null;
  if (rec.body.stream === undefined) {
    const bytes = rec.body.bytes;
    let sent = false;
    rec.body.stream = new ReadableStream({
      pull(c) {
        rec.used = true;
        if (!sent) { sent = true; if (bytes.length) c.enqueue(new Uint8Array(bytes)); }
        c.close();
      },
    });
  }
  return rec.body.stream;
}
async function drain(rec) {
  if (rec.body === null) return new Uint8Array(0);
  if (rec.body.bytes !== undefined && rec.body.stream === undefined) return rec.body.bytes;
  if (rec.body.bytes !== undefined) return rec.body.bytes;
  const reader = rec.body.stream.getReader();
  const chunks = [];
  let n = 0;
  for (;;) {
    const r = await reader.read();
    if (r.done) break;
    const c = r.value instanceof Uint8Array ? r.value : ArrayBuffer.isView(r.value) || r.value instanceof ArrayBuffer ? toBytes(r.value) : enc.encode(String(r.value));
    chunks.push(c);
    n += c.length;
  }
  const out = new Uint8Array(n);
  let o = 0;
  for (const c of chunks) { out.set(c, o); o += c.length; }
  return out;
}
async function consume(rec, name, cls, fn) {
  const pre = "Failed to execute '" + name + "' on '" + cls + "'";
  if (disturbed(rec)) P.fail(fn, 'body stream already read', pre);
  if (locked(rec)) P.fail(fn, 'body stream is locked', pre);
  if (rec.body !== null) rec.used = true;
  return drain(rec);
}
function formFromBytes(bytes, ct, fn) {
  const t = String(ct || '');
  const fd = new FormData();
  if (/^\s*application\/x-www-form-urlencoded/i.test(t)) {
    for (const [k, v] of new URLSearchParams(utf8.decode(bytes))) fd.append(k, v);
    return fd;
  }
  const m = /^\s*multipart\/form-data\s*;.*boundary=(?:"([^"]+)"|([^;]+))/i.exec(t);
  if (!m) P.fail(fn, 'Failed to fetch');
  const latin = (u8) => { let s = ''; for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]); return s; };
  const bin = latin(bytes);
  const parts = bin.split('--' + (m[1] || m[2]).trim());
  const unq = (s) => s.replace(/\\(.)/g, '$1');
  const back = (s) => { const u = new Uint8Array(s.length); for (let i = 0; i < s.length; i++) u[i] = s.charCodeAt(i); return u; };
  for (let i = 1; i < parts.length; i++) {
    if (/^--(\r\n|\r|\n|$)/.test(parts[i])) break;
    const part = parts[i].replace(/^\r\n/, '');
    const sep = part.indexOf('\r\n\r\n');
    if (sep < 0) continue;
    const head = utf8.decode(back(part.slice(0, sep)));
    let content = part.slice(sep + 4);
    if (content.endsWith('\r\n')) content = content.slice(0, -2);
    const cd = /content-disposition:\s*form-data;([^\r\n]*)/i.exec(head);
    if (!cd) continue;
    const nm = /name="((?:[^"\\]|\\.)*)"/i.exec(cd[1]);
    if (!nm) continue;
    const fnm = /filename="((?:[^"\\]|\\.)*)"/i.exec(cd[1]);
    if (fnm) {
      const tm = /content-type:\s*([^\r\n]+)/i.exec(head);
      fd.append(unq(nm[1]), new File([back(content)], unq(fnm[1]) || 'blob', { type: tm ? tm[1].trim() : '' }));
    } else fd.append(unq(nm[1]), utf8.decode(back(content)));
  }
  return fd;
}
function cloneBody(rec, name, cls, fn) {
  const pre = "Failed to execute 'clone' on '" + cls + "'";
  if (disturbed(rec) || locked(rec)) P.fail(fn, cls + ' body is already used', pre);
  if (rec.body === null) return null;
  if (rec.body.stream !== undefined && rec.body.bytes === undefined) {
    const [a, b] = rec.body.stream.tee();
    rec.body.stream = a;
    return { stream: b };
  }
  return { bytes: new Uint8Array(rec.body.bytes) };
}

// Promise-returning methods reject (not throw) on a bad receiver.
function asyncMethod(proto, name, len, impl) {
  P.method(proto, name, len, function __f() {
    try { return impl.call(this, __f); } catch (e) { return Promise.reject(e); }
  });
}
function bodyMethods(proto, cls, slotOf, ctOf) {
  asyncMethod(proto, 'arrayBuffer', 0, function (fn) {
    const s = slotOf(this, fn);
    return consume(s.rec, 'arrayBuffer', cls, fn).then((b) => b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
  });
  asyncMethod(proto, 'blob', 0, function (fn) {
    const s = slotOf(this, fn);
    const ct = ctOf(s);
    return consume(s.rec, 'blob', cls, fn).then((b) => new Blob([b], { type: ct }));
  });
  P.method(proto, 'clone', 0, function __f() {
    const s = slotOf(this, __f);
    const body = cloneBody(s.rec, 'clone', cls, __f);
    return cls === 'Request' ? cloneRequest(this, s, body) : cloneResponse(this, s, body);
  });
  asyncMethod(proto, 'formData', 0, function (fn) {
    const s = slotOf(this, fn);
    const ct = ctOf(s);
    return consume(s.rec, 'formData', cls, fn).then((b) => formFromBytes(b, s.headers.get('content-type'), fn));
  });
  asyncMethod(proto, 'json', 0, function (fn) {
    const s = slotOf(this, fn);
    return consume(s.rec, 'json', cls, fn).then((b) => JSON.parse(utf8.decode(b)));
  });
  asyncMethod(proto, 'text', 0, function (fn) {
    const s = slotOf(this, fn);
    return consume(s.rec, 'text', cls, fn).then((b) => utf8.decode(b));
  });
}
function bodyGetters(proto, slotOf) {
  P.getter(proto, 'body', function __f() { const s = slotOf(this, __f); return ownStream(s.rec); });
  P.getter(proto, 'bodyUsed', function __f() { return disturbed(slotOf(this, __f).rec); });
}

// ---------------------------------------------------------------- Request
const RPRE = "Failed to construct 'Request'";
function reqSlot(o, fn) { const s = o !== null && typeof o === 'object' ? reqSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }

function followSignal(given) {
  const ac = new AbortController();
  if (given.aborted) ac.abort(given.reason);
  else given.addEventListener('abort', () => ac.abort(given.reason), { once: true });
  return ac.signal;
}
function readEnum(init, k, fn) {
  const v = init[k];
  if (v === undefined) return undefined;
  const s = P.conv.DOMString(v, RPRE + ": Failed to read the '" + k + "' property from 'RequestInit'", fn);
  if (ENUMS[k].indexOf(s) < 0) P.fail(fn, "The provided value '" + s + "' is not a valid enum value of type " + ENUM_TYPE[k] + '.', RPRE + ": Failed to read the '" + k + "' property from 'RequestInit'");
  return s;
}

function Request(input) {
  if (new.target === undefined) P.fail(Request, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", RPRE);
  if (arguments.length < 1) P.fail(Request, '1 argument required, but only 0 present.', RPRE);
  let init = arguments[1];
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') P.fail(Request, "The provided value is not of type 'RequestInit'.", RPRE);
  const i = init === undefined ? {} : init;
  const from = input !== null && typeof input === 'object' ? reqSlots.get(input) : undefined;
  let s;
  if (from !== undefined) {
    s = { url: from.url, method: from.method, headers: null, referrer: from.referrer, referrerPolicy: from.referrerPolicy, mode: from.mode,
      credentials: from.credentials, cache: from.cache, redirect: from.redirect, integrity: from.integrity, keepalive: from.keepalive,
      signal: null, rec: null };
  } else {
    const raw = P.conv.DOMString(input, RPRE, Request);
    const u = parseUrl(raw, Request, RPRE);
    if (u.username !== '' || u.password !== '') P.fail(Request, 'Request cannot be constructed from a URL that includes credentials: ' + raw, RPRE);
    s = { url: u.href, method: 'GET', headers: null, referrer: 'about:client', referrerPolicy: '', mode: 'cors', credentials: 'same-origin',
      cache: 'default', redirect: 'follow', integrity: '', keepalive: false, signal: null, rec: null };
  }
  // RequestInit members, lexicographic (WebIDL) order: each read exactly once
  const body = i.body;
  const cache = readEnum(i, 'cache', Request);
  const credentials = readEnum(i, 'credentials', Request);
  readEnum(i, 'duplex', Request);
  const headersInit = i.headers;
  const integrity = i.integrity === undefined ? undefined : P.conv.DOMString(i.integrity, RPRE + ": Failed to read the 'integrity' property from 'RequestInit'", Request);
  const keepalive = i.keepalive;
  let method = i.method === undefined ? undefined : P.conv.DOMString(i.method, RPRE + ": Failed to read the 'method' property from 'RequestInit'", Request);
  const mode = readEnum(i, 'mode', Request);
  readEnum(i, 'priority', Request);
  const redirect = readEnum(i, 'redirect', Request);
  const referrer = i.referrer === undefined ? undefined : P.conv.DOMString(i.referrer, RPRE + ": Failed to read the 'referrer' property from 'RequestInit'", Request);
  const referrerPolicy = readEnum(i, 'referrerPolicy', Request);
  const signal = i.signal;

  if (mode === 'navigate') P.fail(Request, "Cannot construct a Request with a RequestInit whose mode member is set as 'navigate'.", RPRE);
  if (method !== undefined) {
    if (!TOKEN.test(method)) P.fail(Request, "'" + method + "' is not a valid HTTP method.", RPRE);
    const up = method.toUpperCase();
    if (up === 'CONNECT' || up === 'TRACE' || up === 'TRACK') P.fail(Request, "'" + method + "' HTTP method is unsupported.", RPRE);
    s.method = /^(DELETE|GET|HEAD|OPTIONS|POST|PUT)$/.test(up) ? up : method;
  }
  if (referrer !== undefined) {
    if (referrer === '') s.referrer = '';
    else if (referrer === 'about:client') s.referrer = 'about:client';
    else {
      const ru = parseUrl(referrer, Request, RPRE);
      s.referrer = ru.origin === location.origin ? ru.href : 'about:client';
    }
  }
  if (referrerPolicy !== undefined) s.referrerPolicy = referrerPolicy;
  if (mode !== undefined) s.mode = mode;
  if (credentials !== undefined) s.credentials = credentials;
  if (cache !== undefined) s.cache = cache;
  if (redirect !== undefined) s.redirect = redirect;
  if (integrity !== undefined) s.integrity = integrity;
  if (keepalive !== undefined) s.keepalive = Boolean(keepalive);
  if (s.mode === 'no-cors' && s.method !== 'GET' && s.method !== 'HEAD' && s.method !== 'POST') P.fail(Request, "'" + s.method + "' is unsupported in no-cors mode.", RPRE);

  // signal: a fresh AbortSignal that follows the supplied (or inherited) one
  if (signal !== undefined && signal !== null) {
    if (typeof AbortSignal !== 'function' || !(signal instanceof AbortSignal)) P.fail(Request, "Failed to convert value to 'AbortSignal'.", RPRE + ": Failed to read the 'signal' property from 'RequestInit'");
    s.signal = followSignal(signal);
  } else if (signal === undefined && from !== undefined) s.signal = followSignal(from.signal);
  else s.signal = new AbortController().signal;

  // headers: replaced by init.headers, else copied from the input Request
  const guard = s.mode === 'no-cors' ? 'request-no-cors' : 'request';
  if (headersInit !== undefined) s.headers = H.make(headersInit, guard, RPRE, Request);
  else if (from !== undefined) s.headers = H.copy(from.headers, guard);
  else s.headers = H.make(undefined, guard, RPRE, Request);

  // body
  let rec;
  let type = null;
  if (body !== undefined && body !== null) {
    if (s.method === 'GET' || s.method === 'HEAD') P.fail(Request, 'Request with GET/HEAD method cannot have body.', RPRE);
    const x = extract(body, RPRE, Request);
    rec = { body: x.body, used: false };
    type = x.type;
  } else if (from !== undefined && from.rec.body !== null) {
    if (s.method === 'GET' || s.method === 'HEAD') P.fail(Request, 'Request with GET/HEAD method cannot have body.', RPRE);
    if (disturbed(from.rec) || locked(from.rec)) P.fail(Request, 'Cannot construct a Request with a Request object that has already been used.', RPRE);
    rec = { body: from.rec.body, used: false };
    from.rec.used = true;
  } else rec = { body: null, used: false };
  if (type !== null && !H.has(s.headers, 'content-type')) {
    if (s.mode !== 'no-cors' || /^(application\/x-www-form-urlencoded|multipart\/form-data|text\/plain)\s*(;|$)/i.test(type)) H.add(s.headers, 'content-type', type);
  }
  s.rec = rec;
  const self = Reflect.construct(Object, [], new.target);
  reqSlots.set(self, s);
  return self;
}
const RP = P.iface(Request, 'Request', 1);

function cloneRequest(self, s, body) {
  const c = Object.create(RP);
  reqSlots.set(c, { url: s.url, method: s.method, headers: H.copy(s.headers), referrer: s.referrer,
    referrerPolicy: s.referrerPolicy, mode: s.mode, credentials: s.credentials, cache: s.cache, redirect: s.redirect, integrity: s.integrity,
    keepalive: s.keepalive, signal: followSignal(s.signal), rec: { body, used: false } });
  return c;
}

for (const m of ['method', 'url']) P.getter(RP, m, function __f() { return reqSlot(this, __f)[m]; });
P.getter(RP, 'headers', function __f() { return reqSlot(this, __f).headers; });
P.getter(RP, 'destination', function __f() { reqSlot(this, __f); return ''; });
for (const m of ['referrer', 'referrerPolicy', 'mode', 'credentials', 'cache', 'redirect', 'integrity', 'keepalive', 'signal']) {
  P.getter(RP, m, function __f() { return reqSlot(this, __f)[m]; });
}
P.getter(RP, 'duplex', function __f() { reqSlot(this, __f); return 'half'; });
P.getter(RP, 'isHistoryNavigation', function __f() { reqSlot(this, __f); return false; });
P.getter(RP, 'bodyUsed', function __f() { return disturbed(reqSlot(this, __f).rec); });
// arrayBuffer, blob, clone, formData, json, text
bodyMethods(RP, 'Request', reqSlot, (s) => s.headers.get('content-type') || '');
P.getter(RP, 'targetAddressSpace', function __f() { reqSlot(this, __f); return 'unknown'; });
P.getter(RP, 'body', function __f() { return ownStream(reqSlot(this, __f).rec); });
asyncMethod(RP, 'bytes', 0, function (fn) {
  const s = reqSlot(this, fn);
  return consume(s.rec, 'bytes', 'Request', fn).then((b) => new Uint8Array(b));
});
P.seal(Request);

// ---------------------------------------------------------------- Response
const SPRE = "Failed to construct 'Response'";
function resSlot(o, fn) { const s = o !== null && typeof o === 'object' ? resSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
function ushort(v, pre, fn) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre);
  const n = Number(v);
  return isFinite(n) ? ((Math.trunc(n) % 65536) + 65536) % 65536 : 0;
}
function validStatusText(t) {
  for (let i = 0; i < t.length; i++) {
    const c = t.charCodeAt(i);
    if (!(c === 9 || c === 32 || (c >= 33 && c <= 126) || (c >= 128 && c <= 255))) return false;
  }
  return true;
}
const NULL_BODY = new Set([101, 204, 205, 304]);

function buildResponse(newTarget, bodyInit, init, pre, fn, forced) {
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') P.fail(fn, "The provided value is not of type 'ResponseInit'.", pre);
  const i = init === undefined ? {} : init;
  const rd = (k) => pre + ": Failed to read the '" + k + "' property from 'ResponseInit'";
  // ResponseInit, lexicographic: headers, status, statusText
  const headersInit = i.headers;
  const status = i.status === undefined ? 200 : ushort(i.status, rd('status'), fn);
  let statusText = '';
  if (i.statusText !== undefined) {
    statusText = P.conv.DOMString(i.statusText, rd('statusText'), fn);
    for (let k = 0; k < statusText.length; k++) if (statusText.charCodeAt(k) > 255) P.fail(fn, 'String contains non ISO-8859-1 code point.', rd('statusText'));
  }
  if (status < 200 || status > 599) rangeFail('The status provided (' + status + ') is outside the range [200, 599].', pre);
  if (!validStatusText(statusText)) P.fail(fn, 'Invalid statusText', pre);
  const headers = H.make(headersInit, 'response', pre, fn);
  let rec = { body: null, used: false };
  if (bodyInit !== undefined && bodyInit !== null) {
    if (NULL_BODY.has(status)) P.fail(fn, 'Response with null body status cannot have body', pre);
    const x = extract(bodyInit, pre, fn);
    rec = { body: x.body, used: false };
    const type = forced && forced.type ? forced.type : x.type;
    if (type !== null && !H.has(headers, 'content-type')) H.add(headers, 'content-type', type);
  }
  const self = Reflect.construct(Object, [], newTarget);
  resSlots.set(self, { status, statusText, headers, rec, type: 'default', url: '', redirected: false });
  return self;
}
function Response() {
  if (new.target === undefined) P.fail(Response, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", SPRE);
  return buildResponse(new.target, arguments[0], arguments[1], SPRE, Response);
}
const SP = P.iface(Response, 'Response', 0);

function cloneResponse(self, s, body) {
  const c = Object.create(SP);
  resSlots.set(c, { status: s.status, statusText: s.statusText, headers: H.copy(s.headers), rec: { body, used: false },
    type: s.type, url: s.url, redirected: s.redirected });
  return c;
}

P.getter(SP, 'type', function __f() { return resSlot(this, __f).type; });
P.getter(SP, 'url', function __f() { return resSlot(this, __f).url; });
P.getter(SP, 'redirected', function __f() { return resSlot(this, __f).redirected; });
P.getter(SP, 'status', function __f() { return resSlot(this, __f).status; });
P.getter(SP, 'ok', function __f() { const st = resSlot(this, __f).status; return st >= 200 && st <= 299; });
P.getter(SP, 'statusText', function __f() { return resSlot(this, __f).statusText; });
P.getter(SP, 'headers', function __f() { return resSlot(this, __f).headers; });
bodyGetters(SP, resSlot);
bodyMethods(SP, 'Response', resSlot, (s) => s.headers.get('content-type') || '');
asyncMethod(SP, 'bytes', 0, function (fn) {
  const s = resSlot(this, fn);
  return consume(s.rec, 'bytes', 'Response', fn).then((b) => new Uint8Array(b));
});
P.seal(Response);

const SX = "Failed to execute 'redirect' on 'Response'";
P.stat(Response, 'error', 0, function error() {
  const r = buildResponse(Response, null, { status: 200 }, SPRE, error);
  const s = resSlots.get(r);
  s.status = 0; s.type = 'error'; H.guard(s.headers, 'immutable');
  return r;
});
P.stat(Response, 'json', 1, function json(data) {
  if (arguments.length < 1) P.fail(json, '1 argument required, but only 0 present.', "Failed to execute 'json' on 'Response'");
  let text;
  try { text = JSON.stringify(data); } catch (e) {
    if (e instanceof TypeError && /BigInt/.test(e.message)) P.fail(json, e.message, "Failed to execute 'json' on 'Response'");
    throw e;
  }
  if (text === undefined) P.fail(json, 'The data is not JSON serializable', "Failed to execute 'json' on 'Response'");
  return buildResponse(Response, text, arguments[1], SPRE, json, { type: 'application/json' });
});
P.stat(Response, 'redirect', 1, function redirect(url) {
  if (arguments.length < 1) P.fail(redirect, '1 argument required, but only 0 present.', SX);
  const raw = P.conv.DOMString(url, SX, redirect);
  let status = 302;
  if (arguments[1] !== undefined) status = ushort(arguments[1], SX, redirect);
  const u = parseUrl(raw, redirect, SX);
  if (status !== 301 && status !== 302 && status !== 303 && status !== 307 && status !== 308) rangeFail('Invalid status code', SX);
  const r = buildResponse(Response, null, { status }, SPRE, redirect);
  const s = resSlots.get(r);
  H.add(s.headers, 'location', u.href);
  H.guard(s.headers, 'immutable');
  return r;
});

// ---------------------------------------------------------------- hooks for fetch.js / xhr.js
P.expose('__diting_mkResponse', function (body, init) {
  const o = init || {};
  const r = Reflect.construct(Object, [], Response);
  Object.setPrototypeOf(r, SP);
  const headers = H.make(undefined, 'response', SPRE, Response);
  const src = o.headers || {};
  for (const k of Object.keys(src)) H.addLenient(headers, k, String(src[k]));
  let bytes = null;
  if (body !== undefined && body !== null) bytes = typeof body === 'string' ? enc.encode(body) : body instanceof Uint8Array ? body : toBytes(body);
  resSlots.set(r, { status: o.status === undefined ? 200 : o.status, statusText: o.statusText || '', headers, rec: { body: bytes === null ? null : { bytes }, used: false },
    type: o.type || 'default', url: o.url || '', redirected: !!o.redirected });
  return r;
});
P.expose('__diting_reqTake', async function (req) {
  const s = reqSlots.get(req);
  if (s === undefined || s.rec.body === null) return null;
  if (disturbed(s.rec) || locked(s.rec)) throw new TypeError("Failed to execute 'fetch' on 'Window': Cannot construct a Request with a Request object that has already been used.");
  s.rec.used = true;
  return drain(s.rec);
});
if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_mkResponse', '__diting_reqTake');
}

P.expose('Request', Request);
P.expose('Response', Response);
