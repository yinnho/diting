// IntersectionObserverEntry probe (M4 knife 19): shape, constructor/init conversions,
// instance identity of observer-delivered entries. Geometry VALUES are layout-dependent
// and deliberately not compared; only types, nullability and structure are.
(async function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : (typeof x === 'number' && !isFinite(x) ? '#' + x : x); })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 240); } }
  async function ta(n, f) { try { R[n] = conv(await f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 240); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) {
    var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), len: C.length, statics: Object.getOwnPropertyNames(C).filter(function (k) { return k !== 'length' && k !== 'name' && k !== 'prototype'; }), kinds: {}, proto: Object.getPrototypeOf(P) === Object.prototype ? 'Object' : '?', tag: Object.prototype.toString.call(Object.create(P)) };
    s.order.forEach(function (m) { s.kinds[m] = kind(P, m); });
    return s;
  }
  R['shape.IntersectionObserverEntry'] = shape(IntersectionObserverEntry);
  R['shape.IntersectionObserver'] = shape(IntersectionObserver);
  var el = document.createElement('div'); document.body.appendChild(el);
  var rect = new DOMRectReadOnly(1, 2, 3, 4);
  var INIT = { time: 5, target: el, boundingClientRect: rect, intersectionRect: rect, rootBounds: rect };
  t('call', function () { try { IntersectionObserverEntry(INIT); } catch (e) { return e.name + ': ' + e.message; } });
  t('noargs', function () { try { new IntersectionObserverEntry(); } catch (e) { return e.name + ': ' + e.message; } });
  t('ctor', function () { var e = new IntersectionObserverEntry(INIT); return [e.time, e.target === el, e.isIntersecting, e.intersectionRatio, e.isVisible, e.boundingClientRect instanceof DOMRectReadOnly, e.rootBounds instanceof DOMRectReadOnly, Object.keys(e), Object.getOwnPropertyNames(e), Object.prototype.toString.call(e)]; });
  t('ctor.rects', function () { var e = new IntersectionObserverEntry(INIT); return [e.boundingClientRect.x, e.boundingClientRect.width, e.boundingClientRect === e.boundingClientRect, e.boundingClientRect === rect, e.intersectionRect.height, e.rootBounds.y]; });
  t('ctor.flags', function () { var e = new IntersectionObserverEntry({ time: 1, target: el, boundingClientRect: rect, intersectionRect: rect, rootBounds: null, isIntersecting: true, intersectionRatio: 0.5, isVisible: true }); return [e.isIntersecting, e.intersectionRatio, e.isVisible, e.rootBounds]; });
  t('ctor.nullroot', function () { return new IntersectionObserverEntry({ time: 1, target: el, boundingClientRect: rect, intersectionRect: rect }).rootBounds; });
  t('ctor.bad', function () { var o = []; [5, 'x', null, {}, { time: 1 }, { time: 1, target: {} , boundingClientRect: rect, intersectionRect: rect }, { time: 1, target: el, boundingClientRect: {}, intersectionRect: rect }, { time: 'x', target: el, boundingClientRect: rect, intersectionRect: rect }, { time: 1, target: el, boundingClientRect: rect, intersectionRect: rect, intersectionRatio: 'z' }].forEach(function (v) { try { new IntersectionObserverEntry(v); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('ctor.rect.plain', function () { try { new IntersectionObserverEntry({ time: 1, target: el, boundingClientRect: { x: 1 }, intersectionRect: rect }); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } });
  t('toJSON', function () { var e = new IntersectionObserverEntry(INIT); var j = typeof e.toJSON === 'function' ? e.toJSON() : 'none'; return j === 'none' ? j : Object.keys(j); });
  t('brand', function () { var o = []; ['time', 'target', 'isIntersecting', 'rootBounds', 'boundingClientRect', 'isVisible'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(IntersectionObserverEntry.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  // ---- IntersectionObserver face
  t('io.call', function () { try { IntersectionObserver(function () {}); } catch (e) { return e.name + ': ' + e.message; } });
  t('io.noargs', function () { try { new IntersectionObserver(); } catch (e) { return e.name + ': ' + e.message; } });
  t('io.badcb', function () { var o = []; [5, {}, null, 'x'].forEach(function (v) { try { new IntersectionObserver(v); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('io.defaults', function () { var io = new IntersectionObserver(function () {}); return [io.root, io.rootMargin, io.scrollMargin, io.thresholds, io.delay, io.trackVisibility, Object.keys(io), Object.getOwnPropertyNames(io), Object.prototype.toString.call(io)]; });
  t('io.opts', function () { var io = new IntersectionObserver(function () {}, { rootMargin: '10px', threshold: [0.5, 0.1, 1] }); return [io.rootMargin, io.thresholds]; });
  t('io.opts.margin', function () { return ['5px 10px', '1px 2px 3px', '-5% 3px', '0', '1px 2px 3px 4px'].map(function (m) { try { return new IntersectionObserver(function () {}, { rootMargin: m }).rootMargin; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('io.opts.thresh', function () { return [0.3, [], 1, 0].map(function (th) { try { return new IntersectionObserver(function () {}, { threshold: th }).thresholds; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('io.opts.thresh.bad', function () { return [-1, 2, 'x', [0.5, 3], NaN].map(function (th) { try { new IntersectionObserver(function () {}, { threshold: th }); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('io.opts.root.bad', function () { try { new IntersectionObserver(function () {}, { root: {} }); } catch (e) { return e.name + ': ' + e.message; } });
  t('io.opts.root', function () { var io = new IntersectionObserver(function () {}, { root: el }); return io.root === el; });
  t('io.opts.type', function () { return [5, 'x'].map(function (o) { try { new IntersectionObserver(function () {}, o); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('io.thresholds.frozen', function () { var io = new IntersectionObserver(function () {}); return [Object.isFrozen(io.thresholds), io.thresholds === io.thresholds]; });
  t('io.observe.bad', function () { var io = new IntersectionObserver(function () {}); var o = []; [function () { io.observe(); }, function () { io.observe({}); }, function () { io.observe(null); }, function () { io.unobserve({}); }].forEach(function (f) { try { f(); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('io.returns', function () { var io = new IntersectionObserver(function () {}); var d = document.createElement('div'); return [io.observe(d), io.unobserve(d), io.disconnect(), Array.isArray(io.takeRecords())]; });
  t('io.brand', function () { var o = []; ['observe', 'unobserve', 'disconnect', 'takeRecords'].forEach(function (m) { try { IntersectionObserver.prototype[m].call({}, el); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['root', 'rootMargin', 'thresholds'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(IntersectionObserver.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  await ta('io.callback.args', async function () { var d = document.createElement('div'); document.body.appendChild(d); return await new Promise(function (res) { var io = new IntersectionObserver(function (es, obs) { res([Array.isArray(es), es.length, obs === io, this === io, arguments.length]); io.disconnect(); }); io.observe(d); }); });
  await ta('io.callback.once', async function () { var d = document.createElement('div'); document.body.appendChild(d); var n = 0; var io = new IntersectionObserver(function (es) { n += es.length; }); io.observe(d); io.observe(d); await new Promise(function (r) { setTimeout(r, 100); }); io.disconnect(); return n; });
  await ta('observed', async function () {
    var d = document.createElement('div'); d.style.cssText = 'width:50px;height:20px'; document.body.appendChild(d);
    var rec = await new Promise(function (res) { var io = new IntersectionObserver(function (es) { res(es); io.disconnect(); }); io.observe(d); });
    var e = rec[0];
    return [Array.isArray(rec), e instanceof IntersectionObserverEntry, e.target === d, typeof e.time, typeof e.isIntersecting, typeof e.intersectionRatio, e.boundingClientRect instanceof DOMRectReadOnly, e.intersectionRect instanceof DOMRectReadOnly, e.rootBounds === null ? 'null' : (e.rootBounds instanceof DOMRectReadOnly), typeof e.isVisible, Object.keys(e), Object.keys(e.toJSON ? e.toJSON() : {})];
  });
  await ta('observed.time', async function () { var d = document.createElement('div'); document.body.appendChild(d); var t0 = performance.now(); var rec = await new Promise(function (res) { var io = new IntersectionObserver(function (es) { res(es); io.disconnect(); }); io.observe(d); }); return [rec[0].time >= 0, rec[0].time <= performance.now() + 1]; });
  return JSON.stringify(R);
})()
