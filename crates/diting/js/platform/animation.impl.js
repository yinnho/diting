//! module: animation
//! layer: L2
//! exposes: AnimationTimeline, DocumentTimeline, AnimationEffect, KeyframeEffect, Animation, AnimationPlaybackEvent, __diting_animate, __diting_getAnimations
//! requires-load: legacy:Event, legacy:EventTarget, legacy:_eventState, legacy:Document
//! requires-call: legacy:Element, legacy:DOMException, legacy:performance, legacy:document, legacy:setTimeout, legacy:clearTimeout, legacy:queueMicrotask

// Web Animations (M4 knife 17). The bootstrap had Animation / KeyframeEffect /
// AnimationEffect / DocumentTimeline as constructor stubs and Element.animate
// returned a hand-rolled object literal (web_api_gaps.js) that was not an
// Animation, had no real playState machine, no pending state, no
// KeyframeEffect. Pinned by chrome/anim-truth-145.json (anim-probe.js: shape of
// six interfaces, timing validation, keyframe normalization, the play/pause/
// finish/cancel/reverse/updatePlaybackRate state machine, ready/finished promise
// identity, finish/cancel/remove events, replace state, getAnimations) and
// diffed by animation_matches_chrome_truth.
//
// Model: times are ms doubles. An animation keeps startTime / holdTime /
// playbackRate / pendingPlaybackRate and a pending task ('play' | 'pause' |
// 'rate') that a ~16ms "frame" timer completes (ready resolves there, exactly
// where Chrome resolves it at the next frame). currentTime is derived; playState
// is derived. Natural finish is a single timer armed from the remaining time;
// finish/cancel/remove events are dispatched on a frame too.

const FRAME = 16;
const effSlots = new WeakMap();
const tlSlots = new WeakMap();
const animSlots = new WeakMap();
const evSlots = new WeakMap();
let seq = 0;
const live = new Set();

