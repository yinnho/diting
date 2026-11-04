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
globalThis._formValues = globalThis._formValues || {};
globalThis._formChecked = globalThis._formChecked || {};
globalThis._formIndeterminate = globalThis._formIndeterminate || {};
globalThis._fileInputs = globalThis._fileInputs || {};
const _formValues = globalThis._formValues;
const _formChecked = globalThis._formChecked;
const _formIndeterminate = globalThis._formIndeterminate;
const _fileInputs = globalThis._fileInputs;
// FileList-shaped face (length/item/indexed/iterator/toStringTag), shared by
// the input.files getter (#408 lineage) and DataTransfer.files (#58).
// #258: FileList is a real global constructor — boot-time feature detects
// and `x instanceof FileList` guards ReferenceError without it (obscura
// #1232). Constructing throws like Chrome's; the list objects below stay
// the same shape but now carry its prototype, so instanceof holds.
function _FileList() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
Object.defineProperty(_FileList, 'name', { value: 'FileList', configurable: true });
if (typeof FileList === 'undefined') globalThis.FileList = _FileList;
// (M4 刀75) length/item/@@iterator/@@toStringTag are PROTOTYPE members (Chrome 145);
// the instance keeps only its indexed own entries. State rides a WeakMap.
const _flState = new WeakMap();
function _makeFileList(fs) {
  const list = Object.create(_FileList.prototype);
  _flState.set(list, fs);
  for (let i = 0; i < fs.length; i++) list[i] = fs[i];
  return list;
}
{
  const P = _FileList.prototype;
  const st = (o) => { const s = (o !== null && typeof o === 'object') ? _flState.get(o) : undefined; if (!s) throw new TypeError('Illegal invocation'); return s; };
  const lenGet = function () { return st(this).length; };
  Object.defineProperty(lenGet, 'name', { value: 'get length' });
  Object.defineProperty(P, 'length', { get: lenGet, enumerable: true, configurable: true });
  const item = function item(index) {
    const s = st(this);
    if (arguments.length < 1) throw new TypeError("Failed to execute 'item' on 'FileList': 1 argument required, but only 0 present.");
    const i = Number(index) >>> 0;
    return i < s.length ? s[i] : null;
  };
  Object.defineProperty(item, 'length', { value: 1 });
  Object.defineProperty(P, 'item', { value: item, writable: true, enumerable: true, configurable: true });
  Object.defineProperty(P, Symbol.iterator, { value: function values() { const s = st(this); return Array.prototype.values.call(s); }, writable: true, enumerable: false, configurable: true });
  Object.defineProperty(P, Symbol.toStringTag, { value: 'FileList', configurable: true });
}
const _domParse = (cmd, a1, a2) => { try { return JSON.parse(_dom(cmd, a1, a2)); } catch { return null; } };

// HTML "ASCII whitespace": U+0009 TAB, U+000A LF, U+000C FF, U+000D CR, U+0020 SPACE.
// Class token splitting (classList, getElementsByClassName) uses exactly this set.
// JS \s is wider (U+000B, U+00A0, U+2028, etc.), so it must not be used here.
const _ASCII_WS = /[ \t\n\f\r]+/;
function _splitAsciiWhitespace(s) {
  // WebIDL DOMString coercion: null -> "null", undefined -> "undefined".
  return String(s).split(_ASCII_WS).filter(Boolean);
}
// Shared getElementsByClassName: split the argument into an ordered set of
// tokens on ASCII whitespace, then return descendants (in tree order) whose
// class attribute contains every token, as an HTMLCollection (so namedItem and
// named access work on the result). `root` must expose querySelectorAll.
function _getElementsByClassName(root, classNames) {
  const tokens = _splitAsciiWhitespace(classNames);
  if (tokens.length === 0) return __diting_mkCollection([]);
  // Fast path: a single CSS-identifier token goes straight to the native
  // selector engine (the common case). Only multi-token sets or exotic class
  // names (NBSP, leading digits, etc.) fall back to the O(n) JS scan below.
  if (tokens.length === 1 && /^[A-Za-z_-][\w-]*$/.test(tokens[0])) {
    return __diting_mkCollection(root.querySelectorAll("." + tokens[0]));
  }
  const all = root.querySelectorAll("*");
  const matched = [];
  for (let i = 0; i < all.length; i++) {
    const el = all[i];
    const elTokens = _splitAsciiWhitespace(el.getAttribute ? (el.getAttribute("class") || "") : "");
    let ok = true;
    for (let t = 0; t < tokens.length; t++) {
      if (elTokens.indexOf(tokens[t]) < 0) { ok = false; break; }
    }
    if (ok) matched.push(el);
  }
  return __diting_mkCollection(matched);
}
// Parser-order script visibility (#120): while a classic parser script runs,
// <script> elements later in the markup are not yet parsed and must be
// invisible to script-tag queries — KISSY and JSONP loaders locate their own
// tag by reverse-scanning document.getElementsByTagName('script'), and with
// the whole DOM pre-built they would find a later combo <script> and derive
// a wrong loader base. scripts.rs injects __parserScriptOrder (document-order
// nids of every parser script) and bumps __parserScriptCeiling before each
// classic script (deferred/async and post-parse phases run at -1 = visible).
// Scripts absent from the map — dynamically inserted ones — are always
// visible, matching Chrome: appendChild lands in the tree immediately.
function _parserScriptHidden(nid) {
  const ceiling = globalThis.__parserScriptCeiling;
  if (ceiling == null || ceiling < 0) return false;
  const idx = globalThis.__parserScriptIdx && globalThis.__parserScriptIdx.get(nid);
  return idx !== undefined && idx > ceiling;
}
function _parserVisibleIds(ids) {
  const ceiling = globalThis.__parserScriptCeiling;
  if (ceiling == null || ceiling < 0 || !globalThis.__parserScriptIdx) return ids;
  const map = globalThis.__parserScriptIdx;
  const out = [];
  for (let i = 0; i < ids.length; i++) {
    const idx = map.get(ids[i]);
    if (idx === undefined || idx <= ceiling) out.push(ids[i]);
  }
  return out;
}
// Console argument formatter: DevTools-shaped previews instead of
// "[object Object]". Depth/width clamped, cycles marked, getters that
// throw degrade per-property — a log line must never cost the page its
// console call (issue #99: tmall's fileLoader.loadTimeout logged its
// options object, which collapsed to "[object Object]" and hid which
// resource timed out).
const _FMT_DEPTH = 3, _FMT_WIDTH = 20, _FMT_KEYS = 15, _FMT_MAX = 8192;
// A console call must never run page code: Chrome without DevTools open
// formats nothing, and anti-bot scripts plant accessors on the arguments
// (Error.stack, element id, plain props) as a "DevTools attached" tripwire.
// xhs's mnsv2 loader does exactly that — console.log(err) firing its stack
// getter made it conclude "inspected" and silently skip installing mnsv2.
// Own accessors are therefore never invoked; prototype-chain reads (native
// getters like Element.tagName) stay as they were.
const _peek = (v, k) => {
  const d = Object.getOwnPropertyDescriptor(v, k);
  if (d) return "value" in d ? d.value : _PEEK_GETTER;
  return v[k];
};
const _PEEK_GETTER = Symbol("getter");
const _fmtVal = (v, depth, seen) => {
  if (v === null) return "null";
  if (v === undefined) return "undefined";
  const t = typeof v;
  if (t === "string") return depth === 0 ? v : JSON.stringify(v);
  if (t === "number" || t === "boolean" || t === "bigint" || t === "symbol") return String(v);
  if (t === "function") return "[Function" + (v.name ? ": " + v.name : "") + "]";
  try {
    if (v instanceof Error) {
      const st = _peek(v, "stack");
      if (typeof st === "string" && st) return st;
      const nm = _peek(v, "name"), msg = _peek(v, "message");
      return (typeof nm === "string" ? nm : "Error") + (typeof msg === "string" && msg ? ": " + msg : "");
    }
    if (typeof Element !== "undefined" && v instanceof Element) {
      let s = "<" + String(_peek(v, "tagName")).toLowerCase();
      const eid = _peek(v, "id");
      if (typeof eid === "string" && eid) s += "#" + eid;
      const ecl = _peek(v, "className");
      const cl = typeof ecl === "string" ? ecl.trim() : "";
      if (cl) s += "." + cl.split(/\s+/).join(".");
      return s + ">";
    }
    if (typeof Map !== "undefined" && v instanceof Map) {
      if (seen.has(v)) return "[Circular]";
      if (depth >= _FMT_DEPTH) return "Map(" + v.size + ")";
      seen.add(v);
      const items = [];
      let n = 0;
      for (const [k, val] of v) {
        if (n++ >= _FMT_WIDTH) { items.push("\u2026 +" + (v.size - _FMT_WIDTH) + " more"); break; }
        items.push(_fmtVal(k, depth + 1, seen) + " => " + _fmtVal(val, depth + 1, seen));
      }
      seen.delete(v);
      return "Map(" + v.size + ") {" + items.join(", ") + "}";
    }
    if (typeof Set !== "undefined" && v instanceof Set) {
      if (seen.has(v)) return "[Circular]";
      if (depth >= _FMT_DEPTH) return "Set(" + v.size + ")";
      seen.add(v);
      const items = [];
      let n = 0;
      for (const val of v) {
        if (n++ >= _FMT_WIDTH) { items.push("\u2026 +" + (v.size - _FMT_WIDTH) + " more"); break; }
        items.push(_fmtVal(val, depth + 1, seen));
      }
      seen.delete(v);
      return "Set(" + v.size + ") {" + items.join(", ") + "}";
    }
    if (Array.isArray(v)) {
      if (seen.has(v)) return "[Circular]";
      if (depth >= _FMT_DEPTH) return "Array(" + v.length + ")";
      seen.add(v);
      const items = v.slice(0, _FMT_WIDTH).map(x => _fmtVal(x, depth + 1, seen));
      if (v.length > _FMT_WIDTH) items.push("\u2026 +" + (v.length - _FMT_WIDTH) + " more");
      seen.delete(v);
      return "[" + items.join(", ") + "]";
    }
    if (seen.has(v)) return "[Circular]";
    if (depth >= _FMT_DEPTH) {
      let n = 0; try { n = Object.keys(v).length; } catch {}
      return "Object(" + n + " keys)";
    }
    seen.add(v);
    let keys; try { keys = Object.keys(v); } catch { seen.delete(v); return String(v); }
    const parts = [];
    for (const k of keys.slice(0, _FMT_KEYS)) {
      let val;
      try {
        const pv = _peek(v, k);
        val = pv === _PEEK_GETTER ? "[Getter]" : _fmtVal(pv, depth + 1, seen);
      } catch { val = "[Getter threw]"; }
      parts.push(k + ": " + val);
    }
    if (keys.length > _FMT_KEYS) parts.push("\u2026 +" + (keys.length - _FMT_KEYS) + " more");
    seen.delete(v);
    return "{" + parts.join(", ") + "}";
  } catch { return String(v); }
};
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

let _tid = 0;
const _clearedTimers = new Set();
const _intervals = new Set();
// HTML spec timer nesting: a chain of timers each scheduled from inside a
// timer callback gets its delay floored at 4ms after 5 nesting levels.
// Without the floor a setInterval(fn, 1) self-chain runs at op_sleep
// resolution (~450/s), starving every other macrotask on the event loop —
// doudian's publish page ships exactly such an interval in its telemetry SDK
// and rendered permanently blank (#203).
let _timerNesting = 0;
const _TIMER_FLOOR_MS = 4;

