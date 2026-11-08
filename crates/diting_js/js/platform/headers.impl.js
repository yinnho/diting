//! module: headers
//! layer: L2
//! exposes: Headers
//! requires-load: -
//! requires-call: -

// Fetch-standard Headers (M4 knife 15). The bootstrap class was an object map
// keyed by lowercase name whose append() overwrote - duplicate headers were
// lost, nothing was validated or normalized, iteration was insertion-ordered
// and there was no getSetCookie. Pinned by chrome/headers-truth-145.json
// (headers-probe.js, 75 keys: construction corpus, name/value validity,
// normalization, duplicate combining, sorted live iteration, set-cookie,
// iterator protocol) and diffed by headers_match_chrome_truth.
//
// The header list keeps every entry in insertion order as {name (lowercase),
// value}. get() combines same-name values with ', '. Iteration is over the
// SORTED, COMBINED list (set-cookie entries stay separate) and is LIVE: every
// next()/forEach step re-derives the list and indexes into it, exactly as
// Chrome's iterator does (a header added mid-iteration is visited, a deleted
// one shifts the rest).

const lists = new WeakMap();
const TOKEN = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;
const ENUM_PRE = "Failed to construct 'Headers'";

function byteString(v, pre, fn) {
  const s = P.conv.DOMString(v, pre, fn);
  for (let i = 0; i < s.length; i++) {
    if (s.charCodeAt(i) > 255) P.fail(fn, 'String contains non ISO-8859-1 code point.', pre);
  }
  return s;
}
function normalizeValue(v) { return v.replace(/^[\t\n\r ]+|[\t\n\r ]+$/g, ''); }
function checkName(n, pre, fn) { if (!TOKEN.test(n)) P.fail(fn, 'Invalid name', pre); return n.toLowerCase(); }
function checkValue(v, pre, fn) {
  const s = normalizeValue(v);
  if (/[\u0000\r\n]/.test(s)) P.fail(fn, 'Invalid value', pre);
  return s;
}
function slot(o, fn) { const l = o !== null && typeof o === 'object' ? lists.get(o) : undefined; if (l === undefined) P.illegal(fn); return l; }

// sorted + combined view (set-cookie never combines)
function sortedView(list) {
  const names = [];
  for (const e of list) if (!names.includes(e.name)) names.push(e.name);
  names.sort();
  const out = [];
  for (const n of names) {
    if (n === 'set-cookie') { for (const e of list) if (e.name === n) out.push([n, e.value]); }
    else out.push([n, list.filter((e) => e.name === n).map((e) => e.value).join(', ')]);
  }
  return out;
}

function appendTo(list, n, v) { list.push({ name: n, value: v }); }
// Header guards (Fetch §5.2): Request.headers is 'request' (forbidden request-header
// names are silently dropped), 'request-no-cors' (only CORS-safelisted names/values),
// Response.headers is 'response' (Set-Cookie/Set-Cookie2 dropped), 'immutable'
// (Response.error()/redirect()) throws. A plain `new Headers()` is 'none'.
const guards = new WeakMap();
const FORBIDDEN_REQ = new Set(['accept-charset', 'accept-encoding', 'access-control-request-headers', 'access-control-request-method',
  'connection', 'content-length', 'cookie', 'cookie2', 'date', 'dnt', 'expect', 'host', 'keep-alive', 'origin', 'referer',
  'set-cookie', 'te', 'trailer', 'transfer-encoding', 'upgrade', 'via']);
