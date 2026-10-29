//! module: htmlreflect
//! layer: L2
//! exposes: -
//! requires-load: legacy:Node
//! requires-call: legacy:DOMTokenList, legacy:document, legacy:URL, legacy:DOMException, legacy:HTMLCollection, legacy:TextTrack, legacy:Element

// Behavior legs for the HTML element families (M4 knives 6-7). Member SHAPE
// and every uniformly reflected member are generated into
// gen/htmlreflect.gen.js from MEASURED Chrome fingerprints
// (chrome/htmlreflect-truth-145.json); this module owns what is not a plain
// reflection. Tables are keyed 'Short.member' (Meter.value vs Progress.value
// vs Output.value share a name); every fact below is pinned by
// htmlreflect-truth-145.json or htmlform-truth-145.json (Chrome 145).

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

const nodeText = Object.getOwnPropertyDescriptor(Node.prototype, 'textContent');

// ---- shared parsing / conversion
// Chrome rejects trailing garbage here ('9px' reads the default, '1e2' is 100)
const FLOAT = /^[\t\n\f\r ]*(-?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?)[\t\n\f\r ]*$/;
function parseFloatAttr(v) {
  if (v === null) return null;
  const m = FLOAT.exec(v);
  if (!m) return null;
  const n = Number(m[1]);
  return isFinite(n) ? n : null;
}
function fAttr(el, a, dflt) { const n = parseFloatAttr(el.getAttribute(a)); return n === null ? dflt : n; }
// WebIDL `double`: non-finite is a TypeError with Chrome's wording
function toDouble(v, name, iface, fn) {
  const pre = "Failed to set the '" + name + "' property on '" + iface + "'";
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', pre);
  const n = Number(v);
  if (!isFinite(n)) P.fail(fn, 'The provided double value is non-finite.', pre);
  return n;
}
function clamp(n, lo, hi) { return n < lo ? lo : n > hi ? hi : n; }
function dstr(v, name, iface, fn) { return P.conv.DOMString(v, "Failed to set the '" + name + "' property on '" + iface + "'", fn); }
function root(el) { return el.getRootNode ? el.getRootNode() : null; }
function byId(el, id) {
  const r = root(el);
  return r && typeof r.getElementById === 'function' ? r.getElementById(id) : null;
}

// ---- script.async: the "force async" flag (createElement scripts start
// force-async; parser scripts follow the attribute; any set clears it).
const asyncCleared = new WeakSet();
function parserCreated(el) {
  const idx = globalThis.__parserScriptIdx;
  return !!(idx && el._nid !== undefined && el._nid !== null && idx.has(el._nid));
}

// ---- element reflection (popoverTargetElement / commandForElement):
// getter = the explicitly set element, else the id-referenced element in the
// same tree; setting null removes the attribute, an element writes '' and
// remembers the element (htmlform-truth popoverTarget).
const explicitRef = new WeakMap();
function refGet(el, key, attr) {
  const m = explicitRef.get(el);
  if (m && m.has(key)) return m.get(key);
  const id = el.getAttribute(attr);
  return id === null ? null : byId(el, id);
}
function refSet(el, key, attr, v, name, iface, fn) {
  if (v !== null && v !== undefined && !(v instanceof Element)) {
    P.fail(fn, "Failed to convert value to 'Element'.", "Failed to set the '" + name + "' property on '" + iface + "'");
  }
  let m = explicitRef.get(el);
  if (!m) { m = new Map(); explicitRef.set(el, m); }
  if (v === null || v === undefined) { m.delete(key); el.removeAttribute(attr); return; }
  m.set(key, v);
  el.setAttribute(attr, '');
}

// ---- output: value/defaultValue with the default-value-override flag
const outState = new WeakMap();
function os(el) { let s = outState.get(el); if (!s) { s = { override: false, dflt: '' }; outState.set(el, s); } return s; }

// ---- track: minted TextTrack, cached
const trackCache = new WeakMap();

