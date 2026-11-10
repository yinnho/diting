//! module: cssunit
//! layer: L2
//! exposes: -
//! requires-load: -
//! requires-call: -

// issue #67 pilot (CSSUnitValue, the gen family): behavior only. The class
// shape — arity, per-arg double/DOMString conversion preamble, prototype
// accessors value/unit, expose — is generated from the cssom-typedom IDL in
// gen/cssunit.gen.js; this module validates the unit and stores the two
// slots. Every non-obvious fact is pinned by a key in
// tools/webplatform/chrome/cssunit-truth-157.json (real Chrome 157):
//   ctor.valstr / ctor.unitnum — unit is ascii-lowercased BEFORE the table
//     check and the error message keeps the ORIGINAL spelling, so 'PX' is a
//     legal unit storing "px" and a unit of 5 fails as "Invalid unit: 5"
//     (the hand-written face rejected 'PX' outright — migrated face bug).
//   set.value.nan — the value SETTER re-checks finiteness under the
//     "Failed to set the 'value' property on 'CSSUnitValue'" prefix.
//   set.unit — sloppy assignment to the read-only unit is a silent no-op
//     (getter-only property; strict callers throw V8's own message).
// The prototype parent CSSNumericValue is NOT wired here: platform_bundle
// loads before global_surface mints the CSSNumericValue stub, so the link
// comes from global_surface's AUDIT_CHAIN (["CSSUnitValue","CSSNumericValue"]),
// which runs after the mint in the same file.
'use strict';

// exactly the 64 CSS factory names (css_static_face 刀88 table, unchanged);
// 'number' is what typedom_math's CSSMath* kids carry, 'percent' included.
const UNITS = {};
for (const u of ['Hz', 'Q', 'cap', 'ch', 'cm', 'cqb', 'cqh', 'cqi', 'cqmax', 'cqmin', 'cqw', 'deg', 'dpcm', 'dpi', 'dppx', 'dvb', 'dvh', 'dvi', 'dvmax', 'dvmin', 'dvw', 'em', 'ex', 'fr', 'grad', 'ic', 'in', 'kHz', 'lh', 'lvb', 'lvh', 'lvi', 'lvmax', 'lvmin', 'lvw', 'mm', 'ms', 'number', 'pc', 'percent', 'pt', 'px', 'rad', 'rcap', 'rch', 'rem', 'rex', 'ric', 'rlh', 's', 'svb', 'svh', 'svi', 'svmax', 'svmin', 'svw', 'turn', 'vb', 'vh', 'vi', 'vmax', 'vmin', 'vw', 'x']) {
  UNITS[u.toLowerCase()] = true;
}

// SLOT LAYOUT IS A CROSS-FILE CONTRACT, not an implementation detail:
// typedom_math.js (~15 sites) reads x._v / x._u directly for the CSSMath*
// arithmetic and the family-wide toString, and 刀88 pins zero VISIBLE own
// props — so the slots are hidden own properties, same as the hand-written
// face this module replaced.
exports.ctor = function ctor(iface, self, ctx, args, parsed) {
  // parsed rides the B.ctor 5th parameter: [valueNumber, unitString]
  // already converted by the generated preamble.
  const unit = P.asciiLower(parsed[1]);
  if (!UNITS[unit]) P.fail(ctor, 'Invalid unit: ' + parsed[1], ctx);
  Object.defineProperty(self, '_v', { value: parsed[0], writable: true, enumerable: false, configurable: true });
  Object.defineProperty(self, '_u', { value: unit, writable: true, enumerable: false, configurable: true });
};

exports.get = function get(iface, member, self) {
  if (self === null || typeof self !== 'object' || self._u === undefined) P.illegal(get);
  return member === 'value' ? self._v : self._u;
};

exports.set = function set(iface, member, self, v) {
  if (member !== 'value' || self === null || typeof self !== 'object' || self._u === undefined) P.illegal(set);
  self._v = P.conv.double(v, "Failed to set the 'value' property on 'CSSUnitValue'", set);
};

// typedom_math (loads later, after global_surface) reads
// globalThis.CSSUnitValue at call time for its CSSMath* kids; the CSS
// factories in css_static_face.js do the same — nothing to wire here.
