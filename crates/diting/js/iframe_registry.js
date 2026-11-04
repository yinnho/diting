(function () { // @@SCOPE
// god-file split (刀36, iframe registration): the browsing-context
// registry — iframes register when INSERTED into the document (numeric
// window indexing follows tree order), srcdoc documents run their
// scripts as a document load (inline sync, external parser-blocking,
// load event after the last script — obscura #1265), and insertion
// starts src/srcdoc navigation. Consumers all RUNTIME: domcore.js's
// insertion hooks and iframe_realm.js (which owns __iframeBlankLoad,
// published) call the walkers through the publish table;
// f._loadIframeSrc is an instance member. _iframeRegistry is
// file-private; window.length mirrors it at runtime.
const _iframeRegistry = [];
// srcdoc documents run their scripts — a document load, not innerHTML
// (#268). Inline scripts run synchronously in tree order; external ones are
// fetched (resolving against the PARENT base — about:srcdoc has no path of
// its own) and run parser-blocking style, each delaying the ones after it.
// The element's load event fires only after the last script finishes
// (obscura #1265: finish document work before iframe load).
function _runSrcdocScripts(el) {
  const doc = el._iframeDoc, win = el._iframeWin;
  const finish = () => el.dispatchEvent(new Event('load'));
  if (!doc || !win) { Promise.resolve().then(finish); return; }
  const scripts = doc._root ? doc._root.querySelectorAll('script') : [];
  const evalInFrame = (code, label) => {
    try {
      // A srcdoc document is its own global in a real browser; in the
      // single-realm shim the frame's window/document stand in as the
      // script's bindings (parent/top stay the host page's, frameElement
      // is the iframe). module scripts run as classic — best effort.
      const fn = new Function('window', 'document', 'self', 'parent', 'top',
        'frameElement', 'globalThis', code);
      fn.call(win, win, doc, win, globalThis, globalThis, el, win);
    } catch (e) {
      globalThis.__diting_reportUncaughtError?.(e);
    }
  };
  let idx = 0;
  const step = () => {
    while (idx < scripts.length) {
      const s = scripts[idx++];
      const type = (s.getAttribute('type') || 'text/javascript').toLowerCase();
      if (type !== 'text/javascript' && type !== 'module' &&
          type !== 'application/javascript' && type !== 'application/ecmascript') continue;
      const srcAttr = s.getAttribute('src');
      if (!srcAttr) { evalInFrame(s.textContent || '', 'srcdoc-inline'); continue; }
      const abs = (function() { try { return new URL(srcAttr, _docBase()).href; } catch (e) { return null; } })();
      if (!abs) continue;
      fetch(abs, { credentials: 'include' })
        .then(r => r.ok ? r.text() : Promise.reject(new Error('http ' + r.status)))
        .then(code => evalInFrame(code, abs), () => {})
        .then(step);
      return; // wait for the external script before continuing
    }
    finish();
  };
  Promise.resolve().then(step); // async start: document work first, then scripts
}
function _registerIframe(iframeEl) {
  // Idempotent: attachment points (appendChild/innerHTML) and src loading can
  // both reach the same element.
  if (iframeEl._iframeRegistered) return;
  iframeEl._iframeRegistered = true;
  const idx = _iframeRegistry.length;
  _iframeRegistry.push(iframeEl);
  globalThis.length = _iframeRegistry.length;
  Object.defineProperty(globalThis, idx, {
    get() {
      // Eagerly create the browsing context: detector code reads `self[i]`
      // right after inserting fixture markup, before ever touching
      // contentWindow — a null there reads as a missing iframe.
      if (!iframeEl._iframeWin && typeof iframeEl.contentWindow !== 'undefined') {
        void iframeEl.contentWindow;
      }
      return iframeEl._iframeWin || null;
    },
    configurable: true,
    enumerable: false,
  });
}
// Browsers create an iframe's browsing context when it is INSERTED INTO the
// document (not on src load, not on contentWindow access). Detector code
// captures `e = self.length`, appends fixture markup containing an iframe,
// then reads `self[e]` to get that iframe's window. Register iframes found in
// an inserted subtree, in tree order, so numeric window indexing matches.
function _registerIframesIn(node) {
  if (!node || typeof node.querySelectorAll !== 'function') return;
  const frames = node.nodeType === 1 && node.tagName === 'IFRAME'
    ? [node, ...node.querySelectorAll('iframe')]
    : [...node.querySelectorAll('iframe')];
  for (const f of frames) {
    _registerIframe(f);
    // Insertion doesn't just create the browsing context — an iframe that
    // already carries a src attribute starts navigating too. The src
    // property setter only sees JS assignment; framework writers attach
    // the attribute before insertion, so the load has to start here
    // (#121's sucai-selector iframe). _loadIframeSrc's flag collapses the
    // repeat against the post-navigation scan and the write-path scan.
    // srcdoc iframes navigate on insertion the same way (and win over src).
    if (f.hasAttribute('srcdoc')) { f._loadIframeSrc('about:srcdoc'); continue; }
    const s = f.getAttribute('src');
    if (s && s !== 'about:blank') f._loadIframeSrc(s); else __iframeBlankLoad(f);
  }
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_registerIframe', _registerIframe);
  d('_registerIframesIn', _registerIframesIn);
  d('_runSrcdocScripts', _runSrcdocScripts);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
