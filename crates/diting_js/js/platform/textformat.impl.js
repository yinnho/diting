//! module: textformat
//! layer: L2
//! exposes: __diting_isTextFormat
//! requires-load: -
//! requires-call: -

// TextFormat behavior. The class shape — length 0 and the four readonly
// getters in Chrome's order — is generated from the edit-context IDL.
// The constructor argument is a dictionary, so the unsigned-long and enum
// conversions stay here. TextFormatUpdateEvent brands members through
// __diting_isTextFormat.
'use strict';

const slots = new WeakMap();
const STYLES = ['none', 'solid', 'dotted', 'dashed', 'wavy'];
const THICK = ['none', 'thin', 'thick'];

exports.ctor = function ctor(iface, target, ctx, args) {
  let init = args[0];
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') {
    P.fail(ctor, "The provided value is not of type 'TextFormatInit'.", ctx);
  }
  const o = init === undefined ? {} : init;
  const rd = (k) => ctx + ": Failed to read the '" + k + "' property from 'TextFormatInit'";
  const en = (k, list, tn) => {
    if (o[k] === undefined) return 'none';
    const s = P.conv.DOMString(o[k], rd(k), ctor);
    if (list.indexOf(s) < 0) P.fail(ctor, "The provided value '" + s + "' is not a valid enum value of type " + tn + '.', rd(k));
    return s;
  };
  slots.set(target, {
    rangeEnd: o.rangeEnd === undefined ? 0 : P.conv.unsignedLong(o.rangeEnd, rd('rangeEnd'), ctor),
    rangeStart: o.rangeStart === undefined ? 0 : P.conv.unsignedLong(o.rangeStart, rd('rangeStart'), ctor),
    underlineStyle: en('underlineStyle', STYLES, 'UnderlineStyle'),
    underlineThickness: en('underlineThickness', THICK, 'UnderlineThickness'),
  });
};

exports.get = function get(iface, member, target) {
  const s = target !== null && typeof target === 'object' ? slots.get(target) : undefined;
  if (!s) P.illegal(get);
  return s[member];
};

P.expose('__diting_isTextFormat', (o) => o !== null && typeof o === 'object' && slots.has(o));
if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_isTextFormat');
}
