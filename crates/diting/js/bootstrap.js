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

// Custom property names are case-sensitive and carry no camelCase IDL form
// (--mainColor must round-trip verbatim through style.setProperty/getPropertyValue).
const _cssCamelToKebab = (s) => {
  const str = String(s);
  return str.startsWith('--') ? str : str.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
};
const _cssKebabToCamel = (s) => String(s).replace(/-([a-z])/g, (_, c) => c.toUpperCase());

// Standard CSS property names (camelCase). Real CSSStyleDeclaration exposes
// every property as an enumerable accessor, so feature-detection code
// (`'gap' in el.style`) and enumeration (`Object.keys(el.style)`) see the whole
// set, not just the ones that happen to be assigned.
const _CSS_PROPERTY_NAMES = [
  "accentColor","alignContent","alignItems","alignSelf","all","animation","animationDelay",
  "animationDirection","animationDuration","animationFillMode","animationIterationCount",
  "animationName","animationPlayState","animationTimingFunction","appearance","aspectRatio",
  "backdropFilter","backfaceVisibility","background","backgroundAttachment","backgroundBlendMode",
  "backgroundClip","backgroundColor","backgroundImage","backgroundOrigin","backgroundPosition",
  "backgroundPositionX","backgroundPositionY","backgroundRepeat","backgroundSize","blockSize",
  "border","borderBlock","borderBlockColor","borderBlockEnd","borderBlockEndColor","borderBlockEndStyle",
  "borderBlockEndWidth","borderBlockStart","borderBlockStartColor","borderBlockStartStyle",
  "borderBlockStartWidth","borderBlockStyle","borderBlockWidth","borderBottom","borderBottomColor",
  "borderBottomLeftRadius","borderBottomRightRadius","borderBottomStyle","borderBottomWidth",
  "borderCollapse","borderColor","borderImage","borderImageOutset","borderImageRepeat",
  "borderImageSlice","borderImageSource","borderImageWidth","borderInline","borderInlineColor",
  "borderInlineEnd","borderInlineEndColor","borderInlineEndStyle","borderInlineEndWidth",
  "borderInlineStart","borderInlineStartColor","borderInlineStartStyle","borderInlineStartWidth",
  "borderInlineStyle","borderInlineWidth","borderLeft","borderLeftColor","borderLeftStyle",
  "borderLeftWidth","borderRadius","borderRight","borderRightColor","borderRightStyle",
  "borderRightWidth","borderSpacing","borderStyle","borderTop","borderTopColor","borderTopLeftRadius",
  "borderTopRightRadius","borderTopStyle","borderTopWidth","borderWidth","bottom","boxShadow",
  "boxSizing","breakAfter","breakBefore","breakInside","captionSide","caretColor","clear","clip",
  "clipPath","color","colorScheme","columnCount","columnFill","columnGap","columnRule","columnRuleColor",
  "columnRuleStyle","columnRuleWidth","columnSpan","columnWidth","columns","contain","container",
  "containerName","containerType","content","counterIncrement","counterReset","counterSet",
  "cursor","direction","display","emptyCells","filter","flex","flexBasis","flexDirection","flexFlow",
  "flexGrow","flexShrink","flexWrap","float","font","fontFamily","fontFeatureSettings","fontKerning",
  "fontOpticalSizing","fontSize","fontSizeAdjust","fontStretch","fontStyle","fontVariant",
  "fontVariantCaps","fontVariantLigatures","fontVariantNumeric","fontWeight","gap","grid","gridArea",
  "gridAutoColumns","gridAutoFlow","gridAutoRows","gridColumn","gridColumnEnd","gridColumnGap",
  "gridColumnStart","gridGap","gridRow","gridRowEnd","gridRowGap","gridRowStart","gridTemplate",
  "gridTemplateAreas","gridTemplateColumns","gridTemplateRows","height","hyphens","imageRendering",
  "inlineSize","inset","insetBlock","insetBlockEnd","insetBlockStart","insetInline","insetInlineEnd",
  "insetInlineStart","isolation","justifyContent","justifyItems","justifySelf","left","letterSpacing",
  "lineBreak","lineHeight","listStyle","listStyleImage","listStylePosition","listStyleType","margin",
  "marginBlock","marginBlockEnd","marginBlockStart","marginBottom","marginInline","marginInlineEnd",
  "marginInlineStart","marginLeft","marginRight","marginTop","mask","maxBlockSize","maxHeight",
  "maxInlineSize","maxWidth","minBlockSize","minHeight","minInlineSize","minWidth","mixBlendMode",
  "objectFit","objectPosition","offset","opacity","order","outline","outlineColor","outlineOffset",
  "outlineStyle","outlineWidth","overflow","overflowAnchor","overflowWrap","overflowX","overflowY",
  "overscrollBehavior","overscrollBehaviorBlock","overscrollBehaviorInline","overscrollBehaviorX",
  "overscrollBehaviorY","padding","paddingBlock","paddingBlockEnd","paddingBlockStart","paddingBottom",
  "paddingInline","paddingInlineEnd","paddingInlineStart","paddingLeft","paddingRight","paddingTop",
  "pageBreakAfter","pageBreakBefore","pageBreakInside","perspective","perspectiveOrigin","placeContent",
  "placeItems","placeSelf","pointerEvents","position","quotes","resize","right","rotate","rowGap",
  "scale","scrollBehavior","scrollMargin","scrollPadding","scrollSnapAlign","scrollSnapStop",
  "scrollSnapType","tabSize","tableLayout","textAlign","textAlignLast","textCombineUpright",
  "textDecoration","textDecorationColor","textDecorationLine","textDecorationSkipInk",
  "textDecorationStyle","textDecorationThickness","textEmphasis","textIndent","textJustify",
  "textOrientation","textOverflow","textRendering","textShadow","textTransform","textUnderlineOffset",
  "textUnderlinePosition","top","touchAction","transform","transformBox","transformOrigin",
  "transformStyle","transition","transitionDelay","transitionDuration","transitionProperty",
  "transitionTimingFunction","translate","unicodeBidi","userSelect","verticalAlign","visibility",
  "whiteSpace","width","willChange","wordBreak","wordSpacing","wordWrap","writingMode","zIndex","zoom",
];
const _CSS_PROP_SET = new Set(_CSS_PROPERTY_NAMES);
Object.defineProperty(globalThis, '__diting_cssNames', { value: { list: _CSS_PROPERTY_NAMES, set: _CSS_PROP_SET }, writable: true, enumerable: false, configurable: true });

// Parse a `style` attribute string (`"color: red; margin: 5px"`) into the given
// dashed-key store, replacing its contents in place.
function _parseCssInto(props, text) {
  for (const k in props) delete props[k];
  if (text) String(text).split(";").forEach((p) => {
    const i = p.indexOf(":");
    if (i > 0) {
      const k = _cssCamelToKebab(p.slice(0, i).trim());
      const v = p.slice(i + 1).trim();
      // Same door as setProperty: a garbage declaration in a style
      // attribute / cssText string is dropped individually, the way
      // Chrome's partial cssText parse keeps the valid neighbors.
      if (k && v && _cssDeclAccepted(k, v)) props[k] = v;
    }
  });
}
function _serializeCss(props) {
  const e = Object.entries(props);
  return e.length ? e.map(([k, v]) => `${k}: ${v}`).join("; ") + ";" : "";
}

// (刀72) `new CSSStyleDeclaration()` is an Illegal constructor in Chrome; the
// engine's own mints (element.style, rule styles, the computed fallback) go
// through _newCssDecl, which raises a one-shot flag around the construction.
let _cssDeclMint = false;
function _newCssDecl(owner) { _cssDeclMint = true; try { return new CSSStyleDeclaration(owner); } finally { _cssDeclMint = false; } }
Object.defineProperty(globalThis, '__diting_newCssDecl', { value: _newCssDecl, writable: true, enumerable: false, configurable: true });
class CSSStyleDeclaration {
  constructor(owner) {
    if (!_cssDeclMint) throw new TypeError("Failed to construct 'CSSStyleDeclaration': Illegal constructor");
    // Non-enumerable so they never leak through the proxy's own-key traps.
    Object.defineProperty(this, "_props", { value: {}, writable: true, enumerable: false, configurable: true });
    // The owner Element, if any. A live declaration reflects that element's
    // `style` content attribute in both directions; an owner-less declaration
    // (getComputedStyle fallback, stylesheet rules) is purely in-memory.
    Object.defineProperty(this, "_owner", { value: owner || null, writable: true, enumerable: false, configurable: true });
    // Last `style` attribute string we parsed/wrote, so a read can skip the
    // reparse when the attribute has not changed underneath us. Held in a
    // one-field object so the sync helpers can mutate it without a bare
    // `this._x = …` assignment (which the style proxy would reroute into
    // setProperty).
    Object.defineProperty(this, "_sync", { value: { last: null }, writable: true, enumerable: false, configurable: true });
    __hideOwn(this);
  }
  // Pull the owner's `style` attribute into `_props` if it changed since our
  // last read/write. Keeps parsed HTML and setAttribute('style', …) visible via
  // el.style.*. No-op when owner-less.
  _pull() {
    const o = this._owner;
    if (!o) return;
    const attr = o.getAttribute("style");
    if (attr === this._sync.last) return;
    _parseCssInto(this._props, attr);
    this._sync.last = attr;
  }
  // Serialize `_props` back onto the owner's `style` attribute after a mutation,
  // so el.style.x = … and cssText reflect into getAttribute('style') and
  // serialization. No-op when owner-less.
  _push() {
    const o = this._owner;
    if (!o) return;
    const text = _serializeCss(this._props);
    this._sync.last = text;
    if (text) o.setAttribute("style", text);
    else o.removeAttribute("style");
  }
  // Storage is keyed by the dashed CSS name, matching CSSOM. The proxy maps the
  // camelCase IDL access (el.style.fontSize) onto the dashed key (font-size), so
  // getPropertyValue('font-size') and el.style.fontSize stay in sync.
  setProperty(name, value) {
    this._pull();
    const k = _cssCamelToKebab(String(name));
    if (value === "" || value == null) delete this._props[k];
    else {
      const v = String(value);
      // #148: invalid values are silent no-ops — the stored declaration
      // (and the read-back) keeps the previous value, like Chrome.
      const c = k === "font-family" ? _cssFontFamilyCanon(v) : v;
      if (c !== null && _cssDeclAccepted(k, v)) this._props[k] = c;
    }
    this._push();
  }
  removeProperty(name) { this._pull(); const k = _cssCamelToKebab(String(name)); const old = this._props[k]; delete this._props[k]; this._push(); return old || ""; }
  getPropertyValue(name) { this._pull(); return this._props[_cssCamelToKebab(String(name))] || ""; }
  getPropertyPriority() { return ""; }
  get cssText() { this._pull(); return _serializeCss(this._props); }
  set cssText(v) {
    _parseCssInto(this._props, v);
    this._push();
  }
  get length() { this._pull(); return Object.keys(this._props).length; }
  item(i) { this._pull(); return Object.keys(this._props)[i] || ""; }
}
// (刀72) The CSS2Properties intermediate prototype is gone: the Chrome 145
// captures (shape-audit-truth-145, k72) show CSSStyleDeclaration straight over
// Object and no CSS2Properties interface at all (that level appears in later
// Chromes — the old comment cited a 155 capture; the target here is 145).
_markNativeProto(CSSStyleDeclaration.prototype);

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
// --- CSS animation lifecycle events (blitz#863 family 1) -----------------
// diting's animation engine samples styles at css_time (None = end state)
// and has no compositor loop, so by the time page script can observe an
// animation it has already finished. The lifecycle collapses to match:
// `animationstart` fires synchronously at the mutation that makes the
// animation observable (same policy as Element.animate below),
// `animationcancel` fires at the mutation that un-makes it (name changed,
// removed, or the element detached), and `animationend` on a timer carrying
// the declared active duration. Timers are armed only for elements that
// actually animate — animation-less pages (the overwhelming majority) never
// touch the timer system from these hooks. Per-mutation tick scheduling was
// tried and is forbidden: a pending 0ms timer keeps every event-loop slice
// of the nav settle pump timing out, so dynamic imports stop settling
// (their fetch never gets a slice), and reactor-less test isolates abort
// inside op_sleep. elapsedTime reports the declared active duration
// (delay + duration × iteration-count) from computed style, and
// `await animationend` scripts proceed. Iteration boundaries are collapsed
// too but *are* observable: for a finite animation of N iterations the N-1
// interior boundaries fire `animationiteration` from chained timers, and an
// infinite one chains without bound (a 0s duration has no representable
// boundary and is left unarmed — otherwise the chain would busy-spin).
// Comma-separated animation lists are tracked by their first name only.
const _animTracked = new Map();      // nid -> animation state
let _animScanDepth = 0;              // dispatch handlers mutate too — reentrancy guard
const _animDeferredChecks = new Set();

function _cssSeconds(v) {
  const s = String(v == null ? "" : v).trim();
  const n = parseFloat(s);
  if (isNaN(n)) return 0;
  return /ms\s*$/.test(s) ? n / 1000 : n;
}

function _dispatchAnimationEvent(el, type, name, elapsed) {
  try {
    el.dispatchEvent(new AnimationEvent(type, {
      bubbles: true, composed: true,
      animationName: name, elapsedTime: elapsed, pseudoElement: "",
    }));
  } catch (e) {}
}

function _animationNameOf(el) {
  try {
    const raw = String(getComputedStyle(el).getPropertyValue("animation-name") || "").trim();
    if (!raw || raw === "none") return null;
    const first = raw.split(",")[0].trim();
    return first && first !== "none" ? first : null;
  } catch (e) { return null; }
}

function _cancelAnimation(nid, st) {
  if (st.endTimer != null) clearTimeout(st.endTimer);
  if (st.iterTimer != null) clearTimeout(st.iterTimer);
  _animTracked.delete(nid);
  _dispatchAnimationEvent(st.el, "animationcancel", st.name, st.startElapsed);
}

function _startAnimation(el, name) {
  let duration = 0, delay = 0, infinite = false, iterations = 1;
  try {
    const cs = getComputedStyle(el);
    duration = Math.max(0, _cssSeconds(cs.getPropertyValue("animation-duration")));
    delay = _cssSeconds(cs.getPropertyValue("animation-delay"));
    const iterRaw = String(cs.getPropertyValue("animation-iteration-count") || "1").trim();
    infinite = iterRaw === "infinite";
    iterations = infinite ? 1 : (parseFloat(iterRaw) || 0);
  } catch (e) {}
  const startElapsed = delay < 0 ? -delay : 0;
  const st = { el, name, startElapsed, endTimer: null, iterTimer: null };
  _animTracked.set(el._nid, st);
  _dispatchAnimationEvent(el, "animationstart", name, startElapsed);
  // Iteration boundaries — the start of iteration k sits at delay + duration×k.
  // The chain re-arms one timer at a time, so an animating element holds at
  // most one pending iteration timer alongside its end timer: the same posture
  // as the end timer, and never a per-mutation tick. A 0s duration leaves no
  // representable boundary, so it stays unarmed rather than busy-spinning.
  if ((infinite || iterations > 1) && duration > 0) {
    const iterDelayMs = duration * 1000;
    const armIteration = (k, waitMs) => {
      st.iterTimer = setTimeout(() => {
        if (_animTracked.get(el._nid) !== st) return;
        st.iterTimer = null;
        _dispatchAnimationEvent(el, "animationiteration", name, Math.max(0, delay) + duration * k);
        if (infinite || k < iterations - 1) armIteration(k + 1, iterDelayMs);
      }, waitMs);
    };
    armIteration(1, (Math.max(0, delay) + duration) * 1000);
  }
  if (!infinite) {
    // End fires after the declared active duration (Chrome timing): that
    // also keeps a wall-clock window in which a cancel can still be
    // observed between start and end.
    const activeMs = (Math.max(0, delay) + duration * iterations) * 1000;
    st.endTimer = setTimeout(() => {
      if (_animTracked.get(el._nid) !== st) return;
      if (st.iterTimer != null) { clearTimeout(st.iterTimer); st.iterTimer = null; }
      _animTracked.delete(el._nid);
      // Detached before the duration ran out: the end never happened, the
      // removal did (Chrome fires animationcancel at detach; ours lands on
      // the next checkpoint via this timer).
      if (!_nodeInDocument(el)) {
        _dispatchAnimationEvent(el, "animationcancel", name, startElapsed);
        return;
      }
      _dispatchAnimationEvent(el, "animationend", name, Math.max(0, delay) + duration * iterations);
    }, activeMs);
  }
}

// One element: cancel a tracked animation whose name went away or changed,
// start a newly-observable one. Runs synchronously in the mutation.
// Per-epoch budget (#224): a MutationObserver-flooded page rebuilds subtrees
// every batch, and each candidate costs a full-cascade computed-style read
// (every tree write dropped the layout cache) — the animation sweep alone
// measured ~100% of a core on doudian's publish page. Past the cap, NEW
// animations defer to the next epoch (animationstart is async in Chrome too,
// and a flooding page bumps the epoch every batch, so the delay is a few ms);
// tracked elements still get their cancel/change handling unthrottled, and
// quiet pages never come near the cap — zero behavioral delta there.
let _animEpochSpent = -1, _animEpochBudget = 0, _animOverflowQueued = false;
const _animCappedTail = new Set();
const _ANIM_EPOCH_CAP = 512;
function _animationCheckOne(el) {
  if (!_nodeInDocument(el)) return;
  const existing = _animTracked.get(el._nid);
  if (_animEpochSpent !== _ditingMutationEpoch) {
    _animEpochSpent = _ditingMutationEpoch;
    _animEpochBudget = 0;
  }
  if (!existing && _animEpochBudget >= _ANIM_EPOCH_CAP) {
    // Park in a set the synchronous chain NEVER reads: pushing back into
    // _animDeferredChecks here would couple with checkIn's tail drain into a
    // drain↔checkIn mutual recursion on the capped element (stack overflow
    // at 600 inserts). The overflow macrotask merges it back with the budget
    // reset — a quiet page that batch-inserts past the cap still gets its
    // starts (nothing else will bump the epoch), and a flooding page is
    // throttled to ~one burst per tick with the loop yielded in between.
    _animCappedTail.add(el);
    if (!_animOverflowQueued) {
      _animOverflowQueued = true;
      setTimeout(() => {
        _animOverflowQueued = false;
        _animEpochBudget = 0;
        for (const e of _animCappedTail) _animDeferredChecks.add(e);
        _animCappedTail.clear();
        _drainDeferredAnimationChecks();
      }, 50);
    }
    return;
  }
  _animEpochBudget++;
  const name = _animationNameOf(el);
  if (existing && existing.name !== name) _cancelAnimation(el._nid, existing);
  if (name && (!existing || existing.name !== name)) _startAnimation(el, name);
}

// A page with no CSS anywhere cannot start an animation, but the check's
// computed-style read would still force a full layout run on every mutation
// batch — on style-less pages (deep-DOM fixtures, plain markup) that layout
// is the entire cost. The probe is cached per mutation epoch: <style>,
// <link>, [style] arrive or leave only through tree/attribute writes, which
// all bump the epoch, so the cache can't hide a newly-animatable page.
// Constructible stylesheets never reach the engine cascade (their rules are
// a JS-side shim) and WAAPI doesn't go through this path — both unaffected.
let _animCssProbeEpoch = -1, _animCssPossible = false, _animCssTransition = 0;
function _animationCssGate() {
  if (_animCssProbeEpoch === _ditingMutationEpoch) return _animCssPossible;
  const was = _animCssPossible;
  _animCssProbeEpoch = _ditingMutationEpoch;
  try {
    _animCssPossible = !!document.querySelector('style, link[rel~="stylesheet"], [style]');
  } catch (e) { _animCssPossible = true; }
  _animCssTransition = (!was && _animCssPossible) ? 1 : ((was && !_animCssPossible) ? -1 : 0);
  return _animCssPossible;
}

// Collect animation-capable elements (class or style attribute) from a
// subtree that just entered the document, plus the root itself. The cap
// bounds the per-element computed-style reads on big insertions.
function _animationCheckIn(root) {
  if (!root || root.nodeType !== 1 || !root.isConnected) return;
  if (_animScanDepth > 0) { _animDeferredChecks.add(root); return; }
  if (!_animationCssGate()) {
    // CSS just disappeared: anything still tracked lost its declarations —
    // cancel them the way Chrome does when its sheet is removed mid-run.
    if (_animTracked.size) _cancelAnimationsInSubtree(document.documentElement);
    return;
  }
  if (_animCssTransition > 0) {
    // CSS arrived after content was already checked through this gate
    // (async style injection): previously-skipped subtrees may animate now.
    // One capped whole-document sweep mirrors Chrome — animationstart
    // belongs to when the style lands, not when the element did. Riding the
    // depth guard defers it to the end of this very call.
    _animCssTransition = 0;
    if (root !== document.documentElement) {
      _animScanDepth = 1;
      try { _animationCheckIn(document.documentElement); } catch (e) {}
      _animScanDepth = 0;
      _drainDeferredAnimationChecks();
      return;
    }
  }
  _animScanDepth = 1;
  try {
    _animationCheckOne(root);
    let count = 1;
    for (const sel of ["[class]", "[style]"]) {
      const ids = _domParse("query_selector_all_scoped", root._nid, sel) || [];
      for (const id of ids) {
        if (count >= 128) break;
        const el = _wrapEl(+id);
        if (el) { _animationCheckOne(el); count++; }
      }
      if (count >= 128) break;
    }
  } catch (e) {}
  _animScanDepth = 0;
  _drainDeferredAnimationChecks();
}

// Attribute mutations concern the one element; subtree insertions the new
// root. Both are QUEUED, not run in the mutation: a script building a deep
// subtree via N appendChild calls would otherwise force N full layout runs
// (every tree write drops the layout cache, so each check's computed-style
// read re-lays-out the whole document — 2000 nested appends overflowed the
// stack inside layout). One microtask drain after the script's synchronous
// section does the work once on the final tree, which is also Chrome's own
// coalescing: style recalc and animationstart happen at the next rendering
// opportunity, never mid-task.
const _animPendingChecks = new Set();
let _animCheckQueued = false;
function _scheduleAnimationCheck(root) {
  if (!root || root.nodeType !== 1) return;
  _animPendingChecks.add(root);
  if (_animCheckQueued) return;
  _animCheckQueued = true;
  Promise.resolve().then(() => {
    _animCheckQueued = false;
    const batch = Array.from(_animPendingChecks);
    _animPendingChecks.clear();
    for (const r of batch) _animationCheckIn(r);
    // Transition diffs ride the same drain (one recompute per batch, the
    // elements already warm from the animation pass). Attribute-write
    // callers only — subtree insertions must not arm diffs.
    const tb = Array.from(_transPending);
    _transPending.clear();
    for (const el of tb) {
      try { _transitionCheckOne(el); } catch (e) {}
    }
  });
}

function _drainDeferredAnimationChecks() {
  if (_animDeferredChecks.size === 0) return;
  const pending = Array.from(_animDeferredChecks);
  _animDeferredChecks.clear();
  for (const el of pending) _animationCheckIn(el);
}

function _animNodeInSubtree(el, root) {
  let n = el;
  while (n) {
    if (n === root) return true;
    n = n.parentNode;
  }
  return false;
}

// Detachment cancels (Chrome semantics); _animTracked only ever holds
// elements that actually animated, so the walk is trivially small.
function _cancelAnimationsInSubtree(root) {
  if (!root || root.nodeType !== 1) return;
  for (const [nid, st] of Array.from(_animTracked)) {
    if (_animNodeInSubtree(st.el, root)) _cancelAnimation(nid, st);
  }
}

