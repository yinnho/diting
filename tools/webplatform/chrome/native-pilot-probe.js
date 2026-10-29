// DOMRectReadOnly / DOMRect shape + behaviour probe (Rust-native pilot).
// Runs in real Chrome, in the diting engine, and inside the diting_runtime
// pilot test (bare V8 + native templates). Returns a JSON string.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return typeof x === 'number' && !isFinite(x) ? String(x) : (Object.is(x, -0) ? '-0' : x); })); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + ': ' + (e && e.message); } }
  function nat(f) { return /\[native code\]/.test(Function.prototype.toString.call(f)); }
  function desc(o, k) {
    var d = Object.getOwnPropertyDescriptor(o, k);
    if (!d) return null;
    var r = { e: d.enumerable, c: d.configurable };
    if ('value' in d) { r.w = d.writable; r.t = typeof d.value; if (typeof d.value === 'function') { r.len = d.value.length; r.name = d.value.name; r.nat = nat(d.value); } else r.v = (d.value !== null && typeof d.value === 'object') ? '#object' : d.value; }
    else { r.g = d.get ? { name: d.get.name, len: d.get.length, nat: nat(d.get) } : null; r.s = d.set ? { name: d.set.name, len: d.set.length, nat: nat(d.set) } : null; }
    return r;
  }
  function shape(C) {
    var P = C.prototype, o = {};
    Object.getOwnPropertyNames(P).sort().forEach(function (k) { o[k] = desc(P, k); });
    Object.getOwnPropertySymbols(P).forEach(function (k) { o[String(k)] = desc(P, k); });
    var par = Object.getPrototypeOf(P);
    return { len: C.length, name: C.name, proto: desc(C, 'prototype'), own: Object.getOwnPropertyNames(C).sort(), order: Object.getOwnPropertyNames(P), members: o,
      parent: par === Object.prototype ? 'Object' : par.constructor.name, ctorParent: Object.getPrototypeOf(C) === Function.prototype ? 'Function' : Object.getPrototypeOf(C).name,
      static: { fromRect: desc(C, 'fromRect') } };
  }
  var RO = DOMRectReadOnly, D = DOMRect;
  t('shape.RO', function () { return shape(RO); });
  t('shape.Rect', function () { return shape(D); });
  t('global.desc', function () { return [desc(globalThis, 'DOMRect'), desc(globalThis, 'DOMRectReadOnly')]; });
  t('ctor.call', function () { return RO(); });
  t('ctor.call2', function () { return D(); });
  t('ctor.defaults', function () { var r = new D(); return [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]; });
  t('ctor.values', function () { var r = new D(1, 2, 3, 4); return [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]; });
  t('ctor.negative', function () { var r = new D(10, 20, -3, -4); return [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]; });
  t('ctor.conv', function () { var r = new D('5', { valueOf: function () { return 6; } }, null, undefined); return [r.x, r.y, r.width, r.height]; });
  t('ctor.sym', function () { return new D(Symbol('s')); });
  t('ctor.nan', function () { var r = new D(NaN, 1, 2, Infinity); return [r.x, r.top, r.bottom, r.left, r.right]; });
  t('ctor.negzero', function () { var r = new D(-0, 0, 0, 0); return [Object.is(r.x, -0)]; });
  t('ownkeys', function () { return Object.getOwnPropertyNames(new D(1, 2, 3, 4)); });
  t('keys.enum', function () { var k = []; for (var x in new D(1, 2, 3, 4)) k.push(x); return k; });
  t('json', function () { return JSON.stringify(new D(1, 2, 3, 4)); });
  t('toJSON.direct', function () { return new RO(1, 2, 3, 4).toJSON(); });
  t('tag', function () { return [Object.prototype.toString.call(new D()), Object.prototype.toString.call(new RO())]; });
  t('inherit', function () { var r = new D(); return [r instanceof D, r instanceof RO, new RO() instanceof D, Object.getPrototypeOf(D.prototype) === RO.prototype, Object.getPrototypeOf(D) === RO]; });
  t('set.rect', function () { var r = new D(1, 2, 3, 4); r.x = 10; r.width = -5; r.height = '7'; return [r.x, r.width, r.height, r.left, r.right, r.top, r.bottom]; });
  t('set.readonly', function () { var r = new RO(1, 2, 3, 4); r.x = 99; return [r.x]; });
  t('set.readonly.strict', function () { 'use strict'; var r = new RO(1, 2, 3, 4); r.x = 99; return r.x; });
  t('set.derived.strict', function () { 'use strict'; var r = new D(1, 2, 3, 4); r.top = 99; return r.top; });
  t('set.noarg', function () { var r = new D(); return Object.getOwnPropertyDescriptor(D.prototype, 'x').set.call(r); });
  t('set.sym', function () { var r = new D(); r.x = Symbol('s'); });
  t('illegal.get', function () { return Object.getOwnPropertyDescriptor(RO.prototype, 'x').get.call({}); });
  t('illegal.get.proto', function () { return Object.getOwnPropertyDescriptor(RO.prototype, 'x').get.call(RO.prototype); });
  t('illegal.set', function () { return Object.getOwnPropertyDescriptor(D.prototype, 'x').set.call({}, 1); });
  t('illegal.toJSON', function () { return RO.prototype.toJSON.call({}); });
  t('illegal.cross', function () { return Object.getOwnPropertyDescriptor(RO.prototype, 'x').get.call(Object.create(D.prototype)); });
  t('fromRect.basic', function () { var r = D.fromRect({ x: 1, y: 2, width: 3, height: 4 }); return [r instanceof D, r.x, r.y, r.width, r.height]; });
  t('fromRect.ro', function () { var r = RO.fromRect({ x: 1 }); return [r instanceof D, r instanceof RO, r.x, r.y, r.width]; });
  t('fromRect.none', function () { var r = D.fromRect(); return [r.x, r.y, r.width, r.height]; });
  t('fromRect.null', function () { var r = D.fromRect(null); return [r.x]; });
  t('fromRect.bad', function () { return D.fromRect(5); });
  t('fromRect.order', function () { var log = []; D.fromRect({ get height() { log.push('h'); return 1; }, get x() { log.push('x'); return 1; }, get width() { log.push('w'); return 1; }, get y() { log.push('y'); return 1; } }); return log; });
  t('fromRect.this', function () { var f = D.fromRect; return f({ x: 3 }).x; });
  t('subclass', function () { class S extends D { get sum() { return this.x + this.y; } } var s = new S(1, 2, 3, 4); return [s.sum, s instanceof D, Object.prototype.toString.call(s), s.constructor.name, s.width]; });
  t('subclass.ro', function () { class S extends RO { } var s = new S(5, 6, 7, 8); return [s.right, s instanceof RO]; });
  t('reflect.construct', function () { function F() { } F.prototype = { z: 1 }; var r = Reflect.construct(D, [1, 2, 3, 4], F); return [r.z, Object.getPrototypeOf(r) === F.prototype, typeof r.x]; });
  t('structuredClone', function () { var r = structuredClone(new D(1, 2, 3, 4)); return [Object.prototype.toString.call(r), r.x]; });
  t('toString.fn', function () { return [Function.prototype.toString.call(D), Function.prototype.toString.call(D.prototype.constructor)]; });
  t('freeze', function () { var r = Object.freeze(new D(1, 2, 3, 4)); r.x = 9; return r.x; });
  return JSON.stringify(R);
})()
