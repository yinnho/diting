// Form-family behavior probe (M4 knife 7): the stateful / method members the
// reflect fingerprints cannot classify. Runs in Chrome AND the engine; diffed.
(function () {
  var R = {};
  // page URL -> '#B', origin -> '#O' (Chrome's ephemeral server vs the test page)
  function norm(str) {
    if (!/^https?:/.test(location.origin)) return str;
    return str.split(location.href).join('#B').split(location.origin).join('#O');
  }
  function t(n, f) {
    try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(norm(JSON.stringify(v))); }
    catch (e) { R[n] = norm('THROW ' + e.name + ': ' + String(e.message).slice(0, 200)); }
  }
  function mk(tag) { return document.createElement(tag); }
  var d = document.body;
  t('input.showPicker', function () { return mk('input').showPicker(); });
  t('select.showPicker', function () { return mk('select').showPicker(); });
  t('input.webkitEntries', function () { var e = mk('input'); e.type = 'file'; var w = e.webkitEntries; return [Array.isArray(w), w.length, Object.prototype.toString.call(w)]; });
  t('input.webkitEntries.text', function () { var w = mk('input').webkitEntries; return [w === null || w === undefined ? 'nullish' : w.length]; });
  t('input.list', function () {
    var dl = mk('datalist'); dl.id = 'dl1'; d.appendChild(dl);
    var i = mk('input'); d.appendChild(i);
    var a = i.list; i.setAttribute('list', 'dl1'); var b = i.list === dl; i.setAttribute('list', 'nope'); var c = i.list;
    return [a, b, c];
  });
  t('input.imgdims', function () { var i = mk('input'); i.type = 'image'; i.setAttribute('width', '33'); i.height = 7; return [i.width, i.height, i.getAttribute('height'), mk('input').width]; });
  t('popoverTarget', function () {
    var p = mk('div'); p.id = 'pp'; p.setAttribute('popover', ''); d.appendChild(p);
    var b = mk('button'); d.appendChild(b);
    var r0 = b.popoverTargetElement; b.setAttribute('popovertarget', 'pp'); var r1 = b.popoverTargetElement === p;
    b.popoverTargetElement = null; var r2 = b.popoverTargetElement; var r3 = b.getAttribute('popovertarget');
    return [r0, r1, r2, r3];
  });
  t('commandFor', function () {
    var p = mk('div'); p.id = 'cc'; d.appendChild(p);
    var b = mk('button'); d.appendChild(b);
    b.setAttribute('commandfor', 'cc'); return [b.commandForElement === p, b.command];
  });
  t('select.list', function () {
    var s = mk('select'); s.innerHTML = '<option value=a>A</option><option value=b name=nb>B</option>';
    return [s.length, s.item(1).value, s.item(5), s.namedItem('nb') && s.namedItem('nb').value, s.namedItem('zz'), s.item.length, s.namedItem.length];
  });
  t('select.length.set', function () { var s = mk('select'); s.length = 3; return [s.length, s.options.length]; });
  t('option.index', function () { var s = mk('select'); s.innerHTML = '<option>A</option><option>B</option>'; return [s.options[1].index, mk('option').index]; });
  t('option.defaultSelected', function () { var o = mk('option'); o.defaultSelected = true; return [o.defaultSelected, o.hasAttribute('selected'), o.selected]; });
  t('output.defaultValue', function () { var o = mk('output'); o.textContent = 'hi'; var a = [o.defaultValue, o.value]; o.value = 'x'; a.push(o.defaultValue, o.value, o.textContent); return a; });
  t('output.val', function () { var o = mk('output'); return [o.type, o.value, o.htmlFor instanceof DOMTokenList, o.willValidate, o.checkValidity(), o.validationMessage, o.validity && o.validity.valid]; });
  t('object.val', function () { var o = mk('object'); return [o.willValidate, o.checkValidity(), o.reportValidity(), o.validationMessage, o.validity.valid]; });
  t('object.scv', function () { var o = mk('object'); o.setCustomValidity('x'); return [o.validationMessage, o.validity.customError, o.checkValidity(), o.willValidate]; });
  t('object.svg', function () { var o = mk('object'); return [o.getSVGDocument(), o.contentDocument, o.contentWindow]; });
  t('fieldset.val', function () { var f = mk('fieldset'); return [f.type, f.willValidate, f.checkValidity(), f.validity.valid, f.validationMessage]; });
  t('fieldset.elements', function () { var f = mk('fieldset'); f.innerHTML = '<input name=a><select></select><div></div><button></button>'; var e = f.elements; return [e.length, Object.prototype.toString.call(e), e[0].name]; });
  t('textarea.defaultValue', function () { var a = mk('textarea'); a.textContent = 'dv'; var r = [a.defaultValue, a.value]; a.value = 'v2'; r.push(a.defaultValue, a.textContent); a.defaultValue = 'dv3'; r.push(a.textContent, a.value); return r; });
  t('textarea.textLength', function () { var a = mk('textarea'); a.value = 'héllo'; return [a.textLength]; });
  t('marquee', function () { var m = mk('marquee'); return [m.scrollAmount, m.scrollDelay, m.loop, m.behavior, m.direction, m.trueSpeed, typeof m.start, typeof m.stop, m.start(), m.stop(), m.start.length]; });
  t('marquee.set', function () { var m = mk('marquee'); m.scrollAmount = 10; m.scrollDelay = 30; m.loop = 3; m.behavior = 'ALTERNATE'; m.direction = 'up'; return [m.scrollAmount, m.scrollDelay, m.loop, m.behavior, m.getAttribute('behavior'), m.direction]; });
  t('marquee.bad', function () { var m = mk('marquee'); var r = []; try { m.scrollDelay = 5; r.push(m.scrollDelay); } catch (e) { r.push(e.name); } try { m.loop = 0; r.push(m.loop); } catch (e) { r.push(e.name); } try { m.loop = -5; r.push(m.loop); } catch (e) { r.push(e.name); } m.setAttribute('behavior', 'zzz'); r.push(m.behavior); m.setAttribute('direction', 'zzz'); r.push(m.direction); m.setAttribute('scrolldelay', '30'); r.push(m.scrollDelay); return r; });
  t('meter', function () { var m = mk('meter'); var a = [m.min, m.max, m.value, m.low, m.high, m.optimum]; m.setAttribute('min', '10'); m.setAttribute('max', '20'); m.setAttribute('value', '15'); a.push(m.low, m.high, m.optimum); m.setAttribute('low', '12'); m.setAttribute('high', '18'); m.setAttribute('optimum', '99'); a.push(m.low, m.high, m.optimum); m.low = 13; a.push(m.getAttribute('low')); return a; });
  t('meter.base', function () { var m = mk('meter'); m.setAttribute('min', '10'); m.setAttribute('max', '20'); m.setAttribute('value', '50'); return [m.value, m.max, m.min, m.labels && m.labels.length]; });
  t('progress.position', function () { var p = mk('progress'); var a = [p.position, p.value, p.max]; p.setAttribute('max', '4'); p.value = 1; a.push(p.position, p.value); return a; });
  t('map.areas', function () { var m = mk('map'); m.innerHTML = '<area id=a1><div><area id=a2></div>'; var a = m.areas; return [a.length, Object.prototype.toString.call(a), a[0].id, a[1].id]; });
  t('slot.assign', function () { var s = mk('slot'); return [typeof s.assign, s.assign.length, s.assign(), s.assignedNodes().length, s.assignedElements().length]; });
  t('track', function () { var tr = mk('track'); return [tr.readyState, tr.track && Object.prototype.toString.call(tr.track), tr.kind, tr.default, tr.label, tr.srclang, HTMLTrackElement.NONE, HTMLTrackElement.LOADING, HTMLTrackElement.LOADED, HTMLTrackElement.ERROR]; });
  t('track.kind', function () { var tr = mk('track'); tr.kind = 'CAPTIONS'; var a = [tr.kind]; tr.kind = 'zzz'; a.push(tr.kind, tr.getAttribute('kind')); return a; });
  t('template.shadow', function () { var tp = mk('template'); var a = [tp.shadowRootMode, tp.shadowRootDelegatesFocus, tp.shadowRootClonable, tp.shadowRootSerializable]; tp.shadowRootMode = 'open'; a.push(tp.shadowRootMode, tp.getAttribute('shadowrootmode')); tp.setAttribute('shadowrootmode', 'ZZZ'); a.push(tp.shadowRootMode); return a; });
  t('input.defaults', function () { var i = mk('input'); return [i.defaultChecked, i.defaultValue, i.autocomplete, i.formAction, i.formMethod, i.formEnctype, i.formTarget, i.dirName, i.accept, i.pattern, i.useMap, i.align, i.alt, i.readOnly, i.required, i.webkitdirectory]; });
  t('input.defaultValue', function () { var i = mk('input'); i.defaultValue = 'dv'; var a = [i.defaultValue, i.getAttribute('value'), i.value]; i.value = 'x'; a.push(i.defaultValue, i.getAttribute('value')); return a; });
  t('input.defaultChecked', function () { var i = mk('input'); i.type = 'checkbox'; i.defaultChecked = true; var a = [i.defaultChecked, i.checked, i.hasAttribute('checked')]; i.checked = false; a.push(i.defaultChecked, i.hasAttribute('checked')); return a; });
  t('select.autocomplete', function () { var s = mk('select'); return [s.autocomplete, s.required]; });
  return JSON.stringify(R);
})()
