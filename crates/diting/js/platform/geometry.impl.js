//! module: geometry
//! layer: L2
//! exposes: WebKitCSSMatrix
//! requires-load: -
//! requires-call: legacy:DOMException, legacy:_markNative

// Geometry behavior only; the interface SHAPE (classes, descriptor kinds,
// install order, arity, @@toStringTag, statics, inheritance) is generated
// from WebIDL + the Chrome truth overlay in gen/geometry.gen.js. This module
// supplies the math and the Chrome-shaped errors; every fact below that
// could be misremembered is pinned by a key in
// tools/webplatform/chrome/geometry-truth-145.json (captured from real
// Chrome 145) and diffed by the geometry_stack_matches_chrome_truth test.
//
// Storage layout (all Float64Array):
//   matrix: 16 slots, m_{C}{R} at 4*(C-1)+(R-1)  — column-major like the
//           toFloat64Array face: [m11,m12,m13,m14, m21,...] (truth
//           ro.toFloat64Array). a/b/c/d/e/f alias m11/m12/m21/m22/m41/m42.
//   point:  [x,y,z,w]    rect: [x,y,width,height]
//   quad:   four DOMPoint instances (p1..p4 return the same object).
// One WeakMap per FAMILY (RO class and mutable class share it): the
// inherited DOMPointReadOnly getters run with a DOMPoint receiver.
'use strict';

const BAG = { point: new WeakMap(), rect: new WeakMap(), quad: new WeakMap(), matrix: new WeakMap() };
function bag(iface) {
  return BAG[/Matrix/.test(iface) ? 'matrix' : /Point/.test(iface) ? 'point'
    : /Rect/.test(iface) ? 'rect' : 'quad'];
}

// current API prefix for the throw helpers (set at every B.* entry)
let CTX = '';

// ---- coercion helpers (Chrome message wording) -----------------------
function num(v) {
  if (typeof v === 'symbol') P.fail(num, 'Cannot convert a Symbol value to a number', CTX);
  return Number(v);
}
function num2str(v) {
  if (typeof v === 'symbol') P.fail(num2str, 'Cannot convert a Symbol value to a string', CTX);
  return String(v);
}
// dictionary conversion: null/undefined -> fresh defaults; primitives fail.
function asDict(v, dictName) {
  if (v === null || v === undefined) return {};
  if (typeof v !== 'object' && typeof v !== 'function') {
    P.fail(asDict, "The provided value is not of type '" + dictName + "'.", CTX);
  }
  return v;
}
function dictNum(d, k, dflt, dictName) {
  const v = d[k];
  if (v === undefined || v === null) return dflt;
  if (typeof v === 'symbol') {
    P.fail(dictNum, 'Cannot convert a Symbol value to a number',
      CTX + ": Failed to read the '" + k + "' property from '" + dictName + "'");
  }
  return Number(v);
}
function domFail(msg) {
  // Chrome raises DOMException/SyntaxError for transform-list parse failures
  // with the standard "<prefix>: <message>" decorated message (no trailing
  // period on the resolvable-at-parse-time one - truth smv.percent).
  const e = new DOMException(msg, 'SyntaxError');
  Error.captureStackTrace(e, domFail);
  Object.defineProperty(e, 'message',
    { value: CTX + ': ' + msg, writable: true, enumerable: true, configurable: true });
  throw e;
}

// ---- matrix primitives ------------------------------------------------
const ID16 = () => { const m = new Float64Array(16); m[0] = m[5] = m[10] = m[15] = 1; return m; };
const isIdentity16 = (m) => {
  for (let i = 0; i < 16; i++) {
    if (m[i] !== ((i === 0 || i === 5 || i === 10 || i === 15) ? 1 : 0)) return false;
  }
  return true;
};
// A·B in m_{CR} storage: C[4c+r] = Σ_k A[4k+r]·B[4c+k]
function mul(a, b) {
  const c = new Float64Array(16);
  for (let col = 0; col < 4; col++) {
    for (let row = 0; row < 4; row++) {
      let t = 0;
      for (let k = 0; k < 4; k++) t += a[4 * k + row] * b[4 * col + k];
      c[4 * col + row] = t;
    }
  }
  return c;
}
// exact values at right angles — Chrome's rotate(90) answers a crisp 0, not
// cos(π/2) = 6.1e-17 (truth ro.rotate / ro.rotate270 / dm.smv.rotate).
function cosSin(deg) {
  if (isFinite(deg)) {
    const k = deg / 90;
    if (Number.isInteger(k)) return [[1, 0], [0, 1], [-1, 0], [0, -1]][((k % 4) + 4) % 4];
  }
  const rad = deg * Math.PI / 180;
  return [Math.cos(rad), Math.sin(rad)];
}
const D2R = Math.PI / 180;
function rotZM(deg) {
  const [c, s] = cosSin(deg);
  const m = ID16();
  m[0] = c; m[1] = s; m[4] = -s; m[5] = c;
  return m;
}
function rotXM(deg) {
  const [c, s] = cosSin(deg);
  const m = ID16();
  m[5] = c; m[6] = s; m[9] = -s; m[10] = c;
  return m;
}
function rotYM(deg) {
  const [c, s] = cosSin(deg);
  const m = ID16();
  m[0] = c; m[2] = -s; m[8] = s; m[10] = c;
  return m;
}
function axisAngleM(x, y, z, deg) {
  // standard rotation matrix R stored m_{C}{R} = R[R-1][C-1]; a zero axis
  // is the identity (CSS rotate3d(0,0,0,θ)).
  const h = Math.sqrt(x * x + y * y + z * z);
  const m = ID16();
  if (h === 0) return m;
  const ax = x / h, ay = y / h, az = z / h;
  const [c, s] = cosSin(deg);
  const t = 1 - c;
  m[0] = t * ax * ax + c;       m[1] = t * ax * ay + s * az;  m[2] = t * ax * az - s * ay;
  m[4] = t * ax * ay - s * az;  m[5] = t * ay * ay + c;       m[6] = t * ay * az + s * ax;
  m[8] = t * ax * az + s * ay;  m[9] = t * ay * az - s * ax;  m[10] = t * az * az + c;
  return m;
}
function translation(tx, ty, tz) {
  const m = ID16();
  m[12] = tx; m[13] = ty; m[14] = tz;
  return m;
}
function scaleMatrix(x, y, z) {
  const m = ID16();
  m[0] = x; m[5] = y; m[10] = z;
  return m;
}
function post(st, m) { st.m = mul(st.m, m); }
// exact M·T as a structural op: only the last column changes. Full matmul
// against a translation is algebraically the same but multiplies the OTHER
// columns by T's structural zeros — 0·NaN = NaN would bleed a NaN factor
// across the whole matrix (IEEE), where Blink's in-place Translate does not
// (truth ro.scale.z: scale(2, undefined, 3) keeps a = 2, only sy's column NaN).
function postTranslate(st, tx, ty, tz) {
  for (let r = 0; r < 4; r++) {
    st.m[12 + r] += st.m[r] * tx + st.m[4 + r] * ty + st.m[8 + r] * tz;
  }
}

