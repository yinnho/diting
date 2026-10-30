//! module: notification
//! layer: L2
//! exposes: Notification
//! requires-load: legacy:Event, legacy:EventTarget, legacy:_eventState
//! requires-call: legacy:DOMException, legacy:setTimeout, legacy:document, legacy:URL, legacy:structuredClone

// Web Notifications (M4 knife 10). Chrome 145 headless: permission starts
// 'default', the first requestPermission() settles 'denied' (and stays), and
// constructing a Notification without permission asynchronously fires a
// trusted plain `error` Event - the notification never shows. Every fact
// below is pinned by chrome/notification-truth-145.json (notification-probe.js)
// and diffed by notification_matches_chrome_truth.

let permission = 'default';
const slots = new WeakMap();

const PRE = "Failed to construct 'Notification'";

// ---- NotificationOptions conversions
function toUint64(v) {
  if (typeof v === 'symbol') P.fail(Notification, 'Cannot convert a Symbol value to a number', PRE + ": Failed to read the 'timestamp' property from 'NotificationOptions'");
  const n = Number(v);
  if (!isFinite(n)) return 0;
  return Number(BigInt.asUintN(64, BigInt(Math.trunc(n))));
}
function usv(v, name) {
  return P.conv.USVString(v, PRE + ": Failed to read the '" + name + "' property from 'NotificationOptions'", Notification);
}
function str(v, name) {
  return P.conv.DOMString(v, PRE + ": Failed to read the '" + name + "' property from 'NotificationOptions'", Notification);
}
function resolve(v) {
  if (v === '') return '';
  try { return new URL(v, document.baseURI || document.URL).href; } catch (e) { return ''; }
}
// VibratePattern = unsigned long | sequence<unsigned long>; stored frozen
function vibrate(v) {
  const toU32 = (x) => {
    const n = Number(x);
    return isFinite(n) ? ((Math.trunc(n) % 4294967296) + 4294967296) % 4294967296 : 0;
  };
  if (v !== null && typeof v === 'object' && typeof v[Symbol.iterator] === 'function') return Object.freeze(Array.from(v, toU32));
  return Object.freeze([toU32(v)]);
}

// first function/symbol reachable through plain objects, arrays, Maps and Sets
// (the value Chrome's serializer trips on), as its String() form
function uncloneable(v, depth) {
  if (typeof v === 'function' || typeof v === 'symbol') return String(v);
  if (v === null || typeof v !== 'object' || depth > 8) return undefined;
  const kids = v instanceof Map ? Array.from(v.entries()).flat()
    : v instanceof Set ? Array.from(v.values())
    : Array.isArray(v) ? v : Object.values(v);
  for (const k of kids) { const r = uncloneable(k, depth + 1); if (r !== undefined) return r; }
  return undefined;
}

