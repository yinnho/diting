(function () { // @@SCOPE
// god-file split (刀65, event subclass family): the _evx WeakMap state
// scheme (Chrome shape: an event instance's only own property is
// isTrusted; subclass state rides _evx with prototype accessor pairs,
// 刀85 readonly face for dict members), the deno_core unhandledrejection/
// rejectionhandled routing IIFE (registered while Deno is still on the
// global; guarded reads), and the subclass ladder: UIEvent -> Mouse/
// Keyboard/Focus/Input/Composition/Wheel, the plain Event brood
// (Error/Animation/Transition/PopState/Hash/Message/Close/Progress/
// Clipboard/Submit/Toggle/ContentVisibilityAutoStateChange/Promise
// Rejection — note the second, validating PromiseRejectionEvent install
// overwrites the routing block's basic one; both kept verbatim). Event and
// CustomEvent are host natives (event.rs). Classes extend host natives at
// load — any slot works; consumers (pointer_touch extends MouseEvent,
// platform_bundle getters, engine dispatch) resolve at runtime.
const _markNative = globalThis.__diting_markNative;
// Event subclasses still live here, but their instance state (view/
// detail/clientX/...) moved OFF the instance into the `_evx` WeakMap with
// prototype accessor pairs - Chrome's shape: the only own property an
// event instance ever has is isTrusted (sub.ownkeys truth). Engine legs
// that need to write subclass state after construction (offset resolution
// in the element dispatcher) go through the writable accessors.
const _evx = new WeakMap();
function _evxGuard(s) {
  if (!s) throw new TypeError('Illegal invocation');
  return s;
}
function _evxProps(C, keys) {
  for (const k of keys) {
    Object.defineProperty(C.prototype, k, {
      get() { return _evxGuard(_evx.get(this))[k]; },
      set(v) { _evxGuard(_evx.get(this))[k] = v; },
      enumerable: true, configurable: true,
    });
  }
}
// 刀85: Chrome marks every event-dict member ro — the ctor's `_evx.set` is
// the only writer, so the public setter was a shape lie. Getter-only face;
// sloppy writes silently no-op like a Chrome readonly attribute.
function _evxRo(C, keys) {
  for (const k of keys) {
    Object.defineProperty(C.prototype, k, {
      get() { return _evxGuard(_evx.get(this))[k]; },
      enumerable: true, configurable: true,
    });
  }
}
// Chrome surfaces every unhandled promise rejection on the window as a
// cancelable `unhandledrejection` PromiseRejectionEvent (`promise` + `reason`);
// `rejectionhandled` follows when a handler is attached late. deno_core routes
// both through core callbacks — registered while `Deno` is still on the global
// (page init deletes it; the closure itself survives in the snapshot).
// Returning true marks the rejection handled and suppresses deno_core's
// default exception dispatch, which is what preventDefault() means here.
globalThis.PromiseRejectionEvent = class extends Event {
  // WebIDL PromiseRejectionEventInit: Promise<any> promise = null, any reason.
  constructor(t, o = {}) {
    if (arguments.length < 1) throw new TypeError("Failed to construct 'PromiseRejectionEvent': 1 argument required, but only 0 present.");
    super(t, o);
    _evx.set(this, { promise: o.promise !== undefined ? o.promise : null, reason: o.reason });
  }
};
_evxRo(globalThis.PromiseRejectionEvent, ['promise', 'reason']);
(function () {
  const core = globalThis.Deno && globalThis.Deno.core;
  if (!core || typeof core.setUnhandledPromiseRejectionHandler !== 'function') return;
  core.setUnhandledPromiseRejectionHandler(function (promise, reason) {
    const e = new PromiseRejectionEvent('unhandledrejection', { promise: promise, reason: reason, cancelable: true });
    // (#37) the IDL attribute handler (window/body.onunhandledrejection = fn)
    // is fired by the native dispatch's handler hook (event_hooks.js) — from
    // __windowOnHandlers or the `<body onunhandledrejection>` attribute — and
    // can preventDefault to swallow the default report, like Chrome. Firing
    // it here as well would run every handler twice.
    globalThis.dispatchEvent(e);
    // Chrome's default report is a console line and the page keeps running.
    // deno_core's default (op_dispatch_exception) terminates the runtime —
    // it aborts the rest of the rejection batch and the event loop with it,
    // which is CLI semantics, not browser semantics. Always report true so
    // the dispatch path never arms; do our own console report when the event
    // was not prevented.
    if (!e.defaultPrevented) {
      // The reason itself, not String(reason): an Error formats with its
      // stack (#72), anything else with the console's preview — Chrome's
      // "Uncaught (in promise)" line shows both.
      try { console.error('Uncaught (in promise)', reason); } catch (err) {}
    }
    return true;
  });
  if (typeof core.setHandledPromiseRejectionHandler === 'function') {
    core.setHandledPromiseRejectionHandler(function (promise) {
      globalThis.dispatchEvent(new PromiseRejectionEvent('rejectionhandled', { promise: promise }));
    });
  }
})();
// UIEvent must be defined before its subclasses — the whole UI family
// re-parents onto it to match the Chrome prototype chain (a pointer event
// that fails `instanceof UIEvent` is what delegation libraries notice first).
globalThis.UIEvent = class extends Event {
  constructor(t,o={}) { super(t,o);_evx.set(this,{view:o.view||null,detail:o.detail||0}); }
  // Legacy DOM Level 2 initializer. Positional signature per UI Events spec.
  initUIEvent(type,canBubble,cancelable,view,detail) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'initUIEvent' on 'UIEvent': 1 argument required, but only 0 present.");
    this.initEvent(type,canBubble,cancelable);
    const s = _evxGuard(_evx.get(this)); s.view=view===undefined?null:view; s.detail=detail||0;
  }
};
_evxProps(globalThis.UIEvent, ['view', 'detail']);
globalThis.MouseEvent = class extends UIEvent {
  // Chrome shape: offsetX/Y are NOT constructor-dict settable — a script
  // constructed event reads 0 until dispatch resolves them against the
  // target (the dispatchEvent chokepoint below).
  constructor(t,o={}) { super(t,o);_evx.set(this,{view:o.view||null,detail:o.detail||0,screenX:o.screenX||0,screenY:o.screenY||0,clientX:o.clientX||0,clientY:o.clientY||0,ctrlKey:!!o.ctrlKey,altKey:!!o.altKey,shiftKey:!!o.shiftKey,metaKey:!!o.metaKey,button:o.button||0,buttons:o.buttons||0,relatedTarget:o.relatedTarget||null,offsetX:0,offsetY:0}); }
  // Legacy DOM Level 2 initializer. Positional signature per UI Events spec.
  initMouseEvent(type,canBubble,cancelable,view,detail,screenX,screenY,clientX,clientY,ctrlKey,altKey,shiftKey,metaKey,button,relatedTarget) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'initMouseEvent' on 'MouseEvent': 1 argument required, but only 0 present.");
    this.initEvent(type,canBubble,cancelable);
    const s = _evxGuard(_evx.get(this));
    s.view=view===undefined?null:view;
    s.detail=detail||0;
    s.screenX=screenX||0;
    s.screenY=screenY||0;
    s.clientX=clientX||0;
    s.clientY=clientY||0;
    s.ctrlKey=!!ctrlKey;
    s.altKey=!!altKey;
    s.shiftKey=!!shiftKey;
    s.metaKey=!!metaKey;
    s.button=button||0;
    s.relatedTarget=relatedTarget===undefined?null:relatedTarget;
  }
};
_evxProps(globalThis.MouseEvent, ['view','detail','screenX','screenY','clientX','clientY','ctrlKey','altKey','shiftKey','metaKey','button','buttons','relatedTarget','offsetX','offsetY']);
// x/y alias clientX/Y and pageX/pageY add the root scroll — all read-time
// getters (blitz #663 family), so initMouseEvent rewires keep them true.
Object.defineProperty(globalThis.MouseEvent.prototype, 'x', { get() { return this.clientX; }, configurable: true, enumerable: true });
Object.defineProperty(globalThis.MouseEvent.prototype, 'y', { get() { return this.clientY; }, configurable: true, enumerable: true });
Object.defineProperty(globalThis.MouseEvent.prototype, 'pageX', { get() { return this.clientX + (globalThis.scrollX || globalThis.pageXOffset || 0); }, configurable: true, enumerable: true });
Object.defineProperty(globalThis.MouseEvent.prototype, 'pageY', { get() { return this.clientY + (globalThis.scrollY || globalThis.pageYOffset || 0); }, configurable: true, enumerable: true });
globalThis.KeyboardEvent = class extends UIEvent {
  constructor(t,o={}) { super(t,o);_evx.set(this,{view:o.view||null,detail:o.detail||0,key:o.key||"",code:o.code||"",location:o.location||0,ctrlKey:!!o.ctrlKey,altKey:!!o.altKey,shiftKey:!!o.shiftKey,metaKey:!!o.metaKey,repeat:!!o.repeat}); }
  // Legacy DOM Level 3 initializer. Positional signature per the WebKit/Gecko form.
  initKeyboardEvent(type,canBubble,cancelable,view,key,location,ctrlKey,altKey,shiftKey,metaKey) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'initKeyboardEvent' on 'KeyboardEvent': 1 argument required, but only 0 present.");
    this.initEvent(type,canBubble,cancelable);
    const s = _evxGuard(_evx.get(this));
    s.view=view===undefined?null:view;
    s.key=key===undefined?"":String(key);
    s.location=location||0;
    s.ctrlKey=!!ctrlKey;
    s.altKey=!!altKey;
    s.shiftKey=!!shiftKey;
    s.metaKey=!!metaKey;
  }
};
_evxProps(globalThis.KeyboardEvent, ['view','detail','key','code','location','ctrlKey','altKey','shiftKey','metaKey','repeat']);
globalThis.FocusEvent = class extends UIEvent { constructor(t,o={}) { super(t,o);_evx.set(this,{relatedTarget:o.relatedTarget||null}); } };
_evxProps(globalThis.FocusEvent, ['relatedTarget']);
globalThis.InputEvent = class extends UIEvent { constructor(t,o={}) { super(t,o);_evx.set(this,{data:o.data||null,inputType:o.inputType||""}); } };
_evxProps(globalThis.InputEvent, ['data', 'inputType']);
globalThis.ErrorEvent = class extends Event { constructor(t,o={}) { super(t,o);_evx.set(this,{message:o.message||"",filename:o.filename||"",lineno:o.lineno||0,colno:o.colno||0,error:o.error||null}); } };
_evxRo(globalThis.ErrorEvent, ['message','filename','lineno','colno','error']);
// AnimationEvent moved to the generator (issue #67 probe family):
// platform/gen/animationevent.gen.js builds it from the css-animations IDL
// (arity + AnimationEventInit dictionary expansion + Chrome message shapes)
// with zero generator changes — the class below is deleted. animation_lifecycle
// dispatches through the public ctor either way.
globalThis.TransitionEvent = class TransitionEvent extends Event {
  // WebIDL TransitionEventInit: propertyName "" / elapsedTime 0 /
  // pseudoElement "" defaults (blitz#863 family 2). Was an empty Event
  // subclass, so every property read came back undefined.
  constructor(t, o = {}) {
    if (arguments.length < 1) throw new TypeError("Failed to construct 'TransitionEvent': 1 argument required, but only 0 present.");
    super(t, o);
    _evx.set(this, {
      propertyName: o.propertyName !== undefined ? String(o.propertyName) : "",
      elapsedTime: o.elapsedTime !== undefined ? Number(o.elapsedTime) : 0,
      pseudoElement: o.pseudoElement !== undefined ? String(o.pseudoElement) : "",
    });
  }
};
_evxRo(globalThis.TransitionEvent, ['propertyName', 'elapsedTime', 'pseudoElement']);
globalThis.WheelEvent = class extends MouseEvent {
  constructor(t,o={}) {
    super(t,o);
    const s = _evxGuard(_evx.get(this));
    s.deltaX=o.deltaX||0; s.deltaY=o.deltaY||0; s.deltaZ=o.deltaZ||0; s.deltaMode=o.deltaMode||0;
  }
};
_evxProps(globalThis.WheelEvent, ['deltaX', 'deltaY', 'deltaZ', 'deltaMode']);

