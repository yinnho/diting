"""Post-pass for gen_font_metrics.py: move the big truth tables out of the
V8 snapshot.

Why: every realm restores the whole snapshot into its own heap. With the
tables inline (~90MB of JSON literals) a blank session cost ~340MB RSS and
the realm heap 326MB post-restore; without them ~10MB / 7.9MB. Isolates
cannot share heap objects, so the data has to live outside V8.

What: each big `const NAME = <json>;` line is cut out and its top-level
entries written verbatim (raw JSON bytes, so key order -- and every for..in
order the model depends on -- is untouched) into font_metrics_data.tables, with
an [offset, len] index in font_metrics_index.json. The engine embeds both
read-only (one copy per process); the JS side fetches one entry on first
touch through the `font_table` op and keeps it per realm. The load-time
fixups that used to walk every family now run per family when it is first
decoded (`fixFamily`), in the same order. The measuring code is unchanged.

Every edit below is an exact-text replacement asserted to match once: when
the generator's template moves, this fails loudly instead of drifting.
"""
import json
import os
import re

# Tables served lazily. Everything else (LIGAP, MIDF, AKC, SBAND, XP, ...) is
# small and stays inline.
BIG = ['ADV', 'NAMES', 'KERN', 'TRI', 'CFKT', 'TRIF', 'TSP', 'SU', 'SUF', 'AKJ', 'AKE', 'GENQ']

HELPERS = r'''  // (#lazy) The big truth tables live outside the snapshot (one read-only
  // copy per process, see font_metrics_lazy.py); FT fetches one top-level
  // entry by table + key and the proxies below keep it per realm. A missing
  // key falls back to what a plain object literal would answer for it.
  const FDOM = globalThis.__diting_domRaw;
  const FT = (tbl, key) => { const s = FDOM('font_table', tbl, String(key)); return s === '' ? undefined : JSON.parse(s); };
  const lazyArr = (tbl, build) => new Proxy([], { get(t, k) {
    if (typeof k !== 'string' || !/^\d+$/.test(k)) return t[k];
    if (!(k in t)) { const v = FT(tbl, k); t[k] = v === undefined ? undefined : build(v); }
    return t[k];
  } });
  const lazyObj = (tbl, build) => new Proxy(Object.create(null), { get(t, k) {
    if (typeof k !== 'string') return ({})[k];
    if (!(k in t)) { const v = FT(tbl, k); t[k] = v === undefined ? ({})[k] : build(v, k); }
    return t[k];
  } });
  const lazyMap = (tbl, build) => { const c = new Map(); return { get(k) {
    if (!c.has(k)) { const v = FT(tbl, k); c.set(k, v === undefined ? undefined : build(v)); }
    return c.get(k);
  } }; };
  let DEF_ = null;
  const DEF = () => DEF_ || (DEF_ = NAMES["__absent__"]);
'''

