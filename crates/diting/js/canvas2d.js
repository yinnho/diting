(function () { // @@SCOPE
// Canvas 2D rendering context family, split out of bootstrap.js (god-file
// ratchet): TextMetrics (刀74/刀97), the _Canvas2D class (the class IS the
// CanvasRenderingContext2D interface), the 刀117 font accessor (its parser
// lives in canvas_font.js), the getContext/toDataURL/toBlob implementations
// (bootstrap keeps the load-time installs as thin __diting_canvas* call-ins,
// same shape as the ce_attr split) and the #210 bitmap flush hook the Rust
// host evaluates before a render. Everything reached across files —
// canvas_font.js slots, webgl.js _gl* state — is call-time only.
// (M4 刀74) TextMetrics: instances carry NO own state (WeakMap slots), ten
// read-only prototype accessors with pinned names, `new TextMetrics()` is an
// Illegal constructor, measureText mints through the hidden factory. Chrome
// 145 (k74): width + actualBoundingBox{Left,Right,Ascent,Descent} +
// fontBoundingBox{Ascent,Descent} + {hanging,alphabetic,ideographic}Baseline.
// 刀97: `width` and fontBoundingBox come from the captured per-glyph tables
// (font_metrics.js, Chrome 145 macOS truth); the ink bounds and baselines
// stay the 刀74 heuristic MODEL (no rasterizer): shifts follow the relations
// measured on Chrome (alphabetic 0, hanging 0.8*ascent, ideographic -descent,
// top/middle/bottom by the 16px Arial em-box ratios), ink left/right = width
// minus a side-bearing fraction, aligned by textAlign.
(() => {
  const st = new WeakMap();
  const TM = class TextMetrics {
    constructor() { throw new TypeError("Failed to construct 'TextMetrics': Illegal constructor"); }
  };
  const keys = ['width', 'actualBoundingBoxLeft', 'actualBoundingBoxRight', 'fontBoundingBoxAscent', 'fontBoundingBoxDescent',
    'actualBoundingBoxAscent', 'actualBoundingBoxDescent', 'hangingBaseline', 'alphabeticBaseline', 'ideographicBaseline'];
  for (const k of keys) {
    const get = function () {
      const v = (this !== null && typeof this === 'object') ? st.get(this) : undefined;
      if (!v) throw new TypeError('Illegal invocation');
      return v[k];
    };
    Object.defineProperty(get, 'name', { value: 'get ' + k });
    Object.defineProperty(TM.prototype, k, { get, enumerable: true, configurable: true });
  }
  Object.defineProperty(TM.prototype, Symbol.toStringTag, { value: 'TextMetrics', configurable: true });
  globalThis.TextMetrics = TM;
  const r4 = (n) => Math.round(n * 1e4) / 1e4;
  Object.defineProperty(globalThis, '__diting_newTextMetrics', {
    // m = { width, size (px), family ('arial'|'times'|'mono'), text, baseline, align,
    //       vmet: [A, D] from the captured table (刀97) — present whenever the
    //       font resolved to a real table; absent keeps the 刀74 ratio classes }
    value(m) {
      const S = m.size, W = m.width;
      const ratio = m.family === 'mono' ? [0.928, 0.236] : m.family === 'times' ? [0.891, 0.216] : [0.905, 0.212];
      const A = m.vmet ? m.vmet[0] : Math.round(S * ratio[0]);
      const D = m.vmet ? m.vmet[1] : Math.round(S * ratio[1]);
      // shift = distance the chosen textBaseline sits ABOVE the alphabetic baseline
      const hang = 0.8 * A;
      const shift = { top: 0.77539 * S, hanging: hang, middle: 0.77539 * S / 2 - (D + 0.03711 * S) / 2, alphabetic: 0, ideographic: -D, bottom: -(D + 0.03711 * S) }[m.baseline] || 0;
      const ink = S > 0 && /\S/.test(m.text);
      const base = m.align === 'center' ? W / 2 : (m.align === 'right' || m.align === 'end') ? W : 0;
      const lsb = 0.08008 * S, rsb = 0.03711 * S;
      const desc = /[gjpqy,;()\[\]{}|]/.test(m.text) ? 0.21045 * S : 0.01172 * S;
      const o = Object.create(TM.prototype);
      st.set(o, {
        width: W,
        actualBoundingBoxLeft: ink ? r4(base - lsb) : 0,
        actualBoundingBoxRight: ink ? r4(W - base - rsb) : 0,
        fontBoundingBoxAscent: r4(A - shift) + 0,
        fontBoundingBoxDescent: r4(D + shift) + 0,
        actualBoundingBoxAscent: ink ? r4(0.71582 * S - shift) : 0,
        actualBoundingBoxDescent: ink ? r4(desc + shift) : 0,
        hangingBaseline: S > 0 ? r4(hang - shift) : 0,
        alphabeticBaseline: r4(-shift) + 0,
        ideographicBaseline: S > 0 ? r4(-D - shift) : 0,
      });
      return o;
    },
    enumerable: false, configurable: true, writable: true,
  });
})();

class _Canvas2D {
  constructor(canvas) {
    if (canvas === undefined || canvas === null || typeof canvas !== 'object') throw new TypeError("Failed to construct 'CanvasRenderingContext2D': Illegal constructor");
    __diting_liftSlot(this).canvas = canvas;
    this._w = canvas.width || 300;
    this._h = canvas.height || 150;
    // (#117) Pristine canvas = fully transparent black, like Chrome — the
    // old noise-textured init made every "blank canvas" hash unique, which
    // no real browser does. What gets hashed now is only what the page
    // actually drew (fillRect/fillText), deterministic per persona.
    this._buf = new Uint8ClampedArray(this._w * this._h * 4);
    this.fillStyle = '#000000';
    this.strokeStyle = '#000000';
    this.lineWidth = 1;
    this.font = '10px sans-serif';
    this.textAlign = 'start';
    this.textBaseline = 'alphabetic';
    this.globalAlpha = 1;
    this.globalCompositeOperation = 'source-over';
    this._stateStack = [];
    // (#164) Real 2D transform + path state. The old stub no-opped every
    // transform and every fill()/stroke() — qrcode.react feature-detects
    // Path2D.addPath, then draws whole QR modules as path data through
    // ctx.fill(new Path2D(d)) under ctx.scale(), which painted nothing.
    this._m = [1, 0, 0, 1, 0, 0];
    this._path2d = null;
    __hideOwn(this);
  }
  _resize(w, h) {
    // (#164) canvas.width = n resizes the live buffer. Chrome also resets
    // ALL drawing state on a dimension change; qrcode-react re-establishes
    // scale/fillStyle after sizing, so the reset is safe and correct.
    this._w = w;
    this._h = h;
    this._buf = new Uint8ClampedArray(w * h * 4);
    this.fillStyle = '#000000';
    this.strokeStyle = '#000000';
    this.lineWidth = 1;
    this.font = '10px sans-serif';
    this.textAlign = 'start';
    this.textBaseline = 'alphabetic';
    this.globalAlpha = 1;
    this._stateStack = [];
    this._m = [1, 0, 0, 1, 0, 0];
    this._path2d = null;
  }
  _parseColor(css) {
    if (typeof css !== 'string') css = String(css ?? '');
    css = css.trim().toLowerCase();
    if (!css || css === 'none') return [0,0,0,0];
    if (css.startsWith('#')) {
      const hex = css.slice(1);
      if (hex.length === 3) return [parseInt(hex[0]+hex[0],16),parseInt(hex[1]+hex[1],16),parseInt(hex[2]+hex[2],16),255];
      if (hex.length === 6) return [parseInt(hex.slice(0,2),16),parseInt(hex.slice(2,4),16),parseInt(hex.slice(4,6),16),255];
      if (hex.length === 8) return [parseInt(hex.slice(0,2),16),parseInt(hex.slice(2,4),16),parseInt(hex.slice(4,6),16),parseInt(hex.slice(6,8),16)];
    }
    // (#157) The CSS Color function forms, legacy comma AND modern space/
    // slash syntax, with percent channels and percent alpha. Charting libs
    // are hsla-heavy; before this every one of these fell to opaque black.
    const fn = css.match(/^(rgba?|hsla?)\(([^)]*)\)$/);
    if (fn) {
      const parts = fn[2].split('/'); // modern alpha separator; channel bodies never contain '/'
      let alpha = 1;
      if (parts.length === 2) {
        const a = parts[1].trim();
        if (a === 'none') alpha = 0;
        else if (a.endsWith('%')) alpha = parseFloat(a) / 100;
        else alpha = parseFloat(a);
      }
      const chans = parts[0].split(parts[0].indexOf(',') >= 0 ? ',' : /\s+/)
        .map(s => s.trim()).filter(s => s.length);
      if (chans.length >= 3 && isFinite(alpha)) {
        const num = (raw, scale255) => {
          // 'none' reads as 0 (CSS Color 4); percents scale per family.
          if (raw === 'none') return 0;
          const pct = raw.endsWith('%');
          let v = parseFloat(raw);
          if (!isFinite(v)) return NaN;
          if (pct) v = scale255 ? v * 255 / 100 : v / 100;
          return v;
        };
        let [r, g, b] = [num(chans[0], true), num(chans[1], true), num(chans[2], true)];
        if (fn[1][0] === 'h') {
          // hsl(): hue in degrees (wraps), s/l as 0..1 fractions.
          let h = chans[0] === 'none' ? 0 : parseFloat(chans[0]);
          const s = num(chans[1], false), l = num(chans[2], false);
          if (!isFinite(h) || !isFinite(s) || !isFinite(l)) return this._parseNamed(css);
          h = ((h % 360) + 360) % 360;
          const c = (1 - Math.abs(2 * l - 1)) * s;
          const x = c * (1 - Math.abs((h / 60) % 2 - 1));
          const m = l - c / 2;
          const sextant = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x]
            : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
          [r, g, b] = [(sextant[0] + m) * 255, (sextant[1] + m) * 255, (sextant[2] + m) * 255];
        }
        if ([r, g, b].every(isFinite)) {
          const a8 = chans.length >= 4 && parts.length === 1
            ? num(chans[3], false) * 255 // comma syntax carries alpha inline
            : Math.round(alpha * 255);
          return [
            Math.max(0, Math.min(255, Math.round(r))),
            Math.max(0, Math.min(255, Math.round(g))),
            Math.max(0, Math.min(255, Math.round(b))),
            Math.max(0, Math.min(255, Math.round(a8))),
          ];
        }
      }
    }
    return this._parseNamed(css);
  }
  _parseNamed(css) {
    const named = {aliceblue:[240,248,255,255],antiquewhite:[250,235,215,255],aqua:[0,255,255,255],aquamarine:[127,255,212,255],azure:[240,255,255,255],beige:[245,245,220,255],bisque:[255,228,196,255],black:[0,0,0,255],blanchedalmond:[255,235,205,255],blue:[0,0,255,255],blueviolet:[138,43,226,255],brown:[165,42,42,255],burlywood:[222,184,135,255],cadetblue:[95,158,160,255],chartreuse:[127,255,0,255],chocolate:[210,105,30,255],coral:[255,127,80,255],cornflowerblue:[100,149,237,255],cornsilk:[255,248,220,255],crimson:[220,20,60,255],cyan:[0,255,255,255],darkblue:[0,0,139,255],darkcyan:[0,139,139,255],darkgoldenrod:[184,134,11,255],darkgray:[169,169,169,255],darkgreen:[0,100,0,255],darkgrey:[169,169,169,255],darkkhaki:[189,183,107,255],darkmagenta:[139,0,139,255],darkolivegreen:[85,107,47,255],darkorange:[255,140,0,255],darkorchid:[153,50,204,255],darkred:[139,0,0,255],darksalmon:[233,150,122,255],darkseagreen:[143,188,143,255],darkslateblue:[72,61,139,255],darkslategray:[47,79,79,255],darkslategrey:[47,79,79,255],darkturquoise:[0,206,209,255],darkviolet:[148,0,211,255],deeppink:[255,20,147,255],deepskyblue:[0,191,255,255],dimgray:[105,105,105,255],dimgrey:[105,105,105,255],dodgerblue:[30,144,255,255],firebrick:[178,34,34,255],floralwhite:[255,250,240,255],forestgreen:[34,139,34,255],fuchsia:[255,0,255,255],gainsboro:[220,220,220,255],ghostwhite:[248,248,255,255],gold:[255,215,0,255],goldenrod:[218,165,32,255],gray:[128,128,128,255],green:[0,128,0,255],greenyellow:[173,255,47,255],grey:[128,128,128,255],honeydew:[240,255,240,255],hotpink:[255,105,180,255],indianred:[205,92,92,255],indigo:[75,0,130,255],ivory:[255,255,240,255],khaki:[240,230,140,255],lavender:[230,230,250,255],lavenderblush:[255,240,245,255],lawngreen:[124,252,0,255],lemonchiffon:[255,250,205,255],lightblue:[173,216,230,255],lightcoral:[240,128,128,255],lightcyan:[224,255,255,255],lightgoldenrodyellow:[250,250,210,255],lightgray:[211,211,211,255],lightgreen:[144,238,144,255],lightgrey:[211,211,211,255],lightpink:[255,182,193,255],lightsalmon:[255,160,122,255],lightseagreen:[32,178,170,255],lightskyblue:[135,206,250,255],lightslategray:[119,136,153,255],lightslategrey:[119,136,153,255],lightsteelblue:[176,196,222,255],lightyellow:[255,255,224,255],lime:[0,255,0,255],limegreen:[50,205,50,255],linen:[250,240,230,255],magenta:[255,0,255,255],maroon:[128,0,0,255],mediumaquamarine:[102,205,170,255],mediumblue:[0,0,205,255],mediumorchid:[186,85,211,255],mediumpurple:[147,112,219,255],mediumseagreen:[60,179,113,255],mediumslateblue:[123,104,238,255],mediumspringgreen:[0,250,154,255],mediumturquoise:[72,209,204,255],mediumvioletred:[199,21,133,255],midnightblue:[25,25,112,255],mintcream:[245,255,250,255],mistyrose:[255,228,225,255],moccasin:[255,228,181,255],navajowhite:[255,222,173,255],navy:[0,0,128,255],oldlace:[253,245,230,255],olive:[128,128,0,255],olivedrab:[107,142,35,255],orange:[255,165,0,255],orangered:[255,69,0,255],orchid:[218,112,214,255],palegoldenrod:[238,232,170,255],palegreen:[152,251,152,255],paleturquoise:[175,238,238,255],palevioletred:[219,112,147,255],papayawhip:[255,239,213,255],peachpuff:[255,218,185,255],peru:[205,133,63,255],pink:[255,192,203,255],plum:[221,160,221,255],powderblue:[176,224,230,255],purple:[128,0,128,255],rebeccapurple:[102,51,153,255],red:[255,0,0,255],rosybrown:[188,143,143,255],royalblue:[65,105,225,255],saddlebrown:[139,69,19,255],salmon:[250,128,114,255],sandybrown:[244,164,96,255],seagreen:[46,139,87,255],seashell:[255,245,238,255],sienna:[160,82,45,255],silver:[192,192,192,255],skyblue:[135,206,235,255],slateblue:[106,90,205,255],slategray:[112,128,144,255],slategrey:[112,128,144,255],snow:[255,250,250,255],springgreen:[0,255,127,255],steelblue:[70,130,180,255],tan:[210,180,140,255],teal:[0,128,128,255],thistle:[216,191,216,255],tomato:[255,99,71,255],turquoise:[64,224,208,255],violet:[238,130,238,255],wheat:[245,222,179,255],white:[255,255,255,255],whitesmoke:[245,245,245,255],yellow:[255,255,0,255],yellowgreen:[154,205,50,255],transparent:[0,0,0,0]};
    return named[css] || [0,0,0,255];
  }
  _setPixel(x, y, r, g, b, a) {
    x = Math.round(x); y = Math.round(y);
    if (x < 0 || x >= this._w || y < 0 || y >= this._h) return;
    const idx = (y * this._w + x) * 4;
    const alpha = (a / 255) * this.globalAlpha;
    this._buf[idx+0] = Math.round(r * alpha + this._buf[idx+0] * (1 - alpha));
    this._buf[idx+1] = Math.round(g * alpha + this._buf[idx+1] * (1 - alpha));
    this._buf[idx+2] = Math.round(b * alpha + this._buf[idx+2] * (1 - alpha));
    this._buf[idx+3] = Math.min(255, Math.round(a * alpha + this._buf[idx+3] * (1 - alpha)));
  }
  fillRect(x, y, w, h) {
    const rgba = this._parseColor(this.fillStyle);
    const m = this._m;
    // (#164) Axis-aligned CTM (identity/translate/scale): transform the
    // rect and run the tight integer loop. Rotated CTM falls to the
    // polygon scanline raster. Identity reproduces the old pixel-exact
    // behavior (pinned by canvas_fill_style_parses_css_color_grammar).
    if (m[1] === 0 && m[2] === 0) {
      const x0 = m[0]*x + m[4], x1 = m[0]*(x+w) + m[4];
      const y0 = m[3]*y + m[5], y1 = m[3]*(y+h) + m[5];
      this._fillRectRaw(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1-x0), Math.abs(y1-y0), rgba);
    } else {
      this._fillDevicePolys([[this._tp(x, y), this._tp(x+w, y), this._tp(x+w, y+h), this._tp(x, y+h)]], rgba);
    }
  }
  _fillRectRaw(x, y, w, h, rgba) {
    x=Math.round(x); y=Math.round(y); w=Math.round(w); h=Math.round(h);
    for (let py = Math.max(0,y); py < Math.min(this._h, y+h); py++) {
      for (let px = Math.max(0,x); px < Math.min(this._w, x+w); px++) {
        this._setPixel(px, py, rgba[0], rgba[1], rgba[2], rgba[3]);
      }
    }
  }
  clearRect(x, y, w, h) {
    const m = this._m;
    if (m[1] === 0 && m[2] === 0) {
      const x0 = m[0]*x + m[4], x1 = m[0]*(x+w) + m[4];
      const y0 = m[3]*y + m[5], y1 = m[3]*(y+h) + m[5];
      x = Math.round(Math.min(x0, x1)); y = Math.round(Math.min(y0, y1));
      w = Math.round(Math.abs(x1-x0)); h = Math.round(Math.abs(y1-y0));
      for (let py = Math.max(0,y); py < Math.min(this._h, y+h); py++) {
        for (let px = Math.max(0,x); px < Math.min(this._w, x+w); px++) {
          const idx = (py * this._w + px) * 4;
          this._buf[idx] = this._buf[idx+1] = this._buf[idx+2] = this._buf[idx+3] = 0;
        }
      }
    }
    // Rotated clearRect: no-op (filling a rotated polygon with zeros needs
    // a clear-mode scanline; nothing in the wild clears under rotation).
  }
  strokeRect(x, y, w, h) {
    // (#164) Stamp the four transformed edges — correct under any CTM;
    // under identity it lands within a pixel of the old edge-loop.
    const pts = [this._tp(x, y), this._tp(x+w, y), this._tp(x+w, y+h), this._tp(x, y+h)];
    const rgba = this._parseColor(this.strokeStyle);
    const rad = Math.max(1, Math.round(Math.max(1, this.lineWidth) / 2));
    for (let i = 0; i < 4; i++) this._stampSeg(pts[i], pts[(i+1) % 4], rad, rgba);
  }
  // (#117) Anchor math for blitting a rasterized text tile (width/height/
  // baseline, RGBA8). alphabetic is the canvas default; the other baselines
  // map off the tile's baseline. Align shifts x only.
  _blitText(tile, x, y) {
    let top;
    switch (this.textBaseline) {
      case 'top': case 'hanging': top = y; break;
      case 'middle': top = y - tile.height / 2; break;
      case 'bottom': case 'ideographic': top = y - tile.height; break;
      default: top = y - tile.baseline;
    }
    let left = x;
    if (this.textAlign === 'center') left = x - tile.width / 2;
    else if (this.textAlign === 'right' || this.textAlign === 'end') left = x - tile.width;
    const x0 = Math.round(left), y0 = Math.round(top);
    const d = tile.data;
    for (let ty = 0; ty < tile.height; ty++) {
      const py = y0 + ty;
      if (py < 0 || py >= this._h) continue;
      for (let tx = 0; tx < tile.width; tx++) {
        const px = x0 + tx;
        if (px < 0 || px >= this._w) continue;
        const i = (ty * tile.width + tx) * 4;
        if (d[i + 3] === 0) continue;
        this._setPixel(px, py, d[i], d[i + 1], d[i + 2], d[i + 3]);
      }
    }
  }
  fillText(text, x, y) {
    const str = String(text);
    const [r, g, b, a] = this._parseColor(this.fillStyle);
    const fm = String(this.font || '').match(/(\d+(?:\.\d+)?)px/);
    const fontSize = fm ? Math.round(parseFloat(fm[1])) : 10;
    // (#117) Real glyph raster (swash, same font book as the page painter)
    // via the screenshot-gated op; the noise-glyph fallback stays for
    // builds without the font stack. Deterministic per persona — Chrome's
    // canvas hash is stable on a machine too.
    if (_OPS.op_canvas_text) {
      try {
        const tile = _OPS.op_canvas_text(
          str, fontSize,
          /bold|[6-9]00/.test(String(this.font || '')),
          /mono|courier|consol/i.test(String(this.font || '')),
          new Uint8Array([r, g, b, a]),
        );
        if (tile && tile.width) this._blitText(tile, x, y);
        return;
      } catch (e) { /* fall through to the glyph fallback */ }
    }
    const scale = Math.max(1, Math.round(fontSize / 10));
    let cx = Math.round(x);
    for (let i = 0; i < str.length; i++) {
      const code = str.charCodeAt(i);
      for (let row = 0; row < 7; row++) {
        for (let col = 0; col < 5; col++) {
          const on = ((_fpRand(code * 100 + row * 10 + col) > 0.45) &&
                      (row > 0 && row < 6 && col > 0 && col < 4)) ||
                     (_fpRand(code * 200 + row * 7 + col) > 0.7);
          if (on) {
            for (let sy = 0; sy < scale; sy++) {
              for (let sx = 0; sx < scale; sx++) {
                this._setPixel(cx + col*scale + sx, Math.round(y) - 7*scale + row*scale + sy, r, g, b, a);
              }
            }
          }
        }
      }
      cx += 6 * scale;
    }
  }
  strokeText(text, x, y) { this.fillText(text, x, y); }
  measureText(t) {
    // (刀97) width = Σ per-glyph advances from the captured Chrome 145
    // tables (font_metrics.js). Generic families resolve to the default
    // port's mapping; persona-installed fonts carry their real table;
    // fonts the persona claims but the capture Mac lacks keep the stable
    // spread factor on top of the DEFAULT table (still ≥1% off the
    // fallback, like a real installed face); everything else falls back
    // to the default table — Chrome's absent-family behavior, not a
    // uniform per-character step. Kerning pairs are NOT modeled (the
    // kern rows in fonts-metrics-truth-145.json document the residue).
    // (刀117) assignments parse into a spec on the hidden font slot
    // (_dFontSlots, set by the prototype accessor) — resolved px, style/
    // weight/variant and the family list, pinned by the k117a–d chrome
    // probes. The regex decomposition stays as the defensive path for a
    // context carrying no spec (a snapshot minted before the accessor).
    const fsp = _dFontSlots.get(this) || null;
    let famPart = fsp ? '' : String(this.font || '');
    let prePart = '';
    let px = fsp ? fsp.px : 10;
    if (!fsp) {
      const sizeHit = famPart.match(/(?:^|\s)(\d+(?:\.\d+)?)(px|pt|em|rem|%)\b/i);
      // (刀99) face tokens live BEFORE the size token ('bold 100px Arial'):
      // capture that prefix while famPart still holds the full string.
      prePart = sizeHit ? famPart.slice(0, sizeHit.index) : '';
      if (sizeHit) {
        famPart = famPart.slice(sizeHit.index + sizeHit[0].length).replace(/^\s*\/\s*[\d.]+(?:px|pt|em|rem|%)?\s*/, ' ').trim();
      }
    }
    if (arguments.length < 1) throw new TypeError("Failed to execute 'measureText' on 'CanvasRenderingContext2D': 1 argument required, but only 0 present.");
    const text = String(t);
    // (刀74) real font size for the metrics model (px/pt/em/rem; 0 -> all-zero metrics)
    if (!fsp) {
      const mm = String(this.font || '').match(/(?:^|\s)(\d+(?:\.\d+)?|\.\d+)(px|pt|em|rem)\b/i);
      if (mm) px = parseFloat(mm[1]) * ({ px: 1, pt: 4 / 3, em: 16, rem: 16 }[mm[2].toLowerCase()]);
    }
    const FM = globalThis.__ditingFontMetrics;
    let tbl = null, factor = 1;
    if (FM) {
      // (k97) quoted vs unquoted matters for generic names in Chrome: the
      // unquoted generic resolves to its real face (monospace=Menlo…), a
      // quoted "monospace" matches a different hidden face — the quoted
      // rows live in the metrics tables as plain name entries, so generic()
      // only fires for unquoted tokens and lookup() serves the rest.
      // (刀117) spec families re-enter this loop as synthetic raw strings so
      // the quoted-generic (GENQ) vs unquoted-generic split below is shared.
      const famIter = fsp ? fsp.fams.map((f) => (f.q ? '"' + f.n + '"' : f.n)) : famPart.split(',');
      for (const tok of famIter) {
        const raw = tok.trim();
        const name = raw.replace(/^['"]|['"]$/g, '');
        if (!name) continue;
        const quoted = raw.length > 0 && raw[0] !== name[0];
        if (!quoted) {
          const g = FM.generic(name);
          if (g) { tbl = g; break; }
        } else {
          // (刀99) quoted generics resolve to hidden faces (quoted
          // "monospace" is NOT Menlo) — GENQ carries those tables.
          const q = FM.genq ? FM.genq(name) : null;
          if (q) { tbl = q; break; }
        }
        const t2 = FM.lookup(name);
        if (t2) { tbl = t2; break; }
        const f = _installedFontFactor(name);
        if (f !== null) { factor = f; break; }
      }
      if (!tbl) tbl = FM.generic('sans-serif');
    }
    // (刀99) face axis: style/variant/weight tokens select a per-font FACE
    // table (bold/italic faces, small-caps substitution, numeric weights).
    // (刀100 d) under small-caps, weight/italic still select a face: chrome
    // synthesizes caps on the resolved weight/italic instance, and most
    // families' composites are DISTINCT (fonts-faces-truth scLattice maps
    // each variant to the family's captured composite tag; variants whose
    // lattice entry is absent collapse to S — that collapse is itself
    // chrome-measured, not a fallback guess).
    let face = null;
    if (fsp) {
      // (刀117) spec-driven selection — same lattice semantics, but weight is
      // the RESOLVED int (bolder→700, lighter→100, 450.5→450) and only a
      // NUMERIC weight keeps its own lattice key: keyword bold stays the 'b'
      // class even though both resolve to 700. oblique-with-angle reads as
      // normal here (computed style), so its face is the regular one.
      const wv = fsp.weight;
      const italTok = fsp.italic;
      const boldTok = wv >= 600;
      if (fsp.smallCaps) {
        let vkey = null;
        if (fsp.weightNum && wv >= 100 && wv <= 900 && wv !== 400) vkey = String(wv);
        else if (boldTok) vkey = italTok ? 'z' : 'b';
        else if (italTok) vkey = 'i';
        face = (tbl && tbl.scL ? tbl.scL[vkey] : null) || 'S';
      }
      else {
        // only a NUMERIC weight owns a lattice key ('700' ≠ 'Z' for Avenir
        // Next — 刀100 never-fold law); keyword bold resolves to 700 but
        // must stay on the 'B'/'Z' class letters.
        if (fsp.weightNum && wv === 500) face = 'M';
        else if (fsp.weightNum && wv >= 100 && wv <= 300) face = String(wv);
        // k117: numeric >=600 keeps its own lattice key ONLY when the family
        // captured that face; a MISSING key folds to the class letters with
        // the italic honored ('700 italic' Avenir Next renders Z in chrome —
        // the old missing-key fallback dropped the italic and read B).
        else if (fsp.weightNum && wv >= 600) {
          const nk = String(wv);
          face = (tbl && tbl.f && tbl.f[nk]) ? nk : (italTok ? 'Z' : 'B');
        }
        else if (boldTok && italTok) face = 'Z';
        else if (boldTok) face = 'B';
        else if (italTok) face = 'I';
      }
    }
    else {
      const preL = prePart.toLowerCase();
      if (/\bsmall-caps\b/.test(preL)) {
        const wm = preL.match(/(?:^|\s)(\d{3})\b/);
        const wv = wm ? parseInt(wm[1], 10) : 0;
        const italTok = /\b(italic|oblique)\b/.test(preL);
        const boldTok = /\b(bold|bolder)\b/.test(preL);
        // numeric weights are their OWN lattice keys — 800-sc ≠ bold-sc for
        // Gill Sans (S800 class ≠ SB class), so never fold wv>=700 into 'b'.
        let vkey = null;
        if (wv >= 100 && wv <= 900 && wv !== 400) vkey = String(wv);
        else if (boldTok) vkey = italTok ? 'z' : 'b';
        else if (italTok) vkey = 'i';
        // lattice carries only captured composites; everything else swallows
        face = (tbl && tbl.scL ? tbl.scL[vkey] : null) || 'S';
      }
      else {
        const wm = preL.match(/(?:^|\s)(\d{3})\b/);
        const wv = wm ? parseInt(wm[1], 10) : 0;
        const boldTok = /\b(bold|bolder)\b/.test(preL) || wv >= 600;
        const italTok = /\b(italic|oblique)\b/.test(preL);
        if (wv === 500) face = 'M';
        else if (wv >= 100 && wv <= 300) face = String(wv);
        // k117: same missing-key fold as the spec branch — italic honored.
        else if (wv >= 600) {
          const nk = String(wv);
          face = (tbl && tbl.f && tbl.f[nk]) ? nk : (italTok ? 'Z' : 'B');
        }
        else if (boldTok && italTok) face = 'Z';
        else if (boldTok) face = 'B';
        else if (italTok) face = 'I';
      }
    }
    let ftbl = tbl;
    if (face && tbl && tbl.f) {
      ftbl = tbl.f[face];
      if (!ftbl) {
        // (刀100 d) sc tags fall back S-first; numeric >=600 keeps the old
        // B fallback; every other miss (=R faces: I/200/300…) reads the base table.
        if (face[0] === 'S') ftbl = tbl.f.S || tbl;
        else if (+face >= 600) ftbl = tbl.f.B || tbl;
        else ftbl = tbl;
      }
    }
    const fl = (fsp ? fsp.fams.map((f) => f.n).join(' ') : famPart).toLowerCase();
    const family = /mono|courier|menlo|consolas/.test(fl) ? 'mono' : /times|serif(?!.*sans)|georgia/.test(fl) && !/sans/.test(fl) ? 'times' : 'arial';
    const zero = px <= 0;
    let w = 0;
    if (!zero) {
      if (FM) w = Math.round(FM.sum(ftbl, text, px) * factor * 1e6) / 1e6;
      else { const fontSize = parseInt(this.font) || 10; const scale = Math.max(1, Math.round(fontSize / 10)); w = Math.round(text.length * 6 * scale * factor * 100) / 100; }
    }
    return globalThis.__diting_newTextMetrics({ width: zero ? 0 : w, size: zero ? 0 : px, family, text, baseline: this.textBaseline, align: this.textAlign, vmet: FM && !zero ? FM.vertical(ftbl, px) : null });
  }
  getImageData(x, y, w, h) {
    x=Math.round(x); y=Math.round(y); w=Math.round(w); h=Math.round(h);
    const data = new Uint8ClampedArray(w * h * 4);
    for (let py = 0; py < h; py++) {
      for (let px = 0; px < w; px++) {
        const srcX = x + px, srcY = y + py;
        const dstIdx = (py * w + px) * 4;
        if (srcX >= 0 && srcX < this._w && srcY >= 0 && srcY < this._h) {
          const srcIdx = (srcY * this._w + srcX) * 4;
          data[dstIdx] = this._buf[srcIdx];
          data[dstIdx+1] = this._buf[srcIdx+1];
          data[dstIdx+2] = this._buf[srcIdx+2];
          data[dstIdx+3] = this._buf[srcIdx+3];
        }
      }
    }
    return { data, width: w, height: h };
  }
  putImageData(imageData, dx, dy) {
    dx=Math.round(dx); dy=Math.round(dy);
    const {data, width: w, height: h} = imageData;
    for (let py = 0; py < h; py++) {
      for (let px = 0; px < w; px++) {
        const srcIdx = (py * w + px) * 4;
        const x = dx + px, y = dy + py;
        if (x >= 0 && x < this._w && y >= 0 && y < this._h) {
          const dstIdx = (y * this._w + x) * 4;
          this._buf[dstIdx] = data[srcIdx];
          this._buf[dstIdx+1] = data[srcIdx+1];
          this._buf[dstIdx+2] = data[srcIdx+2];
          this._buf[dstIdx+3] = data[srcIdx+3];
        }
      }
    }
  }
  createImageData(w, h) { return { data: new Uint8ClampedArray(w*h*4), width: w, height: h }; }
  drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh) {
    if (!(img && img._ctx && img._ctx._buf)) return;
    const src = img._ctx;
    // (#164) Arg shuffles: 3-arg (dx,dy) and 5-arg (dx,dy,dw,dh) forms
    // draw the whole source; 9-arg is the full source→dest mapping.
    if (arguments.length < 9) {
      dx = sx; dy = sy; dw = sw; dh = sh; // 3-arg: both undefined → whole src
      sx = 0; sy = 0; sw = src._w; sh = src._h;
    }
    dw = dw ?? sw; dh = dh ?? sh;
    const m = this._m;
    if (m[1] !== 0 || m[2] !== 0) return; // rotated draw: out of scope
    const Dx = m[0]*dx + m[4], Dy = m[3]*dy + m[5];
    const Dw = Math.abs(m[0]*dw), Dh = Math.abs(m[3]*dh);
    if (!(Dw > 0) || !(Dh > 0)) return;
    const fx = sw / Dw, fy = sh / Dh;
    const x0 = Math.round(Dx), y0 = Math.round(Dy);
    for (let py = 0; py < Dh; py++) {
      for (let px = 0; px < Dw; px++) {
        const srcX = Math.floor(sx + px * fx), srcY = Math.floor(sy + py * fy);
        if (srcX >= 0 && srcX < src._w && srcY >= 0 && srcY < src._h) {
          const srcIdx = (srcY * src._w + srcX) * 4;
          this._setPixel(x0 + px, y0 + py, src._buf[srcIdx], src._buf[srcIdx+1], src._buf[srcIdx+2], src._buf[srcIdx+3]);
        }
      }
    }
  }
  // ---- (#164) transforms: real CTM, user-space paths, scanline raster ----
  _mul(o) {
    const m = this._m;
    this._m = [
      m[0]*o[0] + m[2]*o[1],
      m[1]*o[0] + m[3]*o[1],
      m[0]*o[2] + m[2]*o[3],
      m[1]*o[2] + m[3]*o[3],
      m[0]*o[4] + m[2]*o[5] + m[4],
      m[1]*o[4] + m[3]*o[5] + m[5],
    ];
  }
  _tp(x, y) {
    const m = this._m;
    return [m[0]*x + m[2]*y + m[4], m[1]*x + m[3]*y + m[5]];
  }
  _inv(x, y) {
    const m = this._m, det = m[0]*m[3] - m[1]*m[2];
    if (!det) return [x, y];
    return [
      (m[3]*(x - m[4]) - m[2]*(y - m[5])) / det,
      (m[0]*(y - m[5]) - m[1]*(x - m[4])) / det,
    ];
  }
  translate(x, y) { this._mul([1, 0, 0, 1, x, y]); }
  scale(x, y) { if (arguments.length < 2) y = x; this._mul([x, 0, 0, y, 0, 0]); }
  rotate(a) { const c = Math.cos(a), s = Math.sin(a); this._mul([c, s, -s, c, 0, 0]); }
  transform(a, b, c, d, e, f) { this._mul([a, b, c, d, e, f]); }
  setTransform(a, b, c, d, e, f) {
    this._m = (arguments.length >= 6) ? [a, b, c, d, e, f] : [1, 0, 0, 1, 0, 0];
  }
  resetTransform() { this._m = [1, 0, 0, 1, 0, 0]; }
  getTransform() {
    const m = this._m;
    return { a: m[0], b: m[1], c: m[2], d: m[3], e: m[4], f: m[5] };
  }
  _p2d() { if (!this._path2d) this._path2d = new Path2D(); return this._path2d; }
  beginPath() { this._path2d = new Path2D(); }
  moveTo(x, y) { this._p2d().moveTo(x, y); }
  lineTo(x, y) { this._p2d().lineTo(x, y); }
  closePath() { this._p2d().closePath(); }
  bezierCurveTo(a, b, c, d, e, f) { this._p2d().bezierCurveTo(a, b, c, d, e, f); }
  quadraticCurveTo(a, b, c, d) { this._p2d().quadraticCurveTo(a, b, c, d); }
  arc(x, y, r, s, e, ccw) { this._p2d().arc(x, y, r, s, e, ccw); }
  arcTo(x1, y1, x2, y2, r) { this._p2d().arcTo(x1, y1, x2, y2, r); }
  ellipse(x, y, rx, ry, rot, s, e, ccw) { this._p2d().ellipse(x, y, rx, ry, rot, s, e, ccw); }
  rect(x, y, w, h) { this._p2d().rect(x, y, w, h); }
  // Scanline polygon rasterizer with nonzero winding (default) and
  // even-odd. Fill closes open subpaths implicitly, like Chrome.
  _fillDevicePolys(polys, rgba, rule) {
    const edges = [];
    let ylo = Infinity, yhi = -Infinity;
    const push = (x1, y1, x2, y2) => {
      if (y1 === y2) return;
      edges.push([x1, y1, x2, y2]);
      if (y1 < ylo) ylo = y1;
      if (y1 > yhi) yhi = y1;
      if (y2 < ylo) ylo = y2;
      if (y2 > yhi) yhi = y2;
    };
    for (const pts of polys) {
      if (!pts || pts.length < 2) continue;
      let p = pts[0];
      for (let i = 1; i < pts.length; i++) { push(p[0], p[1], pts[i][0], pts[i][1]); p = pts[i]; }
      push(p[0], p[1], pts[0][0], pts[0][1]);
    }
    if (!edges.length) return;
    const evenodd = rule === 'evenodd';
    const py0 = Math.max(0, Math.floor(ylo)), py1 = Math.min(this._h - 1, Math.ceil(yhi));
    for (let py = py0; py <= py1; py++) {
      const yc = py + 0.5;
      const evs = [];
      for (const e of edges) {
        if ((e[1] <= yc && yc < e[3]) || (e[3] <= yc && yc < e[1])) {
          const t = (yc - e[1]) / (e[3] - e[1]);
          evs.push([e[0] + t * (e[2] - e[0]), e[3] > e[1] ? 1 : -1]);
        }
      }
      if (!evs.length) continue;
      evs.sort((a, b) => a[0] - b[0]);
      let wind = 0, span = 0, open = false;
      for (const ev of evs) {
        wind += ev[1];
        const inside = evenodd ? (wind % 2 !== 0) : wind !== 0;
        if (inside && !open) { span = ev[0]; open = true; }
        else if (!inside && open) {
          const px0 = Math.max(0, Math.ceil(span - 0.5));
          const px1 = Math.min(this._w, Math.ceil(ev[0] - 0.5));
          for (let px = px0; px < px1; px++) this._setPixel(px, py, rgba[0], rgba[1], rgba[2], rgba[3]);
          open = false;
        }
      }
    }
  }
  fill(path, rule) {
    if (typeof path === 'string') { rule = path; path = undefined; }
    const subs = (path && path._subs) ? path._subs : this._p2d()._subs;
    const polys = [];
    for (const s of subs) {
      if (!s || s.p.length < 2) continue;
      const pts = new Array(s.p.length);
      for (let i = 0; i < s.p.length; i++) pts[i] = this._tp(s.p[i][0], s.p[i][1]);
      polys.push(pts);
    }
    this._fillDevicePolys(polys, this._parseColor(this.fillStyle), rule);
  }
  // Stroke = square pen stamps along the transformed polylines. lineWidth
  // < 1 clamps to 1 device px (Chrome never strokes sub-pixel-thin).
  stroke(path) {
    const subs = (path && path._subs) ? path._subs : this._p2d()._subs;
    const rgba = this._parseColor(this.strokeStyle);
    const rad = Math.max(1, Math.round(Math.max(1, this.lineWidth) / 2));
    for (const s of subs) {
      if (!s || s.p.length < 2) continue;
      const pts = s.p.map((p) => this._tp(p[0], p[1]));
      for (let i = 0; i + 1 < pts.length; i++) this._stampSeg(pts[i], pts[i+1], rad, rgba);
      if (s.c) this._stampSeg(pts[pts.length-1], pts[0], rad, rgba);
    }
  }
  _stampSeg(p, q, rad, rgba) {
    const dx = q[0] - p[0], dy = q[1] - p[1];
    const steps = Math.max(1, Math.ceil((Math.abs(dx) + Math.abs(dy)) * 2));
    for (let i = 0; i <= steps; i++) {
      const x = p[0] + dx * i / steps, y = p[1] + dy * i / steps;
      for (let oy = -rad; oy <= rad; oy++) {
        for (let ox = -rad; ox <= rad; ox++) {
          this._setPixel(x + ox, y + oy, rgba[0], rgba[1], rgba[2], rgba[3]);
        }
      }
    }
  }
  clip() {}
  isPointInPath(a, b, c) {
    let path = null, x, y, rule;
    if (a && a._subs) { path = a; x = b; y = c; } else { x = a; y = b; rule = c; }
    const [ux, uy] = this._inv(x, y);
    const subs = (path || this._p2d())._subs;
    const evenodd = rule === 'evenodd';
    let wind = 0;
    for (const s of subs) {
      if (!s || s.p.length < 2) continue;
      for (let i = 0, j = s.p.length - 1; i < s.p.length; j = i++) {
        const yi = s.p[i][1], yj = s.p[j][1];
        if ((yi > uy) !== (yj > uy)) {
          const xint = s.p[j][0] + (uy - yj) * (s.p[i][0] - s.p[j][0]) / (yi - yj);
          if (ux < xint) wind += evenodd ? 1 : (yi > yj ? 1 : -1);
        }
      }
    }
    return evenodd ? (wind % 2 !== 0) : wind !== 0;
  }
  isPointInStroke(a, b) {
    const path = (a && a._subs) ? a : null;
    const x = path ? b : a, y = path ? undefined : b;
    if (y === undefined) return false;
    const [ux, uy] = this._inv(x, y);
    const subs = (path || this._p2d())._subs;
    const tol = Math.max(0.5, this.lineWidth / 2);
    for (const s of subs) {
      if (!s || s.p.length < 2) continue;
      const n = s.c ? s.p.length : s.p.length - 1;
      for (let i = 0; i < n; i++) {
        const p1 = s.p[i], p2 = s.p[(i+1) % s.p.length];
        const dx = p2[0]-p1[0], dy = p2[1]-p1[1];
        const len2 = dx*dx + dy*dy;
        const t = len2 ? Math.max(0, Math.min(1, ((ux-p1[0])*dx + (uy-p1[1])*dy) / len2)) : 0;
        const px = p1[0] + dx*t, py = p1[1] + dy*t;
        if (Math.hypot(ux - px, uy - py) <= tol) return true;
      }
    }
    return false;
  }
  save() {
    this._stateStack.push({
      fillStyle: this.fillStyle, strokeStyle: this.strokeStyle,
      globalAlpha: this.globalAlpha, font: this.font, lineWidth: this.lineWidth,
      textAlign: this.textAlign, textBaseline: this.textBaseline,
      _m: this._m.slice(),
    });
  }
  restore() {
    const s = this._stateStack.pop();
    if (s) { const m = s._m; Object.assign(this, s); this._m = m; }
  }
}

