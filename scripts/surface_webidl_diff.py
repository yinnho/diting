#!/usr/bin/env python3
"""WebIDL surface diff (docs/JS-SURFACE-PLAN step 2): engine surface vs webref IDL.

Inputs (all fetched into WORK, nothing checked in):
  WORK/wr/ed/idlparsed/*.json  curl -L https://codeload.github.com/w3c/webref/tar.gz/refs/heads/curated
                               | tar -xz --strip-components=1 --include='*/ed/idlparsed/*' -C WORK/wr
  WORK/bcd.json                https://unpkg.com/@mdn/browser-compat-data@5.6.30/data.json
  engine snapshot              crates/diting/src/diting_js/runtime/surface_snapshot.txt

Two-phase because member presence is probed on INSTANCES by the engine:
  1. python3 scripts/surface_webidl_diff.py WORK members   # writes WORK/stdmembers.json
  2. SURFACE_STD_MEMBERS=WORK/stdmembers.json SURFACE_STD_OUT=WORK/probe.json \
       cargo test -p diting --lib js_surface_webidl_member_probe -- --ignored
  3. python3 scripts/surface_webidl_diff.py WORK report    # writes js/SURFACE-WEBIDL.md

"Gap" = standard member the engine lacks AND Chrome ships it (BCD chrome
version_added set, no flags, not removed). Interfaces whose instance could not
be constructed are probed on the prototype only and listed as unverified.
"""
import collections, glob, json, os, sys

WORK, MODE = sys.argv[1], sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
SNAP = os.path.join(HERE, '..', 'crates/diting/src/diting_js/runtime/surface_snapshot.txt')
OUT = os.path.join(HERE, '..', 'crates/diting/js/SURFACE-WEBIDL.md')

ifaces, partials, includes, mixins = {}, collections.defaultdict(list), [], {}
win = set()

def members(i):
    out = set()
    for m in i.get('members', []):
        if m['type'] in ('operation', 'attribute', 'const') and m.get('name'):
            if m['type'] == 'operation' and m.get('special') == 'static':
                continue
            out.add(m['name'])
    return out

for f in glob.glob(os.path.join(WORK, 'wr/ed/idlparsed/*.json')):
    d = json.load(open(f))['idlparsed']
    for k in ('*', 'Window'):
        win.update(d['jsNames']['constructors'].get(k, []))
    for n, i in d['idlNames'].items():
        if i['type'] == 'interface mixin': mixins[n] = members(i)
        elif i['type'] == 'interface': ifaces[n] = {'m': members(i), 'inh': i.get('inheritance')}
    for n, l in d['idlExtendedNames'].items():
        for i in l:
            if i['type'] == 'includes': includes.append((i['target'], i['includes']))
            elif i['type'] in ('interface', 'interface mixin'): partials[n].append(members(i))
for n, l in partials.items():
    for m in l:
        if n in ifaces: ifaces[n]['m'] |= m
        elif n in mixins: mixins[n] |= m
for t, mx in includes:
    if t in ifaces and mx in mixins: ifaces[t]['m'] |= mixins[mx]

def allm(n):
    i = ifaces.get(n)
    return set() if not i else i['m'] | (allm(i['inh']) if i['inh'] else set())

if MODE == 'members':
    out = {n: sorted(allm(n) - {'constructor'}) for n in sorted(ifaces) if n in win and allm(n)}
    json.dump(out, open(os.path.join(WORK, 'stdmembers.json'), 'w'))
    print(len(out), 'interfaces')
    sys.exit()

bcd = json.load(open(os.path.join(WORK, 'bcd.json')))['api']
def chrome_ok(c):
    s = c.get('support', {}).get('chrome')
    if isinstance(s, list): s = s[0]
    if not s or s.get('flags') or s.get('version_removed'): return False
    v = s.get('version_added')
    return bool(v) and v != 'preview'
