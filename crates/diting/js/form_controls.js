(function () { // @@SCOPE
// god-file split (刀67, form control semantics): the submitter validator
// (_isSubmitButton — shared by requestSubmit validation and the internal
// click path so a click can never hand requestSubmit a submitter it would
// reject), label association (_LABELABLE selector + _labeledControl:
// for-matched AND wrapping, exactly what the label properties reflect),
// __diting_dispatchTrustedClick (the CDP trusted-click path: pre-click
// flip via form_activation's triplet, dispatch, change events, fallback
// activation + triple-select), the fieldset-disabled chain (_firstLegend/
// _fieldsetDisabled/_isFormControlDisabled — a control inside a disabled
// fieldset is disabled unless it is inside the FIRST legend), and
// _fireBlurFamily (the switch-away leg of el.focus(): blur + focusout on
// the old holder, mirrors set_focused). Consumers (domcore label props +
// focus paths, form_activation flip guards, interaction_handlers) are all
// runtime reads of d()-published names. No marking, no install-time deps.
// A valid form submitter: <button> that is not reset/button, or
// <input type=submit|image>. Shared by requestSubmit's argument validation
// and the internal click path, so a click can never hand requestSubmit a
// submitter it would reject (upstream 7e2cabf/ccfa5fb).
function _isSubmitButton(el) {
  if (!el || typeof el.localName !== "string") return false;
  const type = ((el.getAttribute && el.getAttribute("type")) || "").toLowerCase();
  if (el.localName === "button") return type !== "reset" && type !== "button";
  if (el.localName === "input") return type === "submit" || type === "image";
  return false;
}

// Labelable elements per the HTML spec: button, input (except type=hidden),
// meter, output, progress, select, textarea. A <label> activates its labeled
// control: the element referenced by `for`, else the first labelable descendant.
const _LABELABLE = 'button,input:not([type=hidden]),meter,output,progress,select,textarea';
function _labeledControl(label) {
  if (!label || label.tagName !== 'LABEL') return null;
  // A present `for` attribute means association by id only; an empty value
  // associates nothing (no fallback to a descendant).
  const forId = label.getAttribute ? label.getAttribute('for') : null;
  if (forId !== null && forId !== undefined) {
    if (forId === '') return null;
    const doc = label.ownerDocument || globalThis.document;
    const el = doc && doc.getElementById ? doc.getElementById(forId) : null;
    if (!el) return null;
    return el.matches && el.matches(_LABELABLE) ? el : null;
  }
  return label.querySelector ? label.querySelector(_LABELABLE) : null;
}

// Activation state for a checkable input's legacy pre-click flip: the
// checkbox/radio state changes BEFORE the click event dispatches (listeners
// observe the new state) and reverts if the event is cancelled. Shared by
// Element.click() and by unmanaged synthetic dispatches through
// dispatchEvent() — Chrome runs activation behavior for untrusted clicks too,
// so `cb.dispatchEvent(new MouseEvent('click'))` toggles like cb.click() does.

// Trusted CDP click: the Rust Input domain drives the same activation
// sequence as Element.click() but with a positioned, trusted event object.
// input/change events fired here are trusted-marked like the click itself.
globalThis.__diting_dispatchTrustedClick = function (target, event, tripleSelect) {
  if (!target) return false;
  if (target._ditingClickInProgress) return true;
  target._ditingClickInProgress = true;
  try {
    const flip = _preClickFlip(target);
    const cancelled = !target.dispatchEvent(event);
    if (cancelled) {
      _cancelPreClickFlip(flip);
      return false;
    }
    const mk = globalThis.__diting_markTrusted || function (e) { return e; };
    if (flip && flip.el.checked !== flip.oldChecked) {
      try { flip.el.dispatchEvent(mk(new Event('input', {bubbles: true}))); } catch (e) {}
      try { flip.el.dispatchEvent(mk(new Event('change', {bubbles: true}))); } catch (e) {}
      return true;
    }
    const activated = _postClickActivation(target);
    if (!activated && tripleSelect) {
      const tag = target.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') {
        const len = target.value ? target.value.length : 0;
        if (target.setSelectionRange) target.setSelectionRange(0, len);
        else { target.selectionStart = 0; target.selectionEnd = len; }
      }
    }
    return true;
  } finally {
    target._ditingClickInProgress = false;
  }
};

// The first <legend> child of a fieldset — the only legend whose descendants
// a disabled fieldset does NOT disable (HTML spec: "the first legend child").
function _firstLegend(fieldset) {
  const kids = fieldset.children;
  for (let i = 0; i < kids.length; i++) if (kids[i].tagName === 'LEGEND') return kids[i];
  return null;
}

// A control inside <fieldset disabled> is disabled unless the path from it to
// the fieldset crosses that fieldset's first <legend>. Walk ancestors; at each
// disabled fieldset on the path, the child we came through must be its first
// legend or the control is fieldset-disabled.
function _fieldsetDisabled(el) {
  let node = el;
  while (node) {
    const p = node.parentNode;
    if (!p || !p.tagName) break;
    if (p.tagName === 'FIELDSET' && p.hasAttribute && p.hasAttribute('disabled')) {
      if (!(node.tagName === 'LEGEND' && node === _firstLegend(p))) return true;
    }
    node = p;
  }
  return false;
}

// Actually-disabled per spec: a form control with its own disabled attribute,
// or any form control disabled by an ancestor fieldset. Disabled controls have
// no activation behaviour and dispatch no click event at all.
function _isFormControlDisabled(el) {
  const t = el.tagName;
  if (t !== 'INPUT' && t !== 'BUTTON' && t !== 'SELECT' && t !== 'TEXTAREA') return false;
  if ((el.hasAttribute && el.hasAttribute('disabled')) || el.disabled) return true;
  return _fieldsetDisabled(el);
}

// Clear focus and fire the leaving half of the focus event family
// (blur non-bubbling, focusout bubbling). Shared by el.blur() and the
// switch-away leg of el.focus() — Chrome runs blur+focusout on the old
// holder before focus+focusin on the new one.
function _fireBlurFamily(el) {
  globalThis.__diting_focused = null;
  if (el && el._nid !== undefined) _domRaw("set_focused", String(el._nid), "0");
  if (!el) return;
  el.dispatchEvent(new FocusEvent("blur"));
  el.dispatchEvent(new FocusEvent("focusout", {bubbles: true}));
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_LABELABLE', _LABELABLE);
  d('_fireBlurFamily', _fireBlurFamily);
  d('_isFormControlDisabled', _isFormControlDisabled);
  d('_isSubmitButton', _isSubmitButton);
  d('_labeledControl', _labeledControl);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