const GET = {
  'Script.async'(el) { return el.hasAttribute('async') || (!asyncCleared.has(el) && !parserCreated(el)); },
  'Title.text'(el) { return nodeText.get.call(el); },
  'Script.text'(el) { return nodeText.get.call(el); },
  'Script.textContent'(el) { return nodeText.get.call(el); },
  'Script.innerText'(el) { return nodeText.get.call(el); },
  // Chrome: the sheet's disabled flag; an unattached <style> has no sheet
  'Style.disabled'(el) { return el.sheet ? !!el.sheet.disabled : false; },
  // <base href>: no attribute -> the document's fallback base URL
  'Base.href'(el) {
    const raw = el.getAttribute('href');
    if (raw === null) return document.URL;
    try { return new URL(raw, document.URL).href; } catch (e) { return String(raw); }
  },
  'Legend.form'(el) {
    const p = el.parentNode;
    return p && p.localName === 'fieldset' ? (p.form || null) : null;
  },

  // ---- input
  'Input.list'(el) {
    const id = el.getAttribute('list');
    if (id === null) return null;
    const t = byId(el, id);
    return t && t.localName === 'datalist' ? t : null;
  },
  'Input.webkitEntries'() { return []; },
  // width/height only mean something for type=image; every other type reads 0
  'Input.width'(el) { return imgDim(el, 'width'); },
  'Input.height'(el) { return imgDim(el, 'height'); },
  'Input.popoverTargetElement'(el) { return refGet(el, 'popover', 'popovertarget'); },
  'Button.popoverTargetElement'(el) { return refGet(el, 'popover', 'popovertarget'); },
  'Button.commandForElement'(el) { return refGet(el, 'command', 'commandfor'); },

  // ---- select / option / textarea / output
  'Select.length'(el) { return el.options.length; },
  'Select.type'(el) { return el.hasAttribute('multiple') ? 'select-multiple' : 'select-one'; },
  'Option.index'(el) {
    const sel = el.parentNode && (el.parentNode.localName === 'select' ? el.parentNode
      : el.parentNode.parentNode && el.parentNode.parentNode.localName === 'select' ? el.parentNode.parentNode : null);
    return sel ? Array.prototype.indexOf.call(sel.options, el) : 0;
  },
  'TextArea.defaultValue'(el) { return nodeText.get.call(el); },
  'TextArea.textLength'(el) { return String(el.value).length; },
  'Output.defaultValue'(el) { const s = os(el); return s.override ? s.dflt : nodeText.get.call(el); },
  'Output.value'(el) { return nodeText.get.call(el); },

  // ---- marquee / meter / progress
  'Marquee.loop'(el) {
    const n = P.parseIntAttr(el.getAttribute('loop') === null ? '' : el.getAttribute('loop'));
    return n === null || n < 1 && n !== -1 || n > 2147483647 ? -1 : n;
  },
  'Meter.min'(el) { return fAttr(el, 'min', 0); },
  'Meter.max'(el) { return Math.max(fAttr(el, 'max', 1), fAttr(el, 'min', 0)); },
  'Meter.value'(el) { return clamp(fAttr(el, 'value', 0), GET['Meter.min'](el), GET['Meter.max'](el)); },
  'Meter.low'(el) { return clamp(fAttr(el, 'low', GET['Meter.min'](el)), GET['Meter.min'](el), GET['Meter.max'](el)); },
  'Meter.high'(el) {
    const lo = GET['Meter.low'](el), mx = GET['Meter.max'](el);
    return clamp(fAttr(el, 'high', mx), lo, mx);
  },
  'Meter.optimum'(el) {
    const mn = GET['Meter.min'](el), mx = GET['Meter.max'](el);
    return clamp(fAttr(el, 'optimum', (mn + mx) / 2), mn, mx);
  },
  'Progress.max'(el) { const n = fAttr(el, 'max', 1); return n > 0 ? n : 1; },
  'Progress.value'(el) {
    const n = fAttr(el, 'value', 0);
    return n < 0 ? 0 : Math.min(n, GET['Progress.max'](el));
  },
  'Progress.position'(el) {
    return el.hasAttribute('value') && parseFloatAttr(el.getAttribute('value')) !== null
      ? GET['Progress.value'](el) / GET['Progress.max'](el) : -1;
  },

  // ---- fieldset.elements / map.areas (snapshot collections)
  'FieldSet.elements'(el) {
    const out = [];
    for (const c of el.querySelectorAll('button, fieldset, input, object, output, select, textarea')) {
      if (c.localName === 'input' && (c.getAttribute('type') || '').toLowerCase() === 'image') continue;
      out.push(c);
    }
    return HTMLCollection._from(out);
  },
  'Map.areas'(el) { return HTMLCollection._from(Array.from(el.querySelectorAll('area'))); },

  // ---- track
  'Track.readyState'() { return 0; },
  'Track.track'(el) {
    let t = trackCache.get(el);
    if (!t) { t = Object.create(TextTrack.prototype); trackCache.set(el, t); }
    return t;
  },
};

