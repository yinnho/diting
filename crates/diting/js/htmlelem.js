(function () { // @@SCOPE
// god-file split: bootstrap's real native marker, reached through its
// hidden prop (house style — see ctor_ladders.js) so no cross-file load
// edge appears in the dependency ratchet.
const _markNative = globalThis.__diting_markNative;

globalThis.HTMLElement = {
  HTMLElement: class HTMLElement extends globalThis.Element {
    constructor(nid) {
      if (typeof nid === "number") {
        super(nid);
        return;
      }
      // Registry upgrade: return the live element. A derived constructor
      // returning an object skips the this-binding check entirely, and the
      // object becomes `this` for every constructor above us in the chain —
      // native class fields and Babel's _possibleConstructorReturn both
      // initialize onto it. No super() needed (and none possible: there is
      // no fresh element to initialize).
      if (__ceUpgradeTarget) return __ceUpgradeTarget;
      // Page code constructing a defined custom element. Chrome's registry
      // lets new (customElements.get(name))() mint a detached element that
      // fires connectedCallback when inserted; the constructor returning
      // the cached wrapper keeps identity equal to every later lookup
      // (querySelector would otherwise hand back a different instance).
      const nt = new.target;
      const name = nt && globalThis.customElements && globalThis.customElements._byCtor
        ? globalThis.customElements._byCtor.get(nt)
        : null;
      if (name) {
        const fresh = _wrapEl(+_dom("create_element", String(name).toLowerCase(), "http://www.w3.org/1999/xhtml"));
        if (fresh) {
          Object.setPrototypeOf(fresh, nt.prototype);
          fresh.__customUpgraded = true;
          fresh.__customConnected = false;
          return fresh;
        }
      }
      throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "HTMLElement") + "': Illegal constructor");
    }
  },
}.HTMLElement;

// Real pages discriminate elements per interface — webpack style-loader
// gates style injection on `document.querySelector('head') instanceof
// window.HTMLIFrameElement`, and player/form libs do the same. The old
// `= Element` aliases made every instanceof true for every element —
// bilibili's player core threw "Couldn't find a style target" at boot
// because head passed the iframe test, and the player never mounted. That
// was fixed first with a Symbol.hasInstance tagName check over throwing
// constructor shells sharing Element.prototype (#1001 v0); the remaining
// half (#27): those shared prototypes made constructor.name read "Element"
// for everything and let a patch on HTMLDivElement.prototype leak onto every
// element. Now each interface is a real class extending the real HTMLElement
// above; _elementClassFor routes by tagName through _tagInterfaces, and
// hasInstance stays as a second opinion so wrappers built before this
// block (or cross-realm objects) still classify by tag.
function _htmlInterface(name, tags, attrs, Base) {
  const C = { [name]: class extends (Base || globalThis.HTMLElement) {
    constructor(nid) {
      if (typeof nid !== "number") {
        throw new TypeError("Failed to construct '" + name + "': Illegal constructor");
      }
      super(nid);
    }
  } }[name];
  Object.defineProperty(C, Symbol.hasInstance, {
    value(obj) {
      if (!obj) return false;
      if (!tags.includes(String(obj.tagName || '').toLowerCase())) return false;
      // Namespace-blind matching answered true for svg <a> against
      // HTMLAnchorElement — the HTML interfaces never claim an SVG/MathML
      // element (#28).
      const ns = obj.namespaceURI;
      return ns === undefined || ns === "http://www.w3.org/1999/xhtml";
    },
    configurable: true,
  });
  // (#32/#33/#34) Legacy reflected attributes ([Reflect] DOMString; div/p/h*
  // align, body color family, table families). Getter: absent content attr
  // -> "" (className precedent). Setter null handling is the only thing
  // [LegacyNullToEmptyString] changes — entries are either a plain name
  // (null -> "null") or an object: {name, nullEmpty} (null -> ""),
  // {name, attr} when the IDL name differs from the content-attr name
  // (ch -> "char"), {name, boolean} for reflected booleans (presence is
  // the bit; setter true -> empty value, false -> removeAttribute).
  // undefined -> "undefined" for the string modes.
  //
  // (#35) {name, ulong: [min, max, def]} — [Reflect, ReflectRange,
  // ReflectDefault] unsigned long (colSpan/rowSpan/span). The asymmetry is
  // the trap (WHATWG common-dom-interfaces.html, verified against Chrome
  // 152): the GETTER clamps — parse the content attribute with the HTML
  // non-negative integer rules (skip leading whitespace, optional '+',
  // digits, stop at the first non-digit so "12abc" is 12 and "3.9" is 3);
  // a parsed value outside [min, max] comes back as the boundary, while a
  // missing/garbage attribute comes back as def. The SETTER never clamps:
  // ToUint32 the value (Number(v) >>> 0 does NaN/inf -> 0, truncation and
  // mod 2^32 in one op — Chrome writes "1410065408" for colSpan = 1e10),
  // write it verbatim while it fits in [0, 2^31-1], otherwise write def.
  // {name, keywords: [...]} — enumerated reflect (th scope): getter
  // lowercases and returns the keyword only on an exact match (no
  // whitespace trimming, "auto" is NOT in the set), else ""; setter is the
  // plain verbatim write.
  for (const a of attrs || []) {
    const an = typeof a === 'string' ? a : a.name;
    const cn = (a !== null && typeof a === 'object' && a.attr) || an;
    if (a !== null && typeof a === 'object' && a.ulong) {
      const [umin, umax, udef] = a.ulong;
      Object.defineProperty(C.prototype, an, {
        get() {
          const v = this.getAttribute(cn);
          if (v !== null) {
            const m = /^\s*\+?(\d+)/.exec(v);
            if (m !== null) {
              const p = +m[1];
              if (p >= umin && p <= umax) return p;
              return p < umin ? umin : umax;
            }
          }
          return udef;
        },
        set(v) {
          const x = Number(v) >>> 0;
          this.setAttribute(cn, String(x <= 2147483647 ? x : udef));
        },
        configurable: true,
        enumerable: true,
      });
      continue;
    }
    if (a !== null && typeof a === 'object' && a.keywords) {
      Object.defineProperty(C.prototype, an, {
        get() {
          const v = this.getAttribute(cn);
          if (v === null) return '';
          const lv = v.toLowerCase();
          return a.keywords.indexOf(lv) !== -1 ? lv : '';
        },
        set(v) { this.setAttribute(cn, String(v)); },
        configurable: true,
        enumerable: true,
      });
      continue;
    }
    if (a !== null && typeof a === 'object' && a.boolean) {
      Object.defineProperty(C.prototype, an, {
        get() { return this.hasAttribute(cn); },
        set(v) { if (v) this.setAttribute(cn, ''); else this.removeAttribute(cn); },
        configurable: true,
        enumerable: true,
      });
      continue;
    }
    const nullEmpty = a !== null && typeof a === 'object' && a.nullEmpty;
    Object.defineProperty(C.prototype, an, {
      get() { const v = this.getAttribute(cn); return v === null ? '' : v; },
      set(v) { this.setAttribute(cn, v === null && nullEmpty ? '' : String(v)); },
      configurable: true,
      enumerable: true,
    });
  }
  for (const t of tags) _tagInterfaces.set(t, C);
  return C;
}

