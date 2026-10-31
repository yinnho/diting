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
  });

  // ---- legacy constants ---------------------------------------------------------
  section('constants', () => {
    for (const [n, v] of [['CONNECTING', 0], ['OPEN', 1], ['CLOSING', 2], ['CLOSED', 3]]) cst('WebSocket', n, v, true);
    for (const [n, v] of [['CONNECTING', 0], ['OPEN', 1], ['CLOSED', 2]]) cst('EventSource', n, v, true);
    for (const [n, v] of [['TEMPORARY', 0], ['PERSISTENT', 1]]) cst('Window', n, v, true);
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

  // ---- File / FileList-adjacent / media / geometry -------------------------------
  section('file', () => {
    ro('File', 'lastModifiedDate', function () { return new Date(this.lastModified); });
    ro('File', 'webkitRelativePath', () => '');
    ro('ImageData', 'colorSpace', () => 'srgb');
    ro('ImageData', 'pixelFormat', () => 'rgba-unorm8');
    // Chrome's ImageData instance owns `data` while the prototype also answers it; the
    // setter turns the constructor's plain assignment into that own data property.
    def(proto('ImageData'), 'data', {
      get: pin(function () { const d = Object.getOwnPropertyDescriptor(obj(this), 'data'); return d ? d.value : undefined; }, 'get data'),
      set: pin(function (v) { Object.defineProperty(obj(this), 'data', { value: v, writable: true, enumerable: true, configurable: true }); }, 'set data', 1),
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
    for (const cn of ['SVGPolygonElement', 'SVGPolylineElement']) {
      const list = function (s) {
        if (s.v.pts) return s.v.pts;
        const C = G.SVGPointList, o = (C && C.prototype) ? Object.create(C.prototype) : {};
        return (s.v.pts = o);
      };
      ro(cn, 'points', function (s) { return list(s); });
      ro(cn, 'animatedPoints', function (s) { return list(s); });
    }
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
})();
