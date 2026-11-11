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
        // A synchronous page wall is never cut short for the wait's sake: a
        // terminate mid-task strands whatever the page set without a
        // `finally` (react-dom's executionContext bits, #100), poisoning the
        // realm for good — Chrome runs the task to completion and so do we.
        // (The #63 eval patience watchdog did behead such walls at 500ms;
        // retired.) The only cut is this whole-wait backstop, for a true
        // spin; on fire, exit early (disarm cancels the termination so the
        // isolate stays usable).
        let wd = self.arm_watchdog(std::time::Duration::from_millis(max_total_ms + 500));
        // What the patience bought was latency, and the latency has a
        // different cause: the macrotask drain runs every queued task in one
        // JS turn, so a result the awaited task settled was only seen after
        // the page's NEXT wall too (a 100ms promise behind 1.2s walls read
        // 2.4s). While a host waits, the drain yields a real loop pass after
        // each task (Chrome's task boundary), and every pass below checks
        // the result — one wall, not two.
        self.set_host_wait(true);
        // False on deadline/watchdog: the caller must not read a result slot
        // the script never assigned (that's how a timeout used to become a
        // silent `null`).
        let mut settled = false;
        loop {
            if done_check(self) {
                settled = true;
                break;
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }
            // Pump for a short slice, checking the predicate after every
            // loop pass. If the loop returns idle in <tick_ms we check it
            // again here. #50: the checked loop shares run_event_loop's entry
            // heal, clearing a terminate flag left on a parked isolate.
            let ran = tokio::time::timeout(
                tokio::time::Duration::from_millis(tick_ms),
                self.run_event_loop_checked(Some(&mut done_check)),
            )
            .await;
            if matches!(ran, Ok(Ok(true))) {
                settled = true;
                break;
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
        if self.disarm_watchdog(wd) {
            tracing::warn!("promise wait terminated by watchdog (sync spin in event loop)");
        }
        self.set_host_wait(false);
        settled
    }

    /// Tell the macrotask drain a host is (or no longer is) waiting on a
    /// result, so it yields a loop pass between tasks (timer_belt.js).
    fn set_host_wait(&mut self, on: bool) {
        let _ = self.runtime.execute_script(
            "<host-wait>",
            if on {
                "globalThis.__diting_mt_host_wait && globalThis.__diting_mt_host_wait(true)"
            } else {
                "globalThis.__diting_mt_host_wait && globalThis.__diting_mt_host_wait(false)"
            },
        );
    }
}