// type=image dimension: parse the attribute; anything else reads 0
function imgDim(el, a) {
  if ((el.getAttribute('type') || '').toLowerCase() !== 'image') return 0;
  const n = P.parseIntAttr(el.getAttribute(a) === null ? '' : el.getAttribute(a));
  return n === null || n < 0 || n > 4294967295 ? 0 : n;
}
function setUlong(el, a, v, name, iface, fn) {
  const u = P.conv.unsignedLong(v, "Failed to set the '" + name + "' property on '" + iface + "'", fn);
  el.setAttribute(a, String(u > 2147483647 ? 0 : u));
}

const SET = {
  'Script.async'(el, v) {
    asyncCleared.add(el);
    if (v) el.setAttribute('async', ''); else el.removeAttribute('async');
  },
  // DOMString conversion: null reads back 'null' (truth Title.text/Script.text)
  'Title.text'(el, v) { nodeText.set.call(el, dstr(v, 'text', 'HTMLTitleElement', SET['Title.text'])); },
  'Script.text'(el, v) { nodeText.set.call(el, dstr(v, 'text', 'HTMLScriptElement', SET['Script.text'])); },
  'Script.textContent'(el, v) { nodeText.set.call(el, v === null ? '' : String(v)); },
  'Script.innerText'(el, v) { nodeText.set.call(el, String(v)); },
  'Style.disabled'(el, v) { if (el.sheet) el.sheet.disabled = !!v; },
  'Base.href'(el, v) { el.setAttribute('href', dstr(v, 'href', 'HTMLBaseElement', SET['Base.href'])); },

  'Input.width'(el, v) { setUlong(el, 'width', v, 'width', 'HTMLInputElement', SET['Input.width']); },
  'Input.height'(el, v) { setUlong(el, 'height', v, 'height', 'HTMLInputElement', SET['Input.height']); },
  'Input.popoverTargetElement'(el, v) { refSet(el, 'popover', 'popovertarget', v, 'popoverTargetElement', 'HTMLInputElement', SET['Input.popoverTargetElement']); },
  'Button.popoverTargetElement'(el, v) { refSet(el, 'popover', 'popovertarget', v, 'popoverTargetElement', 'HTMLButtonElement', SET['Button.popoverTargetElement']); },
  'Button.commandForElement'(el, v) { refSet(el, 'command', 'commandfor', v, 'commandForElement', 'HTMLButtonElement', SET['Button.commandForElement']); },

  'Select.length'(el, v) {
    const n = P.conv.unsignedLong(v, "Failed to set the 'length' property on 'HTMLSelectElement'", SET['Select.length']);
    if (n > 100000) return;
    let cur = el.options.length;
    while (cur < n) { el.appendChild(document.createElement('option')); cur++; }
    while (cur > n) { const o = el.options[cur - 1]; if (o && o.parentNode) o.parentNode.removeChild(o); cur--; }
  },
  'TextArea.defaultValue'(el, v) { nodeText.set.call(el, dstr(v, 'defaultValue', 'HTMLTextAreaElement', SET['TextArea.defaultValue'])); },
  // output.value: the first write freezes the default (defaultValue stays)
  'Output.value'(el, v) {
    const s = os(el);
    const str = dstr(v, 'value', 'HTMLOutputElement', SET['Output.value']);
    if (!s.override) { s.dflt = nodeText.get.call(el); s.override = true; }
    nodeText.set.call(el, str);
  },
  'Output.defaultValue'(el, v) {
    const s = os(el);
    const str = dstr(v, 'defaultValue', 'HTMLOutputElement', SET['Output.defaultValue']);
    s.dflt = str;
    if (!s.override) nodeText.set.call(el, str);
  },

  'Marquee.loop'(el, v) {
    if (typeof v === 'symbol') P.fail(SET['Marquee.loop'], 'Cannot convert a Symbol value to a number', "Failed to set the 'loop' property on 'HTMLMarqueeElement'");
    const n = Number(v);
    let i = isFinite(n) ? ((Math.trunc(n) % 4294967296) + 4294967296) % 4294967296 : 0;
    if (i > 2147483647) i -= 4294967296; // ToInt32
    if (i === 0 || i < -1 || i > 2147483647) {
      throw new DOMException("Failed to set the 'loop' property on 'HTMLMarqueeElement': The provided value (" + i + ') is neither positive nor -1.', 'IndexSizeError');
    }
    el.setAttribute('loop', String(i));
  },
  'Meter.min'(el, v) { el.setAttribute('min', String(toDouble(v, 'min', 'HTMLMeterElement', SET['Meter.min']))); },
  'Meter.max'(el, v) { el.setAttribute('max', String(toDouble(v, 'max', 'HTMLMeterElement', SET['Meter.max']))); },
  'Meter.value'(el, v) { el.setAttribute('value', String(toDouble(v, 'value', 'HTMLMeterElement', SET['Meter.value']))); },
  'Meter.low'(el, v) { el.setAttribute('low', String(toDouble(v, 'low', 'HTMLMeterElement', SET['Meter.low']))); },
  'Meter.high'(el, v) { el.setAttribute('high', String(toDouble(v, 'high', 'HTMLMeterElement', SET['Meter.high']))); },
  'Meter.optimum'(el, v) { el.setAttribute('optimum', String(toDouble(v, 'optimum', 'HTMLMeterElement', SET['Meter.optimum']))); },
  // progress: a non-positive max / negative value is written as the default
  'Progress.max'(el, v) {
    const n = toDouble(v, 'max', 'HTMLProgressElement', SET['Progress.max']);
    el.setAttribute('max', String(n > 0 ? n : 1));
  },
  'Progress.value'(el, v) {
    const n = toDouble(v, 'value', 'HTMLProgressElement', SET['Progress.value']);
    el.setAttribute('value', String(n < 0 ? 0 : n));
  },
};

