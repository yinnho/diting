//! module: canvasstate
//! layer: L2
//! exposes: -
//! requires-load: -
//! requires-call: -

// CanvasRenderingContext2D state attributes and methods the engine's 2D context never had (M4 knife 41). The context object
// (bootstrap.js _Canvas2D, now THE CanvasRenderingContext2D class) returned `undefined`/`null` for lang, direction, fontKerning,
// fontStretch, fontVariantCaps, textRendering, letterSpacing, wordSpacing, filter, imageSmoothing*, shadow*, lineCap, lineJoin,
// miterLimit, lineDashOffset - and accepted any value without validating it. Canvas fingerprinting scripts both READ these
// defaults and WRITE them (shadowBlur, lineCap, ...). Pinned by chrome/cvstate-truth-145.json (cvstate-probe.js) and diffed by
// canvasstate_matches_chrome_truth. Chrome ignores invalid enum values and non-finite numbers silently; 'inherit' for direction
// resolves to the canvas's direction ('ltr'). No rendering effect (the software rasterizer here has no shadow/filter pipeline),
// exactly as before - this is the state surface.

const C = globalThis.CanvasRenderingContext2D;
const P_ = C.prototype;
const slots = new WeakMap();
function st(o, fn) {
  if (o === null || typeof o !== 'object' || !(o instanceof C)) P.illegal(fn);
  let s = slots.get(o);
  if (s === undefined) { s = defaults(); slots.set(o, s); }
  return s;
}
function defaults() {
  return { lang: 'inherit', direction: 'ltr', fontKerning: 'auto', fontStretch: 'normal', fontVariantCaps: 'normal', letterSpacing: '0px',
    textRendering: 'auto', wordSpacing: '0px', filter: 'none', imageSmoothingQuality: 'low', imageSmoothingEnabled: true,
    shadowColor: 'rgba(0, 0, 0, 0)', shadowOffsetX: 0, shadowOffsetY: 0, shadowBlur: 0, lineCap: 'butt', lineJoin: 'miter',
    miterLimit: 10, lineDashOffset: 0, lineDash: [] };
}
const ENUMS = {
  fontKerning: ['auto', 'normal', 'none'],
  fontStretch: ['ultra-condensed', 'extra-condensed', 'condensed', 'semi-condensed', 'normal', 'semi-expanded', 'expanded', 'extra-expanded', 'ultra-expanded'],
  fontVariantCaps: ['normal', 'small-caps', 'all-small-caps', 'petite-caps', 'all-petite-caps', 'unicase', 'titling-caps'],
  imageSmoothingQuality: ['low', 'medium', 'high'],
  lineCap: ['butt', 'round', 'square'],
  lineJoin: ['round', 'bevel', 'miter'],
  textRendering: ['auto', 'optimizeSpeed', 'optimizeLegibility', 'geometricPrecision'],
};
const LEN = /^([+-]?(?:\d+\.?\d*|\.\d+))(px|em|rem|pt|pc|cm|mm|in|ex|ch|vw|vh|vmin|vmax|q)$/i;
const num = (v) => { const n = Number(v); return n; };

function attr(name, set) {
  P.accessor(P_, name, function __f() { return st(this, __f)[name]; }, function __f(v) {
    const s = st(this, __f);
    P.args("Failed to set the '" + name + "' property on 'CanvasRenderingContext2D'", 1, 1, __f);
    set(s, v, __f);
  });
}
const enumAttr = (name) => attr(name, (s, v) => { const x = String(v); if (ENUMS[name].indexOf(x) >= 0) s[name] = x; });
const finiteAttr = (name, ok) => attr(name, (s, v) => { const n = num(v); if (isFinite(n) && (!ok || ok(n))) s[name] = n; });
const spacingAttr = (name) => attr(name, (s, v) => {
  const x = String(v);
  // Chrome accepts only a well-formed CSS length: no surrounding whitespace, and unitless values (including "0") are rejected
  if (LEN.test(x)) s[name] = x;
});

// Chrome's relative order of the attributes: lang first, the text/line state in the order below (shape_fix ORDER finalizes it).
attr('lang', (s, v) => { s.lang = String(v); });
attr('direction', (s, v) => { const x = String(v); if (x === 'ltr' || x === 'rtl') s.direction = x; else if (x === 'inherit') s.direction = 'ltr'; });
enumAttr('fontKerning'); enumAttr('fontStretch'); enumAttr('fontVariantCaps');
spacingAttr('letterSpacing'); enumAttr('textRendering'); spacingAttr('wordSpacing');
attr('filter', (s, v) => { const x = String(v).trim(); if (x === 'none' || /^[a-z-]+\(.*\)$/i.test(x)) s.filter = x; });
enumAttr('imageSmoothingQuality');
attr('shadowColor', (s, v) => { const x = String(v).trim(); if (x !== '') s.shadowColor = x; });
enumAttr('lineCap'); enumAttr('lineJoin');
attr('imageSmoothingEnabled', (s, v) => { s.imageSmoothingEnabled = Boolean(v); });
finiteAttr('shadowOffsetX'); finiteAttr('shadowOffsetY');
finiteAttr('shadowBlur', (n) => n >= 0); finiteAttr('miterLimit', (n) => n > 0); finiteAttr('lineDashOffset');

