(function () { // @@SCOPE
// god-file split (刀31, window named access #48): elements with an id or
// (form/iframe/embed/object/img) a name attribute are reachable as window
// properties. The _namedHolder IIFE runs at LOAD: parks the gate proxy
// under globalThis (setPrototypeOf) + installs the hidden
// __diting_namedHolder member — window_extras.js's EventTarget rewire
// reads that member at its later load and re-points the layer. Load-time
// semantics are order-free here: named getters only appear at RUNTIME
// (_namedTrack via mutation hooks), so the gate filters an empty set
// during every platform file's load. Consumers, all RUNTIME: domcore.js
// insertion/removal/attr paths call the scan/unscan walkers; Rust entry
// points (eval wrapper, callFunctionOn, page scripts) call
// globalThis._namedBoot — analyzer-blind, pinned in exports-extra so the
// d() publish stays; _namedScanned is a mutable let -> a() accessor.
// ---- (#48) Window named access: window[id] / bare `id` identifier ----
// HTML5 named access on the Window object: elements with an id (any tag) or
// a name attribute (form/iframe/embed/object/img — the spec's shortlist)
// are reachable as window properties. Lives as accessor get-traps so reads
// reflect the tree at read time. Shadowing per spec: real own globals win —
// an explicit var/function declaration or assignment redefines our
// configurable slot into a data property, which is Chrome's resolution
// order (declaration > named access). Names in array-index form never
// become named properties.
const _namedEls = new Map();     // name -> Set<nid>, insertion order
const _namedGetters = new Map(); // name -> installed get fn (identity check)
let _namedScanned = false;       // one lazy full scan for the parser-built tree
// Chrome resolves named access through a WindowProperties layer between
// Window.prototype and EventTarget.prototype (final chain after rewire
// below): window.q works, but the name NEVER shows in
// getOwnPropertyNames(window) — the winKeys face only counts real globals.
// Own getters on globalThis leak every element id/name into that face, and
// a getter is never uninstalled, so the face only grows across a page's
// life. Park the getters on a proto layer: get/in/bare-identifier all
// walk the chain, page own props still shadow, and gopn(window) stays
// clean. Early install inserts the gate under globalThis temporarily
// (Window/EventTarget do not exist yet); the EventTarget rewire later
// moves it to Chrome order: window → Window.prototype → gate → ET.
// Chrome's WindowProperties layer is also EMPTY to its own enumeration
// (gopn at the layer = 0 entries — verified 10-17): resolution is pure
// interception, not own descriptors. A raw holder would leak every
// installed name to a proto-walking fingerprint (ark-class VMs walk the
// chain), so the layer is a gate that hides ONLY the named installs —
// ownKeys and gOPD return them as absent, everything else (page
// defineProperty on the layer) passes through untouched. get/has/
// defineProperty forward raw, so resolution, `in`, and foreign
// relocations behave exactly as before.
const _namedHolder = (function () {
  try {
    const P0 = Object.getPrototypeOf(globalThis);
    if (!P0) return globalThis; // defensive: no proto layer to park on
    const h = Object.create(P0);
    const gate = new Proxy(h, {
      ownKeys(t) { return Reflect.ownKeys(t).filter((k) => !_namedGetters.has(k)); },
      getOwnPropertyDescriptor(t, k) {
        if (_namedGetters.has(k)) return undefined;
        return Reflect.getOwnPropertyDescriptor(t, k);
      },
    });
    Object.setPrototypeOf(globalThis, gate);
    Object.defineProperty(globalThis, '__diting_namedHolder', { value: h, writable: true, enumerable: false, configurable: true });
    return h; // installs/uninstalls address the raw target
  } catch (e) { return globalThis; }
})();
function _namedNames(el) {
  if (!el || el.nodeType !== 1) return [];
  const out = [];
  const id = el.id;
  if (id) out.push(id);
  const t = el.tagName;
  if (t === 'FORM' || t === 'IFRAME' || t === 'EMBED' || t === 'OBJECT' || t === 'IMG') {
    const n = el.getAttribute('name');
    if (n) out.push(n);
  }
  return out;
}
function _namedInstall(name) {
  // Idempotence short-circuit: if our getter still owns the slot, the
  // defineProperty (plus fresh get/set closures) is pure waste — id/name
  // rewrite storms re-track the same names constantly.
  const cur = _namedGetters.get(name);
  const d = Object.getOwnPropertyDescriptor(_namedHolder, name);
  if (cur && d && d.get === cur) return;
  // A foreign own slot on the holder (the relocated EventTarget trio,
  // constructor, a page defineProperty on the layer, …) keeps the slot —
  // real faces win over named access, Chrome's resolution order.
  if (d && d.get !== cur) return;
  const get = () => _namedLookup(name);
  const set = (v) => {
    // Assignment lands as an own data prop on window (Chrome: named props
    // don't intercept [[Set]]), shadowing the named access from now on.
    try {
      Object.defineProperty(globalThis, name, { value: v, writable: true, configurable: true, enumerable: true });
    } catch (e) {}
  };
  _namedGetters.set(name, get);
  try {
    Object.defineProperty(_namedHolder, name, { get, set, configurable: true, enumerable: true });
  } catch (e) { _namedGetters.delete(name); }
}
function _namedTrack(el) {
  if (!el || el.nodeType !== 1) return;
  for (const n of _namedNames(el)) {
    // Array-index names never become named properties (spec).
    if (!n || /^\d+$/.test(n)) continue;
    let s = _namedEls.get(n);
    if (s) {
      // Already tracking this element under this name — the rewrite storm
      // re-tracks constantly; only a fresh (element, name) pair installs.
      if (s.has(el._nid)) continue;
      s.add(el._nid);
    } else {
      s = new Set(); _namedEls.set(n, s); s.add(el._nid);
    }
    _namedInstall(n);
  }
  _docTrack(el);
}
function _namedUntrackBy(nid, names) {
  for (const n of names) {
    const s = _namedEls.get(n);
    if (s) {
      s.delete(nid);
      if (!s.size) {
        _namedEls.delete(n);
        // Live-tree reflection: with no bearer left, pull the getter off
        // the holder so the named layer shrinks with the DOM (own props
        // on globalThis keep shadowing anything a page stored earlier).
        _namedGetters.delete(n);
        try { delete _namedHolder[n]; } catch (e) {}
      }
    }
  }
  if (_docEls.size) _docUntrackBy(nid, names);
}
function _namedUntrack(el) {
  if (!el || el.nodeType !== 1) return;
  _namedUntrackBy(el._nid, _namedNamesAll(el));
}
function _namedLookup(name) {
  const pick = () => {
    const s = _namedEls.get(name);
    if (!s) return undefined;
    for (const nid of s) {
      const el = _wrapEl(+nid);
      if (el && el.nodeType === 1 && el.isConnected) return el;
      s.delete(nid); // dead nid (innerHTML rebuilt the subtree)
    }
    if (!s.size) _namedEls.delete(name);
    return undefined;
  };
  // HTML spec named property access: an iframe (via id OR name) exposes its
  // nested browsing context — the WindowProxy, never the element itself.
  // window.frames === window, so frames['x'] lands here too (#48 face;
  // moli's 2026-10 cross-origin named-child cluster is the same surface).
  const unbox = (el) => {
    if (el && el.tagName === 'IFRAME') {
      try {
        const w = el.contentWindow;
        if (w) return w;
      } catch (e) {}
    }
    return el;
  };
  let el = pick();
  if (el) return unbox(el);
  if (!_namedScanned) {
    // Parser-built trees never pass through the mutation hooks, so the
    // first named miss pays one full scan. Later inserts/removals/attr
    // writes all go through the hooks, which keep the registry current.
    _namedScanned = true;
    try { _namedScanIn(globalThis.document); } catch (e) {}
    el = pick();
  }
  return unbox(el);
}
// #48: Chrome resolves bare identifiers and window[id] to id/name elements
// BEFORE any script runs — there is no "first touch" moment. The scan above
// piggybacks on lazy paths (first named miss, first `document` touch), but a
// script whose FIRST statement is a bare identifier (`hero.focus()`) reaches
// for the name before either lazy path can fire. So every Rust entry point
// that runs foreign code — the eval wrapper (runtime.rs INLINE_EVAL_JS, agent
// evaluate + CDP evaluate), callFunctionOn's wrapper, and the page script
// loop (page/scripts.rs) — calls this first. Touching `document` runs the
// getter above, which materializes the tree and pays the scan; if no DOM is
// attached yet the getter returns null WITHOUT setting the flag, so later
// calls retry. After the first success this is a one-flag no-op.
function _namedBoot() {
  if (!_namedScanned) globalThis.document;
}
function _namedScanIn(root) {
  if (!root) return;
  _namedTrack(root);
  // The document (nodeType 9) scans globally; an element scans its subtree.
  if ((root.nodeType === 1 || root.nodeType === 9) && typeof root.querySelectorAll === 'function') {
    const els = root.querySelectorAll('[id], form[name], iframe[name], embed[name], object[name], img[name]');
    for (let i = 0; i < els.length; i++) _namedTrack(els[i]);
  }
}
function _namedUnscanIn(root) { // removal: walk live children (root may be detached)
  if (!root) return;
  _namedUntrack(root);
  if (root.nodeType === 1) {
    const kids = root.childNodes || [];
    for (let i = 0; i < kids.length; i++) {
      if (kids[i] && kids[i].nodeType === 1) _namedUnscanIn(kids[i]);
    }
  }
}

// ---- (#16) Document named access: document.<name> ----
// Chrome truth (headless probe 2026-11-05, issue #16): supported names come
// from the name attribute of applet/embed/form/iframe/img/object, plus the
// id of any element that ALSO carries a name — a bare id never resolves on
// document (that's the WINDOW half of the family). HTMLDocument is
// [LegacyOverrideBuiltIns]: a bearing element shadows PROTOTYPE members
// (document.forms with <img name=forms> reads the img) while real OWN props
// (location — [LegacyUnforgeable], page assignments) win. The exposed face
// is a plain own VALUE property (gOPD shows value+writable+enumerable+
// configurable all true; gopn/Object.keys/for-in list it), so this half
// installs data props stamped by the mutation hooks instead of getters —
// reads need no lazy path and the descriptor shape matches Chrome exactly.
// Divergences kept deliberately: page writes over a named slot land and
// stick until the next hook re-stamps (Chrome absorbs the write forever —
// one slot cannot be both a value descriptor and write-proof in plain JS).
// delete removes our prop and it stays removed until a NEW bearer enters —
// same as Chrome.
const _docEls = new Map();        // name -> Set<nid>, insertion order
const _docInstalled = new Set();  // names this file owns on the document
const _DOC_NAMED_TAGS = { APPLET: 1, EMBED: 1, FORM: 1, IFRAME: 1, IMG: 1, OBJECT: 1 };
function _docNames(el) {
  if (!el || el.nodeType !== 1) return [];
  const out = [];
  if (_DOC_NAMED_TAGS[el.tagName]) {
    const n = el.getAttribute('name');
    if (n) {
      out.push(n);
      // id contributes only when a name is also present (HTML spec,
      // supported property names item 2).
      const id = el.id;
      if (id && id !== n) out.push(id);
    }
  }
  return out;
}
function _namedNamesAll(el) { // union of both registries' names (untrack snapshots)
  const a = _namedNames(el), b = _docNames(el);
  if (!b.length) return a;
  if (!a.length) return b;
  return [...new Set(a.concat(b))];
}
function _docStamp(name) {
  const doc = globalThis.document;
  if (!doc || doc.nodeType !== 9) return;
  const s = _docEls.get(name);
  const bearers = [];
  if (s) {
    for (const nid of s) {
      const el = _wrapEl(+nid);
      if (el && el.nodeType === 1 && el.isConnected) bearers.push(el);
      else s.delete(nid); // dead nid (innerHTML rebuilt the subtree)
    }
    if (!s.size) _docEls.delete(name);
  }
  const ours = _docInstalled.has(name);
  if (!bearers.length) {
    if (ours) { _docInstalled.delete(name); try { delete doc[name]; } catch (e) {} }
    return;
  }
  // Iframe bearers expose their nested browsing context (document.ifr ===
  // window.ifr, both probes); if the frame's window isn't materialized at
  // stamp time the element itself stands in until a later hook re-stamps.
  let value;
  if (bearers.length === 1) {
    value = bearers[0];
    if (value.tagName === 'IFRAME') {
      try { const w = value.contentWindow; if (w) value = w; } catch (e) {}
    }
  } else {
    value = __diting_mkCollection(bearers); // tree order approximated by insertion order
  }
  // Real own slots win: location, page assignments, engine internals keep
  // theirs and no named prop is installed over them (Chrome's resolution).
  if (!ours && Object.prototype.hasOwnProperty.call(doc, name)) return;
  try {
    Object.defineProperty(doc, name, { value, writable: true, enumerable: true, configurable: true });
    _docInstalled.add(name);
  } catch (e) {}
}
function _docTrack(el) {
  for (const n of _docNames(el)) {
    if (!n || /^\d+$/.test(n)) continue; // array-index names never resolve (spec)
    let s = _docEls.get(n);
    if (s) {
      // Known (element, name) with a live prop: nothing changed. The
      // NOT-installed case still stamps — that's a detached registration
      // whose element just connected (subtree attach re-tracks children).
      if (s.has(el._nid) && _docInstalled.has(n)) continue;
      s.add(el._nid);
    } else {
      s = new Set(); _docEls.set(n, s); s.add(el._nid);
    }
    _docStamp(n);
  }
}
function _docUntrackBy(nid, names) {
  for (const n of names) {
    const s = _docEls.get(n);
    if (!s || !s.has(nid)) continue;
    s.delete(nid);
    if (!s.size) _docEls.delete(n);
    _docStamp(n);
  }
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_docEls', _docEls);
  d('_namedBoot', _namedBoot);
  d('_namedEls', _namedEls);
  d('_namedNamesAll', _namedNamesAll);
  d('_namedScanIn', _namedScanIn);
  a('_namedScanned', () => _namedScanned, (v) => { _namedScanned = v; });
  d('_namedTrack', _namedTrack);
  d('_namedUnscanIn', _namedUnscanIn);
  d('_namedUntrack', _namedUntrack);
  d('_namedUntrackBy', _namedUntrackBy);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
