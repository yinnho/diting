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
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
