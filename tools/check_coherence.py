#!/usr/bin/env python3
"""Chrome-version coherence gate (owner ruling 2026-10-29: every layer is the SAME real Chrome).

Single source: crates/diting/chrome-target.toml. This gate (offline, no build needed) checks:
  1. full_version is a version Google really published (vendored Chrome for Testing list)
  2. tls_profile == chrome<major>   (build.rs additionally fails the BUILD if wreq-util lacks it)
  3. the JS full-version table (__DITING_FULL_BUILD) agrees with full_version
  4. the target's truth bundle is checked in (tools/webplatform/chrome/wire/chrome-<full>.json)
  5. no Chrome-version literal in product code disagrees with the target
     (UA `Chrome/<n>`, Client Hints `"Chromium";v="<n>"`); put `coherence-allow: <why>` on a line
     to exempt it. Tests (tests.rs, tests/, anything after `#[cfg(test)]`) and comments are skipped.
Informational: how far the target lags Chrome stable.
"""
import json, os, re, subprocess, sys, tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
T = tomllib.load(open(os.path.join(ROOT, "crates/diting/chrome-target.toml"), "rb"))
major, full = int(T["major"]), T["full_version"]
cft = json.load(open(os.path.join(ROOT, "tools/webplatform/chrome/cft-builds.json")))
errs = []

# 1. real version
build = ".".join(full.split(".")[2:])
if not full.startswith("%d." % major):
    errs.append("full_version %s is not Chrome %d" % (full, major))
if build not in cft["builds"].get(str(major), []):
    errs.append("full_version %s was never published by Google (not in Chrome for Testing list)" % full)
# 2. tls profile
if T["tls_profile"] != "chrome%d" % major:
    errs.append("tls_profile %r != chrome%d" % (T["tls_profile"], major))
# 3. JS table
js = open(os.path.join(ROOT, "crates/diting/js/bootstrap.js"), encoding="utf8").read()
m = re.search(r"const __DITING_FULL_BUILD = \{(.*?)\};", js, re.S)
table = dict(re.findall(r'"(\d+)":\s*"([\d.]+)"', m.group(1))) if m else {}
if table.get(str(major)) != build:
    errs.append("JS __DITING_FULL_BUILD[%d] = %r but chrome-target.toml says build %r" % (major, table.get(str(major)), build))
# 3b. the JS fallbacks tools/apply_chrome_target.py maintains
fm = re.search(r'return m \? m\[1\] : "(\d+)";', js)
if not fm or int(fm.group(1)) != major:
    errs.append("JS __ditingChromeMajorFromUA fallback %r != target %d (run tools/apply_chrome_target.py)" % (fm.group(1) if fm else None, major))
# 4. truth bundle
bundle = "tools/webplatform/chrome/wire/chrome-%s.json" % full
if not os.path.exists(os.path.join(ROOT, bundle)):
    errs.append("no truth bundle for the target: capture it with tools/webplatform/capture/wire_capture.py -> %s" % bundle)

# 5. literals
PAT = [re.compile(r"Chrome/(\d{2,3})\b"), re.compile(r'"Chromium\\?";\s*v=\\?"(\d{2,3})')]
files = subprocess.run(["git", "ls-files", "crates/diting/src", "crates/diting/js", "src"], cwd=ROOT,
                       capture_output=True, text=True).stdout.split()
for f in files:
    if not f.endswith((".rs", ".js")):
        continue
    base = os.path.basename(f)
    if base in ("tests.rs", "surface_snapshot.txt", "platform_bundle.js") or "/tests/" in f:
        continue
    lines = open(os.path.join(ROOT, f), encoding="utf8", errors="replace").read().split("\n")
    for i, line in enumerate(lines, 1):
        if f.endswith(".rs") and line.strip().startswith("#[cfg(test)]"):
            break                      # tests live at the bottom of the file by convention
        st = line.strip()
        if st.startswith(("//", "*", "/*")) or "coherence-allow:" in line:
            continue
        for p in PAT:
            for v in p.findall(line):
                if int(v) != major:
                    errs.append("%s:%d claims Chrome %s but the target is %d: %s" % (f, i, v, major, st[:90]))

stable = int(cft["_meta"].get("stable", "0").split(".")[0] or 0)
if errs:
    print("coherence FAIL (%d):" % len(errs), file=sys.stderr)
    for e in errs:
        print("  - " + e, file=sys.stderr)
    sys.exit(1)
print("coherence ok: target Chrome %s (TLS %s, %s %s) | lag to stable %s: %d major(s) [informational]" % (
    full, T["tls_profile"], T["platform"], T["arch"], cft["_meta"].get("stable", "?"), max(0, stable - major)))
