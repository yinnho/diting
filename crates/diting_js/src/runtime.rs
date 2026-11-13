use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use diting_dom::DomTree;
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
use watchdog::watchdog_terminate;
pub use watchdog::{is_zombie_thread, set_zombie_thread_for_test, spawn_watchdog, WatchdogToken};

// The eval/callFunctionOn wrapper-source builders (INLINE_EVAL_JS trampoline,
// object slots, meta extraction, the #114 await-settle wrappers).
mod eval_code;

// The Chrome RemoteObject wire shapes (number spelling, info_from_json/meta
// constructors) — split from the module root (god-file ratchet).
mod remote_eval;
mod remote_object;

// The host→realm wiring setters/drain family — split from the module root
// (god-file ratchet).
mod wiring;

mod snapshot_blob {
    include!(concat!(env!("OUT_DIR"), "/snapshot_blob.rs"));
}
use snapshot_blob::SNAPSHOT;

/// Budget for awaiting a script's promise before declaring an eval timeout.
/// The session eval face exposes this per-call (`timeout_ms`); callers that
/// do slow page-side work (uploads through the page's own fetch) pass a
/// larger budget instead of hitting a silent-null (0.4.1 taobao report).
pub const DEFAULT_AWAIT_BUDGET_MS: u64 = 5000;

/// Rich evaluate result: typed remote-object metadata plus optional JSON
/// `value`. HTTP/session callers usually read `value`; `object_id` /
/// `subtype` / `class_name` support DOM-handle style follow-ups without a
/// separate wire protocol.
#[cfg_attr(not(test), allow(dead_code))] // tests cross-check the type metadata
#[derive(Debug, Clone)]
pub struct RemoteObjectInfo {
    pub js_type: String,
    pub subtype: Option<String>,
    pub class_name: String,
    /// CDP preview text ("Object" / "Array(3)" …). Nothing renders it yet.
    #[allow(dead_code)]
    pub description: String,
    pub object_id: Option<String>,
    pub value: Option<serde_json::Value>,
}

/// Outcome of [`JsRuntime::evaluate_remote_outcome`] / callFunctionOn: the
/// remote object plus any thrown (or rejected) exception. Captured separately so callers can
/// emit `Runtime.exceptionThrown` + `exceptionDetails` instead of collapsing a
/// throw into a plain `undefined`/`null` result.
#[derive(Debug, Clone)]
pub struct EvalOutcome {
    pub info: RemoteObjectInfo,
    pub exception: Option<ExceptionInfo>,
}

/// Details of a sync-thrown or awaited-rejection exception, enough to
/// surface the exception instead of collapsing it to `undefined`.
#[derive(Debug, Clone)]
pub struct ExceptionInfo {
    /// "Uncaught" for a sync throw, "Uncaught (in promise)" for a rejection.
    pub text: String,
    /// Human-readable message, e.g. "Error: boom".
    pub description: String,
    /// Error constructor name, e.g. "Error" / "TypeError".
    pub class_name: String,
    /// Object-store id of the error object, when one was allocated.
    pub object_id: Option<String>,
    /// First stack frame, e.g. "at <anonymous>:1:13" — the throw site.
    pub stack_first: Option<String>,
    /// Line/column of the user script frame (the `<anonymous>:L:C` frame),
    /// when one was found — bootstrap eval wrapper frames are skipped so
    /// the position points into the caller's script.
    pub line: Option<u32>,
    pub col: Option<u32>,
}

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