globalThis.CompositionEvent = class extends UIEvent {
  constructor(t,o={}) { super(t,o);_evx.set(this,{view:o.view||null,detail:o.detail||0,data:o.data||""}); }
  // Legacy DOM Level 3 initializer. Positional signature per UI Events spec.
  initCompositionEvent(type,canBubble,cancelable,view,data) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'initCompositionEvent' on 'CompositionEvent': 1 argument required, but only 0 present.");
    this.initEvent(type,canBubble,cancelable);
    const s = _evxGuard(_evx.get(this)); s.view=view===undefined?null:view; s.data=data===undefined?"":String(data);
  }
};
_evxProps(globalThis.CompositionEvent, ['view', 'detail', 'data']);
globalThis.PopStateEvent = class extends Event {
  constructor(type, init) {
    super(type, init || {});
    // Real PopStateEvent exposes `state` from the entry being navigated to.
    // The earlier stub inherited Event but never stored state, so
    // `popstate.state` was always undefined and SPA routers reading
    // `event.state` to restore route info would mis-render.
    _evx.set(this, { state: init && 'state' in init ? init.state : null });
  }
};
_evxProps(globalThis.PopStateEvent, ['state']);
globalThis.HashChangeEvent = class extends Event {};
globalThis.MessageEvent = class extends Event {
  constructor(t,o={}) {
    super(t,o);
    _evx.set(this,{data:o.data,origin:o.origin===undefined?'':String(o.origin),lastEventId:o.lastEventId===undefined?'':String(o.lastEventId),source:o.source===undefined?null:o.source,ports:o.ports===undefined?[]:o.ports});
  }
};
_evxRo(globalThis.MessageEvent, ['data', 'origin', 'lastEventId', 'source', 'ports']);
globalThis.CloseEvent = class extends Event { constructor(t,o={}) { super(t,o);_evx.set(this,{wasClean:!!o.wasClean,code:o.code===undefined?0:(o.code|0),reason:o.reason===undefined?'':String(o.reason)}); } };
_evxRo(globalThis.CloseEvent, ['wasClean', 'code', 'reason']);
globalThis.ProgressEvent = class ProgressEvent extends Event {
  constructor(type, init) {
    super(type, init || {});
    const i = init || {};
    _evx.set(this, {
      lengthComputable: !!i.lengthComputable,
      loaded: i.loaded != null ? Number(i.loaded) : 0,
      total: i.total != null ? Number(i.total) : 0,
    });
  }
};
_evxRo(globalThis.ProgressEvent, ['lengthComputable', 'loaded', 'total']);
globalThis.ClipboardEvent = class extends Event {};
globalThis.SubmitEvent = class extends Event {};

