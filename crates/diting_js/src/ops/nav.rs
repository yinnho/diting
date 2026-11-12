// 导航与宿主回调：navigate/history 跳转 + binding_called 上抛 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

#[diting_op]
pub(super) fn op_navigate(state: &OpState, #[string] url: &str, #[string] method: &str, #[string] body: &str) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    // Only queue the navigation — do NOT move the realm URL here. The URL is
    // written on commit (init_js → set_url from the page's committed URL).
    // Moving it early let synchronous JS between the assignment and the
    // actual navigation read and write another origin's cookies through
    // document.cookie, whose ops derive the domain from this URL
    // (SOP bypass, obscura #940).
    gs.pending_navigation = Some((url.to_string(), method.to_string(), body.to_string()));
}

/// Queue a session-history jump (history.back/forward/go across a document
/// boundary — obscura#1105). `target` is an absolute index into the page's
/// real session history; the pump (`Page::process_pending_navigation`) moves
/// the cursor and refetches the entry's document.
#[diting_op]
pub(super) fn op_history_jump_to(state: &OpState, target: i32) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.pending_history_jump = Some(target);
}

// Records a binding call from page JS. The CDP layer drains this queue
// after every dispatch and emits one `Runtime.bindingCalled` event per
// entry, that's how puppeteer's `page.exposeFunction` callbacks fire.
#[diting_op]
pub(super) fn op_binding_called(state: &OpState, #[string] name: &str, #[string] payload: &str) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.pending_binding_calls
        .push((name.to_string(), payload.to_string()));
    // The CDP layer drains this after every dispatch, but a page can loop
    // calling an exposed binding many times inside one long evaluate — same
    // unbounded-queue shape as js_network_events (obscura #705 class).
    const MAX_PENDING_BINDING_CALLS: usize = 4096;
    if gs.pending_binding_calls.len() > MAX_PENDING_BINDING_CALLS {
        let overflow = gs.pending_binding_calls.len() - MAX_PENDING_BINDING_CALLS;
        gs.pending_binding_calls.drain(0..overflow);
    }
}

