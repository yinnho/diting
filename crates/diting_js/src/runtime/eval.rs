//! Expression evaluation on a live realm.
//!
//! `evaluate` runs one expression and returns JSON. `evaluate_with_timeout`
//! bounds that with the V8 watchdog. `v8_to_json` is the shared conversion
//! the remote-object path also uses.

impl super::JsRuntime {
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

    pub(super) fn v8_to_json(
        &mut self,
        result: v8::Global<v8::Value>,
    ) -> Result<serde_json::Value, String> {
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
}
