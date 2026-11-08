//! module: uievent
//! layer: L2
//! exposes: UIEvent, MouseEvent, WheelEvent, KeyboardEvent, InputEvent, FocusEvent, CompositionEvent, PointerEvent, __diting_uiOffset
//! requires-load: legacy:Event
//! requires-call: legacy:EventTarget, legacy:InputDeviceCapabilities, legacy:DataTransfer

// UI event family (M4 knife 8): UIEvent / MouseEvent / WheelEvent /
// KeyboardEvent / InputEvent / FocusEvent / CompositionEvent / PointerEvent.
// The bootstrap classes kept a hand-picked subset of the init dictionary
// (`o.screenX||0`) and dropped everything else, so movementX, keyCode,
// tiltX, getModifierState ... could not exist at all (92 of 111 probe keys
// off). Every fact below is pinned by chrome/uievent-truth-145.json (real
// Chrome 145, uievent-probe.js) and diffed by uievent_family_matches_chrome_truth.
//
// Shape: the constructor is a plain function that builds the native Event with
// Reflect.construct(Event, ..., new.target) (so subclasses - legacy TouchEvent,
// DragEvent ... - keep working through super()), then fills a PRIVATE slot from
// the init dictionary in WebIDL order (ancestor dictionaries first, members
// lexicographic within a dictionary) with Chrome's per-type conversions and
// error wording. Member getters read the slot; `which` is derived exactly the
// way Chrome derives it. Own-member order is the table order = Chrome's.

const NativeEvent = Event;
const slots = new WeakMap();

const INPUT_TYPES = new Set(['insertText', 'insertReplacementText', 'insertLineBreak', 'insertParagraph', 'insertOrderedList',
  'insertUnorderedList', 'insertHorizontalRule', 'insertFromYank', 'insertFromDrop', 'insertFromPaste', 'insertFromPasteAsQuotation',
  'insertTranspose', 'insertCompositionText', 'insertLink', 'deleteWordBackward', 'deleteWordForward', 'deleteSoftLineBackward',
  'deleteSoftLineForward', 'deleteEntireSoftLine', 'deleteHardLineBackward', 'deleteHardLineForward', 'deleteByDrag', 'deleteByCut',
  'deleteContent', 'deleteContentBackward', 'deleteContentForward', 'historyUndo', 'historyRedo', 'formatBold', 'formatItalic',
  'formatUnderline', 'formatStrikeThrough', 'formatSuperscript', 'formatSubscript', 'formatJustifyFull', 'formatJustifyCenter',
  'formatJustifyRight', 'formatJustifyLeft', 'formatIndent', 'formatOutdent', 'formatRemove', 'formatSetBlockTextDirection',
  'formatSetInlineTextDirection', 'formatBackColor', 'formatFontColor', 'formatFontName']);

// ---- dictionary member conversions (WebIDL types the family uses)
function num(v, fn, pre) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre);
  return Number(v);
}
function wrap(n, bits, signed) {
  if (!isFinite(n)) return 0;
  const m = 2 ** bits;
  const t = ((Math.trunc(n) % m) + m) % m;
  return signed && t >= m / 2 ? t - m : t;
}
const CONV = {
  bool: (v) => !!v,
  long: (v, fn, pre) => wrap(num(v, fn, pre), 32, true),
  ulong: (v, fn, pre) => wrap(num(v, fn, pre), 32, false),
  short: (v, fn, pre) => wrap(num(v, fn, pre), 16, true),
  ushort: (v, fn, pre) => wrap(num(v, fn, pre), 16, false),
  // `double` that Chrome stores as an int (screenX/clientX...): non-finite
  // throws, the value is truncated toward zero but NOT wrapped (4294967301 stays)
  dtrunc: (v, fn, pre) => {
    const n = num(v, fn, pre);
    if (!isFinite(n)) P.fail(fn, 'The provided double value is non-finite.', pre);
    return Math.trunc(n);
  },
  double: (v, fn, pre) => {
    const n = num(v, fn, pre);
    if (!isFinite(n)) P.fail(fn, 'The provided double value is non-finite.', pre);
    return n;
  },
  float: (v, fn, pre) => {
    const n = num(v, fn, pre);
    if (!isFinite(n) || !isFinite(Math.fround(n))) P.fail(fn, 'The provided float value is non-finite.', pre);
    return Math.fround(n);
  },
  DOMString: (v, fn, pre) => P.conv.DOMString(v, pre, fn),
  // DOMString? : null and undefined both land as null (InputEvent.data)
  // InputEvent.inputType keeps only the Input Events level-2 vocabulary; any
  // other string reads back '' (truth conv.InputEvent.inputType)
  InputType: (v, fn, pre) => { const t = P.conv.DOMString(v, pre, fn); return INPUT_TYPES.has(t) ? t : ''; },
  DOMStringN: (v, fn, pre) => (v === null || v === undefined ? null : P.conv.DOMString(v, pre, fn)),
  Window: (v, fn, pre) => {
    if (v === null || v === undefined) return null;
    if (v === globalThis || (typeof v === 'object' && v.window === v)) return v;
    P.fail(fn, "Failed to convert value to 'Window'.", pre);
  },
  EventTarget: (v, fn, pre) => {
    if (v === null || v === undefined) return null;
    if (v === globalThis || v instanceof EventTarget) return v;
    P.fail(fn, "Failed to convert value to 'EventTarget'.", pre);
  },
  InputDeviceCapabilities: (v, fn, pre) => {
    if (v === null || v === undefined) return null;
    if (typeof InputDeviceCapabilities === 'function' && v instanceof InputDeviceCapabilities) return v;
    P.fail(fn, "Failed to convert value to 'InputDeviceCapabilities'.", pre);
  },
  DataTransferN: (v, fn, pre) => {
    if (v === null || v === undefined) return null;
    if (typeof DataTransfer === 'function' && v instanceof DataTransfer) return v;
    P.fail(fn, "Failed to convert value to 'DataTransfer'.", pre);
  },
};