// #38, 2026-10-05: timers no longer get one tokio sleep each. Same-deadline
// sleeps complete in the OP DRIVER's order, not scheduling order — the wheel
// holds a slot as a LIFO stack, the completion FuturesUnordered wakes onto
// another LIFO, and deno_core delivers results 128 per event-loop tick, so
// the two reversals only cancel for small quiet batches. Live on an fxg boot
// storm (1500 same-delay timers): 0..127,309..182,310..437,... — dozens of
// 128-item chunks, some ascending, some descending. HTML's timer task source
// is ordered: same-deadline timers run in scheduling order. The fix is the
// shape every real browser uses — one sorted timer heap, ordered by
// (deadline, sequence), with outstanding sleeps as nothing more than wakeups.
// An op sleep cannot be cancelled, so arming an earlier timer may leave a
// stale sleep in flight; a stale wake (its deadline no longer the armed
// minimum) is a no-op, and the arm policy (`e.d < armed`) keeps at most one
// sleep covering the true earliest deadline.
const _timerHeap = [];
let _timerSeq = 0;
let _timerArmed = Infinity;
const _timerLess = (a, b) => a.d < b.d || (a.d === b.d && a.s < b.s);
const _timerPush = (e) => {
  const h = _timerHeap;
  h.push(e);
  let i = h.length - 1;
  while (i > 0) {
    const p = (i - 1) >> 1;
    if (!_timerLess(h[i], h[p])) break;
    const t = h[i]; h[i] = h[p]; h[p] = t;
    i = p;
  }
};
const _timerPop = () => {
  const h = _timerHeap;
  const top = h[0];
  const last = h.pop();
  if (h.length) {
    h[0] = last;
    let i = 0;
    for (;;) {
      const l = 2 * i + 1, r = l + 1;
      let m = i;
      if (l < h.length && _timerLess(h[l], h[m])) m = l;
      if (r < h.length && _timerLess(h[r], h[m])) m = r;
      if (m === i) break;
      const t = h[i]; h[i] = h[m]; h[m] = t;
      i = m;
    }
  }
  return top;
};
const _timerWake = (d) => {
  if (d !== _timerArmed) { _wheelStale++; return; } // stale: a sleep for a superseded deadline
  _wheelWakes++;
  _timerArmed = Infinity;
  // Fixed snapshot: entries whose f() enqueues more timers during this loop
  // must not extend the pop window mid-wake.
  const now = Date.now();
  let e;
  while ((e = _timerHeap[0]) && e.d <= now) {
    _timerPop();
    // #240: e.f() -> _runAsMacrotask -> _mtArm makes op calls; a sync
    // throw here would abort the pop loop and skip the re-arm below,
    // leaving the remaining due entries rotting with armed === Infinity.
    try { e.f(); } catch (err) { _wheelThrows++; globalThis.__diting_reportUncaughtError(err); }
  }
  _timerArmNext();
};
// #39 discipline: termination unwinds pending ops WITHOUT settling them, so
// the armed sleep can vanish and the earliest deadline would rot while
// `armed` claims coverage. Same cure as the trampoline patrol — a slow
// sleep that re-delivers anything already past deadline.
// #240: the patrol had two silent-death holes of its own. (1) Its reject
// arm was `() => {}` — a rejected patrol sleep left `_timerPatrol`
// latched true, so no new patrol could ever arm. (2) A completion that
// evaporates entirely (severed batch) ALSO leaves the flag latched, and
// `_timerArmed` keeps the now-stale deadline, so `_timerArmNext`'s
// `e.d >= _timerArmed` gate refuses every later timer — observed live on
// a doudian publish run (2026-10-09): fetch callbacks and eval-await
// fine, every setTimeout/setInterval/rAF dead for the rest of the
// session, trampoline idle-healthy. Both rejection arms now run the
// patrol fn itself (idempotent: unlatch, wake due, re-arm), and the
// engine's recover hook force-heals the wheel (see below).
let _wheelWakes = 0;     // wake callbacks that matched the armed deadline
let _wheelStale = 0;     // wake callbacks no-opped as superseded
let _wheelThrows = 0;    // sync throws out of op_sleep calls (must not half-commit)
let _wheelPatrols = 0;   // patrol passes run
// #240 face-2 postmortem ring: the last few fetch lifecycle stages, so a
// fetch whose await continuation vanishes under load names the stage it
// died at instead of leaving "outer resolved, continuation never ran" as
// a four-way contradiction. Stages: armed (registry set), opok/operr (the
// op promise settled), takeok/takerej (heartbeat-probe recovery path),
// resume (await _outer continuation ran), resp (Response constructed),
// fthrow (rethrow after a failed await). [tag, key, Date.now()%1e5].
let _fetchRing = [];
function _fetchStage(tag, fk) {
  _fetchRing.push([tag, fk, Date.now() % 100000]);
  if (_fetchRing.length > 64) _fetchRing.shift();
}
let _timerPatrol = false;
const _timerPatrolFn = () => {
  _timerPatrol = false;
  _wheelPatrols++;
  const e = _timerHeap[0];
  if (e && e.d <= Date.now()) {
    if (_timerArmed !== Infinity) _timerWake(_timerArmed);
    else _timerArmNext();
  }
  if (_timerHeap.length && !_timerPatrol) {
    _timerPatrol = true;
    _OPS.op_sleep(1500).then(_timerPatrolFn, _timerPatrolFn);
  }
};
const _timerArmNext = () => {
  const e = _timerHeap[0];
  if (!e || e.d >= _timerArmed) return;
  const prev = _timerArmed;
  _timerArmed = e.d;
  try {
    if (!_timerPatrol && _timerHeap.length) {
      _timerPatrol = true;
      _OPS.op_sleep(1500).then(_timerPatrolFn, _timerPatrolFn);
    }
    const wait = Math.max(0, e.d - Date.now());
    _OPS.op_sleep(wait).then(() => _timerWake(e.d), () => { _timerPatrolFn(); });
  } catch (err) {
    // The armed deadline was committed above; an op call that throws
    // synchronously would leave it claiming coverage with no sleep in
    // flight — the #240 corpse's exact state. Revert to the previous
    // coverage so the next schedule retries the arm instead of rotting
    // behind the gate.
    _wheelThrows++;
    _timerArmed = prev;
    throw err;
  }
};

const _scheduleAfter = (delay, level, fn) => {
  const d0 = Math.max(0, Number(delay) || 0);
  // Per HTML's timer-initialisation steps, a timer whose nesting level is
  // greater than 4 has its timeout clamped to 4ms. The level travels with the
  // CHAIN — a timer scheduled from inside a level-N timer task is level N+1 —
  // so reading a live counter at fire time would see it restored between
  // fires and never exceed 1; it must be captured at schedule time.
  const d = level > 4 ? Math.max(_TIMER_FLOOR_MS, d0) : d0;
  // setTimeout(0) must be a MACROtask: it runs on a later event-loop turn,
  // after the current task's entire microtask queue has drained — the same
  // task boundary a real browser provides. Delivering it as a microtask
  // breaks every scheduler built on the distinction (React 19's Scheduler
  // defers work through MessageChannel/setTimeout specifically to land on
  // that boundary), so even delay-0 timers round-trip through op_sleep:
  // a resolved async op is the cheapest true event-loop turn the runtime
  // offers.
  _timerPush({ d: Date.now() + d, s: _timerSeq++, f: fn });
  _timerArmNext();
};

// A timer/message delivery must land at a TASK BOUNDARY, never at a
// microtask checkpoint inside another task's synchronous window. The
// op_sleep continuation a MessageChannel post resolves to is a microtask:
// it fires at the next checkpoint, which on heavy pages is routinely
// INSIDE a synchronous event dispatch — React 16's scheduler then runs
// performSyncWorkOnRoot while CommitContext is set and throws #327
// ("This root is already working"), after which the root stops committing
// anything: every setState queues forever and the page looks frozen
// (taobao publish form, 2026-10-04). Browsers get this discipline free
// from the task queue; here the delivery itself re-arms until the
// dispatch/timer window it landed in has closed.
//
// #30, 2026-10-04: the first cut re-armed as `_OPS.op_sleep(0).then(attempt)`
// per checkpoint. A synchronous handler that itself makes many op calls
// triggers a checkpoint PER OP, each spawning another armed chain — fan-out,
// not deferral: fxg.jinritemai.com burned a full core re-arming ~320×/s
// (Promise.then counter: 2560 calls in 8s, all named `attempt`), starving
// the very event loop the trampoline was supposed to protect. The fix:
// exactly ONE armed chain (the `_mtArmed` gate), a FIFO the chain drains
// one item per turn, and a backoff past 8 consecutive defers so an
// op-heavy window cannot turn waiting into spinning. `_mtDepth` now also
// brackets the page's own synchronous dispatch loops (`_mtWindow`), so a
// delivery that would land mid-dispatchEvent defers instead — which is the
// #327 case the timer-only guard missed.
// #37, 2026-10-04: the drain is an async loop that yields to a microtask
// checkpoint between items (a browser drains microtasks after EVERY task) and
// to one true op turn every 64 items so the Rust side can deliver the op
// completions (fetch bodies, timer wheels) that feed the queue.
// #39, 2026-10-05: the #37 escape hatch was PUSH-COUNTED (retire after 1000
// stale pushes) — it needs the page to keep pushing. fxg's telemetry is a
// self-chained setTimeout: once the chain dies its callbacks stop running,
// so nothing ever schedules the next push. Observed live on a wedged create
// page: {qlen:160819, stale:0, armed:true} — 160k payloads rotting with the
// staleness counter frozen at zero, no re-arm ever. Healing now rests on a
// timestamp instead: the drain stamps every loop iteration, a push retires
// a chain that has made no progress for 250ms, and every arm launches an
// independent patrol sleep that heals an armed-but-silent chain even on a
// page with zero pushes. A patrol from generation N dies on any later arm,
// so retries cannot accumulate.
let _mtDepth = 0;
let _mtArmed = false;
const _mtQueue = [];
let _mtHead = 0;   // consume cursor: shift() memmoves the whole backing store, O(n) per item
const _mtQLen = () => _mtQueue.length - _mtHead;
const _mtCompact = () => { if (_mtHead && _mtHead >= _mtQueue.length - _mtHead) { _mtQueue.splice(0, _mtHead); _mtHead = 0; } };
let _mtDefers = 0;
let _mtBeats = 0;      // every drain-loop iteration
let _mtItems = 0;      // every payload executed (cursor read)
let _mtRearms = 0;     // every arm of a fresh chain
let _mtGen = 0;        // bumped to retire a chain that stopped progressing
let _mtStale = 0;      // pushes since an item last ran (diagnostics only)
let _mtLastBeat = 0;   // last drain-loop iteration (chain liveness timestamp)
const _mtDrain = async () => {
  const gen = _mtGen;
  let tick = 0;
  for (;;) {
    if (gen !== _mtGen) return;   // retired: a fresher chain owns the queue
    _mtLastBeat = Date.now();
    if (_mtDepth > 0) {
      _mtDefers++;
      await _OPS.op_sleep(_mtDefers > 8 ? 4 : 0);
      continue;
    }
    _mtDefers = 0;
    if (_mtQLen() === 0) { _mtArmed = false; _mtCompact(); return; }
    _mtDepth++;
    _mtItems++;
    _mtStale = 0;
    const _fn = _mtQueue[_mtHead++];
    try { _fn(); }
    catch (e) { globalThis.__diting_reportUncaughtError(e); }
    finally { _mtDepth--; }
    _mtBeats++;
    await Promise.resolve();
    if ((++tick & 63) === 0) await _OPS.op_sleep(0);
  }
};
const _mtArm = () => {
  _mtStale = 0;
  _mtArmed = true;
  _mtRearms++;
  _mtGen++;
  _mtLastBeat = Date.now();
  const gen = _mtGen;
  // The arm's op_sleep continuation can vanish (interrupt unwinds pending
  // ops without settling them); swallow a rejection the same way — the
  // patrol below is what recovers, never this .then itself.
  _OPS.op_sleep(0).then(_mtDrain, () => {});
  _OPS.op_sleep(1500).then(() => {
    if (_mtArmed && gen === _mtGen && Date.now() - _mtLastBeat > 1200) {
      _mtArm();
    }
  }, () => {});
};
const _runAsMacrotask = (fn) => {
  _mtQueue.push(fn);
  _mtStale++;
  // `armed` alone can lie about liveness: a chain whose op continuations
  // vanished stays armed forever. The timestamp is the truth — a healthy
  // chain iterates (draining or deferring) every few ms, so one push more
  // than 250ms after the last iteration is a dead chain. Retire and re-arm.
  if (_mtArmed && Date.now() - _mtLastBeat < 250) return;
  _mtArm();
};
// Bracket a synchronous dispatch window (listener loops): deliveries
// armed while the window is open defer until after it closes.
const _mtWindow = (body) => {
  _mtDepth++;
  try { return body(); } finally { _mtDepth--; }
};
// #39: terminate_execution unwinds the JS stack WITHOUT running finally
// blocks, so a bracket open at the fire (a payload or a dispatch window)
// leaks _mtDepth>0 forever and every later drain defers into infinity
// while looking alive (observed: defers climbing, qlen rotting). The
// engine calls this from its termination recovery points, where nothing
// JS is executing and any positive depth is necessarily leaked.
// #227: also re-arm when a backlog waits — recovery points are the one
// place that can retire a corpse chain, and a quiet page (its own timers
// died with the chain) never pushes to do it from _runAsMacrotask.
// #240: the timer wheel needs the same heal. A severed completion (or a
// latched patrol flag) leaves `_timerArmed` holding a stale deadline, so
// `_timerArmNext`'s `e.d >= _timerArmed` gate refuses every later timer
// — the wheel has NO self-heal in that state: new pushes don't retry the
// arm, the patrol flag blocks the patrol, and the page's every
// timer/interval/rAF rots for the rest of the session while fetches and
// evals stay fine. Unlatch the patrol and force a fresh arm for whatever
// is actually pending.
globalThis.__diting_mt_recover_termination = () => {
  _mtDepth = 0; __probeInflightFetches();
  if (_mtQLen()) _mtArm();
  _timerPatrol = false;
  if (_timerHeap.length) { _timerArmed = Infinity; _timerArmNext(); }
};
// #227 stall predicate for the engine pump — the only witness independent
// of JS continuations. A terminate severs the drain's op_sleep
// continuation AND its sibling 1500ms patrol together (the completion
// batch evaporates, cf. #226), and a quiet page pushes nothing, so the
// 250ms retire never fires: armed corpse, backlog rotting (observed live:
// qlen 160819 with stale:0). Healthy chains beat every few ms (draining)
// or every ~4ms (deferring inside a dispatch window); 600ms of silence
// with work queued can only be a corpse.
globalThis.__diting_mt_stalled = () =>
  _mtArmed && _mtQLen() > 0 && Date.now() - _mtLastBeat > 600;
