(function () { // @@SCOPE
// god-file split (刀30, custom elements): the CustomElementRegistry class
// (define/get/getName/upgrade/whenDefined, prototype-swap _upgradeElement)
// plus the dynamic upgrade walks — elements appearing AFTER define()
// (createElement+append, innerHTML markup, detached-then-attached subtrees)
// get their constructor + connectedCallback via the insertion hook, and
// removal mirrors it with disconnectedCallback. The singleton install runs
// at load (constructor calls __hideOwn, published by bootstrap which loads
// earlier). Cross-file, all RUNTIME: htmlelem.js's Element constructor reads
// the live __ceUpgradeTarget (mutable let -> a() accessor), domcore.js's
// insertion/removal paths and bootstrap's __prepareInsertedSubtree call the
// walks, the ce_attr plugin module is reached via optional __diting_ce*
// members. Order-free; auto-published.
// --- dynamic custom element upgrades (obscura #993 lineage) ------------
// define() only sweeps elements already in the document at definition time.
// Elements that appear LATER — document.createElement + append, markup set
// through innerHTML, subtrees built detached then attached — were never
// upgraded: the constructor never ran and connectedCallback never fired.
// That is exactly how craigslist's search shell hydrates (it defines
// <cl-search-result> at boot, then createElement's one per result inside
// the fetch callback), and any data-driven web-component render path hits
// the same hole. The walk rides the insertion hook that dynamic scripts
// already use, and is gated on a non-empty registry so pages defining no
// custom elements pay one boolean check.
function __customRegistry() {
  return (typeof customElements !== 'undefined' && customElements._registry) ? customElements._registry : null;
}
// The element a registry upgrade is currently running a constructor against.
// HTMLElement's constructor reads it (bootstrap tail): returning that element
// from the base constructor makes it the `this` of every derived constructor
// up the chain — native classes and Babel's _callSuper both honor a base
// constructor returning an object — so author field initializers land on the
// live element instead of dying on the old `cls.call(el)` (which throws for
// any real class: ES classes refuse .call, Babel's _classCallCheck refuses a
// foreign this). Save/restore, not clear: a constructor that appends another
// custom element triggers a nested upgrade that must restore ours.
let __ceUpgradeTarget = null;
// attributeChangedCallback capture/dispatch lives in the ce_attr platform
// module (define/_upgradeElement/writers call in via the __diting_ce* faces).
function __upgradeCustomElementsIn(root, includeRoot) {
  const reg = __customRegistry();
  if (!reg || !reg.size || !root) return;
  const queue = [];
  if (includeRoot) {
    queue.push(root);
  } else if (root.nodeType === 1 || root.nodeType === 11) {
    const kids = root.childNodes;
    for (let i = 0; i < kids.length; i++) queue.push(kids[i]);
  }
  // Cursor walk instead of recursion: hydration bursts insert deep markup
  // and the JS stack must not be the depth limit (cloneNode uses the same
  // posture). Children enqueue before an element's own callbacks run, so
  // upgrades land in tree order; children an upgrade itself appends arrive
  // through their own appendChild hook and re-visit idempotently here.
  for (let i = 0; i < queue.length; i++) {
    const el = queue[i];
    const kids = el.childNodes;
    for (let j = 0; j < kids.length; j++) queue.push(kids[j]);
    if (el.nodeType !== 1) continue;
    const cls = reg.get(el.localName);
    if (!cls) continue;
    if (!el.__customUpgraded) {
      // In-document: _upgradeElement fires connectedCallback itself.
      customElements._upgradeElement(el, cls);
    } else if (!el.__customConnected && typeof el.connectedCallback === 'function' && el.isConnected) {
      el.__customConnected = true;
      try { el.connectedCallback(); } catch (e) {}
    }
  }
}
// Removal is the mirror: upgraded elements leaving the document get
// disconnectedCallback and drop their connection flag, so a later
// re-insertion fires connectedCallback again (Chrome's connect/disconnect
// cycle — move/reshuffle logic in lit-style frameworks depends on it).
function __disconnectCustomElementsIn(root) {
  const reg = __customRegistry();
  if (!reg || !reg.size || !root) return;
  const queue = [root];
  for (let i = 0; i < queue.length; i++) {
    const el = queue[i];
    const kids = el.childNodes;
    for (let j = 0; j < kids.length; j++) queue.push(kids[j]);
    if (el.nodeType !== 1 || !el.__customConnected) continue;
    el.__customConnected = false;
    if (typeof el.disconnectedCallback === 'function') {
      try { el.disconnectedCallback(); } catch (e) {}
    }
  }
}

// --- registry class ---
function _isConstructorCE(v) {
  if (typeof v !== 'function') return false;
  try { Reflect.construct(function () {}, [], v); return true; } catch (e) { return false; }
}
const _CE_RESERVED = new Set(['annotation-xml', 'color-profile', 'font-face', 'font-face-src', 'font-face-uri', 'font-face-format', 'font-face-name', 'missing-glyph']);
function _isValidCustomElementName(name) {
  if (typeof name !== 'string' || _CE_RESERVED.has(name)) return false;
  // PotentialCustomElementName (approx): lowercase start, a hyphen, no uppercase.
  return /^[a-z][a-z0-9._\u00b7\u00c0-\uffff-]*-[a-z0-9._\u00b7\u00c0-\uffff-]*$/.test(name);
}
class CustomElementRegistry {
  constructor() { this._registry = new Map(); this._byCtor = new Map(); this._whenDefinedResolvers = new Map(); this._defining = false;  __hideOwn(this);}
  define(name, cls, opts) {
    if (!_isConstructorCE(cls)) throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': parameter 2 is not a constructor.");
    if (!_isValidCustomElementName(name)) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': \"" + name + "\" is not a valid custom element name", "SyntaxError");
    if (this._defining) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': operation is not supported while a definition is in progress", "NotSupportedError");
    if (this._registry.has(name)) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': the name \"" + name + "\" has already been used with this registry", "NotSupportedError");
    if (this._byCtor.has(cls)) throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': the constructor has already been used with this registry", "NotSupportedError");
    this._defining = true;
    try {
      // attributeChangedCallback + observedAttributes are read once, here, by
      // the ce_attr platform module (a later prototype patch is never called;
      // errors reading them abort the define, as in Chrome).
      globalThis.__diting_ceCapture?.(cls);
      this._byCtor.set(cls, name); this._defineInner(name, cls, opts);
    } finally { this._defining = false; }
  }
  _defineInner(name, cls, opts) {
    this._registry.set(name, cls);
    // Upgrade existing matching elements: instantiate the class on each,
    // fire connectedCallback if the element is in the document. Without
    // this, lit / MusicKit / Polymer components never wire up their
    // shadow DOM or render, leaving heavy chunks of YouTube,
    // music.apple.com, and any web-component site as empty shells.
    try {
      const matches = globalThis.document?.querySelectorAll(name) || [];
      for (const el of matches) this._upgradeElement(el, cls);
    } catch (e) {}
    const resolvers = this._whenDefinedResolvers.get(name);
    if (resolvers) {
      for (const r of resolvers) r(cls);
      this._whenDefinedResolvers.delete(name);
    }
  }
  _upgradeElement(el, cls) {
    if (el.__customUpgraded) return;
    el.__customUpgraded = true;
    try {
      // Swap the prototype instead of copying prototype methods onto the
      // instance: the wrapper is per-nid cached (same node, same instance
      // forever), so one swap upgrades every later lookup too, and methods
      // stay on the prototype where author code (and instanceof) expects
      // them. cls.prototype chains to HTMLElement.prototype, so the engine
      // levels below stay reachable.
      Object.setPrototypeOf(el, cls.prototype);
      // Run the author's constructor against the live element. Reflect
      // construct drives the real [[Construct]] path (cls.call is dead for
      // classes), and the base HTMLElement constructor sees
      // __ceUpgradeTarget and returns this element, which rebinds `this`
      // for the rest of the chain — constructor field initializers
      // (attachShadow, reactive stores, event wiring) land on the element.
      const prev = __ceUpgradeTarget;
      __ceUpgradeTarget = el;
      try { Reflect.construct(cls, []); } catch (e) {}
      finally { __ceUpgradeTarget = prev; }
      // Upgrade reactions: one attributeChangedCallback(name, null, value)
      // per observed attribute already present, before connectedCallback
      // (Chrome probe: ctor, acc name, acc url, conn) — the ce_attr module.
      globalThis.__diting_ceUpgradeReactions?.(el, cls);
      if (typeof el.connectedCallback === 'function' && globalThis.document?.contains?.(el)) {
        // Mark the connection so the insertion/removal walks can cycle it
        // (Chrome fires connectedCallback again on every re-insertion).
        el.__customConnected = true;
        try { el.connectedCallback(); } catch (e) {}
      }
    } catch (e) {}
  }
  get(name) { return this._registry.get(name); }
  getName(cls) {
    if (!_isConstructorCE(cls)) throw new TypeError("Failed to execute 'getName' on 'CustomElementRegistry': parameter 1 is not a constructor.");
    return this._byCtor.has(cls) ? this._byCtor.get(cls) : null;
  }
  whenDefined(name) {
    if (!_isValidCustomElementName(name)) return Promise.reject(new DOMException("Failed to execute 'whenDefined' on 'CustomElementRegistry': \"" + name + "\" is not a valid custom element name", "SyntaxError"));
    const cls = this._registry.get(name);
    if (cls) return Promise.resolve(cls);
    return new Promise((resolve) => {
      const list = this._whenDefinedResolvers.get(name) || [];
      list.push(resolve);
      this._whenDefinedResolvers.set(name, list);
    });
  }
  upgrade(root) {
    if (!root || !root.querySelectorAll) return;
    for (const [name, cls] of this._registry.entries()) {
      const matches = root.querySelectorAll(name);
      for (const el of matches) this._upgradeElement(el, cls);
    }
  }
}
globalThis.CustomElementRegistry = CustomElementRegistry;
globalThis.customElements = new CustomElementRegistry();

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  a('__ceUpgradeTarget', () => __ceUpgradeTarget, (v) => { __ceUpgradeTarget = v; });
  d('__disconnectCustomElementsIn', __disconnectCustomElementsIn);
  d('__upgradeCustomElementsIn', __upgradeCustomElementsIn);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
