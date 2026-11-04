#!/usr/bin/env python3
"""Apply crates/diting/chrome-target.toml to the places that cannot read it at build time.

The Rust side derives everything from the toml via build.rs. The legacy JS sources baked into
the V8 snapshot (bootstrap.js + the god-file split files) cannot, so three derived points are
rewritten here - deterministically, never by hand:
  1. the fallback UA literal in `get userAgent()` (used when no UA was injected, e.g. bare test runtimes;
     lives in navigator_persona.js since the 刀13 split)
  2. the fallback major in __ditingChromeMajorFromUA()
  3. the __DITING_FULL_BUILD entry for the target major
Each anchor is scanned across ALL js/ files (a knife may move its host file); totals must stay 1/1/1.
`tools/check_coherence.py` fails CI if any of them disagrees with the toml.
  tools/apply_chrome_target.py          rewrite
  tools/apply_chrome_target.py --check  exit 1 if a rewrite would change anything
"""
import os, re, sys, tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
T = tomllib.load(open(os.path.join(ROOT, "crates/diting/chrome-target.toml"), "rb"))
major, full = int(T["major"]), T["full_version"]
build = ".".join(full.split(".")[2:])
jsdir = os.path.join(ROOT, "crates/diting/js")
files = ["bootstrap.js"] + sorted(f for f in os.listdir(jsdir) if f.endswith(".js") and f != "bootstrap.js")

UA_PAT = r'(get userAgent\(\) \{ return globalThis\.__diting_ua \|\| "Mozilla/5\.0 \(Macintosh; Intel Mac OS X 10_15_7\) AppleWebKit/537\.36 \(KHTML, like Gecko\) Chrome/)\d+(\.0\.0\.0 Safari/537\.36")'
MAJ_PAT = r'(return m \? m\[1\] : ")\d+(";)'
BLD_PAT = r'("%d": ")[\d.]+(")' % major

totals = [0, 0, 0]
changed = []
for name in files:
    path = os.path.join(jsdir, name)
    src = open(path, encoding="utf8").read()
    out = src
    out, n1 = re.subn(UA_PAT, r"\g<1>%d\g<2>" % major, out)
    out, n2 = re.subn(MAJ_PAT, r"\g<1>%d\g<2>" % major, out)
    out, n3 = re.subn(BLD_PAT, r"\g<1>%s\g<2>" % build, out, count=1)
    for i, n in enumerate((n1, n2, n3)):
        totals[i] += n
    if out != src:
        changed.append(name)
        if "--check" not in sys.argv:
            open(path, "w", encoding="utf8").write(out)

if tuple(totals) != (1, 1, 1):
    sys.exit("apply_chrome_target: expected 1/1/1 rewrite points, matched %d/%d/%d across js/ (host file moved?)" % tuple(totals))
if "--check" in sys.argv:
    sys.exit(0 if not changed else "%s not in sync with chrome-target.toml - run tools/apply_chrome_target.py" % ", ".join(changed))
if changed:
    print("updated for Chrome %s: %s" % (full, ", ".join(changed)))
else:
    print("already in sync with Chrome %s" % full)
