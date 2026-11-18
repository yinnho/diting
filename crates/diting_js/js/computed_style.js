(function () { // @@SCOPE
// god-file split: bootstrap's real native marker, reached through its
// hidden prop (house style — see serializer.js) so no cross-file load
// edge appears in the dependency ratchet.
const _markNative = globalThis.__diting_markNative;

// Chrome's enumerable computed-style property set (kebab-case); count AND
// enumeration are fingerprint surfaces (Castle cssKeys).
// Chrome 157's getComputedStyle longhand list, in Chrome's order (480 names; shorthands are NOT items of a computed declaration).
// Truth: Array.from(getComputedStyle(document.body)) on real Chrome 157.0.8103.38 (2026-11-06, #28): the 145 list's 413 kept
// their relative order, none removed, 67 inserted (rule/trigger families, scroll-margin/padding longhands, aspect-ratio, ...).
const _COMPUTED_PROPS_KEBAB = ('accent-color align-content align-items align-self alignment-baseline anchor-name anchor-scope animation-composition ' +
  'animation-delay animation-direction animation-duration animation-fill-mode animation-iteration-count animation-name ' +
  'animation-play-state animation-range-end animation-range-start animation-timeline animation-timing-function ' +
  'animation-trigger app-region appearance aspect-ratio backdrop-filter backface-visibility background-attachment ' +
  'background-blend-mode background-clip background-color background-image background-origin background-position ' +
  'background-repeat background-size baseline-shift baseline-source block-size border-block-end-color ' +
  'border-block-end-style border-block-end-width border-block-start-color border-block-start-style ' +
  'border-block-start-width border-bottom-color border-bottom-left-radius border-bottom-right-radius border-bottom-style ' +
  'border-bottom-width border-collapse border-end-end-radius border-end-start-radius border-image-outset ' +
  'border-image-repeat border-image-slice border-image-source border-image-width border-inline-end-color ' +
  'border-inline-end-style border-inline-end-width border-inline-start-color border-inline-start-style ' +
  'border-inline-start-width border-left-color border-left-style border-left-width border-right-color border-right-style ' +
  'border-right-width border-shape border-start-end-radius border-start-start-radius border-top-color ' +
  'border-top-left-radius border-top-right-radius border-top-style border-top-width bottom box-decoration-break ' +
  'box-shadow box-sizing break-after break-before break-inside buffered-rendering caption-side caret-animation ' +
  'caret-color caret-shape clear clip clip-path clip-rule color color-interpolation color-interpolation-filters ' +
  'color-rendering color-scheme column-count column-fill column-gap column-height column-rule-break column-rule-color ' +
  'column-rule-inset-cap-end column-rule-inset-cap-start column-rule-inset-junction-end column-rule-inset-junction-start ' +
  'column-rule-style column-rule-visibility-items column-rule-width column-span column-width column-wrap contain ' +
  'contain-intrinsic-block-size contain-intrinsic-height contain-intrinsic-inline-size contain-intrinsic-size ' +
  'contain-intrinsic-width container-name container-type content content-visibility corner-bottom-left-shape ' +
  'corner-bottom-right-shape corner-end-end-shape corner-end-start-shape corner-start-end-shape corner-start-start-shape ' +
  'corner-top-left-shape corner-top-right-shape counter-increment counter-reset counter-set cursor cx cy d direction ' +
  'display dominant-baseline dynamic-range-limit empty-cells field-sizing fill fill-opacity fill-rule filter flex-basis ' +
  'flex-direction flex-grow flex-line-count flex-shrink flex-wrap float flood-color flood-opacity font-family ' +
  'font-feature-settings font-kerning font-language-override font-optical-sizing font-palette font-size font-size-adjust ' +
  'font-stretch font-style font-synthesis-small-caps font-synthesis-style font-synthesis-weight font-variant ' +
  'font-variant-alternates font-variant-caps font-variant-east-asian font-variant-emoji font-variant-ligatures ' +
  'font-variant-numeric font-variant-position font-variation-settings font-weight forced-color-adjust frame-sizing ' +
  'grid-auto-columns grid-auto-flow grid-auto-rows grid-column-end grid-column-start grid-row-end grid-row-start ' +
  'grid-template-areas grid-template-columns grid-template-rows height hyphenate-character hyphenate-limit-chars hyphens ' +
  'image-orientation image-rendering initial-letter inline-size inset-block-end inset-block-start inset-inline-end ' +
  'inset-inline-start interactivity interest-delay-end interest-delay-start interpolate-size isolation justify-content ' +
  'justify-items justify-self left letter-spacing lighting-color line-break line-height list-style-image ' +
  'list-style-position list-style-type margin-block-end margin-block-start margin-bottom margin-inline-end ' +
  'margin-inline-start margin-left margin-right margin-top margin-trim marker-end marker-mid marker-start mask-clip ' +
  'mask-composite mask-image mask-mode mask-origin mask-position mask-repeat mask-size mask-type math-depth math-shift ' +
  'math-style max-block-size max-height max-inline-size max-width min-block-size min-height min-inline-size min-width ' +
  'mix-blend-mode object-fit object-position object-view-box offset-anchor offset-distance offset-path offset-position ' +
  'offset-rotate opacity order orphans outline-color outline-offset outline-style outline-width overflow-anchor ' +
  'overflow-block overflow-clip-margin overflow-inline overflow-wrap overflow-x overflow-y overlay ' +
  'overscroll-behavior-block overscroll-behavior-inline overscroll-behavior-x overscroll-behavior-y padding-block-end ' +
  'padding-block-start padding-bottom padding-inline-end padding-inline-start padding-left padding-right padding-top ' +
  'paint-order perspective perspective-origin pointer-events position position-anchor position-area ' +
  'position-try-fallbacks position-try-order position-visibility print-color-adjust quotes r reading-flow reading-order ' +
  'resize right rotate row-gap row-rule-break row-rule-color row-rule-inset-cap-end row-rule-inset-cap-start ' +
  'row-rule-inset-junction-end row-rule-inset-junction-start row-rule-style row-rule-visibility-items row-rule-width ' +
  'ruby-align ruby-overhang ruby-position rule-overlap rx ry scale scroll-axis-lock scroll-behavior ' +
  'scroll-initial-target scroll-margin-block-end scroll-margin-block-start scroll-margin-bottom scroll-margin-inline-end ' +
  'scroll-margin-inline-start scroll-margin-left scroll-margin-right scroll-margin-top scroll-marker-group ' +
  'scroll-padding-block-end scroll-padding-block-start scroll-padding-bottom scroll-padding-inline-end ' +
  'scroll-padding-inline-start scroll-padding-left scroll-padding-right scroll-padding-top scroll-snap-align ' +
  'scroll-snap-stop scroll-snap-type scroll-target-group scroll-timeline-axis scroll-timeline-name scrollbar-color ' +
  'scrollbar-gutter scrollbar-width shape-image-threshold shape-margin shape-outside shape-rendering speak stop-color ' +
  'stop-opacity stroke stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit ' +
  'stroke-opacity stroke-width tab-size table-layout text-align text-align-last text-anchor text-autospace text-box-edge ' +
  'text-box-trim text-combine-upright text-decoration text-decoration-color text-decoration-inset text-decoration-line ' +
  'text-decoration-skip-ink text-decoration-style text-decoration-thickness text-emphasis-color text-emphasis-position ' +
  'text-emphasis-style text-fit text-indent text-justify text-orientation text-overflow text-rendering text-shadow ' +
  'text-size-adjust text-spacing-trim text-transform text-underline-offset text-underline-position text-wrap-mode ' +
  'text-wrap-style timeline-scope timeline-trigger-activation-range-end timeline-trigger-activation-range-start ' +
  'timeline-trigger-active-range-end timeline-trigger-active-range-start timeline-trigger-name timeline-trigger-source ' +
  'top touch-action transform transform-box transform-origin transform-style transition-behavior transition-delay ' +
  'transition-duration transition-property transition-timing-function translate trigger-scope unicode-bidi user-select ' +
  'vector-effect vertical-align view-timeline-axis view-timeline-inset view-timeline-name view-transition-class ' +
  'view-transition-group view-transition-name view-transition-scope visibility white-space-collapse widows width ' +
  'will-change window-drag word-break word-spacing writing-mode x y z-index zoom -webkit-border-horizontal-spacing ' +
  '-webkit-border-image -webkit-border-vertical-spacing -webkit-box-align -webkit-box-decoration-break ' +
  '-webkit-box-direction -webkit-box-flex -webkit-box-ordinal-group -webkit-box-orient -webkit-box-pack ' +
  '-webkit-box-reflect -webkit-font-smoothing -webkit-line-break -webkit-line-clamp -webkit-locale ' +
  '-webkit-mask-box-image -webkit-mask-box-image-outset -webkit-mask-box-image-repeat -webkit-mask-box-image-slice ' +
  '-webkit-mask-box-image-source -webkit-mask-box-image-width -webkit-mask-position-x -webkit-mask-position-y ' +
  '-webkit-rtl-ordering -webkit-ruby-position -webkit-tap-highlight-color -webkit-text-combine ' +
  '-webkit-text-decorations-in-effect -webkit-text-fill-color -webkit-text-orientation -webkit-text-security ' +
  '-webkit-text-stroke-color -webkit-text-stroke-width -webkit-user-drag -webkit-user-modify -webkit-writing-mode').split(' ');
const _camelCache = new Map();
const _camel = (kebab) => {
  let c = _camelCache.get(kebab);
  if (c === undefined) {
    c = kebab.replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
    _camelCache.set(kebab, c);
  }
  return c;
};
const _computedKebab = () => _COMPUTED_PROPS_KEBAB;

// getComputedStyle() wrappers share one immutable native snapshot per
// element until the DOM mutates — frameworks call getComputedStyle()
// repeatedly on the same few roots, and jQuery reads one property per
// call, so a fresh snapshot per wrapper would dominate real-page startup
// (upstream learned this the same way).
const _computedStyleSnapshotCache = new WeakMap();
// Pseudo-element wrappers cache separately: the host's snapshot holds only
// the element's own cascade, and (::before, ::after) live in per-element
// pseudo tables on the native side.
const _pseudoStyleSnapshotCache = new WeakMap();
// CSSOM GetComputedStyle argument grammar: null/undefined/'' select the
// element itself; the four legacy single-colon names stay valid; any
// `::name` is accepted (Chrome answers an initial-value declaration for
// ones it doesn't model — it does NOT throw for unknown `::` forms);
// everything else is a TypeError.
const _normalizePseudoElt = (pseudoElt) => {
  if (pseudoElt == null) return null;
  const s = String(pseudoElt).trim().toLowerCase();
  if (s === '') return null;
  if (s.startsWith('::')) return s;
  if (s === ':before' || s === ':after' || s === ':first-line' || s === ':first-letter') {
    return ':' + s;
  }
  throw new TypeError(`Failed to execute 'getComputedStyle' on 'Window': '${pseudoElt}' is not a valid pseudo-element.`);
};
const _NAMED_COLOR = {aliceblue:[240,248,255,255],antiquewhite:[250,235,215,255],aqua:[0,255,255,255],aquamarine:[127,255,212,255],azure:[240,255,255,255],beige:[245,245,220,255],bisque:[255,228,196,255],black:[0,0,0,255],blanchedalmond:[255,235,205,255],blue:[0,0,255,255],blueviolet:[138,43,226,255],brown:[165,42,42,255],burlywood:[222,184,135,255],cadetblue:[95,158,160,255],chartreuse:[127,255,0,255],chocolate:[210,105,30,255],coral:[255,127,80,255],cornflowerblue:[100,149,237,255],cornsilk:[255,248,220,255],crimson:[220,20,60,255],cyan:[0,255,255,255],darkblue:[0,0,139,255],darkcyan:[0,139,139,255],darkgoldenrod:[184,134,11,255],darkgray:[169,169,169,255],darkgreen:[0,100,0,255],darkgrey:[169,169,169,255],darkkhaki:[189,183,107,255],darkmagenta:[139,0,139,255],darkolivegreen:[85,107,47,255],darkorange:[255,140,0,255],darkorchid:[153,50,204,255],darkred:[139,0,0,255],darksalmon:[233,150,122,255],darkseagreen:[143,188,143,255],darkslateblue:[72,61,139,255],darkslategray:[47,79,79,255],darkslategrey:[47,79,79,255],darkturquoise:[0,206,209,255],darkviolet:[148,0,211,255],deeppink:[255,20,147,255],deepskyblue:[0,191,255,255],dimgray:[105,105,105,255],dimgrey:[105,105,105,255],dodgerblue:[30,144,255,255],firebrick:[178,34,34,255],floralwhite:[255,250,240,255],forestgreen:[34,139,34,255],fuchsia:[255,0,255,255],gainsboro:[220,220,220,255],ghostwhite:[248,248,255,255],gold:[255,215,0,255],goldenrod:[218,165,32,255],gray:[128,128,128,255],green:[0,128,0,255],greenyellow:[173,255,47,255],grey:[128,128,128,255],honeydew:[240,255,240,255],hotpink:[255,105,180,255],indianred:[205,92,92,255],indigo:[75,0,130,255],ivory:[255,255,240,255],khaki:[240,230,140,255],lavender:[230,230,250,255],lavenderblush:[255,240,245,255],lawngreen:[124,252,0,255],lemonchiffon:[255,250,205,255],lightblue:[173,216,230,255],lightcoral:[240,128,128,255],lightcyan:[224,255,255,255],lightgoldenrodyellow:[250,250,210,255],lightgray:[211,211,211,255],lightgreen:[144,238,144,255],lightgrey:[211,211,211,255],lightpink:[255,182,193,255],lightsalmon:[255,160,122,255],lightseagreen:[32,178,170,255],lightskyblue:[135,206,250,255],lightslategray:[119,136,153,255],lightslategrey:[119,136,153,255],lightsteelblue:[176,196,222,255],lightyellow:[255,255,224,255],lime:[0,255,0,255],limegreen:[50,205,50,255],linen:[250,240,230,255],magenta:[255,0,255,255],maroon:[128,0,0,255],mediumaquamarine:[102,205,170,255],mediumblue:[0,0,205,255],mediumorchid:[186,85,211,255],mediumpurple:[147,112,219,255],mediumseagreen:[60,179,113,255],mediumslateblue:[123,104,238,255],mediumspringgreen:[0,250,154,255],mediumturquoise:[72,209,204,255],mediumvioletred:[199,21,133,255],midnightblue:[25,25,112,255],mintcream:[245,255,250,255],mistyrose:[255,228,225,255],moccasin:[255,228,181,255],navajowhite:[255,222,173,255],navy:[0,0,128,255],oldlace:[253,245,230,255],olive:[128,128,0,255],olivedrab:[107,142,35,255],orange:[255,165,0,255],orangered:[255,69,0,255],orchid:[218,112,214,255],palegoldenrod:[238,232,170,255],palegreen:[152,251,152,255],paleturquoise:[175,238,238,255],palevioletred:[219,112,147,255],papayawhip:[255,239,213,255],peachpuff:[255,218,185,255],peru:[205,133,63,255],pink:[255,192,203,255],plum:[221,160,221,255],powderblue:[176,224,230,255],purple:[128,0,128,255],rebeccapurple:[102,51,153,255],red:[255,0,0,255],rosybrown:[188,143,143,255],royalblue:[65,105,225,255],saddlebrown:[139,69,19,255],salmon:[250,128,114,255],sandybrown:[244,164,96,255],seagreen:[46,139,87,255],seashell:[255,245,238,255],sienna:[160,82,45,255],silver:[192,192,192,255],skyblue:[135,206,235,255],slateblue:[106,90,205,255],slategray:[112,128,144,255],slategrey:[112,128,144,255],snow:[255,250,250,255],springgreen:[0,255,127,255],steelblue:[70,130,180,255],tan:[210,180,140,255],teal:[0,128,128,255],thistle:[216,191,216,255],tomato:[255,99,71,255],turquoise:[64,224,208,255],violet:[238,130,238,255],wheat:[245,222,179,255],white:[255,255,255,255],whitesmoke:[245,245,245,255],yellow:[255,255,0,255],yellowgreen:[154,205,50,255]};
// Chrome's computed spelling: opaque colors are rgb(), anything with alpha
// is rgba() with the shortest alpha that round-trips to the same 8-bit value.
function _formatCssomColor(c) {
  const r = c[0], g = c[1], b = c[2], a = c[3];
  if (a === 255) return "rgb(" + r + ", " + g + ", " + b + ")";
  const exact = a / 255;
  const two = Math.round(exact * 100) / 100;
  let s = (Math.round(two * 255) === a ? two.toFixed(2) : exact.toFixed(3));
  s = s.replace(/0+$/, "").replace(/\.$/, "");
  if (s === "") s = "0";
  return "rgba(" + r + ", " + g + ", " + b + ", " + s + ")";
}
function _chan255(tok) {
  if (tok.endsWith("%")) return Math.max(0, Math.min(255, Math.round(parseFloat(tok) * 2.55)));
  const n = parseFloat(tok);
  return isFinite(n) ? Math.max(0, Math.min(255, Math.round(n))) : null;
}
function _alpha255(tok) {
  if (tok == null || String(tok).trim() === "") return 255;
  tok = String(tok).trim();
  if (tok.endsWith("%")) return Math.max(0, Math.min(255, Math.round(parseFloat(tok) * 2.55)));
  const n = parseFloat(tok);
  return isFinite(n) ? Math.max(0, Math.min(255, Math.round(n * 255))) : null;
}
function _parseHexColor(s) {
  if (s[0] !== "#") return null;
  const h = s.slice(1);
  if (!/^[0-9a-f]+$/.test(h)) return null;
  const ex = (x) => parseInt(x, 16);
  if (h.length === 3 || h.length === 4) {
    return [ex(h[0] + h[0]), ex(h[1] + h[1]), ex(h[2] + h[2]), h.length === 4 ? ex(h[3] + h[3]) : 255];
  }
  if (h.length === 6 || h.length === 8) {
    return [ex(h.slice(0, 2)), ex(h.slice(2, 4)), ex(h.slice(4, 6)), h.length === 8 ? ex(h.slice(6, 8)) : 255];
  }
  return null;
}
function _splitColorFn(body) {
  const slash = body.indexOf("/");
  const head = slash < 0 ? body : body.slice(0, slash);
  const ch = head.split(/[\s,]+/).filter(Boolean);
  return [ch, slash < 0 ? null : body.slice(slash + 1)];
}
function _parseRgbFn(s) {
  const m = /^rgba?\((.*)\)$/.exec(s);
  if (!m) return null;
  const [ch, alphaTok] = _splitColorFn(m[1]);
  if (ch.length !== 3 && !(ch.length === 4 && alphaTok == null)) return null;
  const r = _chan255(ch[0]), g = _chan255(ch[1]), b = _chan255(ch[2]);
  const a = ch.length === 4 ? _alpha255(ch[3]) : _alpha255(alphaTok);
  if (r == null || g == null || b == null || a == null) return null;
  return [r, g, b, a];
}
function _hslToRgb(h, sat, lig, a) {
  h = ((h % 360) + 360) % 360;
  sat = Math.max(0, Math.min(100, sat)) / 100;
  lig = Math.max(0, Math.min(100, lig)) / 100;
  const c = (1 - Math.abs(2 * lig - 1)) * sat;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = lig - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; }
  else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; }
  else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; }
  else { r = c; b = x; }
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255), a];
}
function _parseHslFn(s) {
  const m = /^hsla?\((.*)\)$/.exec(s);
  if (!m) return null;
  const [ch, alphaTok] = _splitColorFn(m[1]);
  if (ch.length !== 3 && !(ch.length === 4 && alphaTok == null)) return null;
  let hue = ch[0];
  if (hue.endsWith("turn")) hue = parseFloat(hue) * 360;
  else hue = parseFloat(hue);
  const sat = parseFloat(ch[1]), lig = parseFloat(ch[2]);
  if (!isFinite(hue) || !isFinite(sat) || !isFinite(lig)) return null;
  const a = ch.length === 4 ? _alpha255(ch[3]) : _alpha255(alphaTok);
  if (a == null) return null;
  return _hslToRgb(hue, sat, lig, a);
}
// Legacy colors collapse to rgb()/rgba(). Modern functions (oklch, color())
// stay as written: this returns null and the caller keeps the authored text.
function _cssComputedColor(raw) {
  const s = String(raw).trim().toLowerCase();
  if (!s || s === "currentcolor") return null;
  const c = s === "transparent" ? [0, 0, 0, 0]
    : (_NAMED_COLOR[s] || _parseHexColor(s) || _parseRgbFn(s) || _parseHslFn(s));
  return c ? _formatCssomColor(c) : null;
}
function _isComputedColorProp(kebab) {
  return kebab === "color" || kebab.endsWith("-color");
}
// Keywords whose computed spelling is not the specified one. null means
// "leave the inline text alone".
function _cssComputedKeyword(kebab, raw) {
  const v = String(raw).trim().toLowerCase();
  if (kebab === 'column-rule-width' || kebab === 'row-rule-width') {
    if (v === 'thin') return '1px';
    if (v === 'medium') return '3px';
    if (v === 'thick') return '5px';
  }
  if ((kebab === 'column-rule-break' || kebab === 'row-rule-break') && v === 'auto') return 'normal';
  if (kebab.startsWith('corner-') && kebab.endsWith('-shape')) {
    const shape = { round: 'superellipse(1)', bevel: 'superellipse(0)', scoop: 'superellipse(-1)', notch: 'superellipse(-infinity)', squircle: 'superellipse(2)' };
    if (shape[v]) return shape[v];
  }
  if (kebab === 'offset-rotate') {
    if (v === 'auto') return 'auto 0deg';
    if (v === 'reverse') return 'auto 180deg';
  }
  if (kebab === 'text-emphasis-position') {
    const parts = v.split(/\s+/);
    const block = parts.find((p) => p === 'over' || p === 'under');
    const side = parts.find((p) => p === 'left' || p === 'right');
    if (block && side === 'right') return block;
    if (block && side === 'left') return block + ' left';
    if (block && !side) return block;
  }
  return null;
}
// Comma-separated longhands repeat the shorter side (CSS list alignment).
function _cssLayers(vals) {
  const lists = vals.map((v) => String(v).split(',').map((s) => s.trim()).filter((s) => s.length));
  if (lists.some((l) => l.length === 0)) return null;
  const n = lists.reduce((m, l) => Math.max(m, l.length), 0);
  const rows = [];
  for (let i = 0; i < n; i++) rows.push(lists.map((l) => l[i % l.length]));
  return rows;
}
// Chrome computed `animation` uses grammar order and drops initial pieces.
// All-initial is "none". A 0s duration stays only when a delay would
// otherwise be read as the duration. An explicit 0s with no other change
// is indistinguishable from the initial here (the cascade stores 0).
function _cssAnimationShorthand(dur, ease, delay, iter, dir, fill, play, name) {
  const rows = _cssLayers([dur, ease, delay, iter, dir, fill, play, name]);
  if (!rows) return '';
  return rows.map((r) => {
    const bits = [];
    const delaySet = r[2] !== '0s';
    if (r[0] !== '0s' || delaySet) bits.push(r[0]);
    if (r[1] !== 'ease') bits.push(r[1]);
    if (delaySet) bits.push(r[2]);
    if (r[3] !== '1') bits.push(r[3]);
    if (r[4] !== 'normal') bits.push(r[4]);
    if (r[5] !== 'none') bits.push(r[5]);
    if (r[6] !== 'running') bits.push(r[6]);
    if (r[7] !== 'none') bits.push(r[7]);
    return bits.length ? bits.join(' ') : 'none';
  }).join(', ');
}
// Chrome computed `transition` drops `all`, `0s`, `ease`, and `normal`
// when anything else remains. All-initial is "all".
function _cssTransitionShorthand(prop, dur, ease, delay, behavior) {
  const rows = _cssLayers([prop, dur, ease, delay, behavior]);
  if (!rows) return '';
  return rows.map((r) => {
    const bits = [];
    const delaySet = r[3] !== '0s';
    if (r[0] !== 'all') bits.push(r[0]);
    if (r[1] !== '0s' || delaySet) bits.push(r[1]);
    if (r[2] !== 'ease') bits.push(r[2]);
    if (delaySet) bits.push(r[3]);
    if (r[4] !== 'normal') bits.push(r[4]);
    return bits.length ? bits.join(' ') : 'all';
  }).join(', ');
}
// Physical four-value collapse (margin, inset, scroll-margin): one value
// when all match, two when top==bottom and left==right, three when only
// the inline sides match.
function _cssCollapse4(t, r, b, l) {
  if (!t || !r || !b || !l) return '';
  if (t === r && r === b && b === l) return t;
  if (t === b && r === l) return t + ' ' + r;
  if (r === l) return t + ' ' + r + ' ' + b;
  return t + ' ' + r + ' ' + b + ' ' + l;
}
function _cssPair(a, b) {
  if (!a || !b) return '';
  return a === b ? a : a + ' ' + b;
}
function _cssExpand4(raw) {
  const p = String(raw).trim().split(/\s+/).filter(Boolean);
  if (!p.length || p.length > 4) return null;
  const t = p[0], r = p[1] || t, b = p[2] || t, l = p[3] || r;
  return [t, r, b, l];
}
// One border edge in Chrome's computed order: width, style, color.
function _columnsString(width, count) {
  const w = !width || width === 'auto';
  const n = !count || count === 'auto';
  if (w && n) return 'auto';
  if (w) return count;
  if (n) return width;
  return width + ' ' + count;
}
function _parseColumns(raw) {
  let width = '', count = '';
  for (const t of String(raw).trim().split(/\s+/)) {
    if (!t || t.toLowerCase() === 'auto') continue;
    if (/^\d+(?:\.\d+)?$/.test(t)) count = t;
    else width = t;
  }
  return { width: width || 'auto', count: count || 'auto' };
}
function _fontSynthesisBits(weight, style, small) {
  const bits = [];
  if (weight !== 'none') bits.push('weight');
  if (style !== 'none') bits.push('style');
  if (small !== 'none') bits.push('small-caps');
  return bits.length ? bits.join(' ') : 'none';
}
function _emphasisStyle(style) {
  if (!style) return '';
  return style.toLowerCase().startsWith('filled ') ? style.slice(7) : style;
}
// text-decoration shorthand pieces. Line order is underline, overline,
// line-through. `none` clears lines. Color stays empty when absent so the
// caller can tell currentcolor from an explicit color.
function _parseTextDecoration(raw) {
  const lineKw = { underline: 1, overline: 1, 'line-through': 1, blink: 1, none: 1 };
  const styleKw = { solid: 1, double: 1, dotted: 1, dashed: 1, wavy: 1 };
  const order = ['underline', 'overline', 'line-through', 'blink'];
  const lines = [];
  let style = '', color = '', thick = '', none = false;
  for (const p of String(raw).trim().split(/\s+/)) {
    if (!p) continue;
    const low = p.toLowerCase();
    if (lineKw[low]) {
      if (low === 'none') { none = true; lines.length = 0; }
      else if (!lines.includes(low)) lines.push(low);
    } else if (styleKw[low]) style = low;
    else if (low === 'auto' || low === 'from-font' || /^(?:0|-?[\d.]+)(?:px|em|rem|%)$/.test(low)) {
      thick = low === '0' ? '0px' : p;
    } else color = _cssComputedColor(p) || p;
  }
  const line = none && !lines.length ? 'none' : order.filter((k) => lines.includes(k)).join(' ');
  return { line, style, color, thick };
}
function _parseBorderEdge(raw) {
  const styles = { none: 1, hidden: 1, dotted: 1, dashed: 1, solid: 1, double: 1, groove: 1, ridge: 1, inset: 1, outset: 1 };
  const widths = { thin: '1px', medium: '3px', thick: '5px' };
  let width = '', style = '', color = '';
  for (const p of String(raw).trim().split(/\s+/)) {
    const low = p.toLowerCase();
    if (styles[low]) style = low;
    else if (widths[low]) width = widths[low];
    else if (/^(?:0|-?[\d.]+)(?:px|em|rem|%)$/.test(low)) width = low === '0' ? '0px' : p;
    else color = _cssComputedColor(p) || p;
  }
  return { width, style, color };
}
globalThis.getComputedStyle = (el, pseudoElt) => {
  const pseudoName = _normalizePseudoElt(pseudoElt);
  if (!el) el = document.body || {};
  const style = el?.style || el?._style || _newCssDecl();
  const cacheable = (typeof el === 'object' && el !== null) || typeof el === 'function';
  let snapshot;
  if (pseudoName) {
    let perPseudo = cacheable ? _pseudoStyleSnapshotCache.get(el) : null;
    if (!perPseudo) {
      perPseudo = new Map();
      if (cacheable) _pseudoStyleSnapshotCache.set(el, perPseudo);
    }
    snapshot = perPseudo.get(pseudoName);
    if (!snapshot) {
      snapshot = { rendered: null, epoch: -1 };
      perPseudo.set(pseudoName, snapshot);
    }
  } else {
    snapshot = cacheable ? _computedStyleSnapshotCache.get(el) : null;
    if (!snapshot) {
      snapshot = { rendered: null, epoch: -1 };
      if (cacheable) _computedStyleSnapshotCache.set(el, snapshot);
    }
  }
  // The cascade snapshot from the layout run (whole table per call,
  // upstream's op shape): stylesheet rules + the folded inline style
  // attribute. Absent without a layout run (default build) or for
  // properties outside the engine's table — the fallbacks below keep
  // serving those.
  const refreshRendered = () => {
    if (snapshot.epoch === _ditingEpochNow()) return;
    snapshot.epoch = _ditingEpochNow();
    snapshot.rendered = null;
    if (el?._nid != null) {
      try {
        const raw = _domRaw("computed_style", String(el._nid | 0), pseudoName || "");
        if (raw && raw !== 'null') snapshot.rendered = JSON.parse(raw);
      } catch (e) {}
    }
  };
  // React virtualization libraries (react-window, tanstack-virtual,
  // react-virtuoso) all compute container dimensions via getComputedStyle.
  // The defaults table previously returned `auto` for width/height and
  // `'static'` for position, which made every list render 0 items. Pulling
  // width/height from the synthesized bounding rect makes those libraries
  // actually render content.
  const dimensionFor = (name) => {
    try {
      const r = el.getBoundingClientRect && el.getBoundingClientRect();
      if (!r) return null;
      switch (name) {
        case 'width': case 'inline-size':
          return r.width != null ? `${r.width}px` : null;
        case 'height': case 'block-size':
          return r.height != null ? `${r.height}px` : null;
        case 'left': return r.left != null ? `${r.left}px` : null;
        case 'top': return r.top != null ? `${r.top}px` : null;
        case 'right': return r.right != null ? `${r.right}px` : null;
        case 'bottom': return r.bottom != null ? `${r.bottom}px` : null;
        case 'client-width': case 'offset-width':
          return r.width != null ? `${r.width}px` : null;
        case 'client-height': case 'offset-height':
          return r.height != null ? `${r.height}px` : null;
      }
    } catch (e) {}
    return null;
  };

  const defaultsKebab = {
    display: 'block', visibility: 'visible', opacity: '1',
    position: 'static', overflow: 'visible',
    transform: 'none', 'transform-origin': '50% 50% 0px',
    transition: 'none', animation: 'none',
    float: 'none', clear: 'none',
    margin: '0px', padding: '0px',
    'margin-top': '0px', 'margin-right': '0px', 'margin-bottom': '0px', 'margin-left': '0px',
    'padding-top': '0px', 'padding-right': '0px', 'padding-bottom': '0px', 'padding-left': '0px',
    'font-size': '16px', 'line-height': 'normal', 'font-weight': '400',
    'font-family': 'Times',
    color: 'rgb(0, 0, 0)', 'background-color': 'rgba(0, 0, 0, 0)',
    'border-width': '0px', 'border-style': 'none', 'border-color': 'rgb(0, 0, 0)',
    'border-top-width': '0px', 'border-right-width': '0px',
    'border-bottom-width': '0px', 'border-left-width': '0px',
    'border-radius': '0px',
    'border-top-left-radius': '0px', 'border-top-right-radius': '0px',
    'border-bottom-right-radius': '0px', 'border-bottom-left-radius': '0px',
    'max-width': 'none', 'max-height': 'none',
    rotate: 'none', scale: 'none', translate: 'none', 'transform-box': 'view-box',
    'font-optical-sizing': 'auto', 'font-size-adjust': 'none',
    'font-synthesis-weight': 'auto', 'font-synthesis-style': 'auto', 'font-synthesis-small-caps': 'auto',
    'font-variant-ligatures': 'normal',
    'overflow-anchor': 'auto', 'overscroll-behavior-x': 'auto', 'overscroll-behavior-y': 'auto',
    'scroll-snap-type': 'none', 'scroll-snap-align': 'none',
    'text-underline-position': 'auto', 'text-orientation': 'mixed', 'text-wrap-mode': 'wrap',
    'white-space-collapse': 'collapse',
    'mask-clip': 'border-box', 'mask-origin': 'border-box', 'mask-repeat': 'repeat',
    'mask-mode': 'match-source', 'mask-composite': 'add',
    'offset-path': 'none', 'offset-distance': '0px', 'offset-rotate': 'auto 0deg',
    'column-fill': 'balance', 'column-rule-width': '3px',
    'border-block-start-style': 'none', 'border-block-end-style': 'none',
    'border-inline-start-style': 'none', 'border-inline-end-style': 'none',
    'border-block-start-width': '0px', 'border-block-end-width': '0px',
    'border-inline-start-width': '0px', 'border-inline-end-width': '0px',
    'scroll-margin-top': '0px', 'scroll-margin-right': '0px',
    'scroll-margin-bottom': '0px', 'scroll-margin-left': '0px',
    'scroll-padding-top': 'auto', 'scroll-padding-right': 'auto',
    'scroll-padding-bottom': 'auto', 'scroll-padding-left': 'auto',
    'justify-self': 'auto',
    'grid-column-start': 'auto', 'grid-column-end': 'auto',
    'grid-row-start': 'auto', 'grid-row-end': 'auto', 'grid-template-areas': 'none',
    'font-variant-numeric': 'normal', 'font-variant-east-asian': 'normal',
    'font-variant-position': 'normal', 'font-variant-alternates': 'normal',
    'font-feature-settings': 'normal',
    'text-rendering': 'auto', 'text-size-adjust': 'auto',
    'image-orientation': 'from-image', 'content-visibility': 'visible',
    'overflow-clip-margin': '0px',
    'counter-increment': 'none', 'counter-reset': 'none', 'counter-set': 'none',
    fill: 'rgb(0, 0, 0)', 'fill-opacity': '1', 'fill-rule': 'nonzero',
    stroke: 'none', 'stroke-width': '1px', 'stroke-opacity': '1',
    'stroke-linecap': 'butt', 'stroke-linejoin': 'miter', 'stroke-miterlimit': '4',
    'stroke-dasharray': 'none', 'stroke-dashoffset': '0px',
    'paint-order': 'normal', 'vector-effect': 'none',
    'clip-rule': 'nonzero', 'text-anchor': 'start',
    '-webkit-text-stroke-width': '0px', '-webkit-text-security': 'none',
    '-webkit-font-smoothing': 'auto', '-webkit-user-drag': 'auto',
    '-webkit-box-align': 'stretch', '-webkit-box-pack': 'start',
    '-webkit-box-orient': 'horizontal', '-webkit-box-direction': 'normal',
    '-webkit-box-flex': '0', '-webkit-box-ordinal-group': '1',
    '-webkit-box-decoration-break': 'slice', '-webkit-line-break': 'auto',
    '-webkit-rtl-ordering': 'logical', '-webkit-user-modify': 'read-only',
    '-webkit-writing-mode': 'horizontal-tb', '-webkit-text-combine': 'none',
    '-webkit-ruby-position': 'before',
    '-webkit-border-horizontal-spacing': '0px', '-webkit-border-vertical-spacing': '0px',
    'animation-composition': 'replace', 'animation-timeline': 'auto',
    'animation-range-start': 'normal', 'animation-range-end': 'normal',
    'transition-behavior': 'normal',
    'scroll-timeline-name': 'none', 'scroll-timeline-axis': 'block',
    'view-timeline-name': 'none', 'view-timeline-axis': 'block', 'view-timeline-inset': 'auto',
    'view-transition-name': 'none', 'view-transition-group': 'normal',
    'marker-start': 'none', 'marker-mid': 'none', 'marker-end': 'none',
    d: 'none',
    'flood-color': 'rgb(0, 0, 0)', 'flood-opacity': '1',
    'stop-color': 'rgb(0, 0, 0)', 'stop-opacity': '1',
    'lighting-color': 'rgb(255, 255, 255)',
    'color-interpolation': 'srgb', 'dominant-baseline': 'auto',
    'alignment-baseline': 'auto', 'baseline-shift': '0px',
    'scrollbar-width': 'auto', 'scrollbar-gutter': 'auto',
    'math-style': 'normal', 'math-depth': '0', 'math-shift': 'normal',
    'field-sizing': 'fixed', 'forced-color-adjust': 'auto',
    clip: 'auto', 'box-decoration-break': 'slice',
    'contain-intrinsic-width': 'none', 'contain-intrinsic-height': 'none',
    'contain-intrinsic-size': 'none', 'contain-intrinsic-block-size': 'none',
    'contain-intrinsic-inline-size': 'none',
    'scrollbar-color': 'auto', 'print-color-adjust': 'economy',
    'ruby-position': 'over', 'ruby-align': 'space-around',
    'shape-margin': '0px', 'shape-image-threshold': '0', 'shape-rendering': 'auto',
    'position-anchor': 'normal', 'position-area': 'none',
    'position-try-fallbacks': 'none', 'position-try-order': 'normal', 'position-visibility': 'anchor-visible',
    'text-combine-upright': 'none', 'hyphenate-character': 'auto',
    'scroll-snap-stop': 'normal', overlay: 'none',
    'color-rendering': 'auto', 'color-interpolation-filters': 'linearrgb',
    'offset-anchor': 'auto',
    'inset-block-start': 'auto', 'inset-block-end': 'auto',
    'inset-inline-start': 'auto', 'inset-inline-end': 'auto',
    cx: '0px', cy: '0px', r: '0px', rx: 'auto', ry: 'auto', x: '0px', y: '0px',
    'caret-shape': 'auto', 'text-box-trim': 'none', 'interpolate-size': 'numeric-only',
    'font-palette': 'normal', 'font-language-override': 'normal',
    'mask-size': 'auto', 'mask-position': '0% 0%', 'mask-type': 'luminance',
    'border-start-start-radius': '0px', 'border-start-end-radius': '0px',
    'border-end-start-radius': '0px', 'border-end-end-radius': '0px',
    'margin-trim': 'none',
    'reading-flow': 'normal', 'reading-order': '0',
    'scroll-initial-target': 'none',
    'scroll-margin-block-start': '0px', 'scroll-margin-block-end': '0px',
    'scroll-margin-inline-start': '0px', 'scroll-margin-inline-end': '0px',
    'scroll-padding-block-start': 'auto', 'scroll-padding-block-end': 'auto',
    'scroll-padding-inline-start': 'auto', 'scroll-padding-inline-end': 'auto',
    '-webkit-locale': 'auto', '-webkit-text-orientation': 'vertical-right',
    interactivity: 'auto', 'interest-delay-start': 'normal', 'interest-delay-end': 'normal',
    'text-decoration-inset': '0px', 'text-spacing-trim': 'normal', 'text-autospace': 'no-autospace',
    'anchor-name': 'none', 'anchor-scope': 'none', 'app-region': 'none',
    'corner-top-left-shape': 'superellipse(1)', 'corner-top-right-shape': 'superellipse(1)',
    'corner-bottom-right-shape': 'superellipse(1)', 'corner-bottom-left-shape': 'superellipse(1)',
    'corner-start-start-shape': 'superellipse(1)', 'corner-start-end-shape': 'superellipse(1)',
    'corner-end-start-shape': 'superellipse(1)', 'corner-end-end-shape': 'superellipse(1)',
    'font-variant-emoji': 'normal', 'font-variation-settings': 'normal',
    'hyphenate-limit-chars': 'auto', 'initial-letter': 'normal',
    'max-block-size': 'none', 'max-inline-size': 'none',
    'min-block-size': '0px', 'min-inline-size': '0px',
    'object-view-box': 'none', 'offset-position': 'normal',
    'overflow-block': 'visible', 'overflow-inline': 'visible',
    'overscroll-behavior-block': 'auto', 'overscroll-behavior-inline': 'auto',
    'row-rule-style': 'none', 'row-rule-width': '3px',
    'scroll-target-group': 'none', speak: 'normal',
    'text-box-edge': 'auto', 'text-wrap-style': 'auto',
    'timeline-scope': 'none',
    'view-transition-class': 'none', 'view-transition-scope': 'none',
    '-webkit-box-reflect': 'none', '-webkit-text-decorations-in-effect': 'none',
    '-webkit-mask-position-x': '0%', '-webkit-mask-position-y': '0%',
    '-webkit-mask-box-image-source': 'none', '-webkit-mask-box-image-slice': '0 fill',
    '-webkit-mask-box-image-width': 'auto', '-webkit-mask-box-image-outset': '0',
    '-webkit-mask-box-image-repeat': 'stretch',
    'baseline-source': 'auto', 'caret-animation': 'auto',
    'column-wrap': 'auto', 'column-rule-break': 'normal', 'row-rule-break': 'normal',
    'border-shape': 'none', 'dynamic-range-limit': 'no-limit',
    'ruby-overhang': 'auto', 'scroll-marker-group': 'none', 'text-fit': 'none',
    'timeline-trigger-name': 'none', 'trigger-scope': 'none',
    'animation-trigger': 'none', 'buffered-rendering': 'auto', 'column-height': 'auto',
    'column-rule-inset-cap-end': '0px', 'column-rule-inset-cap-start': '0px',
    'column-rule-inset-junction-end': '0px', 'column-rule-inset-junction-start': '0px',
    'column-rule-visibility-items': 'normal', 'flex-line-count': '1', 'frame-sizing': 'auto',
    'row-rule-inset-cap-end': '0px', 'row-rule-inset-cap-start': '0px',
    'row-rule-inset-junction-end': '0px', 'row-rule-inset-junction-start': '0px',
    'row-rule-visibility-items': 'normal', 'rule-overlap': 'row-over-column',
    'scroll-axis-lock': 'auto',
    'timeline-trigger-activation-range-end': 'normal',
    'timeline-trigger-activation-range-start': 'normal',
    'timeline-trigger-active-range-end': 'auto', 'timeline-trigger-active-range-start': 'auto',
    'timeline-trigger-source': 'auto', 'window-drag': 'none',
    '-webkit-border-image': 'none', '-webkit-mask-box-image': 'none',
    '-webkit-tap-highlight-color': 'rgba(0, 0, 0, 0.18)',
    'z-index': 'auto', 'pointer-events': 'auto',
    'box-sizing': 'content-box', cursor: 'auto',
    'white-space': 'normal', 'text-align': 'start', 'text-align-last': 'auto', 'text-transform': 'none',
    'vertical-align': 'baseline',
    'line-clamp': 'none', '-webkit-line-clamp': 'none',
    'accent-color': 'auto',
    'column-count': 'auto', 'column-gap': 'normal', 'row-gap': 'normal',
    'column-rule-style': 'none',
    'outline-style': 'none', 'outline-offset': '0px',
    'text-decoration-style': 'solid', 'text-decoration-thickness': 'auto',
    'text-emphasis-style': 'none', 'text-emphasis-position': 'over',
    'letter-spacing': 'normal', 'word-break': 'normal', 'overflow-wrap': 'normal',
    hyphens: 'manual', 'tab-size': '8', 'line-break': 'auto',
    'text-justify': 'auto', 'text-underline-offset': 'auto', 'text-decoration-skip-ink': 'auto',
    'list-style-image': 'none', quotes: 'auto', orphans: '2', widows: '2',
    'empty-cells': 'show',
    'background-attachment': 'scroll', 'background-origin': 'padding-box',
    'background-blend-mode': 'normal',
    'border-image-source': 'none', 'border-image-slice': '100%',
    'border-image-width': '1', 'border-image-outset': '0', 'border-image-repeat': 'stretch',
    'object-position': '50% 50%', 'image-rendering': 'auto',
    'grid-auto-flow': 'row', 'grid-auto-columns': 'auto', 'grid-auto-rows': 'auto',
    'align-content': 'normal', 'justify-items': 'normal',
    'place-content': 'normal', 'place-items': 'normal', 'place-self': 'auto',
    'column-width': 'auto', 'column-span': 'none',
    'break-before': 'auto', 'break-after': 'auto', 'break-inside': 'auto',
    'font-stretch': '100%', 'font-kerning': 'auto',
    'transform-style': 'flat', 'perspective-origin': '50% 50%',
    'scroll-behavior': 'auto', 'overscroll-behavior': 'auto', 'touch-action': 'auto',
    'shape-outside': 'none',
    'flex-direction': 'row', 'flex-wrap': 'nowrap', 'align-items': 'normal',
    'justify-content': 'normal', gap: 'normal',
    'grid-template-columns': 'none', 'grid-template-rows': 'none',
    'will-change': 'auto', 'backface-visibility': 'visible',
    // Chrome initial values for the computed properties scripts branch on
    // (obscura #771): an '' read is indistinguishable from "not set", so
    // feature probes silently took the wrong path. Values match Chromium 147.
    'background-image': 'none', 'background-size': 'auto',
    'background-position': '0% 0%', 'background-repeat': 'repeat',
    'font-style': 'normal', 'word-spacing': '0px',
    'text-decoration': 'none', 'text-decoration-line': 'none', 'text-indent': '0px',
    'list-style-position': 'outside', 'list-style-type': 'disc', 'border-spacing': '0px', 'caption-side': 'top',
    'align-self': 'auto', 'flex-grow': '0', 'flex-shrink': '1',
    'flex-basis': 'auto', 'grid-column': 'auto', 'grid-row': 'auto',
    'transition-property': 'all', 'transition-duration': '0s',
    'animation-name': 'none', 'animation-duration': '0s', 'animation-delay': '0s',
    'animation-direction': 'normal', 'animation-fill-mode': 'none', 'animation-play-state': 'running',
    'animation-iteration-count': '1', 'animation-timing-function': 'ease',
    'transition-delay': '0s', 'transition-timing-function': 'ease',
    'user-select': 'auto', 'box-shadow': 'none', 'text-shadow': 'none',
    'content': 'normal', 'order': '0', 'object-fit': 'fill',
    'resize': 'none', 'appearance': 'none', 'filter': 'none',
    'mix-blend-mode': 'normal', 'writing-mode': 'horizontal-tb', 'direction': 'ltr',
    'unicode-bidi': 'isolate', 'isolation': 'auto', 'contain': 'none',
    'aspect-ratio': 'auto', 'zoom': '1',
    'min-height': '0px', 'min-width': '0px',
    // outline-width resolves to the computed `medium` (3px) even while
    // outline-style is none — verified against local Chromium, not the
    // used value (0px) the spec's resolved-value rule suggests.
    'outline-width': '3px',
  };

  // A css-wide keyword is never a valid computed value — `color: initial`
  // must resolve downstream (snapshot → default), never echo verbatim (#218).
  const _CSS_WIDE_KW = /^\s*(inherit|initial|unset|revert|revert-layer)\s*$/i;

  const lookup = (rawProp) => {
    if (typeof rawProp !== 'string') return '';
    // Snapshot first: it already resolved the cascade, inline included.
    refreshRendered();
    // Custom property names are case-SENSITIVE and never camelCase — the
    // kebab+lowercase pass would corrupt --mainColor into --main-color.
    // Past the snapshot, the inline-style fallback below still applies
    // (el.style.setProperty('--x', …) is part of the cascade).
    let kebab = rawProp.startsWith('--')
      ? rawProp
      : rawProp.replace(/([A-Z])/g, '-$1').toLowerCase();
    // Camel vendor aliases carry their dash on the FRONT: webkitLineClamp
    // kebabs to -webkit-line-clamp, not webkit-line-clamp — without the
    // prefix dash the snapshot key never matches and every camel webkit*
    // read fell to the inline echo as '' (#23; kebab-channel reads like
    // getPropertyValue('-webkit-mask-image') always worked). No standard
    // property starts with "webkit-", so the prepend is unambiguous.
    if (!rawProp.startsWith('--') && kebab.startsWith('webkit-')) kebab = '-' + kebab;
    if (snapshot.rendered && Object.prototype.hasOwnProperty.call(snapshot.rendered, kebab)) {
      return snapshot.rendered[kebab];
    }
    // Box offsets outrank the inline echo: Chrome's resolved value for a
    // positioned element is the used value; an inline write the parser
    // dropped — style.left='NaNpx' (#129) — must never read back through.
    if (!pseudoName && (kebab === 'left' || kebab === 'top'
        || kebab === 'right' || kebab === 'bottom')) {
      const dim = dimensionFor(kebab);
      if (dim != null) return dim;
    }
    const used = pseudoName ? null : globalThis.__usedSize?.(el, kebab, lookup);
    if (used != null) return used;
    // Inline value next — CSSOM writes not yet folded into a snapshot
    // (or no layout run at all); pseudo-elements carry no inline style.
    if (!pseudoName) {
      const inlineVal = target.getPropertyValue ? target.getPropertyValue(rawProp) : '';
      if (inlineVal && !_CSS_WIDE_KW.test(inlineVal)) {
        // These shorthands are rebuilt from longhands below so colors
        // collapse to rgb() and unequal borders stay "".
        const rebuilt = kebab === 'margin' || kebab === 'padding' || kebab === 'border'
          || kebab === 'outline' || kebab === 'border-top' || kebab === 'border-right'
          || kebab === 'border-bottom' || kebab === 'border-left'
          || kebab === 'font' || kebab === 'flex' || kebab === 'flex-flow'
          || kebab === 'list-style' || kebab === 'background'
          || kebab === 'animation' || kebab === 'transition'
          || kebab === 'margin-block' || kebab === 'margin-inline'
          || kebab === 'padding-block' || kebab === 'padding-inline'
          || kebab === 'inset' || kebab === 'inset-block' || kebab === 'inset-inline'
          || kebab === 'border-block' || kebab === 'border-inline'
          || kebab === 'border-block-start' || kebab === 'border-block-end'
          || kebab === 'border-inline-start' || kebab === 'border-inline-end'
          || kebab === 'column-rule' || kebab === 'row-rule'
          || kebab === 'scroll-margin' || kebab === 'scroll-padding'
          || kebab === 'text-emphasis' || kebab === 'text-wrap' || kebab === 'columns'
          || kebab === 'font-synthesis' || kebab === '-webkit-text-stroke'
          || kebab === 'gap' || kebab === 'text-decoration';
        if (!rebuilt) {
          if (_isComputedColorProp(kebab)) {
            const painted = _cssComputedColor(inlineVal);
            if (painted) return painted;
          }
          const folded = _cssComputedKeyword(kebab, inlineVal);
          if (folded != null) return folded;
          // caret-color:auto is currentcolor. Fall through to the alias
          // below instead of echoing the keyword.
          const caretAuto = kebab === 'caret-color'
            && /^(auto|currentcolor)$/i.test(inlineVal.trim());
          if (!caretAuto) return inlineVal;
        }
      }
    }
    // A pseudo-element generates no box of its own: Chrome answers 'auto'
    // for width/height instead of the host's geometry — element-only.
    if (pseudoName) {
      if (kebab === 'width' || kebab === 'inline-size' || kebab === 'height' || kebab === 'block-size') return 'auto';
    } else {
      const dim = dimensionFor(kebab);
      if (dim != null) return dim;
    }
    // Initial value is currentcolor. Chrome's computed value is the used
    // color, so an unset read is rgb(0, 0, 0), not ''. An authored legacy
    // color collapses to rgb()/rgba(); currentcolor resolves the same way.
    if (kebab === '-webkit-text-fill-color' || kebab === '-webkit-text-stroke-color'
        || kebab === '-webkit-text-emphasis-color' || kebab === 'outline-color'
        || kebab === 'text-decoration-color' || kebab === 'text-emphasis-color'
        || kebab === 'column-rule-color'
        || kebab === 'border-block-start-color' || kebab === 'border-block-end-color'
        || kebab === 'border-inline-start-color' || kebab === 'border-inline-end-color'
        || kebab === 'row-rule-color' || kebab === 'caret-color') {
      if (!pseudoName && target.getPropertyValue) {
        const specified = target.getPropertyValue(kebab);
        const low = specified.trim().toLowerCase();
        const follows = !specified || _CSS_WIDE_KW.test(specified) || low === 'currentcolor'
          || (kebab === 'caret-color' && low === 'auto');
        if (!follows) return _cssComputedColor(specified) || specified;
      }
      return lookup('color');
    }
    // margin/padding shorthands are not in the cascade snapshot. Chrome
    // collapses equal sides: body is "8px", not the static "0px" fallback.
    if (kebab === 'margin' || kebab === 'padding') {
      const t = lookup(kebab + '-top');
      const r = lookup(kebab + '-right');
      const b = lookup(kebab + '-bottom');
      const l = lookup(kebab + '-left');
      if (t && r && b && l) {
        if (t === r && r === b && b === l) return t;
        if (t === b && r === l) return t + ' ' + r;
        if (r === l) return t + ' ' + r + ' ' + b;
        return t + ' ' + r + ' ' + b + ' ' + l;
      }
    }
    // border-top is "width style color". The border shorthand is that
    // string only when all four sides match; otherwise Chrome answers "".
    if (kebab === 'border-top' || kebab === 'border-right' || kebab === 'border-bottom' || kebab === 'border-left') {
      const side = kebab.slice(7);
      const w = lookup('border-' + side + '-width');
      const s = lookup('border-' + side + '-style');
      const c = lookup('border-' + side + '-color');
      if (w && s && c) return w + ' ' + s + ' ' + c;
    }
    if (kebab === 'border') {
      const one = (side) => {
        const w = lookup('border-' + side + '-width');
        const s = lookup('border-' + side + '-style');
        const c = lookup('border-' + side + '-color');
        return w && s && c ? w + ' ' + s + ' ' + c : '';
      };
      const t = one('top'), r = one('right'), b = one('bottom'), l = one('left');
      if (t && t === r && t === b && t === l) return t;
      return '';
    }
    if (kebab === 'font') {
      const style = lookup('font-style');
      const variant = lookup('font-variant');
      const weight = lookup('font-weight');
      const stretch = lookup('font-stretch');
      const size = lookup('font-size');
      const lh = lookup('line-height');
      const family = lookup('font-family');
      if (size && family) {
        const bits = [];
        if (style && style !== 'normal') bits.push(style);
        if (variant && variant !== 'normal') bits.push(variant);
        if (weight && weight !== '400' && weight !== 'normal') bits.push(weight);
        if (stretch && stretch !== '100%' && stretch !== 'normal') bits.push(stretch);
        bits.push(lh && lh !== 'normal' ? size + ' / ' + lh : size);
        bits.push(family);
        return bits.join(' ');
      }
    }
    if (kebab === 'flex') {
      const g = lookup('flex-grow'), s = lookup('flex-shrink'), b = lookup('flex-basis');
      if (g && s && b) return g + ' ' + s + ' ' + b;
    }
    if (kebab === 'flex-flow') {
      const d = lookup('flex-direction'), w = lookup('flex-wrap');
      if (d && w) return d + ' ' + w;
    }
    if (kebab === 'list-style') {
      // Chrome's computed order is position, image, type: "outside none disc".
      const t = lookup('list-style-type'), p = lookup('list-style-position'), i = lookup('list-style-image');
      if (t && p && i) return p + ' ' + i + ' ' + t;
    }
    if (kebab === 'background') {
      const c = lookup('background-color'), img = lookup('background-image');
      const rep = lookup('background-repeat'), att = lookup('background-attachment');
      const pos = lookup('background-position'), size = lookup('background-size');
      const origin = lookup('background-origin'), clip = lookup('background-clip');
      if (c && img && rep && att && pos && size && origin && clip) {
        return c + ' ' + img + ' ' + rep + ' ' + att + ' ' + pos + ' / ' + size + ' ' + origin + ' ' + clip;
      }
    }
    if (kebab === 'animation') {
      const dur = lookup('animation-duration'), ease = lookup('animation-timing-function');
      const delay = lookup('animation-delay'), iter = lookup('animation-iteration-count');
      const dir = lookup('animation-direction'), fill = lookup('animation-fill-mode');
      const play = lookup('animation-play-state'), name = lookup('animation-name');
      if (dur && ease && delay && iter && dir && fill && play && name) {
        return _cssAnimationShorthand(dur, ease, delay, iter, dir, fill, play, name);
      }
    }
    if (kebab === 'transition') {
      const prop = lookup('transition-property'), dur = lookup('transition-duration');
      const ease = lookup('transition-timing-function'), delay = lookup('transition-delay');
      const behavior = lookup('transition-behavior');
      if (prop && dur && ease && delay && behavior) {
        return _cssTransitionShorthand(prop, dur, ease, delay, behavior);
      }
    }
    if (kebab === 'margin-block' || kebab === 'margin-inline'
        || kebab === 'padding-block' || kebab === 'padding-inline') {
      const axis = kebab.endsWith('block') ? 'block' : 'inline';
      const kind = kebab.startsWith('margin') ? 'margin' : 'padding';
      return _cssPair(lookup(kind + '-' + axis + '-start'), lookup(kind + '-' + axis + '-end'));
    }
    if (kebab === 'inset') {
      return _cssCollapse4(lookup('top'), lookup('right'), lookup('bottom'), lookup('left'));
    }
    if (kebab === 'inset-block') return _cssPair(lookup('top'), lookup('bottom'));
    if (kebab === 'inset-inline') return _cssPair(lookup('left'), lookup('right'));
    if (kebab === 'border-block-start' || kebab === 'border-block-end'
        || kebab === 'border-inline-start' || kebab === 'border-inline-end') {
      const readEdge = (name) => {
        const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(name) : '';
        if (!specified || _CSS_WIDE_KW.test(specified)) return '';
        const edge = _parseBorderEdge(specified);
        return edge.width && edge.style && edge.color ? edge.width + ' ' + edge.style + ' ' + edge.color : '';
      };
      const own = readEdge(kebab);
      if (own) return own;
      // border-block sets both block edges; the longhands are not stored.
      const shared = readEdge(kebab.includes('block') ? 'border-block' : 'border-inline');
      if (shared) return shared;
      const w = lookup(kebab + '-width'), s = lookup(kebab + '-style'), c = lookup(kebab + '-color');
      if (w && s && c) return w + ' ' + s + ' ' + c;
    }
    if (kebab === 'border-block' || kebab === 'border-inline') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const edge = _parseBorderEdge(specified);
        if (edge.width && edge.style && edge.color) return edge.width + ' ' + edge.style + ' ' + edge.color;
      }
      const axis = kebab.endsWith('block') ? 'block' : 'inline';
      const a = lookup('border-' + axis + '-start');
      const b = lookup('border-' + axis + '-end');
      if (a && a === b) return a;
      return '';
    }
    if (kebab === 'column-rule' || kebab === 'row-rule') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      let width = '', style = '', color = '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const edge = _parseBorderEdge(specified);
        width = edge.width; style = edge.style; color = edge.color;
      }
      if (!width) width = lookup(kebab + '-width');
      if (!style) style = lookup(kebab + '-style');
      if (!color) color = lookup(kebab + '-color');
      if (width && color) return style && style !== 'none' ? width + ' ' + style + ' ' + color : width + ' ' + color;
    }
    if (kebab === 'scroll-margin' || kebab === 'scroll-padding') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const sides = _cssExpand4(specified);
        if (sides) return _cssCollapse4(sides[0], sides[1], sides[2], sides[3]);
      }
      return _cssCollapse4(
        lookup(kebab + '-top'), lookup(kebab + '-right'),
        lookup(kebab + '-bottom'), lookup(kebab + '-left')
      );
    }
    if (kebab === 'text-emphasis') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      let style = '', color = '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const shapes = { none: 1, dot: 1, circle: 1, 'double-circle': 1, triangle: 1, sesame: 1 };
        const fills = { filled: 1, open: 1 };
        const parts = [];
        for (const p of specified.trim().split(/\s+/)) {
          const low = p.toLowerCase();
          if (shapes[low] || fills[low]) parts.push(low);
          else color = _cssComputedColor(p) || p;
        }
        style = _emphasisStyle(parts.join(' '));
      }
      if (!style) style = _emphasisStyle(lookup('text-emphasis-style'));
      if (!color) color = lookup('text-emphasis-color');
      if (style && color) return style + ' ' + color;
    }
    if (kebab === 'text-wrap') {
      const modes = { wrap: 1, nowrap: 1 };
      const styles = { auto: 1, balance: 1, pretty: 1, stable: 1 };
      let mode = '', tws = '';
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        for (const p of specified.trim().toLowerCase().split(/\s+/)) {
          if (modes[p]) mode = p;
          else if (styles[p]) tws = p;
        }
      }
      if (!mode) mode = lookup('text-wrap-mode');
      if (!tws) tws = lookup('text-wrap-style');
      if (tws && tws !== 'auto') return tws;
      if (mode) return mode;
    }
    if (kebab === 'columns') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const parsed = _parseColumns(specified);
        return _columnsString(parsed.width, parsed.count);
      }
      return _columnsString(lookup('column-width'), lookup('column-count'));
    }
    if (kebab === 'font-synthesis') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const low = specified.trim().toLowerCase();
        if (low === 'none') return 'none';
        const on = new Set(low.split(/\s+/));
        return _fontSynthesisBits(on.has('weight') ? 'auto' : 'none', on.has('style') ? 'auto' : 'none', on.has('small-caps') ? 'auto' : 'none');
      }
      return _fontSynthesisBits(lookup('font-synthesis-weight'), lookup('font-synthesis-style'), lookup('font-synthesis-small-caps'));
    }
    if (kebab === '-webkit-text-stroke') {
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue(kebab) : '';
      let width = '', color = '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const widths = { thin: '1px', medium: '3px', thick: '5px' };
        for (const p of specified.trim().split(/\s+/)) {
          const low = p.toLowerCase();
          if (widths[low]) width = widths[low];
          else if (/^(?:0|-?[\d.]+)(?:px|em|rem|%)$/.test(low)) width = low === '0' ? '0px' : p;
          else color = _cssComputedColor(p) || p;
        }
      }
      if (!width) width = lookup('-webkit-text-stroke-width');
      if (!color) color = lookup('-webkit-text-stroke-color');
      if (width && color) return width + ' ' + color;
    }
    if (kebab === 'offset') {
      const path = lookup('offset-path'), dist = lookup('offset-distance'), rot = lookup('offset-rotate');
      if (path && dist && rot) return path + ' ' + dist + ' ' + rot;
    }
    if (kebab === 'mask' || kebab === '-webkit-mask') {
      const img = lookup(kebab === 'mask' ? 'mask-image' : '-webkit-mask-image');
      if (!img || img === 'none') return 'none';
      return img;
    }
    if (kebab === 'text-decoration') {
      // solid, currentcolor, and auto thickness are omitted. An explicit
      // color is kept even when the line is none ("rgb(255, 0, 0)").
      // currentcolor follows the used color on the longhand but stays out
      // of the shorthand, including when `color` itself is not black.
      let style = '', color = '', thick = '';
      const specified = !pseudoName && target.getPropertyValue ? target.getPropertyValue('text-decoration') : '';
      if (specified && !_CSS_WIDE_KW.test(specified)) {
        const parsed = _parseTextDecoration(specified);
        style = parsed.style;
        color = parsed.color;
        thick = parsed.thick;
      }
      const readLong = (name) => {
        if (pseudoName || !target.getPropertyValue) return '';
        const v = target.getPropertyValue(name);
        return v && !_CSS_WIDE_KW.test(v) ? v : '';
      };
      if (!style) style = String(lookup('text-decoration-style') || '').toLowerCase();
      if (!color) {
        const snap = snapshot.rendered;
        if (snap && Object.prototype.hasOwnProperty.call(snap, 'text-decoration-color')) {
          color = snap['text-decoration-color'];
        } else {
          const c = readLong('text-decoration-color');
          if (c && c.toLowerCase() !== 'currentcolor') color = _cssComputedColor(c) || c;
        }
      }
      if (!thick) thick = lookup('text-decoration-thickness') || '';
      const line = lookup('text-decoration-line');
      const bits = [];
      if (line && line !== 'none') bits.push(line);
      if (style && style !== 'solid') bits.push(style);
      if (color) bits.push(color);
      if (thick && thick.toLowerCase() !== 'auto') bits.push(thick === '0' ? '0px' : thick);
      return bits.length ? bits.join(' ') : 'none';
    }
    if (kebab === 'gap') {
      const row = lookup('row-gap'), col = lookup('column-gap');
      if (row && col) return row === col ? row : row + ' ' + col;
    }
    if (kebab === 'grid') {
      const parts = ['grid-template-rows', 'grid-template-columns', 'grid-template-areas', 'grid-auto-flow', 'grid-auto-rows', 'grid-auto-columns'].map((n) => lookup(n));
      if (parts.every(Boolean)) return parts.join(' / ');
    }
    if (kebab === 'place-content') {
      const a = lookup('align-content'), j = lookup('justify-content');
      if (a && j) return a === j ? a : a + ' ' + j;
    }
    if (kebab === 'place-items') {
      const a = lookup('align-items'), j = lookup('justify-items');
      if (a && j) return a === j ? a : a + ' ' + j;
    }
    if (kebab === 'place-self') {
      const a = lookup('align-self'), j = lookup('justify-self');
      if (a && j) return a === j ? a : a + ' ' + j;
    }
    if (kebab === 'border-image') {
      const src = lookup('border-image-source');
      if (!src || src === 'none') return 'none';
    }
    if (kebab === 'marker') {
      const s = lookup('marker-start'), m = lookup('marker-mid'), e = lookup('marker-end');
      if (s && m && e) return s === m && m === e ? s : '';
    }
    if (kebab === 'outline') {
      // Cascade doesn't store outline longhands. An authored shorthand is
      // reordered to Chrome's "color style width".
      if (!pseudoName && target.getPropertyValue) {
        const specified = target.getPropertyValue('outline');
        if (specified && !_CSS_WIDE_KW.test(specified)) {
          const styles = { none: 1, hidden: 1, dotted: 1, dashed: 1, solid: 1, double: 1, groove: 1, ridge: 1, inset: 1, outset: 1 };
          const widths = { thin: '1px', medium: '3px', thick: '5px' };
          let width = '', style = '', color = '';
          for (const p of specified.trim().split(/\s+/)) {
            if (styles[p]) style = p;
            else if (widths[p]) width = widths[p];
            else if (/^(?:0|-?[\d.]+)(?:px|em|rem|%)$/.test(p)) width = p;
            else color = _cssComputedColor(p) || p;
          }
          if (!color) color = lookup('outline-color');
          if (!style) style = lookup('outline-style');
          if (!width) width = lookup('outline-width');
          if (color && style && width) return color + ' ' + style + ' ' + width;
        }
      }
      const c = lookup('outline-color');
      const s = lookup('outline-style');
      const w = lookup('outline-width');
      if (c && s && w) return c + ' ' + s + ' ' + w;
    }
    // Unset perspective-origin resolves against the border box. No box
    // (display:none) keeps the specified "50% 50%". An authored value
    // already returned from the inline echo above.
    if (kebab === 'perspective-origin' && el && el._nid != null) {
      try {
        const raw = _domRaw('local_geom', String(el._nid | 0), '');
        if (raw && raw !== 'null') {
          const p = JSON.parse(raw);
          if (Array.isArray(p) && p.length >= 4) {
            const fmt = (n) => {
              const r = Math.round(n * 100) / 100;
              return Math.abs(r - Math.round(r)) < 1e-6 ? String(Math.round(r)) : String(r);
            };
            return fmt(p[2] / 2) + 'px ' + fmt(p[3] / 2) + 'px';
          }
        }
      } catch (e) {}
    }
    if (defaultsKebab[rawProp]) return defaultsKebab[rawProp];
    if (defaultsKebab[kebab]) return defaultsKebab[kebab];
    return globalThis.__customPropInitial?.(kebab) ?? '';
  };

  const target = style;
  return new Proxy(style, {
    get(_, prop) {
      if (prop === Symbol.toPrimitive) return undefined;
      // The computed object's brand is CSSStyleDeclaration (Chrome answers
      // '[object CSSStyleDeclaration]' from Object.prototype.toString and
      // `cs.constructor === CSSStyleDeclaration`). A tagless proxy reads as
      // '[object Object]' with constructor Object — i.e. isPlainObject() —
      // which @alifd/next's getStyle treats as "no computed style" and
      // returns null for; place() then parseFloat(null)s it into
      // left/top: NaNpx (tmall publish SKU drawer, #120).
      if (prop === Symbol.toStringTag) return "CSSStyleDeclaration";
      if (prop === "constructor") return CSSStyleDeclaration;
      // Interface members first — `prop in target` would otherwise answer
      // with the inline style object's own (length: 0, cssText: '') values.
      if (prop === 'getPropertyValue') return (name) => lookup(name);
      if (prop === 'getPropertyPriority') return () => '';
      if (prop === 'item') return (i) => _computedKebab()[i] || '';
      if (prop === 'length') return _computedKebab().length;
      // Indexed access (cs[0], Array.from(cs)) — Chrome answers with the
      // kebab-case property name. Castle's cssKeys collector iterates exactly
      // this way; falling through to lookup() returned '' for every index.
      if (typeof prop === 'string' && prop !== '' && !isNaN(prop) && prop.indexOf('.') === -1) {
        const n = Number(prop);
        if (Number.isInteger(n) && n >= 0 && n < _computedKebab().length) return _computedKebab()[n];
      }
      if (prop === 'cssText') return '';
      if (prop === 'parentRule') return null;
      // CSS property names must resolve through lookup BEFORE the
      // `prop in target` branch: element.style's named-property surface
      // claims every CSS property, so that branch would short-circuit
      // computed reads to the (usually empty) INLINE value —
      // getComputedStyle(el).width answered '' even while
      // getBoundingClientRect().width reported real geometry.
      if (typeof prop === 'string') { const v = lookup(prop); if (v) return v; }
      if (prop in target) return target[prop];
      if (typeof prop === 'string') return lookup(prop);
      return undefined;
    },
    // Real Chrome enumerates ~470 camelCase property names on a computed
    // style (Object.keys / for-in). A featureless object reads as 0 keys,
    // which fingerprinters (Castle cssKeys) score as headless.
    // Chrome: the 480 indexed longhands (own, enumerable) followed by the 750 camelCase property names (the epub* ones are not
    // enumerable); the same name list as element.style (platform/cssnames.impl.js)
    ownKeys() {
      const k = _computedKebab(), out = [];
      for (let i = 0; i < k.length; i++) out.push(String(i));
      return out.concat(globalThis.__diting_cssNames.list);
    },
    getOwnPropertyDescriptor(_, prop) {
      if (typeof prop === 'string') {
        if (/^(0|[1-9]\d*)$/.test(prop)) {
          const k = _computedKebab();
          return +prop < k.length ? { value: k[+prop], writable: false, enumerable: true, configurable: true } : undefined;
        }
        if (globalThis.__diting_cssNames.set.has(prop)) {
          return { value: lookup(prop), writable: true, enumerable: !/^epub/.test(prop), configurable: true };
        }
      }
      return Object.getOwnPropertyDescriptor(target, prop);
    },
    has(_, prop) {
      if (typeof prop === 'string') {
        if (/^(0|[1-9]\d*)$/.test(prop)) return +prop < _computedKebab().length;
        if (globalThis.__diting_cssNames.set.has(prop)) return true;
      }
      return prop in target;
    },
  });
};
// Marked in bootstrap's big list before the split; that list runs before
// this file loads, so the entry moved here, after the install.
[
  getComputedStyle,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });
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
