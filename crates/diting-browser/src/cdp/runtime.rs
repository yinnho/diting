//! `Runtime.*` / `Input.*` / `DOM.*` on the page isolate, and the sort for
//! every other page-level command the bridge forwards (those live in
//! `emulation`). Object ids live in the realm's store (`evaluate_remote` /
//! `callFunctionOn`); a thrown script comes back as `exceptionDetails`,
//! not a protocol error. An `objectId` the page does not hold is `-32000`
//! in Chrome's word for what is wrong with it ([`dead_handle`]) — the
//! engine would run the call against the global object — and an engine
//! failure (a watchdog, an eval budget) is a protocol error, `-32000` or
//! `-32603`, never a result.
//!
//! `DOM.*` names a node by `objectId` or by `backendNodeId`. The engine has
//! no node inspector, so the bridge keeps its own numbers: a registry on
//! the page that `DOM.describeNode` numbers nodes from. The registry is
//! new with every document and the hub holds the counter, so the numbers
//! go on through a navigation: an old one names nothing in the next
//! document.
//! `nodeId` is the frontend's numbering, handed out by `DOM.getDocument`,
//! which the bridge does not answer — so a `nodeId` is a node not found.
//! What Chrome answers for a node it cannot find or lay out is answered
//! here in the same words.
//! - `DOM.describeNode` answers the node on its own: `depth` and `pierce`
//!   are taken and not read, so there is no `children` under it.
//!
//! The parameters are read the way Chrome reads them (`params`), and a
//! refusal carries the code Chrome gives it.
//!
//! Where this engine's answers differ from Chrome's:
//! - An object id is this engine's own spelling, not Chrome's
//!   `<script>.<context>.<object>`: an id carries its realm's serial and
//!   its number, and an id from a realm that is gone, a number that was
//!   never handed out or a handle that was released is Chrome's word for
//!   it. A client has no way to tell the three apart by looking — only by
//!   the answer it gets, which is Chrome's. The blank in front of an id is
//!   nothing and the one behind it makes it no id, as in Chrome.
//! - `Runtime.evaluate` reads `expression` as Chrome does, as a script:
//!   statements, then an expression — and `{"a": 1}` on its own is a
//!   `SyntaxError`. (The HTTP and flow faces of an eval take the forgiving
//!   text an object literal is.)
//! - The engine runs one realm: a command naming a context the page
//!   announced runs in the page's world, an isolated world's name
//!   not withstanding.
//! - `Runtime.evaluate` / `Runtime.callFunctionOn` answer objects without a
//!   preview (`generatePreview` is taken and not used), and a script has
//!   the bridge's own 30 s (`timeout` is not read). `throwOnSideEffect`
//!   and `includeCommandLineAPI` are refused when true: nothing checks a
//!   script or a call for side effects, and there is no command line API.
//!   What cannot matter here is read and let go: `disableBreaks` (no
//!   debugger to break on), `replMode` (a script either way),
//!   `allowUnsafeEvalBlockedByCSP` (nothing is blocked). Of
//!   `serializationOptions`, `idOnly` is what they answer anyway and `json`
//!   is `returnByValue`; `deep` is refused, nothing here builds a
//!   DeepSerializedValue, and `maxDimensionDepth` is not read.
//! - `Runtime.callFunctionOn` on a declaration that comes to no function
//!   is Chrome's word for it ("Given expression does not evaluate to a
//!   function"), and the declaration may throw first.
//! - `unserializableValue` in a call argument is what V8 makes of it
//!   (`InjectedScript::resolveCallArgument`): a script of its own, run
//!   before the call — a text that will not run is "Couldn't parse value
//!   object in call argument".
//!   - A stack is the page's own frames in Chrome's spelling — the scripts
//!     an eval ran, and a `callFunctionOn` declaration, are shown as the
//!     scripts of their own Chrome compiles them as; the engine's wrappers,
//!     event loop and DOM are left out. What is not Chrome's: a page's
//!     inline `<script>` is named `<script>`, not by the document's URL; an
//!     external script's frames keep their URL; code the page compiled
//!     itself by `eval` or `new Function` loses the `eval at` that says
//!     which line of what called it (V8 keeps no such position for a nested
//!     eval); an inline event handler's function is named as the engine
//!     compiled it (`HTMLButtonElement.eval (eval at _resolveInlineHandler
//!     (unknown source), …)`), where Chrome writes `HTMLButtonElement.onclick
//!     (url:L:C)`. `exceptionDetails` differs as `exception_details` says.
//! - `Input.*` is what the session's own interaction dispatches, by script:
//!   a mouse event is the pointer and mouse events on what the page has at
//!   that point. A key event puts its `text` in the focused field — no
//!   `keydown` or `keyup` is dispatched, so a key without text moves
//!   nothing and `keyUp` is an empty ok — except Ctrl/Meta+A, which
//!   selects the field.
use diting::diting_js::runtime::{
    EvalInput, EvalOutcome, ExceptionInfo, JsRuntime, ObjectIdState, RemoteObjectInfo,
    BAD_CALL_ARGUMENT,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::oneshot;

use super::objects;
use super::Failure;
use crate::page::{Page, INTERACTION_EVAL_TIMEOUT};
use crate::session::{
    mouse_button_code, mouse_button_mask, mouse_down_js, mouse_move_js, mouse_up_js, HandleParts,
    INPUT_HELPERS,
};

use super::params::{
    integer, invalid, invalid_params, kinds, number, optional_boolean, optional_integer,
    optional_number, optional_string, string, Kind,
};

pub(super) const AWAIT_MS: u64 = 30_000;

/// Stripped by the dispatcher before the command result is written. Carries
/// console calls and the document.write load signal for this command.
pub(crate) const SIDE_KEY: &str = "__ditingSide";

/// Run one page-level CDP command on the session thread. `init_scripts` is
/// the session's `Page.addScriptToEvaluateOnNewDocument` registry, as
/// (identifier, source) pairs.
pub(crate) async fn dispatch(
    page: &mut Page,
    init_scripts: &mut Vec<(String, String)>,
    method: String,
    params: Value,
    reply: oneshot::Sender<Result<Result<Value, Failure>, std::convert::Infallible>>,
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
        "DOM.describeNode" => describe_node(page, &params).await,
        "DOM.resolveNode" => resolve_node(page, &params).await,
        "DOM.getBoxModel" => box_model(page, &params).await,
        other => super::emulation::handle(page, init_scripts, other, &params).await,
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
    let _ = reply.send(Ok(result));
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
pub(super) fn console_type(level: &str) -> Option<&'static str> {
    Some(match level {
        "dialog" => return None,
        "warn" | "warning" => "warning",
        "error" => "error",
        "debug" => "debug",
        "info" => "info",
        _ => "log",
    })
}

/// Fields `Runtime.evaluate` and `Runtime.callFunctionOn` both declare and
/// the bridge takes without reading.
const RUNTIME_UNREAD: &[(&str, Kind)] = &[
    ("objectGroup", Kind::Text),
    ("silent", Kind::Bool),
    ("generatePreview", Kind::Bool),
    ("userGesture", Kind::Bool),
];

/// Fields only `Runtime.evaluate` declares, and that cannot matter here.
/// The two of its sort that could — `includeCommandLineAPI`,
/// `throwOnSideEffect` — are read by [`no_such_flag`]. `callFunctionOn`
/// declares none of these, so there they are not read at all, as in Chrome.
const EVALUATE_UNREAD: &[(&str, Kind)] = &[
    ("disableBreaks", Kind::Bool),
    ("replMode", Kind::Bool),
    ("allowUnsafeEvalBlockedByCSP", Kind::Bool),
];

/// What `Runtime.evaluate` asks the engine for, already read off the
/// wire — the face's translation of the params into IR vocabulary.
pub(super) struct EvalArgs {
    pub script: String,
    pub await_promise: bool,
    pub return_by_value: bool,
}

/// `Runtime.evaluate`'s params, checked the way Chrome checks them —
/// everything up to the engine call, which is the session's
/// `EvalRemote` command now.
pub(super) fn eval_params(params: &Value) -> Result<EvalArgs, Failure> {
    const METHOD: &str = "Runtime.evaluate";
    let expression = string(params, "expression")?;
    kinds(params, RUNTIME_UNREAD)?;
    kinds(params, EVALUATE_UNREAD)?;
    kinds(params, &[("timeout", Kind::Number)])?;
    let return_by_value = by_value(METHOD, params)?;
    let await_promise = flag(params, "awaitPromise")?;
    no_such_flag(
        METHOD,
        params,
        &[
            ("includeCommandLineAPI", "there is no command line API"),
            (
                "throwOnSideEffect",
                "nothing checks a script for side effects",
            ),
        ],
    )?;
    let context = optional_integer(params, "contextId")?;
    let unique = optional_string(params, "uniqueContextId")?;
    if context.is_some() && unique.is_some() {
        return Err(invalid(
            "contextId and uniqueContextId are mutually exclusive",
        ));
    }
    live_context(params, context, unique)?;
    Ok(EvalArgs {
        script: expression.to_string(),
        await_promise,
        return_by_value,
    })
}

/// What `Runtime.callFunctionOn` asks the engine for, already read off
/// the wire — the face's translation into IR vocabulary, with every id
/// pre-read into [`HandleParts`] so the session thread never sees a
/// spelling.
pub(super) struct CallArgs {
    pub declaration: String,
    pub object_id: Option<String>,
    pub arguments: Vec<Value>,
    pub await_promise: bool,
    pub return_by_value: bool,
    pub receiver: Option<HandleParts>,
    pub arg_ids: Vec<Option<HandleParts>>,
}

/// `Runtime.callFunctionOn`'s params, checked the way Chrome checks
/// them — everything up to the engine call, which is the session's
/// `CallOn` command now.
pub(super) fn call_params(params: &Value) -> Result<CallArgs, Failure> {
    const METHOD: &str = "Runtime.callFunctionOn";
    let declaration = string(params, "functionDeclaration")?;
    let object_id = optional_string(params, "objectId")?;
    let arguments = arguments(params)?;
    kinds(params, RUNTIME_UNREAD)?;
    let return_by_value = by_value(METHOD, params)?;
    let await_promise = flag(params, "awaitPromise")?;
    no_such_flag(
        METHOD,
        params,
        &[(
            "throwOnSideEffect",
            "nothing checks a call for side effects",
        )],
    )?;
    let context = optional_integer(params, "executionContextId")?;
    let unique = optional_string(params, "uniqueContextId")?;
    match [object_id.is_some(), context.is_some(), unique.is_some()]
        .iter()
        .filter(|named| **named)
        .count()
    {
        0 => {
            return Err(invalid(
                "Either objectId or executionContextId or uniqueContextId must be specified",
            ))
        }
        // Chrome takes one of the three, never two.
        1 => {}
        _ => {
            return Err(invalid(
                "ObjectId, executionContextId and uniqueContextId must mutually exclude each other",
            ))
        }
    }
    live_context(params, context, unique)?;
    // Every id the call names, read once here — the session thread asks
    // the engine about the parts, never the spelling.
    let receiver = object_id.as_deref().map(objects::handle_parts);
    let arg_ids = arguments
        .iter()
        .map(|argument| {
            argument
                .get("objectId")
                .and_then(Value::as_str)
                .map(objects::handle_parts)
        })
        .collect();
    Ok(CallArgs {
        declaration: declaration.to_string(),
        object_id: object_id.map(str::to_string),
        arguments,
        await_promise,
        return_by_value,
        receiver,
        arg_ids,
    })
}

/// The wire answer for an `EvalRemote`/`CallOn` outcome — what
/// `settled()` builds, with the by-value read already done on the
/// session thread. `pre_info` is the thrown value's info from before
/// that read, which is what `exceptionDetails` shows.
pub(super) fn eval_result(
    outcome: EvalOutcome,
    pre_info: Option<RemoteObjectInfo>,
) -> Result<Value, Failure> {
    let thrown = outcome.exception.as_ref().map(|_| {
        pre_info.map_or_else(|| remote_object(&outcome.info), |info| remote_object(&info))
    });
    Ok(outcome_result(outcome, thrown))
}

async fn evaluate(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    const METHOD: &str = "Runtime.evaluate";
    let expression = string(params, "expression")?;
    kinds(params, RUNTIME_UNREAD)?;
    kinds(params, EVALUATE_UNREAD)?;
    kinds(params, &[("timeout", Kind::Number)])?;
    let return_by_value = by_value(METHOD, params)?;
    let await_promise = flag(params, "awaitPromise")?;
    no_such_flag(
        METHOD,
        params,
        &[
            ("includeCommandLineAPI", "there is no command line API"),
            (
                "throwOnSideEffect",
                "nothing checks a script for side effects",
            ),
        ],
    )?;
    let context = optional_integer(params, "contextId")?;
    let unique = optional_string(params, "uniqueContextId")?;
    if context.is_some() && unique.is_some() {
        return Err(invalid(
            "contextId and uniqueContextId are mutually exclusive",
        ));
    }
    live_context(params, context, unique)?;
    // A handle first, whatever was asked for: it says what the result is,
    // and a value asked for is read off it after.
    let js = page.inner.js.as_mut().ok_or("page has no JS runtime")?;
    let outcome = js
        .evaluate_remote_outcome(
            expression,
            false,
            await_promise,
            AWAIT_MS,
            None,
            EvalInput::Script,
        )
        .await
        .map_err(engine_failure)?;
    settled(js, outcome, return_by_value)
}

/// `returnByValue` — and `serialization: "json"`, which asks the same: the
/// value where the handle was, the way Chrome brings one back
/// ([`JsRuntime::value_of`]). A primitive is its own value already, and
/// `null` keeps its `subtype`. A symbol has no value to bring back.
///
/// What a promise was rejected with comes back the way a result does; what a
/// script threw does not — it is shown as thrown, whatever was asked for
/// (Chrome 158, measured).
fn serialize_by_value(js: &mut JsRuntime, outcome: &mut EvalOutcome) -> Result<(), Failure> {
    let rejected = match &outcome.exception {
        Some(exception) if !exception.in_promise => return Ok(()),
        Some(_) => true,
        None => false,
    };
    let info = &mut outcome.info;
    match info.js_type.as_str() {
        "object" | "function" if info.subtype.as_deref() != Some("null") => {}
        "symbol" => {
            // The reply is the error, and nothing will name a rejected
            // one's slot.
            if rejected {
                if let Some(id) = &info.object_id {
                    js.release_object(id);
                }
            }
            return Err("Object couldn't be returned by value".into());
        }
        _ => return Ok(()),
    }
    let Some(id) = info.object_id.clone() else {
        return Ok(());
    };
    let read = js.value_of(&id);
    // Asked for by value: there is no handle to hand out, so none is kept —
    // unless the one in `exceptionDetails` still names the slot, and the
    // reply is not the error.
    if !rejected || read.is_err() {
        js.release_object(&id);
    }
    let value = read.map_err(engine_failure)?;
    let js_type = info.js_type.clone();
    *info = RemoteObjectInfo {
        js_type,
        subtype: None,
        class_name: String::new(),
        description: String::new(),
        object_id: None,
        value: Some(value),
        unserializable_value: None,
    };
    Ok(())
}

/// What the engine could not do, as Chrome says it. A value the engine
/// could not bring back comes in Chrome's own words, and the one Chrome
/// gives a code of its own is the getter that threw.
pub(super) fn engine_failure(err: String) -> Failure {
    match err.as_str() {
        "Internal error" => Failure::Internal(err),
        _ => Failure::from(err),
    }
}

async fn call_function_on(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    const METHOD: &str = "Runtime.callFunctionOn";
    let declaration = string(params, "functionDeclaration")?;
    let object_id = optional_string(params, "objectId")?;
    let arguments = arguments(params)?;
    kinds(params, RUNTIME_UNREAD)?;
    let return_by_value = by_value(METHOD, params)?;
    let await_promise = flag(params, "awaitPromise")?;
    no_such_flag(
        METHOD,
        params,
        &[(
            "throwOnSideEffect",
            "nothing checks a call for side effects",
        )],
    )?;
    let context = optional_integer(params, "executionContextId")?;
    let unique = optional_string(params, "uniqueContextId")?;
    match [object_id.is_some(), context.is_some(), unique.is_some()]
        .iter()
        .filter(|named| **named)
        .count()
    {
        0 => {
            return Err(invalid(
                "Either objectId or executionContextId or uniqueContextId must be specified",
            ))
        }
        // Chrome takes one of the three, never two.
        1 => {}
        _ => {
            return Err(invalid(
                "ObjectId, executionContextId and uniqueContextId must mutually exclude each other",
            ))
        }
    }
    live_context(params, context, unique)?;
    let js = page.inner.js.as_mut().ok_or("page has no JS runtime")?;
    // With a handle the page does not hold the call would run against the
    // global object, and that is never let through. The receiver is asked
    // about first, as Chrome does, then each argument.
    if let Some(dead) = object_id
        .as_deref()
        .and_then(|id| dead_handle(objects::state(js, id)))
    {
        return Err(dead.into());
    }
    // Each argument in order, as Chrome reads them: a dead `objectId` is
    // its own word — and an `unserializableValue` that will not run is
    // Chrome's, before any handle is let go. `objectId` and `value` each
    // mask an `unserializableValue` beside them.
    for argument in &arguments {
        if let Some(id) = argument.get("objectId").and_then(Value::as_str) {
            if let Some(dead) = dead_argument(objects::state(js, id)) {
                return Err(dead.into());
            }
        } else if argument.get("value").is_none() {
            if let Some(text) = argument.get("unserializableValue").and_then(Value::as_str) {
                let outcome = js
                    .evaluate_remote_outcome(text, false, false, AWAIT_MS, None, EvalInput::Script)
                    .await;
                let bad = match outcome {
                    Err(_) => true,
                    Ok(outcome) => outcome.exception.is_some(),
                };
                if bad {
                    return Err(BAD_CALL_ARGUMENT.into());
                }
            }
        }
    }
    // A handle first, whatever was asked for, as `evaluate` does.
    let outcome = js
        .call_function_on_remote_outcome(
            declaration,
            object_id,
            &arguments,
            false,
            await_promise,
            None,
        )
        .await
        .map_err(engine_failure)?;
    settled(js, outcome, return_by_value)
}

/// A boolean flag as Chrome defaults it: false when left out.
fn flag(params: &Value, field: &str) -> Result<bool, Failure> {
    Ok(optional_boolean(params, field)?.unwrap_or(false))
}

/// The flags the engine has nothing behind, each with the reason: true is
/// refused — the call would have been made to mean something it cannot —
/// and false is what they do here.
pub(super) fn no_such_flag(
    method: &str,
    params: &Value,
    flags: &[(&str, &str)],
) -> Result<(), Failure> {
    for (field, why) in flags {
        if flag(params, field)? {
            return Err(Failure::from(format!(
                "{method}: {field} is not supported, {why}"
            )));
        }
    }
    Ok(())
}

/// `serializationOptions`: whether it asks for the value, when it is there
/// (Chrome 158, measured). The object is read whole first — a
/// `serializationOptions` without `serialization` is "Invalid parameters",
/// whatever else the call says. Then `idOnly` is what the bridge answers
/// anyway, `json` is `returnByValue` there; `deep` is refused, nothing here
/// builds a DeepSerializedValue. `maxDimensionDepth` is not read: the value
/// comes back whole.
fn serialization(method: &str, params: &Value) -> Result<Option<bool>, Failure> {
    let Some(options) = params.get("serializationOptions") else {
        return Ok(None);
    };
    if !options.is_object() {
        return Err(invalid_params());
    }
    if options
        .get("additionalParameters")
        .is_some_and(|more| !more.is_object())
    {
        return Err(invalid_params());
    }
    let name = options
        .get("serialization")
        .and_then(Value::as_str)
        .ok_or_else(invalid_params)?;
    match name {
        "idOnly" => Ok(Some(false)),
        "json" => Ok(Some(true)),
        "deep" => Err(Failure::from(format!(
            "{method}: serializationOptions.serialization 'deep' is not supported"
        ))),
        other => Err(invalid(&format!(
            "Unknown serializationOptions.serialization value {other}"
        ))),
    }
}

/// Whether the value is asked for: `serializationOptions` says when it is
/// there — `idOnly` is a handle, `returnByValue` or no — and else
/// `returnByValue` does. Both are read either way, as Chrome reads them.
fn by_value(method: &str, params: &Value) -> Result<bool, Failure> {
    let flag = flag(params, "returnByValue")?;
    Ok(serialization(method, params)?.unwrap_or(flag))
}

/// The contexts a command names have to be ones the page announced — the
/// bridge lists them under `__ditingContexts` before the command leaves it.
/// A context's `uniqueId` is its id spelled out, as the bridge announces it.
fn live_context(params: &Value, context: Option<i64>, unique: Option<&str>) -> Result<(), Failure> {
    if context.is_some_and(|named| !context_is_live(params, named)) {
        return Err(Failure::from("Cannot find context with specified id"));
    }
    if let Some(unique) = unique {
        let empty = Vec::new();
        let spelled = params
            .get("__ditingContexts")
            .and_then(Value::as_array)
            .unwrap_or(&empty)
            .iter()
            .filter_map(Value::as_i64)
            .any(|id| super::unique_id(id) == unique);
        if !spelled {
            return Err(invalid("invalid uniqueContextId"));
        }
    }
    Ok(())
}

/// Whether the page announced this context id.
fn context_is_live(params: &Value, context: i64) -> bool {
    let empty = Vec::new();
    params
        .get("__ditingContexts")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
        .iter()
        .any(|id| id.as_i64() == Some(context))
}

/// What Chrome says of an `objectId` that names nothing it will run
/// against — three answers, by what is wrong with the id
/// ([`ObjectIdState`]). Nothing for a handle the page holds.
fn dead_handle(state: ObjectIdState) -> Option<&'static str> {
    match state {
        ObjectIdState::Held => None,
        ObjectIdState::Unparseable => Some("Invalid remote object id"),
        ObjectIdState::Gone => Some("Cannot find context with specified id"),
        ObjectIdState::Released => Some("Could not find object with given id"),
    }
}

