//! `Runtime.*` on the page isolate. Object ids live in the realm's store
//! (`evaluate_remote` / `callFunctionOn`); a thrown script comes back as
//! `exceptionDetails`, not a protocol error.
use diting::diting_js::runtime::{EvalOutcome, RemoteObjectInfo};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::page::{Page, INTERACTION_EVAL_TIMEOUT};
use crate::session::{
    mouse_button_code, mouse_button_mask, mouse_down_js, mouse_move_js, mouse_up_js, INPUT_HELPERS,
};

const AWAIT_MS: u64 = 30_000;

/// Stripped by the dispatcher before the command result is written. Carries
/// console calls and the document.write load signal for this command.
pub(crate) const SIDE_KEY: &str = "__ditingSide";

pub(crate) async fn on_runtime(
    page: &mut Page,
    method: String,
    params: Value,
    reply: oneshot::Sender<Result<Value, String>>,
) {
    let result = match method.as_str() {
        "Runtime.evaluate" => evaluate(page, &params).await,
        "Runtime.callFunctionOn" => call_function_on(page, &params).await,
        "Runtime.releaseObject" => release(page, &params),
        "Input.dispatchMouseEvent" => mouse_event(page, &params),
        "Input.insertText" => insert_text(page, &params),
        "Input.dispatchKeyEvent" => key_event(page, &params),
        "DOM.getContentQuads" => content_quads(page, &params).await,
        "DOM.scrollIntoViewIfNeeded" => scroll_into_view(page, &params).await,
        other => Err(format!("'{other}' wasn't found")),
    };
    // Take the queues here, on the session thread, before the actor's
    // post-command drain moves them into the HTTP console ring.
    let result = match result {
        Ok(mut value) => {
            if let Some(side) = page_side(page, &params) {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert(SIDE_KEY.to_string(), side);
                }
            }
            Ok(value)
        }
        Err(err) => Err(err),
    };
    let _ = reply.send(result);
}

fn page_side(page: &mut Page, params: &Value) -> Option<Value> {
    let calls = page.inner.take_pending_console_calls();
    let write_nav = page.inner.take_pending_write_nav();
    let console: Vec<Value> = calls
        .into_iter()
        .filter_map(|(level, text, url)| {
            let kind = console_type(&level)?;
            Some(json!({ "type": kind, "text": text, "url": url }))
        })
        .collect();
    if console.is_empty() && !write_nav {
        return None;
    }
    let context_id = params
        .get("contextId")
        .or_else(|| params.get("executionContextId"))
        .and_then(|v| v.as_i64())
        .filter(|id| *id > 0);
    Some(json!({
        "console": console,
        "writeNav": write_nav,
        "contextId": context_id,
    }))
}

/// Playwright's setContent matches `console.debug` only. `dialog` is a
/// session-console record, not a page console API call.
fn console_type(level: &str) -> Option<&'static str> {
    Some(match level {
        "dialog" => return None,
        "warn" | "warning" => "warning",
        "error" => "error",
        "debug" => "debug",
        "info" => "info",
        _ => "log",
    })
}

async fn evaluate(page: &mut Page, params: &Value) -> Result<Value, String> {
    let expression = params
        .get("expression")
        .and_then(|v| v.as_str())
        .ok_or("Runtime.evaluate: missing expression")?;
    let return_by_value = params
        .get("returnByValue")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let await_promise = params
        .get("awaitPromise")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if page.inner.js.is_none() {
        return Err("page has no JS runtime".to_string());
    }
    let outcome = page
        .inner
        .evaluate_remote_outcome(expression, return_by_value, await_promise, AWAIT_MS, None)
        .await;
    Ok(outcome_result(outcome))
}

