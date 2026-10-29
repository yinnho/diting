// DOMException shape + behaviour probe. Runs in real Chrome, the diting engine,
// and the diting_runtime native-pilot test (bare V8). Returns a JSON string.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + ': ' + (e && e.message); } }
  function nat(f) { return /\[native code\]/.test(Function.prototype.toString.call(f)); }
  function desc(o, k) {
    var d = Object.getOwnPropertyDescriptor(o, k);
    if (!d) return null;
    var r = { e: d.enumerable, c: d.configurable };
    if ('value' in d) { r.w = d.writable; r.t = typeof d.value; if (typeof d.value === 'function') { r.len = d.value.length; r.name = d.value.name; r.nat = nat(d.value); } else r.v = (d.value !== null && typeof d.value === 'object') ? '#object' : d.value; }
    else { r.g = d.get ? { name: d.get.name, len: d.get.length, nat: nat(d.get) } : null; r.s = d.set ? 'fn' : null; }
    return r;
  }
  var D = DOMException, P = D.prototype;
  t('shape', function () { var o = {}; Object.getOwnPropertyNames(P).sort().forEach(function (k) { o[k] = desc(P, k); }); Object.getOwnPropertySymbols(P).forEach(function (k) { o[String(k)] = desc(P, k); });
    return { len: D.length, name: D.name, proto: desc(D, 'prototype'), own: Object.getOwnPropertyNames(D).sort(), order: Object.getOwnPropertyNames(P), members: o, parentIsErrorProto: Object.getPrototypeOf(P) === Error.prototype, ctorParent: Object.getPrototypeOf(D) === Function.prototype ? 'Function' : '?' }; });
  t('global.desc', function () { return desc(globalThis, 'DOMException'); });
  t('ctor.noargs', function () { var e = new D(); return [e.name, e.message, e.code, String(e)]; });
  t('ctor.args', function () { var e = new D('m', 'AbortError'); return [e.name, e.message, e.code, String(e), e instanceof Error, Object.prototype.toString.call(e)]; });
  t('ctor.one', function () { var e = new D('only'); return [e.name, e.message, e.code]; });
  t('ownkeys', function () { return Object.getOwnPropertyNames(new D('m', 'AbortError')); });
  t('keys.forin', function () { var e = new D('m', 'AbortError'), k = []; for (var x in e) k.push(x); return k; });
  t('unknownName', function () { var e = new D('m', 'Nope'); return [e.name, e.code]; });
  t('legacyCodes', function () { return ['IndexSizeError', 'InvalidStateError', 'DataCloneError', 'AbortError', 'TimeoutError', 'NotFoundError', 'QuotaExceededError', 'SyntaxError'].map(function (n) { return new D('', n).code; }); });
  t('assign', function () { var e = new D('m', 'AbortError'); e.name = 'X'; e.message = 'Y'; e.code = 9; return [e.name, e.message, e.code]; });
  t('assign.strict', function () { 'use strict'; var e = new D('m', 'AbortError'); e.name = 'X'; });
  t('call', function () { return D('a'); });
  t('stack', function () { var e = new D('m', 'AbortError'); return [typeof e.stack, 'stack' in e]; });
  t('illegal.name', function () { return Object.getOwnPropertyDescriptor(P, 'name').get.call({}); });
  t('illegal.proto', function () { return P.name; });
  t('illegal.message', function () { return Object.getOwnPropertyDescriptor(P, 'message').get.call(Object.create(P)); });
  t('consts', function () { return [D.ABORT_ERR, P.ABORT_ERR, desc(D, 'ABORT_ERR'), desc(P, 'ABORT_ERR'), D.DATA_CLONE_ERR, D.VALIDATION_ERR, D.DOMSTRING_SIZE_ERR]; });
  t('consts.assign', function () { D.ABORT_ERR = 5; return D.ABORT_ERR; });
  t('tag', function () { return [desc(P, Symbol.toStringTag), Object.prototype.toString.call(P)]; });
  t('msgConv', function () { return new D({ toString: function () { return 'o'; } }, { toString: function () { return 'AbortError'; } }).message; });
  t('msgConv.num', function () { var e = new D(12, 34); return [e.message, e.name]; });
  t('msgConv.undef', function () { var e = new D(undefined, undefined); return [e.message, e.name]; });
  t('msgConv.null', function () { var e = new D(null, null); return [e.message, e.name]; });
  t('sym', function () { return new D(Symbol('s')); });
  t('sym.name', function () { return new D('m', Symbol('s')); });
  t('toString', function () { var e = new D('boom', 'AbortError'); return [e.toString(), Error.prototype.toString.call(e), String(new D('', 'AbortError')), String(new D('only msg'))]; });
  t('json', function () { return JSON.stringify(new D('m', 'AbortError')); });
  t('subclass', function () { class E extends D { constructor() { super('sub', 'OperationError'); this.extra = 1; } } var e = new E(); return [e.name, e.message, e.code, e.extra, e instanceof D, e instanceof Error, Object.prototype.toString.call(e), e.constructor.name]; });
  t('subclass.ownkeys', function () { class E extends D { constructor() { super('sub', 'x'); this.extra = 1; } } return Object.getOwnPropertyNames(new E()); });
  t('reflect.construct', function () { function F() { } F.prototype = { z: 1 }; var e = Reflect.construct(D, ['m', 'AbortError'], F); return [e.z, Object.getPrototypeOf(e) === F.prototype, typeof e.name]; });
  t('captureStackTrace', function () { var e = new D('m', 'AbortError'); Error.captureStackTrace(e); return [typeof e.stack, Object.getOwnPropertyNames(e)]; });
  t('throwcatch', function () { try { throw new D('thrown', 'DataCloneError'); } catch (e) { return [e.name, e.code, e instanceof D]; } });
  t('instanceof', function () { var e = new D('x'); return [e instanceof D, e instanceof Error, Error.prototype.isPrototypeOf(e), D.prototype.constructor === D]; });
  t('freeze', function () { var e = Object.freeze(new D('m', 'AbortError')); return [e.name, Object.isFrozen(e)]; });
  t('defineProp', function () { var e = new D('m', 'AbortError'); Object.defineProperty(e, 'name', { value: 'Own' }); return [e.name, Object.getOwnPropertyNames(e)]; });
  t('cause.like', function () { var e = new D('m', 'AbortError'); e.cause = 5; return [e.cause, Object.getOwnPropertyNames(e)]; });
  return JSON.stringify(R);
})()