// ---- DOMMatrixInit / DOMPointInit / DOMRectInit ------------------------
const ALIAS = { a: 0, b: 1, c: 4, d: 5, e: 12, f: 13, m11: 0, m12: 1, m13: 2, m14: 3, m21: 4, m22: 5, m23: 6, m24: 7, m31: 8, m32: 9, m33: 10, m34: 11, m41: 12, m42: 13, m43: 14, m44: 15 };
const IS_2D_SLOT = [0, 1, 4, 5, 12, 13];
const M3D_NAMES = ['m13', 'm14', 'm23', 'm24', 'm31', 'm32', 'm33', 'm34', 'm43', 'm44'];

function readMatrixInit(d) {
  // 3D-ness is VALUE-based against each member's DICTIONARY DEFAULT
  // (m33/m44 default to 1, the rest to 0): a live DOMMatrix instance has
  // every member defined (getters), and m13 present AS 0 or m44 AS 1 keeps
  // the matrix 2D (truth dm.fromMatrix / dm.multiplySelf with real
  // matrices; a wrong m44!==0 check read every live 2D matrix as 3D).
  const val = (k) => (d[k] === undefined || d[k] === null ? undefined : dictNum(d, k, 0, 'DOMMatrixInit'));
  let has3D = false;
  for (const k of M3D_NAMES) {
    const dflt = k === 'm33' || k === 'm44' ? 1 : 0;
    if (val(k) !== undefined && val(k) !== dflt) { has3D = true; break; }
  }
  let is2D;
  if (d.is2D !== undefined) {
    is2D = !!d.is2D;
    if (is2D && has3D) {
      P.fail(readMatrixInit, 'The is2D member is set to true but the input matrix is a 3d matrix.', CTX);
    }
  } else {
    is2D = !has3D; // nothing 3D-valued present (or nothing at all) -> 2D (truth dm.fromMatrix.empty)
  }
  const m = ID16();
  for (const k of Object.keys(ALIAS)) {
    if (d[k] !== undefined) m[ALIAS[k]] = dictNum(d, k, 0, 'DOMMatrixInit');
  }
  return { m, is2D };
}
function readPointInit(d) {
  return [dictNum(d, 'x', 0, 'DOMPointInit'), dictNum(d, 'y', 0, 'DOMPointInit'),
    dictNum(d, 'z', 0, 'DOMPointInit'), dictNum(d, 'w', 1, 'DOMPointInit')];
}
function readRectInit(d) {
  return [dictNum(d, 'x', 0, 'DOMRectInit'), dictNum(d, 'y', 0, 'DOMRectInit'),
    dictNum(d, 'width', 0, 'DOMRectInit'), dictNum(d, 'height', 0, 'DOMRectInit')];
}

// ---- minting through the public constructors --------------------------
// The generated shells run for real, so every minted instance carries the
// right brand/prototype and its state lands in the family bag.
function mintMatrix(cls, m, is2D) {
  const inst = new globalThis[cls]();
  BAG.matrix.set(inst, { m, is2D });
  return inst;
}
// cls: which face the minted point wears — DOMPointReadOnly.fromPoint must
// hand back a DOMPointReadOnly, not a mutable DOMPoint (truth dpro.fromPoint).
function mintPoint(cls, v) { const p = new globalThis[cls](v[0], v[1], v[2], v[3]); return p; }
function mintRect(v) { return new globalThis.DOMRect(v[0], v[1], v[2], v[3]); }

// ---- CSS transform-list parser (setMatrixValue / string ctors) ---------
// Chrome facts pinned by truth keys: '' and 'none' -> identity; percent (and
// any non-px resolvable-later length) -> DOMException "Values must be
// resolvable at parse time" (NO trailing period); calc() of px terms IS
// resolved; numbers accept exponent notation; angle units deg/rad/grad/turn
// with unitless only valid for 0; trailing garbage fails with the WHOLE
// original string in "Failed to parse '<s>'."; is2D = no 3D-only function
// appeared (rotate3d with a z-only axis is still 3D - truth dm.smv.rotate3d).
const KIND = {
  matrix: 'n', matrix3d: 'n',
  scale: 'n', scalex: 'n', scaley: 'n', scalez: 'n', scale3d: 'n',
  rotate: 'a', rotatex: 'a', rotatey: 'a', rotatez: 'a',
  skew: 'a', skewx: 'a', skewy: 'a',
  translate: 'l', translatex: 'l', translatey: 'l', translatez: 'l', translate3d: 'l',
  perspective: 'l',
};
const FORCE3D = { matrix3d: 1, translate3d: 1, translatez: 1, scale3d: 1, scalez: 1, rotate3d: 1, rotatex: 1, rotatey: 1, rotatez: 1, perspective: 1 };

