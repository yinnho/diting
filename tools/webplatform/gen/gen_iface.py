#!/usr/bin/env python3
"""WebIDL -> JS interface skeleton (JS-PLATFORM-ARCH sec 3.2, M2 v0).

usage: gen_iface.py <spec.json> <out.js> <module-name> [truth.json]

Reads a slice written by extract_spec.py and emits ONE module file
(crates/diting/js/platform/gen/<x>.gen.js) with a manifest header. The shape
(constructor, prototype descriptors, arity, brand checks, conversions, Chrome
message wording) comes from the IDL; behavior is delegated:

  strategy "backing"  interface WITHOUT constructor that declares named
                      getter/setter/deleter (a legacy platform object, e.g.
                      Storage). Instances are Proxy objects created by
                      exports.create(backing); `backing` implements the IDL
                      members by name (+ supportedPropertyNames()). The named
                      property semantics are Chrome's, captured in
                      chrome/chrome-storage-truth.json (NOT the bare spec text:
                      names found on the prototype chain stay ordinary).
  strategy "event"    interface WITH constructor inheriting Event, plus a
                      dictionary <Name>Init: attributes live in an internal
                      slot filled from the dictionary; init<Name>() re-inits.
  strategy "value"    pure-value interface WITH constructor (no named
                      specials, not an Event subclass): the geometry family.
                      The generated file owns the SHAPE only - ctor shells
                      (brand check, arg count), prototype accessors/methods
                      (kind + install order from the Chrome truth overlay,
                      arity from the IDL required-arg count), statics,
                      inheritance, @@toStringTag, P.expect registrations.
                      Every body delegates to the behavior module required at
                      load: B.ctor/get/set/call/stat('<Iface>', '<member>',
                      ...). Behavior (math, defaults, dict validation, CSS
                      parsing, error wording) lives in the paired impl module.
  strategy "patch-common" (argv[5] == 'patch-common') no new interfaces:
                      AUGMENT existing native prototypes to Chrome's member
                      placement. M4 knife 2: Chrome 145 owns ~110 on* on
                      HTMLElement.prototype (+SVGElement+Document) and 8 on
                      Element.prototype, while the engine bootstrap installed
                      98 on Element.prototype; ~46 common members are missing
                      or misplaced. The generator consumes
                      chrome/htmlelement-truth-145.json (HT.order/kinds,
                      EL.order/kinds, on.levels, fn.length) plus the
                      declarative tables below and emits, IN CHROME'S OWN
                      ORDER, one statement per member: relocate the misplaced
                      descriptor from Element.prototype (deleting it there),
                      install a reflected attribute via P.reflector (data in
                      the REFLECT table), or delegate to the impl module
                      (non-uniform semantics). Idempotent: every seat is
                      guarded, so regeneration on a drifted engine re-runs
                      cleanly. Knife 4 landed the staged families (part/
                      Typed OM/fullscreen trio/pointer lock/moveBefore/
                      ariaNotify + moveBefore's Document/DocumentFragment
                      hosts); STAGED is the reserve for future knives.
  strategy "patch-media" (argv[5] == 'patch-media') no new interfaces:
                      AUGMENT HTMLMediaElement/HTMLVideoElement/
                      HTMLImageElement prototypes to Chrome's COMPLETE member
                      sets (M4 knife 5): installs go in place (no cross-proto
                      relocation - engine src/name on Element.prototype are
                      shadowed), three kind drifts are fixed in place, and a
                      reseat() walk re-orders each prototype into Chrome's
                      exact own order. Consumes chrome/htmlmedia-truth-145.json
                      (*.order/*.kinds/fn.length); uniform reflection is DATA
                      (P.reflector: this family grew the unsigned-long/
                      crossorigin/url kinds); the rest delegates to the
                      htmlmedia impl module.

v0 covers what Storage/StorageEvent and the geometry family need; new
strategies are added per family.
"""
import json, os, sys

spec_path, out_path, module = sys.argv[1], sys.argv[2], sys.argv[3]
spec = json.load(open(spec_path))
# Chrome overlay (optional): truth file with `order.<Interface>` (storage
# legacy) or `shape.<Interface>` = {order, members, statics} (value strategy,
# from chrome/geometry-truth-145.json). Third-party shape facts (install
# order, accessor-vs-data kind) come from real Chrome, never guesswork.
_truth_path = sys.argv[4] if len(sys.argv) > 4 else os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "chrome", "chrome-storage-truth.json")
_tj = json.load(open(_truth_path)) if os.path.exists(_truth_path) else {}
# storage truth files wrap the probe result in {"result": ...}; geometry
# truth (probe_run output) is the result at top level.
TRUTH = _tj["result"] if "result" in _tj else _tj
N = spec["names"]
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
rel_spec = os.path.relpath(spec_path, ROOT)


def tname(t):
    return t["idlType"] if isinstance(t["idlType"], str) else None


def is_nullable(t):
    return bool(t.get("nullable"))


def conv_expr(t, v, ctx_js, argno=None, what="arg", fn="__f"):
    """JS expression converting value expression `v` per IDL type `t`.
    ctx_js is a JS expression for the 'Failed to ...' prefix."""
    n = tname(t)
    nullable = is_nullable(t)
    if n in ("DOMString", "USVString"):
        c = "conv.%s(%s, %s, %s)" % (n, v, ctx_js, fn)
    elif n == "unsigned long":
        c = "conv.unsignedLong(%s, %s, %s)" % (v, ctx_js, fn)
    elif n == "boolean":
        c = "conv.boolean(%s)" % v
    elif n in N and N[n]["type"] == "interface":
        # interface type: brand check against that interface's slots
        if what == "dict":
            c = "(is%s(%s) ? %s : P.fail(%s, \"Failed to convert value to '%s'.\", %s + \": Failed to read the '\" + FIELD + \"' property from '\" + DICT + \"'\"))" % (n, v, v, fn, n, ctx_js)
        else:
            c = "(is%s(%s) ? %s : P.fail(%s, \"parameter %d is not of type '%s'.\", %s))" % (n, v, v, fn, argno, n, ctx_js)
    else:
        raise SystemExit("gen_iface v0: unsupported IDL type %r" % (n,))
    if nullable:
        return "((%s === null || %s === undefined) ? null : %s)" % (v, v, c)
    return c


def js_default(d):
    if d is None:
        return "undefined"
    if d["type"] == "null":
        return "null"
    if d["type"] == "boolean":
        return "true" if d["value"] else "false"
    if d["type"] == "string":
        return json.dumps(d["value"])
    if d["type"] == "number":
        return d["value"]
    raise SystemExit("unsupported default %r" % (d,))


def members(iface, kinds):
    return [m for m in iface["members"] if m["type"] in kinds]


out = []
codes = {}   # member name -> emitted statement, flushed in overlay order
exports = []
expect = []
requires_load = set()
requires_call = set()
has_value = False
W = out.append


def flush_members(name):
    """Emit collected member statements in Chrome's observed own-property
    order (overlay) when we have it, IDL order otherwise; then seal
    (constructor goes last, as in Chrome)."""
    order = [k for k in TRUTH.get("order." + name, []) if k in codes]
    order += [k for k in codes if k not in order]
    for k in order:
        W(codes[k])
    codes.clear()
    W("P.seal(%s);" % name)


