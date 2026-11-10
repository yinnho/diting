//! Bounded event-loop pumps, split from runtime.rs (#224/#225 ratchet):
//! the run_event_loop* wrappers that enforce a wall-clock budget plus
//! V8-watchdog headroom, and the promise-settle helpers built on the same
//! discipline. The raw `run_event_loop` primitive stays in runtime.rs.

impl super::JsRuntime {
    /// Drive the event loop for at most `budget_ms`, bounded against BOTH async
    /// idle (tokio timeout) and synchronous hangs (V8 watchdog). A microtask
    /// storm that pins the thread is terminated WATCHDOG_HEADROOM_MS past the
    /// budget; a well-behaved page returns as soon as the loop goes idle.
    pub async fn run_event_loop_bounded(&mut self, budget_ms: u64) -> Result<(), String> {
        self.run_event_loop_bounded_with_headroom(
            budget_ms,
            std::time::Duration::from_millis(Self::WATCHDOG_HEADROOM_MS),
            true,
        )
        .await
    }

    /// The busy-freeze (#66) variant of [`Self::run_event_loop_bounded`]:
    /// symmetric headroom (budget + budget) instead of the 5s SPA patience.
    /// The freeze only engages after a realm has already burned through the
    /// duty threshold, so the "legitimate multi-second React commit" defense
    /// of WATCHDOG_HEADROOM_MS no longer applies — and with the long headroom
    /// the throttle was nominal anyway: a 50ms burst could pin the thread for
    /// 5.05s before termination (#224's doudian livelock measured ~100% duty
    /// while "frozen"). Symmetric headroom caps a 50ms burst at 100ms of spin;
    /// a Navigate/SetContent rebuilds the realm and unfreezes as before.
    pub async fn run_event_loop_bounded_throttled(&mut self, budget_ms: u64) -> Result<(), String> {
        self.run_event_loop_bounded_with_headroom(
            budget_ms,
            std::time::Duration::from_millis(budget_ms),
            false,
        )
        .await
    }

