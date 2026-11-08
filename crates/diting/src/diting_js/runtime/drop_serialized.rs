// Guards no data, so every taker recovers from poison (#336): one panic
// during construction must not leave the process unable to build another
// isolate, and the Drop-side taker panicking mid-unwind would abort.
pub(super) static ISOLATE_CONSTRUCT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use diting_runtime::vm::Vm;

/// #108: the inner runtime wrapped so its ISOLATE DISPOSAL serializes with
/// isolate construction under `ISOLATE_CONSTRUCT_LOCK`. Construction was
/// already serialized (upstream obscura #430 lineage); disposal is the one
/// remaining seam where V8 15.0's process-wide JSDispatchTable sees alloc
/// (another isolate's snapshot deserialization, under the lock) race free
/// (this isolate's teardown, previously unlocked). The #108 failure shape —
/// `typeof Date.now` healthy inside a timer callback while the CALL throws
/// "not a function", dev-only, unreproducible on demand — is a call-path
/// (dispatch-entry) failure with a healthy object, exactly what a torn
/// table entry produces; the stress reproducers (tests: `date_now_timer_*`)
/// could not trigger it in ~10k construct/drop cycles, so this is
/// defense-in-depth closing the last uncovered cross-isolate touch point,
/// not a demonstrated fix.
///
/// `ManuallyDrop` is the std drop-exactly-once idiom: `Drop::drop` runs
/// with the lock held and destroys the inner runtime explicitly; the
/// compiler's own field drop is suppressed, so teardown happens exactly
/// once, inside the guard.
pub(super) struct SerializedDropRuntime {
    pub(super) vm: std::mem::ManuallyDrop<Vm>,
    /// #332: set by `JsRuntime::drop` when a watchdog cancel ceremony wedged
    /// — the wedged thread holds the isolate handle's rusty_v8 mutex inside
    /// `terminate_execution()`, and `IsolateHandle::dispose()` takes that
    /// same mutex. The normal teardown would pin the session thread again,
    /// one step after the bounded join already gave up. Skipping it leaks
    /// one realm's heap (plus its foreground-registry entry) per incident —
    /// loudly logged, bounded by incidents, and the process lives.
    leak_isolate: bool,
}

impl std::ops::Deref for SerializedDropRuntime {
    type Target = Vm;
    fn deref(&self) -> &Self::Target {
        &self.vm
    }
}

impl std::ops::DerefMut for SerializedDropRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vm
    }
}

impl SerializedDropRuntime {
    pub(super) fn new(vm: Vm) -> Self {
        Self {
            vm: std::mem::ManuallyDrop::new(vm),
            leak_isolate: false,
        }
    }

    /// #332: arm the leak-degrade path (see `leak_isolate`).
    pub(super) fn mark_isolate_wedged(&mut self) {
        self.leak_isolate = true;
    }
}

impl Drop for SerializedDropRuntime {
    fn drop(&mut self) {
        if self.leak_isolate {
            eprintln!(
                "[watchdog] #332: isolate teardown SKIPPED — its handle mutex is \
                 wedged inside a watchdog's terminate_execution; leaking one \
                 realm's heap"
            );
            // The ManuallyDrop field is never taken, so the Vm leaks instead
            // of blocking the session thread in dispose forever.
            return;
        }
        let _guard = ISOLATE_CONSTRUCT_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: `self` is being dropped and nothing touches the field
        // afterwards; `ManuallyDrop` suppressed the automatic drop, so this
        // read-then-drop runs the isolate teardown exactly once — here,
        // under the construction lock rather than after it.
        unsafe { std::mem::drop(std::ptr::read(&*self.vm)) }
    }
}
