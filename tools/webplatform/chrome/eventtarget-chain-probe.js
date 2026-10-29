// EventTarget-inheritance chain probe (M4 knife 3). Runs in real Chrome AND
// in the diting engine; diffed. Two faces: (1) the full prototype chain of
// every window interface that reaches EventTarget (minus the Node/Window DOM
// core — a different world), (2) live trio round-trips on constructible
// leaves. Returns a JSON string.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + (e && e.name && e.name !== (e.constructor && e.constructor.name) ? '(' + e.name + ')' : '') + ': ' + (e && e.message); } }
  function levelName(p) {
    var d = Object.getOwnPropertyDescriptor(p, 'constructor');
    if (d && d.value && typeof d.value.name === 'string' && d.value.name) return d.value.name;
    var tag = p[Symbol.toStringTag];
    return typeof tag === 'string' ? tag : '?';
  }
  function chainOf(C) {
    var out = [];
    var p = C.prototype;
    var guard = 0;
    while (p && guard++ < 24) { out.push(levelName(p)); p = Object.getPrototypeOf(p); }
    return out;
  }
  // (1) every window own interface whose chain mentions EventTarget, minus
  // the Node branch (elements/documents) and the window chain itself.
  var chains = {};
  Object.getOwnPropertyNames(globalThis).sort().forEach(function (n) {
    try {
      var C = globalThis[n];
      if (typeof C !== 'function' || !C.prototype) return;
      var ch = chainOf(C);
      if (ch.indexOf('EventTarget') < 0) return;
      if (ch.indexOf('Node') >= 0 || ch.indexOf('Window') >= 0) return;
      chains[n] = ch;
    } catch (e) {}
  });
  R.chains = chains;
  // the trio must be INHERITED, never own, at the leaf level (Chrome shape:
  // addEventListener lives on EventTarget.prototype, not on Animation.prototype)
  var ownTrio = [];
  Object.keys(chains).sort().forEach(function (n) {
    try {
      if (Object.prototype.hasOwnProperty.call(globalThis[n].prototype, 'addEventListener')) ownTrio.push(n);
    } catch (e) {}
  });
  R.ownTrio = ownTrio;
  // EventTarget.prototype own face (M3 baseline sanity — should be settled)
  t('ET.own', function () { return Object.getOwnPropertyNames(EventTarget.prototype); });
  t('ET.ctorName', function () { return EventTarget.prototype.constructor.name; });
  // intermediates that exist on window only as chain links (throw on construct)
  t('chain.Sensor', function () { return chainOf(Sensor); });
  t('chain.OrientationSensor', function () { return chainOf(OrientationSensor); });
  t('chain.AudioNode', function () { return chainOf(AudioNode); });
  t('chain.OscillatorNode', function () { return chainOf(OscillatorNode); });
  // (2) live trio on constructible leaves: listener + dispatch round-trip
  function rt(label, make) {
    t(label, function () {
      var o = make();
      var hit = 0;
      o.addEventListener('x', function () { hit++; });
      var ok = o.dispatchEvent(new Event('x'));
      return [o instanceof EventTarget, hit, ok];
    });
  }
  rt('rt.Animation', function () { return new Animation(); });
  rt('rt.EditContext', function () { return new EditContext(); });
  rt('rt.CloseWatcher', function () { return new CloseWatcher(); });
  rt('rt.Accelerometer', function () { return new Accelerometer(); });
  rt('rt.Gyroscope', function () { return new Gyroscope(); });
  rt('rt.AbsOrient', function () { return new AbsoluteOrientationSensor(); });
  rt('rt.SpeechRecognition', function () { return new SpeechRecognition(); });
  rt('rt.CaptureController', function () { return new CaptureController(); });
  rt('rt.IdleDetector', function () { return new IdleDetector(); });
  rt('rt.MediaStream', function () { return new MediaStream(); });
  rt('rt.SpeechSynthesisUtterance', function () { return new SpeechSynthesisUtterance('x'); });
  rt('rt.Notification', function () { return new Notification('t'); });
  rt('rt.OffscreenCanvas', function () { return new OffscreenCanvas(8, 8); });
  rt('rt.SharedWorker', function () { return new SharedWorker('x.js'); });
  // (3) singleton instance faces: the ET family's live singletons. For each:
  // [proto link to its class prototype, instanceof own class, instanceof
  // EventTarget]; 'ABSENT' when the singleton or its class doesn't exist.
  function single(label, get, clsName) {
    t('sin.' + label, function () {
      var o; try { o = get(); } catch (e) { o = undefined; }
      var C = globalThis[clsName];
      if (o === undefined || o === null || typeof C !== 'function') return 'ABSENT';
      return [Object.getPrototypeOf(o) === C.prototype, o instanceof C, o instanceof EventTarget];
    });
  }
  single('performance', function () { return performance; }, 'Performance');
  single('screen', function () { return screen; }, 'Screen');
  single('screen.orientation', function () { return screen.orientation; }, 'ScreenOrientation');
  single('visualViewport', function () { return visualViewport; }, 'VisualViewport');
  single('speechSynthesis', function () { return speechSynthesis; }, 'SpeechSynthesis');
  single('cookieStore', function () { return cookieStore; }, 'CookieStore');
  single('navigation', function () { return navigation; }, 'Navigation');
  single('documentPictureInPicture', function () { return documentPictureInPicture; }, 'DocumentPictureInPicture');
  single('navigator.mediaDevices', function () { return navigator.mediaDevices; }, 'MediaDevices');
  single('navigator.clipboard', function () { return navigator.clipboard; }, 'Clipboard');
  single('navigator.connection', function () { return navigator.connection; }, 'NetworkInformation');
  single('navigator.serviceWorker', function () { return navigator.serviceWorker; }, 'ServiceWorkerContainer');
  single('navigator.bluetooth', function () { return navigator.bluetooth; }, 'Bluetooth');
  single('navigator.serial', function () { return navigator.serial; }, 'Serial');
  single('navigator.usb', function () { return navigator.usb; }, 'USB');
  single('navigator.hid', function () { return navigator.hid; }, 'HID');
  single('navigator.windowControlsOverlay', function () { return navigator.windowControlsOverlay; }, 'WindowControlsOverlay');
  single('navigator.devicePosture', function () { return navigator.devicePosture; }, 'DevicePosture');
  single('navigator.managed', function () { return navigator.managed; }, 'NavigatorManagedData');
  single('matchMedia', function () { return matchMedia('(prefers-color-scheme)'); }, 'MediaQueryList');
  // class-name map for the generator (truth file is the single source)
  R.sincls = {
    'performance': 'Performance',
    'screen': 'Screen',
    'screen.orientation': 'ScreenOrientation',
    'visualViewport': 'VisualViewport',
    'speechSynthesis': 'SpeechSynthesis',
    'cookieStore': 'CookieStore',
    'navigation': 'Navigation',
    'documentPictureInPicture': 'DocumentPictureInPicture',
    'navigator.mediaDevices': 'MediaDevices',
    'navigator.clipboard': 'Clipboard',
    'navigator.connection': 'NetworkInformation',
    'navigator.serviceWorker': 'ServiceWorkerContainer',
    'navigator.bluetooth': 'Bluetooth',
    'navigator.serial': 'Serial',
    'navigator.usb': 'USB',
    'navigator.hid': 'HID',
    'navigator.windowControlsOverlay': 'WindowControlsOverlay',
    'navigator.devicePosture': 'DevicePosture',
    'navigator.managed': 'NavigatorManagedData',
    'matchMedia': 'MediaQueryList'
  };
  return JSON.stringify(R);
})()
