// 刀56 (M4), retargeted to Chrome 157: startOffset / endOffset / collapsed
// stay on AbstractRange. startContainer / endContainer moved to NodeRange,
// which sits between Range/StaticRange and AbstractRange. Neither leaf
// lists any of the five. This lift runs AFTER global_surface (NodeRange
// exists and the chain is wired) and BEFORE stub_members (fill-if-missing
// sees the real getters and stays out). _rngSame is inlined because its
// original is a file-local lexical binding.
(function () {
  const AR = globalThis.AbstractRange;
  if (!AR || !AR.prototype) return;
  const mark = globalThis.__diting_markNative || ((f) => f);
  const defs = {
    startContainer: { get: mark(function startContainer() { return this._sc; }) },
    startOffset: { get: mark(function startOffset() { return this._so; }) },
    endContainer: { get: mark(function endContainer() { return this._ec; }) },
    endOffset: { get: mark(function endOffset() { return this._eo; }) },
    collapsed: { get: mark(function collapsed() { const a = this._sc, b = this._ec; return (a === b || (!!a && !!b && a._nid === b._nid)) && this._so === this._eo; }) },
  };
  const put = (proto, keys) => {
    if (!proto) return;
    for (const k of keys) Object.defineProperty(proto, k, { get: defs[k].get, enumerable: true, configurable: true });
  };
  put(AR.prototype, ['startOffset', 'endOffset', 'collapsed']);
  for (const k of ['startContainer', 'endContainer']) delete AR.prototype[k];
  const NR = globalThis.NodeRange;
  if (NR && NR.prototype) put(NR.prototype, ['startContainer', 'endContainer']);
  for (const C of [globalThis.Range, globalThis.StaticRange]) {
    if (!C || !C.prototype) continue;
    for (const k of Object.keys(defs)) delete C.prototype[k];
  }
})();
