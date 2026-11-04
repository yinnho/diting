(function () { // @@SCOPE
// god-file split: bootstrap's real native marker, reached through its
// hidden prop (house style — see serializer.js) so no cross-file load
// edge appears in the dependency ratchet.
const _markNative = globalThis.__diting_markNative;

// Chrome's enumerable computed-style property set (kebab-case). Real
// Chrome 145 exposes ~470 of these on every getComputedStyle() result;
// count AND enumeration are fingerprint surfaces (Castle cssKeys).
// Chrome 145's getComputedStyle longhand list, in Chrome's order (413 names; shorthands are NOT items of a computed declaration).
const _COMPUTED_PROPS_KEBAB = ('accent-color align-content align-items align-self alignment-baseline anchor-name anchor-scope animation-composition ' +
  'animation-delay animation-direction animation-duration animation-fill-mode animation-iteration-count animation-name ' +
  'animation-play-state animation-range-end animation-range-start animation-timeline animation-timing-function app-region ' +
  'appearance backdrop-filter backface-visibility background-attachment background-blend-mode background-clip background-color ' +
  'background-image background-origin background-position background-repeat background-size baseline-shift baseline-source ' +
  'block-size border-block-end-color border-block-end-style border-block-end-width border-block-start-color ' +
  'border-block-start-style border-block-start-width border-bottom-color border-bottom-left-radius border-bottom-right-radius ' +
  'border-bottom-style border-bottom-width border-collapse border-end-end-radius border-end-start-radius border-image-outset ' +
  'border-image-repeat border-image-slice border-image-source border-image-width border-inline-end-color border-inline-end-style ' +
  'border-inline-end-width border-inline-start-color border-inline-start-style border-inline-start-width border-left-color ' +
  'border-left-style border-left-width border-right-color border-right-style border-right-width border-start-end-radius ' +
  'border-start-start-radius border-top-color border-top-left-radius border-top-right-radius border-top-style border-top-width ' +
  'bottom box-decoration-break box-shadow box-sizing break-after break-before break-inside buffered-rendering caption-side ' +
  'caret-animation caret-color caret-shape clear clip clip-path clip-rule color color-interpolation color-interpolation-filters ' +
  'color-rendering column-count column-gap column-height column-rule-color column-rule-style column-rule-width column-span ' +
  'column-width column-wrap contain-intrinsic-block-size contain-intrinsic-height contain-intrinsic-inline-size ' +
  'contain-intrinsic-size contain-intrinsic-width container-name container-type content corner-bottom-left-shape ' +
  'corner-bottom-right-shape corner-end-end-shape corner-end-start-shape corner-start-end-shape corner-start-start-shape ' +
  'corner-top-left-shape corner-top-right-shape cursor cx cy d direction display dominant-baseline dynamic-range-limit empty-cells ' +
  'field-sizing fill fill-opacity fill-rule filter flex-basis flex-direction flex-grow flex-shrink flex-wrap float flood-color ' +
  'flood-opacity font-family font-kerning font-language-override font-optical-sizing font-palette font-size font-size-adjust ' +
  'font-stretch font-style font-synthesis-small-caps font-synthesis-style font-synthesis-weight font-variant ' +
  'font-variant-alternates font-variant-caps font-variant-east-asian font-variant-emoji font-variant-ligatures ' +
  'font-variant-numeric font-variant-position font-weight grid-auto-columns grid-auto-flow grid-auto-rows grid-column-end ' +
  'grid-column-start grid-row-end grid-row-start grid-template-areas grid-template-columns grid-template-rows height ' +
  'hyphenate-character hyphenate-limit-chars hyphens image-orientation image-rendering initial-letter inline-size inset-block-end ' +
  'inset-block-start inset-inline-end inset-inline-start interactivity interest-delay-end interest-delay-start interpolate-size ' +
  'isolation justify-content justify-items justify-self left letter-spacing lighting-color line-break line-height list-style-image ' +
  'list-style-position list-style-type margin-block-end margin-block-start margin-bottom margin-inline-end margin-inline-start ' +
  'margin-left margin-right margin-top marker-end marker-mid marker-start mask-clip mask-composite mask-image mask-mode ' +
  'mask-origin mask-position mask-repeat mask-size mask-type math-depth math-shift math-style max-block-size max-height ' +
  'max-inline-size max-width min-block-size min-height min-inline-size min-width mix-blend-mode object-fit object-position ' +
  'object-view-box offset-anchor offset-distance offset-path offset-position offset-rotate opacity order orphans outline-color ' +
  'outline-offset outline-style outline-width overflow-anchor overflow-block overflow-clip-margin overflow-inline overflow-wrap ' +
  'overflow-x overflow-y overlay overscroll-behavior-block overscroll-behavior-inline padding-block-end padding-block-start ' +
  'padding-bottom padding-inline-end padding-inline-start padding-left padding-right padding-top paint-order perspective ' +
  'perspective-origin pointer-events position position-anchor position-area position-try-fallbacks position-try-order ' +
  'position-visibility print-color-adjust r reading-flow reading-order resize right rotate row-gap ruby-align ruby-position rx ry ' +
  'scale scroll-behavior scroll-initial-target scroll-margin-block-end scroll-margin-block-start scroll-margin-inline-end ' +
  'scroll-margin-inline-start scroll-marker-group scroll-padding-block-end scroll-padding-block-start scroll-padding-inline-end ' +
  'scroll-padding-inline-start scroll-target-group scroll-timeline-axis scroll-timeline-name scrollbar-color scrollbar-gutter ' +
  'scrollbar-width shape-image-threshold shape-margin shape-outside shape-rendering speak stop-color stop-opacity stroke ' +
  'stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit stroke-opacity stroke-width tab-size ' +
  'table-layout text-align text-align-last text-anchor text-autospace text-box-edge text-box-trim text-decoration ' +
  'text-decoration-color text-decoration-line text-decoration-skip-ink text-decoration-style text-emphasis-color ' +
  'text-emphasis-position text-emphasis-style text-indent text-justify text-overflow text-rendering text-shadow text-size-adjust ' +
  'text-spacing-trim text-transform text-underline-position text-wrap-mode text-wrap-style timeline-scope top touch-action ' +
  'transform transform-origin transform-style transition-behavior transition-delay transition-duration transition-property ' +
  'transition-timing-function translate unicode-bidi user-select vector-effect vertical-align view-timeline-axis ' +
  'view-timeline-inset view-timeline-name view-transition-class view-transition-group view-transition-name visibility ' +
  'white-space-collapse widows width will-change word-break word-spacing writing-mode x y z-index zoom ' +
  '-webkit-border-horizontal-spacing -webkit-border-image -webkit-border-vertical-spacing -webkit-box-align ' +
  '-webkit-box-decoration-break -webkit-box-direction -webkit-box-flex -webkit-box-ordinal-group -webkit-box-orient ' +
  '-webkit-box-pack -webkit-box-reflect -webkit-font-smoothing -webkit-line-break -webkit-line-clamp -webkit-locale ' +
  '-webkit-mask-box-image -webkit-mask-box-image-outset -webkit-mask-box-image-repeat -webkit-mask-box-image-slice ' +
  '-webkit-mask-box-image-source -webkit-mask-box-image-width -webkit-rtl-ordering -webkit-tap-highlight-color ' +
  '-webkit-text-combine -webkit-text-decorations-in-effect -webkit-text-fill-color -webkit-text-orientation -webkit-text-security ' +
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
    'z-index': 'auto', 'pointer-events': 'auto',
    'box-sizing': 'content-box', cursor: 'auto',
    'white-space': 'normal', 'text-align': 'start', 'text-align-last': 'auto', 'text-transform': 'none',
    'line-clamp': 'none', '-webkit-line-clamp': 'none',
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
    const kebab = rawProp.startsWith('--')
      ? rawProp
      : rawProp.replace(/([A-Z])/g, '-$1').toLowerCase();
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
      if (inlineVal && !_CSS_WIDE_KW.test(inlineVal)) return inlineVal;
    }
    // A pseudo-element generates no box of its own: Chrome answers 'auto'
    // for width/height instead of the host's geometry — element-only.
    if (pseudoName) {
      if (kebab === 'width' || kebab === 'inline-size' || kebab === 'height' || kebab === 'block-size') return 'auto';
    } else {
      const dim = dimensionFor(kebab);
      if (dim != null) return dim;
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
    // Chrome: the 413 indexed longhands (own, enumerable) followed by the 690 camelCase property names (the epub* ones are not
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
  d('_markNative', _markNative);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
