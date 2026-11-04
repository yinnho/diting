"use strict";
(function () { // @@SCOPE

globalThis.__diting_errors = [];

// 刀84: the shared hidden-slot channel for sealed (Chrome-readonly) prototype
// accessors. lift_own.js installs the getters; every internal writer calls
// __diting_liftSlot(obj).member = v. Defined here — platform_bundle constructs
// sealed classes (TransformStream family) during snapshot, before lift_own runs.
globalThis.__diting_liftSlots = new WeakMap();
globalThis.__diting_liftSlot = function (o) {
  let s = globalThis.__diting_liftSlots.get(o);
  if (s === undefined) { s = Object.create(null); globalThis.__diting_liftSlots.set(o, s); }
  return s;
};

globalThis.onerror = function(msg, src, line, col, error) {
  globalThis.__diting_errors.push({msg: String(msg), src: String(src||""), line, error: String(error||"")});
};

// Uncaught-exception reporting (Chrome parity): a classic script that
// throws, or a listener/timer callback that throws, must surface on the
// window error hooks — onerror + an ErrorEvent('error') dispatched on
// window — and as an "Uncaught …" console entry, not just a host-side
// log line. Error-monitoring SDKs (Sentry family) install through exactly
// these hooks and otherwise see a silent page. Rust calls the string
// form when a classic script's Err crosses the bridge; the in-JS catch
// sites use the error-object wrapper below to preserve `error`.
globalThis.__diting_reportUncaught = function(msg, src, line, err) {
  try {
    const message = String(msg == null ? '' : msg);
    const source = String(src || '');
    const label = /^Uncaught /.test(message) ? message : 'Uncaught ' + message;
    if (globalThis.__diting_reporting) {
      // An error-event listener (or onerror) threw while we were already
      // reporting: Chrome does not re-enter the error pipeline for it.
      try { console.error(err && err.stack ? err : label); } catch (_) {}
      return;
    }
    globalThis.__diting_reporting = true;
    try {
      // (#37) window/body.onerror write through the reflecting accessor
      // into __windowOnHandlers; the pre-install data prop above is the
      // default capture. Store first, data prop as the default.
      const _hs = globalThis.__windowOnHandlers;
      const _onerr = (_hs && Object.prototype.hasOwnProperty.call(_hs, 'onerror'))
        ? _hs.onerror : globalThis.onerror;
      if (typeof _onerr === 'function') {
        try { _onerr(message, source, line | 0, 0, err || null); } catch (_) {}
      }
      try {
        globalThis.dispatchEvent(new ErrorEvent('error', {
          message: message, filename: source, lineno: line | 0, error: err || null
        }));
      } catch (_) {}
      try {
        if (err && err.stack) console.error(err); else console.error(label);
      } catch (_) {}
    } finally { globalThis.__diting_reporting = false; }
  } catch (_) {}
};
globalThis.__diting_reportUncaughtError = function(err, src) {
  const message = (err && err.message)
    ? ((err.name && err.name !== 'Error' && String(err.message).indexOf(err.name) !== 0)
        ? err.name + ': ' + err.message : String(err.message))
    : String(err);
  var source = src || '';
  var line = 0;
  if (!source && err && err.stack) {
    // Chrome reports the throw site's script URL and line; a callback we
    // invoked runs eval'd code whose frames read "<anonymous>:L:C" (or
    // "url:L:C"). The first stack frame after the message line is the
    // honest analog — better than reporting no position at all.
    var frames = String(err.stack).split('\n');
    for (var i = 1; i < frames.length; i++) {
      var m = /([^\s()]+):(\d+):\d+/.exec(frames[i]);
      if (m) { source = m[1]; line = m[2] | 0; break; }
    }
  }
  globalThis.__diting_reportUncaught(message, source, line, err);
};
// Event state facade: the native Event (crates/diting_runtime/src/native/
// event.rs) keeps its state in a private slot - invisible and unwritable
// from page JS. Its JS bridge `__diting_event_state` lost its last engine
// consumer when the Element and Document dispatch trilogies went native
// (M3 B3/B4) - the core pins dispatch state itself - so the global is now
// deleted at bake purely to keep the native facade out of page sight.
//
// window itself dispatches through the NATIVE EventTarget core now (M3
// step B1): the three own addEventListener/removeEventListener/
// dispatchEvent globals are gone and the chain (window -> Window.prototype
// -> named-access gate -> EventTarget.prototype, see the rewire at the
// _namedHolder) serves them. The window-specific on* legs (__windowOnHandlers
// + <body onX> inline fallback, 'error' pipeline-only) live in the
// `handler` hook in event_hooks.js.
// Platform modules that dispatch UA-originated events (sensor errors, ...) need
// isTrusted=true, which only this facade can set. Keep it as a LEXICAL binding
// (never a global property, so page JS still cannot see it) before the delete.
const _eventState = globalThis.__diting_event_state;
delete globalThis.__diting_event_state;
// (M3 B1) window dispatches through the native EventTarget core: the trio
// installed here IS the native one (the runtime installs it on
// EventTarget.prototype before any JS runs). global_surface.js relocates
// them to the proto layer later, exactly as it did the legacy trio, so
// window keeps no own event-method props on the Chrome face. The
// window-specific on* legs (__windowOnHandlers + <body onX> inline
// fallback, 'error' pipeline-only suppression) ride the native dispatch's
// handler hook (event_hooks.js).
globalThis.addEventListener = globalThis.EventTarget.prototype.addEventListener;
globalThis.removeEventListener = globalThis.EventTarget.prototype.removeEventListener;
globalThis.dispatchEvent = globalThis.EventTarget.prototype.dispatchEvent;

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
const _DOM_MUTATION_COMMANDS = new Set([
  "append_child", "insert_before", "remove_child",
  "set_attribute", "remove_attribute",
  "set_text_content", "set_inner_html", "document_write_reset",
  // Not a tree mutation, but it lands a new stylesheet in the cascade —
  // the getComputedStyle snapshot epoch must see it like any style write.
  "ext_sheet_put",
  // Media emulation and viewport changes re-evaluate @media arms without
  // touching the tree — same staleness class as a new stylesheet, so the
  // gCS snapshot epoch must go stale on them too (issue #29).
  "set_media_env",
  "set_viewport",
  // The form-control dirty value mirror: paint reads it (Rust side drops
  // the layout cache on the same command), so the snapshot epoch must
  // count it too. set_live_checked is the same posture for the
  // checkbox/radio dirty checkedness the widget paint reads.
  "set_live_value",
  "set_live_checked",
  // Focus changes what :focus/:focus-within/:focus-visible rules resolve
  // to — the getComputedStyle snapshot epoch must go stale on it (blitz#839).
  "set_focused",
  // Hover/active likewise (#152): a mousemove that changes the hit target
  // or a press/release flips :hover/:active resolution.
  "set_hover",
  "set_active",
]);
  // #225: tree-edge memo. The only op_dom commands that change
  // parent/child/sibling links are these — textContent and the children
  // swap are built from remove_child/append_child loops, and the layout
  // engine never mutates DomStore edges. Attribute churn does NOT clear
  // it: the doudian storm's currency is set_attribute (dataset writes),
  // and flushing the memo per write would re-pay the op crossing the memo
  // exists to skip. The op costs a string-marshal round-trip per access
  // (arg stringify, Rust parse, result String alloc, JS + conversion) —
  // get parentNode alone ate 79.9% of a wedged boot's CPU.
  const _TREE_EDGE_MUTATION_COMMANDS = new Set([
    "append_child", "insert_before", "remove_child",
    "set_inner_html", "document_write", "document_write_reset",
  ]);
  const _edgeMemo = new Map();
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
const _domStrA1 = new Set([
  "create_element", "create_text_node", "create_comment_node",
  "create_processing_instruction", "create_doctype",
  "create_document_fragment",
  "query_selector", "query_selector_all", "get_element_by_id",
  "document_node_id", "document_title", "set_document_title", "document_referrer", "document_url", "document_base_url", "document_encoding", "document_content_type", "document_referrer_policy", "history_pos", "history_total",
  "document_element", "document_doctype",
  "document_write", "document_write_reset",
  "add_css_transition",
]);
const _domNumA2 = new Set(["append_child", "insert_before", "compare_order"]);
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

[Error, TypeError, ReferenceError, SyntaxError, RangeError, URIError, EvalError].forEach(E => {
  try {
    Object.defineProperty(E.prototype, 'name', {
      value: E.name, writable: true, enumerable: false, configurable: true, // Chrome: -cw
    });
  } catch(e) {}
});

const _stackCache = new WeakMap();
const _origStackDesc = Object.getOwnPropertyDescriptor(Error.prototype, 'stack');
if (_origStackDesc && _origStackDesc.get) {
  Object.defineProperty(Error.prototype, 'stack', {
    configurable: false, enumerable: false,
    get: function() {
      if (!_stackCache.has(this)) _stackCache.set(this, _origStackDesc.get.call(this));
      return _stackCache.get(this);
    }
  });
}

let _fpSeed = 0;
function _fpRand(salt) {
  let h = (_fpSeed ^ (salt || 0)) | 0;
  h = Math.imul(h ^ (h >>> 16), 0x45d9f3b);
  h = Math.imul(h ^ (h >>> 13), 0x45d9f3b);
  return ((h ^ (h >>> 16)) >>> 0) / 0xFFFFFFFF;
}

var _fpCache = null;
// The persona must cohere with the UA the HTTP layer sends. A Windows
// Direct3D11 ANGLE renderer behind a macOS or Linux UA (what the old
// single-pool randomization produced) is a geographic/hardware
// impossibility that risk engines (Castle) score immediately.
function _fpPlatform() {
  const ua = globalThis.__diting_ua || '';
  if (ua.indexOf('Windows') !== -1) return 'win';
  if (ua.indexOf('Macintosh') !== -1 || ua.indexOf('Mac OS X') !== -1) return 'mac';
  if (ua.indexOf('Android') !== -1) return 'linux';
  return 'linux';
}
function _getFp() {
  // The cache is keyed by platform: during the V8 snapshot build no UA is
  // configured yet, so the first materialization defaults to the linux
  // persona. Once __diting_ua is set (per context, before __diting_init),
  // the cache rebuilds for the real platform instead of serving the frozen
  // snapshot-time one (which put Mesa GL strings behind a macOS UA).
  if (_fpCache && _fpCache.platform === _fpPlatform()) return _fpCache;
  const plat = _fpPlatform();
  // Per-platform GPU pools — vendor string and renderer must describe the
  // same machine the UA describes.
  const pools = {
    win: {
      gpu: [
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 2070 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (AMD, AMD Radeon RX 580 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (AMD, AMD Radeon RX 6700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce GTX 1080 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (Intel, Intel(R) UHD Graphics 770 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (AMD, AMD Radeon RX 5700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)',
      ],
      // Device coherence (fp-diff-1014 P1): [arch, cores] per GPU entry, so
      // ANGLE Apple M3 can never report architecture x86 or 12 cores — a
      // detector cross-checking UNMASKED_RENDERER vs getHighEntropyValues
      // vs hardwareConcurrency saw three different machines before.
      hw: [
        ['x86', 12], ['x86', 12], ['x86', 16], ['x86', 8], ['x86', 8],
        ['x86', 16], ['x86', 12], ['x86', 16], ['x86', 16], ['x86', 12],
        ['x86', 16], ['x86', 16],
      ],
      // RAM options per GPU entry, already in deviceMemory shape (Chrome
      // rounds actual GiB down to a power of 2; budget boxes 8/16, gaming
      // rigs 16/32 — see the P8 note by __diting_setPersona).
      mem: [
        [16, 32], [16, 32], [16, 32], [8, 16], [8, 16],
        [8, 16], [16, 32], [16, 32], [16, 32], [16, 32],
        [16, 32], [16, 32],
      ],
      vendor: [
        'Google Inc. (NVIDIA)','Google Inc. (NVIDIA)','Google Inc. (NVIDIA)',
        'Google Inc. (Intel)','Google Inc. (Intel)',
        'Google Inc. (AMD)','Google Inc. (AMD)',
        'Google Inc. (NVIDIA)','Google Inc. (NVIDIA)',
        'Google Inc. (Intel)','Google Inc. (AMD)','Google Inc. (NVIDIA)',
      ],
      // UA-CH platformVersion per OS family (mac = Sequoia 15.2, win = 11 23H2,
      // linux = Ubuntu 24.04 kernel line).
      platformVersion: '15.0.0',
      screens: [[1920,1080],[2560,1440],[1366,768],[1536,864],[1440,900],[1680,1050],[1280,720],[3840,2160]],
      dprs: [1,1,1,1.25,1,1,1,1],
    },
    mac: {
      gpu: [
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M3 Pro, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Version)',
        'ANGLE (Intel, Intel(R) Iris(TM) Plus Graphics 645, OpenGL 4.1)',
      ],
      // M1/M2/M3 = 4P+4E, M2 Pro = 6P+4E, M3 Pro = 6P+6E, M4 = 4P+6E,
      // Iris 645 = 2018 MacBook Air (i5-8210Y, 2c/4t → 4).
      hw: [
        ['arm', 8], ['arm', 8], ['arm', 10], ['arm', 8], ['arm', 12],
        ['arm', 10], ['x86', 4],
      ],
      // RAM options per chip, in deviceMemory shape (GiB rounded down to a
      // power of 2 — 18 GB M3 Pro reports 16, 36 GB reports 32, 24 GB M4
      // reports 16). P8: an M3 Pro persona claiming 8 GB describes a machine
      // Apple never sold — the exact class of cross-field lie 300015 hunts.
      mem: [
        [8, 16], [8, 16], [16, 32], [8, 16], [16, 32],
        [16, 32], [8, 16],
      ],
      vendor: [
        'Google Inc. (Apple)','Google Inc. (Apple)','Google Inc. (Apple)',
        'Google Inc. (Apple)','Google Inc. (Apple)','Google Inc. (Apple)',
        'Google Inc. (Intel)',
      ],
      platformVersion: '15.2.0',
      screens: [[1512,982],[1728,1117],[1440,900],[1920,1080],[2560,1440],[1170,2532]],
      // Built-in retina panels run at 2x; the two external sizes run at 1x.
      dprs: [2,2,2,1,1,3],
    },
    linux: {
      gpu: [
        'ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CML GT2), OpenGL 4.6 (Core Profile) Mesa 23.2.1)',
        'ANGLE (Intel, Mesa Intel(R) Iris(R) Xe Graphics (TGL GT2), OpenGL 4.6 (Core Profile) Mesa 23.2.1)',
        'ANGLE (AMD, AMD Radeon RX 6700 XT (navi22, LLVM 15.0.7, DRM 3.54, LLVM 15.0.7), OpenGL 4.6 (Core Profile) Mesa 23.2.1)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060/PCIe/SSE2, OpenGL 4.6.0 NVIDIA 535.183.01)',
      ],
      hw: [['x86', 12], ['x86', 8], ['x86', 16], ['x86', 12]],
      mem: [[8, 16], [8, 16], [16, 32], [16, 32]],
      vendor: [
        'Google Inc. (Intel)','Google Inc. (Intel)','Google Inc. (AMD)','Google Inc. (NVIDIA)',
      ],
      platformVersion: '6.8.0',
      screens: [[1920,1080],[2560,1440],[1366,768],[3840,2160],[1680,1050]],
      dprs: [1,1,1,1,1],
    },
  };
  const pool = pools[plat];
  const idx = Math.floor(_fpRand(42) * pool.gpu.length);
  const sIdx = Math.floor(_fpRand(300) * pool.screens.length);
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  let cfp = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUg';
  for (let i = 0; i < 40; i++) cfp += chars[Math.floor(_fpRand(500 + i) * 64)];
  cfp += '==';
  _fpCache = {
    platform: plat,
    gpu: pool.gpu[idx], gpuVendor: pool.vendor[idx],
    arch: pool.hw[idx][0], hw: pool.hw[idx][1], bitness: '64',
    mem: pool.mem[idx][Math.floor(_fpRand(401) * pool.mem[idx].length)],
    platformVersion: pool.platformVersion,
    screen: pool.screens[sIdx], dpr: pool.dprs[sIdx],
    audioBaseLatency: 0.002 + _fpRand(100) * 0.008,
    audioSampleRate: [44100, 48000][Math.floor(_fpRand(101) * 2)],
    compThreshold: -24 + (_fpRand(102) - 0.5) * 4,
    compKnee: 30 + (_fpRand(103) - 0.5) * 4,
    compRatio: 12 + (_fpRand(104) - 0.5) * 4,
    batteryLevel: 0.5 + _fpRand(200) * 0.5,
    batteryCharging: _fpRand(201) > 0.3,
    canvasFingerprint: cfp,
  };
  return _fpCache;
}
function _fp(key) { return _getFp()[key]; }
// (M3 B3/B4) The `_eventRegistry` / `_eventRegistryCap` buckets and their
// helpers went with the Element and Document dispatch trilogies — listener
// storage is the native core's private slot now (`diting#et-listeners`,
// crates/diting_runtime/src/native/event_target.rs).

const _domParse = (cmd, a1, a2) => { try { return JSON.parse(_dom(cmd, a1, a2)); } catch { return null; } };

const _consoleFn = (level, args) => {
  try {
    let joined = args.map(a => _fmtVal(a, 0, new WeakSet())).join(" ");
    if (joined.length > _FMT_MAX) joined = joined.slice(0, _FMT_MAX) + "\u2026 (truncated " + (joined.length - _FMT_MAX) + " chars)";
    _OPS.op_console_msg(level, joined);
  } catch {}
  if (level === "error") {
    try {
      globalThis.__diting_errors = globalThis.__diting_errors || [];
      globalThis.__diting_errors.push({msg: args.map(a => {
        if (a !== null && typeof a === "object") {
          const st = _peek(a, "stack");
          if (typeof st === "string" && st) return st;
          const m = _peek(a, "message");
          if (typeof m === "string" && m) return m;
          return _fmtVal(a, 0, new WeakSet());
        }
        return String(a);
      }).join(" ")});
    } catch {}
  }
};

globalThis.console = {
  log: (...a) => _consoleFn("log", a), warn: (...a) => _consoleFn("warn", a),
  error: (...a) => _consoleFn("error", a), info: (...a) => _consoleFn("log", a),
  debug: (...a) => _consoleFn("debug", a),
  dir: () => {}, trace: () => {}, table: () => {}, group: () => {},
  groupEnd: () => {}, groupCollapsed: () => {}, time: () => {}, timeEnd: () => {},
  timeLog: () => {}, count: () => {}, countReset: () => {}, clear: () => {},
  assert: (c, ...a) => { if (!c) _consoleFn("error", ["Assertion failed:", ...a]); },
  createTask(name) {
    // Chrome 130+ Console.createTask: a task carries a stack capture for
    // async continuation attribution. The shape (object with run()) is what
    // library detectors probe; the stack metadata is inert here.
    if (arguments.length === 0) throw new TypeError("Failed to execute 'createTask' on 'Console': 1 argument required, but only 0 present.");
    return {
      name: String(name),
      run(fn, ...args) {
        if (typeof fn !== "function") throw new TypeError("Failed to execute 'run' on 'ConsoleTask': parameter 1 is not of type 'Function'.");
        try { return fn.apply(undefined, args); }
        catch (e) { globalThis.__diting_reportUncaughtError(e); }
      },
    };
  },
  context(label) {
    // Chrome-only console.context([label]) → ConsoleContext: an object whose
    // console methods run under the label. Detectors probe its presence;
    // label routing is inert here, so methods delegate to plain console.
    const ctx = { label: label === undefined ? "" : String(label) };
    for (const m of ["log", "info", "warn", "error", "debug", "dir", "table",
      "trace", "group", "groupEnd", "groupCollapsed", "time", "timeLog",
      "timeEnd", "count", "countReset", "clear", "assert"]) {
      if (typeof this[m] === "function") ctx[m] = this[m].bind(this);
    }
    return ctx;
  },
};

// DOMStringMap backs Element.dataset. Chrome throws on `new DOMStringMap()`;
// the construction key keeps instances engine-only (upstream ec05ed0).
const _domStringMapKey = {};
class DOMStringMap {
  constructor(key) {
    if (key !== _domStringMapKey) {
      throw new TypeError("Failed to construct 'DOMStringMap': Illegal constructor");
    }
  }
  get [Symbol.toStringTag]() { return "DOMStringMap"; }
}

const _styleProxy = (decl) => new Proxy(decl, {
  get(t, p) {
    if (typeof p === "symbol" || p in t) return t[p];
    if (/^\d+$/.test(p)) return t.item(+p);
    return t.getPropertyValue(p);
  },
  set(t, p, v) {
    if (typeof p === "symbol") { t[p] = v; return true; }
    if (p === "cssText") { t.cssText = v; return true; }
    if (/^\d+$/.test(p)) return true;
    if (p in Object.getPrototypeOf(t)) {
      // (刀75) prototype ACCESSORS with a setter (cssFloat) run it against the declaration;
      // every other prototype member (methods, readonly names) stays a silent no-op.
      for (let o = Object.getPrototypeOf(t); o; o = Object.getPrototypeOf(o)) {
        const d = Object.getOwnPropertyDescriptor(o, p);
        if (d) { if (d.set) d.set.call(t, v); break; }
      }
      return true;
    }
    t.setProperty(p, v);
    return true;
  },
  has(t, p) {
    if (typeof p !== "string") return Reflect.has(t, p);
    if (p in Object.getPrototypeOf(t)) return true;
    t._pull();
    if (_cssCamelToKebab(p) in t._props) return true;
    if (_CSS_PROP_SET.has(p) || _CSS_PROP_SET.has(_cssKebabToCamel(p))) return true;
    return /^\d+$/.test(p) && +p < t.length;
  },
  ownKeys(t) {
    t._pull();
    const keys = [];
    const n = t.length;
    for (let i = 0; i < n; i++) keys.push(String(i));
    const names = new Set(_CSS_PROPERTY_NAMES);
    for (const k of Object.keys(t._props)) names.add(_cssKebabToCamel(k));
    for (const name of names) keys.push(name);
    return keys;
  },
  getOwnPropertyDescriptor(t, p) {
    if (typeof p !== "string") return Reflect.getOwnPropertyDescriptor(t, p);
    t._pull();
    if (/^\d+$/.test(p) && +p < t.length) return { value: t.item(+p), writable: false, enumerable: true, configurable: true };
    if (_cssCamelToKebab(p) in t._props || _CSS_PROP_SET.has(p) || _CSS_PROP_SET.has(_cssKebabToCamel(p))) {
      return { value: t.getPropertyValue(p), writable: true, enumerable: !/^epub/.test(p), configurable: true };
    }
    return undefined;
  },
});

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

// Prepare a single script element for execution, unless it has already
// started. The native flag (op_script_try_start) is authoritative and survives
// moves and cloneNode(), so a script inserted a second time runs no more than
// once (upstream 41a8e1c).
// Decode a data: URL whose payload is JavaScript (upstream 0c4740a + f841205
// final form). op_fetch_url's HTTP client cannot fetch the data: scheme, so
// dynamic <script src="data:text/javascript,..."> never ran. Chromium accepts
// any MIME type here (the fetch layer is bypassed entirely), rejects stray
// padding in base64, and handles %-escapes + non-ASCII by re-encoding through
// UTF-8 — matching that shape byte-for-byte avoids a fidelity gap pages can
// probe. _hexv is defined in url_search_params.js; resolution happens at call time.
function _decodeDataScriptUrl(url) {
  const comma = url.indexOf(',');
  if (!url.startsWith('data:') || comma < 5) {
    throw new TypeError('Invalid dynamic script data URL');
  }

  const meta = url.slice(5, comma);
  const fragment = url.indexOf('#', comma + 1);
  const payload = url.slice(comma + 1, fragment < 0 ? url.length : fragment);
  if (meta.split(';').some(part => part.toLowerCase() === 'base64')) {
    let encoded = payload.replace(/[\r\n\t\f ]/g, '');
    const remainder = encoded.length % 4;
    if (remainder === 1 || !/^[A-Za-z0-9+/]*={0,2}$/.test(encoded) || /=/.test(encoded.slice(0, -2))) {
      throw new TypeError('Invalid dynamic script data URL base64');
    }
    if (remainder > 0) encoded += '='.repeat(4 - remainder);
    if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(encoded)) {
      throw new TypeError('Invalid dynamic script data URL base64');
    }
    return new TextDecoder().decode(_base64ToUint8Array(encoded));
  }

  const bytes = [];
  for (let i = 0; i < payload.length; i++) {
    const code = payload.charCodeAt(i);
    if (code === 0x25 && i + 2 < payload.length) {
      const hi = _hexv(payload.charCodeAt(i + 1));
      const lo = _hexv(payload.charCodeAt(i + 2));
      if (hi >= 0 && lo >= 0) {
        bytes.push(hi * 16 + lo);
        i += 2;
        continue;
      }
    }
    if (code < 0x80) {
      bytes.push(code);
    } else {
      const character = String.fromCodePoint(payload.codePointAt(i));
      if (character.length === 2) i++;
      const encoded = new TextEncoder().encode(character);
      for (let j = 0; j < encoded.length; j++) bytes.push(encoded[j]);
    }
  }
  return new TextDecoder().decode(new Uint8Array(bytes));
}

// A valid form submitter: <button> that is not reset/button, or
// <input type=submit|image>. Shared by requestSubmit's argument validation
// and the internal click path, so a click can never hand requestSubmit a
// submitter it would reject (upstream 7e2cabf/ccfa5fb).
function _isSubmitButton(el) {
  if (!el || typeof el.localName !== "string") return false;
  const type = ((el.getAttribute && el.getAttribute("type")) || "").toLowerCase();
  if (el.localName === "button") return type !== "reset" && type !== "button";
  if (el.localName === "input") return type === "submit" || type === "image";
  return false;
}

// Labelable elements per the HTML spec: button, input (except type=hidden),
// meter, output, progress, select, textarea. A <label> activates its labeled
// control: the element referenced by `for`, else the first labelable descendant.
const _LABELABLE = 'button,input:not([type=hidden]),meter,output,progress,select,textarea';
function _labeledControl(label) {
  if (!label || label.tagName !== 'LABEL') return null;
  // A present `for` attribute means association by id only; an empty value
  // associates nothing (no fallback to a descendant).
  const forId = label.getAttribute ? label.getAttribute('for') : null;
  if (forId !== null && forId !== undefined) {
    if (forId === '') return null;
    const doc = label.ownerDocument || globalThis.document;
    const el = doc && doc.getElementById ? doc.getElementById(forId) : null;
    if (!el) return null;
    return el.matches && el.matches(_LABELABLE) ? el : null;
  }
  return label.querySelector ? label.querySelector(_LABELABLE) : null;
}

// Activation state for a checkable input's legacy pre-click flip: the
// checkbox/radio state changes BEFORE the click event dispatches (listeners
// observe the new state) and reverts if the event is cancelled. Shared by
// Element.click() and by unmanaged synthetic dispatches through
// dispatchEvent() — Chrome runs activation behavior for untrusted clicks too,
// so `cb.dispatchEvent(new MouseEvent('click'))` toggles like cb.click() does.

// Trusted CDP click: the Rust Input domain drives the same activation
// sequence as Element.click() but with a positioned, trusted event object.
// input/change events fired here are trusted-marked like the click itself.
globalThis.__diting_dispatchTrustedClick = function (target, event, tripleSelect) {
  if (!target) return false;
  if (target._ditingClickInProgress) return true;
  target._ditingClickInProgress = true;
  try {
    const flip = _preClickFlip(target);
    const cancelled = !target.dispatchEvent(event);
    if (cancelled) {
      _cancelPreClickFlip(flip);
      return false;
    }
    const mk = globalThis.__diting_markTrusted || function (e) { return e; };
    if (flip && flip.el.checked !== flip.oldChecked) {
      try { flip.el.dispatchEvent(mk(new Event('input', {bubbles: true}))); } catch (e) {}
      try { flip.el.dispatchEvent(mk(new Event('change', {bubbles: true}))); } catch (e) {}
      return true;
    }
    const activated = _postClickActivation(target);
    if (!activated && tripleSelect) {
      const tag = target.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') {
        const len = target.value ? target.value.length : 0;
        if (target.setSelectionRange) target.setSelectionRange(0, len);
        else { target.selectionStart = 0; target.selectionEnd = len; }
      }
    }
    return true;
  } finally {
    target._ditingClickInProgress = false;
  }
};

// The first <legend> child of a fieldset — the only legend whose descendants
// a disabled fieldset does NOT disable (HTML spec: "the first legend child").
function _firstLegend(fieldset) {
  const kids = fieldset.children;
  for (let i = 0; i < kids.length; i++) if (kids[i].tagName === 'LEGEND') return kids[i];
  return null;
}

// A control inside <fieldset disabled> is disabled unless the path from it to
// the fieldset crosses that fieldset's first <legend>. Walk ancestors; at each
// disabled fieldset on the path, the child we came through must be its first
// legend or the control is fieldset-disabled.
function _fieldsetDisabled(el) {
  let node = el;
  while (node) {
    const p = node.parentNode;
    if (!p || !p.tagName) break;
    if (p.tagName === 'FIELDSET' && p.hasAttribute && p.hasAttribute('disabled')) {
      if (!(node.tagName === 'LEGEND' && node === _firstLegend(p))) return true;
    }
    node = p;
  }
  return false;
}

// Actually-disabled per spec: a form control with its own disabled attribute,
// or any form control disabled by an ancestor fieldset. Disabled controls have
// no activation behaviour and dispatch no click event at all.
function _isFormControlDisabled(el) {
  const t = el.tagName;
  if (t !== 'INPUT' && t !== 'BUTTON' && t !== 'SELECT' && t !== 'TEXTAREA') return false;
  if ((el.hasAttribute && el.hasAttribute('disabled')) || el.disabled) return true;
  return _fieldsetDisabled(el);
}

// Clear focus and fire the leaving half of the focus event family
// (blur non-bubbling, focusout bubbling). Shared by el.blur() and the
// switch-away leg of el.focus() — Chrome runs blur+focusout on the old
// holder before focus+focusin on the new one.
function _fireBlurFamily(el) {
  globalThis.__diting_focused = null;
  if (el && el._nid !== undefined) _domRaw("set_focused", String(el._nid), "0");
  if (!el) return;
  el.dispatchEvent(new FocusEvent("blur"));
  el.dispatchEvent(new FocusEvent("focusout", {bubbles: true}));
}

// Hover/active tree mirrors (#152). The interaction layer (INPUT_HELPERS,
// eval'd per interaction) owns the pointer markers and the transition
// events; these bridges carry the same facts through _domRaw so the
// mutation epoch — and with it the style snapshot — goes stale on every
// hover/active move. Two-arg shape keeps them stateless: setting wins,
// clearing names the node it is leaving (the Rust arm is identity-guarded,
// like set_focused's clear leg).
globalThis.__diting_setHoverTree = function (prevEl, nextEl) {
  if (nextEl && nextEl._nid !== undefined) _domRaw("set_hover", String(nextEl._nid), "1");
  else if (prevEl && prevEl._nid !== undefined) _domRaw("set_hover", String(prevEl._nid), "0");
};
globalThis.__diting_setActiveTree = function (prevEl, nextEl) {
  if (nextEl && nextEl._nid !== undefined) _domRaw("set_active", String(nextEl._nid), "1");
  else if (prevEl && prevEl._nid !== undefined) _domRaw("set_active", String(prevEl._nid), "0");
};

function _resolveUrl(url) {
  // Coerce up front: a URL object passed to location.href/assign/replace has
  // no .startsWith and would throw here (upstream fe26417).
  url = String(url);
  if (!url) return url;
  if (url.startsWith('http://') || url.startsWith('https://') || url.startsWith('about:')) return url;
  try { return new URL(url, _docBase()).href; } catch(e) { return url; }
}
// `__virtualUrl` is set by `history.pushState`/`replaceState` (and cleared by
// any real navigation). When set, `location.href` and friends read it instead
// of the underlying `document_url`. Without this, client-side routers
// (Next.js, React Router, vue-router) call `pushState` but the URL never
// changes, so their `useLocation` hooks return the wrong path and the UI
// freezes on the original route.
globalThis.__virtualUrl = null;
function __currentUrl() {
  return globalThis.__virtualUrl || _domParse("document_url") || "about:blank";
}
// Parsed-URL memo for the Location part getters (10-27 复盘: heavy pages
// read location.origin/protocol/host thousands of times; each read used to
// pay a fresh `new URL(this.href)` AND — via __currentUrl — a document_url
// op round-trip, so the parts re-parsed a URL that hadn't changed). The
// memo keys on the href string itself: navigation swaps the string, the
// entry rolls. Part setters are absent on Location (assign-only API), so
// the parsed snapshot can't go stale within one href.
let __locMemoKey = null, __locMemoURL = null;
function __locParts() {
  const href = __currentUrl();
  if (href !== __locMemoKey) {
    try { __locMemoURL = new URL(href); } catch (e) { __locMemoURL = null; }
    __locMemoKey = href;
  }
  return __locMemoURL;
}
globalThis.location = {
  get href() { return __currentUrl(); },
  set href(url) { var r = _resolveUrl(url); globalThis.__virtualUrl = r; _OPS.op_navigate(r, 'GET', ''); },
  get origin() { var u = __locParts(); return u ? u.origin : ""; },
  get protocol() { var u = __locParts(); return u ? u.protocol : ""; },
  get host() { var u = __locParts(); return u ? u.host : ""; },
  get hostname() { var u = __locParts(); return u ? u.hostname : ""; },
  get pathname() { var u = __locParts(); return u ? u.pathname : "/"; },
  get search() { var u = __locParts(); return u ? u.search : ""; },
  get hash() { var u = __locParts(); return u ? u.hash : ""; },
  get port() { var u = __locParts(); return u ? u.port : ""; },
  get fragmentDirective() { return __diting_fragmentDirective; },
  toString() { return this.href; },
  assign(url) { var r = _resolveUrl(url); globalThis.__virtualUrl = r; _OPS.op_navigate(r, 'GET', ''); },
  reload() { var r = _resolveUrl(this.href); globalThis.__virtualUrl = r; _OPS.op_navigate(r, 'GET', ''); },
  replace(url) { var r = _resolveUrl(url); globalThis.__virtualUrl = r; _OPS.op_navigate(r, 'GET', ''); },
};
const _locationObj = globalThis.location;
Object.defineProperty(globalThis, 'location', {
  // __diting_frame_location is the frame-eval swap slot: while the engine
  // brackets an evaluation into an iframe's globals, reads of `location`
  // must see the frame's location. The property itself is non-configurable
  // (matching Chrome's unforgeable Location), so the swap goes through this
  // data slot instead of redefining the accessor.
  get() { return globalThis.__diting_frame_location || _locationObj; },
  set(url) { globalThis.__diting_locNav(url, false); },
  configurable: false,
  enumerable: true,
});

// Secure Contexts (W3C §is-origin-potentially-trustworthy): https/wss are
// secure, plain http/ws only on the localhost family, data:/file:/blob:
// inheriting a secure scheme. Computed per access off the live URL — the
// property can't be baked into the snapshot because navigation changes the
// answer. Real pages check this before generating risk params (Tmall's AWSC
// prints "未使用 HTTPS" and walks its degraded branch when it reads
// undefined), so the getter must exist on window, not just the worker scope.
Object.defineProperty(globalThis, 'isSecureContext', {
  get() {
    let u;
    try { u = new URL(__currentUrl()); } catch { return false; }
    if (u.protocol === 'https:' || u.protocol === 'wss:') return true;
    if (u.protocol === 'data:' || u.protocol === 'file:') return true;
    if (u.protocol === 'blob:') {
      try { return new URL(u.protocol === 'blob:' ? u.href.slice(5) : u.href).protocol === 'https:'; }
      catch { return false; }
    }
    if (u.protocol === 'http:' || u.protocol === 'ws:') {
      const h = u.hostname;
      return h === 'localhost' || h.endsWith('.localhost')
        || h === '::1' || h === '[::1]'
        || /^127\./.test(h) || h === '0.0.0.0';
    }
    return false;
  },
  configurable: true,
  enumerable: true,
});

globalThis.window = globalThis;
globalThis.self = globalThis;
globalThis.top = globalThis;
globalThis.parent = globalThis;
globalThis.frames = globalThis;
globalThis.frameElement = null;
globalThis.length = 0;

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

function _nodeInDocument(node) {
  let cur = node;
  let steps = 0;
  while (cur && steps++ < 200) {
    if (cur.nodeType === 9) return true;
    cur = cur.parentNode;
  }
  return false;
}
// XMLHttpRequestEventTarget — spec-required ancestor for XHR's EventTarget
// face. M3 step B2: the trio resolves through the chain to the NATIVE core
// (XHRET.prototype → EventTarget.prototype), exactly Chrome's chain — no
// own dispatch code remains. zone.js walks XHRET.prototype for the trio
// descriptors and falls through to EventTarget.prototype, where the native
// methods live. The on* handler IDL attributes are installed as prototype
// accessors by xhr.js (`__acc`) so the core's handler leg fires them.
class XMLHttpRequestEventTarget extends EventTarget {}
globalThis.XMLHttpRequestEventTarget = XMLHttpRequestEventTarget;
_markNative(XMLHttpRequestEventTarget);

// Decode a response body honoring the Content-Type charset, so fetch()/XHR
// over non-UTF-8 resources (GBK, Shift_JIS, ISO-8859-x, ...) return correctly
// decoded text instead of mojibake. The UTF-8 case (the overwhelming majority)
// takes the plain TextDecoder fast path; only an explicit non-UTF-8 charset
// routes through TextDecoder(label), which falls back to UTF-8 on a bad label.
function _decodeBodyWithCharset(bytes, headers) {
  let label = '';
  try {
    const ct = headers && typeof headers.get === 'function' ? (headers.get('content-type') || '') : '';
    const m = /charset\s*=\s*"?([^";]+)"?/i.exec(ct);
    if (m) label = m[1].trim();
  } catch (e) {}
  if (!label || /^utf-?8$/i.test(label)) return new TextDecoder().decode(bytes);
  try { return new TextDecoder(label).decode(bytes); }
  catch (e) { return new TextDecoder().decode(bytes); }
}


