// VTTCue / TextTrackCue behavior takeover. global_surface.js minted VTTCue as
// an inert stub whose constructor always threw "3 arguments required, but
// only 0 present." — even for `new VTTCue(0, 0, "")`. hls.js builds a cue at
// module top level (`new VTTCue(0, 0, "")`), so every page bundling it died
// before mounting (WeChat store, 2026-11-03). Same takeover shape as
// speech_gaps.js: the minted prototypes stay (shape/brand tables survive),
// only the constructor and member bodies are replaced; state lives in a
// closure WeakMap so instances own no properties (Chrome: none).
// Behavior per Chrome probes (getCueAsHTML, setters, ladders): startTime is a
// restricted double, endTime unrestricted (Infinity ok, NaN rejected by the
// constructor with its own message), line/position take double|"auto",
// position/size range [0, 100], vertical/align are enums whose bad values are
// ignored. Loaded after stub_members.js/stub_defaults.js (which install the
// accessors being replaced) — defineProperty on an existing key keeps its order.
(() => {
  const G = globalThis;
  const minted = G.VTTCue, TTC = G.TextTrackCue;
  if (typeof minted !== 'function' || !minted.prototype || typeof TTC !== 'function' || !TTC.prototype) return;
  const mark = G.__diting_markNative || ((f) => f);
  const pin = (f, n) => { Object.defineProperty(f, 'name', { value: n, configurable: true }); return mark(f); };
  const st = new WeakMap();
  const S = (self) => { const s = (self !== null && typeof self === 'object') ? st.get(self) : undefined; if (!s) throw new TypeError('Illegal invocation'); return s; };
  const VERTICAL = ['', 'rl', 'lr'], ALIGN = ['start', 'center', 'end', 'left', 'right'];
  const num = (v, fail) => { const n = Number(v); if (!Number.isFinite(n)) throw new TypeError(fail + 'The provided double value is non-finite.'); return n; };
  const setMsg = (prop, iface) => "Failed to set the '" + prop + "' property on '" + iface + "': ";
  const autoOr = (v, prop) => {
    if (typeof v === 'string') {
      if (v === 'auto') return 'auto';
      throw new TypeError(setMsg(prop, 'VTTCue') + "The provided value '" + v + "' is not a valid enum value of type AutoKeyword.");
    }
    if (v !== null && typeof v === 'object') throw new TypeError(setMsg(prop, 'VTTCue') + "The provided value '" + String(v) + "' is not a valid enum value of type AutoKeyword.");
    return num(v, setMsg(prop, 'VTTCue'));
  };
  const range = (n, prop) => {
    if (n < 0 || n > 100) throw new DOMException(setMsg(prop, 'VTTCue') + 'The value provided (' + n + ') is outside the range [0, 100].', 'IndexSizeError');
    return n;
  };

  function VTTCue(startTime, endTime, text) {
    const F = "Failed to construct 'VTTCue': ";
    if (!new.target) throw new TypeError(F + "Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    if (arguments.length < 3) throw new TypeError(F + '3 arguments required, but only ' + arguments.length + ' present.');
    const start = num(startTime, F);
    const end = Number(endTime);
    if (Number.isNaN(end)) throw new TypeError(F + 'The endTime is not a number.');
    const self = typeof G.EventTarget === 'function' ? Reflect.construct(G.EventTarget, [], new.target) : Object.create(new.target.prototype);
    st.set(self, {
      id: '', startTime: start, endTime: end, pauseOnExit: false,
      vertical: '', snapToLines: true, line: 'auto', position: 'auto', size: 100, align: 'center', text: String(text),
    });
    return self;
  }
  const P = minted.prototype;
  Object.defineProperty(VTTCue, 'length', { value: 3, configurable: true });
  Object.defineProperty(VTTCue, 'prototype', { value: P, writable: false, enumerable: false, configurable: false });
  Object.defineProperty(P, 'constructor', { value: VTTCue, writable: true, enumerable: false, configurable: true });
  Object.setPrototypeOf(VTTCue, TTC);
  mark(VTTCue);
  if (G.__diting_stubs) G.__diting_stubs.add(VTTCue);
  Object.defineProperty(G, 'VTTCue', { value: VTTCue, writable: true, enumerable: false, configurable: true });

  // accessor (re)definition keeping each key's current flags and position
  const acc = (obj, k, get, set) => {
    const d = Object.getOwnPropertyDescriptor(obj, k) || { enumerable: true, configurable: true };
    Object.defineProperty(obj, k, { get: pin(get, 'get ' + k), set: set ? pin(set, 'set ' + k) : undefined, enumerable: d.enumerable, configurable: d.configurable });
  };
  const TP = TTC.prototype;
  acc(TP, 'track', function () { S(this); return null; });
  acc(TP, 'id', function () { return S(this).id; }, function (v) { S(this).id = String(v); });
  acc(TP, 'startTime', function () { return S(this).startTime; }, function (v) { const s = S(this); s.startTime = num(v, setMsg('startTime', 'TextTrackCue')); });
  acc(TP, 'endTime', function () { return S(this).endTime; }, function (v) {
    const s = S(this), n = Number(v);
    if (Number.isNaN(n)) throw new TypeError(setMsg('endTime', 'TextTrackCue') + 'The provided double value is non-finite.');
    s.endTime = n;
  });
  acc(TP, 'pauseOnExit', function () { return S(this).pauseOnExit; }, function (v) { S(this).pauseOnExit = Boolean(v); });

  acc(P, 'vertical', function () { return S(this).vertical; }, function (v) { const s = S(this), x = String(v); if (VERTICAL.indexOf(x) >= 0) s.vertical = x; });
  acc(P, 'snapToLines', function () { return S(this).snapToLines; }, function (v) { S(this).snapToLines = Boolean(v); });
  acc(P, 'line', function () { return S(this).line; }, function (v) { const s = S(this); s.line = autoOr(v, 'line'); });
  acc(P, 'position', function () { return S(this).position; }, function (v) { const s = S(this), x = autoOr(v, 'position'); s.position = x === 'auto' ? x : range(x, 'position'); });
  acc(P, 'size', function () { return S(this).size; }, function (v) { const s = S(this); s.size = range(num(v, setMsg('size', 'VTTCue')), 'size'); });
  acc(P, 'align', function () { return S(this).align; }, function (v) { const s = S(this), x = String(v); if (ALIGN.indexOf(x) >= 0) s.align = x; });
  acc(P, 'text', function () { return S(this).text; }, function (v) { S(this).text = String(v); });

  // WebVTT cue text -> DOM (WebVTT §6.4 parsing, §6.5 node mapping): b/i/u/ruby/rt
  // keep their names, c -> span.class, v -> span[title], lang -> span[lang];
  // timestamp tags and unknown tags drop, character references decode.
  const ENT = { amp: '&', lt: '<', gt: '>', lrm: '‎', rlm: '‏', nbsp: ' ', quot: '"', apos: "'" };
  const decode = (s) => s.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[A-Za-z]+);/g, (m, e) => {
    if (e[0] === '#') { const c = e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10); return c > 0 && c <= 0x10ffff ? String.fromCodePoint(c) : m; }
    return Object.prototype.hasOwnProperty.call(ENT, e) ? ENT[e] : m;
  });
  const KEEP = { b: 'b', i: 'i', u: 'u', ruby: 'ruby', rt: 'rt', c: 'span', v: 'span', lang: 'span' };
  const cueToFragment = (text) => {
    const doc = G.document;
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
  };
  const gch = function getCueAsHTML() { return cueToFragment(S(this).text); };
  const gd = Object.getOwnPropertyDescriptor(P, 'getCueAsHTML') || { enumerable: true, configurable: true, writable: true };
  Object.defineProperty(gch, 'length', { value: 0, configurable: true });
  Object.defineProperty(P, 'getCueAsHTML', { value: pin(gch, 'getCueAsHTML'), enumerable: gd.enumerable, configurable: gd.configurable, writable: gd.writable });
})();