/// The same for an `objectId` among the arguments, where Chrome words an
/// id of a world that is not the receiver's — or of a document gone — its
/// own way.
fn dead_argument(state: ObjectIdState) -> Option<&'static str> {
    match state {
        ObjectIdState::Gone => {
            Some("Argument should belong to the same JavaScript world as target object")
        }
        state => dead_handle(state),
    }
}

/// `arguments`: each one an object, carrying an `objectId` that is a string
/// and an `unserializableValue` that is a string.
fn arguments(params: &Value) -> Result<Vec<Value>, Failure> {
    let Some(list) = params.get("arguments") else {
        return Ok(Vec::new());
    };
    let list = list.as_array().ok_or_else(invalid_params)?;
    for argument in list {
        let Some(object) = argument.as_object() else {
            return Err(invalid_params());
        };
        // `value`, `objectId` and `unserializableValue` are the only keys an
        // argument may carry; anything else is not an argument at all.
        if !object
            .keys()
            .all(|key| matches!(key.as_str(), "value" | "objectId" | "unserializableValue"))
        {
            return Err(invalid_params());
        }
        if object.get("objectId").is_some_and(|id| !id.is_string())
            || argument
                .get("unserializableValue")
                .is_some_and(|value| !value.is_string())
        {
            return Err(invalid_params());
        }
    }
    Ok(list.clone())
}