def header_gen(name):
    return "// ---- %s (generated from %s) ----" % (name, rel_spec)


# interface iteration order: parents before children (P.iface links the
# parent ctor at load; JSON order can put a child first).
_iface_names = [n for n, i in N.items() if i["type"] == "interface"]
_seen_order, _done = [], set()


def _visit_iface(n):
    if n in _done or n not in N or N[n]["type"] != "interface":
        return
    parent = N[n].get("inheritance")
    if parent in N and N[parent]["type"] == "interface" and parent in _iface_names:
        _visit_iface(parent)
    _done.add(n)
    _seen_order.append(n)


for _n in _iface_names:
    _visit_iface(n=_n)

# --------------------------------------------------------------------------
# strategy "patch-common": HTMLElement/Element common-member layer (M4 knife 2)
# --------------------------------------------------------------------------
if len(sys.argv) > 5 and sys.argv[5] == "patch-common":
    # ---- declarative tables (DATA; every entry is pinned by a key in the
    # truth file - kinds/defaults by def/set.* keys, placement by HT.order/
    # EL.order/on.levels, arity by fn.length). Reflection semantics that are
    # uniform across members are DATA here and one runtime (P.reflector);
    # members with non-uniform semantics delegate to the impl module.
    REFLECT = {  # name -> (attr, kind, known|None, default, fallback|None)
        "title": ("title", "string", None, None, None),
        "lang": ("lang", "string", None, None, None),
        "dir": ("dir", "string-known", ["", "ltr", "rtl", "auto"], "", None),
        "hidden": ("hidden", "boolean-presence", None, None, None),
        "autofocus": ("autofocus", "boolean-presence", None, None, None),
        "accessKey": ("accesskey", "string", None, None, None),
        "draggable": ("draggable", "boolean-truefalse", None, None, None),
        "inert": ("inert", "boolean-presence", None, None, None),
        "autocapitalize": ("autocapitalize", "known-fallback",
                           ["none", "sentences", "words", "characters"], "", "sentences"),
        "enterKeyHint": ("enterkeyhint", "string-known",
                         ["enter", "done", "go", "next", "previous", "search", "send"], "", None),
        "inputMode": ("inputmode", "string-known",
                      ["none", "text", "tel", "url", "email", "numeric", "decimal", "search"], "", None),
        "virtualKeyboardPolicy": ("virtualkeyboardpolicy", "string-known", ["auto", "manual"], "", None),
        "writingSuggestions": ("writingsuggestions", "known-fallback", ["true", "false"], "true", "true"),
    }
    REFLECT_EL = {"elementTiming": ("elementtiming", "string", None, None, None)}
    IMPL_ACCESSOR_HT = ["translate", "spellcheck", "popover", "nonce",
                        "contentEditable", "outerText", "editContext"]
    IMPL_GETTER_HT = ["isContentEditable", "attributeStyleMap"]
    IMPL_METHOD_HT = {"attachInternals": 0, "showPopover": 0, "hidePopover": 0, "togglePopover": 0}
    IMPL_GETTER_EL = {"prefix": "null", "assignedSlot": "null", "clientLeft": "0",
                      "clientTop": "0", "currentCSSZoom": "1"}
    ARIA_REF_ELEMENTS = [n for n in TRUTH["EL.order"] if n.startswith("aria")
                         and (n.endswith("Element") or n.endswith("Elements"))]
    IMPL_ACCESSOR_EL = ARIA_REF_ELEMENTS  # getter null, setter swallow
    # knife 4: part is a rw accessor with real get/set legs (cached
    # DOMTokenList / attribute write) - not the aria null+swallow shape.
    IMPL_ACCESSOR_EL_RW = ["part"]
    IMPL_METHOD_EL = {"hasAttributeNS": 2, "scrollIntoViewIfNeeded": 0, "webkitMatchesSelector": 1,
                      "computedStyleMap": 0, "moveBefore": 2, "requestFullscreen": 0,
                      "webkitRequestFullScreen": 0, "webkitRequestFullscreen": 0,
                      "requestPointerLock": 0, "ariaNotify": 1}
    # moveBefore also lives on Document/DocumentFragment (truth DOC.moveBefore
    # len 2 / FRAG.moveBefore typeof function; error messages carry the host
    # interface name in the ctx prefix - truth mb.doc).
    MOVE_BEFORE_HOSTS = {"Document": "DOC", "DocumentFragment": "FRAG"}
    STAGED_HT = {}
    # engine bootstrap extras Chrome 145 owns on NONE of the probed levels
    # (on.levels): deprecated/never-shipped handlers. Deleted when present.
    EXTRA_ON_REMOVED = ["ondragexit", "onfocusin", "onfocusout", "ontouchcancel",
                        "ontouchend", "ontouchmove", "ontouchstart"]
    # existing engine read-only faces Chrome ships read-write: the descriptor
    # keeps its native getter, the setter delegates to the impl module.
    KIND_UPGRADE_EL = ["classList", "outerHTML"]
    STAGED_EL = {}
    SVG_SHARE = ["dataset", "style", "tabIndex", "focus", "blur"]  # HTMLOrSVGOrMath + inline-style mixins
    # members the engine ALREADY owns on Element.prototype (bootstrap or
    # native): pure relocation, no install closure. If the engine ever drops
    # one, seat() no-ops and the truth-diff test fails - the guard is the
    # test, not the generator.
    PURE_MOVE = {"offsetParent", "offsetTop", "offsetLeft", "offsetWidth",
                 "offsetHeight", "innerText", "style", "dataset", "tabIndex",
                 "blur", "click", "focus"}

    # ---- build-time assertions: the tables must agree with the truth file
    levels = TRUTH["on.levels"]
    on_ht = [n for n in TRUTH["HT.order"] if n.startswith("on")]
    on_el = sorted(n for n, v in levels.items() if v == "EL+DOC")
    on_doc = sorted(n for n, v in levels.items() if "DOC" in v.split("+"))
    for n, v in levels.items():
        if v == "EL+DOC" and n in on_ht:
            raise SystemExit("patch: %s claimed on both EL and HT" % n)
    for m, ln in IMPL_METHOD_HT.items():
        if TRUTH["fn.length"].get(m) not in (None, ln):
            raise SystemExit("patch: fn.length mismatch for %s" % m)
    for m, ln in IMPL_METHOD_EL.items():
        if TRUTH["fn.length"].get(m) not in (None, ln):
            raise SystemExit("patch: fn.length mismatch for %s" % m)
    if TRUTH["webkitMatchesSelector"][2] != IMPL_METHOD_EL["webkitMatchesSelector"]:
        raise SystemExit("patch: webkitMatchesSelector arity")
    accounted = set(on_ht) | set(on_el) | set(REFLECT) | set(REFLECT_EL) \
        | set(IMPL_ACCESSOR_HT) | set(IMPL_GETTER_HT) | set(IMPL_METHOD_HT) \
        | set(IMPL_GETTER_EL) | set(IMPL_ACCESSOR_EL) | set(IMPL_METHOD_EL) \
        | set(STAGED_HT) | set(STAGED_EL) | {"constructor"} | PURE_MOVE
    for k in TRUTH["HT.order"]:
        if k not in accounted:
            raise SystemExit("patch: HT.order member %r unaccounted" % k)

    po = []
    PW = po.append
    PW("// ---- on* accessors: bootstrap _defineOnHandler semantics (store")
    PW("// __onHandlers per instance; inline content-attribute fallback for")
    PW("// element faces). installOn is idempotent; seat() additionally")
    PW("// RELOCATES a descriptor the bootstrap put on Element.prototype -")
    PW("// Chrome owns it on HTMLElement (+SVGElement for the shared set) -")
    PW("// deleting it from Element.prototype in the same step.")
    PW("function installOn(P, name, fallback) {")
    PW("  if (Object.prototype.hasOwnProperty.call(P, name)) return;")
    PW("  const get = function () {")
    PW("    const store = this.__onHandlers;")
    PW("    if (store && Object.prototype.hasOwnProperty.call(store, name)) return store[name];")
    PW("    if (fallback && typeof this._resolveInlineHandler === 'function') return this._resolveInlineHandler(name);")
    PW("    return null;")
    PW("  };")
    PW("  const set = function (fn) {")
    PW("    if (typeof fn !== 'function') fn = null;")
    PW("    const store = this.__onHandlers || (this.__onHandlers = {});")
    PW("    store[name] = fn;")
    PW("  };")
    PW("  Object.defineProperty(get, 'name', { value: 'get ' + name, configurable: true });")
    PW("  Object.defineProperty(get, 'length', { value: 0, configurable: true });")
    PW("  Object.defineProperty(set, 'name', { value: 'set ' + name, configurable: true });")
    PW("  Object.defineProperty(set, 'length', { value: 1, configurable: true });")
    PW("  _markNative(get); _markNative(set);")
    PW("  Object.defineProperty(P, name, { configurable: true, enumerable: true, get: get, set: set });")
    PW("}")
    PW("function seat(name, install) {")
    PW("  const d = Object.getOwnPropertyDescriptor(EL, name);")
    PW("  if (d) {")
    PW("    Object.defineProperty(HT, name, d);")
    PW("    if (SVG && SVG_SHARE.indexOf(name) >= 0 && !Object.prototype.hasOwnProperty.call(SVG, name)) Object.defineProperty(SVG, name, d);")
    PW("    delete EL[name];")
    PW("  } else if (install && !Object.prototype.hasOwnProperty.call(HT, name)) install();")
    PW("}")
    PW("function seatOn(name) {")
    PW("  const d = Object.getOwnPropertyDescriptor(EL, name);")
    PW("  if (d) { Object.defineProperty(HT, name, d); delete EL[name]; }")
    PW("  else installOn(HT, name, true);")
    PW("}")
    PW("const SVG_SHARE = %s;" % json.dumps(SVG_SHARE))
    PW("function uprw(name, setter) {")
    PW("  Object.defineProperty(setter, 'name', { value: 'set ' + name, configurable: true });")
    PW("  Object.defineProperty(setter, 'length', { value: 1, configurable: true });")
    PW("  _markNative(setter);")
    PW("  const d = Object.getOwnPropertyDescriptor(EL, name);")
    PW("  if (d && d.get && !d.set && !Object.prototype.hasOwnProperty.call(d, 'value')) {")
    PW("    Object.defineProperty(EL, name, { get: d.get, set: setter, enumerable: true, configurable: true });")
    PW("  }")
    PW("}")
    PW("")
    PW("// ---- HTMLElement.prototype, installed in Chrome's own order (HT.order):")
    PW("// the emitted sequence IS the own-property order; `constructor` is")
    PW("// re-seated at Chrome's position (139th of 141).")
    for k in TRUTH["HT.order"]:
        if k == "constructor":
            PW("delete HT.constructor;")
            PW("Object.defineProperty(HT, 'constructor', { value: HTMLElement, writable: true, enumerable: false, configurable: true });")
        elif k in on_ht:
            PW("seatOn('%s');" % k)
        elif k in REFLECT:
            attr, kind, known, dflt, fallback = REFLECT[k]
            spec = {"attr": attr, "kind": kind}
            if known is not None:
                spec["known"] = known
            if dflt is not None:
                spec["default"] = dflt
            if fallback is not None:
                spec["fallback"] = fallback
            # seat() relocates when the bootstrap owns the member on
            # Element.prototype, installs the reflector otherwise; the
            # closure keeps re-runs on a drifted engine idempotent.
            PW("seat('%s', function () { P.reflector(HT, '%s', %s); });" % (k, k, json.dumps(spec, sort_keys=True)))
        elif k in IMPL_ACCESSOR_HT:
            PW("seat('%s', function () { P.accessor(HT, '%s', function __f() { return B.get('%s', this); }, function __f(v) { B.set('%s', this, v); }); });" % (k, k, k, k))
        elif k in IMPL_GETTER_HT:
            PW("seat('%s', function () { P.getter(HT, '%s', function __f() { return B.get('%s', this); }); });" % (k, k, k))
        elif k in IMPL_METHOD_HT:
            ln = IMPL_METHOD_HT[k]
            ctx = "\"Failed to execute '%s' on 'HTMLElement'\"" % k
            PW("seat('%s', function () { P.method(HT, '%s', %d, function __f() { const ctx = %s; P.args(ctx, arguments.length, %d, __f); return B.call('%s', this, ctx, arguments); }); });" % (k, k, ln, ctx, ln, k))
        elif k in STAGED_HT:
            PW("// staged (SURFACE.md): %s - %s" % (k, STAGED_HT[k]))
        else:
            # pure relocation of a member the engine already owns on EL
            PW("seat('%s', null);" % k)
    PW("")
    # arity alignment for MOVED native functions whose engine signature
    # kept optional args Chrome doesn't count (truth fn.length)
    fix_targets = [n for n in TRUTH["fn.length"] if n in set(PURE_MOVE) | set(IMPL_METHOD_HT)]
    PW("// ---- arity alignment for relocated natives (truth fn.length: the")
    PW("// engine declared optional args Chrome doesn't count)")
    PW("function fixArity(name, len) {")
    PW("  const d = Object.getOwnPropertyDescriptor(HT, name);")
    PW("  if (d && typeof d.value === 'function' && d.value.length !== len) { try { Object.defineProperty(d.value, 'length', { value: len, configurable: true }); } catch (e) {} }")
    PW("}")
    for n in sorted(fix_targets):
        PW("fixArity('%s', %d);" % (n, TRUTH["fn.length"][n]))
    PW("")
    PW("// ---- Element.prototype additions (Chrome EL.order order; every")
    PW("// staged family landed at knife 4, so the contract is the FULL order)")
    for k in TRUTH["EL.order"]:
        if k in on_el:
            PW("installOn(EL, '%s', false);" % k)
        elif k in REFLECT_EL:
            attr, kind, known, dflt, fallback = REFLECT_EL[k]
            PW("if (!Object.prototype.hasOwnProperty.call(EL, '%s')) P.reflector(EL, '%s', %s);" % (k, k, json.dumps({"attr": attr, "kind": kind}, sort_keys=True)))
        elif k in IMPL_GETTER_EL:
            PW("if (!Object.prototype.hasOwnProperty.call(EL, '%s')) P.getter(EL, '%s', function __f() { return %s; });" % (k, k, IMPL_GETTER_EL[k]))
        elif k in IMPL_ACCESSOR_EL_RW:
            PW("if (!Object.prototype.hasOwnProperty.call(EL, '%s')) P.accessor(EL, '%s', function __f() { return B.get('%s', this); }, function __f(v) { B.set('%s', this, v); });" % (k, k, k, k))
        elif k in IMPL_ACCESSOR_EL:
            PW("if (!Object.prototype.hasOwnProperty.call(EL, '%s')) P.accessor(EL, '%s', function __f() { return null; }, function __f(v) { B.set('%s', this, v); });" % (k, k, k))
        elif k in IMPL_METHOD_EL:
            ln = IMPL_METHOD_EL[k]
            ctx = "\"Failed to execute '%s' on 'Element'\"" % k
            PW("if (!Object.prototype.hasOwnProperty.call(EL, '%s')) P.method(EL, '%s', %d, function __f() { const ctx = %s; P.args(ctx, arguments.length, %d, __f); return B.call('%s', this, ctx, arguments); });" % (k, k, ln, ctx, ln, k))
    PW("")
    PW("// ---- moveBefore on the other two hosts (truth DOC.moveBefore /")
    PW("// FRAG.moveBefore): same op through the impl, host name in the ctx")
    for iface, varname in sorted(MOVE_BEFORE_HOSTS.items()):
        ctx = "\"Failed to execute 'moveBefore' on '%s'\"" % iface
        PW("if (!Object.prototype.hasOwnProperty.call(%s, 'moveBefore')) P.method(%s, 'moveBefore', 2, function __f() { const ctx = %s; P.args(ctx, arguments.length, 2, __f); return B.call('moveBefore', this, ctx, arguments); });" % (varname, varname, ctx))
    PW("")
    PW("// ---- read-only faces Chrome ships read-write (setter via impl)")
    for k in KIND_UPGRADE_EL:
        PW("uprw('%s', function __f(v) { B.set('%s', this, v); });" % (k, k))
    PW("")
    PW("// ---- engine on* extras Chrome 145 owns on no level (on.levels)")
    for k in EXTRA_ON_REMOVED:
        PW("if (Object.prototype.hasOwnProperty.call(EL, '%s')) delete EL['%s'];" % (k, k))
        PW("if (Object.prototype.hasOwnProperty.call(DOC, '%s')) delete DOC['%s'];" % (k, k))
    PW("")
    PW("// ---- Document.prototype: Chrome owns all 110 on* here; the engine's")
    PW("// 98 were bootstrap-installed - add whatever is missing (no inline")
    PW("// fallback on the document face, matching the bootstrap's choice).")
    PW("for (const n of %s) installOn(DOC, n, false);" % json.dumps(on_doc))
    PW("")
    PW("// ---- SVGElement.prototype: GlobalEventHandlers + the")
    PW("// HTMLOrSVGOrMath/inline-style mixin faces (dataset/style/tabIndex/")
    PW("// focus/blur arrive via seat()'s SVG_SHARE copy; nonce installs here).")
    PW("if (SVG) {")
    PW("  for (const n of %s) installOn(SVG, n, true);" % json.dumps(on_ht))
    PW("  if (!Object.prototype.hasOwnProperty.call(SVG, 'nonce')) P.accessor(SVG, 'nonce', function __f() { return B.get('nonce', this); }, function __f(v) { B.set('nonce', this, v); });")
    PW("}")
    staged_all = sorted(set(STAGED_HT) | set(STAGED_EL))
    pheader = [
        "//! GENERATED by tools/webplatform/gen/gen_iface.py (patch-common strategy) from chrome/htmlelement-truth-145.json (Chrome 145) - do not edit",
        "//! module: %s" % module,
        "//! layer: L2",
        "//! exposes: -",
        "//! requires-load: htmlelement, legacy:_markNative, legacy:Document, legacy:Element, legacy:HTMLElement, legacy:SVGElement, legacy:DocumentFragment",
        "//! requires-call: legacy:_markNative",
        "",
        "// M4 knife 2: Chrome 145's HTMLElement/Element common-member layer.",
        "// Chrome owns 102 on* + 38 members on HTMLElement.prototype, 8 on* on",
        "// Element.prototype, all 110 on* on Document.prototype; the engine",
        "// bootstrap had 98 on* on Element.prototype and HTMLElement.prototype",
        "// bare. Members are seated in Chrome's own order; uniform reflection",
        "// is DATA (P.reflector); non-uniform semantics delegate to the",
        "// htmlelement impl module. Knife 4 landed the last staged families",
        "// (part/Typed OM/fullscreen trio/pointer lock/moveBefore/ariaNotify)",
        "// and moveBefore's Document/DocumentFragment hosts.",
        "// Staged (later knives, SURFACE.md ledger): " + (", ".join(staged_all) if staged_all else "none - all families landed (knife 4)"),
        "const B = require('htmlelement');",
        "const HT = HTMLElement.prototype, EL = Element.prototype, DOC = Document.prototype;",
        "const FRAG = typeof DocumentFragment !== 'undefined' ? DocumentFragment.prototype : null;",
        "const SVG = typeof SVGElement !== 'undefined' ? SVGElement.prototype : null;",
        "",
    ]
    open(out_path, "w").write("\n".join(pheader + po) + "\n")
    print("wrote", os.path.relpath(out_path, ROOT), len(po), "lines")
    sys.exit(0)

