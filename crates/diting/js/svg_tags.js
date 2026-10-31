// 刀77: SVG tag -> interface wiring for the tags the engine still wrapped as a bare SVGElement (filters, mask,
// textPath, view, script, metadata, mpath, animateMotion/animateTransform/set, ...). Names captured from Chrome 145
// (probe k77-tags: createElementNS(svg, tag).constructor.name). The interface classes exist (minted + earlier knives'
// member faces); this only teaches the element factory which prototype each tag's wrapper wears.
(() => {
  const map = globalThis.__diting_svgTagMap;
  const SVGEl = globalThis.SVGElement;
  if (!map || typeof SVGEl !== 'function') return;
  const TAGS = {"animateMotion":"SVGAnimateMotionElement","animateTransform":"SVGAnimateTransformElement","feBlend":"SVGFEBlendElement","feColorMatrix":"SVGFEColorMatrixElement","feComponentTransfer":"SVGFEComponentTransferElement","feComposite":"SVGFECompositeElement","feConvolveMatrix":"SVGFEConvolveMatrixElement","feDiffuseLighting":"SVGFEDiffuseLightingElement","feDisplacementMap":"SVGFEDisplacementMapElement","feDistantLight":"SVGFEDistantLightElement","feFlood":"SVGFEFloodElement","feFuncA":"SVGFEFuncAElement","feFuncB":"SVGFEFuncBElement","feFuncG":"SVGFEFuncGElement","feFuncR":"SVGFEFuncRElement","feGaussianBlur":"SVGFEGaussianBlurElement","feImage":"SVGFEImageElement","feMerge":"SVGFEMergeElement","feMergeNode":"SVGFEMergeNodeElement","feMorphology":"SVGFEMorphologyElement","feOffset":"SVGFEOffsetElement","fePointLight":"SVGFEPointLightElement","feSpecularLighting":"SVGFESpecularLightingElement","feSpotLight":"SVGFESpotLightElement","feTile":"SVGFETileElement","feTurbulence":"SVGFETurbulenceElement","filter":"SVGFilterElement","mask":"SVGMaskElement","metadata":"SVGMetadataElement","mpath":"SVGMPathElement","script":"SVGScriptElement","set":"SVGSetElement","textPath":"SVGTextPathElement","view":"SVGViewElement"};
  for (const tag of Object.keys(TAGS)) {
    const C = globalThis[TAGS[tag]];
    if (typeof C !== 'function' || !C.prototype || map.has(tag) && map.get(tag).prototype === C.prototype) continue;
    // the wrapper factory is called as `new W(nodeId)`; it builds the element through SVGElement's node-id gate
    // but wearing the interface's own prototype, so constructor/instanceof/members are the interface's
    const W = function (nid) { return Reflect.construct(SVGEl, [nid], W); };
    W.prototype = C.prototype;
    map.set(tag, W);
  }
})();
