//! module: htmlelement
//! layer: L2
//! exposes: -
//! requires-load: stylemap
//! requires-call: legacy:DOMException, legacy:_markNative, legacy:document, legacy:DOMTokenList

// HTMLElement/Element behavior only; the member SHAPE (placement, order,
// descriptor kinds, arity) is generated from the Chrome truth overlay in
// gen/htmlelement.gen.js, and uniform attribute reflection is DATA there
// (P.reflector). This module owns the NON-uniform semantics; every fact
// below that could be misremembered is pinned by a key in
// tools/webplatform/chrome/htmlelement-truth-145.json (captured from real
// Chrome 145) and diffed by the htmlelement_members_match_chrome_truth test.
// Knife-4 staged-family behavior (part/attributeStyleMap/computedStyleMap/
// fullscreen trio/pointer lock/moveBefore/ariaNotify) is pinned by
// element-staged-truth-145.json and diffed by
// element_staged_members_match_chrome_truth.

// ---- nonce: internal slot, deliberately NOT reflected (truth set.nonce:
// property 'abc' -> getAttribute('nonce') === null). Chrome hides the
// attribute value once the element is connected; that nuance is staged.
const nonceSlots = new WeakMap();

// ---- popover shown-state: no invoker/layout machinery yet - toggle flips
// a per-element flag (first call on a plain div returns true, truth
// togglePopover.plain); show/hide are Chrome's no-ops (truth
// showPopover.plain === #undefined on a non-popover element).
const popoverShown = new WeakSet();

// ---- staged-family state (knife 4): part DOMTokenList is cached per
// element (truth part.same); the Typed OM maps cache in stylemap.
const partCache = new WeakMap();
const SM = require('stylemap');

const GET = {
  translate(el) { return el.getAttribute('translate') === 'no' ? false : true; },
  spellcheck(el) {
    const v = el.getAttribute('spellcheck');
    return v === 'true' ? true : v === 'false' ? false : true; // invalid attr -> default true (truth spellcheck.attr.invalid)
  },
  popover(el) {
    const v = el.getAttribute('popover');
    if (v === null) return null;
    if (v === '') return 'auto';
    const low = v.toLowerCase();
    return low === 'auto' || low === 'manual' ? low : 'manual';
  },
  nonce(el) { return nonceSlots.get(el) || ''; },
  contentEditable(el) {
    const v = el.getAttribute('contenteditable');
    if (v === null) return 'inherit';
    const low = v.toLowerCase();
    if (low === '') return 'true'; // empty attribute = the true state (htmlreflect-truth vocab[''])
    return low === 'true' || low === 'false' || low === 'plaintext-only' ? low : 'inherit';
  },
  isContentEditable(el) {
    const v = el.getAttribute('contenteditable');
    if (v === null) return false;
    const low = v.toLowerCase();
    return low === '' || low === 'true' || low === 'plaintext-only';
  },
  outerText(el) { return el.innerText; },
  editContext() { return null; },
  part(el) {
    let p = partCache.get(el);
    if (!p) { p = new DOMTokenList(el, 'part'); partCache.set(el, p); }
    return p;
  },
  attributeStyleMap(el) { return SM.forElement(el); },
};

