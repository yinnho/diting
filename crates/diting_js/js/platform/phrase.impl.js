//! module: phrase
//! layer: L2
//! exposes: __diting_isSpeechPhrase
//! requires-load: -
//! requires-call: legacy:DOMException

// SpeechRecognitionPhrase behavior. The class shape — one required DOMString,
// the readonly phrase/boost getters in Chrome's order — is generated from
// the speech-api IDL in gen/phrase.gen.js. `float` is not a simple ctor
// type, so the boost conversion and the [0, 10] check stay here.
// Pinned by tools/webplatform/chrome/speech-truth-145.json.
'use strict';

const slots = new WeakMap();
const pre = "Failed to construct 'SpeechRecognitionPhrase'";

function f32(v, fn) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre);
  const n = Number(v);
  if (!isFinite(n) || !isFinite(Math.fround(n))) P.fail(fn, 'The provided float value is non-finite.', pre);
  return Math.fround(n);
}

exports.ctor = function ctor(iface, target, ctx, args) {
  const text = P.conv.DOMString(args[0], pre, ctor);
  const boost = args.length < 2 || args[1] === undefined ? 1 : f32(args[1], ctor);
  if (boost < 0 || boost > 10) {
    throw new DOMException(pre + ': Speech recognition phrase boost value must be inside the range [0, 10].', 'SyntaxError');
  }
  slots.set(target, { phrase: text, boost: boost });
};

exports.get = function get(iface, member, target) {
  const s = target !== null && typeof target === 'object' ? slots.get(target) : undefined;
  if (!s) P.illegal(get);
  return s[member];
};

P.expose('__diting_isSpeechPhrase', (o) => o !== null && typeof o === 'object' && slots.has(o));
if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_isSpeechPhrase');
}
