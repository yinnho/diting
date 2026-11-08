//! module: intersection
//! layer: L2
//! exposes: IntersectionObserver, IntersectionObserverEntry
//! requires-load: legacy:DOMRectReadOnly, legacy:Element, legacy:Document
//! requires-call: legacy:DOMException, legacy:performance, legacy:setTimeout, legacy:MutationObserver, legacy:document, legacy:innerWidth, legacy:innerHeight, legacy:__diting_reportUncaughtError

// IntersectionObserver + IntersectionObserverEntry (M4 knife 19). The bootstrap
// class carried an underscore-method face (_fireFor in the prototype, constructor
// length 2, hand-picked getters), delivered plain object literals and exported
// an empty IntersectionObserverEntry. Chrome 145: the entry's constructor is
// illegal and its eight members are prototype getters over a private slot;
// the observer validates callback/options with exact wording, normalizes
// rootMargin/scrollMargin to four tokens, keeps thresholds as sorted float32 and
// hands out a fresh frozen copy per read. Pinned by chrome/ioentry-truth-145.json
// (ioentry-probe.js, 31 keys) and diffed by intersection_matches_chrome_truth.
//
// BEHAVIOUR POLICY (unchanged, moved from bootstrap.js): without a layout
// engine every observed target is treated as fully in the viewport
// (isIntersecting true, ratio 1). Real libraries lean on this in three patterns:
//   1. Lazy load: observe(img) -> first intersection -> load src -> unobserve.
//   2. Infinite scroll: observe(sentinel) -> load more -> new sentinel mounts.
//      Needs re-fires as the DOM grows.
//   3. Reveal-on-scroll animations: one fire is enough.
// To cover (2) without spinning forever, observe() burst-fires on an exponential
// backoff and every DOM mutation re-fires all live observers (debounced by a
// microtask); a per-observer cap stops a never-disconnected observer looping.

const slots = new WeakMap();
const eslots = new WeakMap();
const live = [];
const MAX_FIRES = 256;

