//! V8 execution watchdogs: the armed-token family behind
//! [`JsRuntime::arm_watchdog`](super::JsRuntime::arm_watchdog) — the cancel
//! channel + join handle, the debug stack-dump sampler
//! (DITING_WATCHDOG_STACK_DUMP=1), the arm/disarm accessors on
//! JsRuntime, and `spawn_watchdog` for callers holding a bare isolate
//! handle. Split from the module root (god-file ratchet); #66's
//! sustained-duty busy accounting lives here too — same family, the
//! defenses against a realm burning a core.
use v8::{self, IsolateHandle};

/// Handle to an armed V8 execution watchdog (see [`JsRuntime::arm_watchdog`]).
/// Holds the cancel channel and the watchdog thread; pass it back to
/// `disarm_watchdog` to stop the watchdog and learn whether it fired.
pub struct WatchdogToken {
    pair: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    join: Option<std::thread::JoinHandle<()>>,
    fired: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set at the top of cancel_and_join, BEFORE the mutex: the post-fire
    /// grace poll in the watchdog thread reads this, not the mutex flag,
    /// because the mutex stays in the watchdog thread's hands through the
    /// grace — polling it would deadlock stop()'s lock for the whole grace.
    /// "Owner came back" is exactly what the grace wants to detect.
    resurfaced: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Debug knob (DITING_WATCHDOG_STACK_DUMP=1): a sampler thread that
    /// requests a stack-dump interrupt every 250ms while this watchdog is
    /// armed, so the FIRST sample lands mid-spin — long before any terminate
    /// (and its subsequent recovery scripts) destroys the stack we want.
    /// `None` when the knob is off.
    sampler_stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// #332: the realm's wedge flag, shared from the owning JsRuntime at arm
    /// time (like `stale_termination` flows into the thread closure). Set by
    /// [`WatchdogToken::cancel_and_join`] when the cancel ceremony blew the
    /// [`WATCHDOG_CANCEL_GRACE`] wall; the runtime's Drop reads it to skip
    /// the isolate teardown (the wedged thread holds the same rusty_v8
    /// handle mutex `IsolateHandle::dispose()` needs). Living in the token —
    /// not written back through a runtime pointer — is what lets the
    /// ORPHANED-arm Drop path (tokio preemption between arm and disarm)
    /// report the wedge too.
    wedge_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// The isolate's own thread (every arm site runs on it). The post-fire
    /// grace registers it in [`ZOMBIE_THREADS`]; a late resurface clears it.
    owner: std::thread::ThreadId,
}

/// Threads whose isolate swallowed a terminate (rusty_v8 #2088 shape): the
/// post-fire grace ran out without the owner coming back. Nothing can unwind
/// such a thread, but whoever waits on it can stop waiting — the session
/// layer checks this while awaiting a command reply, so one poisoned page
/// fails its request instead of hanging it (and the manager lock) forever.
static ZOMBIE_THREADS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<std::thread::ThreadId>>,
> = std::sync::LazyLock::new(Default::default);

/// Whether `thread`'s isolate is currently declared a zombie (see
/// [`ZOMBIE_THREADS`]). Cheap: one uncontended lock + a set probe.
pub fn is_zombie_thread(thread: std::thread::ThreadId) -> bool {
    zombies().contains(&thread)
}

/// Poison-recovering guard on [`ZOMBIE_THREADS`] (#336): a poisoned set
/// must not read as "nobody is a zombie" forever — the set's insert/remove
/// critical sections leave it intact.
fn zombies() -> std::sync::MutexGuard<'static, std::collections::HashSet<std::thread::ThreadId>> {
    ZOMBIE_THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Test seam for the product's session layer (its reply wait keys on the
/// verdict): declare or lift `thread` without driving a real spin.
#[doc(hidden)]
pub fn set_zombie_thread_for_test(thread: std::thread::ThreadId, zombie: bool) {
    let mut z = zombies();
    if zombie {
        z.insert(thread);
    } else {
        z.remove(&thread);
    }
}

/// Fire → zombie verdict. A terminate cannot land inside a Rust op either:
/// page JS that forces a seconds-long layout (#109: a 13s gBCR) leaves the
/// owner unreachable but ALIVE, and a verdict there would fail a request
/// that was about to succeed and drop a working session. 20s clears every
/// legitimate op seen in the wild; only a true hole waits that long. The
/// 3s mark below stays as an early warning in the log.
const ZOMBIE_VERDICT_AFTER: std::time::Duration = if cfg!(test) {
    std::time::Duration::from_secs(3)
} else {
    std::time::Duration::from_secs(20)
};