// ---- dictionaries: name -> { dict, members: { member: [conv, default] } }
const MODS = ['ctrlKey', 'shiftKey', 'altKey', 'metaKey', 'modifierAltGraph', 'modifierCapsLock', 'modifierFn',
  'modifierFnLock', 'modifierHyper', 'modifierNumLock', 'modifierOS', 'modifierScrollLock', 'modifierSuper',
  'modifierSymbol', 'modifierSymbolLock'];
const D = {
  UI: { dict: 'UIEventInit', m: { detail: ['long', 0], sourceCapabilities: ['InputDeviceCapabilities', null], view: ['Window', null], which: ['ulong', 0] } },
  MOD: { dict: 'EventModifierInit', m: Object.fromEntries(MODS.map((k) => [k, ['bool', false]])) },
  MOUSE: { dict: 'MouseEventInit', m: {
    button: ['short', 0], buttons: ['ushort', 0], clientX: ['dtrunc', 0], clientY: ['dtrunc', 0],
    movementX: ['long', 0], movementY: ['long', 0], relatedTarget: ['EventTarget', null],
    screenX: ['dtrunc', 0], screenY: ['dtrunc', 0] } },
  WHEEL: { dict: 'WheelEventInit', m: { deltaMode: ['ulong', 0], deltaX: ['double', 0], deltaY: ['double', 0], deltaZ: ['double', 0] } },
  KEY: { dict: 'KeyboardEventInit', m: {
    charCode: ['long', 0], code: ['DOMString', ''], isComposing: ['bool', false], key: ['DOMString', ''],
    keyCode: ['long', 0], location: ['ulong', 0], repeat: ['bool', false] } },
  INPUT: { dict: 'InputEventInit', m: { data: ['DOMStringN', null], dataTransfer: ['DataTransferN', null], inputType: ['InputType', ''], isComposing: ['bool', false] } },
  FOCUS: { dict: 'FocusEventInit', m: { relatedTarget: ['EventTarget', null] } },
  COMP: { dict: 'CompositionEventInit', m: { data: ['DOMString', ''] } },
  // PointerEvent keeps fractional coordinates (MouseEvent truncates them)
  MOUSEP: { dict: 'MouseEventInit', m: {
    button: ['short', 0], buttons: ['ushort', 0], clientX: ['double', 0], clientY: ['double', 0],
    movementX: ['long', 0], movementY: ['long', 0], relatedTarget: ['EventTarget', null],
    screenX: ['double', 0], screenY: ['double', 0] } },
  POINTER: { dict: 'PointerEventInit', m: {
    altitudeAngle: ['double', undefined], azimuthAngle: ['double', undefined], height: ['double', 1],
    persistentDeviceId: ['long', 0], pointerId: ['long', 0], pointerType: ['DOMString', ''], pressure: ['float', 0],
    tangentialPressure: ['float', 0], tiltX: ['long', undefined], tiltY: ['long', undefined], twist: ['long', 0],
    width: ['double', 1], isPrimary: ['bool', false] } },
};
const CHAIN = {
  UIEvent: [D.UI], MouseEvent: [D.UI, D.MOD, D.MOUSE], WheelEvent: [D.UI, D.MOD, D.MOUSE, D.WHEEL],
  KeyboardEvent: [D.UI, D.MOD, D.KEY], InputEvent: [D.UI, D.INPUT], FocusEvent: [D.UI, D.FOCUS],
  CompositionEvent: [D.UI, D.COMP], PointerEvent: [D.UI, D.MOD, D.MOUSEP, D.POINTER],
};
const KIND = { UIEvent: 'ui', MouseEvent: 'mouse', WheelEvent: 'mouse', PointerEvent: 'mouse', KeyboardEvent: 'key', InputEvent: 'ui', FocusEvent: 'ui', CompositionEvent: 'ui' };

