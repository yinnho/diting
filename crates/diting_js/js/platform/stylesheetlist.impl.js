//! module: stylesheetlist
//! layer: L2
//! exposes: StyleSheetList, __diting_mkSheetList
//! requires-load: -
//! requires-call: -

// StyleSheetList (M4 knife 53). `document.styleSheets` was a plain Object literal with an own `length`, an own `item` method and an
// own @@iterator ([object Object]); Chrome's is a StyleSheetList: indexed own properties only, `length`/`item`/@@iterator on the
// prototype.

function StyleSheetList() { P.fail(StyleSheetList, 'Illegal constructor', "Failed to construct 'StyleSheetList'"); }
const SP = P.iface(StyleSheetList, 'StyleSheetList', 0);
const len = new WeakMap();
const lenOf = (o, fn) => { const n = o !== null && typeof o === 'object' ? len.get(o) : undefined; if (n === undefined) P.illegal(fn); return n; };
P.getter(SP, 'length', function __f() { return lenOf(this, __f); });
P.method(SP, 'item', 1, function __f(i) {
  const n = lenOf(this, __f); P.args("Failed to execute 'item' on 'StyleSheetList'", arguments.length, 1, __f);
  const k = P.conv.unsignedLong(i, "Failed to execute 'item' on 'StyleSheetList'", __f);
  return k < n ? this[k] : null;
});
Object.defineProperty(SP, Symbol.iterator, { value: Array.prototype.values, writable: true, enumerable: false, configurable: true });
P.seal(StyleSheetList);

function make(sheets) {
  const o = Object.create(SP);
  for (let i = 0; i < sheets.length; i++) o[i] = sheets[i];
  len.set(o, sheets.length);
  return o;
}
P.expose('StyleSheetList', StyleSheetList);
P.exposeValue('__diting_mkSheetList', make);
