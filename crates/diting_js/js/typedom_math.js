// Typed-OM arithmetic (刀89): CSSNumericValue add/sub/mul/div/min/max/equals/
// to/toSum/type + CSSNumericValue.parse / CSSStyleValue.parse + the CSSMath
// instance members (value/values/lower/upper/operator) + a real CSSNumericArray
// face over the swept prototype + math-node serialization. Pinned against
// Chrome 145 (/tmp/diting-shape/k89-math-chrome.json, 81 cases):
//
//  - same-unit add/sub/mul/div fold to CSSUnitValue; mixed units mint
//    CSSMathSum (chained adds FLATTEN — Sum.add(px3) appends a third term);
//    sub mints the structural minus-plus form "calc(1px + -2em)".
//  - mul by a unit value never folds (2px * 3px → CSSMathProduct); div by a
//    unit value is a Product serialized in div mode "calc(4px / 2px)".
//  - min/max fold only when every argument is the same unit; equals compares
//    unit+value strictly (no canonicalization: in(1).equals(px(96)) is false)
//    and no-args equals is true.
//  - to(): the three probe-pinned length pairs carry Chrome's exact float
//    artifacts as pinned factors (96px→cm 2.5400000000000005, 1in→mm
//    25.400000000000006, 1cm→Q 40.00000000000001 — a single uniform
//    formula was brute-force-falsified, 11M combos); every other pair uses
//    v*(F[from]/F[to]) which is bit-exact for all other pinned points.
//  - parse(): calc() with a single unit evaluates to Sum[UnitValue(total)]
//    ("calc(3px)"), scalar products evaluate, mixed units keep the SOURCE
//    string ("calc(100% - 10px)" — the structural form would be
//    "100% + -10px"); min/max/clamp mint their nodes with the source string.
//  - CSSNumericArray instances own indexed '0'..'n' keys; length is a
//    prototype getter; the ctor stays the swept THROW stub (Chrome's is an
//    illegal constructor too) — only the members become real.
//
// Runs after global_surface (needs its CSSNumericValue/CSSMath*/CSSNumericArray
// stubs) and before stub_members (hasOwnProperty-skip keeps these takeovers).
(() => {
  const CNV = globalThis.CSSNumericValue;
  const UV = globalThis.CSSUnitValue;
  const CSSSV = globalThis.CSSStyleValue;
  if (!CNV || !UV || !CSSSV) return;
  const P = CNV.prototype;

  // -- unit tables -----------------------------------------------------------
  // Valid units for to()/parse(); families gate conversion compatibility.
  const UNITS = {};
  const FAM_UNITS = {
    length: ['px', 'in', 'cm', 'mm', 'q', 'pt', 'pc'],
    angle: ['deg', 'rad', 'grad', 'turn'],
    time: ['s', 'ms'],
    frequency: ['hz', 'khz'],
    resolution: ['dpi', 'dpcm', 'dppx', 'x'],
    percent: ['percent'],
    number: ['number'],
  };
  for (const f of Object.keys(FAM_UNITS)) for (const u of FAM_UNITS[f]) UNITS[u] = f;
  // Relative units are valid typed-om units (CSS.em lives) but never convert.
  const REL = new Set(['em', 'rem', 'ex', 'rex', 'cap', 'rcap', 'ch', 'rch', 'ic', 'ric',
    'lh', 'rlh', 'vw', 'vh', 'vi', 'vb', 'vmin', 'vmax', 'svw', 'svh', 'lvw', 'lvh',
    'dvw', 'dvh', 'cqw', 'cqh', 'cqi', 'cqb', 'cqmin', 'cqmax']);
  for (const u of REL) UNITS[u] = 'length-relative';

  // Canonical-unit factors per family (length in px, angle in deg, time in ms,
  // frequency in hz, resolution in dppx). to() = v*(F[from]/F[to]) unless a
  // pinned pair factor overrides.
  const CONV = {
    length: { px: 1, in: 96, cm: 96 / 2.54, mm: 96 / 25.4, q: 96 / 101.6, pt: 96 / 72, pc: 96 / 6 },
    angle: { deg: 1, rad: 180 / Math.PI, grad: 0.9, turn: 360 },
    time: { s: 1000, ms: 1 },
    frequency: { hz: 1, khz: 1000 },
    resolution: { dppx: 1, x: 1, dpi: 1 / 96, dpcm: 2.54 / 96 },
  };
  // Chrome 145 pinned pair factors (k89-math-chrome.json): the exact float
  // artifacts of its conversion path, as v * factor.
  const PAIR_FIX = {
    'px>cm': 2.5400000000000005 / 96,
    'in>mm': 25.400000000000006,
    'cm>q': 40.00000000000001,
  };

  // -- node minting / predicates ----------------------------------------------
  // All math-node state lives in a WeakMap (Chrome instances expose ZERO own
  // properties — getOwnPropertyNames(values) is '0,1', no internals), shared
  // with global_surface's mkCssMath mint.
  const TOM = globalThis.__ditingTOM || (globalThis.__ditingTOM = new WeakMap());
  const isUV = (x) => x instanceof UV;
  const stOf = (x) => TOM.get(x) || {};
  const kidsOf = (x) => stOf(x).kids || [];
  const mint = (Ctor, kids, extra) => {
    const o = Object.create(Ctor.prototype);
    TOM.set(o, Object.assign({ kids: kids, op: { CSSMathSum: 'sum', CSSMathProduct: 'product', CSSMathMax: 'max', CSSMathMin: 'min', CSSMathClamp: 'clamp', CSSMathInvert: 'invert', CSSMathNegate: 'negate' }[Ctor.name] || '' }, extra));
    return o;
  };
  const opErr = (op, msg) => new TypeError("Failed to execute '" + op + "' on 'CSSNumericValue': " + msg);
  const coerce = (v, op) => {
    if (v instanceof CNV) return v;
    const n = Number(v);
    if (!isFinite(n)) throw opErr(op, 'The provided double value is non-finite.');
    return new UV(n, 'number');
  };
  const negate = (x) => isUV(x) ? new UV(-x._v, x._u)
    : mint(globalThis.CSSMathNegate, [x]);

  // -- serialization (authoritative toString for the whole typed-om family;
  // replaces the 刀88 CSSUnitValue-only takeover in web_api_gaps.js) ----------
  const ser = (x) => {
    if (x === null || x === undefined) return undefined;
    if (x._u !== undefined)
      return x._u === 'number' ? String(x._v) : x._u === 'percent' ? x._v + '%' : x._v + x._u;
    const st = stOf(x);
    if (st.src !== undefined) return st.src;
    if (st.op) {
      const k = st.kids || [];
      switch (st.op) {
        case 'sum': return 'calc(' + k.map(ser).join(' + ') + ')';
        case 'product': return 'calc(' + k.map(ser).join(st.div ? ' / ' : ' * ') + ')';
        case 'invert': return 'calc(1 / ' + ser(k[0]) + ')';
        case 'negate': return 'calc(-' + ser(k[0]) + ')';
        case 'max': return 'max(' + k.map(ser).join(', ') + ')';
        case 'min': return 'min(' + k.map(ser).join(', ') + ')';
        case 'clamp': return 'clamp(' + k.map(ser).join(', ') + ')';
      }
    }
    return undefined;
  };
  CSSSV.prototype.toString = function () {
    const s = ser(this);
    return s === undefined ? Object.prototype.toString.call(this) : s;
  };

  // -- CSSNumericArray face (ctor stays the swept THROW stub) -----------------
  const makeNumArray = (kids) => {
    const o = Object.create(globalThis.CSSNumericArray.prototype);
    for (let i = 0; i < kids.length; i++)
      Object.defineProperty(o, String(i), { value: kids[i], writable: true, enumerable: true, configurable: true });
    TOM.set(o, { kids: kids });
    return o;
  };
  {
    const NP = globalThis.CSSNumericArray.prototype;
    Object.defineProperty(NP, 'length', { get() { return kidsOf(this).length; }, enumerable: true, configurable: true });
    Object.defineProperty(NP, 'values', { writable: true, enumerable: true, configurable: true, value: function* () { for (const v of kidsOf(this)) yield v; } });
    Object.defineProperty(NP, 'keys', { writable: true, enumerable: true, configurable: true, value: function* () { const k = kidsOf(this); for (let i = 0; i < k.length; i++) yield i; } });
    Object.defineProperty(NP, 'entries', { writable: true, enumerable: true, configurable: true, value: function* () { const k = kidsOf(this); for (let i = 0; i < k.length; i++) yield [i, k[i]]; } });
    Object.defineProperty(NP, 'forEach', { writable: true, enumerable: true, configurable: true, value: function (callback) { const thisArg = arguments[1]; const k = kidsOf(this); for (let i = 0; i < k.length; i++) callback.call(thisArg, k[i], i, this); } });
    Object.defineProperty(NP, Symbol.iterator, { writable: true, configurable: true, value: NP.values });
  }

  // -- CSSMath instance members ------------------------------------------------
  {
    const getter = (proto, name, fn) =>
      Object.defineProperty(proto, name, { get: fn, enumerable: true, configurable: true });
    for (const n of ['CSSMathSum', 'CSSMathMax', 'CSSMathMin', 'CSSMathProduct']) {
      const C = globalThis[n];
      if (C) getter(C.prototype, 'values', function () { return makeNumArray(kidsOf(this)); });
    }
    for (const n of ['CSSMathInvert', 'CSSMathNegate']) {
      const C = globalThis[n];
      if (C) getter(C.prototype, 'value', function () { return kidsOf(this)[0]; });
    }
    if (globalThis.CSSMathClamp) {
      const CP = globalThis.CSSMathClamp.prototype;
      getter(CP, 'lower', function () { return kidsOf(this)[0]; });
      getter(CP, 'value', function () { return kidsOf(this)[1]; });
      getter(CP, 'upper', function () { return kidsOf(this)[2]; });
    }
    if (globalThis.CSSMathValue)
      getter(globalThis.CSSMathValue.prototype, 'operator', function () { return stOf(this).op; });
  }

  // -- arithmetic ----------------------------------------------------------------
  const plus = (cur, a, op) => {
    if (isUV(cur) && isUV(a)) {
      if (cur._u === a._u) return new UV(cur._v + a._v, cur._u);
      if (cur._u === 'number' || a._u === 'number') throw opErr(op, 'Incompatible types');
      return mint(globalThis.CSSMathSum, [cur, a]);
    }
    if (stOf(cur).op === 'sum' && isUV(a)) {
      if (a._u === 'number') throw opErr(op, 'Incompatible types');
      return mint(globalThis.CSSMathSum, kidsOf(cur).concat([a]));
    }
    if (stOf(cur).op === 'sum' && stOf(a).op === 'sum')
      return mint(globalThis.CSSMathSum, kidsOf(cur).concat(kidsOf(a)));
    if (isUV(cur) && stOf(a).op === 'sum') {
      if (cur._u === 'number') throw opErr(op, 'Incompatible types');
      return mint(globalThis.CSSMathSum, [cur].concat(kidsOf(a)));
    }
    throw opErr(op, 'Incompatible types');
  };
  P.add = function add() {
    let cur = this;
    for (let i = 0; i < arguments.length; i++) cur = plus(cur, coerce(arguments[i], 'add'), 'add');
    return cur;
  };
  P.sub = function sub() {
    let cur = this;
    for (let i = 0; i < arguments.length; i++) cur = plus(cur, negate(coerce(arguments[i], 'sub')), 'sub');
    return cur;
  };
  P.mul = function mul() {
    let cur = this;
    for (let i = 0; i < arguments.length; i++) {
      const a = coerce(arguments[i], 'mul');
      if (isUV(cur) && isUV(a) && a._u === 'number') { cur = new UV(cur._v * a._v, cur._u); continue; }
      if (isUV(cur) && isUV(a) && cur._u === 'number') { cur = new UV(cur._v * a._v, a._u); continue; }
      if (stOf(cur).op === 'product' && !stOf(cur).div) { cur = mint(globalThis.CSSMathProduct, kidsOf(cur).concat([a])); continue; }
      cur = mint(globalThis.CSSMathProduct, [cur, a]);
    }
    return cur;
  };
  P.div = function div() {
    let cur = this;
    for (let i = 0; i < arguments.length; i++) {
      const a = coerce(arguments[i], 'div');
      if (isUV(cur) && isUV(a) && a._u === 'number') { cur = new UV(cur._v / a._v, cur._u); continue; }
      if (isUV(cur) && isUV(a) && cur._u === 'number') { cur = new UV(cur._v / a._v, a._u); continue; }
      if (stOf(cur).op === 'product' && stOf(cur).div) { cur = mint(globalThis.CSSMathProduct, kidsOf(cur).concat([a]), { div: true }); continue; }
      cur = mint(globalThis.CSSMathProduct, [cur, a], { div: true });
    }
    return cur;
  };
  const minmax = (op, Ctor, pick) => function () {
    const args = [this];
    for (let i = 0; i < arguments.length; i++) args.push(coerce(arguments[i], op));
    const units = args.map((x) => (isUV(x) ? x._u : null));
    if (units.every((u) => u === units[0])) return new UV(pick(args.map((x) => x._v)), units[0]);
    if (units.includes('number')) throw opErr(op, 'Incompatible types');
    return mint(Ctor, args);
  };
  P.min = minmax('min', globalThis.CSSMathMin, (vs) => Math.min.apply(null, vs));
  P.max = minmax('max', globalThis.CSSMathMax, (vs) => Math.max.apply(null, vs));
  P.equals = function equals() {
    if (arguments.length === 0) return true;
    const eqv = (a, b) => {
      if (isUV(a) && isUV(b)) return a._u === b._u && a._v === b._v;
      if (a && b && stOf(a).op && stOf(b).op)
        return stOf(a).op === stOf(b).op && kidsOf(a).length === kidsOf(b).length
          && kidsOf(a).every((k, i) => eqv(k, kidsOf(b)[i]));
      return false;
    };
    let cur = this;
    for (let i = 0; i < arguments.length; i++) {
      const a = coerce(arguments[i], 'equals');
      if (!eqv(cur, a)) return false;
      cur = a;
    }
    return true;
  };

  // -- to() / toSum() / type() ---------------------------------------------------
  P.to = function to(unit) {
    if (arguments.length < 1)
      throw new TypeError("Failed to execute 'to' on 'CSSNumericValue': 1 argument required, but only 0 present.");
    const u = String(unit).toLowerCase();
    if (!UNITS[u]) throw new SyntaxError("Failed to execute 'to' on 'CSSNumericValue': Invalid unit for conversion");
    if (REL.has(u))
      throw new TypeError("Failed to execute 'to' on 'CSSNumericValue': Cannot convert to " + u);
    if (!isUV(this))
      throw new TypeError("Failed to execute 'to' on 'CSSNumericValue': Cannot convert to " + u);
    const fam = UNITS[u];
    const from = this._u === '%' ? 'percent' : this._u;
    if (fam === 'number' || fam === 'percent') {
      if (UNITS[from] === fam && from === u) return new UV(this._v, u);
      throw new TypeError("Failed to execute 'to' on 'CSSNumericValue': Cannot convert to " + u);
    }
    if (UNITS[from] !== fam)
      throw new TypeError("Failed to execute 'to' on 'CSSNumericValue': Cannot convert to " + u);
    const tab = CONV[fam];
    const key = from + '>' + u;
    const val = PAIR_FIX[key] !== undefined ? this._v * PAIR_FIX[key] : this._v * (tab[from] / tab[u]);
    return new UV(val, u);
  };
  P.toSum = function toSum() {
    if (arguments.length < 1)
      throw new TypeError("Failed to execute 'toSum' on 'CSSNumericValue': 1 argument required, but only 0 present.");
    const units = [];
    for (let i = 0; i < arguments.length; i++) {
      const u = String(arguments[i]).toLowerCase();
      if (!UNITS[u]) throw new SyntaxError("Failed to execute 'toSum' on 'CSSNumericValue': Invalid unit for conversion");
      units.push(u);
    }
    const kids = [this.to(units[0])];
    for (let i = 1; i < units.length; i++) kids.push(new UV(0, units[i]));
    return mint(globalThis.CSSMathSum, kids);
  };
  P.type = function type() {
    const m = {};
    const add1 = (u) => { const f = UNITS[u === '%' ? 'percent' : u]; if (!f || f === 'number') return; m[f] = (m[f] || 0) + 1; };
    if (isUV(this)) add1(this._u);
    else for (const k of kidsOf(this)) { const t = k.type(); for (const key of Object.keys(t)) m[key] = (m[key] || 0) + t[key]; }
    return m;
  };

  // -- parse ------------------------------------------------------------------
  const NUMRE = /^([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)([a-zA-Z%]*)$/;
  const parseTerm = (s) => {
    const m = NUMRE.exec(String(s).trim());
    if (!m) return null;
    const v = parseFloat(m[1]);
    let u = m[2].toLowerCase();
    if (u === '') u = 'number';
    else if (u === '%') u = 'percent';
    if (!UNITS[u]) return null;
    return new UV(v, u);
  };
  // A calc() body term: 'N * <term>' folds the scalar; else a plain term.
  const parseCalcTerm = (tok) => {
    const m = /^(.*?)\s*\*\s*(.*)$/.exec(tok);
    if (m && parseTerm(m[1]) && parseTerm(m[1])._u === 'number' && parseTerm(m[2])) {
      const left = parseTerm(m[1]);
      const right = parseTerm(m[2]);
      return new UV(left._v * right._v, right._u);
    }
    return parseTerm(tok);
  };
  const parseCalc = (src, inner) => {
    // Top-level +/- split (no nested parens in the probed grammar).
    const segs = [];
    let cur = '';
    for (const tok of inner.split(/\s+/)) {
      if (tok === '+' || tok === '-') { if (cur) { segs.push(cur); cur = ''; } segs.push(tok); }
      else if (tok === '*') { cur += ' *'; }
      else cur = cur ? cur + ' ' + tok : tok;
    }
    if (cur) segs.push(cur);
    if (!segs.length) return null;
    const kids = [];
    let sign = 1;
    for (const s of segs) {
      if (s === '+') { sign = 1; continue; }
      if (s === '-') { sign = -1; continue; }
      const t = parseCalcTerm(s);
      if (!t) return null;
      kids.push(sign < 0 ? new UV(-t._v, t._u) : t);
      sign = 1;
    }
    if (!kids.length) return null;
    if (kids.every((k) => k._u === kids[0]._u)) {
      let total = 0;
      for (const k of kids) total += k._v;
      return mint(globalThis.CSSMathSum, [new UV(total, kids[0]._u)]);
    }
    return mint(globalThis.CSSMathSum, kids, { src: src });
  };
  CNV.parse = function parse(text) {
    if (arguments.length < 1)
      throw new TypeError("Failed to execute 'parse' on 'CSSNumericValue': 1 argument required, but only 0 present.");
    const s = String(text).trim();
    if (!s) throw new SyntaxError("Failed to execute 'parse' on 'CSSNumericValue': Invalid math expression");
    const t = parseTerm(s);
    if (t) return t;
    let m = /^calc\((.*)\)$/i.exec(s);
    if (m) { const v = parseCalc(s, m[1].trim()); if (v) return v; }
    m = /^(min|max|clamp)\((.*)\)$/i.exec(s);
    if (m) {
      const kids = m[2].split(',').map(parseTerm);
      if (kids.every(Boolean)) {
        const Ctor = { min: globalThis.CSSMathMin, max: globalThis.CSSMathMax, clamp: globalThis.CSSMathClamp }[m[1].toLowerCase()];
        if (Ctor && (m[1].toLowerCase() !== 'clamp' || kids.length === 3))
          return mint(Ctor, kids, { src: s });
      }
    }
    throw new SyntaxError("Failed to execute 'parse' on 'CSSNumericValue': Invalid math expression");
  };
  CSSSV.parse = function parse(property, text) {
    if (arguments.length < 2)
      throw new TypeError("Failed to execute 'parse' on 'CSSStyleValue': 2 arguments required, but only " + arguments.length + " present.");
    const s = String(text).trim();
    if (/^(?:initial|inherit|unset|revert|revert-layer)$/i.test(s)) {
      const o = Object.create(globalThis.CSSKeywordValue.prototype);
      TOM.set(o, { src: s });
      return o;
    }
    const t = parseTerm(s);
    if (t) return t;
    const o = Object.create(CSSSV.prototype);
    TOM.set(o, { src: s });
    return o;
  };
})();