function forbiddenReq(n) { return FORBIDDEN_REQ.has(n) || n.startsWith('proxy-') || n.startsWith('sec-'); }
function safelisted(n, v) {
  if (v.length > 128) return false;
  if (n === 'accept') return !/[\u0000-\u0008\u000a-\u001f"()/:<>?@[\\\]{}\u007f]/.test(v);
  if (n === 'accept-language' || n === 'content-language') return /^[0-9A-Za-z *,\-.;=]*$/.test(v);
  if (n === 'content-type') return /^(application\/x-www-form-urlencoded|multipart\/form-data|text\/plain)\s*(;|$)/i.test(v);
  if (n === 'range') return /^bytes=\d+-\d*$/.test(v);
  return false;
}
function blocked(guard, n, v) {
  if (guard === 'request') return forbiddenReq(n);
  if (guard === 'request-no-cors') return forbiddenReq(n) || !safelisted(n, v);
  if (guard === 'response') return n === 'set-cookie' || n === 'set-cookie2';
  return false;
}
function gappend(guard, list, n, v) { if (!blocked(guard, n, v)) appendTo(list, n, v); }
function guardOf(h) { return guards.get(h) || 'none'; }
function immutable(h, m, fn) { if (guardOf(h) === 'immutable') P.fail(fn, 'Headers are immutable', "Failed to execute '" + m + "' on 'Headers'"); }

function fill(list, init, fn, ENUM_PRE, guard) {
  const NOT = "The provided value is not of type '(record<ByteString, ByteString> or sequence<sequence<ByteString>>)'.";
  if (init === undefined) return;
  if (init === null || (typeof init !== 'object' && typeof init !== 'function')) P.fail(fn, NOT, ENUM_PRE);
  if (typeof init[Symbol.iterator] === 'function') {
    for (const pair of init) {
      if (pair === null || typeof pair !== 'object' || typeof pair[Symbol.iterator] !== 'function') {
        P.fail(fn, 'The provided value cannot be converted to a sequence.', ENUM_PRE);
      }
      const p = Array.from(pair);
      if (p.length !== 2) P.fail(fn, 'Invalid value', ENUM_PRE);
      const n = checkName(byteString(p[0], ENUM_PRE, fn), ENUM_PRE, fn);
      gappend(guard, list, n, checkValue(byteString(p[1], ENUM_PRE, fn), ENUM_PRE, fn));
    }
    return;
  }
  // record: own enumerable keys (symbols included - they fail on conversion)
  for (const k of Reflect.ownKeys(init)) {
    const d = Object.getOwnPropertyDescriptor(init, k);
    if (!d || !d.enumerable) continue;
    const n = checkName(byteString(k, ENUM_PRE, fn), ENUM_PRE, fn);
    gappend(guard, list, n, checkValue(byteString(init[k], ENUM_PRE, fn), ENUM_PRE, fn));
  }
}

function Headers() {
  if (new.target === undefined) P.fail(Headers, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", ENUM_PRE);
  const list = [];
  fill(list, arguments[0], Headers, ENUM_PRE, 'none');
  const h = Reflect.construct(Object, [], new.target);
  lists.set(h, list);
  return h;
}
const HP = P.iface(Headers, 'Headers', 0);

const pre = (m) => "Failed to execute '" + m + "' on 'Headers'";
function need(args, n, m, fn) {
  if (args.length < n) P.fail(fn, n + ' argument' + (n === 1 ? '' : 's') + ' required, but only ' + args.length + ' present.', pre(m));
}
P.method(HP, 'append', 2, function __f() {
  const l = slot(this, __f); need(arguments, 2, 'append', __f);
  const n = checkName(byteString(arguments[0], pre('append'), __f), pre('append'), __f);
  const v = checkValue(byteString(arguments[1], pre('append'), __f), pre('append'), __f);
  immutable(this, 'append', __f); gappend(guardOf(this), l, n, v);
});
P.method(HP, 'delete', 1, function __f() {
  const l = slot(this, __f); need(arguments, 1, 'delete', __f);
  const n = checkName(byteString(arguments[0], pre('delete'), __f), pre('delete'), __f);
  immutable(this, 'delete', __f);
  if (blocked(guardOf(this), n, '')) return;
  for (let i = l.length - 1; i >= 0; i--) if (l[i].name === n) l.splice(i, 1);
});
P.method(HP, 'get', 1, function __f() {
  const l = slot(this, __f); need(arguments, 1, 'get', __f);
  const n = checkName(byteString(arguments[0], pre('get'), __f), pre('get'), __f);
  const vs = l.filter((e) => e.name === n).map((e) => e.value);
  return vs.length === 0 ? null : vs.join(', ');
});
P.method(HP, 'getSetCookie', 0, function __f() {
  const l = slot(this, __f);
  return l.filter((e) => e.name === 'set-cookie').map((e) => e.value);
});
P.method(HP, 'has', 1, function __f() {
  const l = slot(this, __f); need(arguments, 1, 'has', __f);
  const n = checkName(byteString(arguments[0], pre('has'), __f), pre('has'), __f);
  return l.some((e) => e.name === n);
});
P.method(HP, 'set', 2, function __f() {
  const l = slot(this, __f); need(arguments, 2, 'set', __f);
  const n = checkName(byteString(arguments[0], pre('set'), __f), pre('set'), __f);
  const v = checkValue(byteString(arguments[1], pre('set'), __f), pre('set'), __f);
  immutable(this, 'set', __f);
  if (blocked(guardOf(this), n, v)) return;
  const first = l.findIndex((e) => e.name === n);
  if (first < 0) { appendTo(l, n, v); return; }
  l[first].value = v;
  for (let i = l.length - 1; i > first; i--) if (l[i].name === n) l.splice(i, 1);
});

// ---- iterator: live, index-based over the sorted combined view
const IterProto = Object.create(Object.getPrototypeOf(Object.getPrototypeOf([][Symbol.iterator]())));
const iterState = new WeakMap();
P.method(IterProto, 'next', 0, function __f() {
  const s = iterState.get(this);
  if (s === undefined) P.illegal(__f);
  if (s.done) return { value: undefined, done: true };
  const view = sortedView(s.list);
  if (s.index >= view.length) { s.done = true; return { value: undefined, done: true }; }
  const e = view[s.index++];
  return { value: s.kind === 'keys' ? e[0] : s.kind === 'values' ? e[1] : [e[0], e[1]], done: false };
});
Object.defineProperty(IterProto, Symbol.toStringTag, { value: 'Headers Iterator', configurable: true });
function mkIter(h, kind, fn) {
  const it = Object.create(IterProto);
  iterState.set(it, { list: slot(h, fn), index: 0, kind, done: false });
  return it;
}
P.method(HP, 'entries', 0, function __f() { return mkIter(this, 'entries', __f); });
P.method(HP, 'forEach', 1, function __f(cb, thisArg) {
  const l = slot(this, __f); need(arguments, 1, 'forEach', __f);
  if (typeof cb !== 'function') P.fail(__f, "parameter 1 is not of type 'Function'.", pre('forEach'));
  for (let i = 0; ; i++) {
    const view = sortedView(l);
    if (i >= view.length) break;
    cb.call(thisArg, view[i][1], view[i][0], this);
  }
});
P.method(HP, 'keys', 0, function __f() { return mkIter(this, 'keys', __f); });
P.method(HP, 'values', 0, function __f() { return mkIter(this, 'values', __f); });
P.seal(Headers);
Object.defineProperty(HP, Symbol.iterator, { value: HP.entries, writable: true, enumerable: false, configurable: true });

// Internal faces for Request/Response (fetchapi): build a guarded Headers with the
// caller's error prefix, and raw list access for body-derived content-type.
exports.make = function (init, guard, preText, fn) {
  const list = [];
  fill(list, init, fn, preText, guard);
  const h = Object.create(HP);
  lists.set(h, list);
  guards.set(h, guard);
  return h;
};
exports.has = function (h, name) { const l = lists.get(h); return l !== undefined && l.some((e) => e.name === name); };
exports.add = function (h, name, value) { const l = lists.get(h); if (l !== undefined) appendTo(l, name, value); };
exports.copy = function (h, guard) { const l = lists.get(h); const c = Object.create(HP); lists.set(c, l.map((e) => ({ name: e.name, value: e.value }))); guards.set(c, guard === undefined ? guardOf(h) : guard); return c; };
// network-built Response headers: bad names are skipped, never thrown
exports.addLenient = function (h, name, value) {
  const n = name.toLowerCase();
  if (!TOKEN.test(n)) return;
  const v = normalizeValue(value);
  if (/[\u0000\r\n]/.test(v) || blocked(guardOf(h), n, v)) return;
  appendTo(lists.get(h), n, v);
};
exports.guard = function (h, g) { guards.set(h, g); };

P.expose('Headers', Headers);