// --- CSS transition lifecycle (v1) ---------------------------------------
// A transition is defined by a *diff* between before/after computed values,
// which the engine's sampler cannot see — it only overrides the cascade at
// css_time. Trigger detection therefore lives here: every style/class
// attribute write queues the element, and the same microtask that drains
// animation checks diffs each queued element against its last observed
// values. One recompute per mutation batch (amortized, epoch-cached) — the
// same coalescing posture as animationstart; a per-write synchronous read
// was rejected because it forces a full cascade on every class write during
// hydration. Elements never observed before cannot diff, which matches
// Chrome: no transition on an element's first style resolution. v1 watches
// opacity/color/background-color/transform; CSSOM rule mutations surface
// on the next observation rather than at the rule change. transitioncancel
// fires the two ways Chrome fires it: a re-trigger on the same property
// preempts the in-flight entry, and removing the element from the document.
const _transWatched = ["opacity", "color", "background-color", "transform"];
const _transLast = new WeakMap();  // el -> {opacity, color, background-color, transform}
const _transPending = new Set();
const _transTracked = new Map();   // nid -> { el, timers: Map(prop -> {run,start,end}) }

function _dispatchTransitionEvent(el, type, prop, elapsed) {
  try {
    el.dispatchEvent(new TransitionEvent(type, {
      bubbles: true, composed: true,
      propertyName: prop, elapsedTime: elapsed, pseudoElement: "",
    }));
  } catch (e) {}
}

// Seconds the transition had been running when it was canceled (Chrome
// fills transitioncancel.elapsedTime with the active duration so far; a
// cancel during the delay window reports 0).
function _transElapsed(t) {
  if (t.startWall == null) return 0;
  return Math.max(0, (Date.now() - t.startWall) / 1000);
}

function _armTransitionEvents(el, prop, duration, delay) {
  let tr = _transTracked.get(el._nid);
  if (!tr) { tr = { el, timers: new Map() }; _transTracked.set(el._nid, tr); }
  const old = tr.timers.get(prop);
  if (old) {
    for (const t of [old.run, old.start, old.end]) if (t != null) clearTimeout(t);
    _dispatchTransitionEvent(el, "transitioncancel", prop, _transElapsed(old));
  }
  const t = { run: null, start: null, end: null, startWall: null };
  tr.timers.set(prop, t);
  _dispatchTransitionEvent(el, "transitionrun", prop, 0);
  const fireStart = () => {
    if (_transTracked.get(el._nid) !== tr || tr.timers.get(prop) !== t) return;
    t.startWall = Date.now();
    _dispatchTransitionEvent(el, "transitionstart", prop, 0);
  };
  if (delay > 0) t.start = setTimeout(fireStart, delay * 1000);
  else fireStart();
  t.end = setTimeout(() => {
    if (_transTracked.get(el._nid) !== tr || tr.timers.get(prop) !== t) return;
    tr.timers.delete(prop);
    if (!tr.timers.size) _transTracked.delete(el._nid);
    if (!_nodeInDocument(el)) return;
    _dispatchTransitionEvent(el, "transitionend", prop, duration);
  }, (delay + duration) * 1000);
}

// Detachment cancels a running transition (Chrome semantics): every
// pending timer in the removed subtree fires transitioncancel and its
// tracking entry goes away, exactly like animationcancel above.
function _cancelTransitionsInSubtree(root) {
  if (!root || root.nodeType !== 1) return;
  for (const [nid, tr] of Array.from(_transTracked)) {
    if (!_animNodeInSubtree(tr.el, root)) continue;
    for (const [prop, t] of Array.from(tr.timers)) {
      for (const k of [t.run, t.start, t.end]) if (k != null) clearTimeout(k);
      _dispatchTransitionEvent(tr.el, "transitioncancel", prop, _transElapsed(t));
    }
    _transTracked.delete(nid);
  }
}

function _transitionCheckOne(el) {
  if (!el || el.nodeType !== 1 || el._nid == null) return;
  if (!_nodeInDocument(el)) return;
  let snap = null;
  try { snap = getComputedStyle(el); } catch (e) { return; }
  const now = {};
  for (const p of _transWatched) now[p] = snap.getPropertyValue(p);
  const prev = _transLast.get(el);
  if (prev) {
    const durations = String(snap.getPropertyValue("transition-duration") || "").split(",").map(_cssSeconds);
    let durMax = 0;
    for (const d of durations) if (d > durMax) durMax = d;
    if (durMax > 0) {
      const props = String(snap.getPropertyValue("transition-property") || "all").split(",").map((s) => s.trim());
      const delays = String(snap.getPropertyValue("transition-delay") || "").split(",").map(_cssSeconds);
      const easings = String(snap.getPropertyValue("transition-timing-function") || "ease").split(",").map((s) => s.trim());
      for (const p of _transWatched) {
        if (String(now[p]) === String(prev[p])) continue;
        let k = props.indexOf(p);
        if (k < 0) k = props.indexOf("all");
        if (k < 0) continue;
        const duration = durations[k % durations.length] || 0;
        const delay = delays[k % delays.length] || 0;
        if (duration <= 0 && delay <= 0) continue;
        try {
          _dom("add_css_transition", String(el._nid), JSON.stringify({
            prop: p, from: prev[p], to: now[p],
            duration: duration * 1000, delay: delay * 1000,
            easing: easings[k % easings.length] || "ease",
          }));
        } catch (e) { continue; }
        _armTransitionEvents(el, p, duration, delay);
      }
    }
  }
  _transLast.set(el, now);
}

function _scheduleTransitionCheck(el) {
  if (!el || el.nodeType !== 1) return;
  _transPending.add(el);
}

// --- dynamic custom element upgrades (obscura #993 lineage) ------------
// define() only sweeps elements already in the document at definition time.
// Elements that appear LATER — document.createElement + append, markup set
// through innerHTML, subtrees built detached then attached — were never
// upgraded: the constructor never ran and connectedCallback never fired.
// That is exactly how craigslist's search shell hydrates (it defines
// <cl-search-result> at boot, then createElement's one per result inside
// the fetch callback), and any data-driven web-component render path hits
// the same hole. The walk rides the insertion hook that dynamic scripts
// already use, and is gated on a non-empty registry so pages defining no
// custom elements pay one boolean check.
function __customRegistry() {
  return (typeof customElements !== 'undefined' && customElements._registry) ? customElements._registry : null;
}
// The element a registry upgrade is currently running a constructor against.
// HTMLElement's constructor reads it (bootstrap tail): returning that element
// from the base constructor makes it the `this` of every derived constructor
// up the chain — native classes and Babel's _callSuper both honor a base
// constructor returning an object — so author field initializers land on the
// live element instead of dying on the old `cls.call(el)` (which throws for
// any real class: ES classes refuse .call, Babel's _classCallCheck refuses a
// foreign this). Save/restore, not clear: a constructor that appends another
// custom element triggers a nested upgrade that must restore ours.
let __ceUpgradeTarget = null;
// attributeChangedCallback capture/dispatch lives in the ce_attr platform
// module (define/_upgradeElement/writers call in via the __diting_ce* faces).
function __upgradeCustomElementsIn(root, includeRoot) {
  const reg = __customRegistry();
  if (!reg || !reg.size || !root) return;
  const queue = [];
  if (includeRoot) {
    queue.push(root);
  } else if (root.nodeType === 1 || root.nodeType === 11) {
    const kids = root.childNodes;
    for (let i = 0; i < kids.length; i++) queue.push(kids[i]);
  }
  // Cursor walk instead of recursion: hydration bursts insert deep markup
  // and the JS stack must not be the depth limit (cloneNode uses the same
  // posture). Children enqueue before an element's own callbacks run, so
  // upgrades land in tree order; children an upgrade itself appends arrive
  // through their own appendChild hook and re-visit idempotently here.
  for (let i = 0; i < queue.length; i++) {
    const el = queue[i];
    const kids = el.childNodes;
    for (let j = 0; j < kids.length; j++) queue.push(kids[j]);
    if (el.nodeType !== 1) continue;
    const cls = reg.get(el.localName);
    if (!cls) continue;
    if (!el.__customUpgraded) {
      // In-document: _upgradeElement fires connectedCallback itself.
      customElements._upgradeElement(el, cls);
    } else if (!el.__customConnected && typeof el.connectedCallback === 'function' && el.isConnected) {
      el.__customConnected = true;
      try { el.connectedCallback(); } catch (e) {}
    }
  }
}
// Removal is the mirror: upgraded elements leaving the document get
// disconnectedCallback and drop their connection flag, so a later
// re-insertion fires connectedCallback again (Chrome's connect/disconnect
// cycle — move/reshuffle logic in lit-style frameworks depends on it).
function __disconnectCustomElementsIn(root) {
  const reg = __customRegistry();
  if (!reg || !reg.size || !root) return;
  const queue = [root];
  for (let i = 0; i < queue.length; i++) {
    const el = queue[i];
    const kids = el.childNodes;
    for (let j = 0; j < kids.length; j++) queue.push(kids[j]);
    if (el.nodeType !== 1 || !el.__customConnected) continue;
    el.__customConnected = false;
    if (typeof el.disconnectedCallback === 'function') {
      try { el.disconnectedCallback(); } catch (e) {}
    }
  }
}
// ---- (#48) Window named access: window[id] / bare `id` identifier ----
// HTML5 named access on the Window object: elements with an id (any tag) or
// a name attribute (form/iframe/embed/object/img — the spec's shortlist)
// are reachable as window properties. Lives as accessor get-traps so reads
// reflect the tree at read time. Shadowing per spec: real own globals win —
// an explicit var/function declaration or assignment redefines our
// configurable slot into a data property, which is Chrome's resolution
// order (declaration > named access). Names in array-index form never
// become named properties.
const _namedEls = new Map();     // name -> Set<nid>, insertion order
const _namedGetters = new Map(); // name -> installed get fn (identity check)
let _namedScanned = false;       // one lazy full scan for the parser-built tree
// Chrome resolves named access through a WindowProperties layer between
// Window.prototype and EventTarget.prototype (final chain after rewire
// below): window.q works, but the name NEVER shows in
// getOwnPropertyNames(window) — the winKeys face only counts real globals.
// Own getters on globalThis leak every element id/name into that face, and
// a getter is never uninstalled, so the face only grows across a page's
// life. Park the getters on a proto layer: get/in/bare-identifier all
// walk the chain, page own props still shadow, and gopn(window) stays
// clean. Early install inserts the gate under globalThis temporarily
// (Window/EventTarget do not exist yet); the EventTarget rewire later
// moves it to Chrome order: window → Window.prototype → gate → ET.
// Chrome's WindowProperties layer is also EMPTY to its own enumeration
// (gopn at the layer = 0 entries — verified 10-17): resolution is pure
// interception, not own descriptors. A raw holder would leak every
// installed name to a proto-walking fingerprint (ark-class VMs walk the
// chain), so the layer is a gate that hides ONLY the named installs —
// ownKeys and gOPD return them as absent, everything else (page
// defineProperty on the layer) passes through untouched. get/has/
// defineProperty forward raw, so resolution, `in`, and foreign
// relocations behave exactly as before.
const _namedHolder = (function () {
  try {
    const P0 = Object.getPrototypeOf(globalThis);
    if (!P0) return globalThis; // defensive: no proto layer to park on
    const h = Object.create(P0);
    const gate = new Proxy(h, {
      ownKeys(t) { return Reflect.ownKeys(t).filter((k) => !_namedGetters.has(k)); },
      getOwnPropertyDescriptor(t, k) {
        if (_namedGetters.has(k)) return undefined;
        return Reflect.getOwnPropertyDescriptor(t, k);
      },
    });
    Object.setPrototypeOf(globalThis, gate);
    Object.defineProperty(globalThis, '__diting_namedHolder', { value: h, writable: true, enumerable: false, configurable: true });
    return h; // installs/uninstalls address the raw target
  } catch (e) { return globalThis; }
})();
function _namedNames(el) {
  if (!el || el.nodeType !== 1) return [];
  const out = [];
  const id = el.id;
  if (id) out.push(id);
  const t = el.tagName;
  if (t === 'FORM' || t === 'IFRAME' || t === 'EMBED' || t === 'OBJECT' || t === 'IMG') {
    const n = el.getAttribute('name');
    if (n) out.push(n);
  }
  return out;
}
function _namedInstall(name) {
  // Idempotence short-circuit: if our getter still owns the slot, the
  // defineProperty (plus fresh get/set closures) is pure waste — id/name
  // rewrite storms re-track the same names constantly.
  const cur = _namedGetters.get(name);
  const d = Object.getOwnPropertyDescriptor(_namedHolder, name);
  if (cur && d && d.get === cur) return;
  // A foreign own slot on the holder (the relocated EventTarget trio,
  // constructor, a page defineProperty on the layer, …) keeps the slot —
  // real faces win over named access, Chrome's resolution order.
  if (d && d.get !== cur) return;
  const get = () => _namedLookup(name);
  const set = (v) => {
    // Assignment lands as an own data prop on window (Chrome: named props
    // don't intercept [[Set]]), shadowing the named access from now on.
    try {
      Object.defineProperty(globalThis, name, { value: v, writable: true, configurable: true, enumerable: true });
    } catch (e) {}
  };
  _namedGetters.set(name, get);
  try {
    Object.defineProperty(_namedHolder, name, { get, set, configurable: true, enumerable: true });
  } catch (e) { _namedGetters.delete(name); }
}
function _namedTrack(el) {
  if (!el || el.nodeType !== 1) return;
  for (const n of _namedNames(el)) {
    // Array-index names never become named properties (spec).
    if (!n || /^\d+$/.test(n)) continue;
    let s = _namedEls.get(n);
    if (s) {
      // Already tracking this element under this name — the rewrite storm
      // re-tracks constantly; only a fresh (element, name) pair installs.
      if (s.has(el._nid)) continue;
      s.add(el._nid);
    } else {
      s = new Set(); _namedEls.set(n, s); s.add(el._nid);
    }
    _namedInstall(n);
  }
}
function _namedUntrackBy(nid, names) {
  for (const n of names) {
    const s = _namedEls.get(n);
    if (s) {
      s.delete(nid);
      if (!s.size) {
        _namedEls.delete(n);
        // Live-tree reflection: with no bearer left, pull the getter off
        // the holder so the named layer shrinks with the DOM (own props
        // on globalThis keep shadowing anything a page stored earlier).
        _namedGetters.delete(n);
        try { delete _namedHolder[n]; } catch (e) {}
      }
    }
  }
}
function _namedUntrack(el) {
  if (!el || el.nodeType !== 1) return;
  _namedUntrackBy(el._nid, _namedNames(el));
}
function _namedLookup(name) {
  const pick = () => {
    const s = _namedEls.get(name);
    if (!s) return undefined;
    for (const nid of s) {
      const el = _wrapEl(+nid);
      if (el && el.nodeType === 1 && el.isConnected) return el;
      s.delete(nid); // dead nid (innerHTML rebuilt the subtree)
    }
    if (!s.size) _namedEls.delete(name);
    return undefined;
  };
  // HTML spec named property access: an iframe (via id OR name) exposes its
  // nested browsing context — the WindowProxy, never the element itself.
  // window.frames === window, so frames['x'] lands here too (#48 face;
  // moli's 2026-10 cross-origin named-child cluster is the same surface).
  const unbox = (el) => {
    if (el && el.tagName === 'IFRAME') {
      try {
        const w = el.contentWindow;
        if (w) return w;
      } catch (e) {}
    }
    return el;
  };
  let el = pick();
  if (el) return unbox(el);
  if (!_namedScanned) {
    // Parser-built trees never pass through the mutation hooks, so the
    // first named miss pays one full scan. Later inserts/removals/attr
    // writes all go through the hooks, which keep the registry current.
    _namedScanned = true;
    try { _namedScanIn(globalThis.document); } catch (e) {}
    el = pick();
  }
  return unbox(el);
}
// #48: Chrome resolves bare identifiers and window[id] to id/name elements
// BEFORE any script runs — there is no "first touch" moment. The scan above
// piggybacks on lazy paths (first named miss, first `document` touch), but a
// script whose FIRST statement is a bare identifier (`hero.focus()`) reaches
// for the name before either lazy path can fire. So every Rust entry point
// that runs foreign code — the eval wrapper (runtime.rs INLINE_EVAL_JS, agent
// evaluate + CDP evaluate), callFunctionOn's wrapper, and the page script
// loop (page/scripts.rs) — calls this first. Touching `document` runs the
// getter above, which materializes the tree and pays the scan; if no DOM is
// attached yet the getter returns null WITHOUT setting the flag, so later
// calls retry. After the first success this is a one-flag no-op.
function _namedBoot() {
  if (!_namedScanned) globalThis.document;
}
function _namedScanIn(root) {
  if (!root) return;
  _namedTrack(root);
  // The document (nodeType 9) scans globally; an element scans its subtree.
  if ((root.nodeType === 1 || root.nodeType === 9) && typeof root.querySelectorAll === 'function') {
    const els = root.querySelectorAll('[id], form[name], iframe[name], embed[name], object[name], img[name]');
    for (let i = 0; i < els.length; i++) _namedTrack(els[i]);
  }
}
function _namedUnscanIn(root) { // removal: walk live children (root may be detached)
  if (!root) return;
  _namedUntrack(root);
  if (root.nodeType === 1) {
    const kids = root.childNodes || [];
    for (let i = 0; i < kids.length; i++) {
      if (kids[i] && kids[i].nodeType === 1) _namedUnscanIn(kids[i]);
    }
  }
}
function __prepareInsertedSubtree(root) {
  if (!root || !root.isConnected) return;
  __upgradeCustomElementsIn(root, true);
  const scripts = [];
  const seen = new Set();
  if (root.nodeType === 1 && root.tagName === 'SCRIPT') {
    scripts.push(root);
    seen.add(root._nid);
  }
  const ids = _domParse("query_selector_all_scoped", root._nid, "script") || [];
  for (const nid of ids) {
    const script = _wrapEl(+nid);
    if (script && !seen.has(script._nid)) {
      scripts.push(script);
      seen.add(script._nid);
    }
  }
  for (const script of scripts) __prepareInsertedScript(script);
  __prepareInsertedStylesheetLinksIn(root);
  __prepareInsertedStylesIn(root);
  // Elements entering the document start their CSS animations.
  _scheduleAnimationCheck(root);
}

// --- dynamic <style> load events ---------------------------------------
// Per HTML, a <style> element fires `load` (and its <link rel=stylesheet>
// sibling) once its sheet **and every @import** has loaded. diting's CSS
// parser drops @import outright (diting_css parses it away — there is no
// second fetch to wait on), so a connected <style>'s sheet is ready the
// moment its text is in the tree: the spec's "queue a task" collapses to a
// 0ms task. Loader-side code that awaits `style.onload` (common around
// @import-based theming) would otherwise hang forever on a pending promise.
// Firing happens only on connection, mirroring the <link> path's per-element
// guard; a text mutation of an already-connected <style> does not re-fire.
function __prepareInsertedStylesIn(root) {
  if (!root || !root.isConnected) return;
  const styles = [];
  const seen = new Set();
  if (root.nodeType === 1 && (root.tagName || '').toUpperCase() === 'STYLE') {
    styles.push(root);
    seen.add(root._nid);
  }
  const ids = _domParse("query_selector_all_scoped", root._nid, "style") || [];
  for (const nid of ids) {
    if (seen.has(+nid)) continue;
    const el = _wrapEl(+nid);
    if (el) { styles.push(el); seen.add(+nid); }
  }
  for (const el of styles) {
    setTimeout(() => {
      // Disconnected before the task ran: no sheet is live, so no load.
      if (_nodeInDocument(el)) __fireLinkLoadEvent(el, 'load');
    }, 0);
  }
}

// --- dynamic <link rel=stylesheet> -------------------------------------
// Chrome fetches a stylesheet link when it becomes connected (or its href
// changes while connected) and fires `load`/`error` on the element when the
// fetch settles — with no timeout. Loader-side waits on that event are
// load-bearing: webpack's mini-css chunk runtime (d.f.miniCss) resolves its
// chunk promise from the link's load event, so a missing event leaves the
// promise pending forever and freezes every lazy route behind it (tmall
// pc-shop-webapp renders a styled blank page exactly this way).
function __fireLinkLoadEvent(link, type) {
  // dispatchEvent alone routes the on* property handler too; a manual
  // property call ahead of it double-fires (same disease as the
  // dynamic-script load path, #121).
  try { link.dispatchEvent(new Event(type)); } catch (e) {}
}
function __prepareInsertedStylesheetLinksIn(root) {
  if (!root || !root.isConnected) return;
  const links = [];
  const seen = new Set();
  if (root.nodeType === 1 && root.tagName === 'LINK') { links.push(root); seen.add(root._nid); }
  const ids = _domParse("query_selector_all_scoped", root._nid, "link") || [];
  for (const nid of ids) {
    if (seen.has(+nid)) continue;
    const link = _wrapEl(+nid);
    if (link) { links.push(link); seen.add(+nid); }
  }
  // Re-insertion restarts a sheet's load in a browser (usually from cache),
  // so the per-element URL guard resets on the insertion path; the cached
  // sheet object from the previous load dies with it.
  for (const link of links) { delete link._sheetLoadUrl; delete link._sheetObj; __prepareInsertedStylesheetLink(link); }
}
function __prepareInsertedStylesheetLink(link) {
  if (!link || link.nodeType !== 1 || (link.tagName || '').toUpperCase() !== 'LINK') return;
  const relTokens = (link.getAttribute('rel') || '').toLowerCase().split(/\s+/);
  const isSheet = relTokens.indexOf('stylesheet') >= 0;
  // <link rel=preload>/<link rel=modulepreload> never applies a sheet — it
  // warms the fetch and reports settle. Loader-side waits on those events
  // are load-bearing the same way (webfont/modulepreload orchestrators
  // inject their dependency off onload); before #259 both settle paths
  // fired nothing, so waiters hung forever on success AND failure.
  const isPreload = relTokens.indexOf('preload') >= 0 || relTokens.indexOf('modulepreload') >= 0;
  if (!isSheet && !isPreload) return;
  const raw = link.getAttribute('href');
  if (!raw) return;
  let abs;
  try { abs = new URL(raw, globalThis.location?.href || 'http://localhost/').href; } catch (e) { return; }
  // One fetch per (element, URL); a later href change re-enters with the new
  // URL and starts a fresh load, like the browser.
  if (link._sheetLoadUrl === abs) return;
  link._sheetLoadUrl = abs;
  const fireLoad = () => __fireLinkLoadEvent(link, 'load');
  if (_extSheetBodies()[abs] != null) {
    // Already fetched at navigation — shells often preload the same CSS the
    // chunk runtime then re-requests under a differently-shaped URL string.
    // The load still fires, as a task like the completed fetch it stands in
    // for.
    setTimeout(fireLoad, 0);
    return;
  }
  // data: URLs never hit the network — settle as a 0ms task like an
  // already-cached fetch (Chrome fires load for data: preloads too).
  if (abs.startsWith('data:')) { setTimeout(fireLoad, 0); return; }
  const pageOrigin = (function() { try { return new URL(globalThis.location?.href || 'about:blank').origin; } catch (e) { return ''; } })();
  (async () => {
    try {
      // Same settle-loop bracket as dynamic <script src>: this fetch rides
      // the op-level client, invisible to the page client's
      // active_requests() counter.
      _OPS.op_dyn_script_fetch_begin();
      let body;
      try {
        // Stylesheet requests are no-cors with same-origin credentials (no
        // crossorigin attribute) — unlike classic scripts, whose JSONP-era
        // include policy is deliberately looser.
        // #126: one real sample; dynamic sheets still block first paint.
        const _rtT0 = globalThis.performance ? performance.now() : 0;
        const rawResp = await _OPS.op_fetch_url(abs, "GET", "{}", "", pageOrigin, "no-cors", "same-origin", "\u0000about:client");
        const parsed = JSON.parse(rawResp);
        __recordFetchTiming(_rtT0, abs, 'link', parsed, { renderBlocking: isSheet });
        if (!(parsed.status >= 200 && parsed.status <= 299)) {
          const why = parsed.error || parsed.corsError;
          throw new Error('HTTP ' + (parsed.status || 0) + (why ? ': ' + why : ''));
        }
        body = parsed.body;
      } finally {
        _OPS.op_dyn_script_fetch_end();
      }
      if (body != null) {
        // Sheets apply through the ext-sheet cache; a warmed preload parks
        // its body there too, so the <link rel=stylesheet> that follows the
        // preload hits the cache and settles instantly — the browser's
        // preload-then-use shape.
        _domRaw("ext_sheet_put", abs, body);
        if (_extSheetsMap) _extSheetsMap[abs] = body;
      }
      fireLoad();
    } catch (e) {
      console.error('Dynamic stylesheet fetch error (' + abs + '):', e.message);
      __fireLinkLoadEvent(link, 'error');
    }
  })();
}

