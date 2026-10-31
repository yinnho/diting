#!/usr/bin/env python3
"""Generate crates/diting/js/font_metrics.js from
tools/webplatform/chrome/fonts-metrics-truth-145.json (+ fonts-kern-truth-145.json).

(刀97) canvas measureText per-glyph advance tables. Each name: `t` = index
into ADV (95 advance values in em, index = charCode - 32; tables identical
to the default are shared), `asc`/`desc` = fontBoundingBox em ratios from
the 1000px capture (round(px*ratio) model), `s` = per-size Ascent/Descent
overrides for the spot sizes where CoreText quantization deviates from the
ratio model. Generic CSS families map to the table Chrome's default port
resolves them to.

(刀98) pairwise kerning: `kr` = index into KERN (adjacent-pair table in
px@100 ints, interned across fonts — the quoted-generic hidden faces carry
their own), `k3` = index into TRI (triple exceptions: ligature chains and
script contextual joins the pair model cannot express). system-ui
additionally carries optical kern knots (kkM: em@16/20/24 in 1e-4em) with
per-integer band overrides (krbM: px*100 at sizes 6..23) where GPOS device
tables make a pair oscillate at small sizes.

Usage: python3 tools/webplatform/gen/gen_font_metrics.py
"""
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
TRUTH = os.path.join(ROOT, 'tools/webplatform/chrome/fonts-metrics-truth-145.json')
KTRUTH = os.path.join(ROOT, 'tools/webplatform/chrome/fonts-kern-truth-145.json')
OUT = os.path.join(ROOT, 'crates/diting/js/font_metrics.js')

# generic family -> captured table name (Chrome 145 macOS zh persona: UNQUOTED
# generic resolution — monospace=Menlo, serif=Songti SC, cursive=Kaiti SC,
# fantasy=Papyrus; quoted "monospace" etc resolve to a different hidden face
# and stay as plain name entries served by lookup() for quoted tokens).
GENERIC = {
    'sans-serif': '__absent__',
    '-apple-system': '__absent__',
    'system-ui': 'system-ui',
    'serif': 'Songti SC',
    'monospace': 'Menlo',
    'cursive': 'Kaiti SC',
    'fantasy': 'Papyrus',
}
# canvas font shorthand in Chrome does NOT recognize ui-* generics (k97
# probe: ui-sans-serif/ui-serif/ui-monospace all measure as the default
# font) — they stay in the map so the engine does the same.
GENERIC.update({'ui-sans-serif': '__absent__', 'ui-serif': '__absent__', 'ui-monospace': '__absent__'})

SPOT = [10, 12, 16, 20, 32]
# vert100 anchors the ratio model at 100px too: CoreText per-size values
# round .5 cases differently than round(px*ratio) at some sizes.
SPOT = SPOT + [100]


def jsround(x):
    # Math.round semantics (half-up) — Python round() is banker's, which
    # disagreed with the runtime on exact-.5 products and skipped overrides.
    import math
    return math.floor(x + 0.5)


