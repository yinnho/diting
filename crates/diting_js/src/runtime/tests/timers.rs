//! timer wheel/macrotask/watchdog/busy freeze/heap guard
use super::super::*;
use super::common::setup_runtime;

/// Claim step 10d (docs/diting_runtime认领计划_20261017.md): the settle
/// pipeline is OURS — Rust SettleQueue + `__diting_settle` + the promise
/// ring baked by the PRELUDE replaced js/core_base.js's tick overlay
/// (which died with the deno host). Page JS can't see any of it (the
/// Deno global is deleted per page init), so this pins the settlement
/// path end-to-end: setTimeout rides an async op whose promise resolves
/// through OUR loop, and the retired core_base claim marker stays
/// absent.
#[tokio::test(flavor = "current_thread")]
async fn claimed_event_loop_tick_face() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    await new Promise(r => setTimeout(r, 10));
                    return JSON.stringify({
                        settled: true,
                        retired: typeof globalThis.__diting_core_base === "undefined",
                    });
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v["settled"],
        serde_json::json!(true),
        "the setTimeout settlement must resolve through our loop"
    );
    assert_eq!(
        v["retired"],
        serde_json::json!(true),
        "__diting_core_base must not exist — core_base.js retired with the deno host"
    );
}

/// Issue #227 regression, behead shape: a watchdog fire during a
/// macrotask payload (synchronous overrun inside `_mtQueue.shift()()`)
/// unwinds the JS stack without the `finally { _mtDepth-- }` (#39).
/// The #50 poll-boundary heal or the 1500ms sibling patrol must
/// recover — the probe timer armed before the fire must still deliver.
#[tokio::test(flavor = "current_thread")]
async fn timer_fires_after_watchdog_beheads_macrotask_payload() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // The #227 stall predicate ships in the snapshot; a typo there
    // silently neuters the stall heal (the Rust side maps any non-bool
    // to false). Pin presence and the healthy-realm answer.
    assert_eq!(
        rt.evaluate("typeof globalThis.__diting_mt_stalled")
            .unwrap(),
        serde_json::Value::String("function".into())
    );
    assert_eq!(
        rt.evaluate("globalThis.__diting_mt_stalled()").unwrap(),
        serde_json::Value::Bool(false)
    );
    rt.evaluate(
        "globalThis.__fired = false;
             // Payload overrun: 3s of sync work, beheaded by a 100ms
             // watchdog with _mtDepth == 1 leaked.
             setTimeout(() => { const t = Date.now(); while (Date.now() - t < 3000) {} }, 0);
             // The #227 probe: must still deliver after the fire.
             setTimeout(() => { globalThis.__fired = true; }, 50);
             // Retire push: fires >250ms after the dead chain's last beat,
             // retiring the dead chain so a fresh one owns the queue.
             setTimeout(() => {}, 700);",
    )
    .unwrap();

    // Behead the payload mid-spin. (The throttled pump used to be the
    // beheader here; it no longer cuts tasks, so arm a short wall directly —
    // a true spin still meets one on every other pump path.)
    let wd = rt.arm_watchdog(std::time::Duration::from_millis(100));
    let _ = tokio::time::timeout(std::time::Duration::from_millis(50), rt.run_event_loop()).await;
    let _ = rt.disarm_watchdog(wd);
    assert!(
        rt.watchdog_fired_total() >= 1,
        "precondition: the watchdog must have fired inside the payload"
    );

    // Pump past the probe (50ms) and the retire push (700ms).
    let mut fired = false;
    for _ in 0..30 {
        let _ = rt.run_event_loop_bounded(200).await;
        if rt
            .evaluate("globalThis.__fired")
            .unwrap()
            .as_bool()
            .unwrap_or(false)
        {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "probe timer never fired after the watchdog beheaded a payload"
    );
    assert_eq!(
        rt.evaluate("globalThis.__MT_STATE().depth")
            .unwrap()
            .as_f64()
            .unwrap(),
        0.0,
        "trampoline depth must not stay leaked"
    );
}

