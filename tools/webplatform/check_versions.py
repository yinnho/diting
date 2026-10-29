#!/usr/bin/env python3
"""Offline gate: every full Chrome version the engine can claim must be a version Google
actually published (Chrome for Testing known-good list, vendored as chrome/cft-builds.json).
A claimed version that never shipped (e.g. 145.0.6509.41 - build 6509 is Chrome 127's) is a
fingerprint tell a detector can check against the public version database.

Checks: crates/diting/js/bootstrap.js  __DITING_FULL_BUILD  { "<major>": "<build.patch>" }
Refresh the vendored list with tools/webplatform/capture/refresh_cft_builds.py (network).
"""
import json, os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
real = json.load(open(os.path.join(ROOT, "tools/webplatform/chrome/cft-builds.json")))["builds"]
src = open(os.path.join(ROOT, "crates/diting/js/bootstrap.js"), encoding="utf8").read()
m = re.search(r"const __DITING_FULL_BUILD = \{(.*?)\};", src, re.S)
if not m:
    sys.exit("check_versions: __DITING_FULL_BUILD not found in bootstrap.js")
table = dict(re.findall(r'"(\d+)":\s*"([\d.]+)"', m.group(1)))
bad = [(k, v) for k, v in table.items() if v not in real.get(k, [])]
for k, v in bad:
    print("FAIL: Chrome %s.0.%s was never published (not in Chrome for Testing known-good list)" % (k, v), file=sys.stderr)
if not table:
    sys.exit("check_versions: empty table")
if bad:
    sys.exit(1)
print("claimed Chrome versions are all real (%d majors checked)" % len(table))