globalThis.ResizeObserver = class ResizeObserver {
  constructor(callback) {
    this._callback = callback;
    this._targets = new Set();
    this._connected = true;
    this._fireCount = 0;
    __hideOwn(this);
  }
  _fireFor(targets) {
    if (!this._connected || !targets.length) return;
    const records = targets.map(target => {
      const r = target.getBoundingClientRect ? target.getBoundingClientRect() : { x: 0, y: 0, width: 100, height: 20 };
      return {
        target,
        contentRect: { x: r.x || 0, y: r.y || 0, width: r.width || 100, height: r.height || 20, top: r.top || 0, left: r.left || 0, bottom: r.bottom || 20, right: r.right || 100 },
        borderBoxSize: [{ blockSize: r.height || 20, inlineSize: r.width || 100 }],
        contentBoxSize: [{ blockSize: r.height || 20, inlineSize: r.width || 100 }],
        devicePixelContentBoxSize: [{ blockSize: r.height || 20, inlineSize: r.width || 100 }],
      };
    });
    try { this._callback(records, this); } catch (e) { globalThis.__diting_reportUncaughtError(e); }
  }
  observe(el) {
    if (!el || !this._connected) return;
    if (this._targets.has(el)) return;
    this._targets.add(el);
    Promise.resolve().then(() => this._fireFor([el]));
    [200, 800].forEach(delay => {
      setTimeout(() => {
        if (this._connected && this._targets.has(el) && this._fireCount < 16) {
          this._fireCount++;
          this._fireFor([el]);
        }
      }, delay);
    });
  }
  unobserve(el) { this._targets.delete(el); }
  disconnect() { this._connected = false; this._targets.clear(); }
  // 刀57: takeRecords deleted — Chrome's ResizeObserver.prototype carries
  // only observe/unobserve/disconnect; pending-record draining is a
  // MutationObserver/PerformanceObserver face, not this one.
};

