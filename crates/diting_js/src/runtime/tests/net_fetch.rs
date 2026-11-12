//! fetch API 行为：body 编解码/redirect 链/abort/reqres/流
use super::super::*;
use super::common::{setup_runtime, setup_runtime_with_cookies};

/// Response.url/redirected must follow Chrome semantics (final URL after
/// redirects, `redirected` true when hops were followed). The tmall
/// report's mtop API redirected into `_____tmd_____/punish` while the
/// fetch row still claimed the API URL — the visibility hole behind the
/// "promise pending with no signal" diagnosis.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_reports_final_url_and_redirected_flag() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // Hop 1: /start answers 302 -> /landing. Hop 2: /landing answers 200.
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        stream
                .write_all(b"HTTP/1.1 302 Found\r\nlocation: /landing\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                .unwrap();
        stream.flush().unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const r = await fetch('/start');
                    return { url: r.url, redirected: String(r.redirected), body: await r.text() };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "url": format!("http://127.0.0.1:{}/landing", port),
            "redirected": "true",
            "body": "ok",
        })
    );
}

/// fetch must honor AbortSignal (batch-82 leftover): pre-flight reject
/// (no request at all), mid-flight reject with the signal's reason while
/// the underlying walk is still pending, AbortSignal.timeout's
/// TimeoutError, reason identity for abort(reason), throwIfAborted, and a
/// real default signal on Request objects.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_honors_abort_signal() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // Two slow responses: leg 1 (mid-flight abort) and leg 3
        // (AbortSignal.timeout) both hit the server but must reject long
        // before the 800ms body lands. Leg 2 (pre-aborted) never connects.
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(800));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok");
            let _ = stream.flush();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const out = {};
                    // Leg 1: abort lands while the response is still in flight.
                    let fired = 0;
                    const ac1 = new AbortController();
                    ac1.signal.addEventListener('abort', () => { fired++; });
                    const p1 = fetch('/slow', { signal: ac1.signal })
                        .then(() => 'resolved', e => 'rejected:' + e.name);
                    setTimeout(() => ac1.abort(), 50);
                    out.midFlight = await p1;
                    out.abortEventFired = fired === 1;

                    // Leg 2: pre-aborted signal rejects with the caller's
                    // reason before any network activity.
                    const ac2 = new AbortController();
                    const myErr = new Error('mine');
                    ac2.abort(myErr);
                    out.reasonIdentity = ac2.signal.reason === myErr;
                    let threw = null;
                    try { ac2.signal.throwIfAborted(); } catch (e) { threw = e; }
                    out.throwIfAbortedSameObject = threw === myErr;
                    out.preAborted = await fetch('/slow', { signal: ac2.signal })
                        .then(() => 'resolved', e => 'rejected:' + e.name);

                    // Leg 3: AbortSignal.timeout rejects with TimeoutError.
                    out.timeout = await fetch('/slow', { signal: AbortSignal.timeout(120) })
                        .then(() => 'resolved', e => 'rejected:' + e.name);

                    // Surface: default reason is an AbortError DOMException,
                    // any() follows an already-aborted source, and Request's
                    // default signal is a real AbortSignal.
                    const ac4 = new AbortController();
                    ac4.abort();
                    out.defaultReasonName = ac4.signal.reason.name;
                    out.anyFollowsAborted = AbortSignal.any([ac2.signal]).aborted === true;
                    out.requestDefaultSignal = new Request('/x').signal instanceof AbortSignal;
                    return out;
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "midFlight": "rejected:AbortError",
            "abortEventFired": true,
            "reasonIdentity": true,
            "throwIfAbortedSameObject": true,
            // plain Error('mine') — the reason passes through untouched
            "preAborted": "rejected:Error",
            "timeout": "rejected:TimeoutError",
            "defaultReasonName": "AbortError",
            "anyFollowsAborted": true,
            "requestDefaultSignal": true,
        })
    );
}

/// Issue #50 regression: a watchdog that fires while the isolate is parked
/// (between commands — disarm starved on a busy session task) leaves V8
/// execution-termination PENDING. The next V8 entry is then the op-driver
/// tick delivering the just-finished fetch op, so the pending termination
/// materializes there: tick_fn.call throws, the op batch (the fetch
/// promise's args) is destroyed, and the page-side promise never settles —
/// the xhs upload-content hang. run_event_loop now heals the stale flag at
/// the top of every poll, so delivery must survive a parked fire.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_promise_survives_parked_watchdog_fire() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        // The response lands well after the 100ms watchdog budget (fire
        // while parked) and well before we poll again — deterministic
        // parked-fire shape, no starvation race required.
        std::thread::sleep(std::time::Duration::from_millis(400));
        let body = "x".repeat(4096);
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    rt.evaluate(
        "globalThis.__r = -1;
             fetch('/hold').then(r => r.text())
             .then(t => { globalThis.__r = t.length; },
                   e => { globalThis.__r = 'err:' + e; });",
    )
    .unwrap();

    // Arm a watchdog whose budget expires while nothing polls the isolate.
    let wd = rt.arm_watchdog(std::time::Duration::from_millis(100));
    // Parked window: the fire lands (~100ms, termination goes pending) and
    // the op completes into the driver queue (~400ms) — nobody enters V8
    // in between.
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;

    // The heal must clear the pending termination before the op-driver
    // tick delivers the fetch result; the promise settles instead of
    // hanging forever.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run_event_loop()).await;
    let fired = rt.disarm_watchdog(wd);
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert!(
        fired,
        "precondition: the watchdog must have fired while parked"
    );
    assert_eq!(
        rt.evaluate("globalThis.__r").unwrap().as_f64().unwrap(),
        4096.0
    );
}