# --------------------------------------------------------------------------
# strategy "patch-media": HTMLMediaElement/HTMLVideoElement/HTMLImageElement
# member family (M4 knife 5)
# --------------------------------------------------------------------------
if len(sys.argv) > 5 and sys.argv[5] == "patch-media":
    # Chrome owns the three prototypes COMPLETE (own-name sets with zero
    # engine extras - htmlmedia-truth-145.json *.order/*.kinds vs the engine
    # baseline 25/5/3 own), so unlike patch-common's seat() there is no
    # cross-prototype relocation: every missing member installs HERE, every
    # pre-owned member stays (with up to three kind fixes), and a final
    # reseat() walk re-orders each prototype into Chrome's exact own order.
    # Engine members owned one level up (src/name on Element.prototype) are
    # shadowed by the install, never relocated.
    REFLECT = {  # interface -> name -> (attr, kind, known|None, default, fallback)
        "MED": {
            "autoplay": ("autoplay", "boolean-presence", None, None, None),
            "loop": ("loop", "boolean-presence", None, None, None),
            "controls": ("controls", "boolean-presence", None, None, None),
            "defaultMuted": ("muted", "boolean-presence", None, None, None),
            "disableRemotePlayback": ("disableremoteplayback", "boolean-presence", None, None, None),
            # truth med.preload*: absent AND invalid both read 'metadata'
            "preload": ("preload", "known-fallback", ["none", "metadata", "auto"], "metadata", "metadata"),
            "crossOrigin": ("crossorigin", "crossorigin", None, None, None),
        },
        "VID": {
            "width": ("width", "unsigned-long", None, None, None),
            "height": ("height", "unsigned-long", None, None, None),
            "poster": ("poster", "url", None, None, None),
            "playsInline": ("playsinline", "boolean-presence", None, None, None),
            # media_late.js's installs (this family owns them now; the
            # satellite block was deleted - it ran after the bundle and
            # clobbered the module's on* accessors)
            "disablePictureInPicture": ("disablepictureinpicture", "boolean-presence", None, None, None),
        },
        "IMG": {
            "alt": ("alt", "string", None, None, None),
            "srcset": ("srcset", "string", None, None, None),
            "crossOrigin": ("crossorigin", "crossorigin", None, None, None),
            "useMap": ("usemap", "string", None, None, None),
            "isMap": ("ismap", "boolean-presence", None, None, None),
            "width": ("width", "unsigned-long", None, None, None),
            "height": ("height", "unsigned-long", None, None, None),
            "referrerPolicy": ("referrerpolicy", "string-known",
                               ["", "no-referrer", "no-referrer-when-downgrade", "origin",
                                "origin-when-cross-origin", "same-origin", "strict-origin",
                                "strict-origin-when-cross-origin", "unsafe-url"], "", None),
            "decoding": ("decoding", "known-fallback", ["sync", "async", "auto"], "auto", "auto"),
            "fetchPriority": ("fetchpriority", "string-known", ["auto", "high", "low"], "auto", None),
            "loading": ("loading", "known-fallback", ["eager", "lazy"], "auto", "auto"),
            "lowsrc": ("lowsrc", "string", None, None, None),
            "align": ("align", "string", None, None, None),
            "hspace": ("hspace", "unsigned-long", None, None, None),
            "vspace": ("vspace", "unsigned-long", None, None, None),
            "longDesc": ("longdesc", "string", None, None, None),
            "border": ("border", "string", None, None, None),
            "attributionSrc": ("attributionsrc", "string", None, None, None),
            "browsingTopics": ("browsingtopics", "boolean-presence", None, None, None),
            "sharedStorageWritable": ("sharedstoragewritable", "boolean-presence", None, None, None),
        },
    }
    CONST_GETTER = {  # name -> JS literal the getter returns (Chrome-true static faces)
        "MED": {"error": "null", "currentSrc": "''", "sinkId": "''",
                "webkitAudioDecodedByteCount": "0", "webkitVideoDecodedByteCount": "0"},
        "VID": {"videoWidth": "0", "videoHeight": "0",
                "webkitDecodedFrameCount": "0", "webkitDroppedFrameCount": "0"},
        "IMG": {"naturalWidth": "0", "naturalHeight": "0", "currentSrc": "''", "x": "0", "y": "0"},
    }
    IMPL_GETTER = {"MED": ["mediaKeys", "textTracks", "remote"], "VID": [], "IMG": ["complete"]}
    IMPL_ACCESSOR = {"MED": ["volume", "muted", "preservesPitch", "defaultPlaybackRate",
                             "srcObject", "controlsList"], "VID": [], "IMG": []}
    IMPL_METHOD = {"MED": {"addTextTrack": 1, "captureStream": 0, "setSinkId": 1, "setMediaKeys": 1},
                   "VID": {"getVideoPlaybackQuality": 0, "requestVideoFrameCallback": 1,
                           "cancelVideoFrameCallback": 1, "requestPictureInPicture": 0},
                   "IMG": {"decode": 0}}
    # engine owns these on the SAME prototype already: reseat only
    PURE_MOVE = {
        "MED": ["networkState", "buffered", "readyState", "seeking", "currentTime", "duration",
                "paused", "playbackRate", "played", "seekable", "ended",
                "canPlayType", "load", "pause", "play",
                "NETWORK_EMPTY", "NETWORK_IDLE", "NETWORK_LOADING", "NETWORK_NO_SOURCE",
                "HAVE_NOTHING", "HAVE_METADATA", "HAVE_CURRENT_DATA", "HAVE_FUTURE_DATA",
                "HAVE_ENOUGH_DATA"],
        "VID": [],
        "IMG": ["src"],
    }
    ON_INSTALL = {"MED": ["onencrypted", "onwaitingforkey"],
                  "VID": ["onenterpictureinpicture", "onleavepictureinpicture"], "IMG": []}
    IFACE = {"MED": "HTMLMediaElement", "VID": "HTMLVideoElement", "IMG": "HTMLImageElement"}
    # kind corrections on engine-owned descriptors (truth kinds vs engine
    # baseline): duration rw->ro; sizes ro->rw (setter writes the attribute);
    # the VID on* pair are DATA props -> installOn accessors (delete first:
    # installOn is own-check-guarded).
    RO_STRIP = {"MED": ["duration"]}
    RW_ADD_ATTR = {"IMG": {"sizes": "sizes"}}
    # src (media) / name (image): Chrome owns them on the interface, the
    # engine on Element.prototype - and its src setter is LOAD-BEARING
    # (MSE blob attach, script preparation, iframe loads; the getter already
    # resolves URLs like a browser). Install an accessor on the interface
    # that delegates BOTH legs to the Element descriptor: placement becomes
    # Chrome-true, engine semantics untouched.
    CHAIN_DELEGATE = {"MED": ["src"], "VID": [], "IMG": ["name"]}

    # ---- build-time assertions against the truth file
    for tag in ("MED", "VID", "IMG"):
        order = TRUTH[tag + ".order"]
        kinds = TRUTH[tag + ".kinds"]
        accounted = (set(REFLECT[tag]) | set(CONST_GETTER[tag]) | set(IMPL_GETTER[tag])
                     | set(IMPL_ACCESSOR[tag]) | set(IMPL_METHOD[tag]) | set(PURE_MOVE[tag])
                     | set(ON_INSTALL[tag]) | set(RO_STRIP.get(tag, []))
                     | set(RW_ADD_ATTR.get(tag, [])) | set(CHAIN_DELEGATE[tag]) | {"constructor"})
        for k in order:
            if k not in accounted:
                raise SystemExit("patch-media: %s.order member %r unaccounted" % (tag, k))
        # post-fix kind expectations: reflect/accessor/on = rw, getters = ro,
        # pure moves keep their truth kind
        for k in set(REFLECT[tag]) | set(IMPL_ACCESSOR[tag]) | set(ON_INSTALL[tag]):
            if kinds.get(k) != "rw":
                raise SystemExit("patch-media: %s.%s expected rw, truth says %r" % (tag, k, kinds.get(k)))
        for k in list(CONST_GETTER[tag]) + list(IMPL_GETTER[tag]):
            if kinds.get(k) != "ro":
                raise SystemExit("patch-media: %s.%s expected ro, truth says %r" % (tag, k, kinds.get(k)))
    for tag, methods in IMPL_METHOD.items():
        for m, ln in methods.items():
            if TRUTH["fn.length"].get(m) != ln:
                raise SystemExit("patch-media: fn.length mismatch for %s" % m)

    po = []
    PW = po.append
    PW("// ---- on* accessors, bootstrap _defineOnHandler semantics (same")
    PW("// store/inline-fallback contract as gen/htmlelement.gen.js).")
    PW("function installOn(P, name, fallback) {")
    PW("  if (Object.prototype.hasOwnProperty.call(P, name)) return;")
    PW("  const get = function () {")
    PW("    const store = this.__onHandlers;")
    PW("    if (store && Object.prototype.hasOwnProperty.call(store, name)) return store[name];")
    PW("    if (fallback && typeof this._resolveInlineHandler === 'function') return this._resolveInlineHandler(name);")
    PW("    return null;")
    PW("  };")
    PW("  const set = function (fn) {")
    PW("    if (typeof fn !== 'function') fn = null;")
    PW("    const store = this.__onHandlers || (this.__onHandlers = {});")
    PW("    store[name] = fn;")
    PW("  };")
    PW("  Object.defineProperty(get, 'name', { value: 'get ' + name, configurable: true });")
    PW("  Object.defineProperty(get, 'length', { value: 0, configurable: true });")
    PW("  Object.defineProperty(set, 'name', { value: 'set ' + name, configurable: true });")
    PW("  Object.defineProperty(set, 'length', { value: 1, configurable: true });")
    PW("  _markNative(get); _markNative(set);")
    PW("  Object.defineProperty(P, name, { configurable: true, enumerable: true, get: get, set: set });")
    PW("}")
    PW("function has(P, n) { return Object.prototype.hasOwnProperty.call(P, n); }")
    PW("// final own-order pass: walk Chrome's order, delete+redefine each")
    PW("// descriptor in place - after the walk the prototype's own order IS")
    PW("// Chrome's (idempotent: a second walk re-seats the same order).")
    PW("// Non-configurable descriptors cannot move: the nine WebIDL")
    PW("// readiness/network constants (bootstrap: {value} = non-configurable)")
    PW("// stay put and cluster at the front; the truth test normalizes them")
    PW("// out of the order comparison (a bootstrap-layer fact, SURFACE.md).")
    PW("function reseat(P, order) {")
    PW("  for (const n of order) {")
    PW("    const d = Object.getOwnPropertyDescriptor(P, n);")
    PW("    if (d && d.configurable) { delete P[n]; Object.defineProperty(P, n, d); }")
    PW("  }")
    PW("}")

    for tag in ("MED", "VID", "IMG"):
        iface = IFACE[tag]
        order = TRUTH[tag + ".order"]
        PW("")
        PW("// ---- %s.prototype (Chrome own order %s.order; installs guarded," % (iface, tag))
        PW("// kind fixes applied, reseat() walk last)")
        for k in order:
            if k == "constructor":
                continue  # reseat moves it to Chrome's position
            if k in REFLECT[tag]:
                attr, kind, known, dflt, fallback = REFLECT[tag][k]
                spec = {"attr": attr, "kind": kind}
                if known is not None:
                    spec["known"] = known
                if dflt is not None:
                    spec["default"] = dflt
                if fallback is not None:
                    spec["fallback"] = fallback
                PW("if (!has(%s, '%s')) P.reflector(%s, '%s', %s);" % (tag, k, tag, k, json.dumps(spec, sort_keys=True)))
            elif k in CONST_GETTER[tag]:
                PW("if (!has(%s, '%s')) P.getter(%s, '%s', function __f() { return %s; });" % (tag, k, tag, k, CONST_GETTER[tag][k]))
            elif k in IMPL_GETTER[tag]:
                PW("if (!has(%s, '%s')) P.getter(%s, '%s', function __f() { return B.get('%s', this); });" % (tag, k, tag, k, k))
            elif k in IMPL_ACCESSOR[tag]:
                PW("if (!has(%s, '%s')) P.accessor(%s, '%s', function __f() { return B.get('%s', this); }, function __f(v) { B.set('%s', this, v); });" % (tag, k, tag, k, k, k))
            elif k in IMPL_METHOD[tag]:
                ln = IMPL_METHOD[tag][k]
                ctx = "\"Failed to execute '%s' on '%s'\"" % (k, iface)
                PW("if (!has(%s, '%s')) P.method(%s, '%s', %d, function __f() { const ctx = %s; P.args(ctx, arguments.length, %d, __f); return B.call('%s', this, ctx, arguments); });" % (tag, k, tag, k, ln, ctx, ln, k))
            elif k in ON_INSTALL[tag]:
                # the VID pair pre-exist as DATA props: clear first
                PW("delete %s['%s']; installOn(%s, '%s', true);" % (tag, k, tag, k))
            elif k in RO_STRIP.get(tag, []):
                PW("(function () { const d = Object.getOwnPropertyDescriptor(%s, '%s'); if (d && d.get && d.set) Object.defineProperty(%s, '%s', { get: d.get, set: undefined, enumerable: true, configurable: true }); })();" % (tag, k, tag, k))
            elif k in RW_ADD_ATTR.get(tag, []):
                PW("(function () { const d = Object.getOwnPropertyDescriptor(%s, '%s'); if (d && d.get && !d.set) { const set = function (v) { this.setAttribute('%s', v === undefined || v === null ? '' : String(v)); }; Object.defineProperty(set, 'name', { value: 'set %s', configurable: true }); Object.defineProperty(set, 'length', { value: 1, configurable: true }); _markNative(set); Object.defineProperty(%s, '%s', { get: d.get, set: set, enumerable: true, configurable: true }); } })();" % (tag, k, REFLECT[tag][k][0] if k in REFLECT[tag] else k.lower(), k, tag, k))
            elif k in CHAIN_DELEGATE[tag]:
                PW("if (!has(%s, '%s')) { const _d = Object.getOwnPropertyDescriptor(Element.prototype, '%s'); if (_d && _d.get && _d.set) P.accessor(%s, '%s', function __f() { return _d.get.call(this); }, function __f(v) { _d.set.call(this, v); }); }" % (tag, k, k, tag, k))
            elif k in PURE_MOVE[tag]:
                PW("// %s: engine-owned, kept (reseat relocates)" % k)
            else:
                raise SystemExit("patch-media: unhandled %s.%s" % (tag, k))
        PW("reseat(%s, %s);" % (tag, json.dumps(order)))
    PW("")
    pheader = [
        "//! GENERATED by tools/webplatform/gen/gen_iface.py (patch-media strategy) from chrome/htmlmedia-truth-145.json (Chrome 145) - do not edit",
        "//! module: %s" % module,
        "//! layer: L2",
        "//! exposes: -",
        "//! requires-load: htmlmedia, legacy:_markNative, legacy:Element, legacy:HTMLMediaElement, legacy:HTMLImageElement, legacy:HTMLVideoElement",
        "//! requires-call: legacy:_markNative",
        "",
        "// M4 knife 5: the media/image reflected-attribute family. Chrome owns",
        "// HTMLMediaElement/HTMLVideoElement/HTMLImageElement prototypes complete",
        "// (53/16/30 own names); the engine had 25/5/3 with three kind drifts",
        "// (duration rw, sizes ro, the VID on* pair as data props). Uniform",
        "// reflection is DATA (P.reflector - this family grew the",
        "// unsigned-long/crossorigin/url kinds); non-uniform semantics delegate",
        "// to the htmlmedia impl module; a reseat() walk pins each prototype's",
        "// own order to Chrome's exact sequence.",
        "const B = require('htmlmedia');",
        "const MED = HTMLMediaElement.prototype, VID = HTMLVideoElement.prototype, IMG = HTMLImageElement.prototype;",
        "",
    ]
    open(out_path, "w").write("\n".join(pheader + po) + "\n")
    print("wrote", os.path.relpath(out_path, ROOT), len(po), "lines")
    sys.exit(0)