function dbl(v, prefix, fn) {
  if (typeof v === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', prefix);
  const n = Number(v);
  if (!isFinite(n)) P.fail(fn, 'The provided double value is non-finite.', prefix);
  return n;
}
function domErr(msg, name) { return new DOMException(msg, name); }
function pnow() { const n = performance.now(); return n > 0 ? n : 0; }

// ---------------------------------------------------------------- easing
const NAMED = { linear: null, ease: [0.25, 0.1, 0.25, 1], 'ease-in': [0.42, 0, 1, 1], 'ease-out': [0, 0, 0.58, 1], 'ease-in-out': [0.42, 0, 0.58, 1] };
function validEasing(s) {
  s = s.trim();
  if (s in NAMED || s === 'step-start' || s === 'step-end') return true;
  let m = /^cubic-bezier\(\s*([-+.\deE]+)\s*,\s*([-+.\deE]+)\s*,\s*([-+.\deE]+)\s*,\s*([-+.\deE]+)\s*\)$/.exec(s);
  if (m) { const a = m.slice(1).map(Number); return a.every(isFinite) && a[0] >= 0 && a[0] <= 1 && a[2] >= 0 && a[2] <= 1; }
  if (/^steps\(\s*\d+\s*(,\s*(start|end|jump-start|jump-end|jump-none|jump-both)\s*)?\)$/.test(s)) return true;
  return /^linear\(.+\)$/.test(s);
}
function bezier(x1, y1, x2, y2, x) {
  if (x <= 0) return 0;
  if (x >= 1) return 1;
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  let t = x;
  for (let i = 0; i < 8; i++) {
    const e = ((ax * t + bx) * t + cx) * t - x;
    if (Math.abs(e) < 1e-7) break;
    const d = (3 * ax * t + 2 * bx) * t + cx;
    if (Math.abs(d) < 1e-6) break;
    t -= e / d;
  }
  return ((ay * t + by) * t + cy) * t;
}
function ease(s, x) {
  s = s.trim();
  if (s === 'linear' || /^linear\(/.test(s)) return x;
  if (s in NAMED) { const b = NAMED[s]; return bezier(b[0], b[1], b[2], b[3], x); }
  if (s === 'step-start') return x <= 0 ? 0 : 1;
  if (s === 'step-end') return x >= 1 ? 1 : 0;
  let m = /^cubic-bezier\(([^)]*)\)$/.exec(s);
  if (m) { const a = m[1].split(',').map(Number); return bezier(a[0], a[1], a[2], a[3], x); }
  m = /^steps\(\s*(\d+)\s*(?:,\s*([\w-]+)\s*)?\)$/.exec(s);
  if (m) {
    const n = Number(m[1]), pos = m[2] || 'end';
    let k = Math.floor(x * n);
    if (pos === 'start' || pos === 'jump-start' || pos === 'jump-both') k += 1;
    const steps = pos === 'jump-both' ? n + 1 : pos === 'jump-none' ? n - 1 : n;
    return Math.min(Math.max(steps === 0 ? 0 : k / steps, 0), 1);
  }
  return x;
}

// ---------------------------------------------------------------- timing
const ENUM = {
  direction: ['normal', 'reverse', 'alternate', 'alternate-reverse'],
  fill: ['none', 'forwards', 'backwards', 'both', 'auto'],
};
const ENUM_NAME = { direction: 'PlaybackDirection', fill: 'FillMode' };
function defTiming() { return { delay: 0, direction: 'normal', duration: 'auto', easing: 'linear', endDelay: 0, fill: 'auto', iterationStart: 0, iterations: 1 }; }
// src: number | EffectTiming-like | undefined, merged onto base (a fresh object)
function parseTiming(src, base, pre, fn, dictName) {
  const t = Object.assign({}, base);
  if (src === undefined || src === null) return t;
  if (typeof src === 'number' || typeof src === 'string' || typeof src === 'boolean') { checkDuration(t, typeof src === 'number' ? src : Number(src), pre, fn); return t; }
  const rd = (k) => pre + ": Failed to read the '" + k + "' property from '" + dictName + "'";
  const get = (k) => src[k];
  let v;
  if ((v = get('delay')) !== undefined) t.delay = dbl(v, rd('delay'), fn);
  if ((v = get('direction')) !== undefined) t.direction = enumv('direction', v, rd('direction'), fn);
  if ((v = get('duration')) !== undefined) checkDuration(t, v, pre, fn);
  if ((v = get('easing')) !== undefined) {
    const s = P.conv.DOMString(v, rd('easing'), fn);
    if (!validEasing(s)) P.fail(fn, "'" + s + "' is not a valid value for easing", pre);
    t.easing = s;
  }
  if ((v = get('endDelay')) !== undefined) t.endDelay = Number(v);
  if ((v = get('fill')) !== undefined) t.fill = enumv('fill', v, rd('fill'), fn);
  if ((v = get('iterationStart')) !== undefined) {
    const n = Number(v);
    if (!(n >= 0)) P.fail(fn, 'iterationStart must be non-negative', pre);
    t.iterationStart = n;
  }
  if ((v = get('iterations')) !== undefined) {
    const n = Number(v);
    if (!(n >= 0)) P.fail(fn, 'iterationCount must be non-negative', pre);
    t.iterations = n;
  }
  return t;
}
function enumv(k, v, prefix, fn) {
  const s = P.conv.DOMString(v, prefix, fn);
  if (ENUM[k].indexOf(s) < 0) P.fail(fn, "The provided value '" + s + "' is not a valid enum value of type " + ENUM_NAME[k] + '.', prefix);
  return s;
}
function checkDuration(t, v, pre, fn) {
  if (typeof v === 'string') { if (v === 'auto') { t.duration = 'auto'; return; } P.fail(fn, 'duration must be non-negative or auto', pre); }
  const n = Number(v);
  if (!(n >= 0)) P.fail(fn, 'duration must be non-negative or auto', pre);
  t.duration = n;
}
function orderTiming(t) {
  return { delay: t.delay, direction: t.direction, duration: t.duration, easing: t.easing, endDelay: t.endDelay, fill: t.fill, iterationStart: t.iterationStart, iterations: t.iterations };
}

// times are computed in seconds then scaled, as Chrome does (215.00000000000003)
function endsOf(t) {
  const dur = t.duration === 'auto' ? 0 : t.duration;
  const delay = t.delay / 1000, dur_s = dur / 1000, end = t.endDelay / 1000;
  const active_s = dur_s === 0 || t.iterations === 0 ? 0 : dur_s * t.iterations;
  return { active: active_s * 1000, end: Math.max(delay + active_s + end, 0) * 1000 };
}
function computeTiming(t, local, rate) {
  const dur = t.duration === 'auto' ? 0 : t.duration;
  const e = endsOf(t);
  const out = Object.assign({}, orderTiming(t), { duration: dur, fill: t.fill === 'auto' ? 'none' : t.fill, activeDuration: e.active, currentIteration: null, endTime: e.end, localTime: local === undefined ? null : local, progress: null });
  if (local === null || local === undefined) return out;
  const before = Math.max(Math.min(t.delay, e.end), 0), after = Math.max(Math.min(t.delay + e.active, e.end), 0);
  const backwards = rate < 0;
  let phase = 'active';
  if (local < before || (backwards && local === before)) phase = 'before';
  else if (local > after || (!backwards && local === after)) phase = 'after';
  const fill = out.fill;
  let active = null;
  if (phase === 'before') active = fill === 'backwards' || fill === 'both' ? Math.max(local - t.delay, 0) : null;
  else if (phase === 'active') active = local - t.delay;
  else active = fill === 'forwards' || fill === 'both' ? Math.max(Math.min(local - t.delay, e.active), 0) : null;
  if (active === null) return out;
  let overall = dur === 0 ? (phase === 'before' ? 0 : t.iterations) : active / dur;
  const ov = isFinite(overall) ? overall + t.iterationStart : overall;
  let simple = isFinite(ov) ? ov % 1 : t.iterationStart % 1;
  if (simple === 0 && (phase === 'active' || phase === 'after') && active === e.active && t.iterations !== 0) simple = 1;
  let iter;
  if (phase === 'after' && t.iterations === Infinity) iter = Infinity;
  else if (simple === 1) iter = Math.floor(ov) - 1;
  else iter = Math.floor(ov);
  out.currentIteration = iter;
  let dir = t.direction;
  let rev = dir === 'reverse' || (dir === 'alternate' && iter % 2 === 1) || (dir === 'alternate-reverse' && iter % 2 === 0);
  const directed = rev ? 1 - simple : simple;
  out.progress = ease(t.easing, directed);
  return out;
}

// ---------------------------------------------------------------- keyframes
const COMPOSITE_AUTO = ['replace', 'add', 'accumulate', 'auto'];
function objCheck(v, prefix, fn, param) {
  if (v === null || (typeof v !== 'object' && typeof v !== 'function')) P.fail(fn, 'parameter ' + param + " is not of type 'object'.", prefix);
}
function kfEnum(name, list, v, prefix, fn, dict) {
  const s = P.conv.DOMString(v, prefix, fn);
  if (list.indexOf(s) < 0) P.fail(fn, "The provided value '" + s + "' is not a valid enum value of type CompositeOperationOrAuto.", prefix + ": Failed to read the '" + name + "' property from '" + dict + "'");
  return s;
}
function parseKeyframes(v, pre, param, fn) {
  if (v === undefined || v === null) return [];
  objCheck(v, pre, fn, param);
  const out = [];
  const reserved = { offset: 1, easing: 1, composite: 1 };
  const ek = (k, val) => {
    const s = P.conv.DOMString(val, pre, fn);
    if (!validEasing(s)) P.fail(fn, "'" + s + "' is not a valid value for easing", pre);
    return s;
  };
  if (typeof v[Symbol.iterator] === 'function') {
    for (const item of v) {
      objCheck(item, pre, fn, param);
      const o = { props: {}, offset: null, easing: 'linear', composite: 'auto', computed: 0 };
      if (item.composite !== undefined) o.composite = kfEnum('composite', COMPOSITE_AUTO, item.composite, pre, fn, 'BaseKeyframe');
      if (item.easing !== undefined) o.easing = ek('easing', item.easing);
      if (item.offset !== undefined && item.offset !== null) o.offset = Number(item.offset);
      for (const k of Object.keys(item)) if (!reserved[k]) o.props[k] = P.conv.DOMString(item[k], pre, fn);
      out.push(o);
    }
    for (const o of out) if (o.offset !== null && !(o.offset >= 0 && o.offset <= 1)) P.fail(fn, 'Offsets must be null or in the range [0,1].', pre);
    let last = -Infinity;
    for (const o of out) if (o.offset !== null) { if (o.offset < last) P.fail(fn, 'Offsets must be monotonically non-decreasing.', pre); last = o.offset; }
    // fill computed offsets: first -> 0, last -> 1, runs of unspecified spaced evenly
    const n = out.length;
    if (n > 0) {
      if (out[0].offset === null) out[0].computed = 0; else out[0].computed = out[0].offset;
      for (let i = 0; i < n; i++) if (out[i].offset !== null) out[i].computed = out[i].offset;
      if (out[n - 1].offset === null) out[n - 1].computed = 1;
      let i = 0;
      while (i < n) {
        if (out[i].offset === null && !(i === 0) && !(i === n - 1)) {
          let j = i;
          while (j < n && out[j].offset === null && j !== n - 1) j++;
          const lo = out[i - 1].computed, hi = out[j].computed, span = j - i + 1;
          for (let k = i; k < j; k++) out[k].computed = lo + ((hi - lo) * (k - i + 1)) / span;
          i = j;
        } else i++;
      }
    }
    return out;
  }
  // property-indexed
  const composite = v.composite === undefined ? 'auto' : kfEnum('composite', COMPOSITE_AUTO, v.composite, pre, fn, 'BasePropertyIndexedKeyframe');
  let easing = 'linear';
  if (v.easing !== undefined) easing = ek('easing', v.easing);
  let offsets = null;
  if (v.offset !== undefined && v.offset !== null) offsets = Array.isArray(v.offset) ? v.offset.map((x) => (x === null ? null : Number(x))) : [Number(v.offset)];
  const byOffset = new Map();
  for (const k of Object.keys(v)) {
    if (reserved[k]) continue;
    const raw = v[k];
    const vals = Array.isArray(raw) ? raw : [raw];
    const nv = vals.length;
    vals.forEach((val, i) => {
      let off = offsets !== null && offsets[i] !== undefined && offsets[i] !== null ? offsets[i] : null;
      const comp = off !== null ? off : nv === 1 ? 1 : i / (nv - 1);
      if (!(comp >= 0 && comp <= 1)) P.fail(fn, 'Offsets must be null or in the range [0,1].', pre);
      const key = String(comp);
      let o = byOffset.get(key);
      if (!o) { o = { props: {}, offset: off, easing, composite, computed: comp }; byOffset.set(key, o); }
      o.props[k] = P.conv.DOMString(val, pre, fn);
    });
  }
  return Array.from(byOffset.values()).sort((a, b) => a.computed - b.computed);
}
function kfOut(list) {
  return list.map((o) => Object.assign({}, o.props, { offset: o.offset, computedOffset: o.computed, easing: o.easing, composite: o.composite }));
}

// ---------------------------------------------------------------- AnimationTimeline / DocumentTimeline
function AnimationTimeline() { P.fail(AnimationTimeline, 'Illegal constructor', "Failed to construct 'AnimationTimeline'"); }
const ATP = P.iface(AnimationTimeline, 'AnimationTimeline', 0);
function tlSlot(o, fn) { const s = o !== null && typeof o === 'object' ? tlSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
P.getter(ATP, 'currentTime', function __f() { const s = tlSlot(this, __f); return pnow() - s.origin; });
P.getter(ATP, 'duration', function __f() { tlSlot(this, __f); return null; });
P.seal(AnimationTimeline);

const DPRE = "Failed to construct 'DocumentTimeline'";
function DocumentTimeline() {
  if (new.target === undefined) P.fail(DocumentTimeline, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", DPRE);
  let o = arguments[0];
  if (o === null) o = undefined;
  let origin = 0;
  if (o !== undefined) {
    if (typeof o !== 'object' && typeof o !== 'function') P.fail(DocumentTimeline, "The provided value is not of type 'DocumentTimelineOptions'.", DPRE);
    if (o.originTime !== undefined) origin = dbl(o.originTime, DPRE + ": Failed to read the 'originTime' property from 'DocumentTimelineOptions'", DocumentTimeline);
  }
  const self = Reflect.construct(Object, [], new.target);
  tlSlots.set(self, { origin });
  return self;
}
const DTP = P.iface(DocumentTimeline, 'DocumentTimeline', 0, AnimationTimeline);
P.seal(DocumentTimeline);
let docTimeline = null;

// ---------------------------------------------------------------- AnimationEffect / KeyframeEffect
function AnimationEffect() { P.fail(AnimationEffect, 'Illegal constructor', "Failed to construct 'AnimationEffect'"); }
const AEP = P.iface(AnimationEffect, 'AnimationEffect', 0);
function effSlot(o, fn) { const s = o !== null && typeof o === 'object' ? effSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
function animCur(s) { return s.animation === null ? null : currentTime(s.animation); }
function animRate(s) { return s.animation === null ? 1 : s.animation.rate; }
P.method(AEP, 'getComputedTiming', 0, function __f() { const s = effSlot(this, __f); const c = animCur(s); return computeTiming(s.timing, c, animRate(s)); });
P.method(AEP, 'getTiming', 0, function __f() { return orderTiming(effSlot(this, __f).timing); });
P.method(AEP, 'updateTiming', 0, function __f() {
  const s = effSlot(this, __f);
  s.timing = parseTiming(arguments[0], s.timing, "Failed to execute 'updateTiming' on 'AnimationEffect'", __f, 'OptionalEffectTiming');
  if (s.animation !== null) changed(s.animation);
});
P.seal(AnimationEffect);

const KPRE = "Failed to construct 'KeyframeEffect'";
function newEffect(newTarget, target, keyframes, timing, composite, pseudo) {
  const self = Reflect.construct(Object, [], newTarget);
  effSlots.set(self, { target, pseudo, composite, timing, keyframes, animation: null });
  return self;
}
function isElement(v) { return typeof Element === 'function' && v instanceof Element; }
function KeyframeEffect(target) {
  if (new.target === undefined) P.fail(KeyframeEffect, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", KPRE);
  if (arguments.length < 1) P.fail(KeyframeEffect, '1 argument required, but only 0 present.', KPRE);
  const src = target !== null && typeof target === 'object' ? effSlots.get(target) : undefined;
  if (src !== undefined && arguments.length === 1) {
    return newEffect(new.target, src.target, src.keyframes.map((k) => Object.assign({}, k, { props: Object.assign({}, k.props) })), Object.assign({}, src.timing), src.composite, src.pseudo);
  }
  if (target !== null && !isElement(target)) P.fail(KeyframeEffect, "parameter 1 is not of type 'Element'.", KPRE);
  const keyframes = parseKeyframes(arguments[1], KPRE, 2, KeyframeEffect);
  const opts = arguments[2];
  const timing = parseTiming(opts, defTiming(), KPRE, KeyframeEffect, 'EffectTiming');
  let composite = 'replace', pseudo = null;
  if (opts !== null && typeof opts === 'object') {
    if (opts.composite !== undefined) {
      const c = P.conv.DOMString(opts.composite, KPRE, KeyframeEffect);
      if (['replace', 'add', 'accumulate'].indexOf(c) < 0) P.fail(KeyframeEffect, "The provided value '" + c + "' is not a valid enum value of type CompositeOperation.", KPRE + ": Failed to read the 'composite' property from 'KeyframeEffectOptions'");
      composite = c;
    }
    if (opts.pseudoElement !== undefined && opts.pseudoElement !== null) pseudo = P.conv.DOMString(opts.pseudoElement, KPRE, KeyframeEffect);
  }
  return newEffect(new.target, target, keyframes, timing, composite, pseudo);
}
const KEP = P.iface(KeyframeEffect, 'KeyframeEffect', 1, AnimationEffect);
P.accessor(KEP, 'target', function __f() { return effSlot(this, __f).target; }, function __f(v) {
  const s = effSlot(this, __f);
  if (v !== null && !isElement(v)) P.fail(__f, "Failed to convert value to 'Element'.", "Failed to set the 'target' property on 'KeyframeEffect'");
  s.target = v;
});
P.accessor(KEP, 'pseudoElement', function __f() { return effSlot(this, __f).pseudo; }, function __f(v) {
  const s = effSlot(this, __f);
  if (v === null || v === undefined) { s.pseudo = null; return; }
  const str = P.conv.DOMString(v, "Failed to set the 'pseudoElement' property on 'KeyframeEffect'", __f);
  if (!/^::[a-z-]+(\(.*\))?$/.test(str)) throw domErr("Failed to set the 'pseudoElement' property on 'KeyframeEffect': " + str + ' is a syntactically invalid pseudo-element', 'SyntaxError');
  s.pseudo = str;
});
P.accessor(KEP, 'composite', function __f() { return effSlot(this, __f).composite; }, function __f(v) {
  const s = effSlot(this, __f);
  const c = String(v);
  if (c === 'replace' || c === 'add' || c === 'accumulate') s.composite = c;
});
P.method(KEP, 'getKeyframes', 0, function __f() { return kfOut(effSlot(this, __f).keyframes); });
P.method(KEP, 'setKeyframes', 1, function __f(k) {
  const s = effSlot(this, __f);
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', "Failed to execute 'setKeyframes' on 'KeyframeEffect'");
  s.keyframes = parseKeyframes(k, "Failed to execute 'setKeyframes' on 'KeyframeEffect'", 1, __f);
});
P.seal(KeyframeEffect);

// ---------------------------------------------------------------- AnimationPlaybackEvent
const EPRE = "Failed to construct 'AnimationPlaybackEvent'";
function AnimationPlaybackEvent(type) {
  if (new.target === undefined) P.fail(AnimationPlaybackEvent, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", EPRE);
  P.args(EPRE, arguments.length, 1, AnimationPlaybackEvent);
  const init = arguments[1];
  const ev = Reflect.construct(Event, [type, init], new.target);
  const rd = (k) => EPRE + ": Failed to read the '" + k + "' property from 'AnimationPlaybackEventInit'";
  const num = (k) => (init === undefined || init === null || init[k] === undefined || init[k] === null ? null : dbl(init[k], rd(k), AnimationPlaybackEvent));
  evSlots.set(ev, { currentTime: num('currentTime'), timelineTime: num('timelineTime') });
  return ev;
}
const APEP = P.iface(AnimationPlaybackEvent, 'AnimationPlaybackEvent', 1, Event);
function evSlot(o, fn) { const s = o !== null && typeof o === 'object' ? evSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
P.getter(APEP, 'currentTime', function __f() { return evSlot(this, __f).currentTime; });
P.getter(APEP, 'timelineTime', function __f() { return evSlot(this, __f).timelineTime; });
P.seal(AnimationPlaybackEvent);

// ---------------------------------------------------------------- Animation core
function deferred(resolved) {
  const d = { settled: false, promise: null, resolve: null, reject: null };
  d.promise = new Promise((res, rej) => { d.resolve = res; d.reject = rej; });
  d.promise.catch(() => {});
  if (resolved) { d.settled = true; }
  return d;
}
function settle(d, value) { if (!d.settled) { d.settled = true; d.resolve(value); } }
function rejectWith(d, err) { if (!d.settled) { d.settled = true; d.reject(err); } }
const abortErr = () => domErr('The user aborted a request.', 'AbortError');

function aSlot(o, fn) { const s = o !== null && typeof o === 'object' ? animSlots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; }
function tlNow(a) { return a.timeline === null ? null : pnow() - tlSlots.get(a.timeline).origin; }
function effEnd(a) { return a.effect === null ? 0 : endsOf(effSlots.get(a.effect).timing).end; }
function effRate(a) { return a.pendingRate !== null ? a.pendingRate : a.rate; }
function currentTime(a) {
  if (a.hold !== null) return a.hold;
  const t = tlNow(a);
  if (t === null || a.startTime === null) return null;
  return (t - a.startTime) * a.rate;
}
function playStateOf(a) {
  const cur = currentTime(a);
  if (cur === null && a.task === null) return 'idle';
  if (a.task === 'pause' || (a.startTime === null && a.task !== 'play' && a.task !== 'rate')) return 'paused';
  const end = effEnd(a);
  const er = effRate(a);
  if (cur !== null && ((er > 0 && cur >= end) || (er < 0 && cur <= 0))) return 'finished';
  return 'running';
}
function fire(a, type, frame) {
  const self = a.self;
  setTimeout(() => {
    const t = tlNow(a);
    const e = new AnimationPlaybackEvent(type, { currentTime: type === 'cancel' ? null : currentTime(a), timelineTime: t });
    _eventState(e, 'isTrusted', true);
    try { self.dispatchEvent(e); } catch (x) {}
  }, FRAME);
}
function scheduleTask(a) {
  if (a.frame !== 0) return;
  a.frame = setTimeout(() => { a.frame = 0; runTask(a); }, FRAME);
}
function ensurePendingReady(a) { if (a.ready.settled) a.ready = deferred(false); }
function runTask(a) {
  const task = a.task;
  if (task === null) return;
  const t = tlNow(a);
  if (t === null) { a.frame = 0; scheduleTask(a); return; }
  if (task === 'play') {
    if (a.pendingRate !== null) { a.rate = a.pendingRate; a.pendingRate = null; }
    if (a.hold !== null) {
      a.startTime = a.rate !== 0 ? t - a.hold / a.rate : t;
      if (a.rate !== 0) a.hold = null;
    }
  } else if (task === 'pause') {
    const cur = currentTime(a);
    if (a.pendingRate !== null) { a.rate = a.pendingRate; a.pendingRate = null; }
    if (cur !== null) a.hold = cur;
    a.startTime = null;
  } else if (task === 'rate') {
    const cur = currentTime(a);
    if (a.pendingRate !== null) {
      a.rate = a.pendingRate; a.pendingRate = null;
      if (cur !== null && a.hold === null && a.rate !== 0) a.startTime = t - cur / a.rate;
    }
  }
  a.task = null;
  settle(a.ready, a.self);
  changed(a);
}
function cancelTask(a) {
  if (a.frame !== 0) { clearTimeout(a.frame); a.frame = 0; }
  a.task = null;
}
function arm(a) {
  if (a.timer !== 0) { clearTimeout(a.timer); a.timer = 0; }
  if (playStateOf(a) !== 'running' || a.startTime === null || a.rate === 0 || a.hold !== null) return;
  const cur = currentTime(a), end = effEnd(a);
  const remain = a.rate > 0 ? (end - cur) / a.rate : cur / -a.rate;
  if (!isFinite(remain) || remain < 0) return;
  a.timer = setTimeout(() => { a.timer = 0; changed(a); }, Math.ceil(remain) + 1);
}
// recompute finished state, registry, timers; notify finish; replace sweep
function changed(a) {
  if (a.task === null && a.hold !== null && a.startTime !== null && a.rate !== 0 && playStateOf(a) !== 'finished') {
    const t = tlNow(a);
    if (t !== null) { a.startTime = t - a.hold / a.rate; a.hold = null; }
  }
  const ps = playStateOf(a);
  if (ps === 'finished' && a.task === null) {
    const end = effEnd(a);
    if (a.hold === null) a.hold = a.rate > 0 ? end : 0;
    if (!a.finishNotified) {
      a.finishNotified = true;
      queueMicrotask(() => { if (playStateOf(a) === 'finished') settle(a.finished, a.self); });
      fire(a, 'finish');
      sweep();
    }
  } else if (ps !== 'finished') {
    a.finishNotified = false;
  }
  if (a.effect !== null && ps !== 'idle') live.add(a); else live.delete(a);
  if (ps === 'finished' && !inEffect(a)) live.delete(a);
  arm(a);
}
function inEffect(a) {
  if (a.effect === null) return false;
  const s = effSlots.get(a.effect);
  return computeTiming(s.timing, currentTime(a), a.rate).progress !== null;
}
function relevant(a) {
  if (a.replace === 'removed' || a.effect === null) return false;
  const ps = playStateOf(a);
  if (ps === 'idle') return false;
  return ps !== 'finished' || inEffect(a);
}
function props(a) { const s = effSlots.get(a.effect); const o = new Set(); for (const k of s.keyframes) for (const p of Object.keys(k.props)) o.add(p); return o; }
// removal of replaced animations (spec: replace state); frame-agnostic here
function sweep() {
  const fin = Array.from(live).filter((a) => a.effect !== null && playStateOf(a) === 'finished' && a.replace === 'active' && effSlots.get(a.effect).target !== null)
    .sort((x, y) => x.seq - y.seq);
  for (let i = 0; i < fin.length; i++) {
    const x = fin[i], xs = effSlots.get(x.effect);
    for (let j = fin.length - 1; j > i; j--) {
      const y = fin[j], ys = effSlots.get(y.effect);
      if (ys.target !== xs.target || ys.composite !== 'replace' || !inEffect(y)) continue;
      if (ys.keyframes.some((k) => k.composite !== 'auto' && k.composite !== 'replace')) continue;
      const yp = props(y);
      let all = true;
      for (const p of props(x)) if (!yp.has(p)) { all = false; break; }
      if (!all) continue;
      x.replace = 'removed';
      live.delete(x);
      fire(x, 'remove');
      break;
    }
  }
}

const APRE = "Failed to execute '";
function seek(a, v) {
  if (a.task === 'pause') { cancelTask(a); a.startTime = null; if (a.pendingRate !== null) { a.rate = a.pendingRate; a.pendingRate = null; } settle(a.ready, a.self); }
  const t = tlNow(a);
  if (a.startTime === null || t === null || a.rate === 0) a.hold = v;
  else { a.startTime = t - v / a.rate; a.hold = null; }
}
function doPlay(a, fn, rewind) {
  const er = effRate(a), cur = currentTime(a), end = effEnd(a);
  let seeked = false;
  if (rewind) {
    if (er > 0 && (cur === null || cur < 0 || cur >= end)) { a.hold = 0; seeked = true; }
    else if (er < 0 && (cur === null || cur <= 0 || cur > end)) {
      if (end === Infinity) throw domErr("Failed to execute 'play' on 'Animation': Cannot play reversed Animation with infinite target effect end.", 'InvalidStateError');
      a.hold = end; seeked = true;
    } else if (er === 0 && cur === null) { a.hold = 0; seeked = true; }
  }
  const hadTask = a.task !== null;
  if (a.hold !== null) a.startTime = null;
  if (a.hold === null && !hadTask && a.pendingRate === null && !seeked) { changed(a); return; }
  if (a.finished.settled) a.finished = deferred(false);
  a.finishNotified = false;
  a.task = 'play';
  ensurePendingReady(a);
  scheduleTask(a);
  changed(a);
}
function doPause(a, fn) {
  if (a.task === 'pause') return;
  if (playStateOf(a) === 'paused' && a.task === null) return;
  const er = effRate(a);
  if (currentTime(a) === null) {
    if (er >= 0) a.hold = 0;
    else {
      const end = effEnd(a);
      if (end === Infinity) throw domErr("Failed to execute 'pause' on 'Animation': Cannot pause reversed Animation with infinite target effect end.", 'InvalidStateError');
      a.hold = end;
    }
  }
  const hadPlay = a.task === 'play';
  a.task = 'pause';
  if (!hadPlay) ensurePendingReady(a);
  scheduleTask(a);
  changed(a);
}
function doCancel(a) {
  if (playStateOf(a) !== 'idle') {
    const err = abortErr();
    rejectWith(a.finished, err);
    if (!a.ready.settled) rejectWith(a.ready, abortErr());
    fire(a, 'cancel');
    a.finished = deferred(false);
    a.ready = deferred(true); a.ready.resolve(a.self);
  }
  cancelTask(a);
  if (a.pendingRate !== null) { a.rate = a.pendingRate; a.pendingRate = null; }
  a.hold = null; a.startTime = null; a.finishNotified = false;
  changed(a);
}

function Animation() {
  if (new.target === undefined) P.fail(Animation, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'Animation'");
  const pre = "Failed to construct 'Animation'";
  let effect = arguments[0], timeline = arguments[1];
  if (effect === undefined || effect === null) effect = null;
  else if (effSlots.get(effect) === undefined) P.fail(Animation, "parameter 1 is not of type 'AnimationEffect'.", pre);
  if (timeline === undefined) timeline = documentTimeline();
  else if (timeline !== null && (typeof timeline !== 'object' || tlSlots.get(timeline) === undefined)) P.fail(Animation, "parameter 2 is not of type 'AnimationTimeline'.", pre);
  return makeAnimation(new.target, effect, timeline);
}
function documentTimeline() { if (docTimeline === null) docTimeline = Reflect.construct(DocumentTimeline, [], DocumentTimeline); return docTimeline; }
function makeAnimation(newTarget, effect, timeline) {
  const self = Reflect.construct(EventTarget, [], newTarget);
  const a = { self, effect, timeline, startTime: null, hold: null, rate: 1, pendingRate: null, task: null, frame: 0, timer: 0, ready: deferred(true), finished: deferred(false),
    id: '', replace: 'active', finishNotified: false, seq: ++seq, handlers: {}, rangeStart: null, rangeEnd: null };
  a.ready.resolve(self);
  animSlots.set(self, a);
  if (effect !== null) { const old = effSlots.get(effect); if (old.animation !== null) old.animation.effect = null; old.animation = a; }
  return self;
}
const AP = P.iface(Animation, 'Animation', 0, EventTarget);

P.accessor(AP, 'effect', function __f() { return aSlot(this, __f).effect; }, function __f(v) {
  const a = aSlot(this, __f);
  if (v !== null && v !== undefined && effSlots.get(v) === undefined) P.fail(__f, "Failed to convert value to 'AnimationEffect'.", "Failed to set the 'effect' property on 'Animation'");
  const next = v === undefined ? null : v;
  if (a.effect === next) return;
  if (a.effect !== null) effSlots.get(a.effect).animation = null;
  if (next !== null) { const ns = effSlots.get(next); if (ns.animation !== null) { ns.animation.effect = null; changed(ns.animation); } ns.animation = a; }
  a.effect = next;
  changed(a);
});
P.accessor(AP, 'timeline', function __f() { return aSlot(this, __f).timeline; }, function __f(v) {
  const a = aSlot(this, __f);
  if (v !== null && v !== undefined && (typeof v !== 'object' || tlSlots.get(v) === undefined)) P.fail(__f, "Failed to convert value to 'AnimationTimeline'.", "Failed to set the 'timeline' property on 'Animation'");
  a.timeline = v === undefined ? null : v;
  changed(a);
});
P.accessor(AP, 'startTime', function __f() { return aSlot(this, __f).startTime; }, function __f(v) {
  const a = aSlot(this, __f);
  const pre = "Failed to set the 'startTime' property on 'Animation'";
  const nv = v === null || v === undefined ? null : dbl(v, pre, __f);
  const prev = currentTime(a);
  a.startTime = nv;
  if (nv !== null) { if (a.rate !== 0) a.hold = null; else if (a.hold === null) a.hold = prev; }
  else if (prev !== null) a.hold = prev;
  if (a.task !== null) { const had = a.task; cancelTask(a); if (a.pendingRate !== null) { a.rate = a.pendingRate; a.pendingRate = null; } settle(a.ready, a.self); }
  changed(a);
});
P.accessor(AP, 'currentTime', function __f() { return currentTime(aSlot(this, __f)); }, function __f(v) {
  const a = aSlot(this, __f);
  const pre = "Failed to set the 'currentTime' property on 'Animation'";
  if (v === null || v === undefined) {
    if (currentTime(a) !== null) P.fail(__f, 'currentTime may not be changed from resolved to unresolved', pre);
    return;
  }
  seek(a, dbl(v, pre, __f));
  changed(a);
});
P.accessor(AP, 'playbackRate', function __f() { return aSlot(this, __f).rate; }, function __f(v) {
  const a = aSlot(this, __f);
  const n = dbl(v, "Failed to set the 'playbackRate' property on 'Animation'", __f);
  const prev = currentTime(a);
  a.pendingRate = null;
  a.rate = n;
  if (prev !== null) seek(a, prev);
  changed(a);
});
for (const k of ['rangeStart', 'rangeEnd']) {
  P.accessor(AP, k, function __f() { aSlot(this, __f); return null; }, function __f(v) { aSlot(this, __f); });
}
P.getter(AP, 'playState', function __f() { return playStateOf(aSlot(this, __f)); });
P.getter(AP, 'replaceState', function __f() { return aSlot(this, __f).replace; });
P.getter(AP, 'pending', function __f() { return aSlot(this, __f).task !== null; });
P.accessor(AP, 'id', function __f() { return aSlot(this, __f).id; }, function __f(v) { aSlot(this, __f).id = P.conv.DOMString(v, "Failed to set the 'id' property on 'Animation'", __f); });
for (const type of ['finish', 'cancel', 'remove']) {
  P.accessor(AP, 'on' + type, function __f() { const h = aSlot(this, __f).handlers[type]; return h ? h.fn : null; }, function __f(v) {
    const a = aSlot(this, __f);
    let h = a.handlers[type];
    if (!h) { h = a.handlers[type] = { fn: null, bound: false }; }
    h.fn = typeof v === 'function' ? v : null;
  });
}
P.getter(AP, 'finished', function __f() { return aSlot(this, __f).finished.promise; });
P.getter(AP, 'ready', function __f() { return aSlot(this, __f).ready.promise; });
const guard = (name, impl) => P.method(AP, name, 0, function __f() { const a = aSlot(this, __f); return impl(a, __f); });
guard('cancel', (a) => { doCancel(a); });
P.method(AP, 'commitStyles', 0, function __f() {
  const a = aSlot(this, __f);
  const eff = a.effect === null ? null : effSlots.get(a.effect);
  if (eff === null || eff.target === null) return;
  if (!eff.target.isConnected) throw domErr("Failed to execute 'commitStyles' on 'Animation': Target element is not rendered.", 'InvalidStateError');
  const c = computeTiming(Object.assign({}, eff.timing, { fill: 'both' }), currentTime(a), a.rate);
  if (c.progress === null) return;
  const names = new Set();
  for (const k of eff.keyframes) for (const p of Object.keys(k.props)) names.add(p);
  for (const p of names) {
    const ks = eff.keyframes.filter((k) => p in k.props);
    const v = valueAt(ks, p, c.progress);
    if (v !== null) eff.target.style.setProperty(p === 'cssFloat' ? 'float' : p.replace(/[A-Z]/g, (m) => '-' + m.toLowerCase()), v);
  }
});
function valueAt(ks, p, x) {
  if (ks.length === 0) return null;
  if (x <= ks[0].computed) return ks[0].props[p];
  if (x >= ks[ks.length - 1].computed) return ks[ks.length - 1].props[p];
  for (let i = 0; i + 1 < ks.length; i++) {
    const a = ks[i], b = ks[i + 1];
    if (x >= a.computed && x <= b.computed) {
      const f = b.computed === a.computed ? 1 : ease(a.easing, (x - a.computed) / (b.computed - a.computed));
      const ma = /^(-?\d*\.?\d+)([a-z%]*)$/i.exec(a.props[p]), mb = /^(-?\d*\.?\d+)([a-z%]*)$/i.exec(b.props[p]);
      if (ma && mb && ma[2] === mb[2]) return String(+ma[1] + (+mb[1] - +ma[1]) * f) + ma[2];
      return f < 0.5 ? a.props[p] : b.props[p];
    }
  }
  return ks[ks.length - 1].props[p];
}
P.method(AP, 'finish', 0, function __f() {
  const a = aSlot(this, __f);
  const er = effRate(a), end = effEnd(a);
  if (er === 0) throw domErr("Failed to execute 'finish' on 'Animation': Cannot finish Animation with a playbackRate of 0.", 'InvalidStateError');
  if (er > 0 && end === Infinity) throw domErr("Failed to execute 'finish' on 'Animation': Cannot finish Animation with an infinite target effect end.", 'InvalidStateError');
  a.rate = er; a.pendingRate = null;
  const limit = a.rate > 0 ? end : 0;
  seek(a, limit);
  const t = tlNow(a);
  if (a.startTime === null && t !== null) { a.startTime = t - limit / a.rate; a.hold = null; }
  if (a.task !== null) { cancelTask(a); settle(a.ready, a.self); }
  changed(a);
});
guard('pause', (a, fn) => { doPause(a, fn); });
guard('persist', (a) => { a.replace = 'persisted'; });
guard('play', (a, fn) => { doPlay(a, fn, true); });
guard('reverse', (a, fn) => {
  if (a.timeline === null) throw domErr("Failed to execute 'reverse' on 'Animation': Cannot reverse an animation with no active timeline", 'InvalidStateError');
  const orig = a.pendingRate, er = effRate(a);
  updateRate(a, -er);
  try { doPlay(a, fn, true); } catch (e) { a.pendingRate = orig; throw e; }
});
function updateRate(a, v) {
  const ps = playStateOf(a);
  if (ps === 'idle' || ps === 'paused') {
    const prev = currentTime(a);
    a.pendingRate = null; a.rate = v;
    if (prev !== null) seek(a, prev);
    changed(a);
    return;
  }
  a.pendingRate = v;
  if (a.task === null) { a.task = 'rate'; ensurePendingReady(a); scheduleTask(a); }
  changed(a);
}
P.method(AP, 'updatePlaybackRate', 1, function __f() {
  const a = aSlot(this, __f);
  const pre = "Failed to execute 'updatePlaybackRate' on 'Animation'";
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  updateRate(a, dbl(arguments[0], pre, __f));
});
P.getter(AP, 'overallProgress', function __f() {
  const a = aSlot(this, __f);
  const cur = currentTime(a);
  if (cur === null) return null;
  const end = effEnd(a);
  return end === 0 ? (cur >= 0 ? 1 : 0) : Math.min(Math.max(cur / end, 0), 1);
});
P.seal(Animation);

// ---------------------------------------------------------------- Element.animate / getAnimations (called by web_api_gaps.js glue)
const ANIMATE = "Failed to execute 'animate' on 'Element'";
P.expose('__diting_animate', function (el, keyframes, options, fn) {
  const kfs = parseKeyframes(keyframes, ANIMATE, 1, fn);
  const timing = parseTiming(options, defTiming(), ANIMATE, fn, 'EffectTiming');
  let composite = 'replace', pseudo = null, id = '';
  if (options !== null && typeof options === 'object') {
    if (options.composite !== undefined) {
      const c = String(options.composite);
      if (['replace', 'add', 'accumulate'].indexOf(c) < 0) P.fail(fn, "The provided value '" + c + "' is not a valid enum value of type CompositeOperation.", ANIMATE + ": Failed to read the 'composite' property from 'KeyframeAnimationOptions'");
      composite = c;
    }
    if (options.pseudoElement !== undefined && options.pseudoElement !== null) pseudo = String(options.pseudoElement);
    if (options.id !== undefined) id = P.conv.DOMString(options.id, ANIMATE, fn);
  }
  const eff = newEffect(KeyframeEffect, el, kfs, timing, composite, pseudo);
  const anim = makeAnimation(Animation, eff, documentTimeline());
  const a = animSlots.get(anim);
  a.id = id;
  doPlay(a, fn, true);
  return anim;
});
P.expose('__diting_getAnimations', function (root, subtree, connectedOnly) {
  const out = [];
  for (const a of live) {
    if (!relevant(a)) continue;
    const tg = effSlots.get(a.effect).target;
    if (tg === null) continue;
    if (connectedOnly === 'doc' ? tg.isConnected : tg === root || (subtree && root.contains(tg))) out.push(a);
  }
  return out.sort((x, y) => x.seq - y.seq).map((a) => a.self);
});
if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_animate', '__diting_getAnimations');
}
P.getter(Document.prototype, 'timeline', function __f() { return documentTimeline(); });

P.expose('AnimationTimeline', AnimationTimeline);
P.expose('DocumentTimeline', DocumentTimeline);
P.expose('AnimationEffect', AnimationEffect);
P.expose('KeyframeEffect', KeyframeEffect);
P.expose('Animation', Animation);
P.expose('AnimationPlaybackEvent', AnimationPlaybackEvent);
