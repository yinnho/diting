#!/usr/bin/env python3
"""Apply crates/diting/chrome-target.toml to the places that cannot read it at build time.

The Rust side derives everything from the toml via build.rs. The legacy bootstrap.js (baked into
the V8 snapshot) cannot, so three derived points are rewritten here - deterministically, never by hand:
  1. the fallback UA literal in `get userAgent()` (used when no UA was injected, e.g. bare test runtimes)
  2. the fallback major in __ditingChromeMajorFromUA()
  3. the __DITING_FULL_BUILD entry for the target major
`tools/check_coherence.py` fails CI if any of them disagrees with the toml.
  tools/apply_chrome_target.py          rewrite
  tools/apply_chrome_target.py --check  exit 1 if a rewrite would change anything
"""
import os, re, sys, tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
T = tomllib.load(open(os.path.join(ROOT, "crates/diting/chrome-target.toml"), "rb"))
major, full = int(T["major"]), T["full_version"]
build = ".".join(full.split(".")[2:])
path = os.path.join(ROOT, "crates/diting/js/bootstrap.js")
src = open(path, encoding="utf8").read()
out = src

out, n1 = re.subn(r'(get userAgent\(\) \{ return globalThis\.__diting_ua \|\| "Mozilla/5\.0 \(Macintosh; Intel Mac OS X 10_15_7\) AppleWebKit/537\.36 \(KHTML, like Gecko\) Chrome/)\d+(\.0\.0\.0 Safari/537\.36")',
                  r"\g<1>%d\g<2>" % major, out)
out, n2 = re.subn(r'(return m \? m\[1\] : ")\d+(";)', r"\g<1>%d\g<2>" % major, out)
out, n3 = re.subn(r'("%d": ")[\d.]+(")' % major, r"\g<1>%s\g<2>" % build, out, count=1)
if (n1, n2, n3) != (1, 1, 1):
    sys.exit("apply_chrome_target: expected 1/1/1 rewrite points, matched %d/%d/%d (bootstrap.js changed shape?)" % (n1, n2, n3))
if "--check" in sys.argv:
    sys.exit(0 if out == src else "bootstrap.js is not in sync with chrome-target.toml - run tools/apply_chrome_target.py")
if out != src:
    open(path, "w", encoding="utf8").write(out)
    print("bootstrap.js updated for Chrome %s" % full)
else:
    print("bootstrap.js already in sync with Chrome %s" % full)
