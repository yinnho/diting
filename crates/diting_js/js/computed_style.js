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
    transform: 'none', 'transform-origin': '0px 0px',
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
    'offset-path': 'none', 'offset-distance': '0px', 'offset-rotate': 'auto',
    'column-fill': 'balance', 'column-rule-width': 'medium',
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
    'position-anchor': 'auto', 'position-area': 'none',
    'position-try-fallbacks': 'none', 'position-try-order': 'normal', 'position-visibility': 'always',
    'text-combine-upright': 'none', 'hyphenate-character': 'auto',
    'scroll-snap-stop': 'normal', overlay: 'none',
    'color-rendering': 'auto', 'color-interpolation-filters': 'linearrgb',
    'offset-anchor': 'auto',
    'inset-block-start': 'auto', 'inset-block-end': 'auto',
    'inset-inline-start': 'auto', 'inset-inline-end': 'auto',
    cx: '0px', cy: '0px', r: '0px', rx: 'auto', ry: 'auto', x: '0px', y: '0px',
    'caret-shape': 'auto', 'text-box-trim': 'none', 'interpolate-size': 'numeric-only',
    'font-palette': 'normal', 'font-language-override': 'normal',
    'mask-size': 'auto', 'mask-position': '50% 50%', 'mask-type': 'luminance',
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
    'z-index': 'auto', 'pointer-events': 'auto',
    'box-sizing': 'content-box', cursor: 'auto',
    'white-space': 'normal', 'text-align': 'start', 'text-align-last': 'auto', 'text-transform': 'none',
    'vertical-align': 'baseline',
    'line-clamp': 'none', '-webkit-line-clamp': 'none',
    'accent-color': 'auto', 'caret-color': 'auto',
    'column-count': 'auto', 'column-gap': 'normal', 'row-gap': 'normal',
    'column-rule-style': 'none',
    'outline-style': 'none', 'outline-offset': '0px',
    'text-decoration-style': 'solid', 'text-decoration-thickness': 'auto',
    'text-emphasis-style': 'none', 'text-emphasis-position': 'over right',
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
    'animation-name': 'none', 'animation-duration': '0s',
    'animation-iteration-count': '1', 'animation-timing-function': 'ease',
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
        if (_isComputedColorProp(kebab)) {
          const painted = _cssComputedColor(inlineVal);
          if (painted) return painted;
        }
        return inlineVal;
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
        || kebab === 'border-inline-start-color' || kebab === 'border-inline-end-color') {
      if (!pseudoName && target.getPropertyValue) {
        const specified = target.getPropertyValue(kebab);
        if (specified && !_CSS_WIDE_KW.test(specified) && specified.toLowerCase() !== 'currentcolor') {
          return _cssComputedColor(specified) || specified;
        }
      }
      return lookup('color');
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
