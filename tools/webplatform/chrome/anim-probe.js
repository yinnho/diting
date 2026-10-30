// Animation / AnimationTimeline / DocumentTimeline / AnimationPlaybackEvent probe (M4 knife 17).
// Deterministic by construction: long durations, no wall-clock values in results.
(async function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : (typeof x === 'number' && !isFinite(x) ? '#' + x : x); })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 200); } }
  async function ta(n, f) { try { R[n] = conv(await f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 200); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) {
    var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), len: C.length, statics: Object.getOwnPropertyNames(C).filter(function (k) { return k !== 'length' && k !== 'name' && k !== 'prototype'; }), kinds: {}, proto: Object.getPrototypeOf(P) === Object.prototype ? 'Object' : Object.getPrototypeOf(P).constructor.name, tag: Object.prototype.toString.call(Object.create(P)) };
    s.order.forEach(function (m) { s.kinds[m] = kind(P, m); });
    return s;
  }
  ['Animation', 'AnimationTimeline', 'DocumentTimeline', 'AnimationPlaybackEvent', 'KeyframeEffect', 'AnimationEffect'].forEach(function (n) { R['shape.' + n] = typeof globalThis[n] === 'function' ? shape(globalThis[n]) : 'absent'; });
  var el = document.createElement('div'); document.body.appendChild(el);
  var KF = [{ opacity: 0 }, { opacity: 1 }];
  function mk(o) { return el.animate(KF, o === undefined ? 100000 : o); }
  t('timeline.same', function () { return [document.timeline === document.timeline, document.timeline instanceof DocumentTimeline, document.timeline instanceof AnimationTimeline, Object.getOwnPropertyNames(document.timeline)]; });
  t('timeline.vals', function () { var tl = document.timeline; return [typeof tl.currentTime, tl.currentTime >= 0, tl.duration]; });
  t('timeline.ctor', function () { var a = new DocumentTimeline(); var b = new DocumentTimeline({ originTime: 100 }); return [typeof a.currentTime, a.currentTime >= 0, b.currentTime <= a.currentTime]; });
  t('timeline.ctor.bad', function () { try { new DocumentTimeline({ originTime: 'x' }); } catch (e) { return e.name + ': ' + e.message; } });
  t('timeline.illegal', function () { try { new AnimationTimeline(); } catch (e) { return e.name + ': ' + e.message; } });
  t('timeline.call', function () { try { DocumentTimeline(); } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.call', function () { try { Animation(); } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.inst', function () { var a = mk(); return [a instanceof Animation, a instanceof EventTarget, Object.prototype.toString.call(a), Object.getOwnPropertyNames(a), Object.keys(a)]; });
  t('anim.fresh', function () { var a = mk(); return { id: a.id, playState: a.playState, pending: a.pending, rate: a.playbackRate, replace: a.replaceState, tl: a.timeline === document.timeline, eff: a.effect instanceof KeyframeEffect, effTarget: a.effect.target === el, ct: typeof a.currentTime, st: a.startTime === null ? 'null' : typeof a.startTime, onfinish: a.onfinish, oncancel: a.oncancel, onremove: a.onremove, ready: a.ready instanceof Promise, finished: a.finished instanceof Promise, rsame: a.ready === a.ready, fsame: a.finished === a.finished }; });
  t('anim.ctor.noargs', function () { var a = new Animation(); return [a.effect, a.timeline === document.timeline, a.playState, a.currentTime, a.startTime, a.pending, a.id]; });
  t('anim.ctor.effect', function () { var e = new KeyframeEffect(el, KF, 1000); var a = new Animation(e); return [a.effect === e, a.timeline === document.timeline, a.playState, a.pending, a.currentTime, a.startTime]; });
  t('anim.ctor.timeline', function () { var e = new KeyframeEffect(el, KF, 1000); var a = new Animation(e, null); return [a.timeline, a.playState, a.currentTime]; });
  t('anim.ctor.bad', function () { var o = []; [5, {}, 'x'].forEach(function (v) { try { new Animation(v); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); try { new Animation(null, {}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('anim.play.pending', function () { var a = mk(); var s0 = [a.playState, a.pending]; a.pause(); return [s0, a.playState, a.pending, a.currentTime]; });
  t('anim.cancel.sync', function () { var a = mk(); a.cancel(); return [a.playState, a.pending, a.currentTime, a.startTime, a.effect === null ? 'null' : 'eff']; });
  t('anim.pause.sync', function () { var a = mk(); a.pause(); return [a.playState, a.pending]; });
  t('anim.finish.sync', function () { var a = mk(1000); a.finish(); return [a.playState, a.pending, a.currentTime, a.startTime !== null]; });
  t('anim.finish.infinite', function () { var a = el.animate(KF, { duration: 1000, iterations: Infinity }); try { a.finish(); } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.finish.reverse0', function () { var a = mk(1000); a.playbackRate = -1; a.currentTime = 0; try { a.finish(); return [a.playState, a.currentTime]; } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.currentTime.set', function () { var a = mk(); a.currentTime = 500; return [a.currentTime, a.playState, a.pending]; });
  t('anim.currentTime.null', function () { var a = mk(); try { a.currentTime = null; return [a.currentTime, a.playState]; } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.currentTime.bad', function () { var a = mk(); var o = []; ['x', NaN, Infinity, {}].forEach(function (v) { try { a.currentTime = v; o.push(String(a.currentTime)); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('anim.startTime.set', function () { var a = mk(); a.startTime = document.timeline.currentTime - 250; return [a.startTime === null, a.currentTime >= 250, a.playState, a.pending]; });
  t('anim.startTime.null', function () { var a = mk(); a.startTime = null; return [a.startTime, a.playState, a.currentTime === null ? 'null' : 'num']; });
  t('anim.rate.set', function () { var a = mk(); a.playbackRate = 2; return [a.playbackRate, a.playState, a.pending]; });
  t('anim.rate.bad', function () { var a = mk(); var o = []; ['x', NaN, Infinity, 0, -1.5, null].forEach(function (v) { try { a.playbackRate = v; o.push(String(a.playbackRate)); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('anim.updateRate', function () { var a = mk(); a.updatePlaybackRate(3); return [a.playbackRate, a.pending, a.playState]; });
  t('anim.updateRate.bad', function () { var a = mk(); try { a.updatePlaybackRate(); } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.reverse', function () { var a = mk(); a.reverse(); return [a.playbackRate, a.playState, a.pending]; });
  t('anim.reverse.finished', function () { var a = mk(1000); a.finish(); a.reverse(); return [a.playbackRate, a.playState, a.pending, a.currentTime]; });
  t('anim.play.finished', function () { var a = mk(1000); a.finish(); a.play(); return [a.playState, a.pending, a.currentTime]; });
  t('anim.play.canceled', function () { var a = mk(1000); a.cancel(); a.play(); return [a.playState, a.pending]; });
  t('anim.id', function () { var a = mk(); a.id = 5; var b = mk({ duration: 1000, id: 'opt' }); return [a.id, typeof a.id, b.id]; });
  t('anim.effect.set', function () { var a = mk(); var e = new KeyframeEffect(el, KF, 10); a.effect = e; var o = [a.effect === e]; a.effect = null; o.push(a.effect, a.playState); return o; });
  t('anim.effect.bad', function () { var a = mk(); try { a.effect = {}; } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.timeline.set', function () { var a = mk(); a.timeline = null; return [a.timeline, a.playState, a.currentTime]; });
  t('anim.timeline.bad', function () { var a = mk(); try { a.timeline = {}; } catch (e) { return e.name + ': ' + e.message; } });
  t('anim.persist', function () { var a = mk(); a.persist(); return [a.replaceState]; });
  t('anim.commitStyles', function () { var d = document.createElement('div'); document.body.appendChild(d); var a = d.animate([{ opacity: 0.25 }, { opacity: 0.25 }], 1000); a.finish(); a.commitStyles(); return d.style.opacity; });
  t('anim.brand', function () { var o = []; ['play', 'pause', 'cancel', 'finish', 'reverse', 'persist', 'commitStyles'].forEach(function (m) { try { Animation.prototype[m].call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['playState', 'pending', 'currentTime', 'effect'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(Animation.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('anim.getAnimations', function () { var d = document.createElement('div'); document.body.appendChild(d); var a = d.animate(KF, 100000); var b = d.animate(KF, 100000); var l = d.getAnimations(); var r = [l.length, l[0] === a, l[1] === b]; a.cancel(); r.push(d.getAnimations().length); b.finish(); r.push(d.getAnimations().length); return r; });
  t('doc.getAnimations', function () { var d = document.createElement('div'); document.body.appendChild(d); var a = d.animate(KF, 100000); return [document.getAnimations().indexOf(a) >= 0, a instanceof Animation]; });
  // ---- KeyframeEffect / AnimationEffect
  t('kf.call', function () { try { KeyframeEffect(el, KF); } catch (e) { return e.name + ': ' + e.message; } });
  t('kf.noargs', function () { try { new KeyframeEffect(); } catch (e) { return e.name + ': ' + e.message; } });
  t('kf.illegal', function () { try { new AnimationEffect(); } catch (e) { return e.name + ': ' + e.message; } });
  t('kf.inst', function () { var e = new KeyframeEffect(el, KF); return [e instanceof AnimationEffect, e.target === el, e.pseudoElement, e.composite, Object.prototype.toString.call(e), Object.getOwnPropertyNames(e)]; });
  t('kf.target.null', function () { var e = new KeyframeEffect(null, KF); return [e.target, e.getKeyframes().length]; });
  t('kf.target.bad', function () { try { new KeyframeEffect({}, KF); } catch (e) { return e.name + ': ' + e.message; } });
  t('kf.timing.default', function () { return new KeyframeEffect(el, KF).getTiming(); });
  t('kf.timing.num', function () { return new KeyframeEffect(el, KF, 250).getTiming(); });
  t('kf.timing.obj', function () { return new KeyframeEffect(el, KF, { duration: 100, delay: 5, endDelay: 2, iterations: 3, iterationStart: 0.5, direction: 'alternate', easing: 'ease-in', fill: 'both' }).getTiming(); });
  t('kf.timing.auto', function () { return new KeyframeEffect(el, KF, { duration: 'auto' }).getTiming().duration; });
  t('kf.timing.inf', function () { return new KeyframeEffect(el, KF, { iterations: Infinity }).getTiming().iterations; });
  t('kf.timing.bad', function () { var o = []; [{ duration: -1 }, { iterations: -1 }, { direction: 'zz' }, { fill: 'zz' }, { duration: 'x' }, { iterationStart: -1 }, { easing: 'zz' }, -5].forEach(function (v) { try { new KeyframeEffect(el, KF, v); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('kf.computed', function () { var c = new KeyframeEffect(el, KF, { duration: 100, delay: 10, iterations: 2, endDelay: 5 }).getComputedTiming(); return Object.keys(c).map(function (k) { return [k, c[k]]; }); });
  t('kf.computed.attached', function () { var a = el.animate(KF, 100000); var c = a.effect.getComputedTiming(); return [c.activeDuration, c.endTime, c.localTime === null ? 'null' : typeof c.localTime, c.progress === null ? 'null' : typeof c.progress, c.currentIteration === null ? 'null' : typeof c.currentIteration]; });
  t('kf.updateTiming', function () { var e = new KeyframeEffect(el, KF, 100); e.updateTiming({ delay: 7, fill: 'forwards' }); var g = e.getTiming(); return [g.delay, g.fill, g.duration]; });
  t('kf.updateTiming.bad', function () { var e = new KeyframeEffect(el, KF, 100); try { e.updateTiming({ duration: -3 }); } catch (x) { return x.name + ': ' + x.message; } });
  t('kf.keyframes.arr', function () { return new KeyframeEffect(el, [{ opacity: 0, color: 'red' }, { opacity: 0.5, offset: 0.25, easing: 'ease-in' }, { opacity: 1 }]).getKeyframes(); });
  t('kf.keyframes.obj', function () { return new KeyframeEffect(el, { opacity: [0, 1], width: ['10px', '20px', '30px'] }).getKeyframes(); });
  t('kf.keyframes.camel', function () { return new KeyframeEffect(el, [{ backgroundColor: 'red', cssFloat: 'left', marginTop: '1px' }]).getKeyframes(); });
  t('kf.keyframes.composite', function () { return new KeyframeEffect(el, [{ opacity: 0, composite: 'add' }, { opacity: 1 }], { composite: 'replace' }).getKeyframes().map(function (k) { return k.composite; }); });
  t('kf.keyframes.null', function () { return [new KeyframeEffect(el, null).getKeyframes(), new KeyframeEffect(el, []).getKeyframes()]; });
  t('kf.keyframes.bad', function () { var o = []; [5, 'x', [{ offset: 2 }], [{ offset: 0.5 }, { offset: 0.2 }], [{ easing: 'zz' }], [{ composite: 'zz' }]].forEach(function (v) { try { new KeyframeEffect(el, v); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('kf.setKeyframes', function () { var e = new KeyframeEffect(el, KF); e.setKeyframes([{ width: '1px' }]); return e.getKeyframes(); });
  t('kf.composite.set', function () { var e = new KeyframeEffect(el, KF); e.composite = 'add'; var o = [e.composite]; try { e.composite = 'zz'; } catch (x) { } o.push(e.composite); return o; });
  t('kf.pseudo', function () { var e = new KeyframeEffect(el, KF); e.pseudoElement = '::before'; var o = [e.pseudoElement]; try { e.pseudoElement = 'zz'; o.push('ok'); } catch (x) { o.push(x.name + ': ' + x.message); } return o; });
  t('kf.copy', function () { var e = new KeyframeEffect(el, KF, 123); var c = new KeyframeEffect(e); return [c.target === el, c.getTiming().duration, c.getKeyframes().length]; });
  t('kf.target.set', function () { var e = new KeyframeEffect(el, KF); var d = document.createElement('div'); e.target = d; return [e.target === d]; });
  t('anim.options.id', function () { var a = el.animate(KF, { id: 'x', duration: 5 }); return [a.id, a.effect.getTiming().duration, Object.keys(a.effect.getTiming())]; });
  t('anim.animate.bad', function () { var o = []; [undefined, 5, 'x', [{ offset: 3 }]].forEach(function (v) { try { el.animate(v, 100); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); try { el.animate(KF, -1); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  // ---- async lifecycle
  await ta('async.ready', async function () { var a = mk(); var p0 = a.pending; var r = await a.ready; return [p0, r === a, a.pending, a.playState, a.startTime !== null, typeof a.currentTime]; });
  await ta('async.finished.promise', async function () { var a = mk(30); var f = await a.finished; return [f === a, a.playState, a.currentTime]; });
  await ta('async.finish.events', async function () { var a = mk(40); var log = []; a.onfinish = function (e) { log.push(['onfinish', e.type, e.currentTime, e.timelineTime === null ? 'null' : typeof e.timelineTime, e instanceof AnimationPlaybackEvent, e.target === a]); }; a.addEventListener('finish', function () { log.push('listener'); }); await a.finished; await new Promise(function (r) { setTimeout(r, 300); }); return log; });
  await ta('async.cancel.reject', async function () { var a = mk(); var f = a.finished; var log = []; a.oncancel = function (e) { log.push(['cancel', e.currentTime, typeof e.timelineTime]); }; a.cancel(); try { await f; } catch (e) { log.push(e.name + ': ' + e.message); } await new Promise(function (r) { setTimeout(r, 300); }); return [log, a.finished === f]; });
  await ta('async.cancel.ready', async function () { var a = mk(); a.pause(); var r = a.ready; a.cancel(); try { await r; return 'resolved'; } catch (e) { return e.name + ': ' + e.message; } });
  await ta('async.finish.reuse', async function () { var a = mk(1000); var f1 = a.finished; a.finish(); await f1; var f2 = a.finished; a.play(); return [f1 === f2, a.finished === f1, a.playState]; });
  await ta('async.pause.ready', async function () { var a = mk(); await a.ready; a.pause(); var p = a.pending; var r = await a.ready; return [p, a.pending, a.playState, a.currentTime >= 0]; });
  await ta('async.progress', async function () { var a = mk(); await a.ready; var c0 = a.currentTime; await new Promise(function (r) { setTimeout(r, 60); }); var c1 = a.currentTime; return [c1 > c0, c1 - c0 < 5000]; });
  await ta('async.paused.hold', async function () { var a = mk(); await a.ready; a.pause(); await a.ready; var c0 = a.currentTime; await new Promise(function (r) { setTimeout(r, 40); }); return [a.currentTime === c0, a.playState]; });
  await ta('async.rate.ct', async function () { var a = mk(); a.playbackRate = 0; await a.ready; var c0 = a.currentTime; await new Promise(function (r) { setTimeout(r, 40); }); return [a.currentTime === c0, a.playState]; });
  await ta('async.remove', async function () { var d = document.createElement('div'); document.body.appendChild(d); var a = d.animate(KF, { duration: 10, fill: 'forwards' }); var b = d.animate(KF, { duration: 10, fill: 'forwards' }); await b.finished; await new Promise(function (r) { setTimeout(r, 300); }); return [a.replaceState, b.replaceState]; });
  await ta('async.event.ctor', async function () { var e = new AnimationPlaybackEvent('finish', { currentTime: 5, timelineTime: 7 }); var d = new AnimationPlaybackEvent('x'); return [e.currentTime, e.timelineTime, d.currentTime, d.timelineTime, e instanceof Event, e.bubbles]; });
  return JSON.stringify(R);
})()