function Notification(title, options) {
  if (new.target === undefined) P.fail(Notification, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", PRE);
  if (arguments.length < 1) P.fail(Notification, '1 argument required, but only 0 present.', PRE);
  const t = P.conv.DOMString(title, PRE, Notification);
  if (options !== undefined && options !== null && typeof options !== 'object' && typeof options !== 'function') {
    P.fail(Notification, "The provided value is not of type 'NotificationOptions'.", PRE);
  }
  const o = options === undefined || options === null ? {} : options;
  const s = { title: t, closed: false, handlers: { click: null, show: null, error: null, close: null } };
  // dictionary members in WebIDL (lexicographic) order
  const actions = o.actions;
  if (actions !== undefined) s.actionsLen = Array.from(actions).length;
  s.badge = o.badge === undefined ? '' : resolve(usv(o.badge, 'badge'));
  s.body = o.body === undefined ? '' : str(o.body, 'body');
  s.dataRaw = o.data;
  if (o.dir !== undefined) {
    const d = str(o.dir, 'dir');
    if (d !== 'auto' && d !== 'ltr' && d !== 'rtl') {
      P.fail(Notification, "The provided value '" + d + "' is not a valid enum value of type NotificationDirection.",
        PRE + ": Failed to read the 'dir' property from 'NotificationOptions'");
    }
    s.dir = d;
  } else s.dir = 'auto';
  s.icon = o.icon === undefined ? '' : resolve(usv(o.icon, 'icon'));
  if (o.image !== undefined) usv(o.image, 'image'); // read and discarded (not an instance member in 145)
  s.lang = o.lang === undefined ? '' : str(o.lang, 'lang');
  s.renotify = !!o.renotify;
  s.requireInteraction = !!o.requireInteraction;
  s.silent = !!o.silent;
  s.tag = o.tag === undefined ? '' : str(o.tag, 'tag');
  s.timestamp = o.timestamp === undefined ? Date.now() : toUint64(o.timestamp);
  s.vibrate = o.vibrate === undefined ? Object.freeze([]) : vibrate(o.vibrate);
  // cross-member validation (after conversion)
  if (s.silent && s.vibrate.length > 0) P.fail(Notification, 'Silent notifications must not specify vibration patterns.', PRE);
  if (s.renotify && s.tag === '') P.fail(Notification, 'Notifications which set the renotify flag must specify a non-empty tag.', PRE);
  if (s.actionsLen > 0) {
    P.fail(Notification, 'Actions are only supported for persistent notifications shown using ServiceWorkerRegistration.showNotification().', PRE);
  }
  // data: structured-cloned once at construction, cached
  if (s.dataRaw === undefined || s.dataRaw === null) s.data = null;
  else {
    try { s.data = structuredClone(s.dataRaw); }
    catch (e) {
      // Chrome names the offending value: "<String(value)> could not be cloned."
      const bad = uncloneable(s.dataRaw, 0);
      throw new DOMException(PRE + ': ' + (bad !== undefined ? bad + ' could not be cloned.' : (e && e.message ? e.message : 'could not be cloned.')), 'DataCloneError');
    }
  }
  s.actions = Object.freeze([]);
  const n = Reflect.construct(EventTarget, [], new.target);
  slots.set(n, s);
  // without permission the notification can never show: report it as a
  // trusted error event on a later task (even when close() ran meanwhile)
  if (permission !== 'granted') {
    setTimeout(function () {
      const e = new Event('error');
      _eventState(e, 'isTrusted', true);
      n.dispatchEvent(e);
    }, 0);
  }
  return n;
}
const NP = P.iface(Notification, 'Notification', 1, EventTarget);
function slot(o, fn) {
  const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined;
  if (s === undefined) P.illegal(fn);
  return s;
}
// Chrome's own order: the four handler attributes first, then the data members
for (const ev of ['click', 'show', 'error', 'close']) {
  P.accessor(NP, 'on' + ev,
    function __f() { return slot(this, __f).handlers[ev]; },
    function __f(v) { slot(this, __f).handlers[ev] = typeof v === 'function' ? v : null; });
}
for (const m of ['title', 'dir', 'lang', 'body', 'tag', 'icon', 'badge', 'vibrate', 'timestamp', 'renotify', 'silent', 'requireInteraction', 'data', 'actions']) {
  P.getter(NP, m, function __f() { return slot(this, __f)[m]; });
}
P.method(NP, 'close', 0, function __f() { slot(this, __f).closed = true; });
P.seal(Notification);

// statics, in Chrome's order: permission, maxActions, requestPermission
P.getter(Notification, 'permission', function __f() { return permission; });
P.getter(Notification, 'maxActions', function __f() { return 2; });
P.stat(Notification, 'requestPermission', 0, function requestPermission(callback) {
  if (callback !== undefined && typeof callback !== 'function') {
    P.fail(requestPermission, "parameter 1 is not of type 'Function'.", "Failed to execute 'requestPermission' on 'Notification'");
  }
  // headless Chrome has no prompt UI: the first request resolves 'denied' and sticks
  if (permission === 'default') permission = 'denied';
  const result = permission;
  if (typeof callback === 'function') {
    Promise.resolve().then(function () { try { callback(result); } catch (e) { /* reported by the host */ } });
  }
  return Promise.resolve(result);
});

P.expose('Notification', Notification);
