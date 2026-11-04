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

globalThis.__diting_shadowHostNames = new Set(['article','aside','blockquote','body','div','footer','h1','h2','h3','h4','h5','h6','header','main','nav','p','section','span']);

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

[
  // (刀64) matchMedia moved to media_queries.js — the install wraps
  // _markNative at its site (marking travels, 刀22/刀54 precedent).
  // (刀54) the timer/raf faces moved to timer_belt.js — their native
  // marking migrated there (bootstrap loads first; 刀22 precedent).
  globalThis.structuredClone, globalThis.reportError,
  console.log, console.warn, console.error, console.info, console.debug,
  console.dir, console.assert,
  // (刀22) Notification + chrome.csi/loadTimes moved to bchk_apis.js —
  // their native marking migrated there (bootstrap loads first and would
  // see undefined here; the member reads also hid from the analyzer).
PerformanceObserver,
  // (刀70) ResizeObserver moved to resize_observer.js — its marking
  // migrated there (install site self-marks, 刀22/刀54 precedent);
  // (刀58) MutationObserver moved to mutation_observer.js — same.
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
  d('_DOM_MUTATION_COMMANDS', _DOM_MUTATION_COMMANDS);
  d('_OPS', _OPS);
  d('__acc', __acc);
  d('__accRo', __accRo);
  d('__currentUrl', __currentUrl);
  d('__def', __def);
  d('__ditingReorderProto', __ditingReorderProto);
  d('__hideOwn', __hideOwn);
  d('__locParts', __locParts);
  d('_ditingEpochNow', _ditingEpochNow);
  a('_ditingMutationEpoch', () => _ditingMutationEpoch, (v) => { _ditingMutationEpoch = v; });
  d('_dom', _dom);
  d('_domParse', _domParse);
  d('_domRaw', _domRaw);
  d('_edgeMemo', _edgeMemo);
  d('_eventState', _eventState);
  d('_locationObj', _locationObj);
  d('_markNativeProto', _markNativeProto);
  d('_mirrorSelectLabel', _mirrorSelectLabel);
  d('_nodeInDocument', _nodeInDocument);
  d('_resolveUrl', _resolveUrl);
  d('_shallowCloneNode', _shallowCloneNode);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
