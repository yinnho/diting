use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use diting_env::env_knobs;

/// Re-exported so other crates (obscura-browser, obscura-cdp) can name the V8
/// isolate handle without taking a direct dependency on diting_runtime.
pub use v8::IsolateHandle;

use crate::module_loader::DitingModuleLoader;
use crate::ops::{our_decls, JsState};
use diting_runtime::vm::Vm;
use diting_runtime::ModuleId;

// The V8 termination watchdog family (token, stack-dump sampler, spawn) —
// split from the module root (god-file ratchet). Re-exported so the
// historical `diting_js::runtime::spawn_watchdog` path keeps resolving.
mod drop_serialized;
mod heap_guard;
mod pump;
mod watchdog;

use drop_serialized::{SerializedDropRuntime, ISOLATE_CONSTRUCT_LOCK};
use heap_guard::{install_heap_limit_guard, HeapLimitState};
pub use watchdog::{is_zombie_thread, set_zombie_thread_for_test, spawn_watchdog, WatchdogToken};

// The eval/callFunctionOn wrapper-source builders (INLINE_EVAL_JS trampoline,
// object slots, meta extraction, the #114 await-settle wrappers).
mod eval_code;
// Expression evaluation and the V8-value-to-JSON conversion.
mod eval;

// The Chrome RemoteObject wire shapes (number spelling, info_from_json/meta
// constructors) — split from the module root (god-file ratchet).
mod remote_eval;
mod remote_object;
pub use remote_eval::{EvalOutcome, ExceptionInfo, DEFAULT_AWAIT_BUDGET_MS};
pub use remote_object::RemoteObjectInfo;

// The host→realm wiring setters/drain family — split from the module root
// (god-file ratchet). wiring stays free of script execution.
mod wiring;
// Navigator persona writes enter V8 (UA, fingerprint seed, language, platform).
mod persona;
// ES module fetch and evaluation. The raw script entry stays in this file.
mod module_eval;

mod snapshot_blob {
    include!(concat!(env!("OUT_DIR"), "/snapshot_blob.rs"));
}
use snapshot_blob::SNAPSHOT;

pub struct JsRuntime {
    runtime: SerializedDropRuntime,
    state: Rc<RefCell<JsState>>,
    object_store: HashMap<String, String>,
    object_counter: u64,
    /// Thread-safe handle to this runtime's V8 isolate, captured at
    /// construction. Lets a watchdog be armed from `&self` (the CDP dispatcher
    /// only holds `&Page` on the hot path) and is stable for the isolate's life.
    isolate_handle: IsolateHandle,
    /// Set by the near-heap-limit callback when it had to terminate a script.
    /// The next V8 entry point recovers the isolate (cancel termination,
    /// restore the real limit) before running more JS.
    heap_limit_state: std::sync::Arc<HeapLimitState>,
    /// How many times a watchdog had to terminate the isolate. Read before/after
    /// an event-loop pump to detect that a page is storming (a terminated
    /// microtask loop re-queues itself on the next pump, so pumping it again
    /// just re-feeds the storm).
    watchdog_fired_total: std::cell::Cell<u64>,
    /// #71: when the last #240 stall post-mortem was written, and how many
    /// hits were folded since — the dump is rate-limited per realm.
    stall_log_at: std::cell::Cell<Option<std::time::Instant>>,
    stall_log_folded: std::cell::Cell<u64>,
    /// #66: cumulative wall time actually spent inside `poll_event_loop`
    /// across this realm's life (nanoseconds). Pure execution time — a poll
    /// that finds nothing ready registers the reactor bookkeeping only, so
    /// pages parked waiting on timers barely move it. The idle pump diffs
    /// this over a trailing window to catch burners whose every callback
    /// stays under each watchdog budget (they never fire one, so
    /// `watchdog_fired_total` alone cannot see them).
    v8_active_ns: std::cell::Cell<u64>,
    /// #50: set by the watchdog thread at the moment it calls
    /// `terminate_execution()`. `IsolateHandle::is_execution_terminating()`
    /// only reports an *active* termination (one propagating on the stack);
    /// a fire that lands while the session task is parked leaves the flag
    /// pending and invisible until the next V8 entry — by then it kills the
    /// op-delivery tick and drops the batch. `run_event_loop` consults THIS
    /// flag at every poll boundary and cancels the stale termination.
    stale_termination: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// #39: set by the `run_event_loop` poll heal whenever it clears a
    /// termination. A terminate_execution unwinds the JS stack without
    /// running finally blocks, so any trampoline bracket open at the fire
    /// leaks — the next loop entry (nothing JS executing then) resets it
    /// via the bootstrap recovery hook.
    termination_unwound: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// #332: set by a WatchdogToken whose cancel ceremony blew
    /// `WATCHDOG_CANCEL_GRACE` (the watchdog thread is wedged inside
    /// terminate_execution, or in its own exit). Flows into every token at
    /// arm time so the orphaned-arm Drop path can report it too; this
    /// struct's Drop reads it to skip the isolate teardown — the wedged
    /// thread holds the rusty_v8 handle mutex that
    /// `IsolateHandle::dispose()` would block on forever.
    watchdog_wedge: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Per-module evaluation outcome cache (upstream 4f6d256): browsers
    /// evaluate a module script exactly once per document. Our module map
    /// treats a second mod_evaluate of the same ModuleId like the spec's
    /// module-map no-op instead of panicking, so a page loading the same
    /// module URL twice (duplicate <script type=module src>, or a root
    /// already evaluated earlier as another graph's dependency) stays
    /// legal without this — the cache keeps the outcome readable too.
    module_evaluations: HashMap<ModuleId, Result<(), String>>,
    /// The ES-module loader this runtime was built with, kept so
    /// `set_http_client` can hand it the context client (the Rc also lives
    /// inside the Vm's module map, which gives nothing back).
    module_loader: Rc<DitingModuleLoader>,
}

