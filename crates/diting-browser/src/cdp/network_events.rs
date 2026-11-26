//! The page's requests as `Network.*` events. The engine files a request
//! as one finished row; Chrome tells it in steps, so each row is told as
//! `requestWillBeSent`, then `responseReceived` and `loadingFinished` —
//! or `loadingFailed` when no response came — to every session that has
//! `Network` on. Times are the row's own, not live ones, and a redirect
//! is told by where it landed, not hop by hop.

use diting::diting_browser::page::NetworkEvent;
use serde_json::{json, Map, Value};

use super::{loader_of, to_sessions, Hub, Out, PageSlot};

/// Chrome's resource types; what the engine calls anything else is `Other`.
const TYPES: &[&str] = &[
    "Document",
    "Stylesheet",
    "Image",
    "Media",
    "Font",
    "Script",
    "TextTrack",
    "XHR",
    "Fetch",
    "Prefetch",
    "EventSource",
    "WebSocket",
    "Manifest",
    "SignedExchange",
    "Ping",
    "CSPViolationReport",
    "Preflight",
    "Other",
];

fn resource_type(row: &NetworkEvent) -> &str {
    TYPES
        .iter()
        .find(|known| **known == row.resource_type)
        .copied()
        .unwrap_or("Other")
}

fn headers(map: &std::collections::HashMap<String, String>) -> Value {
    Value::Object(
        map.iter()
            .map(|(name, value)| (name.clone(), json!(value)))
            .collect::<Map<_, _>>(),
    )
}

/// The events for `rows`, written for the sessions of `page` that have
/// `Network` on. `local` is the socket a command came in on, or 0 for
/// none.
pub(super) fn network_frames(
    hub: &Hub,
    local: u64,
    page: &PageSlot,
    rows: &[NetworkEvent],
    out: &mut Out,
) {
    let target = page.target_id.as_str();
    let loader = loader_of(page);
    for row in rows {
        let kind = resource_type(row);
        let started = row.started.unwrap_or(row.timestamp);
        let mut send = |method: &str, params: Value| {
            to_sessions(
                hub,
                out,
                local,
                target,
                |slot| slot.network_on,
                method,
                || params.clone(),
            );
        };
        send(
            "Network.requestWillBeSent",
            json!({
                "requestId": row.request_id,
                "loaderId": loader,
                "documentURL": page.url,
                "request": {
                    "url": row.url,
                    "method": row.method,
                    "headers": headers(&row.headers),
                    "initialPriority": "High",
                    "referrerPolicy": "strict-origin-when-cross-origin",
                },
                "timestamp": started,
                "wallTime": started,
                "initiator": { "type": "other" },
                "redirectHasExtraInfo": false,
                "type": kind,
                "frameId": target,
                "hasUserGesture": false,
            }),
        );
        if row.error.is_some() || row.status == 0 {
            send(
                "Network.loadingFailed",
                json!({
                    "requestId": row.request_id,
                    "timestamp": row.timestamp,
                    "type": kind,
                    "errorText": row.error.as_deref().unwrap_or("net::ERR_FAILED"),
                    "canceled": false,
                }),
            );
            continue;
        }
        let landed = if row.final_url.is_empty() {
            &row.url
        } else {
            &row.final_url
        };
        let mime = row
            .response_headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.split(';').next().unwrap_or("").trim().to_string())
            .unwrap_or_default();
        send(
            "Network.responseReceived",
            json!({
                "requestId": row.request_id,
                "loaderId": loader,
                "timestamp": row.timestamp,
                "type": kind,
                "response": {
                    "url": landed,
                    "status": row.status,
                    "statusText": "",
                    "headers": headers(&row.response_headers),
                    "mimeType": mime,
                    "connectionReused": false,
                    "connectionId": 0,
                    "encodedDataLength": row.body_size,
                    "securityState": "unknown",
                },
                "hasExtraInfo": false,
                "frameId": target,
            }),
        );
        send(
            "Network.loadingFinished",
            json!({
                "requestId": row.request_id,
                "timestamp": row.timestamp,
                "encodedDataLength": row.body_size,
            }),
        );
    }
}