globalThis.HTMLDivElement = _htmlInterface('HTMLDivElement', ['div'], ['align']);
globalThis.HTMLSpanElement = _htmlInterface('HTMLSpanElement', ['span']);
globalThis.HTMLParagraphElement = _htmlInterface('HTMLParagraphElement', ['p'], ['align']);
globalThis.HTMLAnchorElement = _htmlInterface('HTMLAnchorElement', ['a']);
globalThis.HTMLImageElement = _htmlInterface('HTMLImageElement', ['img']);
globalThis.HTMLInputElement = _htmlInterface('HTMLInputElement', ['input']);
globalThis.HTMLButtonElement = _htmlInterface('HTMLButtonElement', ['button']);
// (#203) HTMLUnknownElement is its own interface in Chrome — prototype
// below HTMLElement, '[object HTMLUnknownElement]' brand, Illegal
// constructor. It used to be a bare alias of Element, which the brand
// sweep below then obeyed by stamping Element.prototype with THIS name
// (every element answered '[object HTMLUnknownElement]'). Wrapping of
// unlisted known tags (h1, table, …) still routes to HTMLElement, so no
// instance is minted here — the class exists for identity, instanceof
// against the real chain, and the brand.
globalThis.HTMLUnknownElement = {
  HTMLUnknownElement: class HTMLUnknownElement extends globalThis.HTMLElement {
    constructor(nid) {
      if (typeof nid !== "number") {
        throw new TypeError("Failed to construct 'HTMLUnknownElement': Illegal constructor");
      }
      super(nid);
    }
  },
}.HTMLUnknownElement;
globalThis.HTMLFormElement = class HTMLFormElement extends globalThis.HTMLElement {
  get elements() { return __diting_mkCollection(this.querySelectorAll("input, select, textarea, button, fieldset, output, object"), 'controls'); }
  get length() { return this.elements.length; }
  // Inherit submit() from Element.prototype: it dispatches the cancelable
  // 'submit' event and (if not prevented) builds form data and navigates.
  // Chrome reset(): the cancelable 'reset' event fires first (a listener
  // can veto), then every listed control restores its PARSED default — the
  // value attribute for inputs/buttons, the child text for textareas, the
  // selected attribute for a select's options, the checked attribute for
  // checkables. The old `f.value = ''` did neither: it skipped the event
  // and wiped defaults Chrome never wipes.
  reset() {
    const cancelled = !this.dispatchEvent(new Event('reset', { bubbles: true, cancelable: true }));
    if (cancelled) return;
    for (const f of this.elements) {
      const tag = f.localName;
      if (tag === 'select') {
        // Direct _selected writes like selectedIndex's: restore every
        // option to its parsed default, then mirror the label once. (The
        // checkable arm below goes through the checked SETTER, whose own
        // mirror rides along.)
        for (const o of f.options) o._selected = o.hasAttribute('selected');
        _mirrorSelectLabel(f);
        continue;
      }
      if (tag === 'input') {
        const type = (f.getAttribute('type') || '').toLowerCase();
        if (type === 'checkbox' || type === 'radio') { f.checked = f.hasAttribute('checked'); continue; }
      }
      if ('value' in f) {
        f.value = tag === 'textarea'
          ? (f.textContent || '')
          : (f.getAttribute('value') !== null ? f.getAttribute('value') : '');
      }
    }
  }
};
// HTMLFormElement named property access (`form.q` → the control named q, by
// id or name — the spec's past-names map, minus the map's historical bits).
// Real props/methods always win (Reflect.get resolves first), so a control
// named "submit" can't shadow form.submit — same precedence as Chrome.
// Surfaced by the hosted live-view page: an inline onclick reading
// document.forms[0].q.value threw, because the forms *collection* shipped
// without the form element's own named access. Applied per-form at wrap time
// (forms are rare, so the per-instance Proxy costs nothing at scale).
const _formNamedProxy = {
  get(t, k, r) {
    const v = Reflect.get(t, k, r);
    if (v !== undefined || typeof k !== "string") return v;
    return t.elements ? (t.elements.namedItem(k) || undefined) : undefined;
  },
  has(t, k) {
    if (Reflect.has(t, k)) return true;
    return typeof k === "string" && !!(t.elements && t.elements.namedItem(k));
  },
};
globalThis.HTMLSelectElement = _htmlInterface('HTMLSelectElement', ['select']);
globalThis.HTMLTextAreaElement = _htmlInterface('HTMLTextAreaElement', ['textarea']);
globalThis.HTMLLabelElement = _htmlInterface('HTMLLabelElement', ['label']);

