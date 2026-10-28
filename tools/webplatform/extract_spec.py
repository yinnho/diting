#!/usr/bin/env python3
"""Extract per-family WebIDL slices from a PINNED webref commit into spec/.

usage: extract_spec.py <webref-sha> <out.json> <Name> [<Name> ...]
  <Name> = interface / dictionary / mixin names to pull (partials and `includes`
  statements targeting them are pulled too). The tarball is cached in
  $WEBPLATFORM_CACHE (default /tmp/webplatform-cache); the output records the
  webref sha so the slice is reproducible.
"""
import glob, io, json, os, sys, tarfile, urllib.request

sha, out, names = sys.argv[1], sys.argv[2], set(sys.argv[3:])
cache = os.environ.get("WEBPLATFORM_CACHE", "/tmp/webplatform-cache")
os.makedirs(cache, exist_ok=True)
tgz = os.path.join(cache, f"webref-{sha}.tgz")
if not os.path.exists(tgz):
    urllib.request.urlretrieve(f"https://codeload.github.com/w3c/webref/tar.gz/{sha}", tgz)

slice_ = {"webref": sha, "names": {}, "partials": {}, "includes": [], "specs": {}}
with tarfile.open(tgz) as t:
    for m in t.getmembers():
        if "/ed/idlparsed/" not in m.name or not m.name.endswith(".json"):
            continue
        d = json.load(t.extractfile(m))
        spec = d["spec"]["shortname"] if "shortname" in d["spec"] else d["spec"].get("title", "")
        p = d["idlparsed"]
        for n in names:
            if n in p["idlNames"]:
                slice_["names"][n] = p["idlNames"][n]
                slice_["specs"][n] = d["spec"].get("url", spec)
            for part in p["idlExtendedNames"].get(n, []):
                if part["type"] in ("interface", "interface mixin", "dictionary") and part.get("partial"):
                    slice_["partials"].setdefault(n, []).append(part)
                if part["type"] == "includes":
                    slice_["includes"].append(part)
missing = names - set(slice_["names"])
if missing:
    sys.exit(f"not found in webref {sha}: {sorted(missing)}")
# mixin bodies referenced by `includes` for the requested names
open(out, "w").write(json.dumps(slice_, indent=1, ensure_ascii=False, sort_keys=True) + "\n")
print("wrote", out, sorted(slice_["names"]))
