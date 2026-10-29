// UI event family probe (M4 knife 8): UIEvent / MouseEvent / WheelEvent /
// KeyboardEvent / InputEvent / FocusEvent / CompositionEvent / PointerEvent.
// Runs in real Chrome AND the diting engine; diffed key by key.
(function () {
  var R = {};
  function t(n, f) { try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return typeof x === 'number' && !isFinite(x) ? '#' + String(x) : x; })); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 170); } }
  var CL = ['UIEvent', 'MouseEvent', 'WheelEvent', 'KeyboardEvent', 'InputEvent', 'FocusEvent', 'CompositionEvent', 'PointerEvent'];

  // ---- shape: own names + kinds + fn.length per prototype and per ctor
  R.shape = {};
  CL.forEach(function (n) {
    var C = window[n], P = C.prototype, o = {};
    var names = Object.getOwnPropertyNames(P);
    var kinds = {}, lens = {};
    names.forEach(function (m) {
      var d = Object.getOwnPropertyDescriptor(P, m);
      kinds[m] = d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value;
      if (typeof d.value === 'function') lens[m] = d.value.length;
    });
    R.shape[n] = { order: names, kinds: kinds, lens: lens, ctorLen: C.length, parent: Object.getPrototypeOf(C).name,
      protoParent: Object.getPrototypeOf(P).constructor.name, tag: Object.prototype.toString.call(Object.create(P)),
      statics: Object.getOwnPropertyNames(C) };
  });

  // ---- construction errors
  CL.forEach(function (n) {
    var C = window[n];
    t('ctor0.' + n, function () { return new C(); });
    t('call.' + n, function () { return C('x'); });
    t('badInit.' + n, function () { return new C('x', 5); });
    t('nullInit.' + n, function () { var e = new C('x', null); return [e.type, e.bubbles]; });
    t('undefInit.' + n, function () { var e = new C('x', undefined); return [e.type]; });
  });

  // ---- defaults of every dictionary-backed member
  var DEFS = {
    UIEvent: ['view', 'detail', 'sourceCapabilities', 'which'],
    MouseEvent: ['screenX', 'screenY', 'clientX', 'clientY', 'x', 'y', 'pageX', 'pageY', 'offsetX', 'offsetY', 'layerX', 'layerY', 'movementX', 'movementY', 'button', 'buttons', 'relatedTarget', 'ctrlKey', 'shiftKey', 'altKey', 'metaKey', 'which', 'detail', 'fromElement', 'toElement'],
    WheelEvent: ['deltaX', 'deltaY', 'deltaZ', 'deltaMode', 'wheelDelta', 'wheelDeltaX', 'wheelDeltaY', 'which'],
    KeyboardEvent: ['key', 'code', 'location', 'repeat', 'isComposing', 'charCode', 'keyCode', 'which', 'ctrlKey', 'shiftKey', 'altKey', 'metaKey'],
    InputEvent: ['data', 'isComposing', 'inputType', 'dataTransfer', 'which'],
    FocusEvent: ['relatedTarget', 'which'],
    CompositionEvent: ['data', 'which'],
    PointerEvent: ['pointerId', 'width', 'height', 'pressure', 'tangentialPressure', 'tiltX', 'tiltY', 'twist', 'altitudeAngle', 'azimuthAngle', 'pointerType', 'isPrimary', 'persistentDeviceId', 'button', 'buttons', 'which', 'pageX', 'movementX', 'layerX']
  };
  Object.keys(DEFS).forEach(function (n) {
    t('def.' + n, function () { var e = new window[n]('x'), o = {}; DEFS[n].forEach(function (m) { var v = e[m]; o[m] = v === undefined ? '#undefined' : typeof v === 'object' && v !== null ? 'OBJ:' + v.constructor.name : v; }); return o; });
  });

  // ---- dictionary conversions: value -> member read-back
  var CONV = [['num', 5], ['str', '7'], ['strABC', 'abc'], ['neg', -3], ['float', 2.7], ['big', 4294967296 + 5], ['nan', NaN], ['bool', true], ['null', null], ['undef', undefined], ['obj', {}]];
  function conv(cls, member, vals) {
    t('conv.' + cls + '.' + member, function () {
      var o = {};
      (vals || CONV).forEach(function (c) {
        try { var init = {}; init[member] = c[1]; var e = new window[cls]('x', init); var v = e[member]; o[c[0]] = v === undefined ? '#undefined' : typeof v === 'number' && !isFinite(v) ? '#' + v : typeof v === 'object' && v !== null ? 'OBJ' : v; }
        catch (er) { o[c[0]] = 'THROW ' + er.name + ': ' + String(er.message).slice(0, 110); }
      });
      return o;
    });
  }
  ['detail', 'screenX', 'clientY', 'movementX', 'movementY', 'button', 'buttons', 'ctrlKey', 'altKey', 'relatedTarget', 'view'].forEach(function (m) { conv('MouseEvent', m); });
  ['deltaX', 'deltaMode'].forEach(function (m) { conv('WheelEvent', m); });
  ['key', 'code', 'location', 'repeat', 'isComposing', 'charCode', 'keyCode'].forEach(function (m) { conv('KeyboardEvent', m); });
  ['data', 'inputType', 'isComposing'].forEach(function (m) { conv('InputEvent', m); });
  conv('CompositionEvent', 'data');
  ['pointerId', 'width', 'height', 'pressure', 'tiltX', 'tiltY', 'twist', 'tangentialPressure', 'pointerType', 'isPrimary', 'persistentDeviceId'].forEach(function (m) { conv('PointerEvent', m); });
  conv('UIEvent', 'detail'); conv('UIEvent', 'view'); conv('UIEvent', 'sourceCapabilities');

  // ---- derived members
  t('which.mouse', function () { var o = {}; [0, 1, 2, 3, 4].forEach(function (b) { o['b' + b] = new MouseEvent('x', { button: b }).which; }); return o; });
  t('which.key', function () { return [new KeyboardEvent('keydown', { keyCode: 65 }).which, new KeyboardEvent('keydown', { charCode: 66 }).which, new KeyboardEvent('keypress', { charCode: 67, keyCode: 68 }).which, new KeyboardEvent('keydown', { which: 69 }).which, new KeyboardEvent('keydown', { key: 'a' }).keyCode]; });
  t('which.other', function () { return [new UIEvent('x', { which: 9 }).which, new FocusEvent('x').which, new InputEvent('x', { which: 3 }).which, new WheelEvent('wheel', { button: 1 }).which]; });
  t('layer', function () { var e = new MouseEvent('x', { clientX: 10, clientY: 20, screenX: 30 }); return [e.layerX, e.layerY, e.offsetX, e.offsetY, e.pageX, e.pageY, e.x, e.y]; });
  t('movement.pass', function () { var e = new MouseEvent('x', { movementX: 4, movementY: -6 }); return [e.movementX, e.movementY]; });
  t('screen.alias', function () { var e = new PointerEvent('x', { screenX: 30.5, clientX: 7.25 }); return [e.screenX, e.clientX, e.x, e.pageX]; });
  t('ptr.angles', function () { var o = {}; [[0, 0], [30, 0], [0, 45], [30, 45], [90, 0], [-90, 0], [90, 90]].forEach(function (p) { var e = new PointerEvent('x', { tiltX: p[0], tiltY: p[1] }); o[p.join(',')] = [e.altitudeAngle, e.azimuthAngle]; }); o.dflt = (function () { var e = new PointerEvent('x'); return [e.altitudeAngle, e.azimuthAngle]; })(); return o; });
  t('ptr.explicit', function () { var e = new PointerEvent('x', { altitudeAngle: 0.5, azimuthAngle: 1, tiltX: 20 }); return [e.altitudeAngle, e.azimuthAngle, e.tiltX, e.tiltY]; });
  t('ptr.methods', function () { var e = new PointerEvent('x'); var c = e.getCoalescedEvents(), p = e.getPredictedEvents(); return [Array.isArray(c), c.length, Array.isArray(p), p.length, e.getCoalescedEvents.length]; });
  t('ptr.defaults2', function () { var e = new PointerEvent('pointerdown'); return [e.width, e.height, e.pressure, e.pointerType, e.isPrimary, e.pointerId]; });

  // ---- getModifierState
  t('gms.keys', function () { var e = new MouseEvent('x', { ctrlKey: true, shiftKey: true }); var o = {}; ['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock', 'Fn', 'FnLock', 'Hyper', 'NumLock', 'OS', 'ScrollLock', 'Super', 'Symbol', 'SymbolLock', 'Nope', 'control', ''].forEach(function (k) { o[k] = e.getModifierState(k); }); return o; });
  t('gms.dict', function () { var e = new MouseEvent('x', { modifierCapsLock: true, modifierAltGraph: true, modifierNumLock: true, modifierFn: true, modifierSymbol: true, modifierOS: true, modifierScrollLock: true }); var o = {}; ['CapsLock', 'AltGraph', 'NumLock', 'Fn', 'Symbol', 'OS', 'ScrollLock', 'Hyper'].forEach(function (k) { o[k] = e.getModifierState(k); }); return o; });
  t('gms.key', function () { var e = new KeyboardEvent('x', { altKey: true, modifierCapsLock: true }); return [e.getModifierState('Alt'), e.getModifierState('CapsLock'), e.getModifierState('Shift')]; });
  t('gms.args', function () { var e = new MouseEvent('x'); return [e.getModifierState.length, (function () { try { e.getModifierState(); } catch (er) { return er.name + ': ' + er.message; } })()]; });
  t('gms.wheel', function () { var e = new WheelEvent('wheel', { shiftKey: true }); return [e.getModifierState('Shift'), typeof e.getModifierState]; });
  t('gms.input', function () { var e = new InputEvent('x'); return [typeof e.getModifierState, typeof new UIEvent('x').getModifierState, typeof new FocusEvent('x').getModifierState]; });

  // ---- keyboard details
  t('key.init', function () { var e = new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13, repeat: true, location: 1, isComposing: true }); return [e.key, e.code, e.keyCode, e.charCode, e.which, e.repeat, e.location, e.isComposing]; });
  t('key.init.legacy', function () { var e = new KeyboardEvent('x'); var r = []; try { e.initKeyboardEvent('keydown', true, true, window, 'a', 2, true, false, true, false); r.push(e.key, e.location, e.ctrlKey, e.type, e.bubbles); } catch (er) { r.push('THROW ' + er.message); } return r; });
  t('key.init.len', function () { return [KeyboardEvent.prototype.initKeyboardEvent && KeyboardEvent.prototype.initKeyboardEvent.length, MouseEvent.prototype.initMouseEvent.length, UIEvent.prototype.initUIEvent.length]; });
  t('input.methods', function () { var e = new InputEvent('beforeinput', { inputType: 'insertText', data: 'a' }); var r = e.getTargetRanges(); return [Array.isArray(r), r.length, e.getTargetRanges.length, e.dataTransfer]; });
  t('input.dt', function () { var e = new InputEvent('x', { dataTransfer: null }); return [e.dataTransfer]; });
  t('wheel.legacy', function () { var e = new WheelEvent('wheel', { deltaY: 100, deltaX: -50 }); return [e.wheelDelta, e.wheelDeltaX, e.wheelDeltaY, e.deltaMode, WheelEvent.DOM_DELTA_PIXEL, WheelEvent.DOM_DELTA_LINE, WheelEvent.DOM_DELTA_PAGE]; });
  t('mouse.consts', function () { return [typeof MouseEvent.DOM_VK, Object.getOwnPropertyNames(MouseEvent), Object.getOwnPropertyNames(UIEvent)]; });
  t('chain', function () { var e = new PointerEvent('x'); return [e instanceof MouseEvent, e instanceof UIEvent, e instanceof Event, Object.getPrototypeOf(PointerEvent) === MouseEvent, Object.getPrototypeOf(WheelEvent) === MouseEvent, Object.getPrototypeOf(KeyboardEvent) === UIEvent]; });
  t('inherit.event', function () { var e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, composed: true }); return [e.bubbles, e.cancelable, e.composed, e.isTrusted, e.defaultPrevented, e.eventPhase, e.timeStamp > 0, e.srcElement, e.returnValue, e.cancelBubble]; });
  return JSON.stringify(R);
})()
