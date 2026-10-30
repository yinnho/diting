// 刀59 (M4): the SVG root face — SVGSVGElement's 34 missing prototype members
// plus its 3 zoom constants (truth: shape-audit-truth-145.json, 37 mismatch),
// and the real storage the face reaches through: the legacy SVG value types
// (SVGAngle/SVGLength/SVGMatrix/SVGNumber/SVGPoint/SVGRect/SVGTransform/
// SVGAnimatedLength/SVGPreserveAspectRatio/SVGAnimatedPreserveAspectRatio)
// were minted shells whose members returned undefined (stub_members-era
// fill). This file installs working accessors BEFORE stub_members runs, so
// the fill-if-missing pass stays out. Every behavior below was probed
// against Chrome 145 (receipts in SURFACE.md 刀59); unprobed corners follow
// the SVG 1.1/2 spec and are marked GUESS in comments.
//
// Loads after global_surface (the shells exist) and after element_sink
// (viewBox already lifted onto SVGSVGElement.prototype); stub_members and
// the generated tables run later and only fill gaps.
(() => {
  const mark = (fn) => { if (typeof fn === 'function') _markNative(fn); return fn; };
  // def helper: value property
  const dval = (obj, name, value, flags) =>
    Object.defineProperty(obj, name, Object.assign({ value }, flags));
  // acc helper: accessor pair, Chrome flags ec (enumerable+configurable)
  const dacc = (obj, name, get, set) =>
    Object.defineProperty(obj, name, { get: mark(get), set: set ? mark(set) : undefined, enumerable: true, configurable: true });
  // mth helper: method, Chrome flags ecw
  const dmth = (obj, name, fn) =>
    Object.defineProperty(obj, name, { value: mark(fn), writable: true, enumerable: true, configurable: true });

  // ---------- unit tables (96 px per inch, Chrome's fixed 300x150 viewport) --
  const PXF = { 1: 1, 5: 1, 6: 96 / 2.54, 7: 96 / 25.4, 8: 96, 9: 4 / 3, 10: 16 };
  const USUF = { 2: '%', 3: 'em', 4: 'ex', 5: 'px', 6: 'cm', 7: 'mm', 8: 'in', 9: 'pt', 10: 'pc' };
  const SUF2U = { '%': 2, em: 3, ex: 4, px: 5, cm: 6, mm: 7, in: 8, pt: 9, pc: 10 };
  const LEN_RX = /^\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)\s*(%|em|ex|px|cm|mm|in|pt|pc)?\s*$/;
  const pctBase = (attr) => (attr === 'height' || attr === 'y') ? 150 : 300; // Chrome default viewport 300x150 (probed: width '100%' -> 300, height -> 150)

  // ---------- SVGLength ------------------------------------------------------
  // Storage model: attached lengths ([el, attrName]) are a VIEW of the
  // attribute — every read parses it, every write serializes back (probed:
  // baseVal.value=42 -> attribute "42", live attr set -> baseVal tracks).
  // Detached lengths (createSVGLength) hold _v/_u. animVal carries _ro and
  // every mutating setter throws NoModificationAllowedError (probed).
  {
    const P = SVGLength.prototype;
    const raw = function () {
      if (this._attEl) {
        const a = this._attEl.getAttribute(this._attName);
        if (a == null) {
          if (this._attName === 'width' || this._attName === 'height') return [100, 2];
          return [0, 1]; // x/y absent -> 0 number (probed)
        }
        const m = LEN_RX.exec(a);
        if (!m) return this._attName === 'width' || this._attName === 'height' ? [100, 2] : [0, 1];
        return [Number(m[1]), m[2] ? SUF2U[m[2]] : 1];
      }
      return [this._v, this._u];
    };
    const write = function (s) {
      if (this._ro) throw new DOMException("Failed to set the value: the object is read-only.", 'NoModificationAllowedError');
      if (this._attEl) this._attEl.setAttribute(this._attName, s);
      else { const m = LEN_RX.exec(String(s)); if (m) { this._v = Number(m[1]); this._u = m[2] ? SUF2U[m[2]] : 1; } }
    };
    const toPx = function (v, u) {
      if (u === 2) {
        if (this._attEl) return v / 100 * pctBase(this._attName);
        throw new DOMException("Failed to read the 'value' property from 'SVGLength': Could not resolve relative length.", 'NotSupportedError'); // probed on detached %
      }
      if (u === 3) return Math.fround(v * 16);  // GUESS: em at default font-size 16
      if (u === 4) return Math.fround(v * 8);   // GUESS: ex at half
      // Chrome stores SVG numbers as float32 — 5pt reads 6.666666507720947,
      // 10mm reads 37.7952766418457 (probed), not the float64 product.
      return Math.fround(v * PXF[u]);
    };
    dacc(P, 'unitType', function unitType() { return raw.call(this)[1]; });
    dacc(P, 'value',
      function value() { const [v, u] = raw.call(this); return toPx.call(this, v, u); },
      function value(px) {
        if (this._ro) throw new DOMException("Failed to set the 'value' property: the object is read-only.", 'NoModificationAllowedError');
        px = Number(px);
        const [v, u] = raw.call(this);
        let s;
        if (u === 2 && this._attEl) s = (px / pctBase(this._attName) * 100) + '%';
        else if (u >= 3 && USUF[u]) s = (px / (PXF[u] || 1)) + USUF[u];
        else s = String(px);
        if (this._attEl) this._attEl.setAttribute(this._attName, s);
        else this._v = u === 2 ? v : px / (PXF[u] || 1);
      });
    dacc(P, 'valueInSpecifiedUnits',
      function valueInSpecifiedUnits() { return raw.call(this)[0]; },
      function valueInSpecifiedUnits(v) {
        if (this._ro) throw new DOMException("Failed to set the 'valueInSpecifiedUnits' property: the object is read-only.", 'NoModificationAllowedError');
        if (this._attEl) this._attEl.setAttribute(this._attName, v + (USUF[raw.call(this)[1]] || ''));
        else this._v = Number(v);
      });
    dacc(P, 'valueAsString',
      function valueAsString() { const [v, u] = raw.call(this); return v + (USUF[u] || ''); },
      function valueAsString(s) {
        const m = LEN_RX.exec(String(s));
        if (!m) throw new SyntaxError("Failed to set the 'valueAsString' property from 'SVGLength'."); // probed: SyntaxError
        write.call(this, String(s).trim());
      });
    dmth(P, 'newValueSpecifiedUnits', function newValueSpecifiedUnits(u, v) {
      if (this._ro) throw new DOMException("Failed to execute 'newValueSpecifiedUnits': the object is read-only.", 'NoModificationAllowedError');
      const s = Number(v) + (USUF[u] || '');
      if (this._attEl) this._attEl.setAttribute(this._attName, s); else { this._v = Number(v); this._u = u; }
    });
    dmth(P, 'convertToSpecifiedUnits', function convertToSpecifiedUnits(u2) {
      if (this._ro) throw new DOMException("Failed to execute 'convertToSpecifiedUnits': the object is read-only.", 'NoModificationAllowedError');
      // probed: 72pt -> convert(pc) -> value stays 96px, unit pc, string "6pc"
      const [v, u] = raw.call(this);
      if (u === 2) { if (!this._attEl) return; this._u = u2; return; } // GUESS: detached % stays
      const px = toPx.call(this, v, u);
      if (this._attEl) this._attEl.setAttribute(this._attName, (px / (PXF[u2] || 1)) + (USUF[u2] || ''));
      else { this._v = px / (PXF[u2] || 1); this._u = u2; }
    });
    globalThis.__diting_svgLength = (el, name, ro) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name; o._v = 0; o._u = 1; o._ro = !!ro;
      return o;
    };
  }

  // ---------- SVGAngle -------------------------------------------------------
  // unitType: 0 unknown, 1 unspecified, 2 deg, 3 rad, 4 grad. value is in
  // DEGREES (probed: deg 90 -> value 90; rad/grad convert per spec).
  {
    const P = SVGAngle.prototype;
    const ASUF = { 2: 'deg', 3: 'rad', 4: 'grad' };
    const ARX = /^\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)\s*(deg|rad|grad)?\s*$/;
    const SUF2A = { deg: 2, rad: 3, grad: 4 };
    dacc(P, 'unitType', function unitType() { return this._u == null ? 1 : this._u; });
    dacc(P, 'value',
      function value() {
        const u = this._u == null ? 1 : this._u, v = this._v || 0;
        if (u === 3) return v * 180 / Math.PI;
        if (u === 4) return v * 0.9;
        return v;
      },
      function value(deg) {
        deg = Number(deg);
        const u = this._u == null ? 1 : this._u;
        this._v = u === 3 ? deg * Math.PI / 180 : u === 4 ? deg / 0.9 : deg;
      });
    dacc(P, 'valueInSpecifiedUnits',
      function valueInSpecifiedUnits() { return this._v || 0; },
      function valueInSpecifiedUnits(v) { this._v = Number(v); });
    dacc(P, 'valueAsString',
      function valueAsString() { const u = this._u == null ? 1 : this._u; return (this._v || 0) + (ASUF[u] || ''); },
      function valueAsString(s) {
        const m = ARX.exec(String(s));
        if (!m) throw new SyntaxError("Failed to set the 'valueAsString' property from 'SVGAngle'.");
        this._v = Number(m[1]); this._u = m[2] ? SUF2A[m[2]] : 1;
      });
    dmth(P, 'newValueSpecifiedUnits', function newValueSpecifiedUnits(u, v) { this._u = u; this._v = Number(v); });
    dmth(P, 'convertToSpecifiedUnits', function convertToSpecifiedUnits(u2) {
      const deg = this.value; this._u = u2;
      this._v = u2 === 3 ? deg * Math.PI / 180 : u2 === 4 ? deg / 0.9 : deg;
    });
    globalThis.__diting_svgAngle = () => { const o = Object.create(P); o._u = 1; o._v = 0; return o; };
  }

  // ---------- SVGNumber / SVGRect -------------------------------------------
  if (typeof SVGNumber === 'function') {
    dacc(SVGNumber.prototype, 'value', function value() { return this._v || 0; }, function value(v) { this._v = Number(v); });
    globalThis.__diting_svgNumber = () => { const o = Object.create(SVGNumber.prototype); o._v = 0; return o; };
  }
  if (typeof SVGRect === 'function') {
    const P = SVGRect.prototype;
    for (const k of ['x', 'y', 'width', 'height']) {
      dacc(P, k, new Function('k', 'return function ' + k + '() { return this._r ? this._r.' + k + ' : 0; };')(k),
        new Function('k', 'return function ' + k + '(v) { if (!this._r) this._r = { x: 0, y: 0, width: 0, height: 0 }; this._r.' + k + ' = Number(v); };')(k));
    }
    globalThis.__diting_svgRect = (r) => { const o = Object.create(P); o._r = r || { x: 0, y: 0, width: 0, height: 0 }; return o; };
  }

  // ---------- SVGPoint -------------------------------------------------------
  // bootstrap's class stays (getPointAtLength constructs it); the ctor now
  // writes the hidden _ps store and this installs the Chrome truth face
  // (x/y accessors rw:ec + a COMPUTING matrixTransform — the old stub
  // returned `this`). currentTranslate mints points sharing one _ps object
  // (probed: fresh wrapper each read, writes persist).
  {
    const P = SVGPoint.prototype;
    dacc(P, 'x', function x() { return this._ps ? this._ps.x : 0; }, function x(v) { if (this._ps) this._ps.x = Number(v); });
    dacc(P, 'y', function y() { return this._ps ? this._ps.y : 0; }, function y(v) { if (this._ps) this._ps.y = Number(v); });
    dmth(P, 'matrixTransform', function matrixTransform(m) {
      const x = this._ps ? this._ps.x : 0, y = this._ps ? this._ps.y : 0;
      const g = (k) => Number(m && m[k]) || 0;
      return new SVGPoint(g('a') * x + g('c') * y + g('e'), g('b') * x + g('d') * y + g('f'));
    });
    globalThis.__diting_svgPoint = (store) => { const o = Object.create(P); o._ps = store || { x: 0, y: 0 }; return o; };
  }

  // ---------- SVGMatrix ------------------------------------------------------
  // Storage _m = [a,b,c,d,e,f]; every op is functional (probed: translate
  // leaves the receiver untouched). Non-invertible inverse throws
  // (message GUESS, throw itself is spec).
  {
    const P = SVGMatrix.prototype;
    for (let i = 0; i < 6; i++) {
      const k = 'abcdef'[i];
      dacc(P, k, new Function('i', 'k', 'return function ' + k + '() { return this._m ? this._m[i] : (i === 0 || i === 3 ? 1 : 0); };')(i, k),
        new Function('i', 'k', 'return function ' + k + '(v) { if (!this._m) this._m = [1, 0, 0, 1, 0, 0]; this._m[i] = Number(v); };')(i, k));
    }
    const mk = (m) => { const o = Object.create(P); o._m = m; return o; };
    const M = (o) => o && o._m ? o._m.slice() : [1, 0, 0, 1, 0, 0];
    const mul = (x, y) => [ // x * y (post-multiply)
      x[0] * y[0] + x[2] * y[1], x[1] * y[0] + x[3] * y[1],
      x[0] * y[2] + x[2] * y[3], x[1] * y[2] + x[3] * y[3],
      x[0] * y[4] + x[2] * y[5] + x[4], x[1] * y[4] + x[3] * y[5] + x[5]];
    globalThis.__diting_svgMatrix = () => mk([1, 0, 0, 1, 0, 0]);
    // 刀59 fix: every SVGMatrix op is PURE in Chrome 145 — it returns a new
    // matrix and leaves the receiver untouched (probed: translate(1,2) keeps
    // e/f at 0 on the original). All ops below mint, never mutate.
    const pure = (m6) => mk(m6);
    dmth(P, 'multiply', function multiply(m2) { return pure(mul(M(this), M(m2))); });
    dmth(P, 'translate', function translate(x, y) { return pure(mul(M(this), [1, 0, 0, 1, Number(x) || 0, Number(y) || 0])); });
    dmth(P, 'scale', function scale(f) { f = Number(f) || 0; return pure(mul(M(this), [f, 0, 0, f, 0, 0])); });
    dmth(P, 'scaleNonUniform', function scaleNonUniform(sx, sy) { return pure(mul(M(this), [Number(sx) || 0, 0, 0, Number(sy) || 0, 0, 0])); });
    dmth(P, 'rotate', function rotate(a) {
      const r = (Number(a) || 0) * Math.PI / 180, c = Math.cos(r), s = Math.sin(r);
      return pure(mul(M(this), [c, s, -s, c, 0, 0]));
    });
    dmth(P, 'rotateFromVector', function rotateFromVector(x, y) {
      x = Number(x) || 0; y = Number(y) || 0;
      const m0 = M(this);
      if (x === 0 && y === 0) return pure(m0); // GUESS: spec says silently ignored
      const l = Math.hypot(x, y), r = Math.atan2(y / l, x / l), c = Math.cos(r), s = Math.sin(r);
      return pure(mul(m0, [c, s, -s, c, 0, 0]));
    });
    dmth(P, 'flipX', function flipX() { return pure(mul(M(this), [-1, 0, 0, 1, 0, 0])); });
    dmth(P, 'flipY', function flipY() { return pure(mul(M(this), [1, 0, 0, -1, 0, 0])); });
    dmth(P, 'skewX', function skewX(a) { return pure(mul(M(this), [1, 0, Math.tan((Number(a) || 0) * Math.PI / 180), 1, 0, 0])); });
    dmth(P, 'skewY', function skewY(a) { return pure(mul(M(this), [1, Math.tan((Number(a) || 0) * Math.PI / 180), 0, 1, 0, 0])); });
    dmth(P, 'inverse', function inverse() {
      const m = M(this), det = m[0] * m[3] - m[1] * m[2];
      if (det === 0) throw new DOMException("Failed to execute 'inverse' on 'SVGMatrix': The matrix is not invertible.", 'InvalidStateError');
      return pure([m[3] / det, -m[1] / det, -m[2] / det, m[0] / det,
        (m[2] * m[5] - m[3] * m[4]) / det, (m[1] * m[4] - m[0] * m[5]) / det]);
    });
  }

  // ---------- SVGTransform ---------------------------------------------------
  // type constants already sit on the prototype (mint). setMatrix COPIES
  // (probed), matrix identity is cached per transform (t.matrix === t.matrix).
  {
    const P = SVGTransform.prototype;
    const mkM = (m) => { const o = Object.create(SVGMatrix.prototype); o._m = m; return o; };
    dacc(P, 'type', function type() { return this._tt == null ? 1 : this._tt; });
    dacc(P, 'matrix', function matrix() { if (!this._tm) this._tm = mkM([1, 0, 0, 1, 0, 0]); return this._tm; });
    dacc(P, 'angle', function angle() { return this._ta || 0; });
    const setM = function (m6, tt, ta) { this._tm = mkM(m6); this._tt = tt; this._ta = ta || 0; };
    dmth(P, 'setMatrix', function setMatrix(m) {
      const s = m && m._m ? m._m : [1, 0, 0, 1, 0, 0];
      setM.call(this, [Number(s[0]) || 0, Number(s[1]) || 0, Number(s[2]) || 0, Number(s[3]) || 0, Number(s[4]) || 0, Number(s[5]) || 0], 1, 0);
    });
    dmth(P, 'setTranslate', function setTranslate(x, y) { setM.call(this, [1, 0, 0, 1, Number(x) || 0, Number(y) || 0], 2, 0); });
    dmth(P, 'setScale', function setScale(sx, sy) { setM.call(this, [Number(sx) || 0, 0, 0, Number(sy) || 0, 0, 0], 3, 0); });
    dmth(P, 'setRotate', function setRotate(a, cx, cy) {
      a = Number(a) || 0; cx = Number(cx) || 0; cy = Number(cy) || 0;
      const r = a * Math.PI / 180, c = Math.cos(r), s = Math.sin(r);
      setM.call(this, [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy], 4, a);
    });
    dmth(P, 'setSkewX', function setSkewX(a) { setM.call(this, [1, 0, Math.tan((Number(a) || 0) * Math.PI / 180), 1, 0, 0], 5, Number(a) || 0); });
    dmth(P, 'setSkewY', function setSkewY(a) { setM.call(this, [1, Math.tan((Number(a) || 0) * Math.PI / 180), 0, 1, 0, 0], 6, Number(a) || 0); });
    globalThis.__diting_svgTransform = () => Object.create(P);
  }

  // ---------- SVGPreserveAspectRatio ----------------------------------------
  // align: 1 none, 2..10 xMinYMin..xMaxYMax (default 6); meetOrSlice: 1 meet,
  // 2 slice (default 1). Attached baseVal writes the full keyword string
  // back (probed: align=7 -> "xMaxYMid meet").
  {
    const P = SVGPreserveAspectRatio.prototype;
    const ALN = { none: 1, xminymin: 2, xmidymin: 3, xmaxymin: 4, xminymid: 5, xmidymid: 6, xmaxymid: 7, xminymax: 8, xmidymax: 9, xmaxymax: 10 };
    const ALN_R = { 1: 'none', 2: 'xMinYMin', 3: 'xMidYMin', 4: 'xMaxYMin', 5: 'xMinYMid', 6: 'xMidYMid', 7: 'xMaxYMid', 8: 'xMinYMax', 9: 'xMidYMax', 10: 'xMaxYMax' };
    const raw = function () {
      if (this._attEl) {
        const t = (this._attEl.getAttribute('preserveAspectRatio') || '').trim().toLowerCase().split(/\s+/);
        const al = ALN[t[0] || ''];
        return [al == null ? 6 : al, t[1] === 'slice' ? 2 : 1];
      }
      return [this._align == null ? 6 : this._align, this._mos == null ? 1 : this._mos];
    };
    const write = function (a, m) {
      if (this._ro) throw new DOMException("Failed to set the value: the object is read-only.", 'NoModificationAllowedError');
      if (this._attEl) this._attEl.setAttribute('preserveAspectRatio', (ALN_R[a] || 'xMidYMid') + ' ' + (m === 2 ? 'slice' : 'meet'));
      else { this._align = a; this._mos = m; }
    };
    dacc(P, 'align', function align() { return raw.call(this)[0]; }, function align(v) { write.call(this, Number(v), raw.call(this)[1]); });
    dacc(P, 'meetOrSlice', function meetOrSlice() { return raw.call(this)[1]; }, function meetOrSlice(v) { write.call(this, raw.call(this)[0], Number(v)); });
    globalThis.__diting_svgPAR = (el, ro) => { const o = Object.create(P); o._attEl = el || null; o._ro = !!ro; return o; };
  }

  // ---------- SVGAnimatedLength / SVGAnimatedPreserveAspectRatio ------------
  // Two DISTINCT value objects (probed: baseVal !== animVal), stable across
  // reads; animVal rejects writes.
  if (typeof SVGAnimatedLength === 'function') {
    const P = SVGAnimatedLength.prototype;
    dacc(P, 'baseVal', function baseVal() { return this._b || (this._b = __diting_svgLength(this._attEl, this._attName, false)); });
    dacc(P, 'animVal', function animVal() { return this._a || (this._a = __diting_svgLength(this._attEl, this._attName, true)); });
    globalThis.__diting_svgAnimLength = (el, name) => { const o = Object.create(P); o._attEl = el; o._attName = name; return o; };
  }
  if (typeof SVGAnimatedPreserveAspectRatio === 'function') {
    const P = SVGAnimatedPreserveAspectRatio.prototype;
    dacc(P, 'baseVal', function baseVal() { return this._b || (this._b = __diting_svgPAR(this._attEl, false)); });
    dacc(P, 'animVal', function animVal() { return this._a || (this._a = __diting_svgPAR(this._attEl, true)); });
    globalThis.__diting_svgAnimPAR = (el) => { const o = Object.create(P); o._attEl = el; return o; };
  }

  // ---------- SVGSVGElement --------------------------------------------------
  {
    const SP = SVGSVGElement.prototype;
    const vbDesc = Object.getOwnPropertyDescriptor(SP, 'viewBox');
    if (vbDesc) delete SP.viewBox; // re-added in Chrome's slot below

    dacc(SP, 'x', function x() { (this._sal || (this._sal = {})).x || (this._sal.x = __diting_svgAnimLength(this, 'x')); return this._sal.x; });
    dacc(SP, 'y', function y() { (this._sal || (this._sal = {})).y || (this._sal.y = __diting_svgAnimLength(this, 'y')); return this._sal.y; });
    dacc(SP, 'width', function width() { (this._sal || (this._sal = {})).width || (this._sal.width = __diting_svgAnimLength(this, 'width')); return this._sal.width; });
    dacc(SP, 'height', function height() { (this._sal || (this._sal = {})).height || (this._sal.height = __diting_svgAnimLength(this, 'height')); return this._sal.height; });
    dacc(SP, 'currentScale',
      function currentScale() { return this._cs == null ? 1 : this._cs; },
      function currentScale(v) { this._cs = Number(v) || 0; });
    dacc(SP, 'currentTranslate', function currentTranslate() { return __diting_svgPoint(this._pan || (this._pan = { x: 0, y: 0 })); });
    if (vbDesc) Object.defineProperty(SP, 'viewBox', vbDesc);
    dacc(SP, 'preserveAspectRatio', function preserveAspectRatio() { return this._spar || (this._spar = __diting_svgAnimPAR(this)); });
    dacc(SP, 'zoomAndPan',
      function zoomAndPan() { return this._zap == null ? 2 : this._zap; },
      function zoomAndPan(v) { const n = Number(v); this._zap = (n === 1 || n === 2) ? n : 0; }); // probed: default 2, set 5 -> 0, no attribute sync
    for (const [n, v] of [['SVG_ZOOMANDPAN_UNKNOWN', 0], ['SVG_ZOOMANDPAN_DISABLE', 1], ['SVG_ZOOMANDPAN_MAGNIFY', 2]]) {
      dval(SP, n, v, { enumerable: true });                                      // truth kind number:e-- (proto)
      dval(SVGSVGElement, n, v, { enumerable: true });                           // truth skind number:e-- (ctor)
    }

    // legacy redraw/animation no-ops (probed: suspendRedraw -> 1 always,
    // unsuspend*/forceRedraw/deselectAll -> undefined, clock reads 0 even
    // after setCurrentTime, animationsPaused -> false — no SMIL in Chrome's
    // probe page either).
    dmth(SP, 'animationsPaused', function animationsPaused() { return false; });
    dmth(SP, 'checkEnclosure', function checkEnclosure(el, rect) {
      const b = gbox(el); if (!b) return false;
      const r = rbox(rect); return b[0] >= r[0] && b[1] >= r[1] && b[0] + b[2] <= r[0] + r[2] && b[1] + b[3] <= r[1] + r[3];
    });
    dmth(SP, 'checkIntersection', function checkIntersection(el, rect) {
      const b = gbox(el); if (!b) return false;
      const r = rbox(rect); return b[0] < r[0] + r[2] && b[0] + b[2] > r[0] && b[1] < r[1] + r[3] && b[1] + b[3] > r[1];
    });
    dmth(SP, 'createSVGAngle', function createSVGAngle() { return __diting_svgAngle(); });
    dmth(SP, 'createSVGLength', function createSVGLength() { return __diting_svgLength(null, '', false); });
    dmth(SP, 'createSVGMatrix', function createSVGMatrix() { return __diting_svgMatrix(); });
    dmth(SP, 'createSVGNumber', function createSVGNumber() { return __diting_svgNumber(); });
    dmth(SP, 'createSVGPoint', function createSVGPoint() { return __diting_svgPoint(); });
    dmth(SP, 'createSVGRect', function createSVGRect() { return __diting_svgRect(); });
    dmth(SP, 'createSVGTransform', function createSVGTransform() { return __diting_svgTransform(); });
    dmth(SP, 'createSVGTransformFromMatrix', function createSVGTransformFromMatrix(m) {
      const t = __diting_svgTransform(); t.setMatrix(m); return t;
    });
    dmth(SP, 'deselectAll', function deselectAll() {});
    dmth(SP, 'forceRedraw', function forceRedraw() {});
    dmth(SP, 'getCurrentTime', function getCurrentTime() { return 0; });
    dmth(SP, 'getElementById', function getElementById(id) {
      id = String(id);
      const walk = (n) => {
        for (const c of n.children || []) {
          if (c.getAttribute && c.getAttribute('id') === id) return c;
          const r = walk(c); if (r) return r;
        }
        return null;
      };
      return walk(this);
    });
    dmth(SP, 'getEnclosureList', function getEnclosureList(rect, ref) { return listBy(this, rect, ref, true); });
    dmth(SP, 'getIntersectionList', function getIntersectionList(rect, ref) { return listBy(this, rect, ref, false); });
    dmth(SP, 'pauseAnimations', function pauseAnimations() {});
    dmth(SP, 'setCurrentTime', function setCurrentTime(t) {});
    dmth(SP, 'suspendRedraw', function suspendRedraw(ms) { return 1; });
    dmth(SP, 'unpauseAnimations', function unpauseAnimations() {});
    dmth(SP, 'unsuspendRedraw', function unsuspendRedraw(id) {});
    dmth(SP, 'unsuspendRedrawAll', function unsuspendRedrawAll() {});

    // hit-test geometry: bbox from attributes per graphics tag (no layout
    // dependency; path bbox approximates over every coordinate pair — curve
    // control points included, close enough for list membership).
    const GRAPHICS = new Set(['circle', 'ellipse', 'line', 'path', 'polygon', 'polyline', 'rect', 'text', 'image', 'use']);
    const num = (el, k) => { const v = parseFloat(el.getAttribute && el.getAttribute(k)); return isNaN(v) ? 0 : v; };
    function gbox(el) {
      if (!el || !GRAPHICS.has(el.localName)) return null;
      const ln = el.localName;
      if (ln === 'circle') { const r = Math.max(0, num(el, 'r')); return [num(el, 'cx') - r, num(el, 'cy') - r, 2 * r, 2 * r]; }
      if (ln === 'ellipse') { const rx = Math.max(0, num(el, 'rx')), ry = Math.max(0, num(el, 'ry')); return [num(el, 'cx') - rx, num(el, 'cy') - ry, 2 * rx, 2 * ry]; }
      if (ln === 'rect' || ln === 'image') return [num(el, 'x'), num(el, 'y'), num(el, 'width'), num(el, 'height')];
      if (ln === 'line') {
        const x1 = num(el, 'x1'), y1 = num(el, 'y1'), x2 = num(el, 'x2'), y2 = num(el, 'y2');
        return [Math.min(x1, x2), Math.min(y1, y2), Math.abs(x2 - x1), Math.abs(y2 - y1)];
      }
      if (ln === 'polyline' || ln === 'polygon') {
        const t = (el.getAttribute('points') || '').trim().split(/[\s,]+/).map(Number).filter((v) => !isNaN(v));
        if (t.length < 2) return [0, 0, 0, 0];
        const xs = t.filter((_, i) => i % 2 === 0), ys = t.filter((_, i) => i % 2 === 1);
        const x0 = Math.min(...xs), y0 = Math.min(...ys);
        return [x0, y0, Math.max(...xs) - x0, Math.max(...ys) - y0];
      }
      if (ln === 'path') {
        const t = (el.getAttribute('d') || '').match(/-?\d*\.?\d+(?:e[+-]?\d+)?/gi);
        if (!t || t.length < 2) return [0, 0, 0, 0];
        const n = t.map(Number), xs = [], ys = [];
        for (let i = 0; i + 1 < n.length; i += 2) { xs.push(n[i]); ys.push(n[i + 1]); }
        const x0 = Math.min(...xs), y0 = Math.min(...ys);
        return [x0, y0, Math.max(...xs) - x0, Math.max(...ys) - y0];
      }
      return [0, 0, 0, 0]; // text/use fallback
    }
    const rbox = (r) => [Number(r && r.x) || 0, Number(r && r.y) || 0, Number(r && r.width) || 0, Number(r && r.height) || 0];
    // Chrome's prototype order ends with `constructor`; the class syntax
    // mints it FIRST. Move it last HERE so the raw order already equals
    // truth: gen_shape_fix then emits no ORDER entry for this interface,
    // and shape_fix's reordering pass — which cannot move the
    // non-configurable (e--) zoom constants — never runs against it.
    {
      const d = Object.getOwnPropertyDescriptor(SP, 'constructor');
      if (d && d.configurable) { delete SP.constructor; Object.defineProperty(SP, 'constructor', d); }
    }
    function listBy(svg, rect, ref, enclosure) {
      const r = rbox(rect), out = [];
      const walk = (n) => {
        for (const c of n.children || []) {
          const b = gbox(c);
          if (b && (enclosure
            ? b[0] >= r[0] && b[1] >= r[1] && b[0] + b[2] <= r[0] + r[2] && b[1] + b[3] <= r[1] + r[3]
            : b[0] < r[0] + r[2] && b[0] + b[2] > r[0] && b[1] < r[1] + r[3] && b[1] + b[3] > r[1])) out.push(c);
          walk(c);
        }
      };
      walk(ref && ref.children ? ref : svg);
      return __diting_mkNodeList(out);
    }
  }
})();
