//! Remote-object evaluation: `evaluate` and `callFunctionOn`.
//!
//! These stay methods on `JsRuntime`. This module runs the wrappers and
//! names the runtime types their result is built from; the scripts are
//! built in `eval_code`, the result shapes in `remote_object`.

use super::eval_code::not_function_flag;
use super::remote_object::page_frames;
use super::{JsRuntime, RemoteObjectInfo, StackFrame};

/// Budget for awaiting a script's promise before declaring an eval timeout.
/// The session eval face exposes this per-call (`timeout_ms`); callers that
/// do slow page-side work (uploads through the page's own fetch) pass a
/// larger budget instead of hitting a silent-null (0.4.1 taobao report).
pub const DEFAULT_AWAIT_BUDGET_MS: u64 = 5000;

/// V8's answer to an `unserializableValue` of a call argument that will not
/// run as a script.
pub const BAD_CALL_ARGUMENT: &str = "Couldn't parse value object in call argument";

/// Chrome's answer to a `callFunctionOn` declaration that is no function
/// (`InjectedScript::callFunction`): the declaration is read — it may throw,
/// and that is an exception — and what it comes to is not called.
pub const NOT_A_FUNCTION: &str = "Given expression does not evaluate to a function";

/// Serials of the realms this process has built, one per runtime. A runtime
/// counts its objects from 1, and a page builds a new runtime for each of
/// its documents, so a number on its own comes round again.
static REALM_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(super) fn take_realm_serial() -> u64 {
    REALM_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// What an object id names, for a caller that takes ids from a wire
/// protocol ([`JsRuntime::object_id_state`]). An id says which realm minted
/// it — the field Chrome calls `injectedScriptId` — and the number it was
/// there, so an id of a document gone never names an object of the one
/// after it; and a protocol can tell a client which of three things is
/// wrong with a dead handle, as Chrome does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectIdState {
    /// Minted in this realm, and not released.
    Held,
    /// Not this engine's spelling of an id.
    Unparseable,
    /// This engine's spelling, of a realm that is gone: the page built a
    /// new runtime under it (a navigation).
    Gone,
    /// This realm's spelling, and nothing behind it: released, or a number
    /// this realm has not minted.
    Released,
}

/// Outcome of [`JsRuntime::evaluate_remote_outcome`] / callFunctionOn: the
/// remote object plus any thrown (or rejected) exception. Captured separately so callers can
/// emit `Runtime.exceptionThrown` + `exceptionDetails` instead of collapsing a
/// throw into a plain `undefined`/`null` result.
#[derive(Debug, Clone)]
pub struct EvalOutcome {
    /// What came back — the thrown value too, when something was thrown,
    /// shown as a result is.
    pub info: RemoteObjectInfo,
    pub exception: Option<ExceptionInfo>,
}

/// How the text of an evaluate is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalInput {
    /// As Chrome takes it: a script, and nothing forgiven of it — a bare
    /// `{…}` is a block or a labeled statement, a top-level `return` is a
    /// syntax error, and positions count from its first character, white
    /// space included.
    Script,
    /// As the HTTP and flow faces take it: a bare `{…}` is an object
    /// literal, and a script with a top-level `return` is run as a function
    /// body.
    Forgiving,
}

/// What was thrown, besides the value itself — that is
/// [`EvalOutcome::info`].
#[derive(Debug, Clone)]
pub struct ExceptionInfo {
    /// Whether a promise was rejected with the value, rather than a script
    /// throwing it.
    pub in_promise: bool,
    /// The message line of the value, `Error: boom`; nothing when it has
    /// none. The frames of an Error's stack are in the value's description.
    pub description: String,
    /// The page's own frames of the Error's stack, innermost first.
    pub frames: Vec<StackFrame>,
    /// Whether the value is an `Error` — as Chrome counts one for a
    /// message: a `DOMException` is not.
    pub is_error: bool,
}

impl ExceptionInfo {
    /// "Uncaught", or "Uncaught (in promise)" for a rejection.
    pub fn text(&self) -> &'static str {
        if self.in_promise {
            "Uncaught (in promise)"
        } else {
            "Uncaught"
        }
    }

    /// The frame the error was made in, when a page's was on the stack: the
    /// innermost with a position — a builtin has none.
    pub fn own_frame(&self) -> Option<&StackFrame> {
        self.frames.iter().find(|frame| frame.line > 0)
    }
}