/// Issue #63, synthetic repro of the release-repro shape (2026-11-10
/// timeline burst): a 16-leg fetch storm against a slow endpoint, DOM-churn
/// macrotask bursts on every settlement, and self-chained 1.2s synchronous
/// payloads that the throttled 100ms pump wall beheads — all on one realm,
/// the x.com burst's ingredients without x.com. A healthy realm rides it
/// out: every leg settles, the trampoline depth returns to 0, and a fresh
/// timer still fires afterwards. The churn family (#227/#240/#63) instead
/// eats the storm — legs rot armed without opok, continuations vanish with
/// each beheading. #[ignore] while the race is live (a live race must not
/// gate CI); run with --ignored to observe the signature, un-ignore when
/// the root-cause batch lands.
#[allow(clippy::await_holding_lock)] // the env guard must span the awaits — that's the serialization
#[tokio::test(flavor = "current_thread")]
#[ignore = "#63: live race repro; run with --ignored to see the churn signature"]
async fn fetch_storm_with_sync_overrun_bait_stays_healthy() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // One connection per leg; 1500ms think-time keeps every leg in
        // flight across the beheading rounds. 64 accepts covers both storm
        // waves plus the follow-up generation; the listener drop ends the loop.
        for _ in 0..64 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                std::thread::sleep(std::time::Duration::from_millis(1500));
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
                let _ = stream.flush();
            });
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    // A/B knobs: DITING_REPRO_NO_SPIN=1 drops the beheading bait,
    // DITING_REPRO_NO_CHURN=1 drops the sustained page pressure — the 2x2
    // matrix attributes the eval-latency reading (quiet-realm heal path vs
    // ceremony wall vs queue pressure).
    let spins_on = std::env::var("DITING_REPRO_NO_SPIN").ok().as_deref() != Some("1");
    let churn_on = std::env::var("DITING_REPRO_NO_CHURN").ok().as_deref() != Some("1");
    let churn_target = if churn_on { 600.0 } else { 0.0 };
    rt.evaluate(&format!(
        "globalThis.__SPIN = {spins_on}; globalThis.__CHURN = {churn_on};"
    ))
    .unwrap();
    rt.evaluate(
        r#"
        globalThis.__ok = 0; globalThis.__legs = 0; globalThis.__spins = 0;
        globalThis.__churnTicks = 0;
        const burst = (n) => { for (let i = 0; i < n; i++)
            setTimeout(() => {
                const d = document.createElement('div');
                for (let j = 0; j < 20; j++) d.appendChild(document.createElement('span'));
                document.body.appendChild(d); document.body.removeChild(d);
            }, 0); };
        const leg = (k) => { globalThis.__legs++;
            fetch('/slow?k=' + k).then(r => r.text()).then(() => {
                globalThis.__ok++;
                if (k < 4) { burst(40); leg(k + 16); }
            }, () => { globalThis.__ok++; }); };
        for (let k = 0; k < 16; k++) leg(k);
        // Sustained page-scheduler pressure (the x.com furnace's missing
        // ingredient): 14 items per 20ms tick = ~700 macrotasks/s, running
        // the whole storm window instead of one-shot bursts. Re-arm first —
        // a beheaded self-chain never reschedules itself (learned above).
        (function churn() {
            if (!globalThis.__CHURN || globalThis.__churnTicks++ >= 600) return;
            setTimeout(churn, 20);
            for (let i = 0; i < 14; i++)
                setTimeout(() => {
                    const s = document.createElement('span');
                    document.body.appendChild(s); document.body.removeChild(s);
                }, 0);
        })();
        (function spin() {
            // Re-arm BEFORE the payload spins: a beheaded payload unwinds
            // without running anything after its spin loop, so a reschedule
            // placed there dies with the first fire (observed: two runs,
            // spins frozen at 2, fires=2). Pre-arming keeps the bait alive
            // for all twenty rounds.
            if (!globalThis.__SPIN || globalThis.__spins++ >= 20) return;
            setTimeout(spin, 50);
            setTimeout(() => { const t = Date.now();
                while (Date.now() - t < 1200) {}
                // Second wave from inside the overrun window — the burst
                // arrives WHILE the realm is busy being beheaded, x.com shape.
                if (globalThis.__spins === 9)
                    for (let k = 32; k < 40; k++) leg(k);
            }, 0);
        })();
        "#,
    )
    .unwrap();

    // Storm window (~15s of 100ms throttled slices): sample the witnesses so
    // a local run prints the signature (beats furnace, qlen, watchdog fires,
    // in-flight legs) whether or not the race bites. Every 15th slice also
    // fires the #63 victim — an async eval (the flow auth-ctx shape: promise
    // + timer) that must settle inside its budget even mid-storm.
    let mut worst_qlen = 0usize;
    let mut furnace_slices = 0usize;
    let mut beats_prev = 0f64;
    let mut eval_worst_ms: u128 = 0;
    let mut eval_samples: Vec<String> = Vec::new();
    let mut samples: Vec<String> = Vec::new();
    for slice in 0..150 {
        let _ = rt.run_event_loop_bounded_throttled(100).await;
        if slice % 15 == 7 {
            let t0 = std::time::Instant::now();
            let victim = rt
                .call_function_on_remote(
                    // The victim reports its OWN latency plus wheel/queue
                    // snapshots from both ends of the wait. q0/q1 = macrotask
                    // queue length: a big q1 at resolve means the due timer
                    // starved behind ready items (drain fairness). a0/a1 =
                    // wheel armedIn (ms to the armed sleep's deadline;
                    // negative = the armed deadline is already past = #240
                    // corpse face); sw/pat/wk = stale wakes / patrol passes /
                    // wakes DURING the victim's wait. Careful reading patrols:
                    // a healthy wheel still runs patrols for far-future arms
                    // (the fetch heartbeat), so pat>0 alone is NOT the corpse
                    // face — that is an overdue top with a stale/past a0 plus
                    // stale wakes. Drain starvation shows as q1>>0. The face
                    // this test actually caught: every counter healthy, victim
                    // still ~1.2s late = a synchronous page wall sitting on
                    // the microtask checkpoint the wake needed (the #63
                    // eval-pump fix; js latency ≈ one wall per half).
                    // rd = entries the #63 recover hook delivered
                    // synchronously — the guaranteed-progress channel when
                    // the severable wakes (wk) die with each behead.
                    r#"async () => {
                            const t0 = Date.now();
                            const s0 = __MT_STATE();
                            await new Promise((r) => setTimeout(r, 100));
                            const s1 = __MT_STATE();
                            return 'ok+' + (Date.now() - t0)
                                + ' q0=' + s0.qlen + ' q1=' + s1.qlen
                                + ' a0=' + s0.wheel.armedIn + ' a1=' + s1.wheel.armedIn
                                + ' sw=' + (s1.wheel.staleWakes - s0.wheel.staleWakes)
                                + ' pat=' + (s1.wheel.patrols - s0.wheel.patrols)
                                + ' wk=' + (s1.wheel.wakes - s0.wheel.wakes)
                                + ' rd=' + (s1.wheel.recovers - s0.wheel.recovers);
                        }"#,
                    None,
                    &[],
                    true,
                    true,
                )
                .await;
            eval_worst_ms = eval_worst_ms.max(t0.elapsed().as_millis());
            eval_samples.push(format!(
                "{:?} rust={}ms",
                victim.map(|v| v.value),
                t0.elapsed().as_millis()
            ));
        }
        let s = rt.evaluate("globalThis.__MT_STATE()").unwrap();
        let beats = s.get("beats").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let qlen = s.get("qlen").and_then(|v| v.as_f64()).unwrap_or(0.0) as usize;
        let lag = s.get("lag").and_then(|v| v.as_f64()).unwrap_or(0.0);
        worst_qlen = worst_qlen.max(qlen);
        if beats - beats_prev > 20.0 {
            furnace_slices += 1; // >200 beats in one 100ms slice
        }
        beats_prev = beats;
        samples.push(format!(
            "beats={} qlen={} lag={} inflight={} fires={}",
            beats as u64,
            qlen,
            lag as u64,
            rt.evaluate("globalThis.__inflightFetches.size")
                .unwrap()
                .as_f64()
                .unwrap_or(0.0) as u64,
            rt.watchdog_fired_total(),
        ));
        let ok = rt
            .evaluate("globalThis.__ok")
            .unwrap()
            .as_f64()
            .unwrap_or(0.0);
        let legs = rt
            .evaluate("globalThis.__legs")
            .unwrap()
            .as_f64()
            .unwrap_or(-1.0);
        let spins = rt
            .evaluate("globalThis.__spins")
            .unwrap()
            .as_f64()
            .unwrap_or(99.0);
        let churn = rt
            .evaluate("globalThis.__churnTicks")
            .unwrap()
            .as_f64()
            .unwrap_or(0.0);
        if ok >= legs && (!spins_on || spins > 20.0) && churn > churn_target {
            break;
        }
    }

    // Drain window (bounded 15s): a healthy realm settles every leg even
    // after twenty beheadings — the armed-without-opok legs never do.
    let mut settled = false;
    for _ in 0..75 {
        let _ = rt.run_event_loop_bounded(200).await;
        let ok = rt
            .evaluate("globalThis.__ok")
            .unwrap()
            .as_f64()
            .unwrap_or(0.0);
        let legs = rt
            .evaluate("globalThis.__legs")
            .unwrap()
            .as_f64()
            .unwrap_or(-1.0);
        if ok >= legs {
            settled = true;
            break;
        }
    }
    let tail = &samples[samples.len().saturating_sub(12)..];
    println!("#63 repro sample tail: {tail:?}");
    let head = &samples[..samples.len().min(18)];
    println!("#63 repro sample head: {head:?}");
    println!("#63 repro eval samples: {eval_samples:?}");
    println!(
        "#63 repro: worst_qlen={worst_qlen} furnace_slices={furnace_slices} eval_worst_ms={eval_worst_ms} fires={}",
        rt.watchdog_fired_total()
    );

    assert!(
        settled,
        "storm legs never settled — the #63 armed-no-opok signature (see sample tail above)"
    );
    assert!(
        eval_worst_ms < 5000,
        "an async eval mid-storm took {eval_worst_ms}ms — the #63 EVAL_TIMEOUT victim"
    );
    assert_eq!(
        rt.evaluate("globalThis.__diting_fetch_stalled()")
            .unwrap()
            .as_bool(),
        Some(false),
        "a settled leg lost its continuation — #240 face-2 stall"
    );
    assert_eq!(
        rt.evaluate("globalThis.__MT_STATE()")
            .unwrap()
            .get("depth")
            .and_then(|v| v.as_f64()),
        Some(0.0),
        "trampoline depth must not stay leaked after the storm"
    );
    // Probe liveness after the storm: a fresh timer must still fire.
    rt.evaluate(
        "globalThis.__probe = false; setTimeout(() => { globalThis.__probe = true; }, 100);",
    )
    .unwrap();
    let mut probed = false;
    for _ in 0..20 {
        let _ = rt.run_event_loop_bounded(200).await;
        if rt
            .evaluate("globalThis.__probe")
            .unwrap()
            .as_bool()
            .unwrap_or(false)
        {
            probed = true;
            break;
        }
    }
    assert!(probed, "probe timer never fired after the storm");
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

