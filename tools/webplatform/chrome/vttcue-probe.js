// VTTCue face probe (issue #67 marginal-cost family). Runs in real Chrome AND
// in the diting engine; results are diffed. Same shape() contract as
// cssunit-probe.js. Returns a JSON string.
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
  function cue() { return new VTTCue(1, 2, 'hi'); }
  function all(c) { return [c.startTime, c.endTime, c.text, c.vertical, c.snapToLines, c.line, c.position, c.size, c.align, c.id, c.pauseOnExit, c.track]; }
  t('shape.VTTCue', function () { return shape(self.VTTCue, cue()); });
  t('desc.window', function () { var d = Object.getOwnPropertyDescriptor(self, 'VTTCue'); return [d.writable, d.enumerable, d.configurable]; });
  t('desc.proto', function () { return Object.getOwnPropertyNames(VTTCue.prototype).map(function (k) { var d = Object.getOwnPropertyDescriptor(VTTCue.prototype, k); return k + ':' + (d.enumerable ? 'e' : '-') + (d.configurable ? 'c' : '-') + ('writable' in d ? (d.writable ? 'w' : '-') : ''); }); });
  t('fn.lengths', function () { var P = VTTCue.prototype, o = {}; Object.getOwnPropertyNames(P).forEach(function (k) { var d = Object.getOwnPropertyDescriptor(P, k); if (d.get) o['get ' + k] = [d.get.name, d.get.length]; if (d.set) o['set ' + k] = [d.set.name, d.set.length]; if (typeof d.value === 'function') o[k] = [d.value.name, d.value.length]; }); return o; });
  t('ctor.defaults', function () { return all(cue()); });
  t('ctor.own', function () { return Reflect.ownKeys(cue()).length; });
  t('ctor.et', function () { var c = cue(), n = 0; c.addEventListener('enter', function () { n++; }); c.dispatchEvent(new Event('enter')); return [n, c instanceof EventTarget, c instanceof TextTrackCue]; });
  t('ctor.arity0', function () { new VTTCue(); });
  t('ctor.arity2', function () { new VTTCue(0, 1); });
  t('ctor.nanStart', function () { new VTTCue(NaN, 1, ''); });
  t('ctor.infStart', function () { new VTTCue(Infinity, 1, ''); });
  t('ctor.nanEnd', function () { new VTTCue(0, NaN, ''); });
  t('ctor.infEnd', function () { return all(new VTTCue(0, Infinity, '')); });
  t('ctor.negEnd', function () { return all(new VTTCue(5, -1, '')); });
  t('ctor.strNums', function () { return all(new VTTCue(' 2', '3.5', 7)); });
  t('ctor.symText', function () { new VTTCue(0, 1, Symbol('x')); });
  t('ctor.symStart', function () { new VTTCue(Symbol('x'), 1, ''); });
  t('ctor.objText', function () { return new VTTCue(0, 1, { toString: function () { return 'obj'; } }).text; });
  t('ctor.nullText', function () { return new VTTCue(0, 1, null).text; });
  t('ctor.order', function () { var log = []; try { new VTTCue({ valueOf: function () { log.push('s'); return 1; } }, { valueOf: function () { log.push('e'); return NaN; } }, { toString: function () { log.push('t'); return ''; } }); } catch (e) { log.push(e.message); } return log; });
  t('ctor.enumMsg', function () { try { new VTTCue(1); } catch (e) { return Object.getOwnPropertyDescriptor(e, 'message').enumerable; } });
  t('nonew', function () { VTTCue(0, 1, ''); });
  t('set.vertical', function () { var c = cue(); c.vertical = 'rl'; var a = c.vertical; c.vertical = 'bogus'; return [a, c.vertical]; });
  t('set.align', function () { var c = cue(); c.align = 'left'; var a = c.align; c.align = 'nope'; return [a, c.align]; });
  t('set.line', function () { var c = cue(); c.line = -3.5; var a = c.line; c.line = 'auto'; return [a, c.line]; });
  t('set.lineBad', function () { var c = cue(); c.line = 'x'; });
  t('set.lineNaN', function () { var c = cue(); c.line = NaN; });
  t('set.position', function () { var c = cue(); c.position = 40; var a = c.position; c.position = 'auto'; return [a, c.position]; });
  t('set.positionRange', function () { var c = cue(); c.position = 101; });
  t('set.size', function () { var c = cue(); c.size = 0; return c.size; });
  t('set.sizeRange', function () { var c = cue(); c.size = -1; });
  t('set.sizeNaN', function () { var c = cue(); c.size = NaN; });
  t('set.text', function () { var c = cue(); c.text = 5; return c.text; });
  t('set.snap', function () { var c = cue(); c.snapToLines = 0; return c.snapToLines; });
  t('get.illegal', function () { return Object.getOwnPropertyDescriptor(VTTCue.prototype, 'text').get.call({}); });
  t('html', function () { var f = new VTTCue(0, 1, '<b>x</b> &amp; <c.a.b>y</c><v Bob>z</v>').getCueAsHTML(); var d = document.createElement('div'); d.appendChild(f); return d.innerHTML; });
  t('html.illegal', function () { VTTCue.prototype.getCueAsHTML.call({}); });
  t('json', function () { return JSON.stringify(cue()); });
  return JSON.stringify(R, null, 1);
})()