impl JsRuntime {
    pub async fn evaluate_remote(
        &mut self,
        expression: &str,
        return_by_value: bool,
        await_promise: bool,
    ) -> Result<RemoteObjectInfo, String> {
        match self
            .evaluate_remote_outcome(
                expression,
                return_by_value,
                await_promise,
                DEFAULT_AWAIT_BUDGET_MS,
                None,
                EvalInput::Forgiving,
            )
            .await?
        {
            EvalOutcome {
                info,
                exception: None,
            } => Ok(info),
            EvalOutcome {
                exception: Some(exc),
                ..
            } => {
                if await_promise {
                    Err(format!("Promise rejected: {}", exc.description))
                } else {
                    // Pre-exceptionThrown behavior: a sync throw was swallowed
                    // to `undefined` so callers couldn't distinguish it from a
                    // genuine `undefined` return.
                    Ok(Self::undefined_info())
                }
            }
        }
    }

    /// Like [`evaluate_remote`], but a thrown/rejected expression comes back
    /// as an `EvalOutcome` carrying the exception instead of being collapsed
    /// into `Err("Promise rejected: …")` (await) or `undefined` (sync).
    ///
    /// `Some(frame_nid)` scopes the eval to that iframe: the single-realm
    /// engine shares one isolate across frames, so the main realm's global
    /// bindings (document/window/self/frames/location) are swapped for the
    /// iframe's content window for the duration of the call and restored on
    /// every return path. Timers firing mid-await observe the frame's
    /// globals (accepted v1 edge); globalThis/navigator/screen stay main.
    ///
    /// [`evaluate_remote`](Self::evaluate_remote) is Forgiving; a CDP
    /// `Runtime.evaluate` is [`EvalInput::Script`].
    pub async fn evaluate_remote_outcome(
        &mut self,
        expression: &str,
        return_by_value: bool,
        await_promise: bool,
        await_budget_ms: u64,
        frame_nid: Option<u32>,
        input: EvalInput,
    ) -> Result<EvalOutcome, String> {
        let swapped = match frame_nid {
            None => false,
            Some(nid) => {
                if !self.frame_swap(nid)? {
                    return Err(format!(
                        "frame eval: iframe nid={} content not ready (detached or cross-origin)",
                        nid
                    ));
                }
                true
            }
        };
        let result = self
            .evaluate_remote_outcome_inner(
                expression,
                return_by_value,
                await_promise,
                await_budget_ms,
                input,
            )
            .await;
        if swapped {
            self.frame_restore();
        }
        result
    }

    /// Swap the main realm's global bindings for iframe `nid`'s content
    /// window, returning false when the iframe has no reachable document
    /// (nothing swapped in that case). `document` is an accessor whose
    /// setter switches the active document instance; self/window/frames are
    /// plain data props — assignment is the swap and restore mechanism for
    /// those four. `location` is a non-configurable accessor whose setter
    /// navigates, so it swaps through the `__diting_frame_location` slot its
    /// getter honors (bootstrap.js). The iframe's doc/window are read from
    /// the internal `_iframeDoc`/`_iframeWin` slots, not the public
    /// `contentDocument`/`contentWindow` getters: those apply the web-facing
    /// same-origin gate, while CDP clients are privileged and automate
    /// cross-origin frames as a matter of course (Chrome does the same from
    /// the inspector side). A detached iframe is refused: Chrome discards
    /// the browsing context on removal, so its execution context is gone —
    /// running script (let alone XHR, which the spec gates on a
    /// fully-active document) from a retired frame is not a face (#256 /
    /// moli#1328).
    fn frame_swap(&mut self, nid: u32) -> Result<bool, String> {
        // document/window/self/frames live on the window-table accessors
        // (interface_sweep.js fp-diff-1015): document/window are getter-only
        // non-configurable, so the bare assignments below are silent no-ops
        // for them — every binding that the __ditingWinSlot bag backs is
        // swapped through the slot instead.
        let code = format!(
            "(function() {{ var el = globalThis._wrap && globalThis._wrap({nid}); if (!el || !el.isConnected) return 'not-ready'; var doc = el._iframeDoc || el.contentDocument; var win = el._iframeWin || el.contentWindow; if (!doc || !win) return 'not-ready'; var sl = globalThis.__ditingWinSlot; globalThis.__diting_frame_saved = [document, window, self, frames, sl ? {{d: sl.document, w: sl.window, s: sl.self, f: sl.frames}} : null]; if (sl) {{ sl.document = doc; sl.window = win; sl.self = win; sl.frames = win; }} document = doc; window = win; self = win; frames = win; Object.defineProperty(globalThis, '__diting_frame_location', {{ value: win.location, configurable: true, writable: true, enumerable: false }}); return 'ok'; }})()",
        );
        let v = self
            .runtime
            .execute_script("<frame-swap>", &code)
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(self.v8_to_json(v)?.as_str() == Some("ok"))
    }

