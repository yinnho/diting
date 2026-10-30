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