// ToggleEvent backs the popover beforetoggle/toggle events. oldState and
// newState are "open"/"closed". These events do not bubble; beforetoggle is
// cancelable only for the closed -> open (show) transition, toggle is never
// cancelable. See HTML "popover" and html/semantics/popovers WPT.
globalThis.ToggleEvent = class ToggleEvent extends Event {
  constructor(type, init = {}) {
    super(type, init);
    _evx.set(this, {
      oldState: init.oldState !== undefined ? String(init.oldState) : "",
      newState: init.newState !== undefined ? String(init.newState) : "",
    });
  }
};
_evxRo(globalThis.ToggleEvent, ['oldState', 'newState']);
_markNative(globalThis.ToggleEvent);

// Fires when an auto content-visibility box skips in/out of rendering.
globalThis.ContentVisibilityAutoStateChangeEvent = class ContentVisibilityAutoStateChangeEvent extends Event {
  constructor(type, init = {}) {
    super(type, init);
    _evx.set(this, { skipped: Boolean(init && init.skipped) });
  }
  get skipped() { return _evxGuard(_evx.get(this)).skipped === true; }
};

// Missing PromiseRejectionEvent made core-js misdetect the environment and
// override native Promise with a broken polyfill, breaking Vue rendering
// (upstream #521). StorageEvent completes the storage API surface (#522).
// PromiseRejectionEvent's `promise` member is required — Chrome throws
// TypeError when the init dict lacks it.
globalThis.PromiseRejectionEvent = class PromiseRejectionEvent extends Event {
  constructor(type, init) {
    if (arguments.length < 2 || init == null || !('promise' in Object(init))) {
      throw new TypeError(
        "Failed to construct 'PromiseRejectionEvent': required member promise is undefined."
      );
    }
    super(type, init);
    _evx.set(this, { promise: init.promise, reason: init.reason });
  }
};
_evxRo(globalThis.PromiseRejectionEvent, ['promise', 'reason']);
_markNative(globalThis.PromiseRejectionEvent);

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_evx', _evx);
  d('_evxGuard', _evxGuard);
  d('_evxProps', _evxProps);
  d('_evxRo', _evxRo);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
