//! module: editcontext
//! layer: L2
//! exposes: EditContext, TextFormat, TextUpdateEvent, TextFormatUpdateEvent, CharacterBoundsUpdateEvent
//! requires-load: legacy:Event, legacy:EventTarget, legacy:HTMLElement, legacy:DOMRect
//! requires-call: legacy:DOMException

// EditContext API (M4 knife 18): EditContext, TextFormat and the three update
// events were constructor stubs with no members. The model is the standard's:
// an EditContext owns a text buffer, a selection and per-character bounds; pages
// drive it with updateText/updateSelection/updateCharacterBounds/... and
// HTMLElement.editContext attaches it to exactly one element. Pinned by
// chrome/editctx-truth-145.json (editctx-probe.js, 49 keys) and diffed by
// editcontext_matches_chrome_truth.
//
// Facts worth knowing: updateText swaps a reversed range and clamps both ends to
// the buffer, updateSelection clamps but keeps the order, the selection and text
// are read-only on the interface, characterBounds() hands out a fresh array of
// fresh DOMRects each call, the five handler attributes are accessors that drop
// non-functions to null, and the update events' members are plain slots.

const slots = new WeakMap();
const tfSlots = new WeakMap();
const evSlots = new WeakMap();
const owner = new WeakMap(); // EditContext -> element

const UL = (v, pre, fn) => P.conv.unsignedLong(v, pre, fn);
function isRect(v) { return typeof DOMRect === 'function' && v instanceof DOMRect; }
function copyRect(r) { return DOMRect.fromRect(r); }
function slot(o, fn) { const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }

// ---------------------------------------------------------------- EditContext
const PRE = "Failed to construct 'EditContext'";
function EditContext() {
  if (new.target === undefined) P.fail(EditContext, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", PRE);
  let init = arguments[0];
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') P.fail(EditContext, "The provided value is not of type 'EditContextInit'.", PRE);
  const o = init === undefined ? {} : init;
  const rd = (k) => PRE + ": Failed to read the '" + k + "' property from 'EditContextInit'";
  // dictionary members in lexicographic order
  const selectionEnd = o.selectionEnd === undefined ? 0 : UL(o.selectionEnd, rd('selectionEnd'), EditContext);
  const selectionStart = o.selectionStart === undefined ? 0 : UL(o.selectionStart, rd('selectionStart'), EditContext);
  const text = o.text === undefined ? '' : P.conv.DOMString(o.text, rd('text'), EditContext);
  const self = Reflect.construct(EventTarget, [], new.target);
  const s = { text, selectionStart: 0, selectionEnd: 0, rangeStart: 0, bounds: [], handlers: {} };
  s.selectionStart = Math.min(selectionStart, text.length);
  s.selectionEnd = Math.min(selectionEnd, text.length);
  slots.set(self, s);
  return self;
}
const EP = P.iface(EditContext, 'EditContext', 0, EventTarget);
P.getter(EP, 'text', function __f() { return slot(this, __f).text; });
P.getter(EP, 'selectionStart', function __f() { return slot(this, __f).selectionStart; });
P.getter(EP, 'selectionEnd', function __f() { return slot(this, __f).selectionEnd; });
P.getter(EP, 'characterBoundsRangeStart', function __f() { return slot(this, __f).rangeStart; });
for (const type of ['textupdate', 'textformatupdate', 'characterboundsupdate', 'compositionstart', 'compositionend']) {
  P.accessor(EP, 'on' + type, function __f() { const h = slot(this, __f).handlers[type]; return h === undefined ? null : h; }, function __f(v) {
    slot(this, __f).handlers[type] = typeof v === 'function' ? v : null;
  });
}
P.method(EP, 'attachedElements', 0, function __f() { slot(this, __f); const el = owner.get(this); return el === undefined ? [] : [el]; });
P.method(EP, 'characterBounds', 0, function __f() { return slot(this, __f).bounds.map(copyRect); });
const pre = (m) => "Failed to execute '" + m + "' on 'EditContext'";
P.method(EP, 'updateCharacterBounds', 2, function __f() {
  const s = slot(this, __f);
  P.args(pre('updateCharacterBounds'), arguments.length, 2, __f);
  const start = UL(arguments[0], pre('updateCharacterBounds'), __f);
  const list = Array.from(arguments[1]);
  for (const r of list) if (!isRect(r)) P.fail(__f, "Failed to convert value to 'DOMRect'.", pre('updateCharacterBounds'));
  s.rangeStart = start;
  s.bounds = list.map(copyRect);
});
function boundsMethod(name) {
  P.method(EP, name, 1, function __f() {
    slot(this, __f);
    P.args(pre(name), arguments.length, 1, __f);
    if (!isRect(arguments[0])) P.fail(__f, "parameter 1 is not of type 'DOMRect'.", pre(name));
  });
}
boundsMethod('updateControlBounds');
P.method(EP, 'updateSelection', 2, function __f() {
  const s = slot(this, __f);
  P.args(pre('updateSelection'), arguments.length, 2, __f);
  const a = UL(arguments[0], pre('updateSelection'), __f), b = UL(arguments[1], pre('updateSelection'), __f);
  s.selectionStart = Math.min(a, s.text.length);
  s.selectionEnd = Math.min(b, s.text.length);
});
boundsMethod('updateSelectionBounds');
P.method(EP, 'updateText', 3, function __f() {
  const s = slot(this, __f);
  P.args(pre('updateText'), arguments.length, 3, __f);
  let a = UL(arguments[0], pre('updateText'), __f), b = UL(arguments[1], pre('updateText'), __f);
  const t = P.conv.DOMString(arguments[2], pre('updateText'), __f);
  const n = s.text.length;
  a = Math.min(a, n); b = Math.min(b, n);
  const lo = Math.min(a, b), hi = Math.max(a, b);
  s.text = s.text.slice(0, lo) + t + s.text.slice(hi);
});
P.seal(EditContext);

// element.editContext (on HTMLElement.prototype): attaches to one element at a time
const ALLOWED = /^(article|aside|blockquote|body|canvas|div|footer|header|h[1-6]|main|nav|p|section|span)$/;
P.accessor(HTMLElement.prototype, 'editContext', function __f() {
  const e = editOf.get(this);
  return e === undefined ? null : e;
}, function __f(v) {
  const p = "Failed to set the 'editContext' property on 'HTMLElement'";
  if (v !== null && v !== undefined && (typeof v !== 'object' || slots.get(v) === undefined)) P.fail(__f, "Failed to convert value to 'EditContext'.", p);
  const next = v === undefined ? null : v;
  if (next !== null) {
    const name = String(this.localName || '').toLowerCase();
    if (!(ALLOWED.test(name) && this.namespaceURI === 'http://www.w3.org/1999/xhtml') && !/-/.test(name)) throw new DOMException(p + ': This element does not support EditContext', 'NotSupportedError');
    const cur = owner.get(next);
    if (cur !== undefined && cur !== this) throw new DOMException(p + ': An EditContext can be only be associated with a single element', 'NotSupportedError');
  }
  const old = editOf.get(this);
  if (old !== undefined) owner.delete(old);
  if (next === null) editOf.delete(this); else { editOf.set(this, next); owner.set(next, this); }
});
const editOf = new WeakMap();

// ---------------------------------------------------------------- TextFormat
const TPRE = "Failed to construct 'TextFormat'";
const STYLES = ['none', 'solid', 'dotted', 'dashed', 'wavy'];
const THICK = ['none', 'thin', 'thick'];
function TextFormat() {
  if (new.target === undefined) P.fail(TextFormat, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", TPRE);
  let init = arguments[0];
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') P.fail(TextFormat, "The provided value is not of type 'TextFormatInit'.", TPRE);
  const o = init === undefined ? {} : init;
  const rd = (k) => TPRE + ": Failed to read the '" + k + "' property from 'TextFormatInit'";
  const en = (k, list, tn) => {
    if (o[k] === undefined) return 'none';
    const s = P.conv.DOMString(o[k], rd(k), TextFormat);
    if (list.indexOf(s) < 0) P.fail(TextFormat, "The provided value '" + s + "' is not a valid enum value of type " + tn + '.', rd(k));
    return s;
  };
  const rangeEnd = o.rangeEnd === undefined ? 0 : UL(o.rangeEnd, rd('rangeEnd'), TextFormat);
  const rangeStart = o.rangeStart === undefined ? 0 : UL(o.rangeStart, rd('rangeStart'), TextFormat);
  const underlineStyle = en('underlineStyle', STYLES, 'UnderlineStyle');
  const underlineThickness = en('underlineThickness', THICK, 'UnderlineThickness');
  const self = Reflect.construct(Object, [], new.target);
  tfSlots.set(self, { rangeStart, rangeEnd, underlineStyle, underlineThickness });
  return self;
}
const TP = P.iface(TextFormat, 'TextFormat', 0);
function tfSlot(o, fn) { const s = o !== null && typeof o === 'object' ? tfSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
for (const m of ['rangeStart', 'rangeEnd', 'underlineStyle', 'underlineThickness']) P.getter(TP, m, function __f() { return tfSlot(this, __f)[m]; });
P.seal(TextFormat);

// ---------------------------------------------------------------- events
// members: [name, kind, default]; init dictionary read in lexicographic order
function mkEvent(name, dict, members) {
  const PRE_ = "Failed to construct '" + name + "'";
  const C = {
    [name]: function () {
      if (new.target === undefined) P.fail(C[name], "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", PRE_);
      P.args(PRE_, arguments.length, 1, C[name]);
      const init = arguments[1];
      const ev = Reflect.construct(Event, [arguments[0], init], new.target);
      const o = init === undefined || init === null ? {} : init;
      const rd = (k) => PRE_ + ": Failed to read the '" + k + "' property from '" + dict + "'";
      const s = {};
      for (const k of Object.keys(members).sort()) {
        const kind = members[k];
        if (kind === 'ulong') s[k] = o[k] === undefined ? 0 : UL(o[k], rd(k), C[name]);
        else if (kind === 'string') s[k] = o[k] === undefined ? '' : P.conv.DOMString(o[k], rd(k), C[name]);
        else {
          const list = o[k] === undefined ? [] : Array.from(o[k]);
          for (const f of list) if (tfSlots.get(f) === undefined) P.fail(C[name], "Failed to convert value to 'TextFormat'.", rd(k));
          s[k] = list;
        }
      }
      evSlots.set(ev, s);
      return ev;
    },
  }[name];
  const proto = P.iface(C, name, 1, Event);
  return { C, proto };
}
function evSlot(o, fn) { const s = o !== null && typeof o === 'object' ? evSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
const TUE = mkEvent('TextUpdateEvent', 'TextUpdateEventInit', { selectionEnd: 'ulong', selectionStart: 'ulong', text: 'string', updateRangeEnd: 'ulong', updateRangeStart: 'ulong' });
for (const m of ['updateRangeStart', 'updateRangeEnd', 'text', 'selectionStart', 'selectionEnd']) P.getter(TUE.proto, m, function __f() { return evSlot(this, __f)[m]; });
P.seal(TUE.C);
const TFUE = mkEvent('TextFormatUpdateEvent', 'TextFormatUpdateEventInit', { textFormats: 'formats' });
P.method(TFUE.proto, 'getTextFormats', 0, function __f() { return evSlot(this, __f).textFormats.slice(); });
P.seal(TFUE.C);
const CBUE = mkEvent('CharacterBoundsUpdateEvent', 'CharacterBoundsUpdateEventInit', { rangeEnd: 'ulong', rangeStart: 'ulong' });
for (const m of ['rangeStart', 'rangeEnd']) P.getter(CBUE.proto, m, function __f() { return evSlot(this, __f)[m]; });
P.seal(CBUE.C);

P.expose('EditContext', EditContext);
P.expose('TextFormat', TextFormat);
P.expose('TextUpdateEvent', TUE.C);
P.expose('TextFormatUpdateEvent', TFUE.C);
P.expose('CharacterBoundsUpdateEvent', CBUE.C);
