//! module: domsmall
//! layer: L2
//! exposes: HashChangeEvent, SubmitEvent, PopStateEvent, ClipboardEvent, RTCSessionDescription, InputDeviceCapabilities
//! requires-load: gen/htmlelement, legacy:Event, legacy:CharacterData, legacy:Text, legacy:ProcessingInstruction, legacy:DocumentFragment, legacy:MessageEvent, legacy:DataTransfer, legacy:_evx, legacy:_evxGuard
//! requires-call: legacy:HTMLElement, legacy:URL

// (loads after gen/htmlelement so DocumentFragment.moveBefore exists when the
// prototype is re-seated into Chrome's order)
// Small-member sweep (M4 knife 14): the 1-3 name gaps on interfaces that already
// have their shape - CharacterData/Text/ProcessingInstruction/DocumentFragment
// mixin members, MessageEvent.initMessageEvent/userActivation, DataTransfer data
// access - plus four stubs that had no members at all (HashChangeEvent,
// SubmitEvent, PopStateEvent, ClipboardEvent) and the value classes
// RTCSessionDescription and InputDeviceCapabilities. Every fact is pinned by
// chrome/smallmembers-truth-145.json (smallmembers-probe.js) and diffed by
// smallmembers_match_chrome_truth. Patches to bootstrap classes re-seat each
// prototype into Chrome's own member order.

function reseat(proto, order) {
  for (const n of order) {
    const d = Object.getOwnPropertyDescriptor(proto, n);
    if (d && d.configurable) { delete proto[n]; Object.defineProperty(proto, n, d); }
  }
}

// ================= NonDocumentTypeChildNode on CharacterData; Slottable on Text;
// LinkStyle on ProcessingInstruction; ParentNode.childElementCount on fragments
const CD = CharacterData.prototype;
function elementSibling(node, dir) {
  for (let x = dir > 0 ? node.nextSibling : node.previousSibling; x; x = dir > 0 ? x.nextSibling : x.previousSibling) {
    if (x.nodeType === 1) return x;
  }
  return null;
}
P.getter(CD, 'previousElementSibling', function __f() { if (!(this instanceof CharacterData)) P.illegal(__f); return elementSibling(this, -1); });
P.getter(CD, 'nextElementSibling', function __f() { if (!(this instanceof CharacterData)) P.illegal(__f); return elementSibling(this, 1); });
reseat(CD, ['data', 'length', 'previousElementSibling', 'nextElementSibling', 'after', 'appendData', 'before', 'deleteData',
  'insertData', 'remove', 'replaceData', 'replaceWith', 'substringData', 'constructor']);
P.getter(Text.prototype, 'assignedSlot', function __f() { if (!(this instanceof Text)) P.illegal(__f); return null; });
reseat(Text.prototype, ['wholeText', 'assignedSlot', 'splitText', 'constructor']);
P.getter(ProcessingInstruction.prototype, 'sheet', function __f() { if (!(this instanceof ProcessingInstruction)) P.illegal(__f); return null; });
reseat(ProcessingInstruction.prototype, ['target', 'sheet', 'constructor']);
P.getter(DocumentFragment.prototype, 'childElementCount', function __f() {
  if (!(this instanceof DocumentFragment)) P.illegal(__f);
  let n = 0;
  for (let x = this.firstChild; x; x = x.nextSibling) if (x.nodeType === 1) n++;
  return n;
});
reseat(DocumentFragment.prototype, ['children', 'firstElementChild', 'lastElementChild', 'childElementCount', 'append', 'getElementById',
  'moveBefore', 'prepend', 'querySelector', 'querySelectorAll', 'replaceChildren', 'constructor']);