/// #332: the wall for the whole cancel ceremony in
/// [`WatchdogToken::cancel_and_join`] — acquiring the pair mutex AND
/// joining the thread. The mutex belongs to the watchdog thread from its
/// first lock until it returns (the cvar park releases it only while
/// parked; post-fire it stays in its hands across the grace and
/// keep-knocking loops, both of which poll `resurfaced` every 100ms, so a
/// healthy watchdog completes the ceremony within ~150ms of a cancel).
/// The hang this bounds: a watchdog wedged INSIDE
/// `terminate_execution()` — rusty_v8's `with_isolate_ptr` holds the
/// handle's `isolate_mutex` across the FFI call — never releases the pair
/// mutex and never exits, and the owner's blocking `lock()`/`join()` then
/// pinned every session of a loaded server forever. Past the grace the
/// token leaks the thread on purpose and marks the realm wedged (see
/// `wedge_flag`) so its isolate teardown is skipped too: the same handle
/// mutex blocks `IsolateHandle::dispose()`. One thread + one heap leak
/// per incident, loudly logged, instead of a hang.
const WATCHDOG_CANCEL_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Debug knob (DITING_WATCHDOG_STACK_DUMP=1): dump the live JS stack of
/// a synchronous overrun. Two producers share this callback:
///  - `watchdog_terminate` requests one at fire time (best effort: if the spin
///    is inside a Rust op, no interrupt can land until the op returns, and a
///    terminate from another watchdog may have already unwound the stack);
///  - the per-watchdog sampler requests one every 250ms while armed, so the
///    first samples land MID-SPIN, which is the trustworthy view.
///
/// `data` points at a boxed `u64` request timestamp (ms since epoch) owned by
/// the requester; null means fire-time (the box would be unowned anyway).
/// Off by default; strictly an engine-debugging aid.
fn stack_dump_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

unsafe extern "C" fn dump_spin_stack(
    isolate: v8::UnsafeRawIsolatePtr,
    data: *mut std::ffi::c_void,
) {
    let requested_ms = if data.is_null() {
        None
    } else {
        let ms = unsafe { *std::ptr::NonNull::new(data).unwrap().cast::<u64>().as_ptr() };
        drop(unsafe { Box::from_raw(data as *mut u64) });
        Some(ms)
    };
    let landed_ms = stack_dump_now_ms();
    let mut isolate = unsafe { v8::Isolate::from_raw_isolate_ptr(isolate) };
    let terminating = isolate.is_execution_terminating();
    let tag = match requested_ms {
        Some(req) => {
            let lag = landed_ms.saturating_sub(req);
            format!("sample lag={lag}ms")
        }
        None => "fire".to_string(),
    };
    eprintln!("[watchdog-stack] {landed_ms} {tag} interrupt landed; terminating={terminating}");
    v8::scope!(let hs, &mut isolate);
    // SAFETY: `HandleScope<'i, C>`'s C parameter exists only as PhantomData,
    // so HandleScope<'i, ()> and HandleScope<'i, Context> share a layout, and
    // re-borrowing the live HandleScope as the Context-flavored PinScope the
    // StackTrace API wants is sound. An earlier version entered a synthetic
    // Context here (Context::new + ContextScope) and capture saw 0 frames;
    // entering no context at all rules that interplay out.
    let scope: &v8::PinScope<'_, '_> = unsafe { std::mem::transmute(&*hs) };
    let Some(trace) = v8::StackTrace::current_stack_trace(scope, 24) else {
        eprintln!("[watchdog-stack] {landed_ms} no stack available");
        return;
    };
    let count = trace.get_frame_count().min(24);
    eprintln!("[watchdog-stack] {landed_ms} {count} frames (top first):");
    for i in 0..count {
        let Some(frame) = trace.get_frame(scope, i) else {
            continue;
        };
        let fn_name = frame
            .get_function_name(scope)
            .map(|s| s.to_rust_string_lossy(scope))
            .unwrap_or_default();
        let script = frame
            .get_script_name(scope)
            .map(|s| s.to_rust_string_lossy(scope))
            .unwrap_or_default();
        let fn_name = if fn_name.is_empty() {
            "<anon>"
        } else {
            &fn_name
        };
        // Minified bundles are one line, so name+line:col can't tell 54 inline
        // scripts apart — the V8 script id (compile order) plus a source
        // prefix can. Prefix only on the top frame to keep samples cheap.
        let source_tag = if i == 0 {
            let prefix: String = frame
                .get_script_source(scope)
                .map(|s| {
                    s.to_rust_string_lossy(scope)
                        .chars()
                        .take(80)
                        .map(|c| if c == '\n' { ' ' } else { c })
                        .collect()
                })
                .unwrap_or_default();
            format!(" id={} src≈[{}]", frame.get_script_id(), prefix)
        } else {
            format!(" id={}", frame.get_script_id())
        };
        eprintln!(
            "[watchdog-stack]   #{i} {fn_name} @ {script}:{}:{}{source_tag}",
            frame.get_line_number(),
            frame.get_column()
        );
    }
}