// --- initial-parse stylesheets ------------------------------------------
// The parser-built tree never passes through the insertion hooks above, so
// <style>/<link rel=stylesheet> present in the first HTML never fired load.
// Chrome queues those load events as tasks once each sheet applies; an inline
// script running during the parse still catches them by registering a
// listener before the task runs. page.rs invokes this before the script
// execution loop, which reproduces that ordering: the 0ms tasks queue now
// and fire between script executions as the loop pumps the event loop.
function __prepareInitialStylesheets() {
  const root = globalThis.document && globalThis.document.documentElement;
  if (!root) return;
  __prepareInsertedStylesIn(root);
  __prepareInsertedStylesheetLinksIn(root);
}

// --- parser-path script load events --------------------------------------
// Parser-discovered <script src> never passes through the dynamic insertion
// hooks, so its element never received a 'load' event after execution. Pages
// bootstrap off sibling script elements that way — doudian's SSR emits
// <script defer src=main.js></script><script>document.currentScript
// .previousSibling.addEventListener('load', ...)</script> and its whole
// publish sub-app waits on the defer that hook resolves. Chrome fires the
// event as a task once the script has executed; the 0ms timer keeps that
// "an inline script running later in the parse still catches it" ordering.
// scripts.rs calls this after each external parser script executes ('load')
// and for fetches that never landed ('error').
globalThis.__fireParserScriptEvent = function(nid, type) {
  const el = _wrapEl(+nid);
  if (!el) return;
  setTimeout(function() {
    try { el.dispatchEvent(new Event(type)); } catch (e) {}
  }, 0);
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

const _iframeRegistry = [];
// srcdoc documents run their scripts — a document load, not innerHTML
// (#268). Inline scripts run synchronously in tree order; external ones are
// fetched (resolving against the PARENT base — about:srcdoc has no path of
// its own) and run parser-blocking style, each delaying the ones after it.
// The element's load event fires only after the last script finishes
// (obscura #1265: finish document work before iframe load).
function _runSrcdocScripts(el) {
  const doc = el._iframeDoc, win = el._iframeWin;
  const finish = () => el.dispatchEvent(new Event('load'));
  if (!doc || !win) { Promise.resolve().then(finish); return; }
  const scripts = doc._root ? doc._root.querySelectorAll('script') : [];
  const evalInFrame = (code, label) => {
    try {
      // A srcdoc document is its own global in a real browser; in the
      // single-realm shim the frame's window/document stand in as the
      // script's bindings (parent/top stay the host page's, frameElement
      // is the iframe). module scripts run as classic — best effort.
      const fn = new Function('window', 'document', 'self', 'parent', 'top',
        'frameElement', 'globalThis', code);
      fn.call(win, win, doc, win, globalThis, globalThis, el, win);
    } catch (e) {
      globalThis.__diting_reportUncaughtError?.(e);
    }
  };
  let idx = 0;
  const step = () => {
    while (idx < scripts.length) {
      const s = scripts[idx++];
      const type = (s.getAttribute('type') || 'text/javascript').toLowerCase();
      if (type !== 'text/javascript' && type !== 'module' &&
          type !== 'application/javascript' && type !== 'application/ecmascript') continue;
      const srcAttr = s.getAttribute('src');
      if (!srcAttr) { evalInFrame(s.textContent || '', 'srcdoc-inline'); continue; }
      const abs = (function() { try { return new URL(srcAttr, _docBase()).href; } catch (e) { return null; } })();
      if (!abs) continue;
      fetch(abs, { credentials: 'include' })
        .then(r => r.ok ? r.text() : Promise.reject(new Error('http ' + r.status)))
        .then(code => evalInFrame(code, abs), () => {})
        .then(step);
      return; // wait for the external script before continuing
    }
    finish();
  };
  Promise.resolve().then(step); // async start: document work first, then scripts
}
function _registerIframe(iframeEl) {
  // Idempotent: attachment points (appendChild/innerHTML) and src loading can
  // both reach the same element.
  if (iframeEl._iframeRegistered) return;
  iframeEl._iframeRegistered = true;
  const idx = _iframeRegistry.length;
  _iframeRegistry.push(iframeEl);
  globalThis.length = _iframeRegistry.length;
  Object.defineProperty(globalThis, idx, {
    get() {
      // Eagerly create the browsing context: detector code reads `self[i]`
      // right after inserting fixture markup, before ever touching
      // contentWindow — a null there reads as a missing iframe.
      if (!iframeEl._iframeWin && typeof iframeEl.contentWindow !== 'undefined') {
        void iframeEl.contentWindow;
      }
      return iframeEl._iframeWin || null;
    },
    configurable: true,
    enumerable: false,
  });
}
// Browsers create an iframe's browsing context when it is INSERTED INTO the
// document (not on src load, not on contentWindow access). Detector code
// captures `e = self.length`, appends fixture markup containing an iframe,
// then reads `self[e]` to get that iframe's window. Register iframes found in
// an inserted subtree, in tree order, so numeric window indexing matches.
function _registerIframesIn(node) {
  if (!node || typeof node.querySelectorAll !== 'function') return;
  const frames = node.nodeType === 1 && node.tagName === 'IFRAME'
    ? [node, ...node.querySelectorAll('iframe')]
    : [...node.querySelectorAll('iframe')];
  for (const f of frames) {
    _registerIframe(f);
    // Insertion doesn't just create the browsing context — an iframe that
    // already carries a src attribute starts navigating too. The src
    // property setter only sees JS assignment; framework writers attach
    // the attribute before insertion, so the load has to start here
    // (#121's sucai-selector iframe). _loadIframeSrc's flag collapses the
    // repeat against the post-navigation scan and the write-path scan.
    // srcdoc iframes navigate on insertion the same way (and win over src).
    if (f.hasAttribute('srcdoc')) { f._loadIframeSrc('about:srcdoc'); continue; }
    const s = f.getAttribute('src');
    if (s && s !== 'about:blank') f._loadIframeSrc(s); else __iframeBlankLoad(f);
  }
}
function _nodeInDocument(node) {
  let cur = node;
  let steps = 0;
  while (cur && steps++ < 200) {
    if (cur.nodeType === 9) return true;
    cur = cur.parentNode;
  }
  return false;
}
// Derive navigator.platform / userAgentData.platform from the UA so the JS
// layer's claimed OS matches the HTTP-layer UA. Mismatches (macOS UA +
// "Linux" platform) are a strong anti-bot signal (Baidu Wenku 安全验证).
function __ditingPlatformFromUA() {
  const ua = globalThis.__diting_ua || "";
  // (#40) Real Chrome on Windows reports navigator.platform "Win32" — on
  // 64-bit Windows and 64-bit Chrome alike. "Windows" is the
  // userAgentData.platform value (__ditingUADataPlatformFromUA's job) and
  // no real browser ever produces it here.
  if (ua.indexOf("Windows") !== -1) return "Win32";
  if (ua.indexOf("Macintosh") !== -1 || ua.indexOf("Mac OS X") !== -1) return "MacIntel";
  if (ua.indexOf("iPhone") !== -1 || ua.indexOf("iPad") !== -1) return "iPhone";
  if (ua.indexOf("Android") !== -1) return "Linux armv8l";
  return "Linux x86_64";
}
function __ditingUADataPlatformFromUA() {
  const ua = globalThis.__diting_ua || "";
  if (ua.indexOf("Windows") !== -1) return "Windows";
  if (ua.indexOf("Macintosh") !== -1 || ua.indexOf("Mac OS X") !== -1) return "macOS";
  if (ua.indexOf("Android") !== -1) return "Android";
  return "Linux";
}
// Chrome major from the UA. The profile pool ships one fleet major (154),
// and UA-CH brands hardcoding a different major made brand majors disagree
// with the UA — a one-line cross-check. Also feeds the fullVersionList build
// numbers (real Chrome reports four-part builds there, never x.0.0.0).
function __ditingChromeMajorFromUA() {
  const m = String(globalThis.__diting_ua || "").match(/Chrome\/(\d+)/);
  return m ? m[1] : "145";
}
// Chromium's UA-CH GREASE, computed the way user_agent_utils.cc does instead
// of pinning one generation's face: brand = "Not"+C[m%11]+"A"+C[(m+1)%11]
// +"Brand", grease version = [8,99,24][m%3], and the [grease, Chromium,
// browser] list is shuffled by ORD3[m%6] with shuffled[order[i]] = list[i].
// Verified byte-exact against real Chrome 145 / 153 / 154 / 155 (two CDP
// captures + upstream wreq-util tables). Both the JS face (brands/
// fullVersionList) and the engine's Sec-CH-UA header derive from this —
// a wrong grease name or order is a header-vs-JS tell.
const __DITING_GREASE_CHARS = [" ", "(", ":", "-", ".", "/", ")", ";", "=", "?", "_"];
const __DITING_GREASE_VERS = ["8", "99", "24"];
const __DITING_ORD3 = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
function __ditingBrandList(maj) {
  const grease = { brand: "Not" + __DITING_GREASE_CHARS[maj % 11] + "A" + __DITING_GREASE_CHARS[(maj + 1) % 11] + "Brand", version: __DITING_GREASE_VERS[maj % 3] };
  const list = [grease, { brand: "Chromium", version: String(maj) }, { brand: "Google Chrome", version: String(maj) }];
  const o = __DITING_ORD3[maj % 6];
  const out = [];
  for (let i = 0; i < 3; i++) out[o[i]] = list[i];
  return out;
}
// Real build.patch per Chrome major, taken from Google's Chrome for Testing known-good
// list (tools/webplatform/chrome/cft-builds.json; tools/webplatform/check_versions.py
// fails CI if an entry is not a published version). A claimed full version that never
// shipped (e.g. 145.0.6509.41: build 6509 is Chrome 127's) is a fingerprint tell, so
// this table must NEVER be extrapolated. Used ONLY by fullVersionList/uaFullVersion —
// the reduced UA itself stays x.0.0.0.
const __DITING_FULL_BUILD = { "130": "6723.116", "131": "6778.264", "132": "6834.159", "133": "6943.141", "134": "6998.165", "135": "7049.114", "136": "7103.113", "137": "7151.119", "138": "7204.183", "139": "7258.154", "140": "7339.207", "141": "7390.122", "142": "7444.175", "143": "7499.192", "144": "7559.133", "145": "7632.117", "146": "7680.165", "147": "7727.117", "148": "7778.178", "149": "7827.155", "150": "7871.124", "151": "7922.138", "152": "7977.82", "153": "8010.52", "154": "8037.92", "155": "8059.84", "156": "8078.73", "157": "8103.15", "158": "8127.2", "159": "8129.0" };

function __ditingNearestBuild(maj) {
  let best = null, bestD = Infinity;
  for (const k in __DITING_FULL_BUILD) { const d = Math.abs(Number(k) - maj); if (d < bestD) { bestD = d; best = __DITING_FULL_BUILD[k]; } }
  return best;
}

// ---- Font-presence metrics -------------------------------------------------
// Font-probe fingerprinters (Castle's fonts collector, fingerprintjs,
// WorkOS Radar) append a hidden 72px span and compare offsetWidth/offsetHeight
// across `'Font Name', serif/sans-serif/monospace` lists: an INSTALLED font
// changes the text box, a missing one falls back to the generic family and
// matches the baseline exactly. The installed sets below are the fonts a real
// Chrome can resolve per OS — Windows-only faces (Segoe UI, Calibri,
// Consolas) are absent on macOS and vice versa (Menlo, Monaco), so the
// detected set stays coherent with the UA persona.
const _DITING_FONT_PLATFORM = () => {
  const p = __ditingPlatformFromUA();
  if (p === "Win32") return "win";
  if (p === "MacIntel" || p === "iPhone") return "mac";
  return "linux";
};
const _INSTALLED_FONTS = {
  // fp-diff-1014 P1: a macOS-zh persona answered fonts.check false for
  // PingFang SC / Hiragino / STHeiti — fonts every Mac ships. These lists are
  // the single source for BOTH faces (span-measure widths above and
  // document.fonts below), so they can't drift apart. Mac includes the
  // Office/aftermarket CJK faces (YaHei/SimSun) common on zh machines —
  // missing system fonts is a hard tell, extra installable ones a soft none.
  mac: ["American Typewriter", "Andale Mono", "Apple Braille", "Apple SD Gothic Neo",
    "AppleGothic", "Apple Symbols", "Arial", "Arial Black", "Arial Narrow",
    "Arial Unicode MS", "Avenir", "Avenir Next", "Baskerville", "Big Caslon",
    "Brush Script MT", "Chalkboard", "Chalkduster", "Cochin", "Comic Sans MS",
    "Copperplate", "Courier New", "Futura", "Geneva", "Georgia", "Gill Sans",
    "Helvetica", "Helvetica Neue", "Hiragino Maru Gothic ProN", "Hiragino Sans",
    "Hiragino Sans CNS", "Hiragino Sans GB", "Hoefler Text", "Impact",
    "Kaiti SC", "Kaiti SC Black", "Lucida Grande", "Marker Felt", "Menlo",
    "Microsoft YaHei", "Monaco", "Apple Color Emoji", "Noto Color Emoji", "Optima", "Palatino",
    "Papyrus", "PingFang HK", "PingFang SC", "PingFang TC", "SimSun",
    "Snell Roundhand", "Songti SC", "STHeiti", "STHeiti Light", "STKaiti",
    "STSong", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana",
    "Yuanti SC", "Zapfino"],
  win: ["Arial", "Arial Black", "Arial Narrow", "Bahnschrift", "Calibri",
    "Calibri Light", "Cambria", "Cambria Math", "Candara", "Comic Sans MS",
    "Consolas", "Constantia", "Corbel", "Courier New", "Century Gothic",
    "DengXian", "Ebrima",
    "FangSong", "Franklin Gothic Medium", "Gabriola", "Gadugi", "Garamond",
    "Georgia", "Impact", "Ink Free", "Javanese Text", "KaiTi", "Leelawadee UI",
    "Lucida Console", "Lucida Sans Unicode", "Malgun Gothic", "Marlett",
    "Microsoft Himalaya", "Microsoft JhengHei", "Microsoft New Tai Lue",
    "Microsoft Sans Serif", "Microsoft YaHei", "Microsoft Yi Baiti",
    "MingLiU-ExtB", "Mongolian Baiti", "MS Gothic", "MS PGothic", "MS PMincho",
    "MV Boli", "Myanmar Text", "Nirmala UI", "NSimSun", "Palatino Linotype",
    "Segoe Print", "Segoe Script", "Segoe UI", "Segoe UI Emoji",
    "Segoe UI Historic", "Segoe UI Symbol", "SimHei", "SimSun", "Sitka",
    "Sylfaen", "Symbol", "Tahoma", "Times New Roman", "Trebuchet MS",
    "Verdana", "Webdings", "Wingdings", "Yu Gothic"],
  linux: ["Arial", "C059", "Cantarell", "Courier New", "DejaVu Sans",
    "DejaVu Sans Mono", "DejaVu Serif", "Droid Sans Fallback", "FreeSans",
    "FreeSerif", "Georgia", "Liberation Mono", "Liberation Sans",
    "Liberation Serif", "Noto Color Emoji", "Noto Mono", "Noto Sans",
    "Noto Sans CJK JP", "Noto Sans CJK SC", "Noto Serif", "Noto Serif CJK SC",
    "P052", "Times New Roman", "Trebuchet MS", "Ubuntu", "Ubuntu Mono",
    "Verdana", "WenQuanYi Micro Hei", "WenQuanYi Zen Hei"],
};
// Stable per-name width factor for an installed font, or null when the OS
// persona doesn't have it. Always ≥1% off the generic baseline (0.92–1.14,
// values inside ±1% nudged out) so the probe reliably detects installed
// faces, exactly like a real font's metrics never coincide with the fallback.
function _installedFontFactor(name) {
  const list = _INSTALLED_FONTS[_DITING_FONT_PLATFORM()] || _INSTALLED_FONTS.mac;
  if (list.indexOf(name) === -1) return null;
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (Math.imul(h, 31) + name.charCodeAt(i)) | 0;
  let f = 0.92 + ((h >>> 0) % 23) / 100;
  if (f > 0.99 && f < 1.01) f += 0.03;
  return f;
}
// Per-character advance widths (em) approximating Helvetica for lowercase,
// with caps/digits from the same metrics table. Enough spread for
// "mmMwWLli10Oo#@"-style probe strings to discriminate families.
const _FONT_CHAR_EM = (ch) => {
  switch (ch) {
    case "i": case "l": case "j": return 0.222;
    case "t": case "f": return 0.278;
    case "r": return 0.333;
    case "m": return 0.833;
    case "w": return 0.722;
    case "M": case "W": return 0.944;
    case "L": return 0.611;
    case "O": case "Q": case "G": case "D": case "U": case "H": case "B": case "N": case "R": case "S": case "E": case "K": return 0.722;
    case "C": case "P": case "F": case "A": case "V": case "X": case "Y": return 0.667;
    case "T": case "J": case "Z": case "I": return 0.611;
    case "o": case "0": case "1": case "#": case "e": case "a": case "s": case "c": case "u": case "n": case "d": case "g": case "p": case "q": case "b": case "h": return 0.556;
    case "@": return 0.833;
    default: return /[A-Z]/.test(ch) ? 0.7 : 0.5;
  }
};
// Generic-family width scale + line-height factor (em multiples).
const _GENERIC_FONT = {
  "serif": { w: 0.95, h: 1.15 },
  "sans-serif": { w: 1.0, h: 1.2 },
  "monospace": { w: 1.18, h: 1.21 },
  "cursive": { w: 1.02, h: 1.2 },
  "fantasy": { w: 1.04, h: 1.2 },
  "system-ui": { w: 1.0, h: 1.2 },
  "ui-sans-serif": { w: 1.0, h: 1.2 },
};
// Synthesize a leaf text element's content box from its inline font styles.
// Returns null for elements we shouldn't size this way (containers, no text)
// so their callers keep the default synthesized cell.
function _ditingFontBox(el) {
  try {
    const st = el.style;
    if (!st || !st._props) return null;
    const props = st._props;
    const display = props["display"];
    if (display === "none") return { w: 0, h: 0 };
    if (el.children && el.children.length) return null;
    // Only the font-probe shape (font-family set inline, size left to the
    // text): an element the page sized itself, or one with no font at all,
    // belongs to layout. Without this gate every inline-styled leaf with
    // text reported the text's box instead of its laid-out one.
    if (!(props["font-family"] || props["fontFamily"])) return null;
    if (props["width"] || props["height"] || props["min-width"] || props["min-height"]) return null;
    const text = (el.textContent || "").trim();
    if (!text) return null;
    const fontSize = parseFloat(props["font-size"] || props["fontSize"]) || 16;
    const famRaw = props["font-family"] || props["fontFamily"] || "serif";
    const names = String(famRaw).split(",").map((s) => s.trim().replace(/^['"]|['"]$/g, "")).filter(Boolean);
    // First installed family wins; otherwise the trailing generic.
    let chosen = null, generic = null;
    for (const n of names) {
      if (_GENERIC_FONT[n]) { generic = generic || n; continue; }
      if (chosen === null && _installedFontFactor(n) !== null) { chosen = n; break; }
    }
    if (!generic) generic = /sans/i.test(famRaw) ? "sans-serif" : "serif";
    const g = _GENERIC_FONT[generic] || _GENERIC_FONT["serif"];
    const ff = chosen ? _installedFontFactor(chosen) : 1;
    let w = 0;
    for (const ch of text) w += _FONT_CHAR_EM(ch);
    return {
      w: Math.round(w * fontSize * g.w * ff),
      h: Math.round(fontSize * g.h * (chosen ? 1 + (ff - 1) * 0.45 : 1)),
    };
  } catch { return null; }
}

// Real taffy-layout geometry in document coordinates — the same source
// getBoundingClientRect reads, but WITHOUT its __diting_click_target side
// effect: a plain size read must never stamp the click-target hint.
// Returns {x,y,w,h} or null (layout feature off / detached node).
// Fabricated iframe documents (obscura #976): walk up parentNode looking for
// the marker _IframeDocument stamps on its orphan root. Element.ownerDocument
// can't do this — it answers the MAIN document for every element, including
// iframe-doc ones.
function _ditingIframeDoc(el) {
  for (let n = el; n; n = n.parentNode) {
    if (n._isIframeDocRoot) return n._ownerDoc;
  }
  return null;
}

// The viewport a fabricated iframe document lays out against: the iframe
// ELEMENT's laid-out content box on the host page (Chrome semantics — a
// 400px-wide iframe gives its document a 400px viewport, and 100vh inside
// means that height). Measured through the main run's layout_rect (never
// the sub-run, so no recursion); the 300x150 replaced-box default stands
// until the host page has laid the iframe out. layout_rect is the border
// box, so the UA 2px border (and any padding) comes back off — a default
// iframe frames a 300x150 document, not 304x154.
function _ditingIframeViewport(el) {
  const doc = _ditingIframeDoc(el);
  const ifr = doc && doc._iframeEl;
  if (ifr && ifr._nid != null) {
    try {
      const raw = _domRaw("layout_rect", String(ifr._nid | 0), "");
      const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
      if (Array.isArray(arr) && arr.length === 4 && arr[2] > 0 && arr[3] > 0) {
        const cs = getComputedStyle(ifr);
        const px = (v) => parseFloat(v) || 0;
        const w = arr[2] - px(cs.borderLeftWidth) - px(cs.borderRightWidth)
                        - px(cs.paddingLeft) - px(cs.paddingRight);
        const h = arr[3] - px(cs.borderTopWidth) - px(cs.borderBottomWidth)
                        - px(cs.paddingTop) - px(cs.paddingBottom);
        if (w > 0 && h > 0) return { w, h, arg: w + "," + h };
      }
    } catch (e) { /* host layout unavailable */ }
  }
  return { w: 300, h: 150, arg: "300,150" };
}

function _ditingLayoutBox(el) {
  try {
    if (el._nid == null) return null;
    const idoc = _ditingIframeDoc(el);
    const raw = idoc
      ? _domRaw("iframe_layout_rect", String(el._nid | 0), _ditingIframeViewport(el).arg)
      : _domRaw("layout_rect", String(el._nid | 0), "");
    const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
    if (Array.isArray(arr) && arr.length === 4 && Number.isFinite(arr[0])) {
      return { x: arr[0], y: arr[1], w: arr[2], h: arr[3] };
    }
  } catch { /* no layout box */ }
  return null;
}

// Transform-free LOCAL box origin (blitz #663 family): offsetLeft/offsetTop
// ignore the element's own (and ancestors') paint transforms — they read
// the pre-map border box, which gBCR (transform-mapped) can't answer once
// rotate/skew land. Falls back to the mapped box when the element is
// boxless or the layout feature is off.
function _ditingLocalBox(el) {
  try {
    if (el._nid == null) return null;
    const idoc = _ditingIframeDoc(el);
    const raw = idoc
      ? _domRaw("iframe_local_geom", String(el._nid | 0), _ditingIframeViewport(el).arg)
      : _domRaw("local_geom", String(el._nid | 0), "");
    const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
    if (Array.isArray(arr) && arr.length === 10 && Number.isFinite(arr[0])) {
      return { x: arr[0], y: arr[1], w: arr[2], h: arr[3] };
    }
  } catch { /* no local geometry */ }
  return _ditingLayoutBox(el);
}

// CSSOM: offset*/client*/scroll* of a display:none element are all 0 — the
// init-in-hidden-container pattern (map libs size their container while
// hidden, re-measure on show) must read 0, not the 100x20 stub. Answered
// from the computed_style op so stylesheet-hidden elements count too, not
// just inline `style.display`.
function _ditingDisplayNone(el) {
  try {
    if (el._nid == null) return false;
    const raw = _domRaw("computed_style", String(el._nid | 0), "");
    const s = typeof raw === "string" ? JSON.parse(raw) : raw;
    return !!(s && s.display === "none");
  } catch { return false; }
}

// CSSOM View offsetParent (#128). The walk MUST terminate in null, never
// undefined: Fusion-style accumulators do `e = e.offsetParent;
// while (null !== e && e !== anchor)` — an undefined return passes the
// null test and the next iteration reads .offsetTop of undefined, killing
// the component mid-mount (tmall publish editor / upload selector).
// Chrome semantics mirrored: html/body, an unrendered chain (display:none),
// or position:fixed → null; otherwise the nearest ancestor that is a
// table cell/table or positioned, falling back to body. Detached subtrees
// and shadow-root boundaries fall out of the walk → null. Position/display
// come from the computed_style op (the _ditingDisplayNone precedent); with
// no layout data (blind build) every ancestor reads position undefined,
// degrading to "parent chain up to body" — still a null-terminated chain.
function _ditingOffsetParent(el) {
  try {
    if (el._nid == null) return null;
    const doc = el.ownerDocument;
    if (!doc) return null;
    if (el === doc.documentElement || el === doc.body) return null;
    const tagOf = (e) => (e.tagName || "").toUpperCase();
    if (tagOf(el) === "HTML" || tagOf(el) === "BODY") return null;
    const styleOf = (e) => {
      const raw = _domRaw("computed_style", String(e._nid | 0), "");
      const s = typeof raw === "string" ? JSON.parse(raw) : raw;
      return s || {};
    };
    const self = styleOf(el);
    if (self.display === "none" || self.position === "fixed") return null;
    let p = el.parentNode;
    while (p) {
      if (p.nodeType !== 1) return null; // Document/ShadowRoot: off-tree or shadow boundary
      if (p === doc.body) return p;
      const s = styleOf(p);
      if (s.display === "none") return null;
      const t = tagOf(p);
      if (t === "TD" || t === "TH" || t === "TABLE") return p;
      if (s.position && s.position !== "static") return p;
      p = p.parentNode;
    }
    return null; // ran off the top without hitting body: detached subtree
  } catch { return null; }
}

// PluginArray / MimeTypeArray / Plugin / MimeType — real browsers expose these
// constructors globally, and bot detectors reference them directly (absent →
// ReferenceError) or check `navigator.plugins instanceof PluginArray`.
// Making them real classes (not plain arrays) keeps instanceof working.
// (#125) Chrome's Plugin is array-like over its member MimeType objects with
// name/filename/description/length on the PROTOTYPE (Object.keys shows only
// the member indices), and both directions of the plugin↔mime graph are
// walkable: plugins[0][0].type, plugins[0].namedItem(...), and
// mimeTypes[0].enabledPlugin === navigator.plugins[0]. enabledPlugin=null is
// a shape Chrome never produces on the PDF persona.
globalThis.Plugin = class Plugin {
  // (#125 lineage) Chrome's Plugin instance owns ONLY the member indices
  // (enumerable) plus the mime-type names as non-enumerable named props —
  // `Object.getOwnPropertyNames(plugins[0])` is `0|1|application/pdf|text/pdf`.
  // name/filename/description/length live in a WeakMap behind prototype
  // getters; own `__plugin*` fields (fp-diff-1014 #4) were an instant
  // enumeration tell.
  constructor(name, filename, description, mimes) {
    __pluginStore.set(this, { name, filename, description, length: 0 });
    let n = 0;
    if (mimes) {
      for (const m of mimes) {
        Object.defineProperty(this, String(n++), { value: m, writable: true, configurable: true, enumerable: true });
        if (m && m.type) {
          Object.defineProperty(this, m.type, { value: m, writable: true, configurable: true, enumerable: false });
        }
      }
    }
    __pluginStore.get(this).length = n;
  }
  get name() { return __pluginStore.get(this).name; }
  get filename() { return __pluginStore.get(this).filename; }
  get description() { return __pluginStore.get(this).description; }
  get length() { return __pluginStore.get(this).length; }
  item(i) { return this[i] || null; }
  namedItem(type) {
    for (let i = 0; i < this.length; i++) {
      if (this[i] && this[i].type === type) return this[i];
    }
    return null;
  }
};
globalThis.Plugin.prototype[Symbol.toStringTag] = 'Plugin';
const __pluginStore = new WeakMap();
globalThis.MimeType = class MimeType {
  // WebIDL attributes are prototype accessors; the instance owns nothing
  // (fp-diff-1014 #4 family — own type/description/suffixes/enabledPlugin
  // fields were visible to every enumeration pass).
  constructor(type, description, suffixes, enabledPlugin) {
    __mimeStore.set(this, { type, description, suffixes, enabledPlugin });
  }
  get type() { return __mimeStore.get(this).type; }
  get description() { return __mimeStore.get(this).description; }
  get suffixes() { return __mimeStore.get(this).suffixes; }
  get enabledPlugin() { return __mimeStore.get(this).enabledPlugin; }
};
globalThis.MimeType.prototype[Symbol.toStringTag] = 'MimeType';
const __mimeStore = new WeakMap();
// NOT `extends Array`: Chrome's PluginArray/MimeTypeArray are platform
// array-likes, so `Array.isArray(navigator.plugins)` is false there and true
// here — a one-liner detector. Same shape for MimeTypeArray below.
globalThis.PluginArray = class PluginArray {
  constructor(...items) {
    __pluginArrStore.set(this, items);
    items.forEach((p, i) => Object.defineProperty(this, String(i), { value: p, writable: true, configurable: true, enumerable: true }));
    // Named properties (legacy platform object): own but NOT enumerable, first plugin of a name wins.
    items.forEach((p) => { if (!Object.prototype.hasOwnProperty.call(this, p.name)) Object.defineProperty(this, p.name, { value: p, writable: true, configurable: true, enumerable: false }); });
  }
  get length() { return __pluginArrStore.get(this).length; }
  item(i) { return this[i] || null; }
  namedItem(name) {
    const a = __pluginArrStore.get(this);
    for (let i = 0; i < a.length; i++) if (a[i].name === name) return a[i];
    return null;
  }
  refresh() {}
};
globalThis.PluginArray.prototype[Symbol.toStringTag] = 'PluginArray';
const __pluginArrStore = new WeakMap();
globalThis.MimeTypeArray = class MimeTypeArray {
  constructor(...items) {
    __mimeArrStore.set(this, items);
    items.forEach((m, i) => Object.defineProperty(this, String(i), { value: m, writable: true, configurable: true, enumerable: true }));
    items.forEach((m) => { if (!Object.prototype.hasOwnProperty.call(this, m.type)) Object.defineProperty(this, m.type, { value: m, writable: true, configurable: true, enumerable: false }); });
  }
  get length() { return __mimeArrStore.get(this).length; }
  item(i) { return this[i] || null; }
  namedItem(name) {
    const a = __mimeArrStore.get(this);
    for (let i = 0; i < a.length; i++) if (a[i].type === name) return a[i];
    return null;
  }
};
globalThis.MimeTypeArray.prototype[Symbol.toStringTag] = 'MimeTypeArray';
const __mimeArrStore = new WeakMap();
// Cached singletons: real browsers return the same instance on every access
// (`navigator.plugins === navigator.plugins`); fresh instances would be a
// fingerprint anomaly. The plugin↔mime graph is built once so the member
// MimeType objects are shared by identity with navigator.mimeTypes (Chrome:
// plugins[0][0] === mimeTypes[0]).
let _pluginsInst = null, _mimeTypesInst = null;
function __ditingBuildPdfPersona() {
  if (_pluginsInst) return;
  const mtApp = new MimeType("application/pdf", "Portable Document Format", "pdf", null);
  const mtText = new MimeType("text/pdf", "Portable Document Format", "pdf", null);
  _mimeTypesInst = new MimeTypeArray(mtApp, mtText);
  const members = [mtApp, mtText];
  _pluginsInst = new PluginArray(
    new Plugin("PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("Chrome PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("Chromium PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("Microsoft Edge PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("WebKit built-in PDF", "internal-pdf-viewer", "Portable Document Format", members),
  );
  __mimeStore.get(mtApp).enabledPlugin = _pluginsInst[0];
  __mimeStore.get(mtText).enabledPlugin = _pluginsInst[0];
}

// navigator.connection: Chrome's NetworkInformation. The interface IS on
// window (`new` → Illegal constructor), the prototype holds exactly
// onchange/effectiveType/rtt/downlink/saveData + constructor — in that
// order, constructor LAST (WebIDL layout; the reorder pass at the bottom
// fixes class-syntax's constructor-first). The EventTarget methods arrive
// via the prototype CHAIN (relinked onto _EventTarget.prototype below), so
// addEventListener et al. are never own names of the first prototype. The
// dead-spec faces — type/downlinkMax/ontypechange — are gone from modern
// Chrome (all read undefined). The singleton is Object.create'd:
// gOPN(instance) = [], and addEventListener/removeEventListener keep working
// through _EventTarget.prototype's own listener store.
class NetworkInformation {
  constructor() { throw new TypeError("Failed to construct 'NetworkInformation': Illegal constructor"); }
  get onchange() { return this._onchange || null; }
  set onchange(v) { this._onchange = typeof v === "function" ? v : null; }
  get effectiveType() { return '4g'; }
  get rtt() { return 50; }
  get downlink() { return 10; }
  get saveData() { return false; }
}
_markNative(NetworkInformation);
globalThis.NetworkInformation = NetworkInformation;

// ---- bchkAPIs global face (batch166, datadome-solver generators.go) ------
// Interfaces real Chrome exposes that anti-bot probes check with
// `'X' in window`. Illegal constructors where Chrome forbids `new`; kept
// standalone (no DOM base classes) so definition order stays independent.

globalThis.FragmentDirective = class FragmentDirective {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};
const __diting_fragmentDirective = Object.create(globalThis.FragmentDirective.prototype);

globalThis.UserActivation = class UserActivation {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get hasBeenActive() { return this._hasBeenActive === true; }
  get isActive() { return this._isActive === true; }
};
const __diting_userActivation = Object.create(globalThis.UserActivation.prototype);

globalThis.Keyboard = class Keyboard {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  getLayoutMap() { return Promise.resolve(new Map()); }
  lock() { return Promise.resolve(); }
  unlock() {}
};
const __diting_keyboard = Object.create(globalThis.Keyboard.prototype);

globalThis.LaunchQueue = class LaunchQueue {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  setConsumer(callback) { this._consumer = callback; }
};
const __diting_launchQueue = Object.create(globalThis.LaunchQueue.prototype);

globalThis.BatteryManager = class BatteryManager {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get charging() { return this._charging; }
  get chargingTime() { return this._chargingTime; }
  get dischargingTime() { return this._dischargingTime; }
  get level() { return this._level; }
};

globalThis.AudioSinkInfo = class AudioSinkInfo {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get type() { return "default"; }
};

globalThis.EyeDropper = class EyeDropper {
  // Chrome's color picker needs real user activation; a headless page gets
  // the NotAllowedError branch of the same promise shape.
  open(options) { return Promise.reject(new DOMException("NotAllowedError")); }
};

globalThis.GPUSupportedLimits = class GPUSupportedLimits {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};

globalThis.GeolocationCoordinates = class GeolocationCoordinates {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get latitude() { return this._latitude ?? 0; }
  get longitude() { return this._longitude ?? 0; }
  get accuracy() { return this._accuracy ?? 0; }
  get altitude() { return this._altitude ?? null; }
  get altitudeAccuracy() { return this._altitudeAccuracy ?? null; }
  get heading() { return this._heading ?? null; }
  get speed() { return this._speed ?? null; }
};
globalThis.GeolocationPosition = class GeolocationPosition {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get coords() { return this._coords; }
  get timestamp() { return this._timestamp ?? 0; }
};
globalThis.__diting_makeGeolocationPosition = function (lat, lon, acc) {
  const coords = Object.create(globalThis.GeolocationCoordinates.prototype);
  coords._latitude = lat;
  coords._longitude = lon;
  coords._accuracy = acc;
  const pos = Object.create(globalThis.GeolocationPosition.prototype);
  pos._coords = coords;
  pos._timestamp = Date.now();
  return pos;
};

globalThis.MediaDeviceInfo = class MediaDeviceInfo {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get deviceId() { return this._deviceId || ""; }
  get kind() { return this._kind || ""; }
  get label() { return this._label || ""; }
  get groupId() { return this._groupId || ""; }
  toJSON() { return { deviceId: this.deviceId, groupId: this.groupId, kind: this.kind, label: this.label }; }
};

globalThis.MediaMetadata = class MediaMetadata {
  constructor(init = {}) {
    if (init == null) init = {};
    this._title = init.title === undefined ? "" : String(init.title);
    this._artist = init.artist === undefined ? "" : String(init.artist);
    this._album = init.album === undefined ? "" : String(init.album);
    this._artwork = Array.isArray(init.artwork) ? init.artwork : [];
  }
  get title() { return this._title; }
  set title(v) { this._title = String(v); }
  get artist() { return this._artist; }
  set artist(v) { this._artist = String(v); }
  get album() { return this._album; }
  set album(v) { this._album = String(v); }
  get artwork() { return this._artwork; }
  set artwork(v) { this._artwork = Array.isArray(v) ? v : []; }
};

globalThis.MediaSourceHandle = class MediaSourceHandle {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};

globalThis.PasswordCredential = class PasswordCredential {
  constructor(data) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'PasswordCredential': 1 argument required, but only 0 present.");
    if (data == null || typeof data !== "object") throw new TypeError("Failed to construct 'PasswordCredential': The provided value is not of type 'PasswordCredentialData'.");
    if (data.id === undefined) throw new TypeError("Failed to construct 'PasswordCredential': 'id' cannot be undefined.");
    if (data.password === undefined) throw new TypeError("Failed to construct 'PasswordCredential': 'password' cannot be undefined.");
    this._id = String(data.id);
    this._name = data.name === undefined ? "" : String(data.name);
    this._password = String(data.password);
    // 刀57: id/type answer through Credential.prototype now (Chrome owns
    // them there; PasswordCredential.prototype lists neither).
    this._type = "password";
  }
  get name() { return this._name; }
  get password() { return this._password; }
  get iconURL() { return ""; }
};

globalThis.PushSubscriptionOptions = class PushSubscriptionOptions {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get userVisibleOnly() { return this._userVisibleOnly === true; }
  get applicationServerKey() { return this._applicationServerKey ?? null; }
};

globalThis.Scheduling = class Scheduling {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  isInputPending() { return false; }
};

globalThis.TextTrackCue = class TextTrackCue {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get track() { return this._track ?? null; }
  get id() { return this._id || ""; }
  set id(v) { this._id = String(v); }
  get startTime() { return this._startTime ?? 0; }
  set startTime(v) { this._startTime = Number(v); }
  get endTime() { return this._endTime ?? 0; }
  set endTime(v) { this._endTime = Number(v); }
  get pauseOnExit() { return this._pauseOnExit === true; }
  set pauseOnExit(v) { this._pauseOnExit = Boolean(v); }
};

globalThis.VideoPlaybackQuality = class VideoPlaybackQuality {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get creationTime() { return this._creationTime ?? 0; }
  get totalVideoFrames() { return this._totalVideoFrames ?? 0; }
  get droppedVideoFrames() { return this._droppedVideoFrames ?? 0; }
  get corruptedVideoFrames() { return this._corruptedVideoFrames ?? 0; }
};

globalThis.ViewTransition = class ViewTransition {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get updateCallbackDone() { if (!this._ucd) this._ucd = Promise.resolve(); return this._ucd; }
  get ready() { if (!this._readyP) this._readyP = Promise.resolve(); return this._readyP; }
  get finished() { if (!this._fin) this._fin = Promise.resolve(); return this._fin; }
  skipTransition() {}
};

globalThis.WebGLObject = class WebGLObject {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};

globalThis.WebTransport = class WebTransport {
  constructor(url, options) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'WebTransport': 1 argument required, but only 0 present.");
    try { this._url = new URL(url, _docBase()).href; }
    catch (e) { throw new TypeError("Failed to construct 'WebTransport': The provided value is not a valid URL."); }
  }
  get ready() { if (!this._ready) this._ready = new Promise(() => {}); return this._ready; }
  get closed() { if (!this._closedP) this._closedP = new Promise(() => {}); return this._closedP; }
  get datagrams() {
    if (!this._datagrams) {
      this._datagrams = {
        readable: new ReadableStream(),
        writable: new WritableStream(),
      };
    }
    return this._datagrams;
  }
  close() {}
  createBidirectionalStream() { return Promise.reject(new DOMException("NotAllowedError")); }
  createUnidirectionalStream() { return Promise.reject(new DOMException("NotAllowedError")); }
};

globalThis.WebSocketStream = class WebSocketStream {
  constructor(url, protocols) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'WebSocketStream': 1 argument required, but only 0 present.");
    const wss = __diting_liftSlot(this);
    wss.url = String(url);
    wss.opened = new Promise(() => {});   // Chrome 145: `opened` (the old `opening` name was the pre-ship draft)
    wss.closed = new Promise(() => {});
  }
  close() {}
};

globalThis.CSSCounterStyleRule = class CSSCounterStyleRule {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get name() { return this._name || ""; }
};

globalThis.CSSFontPaletteValuesRule = class CSSFontPaletteValuesRule {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get name() { return this._name || ""; }
};

globalThis.CropTarget = class CropTarget {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  static fromElement(element) { return Object.create(globalThis.CropTarget.prototype); }
};

globalThis.CustomStateSet = class CustomStateSet {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get size() { return 0; }
  add() { return this; }
  has() { return false; }
  delete() { return false; }
  clear() {}
  forEach() {}
  *entries() {}
  *keys() {}
  *values() {}
  [Symbol.iterator]() { return [][Symbol.iterator](); }
};

if (typeof globalThis.AudioNode === "undefined") {
  globalThis.AudioNode = class AudioNode {
    constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
    connect(dest) { return dest; }
    disconnect() {}
  };
}
globalThis.IIRFilterNode = class IIRFilterNode {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  getFrequencyResponse() {}
};

// (#117) Chrome's navigator carries ZERO own enumerable props — all 83 keys
// are getters/methods on Navigator.prototype (probe ground truth from a real
// Chrome 145: Object.keys(navigator) === []). The store below is laid out in
// Chrome's exact own-property order, because detectors diff prototype key
// ORDER, not just membership. Internal writes (persona refresh, viewport
// emulation) go through __diting_navSet on the store; page writes hit
// getter-only prototype accessors and no-op, like Chrome's readonly
// attributes. Values that were previously instance getters stay store
// getters — the hoist calls them against the store, so UA-dependent answers
// keep refreshing without re-hoisting.
// fp-diff-1014 #11: Chrome's navigator.userAgentData has NavigatorUAData on
// its prototype chain — a plain object literal reads as constructor "Object"
// to the cheapest proto walk.
// 300015 P6: the interface is exposed on window (`new` → Illegal
// constructor), members sit on the prototype in Chrome's order (brands,
// mobile, platform, getHighEntropyValues, toJSON, constructor LAST —
// reorder pass at the bottom), and the instance carries ZERO own names
// (gOPN = []). mobile used to be a constructor-stashed own prop: hidden
// from Object.keys but visible to getOwnPropertyNames, and missing from
// the prototype. Desktop persona → always false, like Chrome on macOS.
class NavigatorUAData {
  constructor() { throw new TypeError("Failed to construct 'NavigatorUAData': Illegal constructor"); }
  get brands() {
    return __ditingBrandList(Number(__ditingChromeMajorFromUA()));
  }
  get mobile() { return false; }
  get platform() { return __ditingUADataPlatformFromUA(); }
  getHighEntropyValues(hints) {
    const plat = __ditingUADataPlatformFromUA();
    // Device faces come from the same persona draw as the WebGL renderer —
    // Apple M3 + architecture "x86" was a three-machines-in-one tell.
    const dev = _getFp();
    const maj = __ditingChromeMajorFromUA();
    // Majors outside the table are unsupported personas: reuse the nearest known major
    // instead of extrapolating a build that never shipped.
    const build = __DITING_FULL_BUILD[maj] || __ditingNearestBuild(Number(maj));
    const full = __ditingBrandList(Number(maj)).map(function (b) {
      return { brand: b.brand, version: b.brand === "Chromium" || b.brand === "Google Chrome" ? maj + ".0." + build : b.version + ".0.0.0" };
    });
    return Promise.resolve({
      architecture: dev.arch,
      bitness: dev.bitness,
      brands: __ditingBrandList(Number(maj)),
      fullVersionList: full,
      mobile: false,
      model: "",
      platform: plat,
      platformVersion: dev.platformVersion,
      uaFullVersion: maj + ".0." + build,
      wow64: false,
    });
  }
  toJSON() { return {brands:this.brands,mobile:this.mobile,platform:this.platform}; }
}
globalThis.NavigatorUAData = NavigatorUAData;
// languages is a frozen array and Chrome hands back the same object while
// the locale is unchanged (moli ca22988 same face). Module-scope cache —
// never on __navStore, whose own names feed the prototype setup walk.
let __navLangsKey = null, __navLangsVal = null;
const __navStore = {
  vendorSub: "",
  productSub: "20030107",
  vendor: "Google Inc.",
  maxTouchPoints: 0,
  scheduling: { isInputPending() { return false; } },
  get userActivation() { return __diting_userActivation; },
  geolocation: {
    getCurrentPosition(success, error) {
      const pos = globalThis.__diting_makeGeolocationPosition(
        50.1109 + (_fpRand(500) - 0.5) * 0.1,
        8.6821 + (_fpRand(501) - 0.5) * 0.1,
        10 + _fpRand(502) * 40,
      );
      if (typeof success === 'function') success(pos);
    },
    watchPosition(success, error) {
      if (typeof success === 'function') {
        success(globalThis.__diting_makeGeolocationPosition(
          50.1109 + (_fpRand(503) - 0.5) * 0.1,
          8.6821 + (_fpRand(504) - 0.5) * 0.1,
          10 + _fpRand(505) * 40,
        ));
      }
      return 0;
    },
    clearWatch() {},
  },
  doNotTrack: null,
  webkitTemporaryStorage: { queryUsageAndQuota(cb) { if (cb) cb(0, 5000000000); }, requestQuota() {} },
  webkitPersistentStorage: { queryUsageAndQuota(cb) { if (cb) cb(0, 500000000); }, requestQuota() {} },
  windowControlsOverlay: { visible: false, ontitlebarchange: null, addEventListener(){}, removeEventListener(){}, dispatchEvent(){ return false; } },
  hardwareConcurrency: 8,
  cookieEnabled: true,
  appCodeName: "Mozilla",
  appName: "Netscape",
  get appVersion() { return this.userAgent.replace('Mozilla/', ''); },
  get platform() { return globalThis.__diting_platform_override || __ditingPlatformFromUA(); },
  product: "Gecko",
  get userAgent() { return globalThis.__diting_ua || "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"; },
  get language() { return __ditingLangList()[0]; },
  get languages() {
    const list = __ditingLangList();
    const key = list.join(',');
    if (__navLangsKey !== key) {
      __navLangsKey = key;
      __navLangsVal = Object.freeze(list.slice());
    }
    return __navLangsVal;
  },
  onLine: true,
  // (#78) Chrome's NavigatorAutomationInformation always defines webdriver
  // and reads false on a normal session — `undefined` is the odd one out and
  // a jsvmp scan tell.
  get webdriver() { return false; },
  get plugins() {
    __ditingBuildPdfPersona();
    return _pluginsInst;
  },
  get mimeTypes() {
    __ditingBuildPdfPersona();
    return _mimeTypesInst;
  },
  pdfViewerEnabled: true,
  connection: Object.create(NetworkInformation.prototype),
  getGamepads() { return []; },
  javaEnabled() { return false; },
  sendBeacon(url, data) {
    let abs;
    try {
      abs = new URL(String(url), _docBase()).href;
    } catch (e) {
      return false;
    }
    let body = data == null ? "" : data;
    let hdrs;
    if (typeof data === "string") {
      hdrs = { "content-type": "text/plain;charset=UTF-8" };
    } else if (typeof Blob === "function" && data instanceof Blob) {
      // fetch already rides blob.type as the content-type.
    } else if (typeof URLSearchParams === "function" && data instanceof URLSearchParams) {
      // fetch already applies the urlencoded content-type.
    } else if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
      hdrs = { "content-type": "application/octet-stream" };
    } else if (typeof FormData === "function" && data instanceof FormData) {
      // fetch serializes multipart with a generated boundary.
    } else {
      body = String(data);
      hdrs = { "content-type": "text/plain;charset=UTF-8" };
    }
    // Fire-and-forget: a beacon outlives the page that queued it, so the
    // response is never observed — the promise is discarded and its rejection
    // swallowed (a failing endpoint must not surface as an unhandled
    // rejection). true means the transfer was queued, not delivered.
    const init = { method: "POST", body, mode: "no-cors", credentials: "include", keepalive: true };
    if (hdrs) init.headers = hdrs;
    Promise.resolve(globalThis.fetch(abs, init)).catch(() => {});
    return true;
  },
  vibrate() { return false; },
  deprecatedRunAdAuctionEnforcesKAnonymity: false,
  protectedAudience: {},
  bluetooth: {
    getAvailability() { return Promise.resolve(false); },
    requestDevice() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  },
  clipboard: { writeText(){return Promise.resolve();}, readText(){return Promise.resolve("");} },
  credentials: { get(){return Promise.resolve(null);}, create(){return Promise.resolve(null);}, store(){return Promise.resolve();}, preventSilentAccess(){return Promise.resolve();} },
  get keyboard() { return __diting_keyboard; },
  managed: { getManagedConfiguration() { return Promise.resolve({}); } },
  mediaDevices: {
    enumerateDevices() {
      return Promise.resolve([
        {deviceId:"default",kind:"audioinput",label:"",groupId:"default"},
        {deviceId:"comms",kind:"audioinput",label:"",groupId:"comms"},
        {deviceId:"default",kind:"audiooutput",label:"",groupId:"default"},
        {deviceId:"",kind:"videoinput",label:"",groupId:""},
      ]);
    },
    getUserMedia() { return Promise.reject(new DOMException("NotAllowedError")); },
    getDisplayMedia() { return Promise.reject(new DOMException("NotAllowedError")); },
  },
  serviceWorker: (function () {
    // Chrome's register() resolves a ServiceWorkerRegistration; pages chain
    // reg.scope / reg.unregister() straight off the promise (#83) — the
    // previous stub resolved undefined and the .scope read threw. ready
    // carries the same shape. No worker ever runs; controller stays null,
    // getRegistration keeps resolving undefined so "is one registered?"
    // checks still take the no-SW branch.
    // (300015 P7) register must FOLLOW the script fetch: Chrome only
    // resolves when the worker script answers ok with a JavaScript MIME
    // type. ark registers a script that 404s on the CDN, so real Chrome
    // REJECTS and the app swallows it in .catch() — an unconditional
    // resolve is a one-line bot check (.then() on a dead script = fake
    // browser). Chrome truth (CDP 155, 10-15):
    //   cross-origin -> SecurityError "Failed to register a ServiceWorker:
    //     The origin of the provided scriptURL ('{origin}') does not match
    //     the current origin ('{origin}')." (checked before any fetch)
    //   !ok status  -> TypeError "Failed to register a ServiceWorker for
    //     scope ('{scope}') with script ('{url}'): A bad HTTP response code
    //     ({status}) was received when fetching the script."
    //   non-JS MIME -> SecurityError, same prefix, "The script has an
    //     unsupported MIME type ('{mime}')."
    // Net-failure tail uncaptured (a same-origin dead host can't be staged);
    // Chromium-string best effort. The ok path keeps the stub registration —
    // no worker runs either way.
    function registration(scope) {
      return {
        scope: scope,
        updateViaCache: 'imports',
        active: null, installing: null, waiting: null,
        update: function () { return Promise.resolve(undefined); },
        unregister: function () { return Promise.resolve(true); },
        addEventListener: function () {}, removeEventListener: function () {},
        dispatchEvent: function () { return false; },
      };
    }
    function dirScope(url) {
      return url.slice(0, url.lastIndexOf('/') + 1);
    }
    function swFail(ctor, tail, name) {
      return name ? new ctor(tail, name) : new ctor(tail);
    }
    var JS_MIME = /^(application|text)\/(x-)?(javascript|ecmascript)$/;
    return {
      get ready() { return Promise.resolve(registration(dirScope(location.href))); },
      register: function (scriptUrl, options) {
        var su, scope, suHref;
        try {
          su = new URL(scriptUrl, location.href);
        } catch (e) {
          return Promise.reject(new TypeError("Failed to register a ServiceWorker: The URL is not valid."));
        }
        suHref = su.href;
        try {
          scope = (options && options.scope)
            ? new URL(options.scope, location.href).href
            : dirScope(suHref);
        } catch (e) { scope = dirScope(location.href); }
        if (su.origin !== location.origin) {
          return Promise.reject(swFail(DOMException,
            "Failed to register a ServiceWorker: The origin of the provided scriptURL ('" +
            su.origin + "') does not match the current origin ('" + location.origin + "').", "SecurityError"));
        }
        return fetch(suHref).then(function (r) {
          if (!r.ok) {
            throw swFail(TypeError,
              "Failed to register a ServiceWorker for scope ('" + scope + "') with script ('" +
              suHref + "'): A bad HTTP response code (" + r.status + ") was received when fetching the script.");
          }
          var ct = (r.headers.get('content-type') || '').split(';')[0].trim().toLowerCase();
          if (!JS_MIME.test(ct)) {
            throw swFail(DOMException,
              "Failed to register a ServiceWorker for scope ('" + scope + "') with script ('" +
              suHref + "'): The script has an unsupported MIME type ('" + ct + "').", "SecurityError");
          }
          return registration(scope);
        }, function () {
          throw swFail(TypeError,
            "Failed to register a ServiceWorker for scope ('" + scope + "') with script ('" +
            suHref + "'): Registration failed - the worker script fetch failed.");
        });
      },
      getRegistrations(){ return Promise.resolve([]); },
      getRegistration(){ return Promise.resolve(undefined); },
      controller: null,
    };
  })(),
  virtualKeyboard: {
    boundingRect: { x: 0, y: 0, width: 0, height: 0 },
    overlaysContent: false,
    show(){}, hide(){}, setBoundingRect(){},
    addEventListener(){}, removeEventListener(){},
  },
  wakeLock: { request() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  deviceMemory: 8,
  userAgentData: Object.create(NavigatorUAData.prototype),
  locks: { request() { return Promise.resolve(); }, query() { return Promise.resolve([]); } },
  storage: {
    estimate() { return Promise.resolve({ quota: 5000000000, usage: Math.floor(_fpRand(640) * 100000000) }); },
    persist() { return Promise.resolve(false); },
    persisted() { return Promise.resolve(false); },
  },
  gpu: {
    requestAdapter() { return Promise.resolve(null); },
    getPreferredCanvasFormat() { return 'bgra8unorm'; },
    wgslLanguageFeatures: { has() { return false; }, size: 0 },
  },
  login: { status: "logged-out", setStatus() { return Promise.resolve(); } },
  ink: { requestPresenter() { return Promise.resolve(null); }, debouncePresenter() { return Promise.resolve(null); } },
  mediaCapabilities: {
    decodingInfo() { return Promise.resolve({ supported: true, smooth: true, powerEfficient: true }); },
    encodingInfo() { return Promise.resolve({ supported: true, smooth: true, powerEfficient: true }); },
  },
  permissions: { query(params){
    if (params?.name === 'notifications') return Promise.resolve({state:"prompt",onchange:null});
    return Promise.resolve({state:"granted"});
  } },
  devicePosture: { type: "continuous" },
  hid: { getDevices() { return Promise.resolve([]); }, requestDevice() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  mediaSession: {
    metadata: null, playbackState: "none",
    setActionHandler(){}, setPositionState(){}, setCameraActive(){}, setMicrophoneActive(){},
  },
  presentation: { defaultRequest: null, receiver: null },
  serial: { getPorts() { return Promise.resolve([]); }, requestPort() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  usb: { getDevices() { return Promise.resolve([]); }, requestDevice() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  xr: { isSessionSupported() { return Promise.resolve(false); }, ondevicechange: null, addEventListener(){}, removeEventListener(){} },
  storageBuckets: { keys() { return Promise.resolve([]); }, open() { return Promise.resolve(null); }, delete() { return Promise.resolve(false); }, persist() { return Promise.resolve(false); } },
  adAuctionComponents() { return Promise.resolve([]); },
  runAdAuction() { return Promise.resolve(null); },
  canLoadAdAuctionFencedFrame() { return false; },
  canShare() { return false; },
  share() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  clearAppBadge() { return Promise.resolve(); },
  getBattery() {
    const bm = Object.create(globalThis.BatteryManager.prototype);
    bm._charging = _fp('batteryCharging');
    bm._chargingTime = _fp('batteryCharging') ? 0 : Infinity;
    bm._dischargingTime = _fp('batteryCharging') ? Infinity : Math.floor(3600 + _fpRand(250) * 7200);
    bm._level = _fp('batteryLevel');
    return Promise.resolve(bm);
  },
  getUserMedia() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  requestMIDIAccess() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  requestMediaKeySystemAccess() { return Promise.reject(new DOMException("Unsupported key system", "NotSupportedError")); },
  setAppBadge() { return Promise.resolve(); },
  webkitGetUserMedia(cb, err) { if (err) err(new DOMException("Permission denied.", "NotAllowedError")); },
  clearOriginJoinedAdInterestGroups() {},
  createAuctionNonce() { return (globalThis.crypto && globalThis.crypto.randomUUID) ? globalThis.crypto.randomUUID() : ""; },
  joinAdInterestGroup() { return Promise.resolve(); },
  leaveAdInterestGroup() { return Promise.resolve(); },
  updateAdInterestGroups() {},
  deprecatedReplaceInURN() { return Promise.resolve(null); },
  deprecatedURNToURL() { return Promise.resolve(null); },
  getInstalledRelatedApps() { return Promise.resolve([]); },
  getInterestGroupAdAuctionData() { return Promise.resolve(null); },
  registerProtocolHandler() {},
  unregisterProtocolHandler() {},
};
// (#104) Chrome exposes the Navigator interface object on window — the
// Douyin security SDK subclasses it (`class t extends Navigator`) and died
// with ReferenceError. Illegal constructor like every Chrome interface
// without [Constructor]. (#117) all attributes live on the prototype now, so
// instanceof/getPrototypeOf/key-parity all match Chrome. Prototype accessors
// survive the V8 snapshot (#37).
globalThis.Navigator = class Navigator {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};
for (const _nk of Object.getOwnPropertyNames(__navStore)) {
  const _d = Object.getOwnPropertyDescriptor(__navStore, _nk);
  const _k = _nk;
  if (_d.get || typeof _d.value !== 'function') {
    // Attributes stay getter-only prototype accessors — page writes no-op,
    // exactly like Chrome's readonly navigator attributes (#117).
    Object.defineProperty(globalThis.Navigator.prototype, _k, {
      configurable: true, enumerable: true,
      get: _d.get ? function () { return _d.get.call(__navStore); } : function () { return __navStore[_k]; },
    });
  } else {
    // WebIDL operations are writable data properties on the prototype.
    // #203: instrumentation SDKs (抖店 secsdk's createAspectByPath) reassign
    // navigator.sendBeacon to wrap it; a getter-only accessor turned that
    // strict-mode assignment into a TypeError. Assignment shadows via an
    // own property on `navigator` — the store and __diting_navSet stay
    // untouched, like Chrome.
    Object.defineProperty(globalThis.Navigator.prototype, _k, {
      configurable: true, enumerable: true, writable: true,
      value: _d.value,
    });
  }
}
__def(globalThis.Navigator.prototype, Symbol.toStringTag, 'Navigator');
__def(globalThis, Symbol.toStringTag, 'Window');
globalThis.navigator = Object.create(globalThis.Navigator.prototype);
// The single sanctioned write path for navigator state (persona refresh,
// viewport emulation). Pages writing navigator.* directly hit the
// getter-only prototype accessors and silently no-op — Chrome parity.
globalThis.__diting_navSet = function(k, v) {
  Object.defineProperty(__navStore, k, { value: v, writable: true, configurable: true, enumerable: true });
};

// Key order matches real Chrome's Object.keys(chrome) = ["loadTimes","csi","app"]
// (DataDome-class fingerprints read key order, not just membership). No
// `runtime` key: fp-diff-1014 #2 caught us exposing chrome.runtime as an
// object — on a normal Chrome page it is absent entirely (it only exists in
// extension content-script contexts), so its presence reads as automation.
// Real Chrome's csi/loadTimes are anonymous, constructible natives
// (name "", own `prototype`, toString "function () { [native code] }");
// the `(0, function () {})` shape keeps name inference from naming them.
// xhs's b1 field x45 (__SEC_CAV__) checks exactly this and read 0 for both.
globalThis.chrome = {
  loadTimes: (0, function () {
    const t = Date.now() / 1000;
    const request = t - 0.5 - _fpRand(611) * 0.5;
    const startLoad = request + 0.05 + _fpRand(612) * 0.02;
    const commit = request + 0.3 + _fpRand(613) * 0.4;
    const finishDoc = commit + 0.1 + _fpRand(614) * 0.2;
    const finish = finishDoc + 0.05 + _fpRand(615) * 0.1;
    const firstPaint = commit + 0.03 + _fpRand(616) * 0.1;
    const navTypes = ["BackForward","Reload","Link","Other"];
    return {
      requestTime: request, startLoadTime: startLoad * 1000, commitLoadTime: commit * 1000,
      finishDocumentLoadTime: finishDoc * 1000, finishLoadTime: finish * 1000,
      firstPaintTime: firstPaint * 1000, firstPaintAfterLoadTime: 0,
      navigationType: navTypes[Math.floor(_fpRand(617) * 4)],
      wasFetchedViaSpdy: false, wasNpnNegotiated: false,
      npnNegotiatedProtocol: "http/1.1",
      wasAlternateProtocolAvailable: false, connectionInfo: "http/1.1",
    };
  }),
  csi: (0, function () {
    const t = Date.now();
    return { startE: t - Math.floor(100 + _fpRand(610) * 200), onloadT: t, pageT: 0, tran: 5 };
  }),
  // Key set AND order captured from a real Chrome 155 (fp probe 10-14):
  // isInstalled|getDetails|getIsInstalled|installState|runningState|InstallState|RunningState
  app: {
    isInstalled: false,
    getDetails() { return null; },
    getIsInstalled() { return false; },
    installState() { return "disabled"; },
    runningState() { return "cannot_run"; },
    InstallState: { DISABLED: "disabled", INSTALLED: "installed", NOT_INSTALLED: "not_installed" },
    RunningState: { CANNOT_RUN: "cannot_run", READY_TO_RUN: "ready_to_run", RUNNING: "running" },
  },
};

globalThis.Notification = class Notification {
  static permission = "default";
  static maxActions = 2;
  static requestPermission() { return Promise.resolve(Notification.permission); }
  constructor() {}
  get image() { return null; }
};

// (刀69) Chrome: `new WebGLRenderingContext()` is an illegal constructor — the
// contexts come out of canvas.getContext (Object.create over the prototype).
globalThis.WebGLRenderingContext = class WebGLRenderingContext { constructor() { throw new TypeError("Failed to construct 'WebGLRenderingContext': Illegal constructor"); } };
globalThis.WebGL2RenderingContext = class WebGL2RenderingContext { constructor() { throw new TypeError("Failed to construct 'WebGL2RenderingContext': Illegal constructor"); } };

// (#117) Chrome's screen carries ZERO own enumerable props — every field is
// a getter on Screen.prototype (probe ground truth: Object.keys(screen) ===
// []). The persona refresh writes the hidden store via
// __diting_screenApply, so `screen` (and its orientation object) keeps one
// identity for the context's whole life, like Chrome's.
globalThis.Screen = class Screen { constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); } };
const __scrStore = {
  width: 1920, height: 1080, availWidth: 1920, availHeight: 1040,
  colorDepth: 24, pixelDepth: 24, availTop: 0, availLeft: 0, isExtended: false,
  orientation: { type: "landscape-primary", angle: 0, addEventListener(){}, removeEventListener(){}, dispatchEvent(){ return true; } },
};
for (const _sk of Object.keys(__scrStore)) {
  const _k = _sk;
  Object.defineProperty(globalThis.Screen.prototype, _k, {
    configurable: true, enumerable: true,
    get() { return __scrStore[_k]; },
  });
}
__def(globalThis.Screen.prototype, Symbol.toStringTag, 'Screen');
globalThis.screen = Object.create(globalThis.Screen.prototype);
globalThis.__diting_screenApply = function(sw, sh) {
  __scrStore.width = sw; __scrStore.height = sh;
  __scrStore.availWidth = sw; __scrStore.availHeight = sh - 40;
  __scrStore.availTop = 0; __scrStore.availLeft = 0;
};
// visualViewport is a real interface in Chrome — a detector walking
// Object.getPrototypeOf(visualViewport).constructor.name sees "VisualViewport",
// and width/height are prototype getters, not own enumerable data props.
globalThis.VisualViewport = class VisualViewport {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get offsetLeft() { return this._offsetLeft || 0; }
  get offsetTop() { return this._offsetTop || 0; }
  get pageLeft() { return globalThis.scrollX || 0; }
  get pageTop() { return globalThis.scrollY || 0; }
  get scale() { return this._scale || 1; }
  get width() { return this._width || 0; }
  get height() { return this._height || 0; }
  // no own trio (M4 knife 3): the prototype chains to EventTarget.prototype
  // (global_surface ET_CHAIN) and the native core carries the listeners —
  // the old empty stubs swallowed them.
};
globalThis.__diting_makeVisualViewport = function(w, h) {
  const vv = Object.create(VisualViewport.prototype);
  for (const [k, v] of Object.entries({ _width: w, _height: h, _offsetLeft: 0, _offsetTop: 0, _scale: 1 })) {
    Object.defineProperty(vv, k, { value: v, enumerable: false, configurable: true, writable: true });
  }
  return vv;
};
globalThis.visualViewport = globalThis.__diting_makeVisualViewport(1920, 1000);
globalThis.devicePixelRatio = 1;
// (#105) innerWidth/innerHeight/outer* are prototype accessors in Chrome —
// a page write is silently ignored. As plain writable data props a resize
// polyfill on douyin overwrote them with document extents, skewing every
// rect and click coordinate by a viewport. The snapshot cannot bake the
// accessors (#37 flattening), so defaults live in backing vars here and
// __diting_init installs the getters per context.
var __vpW = 1920, __vpH = 1000, __vpOW = 1920, __vpOH = 1080;
globalThis.__diting_installViewportAccessors = function() {
  const defs = {
    innerWidth: () => __vpW, innerHeight: () => __vpH,
    outerWidth: () => __vpOW, outerHeight: () => __vpOH,
  };
  for (const k in defs) {
    try {
      Object.defineProperty(globalThis, k, { get: defs[k], configurable: true, enumerable: true });
    } catch (e) {}
  }
};
globalThis.scrollX = 0; globalThis.scrollY = 0;
globalThis.pageXOffset = 0; globalThis.pageYOffset = 0;

globalThis.__fetchInterceptEnabled = false;
globalThis.__fetchInterceptCallback = null; // Set by CDP to handle paused requests

function _base64ToUint8Array(b64) {
  const clean = String(b64 || '').replace(/[\r\n\s]/g, '');
  if (!clean) return new Uint8Array();
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const padding = clean.endsWith('==') ? 2 : (clean.endsWith('=') ? 1 : 0);
  const bytes = new Uint8Array((clean.length * 3 >> 2) - padding);
  let out = 0;
  for (let i = 0; i < clean.length; i += 4) {
    const a = alphabet.indexOf(clean[i]);
    const b = alphabet.indexOf(clean[i + 1]);
    const c = clean[i + 2] === '=' ? 0 : alphabet.indexOf(clean[i + 2]);
    const d = clean[i + 3] === '=' ? 0 : alphabet.indexOf(clean[i + 3]);
    const n = (a << 18) | (b << 12) | (c << 6) | d;
    if (out < bytes.length) bytes[out++] = (n >> 16) & 0xff;
    if (out < bytes.length) bytes[out++] = (n >> 8) & 0xff;
    if (out < bytes.length) bytes[out++] = n & 0xff;
  }
  return bytes;
}

function _bodyToUint8Array(body) {
  if (body == null) return new Uint8Array();
  if (body instanceof Uint8Array) return body;
  if (body instanceof ArrayBuffer) return new Uint8Array(body);
  if (ArrayBuffer.isView(body)) return new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
  // obscura's Blob materializes its data into _bytes in the constructor.
  if (body._bytes instanceof Uint8Array) return body._bytes;
  return new TextEncoder().encode(String(body));
}

// Latin-1 binary string: one char per byte (kept for callers that need the
// legacy form).
function _bytesToBinaryString(bytes) { let s = ""; for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]); return s; }

// Base64 of raw bytes — the wire form for request bodies. The old Latin-1
// binary string corrupted non-UTF-8 byte sequences on the deno #[string]
// boundary ([0,128,255] became [0,194,128,195,191], upstream obscura #716);
// base64 is byte-exact across that boundary.
function _bytesToBase64(bytes) {
  const c="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let r="";
  for(let i=0;i<bytes.length;i+=3){
    const a=bytes[i],b=bytes[i+1],d=bytes[i+2];
    r+=c[a>>2]+c[((a&3)<<4)|((b??0)>>4)]+(i+1<bytes.length?c[((b&15)<<2)|((d??0)>>6)]:"=")+(i+2<bytes.length?c[d&63]:"=");
  }
  return r;
}

function _concatBytes(arrays) {
  let total = 0;
  for (let i = 0; i < arrays.length; i++) total += arrays[i].length;
  const out = new Uint8Array(total);
  let off = 0;
  for (let i = 0; i < arrays.length; i++) { out.set(arrays[i], off); off += arrays[i].length; }
  return out;
}

function _arrayBufferFromBytes(bytes) {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

function _installWasmStreamingFallback() {
  if (typeof WebAssembly === 'undefined') return;
  if (WebAssembly.instantiateStreaming && WebAssembly.instantiateStreaming.__ditingFallback) return;
  const nativeInstantiateStreaming = WebAssembly.instantiateStreaming;
  const fallback = async function instantiateStreaming(source, imports) {
    const response = await source;
    if (response && typeof response.arrayBuffer === 'function') {
      return WebAssembly.instantiate(await response.arrayBuffer(), imports);
    }
    if (typeof nativeInstantiateStreaming === 'function') {
      return nativeInstantiateStreaming.call(WebAssembly, response, imports);
    }
    return WebAssembly.instantiate(response, imports);
  };
  fallback.__ditingFallback = true;
  WebAssembly.instantiateStreaming = fallback;
}
_installWasmStreamingFallback();

// WHATWG data: URL processor, byte-level. fetch()/XHR take this branch
// because the HTTP client behind op_fetch_url cannot fetch the data: scheme
// (scripts and images already had their own decoders). Returns
// { bytes, mime }; throws TypeError on malformed input like Chrome does.
function _dataUrlBytes(url) {
  const comma = url.indexOf(',');
  if (comma < 0) throw new TypeError('Invalid data: URL');
  let parts = url.slice(5, comma).toLowerCase().split(';');
  const isBase64 = parts[parts.length - 1] === 'base64';
  if (isBase64) parts = parts.slice(0, -1);
  let mime = parts.join(';');
  if (!mime) mime = 'text/plain;charset=us-ascii';
  const payload = url.slice(comma + 1);
  let bytes;
  if (isBase64) {
    const encoded = payload.replace(/[\r\n\t\f ]/g, '');
    const rem = encoded.length % 4;
    if (rem === 1 || !/^[A-Za-z0-9+/]*={0,2}$/.test(encoded)) {
      throw new TypeError('Invalid data: URL base64 payload');
    }
    bytes = _base64ToUint8Array(rem ? encoded + '='.repeat(4 - rem) : encoded);
  } else {
    bytes = _percentDecodeToBytes(payload);
  }
  return { bytes, mime };
}

// Percent-decode a data: payload to raw bytes: %XX escapes become single
// bytes; every other character contributes its UTF-8 encoding (browsers run
// the URL through UTF-8 first, so unescaped non-ASCII lands here too).
function _percentDecodeToBytes(s) {
  const bytes = [];
  let buf = '';
  const flush = () => {
    if (buf) {
      const enc = new TextEncoder().encode(buf);
      for (let i = 0; i < enc.length; i++) bytes.push(enc[i]);
      buf = '';
    }
  };
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '%' && /^[0-9a-fA-F]{2}$/.test(s.slice(i + 1, i + 3))) {
      flush();
      bytes.push(parseInt(s.slice(i + 1, i + 3), 16));
      i += 2;
    } else {
      buf += s[i];
    }
  }
  flush();
  return new Uint8Array(bytes);
}

// Race a fetch op promise against an AbortSignal. Rejects with the signal's
// reason the moment abort fires; the op's own settlement is swallowed so an
// aborted-then-failing walk never surfaces as an unhandled rejection.
function _raceAbortSignal(signal, opCall) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const onAbort = () => {
      if (settled) return;
      settled = true;
      signal.removeEventListener("abort", onAbort);
      reject(signal.reason);
      opCall.then(() => {}, () => {});
    };
    if (signal.aborted) { onAbort(); return; }
    signal.addEventListener("abort", onAbort);
    opCall.then(
      (v) => { if (!settled) { settled = true; signal.removeEventListener("abort", onAbort); resolve(v); } },
      (e) => { if (!settled) { settled = true; signal.removeEventListener("abort", onAbort); reject(e); } }
    );
  });
}