/// #332 leak-degrade: if any watchdog of this realm wedged (see
/// [`crate::runtime::WatchdogToken`]'s cancel ceremony), the
/// isolate handle's rusty_v8 mutex is still in the wedged thread's hands —
/// the normal `SerializedDropRuntime` teardown would block on it forever,
/// one step after the join already gave up. Mark the leak BEFORE the field
/// drops run (`runtime` is the first field, so its Drop is the first to
/// fire and cannot intervene itself).
impl Drop for JsRuntime {
    fn drop(&mut self) {
        if self
            .watchdog_wedge
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            self.runtime.mark_isolate_wedged();
        }
    }
}

/// Sync callers (tests) have no ambient tokio runtime, but the
/// async op BODIES (op_sleep's Handle::try_current guard, the fetch/ws
/// futures' reactor) need one or they silently no-op. This enters a
/// process-lifetime background runtime; the returned guard must
/// outlive the Vm construction so the first op spawn finds the
/// handle. Delayed tasks keep spawning afterward through the stored
/// `Handle` clone, which stays valid because the runtime never drops.
static BACKGROUND_TOKIO: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn enter_tokio_context() -> Option<tokio::runtime::EnterGuard<'static>> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return None;
    }
    let runtime = BACKGROUND_TOKIO.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("background tokio runtime for V8 platform tasks")
    });
    Some(runtime.enter())
}

