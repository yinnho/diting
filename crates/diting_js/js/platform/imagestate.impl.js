//! module: imagestate
//! layer: L2
//! exposes: -
//! requires-load: gen/htmlmedia, gen/htmlreflect, legacy:HTMLImageElement, legacy:__diting_imgState, legacy:__diting_markNative
//! requires-call: -

// `new Image()` emulation state (M4 knife 46). Chrome's <img> carries no own properties; the constructor used to shadow the prototype
// accessors with instance-level `complete/naturalWidth/naturalHeight/width/height/src`. The per-image state now lives in the
// bootstrap-owned WeakMap `__diting_imgState` and the prototype accessors below consult it first, falling back to the originals for
// every other <img>. Setting `src` on an Image()-made element additionally starts the load emulation (`state.load`).

const IMG = HTMLImageElement.prototype;
const state = __diting_imgState;
const mark = __diting_markNative;

function wrap(name, make) {
  const d = Object.getOwnPropertyDescriptor(IMG, name);
  if (!d || !d.get) return;
  const n = make(d);
  Object.defineProperty(IMG, name, {
    get: mark({ ['get ' + name]: n.get }['get ' + name]),
    set: n.set ? mark({ ['set ' + name]: n.set }['set ' + name]) : d.set,
    enumerable: d.enumerable, configurable: d.configurable,
  });
}

for (const k of ['complete', 'naturalWidth', 'naturalHeight']) {
  wrap(k, (d) => ({ get: function () { const q = state.get(this); return q ? q[k] : d.get.call(this); } }));
}
for (const [k, f, nat] of [['width', 'w', 'naturalWidth'], ['height', 'h', 'naturalHeight']]) {
  wrap(k, (d) => ({
    get: function () { const q = state.get(this); return q ? (q[f] !== null ? q[f] : (this[nat] || 0)) : d.get.call(this); },
    set: function (v) { const q = state.get(this); if (q) q[f] = v >>> 0; else d.set.call(this, v); },
  }));
}
wrap('src', (d) => ({
  get: function () { return d.get.call(this); },
  set: function (v) { d.set.call(this, v); const q = state.get(this); if (q && q.load) q.load(); },
}));
