// god-file split (domcore.js ratchet repay): the innerText machinery — UA tag
// tables, the cascade pocket-read, the block/cell separator walk. Sole consumer
// is Element's innerText getter in domcore.js (call time, via the exports block
// below). Pure helpers: zero installs, zero prototype touches.
(function () { // @@SCOPE

// innerText machinery (see the getter on Element). Tags whose UA default is
// display:none (never rendered) and block-level (line-breaking) respectively.
const _IT_SKIP_TAGS = new Set(['script', 'style', 'template', 'noscript']);
const _IT_BLOCK_TAGS = new Set(('address article aside blockquote center dd details dialog dir div dl dt ' +
  'fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 header hgroup hr li listing main menu nav ol p pre ' +
  'section summary table tfoot tr ul').split(' '));
const _IT_BLOCK_DISPLAY = /^(block|flow-root|list-item|flex|inline-flex|grid|inline-grid|table|table-row|table-row-group|table-header-group|table-footer-group|table-caption)$/;
const _IT_PRE_TAGS = new Set(['pre', 'textarea', 'listing']);
// display / white-space / text-transform only. The full computed-style
// table per node held a GitHub-sized innerText past the fetch deadline.
// Null when the style engine hasn't answered for this element yet.
const _itCascade = (el) => {
  try {
    if (el && el._nid != null) {
      const raw = _domRaw('inner_text_style', String(el._nid | 0), '');
      if (raw && raw !== 'null') {
        const t = JSON.parse(raw);
        if (t && typeof t === 'object') return t;
      }
    }
  } catch (e) {}
  return null;
};
const _innerTextDisplay = (el) => {
  // Inline style first, then the hidden attribute, then the cascade table —
  // the cascade answers with defaults ('block') for any element it has seen,
  // so asking it first would shadow [hidden] entirely whenever a style pass
  // has run (feature-dependent behavior otherwise).
  const s = el && el.style && el.style.display;
  if (s) return s;
  if (el && el.getAttribute && el.getAttribute('hidden') != null) return 'none';
  const cs = _itCascade(el);
  if (cs && cs.display != null) return String(cs.display);
  return null;
};
const _itPushSep = (out, sep) => {
  // Push a structural separator (block '\n', table-cell '\t'). Consecutive
  // cell tabs stack (Chrome emits one per empty cell); a row break eats a
  // trailing tab; a text run before the break loses its trailing spaces.
  while (out.length) {
    const last = out[out.length - 1];
    if (last.s.endsWith('\n')) return;
    if (last.sep && sep === '\n') { out.pop(); continue; }
    if (!last.sep && !last.pre) {
      last.s = last.s.replace(/[ \t]+$/, '');
      if (last.s === '') { out.pop(); continue; }
    }
    break;
  }
  if (!out.length) return;
  out.push({ s: sep, pre: false, sep: true });
};
// 批240 / blitz#924: innerText REFLECTS text-transform (Chrome: uppercase
// renders "ABC DEF", innerText reads it back transformed while textContent
// keeps the source). The computed-style layer is declared-only for this
// inherited prop, so the walk threads `st.tt` — a DECLARED value (including
// 'none', which resets) overrides, unset keeps the ancestor's.
const _itWordJoiner = (c) => {
  const cp = c.codePointAt(0);
  if (cp >= 0x30 && cp <= 0x39) return true; // ASCII digits join (`2abc` stays lowercase)
  if (c === '_' || c === "'" || c === '\u2019' || c === ':' || c === '\u00b7') return true;
  // Han ideographs + kana own their words — they break (`中文abc` caps the a).
  if ((cp >= 0x3040 && cp <= 0x30FF) || (cp >= 0x31F0 && cp <= 0x31FF) ||
      (cp >= 0x3400 && cp <= 0x4DBF) || (cp >= 0x4E00 && cp <= 0x9FFF) ||
      (cp >= 0xF900 && cp <= 0xFAFF) || (cp >= 0x20000 && cp <= 0x2A6DF)) return false;
  return /[\p{L}\p{Nd}]/u.test(c); // letters (Hangul/Latin/Greek/…) and Nd digits join
};
const _itCapitalize = (s) => {
  let out = '', joins = false;
  for (const c of s) {
    out += !joins && /\p{Ll}/u.test(c) ? c.toUpperCase() : c;
    joins = _itWordJoiner(c);
  }
  return out;
};
const _itApplyTransform = (s, tt) => {
  if (tt === 'uppercase') return s.toUpperCase();
  if (tt === 'lowercase') return s.toLowerCase();
  if (tt === 'capitalize') return _itCapitalize(s);
  return s;
};
const _innerTextCollect = (node, out, st) => {
  if (!node) return;
  const nt = node.nodeType;
  if (nt === 3 || nt === 4) {
    if (st.hidden) return;
    let s = String(node.data ?? '');
    if (!s) return;
    s = _itApplyTransform(s, st.tt);
    if (st.pre) { out.push({ s, pre: true }); return; }
    s = s.replace(/[ \t\r\n\f]+/g, ' ');
    if (out.length && out[out.length - 1].s.endsWith('\n')) s = s.replace(/^ +/, '');
    if (s) out.push({ s, pre: false });
    return;
  }
  if (nt !== 1) return;
  const el = node;
  const tag = el.localName;
  if (_IT_SKIP_TAGS.has(tag)) return;
  if (tag === 'br') { _itPushSep(out, '\n'); return; }
  const cs = _itCascade(el);
  let display = (el.style && el.style.display) || null;
  if (display == null && el.getAttribute && el.getAttribute('hidden') != null) display = 'none';
  if (display == null && cs && cs.display != null) display = String(cs.display);
  if (display === 'none') return;
  // Cells break with a tab (Chrome's cell separator), not a newline. Tag
  // shape covers the no-cascade walk; an explicit display (including
  // non-cell overrides on td/th) wins.
  const cell = display === 'table-cell' || (display == null && (tag === 'td' || tag === 'th'));
  const blockish = cell || (display != null
    ? _IT_BLOCK_DISPLAY.test(display)
    : _IT_BLOCK_TAGS.has(tag));
  const sep = cell ? '\t' : '\n';
  let visibility = cs && cs.visibility != null ? String(cs.visibility) : null;
  // Whether the visibility value is DECLARED (cascade answered, or inline
  // style carries one) rather than the walk's 'visible' default — only a
  // declared 'visible' restores text under a hidden ancestor (obscura#1106);
  // an undeclared child inherits whatever state it walked into.
  const visDeclared = visibility != null || !!(el.style && el.style.visibility);
  if (visibility == null) visibility = (el.style && el.style.visibility) || 'visible';
  // Cascade first, inline style as the fallback for elements the style
  // engine hasn't answered for — and in fetch-tier builds the cascade op
  // answers null for EVERYTHING (no layout stack in the binary), making the
  // inline read the only source. Same posture as text-transform below.
  // diting#79: pre*/break-spaces spacing must survive into innerText.
  let ws = cs && typeof cs['white-space'] === 'string' ? cs['white-space'] : '';
  if (!ws) ws = (el.style && el.style.whiteSpace) || '';
  let tt = cs && typeof cs['text-transform'] === 'string' ? cs['text-transform'] : null;
  if (tt == null) tt = (el.style && el.style.textTransform) || null;
  const prevHidden = st.hidden;
  const prevPre = st.pre;
  const prevTt = st.tt;
  if (visibility === 'hidden' || visibility === 'collapse') st.hidden = true;
  else if (visDeclared && visibility === 'visible') st.hidden = false;
  if (_IT_PRE_TAGS.has(tag) || ws.startsWith('pre') || ws === 'break-spaces') st.pre = true;
  if (tt) st.tt = tt;
  // Cells separate on CLOSE only — the previous cell's close already laid
  // the tab, an open-push would double it, and an empty cell still
  // contributes its own tab via its close (Chrome emits one per boundary).
  if (blockish && !cell) _itPushSep(out, sep);
  const kids = el.childNodes;
  for (let i = 0; i < kids.length; i++) _innerTextCollect(kids[i], out, st);
  if (blockish) _itPushSep(out, sep);
  st.hidden = prevHidden;
  st.pre = prevPre;
  st.tt = prevTt;
};

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_IT_PRE_TAGS', _IT_PRE_TAGS);
  d('_innerTextCollect', _innerTextCollect);
  d('_innerTextDisplay', _innerTextDisplay);
  d('_itCascade', _itCascade);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