/// Extract a display message from a caught panic payload (`&str`, `String`,
/// or anything else reduced to "unknown panic").
fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
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

    pub fn set_user_agent(&mut self, ua: &str) {
        let escaped = ua.replace('\\', "\\\\").replace('\'', "\\'");
        // After the UA lands, refresh the platform persona (GPU pool, screen,
        // dpr, hw/memory). The runtime constructor ran __diting_init before
        // any UA was known, so the persona materialized from the linux
        // default — leaving Mesa GL strings behind a macOS UA.
        let _ = self.runtime.execute_script(
            "<set-ua>",
            &format!(
                "globalThis.__diting_ua = '{}'; \
                 globalThis._fpCache = null; globalThis.__diting_hw_plat = undefined; \
                 globalThis.__diting_setPersona();",
                escaped
            ),
        );
    }

    /// Re-pin the hardware-persona seed on a freshly booted realm. The realm
    /// is rebuilt per navigation and `__diting_init` self-deletes after
    /// drawing, so cross-page continuity of screen/dpr/GPU/canvas comes from
    /// the Page handing the same seed back every time (see Page::fp_seed).
    pub fn set_fingerprint_seed(&mut self, seed: u64) {
        let s32 = (seed & 0xFFFF_FFFF) as u32;
        let _ = self.runtime.execute_script(
            "<set-fp-seed>",
            &format!("globalThis.__diting_setFpSeed({});", s32),
        );
    }
    /// Move only the navigator.language(s) persona (the `__diting_lang`
    /// global behind the bootstrap live getters). Deliberately does NOT
    /// re-pin the ICU default: `set_default_locale` is process-global and
    /// the per-isolate Intl cache is sticky anyway (obscura #734), so a
    /// mid-session re-pin could not take effect and would contaminate
    /// sibling isolates (obscura #778 hazard). Intl locale-argument
    /// binding follows via the bootstrap wrapper reading this same global.
    pub fn set_navigator_language(&mut self, lang: &str) {
        let escaped = lang.replace('\\', "\\\\").replace('\'', "\\'");
        let _ = self.runtime.execute_script(
            "<set-lang>",
            &format!("globalThis.__diting_lang = '{}';", escaped),
        );
    }
    /// Move only the navigator.platform persona (the CDP `platform` field of
    /// setUserAgentOverride — per protocol it targets navigator.platform
    /// only, never the UA string, the transport headers, or
    /// userAgentData.platform, which is userAgentMetadata territory).
    pub fn set_navigator_platform(&mut self, platform: &str) {
        let escaped = platform.replace('\\', "\\\\").replace('\'', "\\'");
        let _ = self.runtime.execute_script(
            "<set-platform>",
            &format!("globalThis.__diting_platform_override = '{}';", escaped),
        );
    }
    pub fn set_language(&mut self, lang: &str) {
        self.set_navigator_language(lang);
        // Pin ICU's default locale to the SAME source (obscura#734 lineage):
        // V8's Intl follows the process locale otherwise, so a non-matching
        // LANG leaves Intl.DateTimeFormat().resolvedOptions().locale (and
        // every Intl default) disagreeing with navigator.language and the
        // Accept-Language header the net layer sends - a three-way locale
        // mismatch that's a hard headless tell. Take the first q-weights-
        // stripped BCP-47 tag (the same fold __ditingLangList does for
        // navigator.language).
        let first_tag = lang
            .split(',')
            .next()
            .and_then(|t| t.split(';').next())
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .unwrap_or("zh-CN");
        v8::icu::set_default_locale(first_tag);
    }

    /// Whether the page has nothing that could fire inside the next
    /// `window_ms` (see `__diting_settle_quiet`). Any failure reads as "not
    /// quiet" so the settle loop keeps its full budget.
    pub fn settle_quiet(&mut self, window_ms: u64) -> bool {
        self.evaluate(&format!("globalThis.__diting_settle_quiet({window_ms})"))
            .map(|v| v == serde_json::Value::Bool(true))
            .unwrap_or(false)
    }

    pub fn evaluate(&mut self, expression: &str) -> Result<serde_json::Value, String> {
        self.recover_heap_limit();
        let wrapped = Self::wrap_expression(expression);
        let result = self.execute_script_retry_terminated("<eval>", wrapped)?;
        self.v8_to_json(result)
    }

    pub async fn load_module(&mut self, url: &str, budget_ms: u64) -> Result<(), String> {
        let budget = tokio::time::Duration::from_millis(budget_ms);
        let specifier =
            url::Url::parse(url).map_err(|e| format!("Invalid module URL {}: {}", url, e))?;

        // Fetch the module source. The old impl registered an empty string
        // and called it loaded, so every Vite / Next module bundle "loaded"
        // in 1ms with zero code and the SPA never mounted (issue #205).
        // Failures now propagate (upstream be700f5): a 404/500 must not be
        // evaluated as an empty module and reported as loaded.
        let client = self.state.borrow().http_client.clone();
        let source_code = match client {
            Some(c) => {
                let resp = c
                    .fetch_with_callbacks(
                        &specifier,
                        None,
                        diting_net::ResourceType::ModuleScript,
                        Some(self.module_loader.base_url.as_str()),
                    )
                    .await
                    .map_err(|e| format!("Module fetch failed ({}): {}", url, e))?;
                if !(200..=299).contains(&resp.status) {
                    return Err(format!("Module {} returned HTTP {}", url, resp.status));
                }
                diting_net::decode_non_html(&resp.body, resp.content_type())
            }
            None => {
                return Err(format!(
                    "No http_client wired to runtime; cannot fetch module {}",
                    url
                ));
            }
        };

        // Bound the graph fetch: submodules resolve recursively through the
        // module loader, so a slow import chain must not hang page load.
        let module_id = match tokio::time::timeout(
            budget,
            self.runtime
                .load_side_es_module_from_code(specifier.as_str(), source_code),
        )
        .await
        {
            Ok(r) => r.map_err(|e| format!("Module load error: {}", e))?,
            Err(_) => {
                return Err(format!(
                    "Module graph load timed out after {}ms: {}",
                    budget_ms, url
                ));
            }
        };

        self.drive_module_eval(module_id, budget_ms, &format!("Module {}", url))
            .await
    }

    /// Drive a just-started module evaluation to completion, or up to
    /// `budget_ms`. Returns as soon as the module finishes rather than waiting
    /// for the event loop to go idle: a page timer (Vite's HMR client installs
    /// a setInterval) keeps the loop busy forever, and waiting for idle burned
    /// the whole budget on an early module and starved the one that mounts the
    /// app, leaving #root empty (upstream #374). The outcome is cached per
    /// ModuleId: browsers evaluate a module exactly once per document; our
    /// module map no-ops a second evaluation (Evaluated short-circuit), and
    /// the cache hands the FIRST outcome back for duplicate roots and roots
    /// already evaluated as another graph's dependency.
    async fn drive_module_eval(
        &mut self,
        module_id: ModuleId,
        budget_ms: u64,
        what: &str,
    ) -> Result<(), String> {
        if let Some(outcome) = self.module_evaluations.get(&module_id) {
            return outcome.clone();
        }

        let budget = tokio::time::Duration::from_millis(budget_ms);
        // Backstop for the whole drive, not just the async half. `mod_evaluate`
        // runs the module's top-level code SYNCHRONOUSLY inside the call, and
        // the event-loop poll below runs ready page callbacks — either can pin
        // the thread inside V8, where the tokio budget cannot fire (and neither
        // can the 30s navigation deadline: the current-thread runtime worker
        // itself is blocked, so the HTTP caller gets no answer at all —
        // weixin's appmsg.js did exactly this, hung >2min with no response).
        // Same shape as the post-script settle loop: watchdog 250ms past the
        // budget; disarm cancels/heals a fired termination so the phases after
        // this module still run. Chrome semantics: a killed module ends the
        // module, never the page's load lifecycle.
        let eval_wd = self.arm_watchdog(budget + tokio::time::Duration::from_millis(250));
        // Contained-panic belt (module machinery panics stay module errors,
        // never page-process deaths). The deno_core 0.350 "Module already
        // evaluated" panic face is gone with the host — our map no-ops —
        // but a panic here is still success-shaped when it IS a repeat.
        let evaluation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.runtime.mod_evaluate(module_id)
        }));
        let result = match evaluation {
            Ok(result) => result,
            Err(payload) => {
                let _ = self.disarm_watchdog(eval_wd);
                let message = panic_payload_message(payload);
                let outcome = if message.contains("Module already evaluated") {
                    Ok(())
                } else {
                    Err(format!("{} evaluation panicked: {}", what, message))
                };
                self.module_evaluations.insert(module_id, outcome.clone());
                return outcome;
            }
        };
        tokio::pin!(result);

        // The event-loop arm is polled first (biased): a ready loop error
        // must surface instead of being discarded while awaiting the result.
        // #50: Self::run_event_loop clears a stale watchdog termination at
        // every poll; both arms map to String so the match below is unchanged.
        let outcome = tokio::time::timeout(budget, async {
            let event_loop = self.run_event_loop();
            tokio::pin!(event_loop);
            tokio::select! {
                biased;
                e = &mut event_loop => {
                    e?;
                    (&mut result).await.map_err(|err| err.to_string())
                }
                r = &mut result => r.map_err(|err| err.to_string()),
            }
        })
        .await;
        let wd_fired = self.disarm_watchdog(eval_wd);
        if wd_fired {
            tracing::warn!(
                "{} evaluation watchdog fired; isolate recovered for later scripts",
                what
            );
        }

        // An eval error or timeout is returned to the page lifecycle. The
        // caller may keep rendering, but must not report the module as loaded.
        let outcome = match outcome {
            Ok(Ok(())) => Ok(()),
            // A fired watchdog makes the eval unwind with "execution
            // terminated" — report the timeout, not the termination symptom.
            Ok(Err(_)) if wd_fired => Err(format!(
                "{} evaluation timed out after {}ms",
                what, budget_ms
            )),
            Ok(Err(e)) => Err(format!("{} eval error: {}", what, e)),
            Err(_) => Err(format!(
                "{} evaluation timed out after {}ms",
                what, budget_ms
            )),
        };
        self.module_evaluations.insert(module_id, outcome.clone());
        outcome
    }

    pub async fn load_inline_module(
        &mut self,
        code: &str,
        base_url: &str,
        budget_ms: u64,
    ) -> Result<(), String> {
        let budget = tokio::time::Duration::from_millis(budget_ms);
        let specifier = url::Url::parse(&format!(
            "{}#inline-module-{}",
            base_url, self.object_counter
        ))
        .unwrap_or_else(|_| url::Url::parse("about:blank").unwrap());

        self.object_counter += 1;

        let module_id = match tokio::time::timeout(
            budget,
            self.runtime
                .load_side_es_module_from_code(specifier.as_str(), code.to_string()),
        )
        .await
        {
            Ok(r) => r.map_err(|e| format!("Inline module load error: {}", e))?,
            Err(_) => {
                return Err(format!(
                    "Inline module graph load timed out after {}ms",
                    budget_ms
                ));
            }
        };

        // Same completion-not-idle semantics as load_module (upstream #374):
        // an inline preamble module that installs a timer must not burn the
        // whole budget waiting for an idle that never comes.
        self.drive_module_eval(module_id, budget_ms, "Inline module")
            .await
    }

    pub fn execute_script(&mut self, _name: &str, source: &str) -> Result<(), String> {
        self.runtime
            .execute_script("<script>", source)
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(())
    }

    pub fn execute_script_guarded(&mut self, _name: &str, source: &str) -> Result<(), String> {
        if source.len() < 10_000 {
            self.execute_script(_name, source)
        } else {
            self.execute_script_with_timeout(source, std::time::Duration::from_secs(5))
        }
    }

    pub fn execute_script_with_timeout(
        &mut self,
        source: &str,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        if timeout.is_zero() {
            self.runtime
                .execute_script("<script>", source)
                .map_err(|e| format!("JS error: {}", e))?;
            return Ok(());
        }

        let isolate_handle = self.runtime.isolate_handle();

        let pair = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let pair_clone = pair.clone();

        let watchdog = std::thread::spawn(move || {
            let (lock, cvar) = &*pair_clone;
            let mut cancelled = lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let deadline = std::time::Instant::now() + timeout;

            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    watchdog_terminate(&isolate_handle);
                    return;
                }

                let result = cvar
                    .wait_timeout(cancelled, remaining)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                cancelled = result.0;
                if *cancelled {
                    return;
                }
            }
        });

        let result = self.runtime.execute_script("<script>", source);

        {
            let (lock, cvar) = &*pair;
            let mut cancelled = lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *cancelled = true;
            cvar.notify_one();
        }
        let _ = watchdog.join();

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Uncaught Error: execution terminated") {
                    // Clear the termination NOW. The isolate was terminated by
                    // a watchdog (usually this guard's own 5s one, possibly a
                    // phase watchdog that fired mid-script) — and V8 keeps
                    // the termination flag pending once the script unwinds.
                    // Left set, every subsequent execution on the isolate
                    // fails instantly: the next scripts "die in 5s" they never
                    // ran, unguarded ones error out, module evals come back
                    // "Uncaught null" (weixin article pages hit all three).
                    // Chrome semantics: a killed script ends the script,
                    // never the page — the page's remaining scripts keep
                    // running. cancel_terminate_execution is the same
                    // healing disarm_watchdog applies at its phase boundary.
                    self.runtime.revive();
                    // #227: this guard arms a raw thread with no stale
                    // flag, so the #39/#50 heals are blind to its fires —
                    // a dispatch window beheaded here leaks _mtDepth and
                    // every later chain defer-spins on it forever. Forward
                    // the unwind for the next loop entry.
                    self.termination_unwound
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    tracing::warn!("Script killed after {}s timeout", timeout.as_secs());
                    self.runtime.execute_script("<reset>", "undefined").ok();
                    Ok(())
                } else {
                    Err(format!("JS error: {}", msg))
                }
            }
        }
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

    /// Like [`Self::evaluate`] but bounded by a V8 watchdog, so a `--eval`
    /// expression that loops forever (or awaits a promise that never settles in
    /// synchronous form) cannot hang the process.
    pub fn evaluate_with_timeout(
        &mut self,
        expression: &str,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, String> {
        if timeout.is_zero() {
            return self.evaluate(expression);
        }
        self.recover_heap_limit();
        let wrapped = Self::wrap_expression(expression);
        let token = self.arm_watchdog(timeout);
        let result = self.runtime.execute_script("<eval>", &wrapped);
        let fired = self.disarm_watchdog(token);
        match result {
            Ok(v) if !fired => self.v8_to_json(v),
            Ok(_) => Err("eval timed out".to_string()),
            Err(e) => {
                let msg = e.to_string();
                if fired || msg.contains("execution terminated") {
                    Err("eval timed out".to_string())
                } else {
                    Err(format!("JS error: {}", msg))
                }
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))] // used by suspend_js/resume_js lifecycle tests
    pub fn take_dom(&self) -> Option<DomTree> {
        self.state.borrow_mut().dom.take()
    }

    pub fn with_dom<R>(&self, f: impl FnOnce(&DomTree) -> R) -> Option<R> {
        let state = self.state.borrow();
        state.dom.as_ref().map(f)
    }

    /// Read access to the shared JS state (scroll offset mirror, image byte
    /// table, viewport) for the band-paint frame paths. The closure must not
    /// re-enter the runtime (same contract as `with_dom`).
    #[cfg(feature = "screenshot")]
    pub fn with_state<R>(&self, f: impl FnOnce(&JsState) -> R) -> R {
        let state = self.state.borrow();
        f(&state)
    }

    /// Mutating variant of [`with_state`]. Field writes that affect layout
    /// (e.g. the image byte table) must also drop `layout_cache` —
    /// `ops::store_image_bytes` already does.
    #[cfg(feature = "screenshot")]
    pub fn with_state_mut<R>(&self, f: impl FnOnce(&mut JsState) -> R) -> R {
        let mut state = self.state.borrow_mut();
        f(&mut state)
    }

    fn make_oid(&self, counter: u64) -> String {
        format!("{{\"injectedScriptId\":1,\"id\":{}}}", counter)
    }

    #[cfg_attr(not(test), allow(dead_code))] // helper of call_function_on (test-exercised)
    fn resolve_this(&self, object_id: Option<&str>) -> String {
        match object_id {
            Some(oid) => {
                if let Some(retrieval) = self.object_store.get(oid) {
                    retrieval.clone()
                } else if let Some(nid) = oid
                    .strip_prefix("node-")
                    .and_then(|s| s.parse::<u64>().ok())
                {
                    format!(
                        "(function() {{ \
                            var nid = {nid}; \
                            var cache = globalThis._cache || new Map(); \
                            if (cache.has(nid)) return cache.get(nid); \
                            return null; \
                        }})()"
                    )
                } else {
                    "globalThis".to_string()
                }
            }
            None => "globalThis".to_string(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))] // helper of call_function_on (test-exercised)
    fn build_args(&self, arguments: &[serde_json::Value]) -> (String, String) {
        let mut setup_lines = vec!["globalThis._namedBoot&&globalThis._namedBoot();".to_string()];
        let mut arg_names = Vec::new();

        for (i, arg) in arguments.iter().enumerate() {
            let arg_name = format!("__arg{}", i);
            if let Some(value) = arg.get("value") {
                let json_str =
                    serde_json::to_string(value).unwrap_or_else(|_| "undefined".to_string());
                setup_lines.push(format!("var {} = {};", arg_name, json_str));
            } else if let Some(oid) = arg.get("objectId").and_then(|v| v.as_str()) {
                if let Some(retrieval) = self.object_store.get(oid) {
                    setup_lines.push(format!("var {} = {};", arg_name, retrieval));
                } else {
                    setup_lines.push(format!("var {} = undefined;", arg_name));
                }
            } else if let Some(unser) = arg.get("unserializableValue").and_then(|v| v.as_str()) {
                setup_lines.push(format!("var {} = {};", arg_name, unser));
            } else {
                setup_lines.push(format!("var {} = undefined;", arg_name));
            }
            arg_names.push(arg_name);
        }

        (setup_lines.join("\n"), arg_names.join(", "))
    }

    fn v8_to_json(&mut self, result: v8::Global<v8::Value>) -> Result<serde_json::Value, String> {
        // Our Vm's handle-scope face (with_scope) — the deno_core::scope!
        // rebuild of 0.411, now ours.
        self.runtime.with_scope(|scope| {
            let local = v8::Local::new(scope, result);

            if local.is_undefined() || local.is_null() {
                return Ok(serde_json::Value::Null);
            }
            if local.is_boolean() {
                return Ok(serde_json::Value::Bool(local.boolean_value(scope)));
            }
            if local.is_number() {
                let n = local.number_value(scope).unwrap_or(0.0);
                return Ok(serde_json::json!(n));
            }
            if local.is_string() {
                let s = local.to_rust_string_lossy(scope);
                return Ok(serde_json::Value::String(s));
            }

            let global = scope.get_current_context().global(scope);
            let json_obj_str = v8::String::new(scope, "JSON").unwrap();
            if let Some(json_obj) = global.get(scope, json_obj_str.into()) {
                if let Some(json_obj) = json_obj.to_object(scope) {
                    let stringify_str = v8::String::new(scope, "stringify").unwrap();
                    if let Some(stringify_fn) = json_obj.get(scope, stringify_str.into()) {
                        if let Ok(stringify_fn) = v8::Local::<v8::Function>::try_from(stringify_fn)
                        {
                            let args = [local];
                            if let Some(result) = stringify_fn.call(scope, json_obj.into(), &args) {
                                let json_str = result.to_rust_string_lossy(scope);
                                if let Ok(val) = serde_json::from_str(&json_str) {
                                    return Ok(val);
                                }
                            }
                        }
                    }
                }
            }

            let s = local.to_rust_string_lossy(scope);
            Ok(serde_json::Value::String(s))
        })
    }

    /// `v8_to_json` cannot tell a stored `undefined` from a missing value —
    /// both come back as JSON null — so the byValue read-backs ask the
    /// runtime for the stored value's typeof first (obscura#779).
    fn stored_value_is_undefined(&mut self, oid: &str) -> Result<bool, String> {
        let probe = self
            .runtime
            .execute_script(
                "<typeofResult>",
                &format!("typeof {} === 'undefined'", Self::object_slot(oid)),
            )
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(self.v8_to_json(probe)?.as_bool().unwrap_or(false))
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
