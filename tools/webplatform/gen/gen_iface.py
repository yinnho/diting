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