for name in _seen_order:
    iface = N[name]
    cons = members(iface, {"constructor"})
    # 'special' here means named-property machinery (getter/setter/deleter);
    # 'static' ops are ordinary statics, 'inherit' attrs are plain inherited
    # attributes, and stringifiers are plain methods.
    specials = [m for m in iface["members"]
                if m.get("special") and m["special"] not in ("static", "stringifier", "inherit")]
    inherits = iface.get("inheritance")
    attrs = members(iface, {"attribute"})
    ops = [m for m in members(iface, {"operation"})
           if m.get("name") or m.get("special") == "stringifier"]
    # webref marks statics as special:'static' (not a boolean key)
    statics = [m for m in ops if m.get("special") == "static"]

    W(header_gen(name))

    # ---------------------------------------------------------------- backing
    if not cons and specials:
        W("const slots%s = new WeakMap();" % name)
        W("function is%s(v) { return v !== null && (typeof v === 'object' || typeof v === 'function') && slots%s.has(v); }" % (name, name))
        exports.append("create" + name)
        W("function %s() { P.fail(%s, 'Illegal constructor', new.target ? \"Failed to construct '%s'\" : ''); }" % (name, name, name))
        W("const proto%s = P.iface(%s, '%s', 0);" % (name, name, name))
        for a in attrs:
            codes[a["name"]] = "P.getter(proto%s, '%s', function __f() { const b = slots%s.get(this); if (b === undefined) P.illegal(__f); return b.%s; });" % (name, a["name"], name, a["name"])
            expect.append({"iface": name, "kind": "getter", "name": a["name"], "length": 0})
        for o in ops:
            params = o["arguments"]
            required = sum(1 for p in params if not p.get("optional") and not p.get("variadic"))
            ctx = "\"Failed to execute '%s' on '%s'\"" % (o["name"], name)
            body = ["const b = slots%s.get(this); if (b === undefined) P.illegal(__f);" % name]
            body.append("const ctx = %s; P.args(ctx, arguments.length, %d, __f);" % (ctx, required))
            call_args = []
            for i, p in enumerate(params):
                v = "arguments[%d]" % i
                body.append("const a%d = %s;" % (i, conv_expr(p["idlType"], v, "ctx", i + 1)))
                call_args.append("a%d" % i)
            ret = o["idlType"]["idlType"]
            call = "b.%s(%s)" % (o["name"], ", ".join(call_args))
            if ret == "undefined":
                body.append(call + ";")
            else:
                body.append("return %s;" % call)
            codes[o["name"]] = "P.method(proto%s, '%s', %d, function __f() { %s });" % (name, o["name"], required, " ".join(body))
            expect.append({"iface": name, "kind": "method", "name": o["name"], "length": required})
        flush_members(name)
        # named property semantics (Chrome truth): a name found on the
        # prototype chain stays an ordinary property; otherwise the named
        # getter/setter/deleter apply.
        getter_op = next(m["name"] for m in specials if m["special"] == "getter")
        setter_op = next(m["name"] for m in specials if m["special"] == "setter")
        deleter_op = next(m["name"] for m in specials if m["special"] == "deleter")
        setter_ctx = "\"Failed to set a named property '\" + String(p) + \"' on '%s'\"" % name
        W("""function create%(n)s(backing) {
  const target = Object.create(proto%(n)s);
  const named = (p) => typeof p === 'string' && !(p in target);
  const proxy = new Proxy(target, {
    get: function __t(t, p, r) { if (named(p)) { const v = backing.%(g)s(p); if (v !== null) return v; } return Reflect.get(t, p, r); },
    set: function __t(t, p, v, r) {
      if (named(p)) { backing.%(s)s(p, conv.DOMString(v, %(sctx)s, __t)); return true; }
      return Reflect.set(t, p, v, r);
    },
    defineProperty: function __t(t, p, d) {
      if (named(p) && !('get' in d) && !('set' in d)) { backing.%(s)s(p, conv.DOMString(d.value, %(sctx)s, __t)); return true; }
      return Reflect.defineProperty(t, p, d);
    },
    deleteProperty(t, p) { if (named(p)) { backing.%(d)s(p); return true; } return Reflect.deleteProperty(t, p); },
    has(t, p) { return Reflect.has(t, p) || (typeof p === 'string' && backing.%(g)s(p) !== null); },
    getOwnPropertyDescriptor(t, p) {
      const own = Reflect.getOwnPropertyDescriptor(t, p);
      if (own) return own;
      if (typeof p === 'string') { const v = backing.%(g)s(p); if (v !== null) return { value: v, writable: true, enumerable: true, configurable: true }; }
      return undefined;
    },
    ownKeys(t) { return Array.from(new Set(backing.supportedPropertyNames().concat(Reflect.ownKeys(t)))); },
  });
  slots%(n)s.set(proxy, backing);
  return proxy;
}
P.expose('%(n)s', %(n)s);""" % {"n": name, "g": getter_op, "s": setter_op, "d": deleter_op, "sctx": setter_ctx})
        requires_load.add("legacy:_markNative")
        W("exports.is%s = is%s; exports.create%s = create%s;" % (name, name, name, name))
        expect_entry = {"name": name, "length": 0}

    # ------------------------------------------------------------------ event
    elif cons and inherits == "Event":
        W("const slots%s = new WeakMap();" % name)
        W("function is%s(v) { return v !== null && (typeof v === 'object' || typeof v === 'function') && slots%s.has(v); }" % (name, name))
        dict_name = name + "Init"
        D = N[dict_name]
        fields = sorted(D["members"], key=lambda m: m["name"])  # WebIDL: lexicographic
        c = cons[0]
        req = sum(1 for p in c["arguments"] if not p.get("optional") and not p.get("variadic"))
        W("function %s(type) {" % name)
        W("  if (!new.target) P.fail(%s, \"Please use the 'new' operator, this DOM object constructor cannot be called as a function.\", \"Failed to construct '%s'\");" % (name, name))
        W("  const ctx = \"Failed to construct '%s'\";" % name)
        W("  P.args(ctx, arguments.length, %d, %s);" % (req, name))
        W("  const t = conv.DOMString(type, ctx, %s);" % name)
        W("  const init = arguments[1];")
        W("  if (init !== undefined && init !== null && typeof init !== 'object' && typeof init !== 'function') P.fail(%s, \"The provided value is not of type '%s'.\", ctx);" % (name, dict_name))
        W("  const self = Reflect.construct(Event, [t, (init === undefined || init === null) ? undefined : init], new.target);")
        W("  const s = {};")
        W("  const DICT = '%s';" % dict_name)
        for f in fields:
            W("  { const FIELD = '%s'; const raw = (init === undefined || init === null) ? undefined : init.%s; s.%s = (raw === undefined) ? %s : %s; }" % (
                f["name"], f["name"], f["name"], js_default(f.get("default")), conv_expr(f["idlType"], "raw", "ctx", what="dict", fn=name)))
        W("  slots%s.set(self, s);" % name)
        W("  return self;")
        W("}")
        W("const proto%s = P.iface(%s, '%s', %d, Event);" % (name, name, name, req))
        for a in attrs:
            codes[a["name"]] = "P.getter(proto%s, '%s', function __f() { const s = slots%s.get(this); if (s === undefined) P.illegal(__f); return s.%s; });" % (name, a["name"], name, a["name"])
            expect.append({"iface": name, "kind": "getter", "name": a["name"], "length": 0})
        for o in ops:
            params = o["arguments"]
            required = sum(1 for p in params if not p.get("optional") and not p.get("variadic"))
            if not (o["name"].startswith("init") and [p["name"] for p in params[:3]] == ["type", "bubbles", "cancelable"]):
                raise SystemExit("gen_iface v0: unsupported operation %s.%s" % (name, o["name"]))
            ctx = "\"Failed to execute '%s' on '%s'\"" % (o["name"], name)
            body = ["const s = slots%s.get(this); if (s === undefined) P.illegal(__f);" % name,
                    "const ctx = %s; P.args(ctx, arguments.length, %d, __f);" % (ctx, required)]
            for i, p in enumerate(params):
                v = "(arguments.length > %d ? arguments[%d] : undefined)" % (i, i)
                dflt = js_default(p.get("default"))
                conv = conv_expr(p["idlType"], v, "ctx", i + 1)
                if p.get("optional"):
                    body.append("const a%d = (%s === undefined) ? %s : %s;" % (i, v, dflt, conv))
                else:
                    body.append("const a%d = %s;" % (i, conv))
            body.append("this.initEvent(a0, a1, a2);")
            for i, p in enumerate(params[3:], start=3):
                body.append("s.%s = a%d;" % (p["name"], i))
            codes[o["name"]] = "P.method(proto%s, '%s', %d, function __f() { %s });" % (name, o["name"], required, " ".join(body))
            expect.append({"iface": name, "kind": "method", "name": o["name"], "length": required})
        flush_members(name)
        W("P.expose('%s', %s);" % (name, name))
        W("exports.%s = %s;" % (name, name))
        requires_load.add("legacy:Event")
        requires_load.add("legacy:_markNative")
        requires_call.add("legacy:Event")
        expect_entry = {"name": name, "length": req}
    # ------------------------------------------------------------------ value
    elif cons and not specials and inherits != "Event":
        # geometry family: pure-value classes. SHAPE here (from IDL + the
        # Chrome truth overlay), BEHAVIOR in the paired impl module (B).
        has_value = True
        shape = TRUTH.get("shape." + name)
        if not shape:
            raise SystemExit("gen_iface value: %s needs a shape.<%s> overlay in the truth file" % (module, name))
        req = sum(1 for p in cons[0]["arguments"] if not p.get("optional") and not p.get("variadic"))
        # IDL member index (attrs by name, ops by name; '' + stringifier -> toString)
        idl_attrs = {a["name"]: a for a in attrs}
        idl_ops = {}
        for o in ops:
            idl_ops[o["name"] or "toString"] = o
        W("function %s() {" % name)
        W("  if (!new.target) P.fail(%s, \"Please use the 'new' operator, this DOM object constructor cannot be called as a function.\", \"Failed to construct '%s'\");" % (name, name))
        W("  const ctx = \"Failed to construct '%s'\";" % name)
        W("  P.args(ctx, arguments.length, %d, %s);" % (req, name))
        W("  B.ctor('%s', this, ctx, arguments);" % name)
        W("}")
        parent_expr = inherits if inherits else None
        W("const proto%s = P.iface(%s, '%s', %d%s);" % (name, name, name, req, ", " + parent_expr if parent_expr else ""))
        # members in Chrome's own-property install order, emitted directly
        # (Chrome interleaves `constructor` mid-order on some interfaces);
        # anything the IDL declares but Chrome keeps inherited is skipped;
        # a truth own member missing from the IDL is a build bug.
        for k in shape["order"]:
            if k == "constructor":
                W("P.seal(%s);" % name)
            elif k in idl_attrs:
                kind = shape["members"][k]
                if kind == "rw":
                    W("P.accessor(proto%s, '%s', function __f() { return B.get('%s', '%s', this); }, function __f(v) { B.set('%s', '%s', this, v); });" % (name, k, name, k, name, k))
                    expect.append({"iface": name, "kind": "accessor", "name": k, "length": 0})
                else:
                    W("P.getter(proto%s, '%s', function __f() { return B.get('%s', '%s', this); });" % (name, k, name, k))
                    expect.append({"iface": name, "kind": "getter", "name": k, "length": 0})
            elif k in idl_ops:
                o = idl_ops[k]
                required = sum(1 for p in o["arguments"] if not p.get("optional") and not p.get("variadic"))
                ctx = "\"Failed to execute '%s' on '%s'\"" % (k, name)
                W("P.method(proto%s, '%s', %d, function __f() { const ctx = %s; P.args(ctx, arguments.length, %d, __f); return B.call('%s', '%s', this, ctx, arguments); });" % (name, k, required, ctx, required, name, k))
                expect.append({"iface": name, "kind": "method", "name": k, "length": required})
            else:
                raise SystemExit("gen_iface value: %s.%s is in the Chrome truth order but not in the IDL" % (name, k))
        # statics: on the interface object itself, IDL declaration order
        # (Chrome own-key order for statics follows the IDL).
        for o in statics:
            on = o["name"]
            required = sum(1 for p in o["arguments"] if not p.get("optional") and not p.get("variadic"))
            ctx = "\"Failed to execute '%s' on '%s'\"" % (on, name)
            W("P.stat(%s, '%s', %d, function __f() { const ctx = %s; P.args(ctx, arguments.length, %d, __f); return B.stat('%s', '%s', ctx, arguments); });" % (name, on, required, ctx, required, name, on))
            expect.append({"iface": name, "kind": "static", "name": on, "length": required})
        W("P.expose('%s', %s);" % (name, name))
        requires_load.add(module.replace("gen/", "", 1))
        requires_load.add("legacy:_markNative")
        expect_entry = {"name": name, "length": req}
    else:
        raise SystemExit("gen_iface v0: no strategy for %s" % name)

    # statics go in their own expect list: verify() checks them against the
    # interface OBJECT, members against the prototype.
    members_json = [{k: v for k, v in e.items() if k != "iface"} for e in expect
                    if e["iface"] == name and e["kind"] != "static"]
    statics_json = [{k: v for k, v in e.items() if k != "iface"} for e in expect
                    if e["iface"] == name and e["kind"] == "static"]
    W("P.expect(%s);" % json.dumps(
        dict(expect_entry, members=members_json, statics=statics_json), sort_keys=True))
    W("")

