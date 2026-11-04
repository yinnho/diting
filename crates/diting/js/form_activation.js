(function () { // @@SCOPE
// god-file split (刀60, form activation belt): the native pre/post click
// activation machinery for checkable inputs — the lazy prototype-setter
// cache (_nativeCheckedSet/_nativeIndeterminateSet + _nativeSet; instance
// traps like React's _valueTracker are bypassed by calling the prototype
// setter), the flip/cancel/commit triplet around the click dispatch
// (_preClickFlip: radio exclusivity + checkbox/indeterminate flip;
// _commitPreClickFlip: input+change pair only on surviving difference), and
// _postClickActivation (label forwarding, anchor navigation, submit).
// bootstrap's trusted-CDP click block and details_summary/event_hooks/
// domcore call these at runtime via the published hidden globals; the zone's
// own refs out (_isFormControlDisabled, _labeledControl, _isSubmitButton,
// _LABELABLE, __summaryActivation) are runtime reads of bootstrap-published
// or globalThis-installed names.
// Activation flips `checked` (and `indeterminate`) NATIVELY in Chrome — the
// C++ pre-click activation never runs any JS setter. Pages routinely install
// an own-property `checked` trap (React 16's `_valueTracker` is the canonical
// one: its setter mirrors every assignment into a shadow "last value" so
// change extraction can diff against it). Flipping through the plain
// assignment routes the activation into that trap, the tracker sees the new
// value BEFORE the click event dispatches, and the framework's diff comes up
// empty — every controlled radio/checkbox onChange goes deaf. Bypass
// instance-level traps by calling the prototype's native setter directly.
let _nativeCheckedSet = undefined; // lazy: setter fn | false (unavailable)
let _nativeIndeterminateSet = undefined;
function _nativeSet(el, cacheRef, prop, value) {
  if (cacheRef.v === undefined) {
    const d = Object.getOwnPropertyDescriptor(el.constructor.prototype, prop) ||
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, prop);
    cacheRef.v = d && typeof d.set === 'function' ? d.set : false;
  }
  if (cacheRef.v) cacheRef.v.call(el, value);
  else el[prop] = value;
}
function _activationSetChecked(el, value) {
  if (_nativeCheckedSet === undefined) _nativeCheckedSet = { v: undefined };
  _nativeSet(el, _nativeCheckedSet, 'checked', value);
}
function _activationSetIndeterminate(el, value) {
  if (_nativeIndeterminateSet === undefined) _nativeIndeterminateSet = { v: undefined };
  _nativeSet(el, _nativeIndeterminateSet, 'indeterminate', value);
}
function _preClickFlip(el) {
  if (!el || el.tagName !== 'INPUT' || _isFormControlDisabled(el)) return null;
  const type = (el.getAttribute('type') || '').toLowerCase();
  if (type !== 'checkbox' && type !== 'radio') return null;
  const st = { el: el, type: type, oldChecked: !!el.checked, oldIndeterminate: false, radioStates: null };
  if (type === 'radio') {
    const name = el.getAttribute('name') || '';
    if (name) {
      st.radioStates = [];
      const all = (el.ownerDocument || globalThis.document).querySelectorAll('input');
      for (let i = 0; i < all.length; i++) {
        const r = all[i];
        if ((r.getAttribute('type') || '').toLowerCase() !== 'radio') continue;
        if ((r.getAttribute('name') || '') !== name || r.form !== el.form) continue;
        st.radioStates.push([r, !!r.checked]);
        if (r !== el) _activationSetChecked(r, false);
      }
    }
    _activationSetChecked(el, true);
  } else {
    _activationSetChecked(el, !st.oldChecked);
    st.oldIndeterminate = !!el.indeterminate;
    _activationSetIndeterminate(el, false);
  }
  return st;
}

// A cancelled click restores every prior state the flip touched (the whole
// radio group included); checkbox activation also restores `indeterminate`.
// The restore rides the same native-setter bypass as the flip: a cancelled
// activation must not be visible to page-installed property traps either.
function _cancelPreClickFlip(st) {
  if (!st) return;
  if (st.radioStates) {
    for (let i = 0; i < st.radioStates.length; i++) _activationSetChecked(st.radioStates[i][0], st.radioStates[i][1]);
  } else {
    _activationSetChecked(st.el, st.oldChecked);
    if (st.type === 'checkbox') _activationSetIndeterminate(st.el, st.oldIndeterminate);
  }
}

// Commit a surviving flip: fire input/change when the checked state actually
// changed (an already-checked radio stays silent, like Chrome).
function _commitPreClickFlip(st) {
  if (!st) return;
  if (st.el.checked === st.oldChecked) return;
  try { st.el.dispatchEvent(new Event('input', {bubbles: true})); } catch (e) {}
  try { st.el.dispatchEvent(new Event('change', {bubbles: true})); } catch (e) {}
}

// Post-click activation behaviour for elements that are not checkable inputs:
// label forwarding to the labeled control, anchor navigation, submit buttons.
// Runs after the click dispatch survives uncanceled. Returns what activated
// ('label' | 'link' | 'submit') or null — the trusted CDP path uses the
// return value to gate its triple-click select-all tail.
function _postClickActivation(el) {
  if (globalThis.__summaryActivation(el)) return 'summary';
  const tag = el.tagName;
  // Interactive elements keep their own activation, so a control nested
  // inside a label toggles once instead of forwarding through the label
  // and firing twice.
  const selfInteractive = el.matches && el.matches(_LABELABLE + ',a');
  const labelHost = selfInteractive ? null : (tag === 'LABEL' ? el : (el.closest ? el.closest('label') : null));
  if (labelHost) {
    const control = _labeledControl(labelHost);
    // A disabled control has no activation behaviour — own attribute or
    // disabled through an ancestor fieldset both count.
    if (control && control !== el && !_isFormControlDisabled(control)) {
      control.click();
      return 'label';
    }
  }
  const link = tag === 'A' ? el : (el.closest ? el.closest('a[href]') : null);
  if (link) {
    const href = link.getAttribute('href');
    if (href && !href.startsWith('#') && !href.startsWith('javascript:')) {
      location.assign(href);
      return 'link';
    }
  }
  if (_isSubmitButton(el)) {
    const form = el.closest ? el.closest('form') : null;
    if (form) {
      if (typeof form.requestSubmit === 'function') {
        form.requestSubmit(el);
      } else if (typeof form.submit === 'function') {
        form.submit(el);
      }
      return 'submit';
    }
  }
  return null;
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_cancelPreClickFlip', _cancelPreClickFlip);
  d('_commitPreClickFlip', _commitPreClickFlip);
  d('_postClickActivation', _postClickActivation);
  d('_preClickFlip', _preClickFlip);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
