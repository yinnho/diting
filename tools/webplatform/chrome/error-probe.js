// Error-shape probe for platform-generated APIs (JS-PLATFORM-ARCH sec 13).
// For each throwing call: the stack's first line, e.message, how many frames
// sit ABOVE the probe's own `cap` frame (0 = no shim frames leaked), and the
// own-property shape of the error object. Runs in real Chrome and the engine.
(function () {
  var R = {};
  function shape(e) {
    var o = {};
    ['message', 'stack'].forEach(function (k) {
      var d = Object.getOwnPropertyDescriptor(e, k);
      o[k] = d ? (('value' in d) ? 'data' : 'accessor') + ':' + (d.writable === undefined ? '-' : d.writable ? 'w' : 'r') + (d.enumerable ? 'e' : '-') + (d.configurable ? 'c' : '-') : null;
    });
    o.own = Object.getOwnPropertyNames(e).sort();
    return o;
  }
  function cap(name, f) {
    try { f(); R[name] = 'no-throw'; return; } catch (e) {
      var lines = String(e.stack).split('\n');
      var capIdx = -1;
      for (var i = 1; i < lines.length; i++) { if (/\bat cap\b/.test(lines[i])) { capIdx = i; break; } }
      R[name] = { ctor: e.constructor.name, header: lines[0], message: e.message, framesAbove: capIdx < 0 ? 'no-cap-frame' : capIdx - 1, shape: shape(e) };
    }
  }
  var SP = Storage.prototype, s = sessionStorage;
  cap('illegal.getItem', function () { SP.getItem.call({}, 'a'); });
  cap('illegal.length', function () { Object.getOwnPropertyDescriptor(SP, 'length').get.call({}); });
  cap('illegal.clear', function () { SP.clear.call(undefined); });
  cap('arity.getItem', function () { s.getItem(); });
  cap('arity.setItem', function () { s.setItem('a'); });
  cap('arity.key', function () { s.key(); });
  cap('sym.setItem', function () { s.setItem(Symbol('x'), '1'); });
  cap('new.Storage', function () { new Storage(); });
  cap('call.Storage', function () { Storage(); });
  cap('ev.arity', function () { new StorageEvent(); });
  cap('ev.call', function () { StorageEvent('x'); });
  cap('ev.badinit', function () { new StorageEvent('x', 5); });
  cap('ev.badarea', function () { new StorageEvent('x', { storageArea: {} }); });
  cap('ev.illegal', function () { Object.getOwnPropertyDescriptor(StorageEvent.prototype, 'key').get.call({}); });
  cap('ev.initArity', function () { new StorageEvent('x').initStorageEvent(); });
  cap('ev.initBadArea', function () { new StorageEvent('x').initStorageEvent('x', false, false, null, null, null, '', {}); });
  cap('set.symbolValue', function () { s.foo = Symbol('v'); });
  return JSON.stringify(R);
})()