function parseTransformList(str) {
  const s = String(str);
  const t = s.trim();
  if (t === '' || t.toLowerCase() === 'none') return { m: ID16(), is2D: true };
  let i = 0;
  const n = t.length;
  let acc = null;
  let any3D = false;

  const ws = () => { while (i < n && /\s/.test(t[i])) i++; };
  const resolvable = () => domFail('Values must be resolvable at parse time');
  const parseFail = () => domFail("Failed to parse '" + s + "'.");

  function number() {
    const start = i;
    if (t[i] === '+' || t[i] === '-') i++;
    let digits = 0;
    while (i < n && t[i] >= '0' && t[i] <= '9') { i++; digits++; }
    if (t[i] === '.') { i++; while (i < n && t[i] >= '0' && t[i] <= '9') { i++; digits++; } }
    if (!digits) { i = start; return null; }
    if (t[i] === 'e' || t[i] === 'E') {
      const save = i; i++;
      if (t[i] === '+' || t[i] === '-') i++;
      let ed = 0;
      while (i < n && t[i] >= '0' && t[i] <= '9') { i++; ed++; }
      if (!ed) i = save;
    }
    return parseFloat(t.slice(start, i));
  }
  function unit() {
    const start = i;
    while (i < n && /[a-zA-Z%]/.test(t[i])) i++;
    return t.slice(start, i);
  }
  // <length>: px (or unitless 0); anything font/viewport/percent-dependent
  // is "not resolvable at parse time".
  function lengthValue() {
    if (t.slice(i, i + 5).toLowerCase() === 'calc(') return calcValue();
    const v = number();
    if (v === null) return null;
    const u = unit();
    if (u === '') return v === 0 ? 0 : null;
    if (u === 'px') return v;
    resolvable();
  }
  function angleDeg() {
    if (t.slice(i, i + 5).toLowerCase() === 'calc(') return calcValue();
    const v = number();
    if (v === null) return null;
    const u = unit().toLowerCase();
    if (u === '') return v === 0 ? 0 : null;
    if (u === 'deg') return v;
    if (u === 'rad') return v / D2R;
    if (u === 'grad') return v * 0.9;
    if (u === 'turn') return v * 360;
    return null;
  }
  function plainNumber() {
    if (t.slice(i, i + 5).toLowerCase() === 'calc(') return calcValue();
    const v = number();
    if (v !== null && unit() === '') return v;
    return null;
  }
  // calc( <px-or-number terms with + - * / and parens> ) — enough for the
  // resolvable-at-parse-time subset Chrome accepts (truth smv.calc).
  function calcValue() {
    i += 5; // past 'calc('
    const val = calcExpr();
    ws();
    if (val === null || t[i] !== ')') parseFail();
    i++;
    return val;
  }
  function calcExpr() {
    let v = calcTerm();
    for (;;) {
      ws();
      const op = t[i];
      if (op === '+' || op === '-') {
        i++;
        const r = calcTerm();
        if (v === null || r === null) return null;
        v = op === '+' ? v + r : v - r;
      } else return v;
    }
  }
  function calcTerm() {
    ws();
    let v = calcPrim();
    for (;;) {
      ws();
      const op = t[i];
      if (op === '*' || op === '/') {
        i++;
        const r = calcPrim();
        if (v === null || r === null) return null;
        v = op === '*' ? v * r : v / r;
      } else return v;
    }
  }
  function calcPrim() {
    if (t[i] === '(') {
      i++;
      const v = calcExpr();
      ws();
      if (t[i] !== ')') return null;
      i++;
      return v;
    }
    const v = number();
    if (v === null) return null;
    const u = unit();
    if (u === '' || u === 'px') return v;
    resolvable();
  }

  function fnMatrix(name) {
    // returns Float64Array (the function's matrix) or null on any arg shape
    // mismatch; 3D-only functions are flagged by the caller via FORCE3D.
    const args = [];
    for (;;) {
      ws();
      const kind = name === 'rotate3d' ? (args.length === 3 ? 'a' : 'n') : KIND[name];
      const readFn = kind === 'n' ? plainNumber : kind === 'a' ? angleDeg : lengthValue;
      const v = readFn();
      if (v === null) return null;
      args.push(v);
      ws();
      if (t[i] === ',') { i++; continue; }
      break;
    }
    if (t[i] !== ')') return null;
    i++;
    const m = ID16();
    switch (name) {
      case 'matrix':
        if (args.length !== 6) return null;
        m[0] = args[0]; m[1] = args[1]; m[4] = args[2]; m[5] = args[3]; m[12] = args[4]; m[13] = args[5];
        return m;
      case 'matrix3d':
        if (args.length !== 16) return null;
        for (let k = 0; k < 16; k++) m[k] = args[k];
        return m;
      case 'translate':
        if (args.length < 1 || args.length > 2) return null;
        m[12] = args[0]; m[13] = args.length > 1 ? args[1] : 0;
        return m;
      case 'translatex': case 'translatey': case 'translatez':
        if (args.length !== 1) return null;
        m[name === 'translatex' ? 12 : name === 'translatey' ? 13 : 14] = args[0];
        return m;
      case 'translate3d':
        if (args.length !== 3) return null;
        m[12] = args[0]; m[13] = args[1]; m[14] = args[2];
        return m;
      case 'scale':
        if (args.length < 1 || args.length > 3) return null;
        m[0] = args[0]; m[5] = args.length > 1 ? args[1] : args[0]; m[10] = args.length > 2 ? args[2] : 1;
        return m;
      case 'scalex': case 'scaley': case 'scalez':
        if (args.length !== 1) return null;
        m[name === 'scalex' ? 0 : name === 'scaley' ? 5 : 10] = args[0];
        return m;
      case 'scale3d':
        if (args.length !== 3) return null;
        m[0] = args[0]; m[5] = args[1]; m[10] = args[2];
        return m;
      case 'rotate':
        if (args.length !== 1) return null;
        return rotZM(args[0]);
      case 'rotatex':
        if (args.length !== 1) return null;
        return rotXM(args[0]);
      case 'rotatey':
        if (args.length !== 1) return null;
        return rotYM(args[0]);
      case 'rotatez':
        if (args.length !== 1) return null;
        return rotZM(args[0]);
      case 'rotate3d':
        if (args.length !== 4) return null;
        return axisAngleM(args[0], args[1], args[2], args[3]);
      case 'skewx':
        if (args.length !== 1) return null;
        m[4] = Math.tan(args[0] * D2R);
        return m;
      case 'skewy':
        if (args.length !== 1) return null;
        m[1] = Math.tan(args[0] * D2R);
        return m;
      case 'skew':
        if (args.length < 1 || args.length > 2) return null;
        m[4] = Math.tan(args[0] * D2R);
        if (args.length > 1) m[1] = Math.tan(args[1] * D2R);
        return m;
      case 'perspective':
        if (args.length !== 1) return null;
        if (args[0] !== 0) m[11] = -1 / args[0]; // d=0: identity (CSS)
        return m;
      default:
        return null;
    }
  }

  for (;;) {
    ws();
    if (i >= n) break;
    const start = i;
    while (i < n && /[a-zA-Z0-9]/.test(t[i])) i++;
    const name = t.slice(start, i).toLowerCase();
    ws();
    if (!name || t[i] !== '(') parseFail();
    i++;
    if (KIND[name] === undefined && name !== 'rotate3d') parseFail();
    const fm = fnMatrix(name);
    if (fm === null) parseFail();
    if (FORCE3D[name]) any3D = true;
    acc = acc ? mul(acc, fm) : fm;
  }
  if (!acc) parseFail();
  return { m: acc, is2D: !any3D };
}