/// V8 flag one-shots that MUST run before the process's first isolate:
/// after V8 initializes, flag writes on frozen flags are a V8_Fatal.
/// Called under `ISOLATE_CONSTRUCT_LOCK` by the realm constructor (the
/// retired serve-start warmup was the other caller; every construction
/// path sets these inside the lock, so deferring V8 init to the first
/// session is race-free) — the statics live here so every caller
/// shares the same process-wide `Once`s.
fn v8_flags_before_first_isolate() {
    // One-shot before the first isolate: raise V8's own JS stack
    // ceiling. The default (~984 KB) is fine for hand-written code,
    // but minified SPA bundles (juejin.cn class) recurse past it and
    // the page dies with `RangeError: Maximum call stack size
    // exceeded` before it renders. The flag is in KB; keep a 2 MB
    // margin under the hosting thread's native stack, which callers
    // size to match (config::js_stack_mb, default 32 MB).
    static V8_STACK_FLAG: std::sync::Once = std::sync::Once::new();
    V8_STACK_FLAG.call_once(|| {
        let kb = env_knobs::js_stack_mb().saturating_sub(2).max(1) * 1024;
        // Returns (); V8 itself logs to stderr if a flag is rejected,
        // which would leave the default ~984 KB ceiling in place.
        // Must precede V8 init: lets the document.all mint below flip
        // --allow-natives-syntax on and off again (frozen flags make
        // any later set_flags_from_string a V8_Fatal).
        v8::V8::set_flags_from_string("--no-freeze-flags-after-init");
        // Chrome 157 ships Iterator.concat and stages Iterator.zip /
        // zipKeyed. V8 installs both while restoring a context, and
        // fatals if a snapshot script already owns the name. Leave
        // the three names to V8; do not define them in tail_members.
        v8::V8::set_flags_from_string("--js-iterator-sequencing");
        v8::V8::set_flags_from_string("--js-joint-iteration");
        v8::V8::set_flags_from_string(&format!("--stack-size={kb}"));
    });
    // #224: same one-shot shape for the old-space ceiling. V8's
    // default (~1400MB) stood — doudian's publish boot crossed it
    // mid-commit and the heap guard severed the form init chain
    // (getCategoryDetail took 7 minutes to complete; getSchema
    // retries lost their op deliveries entirely). Commerce-scale
    // heaps are legal; raise the ceiling and let the pressure-GC
    // escalation (#224 A1) handle a page that still won't fit.
    // `DITING_JS_HEAP_MB=0` keeps V8's default.
    static V8_HEAP_FLAG: std::sync::Once = std::sync::Once::new();
    V8_HEAP_FLAG.call_once(|| {
        let mb = env_knobs::js_heap_mb();
        if mb > 0 {
            v8::V8::set_flags_from_string(&format!("--max-old-space-size={mb}"));
        }
    });
    // Forensics escape hatch (#225): verbatim extra flags, applied
    // before the first isolate like the two above. `--prof` is the
    // one that motivated the knob — its sampler thread sees wedges
    // that ignore interrupt checks.
    static V8_EXTRA_FLAG: std::sync::Once = std::sync::Once::new();
    V8_EXTRA_FLAG.call_once(|| {
        let extra = env_knobs::v8_extra_flags();
        if !extra.trim().is_empty() {
            v8::V8::set_flags_from_string(extra.trim());
        }
    });
}

impl JsRuntime {
    /// #276 idle shrink: full-GC the realm (delegates to Vm::shrink_heap).
    /// Called by Page at idle command boundaries; see the knob doc.
    pub fn shrink_heap(&mut self) {
        self.runtime.shrink_heap();
    }

    pub fn new() -> Self {
        Self::with_base_url("about:blank")
    }

    pub fn with_base_url(base_url: &str) -> Self {
        Self::with_base_url_and_proxy(base_url, None)
    }

