//! module: fontfaceset
//! layer: L2
//! exposes: FontFace
//! requires-load: legacy:EventTarget, legacy:Document
//! requires-call: legacy:DOMException

// document.fonts / FontFaceSet (M4 knife 52). The engine's `document.fonts` was a plain ARRAY of fake FontFace-tagged objects with
// `forEach/has/check/...` grafted on as own properties (so `Array.isArray(document.fonts)` was true, `size` undefined, `check()`
// answered false for unknown families), and the stub interfaces FontFaceSet / CSSPseudoElement stood on `window`. Chrome 145: a real
// FontFaceSet (EventTarget, set of FontFace, insertion-ordered, `size`), whose interface object is NOT exposed on window, empty by
// default (only faces a page adds or an @font-face declares), `check()` true for any family no registered face claims (and the font
// shorthand must parse: SyntaxError "Could not resolve '...' as a font."), `load()` a promise of the matching faces.
// Truth: chrome/fonts-truth-145.json (fonts-probe.js, 19 keys, diffed by fonts_matches_chrome_truth).

// ---------------------------------------------------------------- FontFace
const NAMED_STRETCH = /^(ultra-condensed|extra-condensed|condensed|semi-condensed|normal|semi-expanded|expanded|extra-expanded|ultra-expanded)$/i;
const PCT = /^(?:\d+\.?\d*|\.\d+)%$/;
const DEFAULTS = { style: 'normal', weight: 'normal', stretch: 'normal', unicodeRange: 'U+0-10FFFF', variant: 'normal', featureSettings: 'normal',
  display: 'auto', ascentOverride: 'normal', descentOverride: 'normal', lineGapOverride: 'normal', sizeAdjust: '100%', variationSettings: 'normal' };
