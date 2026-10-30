// Own-ENUMERABLE-key probe (M4 knife 53). for-in + hasOwnProperty is the engine's native view (JSON.stringify, Object.assign and
// spread see the same set); Object.keys / getOwnPropertyNames are patched by the engine to hide internals and would mask leaks.
// Runs in Chrome AND the engine.
(function () {
  var R = {};
  function keys(x) { var ks = []; for (var k in x) { if (Object.prototype.hasOwnProperty.call(x, k)) ks.push(k); } return ks; }
  function k(n, f) { try { var x = f(); R[n] = (x !== null && (typeof x === 'object' || typeof x === 'function')) ? keys(x) : 'primitive'; } catch (e) { R[n] = 'THROW ' + e.name; } }
  var h = document.createElement('div'); h.id = 'enumhold'; document.body.appendChild(h);
  ['div', 'span', 'a', 'input', 'select', 'option', 'textarea', 'form', 'img', 'canvas', 'video', 'audio', 'table', 'tr', 'td', 'button', 'label', 'iframe', 'script', 'style', 'link', 'body', 'html', 'head', 'li', 'ul', 'p', 'h1', 'details', 'dialog', 'template', 'slot', 'object', 'embed', 'meter', 'progress', 'fieldset', 'output', 'map', 'area', 'svg', 'optgroup', 'legend', 'caption', 'col', 'base', 'meta', 'title', 'noscript', 'source', 'track', 'param', 'picture', 'menu', 'data', 'time', 'br', 'hr', 'pre', 'blockquote'].forEach(function (t) { k('el.' + t, function () { return document.createElement(t); }); });
  k('el.styled', function () { var d = document.createElement('div'); d.style.color = 'red'; return d; });
  k('el.attached', function () { var d = document.createElement('div'); h.appendChild(d); d.style.color = 'red'; d.classList.add('x'); d.dataset.a = '1'; d.addEventListener('click', function () {}); return d; });
  k('el.listened', function () { var d = document.createElement('input'); d.onclick = function () {}; d.addEventListener('input', function () {}); d.value = 'x'; return d; });
  k('svgEl', function () { return document.createElementNS('http://www.w3.org/2000/svg', 'rect'); });
  k('mathEl', function () { return document.createElementNS('http://www.w3.org/1998/Math/MathML', 'math'); });
  k('text', function () { return document.createTextNode('t'); }); k('comment', function () { return document.createComment('c'); }); k('frag', function () { return document.createDocumentFragment(); });
  k('doc', function () { return document; }); k('docType', function () { return document.doctype || document.implementation.createDocumentType('html', '', ''); });
  k('attr', function () { return document.createAttribute('a'); }); k('pi', function () { return document.createProcessingInstruction ? document.createProcessingInstruction('t', 'd') : null; });
  k('cdata', function () { return document.createCDATASection ? 'x' : null; });
  k('implementation', function () { return document.implementation; }); k('newdoc', function () { return document.implementation.createHTMLDocument('x'); });
  k('range', function () { return document.createRange(); }); k('staticRange', function () { return new StaticRange({ startContainer: document.body, startOffset: 0, endContainer: document.body, endOffset: 0 }); });
  k('selection', function () { return getSelection(); });
  k('treeWalker', function () { return document.createTreeWalker(document); }); k('nodeIter', function () { return document.createNodeIterator(document); });
  k('shadow', function () { return document.createElement('div').attachShadow({ mode: 'open' }); });
  k('nodeList', function () { return document.querySelectorAll('div'); }); k('htmlColl', function () { return document.getElementsByTagName('div'); });
  k('namedNodeMap', function () { return document.body.attributes; }); k('classList', function () { return document.body.classList; }); k('dataset', function () { return document.body.dataset; });
  k('relList', function () { return document.createElement('a').relList; }); k('style', function () { return document.body.style; }); k('computed', function () { return getComputedStyle(document.body); });
  k('sheets', function () { return document.styleSheets; }); k('sheet', function () { var s = document.createElement('style'); s.textContent = 'a{color:red}'; document.head.appendChild(s); return s.sheet; });
  k('cssRule', function () { var s = document.createElement('style'); s.textContent = 'a{color:red}'; document.head.appendChild(s); return s.sheet.cssRules[0]; });
  k('cssRules', function () { var s = document.createElement('style'); s.textContent = 'a{color:red}'; document.head.appendChild(s); return s.sheet.cssRules; });
  k('evt', function () { return new Event('x'); }); k('custom', function () { return new CustomEvent('x', { detail: 1 }); }); k('mouse', function () { return new MouseEvent('x'); });
  k('kb', function () { return new KeyboardEvent('x'); }); k('focus', function () { return new FocusEvent('x'); }); k('input', function () { return new InputEvent('x'); }); k('wheel', function () { return new WheelEvent('x'); });
  k('pointer', function () { return new PointerEvent('x'); }); k('touch', function () { return new TouchEvent('x'); }); k('msg', function () { return new MessageEvent('x', { data: 1 }); });
  k('err', function () { return new ErrorEvent('x'); }); k('pop', function () { return new PopStateEvent('x'); }); k('hash', function () { return new HashChangeEvent('x'); });
  k('closeEv', function () { return new CloseEvent('x'); }); k('progress', function () { return new ProgressEvent('x'); }); k('storageEv', function () { return new StorageEvent('x'); });
  k('anim', function () { return new AnimationEvent('x'); }); k('trans', function () { return new TransitionEvent('x'); }); k('clip', function () { return new ClipboardEvent('x'); }); k('drag', function () { return new DragEvent('x'); });
  k('xhr', function () { return new XMLHttpRequest(); }); k('xhrUpload', function () { return new XMLHttpRequest().upload; });
  k('blob', function () { return new Blob(['a']); }); k('file', function () { return new File(['a'], 'a'); }); k('fileList', function () { return document.createElement('input').files; });
  k('fr', function () { return new FileReader(); }); k('url', function () { return new URL('http://a/'); }); k('usp', function () { return new URLSearchParams('a=1'); });
  k('fd', function () { return new FormData(); }); k('hdr', function () { return new Headers(); }); k('req', function () { return new Request('http://a/'); }); k('res', function () { return new Response('a'); });
  k('ac', function () { return new AbortController(); }); k('as', function () { return new AbortController().signal; });
  k('mo', function () { return new MutationObserver(function () {}); }); k('io', function () { return new IntersectionObserver(function () {}); }); k('ro', function () { return new ResizeObserver(function () {}); });
  k('po', function () { return new PerformanceObserver(function () {}); }); k('moRec', function () { var r = null; var m = new MutationObserver(function () {}); m.observe(h, { childList: true }); h.appendChild(document.createElement('i')); r = m.takeRecords()[0]; return r; });
  k('nav', function () { return navigator; }); k('screen', function () { return screen; }); k('hist', function () { return history; }); k('perf', function () { return performance; }); k('loc', function () { return location; });
  k('timing', function () { return performance.timing; }); k('navTiming', function () { return performance.getEntriesByType('navigation')[0]; }); k('mark', function () { return performance.mark('m'); });
  k('ls', function () { return localStorage; }); k('ss', function () { return sessionStorage; }); k('cookie', function () { return typeof cookieStore !== 'undefined' ? cookieStore : null; });
  k('te', function () { return new TextEncoder(); }); k('td', function () { return new TextDecoder(); }); k('mc', function () { return new MessageChannel(); }); k('port', function () { return new MessageChannel().port1; });
  k('bc', function () { return new BroadcastChannel('x'); }); k('ws', function () { return new WebSocket('ws://127.0.0.1:1'); }); k('es', function () { return new EventSource('http://127.0.0.1:1/'); });
  k('dp', function () { return new DOMParser(); }); k('xs', function () { return new XMLSerializer(); }); k('xpe', function () { return new XPathEvaluator(); });
  k('dr', function () { return new DOMRect(); }); k('drro', function () { return new DOMRectReadOnly(); }); k('dm', function () { return new DOMMatrix(); }); k('pt', function () { return new DOMPoint(); }); k('quad', function () { return new DOMQuad(); });
  k('gbcr', function () { return document.body.getBoundingClientRect(); }); k('rects', function () { return document.body.getClientRects(); });
  k('cv', function () { return document.createElement('canvas').getContext('2d'); }); k('imgd', function () { return new ImageData(1, 1); }); k('path', function () { return new Path2D(); });
  k('grad', function () { return document.createElement('canvas').getContext('2d').createLinearGradient(0, 0, 1, 1); }); k('tm', function () { return document.createElement('canvas').getContext('2d').measureText('a'); });
  k('gl', function () { return document.createElement('canvas').getContext('webgl'); }); k('offc', function () { return typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(1, 1) : null; });
  k('rs', function () { return new ReadableStream(); }); k('ws2', function () { return new WritableStream(); }); k('ts', function () { return new TransformStream(); });
  k('rsReader', function () { return new ReadableStream().getReader(); }); k('rsCtl', function () { var c; new ReadableStream({ start: function (x) { c = x; } }); return c; });
  k('crypto', function () { return crypto; }); k('subtle', function () { return crypto.subtle; }); k('mq', function () { return matchMedia('(min-width:1px)'); });
  k('caches', function () { return typeof caches !== 'undefined' ? caches : null; }); k('idb', function () { return indexedDB; }); k('ce', function () { return customElements; });
  k('anim2', function () { return new Animation(); }); k('kf', function () { return new KeyframeEffect(null, [], 1); }); k('tl', function () { return document.timeline; });
  k('ac2', function () { return new AudioContext(); }); k('ctxDest', function () { return new AudioContext().destination; }); k('gain', function () { return new AudioContext().createGain(); });
  k('rtc', function () { return new RTCPeerConnection(); }); k('speech', function () { return speechSynthesis; }); k('utter', function () { return new SpeechSynthesisUtterance('x'); });
  k('geo', function () { return navigator.geolocation; }); k('perm', function () { return navigator.permissions; }); k('sw', function () { return navigator.serviceWorker; });
  k('conn', function () { return navigator.connection; }); k('plugins', function () { return navigator.plugins; }); k('mimes', function () { return navigator.mimeTypes; }); k('clip2', function () { return navigator.clipboard; });
  k('ua', function () { return navigator.userAgentData; }); k('storage', function () { return navigator.storage; }); k('mediaDev', function () { return navigator.mediaDevices; }); k('mediaSession', function () { return navigator.mediaSession; });
  k('vv', function () { return window.visualViewport; }); k('console', function () { return console; }); k('trusted', function () { return window.trustedTypes; }); k('scheduler', function () { return window.scheduler; });
  k('fonts', function () { return document.fonts; }); k('face', function () { return new FontFace('a', 'local(x)'); }); k('editctx', function () { return new EditContext(); }); k('tmplContent', function () { return document.createElement('template').content; });
  k('opt', function () { return new Option('a', 'b'); }); k('img2', function () { return new Image(); }); k('aud', function () { return new Audio(); });
  k('selfWin', function () { return window; }); k('globalThis', function () { return globalThis; });
  var out = {}; Object.keys(R).sort().forEach(function (n) { out[n] = Array.isArray(R[n]) ? R[n].slice().sort() : R[n]; });
  return JSON.stringify(out);
})()