// ---- sequence ctor leg (Array / iterable / typed arrays) --------------
function fromSequence(v) {
  const items = [];
  if (Array.isArray(v)) {
    for (let k = 0; k < v.length; k++) items.push(num(v[k]));
  } else {
    try {
      for (const x of v) items.push(num(x));
    } catch (e) { return null; }
  }
  if (items.length !== 6 && items.length !== 16) {
    P.fail(fromSequence, 'The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix.', CTX);
  }
  const m = ID16();
  if (items.length === 6) {
    m[0] = items[0]; m[1] = items[1]; m[4] = items[2]; m[5] = items[3]; m[12] = items[4]; m[13] = items[5];
    return { m, is2D: true };
  }
  const m16 = new Float64Array(16);
  for (let k = 0; k < 16; k++) m16[k] = items[k];
  return { m: m16, is2D: false };
}

// ---- generic 4x4 inverse (adjugate); singular/NaN -> null --------------
function inverse16(m) {
  const idx = (r, c) => 4 * c + r;
  const sub3 = (rows, cols) => {
    const s = (rr, cc) => m[idx(rows[rr], cols[cc])];
    return s(0, 0) * (s(1, 1) * s(2, 2) - s(1, 2) * s(2, 1))
      - s(0, 1) * (s(1, 0) * s(2, 2) - s(1, 2) * s(2, 0))
      + s(0, 2) * (s(1, 0) * s(2, 1) - s(1, 1) * s(2, 0));
  };
  let det = 0;
  for (let c = 0; c < 4; c++) {
    const rest = [0, 1, 2, 3].filter((x) => x !== c);
    det += (c % 2 === 0 ? 1 : -1) * m[idx(0, c)] * sub3([1, 2, 3], rest);
  }
  if (det === 0 || !isFinite(det)) return null;
  const out = new Float64Array(16);
  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      const rows = [0, 1, 2, 3].filter((x) => x !== r);
      const cols = [0, 1, 2, 3].filter((x) => x !== c);
      const cof = sub3(rows, cols);
      out[idx(c, r)] = ((r + c) % 2 === 0 ? cof : -cof) / det;
    }
  }
  return out;
}

// ---- argument readers (WebIDL defaults vs no-default NaN law) ----------
// Truth-pinned: tz (declared =0) with explicit undefined -> 0; scaleY /
// rotY / rotZ (NO declared default) missing -> spec prose, explicit
// undefined -> NaN (ro.scale.z / ro.rotate.undef).
function argNum(args, i, dflt) {
  const v = args[i];
  return v === undefined ? dflt : num(v);
}
function argNumNoDflt(args, i, proseDflt) {
  // parameter with no IDL default: MISSING -> spec prose default,
  // PRESENT-undefined -> NaN (ToNumber(undefined))
  return args.length > i ? num(args[i]) : proseDflt;
}

