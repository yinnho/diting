//! module: speech
//! layer: L2
//! exposes: SpeechRecognition, webkitSpeechRecognition, SpeechRecognitionEvent, webkitSpeechRecognitionEvent, SpeechRecognitionErrorEvent, webkitSpeechRecognitionError, SpeechGrammarList, webkitSpeechGrammarList, SpeechGrammar, webkitSpeechGrammar, SpeechRecognitionPhrase, ClipboardChangeEvent
//! requires-load: legacy:Event, legacy:EventTarget, legacy:_eventState
//! requires-call: legacy:DOMException, legacy:setTimeout, legacy:clearTimeout, legacy:document, legacy:URL

// Web Speech recognition + ClipboardChangeEvent (M4 knife 11). Chrome 145
// headless has no recognizer and no permission UI: SpeechRecognition
// constructs, start() asynchronously fires a trusted SpeechRecognitionErrorEvent
// 'not-allowed' and then a trusted plain 'end' (never 'start'); stop()/abort()
// while started cancel the error and leave only 'end'. SpeechRecognitionResult/
// ResultList/Alternative are not exposed on window. The webkit* names are the
// SAME interface objects. Every fact is pinned by chrome/speech-truth-145.json
// (speech-probe.js) and diffed by speech_family_matches_chrome_truth.

// ---- conversions
function f32(v, fn, pre) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre);
  const n = Number(v);
  if (!isFinite(n) || !isFinite(Math.fround(n))) P.fail(fn, 'The provided float value is non-finite.', pre);
  return Math.fround(n);
}
function u32(v, pre, fn) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre);
  const n = Number(v);
  return isFinite(n) ? ((Math.trunc(n) % 4294967296) + 4294967296) % 4294967296 : 0;
}
function abs(v) {
  try { return new URL(v, document.baseURI || document.URL).href; } catch (e) { return v; }
}
function isObj(v) { return (typeof v === 'object' && v !== null) || typeof v === 'function'; }
function seq(v, fn, pre) {
  if (v === null || typeof v !== 'object' || typeof v[Symbol.iterator] !== 'function') {
    P.fail(fn, 'The provided value cannot be converted to a sequence.', pre);
  }
  return Array.from(v, (x) => P.conv.DOMString(x, pre, fn));
}
function trusted(ev) { _eventState(ev, 'isTrusted', true); return ev; }

