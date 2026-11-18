//! `Runtime.*` on the page isolate. Object ids live in the realm's store
//! (`evaluate_remote` / `callFunctionOn`); a thrown script comes back as
//! `exceptionDetails`, not a protocol error.
use diting::diting_js::runtime::{EvalOutcome, RemoteObjectInfo};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::page::Page;

const AWAIT_MS: u64 = 30_000;

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
        other => Err(format!("'{other}' wasn't found")),
    };
    let _ = reply.send(result);
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
