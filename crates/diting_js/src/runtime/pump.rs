//! Bounded event-loop pumps, split from runtime.rs (#224/#225 ratchet):
//! the run_event_loop* wrappers that enforce a wall-clock budget at task
//! boundaries plus a per-task V8-watchdog spin fence, and the promise-settle
//! helpers built on the same discipline. The raw `run_event_loop` primitive
//! stays in runtime.rs.

impl super::JsRuntime {
    /// Drive the event loop for at most `budget_ms`, bounded against BOTH async
    /// idle (tokio timeout) and synchronous hangs (V8 watchdog). A microtask
    /// storm that pins the thread is terminated only at the pump family's
    /// minute-scale spin fence; a well-behaved page returns as soon as the loop
    /// goes idle.
    pub async fn run_event_loop_bounded(&mut self, budget_ms: u64) -> Result<(), String> {
        self.run_event_loop_bounded_with_task_watchdog(
            budget_ms,
            std::time::Duration::from_millis(Self::PUMP_TASK_WATCHDOG_MS),
        )
        .await
    }

    /// The busy-freeze (#66) variant of [`Self::run_event_loop_bounded`]: a
    /// ~`budget_ms` burst that throttles a hot realm WITHOUT cutting its
    /// tasks. It used to cap the burst with a symmetric-headroom watchdog
    /// (budget + budget): the macrotask drain ran every queued task in one
    /// JS turn, so a 50ms burst could otherwise pin the thread for the whole
    /// 5s patience (#224's doudian livelock measured ~100% duty while
    /// "frozen") — but that watchdog beheaded whatever task was running every
    /// burst, a thousand times a session on x.com, stranding page state half
    /// set (the #100 poisoning) and severing fetch continuations. Now the
    /// drain yields a loop pass after every task for the burst's length (the
    /// task-boundary hook the eval wait uses), so the tokio budget ends the
    /// burst at the next boundary, and only a single task running past the
    /// pump family's minute-scale spin fence is cut. A Navigate/SetContent
    /// rebuilds the realm and unfreezes as before.
    pub async fn run_event_loop_bounded_throttled(&mut self, budget_ms: u64) -> Result<(), String> {
        self.run_event_loop_bounded_with_task_watchdog(
            budget_ms,
            std::time::Duration::from_millis(Self::PUMP_TASK_WATCHDOG_MS),
        )
        .await
    }

    pub(super) async fn run_event_loop_bounded_with_task_watchdog(
        &mut self,
        budget_ms: u64,
        task_budget: std::time::Duration,
    ) -> Result<(), String> {
        if budget_ms == 0 {
            return self.run_event_loop().await;
        }
        let budget = std::time::Duration::from_millis(budget_ms);
        // Plain and throttled pumps share the #224/#100 pump watchdog: the
        // deadline is pushed at every event-loop pass, so it bounds ONE
        // synchronous task rather than the whole initialization window. The
        // host-wait flag makes the macrotask drain yield after each task,
        // which is what makes those passes true task boundaries here.
        self.set_host_wait(true);
        let token = self.arm_watchdog_pump(task_budget);
        let result = {
            let mut heartbeat = |_rt: &mut Self| {
                token.push_deadline(task_budget);
                false
            };
            tokio::time::timeout(budget, self.run_event_loop_checked(Some(&mut heartbeat))).await
        };
        self.set_host_wait(false);
        self.disarm_watchdog(token);
        match result {
            Ok(Ok(_)) => Ok(()),
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
        // Same task-boundary discipline as run_event_loop_bounded: the idle
        // pump calls this with short slices, and short fences terminated
        // legitimate multi-second React commits mid-flight (#100). The watchdog
        // is re-armed at every pass instead of measuring the whole window.
        let task_budget = std::time::Duration::from_millis(Self::PUMP_TASK_WATCHDOG_MS);
        self.set_host_wait(true);
        let token = self.arm_watchdog_pump(task_budget);
        let result = {
            let mut heartbeat = |_rt: &mut Self| {
                token.push_deadline(task_budget);
                false
            };
            tokio::time::timeout(budget, self.run_event_loop_checked(Some(&mut heartbeat))).await
        };
        self.set_host_wait(false);
        let fired = self.disarm_watchdog(token);
        matches!(result, Ok(Ok(_))) && !fired
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

    /// Ask the macrotask drain to (or no longer) yield a loop pass between
    /// tasks (timer_belt.js): while a host waits on a result, and while a
    /// throttled burst needs to end at a task boundary.
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
