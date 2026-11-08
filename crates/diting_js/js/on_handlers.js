(function () { // @@SCOPE
// god-file split (刀52, GlobalEventHandlers on* IDL belt): every on<event>
// as an enumerable accessor on Element.prototype, Document.prototype (via
// the deferred queue - drained before any page script) and window (direct,
// the chrome-155 truth set from a real https page, NOT the Element face
// list). The getter reflects the inline content attribute (<div
// oninput="..."> compiles, like Chrome); window.onerror/
// onunhandledrejection stay real data properties (hasOwnProperty guard).
// React's ChangeEventPlugin feature-detects 'oninput' in document and
// silently swallows synthetic input events when it misses - the original
// bug this belt fixes. Deferred registration through the published member
// (bootstrap installs the queue fn first). No marking, no export rows.
// ---- GlobalEventHandlers `on*` IDL properties ----
// Real browsers expose every `on<event>` as an accessor property on
// Element.prototype, Document.prototype and window. Libraries feature-detect
// them: React's ChangeEventPlugin does `'oninput' in document` and, when that
// misses, falls back to an IE9-era propertychange path that silently swallows
// every synthetic `input` event - which is why filling React-controlled
// inputs (native value setter + dispatchEvent('input')) never reached
// onChange in this engine while `click` worked fine. The getter also
// compiles inline content attributes (`<div oninput="...">`), so
// `typeof el.oninput === 'function'` after setAttribute, like Chrome.
// (Fingerprint note: accessors live on the prototype, so they do not show up
// in Object.keys(instance) / hasOwnProperty scans of elements.)
const _GLOBAL_EVENT_NAMES = [
  'abort', 'animationcancel', 'animationend', 'animationiteration',
  'animationstart', 'auxclick', 'beforeinput', 'beforematch', 'beforetoggle',
  'blur', 'cancel', 'canplay', 'canplaythrough', 'change', 'click', 'close',
  'contextlost', 'contextmenu', 'contextrestored', 'copy', 'cuechange', 'cut',
  'dblclick', 'drag', 'dragend', 'dragenter', 'dragexit', 'dragleave',
  'dragover', 'dragstart', 'drop', 'durationchange', 'emptied', 'ended',
  'error', 'focus', 'focusin', 'focusout', 'formdata', 'gotpointercapture',
  'input', 'invalid', 'keydown', 'keypress', 'keyup', 'load', 'loadeddata',
  'loadedmetadata', 'loadstart', 'lostpointercapture', 'mousedown',
  'mouseenter', 'mouseleave', 'mousemove', 'mouseout', 'mouseover', 'mouseup',
  'paste', 'pause', 'play', 'playing', 'pointercancel', 'pointerdown',
  'pointerenter', 'pointerleave', 'pointermove', 'pointerout', 'pointerover',
  'pointerrawupdate', 'pointerup', 'progress', 'ratechange', 'reset',
  'resize', 'scroll', 'scrollend', 'securitypolicyviolation', 'seeked',
  'seeking', 'select', 'selectionchange', 'slotchange', 'stalled', 'submit',
  'suspend', 'timeupdate', 'toggle', 'touchcancel', 'touchend',
  'touchmove', 'touchstart', 'transitioncancel', 'transitionend',
  'transitionrun', 'transitionstart', 'volumechange', 'waiting', 'wheel',
];
function _defineOnHandler(obj, name, storeKey, inlineFallback) {
  if (Object.prototype.hasOwnProperty.call(obj, name)) return;
  Object.defineProperty(obj, name, {
    configurable: true,
    enumerable: true, // Chrome: event-handler IDL attributes are enumerable
    get() {
      const store = this[storeKey];
      if (store && Object.prototype.hasOwnProperty.call(store, name)) return store[name];
      // No JS handler assigned: reflect the inline content attribute, if any.
      if (inlineFallback && typeof this._resolveInlineHandler === 'function') {
        return this._resolveInlineHandler(name);
      }
      return null;
    },
    set(fn) {
      if (typeof fn !== 'function') fn = null;
      const store = this[storeKey] || (this[storeKey] = {});
      store[name] = fn;
    },
  });
}
globalThis.__diting_domDeferred(function () {
  for (const _n of _GLOBAL_EVENT_NAMES) _defineOnHandler(Element.prototype, 'on' + _n, '__onHandlers', true);
  for (const _n of _GLOBAL_EVENT_NAMES) _defineOnHandler(Document.prototype, 'on' + _n, '__onHandlers', false);
});
// window-only handlers (WindowEventHandlers + the rest of GlobalEventHandlers).
// globalThis.onerror / onunhandledrejection are already real data properties;
// the `hasOwnProperty` guard above skips them, keeping the error capture intact.
// chrome-155 truth: the EXACT window on* set, shot from a real https page
// (tools/webplatform/chrome/chrome-window-on-set.json). Not _GLOBAL_EVENT_NAMES — that's the
// Element/Document face (copy/cut/paste/dragexit/touch*/focusin/focusout are
// NOT window handlers in Chrome desktop; installing them here was the own-prop
// face leak this batch fixes).
const _WINDOW_EVENT_NAMES = [
  'abort', 'afterprint', 'animationcancel', 'animationend', 'animationiteration', 'animationstart',
  'appinstalled', 'auxclick', 'beforeinput', 'beforeinstallprompt', 'beforematch', 'beforeprint',
  'beforetoggle', 'beforeunload', 'beforexrselect', 'blur', 'cancel', 'canplay',
  'canplaythrough', 'change', 'click', 'close', 'command', 'contentvisibilityautostatechange',
  'contextlost', 'contextmenu', 'contextrestored', 'cuechange', 'dblclick', 'devicemotion',
  'deviceorientation', 'deviceorientationabsolute', 'drag', 'dragend', 'dragenter', 'dragleave',
  'dragover', 'dragstart', 'drop', 'durationchange', 'emptied', 'ended',
  'error', 'focus', 'formdata', 'gamepadconnected', 'gamepaddisconnected', 'gotpointercapture',
  'hashchange', 'input', 'invalid', 'keydown', 'keypress', 'keyup',
  'languagechange', 'load', 'loadeddata', 'loadedmetadata', 'loadstart', 'lostpointercapture',
  'message', 'messageerror', 'mousedown', 'mouseenter', 'mouseleave', 'mousemove',
  'mouseout', 'mouseover', 'mouseup', 'mousewheel', 'offline', 'online',
  'pagehide', 'pagereveal', 'pageshow', 'pageswap', 'pause', 'play',
  'playing', 'pointercancel', 'pointerdown', 'pointerenter', 'pointerleave', 'pointermove',
  'pointerout', 'pointerover', 'pointerrawupdate', 'pointerup', 'popstate', 'progress',
  'ratechange', 'rejectionhandled', 'reset', 'resize', 'scroll', 'scrollend',
  'scrollsnapchange', 'scrollsnapchanging', 'search', 'securitypolicyviolation', 'seeked', 'seeking',
  'select', 'selectionchange', 'selectstart', 'slotchange', 'stalled', 'storage',
  'submit', 'suspend', 'timeupdate', 'toggle', 'transitioncancel', 'transitionend',
  'transitionrun', 'transitionstart', 'unhandledrejection', 'unload', 'volumechange', 'waiting',
  'webkitanimationend', 'webkitanimationiteration', 'webkitanimationstart', 'webkittransitionend', 'wheel',  'unhandledrejection', 'webkitanimationend', 'webkitanimationiteration',
  'webkitanimationstart', 'webkittransitionend',
];
for (const _n of _WINDOW_EVENT_NAMES) _defineOnHandler(globalThis, 'on' + _n, '__windowOnHandlers', false);
// Fire the on* handler alongside addEventListener listeners for the two
// dispatch paths that did not consult it before (document and window).
// (M3 B3/B4) RETIRED: the document leg is the native core's `handler` hook
// now (event_hooks.js) — it fires document.on* for every dispatch level
// with Chrome's accessor-slot shape, so the JS pre-fire wrapper is gone.
// window.onerror stays pipeline-only, as before (the hook's 'error' arm).
// Macrotask-boundary guard: mark every synchronous dispatch window so
// timer/message deliveries re-arm instead of firing mid-dispatch (see
// _runAsMacrotask). Real browsers never interleave the task queue with
// an in-flight event dispatch; without this, a resolved op_sleep
// continuation fires at an op checkpoint inside a listener — React 16's
// scheduler re-entered its own commit this way and threw #327, freezing
// every later update on the root. (M3 B3/B4) RETIRED: Node, Element and
// Document all dispatch through the native core now, whose enter/exit
// hooks ARE this bracket; event_hooks.js also deletes the inert trio a
// bake-time fill left on Node.prototype so nothing shadows the core.

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_WINDOW_EVENT_NAMES', _WINDOW_EVENT_NAMES);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
