//! Events a page raises on its own — a `setTimeout` that logs between two
//! commands. A command's reply carries the console calls it made; these
//! are the ones nothing answered, pushed from the session thread as it
//! drains them ([`SessionEvent`]) and written to every socket that asked
//! for them.

use serde_json::json;
use tokio::sync::mpsc;

use crate::session::{send_command, SessionCommand, SessionEvent};

use super::{page_side_frames, runtime, Out, SharedHub};

/// Follow `engine`'s events and write them as CDP events for the page
/// `target`. Best effort: a session that cannot be subscribed to simply
/// pushes nothing, and the task ends with the session.
pub(super) async fn follow(hub: SharedHub, target: String, engine: &str) {
    let (sink, mut events) = mpsc::unbounded_channel();
    if send_command(engine, |reply| SessionCommand::Subscribe { sink, reply })
        .await
        .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            let SessionEvent::Console { level, text, url } = event;
            let Some(kind) = runtime::console_type(&level) else {
                continue;
            };
            let side = json!({
                "console": [{ "type": kind, "text": text, "url": url }],
                "writeNav": false,
            });
            let mut out = Out::default();
            {
                let hub = hub.lock().await;
                // The page may not be filed yet, or be gone: no sockets
                // to tell either way.
                let Some(page) = hub.pages.get(&target) else {
                    continue;
                };
                // No socket has the id 0, so every frame goes to an outbox.
                page_side_frames(&hub, 0, page, &side, &mut out);
            }
            for (outbox, frame) in out.spill {
                let _ = outbox.send(frame);
            }
        }
    });
}