// #240 wheel stall predicate, same witness contract: a healthy wheel runs
// a due heap-top within one event-loop turn of its deadline. A heap-top
// 600ms past due can only be a corpse (stale armed gate + latched
// patrol) — the pump force-recovers, which re-arms the wheel. The
// recover itself is idempotent, so a busy page that merely fell behind
// pays one redundant arm, nothing more.
globalThis.__diting_wheel_stalled = () => {
  const e = _timerHeap[0];
  return !!e && e.d <= Date.now() - 600;
};
// #240 face-2 detector: a fetch whose op settled (opok/operr — the outer
// promise resolved or rejected) but whose chained .then continuation
// (resume/fthrow) never ran. On a healthy realm the two are one microtask
// checkpoint apart — the chain callback drains in the SAME turn that ran
// opok — so a multi-second gap is impossible while the engine still pumps
// (evals drive the loop). 8s tolerates even a slow busy window between the
// two stages. The engine's loop-entry witness polls this; on a hit the
// dump below rides the warn line, so an unattended flow wedge names the
// stage it died at without a human eval.
globalThis.__diting_fetch_stalled = () => {
  const now = Date.now() % 100000;
  // Date.now()%1e5 wraps every 100s; age math must survive the wrap.
  const age = (t) => (t <= now ? now - t : now + 100000 - t);
  const seen = new Set();
  let stalled = false;
  for (let i = _fetchRing.length - 1; i >= 0; i--) {
    const [tag, fk, t] = _fetchRing[i];
    if (seen.has(fk)) continue;
    seen.add(fk);
    if ((tag === 'opok' || tag === 'operr' || tag === 'takeok') && age(t) > 8000
        && !_fetchStallReported.has(fk)) {
      // Latch per key: a lost continuation rots in the ring forever, and
      // the witness polls at every loop entry — without the latch the dump
      // would fire once per pump for the rest of the session. A NEW
      // stalled key still reports.
      _fetchStallReported.add(fk);
      stalled = true;
    }
    // armed/resp/resume/fthrow as the last stage of a key is a steady state
    // (in flight / completed) — only a settle-without-continuation rots.
  }
  return stalled;
};
const _fetchStallReported = new Set();
// Post-mortem bundle the witness dumps on any stall (#240): wheel counters,
// the fetch ring tail, and the in-flight key count, one JSON line.
globalThis.__diting_mt_dump = () => JSON.stringify({
  mt: globalThis.__MT_STATE ? __MT_STATE() : null,
  inflight: (typeof __inflightFetches !== 'undefined') ? __inflightFetches.size : null,
  fetchTail: _fetchRing.slice(-16),
});
// Debug probe for the trampoline's live state (#30 follow-up: macrotask
// deliveries dying while op+microtask paths stay alive — 2026-10-04 fxg).
// Read-only snapshot; keep names short, it rides on globalThis.
// #240: `wheel` exposes the timer wheel's state machine — the counters
// name the kill when a corpse re-forms (wakes/stale tell whether sleeps
// deliver, throws whether op calls fired synchronously, patrols whether
// the safety net lived).
globalThis.__MT_STATE = () => ({
  depth: _mtDepth, armed: _mtArmed, qlen: _mtQLen(), defers: _mtDefers,
  beats: _mtBeats, items: _mtItems, rearms: _mtRearms,
  gen: _mtGen, stale: _mtStale, lag: Date.now() - _mtLastBeat,
  wheel: {
    heap: _timerHeap.length,
    armedIn: _timerArmed === Infinity ? null : _timerArmed - Date.now(),
    patrol: _timerPatrol,
    wakes: _wheelWakes, staleWakes: _wheelStale,
    throws: _wheelThrows, patrols: _wheelPatrols,
  },
});

// Per HTML, a string timer handler is compiled and run as a classic script in
// global scope *at fire time*. Indirect eval ((0, eval)) runs in the true
// global scope, so top-level var/function declarations become globals (a
// `new Function(fn)` wrapper keeps them local); deferring to fire time also
// surfaces a SyntaxError when the timer elapses, matching a real browser,
// instead of swallowing it eagerly at scheduling (upstream #558). Function
// handlers pass through unchanged.
const _coerceTimerFn = (fn) => {
  if (typeof fn === "string") {
    const src = fn;
    return () => { (0, eval)(src); };
  }
  return typeof fn === "function" ? fn : null;
};

globalThis.setTimeout = (fn, delay = 0, ...args) => {
  const handler = _coerceTimerFn(fn);
  if (!handler) return ++_tid;
  const id = ++_tid;
  const level = _timerNesting + 1;
  _scheduleAfter(delay, level, () => {
    // The cleared check must live INSIDE the delivered payload: the
    // trampoline runs it one event-loop turn after this callback, and a
    // clearTimeout that lands in that window must still win.
    _runAsMacrotask(() => {
      if (_clearedTimers.has(id)) return;
      const prev = _timerNesting;
      _timerNesting = level;
      try { handler(...args); } catch(e) { globalThis.__diting_reportUncaughtError(e); }
      finally { _timerNesting = prev; }
    });
  });
  return id;
};

globalThis.clearTimeout = (id) => { _clearedTimers.add(id); };

globalThis.setInterval = (fn, delay = 0, ...args) => {
  const handler = _coerceTimerFn(fn);
  if (!handler) return ++_tid;
  const id = ++_tid;
  const base = Math.max(0, Number(delay) || 0);
  _intervals.add(id);
  let fires = 0;
  const tick = () => {
    if (!_intervals.has(id)) return;
    fires++;
    _runAsMacrotask(() => {
      // Same one-turn window as setTimeout: clearInterval called after
      // this payload queued but before it ran must still suppress the
      // tick (the string-handler interval test catches exactly this).
      if (!_intervals.has(id)) return;
      const prev = _timerNesting;
      _timerNesting = fires;
      try { handler(...args); } catch(e) { globalThis.__diting_reportUncaughtError(e); }
      finally { _timerNesting = prev; }
    });
    if (!_intervals.has(id)) return;
    // An interval's self-chain keeps counting as nested, so its cadence
    // floors at the nesting limit after the first few fires — matching how
    // Chrome tames setInterval(fn, 1).
    _scheduleAfter(base, fires + 1, tick);
  };
  _scheduleAfter(base, 1, tick);
  return id;
};

globalThis.clearInterval = (id) => { _intervals.delete(id); _clearedTimers.add(id); };
globalThis.requestAnimationFrame = (fn) => __diting_raf(fn);
globalThis.webkitRequestAnimationFrame = globalThis.requestAnimationFrame;
globalThis.cancelAnimationFrame = (id) => __diting_caf(id);
globalThis.queueMicrotask = globalThis.queueMicrotask || ((fn) => Promise.resolve().then(fn));

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
// probe. _hexv is defined later in this file; resolution happens at call time.
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

// Activation flips `checked` (and `indeterminate`) NATIVELY in Chrome — the
// C++ pre-click activation never runs any JS setter. Pages routinely install
// an own-property `checked` trap (React 16's `_valueTracker` is the canonical
// one: its setter mirrors every assignment into a shadow "last value" so
// change extraction can diff against it). Flipping through the plain
// assignment routes the activation into that trap, the tracker sees the new
// value BEFORE the click event dispatches, and the framework's diff comes up
// empty — every controlled radio/checkbox onChange goes deaf. Bypass
// instance-level traps by calling the prototype's native setter directly.
let _nativeCheckedSet = undefined; // lazy: setter fn | false (unavailable)
let _nativeIndeterminateSet = undefined;
function _nativeSet(el, cacheRef, prop, value) {
  if (cacheRef.v === undefined) {
    const d = Object.getOwnPropertyDescriptor(el.constructor.prototype, prop) ||
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, prop);
    cacheRef.v = d && typeof d.set === 'function' ? d.set : false;
  }
  if (cacheRef.v) cacheRef.v.call(el, value);
  else el[prop] = value;
}
function _activationSetChecked(el, value) {
  if (_nativeCheckedSet === undefined) _nativeCheckedSet = { v: undefined };
  _nativeSet(el, _nativeCheckedSet, 'checked', value);
}
function _activationSetIndeterminate(el, value) {
  if (_nativeIndeterminateSet === undefined) _nativeIndeterminateSet = { v: undefined };
  _nativeSet(el, _nativeIndeterminateSet, 'indeterminate', value);
}
function _preClickFlip(el) {
  if (!el || el.tagName !== 'INPUT' || _isFormControlDisabled(el)) return null;
  const type = (el.getAttribute('type') || '').toLowerCase();
  if (type !== 'checkbox' && type !== 'radio') return null;
  const st = { el: el, type: type, oldChecked: !!el.checked, oldIndeterminate: false, radioStates: null };
  if (type === 'radio') {
    const name = el.getAttribute('name') || '';
    if (name) {
      st.radioStates = [];
      const all = (el.ownerDocument || globalThis.document).querySelectorAll('input');
      for (let i = 0; i < all.length; i++) {
        const r = all[i];
        if ((r.getAttribute('type') || '').toLowerCase() !== 'radio') continue;
        if ((r.getAttribute('name') || '') !== name || r.form !== el.form) continue;
        st.radioStates.push([r, !!r.checked]);
        if (r !== el) _activationSetChecked(r, false);
      }
    }
    _activationSetChecked(el, true);
  } else {
    _activationSetChecked(el, !st.oldChecked);
    st.oldIndeterminate = !!el.indeterminate;
    _activationSetIndeterminate(el, false);
  }
  return st;
}

