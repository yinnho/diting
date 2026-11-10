// CSSUnitValue face probe (issue #67 pilot). Runs in real Chrome AND in the
// diting engine; results are diffed. Same shape() contract as
// geometry-probe.js. Returns a JSON string.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + (e && e.name && e.name !== (e.constructor && e.constructor.name) ? '(' + e.name + ')' : '') + ': ' + (e && e.message); } }
  function shape(C, inst) {
    var P = C.prototype, o = {};
    Object.getOwnPropertyNames(P).sort().forEach(function (k) {
      var d = Object.getOwnPropertyDescriptor(P, k);
      o[k] = d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value;
    });
    var st = {};
    Object.getOwnPropertyNames(C).sort().forEach(function (k) {
      if (k === 'prototype' || k === 'length' || k === 'name') return;
      var d = Object.getOwnPropertyDescriptor(C, k);
      st[k] = (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value) + (d.enumerable ? '+e' : '');
    });
    return { len: C.length, name: C.name, members: o, statics: st,
      order: Object.getOwnPropertyNames(P),
      parent: Object.getPrototypeOf(P) === Object.prototype ? 'Object' : (Object.getPrototypeOf(P).constructor || {}).name,
      tag: Object.prototype.toString.call(inst) };
  }
  var C = self.CSSUnitValue;
  // ---- shape
  t('shape.CSSUnitValue', function () { return shape(C, new CSSUnitValue(1, 'px')); });
  // ---- ctor error ladders (message text + own-enum of message)
  t('ctor.arity', function () { new CSSUnitValue(1); });
  t('ctor.arity0', function () { new CSSUnitValue(); });
  t('ctor.nan', function () { new CSSUnitValue(NaN, 'px'); });
  t('ctor.infinity', function () { new CSSUnitValue(Infinity, 'px'); });
  t('ctor.symbol', function () { new CSSUnitValue(Symbol('x'), 'px'); });
  t('ctor.badunit', function () { new CSSUnitValue(1, 'bogus'); });
  t('ctor.badunit.upper', function () { new CSSUnitValue(1, 'BOGUS'); });
  t('ctor.unitnum', function () { var u = new CSSUnitValue(1, 5); return [u.value, u.unit]; });
  t('ctor.valstr', function () { var u = new CSSUnitValue('  2.5', 'PX'); return [u.value, u.unit]; });
  t('ctor.enumerable', function () { try { new CSSUnitValue(1); } catch (e) { return Object.getOwnPropertyDescriptor(e, 'message').enumerable; } });
  // ---- accessor behavior
  t('get.value', function () { return new CSSUnitValue(1.5, 'px').value; });
  t('get.unit', function () { return new CSSUnitValue(1.5, 'px').unit; });
  t('set.value', function () { var u = new CSSUnitValue(1, 'px'); u.value = 9; return u.value; });
  t('set.value.nan', function () { var u = new CSSUnitValue(1, 'px'); u.value = 'x'; return u.value; });
  t('set.unit', function () { var u = new CSSUnitValue(1, 'px'); try { u.unit = 'em'; return 'SET-THREW-NOT'; } catch (e) { return 'ro-ok ' + u.unit; } });
  t('nonew', function () { CSSUnitValue(1, 'px'); });
  t('json', function () { return JSON.stringify(new CSSUnitValue(1, 'px')); });
  return JSON.stringify(R, null, 1);
})()
