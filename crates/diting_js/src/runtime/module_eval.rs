//! ES module load and evaluation on a live realm.
//!
//! `load_module` fetches a graph; `load_inline_module` evaluates source the
//! page already has. Both drive the evaluation to completion rather than
//! waiting for the event loop to go idle.

use diting_runtime::ModuleId;

use super::JsRuntime;

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

impl JsRuntime {
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
}
