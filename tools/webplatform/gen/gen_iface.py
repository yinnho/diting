#!/usr/bin/env python3
"""WebIDL -> JS interface skeleton (JS-PLATFORM-ARCH sec 3.2, M2 v0).

usage: gen_iface.py <spec.json> <out.js> <module-name>

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

v0 covers what Storage/StorageEvent need; new strategies are added per family.
"""
import json, os, sys

spec_path, out_path, module = sys.argv[1], sys.argv[2], sys.argv[3]
spec = json.load(open(spec_path))
# Chrome overlay (optional): own-property order of each prototype, from
# chrome/chrome-storage-truth.json `order.<Interface>` (JS-PLATFORM-ARCH sec 3.1).
_truth_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "chrome", "chrome-storage-truth.json")
TRUTH = json.load(open(_truth_path))["result"] if os.path.exists(_truth_path) else {}
N = spec["names"]
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
rel_spec = os.path.relpath(spec_path, ROOT)


def tname(t):
    return t["idlType"] if isinstance(t["idlType"], str) else None


def is_nullable(t):
    return bool(t.get("nullable"))


def conv_expr(t, v, ctx_js, argno=None, what="arg"):
    """JS expression converting value expression `v` per IDL type `t`.
    ctx_js is a JS expression for the 'Failed to ...' prefix."""
    n = tname(t)
    nullable = is_nullable(t)
    if n in ("DOMString", "USVString"):
        c = "conv.%s(%s, %s)" % (n, v, ctx_js)
    elif n == "unsigned long":
        c = "conv.unsignedLong(%s, %s)" % (v, ctx_js)
    elif n == "boolean":
        c = "conv.boolean(%s)" % v
    elif n in N and N[n]["type"] == "interface":
        # interface type: brand check against that interface's slots
        if what == "dict":
            msg = "%s + \": Failed to read the '\" + FIELD + \"' property from '\" + DICT + \"': Failed to convert value to '%s'.\"" % (ctx_js, n)
        else:
            msg = "%s + \": parameter %d is not of type '%s'.\"" % (ctx_js, argno, n)
        c = "(is%s(%s) ? %s : (function () { throw new TypeError(%s); })())" % (n, v, v, msg)
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