/// Issue #227 regression, native-core shape: a dispatch window beheaded
/// by the script-timeout watchdog used to leak `_mtDepth > 0` (the JS
/// `_mtWindow` finally died with the stack), wedging every timer chain
/// into defer-spin — the quiet corpse. The window dispatch rides the
/// native EventTarget core now (M3 B1): its enter/exit hooks run
/// Rust-side, so the bracket closes even when the listener is beheaded
/// mid-spin. Pin both: the beheading hit a LIVE dispatch (the listener
/// entered its spin), and the depth is back to 0 the moment the
/// beheaded script returns — the leak shape cannot recur on this path.
#[tokio::test(flavor = "current_thread")]
async fn timer_fires_after_script_timeout_beheads_dispatch_window() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        "globalThis.__fired = false; globalThis.__entered = false;
             setTimeout(() => { globalThis.__fired = true; }, 50);",
    )
    .unwrap();

    // Behead a dispatch window: the listener spins 3s synchronously
    // inside the native core's enter/exit bracket, killed at 200ms by
    // the script-timeout watchdog. The exit hook is Rust-side: it runs
    // even though the listener's JS never returned.
    rt.execute_script_with_timeout(
        "globalThis.addEventListener('probe-evt', () => { \
                 globalThis.__entered = true; \
                 const t = Date.now(); while (Date.now() - t < 3000) {} \
             }); \
             globalThis.dispatchEvent(new Event('probe-evt'));",
        std::time::Duration::from_millis(200),
    )
    .unwrap();
    assert!(
        rt.evaluate("globalThis.__entered")
            .unwrap()
            .as_bool()
            .unwrap_or(false),
        "precondition: the watchdog must have beheaded a live dispatch"
    );
    assert_eq!(
        rt.evaluate("globalThis.__MT_STATE().depth")
            .unwrap()
            .as_f64()
            .unwrap(),
        0.0,
        "the native core's exit hook closes the bracket even under beheading"
    );

    // Pump: the probe's push arms a chain that must NOT defer-spin on
    // the leaked depth — the entry heal must have reset it first.
    let mut fired = false;
    for _ in 0..30 {
        let _ = rt.run_event_loop_bounded(200).await;
        if rt
            .evaluate("globalThis.__fired")
            .unwrap()
            .as_bool()
            .unwrap_or(false)
        {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "probe timer never fired: a leaked _mtDepth kept every chain \
             defer-spinning (the #227 quiet-corpse wedge)"
    );
    assert_eq!(
        rt.evaluate("globalThis.__MT_STATE().depth")
            .unwrap()
            .as_f64()
            .unwrap(),
        0.0,
        "trampoline depth must not stay leaked"
    );
}

