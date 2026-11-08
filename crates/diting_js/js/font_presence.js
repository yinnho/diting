(function () { // @@SCOPE
// god-file split (刀21, font-presence tables + layout measurement helpers):
// the per-OS installed-font tables, the font-box/local-box/layout-box
// measurement primitives and the offsetParent walk — everything element
// geometry emulation reads at call time. External deps are the published
// _domRaw bridge and ua_helpers' __ditingPlatformFromUA; the four internal
// tables are file-private (no cross-file consumer). NOT the generated
// per-glyph advance tables — those live in the 刀97 artifact font_metrics.js.
// ---- Font-presence metrics -------------------------------------------------
// Font-probe fingerprinters (Castle's fonts collector, fingerprintjs,
// WorkOS Radar) append a hidden 72px span and compare offsetWidth/offsetHeight
// across `'Font Name', serif/sans-serif/monospace` lists: an INSTALLED font
// changes the text box, a missing one falls back to the generic family and
// matches the baseline exactly. The installed sets below are the fonts a real
// Chrome can resolve per OS — Windows-only faces (Segoe UI, Calibri,
// Consolas) are absent on macOS and vice versa (Menlo, Monaco), so the
// detected set stays coherent with the UA persona.
const _DITING_FONT_PLATFORM = () => {
  const p = __ditingPlatformFromUA();
  if (p === "Win32") return "win";
  if (p === "MacIntel" || p === "iPhone") return "mac";
  return "linux";
};
const _INSTALLED_FONTS = {
  // fp-diff-1014 P1: a macOS-zh persona answered fonts.check false for
  // PingFang SC / Hiragino / STHeiti — fonts every Mac ships. These lists are
  // the single source for BOTH faces (span-measure widths above and
  // document.fonts below), so they can't drift apart. Mac includes the
  // Office/aftermarket CJK faces (YaHei/SimSun) common on zh machines —
  // missing system fonts is a hard tell, extra installable ones a soft none.
  mac: ["American Typewriter", "Andale Mono", "Apple Braille", "Apple SD Gothic Neo",
    "AppleGothic", "Apple Symbols", "Arial", "Arial Black", "Arial Narrow",
    "Arial Unicode MS", "Avenir", "Avenir Next", "Baskerville", "Big Caslon",
    "Brush Script MT", "Chalkboard", "Chalkduster", "Cochin", "Comic Sans MS",
    "Copperplate", "Courier New", "Futura", "Geneva", "Georgia", "Gill Sans",
    "Helvetica", "Helvetica Neue", "Hiragino Maru Gothic ProN", "Hiragino Sans",
    "Hiragino Sans CNS", "Hiragino Sans GB", "Hoefler Text", "Impact",
    "Kaiti SC", "Kaiti SC Black", "Lucida Grande", "Marker Felt", "Menlo",
    "Microsoft YaHei", "Monaco", "Apple Color Emoji", "Noto Color Emoji", "Optima", "Palatino",
    "Papyrus", "PingFang HK", "PingFang SC", "PingFang TC", "SimSun",
    "Snell Roundhand", "Songti SC", "STHeiti", "STHeiti Light", "STKaiti",
    "STSong", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana",
    "Yuanti SC", "Zapfino"],
  win: ["Arial", "Arial Black", "Arial Narrow", "Bahnschrift", "Calibri",
    "Calibri Light", "Cambria", "Cambria Math", "Candara", "Comic Sans MS",
    "Consolas", "Constantia", "Corbel", "Courier New", "Century Gothic",
    "DengXian", "Ebrima",
    "FangSong", "Franklin Gothic Medium", "Gabriola", "Gadugi", "Garamond",
    "Georgia", "Impact", "Ink Free", "Javanese Text", "KaiTi", "Leelawadee UI",
    "Lucida Console", "Lucida Sans Unicode", "Malgun Gothic", "Marlett",
    "Microsoft Himalaya", "Microsoft JhengHei", "Microsoft New Tai Lue",
    "Microsoft Sans Serif", "Microsoft YaHei", "Microsoft Yi Baiti",
    "MingLiU-ExtB", "Mongolian Baiti", "MS Gothic", "MS PGothic", "MS PMincho",
    "MV Boli", "Myanmar Text", "Nirmala UI", "NSimSun", "Palatino Linotype",
    "Segoe Print", "Segoe Script", "Segoe UI", "Segoe UI Emoji",
    "Segoe UI Historic", "Segoe UI Symbol", "SimHei", "SimSun", "Sitka",
    "Sylfaen", "Symbol", "Tahoma", "Times New Roman", "Trebuchet MS",
    "Verdana", "Webdings", "Wingdings", "Yu Gothic"],
  linux: ["Arial", "C059", "Cantarell", "Courier New", "DejaVu Sans",
    "DejaVu Sans Mono", "DejaVu Serif", "Droid Sans Fallback", "FreeSans",
    "FreeSerif", "Georgia", "Liberation Mono", "Liberation Sans",
    "Liberation Serif", "Noto Color Emoji", "Noto Mono", "Noto Sans",
    "Noto Sans CJK JP", "Noto Sans CJK SC", "Noto Serif", "Noto Serif CJK SC",
    "P052", "Times New Roman", "Trebuchet MS", "Ubuntu", "Ubuntu Mono",
    "Verdana", "WenQuanYi Micro Hei", "WenQuanYi Zen Hei"],
};
// Stable per-name width factor for an installed font, or null when the OS
// persona doesn't have it. Always ≥1% off the generic baseline (0.92–1.14,
// values inside ±1% nudged out) so the probe reliably detects installed
// faces, exactly like a real font's metrics never coincide with the fallback.
function _installedFontFactor(name) {
  const list = _INSTALLED_FONTS[_DITING_FONT_PLATFORM()] || _INSTALLED_FONTS.mac;
  if (list.indexOf(name) === -1) return null;
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (Math.imul(h, 31) + name.charCodeAt(i)) | 0;
  let f = 0.92 + ((h >>> 0) % 23) / 100;
  if (f > 0.99 && f < 1.01) f += 0.03;
  return f;
}
// Per-character advance widths (em) approximating Helvetica for lowercase,
// with caps/digits from the same metrics table. Enough spread for
// "mmMwWLli10Oo#@"-style probe strings to discriminate families.
const _FONT_CHAR_EM = (ch) => {
  switch (ch) {
    case "i": case "l": case "j": return 0.222;
    case "t": case "f": return 0.278;
    case "r": return 0.333;
    case "m": return 0.833;
    case "w": return 0.722;
    case "M": case "W": return 0.944;
    case "L": return 0.611;
    case "O": case "Q": case "G": case "D": case "U": case "H": case "B": case "N": case "R": case "S": case "E": case "K": return 0.722;
    case "C": case "P": case "F": case "A": case "V": case "X": case "Y": return 0.667;
    case "T": case "J": case "Z": case "I": return 0.611;
    case "o": case "0": case "1": case "#": case "e": case "a": case "s": case "c": case "u": case "n": case "d": case "g": case "p": case "q": case "b": case "h": return 0.556;
    case "@": return 0.833;
    default: return /[A-Z]/.test(ch) ? 0.7 : 0.5;
  }
};
// Generic-family width scale + line-height factor (em multiples).
const _GENERIC_FONT = {
  "serif": { w: 0.95, h: 1.15 },
  "sans-serif": { w: 1.0, h: 1.2 },
  "monospace": { w: 1.18, h: 1.21 },
  "cursive": { w: 1.02, h: 1.2 },
  "fantasy": { w: 1.04, h: 1.2 },
  "system-ui": { w: 1.0, h: 1.2 },
  "ui-sans-serif": { w: 1.0, h: 1.2 },
};
// Synthesize a leaf text element's content box from its inline font styles.
// Returns null for elements we shouldn't size this way (containers, no text)
// so their callers keep the default synthesized cell.
function _ditingFontBox(el) {
  try {
    const st = el.style;
    if (!st || !st._props) return null;
    const props = st._props;
    const display = props["display"];
    if (display === "none") return { w: 0, h: 0 };
    if (el.children && el.children.length) return null;
    // Only the font-probe shape (font-family set inline, size left to the
    // text): an element the page sized itself, or one with no font at all,
    // belongs to layout. Without this gate every inline-styled leaf with
    // text reported the text's box instead of its laid-out one.
    if (!(props["font-family"] || props["fontFamily"])) return null;
    if (props["width"] || props["height"] || props["min-width"] || props["min-height"]) return null;
    const text = (el.textContent || "").trim();
    if (!text) return null;
    const fontSize = parseFloat(props["font-size"] || props["fontSize"]) || 16;
    const famRaw = props["font-family"] || props["fontFamily"] || "serif";
    const names = String(famRaw).split(",").map((s) => s.trim().replace(/^['"]|['"]$/g, "")).filter(Boolean);
    // First installed family wins; otherwise the trailing generic.
    let chosen = null, generic = null;
    for (const n of names) {
      if (_GENERIC_FONT[n]) { generic = generic || n; continue; }
      if (chosen === null && _installedFontFactor(n) !== null) { chosen = n; break; }
    }
    if (!generic) generic = /sans/i.test(famRaw) ? "sans-serif" : "serif";
    const g = _GENERIC_FONT[generic] || _GENERIC_FONT["serif"];
    const ff = chosen ? _installedFontFactor(chosen) : 1;
    let w = 0;
    for (const ch of text) w += _FONT_CHAR_EM(ch);
    return {
      w: Math.round(w * fontSize * g.w * ff),
      h: Math.round(fontSize * g.h * (chosen ? 1 + (ff - 1) * 0.45 : 1)),
    };
  } catch { return null; }
}

// Real taffy-layout geometry in document coordinates — the same source
// getBoundingClientRect reads, but WITHOUT its __diting_click_target side
// effect: a plain size read must never stamp the click-target hint.
// Returns {x,y,w,h} or null (layout feature off / detached node).
// Fabricated iframe documents (obscura #976): walk up parentNode looking for
// the marker _IframeDocument stamps on its orphan root. Element.ownerDocument
// can't do this — it answers the MAIN document for every element, including
// iframe-doc ones.
function _ditingIframeDoc(el) {
  for (let n = el; n; n = n.parentNode) {
    if (n._isIframeDocRoot) return n._ownerDoc;
  }
  return null;
}

// The viewport a fabricated iframe document lays out against: the iframe
// ELEMENT's laid-out content box on the host page (Chrome semantics — a
// 400px-wide iframe gives its document a 400px viewport, and 100vh inside
// means that height). Measured through the main run's layout_rect (never
// the sub-run, so no recursion); the 300x150 replaced-box default stands
// until the host page has laid the iframe out. layout_rect is the border
// box, so the UA 2px border (and any padding) comes back off — a default
// iframe frames a 300x150 document, not 304x154.
function _ditingIframeViewport(el) {
  const doc = _ditingIframeDoc(el);
  const ifr = doc && doc._iframeEl;
  if (ifr && ifr._nid != null) {
    try {
      const raw = _domRaw("layout_rect", String(ifr._nid | 0), "");
      const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
      if (Array.isArray(arr) && arr.length === 4 && arr[2] > 0 && arr[3] > 0) {
        const cs = getComputedStyle(ifr);
        const px = (v) => parseFloat(v) || 0;
        const w = arr[2] - px(cs.borderLeftWidth) - px(cs.borderRightWidth)
                        - px(cs.paddingLeft) - px(cs.paddingRight);
        const h = arr[3] - px(cs.borderTopWidth) - px(cs.borderBottomWidth)
                        - px(cs.paddingTop) - px(cs.paddingBottom);
        if (w > 0 && h > 0) return { w, h, arg: w + "," + h };
      }
    } catch (e) { /* host layout unavailable */ }
  }
  return { w: 300, h: 150, arg: "300,150" };
}

function _ditingLayoutBox(el) {
  try {
    if (el._nid == null) return null;
    const idoc = _ditingIframeDoc(el);
    const raw = idoc
      ? _domRaw("iframe_layout_rect", String(el._nid | 0), _ditingIframeViewport(el).arg)
      : _domRaw("layout_rect", String(el._nid | 0), "");
    const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
    if (Array.isArray(arr) && arr.length === 4 && Number.isFinite(arr[0])) {
      return { x: arr[0], y: arr[1], w: arr[2], h: arr[3] };
    }
  } catch { /* no layout box */ }
  return null;
}

// Transform-free LOCAL box origin (blitz #663 family): offsetLeft/offsetTop
// ignore the element's own (and ancestors') paint transforms — they read
// the pre-map border box, which gBCR (transform-mapped) can't answer once
// rotate/skew land. Falls back to the mapped box when the element is
// boxless or the layout feature is off.
function _ditingLocalBox(el) {
  try {
    if (el._nid == null) return null;
    const idoc = _ditingIframeDoc(el);
    const raw = idoc
      ? _domRaw("iframe_local_geom", String(el._nid | 0), _ditingIframeViewport(el).arg)
      : _domRaw("local_geom", String(el._nid | 0), "");
    const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
    if (Array.isArray(arr) && arr.length === 10 && Number.isFinite(arr[0])) {
      return { x: arr[0], y: arr[1], w: arr[2], h: arr[3] };
    }
  } catch { /* no local geometry */ }
  return _ditingLayoutBox(el);
}

// CSSOM: offset*/client*/scroll* of a display:none element are all 0 — the
// init-in-hidden-container pattern (map libs size their container while
// hidden, re-measure on show) must read 0, not the 100x20 stub. Answered
// from the computed_style op so stylesheet-hidden elements count too, not
// just inline `style.display`.
function _ditingDisplayNone(el) {
  try {
    if (el._nid == null) return false;
    const raw = _domRaw("computed_style", String(el._nid | 0), "");
    const s = typeof raw === "string" ? JSON.parse(raw) : raw;
    return !!(s && s.display === "none");
  } catch { return false; }
}

// CSSOM View offsetParent (#128). The walk MUST terminate in null, never
// undefined: Fusion-style accumulators do `e = e.offsetParent;
// while (null !== e && e !== anchor)` — an undefined return passes the
// null test and the next iteration reads .offsetTop of undefined, killing
// the component mid-mount (tmall publish editor / upload selector).
// Chrome semantics mirrored: html/body, an unrendered chain (display:none),
// or position:fixed → null; otherwise the nearest ancestor that is a
// table cell/table or positioned, falling back to body. Detached subtrees
// and shadow-root boundaries fall out of the walk → null. Position/display
// come from the computed_style op (the _ditingDisplayNone precedent); with
// no layout data (blind build) every ancestor reads position undefined,
// degrading to "parent chain up to body" — still a null-terminated chain.
function _ditingOffsetParent(el) {
  try {
    if (el._nid == null) return null;
    const doc = el.ownerDocument;
    if (!doc) return null;
    if (el === doc.documentElement || el === doc.body) return null;
    const tagOf = (e) => (e.tagName || "").toUpperCase();
    if (tagOf(el) === "HTML" || tagOf(el) === "BODY") return null;
    const styleOf = (e) => {
      const raw = _domRaw("computed_style", String(e._nid | 0), "");
      const s = typeof raw === "string" ? JSON.parse(raw) : raw;
      return s || {};
    };
    const self = styleOf(el);
    if (self.display === "none" || self.position === "fixed") return null;
    let p = el.parentNode;
    while (p) {
      if (p.nodeType !== 1) return null; // Document/ShadowRoot: off-tree or shadow boundary
      if (p === doc.body) return p;
      const s = styleOf(p);
      if (s.display === "none") return null;
      const t = tagOf(p);
      if (t === "TD" || t === "TH" || t === "TABLE") return p;
      if (s.position && s.position !== "static") return p;
      p = p.parentNode;
    }
    return null; // ran off the top without hitting body: detached subtree
  } catch { return null; }
}


// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_ditingDisplayNone', _ditingDisplayNone);
  d('_ditingFontBox', _ditingFontBox);
  d('_ditingIframeDoc', _ditingIframeDoc);
  d('_ditingIframeViewport', _ditingIframeViewport);
  d('_ditingLayoutBox', _ditingLayoutBox);
  d('_ditingLocalBox', _ditingLocalBox);
  d('_ditingOffsetParent', _ditingOffsetParent);
  d('_installedFontFactor', _installedFontFactor);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