/// Shared tail of every watchdog fire path. Honors the debug knob above
/// (request a stack dump, give the interrupt a moment to land mid-spin)
/// before terminating. The dump is best-effort: if the spin is inside a Rust
/// op the interrupt only lands after the op returns, and a normally-finishing
/// script just dumps wherever the next V8 entry is — harmless either way.
pub(super) fn watchdog_terminate(handle: &IsolateHandle) {
    if std::env::var("DITING_WATCHDOG_STACK_DUMP").is_ok() {
        // Fire-time dumps are the fallback view; the armed sampler (see
        // spawn_watchdog) is the trustworthy one. 150ms per attempt is plenty
        // for a V8-visible spin (interrupts land at the next stack check);
        // an op-bound spin can't be reached at all.
        handle.request_interrupt(dump_spin_stack, std::ptr::null_mut());
        std::thread::sleep(std::time::Duration::from_millis(150));
        handle.request_interrupt(dump_spin_stack, std::ptr::null_mut());
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    handle.terminate_execution();
}

/// Arm a V8 termination watchdog directly from an isolate handle, with no
/// runtime borrow. The CDP dispatcher uses this to bound every command so a
/// hung page cannot hold the process-wide V8 lock forever. Pair with
/// [`WatchdogToken::stop`]; if `stop` returns true, clear the termination flag
/// via [`JsRuntime::cancel_termination`] before reusing the isolate.
///
/// `stale` (the runtime's `stale_termination` flag) is set at the instant of
/// firing — BEFORE `terminate_execution()` — so `run_event_loop`'s poll-boundary
/// heal can tell a pending termination from V8 internals it cannot observe.
pub fn spawn_watchdog(
    handle: IsolateHandle,
    budget: std::time::Duration,
    stale: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    wedge_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> WatchdogToken {
    let pair = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Debug sampler (DITING_WATCHDOG_STACK_DUMP=1): while this watchdog
    // is armed, poke a stack-dump interrupt every 250ms. For a spin that V8
    // can see (JS loop, or a loop of op calls), samples land mid-spin —
    // before terminate_execution and the recovery scripts destroy the
    // evidence. A lag >>250ms on every sample means the thread was parked
    // inside a single Rust op — the one fact fire-time dumps can't show.
    let sampler_stop = if std::env::var("DITING_WATCHDOG_STACK_DUMP").is_ok() {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sampler_handle = handle.clone();
        let sampler_stop = stop.clone();
        std::thread::spawn(move || {
            while !sampler_stop.load(std::sync::atomic::Ordering::Relaxed) {
                let req = Box::into_raw(Box::new(stack_dump_now_ms()));
                sampler_handle.request_interrupt(dump_spin_stack, req.cast());
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
        Some(stop)
    } else {
        None
    };
    let pair_c = pair.clone();
    let fired_c = fired.clone();
    let resurfaced = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let resurfaced_c = resurfaced.clone();
    let owner = std::thread::current().id();
    let join = std::thread::spawn(move || {
        let (lock, cvar) = &*pair_c;
        let mut cancelled = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let deadline = std::time::Instant::now() + budget;
        loop {
            // Check first: stop() may have set this (and notified into the void)
            // before this thread even started, which happens constantly for fast
            // CDP commands where stop() is called right after spawn. Without this
            // top check the lost notify means we wait the full budget before
            // noticing, and stop()'s join() blocks for that whole time.
            if *cancelled {
                return;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                fired_c.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Some(stale) = stale.as_ref() {
                    stale.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                watchdog_terminate(&handle);
                // Post-fire grace: did the shot land? A terminate only takes
                // effect at a V8 stack check; a TurboFan-optimized loop that
                // calls a builtin (e.g. Array.prototype.fill) has none —
                // rusty_v8 #2088, re-hammering every 10ms for 10s still never
                // lands. The resurface signal is the owner calling stop()/
                // dropping the token (resurfaced is set before the mutex);
                // if that never comes, the isolate is spinning forever and
                // nothing at our layer can reach it. Say so loudly instead
                // of dying silent — 100ms polls so a normal recovery exits
                // the grace (and the join) promptly.
                //
                // Re-hammer from 500ms on: a single shot is not enough when
                // the owner's poll-boundary heal (#50) cancels the landed
                // terminate and the event loop moves on to the NEXT macrotask
                // still inside the same pump — that task then runs with no
                // wall at all. github.com/settings (2026-11-06): each link
                // `load` reached a window listener that re-registers itself
                // (web-vitals whenReady), listeners doubled per link, and the
                // pump spun ~100s past one fire. The budget is spent the
                // moment we fire; JS the owner runs before stop() is overrun
                // by contract (the 500ms lets its short cleanup scripts pass).
                let fired_at = std::time::Instant::now();
                let mut next_hammer = fired_at + std::time::Duration::from_millis(500);
                let mut warned = false;
                loop {
                    if resurfaced_c.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let now = std::time::Instant::now();
                    if now >= next_hammer {
                        if let Some(stale) = stale.as_ref() {
                            stale.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                        handle.terminate_execution();
                        next_hammer = now + std::time::Duration::from_millis(250);
                    }
                    let since = now - fired_at;
                    if !warned && since >= std::time::Duration::from_secs(3) {
                        warned = true;
                        eprintln!(
                            "[watchdog] terminate fired (and re-issued) but the isolate has not \
                             resurfaced within 3s — a long Rust op, or the optimized-loop \
                             termination hole (rusty_v8 #2088 shape); zombie verdict at {}s",
                            ZOMBIE_VERDICT_AFTER.as_secs()
                        );
                    }
                    if since >= ZOMBIE_VERDICT_AFTER {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                eprintln!(
                    "[watchdog] isolate never resurfaced within {}s of the fire — marking the \
                     thread a zombie so its waiters fail instead of hanging (the verdict lifts \
                     if it ever resurfaces)",
                    ZOMBIE_VERDICT_AFTER.as_secs()
                );
                // An owner resurfacing right after this is fine: its
                // cancel_and_join clears the mark only after joining us.
                zombies().insert(owner);
                // Keep knocking (1s cadence) until the owner is back: free
                // for a true #2088 spin, and a heal-and-continue spin that
                // outlived the grace still gets cut at its next JS entry.
                let mut ticks = 0u32;
                while !resurfaced_c.load(std::sync::atomic::Ordering::Relaxed) {
                    ticks += 1;
                    if ticks.is_multiple_of(10) {
                        if let Some(stale) = stale.as_ref() {
                            stale.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                        handle.terminate_execution();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                return;
            }
            let (guard, _) = cvar
                .wait_timeout(cancelled, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cancelled = guard;
            if *cancelled {
                return;
            }
        }
    });
    WatchdogToken {
        pair,
        join: Some(join),
        fired,
        resurfaced,
        sampler_stop,
        wedge_flag,
        owner,
    }
}

/// The bounded half of the #332 cancel ceremony, factored out so the wedge
/// test can drive it without a live isolate: signal the pair (try_lock — the
/// mutex may be held for the wedge's lifetime by a watchdog stuck inside
/// `terminate_execution`, and a blocking `lock()` is exactly the loaded-server
/// hang), then join (poll [`std::thread::JoinHandle::is_finished`] — a
/// blocking join pins the owner on the same wedge). Returns
/// `(signalled, joined)`; `false` in either slot is the wedge verdict.
fn bounded_cancel_and_join(
    pair: &(std::sync::Mutex<bool>, std::sync::Condvar),
    mut join: Option<std::thread::JoinHandle<()>>,
    deadline: std::time::Instant,
) -> (bool, bool) {
    let mut signalled = false;
    {
        let (lock, cvar) = pair;
        loop {
            // A poisoned pair (the watchdog thread panicked) is still ours
            // to signal; skipping it would read as a wedge and leak the
            // isolate (#336).
            let acquired = match lock.try_lock() {
                Ok(guard) => Some(guard),
                Err(std::sync::TryLockError::Poisoned(e)) => Some(e.into_inner()),
                Err(std::sync::TryLockError::WouldBlock) => None,
            };
            if let Some(mut cancelled) = acquired {
                *cancelled = true;
                drop(cancelled);
                cvar.notify_one();
                signalled = true;
                break;
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let joined = match join.take() {
        None => true,
        Some(j) => {
            while !j.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if j.is_finished() {
                let _ = j.join();
                true
            } else {
                // Leak on purpose: the thread is stuck inside a V8 call or in
                // its own exit. Dropping a JoinHandle detaches the thread
                // (it never joins), so this is the leak. If the call ever
                // returns, the loops in the thread see the resurface flag
                // and it exits on its own — merely unjoined.
                drop(j);
                false
            }
        }
    };
    (signalled, joined)
}

impl Drop for WatchdogToken {
    /// Cancellation safety for armed watchdogs. Async callers (the session
    /// idle pump, settle futures) can be dropped between `arm_watchdog` and
    /// `disarm_watchdog` — `tokio::select!` preempts the pump the moment a
    /// command arrives. Without this the orphaned thread later fires
    /// `terminate_execution()` into whatever runs next and bricks the
    /// isolate (no `cancel_terminate_execution` ever follows; measured as
    /// every subsequent eval failing with "Uncaught Error: execution
    /// terminated"). Dropping the token cancels the thread instead: it
    /// sleeps in `wait_timeout` while holding the mutex, so once we acquire
    /// the lock the thread can only wake, see the cancel flag and exit — it
    /// cannot have terminated in between. The remaining sliver (it fired
    /// while we were blocked on the lock) is healed by the
    /// retry-on-terminated path in `evaluate`.
    fn drop(&mut self) {
        self.cancel_and_join();
    }
}

impl WatchdogToken {
    /// Stop the watchdog. Returns true if it had already fired (terminated the
    /// isolate). The caller must then clear the termination flag via
    /// [`JsRuntime::cancel_termination`] before the next eval.
    pub fn stop(mut self) -> bool {
        self.cancel_and_join();
        self.fired.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Shared cancel path for Drop and stop(): flip the resurface flag, run
    /// the #332-bounded ceremony, then halt the stack-dump sampler. Joining
    /// before the sampler stops guarantees the sampler halts only AFTER the
    /// watchdog can no longer fire, so its last in-flight request can at
    /// worst dump a healthy stack once.
    fn cancel_and_join(&mut self) {
        // Before the mutex: the post-fire grace poll in the watchdog thread
        // reads this flag, and the mutex is in that thread's hands until the
        // grace ends.
        self.resurfaced
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let (signalled, joined) = bounded_cancel_and_join(
            &self.pair,
            self.join.take(),
            std::time::Instant::now() + WATCHDOG_CANCEL_GRACE,
        );
        if !signalled || !joined {
            eprintln!(
                "[watchdog] #332 wedge: the watchdog did not finish its cancel \
                 ceremony within {}ms (signalled={signalled}, joined={joined} — \
                 stuck inside terminate_execution or in thread exit) — leaking the \
                 thread; this realm's isolate teardown will be skipped (the same \
                 rusty_v8 handle mutex is wedged)",
                WATCHDOG_CANCEL_GRACE.as_millis()
            );
            if let Some(w) = &self.wedge_flag {
                w.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            // A zombie-verdict entry, if one landed, stays: without a join
            // there is no proof the thread is gone, and waiters must keep
            // failing fast on this realm instead of hanging behind it.
        } else {
            // Late resurface after a zombie verdict (the spin did end on its
            // own): the thread is live again. Its waiters already gave up, so
            // this only matters for the next command — which must not be
            // refused against a thread that is serving.
            if zombies().remove(&self.owner) {
                eprintln!("[watchdog] zombie verdict lifted: the isolate resurfaced late");
            }
        }
        if let Some(stop) = self.sampler_stop.take() {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Whether this watchdog has already terminated the isolate, without
    /// stopping it. Lets a pump loop poll for termination between slices
    /// while keeping the token alive for the final `stop()`.
    pub fn fired(&self) -> bool {
        self.fired.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl super::JsRuntime {
    /// Headroom added on top of the caller's budget before the V8 watchdog
    /// may terminate execution. Termination is a scalpel for infinite spins —
    /// but react-dom 16's `performWorkOnRoot` sets its `executionContext`
    /// bits with no `finally` to clear them, so a terminate that lands
    /// mid-render or mid-commit poisons the realm permanently: every later
    /// scheduler tick throws minified #327 ("Should not already be working")
    /// and every `setState` dies silently — the Tmall publish page (#100)
    /// lost all input write-back and button clicks to exactly this. A heavy
    /// SPA commit legitimately runs for seconds (style + layout per commit
    /// is engine-side cost Chrome amortizes), so the old +500ms razor killed
    /// real work on the idle pump's 200ms slices. Five seconds of patience
    /// still catches any true spin (a spin never finishes regardless of
    /// budget), and the #66 duty-cycle freeze remains the backstop for
    /// under-budget burners.
    pub const WATCHDOG_HEADROOM_MS: u64 = 5_000;

    /// Watchdog duration for the meta_code half of an eval (#110): 10s floor,
    /// else the caller's own await budget — the settle half of the same eval
    /// already waits that long, so a legitimate long synchronous script must
    /// not be killed earlier than the budget its caller explicitly set.
    pub(crate) fn eval_watchdog_duration(await_budget_ms: u64) -> std::time::Duration {
        std::time::Duration::from_millis(await_budget_ms).max(std::time::Duration::from_secs(10))
    }

    /// Arm a hard wall-clock backstop on synchronous V8 work. A page stuck in a
    /// synchronous loop or a microtask storm pins the OS thread inside V8, so
    /// `tokio::time::timeout` (which can only cancel at await points) never
    /// fires. This spawns a watchdog thread that terminates the isolate once
    /// `budget` elapses, forcing V8 to throw an uncatchable error and hand
    /// control back. Always balance with [`Self::disarm_watchdog`].
    pub fn arm_watchdog(&mut self, budget: std::time::Duration) -> WatchdogToken {
        spawn_watchdog(
            self.runtime.isolate_handle(),
            budget,
            Some(self.stale_termination.clone()),
            Some(self.watchdog_wedge.clone()),
        )
    }

    /// #224: the pump-family arm (`settle_pending_work`, netidle,
    /// `run_event_loop_bounded`, `run_event_loop_until_idle`). Same wall as
    /// [`Self::arm_watchdog`], but the budget grows with this realm's fire
    /// history: every termination the watchdog already dealt out doubles
    /// the next pump budget, up to ×128. Doudian's publish boot is the
    /// counterexample that motivated it: minute-scale synchronous
    /// schema/form JS (legal commerce work, just 4-10× slower than Chrome
    /// here) ran into settle 3s+5s / until_idle 200ms+5s walls FIFTEEN
    /// times in one boot — each fire severed whatever promise chain was
    /// mid-commit, and the prefill chain never completed (主图 never
    /// mounted; op deliveries were lost outright). A realm that has
    /// already been beheaded N times is either slow-and-legitimate (the
    /// patience saves it) or a spin (the #66 duty freeze catches it —
    /// a spin burns duty regardless of watchdog budget). NOT applied to
    /// the eval/module/script watchdogs: those are API contracts with
    /// caller-visible timeouts, nor to the #66 throttled burst (that
    /// symmetric fence exists to CONTAIN a burner).
    pub fn arm_watchdog_pump(&mut self, budget: std::time::Duration) -> WatchdogToken {
        let mult = self.pump_patience();
        // Cap the escalated wall at base + 60s. The patience exists for
        // legitimate slow commits (42s of sustained sync measured on this
        // page family) — not to stretch a termination-immune wedge. The
        // first #224 repro pinned the session actor for minutes inside one
        // optimized microtask (a memmove-shaped loop that ignored a pending
        // TerminateExecution; V8 optimized code without interrupt checks
        // doesn't observe termination at all), and at ×128 the settle wall
        // alone was 256s of that. +60s keeps every escalated wall in the
        // "survivable, commands drain within a minute" band.
        let cap = budget + std::time::Duration::from_secs(60);
        let scaled = budget.saturating_mul(mult).min(cap);
        if scaled > budget {
            tracing::debug!(
                "#224 pump watchdog patience ×{mult} (capped at {}ms)",
                scaled.as_millis()
            );
        }
        self.arm_watchdog(scaled)
    }

    /// The current #224 escalation factor: 1 << min(fires, 7), capped ×128.
    fn pump_patience(&self) -> u32 {
        1u32 << self.watchdog_fired_total.get().min(7)
    }

    /// Stop a watchdog armed by [`Self::arm_watchdog`]. If it had already fired
    /// (terminated the isolate), clear V8's termination flag so the isolate is
    /// usable again, and return `true`.
    pub fn disarm_watchdog(&mut self, token: WatchdogToken) -> bool {
        let fired = token.stop();
        if fired {
            // The heal in run_event_loop owns stale-clearing at poll
            // boundaries; this swap only covers the flag the watchdog set
            // between the last poll and this disarm.
            self.stale_termination
                .store(false, std::sync::atomic::Ordering::SeqCst);
            // #227 belt for the race that swap loses: a fire landing
            // inside the final poll of a pump that then exits on its
            // elapsed timeout is never re-polled, so no poll boundary
            // observes the stale flag. Forward the unwind instead — the
            // next loop entry (≤ one pump slice away) resets any trampoline
            // bracket the fire leaked.
            self.termination_unwound
                .store(true, std::sync::atomic::Ordering::SeqCst);
            self.runtime.revive();
            self.watchdog_fired_total
                .set(self.watchdog_fired_total.get() + 1);
            tracing::warn!(
                "V8 watchdog fired: terminated a synchronous overrun ({} fires this realm; pump budgets now ×{})",
                self.watchdog_fired_total.get(),
                self.pump_patience()
            );
        }
        fired
    }

    /// `execute_script` that self-heals from a stray V8 termination. A
    /// watchdog that fired without a paired disarm (the pre-Drop-token
    /// cancellation race; see [`WatchdogToken`]) leaves the isolate's
    /// termination flag set, after which *every* execute fails with
    /// "Uncaught Error: execution terminated" forever. Clear the flag and
    /// retry once: the caller's expression itself didn't run yet, so one
    /// clean retry fully masks the hiccup.
    pub(crate) fn execute_script_retry_terminated(
        &mut self,
        name: &'static str,
        source: String,
    ) -> Result<v8::Global<v8::Value>, String> {
        match self.runtime.execute_script(name, &source) {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("execution terminated") {
                    return Err(format!("JS error: {}", msg));
                }
                tracing::warn!("cleared stray V8 termination flag before {}", name);
                self.runtime.revive();
                self.runtime
                    .execute_script(name, &source)
                    .map_err(|e| format!("JS error: {}", e))
            }
        }
    }

    /// #39: run at every `run_event_loop` entry. A terminate_execution
    /// unwinds the JS stack without running finally blocks, so a trampoline
    /// bracket open at the fire leaks its depth counter forever and the
    /// drain chain defers into infinity while looking alive (fxg wedge:
    /// every later timer/fetch delivery rotted). At loop entry nothing JS
    /// is executing, so an unwind notification (set by the poll-boundary
    /// heal) or a stale flag still pending from a fire-while-parked means
    /// every bracket is leaked — clear the flag and reset the depth before
    /// this loop's pumps wake the chain.
    pub(crate) fn heal_termination_at_loop_entry(&mut self) {
        let unwound = self
            .termination_unwound
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        let stale_entry = self
            .stale_termination
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        if stale_entry {
            self.runtime.revive();
        }
        if unwound || stale_entry {
            let _ = self.runtime.execute_script(
                "<mt-recover>",
                "if (globalThis.__diting_mt_recover_termination) \
                 globalThis.__diting_mt_recover_termination()",
            );
            tracing::warn!("#39: reset macrotask depth after V8 termination");
        }
        // #227: the flags above only carry FIRES. A severed chain can
        // outlive them: the terminate's op-delivery batch evaporated with
        // the drain's op_sleep continuation AND its sibling 1500ms patrol
        // (same arm — cf. #226), and a quiet page (self-chained timers
        // died with the chain) pushes nothing, so the 250ms retire never
        // fires either — the fxg wedge: armed corpse, 160k backlog, probe
        // timers rotting while fetches settle fine. The pump is the only
        // independent witness left: ask the trampoline whether it stalled
        // and run the recover path. Its depth reset is safe here — nothing
        // JS is executing, so any positive depth is a leaked bracket.
        // #240: the timer wheel joined the witness contract. The
        // doudian-publish wedge (2026-10-09) formed with ZERO watchdog
        // fires anywhere in the log — timers/interval/rAF dead for the
        // rest of the session while fetches and eval-await stayed fine,
        // `_timerArmed` stale + patrol latched and no JS-side push able to
        // retry the arm. A due-past heap-top is the one wheel state a
        // healthy realm never shows; the recover path now force-heals it
        // (idempotent — a merely-behind page pays one redundant arm).
        // #240 face-2: the fetch ring joined too. A fetch whose op settled
        // but whose await/.then continuation never ran leaves a stall the
        // wheel witness cannot see (timers stay alive); the detector names
        // it, and the dump below rides the warn line so an unattended flow
        // wedge leaves the ring tail + wheel counters in the log — the
        // post-mortem previously required a human eval against a dead page.
        let stalled = self
            .evaluate(
                "globalThis.__diting_mt_stalled ? \
                 (__diting_mt_stalled() || __diting_wheel_stalled() \
                  || (globalThis.__diting_fetch_stalled && __diting_fetch_stalled())) : false",
            )
            .map(|v| v == serde_json::Value::Bool(true))
            .unwrap_or(false);
        if stalled {
            if let Ok(serde_json::Value::String(s)) =
                self.evaluate("globalThis.__diting_mt_dump ? __diting_mt_dump() : null")
            {
                tracing::warn!("#240 stall post-mortem (mt/wheel/fetch ring): {}", s);
            }
            let _ = self.runtime.execute_script(
                "<mt-force-recover>",
                "if (globalThis.__diting_mt_recover_termination) \
                 globalThis.__diting_mt_recover_termination()",
            );
            tracing::warn!("#227/#240: re-armed stalled macrotask chain / timer wheel");
        }
    }

    /// Total isolate terminations so far (see `watchdog_fired_total`).
    pub fn watchdog_fired_total(&self) -> u64 {
        self.watchdog_fired_total.get()
    }

    /// Cumulative wall time spent inside `poll_event_loop` over this realm's
    /// life (#66 busy accounting — see the field doc). Monotonic; callers
    /// diff it across a window.
    pub fn v8_active_ns(&self) -> u64 {
        self.v8_active_ns.get()
    }

    /// Queue a console entry from the Rust side (level, message), tagged
    /// with the current page URL like op_console does. The busy freeze uses
    /// this so session_console shows why a realm stopped pumping.
    pub fn push_console_entry(&self, level: &str, msg: &str) {
        let log_url = self.state.borrow().url.clone();
        self.state.borrow_mut().pending_console_calls.push((
            level.to_string(),
            msg.to_string(),
            log_url,
        ));
    }
}

#[cfg(test)]
mod cancel_grace_tests {
    use super::*;

    /// #332: a watchdog stuck holding the pair mutex (the post-fire state —
    /// a thread wedged inside terminate_execution never parks in the cvar
    /// again, so it never releases) must turn into a bounded leak, not a
    /// blocking lock + join. Healthy threads still signal and join.
    #[test]
    fn bounded_ceremony_leaks_a_stuck_holder_and_joins_a_healthy_one() {
        // Wedge: a thread that takes the pair mutex and stays parked on a
        // foreign condvar with the guard in hand — it never checks the
        // cancel flag and never exits.
        let pair = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let release =
            std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let pair_c = pair.clone();
        let release_c = release.clone();
        let stuck = std::thread::spawn(move || {
            // Take the pair mutex IN this thread, then park on a foreign
            // condvar with the guard in hand — the exact posture of a
            // watchdog wedged past its fire (never parks in the pair's own
            // cvar again, so it never observes the cancel flag).
            let _hold = pair_c.0.lock().unwrap();
            let (rlock, rcvar) = &*release_c;
            let mut done = rlock.lock().unwrap();
            while !*done {
                done = rcvar.wait(done).unwrap();
            }
        });
        // Let the wedge take effect before the ceremony starts.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
        let t0 = std::time::Instant::now();
        let (signalled, joined) = bounded_cancel_and_join(&pair, Some(stuck), deadline);
        let took = t0.elapsed();
        assert!(
            !signalled && !joined,
            "a mutex-holding never-exiting thread must be declared wedged"
        );
        assert!(
            took < std::time::Duration::from_secs(2),
            "the ceremony must bound, took {took:?}"
        );

        // Healthy shape: a thread that parks in its OWN pair's cvar (the
        // normal watchdog posture) wakes on the notify and exits; both
        // slots true and fast. Fresh pair — the wedged one is still held.
        let healthy_pair =
            std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let healthy = std::thread::spawn({
            let pair = healthy_pair.clone();
            move || {
                let (lock, cvar) = &*pair;
                let mut cancelled = lock.lock().unwrap();
                while !*cancelled {
                    cancelled = cvar.wait(cancelled).unwrap();
                }
            }
        });
        let (signalled, joined) = bounded_cancel_and_join(
            &healthy_pair,
            Some(healthy),
            std::time::Instant::now() + std::time::Duration::from_millis(300),
        );
        assert!(
            signalled && joined,
            "a parked, well-behaved watchdog must signal and join"
        );

        // Release the wedge so the leaked thread can die with the test.
        *release.0.lock().unwrap() = true;
        release.1.notify_one();
    }
}
