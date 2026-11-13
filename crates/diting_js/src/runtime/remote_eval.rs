//! Remote-object evaluation: `evaluate` and `callFunctionOn`.
//!
//! Split from the runtime hub. These stay methods on `JsRuntime`. Pure move.

use super::*;

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
                    Ok(RemoteObjectInfo {
                        js_type: "undefined".into(),
                        subtype: None,
                        class_name: String::new(),
                        description: String::new(),
                        object_id: None,
                        value: None,
                    })
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
    pub async fn evaluate_remote_outcome(
        &mut self,
        expression: &str,
        return_by_value: bool,
        await_promise: bool,
        await_budget_ms: u64,
        frame_nid: Option<u32>,
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
    ) -> Result<EvalOutcome, String> {
        self.object_counter += 1;
        let oid = self.make_oid(self.object_counter);
        let done_counter = self.object_counter;
        // Wrapper semantics (script-not-expression, sentinel protocol, the
        // #114 sync-first await shape) live in eval_code::eval_meta_code.
        let meta_code = Self::eval_meta_code(expression, &oid, done_counter, await_promise);

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
            let sentinel = format!("globalThis.__diting_done_{done_counter} === true");
            let settled = self
                .resolve_promises_until(
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
                .await;
            // An unset sentinel means the budget expired with the promise
            // still pending: the result slot was never assigned, so falling
            // through would read `undefined` and surface as a silent
            // `null` — indistinguishable from a genuine null return (the
            // 0.4.1 taobao report's intermittent `result: null` mid
            // batch-upload). Tell the truth instead: the script may still
            // be running, callers must verify side effects before retry.
            if !settled {
                let preview: String = expression.chars().take(80).collect();
                tracing::warn!(
                    "eval await exceeded {}ms budget (still pending): '{}'",
                    await_budget_ms,
                    preview
                );
                return Err(format!(
                "EVAL_TIMEOUT: expression did not settle within {}ms — the script may still be running; verify side effects before retrying",
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

        let rejected = self
            .runtime
            .execute_script("<readRejected>", "globalThis.__diting_await_rejected")
            .map_err(|e| format!("JS error: {}", e))?;
        let rejected = self.v8_to_json(rejected)?.as_bool().unwrap_or(false);

        if rejected {
            return self.exception_outcome(&oid, await_promise);
        }

        let info = if return_by_value {
            if self.stored_value_is_undefined(&oid)? {
                Self::undefined_info()
            } else {
                let read = self
                    .runtime
                    .execute_script("<readResult>", &Self::object_slot(&oid))
                    .map_err(|e| format!("JS error: {}", e))?;
                let json_val = self.v8_to_json(read)?;
                Self::info_from_json(&json_val)
            }
        } else {
            let meta = self
                .runtime
                .execute_script("<readMeta>", "globalThis.__diting_await_meta")
                .map_err(|e| format!("JS error: {}", e))?;
            let meta_str = self.v8_to_json(meta)?;
            let meta_json = if let serde_json::Value::String(s) = &meta_str {
                serde_json::from_str(s).unwrap_or(meta_str)
            } else {
                meta_str
            };
            Self::info_from_meta(&meta_json, Some(oid.clone()))
        };
        self.object_store
            .insert(oid.clone(), Self::object_slot(&oid));
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
                    Ok(RemoteObjectInfo {
                        js_type: "undefined".into(),
                        subtype: None,
                        class_name: String::new(),
                        description: String::new(),
                        object_id: None,
                        value: None,
                    })
                }
            }
        }
    }

    /// Read the exception object + meta a throwing IIFE left in the globals
    /// and turn it into an `EvalOutcome` carrying the exception.
    fn exception_outcome(&mut self, oid: &str, await_promise: bool) -> Result<EvalOutcome, String> {
        let exc_meta = self
            .runtime
            .execute_script("<readExcMeta>", "globalThis.__diting_await_meta")
            .map_err(|e| format!("JS error: {}", e))?;
        let exc_meta = self.v8_to_json(exc_meta)?;
        let exc_meta = if let serde_json::Value::String(s) = &exc_meta {
            serde_json::from_str(s).unwrap_or(exc_meta)
        } else {
            exc_meta
        };
        let class_name = exc_meta
            .get("className")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = exc_meta
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let stack_first = exc_meta
            .get("stack_first")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let line = exc_meta
            .get("line")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let col = exc_meta
            .get("col")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        self.object_store
            .insert(oid.to_string(), Self::object_slot(oid));

        let text = if await_promise {
            "Uncaught (in promise)"
        } else {
            "Uncaught"
        }
        .to_string();
        let info = RemoteObjectInfo {
            js_type: "object".into(),
            subtype: Some("error".into()),
            class_name: class_name.clone(),
            description: description.clone(),
            object_id: Some(oid.to_string()),
            value: None,
        };
        Ok(EvalOutcome {
            info,
            exception: Some(ExceptionInfo {
                text,
                description,
                class_name,
                object_id: Some(oid.to_string()),
                stack_first,
                line,
                col,
            }),
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
        let (setup, args_list) = self.build_args(arguments);

        self.object_counter += 1;
        let oid = self.make_oid(self.object_counter);
        let exc_meta_fn = Self::exception_meta_extract_js("e");

        if await_promise {
            let done_counter = self.object_counter;
            let code = Self::call_fn_meta_code(
                &setup,
                function_declaration,
                &this_expr,
                &args_list,
                &oid,
                done_counter,
            );

            self.runtime
                .execute_script("<callFnAsync>", &code)
                .map_err(|e| format!("JS error: {}", e))?;

            let sentinel = format!("globalThis.__diting_done_{done_counter} === true");
            let settled = self
                .resolve_promises_until(
                    |rt| {
                        rt.runtime
                            .execute_script("<done?>", &sentinel)
                            .ok()
                            .and_then(|v| rt.v8_to_json(v).ok())
                            .and_then(|j| j.as_bool())
                            .unwrap_or(false)
                    },
                    DEFAULT_AWAIT_BUDGET_MS,
                )
                .await;
            // Same contract as evaluate_remote_outcome: an unsettled budget
            // must not fall through to an empty result slot (silent null).
            if !settled {
                return Err(format!(
                "EVAL_TIMEOUT: callFunctionOn did not settle within {}ms — the function may still be running; verify side effects before retrying",
                DEFAULT_AWAIT_BUDGET_MS
            ));
            }

            let rejected = self
                .runtime
                .execute_script("<readRejected>", "globalThis.__diting_await_rejected")
                .map_err(|e| format!("JS error: {}", e))?;
            if self.v8_to_json(rejected)?.as_bool().unwrap_or(false) {
                return self.exception_outcome(&oid, true);
            }

            let info = if return_by_value {
                if self.stored_value_is_undefined(&oid)? {
                    Self::undefined_info()
                } else {
                    let read = self
                        .runtime
                        .execute_script("<readResult>", &Self::object_slot(&oid))
                        .map_err(|e| format!("JS error: {}", e))?;
                    let json_val = self.v8_to_json(read)?;
                    Self::info_from_json(&json_val)
                }
            } else {
                let meta_result = self
                    .runtime
                    .execute_script("<readMeta>", "globalThis.__diting_await_meta")
                    .map_err(|e| format!("JS error: {}", e))?;
                let meta_str = self.v8_to_json(meta_result)?;
                let meta_json = if let serde_json::Value::String(s) = &meta_str {
                    serde_json::from_str(s).unwrap_or(meta_str.clone())
                } else {
                    meta_str
                };
                Self::info_from_meta(&meta_json, Some(oid.clone()))
            };
            self.object_store
                .insert(oid.clone(), Self::object_slot(&oid));
            return Ok(EvalOutcome {
                info,
                exception: None,
            });
        }

        if return_by_value {
            let code = format!(
                "(function() {{\n\
                {setup}\n\
                var __fn = ({fn_decl});\n\
                var __this = ({this_expr});\n\
                globalThis.__diting_await_rejected = false;\n\
                try {{\n\
                    {slot} = __fn.call(__this, {args});\n\
                    return {slot};\n\
                }} catch(e) {{\n\
                    {slot} = e;\n\
                    globalThis.__diting_await_meta = {exc_meta_fn};\n\
                    globalThis.__diting_await_rejected = true;\n\
                    return undefined;\n\
                }}\n\
            }})()",
                setup = setup,
                fn_decl = function_declaration,
                this_expr = this_expr,
                args = args_list,
                slot = Self::object_slot(&oid),
                exc_meta_fn = exc_meta_fn,
            );
            let result = self
                .runtime
                .execute_script("<callFnByValue>", &code)
                .map_err(|e| format!("JS error: {}", e))?;
            let rejected = self
                .runtime
                .execute_script("<readRejected>", "globalThis.__diting_await_rejected")
                .map_err(|e| format!("JS error: {}", e))?;
            if self.v8_to_json(rejected)?.as_bool().unwrap_or(false) {
                return self.exception_outcome(&oid, false);
            }
            if self.stored_value_is_undefined(&oid)? {
                return Ok(EvalOutcome {
                    info: Self::undefined_info(),
                    exception: None,
                });
            }
            let json_val = self.v8_to_json(result)?;
            return Ok(EvalOutcome {
                info: Self::info_from_json(&json_val),
                exception: None,
            });
        }

        let code = format!(
            "(function() {{\n\
            {setup}\n\
            var __fn = ({fn_decl});\n\
            var __this = ({this_expr});\n\
            var __result;\n\
            try {{\n\
                __result = __fn.call(__this, {args});\n\
                {slot} = __result;\n\
                globalThis.__diting_await_meta = {meta_fn};\n\
                globalThis.__diting_await_rejected = false;\n\
            }} catch(e) {{\n\
                {slot} = e;\n\
                globalThis.__diting_await_meta = {exc_meta_fn};\n\
                globalThis.__diting_await_rejected = true;\n\
            }}\n\
        }})()",
            setup = setup,
            fn_decl = function_declaration,
            this_expr = this_expr,
            args = args_list,
            slot = Self::object_slot(&oid),
            meta_fn = Self::meta_extract_js("__result"),
            exc_meta_fn = exc_meta_fn,
        );
        self.runtime
            .execute_script("<callFnRemote>", &code)
            .map_err(|e| format!("JS error: {}", e))?;
        let rejected = self
            .runtime
            .execute_script("<readRejected>", "globalThis.__diting_await_rejected")
            .map_err(|e| format!("JS error: {}", e))?;
        if self.v8_to_json(rejected)?.as_bool().unwrap_or(false) {
            return self.exception_outcome(&oid, false);
        }
        // The IIFE returns nothing (it parks the meta JSON in a global) —
        // read the global back, same as the evaluate handle path does.
        // Reading the IIFE's own return value used to feed `null` into
        // info_from_meta; it went unnoticed because object_id was threaded
        // through unconditionally (obscura#779 tightened that contract).
        let meta = self
            .runtime
            .execute_script("<readMeta>", "globalThis.__diting_await_meta")
            .map_err(|e| format!("JS error: {}", e))?;
        let meta_str = self.v8_to_json(meta)?;
        let meta_json = if let serde_json::Value::String(s) = &meta_str {
            serde_json::from_str(s).unwrap_or(meta_str.clone())
        } else {
            meta_str
        };
        self.object_store
            .insert(oid.clone(), Self::object_slot(&oid));
        Ok(EvalOutcome {
            info: Self::info_from_meta(&meta_json, Some(oid)),
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
        if self.object_store.remove(object_id).is_some() {
            let code = format!("delete {};", Self::object_slot(object_id));
            let _ = self.runtime.execute_script("<release>", &code);
        }
    }
}
