"use strict";
(function () { // @@SCOPE

// Receiver validation: real browsers throw TypeError("Illegal invocation")
// when a DOM method is called with a fake receiver (e.g.
// Object.create(HTMLSelectElement.prototype).setHTMLUnsafe(...) — a classic
// bot-detector probe). Our shims used to pass this._nid (undefined) down,
// where the Rust op's `parse().unwrap_or(0)` silently targeted node 0 — the
// DOCUMENT — and e.g. set_inner_html wiped the whole page. Commands not in
// _domStrA1 take a numeric node id as a1; reject anything else.
// Script-scoped capture of the deno op table. All runtime op calls go
// through this binding (not the `Deno` global) so __diting_init can DELETE
// globalThis.Deno / __bootstrap per page: a `Deno` property on window is the
// canonical marker of a deno-based runtime, and anti-bot collectors
// (WorkOS Radar) hash window property names. A top-level `const` creates a
// global *lexical* binding — it never appears on the global object at all.
const _OPS = Deno.core.ops;
// DOM mutation epoch (upstream's shape): every tree/attribute write bumps
// this at the one choke point all _domRaw commands pass through, so
// consumers of retained snapshots (getComputedStyle) can tell staleness
// without a native round-trip.
let _ditingMutationEpoch = 0;
// Live epoch read for the split computed-style file (computed_style.js):
// d() publishes values, not bindings — a closure keeps the read live.
const _ditingEpochNow = () => _ditingMutationEpoch;
const _domRaw = (cmd, a1, a2) => {
  if (_DOM_MUTATION_COMMANDS.has(cmd)) _ditingMutationEpoch++;
  if (_TREE_EDGE_MUTATION_COMMANDS.has(cmd)) _edgeMemo.clear();
  return _OPS.op_dom(cmd, String(a1 ?? ""), String(a2 ?? ""));
};
// CDP evaluate runs in this realm but outside this closure; box-model
// handlers read the real layout_rect op through this export — gBCR's
// synthetic 12-col grid fallback would fabricate a box for every boxless
// node, and Chrome errors "Could not compute box model." for those.
globalThis.__diting_domRaw = _domRaw;

// Recompute the label a select displays and land it as the select's
// live_value mirror (form paint batch): paint reads exactly this, and the
// JS walk supersedes Rust's parsed-attribute fallback because it also sees
// JS-side dirtiness (option._selected). The precedence matches the value
// getter the label must agree with: first selected option, else the first
// option, else ''. An option's text is its `label` attribute when present,
// else its content — trimmed either way. An empty label writes through,
// which paint treats as "no mirror" and falls back to the attributes.
function _mirrorSelectLabel(selectEl) {
  const opts = selectEl.querySelectorAll('option');
  let o = null;
  for (let i = 0; i < opts.length; i++) { if (opts[i].selected) { o = opts[i]; break; } }
  if (!o && opts.length) o = opts[0];
  const label = o
    ? ((o.getAttribute('label') !== null ? o.getAttribute('label') : o.textContent) || '').trim()
    : '';
  _domRaw("set_live_value", String(selectEl._nid), label);
}
const _dom = (cmd, a1, a2) => {
  if (!_domStrA1.has(cmd) && (a1 === undefined || a1 === null || a1 === "" || isNaN(+a1))) {
    throw new TypeError("Illegal invocation");
  }
  if (_domNumA2.has(cmd) && (a2 === undefined || a2 === null || a2 === "" || isNaN(+a2))) {
    throw new TypeError("Illegal invocation");
  }
  return _domRaw(cmd, a1, a2);
};

// WeakSet (Obscura steal a36ec393): a page deleting an API member
// (`delete AudioBuffer.prototype.copyToChannel`) must actually release the
// function — a strong Set pins every marked fn forever. The bound refs
// keep toString working even after a page replaces WeakSet.prototype
// methods (the same patch's tamper hardening).
const _nativeFns = new WeakSet();
const _nativeFnHas = _nativeFns.has.bind(_nativeFns);
const _nativeFnAdd = _nativeFns.add.bind(_nativeFns);
const _origToString = Function.prototype.toString;
// Method syntax gives the override the native function's shape: name
// "toString", length 0, non-constructible, and no own `prototype` property
// (upstream 846ed7d). A plain `function() {}` assignment leaks all four.
const _functionToString = {
  toString() {
    if (_nativeFnHas(this)) {
      return `function ${this.name || ''}() { [native code] }`;
    }
    return _origToString.call(this);
  },
}.toString;
Function.prototype.toString = _functionToString;
// (#30) Engine-internal state lives in `_`-prefixed own properties because
// the DOM here is JS-implemented. Chrome keeps its state in native slots and
// exposes nothing — Object.keys(el) is [] there — so hide ours the same way:
// `for..in` walkers (zone.js patchClass, jQuery-style copies, fingerprint
// probes) must not see implementation fields. Only first creation defines
// enumerability; later plain assignments keep the hidden descriptor.
function __def(o, k, v) {
  Object.defineProperty(o, k, { value: v, writable: true, configurable: true });
  return v;
}
function __hideOwn(o) {
  const names = Object.getOwnPropertyNames(o);
  for (let i = 0; i < names.length; i++) {
    const k = names[i];
    if (k.charCodeAt(0) !== 95) continue;
    const d = Object.getOwnPropertyDescriptor(o, k);
    if (!d || !d.enumerable) continue;
    Object.defineProperty(o, k, { enumerable: false });
  }
  return o;
}
// (M3) The WeakMap listener store is GONE — every face that used it rides
// the native EventTarget core now (private v8 slots), and the last
// consumers (the iframe realm trios) went with B2c. The lbox/MQL/message-
// port shims below keep their own per-instance stores.
// DOM §2.9 listener protocol, shared by every event registry: a listener
// is a function (invoked with the target as `this`) or an object with a
// handleEvent method (invoked with the LISTENER itself as `this`) —
// webcomponents-era pages register the object form. null/undefined are
// legal no-op registrations. Blind .call() on the object form is where
// github's 4× "h.call is not a function" came from (#191).
function _invokeListener(h, target, event) {
  if (typeof h === 'function') return h.call(target, event);
  if (h != null && typeof h.handleEvent === 'function') return h.handleEvent(event);
}
// WebIDL binding gate: EventListener is a nullable callback — null (and
// undefined) register as inert no-ops, anything non-object/non-function
// throws at registration time, exactly where Chrome throws it.
function __listenerGate(h) {
  if (h != null && typeof h !== 'function' && typeof h !== 'object') {
    throw new TypeError("Failed to execute 'addEventListener' on 'EventTarget': parameter 2 is not of type 'Object'.");
  }
}
// (#31) Web IDL exposes public attributes on the PROTOTYPE as accessor pairs;
// instances carry nothing (Object.keys(new WebSocket(...)) is [] in Chrome).
// Constructors keep their plain `this.x = ...` writes — those route through
// the setters below and land in hidden `_x` fields, so both faces match.
// Private `_x` slot names minted by __acc; the instance enumeration patch hides them (they are not in the generated internal_names).
const __accPrivs = new Set();
Object.defineProperty(globalThis, '__diting_accPrivs', { value: __accPrivs, writable: true, enumerable: false, configurable: true });
function __acc(C, names) {
  for (const k of names) {
    const priv = '_' + k;
    __accPrivs.add(priv);
    // The backing must NOT be a page-reachable property name. The old
    // `this._x` own-slot face collided with page wrapper conventions:
    // baxia (g.alicdn.com/sd/baxia, tmall risk-control) saves the original
    // handler as `this._onloadend` before wrapping `onloadend`, and our
    // setter then clobbered that saved original with the wrapper — dispatch
    // → wrapper → r._onloadend (= wrapper) → … RangeError, killing the
    // page's whole module system (tmall publish/category, live 2026-11-02).
    // In Chrome the IDL attribute backing is a native slot JS cannot name,
    // so `x._onloadend` and the stored handler never meet. Same privacy via
    // the 刀84 hidden-slot channel. (__accRo keeps the own-prop face: its
    // writers are engine-side __def(o, priv, v) calls, and no page wrapper
    // family saves originals under those ro names.)
    Object.defineProperty(C.prototype, k, {
      get() { const s = globalThis.__diting_liftSlots.get(this); return s === undefined ? undefined : s[priv]; },
      set(v) { __diting_liftSlot(this)[priv] = v; },
      configurable: true,
      enumerable: true,
    });
  }
}
// 刀85: Chrome-ro members get the same hidden `_x` backing but a
// getter-only face. Engine writes land straight in the field via __def —
// the setter's old destination — so construction/state legs are unchanged.
function __accRo(C, names) {
  for (const k of names) {
    const priv = '_' + k;
    __accPrivs.add(priv);
    Object.defineProperty(C.prototype, k, {
      get() { return this[priv]; },
      configurable: true,
      enumerable: true,
    });
  }
}
// (M4 刀56) __lboxProto — the minimal per-instance listener-box trio for
// socket-like shims — is RETIRED. Its classes (EventSource / WebSocket /
// BroadcastChannel) have chained to EventTarget.prototype since the
// global_surface wiring; the trio was a redundant shadow with WEAKER
// semantics than the native core (no options/once/capture, no on* firing).
// The classes now inherit the native trio like every Chrome EventTarget,
// and their delivery fires on* through the native dispatch's handler hook
// (the accessors are prototype-level, so isHandlerSlot passes).

function _markNative(fn) { if (typeof fn === 'function') _nativeFnAdd(fn); return fn; }
// Flat post-bootstrap scripts (lift_own / stub_members / window_extras ...) reach it through this hidden global instead of a free
// reference, so the dependency ratchet's cross-file load-time count stays flat. (`__diting` names are filtered from enumeration.)
Object.defineProperty(globalThis, '__diting_markNative', { value: _markNative, writable: true, enumerable: false, configurable: true });
// god-file split (DOM core): the two install loops below still live here
// but touch domcore names at LOAD time; they queue into a hidden prop and
// interface_sweep.js drains them — before any page script runs — instead
// of joining the cross-file load-time graph.
const _domDeferred = (fn) => {
  let q = globalThis.__diting_dom_deferred;
  if (!Array.isArray(q)) {
    q = [];
    Object.defineProperty(globalThis, '__diting_dom_deferred', { value: q, writable: true, configurable: true, enumerable: false });
  }
  q.push(fn);
};
// god-file split (刀52): on-handler IDL belt registers its deferred through this
// hidden global (ratchet-flat member form, __diting_markNative precedent).
Object.defineProperty(globalThis, '__diting_domDeferred', { value: _domDeferred, writable: true, configurable: true, enumerable: false });

// Mark every method AND accessor on a shim prototype as native. Lie
// detectors (fingerprintjs-style) verify `Function.prototype.toString.call(fn)`
// reports [native code] for each API they probe — an unmarked shim flips
// their "API tampered" flag, and some detectors then run destructive probes
// in the main document as punishment.
function _markNativeProto(proto) {
  if (!proto) return;
  for (const p of Object.getOwnPropertyNames(proto)) {
    if (p === 'constructor') continue;
    const d = Object.getOwnPropertyDescriptor(proto, p);
    if (!d) continue;
    if (typeof d.value === 'function') _markNative(d.value);
    if (typeof d.get === 'function') _markNative(d.get);
    if (typeof d.set === 'function') _markNative(d.set);
  }
}
_nativeFnAdd(_functionToString);

// (M3 B3/B4) The `_eventRegistry` / `_eventRegistryCap` buckets and their
// helpers went with the Element and Document dispatch trilogies — listener
// storage is the native core's private slot now (`diting#et-listeners`,
// crates/diting_runtime/src/native/event_target.rs).

// Shallow-clone one node as a real DOM node: text/comment copy their data,
// fragments come back empty, elements are recreated in their namespace with all
// attributes copied. Used by cloneNode's non-element deep path.
function _shallowCloneNode(node) {
  const nt = node.nodeType;
  if (nt === 3) return document.createTextNode(node.data != null ? node.data : (node.textContent || ""));
  if (nt === 8) return document.createComment(node.data != null ? node.data : (node.nodeValue || ""));
  if (nt === 11) return document.createDocumentFragment();
  if (nt !== 1) return null;
  const ns = node.namespaceURI;
  const el = (ns && ns !== "http://www.w3.org/1999/xhtml")
    ? document.createElementNS(ns, node.nodeName)
    : document.createElement(node.localName || node.nodeName.toLowerCase());
  const names = node.getAttributeNames ? node.getAttributeNames() : [];
  for (const name of names) {
    const v = node.getAttribute(name);
    if (v !== null) el.setAttribute(name, v);
  }
  // CSS declarations currently live on the JS wrapper independently of the
  // DOM attribute. Copy that state as well so styles assigned through
  // `node.style` survive cloning even before attribute reflection runs.
  if (node.style && node.style.cssText) el.style.cssText = node.style.cssText;
  return el;
}

// (face fix) The early `globalThis['on'+ev] = null` loop is gone: it installed
// enumerable data props where Chrome has accessor pairs, and its name set was
// the pre-P2 approximation. The single chrome-exact accessor install at
// on_handlers.js (god-file split 刀52) now owns the whole window on* surface — `in`
// checks and assignment behave the same through the accessors.

globalThis.Window = globalThis.Window || function Window() {};
Object.defineProperty(globalThis.Window, Symbol.hasInstance, {
  value(obj) { return obj === globalThis || (obj && obj.window === obj); },
  configurable: true,
});
// (face fix) window must NOT own `constructor` — Chrome keeps it on
// Window.prototype. global_surface.js moves it to the proto layer and points
// it at Window so `self.constructor === Window` gates still pass.

// ResizeObserver is defined earlier with real per-target firing; the stub
// that previously lived here was a no-op that clobbered the real class.
//

// Event and CustomEvent are native interfaces (crates/diting_runtime/src/
// native/event.rs): state in a private slot, prototype accessors, the
// unforgeable own isTrusted, one dispatch-flag semantics for initEvent.
// Installed with the faces before this script runs.

// FontFace / FontFaceSet / document.fonts live in platform/fontfaceset.impl.js (M4 knife 52).

globalThis.reportError = globalThis.reportError || ((e) => console.error(e));

[
  // (刀64) matchMedia moved to media_queries.js — the install wraps
  // _markNative at its site (marking travels, 刀22/刀54 precedent).
  // (刀54) the timer/raf faces moved to timer_belt.js — their native
  // marking migrated there (bootstrap loads first; 刀22 precedent).
  globalThis.structuredClone, globalThis.reportError,
  // (刀78) the console face moved to console_face.js — its seven members
  // self-mark there (marking travels, 刀22/刀54 precedent).
  // (刀22) Notification + chrome.csi/loadTimes moved to bchk_apis.js —
  // their native marking migrated there (bootstrap loads first and would
  // see undefined here; the member reads also hid from the analyzer).
  // (刀75) PerformanceObserver stub moved to performance_observer.js — the install self-marks there (刀22/刀54 precedent).
  // (刀70) ResizeObserver moved to resize_observer.js — its marking
  // migrated there (install site self-marks, 刀22/刀54 precedent);
  // (刀58) MutationObserver moved to mutation_observer.js — same.
  // mutation_observer.js — its marking migrated there (刀22 precedent).
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });





// Geometry family (DOMPoint/DOMRect/DOMQuad/DOMMatrix*/WebKitCSSMatrix):
// generated shape + behavior module now owns this whole family
// (platform/gen/geometry.gen.js + platform/geometry.impl.js, M4 knife 1).

// Snapshot-time pre-computation of the hide list. Bootstrap.js runs once
// during the V8 snapshot build (build.rs); this line captures the set of
// globals defined by bootstrap that we want to hide and stashes them
// for __diting_init to consume on every subsequent page. The snapshot
// preserves the array as a regular global.
// getOwnPropertyNames, not Object.keys: internals already made non-enumerable
// before this line would be omitted by Object.keys and escape the per-page
// hiding below (upstream 4c33f6d).
globalThis.__diting_hide_list = Object.getOwnPropertyNames(globalThis).filter(k =>
  k.startsWith('_') || k.includes('diting') || k.includes('Diting')
);







/* ===== fp-diff-1014 P0 sweep (runs once at snapshot build) ===== */
// #1 Name every anonymous function-valued global after its property: WebIDL
// operations all report their own name, and table-driven installs (computed
// keys / object literals) produce anonymous functions — `fetch.toString()`
// answering "function () { [native code] }" was a one-line automation tell.
// Retained as a global and re-invoked from interface_sweep.js (the LAST
// snapshot module) — fetch.js/xhr.js/window_faces.js install more globals
// after this file, and a bootstrap-time-only sweep never sees them
// (`globalThis.fetch` was still nameless for exactly that reason).
const _globalThisFixed = new WeakSet();
globalThis.__diting_name_sweep = function () {
  const sweep = (obj) => {
    const names = Object.getOwnPropertyNames(obj);
    for (let i = 0; i < names.length; i++) {
      const k = names[i];
      if (k === 'constructor' || k.startsWith('_') || k.includes('diting')) continue;
      const d = Object.getOwnPropertyDescriptor(obj, k);
      if (d && typeof d.value === 'function' && d.value.name === '') {
        try { Object.defineProperty(d.value, 'name', { value: k, configurable: true }); } catch (e) {}
      }
    }
  };
  sweep(globalThis);
  if (globalThis.Window && Window.prototype) sweep(Window.prototype);
  // Detached EventTarget calls: `const add = document.addEventListener;
  // add(type, fn)` runs with `this` undefined, and Chrome falls back to the
  // global object instead of throwing. Here the registry keyed a WeakMap on
  // `this`, so the call threw "Invalid value used as weak map key" — xhs's
  // mnsv2 loader makes exactly this call inside a try/catch, read the throw
  // as a hostile runtime and silently skipped installing mnsv2/mns0101.
  const evtProtos = [globalThis.Document && Document.prototype,
    globalThis.Element && Element.prototype,
    globalThis.Window && Window.prototype,
    globalThis.EventTarget && EventTarget.prototype];
  for (const P of evtProtos) {
    if (!P) continue;
    for (const m of ['addEventListener', 'removeEventListener', 'dispatchEvent']) {
      const d = Object.getOwnPropertyDescriptor(P, m);
      if (!d || typeof d.value !== 'function' || _globalThisFixed.has(d.value)) continue;
      const orig = d.value;
      const w = ({ [m]() { return orig.apply(this == null ? globalThis : this, arguments); } })[m];
      try { Object.defineProperty(w, 'length', { value: orig.length, configurable: true }); } catch (e) {}
      _markNative(w);
      _globalThisFixed.add(w);
      try { Object.defineProperty(P, m, { value: w, writable: d.writable, enumerable: d.enumerable, configurable: d.configurable }); } catch (e) {}
    }
  }
};
globalThis.__diting_name_sweep();

// #3 support: snapshot the frame-copyable globals. Value props only —
// accessors would alias main-window state into frames. Builder retained as
// a global and re-run from interface_sweep.js (the LAST snapshot module)
// so globals installed by fetch.js/xhr.js/... after bootstrap.js land in
// the copy set too.
globalThis.__diting_frame_globals_build = function () {
  const skip = new Set(['document','location','window','self','top','parent','frames','frameElement','length','name','closed','event','origin',
    // Event-target face must stay frame-local: copying the main window's
    // (addEventListener|removeEventListener|dispatchEvent) as own props
    // hijacks iframe listener registration into the main window's boxes and
    // postMessage delivery into frames goes dead.
    'addEventListener','removeEventListener','dispatchEvent']);
  const out = [];
  const names = Object.getOwnPropertyNames(globalThis);
  for (let i = 0; i < names.length; i++) {
    const k = names[i];
    if (skip.has(k) || k.startsWith('_') || k.includes('diting')) continue;
    const d = Object.getOwnPropertyDescriptor(globalThis, k);
    if (!d || !('value' in d)) continue;
    out.push([k, d]);
  }
  return out;
};
globalThis.__diting_frame_globals = globalThis.__diting_frame_globals_build();
globalThis.__diting_hide_list.push('__diting_frame_globals', '__diting_frame_globals_build', '__diting_name_sweep');

// (M3 B2c) Frames inherit Window.prototype — the link now lives in
// iframe_realm.js, after the class definitions (this site ran at bootstrap
// load time and died in the TDZ catch, so it never linked anything).

// fp-diff-1014: prototype key ORDER. Chrome installs WebIDL attributes
// first and `constructor` late; ours had constructor first (and Screen
// lacked `onchange` entirely). Orders captured from a real Chrome 155 on
// macOS (tools/webplatform/chrome/fp-chrome.json, 2026-10-14). Non-configurable props
// survive untouched — every step is guarded so a pinned descriptor can
// never take the whole bootstrap down.
function __ditingReorderProto(proto, order) {
  const names = Object.getOwnPropertyNames(proto);
  const descs = new Map();
  for (const n of names) descs.set(n, Object.getOwnPropertyDescriptor(proto, n));
  for (const n of names) { try { delete proto[n]; } catch (e) {} }
  const done = new Set();
  for (const n of order) {
    if (descs.has(n) && !done.has(n)) {
      try { Object.defineProperty(proto, n, descs.get(n)); done.add(n); } catch (e) {}
    }
  }
  for (const n of names) {
    if (!done.has(n)) { try { Object.defineProperty(proto, n, descs.get(n)); } catch (e) {} }
  }
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_OPS', _OPS);
  d('__acc', __acc);
  d('__accRo', __accRo);
  d('__def', __def);
  d('__ditingReorderProto', __ditingReorderProto);
  d('__hideOwn', __hideOwn);
  d('_ditingEpochNow', _ditingEpochNow);
  a('_ditingMutationEpoch', () => _ditingMutationEpoch, (v) => { _ditingMutationEpoch = v; });
  d('_dom', _dom);
  d('_domRaw', _domRaw);
  d('_markNativeProto', _markNativeProto);
  d('_mirrorSelectLabel', _mirrorSelectLabel);
  d('_shallowCloneNode', _shallowCloneNode);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