// A cancelled click restores every prior state the flip touched (the whole
// radio group included); checkbox activation also restores `indeterminate`.
// The restore rides the same native-setter bypass as the flip: a cancelled
// activation must not be visible to page-installed property traps either.
function _cancelPreClickFlip(st) {
  if (!st) return;
  if (st.radioStates) {
    for (let i = 0; i < st.radioStates.length; i++) _activationSetChecked(st.radioStates[i][0], st.radioStates[i][1]);
  } else {
    _activationSetChecked(st.el, st.oldChecked);
    if (st.type === 'checkbox') _activationSetIndeterminate(st.el, st.oldIndeterminate);
  }
}

// Commit a surviving flip: fire input/change when the checked state actually
// changed (an already-checked radio stays silent, like Chrome).
function _commitPreClickFlip(st) {
  if (!st) return;
  if (st.el.checked === st.oldChecked) return;
  try { st.el.dispatchEvent(new Event('input', {bubbles: true})); } catch (e) {}
  try { st.el.dispatchEvent(new Event('change', {bubbles: true})); } catch (e) {}
}

// Post-click activation behaviour for elements that are not checkable inputs:
// label forwarding to the labeled control, anchor navigation, submit buttons.
// Runs after the click dispatch survives uncanceled. Returns what activated
// ('label' | 'link' | 'submit') or null — the trusted CDP path uses the
// return value to gate its triple-click select-all tail.
function _postClickActivation(el) {
  if (globalThis.__summaryActivation(el)) return 'summary';
  const tag = el.tagName;
  // Interactive elements keep their own activation, so a control nested
  // inside a label toggles once instead of forwarding through the label
  // and firing twice.
  const selfInteractive = el.matches && el.matches(_LABELABLE + ',a');
  const labelHost = selfInteractive ? null : (tag === 'LABEL' ? el : (el.closest ? el.closest('label') : null));
  if (labelHost) {
    const control = _labeledControl(labelHost);
    // A disabled control has no activation behaviour — own attribute or
    // disabled through an ancestor fieldset both count.
    if (control && control !== el && !_isFormControlDisabled(control)) {
      control.click();
      return 'label';
    }
  }
  const link = tag === 'A' ? el : (el.closest ? el.closest('a[href]') : null);
  if (link) {
    const href = link.getAttribute('href');
    if (href && !href.startsWith('#') && !href.startsWith('javascript:')) {
      location.assign(href);
      return 'link';
    }
  }
  if (_isSubmitButton(el)) {
    const form = el.closest ? el.closest('form') : null;
    if (form) {
      if (typeof form.requestSubmit === 'function') {
        form.requestSubmit(el);
      } else if (typeof form.submit === 'function') {
        form.submit(el);
      }
      return 'submit';
    }
  }
  return null;
}

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
// _WINDOW_EVENT_NAMES below now owns the whole window on* surface — `in`
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

