// #30 window own-property face: accessor names/flags/toString for every own accessor, the [Replaceable] setter semantics,
// the non-configurable set and the full own-name order. Runs in Chrome AND the engine.
(function () {
  var R = {}, g = window, names = Object.getOwnPropertyNames(g);
  function t(n, f) { try { R[n] = f(); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + e.message; } }
  t('order', function () { return names; });
  t('nonConfigurable', function () { return names.filter(function (n) { return !Object.getOwnPropertyDescriptor(g, n).configurable; }); });
  t('accessors', function () {
    var o = {};
    names.forEach(function (n) {
      var d = Object.getOwnPropertyDescriptor(g, n);
      if (!d || !(d.get || d.set)) return;
      var ts = function (f) { return f ? Function.prototype.toString.call(f) : null; };
      o[n] = [d.get && d.get.name, d.set && d.set.name, d.set ? d.set.length : null, d.configurable, d.enumerable, ts(d.get), ts(d.set)];
    });
    return o;
  });
  t('replace', function () {
    var pos = function (n) { return Object.getOwnPropertyNames(g).indexOf(n); };
    var p0 = pos('innerWidth'), setter = Object.getOwnPropertyDescriptor(g, 'innerWidth').set, o = {};
    g.innerWidth = 5;
    var d = Object.getOwnPropertyDescriptor(g, 'innerWidth');
    o.after = [!!d.get, d.value, d.writable, d.enumerable, d.configurable, pos('innerWidth') === p0];
    (function () { 'use strict'; try { scrollX = 7; o.strict = Object.getOwnPropertyDescriptor(g, 'scrollX').value; } catch (e) { o.strict = 'THROW ' + e.name; } })();
    try { setter.call({}, 1); o.brand = 'ok'; } catch (e) { o.brand = e.name + ': ' + e.message; }
    var ns = Object.getOwnPropertyDescriptor(g, 'navigation').set; ns.call(g); o.noArg = [typeof g.navigation];
    return o;
  });
  return JSON.stringify(R);
})()
