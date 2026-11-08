(function () { // @@SCOPE
// god-file split (刀64, media queries): the persona bool table, the mobile/
// touch override tables (later layers replace a feature's whole table — the
// emulated value IS the value), __diting_mqRecompute (fold), the
// expression parser trio (_mqExpr/_mqClause/_mqMatches), the MediaQueryList
// constructor (registry-backed, change events on flush), __diting_mqFlush
// (viewport-change pump) and the matchMedia face — evaluating against the
// live window viewport so JS branching agrees with the CSS cascade.
// Consumers (global_surface/iframe_realm tables, event_hooks registry
// pruning) are string-table data or runtime calls. Native marking at the
// install site (travels, 刀22). Pin _MQ_PERSONA_BOOL flipped (刀52).
const _markNative = globalThis.__diting_markNative;
// matchMedia evaluates against the live window viewport — the same
// innerWidth/innerHeight the persona publishes and set_viewport feeds the
// layout ICB — so a page's JS branching agrees with the @media rules the CSS
// cascade already applied. The old always-false stub answered
// `(min-width:640px)` with false while a 2560px viewport was published:
// wrong for responsive pages and a self-contradiction any fingerprint
// script could catch by cross-checking the two. Covers what page scripts
// actually branch on: width/height bounds, orientation, aspect-ratio,
// resolution, and the preference features pinned to the desktop-light
// persona. Comma (or) / and / not follow the classic grammar; unknown
// features evaluate false, matching Chrome's answer for unsupported ones.
const _MQ_PERSONA_BOOL = {
  'prefers-color-scheme': { light: true, dark: false },
  'prefers-reduced-motion': { 'no-preference': true, reduce: false },
  'prefers-reduced-transparency': { 'no-preference': true, reduce: false },
  'color-gamut': { srgb: true, p3: false, rec2020: false },
  'dynamic-range': { standard: true, high: false },
  'display-mode': { browser: true, standalone: false, 'minimal-ui': false, fullscreen: false, 'window-controls-overlay': false },
  pointer: { fine: true, coarse: false, none: false },
  'any-pointer': { fine: true, coarse: false, none: false },
  hover: { hover: true, none: false },
  'any-hover': { hover: true, none: false },
};
// What a mobile viewport override pins the interaction features to (and the
// phone-shaped preference defaults). Kept as a table so the mobile override
// and the emulated-media override can merge per feature in __diting_mqRecompute.
const _MQ_MOBILE_TABLE = {
  'prefers-color-scheme': { light: true, dark: false },
  'prefers-reduced-motion': { 'no-preference': true, reduce: false },
  'prefers-reduced-transparency': { 'no-preference': true, reduce: false },
  'color-gamut': { srgb: true, p3: false, rec2020: false },
  'dynamic-range': { standard: true, high: false },
  'display-mode': { browser: true, standalone: false, 'minimal-ui': false, fullscreen: false, 'window-controls-overlay': false },
  pointer: { coarse: true, fine: false, none: false },
  'any-pointer': { coarse: true, fine: false, none: false },
  hover: { none: true, hover: false },
  'any-hover': { none: true, hover: false },
};
// What touch emulation (CDP Emulation.setTouchEmulationEnabled) pins — a
// NARROWER table than the mobile viewport: touch changes the pointer/hover
// story only, never the prefers-* preferences or display-mode (#151).
const _MQ_TOUCH_TABLE = {
  pointer: { coarse: true, fine: false, none: false },
  'any-pointer': { coarse: true, fine: false, none: false },
  hover: { none: true, hover: false },
  'any-hover': { none: true, hover: false },
};
// Fold the mobile table, the touch table, and the emulated-media table (CDP
// Emulation.setEmulatedMedia / Playwright page.emulateMedia) into the one
// truth table _mqExpr reads. Later layers replace a feature's whole table —
// Chrome's rule that the emulated value is THE value — so
// prefers-reduced-motion: reduce survives even under a mobile override.
globalThis.__diting_mqRecompute = function () {
  const mobile = globalThis.__diting_mq_mobile, touch = globalThis.__diting_mq_touch, emu = globalThis.__diting_mq_emulated;
  if (!mobile && !touch && !emu) { globalThis.__diting_mq_overrides = null; return; }
  const t = {};
  if (mobile) for (const k in mobile) t[k] = mobile[k];
  if (touch) for (const k in touch) t[k] = touch[k];
  if (emu) for (const k in emu) t[k] = emu[k];
  globalThis.__diting_mq_overrides = t;
};
function _mqExpr(inner) {
  const colon = inner.indexOf(':');
  const feature = (colon < 0 ? inner : inner.slice(0, colon)).trim().toLowerCase();
  const value = colon < 0 ? null : inner.slice(colon + 1).trim().toLowerCase();
  const ov = globalThis.__diting_mq_overrides;
  if (ov && feature in ov) return value != null && ov[feature][value] === true;
  if (feature in _MQ_PERSONA_BOOL) return value != null && _MQ_PERSONA_BOOL[feature][value] === true;
  if (value == null) return false;
  const num = parseFloat(value);
  const vw = globalThis.innerWidth || 0, vh = globalThis.innerHeight || 0;
  switch (feature) {
    case 'min-width': return num <= vw;
    case 'max-width': return num >= vw;
    case 'width': return num === vw;
    case 'min-height': return num <= vh;
    case 'max-height': return num >= vh;
    case 'height': return num === vh;
    case 'orientation': return value === (vw >= vh ? 'landscape' : 'portrait');
    case 'aspect-ratio': case 'min-aspect-ratio': case 'max-aspect-ratio': {
      const m = value.match(/^(\d*\.?\d+)\s*\/\s*(\d*\.?\d+)$/);
      if (!m) return false;
      const want = parseFloat(m[1]) / parseFloat(m[2]), have = vw / vh;
      return feature === 'aspect-ratio' ? Math.abs(have - want) < 1e-6
        : feature === 'min-aspect-ratio' ? have >= want : have <= want;
    }
    case 'resolution': case 'min-resolution': case 'max-resolution': {
      // Normalize the value to device px per css px (dppx) and compare
      // against the persona's devicePixelRatio.
      let dppx;
      if (/dppx|x$/.test(value)) dppx = num;
      else if (/dpcm$/.test(value)) dppx = (num * 96) / 2.54;
      else dppx = num / 96; // dpi; a unitless number is invalid CSS, dpi is the least-wrong read
      const dpr = globalThis.devicePixelRatio || 1;
      return feature === 'resolution' ? Math.abs(dppx - dpr) < 1e-6
        : feature === 'min-resolution' ? dppx <= dpr : dppx >= dpr;
    }
    default: return false;
  }
}
function _mqClause(clause) {
  let s = clause.trim(), invert = false;
  if (/^not\s+/i.test(s)) { invert = true; s = s.replace(/^not\s+/i, ''); }
  s = s.replace(/^only\s+/i, '');
  const parts = s.split(/\s+and\s+/i);
  let i = 0;
  if (parts[0] && parts[0][0] !== '(') {
    const mt = parts[0].toLowerCase();
    // The media type token follows the emulated type when one is active
    // (setEmulatedMedia media:'print'), the persona's screen otherwise.
    const cur = globalThis.__diting_mq_media_type || 'screen';
    if (mt !== 'all' && mt !== cur) return invert;
    i = 1;
  }
  for (; i < parts.length; i++) {
    const p = parts[i].trim();
    if (!p) continue;
    if (!(p[0] === '(' && p[p.length - 1] === ')') || !_mqExpr(p.slice(1, -1))) return invert;
  }
  return !invert;
}
function _mqMatches(q) {
  const s = String(q).trim();
  if (s === '') return true;
  const clauses = [];
  let depth = 0, cur = '';
  for (const ch of s) {
    if (ch === '(') depth++;
    else if (ch === ')') depth--;
    if (ch === ',' && depth === 0) { clauses.push(cur); cur = ''; continue; }
    cur += ch;
  }
  clauses.push(cur);
  return clauses.some((c) => c.trim() !== '' && _mqClause(c));
}
// Live MediaQueryList (issue #25, obscura#1007 same lineage): `matches`
// is a getter re-evaluating against the current viewport/overrides, and
// listeners live on the object. __diting_setViewport /
// __diting_clearViewport flush the registry afterwards and fire `change`
// (carrying matches/media) on every object that crossed — the half
// responsive pages need beyond reading matches once at boot.
// const, not a function declaration: a top-level declaration would mint a NON-configurable window property (Chrome's
// MediaQueryList is a normal configurable interface global; it is installed explicitly below).
const MediaQueryList = function MediaQueryList(q) {
  this._q = String(q);
  this._last = false;
  try { this._last = _mqMatches(this._q); } catch (e) {}
  this.onchange = null;
};
// Read-only prototype attributes (Chrome: no instance own properties, no setters).
Object.defineProperties(MediaQueryList.prototype, {
  matches: { get: _markNative({ get matches() { try { return _mqMatches(this._q); } catch (e) { return false; } } }.__lookupGetter__('matches')), set: undefined, enumerable: true, configurable: true },
  media: { get: _markNative({ get media() { return this._q; } }.__lookupGetter__('media')), set: undefined, enumerable: true, configurable: true },
});
MediaQueryList.prototype.addListener = function (fn) { this.addEventListener('change', fn); };
MediaQueryList.prototype.removeListener = function (fn) { this.removeEventListener('change', fn); };
// 刀56: the hand-rolled trio is gone — MediaQueryList inherits the native
// EventTarget core like Chrome (listeners with full options semantics;
// onchange fires through the dispatch handler hook; the accessor is
// prototype-level, so isHandlerSlot passes).
if (typeof globalThis.MediaQueryList === 'undefined') globalThis.MediaQueryList = MediaQueryList;
const _mqlRegistry = [];
// Re-evaluate every subscribed MQL against the new environment and fire
// change on the ones that flipped. Inert objects (no listener, no
// onchange) are pruned first: their matches is live-computed on read, so
// only subscribed objects need tracking — matchMedia inside a resize
// handler cannot grow the registry unbounded. Listener liveness is the
// hooks' registry mirror (event_hooks.js); _change died with the trio.
globalThis.__diting_mqFlush = function () {
  const hasListeners = globalThis.__diting_etHasListeners || (() => false);
  for (let i = _mqlRegistry.length - 1; i >= 0; i--) {
    const mq = _mqlRegistry[i];
    if (mq.onchange == null && !hasListeners(mq, 'change')) _mqlRegistry.splice(i, 1);
  }
  for (const mq of _mqlRegistry.slice()) {
    const now = mq.matches;
    if (now === mq._last) continue;
    mq._last = now;
    const ev = new Event('change');
    try { ev.matches = now; ev.media = mq._q; } catch (e) {}
    try { mq.dispatchEvent(ev); } catch (e) {}
  }
};
globalThis.matchMedia = _markNative(function matchMedia(q) {
  const mq = new MediaQueryList(q);
  _mqlRegistry.push(mq);
  return mq;
});

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_MQ_MOBILE_TABLE', _MQ_MOBILE_TABLE);
  d('_MQ_PERSONA_BOOL', _MQ_PERSONA_BOOL);
  d('_MQ_TOUCH_TABLE', _MQ_TOUCH_TABLE);
  d('_markNative', _markNative);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
