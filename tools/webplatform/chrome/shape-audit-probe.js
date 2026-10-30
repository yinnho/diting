// Whole-surface shape audit: for every global interface object, the exact own-member list (order, kind,
// enumerability), constructor length, static members, prototype parent and toStringTag. Chrome-side
// output is the scoreboard truth; the engine is diffed against it by tools/webplatform/shape_audit.py.
(function () {
  var R = {};
  function kind(o, m) {
    var d = Object.getOwnPropertyDescriptor(o, m);
    var f = (d.enumerable ? 'e' : '-') + (d.configurable ? 'c' : '-') + (d.get || d.set ? '' : (d.writable ? 'w' : '-'));
    if (d.get || d.set) return (d.set ? 'rw' : 'ro') + ':' + f;
    return typeof d.value + (typeof d.value === 'function' ? ':' + d.value.length : '') + ':' + f;
  }
  var names = Object.getOwnPropertyNames(globalThis).filter(function (n) {
    if (!/^[A-Z]/.test(n)) return false;
    var d = Object.getOwnPropertyDescriptor(globalThis, n);
    return d && typeof d.value === 'function' && d.value.prototype !== undefined && Object.getOwnPropertyNames(d.value.prototype).indexOf('constructor') >= 0;
  });
  names.sort().forEach(function (n) {
    var C = globalThis[n], P = C.prototype;
    var s = { stub: !!(globalThis.__diting_stubs && globalThis.__diting_stubs.has(C)), vals: {}, svals: {}, len: C.length, order: Object.getOwnPropertyNames(P), statics: Object.getOwnPropertyNames(C).filter(function (k) { return k !== 'length' && k !== 'name' && k !== 'prototype'; }), kinds: {}, skinds: {}, syms: Object.getOwnPropertySymbols(P).map(String) };
    var pp = Object.getPrototypeOf(P);
    s.proto = pp === Object.prototype ? 'Object' : pp === null ? 'null' : (pp.constructor && pp.constructor.name) || '?';
    s.order.forEach(function (m) { try { s.kinds[m] = kind(P, m); } catch (e) { s.kinds[m] = 'err'; } });
    s.statics.forEach(function (m) { try { s.skinds[m] = kind(C, m); } catch (e) { s.skinds[m] = 'err'; } });
    s.order.forEach(function (m) { var d = Object.getOwnPropertyDescriptor(P, m); if (d && !d.get && !d.set && (typeof d.value === 'number' || typeof d.value === 'string' || typeof d.value === 'boolean')) s.vals[m] = d.value; });
    s.statics.forEach(function (m) { var d = Object.getOwnPropertyDescriptor(C, m); if (d && !d.get && !d.set && (typeof d.value === 'number' || typeof d.value === 'string' || typeof d.value === 'boolean')) s.svals[m] = d.value; });
    s.symdesc = {};
    Object.getOwnPropertySymbols(P).forEach(function (sy) {
      var d = Object.getOwnPropertyDescriptor(P, sy), key = String(sy), o = { f: (d.enumerable ? 'e' : '-') + (d.configurable ? 'c' : '-') + (d.get || d.set ? '' : (d.writable ? 'w' : '-')) };
      if (d.get || d.set) { o.k = 'acc'; }
      else if (typeof d.value === 'function') {
        o.k = 'fn';
        if (d.value === Array.prototype.values) o.ref = 'Array.values';
        else if (d.value === Array.prototype[Symbol.iterator]) o.ref = 'Array.values';
        else { var hit = s.order.filter(function (m) { var dd = Object.getOwnPropertyDescriptor(P, m); return dd && dd.value === d.value; })[0]; if (hit) o.ref = hit; o.fname = d.value.name; o.flen = d.value.length; }
      } else if (typeof d.value === 'string') { o.k = 'str'; o.v = d.value; }
      else if (d.value && typeof d.value === 'object') { o.k = 'obj'; o.keys = Object.keys(d.value); o.np = Object.getPrototypeOf(d.value) === null; }
      s.symdesc[key] = o;
    });
    R[n] = s;
  });
  return JSON.stringify(R);
})()
