(function () { // @@SCOPE
// god-file split (刀32, insertion preparation): the dynamic-insertion
// pipeline — __prepareInsertedSubtree walks a freshly attached subtree
// (custom-element upgrades ride the custom_elements.js walks it calls at
// runtime, script elements are prepared, stylesheets + styles linked),
// the <style>/<link rel=stylesheet> load-event faces, the initial-parse
// stylesheet pass, and the parser-path script load/error events (Rust
// scripts.rs calls globalThis.__fireParserScriptEvent — member install,
// moves with the family). domcore.js's insertion hooks call every
// __prepareInserted* face at RUNTIME — order-free, auto-published.
// __prepareInitialStylesheets is ALSO called by an injected bare
// identifier (scripts.rs) — analyzer-blind, pinned in exports-extra so
// its d() publish survives the move.
function __prepareInsertedSubtree(root) {
  if (!root || !root.isConnected) return;
  __upgradeCustomElementsIn(root, true);
  const scripts = [];
  const seen = new Set();
  if (root.nodeType === 1 && root.tagName === 'SCRIPT') {
    scripts.push(root);
    seen.add(root._nid);
  }
  const ids = _domParse("query_selector_all_scoped", root._nid, "script") || [];
  for (const nid of ids) {
    const script = _wrapEl(+nid);
    if (script && !seen.has(script._nid)) {
      scripts.push(script);
      seen.add(script._nid);
    }
  }
  for (const script of scripts) __prepareInsertedScript(script);
  __prepareInsertedStylesheetLinksIn(root);
  __prepareInsertedStylesIn(root);
  // Elements entering the document start their CSS animations.
  _scheduleAnimationCheck(root);
}

// --- dynamic <style> load events ---------------------------------------
// Per HTML, a <style> element fires `load` (and its <link rel=stylesheet>
// sibling) once its sheet **and every @import** has loaded. diting's CSS
// parser drops @import outright (diting_css parses it away — there is no
// second fetch to wait on), so a connected <style>'s sheet is ready the
// moment its text is in the tree: the spec's "queue a task" collapses to a
// 0ms task. Loader-side code that awaits `style.onload` (common around
// @import-based theming) would otherwise hang forever on a pending promise.
// Firing happens only on connection, mirroring the <link> path's per-element
// guard; a text mutation of an already-connected <style> does not re-fire.
function __prepareInsertedStylesIn(root) {
  if (!root || !root.isConnected) return;
  const styles = [];
  const seen = new Set();
  if (root.nodeType === 1 && (root.tagName || '').toUpperCase() === 'STYLE') {
    styles.push(root);
    seen.add(root._nid);
  }
  const ids = _domParse("query_selector_all_scoped", root._nid, "style") || [];
  for (const nid of ids) {
    if (seen.has(+nid)) continue;
    const el = _wrapEl(+nid);
    if (el) { styles.push(el); seen.add(+nid); }
  }
  for (const el of styles) {
    setTimeout(() => {
      // Disconnected before the task ran: no sheet is live, so no load.
      if (_nodeInDocument(el)) __fireLinkLoadEvent(el, 'load');
    }, 0);
  }
}

// --- dynamic <link rel=stylesheet> -------------------------------------
// Chrome fetches a stylesheet link when it becomes connected (or its href
// changes while connected) and fires `load`/`error` on the element when the
// fetch settles — with no timeout. Loader-side waits on that event are
// load-bearing: webpack's mini-css chunk runtime (d.f.miniCss) resolves its
// chunk promise from the link's load event, so a missing event leaves the
// promise pending forever and freezes every lazy route behind it (tmall
// pc-shop-webapp renders a styled blank page exactly this way).
function __fireLinkLoadEvent(link, type) {
  // dispatchEvent alone routes the on* property handler too; a manual
  // property call ahead of it double-fires (same disease as the
  // dynamic-script load path, #121).
  try { link.dispatchEvent(new Event(type)); } catch (e) {}
}
function __prepareInsertedStylesheetLinksIn(root) {
  if (!root || !root.isConnected) return;
  const links = [];
  const seen = new Set();
  if (root.nodeType === 1 && root.tagName === 'LINK') { links.push(root); seen.add(root._nid); }
  const ids = _domParse("query_selector_all_scoped", root._nid, "link") || [];
  for (const nid of ids) {
    if (seen.has(+nid)) continue;
    const link = _wrapEl(+nid);
    if (link) { links.push(link); seen.add(+nid); }
  }
  // Re-insertion restarts a sheet's load in a browser (usually from cache),
  // so the per-element URL guard resets on the insertion path; the cached
  // sheet object from the previous load dies with it.
  for (const link of links) { delete link._sheetLoadUrl; delete link._sheetObj; __prepareInsertedStylesheetLink(link); }
}
function __prepareInsertedStylesheetLink(link) {
  if (!link || link.nodeType !== 1 || (link.tagName || '').toUpperCase() !== 'LINK') return;
  const relTokens = (link.getAttribute('rel') || '').toLowerCase().split(/\s+/);
  const isSheet = relTokens.indexOf('stylesheet') >= 0;
  // <link rel=preload>/<link rel=modulepreload> never applies a sheet — it
  // warms the fetch and reports settle. Loader-side waits on those events
  // are load-bearing the same way (webfont/modulepreload orchestrators
  // inject their dependency off onload); before #259 both settle paths
  // fired nothing, so waiters hung forever on success AND failure.
  const isPreload = relTokens.indexOf('preload') >= 0 || relTokens.indexOf('modulepreload') >= 0;
  if (!isSheet && !isPreload) return;
  const raw = link.getAttribute('href');
  if (!raw) return;
  let abs;
  try { abs = new URL(raw, globalThis.location?.href || 'http://localhost/').href; } catch (e) { return; }
  // One fetch per (element, URL); a later href change re-enters with the new
  // URL and starts a fresh load, like the browser.
  if (link._sheetLoadUrl === abs) return;
  link._sheetLoadUrl = abs;
  const fireLoad = () => __fireLinkLoadEvent(link, 'load');
  if (_extSheetBodies()[abs] != null) {
    // Already fetched at navigation — shells often preload the same CSS the
    // chunk runtime then re-requests under a differently-shaped URL string.
    // The load still fires, as a task like the completed fetch it stands in
    // for.
    setTimeout(fireLoad, 0);
    return;
  }
  // data: URLs never hit the network — settle as a 0ms task like an
  // already-cached fetch (Chrome fires load for data: preloads too).
  if (abs.startsWith('data:')) { setTimeout(fireLoad, 0); return; }
  const pageOrigin = (function() { try { return new URL(globalThis.location?.href || 'about:blank').origin; } catch (e) { return ''; } })();
  (async () => {
    try {
      // Same settle-loop bracket as dynamic <script src>: this fetch rides
      // the op-level client, invisible to the page client's
      // active_requests() counter.
      _OPS.op_dyn_script_fetch_begin();
      let body;
      try {
        // Stylesheet requests are no-cors with same-origin credentials (no
        // crossorigin attribute) — unlike classic scripts, whose JSONP-era
        // include policy is deliberately looser.
        // #126: one real sample; dynamic sheets still block first paint.
        const _rtT0 = globalThis.performance ? performance.now() : 0;
        const rawResp = await _OPS.op_fetch_url(abs, "GET", "{}", "", pageOrigin, "no-cors", "same-origin", "\u0000about:client");
        const parsed = JSON.parse(rawResp);
        __recordFetchTiming(_rtT0, abs, 'link', parsed, { renderBlocking: isSheet });
        if (!(parsed.status >= 200 && parsed.status <= 299)) {
          const why = parsed.error || parsed.corsError;
          throw new Error('HTTP ' + (parsed.status || 0) + (why ? ': ' + why : ''));
        }
        body = parsed.body;
      } finally {
        _OPS.op_dyn_script_fetch_end();
      }
      if (body != null) {
        // Sheets apply through the ext-sheet cache; a warmed preload parks
        // its body there too, so the <link rel=stylesheet> that follows the
        // preload hits the cache and settles instantly — the browser's
        // preload-then-use shape.
        _domRaw("ext_sheet_put", abs, body);
        if (_extSheetsMap) _extSheetsMap[abs] = body;
      }
      fireLoad();
    } catch (e) {
      console.error('Dynamic stylesheet fetch error (' + abs + '):', e.message);
      __fireLinkLoadEvent(link, 'error');
    }
  })();
}

// --- initial-parse stylesheets ------------------------------------------
// The parser-built tree never passes through the insertion hooks above, so
// <style>/<link rel=stylesheet> present in the first HTML never fired load.
// Chrome queues those load events as tasks once each sheet applies; an inline
// script running during the parse still catches them by registering a
// listener before the task runs. page.rs invokes this before the script
// execution loop, which reproduces that ordering: the 0ms tasks queue now
// and fire between script executions as the loop pumps the event loop.
function __prepareInitialStylesheets() {
  const root = globalThis.document && globalThis.document.documentElement;
  if (!root) return;
  __prepareInsertedStylesIn(root);
  __prepareInsertedStylesheetLinksIn(root);
}

// --- parser-path script load events --------------------------------------
// Parser-discovered <script src> never passes through the dynamic insertion
// hooks, so its element never received a 'load' event after execution. Pages
// bootstrap off sibling script elements that way — doudian's SSR emits
// <script defer src=main.js></script><script>document.currentScript
// .previousSibling.addEventListener('load', ...)</script> and its whole
// publish sub-app waits on the defer that hook resolves. Chrome fires the
// event as a task once the script has executed; the 0ms timer keeps that
// "an inline script running later in the parse still catches it" ordering.
// scripts.rs calls this after each external parser script executes ('load')
// and for fetches that never landed ('error').
globalThis.__fireParserScriptEvent = function(nid, type) {
  const el = _wrapEl(+nid);
  if (!el) return;
  setTimeout(function() {
    try { el.dispatchEvent(new Event(type)); } catch (e) {}
  }, 0);
};

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__prepareInitialStylesheets', __prepareInitialStylesheets);
  d('__prepareInsertedStylesIn', __prepareInsertedStylesIn);
  d('__prepareInsertedStylesheetLink', __prepareInsertedStylesheetLink);
  d('__prepareInsertedStylesheetLinksIn', __prepareInsertedStylesheetLinksIn);
  d('__prepareInsertedSubtree', __prepareInsertedSubtree);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
