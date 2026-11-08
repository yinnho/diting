(function () { // @@SCOPE
// god-file split (刀28, CSS property-name tables): the camel/kebab case
// codecs + the standard camelCase property list (Chrome 145 truth) + the
// Set form + the hidden __diting_cssNames install. The CSSStyleDeclaration
// class (still in bootstrap) and computed_style.js read these at runtime;
// platform_bundle.js bare-reads __diting_cssNames at ITS load — this file
// must stay before it in the build order (it sits in the #20s, platform
// bundle ~#50).
// Custom property names are case-sensitive and carry no camelCase IDL form
// (--mainColor must round-trip verbatim through style.setProperty/getPropertyValue).
const _cssCamelToKebab = (s) => {
  const str = String(s);
  return str.startsWith('--') ? str : str.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
};
const _cssKebabToCamel = (s) => String(s).replace(/-([a-z])/g, (_, c) => c.toUpperCase());

// Standard CSS property names (camelCase). Real CSSStyleDeclaration exposes
// every property as an enumerable accessor, so feature-detection code
// (`'gap' in el.style`) and enumeration (`Object.keys(el.style)`) see the whole
// set, not just the ones that happen to be assigned.
const _CSS_PROPERTY_NAMES = [
  "accentColor","alignContent","alignItems","alignSelf","all","animation","animationDelay",
  "animationDirection","animationDuration","animationFillMode","animationIterationCount",
  "animationName","animationPlayState","animationTimingFunction","appearance","aspectRatio",
  "backdropFilter","backfaceVisibility","background","backgroundAttachment","backgroundBlendMode",
  "backgroundClip","backgroundColor","backgroundImage","backgroundOrigin","backgroundPosition",
  "backgroundPositionX","backgroundPositionY","backgroundRepeat","backgroundSize","blockSize",
  "border","borderBlock","borderBlockColor","borderBlockEnd","borderBlockEndColor","borderBlockEndStyle",
  "borderBlockEndWidth","borderBlockStart","borderBlockStartColor","borderBlockStartStyle",
  "borderBlockStartWidth","borderBlockStyle","borderBlockWidth","borderBottom","borderBottomColor",
  "borderBottomLeftRadius","borderBottomRightRadius","borderBottomStyle","borderBottomWidth",
  "borderCollapse","borderColor","borderImage","borderImageOutset","borderImageRepeat",
  "borderImageSlice","borderImageSource","borderImageWidth","borderInline","borderInlineColor",
  "borderInlineEnd","borderInlineEndColor","borderInlineEndStyle","borderInlineEndWidth",
  "borderInlineStart","borderInlineStartColor","borderInlineStartStyle","borderInlineStartWidth",
  "borderInlineStyle","borderInlineWidth","borderLeft","borderLeftColor","borderLeftStyle",
  "borderLeftWidth","borderRadius","borderRight","borderRightColor","borderRightStyle",
  "borderRightWidth","borderSpacing","borderStyle","borderTop","borderTopColor","borderTopLeftRadius",
  "borderTopRightRadius","borderTopStyle","borderTopWidth","borderWidth","bottom","boxShadow",
  "boxSizing","breakAfter","breakBefore","breakInside","captionSide","caretColor","clear","clip",
  "clipPath","color","colorScheme","columnCount","columnFill","columnGap","columnRule","columnRuleColor",
  "columnRuleStyle","columnRuleWidth","columnSpan","columnWidth","columns","contain","container",
  "containerName","containerType","content","counterIncrement","counterReset","counterSet",
  "cursor","direction","display","emptyCells","filter","flex","flexBasis","flexDirection","flexFlow",
  "flexGrow","flexShrink","flexWrap","float","font","fontFamily","fontFeatureSettings","fontKerning",
  "fontOpticalSizing","fontSize","fontSizeAdjust","fontStretch","fontStyle","fontVariant",
  "fontVariantCaps","fontVariantLigatures","fontVariantNumeric","fontWeight","gap","grid","gridArea",
  "gridAutoColumns","gridAutoFlow","gridAutoRows","gridColumn","gridColumnEnd","gridColumnGap",
  "gridColumnStart","gridGap","gridRow","gridRowEnd","gridRowGap","gridRowStart","gridTemplate",
  "gridTemplateAreas","gridTemplateColumns","gridTemplateRows","height","hyphens","imageRendering",
  "inlineSize","inset","insetBlock","insetBlockEnd","insetBlockStart","insetInline","insetInlineEnd",
  "insetInlineStart","isolation","justifyContent","justifyItems","justifySelf","left","letterSpacing",
  "lineBreak","lineHeight","listStyle","listStyleImage","listStylePosition","listStyleType","margin",
  "marginBlock","marginBlockEnd","marginBlockStart","marginBottom","marginInline","marginInlineEnd",
  "marginInlineStart","marginLeft","marginRight","marginTop","mask","maxBlockSize","maxHeight",
  "maxInlineSize","maxWidth","minBlockSize","minHeight","minInlineSize","minWidth","mixBlendMode",
  "objectFit","objectPosition","offset","opacity","order","outline","outlineColor","outlineOffset",
  "outlineStyle","outlineWidth","overflow","overflowAnchor","overflowWrap","overflowX","overflowY",
  "overscrollBehavior","overscrollBehaviorBlock","overscrollBehaviorInline","overscrollBehaviorX",
  "overscrollBehaviorY","padding","paddingBlock","paddingBlockEnd","paddingBlockStart","paddingBottom",
  "paddingInline","paddingInlineEnd","paddingInlineStart","paddingLeft","paddingRight","paddingTop",
  "pageBreakAfter","pageBreakBefore","pageBreakInside","perspective","perspectiveOrigin","placeContent",
  "placeItems","placeSelf","pointerEvents","position","quotes","resize","right","rotate","rowGap",
  "scale","scrollBehavior","scrollMargin","scrollPadding","scrollSnapAlign","scrollSnapStop",
  "scrollSnapType","tabSize","tableLayout","textAlign","textAlignLast","textCombineUpright",
  "textDecoration","textDecorationColor","textDecorationLine","textDecorationSkipInk",
  "textDecorationStyle","textDecorationThickness","textEmphasis","textIndent","textJustify",
  "textOrientation","textOverflow","textRendering","textShadow","textTransform","textUnderlineOffset",
  "textUnderlinePosition","top","touchAction","transform","transformBox","transformOrigin",
  "transformStyle","transition","transitionDelay","transitionDuration","transitionProperty",
  "transitionTimingFunction","translate","unicodeBidi","userSelect","verticalAlign","visibility",
  "whiteSpace","width","willChange","wordBreak","wordSpacing","wordWrap","writingMode","zIndex","zoom",
];
const _CSS_PROP_SET = new Set(_CSS_PROPERTY_NAMES);
Object.defineProperty(globalThis, '__diting_cssNames', { value: { list: _CSS_PROPERTY_NAMES, set: _CSS_PROP_SET }, writable: true, enumerable: false, configurable: true });

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_CSS_PROPERTY_NAMES', _CSS_PROPERTY_NAMES);
  d('_CSS_PROP_SET', _CSS_PROP_SET);
  d('_cssCamelToKebab', _cssCamelToKebab);
  d('_cssKebabToCamel', _cssKebabToCamel);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
