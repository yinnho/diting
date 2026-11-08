(function () { // @@SCOPE
// god-file split (刀50, input text-selection + date-draft belt): the _ns_
// WeakMaps (selectionStart/End/Dir mirrors - JS-side truth for reads, every
// write mirrored into the Rust tree so paint resolves the caret; offsets
// verbatim UTF-16, Rust clamps), the selection/typing helpers, the date
// sanitize ladder (days-in-month, ISO weeks, time norm - Chrome rejects
// impossible dates), and the engine hook __diting_dateDraftBase (input.rs
// reads the composition base: a partial date is '' in the DOM but keeps
// composing in the draft). Consumers: domcore at runtime (typing funnels,
// value setters, setRangeText paths) via the auto-migrated d() rows.
// Constraint validation itself lives in form_validation.js. No marking,
// no pins.
// ---- form control validity and text selection ----

// Constraint validation (ValidityState, checkValidity, …) lives in form_validation.js.

// Text selection on Element.prototype
const _ns_selectionStart = new WeakMap();
const _ns_selectionEnd = new WeakMap();
const _ns_selectionDir = new WeakMap();

// Selection mirror (typing-cursor batch): the WeakMaps stay the JS-side
// truth for reads; every write also lands in the Rust tree so paint can
// resolve the caret at collect time. Offsets are the JS numbers verbatim
// (UTF-16 units — the Rust side clamps to the value's char count).
const _ns_mirrorSelection = (el) => {
  try {
    if (!el || el._nid === undefined) return;
    const s = _ns_selectionStart.get(el);
    const e = _ns_selectionEnd.get(el);
    _domRaw("set_selection", el._nid, (s ?? 0) + "," + (e ?? 0));
  } catch (_e) {}
};
// A control whose value is text the user can edit — the same type list the
// CDP insertText guard uses (input.rs).
const _ns_isTextEntry = (el) => {
  if (!el) return false;
  if (el.localName === 'textarea') return true;
  if (el.localName !== 'input') return false;
  const ty = String(el.getAttribute('type') || 'text').toLowerCase();
  return ['button','submit','reset','image','checkbox','radio','file','hidden','range','color'].indexOf(ty) < 0;
};

// The date/time input family runs HTML's value sanitization algorithm
// (agent-browser#2000 absorption): the DOM value only ever holds a complete
// valid string — Chrome stores '' for anything else, including the attribute
// default (<input type=date value="junk"> reads ''). Typing is the wrinkle:
// the CDP insertText/Backspace funnels compose `current + char` off .value,
// which would restart at '' on every keystroke, so an in-flight partial
// rides the _ns_dateDraft WeakMap — the funnels read their composition base
// through __diting_dateDraftBase and keep composing until the string
// validates, then it lands in .value and the draft clears (the engine-side
// equivalent of Chrome's segmented editor).
const _NS_DATE_VALUE_TYPES = ['date', 'month', 'week', 'time', 'datetime-local'];
const _ns_daysInMonth = (y, m) => {
  const leap = (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
  return [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];
};
// ISO week-numbering years with a W53: years starting on Thursday, or leap
// years starting on Wednesday (ISO 8601 §2.2.10 / Blink's week validation).
const _ns_isoWeeksInYear = (y) => {
  const p = (yy) => (yy + Math.floor(yy / 4) - Math.floor(yy / 100) + Math.floor(yy / 400)) % 7;
  return (p(y) === 4 || p(y - 1) === 3) ? 53 : 52;
};
// Normalized time-of-day: seconds (and the fraction) drop when zero,
// exactly like Chrome's "valid normalized local date and time string".
const _ns_normTime = (h, m, s, frac) =>
  (s !== undefined && (+s > 0 || frac)) ? h + ':' + m + ':' + s + (frac || '') : h + ':' + m;
// Returns the SANITIZED value: the normalized complete string, or '' when
// the input is a partial or out-of-range/garbage. Padding must be exact
// (2026-1-2 is not a valid date string), the calendar must be real (Feb 30
// rejected), and every component must be in range.
const _ns_sanitizeDateValue = (ty, raw) => {
  const s = String(raw);
  let m;
  if (ty === 'date') {
    if (!(m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s))) return '';
    const mo = +m[2], d = +m[3];
    return mo >= 1 && mo <= 12 && d >= 1 && d <= _ns_daysInMonth(+m[1], mo) ? s : '';
  }
  if (ty === 'month') {
    if (!(m = /^(\d{4})-(\d{2})$/.exec(s))) return '';
    return +m[2] >= 1 && +m[2] <= 12 ? s : '';
  }
  if (ty === 'week') {
    if (!(m = /^(\d{4})-W(\d{2})$/.exec(s))) return '';
    const w = +m[2];
    return w >= 1 && w <= _ns_isoWeeksInYear(+m[1]) ? s : '';
  }
  if (ty === 'time') {
    if (!(m = /^(\d{2}):(\d{2})(?::(\d{2})(\.\d+)?)?$/.exec(s))) return '';
    if (+m[1] > 23 || +m[2] > 59 || (m[3] !== undefined && +m[3] > 59)) return '';
    return _ns_normTime(m[1], m[2], m[3], m[4]);
  }
  if (ty === 'datetime-local') {
    // The spec grammar allows a space separator; the normalized form uses T.
    // (Groups: 1=date, 2=HH, 3=MM, 4=SS?, 5=fraction? — [T ] captures none.)
    if (!(m = /^(\d{4}-\d{2}-\d{2})[T ](\d{2}):(\d{2})(?::(\d{2})(\.\d+)?)?$/.exec(s))) return '';
    if (_ns_sanitizeDateValue('date', m[1]) === '') return '';
    if (+m[2] > 23 || +m[3] > 59 || (m[4] !== undefined && +m[4] > 59)) return '';
    return m[1] + 'T' + _ns_normTime(m[2], m[3], m[4], m[5]);
  }
  return s;
};
const _ns_dateDraft = new WeakMap();
// Engine-internal hook: the typing funnels (input.rs) read the composition
// base through this — a partial date value is '' in the DOM but keeps
// composing in the draft.
globalThis.__diting_dateDraftBase = function(el, fallback) {
  return _ns_dateDraft.has(el) ? _ns_dateDraft.get(el) : fallback;
};

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_NS_DATE_VALUE_TYPES', _NS_DATE_VALUE_TYPES);
  d('_ns_dateDraft', _ns_dateDraft);
  d('_ns_isTextEntry', _ns_isTextEntry);
  d('_ns_mirrorSelection', _ns_mirrorSelection);
  d('_ns_sanitizeDateValue', _ns_sanitizeDateValue);
  d('_ns_selectionDir', _ns_selectionDir);
  d('_ns_selectionEnd', _ns_selectionEnd);
  d('_ns_selectionStart', _ns_selectionStart);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