// #126: record one real fetch/XHR sample into the resource buffer. `t0`
// brackets the op call in the resource clock; status and body sizes come
// from the op's parsed reply. Failure shapes (blocked/CORS/abort) record
// duration with status 0 and no sizes — Chrome keeps those entries too.
// `opts` carries per-site extras (e.g. renderBlocking for stylesheets).
function __recordFetchTiming(t0, url, initiator, parsed, opts) {
  const p = globalThis.performance;
  if (!p || typeof p._recordResource !== 'function') return;
  const o = {
    name: String(url || ''),
    startTime: t0,
    duration: Math.max(0, performance.now() - t0),
    initiatorType: initiator || 'fetch',
    responseStatus: parsed && parsed.status ? parsed.status : 0,
    encodedBodySize: 0, decodedBodySize: 0, transferSize: 0,
  };
  if (parsed && parsed.bodyBase64) {
    const bytes = parsed.bodyBase64.length ? _base64ToUint8Array(parsed.bodyBase64) : [];
    o.encodedBodySize = o.decodedBodySize = bytes.length;
  } else if (parsed && typeof parsed.body === 'string') {
    o.encodedBodySize = o.decodedBodySize = parsed.body.length;
  }
  if (o.encodedBodySize) {
    // Response headers are real wire cost: approximate with the
    // serialized `name: value\r\n` pairs.
    let hb = 0;
    if (parsed && parsed.headers && typeof parsed.headers === 'object') {
      for (const [k, v] of Object.entries(parsed.headers)) hb += String(k).length + String(v).length + 4;
    }
    o.transferSize = o.encodedBodySize + hb;
  }
  if (opts) for (const k of Object.keys(opts)) o[k] = opts[k];
  p._recordResource(o);
}