if has_value:
    # all interfaces of this module are exposed now — give the impl module
    # its post-expose hook (geometry wires WebKitCSSMatrix onto the freshly
    # exposed DOMMatrix).
    out.append("if (B.ready) B.ready();")
    out.append("")

header = [
    "//! GENERATED by tools/webplatform/gen/gen_iface.py from %s (webref %s) - do not edit" % (rel_spec, spec["webref"][:12]),
    "//! module: %s" % module,
    "//! layer: L2",
    "//! exposes: " + ", ".join(n for n, i in N.items() if i["type"] == "interface"),
    "//! requires-load: " + ", ".join(sorted(requires_load)),
    "//! requires-call: " + (", ".join(sorted(requires_call)) or "-"),
    "",
    "// Backing protocol for strategy 'backing' (see gen_iface.py docstring): the object",
    "// passed to createStorage() implements length, key(i), getItem(k), setItem(k,v),",
    "// removeItem(k), clear() and supportedPropertyNames().",
    "const conv = P.conv;",
    "",
]
if has_value:
    # pull the behavior table at load (bundle order guarantees the impl
    # module ran first - see requires-load).
    header.append("const B = require('%s');" % module.replace("gen/", "", 1))
    header.append("")
open(out_path, "w").write("\n".join(header + out))
print("wrote", os.path.relpath(out_path, ROOT), len(out), "lines")