// ---- PointerEvent tilt <-> spherical angles (Pointer Events spec algorithms)
function tiltToSpherical(tx, ty) {
  const rad = Math.PI / 180;
  if (tx === 0 && ty === 0) return [Math.PI / 2, 0];
  // axis-aligned tilts keep the exact quarter-turn arithmetic (truth ptr.angles:
  // 0,45 -> altitude 0.7853981633974483, not ...484)
  if (tx === 0) return [Math.PI / 2 - Math.abs(ty * rad), ty > 0 ? Math.PI / 2 : 3 * Math.PI / 2];
  if (ty === 0) return [Math.PI / 2 - Math.abs(tx * rad), tx < 0 ? Math.PI : 0];
  if (Math.abs(tx) === 90 || Math.abs(ty) === 90) return [0, 0];
  // tan as sin/cos and altitude as atan2(1, sqrt(x*x+y*y)): the only
  // combination bit-identical to Chrome's C++ (libm) results across the
  // probe grid - V8's Math.tan differs in the last ulp (tan(30deg))
  const tX = Math.sin(tx * rad) / Math.cos(tx * rad), tY = Math.sin(ty * rad) / Math.cos(ty * rad);
  let az = Math.atan2(tY, tX);
  if (az < 0) az += 2 * Math.PI;
  return [Math.atan2(1, Math.sqrt(tX * tX + tY * tY)), az];
}
function sphericalToTilt(alt, az) {
  const deg = 180 / Math.PI, tanAlt = Math.tan(alt);
  const fix = (v) => (isNaN(v) ? 0 : Math.round(Math.atan(v) * deg));
  return [fix(Math.cos(az) / tanAlt), fix(Math.sin(az) / tanAlt)];
}