// matchMedia evaluates against the live window viewport — the same
// innerWidth/innerHeight the persona publishes and set_viewport feeds the
// layout ICB — so a page's JS branching agrees with the @media rules the CSS
// cascade already applied. The old always-false stub answered
// `(min-width:640px)` with false while a 2560px viewport was published:
// wrong for responsive pages and a self-contradiction any fingerprint
// script could catch by cross-checking the two. Covers what page scripts
// actually branch on: width/height bounds, orientation, aspect-ratio,
// resolution, and the preference features pinned to the desktop-light
// persona. Comma (or) / and / not follow the classic grammar; unknown
// features evaluate false, matching Chrome's answer for unsupported ones.
const _MQ_PERSONA_BOOL = {
  'prefers-color-scheme': { light: true, dark: false },
  'prefers-reduced-motion': { 'no-preference': true, reduce: false },
  'prefers-reduced-transparency': { 'no-preference': true, reduce: false },
  'color-gamut': { srgb: true, p3: false, rec2020: false },
  'dynamic-range': { standard: true, high: false },
  'display-mode': { browser: true, standalone: false, 'minimal-ui': false, fullscreen: false, 'window-controls-overlay': false },
  pointer: { fine: true, coarse: false, none: false },
  'any-pointer': { fine: true, coarse: false, none: false },
  hover: { hover: true, none: false },
  'any-hover': { hover: true, none: false },
};
// What a mobile viewport override pins the interaction features to (and the
// phone-shaped preference defaults). Kept as a table so the mobile override
// and the emulated-media override can merge per feature in __diting_mqRecompute.
const _MQ_MOBILE_TABLE = {
  'prefers-color-scheme': { light: true, dark: false },
  'prefers-reduced-motion': { 'no-preference': true, reduce: false },
  'prefers-reduced-transparency': { 'no-preference': true, reduce: false },
  'color-gamut': { srgb: true, p3: false, rec2020: false },
  'dynamic-range': { standard: true, high: false },
  'display-mode': { browser: true, standalone: false, 'minimal-ui': false, fullscreen: false, 'window-controls-overlay': false },
  pointer: { coarse: true, fine: false, none: false },
  'any-pointer': { coarse: true, fine: false, none: false },
  hover: { none: true, hover: false },
  'any-hover': { none: true, hover: false },
};
// What touch emulation (CDP Emulation.setTouchEmulationEnabled) pins — a
// NARROWER table than the mobile viewport: touch changes the pointer/hover
// story only, never the prefers-* preferences or display-mode (#151).
const _MQ_TOUCH_TABLE = {
  pointer: { coarse: true, fine: false, none: false },
  'any-pointer': { coarse: true, fine: false, none: false },
  hover: { none: true, hover: false },
  'any-hover': { none: true, hover: false },
};
// Fold the mobile table, the touch table, and the emulated-media table (CDP
// Emulation.setEmulatedMedia / Playwright page.emulateMedia) into the one
// truth table _mqExpr reads. Later layers replace a feature's whole table —
// Chrome's rule that the emulated value is THE value — so
// prefers-reduced-motion: reduce survives even under a mobile override.
globalThis.__diting_mqRecompute = function () {
  const mobile = globalThis.__diting_mq_mobile, touch = globalThis.__diting_mq_touch, emu = globalThis.__diting_mq_emulated;
  if (!mobile && !touch && !emu) { globalThis.__diting_mq_overrides = null; return; }
  const t = {};
  if (mobile) for (const k in mobile) t[k] = mobile[k];
  if (touch) for (const k in touch) t[k] = touch[k];
  if (emu) for (const k in emu) t[k] = emu[k];
  globalThis.__diting_mq_overrides = t;
};
function _mqExpr(inner) {
  const colon = inner.indexOf(':');
  const feature = (colon < 0 ? inner : inner.slice(0, colon)).trim().toLowerCase();
  const value = colon < 0 ? null : inner.slice(colon + 1).trim().toLowerCase();
  const ov = globalThis.__diting_mq_overrides;
  if (ov && feature in ov) return value != null && ov[feature][value] === true;
  if (feature in _MQ_PERSONA_BOOL) return value != null && _MQ_PERSONA_BOOL[feature][value] === true;
  if (value == null) return false;
  const num = parseFloat(value);
  const vw = globalThis.innerWidth || 0, vh = globalThis.innerHeight || 0;
  switch (feature) {
    case 'min-width': return num <= vw;
    case 'max-width': return num >= vw;
    case 'width': return num === vw;
    case 'min-height': return num <= vh;
    case 'max-height': return num >= vh;
    case 'height': return num === vh;
    case 'orientation': return value === (vw >= vh ? 'landscape' : 'portrait');
    case 'aspect-ratio': case 'min-aspect-ratio': case 'max-aspect-ratio': {
      const m = value.match(/^(\d*\.?\d+)\s*\/\s*(\d*\.?\d+)$/);
      if (!m) return false;
      const want = parseFloat(m[1]) / parseFloat(m[2]), have = vw / vh;
      return feature === 'aspect-ratio' ? Math.abs(have - want) < 1e-6
        : feature === 'min-aspect-ratio' ? have >= want : have <= want;
    }
    case 'resolution': case 'min-resolution': case 'max-resolution': {
      // Normalize the value to device px per css px (dppx) and compare
      // against the persona's devicePixelRatio.
      let dppx;
      if (/dppx|x$/.test(value)) dppx = num;
      else if (/dpcm$/.test(value)) dppx = (num * 96) / 2.54;
      else dppx = num / 96; // dpi; a unitless number is invalid CSS, dpi is the least-wrong read
      const dpr = globalThis.devicePixelRatio || 1;
      return feature === 'resolution' ? Math.abs(dppx - dpr) < 1e-6
        : feature === 'min-resolution' ? dppx <= dpr : dppx >= dpr;
    }
    default: return false;
  }
}
function _mqClause(clause) {
  let s = clause.trim(), invert = false;
  if (/^not\s+/i.test(s)) { invert = true; s = s.replace(/^not\s+/i, ''); }
  s = s.replace(/^only\s+/i, '');
  const parts = s.split(/\s+and\s+/i);
  let i = 0;
  if (parts[0] && parts[0][0] !== '(') {
    const mt = parts[0].toLowerCase();
    // The media type token follows the emulated type when one is active
    // (setEmulatedMedia media:'print'), the persona's screen otherwise.
    const cur = globalThis.__diting_mq_media_type || 'screen';
    if (mt !== 'all' && mt !== cur) return invert;
    i = 1;
  }
  for (; i < parts.length; i++) {
    const p = parts[i].trim();
    if (!p) continue;
    if (!(p[0] === '(' && p[p.length - 1] === ')') || !_mqExpr(p.slice(1, -1))) return invert;
  }
  return !invert;
}
function _mqMatches(q) {
  const s = String(q).trim();
  if (s === '') return true;
  const clauses = [];
  let depth = 0, cur = '';
  for (const ch of s) {
    if (ch === '(') depth++;
    else if (ch === ')') depth--;
    if (ch === ',' && depth === 0) { clauses.push(cur); cur = ''; continue; }
    cur += ch;
  }
  clauses.push(cur);
  return clauses.some((c) => c.trim() !== '' && _mqClause(c));
}
// Live MediaQueryList (issue #25, obscura#1007 same lineage): `matches`
// is a getter re-evaluating against the current viewport/overrides, and
// listeners live on the object. __diting_setViewport /
// __diting_clearViewport flush the registry afterwards and fire `change`
// (carrying matches/media) on every object that crossed — the half
// responsive pages need beyond reading matches once at boot.
// const, not a function declaration: a top-level declaration would mint a NON-configurable window property (Chrome's
// MediaQueryList is a normal configurable interface global; it is installed explicitly below).
const MediaQueryList = function MediaQueryList(q) {
  this._q = String(q);
  this._last = false;
  try { this._last = _mqMatches(this._q); } catch (e) {}
  this.onchange = null;
};
// Read-only prototype attributes (Chrome: no instance own properties, no setters).
Object.defineProperties(MediaQueryList.prototype, {
  matches: { get: _markNative({ get matches() { try { return _mqMatches(this._q); } catch (e) { return false; } } }.__lookupGetter__('matches')), set: undefined, enumerable: true, configurable: true },
  media: { get: _markNative({ get media() { return this._q; } }.__lookupGetter__('media')), set: undefined, enumerable: true, configurable: true },
});
MediaQueryList.prototype.addListener = function (fn) { this.addEventListener('change', fn); };
MediaQueryList.prototype.removeListener = function (fn) { this.removeEventListener('change', fn); };
// 刀56: the hand-rolled trio is gone — MediaQueryList inherits the native
// EventTarget core like Chrome (listeners with full options semantics;
// onchange fires through the dispatch handler hook; the accessor is
// prototype-level, so isHandlerSlot passes).
if (typeof globalThis.MediaQueryList === 'undefined') globalThis.MediaQueryList = MediaQueryList;
const _mqlRegistry = [];
// Re-evaluate every subscribed MQL against the new environment and fire
// change on the ones that flipped. Inert objects (no listener, no
// onchange) are pruned first: their matches is live-computed on read, so
// only subscribed objects need tracking — matchMedia inside a resize
// handler cannot grow the registry unbounded. Listener liveness is the
// hooks' registry mirror (event_hooks.js); _change died with the trio.
globalThis.__diting_mqFlush = function () {
  const hasListeners = globalThis.__diting_etHasListeners || (() => false);
  for (let i = _mqlRegistry.length - 1; i >= 0; i--) {
    const mq = _mqlRegistry[i];
    if (mq.onchange == null && !hasListeners(mq, 'change')) _mqlRegistry.splice(i, 1);
  }
  for (const mq of _mqlRegistry.slice()) {
    const now = mq.matches;
    if (now === mq._last) continue;
    mq._last = now;
    const ev = new Event('change');
    try { ev.matches = now; ev.media = mq._q; } catch (e) {}
    try { mq.dispatchEvent(ev); } catch (e) {}
  }
};
globalThis.matchMedia = _markNative(function matchMedia(q) {
  const mq = new MediaQueryList(q);
  _mqlRegistry.push(mq);
  return mq;
});

