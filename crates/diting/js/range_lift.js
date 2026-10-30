// 刀56 (M4): the Range/StaticRange boundary getters move up to
// AbstractRange.prototype — Chrome owns startContainer / startOffset /
// endContainer / endOffset / collapsed THERE only; neither Range.prototype
// nor StaticRange.prototype lists any of the five. The engine's two classes
// each carried an identical own copy (both read the same _sc/_so/_ec/_eo
// slots), and AbstractRange — minted later by global_surface — sat above
// them with inert stubs. This lift runs AFTER global_surface (the chain
// wiring is in place) and BEFORE stub_members (whose fill-if-missing sees
// the real getters and stays out). Bodies are range_selection.js's,
// verbatim; _rngSame is inlined because its original is a file-local
// lexical binding, not a reachable name here.
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
  for (const k of Object.keys(defs)) {
    const d = defs[k];
    Object.defineProperty(AR.prototype, k, { get: d.get, enumerable: true, configurable: true });
  }
  for (const C of [globalThis.Range, globalThis.StaticRange]) {
    if (!C || !C.prototype) continue;
    for (const k of Object.keys(defs)) delete C.prototype[k];
  }
})();