for name, iface in N.items():
    if iface["type"] != "interface":
        continue
    cons = members(iface, {"constructor"})
    specials = [m for m in iface["members"] if m.get("special")]
    inherits = iface.get("inheritance")
    attrs = members(iface, {"attribute"})
    ops = [m for m in members(iface, {"operation"}) if m.get("name")]

    W(header_gen(name))
    W("const slots%s = new WeakMap();" % name)
    W("function is%s(v) { return v !== null && (typeof v === 'object' || typeof v === 'function') && slots%s.has(v); }" % (name, name))

    # ---------------------------------------------------------------- backing
    if not cons and specials:
        exports.append("create" + name)
        W("function %s() { throw new TypeError(new.target ? \"Failed to construct '%s': Illegal constructor\" : 'Illegal constructor'); }" % (name, name))
        W("const proto%s = P.iface(%s, '%s', 0);" % (name, name, name))
        for a in attrs:
            codes[a["name"]] = "P.getter(proto%s, '%s', function () { const b = slots%s.get(this); if (b === undefined) P.illegal(); return b.%s; });" % (name, a["name"], name, a["name"])
            expect.append({"iface": name, "kind": "getter", "name": a["name"], "length": 0})
        for o in ops:
            params = o["arguments"]
            required = sum(1 for p in params if not p.get("optional") and not p.get("variadic"))
            ctx = "\"Failed to execute '%s' on '%s'\"" % (o["name"], name)
            body = ["const b = slots%s.get(this); if (b === undefined) P.illegal();" % name]
            body.append("const ctx = %s; P.args(ctx, arguments.length, %d);" % (ctx, required))
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
            codes[o["name"]] = "P.method(proto%s, '%s', %d, function %s() { %s });" % (name, o["name"], required, o["name"], " ".join(body))
            expect.append({"iface": name, "kind": "method", "name": o["name"], "length": required})
        flush_members(name)
        # named property semantics (Chrome truth): a name found on the
        # prototype chain stays an ordinary property; otherwise the named
        # getter/setter/deleter apply.
        getter_op = next(m["name"] for m in specials if m["special"] == "getter")
        setter_op = next(m["name"] for m in specials if m["special"] == "setter")
        deleter_op = next(m["name"] for m in specials if m["special"] == "deleter")
        setter_ctx = "\"Failed to set the '\" + String(p) + \"' property on '%s'\"" % name
        W("""function create%(n)s(backing) {
  const target = Object.create(proto%(n)s);
  const named = (p) => typeof p === 'string' && !(p in target);
  const proxy = new Proxy(target, {
    get(t, p, r) { if (named(p)) { const v = backing.%(g)s(p); if (v !== null) return v; } return Reflect.get(t, p, r); },
    set(t, p, v, r) {
      if (named(p)) { backing.%(s)s(p, conv.DOMString(v, %(sctx)s)); return true; }
      return Reflect.set(t, p, v, r);
    },
    defineProperty(t, p, d) {
      if (named(p) && !('get' in d) && !('set' in d)) { backing.%(s)s(p, conv.DOMString(d.value, %(sctx)s)); return true; }
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
        dict_name = name + "Init"
        D = N[dict_name]
        fields = sorted(D["members"], key=lambda m: m["name"])  # WebIDL: lexicographic
        c = cons[0]
        req = sum(1 for p in c["arguments"] if not p.get("optional") and not p.get("variadic"))
        W("function %s(type) {" % name)
        W("  if (!new.target) throw new TypeError(\"Failed to construct '%s': Please use the 'new' operator, this DOM object constructor cannot be called as a function.\");" % name)
        W("  const ctx = \"Failed to construct '%s'\";" % name)
        W("  P.args(ctx, arguments.length, %d);" % req)
        W("  const t = conv.DOMString(type, ctx);")
        W("  const init = arguments[1];")
        W("  if (init !== undefined && init !== null && typeof init !== 'object' && typeof init !== 'function') throw new TypeError(ctx + \": The provided value is not of type '%s'.\");" % dict_name)
        W("  const self = Reflect.construct(Event, [t, (init === undefined || init === null) ? undefined : init], new.target);")
        W("  const s = {};")
        W("  const DICT = '%s';" % dict_name)
        for f in fields:
            W("  { const FIELD = '%s'; const raw = (init === undefined || init === null) ? undefined : init.%s; s.%s = (raw === undefined) ? %s : %s; }" % (
                f["name"], f["name"], f["name"], js_default(f.get("default")), conv_expr(f["idlType"], "raw", "ctx", what="dict")))
        W("  slots%s.set(self, s);" % name)
        W("  return self;")
        W("}")
        W("const proto%s = P.iface(%s, '%s', %d, Event);" % (name, name, name, req))
        for a in attrs:
            codes[a["name"]] = "P.getter(proto%s, '%s', function () { const s = slots%s.get(this); if (s === undefined) P.illegal(); return s.%s; });" % (name, a["name"], name, a["name"])
            expect.append({"iface": name, "kind": "getter", "name": a["name"], "length": 0})
        for o in ops:
            params = o["arguments"]
            required = sum(1 for p in params if not p.get("optional") and not p.get("variadic"))
            if not (o["name"].startswith("init") and [p["name"] for p in params[:3]] == ["type", "bubbles", "cancelable"]):
                raise SystemExit("gen_iface v0: unsupported operation %s.%s" % (name, o["name"]))
            ctx = "\"Failed to execute '%s' on '%s'\"" % (o["name"], name)
            body = ["const s = slots%s.get(this); if (s === undefined) P.illegal();" % name,
                    "const ctx = %s; P.args(ctx, arguments.length, %d);" % (ctx, required)]
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
            codes[o["name"]] = "P.method(proto%s, '%s', %d, function %s() { %s });" % (name, o["name"], required, o["name"], " ".join(body))
            expect.append({"iface": name, "kind": "method", "name": o["name"], "length": required})
        flush_members(name)
        W("P.expose('%s', %s);" % (name, name))
        W("exports.%s = %s;" % (name, name))
        requires_load.add("legacy:Event")
        requires_load.add("legacy:_markNative")
        requires_call.add("legacy:Event")
        expect_entry = {"name": name, "length": req}
    else:
        raise SystemExit("gen_iface v0: no strategy for %s" % name)

    members_json = [{k: v for k, v in e.items() if k != "iface"} for e in expect if e["iface"] == name]
    W("P.expect(%s);" % json.dumps(dict(expect_entry, members=members_json), sort_keys=True))
    W("")

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
open(out_path, "w").write("\n".join(header + out))
print("wrote", os.path.relpath(out_path, ROOT), len(out), "lines")