# Per-family load-time fixups, verbatim from the eager init it replaces, in
# the original order: k3f -> TRIF maps, system-ui triSpot rows, MIDF mid
# layer, system-ui SU/SUF kern + borrows, armed-kernel tags, kfm/krb maps,
# system-ui sba bands (that last one used to sit after XP's comment block).
FIX_FAMILY = r'''  // (#lazy) Load-time fixups, verbatim from the eager init they replace,
  // now run when a family is first decoded. Original order per face:
  // k3f -> TRIF maps, triSpot rows, MIDF mid layer, SUF kern + borrows,
  // armed-kernel tag, kfm/krb maps, sba bands. system-ui (whose SUF/TSP
  // rows are ~36MB) runs them per FACE on first access, so measuring the
  // regular face never decodes the bold/semibold/... rows.
  const tspMap = (o) => new Map(Object.entries(o).map(([t, v]) => [t3key(t), v]));
  const SU_FACES = new Set(["B", "I", "Z", "M", "S", "100", "200", "300", "600", "800", "900", "SB", "SI", "SZ", "S100", "S200", "S300", "S500", "S600", "S800", "S900"]);
  const SU_AK = new Set(['B', 'I', 'M', 'S', '100', '300', '900']);
  const sufCache = new Map();
  // A face's SUF-derived maps, made once: the 600/800/900 borrow shares
  // B's objects as they stood after the SUF step (before B's own krb).
  const sufMaps = (t) => {
    if (!sufCache.has(t)) {
      const sf = SU_FACES.has(t) ? FT('SUF', t) : undefined;
      sufCache.set(t, sf ? { kkM: new Map(Object.entries(sf.kk).map(([k, v]) => [numKey(k), v])),
        vM: new Map(Object.entries(sf.v100).map(([k, v]) => [numKey(k), v])),
        krbM: new Map(Object.entries(sf.kb).map(([k, o]) => [numKey(k), new Map(Object.entries(o).map(([px, v]) => [+px, v]))])) } : null);
    }
    return sufCache.get(t);
  };
  const fixK3f = (e) => {
    if (e.k3f !== undefined) { const o = TRIFM[e.k3f];
      if (o) {
        if (o.m) e.k3m = o.m;
        if (o.k) e.k3k = o.k;
        if (o.g) e.k3g = o.g;
      }
    }
  };
  const fixMaps = (e) => {
    if (e.kfm) { e.kfmM = new Map(Object.entries(e.kfm).map(([p, v]) => [numKey(p), v])); e.kfm = undefined; }
  };
  const fixKrb = (e) => {
    if (e.krb) e.krbM = new Map(Object.entries(e.krb).map(([p, v]) => [numKey(p), v]));
  };
  function fixSystemUiFace(fam, raw, t) {
    const e = raw[t];
    fixK3f(e);
    if (t !== 'R') { const o = FT('TSP', t); if (o) e.k3s = tspMap(o); }
    if (fam.midM && !e.midM) e.midM = fam.midM;
    const m = sufMaps(t);
    if (m) { e.kkM = m.kkM; e.vM = m.vM; e.krbM = m.krbM; }
    if ((t === '100' || t === '200' || t === '300') && !e.kkM) { e.kr = fam.kr; e.kkM = fam.kkM; e.krbM = fam.krbM; }
    if ((t === '600' || t === '800' || t === '900') && !e.kkM && raw.B) { const b = sufMaps('B'); e.kkM = b ? b.kkM : undefined; e.vM = b ? b.vM : undefined; e.krbM = b ? b.krbM : undefined; }
    if (SU_AK.has(t)) e.ak = t;
    fixMaps(e);
    fixKrb(e);
    if (e && e.sba) e.sba = [new Map(Object.entries(e.sba.bump || {}).map(([k, v]) => [+k, v])), new Map(Object.entries(e.sba.arch || {}).map(([k, v]) => [+k, v]))];
  }
  function fixFamily(nk, fam) {
    const mm = MIDM.get(nk);
    if (nk === 'system-ui') {
      const tspR = FT('TSP', 'R');
      if (tspR) fam.k3s = tspMap(tspR);
      if (mm) fam.midM = mm;
      fam.kkM = new Map(Object.entries(FT('SU', 'kk')).map(([k, v]) => [numKey(k), v]));
      fam.krbM = new Map(Object.entries(FT('SU', 'kb')).map(([k, o]) => [numKey(k), new Map(Object.entries(o).map(([px, v]) => [+px, v]))]));
      fam.ak = 'R';
      const raw = fam.f;
      if (raw) {
        const done = new Set();
        fam.f = new Proxy(raw, { get(t, k) {
          if (typeof k === 'string' && !done.has(k) && Object.prototype.hasOwnProperty.call(t, k)) { done.add(k); fixSystemUiFace(fam, t, k); }
          return t[k];
        } });
      }
      return fam;
    }
    const fmap = fam.f;
    if (fmap) for (const t in fmap) fixK3f(fmap[t]);
    if (mm) {
      fam.midM = mm;
      if (fmap) for (const t in fmap) { if (!fmap[t].midM) fmap[t].midM = mm; }
    }
    if (fmap) for (const t in fmap) fixMaps(fmap[t]);
    if (fmap) for (const t in fmap) fixKrb(fmap[t]);
    return fam;
  }
'''