def shipped(n, m):
    while n:
        e = bcd.get(n, {}).get(m)
        if e and chrome_ok(e.get('__compat', {})): return True
        n = ifaces.get(n, {}).get('inh')
    return False

G = set(); P = {}
for l in open(SNAP):
    l = l.rstrip('\n')
    if l.startswith('G '): G.add(l[2:])
std = sorted(n for n in win if n in ifaces)
have = [n for n in std if n in G]
miss = [n for n in std if n not in G]
miss_ship = [n for n in miss if chrome_ok(bcd.get(n, {}).get('__compat', {}))]
JSBUILTIN = set('Object Function Array ArrayBuffer SharedArrayBuffer DataView Date Error EvalError RangeError ReferenceError SyntaxError TypeError URIError AggregateError SuppressedError Promise Proxy Reflect RegExp Map Set WeakMap WeakSet WeakRef FinalizationRegistry Symbol String Number Boolean BigInt JSON Math Atomics Intl Iterator WebAssembly Infinity NaN Float16Array Float32Array Float64Array Int8Array Int16Array Int32Array BigInt64Array BigUint64Array Uint8Array Uint16Array Uint32Array Uint8ClampedArray DisposableStack AsyncDisposableStack Temporal'.split())
extra = sorted(n for n in G if n[:1].isupper() and n not in ifaces and n not in mixins and n not in JSBUILTIN)

probe = json.load(open(os.path.join(WORK, 'probe.json')))
verified = {n: v for n, v in probe.items() if v['probe'] != 'proto'}
unver = sorted(n for n, v in probe.items() if v['probe'] == 'proto')
rows = []
for n, v in verified.items():
    g = sorted(m for m in v['missing'] if shipped(n, m))
    if g: rows.append((n, v['probe'], v['total'], g))
rows.sort(key=lambda r: -len(r[3]))

L = ['# JS 表面 × WebIDL 差集（生成件）', '',
     '> 生成：`scripts/surface_webidl_diff.py`（见脚本头部三步）。基准 = webref `curated` 的 Window 暴露接口 + MDN BCD 5.6.30（Chrome 已发布判据）。',
     '> **缺口 = 标准成员我们没有 且 Chrome 已发布**；实验/带旗标/已移除的不计。成员按**实例**探测（事件/请求类垫片把成员挂实例，不挂原型）。', '',
     '## 接口级', '', f'| 项 | 数 |', '|---|---|',
     f'| 标准 Window 暴露接口 | {len(std)} |', f'| 有 | {len(have)} |',
     f'| 缺（Chrome 已发布） | {len(miss_ship)} |', f'| 缺（实验/未发布/他引擎） | {len(miss)-len(miss_ship)} |',
     f'| 多（非标准暴露，去 JS 内建） | {len(extra)} |', '',
     '**缺·Chrome 已发布**：' + (', '.join(miss_ship) or '无'), '',
     '**缺·其余（不排期）**：' + ', '.join(m for m in miss if m not in miss_ship), '',
     '**多（逐个决定留/藏）**：' + ', '.join(extra), '',
     '> 注：Audio/Image/Option 是合法命名构造器；CSS/NodeFilter 在 IDL 里是 namespace/callback，属标准——判多时先排除。', '',
     '## 成员级缺口（Chrome 已发布 · 已探到实例的接口）', '',
     f'已核 {len(verified)} 个接口；有缺口 {len(rows)} 个；缺口成员合计 {sum(len(r[3]) for r in rows)}。', '',
     '| 接口 | 探法 | 缺/标准成员 | 缺失成员 |', '|---|---|---|---|']
for n, how, tot, g in rows:
    L.append(f'| {n} | {how} | {len(g)}/{tot} | {", ".join(g)} |')
L += ['', '## 原型级探测（实例造不出，成员未核实）', '', ', '.join(unver), '']
open(OUT, 'w').write('\n'.join(L))
print('wrote', OUT, 'rows', len(rows), 'miss_ship', len(miss_ship), 'extra', len(extra))