// per-descriptor validation: returns the canonical string or null when the value does not parse
const VALID = {
  style: (v) => { const t = v.trim(); return /^(normal|italic)$/i.test(t) || /^oblique(\s+-?\d+\.?\d*deg)?$/i.test(t) ? t : null; },
  weight: (v) => { const t = v.trim(); return /^(normal|bold|bolder|lighter)$/i.test(t) || (/^\d+(\.\d+)?$/.test(t) && +t >= 1 && +t <= 1000) ? t : null; },
  stretch: (v) => { const t = v.trim(); return NAMED_STRETCH.test(t) || PCT.test(t) ? t : null; },
  display: (v) => { const t = v.trim(); return /^(auto|block|swap|fallback|optional)$/i.test(t) ? t : null; },
  unicodeRange: (v) => {
    const parts = v.split(',').map((x) => x.trim());
    const out = [];
    for (const p of parts) {
      const m = /^u\+([0-9a-f?]{1,6})(?:-([0-9a-f]{1,6}))?$/i.exec(p);
      if (!m) return null;
      out.push('U+' + m[1].replace(/^0+(?=.)/, '').toUpperCase() + (m[2] ? '-' + m[2].replace(/^0+(?=.)/, '').toUpperCase() : ''));
    }
    return out.join(', ');
  },
  variant: (v) => (v.trim() !== '' ? v.trim() : null),
  featureSettings: (v) => (v.trim() !== '' ? v.trim() : null),
  variationSettings: (v) => (v.trim() !== '' ? v.trim() : null),
  ascentOverride: (v) => { const t = v.trim(); return /^normal$/i.test(t) || PCT.test(t) ? t : null; },
  descentOverride: (v) => { const t = v.trim(); return /^normal$/i.test(t) || PCT.test(t) ? t : null; },
  lineGapOverride: (v) => { const t = v.trim(); return /^normal$/i.test(t) || PCT.test(t) ? t : null; },
  sizeAdjust: (v) => { const t = v.trim(); return PCT.test(t) ? t : null; },
};
const face = new WeakMap(); // FontFace -> { family, desc:{}, status, valid, promise, resolve, reject, loadP }
const fstate = (o, fn) => { const s = o !== null && typeof o === 'object' ? face.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; };
function sourceOk(src) {
  if (typeof src !== 'string') return true; // ArrayBuffer / view
  const toks = []; let cur = '', depth = 0, q = null;
  for (const ch of src) {
    if (q) { cur += ch; if (ch === q) q = null; continue; }
    if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
    if (ch === '(') depth++; else if (ch === ')') depth--;
    if (ch === ',' && depth === 0) { toks.push(cur.trim()); cur = ''; continue; }
    cur += ch;
  }
  toks.push(cur.trim());
  return depth === 0 && toks.length > 0 && toks.every((t) => /^(url|local)\([^]*\)(\s+format\([^]*\))?(\s+tech\([^]*\))?$/i.test(t));
}
function FontFace(family, source, descriptors) {
  if (new.target === undefined) P.fail(FontFace, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'FontFace'");
  P.args("Failed to construct 'FontFace'", arguments.length, 2, FontFace);
  const pfx = "Failed to construct 'FontFace'";
  const fam = P.conv.DOMString(family, pfx, FontFace);
  let src = source;
  if (typeof src !== 'string' && !(src !== null && typeof src === 'object' && (src instanceof ArrayBuffer || ArrayBuffer.isView(src)))) src = String(src);
  if (descriptors !== undefined && descriptors !== null && typeof descriptors !== 'object') P.fail(FontFace, "The provided value is not of type 'FontFaceDescriptors'.", pfx);
  const s = { family: fam, desc: Object.assign({}, DEFAULTS), status: 'unloaded', loadP: null };
  let ok = sourceOk(src);
  const d = descriptors || {};
  for (const k of Object.keys(DEFAULTS)) {
    if (d[k] === undefined) continue;
    const v = VALID[k](String(d[k]));
    if (v === null) ok = false; else s.desc[k] = v;
  }
  s.loaded = new Promise((res, rej) => { s.resolve = res; s.reject = rej; });
  s.loaded.catch(() => {});
  if (!ok) { s.status = 'error'; s.reject(new DOMException('A network error occurred.', 'NetworkError')); }
  const o = Object.create(new.target.prototype);
  face.set(o, s);
  return o;
}
const FFP = P.iface(FontFace, 'FontFace', 2);
Object.defineProperty(FontFace, 'length', { value: 2, writable: false, enumerable: false, configurable: true });
const setErr = (prop, v) => { throw new DOMException("Failed to set the '" + prop + "' property on 'FontFace': Failed to set '" + v + "' as a property value.", 'SyntaxError'); };
P.accessor(FFP, 'family', function __f() { return fstate(this, __f).family; }, function __f(v) { fstate(this, __f).family = P.conv.DOMString(v, "Failed to set the 'family' property on 'FontFace'", __f); });
for (const k of ['style', 'weight', 'stretch', 'unicodeRange', 'variant', 'featureSettings', 'display', 'ascentOverride', 'descentOverride', 'lineGapOverride', 'sizeAdjust']) {
  P.accessor(FFP, k, function __f() { return fstate(this, __f).desc[k]; }, function __f(v) {
    const s = fstate(this, __f); const str = P.conv.DOMString(v, "Failed to set the '" + k + "' property on 'FontFace'", __f);
    const c = VALID[k](str); if (c === null) setErr(k, str, __f); s.desc[k] = c;
  });
}
P.getter(FFP, 'status', function __f() { return fstate(this, __f).status; });
P.getter(FFP, 'loaded', function __f() { try { return fstate(this, __f).loaded; } catch (e) { return Promise.reject(e); } });
P.method(FFP, 'load', 0, function __f() {
  try {
    const s = fstate(this, __f);
    if (s.loadP === null) {
      if (s.status === 'unloaded') { s.status = 'loaded'; s.resolve(this); }
      s.loadP = s.loaded;
    }
    return s.loadP;
  } catch (e) { return Promise.reject(e); }
});
P.accessor(FFP, 'variationSettings', function __f() { return fstate(this, __f).desc.variationSettings; }, function __f(v) {
  const s = fstate(this, __f); const str = P.conv.DOMString(v, "Failed to set the 'variationSettings' property on 'FontFace'", __f);
  const c = VALID.variationSettings(str); if (c === null) setErr('variationSettings', str, __f); s.desc.variationSettings = c;
});
P.seal(FontFace);
P.expose('FontFace', FontFace);

// ---------------------------------------------------------------- FontFaceSet
function FontFaceSet() { P.fail(FontFaceSet, 'Illegal constructor', "Failed to construct 'FontFaceSet'"); }
const FP = P.iface(FontFaceSet, 'FontFaceSet', 0, EventTarget);
const state = new WeakMap(); // set -> { faces:Set, handlers, ready }
const st = (o, fn) => { const s = o !== null && typeof o === 'object' ? state.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; };
const pre = (m) => "Failed to execute '" + m + "' on 'FontFaceSet'";
const isFace = (o) => o !== null && typeof o === 'object' && face.has(o);

// ---- the CSS `font` shorthand, just enough to name the families (and to reject what Chrome rejects)
const STYLE_KW = /^(normal|italic|oblique|small-caps|bold|bolder|lighter|[1-9]00|ultra-condensed|extra-condensed|condensed|semi-condensed|semi-expanded|expanded|extra-expanded|ultra-expanded)$/i;
const SIZE_KW = /^(xx-small|x-small|small|medium|large|x-large|xx-large|xxx-large|smaller|larger)$/i;
const SIZE_NUM = /^[+]?(?:\d+\.?\d*|\.\d+)(?:px|em|rem|%|pt|pc|cm|mm|in|q|ex|ch|vw|vh|vmin|vmax)$/i;
function tokens(s) {
  const out = []; let cur = '', q = null;
  for (const ch of s) {
    if (q) { cur += ch; if (ch === q) q = null; continue; }
    if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
    if (/\s/.test(ch)) { if (cur) { out.push(cur); cur = ''; } continue; }
    cur += ch;
  }
  if (cur) out.push(cur);
  return q ? null : out;
}
function families(font) {
  const toks = tokens(String(font).trim());
  if (!toks || !toks.length) return null;
  let i = 0;
  while (i < toks.length && STYLE_KW.test(toks[i]) && !(/^[1-9]00$/.test(toks[i]) && false)) i++;
  if (i >= toks.length) return null;
  const size = toks[i].split('/')[0];
  if (!(SIZE_NUM.test(size) || SIZE_KW.test(size))) return null;
  i++;
  if (i < toks.length && toks[i][0] === '/') i++;
  const famText = toks.slice(i).join(' ');
  if (!famText) return null;
  const fams = famText.split(',').map((f) => f.trim().replace(/^["']|["']$/g, '')).filter((f) => f !== '');
  return fams.length ? fams : null;
}
const resolve = (font, method, fn) => {
  const f = families(font);
  if (f === null) throw new DOMException(pre(method) + ": Could not resolve '" + String(font) + "' as a font.", 'SyntaxError');
  return f;
};
const matching = (set, fams) => { const out = []; for (const f of set.faces) if (fams.some((n) => n.toLowerCase() === String(face.get(f).family).replace(/^["']|["']$/g, '').toLowerCase())) out.push(f); return out; };

// ---- members, in Chrome's order
for (const t of ['loading', 'loadingdone', 'loadingerror']) {
  P.accessor(FP, 'on' + t, function __f() { const h = st(this, __f).handlers[t]; return h === undefined ? null : h; }, function __f(v) { st(this, __f).handlers[t] = typeof v === 'function' ? v : null; });
}
P.getter(FP, 'ready', function __f() { try { return st(this, __f).ready; } catch (e) { return Promise.reject(e); } });
P.getter(FP, 'status', function __f() { const s = st(this, __f); for (const f of s.faces) if (face.get(f).status === 'loading') return 'loading'; return 'loaded'; });
P.getter(FP, 'size', function __f() { return st(this, __f).faces.size; });
P.method(FP, 'check', 1, function __f(font, text) {
  const s = st(this, __f); P.args(pre('check'), arguments.length, 1, __f);
  const fams = resolve(P.conv.DOMString(font, pre('check'), __f), 'check', __f);
  for (const f of matching(s, fams)) if (face.get(f).status !== 'loaded') return false;
  return true;
});
P.method(FP, 'load', 1, function __f(font, text) {
  try {
    const s = st(this, __f); P.args(pre('load'), arguments.length, 1, __f);
    const fams = resolve(P.conv.DOMString(font, pre('load'), __f), 'load', __f);
    const hit = matching(s, fams);
    return Promise.all(hit.map((f) => f.load())).then(() => hit);
  } catch (e) { return Promise.reject(e); }
});
P.method(FP, 'add', 1, function __f(face) {
  const s = st(this, __f); P.args(pre('add'), arguments.length, 1, __f);
  if (!isFace(face)) P.fail(__f, 'parameter 1 is not of type \'FontFace\'.', pre('add'));
  s.faces.add(face); return this;
});
P.method(FP, 'clear', 0, function __f() { st(this, __f).faces.clear(); });
P.method(FP, 'delete', 1, function __f(face) {
  const s = st(this, __f); P.args(pre('delete'), arguments.length, 1, __f);
  if (!isFace(face)) P.fail(__f, 'parameter 1 is not of type \'FontFace\'.', pre('delete'));
  return s.faces.delete(face);
});
P.method(FP, 'entries', 0, function __f() { return Array.from(st(this, __f).faces, (f) => [f, f])[Symbol.iterator](); });
P.method(FP, 'forEach', 1, function __f(cb, thisArg) {
  const s = st(this, __f); P.args(pre('forEach'), arguments.length, 1, __f);
  if (typeof cb !== 'function') P.fail(__f, 'The callback provided as parameter 1 is not a function.', pre('forEach'));
  for (const f of Array.from(s.faces)) cb.call(thisArg, f, f, this);
});
P.method(FP, 'has', 1, function __f(face) {
  const s = st(this, __f); P.args(pre('has'), arguments.length, 1, __f);
  if (!isFace(face)) P.fail(__f, 'parameter 1 is not of type \'FontFace\'.', pre('has'));
  return s.faces.has(face);
});
P.method(FP, 'keys', 0, function __f() { return Array.from(st(this, __f).faces)[Symbol.iterator](); });
const values = function __f() { return st(this, __f).faces.values(); };
P.method(FP, 'values', 0, values);
Object.defineProperty(FP, Symbol.iterator, { value: values, writable: true, enumerable: false, configurable: true });
// (no `constructor` own property: the interface object is not exposed, Chrome's prototype has none either)

// ---- the single per-document instance, and the Document.fonts accessor
const insts = new WeakMap();
function make() {
  const o = Object.create(FP);
  let done; const ready = Promise.resolve(o);
  state.set(o, { faces: new Set(), handlers: {}, ready });
  return o;
}
const fontsGet = function () { if (this === null || typeof this !== 'object') P.illegal(fontsGet); let o = insts.get(this); if (o === undefined) { o = make(); insts.set(this, o); } return o; };
P.getter(Document.prototype, 'fonts', fontsGet);
