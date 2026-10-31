// 刀70: SpeechSynthesis voice/event behavior takeovers. The minted faces keep
// their prototype objects (sweep-tagged brands survive); only constructors
// and member bodies are replaced. Read the minted faces through globalThis
// (function declarations in this IIFE hoist over the global names - see
// videoframe_gaps.js).
(() => {
  const G = globalThis;
  const pin = (f, n) => { Object.defineProperty(f, 'name', { value: n, configurable: true }); return f; };
  // ---- SpeechSynthesisVoice: instances with WeakMap state, getters on the minted prototype
  const mintedVoice = G.SpeechSynthesisVoice;
  if (typeof mintedVoice === 'function' && mintedVoice.prototype) {
    const vp = mintedVoice.prototype;
    const vs = new WeakMap();
    const guard = (self) => { const s = (self !== null && typeof self === 'object') ? vs.get(self) : undefined; if (!s) throw new TypeError('Illegal invocation'); return s; };
    for (const k of ['voiceURI', 'name', 'lang', 'localService', 'default']) {
      Object.defineProperty(vp, k, { get: pin(function () { return guard(this)[k]; }, 'get ' + k), enumerable: true, configurable: true });
    }
    Object.defineProperty(G, '__diting_speechVoice', {
      value(name, lang, local, def) { const v = Object.create(vp); vs.set(v, { voiceURI: name, name, lang, localService: !!local, default: !!def }); return v; },
      enumerable: false, configurable: true, writable: true,
    });
    Object.defineProperty(G, '__diting_isSpeechVoice', { value: (o) => o !== null && typeof o === 'object' && vs.has(o), enumerable: false, configurable: true, writable: true });
  }
  // ---- SpeechSynthesisEvent / SpeechSynthesisErrorEvent: required-member ctor ladders
  const mintedEv = G.SpeechSynthesisEvent, mintedErr = G.SpeechSynthesisErrorEvent;
  if (typeof G.Event !== 'function' || typeof mintedEv !== 'function' || !mintedEv.prototype) return;
  const es = new WeakMap();
  const ERRS = ['canceled', 'interrupted', 'audio-busy', 'audio-hardware', 'network', 'synthesis-unavailable', 'synthesis-failed', 'language-unavailable', 'voice-unavailable', 'text-too-long', 'invalid-argument', 'not-allowed'];
  const ulong = (v, m, I, N) => {
    const n = Math.trunc(Number(v));
    if (!Number.isFinite(n) || n < 0 || n > 4294967295) throw new TypeError("Failed to construct '" + N + "': Failed to read the '" + m + "' property from '" + I + "': Value is outside the 'unsigned long' value range.");
    return n;
  };
  const build = (N, I, proto, parentCtor, withError) => {
    function Ctor(type, init) {
      if (!new.target) throw new TypeError("Failed to construct '" + N + "': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
      if (arguments.length < 2) throw new TypeError("Failed to construct '" + N + "': 2 arguments required, but only " + arguments.length + ' present.');
      if (init === null || typeof init !== 'object') throw new TypeError("Failed to construct '" + N + "': The provided value is not of type '" + I + "'.");
      if (init.utterance === undefined) throw new TypeError("Failed to construct '" + N + "': Failed to read the 'utterance' property from '" + I + "': Required member is undefined.");
      if (typeof G.__diting_isUtterance === 'function' && !G.__diting_isUtterance(init.utterance)) throw new TypeError("Failed to construct '" + N + "': Failed to read the 'utterance' property from '" + I + "': Failed to convert value to 'SpeechSynthesisUtterance'.");
      let error;
      if (withError) {
        if (init.error === undefined) throw new TypeError("Failed to construct '" + N + "': Failed to read the 'error' property from '" + I + "': Required member is undefined.");
        error = String(init.error);
        if (ERRS.indexOf(error) < 0) throw new TypeError("Failed to construct '" + N + "': Failed to read the 'error' property from '" + I + "': The provided value '" + error + "' is not a valid enum value of type SpeechSynthesisErrorCode.");
      }
      const ev = Reflect.construct(G.Event, [type, init], new.target);
      es.set(ev, {
        utterance: init.utterance, charIndex: init.charIndex === undefined ? 0 : ulong(init.charIndex, 'charIndex', I, N),
        charLength: init.charLength === undefined ? 0 : ulong(init.charLength, 'charLength', I, N),
        elapsedTime: init.elapsedTime === undefined ? 0 : Math.fround(Number(init.elapsedTime)),
        name: init.name === undefined ? '' : String(init.name), error,
      });
      return ev;
    }
    Ctor.prototype = proto;
    Object.defineProperty(Ctor, 'length', { value: 2 });
    Object.defineProperty(Ctor, 'name', { value: N });
    Object.defineProperty(proto, 'constructor', { value: Ctor, writable: true, enumerable: false, configurable: true });
    Object.setPrototypeOf(Ctor, parentCtor);
    const g = (self) => { const s = (self !== null && typeof self === 'object') ? es.get(self) : undefined; if (!s) throw new TypeError('Illegal invocation'); return s; };
    const keys = withError ? ['error'] : ['utterance', 'charIndex', 'charLength', 'elapsedTime', 'name'];
    for (const k of keys) Object.defineProperty(proto, k, { get: pin(function () { return g(this)[k]; }, 'get ' + k), enumerable: true, configurable: true });
    return Ctor;
  };
  const Ev = build('SpeechSynthesisEvent', 'SpeechSynthesisEventInit', mintedEv.prototype, G.Event, false);
  G.SpeechSynthesisEvent = Ev;
  Object.defineProperty(G, 'SpeechSynthesisEvent', { value: Ev, writable: true, configurable: true, enumerable: false });
  if (typeof mintedErr === 'function' && mintedErr.prototype) {
    const Er = build('SpeechSynthesisErrorEvent', 'SpeechSynthesisErrorEventInit', mintedErr.prototype, Ev, true);
    Object.defineProperty(G, 'SpeechSynthesisErrorEvent', { value: Er, writable: true, configurable: true, enumerable: false });
  }
})();
