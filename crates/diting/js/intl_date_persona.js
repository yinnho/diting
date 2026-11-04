(function () { // @@SCOPE
// god-file split (刀27, Intl/Date persona family): the timezone/locale
// derivation chain (lang list -> TZ -> DTF args) + the Intl.DateTimeFormat
// proxy + the Date prototype wall-clock wraps (get/set families, toString
// forms, getTimezoneOffset) + the __ditingNativeOf disguise map + the
// Function.prototype.toString proxy that renders wrapped natives as
// [native code]. The map is ENTIRELY internal to this belt (declared here,
// every use here) — _markNative's registry is a different mechanism
// (_nativeFnAdd), so the bootstrap load-time marking sweep is untouched.
// Load-time installs: Intl.DateTimeFormat proxy, Date prototype wraps,
// Function.prototype.toString proxy — all member form. __ditingLangList's
// one external reader (navigator_persona.js) calls at runtime.
const _OrigDateTimeFormat = Intl.DateTimeFormat;
// Derive the reported timezone from the configured language so the persona
// stays coherent (zh-CN + Europe/Berlin — the old hardcoded default — is a
// mismatch risk engines notice).
// Accept-Language may arrive as a raw header ("zh-CN,zh;q=0.9,en;q=0.8").
// navigator.language must be a single BCP-47 tag and navigator.languages a
// tag list — a q-weighted string in either is a hard headless tell.
function __ditingLangList() {
  // CDP Emulation.setLocaleOverride pin outranks the persona language
  // (Chrome's locale override outranks the accept-language). Pin-over-
  // derivation, same shape as __diting_tz over __ditingTZFromLang.
  const pin = globalThis.__diting_locale;
  const raw = String((typeof pin === 'string' && pin.length) ? pin
    : (globalThis.__diting_lang || 'zh-CN'));
  const tags = raw.split(',').map((s) => s.split(';')[0].trim()).filter(Boolean);
  const out = tags.length ? tags : ['zh-CN'];
  if (out.length === 1 && out[0].indexOf('-') !== -1) out.push(out[0].split('-')[0]);
  return out;
}
function __ditingTZFromLang() {
  const lang = __ditingLangList()[0].toLowerCase();
  const map = {
    'zh': 'Asia/Shanghai', 'ja': 'Asia/Tokyo', 'ko': 'Asia/Seoul',
    'en': 'America/New_York', 'de': 'Europe/Berlin', 'fr': 'Europe/Paris',
    'es': 'Europe/Madrid', 'it': 'Europe/Rome', 'ru': 'Europe/Moscow',
    'pt': 'Europe/Lisbon', 'nl': 'Europe/Amsterdam', 'tr': 'Europe/Istanbul',
  };
  for (const k of Object.keys(map)) {
    if (lang.startsWith(k)) return map[k];
  }
  return 'Asia/Shanghai';
}
// CDP Emulation.setTimezoneOverride. Empty means the language-derived zone.
function __ditingTZ() {
  const pin = globalThis.__diting_tz;
  if (typeof pin === 'string' && pin.length) return pin;
  return __ditingTZFromLang();
}
// Intl's *default* locale comes from the process locale (V8/ICU follows
// LANG), not from the configured language — so on a host running under
// LANG=en_US, navigator.language says zh-CN while
// Intl.DateTimeFormat().resolvedOptions().locale says en-US. Same class of
// mismatch as the timezone one above, and an equally hard headless tell.
// The Rust side pins ICU's default for fresh isolates, but V8 caches the
// resolved default per-isolate after first Intl use, so the authoritative
// fix lives here: bind undefined/null locale args to the configured
// language. Explicit locales pass through untouched.
function __ditingLocaleArg(locales) {
  if (locales === undefined || locales === null) return __ditingLangList()[0];
  return locales;
}
// Wrapping a native constructor with a plain JS function is itself a
// fingerprint: Function.prototype.toString stops returning [native code],
// name/length drift (stock Intl constructors are all length 0), and
// prototype.constructor stops closing on Intl.X. A Proxy with only a
// construct trap forwards everything else to the native target — the spec
// resolves a callable proxy's source text from its target — so nothing has
// to be forged by hand. It also keeps "class constructor requires new":
// a plain wrapper is callable without new and silently returns an
// instance, the proxy's default apply forwards to the native and throws
// like stock.
// Registry backing the toString disguise below: wrapped proxy -> native
// original it stands in for.
const __ditingNativeOf = new Map();
// Shared per-call binding for the DateTimeFormat proxies.
function __ditingDTFArgs(args) {
  if (!args[1]) args[1] = {};
  if (!args[1].timeZone) args[1].timeZone = __ditingTZ();
  args[0] = __ditingLocaleArg(args[0]);
  return args;
}
// V8 names the proxy-trap frame in error stacks after the handler's
// constructor and the trap key: a plain object handler reads
// "Object.construct" between the caller and the native — a stack tell that
// only shows when the constructor throws (independent differ run, obscura#734
// thread). Carrying the traps on a class named after the wrapped constructor
// relabels the frame to match stock's constructor name. The extra frame is
// inherent to trapping in JS and cannot be removed, so this is a relabel.
function __ditingHandler(ctorName, traps) {
  const Carrier = { [ctorName]: class {} }[ctorName];
  return Object.assign(new Carrier(), traps);
}
Intl.DateTimeFormat = new Proxy(_OrigDateTimeFormat, __ditingHandler('DateTimeFormat', {
  construct(target, args, newTarget) {
    return Reflect.construct(target, __ditingDTFArgs(args), newTarget);
  },
  // ECMA-402 legacy: these constructors stay callable without new. The
  // proxy's default call forwarding resolves the legacy NewTarget to the
  // inner native, which skips the construct trap — so bind here too.
  apply(target, thisArg, args) {
    return Reflect.construct(target, __ditingDTFArgs(args), target);
  },
}));
__ditingNativeOf.set(Intl.DateTimeFormat, _OrigDateTimeFormat);
Object.defineProperty(_OrigDateTimeFormat.prototype, 'constructor', {
  value: Intl.DateTimeFormat, writable: true, enumerable: false, configurable: true,
});
const _origResolved = _OrigDateTimeFormat.prototype.resolvedOptions;
_OrigDateTimeFormat.prototype.resolvedOptions = new Proxy(_origResolved, {
  apply(target, thisArg, args) {
    const r = Reflect.apply(target, thisArg, args);
    if (r.timeZone === 'UTC') r.timeZone = __ditingTZ();
    return r;
  },
});
__ditingNativeOf.set(_OrigDateTimeFormat.prototype.resolvedOptions, _origResolved);
// Date's own clock still follows the host zone unless these methods read
// the same pin Intl uses. A proxy over the native keeps
// Function.prototype.toString on the original.
// #221: the DTF is cached per zone — the local-field getters below call
// this on every access and would otherwise allocate a fresh formatter per
// getHours() on hot paths.
let _ditingOffDTF = null, _ditingOffDTFTZ = '';
function __ditingOffsetMinutes(date) {
  const tz = __ditingTZ();
  if (_ditingOffDTFTZ !== tz) {
    _ditingOffDTF = new _OrigDateTimeFormat('en-US', {
      timeZone: tz, hourCycle: 'h23',
      year: 'numeric', month: '2-digit', day: '2-digit',
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
    _ditingOffDTFTZ = tz;
  }
  const parts = _ditingOffDTF.formatToParts(date);
  const pick = (t) => Number(parts.find((p) => p.type === t).value);
  let hour = pick('hour');
  if (hour === 24) hour = 0;
  const asUTC = Date.UTC(pick('year'), pick('month') - 1, pick('day'), hour, pick('minute'), pick('second'));
  return Math.round((date.getTime() - asUTC) / 60000);
}
function __ditingFormatZoned(date, kind) {
  if (Number.isNaN(date.getTime())) return 'Invalid Date';
  const tz = __ditingTZ();
  const dtf = new _OrigDateTimeFormat('en-US', {
    timeZone: tz, hourCycle: 'h23', weekday: 'short', year: 'numeric',
    month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit',
    timeZoneName: 'longOffset',
  });
  const bag = {};
  for (const p of dtf.formatToParts(date)) if (p.type !== 'literal') bag[p.type] = p.value;
  // Zone NAME is the one locale-dependent piece of Date.prototype.toString:
  // zh-Chrome prints "\u2026GMT+0800 (中国标准时间)" while weekday/month stay the
  // spec's English abbreviations. Chrome resolves it from the UI locale; we
  // take the persona's first language tag (same source as navigator.language).
  const uiLang = (typeof __ditingLangList === 'function' ? __ditingLangList()[0] : 'en-US');
  const long = new _OrigDateTimeFormat(uiLang, { timeZone: tz, timeZoneName: 'long' })
    .formatToParts(date).find((p) => p.type === 'timeZoneName');
  let off = bag.timeZoneName || 'GMT';
  off = off.replace(/GMT([+-])(\d{1,2}):(\d{2})/, (_, s, h, m) => 'GMT' + s + h.padStart(2, '0') + m);
  off = off.replace(/GMT([+-])(\d{1,2})$/, (_, s, h) => 'GMT' + s + h.padStart(2, '0') + '00');
  const name = long ? long.value : tz;
  // Date.prototype.toString zero-pads the day-of-month ("Oct 04", spec
  // step 9 of TimeString: ToTwoDigit). 'numeric' above yields "4", and the
  // unpadded form broke SSR text-node comparison on doudian (#203): the
  // server rendered "04", we rendered "4", React threw #418 and discarded
  // the whole server DOM for the client fallback path.
  const day = String(bag.day).padStart(2, '0');
  const clock = bag.hour + ':' + bag.minute + ':' + bag.second;
  const datePart = bag.weekday + ' ' + bag.month + ' ' + day + ' ' + bag.year;
  if (kind === 'date') return datePart;
  if (kind === 'time') return clock + ' ' + off + ' (' + name + ')';
  return datePart + ' ' + clock + ' ' + off + ' (' + name + ')';
}
function __ditingWrapDate(name, impl) {
  const orig = Date.prototype[name];
  if (typeof orig !== 'function') return;
  const wrapped = new Proxy(orig, {
    apply(target, thisArg, args) {
      if (!(thisArg instanceof Date)) return Reflect.apply(target, thisArg, args);
      return impl(target, thisArg, args);
    },
  });
  __ditingNativeOf.set(wrapped, orig);
  Date.prototype[name] = wrapped;
}
__ditingWrapDate('getTimezoneOffset', (_target, date) => __ditingOffsetMinutes(date));
__ditingWrapDate('toString', (_target, date) => __ditingFormatZoned(date, 'all'));
__ditingWrapDate('toDateString', (_target, date) => __ditingFormatZoned(date, 'date'));
__ditingWrapDate('toTimeString', (_target, date) => __ditingFormatZoned(date, 'time'));
__ditingWrapDate('toLocaleString', (target, date, args) => {
  const options = Object.assign({}, args[1]);
  if (!options.timeZone) options.timeZone = __ditingTZ();
  return Reflect.apply(target, date, [args[0], options]);
});
__ditingWrapDate('toLocaleDateString', (target, date, args) => {
  const options = Object.assign({}, args[1]);
  if (!options.timeZone) options.timeZone = __ditingTZ();
  return Reflect.apply(target, date, [args[0], options]);
});
__ditingWrapDate('toLocaleTimeString', (target, date, args) => {
  const options = Object.assign({}, args[1]);
  if (!options.timeZone) options.timeZone = __ditingTZ();
  return Reflect.apply(target, date, [args[0], options]);
});
// #221: the local-field surface must resolve in the persona zone, not the
// host's. Above, the formatter family (toString/…) already formats in
// __ditingTZ() — but the constructor's local-args form and the native
// getter/setter families still resolved in the HOST zone, so on a TZ=UTC
// deployment toString() said "13:06:07 GMT+0800" while getHours() on the
// same instant returned 5. Any page diffing the two faces saw the mismatch.
// Local wall = instant − offset(instant), offset DST-aware via the cached
// DTF above; reading getUTC* off the shifted instant yields exact persona
// fields, and inverting (wall via Date.UTC, plus one offset lookup at the
// target wall — the pre-transition convention V8 itself uses for walls
// inside a DST gap) rebuilds instants from persona fields.
const _OrigDate = Date;
const _ditingHostNativeOffset = __ditingNativeOf.get(Date.prototype.getTimezoneOffset);
const _ditingWall = new _OrigDate(NaN);
function __ditingWallMs(date) {
  const t = date.getTime();
  if (Number.isNaN(t)) { _ditingWall.setTime(NaN); return _ditingWall; }
  _ditingWall.setTime(t - __ditingOffsetMinutes(date) * 60000);
  return _ditingWall;
}
// Fast path: when the host zone's offset matches the persona zone at 8
// probes across a year, the native local-field methods are already
// persona-correct and nothing needs shifting (the dev box in Shanghai runs
// zero overhead; only a TZ-mismatched host pays the wrap cost). Revalidated
// whenever the pin changes, same cache discipline as the offset DTF.
let _ditingHostOK = false, _ditingHostOKTZ = '\0';
function __ditingHostMatchesTZ() {
  const tz = __ditingTZ();
  if (_ditingHostOKTZ !== tz) {
    _ditingHostOK = true;
    const base = Date.UTC(2026, 0, 15);
    for (let i = 0; i < 8; i++) {
      const probe = new _OrigDate(base + i * 45 * 86400000);
      if (__ditingOffsetMinutes(probe) !== _ditingHostNativeOffset.call(probe)) {
        _ditingHostOK = false;
        break;
      }
    }
    _ditingHostOKTZ = tz;
  }
  return _ditingHostOK;
}
for (const [local, utc] of [
  ['getFullYear', 'getUTCFullYear'], ['getMonth', 'getUTCMonth'],
  ['getDate', 'getUTCDate'], ['getDay', 'getUTCDay'],
  ['getHours', 'getUTCHours'], ['getMinutes', 'getUTCMinutes'],
  ['getSeconds', 'getUTCSeconds'], ['getMilliseconds', 'getUTCMilliseconds'],
]) {
  __ditingWrapDate(local, (target, date) => __ditingHostMatchesTZ()
    ? Reflect.apply(target, date, [])
    : __ditingWallMs(date)[utc]());
}
__ditingWrapDate('getYear', (target, date) => __ditingHostMatchesTZ()
  ? Reflect.apply(target, date, [])
  : __ditingWallMs(date).getUTCFullYear() - 1900);
// Setters: rebuild the full persona field vector (current values for the
// fields this setter doesn't mention — Date.UTC's own defaults never leak
// in because only present args overlay), then map back to an instant.
function __ditingZonedSet(date, args, writes) {
  const wall = __ditingWallMs(date);
  const f = [
    wall.getUTCFullYear(), wall.getUTCMonth(), wall.getUTCDate(),
    wall.getUTCHours(), wall.getUTCMinutes(), wall.getUTCSeconds(),
    wall.getUTCMilliseconds(),
  ];
  for (let i = 0; i < writes.length && i < args.length; i++) f[writes[i]] = args[i];
  const w = Date.UTC(f[0], f[1], f[2], f[3], f[4], f[5], f[6]);
  let t = NaN;
  if (!Number.isNaN(w)) {
    _ditingWall.setTime(w);
    t = w + __ditingOffsetMinutes(_ditingWall) * 60000;
  }
  date.setTime(t);
  return t;
}
const _ditingSetSpec = {
  setFullYear: [0, 1, 2], setMonth: [1, 2], setDate: [2],
  setHours: [3, 4, 5, 6], setMinutes: [4, 5, 6], setSeconds: [5, 6],
  setMilliseconds: [6],
};
for (const name of Object.keys(_ditingSetSpec)) {
  const writes = _ditingSetSpec[name];
  __ditingWrapDate(name, (target, date, args) => __ditingHostMatchesTZ()
    ? Reflect.apply(target, date, args)
    : __ditingZonedSet(date, args, writes));
}
__ditingWrapDate('setYear', (target, date, args) => {
  if (__ditingHostMatchesTZ()) return Reflect.apply(target, date, args);
  let y = Number(args[0]);
  if (y >= 0 && y <= 99) y += 1900;
  return __ditingZonedSet(date, [y], [0]);
});
// Constructor, local-args form: interpret the fields in the persona zone.
// Single-string/numeric/date/zero-arg forms pass through untouched, and
// Date.UTC's argument defaults mirror the constructor's own exactly.
Date = new Proxy(_OrigDate, __ditingHandler('Date', {
  construct(target, args, newTarget) {
    if (args.length >= 2 && !__ditingHostMatchesTZ()) {
      let y = Number(args[0]);
      if (y >= 0 && y <= 99) y += 1900;
      // Pass exactly the args the caller gave: an explicit undefined IS
      // "present" per spec and would NaN the whole time value.
      const parts = [y].concat(Array.prototype.slice.call(args, 1));
      const w = Date.UTC.apply(null, parts);
      let t = w;
      if (!Number.isNaN(w)) {
        _ditingWall.setTime(w);
        t = w + __ditingOffsetMinutes(_ditingWall) * 60000;
      }
      return Reflect.construct(target, [t], newTarget);
    }
    return Reflect.construct(target, args, newTarget);
  },
  apply(target, thisArg, args) {
    return __ditingFormatZoned(new _OrigDate(), 'all');
  },
}));
__ditingNativeOf.set(Date, _OrigDate);
Object.defineProperty(_OrigDate.prototype, 'constructor', {
  value: Date, writable: true, enumerable: false, configurable: true,
});
// The remaining (locales, options) Intl constructors get the same
// default-locale binding via the same proxy treatment.
(function() {
  const names = ['NumberFormat', 'Collator', 'PluralRules', 'RelativeTimeFormat',
                 'ListFormat', 'DisplayNames', 'Segmenter'];
  for (const n of names) {
    const Orig = Intl[n];
    if (typeof Orig !== 'function' || !Orig.prototype) continue;
    Intl[n] = new Proxy(Orig, __ditingHandler(n, {
      construct(target, args, newTarget) {
        args[0] = __ditingLocaleArg(args[0]);
        return Reflect.construct(target, args, newTarget);
      },
      apply(target, thisArg, args) {
        args[0] = __ditingLocaleArg(args[0]);
        return Reflect.construct(target, args, target);
      },
    }));
    __ditingNativeOf.set(Intl[n], Orig);
    Object.defineProperty(Orig.prototype, 'constructor', {
      value: Intl[n], writable: true, enumerable: false, configurable: true,
    });
  }
})();
// V8 renders Function.prototype.toString on a proxy over a native as the
// anonymous "function () { [native code] }" — stock shows the name
// ("function NumberFormat() { [native code] }"). Close that last gap by
// routing toString through a proxy of the real Function.prototype.toString
// that answers for the wrapped set from the original and reflects
// everything else untouched. This covers both Intl.X.toString() and a
// direct Function.prototype.toString.call(Intl.X); the disguise itself
// still renders native because the spec resolves a callable proxy's source
// text from its target.
const __ditingFnToString = Function.prototype.toString;
Function.prototype.toString = new Proxy(__ditingFnToString, {
  apply(target, thisArg, args) {
    if (__ditingNativeOf.has(thisArg)) {
      return __ditingFnToString.call(__ditingNativeOf.get(thisArg));
    }
    return Reflect.apply(target, thisArg, args);
  },
});


// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__ditingLangList', __ditingLangList);
  d('__ditingLocaleArg', __ditingLocaleArg);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
