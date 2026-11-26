//! `SessionCommand::EvalRemote` on the session thread — the
//! remote-object evaluate the CDP `Runtime.*` face needs. Split from
//! actor.rs for the god-file cap; the by-value settle is the engine half
//! of what the face's `serialize_by_value` does.

use diting::diting_js::runtime::{
    EvalInput, EvalOutcome, JsRuntime, ObjectIdState, RemoteObjectInfo, BAD_CALL_ARGUMENT,
};
use serde_json::Value;
use tokio::sync::oneshot;

use super::super::super::commands::{CallOnRequest, HandleParts, RemoteReply};
use super::super::super::state::drain_console;
use super::SessionActor;

impl SessionActor {
    /// Same guards as `on_eval`, the outcome itself on the reply, by-value
    /// already read where asked.
    pub(super) async fn on_eval_remote(
        &mut self,
        script: String,
        await_promise: bool,
        return_by_value: bool,
        timeout_ms: Option<u64>,
        reply: RemoteReply,
    ) {
        let Self {
            page,
            last_nav_error,
            console_ring,
            ..
        } = self;
        let outcome = if !page.inner.has_js() {
            Err(format!(
                "page has no JS runtime — the last navigation did not \
                                 complete{}; navigate again",
                last_nav_error
                    .as_deref()
                    .map(|nav| format!(" ({})", nav))
                    .unwrap_or_default()
            ))
        } else {
            let Some(js) = page.inner.js.as_mut() else {
                let _ = reply.send(Err("page has no JS runtime".to_string()));
                return;
            };
            match js
                .evaluate_remote_outcome(
                    &script,
                    false,
                    await_promise,
                    timeout_ms.unwrap_or(30_000),
                    None,
                    EvalInput::Script,
                )
                .await
            {
                Err(err) => Err(err),
                Ok(mut outcome) => {
                    // The face shows a thrown value as a handle even when
                    // the result was asked for by value — keep the info as
                    // it was before the by-value read.
                    let pre_info = (return_by_value && outcome.exception.is_some())
                        .then(|| outcome.info.clone());
                    if return_by_value {
                        settle_by_value(js, &mut outcome).map(|()| (outcome, pre_info))
                    } else {
                        Ok((outcome, None))
                    }
                }
            }
        };
        // The page-side drain a CDP command owes its face — the same two
        // queues `cdp::runtime::dispatch` empties for the Cdp channel.
        let outcome = outcome.map(|(outcome, pre)| {
            (
                outcome,
                pre,
                drain_console(page, console_ring),
                page.inner.take_pending_write_nav(),
            )
        });
        let _ = reply.send(outcome);
        // A pushState the evaluation just ran is the page's move already —
        // adopt it with the command, as Chrome reports it in the same task,
        // instead of a pump slice later (measured 1-2.5s late, past what a
        // client's post-evaluate check waits).
        if !self.console_subs.is_empty()
            && !self.page.inner.has_pending_history_jump()
            && !self.page.inner.has_pending_navigation()
        {
            self.follow_same_document_move();
        }
    }

    /// `SessionCommand::CallOn`: `Runtime.callFunctionOn` — Chrome's
    /// dead-handle order on the session thread (the face already read
    /// each wire id into [`HandleParts`]), the unserializableValue probe,
    /// the call itself, then the same by-value settle `on_eval_remote`
    /// does.
    pub(super) async fn on_call_on(&mut self, args: CallOnRequest, reply: RemoteReply) {
        let CallOnRequest {
            declaration,
            object_id,
            arguments,
            by_value,
            await_promise,
            return_by_value,
            timeout_ms,
            receiver,
            arg_ids,
        } = args;
        let Self {
            page, console_ring, ..
        } = self;
        let outcome = if !page.inner.has_js() {
            Err("page has no JS runtime".to_string())
        } else {
            let Some(js) = page.inner.js.as_mut() else {
                let _ = reply.send(Err("page has no JS runtime".to_string()));
                return;
            };
            // The receiver is asked about first, as Chrome does, then
            // each argument in order.
            if let Some(dead) = receiver
                .as_ref()
                .and_then(|parts| dead_handle_word(handle_state_of(parts, js)))
            {
                Err(dead.to_string())
            } else {
                let mut dead: Option<String> = None;
                for (argument, parts) in arguments.iter().zip(arg_ids.iter()) {
                    if let Some(parts) = parts {
                        if let Some(word) = dead_argument_word(handle_state_of(parts, js)) {
                            dead = Some(word.to_string());
                            break;
                        }
                    } else if argument.get("value").is_none() {
                        if let Some(text) =
                            argument.get("unserializableValue").and_then(Value::as_str)
                        {
                            let outcome = js
                                .evaluate_remote_outcome(
                                    text,
                                    false,
                                    false,
                                    timeout_ms.unwrap_or(30_000),
                                    None,
                                    EvalInput::Script,
                                )
                                .await;
                            let bad = match outcome {
                                Err(_) => true,
                                Ok(outcome) => outcome.exception.is_some(),
                            };
                            if bad {
                                dead = Some(BAD_CALL_ARGUMENT.to_string());
                                break;
                            }
                        }
                    }
                }
                if let Some(dead) = dead {
                    Err(dead)
                } else {
                    match js
                        .call_function_on_remote_outcome(
                            &declaration,
                            object_id.as_deref(),
                            &arguments,
                            by_value,
                            await_promise,
                            None,
                        )
                        .await
                    {
                        Err(err) => Err(err),
                        Ok(mut outcome) => {
                            let pre_info = (return_by_value && outcome.exception.is_some())
                                .then(|| outcome.info.clone());
                            if return_by_value {
                                settle_by_value(js, &mut outcome).map(|()| (outcome, pre_info))
                            } else {
                                Ok((outcome, None))
                            }
                        }
                    }
                }
            }
        };
        // The same page-side drain `on_eval_remote` owes the face.
        let outcome = outcome.map(|(outcome, pre)| {
            (
                outcome,
                pre,
                drain_console(page, console_ring),
                page.inner.take_pending_write_nav(),
            )
        });
        let _ = reply.send(outcome);
        // A pushState the evaluation just ran is the page's move already —
        // adopt it with the command, as Chrome reports it in the same task,
        // instead of a pump slice later (measured 1-2.5s late, past what a
        // client's post-evaluate check waits).
        if !self.console_subs.is_empty()
            && !self.page.inner.has_pending_history_jump()
            && !self.page.inner.has_pending_navigation()
        {
            self.follow_same_document_move();
        }
    }
}