globalThis.HTMLTableElement = _htmlInterface('HTMLTableElement', ['table'], [
  'align', 'border', 'frame', 'rules', 'summary', 'width',
  { name: 'bgColor', nullEmpty: true },
  { name: 'cellPadding', nullEmpty: true },
  { name: 'cellSpacing', nullEmpty: true },
]);
globalThis.HTMLTableRowElement = _htmlInterface('HTMLTableRowElement', ['tr'], [
  'align', { name: 'ch', attr: 'char' }, { name: 'chOff', attr: 'charoff' },
  'vAlign', { name: 'bgColor', nullEmpty: true },
]);
globalThis.HTMLTableCellElement = _htmlInterface('HTMLTableCellElement', ['td', 'th'], [
  'align', 'axis', 'height', 'width', 'headers', 'abbr',
  { name: 'ch', attr: 'char' }, { name: 'chOff', attr: 'charoff' },
  { name: 'noWrap', boolean: true }, 'vAlign',
  { name: 'bgColor', nullEmpty: true },
  { name: 'colSpan', ulong: [1, 1000, 1] },
  { name: 'rowSpan', ulong: [0, 65534, 1] },
  { name: 'scope', keywords: ['row', 'col', 'rowgroup', 'colgroup'] },
]);
globalThis.HTMLTableSectionElement = _htmlInterface('HTMLTableSectionElement', ['thead', 'tbody', 'tfoot'], [
  'align', { name: 'ch', attr: 'char' }, { name: 'chOff', attr: 'charoff' }, 'vAlign',
]);
// (#35) col and colgroup share HTMLTableColElement in Chrome (same face on
// both): the ulong span plus the obsolete.html legacy string family.
globalThis.HTMLTableColElement = _htmlInterface('HTMLTableColElement', ['col', 'colgroup'], [
  'align', { name: 'ch', attr: 'char' }, { name: 'chOff', attr: 'charoff' },
  'vAlign', 'width',
  { name: 'span', ulong: [1, 1000, 1] },
]);
globalThis.HTMLTableCaptionElement = _htmlInterface('HTMLTableCaptionElement', ['caption']);
globalThis.HTMLOptGroupElement = _htmlInterface('HTMLOptGroupElement', ['optgroup']);
globalThis.HTMLIFrameElement = _htmlInterface('HTMLIFrameElement', ['iframe']);
globalThis.HTMLCanvasElement = _htmlInterface('HTMLCanvasElement', ['canvas']);
// HTMLMediaElement — Chrome's abstract layer for every media-state face
// (paused/readyState/duration/play/…). #190: that face used to sit on
// Element.prototype, so EVERY element answered `readyState === 0`; baidu's
// sbase `assetOnload` gate — `/loaded|complete|undefined/.test(node.readyState)`,
// written when script elements had no readyState at all — never opened, and
// every dynamically-loaded superman component timed out 10s in. Defined by
// hand rather than `_htmlInterface`: an interface with no tags would install
// a Symbol.hasInstance that answers false for everything, and
// `video instanceof HTMLMediaElement` must ride the real prototype chain.
globalThis.HTMLMediaElement = class HTMLMediaElement extends globalThis.HTMLElement {
  constructor(nid) {
    if (typeof nid !== "number") {
      throw new TypeError("Failed to construct 'HTMLMediaElement': Illegal constructor");
    }
    super(nid);
  }
};
globalThis.HTMLVideoElement = _htmlInterface('HTMLVideoElement', ['video'], null, globalThis.HTMLMediaElement);
globalThis.HTMLAudioElement = _htmlInterface('HTMLAudioElement', ['audio'], null, globalThis.HTMLMediaElement);
globalThis.HTMLScriptElement = _htmlInterface('HTMLScriptElement', ['script']);
globalThis.HTMLStyleElement = _htmlInterface('HTMLStyleElement', ['style']);
globalThis.HTMLLinkElement = _htmlInterface('HTMLLinkElement', ['link']);
globalThis.HTMLMetaElement = _htmlInterface('HTMLMetaElement', ['meta']);
globalThis.HTMLHeadElement = _htmlInterface('HTMLHeadElement', ['head']);
globalThis.HTMLBodyElement = _htmlInterface('HTMLBodyElement', ['body'], [
  'background',
  { name: 'text', nullEmpty: true }, { name: 'link', nullEmpty: true },
  { name: 'vLink', nullEmpty: true }, { name: 'aLink', nullEmpty: true },
  { name: 'bgColor', nullEmpty: true },
]);
globalThis.HTMLHtmlElement = _htmlInterface('HTMLHtmlElement', ['html']);
globalThis.HTMLBRElement = _htmlInterface('HTMLBRElement', ['br']);
globalThis.HTMLHRElement = _htmlInterface('HTMLHRElement', ['hr']);
globalThis.HTMLUListElement = _htmlInterface('HTMLUListElement', ['ul']);
globalThis.HTMLOListElement = _htmlInterface('HTMLOListElement', ['ol']);
globalThis.HTMLLIElement = _htmlInterface('HTMLLIElement', ['li']);
globalThis.HTMLPreElement = _htmlInterface('HTMLPreElement', ['pre']);
globalThis.HTMLHeadingElement = _htmlInterface('HTMLHeadingElement', ['h1', 'h2', 'h3', 'h4', 'h5', 'h6'], ['align']);
globalThis.HTMLTemplateElement = _htmlInterface('HTMLTemplateElement', ['template']);
globalThis.HTMLSlotElement = _htmlInterface('HTMLSlotElement', ['slot']);
// Slot assignment over native shadow trees: the Rust tree computes the
// assignment (exact names, first same-name slot in shadow-tree order wins,
// fallback children when nothing is assigned). "null" from the op means
// "not a slot in a shadow tree" — spec: no assignment AND no fallback —
// while "[]" means in-shadow-but-unassigned, where the fallback applies.
// flatten walks nested slots here in JS with a seen-set guard.
HTMLSlotElement.prototype.assignedNodes = function assignedNodes(opts) {
  const flatten = !!(opts && opts.flatten);
  const out = [];
  const seen = new Set();
  const walk = (el) => {
    if (seen.has(el)) return;
    seen.add(el);
    const raw = _domParse("assigned_nodes", el._nid);
    let kids;
    if (raw === null || raw === undefined) kids = [];
    else if (raw.length) kids = raw;
    else kids = Array.from(el.childNodes || []).map((c) => c._nid);
    for (const kid of kids) {
      const w = _wrap(kid);
      if (!w) continue;
      if (flatten && w.nodeType === 1 && w.localName === 'slot') walk(w);
      else out.push(w);
    }
  };
  walk(this);
  return out;
};
HTMLSlotElement.prototype.assignedElements = function assignedElements(opts) {
  return this.assignedNodes(opts).filter((n) => n.nodeType === 1);
};
_markNative(HTMLSlotElement.prototype.assignedElements);
_markNative(HTMLSlotElement.prototype.assignedNodes);

globalThis.HTMLOptionElement = _htmlInterface('HTMLOptionElement', ['option']);
// (#98) The .text IDL attributes Chrome exposes on script/anchor/option.
// PDD's goods-list tracker scan reads `el.text` on every script element
// without a src — external scripts have no children, so Chrome answers ""
// where we answered undefined, and its `.indexOf` on that threw inside the
// tracker inventory (2026-10-12). Getter/setter ride textContent, which is
// the observable string for real markup (spec excludes script/style
// descendants from option.text; those never occur inside options).
for (const _if of [HTMLScriptElement, HTMLAnchorElement, HTMLOptionElement]) {
  Object.defineProperty(_if.prototype, 'text', {
    get() { return this.textContent; },
    set(v) { this.textContent = v; },
    configurable: true,
  });
}
globalThis.HTMLDataListElement = _htmlInterface('HTMLDataListElement', ['datalist']);
globalThis.HTMLFieldSetElement = _htmlInterface('HTMLFieldSetElement', ['fieldset']);
globalThis.HTMLLegendElement = _htmlInterface('HTMLLegendElement', ['legend']);
globalThis.HTMLProgressElement = _htmlInterface('HTMLProgressElement', ['progress']);
globalThis.HTMLDetailsElement = _htmlInterface('HTMLDetailsElement', ['details']);
globalThis.HTMLDialogElement = _htmlInterface('HTMLDialogElement', ['dialog']);


