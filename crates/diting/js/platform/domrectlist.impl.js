//! module: domrectlist
//! layer: L2
//! exposes: DOMRectList, __diting_mkRectList
//! requires-load: -
//! requires-call: -

// DOMRectList (M4 knife 53). Element/Range.getClientRects() returned a plain Array with an own `item` method grafted on (an Array
// answers `Array.isArray`, shows `item` in for-in and reports [object Array]); Chrome returns a DOMRectList: indexed properties only,
// `length` and `item` on the prototype. Instances are plain objects over the prototype with the length in a hidden slot.

function DOMRectList() { P.fail(DOMRectList, 'Illegal constructor', "Failed to construct 'DOMRectList'"); }
const LP = P.iface(DOMRectList, 'DOMRectList', 0);
const len = new WeakMap();
const lenOf = (o, fn) => { const n = o !== null && typeof o === 'object' ? len.get(o) : undefined; if (n === undefined) P.illegal(fn); return n; };
P.getter(LP, 'length', function __f() { return lenOf(this, __f); });
P.method(LP, 'item', 1, function __f(i) {
  const n = lenOf(this, __f); P.args("Failed to execute 'item' on 'DOMRectList'", arguments.length, 1, __f);
  const k = P.conv.unsignedLong(i, "Failed to execute 'item' on 'DOMRectList'", __f);
  return k < n ? this[k] : null;
});
Object.defineProperty(LP, Symbol.iterator, { value: Array.prototype.values, writable: true, enumerable: false, configurable: true });
P.seal(DOMRectList);

function make(rects) {
  const o = Object.create(LP);
  for (let i = 0; i < rects.length; i++) o[i] = rects[i];
  len.set(o, rects.length);
  return o;
}
P.expose('DOMRectList', DOMRectList);
P.exposeValue('__diting_mkRectList', make);
