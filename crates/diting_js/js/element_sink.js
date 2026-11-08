// 刀55 (M4): the Element kitchen sink leaves Element.prototype.
//
// For its whole life the engine reflected every HTML/SVG member onto
// Element.prototype with localName guards ("the kitchen sink"), mirroring
// the interesting ones one level down (#27). The whole-surface shape audit
// against Chrome 145 (shape-audit-truth-145.json) says Element.prototype
// carried 94 members Chrome has NOWHERE on Element: a <div> answered
// .value/.rows/.href/.type with guard-rejected ghosts, for-in over an
// element exposed them all, and getOwnPropertyDescriptor(Element.prototype,
// 'requestSubmit') lied. This file runs AFTER the platform bundle so every
// generated install has landed, then finishes the MOVE the bootstrap's
// _interfaceProps table started: the SVG tags keep the members they answer
// today through Element inheritance (same descriptors, same guards —
// behavior-preserving), and then the sink members are deleted from
// Element.prototype outright. The delete list is frozen from the audit;
// a new Element member that Chrome doesn't have belongs on this list, not
// on the prototype.
//
// What is deliberately NOT done here: reshaping the moved SVG members to
// Chrome's real shapes (href as SVGAnimatedString etc.) — that is the SVG
// family knife; the members keep today's runtime answers.

// SVG homes: tags whose answers today ride Element-inherited descriptors.
// The guards inside those descriptors already know these local names.
{
  const _svgHomes = {
    SVGAElement: ['href', 'rel', 'relList', 'type'],
    SVGUseElement: ['href'],
    SVGImageElement: ['href'],
    SVGPatternElement: ['href', 'viewBox'],
    SVGGradientElement: ['href'],
    SVGStyleElement: ['sheet', 'disabled', 'type'],
    SVGSVGElement: ['viewBox'],
    SVGMarkerElement: ['viewBox'],
  };
  for (const [iname, props] of Object.entries(_svgHomes)) {
    const C = globalThis[iname];
    if (!C || !C.prototype) continue;
    for (const p of props) {
      if (Object.prototype.hasOwnProperty.call(C.prototype, p)) continue;
      const d = Object.getOwnPropertyDescriptor(Element.prototype, p);
      if (d) Object.defineProperty(C.prototype, p, d);
    }
  }
}

// The sink, out. Every member here: Chrome does not own it on
// Element.prototype (shape-audit-truth-145.json), and every interface that
// needs it has received the descriptor (bootstrap _interfaceProps table,
// the selection-family retarget onto input/textarea, the SVG homes above,
// or the platform bundle's generated installs).
{
  const _sink = [
    'action', 'add', 'assignedSlots', 'caption', 'cellIndex', 'cells',
    'checked', 'close', 'closedBy', 'cols', 'content', 'contentDocument',
    'contentWindow', 'control', 'createCaption', 'createTBody',
    'createTFoot', 'createTHead', 'deleteCaption', 'deleteCell',
    'deleteRow', 'deleteTFoot', 'deleteTHead', 'disabled', 'files', 'form',
    'hash', 'host', 'hostname', 'href', 'htmlFor', 'indeterminate',
    'insertCell', 'insertRow', 'isConnected', 'labels', 'max', 'maxLength',
    'method', 'min', 'minLength', 'multiple', 'name', 'open', 'options',
    'origin', 'password', 'pathname', 'placeholder', 'port', 'protocol',
    'rel', 'relList', 'requestClose', 'requestSubmit', 'reset',
    'returnValue', 'rev', 'rowIndex', 'rows', 'sandbox', 'scheme', 'search',
    'sectionRowIndex', 'select', 'selected', 'selectedIndex',
    'selectedOptions', 'selectionDirection', 'selectionEnd',
    'selectionStart', 'setRangeText', 'setSelectionRange', 'sheet', 'show',
    'showModal', 'size', 'sizes', 'src', 'srcdoc', 'step', 'stepDown',
    'stepUp', 'submit', 'tBodies', 'tFoot', 'tHead', 'type', 'username',
    'value', 'valueAsDate', 'valueAsNumber', 'viewBox', 'wrap',
  ];
  for (const p of _sink) delete Element.prototype[p];
}