P.method(P_, 'getLineDash', 0, function __f() { return st(this, __f).lineDash.slice(); });
P.method(P_, 'setLineDash', 1, function __f(a) {
  const s = st(this, __f);
  P.args("Failed to execute 'setLineDash' on 'CanvasRenderingContext2D'", arguments.length, 1, __f);
  if (a === null || typeof a !== 'object' || typeof a[Symbol.iterator] !== 'function') P.fail(__f, "The provided value cannot be converted to a sequence.", "Failed to execute 'setLineDash' on 'CanvasRenderingContext2D'");
  const xs = Array.from(a).map(Number);
  if (xs.some((n) => !isFinite(n) || n < 0)) return;
  s.lineDash = xs.length % 2 ? xs.concat(xs) : xs;
});
P.method(P_, 'isContextLost', 0, function __f() { st(this, __f); return false; });
P.method(P_, 'getContextAttributes', 0, function __f() { st(this, __f); return { alpha: true, colorSpace: 'srgb', colorType: 'unorm8', desynchronized: false, toneMapping: { mode: 'standard' }, willReadFrequently: false }; });
P.method(P_, 'drawFocusIfNeeded', 1, function __f() { st(this, __f); P.args("Failed to execute 'drawFocusIfNeeded' on 'CanvasRenderingContext2D'", arguments.length, 1, __f); });
P.method(P_, 'createConicGradient', 3, function __f(a, x, y) {
  st(this, __f);
  P.args("Failed to execute 'createConicGradient' on 'CanvasRenderingContext2D'", arguments.length, 3, __f);
  for (const n of [a, x, y]) if (!isFinite(Number(n))) P.fail(__f, 'The provided double value is non-finite.', "Failed to execute 'createConicGradient' on 'CanvasRenderingContext2D'");
  return this.createLinearGradient(0, 0, 0, 0);
});
P.method(P_, 'reset', 0, function __f() {
  const s = st(this, __f);
  Object.assign(s, defaults());
  this.clearRect(0, 0, this.canvas.width, this.canvas.height);
  this.fillStyle = '#000000'; this.strokeStyle = '#000000'; this.lineWidth = 1; this.globalAlpha = 1;
  this.globalCompositeOperation = 'source-over'; this.font = '10px sans-serif'; this.textAlign = 'start'; this.textBaseline = 'alphabetic';
  if (typeof this.setTransform === 'function') this.setTransform(1, 0, 0, 1, 0, 0);
  this.beginPath();
});
P.method(P_, 'roundRect', 4, function __f(x, y, w, h) {
  st(this, __f);
  const pre = "Failed to execute 'roundRect' on 'CanvasRenderingContext2D'";
  P.args(pre, arguments.length, 4, __f);
  const v = [x, y, w, h].map(Number);
  if (!v.every(isFinite)) return;
  let r = arguments[4];
  const list = r === undefined ? [0] : Array.isArray(r) ? r : [r];
  if (list.length < 1 || list.length > 4) throw new RangeError(pre + ': ' + list.length + ' radii provided. Between one and four radii are necessary.');
  const rad = list.map((q) => (q !== null && typeof q === 'object' ? Number(q.x) : Number(q)));
  if (rad.some((n) => !isFinite(n))) return;
  if (rad.some((n) => n < 0)) throw new RangeError(pre + ': Radius value ' + rad.find((n) => n < 0) + ' is negative.');
  const [tl, tr, br, bl] = rad.length === 1 ? [rad[0], rad[0], rad[0], rad[0]] : rad.length === 2 ? [rad[0], rad[1], rad[0], rad[1]]
    : rad.length === 3 ? [rad[0], rad[1], rad[2], rad[1]] : rad;
  const [X, Y, W, H] = v;
  this.moveTo(X + tl, Y); this.lineTo(X + W - tr, Y); this.arcTo(X + W, Y, X + W, Y + tr, tr);
  this.lineTo(X + W, Y + H - br); this.arcTo(X + W, Y + H, X + W - br, Y + H, br);
  this.lineTo(X + bl, Y + H); this.arcTo(X, Y + H, X, Y + H - bl, bl);
  this.lineTo(X, Y + tl); this.arcTo(X, Y, X + tl, Y, tl);
  this.closePath();
});
