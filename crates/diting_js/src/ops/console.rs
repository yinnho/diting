// console 消息上抛 CDP + alert/confirm/prompt 对话框 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

#[diting_op]
pub(super) fn op_console_msg(state: &OpState, #[string] level: &str, #[string] msg: &str) {
    match level {
        "warn" => tracing::warn!(target: "diting::console", "{}", msg),
        "error" => tracing::error!(target: "diting::console", "{}", msg),
        _ => tracing::info!(target: "diting::console", "{}", msg),
    }
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    // Queue-time URL: the consumer may drain long after a navigation moved
    // the page elsewhere, and per-entry URLs make session_console's
    // url_contains filter truthful.
    let log_url = gs.url.clone();
    gs.pending_console_calls
        .push((level.to_string(), msg.to_string(), log_url));
}

/// window.alert/confirm/prompt land here from the bootstrap stubs. There is
/// no UI to attach a dialog to, so blocking is not an option (the thread
/// that would show the dialog is the same one running the page script);
/// instead each call is answered from the session-side policy —
/// `dialog_accept` (default false = dismiss) plus `dialog_prompt_text` for
/// prompt — and recorded as a level-"dialog" console entry so
/// session_console shows what the page asked. Returns
/// `{"accept":bool,"value":string|null}`; value is only meaningful for
/// prompt, where the JS wrapper falls back to the call's default argument.
#[diting_op]
#[string]
pub(super) fn op_dialog(state: &OpState, #[string] kind: &str, #[string] message: &str) -> String {
    let shared = state.borrow::<SharedState>().clone();
    let mut gs = shared.borrow_mut();
    let (accept, value) = match kind {
        "confirm" => (gs.dialog_accept, None),
        "prompt" => (gs.dialog_accept, gs.dialog_prompt_text.clone()),
        // alert: nothing to answer, recorded for observability only.
        _ => (true, None),
    };
    let dialog_url = gs.url.clone();
    let mut payload = serde_json::json!({ "dialog": kind, "message": message });
    if kind == "confirm" || kind == "prompt" {
        payload["answer"] = serde_json::Value::Bool(accept);
        if kind == "prompt" {
            payload["value"] = value.clone().into();
        }
    }
    gs.pending_console_calls
        .push(("dialog".to_string(), payload.to_string(), dialog_url));
    serde_json::json!({ "accept": accept, "value": value }).to_string()
}