/// Issue #240 regression: the timer-wheel corpse. `_timerArmed`
/// holding a stale deadline with `_timerPatrol` latched makes
/// `_timerArmNext`'s `e.d >= _timerArmed` gate refuse every later
/// arm, so timers, intervals and rAF rot for the rest of the session
/// while fetches and eval-await stay fine. Observed live on a
/// doudian publish run (2026-10-09) with ZERO watchdog fires in the
/// log — the wheel's state machine is the whole failure surface.
/// The corpse is manufactured through the global-lexical seam (same
/// one the port tests use for `_mtQueue`): stale the heap-top entry,
/// point `armed` at it with no sleep covering it, latch the patrol.
#[tokio::test(flavor = "current_thread")]
async fn timer_background_budget_aligns_and_lifts() {
    // #70: an unwatched session's timers get Chrome's hidden-tab budget.
    // Level 1 aligns a wake-up to the next whole second; level 2 aligns
    // chained timers (nesting > 4) to the next minute; dropping back to 0
    // re-derives every pending deadline, so a chained timer parked on a
    // minute boundary fires on time once a command arrives.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        __diting_set_background(1);
        const t0 = Date.now();
        const fired = await new Promise(r => setTimeout(() => r(Date.now()), 10));
        const aligned = fired >= Math.ceil((t0 + 10) / 1000) * 1000;
        __diting_set_background(0);
        const t1 = Date.now();
        await new Promise(r => setTimeout(r, 10));
        const fgFast = Date.now() - t1 < 500;
        // A chain six deep, run at foreground speed up to the fifth link; the
        // budget goes to level 2 right before the sixth (nesting > 4) is
        // scheduled, so it parks on the next minute boundary.
        let deepAt = 0;
        const deep = new Promise(r => {
            const step = (n) => setTimeout(() => {
                if (n === 6) { deepAt = Date.now(); r(); return; }
                if (n === 5) __diting_set_background(2);
                step(n + 1);
            }, 0);
            step(1);
        });
        await new Promise(r => setTimeout(r, 300));
        const parked = deepAt === 0;
        const liftAt = Date.now();
        __diting_set_background(0);
        await deep;
        return { aligned, fgFast, parked, liftedPromptly: deepAt - liftAt < 500 };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({ "aligned": true, "fgFast": true, "parked": true, "liftedPromptly": true })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn timer_wheel_recover_does_not_multiply_patrol_chains() {
    // #71: each recover dropped the patrol latch and armed a new 1500ms
    // chain while the old one was still in flight, and the old one re-armed
    // itself forever — one leaked chain per recover. An idle x.com serve
    // (20699 false-positive recovers) ran ~1200 parallel chains, ~800
    // patrols/s. Twenty recovers must leave one chain: ~2 patrols in 3s.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("setTimeout(() => {}, 600000);").unwrap();
    let _ = rt.run_event_loop_bounded(100).await;
    for _ in 0..20 {
        rt.evaluate("__diting_mt_recover_termination()").unwrap();
        let _ = rt.run_event_loop_bounded(20).await;
    }
    let p0 = rt
        .evaluate("__MT_STATE().wheel.patrols")
        .unwrap()
        .as_f64()
        .unwrap();
    for _ in 0..30 {
        let _ = rt.run_event_loop_bounded(100).await;
    }
    let p1 = rt
        .evaluate("__MT_STATE().wheel.patrols")
        .unwrap()
        .as_f64()
        .unwrap();
    assert!(
        p1 - p0 <= 4.0,
        "patrols in 3s: {} (one chain is ~2)",
        p1 - p0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn timer_wheel_behind_but_moving_is_not_a_stall() {
    // #71: an unattended x.com serve ran its wheel 600-700ms behind for
    // hours with wakes still climbing, and the witness called it a corpse
    // 20699 times in 16h — each hit a forced synchronous delivery plus a
    // multi-KB post-mortem line. Behind-but-moving must stay false however
    // long it lasts; only a wheel with no progress for 1.5s is a corpse.
    let mut rt = setup_runtime("<html><body></body></html>");
    let overdue_top = "setTimeout(() => {}, 5); _timerHeap[0].d -= 700;";
    rt.evaluate(overdue_top).unwrap();
    for _ in 0..4 {
        assert_eq!(
            rt.evaluate("globalThis.__diting_wheel_stalled()").unwrap(),
            serde_json::Value::Bool(false)
        );
        // The wheel moves between looks (deliveries land), yet the heap top
        // stays overdue — the loaded-host steady state.
        rt.evaluate("__diting_mt_recover_termination()").unwrap();
        rt.evaluate(overdue_top).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn timer_wheel_corpse_healed_by_pump_witness() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // Pin the witness contract (house style from #227's predicate
    // pin): present, and a healthy realm answers false.
    assert_eq!(
        rt.evaluate("typeof globalThis.__diting_wheel_stalled")
            .unwrap(),
        serde_json::Value::String("function".into())
    );
    assert_eq!(
        rt.evaluate("globalThis.__diting_wheel_stalled()").unwrap(),
        serde_json::Value::Bool(false)
    );

    rt.evaluate(
        "globalThis.__fired = false;
             setTimeout(() => { globalThis.__fired = true; }, 5);
             // The kill: the entry goes stale-past-due while `armed`
             // points at it with no wake sleep in flight (the +5ms sleep
             // will arrive and stale-no-op) and the patrol is latched.
             _timerHeap[0].d -= 700;
             _timerArmed = _timerHeap[0].d;
             _timerPatrol = true;",
    )
    .unwrap();
    // #71: overdue alone is not a corpse — the first look only records the
    // wheel's progress; a corpse is overdue with no progress for 1.5s.
    assert_eq!(
        rt.evaluate("globalThis.__diting_wheel_stalled()").unwrap(),
        serde_json::Value::Bool(false),
        "first look at an overdue heap-top just records progress"
    );
    std::thread::sleep(std::time::Duration::from_millis(1600));
    assert_eq!(
        rt.evaluate("globalThis.__diting_wheel_stalled()").unwrap(),
        serde_json::Value::Bool(true),
        "precondition: a stale-past-due heap-top with no progress must trip the witness"
    );

    // Pump: the loop-entry witness must force-recover the wheel and
    // deliver the probe.
    let mut fired = false;
    for _ in 0..30 {
        let _ = rt.run_event_loop_bounded(200).await;
        if rt
            .evaluate("globalThis.__fired")
            .unwrap()
            .as_bool()
            .unwrap_or(false)
        {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "probe timer never fired: the wheel corpse was not healed by the pump witness"
    );
    // The healed wheel must not leave the state machine wedged: with
    // the heap drained there is nothing to cover and no patrol to run.
    let wheel = rt
        .evaluate("JSON.stringify(globalThis.__MT_STATE().wheel)")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        wheel.contains("\"heap\":0"),
        "heap should be empty after delivery: {wheel}"
    );
}

/// The #240 heal from the JS side: the termination recover hook
/// (`__diting_mt_recover_termination`, run by the engine at every
/// unwind recovery point) must unlatch the patrol and force a fresh
/// arm — the same hook the #39/#227 paths already call.
#[tokio::test(flavor = "current_thread")]
async fn timer_wheel_corpse_healed_by_recover_hook() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        "globalThis.__fired = false;
             setTimeout(() => { globalThis.__fired = true; }, 5);
             _timerHeap[0].d -= 700;
             _timerArmed = _timerHeap[0].d;
             _timerPatrol = true;
             __diting_mt_recover_termination();",
    )
    .unwrap();

    let mut fired = false;
    for _ in 0..30 {
        let _ = rt.run_event_loop_bounded(200).await;
        if rt
            .evaluate("globalThis.__fired")
            .unwrap()
            .as_bool()
            .unwrap_or(false)
        {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "probe timer never fired: the recover hook did not heal the wheel"
    );
}

/// #240 diagnostics: `__MT_STATE().wheel` names the kill when a corpse
/// re-forms (wakes/staleWakes tell whether sleeps deliver, throws
/// whether op calls fired synchronously, patrols whether the safety
/// net lived). Pin the keys so a refactor cannot silently drop them.
#[tokio::test(flavor = "current_thread")]
async fn timer_wheel_state_counters_surface_in_mt_state() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate("JSON.stringify(globalThis.__MT_STATE().wheel)")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    for key in [
        "\"heap\":",
        "\"armedIn\":",
        "\"patrol\":",
        "\"wakes\":",
        "\"staleWakes\":",
        "\"throws\":",
        "\"patrols\":",
        "\"recovers\":",
    ] {
        assert!(v.contains(key), "wheel state must expose {key}: {v}");
    }
}

// #114: deno_core's execute_script runs no microtask checkpoint, so under
// the old unconditional-`await` wrapper even a sync-completing expression
// had its done-sentinel parked in a microtask — one that only runs on an
// event-loop turn. On a revived persistent session that first turn gets
// swallowed by multi-second page scripts and the eval burns its whole
// budget into EVAL_TIMEOUT for an expression that had already finished.
// Deterministic repro of the starvation half: poison the microtask queue
// with a self-sustaining promise chain (never empties, so any event-loop
// drain spins until the watchdog), then eval `1+1` under awaitPromise —
// it must settle during execute_script without ever entering the loop.
#[tokio::test(flavor = "current_thread")]
async fn test_await_promise_sync_result_settles_without_event_loop_turn() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
            "(function poison(){ var f = function(){ Promise.resolve().then(f); }; Promise.resolve().then(f); })()",
        )
        .unwrap();
    let t0 = std::time::Instant::now();
    let outcome = rt
        .evaluate_remote_outcome("1 + 1", false, true, 5000, None)
        .await
        .unwrap();
    assert_eq!(outcome.info.value.unwrap(), serde_json::json!(2));
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(3),
        "sync result waited {}ms — sentinel must be set during execute_script, not an event-loop turn",
        t0.elapsed().as_millis()
    );
}

