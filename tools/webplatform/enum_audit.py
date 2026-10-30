#!/usr/bin/env python3
"""Own-enumerable-key audit: python3 tools/webplatform/enum_audit.py <engine.json> <chrome.json>
Capture both with probe_run.py (chrome / engine) of chrome/enum-probe.js. Lists, per object, the own enumerable keys (for-in +
hasOwnProperty = what JSON.stringify / Object.assign / spread see) the engine has that Chrome does not, and vice versa."""
import json, sys
E = json.load(open(sys.argv[1])); C = json.load(open(sys.argv[2]))
rows = []
for n, c in C.items():
    e = E.get(n)
    if not isinstance(c, list) or not isinstance(e, list):
        if c != e: rows.append((999, n, ['<%s vs %s>' % (str(e)[:30], str(c)[:30])], []))
        continue
    extra = [k for k in e if k not in c]; miss = [k for k in c if k not in e]
    if extra or miss: rows.append((len(extra) + len(miss), n, extra, miss))
rows.sort(reverse=True)
print('objects', len(C), 'clean', len(C) - len(rows), 'leaky', len(rows))
for k, n, ex, ms in rows: print('%3d %-14s extra=%s missing=%s' % (k, n, ex[:12], ms[:6]))
