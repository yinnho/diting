(function () { // @@SCOPE
// god-file split (刀56, dynamic script preparation): __prepareInsertedScript
// in full — type/module/importmap triage, the fetch-driven execution paths
// (retry-on-network-failure-not-404, JSONP/mtop token dance, data: URLs,
// blob URLs), inline eval with __currentScriptNid bracketing, and the
// disconnected-script-stays-unstarted rule. Moved verbatim from bootstrap;
// consumers (insertion_prep.js tree walk, domcore.js script element
// creation) call at runtime through the published hidden global.
function __prepareInsertedScript(script) {
  const scriptType = (script.getAttribute('type') || '').trim().toLowerCase();
  const isModule = scriptType === 'module';
  const isImportMap = scriptType === 'importmap';
  if (isImportMap) {
    // Import maps consume their start slot even when they contribute
    // nothing (the error path still counts as "ran").
    // Upstream 34373c3: a dynamically inserted import map registers at its
    // insertion point, using the live document base URL. External import
    // maps are not supported (matching this engine's module pipeline).
    const src = script.getAttribute('src');
    let error = '';
    if (src) {
      error = 'External import maps are not supported';
    } else {
      const base = script.baseURI
        || globalThis.location?.href
        || 'about:blank';
      try {
        error = _OPS.op_add_import_map(script.textContent || '', base) || '';
      } catch (e) {
        error = e && e.message ? e.message : String(e);
      }
    }
    if (error) {
      console.error('Import map error:', error);
      queueMicrotask(() => {
        try { script.dispatchEvent(new Event('error')); } catch (_) {}
      });
    }
    return;
  }
  if (scriptType && !isModule && scriptType !== 'text/javascript' && scriptType !== 'application/javascript') {
    return;
  }
  if (isImportMap) {
    if (!_OPS.op_script_try_start(script._nid)) return;
  }
  const src = script.getAttribute('src');
  const code = src ? "" : script.textContent;
  if (!src && !code) {
    // An EMPTY script consumes no start slot: HTML's preparation runs when
    // the script becomes "ready" (src set or text present), not at
    // insertion. The engine used to consume `already-started` here, so
    // createElement → appendChild (empty) → el.src = url never loaded —
    // the most common dynamic-loading idiom (webpack/jsonp loaders,
    // 抖店 chunk injection) silently no-op'd. Leaving the slot lets the
    // src setter below re-enter preparation.
    return;
  }
  if (!_OPS.op_script_try_start(script._nid)) return;
  const prevNid = globalThis.__currentScriptNid;
  if (src) {
    const fullUrl = src.startsWith('http') || src.startsWith('data:')
      ? src
      : new URL(src, globalThis.location?.href || 'http://localhost/').href;
    const pageOrigin = (function() { try { return new URL(globalThis.location?.href || "about:blank").origin; } catch(e) { return ""; } })();
    (async () => {
      try {
        if (isModule) {
          await import(fullUrl);
        } else {
          // The HTML script-fetch algorithm treats an unsuccessful HTTP
          // response as a network error — its body must never become script
          // source (404/500 diagnostic HTML executing as code is both unlike
          // browsers and dangerous). data: URLs bypass the HTTP client
          // entirely and decode in JS (f61493f + 0c4740a/f841205).
          let body;
          if (fullUrl.startsWith('data:')) {
            body = _decodeDataScriptUrl(fullUrl);
          } else if (fullUrl.startsWith('blob:')) {
            // Classic scripts stay MIME-blind (their HTTP path never checked
            // a content-type either); the body lives in __blobObjs.
            const b = globalThis.__blobObjs && globalThis.__blobObjs[fullUrl];
            if (!b || !(b._bytes instanceof Uint8Array)) {
              throw new TypeError('Failed to fetch script: ' + fullUrl);
            }
            body = new TextDecoder().decode(b._bytes);
          } else {
            // Bracket the fetch so the settle loop keeps pumping past its
            // fast-path deadline while this script is still in flight.
            // credentials "include": classic <script src> requests carry and
            // store cookies cross-origin in real browsers (JSONP-era APIs
            // depend on it — taobao mtop's token refresh rotates _m_h5_tk via
            // Set-Cookie on a cross-origin JSONP GET; with "same-origin" the
            // Set-Cookie is dropped and the refresh loops on TOKEN_EMPTY
            // forever, leaving decorated shop pages empty).
            // Lazy-chunk resilience (10-27 复盘: 抖店 UserLimit
            // ChunkLoadError 53414 — a transient network miss on one chunk
            // killed the whole publish flow until a manual refresh). One
            // automatic retry for NETWORK-shaped failures only: 5xx and
            // status-0 fetch errors. 4xx means the resource genuinely
            // isn't there, and policy refusals (SSRF/pattern) must not be
            // re-dialled — those fail identically and instantly.
            const fetchOnce = async () => {
              _OPS.op_dyn_script_fetch_begin();
              try {
                // #126: one real sample per dynamic script load.
                const _rtT0 = globalThis.performance ? performance.now() : 0;
                const raw = await _OPS.op_fetch_url(fullUrl, "GET", "{}", "", pageOrigin, "no-cors", "include", "\u0000about:client");
                const parsed = JSON.parse(raw);
                __recordFetchTiming(_rtT0, fullUrl, 'script', parsed);
                if (!(parsed.status >= 200 && parsed.status <= 299)) {
                  // status 0 means the op refused before any wire traffic —
                  // the reason rides parsed.error (SSRF/pattern block) or
                  // parsed.corsError (post-response CORS gate). Dropping it
                  // printed a bare "HTTP 0" that read like a server-side
                  // failure (the taobao eg.js chase). The final URL rides
                  // parsed.url for the redirect-shaped refusals — a chain
                  // that ends somewhere other than the src is half the
                  // diagnosis.
                  const why = parsed.error || parsed.corsError;
                  let msg = 'HTTP ' + (parsed.status || 0) + (why ? ': ' + why : '');
                  const fin = parsed.final_url || parsed.url;
                  if (fin && fin !== fullUrl) msg += ' -> ' + fin;
                  const err = new Error(msg);
                  err.__retryable = parsed.status === 0 || (parsed.status >= 500 && parsed.status <= 599);
                  throw err;
                }
                return parsed.body;
              } finally {
                _OPS.op_dyn_script_fetch_end();
              }
            };
            try {
              body = await fetchOnce();
            } catch (e1) {
              if (!e1 || !e1.__retryable) throw e1;
              globalThis.__dynScriptRetries = (globalThis.__dynScriptRetries || 0) + 1;
              await new Promise(r => setTimeout(r, 75));
              body = await fetchOnce();
            }
          }
          if (body) {
            globalThis.__currentScriptNid = script._nid;
            try { (0, eval)(body); }
            catch(e) { globalThis.__diting_reportUncaughtError(e, fullUrl); }
            finally { globalThis.__currentScriptNid = prevNid || 0; }
          }
        }
        // dispatchEvent alone routes the on* property handler (element
        // dispatch resolves `on` + type generically) — calling it manually
        // first made onload fire twice. CKEditor's scriptLoader deletes its
        // pending-callback map on the first completion, so the second run
        // threw "Cannot read properties of undefined (reading 'length')"
        // and killed the source-import editor mount (#121).
        try { script.dispatchEvent(new Event('load')); } catch(e) {}
      } catch(e) {
        console.error('Dynamic script fetch error (' + fullUrl + '):', e.message);
        // dispatchEvent fires both the onerror property handler and
        // registered listeners (a listener-only consumer still sees failures).
        try { script.dispatchEvent(new Event('error')); } catch(ex) {}
      }
    })();
  } else {
    if (code) {
      if (isModule) {
        const dataUrl = 'data:text/javascript;base64,' + btoa(unescape(encodeURIComponent(code)));
        (async () => {
          try { await import(dataUrl); }
          catch(e) { globalThis.__diting_reportUncaughtError(e); }
        })();
      } else {
        globalThis.__currentScriptNid = script._nid;
        try { (0, eval)(code); }
        catch(e) { globalThis.__diting_reportUncaughtError(e); }
        finally { globalThis.__currentScriptNid = prevNid || 0; }
      }
    }
  }
}

// HTML's script preparation algorithm leaves a disconnected script unstarted.
// When an ancestor is later connected, insertion steps visit every script in
// that subtree in tree order.

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__prepareInsertedScript', __prepareInsertedScript);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