    /// Construct a runtime whose ES-module loader routes dynamic imports
    /// through `proxy_url` (#139). `None` is equivalent to `with_base_url`
    /// (direct connection).
    pub fn with_base_url_and_proxy(base_url: &str, proxy_url: Option<String>) -> Self {
        diting_css::props::clear_js(); // a new realm forgets script-registered properties
        let state = Rc::new(RefCell::new(JsState::new()));
        let state_clone = state.clone();
        let import_map = state.borrow().import_map.clone();
        let blob_store = state.borrow().blob_store.clone();

        let module_loader = Rc::new(DitingModuleLoader::with_proxy_and_import_map(
            base_url,
            proxy_url,
            import_map.clone(),
            blob_store,
        ));

        // Our Vm spawns no V8-platform tasks of its own, but async op
        // BODIES park on tokio (op_sleep guards with Handle::try_current;
        // the fetch/ws futures need the reactor) — sync callers (tests)
        // have no ambient runtime, so the ops would silently
        // no-op. Enter a process-lifetime background runtime; the guard
        // needs to outlive only the construction below, but keeping it to
        // the end of the function costs nothing.
        let _tokio_context = enter_tokio_context();

        // Serialize isolate construction process-wide: V8's JSDispatchTable
        // setup is not safe to run from several threads at once, and sessions
        // plus one-shot ops each construct on their own thread concurrently
        // (upstream obscura hit this under thread-per-connection, #430).
        // Disposal serializes through the same lock — see
        // `SerializedDropRuntime` (#108).
        let mut runtime = {
            let _construct_guard = ISOLATE_CONSTRUCT_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            v8_flags_before_first_isolate();
            // #276 shared-source campaign: restore-side const mint (pure
            // data — OneByteConst needs no isolate; the build script did
            // the bake-side twin).
            diting_runtime::snapshot::ensure_script_consts(
                crate::snapshot_sources::SNAPSHOT_SRC_ASCII,
            );
            Vm::from_snapshot(SNAPSHOT).expect("our snapshot container revives")
        };
        // #276 memory forensics: point A — the heap as the snapshot restore
        // left it, before any init script runs.
        if env_knobs::realm_heap_stats() {
            let h = runtime.heap_statistics();
            tracing::info!(
                "realm heap [A post-restore]: used {} total {} physical {}",
                h.used_heap_size(),
                h.total_heap_size(),
                h.total_physical_size()
            );
        }

        // 10d law: the ops land INSIDE the Deno.core.ops namespace object the
        // snapshot baked (bootstrap.js captured it as `const _OPS` — object
        // identity survives the snapshot restore). Never on globalThis: op
        // names as window properties would change the fingerprint face.
        let op_state = runtime.install_ops_under(our_decls(), &["Deno", "core", "ops"]);
        op_state.borrow_mut().put(state_clone);
        // #226: async fetch completion mirror — see FetchSettlements in ops.
        op_state
            .borrow_mut()
            .put(crate::ops::FetchSettlements::default());

        runtime.install_modules(module_loader.clone());

        runtime
            .execute_script(
                "<anonymous>"  /* was diting:init; page-visible Error.stack */,
                "globalThis.__diting_objects = {}; globalThis.__diting_oid = 0; globalThis.__diting_init();",
            )
            .expect("init should not fail");

        // document.all needs a real [[IsHTMLDDA]] object (typeof "undefined"
        // yet indexable/callable); only V8's `%GetUndetectable()` can mint
        // one. Natives syntax is on for exactly this one compile, under the
        // construction lock, then switched off so page scripts never parse
        // `%Foo()` (Chrome: SyntaxError).
        {
            let _natives_guard = ISOLATE_CONSTRUCT_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            v8::V8::set_flags_from_string("--allow-natives-syntax");
            let minted = runtime.execute_script(
                "<anonymous>",
                "globalThis.__diting_install_all(%GetUndetectable()); delete globalThis.__diting_install_all;",
            );
            v8::V8::set_flags_from_string("--no-allow-natives-syntax");
            if let Err(e) = minted {
                tracing::warn!("document.all undetectable object not installed: {}", e);
            }
        }

        // #276: post-init shrink — the knob trades ~2ms per realm build
        // for ~1.4MB physical per realm (see Vm::shrink_heap).
        // Concurrency gate (#276): under big pools (crawl shape, 25 ways)
        // the per-navigation GC measured ~8ms CPU with lock contention —
        // +28s wall on the 940-page crawl for −27MB. Few-session
        // deployments (the parked-console shape, where it's free) still
        // shrink.
        if env_knobs::realm_shrink() && diting_runtime::live_isolates() <= 8 {
            let t0 = std::time::Instant::now();
            runtime.shrink_heap();
            if env_knobs::realm_heap_stats() {
                let h = runtime.heap_statistics();
                tracing::info!(
                    "realm heap [S shrunk]: used {} total {} physical {} shrink_ms {:?}",
                    h.used_heap_size(),
                    h.total_heap_size(),
                    h.total_physical_size(),
                    t0.elapsed()
                );
            }
        }
        // #276 memory forensics: point B — after ops install, init script,
        // natives mint; this is the per-realm steady baseline a page starts
        // from.
        if env_knobs::realm_heap_stats() {
            let h = runtime.heap_statistics();
            tracing::info!(
                "realm heap [B post-init]: used {} total {} physical {}",
                h.used_heap_size(),
                h.total_heap_size(),
                h.total_physical_size()
            );
        }
        let isolate_handle = runtime.isolate_handle();
        let heap_limit_state = std::sync::Arc::new(HeapLimitState::default());
        install_heap_limit_guard(
            &mut runtime,
            isolate_handle.clone(),
            heap_limit_state.clone(),
        );

        JsRuntime {
            runtime: SerializedDropRuntime::new(runtime),
            state,
            object_store: HashMap::new(),
            object_counter: 0,
            isolate_handle,
            heap_limit_state,
            watchdog_fired_total: std::cell::Cell::new(0),
            stall_log_at: std::cell::Cell::new(None),
            stall_log_folded: std::cell::Cell::new(0),
            v8_active_ns: std::cell::Cell::new(0),
            stale_termination: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            termination_unwound: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            watchdog_wedge: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            module_evaluations: HashMap::new(),
            module_loader,
        }
    }

