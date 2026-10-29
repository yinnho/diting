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
  register({
    // the timer-deferral window (#327) around a target's listener loop
    enter() { _mtDepth++; },
    exit() { _mtDepth--; },
    report(err) { globalThis.__diting_reportUncaughtError(err); },
    // next target on the event path; plain EventTargets have none
    parent(target) { return target instanceof NodeC ? (target.parentNode || null) : null; },
    // event handler IDL attribute (`el.onfoo = fn`) fires after the listeners
    handler(target, ev) {
      const h = target['on' + ev.type];
      return typeof h === 'function' ? h : undefined;
    },
  });
})();
