(function () { // @@SCOPE
// god-file split (刀23, Screen / viewport family): the Screen face + hidden
// __scrStore + screenApply, VisualViewport + its factory + the installed
// visualViewport/devicePixelRatio, the __vp* viewport vars (bootstrap's
// persona refresh writes them through the generated a() accessors — live
// bindings, 刀8 law) + installViewportAccessors + the scroll/page offsets.
// Screen.prototype's onchange accessor + WebIDL reorder migrated from
// bootstrap's bottom try-cluster at the carve (member form).
// (#117) Chrome's screen carries ZERO own enumerable props — every field is
// a getter on Screen.prototype (probe ground truth: Object.keys(screen) ===
// []). The persona refresh writes the hidden store via
// __diting_screenApply, so `screen` (and its orientation object) keeps one
// identity for the context's whole life, like Chrome's.
globalThis.Screen = class Screen { constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); } };
const __scrStore = {
  width: 1920, height: 1080, availWidth: 1920, availHeight: 1040,
  colorDepth: 24, pixelDepth: 24, availTop: 0, availLeft: 0, isExtended: false,
  orientation: { type: "landscape-primary", angle: 0, addEventListener(){}, removeEventListener(){}, dispatchEvent(){ return true; } },
};
for (const _sk of Object.keys(__scrStore)) {
  const _k = _sk;
  Object.defineProperty(globalThis.Screen.prototype, _k, {
    configurable: true, enumerable: true,
    get() { return __scrStore[_k]; },
  });
}
globalThis.__def(globalThis.Screen.prototype, Symbol.toStringTag, 'Screen'); // member form: bare would add a load-time (file,name) pair
globalThis.screen = Object.create(globalThis.Screen.prototype);
globalThis.__diting_screenApply = function(sw, sh) {
  __scrStore.width = sw; __scrStore.height = sh;
  __scrStore.availWidth = sw; __scrStore.availHeight = sh - 40;
  __scrStore.availTop = 0; __scrStore.availLeft = 0;
};
// visualViewport is a real interface in Chrome — a detector walking
// Object.getPrototypeOf(visualViewport).constructor.name sees "VisualViewport",
// and width/height are prototype getters, not own enumerable data props.
globalThis.VisualViewport = class VisualViewport {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get offsetLeft() { return this._offsetLeft || 0; }
  get offsetTop() { return this._offsetTop || 0; }
  get pageLeft() { return globalThis.scrollX || 0; }
  get pageTop() { return globalThis.scrollY || 0; }
  get scale() { return this._scale || 1; }
  get width() { return this._width || 0; }
  get height() { return this._height || 0; }
  // no own trio (M4 knife 3): the prototype chains to EventTarget.prototype
  // (global_surface ET_CHAIN) and the native core carries the listeners —
  // the old empty stubs swallowed them.
};
globalThis.__diting_makeVisualViewport = function(w, h) {
  const vv = Object.create(VisualViewport.prototype);
  for (const [k, v] of Object.entries({ _width: w, _height: h, _offsetLeft: 0, _offsetTop: 0, _scale: 1 })) {
    Object.defineProperty(vv, k, { value: v, enumerable: false, configurable: true, writable: true });
  }
  return vv;
};
globalThis.visualViewport = globalThis.__diting_makeVisualViewport(1920, 1000);
globalThis.devicePixelRatio = 1;
// (#105) innerWidth/innerHeight/outer* are prototype accessors in Chrome —
// a page write is silently ignored. As plain writable data props a resize
// polyfill on douyin overwrote them with document extents, skewing every
// rect and click coordinate by a viewport. The snapshot cannot bake the
// accessors (#37 flattening), so defaults live in backing vars here and
// __diting_init installs the getters per context.
var __vpW = 1920, __vpH = 1000, __vpOW = 1920, __vpOH = 1080;
globalThis.__diting_installViewportAccessors = function() {
  const defs = {
    innerWidth: () => __vpW, innerHeight: () => __vpH,
    outerWidth: () => __vpOW, outerHeight: () => __vpOH,
  };
  for (const k in defs) {
    try {
      Object.defineProperty(globalThis, k, { get: defs[k], configurable: true, enumerable: true });
    } catch (e) {}
  }
};
globalThis.scrollX = 0; globalThis.scrollY = 0;
globalThis.pageXOffset = 0; globalThis.pageYOffset = 0;

// Screen.prototype layout (migrated from bootstrap's bottom try-cluster at
// the 刀23 carve): the onchange accessor + the reorder pass, constructor
// LAST. Member form — the class is an expression install, the name Screen
// is not a binding here.
try {
  if (globalThis.Screen && !Object.getOwnPropertyDescriptor(globalThis.Screen.prototype, 'onchange')) {
    Object.defineProperty(globalThis.Screen.prototype, 'onchange', {
      get() { return null; }, set() {}, configurable: true, enumerable: true,
    });
  }
  globalThis.__ditingReorderProto(globalThis.Screen.prototype, ['availWidth','availHeight','width','height','colorDepth','pixelDepth','availLeft','availTop','orientation','constructor','onchange','isExtended']);
} catch (e) {}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  a('__vpH', () => __vpH, (v) => { __vpH = v; });
  a('__vpOH', () => __vpOH, (v) => { __vpOH = v; });
  a('__vpOW', () => __vpOW, (v) => { __vpOW = v; });
  a('__vpW', () => __vpW, (v) => { __vpW = v; });
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
