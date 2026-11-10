(function () { // @@SCOPE
// god-file split (刀29, CSS animation/transition lifecycle): the timer-
// armed animation event engine (start fires at the observable mutation,
// cancel at the un-making, end on a timer carrying the declared active
// duration; iteration boundaries chain; animation-less pages never touch
// the timer system) + the transition watcher family. All state
// (_anim*/_trans* maps, sets, counters) lives here; domcore.js and
// event_hooks.js call the schedulers/cancellers at RUNTIME from mutation
// hooks — order-free, auto-published.
// --- CSS animation lifecycle events (blitz#863 family 1) -----------------
// diting's animation engine samples styles at css_time (None = end state)
// and has no compositor loop, so by the time page script can observe an
// animation it has already finished. The lifecycle collapses to match:
// `animationstart` fires synchronously at the mutation that makes the
// animation observable (same policy as Element.animate below),
// `animationcancel` fires at the mutation that un-makes it (name changed,
// removed, or the element detached), and `animationend` on a timer carrying
// the declared active duration. Timers are armed only for elements that
// actually animate — animation-less pages (the overwhelming majority) never
// touch the timer system from these hooks. Per-mutation tick scheduling was
// tried and is forbidden: a pending 0ms timer keeps every event-loop slice
// of the nav settle pump timing out, so dynamic imports stop settling
// (their fetch never gets a slice), and reactor-less test isolates abort
// inside op_sleep. elapsedTime reports the declared active duration
// (delay + duration × iteration-count) from computed style, and
// `await animationend` scripts proceed. Iteration boundaries are collapsed
// too but *are* observable: for a finite animation of N iterations the N-1
// interior boundaries fire `animationiteration` from chained timers, and an
// infinite one chains without bound (a 0s duration has no representable
// boundary and is left unarmed — otherwise the chain would busy-spin).
// Comma-separated animation lists are tracked by their first name only.
const _animTracked = new Map();      // nid -> animation state
let _animScanDepth = 0;              // dispatch handlers mutate too — reentrancy guard
const _animDeferredChecks = new Set();

function _cssSeconds(v) {
  const s = String(v == null ? "" : v).trim();
  const n = parseFloat(s);
  if (isNaN(n)) return 0;
  return /ms\s*$/.test(s) ? n / 1000 : n;
}

function _dispatchAnimationEvent(el, type, name, elapsed) {
  try {
    el.dispatchEvent(new AnimationEvent(type, {
      bubbles: true, composed: true,
      animationName: name, elapsedTime: elapsed, pseudoElement: "",
    }));
  } catch (e) {}
}

// One computed-style read per candidate: the name gate and the timing facts
// need the SAME cascade, and the cascade is the whole per-candidate cost
// (#224). _animationCheckOne used to read it twice — _animationNameOf's read
// for the gate, then _startAnimation's own read for duration/delay/count —
// which doubled every sweep for nothing.
function _animFactsOf(el) {
  const facts = { name: null, duration: 0, delay: 0, infinite: false, iterations: 1 };
  try {
    const cs = getComputedStyle(el);
    const raw = String(cs.getPropertyValue("animation-name") || "").trim();
    if (raw && raw !== "none") {
      const first = raw.split(",")[0].trim();
      facts.name = first && first !== "none" ? first : null;
    }
    facts.duration = Math.max(0, _cssSeconds(cs.getPropertyValue("animation-duration")));
    facts.delay = _cssSeconds(cs.getPropertyValue("animation-delay"));
    const iterRaw = String(cs.getPropertyValue("animation-iteration-count") || "1").trim();
    facts.infinite = iterRaw === "infinite";
    facts.iterations = facts.infinite ? 1 : (parseFloat(iterRaw) || 0);
  } catch (e) {}
  return facts;
}

function _cancelAnimation(nid, st) {
  if (st.endTimer != null) clearTimeout(st.endTimer);
  if (st.iterTimer != null) clearTimeout(st.iterTimer);
  _animTracked.delete(nid);
  _dispatchAnimationEvent(st.el, "animationcancel", st.name, st.startElapsed);
}

function _startAnimation(el, name, facts) {
  const f = facts || { duration: 0, delay: 0, infinite: false, iterations: 1 };
  const duration = f.duration, delay = f.delay, infinite = f.infinite, iterations = f.iterations;
  const startElapsed = delay < 0 ? -delay : 0;
  const st = { el, name, startElapsed, endTimer: null, iterTimer: null };
  _animTracked.set(el._nid, st);
  _dispatchAnimationEvent(el, "animationstart", name, startElapsed);
  // Iteration boundaries — the start of iteration k sits at delay + duration×k.
  // The chain re-arms one timer at a time, so an animating element holds at
  // most one pending iteration timer alongside its end timer: the same posture
  // as the end timer, and never a per-mutation tick. A 0s duration leaves no
  // representable boundary, so it stays unarmed rather than busy-spinning.
  if ((infinite || iterations > 1) && duration > 0) {
    const iterDelayMs = duration * 1000;
    const armIteration = (k, waitMs) => {
      st.iterTimer = setTimeout(() => {
        if (_animTracked.get(el._nid) !== st) return;
        st.iterTimer = null;
        _dispatchAnimationEvent(el, "animationiteration", name, Math.max(0, delay) + duration * k);
        if (infinite || k < iterations - 1) armIteration(k + 1, iterDelayMs);
      }, waitMs);
    };
    armIteration(1, (Math.max(0, delay) + duration) * 1000);
  }
  if (!infinite) {
    // End fires after the declared active duration (Chrome timing): that
    // also keeps a wall-clock window in which a cancel can still be
    // observed between start and end.
    const activeMs = (Math.max(0, delay) + duration * iterations) * 1000;
    st.endTimer = setTimeout(() => {
      if (_animTracked.get(el._nid) !== st) return;
      if (st.iterTimer != null) { clearTimeout(st.iterTimer); st.iterTimer = null; }
      _animTracked.delete(el._nid);
      // Detached before the duration ran out: the end never happened, the
      // removal did (Chrome fires animationcancel at detach; ours lands on
      // the next checkpoint via this timer).
      if (!_nodeInDocument(el)) {
        _dispatchAnimationEvent(el, "animationcancel", name, startElapsed);
        return;
      }
      _dispatchAnimationEvent(el, "animationend", name, Math.max(0, delay) + duration * iterations);
    }, activeMs);
  }
}

// One element: cancel a tracked animation whose name went away or changed,
// start a newly-observable one. Runs synchronously in the mutation.
// Per-epoch budget (#224): a MutationObserver-flooded page rebuilds subtrees
// every batch, and each candidate costs a full-cascade computed-style read
// (every tree write dropped the layout cache) — the animation sweep alone
// measured ~100% of a core on doudian's publish page. Past the cap, NEW
// animations defer to the next epoch (animationstart is async in Chrome too,
// and a flooding page bumps the epoch every batch, so the delay is a few ms);
// tracked elements still get their cancel/change handling unthrottled, and
// quiet pages never come near the cap — zero behavioral delta there.
let _animEpochSpent = -1, _animEpochBudget = 0, _animOverflowQueued = false;
const _animCappedTail = new Set();
const _ANIM_EPOCH_CAP = 512;
function _animationCheckOne(el) {
  if (!_nodeInDocument(el)) return;
  const existing = _animTracked.get(el._nid);
  if (_animEpochSpent !== _ditingMutationEpoch) {
    _animEpochSpent = _ditingMutationEpoch;
    _animEpochBudget = 0;
  }
  if (!existing && _animEpochBudget >= _ANIM_EPOCH_CAP) {
    // Park in a set the synchronous chain NEVER reads: pushing back into
    // _animDeferredChecks here would couple with checkIn's tail drain into a
    // drain↔checkIn mutual recursion on the capped element (stack overflow
    // at 600 inserts). The overflow macrotask merges it back with the budget
    // reset — a quiet page that batch-inserts past the cap still gets its
    // starts (nothing else will bump the epoch), and a flooding page is
    // throttled to ~one burst per tick with the loop yielded in between.
    _animCappedTail.add(el);
    if (!_animOverflowQueued) {
      _animOverflowQueued = true;
      setTimeout(() => {
        _animOverflowQueued = false;
        _animEpochBudget = 0;
        for (const e of _animCappedTail) _animDeferredChecks.add(e);
        _animCappedTail.clear();
        _drainDeferredAnimationChecks();
      }, 50);
    }
    return;
  }
  _animEpochBudget++;
  const facts = _animFactsOf(el);
  const name = facts.name;
  if (existing && existing.name !== name) _cancelAnimation(el._nid, existing);
  if (name && (!existing || existing.name !== name)) _startAnimation(el, name, facts);
}

// A page with no CSS anywhere cannot start an animation, but the check's
// computed-style read would still force a full layout run on every mutation
// batch — on style-less pages (deep-DOM fixtures, plain markup) that layout
// is the entire cost. The probe is cached per mutation epoch: <style>,
// <link>, [style] arrive or leave only through tree/attribute writes, which
// all bump the epoch, so the cache can't hide a newly-animatable page.
// Constructible stylesheets never reach the engine cascade (their rules are
// a JS-side shim) and WAAPI doesn't go through this path — both unaffected.
let _animCssProbeEpoch = -1, _animCssPossible = false, _animCssTransition = 0;
function _animationCssGate() {
  if (_animCssProbeEpoch === _ditingMutationEpoch) return _animCssPossible;
  const was = _animCssPossible;
  _animCssProbeEpoch = _ditingMutationEpoch;
  try {
    _animCssPossible = !!document.querySelector('style, link[rel~="stylesheet"], [style]');
  } catch (e) { _animCssPossible = true; }
  _animCssTransition = (!was && _animCssPossible) ? 1 : ((was && !_animCssPossible) ? -1 : 0);
  return _animCssPossible;
}

// Collect animation-capable elements (class or style attribute) from a
// subtree that just entered the document, plus the root itself. The cap
// bounds the per-element computed-style reads on big insertions.
function _animationCheckIn(root) {
  if (!root || root.nodeType !== 1 || !root.isConnected) return;
  if (_animScanDepth > 0) { _animDeferredChecks.add(root); return; }
  if (!_animationCssGate()) {
    // CSS just disappeared: anything still tracked lost its declarations —
    // cancel them the way Chrome does when its sheet is removed mid-run.
    if (_animTracked.size) _cancelAnimationsInSubtree(document.documentElement);
    return;
  }
  if (_animCssTransition > 0) {
    // CSS arrived after content was already checked through this gate
    // (async style injection): previously-skipped subtrees may animate now.
    // One capped whole-document sweep mirrors Chrome — animationstart
    // belongs to when the style lands, not when the element did. Riding the
    // depth guard defers it to the end of this very call.
    _animCssTransition = 0;
    if (root !== document.documentElement) {
      _animScanDepth = 1;
      try { _animationCheckIn(document.documentElement); } catch (e) {}
      _animScanDepth = 0;
      _drainDeferredAnimationChecks();
      return;
    }
  }
  _animScanDepth = 1;
  try {
    _animationCheckOne(root);
    let count = 1;
    for (const sel of ["[class]", "[style]"]) {
      const ids = _domParse("query_selector_all_scoped", root._nid, sel) || [];
      for (const id of ids) {
        if (count >= 128) break;
        const el = _wrapEl(+id);
        if (el) { _animationCheckOne(el); count++; }
      }
      if (count >= 128) break;
    }
  } catch (e) {}
  _animScanDepth = 0;
  _drainDeferredAnimationChecks();
}

// Attribute mutations concern the one element; subtree insertions the new
// root. Both are QUEUED, not run in the mutation: a script building a deep
// subtree via N appendChild calls would otherwise force N full layout runs
// (every tree write drops the layout cache, so each check's computed-style
// read re-lays-out the whole document — 2000 nested appends overflowed the
// stack inside layout). One microtask drain after the script's synchronous
// section does the work once on the final tree, which is also Chrome's own
// coalescing: style recalc and animationstart happen at the next rendering
// opportunity, never mid-task.
//
// Bounded slices (#224 CI face): the drain used to run the whole batch as
// ONE synchronous wall, and a big mount (600 animated inserts on a slow CI
// runner) blew past the eval pump's 500ms patience wall — the watchdog
// beheaded the drain mid-batch and every un-checked element was lost
// forever (nothing re-arms them; a data-* write never schedules checks) —
// exactly the starvation this family forbids. The sweep now yields every
// _ANIM_SWEEP_SLICE_MS of work and continues on a short timer, so no tick
// the pump waits on carries more than a slice plus one root's capped walk
// (<128 candidates) — comfortably inside every watchdog budget. The
// continuation timer only ever arms mid-burst on animation-heavy mounts
// (the same condition that arms the 50ms overflow macrotask below), NOT
// per mutation — the always-pending 0ms tick that broke nav settle never
// comes back.
const _animPendingChecks = new Set();
const _animSweepWork = [];
let _animCheckQueued = false, _animSweepContArmed = false;
let _animSweepIdx = 0; // committed cursor: work[0, idx) already checked
const _ANIM_SWEEP_SLICE_MS = 40;
// The sweep must survive being beheaded mid-slice: under heavy load even one
// root's full-cascade check can outwall the eval pump's 500ms patience, and
// the watchdog's terminate is uncatchable — it unwinds the slice wherever it
// stands. Two invariants make that harmless: the cursor commits BEFORE each
// check (a beheaded slice never re-runs an element — re-running a finished
// animation would restart it), and the next continuation timer is armed
// BEFORE the slice does its work (a beheaded slice always leaves a pending
// timer behind, so the batch resumes instead of starving). A stray 4ms no-op
// continuation per burst is the price; the timer only ever arms while sweep
// work is pending (animation-heavy mounts), never per mutation.
function _maybeArmSweepCont() {
  if (_animSweepContArmed || _animSweepIdx >= _animSweepWork.length) return;
  _animSweepContArmed = true;
  setTimeout(_drainSweepSlice, 4);
}
function _drainSweepSlice() {
  _animSweepContArmed = false;
  _maybeArmSweepCont();
  const t0 = Date.now();
  while (_animSweepIdx < _animSweepWork.length) {
    _animationCheckIn(_animSweepWork[_animSweepIdx++]);
    if (_animSweepIdx < _animSweepWork.length && Date.now() - t0 > _ANIM_SWEEP_SLICE_MS) break;
  }
  if (_animSweepIdx >= _animSweepWork.length) { _animSweepWork.length = 0; _animSweepIdx = 0; }
}
function _scheduleAnimationCheck(root) {
  if (!root || root.nodeType !== 1) return;
  _animPendingChecks.add(root);
  if (_animCheckQueued) return;
  _animCheckQueued = true;
  Promise.resolve().then(() => {
    _animCheckQueued = false;
    if (_animPendingChecks.size) {
      for (const r of _animPendingChecks) _animSweepWork.push(r);
      _animPendingChecks.clear();
    }
    // First slice runs INLINE in the seeding microtask, not on the 4ms
    // continuation timer: a small batch (the common case) lands its starts in
    // the same tick as the mutation, so the sweep costs no extra pump
    // round-trip. _drainSweepSlice pre-arms the continuation before working,
    // so a big batch — or a beheaded inline slice — still resumes on the
    // timer with the same invariants.
    _drainSweepSlice();
    // Transition diffs ride the same drain (one recompute per batch, the
    // elements already warm from the animation pass). Attribute-write
    // callers only — subtree insertions must not arm diffs.
    const tb = Array.from(_transPending);
    _transPending.clear();
    for (const el of tb) {
      try { _transitionCheckOne(el); } catch (e) {}
    }
  });
}

function _drainDeferredAnimationChecks() {
  if (_animDeferredChecks.size === 0) return;
  const pending = Array.from(_animDeferredChecks);
  _animDeferredChecks.clear();
  for (const el of pending) _animationCheckIn(el);
}

function _animNodeInSubtree(el, root) {
  let n = el;
  while (n) {
    if (n === root) return true;
    n = n.parentNode;
  }
  return false;
}

// Detachment cancels (Chrome semantics); _animTracked only ever holds
// elements that actually animated, so the walk is trivially small.
function _cancelAnimationsInSubtree(root) {
  if (!root || root.nodeType !== 1) return;
  for (const [nid, st] of Array.from(_animTracked)) {
    if (_animNodeInSubtree(st.el, root)) _cancelAnimation(nid, st);
  }
}

// --- CSS transition lifecycle (v1) ---------------------------------------
// A transition is defined by a *diff* between before/after computed values,
// which the engine's sampler cannot see — it only overrides the cascade at
// css_time. Trigger detection therefore lives here: every style/class
// attribute write queues the element, and the same microtask that drains
// animation checks diffs each queued element against its last observed
// values. One recompute per mutation batch (amortized, epoch-cached) — the
// same coalescing posture as animationstart; a per-write synchronous read
// was rejected because it forces a full cascade on every class write during
// hydration. Elements never observed before cannot diff, which matches
// Chrome: no transition on an element's first style resolution. v1 watches
// opacity/color/background-color/transform; CSSOM rule mutations surface
// on the next observation rather than at the rule change. transitioncancel
// fires the two ways Chrome fires it: a re-trigger on the same property
// preempts the in-flight entry, and removing the element from the document.
const _transWatched = ["opacity", "color", "background-color", "transform"];
const _transLast = new WeakMap();  // el -> {opacity, color, background-color, transform}
const _transPending = new Set();
const _transTracked = new Map();   // nid -> { el, timers: Map(prop -> {run,start,end}) }

function _dispatchTransitionEvent(el, type, prop, elapsed) {
  try {
    el.dispatchEvent(new TransitionEvent(type, {
      bubbles: true, composed: true,
      propertyName: prop, elapsedTime: elapsed, pseudoElement: "",
    }));
  } catch (e) {}
}

// Seconds the transition had been running when it was canceled (Chrome
// fills transitioncancel.elapsedTime with the active duration so far; a
// cancel during the delay window reports 0).
function _transElapsed(t) {
  if (t.startWall == null) return 0;
  return Math.max(0, (Date.now() - t.startWall) / 1000);
}

function _armTransitionEvents(el, prop, duration, delay) {
  let tr = _transTracked.get(el._nid);
  if (!tr) { tr = { el, timers: new Map() }; _transTracked.set(el._nid, tr); }
  const old = tr.timers.get(prop);
  if (old) {
    for (const t of [old.run, old.start, old.end]) if (t != null) clearTimeout(t);
    _dispatchTransitionEvent(el, "transitioncancel", prop, _transElapsed(old));
  }
  const t = { run: null, start: null, end: null, startWall: null };
  tr.timers.set(prop, t);
  _dispatchTransitionEvent(el, "transitionrun", prop, 0);
  const fireStart = () => {
    if (_transTracked.get(el._nid) !== tr || tr.timers.get(prop) !== t) return;
    t.startWall = Date.now();
    _dispatchTransitionEvent(el, "transitionstart", prop, 0);
  };
  if (delay > 0) t.start = setTimeout(fireStart, delay * 1000);
  else fireStart();
  t.end = setTimeout(() => {
    if (_transTracked.get(el._nid) !== tr || tr.timers.get(prop) !== t) return;
    tr.timers.delete(prop);
    if (!tr.timers.size) _transTracked.delete(el._nid);
    if (!_nodeInDocument(el)) return;
    _dispatchTransitionEvent(el, "transitionend", prop, duration);
  }, (delay + duration) * 1000);
}

// Detachment cancels a running transition (Chrome semantics): every
// pending timer in the removed subtree fires transitioncancel and its
// tracking entry goes away, exactly like animationcancel above.
function _cancelTransitionsInSubtree(root) {
  if (!root || root.nodeType !== 1) return;
  for (const [nid, tr] of Array.from(_transTracked)) {
    if (!_animNodeInSubtree(tr.el, root)) continue;
    for (const [prop, t] of Array.from(tr.timers)) {
      for (const k of [t.run, t.start, t.end]) if (k != null) clearTimeout(k);
      _dispatchTransitionEvent(tr.el, "transitioncancel", prop, _transElapsed(t));
    }
    _transTracked.delete(nid);
  }
}

function _transitionCheckOne(el) {
  if (!el || el.nodeType !== 1 || el._nid == null) return;
  if (!_nodeInDocument(el)) return;
  let snap = null;
  try { snap = getComputedStyle(el); } catch (e) { return; }
  const now = {};
  for (const p of _transWatched) now[p] = snap.getPropertyValue(p);
  const prev = _transLast.get(el);
  if (prev) {
    const durations = String(snap.getPropertyValue("transition-duration") || "").split(",").map(_cssSeconds);
    let durMax = 0;
    for (const d of durations) if (d > durMax) durMax = d;
    if (durMax > 0) {
      const props = String(snap.getPropertyValue("transition-property") || "all").split(",").map((s) => s.trim());
      const delays = String(snap.getPropertyValue("transition-delay") || "").split(",").map(_cssSeconds);
      const easings = String(snap.getPropertyValue("transition-timing-function") || "ease").split(",").map((s) => s.trim());
      for (const p of _transWatched) {
        if (String(now[p]) === String(prev[p])) continue;
        let k = props.indexOf(p);
        if (k < 0) k = props.indexOf("all");
        if (k < 0) continue;
        const duration = durations[k % durations.length] || 0;
        const delay = delays[k % delays.length] || 0;
        if (duration <= 0 && delay <= 0) continue;
        try {
          _dom("add_css_transition", String(el._nid), JSON.stringify({
            prop: p, from: prev[p], to: now[p],
            duration: duration * 1000, delay: delay * 1000,
            easing: easings[k % easings.length] || "ease",
          }));
        } catch (e) { continue; }
        _armTransitionEvents(el, p, duration, delay);
      }
    }
  }
  _transLast.set(el, now);
}

function _scheduleTransitionCheck(el) {
  if (!el || el.nodeType !== 1) return;
  _transPending.add(el);
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_cancelAnimationsInSubtree', _cancelAnimationsInSubtree);
  d('_cancelTransitionsInSubtree', _cancelTransitionsInSubtree);
  d('_dispatchAnimationEvent', _dispatchAnimationEvent);
  d('_scheduleAnimationCheck', _scheduleAnimationCheck);
  d('_scheduleTransitionCheck', _scheduleTransitionCheck);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