/// `Runtime.releaseObject`. A handle the page does not hold is refused, as
/// in Chrome: nothing was there to let go.
fn release(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    let id = string(params, "objectId")?;
    let js = page.inner.js.as_mut().ok_or("page has no JS runtime")?;
    if let Some(dead) = dead_handle(objects::state(js, id)) {
        return Err(dead.into());
    }
    js.release_object(id);
    Ok(json!({}))
}

/// The mouse fields the bridge declares and does not read (`id` Chrome
/// does not read either).
const MOUSE_UNREAD: &[(&str, Kind)] = &[("timestamp", Kind::Number), ("pointerType", Kind::Text)];

fn mouse_event(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    let kind = string(params, "type")?;
    let x = number(params, "x")?;
    let y = number(params, "y")?;
    let button = optional_string(params, "button")?;
    let click_count = optional_integer(params, "clickCount")?;
    let buttons = optional_integer(params, "buttons")?;
    let modifiers = optional_integer(params, "modifiers")?;
    let deltas = (
        optional_number(params, "deltaX")?,
        optional_number(params, "deltaY")?,
    );
    let force = optional_number(params, "force")?;
    let pressure = optional_number(params, "tangentialPressure")?;
    let tilt = (
        optional_number(params, "tiltX")?,
        optional_number(params, "tiltY")?,
    );
    let twist = optional_integer(params, "twist")?;
    kinds(params, MOUSE_UNREAD)?;
    // In Chrome's order: the type, the button, then what the sort of event
    // needs.
    if !matches!(
        kind,
        "mousePressed" | "mouseReleased" | "mouseMoved" | "mouseWheel"
    ) {
        return Err(invalid(&format!("Unexpected event type '{kind}'")));
    }
    // Chrome's enum, and an empty button is none.
    if !matches!(
        button,
        None | Some("" | "none" | "left" | "middle" | "right" | "back" | "forward")
    ) {
        return Err(invalid("Invalid mouse button"));
    }
    let (dx, dy) = if kind == "mouseWheel" {
        let (Some(dx), Some(dy)) = (deltas.0, deltas.1) else {
            return Err(invalid(
                "'deltaX' and 'deltaY' are expected for mouseWheel event",
            ));
        };
        // What Chrome keeps a wheel delta to: a float's range.
        if [dx, dy]
            .iter()
            .any(|delta| !delta.is_finite() || delta.abs() > f32::MAX as f64)
        {
            return Err(invalid("Wheel deltas exceed the supported range"));
        }
        (dx, dy)
    } else {
        for (name, value, low, high, range) in [
            ("force", force, 0.0, 1.0, "[0,1]"),
            ("tangential_pressure", pressure, -1.0, 1.0, "[-1,1]"),
            ("tilt_x", tilt.0, -90.0, 90.0, "[-90,90]"),
            ("tilt_y", tilt.1, -90.0, 90.0, "[-90,90]"),
        ] {
            if value.is_some_and(|value| value < low || value > high) {
                return Err(invalid(&format!(
                    "'{name}' should be in the range of {range}"
                )));
            }
        }
        if twist.is_some_and(|twist| !(0..=359).contains(&twist)) {
            return Err(invalid("'twist' should be in the range of [0,359]"));
        }
        (0.0, 0.0)
    };
    // Left out, the press is a left one.
    let button = match button {
        Some("" | "none") => "none",
        Some(button) => button,
        None => "left",
    };
    let code = mouse_button_code(button);
    let buttons = buttons.map_or_else(|| mouse_button_mask(button), |buttons| buttons as u64);
    let click_count = click_count.map_or(1, |count| count as u64);
    let modifiers = modifiers.unwrap_or(0) as u64;
    let script = match kind {
        "mousePressed" => mouse_down_js(x, y, code, buttons, click_count, modifiers),
        "mouseReleased" => mouse_up_js(x, y, code, click_count, modifiers),
        "mouseMoved" => mouse_move_js(x, y, buttons, modifiers),
        _ => format!(
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
        ),
    };
    dispatch_input(page, &script)
}

