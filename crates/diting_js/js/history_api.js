(function () { // @@SCOPE
// god-file split (刀25, History API family): the functional History IIFE —
// the in-memory session-history stack, push/replace/go/back/forward on it,
// popstate/hashchange dispatch, and the __diting_history state accessor the
// CDP/tests read. Load position #21 (before plugins.js): navigation.js
// captures history.pushState at LOAD time and interface_sweep.js reads the
// singleton at LOAD time — both sit later in the order, so the install must
// precede them. __virtualUrl is bootstrap's (Location clears/reads it via
// member form; only this family WRITES it through the stack ops).
// Functional History API. The earlier stub returned constant state and was a
// no-op on push/replace, so any SPA that tried to update its URL (Next.js
// client router, React Router, vue-router, hash-based routers) silently
// failed: location.href stayed pinned to the initial page, useLocation hooks
// never updated, and popstate-driven UI froze.
//
// Internally we keep a tiny in-memory stack of {state, url} entries. push/
// replace mutate the stack and set globalThis.__virtualUrl so location.href
// reads the new URL. Real Chrome doesn't fire popstate on push/replace,
// only on user-driven back/forward — we match that exactly.
(() => {
  // Seeded from the engine's real session history (obscura#1105): the JS
  // runtime is rebuilt on every cross-document navigation, so a fresh
  // single-entry stack would report history.length === 1 and swallow
  // back() entirely. The seeding is LAZY — on the first history touch —
  // because this bootstrap runs from the V8 snapshot (before any ops or
  // per-document state exist) and even __diting_init fires inside the
  // runtime constructor, before the page setters land. By the time page
  // script reads history, the seed values are readable through _domParse.
  // Seeded entries other than this document's own belong to another
  // document (cross:true) — go() to one queues a real session-history jump
  // through the engine instead of a virtual move. Entries pushState
  // appends later are this document's own (no cross flag).
  const stack = [{state: null, url: undefined}]; // doc entry; url=undefined means "use document URL"
  let idx = 0;
  let seeded = false;
  const ensureSeed = () => {
    if (seeded) return;
    seeded = true;
    let p = 0, t = 1;
    try {
      p = Math.max(0, parseInt(_domParse("history_pos") || "0", 10) || 0);
      t = Math.max(p + 1, parseInt(_domParse("history_total") || "1", 10) || 1);
    } catch {}
    const pre = stack.splice(0, stack.length);
    for (let i = 0; i < t; i++) {
      stack.push(i === p ? (pre[0] || {state: null, url: undefined}) : {state: null, url: undefined, cross: true});
    }
    // Same-document entries the page pushed before its first history read
    // stack after the doc entry (pre was [doc, ps...] before seeding).
    for (let i = 1; i < pre.length; i++) stack.push(pre[i]);
    idx = p + pre.length - 1;
  };
  // Shared history push/replace state steps (HTML spec): resolve the URL
  // against the document, then throw SecurityError if it fails to parse or
  // parses to a different origin. The old raw-string fallback let a page pin
  // a cross-origin or fake URL into location.href — which the host then
  // adopts as the page's own record of itself (#71).
  const resolveStateUrl = (url) => {
    // A missing url (pushState/replaceState called with < 3 args) keeps the
    // current document URL per the HTML spec — capture it so the entry does
    // not reset location back to the original document URL (upstream #496).
    if (url === null || url === undefined) return __currentUrl();
    const cur = __currentUrl();
    let curOrigin = "null";
    try { curOrigin = new URL(cur).origin; } catch {}
    let resolved;
    try { resolved = new URL(String(url), cur); }
    catch (e) {
      throw new DOMException(
        "A history state object with URL '" + String(url) + "' cannot be created in a document with origin '" + curOrigin + "'",
        "SecurityError");
    }
    if (resolved.origin !== curOrigin) {
      throw new DOMException(
        "A history state object with URL '" + resolved.href + "' cannot be created in a document with origin '" + curOrigin + "'",
        "SecurityError");
    }
    return resolved.href;
  };
  const applyVirtual = () => {
    const entry = stack[idx];
    globalThis.__virtualUrl = entry.url ?? null;
  };
  const fireHashChangeIfNeeded = (prevUrl) => {
    try {
      const next = __currentUrl();
      if (!prevUrl || !next) return;
      const a = new URL(prevUrl), b = new URL(next);
      if (a.origin === b.origin && a.pathname === b.pathname && a.search === b.search && a.hash !== b.hash) {
        const ev = new Event('hashchange');
        ev.oldURL = prevUrl; ev.newURL = next;
        try { globalThis.dispatchEvent(ev); } catch {}
      }
    } catch {}
  };
  globalThis.history = {
    get length() { ensureSeed(); return stack.length; },
    get state() { ensureSeed(); return stack[idx].state; },
    scrollRestoration: "auto",
    pushState(state, _title, url) {
      ensureSeed();
      const prevUrl = __currentUrl();
      const resolved = resolveStateUrl(url);
      // Truncate forward entries (real Chrome drops the forward stack on a
      // new push) then append + advance.
      stack.length = idx + 1;
      stack.push({state: state ?? null, url: resolved});
      idx = stack.length - 1;
      applyVirtual();
      fireHashChangeIfNeeded(prevUrl);
    },
    replaceState(state, _title, url) {
      ensureSeed();
      const prevUrl = __currentUrl();
      const resolved = resolveStateUrl(url);
      stack[idx] = {state: state ?? null, url: resolved};
      applyVirtual();
      fireHashChangeIfNeeded(prevUrl);
    },
    go(n) {
      ensureSeed();
      n = (n | 0);
      if (n === 0) return; // real spec: go(0) reloads. We don't reload SPAs.
      const next = Math.max(0, Math.min(stack.length - 1, idx + n));
      if (next === idx) return;
      if (stack[next].cross) {
        // Cross-document entry (obscura#1105): queue a real session-history
        // jump — the engine moves its cursor and refetches the entry's
        // document. Seeded indices map 1:1 onto the engine's entry indices,
        // so the absolute index goes straight through. popstate fires on the
        // landing document, not this one.
        _OPS.op_history_jump_to(next);
        return;
      }
      const prevUrl = __currentUrl();
      idx = next;
      applyVirtual();
      // Real Chrome fires popstate on back/forward with the destination entry's state.
      try {
        const ev = new PopStateEvent('popstate', {state: stack[idx].state});
        globalThis.dispatchEvent(ev);
      } catch {}
      fireHashChangeIfNeeded(prevUrl);
    },
    back() { this.go(-1); },
    forward() { this.go(1); },
  };
  Object.defineProperty(globalThis, '__diting_history', { value: () => { ensureSeed(); return { stack, index: idx }; } });
})();

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
