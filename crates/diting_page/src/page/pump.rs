//! Idle-pump slice machinery: the session loop's event-loop slices, the #66
//! busy-window accounting, the command-probe token bucket and the wall-clock
//! burst rate limit. Split from navigate.rs (11f) when it crossed the
//! god-file cap; behavior unchanged.

use super::Page;

impl Page {
    /// One background event-loop slice for the idle session loop: pump the
    /// JS event loop for up to `ms`; once the loop goes quiescent, park
    /// until the slice deadline. A real browser's main thread never stops
    /// between user actions - timers, fetch callbacks and promise chains
    /// keep firing. Our sessions previously froze the loop between commands
    /// (blocking `recv()`), which stalled collectors with their own
    /// deadlines: WorkOS Radar's 5s worker-response window expired while
    /// its 5s timer sat un-pumped (measured 31s frozen). Cancellation-safe
    /// at slice boundaries - the caller drops this future via `select!`
    /// when a command arrives, same as the `settle_until_idle` timeout path.
    pub async fn pump_event_loop_slice(&mut self, ms: u64) {
        if ms == 0 {
            return;
        }
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(ms);
        if self.busy_frozen {
            // Token-bucket weighting of the command probe (11f). The probe
            // used to lift the freeze outright until the trailing window
            // re-tripped — and the window needs a fresh 30s measurement, so
            // EVERY command bought the realm 30s+ of full speed: a polling
            // client (waitForFunction, an agent loop) held a burner at 98%
            // indefinitely, the hours-long spin incident. Now a probe slice
            // spends wall time 1:1 from a budget that re-accrues at only a
            // quarter of wall while frozen: the lift's aggregate share of
            // wall time caps at 25% no matter how fast commands arrive,
            // while sparse interactive commands (tmall's clicks) still find
            // a full budget waiting. 250ms minimum grant: below that the
            // slice cannot even finish a meaningful burst, so the command
            // waits for the budget instead of running a useless probe.
            const PROBE_CAP_NS: u64 = 4_000_000_000;
            const PROBE_MIN_NS: u64 = 250_000_000;
            let now = tokio::time::Instant::now();
            let tick = self.probe_tick.unwrap_or(now);
            self.probe_tick = Some(now);
            let wall_ns = (now - tick).as_nanos() as u64;
            self.probe_budget_ns = (self.probe_budget_ns + wall_ns / 4).min(PROBE_CAP_NS);
            if self.busy_probe_pending && self.probe_budget_ns >= PROBE_MIN_NS {
                // #66 command probe (2026-11-02, tmall publish): the freeze
                // only throttles the idle pump, but a command's async
                // afterlife — fetch → callback → setState → dialog render —
                // runs entirely in these slices. tmall's publish editor
                // (6.4s flush() queues, sustained >40% duty, five freezes in
                // one edit session) re-feeds its queue, so the #240
                // cool-burst never fired and the realm sat at 25% for life:
                // clicks dispatched, the 图片空间 picker never rendered.
                // Chrome never throttles a page the user is driving; neither
                // do we — the command buys full speed, priced by the bucket.
                self.busy_probe_pending = false;
                tracing::info!("#66: command on a frozen realm — full-speed probe window");
                // Priced upfront, cancellation-safe: the session loop drops
                // this future when the next command preempts the slice, so
                // settling the cost after the await would never run — and a
                // polling client would grant itself an undrained budget
                // (measured: steady 77% duty with the trailing deduction).
                // The price is the slice's full length; a burner never goes
                // quiescent inside it, so wall time is the honest measure.
                let price = tokio::time::Duration::from_millis(ms).as_nanos() as u64;
                self.probe_budget_ns = self.probe_budget_ns.saturating_sub(price);
                self.pump_slice_unthrottled(deadline).await;
                return;
            }
            // Fall through to the throttled burst below — a pending probe
            // stays armed so a later slice grants it once the budget fills.
            // #66: the busy window tripped for this realm. Throttle, don't
            // park: a parking freeze turned heavy-but-legitimate SPAs into
            // vegetables (doudian's create page measures 83-88% duty for 30s+
            // after boot — real work, just slow in this engine) because a
            // parked realm never runs the JS that completes module loads, so
            // the page stuck half-rendered and every later fetch hung (#203).
            // Chrome throttles heavy pages rather than killing them: a 50ms
            // bounded work burst, then park out the rest of the slice ≈ 25%
            // duty. Forward progress continues (~4× slower); a burner is
            // contained to a quarter core. Each burst stays inside the
            // watchdog-guarded path, and commands still preempt via the
            // session select!.
            //
            // #240: the freeze used to be sticky until Navigate/SetContent
            // rebuilt the realm — once tripped (often during a heavy boot),
            // the session stayed at 25% for life even after the page cooled.
            // A cool burst (V8 active ≪ the 50ms budget) means the burner
            // stopped self-feeding; clear without a document swap. A still-
            // hot burner saturates the budget and stays throttled.
            // 11f wall-clock rate limit: the burst/park ratio only shapes
            // 25% duty when slices complete, and a preempted slice dies
            // after the burst but before the park — polling clients turned
            // the throttle into a burst-only loop at ~75% duty (measured).
            // One burst per 200ms of wall whatever the slicing: 50ms work
            // over a 200ms spacing is the same quarter core, churn-proof.
            let burst_due = self
                .throttled_burst_mark
                .is_none_or(|m| m.elapsed() >= tokio::time::Duration::from_millis(200));
            let work_ms = 50u64.min(ms);
            if burst_due {
                self.throttled_burst_mark = Some(tokio::time::Instant::now());
            }
            if burst_due {
                if let Some(js) = &mut self.js {
                    // Throttled variant (#224): the 50ms host budget ends at
                    // the next task boundary; only one never-ending task hits
                    // the pump family's spin fence.
                    let before = js.v8_active_ns();
                    let _ = js.run_event_loop_bounded_throttled(work_ms).await;
                    let gained = js.v8_active_ns().saturating_sub(before);
                    // 10ms: well under a saturated 50ms burst, well above a quiet
                    // page's housekeeping. Matches the "cool window clears storm
                    // backoff" posture — recovery without realm rebuild.
                    if gained < 10_000_000 {
                        self.busy_frozen = false;
                        self.busy_mark = None;
                        tracing::info!(
                        "#66: realm cooled (burst used {}µs) — unfreezing without document swap",
                        gained / 1_000
                    );
                    }
                }
            }
            tokio::time::sleep_until(deadline).await;
            return;
        }
        if let Some(hot_until) = self.storm_hot_until {
            if hot_until > tokio::time::Instant::now() {
                // Storming page (watchdog-terminated earlier): throttle, not
                // park. A zero-pump slice starved the page's OWN timers —
                // and the wait loop rides this pump, so a flow polling a
                // predicate killed exactly the fire-and-forget work it was
                // waiting on (doudian's abort timer and fetch callback never
                // delivered, xhs CAS's post-success navigation stuck; the
                // eval-await path pumps through resolve_promises_until and
                // never sees this branch, which is why it stayed immune —
                // #240). One bounded 50ms burst, same shape as the freeze
                // below, keeps a re-fed runaway capped at 100ms of spin; the
                // session command loop still preempts at the slice's await
                // points.
                let work_ms = 50u64.min(ms);
                if let Some(js) = &mut self.js {
                    let _ = js.run_event_loop_bounded_throttled(work_ms).await;
                }
                tokio::time::sleep_until(deadline).await;
                return;
            }
        }
        self.pump_slice_unthrottled(deadline).await;
    }