// (#27 → 刀55) Element used to carry the whole kitchen sink of accessors
// (everything the bootstrap reflects lived on Element.prototype), but Chrome
// spreads them across the interfaces — `indeterminate` on
// HTMLInputElement.prototype, `src` on HTMLImageElement.prototype, table
// geometry on the table family. Each interface owns its level; code that
// looks up a property ON the interface (getOwnPropertyDescriptor, for-in
// over a direct prototype, zone.js patchProperty) finds it there. 刀55
// inverted the old MIRROR into a MOVE: the same descriptors (same
// getter/setter functions) are defined one level down — behavior on the
// owning tags and bootstrap-internal reads are untouched — and then
// element_sink.js (after the platform bundle) DELETES them from
// Element.prototype, where Chrome has none of them (a <div> used to answer
// .value/.rows/.href with guard-rejected ghosts).
{
  const _interfaceProps = {
    HTMLAnchorElement: ['href', 'hash', 'host', 'hostname', 'origin', 'password', 'pathname', 'port', 'protocol', 'search', 'username', 'rel', 'rev', 'relList', 'type', 'name'],
    HTMLImageElement: ['src', 'sizes'],
    HTMLInputElement: ['checked', 'indeterminate', 'value', 'files', 'form', 'disabled', 'name', 'type', 'placeholder', 'max', 'min', 'step', 'multiple', 'size', 'maxLength', 'minLength', 'valueAsDate', 'valueAsNumber'],
    HTMLSelectElement: ['selectedIndex', 'selectedOptions', 'options', 'value', 'form', 'disabled', 'name', 'type', 'multiple'],
    HTMLOptionElement: ['selected', 'value', 'form', 'disabled'],
    HTMLTextAreaElement: ['value', 'placeholder', 'form', 'disabled', 'name', 'type', 'maxLength', 'minLength', 'rows', 'cols', 'wrap'],
    HTMLButtonElement: ['form', 'disabled', 'name', 'type', 'value'],
    HTMLTableElement: ['caption', 'rows', 'tBodies', 'tFoot', 'tHead', 'createCaption', 'createTBody', 'createTFoot', 'createTHead', 'deleteCaption', 'deleteRow', 'deleteTFoot', 'deleteTHead', 'insertRow'],
    HTMLTableRowElement: ['cells', 'rowIndex', 'sectionRowIndex', 'deleteCell', 'insertCell'],
    HTMLTableSectionElement: ['rows', 'deleteRow', 'insertRow'],
    HTMLTableCellElement: ['cellIndex'],
    HTMLLinkElement: ['rel', 'rev', 'relList', 'sizes', 'disabled', 'sheet'],
    HTMLStyleElement: ['sheet'],
    HTMLMetaElement: ['content', 'scheme'],
    HTMLLabelElement: ['htmlFor', 'form', 'control'],
    HTMLDataListElement: ['options'],
    HTMLTemplateElement: ['content'],
    HTMLIFrameElement: ['sandbox', 'contentDocument', 'contentWindow', 'src', 'srcdoc'],
    HTMLDialogElement: ['open', 'returnValue', 'closedBy', 'close', 'requestClose', 'show', 'showModal'],
    HTMLFieldSetElement: ['disabled', 'form', 'name', 'type'],
    HTMLProgressElement: ['max', 'value'],
    HTMLMeterElement: ['max', 'min', 'value'],
  };
  // 刀85: these five land Chrome-ro (shape-audit kind diff) — the Element
  // descriptor is copied WITHOUT its set leg; none of those setters did
  // anything for these tags anyway, so sloppy writes now no-op like a
  // Chrome readonly attribute. Defining content here also keeps the
  // htmlreflect gen's delegate() from re-installing the both-legs version.
  const _roOnly = new Set(['HTMLDataListElement.options', 'HTMLSelectElement.options',
    'HTMLTableElement.rows', 'HTMLTableSectionElement.rows', 'HTMLTemplateElement.content']);
  for (const [iname, props] of Object.entries(_interfaceProps)) {
    const C = globalThis[iname];
    if (!C || !C.prototype) continue;
    for (const p of props) {
      if (Object.prototype.hasOwnProperty.call(C.prototype, p)) continue;
      const d = Object.getOwnPropertyDescriptor(globalThis.Element.prototype, p);
      if (!d) continue;
      if (_roOnly.has(iname + '.' + p) && d.get) Object.defineProperty(C.prototype, p, { get: d.get, enumerable: d.enumerable, configurable: d.configurable });
      else Object.defineProperty(C.prototype, p, d);
    }
  }
}

// (#28 → M4 刀72) Real SVG interface chain. The #28 comment here claimed Chrome
// routes SVG through HTMLElement; the Chrome 145 captures say otherwise: the
// chain is SVGSVGElement → SVGGraphicsElement → SVGElement → Element (no
// HTMLElement level — `svg.hidden`, `svg.click`, `svg.innerText`, `svg.title`
// are all undefined there, and `svg instanceof HTMLElement` is false). SVGElement
// owns the GlobalEventHandlers/dataset/style/focus members itself. The
// constructor is the same node-id gate the HTML classes have, minus the
// custom-element faces (there are no SVG custom elements), and names the
// interface actually constructed in its Illegal constructor message.
globalThis.SVGElement = class SVGElement extends globalThis.Element {
  constructor(nid) {
    if (typeof nid === "number") { super(nid); return; }
    throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "SVGElement") + "': Illegal constructor");
  }
};

// (#184) Pages may legally replace the interface globals — the
// webcomponents-era HTMLElement "native shim"
// (`function(){return Reflect.construct(e,[],this.constructor)}`) is the
// canonical shape, and github.com installs one. That shim forwards NO
// constructor arguments, so once the global is swapped, `_wrapEl` routing
// through it constructs the gate with `undefined` and every wrapper mint
// for an unknown/custom tag throws "Failed to construct 'HTMLElement':
// Illegal constructor" (github's hydration retried it hundreds of times).
// Real Chrome never consults the global for its internal wrappers, and
// neither do we: internal wrapping binds the bootstrap-time classes.

// Real Chrome never consults the global for its internal wrappers, and
// neither do we: internal wrapping binds the split-time classes. The three
// hoisted vars became this hidden map; domcore's _elementClassFor/_wrap read
// it through && guards, so pre-install calls (domcore loads first) fall to
// `Element` exactly like the old hoisted-undefined guards did.
Object.defineProperty(globalThis, '__diting_domNative', { value: {
  HTMLElement: globalThis.HTMLElement,
  HTMLFormElement: globalThis.HTMLFormElement,
  SVGElement: globalThis.SVGElement,
}, writable: true, configurable: true, enumerable: false });