    /// Restore the globals stashed by [`frame_swap`]. A restore failure is
    /// logged rather than propagated: the eval result matters more, and the
    /// next navigation's fresh realm clears a stale swap anyway.
    fn frame_restore(&mut self) {
        const RESTORE: &str = "(function() { var s = globalThis.__diting_frame_saved; if (!s) return; var sl = globalThis.__ditingWinSlot; if (s[4] && sl) { sl.document = s[4].d; sl.window = s[4].w; sl.self = s[4].s; sl.frames = s[4].f; } document = s[0]; window = s[1]; self = s[2]; frames = s[3]; delete globalThis.__diting_frame_saved; delete globalThis.__diting_frame_location; })()";
        if let Err(e) = self.runtime.execute_script("<frame-restore>", RESTORE) {
            tracing::warn!("frame restore failed: {}", e);
        }
    }

    async fn evaluate_remote_outcome_inner(
        &mut self,
        expression: &str,
        return_by_value: bool,
        await_promise: bool,
        await_budget_ms: u64,
        input: EvalInput,
    ) -> Result<EvalOutcome, String> {
        self.object_counter += 1;
        let oid = self.make_oid(self.object_counter);
        let done_counter = self.object_counter;
        // Wrapper semantics (script-not-expression, sentinel protocol, the
        // #114 sync-first await shape) live in eval_code::eval_meta_code.
        let meta_code = Self::eval_meta_code(expression, &oid, done_counter, await_promise, input);

        // Watchdog-bound: a runaway expression (`while(1){}`) pins the thread
        // inside V8 where the caller's tokio timeouts cannot reach. The floor
        // is 10s, but when the caller asked for a longer budget (#110: page JS
        // legitimately synchronous longer than 10s — e.g. a gBCR that forces
        // the 13s layout of #109) the script must not be beheaded at 10s while
        // its settle half would have waited. A true spin is still terminated:
        // at the caller's own budget, which they chose.
        //
        // NOT execute_script_retry_terminated: that helper clears the flag and
        // retries once, which would just re-enter the spin - the watchdog
        // becomes useless. Pre-clear any stray flag from an earlier watchdog
        // instead, then run the plain one-shot execute.
        self.runtime.revive();
        let eval_wd = self.arm_watchdog(Self::eval_watchdog_duration(await_budget_ms));
        let result = self.runtime.execute_script("<eval-remote>", &meta_code);
        let eval_fired = self.disarm_watchdog(eval_wd);
        if eval_fired {
            let secs = Self::eval_watchdog_duration(await_budget_ms).as_secs();
            let preview: String = expression.chars().take(80).collect();
            tracing::warn!(
                "eval terminated by watchdog (ran >{}s): '{}'",
                secs,
                preview
            );
            return Err(format!("eval timed out: script ran longer than {}s", secs));
        }
        result.map_err(|e| format!("JS error: {}", e))?;

        if await_promise {
            let __t0 = std::time::Instant::now();
            // An unset sentinel means the budget expired with the promise
            // still pending: the result slot was never assigned, so falling
            // through would read `undefined` and surface as a silent
            // `null` — indistinguishable from a genuine null return (the
            // 0.4.1 taobao report's intermittent `result: null` mid
            // batch-upload). Tell the truth instead: the script may still
            // be running, callers must verify side effects before retry.
            if !self.settled(done_counter, await_budget_ms).await {
                let preview: String = expression.chars().take(80).collect();
                tracing::warn!(
                    "eval await exceeded {}ms budget (still pending): '{}'",
                    await_budget_ms,
                    preview
                );
                return Err(format!(
                    "EVAL_TIMEOUT: expression did not settle within {}ms — the script may still be running; verify side effects before retry",
                    await_budget_ms
                ));
            }
            let __dt = __t0.elapsed();
            if __dt > std::time::Duration::from_secs(1) {
                let preview: String = expression
                    .chars()
                    .take(200)
                    .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
                    .collect();
                tracing::debug!(
                    "Runtime.evaluate awaitPromise took {}ms; expr={}",
                    __dt.as_millis(),
                    preview,
                );
            }
        }

        if self.wrapper_rejected()? {
            return self.exception_outcome(&oid);
        }
        let info = if return_by_value {
            self.stored_json_info(&oid)?
        } else {
            self.stored_meta_info(&oid)?
        };
        self.hold(&oid, &info)?;
        Ok(EvalOutcome {
            info,
            exception: None,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))] // production uses evaluate_remote*; tests exercise callFunctionOn
    pub async fn call_function_on_remote(
        &mut self,
        function_declaration: &str,
        object_id: Option<&str>,
        arguments: &[serde_json::Value],
        return_by_value: bool,
        await_promise: bool,
    ) -> Result<RemoteObjectInfo, String> {
        match self
            .call_function_on_remote_outcome(
                function_declaration,
                object_id,
                arguments,
                return_by_value,
                await_promise,
                None,
            )
            .await?
        {
            EvalOutcome {
                info,
                exception: None,
            } => Ok(info),
            EvalOutcome {
                exception: Some(exc),
                ..
            } => {
                if await_promise {
                    Err(format!("Promise rejected: {}", exc.description))
                } else {
                    Ok(Self::undefined_info())
                }
            }
        }
    }