globalThis.__diting_shadowHostNames = new Set(['article','aside','blockquote','body','div','footer','h1','h2','h3','h4','h5','h6','header','main','nav','p','section','span']);
// ElementInternals: form-associated custom element internals. Validity/state
// are JS-observable; ARIA reflection that needs the accessibility tree is not.
globalThis.ElementInternals = class ElementInternals {
  constructor(el) { this._el = el; this._valid = true; this._flags = {}; this._message = ''; this._value = null; this._states = new Set();  __hideOwn(this);}
  setFormValue(value, state) { this._value = value; }
  setValidity(flags, message, anchor) {
    flags = flags || {};
    const bad = Object.keys(flags).some((k) => k !== 'valid' && flags[k]);
    if (bad && (message == null || message === '')) throw new TypeError("Failed to execute 'setValidity' on 'ElementInternals': The second argument should not be empty if one or more flags in the first argument are true.");
    this._flags = flags; this._valid = !bad; this._message = bad ? String(message) : '';
  }
  checkValidity() { return this._valid; }
  reportValidity() { return this._valid; }
  get validity() {
    const f = this._flags || {};
    return { valid: this._valid, valueMissing: !!f.valueMissing, typeMismatch: !!f.typeMismatch, patternMismatch: !!f.patternMismatch, tooLong: !!f.tooLong, tooShort: !!f.tooShort, rangeUnderflow: !!f.rangeUnderflow, rangeOverflow: !!f.rangeOverflow, stepMismatch: !!f.stepMismatch, badInput: !!f.badInput, customError: !!f.customError };
  }
  get validationMessage() { return this._message || ''; }
  get willValidate() { return true; }
  get form() { return this._el && this._el.closest ? this._el.closest('form') : null; }
  get labels() { return this._el && this._el.labels ? this._el.labels : _nodeList([]); }
  get shadowRoot() { return (this._el && this._el._shadowRoot) || null; }
  get states() { return this._states; }
};
// 刀58 (M4): ElementInternals' ARIAMixin face — 51 members Chrome 145 lists
// (role, 43 nullable-DOMString aria*, 7 element-ref aria*) that the engine
// lacked entirely (the audit's biggest missing family). Chrome-truth storage
// semantics, probed all three ways (attribute→getter, setter→attribute,
// setter→element's own reflection face): the store is INTERNAL ONLY. The
// host's aria-*/role content attributes never feed these getters, these
// setters never touch the attributes, and the element's own ARIAMixin face
// reads the attributes — two independent stores (el.ariaLabel='A' +
// internals.ariaLabel='B' coexist). Element-ref faces store the array as-is
// (no attribute sync either direction; null clears; [] stays an empty list).
{
  const STR = ['role', 'ariaAtomic', 'ariaAutoComplete', 'ariaBrailleLabel',
    'ariaBrailleRoleDescription', 'ariaBusy', 'ariaChecked', 'ariaColCount',
    'ariaColIndex', 'ariaColIndexText', 'ariaColSpan', 'ariaCurrent',
    'ariaDescription', 'ariaDisabled', 'ariaExpanded', 'ariaHasPopup',
    'ariaHidden', 'ariaInvalid', 'ariaKeyShortcuts', 'ariaLabel', 'ariaLevel',
    'ariaLive', 'ariaModal', 'ariaMultiLine', 'ariaMultiSelectable',
    'ariaOrientation', 'ariaPlaceholder', 'ariaPosInSet', 'ariaPressed',
    'ariaReadOnly', 'ariaRelevant', 'ariaRequired', 'ariaRoleDescription',
    'ariaRowCount', 'ariaRowIndex', 'ariaRowIndexText', 'ariaRowSpan',
    'ariaSelected', 'ariaSetSize', 'ariaSort', 'ariaValueMax', 'ariaValueMin',
    'ariaValueNow', 'ariaValueText'];
  const REFS = ['ariaActiveDescendantElement', 'ariaControlsElements',
    'ariaDescribedByElements', 'ariaDetailsElements', 'ariaErrorMessageElements',
    'ariaFlowToElements', 'ariaLabelledByElements'];
  const IP = ElementInternals.prototype;
  for (const k of STR) {
    Object.defineProperty(IP, k, {
      get: function () { const s = this._aria; return s && s[k] !== undefined ? s[k] : null; },
      set: function (v) { (this._aria || (this._aria = {}))[k] = v == null ? undefined : String(v); },
      enumerable: true, configurable: true,
    });
  }
  for (const k of REFS) {
    Object.defineProperty(IP, k, {
      get: function () { const s = this._refs; return s && s[k] !== undefined ? s[k] : null; },
      set: function (v) { (this._refs || (this._refs = {}))[k] = Array.isArray(v) ? v.slice() : null; },
      enumerable: true, configurable: true,
    });
  }
}
// Full standard constant set (issue #439). The partial version here lacked
// FILTER_ACCEPT/REJECT/SKIP and most SHOW_* values, so the canonical
// `acceptNode() { return NodeFilter.FILTER_ACCEPT; }` filter idiom returned
// undefined and TreeWalker/NodeIterator rejected every node.
globalThis.NodeFilter = {
  SHOW_ALL: 0xFFFFFFFF,
  SHOW_ELEMENT: 0x1,
  SHOW_ATTRIBUTE: 0x2,
  SHOW_TEXT: 0x4,
  SHOW_CDATA_SECTION: 0x8,
  SHOW_ENTITY_REFERENCE: 0x10,
  SHOW_ENTITY: 0x20,
  SHOW_PROCESSING_INSTRUCTION: 0x40,
  SHOW_COMMENT: 0x80,
  SHOW_DOCUMENT: 0x100,
  SHOW_DOCUMENT_TYPE: 0x200,
  SHOW_DOCUMENT_FRAGMENT: 0x400,
  SHOW_NOTATION: 0x800,
  FILTER_ACCEPT: 1,
  FILTER_REJECT: 2,
  FILTER_SKIP: 3,
};
// ResizeObserver is defined earlier with real per-target firing; the stub
// that previously lived here was a no-op that clobbered the real class.
//
// Stub replaced by the real PerformanceObserver in performance.js (after
// the Performance timeline is constructed). Kept here so early interface
// sweeps see the name; observe/callback face is #217 / obscura #1166.
globalThis.PerformanceObserver = class {
  constructor(){}
  observe(){}
  disconnect(){}
  takeRecords() { return []; }
  // Honest set: exactly the entry types the Performance timeline records
  // (mark/measure via user timing, navigation, paint). Web-vitals wrappers
  // gate their LCP/CLS/longtask setup on this list — advertising types we
  // never emit would push them into a wait that never resolves.
  static get supportedEntryTypes() { return ['mark', 'measure', 'navigation', 'paint']; }
};