// API-tamper probes check these globals exist with the listed methods on
// their prototypes (e.g. `SVGTextContentElement.prototype.getExtentOfChar`).
globalThis.SVGGraphicsElement = class SVGGraphicsElement extends globalThis.SVGElement {
  getBBox() { return { x: 0, y: 0, width: 0, height: 0 }; }
  getScreenCTM() { return null; }
  getCTM() { return null; }
};
// Chrome sits geometry between graphics and the shapes (path instanceof
// SVGGeometryElement). 批240: getTotalLength/getPointAtLength measure the
// shape in user units through the Rust-side PathMeasure — ovals as Skia's
// exact conic chord measure, explicit d beziers analytically (Chrome's own
// numbers, not the analytic perimeters).
// 刀59: storage moved to the hidden _ps object — x/y are prototype
// accessors now (Chrome truth rw:ec), and matrixTransform COMPUTES
// (installed in svg_root.js; the old stub returned `this`). Sharing one
// _ps object is how currentTranslate keeps writes across fresh wrappers.
globalThis.SVGPoint = class SVGPoint {
  constructor(x = 0, y = 0) { this._ps = { x: Number(x) || 0, y: Number(y) || 0 }; }
};
globalThis.SVGGeometryElement = class SVGGeometryElement extends globalThis.SVGGraphicsElement {
  getTotalLength() { return _OPS.op_svg_path_len(this._nid | 0); }
  getPointAtLength(len) {
    const r = String(_OPS.op_svg_path_point(this._nid | 0, Number(len) || 0));
    const z = r.indexOf("\0");
    // 刀91: minted through __diting_svgPoint — the SVGPoint global is an
    // always-throwing ctor now (Chrome: `new SVGPoint()` is Illegal
    // constructor for every argument list).
    return globalThis.__diting_svgPoint({ x: z < 0 ? 0 : Number(r.slice(0, z)), y: z < 0 ? 0 : Number(r.slice(z + 1)) });
  }
};
globalThis.SVGSVGElement = class SVGSVGElement extends globalThis.SVGGraphicsElement {};
globalThis.SVGTextContentElement = class SVGTextContentElement extends globalThis.SVGGraphicsElement {
  getExtentOfChar() { return { x: 0, y: 0, width: 0, height: 0 }; }
  getSubStringLength() { return 0; }
  getComputedTextLength() { return 0; }
};
globalThis.SVGTextPositioningElement = class SVGTextPositioningElement extends globalThis.SVGTextContentElement {};
globalThis.SVGAnimationElement = class SVGAnimationElement extends globalThis.SVGElement {};
globalThis.SVGFEDropShadowElement = class SVGFEDropShadowElement extends globalThis.SVGElement {};
// (#28) Element interfaces for the common SVG tags, registered by exact
// local name in _svgTagInterfaces (see _elementClassFor). Computed-name
// classes keep constructor.name; bases mirror Chrome's chains.
(() => {
  const defs = [
    // [name, base, [local names]]
    ["SVGGElement", SVGGraphicsElement, ["g"]],
    ["SVGDefsElement", SVGGraphicsElement, ["defs"]],
    ["SVGSwitchElement", SVGGraphicsElement, ["switch"]],
    ["SVGImageElement", SVGGraphicsElement, ["image"]],
    ["SVGUseElement", SVGGraphicsElement, ["use"]],
    ["SVGAElement", SVGGraphicsElement, ["a"]],
    ["SVGForeignObjectElement", SVGGraphicsElement, ["foreignObject"]],
    ["SVGClipPathElement", SVGElement, ["clipPath"]], // Chrome 145: SVGElement, NOT SVGGraphicsElement (no getBBox/getCTM)
    ["SVGPathElement", SVGGeometryElement, ["path"]],
    ["SVGRectElement", SVGGeometryElement, ["rect"]],
    ["SVGCircleElement", SVGGeometryElement, ["circle"]],
    ["SVGEllipseElement", SVGGeometryElement, ["ellipse"]],
    ["SVGLineElement", SVGGeometryElement, ["line"]],
    ["SVGPolylineElement", SVGGeometryElement, ["polyline"]],
    ["SVGPolygonElement", SVGGeometryElement, ["polygon"]],
    ["SVGTextElement", SVGTextPositioningElement, ["text"]],
    ["SVGTSpanElement", SVGTextPositioningElement, ["tspan"]],
    ["SVGAnimateElement", SVGAnimationElement, ["animate", "set", "animateMotion", "animateTransform"]],
    ["SVGFEDropShadowElement", SVGFEDropShadowElement, ["feDropShadow"]],
    ["SVGTitleElement", SVGElement, ["title"]],
    ["SVGDescElement", SVGElement, ["desc"]],
    ["SVGStyleElement", SVGElement, ["style"]],
    ["SVGSymbolElement", SVGGraphicsElement, ["symbol"]], // Chrome 145: graphics (transform/getBBox/getCTM/nearestViewportElement)
    ["SVGMarkerElement", SVGElement, ["marker"]],
    ["SVGPatternElement", SVGElement, ["pattern"]],
    ["SVGStopElement", SVGElement, ["stop"]],
  ];
  for (const [name, Base, tags] of defs) {
    const C = { [name]: class extends Base {} }[name];
    globalThis[name] = C;
    for (const t of tags) globalThis.__diting_svgTagMap.set(t, C);
  }
  // Gradients: linear/radial share the SVGGradientElement level (Chrome).
  const Grad = { SVGGradientElement: class extends globalThis.SVGElement {} }.SVGGradientElement;
  globalThis.SVGGradientElement = Grad;
  const lin = { SVGLinearGradientElement: class extends Grad {} }.SVGLinearGradientElement;
  const rad = { SVGRadialGradientElement: class extends Grad {} }.SVGRadialGradientElement;
  globalThis.SVGLinearGradientElement = lin;
  globalThis.SVGRadialGradientElement = rad;
  globalThis.__diting_svgTagMap.set("linearGradient", lin);
  globalThis.__diting_svgTagMap.set("radialGradient", rad);
  // The svg root itself.
  globalThis.__diting_svgTagMap.set("svg", globalThis.SVGSVGElement);
})();

// (#117) Canvas-only methods live on HTMLCanvasElement.prototype in Chrome —
// they used to sit on Element.prototype, so `'toDataURL' in document.body`
// was true and every non-canvas element answered getContext. Both are bot
// tells; canvas elements still resolve them through the prototype chain.
// god-file split (canvas2d.js): the implementations live there; these thin
// call-ins keep the load-time install, name/arity and native marking here.
HTMLCanvasElement.prototype.getContext = function getContext(type) { return __diting_canvasGetContext.apply(this, arguments); };
HTMLCanvasElement.prototype.toDataURL = function toDataURL(type, quality) { return __diting_canvasToDataURL.call(this, type, quality); };
HTMLCanvasElement.prototype.toBlob = function toBlob(cb, type, quality) { return __diting_canvasToBlob.call(this, cb, type, quality); };
// (#164) canvas.width/height are reflected IDL attributes (default 300/150,
// Chrome cap 32767) that also resize the live 2D buffer. qrcode.react sizes
// the canvas AFTER first paint (s.height = s.width = n*g) — with the old
// stored-property stub the element grew but the drawing surface stayed at
// the getContext-time snapshot.
(function() {
  for (const dim of ['width', 'height']) {
    const fallback = dim === 'width' ? 300 : 150;
    Object.defineProperty(HTMLCanvasElement.prototype, dim, {
      configurable: true,
      get() {
        const v = parseInt(this.getAttribute(dim), 10);
        return (Number.isFinite(v) && v >= 0) ? v : fallback;
      },
      set(v) {
        v = Math.floor(Number(v));
        if (!Number.isFinite(v) || v < 0) v = 0;
        if (v > 32767) v = 32767;
        this.setAttribute(dim, String(v));
        const ctx = this._ctx;
        if (ctx && ctx._resize) {
          ctx._resize(dim === 'width' ? v : this.width,
                      dim === 'height' ? v : this.height);
        }
      },
    });
  }
})();