    pub fn execute_script(&mut self, _name: &str, source: &str) -> Result<(), String> {
        self.runtime
            .execute_script("<script>", source)
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(())
    }

    pub async fn run_event_loop(&mut self) -> Result<(), String> {
        self.run_event_loop_checked(None).await.map(|_| ())
    }

    /// [`Self::run_event_loop`] with a host check after EVERY loop pass:
    /// `stop` returning true ends the run (`Ok(true)`). The eval wait uses it
    /// so a settled result is seen at the next task boundary instead of after
    /// whatever the page queued behind it (see `resolve_promises_until`).
    pub(crate) async fn run_event_loop_checked(
        &mut self,
        mut stop: Option<&mut dyn FnMut(&mut Self) -> bool>,
    ) -> Result<bool, String> {
        self.recover_heap_limit();
        self.heal_termination_at_loop_entry();
        // #50: the watchdog armed around an await window can fire while the
        // session task is parked with no JS on the stack (in the captured
        // repro the task was starved 14.65s past the wall-clock deadline).
        // The terminate flag then sits stale on the isolate, and the first
        // V8 entry after wake — usually our op-delivery settle — dies
        // with "execution terminated", dropping the whole batch of
        // already-popped op results: fetch promises never settle. Detection
        // cannot ask V8: `is_execution_terminating()` only reports an
        // ACTIVE termination (one propagating on the stack) — a fire that
        // landed on a parked isolate leaves the flag pending and reads
        // false. So the watchdog sets our own `stale_termination` flag at
        // the instant of firing; a flag set at a poll boundary is
        // necessarily stale (a genuine synchronous overrun unwinds with Err
        // inside the poll that ran the JS — this heal already ran at the
        // top of that poll). Clear both.
        let isolate = self.isolate_handle.clone();
        let stale = self.stale_termination.clone();
        let unwound_flag = self.termination_unwound.clone();
        // Our host's loop face (10d): one `poll_event_loop` pass IS the
        // reactor pass — async ops and module loads park on this waker
        // (tokio's), settlement delivery and the microtask drain run inside.
        // LoopActivity tells idle from busy: idle completes this future,
        // busy parks in the poll_fn until the next wake.
        let result: Result<bool, String> = std::future::poll_fn(|cx| {
            if stale.swap(false, std::sync::atomic::Ordering::SeqCst) {
                isolate.cancel_terminate_execution();
                unwound_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                tracing::warn!("#50: cleared stale V8 termination before event-loop poll");
            }
            // #66: time the poll itself. Ready tasks (timers, promise
            // continuations, message handlers) execute INSIDE this call;
            // a poll that finds nothing ready costs only reactor
            // bookkeeping. Cumulative delta over a trailing window is the
            // honest "how hot is this realm" signal — parked-waiting time
            // doesn't count, so a page polling on a slow timer never trips
            // the busy freeze while an under-budget interval burner does.
            let poll_t0 = std::time::Instant::now();
            let poll_result = self.runtime.poll_event_loop(cx);
            self.v8_active_ns
                .set(self.v8_active_ns.get() + poll_t0.elapsed().as_nanos() as u64);
            match poll_result {
                Ok(_) if stop.as_mut().is_some_and(|f| f(self)) => std::task::Poll::Ready(Ok(true)),
                Ok(activity) if activity.idle() => std::task::Poll::Ready(Ok(false)),
                Ok(_) => std::task::Poll::Pending,
                Err(e) => std::task::Poll::Ready(Err(e.to_string())),
            }
        })
        .await;
        if let Err(e) = &result {
            // An event-loop error (e.g. an exception inside the settle
            // delivery while resolving op promises) is the silent-loss
            // surface of #50: results already popped from the completed-ops
            // deque are dropped when the delivery aborts, and every pump
            // wrapper (run_event_loop_until_idle / _bounded / `let _ =`)
            // discards the Err. Log it here so the loss leaves a signature.
            tracing::warn!("event loop error (op results may be lost): {}", e);
        }
        result.map_err(|e| format!("Event loop error: {}", e))
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