// Event and CustomEvent are native interfaces (crates/diting_runtime/src/
// native/event.rs): state in a private slot, prototype accessors, the
// unforgeable own isTrusted, one dispatch-flag semantics for initEvent.
// Installed with the faces before this script runs.

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
      try { console.error('Uncaught (in promise) ' + String(reason)); } catch (err) {}
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
globalThis.AnimationEvent = class AnimationEvent extends Event {
  // WebIDL AnimationEventInit: animationName "" / elapsedTime 0 /
  // pseudoElement "" defaults.
  constructor(t, o = {}) {
    if (arguments.length < 1) throw new TypeError("Failed to construct 'AnimationEvent': 1 argument required, but only 0 present.");
    super(t, o);
    _evx.set(this, {
      animationName: o.animationName !== undefined ? String(o.animationName) : "",
      elapsedTime: o.elapsedTime !== undefined ? Number(o.elapsedTime) : 0,
      pseudoElement: o.pseudoElement !== undefined ? String(o.pseudoElement) : "",
    });
  }
};
_evxRo(globalThis.AnimationEvent, ['animationName', 'elapsedTime', 'pseudoElement']);
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

// AbortSignal/AbortController (fetch §abort family): the old pair was a dead
// stub — abort() flipped a flag nothing read, and AbortSignal.timeout handed
// back an object with no listener machinery at all. Pages that hand fetch()
// an AbortController (React Query cancellation, AbortSignal.timeout guards)
// got a fetch that never rejects on abort. Real surface here: abort reasons
// (caller-supplied, or the "signal is aborted without reason" AbortError
// DOMException), the abort event with addEventListener/onabort, static
// abort()/timeout()/any(), and throwIfAborted(). fetch() consumes the signal
// pre-flight and races it mid-flight (see _raceAbortSignal at the fetch site).
let _abortSignalConstructionKey = false;
// Chrome chain truth (probe xhr.chain): AbortSignal.prototype →
// EventTarget.prototype directly — NOT through XMLHttpRequestEventTarget.
class AbortSignal extends EventTarget {
  constructor() {
    super();
    if (!_abortSignalConstructionKey) throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor");
    _abortSignalConstructionKey = false;
    this._aborted = false;
    this._reason = undefined;
    __hideOwn(this);
  }
  get aborted() { return this._aborted; }
  get reason() { return this._reason; }
  get onabort() { return this._onabort || null; }
  set onabort(fn) { this._onabort = fn; }
  throwIfAborted() { if (this._aborted) throw this._reason; }
  _signalAbort(reason) {
    if (this._aborted) return;
    this._aborted = true;
    this._reason = reason !== undefined ? reason
      : new DOMException("signal is aborted without reason", "AbortError");
    // Real Event (M3 B2): listeners and the onabort handler both fire
    // through the native dispatch core now.
    this.dispatchEvent(new Event("abort"));
  }
  static abort(reason) {
    const ac = new AbortController();
    ac.abort(reason);
    return ac.signal;
  }
  static timeout(ms) {
    if (!Number.isFinite(ms) || ms < 0) {
      throw new TypeError("The provided value is negative or not a finite number");
    }
    const ac = new AbortController();
    setTimeout(() => ac.abort(new DOMException("signal timed out", "TimeoutError")), ms);
    return ac.signal;
  }
  static any(signals) {
    const ac = new AbortController();
    for (const s of Array.from(signals || [])) {
      if (!s || typeof s !== "object") {
        throw new TypeError("Argument 1 can not be converted to a sequence");
      }
      if (s.aborted) { ac.abort(s.reason); break; }
      s.addEventListener("abort", () => ac.abort(s.reason));
    }
    return ac.signal;
  }
}
globalThis.AbortSignal = AbortSignal;
globalThis.AbortController = class AbortController {
  constructor() { _abortSignalConstructionKey = true; __diting_liftSlot(this).signal = new AbortSignal(); }
  abort(reason) { this.signal._signalAbort(reason); }
};
_markNative(AbortSignal);
_markNative(AbortSignal.prototype.throwIfAborted);
_markNative(AbortSignal.abort);
_markNative(AbortSignal.timeout);
_markNative(AbortSignal.any);
_markNative(globalThis.AbortController);