/// #240 face-2 detector: a fetch whose op settled (opok) but whose
/// chained continuation (resume/fthrow) never ran is the one ring state
/// a healthy realm never shows — opok and resume are one microtask
/// checkpoint apart, so an 8s gap means the continuation was lost.
/// The detector must name it once per key (the latch keeps the
/// per-loop-entry witness from re-firing on a rotted ring slot), and
/// the dump helper must bundle the ring tail with the wheel counters.
#[tokio::test(flavor = "current_thread")]
async fn fetch_ring_stall_detector_names_lost_continuation() {
    let mut rt = setup_runtime("<html><body></body></html>");
    assert_eq!(
        rt.evaluate("typeof globalThis.__diting_fetch_stalled")
            .unwrap(),
        serde_json::Value::String("function".into())
    );
    assert_eq!(
        rt.evaluate("globalThis.__diting_fetch_stalled()").unwrap(),
        serde_json::Value::Bool(false),
        "healthy realm: no settle-without-continuation"
    );

    // Manufacture the corpse: an opok recorded 9s ago with no
    // resume/fthrow after it — the exact face-2 post-mortem shape.
    rt.evaluate(
        "_fetchRing.push(['armed', 'fkZ', (Date.now() - 9000) % 100000]);
             _fetchRing.push(['opok', 'fkZ', (Date.now() - 9000) % 100000]);",
    )
    .unwrap();
    assert_eq!(
        rt.evaluate("globalThis.__diting_fetch_stalled()").unwrap(),
        serde_json::Value::Bool(true),
        "opok without resume past the 8s window must trip"
    );
    // The latch: the same key must not re-trip — the witness polls at
    // every loop entry and a rotted entry stays in the ring forever.
    assert_eq!(
        rt.evaluate("globalThis.__diting_fetch_stalled()").unwrap(),
        serde_json::Value::Bool(false),
        "a reported key must latch"
    );

    // The dump bundles the ring tail with the wheel counters — the
    // warn line an unattended flow wedge now leaves in the log.
    let dump = rt
        .evaluate("globalThis.__diting_mt_dump()")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(dump.contains("fkZ"), "dump names the stalled key: {dump}");
    assert!(
        dump.contains("wheel"),
        "dump carries the wheel counters: {dump}"
    );
}

