// 刀67: VideoColorSpace behavior. The class is minted by the surface layer
// (no definition site in js/), and the stub_members fill only leaves
// constant getters — but Chrome's constructor stores the init dict (members
// default null, fullRange boolean-coerced, vcs_partial_null keeps explicit
// nulls), the getters read that store, and toJSON returns the dict in
// Chrome's fullRange→matrix→primaries→transfer order (probes
// k67-chrome3/4). Takeover runs before stub_members: fill-if-missing skips
// everything installed here, and the minted prototype object is kept so
// brands/chains stay intact (the 刀65 FeaturePolicy pattern); only the
// constructor function is swapped, with length pinned to Chrome's 0.
(() => {
  // NB: read the minted face through globalThis — the function declaration
  // below hoists and shadows the global name from the IIFE's first line, so
  // a bare `VideoColorSpace.prototype` here would capture the REPLACEMENT's
  // own auto-created prototype and orphan the minted (sweep-tagged) one.
  const minted = globalThis.VideoColorSpace;
  if (typeof minted !== 'function' || !minted.prototype) return;
  const proto = minted.prototype;
  const store = new WeakMap();
  function VideoColorSpace(init) {
    const d = { fullRange: null, matrix: null, primaries: null, transfer: null };
    if (init !== null && typeof init === 'object') {
      if (init.fullRange !== undefined && init.fullRange !== null) d.fullRange = Boolean(init.fullRange);
      if (init.matrix !== undefined && init.matrix !== null) d.matrix = init.matrix;
      if (init.primaries !== undefined && init.primaries !== null) d.primaries = init.primaries;
      if (init.transfer !== undefined && init.transfer !== null) d.transfer = init.transfer;
    }
    store.set(this, d);
    if (typeof __hideOwn === 'function') __hideOwn(this);
  }
  VideoColorSpace.prototype = proto;
  Object.defineProperty(VideoColorSpace, 'length', { value: 0 });
  Object.defineProperty(VideoColorSpace, 'name', { value: 'VideoColorSpace' });
  function guard() { if (!store.has(this)) throw new TypeError('Illegal invocation'); }
  for (const k of ['primaries', 'transfer', 'matrix', 'fullRange']) {
    const f = function () { guard.call(this); return store.get(this)[k]; };
    Object.defineProperty(f, 'name', { value: 'get ' + k });
    Object.defineProperty(proto, k, { get: f, enumerable: true, configurable: true });
  }
  const toJSON = function () {
    guard.call(this);
    const d = store.get(this);
    return { fullRange: d.fullRange, matrix: d.matrix, primaries: d.primaries, transfer: d.transfer };
  };
  Object.defineProperty(toJSON, 'name', { value: 'toJSON' });
  Object.defineProperty(proto, 'toJSON', { value: toJSON, writable: true, enumerable: true, configurable: true });
  Object.defineProperty(globalThis, 'VideoColorSpace', { value: VideoColorSpace, writable: true, configurable: true });
})();