// WHATWG URL parsing/serialization is delegated to the Rust `url` crate via
// op_url_parse / op_url_set. The op returns the full component set as JSON; the
// constructor caches it so getters are plain field reads (no per-access op) and
// the hot paths (navigation, fetch, _resolveUrl) stay cheap. Returns null when
// the input is not a valid URL.
function _urlParseOp(url, base) {
  try {
    const s = _OPS.op_url_parse(String(url), (base === undefined || base === null) ? "" : String(base));
    const c = JSON.parse(s);
    return (c && c.ok) ? c : null;
  } catch (e) { return null; }
}
function _urlSetOp(href, part, value) {
  try {
    const s = _OPS.op_url_set(String(href), part, String(value));
    const c = JSON.parse(s);
    return (c && c.ok) ? c : null;
  } catch (e) { return null; }
}
// Returns just the resolved absolute URL string (no component JSON), or null on
// failure. Cheaper than _urlParseOp for callers that only need the href.
function _urlResolveOp(href, base) {
  try {
    const r = _OPS.op_url_resolve(String(href), (base === undefined || base === null) ? "" : String(base));
    return r ? r : null;
  } catch (e) { return null; }
}
if (typeof URL === 'undefined' || !URL.prototype || !URL.__diting) {
  const _URL = class URL {
    constructor(url, base) {
      // 刀87: Chrome 145 messages (k87-ctor-e/f) — argc first, then an
      // unparseable base is its own error (even when url itself is absolute).
      if (arguments.length < 1) throw new TypeError("Failed to construct 'URL': 1 argument required, but only 0 present.");
      if (base !== undefined && base !== null && !_urlParseOp(String(base), undefined)) throw new TypeError("Failed to construct 'URL': Invalid base URL");
      const c = _urlParseOp(url, base);
      if (!c) throw new TypeError("Failed to construct 'URL': Invalid URL");
      this._c = c;
      this._sp = null;
      __hideOwn(this);
    }
    get href() { return this._c.href; }
    set href(v) { const c = _urlParseOp(v, undefined); if (!c) throw new TypeError("Failed to set the 'href' property on 'URL': Invalid URL"); this._c = c; this._refreshSP(); }
    get protocol() { return this._c.protocol; }
    set protocol(v) { this._set('protocol', v); }
    get username() { return this._c.username; }
    set username(v) { this._set('username', v); }
    get password() { return this._c.password; }
    set password(v) { this._set('password', v); }
    get host() { return this._c.host; }
    set host(v) { this._set('host', v); }
    get hostname() { return this._c.hostname; }
    set hostname(v) { this._set('hostname', v); }
    get port() { return this._c.port; }
    set port(v) { this._set('port', v); }
    get pathname() { return this._c.pathname; }
    set pathname(v) { this._set('pathname', v); }
    get search() { return this._c.search; }
    set search(v) { this._set('search', v); this._refreshSP(); }
    get hash() { return this._c.hash; }
    set hash(v) { this._set('hash', v); }
    get origin() { return this._c.origin; }
    get searchParams() {
      if (!this._sp) { __def(this, '_sp', new URLSearchParams(this._c.search)); this._sp._url = this; }
      return this._sp;
    }
    _set(part, value) { const c = _urlSetOp(this._c.href, part, value); if (c) this._c = c; }
    // search changed on the URL side: refresh the bound searchParams contents.
    _refreshSP() { if (this._sp && this._sp._setFromString) this._sp._setFromString(this._c.search); }
    // searchParams mutated: write the serialized query back without re-refreshing.
    _updateSearch(qs) { this._set('search', qs ? ('?' + qs) : ''); }
    toString() { return this._c.href; }
    toJSON() { return this._c.href; }
    static createObjectURL() { return 'blob:null/fake-' + Math.random().toString(36).slice(2); }
    static revokeObjectURL() {}
    // WHATWG URL.parse: like the constructor but returns null instead of throwing.
    static parse(url, base) { const c = _urlParseOp(url, base); if (!c) return null; const u = Object.create(_URL.prototype); u._c = c; u._sp = null; return u; }
    static canParse(url, base) { return _urlParseOp(url, base) !== null; }
  };
  _URL.__diting = true;
  // (刀96) Chrome's URL is a legacy DOM ctor: calling it without new throws
  // the new-check message, NOT V8's natural class message (k96 probe). Expose
  // a function ctor wrapping the class — same prototype object, statics
  // copied in Chrome's SORDER, new.target routed through Reflect.construct
  // so subclasses keep their own prototype.
  const _URLw = function URL(...args) {
    if (!new.target) throw new TypeError("Failed to construct 'URL': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    // rest → spread keeps the REAL argument count: the class's argc ladder
    // ("1 argument required, but only 0 present.") reads arguments.length.
    return Reflect.construct(_URL, args, new.target);
  };
  for (const s of ['canParse', 'parse', 'createObjectURL', 'revokeObjectURL']) _URLw[s] = _URL[s];
  _URLw.prototype = _URL.prototype;
  _URL.prototype.constructor = _URLw;
  _URLw.__diting = true;
  Object.defineProperty(_URLw, 'length', { value: 1, configurable: true });
  _markNative(_URLw);
  globalThis.URL = _URLw;
}


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

if (typeof TextEncoder === 'undefined') {
  globalThis.TextEncoder = class TextEncoder {
    get encoding() { return 'utf-8'; }
    encode(str) {
      str = String(str);
      const buf = [];
      for (let i = 0; i < str.length; i++) {
        let c = str.charCodeAt(i);
        if (c < 0x80) buf.push(c);
        else if (c < 0x800) { buf.push(0xC0|(c>>6), 0x80|(c&0x3F)); }
        else if (c < 0xD800 || c >= 0xE000) { buf.push(0xE0|(c>>12), 0x80|((c>>6)&0x3F), 0x80|(c&0x3F)); }
        else { c = 0x10000 + (((c & 0x3FF) << 10) | (str.charCodeAt(++i) & 0x3FF)); buf.push(0xF0|(c>>18), 0x80|((c>>12)&0x3F), 0x80|((c>>6)&0x3F), 0x80|(c&0x3F)); }
      }
      return new Uint8Array(buf);
    }
    encodeInto(str, dest) { const enc = this.encode(str); dest.set(enc.slice(0, dest.length)); return { read: str.length, written: Math.min(enc.length, dest.length) }; }
  };
}
// Fast pure-JS UTF-8 decode (the common case: Response/Blob .text(), most
// pages). Avoids the op + JSON round trip for plain UTF-8.
// WHATWG UTF-8 decoder (non-fatal): malformed input yields U+FFFD per the
// spec's "maximal subpart" rule, never garbage code points. Chrome-identical
// for overlongs, surrogates (ED A0..BF), > U+10FFFF and truncated tails.
function _utf8DecodeBytes(bytes, start) {
  const n = bytes.length;
  let i = start | 0, out = '';
  const buf = [];
  const flush = () => { out += String.fromCharCode.apply(null, buf); buf.length = 0; };
  while (i < n) {
    const c = bytes[i++];
    if (c < 0x80) { buf.push(c); }
    else {
      let need, cp, lo = 0x80, hi = 0xBF;
      if (c >= 0xC2 && c <= 0xDF) { need = 1; cp = c & 0x1F; }
      else if (c >= 0xE0 && c <= 0xEF) { need = 2; cp = c & 0x0F; if (c === 0xE0) lo = 0xA0; else if (c === 0xED) hi = 0x9F; }
      else if (c >= 0xF0 && c <= 0xF4) { need = 3; cp = c & 0x07; if (c === 0xF0) lo = 0x90; else if (c === 0xF4) hi = 0x8F; }
      else { buf.push(0xFFFD); continue; }
      let ok = true;
      for (let k = 0; k < need; k++) {
        const b = i < n ? bytes[i] : -1;
        if (b < lo || b > hi) { ok = false; break; }
        lo = 0x80; hi = 0xBF; cp = (cp << 6) | (b & 0x3F); i++;
      }
      if (!ok) { buf.push(0xFFFD); continue; }
      if (cp > 0xFFFF) { const t = cp - 0x10000; buf.push(0xD800 + (t >> 10), 0xDC00 + (t & 0x3FF)); }
      else buf.push(cp);
    }
    if (buf.length >= 8192) flush();
  }
  flush();
  return out;
}
if (typeof TextDecoder === 'undefined') {
  globalThis.TextDecoder = class TextDecoder {
    constructor(label, options) {
      // No-arg construction (Response.text()/Blob.text() and most pages) is
      // UTF-8; skip the label-validation op on that hot path.
      let name;
      if (label === undefined) {
        name = 'utf-8';
      } else {
        name = _OPS.op_encoding_for_label(String(label));
        if (!name) throw new RangeError("Failed to construct 'TextDecoder': The encoding label provided ('" + label + "') is invalid.");
      }
      const o = options || {};
      // Plain assignment: lift_own.js turns these into prototype accessors.
      const ds = __diting_liftSlot(this); ds.encoding = name; ds.fatal = !!o.fatal; ds.ignoreBOM = !!o.ignoreBOM;
      // Bytes carried over between decode(..., {stream:true}) calls
      // (TextDecoderStream splits chunks at arbitrary byte offsets).
      this._pending = new Uint8Array(0);
      this._bomChecked = false;
      __hideOwn(this);
    }
    decode(input, options) {
      const stream = !!(options && options.stream);
      let bytes = input === undefined
        ? new Uint8Array(0)
        : ArrayBuffer.isView(input)
          ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
          : new Uint8Array(input);
      if (this._pending.length || stream) {
        const merged = new Uint8Array(this._pending.length + bytes.length);
        merged.set(this._pending);
        merged.set(bytes, this._pending.length);
        bytes = merged;
        if (stream) {
          if (this.encoding === 'utf-8' && !this.fatal) {
            // Emit only complete UTF-8 sequences; hold the incomplete tail.
            let cut = bytes.length;
            for (let i = bytes.length - 1; i >= 0 && i >= bytes.length - 4; i--) {
              const b = bytes[i];
              if ((b & 0xC0) === 0x80) continue;
              const need = b < 0x80 ? 0 : b < 0xE0 ? 1 : b < 0xF0 ? 2 : 3;
              if (i + need + 1 > bytes.length) cut = i;
              break;
            }
            this._pending = bytes.slice(cut);
            bytes = bytes.slice(0, cut);
          } else {
            // Legacy encodings / fatal mode: withhold output until flush.
            this._pending = bytes;
            return '';
          }
        } else {
          this._pending = new Uint8Array(0);
        }
      }
      if (!stream) this._pending = new Uint8Array(0);
      // Fast path: plain UTF-8, non-fatal (Response/Blob text, most pages).
      if (this.encoding === 'utf-8' && !this.fatal) {
        let off = 0;
        if (!this._bomChecked && bytes.length) {
          this._bomChecked = true;
          if (!this.ignoreBOM && bytes.length >= 3 && bytes[0] === 0xEF && bytes[1] === 0xBB && bytes[2] === 0xBF) off = 3;
        }
        return _utf8DecodeBytes(bytes, off);
      }
      // Legacy encodings / fatal mode: encoding_rs via the op.
      const r = JSON.parse(_OPS.op_text_decode(this.encoding, bytes, this.fatal, this.ignoreBOM));
      if (!r.ok) throw new TypeError("Failed to execute 'decode' on 'TextDecoder': The encoded data was not valid.");
      return r.v;
    }
  };
}

// React Router's SSR stream bootstrap does
// `new ReadableStream(...).pipeThrough(new TextEncoderStream())`; without
// these, hydration never runs and the whole client app is inert.
// Composition, not `extends TransformStream`: TransformStream is installed
// by a deno extension AFTER the snapshot is taken, so it does not exist yet
// when this file is evaluated at build time.
if (typeof TextEncoderStream === 'undefined') {
  globalThis.TextEncoderStream = class TextEncoderStream {
    constructor() {
      let pending = '';
      const enc = new TextEncoder();
      const ts = new TransformStream({
        transform(chunk, controller) {
          chunk = pending + String(chunk);
          pending = '';
          const last = chunk.charCodeAt(chunk.length - 1);
          if (last >= 0xD800 && last < 0xDC00) {
            pending = chunk[chunk.length - 1];
            chunk = chunk.slice(0, -1);
          }
          if (chunk) controller.enqueue(enc.encode(chunk));
        },
        flush(controller) {
          if (pending) controller.enqueue(enc.encode(pending));
        }
      });
      const es = __diting_liftSlot(this);
      es.readable = ts.readable;
      es.writable = ts.writable;
    }
    get encoding() { return 'utf-8'; }
  };
  globalThis.TextDecoderStream = class TextDecoderStream {
    constructor(label, options) {
      const dec = new TextDecoder(label, options);
      const ts = new TransformStream({
        transform(chunk, controller) {
          const out = dec.decode(chunk, { stream: true });
          if (out) controller.enqueue(out);
        },
        flush(controller) {
          const out = dec.decode();
          if (out) controller.enqueue(out);
        }
      });
      const ds2 = __diting_liftSlot(this);
      ds2.readable = ts.readable;
      ds2.writable = ts.writable;
      // 刀84: sealed ro members — internal writes go through the shared lift slot
      ds2.encoding = dec.encoding;
      ds2.fatal = dec.fatal;
      ds2.ignoreBOM = dec.ignoreBOM;
    }
  };
}

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
globalThis.__mutationObservers = [];
globalThis.MutationObserver = class MutationObserver {
  constructor(callback) {
    this._callback = callback;
    this._targets = [];
    this._records = [];
    __hideOwn(this);
  }
  observe(target, options) {
    // WebIDL validates parameter 1 first (Chrome: "parameter 1 is not of
    // type 'Node'."), then the dictionary needs at least one of the three
    // mutation kinds true — both messages pinned against Chrome 155 (#272).
    if (!target || typeof target._nid !== 'number') {
      throw new TypeError("Failed to execute 'observe' on 'MutationObserver': parameter 1 is not of type 'Node'.");
    }
    const o = options || {};
    // Chrome's dictionary coercion: the option-value keys imply their
    // family when the family's own key is ABSENT — attributeOldValue/
    // attributeFilter imply attributes (and records really are delivered
    // for {attributeOldValue:true} alone), characterDataOldValue implies
    // characterData. An explicitly-false family key kills the implication:
    // {attributes:false, attributeOldValue:true} throws (Chrome 155 probe).
    const effAttrs = o.attributes !== undefined
      ? !!o.attributes : (!!o.attributeOldValue || Array.isArray(o.attributeFilter));
    const effCd = o.characterData !== undefined ? !!o.characterData : !!o.characterDataOldValue;
    if (!o.childList && !effAttrs && !effCd) {
      throw new TypeError("Failed to execute 'observe' on 'MutationObserver': The options object must set at least one of 'attributes', 'characterData', or 'childList' to true.");
    }
    // Normalize once (the spec snapshots the dictionary at observe time):
    // the notify bridge reads the effective booleans, not the raw ones.
    const normalized = {
      childList: !!o.childList,
      attributes: effAttrs,
      characterData: effCd,
      subtree: !!o.subtree,
      attributeOldValue: !!o.attributeOldValue,
      characterDataOldValue: !!o.characterDataOldValue,
    };
    if (Array.isArray(o.attributeFilter)) normalized.attributeFilter = o.attributeFilter;
    // Re-observing the same target replaces the options (never stacks a
    // second entry — Chrome delivers one record per mutation per observer).
    this._targets = this._targets.filter(t => t.target !== target);
    this._targets.push({ target, options: normalized });
    if (globalThis.__mutationObservers.indexOf(this) < 0) globalThis.__mutationObservers.push(this);
  }
  disconnect() {
    this._targets = [];
    // Chrome empties the record queue on disconnect: a queued-but-undelivered
    // record is dropped (takeRecords returns [] after), and the flush loop
    // skips unregistered observers, so no late callback fires either.
    this._records = [];
    const idx = globalThis.__mutationObservers.indexOf(this);
    if (idx >= 0) globalThis.__mutationObservers.splice(idx, 1);
  }
  takeRecords() {
    const r = this._records.slice();
    this._records = [];
    return r;
  }
};
// All records queued in the same synchronous block deliver in ONE callback
// per observer, at one microtask checkpoint, in registration order (Chrome
// truth: 3 sync childList mutations → calls=1, records=3). Mutations during
// a callback re-arm the flush for the next checkpoint.
globalThis.__moFlush = function () {
  globalThis.__moFlushArmed = false;
  const list = globalThis.__mutationObservers.slice();
  for (const obs of list) {
    if (!obs._records.length) continue;
    const batch = obs._records.splice(0);
    try { obs._callback.call(obs, batch, obs); }
    catch (e) { globalThis.__diting_reportUncaughtError(e); }
  }
};
globalThis.__moFlushArmed = false;
// (#39) Chrome exposes WebKitMutationObserver as an alias of the same
// constructor (zone.js patches both names; it skips absent classes, but
// `typeof WebKitMutationObserver` must be 'function'). A data prop holding
// the shared function survives the V8 snapshot and shares its native mark.
globalThis.WebKitMutationObserver = globalThis.MutationObserver;
// prevNid/nextNid carry the record's sibling slots (post-insertion
// neighbours for additions, pre-removal neighbours for removals; null for
// replace-all records like innerHTML/textContent writes). oldValue arrives
// from the mutation site (previous attribute value / previous text) and is
// attached per observer only when that observer asked for it — Chrome
// leaves oldValue null for observers without attributeOldValue/
// characterDataOldValue.
globalThis.__notifyMutation = function(type, target_nid, addedNodes, removedNodes, attributeName, oldValue, prevNid, nextNid) {
  if (!globalThis.__mutationObservers.length) return;
  // Use `_wrap` (the canonical node-id → wrapper resolver) instead of a
  // direct cache poke. The previous code referenced `globalThis._cache`,
  // but `_cache` is a module-local Map — the lookup always returned
  // undefined, so the function silently bailed every time. Result: no
  // MutationObserver fired in obscura, ever, despite the call sites being
  // wired up at appendChild / setAttribute. _wrap also lazily creates a
  // wrapper for nodes that didn't have one yet (e.g. children parsed from
  // `set innerHTML`), which we need for record.target/added/removed.
  const target = _wrap(target_nid);
  if (!target) return;
  // Match observers FIRST and build the (allocating) records only if
  // someone wants this mutation — under attribute storms the overwhelming
  // majority of calls match nobody, and the record build (wrap + array
  // map/filter) was pure waste. Semantics unchanged: one record per
  // matching observer; observers whose options produce identical records
  // share one object (Chrome hands both observers the very same record
  // instance when their option sets agree — #272 probe).
  const matched = [];
  // Ancestor nids, collected at most once and shared by every subtree
  // root — one parentNode walk instead of one per observed root.
  let chain = -1;
  for (const obs of globalThis.__mutationObservers) {
    for (const t of obs._targets) {
      const root = t.target;
      if (!root) continue;
      // Filter by type per the observer options. Default behaviour matches
      // real MutationObserver: attribute mutations need options.attributes,
      // characterData mutations need options.characterData, childList
      // needs options.childList. #236: attributeFilter narrows the
      // attribute case — a filter of ['data-allowed'] must not deliver
      // 'data-other' records (spec: filter matches the attribute's
      // local name; ours are unqualified names, so a direct includes).
      const wantsType =
        (type === 'attributes' &&
          t.options.attributes &&
          (!Array.isArray(t.options.attributeFilter) ||
            t.options.attributeFilter.includes(attributeName))) ||
        (type === 'characterData' && t.options.characterData) ||
        (type === 'childList' && t.options.childList);
      if (!wantsType) continue;
      if (root._nid === target_nid) { matched.push([obs, t.options]); break; }
      if (t.options.subtree) {
        if (chain === -1) {
          chain = [];
          for (let cur = target.parentNode; cur; cur = cur.parentNode) chain.push(cur._nid);
        }
        let hit = false;
        for (let i = 0; i < chain.length; i++) {
          if (chain[i] === root._nid) { hit = true; break; }
        }
        if (hit) { matched.push([obs, t.options]); break; }
      }
    }
  }
  if (!matched.length) return;
  // One record object per distinct effective oldValue: observers with the
  // same gating share the instance (Chrome's identity sharing), observers
  // that gated differently get their own.
  const cache = new Map();
  for (const [obs, opts] of matched) {
    let eff = null;
    if (type === 'attributes' && opts.attributeOldValue) eff = oldValue ?? null;
    else if (type === 'characterData' && opts.characterDataOldValue) eff = oldValue ?? null;
    let record = cache.get(eff);
    if (!record) {
      record = __diting_mkMutationRecord({
        type: type, // 'childList', 'attributes', 'characterData'
        target: target,
        addedNodes: addedNodes ? __diting_mkNodeList(addedNodes.map(nid => _wrap(nid)).filter(Boolean)) : __diting_mkNodeList([]),
        removedNodes: removedNodes ? __diting_mkNodeList(removedNodes.map(nid => _wrap(nid)).filter(Boolean)) : __diting_mkNodeList([]),
        attributeName: attributeName || null,
        oldValue: eff,
        previousSibling: prevNid != null ? (_wrap(prevNid) || null) : null,
        nextSibling: nextNid != null ? (_wrap(nextNid) || null) : null,
      });
      cache.set(eff, record);
    }
    obs._records.push(record);
  }
  if (!globalThis.__moFlushArmed) {
    globalThis.__moFlushArmed = true;
    Promise.resolve().then(globalThis.__moFlush);
  }
};

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
if (typeof FormData === "undefined") globalThis.FormData = class FormData {
  // https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#constructing-form-data-set
  constructor(form, submitter) {
    this._d = [];
    if (!form) return;
    const els = form.elements || [];
    for (let i = 0; i < els.length; i++) {
      const el = els[i];
      if (!el || !el.name || el.disabled) continue;
      const tag = (el.tagName || "").toUpperCase();
      if (tag === "FIELDSET" || tag === "OBJECT") continue;
      const t = String(el.type || "").toLowerCase();
      if (t === "submit" || t === "button" || t === "reset" || t === "image") continue;
      if ((t === "checkbox" || t === "radio") && !el.checked) continue;
      if (t === "file") {
        // Spec "constructing the form data set": each selected file becomes
        // its own (name, blob) entry; an empty selection still contributes an
        // empty octet-stream part — upload endpoints key on the part's
        // presence, so skipping it entirely changes what the server sees.
        const fl = el.files;
        if (fl && fl.length) {
          for (const f of fl) this._d.push([el.name, f]);
        } else if (typeof File === "function") {
          this._d.push([el.name, new File([], "", { type: "application/octet-stream" })]);
        }
        continue;
      }
      if (tag === "SELECT" && el.selectedOptions) {
        let any = false;
        for (const opt of el.selectedOptions) { any = true; this._d.push([el.name, String(opt.value ?? "")]); }
        if (any) continue;
      }
      this._d.push([el.name, el.value != null ? String(el.value) : ""]);
    }
    // The submitter's name/value is included (it is the clicked button).
    if (submitter && submitter.name) this._d.push([submitter.name, String(submitter.value ?? "")]);
    __hideOwn(this);
  }
  // Blob values stay objects (spec: append(name, blobValue, filename));
  // everything else converts to USVString. Stringifying a File here used to
  // turn uploads into the literal "[object File]" on the wire.
  append(k, v, filename) {
    k = String(k);
    if (v != null && typeof v === "object" && typeof Blob === "function" && v instanceof Blob) {
      if (filename !== undefined) v = new File([v], String(filename), { type: v.type });
      this._d.push([k, v]);
    } else {
      this._d.push([k, String(v)]);
    }
  }
  set(k, v, filename) {
    k = String(k);
    if (v != null && typeof v === "object" && typeof Blob === "function" && v instanceof Blob) {
      if (filename !== undefined) v = new File([v], String(filename), { type: v.type });
    } else {
      v = String(v);
    }
    const i = this._d.findIndex(([a]) => a === k);
    if (i >= 0) this._d[i] = [k, v]; else this._d.push([k, v]);
  }
  delete(k){this._d=this._d.filter(([a])=>a!==k);}
  get(k){const e=this._d.find(([a])=>a===k);return e?e[1]:null;}
  getAll(k){return this._d.filter(([a])=>a===k).map(([,v])=>v);}
  has(k){return this._d.some(([a])=>a===k);}
  *entries(){for(let i=0;i<this._d.length;i++)yield this._d[i];}
  *keys(){for(const [k] of this._d)yield k;}
  *values(){for(const [,v] of this._d)yield v;}
  [Symbol.iterator](){return this.entries();}
  forEach(cb,thisArg){this._d.forEach(([k,v])=>cb.call(thisArg,v,k,this));}
};
// application/x-www-form-urlencoded serializer: like encodeURIComponent but
// space -> '+' and also percent-encoding the chars encodeURIComponent leaves
// bare ( ! ~ ' ( ) ), keeping the form-urlencoded safe set ( * - . _ ).
function _formEncode(s){
  return encodeURIComponent(String(s)).replace(/%20/g,'+').replace(/[!'()~]/g, c => '%' + c.charCodeAt(0).toString(16).toUpperCase());
}
function _hexv(c){ if(c>=48&&c<=57)return c-48; if(c>=65&&c<=70)return c-55; if(c>=97&&c<=102)return c-87; return -1; }
if (typeof URLSearchParams === "undefined") globalThis.URLSearchParams = class URLSearchParams {
  constructor(init=""){
    this._p=[];
    this._url=null; // set by URL.searchParams so mutations write back to the URL
    if (typeof URLSearchParams === 'function' && init instanceof URLSearchParams) {
      this._p = init._p.map(pair => [pair[0], pair[1]]);
    } else if(typeof init==="string"){
      this._parseString(init);
    } else if (init && typeof init[Symbol.iterator] === 'function') {
      for (const pair of init) {
        const a = Array.from(pair);
        if (a.length !== 2) throw new TypeError("Failed to construct 'URLSearchParams': Each query pair must be an iterable [name, value] tuple");
        this._p.push([String(a[0]), String(a[1])]);
      }
    } else if (init && typeof init === 'object') {
      Object.keys(init).forEach(k => this._p.push([String(k), String(init[k])]));
    }
    __hideOwn(this);
  }
  _decode(s){
    // application/x-www-form-urlencoded percent-decoding: decode each valid %XX
    // byte, leave invalid escapes literal (decodeURIComponent throws on the whole
    // string instead), '+' -> space, then UTF-8 decode the resulting bytes.
    s = String(s);
    const out = [];
    for (let i = 0; i < s.length; i++) {
      const c = s.charCodeAt(i);
      if (c === 0x2B) { out.push(0x20); }
      else if (c === 0x25 && i + 2 < s.length) {
        const a = _hexv(s.charCodeAt(i + 1)), b = _hexv(s.charCodeAt(i + 2));
        if (a >= 0 && b >= 0) { out.push(a * 16 + b); i += 2; } else { out.push(c); }
      } else if (c < 0x80) { out.push(c); }
      else { const e = new TextEncoder().encode(s[i]); for (let j = 0; j < e.length; j++) out.push(e[j]); }
    }
    try { return new TextDecoder().decode(new Uint8Array(out)); } catch (e) { return s; }
  }
  _parseString(s){
    s = String(s).replace(/^\?/, "");
    if (s === "") return;
    for (const pair of s.split("&")) {
      if (pair === "") continue;
      const i = pair.indexOf("=");
      const k = i === -1 ? pair : pair.slice(0, i);
      const v = i === -1 ? "" : pair.slice(i + 1);
      this._p.push([this._decode(k), this._decode(v)]);
    }
  }
  _setFromString(s){ this._p = []; this._parseString(s); }
  _notify(){ if (this._url) this._url._updateSearch(this.toString()); }
  append(k,v){ this._p.push([String(k),String(v)]); this._notify(); }
  get(k){k=String(k); const p=this._p.find(([key])=>key===k); return p?p[1]:null;}
  getAll(k){k=String(k); return this._p.filter(([key])=>key===k).map(pair=>pair[1]);}
  set(k,v){k=String(k); v=String(v); let done=false; const out=[]; for (const pair of this._p){ if(pair[0]===k){ if(!done){ out.push([k,v]); done=true; } } else out.push(pair); } if(!done) out.push([k,v]); this._p=out; this._notify(); }
  delete(k,v){k=String(k); const hv=(v!==undefined); v=String(v); this._p=this._p.filter(([key,val])=> hv ? !(key===k&&val===v) : key!==k); this._notify();}
  has(k,v){k=String(k); const hv=(v!==undefined); v=String(v); return this._p.some(([key,val])=> hv ? (key===k&&val===v) : key===k);}
  sort(){ this._p.sort((a,b)=> a[0]<b[0]?-1:(a[0]>b[0]?1:0)); this._notify(); }
  get size(){ return this._p.length; }
  toString(){return this._p.map(pair=>_formEncode(pair[0])+"="+_formEncode(pair[1])).join("&");}
  forEach(cb,thisArg){this._p.slice().forEach(pair=>cb.call(thisArg,pair[1],pair[0],this));}
  *entries(){ for (const pair of this._p) yield [pair[0],pair[1]]; }
  *keys(){ for (const pair of this._p) yield pair[0]; }
  *values(){ for (const pair of this._p) yield pair[1]; }
  [Symbol.iterator](){ return this.entries(); }
};

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

globalThis.crypto = globalThis.crypto || {
  // Fill an integer TypedArray from the OS CSPRNG. Filling the underlying bytes
  // (not per-element Math.random) keeps the distribution uniform across every
  // typed-array width and is actually cryptographically random.
  getRandomValues(arr) {
    if (!ArrayBuffer.isView(arr) || arr instanceof DataView ||
        arr instanceof Float32Array || arr instanceof Float64Array ||
        (typeof Float16Array !== 'undefined' && arr instanceof Float16Array)) {
      throw new DOMException("The provided ArrayBufferView is not an integer-typed array", "TypeMismatchError");
    }
    if (arr.byteLength > 65536) {
      throw new DOMException("The requested length exceeds 65536 bytes", "QuotaExceededError");
    }
    const bytes = _OPS.op_random_bytes(arr.byteLength);
    new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength).set(bytes);
    return arr;
  },
  randomUUID() {
    const b = _OPS.op_random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    let s = "";
    for (let i = 0; i < 16; i++) {
      s += (b[i] + 0x100).toString(16).slice(1);
      if (i === 3 || i === 5 || i === 7 || i === 9) s += "-";
    }
    return s;
  },
};
globalThis.reportError = globalThis.reportError || ((e) => console.error(e));

globalThis.screenX = 0; globalThis.screenY = 0;
globalThis.screenLeft = 0; globalThis.screenTop = 0;
globalThis.pageXOffset = 0; globalThis.pageYOffset = 0;
globalThis.scrollX = 0; globalThis.scrollY = 0;

// (#27) HTMLElement is a REAL layer between Element and the per-tag
// interfaces — Chrome's chain is HTMLDivElement → HTMLElement → Element →
// Node → EventTarget. Page code does `el instanceof HTMLElement` for feature
// tests and unknown tags (x-foo) report constructor.name "HTMLElement", so
// the alias `= Element` that used to live here was wrong twice: no
// HTMLElement level in the chain, and no per-interface prototype at all.
// The engine passes a numeric node id when it wraps a node; a non-number
// nid means page-side construction, which has exactly two legal faces
// (#55): a registry upgrade running a constructor (rebind to the live
// element) and `new (customElements.get(name))()` (construct a fresh node,
// born upgraded). Everything else stays the Illegal constructor Chrome
// throws for `new HTMLElement()` / `new HTMLDivElement()`.


// Escape on a modal dialog (obscura#952): the CDP keyDown Escape arm lands
// here. The close request goes to the modal dialog containing the focused
// element, else the last open modal dialog in document order (topmost
// opened). Steps mirror requestClose: cancelable `cancel`, then close unless
// default-prevented. Non-modal dialogs are untouched, matching Chrome.
globalThis.__diting_dialogEscapeClose = function () {
  let el = document.activeElement;
  let dlg = null;
  while (el) {
    if (el.localName === 'dialog' && el.hasAttribute && el.hasAttribute('open') && el._dialogModal) { dlg = el; break; }
    el = el.parentElement;
  }
  if (!dlg) {
    const all = document.querySelectorAll('dialog[open]');
    for (let i = all.length - 1; i >= 0; i--) {
      if (all[i]._dialogModal) { dlg = all[i]; break; }
    }
  }
  if (!dlg || typeof dlg.requestClose !== 'function') return false;
  dlg.requestClose();
  return true;
};

// Sequential focus navigation (#14, blitz#899): Tab's default action walks
// the focusable set in Chrome's tab order — positive tabindex first
// (ascending, DOM order on ties), then tabindex=0/natural candidates in
// DOM order — skipping disabled controls, hidden inputs, tabindex="-1",
// and anything hidden via display:none/visibility:hidden (ancestors
// included). Wraps at both ends; an empty set leaves focus alone.
globalThis.__diting_tabNavigate = function (shift) {
  const sel = 'a[href], area[href], button, input, select, textarea, iframe, [tabindex]';
  let nodes;
  try { nodes = document.querySelectorAll(sel); } catch (e) { return false; }
  const hidden = function (el) {
    for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
      const cs = getComputedStyle(n);
      if (cs.display === 'none' || cs.visibility === 'hidden') return true;
    }
    return false;
  };
  const positives = [];
  const naturals = [];
  for (let i = 0; i < nodes.length; i++) {
    const el = nodes[i];
    if (_isFormControlDisabled(el)) continue;
    if (el.tagName === 'INPUT' && String(el.type || '').toLowerCase() === 'hidden') continue;
    let ti = null;
    const raw = el.getAttribute('tabindex');
    if (raw != null) {
      const n = parseInt(raw, 10);
      if (!isNaN(n)) ti = n;
    }
    if (ti === -1) continue;
    if (hidden(el)) continue;
    if (ti != null && ti > 0) positives.push({el: el, ti: ti, i: i});
    else naturals.push({el: el, i: i});
  }
  positives.sort(function (a, b) { return a.ti - b.ti || a.i - b.i; });
  const order = positives.concat(naturals).map(function (e) { return e.el; });
  if (!order.length) return false;
  const cur = globalThis.__diting_focused;
  let idx = -1;
  for (let i = 0; i < order.length; i++) { if (order[i] === cur) { idx = i; break; } }
  let next;
  if (idx === -1) next = shift ? order.length - 1 : 0;
  else next = shift ? (idx - 1 + order.length) % order.length : (idx + 1) % order.length;
  order[next].focus();
  return true;
};
globalThis.GPUAdapter = class GPUAdapter {
  requestDevice() { return Promise.reject(new Error('GPUDevice unavailable')); }
};
globalThis.GPU = class GPU {
  requestAdapter() { return Promise.resolve(null); }
  getPreferredCanvasFormat() { return 'bgra8unorm'; }
};
globalThis.DOMStringMap = DOMStringMap;
// Build a NodeList (no named access, per spec) for querySelectorAll and
// childNodes. Kept light on purpose: querySelectorAll is the hottest query API.
globalThis.__diting_mkNodeList = (els) => _nodeList(els); // addedNodes/removedNodes are NodeLists in Chrome (Array before)
// 刀83: ctor_ladders seals NodeList behind a mint token (Chrome: any `new NodeList()`
// is an Illegal constructor); this factory is the engine's only legal minter.
globalThis.__diting_ctorMint = Symbol('ctor-mint');
function _nodeList(els) {
  const nl = new NodeList(globalThis.__diting_ctorMint);
  for (let i = 0; i < els.length; i++) nl[i] = els[i];
  nl._len = els.length;
  return nl;
}
// NodeList is its own type, not an Array subclass: in a real browser
// Array.isArray(nodeList) is false and Object.prototype.toString reports
// "[object NodeList]". Fingerprinting and feature-detection scripts check both.
// It keeps the array-like surface scripts actually use: indexed access, length,
// item(), forEach(), entries/keys/values, and iteration (so spread and for..of
// work).
__accPrivs.add('_len');
globalThis.NodeList = class NodeList {
  // (`_len` is registered in __accPrivs below so instance enumeration hides it.)
  // `length` is a prototype accessor over a hidden `_len` slot (Chrome: instances own only their indices).
  // 刀85: getter-only — Chrome's NodeList.length is ro; the factory writes `_len` directly.
  constructor() { Object.defineProperty(this, '_len', { value: 0, writable: true, configurable: true }); }
  get length() { return this._len; }
  item(i) { i = i >>> 0; return this[i] != null ? this[i] : null; }
  forEach(cb, thisArg) {
    for (let i = 0; i < this.length; i++) cb.call(thisArg, this[i], i, this);
  }
  *[Symbol.iterator]() { for (let i = 0; i < this.length; i++) yield this[i]; }
  *entries() { for (let i = 0; i < this.length; i++) yield [i, this[i]]; }
  *keys() { for (let i = 0; i < this.length; i++) yield i; }
  *values() { for (let i = 0; i < this.length; i++) yield this[i]; }
  get [Symbol.toStringTag]() { return 'NodeList'; }
};
_markNative(NodeList);
_markNative(NodeList.prototype.item);
_markNative(NodeList.prototype.forEach);

[
  globalThis.matchMedia,
  globalThis.requestAnimationFrame,
  globalThis.cancelAnimationFrame, globalThis.setTimeout, globalThis.clearTimeout,
  globalThis.setInterval, globalThis.clearInterval, globalThis.queueMicrotask,
  globalThis.structuredClone, globalThis.reportError,
  console.log, console.warn, console.error, console.info, console.debug,
  console.dir, console.assert,
  // (刀22) Notification + chrome.csi/loadTimes moved to bchk_apis.js —
  // their native marking migrated there (bootstrap loads first and would
  // see undefined here; the member reads also hid from the analyzer).
  MutationObserver, ResizeObserver, PerformanceObserver,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });



globalThis.opener = null;

globalThis.__blobStore = globalThis.__blobStore || {};
// Blob objects by URL, registered synchronously — Worker construction reads
// these so the createObjectURL → new Worker race can't lose.
globalThis.__blobObjs = globalThis.__blobObjs || {};
// Chrome shape: blob:https://<origin>/<uuid> (blob:null/ for opaque
// origins). The old blob:obscura/ prefix named the engine to any page
// that inspected the returned URL.
// Chrome mints v4 UUIDs (version nibble at 14, variant nibble at 19);
// a fully-random id fails strict UUID parsers.
function _mintBlobUrl() {
  const origin = (globalThis.location && globalThis.location.origin) || 'null';
  let u = '';
  while (u.length < 36) {
    if (u.length === 8 || u.length === 13 || u.length === 18 || u.length === 23) {
      u += '-';
    } else if (u.length === 14) {
      u += '4';
    } else if (u.length === 19) {
      u += '89ab'[Math.floor(Math.random() * 4)];
    } else {
      u += '0123456789abcdef'[Math.floor(Math.random() * 16)];
    }
  }
  return 'blob:' + origin + '/' + u;
}
URL.createObjectURL = function(blob) {
  // Chrome throws on missing / non-Blob input rather than minting a URL;
  // a silent fallback string was both a compat gap and a fingerprint.
  if (arguments.length === 0) {
    throw new TypeError("Failed to execute 'createObjectURL' on 'URL': 1 argument required, but only 0 present.");
  }
  // MediaSource rides the same registry (the MSE stub's attach handshake
  // looks the URL up when a media element's src is set).
  if (blob && typeof blob === 'object' && blob instanceof globalThis.MediaSource) {
    const id = _mintBlobUrl();
    globalThis.__mediaSources = globalThis.__mediaSources || {};
    globalThis.__mediaSources[id] = blob;
    return id;
  }
  if (!blob || typeof blob.text !== 'function') {
    throw new TypeError("Failed to execute 'createObjectURL' on 'URL': parameter 1 is not of type 'Blob'.");
  }
  const id = _mintBlobUrl();
  globalThis.__blobObjs[id] = blob;
  // Mirror into the Rust-side registry: import() and <script type=module>
  // of blob: URLs resolve through the module loader, which runs outside
  // this realm and cannot reach __blobObjs. Best-effort — the JS-side table
  // stays authoritative for fetch()/Worker.
  try {
    _OPS.op_blob_register(
      id,
      blob._bytes instanceof Uint8Array ? blob._bytes : new Uint8Array(0),
      String(blob.type || ''));
  } catch (_) {}
  blob.text().then(text => { globalThis.__blobStore[id] = text; });
  return id;
};
URL.revokeObjectURL = function(url) {
  delete globalThis.__blobStore[url];
  delete globalThis.__blobObjs[url];
  if (globalThis.__mediaSources) delete globalThis.__mediaSources[url];
  try { _OPS.op_blob_revoke(url); } catch (_) {}
};

