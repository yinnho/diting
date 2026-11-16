// M4 刀75: the missing-member tail. Chrome 145 members the engine never carried, on
// interfaces whose implementations are otherwise fine: event-handler attributes,
// legacy constants, small read-only reflections, and a few methods with their
// Chrome argument ladders. Runs after the platform bundle and every takeover so the
// prototypes exist; each section is independent (one failing section cannot hide the
// others) and installs only what the prototype does not already own. Receivers are
// brand-checked loosely (objects only): a foreign OBJECT is not rejected here — the
// ledger in SURFACE.md lists that as the known looseness of this tail.
(() => {
  const G = globalThis;
  const mark = G.__diting_markNative || ((f) => f);
  const pin = (f, n, len) => {
    Object.defineProperty(f, 'name', { value: n, configurable: true });
    if (len !== undefined) Object.defineProperty(f, 'length', { value: len, configurable: true });
    return mark(f);
  };
  const proto = (cn) => (typeof G[cn] === 'function' && G[cn].prototype) ? G[cn].prototype : null;
  const obj = (self) => { if (self === null || (typeof self !== 'object' && typeof self !== 'function')) throw new TypeError('Illegal invocation'); return self; };
  const slots = new WeakMap(); // per-instance scratch: { handlers, values }
  const slot = (self) => { let s = slots.get(self); if (!s) { s = { h: Object.create(null), v: Object.create(null) }; slots.set(self, s); } return s; };
  const section = (name, f) => { try { f(); } catch (e) { try { (G.__diting_tailErrors || (G.__diting_tailErrors = [])).push(name + ': ' + e.message); } catch (e2) {} } };
  const def = (P, name, d) => { if (P && !Object.prototype.hasOwnProperty.call(P, name)) Object.defineProperty(P, name, d); };
  const ro = (cn, name, get) => def(proto(cn), name, { get: pin(function () { return get.call(obj(this), slot(this)); }, 'get ' + name), enumerable: true, configurable: true });
  const rw = (cn, name, get, set) => def(proto(cn), name, {
    get: pin(function () { return get.call(obj(this), slot(this)); }, 'get ' + name),
    set: pin(function (v) { set.call(obj(this), v, slot(this)); }, 'set ' + name, 1), enumerable: true, configurable: true });
  const fn = (cn, name, len, f) => def(proto(cn), name, { value: pin(f, name, len), writable: true, enumerable: true, configurable: true });
  const on = (cn, ...types) => { for (const t of types) rw(cn, 'on' + t, (s) => s.h[t] || null, (v, s) => { s.h[t] = typeof v === 'function' ? v : null; }); };
  const cst = (cn, name, v, ctorToo) => {
    // configurable here so the shape pass can order it; the flag pass seals it e-- afterwards
    def(proto(cn), name, { value: v, enumerable: true, configurable: true });
    if (ctorToo && typeof G[cn] === 'function' && !Object.prototype.hasOwnProperty.call(G[cn], name)) Object.defineProperty(G[cn], name, { value: v, enumerable: true });
  };
  const argc = (a, n, m, i) => { if (a.length < n) throw new TypeError("Failed to execute '" + m + "' on '" + i + "': " + n + ' argument' + (n > 1 ? 's' : '') + ' required, but only ' + a.length + ' present.'); };

  // ---- event-handler attributes -------------------------------------------------
  section('handlers', () => {
    on('BatteryManager', 'chargingchange', 'chargingtimechange', 'dischargingtimechange', 'levelchange');
    on('MediaStream', 'addtrack', 'removetrack', 'active', 'inactive');
    on('OffscreenCanvas', 'contextlost', 'contextrestored');
    on('Performance', 'resourcetimingbufferfull');
    on('RTCDtlsTransport', 'statechange', 'error');
    on('RTCSctpTransport', 'statechange');
    on('ServiceWorkerContainer', 'controllerchange', 'message', 'messageerror');
    on('SharedWorker', 'error');
    on('TextTrackCue', 'enter', 'exit');
    on('VisualViewport', 'resize', 'scroll', 'scrollend');
    on('AudioContext', 'sinkchange');
    on('MediaSource', 'sourceopen', 'sourceended', 'sourceclose');
  });

  // ---- legacy constants ---------------------------------------------------------
  section('constants', () => {
    for (const [n, v] of [['CONNECTING', 0], ['OPEN', 1], ['CLOSING', 2], ['CLOSED', 3]]) cst('WebSocket', n, v, true);
    for (const [n, v] of [['CONNECTING', 0], ['OPEN', 1], ['CLOSED', 2]]) cst('EventSource', n, v, true);
    for (const [n, v] of [['TEMPORARY', 0], ['PERSISTENT', 1]]) cst('Window', n, v, true);
    // Chrome 157 side-of-path constants. Configurable here so shape_fix can
    // seat them after the spacing constants; the flag pass then seals e--.
    // Both the prototype and the constructor get a copy, same as the older
    // TEXTPATH_* constants from the stub table.
    const TP = proto('SVGTextPathElement');
    const TC = G.SVGTextPathElement;
    if (TP && typeof TC === 'function') {
      for (const [n, v] of [['TEXTPATH_SIDETYPE_UNKNOWN', 0], ['TEXTPATH_SIDETYPE_LEFT', 1], ['TEXTPATH_SIDETYPE_RIGHT', 2]]) {
        for (const obj of [TP, TC]) {
          if (!Object.prototype.hasOwnProperty.call(obj, n)) Object.defineProperty(obj, n, { value: v, enumerable: true, configurable: true });
        }
      }
    }
  });

  // ---- audio --------------------------------------------------------------------
  section('audio', () => {
    const an = G.__diting_anState;
    // connect/disconnect: the minted prototype carries no-op stubs; replace them with the graph-aware connect
    const ANP = proto('AudioNode');
    if (ANP && typeof G.__diting_anConnect === 'function') {
      Object.defineProperty(ANP, 'connect', { value: pin(function connect(dest) { return G.__diting_anConnect.call(this, dest); }, 'connect', 1), writable: true, enumerable: true, configurable: true });
      Object.defineProperty(ANP, 'disconnect', { value: pin(function disconnect() {}, 'disconnect', 0), writable: true, enumerable: true, configurable: true });
    }
    const need = (self) => { const s = an && an.get(self); if (!s) throw new TypeError('Illegal invocation'); return s; };
    for (const k of ['context', 'numberOfInputs', 'numberOfOutputs']) ro('AudioNode', k, function () { return need(this)[k]; });
    rw('AudioNode', 'channelCount', function () { return need(this).channelCount; }, function (v) {
      const s = need(this); const n = Number(v) >>> 0;
      if (n < 1 || n > 32) throw new DOMException("Failed to set the 'channelCount' property on 'AudioNode': The channel count provided (" + n + ') is outside the range [1, 32].', 'NotSupportedError');
      s.channelCount = n;
    });
    rw('AudioNode', 'channelCountMode', function () { return need(this).channelCountMode; }, function (v) { const x = String(v); if (x === 'max' || x === 'clamped-max' || x === 'explicit') need(this).channelCountMode = x; });
    rw('AudioNode', 'channelInterpretation', function () { return need(this).channelInterpretation; }, function (v) { const x = String(v); if (x === 'speakers' || x === 'discrete') need(this).channelInterpretation = x; });
    ro('AudioContext', 'sinkId', (s) => (s.v.sinkId === undefined ? '' : s.v.sinkId));
    fn('AudioContext', 'setSinkId', 1, function (id) {
      try { obj(this); argc(arguments, 1, 'setSinkId', 'AudioContext'); } catch (e) { return Promise.reject(e); }
      const s = slot(this); const next = (id !== null && typeof id === 'object') ? { type: String(id.type) } : String(id);
      const changed = JSON.stringify(next) !== JSON.stringify(s.v.sinkId === undefined ? '' : s.v.sinkId);
      s.v.sinkId = next;
      if (changed) Promise.resolve().then(() => { try { this.dispatchEvent(new Event('sinkchange')); } catch (e) {} });
      return Promise.resolve();
    });
    // Chrome 157 put this back (Chrome 145 did not have it). The render
    // quantum is the spec default, 128 frames.
    ro('BaseAudioContext', 'renderQuantumSize', () => 128);
    ro('BaseAudioContext', 'audioWorklet', (s) => {
      if (!s.v.worklet) { const C = G.AudioWorklet; s.v.worklet = (C && C.prototype) ? Object.create(C.prototype) : {}; }
      return s.v.worklet;
    });
  });

  // ---- DOM ----------------------------------------------------------------------
  section('childnode', () => {
    const E = G.Element && G.Element.prototype;
    for (const n of ['after', 'before', 'remove', 'replaceWith']) {
      const d = E && Object.getOwnPropertyDescriptor(E, n);
      if (d && typeof d.value === 'function') fn('DocumentType', n, 0, function (...a) { return d.value.apply(obj(this), a); });
    }
    fn('Range', 'expand', 0, function expand(unit) { obj(this); if (unit !== undefined) String(unit); });
    rw('HTMLHtmlElement', 'version', function () { const v = this.getAttribute('version'); return v === null ? '' : v; }, function (v) { this.setAttribute('version', String(v)); });
    for (const [n, a] of [['privateToken', 'privatetoken']]) rw('HTMLIFrameElement', n, function () { const v = this.getAttribute(a); return v === null ? '' : v; }, function (v) { this.setAttribute(a, String(v)); });
    for (const [n, a] of [['browsingTopics', 'browsingtopics'], ['adAuctionHeaders', 'adauctionheaders'], ['sharedStorageWritable', 'sharedstoragewritable']]) {
      rw('HTMLIFrameElement', n, function () { return this.hasAttribute(a); }, function (v) { if (v) this.setAttribute(a, ''); else this.removeAttribute(a); });
    }
    ro('HTMLIFrameElement', 'featurePolicy', (s) => {
      if (!s.v.fp) { const C = G.FeaturePolicy; s.v.fp = (C && C.prototype) ? Object.create(C.prototype) : (G.document && G.document.featurePolicy) || {}; }
      return s.v.fp;
    });
    for (const [n, a] of [['media', 'media'], ['title', 'title']]) rw('SVGStyleElement', n, function () { const v = this.getAttribute(a); return v === null ? '' : v; }, function (v) { this.setAttribute(a, String(v)); });
    // Chrome 157: ShadowRoot is not an Element, so this is a nullable DOMString
    // slot, not a content attribute. Missing reads null; null and undefined
    // clear it; every other value is String(v) (knife66 rt*).
    rw('ShadowRoot', 'referenceTarget', function (s) {
      return Object.prototype.hasOwnProperty.call(s.v, 'rt') ? s.v.rt : null;
    }, function (v, s) {
      if (typeof v === 'symbol') throw new TypeError("Failed to set the 'referenceTarget' property on 'ShadowRoot': Cannot convert a Symbol value to a string");
      s.v.rt = v === null || v === undefined ? null : String(v);
    });
    fn('HTMLCanvasElement', 'captureStream', 0, function captureStream() { obj(this); return new G.MediaStream(); });
    fn('HTMLCanvasElement', 'transferControlToOffscreen', 0, function transferControlToOffscreen() {
      obj(this);
      if (this._ctxType) throw new DOMException("Failed to execute 'transferControlToOffscreen' on 'HTMLCanvasElement': Cannot transfer control from a canvas that has a rendering context.", 'InvalidStateError');
      return new G.OffscreenCanvas(this.width, this.height);
    });
    fn('Path2D', 'roundRect', 4, function roundRect(x, y, w, h, radii) {
      obj(this); argc(arguments, 4, 'roundRect', 'Path2D');
      x = +x; y = +y; w = +w; h = +h;
      const r = Math.max(0, Math.min(Math.abs(w) / 2, Math.abs(h) / 2, Array.isArray(radii) ? +radii[0] || 0 : (typeof radii === 'number' ? radii : (radii && +radii.x) || 0)));
      if (typeof this.moveTo !== 'function') return;
      this.moveTo(x + r, y); this.lineTo(x + w - r, y); if (this.arcTo) this.arcTo(x + w, y, x + w, y + r, r);
      this.lineTo(x + w, y + h - r); if (this.arcTo) this.arcTo(x + w, y + h, x + w - r, y + h, r);
      this.lineTo(x + r, y + h); if (this.arcTo) this.arcTo(x, y + h, x, y + h - r, r);
      this.lineTo(x, y + r); if (this.arcTo) this.arcTo(x, y, x + r, y, r); if (this.closePath) this.closePath();
    });
  });

  // ---- Chrome 157 element-image canvas face --------------------------------------
  // Validation only. Nothing here records a paint, so the methods that need a
  // cached paint record throw Chrome's message. clearElementGeometry accepts
  // any Element and returns. getElementTransform returns an identity matrix.
  section('element-image', () => {
    const isEl = (v) => v !== null && typeof v === 'object' && typeof G.Element === 'function' && v instanceof G.Element;
    const failType = (iface, method, union) => {
      throw new TypeError("Failed to execute '" + method + "' on '" + iface + "': " + (union
        ? "The provided value is not of type '(Element or ElementImage)'."
        : "parameter 1 is not of type 'Element'."));
    };
    const failState = (iface, method, msg) => {
      throw new DOMException("Failed to execute '" + method + "' on '" + iface + "': " + msg, 'InvalidStateError');
    };
    const toNum = (v, iface, method) => {
      if (typeof v === 'symbol') throw new TypeError("Failed to execute '" + method + "' on '" + iface + "': Cannot convert a Symbol value to a number");
      try { return +v; } catch (e) { throw new TypeError("Failed to execute '" + method + "' on '" + iface + "': " + e.message); }
    };
    const canvasDrawable = (canvas) => {
      const v = canvas.getAttribute && canvas.getAttribute('content');
      return v != null && String(v).toLowerCase() === 'drawable';
    };
    const nearestCanvas = (el) => {
      let n = el.parentNode;
      while (n) {
        if (n.localName === 'canvas' || (typeof n.tagName === 'string' && n.tagName.toUpperCase() === 'CANVAS')) return n;
        n = n.parentNode;
      }
      return null;
    };
    // Type is checked before the canvas content attribute. Then the element
    // needs drawable, then it has to belong to this canvas.
    const prepare = (canvas, el, method, union) => {
      if (!isEl(el)) failType('HTMLCanvasElement', method, union);
      if (!canvasDrawable(canvas)) failState('HTMLCanvasElement', method, method + " requires the canvas to have the content=drawable attribute.");
      if (!el.hasAttribute('drawable')) failState('HTMLCanvasElement', method, "Descendants passed to " + method + " must have the 'drawable' attribute.");
      if (nearestCanvas(el) !== canvas) failState('HTMLCanvasElement', method, 'The element can only be drawn into its nearest ancestor <canvas>.');
    };
    const noPaint = (method) => failState('HTMLCanvasElement', method, 'No cached paint record for element.');
    rw('HTMLCanvasElement', 'content', function () {
      const v = this.getAttribute('content');
      return v != null && String(v).toLowerCase() === 'drawable' ? 'drawable' : 'fallback';
    }, function (v) { this.setAttribute('content', String(v)); });
    on('HTMLCanvasElement', 'paint', 'elementgeometryupdate');
    fn('HTMLCanvasElement', 'captureElementImage', 1, function captureElementImage(el) {
      obj(this); argc(arguments, 1, 'captureElementImage', 'HTMLCanvasElement');
      prepare(this, el, 'captureElementImage', false);
      noPaint('captureElementImage');
    });
    fn('HTMLCanvasElement', 'clearElementGeometry', 1, function clearElementGeometry(el) {
      obj(this); argc(arguments, 1, 'clearElementGeometry', 'HTMLCanvasElement');
      if (!isEl(el)) failType('HTMLCanvasElement', 'clearElementGeometry', true);
    });
    fn('HTMLCanvasElement', 'getElementImageDefaultSize', 1, function getElementImageDefaultSize(el) {
      obj(this); argc(arguments, 1, 'getElementImageDefaultSize', 'HTMLCanvasElement');
      prepare(this, el, 'getElementImageDefaultSize', true);
      noPaint('getElementImageDefaultSize');
    });
    fn('HTMLCanvasElement', 'getElementTransform', 1, function getElementTransform(el) {
      obj(this); argc(arguments, 1, 'getElementTransform', 'HTMLCanvasElement');
      prepare(this, el, 'getElementTransform', false);
      return new G.DOMMatrix();
    });
    fn('HTMLCanvasElement', 'requestPaint', 0, function requestPaint() { obj(this); });
    fn('HTMLCanvasElement', 'updateElementGeometry', 1, function updateElementGeometry(el) {
      obj(this); argc(arguments, 1, 'updateElementGeometry', 'HTMLCanvasElement');
      prepare(this, el, 'updateElementGeometry', true);
    });
    // OffscreenCanvas has no content attribute. clear and update accept any
    // Element. The size query rejects every element.
    fn('OffscreenCanvas', 'clearElementGeometry', 1, function clearElementGeometry(el) {
      obj(this); argc(arguments, 1, 'clearElementGeometry', 'OffscreenCanvas');
      if (!isEl(el)) failType('OffscreenCanvas', 'clearElementGeometry', true);
    });
    fn('OffscreenCanvas', 'getElementImageDefaultSize', 1, function getElementImageDefaultSize(el) {
      obj(this); argc(arguments, 1, 'getElementImageDefaultSize', 'OffscreenCanvas');
      if (!isEl(el)) failType('OffscreenCanvas', 'getElementImageDefaultSize', true);
      failState('OffscreenCanvas', 'getElementImageDefaultSize', 'Elements cannot be drawn into an OffscreenCanvas.');
    });
    fn('OffscreenCanvas', 'updateElementGeometry', 1, function updateElementGeometry(el) {
      obj(this); argc(arguments, 1, 'updateElementGeometry', 'OffscreenCanvas');
      if (!isEl(el)) failType('OffscreenCanvas', 'updateElementGeometry', true);
    });
    const draw = (iface, offscreen) => function drawElementImage(el, x, y) {
      obj(this);
      argc(arguments, 3, 'drawElementImage', iface);
      if (!isEl(el)) failType(iface, 'drawElementImage', true);
      toNum(x, iface, 'drawElementImage');
      toNum(y, iface, 'drawElementImage');
      if (offscreen) failState(iface, 'drawElementImage', 'Elements cannot be drawn into an OffscreenCanvas.');
      const canvas = this.canvas;
      if (!canvas || typeof canvas.getAttribute !== 'function') throw new TypeError('Illegal invocation');
      if (!canvasDrawable(canvas)) failState(iface, 'drawElementImage', 'DrawElementImage requires the canvas to have the content=drawable attribute.');
      if (!el.hasAttribute('drawable')) failState(iface, 'drawElementImage', "Descendants passed to DrawElementImage must have the 'drawable' attribute.");
      if (nearestCanvas(el) !== canvas) failState(iface, 'drawElementImage', 'The element can only be drawn into its nearest ancestor <canvas>.');
      failState(iface, 'drawElementImage', 'No cached paint record for element.');
    };
    fn('CanvasRenderingContext2D', 'drawElementImage', 3, draw('CanvasRenderingContext2D', false));
    fn('OffscreenCanvasRenderingContext2D', 'drawElementImage', 3, draw('OffscreenCanvasRenderingContext2D', true));
  });

  // ---- File / FileList-adjacent / media / geometry -------------------------------
  section('file', () => {
    ro('File', 'lastModifiedDate', function () { return new Date(this.lastModified); });
    ro('File', 'webkitRelativePath', () => '');
    ro('ImageData', 'colorSpace', () => 'srgb');
    ro('ImageData', 'pixelFormat', () => 'rgba-unorm8');
    // Chrome's ImageData instance owns `data` while the prototype also answers it
    // (getter-only, 刀84); the constructor creates the own property directly.
    def(proto('ImageData'), 'data', {
      get: pin(function () { const d = Object.getOwnPropertyDescriptor(obj(this), 'data'); return d ? d.value : undefined; }, 'get data'),
      enumerable: true, configurable: true });
    ro('MediaMetadata', 'chapterInfo', (s) => s.v.ch || (s.v.ch = []));
    ro('VideoPlaybackQuality', 'creationTime', (s) => (s.v.t === undefined ? (s.v.t = (G.performance ? G.performance.now() : 0)) : s.v.t));
    for (const k of ['totalVideoFrames', 'droppedVideoFrames', 'corruptedVideoFrames']) ro('VideoPlaybackQuality', k, () => 0);
    fn('MediaStream', 'getTrackById', 1, function getTrackById(id) {
      obj(this); argc(arguments, 1, 'getTrackById', 'MediaStream');
      const all = typeof this.getTracks === 'function' ? this.getTracks() : [];
      const want = String(id);
      for (const t of all) if (t && t.id === want) return t;
      return null;
    });
    ro('PerformancePaintTiming', 'paintTime', function () { return this.startTime; });
    ro('PerformancePaintTiming', 'presentationTime', () => null);
    fn('PerformancePaintTiming', 'toJSON', 0, function toJSON() { obj(this); return { name: this.name, entryType: this.entryType, startTime: this.startTime, duration: this.duration, paintTime: this.paintTime, presentationTime: this.presentationTime }; });
    // Chrome 157 interaction paint and soft navigation. No interaction is
    // recorded, so the counters stay at rest: paint time follows startTime,
    // presentation time is null, and the largest paint is null.
    const roBrand = (cn, name, get) => {
      const C = G[cn];
      if (typeof C !== 'function') return;
      def(C.prototype, name, { get: pin(function () {
        if (!(this instanceof C)) throw new TypeError('Illegal invocation');
        return get.call(this);
      }, 'get ' + name), enumerable: true, configurable: true });
    };
    const paintNum = function () { return typeof this.startTime === 'number' && isFinite(this.startTime) ? this.startTime : 0; };
    for (const cn of ['InteractionContentfulPaint', 'PerformanceSoftNavigation']) {
      roBrand(cn, 'interactionId', () => 0);
      roBrand(cn, 'paintTime', paintNum);
      roBrand(cn, 'presentationTime', () => null);
    }
    roBrand('InteractionContentfulPaint', 'largestContentfulPaint', () => 0);
    roBrand('PerformanceSoftNavigation', 'navigationType', () => '');
    fn('InteractionContentfulPaint', 'toJSON', 0, function toJSON() {
      if (!(this instanceof G.InteractionContentfulPaint)) throw new TypeError('Illegal invocation');
      return Object.assign(G.PerformanceEntry.prototype.toJSON.call(this), {
        largestContentfulPaint: this.largestContentfulPaint,
        interactionId: this.interactionId,
        paintTime: this.paintTime,
        presentationTime: this.presentationTime,
      });
    });
    fn('PerformanceSoftNavigation', 'getLargestInteractionContentfulPaint', 0, function getLargestInteractionContentfulPaint() {
      if (!(this instanceof G.PerformanceSoftNavigation)) throw new TypeError('Illegal invocation');
      return null;
    });
    const seat = (P, want) => {
      if (!P) return;
      const names = Object.getOwnPropertyNames(P);
      const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
      if (seq.every((k, i) => k === names[i])) return;
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    };
    seat(G.InteractionContentfulPaint && G.InteractionContentfulPaint.prototype, ['largestContentfulPaint', 'interactionId', 'paintTime', 'presentationTime', 'toJSON', 'constructor']);
    seat(G.PerformanceSoftNavigation && G.PerformanceSoftNavigation.prototype, ['navigationType', 'interactionId', 'paintTime', 'presentationTime', 'getLargestInteractionContentfulPaint', 'constructor']);
    ro('Performance', 'interactionCount', () => 0);
    fn('Performance', 'toJSON', 0, function toJSON() {
      obj(this);
      const t = this.timing, n = this.navigation, tj = (o) => { if (!o) return {}; if (typeof o.toJSON === 'function') { const j = o.toJSON(); if (j && typeof j === 'object') return j; } const r = {}; for (const k in o) { let v; try { v = o[k]; } catch (e) { continue; } if (typeof v !== 'function') r[k] = v; } return r; };
      return { timeOrigin: this.timeOrigin, timing: tj(t), navigation: tj(n), eventCounts: {} };
    });
    for (const cn of ['GeolocationCoordinates', 'GeolocationPosition']) {
      fn(cn, 'toJSON', 0, function toJSON() {
        obj(this);
        if (cn === 'GeolocationPosition') return { coords: this.coords && typeof this.coords.toJSON === 'function' ? this.coords.toJSON() : this.coords, timestamp: this.timestamp };
        const r = {}; for (const k of ['accuracy', 'latitude', 'longitude', 'altitude', 'altitudeAccuracy', 'heading', 'speed']) r[k] = this[k];
        return r;
      });
    }
  });

  // ---- CSS ----------------------------------------------------------------------
  section('css', () => {
    const txt = function () { try { return String(this.cssText || ''); } catch (e) { return ''; } };
    const decl = (re) => function () { const m = txt.call(this).match(re); return m ? m[1].trim().replace(/^["']|["']$/g, '') : ''; };
    ro('CSSFontPaletteValuesRule', 'fontFamily', function () { return decl(/font-family\s*:\s*([^;}]+)/i).call(this); });
    ro('CSSFontPaletteValuesRule', 'basePalette', function () { return decl(/base-palette\s*:\s*([^;}]+)/i).call(this); });
    ro('CSSFontPaletteValuesRule', 'overrideColors', function () { return decl(/override-colors\s*:\s*([^;}]+)/i).call(this); });
    ro('CSSImportRule', 'supportsText', () => null);
    ro('CSSKeyframesRule', 'length', function () { return this.cssRules ? this.cssRules.length : 0; });
    ro('CSSScopeRule', 'start', function () { const m = txt.call(this).match(/@scope\s*\(([^)]*)\)/i); return m ? m[1].trim() : null; });
    ro('CSSScopeRule', 'end', function () { const m = txt.call(this).match(/@scope\s*(?:\([^)]*\))?\s*to\s*\(([^)]*)\)/i); return m ? m[1].trim() : null; });
    ro('CSSStyleRule', 'styleMap', function (s) {
      if (!s.v.sm) { const e = G.document.createElement('div'); try { e.style.cssText = this.style.cssText; } catch (x) {} s.v.sm = e.attributeStyleMap; }
      return s.v.sm;
    });
    ro('CSSStyleDeclaration', 'parentRule', function (s) { return this._parentRule || null; });
    rw('CSSStyleDeclaration', 'cssFloat', function () { return this.getPropertyValue('float'); }, function (v) { this.setProperty('float', String(v)); });
  });

  // ---- SVG ----------------------------------------------------------------------
  section('svg', () => {
    const G2 = 'SVGGeometryElement';
    ro(G2, 'pathLength', function (s) { return s.v.pl || (s.v.pl = G.__diting_svgAnimNumber(this, 'pathLength', 0)); });
    fn(G2, 'isPointInFill', 0, function isPointInFill(pt) { obj(this); return false; });
    fn(G2, 'isPointInStroke', 0, function isPointInStroke(pt) { obj(this); return false; });
    // (points/animatedPoints moved to svg_root.js at 刀91 — real SVGPointList
    // views over the attribute; this stub returned an inert object.)
  });

  // ---- PutForwards / replaceable setters (Chrome rw where the engine kept a getter) ----
  section('putforwards', () => {
    const addSetter = (cn, name, set) => {
      const P = proto(cn); const d = P && Object.getOwnPropertyDescriptor(P, name);
      if (d && d.get && !d.set) Object.defineProperty(P, name, { get: d.get, set: pin(function (v) { set.call(obj(this), v); }, 'set ' + name, 1), enumerable: true, configurable: true });
    };
    addSetter('CSSImportRule', 'media', function (v) { this.media.mediaText = String(v); });
    addSetter('CSSMediaRule', 'media', function (v) { this.media.mediaText = String(v); });
    for (const cn of ['CSSKeyframeRule', 'CSSPageRule', 'CSSStyleRule']) addSetter(cn, 'style', function (v) { this.style.cssText = String(v); });
    addSetter('CSSPageRule', 'selectorText', function (v) { String(v); });
    addSetter('SVGAElement', 'relList', function (v) { this.relList.value = String(v); });
  });

  // ---- statics --------------------------------------------------------------------
  section('statics', () => {
    const sdef = (cn, name, d) => { const C = G[cn]; if (typeof C === 'function' && !Object.prototype.hasOwnProperty.call(C, name)) Object.defineProperty(C, name, d); };
    const sfn = (cn, name, len, f) => sdef(cn, name, { value: pin(f, name, len), writable: true, enumerable: true, configurable: true });
    sfn('HTMLScriptElement', 'supports', 1, function supports(type) {
      if (arguments.length < 1) throw new TypeError("Failed to execute 'supports' on 'HTMLScriptElement': 1 argument required, but only 0 present.");
      const t = String(type); return t === 'classic' || t === 'module' || t === 'importmap' || t === 'speculationrules';
    });
    // BluetoothUUID: 16/32-bit aliases canonicalize onto the Bluetooth base UUID; full UUIDs lowercase through
    const UU = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
    const canon = (alias, m) => {
      if (typeof alias === 'number' || (typeof alias === 'string' && /^\d+$/.test(alias) === false && false)) { /* numeric path below */ }
      if (typeof alias === 'number') { const n = alias >>> 0; return n.toString(16).padStart(8, '0') + '-0000-1000-8000-00805f9b34fb'; }
      const str = String(alias);
      if (UU.test(str)) return str;
      throw new TypeError("Failed to execute '" + m + "' on 'BluetoothUUID': Invalid UUID: '" + str + "'.");
    };
    sfn('BluetoothUUID', 'canonicalUUID', 1, function canonicalUUID(alias) { argc(arguments, 1, 'canonicalUUID', 'BluetoothUUID'); return canon(Number(alias) >>> 0 === Number(alias) ? Number(alias) : alias, 'canonicalUUID'); });
    for (const m of ['getService', 'getCharacteristic', 'getDescriptor']) sfn('BluetoothUUID', m, 1, function (name) { argc(arguments, 1, m, 'BluetoothUUID'); return canon(typeof name === 'number' ? name : name, m); });
    sfn('IdentityProvider', 'close', 0, function close() {});
    sfn('IdentityProvider', 'getUserInfo', 1, function getUserInfo(config) {
      if (arguments.length < 1) return Promise.reject(new TypeError("Failed to execute 'getUserInfo' on 'IdentityProvider': 1 argument required, but only 0 present."));
      return Promise.reject(new DOMException('Failed to execute getUserInfo: not signed in', 'NetworkError'));
    });
    sfn('IdentityProvider', 'resolve', 1, function resolve(token) { argc(arguments, 1, 'resolve', 'IdentityProvider'); });
    sdef('MediaSource', 'canConstructInDedicatedWorker', { get: pin(function () { return true; }, 'get canConstructInDedicatedWorker'), enumerable: true, configurable: true });
    sfn('RestrictionTarget', 'fromElement', 1, function fromElement(el) {
      if (arguments.length < 1) return Promise.reject(new TypeError("Failed to execute 'fromElement' on 'RestrictionTarget': 1 argument required, but only 0 present."));
      const C = G.RestrictionTarget;
      return (el instanceof G.Element) ? Promise.resolve(Object.create(C.prototype)) : Promise.reject(new TypeError("Failed to execute 'fromElement' on 'RestrictionTarget': parameter 1 is not of type 'Element'."));
    });
    // Chrome 145 has no Iterator.concat (it shipped later)
    if (G.Iterator && Object.prototype.hasOwnProperty.call(G.Iterator, 'concat')) { try { delete G.Iterator.concat; } catch (e) {} }
  });

  // ---- workers / transports / streams --------------------------------------------
  section('transports', () => {
    ro('RTCDtlsTransport', 'iceTransport', () => null);
    fn('RTCDtlsTransport', 'getRemoteCertificates', 0, function getRemoteCertificates() { obj(this); return []; });
    ro('RTCSctpTransport', 'transport', () => null);
    ro('RTCSctpTransport', 'maxChannels', () => null);
    fn('RTCEncodedAudioFrame', 'getMetadata', 0, function getMetadata() { obj(this); return {}; });
    fn('RTCEncodedAudioFrame', 'toString', 0, function toString() { return Object.prototype.toString.call(obj(this)); });
    for (const n of ['entries', 'keys', 'values']) {
      fn('RTCStatsReport', n, 0, function () {
        obj(this);
        const M = this instanceof Map ? Map.prototype : null;
        return M ? M[n].call(this) : [][n]();
      });
    }
    ro('RTCPeerConnectionIceEvent', 'candidate', function () { return this._candidate === undefined ? null : this._candidate; });
    fn('ServiceWorkerContainer', 'startMessages', 0, function startMessages() { obj(this); });
    // SharedWorker.port: the class assigns it in its constructor — lifted by lift_own (gen_lift_own.py)
    ro('WebTransport', 'protocol', () => '');
    ro('WebTransport', 'incomingUnidirectionalStreams', (s) => s.v.iu || (s.v.iu = new G.ReadableStream()));
    ro('WebTransport', 'incomingBidirectionalStreams', (s) => s.v.ib || (s.v.ib = new G.ReadableStream()));
    ro('GPUAdapter', 'features', (s) => s.v.f || (s.v.f = new Set()));
    ro('GPUAdapter', 'limits', (s) => s.v.l || (s.v.l = {}));
    ro('GPUAdapter', 'info', (s) => s.v.i || (s.v.i = { vendor: '', architecture: '', device: '', description: '' }));
    ro('WritableStreamDefaultController', 'signal', (s) => s.v.sig || (s.v.sig = new G.AbortController().signal));
    ro('ViewTransition', 'types', (s) => s.v.ty || (s.v.ty = new Set()));
    // Chrome 157. The live object from startViewTransition carries its own
    // transitionRoot; this getter is the prototype member.
    ro('ViewTransition', 'transitionRoot', () => null);
    // Chrome 157 AudioContext.playbackStats. Idle context: every counter is 0.
    // One object per context. resetLatency clears only the latency trio.
    section('playback-stats', () => {
      const C = G.AudioPlaybackStats;
      const AC = G.AudioContext;
      if (typeof C !== 'function' || typeof AC !== 'function') return;
      const P = C.prototype;
      const KEYS = ['underrunDuration', 'underrunEvents', 'totalDuration', 'averageLatency', 'minimumLatency', 'maximumLatency'];
      const slots = new WeakMap();
      const byCtx = new WeakMap();
      const brand = (self) => {
        const s = slots.get(self);
        if (!s) throw new TypeError('Illegal invocation');
        return s;
      };
      for (const k of KEYS) {
        const get = pin(function () { return brand(this)[k]; }, 'get ' + k);
        def(P, k, { get, enumerable: true, configurable: true });
      }
      const resetLatency = pin(function resetLatency() {
        const s = brand(this);
        s.averageLatency = 0;
        s.minimumLatency = 0;
        s.maximumLatency = 0;
      }, 'resetLatency', 0);
      def(P, 'resetLatency', { value: resetLatency, writable: true, enumerable: true, configurable: true });
      const toJSON = pin(function toJSON() {
        const s = brand(this);
        const o = {};
        for (const k of KEYS) o[k] = s[k];
        return o;
      }, 'toJSON', 0);
      def(P, 'toJSON', { value: toJSON, writable: true, enumerable: true, configurable: true });
      const ctor = Object.getOwnPropertyDescriptor(P, 'constructor');
      if (ctor && ctor.configurable) { delete P.constructor; Object.defineProperty(P, 'constructor', ctor); }
      if (!Object.prototype.hasOwnProperty.call(P, Symbol.toStringTag)) {
        Object.defineProperty(P, Symbol.toStringTag, { value: 'AudioPlaybackStats', configurable: true });
      }
      const getPs = pin(function () {
        if (!(this instanceof AC)) throw new TypeError('Illegal invocation');
        let obj = byCtx.get(this);
        if (!obj) {
          obj = Object.create(P);
          slots.set(obj, { underrunDuration: 0, underrunEvents: 0, totalDuration: 0, averageLatency: 0, minimumLatency: 0, maximumLatency: 0 });
          byCtx.set(this, obj);
        }
        return obj;
      }, 'get playbackStats');
      def(AC.prototype, 'playbackStats', { get: getPs, enumerable: true, configurable: true });
    });
    fn('ViewTransition', 'waitUntil', 1, function waitUntil(p) { obj(this); argc(arguments, 1, 'waitUntil', 'ViewTransition'); });
    ro('ToggleEvent', 'source', () => null);
    for (const n of ['setAttributionReporting', 'setPrivateToken']) {
      fn('XMLHttpRequest', n, 1, function () {
        obj(this); argc(arguments, 1, n, 'XMLHttpRequest');
        if (this.readyState !== 1) throw new DOMException("Failed to execute '" + n + "' on 'XMLHttpRequest': The object's state must be OPENED.", 'InvalidStateError');
      });
    }
    // SourceBuffer mode/updating/timestampOffset/appendWindow*: the class assigns them in its constructor — lifted by lift_own
  });

  // Chrome 157 createValueRange on input and textarea. Offsets are WebIDL
  // longs (NaN and infinities become 0). end < start collapses at start.
  // A negative or past-the-end offset is IndexSizeError. The result is an
  // OpaqueRange and does not move the control's selection.
  section('input-value-range', () => {
    const OR = G.OpaqueRange;
    if (typeof OR !== 'function') return;
    const toLong = (v, iface) => {
      let n;
      try { n = Number(v); }
      catch (e) { throw new TypeError("Failed to execute 'createValueRange' on '" + iface + "': " + e.message); }
      if (!Number.isFinite(n) || n === 0) return 0;
      n = Math.trunc(n);
      if (n > 0x7fffffff) return 0x7fffffff;
      if (n < -0x80000000) return -0x80000000;
      return n;
    };
    const seatAfter = (P, anchor) => {
      const names = Object.getOwnPropertyNames(P);
      const at = names.indexOf(anchor);
      if (at < 0 || names[at + 1] === 'createValueRange') return;
      const tail = [];
      for (let i = at + 1; i < names.length; i++) {
        if (names[i] === 'createValueRange') continue;
        const d = Object.getOwnPropertyDescriptor(P, names[i]);
        if (d && d.configurable) { tail.push([names[i], d]); delete P[names[i]]; }
      }
      const mine = Object.getOwnPropertyDescriptor(P, 'createValueRange');
      if (mine && mine.configurable) { delete P['createValueRange']; Object.defineProperty(P, 'createValueRange', mine); }
      for (const [n, d] of tail) Object.defineProperty(P, n, d);
    };
    const install = (cn, anchor) => {
      const C = G[cn];
      if (typeof C !== 'function') return;
      const P = C.prototype;
      if (!Object.prototype.hasOwnProperty.call(P, 'createValueRange')) {
        const fn = function createValueRange(start, end) {
          if (!(this instanceof C)) throw new TypeError('Illegal invocation');
          if (arguments.length < 2) {
            throw new TypeError("Failed to execute 'createValueRange' on '" + cn + "': 2 arguments required, but only " + arguments.length + " present.");
          }
          const len = String(this.value == null ? '' : this.value).length;
          let s = toLong(start, cn), e = toLong(end, cn);
          if (!(s >= 0 && e >= 0 && s <= len && e <= len)) {
            throw new DOMException("Failed to execute 'createValueRange' on '" + cn + "': Start or end offset exceeds value length.", 'IndexSizeError');
          }
          if (e < s) e = s;
          const r = Object.create(OR.prototype);
          r._so = s;
          r._eo = e;
          return r;
        };
        pin(fn, 'createValueRange', 2);
        def(P, 'createValueRange', { value: fn, writable: true, enumerable: true, configurable: true });
      }
      seatAfter(P, anchor);
    };
    install('HTMLInputElement', 'webkitEntries');
    install('HTMLTextAreaElement', 'setSelectionRange');
  });

  // Chrome 157 OpaqueRange. disconnect collapses the offsets to 0.
  // Glyph boxes inside a control are not measured here, so a live range
  // reports one empty rect and a disconnected range reports none.
  section('opaque-range', () => {
    const OR = G.OpaqueRange;
    if (typeof OR !== 'function') return;
    const P = OR.prototype;
    const dead = new WeakMap();
    const brand = (self) => { if (!(self instanceof OR)) throw new TypeError('Illegal invocation'); };
    const zero = () => new G.DOMRect();
    fn('OpaqueRange', 'disconnect', 0, function disconnect() {
      brand(this);
      this._so = 0;
      this._eo = 0;
      dead.set(this, true);
    });
    fn('OpaqueRange', 'getBoundingClientRect', 0, function getBoundingClientRect() {
      brand(this);
      return zero();
    });
    fn('OpaqueRange', 'getClientRects', 0, function getClientRects() {
      brand(this);
      const rects = dead.get(this) ? [] : [zero()];
      return G.__diting_mkRectList(rects);
    });
    const want = ['disconnect', 'getBoundingClientRect', 'getClientRects', 'constructor'];
    const names = Object.getOwnPropertyNames(P);
    const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
    if (!seq.every((k, i) => k === names[i])) {
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    }
  });

  // Chrome 157 WebTransport datagram caps. The count is an unsigned long,
  // and 0 is stored as 1. datagrams is one duplex stream, not a plain object.
  section('wt-datagrams', () => {
    const C = G.WebTransportDatagramDuplexStream;
    const WT = G.WebTransport;
    if (typeof C !== 'function' || typeof WT !== 'function' || !C.prototype) return;
    const P = C.prototype;
    const slots = new WeakMap();
    const brand = (self) => { if (!(self instanceof C)) throw new TypeError('Illegal invocation'); };
    const open = (self) => {
      let s = slots.get(self);
      if (!s) {
        s = { readable: new G.ReadableStream(), writable: new G.WritableStream(), inBuf: 1, outBuf: 1 };
        slots.set(self, s);
      }
      return s;
    };
    const toCount = (v, name) => {
      let n;
      try { n = Number(v); }
      catch (e) { throw new TypeError("Failed to set the '" + name + "' property on 'WebTransportDatagramDuplexStream': " + e.message); }
      if (!Number.isFinite(n) || n === 0) n = 0;
      else {
        n = Math.sign(n) * Math.floor(Math.abs(n));
        const m = 0x100000000;
        n %= m;
        if (n < 0) n += m;
      }
      return n === 0 ? 1 : n;
    };
    const acc = (name, key) => def(P, name, {
      get: pin(function () { brand(this); return open(this)[key]; }, 'get ' + name),
      set: pin(function (v) { brand(this); open(this)[key] = toCount(v, name); }, 'set ' + name, 1),
      enumerable: true, configurable: true,
    });
    def(P, 'readable', { get: pin(function () { brand(this); return open(this).readable; }, 'get readable'), enumerable: true, configurable: true });
    def(P, 'writable', { get: pin(function () { brand(this); return open(this).writable; }, 'get writable'), enumerable: true, configurable: true });
    acc('incomingMaxBufferedDatagrams', 'inBuf');
    acc('outgoingMaxBufferedDatagrams', 'outBuf');
    Object.defineProperty(WT.prototype, 'datagrams', {
      get: pin(function () {
        if (this === null || (typeof this !== 'object' && typeof this !== 'function')) throw new TypeError('Illegal invocation');
        if (!this._datagrams) {
          const d = Object.create(P);
          open(d);
          this._datagrams = d;
        }
        return this._datagrams;
      }, 'get datagrams'),
      enumerable: true, configurable: true,
    });
  });

  // Chrome 157 MediaStreamTrackProcessor frame counters. Both stay 0: the
  // engine does not pull frames off the track. The constructor requires a
  // MediaStreamTrack and keeps one readable stream.
  section('mstp', () => {
    const Old = G.MediaStreamTrackProcessor;
    const Track = G.MediaStreamTrack;
    if (typeof Old !== 'function' || !Old.prototype || typeof Track !== 'function') return;
    const P = Old.prototype;
    const slots = new WeakMap();
    function MediaStreamTrackProcessor(init) {
      if (arguments.length < 1) throw new TypeError("Failed to construct 'MediaStreamTrackProcessor': 1 argument required, but only 0 present.");
      const missing = new TypeError("Failed to construct 'MediaStreamTrackProcessor': Failed to read the 'track' property from 'MediaStreamTrackProcessorInit': Required member is undefined.");
      if (init === null || typeof init !== 'object' || !('track' in init) || init.track === undefined) throw missing;
      if (!(init.track instanceof Track)) throw new TypeError("Failed to construct 'MediaStreamTrackProcessor': Failed to read the 'track' property from 'MediaStreamTrackProcessorInit': Failed to convert value to 'MediaStreamTrack'.");
      const o = Object.create(P);
      slots.set(o, { readable: new G.ReadableStream(), total: 0, discarded: 0 });
      return o;
    }
    pin(MediaStreamTrackProcessor, 'MediaStreamTrackProcessor', 1);
    MediaStreamTrackProcessor.prototype = P;
    Object.defineProperty(P, 'constructor', { value: MediaStreamTrackProcessor, writable: true, enumerable: false, configurable: true });
    Object.defineProperty(G, 'MediaStreamTrackProcessor', { value: MediaStreamTrackProcessor, writable: true, enumerable: false, configurable: true });
    const read = (name, pick) => def(P, name, {
      get: pin(function () {
        if (!(this instanceof MediaStreamTrackProcessor)) throw new TypeError('Illegal invocation');
        const s = slots.get(this);
        if (!s) throw new TypeError('Illegal invocation');
        return pick(s);
      }, 'get ' + name),
      enumerable: true, configurable: true,
    });
    read('readable', (s) => s.readable);
    read('totalFrames', (s) => s.total);
    read('discardedFrames', (s) => s.discarded);
  });

  // Chrome 157 TrustedTypePolicy.createParserOptions. The callback sees
  // {runScripts}. A missing callback is an error. The result carries no data.
  section('trusted-parser-options', () => {
    const Policy = G.TrustedTypePolicy;
    const Factory = G.TrustedTypePolicyFactory;
    const Opts = G.TrustedHTMLParserOptions;
    if (typeof Policy !== 'function' || typeof Factory !== 'function' || typeof Opts !== 'function') return;
    if (!Object.prototype.hasOwnProperty.call(Opts.prototype, Symbol.toStringTag)) {
      Object.defineProperty(Opts.prototype, Symbol.toStringTag, { value: 'TrustedHTMLParserOptions', configurable: true });
    }
    const slots = new WeakMap();
    const badOptions = new TypeError("Failed to execute 'createPolicy' on 'TrustedTypePolicyFactory': The provided value is not of type 'TrustedTypePolicyOptions'.");
    def(Factory.prototype, 'createPolicy', { value: pin(function createPolicy(name, options) {
      if (!(this instanceof Factory)) throw new TypeError('Illegal invocation');
      if (options !== undefined && options !== null && typeof options !== 'object') throw badOptions;
      const opts = options && typeof options === 'object' ? options : {};
      const o = Object.create(Policy.prototype);
      slots.set(o, {
        name: String(name),
        parse: typeof opts.createParserOptions === 'function' ? opts.createParserOptions : null,
      });
      return o;
    }, 'createPolicy', 1), writable: true, enumerable: true, configurable: true });
    def(Policy.prototype, 'name', { get: pin(function () {
      if (!(this instanceof Policy)) throw new TypeError('Illegal invocation');
      const s = slots.get(this);
      return s ? s.name : '';
    }, 'get name'), enumerable: true, configurable: true });
    fn('TrustedTypePolicy', 'createParserOptions', 0, function createParserOptions(options) {
      if (!(this instanceof Policy)) throw new TypeError('Illegal invocation');
      if (arguments.length && options !== undefined && options !== null && typeof options !== 'object') {
        throw new TypeError("Failed to execute 'createParserOptions' on 'TrustedTypePolicy': The provided value is not of type 'SetHTMLUnsafeOptions'.");
      }
      const s = slots.get(this);
      if (!s || typeof s.parse !== 'function') {
        const who = s && s.name ? s.name : '';
        throw new TypeError("Failed to execute 'createParserOptions' on 'TrustedTypePolicy': Policy " + who + "'s TrustedTypePolicyOptions did not specify a 'createParserOptions' member.");
      }
      const runScripts = !!(options && typeof options === 'object' && options.runScripts);
      s.parse({ runScripts: runScripts });
      return Object.create(Opts.prototype);
    });
  });

  // Chrome 157 animation triggers. Behaviors are none, pause, play, reset.
  // A trigger keeps each Animation once. ranges is one empty list.
  section('animation-trigger', () => {
    const AT = G.AnimationTrigger;
    const TT = G.TimelineTrigger;
    const List = G.TimelineTriggerRangeList;
    const Anim = G.Animation;
    if (typeof AT !== 'function' || typeof TT !== 'function' || typeof List !== 'function' || typeof Anim !== 'function') return;
    const BEHAVIORS = ['none', 'pause', 'play', 'reset'];
    const state = new WeakMap();
    const bag = (self) => {
      let s = state.get(self);
      if (!s) { s = { anims: [] }; state.set(self, s); }
      return s;
    };
    const brand = (self, C) => { if (!(self instanceof C)) throw new TypeError('Illegal invocation'); };
    const seat = (P, want) => {
      const names = Object.getOwnPropertyNames(P);
      const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
      if (seq.every((k, i) => k === names[i])) return;
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    };
    fn('AnimationTrigger', 'addAnimation', 2, function addAnimation(animation, behavior) {
      brand(this, AT);
      argc(arguments, 2, 'addAnimation', 'AnimationTrigger');
      if (!(animation instanceof Anim)) throw new TypeError("Failed to execute 'addAnimation' on 'AnimationTrigger': parameter 1 is not of type 'Animation'.");
      const b = String(behavior);
      if (BEHAVIORS.indexOf(b) < 0) throw new TypeError("Failed to execute 'addAnimation' on 'AnimationTrigger': The provided value '" + b + "' is not a valid enum value of type AnimationTriggerBehavior.");
      const anims = bag(this).anims;
      if (anims.indexOf(animation) < 0) anims.push(animation);
    });
    fn('AnimationTrigger', 'removeAnimation', 1, function removeAnimation(animation) {
      brand(this, AT);
      argc(arguments, 1, 'removeAnimation', 'AnimationTrigger');
      if (!(animation instanceof Anim)) throw new TypeError("Failed to execute 'removeAnimation' on 'AnimationTrigger': parameter 1 is not of type 'Animation'.");
      const anims = bag(this).anims;
      const i = anims.indexOf(animation);
      if (i >= 0) anims.splice(i, 1);
    });
    fn('AnimationTrigger', 'getAnimations', 0, function getAnimations() {
      brand(this, AT);
      return bag(this).anims.slice();
    });
    const lists = new WeakMap();
    def(TT.prototype, 'ranges', { get: pin(function () {
      brand(this, TT);
      let list = lists.get(this);
      if (!list) { list = Object.create(List.prototype); lists.set(this, list); }
      return list;
    }, 'get ranges'), enumerable: true, configurable: true });
    def(List.prototype, 'length', { get: pin(function () { brand(this, List); return 0; }, 'get length'), enumerable: true, configurable: true });
    fn('TimelineTriggerRangeList', 'item', 1, function item() {
      brand(this, List);
      argc(arguments, 1, 'item', 'TimelineTriggerRangeList');
      return null;
    });
    fn('TimelineTriggerRangeList', 'forEach', 1, function forEach() {
      brand(this, List);
      argc(arguments, 1, 'forEach', 'TimelineTriggerRangeList');
    });
    const empty = () => [][Symbol.iterator]();
    for (const n of ['entries', 'keys', 'values']) fn('TimelineTriggerRangeList', n, 0, function () { brand(this, List); return empty(); });
    seat(AT.prototype, ['addAnimation', 'getAnimations', 'removeAnimation', 'constructor']);
    seat(TT.prototype, ['ranges', 'constructor']);
    seat(List.prototype, ['entries', 'keys', 'values', 'forEach', 'length', 'item', 'constructor']);
  });

  // Chrome 157 TimelineTriggerRange. The constructor stays illegal and no
  // range is minted, so every read fails the internal brand check.
  section('timeline-trigger-range', () => {
    const C = G.TimelineTriggerRange;
    if (typeof C !== 'function' || !C.prototype) return;
    const P = C.prototype;
    const live = new WeakSet();
    const ro = (name) => def(P, name, { get: pin(function () {
      if (!live.has(this)) throw new TypeError('Illegal invocation');
      return null;
    }, 'get ' + name), enumerable: true, configurable: true });
    for (const n of ['timeline', 'activationRangeStart', 'activationRangeEnd', 'activeRangeStart', 'activeRangeEnd']) ro(n);
    const names = Object.getOwnPropertyNames(P);
    const want = ['timeline', 'activationRangeStart', 'activationRangeEnd', 'activeRangeStart', 'activeRangeEnd', 'constructor'];
    const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
    if (!seq.every((k, i) => k === names[i])) {
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    }
  });

  // Chrome 157 <camera>, <microphone>, and <usermedia>. setConstraints does
  // not open a device. Camera and microphone take a constraint set; user
  // media takes a stream-constraint dictionary.
  section('user-media-elements', () => {
    const map = G.__diting_htmlTagMap;
    if (!map || typeof G.HTMLElement !== 'function') return;
    const make = (name, tags) => {
      const C = { [name]: class extends G.HTMLElement {
        constructor(nid) {
          if (typeof nid !== 'number') throw new TypeError("Failed to construct '" + name + "': Illegal constructor");
          super(nid);
        }
      } }[name];
      Object.defineProperty(C, Symbol.hasInstance, { configurable: true, value(obj) {
        if (!obj || typeof obj.tagName !== 'string') return false;
        if (!tags.includes(String(obj.tagName).toLowerCase())) return false;
        const ns = obj.namespaceURI;
        return ns === undefined || ns === 'http://www.w3.org/1999/xhtml';
      } });
      try { Object.defineProperty(C.prototype, Symbol.toStringTag, { value: name, configurable: true }); } catch (e) {}
      Object.defineProperty(G, name, { value: C, writable: true, enumerable: false, configurable: true });
      for (const t of tags) map.set(t, C);
      return C;
    };
    const seat = (P, want) => {
      const names = Object.getOwnPropertyNames(P);
      const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
      if (seq.every((k, i) => k === names[i])) return;
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    };
    const roNull = (C, name) => def(C.prototype, name, { get: pin(function () {
      if (!(this instanceof C)) throw new TypeError('Illegal invocation');
      return null;
    }, 'get ' + name), enumerable: true, configurable: true });
    const dictOk = (v) => v === undefined || v === null || typeof v === 'object';
    const cam = make('HTMLCameraElement', ['camera']);
    const mic = make('HTMLMicrophoneElement', ['microphone']);
    const um = make('HTMLUserMediaElement', ['usermedia']);
    for (const [C, iface] of [[cam, 'HTMLCameraElement'], [mic, 'HTMLMicrophoneElement']]) {
      roNull(C, 'error');
      roNull(C, 'track');
      on(iface, 'cancel', 'error', 'track');
      fn(iface, 'setConstraints', 0, function setConstraints(constraints) {
        if (!(this instanceof C)) throw new TypeError('Illegal invocation');
        if (!dictOk(constraints)) throw new TypeError("Failed to execute 'setConstraints' on '" + iface + "': The provided value is not of type 'MediaTrackConstraintSet'.");
      });
      seat(C.prototype, ['error', 'oncancel', 'onerror', 'ontrack', 'track', 'setConstraints', 'constructor']);
    }
    roNull(um, 'stream');
    roNull(um, 'error');
    on('HTMLUserMediaElement', 'stream', 'cancel', 'error');
    fn('HTMLUserMediaElement', 'setConstraints', 0, function setConstraints(constraints) {
      if (!(this instanceof um)) throw new TypeError('Illegal invocation');
      if (!dictOk(constraints)) throw new TypeError("Failed to execute 'setConstraints' on 'HTMLUserMediaElement': The provided value is not of type 'HTMLMediaStreamConstraints'.");
      if (constraints && typeof constraints === 'object') {
        for (const key of ['video', 'audio']) {
          const v = constraints[key];
          if (v !== undefined && v !== null && typeof v !== 'object') {
            throw new TypeError("Failed to execute 'setConstraints' on 'HTMLUserMediaElement': Failed to read the '" + key + "' property from 'HTMLMediaStreamConstraints': The provided value is not of type 'MediaTrackConstraintSet'.");
          }
        }
      }
    });
    seat(um.prototype, ['onstream', 'stream', 'error', 'oncancel', 'onerror', 'setConstraints', 'constructor']);
  });

  // Chrome 157 XR members. Live sessions are never minted, so the new
  // getters fail the internal brand check. initiateRoomCapture rejects.
  section('xr-157', () => {
    const live = new WeakSet();
    const ro = (iface, name) => {
      const C = G[iface];
      if (typeof C !== 'function' || !C.prototype) return;
      def(C.prototype, name, { get: pin(function () {
        if (!live.has(this)) throw new TypeError('Illegal invocation');
        return null;
      }, 'get ' + name), enumerable: true, configurable: true });
    };
    ro('XRFrame', 'detectedPlanes');
    ro('XRLayerEvent', 'layer');
    ro('XRRenderState', 'layers');
    ro('XRSubImage', 'viewport');
    ro('XRSession', 'maxRenderLayers');
    fn('XRSession', 'initiateRoomCapture', 0, function initiateRoomCapture() {
      return Promise.reject(new TypeError("Failed to execute 'initiateRoomCapture' on 'XRSession': Illegal invocation"));
    });
    const seat = (iface, want) => {
      const C = G[iface];
      if (typeof C !== 'function' || !C.prototype) return;
      const P = C.prototype;
      const names = Object.getOwnPropertyNames(P);
      const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
      if (seq.every((k, i) => k === names[i])) return;
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    };
    seat('XRLayerEvent', ['layer', 'constructor']);
    seat('XRSubImage', ['viewport', 'constructor']);
  });

  // Chrome 157 XR layers and planes. No live object exists, so reads and
  // writes fail the brand check before any value conversion.
  section('xr-layers', () => {
    const live = new WeakSet();
    const acc = (iface, name, rw) => {
      const C = G[iface];
      if (typeof C !== 'function' || !C.prototype) return;
      const desc = { enumerable: true, configurable: true, get: pin(function () {
        if (!live.has(this)) throw new TypeError('Illegal invocation');
        return null;
      }, 'get ' + name) };
      if (rw) desc.set = pin(function () {
        if (!live.has(this)) throw new TypeError('Illegal invocation');
      }, 'set ' + name, 1);
      def(C.prototype, name, desc);
    };
    const seat = (iface, want) => {
      const C = G[iface];
      if (typeof C !== 'function' || !C.prototype) return;
      const P = C.prototype;
      const names = Object.getOwnPropertyNames(P);
      const seq = want.filter((k) => names.indexOf(k) >= 0).concat(names.filter((k) => want.indexOf(k) < 0));
      if (seq.every((k, i) => k === names[i])) return;
      const saved = [];
      for (const k of seq) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d && d.configurable) { saved.push([k, d]); delete P[k]; }
      }
      for (const [k, d] of saved) Object.defineProperty(P, k, d);
    };
    for (const n of ['space', 'orientation', 'onredraw']) acc('XRCubeLayer', n, true);
    for (const n of ['space', 'transform', 'width', 'height', 'onredraw']) acc('XRQuadLayer', n, true);
    for (const n of ['space', 'transform', 'radius', 'centralAngle', 'aspectRatio', 'onredraw']) acc('XRCylinderLayer', n, true);
    for (const n of ['planeSpace', 'polygon', 'orientation', 'lastChangedTime', 'semanticLabel']) acc('XRPlane', n, false);
    seat('XRCubeLayer', ['space', 'orientation', 'onredraw', 'constructor']);
    seat('XRQuadLayer', ['space', 'transform', 'width', 'height', 'onredraw', 'constructor']);
    seat('XRCylinderLayer', ['space', 'transform', 'radius', 'centralAngle', 'aspectRatio', 'onredraw', 'constructor']);
    seat('XRPlane', ['planeSpace', 'polygon', 'orientation', 'lastChangedTime', 'semanticLabel', 'constructor']);
    const method = (iface, name, len) => fn(iface, name, len, function () {
      if (!live.has(this)) throw new TypeError('Illegal invocation');
    });
    for (const n of ['entries', 'keys', 'values']) method('XRPlaneSet', n, 0);
    method('XRPlaneSet', 'forEach', 1);
    method('XRPlaneSet', 'has', 1);
    acc('XRPlaneSet', 'size', false);
    for (const n of ['textureWidth', 'textureHeight', 'textureArrayLength', 'ignoreDepthValues']) acc('XRProjectionLayer', n, false);
    for (const n of ['fixedFoveation', 'deltaPose']) acc('XRProjectionLayer', n, true);
    seat('XRPlaneSet', ['size', 'entries', 'forEach', 'has', 'keys', 'values', 'constructor']);
    seat('XRProjectionLayer', ['textureWidth', 'textureHeight', 'textureArrayLength', 'ignoreDepthValues', 'fixedFoveation', 'deltaPose', 'constructor']);
    for (const n of ['layout', 'mipLevels', 'needsRedraw']) acc('XRCompositionLayer', n, false);
    for (const n of ['blendTextureSourceAlpha', 'forceMonoPresentation', 'opacity']) acc('XRCompositionLayer', n, true);
    method('XRCompositionLayer', 'destroy', 0);
    for (const n of ['space', 'transform', 'radius', 'centralHorizontalAngle', 'upperVerticalAngle', 'lowerVerticalAngle', 'onredraw']) acc('XREquirectLayer', n, true);
    seat('XRCompositionLayer', ['layout', 'blendTextureSourceAlpha', 'forceMonoPresentation', 'opacity', 'mipLevels', 'needsRedraw', 'destroy', 'constructor']);
    seat('XREquirectLayer', ['space', 'transform', 'radius', 'centralHorizontalAngle', 'upperVerticalAngle', 'lowerVerticalAngle', 'onredraw', 'constructor']);
  });

  // Chrome 157 iterator helpers. join defaults to ",". includes uses
  // SameValueZero, so NaN matches NaN. A receiver without next throws when
  // next is called.
  section('iterator-helpers-157', () => {
    const It = G.Iterator;
    if (typeof It !== 'function' || !It.prototype) return;
    const P = It.prototype;
    const same = (a, b) => a === b || (a !== a && b !== b);
    if (!Object.prototype.hasOwnProperty.call(P, 'join')) {
      const join = function join(separator) {
        const sep = arguments.length ? String(separator) : ',';
        let out = '', first = true, step = this.next();
        while (!step.done) {
          if (!first) out += sep;
          first = false;
          const v = step.value;
          out += v == null ? '' : String(v);
          step = this.next();
        }
        return out;
      };
      pin(join, 'join', 1);
      def(P, 'join', { value: join, writable: true, enumerable: true, configurable: true });
    }
    if (!Object.prototype.hasOwnProperty.call(P, 'includes')) {
      const includes = function includes(value) {
        let step = this.next();
        while (!step.done) {
          if (same(step.value, value)) return true;
          step = this.next();
        }
        return false;
      };
      pin(includes, 'includes', 1);
      def(P, 'includes', { value: includes, writable: true, enumerable: true, configurable: true });
    }
  });

  // Chrome 157 Element.pseudo / CSSPseudoElement. Only these types return an
  // object; anything else is null. The same (parent, type) pair is cached.
  // A top-level pseudo's parent is the element. A nested one's parent is the
  // pseudo it was asked on. element is always the originating element.
  section('css-pseudo', () => {
    const C = G.CSSPseudoElement;
    const E = G.Element;
    if (typeof C !== 'function' || typeof E !== 'function') return;
    const ALLOW = new Set(['::before', '::after', '::marker', '::backdrop', '::view-transition']);
    const slots = new WeakMap();
    const cache = new WeakMap();
    const P = C.prototype;
    const brand = (self) => {
      const s = slots.get(self);
      if (!s) throw new TypeError('Illegal invocation');
      return s;
    };
    const getter = (name, pick) => {
      const get = pin(function () { return pick(brand(this)); }, 'get ' + name);
      def(P, name, { get, enumerable: true, configurable: true });
    };
    getter('type', (s) => s.type);
    getter('element', (s) => s.element);
    getter('parent', (s) => s.parent);
    const pseudo = function pseudo(type) {
      const parent = this;
      const onEl = parent instanceof E;
      const onPseudo = slots.has(parent);
      if (!onEl && !onPseudo) throw new TypeError('Illegal invocation');
      if (arguments.length < 1) {
        const iface = onEl ? 'Element' : 'CSSPseudoElement';
        throw new TypeError("Failed to execute 'pseudo' on '" + iface + "': 1 argument required, but only 0 present.");
      }
      const t = String(type);
      if (!ALLOW.has(t)) return null;
      let m = cache.get(parent);
      if (!m) { m = new Map(); cache.set(parent, m); }
      let obj = m.get(t);
      if (!obj) {
        const element = onEl ? parent : slots.get(parent).element;
        obj = Object.create(P);
        slots.set(obj, { type: t, element, parent });
        m.set(t, obj);
      }
      return obj;
    };
    pin(pseudo, 'pseudo', 1);
    def(P, 'pseudo', { value: pseudo, writable: true, enumerable: true, configurable: true });
    def(E.prototype, 'pseudo', { value: pseudo, writable: true, enumerable: true, configurable: true });
    const ctor = Object.getOwnPropertyDescriptor(P, 'constructor');
    if (ctor && ctor.configurable) {
      delete P.constructor;
      Object.defineProperty(P, 'constructor', ctor);
    }
    if (!Object.prototype.hasOwnProperty.call(P, Symbol.toStringTag)) {
      Object.defineProperty(P, Symbol.toStringTag, { value: 'CSSPseudoElement', configurable: true });
    }
  });
})();