/// #240 face 2, manufactured: a fire-and-forget fetch's await
/// continuation vanished under boot-storm load — the net layer
/// 200'd, the settlement pushed, the outer promise resolved (the
/// in-flight key was deleted) and STILL the `await` continuation
/// never ran, with zero terminates anywhere in the log. The live
/// repro is a load-gated race (heavy doudian boots wedge, light
/// ones don't), so manufacture the window instead: many
/// fire-and-forget fetches against a deliberately slow responder,
/// settling while the pump is cancelled at randomized sub-slice
/// budgets — each `run_event_loop_bounded` timeout DROPS the loop
/// future mid-poll, the suspected half-delivery moment —
/// interleaved with sync evals exactly like the wait step's poll
/// loop, mixing the plain and #66-throttled pump shapes. Every
/// fetch must land its marker; the shim's `_fetchRing` stages are
/// dumped on failure to name where a lost one died.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_fire_and_forget_survives_pump_cancellation_storm() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    const N: usize = 40;

    // Slow responder: 200-500ms think time, so settlements arrive
    // continuously while the pump is being dropped at short budgets.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for i in 0..N {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            std::thread::sleep(std::time::Duration::from_millis(200 + (i as u64 % 6) * 50));
            let body = format!("payload-{i}");
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/"));
    rt.evaluate(
        "globalThis.__hits = 0; globalThis.__rej = 0;
             for (var i = 0; i < 40; i++) {
               fetch('/slow?i=' + i)
                 .then(function (r) { return r.text(); })
                 .then(function () { globalThis.__hits++; },
                       function () { globalThis.__rej++; });
             }",
    )
    .unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let budgets = [2u64, 5, 11, 23, 47];
    let mut round = 0usize;
    loop {
        let b = budgets[round % budgets.len()];
        if round.is_multiple_of(2) {
            let _ = rt.run_event_loop_bounded(b).await;
        } else {
            let _ = rt.run_event_loop_bounded_throttled(50).await;
        }
        // The wait step's exact eval shape: sync expression under its
        // own watchdog — this is where production observed the loss.
        let _ =
            rt.evaluate_with_timeout("globalThis.__hits", std::time::Duration::from_millis(200));
        let hits = rt
            .evaluate("globalThis.__hits")
            .unwrap()
            .as_f64()
            .unwrap_or(-1.0);
        let rej = rt
            .evaluate("globalThis.__rej")
            .unwrap()
            .as_f64()
            .unwrap_or(-1.0);
        assert_eq!(rej, 0.0, "probe fetches must not reject under pump churn");
        if hits as usize == N {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "lost fire-and-forget continuations under pump cancellation: \
                 hits={hits}/{N} after 30s — ring tail: {}",
            rt.evaluate("JSON.stringify(_fetchRing.slice(-14))")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "<ring unreadable>".into())
        );
        round += 1;
    }
    // The instrumentation itself must have seen the full lifecycle.
    let ring = rt
        .evaluate("JSON.stringify(_fetchRing)")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        ring.contains("\"resp\""),
        "fetch ring must record resp stages: {ring}"
    );
}

/// (#229) Script rows used to stamp startedDateTime at walk return —
/// response completion. A request that left first but answered second
/// read as later, which is exactly the ambiguity the taobao
/// cookie-rotation race could not resolve. The recorded row now
/// carries the op-entry dispatch time, strictly earlier than the
/// completion stamp when the transport takes measurable time.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn scripted_fetch_event_records_dispatch_before_completion() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const r = await fetch('/slow');
                    return await r.text();
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    assert_eq!(result.value.unwrap(), serde_json::json!("ok"));

    let events = rt.take_js_network_events();
    let ev = events
        .iter()
        .find(|e| e.url.ends_with("/slow"))
        .expect("/slow must leave a recorded event");
    assert!(ev.started > 0.0, "dispatch stamp is unix seconds");
    assert!(
        ev.timestamp - ev.started >= 0.3,
        "dispatch must precede completion by the transport delay, got Δ={}",
        ev.timestamp - ev.started
    );
}

#[tokio::test(flavor = "current_thread")]
async fn fetch_data_and_blob_urls_resolve_locally() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // obscura #907 family: fetch()/XHR on data: and blob: URLs. These
    // never touch the HTTP client (it cannot fetch either scheme) — the
    // data: processor decodes inline bytes, and createObjectURL feeds a
    // registry fetch reads back. The old createObjectURL was a fake stub
    // whose URL nothing could resolve.
    let script = r#"async () => {
            const out = [];
            const t = await fetch('data:text/plain,hello%20w%C3%B6rld');
            out.push(t.status, t.headers.get('content-type'), await t.text());
            const j = await fetch('data:application/json;base64,eyJhIjoxfQ==');
            out.push(j.headers.get('content-type'), await j.text());
            const b = new Blob([new Uint8Array([104, 105])], { type: 'text/x-custom' });
            const u = URL.createObjectURL(b);
            out.push(typeof u === 'string' && u.startsWith('blob:'));
            const r = await fetch(u);
            out.push(r.status, r.headers.get('content-type'), await r.text());
            URL.revokeObjectURL(u);
            let revoked = false;
            try { await fetch(u); } catch (e) { revoked = true; }
            out.push(revoked);
            // XHR rides the same fetch branch.
            const xhrText = await new Promise((res, rej) => {
                const x = new XMLHttpRequest();
                x.open('GET', 'data:text/plain,xhr%21');
                x.onload = () => res(x.responseText + ':' + x.status);
                x.onerror = () => rej(new Error('xhr data: fetch failed'));
                x.send();
            });
            out.push(xhrText);
            return out;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            200,
            "text/plain",
            "hello wörld",
            "application/json",
            "{\"a\":1}",
            true,
            200,
            "text/x-custom",
            "hi",
            true,
            "xhr!:200"
        ])
    );
}

/// #62: transport-layer failures surface to the page exactly as Chrome
/// delivers them — a TypeError whose message is exactly "Failed to fetch".
/// The raw engine reason ("error sending request for uri ...") used to ride
/// the rejection, a string no real browser produces and a face tell on any
/// page that inspects e.message; the detailed reason belongs to the network
/// event, not the promise.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn transport_error_fetch_rejects_with_chrome_typeerror_face() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Bind then drop: a port nothing listens on, so the connection is
    // refused at the transport layer (no CORS leg involved — simple GET).
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        await fetch("http://127.0.0.1:PORT/api");
                        return "not-blocked";
                    } catch (e) {
                        return [e && e.name, e && e.message, e instanceof TypeError]
                            .map(v => String(v)).join("|");
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let face = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    assert_eq!(face, "TypeError|Failed to fetch|true", "got: {face}");

    // The observability split Chrome achieves with the console, we keep in
    // the network event: the refusal records status 0 with the real reason.
    let events = rt.take_js_network_events();
    let refused = events
        .iter()
        .find(|e| e.url.contains(&format!(":{port}/api")))
        .expect("refused fetch must leave a network event");
    assert_eq!(refused.status, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn test_fetch_url_input_decodes_binary_body_base64() {
    // Serves a binary body from a real local server: the bootstrap deletes
    // the `Deno` global (stealth), so the op cannot be monkey-patched from
    // JS. URL-object input resolves against document.URL, and the binary
    // body must reach JS intact via the op's base64 envelope.
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (path_tx, path_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..n]);
        let path = request.lines().next().unwrap_or("").to_string();
        let body = [0u8, 97, 115, 109, 1, 0, 0, 0];
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/wasm\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        stream.flush().unwrap();
        path_tx.send(path).unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const response = await fetch(new URL("/pkg/app_bg.wasm", document.URL));
                return {
                    status: response.status,
                    bytes: Array.from(new Uint8Array(await response.arrayBuffer())),
                };
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "status": 200,
            "bytes": [0, 97, 115, 109, 1, 0, 0, 0],
        })
    );
    let request_line = path_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        request_line.starts_with("GET /pkg/app_bg.wasm "),
        "server should see the resolved URL path, got: {}",
        request_line
    );
}

