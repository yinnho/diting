//! module: inputdevice
//! layer: L2
//! exposes: -
//! requires-load: -
//! requires-call: -

// InputDeviceCapabilities behavior. The class shape — length 0, the
// firesTouchEvents getter in Chrome's order — is generated from the
// input-device-capabilities IDL. The optional dictionary is not a simple
// argument, so the conversion stays here. Chrome 145 does not surface
// pointerMovementScrolls.
'use strict';

const slots = new WeakMap();

function isObj(v) {
  return v !== null && (typeof v === 'object' || typeof v === 'function');
}

exports.ctor = function ctor(iface, target, ctx, args) {
  let init = args[0];
  if (init === null) init = undefined;
  if (init !== undefined && !isObj(init)) {
    P.fail(ctor, "The provided value is not of type 'InputDeviceCapabilitiesInit'.", ctx);
  }
  slots.set(target, {
    firesTouchEvents: init === undefined ? false : !!init.firesTouchEvents,
  });
};

exports.get = function get(iface, member, target) {
  const s = target !== null && typeof target === 'object' ? slots.get(target) : undefined;
  if (!s) P.illegal(get);
  return s.firesTouchEvents;
};
