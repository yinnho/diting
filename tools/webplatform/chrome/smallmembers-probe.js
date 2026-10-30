// Small-member sweep probe (M4 knife 14): CharacterData/DocumentFragment
// mixin members, the form/hash/popstate/clipboard/message events, DataTransfer
// data access, RTCSessionDescription, InputDeviceCapabilities. Runs in Chrome AND the engine; diffed.
(function () {
  var R = {};
  function norm(s) { return /^https?:/.test(location.origin) ? s.split(location.href).join('#B').split(location.origin).join('#O') : s; }
  function t(n, f) { try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(norm(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; }))); } catch (e) { R[n] = norm('THROW ' + e.name + ': ' + String(e.message).slice(0, 220)); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return d ? (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable : 'ABSENT'; }
  function own(C) { return typeof C === 'function' ? { order: Object.getOwnPropertyNames(C.prototype), parent: Object.getPrototypeOf(C).name, len: C.length } : 'MISSING'; }
  var SHAPES = ['Text', 'Comment', 'ProcessingInstruction', 'CharacterData', 'DocumentFragment', 'HashChangeEvent', 'SubmitEvent', 'PopStateEvent', 'MessageEvent', 'ClipboardEvent', 'DataTransfer', 'RTCSessionDescription', 'InputDeviceCapabilities'];
  R.shape = {}; SHAPES.forEach(function (n) { R.shape[n] = own(window[n]); });
  var d = document, body = d.body || d.documentElement;

  // ---- NonDocumentTypeChildNode on CharacterData
  t('sib.proto', function () { return ['Text', 'Comment', 'ProcessingInstruction', 'CharacterData'].map(function (n) { return [n, kind(window[n].prototype, 'nextElementSibling'), kind(window[n].prototype, 'previousElementSibling')]; }); });
  t('sib.vals', function () {
    var div = d.createElement('div'); div.innerHTML = '<a></a>t1<!--c--><b></b>t2<?x y?><i></i>';
    var n = div.childNodes, o = [];
    for (var i = 0; i < n.length; i++) { var x = n[i]; if (x.nodeType === 1) continue; o.push([x.nodeName, x.nextElementSibling && x.nextElementSibling.localName, x.previousElementSibling && x.previousElementSibling.localName]); }
    return o;
  });
  t('sib.orphan', function () { var x = d.createTextNode('z'); return [x.nextElementSibling, x.previousElementSibling]; });
  t('sib.attr', function () { var a = d.createElement('a'); return [typeof a.nextElementSibling, kind(Element.prototype, 'nextElementSibling')]; });
  t('text.assignedSlot', function () { var x = d.createTextNode('a'); return [x.assignedSlot, kind(Text.prototype, 'assignedSlot')]; });
  t('pi.sheet', function () { var p = d.createProcessingInstruction('xml-stylesheet', 'href="a.css"'); return [p.sheet, kind(ProcessingInstruction.prototype, 'sheet')]; });
  t('frag.count', function () { var f = d.createDocumentFragment(); f.appendChild(d.createElement('a')); f.appendChild(d.createTextNode('t')); f.appendChild(d.createElement('b')); return [f.childElementCount, f.children.length, kind(DocumentFragment.prototype, 'childElementCount')]; });
  t('frag.count.empty', function () { return d.createDocumentFragment().childElementCount; });

  // ---- events
  t('hashchange', function () { var e = new HashChangeEvent('hashchange', { oldURL: 'http://a/#1', newURL: 'http://a/#2' }); var f = new HashChangeEvent('x'); return [e.oldURL, e.newURL, f.oldURL, f.newURL, e.type, e.bubbles, Object.getOwnPropertyNames(HashChangeEvent.prototype), Object.getPrototypeOf(HashChangeEvent) === Event]; });
  t('hashchange.conv', function () { var e = new HashChangeEvent('x', { oldURL: 5, newURL: null }); return [e.oldURL, e.newURL]; });
  t('hashchange.noargs', function () { return new HashChangeEvent(); });
  t('hashchange.call', function () { return HashChangeEvent('x'); });
  t('submit.event', function () { var b = d.createElement('button'); var e = new SubmitEvent('submit', { submitter: b }); var f = new SubmitEvent('submit'); return [e.submitter === b, f.submitter, e.type, Object.getOwnPropertyNames(SubmitEvent.prototype), Object.getPrototypeOf(SubmitEvent) === Event, SubmitEvent.length]; });
  t('submit.bad', function () { return new SubmitEvent('submit', { submitter: {} }); });
  t('submit.bad2', function () { return new SubmitEvent('submit', { submitter: 5 }); });
  t('submit.null', function () { return new SubmitEvent('submit', { submitter: null }).submitter; });
  t('submit.svg', function () { var s = d.createElementNS('http://www.w3.org/2000/svg', 'svg'); return new SubmitEvent('submit', { submitter: s }).submitter === s; });
  t('submit.noargs', function () { return new SubmitEvent(); });
  t('submit.real', function () {
    var f = d.createElement('form'), b = d.createElement('button'); b.type = 'submit'; f.appendChild(b); body.appendChild(f);
    var got = []; f.addEventListener('submit', function (e) { got.push([e.constructor.name, e.submitter === b, e.isTrusted]); e.preventDefault(); });
    f.requestSubmit(b); f.requestSubmit(); b.click(); f.remove(); return got;
  });
  t('popstate', function () { var e = new PopStateEvent('popstate', { state: 5 }); return [e.state, e.hasUAVisualTransition, Object.getOwnPropertyNames(PopStateEvent.prototype), new PopStateEvent('x', { hasUAVisualTransition: true }).hasUAVisualTransition]; });
  t('message.init', function () {
    var e = new MessageEvent('message'); var r = [typeof e.initMessageEvent, e.initMessageEvent && e.initMessageEvent.length];
    e.initMessageEvent('foo', true, true, 'data', 'http://o', 'lid', window, []);
    r.push(e.type, e.bubbles, e.cancelable, e.data, e.origin, e.lastEventId, e.source === window, e.ports.length);
    var e2 = new MessageEvent('message'); e2.initMessageEvent('m2'); r.push(e2.type, e2.data, e2.origin, e2.source);
    return r;
  });
  t('message.init.noargs', function () { return new MessageEvent('x').initMessageEvent(); });
  t('message.order', function () { return Object.getOwnPropertyNames(MessageEvent.prototype); });
  t('clipboard.event', function () { var e = new ClipboardEvent('paste'); var f = new ClipboardEvent('copy', { clipboardData: new DataTransfer() }); return [e.clipboardData, f.clipboardData && f.clipboardData.constructor.name, Object.getOwnPropertyNames(ClipboardEvent.prototype), ClipboardEvent.length]; });
  t('clipboard.bad', function () { return new ClipboardEvent('copy', { clipboardData: {} }); });
  t('clipboard.noargs', function () { return new ClipboardEvent(); });

  // ---- DataTransfer
  t('dt.shape', function () { var dt = new DataTransfer(); return [Object.getOwnPropertyNames(DataTransfer.prototype), dt.dropEffect, dt.effectAllowed, dt.types, dt.files && dt.files.length, dt.items && dt.items.length, DataTransfer.length]; });
  t('dt.set', function () { var dt = new DataTransfer(); dt.setData('text/plain', 'hello'); dt.setData('text/html', '<b>x</b>'); return [dt.getData('text/plain'), dt.getData('text/html'), dt.getData('text/zzz'), dt.types, dt.items.length]; });
  t('dt.norm', function () { var dt = new DataTransfer(); dt.setData('Text', 'a'); dt.setData('URL', 'http://x'); return [dt.getData('text/plain'), dt.getData('text'), dt.getData('url'), dt.getData('text/uri-list'), dt.types]; });
  t('dt.case', function () { var dt = new DataTransfer(); dt.setData('TEXT/PLAIN', 'v'); return [dt.types, dt.getData('text/plain'), dt.getData('Text/Plain')]; });
  t('dt.clear', function () { var dt = new DataTransfer(); dt.setData('text/plain', 'a'); dt.setData('text/html', 'b'); dt.clearData('text/plain'); var r = [dt.types]; dt.clearData(); r.push(dt.types); return r; });
  t('dt.clear.fmt', function () { var dt = new DataTransfer(); dt.setData('text/plain', 'a'); dt.clearData('Text'); return dt.types; });
  t('dt.replace', function () { var dt = new DataTransfer(); dt.setData('text/plain', 'a'); dt.setData('text/plain', 'b'); return [dt.getData('text/plain'), dt.types.length]; });
  t('dt.noargs', function () { var dt = new DataTransfer(), r = []; try { dt.setData('x'); } catch (e) { r.push(e.message); } try { dt.getData(); } catch (e) { r.push(e.message); } return [r, dt.clearData.length, dt.getData.length, dt.setData.length]; });
  t('dt.types.frozen', function () { var dt = new DataTransfer(); dt.setData('text/plain', 'a'); return [Object.isFrozen(dt.types), dt.types === dt.types, Array.isArray(dt.types)]; });

  // ---- Headers.getSetCookie

  // ---- media / rtc / canvas / misc
  t('rtcsd', function () { var s = new RTCSessionDescription({ type: 'offer', sdp: 'v=0' }); var j = s.toJSON(); return [j, Object.keys(j), Object.getOwnPropertyNames(RTCSessionDescription.prototype), RTCSessionDescription.prototype.toJSON.length, JSON.stringify(s)]; });
  t('rtcsd.empty', function () { return new RTCSessionDescription({ type: 'answer' }).toJSON(); });
  t('rtcsd.brand', function () { return RTCSessionDescription.prototype.toJSON.call({}); });
  t('rtcsd.noargs', function () { var s = new RTCSessionDescription(); return [s.type, s.sdp]; });
  t('rtcsd.emptyinit', function () { var s = new RTCSessionDescription({}); return [s.type, s.sdp]; });
  t('rtcsd.badtype', function () { return new RTCSessionDescription({ type: 'zzz' }); });
  t('rtcsd.sdponly', function () { var s = new RTCSessionDescription({ sdp: 'x' }); return [s.type, s.sdp]; });
  t('rtcsd.types', function () { var o = {}; ['offer', 'answer', 'pranswer', 'rollback', 'OFFER'].forEach(function (v) { try { o[v] = new RTCSessionDescription({ type: v }).type; } catch (e) { o[v] = 'T'; } }); return o; });
  t('rtcsd.conv', function () { var s = new RTCSessionDescription({ type: 'offer', sdp: 5 }); return [s.sdp, typeof s.sdp]; });
  t('rtcsd.badinit', function () { return new RTCSessionDescription(5); });
  t('rtcsd.call', function () { return RTCSessionDescription({ type: 'offer' }); });
  t('rtcsd.ro', function () { var s = new RTCSessionDescription({ type: 'offer', sdp: 'a' }); s.type = 'answer'; s.sdp = 'b'; return [s.type, s.sdp]; });
  t('rtcsd.own', function () { return Object.getOwnPropertyNames(new RTCSessionDescription({ type: 'offer' })); });
  t('idc', function () { var c = new InputDeviceCapabilities({ firesTouchEvents: true }); return [c.firesTouchEvents, new InputDeviceCapabilities().firesTouchEvents, new InputDeviceCapabilities({ firesTouchEvents: false }).firesTouchEvents, Object.getOwnPropertyNames(InputDeviceCapabilities.prototype), InputDeviceCapabilities.length]; });
  t('idc.conv', function () { return [new InputDeviceCapabilities({ firesTouchEvents: 1 }).firesTouchEvents, new InputDeviceCapabilities({ firesTouchEvents: null }).firesTouchEvents]; });
  return JSON.stringify(R);
})()
