#!/usr/bin/env python3
"""Fingerprint -> reflector spec (M4 knife 6).

Input: chrome/htmlreflect-truth-145.json, produced by htmlreflect-probe.js in
real Chrome 145. For one read-write member the probe recorded a behavior
fingerprint: the default read, the read-back + created attribute after each of
a fixed list of property sets, the read-back after raw setAttribute of each of
a fixed list of attribute values, and (for normalizing members) a keyword
vocabulary sweep.

classify(fp) tries each candidate reflector kind IN ORDER and accepts the
first whose PYTHON MIRROR of core.js's reflector rules reproduces EVERY facet
of the fingerprint. A member no kind can reproduce is returned as None - it
needs hand semantics (impl module) or stays staged, never a guessed kind. That
makes the DATA table a measurement, not a memory, and guards the mirror and
core.js from drifting apart (the gen run fails loudly instead of emitting a
wrong table).

Run directly for a classification report:
  python3 tools/webplatform/gen/reflect_classify.py [iface-substring ...]
"""
import json, math, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
TRUTH_PATH = os.path.join(HERE, "..", "chrome", "htmlreflect-truth-145.json")

UNDEF = object()
SET_VALUES = {
    "str": "ABC", "empty": "", "num5": 5, "neg1": -1, "true": True, "false": False,
    "rel": "/rel/p?q=1", "big": 2147483648, "num1.9": 1.9, "null": None, "undef": UNDEF,
}
ATTR_BASE = "#B"   # the probe normalizes the page URL to '#B' and its origin to '#O'


# ---- JS value semantics (just the slice the probe values exercise)
def js_string(v):
    if v is UNDEF: return "undefined"
    if v is None: return "null"
    if v is True: return "true"
    if v is False: return "false"
    if isinstance(v, float):
        return str(int(v)) if v == int(v) else repr(v)
    if isinstance(v, int): return str(v)
    return v


def js_number(v):
    if v is UNDEF: return math.nan
    if v is None or v is False: return 0
    if v is True: return 1
    if isinstance(v, (int, float)): return v
    t = v.strip()
    if t == "": return 0
    try: return float(t)
    except ValueError: return math.nan


def to_uint32(n):
    if not math.isfinite(n): return 0
    return int(math.trunc(n)) % 4294967296


def to_int32(n):
    t = to_uint32(n)
    return t - 4294967296 if t > 2147483647 else t


def truthy(v):
    if v is UNDEF or v is None or v is False: return False
    if isinstance(v, (int, float)): return v != 0 and not (isinstance(v, float) and math.isnan(v))
    return v != ""


_WS_INT = re.compile(r"^[\t\n\f\r ]*([+-]?)([0-9]+)")


def parse_int_attr(v):
    m = _WS_INT.match(v)
    if not m: return None
    n = int(m.group(2))
    return -n if m.group(1) == "-" else n


def ascii_lower(v):
    return "".join(c.lower() if "A" <= c <= "Z" else c for c in v)


def resolve_url(v):
    if re.match(r"^[a-zA-Z][a-zA-Z0-9+.\-]*:", v): out = v
    elif v.startswith("/"): out = "#O" + v
    elif v == "": out = ATTR_BASE
    else: out = "#O/" + v
    return out.replace(" ", "%20")


# ---- mirror of core.js reflector: get(attr value or None) / set(v) -> attr value or REMOVE
REMOVE = object()


def mirror_get(spec, v):
    kind = spec["kind"]
    d = spec.get("default", "")
    if kind in ("string", "string-nte"): return d if v is None else v
    if kind == "boolean-presence": return v is not None
    if kind == "boolean-truefalse": return v == "true"
    if kind == "enum":
        if v is None: return spec["missing"]
        low = ascii_lower(v)
        if low == "" and "empty" in spec: return spec["empty"]
        return spec["map"].get(low, spec["invalid"])
    if kind in ("unsigned-long", "unsigned-long-dim"):
        if v is None: return 0
        n = parse_int_attr(v)
        mx = 2147483647 if kind == "unsigned-long" else 4294967295
        return 0 if n is None or n < 0 or n > mx else n
    if kind == "long":
        if v is None: return 0
        n = parse_int_attr(v)
        return 0 if n is None or n < -2147483648 or n > 2147483647 else n
    if kind == "crossorigin":
        if v is None: return None
        return "use-credentials" if ascii_lower(v) == "use-credentials" else "anonymous"
    if kind == "url":
        return d if v is None else resolve_url(v)
    raise SystemExit("mirror_get: unknown kind %r" % kind)