// #17: an uncaught exception inside a timer callback reaches BOTH error
// surfaces Chrome exposes — window.onerror with (message, source, line)
// and an ErrorEvent on window whose .error preserves the Error object.
#[tokio::test(flavor = "current_thread")]
async fn uncaught_timer_error_fires_onerror_and_error_event() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const out = {};
            window.onerror = function (msg, src, line) {
                out.onerror = [
                    String(msg).indexOf('boom') !== -1,
                    typeof src === 'string' && src.length > 0,
                    typeof line === 'number' && line > 0,
                ];
            };
            window.addEventListener('error', function (e) {
                out.evt = [
                    e instanceof ErrorEvent,
                    String(e.message).indexOf('boom') !== -1,
                    e.error instanceof Error,
                    e.error && e.error.message === 'boom',
                    typeof e.filename === 'string' && e.filename.length > 0,
                    typeof e.lineno === 'number' && e.lineno > 0,
                ];
            });
            setTimeout(function () { throw new Error('boom'); }, 0);
            await new Promise(r => setTimeout(r, 20));
            return [out.onerror, out.evt];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([[true, true, true], [true, true, true, true, true, true]])
    );
}

// #17: an exception raised INSIDE onerror must not re-enter the error
// pipeline (Chrome does not re-report errors thrown by the error
// handler) — exactly one onerror call for the original throw.
#[tokio::test(flavor = "current_thread")]
async fn throwing_onerror_does_not_reenter_error_pipeline() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            window.__calls = 0;
            window.onerror = function () { window.__calls++; throw new Error('inside onerror'); };
            setTimeout(function () { throw new Error('outer'); }, 0);
            await new Promise(r => setTimeout(r, 20));
            return window.__calls;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(1));
}