/// obscura#664 class: the fetch/XHR redirect budget is the Fetch spec's
/// fixed 20 — WPT `fetch/api/redirect/redirect-count.any.js` pins both
/// ends: the 20th hop succeeds, the 21st fails. (HTTP-3xx during
/// document navigation and the JS navigation-chain document cap are
/// separate budgets; the chain cap lives in diting_browser/page.rs and
/// counts documents, not redirects.)
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_redirect_count_matches_wpt_pair() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // /r/N → 302 → /r/{N-1}; /r/0 is the terminal document.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..48 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let path = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/r/0")
                .to_string();
            let resp = match path.strip_prefix("/r/").and_then(|n| n.parse::<u32>().ok()) {
                    Some(0) => "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 9\r\nconnection: close\r\n\r\nchain-end"
                        .to_string(),
                    Some(n) => format!(
                        "HTTP/1.1 302 Found\r\nlocation: /r/{}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                        n - 1
                    ),
                    None => "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                        .to_string(),
                };
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/r/0", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const outcomes = {};
                    try {
                        const r = await fetch(new URL("/r/20", document.URL));
                        outcomes.twenty = r.status + ":" + (await r.text());
                    } catch (e) { outcomes.twenty = "rejected"; }
                    try {
                        const r = await fetch(new URL("/r/21", document.URL));
                        outcomes.twentyOne = r.status + ":" + (await r.text());
                    } catch (e) { outcomes.twentyOne = "rejected"; }
                    return outcomes;
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let outcomes = result.value.unwrap();
    assert_eq!(
        outcomes["twenty"],
        serde_json::json!("200:chain-end"),
        "the 20th redirect hop must still succeed (http-redirect-fetch step 7: count 20 passes)"
    );
    assert_eq!(
        outcomes["twentyOne"],
        serde_json::json!("rejected"),
        "the 21st redirect hop must fail (count 21 → network error)"
    );
}

/// Upstream #581 class: op_fetch_url buffered the entire response with
/// `.bytes().await` before any limit was consulted, so page JS fetching
/// a multi-GB body OOMed the process (the retained-for-CDP byte limit
/// only gates the cache, not the allocation). The cap must reject as a
/// network-style failure — both when Content-Length advertises it and
/// when an unbounded stream crosses it mid-body.
#[tokio::test(flavor = "current_thread")]
async fn fetch_rejects_response_body_over_limit() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    std::env::set_var("DITING_FETCH_BODY_LIMIT", "1024");

    // Advertised Content-Length over the cap: rejected before buffering.
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 99999999\r\nconnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        });
        let mut rt = setup_runtime("<html><body></body></html>");
        rt.set_url(&format!("http://127.0.0.1:{}/huge", port));
        let result = rt
            .call_function_on_remote(
                r#"async () => {
                        try { await fetch(document.URL); return "resolved"; }
                        catch (e) { return "rejected"; }
                    }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            result.value.unwrap(),
            serde_json::json!("rejected"),
            "oversized advertised Content-Length must reject the fetch"
        );
    }

    // No Content-Length: the stream itself must cross the cap and reject.
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\nconnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            // 8 KiB in small writes: an unbounded stream with no framing.
            for _ in 0..64 {
                if stream.write_all(&[b'x'; 128]).is_err() {
                    break; // client hung up once the cap tripped
                }
            }
            let _ = stream.flush();
        });
        let mut rt = setup_runtime("<html><body></body></html>");
        rt.set_url(&format!("http://127.0.0.1:{}/stream", port));
        let result = rt
            .call_function_on_remote(
                r#"async () => {
                        try { await fetch(document.URL); return "resolved"; }
                        catch (e) { return "rejected"; }
                    }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            result.value.unwrap(),
            serde_json::json!("rejected"),
            "unbounded stream over the cap must reject the fetch"
        );
    }

    std::env::remove_var("DITING_FETCH_BODY_LIMIT");
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

/// Browsers send Origin on every non-GET/HEAD request, including
/// same-origin POSTs (SolidStart server functions 403 without it).
/// Regression: we only set Origin cross-origin, so a same-origin POST
/// reached the wire bare and got rejected.
#[tokio::test(flavor = "current_thread")]
async fn fetch_same_origin_post_sends_origin_header() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (hdr_tx, hdr_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..n]);
        let origin_line = request
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("origin:"))
            .unwrap_or("")
            .to_string();
        let body = b"{}";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
        hdr_tx.send(origin_line).unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/submit", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const r = await fetch(new URL("/_serverFn/x", document.URL), {
                    method: "POST",
                    headers: { "_h": "{\"x-tsr-serverfn\":\"true\"}" },
                    body: JSON.stringify({ _d: [["name", "Diting"]] }),
                });
                return { status: r.status };
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(result.value.unwrap(), serde_json::json!({ "status": 200 }));
    let origin_line = hdr_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        origin_line
            .to_ascii_lowercase()
            .starts_with("origin: http://127.0.0.1:"),
        "same-origin POST must carry Origin, got: {:?}",
        origin_line
    );
}

/// Browsers send Fetch-Metadata (sec-fetch-*) and client-hint headers on
/// scripted requests; WAFs key on `sec-fetch-site: same-origin` and 403
/// requests without them. Regression: op_fetch_url only set User-Agent.
#[tokio::test(flavor = "current_thread")]
async fn fetch_same_origin_post_sends_fetch_metadata_headers() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (hdr_tx, hdr_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..n]);
        let lower = request.to_ascii_lowercase();
        let hdrs = [
            "sec-fetch-site",
            "sec-fetch-mode",
            "sec-fetch-dest",
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "accept",
        ]
        .iter()
        .map(|h| {
            let v = lower
                .lines()
                .find(|l| l.starts_with(&format!("{}:", h)))
                .unwrap_or("")
                .trim()
                .to_string();
            (h.to_string(), v)
        })
        .collect::<std::collections::HashMap<_, _>>();
        let body = b"{}";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
        hdr_tx.send(hdrs).unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/submit", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const r = await fetch(new URL("/api", document.URL), {
                    method: "POST",
                    body: "{}",
                });
                return { status: r.status };
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(result.value.unwrap(), serde_json::json!({ "status": 200 }));
    let hdrs = hdr_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        hdrs.get("sec-fetch-site").map(String::as_str),
        Some("sec-fetch-site: same-origin")
    );
    assert_eq!(
        hdrs.get("sec-fetch-mode").map(String::as_str),
        Some("sec-fetch-mode: cors")
    );
    assert_eq!(
        hdrs.get("sec-fetch-dest").map(String::as_str),
        Some("sec-fetch-dest: empty")
    );
    assert_eq!(
        hdrs.get("sec-ch-ua-mobile").map(String::as_str),
        Some("sec-ch-ua-mobile: ?0")
    );
    assert!(
        hdrs.get("sec-ch-ua")
            .map(|s| {
                let l = s.to_ascii_lowercase();
                l.contains("chromium") && l.contains("google chrome")
            })
            .unwrap_or(false),
        "sec-ch-ua missing, got: {:?}",
        hdrs.get("sec-ch-ua")
    );
    assert!(hdrs
        .get("accept")
        .map(|s| s.contains("*/*"))
        .unwrap_or(false));
}

