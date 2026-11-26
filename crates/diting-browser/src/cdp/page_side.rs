//! The frames a page's own activity is told in: console calls, whether
//! they rode a navigation's reply or were pushed as the session drained
//! them, and the lifecycle a `document.write` rewrite fires. Split from
//! cdp/mod.rs when it crossed the god-file cap; behavior unchanged.

use serde_json::{json, Value};

use super::{lifecycle, loader_of, now_secs, runtime, to_sessions, Hub, Out, PageSlot};

/// [`side_frames`](super::side_frames) for a page already in hand — what
/// the push task has, there being no session whose command it answers.
/// `Runtime.consoleAPICalled` for calls that rode a navigation's reply,
/// told inside the navigation's own batch — after the new document's
/// contexts are announced, so the id they name is one the client knows
/// (Playwright drops the call otherwise). Attributed to the main world:
/// a document's own scripts run there.
pub(super) fn navigation_console_frames(
    hub: &Hub,
    local: u64,
    page: &PageSlot,
    calls: &[(String, String, String)],
    out: &mut Out,
) {
    if calls.is_empty() {
        return;
    }
    let context_id = page
        .worlds
        .iter()
        .find(|world| world.name.is_none())
        .map(|world| world.id)
        .unwrap_or(1);
    let timestamp = now_secs();
    let target = page.target_id.as_str();
    for (level, text, _url) in calls {
        let Some(kind) = runtime::console_type(level) else {
            continue;
        };
        to_sessions(
            hub,
            out,
            local,
            target,
            |slot| slot.runtime_on,
            "Runtime.consoleAPICalled",
            || {
                json!({
                    "type": kind,
                    "args": [{ "type": "string", "value": text }],
                    "executionContextId": context_id,
                    "timestamp": timestamp,
                    "stackTrace": { "callFrames": [] },
                })
            },
        );
    }
}

pub(super) fn page_side_frames(
    hub: &Hub,
    local: u64,
    page: &PageSlot,
    side: &Value,
    out: &mut Out,
) {
    let console = side
        .get("console")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let write_nav = side
        .get("writeNav")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if console.is_empty() && !write_nav {
        return;
    }
    let asked = side
        .get("contextId")
        .and_then(|v| v.as_i64())
        .filter(|id| *id > 0);
    // The utility world is created after Playwright installs its console
    // listener. The main world may have been announced before that, so a
    // call with no context id is attributed to the latest isolated world.
    let context_id = asked.unwrap_or_else(|| {
        page.worlds
            .iter()
            .rev()
            .find(|world| world.name.is_some())
            .or_else(|| page.worlds.iter().find(|world| world.name.is_none()))
            .map(|world| world.id)
            .unwrap_or(1)
    });
    let timestamp = now_secs();
    let target = page.target_id.as_str();
    for call in console {
        let kind = call.get("type").and_then(|v| v.as_str()).unwrap_or("log");
        let text = call.get("text").and_then(|v| v.as_str()).unwrap_or("");
        to_sessions(
            hub,
            out,
            local,
            target,
            |slot| slot.runtime_on,
            "Runtime.consoleAPICalled",
            || {
                json!({
                    "type": kind,
                    "args": [{ "type": "string", "value": text }],
                    "executionContextId": context_id,
                    "timestamp": timestamp,
                    "stackTrace": { "callFrames": [] },
                })
            },
        );
    }
    if write_nav {
        let loader = loader_of(page);
        let event_of = |name: &str| lifecycle(&page.target_id, &loader, name, timestamp);
        to_sessions(
            hub,
            out,
            local,
            target,
            |slot| slot.lifecycle_on,
            "Page.lifecycleEvent",
            || event_of("DOMContentLoaded"),
        );
        to_sessions(
            hub,
            out,
            local,
            target,
            |slot| slot.page_on,
            "Page.domContentEventFired",
            || json!({ "timestamp": timestamp }),
        );
        to_sessions(
            hub,
            out,
            local,
            target,
            |slot| slot.lifecycle_on,
            "Page.lifecycleEvent",
            || event_of("load"),
        );
        to_sessions(
            hub,
            out,
            local,
            target,
            |slot| slot.page_on,
            "Page.loadEventFired",
            || json!({ "timestamp": timestamp }),
        );
    }
}