// ---- ctor / get / set / call / stat ------------------------------------
function ctor(iface, self, ctx, args) {
  CTX = ctx;
  switch (iface) {
    case 'DOMPointReadOnly': case 'DOMPoint':
      BAG.point.set(self, [argNum(args, 0, 0), argNum(args, 1, 0), argNum(args, 2, 0), argNum(args, 3, 1)]);
      return;
    case 'DOMRectReadOnly': case 'DOMRect':
      BAG.rect.set(self, [argNum(args, 0, 0), argNum(args, 1, 0), argNum(args, 2, 0), argNum(args, 3, 0)]);
      return;
    case 'DOMQuad': {
      const pts = [];
      for (let k = 0; k < 4; k++) {
        pts.push(mintPoint('DOMPoint', readPointInit(asDict(args[k], 'DOMPointInit'))));
      }
      BAG.quad.set(self, pts);
      return;
    }
    case 'DOMMatrixReadOnly': case 'DOMMatrix': {
      const init = args[0];
      let st;
      if (init === undefined || init === null) st = { m: ID16(), is2D: true };
      else if (typeof init === 'string') st = parseTransformList(init);
      else if (typeof init[Symbol.iterator] === 'function') st = fromSequence(init);
      else if (typeof init === 'object' && typeof init.m11 === 'number') st = readMatrixInit(init); // copy ctor
      else st = parseTransformList(num2str(init));
      BAG.matrix.set(self, st);
      return;
    }
  }
}

// x' = m11x+m21y+m31z+m41w ... — no division by w (truth dp.mt)
function transform4(m, p) {
  const [x, y, z, w] = p;
  return [m[0] * x + m[4] * y + m[8] * z + m[12] * w,
    m[1] * x + m[5] * y + m[9] * z + m[13] * w,
    m[2] * x + m[6] * y + m[10] * z + m[14] * w,
    m[3] * x + m[7] * y + m[11] * z + m[15] * w];
}

function get(iface, member, self) {
  const st = bag(iface).get(self);
  if (st === undefined) P.illegal(get);
  if (ALIAS[member] !== undefined && /Matrix/.test(iface)) return st.m[ALIAS[member]];
  switch (iface + '.' + member) {
    case 'DOMMatrixReadOnly.is2D': return st.is2D;
    case 'DOMMatrixReadOnly.isIdentity': return isIdentity16(st.m);
    // point/rect: BOTH classes dispatch here (the mutable class owns
    // accessors that call B.get with its own iface string)
    case 'DOMPointReadOnly.x': case 'DOMPoint.x': return st[0];
    case 'DOMPointReadOnly.y': case 'DOMPoint.y': return st[1];
    case 'DOMPointReadOnly.z': case 'DOMPoint.z': return st[2];
    case 'DOMPointReadOnly.w': case 'DOMPoint.w': return st[3];
    case 'DOMRectReadOnly.x': case 'DOMRect.x': return st[0];
    case 'DOMRectReadOnly.y': case 'DOMRect.y': return st[1];
    case 'DOMRectReadOnly.width': case 'DOMRect.width': return st[2];
    case 'DOMRectReadOnly.height': case 'DOMRect.height': return st[3];
    // normalized getters (truth dr.neg: top=min(y,y+h), right=max(x,x+w)...)
    case 'DOMRectReadOnly.top': return Math.min(st[1], st[1] + st[3]);
    case 'DOMRectReadOnly.right': return Math.max(st[0], st[0] + st[2]);
    case 'DOMRectReadOnly.bottom': return Math.max(st[1], st[1] + st[3]);
    case 'DOMRectReadOnly.left': return Math.min(st[0], st[0] + st[2]);
    case 'DOMQuad.p1': case 'DOMQuad.p2': case 'DOMQuad.p3': case 'DOMQuad.p4':
      return st[member[1] - 1];
  }
  P.illegal(get);
}

function set(iface, member, self, v) {
  CTX = "Failed to set the '" + member + "' property on '" + iface + "'";
  const st = bag(iface).get(self);
  if (st === undefined) P.illegal(set);
  if (iface === 'DOMMatrix' && ALIAS[member] !== undefined) {
    const idx = ALIAS[member];
    st.m[idx] = num(v);
    // writing a 3D-only slot always drops the 2D flag (truth dm.m13.set)
    if (!IS_2D_SLOT.includes(idx)) st.is2D = false;
    return;
  }
  switch (iface + '.' + member) {
    case 'DOMPoint.x': case 'DOMPoint.y': case 'DOMPoint.z': case 'DOMPoint.w':
      st['xyzw'.indexOf(member)] = num(v); return;
    case 'DOMRect.x': case 'DOMRect.y': case 'DOMRect.width': case 'DOMRect.height':
      st[['x', 'y', 'width', 'height'].indexOf(member)] = num(v); return;
  }
  P.illegal(set);
}

// Blink serializes these IDL dictionaries with codegen member order =
// lexicographic (truth dm.fromMatrix.empty / dp.toJSON: a..f, is2D,
// isIdentity, m11..m44 / w, x, y, z).
function matrixToJSON(st) {
  const o = {};
  o.a = st.m[0]; o.b = st.m[1]; o.c = st.m[4]; o.d = st.m[5]; o.e = st.m[12]; o.f = st.m[13];
  o.is2D = st.is2D;
  o.isIdentity = isIdentity16(st.m);
  const names = ['m11', 'm12', 'm13', 'm14', 'm21', 'm22', 'm23', 'm24', 'm31', 'm32', 'm33', 'm34', 'm41', 'm42', 'm43', 'm44'];
  for (let k = 0; k < 16; k++) o[names[k]] = st.m[k];
  return o;
}