// Media element state — lives on HTMLMediaElement.prototype (Chrome's home
// for it) so generic elements stop answering media questions: the mount path
// still reads these off the <video> the player just created through
// inheritance; `video.play()` returning undefined (players do
// `video.play().catch(...)`) crashed init on bilibili before these.
Object.defineProperties(HTMLMediaElement.prototype, {
  paused: { configurable: true, get() { return this._playing !== true; } },
  ended: { configurable: true, get() { return false; } },
  seeking: { configurable: true, get() { return false; } },
  duration: { configurable: true, get() { return this._mediaDuration ?? (this._mediaSource ? this._mediaSource.duration : NaN); }, set(v) { this._mediaDuration = Number(v); } },
  currentTime: { configurable: true, get() { return this._currentTime || 0; }, set(v) { this._currentTime = Number(v) || 0; } },
  playbackRate: { configurable: true, get() { return this._playbackRate || 1; }, set(v) { this._playbackRate = Number(v) || 1; } },
  readyState: { configurable: true, get() { return this._mediaReadyState || 0; } },
  networkState: { configurable: true, get() { return this._mediaNetworkState || 0; } },
  buffered: { configurable: true, get() { return this._mediaSource ? _mediaRanges(this._mediaSource._unionBuffered()) : _EMPTY_RANGES; } },
  played: { configurable: true, get() { return _EMPTY_RANGES; } },
  seekable: { configurable: true, get() {
    if (!this._mediaSource) return _EMPTY_RANGES;
    const d = this._mediaSource.duration;
    return Number.isFinite(d) && d > 0 ? _mediaRanges([[0, d]]) : _EMPTY_RANGES;
  } },
});
// HTMLMediaElement readiness/network constants — Chrome exposes them on
// instances (players write `video.readyState === video.HAVE_ENOUGH_DATA`);
// keep them non-enumerable like native.
// (刀76) Chrome 145 carries all nine on the prototype AND the constructor (e--); the prototype copy installs
// configurable so the shape pass can order it (flag pass seals it), the static copy is final.
for (const [k, v] of Object.entries({ HAVE_NOTHING: 0, HAVE_METADATA: 1, HAVE_CURRENT_DATA: 2, HAVE_FUTURE_DATA: 3, HAVE_ENOUGH_DATA: 4,
    NETWORK_EMPTY: 0, NETWORK_IDLE: 1, NETWORK_LOADING: 2, NETWORK_NO_SOURCE: 3 })) {
  Object.defineProperty(HTMLMediaElement.prototype, k, { value: v, enumerable: true, configurable: true });
}
// statics in Chrome's own order (NETWORK_* before HAVE_*)
for (const [k, v] of Object.entries({ NETWORK_EMPTY: 0, NETWORK_IDLE: 1, NETWORK_LOADING: 2, NETWORK_NO_SOURCE: 3,
    HAVE_NOTHING: 0, HAVE_METADATA: 1, HAVE_CURRENT_DATA: 2, HAVE_FUTURE_DATA: 3, HAVE_ENOUGH_DATA: 4 })) {
  Object.defineProperty(HTMLMediaElement, k, { value: v, enumerable: true });
}
HTMLMediaElement.prototype.play = function() {
  const wasPaused = this._playing !== true;
  __def(this, '_playing', true);
  if (wasPaused) {
    setTimeout(() => {
      try { this.dispatchEvent(new Event('play')); this.dispatchEvent(new Event('playing')); } catch (_) {}
    }, 0);
  }
  return Promise.resolve();
};
HTMLMediaElement.prototype.pause = function() {
  const wasPlaying = this._playing === true;
  this._playing = false;
  if (wasPlaying) {
    setTimeout(() => { try { this.dispatchEvent(new Event('pause')); } catch (_) {} }, 0);
  }
};
HTMLMediaElement.prototype.load = function() {};
_markNative(HTMLMediaElement.prototype.play);
_markNative(HTMLMediaElement.prototype.pause);
_markNative(HTMLMediaElement.prototype.load);

_markNative(HTMLCanvasElement.prototype.getContext);
_markNative(HTMLCanvasElement.prototype.toDataURL);
_markNative(HTMLCanvasElement.prototype.toBlob);

// selectionStart/End/Direction live on the two text-entry interfaces in
// Chrome (刀55: they left Element.prototype with the rest of the sink).
for (const _C of [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement]) if (_C && !_C.prototype.selectionStart) {
  Object.defineProperty(_C.prototype, 'selectionStart', {
    get: function() {
      return _ns_selectionStart.get(this) ?? null;
    },
    set: function(v) {
      _ns_selectionStart.set(this, v == null ? null : Math.max(0, parseInt(v, 10) || 0));
      _ns_mirrorSelection(this);
    },
    enumerable: true,
    configurable: true
  });
}

for (const _C of [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement]) if (_C && !_C.prototype.selectionEnd) {
  Object.defineProperty(_C.prototype, 'selectionEnd', {
    get: function() {
      return _ns_selectionEnd.get(this) ?? null;
    },
    set: function(v) {
      _ns_selectionEnd.set(this, v == null ? null : Math.max(0, parseInt(v, 10) || 0));
      _ns_mirrorSelection(this);
    },
    enumerable: true,
    configurable: true
  });
}

for (const _C of [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement]) if (_C && !_C.prototype.selectionDirection) {
  Object.defineProperty(_C.prototype, 'selectionDirection', {
    get: function() {
      return _ns_selectionDir.get(this) ?? 'none';
    },
    set: function(v) {
      _ns_selectionDir.set(this, v === 'forward' || v === 'backward' ? v : 'none');
    },
    enumerable: true,
    configurable: true
  });
}

// setSelectionRange - set text selection range (input/textarea, 刀55)
for (const _C of [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement]) if (_C && !_C.prototype.setSelectionRange) {
  _C.prototype.setSelectionRange = function setSelectionRange(start, end) {
    let direction = arguments[2];
    start = Math.max(0, parseInt(start, 10) || 0);
    end = Math.max(0, parseInt(end, 10) || 0);
    direction = direction === 'forward' || direction === 'backward' ? direction : 'none';
    _ns_selectionStart.set(this, start);
    _ns_selectionEnd.set(this, end);
    _ns_selectionDir.set(this, direction);
    _ns_mirrorSelection(this);
  };
  _markNative(_C.prototype.setSelectionRange);
}

