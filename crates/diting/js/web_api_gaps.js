// Small Web API surfaces the Chrome diff battery found missing (split from
// bootstrap.js): scheduler.postTask/yield, Blob.stream, document.startView-
// Transition, the document.queryCommand* family, xmlns on XMLSerializer's
// HTML output, and a WAAPI animation object with a live playState plus getAnimations() registries.
// Everything is call-time and feature-checked so existing definitions win.
(() => {
  const def = (obj, name, value) => Object.defineProperty(obj, name, { value, writable: true, configurable: true });

  // ---- scheduler --------------------------------------------------------
  const PRIORITY_DELAY = { 'user-blocking': 0, 'user-visible': 1, background: 4 };
  const sched = globalThis.scheduler && typeof globalThis.scheduler === 'object' ? globalThis.scheduler : null;
  if (sched && typeof sched.postTask !== 'function') {
    def(Object.getPrototypeOf(sched) !== Object.prototype ? Object.getPrototypeOf(sched) : sched, 'postTask', function postTask(callback, options) {
      if (typeof callback !== 'function') return Promise.reject(new TypeError("Failed to execute 'postTask' on 'Scheduler': parameter 1 is not of type 'Function'."));
      const o = options || {};
      const signal = o.signal;
      if (signal && signal.aborted) return Promise.reject(signal.reason || new DOMException('The operation was aborted.', 'AbortError'));
      const delay = Math.max(0, Number(o.delay) || 0) + (PRIORITY_DELAY[o.priority] ?? 1);
      return new Promise((resolve, reject) => {
        let done = false;
        const t = setTimeout(() => {
          if (done) return;
          done = true;
          try { resolve(callback()); } catch (e) { reject(e); }
        }, delay);
        if (signal && typeof signal.addEventListener === 'function') {
          signal.addEventListener('abort', () => {
            if (done) return;
            done = true;
            clearTimeout(t);
            reject(signal.reason || new DOMException('The operation was aborted.', 'AbortError'));
          }, { once: true });
        }
      });
    });
    def(Object.getPrototypeOf(sched) !== Object.prototype ? Object.getPrototypeOf(sched) : sched, 'yield', function () {
      return new Promise((resolve) => setTimeout(resolve, 0));
    });
  }

  // ---- Blob.stream ------------------------------------------------------
  if (typeof Blob === 'function' && typeof Blob.prototype.stream !== 'function' && typeof ReadableStream === 'function') {
    def(Blob.prototype, 'stream', function stream() {
      const blob = this;
      return new ReadableStream({
        async start(controller) {
          const buf = await blob.arrayBuffer();
          if (buf.byteLength) controller.enqueue(new Uint8Array(buf));
          controller.close();
        },
      });
    });
    _markNative(Blob.prototype.stream);
  }

  // ---- View Transitions -------------------------------------------------
  if (typeof Document !== 'undefined' && typeof Document.prototype.startViewTransition !== 'function') {
    def(Document.prototype, 'startViewTransition', function startViewTransition(update) {
      const cb = typeof update === 'function' ? update : (update && update.update);
      const updateCallbackDone = new Promise((resolve, reject) => {
        Promise.resolve().then(() => (typeof cb === 'function' ? cb() : undefined)).then(resolve, reject);
      });
      const ready = updateCallbackDone.then(() => undefined);
      const finished = ready.then(() => undefined);
      ready.catch(() => {}); finished.catch(() => {}); updateCallbackDone.catch(() => {});
      return { updateCallbackDone, ready, finished, skipTransition() {}, types: new Set() };
    });
    _markNative(Document.prototype.startViewTransition);
  }

  // ---- document.queryCommand* ------------------------------------------
  const COMMANDS = new Set(['backcolor','bold','contentreadonly','copy','createlink','cut','decreasefontsize','defaultparagraphseparator','delete','fontname','fontsize','forecolor','formatblock','forwarddelete','hilitecolor','increasefontsize','indent','inserthorizontalrule','inserthtml','insertimage','insertlinebreak','insertorderedlist','insertparagraph','inserttext','insertunorderedlist','italic','justifycenter','justifyfull','justifyleft','justifyright','outdent','paste','redo','removeformat','selectall','strikethrough','styleWithCSS'.toLowerCase(),'subscript','superscript','underline','undo','unlink','usecss']);
  if (typeof Document !== 'undefined') {
    const name = (c) => String(c || '').toLowerCase();
    if (typeof Document.prototype.queryCommandSupported !== 'function') def(Document.prototype, 'queryCommandSupported', function queryCommandSupported(c) { return COMMANDS.has(name(c)); });
    if (typeof Document.prototype.queryCommandEnabled !== 'function') def(Document.prototype, 'queryCommandEnabled', function queryCommandEnabled(c) {
      const el = this.activeElement;
      const editable = !!el && (el.isContentEditable || (typeof _ns_isTextEntry === 'function' && _ns_isTextEntry(el)));
      return COMMANDS.has(name(c)) && (editable || ['copy', 'selectall'].includes(name(c)));
    });
    if (typeof Document.prototype.queryCommandState !== 'function') def(Document.prototype, 'queryCommandState', function queryCommandState() { return false; });
    if (typeof Document.prototype.queryCommandIndeterm !== 'function') def(Document.prototype, 'queryCommandIndeterm', function queryCommandIndeterm() { return false; });
    if (typeof Document.prototype.queryCommandValue !== 'function') def(Document.prototype, 'queryCommandValue', function queryCommandValue() { return ''; });
    for (const m of ['queryCommandSupported', 'queryCommandEnabled', 'queryCommandState', 'queryCommandIndeterm', 'queryCommandValue']) _markNative(Document.prototype[m]);
  }

  // ---- XMLSerializer: HTML elements carry the XHTML namespace ----------
  if (typeof XMLSerializer === 'function') {
    const orig = XMLSerializer.prototype.serializeToString;
    def(XMLSerializer.prototype, 'serializeToString', function serializeToString(node) {
      const s = orig.call(this, node);
      const el = node && node.nodeType === 9 ? node.documentElement : node;
      if (!el || el.nodeType !== 1 || el.namespaceURI !== 'http://www.w3.org/1999/xhtml' || el.hasAttribute('xmlns')) return s;
      return s.replace(/^(\s*(?:<!DOCTYPE[^>]*>)?\s*<[A-Za-z][^\s/>]*)/, '$1 xmlns="http://www.w3.org/1999/xhtml"');
    });
    _markNative(XMLSerializer.prototype.serializeToString);
  }

  // ---- WAAPI: real Animation objects come from platform/animation.impl.js;
  // this glue keeps the native animationstart/animationend lifecycle events.
  const origAnimate = Element.prototype.animate;
  if (typeof origAnimate === 'function') {
    Element.prototype.animate = function animate(keyframes, options) {
      const anim = __diting_animate(this, keyframes, options, animate);
      origAnimate.call(this, keyframes, options);
      return anim;
    };
    _markNative(Element.prototype.animate);
    def(Element.prototype, 'getAnimations', function getAnimations(opts) { return __diting_getAnimations(this, !!(opts && opts.subtree), ''); });
    _markNative(Element.prototype.getAnimations);
    if (typeof Document !== 'undefined' && typeof Document.prototype.getAnimations !== 'function') {
      def(Document.prototype, 'getAnimations', function getAnimations() { return __diting_getAnimations(this, false, 'doc'); });
      _markNative(Document.prototype.getAnimations);
    }
    if (typeof ShadowRoot !== 'undefined' && typeof ShadowRoot.prototype.getAnimations !== 'function') {
      def(ShadowRoot.prototype, 'getAnimations', function getAnimations() { return __diting_getAnimations(this, true, ''); });
      _markNative(ShadowRoot.prototype.getAnimations);
    }
  }

  // ---- Credential id/type (刀57) ----------------------------------------
  // Chrome owns id/type on Credential.prototype only (ro:ec); Password/
  // FederatedCredential inherit. The mint carried stub getters (undefined
  // for real values), so before this block PasswordCredential had to keep
  // its own copies — the shape audit's last Credential-family extras. The
  // real getters read the _id/_type slots the constructors set; stub_members
  // (later, fill-if-missing) sees them present and stays out.
  if (typeof Credential === 'function') {
    const gid = function id() { return this._id; };
    const gtype = function type() { return this._type; };
    _markNative(gid);
    _markNative(gtype);
    Object.defineProperty(Credential.prototype, 'id', { get: gid, enumerable: true, configurable: true });
    Object.defineProperty(Credential.prototype, 'type', { get: gtype, enumerable: true, configurable: true });
  }

  // ---- RTC satellite faces (刀62) --------------------------------------
  // RTCDataChannel + RTCRtpTransceiver whole prototype faces and the two
  // constructible event subclasses, every value Chrome-145-probed. This file
  // runs AFTER global_surface minted the shells, so all of it installs
  // load-time (the audit sees it); members land in Chrome's raw order with
  // truth flags (ec everywhere except constructor's -cw).
  if (typeof RTCDataChannel === 'function') {
    const P = RTCDataChannel.prototype;
    for (const k of Object.getOwnPropertyNames(P)) { try { delete P[k]; } catch (e) {} }
    const dg = (n, get, set) => Object.defineProperty(P, n, set ? { get, set, enumerable: true, configurable: true } : { get, enumerable: true, configurable: true });
    const df = (n, f, l) => { if (l !== undefined) Object.defineProperty(f, 'length', { value: l, configurable: true }); _markNative(f); Object.defineProperty(P, n, { value: f, writable: true, enumerable: true, configurable: true }); };
    const onDef = (n) => dg(n, new Function('n', 'return function () { const o = this.__on; return o && o[n] !== undefined ? o[n] : null; };')(n),
      new Function('n', 'return function (v) { if (!this.__on) Object.defineProperty(this, "__on", { value: {}, writable: true, configurable: true }); this.__on[n] = typeof v === "function" ? v : null; };')(n));
    // truth raw order: label … send, constructor LAST
    dg('label', function label() { const d = this.__dc; return d ? d.label : undefined; });
    dg('ordered', function ordered() { const d = this.__dc; return d ? d.ordered !== false : undefined; });
    dg('maxPacketLifeTime', function maxPacketLifeTime() { const d = this.__dc; return d ? d.maxPacketLifeTime : undefined; });
    dg('maxRetransmits', function maxRetransmits() { const d = this.__dc; return d ? d.maxRetransmits : undefined; });
    dg('protocol', function protocol() { const d = this.__dc; return d ? d.protocol : undefined; });
    dg('negotiated', function negotiated() { const d = this.__dc; return d ? d.negotiated : undefined; });
    dg('id', function id() { const d = this.__dc; return d ? d.id : undefined; });
    dg('readyState', function readyState() { const d = this.__dc; return d ? d.rs : undefined; });
    dg('bufferedAmount', function bufferedAmount() { const d = this.__dc; return d ? d.ba || 0 : undefined; });
    dg('bufferedAmountLowThreshold', function bufferedAmountLowThreshold() { const d = this.__dc; return d ? d.batlt || 0 : undefined; },
      function bufferedAmountLowThreshold(v) { const d = this.__dc; if (d) d.batlt = Number(v); });
    onDef('onopen'); onDef('onbufferedamountlow'); onDef('onerror'); onDef('onclosing'); onDef('onclose'); onDef('onmessage');
    dg('binaryType', function binaryType() { const d = this.__dc; return d ? d.bt || 'arraybuffer' : undefined; },
      function binaryType(v) { const d = this.__dc; if (d && (v === 'blob' || v === 'arraybuffer')) d.bt = v; });
    dg('reliable', function reliable() { const d = this.__dc; return d ? d.maxPacketLifeTime == null && d.maxRetransmits == null : undefined; });
    df('close', function close() { const d = this.__dc; if (d && d.rs !== 'closed') d.rs = 'closing'; }, 0);
    df('send', function send(data) {
      const d = this.__dc;
      if (d && d.rs !== 'open') throw new DOMException("Failed to execute 'send' on 'RTCDataChannel': The RTCDataChannel's readyState is not 'open'.", 'InvalidStateError');
    }, 1);
    Object.defineProperty(P, 'constructor', { value: RTCDataChannel, writable: true, enumerable: false, configurable: true });
  }
  if (typeof RTCRtpTransceiver === 'function') {
    const P = RTCRtpTransceiver.prototype;
    for (const k of Object.getOwnPropertyNames(P)) { try { delete P[k]; } catch (e) {} }
    const dg = (n, get, set) => Object.defineProperty(P, n, set ? { get, set, enumerable: true, configurable: true } : { get, enumerable: true, configurable: true });
    const df = (n, f, l) => { if (l !== undefined) Object.defineProperty(f, 'length', { value: l, configurable: true }); _markNative(f); Object.defineProperty(P, n, { value: f, writable: true, enumerable: true, configurable: true }); };
    const setSlot = (o, k, v) => Object.defineProperty(o, k, { value: v, writable: true, configurable: true });
    // probed header-extension tables (audio 5 / video 16); before negotiation
    // the negotiated view answers every URI as 'stopped'.
    const HDR_A = [
      { uri: 'urn:ietf:params:rtp-hdrext:ssrc-audio-level', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time', direction: 'sendrecv' },
      { uri: 'http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01', direction: 'sendrecv' },
      { uri: 'urn:ietf:params:rtp-hdrext:sdes:mid', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/abs-capture-time', direction: 'stopped' },
    ];
    const HDR_V = [
      { uri: 'urn:ietf:params:rtp-hdrext:toffset', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time', direction: 'sendrecv' },
      { uri: 'urn:3gpp:video-orientation', direction: 'sendrecv' },
      { uri: 'http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/playout-delay', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/video-content-type', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/video-timing', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/color-space', direction: 'sendrecv' },
      { uri: 'urn:ietf:params:rtp-hdrext:sdes:mid', direction: 'sendrecv' },
      { uri: 'urn:ietf:params:rtp-hdrext:sdes:rtp-stream-id', direction: 'sendrecv' },
      { uri: 'urn:ietf:params:rtp-hdrext:sdes:repaired-rtp-stream-id', direction: 'sendrecv' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/corruption-detection', direction: 'stopped' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/abs-capture-time', direction: 'stopped' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/generic-frame-descriptor-00', direction: 'stopped' },
      { uri: 'https://aomediacodec.github.io/av1-rtp-spec/#dependency-descriptor-rtp-header-extension', direction: 'stopped' },
      { uri: 'http://www.webrtc.org/experiments/rtp-hdrext/video-layers-allocation00', direction: 'stopped' },
    ];
    // truth raw order: mid, sender, receiver, stopped, direction,
    // currentDirection, the four header-extension methods, stop, constructor
    dg('mid', function mid() { return this.__mid === undefined ? null : this.__mid; });
    dg('sender', function sender() { return this.__sender || null; });
    dg('receiver', function receiver() { return this.__receiver || null; });
    dg('stopped', function stopped() { return !!this.__stopped; });
    // 刀62 truth: invalid direction values are silently ignored (probed), but
    // a stopped transceiver throws InvalidStateError; stop() itself parks
    // direction at 'stopped'.
    dg('direction', function direction() { return this.__dirOv || 'sendrecv'; },
      function direction(v) {
        if (this.__stopped) throw new DOMException("Failed to set the 'direction' property on 'RTCRtpTransceiver': The transceiver is stopped.", 'InvalidStateError');
        if (['sendrecv', 'sendonly', 'recvonly', 'inactive'].includes(v)) setSlot(this, '__dirOv', v);
      });
    dg('currentDirection', function currentDirection() { return this.__curDir === undefined ? null : this.__curDir; });
    df('getHeaderExtensionsToNegotiate', function getHeaderExtensionsToNegotiate() {
      const src = this.__hdrOv || (this.__xkind === 'audio' ? HDR_A : HDR_V);
      return src.map((e) => ({ uri: e.uri, direction: e.direction }));
    }, 0);
    df('getNegotiatedHeaderExtensions', function getNegotiatedHeaderExtensions() {
      const src = this.__xkind === 'audio' ? HDR_A : HDR_V;
      return src.map((e) => ({ uri: e.uri, direction: 'stopped' }));
    }, 0);
    df('setCodecPreferences', function setCodecPreferences(codecs) {
      if (!Array.isArray(codecs) && codecs !== undefined) throw new TypeError("Failed to execute 'setCodecPreferences' on 'RTCRtpTransceiver': The provided value is not of type 'sequence'.");
    }, 1);
    df('setHeaderExtensionsToNegotiate', function setHeaderExtensionsToNegotiate(extensions) {
      if (!Array.isArray(extensions)) throw new TypeError("Failed to execute 'setHeaderExtensionsToNegotiate' on 'RTCRtpTransceiver': The provided value is not of type 'sequence'.");
      if (extensions.length === 0) throw new DOMException("Failed to execute 'setHeaderExtensionsToNegotiate' on 'RTCRtpTransceiver': The list is empty.", 'InvalidModificationError');
      for (const e of extensions) {
        if (!e || typeof e !== 'object' || typeof e.uri !== 'string' || !['sendrecv', 'sendonly', 'recvonly', 'inactive', 'stopped'].includes(e.direction))
          throw new TypeError("Failed to execute 'setHeaderExtensionsToNegotiate' on 'RTCRtpTransceiver': The provided value is not of type 'sequence<RTCHeaderExtensionCapability>'.");
      }
      setSlot(this, '__hdrOv', extensions.map((e) => ({ uri: e.uri, direction: e.direction })));
    }, 1);
    df('stop', function stop() {
      if (!this.__stopped) { setSlot(this, '__stopped', true); setSlot(this, '__dirOv', 'stopped'); }
    }, 0);
    Object.defineProperty(P, 'constructor', { value: RTCRtpTransceiver, writable: true, enumerable: false, configurable: true });
  }
  {
    // RTCTrackEvent / RTCPeerConnectionIceErrorEvent are CONSTRUCTIBLE in
    // Chrome with required init members (probed: garbage receiver → the
    // 'receiver' TypeError; {} → the errorCode one) — rebind the inert shell
    // classes to real constructors. Reflect.construct(Event, …, NewTarget)
    // keeps the Event machinery and the subclass brand.
    const OldTE = globalThis.RTCTrackEvent;
    if (typeof OldTE === 'function') {
      const syms = Object.getOwnPropertySymbols(OldTE.prototype);
      const TE = function RTCTrackEvent(type, init) {
        if (!new.target) throw new TypeError("Failed to construct 'RTCTrackEvent': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
        const i = init && typeof init === 'object' ? init : {};
        if (!(i.receiver instanceof globalThis.RTCRtpReceiver)) throw new TypeError("Failed to construct 'RTCTrackEvent': Failed to read the 'receiver' property from 'RTCTrackEventInit': The provided value is not of type 'RTCRtpReceiver'.");
        if (!(i.track instanceof globalThis.MediaStreamTrack)) throw new TypeError("Failed to construct 'RTCTrackEvent': Failed to read the 'track' property from 'RTCTrackEventInit': The provided value is not of type 'MediaStreamTrack'.");
        if (!(i.transceiver instanceof globalThis.RTCRtpTransceiver)) throw new TypeError("Failed to construct 'RTCTrackEvent': Failed to read the 'transceiver' property from 'RTCTrackEventInit': The provided value is not of type 'RTCRtpTransceiver'.");
        if (i.streams !== undefined && !Array.isArray(i.streams)) throw new TypeError("Failed to construct 'RTCTrackEvent': Failed to read the 'streams' property from 'RTCTrackEventInit': The provided value is not of type 'sequence'.");
        const ev = Reflect.construct(Event, [type], TE);
        Object.defineProperties(ev, {
          __teR: { value: i.receiver, writable: true, configurable: true },
          __teT: { value: i.track, writable: true, configurable: true },
          __teX: { value: i.transceiver, writable: true, configurable: true },
          __teS: { value: Object.freeze(Array.isArray(i.streams) ? i.streams.slice() : []), writable: true, configurable: true },
        });
        return ev;
      };
      const P = TE.prototype;
      Object.setPrototypeOf(P, Event.prototype);  // 刀62: parent chain (truth: Event)
      delete P.constructor;
      const dg = (n, get) => Object.defineProperty(P, n, { get, enumerable: true, configurable: true });
      dg('receiver', function receiver() { return this.__teR === undefined ? undefined : this.__teR; });
      dg('track', function track() { return this.__teT === undefined ? undefined : this.__teT; });
      dg('streams', function streams() { return this.__teS === undefined ? undefined : this.__teS; });
      dg('transceiver', function transceiver() { return this.__teX === undefined ? undefined : this.__teX; });
      Object.defineProperty(P, 'constructor', { value: TE, writable: true, enumerable: false, configurable: true });
      for (const s of syms) { const ds = Object.getOwnPropertyDescriptor(OldTE.prototype, s); if (ds) Object.defineProperty(P, s, ds); }
      globalThis.RTCTrackEvent = TE;
    }
    const OldIEE = globalThis.RTCPeerConnectionIceErrorEvent;
    if (typeof OldIEE === 'function') {
      const syms = Object.getOwnPropertySymbols(OldIEE.prototype);
      const IEE = function RTCPeerConnectionIceErrorEvent(type, init) {
        if (!new.target) throw new TypeError("Failed to construct 'RTCPeerConnectionIceErrorEvent': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
        const i = init && typeof init === 'object' ? init : {};
        if (i.errorCode === undefined) throw new TypeError("Failed to construct 'RTCPeerConnectionIceErrorEvent': Failed to read the 'errorCode' property from 'RTCPeerConnectionIceErrorEventInit': The provided value is not of type 'unsigned short'.");
        const ev = Reflect.construct(Event, [type], IEE);
        Object.defineProperties(ev, {
          __iA: { value: i.address === undefined ? null : i.address, writable: true, configurable: true },
          __iP: { value: i.port === undefined ? null : i.port, writable: true, configurable: true },
          __iH: { value: i.hostCandidate === undefined ? '' : i.hostCandidate, writable: true, configurable: true },
          __iU: { value: i.url === undefined ? '' : i.url, writable: true, configurable: true },
          __iC: { value: i.errorCode, writable: true, configurable: true },
          __iT: { value: i.errorText === undefined ? '' : i.errorText, writable: true, configurable: true },
        });
        return ev;
      };
      const P = IEE.prototype;
      Object.setPrototypeOf(P, Event.prototype);  // 刀62: parent chain (truth: Event)
      delete P.constructor;
      const dg = (n, get) => Object.defineProperty(P, n, { get, enumerable: true, configurable: true });
      dg('address', function address() { return this.__iA === undefined ? undefined : this.__iA; });
      dg('port', function port() { return this.__iP === undefined ? undefined : this.__iP; });
      dg('hostCandidate', function hostCandidate() { return this.__iH === undefined ? undefined : this.__iH; });
      dg('url', function url() { return this.__iU === undefined ? undefined : this.__iU; });
      dg('errorCode', function errorCode() { return this.__iC === undefined ? undefined : this.__iC; });
      dg('errorText', function errorText() { return this.__iT === undefined ? undefined : this.__iT; });
      Object.defineProperty(P, 'constructor', { value: IEE, writable: true, enumerable: false, configurable: true });
      for (const s of syms) { const ds = Object.getOwnPropertyDescriptor(OldIEE.prototype, s); if (ds) Object.defineProperty(P, s, ds); }
      globalThis.RTCPeerConnectionIceErrorEvent = IEE;
    }
  }

  // ---- GPUSupportedLimits 31 limit getters (刀60) --------------------------
  // Chrome 145 lists 31 ro:ec limit getters on GPUSupportedLimits.prototype;
  // the engine's mint carried only `constructor`. No instance is reachable in
  // EITHER world — Chrome for Testing 145 headless answers requestAdapter()
  // with null (probed), and so does the engine — so this face is pure
  // prototype shape: the getters read an internal _lim store and answer
  // undefined on bare objects, exactly what an un-slotted read would show.
  // Order follows truth with `constructor` moved LAST in-file (刀59 trick:
  // the raw order then equals truth, gen_shape_fix emits no ORDER entry, and
  // nothing ever reorders against this prototype).
  if (typeof GPUSupportedLimits === 'function') {
    const LIM = ['maxTextureDimension1D', 'maxTextureDimension2D', 'maxTextureDimension3D',
      'maxTextureArrayLayers', 'maxBindGroups', 'maxBindGroupsPlusVertexBuffers',
      'maxBindingsPerBindGroup', 'maxDynamicUniformBuffersPerPipelineLayout',
      'maxDynamicStorageBuffersPerPipelineLayout', 'maxSampledTexturesPerShaderStage',
      'maxSamplersPerShaderStage', 'maxStorageBuffersPerShaderStage',
      'maxStorageTexturesPerShaderStage', 'maxUniformBuffersPerShaderStage',
      'maxUniformBufferBindingSize', 'maxStorageBufferBindingSize',
      'minUniformBufferOffsetAlignment', 'minStorageBufferOffsetAlignment',
      'maxVertexBuffers', 'maxBufferSize', 'maxVertexAttributes',
      'maxVertexBufferArrayStride', 'maxInterStageShaderVariables', 'maxColorAttachments',
      'maxColorAttachmentBytesPerSample', 'maxComputeWorkgroupStorageSize',
      'maxComputeInvocationsPerWorkgroup', 'maxComputeWorkgroupSizeX',
      'maxComputeWorkgroupSizeY', 'maxComputeWorkgroupSizeZ',
      'maxComputeWorkgroupsPerDimension'];
    const P = GPUSupportedLimits.prototype;
    for (const k of LIM) {
      const g = new Function('k', 'return function ' + k + '() { const l = this._lim; return l ? l[k] : undefined; };')(k);
      _markNative(g);
      Object.defineProperty(P, k, { get: g, enumerable: true, configurable: true });
    }
    const cd = Object.getOwnPropertyDescriptor(P, 'constructor');
    if (cd && cd.configurable) { delete P.constructor; Object.defineProperty(P, 'constructor', cd); }
  }
})();

// 刀88: the CSS namespace's two object faces + real typed-om serialization.
// The unit factories and CSSUnitValue ctor live in bootstrap.js (they need
// nothing late); highlights/paintWorklet need the global_surface minted
// HighlightRegistry/Worklet prototypes, and toString rides the shared
// CSSStyleValue.prototype so CSSUnitValue keeps its truth member set.
(function () {
  if (!globalThis.CSS) return;
  Object.defineProperty(globalThis.CSS, 'highlights', { value: Object.create(globalThis.HighlightRegistry.prototype), writable: true, enumerable: true, configurable: true });
  // (刀88) empty registry: size is a getter counting zero highlights
  // (installed before stub_members' fill, so the getter survives).
  Object.defineProperty(globalThis.HighlightRegistry.prototype, 'size', { get() { return 0; }, enumerable: true, configurable: true });
  Object.defineProperty(globalThis.CSS, 'paintWorklet', { value: Object.create(globalThis.Worklet.prototype), writable: true, enumerable: true, configurable: true });
  Object.defineProperty(globalThis.CSSStyleValue.prototype, 'toString', {
    value: function toString() {
      if (this && typeof this === 'object' && this._u !== undefined && this._v !== undefined) {
        return this._u === 'number' ? String(this._v) : this._u === 'percent' ? this._v + '%' : this._v + this._u;
      }
      return Object.prototype.toString.call(this);
    },
    writable: true, enumerable: true, configurable: true,
  });
})();