fn insert_text(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    let text = string(params, "text")?;
    let literal = serde_json::to_string(text).map_err(|err| err.to_string())?;
    dispatch_input(page, &insert_text_js(&literal))
}

/// The key fields the bridge declares and does not read: a key event here
/// only inserts its `text` (see [`key_event`]).
const KEY_UNREAD: &[(&str, Kind)] = &[
    ("timestamp", Kind::Number),
    ("unmodifiedText", Kind::Text),
    ("keyIdentifier", Kind::Text),
    ("code", Kind::Text),
    ("windowsVirtualKeyCode", Kind::Integer),
    ("nativeVirtualKeyCode", Kind::Integer),
    ("autoRepeat", Kind::Bool),
    ("isKeypad", Kind::Bool),
    ("isSystemKey", Kind::Bool),
    ("location", Kind::Integer),
    ("commands", Kind::Texts),
];

fn key_event(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    let kind = string(params, "type")?;
    let key = optional_string(params, "key")?;
    let text = optional_string(params, "text")?;
    let modifiers = optional_integer(params, "modifiers")?;
    kinds(params, KEY_UNREAD)?;
    if !matches!(kind, "keyDown" | "keyUp" | "rawKeyDown" | "char") {
        return Err(invalid(&format!("Unexpected event type '{kind}'")));
    }
    let (key, text) = (key.unwrap_or(""), text.unwrap_or(""));
    let modifiers = modifiers.unwrap_or(0) as u64;
    if kind == "keyUp" {
        return Ok(json!({}));
    }
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

/// The registry and the readers the `DOM.*` commands share, on the page —
/// one per document. A `backendNodeId` is a number of this page's own: a
/// node keeps its number for as long as the document does, and the numbers
/// keep their nodes. The numbers come from the hub (`fresh`), which counts
/// a page's nodes through its documents: a number of a document gone names
/// nothing in the next one (Chrome counts each document from 1).
const NODE_JS: &str = r"(function(fresh) {
    var dom = globalThis.__diting_cdp_dom;
    if (dom && dom.reg.doc === document) return dom;
    var doc = document;
    var reg = {
        doc: doc,
        ids: new WeakMap(),
        nodes: Object.create(null),
    };
    dom = globalThis.__diting_cdp_dom = {
        reg: reg,
        byId: function(id) { return reg.nodes[id] || null; },
        attached: function(node) {
            return node === doc || !!(doc.documentElement && doc.documentElement.contains(node));
        },
        laidOut: function(node) {
            if (!dom.attached(node)) return false;
            for (var at = node; at && at.nodeType === 1; at = at.parentElement) {
                if (getComputedStyle(at).display === 'none') return false;
            }
            return true;
        },
        // Chrome's spelling of a node: `div#id.class`, `#text`, `#document`.
        label: function(node) {
            var name = String(node.nodeName || 'node').toLowerCase();
            if (node.nodeType !== 1) return name;
            if (node.hasAttribute('id')) name += '#' + node.getAttribute('id');
            if (node.hasAttribute('class')) {
                name += '.' + String(node.getAttribute('class')).split(/\s+/).filter(Boolean).join('.');
            }
            return name;
        },
        // `fresh` is the number the hub reserved for this call: a node
        // without one takes it. (Read off the closure it would be the
        // number of whichever call built the registry — one number for
        // every node.)
        describe: function(node, frameId, fresh) {
            var type = node.nodeType;
            var out = {
                nodeId: 0,
                backendNodeId: dom.id(node, fresh),
                nodeType: type,
                nodeName: node.nodeName,
                localName: node.localName || '',
                nodeValue: node.nodeValue == null ? '' : String(node.nodeValue),
                childNodeCount: node.childNodes ? node.childNodes.length : 0,
            };
            if (type === 1) {
                var attributes = [];
                for (var i = 0; node.attributes && i < node.attributes.length; i++) {
                    attributes.push(node.attributes[i].name, node.attributes[i].value);
                }
                out.attributes = attributes;
                if (frameId && node === doc.documentElement) out.frameId = frameId;
            } else if (type === 9) {
                out.documentURL = node.URL || '';
                out.baseURL = node.baseURI || '';
                out.xmlVersion = '';
                out.compatibilityMode = node.compatMode === 'BackCompat' ? 'QuirksMode' : 'NoQuirksMode';
            }
            return out;
        },
        id: function(node, fresh) {
            var id = reg.ids.get(node);
            if (!id) {
                id = fresh;
                reg.ids.set(node, id);
                reg.nodes[id] = node;
            }
            return id;
        },
        // Four quads in CSS pixels of the viewport: the corners of the
        // content, padding, border and margin boxes, clockwise from the
        // top left.
        box: function(node) {
            var r = node.getBoundingClientRect(), cs = getComputedStyle(node);
            // A border edge is `border-top-width`; padding and margin are
            // `padding-top` and `margin-top` — no `Width` on those.
            var edge = function(prefix, side, suffix) {
                var value = parseFloat(cs[prefix + side + suffix]);
                return isNaN(value) ? 0 : value;
            };
            var sides = function(prefix, suffix) {
                return {
                    t: edge(prefix, 'Top', suffix), r: edge(prefix, 'Right', suffix),
                    b: edge(prefix, 'Bottom', suffix), l: edge(prefix, 'Left', suffix),
                };
            };
            var b = sides('border', 'Width'), p = sides('padding', ''), m = sides('margin', '');
            var quad = function(x0, y0, x1, y1) {
                return [x0, y0, x1, y0, x1, y1, x0, y1];
            };
            return {
                content: quad(r.left + b.l + p.l, r.top + b.t + p.t,
                              r.right - b.r - p.r, r.bottom - b.b - p.b),
                padding: quad(r.left + b.l, r.top + b.t, r.right - b.r, r.bottom - b.b),
                border: quad(r.left, r.top, r.right, r.bottom),
                margin: quad(r.left - m.l, r.top - m.t, r.right + m.r, r.bottom + m.b),
                width: Math.round(r.width),
                height: Math.round(r.height),
            };
        },
    };
    return dom;
})";