const SET = {
  translate(el, v) {
    // boolean IDL attribute over the enumerated content attribute: ToBoolean
    // of ANY value, written 'yes'/'no' (htmlreflect-truth translate.set.*:
    // 'maybe'/5/true -> 'yes'; ''/null/undefined/false -> 'no'). The knife-2
    // reading ("invalid values are a no-op") misread set.translate.bad.
    el.setAttribute('translate', v ? 'yes' : 'no');
  },
  spellcheck(el, v) { el.setAttribute('spellcheck', v ? 'true' : 'false'); },
  popover(el, v) {
    if (v === null) { el.removeAttribute('popover'); return; }
    el.setAttribute('popover', v === undefined ? '' : String(v));
  },
  nonce(el, v) { nonceSlots.set(el, P.conv.DOMString(v, "Failed to set the 'nonce' property on 'HTMLElement'", SET.nonce)); },
  contentEditable(el, v) {
    const s = P.conv.DOMString(v, "Failed to set the 'contentEditable' property on 'HTMLElement'", SET.contentEditable);
    const low = s.toLowerCase();
    if (low === 'true' || low === 'false' || low === 'plaintext-only') { el.setAttribute('contenteditable', low); return; }
    if (low === 'inherit') { el.removeAttribute('contenteditable'); return; }
    throw new DOMException("Failed to set the 'contentEditable' property on 'HTMLElement': The value provided ('" +
      s + "') is not one of 'true', 'false', 'plaintext-only', or 'inherit'.", 'SyntaxError');
  },
  outerText(el, v) {
    // Chrome replaces the element with a text node (truth set.outerText:
    // parentNode is null afterwards); parentless is a silent no-op.
    const parent = el.parentNode;
    if (!parent) return;
    parent.replaceChild(document.createTextNode(String(v)), el);
  },
  editContext() { /* swallow */ },
  classList(el, v) { el.className = v === undefined || v === null ? '' : String(v); },
  outerHTML(el, v) {
    // Chrome replaces the node via fragment parsing; the pinned case is the
    // parentless throw (truth outerHTML.set, exact message).
    const parent = el.parentNode;
    if (!parent) throw new DOMException("Failed to set the 'outerHTML' property on 'Element': This element has no parent node.", 'NoModificationAllowedError');
    el.insertAdjacentHTML('beforebegin', String(v));
    el.remove();
  },
  // aria*Elements: nullable IDREF reflection with no element machinery -
  // the setter accepts and swallows (shape rw per EL.kinds).
  ariaSwallow() { /* swallow */ },
  part(el, v) { el.setAttribute('part', v === undefined || v === null ? '' : String(v)); },
};

const CALL = {
  attachInternals() {
    throw new DOMException("Failed to execute 'attachInternals' on 'HTMLElement': Unable to attach ElementInternals to non-custom elements.", 'NotSupportedError');
  },
  showPopover() { return undefined; },
  hidePopover() { return undefined; },
  togglePopover(el) {
    if (popoverShown.has(el)) { popoverShown.delete(el); return false; }
    popoverShown.add(el);
    return true;
  },
  hasAttributeNS(el, ctx, args) {
    return el.getAttributeNS(args[0], args[1]) !== null;
  },
  scrollIntoViewIfNeeded(el) { el.scrollIntoView(); return undefined; },
  webkitMatchesSelector(el, ctx, args) { return el.matches(args[0]); },

  // ---- staged families (knife 4; every message pinned by
  // element-staged-truth-145.json mb.*/rfs.*/rpl.* keys)
  computedStyleMap(el) { return SM.computedFor(el); },
  moveBefore(el, ctx, args) {
    const node = args[0];
    if (node === null || node === undefined || typeof node !== 'object' || typeof node.nodeType !== 'number') {
      throw new TypeError(ctx + ": parameter 1 is not of type 'Node'.");
    }
    if (node.parentNode !== el) {
      throw new DOMException(ctx + ': State-preserving atomic move cannot be performed on nodes participating in an invalid hierarchy.', 'HierarchyRequestError');
    }
    const ref = args[1];
    if (ref !== null && ref !== undefined && ref.parentNode !== el) {
      throw new DOMException(ctx + ': The node before which the new node is to be inserted is not a child of this node.', 'NotFoundError');
    }
    return el.insertBefore(node, ref === undefined ? null : ref);
  },
  requestFullscreen() { return Promise.reject(new TypeError('Permissions check failed')); },
  webkitRequestFullScreen() { return undefined; },
  webkitRequestFullscreen() { return undefined; },
  requestPointerLock() {
    return Promise.reject(new DOMException('The root document of this element is not valid for pointer lock.', 'WrongDocumentError'));
  },
  ariaNotify() { return undefined; },
};

exports.get = function (name, el) {
  const f = GET[name];
  if (!f) throw new Error('htmlelement impl: no getter ' + name);
  return f(el);
};
exports.set = function (name, el, v) {
  const f = SET[name] || (/^aria\w+Elements?$/.test(name) ? SET.ariaSwallow : null);
  if (!f) throw new Error('htmlelement impl: no setter ' + name);
  f(el, v);
};
exports.call = function (name, el, ctx, args) {
  const f = CALL[name];
  if (!f) throw new Error('htmlelement impl: no operation ' + name);
  return f(el, ctx, args);
};