// ================= tiny init-dictionary event factory
const evSlots = new WeakMap();
const isObj = (v) => (typeof v === 'object' && v !== null) || typeof v === 'function';
const CONV = {
  usv: (v, fn, pre) => P.conv.USVString(v, pre, fn),
  bool: (v) => !!v,
  any: (v) => v,
  htmlElement: (v, fn, pre) => {
    if (v === null || v === undefined) return null;
    // the engine's SVG elements inherit HTMLElement: judge by namespace
    if (v instanceof HTMLElement && v.namespaceURI === 'http://www.w3.org/1999/xhtml') return v;
    P.fail(fn, "Failed to convert value to 'HTMLElement'.", pre);
  },
  dataTransfer: (v, fn, pre) => {
    if (v === null || v === undefined) return null;
    if (v instanceof DataTransfer) return v;
    P.fail(fn, "Failed to convert value to 'DataTransfer'.", pre);
  },
};
function mkEvent(name, dict, members, getters) {
  const C = { [name]: function () {
    const pre = "Failed to construct '" + name + "'";
    if (new.target === undefined) P.fail(C, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
    if (arguments.length < 1) P.fail(C, '1 argument required, but only 0 present.', pre);
    const type = P.conv.DOMString(arguments[0], pre, C);
    const init = arguments[1];
    if (init !== undefined && init !== null && !isObj(init)) P.fail(C, "The provided value is not of type '" + dict + "'.", pre);
    const o = init === undefined || init === null ? {} : init;
    const ev = Reflect.construct(Event, init === undefined || init === null ? [type] : [type, init], new.target);
    const s = {};
    for (const m of Object.keys(members).sort()) {
      const [conv, dflt] = members[m];
      s[m] = o[m] === undefined ? dflt : CONV[conv](o[m], C, pre + ": Failed to read the '" + m + "' property from '" + dict + "'");
    }
    evSlots.set(ev, s);
    return ev;
  } }[name];
  const proto = P.iface(C, name, 1, Event);
  for (const g of getters) {
    P.getter(proto, g, function __f() { const s = evSlots.get(this); if (!s) P.illegal(__f); return s[g]; });
  }
  P.seal(C);
  return C;
}
const HashChangeEvent = mkEvent('HashChangeEvent', 'HashChangeEventInit', { newURL: ['usv', ''], oldURL: ['usv', ''] }, ['oldURL', 'newURL']);
const SubmitEvent = mkEvent('SubmitEvent', 'SubmitEventInit', { submitter: ['htmlElement', null] }, ['submitter']);
const PopStateEvent = mkEvent('PopStateEvent', 'PopStateEventInit', { hasUAVisualTransition: ['bool', false], state: ['any', null] }, ['state', 'hasUAVisualTransition']);
const ClipboardEvent = mkEvent('ClipboardEvent', 'ClipboardEventInit', { clipboardData: ['dataTransfer', null] }, ['clipboardData']);

// ================= MessageEvent: initMessageEvent + userActivation on the bootstrap class
const ME = MessageEvent.prototype;
P.getter(ME, 'userActivation', function __f() { _evxGuard(_evx.get(this)); return null; });
P.method(ME, 'initMessageEvent', 1, function __f() {
  const pre = "Failed to execute 'initMessageEvent' on 'MessageEvent'";
  const s = _evxGuard(_evx.get(this));
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  const a = arguments;
  const type = P.conv.DOMString(a[0], pre, __f);
  if (this.eventPhase !== 0) return;
  this.initEvent(type, !!a[1], !!a[2]);
  s.data = a[3] === undefined ? null : a[3];
  s.origin = a[4] === undefined ? '' : P.conv.USVString(a[4], pre, __f);
  s.lastEventId = a[5] === undefined ? '' : P.conv.DOMString(a[5], pre, __f);
  s.source = a[6] === undefined ? null : a[6];
  s.ports = a[7] === undefined || a[7] === null ? [] : Array.from(a[7]);
});
reseat(ME, ['data', 'origin', 'lastEventId', 'source', 'ports', 'userActivation', 'initMessageEvent', 'constructor']);

// ================= DataTransfer data access on the bootstrap class's string-item store
const DT = DataTransfer.prototype;
function dtFormat(f) {
  const l = f.toLowerCase();
  return l === 'text' ? 'text/plain' : l === 'url' ? 'text/uri-list' : l;
}
const typesCache = new WeakMap();
P.method(DT, 'clearData', 0, function __f(format) {
  if (!Array.isArray(this._dtItems)) P.illegal(__f);
  const keep = [];
  const f = arguments.length === 0 ? null : dtFormat(P.conv.DOMString(format, "Failed to execute 'clearData' on 'DataTransfer'", __f));
  for (const it of this._dtItems) { if (it.kind === 'file' || (f !== null && it.type !== f)) keep.push(it); }
  this._dtItems.length = 0;
  for (const it of keep) this._dtItems.push(it);
});
P.method(DT, 'getData', 1, function __f(format) {
  const pre = "Failed to execute 'getData' on 'DataTransfer'";
  if (!Array.isArray(this._dtItems)) P.illegal(__f);
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  const raw = P.conv.DOMString(format, pre, __f);
  const f = dtFormat(raw);
  const it = this._dtItems.find((x) => x.kind === 'string' && x.type === f);
  if (!it) return '';
  if (raw.toLowerCase() === 'url') {
    // the legacy 'url' format returns the first valid URL of the uri-list, serialized
    for (const line of String(it.data).split(/\r?\n/)) {
      if (line === '' || line[0] === '#') continue;
      try { return new URL(line).href; } catch (e) { return ''; }
    }
    return '';
  }
  return it.data;
});
P.method(DT, 'setData', 2, function __f(format, data) {
  const pre = "Failed to execute 'setData' on 'DataTransfer'";
  if (!Array.isArray(this._dtItems)) P.illegal(__f);
  if (arguments.length < 2) P.fail(__f, '2 arguments required, but only ' + arguments.length + ' present.', pre);
  const f = dtFormat(P.conv.DOMString(format, pre, __f));
  const d = P.conv.DOMString(data, pre, __f);
  const at = this._dtItems.findIndex((x) => x.kind === 'string' && x.type === f);
  if (at >= 0) this._dtItems.splice(at, 1);
  this._dtItems.push({ kind: 'string', type: f, data: d });
});
// types: frozen, and the SAME array until the content changes
const legacyTypes = Object.getOwnPropertyDescriptor(DT, 'types').get;
P.getter(DT, 'types', function __f() {
  const fresh = legacyTypes.call(this);
  const prev = typesCache.get(this);
  if (prev && prev.length === fresh.length && prev.every((v, i) => v === fresh[i])) return prev;
  const frozen = Object.freeze(fresh);
  typesCache.set(this, frozen);
  return frozen;
});
reseat(DT, ['dropEffect', 'effectAllowed', 'items', 'types', 'files', 'clearData', 'getData', 'setData', 'setDragImage', 'constructor']);

// ================= RTCSessionDescription (type and sdp are read-write)
const sdSlots = new WeakMap();
const SDP_TYPES = ['offer', 'answer', 'pranswer', 'rollback'];
function RTCSessionDescription() {
  const pre = "Failed to construct 'RTCSessionDescription'";
  if (new.target === undefined) P.fail(RTCSessionDescription, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  let init = arguments[0];
  if (init === null) init = undefined;
  if (init !== undefined && !isObj(init)) P.fail(RTCSessionDescription, "The provided value is not of type 'RTCSessionDescriptionInit'.", pre);
  const o = init === undefined ? {} : init;
  const rd = (k) => pre + ": Failed to read the '" + k + "' property from 'RTCSessionDescriptionInit'";
  const sdp = o.sdp === undefined ? '' : P.conv.DOMString(o.sdp, rd('sdp'), RTCSessionDescription);
  let type = null;
  if (o.type !== undefined) {
    const t = P.conv.DOMString(o.type, rd('type'), RTCSessionDescription);
    if (!SDP_TYPES.includes(t)) P.fail(RTCSessionDescription, "The provided value '" + t + "' is not a valid enum value of type RTCSdpType.", rd('type'));
    type = t;
  }
  const self = Reflect.construct(Object, [], new.target);
  sdSlots.set(self, { type, sdp });
  return self;
}
const SDP = P.iface(RTCSessionDescription, 'RTCSessionDescription', 0);
function sd(o, fn) { const s = o !== null && typeof o === 'object' ? sdSlots.get(o) : undefined; if (!s) P.illegal(fn); return s; }
P.accessor(SDP, 'type', function __f() { return sd(this, __f).type; }, function __f(v) {
  const t = P.conv.DOMString(v, "Failed to set the 'type' property on 'RTCSessionDescription'", __f);
  if (SDP_TYPES.includes(t)) sd(this, __f).type = t; else sd(this, __f);
});
P.accessor(SDP, 'sdp', function __f() { return sd(this, __f).sdp; },
  function __f(v) { sd(this, __f).sdp = P.conv.DOMString(v, "Failed to set the 'sdp' property on 'RTCSessionDescription'", __f); });
P.method(SDP, 'toJSON', 0, function __f() { const s = sd(this, __f); return { type: s.type, sdp: s.sdp }; });
P.seal(RTCSessionDescription);

// ================= InputDeviceCapabilities({firesTouchEvents})
const idcSlots = new WeakMap();
function InputDeviceCapabilities() {
  const pre = "Failed to construct 'InputDeviceCapabilities'";
  if (new.target === undefined) P.fail(InputDeviceCapabilities, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  let init = arguments[0];
  if (init === null) init = undefined;
  if (init !== undefined && !isObj(init)) P.fail(InputDeviceCapabilities, "The provided value is not of type 'InputDeviceCapabilitiesInit'.", pre);
  const self = Reflect.construct(Object, [], new.target);
  idcSlots.set(self, { firesTouchEvents: init === undefined ? false : !!init.firesTouchEvents });
  return self;
}
const IDP = P.iface(InputDeviceCapabilities, 'InputDeviceCapabilities', 0);
P.getter(IDP, 'firesTouchEvents', function __f() { const s = idcSlots.get(this); if (!s) P.illegal(__f); return s.firesTouchEvents; });
P.seal(InputDeviceCapabilities);

for (const [n, C] of Object.entries({ HashChangeEvent, SubmitEvent, PopStateEvent, ClipboardEvent, RTCSessionDescription, InputDeviceCapabilities })) P.expose(n, C);
