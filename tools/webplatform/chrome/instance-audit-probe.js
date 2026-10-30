// Instance own-key audit: what a script sees on Object.getOwnPropertyNames(instance) for ~120 common objects.
// Chrome instances of WebIDL interfaces carry (almost) no own properties; the engine's real classes often keep members as
// own data properties. Diffed by tools/webplatform/instance_audit.py.
(function () {
  var R = {};
  var el = document.createElement('div'); document.body.appendChild(el);
  var cv = document.createElement('canvas');
  var F = {
    Event: function () { return new Event('x'); }, CustomEvent: function () { return new CustomEvent('x', { detail: 1 }); },
    MouseEvent: function () { return new MouseEvent('click'); }, KeyboardEvent: function () { return new KeyboardEvent('keydown'); },
    Touch: function () { return new Touch({ identifier: 1, target: el }); }, TouchEvent: function () { return new TouchEvent('touchstart'); },
    ErrorEvent: function () { return new ErrorEvent('error'); }, MessageEvent: function () { return new MessageEvent('message'); },
    Blob: function () { return new Blob(['a']); }, File: function () { return new File(['a'], 'n'); }, FileReader: function () { return new FileReader(); },
    FormData: function () { return new FormData(); }, URL: function () { return new URL('http://a.test/'); }, URLSearchParams: function () { return new URLSearchParams('a=1'); },
    Headers: function () { return new Headers(); }, Request: function () { return new Request('http://a.test/'); }, Response: function () { return new Response(''); },
    AbortController: function () { return new AbortController(); }, AbortSignal: function () { return new AbortController().signal; },
    XMLHttpRequest: function () { return new XMLHttpRequest(); }, WebSocket: function () { try { return new WebSocket('ws://127.0.0.1:1/'); } catch (e) { return null; } },
    MessageChannel: function () { return new MessageChannel(); }, MessagePort: function () { return new MessageChannel().port1; }, BroadcastChannel: function () { return new BroadcastChannel('x'); },
    MutationObserver: function () { return new MutationObserver(function () {}); }, ResizeObserver: function () { return new ResizeObserver(function () {}); },
    IntersectionObserver: function () { return new IntersectionObserver(function () {}); }, PerformanceObserver: function () { return new PerformanceObserver(function () {}); },
    TextEncoder: function () { return new TextEncoder(); }, TextDecoder: function () { return new TextDecoder(); },
    DOMParser: function () { return new DOMParser(); }, XMLSerializer: function () { return new XMLSerializer(); },
    Range: function () { return document.createRange(); }, Selection: function () { return getSelection(); },
    DOMRect: function () { return new DOMRect(1, 2, 3, 4); }, DOMPoint: function () { return new DOMPoint(1, 2); }, DOMMatrix: function () { return new DOMMatrix(); },
    ImageData: function () { return new ImageData(1, 1); }, Path2D: function () { return new Path2D(); },
    CanvasRenderingContext2D: function () { return cv.getContext('2d'); }, TextMetrics: function () { return cv.getContext('2d').measureText('a'); },
    CanvasGradient: function () { return cv.getContext('2d').createLinearGradient(0, 0, 1, 1); },
    Image: function () { return new Image(); }, Audio: function () { return new Audio(); },
    HTMLDivElement: function () { return el; }, HTMLInputElement: function () { return document.createElement('input'); },
    HTMLAnchorElement: function () { return document.createElement('a'); }, HTMLFormElement: function () { return document.createElement('form'); },
    HTMLImageElement: function () { return document.createElement('img'); }, HTMLVideoElement: function () { return document.createElement('video'); },
    HTMLCanvasElement: function () { return cv; }, HTMLIFrameElement: function () { return document.createElement('iframe'); },
    HTMLScriptElement: function () { return document.createElement('script'); }, HTMLSelectElement: function () { return document.createElement('select'); },
    HTMLOptionElement: function () { return new Option('a', 'b'); }, HTMLTextAreaElement: function () { return document.createElement('textarea'); },
    SVGSVGElement: function () { return document.createElementNS('http://www.w3.org/2000/svg', 'svg'); },
    Text: function () { return document.createTextNode('a'); }, Comment: function () { return document.createComment('a'); },
    Attr: function () { el.setAttribute('q', '1'); return el.getAttributeNode('q'); },
    NamedNodeMap: function () { return el.attributes; }, NodeList: function () { return el.childNodes; }, HTMLCollection: function () { return document.getElementsByTagName('div'); },
    DOMTokenList: function () { return el.classList; }, CSSStyleDeclaration: function () { return el.style; }, DOMStringMap: function () { return el.dataset; },
    CSSStyleSheet: function () { var s = document.createElement('style'); document.head.appendChild(s); return s.sheet; },
    Document: function () { return document; }, DocumentFragment: function () { return document.createDocumentFragment(); },
    Window: function () { return window; }, Location: function () { return location; }, History: function () { return history; },
    Navigator: function () { return navigator; }, Screen: function () { return screen; }, Performance: function () { return performance; },
    Storage: function () { return localStorage; }, Crypto: function () { return crypto; }, SubtleCrypto: function () { return crypto.subtle; },
    Notification: function () { return new Notification('x'); }, PerformanceEntry: function () { performance.mark('m'); return performance.getEntriesByName('m')[0]; },
    PerformanceNavigationTiming: function () { return performance.getEntriesByType('navigation')[0] || null; },
    Worker: function () { return null; }, ReadableStream: function () { return new ReadableStream(); }, WritableStream: function () { return new WritableStream(); },
    TransformStream: function () { return new TransformStream(); }, CompressionStream: function () { return new CompressionStream('gzip'); },
    MediaStream: function () { return new MediaStream(); }, RTCPeerConnection: function () { return new RTCPeerConnection(); },
    SpeechSynthesisUtterance: function () { return new SpeechSynthesisUtterance('a'); }, AudioContext: function () { return new AudioContext(); },
    EventTarget: function () { return new EventTarget(); }, Map: function () { return new Map(); },
    CSSRule: function () { var s = document.createElement('style'); s.textContent = 'a{color:red}'; document.head.appendChild(s); return s.sheet.cssRules[0]; },
    MediaQueryList: function () { return matchMedia('(min-width:1px)'); }, Geolocation: function () { return navigator.geolocation; },
    Clipboard: function () { return navigator.clipboard; }, Permissions: function () { return navigator.permissions; }, ServiceWorkerContainer: function () { return navigator.serviceWorker; },
    Plugin: function () { return navigator.plugins[0] || null; }, MimeTypeArray: function () { return navigator.mimeTypes; }, PluginArray: function () { return navigator.plugins; },
    IDBFactory: function () { return indexedDB; }, CustomElementRegistry: function () { return customElements; }, TreeWalker: function () { return document.createTreeWalker(document); },
    NodeIterator: function () { return document.createNodeIterator(document); }, XPathResult: function () { return document.evaluate('1', document, null, 1, null); },
    DOMException: function () { return new DOMException('m', 'AbortError'); }, Error: function () { return new Error('m'); },
    PointerEvent: function () { return new PointerEvent('pointerdown'); }, WheelEvent: function () { return new WheelEvent('wheel'); },
    InputEvent: function () { return new InputEvent('input'); }, FocusEvent: function () { return new FocusEvent('focus'); },
    SubmitEvent: function () { return new SubmitEvent('submit'); }, StorageEvent: function () { return new StorageEvent('storage'); },
    PromiseRejectionEvent: function () { return new PromiseRejectionEvent('x', { promise: Promise.resolve(), reason: 1 }); },
    ProgressEvent: function () { return new ProgressEvent('progress'); }, CloseEvent: function () { return new CloseEvent('close'); },
    HashChangeEvent: function () { return new HashChangeEvent('hashchange'); }, PopStateEvent: function () { return new PopStateEvent('popstate'); },
    AnimationEvent: function () { return new AnimationEvent('animationend'); }, TransitionEvent: function () { return new TransitionEvent('transitionend'); },
    ClipboardEvent: function () { return new ClipboardEvent('copy'); }, DragEvent: function () { return new DragEvent('drag'); }, DataTransfer: function () { return new DataTransfer(); },
    Animation: function () { return el.animate([{ opacity: 0 }, { opacity: 1 }], 100000); }, KeyframeEffect: function () { return new KeyframeEffect(el, [], 1); },
    Intl: function () { return new Intl.DateTimeFormat(); }
  };
  Object.keys(F).forEach(function (n) {
    var o; try { o = F[n](); } catch (e) { R[n] = 'ERR'; return; }
    if (o === null || o === undefined) { R[n] = 'NULL'; return; }
    try { R[n] = { names: Object.getOwnPropertyNames(o).filter(function (k) { return k !== 'length' || typeof o !== 'function'; }), syms: Object.getOwnPropertySymbols(o).map(String), enumKeys: Object.keys(o) }; } catch (e) { R[n] = 'ERR2'; }
  });
  try { cv.remove(); el.remove(); } catch (e) {}
  return JSON.stringify(R);
})()
