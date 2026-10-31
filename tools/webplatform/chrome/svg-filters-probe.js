(function(){
const NS = 'http://www.w3.org/2000/svg';
const T = { SVGMaskElement: 'mask', SVGFilterElement: 'filter', SVGMPathElement: 'mpath', SVGScriptElement: 'script', SVGTextPathElement: 'textPath', SVGViewElement: 'view',
  SVGFEBlendElement: 'feBlend', SVGFEColorMatrixElement: 'feColorMatrix', SVGFEComponentTransferElement: 'feComponentTransfer', SVGFECompositeElement: 'feComposite',
  SVGFEConvolveMatrixElement: 'feConvolveMatrix', SVGFEDiffuseLightingElement: 'feDiffuseLighting', SVGFEDisplacementMapElement: 'feDisplacementMap', SVGFEDistantLightElement: 'feDistantLight',
  SVGFEFloodElement: 'feFlood', SVGFEFuncAElement: 'feFuncA', SVGFEGaussianBlurElement: 'feGaussianBlur', SVGFEImageElement: 'feImage', SVGFEMergeElement: 'feMerge', SVGFEMergeNodeElement: 'feMergeNode',
  SVGFEMorphologyElement: 'feMorphology', SVGFEOffsetElement: 'feOffset', SVGFEPointLightElement: 'fePointLight', SVGFESpecularLightingElement: 'feSpecularLighting', SVGFESpotLightElement: 'feSpotLight',
  SVGFETileElement: 'feTile', SVGFETurbulenceElement: 'feTurbulence' };
const out = {};
const describe = (v) => {
  if (v === null || v === undefined) return { t: String(v) };
  const tp = typeof v;
  if (tp !== 'object') return { t: tp, v: v };
  const cn = v.constructor && v.constructor.name; const r = { c: cn };
  if ('baseVal' in v) { const b = v.baseVal; const bt = typeof b; if (bt === 'object' && b !== null) { r.b = {}; if ('unitType' in b) { r.b = ['len', b.unitType, b.value]; } else if ('numberOfItems' in b) { r.b = ['list', b.numberOfItems]; } else if ('align' in b) { r.b = ['par', b.align, b.meetOrSlice]; } else r.b = ['obj']; } else r.b = ['val', b]; }
  return r;
};
for (const [iface, tag] of Object.entries(T)) {
  const el = document.createElementNS(NS, tag); const P = window[iface].prototype; const res = {};
  for (const k of Object.getOwnPropertyNames(P)) { if (k === 'constructor') continue; const d = Object.getOwnPropertyDescriptor(P, k); if (!d.get) { if (typeof d.value === 'number') res[k] = { const: d.value }; continue; } try { res[k] = describe(el[k]); } catch (e) { res[k] = { err: e.name }; } }
  out[iface] = res;
}
const ct = document.createElementNS(NS, 'feFuncR'); const CP = SVGComponentTransferFunctionElement.prototype; out.SVGComponentTransferFunctionElement = {};
for (const k of Object.getOwnPropertyNames(CP)) { if (k === 'constructor') continue; const d = Object.getOwnPropertyDescriptor(CP, k); if (!d.get) { if (typeof d.value === 'number') out.SVGComponentTransferFunctionElement[k] = { const: d.value }; continue; } try { out.SVGComponentTransferFunctionElement[k] = describe(ct[k]); } catch (e) { out.SVGComponentTransferFunctionElement[k] = { err: e.name }; } }
// attribute round trips for enums: keyword -> baseVal
const kw = {};
const rt = (iface, tag, member, attr, values) => { const el = document.createElementNS(NS, tag); kw[iface + '.' + member] = values.map(v => { el.setAttribute(attr, v); return [v, el[member].baseVal]; }); };
rt('SVGFEBlendElement', 'feBlend', 'mode', 'mode', ['normal', 'multiply', 'screen', 'darken', 'lighten', 'overlay', 'junk']);
rt('SVGFEColorMatrixElement', 'feColorMatrix', 'type', 'type', ['matrix', 'saturate', 'hueRotate', 'luminanceToAlpha', 'junk']);
rt('SVGFECompositeElement', 'feComposite', 'operator', 'operator', ['over', 'in', 'out', 'atop', 'xor', 'arithmetic', 'junk']);
rt('SVGFEConvolveMatrixElement', 'feConvolveMatrix', 'edgeMode', 'edgeMode', ['duplicate', 'wrap', 'none', 'junk']);
rt('SVGFEDisplacementMapElement', 'feDisplacementMap', 'xChannelSelector', 'xChannelSelector', ['R', 'G', 'B', 'A', 'junk']);
rt('SVGFEMorphologyElement', 'feMorphology', 'operator', 'operator', ['erode', 'dilate', 'junk']);
rt('SVGFETurbulenceElement', 'feTurbulence', 'type', 'type', ['fractalNoise', 'turbulence', 'junk']);
rt('SVGFETurbulenceElement', 'feTurbulence', 'stitchTiles', 'stitchTiles', ['stitch', 'noStitch', 'junk']);
rt('SVGTextPathElement', 'textPath', 'method', 'method', ['align', 'stretch', 'junk']);
rt('SVGTextPathElement', 'textPath', 'spacing', 'spacing', ['auto', 'exact', 'junk']);
rt('SVGMaskElement', 'mask', 'maskUnits', 'maskUnits', ['userSpaceOnUse', 'objectBoundingBox', 'junk']);
rt('SVGFilterElement', 'filter', 'filterUnits', 'filterUnits', ['userSpaceOnUse', 'objectBoundingBox', 'junk']);
rt('SVGFilterElement', 'filter', 'primitiveUnits', 'primitiveUnits', ['userSpaceOnUse', 'objectBoundingBox', 'junk']);
out.__kw = kw;
// consts on the classes
out.__consts = {};
for (const iface of Object.keys(T).concat(['SVGComponentTransferFunctionElement'])) { const C = window[iface]; const cs = {}; for (const k of Object.getOwnPropertyNames(C)) if (/^SVG_|^TEXTPATH_/.test(k)) cs[k] = C[k]; if (Object.keys(cs).length) out.__consts[iface] = cs; }
out.__script = (() => { const s = document.createElementNS(NS, 'script'); const r = [s.type, s.async]; s.type = 'text/x'; s.async = true; r.push(s.getAttribute('type'), s.hasAttribute('async'), s.async); return r; })();
out.__view = (() => { const v = document.createElementNS(NS, 'view'); const r = [v.zoomAndPan]; v.zoomAndPan = 1; r.push(v.getAttribute('zoomAndPan'), v.zoomAndPan); return r; })();
return JSON.stringify(out);
})()
