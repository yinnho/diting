//! module: docmisc
//! layer: L2
//! exposes: CaretPosition, FragmentDirective
//! requires-load: legacy:Document, legacy:HTMLCollection
//! requires-call: legacy:Element, legacy:DOMRect, legacy:document

// Document legacy/misc members (M4 knife 20). None of these existed on
// Document.prototype - and because they were absent, page assignments such as
// `document.bgColor = 'red'` or `document.fullscreen = true` silently minted
// own data properties on the document (fingerprint-visible, and the "value" then
// read back as whatever the page wrote). Pinned by chrome/docmisc-truth-145.json
// (docmisc-probe.js, 36 keys) and diffed by docmisc_matches_chrome_truth.
//
// Shape: the members are installed in Chrome's relative order. The five colour
// attributes reflect the body's legacy attributes (bgcolor/text/link/vlink/alink,
// null -> ''), anchors/embeds/plugins are LIVE collections with stable identity
// (embeds === plugins, anchors = a[name] by attribute presence), designMode
// normalizes on/off case-insensitively and ignores everything else, the fullscreen
// and pointer-lock faces report the idle state (fullscreen*, rw descriptors whose
// setters ignore writes), and the privacy members are promise-returning with
// argument-count rejection.

const isDoc = (o) => o !== null && typeof o === 'object' && o instanceof Document;
const D = Document.prototype;
let designMode = 'off';

function live(selector) {
  const fresh = () => HTMLCollection._from(document.querySelectorAll(selector));
  return new Proxy(HTMLCollection._from([]), {
    get(t, k) { const c = fresh(); const v = Reflect.get(c, k); return typeof v === 'function' ? v.bind(c) : v; },
    has(t, k) { return Reflect.has(fresh(), k); },
    ownKeys() { return Reflect.ownKeys(fresh()); },
    getOwnPropertyDescriptor(t, k) { const d = Reflect.getOwnPropertyDescriptor(fresh(), k); if (d) d.configurable = true; return d; },
  });
}
const cache = {};
const coll = (name, sel) => cache[name] || (cache[name] = live(sel));

function pad(n) { return String(n).padStart(2, '0'); }
function body() { return document.body; }
function colorAttr(name, attr, order) {
  P.accessor(D, name, function __f() {
    if (!isDoc(this)) P.illegal(__f);
    const b = body();
    const v = b ? b.getAttribute(attr) : null;
    return v === null ? '' : v;
  }, function __f(v) {
    if (!isDoc(this)) P.illegal(__f);
    const s = v === null ? '' : P.conv.DOMString(v, "Failed to set the '" + name + "' property on 'Document'", __f);
    const b = body();
    if (b) b.setAttribute(attr, s);
  });
}
const ro = (name, fn) => P.getter(D, name, function __f() { if (!isDoc(this)) P.illegal(__f); return fn(); });
const meth = (name, len, fn) => P.method(D, name, len, function __f() { if (!isDoc(this)) P.illegal(__f); return fn.apply(this, arguments); });
const noopSet = (name, get) => P.accessor(D, name, function __f() { if (!isDoc(this)) P.illegal(__f); return get(); }, function __f() { if (!isDoc(this)) P.illegal(__f); });

// ---- installed in Chrome's relative order
ro('lastModified', () => {
  const d = new Date();
  return pad(d.getMonth() + 1) + '/' + pad(d.getDate()) + '/' + d.getFullYear() + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
});
ro('embeds', () => coll('embeds', 'embed'));
ro('plugins', () => coll('embeds', 'embed'));
P.accessor(D, 'designMode', function __f() { if (!isDoc(this)) P.illegal(__f); return designMode; }, function __f(v) {
  if (!isDoc(this)) P.illegal(__f);
  const s = P.conv.DOMString(v, "Failed to set the 'designMode' property on 'Document'", __f).toLowerCase();
  if (s === 'on' || s === 'off') designMode = s;
});
ro('anchors', () => coll('anchors', 'a[name]'));
ro('applets', () => cache.applets || (cache.applets = HTMLCollection._from([])));
colorAttr('fgColor', 'text');
colorAttr('linkColor', 'link');
colorAttr('vlinkColor', 'vlink');
colorAttr('alinkColor', 'alink');
colorAttr('bgColor', 'bgcolor');
ro('wasDiscarded', () => false);
ro('prerendering', () => false);
noopSet('fullscreenEnabled', () => true);
noopSet('fullscreen', () => false);
ro('rootElement', () => null);
ro('pointerLockElement', () => null);
noopSet('fullscreenElement', () => null);
meth('captureEvents', 0, () => undefined);

// CaretPosition: hit-tests with elementFromPoint; the engine has no inline layout caret model, so the
// position is the start of the element under the point.
function CaretPosition() { P.fail(CaretPosition, 'Illegal constructor', "Failed to construct 'CaretPosition'"); }
const CP = P.iface(CaretPosition, 'CaretPosition', 0);
const cps = new WeakMap();
const cslot = (o, fn) => { const s = o !== null && typeof o === 'object' ? cps.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; };
P.getter(CP, 'offsetNode', function __f() { return cslot(this, __f).node; });
P.getter(CP, 'offset', function __f() { return cslot(this, __f).offset; });
P.method(CP, 'getClientRect', 0, function __f() {
  const s = cslot(this, __f);
  const el = s.node && s.node.nodeType === 1 ? s.node : s.node && s.node.parentElement;
  return el && el.getBoundingClientRect ? DOMRect.fromRect(el.getBoundingClientRect()) : null;
});
P.seal(CaretPosition);
meth('caretPositionFromPoint', 2, function () {
  P.args("Failed to execute 'caretPositionFromPoint' on 'Document'", arguments.length, 2, this.caretPositionFromPoint);
  const x = Number(arguments[0]), y = Number(arguments[1]);
  const el = document.elementFromPoint(x, y);
  if (el === null || el === undefined) return null;
  const c = Object.create(CP);
  cps.set(c, { node: el, offset: 0 });
  return c;
});
meth('clear', 0, () => undefined);
meth('exitFullscreen', 0, function () {
  return Promise.reject(new TypeError("Failed to execute 'exitFullscreen' on 'Document': Document not active"));
});
meth('exitPointerLock', 0, () => undefined);
meth('hasUnpartitionedCookieAccess', 0, () => Promise.resolve(true));
meth('releaseEvents', 0, () => undefined);

function FragmentDirective() { P.fail(FragmentDirective, 'Illegal constructor', "Failed to construct 'FragmentDirective'"); }
P.iface(FragmentDirective, 'FragmentDirective', 0);
P.seal(FragmentDirective);
let fd = null;
ro('fragmentDirective', () => fd || (fd = Object.create(FragmentDirective.prototype)));
function privacy(name) {
  P.method(D, name, 1, function __f() {
    if (!isDoc(this)) P.illegal(__f);
    if (arguments.length < 1) return Promise.reject((() => { try { P.fail(__f, '1 argument required, but only 0 present.', "Failed to execute '" + name + "' on 'Document'"); } catch (e) { return e; } })());
    return Promise.resolve(false);
  });
}
privacy('hasPrivateToken');
privacy('hasRedemptionRecord');

P.expose('CaretPosition', CaretPosition);
P.expose('FragmentDirective', FragmentDirective);