def mirror_set(spec, v):
    kind = spec["kind"]
    if kind == "boolean-presence": return "" if truthy(v) else REMOVE
    if kind == "boolean-truefalse": return "true" if truthy(v) else "false"
    if kind in ("unsigned-long", "unsigned-long-dim"):
        u = to_uint32(js_number(v))
        return str(0 if u > 2147483647 else u)
    if kind == "long": return str(to_int32(js_number(v)))
    if kind == "crossorigin":
        return REMOVE if v is None or v is UNDEF else js_string(v)
    if kind == "string-nte": return "" if v is None else js_string(v)
    if spec.get("nullRemoves") and (v is None or v is UNDEF): return REMOVE
    return js_string(v)


def predict_matches(spec, fp):
    """True iff the mirror reproduces every facet of the fingerprint."""
    attr = fp["attr"]
    if mirror_get(spec, None) != fp["def"]: return False
    for key, val in SET_VALUES.items():
        want = fp["set"].get(key)
        if want is None or "throw" in want: return False
        w = mirror_set(spec, val)
        written = None if w is REMOVE else [attr, w]
        if want["attr"] != written: return False
        if want["read"] != mirror_get(spec, None if w is REMOVE else w): return False
    for av, want in fp["fromAttr"].items():
        if mirror_get(spec, av) != want: return False
    for v, want in fp.get("vocab", {}).items():
        if mirror_get(spec, v) != want: return False
    return True


def derive_enum(fp):
    vocab = fp.get("vocab")
    if not vocab: return None
    invalid = fp["fromAttr"]["ABC"]
    spec = {"attr": fp["attr"], "kind": "enum", "missing": fp["def"], "invalid": invalid, "map": {}}
    for k, r in vocab.items():
        if k == "" or k != k.lower(): continue
        if r != invalid: spec["map"][k] = r
    if vocab.get("") != invalid: spec["empty"] = vocab[""]
    return spec


def classify(fp, kind="rw"):
    """-> (spec, None) or (None, reason). spec is the P.reflector DATA."""
    if kind != "rw": return None, "not-rw"
    if "attr" not in fp: return None, "stateful"
    attr = fp["attr"]
    if fp["def"] == "OBJ:DOMTokenList":
        return {"attr": attr, "kind": "tokenlist"}, None
    cands = []
    for k in ("boolean-presence", "boolean-truefalse", "url", "string", "string-nte",
              "unsigned-long", "unsigned-long-dim", "long", "crossorigin"):
        sp = {"attr": attr, "kind": k}
        if k == "crossorigin": sp["default"] = None
        cands.append(sp)
    e = derive_enum(fp)
    if e:
        cands.append(e)
        cands.append(dict(e, nullRemoves=True))
    for sp in cands:
        if predict_matches(sp, fp): return sp, None
    return None, "no-kind-reproduces"


def load_truth():
    return json.load(open(TRUTH_PATH))


if __name__ == "__main__":
    T = load_truth()
    want = sys.argv[1:]
    tally = {}
    for iface, mem in T["fp"].items():
        if want and not any(w.lower() in iface.lower() for w in want): continue
        for m, fp in mem.items():
            spec, why = classify(fp, T["kinds"][iface][m])
            key = spec["kind"] if spec else "UNCLASSIFIED(%s)" % why
            tally.setdefault(key, []).append("%s.%s" % (iface[4:-7] or "El", m))
    for k, v in sorted(tally.items(), key=lambda kv: -len(kv[1])):
        print("%-34s %3d" % (k, len(v)), " ".join(v[:14]) + (" ..." if len(v) > 14 else ""))
