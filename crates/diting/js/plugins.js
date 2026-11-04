(function () { // @@SCOPE

// PluginArray / MimeTypeArray / Plugin / MimeType — real browsers expose these
// constructors globally, and bot detectors reference them directly (absent →
// ReferenceError) or check `navigator.plugins instanceof PluginArray`.
// Making them real classes (not plain arrays) keeps instanceof working.
// (#125) Chrome's Plugin is array-like over its member MimeType objects with
// name/filename/description/length on the PROTOTYPE (Object.keys shows only
// the member indices), and both directions of the plugin↔mime graph are
// walkable: plugins[0][0].type, plugins[0].namedItem(...), and
// mimeTypes[0].enabledPlugin === navigator.plugins[0]. enabledPlugin=null is
// a shape Chrome never produces on the PDF persona.
globalThis.Plugin = class Plugin {
  // (#125 lineage) Chrome's Plugin instance owns ONLY the member indices
  // (enumerable) plus the mime-type names as non-enumerable named props —
  // `Object.getOwnPropertyNames(plugins[0])` is `0|1|application/pdf|text/pdf`.
  // name/filename/description/length live in a WeakMap behind prototype
  // getters; own `__plugin*` fields (fp-diff-1014 #4) were an instant
  // enumeration tell.
  constructor(name, filename, description, mimes) {
    __pluginStore.set(this, { name, filename, description, length: 0 });
    let n = 0;
    if (mimes) {
      for (const m of mimes) {
        Object.defineProperty(this, String(n++), { value: m, writable: true, configurable: true, enumerable: true });
        if (m && m.type) {
          Object.defineProperty(this, m.type, { value: m, writable: true, configurable: true, enumerable: false });
        }
      }
    }
    __pluginStore.get(this).length = n;
  }
  get name() { return __pluginStore.get(this).name; }
  get filename() { return __pluginStore.get(this).filename; }
  get description() { return __pluginStore.get(this).description; }
  get length() { return __pluginStore.get(this).length; }
  item(i) { return this[i] || null; }
  namedItem(type) {
    for (let i = 0; i < this.length; i++) {
      if (this[i] && this[i].type === type) return this[i];
    }
    return null;
  }
};
globalThis.Plugin.prototype[Symbol.toStringTag] = 'Plugin';
const __pluginStore = new WeakMap();
globalThis.MimeType = class MimeType {
  // WebIDL attributes are prototype accessors; the instance owns nothing
  // (fp-diff-1014 #4 family — own type/description/suffixes/enabledPlugin
  // fields were visible to every enumeration pass).
  constructor(type, description, suffixes, enabledPlugin) {
    __mimeStore.set(this, { type, description, suffixes, enabledPlugin });
  }
  get type() { return __mimeStore.get(this).type; }
  get description() { return __mimeStore.get(this).description; }
  get suffixes() { return __mimeStore.get(this).suffixes; }
  get enabledPlugin() { return __mimeStore.get(this).enabledPlugin; }
};
globalThis.MimeType.prototype[Symbol.toStringTag] = 'MimeType';
const __mimeStore = new WeakMap();
// NOT `extends Array`: Chrome's PluginArray/MimeTypeArray are platform
// array-likes, so `Array.isArray(navigator.plugins)` is false there and true
// here — a one-liner detector. Same shape for MimeTypeArray below.
globalThis.PluginArray = class PluginArray {
  constructor(...items) {
    __pluginArrStore.set(this, items);
    items.forEach((p, i) => Object.defineProperty(this, String(i), { value: p, writable: true, configurable: true, enumerable: true }));
    // Named properties (legacy platform object): own but NOT enumerable, first plugin of a name wins.
    items.forEach((p) => { if (!Object.prototype.hasOwnProperty.call(this, p.name)) Object.defineProperty(this, p.name, { value: p, writable: true, configurable: true, enumerable: false }); });
  }
  get length() { return __pluginArrStore.get(this).length; }
  item(i) { return this[i] || null; }
  namedItem(name) {
    const a = __pluginArrStore.get(this);
    for (let i = 0; i < a.length; i++) if (a[i].name === name) return a[i];
    return null;
  }
  refresh() {}
};
globalThis.PluginArray.prototype[Symbol.toStringTag] = 'PluginArray';
const __pluginArrStore = new WeakMap();
globalThis.MimeTypeArray = class MimeTypeArray {
  constructor(...items) {
    __mimeArrStore.set(this, items);
    items.forEach((m, i) => Object.defineProperty(this, String(i), { value: m, writable: true, configurable: true, enumerable: true }));
    items.forEach((m) => { if (!Object.prototype.hasOwnProperty.call(this, m.type)) Object.defineProperty(this, m.type, { value: m, writable: true, configurable: true, enumerable: false }); });
  }
  get length() { return __mimeArrStore.get(this).length; }
  item(i) { return this[i] || null; }
  namedItem(name) {
    const a = __mimeArrStore.get(this);
    for (let i = 0; i < a.length; i++) if (a[i].type === name) return a[i];
    return null;
  }
};
globalThis.MimeTypeArray.prototype[Symbol.toStringTag] = 'MimeTypeArray';
const __mimeArrStore = new WeakMap();
// Cached singletons: real browsers return the same instance on every access
// (`navigator.plugins === navigator.plugins`); fresh instances would be a
// fingerprint anomaly. The plugin↔mime graph is built once so the member
// MimeType objects are shared by identity with navigator.mimeTypes (Chrome:
// plugins[0][0] === mimeTypes[0]).
let _pluginsInst = null, _mimeTypesInst = null;
function __ditingBuildPdfPersona() {
  if (_pluginsInst) return;
  const mtApp = new MimeType("application/pdf", "Portable Document Format", "pdf", null);
  const mtText = new MimeType("text/pdf", "Portable Document Format", "pdf", null);
  _mimeTypesInst = new MimeTypeArray(mtApp, mtText);
  const members = [mtApp, mtText];
  _pluginsInst = new PluginArray(
    new Plugin("PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("Chrome PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("Chromium PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("Microsoft Edge PDF Viewer", "internal-pdf-viewer", "Portable Document Format", members),
    new Plugin("WebKit built-in PDF", "internal-pdf-viewer", "Portable Document Format", members),
  );
  __mimeStore.get(mtApp).enabledPlugin = _pluginsInst[0];
  __mimeStore.get(mtText).enabledPlugin = _pluginsInst[0];
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__ditingBuildPdfPersona', __ditingBuildPdfPersona);
  a('_mimeTypesInst', () => _mimeTypesInst, (v) => { _mimeTypesInst = v; });
  a('_pluginsInst', () => _pluginsInst, (v) => { _pluginsInst = v; });
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
