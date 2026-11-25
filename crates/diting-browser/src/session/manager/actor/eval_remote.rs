//! `SessionCommand::EvalRemote` on the session thread — the
//! remote-object evaluate the CDP `Runtime.*` face needs. Split from
//! actor.rs for the god-file cap; the by-value settle is the engine half
//! of what the face's `serialize_by_value` does.

use diting::diting_js::runtime::{EvalInput, EvalOutcome, JsRuntime, RemoteObjectInfo};
use tokio::sync::oneshot;

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
        reply: oneshot::Sender<
            Result<
                (
                    EvalOutcome,
                    Option<RemoteObjectInfo>,
                    Vec<(String, String, String)>,
                    bool,
                ),
                String,
            >,
        >,
    ) {
        let Self {
            page,
            last_nav_error,
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
                page.inner.take_pending_console_calls(),
                page.inner.take_pending_write_nav(),
            )
        });
        let _ = reply.send(outcome);
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