// ---- construction
function isObjectLike(v) { return (typeof v === 'object' && v !== null) || typeof v === 'function'; }
function construct(name, fnRef, args, nt) {
  const pre = "Failed to construct '" + name + "'";
  if (nt === undefined) P.fail(fnRef, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  if (args.length < 1) P.fail(fnRef, '1 argument required, but only 0 present.', pre);
  const type = P.conv.DOMString(args[0], pre, fnRef);
  const init = args[1];
  const lastDict = CHAIN[name][CHAIN[name].length - 1].dict;
  if (init !== undefined && init !== null && !isObjectLike(init)) {
    P.fail(fnRef, "The provided value is not of type '" + lastDict + "'.", pre);
  }
  const ev = Reflect.construct(NativeEvent, init === undefined || init === null ? [type] : [type, init], nt);
  const s = { cls: name };
  for (const lvl of CHAIN[name]) {
    for (const mem of Object.keys(lvl.m).sort()) {
      const [conv, dflt] = lvl.m[mem];
      const raw = init === undefined || init === null ? undefined : init[mem];
      s[mem] = raw === undefined ? dflt
        : CONV[conv](raw, fnRef, pre + ": Failed to read the '" + mem + "' property from '" + lvl.dict + "'");
    }
  }
  if (name === 'PointerEvent') {
    const hasTilt = s.tiltX !== undefined || s.tiltY !== undefined;
    const hasAng = s.altitudeAngle !== undefined || s.azimuthAngle !== undefined;
    if (hasAng && !hasTilt) {
      const [tx, ty] = sphericalToTilt(s.altitudeAngle === undefined ? Math.PI / 2 : s.altitudeAngle, s.azimuthAngle === undefined ? 0 : s.azimuthAngle);
      s.tiltX = tx; s.tiltY = ty;
    }
    if (s.tiltX === undefined) s.tiltX = 0;
    if (s.tiltY === undefined) s.tiltY = 0;
    if (!hasAng || s.altitudeAngle === undefined || s.azimuthAngle === undefined) {
      const [alt, az] = tiltToSpherical(s.tiltX, s.tiltY);
      if (s.altitudeAngle === undefined) s.altitudeAngle = alt;
      if (s.azimuthAngle === undefined) s.azimuthAngle = az;
    }
  }
  s.offX = undefined; s.offY = undefined; // filled by the dispatch hook (below)
  slots.set(ev, s);
  return ev;
}
function slot(o, fn, cls) {
  const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined;
  if (s === undefined) P.illegal(fn);
  void cls;
  return s;
}

// ---- generic definers
function getter(proto, name, read) {
  P.getter(proto, name, function __f() { return read(slot(this, __f), this); });
}
function constant(ctor, name, value) {
  Object.defineProperty(ctor, name, { value, enumerable: true });
  Object.defineProperty(ctor.prototype, name, { value, enumerable: true });
}
const scroll = (axis) => (axis === 'x' ? (globalThis.scrollX || globalThis.pageXOffset || 0) : (globalThis.scrollY || globalThis.pageYOffset || 0));
function initPrelude(ev, args, cls, method) {
  if (args.length < 1) P.fail(method, '1 argument required, but only 0 present.', "Failed to execute '" + method.name + "' on '" + cls + "'");
}
function legacyInit(ev, args) {
  // initEvent(type, bubbles, cancelable) semantics; ignored mid-dispatch
  if (ev.eventPhase !== 0) return false;
  ev.initEvent(P.conv.DOMString(args[0], '', legacyInit), !!args[1], !!args[2]);
  return true;
}

// ================= UIEvent
function UIEvent() { return construct('UIEvent', UIEvent, arguments, new.target); }
const UIP = P.iface(UIEvent, 'UIEvent', 1, NativeEvent);
getter(UIP, 'view', (s) => s.view);
getter(UIP, 'detail', (s) => s.detail);
getter(UIP, 'sourceCapabilities', (s) => s.sourceCapabilities);
// which: UIEvent itself reads 0; mouse-family events derive it from button,
// keyboard events from keyCode (truth which.mouse / which.key / which.other)
getter(UIP, 'which', (s) => (KIND[s.cls] === 'mouse' ? s.button + 1 : KIND[s.cls] === 'key' ? s.keyCode : 0));
P.method(UIP, 'initUIEvent', 1, function initUIEvent() {
  initPrelude(this, arguments, 'UIEvent', initUIEvent);
  const s = slot(this, initUIEvent);
  if (!legacyInit(this, arguments)) return;
  s.view = CONV.Window(arguments[3], initUIEvent, "Failed to execute 'initUIEvent' on 'UIEvent': parameter 4");
  s.detail = wrap(Number(arguments[4]), 32, true);
});
P.seal(UIEvent);

// ================= MouseEvent
function MouseEvent() { return construct('MouseEvent', MouseEvent, arguments, new.target); }
const MP = P.iface(MouseEvent, 'MouseEvent', 1, UIEvent);
for (const n of ['screenX', 'screenY', 'clientX', 'clientY', 'ctrlKey', 'shiftKey', 'altKey', 'metaKey', 'button', 'buttons', 'relatedTarget']) {
  getter(MP, n, (s) => s[n]);
}
getter(MP, 'pageX', (s) => s.clientX + scroll('x'));
getter(MP, 'pageY', (s) => s.clientY + scroll('y'));
getter(MP, 'x', (s) => s.clientX);
getter(MP, 'y', (s) => s.clientY);
// offset/layer: an undispatched event reads its page position (truth `layer`);
// the dispatch hook resolves offset against the target's paint transform
getter(MP, 'offsetX', (s) => (s.offX !== undefined ? s.offX : s.clientX + scroll('x')));
getter(MP, 'offsetY', (s) => (s.offY !== undefined ? s.offY : s.clientY + scroll('y')));
getter(MP, 'movementX', (s) => s.movementX);
getter(MP, 'movementY', (s) => s.movementY);
getter(MP, 'fromElement', () => null);
getter(MP, 'toElement', () => null);
getter(MP, 'layerX', (s) => (s.offX !== undefined ? s.offX : s.clientX + scroll('x')));
getter(MP, 'layerY', (s) => (s.offY !== undefined ? s.offY : s.clientY + scroll('y')));
const MODMAP = { Control: 'ctrlKey', Shift: 'shiftKey', Alt: 'altKey', Meta: 'metaKey' };
function getModifierState(cls) {
  return function getModifierState() {
    const pre = "Failed to execute 'getModifierState' on '" + cls + "'";
    const s = slot(this, getModifierState);
    if (arguments.length < 1) P.fail(getModifierState, '1 argument required, but only 0 present.', pre);
    const k = P.conv.DOMString(arguments[0], pre, getModifierState);
    if (MODMAP[k]) return s[MODMAP[k]];
    if (k === 'AltGraph' || k === 'CapsLock' || k === 'Fn' || k === 'FnLock' || k === 'Hyper' || k === 'NumLock' ||
        k === 'ScrollLock' || k === 'Super' || k === 'Symbol' || k === 'SymbolLock') return s['modifier' + k];
    return false;
  };
}
P.method(MP, 'getModifierState', 1, getModifierState('MouseEvent'));
P.method(MP, 'initMouseEvent', 1, function initMouseEvent() {
  initPrelude(this, arguments, 'MouseEvent', initMouseEvent);
  const s = slot(this, initMouseEvent);
  if (!legacyInit(this, arguments)) return;
  const a = arguments, pre = "Failed to execute 'initMouseEvent' on 'MouseEvent'";
  s.view = CONV.Window(a[3], initMouseEvent, pre + ': parameter 4');
  s.detail = wrap(Number(a[4]), 32, true);
  s.screenX = Math.trunc(Number(a[5]) || 0); s.screenY = Math.trunc(Number(a[6]) || 0);
  s.clientX = Math.trunc(Number(a[7]) || 0); s.clientY = Math.trunc(Number(a[8]) || 0);
  s.ctrlKey = !!a[9]; s.altKey = !!a[10]; s.shiftKey = !!a[11]; s.metaKey = !!a[12];
  s.button = wrap(Number(a[13]), 16, true);
  s.relatedTarget = CONV.EventTarget(a[14], initMouseEvent, pre + ': parameter 15');
});
P.seal(MouseEvent);

// ================= WheelEvent
function WheelEvent() { return construct('WheelEvent', WheelEvent, arguments, new.target); }
const WP = P.iface(WheelEvent, 'WheelEvent', 1, MouseEvent);
for (const n of ['deltaX', 'deltaY', 'deltaZ', 'deltaMode']) getter(WP, n, (s) => s[n]);
getter(WP, 'wheelDeltaX', (s) => Math.trunc(s.deltaX));
getter(WP, 'wheelDeltaY', (s) => Math.trunc(s.deltaY));
getter(WP, 'wheelDelta', (s) => (Math.trunc(s.deltaY) !== 0 ? Math.trunc(s.deltaY) : Math.trunc(s.deltaX)));
constant(WheelEvent, 'DOM_DELTA_PIXEL', 0);
constant(WheelEvent, 'DOM_DELTA_LINE', 1);
constant(WheelEvent, 'DOM_DELTA_PAGE', 2);
P.seal(WheelEvent);

// ================= KeyboardEvent
function KeyboardEvent() { return construct('KeyboardEvent', KeyboardEvent, arguments, new.target); }
const KP = P.iface(KeyboardEvent, 'KeyboardEvent', 1, UIEvent);
for (const n of ['key', 'code', 'location', 'ctrlKey', 'shiftKey', 'altKey', 'metaKey', 'repeat', 'isComposing', 'charCode', 'keyCode']) {
  getter(KP, n, (s) => s[n]);
}
constant(KeyboardEvent, 'DOM_KEY_LOCATION_STANDARD', 0);
constant(KeyboardEvent, 'DOM_KEY_LOCATION_LEFT', 1);
constant(KeyboardEvent, 'DOM_KEY_LOCATION_RIGHT', 2);
constant(KeyboardEvent, 'DOM_KEY_LOCATION_NUMPAD', 3);
P.method(KP, 'getModifierState', 1, getModifierState('KeyboardEvent'));
// Chrome's legacy initKeyboardEvent takes a key IDENTIFIER that it ignores:
// key stays '' (truth key.init.legacy), location and modifiers apply
P.method(KP, 'initKeyboardEvent', 1, function initKeyboardEvent() {
  initPrelude(this, arguments, 'KeyboardEvent', initKeyboardEvent);
  const s = slot(this, initKeyboardEvent);
  if (!legacyInit(this, arguments)) return;
  const a = arguments;
  s.view = CONV.Window(a[3], initKeyboardEvent, "Failed to execute 'initKeyboardEvent' on 'KeyboardEvent': parameter 4");
  s.location = wrap(Number(a[5]), 32, false);
  s.ctrlKey = !!a[6]; s.altKey = !!a[7]; s.shiftKey = !!a[8]; s.metaKey = !!a[9];
});
P.seal(KeyboardEvent);

// ================= InputEvent
function InputEvent() { return construct('InputEvent', InputEvent, arguments, new.target); }
const IP = P.iface(InputEvent, 'InputEvent', 1, UIEvent);
getter(IP, 'data', (s) => s.data);
getter(IP, 'isComposing', (s) => s.isComposing);
getter(IP, 'inputType', (s) => s.inputType);
getter(IP, 'dataTransfer', (s) => s.dataTransfer);
P.method(IP, 'getTargetRanges', 0, function getTargetRanges() { slot(this, getTargetRanges); return []; });
P.seal(InputEvent);

// ================= FocusEvent
function FocusEvent() { return construct('FocusEvent', FocusEvent, arguments, new.target); }
const FP = P.iface(FocusEvent, 'FocusEvent', 1, UIEvent);
getter(FP, 'relatedTarget', (s) => s.relatedTarget);
P.seal(FocusEvent);

// ================= CompositionEvent
function CompositionEvent() { return construct('CompositionEvent', CompositionEvent, arguments, new.target); }
const CP = P.iface(CompositionEvent, 'CompositionEvent', 1, UIEvent);
getter(CP, 'data', (s) => s.data);
P.method(CP, 'initCompositionEvent', 1, function initCompositionEvent() {
  initPrelude(this, arguments, 'CompositionEvent', initCompositionEvent);
  const s = slot(this, initCompositionEvent);
  if (!legacyInit(this, arguments)) return;
  s.view = CONV.Window(arguments[3], initCompositionEvent, "Failed to execute 'initCompositionEvent' on 'CompositionEvent': parameter 4");
  s.data = arguments[4] === undefined ? '' : P.conv.DOMString(arguments[4], '', initCompositionEvent);
});
P.seal(CompositionEvent);

// ================= PointerEvent (Chrome's own order has `constructor` mid-list
// and getCoalescedEvents after it - reproduced as is)
function PointerEvent() { return construct('PointerEvent', PointerEvent, arguments, new.target); }
const PP = P.iface(PointerEvent, 'PointerEvent', 1, MouseEvent);
for (const n of ['pointerId', 'width', 'height', 'pressure', 'tiltX', 'tiltY', 'azimuthAngle', 'altitudeAngle', 'tangentialPressure', 'twist', 'pointerType', 'isPrimary']) {
  getter(PP, n, (s) => s[n]);
}
P.method(PP, 'getPredictedEvents', 0, function getPredictedEvents() { slot(this, getPredictedEvents); return []; });
getter(PP, 'persistentDeviceId', (s) => s.persistentDeviceId);
P.seal(PointerEvent);
P.method(PP, 'getCoalescedEvents', 0, function getCoalescedEvents() { slot(this, getCoalescedEvents); return []; });

// ================= take over from the bootstrap classes
// 1. the old subclasses (TouchEvent, DragEvent, ...) re-parent onto the new
//    prototypes; their super() calls resolve through [[GetPrototypeOf]] of the
//    active function, so the move is enough.
// 2. publish the globals (non-enumerable, like every interface object).
const NEW = { UIEvent, MouseEvent, WheelEvent, KeyboardEvent, InputEvent, FocusEvent, CompositionEvent, PointerEvent };
const OLD = {};
for (const n of Object.keys(NEW)) OLD[n] = globalThis[n];
for (const key of Object.getOwnPropertyNames(globalThis)) {
  let C;
  try { C = globalThis[key]; } catch (e) { continue; }
  if (typeof C !== 'function' || !C.prototype || Object.values(NEW).includes(C)) continue;
  for (const n of Object.keys(NEW)) {
    const op = OLD[n] && OLD[n].prototype;
    if (op && Object.getPrototypeOf(C.prototype) === op) {
      Object.setPrototypeOf(C.prototype, NEW[n].prototype);
      Object.setPrototypeOf(C, NEW[n]);
    }
  }
}
for (const n of Object.keys(NEW)) P.expose(n, NEW[n]);

// The dispatch chokepoint (event_hooks.js origin()) resolves offsetX/offsetY
// against the target's paint transform. The members are read-only getters
// now, so the write goes through this hidden hook instead of an assignment.
P.expose('__diting_uiOffset', function (ev, x, y) {
  const s = slots.get(ev);
  if (s) { s.offX = x; s.offY = y; }
});
if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_uiOffset');
}