#[tokio::test(flavor = "current_thread")]
async fn test_timer_string_handlers_run_in_global_scope_at_fire_time() {
    // Upstream 452cc85: string setTimeout/setInterval handlers run as
    // global-scope classic scripts at fire time — declarations become
    // globals, and a syntax error surfaces when the timer elapses instead
    // of being swallowed at scheduling. We used to drop string handlers
    // entirely (silent no-op that still returned a timer id).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            setTimeout('var strVarDecl = 7; window.__strRan = "ran";', 0);
            let scheduleThrew = false;
            try { setTimeout('this is (not javascript', 0); } catch (e) { scheduleThrew = true; }
            window.__intervalCount = 0;
            const iid = setInterval('window.__intervalCount++; clearInterval(window.__iid);', 0);
            window.__iid = iid;
            await new Promise(r => setTimeout(r, 10));
            return [window.__strRan, strVarDecl, scheduleThrew, window.__intervalCount, typeof iid];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["ran", 7, false, 1, "number"])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_same_deadline_timers_fire_in_fifo_order() {
    // HTML's timer task source is ordered: timers with the same deadline
    // run in scheduling order. We used to hand every timer its own
    // tokio sleep, and same-tick completions settle in the op driver's
    // completion order — not scheduling order (#38).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const order = [];
            for (let i = 0; i < 30; i++) setTimeout(() => order.push(i), 0);
            // nonzero same delay: all land in the same timer-wheel slot
            const wheel = [];
            for (let i = 0; i < 30; i++) setTimeout(() => wheel.push(i), 40);
            await new Promise(r => setTimeout(r, 80));
            // mixed delays, interleaved scheduling: all 0ms in order, then
            // all 5ms in order.
            const mixed = [];
            for (let i = 0; i < 20; i++) {
                const d = i % 2 === 0 ? 0 : 5;
                setTimeout(() => mixed.push(i), d);
            }
            // scheduled from inside timer callbacks (async arming, the way
            // real pages chain timers)
            const nested = [];
            setTimeout(() => { nested.push('a'); setTimeout(() => nested.push('a1'), 0); }, 0);
            setTimeout(() => { nested.push('b'); setTimeout(() => nested.push('b1'), 0); }, 0);
            setTimeout(() => nested.push('c'), 0);
            await new Promise(r => setTimeout(r, 120));
            // The live observation (#30 session, fxg boot storm): 1500
            // same-delay timers completed 0..127,1499,128..1498 — one
            // mid-sequence inversion. Small batches don't trip it.
            const mass = [];
            for (let i = 0; i < 1500; i++) setTimeout(() => mass.push(i), 50);
            await new Promise(r => setTimeout(r, 200));
            return [order.join(','), wheel.join(','), mixed.join(','), nested.join(','), mass.join(',')];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let expected_order: Vec<String> = (0..30).map(|i| i.to_string()).collect();
    let mut expected_mixed: Vec<String> = (0..20).step_by(2).map(|i| i.to_string()).collect();
    expected_mixed.extend((1..20).step_by(2).map(|i| i.to_string()));
    assert_eq!(
        v[0],
        serde_json::json!(expected_order.join(",")),
        "delay-0 FIFO: {:?}",
        v[0]
    );
    assert_eq!(
        v[1],
        serde_json::json!(expected_order.join(",")),
        "same-wheel-slot FIFO: {:?}",
        v[1]
    );
    assert_eq!(
        v[2],
        serde_json::json!(expected_mixed.join(",")),
        "mixed-delay ordering: {:?}",
        v[2]
    );
    assert_eq!(
        v[3],
        serde_json::json!("a,b,c,a1,b1"),
        "nested-timer ordering: {:?}",
        v[3]
    );
    let expected_mass: Vec<String> = (0..1500).map(|i| i.to_string()).collect();
    assert_eq!(
        v[4],
        serde_json::json!(expected_mass.join(",")),
        "1500 same-deadline FIFO: {:?}",
        v[4]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_throwing_timer_is_contained_and_later_timers_still_fire() {
    // Upstream #394: a page timer that throws (Booking.com's
    // "Cannot redefine property: src" inside a timer) took the whole
    // obscura process down with it. Timer callbacks are page code —
    // a throw must be caught at the timer boundary and the loop must
    // keep servicing later timers (same for setInterval ticks).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            setTimeout(() => { throw new TypeError('boom-timeout'); }, 0);
            let intervalTicks = 0;
            const iid = setInterval(() => {
                intervalTicks++;
                if (intervalTicks === 1) throw new RangeError('boom-interval');
                if (intervalTicks >= 3) clearInterval(iid);
            }, 0);
            setTimeout(() => { window.__survivor = 'ran'; }, 0);
            await new Promise(r => setTimeout(r, 30));
            return [window.__survivor, intervalTicks];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(["ran", 3]));
}

// ------------------------------------------------------------------
// #108 stress reproducers. The failure shape: inside a setTimeout
// callback `typeof Date.now` reads "function" but the call throws
// "Date.now(...) is not a function" — a call-path (dispatch) failure
// with a healthy object, seen only in dev builds, only from the
// timer/promise-reaction delivery path (top-level scripts and
// evaluate() stayed clean), never reproducible on demand and
// time-clustered across concurrent runs. These tests are #[ignore]d
// (too slow for the default suite): run with
// `cargo test -p diting --features screenshot -- --ignored date_now`
// when touching isolate lifecycle or the event-loop pump.
//
// Probe: capture typeof at fire time, call under try/catch, report
// through globals the test reads after the pump — the exact evidence
// channel the #108 samples used.

#[tokio::test(flavor = "current_thread")]
#[ignore = "#108 stress: single-isolate timer hammer, minutes-scale"]
async fn date_now_timer_hammer_single_isolate() {
    let mut rt = setup_runtime("<html><body></body></html>");
    for i in 0..2000 {
        rt.evaluate(
            "window.__m1 = 0; window.__err = ''; window.__typeof = '';\
                 setTimeout(function () {\
                     window.__typeof = typeof Date.now;\
                     try { window.__m1 = Date.now(); }\
                     catch (e) { window.__err = (e && e.message) || String(e); }\
                 }, 2)",
        )
        .unwrap();
        rt.run_event_loop_bounded(600).await.unwrap();
        // Interleave an execute_script-path call between pumps, the
        // way session commands interleave with idle slices.
        let ty = rt.evaluate("typeof Date.now").unwrap();
        let m1 = rt.evaluate("window.__m1").unwrap();
        let err = rt.evaluate("window.__err").unwrap();
        let fired_typeof = rt.evaluate("window.__typeof").unwrap();
        assert_eq!(ty, serde_json::json!("function"), "iter {i}: execute path");
        assert!(
            m1.as_f64().unwrap_or(0.0) > 0.0,
            "iter {i}: timer callback never landed (m1={m1:?})"
        );
        assert_eq!(
            err,
            serde_json::json!(""),
            "iter {i}: timer-callback Date.now threw ({err:?}, typeof at fire time = {fired_typeof:?})"
        );
        assert_eq!(
            fired_typeof,
            serde_json::json!("function"),
            "iter {i}: typeof inside the timer callback"
        );
    }
}

#[test]
#[ignore = "#108 stress: multi-isolate construct/drop churn, minutes-scale"]
fn date_now_timer_hammer_isolate_churn() {
    // Mimics the failing environment: many runtimes built and dropped
    // concurrently (cargo-test threads / stateless fetch paths), each
    // one running the #108 timer probe through the same bounded pump
    // the session idle loop uses. Construction is serialized by
    // ISOLATE_CONSTRUCT_LOCK; drop is NOT — if V8 15.0's process-wide
    // JSDispatchTable has an alloc-vs-free race on that seam, this
    // is the load that widens it.
    let mut handles = Vec::new();
    for t in 0..6u32 {
        handles.push(
            std::thread::Builder::new()
                .name(format!("dt-now-{t}"))
                .stack_size(diting_env::env_knobs::js_stack_mb() * 1024 * 1024)
                .spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("per-thread tokio runtime");
                    rt.block_on(async move {
                        for i in 0..200usize {
                            let mut js = setup_runtime("<html><body></body></html>");
                            js.evaluate(
                                "window.__m1 = 0; window.__err = ''; window.__typeof = '';\
                                 setTimeout(function () {\
                                     window.__typeof = typeof Date.now;\
                                     try { window.__m1 = Date.now(); }\
                                     catch (e) { window.__err = (e && e.message) || String(e); }\
                                 }, 2)",
                            )
                            .unwrap();
                            js.run_event_loop_bounded(600).await.unwrap();
                            let m1 = js.evaluate("window.__m1").unwrap();
                            let err = js.evaluate("window.__err").unwrap();
                            let ty = js.evaluate("window.__typeof").unwrap();
                            assert!(
                                m1.as_f64().unwrap_or(0.0) > 0.0,
                                "thread {t} iter {i}: callback never landed (m1={m1:?})"
                            );
                            assert_eq!(
                                err,
                                serde_json::json!(""),
                                "thread {t} iter {i}: Date.now threw ({err:?}, typeof={ty:?})"
                            );
                            // drop runs here UNLOCKED vs other threads'
                            // constructions — the seam under test
                        }
                    });
                })
                .expect("spawn churn thread"),
        );
    }
    for h in handles {
        h.join().expect("churn thread panicked");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_timer_nesting_clamps_past_level_four() {
    // HTML spec: chained setTimeout(0) past nesting level 4 floors at 4ms.
    // The clamp must NOT fire on ordinary top-level timers — an
    // unconditional clamp stalled the doudian boot (later ruled out as the
    // freeze root cause, but the nesting gate stays spec-pinned).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const p = new Promise((resolve) => {
                const stamps = [];
                const t0 = performance.now();
                let level = 0;
                const tick = () => {
                    stamps.push(performance.now() - t0);
                    level += 1;
                    if (level >= 10) { resolve(stamps); return; }
                    setTimeout(tick, 0);
                };
                setTimeout(tick, 0);
            });
            return Promise.race([p, new Promise((r) => setTimeout(() => r(null), 5000))]);
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert!(v.is_array(), "chain must finish inside 5s");
    let stamps: Vec<f64> = serde_json::from_value(v).unwrap();
    assert_eq!(stamps.len(), 10, "stamps: {:?}", stamps);
    // Fires 1-5 (nesting ≤4) are unclamped; fire 6 onward each wait ≥4ms.
    let diffs: Vec<String> = stamps
        .windows(2)
        .map(|w| format!("{:.2}", w[1] - w[0]))
        .collect();
    assert!(
        stamps[5] - stamps[4] >= 3.5,
        "first clamped step must wait the 4ms floor, diffs={:?}",
        diffs
    );
    assert!(
        stamps[9] - stamps[5] >= 14.0,
        "four clamped steps must each floor at 4ms, diffs={:?}",
        diffs
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_throwing_macrotask_payload_does_not_sever_chain() {
    // #37: a payload that throws synchronously used to reject the whole
    // drain chain as an unhandled rejection — after it, `armed` stayed
    // true and every later timer/message delivery rotted in the queue
    // (fxg wedge: depth 0, armed true, 610+ payloads). The drain now
    // catches payload errors (routing them to the uncaught-error
    // channel) and keeps draining.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const got = [];
            setTimeout(() => { throw new Error('boom'); }, 0);
            setTimeout(() => got.push('after-boom'), 0);
            const ch = new MessageChannel();
            ch.port1.onmessage = () => got.push('msg');
            setTimeout(() => ch.port2.postMessage('x'), 0);
            return new Promise((resolve) => {
                setTimeout(() => resolve(got.slice()), 50);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let got = v.as_array().unwrap();
    assert_eq!(
        got,
        &["after-boom", "msg"],
        "deliveries after a throwing payload must survive: {:?}",
        got
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_macrotask_staleness_escape_keeps_window_discipline() {
    // #37: when pushes outpace item runs past the staleness threshold,
    // _runAsMacrotask retires the chain (generation bump) and re-arms
    // from op_sleep — never inline. During a live dispatch window the
    // escape must churn chains without losing, duplicating, or
    // re-entering deliveries: 1500 pushes inside one listener crosses
    // the 1000-push budget mid-window, so this exercises the retire
    // path under exactly the conditions that must not break.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const hits = [];
            globalThis.__hits = hits;
            document.addEventListener('ping', () => {
                for (let i = 0; i < 1500; i++) setTimeout(() => hits.push(i), 0);
            });
            document.dispatchEvent(new Event('ping'));
            return new Promise((resolve) => {
                setTimeout(() => resolve({
                    hits: globalThis.__hits.slice(),
                    state: globalThis.__MT_STATE(),
                }), 120);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let hits = v["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1500, "every delivery lands exactly once");
    // NOTE: NOT asserting FIFO here. Equal-deadline op_sleep completions
    // come off the tokio timer wheel in non-registration order (observed
    // 0..127,1499,128..1498), so push order into the macrotask queue is
    // already a permutation. Chrome fires same-expiry timers in
    // insertion order — a real parity gap, but a _scheduleAfter/timer-
    // wheel one, orthogonal to the #37 chain liveness this test pins.
    let mut sorted: Vec<i64> = hits.iter().map(|h| h.as_i64().unwrap_or(-1)).collect();
    sorted.sort();
    let expected: Vec<i64> = (0..1500).collect();
    assert_eq!(sorted, expected, "exactly-once, no loss, no dup");
    // 1500 window pushes + the 1 resolver timer payload: items is a
    // lifetime counter, so exactly-once shows up as 1501, not 1500.
    assert_eq!(
        v["state"]["items"].as_i64(),
        Some(1501),
        "items counted once each"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_macrotask_backlog_drains_with_microtask_checkpoints() {
    // #37: the drain loop yields to a microtask checkpoint between
    // items, so a payload's promise reactions all settle before the
    // next payload runs — the browser's task/microtask boundary. The
    // old per-item op round trip capped throughput at ~677 items/s
    // while fxg pushed ~2200/s (qlen 610→58887 in 26s); the await-based
    // checkpoint keeps a deep backlog from becoming a minutes-long
    // timer blackout.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const seen = [];
            let prevSettled = true;
            globalThis.__seen = seen;
            for (let i = 0; i < 2000; i++) {
                setTimeout(() => {
                    seen.push(prevSettled);
                    prevSettled = false;
                    Promise.resolve().then(() => { prevSettled = true; });
                }, 0);
            }
            return new Promise((resolve) => {
                setTimeout(() => resolve({ seen: globalThis.__seen.slice() }), 400);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let seen = v["seen"].as_array().unwrap();
    assert_eq!(seen.len(), 2000, "whole backlog drained: {}", seen.len());
    assert!(
        seen.iter().all(|x| x.as_bool() == Some(true)),
        "each payload observes the previous payload's microtask settled"
    );
}

/// hosted-incident 10-14: each heap-guard TERMINATION (the second-trip
/// arm of recover_heap_limit — the pressure-GC rescue is not one) is a
/// strike on the realm. The session actor drops the realm at three; a
/// page that keeps climbing after two full rescues is unmanageable and
/// its isolate must go back to the process instead of riding
/// terminate→GC→climb forever (the public zhipin/yupao loop climbed to
/// 7GB this way). Flags driven by hand — the callback needs a real heap
/// climb; recover_heap_limit is the safe point it hands control to.
#[test]
fn heap_guard_strikes_count_terminations_per_realm() {
    let mut rt = setup_runtime("<html><body></body></html>");
    assert_eq!(rt.heap_guard_strikes(), 0);

    for _ in 0..2 {
        rt.heap_limit_state
            .tripped
            .store(true, std::sync::atomic::Ordering::SeqCst);
        rt.heap_limit_state
            .restore_limit
            .store(256 * 1024 * 1024, std::sync::atomic::Ordering::SeqCst);
        assert!(rt.recover_heap_limit());
    }
    assert_eq!(rt.heap_guard_strikes(), 2);

    // A pressure-only cycle (first-trip GC rescue) is not a strike.
    rt.heap_limit_state
        .pressured
        .store(true, std::sync::atomic::Ordering::SeqCst);
    rt.heap_limit_state
        .restore_limit
        .store(256 * 1024 * 1024, std::sync::atomic::Ordering::SeqCst);
    assert!(rt.recover_heap_limit());
    assert_eq!(rt.heap_guard_strikes(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn test_settle_quiet_ignores_far_timers_but_not_near_work() {
    // The settle loop's early exit: a page parked on a far-off timer is
    // quiet for any window that ends before it; a due timer or an
    // in-flight fetch is not.
    let mut rt = setup_runtime("<html><body></body></html>");
    assert!(rt.settle_quiet(500), "fresh page has nothing pending");

    rt.evaluate("setTimeout(function () {}, 4000)").unwrap();
    assert!(rt.settle_quiet(500), "a 4s timer cannot fire inside 500ms");
    assert!(!rt.settle_quiet(5000), "but it can inside a 5s window");

    rt.evaluate("setTimeout(function () {}, 50)").unwrap();
    assert!(
        !rt.settle_quiet(500),
        "a 50ms timer fires inside the window"
    );
    let _ = rt.run_event_loop_bounded(300).await;
    assert!(
        rt.settle_quiet(500),
        "once it has fired only the far timer is left"
    );

    rt.evaluate("globalThis.__p = fetch('data:text/plain,x'); 1")
        .unwrap();
    assert!(!rt.settle_quiet(500), "an in-flight fetch is never quiet");
    let _ = rt.run_event_loop_bounded(300).await;
    assert!(rt.settle_quiet(500), "settled fetch is quiet again");
}

#[test]
fn test_watchdog_zombie_verdict_marks_and_lifts_owner_thread() {
    // rusty_v8 #2088 (reproduced on 150.4.0 and 152.2.0): terminate_execution never lands
    // inside a TurboFan-optimized loop calling Array.prototype.fill. This
    // loop is bounded (~7s) so it ends on its own (90M iterations: once the
    // font tables left the snapshot the realm heap went 326MB -> 8MB and the
    // old 1.5M-iteration loop dropped from ~7s of GC to ~130ms, finishing
    // before the watchdog could even fire): the terminate fired at
    // 200ms is swallowed (re-hammering too), the post-fire grace (3s under
    // cfg(test), 20s in production — ZOMBIE_VERDICT_AFTER) runs out and must register
    // the owner thread as a zombie (the session layer's cue to stop waiting
    // on it), and the loop's natural end must lift the verdict again.
    let (tid_tx, tid_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut rt = setup_runtime("<html><body></body></html>");
        tid_tx.send(std::thread::current().id()).unwrap();
        let wd = rt.arm_watchdog(std::time::Duration::from_millis(200));
        let t0 = std::time::Instant::now();
        let _ = rt.evaluate(
            "(() => { let x; for (let i = 0; i < 90000000; i++) x = new Array(64).fill(1); })()",
        );
        let ran = t0.elapsed();
        let fired = rt.disarm_watchdog(wd);
        (fired, ran)
    });
    let tid = tid_rx.recv().unwrap();
    let t0 = std::time::Instant::now();
    let mut marked = false;
    while t0.elapsed() < std::time::Duration::from_secs(5) {
        if crate::runtime::is_zombie_thread(tid) {
            marked = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let (fired, ran) = worker.join().unwrap();
    assert!(fired, "precondition: the watchdog fired");
    assert!(
        ran > std::time::Duration::from_secs(4),
        "precondition: the terminate must NOT have landed (#2088 hole) — the loop ran {ran:?}"
    );
    assert!(
        marked,
        "the post-fire grace must declare the owner thread a zombie"
    );
    assert!(
        !crate::runtime::is_zombie_thread(tid),
        "the late resurface must lift the verdict"
    );
}

#[test]
fn test_watchdog_rehammers_after_heal_and_continue() {
    // A single terminate is not enough when the owner heals it (the #50
    // poll-boundary cancel) and keeps running JS under the same, already
    // fired token — the next task runs with no wall. Mimic that loop: four
    // 3s spins, revived after each. The first is cut by the fire (200ms);
    // the re-hammer (from fire+500ms, every 250ms) must cut the rest too.
    let mut rt = setup_runtime("<html><body></body></html>");
    let wd = rt.arm_watchdog(std::time::Duration::from_millis(200));
    let t0 = std::time::Instant::now();
    for _ in 0..4 {
        let _ = rt.runtime.execute_script(
            "<spin>",
            "{ const end = Date.now() + 3000; while (Date.now() < end) {} }",
        );
        rt.runtime.revive();
    }
    let total = t0.elapsed();
    let fired = rt.disarm_watchdog(wd);
    assert!(fired, "precondition: the watchdog fired");
    assert!(
        total < std::time::Duration::from_secs(4),
        "every spin after the heal must be re-terminated, took {total:?} (unbounded = ~9s)"
    );
    assert!(
        !crate::runtime::is_zombie_thread(std::thread::current().id()),
        "a re-hammered spin resurfaces — no zombie verdict"
    );
    assert_eq!(rt.evaluate("1 + 1").unwrap().as_f64(), Some(2.0));
}

#[tokio::test(flavor = "current_thread")]
async fn busy_freeze_burst_throttles_without_cutting_tasks() {
    // The #66 freeze's 50ms burst used a budget+budget watchdog: every burst
    // beheaded the running task (x.com: 1000+ fires a session, page state
    // stranded half-set, fetch continuations severed). Now a task runs to
    // completion, and the burst still ends at a task boundary near its budget.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        "globalThis.__ctx = 0; globalThis.__wall = 0;
         setTimeout(() => { __ctx = 1; const t = Date.now(); while (Date.now() - t < 300) {} __ctx = 0; __wall = 1; }, 0);",
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded_throttled(50).await;
    assert_eq!(rt.watchdog_fired_total(), 0, "a 300ms task is not a spin");
    assert_eq!(
        rt.evaluate("[__ctx, __wall]").unwrap(),
        serde_json::json!([0, 1]),
        "the task ran to completion, nothing stranded"
    );

    // Containment: a hundred 5ms tasks don't all run in one 50ms burst.
    rt.evaluate(
        "globalThis.__n = 0;
         for (let i = 0; i < 100; i++) setTimeout(() => { const t = Date.now(); while (Date.now() - t < 5) {} __n++; }, 0);",
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    let _ = rt.run_event_loop_bounded_throttled(50).await;
    let ms = t0.elapsed().as_millis();
    let n = rt.evaluate("__n").unwrap().as_f64().unwrap();
    assert!(n < 100.0, "the burst ran every queued task ({n}) in {ms}ms");
    assert!(
        ms < 250,
        "the burst overran its budget: {ms}ms for {n} tasks"
    );
}