/// The Fetch standard allows 20 redirect hops and rejects the 21st
/// (upstream 4b90ec3). A local chain of exactly 20 must arrive; one of 21
/// must fail.
#[tokio::test(flavor = "current_thread")]
async fn fetch_follows_twenty_redirects_and_rejects_twenty_one() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    fn chain_server(hops: usize) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for _ in 0..=hops {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/");
                let step: usize = path.trim_start_matches("/hop").parse().unwrap_or(0);
                let response = if step < hops {
                    format!(
                        "HTTP/1.1 302 Found\r\nlocation: /hop{}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                        step + 1
                    )
                } else {
                    "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
                        .to_string()
                };
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        port
    }

    let fetch_status = |port: u16| {
        let rt = setup_runtime("<html><body></body></html>");
        rt.set_url(&format!("http://127.0.0.1:{}/", port));
        rt
    };
    let script = r#"async () => {
            try {
                const r = await fetch("/hop0");
                return "status:" + r.status;
            } catch (e) {
                return "error:" + (e && e.message);
            }
        }"#;

    let port20 = chain_server(20);
    let mut rt = fetch_status(port20);
    let ok = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(ok.value.unwrap(), serde_json::json!("status:200"));

    let port21 = chain_server(21);
    let mut rt = fetch_status(port21);
    let err = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        err.value.unwrap(),
        serde_json::json!("error:Failed to fetch")
    );

    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

/// fetch() must serialize FormData (incl. File parts with filename and
/// Content-Type), Blob, and TypedArray bodies the way a browser does
/// (upstream 3eb28da / 260c4c0). String(body) used to send "[object Blob]".
#[tokio::test(flavor = "current_thread")]
async fn fetch_serializes_formdata_blob_and_typed_bodies() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (req_tx, req_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..4 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut raw = Vec::new();
            let mut buf = [0u8; 4096];
            // Read headers, then exactly content-length body bytes.
            let mut header_end = None;
            let mut content_len = 0usize;
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
                if header_end.is_none() {
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = Some(pos + 4);
                        let head = String::from_utf8_lossy(&raw[..pos]);
                        for line in head.lines() {
                            if let Some(v) =
                                line.to_ascii_lowercase().strip_prefix("content-length:")
                            {
                                content_len = v.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                }
                if let Some(end) = header_end {
                    if raw.len() >= end + content_len {
                        break;
                    }
                }
            }
            req_tx.send(raw).unwrap();
            let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
            let _ = stream.flush();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/", port));
    let result = rt.call_function_on_remote(
            r#"async (port) => {
                const out = [];
                const run = async (tag, fn) => { try { out.push(tag + ":" + (await fn()).status); } catch (e) { out.push(tag + "!:" + (e && (e.message || e.name))); } };
                const fd = new FormData();
                fd.append("field", "value");
                fd.append("upload", new File([new Uint8Array([1, 2, 3])], "a.bin", { type: "application/octet-stream" }));
                await run("plain", () => fetch("http://127.0.0.1:" + port + "/plain", { method: "POST", body: "x=1" }));
                await run("fd", () => fetch("http://127.0.0.1:" + port + "/fd", { method: "POST", body: fd }));
                await run("blob", () => fetch("http://127.0.0.1:" + port + "/blob", { method: "POST", body: new Blob(["hello"], { type: "text/plain" }) }));
                await run("typed", () => fetch("http://127.0.0.1:" + port + "/typed", { method: "POST", body: new Uint8Array([65, 66, 67]) }));
                return out.join("|");
            }"#,
            None,
            &[serde_json::json!({ "value": port })],
            true,
            true,
        ).await.unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("plain:200|fd:200|blob:200|typed:200")
    );

    let plain_raw = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        plain_raw.ends_with(b"x=1"),
        "plain body mismatch: {:?}",
        plain_raw
    );

    let fd_req = String::from_utf8_lossy(
        &req_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
    )
    .into_owned();
    assert!(
        fd_req.contains("content-type: multipart/form-data; boundary="),
        "missing multipart header: {}",
        fd_req
    );
    assert!(
        fd_req.contains("name=\"field\"\r\n\r\nvalue"),
        "missing field part: {}",
        fd_req
    );
    assert!(
        fd_req.contains("filename=\"a.bin\""),
        "missing filename: {}",
        fd_req
    );
    assert!(
        fd_req.contains("application/octet-stream"),
        "missing part content-type: {}",
        fd_req
    );

    let blob_raw = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let blob_req = String::from_utf8_lossy(&blob_raw).into_owned();
    assert!(
        blob_req.contains("content-type: text/plain"),
        "missing blob content-type: {}",
        blob_req
    );
    assert!(
        blob_req.ends_with("hello"),
        "blob body mismatch: {}",
        blob_req
    );

    let typed_raw = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        typed_raw.ends_with(b"ABC"),
        "typed body mismatch: {:?}",
        typed_raw
    );
}

/// Binary request bodies must arrive byte-exact. The deno `#[string]`
/// boundary used to UTF-8-encode the Latin-1 binary-string body channel,
/// corrupting `[0,128,255]` into `[0,194,128,195,191]` (upstream obscura
/// #716). Bodies are now base64-encoded in the JS shim (ASCII-safe across
/// that boundary) and decoded in `op_fetch_url`, so non-ASCII bytes survive
/// intact across the typed-array, Blob and multipart File paths.
#[tokio::test(flavor = "current_thread")]
async fn fetch_binary_bodies_are_byte_exact() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (req_tx, req_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..3 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut raw = Vec::new();
            let mut buf = [0u8; 4096];
            let mut header_end = None;
            let mut content_len = 0usize;
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
                if header_end.is_none() {
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = Some(pos + 4);
                        let head = String::from_utf8_lossy(&raw[..pos]);
                        for line in head.lines() {
                            if let Some(v) =
                                line.to_ascii_lowercase().strip_prefix("content-length:")
                            {
                                content_len = v.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                }
                if let Some(end) = header_end {
                    if raw.len() >= end + content_len {
                        break;
                    }
                }
            }
            req_tx.send(raw).unwrap();
            let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
            let _ = stream.flush();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/", port));
    let result = rt.call_function_on_remote(
            r#"async (port) => {
                const u8 = new Uint8Array([0, 128, 255, 16]);
                await fetch("http://127.0.0.1:" + port + "/typed", { method: "POST", body: u8 });
                await fetch("http://127.0.0.1:" + port + "/blob", { method: "POST", body: new Blob([u8]) });
                const fd = new FormData();
                fd.append("f", new File([u8], "b.bin", { type: "application/octet-stream" }));
                await fetch("http://127.0.0.1:" + port + "/fd", { method: "POST", body: fd });
                return "ok";
            }"#,
            None,
            &[serde_json::json!({ "value": port })],
            true,
            true,
        ).await.unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    assert_eq!(result.value.unwrap(), serde_json::json!("ok"));

    let body_bytes = |raw: &[u8]| -> Vec<u8> {
        let pos = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| p + 4)
            .unwrap_or(0);
        raw[pos..].to_vec()
    };

    let typed = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        body_bytes(&typed),
        vec![0, 128, 255, 16],
        "typed array body corrupted: {:?}",
        &typed[typed.len().saturating_sub(16)..]
    );

    let blob = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        body_bytes(&blob),
        vec![0, 128, 255, 16],
        "blob body corrupted"
    );

    let fd = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let needle = [0u8, 128, 255, 16];
    assert!(
        fd.windows(4).any(|w| w == needle),
        "multipart file part corrupted: {:?}",
        &fd[fd.len().saturating_sub(48)..]
    );
}