// Window-level scrolling (upstream #468). The dominant infinite-scroll idiom
// is window-level — window.scrollTo(0, body.scrollHeight), window.scrollBy(0,
// 500), then a window 'scroll' listener — and no-op stubs meant the offset
// never moved, the sentinel never tripped, and automation that scrolled to
// trigger loading silently got one screenful.
//
// The page offset is stored on the scrolling element rather than in separate
// window state, so window.scrollY and document.scrollingElement.scrollTop are
// two views of one value, which is what pages assume. As with elements there
// is no layout, so the offset cannot be clamped to a real maximum.
function _scrollRoot() {
  const doc = globalThis.document;
  return (doc && doc.scrollingElement) || null;
}
function _windowScroll(x, y, relative) {
  const root = _scrollRoot();
  if (!root) return;
  const beforeLeft = root.scrollLeft || 0;
  const beforeTop = root.scrollTop || 0;
  let left, top;
  if (x !== null && typeof x === 'object') { left = x.left; top = x.top; }
  else { left = x; top = y; }
  // The scrollTop/scrollLeft setters fire their own element-path scroll when
  // not suppressed; the window path dispatches for itself below, so silence
  // the setter's implicit fire or the document hears the scrollend twice
  // (once bubbled from the root element, once dispatched).
  root._scrollSuppress = true;
  if (left !== undefined) {
    root.scrollLeft = (relative ? (root.scrollLeft || 0) : 0) + (+left || 0);
  }
  if (top !== undefined) {
    root.scrollTop = (relative ? (root.scrollTop || 0) : 0) + (+top || 0);
  }
  root._scrollSuppress = false;
  if ((root.scrollLeft || 0) === beforeLeft && (root.scrollTop || 0) === beforeTop) {
    return;
  }
  // Async, matching the element path. The scroll pair fires at the document
  // AND the window — scroll never bubbles, and Chrome fires it at both
  // separately. scrollend (blitz#354) trails in the same tick and BUBBLES:
  // since the M3 native core the document dispatch propagates to the window
  // (document → window path edge), so one dispatch at the document covers
  // both — the old second, explicit window dispatch would double-fire it.
  setTimeout(() => {
    try {
      const doc = globalThis.document;
      if (doc) doc.dispatchEvent(new Event('scroll', { bubbles: false }));
      globalThis.dispatchEvent(new Event('scroll', { bubbles: false }));
      if (doc) doc.dispatchEvent(new Event('scrollend', { bubbles: true }));
    } catch (e) {}
  }, 0);
}
globalThis.scrollTo = function(x, y) { _windowScroll(x, y, false); };
globalThis.scrollBy = function(x, y) { _windowScroll(x, y, true); };
globalThis.scroll = function(x, y) { _windowScroll(x, y, false); };
_markNative(globalThis.scrollTo);
_markNative(globalThis.scrollBy);
_markNative(globalThis.scroll);
// Read-only accessors, as on a real Window: assigning window.scrollY does not
// scroll the page. These replace the hard-coded 0 data properties defined
// earlier, so they must stay after them.
for (const [name, offset] of [
  ['scrollX', 'scrollLeft'], ['pageXOffset', 'scrollLeft'],
  ['scrollY', 'scrollTop'], ['pageYOffset', 'scrollTop'],
]) {
  Object.defineProperty(globalThis, name, {
    configurable: true,
    enumerable: true,
    get() { const root = _scrollRoot(); return root ? (root[offset] || 0) : 0; },
  });
}
globalThis.focus = function() {};
globalThis.blur = function() {};
globalThis.print = function() {};
// alert/confirm/prompt never block: the host answers each from the
// session-side dialog policy (default: dismiss) and records it as a
// level-"dialog" console entry. See op_dialog.
const _dlgArg = v => (v === undefined ? "undefined" : v === null ? "null" : String(v));
const _dialog = (kind, msg) => {
  let r;
  try { r = JSON.parse(_OPS.op_dialog(kind, _dlgArg(msg))); } catch { r = { accept: false, value: null }; }
  return r;
};
globalThis.alert = function(msg) { _dialog("alert", msg); };
globalThis.confirm = function(msg) { return _dialog("confirm", msg).accept; };
globalThis.prompt = function(msg, def) {
  const r = _dialog("prompt", msg);
  if (!r.accept) return null;
  if (r.value !== null && r.value !== undefined) return r.value;
  return def === undefined || def === null ? "" : String(def);
};
globalThis.open = function() { return null; };
globalThis.close = function() {};
globalThis.stop = function() {};
globalThis.postMessage = function(message, targetOrigin) {
  // Self-targeted postMessage (spec: window.postMessage to itself). Honor
  // the targetOrigin gate like the iframe path: '*'/'/'/matching origin
  // delivers; anything else drops silently (#704).
  let t = targetOrigin;
  if (t === undefined || t === null) t = '*';
  if (typeof t !== 'string' || t === '') return;
  if (t !== '*' && t !== '/') {
    const selfOrigin = (function() { try { return new URL(globalThis.location?.href || 'about:blank').origin; } catch(e) { return ''; } })();
    let tOrigin = '';
    try { tOrigin = new URL(t).origin; } catch(e) {}
    if (tOrigin === '' || tOrigin !== selfOrigin) return;
  }
  const event = new MessageEvent('message', {
    data: message,
    origin: (function() { try { return new URL(globalThis.location?.href || 'about:blank').origin; } catch(e) { return ''; } })(),
    source: globalThis,
  });
  Promise.resolve().then(() => { globalThis.dispatchEvent?.(event); });
};

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



