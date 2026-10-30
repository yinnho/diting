// Speech recognition family + ClipboardChangeEvent probe (M4 knife 11).
// Chrome 145 headless: SpeechRecognition constructs, start() asynchronously
// fires a trusted SpeechRecognitionErrorEvent 'not-allowed' then a trusted
// plain 'end' (no 'start' event); stop()/abort() cancel the error and leave
// only 'end'. Runs in Chrome AND the engine; diffed key by key.
(async function () {
  var R = {};
  function norm(str) {
    if (!/^https?:/.test(location.origin)) return str;
    return str.split(location.href).join('#B').split(location.origin).join('#O');
  }
  function t(n, f) {
    try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(norm(JSON.stringify(v, function (k, x) { return typeof x === 'bigint' ? 'BIGINT:' + x : x; }))); }
    catch (e) { R[n] = norm('THROW ' + e.name + ': ' + String(e.message).slice(0, 240)); }
  }
  async function p(n, f) { try { var v = f(); R[n] = v && v.then ? await v : v; } catch (e) { R[n] = 'THROW ' + e.name + ': ' + e.message; } }
  var names = ['SpeechRecognition', 'webkitSpeechRecognition', 'SpeechRecognitionEvent', 'webkitSpeechRecognitionEvent', 'SpeechRecognitionErrorEvent', 'webkitSpeechRecognitionError', 'SpeechGrammarList', 'webkitSpeechGrammarList', 'SpeechGrammar', 'webkitSpeechGrammar', 'SpeechRecognitionResult', 'SpeechRecognitionResultList', 'SpeechRecognitionAlternative', 'SpeechRecognitionPhrase', 'ClipboardChangeEvent'];
  R.exist = {}; names.forEach(function (n) { R.exist[n] = typeof window[n]; });
  R.webkit = [webkitSpeechRecognition === SpeechRecognition, webkitSpeechRecognitionEvent === SpeechRecognitionEvent, webkitSpeechRecognitionError === SpeechRecognitionErrorEvent, webkitSpeechGrammarList === SpeechGrammarList, webkitSpeechGrammar === SpeechGrammar, webkitSpeechRecognition.name, Object.getOwnPropertyDescriptor(window, 'webkitSpeechRecognition').enumerable];
  R.shape = {};
  ['SpeechRecognition', 'SpeechRecognitionEvent', 'SpeechRecognitionErrorEvent', 'SpeechGrammarList', 'SpeechGrammar', 'SpeechRecognitionPhrase', 'ClipboardChangeEvent'].forEach(function (n) {
    var C = window[n], P = C.prototype, k = {};
    Object.getOwnPropertyNames(P).forEach(function (m) { var d = Object.getOwnPropertyDescriptor(P, m); k[m] = (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; });
    var sk = {}; Object.getOwnPropertyNames(C).forEach(function (m) { var d = Object.getOwnPropertyDescriptor(C, m); sk[m] = (d.get || d.set ? 'acc' : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; });
    R.shape[n] = { order: Object.getOwnPropertyNames(P), kinds: k, parent: Object.getPrototypeOf(C).name, len: C.length, statics: sk, tag: Object.prototype.toString.call(Object.create(P)) };
  });
  t('chain', function () { return [Object.getPrototypeOf(SpeechRecognition) === EventTarget, new SpeechRecognition() instanceof EventTarget, Object.getPrototypeOf(SpeechRecognitionEvent) === Event, Object.getPrototypeOf(SpeechRecognitionErrorEvent) === Event, Object.getPrototypeOf(ClipboardChangeEvent) === Event]; });
  // ---- SpeechRecognition
  t('sr.fresh', function () { var s = new SpeechRecognition(); return [s.lang, s.continuous, s.interimResults, s.maxAlternatives, s.grammars && s.grammars.constructor.name, s.grammars && s.grammars.length, s.onstart, s.onerror, s.onresult, Object.getOwnPropertyNames(s), s.processLocally, s.phrases && s.phrases.constructor.name, s.phrases && s.phrases.length, s.phrases === s.phrases, s.grammars === s.grammars]; });
  t('sr.set', function () { var s = new SpeechRecognition(); s.lang = 'zh-CN'; s.continuous = 1; s.interimResults = true; s.maxAlternatives = 3; return [s.lang, s.continuous, s.interimResults, s.maxAlternatives]; });
  t('sr.maxalt', function () { var s = new SpeechRecognition(), o = {}; [['str', '5'], ['neg', -1], ['big', 4294967297], ['nan', NaN], ['zero', 0]].forEach(function (c) { s.maxAlternatives = c[1]; o[c[0]] = s.maxAlternatives; }); return o; });
  t('sr.lang', function () { var s = new SpeechRecognition(); s.lang = null; var a = s.lang; s.lang = undefined; var b = s.lang; s.lang = 5; return [a, b, s.lang]; });
  t('sr.bools', function () { var s = new SpeechRecognition(); s.continuous = 'x'; s.interimResults = 0; s.processLocally = 1; return [s.continuous, s.interimResults, s.processLocally]; });
  t('sr.grammars.set', function () { var s = new SpeechRecognition(); var g = new SpeechGrammarList(); s.grammars = g; var r = [s.grammars === g]; try { s.grammars = {}; } catch (e) { r.push(e.name + ': ' + e.message); } return r; });
  t('sr.phrases.set', function () { var s = new SpeechRecognition(); try { s.phrases = []; return ['ok', s.phrases.length]; } catch (e) { return [e.name + ': ' + e.message]; } });
  t('sr.call', function () { return SpeechRecognition(); });
  t('sr.brand', function () { return SpeechRecognition.prototype.start.call({}); });
  t('sr.brand.get', function () { return Object.getOwnPropertyDescriptor(SpeechRecognition.prototype, 'lang').get.call({}); });
  t('handlers', function () { var s = new SpeechRecognition(), f = function () {}; s.onresult = f; s.onend = 5; return [s.onresult === f, s.onend]; });
  t('sr.statics', function () { return [SpeechRecognition.available.length, SpeechRecognition.install.length]; });
  await p('sr.available', function () { return SpeechRecognition.available({ langs: ['en-US'], processLocally: false }); });
  await p('sr.available.local', function () { return SpeechRecognition.available({ langs: ['en-US'], processLocally: true }); });
  await p('sr.available.noargs', function () { return SpeechRecognition.available(); });
  await p('sr.available.emptylangs', function () { return SpeechRecognition.available({ langs: [] }); });
  await p('sr.available.nolangs', function () { return SpeechRecognition.available({}); });
  await p('sr.install', function () { return SpeechRecognition.install({ langs: ['en-US'], processLocally: true }); });
  await p('sr.install.noargs', function () { return SpeechRecognition.install(); });
  await p('sr.install.nonlocal', function () { return SpeechRecognition.install({ langs: ['en-US'], processLocally: false }); });
  await p('sr.install.emptylangs', function () { return SpeechRecognition.install({ langs: [] }); });
  await p('sr.install.nolangs', function () { return SpeechRecognition.install({}); });
  await p('sr.available.langs.multi', function () { return SpeechRecognition.available({ langs: ['en-US', 'zh-CN'], processLocally: true }); });
  await p('sr.available.langs.bad', function () { return SpeechRecognition.available({ langs: 5 }); });
  await p('sr.available.opt.bad', function () { return SpeechRecognition.available(5); });
  await p('sr.available.local.str', function () { return SpeechRecognition.available({ langs: ['xx-ZZ'], processLocally: 'yes' }); });
  // ---- grammar list / grammar / phrase
  t('sgl', function () { var g = new SpeechGrammarList(); return [g.length, g.addFromString.length, typeof g.addFromUri, g.item.length, typeof g.addFromURI, g[0], 0 in g]; });
  t('sgl.add', function () { var g = new SpeechGrammarList(); g.addFromString('#JSGF V1.0;', 0.5); return [g.length, g[0].src, g[0].weight, g.item(0) === g[0], g.item(5), g[0] instanceof SpeechGrammar]; });
  t('sgl.add.w', function () { var g = new SpeechGrammarList(); g.addFromString('a'); g.addFromString('b', 0.2); g.addFromUri('http://x.test/g.xml', 0.7); g.addFromUri('rel.xml'); return [g.length, g[0].weight, g[1].weight, g[1].src, g[2].src, g[2].weight, g[3].src, Object.keys(g)]; });
  t('sgl.weight', function () { var g = new SpeechGrammarList(), o = {}; [['neg', -1], ['big', 3], ['nan', NaN], ['str', '0.5']].forEach(function (c) { try { g.addFromString('x', c[1]); o[c[0]] = g[g.length - 1].weight; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('sgl.noargs', function () { var g = new SpeechGrammarList(), r = []; try { g.item(); } catch (e) { r.push(e.message); } try { g.addFromString(); } catch (e) { r.push(e.message); } try { g.addFromUri(); } catch (e) { r.push(e.message); } return r; });
  t('sgl.item.conv', function () { var g = new SpeechGrammarList(); g.addFromString('a'); return [g.item('0') === g[0], g.item(-1), g.item(1.9), g.item(NaN) === g[0]]; });
  t('grammar', function () { var g = new SpeechGrammar(); var a = [g.src, g.weight]; g.src = 'x'; g.weight = 0.3; a.push(g.src, g.weight); return a; });
  t('grammar.weight', function () { var g = new SpeechGrammar(), o = {}; [['neg', -1], ['big', 5], ['nan', NaN], ['str', '0.5']].forEach(function (c) { try { g.weight = c[1]; o[c[0]] = g.weight; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('phrase', function () { var p = new SpeechRecognitionPhrase('hello', 2), d = new SpeechRecognitionPhrase('x'); return [p.phrase, p.boost, d.phrase, d.boost]; });
  t('phrase.noargs', function () { return new SpeechRecognitionPhrase(); });
  t('phrase.boost', function () { var o = {}; [['neg', -1], ['big', 11], ['nan', NaN], ['str', '3'], ['ten', 10]].forEach(function (c) { try { o[c[0]] = new SpeechRecognitionPhrase('x', c[1]).boost; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  // ---- events
  t('sree', function () { var e = new SpeechRecognitionErrorEvent('error', { error: 'no-speech', message: 'm' }); return [e.error, e.message, e.type, e.bubbles, e.cancelable, Object.getOwnPropertyNames(e)]; });
  t('sree.vals', function () { var o = {}; [['none', {}], ['noerror', { message: 'm' }], ['bad', { error: 'zzz' }], ['num', { error: 5 }], ['null', { error: null }]].forEach(function (c) { var e = new SpeechRecognitionErrorEvent('error', c[1]); o[c[0]] = [e.error, e.message]; }); return o; });
  t('sree.noinit', function () { var e = new SpeechRecognitionErrorEvent('error'); return [e.error, e.message]; });
  t('sree.noargs', function () { return new SpeechRecognitionErrorEvent(); });
  t('sre.vals', function () { var o = {}; [['none', {}], ['idx', { resultIndex: 3 }], ['neg', { resultIndex: -1 }], ['str', { resultIndex: '7' }], ['results.obj', { results: {} }], ['results.null', { results: null }], ['results.arr', { results: [] }]].forEach(function (c) { try { var e = new SpeechRecognitionEvent('result', c[1]); o[c[0]] = [e.resultIndex, e.results === null ? null : typeof e.results]; } catch (er) { o[c[0]] = er.name + ': ' + er.message; } }); return o; });
  t('sre.noinit', function () { var e = new SpeechRecognitionEvent('result'); return [e.resultIndex, e.results, e.type]; });
  t('sre.noargs', function () { return new SpeechRecognitionEvent(); });
  // ---- start()/stop()/abort() sequences
  async function ev(label, fn) {
    var s = new SpeechRecognition(), log = [], hlog = [];
    ['audiostart', 'soundstart', 'speechstart', 'speechend', 'soundend', 'audioend', 'result', 'nomatch', 'error', 'start', 'end'].forEach(function (n) { s.addEventListener(n, function (e) { log.push(n + ':' + e.constructor.name + ':' + (e.error || '') + ':' + (e.message === undefined ? '' : e.message) + ':' + e.isTrusted + ':' + e.bubbles + ':' + e.cancelable + ':' + (e.target === s)); }); });
    s.onend = function () { hlog.push('onend'); }; s.onerror = function () { hlog.push('onerror'); };
    try { fn(s, log); } catch (e) { log.push('THROW ' + e.name + ': ' + e.message); }
    await new Promise(function (r) { setTimeout(r, 400); });
    R[label] = { log: log, hlog: hlog };
  }
  await ev('ev.start', function (s, log) { log.push('start->' + s.start()); });
  await ev('ev.twice', function (s, log) { s.start(); try { s.start(); } catch (e) { log.push('2nd THROW ' + e.name + ': ' + e.message); } });
  await ev('ev.stop', function (s) { s.stop(); });
  await ev('ev.abort', function (s) { s.abort(); });
  await ev('ev.startstop', function (s) { s.start(); s.stop(); });
  await ev('ev.startabort', function (s) { s.start(); s.abort(); });
  await ev('ev.restart', function (s) { s.start(); setTimeout(function () { try { s.start(); } catch (e) { R['restart.throw'] = e.message; } }, 150); });
  // ---- ClipboardChangeEvent(init) - no type argument; changeId is a BigInt
  t('cce.noargs', function () { var e = new ClipboardChangeEvent(); return [e.type, e.types, e.changeId, e instanceof Event, e.bubbles, e.cancelable, e.composed, e.isTrusted, Object.getOwnPropertyNames(e)]; });
  [['empty', '{}'], ['types', "{types:['a','b']}"], ['typesBad', '{types:5}'], ['changeId', '{changeId:5n}'], ['changeIdNum', '{changeId:5}'], ['changeIdStr', "{changeId:'7'}"], ['changeIdNeg', '{changeId:-1n}'], ['changeIdBig', '{changeId:2n**70n}'], ['bubbles', '{bubbles:true, cancelable:true}'], ['null', 'null'], ['undef', 'undefined'], ['str', "'s'"], ['num', '5'], ['fn', '(function(){})']].forEach(function (c) {
    t('cce.init.' + c[0], function () { var e = new ClipboardChangeEvent(eval('(' + c[1] + ')')); return [e.type, e.types, e.changeId, e.bubbles, e.cancelable]; });
  });
  t('cce.typesProps', function () { var e = new ClipboardChangeEvent({ types: ['a'] }); return [Object.isFrozen(e.types), Array.isArray(e.types), e.types === e.types]; });
  t('cce.typesCopy', function () { var a = ['x']; var e = new ClipboardChangeEvent({ types: a }); a.push('y'); return [e.types]; });
  t('cce.call', function () { return ClipboardChangeEvent(); });
  t('cce.brand', function () { return Object.getOwnPropertyDescriptor(ClipboardChangeEvent.prototype, 'changeId').get.call({}); });
  return JSON.stringify(R);
})()