    /// Whether the wrapper's declaration came to no function
    /// ([`call_fn_meta_code`](Self::call_fn_meta_code)): the flag it leaves,
    /// read once.
    fn declaration_is_not_function(&mut self, done_counter: u64) -> Result<bool, String> {
        let flag = not_function_flag(done_counter);
        let read = self
            .runtime
            .execute_script(
                "<readNotFunction>",
                &format!(
                    "(function() {{ var f = globalThis.{flag}; delete globalThis.{flag}; return f === true; }})()"
                ),
            )
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(self.v8_to_json(read)?.as_bool().unwrap_or(false))
    }

    /// Whether the wrapper's done sentinel turned true within the budget,
    /// giving the event loop the turns to let it.
    async fn settled(&mut self, done_counter: u64, await_budget_ms: u64) -> bool {
        let sentinel = format!("globalThis.__diting_done_{done_counter} === true");
        self.resolve_promises_until(
            |rt| {
                rt.runtime
                    .execute_script("<done?>", &sentinel)
                    .ok()
                    .and_then(|v| rt.v8_to_json(v).ok())
                    .and_then(|j| j.as_bool())
                    .unwrap_or(false)
            },
            await_budget_ms,
        )
        .await
    }

    /// Whether the wrapper threw what it ran — the flag its `catch` sets.
    fn wrapper_rejected(&mut self) -> Result<bool, String> {
        let read = self
            .runtime
            .execute_script("<readRejected>", "globalThis.__diting_await_rejected")
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(self.v8_to_json(read)?.as_bool().unwrap_or(false))
    }