// ---- form control validity and text selection ----

// Constraint validation (ValidityState, checkValidity, …) lives in form_validation.js.

// Text selection on Element.prototype
const _ns_selectionStart = new WeakMap();
const _ns_selectionEnd = new WeakMap();
const _ns_selectionDir = new WeakMap();

// Selection mirror (typing-cursor batch): the WeakMaps stay the JS-side
// truth for reads; every write also lands in the Rust tree so paint can
// resolve the caret at collect time. Offsets are the JS numbers verbatim
// (UTF-16 units — the Rust side clamps to the value's char count).
const _ns_mirrorSelection = (el) => {
  try {
    if (!el || el._nid === undefined) return;
    const s = _ns_selectionStart.get(el);
    const e = _ns_selectionEnd.get(el);
    _domRaw("set_selection", el._nid, (s ?? 0) + "," + (e ?? 0));
  } catch (_e) {}
};
// A control whose value is text the user can edit — the same type list the
// CDP insertText guard uses (input.rs).
const _ns_isTextEntry = (el) => {
  if (!el) return false;
  if (el.localName === 'textarea') return true;
  if (el.localName !== 'input') return false;
  const ty = String(el.getAttribute('type') || 'text').toLowerCase();
  return ['button','submit','reset','image','checkbox','radio','file','hidden','range','color'].indexOf(ty) < 0;
};

// The date/time input family runs HTML's value sanitization algorithm
// (agent-browser#2000 absorption): the DOM value only ever holds a complete
// valid string — Chrome stores '' for anything else, including the attribute
// default (<input type=date value="junk"> reads ''). Typing is the wrinkle:
// the CDP insertText/Backspace funnels compose `current + char` off .value,
// which would restart at '' on every keystroke, so an in-flight partial
// rides the _ns_dateDraft WeakMap — the funnels read their composition base
// through __diting_dateDraftBase and keep composing until the string
// validates, then it lands in .value and the draft clears (the engine-side
// equivalent of Chrome's segmented editor).
const _NS_DATE_VALUE_TYPES = ['date', 'month', 'week', 'time', 'datetime-local'];
const _ns_daysInMonth = (y, m) => {
  const leap = (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
  return [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];
};
// ISO week-numbering years with a W53: years starting on Thursday, or leap
// years starting on Wednesday (ISO 8601 §2.2.10 / Blink's week validation).
const _ns_isoWeeksInYear = (y) => {
  const p = (yy) => (yy + Math.floor(yy / 4) - Math.floor(yy / 100) + Math.floor(yy / 400)) % 7;
  return (p(y) === 4 || p(y - 1) === 3) ? 53 : 52;
};
// Normalized time-of-day: seconds (and the fraction) drop when zero,
// exactly like Chrome's "valid normalized local date and time string".
const _ns_normTime = (h, m, s, frac) =>
  (s !== undefined && (+s > 0 || frac)) ? h + ':' + m + ':' + s + (frac || '') : h + ':' + m;
// Returns the SANITIZED value: the normalized complete string, or '' when
// the input is a partial or out-of-range/garbage. Padding must be exact
// (2026-1-2 is not a valid date string), the calendar must be real (Feb 30
// rejected), and every component must be in range.
const _ns_sanitizeDateValue = (ty, raw) => {
  const s = String(raw);
  let m;
  if (ty === 'date') {
    if (!(m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s))) return '';
    const mo = +m[2], d = +m[3];
    return mo >= 1 && mo <= 12 && d >= 1 && d <= _ns_daysInMonth(+m[1], mo) ? s : '';
  }
  if (ty === 'month') {
    if (!(m = /^(\d{4})-(\d{2})$/.exec(s))) return '';
    return +m[2] >= 1 && +m[2] <= 12 ? s : '';
  }
  if (ty === 'week') {
    if (!(m = /^(\d{4})-W(\d{2})$/.exec(s))) return '';
    const w = +m[2];
    return w >= 1 && w <= _ns_isoWeeksInYear(+m[1]) ? s : '';
  }
  if (ty === 'time') {
    if (!(m = /^(\d{2}):(\d{2})(?::(\d{2})(\.\d+)?)?$/.exec(s))) return '';
    if (+m[1] > 23 || +m[2] > 59 || (m[3] !== undefined && +m[3] > 59)) return '';
    return _ns_normTime(m[1], m[2], m[3], m[4]);
  }
  if (ty === 'datetime-local') {
    // The spec grammar allows a space separator; the normalized form uses T.
    // (Groups: 1=date, 2=HH, 3=MM, 4=SS?, 5=fraction? — [T ] captures none.)
    if (!(m = /^(\d{4}-\d{2}-\d{2})[T ](\d{2}):(\d{2})(?::(\d{2})(\.\d+)?)?$/.exec(s))) return '';
    if (_ns_sanitizeDateValue('date', m[1]) === '') return '';
    if (+m[2] > 23 || +m[3] > 59 || (m[4] !== undefined && +m[4] > 59)) return '';
    return m[1] + 'T' + _ns_normTime(m[2], m[3], m[4], m[5]);
  }
  return s;
};
const _ns_dateDraft = new WeakMap();
// Engine-internal hook: the typing funnels (input.rs) read the composition
// base through this — a partial date value is '' in the DOM but keeps
// composing in the draft.
globalThis.__diting_dateDraftBase = function(el, fallback) {
  return _ns_dateDraft.has(el) ? _ns_dateDraft.get(el) : fallback;
};


// ---- Response.blob() on the real fetch path ----

if (typeof Response !== 'undefined' && Response.prototype && !Response.prototype.blob) {
  Response.prototype.blob = async function() {
    const bytes = await this.arrayBuffer();
    const contentType = this.headers && typeof this.headers.get === 'function' ? this.headers.get('content-type') : '';
    return new Blob([new Uint8Array(bytes)], { type: contentType || '' });
  };
  _markNative(Response.prototype.blob);
}
if (typeof Response !== 'undefined' && Response.prototype && !Response.prototype.text) {
  Response.prototype.text = async function() {
    const buffer = await this.arrayBuffer();
    return new TextDecoder().decode(new Uint8Array(buffer));
  };
  _markNative(Response.prototype.text);
}
if (typeof Response !== 'undefined' && Response.prototype && !Response.prototype.json) {
  Response.prototype.json = async function() {
    return JSON.parse(await this.text());
  };
  _markNative(Response.prototype.json);
}
// arrayBuffer is the body primitive that blob/text/json derive from; the
// engine's Response provides it natively, so it is intentionally not shimmed
// here (a JS fallback could only recurse into itself).

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
_domDeferred(function () {
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
  d('_NS_DATE_VALUE_TYPES', _NS_DATE_VALUE_TYPES);
  d('_OPS', _OPS);
  d('_WINDOW_EVENT_NAMES', _WINDOW_EVENT_NAMES);
  d('__acc', __acc);
  d('__accRo', __accRo);
  d('__currentUrl', __currentUrl);
  d('__def', __def);
  d('__ditingReorderProto', __ditingReorderProto);
  d('__hideOwn', __hideOwn);
  d('__locParts', __locParts);
  d('__prepareInsertedScript', __prepareInsertedScript);
  d('_cancelPreClickFlip', _cancelPreClickFlip);
  d('_commitPreClickFlip', _commitPreClickFlip);
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
  a('_fetchRing', () => _fetchRing, (v) => { _fetchRing = v; });
  d('_fetchStage', _fetchStage);
  d('_fileInputs', _fileInputs);
  d('_fireBlurFamily', _fireBlurFamily);
  d('_formChecked', _formChecked);
  d('_formIndeterminate', _formIndeterminate);
  d('_formValues', _formValues);
  d('_fp', _fp);
  a('_fpCache', () => _fpCache, (v) => { _fpCache = v; });
  d('_fpRand', _fpRand);
  a('_fpSeed', () => _fpSeed, (v) => { _fpSeed = v; });
  d('_getElementsByClassName', _getElementsByClassName);
  d('_getFp', _getFp);
  d('_isFormControlDisabled', _isFormControlDisabled);
  d('_isSubmitButton', _isSubmitButton);
  d('_labeledControl', _labeledControl);
  d('_locationObj', _locationObj);
  d('_makeFileList', _makeFileList);
  d('_markNativeProto', _markNativeProto);
  d('_mirrorSelectLabel', _mirrorSelectLabel);
  a('_mtDepth', () => _mtDepth, (v) => { _mtDepth = v; });
  d('_mtQueue', _mtQueue);
  d('_mtWindow', _mtWindow);
  d('_nodeInDocument', _nodeInDocument);
  d('_nodeList', _nodeList);
  d('_ns_dateDraft', _ns_dateDraft);
  d('_ns_isTextEntry', _ns_isTextEntry);
  d('_ns_mirrorSelection', _ns_mirrorSelection);
  d('_ns_sanitizeDateValue', _ns_sanitizeDateValue);
  d('_ns_selectionDir', _ns_selectionDir);
  d('_ns_selectionEnd', _ns_selectionEnd);
  d('_ns_selectionStart', _ns_selectionStart);
  d('_parserScriptHidden', _parserScriptHidden);
  d('_parserVisibleIds', _parserVisibleIds);
  d('_postClickActivation', _postClickActivation);
  d('_preClickFlip', _preClickFlip);
  d('_resolveUrl', _resolveUrl);
  d('_runAsMacrotask', _runAsMacrotask);
  d('_scheduleAfter', _scheduleAfter);
  d('_shallowCloneNode', _shallowCloneNode);
  d('_styleProxy', _styleProxy);
  d('_timerArmNext', _timerArmNext);
  a('_timerArmed', () => _timerArmed, (v) => { _timerArmed = v; });
  d('_timerHeap', _timerHeap);
  a('_timerPatrol', () => _timerPatrol, (v) => { _timerPatrol = v; });
  d('_urlParseOp', _urlParseOp);
  d('_urlResolveOp', _urlResolveOp);
  d('_urlSetOp', _urlSetOp);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