/// The key a `DOM.*` script reports its verdict under.
const VERDICT: &str = "__diting";
const EITHER: &str = "Either nodeId, backendNodeId or objectId must be specified";
const NO_NODE: &str = "No node with given id found";
const NO_BOX: &str = "Could not compute box model.";

/// Why a `DOM.*` script did not run. All but the last are Chrome's own
/// answers — see [`words`].
enum Refusal {
    /// `objectId` names nothing the page holds: Chrome's word for the way
    /// it is dead ([`dead_handle`]).
    Dead(&'static str),
    /// `backendNodeId` is no number this page gave.
    NoNumber,
    /// A `nodeId`: the frontend's own numbering, which nothing here hands
    /// out.
    NoFrontendNode,
    /// A field is of the wrong kind: `objectId` no string, `backendNodeId`
    /// no whole number, `nodeId` no whole number.
    Invalid,
    /// The `objectId` is not a node's.
    NotANode,
    /// The node left the document.
    Detached,
    /// The node is in the document, and nothing displays it.
    NoLayout,
    /// No node was named, in the command's own words.
    Unnamed(&'static str),
    /// The bridge's script threw, or the page has no runtime.
    Fault(String),
}

/// The failure a refusal is, in Chrome's words: a `backendNodeId` that is
/// no whole number is `-32602` there, every other refusal `-32000`.
fn words(refusal: Refusal) -> Failure {
    match refusal {
        Refusal::Dead(word) => word.into(),
        Refusal::NoNumber => "No node found for given backend id".into(),
        Refusal::NoFrontendNode => "Could not find node with given id".into(),
        Refusal::Invalid => invalid_params(),
        Refusal::NotANode => "Object id doesn't reference a Node".into(),
        Refusal::Detached => "Node is detached from document".into(),
        Refusal::NoLayout => "Node does not have a layout object".into(),
        Refusal::Unnamed(words) => words.into(),
        Refusal::Fault(message) => Failure::from(message),
    }
}

/// Run `body` on the node a `DOM.*` command names. `guards` is 0 for none,
/// 1 to refuse a node that left the document, 2 to refuse one nothing
/// displays as well. A `nodeId` is looked up first, then a `backendNodeId`,
/// then an `objectId` — where `by_handle`; the handle is `this` inside the
/// body, and when a number came with it the number alone names the node.
/// What the body returns comes back by value — as a handle when `as_handle`
/// — and it starts with `return` whenever it refuses.
async fn on_node(
    page: &mut Page,
    params: &Value,
    guards: u8,
    by_handle: bool,
    as_handle: bool,
    body: &str,
    unnamed: &'static str,
) -> Result<EvalOutcome, Refusal> {
    // Every field is read first, as Chrome reads them; `objectId` is typed
    // only for a command that can name a node by it.
    let mut handle = if by_handle {
        optional_string(params, "objectId")
            .map_err(|_| Refusal::Invalid)?
            .map(str::to_string)
    } else {
        None
    };
    let number = optional_integer(params, "backendNodeId").map_err(|_| Refusal::Invalid)?;
    // A `nodeId` is looked up first, as in Chrome — and is never found.
    if optional_integer(params, "nodeId")
        .map_err(|_| Refusal::Invalid)?
        .is_some()
    {
        return Err(Refusal::NoFrontendNode);
    }
    // A number is looked up next, and is what names the node when a handle
    // came with it too: the handle is not asked about then.
    let number = match number {
        // Chrome looks 0 up too, and finds nothing.
        Some(0) => return Err(Refusal::NoNumber),
        Some(number) => {
            handle = None;
            Some(number)
        }
        None if handle.is_some() => None,
        None => return Err(Refusal::Unnamed(unnamed)),
    };
    let js = page
        .inner
        .js
        .as_mut()
        .ok_or_else(|| Refusal::Fault("page has no JS runtime".to_string()))?;
    if let Some(dead) = handle
        .as_deref()
        .and_then(|id| dead_handle(objects::state(js, id)))
    {
        return Err(Refusal::Dead(dead));
    }
    // The number the hub reserved, for the one command that hands numbers
    // out; nothing for the rest.
    let fresh = params.get("__ditingNodeId").cloned().unwrap_or(Value::Null);
    let declaration = format!(
        "function(__id, __guards, __fresh) {{\
            var dom = ({NODE_JS})(__fresh);\
            var node = __id == null ? this : dom.byId(__id);\
            if (!node) return {{ {VERDICT}: 'noNumber' }};\
            if (!(node instanceof Node)) return {{ {VERDICT}: 'notANode' }};\
            if (__guards > 0 && !dom.attached(node)) return {{ {VERDICT}: 'detached' }};\
            if (__guards > 1 && !dom.laidOut(node)) return {{ {VERDICT}: 'noLayout' }};\
            {body}\
        }}"
    );
    // A handle is `this`; a number is looked up. The guards ride along
    // either way — as CDP `CallArgument`s, which is what the runtime reads.
    let arguments = vec![
        json!({ "value": number }),
        json!({ "value": guards }),
        json!({ "value": fresh }),
    ];
    let outcome = js
        .call_function_on_remote_outcome(
            &declaration,
            handle.as_deref(),
            &arguments,
            // Chrome's shape: the node comes back as a handle only from
            // `DOM.resolveNode`; everything else is read off it by value.
            !as_handle,
            false,
            None,
        )
        .await
        .map_err(Refusal::Fault)?;
    if as_handle {
        return match outcome.exception.as_ref() {
            Some(_) => Err(Refusal::Fault(
                "DOM.resolveNode: the node did not come back".into(),
            )),
            None => Ok(outcome),
        };
    }
    match value_of(&outcome)?.get(VERDICT).and_then(Value::as_str) {
        Some("noNumber") => Err(Refusal::NoNumber),
        Some("notANode") => Err(Refusal::NotANode),
        Some("detached") => Err(Refusal::Detached),
        Some("noLayout") => Err(Refusal::NoLayout),
        _ => Ok(outcome),
    }
}

/// What a by-value script returned.
fn value_of(outcome: &EvalOutcome) -> Result<Value, Refusal> {
    if let Some(exception) = &outcome.exception {
        return Err(Refusal::Fault(format!(
            "the DOM script threw: {}",
            exception.description
        )));
    }
    Ok(outcome.info.value.clone().unwrap_or(Value::Null))
}

async fn describe_node(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    kinds(params, &[("depth", Kind::Integer), ("pierce", Kind::Bool)])?;
    // The hub names the frame the session is on; the page has one.
    let frame = serde_json::to_string(params.get("__ditingFrameId").unwrap_or(&Value::Null))
        .unwrap_or_else(|_| "null".to_string());
    let outcome = on_node(
        page,
        params,
        0,
        true,
        false,
        &format!("return dom.describe(node, {frame}, __fresh);"),
        EITHER,
    )
    .await
    .map_err(words)?;
    let node = value_of(&outcome).map_err(words)?;
    Ok(json!({ "node": node }))
}

/// `DOM.resolveNode` words a number it does not know its own way — and a
/// `nodeId`, the frontend's, by whether a number came beside it, as Chrome
/// does.
fn resolve_words(refusal: Refusal, numbered: bool) -> Failure {
    match (refusal, numbered) {
        (Refusal::NoNumber, _) | (Refusal::NoFrontendNode, false) => NO_NODE.into(),
        (Refusal::NoFrontendNode, true) => UNNAMED.into(),
        (other, _) => words(other),
    }
}

/// `DOM.resolveNode`: the node as a handle. Two scripts — one to read what
/// the handle should say of it, one to hand it back. A number names the
/// node here, never a handle: an `objectId` is not read, as Chrome refuses
/// one outright. The node has to be of the document of the context it is
/// asked for.
const UNNAMED: &str = "Either nodeId or backendNodeId must be specified.";

async fn resolve_node(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    kinds(
        params,
        &[
            ("objectGroup", Kind::Text),
            ("executionContextId", Kind::Integer),
        ],
    )?;
    let numbered = params.get("backendNodeId").is_some();
    const FOUND: &str = "return { className: (node.constructor && node.constructor.name) \
                         || node.nodeName, label: dom.label(node) };";
    let found = on_node(page, params, 0, false, false, FOUND, UNNAMED)
        .await
        .map_err(|refusal| resolve_words(refusal, numbered))?;
    let found = value_of(&found).map_err(words)?;
    if let Some(context) = params.get("executionContextId").and_then(Value::as_i64) {
        if !context_is_live(params, context) {
            return Err("Node with given id does not belong to the document".into());
        }
    }
    let held = on_node(page, params, 0, false, true, "return node;", UNNAMED)
        .await
        .map_err(|refusal| resolve_words(refusal, numbered))?;
    let object_id = held
        .info
        .object_id
        .ok_or("DOM.resolveNode: the node did not come back")?;
    Ok(json!({ "object": {
        "type": "object",
        "subtype": "node",
        "className": found["className"],
        "description": found["label"],
        "objectId": object_id,
    } }))
}

async fn box_model(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    let outcome = match on_node(
        page,
        params,
        2,
        true,
        false,
        "return dom.box(node);",
        EITHER,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(Refusal::Detached | Refusal::NoLayout) => return Err(NO_BOX.into()),
        Err(other) => return Err(words(other)),
    };
    let model = value_of(&outcome).map_err(words)?;
    Ok(json!({ "model": model }))
}

/// A node Chrome cannot lay out is left out, not an error: `{"quads":[]}`.
/// A node it cannot find is an error, as everywhere else.
async fn content_quads(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    const BODY: &str = "\
        if (!dom.laidOut(node)) return [];\
        var r = node.getBoundingClientRect();\
        if (r.width <= 0 || r.height <= 0) return [];\
        return [[r.left, r.top, r.right, r.top, r.right, r.bottom, r.left, r.bottom]];";
    let outcome = match on_node(page, params, 0, true, false, BODY, EITHER).await {
        Ok(outcome) => outcome,
        Err(Refusal::Detached | Refusal::NoLayout) => return Ok(json!({ "quads": [] })),
        Err(other) => return Err(words(other)),
    };
    let quads = value_of(&outcome).map_err(words)?;
    let quads = quads.as_array().cloned().unwrap_or_default();
    Ok(json!({ "quads": quads }))
}

/// `nearest` is a no-op when the box already intersects the viewport, so
/// an on-screen click does not move scroll. A sub-rect (a position click
/// inside a box taller than the viewport) is nudged in afterwards.
async fn scroll_into_view(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    // `rect`: all four sides have to be sent.
    let rect = match params.get("rect") {
        None => Value::Null,
        Some(rect) => {
            for side in ["x", "y", "width", "height"] {
                integer(rect, side)?;
            }
            rect.clone()
        }
    };
    let rect = serde_json::to_string(&rect).unwrap_or_else(|_| "null".to_string());
    let body = format!(
        "\
        if (!dom.laidOut(node)) return {{ {VERDICT}: 'noLayout' }};\
        if (node.nodeType === 1) node.scrollIntoView({{block:'nearest', inline:'nearest'}});\
        var rect = {rect};\
        if (!rect || typeof rect !== 'object' || node.nodeType !== 1) return 1;\
        var box = node.getBoundingClientRect();\
        var x = box.x + (+rect.x || 0);\
        var y = box.y + (+rect.y || 0);\
        var w = rect.width == null ? box.width : +rect.width;\
        var h = rect.height == null ? box.height : +rect.height;\
        var vw = window.innerWidth || 0, vh = window.innerHeight || 0;\
        var dx = x < 0 ? x : (x + w > vw ? x + w - vw : 0);\
        var dy = y < 0 ? y : (y + h > vh ? y + h - vh : 0);\
        if (!dx && !dy) return 1;\
        var root = document.scrollingElement || document.documentElement;\
        if (!root) return 1;\
        if (dx) root.scrollLeft = (root.scrollLeft || 0) + dx;\
        if (dy) root.scrollTop = (root.scrollTop || 0) + dy;\
        return 1;"
    );
    let outcome = on_node(page, params, 1, true, false, &body, EITHER)
        .await
        .map_err(words)?;
    value_of(&outcome).map_err(words)?;
    Ok(json!({}))
}

fn dispatch_input(page: &mut Page, script: &str) -> Result<Value, Failure> {
    page.evaluate_with_timeout_result(INPUT_HELPERS, INTERACTION_EVAL_TIMEOUT)
        .map_err(|err| format!("input helpers: {err}"))?;
    page.evaluate_with_timeout_result(script, INTERACTION_EVAL_TIMEOUT)
        .map_err(|err| format!("input: {err}"))?;
    Ok(json!({}))
}

/// The reply to an evaluate or a call: what came back — by value where that
/// was asked for — and, when something was thrown, what it was, beside it.
fn settled(
    js: &mut JsRuntime,
    mut outcome: EvalOutcome,
    return_by_value: bool,
) -> Result<Value, Failure> {
    // What was thrown, the way a result shows it: taken before a value
    // asked for is read off its slot.
    let thrown = outcome
        .exception
        .as_ref()
        .map(|_| remote_object(&outcome.info));
    if return_by_value {
        serialize_by_value(js, &mut outcome)?;
    }
    Ok(outcome_result(outcome, thrown))
}

/// Chrome sends the exception as `result` too, beside `exceptionDetails` —
/// "for compatibility reasons, even though it's accessible through
/// exceptionDetails.exception" (its inspector's own words). So does the
/// bridge: a reply never has the one without the other.
fn outcome_result(outcome: EvalOutcome, thrown: Option<Value>) -> Value {
    let mut reply = serde_json::Map::new();
    reply.insert("result".into(), remote_object(&outcome.info));
    if let (Some(exception), Some(thrown)) = (outcome.exception, thrown) {
        reply.insert(
            "exceptionDetails".into(),
            exception_details(&exception, thrown),
        );
    }
    Value::Object(reply)
}

static EXCEPTION_IDS: AtomicU64 = AtomicU64::new(0);

/// `exceptionDetails` (Chrome 158, measured): the thrown value as a result
/// is shown; the position is 0-based. What a promise was rejected with has
/// none — 0, 0 and no script — and its text carries the message of an
/// Error and of nothing else.
///
/// Where this engine differs: the position is where the Error was made —
/// the innermost frame of its stack — where Chrome's is where it was
/// thrown (the two are one for a fault of the engine's own, such as reading
/// a property of `null`). With no frame to go by there is none (`-1`):
/// Chrome has the `throw` there, and the parser's position for a script
/// that does not parse. `scriptId` is always `"0"`: the engine numbers no
/// scripts.
fn exception_details(exception: &ExceptionInfo, thrown: Value) -> Value {
    let mut details = json!({
        "exceptionId": EXCEPTION_IDS.fetch_add(1, Ordering::Relaxed) + 1,
        "text": exception.text(),
        "lineNumber": 0,
        "columnNumber": 0,
    });
    if exception.in_promise {
        if exception.is_error {
            details["text"] = format!("{} {}", exception.text(), exception.description).into();
        }
    } else {
        details["lineNumber"] = json!(-1);
        details["columnNumber"] = json!(-1);
        if let Some(frame) = exception.own_frame() {
            details["lineNumber"] = json!(frame.line - 1);
            details["columnNumber"] = json!(frame.column - 1);
        }
        details["scriptId"] = json!("0");
    }
    details["exception"] = thrown;
    details
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
    if let Some(value) = &info.unserializable_value {
        obj.insert("unserializableValue".into(), json!(value));
    }
    if let Some(value) = &info.value {
        obj.insert("value".into(), value.clone());
    }
    Value::Object(obj)
}
