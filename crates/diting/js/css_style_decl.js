(function () { // @@SCOPE
// god-file split (刀34, CSSStyleDeclaration): the style-attribute parse/
// serialize pair, the illegal-constructor class (mint door _newCssDecl),
// and its native-proto marking. Consumers are all RUNTIME — computed_
// style.js / domcore.js call _newCssDecl; the class name's 11-file
// footprint is string tables and comments only (shape_fix PROTO,
// method_ladders, window_order ORDER...); _cssDeclAccepted arrives from
// css_decl_door.js and camel/kebab codecs from css_names.js through the
// publish table. The load-time proto marking uses the streams.js
// member-alias form (globalThis._markNativeProto) — a bare load call
// would trip the crossFileLoadTimeNames ratchet.
const _markNativeProto = globalThis._markNativeProto;
// Parse a `style` attribute string (`"color: red; margin: 5px"`) into the given
// dashed-key store, replacing its contents in place.
function _parseCssInto(props, text) {
  for (const k in props) delete props[k];
  if (text) String(text).split(";").forEach((p) => {
    const i = p.indexOf(":");
    if (i > 0) {
      const k = _cssCamelToKebab(p.slice(0, i).trim());
      const v = p.slice(i + 1).trim();
      // Same door as setProperty: a garbage declaration in a style
      // attribute / cssText string is dropped individually, the way
      // Chrome's partial cssText parse keeps the valid neighbors.
      if (k && v && _cssDeclAccepted(k, v)) props[k] = v;
    }
  });
}
function _serializeCss(props) {
  const e = Object.entries(props);
  return e.length ? e.map(([k, v]) => `${k}: ${v}`).join("; ") + ";" : "";
}

// (刀72) `new CSSStyleDeclaration()` is an Illegal constructor in Chrome; the
// engine's own mints (element.style, rule styles, the computed fallback) go
// through _newCssDecl, which raises a one-shot flag around the construction.
let _cssDeclMint = false;
function _newCssDecl(owner) { _cssDeclMint = true; try { return new CSSStyleDeclaration(owner); } finally { _cssDeclMint = false; } }
Object.defineProperty(globalThis, '__diting_newCssDecl', { value: _newCssDecl, writable: true, enumerable: false, configurable: true });
class CSSStyleDeclaration {
  constructor(owner) {
    if (!_cssDeclMint) throw new TypeError("Failed to construct 'CSSStyleDeclaration': Illegal constructor");
    // Non-enumerable so they never leak through the proxy's own-key traps.
    Object.defineProperty(this, "_props", { value: {}, writable: true, enumerable: false, configurable: true });
    // The owner Element, if any. A live declaration reflects that element's
    // `style` content attribute in both directions; an owner-less declaration
    // (getComputedStyle fallback, stylesheet rules) is purely in-memory.
    Object.defineProperty(this, "_owner", { value: owner || null, writable: true, enumerable: false, configurable: true });
    // Last `style` attribute string we parsed/wrote, so a read can skip the
    // reparse when the attribute has not changed underneath us. Held in a
    // one-field object so the sync helpers can mutate it without a bare
    // `this._x = …` assignment (which the style proxy would reroute into
    // setProperty).
    Object.defineProperty(this, "_sync", { value: { last: null }, writable: true, enumerable: false, configurable: true });
    __hideOwn(this);
  }
  // Pull the owner's `style` attribute into `_props` if it changed since our
  // last read/write. Keeps parsed HTML and setAttribute('style', …) visible via
  // el.style.*. No-op when owner-less.
  _pull() {
    const o = this._owner;
    if (!o) return;
    const attr = o.getAttribute("style");
    if (attr === this._sync.last) return;
    _parseCssInto(this._props, attr);
    this._sync.last = attr;
  }
  // Serialize `_props` back onto the owner's `style` attribute after a mutation,
  // so el.style.x = … and cssText reflect into getAttribute('style') and
  // serialization. No-op when owner-less.
  _push() {
    const o = this._owner;
    if (!o) return;
    const text = _serializeCss(this._props);
    this._sync.last = text;
    if (text) o.setAttribute("style", text);
    else o.removeAttribute("style");
  }
  // Storage is keyed by the dashed CSS name, matching CSSOM. The proxy maps the
  // camelCase IDL access (el.style.fontSize) onto the dashed key (font-size), so
  // getPropertyValue('font-size') and el.style.fontSize stay in sync.
  setProperty(name, value) {
    this._pull();
    const k = _cssCamelToKebab(String(name));
    if (value === "" || value == null) delete this._props[k];
    else {
      const v = String(value);
      // #148: invalid values are silent no-ops — the stored declaration
      // (and the read-back) keeps the previous value, like Chrome.
      const c = k === "font-family" ? _cssFontFamilyCanon(v) : v;
      if (c !== null && _cssDeclAccepted(k, v)) this._props[k] = c;
    }
    this._push();
  }
  removeProperty(name) { this._pull(); const k = _cssCamelToKebab(String(name)); const old = this._props[k]; delete this._props[k]; this._push(); return old || ""; }
  getPropertyValue(name) { this._pull(); return this._props[_cssCamelToKebab(String(name))] || ""; }
  getPropertyPriority() { return ""; }
  get cssText() { this._pull(); return _serializeCss(this._props); }
  set cssText(v) {
    _parseCssInto(this._props, v);
    this._push();
  }
  get length() { this._pull(); return Object.keys(this._props).length; }
  item(i) { this._pull(); return Object.keys(this._props)[i] || ""; }
}
// (刀72) The CSS2Properties intermediate prototype is gone: the Chrome 145
// captures (shape-audit-truth-145, k72) show CSSStyleDeclaration straight over
// Object and no CSS2Properties interface at all (that level appears in later
// Chromes — the old comment cited a 155 capture; the target here is 145).
_markNativeProto(CSSStyleDeclaration.prototype);

// Type of element.style / getComputedStyle(); the class binding above is
// lexical, so without this `window.CSSStyleDeclaration` was undefined and
// `el.style instanceof CSSStyleDeclaration` threw (upstream a0e1ba5).
globalThis.CSSStyleDeclaration = CSSStyleDeclaration;
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('CSSStyleDeclaration', CSSStyleDeclaration);
  d('_markNativeProto', _markNativeProto);
  d('_newCssDecl', _newCssDecl);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