    /// The unthrottled body of one pump slice: feed the JS event loop until
    /// the deadline, parking early once the page goes quiescent. Shared by
    /// the ordinary path and a granted #66 probe window — the probe caller
    /// prices the slice out of the token bucket after it returns.
    async fn pump_slice_unthrottled(&mut self, deadline: tokio::time::Instant) {
        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return;
            }
            let remaining = (deadline - now).as_millis() as u64;
            let idle = self.settle_until_idle(remaining).await;
            if self.track_busy_window() {
                // Frozen mid-slice: stop feeding the loop for this slice.
                return;
            }
            if idle {
                // Quiescent: park for the rest of the slice. The session
                // command loop races this against command arrival, so a
                // new command preempts the park immediately.
                tokio::time::sleep_until(deadline).await;
                return;
            }
        }
    }

    /// #66 busy accounting for the idle pump: diff the runtime's cumulative
    /// `v8_active_ns` over a trailing window of `js_busy_limit_secs()`
    /// (default 30s, 0 disables). Freeze when ≥40% of the window's wall
    /// time was spent inside `poll_event_loop` — that is a realm whose JS
    /// never yields and never overruns, i.e. a burner tuned to sit under
    /// every watchdog budget. Returns `true` when this call tripped the
    /// freeze. Parked-waiting time (a page polling on a slow timer) doesn't
    /// count toward the budget, so ordinary self-refreshing pages stay up.
    pub fn track_busy_window(&mut self) -> bool {
        let limit_secs = diting_env::env_knobs::js_busy_limit_secs();
        if limit_secs == 0 {
            return false;
        }
        let Some(js) = &self.js else { return false };
        let now = tokio::time::Instant::now();
        let active = js.v8_active_ns();
        let Some((t0, a0)) = self.busy_mark else {
            self.busy_mark = Some((now, active));
            return false;
        };
        let elapsed = now - t0;
        if elapsed < tokio::time::Duration::from_secs(limit_secs) {
            return false;
        }
        let gained = active.saturating_sub(a0);
        // 40%, not 80: an interval burner's in-engine duty tops out around
        // 46-60% (measured — each 40ms tick lands ~78ms apart once timer
        // dispatch latency is paid), so a peak-shaped threshold never fires
        // on the very storm it exists for. The signal is SUSTAINED duty:
        // two-fifths of a 30s window inside poll_event_loop is half a core
        // burned for half a minute, which no legitimate idle page does.
        let hot = gained as u128 >= elapsed.as_nanos() * 2 / 5;
        self.busy_mark = Some((now, active));
        // Duty % rides every freeze (and near-freeze) line: the threshold is
        // tuned against real page measurements, and this number is the only
        // way to see how close a legitimate heavy page came to the cliff.
        let duty_pct = (gained as u128 * 100 / elapsed.as_nanos().max(1)) as u64;
        tracing::warn!(
            "#66 busy window: {duty_pct}% duty over {}s",
            elapsed.as_secs()
        );
        if hot {
            if !self.busy_frozen {
                // A fresh trip starts the probe meter empty: the first
                // command accrues its window out of the frozen slices that
                // follow. A re-trip inside a probe slice must NOT reseed —
                // that would zero the very budget the probe is spending.
                self.probe_budget_ns = 0;
                self.probe_tick = None;
            }
            self.busy_frozen = true;
            let secs = elapsed.as_secs();
            tracing::error!(
                "#66: throttling realm to 25% duty after {secs}s at {duty_pct}% V8 busy \
                 (DITING_JS_BUSY_LIMIT_SECS={limit_secs}); \
                 navigate/setContent to unfreeze"
            );
            if let Some(js) = &self.js {
                js.push_console_entry(
                    "error",
                    "diting: JS throttled to 25% duty after sustained busy loop \
                     (see DITING_JS_BUSY_LIMIT_SECS); navigate or setContent to \
                     restart the page, or send another session command for a \
                     full-speed window",
                );
            }
        }
        hot
    }

    /// Whether the busy window (#66) is throttling this realm's idle pump.
    /// Throttled realms run bounded 50ms bursts (≈25% duty); a cool burst
    /// clears the flag, a session command arms a full-speed probe window,
    /// and a document swap still rebuilds the realm.
    pub fn js_busy_frozen(&self) -> bool {
        self.busy_frozen
    }

    /// The runtime's cumulative nanoseconds inside `poll_event_loop` — the
    /// single counter every pump path feeds, idle slices and command-driven
    /// eval pumping alike (11f: the command path consumes duty too, and the
    /// session actor prices it against the freeze out of this counter).
    pub fn v8_active_ns(&self) -> u64 {
        self.js.as_ref().map_or(0, |js| js.v8_active_ns())
    }

    /// Arm the #66 command probe: the session actor calls this on every
    /// command arrival. Cheap on a healthy realm (a flag check); on a
    /// frozen one it buys the next pump slice at full speed, because the
    /// async work a command kicks off (dialog mounts, module loads) runs in
    /// the idle pump, not in the command itself.
    pub fn note_command_arrived(&mut self) {
        if self.busy_frozen {
            self.busy_probe_pending = true;
        }
    }
}