def _spans(text):
    """Top-level entries of a JSON object/array as (key, raw_value_text)."""
    i = 0
    n = len(text)
    out = []
    opener = text[0]
    assert opener in '[{'
    i = 1
    idx = 0

    def skip_ws(j):
        while j < n and text[j] in ' \t\r\n':
            j += 1
        return j

    def value_end(j):
        depth = 0
        while j < n:
            c = text[j]
            if c == '"':
                j += 1
                while text[j] != '"':
                    j += 2 if text[j] == '\\' else 1
            elif c in '[{':
                depth += 1
            elif c in ']}':
                if depth == 0:
                    return j
                depth -= 1
            elif c == ',' and depth == 0:
                return j
            j += 1
        raise ValueError('unterminated')

    while True:
        i = skip_ws(i)
        if text[i] in ']}':
            break
        if opener == '{':
            assert text[i] == '"'
            j = i + 1
            while text[j] != '"':
                j += 2 if text[j] == '\\' else 1
            key = json.loads(text[i:j + 1])
            i = skip_ws(j + 1)
            assert text[i] == ':'
            i = skip_ws(i + 1)
        else:
            key = str(idx)
            idx += 1
        j = value_end(i)
        out.append((key, text[i:j]))
        i = skip_ws(j)
        if text[i] == ',':
            i += 1
    return out


def _replace_once(src, old, new):
    c = src.count(old)
    assert c == 1, 'font_metrics_lazy: expected one match, got %d for: %r' % (c, old[:120])
    return src.replace(old, new)


def _cut_block(src, start, end):
    """Remove the block from the line starting `start` through the line `end`."""
    i = src.index(start)
    assert src.count(start) == 1, start
    j = src.index(end, i) + len(end)
    return src[:i] + src[j:]