if (typeof Headers === "undefined") {
  globalThis.Headers = class Headers {
    constructor(init={}) { this._h={}; if(init) { if(init instanceof Headers) { init.forEach((v,k)=>{this._h[k]=v;}); } else if(typeof init==="object") { for(const[k,v]of Object.entries(init)) this._h[k.toLowerCase()]=String(v); } }  __hideOwn(this);}
    get(n) { return this._h[n.toLowerCase()]??null; } set(n,v) { this._h[n.toLowerCase()]=String(v); }
    has(n) { return n.toLowerCase() in this._h; } delete(n) { delete this._h[n.toLowerCase()]; }
    append(n,v) { this._h[n.toLowerCase()]=String(v); }
    forEach(cb) { for(const[k,v] of Object.entries(this._h)) cb(v,k,this); }
    entries() { return Object.entries(this._h)[Symbol.iterator](); }
    keys() { return Object.keys(this._h)[Symbol.iterator](); }
    values() { return Object.values(this._h)[Symbol.iterator](); }
    [Symbol.iterator]() { return this.entries(); }
  };
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

globalThis.requestIdleCallback = globalThis.requestIdleCallback || function requestIdleCallback(cb, opts) {
  const start = Date.now();
  return setTimeout(() => {
    cb({
      didTimeout: false,
      timeRemaining() { return Math.max(0, 50 - (Date.now() - start)); },
    });
  }, 1);
};
globalThis.cancelIdleCallback = globalThis.cancelIdleCallback || function cancelIdleCallback(id) { clearTimeout(id); };
_markNative(globalThis.requestIdleCallback);
_markNative(globalThis.cancelIdleCallback);

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
// Returns the one Selection instance for a document (cached on the document),
// so window.getSelection() === document.getSelection(). The real Selection
// class is defined below, after Range. _selectionFor is hoisted.
function _selectionFor(doc) {
  if (!doc) return null;
  if (!doc._selection) doc._selection = new Selection(doc);
  return doc._selection;
}
globalThis.getSelection = _markNative(function getSelection() {
  return _selectionFor(globalThis.document);
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
function _isConstructorCE(v) {
  if (typeof v !== 'function') return false;
  try { Reflect.construct(function () {}, [], v); return true; } catch (e) { return false; }
}
const _CE_RESERVED = new Set(['annotation-xml', 'color-profile', 'font-face', 'font-face-src', 'font-face-uri', 'font-face-format', 'font-face-name', 'missing-glyph']);
function _isValidCustomElementName(name) {
  if (typeof name !== 'string' || _CE_RESERVED.has(name)) return false;
  // PotentialCustomElementName (approx): lowercase start, a hyphen, no uppercase.
  return /^[a-z][a-z0-9._\u00b7\u00c0-\uffff-]*-[a-z0-9._\u00b7\u00c0-\uffff-]*$/.test(name);
}
class CustomElementRegistry {
  constructor() { this._registry = new Map(); this._byCtor = new Map(); this._whenDefinedResolvers = new Map(); this._defining = false;  __hideOwn(this);}
  define(name, cls, opts) {
    if (!_isConstructorCE(cls)) throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': parameter 2 is not a constructor.");
    if (!_isValidCustomElementName(name)) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': \"" + name + "\" is not a valid custom element name", "SyntaxError");
    if (this._defining) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': operation is not supported while a definition is in progress", "NotSupportedError");
    if (this._registry.has(name)) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': the name \"" + name + "\" has already been used with this registry", "NotSupportedError");
    if (this._byCtor.has(cls)) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': the constructor has already been used with this registry", "NotSupportedError");
    this._defining = true;
    try {
      // attributeChangedCallback + observedAttributes are read once, here, by
      // the ce_attr platform module (a later prototype patch is never called;
      // errors reading them abort the define, as in Chrome).
      globalThis.__diting_ceCapture?.(cls);
      this._byCtor.set(cls, name); this._defineInner(name, cls, opts);
    } finally { this._defining = false; }
  }
  _defineInner(name, cls, opts) {
    this._registry.set(name, cls);
    // Upgrade existing matching elements: instantiate the class on each,
    // fire connectedCallback if the element is in the document. Without
    // this, lit / MusicKit / Polymer components never wire up their
    // shadow DOM or render, leaving heavy chunks of YouTube,
    // music.apple.com, and any web-component site as empty shells.
    try {
      const matches = globalThis.document?.querySelectorAll(name) || [];
      for (const el of matches) this._upgradeElement(el, cls);
    } catch (e) {}
    const resolvers = this._whenDefinedResolvers.get(name);
    if (resolvers) {
      for (const r of resolvers) r(cls);
      this._whenDefinedResolvers.delete(name);
    }
  }
  _upgradeElement(el, cls) {
    if (el.__customUpgraded) return;
    el.__customUpgraded = true;
    try {
      // Swap the prototype instead of copying prototype methods onto the
      // instance: the wrapper is per-nid cached (same node, same instance
      // forever), so one swap upgrades every later lookup too, and methods
      // stay on the prototype where author code (and instanceof) expects
      // them. cls.prototype chains to HTMLElement.prototype, so the engine
      // levels below stay reachable.
      Object.setPrototypeOf(el, cls.prototype);
      // Run the author's constructor against the live element. Reflect
      // construct drives the real [[Construct]] path (cls.call is dead for
      // classes), and the base HTMLElement constructor sees
      // __ceUpgradeTarget and returns this element, which rebinds `this`
      // for the rest of the chain — constructor field initializers
      // (attachShadow, reactive stores, event wiring) land on the element.
      const prev = __ceUpgradeTarget;
      __ceUpgradeTarget = el;
      try { Reflect.construct(cls, []); } catch (e) {}
      finally { __ceUpgradeTarget = prev; }
      // Upgrade reactions: one attributeChangedCallback(name, null, value)
      // per observed attribute already present, before connectedCallback
      // (Chrome probe: ctor, acc name, acc url, conn) — the ce_attr module.
      globalThis.__diting_ceUpgradeReactions?.(el, cls);
      if (typeof el.connectedCallback === 'function' && globalThis.document?.contains?.(el)) {
        // Mark the connection so the insertion/removal walks can cycle it
        // (Chrome fires connectedCallback again on every re-insertion).
        el.__customConnected = true;
        try { el.connectedCallback(); } catch (e) {}
      }
    } catch (e) {}
  }
  get(name) { return this._registry.get(name); }
  getName(cls) {
    if (!_isConstructorCE(cls)) throw new TypeError("Failed to execute 'getName' on 'CustomElementRegistry': parameter 1 is not a constructor.");
    return this._byCtor.has(cls) ? this._byCtor.get(cls) : null;
  }
  whenDefined(name) {
    if (!_isValidCustomElementName(name)) return Promise.reject(new DOMException("Failed to execute 'whenDefined' on 'CustomElementRegistry': \"" + name + "\" is not a valid custom element name", "SyntaxError"));
    const cls = this._registry.get(name);
    if (cls) return Promise.resolve(cls);
    return new Promise((resolve) => {
      const list = this._whenDefinedResolvers.get(name) || [];
      list.push(resolve);
      this._whenDefinedResolvers.set(name, list);
    });
  }
  upgrade(root) {
    if (!root || !root.querySelectorAll) return;
    for (const [name, cls] of this._registry.entries()) {
      const matches = root.querySelectorAll(name);
      for (const el of matches) this._upgradeElement(el, cls);
    }
  }
}
globalThis.CustomElementRegistry = CustomElementRegistry;
globalThis.customElements = new CustomElementRegistry();
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
// Normalize one Blob part to bytes. `native` newline normalization applies to
// string parts when the Blob/File `endings` option is "native".
function _blobPartConvert(p) {
  if (p == null) return { bytes: new Uint8Array(0) };
  if (typeof Blob === "function" && p instanceof Blob) return { bytes: p._bytes || new Uint8Array(0) };
  if (p instanceof ArrayBuffer) return { bytes: new Uint8Array(p.slice(0)) };
  if (ArrayBuffer.isView(p)) return { bytes: new Uint8Array(p.buffer.slice(p.byteOffset, p.byteOffset + p.byteLength)) };
  return { str: String(p) };
}
function _bytesToBinaryString(bytes) { let s = ""; for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]); return s; }
if (typeof Blob === "undefined") globalThis.Blob = class Blob {
  constructor(parts, opts) {
    opts = opts || {};
    // WebIDL converts arguments left to right: every part's union conversion
    // (ToString side effects included) fires before the options dictionary
    // is touched — Chrome pins part-toString > endings > type — and each bag
    // getter must fire exactly once (`x != null ? String(x)` reads twice).
    const conv = [];
    if (parts != null) {
      if (typeof parts === "string" || typeof parts[Symbol.iterator] !== "function") throw new TypeError("Failed to construct 'Blob': The provided value cannot be converted to a sequence.");
      for (const p of parts) conv.push(_blobPartConvert(p));
    }
    const endingsRaw = opts.endings;
    const endings = endingsRaw != null ? String(endingsRaw) : "transparent";
    if (endings !== "transparent" && endings !== "native") throw new TypeError("Failed to construct 'Blob': The provided value '" + endings + "' is not a valid enum value of type EndingType.");
    const native = endings === "native";
    const chunks = []; let total = 0;
    const enc = new TextEncoder();
    for (const c of conv) {
      const b = c.bytes != null ? c.bytes : enc.encode(native ? c.str.replace(/\r\n|\r|\n/g, "\n") : c.str);
      chunks.push(b); total += b.length;
    }
    const data = new Uint8Array(total); let off = 0;
    for (const c of chunks) { data.set(c, off); off += c.length; }
    this._bytes = data;
    __diting_liftSlot(this).size = total;
    const typeRaw = opts.type;
    const t = typeRaw != null ? String(typeRaw) : "";
    __diting_liftSlot(this).type = /^[\x20-\x7e]*$/.test(t) ? t.toLowerCase() : "";
    __hideOwn(this);
  }
  get [Symbol.toStringTag]() { return "Blob"; }
  slice(start, end, contentType) {
    const len = this.size;
    const s = start === undefined ? 0 : (start < 0 ? Math.max(len + start, 0) : Math.min(start, len));
    let e = end === undefined ? len : (end < 0 ? Math.max(len + end, 0) : Math.min(end, len));
    if (e < s) e = s;
    const out = new Blob([], contentType != null ? { type: contentType } : {});
    out._bytes = this._bytes.slice(s, e);
    out.size = out._bytes.length;
    return out;
  }
  text() { return Promise.resolve(new TextDecoder().decode(this._bytes)); }
  arrayBuffer() { return Promise.resolve(_arrayBufferFromBytes(this._bytes)); }
  bytes() { return Promise.resolve(this._bytes.slice()); }
};
if (typeof File === "undefined") globalThis.File = class File extends Blob {
  constructor(parts, name, opts) {
    if (arguments.length < 2) throw new TypeError("Failed to construct 'File': 2 arguments required, but only " + arguments.length + " present.");
    opts = opts || {};
    super(parts, opts);
    const fs = __diting_liftSlot(this);
    fs.name = String(name);
    fs.lastModified = opts.lastModified != null ? Number(opts.lastModified) : Date.now();
  }
  get [Symbol.toStringTag]() { return "File"; }
};
// (#58) DataTransfer: the standard file-upload injection pattern is
// `const dt = new DataTransfer(); dt.items.add(file); input.files = dt.files;`
// (Puppeteer's setInputFiles lineage). Zero-arg constructible since Chrome
// 60; drag-only surfaces (setDragImage, dropEffect outside a real drag)
// are inert but present.
if (typeof DataTransfer === "undefined") globalThis.DataTransfer = class DataTransfer {
  constructor() { this._dtItems = []; this._dropEffect = 'none'; this._effectAllowed = 'none'; }
  get dropEffect() { return this._dropEffect; }
  set dropEffect(v) { v = String(v); if (v === 'none' || v === 'copy' || v === 'link' || v === 'move') this._dropEffect = v; }
  get effectAllowed() { return this._effectAllowed; }
  set effectAllowed(v) { v = String(v); if (/^(none|copy|copyLink|copyMove|link|linkMove|move|all|uninitialized)$/.test(v)) this._effectAllowed = v; }
  get types() {
    // Spec: the string items' types in order, then "Files" once if any file
    // item is present — order does not follow insertion.
    const t = [];
    for (const it of this._dtItems) if (it.kind === 'string' && !t.includes(it.type)) t.push(it.type);
    if (this._dtItems.some((i) => i.kind === 'file') && !t.includes('Files')) t.push('Files');
    return t;
  }
  get files() { return _makeFileList(this._dtItems.filter((i) => i.kind === 'file').map((i) => i.file)); }
  get items() {
    const dt = this;
    if (!dt._dtItemsView) {
      const view = {
        get length() { return dt._dtItems.length; },
        add(a, b) {
          if (b === undefined && a instanceof File) {
            const it = { kind: 'file', type: a.type || '', file: a };
            dt._dtItems.push(it);
          } else if (typeof a === 'string') {
            // Single-argument string adds throw per spec; accept as text/plain —
            // the common page-level usage never checks the return value.
            dt._dtItems.push({ kind: 'string', type: b === undefined ? 'text/plain' : String(b), data: a });
          } else {
            return null; // file+format pair (spec: throw) — inert no-op
          }
          const item = dt._dtItems[dt._dtItems.length - 1];
          return {
            get kind() { return item.kind; },
            get type() { return item.type; },
            getAsFile() { return item.kind === 'file' ? item.file : null; },
            getAsString(cb) { if (item.kind === 'string' && typeof cb === 'function') queueMicrotask(() => cb(item.data)); },
          };
        },
        clear() { dt._dtItems.length = 0; },
        remove(i) { if (i >= 0 && i < dt._dtItems.length) dt._dtItems.splice(i, 1); },
      };
      Object.defineProperty(view, Symbol.toStringTag, { value: 'DataTransferItemList' });
      // Indexed access reads through to the live backing array (the view
      // outlives add/remove calls), each index returning a fresh item face.
      dt._dtItemsView = new Proxy(view, {
        get(t, k) {
          if (typeof k === 'string' && /^\d+$/.test(k)) {
            const it = dt._dtItems[+k];
            if (!it) return undefined;
            return {
              get kind() { return it.kind; },
              get type() { return it.type; },
              getAsFile() { return it.kind === 'file' ? it.file : null; },
              getAsString(cb) { if (it.kind === 'string' && typeof cb === 'function') queueMicrotask(() => cb(it.data)); },
            };
          }
          return t[k];
        },
      });
    }
    return dt._dtItemsView;
  }
  setDragImage() {}
  get [Symbol.toStringTag]() { return 'DataTransfer'; }
};
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
// Real structured clone (not JSON). JSON.parse(JSON.stringify) silently drops
// ArrayBuffer/TypedArray (they serialize to {}), so Cloudflare's turnstile
// orchestrate loses every byte it tries to round-trip through postMessage and
// the challenge never completes (issue #389). Clone buffers, typed arrays,
// maps/sets, dates, errors, and plain objects recursively; platform objects
// that register a clone hook (see crypto.subtle, registered in the WebCrypto
// group) are routed there.
function _structuredClone(value, seen) {
  // Functions and symbols are not structured-cloneable (HTML structured clone,
  // DataCloneError). This must run before the primitive early-return below,
  // which would otherwise pass them through by reference.
  if (typeof value === "function" || typeof value === "symbol") {
    throw new DOMException("Failed to execute 'structuredClone': value could not be cloned.", "DataCloneError");
  }
  if (value === null || typeof value !== "object") return value;
  if (seen.has(value)) return seen.get(value);
  // Typed arrays: copy the underlying buffer slice. DataView has no .slice(),
  // so slice its buffer over the view's range and wrap a fresh view.
  if (ArrayBuffer.isView(value)) {
    if (value instanceof DataView) {
      const buf = value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength);
      const copy = new DataView(buf);
      seen.set(value, copy);
      return copy;
    }
    const Ctor = value.constructor;
    const copy = new Ctor(value.slice());
    seen.set(value, copy);
    return copy;
  }
  if (value instanceof ArrayBuffer) {
    const copy = value.slice(0);
    seen.set(value, copy);
    return copy;
  }
  if (typeof SharedArrayBuffer !== "undefined" && value instanceof SharedArrayBuffer) {
    return value; // transferable, not copyable
  }
  if (value instanceof Date) return new Date(value.getTime());
  if (value instanceof RegExp) return new RegExp(value.source, value.flags);
  if (value instanceof Map) {
    const m = new Map();
    seen.set(value, m);
    for (const [k, v] of value) m.set(_structuredClone(k, seen), _structuredClone(v, seen));
    return m;
  }
  if (value instanceof Set) {
    const s = new Set();
    seen.set(value, s);
    for (const v of value) s.add(_structuredClone(v, seen));
    return s;
  }
  // Platform objects (consulted BEFORE the generic Error path: a native DOMException is an Error by prototype but keeps its state in slots) that carry internal slots opt into cloning via a hook
  // (CryptoKey re-registers its key material so the clone stays usable by
  // crypto.subtle). Anything else with a registered hook takes that path.
  if (typeof value[Symbol.toStringTag] === "string" && globalThis.__diting_clone_hooks) {
    const hook = globalThis.__diting_clone_hooks[value[Symbol.toStringTag]];
    if (typeof hook === "function") return hook(value, seen);
  }
  if (value instanceof Error) {
    const Ctor = value.constructor || Error;
    const e = new Ctor(value.message);
    // Record the clone before recursing into `cause`, otherwise a cycle
    // through the error (e.cause === e) recurses until the stack overflows.
    seen.set(value, e);
    if (value.name) e.name = value.name;
    if (value.stack) e.stack = value.stack;
    if (value.cause !== undefined) e.cause = _structuredClone(value.cause, seen);
    return e;
  }
  // Plain objects clone onto Object.prototype (like Chrome), not the source's
  // prototype. Define each property instead of assigning it: a source with an
  // own enumerable `__proto__` data prop (what JSON.parse('{"__proto__":…}')
  // yields) would otherwise hit the inherited __proto__ setter and reparent
  // the clone instead of copying the property.
  const out = Array.isArray(value) ? [] : {};
  seen.set(value, out);
  for (const k in value) {
    if (Object.prototype.hasOwnProperty.call(value, k)) {
      const cloned = _structuredClone(value[k], seen);
      // Only `__proto__` needs defineProperty: plain assignment would hit the
      // inherited prototype setter and reparent the clone instead of adding an
      // own data property. Every other key takes the fast assignment path.
      if (k === "__proto__") {
        Object.defineProperty(out, k, {
          value: cloned,
          writable: true,
          enumerable: true,
          configurable: true,
        });
      } else {
        out[k] = cloned;
      }
    }
  }
  // Symbols are not enumerable via for-in; copy own symbol-keyed properties.
  const syms = Object.getOwnPropertySymbols(value);
  for (const s of syms) {
    const d = Object.getOwnPropertyDescriptor(value, s);
    if (d && "value" in d) out[s] = _structuredClone(d.value, seen);
  }
  return out;
}
globalThis.structuredClone = globalThis.structuredClone || ((v) => _structuredClone(v, new Map()));
globalThis.reportError = globalThis.reportError || ((e) => console.error(e));

