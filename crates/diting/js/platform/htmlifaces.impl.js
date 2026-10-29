//! module: htmlifaces
//! layer: L2
//! exposes: HTMLAreaElement, HTMLBaseElement, HTMLDListElement, HTMLDirectoryElement, HTMLEmbedElement, HTMLFontElement, HTMLFrameElement, HTMLMenuElement, HTMLModElement, HTMLParamElement, HTMLQuoteElement, HTMLSourceElement, HTMLTimeElement, HTMLTitleElement, HTMLMapElement, HTMLMarqueeElement, HTMLMeterElement, HTMLObjectElement, HTMLOutputElement, HTMLTrackElement
//! requires-load: legacy:_htmlInterface, legacy:HTMLElement, legacy:HTMLButtonElement
//! requires-call: -

// Element interfaces the engine used to know only as global_surface
// ctor-throw stubs (M4 knife 6). Their tags created plain HTMLElement
// instances, so `document.createElement('source') instanceof HTMLSourceElement`
// was false (Chrome: true) - a type test real players and frameworks make.
// The platform now owns the interface objects BEFORE global_surface runs (its
// THROW loop SKIPs names already present - the Performance/StylePropertyMap
// handover pattern), built with the bootstrap's own _htmlInterface mechanism
// so tag routing (_tagInterfaces) and the Symbol.hasInstance second opinion
// behave exactly like HTMLAnchorElement/HTMLLinkElement. Members arrive from
// gen/htmlreflect.gen.js; tags per the HTML spec's element-interface table.
const DEFS = {
  HTMLAreaElement: ['area'],
  HTMLBaseElement: ['base'],
  HTMLDListElement: ['dl'],
  HTMLDirectoryElement: ['dir'],
  HTMLEmbedElement: ['embed'],
  HTMLFontElement: ['font'],
  HTMLFrameElement: ['frame'],
  HTMLMenuElement: ['menu'],
  HTMLModElement: ['ins', 'del'],
  HTMLParamElement: ['param'],
  HTMLQuoteElement: ['blockquote', 'q'],
  HTMLSourceElement: ['source'],
  HTMLTimeElement: ['time'],
  HTMLTitleElement: ['title'],
  HTMLMapElement: ['map'],
  HTMLMarqueeElement: ['marquee'],
  HTMLMeterElement: ['meter'],
  HTMLObjectElement: ['object'],
  HTMLOutputElement: ['output'],
  HTMLTrackElement: ['track'],
};

for (const name of Object.keys(DEFS)) {
  if (typeof globalThis[name] === 'function') continue; // a legacy script already owns it
  P.expose(name, _htmlInterface(name, DEFS[name]));
}

// Constraint-validation members for the three interfaces form_validation.js
// cannot reach (it runs before this bundle, when HTMLObjectElement/
// HTMLOutputElement did not exist yet): object and output are barred from
// validation (willValidate false, checkValidity true) but carry the API, so
// they borrow the descriptors form_validation installed on <button>. The
// functions are `this`-generic (state(this._el)).
for (const name of ['HTMLObjectElement', 'HTMLOutputElement', 'HTMLFieldSetElement']) {
  const proto = globalThis[name] && globalThis[name].prototype;
  if (!proto || typeof HTMLButtonElement !== 'function') continue;
  for (const m of ['willValidate', 'validity', 'validationMessage', 'checkValidity', 'reportValidity', 'setCustomValidity']) {
    const d = Object.getOwnPropertyDescriptor(HTMLButtonElement.prototype, m);
    if (d && !Object.prototype.hasOwnProperty.call(proto, m)) Object.defineProperty(proto, m, d);
  }
}