// scale origin handling shared by scale/scale3d legs.
// uniform=true -> scale3d shape (s, ox, oy, oz); else (sx, sy, sz, ox, oy, oz).
function doScale(st, args, uniform) {
  let sx, sy, sz, ox, oy, oz;
  if (uniform) {
    sx = sy = sz = argNum(args, 0, 1);
    ox = argNum(args, 1, 0); oy = argNum(args, 2, 0); oz = argNum(args, 3, 0);
  } else {
    sx = argNum(args, 0, 1);
    sy = argNumNoDflt(args, 1, sx); // missing -> scaleX, undefined -> NaN
    sz = argNum(args, 2, 1);
    ox = argNum(args, 3, 0); oy = argNum(args, 4, 0); oz = argNum(args, 5, 0);
  }
  // multiply(scale): this·S scales COLUMNS (col0 *= sx, col1 *= sy, col2 *= sz)
  // — done in place like Blink's Scale, never through the full matmul, so a
  // NaN factor stays inside its own column (truth ro.scale.z: a=2, d=NaN).
  const moved = ox !== 0 || oy !== 0 || oz !== 0;
  if (moved) postTranslate(st, ox, oy, oz);
  for (let c = 0; c < 3; c++) {
    const s = c === 0 ? sx : c === 1 ? sy : sz;
    st.m[c * 4] *= s; st.m[c * 4 + 1] *= s; st.m[c * 4 + 2] *= s; st.m[c * 4 + 3] *= s;
  }
  if (moved) postTranslate(st, -ox, -oy, -oz);
  if (sz !== 1 || oz !== 0) st.is2D = false;
}

// rotate(): with rotY AND rotZ both MISSING the single argument is a
// Z-axis rotation that keeps is2D; with rotY or rotZ engaged (even as
// explicit undefined -> NaN) it is the three-axis path (truth
// dm.rotateSelf / ro.rotate.xy / ro.rotate.undef).
function rotateInto(st, args, mutate) {
  const rx = argNum(args, 0, 0);
  const single = args.length < 2;
  const ry = single ? 0 : num(args[1]);
  const rz = args.length < 2 || args.length < 3 ? 0 : num(args[2]);
  const parts = single ? [rotZM(rx)] : [rotXM(rx), rotYM(ry), rotZM(rz)];
  let m = st.m;
  for (const p of parts) m = mul(m, p);
  const is2D = single ? st.is2D : st.is2D && rx === 0 && ry === 0;
  if (mutate) { st.m = m; if (is2D !== st.is2D) st.is2D = is2D; }
  return { m, is2D };
}

