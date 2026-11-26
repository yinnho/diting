//! Events a page raises on its own — a `setTimeout` that logs between two
//! commands. A command's reply carries the console calls it made; these
//! are the ones nothing answered, pushed from the session thread as it
//! drains them ([`SessionEvent`]) and written to every socket that asked
//! for them.

use serde_json::json;
use tokio::sync::mpsc;

use crate::session::{send_command, SessionCommand, SessionEvent};

use super::network_events::network_frames;
use super::{page_side, runtime, target, Out, SharedHub};

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
            let mut out = Out::default();
            {
                let mut hub = hub.lock().await;
                // The page may not be filed yet, or be gone: no sockets
                // to tell either way.
                if !hub.pages.contains_key(&target) {
                    continue;
                }
                // No socket has the id 0, so every frame goes to an outbox.
                match event {
                    SessionEvent::Navigated(nav) => {
                        // The same order a `Page.navigate` answers in: the
                        // new loader first, then the requests it carried,
                        // then the navigation.
                        if let Some(page) = hub.pages.get_mut(&target) {
                            page.loader += 1;
                            if let Some((url, title)) = &nav.landed {
                                page.url = url.clone();
                                page.title = title.clone();
                            }
                            super::remember_doc_rows(page, &nav.rows);
                            target::renew_worlds(page);
                        }
                        let page = &hub.pages[&target];
                        // A history step is served from where the browser kept
                        // the document (`histjump-probe.cjs`): nothing is requested.
                        if !nav.history {
                            network_frames(&hub, 0, page, &nav.rows, &mut out);
                        }
                        if nav.landed.is_some() {
                            target::nav_events(&hub, page, 0, &mut out);
                        }
                    }
                    SessionEvent::SameDocument { url, kind } => {
                        if let Some(page) = hub.pages.get_mut(&target) {
                            page.url = url.clone();
                        }
                        target::same_document_events(
                            &hub,
                            &hub.pages[&target],
                            &url,
                            kind,
                            &mut out,
                        );
                    }
                    SessionEvent::WriteNav => {
                        let side = json!({ "console": [], "writeNav": true });
                        page_side::page_side_frames(&hub, 0, &hub.pages[&target], &side, &mut out);
                    }
                    SessionEvent::Console { level, text, url } => {
                        let Some(kind) = runtime::console_type(&level) else {
                            continue;
                        };
                        let side = json!({
                            "console": [{ "type": kind, "text": text, "url": url }],
                            "writeNav": false,
                        });
                        page_side::page_side_frames(&hub, 0, &hub.pages[&target], &side, &mut out);
                    }
                    SessionEvent::Network(row) => {
                        network_frames(&hub, 0, &hub.pages[&target], &[*row], &mut out);
                    }
                }
            }
            for (outbox, frame) in out.spill {
                let _ = outbox.send(frame);
            }
        }
    });
}
