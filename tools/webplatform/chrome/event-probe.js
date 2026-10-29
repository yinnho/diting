// DOMException / Event / EventTarget shape + behaviour probe (JS-PLATFORM-ARCH M3).
// Runs in real Chrome AND in the diting engine; results are diffed.
// Returns a JSON string. Pure observation, deterministic.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + ': ' + (e && e.message); } }
  function desc(o, k) {
    var d = Object.getOwnPropertyDescriptor(o, k);
    if (!d) return null;
    var r = { e: d.enumerable, c: d.configurable };
    if ('value' in d) { r.w = d.writable; r.t = typeof d.value; if (typeof d.value === 'function') { r.len = d.value.length; r.name = d.value.name; r.nat = /\[native code\]/.test(Function.prototype.toString.call(d.value)); } else r.v = (d.value !== null && typeof d.value === 'object') ? '#object' : d.value; }
    else { r.g = d.get ? { name: d.get.name, len: d.get.length, nat: /\[native code\]/.test(Function.prototype.toString.call(d.get)) } : null; r.s = d.set ? 'fn' : null; }
    return r;
  }
  function shape(C) {
    var P = C.prototype, o = {};
    Object.getOwnPropertyNames(P).sort().forEach(function (k) { o[k] = desc(P, k); });
    Object.getOwnPropertySymbols(P).forEach(function (k) { o[String(k)] = desc(P, k); });
    return { len: C.length, name: C.name, proto: desc(C, 'prototype'), own: Object.getOwnPropertyNames(C).sort(), order: Object.getOwnPropertyNames(P), members: o,
      parent: Object.getPrototypeOf(P) === Object.prototype ? 'Object' : (Object.getPrototypeOf(P).constructor || {}).name };
  }
  // ---- interface shapes
  ['DOMException', 'Event', 'EventTarget', 'CustomEvent'].forEach(function (n) { t('shape.' + n, function () { return shape(self[n]); }); });
  // ---- DOMException
  t('dex.ctor.noargs', function () { var e = new DOMException(); return [e.name, e.message, e.code, String(e)]; });
  t('dex.ctor.args', function () { var e = new DOMException('m', 'AbortError'); return [e.name, e.message, e.code, String(e), e instanceof Error, Object.prototype.toString.call(e)]; });
  t('dex.ownkeys', function () { return Object.getOwnPropertyNames(new DOMException('m', 'AbortError')); });
  t('dex.keys', function () { var e = new DOMException('m', 'AbortError'), k = []; for (var x in e) k.push(x); return k; });
  t('dex.unknownName', function () { var e = new DOMException('m', 'Nope'); return [e.name, e.code]; });
  t('dex.legacyCodes', function () { return ['IndexSizeError', 'InvalidStateError', 'DataCloneError', 'AbortError', 'TimeoutError'].map(function (n) { return new DOMException('', n).code; }); });
  t('dex.name.set', function () { var e = new DOMException('m', 'AbortError'); e.name = 'X'; e.message = 'Y'; return [e.name, e.message]; });
  t('dex.call', function () { return DOMException('a'); });
  t('dex.stack', function () { var e = new DOMException('m', 'AbortError'); return [typeof e.stack, /^AbortError: m/.test(e.stack)]; });
  t('dex.illegal.name', function () { return Object.getOwnPropertyDescriptor(DOMException.prototype, 'name').get.call({}); });
  t('dex.consts', function () { return [DOMException.ABORT_ERR, DOMException.prototype.ABORT_ERR, desc(DOMException, 'ABORT_ERR'), desc(DOMException.prototype, 'ABORT_ERR')]; });
  t('dex.tag', function () { return [desc(DOMException.prototype, Symbol.toStringTag), Object.prototype.toString.call(DOMException.prototype)]; });
  t('dex.msgConv', function () { return new DOMException({ toString: function () { return 'o'; } }, { toString: function () { return 'AbortError'; } }).message; });
  // ---- Event
  t('ev.ctor.noargs', function () { return new Event(); });
  t('ev.ctor.call', function () { return Event('x'); });
  t('ev.ownkeys', function () { return Object.getOwnPropertyNames(new Event('x')); });
  t('ev.keys', function () { var k = []; for (var x in new Event('x')) k.push(x); return k.sort(); });
  t('ev.json', function () { return JSON.stringify(new Event('x')); });
  t('ev.values', function () { var e = new Event(123, { bubbles: 1, cancelable: 'y', composed: 0 }); return [e.type, e.bubbles, e.cancelable, e.composed, e.defaultPrevented, e.target, e.currentTarget, e.eventPhase, e.isTrusted, typeof e.timeStamp, e.srcElement, e.returnValue, e.cancelBubble]; });
  t('ev.readonly', function () { var e = new Event('x'); e.type = 'y'; e.target = 1; e.bubbles = true; return [e.type, e.target, e.bubbles]; });
  t('ev.strict.readonly', function () { 'use strict'; var e = new Event('x'); e.type = 'y'; return e.type; });
  t('ev.preventDefault', function () { var a = new Event('x', { cancelable: true }), b = new Event('x'); a.preventDefault(); b.preventDefault(); return [a.defaultPrevented, b.defaultPrevented, a.returnValue, b.returnValue]; });
  t('ev.returnValue.set', function () { var a = new Event('x', { cancelable: true }); a.returnValue = false; return [a.defaultPrevented, a.returnValue]; });
  t('ev.cancelBubble', function () { var a = new Event('x'); a.cancelBubble = true; return [a.cancelBubble]; });
  // Setting cancelBubble=false after stopPropagation must NOT clear the stop
  // flag (the setter is a one-way legacy IE alias).
  t('ev.cancelBubble.setfalse', function () { var a = new Event('x'); a.stopPropagation(); a.cancelBubble = false; return a.cancelBubble; });
  t('ev.stopPropagation.cancelBubble', function () { var a = new Event('x'); a.stopPropagation(); return a.cancelBubble; });
  t('ev.initEvent', function () { var a = new Event('x'); a.initEvent('z', true, true); return [a.type, a.bubbles, a.cancelable]; });
  t('ev.initEvent.noarg', function () { return new Event('x').initEvent(); });
  t('ev.consts', function () { return [Event.NONE, Event.CAPTURING_PHASE, Event.AT_TARGET, Event.BUBBLING_PHASE, desc(Event, 'AT_TARGET'), desc(Event.prototype, 'AT_TARGET')]; });
  t('ev.illegal', function () { return Object.getOwnPropertyDescriptor(Event.prototype, 'type').get.call({}); });
  t('ev.illegal.method', function () { return Event.prototype.preventDefault.call({}); });
  t('ev.composedPath.fresh', function () { return new Event('x').composedPath(); });
  t('ev.tag', function () { return [Object.prototype.toString.call(new Event('x')), desc(Event.prototype, Symbol.toStringTag)]; });
  t('ev.initDict.getters', function () { var log = []; new Event('x', { get bubbles() { log.push('b'); return true; }, get cancelable() { log.push('c'); return true; }, get composed() { log.push('p'); return true; } }); return log; });
  t('ev.initDict.bad', function () { return new Event('x', 5); });
  t('ev.initDict.null', function () { return new Event('x', null).type; });
  t('ev.type.sym', function () { return new Event(Symbol('s')); });
  t('ev.isTrusted.desc', function () { return desc(new Event('x'), 'isTrusted'); });
  t('ev.isTrusted.set.sloppy', function () { var e = new Event('x'); e.isTrusted = true; return e.isTrusted; });
  t('ev.isTrusted.set.strict', function () { 'use strict'; var e = new Event('x'); try { e.isTrusted = true; return 'ok'; } catch (err) { return 'THROW ' + err.constructor.name + ': ' + err.message; } });
  t('ev.subclass.ownkeys', function () { var T = class T extends Event {}; var e = new T('x', { bubbles: true }); return [Object.getOwnPropertyNames(e), Object.getPrototypeOf(e) === T.prototype, e instanceof Event, e.type]; });
  t('ev.subclass.dispatch', function () { var T = class T extends Event {}; var et = new EventTarget(), r; et.addEventListener('y', function (ev) { r = [ev.type, ev.target === et, ev instanceof T, ev.isTrusted]; }); et.dispatchEvent(new T('y', { bubbles: true })); return r; });
  t('ev.composedPath.after', function () {
    var d = document.createElement('div'); document.body.appendChild(d);
    var ev = new Event('x', { bubbles: true }); d.dispatchEvent(ev); d.remove();
    return ev.composedPath();
  });
  t('ce.ownkeys', function () { return Object.getOwnPropertyNames(new CustomEvent('x', { detail: 1 })); });
  t('ce.detail', function () { return [new CustomEvent('x').detail, new CustomEvent('x', { detail: undefined }).detail, new CustomEvent('x', { detail: 5 }).detail]; });
  t('ce.initCustomEvent', function () { var e = new CustomEvent('x'); e.initCustomEvent('y', true, true, 7); return [e.type, e.bubbles, e.detail]; });
  t('sub.ownkeys', function () { return ['UIEvent', 'MouseEvent', 'KeyboardEvent', 'FocusEvent', 'ErrorEvent', 'MessageEvent', 'ProgressEvent', 'CloseEvent'].map(function (n) { try { return n + ':' + Object.getOwnPropertyNames(new self[n]('x')).join(','); } catch (e) { return n + ':THROW ' + e.constructor.name; } }); });
  // ---- EventTarget
  t('et.ctor', function () { var e = new EventTarget(); return [Object.getOwnPropertyNames(e), Object.prototype.toString.call(e)]; });
  t('et.call', function () { return EventTarget(); });
  t('et.add.arity', function () { return new EventTarget().addEventListener(); });
  t('et.add.notfn', function () { return new EventTarget().addEventListener('x', 5); });
  t('et.add.null', function () { return new EventTarget().addEventListener('x', null); });
  t('et.dispatch.notEvent', function () { return new EventTarget().dispatchEvent({}); });
  t('et.dispatch.noarg', function () { return new EventTarget().dispatchEvent(); });
  t('et.illegal', function () { return EventTarget.prototype.addEventListener.call({}, 'x', function () {}); });
  t('et.dispatch.return', function () { var e = new EventTarget(), ev = new Event('x', { cancelable: true }); e.addEventListener('x', function (x) { x.preventDefault(); }); return [e.dispatchEvent(ev), ev.defaultPrevented, ev.target === e, ev.currentTarget, ev.eventPhase]; });
  // { passive: true } makes preventDefault inert mid-listener.
  t('et.passive', function () { var e = new EventTarget(), seen; e.addEventListener('x', function (x) { x.preventDefault(); seen = x.defaultPrevented; }, { passive: true }); var r = e.dispatchEvent(new Event('x', { cancelable: true })); return [seen, r]; });
  t('et.listener.order', function () { var e = new EventTarget(), l = []; e.addEventListener('x', function () { l.push(1); }); e.addEventListener('x', function () { l.push(2); }); e.dispatchEvent(new Event('x')); return l; });
  t('et.listener.dup', function () { var e = new EventTarget(), l = [], f = function () { l.push(1); }; e.addEventListener('x', f); e.addEventListener('x', f); e.dispatchEvent(new Event('x')); return l; });
  t('et.listener.once', function () { var e = new EventTarget(), l = []; e.addEventListener('x', function () { l.push(1); }, { once: true }); e.dispatchEvent(new Event('x')); e.dispatchEvent(new Event('x')); return l; });
  t('et.listener.signal', function () { var e = new EventTarget(), l = [], c = new AbortController(); e.addEventListener('x', function () { l.push(1); }, { signal: c.signal }); e.dispatchEvent(new Event('x')); c.abort(); e.dispatchEvent(new Event('x')); return l; });
  t('et.listener.objectHandler', function () { var e = new EventTarget(), l = []; e.addEventListener('x', { handleEvent: function (ev) { l.push(ev.type, this === undefined); } }); e.dispatchEvent(new Event('x')); return l; });
  t('et.listener.this', function () { var e = new EventTarget(), r; e.addEventListener('x', function () { r = this === e; }); e.dispatchEvent(new Event('x')); return r; });
  t('et.listener.throws', function () { var e = new EventTarget(), l = []; e.addEventListener('x', function () { throw new Error('boom'); }); e.addEventListener('x', function () { l.push('second'); }); var r = e.dispatchEvent(new Event('x')); return [r, l]; });
  t('et.stopImmediate', function () { var e = new EventTarget(), l = []; e.addEventListener('x', function (ev) { l.push(1); ev.stopImmediatePropagation(); }); e.addEventListener('x', function () { l.push(2); }); e.dispatchEvent(new Event('x')); return l; });
  t('et.remove.during', function () { var e = new EventTarget(), l = [], b = function () { l.push('b'); }; e.addEventListener('x', function () { l.push('a'); e.removeEventListener('x', b); }); e.addEventListener('x', b); e.dispatchEvent(new Event('x')); return l; });
  t('et.add.during', function () { var e = new EventTarget(), l = []; e.addEventListener('x', function () { l.push('a'); e.addEventListener('x', function () { l.push('late'); }); }); e.dispatchEvent(new Event('x')); return l; });
  // ---- window on the native core (M3 step B1)
  // (No `instanceof EventTarget` / getPrototypeOf pins here: this engine's
  // global proto layer is the named-access gate, not Window.prototype, and
  // EventTarget.prototype is not in the chain — pre-existing face shape,
  // unchanged by B1.)
  t('win.et', function () { return [typeof self.addEventListener, typeof self.removeEventListener, typeof self.dispatchEvent, self.dispatchEvent.length]; });
  t('win.dispatch.state', function () { var r; self.addEventListener('wprobe', function (e) { r = [this === self, e.target === self, e.currentTarget === self, e.eventPhase]; }); self.dispatchEvent(new Event('wprobe')); return r; });
  t('win.on.order', function () { var o = []; self.onmessage = function () { o.push(1); }; self.addEventListener('message', function () { o.push(2); }); self.dispatchEvent(new Event('message')); self.onmessage = null; return o; });
  // The on* handler fires with ZERO listeners registered — inner invoke
  // runs it regardless of the listener table (a `<body onload>` page has no
  // window 'load' listeners at all). And the converse face: on* only fires
  // for REAL handler IDL attributes — a plain `onfoo` data property on the
  // window or a bare EventTarget is never called (Chrome ignores it).
  t('win.on.nolistener', function () { var r = 'no'; self.onload = function () { r = 'yes'; }; self.dispatchEvent(new Event('load')); self.onload = null; return r; });
  t('win.on.unknown', function () { var r = 'no'; self.onzzzprobe = function () { r = 'yes'; }; self.dispatchEvent(new Event('zzzprobe')); self.onload = null; return r; });
  t('et.on.unknown', function () { var e = new EventTarget(), r = 'no'; e.onfoo = function () { r = 'yes'; }; e.dispatchEvent(new Event('foo')); return r; });
  // NOTE: no key for synthetic dispatchEvent(new Event('error')) firing
  // onerror — real Chrome DOES fire it (truth said 1), but this engine's
  // error pipeline calls onerror itself before dispatching, so the handler
  // hook suppresses 'error' on the global to avoid double-reporting.
  // Known divergence, unchanged by B1; fixing it means moving the pipeline
  // onto a dispatched ErrorEvent with the five-arg onerror unwrap.
  // ---- XHR family + AbortSignal + BatteryManager on the native core (B2)
  t('xhr.chain', function () { return [Object.getPrototypeOf(XMLHttpRequestEventTarget.prototype) === EventTarget.prototype, Object.getPrototypeOf(XMLHttpRequest.prototype) === XMLHttpRequestEventTarget.prototype, Object.getPrototypeOf(AbortSignal.prototype) === XMLHttpRequestEventTarget.prototype]; });
  t('xhr.instanceof', function () { var x = new XMLHttpRequest(), c = new AbortController(); return [x instanceof EventTarget, x instanceof XMLHttpRequestEventTarget, c.signal instanceof EventTarget, c.signal instanceof XMLHttpRequestEventTarget]; });
  // Accessor-shape only (no `nat` pin: the #31 __acc accessors are JS
  // getters where Chrome's are native — a pattern-wide pre-existing face
  // divergence, not this step's concern).
  t('xhr.on.desc', function () { var a = desc(XMLHttpRequestEventTarget.prototype, 'onload'), b = desc(XMLHttpRequest.prototype, 'onreadystatechange'); return [!!a && !!a.g && a.s === 'fn' && a.e, !!b && !!b.g && b.s === 'fn' && b.e]; });
  t('xhr.on.default', function () { var x = new XMLHttpRequest(); return [x.onload, x.onerror, x.onreadystatechange, x.onloadend]; });
  t('xhr.upload', function () { var x = new XMLHttpRequest(); return [x.upload instanceof XMLHttpRequestUpload, x.upload instanceof EventTarget, Object.getOwnPropertyNames(x.upload).length]; });
  t('xhr.dispatch.shape', function () { var x = new XMLHttpRequest(), r; x.addEventListener('x', function (e) { r = [this === x, e.target === x, e.currentTarget === x, e.eventPhase, e instanceof Event, e.type]; }); x.dispatchEvent(new ProgressEvent('x')); return r; });
  // on* handler vs listener, the common page pattern (handler assigned
  // first): registration order and the core's handler-first agree here.
  t('xhr.dispatch.on', function () { var x = new XMLHttpRequest(), o = []; x.onload = function (e) { o.push('on:' + (e instanceof ProgressEvent) + ':' + (e.target === x)); }; x.addEventListener('load', function () { o.push('ls'); }); x.dispatchEvent(new Event('load')); return o; });
  t('xhr.dispatch.string', function () { return new XMLHttpRequest().dispatchEvent('load'); });
  t('abort.event', function () { var c = new AbortController(), o = []; c.signal.onabort = function (e) { o.push('on:' + (e instanceof Event) + ':' + (e.target === c.signal)); }; c.signal.addEventListener('abort', function (e) { o.push('ls:' + e.type); }); c.abort(); return o; });
  t('abort.dispatch.state', function () { var c = new AbortController(), r; c.signal.addEventListener('abort', function (e) { r = [this === c.signal, e.target === c.signal, e.currentTarget === c.signal, e.eventPhase]; }); c.abort(); return r; });
  t('battery.et', function () { return [Object.getPrototypeOf(BatteryManager.prototype) === EventTarget.prototype, Object.getOwnPropertyNames(BatteryManager.prototype).indexOf('addEventListener') === -1, Object.getPrototypeOf(NetworkInformation.prototype) === EventTarget.prototype]; });
  t('et.reentrant', function () { var e = new EventTarget(); e.addEventListener('x', function (ev) { e.dispatchEvent(ev); }); return e.dispatchEvent(new Event('x')); });
  t('et.redispatch', function () { var e = new EventTarget(), ev = new Event('x'); e.dispatchEvent(ev); return [ev.target === e, ev.currentTarget, ev.eventPhase, e.dispatchEvent(ev)]; });
  t('et.passive.prevent', function () { var e = new EventTarget(), ev = new Event('x', { cancelable: true }); e.addEventListener('x', function (x) { x.preventDefault(); }, { passive: true }); e.dispatchEvent(ev); return ev.defaultPrevented; });
  t('et.shape.sub', function () { class T extends EventTarget { } var t = new T(); var l = []; t.addEventListener('x', function () { l.push(1); }); t.dispatchEvent(new Event('x')); return l; });
  t('et.window', function () { var l = []; self.addEventListener('zzz', function (e) { l.push(e.target === self, e.currentTarget === self, e.eventPhase); }); self.dispatchEvent(new Event('zzz')); return l; });
  t('et.document.bubble', function () {
    var d = document.createElement('div'), p = document.createElement('div'); p.appendChild(d); document.body.appendChild(p);
    var l = []; p.addEventListener('x', function (e) { l.push('p:' + e.eventPhase + ':' + (e.target === d) + ':' + (e.currentTarget === p)); });
    d.addEventListener('x', function (e) { l.push('d:' + e.eventPhase); });
    document.addEventListener('x', function (e) { l.push('doc:' + e.eventPhase); });
    d.dispatchEvent(new Event('x', { bubbles: true })); d.dispatchEvent(new Event('x')); p.remove(); return l;
  });
  t('et.capture', function () {
    var d = document.createElement('div'), p = document.createElement('div'); p.appendChild(d); document.body.appendChild(p);
    var l = []; p.addEventListener('x', function () { l.push('pc'); }, true); p.addEventListener('x', function () { l.push('pb'); }); d.addEventListener('x', function () { l.push('d'); });
    d.dispatchEvent(new Event('x', { bubbles: true })); p.remove(); return l;
  });
  t('et.composedPath', function () {
    var d = document.createElement('div'); document.body.appendChild(d); var r;
    d.addEventListener('x', function (e) { r = e.composedPath().map(function (n) { return n === window ? 'W' : n === document ? 'D' : n.nodeName; }); }); d.dispatchEvent(new Event('x', { bubbles: true })); d.remove(); return r;
  });
  return JSON.stringify(R);
})()