function call(iface, member, self, ctx, args) {
  CTX = ctx;
  const st = bag(iface).get(self);
  if (st === undefined) P.illegal(call);
  switch (iface + '.' + member) {
    // ---------------- DOMPointReadOnly ----------------
    case 'DOMPointReadOnly.matrixTransform': {
      const m = readMatrixInit(asDict(args[0], 'DOMMatrixInit')).m;
      return mintPoint('DOMPoint', transform4(m, st));
    }
    case 'DOMPointReadOnly.toJSON':
      return { w: st[3], x: st[0], y: st[1], z: st[2] };
    // ---------------- DOMRectReadOnly -----------------
    case 'DOMRectReadOnly.toJSON':
      return { bottom: Math.max(st[1], st[1] + st[3]), height: st[3],
        left: Math.min(st[0], st[0] + st[2]), right: Math.max(st[0], st[0] + st[2]),
        top: Math.min(st[1], st[1] + st[3]), width: st[2], x: st[0], y: st[1] };
    // ---------------- DOMQuad -------------------------
    case 'DOMQuad.getBounds': {
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const p of st) {
        minX = Math.min(minX, p.x); maxX = Math.max(maxX, p.x);
        minY = Math.min(minY, p.y); maxY = Math.max(maxY, p.y);
      }
      return mintRect([minX, minY, maxX - minX, maxY - minY]);
    }
    case 'DOMQuad.toJSON': {
      const o = {};
      for (let k = 0; k < 4; k++) {
        const p = st[k];
        o['p' + (k + 1)] = { w: p.w, x: p.x, y: p.y, z: p.z };
      }
      return o;
    }
    // ---------------- DOMMatrixReadOnly ops -----------
    // (also serve DOMMatrix receivers through inheritance; every op mints a
    // mutable DOMMatrix per spec)
    case 'DOMMatrixReadOnly.translate': {
      const tx = argNum(args, 0, 0), ty = argNum(args, 1, 0), tz = argNum(args, 2, 0);
      const r = { m: new Float64Array(st.m), is2D: st.is2D };
      if (tx !== 0 || ty !== 0 || tz !== 0) postTranslate(r, tx, ty, tz);
      return mintMatrix('DOMMatrix', r.m, st.is2D && tz === 0);
    }
    case 'DOMMatrixReadOnly.scale': case 'DOMMatrixReadOnly.scaleNonUniform': {
      const r = { m: new Float64Array(st.m), is2D: st.is2D };
      doScale(r, member === 'scaleNonUniform'
        ? [argNum(args, 0, 1), argNum(args, 1, 1), 1, 0, 0, 0] : args, false);
      return mintMatrix('DOMMatrix', r.m, r.is2D);
    }
    case 'DOMMatrixReadOnly.scale3d': {
      const r = { m: new Float64Array(st.m), is2D: st.is2D };
      doScale(r, args, true);
      return mintMatrix('DOMMatrix', r.m, r.is2D);
    }
    case 'DOMMatrixReadOnly.rotate': {
      const r = rotateInto(st, args, false);
      return mintMatrix('DOMMatrix', r.m, r.is2D);
    }
    case 'DOMMatrixReadOnly.rotateFromVector': {
      const x = argNum(args, 0, 0), y = argNum(args, 1, 0);
      const h = Math.sqrt(x * x + y * y);
      if (h === 0) return mintMatrix('DOMMatrix', new Float64Array(st.m), st.is2D);
      const m = ID16();
      m[0] = x / h; m[1] = y / h; m[4] = -y / h; m[5] = x / h; // exact 0.6/0.8 for (3,4)
      return mintMatrix('DOMMatrix', mul(st.m, m), st.is2D);
    }
    case 'DOMMatrixReadOnly.rotateAxisAngle': {
      const x = argNum(args, 0, 0), y = argNum(args, 1, 0), z = argNum(args, 2, 0), ang = argNum(args, 3, 0);
      const m = axisAngleM(x, y, z, ang);
      return mintMatrix('DOMMatrix', mul(st.m, m), st.is2D && x === 0 && y === 0 && z === 1);
    }
    case 'DOMMatrixReadOnly.skewX': {
      const m = ID16();
      m[4] = Math.tan(argNum(args, 0, 0) * D2R);
      return mintMatrix('DOMMatrix', mul(st.m, m), st.is2D);
    }
    case 'DOMMatrixReadOnly.skewY': {
      const m = ID16();
      m[1] = Math.tan(argNum(args, 0, 0) * D2R);
      return mintMatrix('DOMMatrix', mul(st.m, m), st.is2D);
    }
    case 'DOMMatrixReadOnly.multiply': {
      const om = readMatrixInit(asDict(args[0], 'DOMMatrixInit'));
      return mintMatrix('DOMMatrix', mul(st.m, om.m), st.is2D && om.is2D);
    }
    case 'DOMMatrixReadOnly.flipX': {
      const m = ID16(); m[0] = -1;
      return mintMatrix('DOMMatrix', mul(st.m, m), st.is2D);
    }
    case 'DOMMatrixReadOnly.flipY': {
      const m = ID16(); m[5] = -1;
      return mintMatrix('DOMMatrix', mul(st.m, m), st.is2D);
    }
    case 'DOMMatrixReadOnly.inverse': {
      const inv = inverse16(st.m);
      if (inv === null) return mintMatrix('DOMMatrix', new Float64Array(16).fill(NaN), false);
      return mintMatrix('DOMMatrix', inv, st.is2D);
    }
    case 'DOMMatrixReadOnly.transformPoint': {
      const p = readPointInit(asDict(args[0], 'DOMPointInit'));
      return mintPoint('DOMPoint', transform4(st.m, p));
    }
    case 'DOMMatrixReadOnly.toFloat32Array':
      return new Float32Array(st.m);
    case 'DOMMatrixReadOnly.toFloat64Array':
      return new Float64Array(st.m);
    case 'DOMMatrixReadOnly.toJSON':
      return matrixToJSON(st);
    case 'DOMMatrixReadOnly.toString':
      if (st.is2D) {
        return 'matrix(' + [st.m[0], st.m[1], st.m[4], st.m[5], st.m[12], st.m[13]].join(', ') + ')';
      }
      return 'matrix3d(' + Array.from(st.m).join(', ') + ')';
    // ---------------- DOMMatrix mutators ---------------
    case 'DOMMatrix.multiplySelf': case 'DOMMatrix.preMultiplySelf': {
      const om = readMatrixInit(asDict(args[0], 'DOMMatrixInit'));
      st.m = member === 'multiplySelf' ? mul(st.m, om.m) : mul(om.m, st.m);
      st.is2D = st.is2D && om.is2D;
      return self;
    }
    case 'DOMMatrix.translateSelf': {
      const tx = argNum(args, 0, 0), ty = argNum(args, 1, 0), tz = argNum(args, 2, 0);
      if (tx !== 0 || ty !== 0 || tz !== 0) postTranslate(st, tx, ty, tz);
      if (tz !== 0) st.is2D = false;
      return self;
    }
    case 'DOMMatrix.scaleSelf': doScale(st, args, false); return self;
    case 'DOMMatrix.scale3dSelf': doScale(st, args, true); return self;
    case 'DOMMatrix.rotateSelf': {
      rotateInto(st, args, true);
      return self;
    }
    case 'DOMMatrix.rotateFromVectorSelf': {
      const x = argNum(args, 0, 0), y = argNum(args, 1, 0);
      const h = Math.sqrt(x * x + y * y);
      if (h !== 0) {
        const m = ID16();
        m[0] = x / h; m[1] = y / h; m[4] = -y / h; m[5] = x / h;
        post(st, m);
      }
      return self;
    }
    case 'DOMMatrix.rotateAxisAngleSelf': {
      const x = argNum(args, 0, 0), y = argNum(args, 1, 0), z = argNum(args, 2, 0), ang = argNum(args, 3, 0);
      post(st, axisAngleM(x, y, z, ang));
      if (!(x === 0 && y === 0 && z === 1)) st.is2D = false;
      return self;
    }
    case 'DOMMatrix.skewXSelf': {
      const m = ID16();
      m[4] = Math.tan(argNum(args, 0, 0) * D2R);
      post(st, m);
      return self;
    }
    case 'DOMMatrix.skewYSelf': {
      const m = ID16();
      m[1] = Math.tan(argNum(args, 0, 0) * D2R);
      post(st, m);
      return self;
    }
    case 'DOMMatrix.invertSelf': {
      const inv = inverse16(st.m);
      if (inv === null) { st.m = new Float64Array(16).fill(NaN); st.is2D = false; }
      else st.m = inv;
      return self;
    }
    case 'DOMMatrix.setMatrixValue': {
      BAG.matrix.set(self, parseTransformList(num2str(args[0])));
      return self;
    }
  }
  P.illegal(call);
}