    /// The JSON a wrapper left in `globalThis.__diting_await_meta`: what it
    /// stored, or what its `catch` caught.
    fn read_await_meta(&mut self) -> Result<serde_json::Value, String> {
        let read = self
            .runtime
            .execute_script("<readMeta>", "globalThis.__diting_await_meta")
            .map_err(|e| format!("JS error: {}", e))?;
        Ok(match self.v8_to_json(read)? {
            serde_json::Value::String(text) => {
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text))
            }
            json => json,
        })
    }

    /// What the wrapper stored, as its meta says it is.
    fn stored_meta_info(&mut self, oid: &str) -> Result<RemoteObjectInfo, String> {
        let meta = self.read_await_meta()?;
        Ok(Self::info_from_meta(&meta, Some(oid.to_string())))
    }

    /// The value of what the wrapper stored, through JSON — the by-value of
    /// the HTTP and session faces. (The CDP bridge takes a handle first and
    /// reads its value with [`value_of`](Self::value_of), which is Chrome's
    /// walk, not JSON's.)
    fn stored_json_info(&mut self, oid: &str) -> Result<RemoteObjectInfo, String> {
        if self.stored_value_is_undefined(oid)? {
            return Ok(Self::undefined_info());
        }
        let read = self
            .runtime
            .execute_script("<readResult>", &Self::object_slot(oid))
            .map_err(|e| format!("JS error: {}", e))?;
        let json = self.v8_to_json(read)?;
        Ok(Self::info_from_json(&json))
    }

    /// Keep what the wrapper stored in the object store when it is something
    /// a client can call into — an object, a function, a symbol — and let
    /// go of its slot when it is not (a number, a string, `undefined`): that
    /// slot would otherwise stay for the page's lifetime, one per eval.
    fn hold(&mut self, oid: &str, info: &RemoteObjectInfo) -> Result<(), String> {
        match info.object_id.as_deref() {
            Some(id) => {
                self.object_store
                    .insert(id.to_string(), Self::object_slot(id));
                Ok(())
            }
            None => self
                .runtime
                .execute_script("<release>", &format!("delete {};", Self::object_slot(oid)))
                .map(|_| ())
                .map_err(|e| format!("JS error: {}", e)),
        }
    }

    /// What the wrapper's `catch` caught — the JSON of
    /// [`exception_meta_extract_js`](Self::exception_meta_extract_js) — as
    /// an `EvalOutcome` carrying it.
    fn exception_outcome(&mut self, oid: &str) -> Result<EvalOutcome, String> {
        let meta = self.read_await_meta()?;
        let info = Self::info_from_meta(
            meta.get("meta").unwrap_or(&serde_json::Value::Null),
            Some(oid.to_string()),
        );
        self.hold(oid, &info)?;
        let field = |name: &str| meta.get(name);
        let in_promise = field("inPromise")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let exception = ExceptionInfo {
            in_promise,
            description: field("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            frames: page_frames(field("stack").and_then(|v| v.as_str()).unwrap_or("")),
            is_error: field("error").and_then(|v| v.as_bool()).unwrap_or(false),
        };
        Ok(EvalOutcome {
            info,
            exception: Some(exception),
        })
    }

    /// Like [`call_function_on_remote`], but a throwing/rejecting function is
    /// reported as an `EvalOutcome` exception instead of being collapsed into
    /// `Err("Promise rejected: …")` or a bare `undefined`.
    /// `frame_nid` scopes the call to an iframe via the same global-binding
    /// swap as [`evaluate_remote_outcome`].
    pub async fn call_function_on_remote_outcome(
        &mut self,
        function_declaration: &str,
        object_id: Option<&str>,
        arguments: &[serde_json::Value],
        return_by_value: bool,
        await_promise: bool,
        frame_nid: Option<u32>,
    ) -> Result<EvalOutcome, String> {
        let swapped = match frame_nid {
            None => false,
            Some(nid) => {
                if !self.frame_swap(nid)? {
                    return Err(format!(
                        "frame callFunctionOn: iframe nid={} content not ready (detached or cross-origin)",
                        nid
                    ));
                }
                true
            }
        };
        let result = self
            .call_function_on_remote_outcome_inner(
                function_declaration,
                object_id,
                arguments,
                return_by_value,
                await_promise,
            )
            .await;
        if swapped {
            self.frame_restore();
        }
        result
    }

    async fn call_function_on_remote_outcome_inner(
        &mut self,
        function_declaration: &str,
        object_id: Option<&str>,
        arguments: &[serde_json::Value],
        return_by_value: bool,
        await_promise: bool,
    ) -> Result<EvalOutcome, String> {
        let this_expr = self.resolve_this(object_id);
        let (setup, args_list) = self.build_args(arguments)?;

        self.object_counter += 1;
        let oid = self.make_oid(self.object_counter);
        let done_counter = self.object_counter;
        let code = Self::call_fn_meta_code(
            &setup,
            function_declaration,
            &this_expr,
            &args_list,
            &oid,
            done_counter,
            await_promise,
        );
        let name = if await_promise {
            "<callFnAsync>"
        } else {
            "<callFnRemote>"
        };
        // Watchdog-bound like an evaluate: a declaration that spins
        // (`function () { while (1) {} }`) pins the page's thread inside V8,
        // where no caller's timeout reaches — and the call is never
        // answered. The same wall and the same words as an evaluate's.
        self.runtime.revive();
        let call_wd = self.arm_watchdog(Self::eval_watchdog_duration(DEFAULT_AWAIT_BUDGET_MS));
        let result = self.runtime.execute_script(name, &code);
        let call_fired = self.disarm_watchdog(call_wd);
        if call_fired {
            let secs = Self::eval_watchdog_duration(DEFAULT_AWAIT_BUDGET_MS).as_secs();
            let preview: String = function_declaration.chars().take(80).collect();
            tracing::warn!(
                "callFunctionOn terminated by watchdog (ran >{}s): '{}'",
                secs,
                preview
            );
            return Err(format!("eval timed out: script ran longer than {}s", secs));
        }
        result.map_err(|e| format!("JS error: {}", e))?;
        // The declaration came to no function and was not called: nothing
        // will settle, so it is asked of before anything waits.
        if self.declaration_is_not_function(done_counter)? {
            return Err(NOT_A_FUNCTION.to_string());
        }

        if await_promise {
            // Same contract as evaluate_remote_outcome: an unsettled budget
            // must not fall through to an empty result slot (silent null).
            if !self.settled(done_counter, DEFAULT_AWAIT_BUDGET_MS).await {
                return Err(format!(
                    "EVAL_TIMEOUT: callFunctionOn did not settle within {}ms — the function may still be running; verify side effects before retry",
                    DEFAULT_AWAIT_BUDGET_MS
                ));
            }
        }

        if self.wrapper_rejected()? {
            return self.exception_outcome(&oid);
        }
        let info = if return_by_value {
            self.stored_json_info(&oid)?
        } else {
            self.stored_meta_info(&oid)?
        };
        self.hold(&oid, &info)?;
        Ok(EvalOutcome {
            info,
            exception: None,
        })
    }
    #[cfg_attr(not(test), allow(dead_code))] // exercised via tests; CDP consumer pending
    pub async fn call_function_on(
        &mut self,
        function_declaration: &str,
        object_id: Option<&str>,
        arguments: &[serde_json::Value],
        return_by_value: bool,
    ) -> Result<RemoteObjectInfo, String> {
        self.call_function_on_remote(
            function_declaration,
            object_id,
            arguments,
            return_by_value,
            false,
        )
        .await
    }
    #[cfg_attr(not(test), allow(dead_code))] // tests: release_object_with_adversarial_id_is_inert
    pub fn release_object(&mut self, object_id: &str) {
        let object_id = Self::id_key(object_id);
        if self.object_store.remove(object_id).is_some() {
            let code = format!("delete {};", Self::object_slot(object_id));
            let _ = self.runtime.execute_script("<release>", &code);
        }
    }

    /// An id as the engine spells it. Chrome's number reader steps over the
    /// blanks before an id and stops at the first one after it: a blank in
    /// front of one is not part of it, and one behind it is no id.
    fn id_key(object_id: &str) -> &str {
        object_id.trim_start()
    }

    /// The value of a held object the way `returnByValue` brings one back
    /// in Chrome ([`Self::by_value_js`]) — for a caller that took the handle
    /// first and asks for the value after. What keeps a value from coming
    /// back is an error in Chrome's words for the two it names, and
    /// "Internal error" for a getter that threw.
    pub fn value_of(&mut self, object_id: &str) -> Result<serde_json::Value, String> {
        let object_id = Self::id_key(object_id);
        let code = Self::by_value_js(&Self::object_slot(object_id));
        let read = self
            .runtime
            .execute_script("<readValue>", &code)
            .map_err(|e| format!("JS error: {}", e))?;
        let text = match self.v8_to_json(read)? {
            serde_json::Value::String(text) => text,
            _ => return Err(format!("object {object_id}: no value came back")),
        };
        match text.as_str() {
            "c" => Err("Object reference chain is too long".into()),
            "p" => Err("Object couldn't be returned by value".into()),
            "x" => Err("Internal error".into()),
            "n" => Err(format!("object {object_id} is not held")),
            json => match json.strip_prefix('v') {
                Some(json) => serde_json::from_str(json)
                    .map_err(|e| format!("object {object_id}: value is not JSON: {e}")),
                None => Err(format!("object {object_id}: no value came back")),
            },
        }
    }

    /// Whether `object_id` still names an object:
    /// [`ObjectIdState::Held`]. [`resolve_this`](Self::resolve_this) does
    /// not answer that — it runs an id it cannot find against the global
    /// object — so a caller that takes ids from a wire protocol asks here
    /// first. Ids of the `node-<nid>` spelling are not held.
    pub fn has_object(&mut self, object_id: &str) -> bool {
        self.object_id_state(object_id) == ObjectIdState::Held
    }

    /// Which of the four an id is ([`ObjectIdState`]). An id is the exact
    /// spelling the engine wrote, a blank in front of it aside
    /// ([`Self::id_key`]). A realm of this runtime's is read before its
    /// number, as Chrome reads one: an id of a realm that is gone is `Gone`
    /// whatever its number says. A part that is no integer makes the id no
    /// id — `"id": "x"`, `1.5`, `null` — the way Chrome's number reader
    /// takes it, with the one thing it takes for a number besides a number:
    /// a part left empty reads as 0, so a realm left empty is the realm
    /// there is none of, and a number left empty the 0 no realm minted.
    pub fn object_id_state(&mut self, object_id: &str) -> ObjectIdState {
        let spelled = Self::id_key(object_id);
        // `Err(())`: not an id. The number may be `None`: it was left empty.
        let parsed = serde_json::from_str::<serde_json::Value>(spelled)
            .map_err(|_| ())
            .and_then(|value| {
                let part = |field: &str| match value.get(field) {
                    Some(serde_json::Value::String(empty)) if empty.is_empty() => Ok(None),
                    Some(other) => other.as_i64().map(Some).ok_or(()),
                    None => Err(()),
                };
                Ok((part("injectedScriptId")?, part("id")?))
            });
        match parsed {
            Ok(parts) => self.parts_state(spelled, parts, true),
            // No JSON at all: Chrome's number reader still takes each part
            // by itself — `"id":""2` reads an empty id, not no id.
            Err(()) => Self::lenient_parts(spelled)
                .map(|parts| self.parts_state(spelled, parts, false))
                .unwrap_or(ObjectIdState::Unparseable),
        }
    }

    /// What the parts of an id make it, once read — strictly, where the
    /// spelling has to be the one the engine wrote, or leniently, where
    /// only the parts name it.
    fn parts_state(
        &mut self,
        spelled: &str,
        (realm, number): (Option<i64>, Option<i64>),
        strict: bool,
    ) -> ObjectIdState {
        if realm.is_none() {
            return ObjectIdState::Gone;
        }
        if realm != Some(self.realm as i64) {
            return ObjectIdState::Gone;
        }
        let Some(number) = number else {
            return ObjectIdState::Released;
        };
        if strict && spelled != Self::oid(realm.unwrap(), number) {
            return ObjectIdState::Unparseable;
        }
        if self.held_in_page(&Self::oid(realm.unwrap(), number)) {
            ObjectIdState::Held
        } else {
            ObjectIdState::Released
        }
    }

    /// The two parts of an id that is no JSON, read the way Chrome's number
    /// reader takes them: each part on its own, a part that is no integer
    /// making the whole no id, one left empty reading as none. `None` when
    /// a part cannot be found at all.
    fn lenient_parts(spelled: &str) -> Option<(Option<i64>, Option<i64>)> {
        // What follows the closing brace is no id at all.
        if !spelled.trim_end().ends_with('}') {
            return None;
        }
        fn part(text: &str) -> Option<Result<Option<i64>, ()>> {
            let text = text.trim_start().strip_prefix(':')?.trim_start();
            if let Some(inner) = text.strip_prefix('"') {
                let end = inner.find('"')?;
                return Some(if inner[..end].is_empty() {
                    Ok(None)
                } else {
                    Err(())
                });
            }
            // Chrome's number reader: a sign and digits — leading blanks
            // and zeroes aside — and the field's own end right after them.
            let bytes = text.as_bytes();
            let mut at = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
            let digits = at;
            while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                at += 1;
            }
            if at == digits || !matches!(bytes.get(at), None | Some(b',') | Some(b'}')) {
                return Some(Err(()));
            }
            Some(text[..at].parse::<i64>().map(Some).map_err(|_| ()))
        }
        let realm_at = spelled.find("\"injectedScriptId\"")?;
        let realm = part(&spelled[realm_at + "\"injectedScriptId\"".len()..])?.ok()?;
        let id_at = spelled[realm_at..].find("\"id\"")? + realm_at;
        let number = part(&spelled[id_at + "\"id\"".len()..])?.ok()?;
        Some((realm, number))
    }

    /// The store has the id, and the page's own table still has its object.
    fn held_in_page(&mut self, object_id: &str) -> bool {
        if !self.object_store.contains_key(object_id) {
            return false;
        }
        let lit = serde_json::to_string(object_id).unwrap_or_else(|_| "\"\"".to_string());
        let held = self.runtime.execute_script(
            "<held?>",
            &format!("({lit} in (globalThis.__diting_objects || {{}}))"),
        );
        held.ok()
            .and_then(|value| self.v8_to_json(value).ok())
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    /// An id says its realm and its number: `{"injectedScriptId":<realm>,
    /// "id":<n>}`, and is spelled no other way.
    fn make_oid(&self, counter: u64) -> String {
        Self::oid(self.realm as i64, counter as i64)
    }

    fn oid(realm: i64, counter: i64) -> String {
        format!("{{\"injectedScriptId\":{realm},\"id\":{counter}}}")
    }

    pub(super) fn resolve_this(&self, object_id: Option<&str>) -> String {
        match object_id.map(Self::id_key) {
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

    /// The call's arguments: the lines that bind them and the list the call
    /// takes. One key of an argument says what it is, as V8 reads one
    /// (`InjectedScript::resolveCallArgument`): its `objectId` if it has one,
    /// else its `value` — so a `value` or an `objectId` beside an
    /// `unserializableValue` is what goes in, and that script does not run
    /// at all. A `value` is its JSON, an `objectId` is its slot — `undefined`
    /// when the store does not have it, which is why a caller that takes ids
    /// from a wire protocol asks [`object_id_state`](Self::object_id_state)
    /// first. An `unserializableValue` is a script of its own, run here
    /// before the call the way V8 runs it: a text that will not run is
    /// Chrome's [word for it](BAD_CALL_ARGUMENT), not a page error.
    /// Its value waits in a slot of its own, which the binding line empties.
    fn build_args(&mut self, arguments: &[serde_json::Value]) -> Result<(String, String), String> {
        let mut setup_lines = Vec::new();
        let mut arg_names = Vec::new();

        for (i, arg) in arguments.iter().enumerate() {
            let arg_name = format!("__arg{}", i);
            if let Some(oid) = arg.get("objectId").and_then(|v| v.as_str()) {
                if let Some(retrieval) = self.object_store.get(Self::id_key(oid)) {
                    setup_lines.push(format!("var {} = {};", arg_name, retrieval));
                } else {
                    setup_lines.push(format!("var {} = undefined;", arg_name));
                }
            } else if let Some(value) = arg.get("value") {
                let json_str =
                    serde_json::to_string(value).unwrap_or_else(|_| "undefined".to_string());
                setup_lines.push(format!("var {} = {};", arg_name, json_str));
            } else if let Some(text) = arg.get("unserializableValue").and_then(|v| v.as_str()) {
                self.object_counter += 1;
                let slot = Self::object_slot(&self.make_oid(self.object_counter));
                self.runtime
                    .execute_script("<callArgument>", &Self::call_argument_code(text, &slot))
                    .map_err(|_| BAD_CALL_ARGUMENT.to_string())?;
                setup_lines.push(format!(
                    "var {arg_name} = (function() {{ var value = {slot}; delete {slot}; return value; }})();"
                ));
            } else {
                setup_lines.push(format!("var {} = undefined;", arg_name));
            }
            arg_names.push(arg_name);
        }

        Ok((setup_lines.join("\n"), arg_names.join(", ")))
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