// Latin1 binary strings per the HTML spec, not UTF-8: Chrome's btoa("é") is
// "6Q==" (the single byte 0xE9), and anything above 0xFF throws
// InvalidCharacterError — the old TextEncoder version diverged on both
// counts (two-byte UTF-8 output, silent acceptance of astral chars), which
// breaks signature-style round-trips against servers that expect the spec.
globalThis.btoa = globalThis.btoa || ((s) => {
  // WebIDL DOMString: every argument is ToString-coerced first, so btoa(123)
  // is "MTIz" and btoa(null) is "bnVsbA==" — matching atob's String(s) below.
  s = String(s);
  for (let i = 0; i < s.length; i++)
    if (s.charCodeAt(i) > 0xFF)
      throw new DOMException("Failed to execute 'btoa' on 'Window': The string to be encoded contains characters outside of the Latin1 range.", "InvalidCharacterError");
  const c="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"; let r="";
  for(let i=0;i<s.length;i+=3){const a=s.charCodeAt(i),b=s.charCodeAt(i+1)||0,cc=s.charCodeAt(i+2)||0; r+=c[a>>2]+c[((a&3)<<4)|(b>>4)]+(i+1<s.length?c[((b&15)<<2)|(cc>>6)]:"=")+(i+2<s.length?c[cc&63]:"=");} return r; });