    async fn run_event_loop_bounded_with_headroom(
        &mut self,
        budget_ms: u64,
        headroom: std::time::Duration,
        escalate: bool,
    ) -> Result<(), String> {
        if budget_ms == 0 {
            return self.run_event_loop().await;
        }
        let budget = std::time::Duration::from_millis(budget_ms);
        // #224: the plain pump arms through arm_watchdog_pump (patience
        // grows with this realm's fire history); the #66 throttled burst
        // keeps the flat wall — its whole job is containment.
        let token = if escalate {
            self.arm_watchdog_pump(budget + headroom)
        } else {
            self.arm_watchdog(budget + headroom)
        };
        let result = tokio::time::timeout(budget, self.run_event_loop()).await;
        self.disarm_watchdog(token);
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) if e.contains("execution terminated") => Ok(()),
            Ok(Err(e)) => Err(e),
            // tokio idle-timeout is the normal "settled" exit, not an error.
            Err(_) => Ok(()),
        }
    }

    /// Drive the event loop until it goes idle (no pending ops, tasks, or
    /// timers), capped at `max_ms`. Returns `true` if the loop actually went
    /// idle within the budget — `false` means work was still in flight (a
    /// long fetch, an interval timer, or a synchronous overrun terminated by
    /// the watchdog). Unlike [`Self::run_event_loop_bounded`], the caller can
    /// tell "settled" from "still busy", which is what click/transition flows
    /// need: a client-side route change is only done when the flight fetch,
    /// parse, render, and pushState have all drained.
    pub async fn run_event_loop_until_idle(&mut self, max_ms: u64) -> bool {
        let budget = std::time::Duration::from_millis(max_ms);
        // Same headroom rationale as run_event_loop_bounded: the idle pump
        // calls this with 200ms slices, and +500ms terminated legitimate
        // multi-second React commits mid-flight (#100). See
        // WATCHDOG_HEADROOM_MS. Pump-family arm (#224): patience escalates
        // with this realm's fire history.
        let token = self.arm_watchdog_pump(
            budget + std::time::Duration::from_millis(Self::WATCHDOG_HEADROOM_MS),
        );
        let result = tokio::time::timeout(budget, self.run_event_loop()).await;
        let fired = self.disarm_watchdog(token);
        matches!(result, Ok(Ok(()))) && !fired
    }

    /// Pump the event loop until `done_check` returns true (e.g. an IIFE
    /// has written its result sentinel), or `max_total_ms` elapses.
    ///
    /// Why this exists: `run_event_loop(default)` only returns when there is
    /// no pending work. Page JS routinely schedules long setTimeouts
    /// (IntersectionObserver re-fires at 7s, requestIdleCallback, etc.) that
    /// the caller does not care about. With the plain timeout we waited 5s
    /// even when the IIFE we cared about resolved in <1ms — the click flow
    /// added ~7s per click because Puppeteer's `isIntersectingViewport`
    /// disconnects its observer in the callback, but our scheduled
    /// re-fires keep the event loop "busy" until they all fire.
    pub async fn resolve_promises_until<F>(&mut self, mut done_check: F, max_total_ms: u64) -> bool
    where
        F: FnMut(&mut Self) -> bool,
    {
        let start = tokio::time::Instant::now();
        let deadline = start + tokio::time::Duration::from_millis(max_total_ms);
        let mut tick_ms: u64 = 1;
        // Whole-wait backstop: every tick below is bounded (the patience
        // watchdog until escalation, 2x tick after), so this only fires when
        // that containment itself fails — e.g. a wedged disarm (#332 face).
        // On fire, exit early (disarm cancels the termination so the isolate
        // stays usable).
        let wd = self.arm_watchdog(std::time::Duration::from_millis(max_total_ms + 500));
        // #63: with the raw loop, a synchronous page wall inside a tick runs
        // to COMPLETION — the only bound was the whole-budget watchdog above
        // — so a 1.2s wall starves both the awaited promise's timer delivery
        // (its wake needs a microtask checkpoint the wall is sitting on) and
        // the done_check that would notice the settle (measured on the fetch
        // + behead-storm repro: a nominal 100ms promise read js+1259ms /
        // rust+2478ms = one full wall per half; the wheel itself was healthy
        // throughout — patrols and arms all sane). Containment, in two
        // phases: while patience remains the ticks stay BYTE-IDENTICAL to
        // the raw shape below — the only new intervention is this one
        // patience watchdog, armed at wait start, which beheads the first
        // wall at EVAL_WALL_PATIENCE_MS (legitimate mid-commit page work
        // gets the same patience defense as WATCHDOG_HEADROOM_MS;
        // Rust-side walls are terminate-immune regardless). Do NOT route
        // these ticks through run_event_loop_bounded_with_headroom: routing
        // them measurably disturbed a legitimate engine-side batch even
        // with no watchdog able to fire (the #224 animation-sweep test
        // lost 130 of 600 starts to a 10ms tick floor + per-tick arm
        // ceremony). Once the patience watchdog fires, every remaining tick
        // escalates to the #66 throttled pump, capping each wall at 2x the
        // tick, the same containment the driver's outer slices already
        // apply everywhere else.
        // Patience never outlives the wait itself (a short-budget eval
        // breaks on its deadline and disarms this below).
        let mut patience_wd = Some(self.arm_watchdog(std::time::Duration::from_millis(
            Self::EVAL_WALL_PATIENCE_MS.min(max_total_ms),
        )));
        let mut escalated = false;
        // False on deadline/watchdog: the caller must not read a result slot
        // the script never assigned (that's how a timeout used to become a
        // silent `null`).
        let mut settled = false;
        loop {
            // The patience watchdog beheaded a wall mid-tick (the tick above
            // just returned from its terminate). Disarm FIRST — the fired
            // disarm clears the stale termination flag and revives the
            // isolate — so done_check's probe runs clean, and fall through
            // to containment for the rest of this wait.
            if patience_wd.as_ref().is_some_and(|w| w.fired()) {
                escalated = true;
                if let Some(w) = patience_wd.take() {
                    self.disarm_watchdog(w);
                }
            }
            if done_check(self) {
                settled = true;
                break;
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }
            // Pump for a short slice. If the loop returns idle in <tick_ms,
            // run_event_loop returns Ok and we check the predicate again.
            // #50: routed through Self::run_event_loop so each poll first
            // clears a terminate flag the watchdog left on a parked isolate.
            if escalated {
                let _ = self.run_event_loop_bounded_throttled(50).await;
            } else {
                // #69: patience is per WALL, not per wait. Armed once, it
                // fired 500ms into the wait on whatever was running — a
                // caller's own chain of short awaits (four RSA keygens)
                // crossing the mark got beheaded and the eval sat out its
                // full 5s budget. And escalating once patience merely
                // ELAPSED fenced every later tick at 2x50ms with no wall ever
                // seen, so a saturated runner's ordinary 150ms chunk died
                // too (ECDSA/OAEP batteries). Pushed at every tick, the
                // watchdog stays on duty for the whole wait and only fires on
                // one tick running past patience — a real wall — and only
                // that fire escalates.
                if let Some(w) = patience_wd.as_ref() {
                    w.push_deadline(
                        tokio::time::Duration::from_millis(Self::EVAL_WALL_PATIENCE_MS)
                            .min(deadline.saturating_duration_since(now)),
                    );
                }
                let _ = tokio::time::timeout(
                    tokio::time::Duration::from_millis(tick_ms),
                    self.run_event_loop(),
                )
                .await;
            }
            if wd.fired() {
                break;
            }
            // Backoff so a hung promise doesn't burn CPU. Caps at 50ms;
            // worst case we miss the result by <50ms.
            if tick_ms < 50 {
                tick_ms = (tick_ms * 2).min(50);
            }
        }
        if let Some(w) = patience_wd.take() {
            let _ = self.disarm_watchdog(w);
        }
        if self.disarm_watchdog(wd) {
            tracing::warn!("promise wait terminated by watchdog (sync spin in event loop)");
        }
        settled
    }
}