// setRangeText - replace selection with text (input/textarea, 刀55)
for (const _C of [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement]) if (_C && !_C.prototype.setRangeText) {
  _C.prototype.setRangeText = function setRangeText(replacement) {
    const val = this.value;
    if (!val) return;
    const strVal = String(val);
    const start = arguments[1] === undefined ? (this.selectionStart ?? 0) : Math.max(0, parseInt(arguments[1], 10) || 0);
    const end = arguments[2] === undefined ? (this.selectionEnd ?? 0) : Math.max(0, parseInt(arguments[2], 10) || 0);
    const newValue = strVal.slice(0, start) + String(replacement) + strVal.slice(end);
    this.value = newValue;
    const selectMode = arguments[3] || 'preserve';
    if (selectMode === 'select') {
      const replLen = String(replacement).length;
      _ns_selectionStart.set(this, start);
      _ns_selectionEnd.set(this, start + replLen);
      _ns_selectionDir.set(this, 'none');
    } else if (selectMode === 'start') {
      _ns_selectionStart.set(this, start);
      _ns_selectionEnd.set(this, start);
      _ns_selectionDir.set(this, 'none');
    } else if (selectMode === 'end') {
      const replLen = String(replacement).length;
      _ns_selectionStart.set(this, start + replLen);
      _ns_selectionEnd.set(this, start + replLen);
      _ns_selectionDir.set(this, 'none');
    }
    _ns_mirrorSelection(this);
  };
  _markNative(_C.prototype.setRangeText);
}

// select - select all text in the element (input/textarea, 刀55)
for (const _C of [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement]) if (_C && !_C.prototype.select) {
  _C.prototype.select = function select() {
    const val = this.value;
    if (val === undefined || val === null) return;
    const len = String(val).length;
    _ns_selectionStart.set(this, 0);
    _ns_selectionEnd.set(this, len);
    _ns_selectionDir.set(this, 'none');
    _ns_mirrorSelection(this);
  };
  _markNative(_C.prototype.select);
}

if (typeof Image === 'undefined') {
  // In a real browser `new Image()` is `document.createElement('img')`, i.e. a
  // full HTMLImageElement. A plain-class shim has no `.style`, so
  // `new Image().style` was `undefined` and libraries that touch it on a
  // detached image threw (upstream issue #350). Build a real element so
  // `.style`, attribute reflection, and event dispatch all come for free.
  // Per-image emulation state lives in a WeakMap, NOT on the instance: Chrome's <img> has no own properties. The prototype
  // accessors for complete/naturalWidth/naturalHeight/width/height/src are wrapped below to consult it.
  const _imgState = new WeakMap();
  Object.defineProperty(globalThis, '__diting_imgState', { value: _imgState, writable: true, enumerable: false, configurable: true });
  globalThis.Image = function Image(width, height) {
    const img = document.createElement('img');
    // complete/naturalWidth/naturalHeight are prototype getter-ONLY
    // accessors now (platform gen/htmlmedia.gen.js, M4 knife 5): internal
    // emulation state writes go through the hidden-slot channel, the same
    // one _playing uses. An own (non-enumerable) slot shadows the getter.
    // #236: width/height are accessors, not data props. The HTML spec has
    // the IDL getter fall back to the natural size when no value was ever
    // set — `new Image()` with a decoded 1×1 src must read 1, not the
    // constructor-era 0 (SDKs gate uploads on image.width and saw a
    // phantom zero). A page or constructor assignment pins the value.
    const st = { complete: true, naturalWidth: 0, naturalHeight: 0, // Chrome: `new Image().complete` is true until a src starts loading
      w: width !== undefined ? (width >>> 0) : null,
      h: height !== undefined ? (height >>> 0) : null, load: null };
    _imgState.set(img, st);
    // There is no real image decoder, so emulate a successful decode: assigning
    // `.src` flips `complete` and fires `load` on a later tick. Lazy loaders
    // and preloaders that create `new Image()`, set `.src`, and wait for
    // `onload` (or addEventListener('load')) would hang forever otherwise.
    // A page-owned own `src` (Booking.com, upstream #394) shadows the prototype accessor, so the emulation simply never runs.
    st.load = function () {
          if (!img.getAttribute('src')) return;
          st.complete = false;
          // (#41) Real load semantics: the fetch attempt decides load vs
          // error. The old stub fired `load` unconditionally, so probes
          // pinning load faces to unroutable hosts read fake success.
          // Same subresource shape as dynamic <script src> (credentials
          // include — image loads carry cookies cross-origin in real
          // browsers); 2xx dispatches `load`, anything else (pre-wire
          // refusal, network error, HTTP error status) dispatches `error`.
          const fullUrl = _resolveUrl(img.getAttribute('src'));
          // dimsJson (from op_image_info): header-parsed natural size, or
          // '{}' when unparsable. Real numbers matter because sites gate on
          // naturalWidth != 0 to decide whether an artifact decoded (the xhs
          // zeus AB SDK retries its PNG artifact 25x and dead-ends on 0).
          // Missing/garbage input keeps the old width/height fallback — the
          // load/error decision (#41: transport status) stays with the
          // caller, this only fills in numbers.
          const succeed = function (dimsJson) {
            st.complete = true;
            try {
              const d = JSON.parse(dimsJson || '{}');
              if (d && d.width > 0) {
                st.naturalWidth = d.width;
                st.naturalHeight = d.height;
              }
            } catch (e) {}
            st.naturalWidth = img.naturalWidth || img.width || 0;
            st.naturalHeight = img.naturalHeight || img.height || 0;
            try { img.dispatchEvent(new Event('load')); } catch (e) {}
          };
          const dimsFromBase64 = function (b64) {
            try { return _OPS.op_image_info(String(b64 || '')); } catch (e) { return '{}'; }
          };
          const fail = function () {
            st.complete = false;
            try { img.dispatchEvent(new Event('error')); } catch (e) {}
          };
          if (fullUrl.startsWith('data:')) {
            // Header-parse the embedded payload — comma may be absent on a
            // malformed data: URL; slice(empty) then fails to parse and the
            // fallback keeps the old behavior.
            const comma = fullUrl.indexOf(',');
            setTimeout(function () {
              succeed(dimsFromBase64(comma < 0 ? '' : fullUrl.slice(comma + 1)));
            }, 0);
          } else if (fullUrl.startsWith('blob:')) {
            setTimeout(function () {
              const b = globalThis.__blobObjs && globalThis.__blobObjs[fullUrl];
              if (!(b && b._bytes instanceof Uint8Array)) { fail(); return; }
              // #52: identical bytes parse fine through a data: URL, but the
              // blob: branch used to dispatch `load` with 0×0 dims — upload
              // pipelines that probe dimensions via createObjectURL (the xhs
              // publish page) dead-end silently on width=0/ratio=NaN and
              // never fire the upload request. Bytes → base64 in 32k chunks
              // (same as createImageBitmap) and reuse the shared parser.
              let bin = '';
              const CHUNK = 0x8000;
              for (let i = 0; i < b._bytes.length; i += CHUNK) {
                bin += String.fromCharCode.apply(null, b._bytes.subarray(i, i + CHUNK));
              }
              let dimsJson = '{}';
              try { dimsJson = _OPS.op_image_info(btoa(bin)); } catch (e) {}
              succeed(dimsJson);
            }, 0);
          } else {
            const pageOrigin = (function() { try { const u = new URL(_domParse("document_url") || "about:blank"); return u.origin; } catch(e) { return ""; } })();
            Promise.resolve().then(function () {
              return _OPS.op_fetch_url(fullUrl, "GET", "{}", "", pageOrigin, "no-cors", "include", "\u0000about:client");
            }).then(function (raw) {
              let ok = false;
              let bodyB64 = '';
              try { const parsed = JSON.parse(raw); ok = parsed.status >= 200 && parsed.status <= 299; bodyB64 = parsed.bodyBase64 || ''; } catch (e) {}
              if (ok) { succeed(dimsFromBase64(bodyB64)); } else { fail(); }
            }).catch(fail);
          }
    };
    return img;
  };
  globalThis.Image.prototype = globalThis.HTMLImageElement.prototype;
}

