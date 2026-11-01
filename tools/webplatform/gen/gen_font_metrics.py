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
FTRUTH = os.path.join(ROOT, 'tools/webplatform/chrome/fonts-faces-truth-145.json')
OUT = os.path.join(ROOT, 'crates/diting/js/font_metrics.js')

# generic family -> captured table name (Chrome 145 macOS zh persona: UNQUOTED
# generic resolution — monospace=Menlo, serif=Songti SC, cursive=Kaiti SC,
# fantasy=Papyrus; quoted "monospace" etc resolve to a different hidden face
# and stay as plain name entries served by lookup() for quoted tokens).
GENERIC = {
    'sans-serif': '__absent__',
    '-apple-system': '__absent__',
    'system-ui': 'system-ui',
    'serif': 'songti sc',
    'monospace': 'menlo',
    'cursive': 'kaiti sc',
    'fantasy': 'papyrus',
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
    fd = json.load(open(FTRUTH)) if os.path.exists(FTRUTH) else None
    # scKernBands keys are display-case font names; index them by the
    # lowercase NAMES key once.
    SCB = ({k.lower(): v for k, v in fd['scKernBands'].items()}
           if fd and fd.get('scKernBands') else {})
    # (刀100 d) faces whose kern was captured over the full universe
    # (composites + the plain-S legs recaptured in d3S): absent pair =
    # measured zero — krS suppresses the R-kern fallback.
    KFULL = set(fd.get('kernFull') or [])
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
        # px@100 ×100; keep 2 decimals of the scaled value when the truth
        # carries them (k99 r6l: kernFaces recaptured at 4dp — the old int
        # rounding cost up to 0.005px/pair at 100px). 2dp truths emit as
        # plain ints unchanged.
        if not m:
            return None
        out = {}
        for k, v in m.items():
            s = round(v * 100, 2)
            out[k] = int(s) if s == int(s) else s
        return out

    # ---- (刀99) face axis: per-font face tables from the faces truth ----
    # (刀100) 600/800/900 joined the tag set: Kefa's 800/900 faces are NOT
    # bold-row copies (sentinel 1020.40 vs B 958.30), so fonts whose truth
    # rows exist get exact faces; the rest keep the >=600→B fallback.
    # (刀100 d) sc composites: chrome synthesizes small-caps on the resolved
    # weight/italic face, so bsc/isc/zsc/numeric-sc are DISTINCT faces per
    # family; scLattice maps runtime variants onto these tags.
    SC_COMP = {'SB', 'SI', 'SZ', 'S100', 'S200', 'S300', 'S500', 'S600', 'S800', 'S900'}
    FTAGS = ['B', 'I', 'Z', 'M', 'S',
             'SB', 'SI', 'SZ', 'S100', 'S200', 'S300', 'S500', 'S600', 'S800', 'S900',
             '100', '200', '300', '600', '800', '900']
    VSPOT = [10, 12, 16, 20, 32]

    def build_vert(fe, vf):
        # vertF row: '16'/'100' absolute, '1000' ratio ints, 'spot' = [A,D]
        # at VSPOT[0,1,3,4] (16 lives under its own key).
        a1000, d1000 = vf['1000']
        fe['asc'] = round(a1000 / 1000.0, 6)
        fe['desc'] = round(d1000 / 1000.0, 6)
        vals = {10: vf['spot'][0], 12: vf['spot'][1], 16: vf['16'],
                20: vf['spot'][2], 32: vf['spot'][3], 100: vf['100']}
        spots = {}
        for px, (A, D) in vals.items():
            if jsround(px * fe['asc']) != A or jsround(px * fe['desc']) != D:
                spots[str(px)] = [A, D]
        if spots:
            fe['s'] = spots

    def face_entry(rk, tag):
        # rk = res key of the font, tag = face tag; returns a standalone
        # table entry or None when the face resolves to R.
        v = fd['res'].get(rk, {}).get(tag)
        if v is None or v == '=R':
            return None
        if v == '=DEFBOLD':
            return dict(DEFBOLD)
        row = fd['tables'][v]
        return row

    # the shared DEFBOLD table (bold "PingFang SC") — authoritative index in
    # _meta.defboldIdx; the last-entry position only as fallback (刀100 d:
    # face-table appends had silently displaced it, resolving 50 '=DEFBOLD'
    # faces onto a stray sc row).
    DEFBOLD = {}
    if fd:
        dbi = (fd.get('_meta') or {}).get('defboldIdx')
        dbrow = fd['tables'][dbi] if isinstance(dbi, int) and 0 <= dbi < len(fd['tables']) else fd['tables'][-1]
        DEFBOLD = {'t': intern([round(x, 6) for x in dbrow[:95]]),
                   'x': intern([round(x, 6) for x in dbrow[95:]])}
        vf = fd['vertF'].get('PingFang SC#B')
        if vf:
            build_vert(DEFBOLD, vf)
        dbk = to_int_table(fd['kernFaces'].get('=DEFBOLD', {}))
        if dbk:
            DEFBOLD['kr'] = kintern(kcanon, dbk)

    # (刀100 C3) face-triple deltas: 198 font#tag combos deviate from their
    # font's R triple table, and NOT proportionally (face/R ratio IQR
    # 0.53-0.84 — a scalar can't model it). Delta layer instead of full
    # per-face tables (0.9MB vs 8MB): m = measured px@100 overrides that
    # beat TMAP, k = R triples the face kills. Values scale x100 like
    # every kern truth. Keys are display-case Font#tag (res convention).
    triFaces = (fd.get('triFaces') or {}) if fd else {}
    trif = {}
    for key, spec in triFaces.items():
        ent = {}
        mt = to_int_table(spec.get('mod') or {})
        if mt:
            ent['m'] = mt
        if spec.get('kill'):
            ent['k'] = spec['kill']
        if ent:
            trif[key] = ent
    # (刀100 c5) whitespace-edge triple mid-value layer: standalone captures
    # for windows with U+202F at an edge mispredict longer strings. Mass
    # capture (2,621 R windows + 3,341 TRIF windows, flanked-vs-standalone
    # decomposition, c5m/c5g): mid fire/partial/gate is a per-WINDOW property,
    # flank-invariant; faces follow their family except where triMidFaces
    # carries a face-level override (MF kills at face level, AT faces gate,
    # CB/CD keep partial v_mid values). Stored where |v_mid - v| > tol.
    triMid = (kd.get('triMid') or {}) if kd else {}
    triMidFaces = (fd.get('triMidFaces') or {}) if fd else {}
    for key, spec in triMidFaces.items():
        if key in trif:
            gt = to_int_table(spec)
            if gt:
                trif[key]['g'] = gt

    def build_faces(rk, entry):
        # attach entry['f'] = {tag: face table entry} for every face that is
        # NOT plain R. Near-R faces (ascii row == R, ext-fallback-only diffs)
        # reference the base table + sparse `xp` ext overrides.
        fmap = {}
        for tag in FTAGS:
            row = face_entry(rk, tag)
            if row is None:
                continue
            if isinstance(row, dict):
                fe = row  # DEFBOLD share
            else:
                fe = {}
                ascii95 = [round(x, 6) for x in row[:95]]
                base_t = entry.get('t')
                # near-R with 1e-5 em tolerance: the w100/200/300 face rows
                # captured at 5-decimal precision are ROUNDING variants of R
                # (probed: Chrome resolves weight 100/200/300 to the regular
                # face for these fonts — same table AND same kern).
                if base_t is not None and all(
                        abs(a - b) <= 1e-5 for a, b in zip(canon[base_t], ascii95)):
                    fe['t'] = base_t
                    # entry['x'] is attached in the ext pass AFTER this loop,
                    # so diff the face ext row against the k97 truth directly.
                    kd_name = rk[2:] if rk.startswith('g:') else rk
                    extR = d['ext']['adv'].get(kd_name)
                    if extR:
                        xp = {i: round(row[95 + i], 6) for i in range(210)
                              if i < len(extR) and abs(row[95 + i] - extR[i] / 100.0) > 1e-5}
                        if xp:
                            fe['xp'] = xp
                        # (刀100 C3) near-R faces must carry the R ext row as
                        # their base: xAdv falls xp-misses through to tbl.x,
                        # and without it every non-override ext slot read
                        # __absent__ (latent since 刀99; caught by Zapfino#B
                        # ⁝=27.78 vs chrome 100.0). Same values the ext pass
                        # interns later — intern() dedups to one table.
                        fe['x'] = intern([round(v / 100.0, 6) for v in extR])
                    # near-R face: the primary face is the regular one, so
                    # its kern (when unmeasured for this face) IS the R kern —
                    # Zapfino bold kerns exactly like regular (probed).
                    if (rk + '#' + tag) not in fd['kernFaces'] and entry.get('kr') is not None:
                        fe['kr'] = entry['kr']
                else:
                    fe['t'] = intern(ascii95)
                    fe['x'] = intern([round(x, 6) for x in row[95:]])
                vf = fd['vertF'].get(rk + '#' + tag)
                if vf:
                    build_vert(fe, vf)
                kf = fd['kernFaces'].get(rk + '#' + tag)
                if kf:
                    kt2 = to_int_table(kf)
                    if kt2:
                        fe['kr'] = kintern(kcanon, kt2)
                        if (rk + '#' + tag) in KFULL:
                            # (刀100 d) full-universe capture: absent pair =
                            # measured zero (Arial#SB "Ta"=0 vs R -11.08).
                            fe['krS'] = 1
                elif kf is not None:
                    # (刀100 d) captured and measured EMPTY — chrome has zero
                    # kern on this face; never fall back to krR/kfm (Gill
                    # Sans#S800: chrome 800-sc kern is all-zero).
                    fe['kr0'] = 1
                # (刀100 C4) face-pair gap fill: the k99 face capture carried
                # only ~2/3 of the R pair universe per face (Avenir#I lacked
                # .\" — engine 0 vs chrome -8.4). kernFaceMiss holds measured
                # deltas (face value, 0 = kill) for the missing pairs; krR
                # falls the leftovers back to R (14k of 33k measured ≈R).
                kfm = fd.get('kernFaceMiss', {}).get(rk + '#' + tag)
                if kfm:
                    fe['kfm'] = to_int_table(kfm)
                if entry.get('kr') is not None:
                    fe['krR'] = entry['kr']
                # (刀99) small-caps px-quantization spots: CoreText snaps the
                # synthesized sc glyph advances off the linear em*px model at
                # small integer sizes (fixed ~1/3px pattern, 62 chars); store
                # per-size sparse em overrides from the r4 capture.
                if tag == 'S' or tag in SC_COMP:
                    asp = {}
                    # composite spots live under fam#tag@px keys
                    pfx = rk if tag == 'S' else rk + '#' + tag
                    sizes = sorted({int(k.rsplit('@', 1)[1])
                                    for k in fd['scspot'] if k.startswith(pfx + '@')})
                    for px in sizes:
                        cap = fd['scspot'].get(pfx + '@' + str(px))
                        if not cap:
                            continue
                        fixes = {i: round(cap[i] / px, 6)
                                 for i in range(min(len(cap), len(row)))
                                 if abs(cap[i] - row[i] * px) > 0.005}
                        if fixes:
                            asp[str(px)] = fixes
                    if asp:
                        fe['as'] = asp
                # (刀99 r6e/r6f) small-caps kern quantizes below ~32px like
                # the advances: per-pair band arrays 6..32 (px*100 ints),
                # consumed by kernOf's kbM branch (band length drives the
                # cap). Truth keys are display-case; the res/scspot keys the
                # caller passes are display-case too — normalize both ways.
                if tag == 'S' or tag in SC_COMP:
                    # rk here is the display-case truth name (res/scspot keys);
                    # scKernBands keys are display-case too but normalize both
                    # ways to be safe. Composite bands live under fam#tag keys.
                    scb = ((SCB.get(rk + '#' + tag) or SCB.get((rk + '#' + tag).lower()))
                           if tag in SC_COMP
                           else (SCB.get(rk) or SCB.get(rk.lower())))
                    if scb:
                        fe['krb'] = scb
                # (刀99/刀100) face triples: inherit the R triple table, then
                # ride the measured C3 delta layer on top when captured —
                # k3m overrides beat TMAP, k3k kills R-only triples.
                if entry.get('k3') is not None:
                    fe['k3'] = entry['k3']
                if (rk + '#' + tag) in triFaces:
                    fe['k3f'] = rk + '#' + tag
            if fe:
                fmap[tag] = fe
        if fmap:
            entry['f'] = fmap

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
        # (刀99) faces: B/I/Z/M/S + numeric-weight tables for this font.
        if fd:
            rk = name if fd['res'].get(name) else ('g:' + name if fd['res'].get('g:' + name) else None)
            if rk:
                build_faces(rk, entry)
    # (刀100 d) sc composite alias lattice: runtime variant key -> composite
    # tag. Keys missing from the lattice swallow to the plain S face (that IS
    # chrome's collapse for those variants — the lattice only carries rows
    # whose composite table was captured).
    if fd and fd.get('scLattice'):
        for fam, lat in fd['scLattice'].items():
            nk2 = fam.lower()
            if nk2.startswith('g:'):
                nk2 = nk2[2:]
            if nk2 in names:
                names[nk2]['scL'] = lat

    # generic entries resolve through NAMES at runtime so the resolved object is
    # the single NAMES entry (k98 attaches the system-ui kern maps onto it).
    generic = dict(GENERIC)

    # ---- (刀99) system-ui face ladders: every face is a variable-font ----
    # instance with its OWN optical axis (verified: face-vs-R delta is NOT
    # constant in em), so each carries the full k/xk knot set. Kern maps for
    # the keyword faces come from suKern; numeric-weight kern is deferred
    # (刀100) and approximated by the nearest keyword face at emission.
    SUFACE_KEYS = [('B', 'B'), ('I', 'I'), ('Z', 'Z'), ('M', 'M'), ('S', 'S'),
                   ('w100', '100'), ('w200', '200'), ('w300', '300'),
                   ('w600', '600'), ('w800', '800'), ('w900', '900')]
    if fd and 'system-ui' in names:
        su = names['system-ui']
        su.setdefault('f', {})
        sizes = sorted(fd['suFaces']['B'], key=float)
        for src, key in SUFACE_KEYS:
            lad = fd['suFaces'][src]
            fe = {
                'k': {'s': [float(s) for s in sizes],
                      't': [intern([round(v / float(s), 6) for v in lad[s][:95]]) for s in sizes]},
                'xk': {'s': [float(s) for s in sizes],
                       't': [intern([round(v / float(s), 6) for v in lad[s][95:]]) for s in sizes]},
            }
            vf = fd['vertF'].get('g:system-ui#' + src) or fd['vertF'].get('system-ui#' + src)
            if vf:
                build_vert(fe, vf)
            # (刀100 C3) su faces carry the R triple baseline (they had none
            # — faces were ladder+kern only) plus their own triFaces delta.
            if su.get('k3') is not None:
                fe['k3'] = su['k3']
            if ('system-ui#' + key) in triFaces:
                fe['k3f'] = 'system-ui#' + key
            su['f'][key] = fe

    # ---- (刀99) quoted generics: serif/monospace/cursive/fantasy quoted ----
    # resolve to hidden faces distinct from their unquoted tables.
    GENQ = {}
    if fd:
        for g in GENERIC:
            q = fd['res'].get('q:' + g)
            u = fd['res'].get('g:' + g)
            if not q or not u:
                continue
            if all(q.get(k) == u.get(k) for k in ['R'] + FTAGS):
                continue  # quoted == unquoted (real family name)
            row = fd['tables'][q['R']]
            qe = {'t': intern([round(x, 6) for x in row[:95]]),
                  'x': intern([round(x, 6) for x in row[95:]])}
            # quoted generics share one hidden-face box (r8: all 1060/340
            # except quoted system-ui, which never reaches GENQ).
            qv = fd.get('qVert', {}).get(g)
            if qv:
                build_vert(qe, {'1000': qv['1000'], '16': qv['16'], '100': qv['100'],
                                'spot': [qv['10'], qv['12'], qv['20'], qv['32']]})
            qk = to_int_table(fd.get('qRkern', {}).get(g, {}))
            if qk:
                qe['kr'] = kintern(kcanon, qk)
            fmap = {}
            for tag in FTAGS:
                v = q.get(tag)
                if v is None or v == '=R':
                    continue
                if v == '=DEFBOLD':
                    fmap[tag] = dict(DEFBOLD)
                    continue
                frow = fd['tables'][v]
                fe = {'t': intern([round(x, 6) for x in frow[:95]]),
                      'x': intern([round(x, 6) for x in frow[95:]])}
                vf2 = fd['vertF'].get('q:' + g + '#' + tag)
                if vf2:
                    build_vert(fe, vf2)
                kf = fd['kernFaces'].get('q:' + g + '#' + tag)
                if kf:
                    kt2 = to_int_table(kf)
                    if kt2:
                        fe['kr'] = kintern(kcanon, kt2)
                        if ('q:' + g + '#' + tag) in KFULL:
                            fe['krS'] = 1
                elif kf is not None:
                    fe['kr0'] = 1
                # (刀100 d) quoted-generic sc faces quantize at small sizes
                # like their unquoted kin — the q: scspot rows existed since
                # k99 but were never attached here (latent gap, closed now).
                if tag == 'S' or tag in SC_COMP:
                    pfx = 'q:' + g + ('' if tag == 'S' else '#' + tag)
                    asp = {}
                    sizes = sorted({int(k.rsplit('@', 1)[1])
                                    for k in fd['scspot'] if k.startswith(pfx + '@')})
                    for px in sizes:
                        cap = fd['scspot'].get(pfx + '@' + str(px))
                        if not cap:
                            continue
                        fixes = {i: round(cap[i] / px, 6)
                                 for i in range(min(len(cap), len(frow)))
                                 if abs(cap[i] - frow[i] * px) > 0.005}
                        if fixes:
                            asp[str(px)] = fixes
                    if asp:
                        fe['as'] = asp
                    scb = SCB.get(pfx) or SCB.get(pfx.lower())
                    if scb:
                        fe['krb'] = scb
                fmap[tag] = fe
            if fmap:
                qe['f'] = fmap
            qlat = (fd.get('scLattice') or {}).get('q:' + g)
            if qlat:
                qe['scL'] = qlat
            GENQ[g] = qe

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
        lines.append('  // (per-integer band overrides, px*100) — 6..23 where GPOS device')
        lines.append('  // tables oscillate; 刀100 extends qualifying pairs to 6..32 (optical')
        lines.append('  // curve past the knots) and the —— decay pair to 6..80 (chrome')
        lines.append('  // relaxes it to v-linear by ~80px, v itself 0).')
        lines.append('  const KERN = %s;' % json.dumps(kcanon, separators=(',', ':')))
        lines.append('  const numKey = (s) => s.charCodeAt(0) * 65536 + s.charCodeAt(1);')
        lines.append('  const KMAP = KERN.map((o) => { const m = new Map(); for (const k in o) m.set(numKey(k), o[k]); return m; });')
    if tcanon:
        lines.append('  // (k98) triple exceptions (ligature chains, script joins): px@100 ints.')
        lines.append('  const TRI = %s;' % json.dumps(tcanon, separators=(',', ':')))
        lines.append('  const TMAP = TRI.map((o) => { const m = new Map(); for (const k in o) m.set(k.charCodeAt(0) * 4294967296 + k.charCodeAt(1) * 65536 + k.charCodeAt(2), o[k]); return m; });')
        # (刀100 c5b ladder) gate refined: ONLY U+202F at window first/last is
        # standalone-only (AN_l_11/AT_l_10/AT_f_c1/CB_f_q2/HT_f_74 all gate on
        # every flank; string-end is irrelevant). U+202F mid fires (’⁯A rows),
        # and the whole class-N universe (U+00A0, U+2000..200A, U+2028/2029 —
        # identical v per family, one glyph-substitution path) fires at every
        # position. No WS3 set needed.
    if trif:
        lines.append('  // (刀100 C3) face-triple deltas: TRIF[font#tag] = {m: measured')
        lines.append('  // px@100 overrides (x100 ints, beat TMAP), k: R triples the face')
        lines.append('  // kills}. k3f strings on face entries resolve into k3m/k3k Maps.')
        lines.append('  const TRIF = %s;' % json.dumps(trif, separators=(',', ':')))
        lines.append('  const t3key = (s) => s.charCodeAt(0) * 4294967296 + s.charCodeAt(1) * 65536 + s.charCodeAt(2);')
        lines.append('  const TRIFM = new Map();')
        lines.append('  for (const key in TRIF) { const o = TRIF[key];')
        lines.append("    TRIFM.set(key, { m: o.m ? new Map(Object.entries(o.m).map(([t, v]) => [t3key(t), v])) : null,")
        lines.append('      k: o.k ? new Set(o.k.map(t3key)) : null,')
        lines.append('      g: o.g ? new Map(Object.entries(o.g).map(([t, v]) => [t3key(t), v])) : null });')
        lines.append('  }')
        lines.append('  for (const nk in NAMES) { const fmap = NAMES[nk].f;')
        lines.append('    if (!fmap) continue;')
        lines.append('    for (const t in fmap) { const e = fmap[t];')
        lines.append('      if (e.k3f !== undefined && TRIFM.has(e.k3f)) {')
        lines.append('        const o = TRIFM.get(e.k3f);')
        lines.append('        if (o.m) e.k3m = o.m;')
        lines.append('        if (o.k) e.k3k = o.k;')
        lines.append('        if (o.g) e.k3g = o.g;')
        lines.append('      }')
        lines.append('    }')
        lines.append('  }')
    if triMid:
        lines.append('  // (刀100 c5) triple mid-value layer: MIDF[fam] = {window: px@100')
        lines.append('  // int} replaces the RESOLVED triple value (face override, kill or R')
        lines.append('  // inherit alike) whenever the text is longer than the 3-char window.')
        lines.append('  // Face-level k3g beats the family map; killed windows stay dead.')
        midf = {fam.lower(): to_int_table(wins) for fam, wins in triMid.items()}
        lines.append('  const MIDF = %s;' % json.dumps(midf, separators=(',', ':')))
        lines.append('  const MIDM = new Map();')
        lines.append('  for (const fam in MIDF) MIDM.set(fam, new Map(Object.entries(MIDF[fam]).map(([t, v]) => [t.charCodeAt(0) * 4294967296 + t.charCodeAt(1) * 65536 + t.charCodeAt(2), v])));')
        lines.append('  for (const nk in NAMES) { const mm = MIDM.get(nk); if (mm) {')
        lines.append('    NAMES[nk].midM = mm;')
        lines.append('    const fm2 = NAMES[nk].f;')
        lines.append('    if (fm2) for (const t in fm2) { if (!fm2[t].midM) fm2[t].midM = mm; }')
        lines.append('  } }')
    if kd.get('su'):
        lines.append('  const SU = %s;' % json.dumps(kd['su'], separators=(',', ':')))
        lines.append('  NAMES["system-ui"].kkM = new Map(Object.entries(SU.kk).map(([k, v]) => [numKey(k), v]));')
        lines.append('  NAMES["system-ui"].krbM = new Map(Object.entries(SU.kb).map(([k, v]) => [numKey(k), v]));')
    if fd and fd.get('suKern'):
        SUK_TAGS = ['B', 'I', 'Z', 'M', 'S', '600', '800', '900']
        suk = {tag: fd['suKern'][tag] for tag in SUK_TAGS if tag in fd['suKern']}
        lines.append('  // (刀99) system-ui FACE kern: kk knots [em@16/20/24 in 1e-4em], v100')
        lines.append('  // linear anchors, kb = SPARSE band overrides (px*100 at integer sizes')
        lines.append('  // 6..23 GPOS oscillation, 刀100-gated 24..80 optical/decay legs, only')
        lines.append('  // where chrome leaves the kk ladder). Numeric-weight faces')
        lines.append('  // 600/800/900 carry their OWN captured kern (r6d p3); the borrow')
        lines.append("  // lines are fallbacks only for sections missing from the truth.")
        lines.append('  const SUF = %s;' % json.dumps(suk, separators=(',', ':')))
        lines.append("  const suF = NAMES['system-ui'].f;")
        lines.append("  for (const t of %s) {" % json.dumps(SUK_TAGS))
        lines.append('    if (!SUF[t]) continue;')
        lines.append('    const e = suF[t]; if (!e) continue;')
        lines.append('    e.kkM = new Map(Object.entries(SUF[t].kk).map(([k, v]) => [numKey(k), v]));')
        lines.append('    e.vM = new Map(Object.entries(SUF[t].v100).map(([k, v]) => [numKey(k), v]));')
        lines.append('    e.krbM = new Map(Object.entries(SUF[t].kb).map(([k, o]) => [numKey(k), new Map(Object.entries(o).map(([px, v]) => [+px, v]))]));')
        lines.append('  }')
        lines.append("  for (const w of ['100', '200', '300']) { const e = suF[w]; if (e && !e.kkM) { e.kr = NAMES['system-ui'].kr; e.kkM = NAMES['system-ui'].kkM; e.krbM = NAMES['system-ui'].krbM; } }")
        lines.append("  for (const w of ['600', '800', '900']) { const e = suF[w]; if (e && !e.kkM && suF.B) { e.kkM = suF.B.kkM; e.vM = suF.B.vM; e.krbM = suF.B.krbM; } }")
    if fd and fd.get('kernFaceMiss'):
        # kfm dicts ride the face entries as JSON; resolve to Maps once.
        lines.append('  // (刀100 C4) face-pair gap fill: kfmM beats the face table miss,')
        lines.append('  // krR (family R kern index) catches the ≈R leftovers.')
        lines.append('  for (const nk in NAMES) { const fmap = NAMES[nk].f;')
        lines.append('    if (!fmap) continue;')
        lines.append('    for (const t in fmap) { const e = fmap[t];')
        lines.append('      if (e.kfm) { e.kfmM = new Map(Object.entries(e.kfm).map(([p, v]) => [numKey(p), v])); e.kfm = undefined; }')
        lines.append('    }')
        lines.append('  }')
    if fd and fd.get('scKernBands'):
        # sc kern bands: pair -> [px*100 at 6..23], same array shape kernOf's
        # kbM branch already consumes for the base system-ui bands.
        lines.append('  for (const k of Object.keys(NAMES)) { const fm3 = NAMES[k].f;')
        lines.append('    if (!fm3) continue;')
        lines.append('    for (const t in fm3) { const e = fm3[t];')
        lines.append("      if (e.krb) e.krbM = new Map(Object.entries(e.krb).map(([p, v]) => [numKey(p), v])); } }")
    if GENQ:
        lines.append('  // (刀99) quoted generics resolve to hidden faces with their own')
        lines.append('  // tables (quoted serif/monospace/cursive/fantasy differ from the')
        lines.append('  // unquoted resolution; quoted real family names fall through to')
        lines.append('  // lookup() and match the unquoted table).')
        lines.append('  const GENQ = %s;' % json.dumps(GENQ, separators=(',', ':')))
    if kcanon:
        lines.append('  // kern of an adjacent pair (px at the given size); band overrides win')
        lines.append('  // below 24, optical knots blend 16..28, >=28 is exact linear.')
        lines.append('  const kernOf = (m, kkM, kbM, a, b, px, fm, fr) => {')
        lines.append('    const key = a * 65536 + b;')
        lines.append('    let v = m.get(key);')
        lines.append('    if (v === undefined && fm) v = fm.get(key);')
        lines.append('    if (v === undefined && fr) v = fr.get(key);')
        lines.append('    if (v === undefined) return 0;')
        lines.append('    if (kbM) {')
        lines.append('      const B = kbM.get(key);')
        lines.append('      // half-grid bands (刀100): columns at 0.5px steps from 6 —')
        lines.append('      // 37-wide = 6..23.5 (the >=24 kk ladder models the rest),')
        lines.append('      // 54-wide = 6..32.5 (optical curve past the knots; the sc')
        lines.append('      // quantization bands), 148-wide = 6..79.5 decay pairs (—— only:')
        lines.append('      // chrome relaxes it to v-linear by ~80px, v itself 0. Exit')
        lines.append('      // blends into the ladder at the half-step past the last column.')
        lines.append('      if (B !== undefined && px < 6 + B.length / 2) {')
        lines.append('        if (px <= 6) return B[0] * px / 600;')
        lines.append('        const t = (px - 6) * 2, lo = Math.floor(t);')
        lines.append('        if (lo >= B.length - 1) {')
        lines.append('          const K = kkM ? kkM.get(key) : undefined;')
        lines.append('          const lastPx = 6 + (B.length - 1) / 2;')
        lines.append('          const kB = B[B.length - 1] / 100;')
        lines.append('          // exit target = the kk ladder value one half-step past the')
        lines.append('          // band so the handover is continuous: <28 keeps the K[2]')
        lines.append('          // knot anchor, >=28 is the v-linear regime.')
        lines.append('          const pN = 6 + B.length / 2;')
        lines.append('          const kN = !K || pN >= 28 ? v * pN / 10000')
        lines.append('            : (K[2] + (v - K[2]) * (pN - 24) / 4) * pN / 10000;')
        lines.append('          return kB + (kN - kB) * (px - lastPx);')
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
        if fd and fd.get('suKern'):
            lines.append('  // (刀99) su FACE kern: no fixed kr table — the linear anchor comes')
            lines.append('  // from vM (px@100 ints); kbM is SPARSE ({pair: {px: px*100}}), an')
            lines.append('  // anchor hit wins outright. 刀100 recaptures the anchors on the')
            lines.append('// half grid (0.5px keys 6..32.5, gated vs the kk ladder, plus the')
            lines.append('  // —— decay legs to 79.5); a miss falls to the kk model.')
            lines.append('  const kernFace = (kkM, vM, kbM, a, b, px) => {')
            lines.append('    const key = a * 65536 + b;')
            lines.append('    const K = kkM.get(key);')
            lines.append('    if (K === undefined) return 0;')
            lines.append('    if (px < 81 && kbM) {')
            lines.append('      const O = kbM.get(key);')
            lines.append('      if (O) {')
            lines.append('        // half-grid anchors: g = px*2 so 0.5 keys are integral; a hit at')
            lines.append('        // floor half-step wins, fractional interpolates to the next.')
            lines.append('        const g = px * 2, lo = Math.floor(g);')
            lines.append('        let A = O.get(lo / 2), B2 = O.get((lo + 1) / 2), t = g - lo;')
            lines.append('        if (A === undefined && t === 0 && lo > 0) {')
            lines.append('          A = O.get((lo - 1) / 2); B2 = O.get((lo + 1) / 2); t = 1;')
            lines.append('        }')
            lines.append('        if (A !== undefined) {')
            lines.append('          if (B2 !== undefined && t > 0) return (A + (B2 - A) * t) / 100;')
            lines.append('          return A / 100;')
            lines.append('        }')
            lines.append('      }')
            lines.append('    }')
            lines.append('    const v = vM.get(key) || 0;')
            lines.append('    if (px >= 28) return v * px / 10000;')
            lines.append('    if (px >= 24) return (K[2] + (v - K[2]) * (px - 24) / 4) * px / 10000;')
            lines.append('    if (px >= 20) return (K[1] + (K[2] - K[1]) * (px - 20) / 4) * px / 10000;')
            lines.append('    if (px >= 16) return (K[0] + (K[1] - K[0]) * (px - 16) / 4) * px / 10000;')
            lines.append('    return K[0] * px / 10000;')
            lines.append('  };')
    if ext_cp:
        lines.append('  // EXT: non-ASCII codepoints with per-font advances (Latin-1 + General')
        lines.append('  // Punctuation + symbols). XI maps codepoint -> row index.')
        lines.append('  const XP = %s;' % json.dumps(ext_cp, separators=(',', ':')))
        lines.append('  const XI = new Map();')
        lines.append('  for (let i = 0; i < XP.length; i++) XI.set(XP[i], i);')
        lines.append('  const xAdv = (tbl, i, px) => {')
        if fd:
            lines.append('    if (tbl.xp && tbl.xp[i] !== undefined) return tbl.xp[i] * px;')
            lines.append("    if (tbl.as) { const A = tbl.as[px]; if (A && A[95 + i] !== undefined) return A[95 + i] * px; }")
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
    if fd:
        lines.append("    if (tbl.as) { const A = tbl.as[px]; if (A && A[i] !== undefined) return A[i] * px; }")
    lines.append('    return ADV[tbl.t][i] * px;')
    lines.append('  };')
    lines.append('  globalThis.__ditingFontMetrics = {')
    lines.append('    lookup(name) { return NAMES[String(name).toLowerCase()] || null; },')
    lines.append('    generic(name) { const g = GENERIC[String(name).toLowerCase()]; return g ? NAMES[g] : null; },')
    if GENQ:
        lines.append('    genq(name) { const q = GENQ[String(name).toLowerCase()]; return q || null; },')
    lines.append('    sum(tbl, text, px) {')
    lines.append('      let w = 0;')
    if kcanon:
        # (刀100 c5g) faces without their own kern table (kr absent — 60
        # numeric-weight/synthetic faces) must fall back to the family R table
        # via krR, not to zero kern: AT#100 dropped chrome's kern("v,")=-7.4.
        lines.append('      const m = tbl.kr === undefined ? (tbl.krR === undefined || tbl.kr0 ? null : KMAP[tbl.krR]) : KMAP[tbl.kr];')
        lines.append('      const kkM = tbl.kkM || null, kbM = tbl.krbM || null;')
        lines.append('      const tm = tbl.k3 === undefined ? null : TMAP[tbl.k3];')
        lines.append('      const k3m = tbl.k3m || null, k3k = tbl.k3k || null, k3g = tbl.k3g || null;')
        lines.append('      const mid3M = tbl.midM || null;')
        lines.append('      const fmM = tbl.kfmM || null;')
        lines.append('      const frM = tbl.krR === undefined ? null : KMAP[tbl.krR];')
        lines.append('      const faceSu = !m && kkM && tbl.vM;')
        lines.append('      let p1 = -1, p2 = -1;')
    lines.append('      for (let i = 0; i < text.length; i++) {')
    lines.append('        let c = text.charCodeAt(i);')
    lines.append('        if (c >= 0xD800 && c <= 0xDBFF && i + 1 < text.length) {')
    lines.append('          const lo = text.charCodeAt(i + 1);')
    lines.append('          if (lo >= 0xDC00 && lo <= 0xDFFF) { c = 0x10000 + ((c - 0xD800) << 10) + (lo - 0xDC00); i++; }')
    lines.append('        }')
    lines.append('        w += adv(tbl, c, px);')
    if kcanon:
        lines.append('        if (p1 >= 0 && m) w += kernOf(m, kkM, kbM, p1, c, px, fmM, tbl.krS ? null : frM);')
        if fd and fd.get('suKern'):
            lines.append('        else if (p1 >= 0 && faceSu) w += kernFace(kkM, tbl.vM, kbM, p1, c, px);')
        lines.append('        if (p2 >= 0 && (tm || k3m || k3k)) { const q = p2 * 4294967296 + p1 * 65536 + c;')
        lines.append('          const ov = k3m ? k3m.get(q) : undefined;')
        lines.append('          let tv = ov !== undefined ? ov')
        lines.append('            : (k3k && k3k.has(q)) ? undefined : (tm ? tm.get(q) : undefined);')
        lines.append('          if (tv !== undefined && text.length !== 3) {')
        lines.append('            const gv = k3g ? k3g.get(q) : undefined;')
        lines.append('            const fv = gv !== undefined ? gv : (mid3M ? mid3M.get(q) : undefined);')
        lines.append('            if (fv !== undefined) tv = fv;')
        lines.append('          }')
        lines.append('          if (tv !== undefined) w += tv * px / 10000;')
        lines.append('        }')
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
          '%d kern tables (%d pairs), %d triple tables (%d), %d su knots, %d su bands, '
          '%d trif combos (%d mods, %d kills)' % (
              os.path.relpath(OUT, ROOT), len(canon), len(names), len(generic), spots_n,
              len(kcanon), sum(len(t) for t in kcanon), len(tcanon), sum(len(t) for t in tcanon),
              len(kd.get('su', {}).get('kk', {})), len(kd.get('su', {}).get('kb', {})),
              len(trif), sum(len(v.get('m', {})) for v in trif.values()),
              sum(len(v.get('k', [])) for v in trif.values())))


if __name__ == '__main__':
    main()
