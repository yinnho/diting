// Version-coherence prune. chrome-target.toml pins Chrome 145 ("all layers the SAME real version"). The older window-key truth
// the global_surface tables were generated from (chrome/fp-chrome*.json) was captured from Chrome 155 (fp-chrome.json carries
// that UA), so it lists interfaces that 145 does not have. Chrome for Testing 145.0.7632.117 (the shape-audit truth) lacks every
// name below: a page enumerating Object.getOwnPropertyNames(window) must not see them. (XSLTProcessor: XSLT was removed from
// Chrome in 142.) FontFaceSet and CSSPseudoElement are also not exposed on the 145 window (document.fonts is a real FontFaceSet
// from platform/fontfaceset.impl.js, whose interface object stays unexposed). Loaded LAST.
(function () {
  const GONE = ['AnimationTrigger', 'AudioPlaybackStats', 'HTMLCameraElement', 'HTMLMicrophoneElement', 'HTMLUserMediaElement',
    'InteractionContentfulPaint', 'LanguageModel', 'NodeRange', 'OpaqueRange', 'PerformanceSoftNavigation', 'PermissionsPolicy',
    'TimelineTriggerRange', 'TimelineTriggerRangeList', 'TimelineTrigger', 'XRCompositionLayer', 'XRCubeLayer', 'XRCylinderLayer',
    'XREquirectLayer', 'XRLayerEvent', 'XRPlane', 'XRPlaneSet', 'XRProjectionLayer', 'XRQuadLayer', 'XRSubImage', 'XRWebGLSubImage',
    'Sanitizer', 'XSLTProcessor', 'requestResize', 'FontFaceSet', 'CSSPseudoElement'];
  for (const n of GONE) { try { delete globalThis[n]; } catch (e) {} }
})();