// Decoding must fail loud on non-alphabet input: a stray char used to decode
// to garbage bytes through indexOf's -1 instead of throwing, so corrupt
// payloads sailed through parsers. ASCII whitespace is ignored, '=' is
// padding (at most two, tail only), and a lone trailing char (%4 === 1) is
// not a decodable quantum.
globalThis.atob = globalThis.atob || ((s) => {
  const bad = () => new DOMException("Failed to execute 'atob' on 'Window': The string to be decoded is not correctly encoded.", "InvalidCharacterError");
  const c="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let body = String(s).replace(/[ \t\n\f\r]/g, "");
  if (body.endsWith("=")) body = body.slice(0, -1);
  if (body.endsWith("=")) body = body.slice(0, -1);
  if (body.includes("=") || body.length % 4 === 1) throw bad();
  const v=[];
  for (let i = 0; i < body.length; i++) {
    const x = c.indexOf(body[i]);
    if (x < 0) throw bad();
    v.push(x);
  }
  let r=[]; for(let i=0;i<v.length;i+=4){const a=v[i],b=v[i+1],cc=v[i+2],d=v[i+3]; r.push((a<<2)|(b>>4)); if(cc!==undefined)r.push(((b&15)<<4)|(cc>>2)); if(d!==undefined)r.push(((cc&3)<<6)|d);} let out=""; for (let k=0;k<r.length;k++) out += String.fromCharCode(r[k]); return out; });

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
globalThis.screenX = 0; globalThis.screenY = 0;
globalThis.screenLeft = 0; globalThis.screenTop = 0;
globalThis.pageXOffset = 0; globalThis.pageYOffset = 0;
globalThis.scrollX = 0; globalThis.scrollY = 0;

// CSS.supports routes to the Rust @supports evaluator (op_css_supports).
// The old `return false` stub answered "unsupported" to every probe, so
// feature-detecting pages took fallback render branches that diverged from
// their SSR HTML — doudian's publish page then failed React hydration
// (#418) and discarded the whole server DOM (#203).
// 刀88: typed-om unit family (Chrome 145 truth k88-typedom/k88-mini).
// CSSUnitValue is a real constructible value object — value rw / unit ro as
// PROTOTYPE accessors (strict unit set throws V8's own getter-only message),
// no own enumerable keys, constructor LAST in own-name order. The unit set
// is exactly the 64 CSS factory names, canonical unit = lowercased name
// (Hz→hz, kHz→khz, Q→q); factories take a plain double (CSS.px(CSS.px(3))
// throws non-finite in 145 — no CSSNumericValue bypass on the factories,
// only the CSSMath* ctors accept numeric values).
const __diting_cssUnits = {};
for (const __u of ['Hz', 'Q', 'cap', 'ch', 'cm', 'cqb', 'cqh', 'cqi', 'cqmax', 'cqmin', 'cqw', 'deg', 'dpcm', 'dpi', 'dppx', 'dvb', 'dvh', 'dvi', 'dvmax', 'dvmin', 'dvw', 'em', 'ex', 'fr', 'grad', 'ic', 'in', 'kHz', 'lh', 'lvb', 'lvh', 'lvi', 'lvmax', 'lvmin', 'lvw', 'mm', 'ms', 'number', 'pc', 'percent', 'pt', 'px', 'rad', 'rcap', 'rch', 'rem', 'rex', 'ric', 'rlh', 's', 'svb', 'svh', 'svi', 'svmax', 'svmin', 'svw', 'turn', 'vb', 'vh', 'vi', 'vmax', 'vmin', 'vw', 'x']) __diting_cssUnits[__u.toLowerCase()] = true;
globalThis.CSSUnitValue = function CSSUnitValue(value, unit) {
  if (arguments.length < 2) throw new TypeError("Failed to construct 'CSSUnitValue': 2 arguments required, but only " + arguments.length + " present.");
  const v = Number(value);
  if (!isFinite(v)) throw new TypeError("Failed to construct 'CSSUnitValue': The provided double value is non-finite.");
  const u = String(unit);
  if (!__diting_cssUnits[u]) throw new TypeError("Failed to construct 'CSSUnitValue': Invalid unit: " + u);
  __def(this, '_v', v);
  __def(this, '_u', u);
  __hideOwn(this);
};
Object.defineProperty(globalThis.CSSUnitValue.prototype, 'value', { get() { return this._v; }, set(v) { this._v = Number(v); }, enumerable: true, configurable: true });
Object.defineProperty(globalThis.CSSUnitValue.prototype, 'unit', { get() { return this._u; }, enumerable: true, configurable: true });
{ const __P = globalThis.CSSUnitValue.prototype, __c = __P.constructor; delete __P.constructor; Object.defineProperty(__P, 'constructor', { value: __c, writable: true, configurable: true }); }
const __diting_cssReg = new Set();
const __diting_cssFactory = (name, unit) => {
  const f = function (v) {
    if (arguments.length < 1) throw new TypeError("Failed to execute '" + name + "' on 'CSS': 1 argument required, but only 0 present.");
    const n = Number(v);
    if (!isFinite(n)) throw new TypeError("Failed to execute '" + name + "' on 'CSS': The provided double value is non-finite.");
    return new globalThis.CSSUnitValue(n, unit);
  };
  try { Object.defineProperty(f, 'name', { value: name, configurable: true }); } catch (e) {}
  try { Object.defineProperty(f, 'length', { value: 1, configurable: true }); } catch (e) {}
  return f;
};

globalThis.CSS = {
  supports(a, b) {
    try {
      if (typeof b === 'string') return _OPS.op_css_supports(a + ':' + b);
      return _OPS.op_css_supports(String(a));
    } catch { return false; }
  },
  escape(s) {
    s = String(s);
    return s.replace(/[^a-zA-Z0-9_-]/g, (c) => '\\' + c);
  },
  // registerProperty is owned by css_register.js (registry + syntax + Rust
  // cascade mirror); it installs onto this namespace after bootstrap.
};
{
  const __facs = ['Hz', 'Q', 'cap', 'ch', 'cm', 'cqb', 'cqh', 'cqi', 'cqmax', 'cqmin', 'cqw', 'deg', 'dpcm', 'dpi', 'dppx', 'dvb', 'dvh', 'dvi', 'dvmax', 'dvmin', 'dvw', 'em', 'ex', 'fr', 'grad', 'ic', 'in', 'kHz', 'lh', 'lvb', 'lvh', 'lvi', 'lvmax', 'lvmin', 'lvw', 'mm', 'ms', 'number', 'pc', 'percent', 'pt', 'px', 'rad', 'rcap', 'rch', 'rem', 'rex', 'ric', 'rlh', 's', 'svb', 'svh', 'svi', 'svmax', 'svmin', 'svw', 'turn', 'vb', 'vh', 'vi', 'vmax', 'vmin', 'vw', 'x'];
  for (const k of __facs) Object.defineProperty(globalThis.CSS, k, { value: __diting_cssFactory(k, k.toLowerCase()), writable: true, enumerable: true, configurable: true });
}

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
// Type of element.style / getComputedStyle(); the class binding above is
// lexical, so without this `window.CSSStyleDeclaration` was undefined and
// `el.style instanceof CSSStyleDeclaration` threw (upstream a0e1ba5).
globalThis.CSSStyleDeclaration = CSSStyleDeclaration;
globalThis.DOMStringMap = DOMStringMap;
const _EventTarget = globalThis.EventTarget;
// conn picks up the EventTarget methods through the chain, exactly like
// Chrome (NetworkInformation.prototype → EventTarget.prototype), so they
// are never own names of the first prototype.
Object.setPrototypeOf(NetworkInformation.prototype, _EventTarget.prototype);
// BatteryManager: same chain rule (M3 B2 — the old trio were inert stubs;
// getBattery()'s instance is an Object.create of this prototype).
Object.setPrototypeOf(globalThis.BatteryManager.prototype, _EventTarget.prototype);
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
  navigator.getBattery, navigator.getGamepads, navigator.sendBeacon,
  navigator.javaEnabled, navigator.geolocation?.getCurrentPosition,
  navigator.geolocation?.watchPosition,
  navigator.serviceWorker?.register,
  navigator.serviceWorker?.addEventListener,
  navigator.serviceWorker?.removeEventListener,
  navigator.serviceWorker?.dispatchEvent,
  navigator.serviceWorker?.getRegistration,
  navigator.permissions?.query, navigator.credentials?.get,
  navigator.storage?.estimate, navigator.storage?.persist, navigator.storage?.persisted,
  globalThis.matchMedia,
  globalThis.getSelection, globalThis.requestAnimationFrame,
  globalThis.cancelAnimationFrame, globalThis.setTimeout, globalThis.clearTimeout,
  globalThis.setInterval, globalThis.clearInterval, globalThis.queueMicrotask,
  globalThis.structuredClone, globalThis.reportError,
  globalThis.btoa, globalThis.atob,
  console.log, console.warn, console.error, console.info, console.debug,
  console.dir, console.assert,
  Notification, Notification.requestPermission,
  window.chrome?.csi, window.chrome?.loadTimes,
  MutationObserver, ResizeObserver, PerformanceObserver,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });



const _OrigDateTimeFormat = Intl.DateTimeFormat;
// Derive the reported timezone from the configured language so the persona
// stays coherent (zh-CN + Europe/Berlin — the old hardcoded default — is a
// mismatch risk engines notice).
// Accept-Language may arrive as a raw header ("zh-CN,zh;q=0.9,en;q=0.8").
// navigator.language must be a single BCP-47 tag and navigator.languages a
// tag list — a q-weighted string in either is a hard headless tell.
function __ditingLangList() {
  // CDP Emulation.setLocaleOverride pin outranks the persona language
  // (Chrome's locale override outranks the accept-language). Pin-over-
  // derivation, same shape as __diting_tz over __ditingTZFromLang.
  const pin = globalThis.__diting_locale;
  const raw = String((typeof pin === 'string' && pin.length) ? pin
    : (globalThis.__diting_lang || 'zh-CN'));
  const tags = raw.split(',').map((s) => s.split(';')[0].trim()).filter(Boolean);
  const out = tags.length ? tags : ['zh-CN'];
  if (out.length === 1 && out[0].indexOf('-') !== -1) out.push(out[0].split('-')[0]);
  return out;
}
function __ditingTZFromLang() {
  const lang = __ditingLangList()[0].toLowerCase();
  const map = {
    'zh': 'Asia/Shanghai', 'ja': 'Asia/Tokyo', 'ko': 'Asia/Seoul',
    'en': 'America/New_York', 'de': 'Europe/Berlin', 'fr': 'Europe/Paris',
    'es': 'Europe/Madrid', 'it': 'Europe/Rome', 'ru': 'Europe/Moscow',
    'pt': 'Europe/Lisbon', 'nl': 'Europe/Amsterdam', 'tr': 'Europe/Istanbul',
  };
  for (const k of Object.keys(map)) {
    if (lang.startsWith(k)) return map[k];
  }
  return 'Asia/Shanghai';
}
// CDP Emulation.setTimezoneOverride. Empty means the language-derived zone.
function __ditingTZ() {
  const pin = globalThis.__diting_tz;
  if (typeof pin === 'string' && pin.length) return pin;
  return __ditingTZFromLang();
}
// Intl's *default* locale comes from the process locale (V8/ICU follows
// LANG), not from the configured language — so on a host running under
// LANG=en_US, navigator.language says zh-CN while
// Intl.DateTimeFormat().resolvedOptions().locale says en-US. Same class of
// mismatch as the timezone one above, and an equally hard headless tell.
// The Rust side pins ICU's default for fresh isolates, but V8 caches the
// resolved default per-isolate after first Intl use, so the authoritative
// fix lives here: bind undefined/null locale args to the configured
// language. Explicit locales pass through untouched.
function __ditingLocaleArg(locales) {
  if (locales === undefined || locales === null) return __ditingLangList()[0];
  return locales;
}
// Wrapping a native constructor with a plain JS function is itself a
// fingerprint: Function.prototype.toString stops returning [native code],
// name/length drift (stock Intl constructors are all length 0), and
// prototype.constructor stops closing on Intl.X. A Proxy with only a
// construct trap forwards everything else to the native target — the spec
// resolves a callable proxy's source text from its target — so nothing has
// to be forged by hand. It also keeps "class constructor requires new":
// a plain wrapper is callable without new and silently returns an
// instance, the proxy's default apply forwards to the native and throws
// like stock.
// Registry backing the toString disguise below: wrapped proxy -> native
// original it stands in for.
const __ditingNativeOf = new Map();
// Shared per-call binding for the DateTimeFormat proxies.
function __ditingDTFArgs(args) {
  if (!args[1]) args[1] = {};
  if (!args[1].timeZone) args[1].timeZone = __ditingTZ();
  args[0] = __ditingLocaleArg(args[0]);
  return args;
}
// V8 names the proxy-trap frame in error stacks after the handler's
// constructor and the trap key: a plain object handler reads
// "Object.construct" between the caller and the native — a stack tell that
// only shows when the constructor throws (independent differ run, obscura#734
// thread). Carrying the traps on a class named after the wrapped constructor
// relabels the frame to match stock's constructor name. The extra frame is
// inherent to trapping in JS and cannot be removed, so this is a relabel.
function __ditingHandler(ctorName, traps) {
  const Carrier = { [ctorName]: class {} }[ctorName];
  return Object.assign(new Carrier(), traps);
}
Intl.DateTimeFormat = new Proxy(_OrigDateTimeFormat, __ditingHandler('DateTimeFormat', {
  construct(target, args, newTarget) {
    return Reflect.construct(target, __ditingDTFArgs(args), newTarget);
  },
  // ECMA-402 legacy: these constructors stay callable without new. The
  // proxy's default call forwarding resolves the legacy NewTarget to the
  // inner native, which skips the construct trap — so bind here too.
  apply(target, thisArg, args) {
    return Reflect.construct(target, __ditingDTFArgs(args), target);
  },
}));
__ditingNativeOf.set(Intl.DateTimeFormat, _OrigDateTimeFormat);
Object.defineProperty(_OrigDateTimeFormat.prototype, 'constructor', {
  value: Intl.DateTimeFormat, writable: true, enumerable: false, configurable: true,
});
const _origResolved = _OrigDateTimeFormat.prototype.resolvedOptions;
_OrigDateTimeFormat.prototype.resolvedOptions = new Proxy(_origResolved, {
  apply(target, thisArg, args) {
    const r = Reflect.apply(target, thisArg, args);
    if (r.timeZone === 'UTC') r.timeZone = __ditingTZ();
    return r;
  },
});
__ditingNativeOf.set(_OrigDateTimeFormat.prototype.resolvedOptions, _origResolved);
// Date's own clock still follows the host zone unless these methods read
// the same pin Intl uses. A proxy over the native keeps
// Function.prototype.toString on the original.
// #221: the DTF is cached per zone — the local-field getters below call
// this on every access and would otherwise allocate a fresh formatter per
// getHours() on hot paths.
let _ditingOffDTF = null, _ditingOffDTFTZ = '';
function __ditingOffsetMinutes(date) {
  const tz = __ditingTZ();
  if (_ditingOffDTFTZ !== tz) {
    _ditingOffDTF = new _OrigDateTimeFormat('en-US', {
      timeZone: tz, hourCycle: 'h23',
      year: 'numeric', month: '2-digit', day: '2-digit',
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
    _ditingOffDTFTZ = tz;
  }
  const parts = _ditingOffDTF.formatToParts(date);
  const pick = (t) => Number(parts.find((p) => p.type === t).value);
  let hour = pick('hour');
  if (hour === 24) hour = 0;
  const asUTC = Date.UTC(pick('year'), pick('month') - 1, pick('day'), hour, pick('minute'), pick('second'));
  return Math.round((date.getTime() - asUTC) / 60000);
}
function __ditingFormatZoned(date, kind) {
  if (Number.isNaN(date.getTime())) return 'Invalid Date';
  const tz = __ditingTZ();
  const dtf = new _OrigDateTimeFormat('en-US', {
    timeZone: tz, hourCycle: 'h23', weekday: 'short', year: 'numeric',
    month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit',
    timeZoneName: 'longOffset',
  });
  const bag = {};
  for (const p of dtf.formatToParts(date)) if (p.type !== 'literal') bag[p.type] = p.value;
  // Zone NAME is the one locale-dependent piece of Date.prototype.toString:
  // zh-Chrome prints "\u2026GMT+0800 (中国标准时间)" while weekday/month stay the
  // spec's English abbreviations. Chrome resolves it from the UI locale; we
  // take the persona's first language tag (same source as navigator.language).
  const uiLang = (typeof __ditingLangList === 'function' ? __ditingLangList()[0] : 'en-US');
  const long = new _OrigDateTimeFormat(uiLang, { timeZone: tz, timeZoneName: 'long' })
    .formatToParts(date).find((p) => p.type === 'timeZoneName');
  let off = bag.timeZoneName || 'GMT';
  off = off.replace(/GMT([+-])(\d{1,2}):(\d{2})/, (_, s, h, m) => 'GMT' + s + h.padStart(2, '0') + m);
  off = off.replace(/GMT([+-])(\d{1,2})$/, (_, s, h) => 'GMT' + s + h.padStart(2, '0') + '00');
  const name = long ? long.value : tz;
  // Date.prototype.toString zero-pads the day-of-month ("Oct 04", spec
  // step 9 of TimeString: ToTwoDigit). 'numeric' above yields "4", and the
  // unpadded form broke SSR text-node comparison on doudian (#203): the
  // server rendered "04", we rendered "4", React threw #418 and discarded
  // the whole server DOM for the client fallback path.
  const day = String(bag.day).padStart(2, '0');
  const clock = bag.hour + ':' + bag.minute + ':' + bag.second;
  const datePart = bag.weekday + ' ' + bag.month + ' ' + day + ' ' + bag.year;
  if (kind === 'date') return datePart;
  if (kind === 'time') return clock + ' ' + off + ' (' + name + ')';
  return datePart + ' ' + clock + ' ' + off + ' (' + name + ')';
}
function __ditingWrapDate(name, impl) {
  const orig = Date.prototype[name];
  if (typeof orig !== 'function') return;
  const wrapped = new Proxy(orig, {
    apply(target, thisArg, args) {
      if (!(thisArg instanceof Date)) return Reflect.apply(target, thisArg, args);
      return impl(target, thisArg, args);
    },
  });
  __ditingNativeOf.set(wrapped, orig);
  Date.prototype[name] = wrapped;
}
__ditingWrapDate('getTimezoneOffset', (_target, date) => __ditingOffsetMinutes(date));
__ditingWrapDate('toString', (_target, date) => __ditingFormatZoned(date, 'all'));
__ditingWrapDate('toDateString', (_target, date) => __ditingFormatZoned(date, 'date'));
__ditingWrapDate('toTimeString', (_target, date) => __ditingFormatZoned(date, 'time'));
__ditingWrapDate('toLocaleString', (target, date, args) => {
  const options = Object.assign({}, args[1]);
  if (!options.timeZone) options.timeZone = __ditingTZ();
  return Reflect.apply(target, date, [args[0], options]);
});
__ditingWrapDate('toLocaleDateString', (target, date, args) => {
  const options = Object.assign({}, args[1]);
  if (!options.timeZone) options.timeZone = __ditingTZ();
  return Reflect.apply(target, date, [args[0], options]);
});
__ditingWrapDate('toLocaleTimeString', (target, date, args) => {
  const options = Object.assign({}, args[1]);
  if (!options.timeZone) options.timeZone = __ditingTZ();
  return Reflect.apply(target, date, [args[0], options]);
});
// #221: the local-field surface must resolve in the persona zone, not the
// host's. Above, the formatter family (toString/…) already formats in
// __ditingTZ() — but the constructor's local-args form and the native
// getter/setter families still resolved in the HOST zone, so on a TZ=UTC
// deployment toString() said "13:06:07 GMT+0800" while getHours() on the
// same instant returned 5. Any page diffing the two faces saw the mismatch.
// Local wall = instant − offset(instant), offset DST-aware via the cached
// DTF above; reading getUTC* off the shifted instant yields exact persona
// fields, and inverting (wall via Date.UTC, plus one offset lookup at the
// target wall — the pre-transition convention V8 itself uses for walls
// inside a DST gap) rebuilds instants from persona fields.
const _OrigDate = Date;
const _ditingHostNativeOffset = __ditingNativeOf.get(Date.prototype.getTimezoneOffset);
const _ditingWall = new _OrigDate(NaN);
function __ditingWallMs(date) {
  const t = date.getTime();
  if (Number.isNaN(t)) { _ditingWall.setTime(NaN); return _ditingWall; }
  _ditingWall.setTime(t - __ditingOffsetMinutes(date) * 60000);
  return _ditingWall;
}
// Fast path: when the host zone's offset matches the persona zone at 8
// probes across a year, the native local-field methods are already
// persona-correct and nothing needs shifting (the dev box in Shanghai runs
// zero overhead; only a TZ-mismatched host pays the wrap cost). Revalidated
// whenever the pin changes, same cache discipline as the offset DTF.
let _ditingHostOK = false, _ditingHostOKTZ = '\0';
function __ditingHostMatchesTZ() {
  const tz = __ditingTZ();
  if (_ditingHostOKTZ !== tz) {
    _ditingHostOK = true;
    const base = Date.UTC(2026, 0, 15);
    for (let i = 0; i < 8; i++) {
      const probe = new _OrigDate(base + i * 45 * 86400000);
      if (__ditingOffsetMinutes(probe) !== _ditingHostNativeOffset.call(probe)) {
        _ditingHostOK = false;
        break;
      }
    }
    _ditingHostOKTZ = tz;
  }
  return _ditingHostOK;
}
for (const [local, utc] of [
  ['getFullYear', 'getUTCFullYear'], ['getMonth', 'getUTCMonth'],
  ['getDate', 'getUTCDate'], ['getDay', 'getUTCDay'],
  ['getHours', 'getUTCHours'], ['getMinutes', 'getUTCMinutes'],
  ['getSeconds', 'getUTCSeconds'], ['getMilliseconds', 'getUTCMilliseconds'],
]) {
  __ditingWrapDate(local, (target, date) => __ditingHostMatchesTZ()
    ? Reflect.apply(target, date, [])
    : __ditingWallMs(date)[utc]());
}
__ditingWrapDate('getYear', (target, date) => __ditingHostMatchesTZ()
  ? Reflect.apply(target, date, [])
  : __ditingWallMs(date).getUTCFullYear() - 1900);
// Setters: rebuild the full persona field vector (current values for the
// fields this setter doesn't mention — Date.UTC's own defaults never leak
// in because only present args overlay), then map back to an instant.
function __ditingZonedSet(date, args, writes) {
  const wall = __ditingWallMs(date);
  const f = [
    wall.getUTCFullYear(), wall.getUTCMonth(), wall.getUTCDate(),
    wall.getUTCHours(), wall.getUTCMinutes(), wall.getUTCSeconds(),
    wall.getUTCMilliseconds(),
  ];
  for (let i = 0; i < writes.length && i < args.length; i++) f[writes[i]] = args[i];
  const w = Date.UTC(f[0], f[1], f[2], f[3], f[4], f[5], f[6]);
  let t = NaN;
  if (!Number.isNaN(w)) {
    _ditingWall.setTime(w);
    t = w + __ditingOffsetMinutes(_ditingWall) * 60000;
  }
  date.setTime(t);
  return t;
}
const _ditingSetSpec = {
  setFullYear: [0, 1, 2], setMonth: [1, 2], setDate: [2],
  setHours: [3, 4, 5, 6], setMinutes: [4, 5, 6], setSeconds: [5, 6],
  setMilliseconds: [6],
};
for (const name of Object.keys(_ditingSetSpec)) {
  const writes = _ditingSetSpec[name];
  __ditingWrapDate(name, (target, date, args) => __ditingHostMatchesTZ()
    ? Reflect.apply(target, date, args)
    : __ditingZonedSet(date, args, writes));
}
__ditingWrapDate('setYear', (target, date, args) => {
  if (__ditingHostMatchesTZ()) return Reflect.apply(target, date, args);
  let y = Number(args[0]);
  if (y >= 0 && y <= 99) y += 1900;
  return __ditingZonedSet(date, [y], [0]);
});
// Constructor, local-args form: interpret the fields in the persona zone.
// Single-string/numeric/date/zero-arg forms pass through untouched, and
// Date.UTC's argument defaults mirror the constructor's own exactly.
Date = new Proxy(_OrigDate, __ditingHandler('Date', {
  construct(target, args, newTarget) {
    if (args.length >= 2 && !__ditingHostMatchesTZ()) {
      let y = Number(args[0]);
      if (y >= 0 && y <= 99) y += 1900;
      // Pass exactly the args the caller gave: an explicit undefined IS
      // "present" per spec and would NaN the whole time value.
      const parts = [y].concat(Array.prototype.slice.call(args, 1));
      const w = Date.UTC.apply(null, parts);
      let t = w;
      if (!Number.isNaN(w)) {
        _ditingWall.setTime(w);
        t = w + __ditingOffsetMinutes(_ditingWall) * 60000;
      }
      return Reflect.construct(target, [t], newTarget);
    }
    return Reflect.construct(target, args, newTarget);
  },
  apply(target, thisArg, args) {
    return __ditingFormatZoned(new _OrigDate(), 'all');
  },
}));
__ditingNativeOf.set(Date, _OrigDate);
Object.defineProperty(_OrigDate.prototype, 'constructor', {
  value: Date, writable: true, enumerable: false, configurable: true,
});
// The remaining (locales, options) Intl constructors get the same
// default-locale binding via the same proxy treatment.
(function() {
  const names = ['NumberFormat', 'Collator', 'PluralRules', 'RelativeTimeFormat',
                 'ListFormat', 'DisplayNames', 'Segmenter'];
  for (const n of names) {
    const Orig = Intl[n];
    if (typeof Orig !== 'function' || !Orig.prototype) continue;
    Intl[n] = new Proxy(Orig, __ditingHandler(n, {
      construct(target, args, newTarget) {
        args[0] = __ditingLocaleArg(args[0]);
        return Reflect.construct(target, args, newTarget);
      },
      apply(target, thisArg, args) {
        args[0] = __ditingLocaleArg(args[0]);
        return Reflect.construct(target, args, target);
      },
    }));
    __ditingNativeOf.set(Intl[n], Orig);
    Object.defineProperty(Orig.prototype, 'constructor', {
      value: Intl[n], writable: true, enumerable: false, configurable: true,
    });
  }
})();
// V8 renders Function.prototype.toString on a proxy over a native as the
// anonymous "function () { [native code] }" — stock shows the name
// ("function NumberFormat() { [native code] }"). Close that last gap by
// routing toString through a proxy of the real Function.prototype.toString
// that answers for the wrapped set from the original and reflects
// everything else untouched. This covers both Intl.X.toString() and a
// direct Function.prototype.toString.call(Intl.X); the disguise itself
// still renders native because the spec resolves a callable proxy's source
// text from its target.
const __ditingFnToString = Function.prototype.toString;
Function.prototype.toString = new Proxy(__ditingFnToString, {
  apply(target, thisArg, args) {
    if (__ditingNativeOf.has(thisArg)) {
      return __ditingFnToString.call(__ditingNativeOf.get(thisArg));
    }
    return Reflect.apply(target, thisArg, args);
  },
});

if (typeof navigator.credentials === 'undefined') {
  navigator.credentials = { get(){return Promise.resolve(null);}, create(){return Promise.resolve(null);}, store(){return Promise.resolve();}, preventSilentAccess(){return Promise.resolve();} };
}

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
globalThis.requestIdleCallback = globalThis.requestIdleCallback || function(cb) { return setTimeout(cb, 0); };
globalThis.cancelIdleCallback = globalThis.cancelIdleCallback || function(id) { clearTimeout(id); };

// Geometry family (DOMPoint/DOMRect/DOMQuad/DOMMatrix*/WebKitCSSMatrix):
// generated shape + behavior module now owns this whole family
// (platform/gen/geometry.gen.js + platform/geometry.impl.js, M4 knife 1).



// Real network sockets aren't implemented; we don't have a runtime WS / SSE
// client in V8. But pages that wait for an `open` event (Vite HMR clients
// embedded on docs sites, live-dashboards, anything calling
// `await new Promise(r => ws.addEventListener('open', r))`) silently hang
// forever otherwise. Fire `open` after a microtask so the consumer at least
// proceeds; subsequent messages never arrive, which is no worse than the
// current "no signal whatsoever" behaviour.
if (typeof EventSource === 'undefined') {
  globalThis.EventSource = class EventSource {
    constructor(url, init) {
      __def(this, '_url', url);
      __def(this, '_readyState', 0); // CONNECTING
      __def(this, '_withCredentials', !!(init && init.withCredentials));
      this.onopen = null; this.onmessage = null; this.onerror = null;
      Promise.resolve().then(() => {
        if (this.readyState !== 0) return;
        __def(this, '_readyState', 1); // OPEN
        // 刀56: the native dispatch fires on* itself (handler hook) — the
        // manual onopen call is gone or it would fire twice.
        try { this.dispatchEvent(new Event('open')); } catch (e) {}
      });
      __hideOwn(this);
    }
    close() { __def(this, '_readyState', 2); }
    static CONNECTING = 0; static OPEN = 1; static CLOSED = 2;
  };
  __accRo(globalThis.EventSource, ['url', 'readyState', 'withCredentials']);
  __acc(globalThis.EventSource, ['onopen', 'onmessage', 'onerror']);
}

if (typeof BroadcastChannel === 'undefined') {
  // Same-origin message bus. Chrome delivers each post as a task to every
  // OTHER open channel with the same name — across tabs, dedicated workers,
  // and SharedWorkers alike, with the payload structured-cloned per
  // receiver. PDD's titan channel rides it both ways: the page posts
  // queryStatus on the SharedWorker port and the worker's replyStatus →
  // createSocket handshake comes back over BroadcastChannel('WebSocketChannel')
  // (2026-10-12: the old postMessage(){} stub hung mms goods_list at
  // loading forever — the SDK waits for a reply that never came). One realm
  // here means one registry covers every "context". A channel with a live
  // message handler stays reachable through the registry, matching the
  // spec's liveness rule (the page SDK drops its own reference).
  const _bcReg = new Map();
  globalThis.BroadcastChannel = class BroadcastChannel {
    constructor(name) {
      __def(this, '_name', String(name));
      this.onmessage = null;
      this.onmessageerror = null;
      this._closed = false;
      let set = _bcReg.get(this.name);
      if (!set) { set = new Set(); _bcReg.set(this.name, set); }
      set.add(this);
      __hideOwn(this);
    }
    postMessage(msg) {
      if (this._closed) {
        throw new DOMException("Failed to execute 'postMessage' on 'BroadcastChannel': the channel is closed.", 'InvalidStateError');
      }
      const set = _bcReg.get(this.name);
      if (!set) return;
      for (const ch of set) {
        if (ch === this || ch._closed) continue;
        _runAsMacrotask(() => {
          if (ch._closed) return;
          ch._fire(structuredClone(msg));
        });
      }
    }
    // 刀56: a REAL MessageEvent through the native dispatch — the hand-rolled
    // fake ({type,data,target,currentTarget} plain object) failed
    // `ev instanceof MessageEvent` and fired onmessage manually; the native
    // core stamps target/currentTarget and fires on* via the handler hook.
    _fire(data) {
      try { this.dispatchEvent(new MessageEvent('message', { data })); } catch (e) {}
    }
    close() {
      if (this._closed) return;
      this._closed = true;
      const set = _bcReg.get(this.name);
      if (set) { set.delete(this); if (!set.size) _bcReg.delete(this.name); }
    }
  };
  __accRo(globalThis.BroadcastChannel, ['name']);
  __acc(globalThis.BroadcastChannel, ['onmessage', 'onmessageerror']);
}


if (typeof Selection === 'undefined') {
  globalThis.Selection = class Selection {
    constructor(){this.anchorNode=null;this.focusNode=null;this.rangeCount=0;this.isCollapsed=true;this.type='None';}
    getRangeAt(){return null;} collapse(){} extend(){} selectAllChildren(){} deleteFromDocument(){}
    addRange(){} removeRange(){} removeAllRanges(){} toString(){return '';}
  };
}

if (typeof Range === 'undefined') {
  globalThis.Range = class Range {
    constructor(){this.startContainer=null;this.startOffset=0;this.endContainer=null;this.endOffset=0;this.collapsed=true;this.commonAncestorContainer=null;}
    setStart(n,o){this.startContainer=n;this.startOffset=o;} setEnd(n,o){this.endContainer=n;this.endOffset=o;}
    collapse(){} selectNode(){} selectNodeContents(){} cloneContents(){return document?.createDocumentFragment();}
    deleteContents(){} insertNode(){} getBoundingClientRect(){return new DOMRect();}
    getClientRects(){return [];} cloneRange(){return new Range();} toString(){return '';}
  };
}

// SharedWorker (#213): one script instance per (url, name), with a real
// MessagePort pair bridging every constructor call to it. The old stub
// swallowed each port.postMessage and never replied, so an SDK that
// coordinates or signs through a worker hung on its first request — 抖店's
// sif-dxs security layer builds exactly that channel (self.onconnect →
// event.ports[0] → port.start(), then every page request waits on it) and the
// page died on the SDK's own timeout with a blank screen. Scripts execute in
// the page realm like the dedicated-worker path; sharing is per page and per
// (url, name), which is what a single document can observe.
if (typeof SharedWorker === 'undefined') {
  globalThis.__sharedWorkerInstances = globalThis.__sharedWorkerInstances || new Map();
  function _sharedWorkerKey(url, name) {
    let abs = String(url == null ? '' : url);
    try { abs = new URL(abs, (globalThis.location && globalThis.location.href) || 'https://example.com/').href; } catch {}
    return abs + '\u0000' + (name == null ? '' : String(name));
  }
  // SharedWorkerGlobalScope shape: the common worker surface plus onconnect.
  // No postMessage on the global — messages travel on connected ports, which
  // arrive as the connect event's ports[0], exactly as Chrome delivers them.
  function _sharedWorkerScope(shim) {
    const scope = _workerSurfaceCommon(_workerImportScripts(shim));
    scope.onconnect = null;
    scope.onmessage = null;
    scope.addEventListener = (type, fn) => { scope['on' + type] = fn; };
    scope.removeEventListener = (type, fn) => { if (scope['on' + type] === fn) scope['on' + type] = null; };
    scope.close = () => { shim._closed = true; };
    scope.self = scope;
    return scope;
  }
  function _runSharedScript(inst) {
    const scope = inst.scope;
    const fn = new Function('self', 'window', 'document', 'navigator', 'location', 'importScripts',
      'with (self) {\n' + inst.code + '\n}');
    const restore = _maskMainThreadGlobals();
    try { fn(_workerScopeChain(scope), undefined, undefined, scope.navigator, scope.location, scope.importScripts); }
    finally { restore(); }
  }
  function _connectSharedPort(inst, workerEnd) {
    const scope = inst.scope;
    if (!scope) return;
    const evt = { type: 'connect', ports: [workerEnd], target: scope, currentTarget: scope };
    // Worker-code entry point: mask the page global for the synchronous
    // dispatch (the wheel reads the DOM family off globalThis via
    // webpack's global helper, which no with-scope can shadow).
    const restore = _maskMainThreadGlobals();
    try {
      // A SharedWorker script may register onconnect (the spec path) or, in
      // the wild, a plain onmessage; either way the port is what carries the
      // traffic, and the worker-side port buffers page messages sent before
      // the handler attached.
      if (typeof scope.onconnect === 'function') scope.onconnect(evt);
      else if (typeof scope.onmessage === 'function') scope.onmessage(evt);
    } catch (e) {
      console.error('SharedWorker error:', e && e.message, e && e.stack);
      if (typeof inst.onerror === 'function') { try { inst.onerror({ message: String((e && e.message) || e), error: e }); } catch (_) {} }
    } finally { restore(); }
  }
  globalThis.SharedWorker = class SharedWorker {
    constructor(url, name) {
      this.onerror = null;
      __hideOwn(this);
      const key = _sharedWorkerKey(url, name);
      let inst = globalThis.__sharedWorkerInstances.get(key);
      if (!inst) {
        inst = { code: null, scope: null, ready: false, onerror: null, shim: { _url: null, _imported: {} }, pendingPorts: [] };
        globalThis.__sharedWorkerInstances.set(key, inst);
        _workerSourceFor(url)
          .then(res => {
            inst.shim._url = res.fetchUrl;
            return _preloadWorkerImports(inst.shim, res.code).then(() => res.code);
          })
          .then(code => {
            inst.code = code;
            if (typeof code === 'string') {
              try {
                inst.scope = _sharedWorkerScope(inst.shim);
                _runSharedScript(inst);
              } catch (e) {
                console.error('SharedWorker error:', e && e.message, e && e.stack);
                inst.error = e;
              }
            }
            inst.ready = true;
            const ports = inst.pendingPorts;
            inst.pendingPorts = [];
            for (const p of ports) _connectSharedPort(inst, p);
          });
      }
      const [pageEnd, workerEnd] = _newPortPair();
      if (inst.ready && inst.scope) _connectSharedPort(inst, workerEnd);
      else inst.pendingPorts.push(workerEnd);
      __diting_liftSlot(this).port = pageEnd;
    }
  };
  try { _markNative(globalThis.SharedWorker); } catch (_) {}
}
if (typeof ServiceWorkerContainer === 'undefined') {
  globalThis.ServiceWorkerContainer = class { register(){return Promise.resolve();} getRegistrations(){return Promise.resolve([]);} };
}

if (typeof URLPattern === 'undefined') {
  globalThis.URLPattern = class URLPattern {
    constructor(pattern){this._pattern=pattern||{};} test(){return false;} exec(){return null; __hideOwn(this);}
  };
}


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
try {
  if (globalThis.Screen && !Object.getOwnPropertyDescriptor(Screen.prototype, 'onchange')) {
    Object.defineProperty(Screen.prototype, 'onchange', {
      get() { return null; }, set() {}, configurable: true, enumerable: true,
    });
  }
  __ditingReorderProto(Screen.prototype, ['availWidth','availHeight','width','height','colorDepth','pixelDepth','availLeft','availTop','orientation','constructor','onchange','isExtended']);
} catch (e) {}
// (300015 P6) WebIDL layout puts constructor LAST on these two; class
// syntax always emits it first. Chrome order captured 1015.
try {
  __ditingReorderProto(NetworkInformation.prototype, ['onchange','effectiveType','rtt','downlink','saveData','constructor']);
  __ditingReorderProto(NavigatorUAData.prototype, ['brands','mobile','platform','getHighEntropyValues','toJSON','constructor']);
} catch (e) {}
try {
  __ditingReorderProto(Navigator.prototype, [
    'vendorSub','productSub','vendor','maxTouchPoints','scheduling','userActivation','geolocation','doNotTrack',
    'webkitTemporaryStorage','webkitPersistentStorage','windowControlsOverlay','hardwareConcurrency','cookieEnabled',
    'appCodeName','appName','appVersion','platform','product','userAgent','language','languages','onLine','webdriver',
    'plugins','mimeTypes','pdfViewerEnabled','connection','getGamepads','javaEnabled','sendBeacon','vibrate','constructor',
    'deprecatedRunAdAuctionEnforcesKAnonymity','protectedAudience','bluetooth','clipboard','credentials',
    'keyboard','managed','mediaDevices','serviceWorker','virtualKeyboard','wakeLock','deviceMemory','userAgentData',
    'locks','storage','gpu','login','ink','mediaCapabilities','permissions','devicePosture','hid','mediaSession',
    'presentation','serial','usb','xr','storageBuckets','adAuctionComponents','runAdAuction','canLoadAdAuctionFencedFrame',
    'canShare','share','clearAppBadge','getBattery','getUserMedia','requestMIDIAccess','requestMediaKeySystemAccess',
    'setAppBadge','webkitGetUserMedia','clearOriginJoinedAdInterestGroups','createAuctionNonce','joinAdInterestGroup',
    'leaveAdInterestGroup','updateAdInterestGroups','deprecatedReplaceInURN','deprecatedURNToURL','getInstalledRelatedApps',
    'getInterestGroupAdAuctionData','registerProtocolHandler','unregisterProtocolHandler',
  ]);
} catch (e) {}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('CSSStyleDeclaration', CSSStyleDeclaration);
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
  a('__ceUpgradeTarget', () => __ceUpgradeTarget, (v) => { __ceUpgradeTarget = v; });
  d('__currentUrl', __currentUrl);
  d('__def', __def);
  d('__disconnectCustomElementsIn', __disconnectCustomElementsIn);
  d('__ditingBrandList', __ditingBrandList);
  d('__ditingLangList', __ditingLangList);
  d('__ditingLocaleArg', __ditingLocaleArg);
  d('__ditingPlatformFromUA', __ditingPlatformFromUA);
  d('__hideOwn', __hideOwn);
  d('__locParts', __locParts);
  d('__prepareInitialStylesheets', __prepareInitialStylesheets);
  d('__prepareInsertedScript', __prepareInsertedScript);
  d('__prepareInsertedStylesIn', __prepareInsertedStylesIn);
  d('__prepareInsertedStylesheetLink', __prepareInsertedStylesheetLink);
  d('__prepareInsertedStylesheetLinksIn', __prepareInsertedStylesheetLinksIn);
  d('__prepareInsertedSubtree', __prepareInsertedSubtree);
  d('__recordFetchTiming', __recordFetchTiming);
  d('__upgradeCustomElementsIn', __upgradeCustomElementsIn);
  d('_arrayBufferFromBytes', _arrayBufferFromBytes);
  d('_base64ToUint8Array', _base64ToUint8Array);
  d('_bodyToUint8Array', _bodyToUint8Array);
  d('_bytesToBase64', _bytesToBase64);
  d('_bytesToBinaryString', _bytesToBinaryString);
  d('_cancelAnimationsInSubtree', _cancelAnimationsInSubtree);
  d('_cancelPreClickFlip', _cancelPreClickFlip);
  d('_cancelTransitionsInSubtree', _cancelTransitionsInSubtree);
  d('_commitPreClickFlip', _commitPreClickFlip);
  d('_concatBytes', _concatBytes);
  d('_dataUrlBytes', _dataUrlBytes);
  d('_decodeBodyWithCharset', _decodeBodyWithCharset);
  d('_decodeDataScriptUrl', _decodeDataScriptUrl);
  d('_dispatchAnimationEvent', _dispatchAnimationEvent);
  d('_ditingDisplayNone', _ditingDisplayNone);
  d('_ditingEpochNow', _ditingEpochNow);
  d('_ditingFontBox', _ditingFontBox);
  d('_ditingIframeDoc', _ditingIframeDoc);
  d('_ditingIframeViewport', _ditingIframeViewport);
  d('_ditingLayoutBox', _ditingLayoutBox);
  d('_ditingLocalBox', _ditingLocalBox);
  d('_ditingOffsetParent', _ditingOffsetParent);
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
  d('_installedFontFactor', _installedFontFactor);
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
  d('_namedBoot', _namedBoot);
  d('_namedEls', _namedEls);
  d('_namedNames', _namedNames);
  d('_namedScanIn', _namedScanIn);
  a('_namedScanned', () => _namedScanned, (v) => { _namedScanned = v; });
  d('_namedTrack', _namedTrack);
  d('_namedUnscanIn', _namedUnscanIn);
  d('_namedUntrack', _namedUntrack);
  d('_namedUntrackBy', _namedUntrackBy);
  d('_newCssDecl', _newCssDecl);
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
  d('_raceAbortSignal', _raceAbortSignal);
  d('_registerIframe', _registerIframe);
  d('_registerIframesIn', _registerIframesIn);
  d('_resolveUrl', _resolveUrl);
  d('_runAsMacrotask', _runAsMacrotask);
  d('_runSrcdocScripts', _runSrcdocScripts);
  d('_scheduleAfter', _scheduleAfter);
  d('_scheduleAnimationCheck', _scheduleAnimationCheck);
  d('_scheduleTransitionCheck', _scheduleTransitionCheck);
  d('_selectionFor', _selectionFor);
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