const CALL = {
  // Chrome answers null unless the embedded content is an SVG document
  'Embed.getSVGDocument'() { return null; },
  'IFrame.getSVGDocument'() { return null; },
  'Object.getSVGDocument'() { return null; },
  'Anchor.toString'(el) { return el.href; },
  'Area.toString'(el) { return el.href; },
  // fallbacks only (the engine chain owns submit/requestSubmit; the
  // generated seat delegates to it first)
  'Form.submit'() { return undefined; },
  'Form.requestSubmit'() { return undefined; },

  // no user activation in a script-driven context: Chrome refuses
  'Input.showPicker'() {
    throw new DOMException("Failed to execute 'showPicker' on 'HTMLInputElement': HTMLInputElement::showPicker() requires a user gesture.", 'NotAllowedError');
  },
  'Select.showPicker'() {
    throw new DOMException("Failed to execute 'showPicker' on 'HTMLSelectElement': showPicker() requires a user gesture.", 'NotAllowedError');
  },
  'Select.item'(el, ctx, args) {
    const o = el.options[P.conv.unsignedLong(args[0], ctx, CALL['Select.item'])];
    return o === undefined ? null : o;
  },
  'Select.namedItem'(el, ctx, args) {
    const name = P.conv.DOMString(args[0], ctx, CALL['Select.namedItem']);
    if (name === '') return null;
    for (const o of el.options) if (o.id === name || o.getAttribute('name') === name) return o;
    return null;
  },
  'Marquee.start'() { return undefined; },
  'Marquee.stop'() { return undefined; },
  'Slot.assign'() { return undefined; },
};

function find(T, kind, name) {
  const f = T[name];
  if (!f) throw new Error('htmlreflect impl: no ' + kind + ' ' + name);
  return f;
}
exports.get = function (name, el) { return find(GET, 'getter', name)(el); };
exports.set = function (name, el, v) { find(SET, 'setter', name)(el, v); };
exports.call = function (name, el, ctx, args) { return find(CALL, 'operation', name)(el, ctx, args); };
