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
          if (this._def) return this._def; // 刀63: per-attach default (marker 3, FE [0,%], pattern 0)
          if (this._attName === 'width' || this._attName === 'height') return [100, 2];
          return [0, 1]; // x/y absent -> 0 number (probed)
        }
        const m = LEN_RX.exec(a);
        if (!m) return this._def || (this._attName === 'width' || this._attName === 'height' ? [100, 2] : [0, 1]);
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
        if (this._noPct) throw new DOMException("Failed to read the 'value' property from 'SVGLength': Could not resolve relative length.", 'NotSupportedError'); // 刀63: FE-region % never resolves (probed on feDropShadow x/y/w/h)
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
    globalThis.__diting_svgLength = (el, name, ro, def, noPct) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name; o._v = 0; o._u = 1; o._ro = !!ro; o._def = def || null; o._noPct = !!noPct;
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
    // 刀63: attached angles view an attribute (orient). Non-numeric values
    // ('auto', 'auto-start-reverse', junk) and absence read [0, unspecified]
    // (probed); every write serializes number+unit back (probed: value=20 ->
    // '20', setOrientToAngle(30deg) -> '30deg'). Detached angles unchanged.
    const araw = function () {
      if (this._attEl) {
        const m = ARX.exec(String(this._attEl.getAttribute(this._attName) == null ? '' : this._attEl.getAttribute(this._attName)));
        return m ? [Number(m[1]), m[2] ? SUF2A[m[2]] : 1] : [0, 1];
      }
      return [this._v == null ? 0 : this._v, this._u == null ? 1 : this._u];
    };
    const awrite = function (v, u) {
      if (this._attEl) this._attEl.setAttribute(this._attName, String(v) + (ASUF[u] || ''));
      else { this._v = v; this._u = u; }
    };
    dacc(P, 'unitType', function unitType() { return araw.call(this)[1]; });
    dacc(P, 'value',
      function value() {
        const r = araw.call(this), u = r[1], v = r[0];
        if (u === 3) return v * 180 / Math.PI;
        if (u === 4) return v * 0.9;
        return v;
      },
      function value(deg) {
        deg = Number(deg);
        const r = araw.call(this), u = r[1];
        awrite.call(this, u === 3 ? deg * Math.PI / 180 : u === 4 ? deg / 0.9 : deg, u);
      });
    dacc(P, 'valueInSpecifiedUnits',
      function valueInSpecifiedUnits() { return araw.call(this)[0]; },
      function valueInSpecifiedUnits(v) { awrite.call(this, Number(v), araw.call(this)[1]); });
    dacc(P, 'valueAsString',
      function valueAsString() { const r = araw.call(this); return r[0] + (ASUF[r[1]] || ''); },
      function valueAsString(s) {
        const m = ARX.exec(String(s));
        if (!m) throw new SyntaxError("Failed to set the 'valueAsString' property from 'SVGAngle'.");
        awrite.call(this, Number(m[1]), m[2] ? SUF2A[m[2]] : 1);
      });
    dmth(P, 'newValueSpecifiedUnits', function newValueSpecifiedUnits(u, v) { awrite.call(this, Number(v), u); });
    dmth(P, 'convertToSpecifiedUnits', function convertToSpecifiedUnits(u2) {
      const deg = this.value;
      awrite.call(this, u2 === 3 ? deg * Math.PI / 180 : u2 === 4 ? deg / 0.9 : deg, u2);
    });
    globalThis.__diting_svgAngle = () => { const o = Object.create(P); o._u = 1; o._v = 0; return o; };
    globalThis.__diting_svgAngleAttach = (el, name) => { const o = Object.create(P); o._attEl = el || null; o._attName = name; return o; };
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
    dacc(P, 'baseVal', function baseVal() { return this._b || (this._b = __diting_svgLength(this._attEl, this._attName, false, this._def, this._noPct)); });
    dacc(P, 'animVal', function animVal() { return this._a || (this._a = __diting_svgLength(this._attEl, this._attName, true, this._def, this._noPct)); });
    globalThis.__diting_svgAnimLength = (el, name, def, noPct) => { const o = Object.create(P); o._attEl = el; o._attName = name; o._def = def || null; o._noPct = !!noPct; return o; };
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

  // ========== 刀63: the remaining animated shells become working views ======
  // SVGAnimatedNumber/Enumeration/String/Angle/TransformList and the list
  // types were mint shells (baseVal/animVal -> undefined). Each gets a real
  // storage model now: attached views parse the attribute on every read and
  // serialize on write (probed); detached objects keep 刀59 semantics. Bodies
  // are redefined IN PLACE — these rows already pass order, so no wipe, no
  // ctor move. Probed corners carry receipts (SURFACE.md 刀63); serialize
  // formats for mutation paths Chrome never showed are marked GUESS.
  const NUMRX = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/;

  // ---- SVGAnimatedNumber: float attr view (feDropShadow dx/dy/std____) ----
  if (typeof SVGAnimatedNumber === 'function') {
    const P = SVGAnimatedNumber.prototype;
    const nread = function () {
      if (this._ovr !== undefined) return this._ovr; // setStdDeviation override (probed: never serialized)
      if (!this._attEl) return this._v == null ? 0 : this._v;
      const ov = this._attEl._sdOv; // shared pair override slot (setStdDeviation(x,y))
      if (ov) return this._pair === 1 ? ov[1] : ov[0];
      const a = this._attEl.getAttribute(this._attName);
      if (a == null) return this._def == null ? 0 : this._def; // absent -> default (probed: dx 2)
      if (this._pair != null) { // stdDeviation is a 'x y' pair (probed: '4 6' -> 4/6, single -> both)
        const t = String(a).trim().split(/[\s,]+/);
        const x = NUMRX.test(t[0] || '') ? Number(t[0]) : (this._def == null ? 0 : this._def);
        return this._pair === 1 ? (NUMRX.test(t[1] || '') ? Number(t[1]) : x) : x;
      }
      const t = String(a).trim();
      return NUMRX.test(t) ? Number(t) : (this._def == null ? 0 : this._def); // junk -> default (probed)
    };
    dacc(P, 'baseVal', function baseVal() { return nread.call(this); },
      function baseVal(v) {
        if (this._attEl && this._attName) this._attEl.setAttribute(this._attName, String(Number(v))); // probed: 3.5 -> '3.5'
        else this._v = Number(v);
      });
    dacc(P, 'animVal', function animVal() { return nread.call(this); }); // GUESS: mirrors baseVal (string animVal probed to mirror)
    globalThis.__diting_svgAnimNumber = (el, name, def, pair) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name; o._def = def == null ? null : def; o._pair = pair == null ? null : (pair ? 1 : 0);
      return o;
    };
  }

  // ---- SVGAnimatedEnumeration: keyword attr view ---------------------------
  if (typeof SVGAnimatedEnumeration === 'function') {
    const P = SVGAnimatedEnumeration.prototype;
    const eread = function () {
      if (!this._attEl) return this._ov == null ? (this._def || 0) : this._ov;
      if (this._custom) return this._custom(this._attEl); // e.g. marker orientType (probed logic)
      const a = this._attEl.getAttribute(this._attName);
      if (a == null) return this._def;
      const v = this._kw && this._kw[a];
      return v != null ? v : this._def; // junk keyword -> default (probed x3)
    };
    dacc(P, 'baseVal', function baseVal() { return eread.call(this); },
      function baseVal(v) { // GUESS: reverse-map serialize (write path unprobed)
        if (this._attEl && this._kw && !this._custom) {
          let rk = null;
          for (const k in this._kw) if (this._kw[k] === Number(v)) { rk = k; break; }
          if (rk != null) { this._attEl.setAttribute(this._attName, rk); return; }
        }
        this._ov = Number(v);
      });
    dacc(P, 'animVal', function animVal() { return eread.call(this); });
    globalThis.__diting_svgAnimEnum = (el, name, kw, def, custom) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name; o._kw = kw || null; o._def = def || 0; o._custom = custom || null;
      return o;
    };
  }

  // ---- SVGAnimatedString: plain string attr view (in1/result/href) --------
  if (typeof SVGAnimatedString === 'function') {
    const P = SVGAnimatedString.prototype;
    const sread = function () {
      if (!this._attEl) return this._v == null ? '' : this._v;
      const a = this._attEl.getAttribute(this._attName);
      return a == null ? '' : a;
    };
    dacc(P, 'baseVal', function baseVal() { return sread.call(this); },
      function baseVal(v) {
        if (this._attEl && this._attName) this._attEl.setAttribute(this._attName, String(v)); // probed: serializes
        else this._v = String(v);
      });
    dacc(P, 'animVal', function animVal() { return sread.call(this); }); // probed: mirrors baseVal
    globalThis.__diting_svgAnimString = (el, name) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name;
      return o;
    };
  }

  // ---- SVGAnimatedAngle: two angle views of one attr (marker orient) ------
  if (typeof SVGAnimatedAngle === 'function') {
    const P = SVGAnimatedAngle.prototype;
    dacc(P, 'baseVal', function baseVal() { return this._b || (this._b = __diting_svgAngleAttach(this._attEl, this._attName)); });
    dacc(P, 'animVal', function animVal() { return this._a || (this._a = __diting_svgAngleAttach(this._attEl, this._attName)); }); // GUESS: anim mirrors (base/anim both probed [0,1])
    globalThis.__diting_svgAnimAngle = (el, name) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name;
      return o;
    };
  }

  // ---- SVGTransformList: transform-list attr view (patternTransform) ------
  if (typeof SVGTransformList === 'function') {
    const P = SVGTransformList.prototype;
    const parseT = (str) => {
      const out = [];
      const rx = /(matrix|translate|scale|rotate|skewX|skewY)\s*\(([^)]*)\)/g;
      let m;
      while ((m = rx.exec(str || ''))) {
        const a = (m[2] || '').trim().split(/[\s,]+/).filter(Boolean).map(Number);
        const t = __diting_svgTransform();
        if (m[1] === 'matrix' && a.length >= 6) t.setMatrix({ _m: a.slice(0, 6) });
        else if (m[1] === 'translate') t.setTranslate(a[0] || 0, a[1] || 0);
        else if (m[1] === 'scale') t.setScale(a.length ? a[0] : 1, a.length > 1 ? a[1] : (a.length ? a[0] : 1));
        else if (m[1] === 'rotate') t.setRotate(a[0] || 0, a[1] || 0, a[2] || 0); // rotate(a [cx cy]) -> type 4 (probed)
        else if (m[1] === 'skewX') t.setSkewX(a[0] || 0);
        else if (m[1] === 'skewY') t.setSkewY(a[0] || 0);
        out.push(t);
      }
      return out;
    };
    const ser1 = (t) => { // GUESS: write serialization unprobed (read side probed)
      if (t._tt === 2) return 'translate(' + t._tm._m[4] + ',' + t._tm._m[5] + ')';
      if (t._tt === 3) return 'scale(' + t._tm._m[0] + ',' + t._tm._m[3] + ')';
      if (t._tt === 4) return 'rotate(' + (t._ta || 0) + ')';
      if (t._tt === 5) return 'skewX(' + (t._ta || 0) + ')';
      if (t._tt === 6) return 'skewY(' + (t._ta || 0) + ')';
      return 'matrix(' + t._tm._m.join(',') + ')';
    };
    const titems = function () {
      const cur = this._attEl ? (this._attEl.getAttribute(this._attName) || '') : '';
      if (!this._arr || this._lastS !== cur) { this._arr = parseT(cur); this._lastS = cur; } // attr is truth: live re-parse
      return this._arr;
    };
    const tsync = function () {
      if (this._attEl && this._attName) {
        this._attEl.setAttribute(this._attName, (this._arr || []).map(ser1).join(' ')); // GUESS
        this._lastS = this._attEl.getAttribute(this._attName) || '';
      }
    };
    const oorT = (m, i) => new DOMException("Failed to execute '" + m + "' on 'SVGTransformList': The index provided (" + i + ") is out of range.", 'IndexSizeError');
    dacc(P, 'length', function length() { return titems.call(this).length; });
    dacc(P, 'numberOfItems', function numberOfItems() { return titems.call(this).length; });
    dmth(P, 'appendItem', function appendItem(t) { const a = titems.call(this); a.push(t); this._arr = a; tsync.call(this); return t; }); // GUESS
    dmth(P, 'clear', function clear() { this._arr = []; tsync.call(this); }); // GUESS
    dmth(P, 'consolidate', function consolidate() { const a = titems.call(this); return a.length < 2 ? null : a[0]; }); // GUESS: single/no item -> null
    dmth(P, 'createSVGTransformFromMatrix', function createSVGTransformFromMatrix(m) { const t = __diting_svgTransform(); t.setMatrix(m); return t; }); // GUESS
    dmth(P, 'getItem', function getItem(i) {
      const a = titems.call(this);
      if (i < 0 || i >= a.length) throw oorT('getItem', i); // probed: empty list getItem(0) -> IndexSizeError
      return a[i];
    });
    dmth(P, 'initialize', function initialize(t) { this._arr = [t]; tsync.call(this); return t; }); // GUESS
    dmth(P, 'insertItemBefore', function insertItemBefore(t, i) { // GUESS
      const a = titems.call(this);
      a.splice(Math.min(Math.max(i, 0), a.length), 0, t);
      this._arr = a; tsync.call(this);
      return t;
    });
    dmth(P, 'removeItem', function removeItem(i) { // GUESS
      const a = titems.call(this);
      if (i < 0 || i >= a.length) throw oorT('removeItem', i);
      return a.splice(i, 1)[0];
    });
    dmth(P, 'replaceItem', function replaceItem(t, i) { // GUESS
      const a = titems.call(this);
      if (i < 0 || i >= a.length) throw oorT('replaceItem', i);
      a[i] = t; this._arr = a; tsync.call(this);
      return t;
    });
    globalThis.__diting_svgTransformList = (el, name) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name;
      return o;
    };
  }

  // ---- SVGAnimatedTransformList wiring -------------------------------------
  if (typeof SVGAnimatedTransformList === 'function') {
    const P = SVGAnimatedTransformList.prototype;
    dacc(P, 'baseVal', function baseVal() { return this._b || (this._b = __diting_svgTransformList(this._attEl, this._attName)); });
    dacc(P, 'animVal', function animVal() { return this._a || (this._a = __diting_svgTransformList(this._attEl, this._attName)); }); // two objects (probed on sibling types)
    globalThis.__diting_svgAnimTransformList = (el, name) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name;
      return o;
    };
  }

  // ---- SVGStringList: whitespace-token attr view (requiredExtensions…) ----
  if (typeof SVGStringList === 'function') {
    const P = SVGStringList.prototype;
    const stoks = function () {
      const cur = this._attEl ? (this._attEl.getAttribute(this._attName) || '') : '';
      if (!this._arr || this._lastS !== cur) { this._arr = cur === '' ? [] : cur.trim().split(/\s+/).filter(Boolean); this._lastS = cur; } // attr is truth: live re-parse
      return this._arr;
    };
    const ssync = function () {
      if (this._attEl && this._attName) {
        this._attEl.setAttribute(this._attName, (this._arr || []).join(' ')); // GUESS
        this._lastS = this._attEl.getAttribute(this._attName) || '';
      }
    };
    const oorS = (m, i) => new DOMException("Failed to execute '" + m + "' on 'SVGStringList': The index provided (" + i + ") is out of range.", 'IndexSizeError'); // name+format GUESS
    dacc(P, 'length', function length() { return stoks.call(this).length; });
    dacc(P, 'numberOfItems', function numberOfItems() { return stoks.call(this).length; });
    dmth(P, 'appendItem', function appendItem(sv) { const a = stoks.call(this).slice(); a.push(String(sv)); this._arr = a; ssync.call(this); return String(sv); }); // GUESS
    dmth(P, 'clear', function clear() { this._arr = []; ssync.call(this); }); // GUESS
    dmth(P, 'getItem', function getItem(i) { const a = stoks.call(this); if (i < 0 || i >= a.length) throw oorS('getItem', i); return a[i]; });
    dmth(P, 'initialize', function initialize(sv) { this._arr = [String(sv)]; ssync.call(this); return String(sv); }); // GUESS
    dmth(P, 'insertItemBefore', function insertItemBefore(sv, i) { // GUESS
      const a = stoks.call(this).slice();
      a.splice(Math.min(Math.max(i, 0), a.length), 0, String(sv));
      this._arr = a; ssync.call(this);
      return String(sv);
    });
    dmth(P, 'removeItem', function removeItem(i) { // GUESS
      const a = stoks.call(this);
      if (i < 0 || i >= a.length) throw oorS('removeItem', i);
      return a.splice(i, 1)[0];
    });
    dmth(P, 'replaceItem', function replaceItem(sv, i) { // GUESS
      const a = stoks.call(this);
      if (i < 0 || i >= a.length) throw oorS('replaceItem', i);
      a[i] = String(sv); this._arr = a; ssync.call(this);
      return String(sv);
    });
    globalThis.__diting_svgStringList = (el, name) => {
      const o = Object.create(P);
      o._attEl = el || null; o._attName = name;
      return o;
    };
  }

  // ---- 刀63 element-face helpers -------------------------------------------
  // Wipe a minted element prototype (keeping named descriptors to re-install
  // in Chrome's slot) and re-add the constructor LAST, so raw order equals
  // truth and gen_shape_fix emits no ORDER row.
  const wipeFace = (P, keep) => {
    const save = {};
    for (const k of Object.getOwnPropertyNames(P)) {
      if (keep && keep.indexOf(k) >= 0) {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (d) save[k] = d;
      }
      if (k !== 'constructor') {
        const d = Object.getOwnPropertyDescriptor(P, k);
        if (!d || d.configurable) delete P[k];
      }
    }
    return save;
  };
  const ctorLast = (P) => {
    const d = Object.getOwnPropertyDescriptor(P, 'constructor');
    if (d && d.configurable) { delete P.constructor; Object.defineProperty(P, 'constructor', d); }
  };
  const animGet = (P, n, mk) =>
    dacc(P, n, function () { const c = this._a63 || (this._a63 = {}); return c[n] || (c[n] = mk(this)); });

  // ---- SVGMarkerElement ----------------------------------------------------
  if (typeof SVGMarkerElement === 'function') {
    const P = SVGMarkerElement.prototype;
    const save = wipeFace(P, ['viewBox']);
    // orientType (probed): absent/'0'-ish/numeric/junk -> 2 ANGLE (SVG 1.1
    // default '0'), 'auto' -> 1, 'auto-start-reverse' -> 0.
    const orientType = (el) => {
      const a = el.getAttribute('orient');
      if (a === 'auto') return 1;
      if (a === 'auto-start-reverse') return 0;
      return 2;
    };
    animGet(P, 'refX', (el) => __diting_svgAnimLength(el, 'refX', [0, 1]));
    animGet(P, 'refY', (el) => __diting_svgAnimLength(el, 'refY', [0, 1]));
    animGet(P, 'markerUnits', (el) => __diting_svgAnimEnum(el, 'markerUnits', { userSpaceOnUse: 1, strokeWidth: 2, 'stroke-width': 2 }, 2));
    animGet(P, 'markerWidth', (el) => __diting_svgAnimLength(el, 'markerWidth', [3, 1]));
    animGet(P, 'markerHeight', (el) => __diting_svgAnimLength(el, 'markerHeight', [3, 1]));
    animGet(P, 'orientType', (el) => __diting_svgAnimEnum(el, 'orient', null, 2, orientType));
    animGet(P, 'orientAngle', (el) => __diting_svgAnimAngle(el, 'orient'));
    if (save.viewBox) Object.defineProperty(P, 'viewBox', save.viewBox);
    animGet(P, 'preserveAspectRatio', (el) => __diting_svgAnimPAR(el));
    const MK_U = [['SVG_MARKERUNITS_UNKNOWN', 0], ['SVG_MARKERUNITS_USERSPACEONUSE', 1], ['SVG_MARKERUNITS_STROKEWIDTH', 2]];
    const MK_O = [['SVG_MARKER_ORIENT_UNKNOWN', 0], ['SVG_MARKER_ORIENT_AUTO', 1], ['SVG_MARKER_ORIENT_ANGLE', 2]];
    for (const [n, v] of MK_U.concat(MK_O)) {
      dval(P, n, v, { enumerable: true });
      dval(SVGMarkerElement, n, v, { enumerable: true });
    }
    dmth(P, 'setOrientToAngle', function setOrientToAngle(angle) {
      if (!(angle instanceof SVGAngle)) throw new TypeError("Failed to execute 'setOrientToAngle' on 'SVGMarkerElement': parameter 1 is not of type 'SVGAngle'."); // name probed (numeric arg), message GUESS
      this.setAttribute('orient', angle.valueAsString); // probed: 45 -> '45', 30deg -> '30deg'
    });
    dmth(P, 'setOrientToAuto', function setOrientToAuto() { this.setAttribute('orient', 'auto'); }); // probed
    ctorLast(P);
  }

  // ---- SVGAnimationElement -------------------------------------------------
  if (typeof SVGAnimationElement === 'function') {
    const P = SVGAnimationElement.prototype;
    wipeFace(P);
    const DURRX = /^\s*(\d+(?:\.\d+)?|\.\d+)(ms|s|m|h)?\s*$/;
    dacc(P, 'targetElement', function targetElement() {
      // probed: bare, in-parent and dangling href all -> null; only a live
      // #id resolve is GUESS
      let h = this.getAttribute('href');
      if (h == null) h = this.getAttributeNS('http://www.w3.org/1999/xlink', 'href');
      if (h && h.charCodeAt(0) === 35) {
        const doc = this.ownerDocument;
        const el = doc && doc.getElementById ? doc.getElementById(h.slice(1)) : null;
        return el || null;
      }
      return null;
    });
    const onacc = (n) => dacc(P, 'on' + n,
      function () { const v = (this._a63 || (this._a63 = {}))['on' + n]; return typeof v === 'function' ? v : null; },
      function (v) { (this._a63 || (this._a63 = {}))['on' + n] = typeof v === 'function' ? v : null; }); // probed: non-function coerces to null
    onacc('begin'); onacc('end'); onacc('repeat');
    animGet(P, 'requiredExtensions', (el) => __diting_svgStringList(el, 'requiredExtensions'));
    animGet(P, 'systemLanguage', (el) => __diting_svgStringList(el, 'systemLanguage'));
    dmth(P, 'beginElement', function beginElement() {}); // probed: no-op -> undefined
    dmth(P, 'beginElementAt', function beginElementAt(offset) {}); // probed: no-op
    dmth(P, 'endElement', function endElement() {}); // probed: no-op
    dmth(P, 'endElementAt', function endElementAt(offset) {}); // probed: no-op
    dmth(P, 'getCurrentTime', function getCurrentTime() { return 0; }); // probed
    dmth(P, 'getSimpleDuration', function getSimpleDuration() { // probed: '5s' -> 5; absent/'indefinite' -> NotSupportedError
      const m = DURRX.exec(String(this.getAttribute('dur') == null ? '' : this.getAttribute('dur')));
      if (!m) throw new DOMException("Failed to execute 'getSimpleDuration' on 'SVGAnimationElement': No simple duration defined.", 'NotSupportedError');
      const u = m[2];
      const v = Number(m[1]);
      return u === 'ms' ? v / 1000 : u === 'm' ? v * 60 : u === 'h' ? v * 3600 : v; // clock format GUESS
    });
    dmth(P, 'getStartTime', function getStartTime() {
      throw new DOMException("Failed to execute 'getStartTime' on 'SVGAnimationElement': No current interval.", 'InvalidStateError'); // probed verbatim
    });
    ctorLast(P);
  }

  // ---- SVGTextContentElement -----------------------------------------------
  if (typeof SVGTextContentElement === 'function') {
    const P = SVGTextContentElement.prototype;
    wipeFace(P);
    animGet(P, 'textLength', (el) => __diting_svgAnimLength(el, 'textLength', [0, 1]));
    animGet(P, 'lengthAdjust', (el) => __diting_svgAnimEnum(el, 'lengthAdjust', { spacing: 1, spacingAndGlyphs: 2 }, 1));
    for (const [n, v] of [['LENGTHADJUST_UNKNOWN', 0], ['LENGTHADJUST_SPACING', 1], ['LENGTHADJUST_SPACINGANDGLYPHS', 2]]) {
      dval(P, n, v, { enumerable: true });
      dval(SVGTextContentElement, n, v, { enumerable: true });
    }
    // No text layout in the engine: nchars is always 0 (probed: even with
    // textContent 'abc' both engines report 0 — engines agree by design).
    dmth(P, 'getCharNumAtPosition', function getCharNumAtPosition(point) { return -1; }); // probed
    dmth(P, 'getComputedTextLength', function getComputedTextLength() { return 0; }); // probed
    const charOOR = (m) => new DOMException("Failed to execute '" + m + "' on 'SVGTextContentElement': The charnum provided (0) is out of range.", 'IndexSizeError'); // format GUESS (throw probed)
    dmth(P, 'getEndPositionOfChar', function getEndPositionOfChar(n) { throw charOOR('getEndPositionOfChar'); }); // probed: throws on nchars 0
    dmth(P, 'getExtentOfChar', function getExtentOfChar(n) { throw charOOR('getExtentOfChar'); }); // probed: throws
    dmth(P, 'getNumberOfChars', function getNumberOfChars() { return 0; }); // probed
    dmth(P, 'getRotationOfChar', function getRotationOfChar(n) { throw charOOR('getRotationOfChar'); }); // probed: throws
    dmth(P, 'getStartPositionOfChar', function getStartPositionOfChar(n) { throw charOOR('getStartPositionOfChar'); }); // probed: throws
    dmth(P, 'getSubStringLength', function getSubStringLength(charnum, nchars) { // probed: (0,0) -> 0, (0,5)/(5,0) -> throw
      if (charnum === 0 && nchars === 0) return 0;
      throw charOOR('getSubStringLength');
    });
    dmth(P, 'selectSubString', function selectSubString(charnum, nchars) { throw charOOR('selectSubString'); }); // probed: throws even (0,0)
    ctorLast(P);
  }

  // ---- SVGFEDropShadowElement ----------------------------------------------
  if (typeof SVGFEDropShadowElement === 'function') {
    const P = SVGFEDropShadowElement.prototype;
    // 刀62 lesson: cut the empty intermediate duplicate shell so the parent
    // audit reads SVGElement (engine had a dead double-prototype here).
    Object.setPrototypeOf(P, SVGElement.prototype);
    wipeFace(P);
    animGet(P, 'in1', (el) => __diting_svgAnimString(el, 'in'));
    animGet(P, 'dx', (el) => __diting_svgAnimNumber(el, 'dx', 2, false));
    animGet(P, 'dy', (el) => __diting_svgAnimNumber(el, 'dy', 2, false));
    animGet(P, 'stdDeviationX', (el) => __diting_svgAnimNumber(el, 'stdDeviation', 2, 0)); // 'x y' pair, X half
    animGet(P, 'stdDeviationY', (el) => __diting_svgAnimNumber(el, 'stdDeviation', 2, 1));
    animGet(P, 'x', (el) => __diting_svgAnimLength(el, 'x', [0, 2], true)); // probed: FE region % never resolves
    animGet(P, 'y', (el) => __diting_svgAnimLength(el, 'y', [0, 2], true));
    animGet(P, 'width', (el) => __diting_svgAnimLength(el, 'width', [0, 2], true));
    animGet(P, 'height', (el) => __diting_svgAnimLength(el, 'height', [0, 2], true));
    animGet(P, 'result', (el) => __diting_svgAnimString(el, 'result'));
    dmth(P, 'setStdDeviation', function setStdDeviation(x, y) {
      const nx = Number(x), ny = Number(y);
      if (typeof x === 'undefined' || typeof y === 'undefined' || !isFinite(nx) || !isFinite(ny)) {
        // probed: missing args and 'a'/'b' -> TypeError (restricted float)
        throw new TypeError("Failed to execute 'setStdDeviation' on 'SVGFEDropShadowElement': The provided double value is non-finite.");
      }
      this._sdOv = [nx, ny]; // probed: internal override, attr NOT serialized
    });
    ctorLast(P);
  }

  // ---- SVGPatternElement ---------------------------------------------------
  if (typeof SVGPatternElement === 'function') {
    const P = SVGPatternElement.prototype;
    const save = wipeFace(P, ['viewBox', 'href']);
    const PU = { userSpaceOnUse: 1, objectBoundingBox: 2 };
    animGet(P, 'patternUnits', (el) => __diting_svgAnimEnum(el, 'patternUnits', PU, 2)); // probed: default 2, junk -> 2
    animGet(P, 'patternContentUnits', (el) => __diting_svgAnimEnum(el, 'patternContentUnits', PU, 1)); // probed: default 1
    animGet(P, 'patternTransform', (el) => __diting_svgAnimTransformList(el, 'patternTransform'));
    animGet(P, 'x', (el) => __diting_svgAnimLength(el, 'x', [0, 1])); // per-attach defaults override 刀59's width/height heuristic
    animGet(P, 'y', (el) => __diting_svgAnimLength(el, 'y', [0, 1]));
    animGet(P, 'width', (el) => __diting_svgAnimLength(el, 'width', [0, 1]));
    animGet(P, 'height', (el) => __diting_svgAnimLength(el, 'height', [0, 1]));
    if (save.viewBox) Object.defineProperty(P, 'viewBox', save.viewBox);
    animGet(P, 'preserveAspectRatio', (el) => __diting_svgAnimPAR(el));
    animGet(P, 'href', (el) => __diting_svgAnimString(el, 'href')); // truth ro:ec — Chrome returns an SVGAnimatedString view (was a stale rw shell)
    animGet(P, 'requiredExtensions', (el) => __diting_svgStringList(el, 'requiredExtensions'));
    animGet(P, 'systemLanguage', (el) => __diting_svgStringList(el, 'systemLanguage'));
    ctorLast(P);
  }

})();
