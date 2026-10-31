// 刀78: constructor argument ladders. Chrome rejects `new X()` with "N argument(s) required, but only 0 present." for
// these interfaces (captured from Chrome 145, k79-ctor); the engine's classes were constructible without arguments.
// The global is replaced by a thin wrapper that checks the count and delegates through Reflect.construct, keeping the
// ORIGINAL prototype object (instanceof, constructor, subclassing, statics all carry over); engine-internal creators
// always pass their arguments. The nid-based node classes additionally reject the zero-argument page form
// (`new Node()`/`new Element()` ... is an Illegal constructor; the engine mints them with a numeric node id).
(() => {
  const G = globalThis;
  const mark = G.__diting_markNative || ((f) => f);
  const ARGS = {"AudioBuffer":1,"BroadcastChannel":1,"CloseEvent":1,"ContentVisibilityAutoStateChangeEvent":1,"ErrorEvent":1,"EventSource":1,"ImageData":2,"MessageEvent":1,"MutationObserver":1,"OffscreenCanvas":2,"PerformanceMark":1,"ProgressEvent":1,"RTCError":1,"ResizeObserver":1,"SharedWorker":1,"ToggleEvent":1,"Touch":1,"TouchEvent":1,"Worker":1};
  // 刀83: interfaces Chrome refuses to construct with ANY argument list
  // (k79-ctor-c: "Failed to construct 'X': Illegal constructor" for both
  // zero and non-zero args). The engine still needs to mint them internally,
  // so the wrapper admits exactly one caller — the holder of the hidden
  // __diting_ctorMint token — and throws for everything the page can write.
  const MINT = ['NodeList', 'ImageBitmap'];
  // (SVGPoint left this list at 刀91: Chrome throws Illegal constructor for
  // EVERY argument list, not just zero args — svg_root.js owns the always-
  // throwing face now that internal minting goes through __diting_svgPoint.)
  const ILLEGAL0 = ['Node', 'Element', 'CharacterData', 'CDATASection', 'DocumentType', 'ProcessingInstruction', 'XMLDocument', 'BaseAudioContext', 'CustomElementRegistry', 'DOMTokenList', 'ElementInternals', 'GPU', 'GPUAdapter', 'MediaQueryList', 'MessagePort', 'MimeType', 'MimeTypeArray', 'PerformanceEntry', 'PerformanceObserverEntryList', 'Plugin', 'PluginArray', 'Selection', 'ServiceWorkerContainer', 'SourceBuffer', 'XMLHttpRequestEventTarget'];
  const wrap = (name, check) => {
    const Orig = G[name];
    if (typeof Orig !== 'function' || !Orig.prototype) return;
    const W = function (...a) {
      if (!new.target) return Orig.apply(this, a);
      check(a);
      return Reflect.construct(Orig, a, new.target);
    };
    Object.defineProperty(W, 'name', { value: name, configurable: true });
    Object.defineProperty(W, 'length', { value: Orig.length, configurable: true });
    for (const k of Reflect.ownKeys(Orig)) {
      if (k === 'length' || k === 'name' || k === 'prototype') continue;
      try { Object.defineProperty(W, k, Object.getOwnPropertyDescriptor(Orig, k)); } catch (e) {}
    }
    Object.defineProperty(W, 'prototype', { value: Orig.prototype, writable: false, enumerable: false, configurable: false });
    Object.setPrototypeOf(W, Object.getPrototypeOf(Orig));
    try { Object.defineProperty(Orig.prototype, 'constructor', { value: W, writable: true, enumerable: false, configurable: true }); } catch (e) {}
    mark(W);
    // legacy aliases of the same class (WebKitMutationObserver === MutationObserver) follow the wrapper
    for (const k of Object.getOwnPropertyNames(G)) {
      if (k !== name && /^WebKit/.test(k) && G[k] === Orig) { const ad = Object.getOwnPropertyDescriptor(G, k); try { Object.defineProperty(G, k, { value: W, writable: ad.writable, enumerable: ad.enumerable, configurable: true }); } catch (e) {} }
    }
    const d = Object.getOwnPropertyDescriptor(G, name);
    Object.defineProperty(G, name, { value: W, writable: d ? d.writable : true, enumerable: d ? d.enumerable : false, configurable: true });
  };
  for (const name of Object.keys(ARGS)) {
    const n = ARGS[name];
    wrap(name, (a) => { if (a.length < n) throw new TypeError("Failed to construct '" + name + "': " + n + ' argument' + (n > 1 ? 's' : '') + ' required, but only ' + a.length + ' present.'); });
  }
  for (const name of ILLEGAL0) wrap(name, (a) => { if (a.length === 0) throw new TypeError("Failed to construct '" + name + "': Illegal constructor"); });
  const M = globalThis.__diting_ctorMint;
  for (const name of MINT) wrap(name, (a) => { if (a.length === 0 || a[0] !== M) throw new TypeError("Failed to construct '" + name + "': Illegal constructor"); });
})();