/// Request.formData() / Response.formData(): react-router and remix route
/// actions open every form submission with request.formData() — the missing
/// method wedged the client action chain behind "e.formData is not a
/// function" (glama.ai admin form). Covers the FormData-body passthrough,
/// urlencoded pair parsing, multipart parts (File entries), and the spec
/// TypeError on a non-form content-type.
#[tokio::test(flavor = "current_thread")]
async fn request_response_formdata_parse() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(r#"
            globalThis.__out = [];
            const fd = new FormData();
            fd.append("intent", "build");
            fd.append("steps", '["a","b"]');
            new Request("https://x.test/post", { method: "POST", body: fd })
                .formData()
                .then((f) => {
                    __out.push(f.get("intent") === "build", f.get("steps") === '["a","b"]');
                    // react-router converts file-less form submissions to a
                    // URLSearchParams body (default urlencoded encType) and the
                    // browser infers the content-type — the engine must too.
                    const uspReq = new Request("https://x.test/post", { method: "POST", body: new URLSearchParams("a=1&b=x+y") });
                    __out.push(uspReq.headers.get("content-type") === "application/x-www-form-urlencoded;charset=UTF-8");
                    return uspReq.formData();
                })
                .then((f) => {
                    __out.push(f.get("a") === "1", f.get("b") === "x y");
                    return new Request("https://x.test/post", {
                        method: "POST",
                        headers: { "content-type": "application/x-www-form-urlencoded;charset=UTF-8" },
                        body: "a=1&b=x+y&c=%E4%B8%AD",
                    }).formData();
                })
                .then((f) => {
                    __out.push(f.get("a") === "1", f.get("b") === "x y", f.get("c") === "中");
                    const b = "----ditingT";
                    const mp = [
                        "--" + b,
                        'Content-Disposition: form-data; name="field"',
                        "",
                        "plain",
                        "--" + b,
                        'Content-Disposition: form-data; name="up"; filename="a.bin"',
                        "Content-Type: application/octet-stream",
                        "",
                        "BIN",
                        "--" + b + "--",
                        "",
                    ].join("\r\n");
                    return new Response(mp, {
                        headers: { "content-type": "multipart/form-data; boundary=" + b },
                    }).formData();
                })
                .then((f) => {
                    const up = f.get("up");
                    __out.push(f.get("field") === "plain", up instanceof File, up.name === "a.bin", up.type === "application/octet-stream");
                    return new Request("https://x.test/post", {
                        method: "POST", body: "hi", headers: { "content-type": "text/plain" },
                    }).formData().then(() => "no-throw", (e) => (e instanceof TypeError ? "TypeError" : "other"));
                })
                .then((v) => { __out.push(v); })
                .catch((e) => { __out.push("ERR:" + (e && (e.message || e.name))); });
        "#).unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let result = rt.evaluate("globalThis.__out").unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            "TypeError"
        ])
    );
}

/// RequestCredentials end-to-end (upstream b744b9b): same-origin (the
/// default) neither sends nor stores cookies cross-origin; "include" does
/// both, and a credentialed CORS response without Allow-Credentials +
/// exact origin is blocked.
#[tokio::test(flavor = "current_thread")]
async fn fetch_honors_request_credentials_across_origins() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    use std::io::{Read, Write};
    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
    fn cookie_header(req: &str) -> String {
        req.lines()
            .find(|l| l.to_ascii_lowercase().starts_with("cookie:"))
            .map(|l| l[7..].trim().to_string())
            .unwrap_or_default()
    }

    // Page origin: stores a cookie so the same-origin store path runs.
    let listener_a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_a = listener_a.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener_a.accept().unwrap();
        read_request(&mut stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\nset-cookie: a=1; Path=/\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").unwrap();
    });

    // Cross origin B: mirrors CORS for the page origin, sets b=1 each time.
    let listener_b = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_b = listener_b.local_addr().unwrap().port();
    let (cookie_tx, cookie_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let origin = format!("http://127.0.0.1:{}", port_a);
        for _ in 0..3 {
            let Ok((mut stream, _)) = listener_b.accept() else {
                return;
            };
            let req = read_request(&mut stream);
            cookie_tx.send(cookie_header(&req)).unwrap();
            let resp = format!(
                "HTTP/1.1 200 OK\r\naccess-control-allow-origin: {}\r\naccess-control-allow-credentials: true\r\nset-cookie: b=1; Path=/\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                origin
            );
            stream.write_all(resp.as_bytes()).unwrap();
        }
    });

    // Cross origin C: wildcard ACAO without Allow-Credentials — fine for
    // non-credentialed, blocked for credentials:include.
    let listener_c = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_c = listener_c.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener_c.accept().unwrap();
        read_request(&mut stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").unwrap();
    });

    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port_a));
    let result = rt.call_function_on_remote(
            r#"async (pa, pb, pc) => {
                const A = "http://127.0.0.1:" + pa, B = "http://127.0.0.1:" + pb, C = "http://127.0.0.1:" + pc;
                const out = [];
                await fetch(A + "/seed");
                out.push("r1:" + (await fetch(B + "/x")).status);
                out.push("r2:" + (await fetch(B + "/x", { credentials: "include" })).status);
                out.push("r3:" + (await fetch(B + "/x", { credentials: "include" })).status);
                try {
                    await fetch(C + "/x", { credentials: "include" });
                    out.push("c:ok");
                } catch (e) {
                    out.push("c:" + (e && e.message));
                }
                return out.join("|");
            }"#,
            None,
            &[
                serde_json::json!({ "value": port_a }),
                serde_json::json!({ "value": port_b }),
                serde_json::json!({ "value": port_c }),
            ],
            true,
            true,
        ).await.unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let expected = "r1:200|r2:200|r3:200|c:Failed to fetch".to_string();
    assert_eq!(result.value.unwrap(), serde_json::json!(expected));
    let c1 = cookie_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let c2 = cookie_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let c3 = cookie_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    // Cookies are host-scoped (RFC 6265 ignores the port), so once
    // credentials are allowed, B receives every 127.0.0.1 cookie.
    assert_eq!((c1.as_str(), c2.as_str()), ("", "a=1"));
    assert!(
        c3.split("; ").any(|c| c == "b=1"),
        "stored cookie missing: {}",
        c3
    );
    let b_url = url::Url::parse(&format!("http://127.0.0.1:{}/", port_b)).unwrap();
    assert!(jar
        .get_cookie_header(&b_url)
        .split("; ")
        .any(|c| c == "b=1"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_response_array_buffer_preserves_typed_array_view() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const bytes = new Uint8Array([9, 0, 97, 115, 109, 1, 8]);
                const response = new Response(bytes.subarray(1, 6));
                return Array.from(new Uint8Array(await response.arrayBuffer()));
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([0, 97, 115, 109, 1])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_wasm_instantiate_streaming_uses_response_array_buffer() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const bytes = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
                const result = await WebAssembly.instantiateStreaming(
                    Promise.resolve(new Response(bytes)),
                    {},
                );
                return result.instance instanceof WebAssembly.Instance;
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    assert_eq!(result.value.unwrap(), serde_json::json!(true));
}

#[tokio::test(flavor = "current_thread")]
async fn test_request_and_response_keep_binary_bodies_intact() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.call_function_on_remote(
            r#"async () => {
                const pb = new Uint8Array([0xEF, 0xBF, 0xBD, 0x37, 0x00, 0xFF, 0x80, 0x10]);
                const hex = (ab) => Array.from(new Uint8Array(ab)).map(b => b.toString(16)).join(',');
                const want = hex(pb);
                const out = [];
                for (const body of [pb, pb.buffer, new Blob([pb])]) {
                    const r = new Request('http://x/', { method: 'POST', body });
                    out.push(hex(await r.clone().arrayBuffer()) === want);
                    out.push(hex(await r.clone().bytes()) === want);
                    out.push(hex(await (await r.clone().blob()).arrayBuffer()) === want);
                }
                out.push(hex(await new Response(pb).bytes()) === want);
                return out.every(Boolean);
            }"#,
            None,
            &[],
            true,
            true,
        ).await.unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(true));
}

