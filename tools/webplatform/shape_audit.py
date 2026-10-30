#!/usr/bin/env python3
"""Shape audit scoreboard: diff the engine's whole-surface shapes against Chrome's.

  python3 tools/webplatform/shape_audit.py <engine.json> [--top N] [--iface NAME]

Inputs: chrome/shape-audit-truth-145.json (Chrome 145) and an engine capture made with
  python3 tools/webplatform/capture/probe_run.py engine tools/webplatform/chrome/shape-audit-probe.js <engine.json>
Per interface it classifies: absent / missing members / extra members (internal helpers leaking onto public
prototypes are the interesting ones) / order / kind (accessor vs data, enumerability) / ctor length / statics / parent.
"""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
T = json.load(open(os.path.join(HERE, 'chrome/shape-audit-truth-145.json')))
E = json.load(open(sys.argv[1]))
only = sys.argv[sys.argv.index('--iface') + 1] if '--iface' in sys.argv else None
top = int(sys.argv[sys.argv.index('--top') + 1]) if '--top' in sys.argv else 40
cls = collections.defaultdict(list)
score = {}
for n, c in T.items():
    if only and n != only: continue
    e = E.get(n)
    if e is None:
        cls['absent'].append(n); score[n] = 1000; continue
    p = []
    miss = [m for m in c['order'] if m not in e['order']]
    extra = [m for m in e['order'] if m not in c['order']]
    if miss: p.append(('missing', miss))
    if extra: p.append(('extra', extra))
    common_c = [m for m in c['order'] if m in e['order']]
    common_e = [m for m in e['order'] if m in c['order']]
    if common_c != common_e: p.append(('order', []))
    kd = [m for m in common_c if c['kinds'].get(m) != e['kinds'].get(m)]
    if kd: p.append(('kind', kd))
    if c['len'] != e['len']: p.append(('len', ['%s!=%s' % (c['len'], e['len'])]))
    sm = [m for m in c['statics'] if m not in e['statics']]; se = [m for m in e['statics'] if m not in c['statics']]
    if sm or se: p.append(('statics', ['-' + x for x in sm] + ['+' + x for x in se]))
    if c['proto'] != e['proto']: p.append(('parent', [c['proto'] + '!=' + e['proto']]))
    if c.get('syms') != e.get('syms'): p.append(('syms', [str(c.get('syms')) + '!=' + str(e.get('syms'))]))
    if not p: cls['same'].append(n); continue
    for k, v in p: cls[k].append(n)
    score[n] = sum(len(v) or 1 for k, v in p)
    if only: print(json.dumps(p, indent=1))
if not only:
    print('interfaces', len(T), 'same', len(cls['same']))
    for k in ['absent', 'missing', 'extra', 'order', 'kind', 'len', 'statics', 'parent', 'syms']:
        print('%-8s %4d' % (k, len(cls[k])))
    print('\ninterfaces carrying EXTRA (non-Chrome) own members on the prototype:')
    ex = [(n, [m for m in E[n]['order'] if m not in T[n]['order']]) for n in cls['extra']]
    for n, m in ex[:top]: print('  ', n, m[:8])
    print('\nworst by mismatch count:')
    for n, s in sorted(score.items(), key=lambda x: -x[1])[:top]: print('  %4d %s' % (s, n))