// (刀117) font is a Chrome-shaped prototype accessor (shape audit 'rw:ec'):
// a rejected assignment leaves the previous value untouched (the setter
// silently ignores), and the getter returns the parsed canonical serial.
// The spec itself lives in the closure WeakMap — the context keeps ZERO
// own properties for font, like Chrome.
Object.defineProperty(_Canvas2D.prototype, 'font', {
  get: function () { const s = _dFontSlots.get(this); return s ? s.serial : '10px sans-serif'; },
  set: function (v) { const sp = _ditingParseFont(v); if (sp) _dFontSlots.set(this, sp); },
  enumerable: true,
  configurable: true,
});

const __diting_canvasGetContext = function(type) {
  // (#164) width/height were plain own data properties (parser/JS writes
  // never reflected to the content attribute). Fold any pre-ctx write into
  // the attribute so the prototype accessors installed below see one truth
  // from here on, then drop the own property so they take over.
  for (const dim of ['width', 'height']) {
    const d = Object.getOwnPropertyDescriptor(this, dim);
    if (d && 'value' in d && d.configurable) {
      let v = Math.floor(Number(d.value));
      if (!Number.isFinite(v) || v < 0) v = 0;
      if (v > 32767) v = 32767;
      this.setAttribute(dim, String(v));
      delete this[dim];
    }
  }
  // (#117 + fp-diff-1014 #5) One context type per canvas, Chrome semantics:
  // after a 2d context, webgl/webgl2 return null (and vice versa). 'webgl'
  // and 'experimental-webgl' alias each other. The old code happily served
  // 2d, webgl AND webgl2 off the same element — Chrome never does.
  const _ctxNorm = (type === 'experimental-webgl') ? 'webgl' : type;
  if (this._ctxType && this._ctxType !== _ctxNorm) return null;
  if (type === '2d') {
    if (!this._ctx) {
      __def(this, '_ctxType', _ctxNorm);
      __def(this, '_ctx', new _Canvas2D(this));
    }
    return this._ctx;
  }
  if (type === 'webgl' || type === 'experimental-webgl' || type === 'webgl2') {
    if (this._glCtx) return this._glCtx;
    // (P8 #300015) Chrome shape: the instance carries ZERO own properties of
    // any kind — every method/constant/accessor resolves through the
    // WebGL{,2}RenderingContext.prototype tables, and the canvas link lives
    // in _glCanvasMap. The old base object shadowed ~80 methods and constants
    // as own props, so Object.keys(gl) leaked the whole engine.
    const base = {};
    Object.setPrototypeOf(base, (type === 'webgl2')
      ? globalThis.WebGL2RenderingContext.prototype
      : globalThis.WebGLRenderingContext.prototype);
    // (刀69) The Proxy only exists to hand accessors the base as `this`. It
    // used to also turn every name the tables lack into a number-ish no-op;
    // the tables are complete against Chrome now (member order/kinds all
    // match), so a missing name is undefined exactly like Chrome — webgl1 has
    // no fenceSync, `gl.fenceSync` is falsy, feature detection works.
    const gl = new Proxy(base, {
      get(target, prop) {
        // Reflect.get keeps the accessor receiver on `target`, so the
        // canvas/drawingBuffer* getters resolve their WeakMap entry (keyed on base).
        return Reflect.get(target, prop, target);
      },
    });
    _glBaseOf.set(gl, base);
    _glCanvasMap.set(base, this);
    {
      // WebGLContextAttributes: booleans coerce through ToBoolean (undefined = the
      // member's default), powerPreference is an enum whose bad value is a TypeError.
      const o = arguments[1], a = {};
      if (o !== null && o !== undefined && typeof o === 'object') {
        for (const k of ['alpha', 'antialias', 'depth', 'desynchronized', 'failIfMajorPerformanceCaveat', 'premultipliedAlpha', 'preserveDrawingBuffer', 'stencil', 'xrCompatible']) {
          if (o[k] !== undefined) a[k] = !!o[k];
        }
        if (o.powerPreference !== undefined) {
          const pp = String(o.powerPreference);
          if (pp !== 'default' && pp !== 'high-performance' && pp !== 'low-power') {
            throw new TypeError("Failed to execute 'getContext' on 'HTMLCanvasElement': Failed to read the 'powerPreference' property from 'WebGLContextAttributes': The provided value '" + pp + "' is not a valid enum value of type WebGLPowerPreference.");
          }
          a.powerPreference = pp;
        }
      }
      _glAttrs.set(base, a);
    }
    __def(this, '_ctxType', _ctxNorm);
    __def(this, '_glCtx', gl);
    return gl;
  }
  return null;
};
const __diting_canvasToDataURL = function(type, quality) {
  const ctx = this._ctx;
  // (300015 P4) Type dispatch: 'image/jpeg' on a DRAWN canvas returns real
  // baseline JPEG bytes. The XHS create page fingerprints a drawn canvas
  // via toDataURL('image/jpeg') and checks the bytes — PNG bytes under a
  // png data: prefix here were a one-line in-page detect. Out-of-range /
  // non-number quality is IGNORED per spec (default 0.92), not clamped.
  var jt = String(type == null ? '' : type).split(';')[0].trim().toLowerCase();
  var isJpeg = jt === 'image/jpeg' || jt === 'image/jpg' || jt === 'jpeg' || jt === 'jpg';
  var q = (typeof quality === 'number' && isFinite(quality) && quality >= 0 && quality <= 1)
    ? quality : 0.92;
  if (ctx && ctx._buf && (isJpeg ? _OPS.op_canvas_jpeg : _OPS.op_canvas_png)) {
    if (isJpeg) {
      try {
        const px = new Uint8Array(ctx._buf.buffer, ctx._buf.byteOffset, ctx._buf.byteLength);
        const jpg = new Uint8Array(_OPS.op_canvas_jpeg(ctx._w, ctx._h, px, q));
        let s = '';
        for (let i = 0; i < jpg.length; i += 0x8000) {
          s += String.fromCharCode.apply(null, jpg.subarray(i, i + 0x8000));
        }
        return 'data:image/jpeg;base64,' + btoa(s);
      } catch (e) { /* fall through to the png path */ }
    }
    try {
      // (#117) A REAL PNG of the 2D buffer — decodable and dimension-correct
      // server-side. The old fixed-shape fake base64 failed any PNG decode.
      // The buffer is Uint8ClampedArray, which op2's #[buffer] arg rejects
      // ("expected typed ArrayBufferView") — re-view it as Uint8Array over
      // the same memory, zero-copy.
      const px = new Uint8Array(ctx._buf.buffer, ctx._buf.byteOffset, ctx._buf.byteLength);
      const png = new Uint8Array(_OPS.op_canvas_png(ctx._w, ctx._h, px));
      let s = '';
      // Chunked: one fromCharCode(...) over the whole buffer blows the arg
      // limit on real canvas sizes.
      for (let i = 0; i < png.length; i += 0x8000) {
        s += String.fromCharCode.apply(null, png.subarray(i, i + 0x8000));
      }
      return 'data:image/png;base64,' + btoa(s);
    } catch (e) { /* fall through to the blank encode */ }
  }
  // (300015 P3) A canvas with no 2D context is a pristine blank surface, not
  // a fake-stub case: Chrome encodes real bytes of all-zero pixels. The old
  // seeded `_fp('canvasFingerprint')` stub returned an 86-char corrupt PNG —
  // exactly what the XHS shield's toDataURL('image/webp') probe hashed. The
  // XHS-relevant formats/dims are layered over by canvas_face.js; this is
  // the honest floor for everything else.
  try {
    if (_OPS.op_canvas_png) {
      const w = this.width || 300, h = this.height || 150;
      const px = new Uint8ClampedArray(w * h * 4);
      const png = new Uint8Array(_OPS.op_canvas_png(w, h, px));
      let s = '';
      for (let i = 0; i < png.length; i += 0x8000) {
        s += String.fromCharCode.apply(null, png.subarray(i, i + 0x8000));
      }
      return 'data:image/png;base64,' + btoa(s);
    }
  } catch (e) {}
  return _fp('canvasFingerprint');
};
const __diting_canvasToBlob = function(cb, type, quality) {
  const ctx = this._ctx;
  // (300015 P4) Same type dispatch as toDataURL above.
  var jt = String(type == null ? '' : type).split(';')[0].trim().toLowerCase();
  var isJpeg = jt === 'image/jpeg' || jt === 'image/jpg' || jt === 'jpeg' || jt === 'jpg';
  var q = (typeof quality === 'number' && isFinite(quality) && quality >= 0 && quality <= 1)
    ? quality : 0.92;
  if (ctx && ctx._buf && (isJpeg ? _OPS.op_canvas_jpeg : _OPS.op_canvas_png)) {
    if (isJpeg) {
      try {
        const px = new Uint8Array(ctx._buf.buffer, ctx._buf.byteOffset, ctx._buf.byteLength);
        cb(new Blob([new Uint8Array(_OPS.op_canvas_jpeg(ctx._w, ctx._h, px, q))], { type: 'image/jpeg' }));
        return;
      } catch (e) { /* fall through to the png path */ }
    }
    try {
      const px = new Uint8Array(ctx._buf.buffer, ctx._buf.byteOffset, ctx._buf.byteLength);
      cb(new Blob([new Uint8Array(_OPS.op_canvas_png(ctx._w, ctx._h, px))], { type: 'image/png' }));
      return;
    } catch (e) {}
  }
  // (300015 P3) Same floor as toDataURL above: no-ctx canvas → real blank
  // PNG blob, never the empty-string blob.
  try {
    if (_OPS.op_canvas_png) {
      const w = this.width || 300, h = this.height || 150;
      const px = new Uint8ClampedArray(w * h * 4);
      cb(new Blob([new Uint8Array(_OPS.op_canvas_png(w, h, px))], { type: 'image/png' }));
      return;
    }
  } catch (e) {}
  cb(new Blob(['']));
};