#[tokio::test(flavor = "current_thread")]
async fn test_local_fetch_resolves_on_task_boundary() {
    // data:/blob: fetch must resolve on a task, never inline as a microtask.
    // Resolving inline let `while (true) { await fetch('data:...') }` run
    // ~27k iterations/sec inside one microtask drain, starving every timer
    // and event on the page (measured: 50ms timer dead for 3+ seconds
    // while 111k inline fetches resolved in 4s). Chrome delivers even a
    // local response on a task; the 50ms timer armed before the cascade
    // must fire WHILE the cascade runs, not after it drains.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const p = new Promise((resolve) => {
                let fired = null;
                const t0 = performance.now();
                setTimeout(() => { fired = performance.now() - t0; }, 50);
                (async () => {
                    // Time-boxed, not count-boxed: a fixed 2000 fetches took
                    // longer than the eval budget on a slow CI runner.
                    while (performance.now() - t0 < 400) { await fetch('data:text/plain,x'); }
                    resolve({ fired: fired === null ? -1 : Math.round(fired) });
                })();
            });
            return Promise.race([p, new Promise((r) => setTimeout(() => r(null), 10000))]);
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert!(v.is_object(), "cascade must finish inside 10s");
    let fired = v["fired"].as_i64().unwrap();
    // fired == -1 means the cascade drained before the timer ran (the
    // starvation bug); an absolute upper bound would flake on slow CI.
    assert!(
        fired >= 50,
        "50ms timer must fire mid-cascade (task-per-fetch), fired={}ms",
        fired
    );
}

/// #203 root cause regression: `sec-fetch-site` must follow the
/// initiator's SITE (registrable domain), not full-origin equality. A
/// sibling-origin fetch (127.0.0.1:A → 127.0.0.1:B — the loopback
/// stand-in for item.upload.taobao.com → everyhelp.taobao.com) is
/// `same-site`; the old origin-equality code labeled it `cross-site`, a
/// combination real Chrome never produces while carrying that site's Lax
/// login cookies, and taobao risk control answered with a defensive
/// logout. localhost vs 127.0.0.1 is genuinely cross-site.
#[tokio::test(flavor = "current_thread")]
async fn fetch_sec_fetch_site_uses_site_not_origin() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let probe = |host: &str| {
        let listener = std::net::TcpListener::bind((host, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = b"{}";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body);
            let _ = stream.flush();
            tx.send(request).unwrap();
        });
        (port, rx)
    };

    let (doc_port, doc_rx) = probe("127.0.0.1");
    let (sibling_port, sibling_rx) = probe("127.0.0.1");
    let (foreign_port, foreign_rx) = probe("localhost");

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/doc", doc_port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    // Cross-origin without ACAO → the promise rejects after
                    // the request itself has gone out; the probe on the wire
                    // is the assertion surface, not the response.
                    try { await fetch("http://127.0.0.1:SIBLING/api"); } catch (e) {}
                    try { await fetch("http://localhost:FOREIGN/api"); } catch (e) {}
                    return 1;
                }"#
            .replace("SIBLING", &sibling_port.to_string())
            .replace("FOREIGN", &foreign_port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(result.value.unwrap(), serde_json::json!(1));
    let sibling_req = sibling_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let line = sibling_req
        .to_ascii_lowercase()
        .lines()
        .find(|l| l.starts_with("sec-fetch-site:"))
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        line.trim(),
        "sec-fetch-site: same-site",
        "sibling-origin request must self-describe same-site, request:\n{sibling_req}"
    );
    let foreign_req = foreign_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let line = foreign_req
        .to_ascii_lowercase()
        .lines()
        .find(|l| l.starts_with("sec-fetch-site:"))
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        line.trim(),
        "sec-fetch-site: cross-site",
        "different-site request must self-describe cross-site, request:\n{foreign_req}"
    );
    let _ = doc_rx;
}

/// M4 knife 15: Headers (headers-probe.js, 75 keys, real Chrome 145): the
/// construction corpus (record / sequence / Headers-like / bad shapes with
/// exact wording), name/value validity and normalization, same-name combining,
/// sorted LIVE iteration, set-cookie separation + getSetCookie, the iterator
/// protocol, JSON/own shape.
/// (the four Request/Response cross-keys went green with knife 16.)
#[tokio::test(flavor = "current_thread")]
async fn headers_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/headers-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/headers-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 16: Request / Response (reqres-probe.js, 96 keys, real Chrome 145):
/// prototype shape and member order, `headers` as a prototype getter, construction
/// validation with exact wording (URL parse / credentials, method tokens and
/// forbidden methods, enum members, GET/HEAD body, mode navigate, referrer,
/// signal, status range and null-body statuses, statusText), body-derived
/// Content-Type, header guards (forbidden request headers, no-cors safelist,
/// Response Set-Cookie filtering), Response.error/redirect/json, clone and tee,
/// bodyUsed / locked / disturbed semantics and the byte readers.
#[tokio::test(flavor = "current_thread")]
async fn reqres_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/reqres-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/reqres-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