function stat(iface, member, ctx, args) {
  CTX = ctx;
  switch (iface + '.' + member) {
    case 'DOMMatrixReadOnly.fromMatrix': case 'DOMMatrix.fromMatrix': {
      const mi = readMatrixInit(asDict(args[0], 'DOMMatrixInit'));
      return mintMatrix(iface, mi.m, mi.is2D);
    }
    case 'DOMMatrixReadOnly.fromFloat32Array': case 'DOMMatrix.fromFloat32Array':
    case 'DOMMatrixReadOnly.fromFloat64Array': case 'DOMMatrix.fromFloat64Array': {
      const want32 = member === 'fromFloat32Array';
      const Ctor = want32 ? Float32Array : Float64Array;
      const a = args[0];
      if (!(a instanceof Ctor)) {
        P.fail(stat, "The provided value is not of type '" + (want32 ? 'Float32Array' : 'Float64Array') + "'.", CTX);
      }
      // 6-element (2D) form: m33/m44 keep their identity 1 (truth dm.ff32.nan
      // — only the six 2D slots are overwritten, the rest stay from I).
      const m = ID16();
      if (a.length === 6) {
        m[0] = a[0]; m[1] = a[1]; m[4] = a[2]; m[5] = a[3]; m[12] = a[4]; m[13] = a[5];
        return mintMatrix(iface, m, true);
      }
      if (a.length === 16) {
        for (let k = 0; k < 16; k++) m[k] = a[k];
        return mintMatrix(iface, m, false);
      }
      P.fail(stat, 'The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix.', CTX);
      return null;
    }
    case 'DOMPointReadOnly.fromPoint': case 'DOMPoint.fromPoint':
      return mintPoint(iface, readPointInit(asDict(args[0], 'DOMPointInit')));
    case 'DOMRectReadOnly.fromRect': case 'DOMRect.fromRect':
      return mintRect(readRectInit(asDict(args[0], 'DOMRectInit')));
    case 'DOMQuad.fromRect': {
      const [x, y, w, h] = readRectInit(asDict(args[0], 'DOMRectInit'));
      const q = new globalThis.DOMQuad();
      BAG.quad.set(q, [
        mintPoint('DOMPoint', [x, y, 0, 1]), mintPoint('DOMPoint', [x + w, y, 0, 1]),
        mintPoint('DOMPoint', [x + w, y + h, 0, 1]), mintPoint('DOMPoint', [x, y + h, 0, 1])]);
      return q;
    }
    case 'DOMQuad.fromQuad': {
      const d = asDict(args[0], 'DOMQuadInit');
      const q = new globalThis.DOMQuad();
      const pts = [];
      for (const k of ['p1', 'p2', 'p3', 'p4']) {
        pts.push(mintPoint('DOMPoint', readPointInit(asDict(d[k], 'DOMPointInit'))));
      }
      BAG.quad.set(q, pts);
      return q;
    }
  }
  P.illegal(stat);
}

exports.ctor = ctor;
exports.get = get;
exports.set = set;
exports.call = call;
exports.stat = stat;

// ---- WebKitCSSMatrix: DOMMatrix's legacy factory function --------------
// Chrome 145 facts (truth wkcm.*): NOT a subclass — it SHARES
// DOMMatrix.prototype, its own [[Prototype]] is the PARENT interface object
// (DOMMatrixReadOnly), its name is "DOMMatrix", and it carries the three
// statics. Constructed from an optional transform-list string ('' default).
// The wiring needs globalThis.DOMMatrix, which the generated module exposes
// AFTER this module loads — so it happens in ready(), called at the gen
// module's tail.
function WebKitCSSMatrix() {
  if (!new.target) {
    P.fail(WebKitCSSMatrix, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'WebKitCSSMatrix'");
  }
  CTX = "Failed to construct 'WebKitCSSMatrix'";
  const s = arguments.length > 0 && arguments[0] !== undefined ? num2str(arguments[0]) : '';
  BAG.matrix.set(this, parseTransformList(s));
}
P.expose('WebKitCSSMatrix', WebKitCSSMatrix);

let wired = false;
function ready() {
  if (wired) return;
  wired = true;
  const DM = globalThis.DOMMatrix, DMR = globalThis.DOMMatrixReadOnly;
  if (!DM || !DMR) return;
  Object.defineProperty(WebKitCSSMatrix, 'name', { value: 'DOMMatrix', configurable: true });
  WebKitCSSMatrix.prototype = DM.prototype;
  Object.setPrototypeOf(WebKitCSSMatrix, DMR);
  const wk = (op) => "Failed to execute '" + op + "' on 'WebKitCSSMatrix'";
  P.stat(WebKitCSSMatrix, 'fromMatrix', 1, function fromMatrix(init) {
    return stat('DOMMatrix', 'fromMatrix', wk('fromMatrix'), arguments);
  });
  P.stat(WebKitCSSMatrix, 'fromFloat32Array', 1, function fromFloat32Array(arr) {
    return stat('DOMMatrix', 'fromFloat32Array', wk('fromFloat32Array'), arguments);
  });
  P.stat(WebKitCSSMatrix, 'fromFloat64Array', 1, function fromFloat64Array(arr) {
    return stat('DOMMatrix', 'fromFloat64Array', wk('fromFloat64Array'), arguments);
  });
  _markNative(WebKitCSSMatrix);
}
exports.ready = ready;
