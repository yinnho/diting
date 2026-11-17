//! module: csskeyword
//! layer: L2
//! exposes: -
//! requires-load: -
//! requires-call: -

// CSSKeywordValue behavior. The class shape — one required USVString, the
// read-write value accessor in Chrome's order — is generated from the
// typed-om IDL in gen/csskeyword.gen.js. CSSStyleValue.parse of a keyword
// uses this constructor, so the source string typedom serialization reads
// is stored in the shared __ditingTOM map created by platform/core.js.
// Member order is the Chrome 145 shape audit.
'use strict';

const TOM = globalThis.__ditingTOM;

exports.ctor = function ctor(iface, target, ctx, args, parsed) {
  const value = parsed[0];
  TOM.set(target, { src: value, value: value });
};

exports.get = function get(iface, member, target) {
  const s = target !== null && typeof target === 'object' ? TOM.get(target) : undefined;
  if (!s || s.value === undefined) P.illegal(get);
  return s.value;
};

exports.set = function set(iface, member, target, v) {
  const s = target !== null && typeof target === 'object' ? TOM.get(target) : undefined;
  if (!s || s.value === undefined) P.illegal(set);
  const next = P.conv.USVString(v, "Failed to set the 'value' property on 'CSSKeywordValue'", set);
  s.value = next;
  s.src = next;
};