// ---------------------------------------------------------------- entry
function IntersectionObserverEntry() {
  if (new.target === undefined) P.fail(IntersectionObserverEntry, 'Illegal constructor');
  P.fail(IntersectionObserverEntry, 'Illegal constructor', "Failed to construct 'IntersectionObserverEntry'");
}
const EP = P.iface(IntersectionObserverEntry, 'IntersectionObserverEntry', 0);
function eslot(o, fn) { const s = o !== null && typeof o === 'object' ? eslots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
for (const m of ['time', 'rootBounds', 'boundingClientRect', 'intersectionRect', 'isIntersecting', 'isVisible', 'intersectionRatio', 'target']) {
  P.getter(EP, m, function __f() { return eslot(this, __f)[m]; });
}
P.seal(IntersectionObserverEntry);
const ro = (r) => (r === null || r === undefined ? null : DOMRectReadOnly.fromRect(r));
function mkEntry(init) {
  const e = Object.create(EP);
  eslots.set(e, {
    time: performance.now(), rootBounds: ro(init.rootBounds), boundingClientRect: ro(init.boundingClientRect),
    intersectionRect: ro(init.intersectionRect), isIntersecting: init.isIntersecting, isVisible: false,
    intersectionRatio: init.intersectionRatio, target: init.target,
  });
  return e;
}

// ---------------------------------------------------------------- observer
const PRE = "Failed to construct 'IntersectionObserver'";
function parseMargin(v, fn) {
  const toks = String(v).trim().split(/\s+/);
  if (toks.length < 1 || toks.length > 4 || !toks.every((t) => /^-?(\d+\.?\d*|\.\d+)(px|%)$/.test(t))) {
    throw new DOMException(PRE + ': rootMargin must be specified in pixels or percent.', 'SyntaxError');
  }
  const [t, r = t, b = t, l = r] = toks;
  return [t, r, b, l].join(' ');
}
function rangeFail(msg) {
  const e = new RangeError(msg);
  Object.defineProperty(e, 'message', { value: PRE + ': ' + msg, writable: true, enumerable: true, configurable: true });
  throw e;
}
function dbl(v, prefix, fn) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', prefix);
  const n = Number(v);
  if (!isFinite(n)) P.fail(fn, 'The provided double value is non-finite.', prefix);
  return n;
}
function IntersectionObserver(callback) {
  if (new.target === undefined) P.fail(IntersectionObserver, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", PRE);
  P.args(PRE, arguments.length, 1, IntersectionObserver);
  if (typeof callback !== 'function') P.fail(IntersectionObserver, "parameter 1 is not of type 'Function'.", PRE);
  let init = arguments[1];
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') P.fail(IntersectionObserver, "The provided value is not of type 'IntersectionObserverInit'.", PRE);
  const o = init === undefined ? {} : init;
  const rd = (k) => PRE + ": Failed to read the '" + k + "' property from 'IntersectionObserverInit'";
  // dictionary members in lexicographic order
  const delay = o.delay === undefined ? 0 : dbl(o.delay, rd('delay'), IntersectionObserver);
  let root = null;
  if (o.root !== undefined && o.root !== null) {
    if (!(o.root instanceof Element) && !(o.root instanceof Document)) P.fail(IntersectionObserver, "The provided value is not of type '(Document or Element)'.", rd('root'));
    root = o.root;
  }
  const rootMargin = o.rootMargin === undefined ? '0px 0px 0px 0px' : parseMargin(P.conv.DOMString(o.rootMargin, rd('rootMargin'), IntersectionObserver), IntersectionObserver);
  const scrollMargin = o.scrollMargin === undefined ? '0px 0px 0px 0px' : parseMargin(P.conv.DOMString(o.scrollMargin, rd('scrollMargin'), IntersectionObserver), IntersectionObserver);
  let ths = [0];
  if (o.threshold !== undefined) {
    const raw = typeof o.threshold === 'object' && o.threshold !== null && typeof o.threshold[Symbol.iterator] === 'function' ? Array.from(o.threshold) : [o.threshold];
    ths = raw.map((v) => dbl(v, rd('threshold'), IntersectionObserver));
    for (const v of ths) if (!(v >= 0 && v <= 1)) rangeFail('Threshold values must be numbers between 0 and 1');
    ths = ths.map(Math.fround).sort((a, b) => a - b);
    if (ths.length === 0) ths = [0];
  }
  const trackVisibility = o.trackVisibility === undefined ? false : Boolean(o.trackVisibility);
  const self = Reflect.construct(Object, [], new.target);
  slots.set(self, { callback, root, rootMargin, scrollMargin, thresholds: ths, delay, trackVisibility, targets: new Set(), connected: true, fires: 0 });
  live.push(self);
  return self;
}
const IP = P.iface(IntersectionObserver, 'IntersectionObserver', 1);
function slot(o, fn) { const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
P.getter(IP, 'root', function __f() { return slot(this, __f).root; });
P.getter(IP, 'rootMargin', function __f() { return slot(this, __f).rootMargin; });
P.getter(IP, 'scrollMargin', function __f() { return slot(this, __f).scrollMargin; });
P.getter(IP, 'thresholds', function __f() { return Object.freeze(slot(this, __f).thresholds.slice()); });
P.getter(IP, 'delay', function __f() { return slot(this, __f).delay; });
P.getter(IP, 'trackVisibility', function __f() { return slot(this, __f).trackVisibility; });

function fire(self, s, targets) {
  if (!s.connected || !targets.length || s.fires >= MAX_FIRES) return;
  s.fires++;
  const vw = globalThis.innerWidth || 1280, vh = globalThis.innerHeight || 720;
  const records = targets.map((target) => {
    const r = target.getBoundingClientRect ? target.getBoundingClientRect() : { x: 0, y: 0, width: 100, height: 20 };
    return mkEntry({ target, isIntersecting: true, intersectionRatio: 1, boundingClientRect: r, intersectionRect: r, rootBounds: { x: 0, y: 0, width: vw, height: vh } });
  });
  try { s.callback.call(self, records, self); } catch (e) { globalThis.__diting_reportUncaughtError(e); }
}
P.method(IP, 'disconnect', 0, function __f() {
  const s = slot(this, __f);
  s.connected = false;
  s.targets.clear();
  const i = live.indexOf(this);
  if (i >= 0) live.splice(i, 1);
});
P.method(IP, 'observe', 1, function __f(el) {
  const s = slot(this, __f);
  P.args("Failed to execute 'observe' on 'IntersectionObserver'", arguments.length, 1, __f);
  if (!(el instanceof Element)) P.fail(__f, "parameter 1 is not of type 'Element'.", "Failed to execute 'observe' on 'IntersectionObserver'");
  if (!s.connected || s.targets.has(el)) return;
  s.targets.add(el);
  const self = this;
  Promise.resolve().then(() => fire(self, s, [el]));
  // exponential burst: covers infinite-scroll sentinels that re-arm after content lands
  [120, 500, 1500, 3500, 7000].forEach((delay) => {
    setTimeout(() => { if (s.connected && s.targets.has(el)) fire(self, s, [el]); }, delay);
  });
});
P.method(IP, 'takeRecords', 0, function __f() { slot(this, __f); return []; });
P.method(IP, 'unobserve', 1, function __f(el) {
  const s = slot(this, __f);
  P.args("Failed to execute 'unobserve' on 'IntersectionObserver'", arguments.length, 1, __f);
  if (!(el instanceof Element)) P.fail(__f, "parameter 1 is not of type 'Element'.", "Failed to execute 'unobserve' on 'IntersectionObserver'");
  s.targets.delete(el);
});
P.seal(IntersectionObserver);

// DOM growth re-fires every live observer (debounced by a microtask)
(function () {
  let pending = false;
  const reFire = () => {
    for (const obs of live.slice()) {
      const s = slots.get(obs);
      if (!s.connected) continue;
      const ts = [...s.targets];
      if (ts.length) fire(obs, s, ts);
    }
  };
  const wireUp = () => {
    if (!globalThis.document || !globalThis.document.body) return;
    const mo = new MutationObserver(() => {
      if (pending) return;
      pending = true;
      Promise.resolve().then(() => { pending = false; reFire(); });
    });
    try { mo.observe(globalThis.document.body, { childList: true, subtree: true }); } catch (e) {}
  };
  if (globalThis.document && globalThis.document.body) wireUp(); else Promise.resolve().then(wireUp);
})();

P.expose('IntersectionObserver', IntersectionObserver);
P.expose('IntersectionObserverEntry', IntersectionObserverEntry);
