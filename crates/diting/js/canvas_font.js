(function () { // @@SCOPE
// Canvas 2D font attribute: the `font` shorthand parser and per-context font
// slots, split out of bootstrap.js (god-file ratchet). Everything here is
// declarations; _Canvas2D (bootstrap) reaches it only at call time, when a
// context is constructed or its font is assigned — after this script ran.
// (刀117) Canvas font shorthand parser — Chrome 145 semantics, pinned by the
// k117a–d probe captures. The font attribute becomes a real prototype
// accessor: an invalid assignment is IGNORED (the previous value survives),
// and the getter returns the canonical serial 'italic bold small-caps <px>
// <families>'. Sizes resolve at ASSIGNMENT time: em/% are ×10 of the FIXED
// initial size (never the current font or the element style), ex/ch carry
// the initial font's constants except for a lone unquoted generic monospace
// family, vw/vh snapshot the live viewport, and calc() clamps a negative
// result to 0px while a negative length literal rejects. Line-height parses
// but never serializes; stretch keywords parse and are dropped.
const _dF_KW_SIZE = { 'xx-small': 9, 'x-small': 10, small: 13, medium: 16, large: 18, 'x-large': 24, 'xx-large': 32, 'xxx-large': 48 };
const _dF_STRETCH = { condensed: 1, 'ultra-condensed': 1, 'extra-condensed': 1, 'semi-condensed': 1, 'semi-expanded': 1, expanded: 1, 'extra-expanded': 1, 'ultra-expanded': 1 };
const _dF_GENERICS = { serif: 1, 'sans-serif': 1, monospace: 1, cursive: 1, fantasy: 1, 'system-ui': 1, math: 1, emoji: 1, fangsong: 1, 'ui-serif': 1, 'ui-sans-serif': 1, 'ui-monospace': 1, 'ui-rounded': 1 };
const _dF_CSS_WIDE = { inherit: 1, initial: 1, unset: 1, revert: 1, 'revert-layer': 1 };
const _dF_UNITS = { px: 1, pt: 1, pc: 1, in: 1, cm: 1, mm: 1, q: 1, em: 1, rem: 1, ex: 1, ch: 1, vw: 1, vh: 1, vmin: 1, vmax: 1, svw: 1, svh: 1, dvw: 1, dvh: 1, lvw: 1, lvh: 1, svmin: 1, svmax: 1, dvmin: 1, dvmax: 1, lvmin: 1, lvmax: 1, cqw: 1, cqh: 1, cqi: 1, cqb: 1, cqmin: 1, cqmax: 1 };
// one dot max per mantissa — '700.' leaves a stray '.' (rejects) while
// '1.5.5px' re-lexes '.5px' as a fresh number+unit (weight 1 + size 0.5px)
const _dF_NUM = /[+-]?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][+-]?\d+)?/y;
function _dFEscape(s, j) {
  const n = s.length;
  const d = s[j + 1];
  if (d === undefined) return { ch: '\uFFFD', next: n };
  if (d === '\n') return { ch: '', next: j + 2 };
  let h = '', k = j + 1;
  while (k < n && h.length < 6 && /[0-9a-fA-F]/.test(s[k])) { h += s[k]; k++; }
  if (h) {
    if (s[k] === ' ') k++;
    const cp = parseInt(h, 16);
    if (cp === 0 || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) return { ch: '\uFFFD', next: k };
    return { ch: String.fromCodePoint(cp), next: k };
  }
  return { ch: d, next: j + 2 };
}
function _dFReadIdent(s, i) {
  const n = s.length;
  let j = i, v = '';
  if (s[j] === '-') { v += '-'; j++; if (s[j] === '-') { v += '-'; j++; } }
  if (j >= n) return null;
  {
    const c = s[j];
    if (c === '\\') { const r = _dFEscape(s, j); v += r.ch; j = r.next; }
    else {
      const cc = c.charCodeAt(0);
      if (!(cc >= 97 && cc <= 122 || cc >= 65 && cc <= 90 || cc === 95 || cc >= 0xA0)) return null;
      v += c; j++;
    }
  }
  for (;;) {
    if (j >= n) break;
    const c = s[j];
    if (c === '\\') { const r = _dFEscape(s, j); v += r.ch; j = r.next; continue; }
    const cc = c.charCodeAt(0);
    if (cc >= 97 && cc <= 122 || cc >= 65 && cc <= 90 || cc === 95 || cc >= 0xA0 || (cc >= 48 && cc <= 57) || cc === 45) { v += c; j++; continue; }
    break;
  }
  return { v: v, next: j };
}
function _dFTokenize(s) {
  const T = [];
  const n = s.length;
  let i = 0;
  while (i < n) {
    const c = s[i], cc = s.charCodeAt(i);
    if (c === ' ' || cc === 9 || cc === 10 || cc === 12 || cc === 13) {
      while (i < n) { const d = s.charCodeAt(i); if (d === 32 || d === 9 || d === 10 || d === 12 || d === 13) i++; else break; }
      T.push({ t: 'ws' });
      continue;
    }
    if (c === '/' && s[i + 1] === '*') {
      const e = s.indexOf('*/', i + 2);
      if (e < 0) return null;
      i = e + 2;
      continue;
    }
    if (c === '"' || c === "'") {
      let j = i + 1, out = '';
      for (;;) {
        if (j >= n) break;
        const d = s[j];
        if (d === '\\') { const r = _dFEscape(s, j); if (!r) return null; out += r.ch; j = r.next; continue; }
        if (d === c) { j++; break; }
        if (d === '\n') return null;
        out += d; j++;
      }
      T.push({ t: 'str', v: out });
      i = j;
      continue;
    }
    _dF_NUM.lastIndex = i;
    const m = _dF_NUM.exec(s);
    if (m) {
      const val = parseFloat(m[0]);
      i += m[0].length;
      const id = _dFReadIdent(s, i);
      if (id) { T.push({ t: 'dim', v: val, u: id.v.toLowerCase() }); i = id.next; continue; }
      if (s[i] === '%') { T.push({ t: 'pct', v: val }); i++; continue; }
      T.push({ t: 'num', v: val });
      continue;
    }
    const id = _dFReadIdent(s, i);
    if (id) {
      if (s[id.next] === '(') { T.push({ t: 'func', name: id.v }); i = id.next + 1; continue; }
      T.push({ t: 'ident', v: id.v });
      i = id.next;
      continue;
    }
    T.push({ t: 'delim', v: c });
    i++;
  }
  return T;
}
function _dFCalc(T, start) {
  let i = start + 1;
  const n = T.length;
  function pPrim() {
    while (i < n && T[i].t === 'ws') i++;
    if (i >= n) return null;
    const t = T[i];
    if (t.t === 'delim' && t.v === '(') {
      i++;
      const r = pAdd();
      if (!r) return null;
      while (i < n && T[i].t === 'ws') i++;
      if (i >= n || !(T[i].t === 'delim' && T[i].v === ')')) return null;
      i++;
      return r;
    }
    if (t.t === 'delim' && t.v === '-') { i++; const r = pPrim(); if (!r) return null; return { v: -r.v, u: r.u }; }
    if (t.t === 'delim' && t.v === '+') { i++; return pPrim(); }
    if (t.t === 'num') { i++; return { v: t.v, u: null }; }
    if (t.t === 'dim') { i++; return _dF_UNITS[t.u] ? { v: t.v, u: t.u } : null; }
    if (t.t === 'pct') { i++; return { v: t.v * 0.1, u: 'px' }; }
    return null;
  }
  function pMul() {
    let a = pPrim();
    if (!a) return null;
    for (;;) {
      while (i < n && T[i].t === 'ws') i++;
      if (i < n && T[i].t === 'delim' && (T[i].v === '*' || T[i].v === '/')) {
        const op = T[i].v;
        i++;
        const b = pPrim();
        if (!b) return null;
        if (op === '*') {
          if (a.u !== null && b.u !== null) return null;
          a = { v: a.v * b.v, u: a.u === null ? b.u : a.u };
        } else {
          if (b.u !== null || b.v === 0) return null;
          a = { v: a.v / b.v, u: a.u };
        }
      } else return a;
    }
  }
  function pAdd() {
    let a = pMul();
    if (!a) return null;
    for (;;) {
      while (i < n && T[i].t === 'ws') i++;
      if (i < n && T[i].t === 'delim' && (T[i].v === '+' || T[i].v === '-')) {
        const op = T[i].v;
        i++;
        const b = pMul();
        if (!b) return null;
        if (a.u !== b.u) return null;
        a = { v: op === '+' ? a.v + b.v : a.v - b.v, u: a.u };
      } else return a;
    }
  }
  const r = pAdd();
  if (!r) return null;
  while (i < n && T[i].t === 'ws') i++;
  if (i >= n || !(T[i].t === 'delim' && T[i].v === ')')) return null;
  i++;
  if (r.u === null) return null;
  return { v: r.v, u: r.u, next: i };
}
function _dFUnitPx(u, mono) {
  switch (u) {
    case 'px': return 1;
    case 'pt': return 4 / 3;
    case 'pc': return 16;
    case 'in': return 96;
    case 'cm': return 96 / 2.54;
    case 'mm': return 96 / 25.4;
    case 'q': return 96 / 101.6;
    case 'em': return 10;
    case 'rem': return 10;
    // (k117a/b) 0.5ex serializes 6.73096px and 3ex 13.4619px — the constant
    // sits in [4.4873033, 4.48731); 4.487305 satisfies both roundings. ch is
    // the exact '0' advance; the mono-generic constants are Menlo's.
    case 'ex': return mono ? 3.64593 : 4.487305;
    case 'ch': return mono ? 4.0625 : 5;
  }
  const W = globalThis.innerWidth || 1920, H = globalThis.innerHeight || 1000;
  const mn = Math.min(W, H), mx = Math.max(W, H);
  switch (u) {
    case 'vw': case 'svw': case 'dvw': case 'lvw': case 'cqw': return W / 100;
    case 'vh': case 'svh': case 'dvh': case 'lvh': case 'cqh': return H / 100;
    case 'vmin': case 'svmin': case 'dvmin': case 'lvmin': case 'cqi': case 'cqb': case 'cqmin': return mn / 100;
    case 'vmax': case 'svmax': case 'dvmax': case 'lvmax': case 'cqmax': return mx / 100;
  }
  return NaN;
}
function _dFSerialPx(v) {
  if (v === 0) return '0';
  // k117: the computed size is stored as float32 — serialize toPrecision(6)
  // of Math.fround(v) (6.7199949px serializes as "6.72px" because
  // f32 snaps it to 6.71999502, past the 6-sig half). The exponential form
  // is kept verbatim (1e-7px -> "1.00000e-7px"); only the decimal branch
  // goes through Number -> String trimming ("100.000" -> "100").
  const tp = Math.fround(v).toPrecision(6);
  return tp.indexOf('e') >= 0 ? tp : String(Number(tp));
}
// k117: the RENDER size is a second, independent quantization — round to 6
// DECIMALS, then truncate the decimal string to 2 places (6.7199990 renders
// at 6.71 while 6.7199999 renders at 6.72; 9999.999 renders at 9999.99 even
// though the serial says 10000px; 0.145 -> 0.14; anything < 0.0000005 -> 0).
function _dFRenderPx(v) {
  if (v === 0) return 0;
  let s = String(Number(Math.fround(v).toFixed(6)));
  const d = s.indexOf('.');
  if (d >= 0) s = s.slice(0, d + 3);
  return Number(s);
}
function _dFValidIdentName(n) {
  if (n === '') return false;
  for (let k = 0; k < n.length; k++) {
    const cc = n.charCodeAt(k);
    const ok = cc >= 97 && cc <= 122 || cc >= 65 && cc <= 90 || cc === 95 || cc >= 0xA0 || (k > 0 && cc >= 48 && cc <= 57) || cc === 45;
    if (!ok) return false;
  }
  return !(n.charCodeAt(0) >= 48 && n.charCodeAt(0) <= 57);
}
function _dFSerialQuote(f) {
  // a quoted generic stays quoted (it names the literal face, not the
  // generic); multi-ident names, digit starts, '--' customs and anything
  // with non-ident characters quote up
  if (f.n === '') return true;
  if (!_dFValidIdentName(f.n)) return true;
  if (f.n.indexOf('--') === 0) return true;
  if (f.q && _dF_GENERICS[f.n.toLowerCase()]) return true;
  return false;
}
function _ditingParseFont(input) {
  let s;
  try { s = String(input); } catch (e) { return null; }
  const T = _dFTokenize(s);
  if (!T) return null;
  const n = T.length;
  let i = 0;
  let style = null, weight = null, weightNum = false;
  let styleSet = false, weightSet = false, variantSet = false, variant = false;
  let st = null;
  while (i < n) {
    const t = T[i];
    if (t.t === 'ws') { i++; continue; }
    if (t.t === 'ident') {
      const k = t.v.toLowerCase();
      if (k === 'normal') { i++; continue; }
      if (k === 'italic' || k === 'oblique') {
        if (styleSet) return null;
        styleSet = true;
        style = 'italic';
        i++;
        if (k === 'oblique') {
          let j = i;
          while (j < n && T[j].t === 'ws') j++;
          if (j < n && T[j].t === 'dim' && (T[j].u === 'deg' || T[j].u === 'grad' || T[j].u === 'rad' || T[j].u === 'turn')) {
            // the grad leg goes through radians — 100grad lands at
            // 90.00000000000001 and rejects, 0.25turn stays exactly 90
            const d = T[j].u === 'deg' ? T[j].v : T[j].u === 'grad' ? (T[j].v * (Math.PI / 200)) * (180 / Math.PI) : T[j].u === 'rad' ? T[j].v * 180 / Math.PI : T[j].v * 360;
            if (!(Math.abs(d) <= 90)) return null;
            style = 'angle';
            i = j + 1;
          }
        }
        continue;
      }
      if (k === 'small-caps') { if (variantSet) return null; variantSet = true; variant = true; i++; continue; }
      if (k === 'bold' || k === 'bolder') { if (weightSet) return null; weightSet = true; weight = 700; i++; continue; }
      if (k === 'lighter') { if (weightSet) return null; weightSet = true; weight = 100; i++; continue; }
      if (_dF_STRETCH[k]) { i++; continue; }
      if (k === 'larger' || k === 'smaller') { st = { k: 'rel', v: k }; i++; break; }
      if (Object.prototype.hasOwnProperty.call(_dF_KW_SIZE, k)) { st = { k: 'kw', v: _dF_KW_SIZE[k] }; i++; break; }
      return null;
    }
    if (t.t === 'num') {
      if (weightSet) return null;
      if (!(t.v >= 1 && t.v <= 1000)) return null;
      weightSet = true;
      weight = t.v;
      weightNum = true;
      i++;
      continue;
    }
    if (t.t === 'dim') {
      if (!_dF_UNITS[t.u]) return null;
      st = { k: 'dim', v: t.v, u: t.u };
      i++;
      break;
    }
    if (t.t === 'pct') { st = { k: 'pct', v: t.v }; i++; break; }
    if (t.t === 'func') {
      if (t.name.toLowerCase() !== 'calc') return null;
      const r = _dFCalc(T, i);
      if (!r) return null;
      st = { k: 'calc', v: r.v, u: r.u };
      i = r.next;
      break;
    }
    return null;
  }
  if (!st) return null;
  // optional line-height — accepted, validated, never serialized
  while (i < n && T[i].t === 'ws') i++;
  if (i < n && T[i].t === 'delim' && T[i].v === '/') {
    i++;
    while (i < n && T[i].t === 'ws') i++;
    const lt = T[i];
    if (!lt) return null;
    if (lt.t === 'ident') { if (lt.v.toLowerCase() !== 'normal') return null; i++; }
    else if (lt.t === 'num') { if (!(lt.v >= 0)) return null; i++; }
    else if (lt.t === 'pct') { if (!(lt.v >= 0)) return null; i++; }
    else if (lt.t === 'dim') { if (!_dF_UNITS[lt.u] || !(lt.v >= 0)) return null; i++; }
    else return null;
    while (i < n && T[i].t === 'ws') i++;
  }
  if (i >= n) return null;
  // families: ident sequences joined with single spaces, or quoted strings;
  // comma-separated; CSS-wide keywords are not family names
  const fams = [];
  for (;;) {
    const t = T[i];
    if (t.t === 'ident') {
      if (fams.length === 0 && _dF_CSS_WIDE[t.v.toLowerCase()]) return null;
      const parts = [t.v];
      i++;
      for (;;) {
        let j = i;
        while (j < n && T[j].t === 'ws') j++;
        if (j < n && T[j].t === 'ident') { parts.push(T[j].v); i = j + 1; }
        else { i = j; break; }
      }
      fams.push({ n: parts.join(' '), q: false });
    } else if (t.t === 'str') {
      fams.push({ n: t.v, q: true });
      i++;
      let j = i;
      while (j < n && T[j].t === 'ws') j++;
      if (j < n && !(T[j].t === 'delim' && T[j].v === ',')) return null;
      i = j;
    } else return null;
    if (i >= n) break;
    if (!(T[i].t === 'delim' && T[i].v === ',')) return null;
    i++;
    while (i < n && T[i].t === 'ws') i++;
    if (i >= n) return null;
    if (T[i].t === 'delim' && T[i].v === ',') return null;
  }
  // px resolves at assignment time; ex/ch read the mono constants only for
  // a lone unquoted generic monospace family (k117d: 'monospace, Arial'
  // keeps the default refs)
  const monoUnit = fams.length === 1 && !fams[0].q && fams[0].n.toLowerCase() === 'monospace';
  let px;
  if (st.k === 'dim' || st.k === 'calc') px = st.v * _dFUnitPx(st.u, monoUnit);
  else if (st.k === 'pct') px = st.v * 0.1;
  else if (st.k === 'kw') px = st.v;
  else px = st.v === 'larger' ? 12 : 10 * 10 / 12;
  if (!isFinite(px)) return null;
  if (px < 0) { if (st.k === 'calc') px = 0; else return null; }
  // k117: computed font size clamps at 10000px (12345678px/1e21px all
  // collapse to serial "10000px" and render identically).
  if (px > 10000) px = 10000;
  const wi = weight === null ? 0 : Math.trunc(weight);
  const parts = [];
  if (style === 'italic') parts.push('italic');
  if (weight !== null) {
    // k117: the keyword map (700→bold, 400 dropped) fires only on the EXACT
    // float — 700.5 serializes its truncated int '700', never 'bold'
    // (k117k: '700.5 0.5px Arial' → '700 0.5px Arial').
    if (weight === 700) parts.push('bold');
    else if (weight !== 400) parts.push(String(wi));
  }
  if (variant) parts.push('small-caps');
  parts.push(_dFSerialPx(px) + 'px');
  parts.push(fams.map((f) => (_dFSerialQuote(f) ? '"' + f.n + '"' : f.n)).join(', '));
  return { serial: parts.join(' '), px: _dFRenderPx(px), weight: wi, weightNum: weightNum, italic: style === 'italic', smallCaps: variant, fams: fams };
}
// per-context spec slots — no own property ever appears on the context
const _dFontSlots = new WeakMap();
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_dFontSlots', _dFontSlots);
  d('_ditingParseFont', _ditingParseFont);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