def main():
    d = json.load(open(TRUTH))
    kd = json.load(open(KTRUTH))
    tables = {}
    canon = []

    def intern(vals):
        for i, c in enumerate(canon):
            if c == vals:
                return i
        canon.append(vals)
        return len(canon) - 1

    # (k98) kern + triple canons: values as px@100 ints, dicts interned.
    kcanon = []
    tcanon = []

    def kintern(lst, dct):
        for i, c in enumerate(lst):
            if c == dct:
                return i
        lst.append(dct)
        return len(lst) - 1

    def to_int_table(m):
        return {k: int(round(v * 100)) for k, v in m.items()} if m else None

    names = {}
    for name, advs in sorted(d['adv'].items()):
        ems = [round(v / 100.0, 6) for v in advs]
        a1000, d1000 = d['vert1000'][name]
        entry = {
            't': intern(ems),
            'asc': round(a1000 / 1000.0, 6),
            'desc': round(d1000 / 1000.0, 6),
        }
        # per-size overrides only where jsround(px*ratio) misses the capture
        spots = {}
        anchors = list(zip(SPOT, d['vertspot'][name])) + [(100, tuple(d['vert100'][name]))]
        for px, (A, D) in anchors:
            if jsround(px * entry['asc']) != A or jsround(px * entry['desc']) != D:
                spots[str(px)] = [A, D]
        # (k98) system-ui box at 5/6/7 (r5 bx leg): CoreText quantizes the
        # bounding box below the k97 spot ladder too — same jsround rule.
        if name == 'system-ui' and 'bx' in kd.get('su', {}):
            for px_s, AD in kd['su']['bx'].items():
                px = float(px_s)
                if jsround(px * entry['asc']) != AD[0] or jsround(px * entry['desc']) != AD[1]:
                    spots[px_s] = AD
        if spots:
            entry['s'] = spots
        # optical legs (k97): system-ui's variable optical axis = per-char em
        # interpolated over captured knots; Apple Color Emoji's 13 keycap-source
        # chars = shared bitmap integer px ladder below 24px.
        if 'opt' in d and name in d['opt']:
            o = d['opt'][name]
            if 'adv' in o:
                # o['adv'] keys are size strings; sort_keys on the truth file
                # string-sorts them ('10'<'100'<'11') — index by key, never zip.
                entry['k'] = {
                    's': o['sizes'],
                    't': [intern([round(v / float(s), 6) for v in o['adv'][str(s)]]) for s in o['sizes']],
                }
            else:
                entry['es'] = {'l': o['ladder'], 'f': o['sizes'][0], 'x': [1 if i in set(o['idx']) else 0 for i in range(95)]}
        names[name.lower()] = entry
        krn = to_int_table(kd['kern'].get(name, {}))
        if krn:
            entry['kr'] = kintern(kcanon, krn)
        tr = to_int_table(kd['tri'].get(name, {}))
        if tr:
            entry['k3'] = kintern(tcanon, tr)
    # generic entries resolve through NAMES at runtime so the resolved object is
    # the single NAMES entry (k98 attaches the system-ui kern maps onto it).
    generic = dict(GENERIC)

    # EXT: 210 non-ASCII codepoints, per-font rows at 100px (interned into the
    # same canon — different lengths never collide); the optical-axis font
    # additionally carries knot rows for its EXT columns.
    ext_cp = []
    if 'ext' in d:
        ext_cp = d['ext']['cp']
        for name, row in sorted(d['ext']['adv'].items()):
            if name.lower() in names:
                names[name.lower()]['x'] = intern([round(v / 100.0, 6) for v in row])
        if d['ext'].get('su'):
            names['system-ui']['xk'] = {
                's': d['opt']['system-ui']['sizes'],
                't': [intern([round(v / float(s), 6) for v in d['ext']['su'][str(s)]]) for s in d['opt']['system-ui']['sizes']],
            }

    lines = []
    lines.append('// GENERATED by tools/webplatform/gen/gen_font_metrics.py from')
    lines.append('// chrome/fonts-metrics-truth-145.json - do not edit.')
    lines.append('// (刀97) per-glyph canvas advance tables (em) + fontBoundingBox ratios,')
    lines.append('// captured from Chrome 145 on the macOS arm64 default port. `t` indexes')
    lines.append('// ADV (tables identical to the default are shared), index = charCode - 32;')
    lines.append('// `s` = per-size Ascent/Descent overrides where CoreText small-size')
    lines.append('// quantization deviates from round(px*ratio).')
    lines.append('// (刀98) pairwise kern tables KERN/TRI (px@100 ints) + system-ui optical')
    lines.append('// kern knots and small-size band overrides; sum() adds kern and triple')
    lines.append('// terms — see chrome/fonts-kern-truth-145.json for the model and its')
    lines.append('// measured boundaries.')
    lines.append('(function () {')
    lines.append('  const ADV = %s;' % json.dumps(canon, separators=(',', ':')))
    lines.append('  const NAMES = %s;' % json.dumps(names, separators=(',', ':'), sort_keys=True))
    lines.append('  const GENERIC = %s;' % json.dumps(generic, separators=(',', ':'), sort_keys=True))
    lines.append('  // CJK ideographs/kana measure exactly 1em/char, Hangul 0.865em (k97 cjk leg).')
    lines.append('  const isCJK = (c) => (c >= 0x2E80 && c <= 0x9FFF) || (c >= 0xF900 && c <= 0xFAFF) || (c >= 0x20000 && c <= 0x2FA1F);')
    lines.append("  const isHangul = (c) => c >= 0xAC00 && c <= 0xD7A3;")
    lines.append('  const def = NAMES["__absent__"];')
    lines.append("  const FALLBACK = 79; // 'o'-class advance for non-ASCII chars without a rule")
    if kcanon:
        lines.append('  // (k98) pairwise kerning: KERN[i] = {pair: px@100 int}; KMAP keys are')
        lines.append('  // numeric (cp0*65536 + cp1). system-ui additionally carries kkM (optical')
        lines.append('  // kern knots [em@16, em@20, em@24] in 1e-4em, linear >=28) and krbM')
        lines.append('  // (per-integer band overrides, px*100 at sizes 6..23) for the pairs where')
        lines.append('  // GPOS device tables oscillate at small sizes.')
        lines.append('  const KERN = %s;' % json.dumps(kcanon, separators=(',', ':')))
        lines.append('  const numKey = (s) => s.charCodeAt(0) * 65536 + s.charCodeAt(1);')
        lines.append('  const KMAP = KERN.map((o) => { const m = new Map(); for (const k in o) m.set(numKey(k), o[k]); return m; });')
    if tcanon:
        lines.append('  // (k98) triple exceptions (ligature chains, script joins): px@100 ints.')
        lines.append('  const TRI = %s;' % json.dumps(tcanon, separators=(',', ':')))
        lines.append('  const TMAP = TRI.map((o) => { const m = new Map(); for (const k in o) m.set(k.charCodeAt(0) * 4294967296 + k.charCodeAt(1) * 65536 + k.charCodeAt(2), o[k]); return m; });')
    if kd.get('su'):
        lines.append('  const SU = %s;' % json.dumps(kd['su'], separators=(',', ':')))
        lines.append('  NAMES["system-ui"].kkM = new Map(Object.entries(SU.kk).map(([k, v]) => [numKey(k), v]));')
        lines.append('  NAMES["system-ui"].krbM = new Map(Object.entries(SU.kb).map(([k, v]) => [numKey(k), v]));')
    if kcanon:
        lines.append('  // kern of an adjacent pair (px at the given size); band overrides win')
        lines.append('  // below 24, optical knots blend 16..28, >=28 is exact linear.')
        lines.append('  const kernOf = (m, kkM, kbM, a, b, px) => {')
        lines.append('    const key = a * 65536 + b;')
        lines.append('    const v = m.get(key);')
        lines.append('    if (v === undefined) return 0;')
        lines.append('    if (kbM) {')
        lines.append('      const B = kbM.get(key);')
        lines.append('      if (B !== undefined && px < 24) {')
        lines.append('        if (px <= 6) return B[0] * px / 600;')
        lines.append('        const t = px - 6, lo = Math.floor(t);')
        lines.append('        if (lo >= B.length - 1) {')
        lines.append('          const K = kkM.get(key);')
        lines.append('          const k23 = B[B.length - 1] / 100;')
        lines.append('          const k24 = K ? K[2] * 24 / 10000 : v * 24 / 10000;')
        lines.append('          return k23 + (k24 - k23) * (px - 23);')
        lines.append('        }')
        lines.append('        return (B[lo] + (B[lo + 1] - B[lo]) * (t - lo)) / 100;')
        lines.append('      }')
        lines.append('    }')
        lines.append('    if (kkM) {')
        lines.append('      const K = kkM.get(key);')
        lines.append('      if (K !== undefined) {')
        lines.append('        if (px >= 28) return v * px / 10000;')
        lines.append('        if (px >= 24) return (K[2] + (v - K[2]) * (px - 24) / 4) * px / 10000;')
        lines.append('        if (px >= 20) return (K[1] + (K[2] - K[1]) * (px - 20) / 4) * px / 10000;')
        lines.append('        if (px >= 16) return (K[0] + (K[1] - K[0]) * (px - 16) / 4) * px / 10000;')
        lines.append('        return K[0] * px / 10000;')
        lines.append('      }')
        lines.append('    }')
        lines.append('    return v * px / 10000;')
        lines.append('  };')
    if ext_cp:
        lines.append('  // EXT: non-ASCII codepoints with per-font advances (Latin-1 + General')
        lines.append('  // Punctuation + symbols). XI maps codepoint -> row index.')
        lines.append('  const XP = %s;' % json.dumps(ext_cp, separators=(',', ':')))
        lines.append('  const XI = new Map();')
        lines.append('  for (let i = 0; i < XP.length; i++) XI.set(XP[i], i);')
        lines.append('  const xAdv = (tbl, i, px) => {')
        lines.append('    if (tbl.xk) {')
        lines.append('      const S = tbl.xk.s, T = tbl.xk.t;')
        lines.append('      if (px <= S[0]) return ADV[T[0]][i] * px;')
        lines.append('      const last = S.length - 1;')
        lines.append('      if (px >= S[last]) return ADV[T[last]][i] * px;')
        lines.append('      let j = 1; while (S[j] < px) j++;')
        lines.append('      const a = ADV[T[j - 1]][i], b = ADV[T[j]][i];')
        lines.append('      return (a + (b - a) * (px - S[j - 1]) / (S[j] - S[j - 1])) * px;')
        lines.append('    }')
        lines.append('    return tbl.x === undefined ? ADV[def.x][i] * px : ADV[tbl.x][i] * px;')
        lines.append('  };')
    lines.append('  // advance of ch (in px at the given size) for a resolved table:')
    lines.append('  //  - k: variable optical axis (SF Pro) -> per-char em interpolated over knots')
    lines.append('  //  - es: bitmap integer px ladder (Apple Color Emoji keycap chars) at small sizes')
    lines.append('  //  - else: fixed em table * px')
    lines.append('  const adv = (tbl, c, px) => {')
    if ext_cp:
        lines.append('    if (c > 126 && XI.has(c)) { return xAdv(tbl, XI.get(c), px); }')
    lines.append('    if (isCJK(c)) return px;')
    lines.append('    if (isHangul(c)) return 0.865 * px;')
    lines.append('    const i = c - 32;')
    lines.append('    if (i < 0 || i >= 95) return ADV[def.t][FALLBACK] * px;')
    lines.append('    if (tbl.k) {')
    lines.append('      const S = tbl.k.s, T = tbl.k.t;')
    lines.append('      if (px <= S[0]) return ADV[T[0]][i] * px;')
    lines.append('      const last = S.length - 1;')
    lines.append('      if (px >= S[last]) return ADV[T[last]][i] * px;')
    lines.append('      let j = 1; while (S[j] < px) j++;')
    lines.append('      const a = ADV[T[j - 1]][i], b = ADV[T[j]][i];')
    lines.append('      return (a + (b - a) * (px - S[j - 1]) / (S[j] - S[j - 1])) * px;')
    lines.append('    }')
    lines.append('    if (tbl.es && tbl.es.x[i] && px < tbl.es.f + tbl.es.l.length) {')
    lines.append('      const L = tbl.es.l, t = px - tbl.es.f;')
    lines.append('      if (t <= 0) return L[0];')
    lines.append('      const lo = Math.floor(t);')
    lines.append('      return lo >= L.length - 1 ? L[L.length - 1] : L[lo] + (L[lo + 1] - L[lo]) * (t - lo);')
    lines.append('    }')
    lines.append('    return ADV[tbl.t][i] * px;')
    lines.append('  };')
    lines.append('  globalThis.__ditingFontMetrics = {')
    lines.append('    lookup(name) { return NAMES[String(name).toLowerCase()] || null; },')
    lines.append('    generic(name) { const g = GENERIC[String(name).toLowerCase()]; return g ? NAMES[g] : null; },')
    lines.append('    sum(tbl, text, px) {')
    lines.append('      let w = 0;')
    if kcanon:
        lines.append('      const m = tbl.kr === undefined ? null : KMAP[tbl.kr];')
        lines.append('      const kkM = tbl.kkM || null, kbM = tbl.krbM || null;')
        lines.append('      const tm = tbl.k3 === undefined ? null : TMAP[tbl.k3];')
        lines.append('      let p1 = -1, p2 = -1;')
    lines.append('      for (let i = 0; i < text.length; i++) {')
    lines.append('        let c = text.charCodeAt(i);')
    lines.append('        if (c >= 0xD800 && c <= 0xDBFF && i + 1 < text.length) {')
    lines.append('          const lo = text.charCodeAt(i + 1);')
    lines.append('          if (lo >= 0xDC00 && lo <= 0xDFFF) { c = 0x10000 + ((c - 0xD800) << 10) + (lo - 0xDC00); i++; }')
    lines.append('        }')
    lines.append('        w += adv(tbl, c, px);')
    if kcanon:
        lines.append('        if (p1 >= 0 && m) w += kernOf(m, kkM, kbM, p1, c, px);')
        lines.append('        if (p2 >= 0 && tm) { const tv = tm.get(p2 * 4294967296 + p1 * 65536 + c); if (tv !== undefined) w += tv * px / 10000; }')
        lines.append('        p2 = p1; p1 = c;')
    lines.append('      }')
    lines.append('      return w;')
    lines.append('    },')
    lines.append('    vertical(tbl, px) {')
    lines.append('      const s = tbl.s && tbl.s[px];')
    lines.append('      return s ? [s[0], s[1]] : [Math.round(px * tbl.asc), Math.round(px * tbl.desc)];')
    lines.append('    },')
    lines.append('  };')
    lines.append('})();')
    src = '\n'.join(lines) + '\n'
    open(OUT, 'w', encoding='utf8').write(src)
    spots_n = sum(1 for n in names.values() if 's' in n)
    print('wrote %s: %d unique tables, %d names, %d generics, %d fonts with size overrides, '
          '%d kern tables (%d pairs), %d triple tables (%d), %d su knots, %d su bands' % (
              os.path.relpath(OUT, ROOT), len(canon), len(names), len(generic), spots_n,
              len(kcanon), sum(len(t) for t in kcanon), len(tcanon), sum(len(t) for t in tcanon),
              len(kd.get('su', {}).get('kk', {})), len(kd.get('su', {}).get('kb', {}))))


if __name__ == '__main__':
    main()
