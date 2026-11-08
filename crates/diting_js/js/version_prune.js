// Version-coherence prune. chrome-target.toml pins Chrome 157 ("all layers the SAME real version").
// The global_surface tables were generated from a Chrome 155 capture, so they can carry names the
// claimed version lacks; this file subtracts those. Truth = Object.getOwnPropertyNames(globalThis)
// on local Chrome 157.0.8103.38 (1239 keys; tools/webplatform/chrome/winkeys-truth-157.json).
// 2026-11-06 re-truth: every name the 145-era GONE list pruned is back on the 157 window
// (XSLTProcessor included - the Chrome 142 removal was reverted), so the list flipped: 157 REMOVED
// the SharedStorage family that 155 still exposed. The remaining 157-vs-engine delta is four
// names the tables never carried (CanvasPaintEvent, ElementGeometryUpdateEvent, ElementImage,
// TrustedHTMLParserOptions) - engine capability gap tracked in issue #25, not prunable here.
// Loaded LAST.
(function () {
  const GONE = ['SharedStorage', 'SharedStorageAppendMethod', 'SharedStorageClearMethod',
    'SharedStorageDeleteMethod', 'SharedStorageModifierMethod', 'SharedStorageSetMethod',
    'SharedStorageWorklet', 'sharedStorage'];
  for (const n of GONE) { try { delete globalThis[n]; } catch (e) {} }
})();
