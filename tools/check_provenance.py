#!/usr/bin/env python3
"""Clean-room ratchet (owner ruling 2026-10-29: "learn from competitors, never fork them").

Counts `upstream <commit>` / `upstream #<n>` markers in the sources. Those markers mean
"this change tracks a commit of another project". New code must not add any: the count
may only go DOWN as adapted code is replaced. Baseline: tools/provenance-baseline.json.
  tools/check_provenance.py            verify (CI)
  tools/check_provenance.py --update   lower the baseline after code was replaced
References like "peer issue #738" in comments are fine - a competitor's public bug report
is a signal to write OUR OWN probe against real Chrome, not code to carry over.
"""
import json, os, re, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "provenance-baseline.json")
pat = re.compile(r"upstream (?:#[0-9]+|[0-9a-f]{7,})")

files = subprocess.run(["git", "ls-files", "crates", "src"], cwd=ROOT, capture_output=True, text=True).stdout.split()
n, per = 0, {}
for f in files:
    if not f.endswith((".js", ".rs", ".py", ".md", ".toml")):
        continue
    try:
        c = len(pat.findall(open(os.path.join(ROOT, f), encoding="utf8", errors="replace").read()))
    except OSError:
        continue
    if c:
        per[f] = c
        n += c

if "--update" in sys.argv:
    json.dump({"upstream_markers": n, "files": len(per),
               "note": "may only go down; see tools/check_provenance.py"}, open(BASE, "w"), indent=1)
    print("baseline ->", n, "markers in", len(per), "files")
    sys.exit(0)

base = json.load(open(BASE))["upstream_markers"]
if n > base:
    print("FAIL: %d 'upstream <ref>' markers > baseline %d. Clean-room rule: do not track other projects' commits;"
          " write your own probe against real Chrome and implement it yourself." % (n, base), file=sys.stderr)
    for f, c in sorted(per.items(), key=lambda x: -x[1])[:10]:
        print("   %3d %s" % (c, f), file=sys.stderr)
    sys.exit(1)
print("provenance ratchet ok: %d markers (baseline %d), %d files" % (n, base, len(per)))
