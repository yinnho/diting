(function () { // @@SCOPE
// god-file split (刀87, CDP emulation belt): the persona/viewport/touch/
// media/fp-seed knobs Rust and __diting_init drive — setPersona (screen,
// dpr, hw cores, deviceMemory from the persona pool; publishes the
// viewport to the Rust layout layer), setViewport/clearViewport (the
// device-metrics override window — persona stays the identity, the
// override is the window), __diting_touchMax/setTouch (touch capability
// face without touching metrics), setMediaFeatures (emulated-media
// environment), setFpSeed (cross-realm seed re-pin). Every cross-file
// touch is member-form or accessor/d()-mediated and resolved at CALL
// time: _fp/_fpPlatform (fingerprint d()), _fpSeed/_fpCache (fingerprint
// a() live accessors), _domRaw (d()), __vp* (screen_viewport a()),
// mqRecompute/mqFlush (media_queries members), navSet/screenApply/
// makeVisualViewport (members). Callers: __diting_init (member call),
// Rust set_user_agent / CDP override paths (member/eval). Any slot safe;
// registered beside media_queries' hook table.
// (Re)apply the platform persona: screen, dpr, hardwareConcurrency,
// deviceMemory. Called from __diting_init AND again from Rust's
// set_user_agent — the runtime constructor runs init before the context's
// UA is known, so the persona must refresh once the real UA lands.
globalThis.__diting_setPersona = function() {
  const scr = _fp('screen');
  const sw = scr[0], sh = scr[1];
  globalThis.__diting_screenApply(sw, sh);
  globalThis.visualViewport = globalThis.__diting_makeVisualViewport(sw, sh - 80);
  // From the persona pool, so a retina Mac panel reports 2x (the old
  // width-only heuristic gave 1x on 1512x982 — impossible for that panel).
  globalThis.devicePixelRatio = _fp('dpr') || (sw >= 2560 ? 2 : 1);
  __vpW = sw; __vpH = sh - 80; __vpOW = sw; __vpOH = sh;
  // Publish the persona viewport to the Rust layout layer so
  // getBoundingClientRect / element rects anchor the initial containing
  // block to the same window scripts see (not the pre-persona default).
  try { _domRaw("set_viewport", String(sw), String(sh - 80)); } catch (e) {}

  // Stable for the life of the process (drawn once), not per navigation —
  // hardwareConcurrency flipping between pages of one visit is its own
  // automation tell. Cores come from the persona device table (_fp('hw')) so
  // they stay coherent with the GPU/UAData architecture face. (P8) Memory is
  // ALSO persona-driven now (_fp('mem'), drawn from per-chip RAM options):
  // Chrome 155 dropped the 8 GiB deviceMemory cap, so real machines report
  // 16/32 — and an M3 Pro persona claiming 8 GB is a config Apple never
  // shipped, a cross-field contradiction the 300015 scorer can key on.
  const plat = _fpPlatform();
  if (globalThis.__diting_hw === undefined || globalThis.__diting_hw_plat !== plat) {
    globalThis.__diting_hw = _fp('hw');
    globalThis.__diting_mem = _fp('mem');
    globalThis.__diting_hw_plat = plat;
  }
  globalThis.__diting_navSet('hardwareConcurrency', globalThis.__diting_hw);
  globalThis.__diting_navSet('deviceMemory', globalThis.__diting_mem);
};

// Viewport override for session_viewport / CDP setDeviceMetricsOverride.
// The persona stays the identity of the machine; the override is the window
// the page is looked at through — screen keeps the persona while
// innerWidth/innerHeight/visualViewport/outer* move, which is what Chrome's
// device emulation does too. The mobile flag swaps the matchMedia pointer/
// hover answers to coarse/none: a phone-sized viewport answering
// `pointer: fine` is a contradiction a fingerprint script can cross-check
// against maxTouchPoints, so those move together.
// devicePixelRatio follows the same split: the 4th arg (CDP
// setDeviceMetricsOverride's deviceScaleFactor) pins what scripts see when
// > 0, 0 restores the persona's (Chromium's "0 = default"), undefined
// leaves it untouched.
globalThis.__diting_setViewport = function(w, h, mobile, dpr) {
  __vpW = w; __vpH = h; __vpOW = w; __vpOH = h;
  if (typeof dpr === 'number' && dpr > 0) globalThis.devicePixelRatio = dpr;
  else if (dpr === 0) globalThis.devicePixelRatio = _fp('dpr') || (globalThis.screen.width >= 2560 ? 2 : 1);
  globalThis.visualViewport = globalThis.__diting_makeVisualViewport(w, h);
  globalThis.__diting_mq_mobile = mobile ? _MQ_MOBILE_TABLE : null;
  globalThis.__diting_mqRecompute();
  // (#117) through the store: defineProperty on the hoisted instance would
  // materialize an OWN prop and break Object.keys(navigator) === [].
  // The touch pin outranks the device class (an explicit
  // setTouchEmulationEnabled survives a viewport re-pin; #151).
  globalThis.__diting_navSet('maxTouchPoints', __diting_touchMax() || (mobile ? 5 : 0));
  // Feed the Rust layout ICB so element rects and @media cascade see the
  // same width the scripts do.
  try { _domRaw("set_viewport", String(w), String(h)); } catch (e) {}
  try { globalThis.dispatchEvent(new Event('resize')); } catch (e) {}
  // Media queries re-evaluated after the resize: subscribed MQLs that
  // crossed fire change (issue #25).
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// Drop the override: persona viewport everywhere, desktop pointer answers.
// Emulated media (setEmulatedMedia) and touch emulation are separate Chrome
// knobs and survive.
globalThis.__diting_clearViewport = function() {
  globalThis.__diting_mq_mobile = null;
  globalThis.__diting_mqRecompute();
  __diting_setPersona();
  globalThis.__diting_navSet('maxTouchPoints', __diting_touchMax());
  try { globalThis.dispatchEvent(new Event('resize')); } catch (e) {}
  // Media queries re-evaluated after the resize: subscribed MQLs that
  // crossed fire change (issue #25).
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// Touch emulation (CDP Emulation.setTouchEmulationEnabled, #151): swap the
// interaction-media answers and maxTouchPoints WITHOUT touching the
// viewport — device metrics are setDeviceMetricsOverride's knob, this is
// only the touch capability face. The pin is the point count (> 0); null
// restores the device class's own touch story (0 desktop, 5 mobile).
function __diting_touchMax() {
  const t = globalThis.__diting_touch_points;
  return typeof t === 'number' && t > 0 ? t : 0;
}
globalThis.__diting_setTouch = function(enabled, maxPoints) {
  globalThis.__diting_mq_touch = enabled ? _MQ_TOUCH_TABLE : null;
  globalThis.__diting_touch_points = enabled ? (maxPoints > 0 ? maxPoints : 1) : null;
  globalThis.__diting_mqRecompute();
  globalThis.__diting_navSet('maxTouchPoints', enabled ? globalThis.__diting_touch_points
    : (globalThis.__diting_mq_mobile ? 5 : 0));
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// Emulated media environment (CDP Emulation.setEmulatedMedia, Playwright's
// page.emulateMedia): featuresJson is a JSON array of [name, value] pairs
// REPLACING all emulated preferences (Chrome semantics — an absent feature
// stops being emulated), mediaType is 'print' or 'screen'. The same pairs
// go to the Rust layout state so the @media cascade re-parses in agreement
// (two faces, one truth), then subscribed MQLs flush (issue #29).
globalThis.__diting_setMediaFeatures = function(featuresJson, mediaType) {
  let pairs = [];
  try { pairs = JSON.parse(featuresJson) || []; } catch (e) {}
  const emu = {};
  if (Array.isArray(pairs)) {
    for (const pair of pairs) {
      if (!Array.isArray(pair) || pair.length < 2) continue;
      const name = String(pair[0]).toLowerCase(), value = String(pair[1]).toLowerCase();
      if (!name) continue;
      emu[name] = {};
      emu[name][value] = true;
    }
  }
  globalThis.__diting_mq_emulated = Object.keys(emu).length ? emu : null;
  globalThis.__diting_mq_media_type = mediaType === 'print' ? 'print' : 'screen';
  try { _domRaw("set_media_env", featuresJson, globalThis.__diting_mq_media_type); } catch (e) {}
  globalThis.__diting_mqRecompute();
  try { globalThis.__diting_mqFlush(); } catch (e) {}
};

// The hardware persona (screen/dpr/GPU/canvas) must hold for a whole
// visit: a real machine does not change its panel or GPU between pages,
// and identity-level values flipping per navigation is its own automation
// tell (the invariant hardwareConcurrency already keeps). __diting_init
// runs once per realm (it self-deletes), so cross-page continuity comes
// from the host re-pinning the same seed via __diting_setFpSeed after the
// fresh realm boots — Page-owned state, like the viewport override.
globalThis.__diting_setFpSeed = function(s) {
  _fpSeed = s | 0;
  _fpCache = null;
  // __diting_init's throwaway draw already filled the hw/deviceMemory guard
  // cache before this re-pin lands; drop it so the persona redraws them from
  // the pinned seed (the guard then holds them put for the rest of the realm).
  globalThis.__diting_hw = undefined;
  globalThis.__diting_mem = undefined;
  globalThis.__diting_hw_plat = undefined;
  __diting_setPersona();
};

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
