// Host hooks for the native EventTarget dispatch core (crates/diting_runtime/
// src/native/event_target.rs). The core owns the algorithm (DOM sec 2.9);
// everything environment-specific reaches it through this one object,
// registered once through the one-shot native `__diting_event_hooks_set`
// (deleted right after use so no page ever sees it).
(() => {
  const register = globalThis.__diting_event_hooks_set;
  delete globalThis.__diting_event_hooks_set;
  // Captured now: a page overwriting window.Node cannot redirect it. (The
  // Event brand check is native - the state private slot - so the hooks no
  // longer carry an isEvent leg.)
  const NodeC = globalThis.Node;
  // Is `key` on `target` a real event-handler IDL attribute? Those install
  // as accessor properties (own on the global, prototype-level on Element/
  // Document); a page-assigned plain data property (`et.onfoo = f` on a bare
  // EventTarget, `window.onzzzprobe = f`, or shadowing an accessor with
  // defineProperty) is NOT a handler — Chrome never fires it. The first
  // descriptor found on the chain decides.
  function isHandlerSlot(t, key) {
    let o = t;
    for (let i = 0; i < 12 && o; i++) {
      const d = Object.getOwnPropertyDescriptor(o, key);
      if (d) return !('value' in d) && typeof d.get === 'function';
      o = Object.getPrototypeOf(o);
    }
    return false;
  }
  register({
    // the timer-deferral window (#327) around a target's listener loop
    enter() { _mtDepth++; },
    exit() { _mtDepth--; },
    report(err) { globalThis.__diting_reportUncaughtError(err); },
    // next target on the event path; plain EventTargets have none
    parent(target) { return target instanceof NodeC ? (target.parentNode || null) : null; },
    // Event handler IDL attribute (`el.onfoo = fn` / window.onfoo). Fires
    // BEFORE the listeners (the core calls it first in the non-capture
    // leg): the common page pattern assigns on* in markup or early script
    // and lets libraries addEventListener later, so handler-first matches
    // Chrome's registration order for that pattern. Known divergence: a
    // handler assigned AFTER an addEventListener still fires first.
    handler(target, ev) {
      const type = ev.type;
      const key = 'on' + type;
      if (target === globalThis) {
        // (#37) 'error' is pipeline-only in Chrome: onerror runs from the
        // uncaught-error pipeline (__diting_reportUncaught), never from a
        // synthetic dispatch — and the pipeline already called it, so
        // firing here too would double-report.
        if (type === 'error') return undefined;
        const store = globalThis.__windowOnHandlers;
        const has = store ? Object.prototype.hasOwnProperty.call(store, key) : false;
        // (#37) fall back to the `<body onX="...">` content attribute only
        // when nothing was ever assigned through JS — an explicit null
        // (non-callable assignment) overwrites the attribute handler.
        const h = has ? store[key]
          : (_BODY_WINDOW_EVENT_SET.has(type) ? _bodyWindowInline(key) : null);
        return typeof h === 'function' ? h : undefined;
      }
      const h = target[key];
      return typeof h === 'function' && isHandlerSlot(target, key) ? h : undefined;
    },
  });
})();
