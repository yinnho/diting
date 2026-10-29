//! module: htmlreflect
//! layer: L2
//! exposes: -
//! requires-load: legacy:Node
//! requires-call: legacy:DOMTokenList, legacy:document, legacy:URL

// Behavior legs for the HTML reflected-attribute element family (M4 knife 6).
// The member SHAPE and every uniformly reflected member is generated into
// gen/htmlreflect.gen.js from MEASURED Chrome fingerprints
// (chrome/htmlreflect-truth-145.json); this module owns only what is not a
// plain reflection: cached DOMTokenList faces, script.async's force-async
// flag, and the few stateful accessors / constant answers.

// ---- DOMTokenList faces (relList / blocking / sandbox ...): one cached
// list per (element, content attribute); `supported` is the MEASURED
// supports() set (null => supports() throws, as Chrome's sizes does).
const tokenCache = new WeakMap();
exports.tokens = function (el, attr, supported) {
  let m = tokenCache.get(el);
  if (!m) { m = new Map(); tokenCache.set(el, m); }
  let t = m.get(attr);
  if (!t) { t = new DOMTokenList(el, attr, supported === null ? undefined : supported); m.set(attr, t); }
  return t;
};

// ---- script.async: the "force async" flag. createElement('script') starts
// force-async (getter reads true with no attribute); parser-created scripts
// do not (their getter follows the attribute); any set clears the flag and
// then reflects the boolean attribute. Parser scripts are recognized by the
// document-order registry scripts.rs injects.
const asyncCleared = new WeakSet();
function parserCreated(el) {
  const idx = globalThis.__parserScriptIdx;
  return !!(idx && el._nid !== undefined && el._nid !== null && idx.has(el._nid));
}

const nodeText = Object.getOwnPropertyDescriptor(Node.prototype, 'textContent');

const GET = {
  async(el) { return el.hasAttribute('async') || (!asyncCleared.has(el) && !parserCreated(el)); },
  // <title>.text / <script>.text: child text content
  text(el) { return nodeText.get.call(el); },
  textContent(el) { return nodeText.get.call(el); },
  innerText(el) { return nodeText.get.call(el); },
  // Chrome: the sheet's disabled flag; an unattached <style> has no sheet and reads false
  disabled(el) { return el.sheet ? !!el.sheet.disabled : false; },
  // <base href>: with no attribute the getter answers the document's
  // fallback base URL (document.URL); with one it resolves against that
  // fallback base - not against the base element's own effect (truth Base.href def).
  href(el) {
    const raw = el.getAttribute('href');
    if (raw === null) return document.URL;
    try { return new URL(raw, document.URL).href; } catch (e) { return String(raw); }
  },
  // legend.form: the form owner of a parent fieldset, else null
  form(el) {
    const p = el.parentNode;
    return p && p.localName === 'fieldset' ? (p.form || null) : null;
  },
};

const SET = {
  async(el, v) {
    asyncCleared.add(el);
    if (v) el.setAttribute('async', ''); else el.removeAttribute('async');
  },
  // DOMString conversion: null reads back 'null' (truth Title.text/Script.text)
  text(el, v) { nodeText.set.call(el, P.conv.DOMString(v, "Failed to set the 'text' property on '" + el.constructor.name + "'", SET.text)); },
  textContent(el, v) { nodeText.set.call(el, v === null ? '' : String(v)); },
  innerText(el, v) { nodeText.set.call(el, String(v)); },
  href(el, v) { el.setAttribute('href', P.conv.DOMString(v, "Failed to set the 'href' property on 'HTMLBaseElement'", SET.href)); },
  disabled(el, v) { if (el.sheet) el.sheet.disabled = !!v; },
};

const CALL = {
  // Chrome answers null unless the embedded content is an SVG document
  getSVGDocument() { return null; },
  toString(el) { return el.href; },
  // fallbacks only (the engine chain owns submit/requestSubmit; the
  // generated seat delegates to it first)
  submit() { return undefined; },
  requestSubmit() { return undefined; },
};

exports.get = function (name, el) {
  const f = GET[name];
  if (!f) throw new Error('htmlreflect impl: no getter ' + name);
  return f(el);
};
exports.set = function (name, el, v) {
  const f = SET[name];
  if (!f) throw new Error('htmlreflect impl: no setter ' + name);
  f(el, v);
};
exports.call = function (name, el, ctx, args) {
  const f = CALL[name];
  if (!f) throw new Error('htmlreflect impl: no operation ' + name);
  return f(el, ctx, args);
};
