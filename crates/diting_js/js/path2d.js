(function () { // @@SCOPE

if (typeof Path2D === 'undefined') {
  // (#164) A real Path2D: subpaths in user space, built through the
  // command API or the SVG path-data constructor (new Path2D('M0 0h8v8…')).
  // qrcode.react feature-detects addPath, then hands whole QR modules to
  // ctx.fill(new Path2D(d)) — the old all-no-op stub made the detect pass
  // and the fill paint nothing (blank QR canvas on taobao havana login).
  globalThis.Path2D = class Path2D {
    constructor(d) {
      this._subs = [];   // [{p: [[x,y],…], c: closed}]
      this._cur = null;
      this._cx = 0; this._cy = 0;    // current point
      this._sx = 0; this._sy = 0;    // subpath start (Z returns here)
      this._lc = null; this._lct = null; // last control point + its kind (S/T reflection)
      __hideOwn(this);
      if (typeof d === 'string' && d) this._svg(d);
    }
    moveTo(x, y) {
      this._cur = { p: [[x, y]], c: false };
      this._subs.push(this._cur);
      this._cx = this._sx = x; this._cy = this._sy = y;
      this._lc = null; this._lct = null;
    }
    lineTo(x, y) {
      if (!this._cur) this.moveTo(this._cx, this._cy);
      this._cur.p.push([x, y]);
      this._cx = x; this._cy = y;
      this._lc = null; this._lct = null;
    }
    closePath() {
      if (this._cur) { this._cur.c = true; this._cur = null; }
      this._cx = this._sx; this._cy = this._sy;
      this._lc = null; this._lct = null;
    }
    rect(x, y, w, h) {
      this.moveTo(x, y);
      this.lineTo(x + w, y); this.lineTo(x + w, y + h); this.lineTo(x, y + h);
      this.closePath();
    }
    bezierCurveTo(c1x, c1y, c2x, c2y, x, y) {
      if (!this._cur) this.moveTo(this._cx, this._cy);
      const x0 = this._cx, y0 = this._cy;
      for (let i = 1; i <= 16; i++) {
        const t = i / 16, u = 1 - t;
        this._cur.p.push([
          u*u*u*x0 + 3*u*u*t*c1x + 3*u*t*t*c2x + t*t*t*x,
          u*u*u*y0 + 3*u*u*t*c1y + 3*u*t*t*c2y + t*t*t*y,
        ]);
      }
      this._cx = x; this._cy = y;
      this._lc = [c2x, c2y]; this._lct = 'c';
    }
    quadraticCurveTo(cx, cy, x, y) {
      if (!this._cur) this.moveTo(this._cx, this._cy);
      const x0 = this._cx, y0 = this._cy;
      for (let i = 1; i <= 16; i++) {
        const t = i / 16, u = 1 - t;
        this._cur.p.push([
          u*u*x0 + 2*u*t*cx + t*t*x,
          u*u*y0 + 2*u*t*cy + t*t*y,
        ]);
      }
      this._cx = x; this._cy = y;
      this._lc = [cx, cy]; this._lct = 'q';
    }
    arc(x, y, r, start, end, ccw) { this._ellipse(x, y, r, r, 0, start, end, ccw); }
    ellipse(x, y, rx, ry, rot, start, end, ccw) { this._ellipse(x, y, rx, ry, rot, start, end, ccw); }
    _ellipse(x, y, rx, ry, rot, start, end, ccw) {
      rx = Math.abs(rx) || 0; ry = Math.abs(ry) || 0;
      start = start || 0; end = end || 0;
      let sweep = end - start;
      const TAU = Math.PI * 2;
      if (!ccw) { while (sweep < 0) sweep += TAU; if (sweep > TAU) sweep = TAU; if (sweep === 0 && (rx || ry)) sweep = TAU; }
      else { while (sweep > 0) sweep -= TAU; if (sweep < -TAU) sweep = -TAU; if (sweep === 0 && (rx || ry)) sweep = -TAU; }
      if (!rx || !ry) { this.lineTo(x, y); return; }
      const n = Math.max(8, Math.ceil(Math.abs(sweep) / (Math.PI / 32)));
      const cosR = Math.cos(rot || 0), sinR = Math.sin(rot || 0);
      const P = (a) => {
        const ex = rx * Math.cos(a), ey = ry * Math.sin(a);
        return [x + ex * cosR - ey * sinR, y + ex * sinR + ey * cosR];
      };
      if (!this._cur) { const p0 = P(start); this.moveTo(p0[0], p0[1]); }
      for (let i = 1; i <= n; i++) {
        const p = P(start + sweep * i / n);
        if (!this._cur) this.moveTo(p[0], p[1]); else this._cur.p.push(p);
        this._cx = p[0]; this._cy = p[1];
      }
      this._lc = null; this._lct = null;
    }
    // Standard tangent-arc: line from the current point to the tangent
    // start, then the arc to the tangent end (spec: no trailing line to
    // the (x2,y2) corner — the next path op draws that).
    arcTo(x1, y1, x2, y2, r) {
      if (!(r >= 0)) throw new RangeError('IndexSizeError');
      const x0 = this._cx, y0 = this._cy;
      if (r === 0 || (x0 === x1 && y0 === y1) || (x1 === x2 && y1 === y2) ||
          (x1 - x0) * (y2 - y1) - (x2 - x1) * (y1 - y0) === 0) {
        this.lineTo(x1, y1); return;
      }
      const d1 = Math.hypot(x1 - x0, y1 - y0), d2 = Math.hypot(x2 - x1, y2 - y1);
      if (!d1 || !d2) { this.lineTo(x1, y1); return; }
      const ux1 = (x1 - x0) / d1, uy1 = (y1 - y0) / d1;
      const ux2 = (x2 - x1) / d2, uy2 = (y2 - y1) / d2;
      const th = Math.acos(Math.max(-1, Math.min(1, ux1 * ux2 + uy1 * uy2)));
      const L = r / Math.tan(th / 2);
      const ax = x1 - ux1 * L, ay = y1 - uy1 * L;
      const bx = x1 + ux2 * L, by = y1 + uy2 * L;
      this.lineTo(ax, ay);
      const cross = ux1 * uy2 - uy1 * ux2;
      this._ellipse(x1, y1, r, r, 0, Math.atan2(ay - y1, ax - x1), Math.atan2(by - y1, bx - x1), cross < 0);
      this._cx = bx; this._cy = by;
    }
    addPath(path, m) {
      if (!path || !path._subs) return;
      const xf = m
        ? (Array.isArray(m) ? m : [m.a ?? 1, m.b ?? 0, m.c ?? 0, m.d ?? 1, m.e ?? 0, m.f ?? 0])
        : null;
      for (const s of path._subs) {
        const p = xf
          ? s.p.map((pt) => [xf[0]*pt[0] + xf[2]*pt[1] + xf[4], xf[1]*pt[0] + xf[3]*pt[1] + xf[5]])
          : s.p.map((pt) => [pt[0], pt[1]]);
        this._subs.push({ p, c: s.c });
      }
      this._cur = null;
    }
    // SVG path data (https://www.w3.org/TR/SVG11/paths.html#PathData) —
    // every command letter, implicit repeats (M→L after the first pair),
    // relative forms, H/V shorthands, and S/T control-point reflection.
    _svg(d) {
      const toks = d.match(/[MmZzLlHhVvCcSsQqTtAa]|[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?/g) || [];
      let i = 0, cmd = '', pairs = 0;
      const num = () => { const v = parseFloat(toks[i++]); return isFinite(v) ? v : 0; };
      while (i < toks.length) {
        if (/[MmZzLlHhVvCcSsQqTtAa]/.test(toks[i])) {
          cmd = toks[i++]; pairs = 0;
          if (cmd === 'Z' || cmd === 'z') { this.closePath(); continue; }
        }
        const rel = cmd === cmd.toLowerCase();
        const C = cmd.toUpperCase();
        if (C === 'M' || C === 'L') {
          const x = num() + (rel ? this._cx : 0), y = num() + (rel ? this._cy : 0);
          if (C === 'M' && pairs === 0) this.moveTo(x, y); else this.lineTo(x, y);
          pairs++;
        } else if (C === 'H') {
          this.lineTo(num() + (rel ? this._cx : 0), this._cy);
        } else if (C === 'V') {
          this.lineTo(this._cx, num() + (rel ? this._cy : 0));
        } else if (C === 'C') {
          const c1x = num() + (rel ? this._cx : 0), c1y = num() + (rel ? this._cy : 0);
          const c2x = num() + (rel ? this._cx : 0), c2y = num() + (rel ? this._cy : 0);
          const x = num() + (rel ? this._cx : 0), y = num() + (rel ? this._cy : 0);
          this.bezierCurveTo(c1x, c1y, c2x, c2y, x, y);
        } else if (C === 'S') {
          const lc = (this._lct === 'c' && this._lc) ? [2*this._cx - this._lc[0], 2*this._cy - this._lc[1]] : [this._cx, this._cy];
          const c2x = num() + (rel ? this._cx : 0), c2y = num() + (rel ? this._cy : 0);
          const x = num() + (rel ? this._cx : 0), y = num() + (rel ? this._cy : 0);
          this.bezierCurveTo(lc[0], lc[1], c2x, c2y, x, y);
        } else if (C === 'Q') {
          const cx = num() + (rel ? this._cx : 0), cy = num() + (rel ? this._cy : 0);
          const x = num() + (rel ? this._cx : 0), y = num() + (rel ? this._cy : 0);
          this.quadraticCurveTo(cx, cy, x, y);
        } else if (C === 'T') {
          const lc = (this._lct === 'q' && this._lc) ? [2*this._cx - this._lc[0], 2*this._cy - this._lc[1]] : [this._cx, this._cy];
          const x = num() + (rel ? this._cx : 0), y = num() + (rel ? this._cy : 0);
          this.quadraticCurveTo(lc[0], lc[1], x, y);
        } else if (C === 'A') {
          const rx = num(), ry = num(), rot = num(), laf = num(), sf = num();
          const x2 = num() + (rel ? this._cx : 0), y2 = num() + (rel ? this._cy : 0);
          this._svgArc(rx, ry, rot, laf, sf, x2, y2);
        } else {
          i++; // unknown letter: skip
        }
      }
    }
    // Endpoint→center arc parameterization (SVG spec appendix F.6.5).
    _svgArc(rx, ry, rotDeg, laf, sf, x2, y2) {
      const x1 = this._cx, y1 = this._cy;
      if (!rx || !ry || (x1 === x2 && y1 === y2)) { this.lineTo(x2, y2); return; }
      rx = Math.abs(rx); ry = Math.abs(ry);
      const phi = rotDeg * Math.PI / 180, cph = Math.cos(phi), sph = Math.sin(phi);
      const dx2 = (x1 - x2) / 2, dy2 = (y1 - y2) / 2;
      const x1p = cph * dx2 + sph * dy2, y1p = -sph * dx2 + cph * dy2;
      const lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
      if (lam > 1) { const s = Math.sqrt(lam); rx *= s; ry *= s; }
      const den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
      let co = den ? Math.sqrt(Math.max(0, (rx * rx * ry * ry - den) / den)) : 0;
      if (laf === sf) co = -co;
      const cxp = co * rx * y1p / ry, cyp = -co * ry * x1p / rx;
      const cx = cph * cxp - sph * cyp + (x1 + x2) / 2;
      const cy = sph * cxp + cph * cyp + (y1 + y2) / 2;
      const ang = (ux, uy, vx, vy) => {
        const dot = ux * vx + uy * vy;
        const len = Math.sqrt((ux*ux + uy*uy) * (vx*vx + vy*vy)) || 1;
        let a = Math.acos(Math.max(-1, Math.min(1, dot / len)));
        if (ux * vy - uy * vx < 0) a = -a;
        return a;
      };
      const a1 = ang(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
      let da = ang((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
      if (!sf && da > 0) da -= Math.PI * 2;
      if (sf && da < 0) da += Math.PI * 2;
      if (!this._cur) {
        const px = cx + rx * Math.cos(a1) * cph - ry * Math.sin(a1) * sph;
        const py = cy + rx * Math.cos(a1) * sph + ry * Math.sin(a1) * cph;
        this.moveTo(px, py);
      }
      this._ellipse(cx, cy, rx, ry, phi, a1, a1 + da, !sf);
    }
  };
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
