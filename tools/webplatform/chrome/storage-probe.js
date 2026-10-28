// Storage / StorageEvent probe. Runs in real Chrome AND in the diting engine;
// the two JSON results are compared (tools/webplatform, JS-PLATFORM-ARCH M2).
// Returns a JSON string. Pure observation: no network, deterministic.
(function () {
  var R = {};
  function t(name, f) { try { R[name] = f(); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + ': ' + (e && e.message); } }
  function desc(o, k) {
    var d = Object.getOwnPropertyDescriptor(o, k);
    if (!d) return null;
    var r = { e: d.enumerable, c: d.configurable };
    if ('value' in d) { r.w = d.writable; r.t = typeof d.value; if (typeof d.value === 'function') { r.len = d.value.length; r.name = d.value.name; r.nat = /\[native code\]/.test(Function.prototype.toString.call(d.value)); } }
    else { r.g = d.get ? { name: d.get.name, len: d.get.length, nat: /\[native code\]/.test(Function.prototype.toString.call(d.get)) } : null; r.s = d.set ? 'fn' : null; }
    return r;
  }
  var SP = Storage.prototype, EP = StorageEvent.prototype;
  t('iface.Storage', function () { return { len: Storage.length, name: Storage.name, proto: desc(Storage, 'prototype'), tag: desc(SP, Symbol.toStringTag), ctor: desc(SP, 'constructor'), own: Object.getOwnPropertyNames(Storage).sort() }; });
  t('order.Storage', function () { return Object.getOwnPropertyNames(SP); });
  t('order.StorageEvent', function () { return Object.getOwnPropertyNames(EP); });
  t('proto.Storage', function () { var o = {}; Object.getOwnPropertyNames(SP).sort().forEach(function (k) { o[k] = desc(SP, k); }); return o; });
  t('iface.StorageEvent', function () { return { len: StorageEvent.length, name: StorageEvent.name, proto: desc(StorageEvent, 'prototype'), tag: desc(EP, Symbol.toStringTag), inherits: Object.getPrototypeOf(EP) === Event.prototype, ctorInherits: Object.getPrototypeOf(StorageEvent) === Event, own: Object.getOwnPropertyNames(StorageEvent).sort() }; });
  t('proto.StorageEvent', function () { var o = {}; Object.getOwnPropertyNames(EP).sort().forEach(function (k) { o[k] = desc(EP, k); }); return o; });
  t('new Storage()', function () { return new Storage(); });
  t('Storage()', function () { return Storage(); });
  t('illegal.length', function () { return Object.getOwnPropertyDescriptor(SP, 'length').get.call({}); });
  t('illegal.getItem', function () { return SP.getItem.call({}, 'a'); });
  t('illegal.clear', function () { return SP.clear.call(undefined); });
  var s = sessionStorage;
  var SET = function (k, v) { return SP.setItem.call(s, k, v); }, GET = function (k) { return SP.getItem.call(s, k); }, CLR = function () { return SP.clear.call(s); }, KEY = function (i) { return SP.key.call(s, i); }, LEN = function () { return Object.getOwnPropertyDescriptor(SP, 'length').get.call(s); };
  CLR();
  t('arity.getItem', function () { return SP.getItem.call(s); });
  t('arity.setItem', function () { return SP.setItem.call(s, 'a'); });
  t('arity.removeItem', function () { return SP.removeItem.call(s); });
  t('arity.key', function () { return SP.key.call(s); });
  t('sym.setItem', function () { return SET(Symbol('x'), '1'); });
  t('conv.setItem', function () { SET(1, { toString: function () { return 'obj'; } }); return [GET('1'), LEN()]; });
  t('conv.undef', function () { SET(undefined, null); return [GET('undefined'), LEN()]; });
  t('key.conv', function () { return [KEY(0), KEY('1'), KEY(-1), KEY(99), KEY(NaN), KEY(4294967296)]; });
  CLR();
  t('assign.new', function () { s.foo = 1; return [s.foo, typeof s.foo, GET('foo'), LEN()]; });
  CLR(); SET('b', '2'); SET('a', '1'); SET('c', '3');
  t('keys.order', function () { return Object.keys(s); });
  t('gopn', function () { return Object.getOwnPropertyNames(s); });
  t('forin', function () { var o = []; for (var k in s) o.push(k); return o.filter(function (k) { return ['a', 'b', 'c'].indexOf(k) >= 0 || /^(key|getItem|setItem|removeItem|clear|length)$/.test(k); }); });
  t('in', function () { return ['a' in s, 'zz' in s, 'getItem' in s, 'toString' in s]; });
  t('hasOwn', function () { return [Object.prototype.hasOwnProperty.call(s, 'a'), Object.prototype.hasOwnProperty.call(s, 'getItem'), Object.prototype.hasOwnProperty.call(s, 'zz')]; });
  t('gopd.item', function () { return desc(s, 'a'); });
  t('gopd.method', function () { return desc(s, 'getItem'); });
  t('delete', function () { return [delete s.a, GET('a'), LEN(), delete s.nothere, delete s.getItem, typeof s.getItem]; });
  t('proto.shadow', function () { CLR(); SET('toString', 'T'); return [typeof s.toString, GET('toString'), Object.keys(s)]; });
  t('symbol.prop', function () { CLR(); var sy = Symbol('q'); s[sy] = 5; return [s[sy], LEN(), Object.getOwnPropertySymbols(s).length]; });
  t('tag', function () { return [Object.prototype.toString.call(s), String(s), s instanceof Storage, s.constructor === Storage]; });
  t('same.obj', function () { return [localStorage === window.localStorage, localStorage !== sessionStorage]; });
  t('freeze', function () { return [Object.isExtensible(s), Object.isFrozen(s)]; });
  // ---- quirks: items can SHADOW prototype members; isolated and last. Restore via prototype methods.
  t('assign.method', function () { CLR(); s.setItem = 'v'; return [typeof s.setItem, s.getItem('setItem'), LEN(), Object.keys(s)]; });
  t('assign.length', function () { CLR(); s.length = 7; return [typeof s.length, GET('length'), Object.keys(s)]; });
  t('define.data', function () { CLR(); Object.defineProperty(s, 'dd', { value: 'x', configurable: true, enumerable: true, writable: true }); return [GET('dd'), desc(s, 'dd')]; });
  t('define.accessor', function () { CLR(); return Reflect.defineProperty(s, 'acc', { get: function () { return 1; }, configurable: true }); });
  t('define.method', function () { CLR(); return [Reflect.defineProperty(s, 'getItem', { value: 'zz', configurable: true }), typeof s.getItem, GET('getItem')]; });
  CLR();
  // ---- StorageEvent
  t('ev.new', function () { var e = new StorageEvent('storage'); return [e.type, e.key, e.oldValue, e.newValue, e.url, e.storageArea, e.bubbles, e.cancelable]; });
  t('ev.init', function () { var e = new StorageEvent('storage', { key: 'k', oldValue: 'o', newValue: 'n', url: 'http://x/', storageArea: localStorage, bubbles: true }); return [e.key, e.oldValue, e.newValue, e.url, e.storageArea === localStorage, e.bubbles]; });
  t('ev.conv', function () { var e = new StorageEvent('storage', { key: 5, newValue: undefined, oldValue: null, url: 7 }); return [e.key, e.newValue, e.oldValue, e.url]; });
  t('ev.arity', function () { return new StorageEvent(); });
  t('ev.call', function () { return StorageEvent('storage'); });
  t('ev.badinit', function () { return new StorageEvent('storage', 5); });
  t('ev.badarea', function () { return new StorageEvent('storage', { storageArea: {} }); });
  t('ev.nullinit', function () { var e = new StorageEvent('storage', null); return [e.key, e.url]; });
  t('ev.ownkeys', function () { return Object.keys(new StorageEvent('storage', { key: 'k' })); });
  t('ev.readonly', function () { var e = new StorageEvent('storage', { key: 'k' }); e.key = 'z'; return e.key; });
  t('ev.illegal', function () { return Object.getOwnPropertyDescriptor(EP, 'key').get.call({}); });
  t('ev.initStorageEvent', function () { var e = document.createEvent('StorageEvent'); e.initStorageEvent('storage', true, false, 'k', 'o', 'n', 'u', localStorage); return [e.type, e.bubbles, e.cancelable, e.key, e.oldValue, e.newValue, e.url, e.storageArea === localStorage, StorageEvent.prototype.initStorageEvent.length]; });
  t('ev.initArity', function () { return new StorageEvent('storage').initStorageEvent(); });
  t('ev.tag', function () { return [Object.prototype.toString.call(new StorageEvent('x')), new StorageEvent('x') instanceof Event]; });
  t('ev.areaBadType', function () { return Reflect.ownKeys(new StorageEvent('x')).length; });
  return JSON.stringify(R);
})()
