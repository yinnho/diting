// Pointer / touch event chain, split out of bootstrap.js wholesale (god-file
// ratchet, pure move; JS-SURFACE-PLAN step 3): PointerEvent, the pointer
// capture registry that the session/CDP mouse chains retarget through
// (interact.rs: __diting_pointerCaptureFor / ...Release), Touch, TouchEvent.
// Loads right after cssom_sheets, before global_surface and the sweep, so
// the classes sit where they always did for every later script. The three
// __diting_* globals are registered into __diting_hide_list explicitly below
// (redundant with interface_sweep.js's final refresh, which also covers files
// loading before the sweep; kept so the intent is local).

// A weak `PointerEvent extends Event` used to squat on this name earlier in
// the bootstrap, which kept the real class below dead behind its
// typeof-guard. Chrome shape: pointerType defaults to '' (only real input
// carries 'mouse'/'pen'/'touch'), isPrimary defaults to false.
globalThis.PointerEvent = class PointerEvent extends MouseEvent {
  constructor(type, opts={}) {
    super(type, opts);
    const s = _evxGuard(_evx.get(this));
    s.pointerId = opts.pointerId || 0; s.width = opts.width || 1; s.height = opts.height || 1;
    s.pressure = opts.pressure || 0; s.pointerType = opts.pointerType !== undefined ? opts.pointerType : ''; s.isPrimary = !!opts.isPrimary;
  }
}
_evxProps(globalThis.PointerEvent, ['pointerId', 'width', 'height', 'pressure', 'pointerType', 'isPrimary']);

// Pointer capture (实地反馈 2026-10-28「出现滑块，拖不了」root cause): drag-slider CAPTCHAs and
// drag handles call setPointerCapture(pointerId) in their pointerdown
// handler and listen for pointermove ON THE HANDLE — the API being absent
// made the handler throw before the drag even started, and hit-testing
// each move would miss the handle anyway once it moved. The capture map
// below is the retargeting source for the session/CDP mouse chains
// (__diting_pointerCaptureFor in interact.rs): while a pointer is
// captured, moves and the up dispatch to the capture element with the
// real coordinates. Capture auto-releases on pointerup via
// __diting_pointerCaptureRelease (Chrome's implicit release).
if (typeof Element !== 'undefined' && Element.prototype && !Element.prototype.setPointerCapture) {
  const __ditingPtrCaps = new Map(); // pointerId -> capturing element
  globalThis.__diting_ptrCaps = __ditingPtrCaps;
  globalThis.__diting_pointerCaptureFor = function(id) {
    const el = __ditingPtrCaps.get(id);
    if (!el) return null;
    if (!el.isConnected) { __ditingPtrCaps.delete(id); return null; }
    return el;
  };
  globalThis.__diting_pointerCaptureRelease = function(id) {
    const el = __ditingPtrCaps.get(id);
    if (!el) return;
    __ditingPtrCaps.delete(id);
    try { el.dispatchEvent(new PointerEvent('lostpointercapture', { pointerId: id })); } catch (e) {}
  };
  Element.prototype.setPointerCapture = function(id) {
    __ditingPtrCaps.set(id, this);
    try { this.dispatchEvent(new PointerEvent('gotpointercapture', { pointerId: id })); } catch (e) {}
  };
  Element.prototype.releasePointerCapture = function(id) {
    if (__ditingPtrCaps.get(id) === this) globalThis.__diting_pointerCaptureRelease(id);
  };
  Element.prototype.hasPointerCapture = function(id) {
    return __ditingPtrCaps.get(id) === this;
  };
  _markNative(Element.prototype.setPointerCapture);
  _markNative(Element.prototype.releasePointerCapture);
  _markNative(Element.prototype.hasPointerCapture);
}

// Touch family (#156, CDP Input.dispatchTouchEvent): Chrome-exposed
// constructors with the DOM's field set. Touch is a data handle the
// dispatcher mints per point; TouchEvent carries the three read-only
// lists (touches / targetTouches / changedTouches).
globalThis.Touch = class Touch {
  constructor(opts = {}) {
    const s = __diting_liftSlot(this);
    s.identifier = opts.identifier || 0;
    s.target = opts.target || null;
    s.clientX = opts.clientX || 0;
    s.clientY = opts.clientY || 0;
    s.screenX = opts.screenX !== undefined ? opts.screenX : s.clientX;
    s.screenY = opts.screenY !== undefined ? opts.screenY : s.clientY;
    s.pageX = opts.pageX !== undefined ? opts.pageX : s.clientX;
    s.pageY = opts.pageY !== undefined ? opts.pageY : s.clientY;
    s.radiusX = opts.radiusX || 1;
    s.radiusY = opts.radiusY || 1;
    s.rotationAngle = opts.rotationAngle || 0;
    s.force = opts.force || 0;
  }
};
globalThis.TouchEvent = class TouchEvent extends UIEvent {
  constructor(type, opts = {}) {
    super(type, opts);
    _evx.set(this, {
      touches: opts.touches || [],
      targetTouches: opts.targetTouches || opts.touches || [],
      changedTouches: opts.changedTouches || [],
      altKey: !!opts.altKey,
      ctrlKey: !!opts.ctrlKey,
      metaKey: !!opts.metaKey,
      shiftKey: !!opts.shiftKey,
    });
  }
};
_evxRo(globalThis.TouchEvent, ['touches', 'targetTouches', 'changedTouches', 'altKey', 'ctrlKey', 'metaKey', 'shiftKey']);

globalThis.__diting_hide_list.push('__diting_ptrCaps', '__diting_pointerCaptureFor', '__diting_pointerCaptureRelease');