/// What the face's [`HandleParts`] make of an id, once the engine is
/// asked — the same order `cdp::objects::state` reads them in: no parts
/// at all is no id, a realm that is not this runtime's is gone whatever
/// its number says, an empty number is released, a non-canonical
/// spelling is no id, and only then the engine's own answer.
fn handle_state_of(parts: &HandleParts, js: &mut JsRuntime) -> ObjectIdState {
    match parts {
        HandleParts::NoId => ObjectIdState::Unparseable,
        HandleParts::Parts { realm: None, .. } => ObjectIdState::Gone,
        HandleParts::Parts {
            realm: Some(realm), ..
        } if *realm != js.realm_id() => ObjectIdState::Gone,
        HandleParts::Parts { number: None, .. } => ObjectIdState::Released,
        HandleParts::Parts {
            canonical: false, ..
        } => ObjectIdState::Unparseable,
        HandleParts::Parts {
            realm: Some(realm),
            number: Some(number),
            ..
        } => js.object_state_of(*realm, *number),
    }
}

/// What Chrome says of an `objectId` that names nothing it will run
/// against — the same three words `cdp::runtime::dead_handle` gives,
/// repeated here until the Cdp channel is gone and they live only here.
fn dead_handle_word(state: ObjectIdState) -> Option<&'static str> {
    match state {
        ObjectIdState::Held => None,
        ObjectIdState::Unparseable => Some("Invalid remote object id"),
        ObjectIdState::Gone => Some("Cannot find context with specified id"),
        ObjectIdState::Released => Some("Could not find object with given id"),
    }
}

/// The same for an `objectId` among the arguments, where Chrome words an
/// id of a world that is not the receiver's its own way.
fn dead_argument_word(state: ObjectIdState) -> Option<&'static str> {
    match state {
        ObjectIdState::Gone => {
            Some("Argument should belong to the same JavaScript world as target object")
        }
        state => dead_handle_word(state),
    }
}

impl SessionActor {
    /// `SessionCommand::ReleaseObject`: a handle the page does not hold
    /// is refused, as in Chrome — nothing was there to let go.
    pub(super) async fn on_release_object(
        &mut self,
        object_id: String,
        parts: HandleParts,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        let Some(js) = self.page.inner.js.as_mut() else {
            let _ = reply.send(Err("page has no JS runtime".to_string()));
            return;
        };
        let state = handle_state_of(&parts, js);
        let _ = reply.send(match dead_handle_word(state) {
            Some(dead) => Err(dead.to_string()),
            None => {
                js.release_object(&object_id);
                Ok(())
            }
        });
    }
}

/// `returnByValue` — the engine half of what the CDP face's
/// `serialize_by_value` does: read the held object by value, let the
/// handle go unless the exception still names its slot, and leave the
/// outcome carrying the value instead of the handle. Errors stay
/// strings; the face maps them to protocol words.
fn settle_by_value(js: &mut JsRuntime, outcome: &mut EvalOutcome) -> Result<(), String> {
    let rejected = match &outcome.exception {
        Some(exception) if !exception.in_promise => return Ok(()),
        Some(_) => true,
        None => false,
    };
    let info = &mut outcome.info;
    match info.js_type.as_str() {
        "object" | "function" if info.subtype.as_deref() != Some("null") => {}
        "symbol" => {
            if rejected {
                if let Some(id) = &info.object_id {
                    js.release_object(id);
                }
            }
            return Err("Object couldn't be returned by value".to_string());
        }
        _ => return Ok(()),
    }
    let Some(id) = info.object_id.clone() else {
        return Ok(());
    };
    let read = js.value_of(&id);
    if !rejected || read.is_err() {
        js.release_object(&id);
    }
    let value = read?;
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
