// Host hooks for the native EventTarget dispatch core (crates/diting_runtime/
// src/native/event_target.rs). The core owns the algorithm (DOM sec 2.9);
// everything environment-specific reaches it through this one object,
// registered once through the one-shot native `__diting_event_hooks_set`
// (deleted right after use so no page ever sees it).
(() => {
  const register = globalThis.__diting_event_hooks_set;
  delete globalThis.__diting_event_hooks_set;
  // Captured now: a page overwriting window.Node/window.document cannot
  // redirect the path walk. (The Event brand check is native - the state
  // private slot - so the hooks no longer carry an isEvent leg.)
  const NodeC = globalThis.Node;
  const DocC = globalThis.document;
  // (M4 刀56) JS-side liveness mirror of the native listener registry:
  // registered/unregistered keep it exact (the native core dedups and
  // funnels every removal — explicit, once-drain, abort-cleanup — through
  // one spot). Consumers that must know a target still has listeners
  // without being able to see the registry: MessagePort's orphan-drop
  // heuristic (#237: a handler-less port's queue is dropped, a port with
  // a live listener must never be) and MediaQueryList registry pruning.
  const liveTypes = new WeakMap();
  function noteAdd(t, ty) {
    let s = liveTypes.get(t);
    if (!s) { s = new Set(); liveTypes.set(t, s); }
    s.add(String(ty));
  }
  function noteDrop(t, ty) {
    const s = liveTypes.get(t);
    if (s) s.delete(String(ty));
  }
  Object.defineProperty(globalThis, '__diting_etHasListeners', {
    value(t, ty) {
      const s = liveTypes.get(t);
      return !!s && s.has(String(ty));
    },
    writable: true, enumerable: false, configurable: true,
  });
  // A 'message' listener on a MessagePort starts it, exactly like the
  // class's own addEventListener used to (and onmessage still does).
  const PortC = globalThis.MessagePort;
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
    report(err) {
      // (刀68) IndexedDB's upgradeneeded dispatch records a throwing handler.
      const tap = globalThis.__diting_etErrTap;
      if (tap && tap.active > 0 && tap.err === null) tap.err = err === undefined || err === null ? new Error('handler threw') : err;
      globalThis.__diting_reportUncaughtError(err);
    },
    // next target on the event path; plain EventTargets have none. The
    // document → window edge is explicit (document.parentNode is null, but
    // Chrome's propagation path continues into the Window — bubbled element
    // events reach window listeners); it terminates there, the global is
    // not a Node.
    parent(target) {
      // (刀68) IndexedDB's request -> transaction -> database bubbling hops
      // (hop table owned by indexeddb.js; plain EventTargets, not Nodes).
      const hop = globalThis.__diting_etParents;
      if (hop) { const p = hop.get(target); if (p) return p; }
      if (!(target instanceof NodeC)) return null;
      if (target.parentNode) return target.parentNode;
      // The document is the only null-parent Node that still propagates.
      // The captured DocC is the SNAPSHOT realm's document — a fresh realm
      // mints a fresh instance, so identity alone never matched at runtime
      // (composedPath truth: Chrome ends ["D","W"], engine ended at "D").
      // Accept the live binding as well; detached elements (also null-parent)
      // stay out, and an iframe's contentDocument is not this realm's
      // document, so it does not jump to the main window.
      if (target === DocC || target === globalThis.document) return globalThis;
      // (M3 B2c) frame content: the frame's root hangs off nothing, but its
      // owner is the frame document, and the frame document's next hop is
      // the frame window — the frame analog of the document → window edge,
      // so events dispatched inside a frame reach the frame's document and
      // window listeners (and stop there: the frame window is not a Node,
      // and a frame never leaks into the host document's path).
      if (target._isIframeDocRoot) return target._ownerDoc || null;
      if (target.nodeType === 9 && target._iframeEl !== undefined) {
        const dv = target.defaultView;
        return dv && dv !== globalThis ? dv : null;
      }
      return null;
    },
    // Event handler IDL attribute (`el.onfoo = fn` / window.onfoo). Fires
    // BEFORE the listeners (the core calls it first in the non-capture
    // leg): the common page pattern assigns on* in markup or early script
    // and lets libraries addEventListener later, so handler-first matches
    // Chrome's registration order for that pattern. Known divergence: a
    // handler assigned AFTER an addEventListener still fires first. A
    // `false` return cancels a cancelable event (the core does that —
    // inline `onclick="...; return false"` contract).
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
      if (typeof h === 'function' && isHandlerSlot(target, key)) return h;
      // Nodes: the inline `onclick="..."` content attribute is an event
      // handler too — same precedence as Chrome (the IDL property wins,
      // the content attribute backs it up when the property is unset).
      if (target instanceof NodeC && typeof target._resolveInlineHandler === 'function') {
        const inline = target._resolveInlineHandler(key);
        if (typeof inline === 'function') return inline;
      }
      return undefined;
    },
    // Once per dispatch, right after the target is stamped: mouse offset
    // resolution against the origin's paint transform (the #663 leg of the
    // retired Element dispatcher — doc-space hit point inverse-mapped into
    // the element's local space).
    origin(target, ev) {
      if (!(target instanceof NodeC)) return;
      if (typeof ev.clientX !== 'number' || typeof ev.clientY !== 'number') return;
      if (target._nid === undefined) return;
      try {
        const sx = (globalThis.scrollX || globalThis.pageXOffset || 0);
        const sy = (globalThis.scrollY || globalThis.pageYOffset || 0);
        const raw = _domRaw('event_offset', String(target._nid | 0), (ev.clientX + sx) + ',' + (ev.clientY + sy));
        const pair = raw && raw !== 'null' ? JSON.parse(raw) : null;
        if (Array.isArray(pair) && pair.length === 2 && isFinite(pair[0]) && isFinite(pair[1])) {
          // offsetX/Y are read-only getters on the platform MouseEvent: the write goes
          // through the module's hidden hook (platform/uievent.impl.js)
          if (typeof __diting_uiOffset === 'function') __diting_uiOffset(ev, pair[0], pair[1]);
          else { ev.offsetX = pair[0]; ev.offsetY = pair[1]; }
        }
      } catch (e) { /* keep constructor defaults */ }
    },
    // Click activation, part 1: the pre-flip (checkbox/radio snap before
    // listeners so a prevented click can flip it back). undefined = no arm.
    activationPre(target, type) {
      if (type !== 'click' || !(target instanceof NodeC)) return undefined;
      if (target._ditingClickInProgress) return undefined;
      return _preClickFlip(target);
    },
    // Click activation, part 2 (dispatch epilogue): commit or cancel the
    // flip, then the post-click activation (label forwarding, form
    // submission). A boolean return overrides dispatchEvent's result —
    // the checked-change arm ends the dispatch without activation.
    activationPost(target, ev, flip) {
      if (flip) {
        if (ev.defaultPrevented) { _cancelPreClickFlip(flip); return undefined; }
        _commitPreClickFlip(flip);
        if (flip.el.checked !== flip.oldChecked) return !ev.defaultPrevented;
        _postClickActivation(target);
        return undefined;
      }
      if (ev.type === 'click' && !ev.defaultPrevented && !target._ditingClickInProgress && target instanceof NodeC) {
        _postClickActivation(target);
      }
      return undefined;
    },
    // A listener landed: animation* registrations are the practical signal
    // a page cares about markup-declared animations (no mutation hook sees
    // those), so scan once for candidates. 刀56: also mirrored into the
    // liveness map, and a 'message' listener starts a MessagePort.
    registered(target, type) {
      noteAdd(target, type);
      if (PortC && target instanceof PortC && type === 'message' && typeof target.start === 'function') {
        try { target.start(); } catch (e) {}
      }
      if (typeof type === 'string' && type.indexOf('animation') === 0
          && target instanceof NodeC && _nodeInDocument(target)) {
        _scheduleAnimationCheck(document.body || target);
      }
    },
    unregistered(target, type) { noteDrop(target, type); },
  });
  // (M3 B3/B4) Node.prototype must NOT own the EventTarget trio — Chrome
  // keeps those on EventTarget.prototype and every Node inherits them down
  // the chain. Some bake-time surface pass fill-if-missing installs inert
  // native-marked length-0 stubs there (fill-if-missing: verified by trap —
  // a pre-existing accessor stops the fill; identity of the installer
  // itself never pinned, see SURFACE.md). Harmless while Element/Document
  // carried real trilogies above it; the moment B3/B4 removed those, the
  // stubs became the resolution target and every element/document listener
  // went dead. This file is the LAST snapshot script, so every earlier
  // fill has already landed — delete all three (plus the legacy mtDepth
  // dispatch wrapper bootstrap used to wrap here) and let resolution fall
  // through to the native core.
  for (const m of ['addEventListener', 'removeEventListener', 'dispatchEvent']) {
    const d = Object.getOwnPropertyDescriptor(NodeC.prototype, m);
    if (d && d.configurable) { try { delete NodeC.prototype[m]; } catch (e) {} }
  }
})();
