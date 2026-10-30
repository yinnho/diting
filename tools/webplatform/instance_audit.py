#!/usr/bin/env python3
"""Instance own-key audit: python3 tools/webplatform/instance_audit.py <engine.json> [chrome.json]
Capture both sides with probe_run.py (chrome / engine) of chrome/instance-audit-probe.js. Prints, per object kind, the own property
names the engine has that Chrome's instance does not (the fingerprint-visible leak), sorted by count."""
import json, sys
E = json.load(open(sys.argv[1])); C = json.load(open(sys.argv[2] if len(sys.argv) > 2 else sys.argv[1].replace('-e', '-c')))
rows = []
for n, c in C.items():
    e = E.get(n)
    if not isinstance(c, dict) or not isinstance(e, dict): continue
    extra = [k for k in e['names'] if k not in c['names']]
    miss = [k for k in c['names'] if k not in e['names']]
    sx = [k for k in e['syms'] if k not in c['syms']]
    if extra or miss or sx: rows.append((len(extra) + len(miss) + len(sx), n, extra, miss, sx))
rows.sort(reverse=True)
print('objects', len(C), 'clean', len(C) - len(rows), 'leaky', len(rows))
for k, n, ex, ms, sx in rows: print('%3d %-28s extra=%s missing=%s syms+=%s' % (k, n, ex[:10], ms[:5], sx))
