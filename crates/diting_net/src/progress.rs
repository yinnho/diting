//! "The response has started" signal for callers that bound a fetch by wall
//! clock. A total-duration cap cannot tell a dead server from a live but slow
//! body; the transport's own timeouts can (connect 10s, per-chunk read stall
//! 30s, #49), so a caller that wants to keep waiting on live transfers only
//! needs to know whether the response headers came back. Scoped per future
//! with a task-local, so no transport signature changes: the script loader
//! wraps each fetch in [`track_response_start`]; both transports call
//! [`mark_response_started`] when headers arrive.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

tokio::task_local! {
    static RESPONSE_STARTED: Arc<AtomicBool>;
}

/// Run `fut` with `flag` set to true once a response (headers) arrives.
pub async fn track_response_start<F: std::future::Future>(
    flag: Arc<AtomicBool>,
    fut: F,
) -> F::Output {
    RESPONSE_STARTED.scope(flag, fut).await
}

/// Called by the transports right after the response headers arrive; a no-op
/// outside [`track_response_start`].
pub(crate) fn mark_response_started() {
    let _ = RESPONSE_STARTED.try_with(|f| f.store(true, Ordering::Relaxed));
}