async fn call_function_on(page: &mut Page, params: &Value) -> Result<Value, String> {
    let declaration = params
        .get("functionDeclaration")
        .and_then(|v| v.as_str())
        .ok_or("Runtime.callFunctionOn: missing functionDeclaration")?;
    let object_id = params.get("objectId").and_then(|v| v.as_str());
    let return_by_value = params
        .get("returnByValue")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let await_promise = params
        .get("awaitPromise")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let arguments = params
        .get("arguments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let js = page.inner.js.as_mut().ok_or("page has no JS runtime")?;
    let outcome = js
        .call_function_on_remote_outcome(
            declaration,
            object_id,
            &arguments,
            return_by_value,
            await_promise,
            None,
        )
        .await?;
    Ok(outcome_result(outcome))
}

fn release(page: &mut Page, params: &Value) -> Result<Value, String> {
    if let Some(id) = params.get("objectId").and_then(|v| v.as_str()) {
        if let Some(js) = page.inner.js.as_mut() {
            js.release_object(id);
        }
    }
    Ok(json!({}))
}

fn mouse_event(page: &mut Page, params: &Value) -> Result<Value, String> {
    let kind = params.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let button_name = params
        .get("button")
        .and_then(|v| v.as_str())
        .unwrap_or("left");
    let code = mouse_button_code(button_name);
    let buttons = params
        .get("buttons")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| mouse_button_mask(button_name));
    let click_count = params
        .get("clickCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let modifiers = params
        .get("modifiers")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let script = match kind {
        "mousePressed" => mouse_down_js(x, y, code, buttons, click_count, modifiers),
        "mouseReleased" => mouse_up_js(x, y, code, click_count, modifiers),
        "mouseMoved" => mouse_move_js(x, y, buttons, modifiers),
        "mouseWheel" => {
            let dx = params.get("deltaX").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dy = params.get("deltaY").and_then(|v| v.as_f64()).unwrap_or(0.0);
            format!(
                "(function() {{\
                    var hit = globalThis.__diting_hitTarget ? globalThis.__diting_hitTarget({x},{y}) : null;\
                    var target = (hit && hit.el) || document.body;\
                    if (target) target.dispatchEvent(new WheelEvent('wheel', {{bubbles:true,cancelable:true,clientX:{x},clientY:{y},deltaX:{dx},deltaY:{dy}}}));\
                    window.scrollBy({dx},{dy});\
                }})()",
                x = x,
                y = y,
                dx = dx,
                dy = dy,
            )
        }
        _ => return Ok(json!({})),
    };
    dispatch_input(page, &script)
}

fn insert_text(page: &mut Page, params: &Value) -> Result<Value, String> {
    let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let literal = serde_json::to_string(text).map_err(|err| err.to_string())?;
    dispatch_input(page, &insert_text_js(&literal))
}

fn key_event(page: &mut Page, params: &Value) -> Result<Value, String> {
    let kind = params.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if kind == "keyUp" {
        return Ok(json!({}));
    }
    let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
    let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let modifiers = params
        .get("modifiers")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    // Ctrl or Meta + A selects the field. Playwright's fill selects in the
    // page before insertText; keyboard.press("Control+A") comes through here.
    if modifiers & 0b110 != 0 && key.eq_ignore_ascii_case("a") {
        return dispatch_input(
            page,
            "(function() {\
                var el = document.activeElement;\
                if (!el) return;\
                if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {\
                    try { el.setSelectionRange(0, el.value.length); } catch (e) {}\
                } else if (el.isContentEditable) {\
                    var range = document.createRange();\
                    range.selectNodeContents(el);\
                    var sel = window.getSelection();\
                    if (sel) { sel.removeAllRanges(); sel.addRange(range); }\
                }\
            })()",
        );
    }
    if text.is_empty() {
        return Ok(json!({}));
    }
    let literal = serde_json::to_string(text).map_err(|err| err.to_string())?;
    dispatch_input(page, &insert_text_js(&literal))
}

fn insert_text_js(literal: &str) -> String {
    format!(
        "(function() {{\
            var text = {literal};\
            var el = document.activeElement;\
            if (!el || el === document.body || el === document.documentElement) return;\
            if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {{\
                var start = el.selectionStart == null ? el.value.length : el.selectionStart;\
                var end = el.selectionEnd == null ? start : el.selectionEnd;\
                var proto = el.tagName === 'TEXTAREA' ? HTMLTextAreaElement : HTMLInputElement;\
                var desc = Object.getOwnPropertyDescriptor(proto.prototype, 'value');\
                var next = el.value.slice(0, start) + text + el.value.slice(end);\
                if (desc && desc.set) desc.set.call(el, next); else el.value = next;\
                var pos = start + text.length;\
                try {{ el.setSelectionRange(pos, pos); }} catch (e) {{}}\
                el.dispatchEvent(new Event('input', {{bubbles:true}}));\
                return;\
            }}\
            if (el.isContentEditable) document.execCommand('insertText', false, text);\
        }})()"
    )
}

async fn scroll_into_view(page: &mut Page, params: &Value) -> Result<Value, String> {
    let object_id = params.get("objectId").and_then(|v| v.as_str());
    let rect = params.get("rect").cloned().unwrap_or(Value::Null);
    let rect_js = serde_json::to_string(&rect).unwrap_or_else(|_| "null".to_string());
    // `nearest` is a no-op when the box already intersects the viewport, so
    // an on-screen click does not move scroll. A sub-rect (a position click
    // inside a box taller than the viewport) is nudged in afterwards.
    let declaration = format!(
        "function() {{\
            if (!this || typeof this.scrollIntoView !== 'function') return;\
            this.scrollIntoView({{block:'nearest', inline:'nearest'}});\
            var rect = {rect_js};\
            if (!rect || typeof rect !== 'object' || !this.getBoundingClientRect) return;\
            var box = this.getBoundingClientRect();\
            var x = box.x + (+rect.x || 0);\
            var y = box.y + (+rect.y || 0);\
            var w = rect.width == null ? box.width : +rect.width;\
            var h = rect.height == null ? box.height : +rect.height;\
            var vw = window.innerWidth || 0, vh = window.innerHeight || 0;\
            var dx = x < 0 ? x : (x + w > vw ? x + w - vw : 0);\
            var dy = y < 0 ? y : (y + h > vh ? y + h - vh : 0);\
            if (!dx && !dy) return;\
            var root = document.scrollingElement || document.documentElement;\
            if (!root) return;\
            if (dx) root.scrollLeft = (root.scrollLeft || 0) + dx;\
            if (dy) root.scrollTop = (root.scrollTop || 0) + dy;\
        }}"
    );
    let js = page.inner.js.as_mut().ok_or("page has no JS runtime")?;
    let _ = js
        .call_function_on_remote_outcome(&declaration, object_id, &[], true, false, None)
        .await?;
    Ok(json!({}))
}

async fn content_quads(page: &mut Page, params: &Value) -> Result<Value, String> {
    let object_id = params.get("objectId").and_then(|v| v.as_str());
    let js = page.inner.js.as_mut().ok_or("page has no JS runtime")?;
    let outcome = js
        .call_function_on_remote_outcome(
            "function(){ if (!this || !this.getBoundingClientRect) return null; var r = this.getBoundingClientRect(); return {x:r.x, y:r.y, w:r.width, h:r.height}; }",
            object_id,
            &[],
            true,
            false,
            None,
        )
        .await?;
    if outcome.exception.is_some() {
        return Ok(json!({ "quads": [] }));
    }
    let value = outcome.info.value.unwrap_or(Value::Null);
    let x = value.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = value.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let w = value.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let h = value.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if w <= 0.0 || h <= 0.0 {
        return Ok(json!({ "quads": [] }));
    }
    Ok(json!({
        "quads": [[x, y, x + w, y, x + w, y + h, x, y + h]]
    }))
}

fn dispatch_input(page: &mut Page, script: &str) -> Result<Value, String> {
    page.evaluate_with_timeout_result(INPUT_HELPERS, INTERACTION_EVAL_TIMEOUT)
        .map_err(|err| format!("input helpers: {err}"))?;
    page.evaluate_with_timeout_result(script, INTERACTION_EVAL_TIMEOUT)
        .map_err(|err| format!("input: {err}"))?;
    Ok(json!({}))
}

fn outcome_result(outcome: EvalOutcome) -> Value {
    let mut body = json!({ "result": remote_object(&outcome.info) });
    if let Some(exc) = outcome.exception {
        let mut exception = json!({
            "type": "object",
            "subtype": "error",
            "className": exc.class_name,
            "description": exc.description,
        });
        if let Some(id) = &exc.object_id {
            exception["objectId"] = json!(id);
        }
        let mut details = json!({
            "exceptionId": 1,
            "text": exc.text,
            "lineNumber": exc.line.unwrap_or(0),
            "columnNumber": exc.col.unwrap_or(0),
            "exception": exception,
        });
        if exc.line.is_some() || exc.col.is_some() {
            details["stackTrace"] = json!({
                "callFrames": [{
                    "functionName": "",
                    "scriptId": "0",
                    "url": "",
                    "lineNumber": exc.line.unwrap_or(0),
                    "columnNumber": exc.col.unwrap_or(0),
                }]
            });
        }
        body["exceptionDetails"] = details;
    }
    body
}

fn remote_object(info: &RemoteObjectInfo) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), json!(info.js_type));
    if let Some(subtype) = &info.subtype {
        obj.insert("subtype".into(), json!(subtype));
    }
    if !info.class_name.is_empty() {
        obj.insert("className".into(), json!(info.class_name));
    }
    if !info.description.is_empty() {
        obj.insert("description".into(), json!(info.description));
    }
    if let Some(id) = &info.object_id {
        obj.insert("objectId".into(), json!(id));
    }
    if let Some(value) = &info.value {
        obj.insert("value".into(), value.clone());
    }
    Value::Object(obj)
}