// FontFace / FontFaceSet / document.fonts live in platform/fontfaceset.impl.js (M4 knife 52).

// fp-diff-1014 P1: Chrome desktop exposes SharedArrayBuffer ONLY inside
// cross-origin-isolated contexts (since 92) — a plain page reads
// typeof SharedArrayBuffer === "undefined". The deno host global leaked the
// constructor on every realm. The delete can't live here at snapshot time —
// V8's context deserializer rebuilds the intrinsic function table wholesale,
// so a snapshot-time delete silently un-deletes on every restore (live
// probe: typeof came back "function" while the runtime delete works). It
// runs per-realm from __diting_init instead; only the isolation FLAG is
// defined here (adds persist in the snapshot, intrinsic deletes don't).
// The structured-clone path above guards with typeof, and no page can
// construct one to clone.
if (!('crossOriginIsolated' in globalThis)) {
  Object.defineProperty(globalThis, 'crossOriginIsolated', { value: false, configurable: true });
}

globalThis.reportError = globalThis.reportError || ((e) => console.error(e));

globalThis.screenX = 0; globalThis.screenY = 0;
globalThis.screenLeft = 0; globalThis.screenTop = 0;
globalThis.pageXOffset = 0; globalThis.pageYOffset = 0;
globalThis.scrollX = 0; globalThis.scrollY = 0;



globalThis.DOMStringMap = DOMStringMap;

[
  globalThis.matchMedia,
  // (刀54) the timer/raf faces moved to timer_belt.js — their native
  // marking migrated there (bootstrap loads first; 刀22 precedent).
  globalThis.structuredClone, globalThis.reportError,
  console.log, console.warn, console.error, console.info, console.debug,
  console.dir, console.assert,
  // (刀22) Notification + chrome.csi/loadTimes moved to bchk_apis.js —
  // their native marking migrated there (bootstrap loads first and would
  // see undefined here; the member reads also hid from the analyzer).
ResizeObserver, PerformanceObserver,  // (刀58) MutationObserver moved to
  // mutation_observer.js — its marking migrated there (刀22 precedent).
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });





// Geometry family (DOMPoint/DOMRect/DOMQuad/DOMMatrix*/WebKitCSSMatrix):
// generated shape + behavior module now owns this whole family
// (platform/gen/geometry.gen.js + platform/geometry.impl.js, M4 knife 1).



// (Re)apply the platform persona: screen, dpr, hardwareConcurrency,
// deviceMemory. Called from __diting_init AND again from Rust's
// set_user_agent — the runtime constructor runs init before the context's
// UA is known, so the persona must refresh once the real UA lands.
globalThis.__diting_setPersona = function() {
  const scr = _fp('screen');
  const sw = scr[0], sh = scr[1];
  globalThis.__diting_screenApply(sw, sh);
  globalThis.visualViewport = globalThis.__diting_makeVisualViewport(sw, sh - 80);
  // From the persona pool, so a retina Mac panel reports 2x (the old
  // width-only heuristic gave 1x on 1512x982 — impossible for that panel).
  globalThis.devicePixelRatio = _fp('dpr') || (sw >= 2560 ? 2 : 1);
  __vpW = sw; __vpH = sh - 80; __vpOW = sw; __vpOH = sh;
  // Publish the persona viewport to the Rust layout layer so
  // getBoundingClientRect / element rects anchor the initial containing
  // block to the same window scripts see (not the pre-persona default).
  try { _domRaw("set_viewport", String(sw), String(sh - 80)); } catch (e) {}

  // Stable for the life of the process (drawn once), not per navigation —
  // hardwareConcurrency flipping between pages of one visit is its own
  // automation tell. Cores come from the persona device table (_fp('hw')) so
  // they stay coherent with the GPU/UAData architecture face. (P8) Memory is
  // ALSO persona-driven now (_fp('mem'), drawn from per-chip RAM options):
  // Chrome 155 dropped the 8 GiB deviceMemory cap, so real machines report
  // 16/32 — and an M3 Pro persona claiming 8 GB is a config Apple never
  // shipped, a cross-field contradiction the 300015 scorer can key on.
  const plat = _fpPlatform();
  if (globalThis.__diting_hw === undefined || globalThis.__diting_hw_plat !== plat) {
    globalThis.__diting_hw = _fp('hw');
    globalThis.__diting_mem = _fp('mem');
    globalThis.__diting_hw_plat = plat;
  }
  globalThis.__diting_navSet('hardwareConcurrency', globalThis.__diting_hw);
  globalThis.__diting_navSet('deviceMemory', globalThis.__diting_mem);
};