// The 2D context implementation class IS the interface: `ctx instanceof CanvasRenderingContext2D`, `ctx.constructor.name` and
// Object.prototype.toString.call(ctx) must read like Chrome's (they used to expose `_Canvas2D` / "[object Object]", with the
// global an empty stub class the instances never inherited from).
Object.defineProperty(_Canvas2D, 'name', { value: 'CanvasRenderingContext2D', configurable: true });
globalThis.CanvasRenderingContext2D = _Canvas2D;

// Canvas compositing (#210): the 2D buffer is a JS-side array, so the page
// painter has no way to see it — screenshots rendered canvas boxes empty
// (the merchant QR went gray while toDataURL stayed correct). The host
// evaluates this before a render: every drawn canvas snapshots its buffer
// into the engine's bitmap table (op_canvas_store), which marks a
// paint-only invalidation so the frame actually repaints. Absent in
// builds without the screenshot feature — the guard keeps this a no-op.
globalThis.__diting_flush_canvas_bitmaps = function __diting_flush_canvas_bitmaps() {
  try {
    if (!_OPS.op_canvas_store) return 0;
    const els = document.querySelectorAll('canvas');
    let n = 0;
    for (let i = 0; i < els.length; i++) {
      const el = els[i];
      const ctx = el._ctx;
      if (!ctx || !ctx._buf || el._nid === undefined) continue;
      try {
        // Uint8ClampedArray is rejected by the #[buffer] arg — re-view the
        // same memory as Uint8Array, zero-copy (same reason as toDataURL).
        const px = new Uint8Array(ctx._buf.buffer, ctx._buf.byteOffset, ctx._buf.byteLength);
        if (_OPS.op_canvas_store(el._nid, ctx._w, ctx._h, px)) n++;
      } catch (_e) {}
    }
    return n;
  } catch (_e) { return 0; }
};
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__diting_canvasGetContext', __diting_canvasGetContext);
  d('__diting_canvasToBlob', __diting_canvasToBlob);
  d('__diting_canvasToDataURL', __diting_canvasToDataURL);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