def lazify(src, data_path, index_path):
    lines = src.split('\n')
    blob = bytearray()
    index = {}
    out = []
    pat = re.compile(r'^  const (%s) = (.*);$' % '|'.join(BIG))
    seen = set()
    for l in lines:
        m = pat.match(l)
        if not m:
            out.append(l)
            continue
        name, payload = m.group(1), m.group(2)
        seen.add(name)
        json.loads(payload)  # pure data, or this post-pass is wrong
        tbl = {}
        for key, raw in _spans(payload):
            b = raw.encode('ascii')
            tbl[key] = [len(blob), len(b)]
            blob += b
        index[name] = tbl
        if name == 'ADV':
            out.append(HELPERS.rstrip('\n'))
            out.append("  const ADV = lazyArr('ADV', (v) => v);")
        elif name == 'NAMES':
            out.append("  const NAMES = lazyObj('NAMES', (v, nk) => fixFamily(nk, v));")
        elif name == 'GENQ':
            out.append("  const GENQ = lazyObj('GENQ', (v) => v);")
    assert seen == set(BIG), 'font_metrics_lazy: missing tables %s' % (set(BIG) - seen)
    src = '\n'.join(out)

    src = _replace_once(src, '  const def = NAMES["__absent__"];\n', '')
    src = _replace_once(src, 'ADV[def.x]', 'ADV[DEF().x]')
    src = _replace_once(src, 'ADV[def.t]', 'ADV[DEF().t]')
    src = _replace_once(src,
        '  const KMAP = KERN.map((o) => {',
        "  const KMAP = lazyArr('KERN', (o) => {")
    src = _replace_once(src,
        '  const TMAP = TRI.map((o) => {',
        "  const TMAP = lazyArr('TRI', (o) => {")
    src = _replace_once(src,
        '  const CFKTM = CFKT.map((o) => {',
        "  const CFKTM = lazyArr('CFKT', (o) => {")
    src = _replace_once(src,
        '''  const TRIFM = new Map();
  for (const key in TRIF) { const o = TRIF[key];
    TRIFM.set(key, { m: o.m ? new Map(Object.entries(o.m).map(([t, v]) => [t3key(t), v])) : null,
      k: o.k ? new Set(o.k.map(t3key)) : null,
      g: o.g ? new Map(Object.entries(o.g).map(([t, v]) => [t3key(t), v])) : null });
  }
''',
        '''  const TRIFM = lazyObj('TRIF', (o) => ({ m: o.m ? new Map(Object.entries(o.m).map(([t, v]) => [t3key(t), v])) : null,
      k: o.k ? new Set(o.k.map(t3key)) : null,
      g: o.g ? new Map(Object.entries(o.g).map(([t, v]) => [t3key(t), v])) : null }));
''')
    src = _cut_block(src, '  for (const nk in NAMES) { const fmap = NAMES[nk].f;\n    if (!fmap) continue;\n    for (const t in fmap) { const e = fmap[t];\n      if (e.k3f',
                     '        if (o.g) e.k3g = o.g;\n      }\n    }\n  }\n')
    src = _cut_block(src, '  const TSPM = new Map();\n',
                     "  for (const [tag, mm] of TSPM) { if (tag !== 'R' && suF4 && suF4[tag]) suF4[tag].k3s = mm; }\n")
    src = _cut_block(src, '  for (const nk in NAMES) { const mm = MIDM.get(nk); if (mm) {\n',
                     '    if (fm2) for (const t in fm2) { if (!fm2[t].midM) fm2[t].midM = mm; }\n  } }\n')
    src = _cut_block(src, '  NAMES["system-ui"].kkM = new Map(',
                     'new Map(Object.entries(o).map(([px, v]) => [+px, v]))]));\n')
    src = _cut_block(src, "  const suF = NAMES['system-ui'].f;\n",
                     "e.kkM = suF.B.kkM; e.vM = suF.B.vM; e.krbM = suF.B.krbM; } }\n")
    src = _replace_once(src,
        '  const AKJM = new Map(Object.entries(AKJ).map(([t, o]) => [t, new Map(Object.entries(o).map(([k, v]) => [+k, v]))]));\n',
        "  const AKJM = lazyMap('AKJ', (o) => new Map(Object.entries(o).map(([k, v]) => [+k, v])));\n")
    src = _replace_once(src,
        '  const AKEM = new Map(Object.entries(AKE).map(([t, o]) => [t, new Map(Object.entries(o).map(([k, v]) => [+k, v]))]));\n',
        "  const AKEM = lazyMap('AKE', (o) => new Map(Object.entries(o).map(([k, v]) => [+k, v])));\n")
    src = _cut_block(src, "  NAMES['system-ui'].ak = 'R';\n",
                     "if (akF[t]) akF[t].ak = t; }\n")
    src = _cut_block(src, '  for (const nk in NAMES) { const fmap = NAMES[nk].f;\n    if (!fmap) continue;\n    for (const t in fmap) { const e = fmap[t];\n      if (e.kfm)',
                     'e.kfm = undefined; }\n    }\n  }\n')
    src = _cut_block(src, '  for (const k of Object.keys(NAMES)) { const fm3 = NAMES[k].f;\n',
                     'if (e.krb) e.krbM = new Map(Object.entries(e.krb).map(([p, v]) => [numKey(p), v])); } }\n')
    src = _cut_block(src, '  const sfe = NAMES["system-ui"].f;\n',
                     "new Map(Object.entries(e.sba.arch || {}).map(([k, v]) => [+k, v]))];\n  }\n")
    src = _replace_once(src, '  const AKCE = new Set([]);\n', '  const AKCE = new Set([]);\n' + FIX_FAMILY)

    # Nothing may still read the cut tables by name.
    code = re.sub(r'//[^\n]*', '', src)
    for name in ['KERN', 'TRI', 'CFKT', 'TRIF', 'TSP', 'SU', 'SUF', 'AKJ', 'AKE', 'TSPM']:
        hits = [m.start() for m in re.finditer(r'(?<![\w\'"])%s(?![\w\'"])' % name, code)]
        assert not hits, 'font_metrics_lazy: %s still referenced' % name

    with open(data_path, 'wb') as f:
        f.write(bytes(blob))
    with open(index_path, 'w', encoding='utf8') as f:
        json.dump(index, f, separators=(',', ':'), sort_keys=True)
        f.write('\n')
    return src