// Viewport override for session_viewport / CDP setDeviceMetricsOverride.
// The persona stays the identity of the machine; the override is the window
// the page is looked at through — screen keeps the persona while
// innerWidth/innerHeight/visualViewport/outer* move, which is what Chrome's
// device emulation does too. The mobile flag swaps the matchMedia pointer/
// hover answers to coarse/none: a phone-sized viewport answering
// `pointer: fine` is a contradiction a fingerprint script can cross-check
// against maxTouchPoints, so those move together.
// devicePixelRatio follows the same split: the 4th arg (CDP
// setDeviceMetricsOverride's deviceScaleFactor) pins what scripts see when
// > 0, 0 restores the persona's (Chromium's "0 = default"), undefined
// leaves it untouched.
globalThis.__diting_setViewport = function(w, h, mobile, dpr) {
  __vpW = w; __vpH = h; __vpOW = w; __vpOH = h;
  if (typeof dpr === 'number' && dpr > 0) globalThis.devicePixelRatio = dpr;
  else if (dpr === 0) globalThis.devicePixelRatio = _fp('dpr') || (globalThis.screen.width >= 2560 ? 2 : 1);
  globalThis.visualViewport = globalThis.__diting_makeVisualViewport(w, h);
  globalThis.__diting_mq_mobile = mobile ? _MQ_MOBILE_TABLE : null;
  globalThis.__diting_mqRecompute();
  // (#117) through the store: defineProperty on the hoisted instance would
  // materialize an OWN prop and break Object.keys(navigator) === [].
  // The touch pin outranks the device class (an explicit
  // setTouchEmulationEnabled survives a viewport re-pin; #151).
  globalThis.__diting_navSet('maxTouchPoints', __diting_touchMax() || (mobile ? 5 : 0));
  // Feed the Rust layout ICB so element rects and @media cascade see the
  // same width the scripts do.
  try { _domRaw("set_viewport", String(w), String(h)); } catch (e) {}
  try { globalThis.dispatchEvent(new Event('resize')); } catch (e) {}
  // Media queries re-evaluated after the resize: subscribed MQLs that
  // crossed fire change (issue #25).
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// Drop the override: persona viewport everywhere, desktop pointer answers.
// Emulated media (setEmulatedMedia) and touch emulation are separate Chrome
// knobs and survive.
globalThis.__diting_clearViewport = function() {
  globalThis.__diting_mq_mobile = null;
  globalThis.__diting_mqRecompute();
  __diting_setPersona();
  globalThis.__diting_navSet('maxTouchPoints', __diting_touchMax());
  try { globalThis.dispatchEvent(new Event('resize')); } catch (e) {}
  // Media queries re-evaluated after the resize: subscribed MQLs that
  // crossed fire change (issue #25).
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// Touch emulation (CDP Emulation.setTouchEmulationEnabled, #151): swap the
// interaction-media answers and maxTouchPoints WITHOUT touching the
// viewport — device metrics are setDeviceMetricsOverride's knob, this is
// only the touch capability face. The pin is the point count (> 0); null
// restores the device class's own touch story (0 desktop, 5 mobile).
function __diting_touchMax() {
  const t = globalThis.__diting_touch_points;
  return typeof t === 'number' && t > 0 ? t : 0;
}
globalThis.__diting_setTouch = function(enabled, maxPoints) {
  globalThis.__diting_mq_touch = enabled ? _MQ_TOUCH_TABLE : null;
  globalThis.__diting_touch_points = enabled ? (maxPoints > 0 ? maxPoints : 1) : null;
  globalThis.__diting_mqRecompute();
  globalThis.__diting_navSet('maxTouchPoints', enabled ? globalThis.__diting_touch_points
    : (globalThis.__diting_mq_mobile ? 5 : 0));
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// Emulated media environment (CDP Emulation.setEmulatedMedia, Playwright's
// page.emulateMedia): featuresJson is a JSON array of [name, value] pairs
// REPLACING all emulated preferences (Chrome semantics — an absent feature
// stops being emulated), mediaType is 'print' or 'screen'. The same pairs
// go to the Rust layout state so the @media cascade re-parses in agreement
// (two faces, one truth), then subscribed MQLs flush (issue #29).
globalThis.__diting_setMediaFeatures = function(featuresJson, mediaType) {
  let pairs = [];
  try { pairs = JSON.parse(featuresJson) || []; } catch (e) {}
  const emu = {};
  if (Array.isArray(pairs)) {
    for (const pair of pairs) {
      if (!Array.isArray(pair) || pair.length < 2) continue;
      const name = String(pair[0]).toLowerCase(), value = String(pair[1]).toLowerCase();
      if (!name) continue;
      emu[name] = {};
      emu[name][value] = true;
    }
  }
  globalThis.__diting_mq_emulated = Object.keys(emu).length ? emu : null;
  globalThis.__diting_mq_media_type = mediaType === 'print' ? 'print' : 'screen';
  try { _domRaw("set_media_env", featuresJson, globalThis.__diting_mq_media_type); } catch (e) {}
  globalThis.__diting_mqRecompute();
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// The hardware persona (screen/dpr/GPU/canvas) must hold for a whole
// visit: a real machine does not change its panel or GPU between pages,
// and identity-level values flipping per navigation is its own automation
// tell (the invariant hardwareConcurrency already keeps). __diting_init
// runs once per realm (it self-deletes), so cross-page continuity comes
// from the host re-pinning the same seed via __diting_setFpSeed after the
// fresh realm boots — Page-owned state, like the viewport override.
globalThis.__diting_setFpSeed = function(s) {
  _fpSeed = s | 0;
  _fpCache = null;
  // __diting_init's throwaway draw already filled the hw/deviceMemory guard
  // cache before this re-pin lands; drop it so the persona redraws them from
  // the pinned seed (the guard then holds them put for the rest of the realm).
  globalThis.__diting_hw = undefined;
  globalThis.__diting_mem = undefined;
  globalThis.__diting_hw_plat = undefined;
  __diting_setPersona();
};

globalThis.__diting_init = function() {
  _fpSeed = Date.now() ^ (Math.random() * 0xFFFFFFFF >>> 0);
  _fpCache = null;
  // A real navigation just completed (this runs after set_url), so drop any
  // URL a location setter previewed synchronously and let document_url drive
  // location.href again, including any redirect target.
  globalThis.__virtualUrl = null;
  _installWasmStreamingFallback();
  // (#37) V8 snapshot creation flattens accessors on the global object to
  // data:null props, so the window on* family installs here, per context.
  globalThis.__diting_installWindowOnHandlers();
  // (#105) Same flattening: the window size accessors install here too,
  // so page writes to innerWidth/innerHeight are ignored like Chrome's.
  globalThis.__diting_installViewportAccessors();

  globalThis.__diting_setPersona();

  // Per-realm performance.now() coarsening offset (see performance.js): the
  // module-level draw happens once at snapshot build, which would freeze one
  // bucket offset onto EVERY context the process ever serves — cross-session
  // aggregatable. Re-draw per realm onto the performance object's hidden _
  // slot family (_marks/_measures), not a new window global.
  try {
    Object.defineProperty(globalThis.performance, '_noise', {
      value: Math.random() * 0.1, writable: true, configurable: true,
    });
  } catch (e) {}

  // SharedArrayBuffer delete lives HERE, not at snapshot top level: V8's
  // context deserializer rebuilds the intrinsic table on every restore, so
  // the snapshot-time delete never survived (see the FontFace block above
  // for the full story). __diting_init runs per realm post-restore, where
  // the delete actually sticks.
  try { delete globalThis.SharedArrayBuffer; } catch (e) {}

  const t0 = Date.now() + Math.floor(_fpRand(641) * 100) - 50;
  // NavigationTiming offsets: DCL 60–600ms after navigationStart, load
  // DCL+40–800ms after that, with a plausible connect/response chain ahead
  // of domInteractive. All-equal epoch stamps (the old shape) made every
  // derived duration 0 — telemetry reading getEntriesByType('navigation')
  // got a zero-duration page, which is its own automation tell.
  const dcl = 60 + Math.floor(_fpRand(642) * 540);
  const load = dcl + 40 + Math.floor(_fpRand(643) * 760);
  // (#78) Chrome's timeOrigin is a float; navigationStart stays integral.
  globalThis.performance._timeOrigin = t0 + _fpRand(644) * 0.9;
  globalThis.performance._timing = {
    navigationStart: t0,
    fetchStart: t0 + 1, domainLookupStart: t0 + 2, domainLookupEnd: t0 + 12,
    connectStart: t0 + 12, connectEnd: t0 + 48, secureConnectionStart: t0 + 18,
    requestStart: t0 + 50, responseStart: t0 + 85, responseEnd: t0 + 130,
    domInteractive: t0 + Math.floor(dcl * 0.8),
    domContentLoadedEventStart: t0 + dcl - 4, domContentLoadedEventEnd: t0 + dcl,
    domComplete: t0 + load, loadEventStart: t0 + load, loadEventEnd: t0 + load,
  };
  // Derived entries (navigation/paint) are cached per navigation.
  if (globalThis.performance._resetDerived) globalThis.performance._resetDerived();
  globalThis.performance._memory = {
    jsHeapSizeLimit: 2172649472,
    totalJSHeapSize: 15000000 + Math.floor(_fpRand(620) * 85000000),
    usedJSHeapSize: 8000000 + Math.floor(_fpRand(621) * 42000000),
  };
  // Notification.permission stays "default" — the class default. A random
  // granted/default coin flip (old line) read as automation: fresh Chrome
  // profiles are always "default" until the OS prompt answers (fp-diff-1014 #8).

  // Hide internals (_*, obscura, Obscura). The set of keys is static at
  // snapshot-build time, so we precompute it ONCE below (after this
  // function definition) and reuse it on every page init. Was an
  // Object.keys + filter on every navigation, ~5-40ms per page on
  // SPAs that load 1000+ globals.
  const toHide = globalThis.__diting_hide_list || [];
  for (let i = 0; i < toHide.length; i++) {
    try { Object.defineProperty(globalThis, toHide[i], { enumerable: false }); } catch(e) {}
  }

  // Strip the deno runtime globals from the page's window. A `Deno` (or
  // `__bootstrap`) own property on window is the canonical marker of a
  // deno-based runtime — precisely what an AI-agent-blocker looks for — and
  // it survives enumerable:false because collectors enumerate with
  // getOwnPropertyNames, not Object.keys. All engine op calls run through
  // the script-scoped `_OPS` binding, so the globals are dead weight here.
  // Both are configurable in deno_core and delete cleanly; keep the try
  // guards in case a future deno_core makes them non-configurable.
  try { delete globalThis.Deno; } catch(e) {}
  try { delete globalThis.__bootstrap; } catch(e) {}

  // Non-configurable function declarations above (the engine's `_`-prefixed
  // helpers and `__diting*` bookkeeping) cannot be deleted, so hide them at
  // the enumeration boundary instead: Radar's windowFeatures collector
  // hashes Object.getOwnPropertyNames(window), and fingerprinting scripts
  // also enumerate with Reflect.ownKeys / Object.keys /
  // Object.getOwnPropertyDescriptors (upstream 4c33f6d). The wrappers only
  // filter the global object — every other receiver sees the untouched
  // native result — and are marked native so toString lie-detectors report
  // [native code].
  if (!globalThis.__diting_gopn_patched) {
    // Hide engine bookkeeping from window enumeration, but do NOT blanket-
    // strip every `_`-prefixed name. Chrome exposes page-installed props
    // like `_webmsxyw` (ark/itemark AS signer) in getOwnPropertyNames, and
    // xhs 300015 "risk plugin" probes hash that face. A blanket
    // `startsWith('_')` made our winKeys diverge on every page that
    // installs a `_` signer — leading candidate for the ark VM refusing
    // to take over (www-family X-s≈664 / no x-s-common vs Chrome
    // ark-family X-s≈408 + common≈2108).
    // Policy:
    //   - `__*` double-underscore: engine/recipe bookkeeping → hide
    //   - single `_` in the snapshot hide list: engine helpers → hide
    //   - `*diting*` / `__bootstrap`: engine → hide
    //   - page `_webmsxyw` and friends: visible like Chrome
    // `__diting_objects` & friends created by Rust AFTER the snapshot
    // froze the list are still caught by the diting / `__` arms.
    let _hideSet = null;
    // crypto_subtle.js / ed25519.js helper function declarations (block-level declarations hoist to global
    // scope and ed25519.js shares them): not `_`-prefixed, so the membership set above cannot catch them.
    const _extraHide = new Set(['makeKey', 'keyBytes', 'ecdsaUnflat', 'ecMaterial', 'requireEcdsaUsages', 'ecUsagesFor', 'normalizeAlgo',
      'normalizeHash', 'b64urlToBytes', 'bytesToB64url', 'runOp', 'keyAlgorithmFor', 'checkUsageStrings', 'checkKeyUsages', 'jwkRequire',
      'jwkMemberBytes', 'jwkValidateCommon', 'requireHashParam', 'rsaFace', 'rsaUnflat', 'rsaMaterial', 'rsaImportKey', 'rsaGenerateKey',
      'subtleCipher', 'checkUsages']);
    const _isInternal = (n) => {
      if (typeof n !== 'string') return false;
      if (_extraHide.has(n)) return true;
      if (n.includes('diting') || n.includes('Diting') || n === '__bootstrap') return true;
      if (n.startsWith('__')) return true;
      // `_` names and the scope-exported internals (the generated @@EXPORTS block registers them) come from the hide list
      if (_hideSet === null) {
        _hideSet = new Set(globalThis.__diting_hide_list || []);
      }
      return _hideSet.has(n);
    };
    const _gopn = Object.getOwnPropertyNames;
    const _ownKeys = Reflect.ownKeys;
    const _keys = Object.keys;
    const _gopds = Object.getOwnPropertyDescriptors;
    const _patch = (obj, key, impl) => {
      _markNative(impl);
      try { Object.defineProperty(impl, 'name', { value: key }); } catch(e) {}
      try { Object.defineProperty(obj, key, { value: impl, writable: true, enumerable: false, configurable: true }); } catch(e) {}
    };
    // Engine helpers living on PLATFORM interface objects (`_pump`, `_fire`, `_signalAbort` ...) are
    // not Chrome members; Chrome's own-name list for those prototypes never has them. Hide `_`-prefixed
    // names whenever the receiver is a platform interface constructor or its prototype (page classes
    // defined later are not in the set, so a page's own `_x` stays visible).
    const _ifaceObjs = new WeakSet();
    const _jsBuiltin = /^(Object|Function|Array|String|Number|Boolean|Symbol|BigInt|Date|RegExp|Promise|Proxy|Map|Set|WeakMap|WeakSet|WeakRef|FinalizationRegistry|ArrayBuffer|SharedArrayBuffer|DataView|Atomics|Intl|WebAssembly|(Eval|Range|Reference|Syntax|Type|URI|Aggregate)?Error|(Big)?(Int|Uint|Float)(8|16|32|64)(Clamped)?Array|Iterator)$/;
    for (const n of _gopn(globalThis)) {
      if (!/^[A-Z]/.test(n) || _jsBuiltin.test(n)) continue;
      let d; try { d = Object.getOwnPropertyDescriptor(globalThis, n); } catch (e) { continue; }
      const C = d && d.value;
      if (typeof C === 'function') {
        _ifaceObjs.add(C);
        if (C.prototype && typeof C.prototype === 'object') _ifaceObjs.add(C.prototype);
      }
    }
    const _gpo = Object.getPrototypeOf;
    const _hideUnderscore = (names) => names.filter(n => typeof n !== 'string' || n.charCodeAt(0) !== 95);
    // instances: only the names the engine's own scripts use (internal_names.js, generated) - a page's own `_x`
    // expando on document / performance / any platform object stays visible like in Chrome.
    const _hideInternalOnInstance = (names) => {
      const set = globalThis.__diting_internal_names, ap = globalThis.__diting_accPrivs;
      return set ? names.filter(n => typeof n !== 'string' || !(set.has(n) || (ap && ap.has(n)) || /^_on[a-z]+$/.test(n))) : names;
    };
    const _hideOn = (o, names) => {
      if (o === globalThis) return names.filter(n => !_isInternal(n));
      if (o === null || (typeof o !== 'object' && typeof o !== 'function')) return names;
      if (_ifaceObjs.has(o)) return _hideUnderscore(names);
      // an INSTANCE of a platform interface (its direct prototype is the platform prototype): the engine keeps
      // helper state in `_x` own properties that Chrome's instances do not have. Instances of page-defined
      // subclasses have a page prototype and keep their `_x` (Lit/Stencil style fields) visible.
      if (typeof o === 'object') { let pr; try { pr = _gpo(o); } catch (e) { return names; } if (pr !== null && _ifaceObjs.has(pr)) return _hideInternalOnInstance(names); }
      return names;
    };
    _patch(Object, 'getOwnPropertyNames', function(o) { return _hideOn(o, _gopn(o)); });
    _patch(Reflect, 'ownKeys', function(o) { return _hideOn(o, _ownKeys(o)); });
    _patch(Object, 'keys', function(o) { return _hideOn(o, _keys(o)); });
    _patch(Object, 'getOwnPropertyDescriptors', function(o) {
      const all = _gopds(o);
      if (o === globalThis) { for (const n of _gopn(all)) { if (_isInternal(n)) delete all[n]; } }
      else if (o !== null && typeof o === 'object' || typeof o === 'function') {
        let hide = _ifaceObjs.has(o);
        if (!hide && typeof o === 'object') { try { const pr = _gpo(o); hide = pr !== null && _ifaceObjs.has(pr); } catch (e) {} }
        if (hide) { const keep = typeof o === 'object' && !_ifaceObjs.has(o) ? _hideInternalOnInstance : _hideUnderscore; const kept = new Set(keep(_gopn(all))); for (const n of _gopn(all)) { if (!kept.has(n)) delete all[n]; } }
      }
      return all;
    });
    globalThis.__diting_gopn_patched = true;
  }
  if (typeof globalThis.__diting_demote_own === 'function') { try { globalThis.__diting_demote_own(); } catch (e) {} }
  if (typeof globalThis.__diting_reorder_window === 'function') { try { globalThis.__diting_reorder_window(); } catch (e) {} }
  delete globalThis.__diting_init;
};

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
  d('DOMStringMap', DOMStringMap);
  d('XMLHttpRequestEventTarget', XMLHttpRequestEventTarget);
  d('_DOM_MUTATION_COMMANDS', _DOM_MUTATION_COMMANDS);
  d('_LABELABLE', _LABELABLE);
  d('_MQ_PERSONA_BOOL', _MQ_PERSONA_BOOL);
  d('_OPS', _OPS);
  d('__acc', __acc);
  d('__accRo', __accRo);
  d('__currentUrl', __currentUrl);
  d('__def', __def);
  d('__ditingReorderProto', __ditingReorderProto);
  d('__hideOwn', __hideOwn);
  d('__locParts', __locParts);
  d('_decodeBodyWithCharset', _decodeBodyWithCharset);
  d('_decodeDataScriptUrl', _decodeDataScriptUrl);
  d('_ditingEpochNow', _ditingEpochNow);
  a('_ditingMutationEpoch', () => _ditingMutationEpoch, (v) => { _ditingMutationEpoch = v; });
  d('_dom', _dom);
  d('_domParse', _domParse);
  d('_domRaw', _domRaw);
  d('_domStringMapKey', _domStringMapKey);
  d('_edgeMemo', _edgeMemo);
  d('_eventState', _eventState);
  d('_evx', _evx);
  d('_evxGuard', _evxGuard);
  d('_evxProps', _evxProps);
  d('_evxRo', _evxRo);
  d('_fireBlurFamily', _fireBlurFamily);
  d('_fp', _fp);
  a('_fpCache', () => _fpCache, (v) => { _fpCache = v; });
  d('_fpRand', _fpRand);
  a('_fpSeed', () => _fpSeed, (v) => { _fpSeed = v; });
  d('_getFp', _getFp);
  d('_isFormControlDisabled', _isFormControlDisabled);
  d('_isSubmitButton', _isSubmitButton);
  d('_labeledControl', _labeledControl);
  d('_locationObj', _locationObj);
  d('_markNativeProto', _markNativeProto);
  d('_mirrorSelectLabel', _mirrorSelectLabel);
  d('_nodeInDocument', _nodeInDocument);
  d('_resolveUrl', _resolveUrl);
  d('_shallowCloneNode', _shallowCloneNode);
  d('_styleProxy', _styleProxy);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
