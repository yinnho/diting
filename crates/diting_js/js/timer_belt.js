(function () { // @@SCOPE
// god-file split (刀54, timer wheel + macrotask trampoline belt): the whole
// event-loop machinery bootstrap grew around the timer/task boundary — the
// #38 sorted timer heap (one armed op_sleep at a time, patrol safety net),
// the #30 macrotask trampoline landing deliveries at task boundaries never
// mid-dispatch, the #39/#227 stall predicates + recover hook the engine
// pumps, the #240 fetch post-mortem ring, and the setTimeout/setInterval/
// rAF/queueMicrotask faces. Moved verbatim from bootstrap; every cross-file
// use is a runtime read of published hidden globals (bootstrap installs
// _OPS and the error reporter before this file loads). Native marking for
// the timer faces migrated here — bootstrap loads first and would mark
// undefined (刀22 precedent).
let _tid = 0;
const _clearedTimers = new Set();
const _intervals = new Set();
// HTML spec timer nesting: a chain of timers each scheduled from inside a
// timer callback gets its delay floored at 4ms after 5 nesting levels.
// Without the floor a setInterval(fn, 1) self-chain runs at op_sleep
// resolution (~450/s), starving every other macrotask on the event loop —
// doudian's publish page ships exactly such an interval in its telemetry SDK
// and rendered permanently blank (#203).
let _timerNesting = 0;
const _TIMER_FLOOR_MS = 4;

// #38, 2026-10-05: timers no longer get one tokio sleep each. Same-deadline
// sleeps complete in the OP DRIVER's order, not scheduling order — the wheel
// holds a slot as a LIFO stack, the completion FuturesUnordered wakes onto
// another LIFO, and deno_core delivers results 128 per event-loop tick, so
// the two reversals only cancel for small quiet batches. Live on an fxg boot
// storm (1500 same-delay timers): 0..127,309..182,310..437,... — dozens of
// 128-item chunks, some ascending, some descending. HTML's timer task source
// is ordered: same-deadline timers run in scheduling order. The fix is the
// shape every real browser uses — one sorted timer heap, ordered by
// (deadline, sequence), with outstanding sleeps as nothing more than wakeups.
// An op sleep cannot be cancelled, so arming an earlier timer may leave a
// stale sleep in flight; a stale wake (its deadline no longer the armed
// minimum) is a no-op, and the arm policy (`e.d < armed`) keeps at most one
// sleep covering the true earliest deadline.
const _timerHeap = [];
let _timerSeq = 0;
let _timerArmed = Infinity;
const _timerLess = (a, b) => a.d < b.d || (a.d === b.d && a.s < b.s);
const _timerPush = (e) => {
  const h = _timerHeap;
  h.push(e);
  let i = h.length - 1;
  while (i > 0) {
    const p = (i - 1) >> 1;
    if (!_timerLess(h[i], h[p])) break;
    const t = h[i]; h[i] = h[p]; h[p] = t;
    i = p;
  }
};
const _timerPop = () => {
  const h = _timerHeap;
  const top = h[0];
  const last = h.pop();
  if (h.length) {
    h[0] = last;
    let i = 0;
    for (;;) {
      const l = 2 * i + 1, r = l + 1;
      let m = i;
      if (l < h.length && _timerLess(h[l], h[m])) m = l;
      if (r < h.length && _timerLess(h[r], h[m])) m = r;
      if (m === i) break;
      const t = h[i]; h[i] = h[m]; h[m] = t;
      i = m;
    }
  }
  return top;
};
const _timerWake = (d) => {
  if (d !== _timerArmed) { _wheelStale++; return; } // stale: a sleep for a superseded deadline
  _wheelWakes++;
  _timerArmed = Infinity;
  // Fixed snapshot: entries whose f() enqueues more timers during this loop
  // must not extend the pop window mid-wake.
  const now = Date.now();
  let e;
  while ((e = _timerHeap[0]) && e.d <= now) {
    _timerPop();
    // #240: e.f() -> _runAsMacrotask -> _mtArm makes op calls; a sync
    // throw here would abort the pop loop and skip the re-arm below,
    // leaving the remaining due entries rotting with armed === Infinity.
    try { e.f(); } catch (err) { _wheelThrows++; globalThis.__diting_reportUncaughtError(err); }
  }
  _timerArmNext();
};
// #39 discipline: termination unwinds pending ops WITHOUT settling them, so
// the armed sleep can vanish and the earliest deadline would rot while
// `armed` claims coverage. Same cure as the trampoline patrol — a slow
// sleep that re-delivers anything already past deadline.
// #240: the patrol had two silent-death holes of its own. (1) Its reject
// arm was `() => {}` — a rejected patrol sleep left `_timerPatrol`
// latched true, so no new patrol could ever arm. (2) A completion that
// evaporates entirely (severed batch) ALSO leaves the flag latched, and
// `_timerArmed` keeps the now-stale deadline, so `_timerArmNext`'s
// `e.d >= _timerArmed` gate refuses every later timer — observed live on
// a doudian publish run (2026-10-09): fetch callbacks and eval-await
// fine, every setTimeout/setInterval/rAF dead for the rest of the
// session, trampoline idle-healthy. Both rejection arms now run the
// patrol fn itself (idempotent: unlatch, wake due, re-arm), and the
// engine's recover hook force-heals the wheel (see below).
let _wheelWakes = 0;     // wake callbacks that matched the armed deadline
let _wheelStale = 0;     // wake callbacks no-opped as superseded
let _wheelThrows = 0;    // sync throws out of op_sleep calls (must not half-commit)
let _wheelPatrols = 0;   // patrol passes run
// #240 face-2 postmortem ring: the last few fetch lifecycle stages, so a
// fetch whose await continuation vanishes under load names the stage it
// died at instead of leaving "outer resolved, continuation never ran" as
// a four-way contradiction. Stages: armed (registry set), opok/operr (the
// op promise settled), takeok/takerej (heartbeat-probe recovery path),
// resume (await _outer continuation ran), resp (Response constructed),
// fthrow (rethrow after a failed await). [tag, key, Date.now()%1e5].
let _fetchRing = [];
function _fetchStage(tag, fk) {
  _fetchRing.push([tag, fk, Date.now() % 100000]);
  if (_fetchRing.length > 64) _fetchRing.shift();
}
let _timerPatrol = false;
// #71: one patrol chain per wheel, enforced by generation — same contract as
// the macrotask chain's _mtGen. Every path that drops the latch (the recover
// hook, a rejected wake sleep) then arms a fresh chain while the previous
// one's 1500ms sleep is still in flight, and that old chain re-armed itself
// forever: an idle x.com serve ended up with ~1200 parallel chains (47.8M
// patrols in 16h, ~800/s). A superseded chain now dies on its next wake.
let _timerPatrolGen = 0;
const _timerArmPatrol = () => {
  _timerPatrol = true;
  const gen = ++_timerPatrolGen;
  const next = () => _timerPatrolFn(gen);
  _OPS.op_sleep(1500).then(next, next);
};
const _timerPatrolFn = (gen) => {
  if (gen !== undefined && gen !== _timerPatrolGen) return;
  _timerPatrol = false;
  _wheelPatrols++;
  const e = _timerHeap[0];
  if (e && e.d <= Date.now()) {
    if (_timerArmed !== Infinity) _timerWake(_timerArmed);
    else _timerArmNext();
  }
  if (_timerHeap.length && !_timerPatrol) _timerArmPatrol();
};
const _timerArmNext = () => {
  const e = _timerHeap[0];
  if (!e || e.d >= _timerArmed) return;
  const prev = _timerArmed;
  _timerArmed = e.d;
  try {
    if (!_timerPatrol && _timerHeap.length) _timerArmPatrol();
    const wait = Math.max(0, e.d - Date.now());
    _OPS.op_sleep(wait).then(() => _timerWake(e.d), () => { _timerPatrolFn(); });
  } catch (err) {
    // The armed deadline was committed above; an op call that throws
    // synchronously would leave it claiming coverage with no sleep in
    // flight — the #240 corpse's exact state. Revert to the previous
    // coverage so the next schedule retries the arm instead of rotting
    // behind the gate.
    _wheelThrows++;
    _timerArmed = prev;
    throw err;
  }
};

const _scheduleAfter = (delay, level, fn) => {
  const d0 = Math.max(0, Number(delay) || 0);
  // Per HTML's timer-initialisation steps, a timer whose nesting level is
  // greater than 4 has its timeout clamped to 4ms. The level travels with the
  // CHAIN — a timer scheduled from inside a level-N timer task is level N+1 —
  // so reading a live counter at fire time would see it restored between
  // fires and never exceed 1; it must be captured at schedule time.
  const d = level > 4 ? Math.max(_TIMER_FLOOR_MS, d0) : d0;
  // setTimeout(0) must be a MACROtask: it runs on a later event-loop turn,
  // after the current task's entire microtask queue has drained — the same
  // task boundary a real browser provides. Delivering it as a microtask
  // breaks every scheduler built on the distinction (React 19's Scheduler
  // defers work through MessageChannel/setTimeout specifically to land on
  // that boundary), so even delay-0 timers round-trip through op_sleep:
  // a resolved async op is the cheapest true event-loop turn the runtime
  // offers.
  const o = Date.now() + d;
  _timerPush({ d: _timerAlign(o, level), o, l: level, s: _timerSeq++, f: fn });
  _timerArmNext();
};

// #70 background budget, Chrome's hidden-tab policy (the engine sets the
// level from how long the session has gone without a command; visibility
// itself is not touched — pages and flows key behaviour off it): level 1
// aligns every timer wake-up to the next whole second; level 2 (Chrome's
// intensive wake-up throttling, 5 more minutes on) aligns chained timers —
// nesting level > 4 — to the next whole minute. An unwatched x.com session
// otherwise ran at foreground speed for 16h (1.55M fetches, 1270
// macrotasks/s). Entries keep their original deadline `o`, so a level
// change re-derives every pending deadline: a command arriving after an
// idle stretch gets its timers back on time instead of waiting out a
// minute boundary.
let _timerBackground = 0;
const _timerAlign = (o, level) => {
  if (_timerBackground === 0) return o;
  const step = _timerBackground >= 2 && level > 4 ? 60000 : 1000;
  return Math.ceil(o / step) * step;
};
globalThis.__diting_set_background = (lv) => {
  const level = lv | 0;
  if (level === _timerBackground) return;
  _timerBackground = level;
  if (!_timerHeap.length) return;
  for (const e of _timerHeap) if (e.o !== undefined) e.d = _timerAlign(e.o, e.l);
  // A sorted array is a valid min-heap under the same order.
  _timerHeap.sort((a, b) => (_timerLess(a, b) ? -1 : _timerLess(b, a) ? 1 : 0));
  // The armed sleep may now be for a deadline that moved: drop the gate so
  // the next arm covers the new top (the old sleep wakes stale and no-ops).
  _timerArmed = Infinity;
  _timerArmNext();
};

// A timer/message delivery must land at a TASK BOUNDARY, never at a
// microtask checkpoint inside another task's synchronous window. The
// op_sleep continuation a MessageChannel post resolves to is a microtask:
// it fires at the next checkpoint, which on heavy pages is routinely
// INSIDE a synchronous event dispatch — React 16's scheduler then runs
// performSyncWorkOnRoot while CommitContext is set and throws #327
// ("This root is already working"), after which the root stops committing
// anything: every setState queues forever and the page looks frozen
// (taobao publish form, 2026-10-04). Browsers get this discipline free
// from the task queue; here the delivery itself re-arms until the
// dispatch/timer window it landed in has closed.
//
// #30, 2026-10-04: the first cut re-armed as `_OPS.op_sleep(0).then(attempt)`
// per checkpoint. A synchronous handler that itself makes many op calls
// triggers a checkpoint PER OP, each spawning another armed chain — fan-out,
// not deferral: fxg.jinritemai.com burned a full core re-arming ~320×/s
// (Promise.then counter: 2560 calls in 8s, all named `attempt`), starving
// the very event loop the trampoline was supposed to protect. The fix:
// exactly ONE armed chain (the `_mtArmed` gate), a FIFO the chain drains
// one item per turn, and a backoff past 8 consecutive defers so an
// op-heavy window cannot turn waiting into spinning. `_mtDepth` now also
// brackets the page's own synchronous dispatch loops (`_mtWindow`), so a
// delivery that would land mid-dispatchEvent defers instead — which is the
// #327 case the timer-only guard missed.
// #37, 2026-10-04: the drain is an async loop that yields to a microtask
// checkpoint between items (a browser drains microtasks after EVERY task) and
// to one true op turn every 64 items so the Rust side can deliver the op
// completions (fetch bodies, timer wheels) that feed the queue.
// #39, 2026-10-05: the #37 escape hatch was PUSH-COUNTED (retire after 1000
// stale pushes) — it needs the page to keep pushing. fxg's telemetry is a
// self-chained setTimeout: once the chain dies its callbacks stop running,
// so nothing ever schedules the next push. Observed live on a wedged create
// page: {qlen:160819, stale:0, armed:true} — 160k payloads rotting with the
// staleness counter frozen at zero, no re-arm ever. Healing now rests on a
// timestamp instead: the drain stamps every loop iteration, a push retires
// a chain that has made no progress for 250ms, and every arm launches an
// independent patrol sleep that heals an armed-but-silent chain even on a
// page with zero pushes. A patrol from generation N dies on any later arm,
// so retries cannot accumulate.
let _mtDepth = 0;
let _mtArmed = false;
const _mtQueue = [];
let _mtHead = 0;   // consume cursor: shift() memmoves the whole backing store, O(n) per item
const _mtQLen = () => _mtQueue.length - _mtHead;
const _mtCompact = () => { if (_mtHead && _mtHead >= _mtQueue.length - _mtHead) { _mtQueue.splice(0, _mtHead); _mtHead = 0; } };
let _mtDefers = 0;
let _mtBeats = 0;      // every drain-loop iteration
let _mtItems = 0;      // every payload executed (cursor read)
let _mtRearms = 0;     // every arm of a fresh chain
let _mtGen = 0;        // bumped to retire a chain that stopped progressing
let _mtStale = 0;      // pushes since an item last ran (diagnostics only)
let _mtLastBeat = 0;   // last drain-loop iteration (chain liveness timestamp)
const _mtDrain = async () => {
  const gen = _mtGen;
  let tick = 0;
  for (;;) {
    if (gen !== _mtGen) return;   // retired: a fresher chain owns the queue
    _mtLastBeat = Date.now();
    if (_mtDepth > 0) {
      _mtDefers++;
      await _OPS.op_sleep(_mtDefers > 8 ? 4 : 0);
      continue;
    }
    _mtDefers = 0;
    if (_mtQLen() === 0) { _mtArmed = false; _mtCompact(); return; }
    _mtDepth++;
    _mtItems++;
    _mtStale = 0;
    const _fn = _mtQueue[_mtHead++];
    try { _fn(); }
    catch (e) { globalThis.__diting_reportUncaughtError(e); }
    finally { _mtDepth--; }
    _mtBeats++;
    await Promise.resolve();
    if ((++tick & 63) === 0) await _OPS.op_sleep(0);
  }
};
const _mtArm = () => {
  _mtStale = 0;
  _mtArmed = true;
  _mtRearms++;
  _mtGen++;
  _mtLastBeat = Date.now();
  const gen = _mtGen;
  // The arm's op_sleep continuation can vanish (interrupt unwinds pending
  // ops without settling them); swallow a rejection the same way — the
  // patrol below is what recovers, never this .then itself.
  _OPS.op_sleep(0).then(_mtDrain, () => {});
  _OPS.op_sleep(1500).then(() => {
    if (_mtArmed && gen === _mtGen && Date.now() - _mtLastBeat > 1200) {
      _mtArm();
    }
  }, () => {});
};
const _runAsMacrotask = (fn) => {
  _mtQueue.push(fn);
  _mtStale++;
  // `armed` alone can lie about liveness: a chain whose op continuations
  // vanished stays armed forever. The timestamp is the truth — a healthy
  // chain iterates (draining or deferring) every few ms, so one push more
  // than 250ms after the last iteration is a dead chain. Retire and re-arm.
  if (_mtArmed && Date.now() - _mtLastBeat < 250) return;
  _mtArm();
};
// Bracket a synchronous dispatch window (listener loops): deliveries
// armed while the window is open defer until after it closes.
const _mtWindow = (body) => {
  _mtDepth++;
  try { return body(); } finally { _mtDepth--; }
};
// #39: terminate_execution unwinds the JS stack WITHOUT running finally
// blocks, so a bracket open at the fire (a payload or a dispatch window)
// leaks _mtDepth>0 forever and every later drain defers into infinity
// while looking alive (observed: defers climbing, qlen rotting). The
// engine calls this from its termination recovery points, where nothing
// JS is executing and any positive depth is necessarily leaked.
// #227: also re-arm when a backlog waits — recovery points are the one
// place that can retire a corpse chain, and a quiet page (its own timers
// died with the chain) never pushes to do it from _runAsMacrotask.
// #240: the timer wheel needs the same heal. A severed completion (or a
// latched patrol flag) leaves `_timerArmed` holding a stale deadline, so
// `_timerArmNext`'s `e.d >= _timerArmed` gate refuses every later timer
// — the wheel has NO self-heal in that state: new pushes don't retry the
// arm, the patrol flag blocks the patrol, and the page's every
// timer/interval/rAF rots for the rest of the session while fetches and
// evals stay fine. Unlatch the patrol and force a fresh arm for whatever
// is actually pending.
// #63: the re-arm alone still loses under SUSTAINED overruns. The fresh
// arm is itself a severable op_sleep, so a behead cadence faster than the
// wake latency severs each heal's arm again — ping-pong, no forward
// progress (synthetic storm, 2/3 runs: victim timer 1.28s past due with
// pat=7/sw=3/wk=2 riding the window; production face = EVAL_TIMEOUT at
// the 5s budget). This point is the one moment the engine guarantees no
// JS is executing, so the recover now DELIVERS the due heap entries
// synchronously — same shape as `_timerWake`'s pop loop, one guaranteed
// step of forward progress per fire instead of racing the next one.
let _wheelRecoverDelivers = 0; // due entries delivered by the recover hook
globalThis.__diting_mt_recover_termination = () => {
  _mtDepth = 0; __probeInflightFetches();
  // #224: the animation sweep's guards leak the same way as _mtDepth.
  try { _animHealAfterBehead(); } catch (_) {}
  if (_mtQLen()) _mtArm();
  _timerPatrol = false;
  if (_timerHeap.length) {
    _timerArmed = Infinity;
    const now = Date.now();
    let e;
    while ((e = _timerHeap[0]) && e.d <= now) {
      _timerPop();
      _wheelRecoverDelivers++;
      try { e.f(); } catch (err) { _wheelThrows++; globalThis.__diting_reportUncaughtError(err); }
    }
    _timerArmNext();
  }
};
// #227 stall predicate for the engine pump — the only witness independent
// of JS continuations. A terminate severs the drain's op_sleep
// continuation AND its sibling 1500ms patrol together (the completion
// batch evaporates, cf. #226), and a quiet page pushes nothing, so the
// 250ms retire never fires: armed corpse, backlog rotting (observed live:
// qlen 160819 with stale:0). Healthy chains beat every few ms (draining)
// or every ~4ms (deferring inside a dispatch window); 600ms of silence
// with work queued can only be a corpse.
globalThis.__diting_mt_stalled = () =>
  _mtArmed && _mtQLen() > 0 && Date.now() - _mtLastBeat > 600;
// #240 wheel stall predicate, same witness contract: a heap-top past due
// with a wheel that has STOPPED (stale armed gate + latched patrol) is a
// corpse — the pump force-recovers, which re-arms the wheel. #71: overdue
// alone is not a corpse. On a loaded host a live wheel runs 600-700ms
// behind for hours (wakes still climbing every look), and calling that a
// stall fired the recover — a synchronous delivery of every due entry —
// plus a multi-KB post-mortem line ~1/s at peak, piling load onto the very
// lag that tripped it (20699 hits in 16h on an idle x.com serve). So a
// corpse is overdue AND no wheel progress for 1.5s across looks.
let _wheelSeenProgress = -1, _wheelSeenAt = 0;
globalThis.__diting_wheel_stalled = () => {
  const e = _timerHeap[0];
  const now = Date.now();
  if (!e || e.d > now - 600) { _wheelSeenProgress = -1; return false; }
  const progress = _wheelWakes + _wheelRecoverDelivers;
  if (progress !== _wheelSeenProgress) {
    _wheelSeenProgress = progress;
    _wheelSeenAt = now;
    return false;
  }
  return now - _wheelSeenAt > 1500;
};
// #240 face-2 detector: a fetch whose op settled (opok/operr — the outer
// promise resolved or rejected) but whose chained .then continuation
// (resume/fthrow) never ran. On a healthy realm the two are one microtask
// checkpoint apart — the chain callback drains in the SAME turn that ran
// opok — so a multi-second gap is impossible while the engine still pumps
// (evals drive the loop). 8s tolerates even a slow busy window between the
// two stages. The engine's loop-entry witness polls this; on a hit the
// dump below rides the warn line, so an unattended flow wedge names the
// stage it died at without a human eval.
globalThis.__diting_fetch_stalled = () => {
  const now = Date.now() % 100000;
  // Date.now()%1e5 wraps every 100s; age math must survive the wrap.
  const age = (t) => (t <= now ? now - t : now + 100000 - t);
  const seen = new Set();
  let stalled = false;
  for (let i = _fetchRing.length - 1; i >= 0; i--) {
    const [tag, fk, t] = _fetchRing[i];
    if (seen.has(fk)) continue;
    seen.add(fk);
    if ((tag === 'opok' || tag === 'operr' || tag === 'takeok') && age(t) > 8000
        && !_fetchStallReported.has(fk)) {
      // Latch per key: a lost continuation rots in the ring forever, and
      // the witness polls at every loop entry — without the latch the dump
      // would fire once per pump for the rest of the session. A NEW
      // stalled key still reports.
      _fetchStallReported.add(fk);
      stalled = true;
    }
    // armed/resp/resume/fthrow as the last stage of a key is a steady state
    // (in flight / completed) — only a settle-without-continuation rots.
  }
  return stalled;
};
const _fetchStallReported = new Set();
// Post-mortem bundle the witness dumps on any stall (#240): wheel counters,
// the fetch ring tail, and the in-flight key count, one JSON line.
globalThis.__diting_mt_dump = () => JSON.stringify({
  mt: globalThis.__MT_STATE ? __MT_STATE() : null,
  inflight: (typeof __inflightFetches !== 'undefined') ? __inflightFetches.size : null,
  fetchTail: _fetchRing.slice(-16),
});
// Debug probe for the trampoline's live state (#30 follow-up: macrotask
// deliveries dying while op+microtask paths stay alive — 2026-10-04 fxg).
// Read-only snapshot; keep names short, it rides on globalThis.
// #240: `wheel` exposes the timer wheel's state machine — the counters
// name the kill when a corpse re-forms (wakes/stale tell whether sleeps
// deliver, throws whether op calls fired synchronously, patrols whether
// the safety net lived). #63: rd counts due entries the recover hook had
// to deliver synchronously — under sustained beheads it is the only
// channel that still makes progress (wakes die with each severed arm).
globalThis.__MT_STATE = () => ({
  depth: _mtDepth, armed: _mtArmed, qlen: _mtQLen(), defers: _mtDefers,
  beats: _mtBeats, items: _mtItems, rearms: _mtRearms,
  gen: _mtGen, stale: _mtStale, lag: Date.now() - _mtLastBeat,
  wheel: {
    heap: _timerHeap.length,
    armedIn: _timerArmed === Infinity ? null : _timerArmed - Date.now(),
    patrol: _timerPatrol,
    wakes: _wheelWakes, staleWakes: _wheelStale,
    throws: _wheelThrows, patrols: _wheelPatrols,
    recovers: _wheelRecoverDelivers,
  },
});

// Per HTML, a string timer handler is compiled and run as a classic script in
// global scope *at fire time*. Indirect eval ((0, eval)) runs in the true
// global scope, so top-level var/function declarations become globals (a
// `new Function(fn)` wrapper keeps them local); deferring to fire time also
// surfaces a SyntaxError when the timer elapses, matching a real browser,
// instead of swallowing it eagerly at scheduling (upstream #558). Function
// handlers pass through unchanged.
const _coerceTimerFn = (fn) => {
  if (typeof fn === "string") {
    const src = fn;
    return () => { (0, eval)(src); };
  }
  return typeof fn === "function" ? fn : null;
};

globalThis.setTimeout = (fn, delay = 0, ...args) => {
  const handler = _coerceTimerFn(fn);
  if (!handler) return ++_tid;
  const id = ++_tid;
  const level = _timerNesting + 1;
  _scheduleAfter(delay, level, () => {
    // The cleared check must live INSIDE the delivered payload: the
    // trampoline runs it one event-loop turn after this callback, and a
    // clearTimeout that lands in that window must still win.
    _runAsMacrotask(() => {
      if (_clearedTimers.has(id)) return;
      const prev = _timerNesting;
      _timerNesting = level;
      try { handler(...args); } catch(e) { globalThis.__diting_reportUncaughtError(e); }
      finally { _timerNesting = prev; }
    });
  });
  return id;
};

globalThis.clearTimeout = (id) => { _clearedTimers.add(id); };

globalThis.setInterval = (fn, delay = 0, ...args) => {
  const handler = _coerceTimerFn(fn);
  if (!handler) return ++_tid;
  const id = ++_tid;
  const base = Math.max(0, Number(delay) || 0);
  _intervals.add(id);
  let fires = 0;
  const tick = () => {
    if (!_intervals.has(id)) return;
    fires++;
    _runAsMacrotask(() => {
      // Same one-turn window as setTimeout: clearInterval called after
      // this payload queued but before it ran must still suppress the
      // tick (the string-handler interval test catches exactly this).
      if (!_intervals.has(id)) return;
      const prev = _timerNesting;
      _timerNesting = fires;
      try { handler(...args); } catch(e) { globalThis.__diting_reportUncaughtError(e); }
      finally { _timerNesting = prev; }
    });
    if (!_intervals.has(id)) return;
    // An interval's self-chain keeps counting as nested, so its cadence
    // floors at the nesting limit after the first few fires — matching how
    // Chrome tames setInterval(fn, 1).
    _scheduleAfter(base, fires + 1, tick);
  };
  _scheduleAfter(base, 1, tick);
  return id;
};

globalThis.clearInterval = (id) => { _intervals.delete(id); _clearedTimers.add(id); };
globalThis.requestAnimationFrame = (fn) => __diting_raf(fn);
globalThis.webkitRequestAnimationFrame = globalThis.requestAnimationFrame;
globalThis.cancelAnimationFrame = (id) => __diting_caf(id);
globalThis.queueMicrotask = globalThis.queueMicrotask || ((fn) => Promise.resolve().then(fn));

// 刀54: native marking for the faces installed above, migrated from
// bootstrap's mark list (mark where installed — 刀22 precedent).
const _markNative = globalThis.__diting_markNative;
_markNative(globalThis.requestAnimationFrame);
_markNative(globalThis.cancelAnimationFrame);
_markNative(globalThis.setTimeout);
_markNative(globalThis.clearTimeout);
_markNative(globalThis.setInterval);
_markNative(globalThis.clearInterval);
_markNative(globalThis.queueMicrotask);

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  a('_fetchRing', () => _fetchRing, (v) => { _fetchRing = v; });
  d('_fetchStage', _fetchStage);
  a('_mtDepth', () => _mtDepth, (v) => { _mtDepth = v; });
  d('_mtQueue', _mtQueue);
  d('_mtWindow', _mtWindow);
  d('_runAsMacrotask', _runAsMacrotask);
  d('_scheduleAfter', _scheduleAfter);
  d('_timerArmNext', _timerArmNext);
  a('_timerArmed', () => _timerArmed, (v) => { _timerArmed = v; });
  d('_timerHeap', _timerHeap);
  a('_timerPatrol', () => _timerPatrol, (v) => { _timerPatrol = v; });
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
