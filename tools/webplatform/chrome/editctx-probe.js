// EditContext family probe (M4 knife 18): EditContext, TextFormat, TextUpdateEvent,
// TextFormatUpdateEvent, CharacterBoundsUpdateEvent. Runs in Chrome AND the engine.
(function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : (typeof x === 'number' && !isFinite(x) ? '#' + x : x); })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 200); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) {
    var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), len: C.length, statics: Object.getOwnPropertyNames(C).filter(function (k) { return k !== 'length' && k !== 'name' && k !== 'prototype'; }), kinds: {}, proto: Object.getPrototypeOf(P).constructor.name, tag: Object.prototype.toString.call(Object.create(P)) };
    s.order.forEach(function (m) { s.kinds[m] = kind(P, m); });
    return s;
  }
  ['EditContext', 'TextFormat', 'TextUpdateEvent', 'TextFormatUpdateEvent', 'CharacterBoundsUpdateEvent'].forEach(function (n) { R['shape.' + n] = typeof globalThis[n] === 'function' ? shape(globalThis[n]) : 'absent'; });
  t('call', function () { try { EditContext(); } catch (e) { return e.name + ': ' + e.message; } });
  t('inst', function () { var c = new EditContext(); return [c instanceof EventTarget, Object.prototype.toString.call(c), Object.getOwnPropertyNames(c)]; });
  t('defaults', function () { var c = new EditContext(); return { text: c.text, ss: c.selectionStart, se: c.selectionEnd, cbrs: c.characterBoundsRangeStart, attached: c.attachedElements(), cb: c.characterBounds(), ontextupdate: c.ontextupdate, onblur: c.onblur, oncompositionstart: c.oncompositionstart }; });
  t('init', function () { var c = new EditContext({ text: 'hello', selectionStart: 1, selectionEnd: 3 }); return [c.text, c.selectionStart, c.selectionEnd]; });
  t('init.bad', function () { var o = []; [5, 'x', { selectionStart: -1 }, { selectionStart: 'a' }, { text: 5 }, { selectionStart: 9, selectionEnd: 2 }].forEach(function (v) { try { var c = new EditContext(v); o.push([c.text, c.selectionStart, c.selectionEnd]); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('init.null', function () { var c = new EditContext(null); return [c.text, c.selectionStart]; });
  t('updateText', function () { var c = new EditContext({ text: 'abcdef' }); c.updateText(1, 3, 'XYZ'); return [c.text]; });
  t('updateText.beyond', function () { var c = new EditContext({ text: 'abc' }); c.updateText(2, 99, 'Z'); var a = c.text; c.updateText(50, 60, 'Q'); return [a, c.text]; });
  t('updateText.reverse', function () { var c = new EditContext({ text: 'abcdef' }); c.updateText(4, 1, 'Z'); return c.text; });
  t('updateText.args', function () { var c = new EditContext(); try { c.updateText(1, 2); } catch (e) { return e.name + ': ' + e.message; } });
  t('updateText.conv', function () { var c = new EditContext({ text: 'abc' }); c.updateText('1', 2.9, 5); return c.text; });
  t('updateSelection', function () { var c = new EditContext({ text: 'abcdef' }); c.updateSelection(2, 4); return [c.selectionStart, c.selectionEnd]; });
  t('updateSelection.beyond', function () { var c = new EditContext({ text: 'abc' }); c.updateSelection(5, 9); return [c.selectionStart, c.selectionEnd]; });
  t('updateSelection.reverse', function () { var c = new EditContext({ text: 'abcdef' }); c.updateSelection(4, 1); return [c.selectionStart, c.selectionEnd]; });
  t('updateSelection.args', function () { var c = new EditContext(); try { c.updateSelection(1); } catch (e) { return e.name + ': ' + e.message; } });
  t('selection.set', function () { var c = new EditContext({ text: 'abc' }); c.selectionStart = 2; var a = [c.selectionStart, c.selectionEnd]; c.selectionEnd = 1; return [a, c.selectionStart, c.selectionEnd]; });
  t('selection.set.beyond', function () { var c = new EditContext({ text: 'abc' }); c.selectionEnd = 99; return [c.selectionStart, c.selectionEnd]; });
  t('text.readonly', function () { var c = new EditContext({ text: 'abc' }); c.text = 'zz'; return c.text; });
  t('bounds.update', function () { var c = new EditContext({ text: 'abc' }); c.updateCharacterBounds(1, [new DOMRect(1, 2, 3, 4), new DOMRect(5, 6, 7, 8)]); return [c.characterBoundsRangeStart, c.characterBounds().map(function (r) { return [r.x, r.y, r.width, r.height]; })]; });
  t('bounds.fresh', function () { var c = new EditContext({ text: 'abc' }); c.updateCharacterBounds(0, [new DOMRect(1, 1, 1, 1)]); return [c.characterBounds() === c.characterBounds(), c.characterBounds()[0] instanceof DOMRect]; });
  t('bounds.bad', function () { var c = new EditContext(); try { c.updateCharacterBounds(0, [{}]); return c.characterBounds().length; } catch (e) { return e.name + ': ' + e.message; } });
  t('bounds.args', function () { var c = new EditContext(); try { c.updateCharacterBounds(0); } catch (e) { return e.name + ': ' + e.message; } });
  t('control.bounds', function () { var c = new EditContext(); c.updateControlBounds(new DOMRect(1, 2, 3, 4)); c.updateSelectionBounds(new DOMRect(5, 6, 7, 8)); return 'ok'; });
  t('control.bounds.bad', function () { var c = new EditContext(); var o = []; [function () { c.updateControlBounds(); }, function () { c.updateControlBounds({}); }, function () { c.updateSelectionBounds(5); }].forEach(function (f) { try { f(); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('attach.el', function () { var c = new EditContext(); var d = document.createElement('div'); d.editContext = c; var a = c.attachedElements(); var o = [a.length, a[0] === d, d.editContext === c]; d.editContext = null; o.push(c.attachedElements().length, d.editContext); return o; });
  t('attach.bad', function () { var d = document.createElement('div'); try { d.editContext = {}; } catch (e) { return e.name + ': ' + e.message; } });
  t('attach.twice', function () { var c = new EditContext(); var d = document.createElement('div'), e = document.createElement('div'); d.editContext = c; try { e.editContext = c; return [c.attachedElements().length, d.editContext === c, e.editContext === c]; } catch (x) { return x.name + ': ' + x.message; } });
  t('attach.invalid', function () { var c = new EditContext(); var d = document.createElement('input'); try { d.editContext = c; return 'ok'; } catch (e) { return e.name + ': ' + e.message; } });
  t('attach.inherit', function () { var d = document.createElement('div'); return [typeof Object.getOwnPropertyDescriptor(Element.prototype, 'editContext'), 'editContext' in HTMLElement.prototype, d.editContext]; });
  t('brand', function () { var o = []; ['updateText', 'updateSelection', 'updateControlBounds', 'attachedElements', 'characterBounds'].forEach(function (m) { try { EditContext.prototype[m].call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['text', 'selectionStart', 'characterBoundsRangeStart'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(EditContext.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('events.handlers', function () { var c = new EditContext(); var o = {}; ['ontextupdate', 'ontextformatupdate', 'oncharacterboundsupdate', 'oncompositionstart', 'oncompositionend', 'onblur'].forEach(function (k) { o[k] = [k in c, c[k]]; var f = function () {}; c[k] = f; o[k].push(c[k] === f); c[k] = 5; o[k].push(c[k]); }); return o; });
  // ---- TextFormat
  t('tf.ctor', function () { var f = new TextFormat(); return [f.rangeStart, f.rangeEnd, f.textColor, f.backgroundColor, f.underlineColor, f.underlineStyle, f.underlineThickness]; });
  t('tf.init', function () { var f = new TextFormat({ rangeStart: 1, rangeEnd: 4, underlineStyle: 'dotted', underlineThickness: 'thick', textColor: 'red' }); return [f.rangeStart, f.rangeEnd, f.underlineStyle, f.underlineThickness, f.textColor]; });
  t('tf.bad', function () { var o = []; [{ underlineStyle: 'zz' }, { underlineThickness: 'zz' }, { rangeStart: -1 }, 5].forEach(function (v) { try { new TextFormat(v); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('tf.set', function () { var f = new TextFormat(); f.rangeStart = 3; f.underlineStyle = 'wavy'; f.underlineStyle = 'zz'; f.textColor = 'blue'; return [f.rangeStart, f.underlineStyle, f.textColor]; });
  t('tf.call', function () { try { TextFormat(); } catch (e) { return e.name + ': ' + e.message; } });
  // ---- events
  t('tue.ctor', function () { var e = new TextUpdateEvent('textupdate', { text: 'hi', updateRangeStart: 1, updateRangeEnd: 2, selectionStart: 3, selectionEnd: 4 }); return [e.type, e.text, e.updateRangeStart, e.updateRangeEnd, e.selectionStart, e.selectionEnd, e instanceof Event, e.bubbles]; });
  t('tue.defaults', function () { var e = new TextUpdateEvent('textupdate'); return [e.text, e.updateRangeStart, e.updateRangeEnd, e.selectionStart, e.selectionEnd]; });
  t('tue.noargs', function () { try { new TextUpdateEvent(); } catch (e) { return e.name + ': ' + e.message; } });
  t('tfue.ctor', function () { var f = new TextFormat({ rangeStart: 1 }); var e = new TextFormatUpdateEvent('textformatupdate', { textFormats: [f] }); return [e.getTextFormats().length, e.getTextFormats()[0] instanceof TextFormat, e.getTextFormats() === e.getTextFormats()]; });
  t('tfue.empty', function () { return new TextFormatUpdateEvent('x').getTextFormats(); });
  t('cbue.ctor', function () { var e = new CharacterBoundsUpdateEvent('characterboundsupdate', { rangeStart: 2, rangeEnd: 5 }); return [e.rangeStart, e.rangeEnd, e instanceof Event]; });
  t('cbue.defaults', function () { var e = new CharacterBoundsUpdateEvent('x'); return [e.rangeStart, e.rangeEnd]; });
  t('ev.getters', function () { var o = []; [[TextUpdateEvent, 'text'], [CharacterBoundsUpdateEvent, 'rangeStart']].forEach(function (p) { try { Object.getOwnPropertyDescriptor(p[0].prototype, p[1]).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  return JSON.stringify(R);
})()