// ================= SpeechRecognitionPhrase(phrase, boost = 1)
const phraseSlots = new WeakMap();
function SpeechRecognitionPhrase(phrase, boost) {
  const pre = "Failed to construct 'SpeechRecognitionPhrase'";
  if (new.target === undefined) P.fail(SpeechRecognitionPhrase, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  if (arguments.length < 1) P.fail(SpeechRecognitionPhrase, '1 argument required, but only 0 present.', pre);
  const text = P.conv.DOMString(phrase, pre, SpeechRecognitionPhrase);
  const b = boost === undefined ? 1 : f32(boost, SpeechRecognitionPhrase, pre);
  if (b < 0 || b > 10) {
    throw new DOMException(pre + ': Speech recognition phrase boost value must be inside the range [0, 10].', 'SyntaxError');
  }
  const o = Reflect.construct(Object, [], new.target);
  phraseSlots.set(o, { phrase: text, boost: b });
  return o;
}
const PhP = P.iface(SpeechRecognitionPhrase, 'SpeechRecognitionPhrase', 1);
for (const m of ['phrase', 'boost']) {
  P.getter(PhP, m, function __f() { const s = phraseSlots.get(this); if (!s) P.illegal(__f); return s[m]; });
}
P.seal(SpeechRecognitionPhrase);

// ================= SpeechGrammar (src: USVString resolved, weight: float)
const grammarSlots = new WeakMap();
function SpeechGrammar() {
  if (new.target === undefined) P.fail(SpeechGrammar, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'SpeechGrammar'");
  const o = Reflect.construct(Object, [], new.target);
  grammarSlots.set(o, { src: '', weight: 1 });
  return o;
}
const GP = P.iface(SpeechGrammar, 'SpeechGrammar', 0);
function gslot(o, fn) { const s = o !== null && typeof o === 'object' ? grammarSlots.get(o) : undefined; if (!s) P.illegal(fn); return s; }
P.accessor(GP, 'src', function __f() { return gslot(this, __f).src; },
  function __f(v) { gslot(this, __f).src = abs(P.conv.USVString(v, "Failed to set the 'src' property on 'SpeechGrammar'", __f)); });
P.accessor(GP, 'weight', function __f() { return gslot(this, __f).weight; },
  function __f(v) { gslot(this, __f).weight = f32(v, __f, "Failed to set the 'weight' property on 'SpeechGrammar'"); });
P.seal(SpeechGrammar);
function mkGrammar(src, weight) {
  const g = new SpeechGrammar();
  const s = grammarSlots.get(g); s.src = src; s.weight = weight;
  return g;
}

// ================= SpeechGrammarList (own enumerable index properties)
const listSlots = new WeakMap();
function SpeechGrammarList() {
  if (new.target === undefined) P.fail(SpeechGrammarList, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'SpeechGrammarList'");
  const o = Reflect.construct(Object, [], new.target);
  listSlots.set(o, []);
  return o;
}
const LP = P.iface(SpeechGrammarList, 'SpeechGrammarList', 0);
function lslot(o, fn) { const s = o !== null && typeof o === 'object' ? listSlots.get(o) : undefined; if (!s) P.illegal(fn); return s; }
function push(list, g) {
  const items = listSlots.get(list);
  Object.defineProperty(list, String(items.length), { value: g, writable: false, enumerable: true, configurable: true });
  items.push(g);
}
P.getter(LP, 'length', function __f() { return lslot(this, __f).length; });
P.method(LP, 'addFromString', 1, function __f(string, weight) {
  const pre = "Failed to execute 'addFromString' on 'SpeechGrammarList'";
  lslot(this, __f);
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  const s = P.conv.DOMString(string, pre, __f);
  const w = weight === undefined ? 1 : f32(weight, __f, pre);
  push(this, mkGrammar('data:application/xml,' + encodeURIComponent(s), w));
});
P.method(LP, 'addFromUri', 1, function __f(src, weight) {
  const pre = "Failed to execute 'addFromUri' on 'SpeechGrammarList'";
  lslot(this, __f);
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  const s = P.conv.USVString(src, pre, __f);
  const w = weight === undefined ? 1 : f32(weight, __f, pre);
  push(this, mkGrammar(abs(s), w));
});
P.method(LP, 'item', 1, function __f(index) {
  const pre = "Failed to execute 'item' on 'SpeechGrammarList'";
  const items = lslot(this, __f);
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  const i = u32(index, pre, __f);
  return i < items.length ? items[i] : null;
});
P.seal(SpeechGrammarList);

// ================= events
const errSlots = new WeakMap();
const resSlots = new WeakMap();
function SpeechRecognitionErrorEvent(type, init) {
  const pre = "Failed to construct 'SpeechRecognitionErrorEvent'";
  if (new.target === undefined) P.fail(SpeechRecognitionErrorEvent, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  if (arguments.length < 1) P.fail(SpeechRecognitionErrorEvent, '1 argument required, but only 0 present.', pre);
  const t = P.conv.DOMString(type, pre, SpeechRecognitionErrorEvent);
  if (init !== undefined && init !== null && !isObj(init)) P.fail(SpeechRecognitionErrorEvent, "The provided value is not of type 'SpeechRecognitionErrorEventInit'.", pre);
  const o = init === undefined || init === null ? {} : init;
  const ev = Reflect.construct(Event, init === undefined || init === null ? [t] : [t, init], new.target);
  const ip = pre + ": Failed to read the '";
  errSlots.set(ev, {
    error: o.error === undefined ? '' : P.conv.DOMString(o.error, ip + "error' property from 'SpeechRecognitionErrorEventInit'", SpeechRecognitionErrorEvent),
    message: o.message === undefined ? '' : P.conv.DOMString(o.message, ip + "message' property from 'SpeechRecognitionErrorEventInit'", SpeechRecognitionErrorEvent),
  });
  return ev;
}
const EEP = P.iface(SpeechRecognitionErrorEvent, 'SpeechRecognitionErrorEvent', 1, Event);
for (const m of ['error', 'message']) {
  P.getter(EEP, m, function __f() { const s = errSlots.get(this); if (!s) P.illegal(__f); return s[m]; });
}
P.seal(SpeechRecognitionErrorEvent);

function SpeechRecognitionEvent(type, init) {
  const pre = "Failed to construct 'SpeechRecognitionEvent'";
  if (new.target === undefined) P.fail(SpeechRecognitionEvent, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  if (arguments.length < 1) P.fail(SpeechRecognitionEvent, '1 argument required, but only 0 present.', pre);
  const t = P.conv.DOMString(type, pre, SpeechRecognitionEvent);
  if (init !== undefined && init !== null && !isObj(init)) P.fail(SpeechRecognitionEvent, "The provided value is not of type 'SpeechRecognitionEventInit'.", pre);
  const o = init === undefined || init === null ? {} : init;
  const ev = Reflect.construct(Event, init === undefined || init === null ? [t] : [t, init], new.target);
  const ip = pre + ": Failed to read the '";
  const idx = o.resultIndex === undefined ? 0 : u32(o.resultIndex, ip + "resultIndex' property from 'SpeechRecognitionEventInit'", SpeechRecognitionEvent);
  // SpeechRecognitionResultList is not exposed on window: only null can be passed
  if (o.results !== undefined && o.results !== null) {
    P.fail(SpeechRecognitionEvent, "Failed to convert value to 'SpeechRecognitionResultList'.", ip + "results' property from 'SpeechRecognitionEventInit'");
  }
  resSlots.set(ev, { resultIndex: idx, results: null });
  return ev;
}
const REP = P.iface(SpeechRecognitionEvent, 'SpeechRecognitionEvent', 1, Event);
for (const m of ['resultIndex', 'results']) {
  P.getter(REP, m, function __f() { const s = resSlots.get(this); if (!s) P.illegal(__f); return s[m]; });
}
P.seal(SpeechRecognitionEvent);

// ================= SpeechRecognition
const recSlots = new WeakMap();
const HANDLERS = ['audiostart', 'soundstart', 'speechstart', 'speechend', 'soundend', 'audioend', 'result', 'nomatch', 'error', 'start', 'end'];
function SpeechRecognition() {
  if (new.target === undefined) P.fail(SpeechRecognition, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'SpeechRecognition'");
  const r = Reflect.construct(EventTarget, [], new.target);
  const h = {}; for (const n of HANDLERS) h[n] = null;
  recSlots.set(r, { lang: '', continuous: false, interimResults: false, maxAlternatives: 1, grammars: new SpeechGrammarList(),
    processLocally: false, phrases: [], handlers: h, started: false, timer: null });
  return r;
}
const RP = P.iface(SpeechRecognition, 'SpeechRecognition', 0, EventTarget);
function rslot(o, fn) { const s = o !== null && typeof o === 'object' ? recSlots.get(o) : undefined; if (!s) P.illegal(fn); return s; }
const rpre = (n) => "Failed to set the '" + n + "' property on 'SpeechRecognition'";
P.accessor(RP, 'grammars', function __f() { return rslot(this, __f).grammars; }, function __f(v) {
  const s = rslot(this, __f);
  if (v === null || typeof v !== 'object' || !listSlots.has(v)) P.fail(__f, "Failed to convert value to 'SpeechGrammarList'.", rpre('grammars'));
  s.grammars = v;
});
P.accessor(RP, 'lang', function __f() { return rslot(this, __f).lang; },
  function __f(v) { rslot(this, __f).lang = P.conv.DOMString(v, rpre('lang'), __f); });
P.accessor(RP, 'continuous', function __f() { return rslot(this, __f).continuous; }, function __f(v) { rslot(this, __f).continuous = !!v; });
P.accessor(RP, 'interimResults', function __f() { return rslot(this, __f).interimResults; }, function __f(v) { rslot(this, __f).interimResults = !!v; });
P.accessor(RP, 'maxAlternatives', function __f() { return rslot(this, __f).maxAlternatives; },
  function __f(v) { rslot(this, __f).maxAlternatives = u32(v, rpre('maxAlternatives'), __f); });
for (const n of HANDLERS) {
  P.accessor(RP, 'on' + n, function __f() { return rslot(this, __f).handlers[n]; },
    function __f(v) { rslot(this, __f).handlers[n] = typeof v === 'function' ? v : null; });
}
function fireEnd(r, s) {
  s.timer = setTimeout(function () {
    s.timer = null; s.started = false;
    r.dispatchEvent(trusted(new Event('end')));
  }, 0);
}
P.method(RP, 'abort', 0, function __f() {
  const s = rslot(this, __f);
  if (!s.started) return;
  if (s.timer !== null) clearTimeout(s.timer);
  fireEnd(this, s);
});
P.method(RP, 'start', 0, function __f() {
  const s = rslot(this, __f);
  if (s.started) {
    throw new DOMException("Failed to execute 'start' on 'SpeechRecognition': recognition has already started.", 'InvalidStateError');
  }
  s.started = true;
  const r = this;
  // no recognizer and no permission prompt: the error and the end follow on a later task
  s.timer = setTimeout(function () {
    s.timer = null;
    r.dispatchEvent(trusted(new SpeechRecognitionErrorEvent('error', { error: 'not-allowed' })));
    s.started = false;
    r.dispatchEvent(trusted(new Event('end')));
  }, 0);
});
P.method(RP, 'stop', 0, function __f() {
  const s = rslot(this, __f);
  if (!s.started) return;
  if (s.timer !== null) clearTimeout(s.timer);
  fireEnd(this, s);
});
P.accessor(RP, 'processLocally', function __f() { return rslot(this, __f).processLocally; }, function __f(v) { rslot(this, __f).processLocally = !!v; });
P.accessor(RP, 'phrases', function __f() { return rslot(this, __f).phrases; }, function __f(v) {
  const s = rslot(this, __f);
  const arr = seqAny(v, __f);
  for (const p of arr) {
    if (p === null || typeof p !== 'object' || !phraseSlots.has(p)) P.fail(__f, "Failed to convert value to 'SpeechRecognitionPhrase'.", rpre('phrases'));
  }
  s.phrases = arr;
});
function seqAny(v, fn) {
  if (v === null || typeof v !== 'object' || typeof v[Symbol.iterator] !== 'function') {
    P.fail(fn, 'The provided value cannot be converted to a sequence.', rpre('phrases'));
  }
  return Array.from(v);
}
P.seal(SpeechRecognition);

// statics: on-device availability. Known languages are 'downloadable' when
// processing locally (and installing needs a user gesture), everything is
// 'available' remotely, unknown local languages are 'unavailable'.
const LOCAL_LANGS = new Set(['en', 'zh', 'es', 'fr', 'de', 'ja', 'ko', 'it', 'pt', 'ru', 'ar', 'hi', 'nl', 'pl', 'tr', 'sv']);
function options(name, fn, args) {
  const pre = "Failed to execute '" + name + "' on 'SpeechRecognition'";
  if (args.length < 1) P.fail(fn, '1 argument required, but only 0 present.', pre);
  const o = args[0];
  if (!isObj(o)) P.fail(fn, "The provided value is not of type 'SpeechRecognitionOptions'.", pre);
  const lp = pre + ": Failed to read the 'langs' property from 'SpeechRecognitionOptions'";
  if (o.langs === undefined) P.fail(fn, 'Required member is undefined.', lp);
  const langs = seq(o.langs, fn, lp);
  if (langs.length === 0) P.fail(fn, 'Langs array cannot be empty.', pre);
  return { langs, local: !!o.processLocally };
}
function availability(o) {
  if (!o.local) return 'available';
  return o.langs.every((l) => LOCAL_LANGS.has(String(l).split('-')[0].toLowerCase())) ? 'downloadable' : 'unavailable';
}
P.stat(SpeechRecognition, 'available', 1, function available() {
  try { return Promise.resolve(availability(options('available', available, arguments))); } catch (e) { return Promise.reject(e); }
});
P.stat(SpeechRecognition, 'install', 1, function install() {
  try {
    const o = options('install', install, arguments);
    const a = availability(o);
    if (a === 'downloadable') {
      return Promise.reject(new DOMException("Failed to execute 'install' on 'SpeechRecognition': Requires handling a user gesture when availability is \"downloadable\".", 'NotAllowedError'));
    }
    return Promise.resolve(false);
  } catch (e) { return Promise.reject(e); }
});

// ================= ClipboardChangeEvent(init): no type argument (always
// 'clipboardchange'), types is a fresh frozen array per read, changeId a BigInt
const cceSlots = new WeakMap();
function ClipboardChangeEvent(init) {
  const pre = "Failed to construct 'ClipboardChangeEvent'";
  if (new.target === undefined) P.fail(ClipboardChangeEvent, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  if (init !== undefined && init !== null && !isObj(init)) P.fail(ClipboardChangeEvent, "The provided value is not of type 'ClipboardChangeEventInit'.", pre);
  const o = init === undefined || init === null ? {} : init;
  const ip = pre + ": Failed to read the '";
  let types = [];
  if (o.types !== undefined) types = seq(o.types, ClipboardChangeEvent, ip + "types' property from 'ClipboardChangeEventInit'");
  let id = 0n;
  if (o.changeId !== undefined) {
    const v = o.changeId;
    if (typeof v === 'number') P.fail(ClipboardChangeEvent, 'Cannot convert ' + v + ' to a BigInt', ip + "changeId' property from 'ClipboardChangeEventInit'");
    // signed, unbounded (truth init.changeIdNeg / changeIdBig); strings parse, numbers throw
    try { id = BigInt(v); } catch (e) {
      if (e instanceof RangeError || e instanceof SyntaxError || e instanceof TypeError) {
        P.fail(ClipboardChangeEvent, 'Cannot convert ' + String(v) + ' to a BigInt', ip + "changeId' property from 'ClipboardChangeEventInit'");
      }
      throw e;
    }
  }
  const ev = Reflect.construct(Event, ['clipboardchange', o], new.target);
  cceSlots.set(ev, { types, id });
  return ev;
}
const CP = P.iface(ClipboardChangeEvent, 'ClipboardChangeEvent', 0, Event);
P.getter(CP, 'types', function __f() { const s = cceSlots.get(this); if (!s) P.illegal(__f); return Object.freeze(s.types.slice()); });
P.getter(CP, 'changeId', function __f() { const s = cceSlots.get(this); if (!s) P.illegal(__f); return s.id; });
P.seal(ClipboardChangeEvent);

// ================= publish (webkit* names are the SAME objects)
const PUB = { SpeechRecognition, SpeechRecognitionEvent, SpeechRecognitionErrorEvent, SpeechGrammarList, SpeechGrammar,
  SpeechRecognitionPhrase, ClipboardChangeEvent };
for (const [n, C] of Object.entries(PUB)) P.expose(n, C);
P.expose('webkitSpeechRecognition', SpeechRecognition);
P.expose('webkitSpeechRecognitionEvent', SpeechRecognitionEvent);
P.expose('webkitSpeechRecognitionError', SpeechRecognitionErrorEvent);
P.expose('webkitSpeechGrammarList', SpeechGrammarList);
P.expose('webkitSpeechGrammar', SpeechGrammar);