// `new Audio(url)` is `document.createElement('audio')` with preload=auto and src (Chrome); like Image it builds a real element.
if (typeof Audio === 'undefined') {
  globalThis.Audio = function Audio(src) {
    if (new.target === undefined) throw new TypeError("Failed to construct 'Audio': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    const a = document.createElement('audio');
    a.setAttribute('preload', 'auto');
    if (src !== undefined) a.setAttribute('src', String(src));
    return a;
  };
  globalThis.Audio.prototype = globalThis.HTMLAudioElement.prototype;
}

// `new Option(text, value, defaultSelected, selected)` — the legacy factory for
// <option>. Same shape as `new Image()`: build a real element so value/selected
// semantics above and select.add()/appendChild all work. Framework form code
// (glama.ai admin) populates selects this way; `Option is not defined` killed
// its hydration. option.text mirrors textContent (instance-level: parsed
// <option> elements keep their existing surface).
if (typeof Option === 'undefined') {
  globalThis.Option = function Option(text, value, defaultSelected, selected) {
    const opt = document.createElement('option');
    if (text !== undefined && text !== null) opt.textContent = String(text);
    if (value !== undefined && value !== null) opt.setAttribute('value', String(value));
    if (defaultSelected) opt.setAttribute('selected', '');
    if (selected) opt.selected = true;
    return opt;
  };
  globalThis.Option.prototype = globalThis.HTMLOptionElement ? globalThis.HTMLOptionElement.prototype : (globalThis.HTMLElement ? globalThis.HTMLElement.prototype : globalThis.Element.prototype);
}

// (#37) The window-reflecting on* family on HTMLBodyElement. Chrome 152:
// 24 handler names whose body accessors ARE the window's — one shared
// slot, identity both ways (body.X = f ⇔ window.X === f), non-callables
// store null, descriptors enumerable+configurable. The `<body onX="...">`
// content attribute for these names installs as the WINDOW's handler:
// both getters return the same compiled function and it fires when the
// event dispatches on window — which is why the Rust load path can stop
// special-casing `<body onload>` and just dispatch. The six names
// Element.prototype already carries (blur/error/focus/load/resize/
// scroll) get shadowed here, exactly like Chrome's prototype chain.
const _BODY_WINDOW_EVENT_NAMES = [
  'afterprint', 'beforeprint', 'beforeunload', 'blur', 'error', 'focus',
  'gamepadconnected', 'gamepaddisconnected', 'hashchange', 'languagechange',
  'load', 'message', 'messageerror', 'offline', 'online', 'pagehide',
  'pageshow', 'popstate', 'rejectionhandled', 'resize', 'scroll', 'storage',
  'unhandledrejection', 'unload',
];
const _BODY_WINDOW_EVENT_SET = new Set(_BODY_WINDOW_EVENT_NAMES);
function _bodyWindowInline(name) {
  const b = globalThis.document && globalThis.document.body;
  if (b && b.localName === 'body' && typeof b._resolveInlineHandler === 'function') {
    return b._resolveInlineHandler(name);
  }
  return null;
}
function _bodyWindowReflecting(key) {
  return {
    configurable: true,
    enumerable: true,
    get() {
      const store = globalThis.__windowOnHandlers;
      if (store && Object.prototype.hasOwnProperty.call(store, key)) return store[key];
      return _bodyWindowInline(key);
    },
    set(fn) {
      if (typeof fn !== 'function') fn = null;
      const store = globalThis.__windowOnHandlers || (globalThis.__windowOnHandlers = {});
      store[key] = fn;
    },
  };
}
// V8 snapshots cannot hold JS accessors on the global object: every on*
// accessor the loop above installed during snapshot creation comes back
// as a writable data:null property, so at runtime window.on* writes
// landed in dead data props and only the direct onload call ever read
// them. __diting_init re-runs this installer per context (idempotent —
// re-defining the accessors never clears __windowOnHandlers). Data
// properties that hold a real function are left alone: onerror is the
// uncaught-error capture. The body-reflecting members of the family get
// the shared-slot descriptor; the rest get the plain store-backed one.
globalThis.__diting_installWindowOnHandlers = function() {
  for (const _n of _WINDOW_EVENT_NAMES) {
    const _key = 'on' + _n;
    const _cur = Object.getOwnPropertyDescriptor(globalThis, _key);
    if (_cur && !('get' in _cur) && typeof _cur.value === 'function') continue;
    const _d = _BODY_WINDOW_EVENT_SET.has(_n) ? _bodyWindowReflecting(_key) : {
      configurable: true,
      get() {
        const store = globalThis.__windowOnHandlers;
        if (store && Object.prototype.hasOwnProperty.call(store, _key)) return store[_key];
        return null;
      },
      set(fn) {
        if (typeof fn !== 'function') fn = null;
        const store = globalThis.__windowOnHandlers || (globalThis.__windowOnHandlers = {});
        store[_key] = fn;
      },
    };
    _markNative(_d.get);
    _markNative(_d.set);
    Object.defineProperty(globalThis, _key, _d);
  }
};
// Prototype accessors survive the snapshot, so the body side installs at
// top level — each realm's body wrappers share this prototype.
for (const _n of _BODY_WINDOW_EVENT_NAMES) {
  Object.defineProperty(HTMLBodyElement.prototype, 'on' + _n, _bodyWindowReflecting('on' + _n));
}

// The DOM slice of bootstrap's big mark list, re-minted here. Same evaluated
// set as before the split: every entry below was already defined by this
// point in the old single file (the entries that were not — attachShadow,
// the canvas trio — were silently skipped by the typeof guard and are marked
// at their real install sites instead).
[
  HTMLFormElement.prototype.reset,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_BODY_WINDOW_EVENT_SET', _BODY_WINDOW_EVENT_SET);
  d('_bodyWindowInline', _bodyWindowInline);
  d('_formNamedProxy', _formNamedProxy);
  d('_htmlInterface', _htmlInterface);
  d('_markNative', _markNative);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
