//! module: canvasobjects
//! layer: L2
//! exposes: CanvasGradient, CanvasPattern
//! requires-load: -
//! requires-call: legacy:DOMException

// CanvasGradient / CanvasPattern and their factories (M4 knife 42). createLinearGradient returned an object literal
// `{ addColorStop(){}, _x0, _y0, _x1, _y1 }`, createRadialGradient a bare `{ addColorStop(){} }` and createPattern `{}`: not
// CanvasGradient/CanvasPattern instances, no argument validation, no addColorStop checks, `_x0`-style keys leaking. Chrome 145:
// illegal-constructor interfaces, validated factories ("N arguments required", non-finite doubles, negative radii as
// IndexSizeError, pattern image type / repetition / zero-size canvas), addColorStop with the (0.0, 1.0) range check and
// "could not be parsed as a color". Pinned by chrome/cvobj-truth-145.json (cvobj-probe.js) and diffed by
// canvasobjects_match_chrome_truth. Gradients/patterns have no rendering effect here (the software rasterizer has no gradient
// pipeline) - the state is recorded for a future one.

const C = globalThis.CanvasRenderingContext2D;
const P_ = C.prototype;
const grads = new WeakMap();
const pats = new WeakMap();
const CTX = "Failed to execute '";
const ctxPre = (m) => CTX + m + "' on 'CanvasRenderingContext2D'";

function CanvasGradient() { P.fail(CanvasGradient, 'Illegal constructor', "Failed to construct 'CanvasGradient'"); }
const GP = P.iface(CanvasGradient, 'CanvasGradient', 0);
function CanvasPattern() { P.fail(CanvasPattern, 'Illegal constructor', "Failed to construct 'CanvasPattern'"); }
const PP = P.iface(CanvasPattern, 'CanvasPattern', 0);

function validColor(v) {
  if (v === '') return false;
  const x = String(v).trim();
  if (/^currentcolor$/i.test(x) || /^transparent$/i.test(x)) return true;
  try { const css = globalThis.CSS; if (css && typeof css.supports === 'function') return css.supports('color', x); } catch (e) {}
  return /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(x) || /^[a-z]+$/i.test(x) || /^(rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color)\(.+\)$/i.test(x);
}
P.method(GP, 'addColorStop', 2, function __f(offset, color) {
  const g = this !== null && typeof this === 'object' ? grads.get(this) : undefined;
  if (g === undefined) P.illegal(__f);
  const pre = ctxPre('addColorStop').replace("'CanvasRenderingContext2D'", "'CanvasGradient'");
  P.args(pre, arguments.length, 2, __f);
  const o = Number(offset);
  if (!isFinite(o)) P.fail(__f, 'The provided double value is non-finite.', pre);
  if (o < 0 || o > 1) throw new DOMException(pre + ': The provided value (' + o + ') is outside the range (0.0, 1.0).', 'IndexSizeError');
  const c = P.conv.DOMString(color, pre, __f);
  if (!validColor(c)) throw new DOMException(pre + ": The value provided ('" + c + "') could not be parsed as a color.", 'SyntaxError');
  g.stops.push([o, c]);
});
P.seal(CanvasGradient);

const dbl = (v, pre, fn) => { if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre); const n = Number(v); if (!isFinite(n)) P.fail(fn, 'The provided double value is non-finite.', pre); return n; };
function ctxCheck(o, fn) { if (o === null || typeof o !== 'object' || !(o instanceof C)) P.illegal(fn); }
function mkGradient(kind, vals) { const g = Object.create(GP); grads.set(g, { kind, vals, stops: [] }); return g; }
P.method(P_, 'createLinearGradient', 4, function __f() {
  ctxCheck(this, __f);
  const pre = ctxPre('createLinearGradient');
  P.args(pre, arguments.length, 4, __f);
  return mkGradient('linear', Array.prototype.slice.call(arguments, 0, 4).map((v) => dbl(v, pre, __f)));
});
P.method(P_, 'createRadialGradient', 6, function __f() {
  ctxCheck(this, __f);
  const pre = ctxPre('createRadialGradient');
  P.args(pre, arguments.length, 6, __f);
  const v = Array.prototype.slice.call(arguments, 0, 6).map((x) => dbl(x, pre, __f));
  if (v[2] < 0) throw new DOMException(pre + ': The r0 provided is less than 0.', 'IndexSizeError');
  if (v[5] < 0) throw new DOMException(pre + ': The r1 provided is less than 0.', 'IndexSizeError');
  return mkGradient('radial', v);
});
P.method(P_, 'createConicGradient', 3, function __f() {
  ctxCheck(this, __f);
  const pre = ctxPre('createConicGradient');
  P.args(pre, arguments.length, 3, __f);
  return mkGradient('conic', Array.prototype.slice.call(arguments, 0, 3).map((v) => dbl(v, pre, __f)));
});
const IMG_TYPES = '(CSSImageValue or HTMLCanvasElement or HTMLImageElement or HTMLVideoElement or ImageBitmap or OffscreenCanvas or SVGImageElement or VideoFrame)';
const REPS = ['repeat', 'no-repeat', 'repeat-x', 'repeat-y'];
function isImageSource(v) {
  if (v === null || typeof v !== 'object') return false;
  const G = globalThis;
  return [G.HTMLCanvasElement, G.HTMLImageElement, G.HTMLVideoElement, G.ImageBitmap, G.OffscreenCanvas, G.SVGImageElement, G.VideoFrame]
    .some((K) => typeof K === 'function' && v instanceof K);
}
P.method(P_, 'createPattern', 2, function __f(image, rep) {
  ctxCheck(this, __f);
  const pre = ctxPre('createPattern');
  P.args(pre, arguments.length, 2, __f);
  if (!isImageSource(image)) P.fail(__f, "The provided value is not of type '" + IMG_TYPES + "'.", pre);
  let r = 'repeat';
  if (rep !== null && rep !== undefined && String(rep) !== '') {
    r = P.conv.DOMString(rep, pre, __f);
    if (REPS.indexOf(r) < 0) throw new DOMException(pre + ": The provided type ('" + r + "') is not one of 'repeat', 'no-repeat', 'repeat-x', or 'repeat-y'.", 'SyntaxError');
  }
  const HC = globalThis.HTMLCanvasElement, HI = globalThis.HTMLImageElement;
  if (typeof HC === 'function' && image instanceof HC && (image.width === 0 || image.height === 0)) {
    throw new DOMException(pre + ': The image argument is a canvas element with a width or height of 0.', 'InvalidStateError');
  }
  if (typeof HI === 'function' && image instanceof HI && image.complete === false) return null;
  const p = Object.create(PP);
  pats.set(p, { image, repetition: r, transform: null });
  return p;
});
P.method(PP, 'setTransform', 0, function __f() {
  const s = this !== null && typeof this === 'object' ? pats.get(this) : undefined;
  if (s === undefined) P.illegal(__f);
  const m = arguments[0];
  if (m !== undefined && m !== null && typeof m !== 'object') P.fail(__f, "The provided value is not of type 'DOMMatrix2DInit'.", "Failed to execute 'setTransform' on 'CanvasPattern'");
  s.transform = m === undefined ? null : m;
});
P.seal(CanvasPattern);

P.expose('CanvasGradient', CanvasGradient);
P.expose('CanvasPattern', CanvasPattern);
