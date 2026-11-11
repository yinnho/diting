//! module: vttcue
//! layer: L2
//! exposes: __diting_cueState
//! requires-load: -
//! requires-call: legacy:DOMException

// issue #67 marginal-cost family (VTTCue): behavior only, moved out of the
// hand-written text_cue.js. The class shape — arity, the double /
// unrestricted double / DOMString preamble, the seven accessors and
// getCueAsHTML in Chrome's own order — is generated from the webvtt IDL in
// gen/vttcue.gen.js with NO generator change. Pinned by
// tools/webplatform/chrome/vttcue-truth-157.json (real Chrome 157):
//   ctor.nanEnd / ctor.order — endTime is unrestricted, so the preamble lets
//     NaN through and the "endTime is not a number" check runs here, after
//     all three arguments were converted.
//   ctor.own / ctor.et — no own properties; the instance is an EventTarget
//     (hls.js listens on cues) and a TextTrackCue (global_surface's
//     AUDIT_CHAIN links the prototype after it mints TextTrackCue).
//   set.* — enums ignore bad values; line/position take double|"auto";
//     position/size are range-checked [0, 100].
// The TextTrackCue members (id/startTime/endTime/pauseOnExit/track) stay in
// text_cue.js, which loads after global_surface mints TextTrackCue; it reads
// this module's state through the hidden __diting_cueState at CALL time.
'use strict';

const st = new WeakMap();
P.expose('__diting_cueState', st);
if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_cueState');
}

const VERTICAL = ['', 'rl', 'lr'], ALIGN = ['start', 'center', 'end', 'left', 'right'];
const setCtx = (prop) => "Failed to set the '" + prop + "' property on 'VTTCue'";
// LineAndPositionSetting = (double or AutoKeyword): a string must be "auto",
// an object is not a double either; everything else goes through double.
function autoOr(v, prop, fn) {
  if (typeof v === 'string' || (v !== null && typeof v === 'object')) {
    if (v === 'auto') return 'auto';
    P.fail(fn, "The provided value '" + String(v) + "' is not a valid enum value of type AutoKeyword.", setCtx(prop));
  }
  return P.conv.double(v, setCtx(prop), fn);
}
function range(n, prop) {
  if (n < 0 || n > 100) throw new DOMException(setCtx(prop) + ': The value provided (' + n + ') is outside the range [0, 100].', 'IndexSizeError');
  return n;
}
function S(target, fn) {
  const s = target !== null && typeof target === 'object' ? st.get(target) : undefined;
  if (!s) P.illegal(fn);
  return s;
}

exports.ctor = function ctor(iface, target, ctx, args, parsed) {
  if (Number.isNaN(parsed[1])) P.fail(ctor, 'The endTime is not a number.', ctx);
  st.set(target, {
    id: '', startTime: parsed[0], endTime: parsed[1], pauseOnExit: false,
    vertical: '', snapToLines: true, line: 'auto', position: 'auto', size: 100, align: 'center', text: parsed[2],
  });
};

exports.get = function get(iface, member, target) {
  return S(target, get)[member];
};

exports.set = function set(iface, member, target, v) {
  const s = S(target, set);
  switch (member) {
    case 'vertical': { const x = String(v); if (VERTICAL.indexOf(x) >= 0) s.vertical = x; return; }
    case 'align': { const x = String(v); if (ALIGN.indexOf(x) >= 0) s.align = x; return; }
    case 'snapToLines': s.snapToLines = Boolean(v); return;
    case 'line': s.line = autoOr(v, 'line', set); return;
    case 'position': { const x = autoOr(v, 'position', set); s.position = x === 'auto' ? x : range(x, 'position'); return; }
    case 'size': s.size = range(P.conv.double(v, setCtx('size'), set), 'size'); return;
    case 'text': s.text = P.conv.DOMString(v, setCtx('text'), set); return;
  }
};

// WebVTT cue text -> DOM (WebVTT §6.4 parsing, §6.5 node mapping): b/i/u/ruby/rt
// keep their names, c -> span.class, v -> span[title], lang -> span[lang];
// timestamp tags and unknown tags drop, character references decode.
const ENT = { amp: '&', lt: '<', gt: '>', lrm: '‎', rlm: '‏', nbsp: ' ', quot: '"', apos: "'" };
const decode = (s) => s.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[A-Za-z]+);/g, (m, e) => {
  if (e[0] === '#') { const c = e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10); return c > 0 && c <= 0x10ffff ? String.fromCodePoint(c) : m; }
  return Object.prototype.hasOwnProperty.call(ENT, e) ? ENT[e] : m;
});
const KEEP = { b: 'b', i: 'i', u: 'u', ruby: 'ruby', rt: 'rt', c: 'span', v: 'span', lang: 'span' };
function cueToFragment(text) {
  const doc = globalThis.document;
  const frag = doc.createDocumentFragment();
  const stack = [{ node: frag, tag: '' }];
  const re = /<([^>]*)>|([^<]+)/g;
  let m;
  while ((m = re.exec(text))) {
    const top = stack[stack.length - 1];
    if (m[2] !== undefined) { top.node.appendChild(doc.createTextNode(decode(m[2]))); continue; }
    const raw = m[1];
    if (raw[0] === '/') {
      const name = raw.slice(1).trim().split(/[.\s]/)[0];
      for (let i = stack.length - 1; i > 0; i--) if (stack[i].tag === name) { stack.length = i; break; }
      continue;
    }
    const head = raw.split(/[\s]/)[0];
    const [name, ...classes] = head.split('.');
    if (!Object.prototype.hasOwnProperty.call(KEEP, name)) continue;
    if (name === 'rt' && top.tag !== 'ruby') continue;
    const el = doc.createElement(KEEP[name]);
    const cls = classes.filter(Boolean);
    if (cls.length) el.className = cls.join(' ');
    const ann = raw.slice(head.length).trim();
    if (name === 'v') el.setAttribute('title', decode(ann));
    if (name === 'lang') el.setAttribute('lang', ann);
    top.node.appendChild(el);
    stack.push({ node: el, tag: name });
  }
  return frag;
}

exports.call = function call(iface, member, target) {
  // getCueAsHTML is the only VTTCue operation.
  return cueToFragment(S(target, call).text);
};
