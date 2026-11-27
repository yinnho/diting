//! The page's requests as `Network.*` events. The engine files a request
//! as one finished row; Chrome tells it in steps, so each row is told as
//! `requestWillBeSent`, then `responseReceived` and `loadingFinished` —
//! or `loadingFailed` when no response came — to every session that has
//! `Network` on. Times are the row's own, not live ones. A redirect chain
//! is told hop by hop, as Chrome does (`redir-probe.cjs`): one
//! `requestWillBeSent` per hop under one `requestId`, each hop after the
//! first carrying the response that sent it onward, and only the final
//! hop answering with `responseReceived`/`loadingFinished`.

use diting::diting_browser::page::NetworkEvent;
use diting::diting_net::NetFailure;
use serde_json::{json, Map, Value};

use super::params::string;
use super::{loader_of, page_of, session_msg, to_sessions, Failure, Hub, Out, PageSlot, SharedHub};
use crate::session::{send_command, SessionCommand};

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

/// How a request that got no response failed, in Chrome's words: the
/// engine's [`NetFailure`] for it, spelled the way DevTools spells it.
///
/// Probed on Chrome 158: a refused connection, a CORS refusal because the
/// allow-origin header is missing or names another origin, and a URL
/// blocked by `Network.setBlockedURLs` (no `errorText`, `blockedReason:
/// inspector`). `ERR_NAME_NOT_RESOLVED` is Chrome's name for a failed
/// lookup but was not probed: the sandbox answers every name. A timeout
/// is told apart by the engine but not probed (Chrome waits minutes), so it
/// says what everything unread says: `net::ERR_FAILED`. A connection closed
/// without a word, a reset, a body that ends short and a chunked body that
/// is cut were all probed (`reset-probe.cjs`). One caveat: the engine says
/// "empty response" for any connection that closes before a response
/// begins; Chrome's name for one that closes after *some* header bytes is
/// not probed.
#[derive(Debug, PartialEq)]
pub(super) struct Failed {
    pub error_text: String,
    pub blocked_reason: Option<&'static str>,
    pub cors: Option<(&'static str, String)>,
}

pub(super) fn failed(failure: &NetFailure) -> Failed {
    let plain = |text: &str| Failed {
        error_text: text.to_string(),
        blocked_reason: None,
        cors: None,
    };
    let cors = |kind: &'static str, parameter: String| Failed {
        error_text: "net::ERR_FAILED".to_string(),
        blocked_reason: None,
        cors: Some((kind, parameter)),
    };
    match failure {
        NetFailure::ConnectionRefused => plain("net::ERR_CONNECTION_REFUSED"),
        NetFailure::NameNotResolved => plain("net::ERR_NAME_NOT_RESOLVED"),
        NetFailure::UnsafePort => plain("net::ERR_UNSAFE_PORT"),
        NetFailure::EmptyResponse => plain("net::ERR_EMPTY_RESPONSE"),
        NetFailure::ConnectionReset => plain("net::ERR_CONNECTION_RESET"),
        NetFailure::ContentLengthMismatch => plain("net::ERR_CONTENT_LENGTH_MISMATCH"),
        NetFailure::IncompleteChunkedEncoding => plain("net::ERR_INCOMPLETE_CHUNKED_ENCODING"),
        NetFailure::BlockedByInspector => Failed {
            error_text: String::new(),
            blocked_reason: Some("inspector"),
            cors: None,
        },
        NetFailure::CorsMissingAllowOrigin => cors("MissingAllowOriginHeader", String::new()),
        NetFailure::CorsAllowOriginMismatch(allowed) => {
            cors("AllowOriginMismatch", allowed.clone())
        }
        NetFailure::TimedOut | NetFailure::Other => plain("net::ERR_FAILED"),
    }
}

/// A failed navigation's `errorText`: always a `net::ERR_*` name, as
/// Playwright's `page.goto` error carries it.
pub(super) fn navigation_error(failure: &NetFailure) -> String {
    match failed(failure).error_text {
        text if text.is_empty() => "net::ERR_FAILED".to_string(),
        text => text,
    }
}

/// Who asked: a script for what a script fetched, otherwise nothing the
/// engine tracks.
fn initiator(kind: &str) -> Value {
    match kind {
        "Fetch" | "XHR" => json!({ "type": "script", "stack": { "callFrames": [] } }),
        _ => json!({ "type": "other" }),
    }
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
        // A document's wire id is the loader's: `requestId == loaderId` is
        // how Chrome marks the request the navigation carries, and what
        // Playwright's `page.goto` looks for. Other rows keep the engine's
        // own id. The row behind a loader id is remembered by the caller
        // (`remember_doc_rows`) for `Network.getResponseBody`.
        let wire_id = if kind == "Document" {
            loader.as_str()
        } else {
            row.request_id.as_str()
        };
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
        // A hop of the chain: the first is the request as it went out,
        // each next one is what the previous hop's redirect sent for, and
        // carries that response. All share one `requestId`, as Chrome's do.
        let hops: Vec<(&str, Option<Value>)> = if row.redirects.is_empty() {
            vec![(row.url.as_str(), None)]
        } else {
            let mut hops = vec![(row.url.as_str(), None)];
            for (i, hop) in row.redirects.iter().enumerate() {
                let from = if i == 0 {
                    row.url.as_str()
                } else {
                    row.redirects[i - 1].url.as_str()
                };
                let redirect_response = json!({
                    "url": from,
                    "status": hop.status,
                    "statusText": "",
                    "headers": {},
                    "mimeType": "",
                    "fromDiskCache": false,
                    "fromServiceWorker": false,
                    "fromPrefetchCache": false,
                    "connectionReused": false,
                    "connectionId": 0,
                    "encodedDataLength": 0,
                    "securityState": "unknown",
                });
                hops.push((hop.url.as_str(), Some(redirect_response)));
            }
            hops
        };
        // A row the Fetch drainer already announced beside its pause must
        // not be told as a request twice — the pairing frame went out when
        // the request was still in flight, this one would only duplicate it.
        // The response frames below still go out: they carry what the pause
        // could not know.
        let already_announced = page.announced.contains(wire_id);
        for (hop_url, redirect_response) in hops {
            if already_announced {
                break;
            }
            let mut frame = json!({
                "requestId": wire_id,
                "loaderId": loader,
                "documentURL": page.url,
                "request": {
                    "url": hop_url,
                    "method": row.method,
                    "headers": headers(&row.headers),
                    "initialPriority": "High",
                    "referrerPolicy": "strict-origin-when-cross-origin",
                    "mixedContentType": "none",
                },
                "timestamp": started,
                "wallTime": started,
                "initiator": initiator(kind),
                "redirectHasExtraInfo": false,
                "type": kind,
                "frameId": target,
                "hasUserGesture": false,
            });
            if let Some(redirect_response) = redirect_response {
                frame["redirectResponse"] = redirect_response;
            }
            send("Network.requestWillBeSent", frame);
        }
        if row.error.is_some() || row.status == 0 {
            let why = failed(row.failure.as_ref().unwrap_or(&NetFailure::Other));
            let mut params = json!({
                "requestId": wire_id,
                "timestamp": row.timestamp,
                "type": kind,
                "errorText": why.error_text,
                "canceled": false,
            });
            if let Some(reason) = why.blocked_reason {
                params["blockedReason"] = json!(reason);
            }
            if let Some((error, parameter)) = why.cors {
                params["corsErrorStatus"] =
                    json!({ "corsError": error, "failedParameter": parameter });
            }
            send("Network.loadingFailed", params);
            continue;
        }
        let landed = if row.final_url.is_empty() {
            &row.url
        } else {
            &row.final_url
        };
        let content_type = row
            .response_headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.as_str())
            .unwrap_or("");
        let mime = content_type.split(';').next().unwrap_or("").trim();
        let charset = content_type
            .split(';')
            .skip(1)
            .find_map(|part| part.trim().strip_prefix("charset="))
            .unwrap_or("")
            .trim_matches('"');
        send(
            "Network.responseReceived",
            json!({
                "requestId": wire_id,
                "loaderId": loader,
                "timestamp": row.timestamp,
                "type": kind,
                "response": {
                    "url": landed,
                    "status": row.status,
                    "statusText": "",
                    "headers": headers(&row.response_headers),
                    "mimeType": mime,
                    "charset": charset,
                    "fromDiskCache": false,
                    "fromServiceWorker": false,
                    "fromPrefetchCache": false,
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
                "requestId": wire_id,
                "timestamp": row.timestamp,
                "encodedDataLength": row.body_size,
                "encodedBodyLength": row.body_size,
            }),
        );
    }
}

/// `Network.getResponseBody`: the body the page kept for the request, as
/// Chrome words it — text as is, anything else base64. A request nothing
/// was kept for is `No resource with given identifier found`, enabled or
/// not.
pub(super) async fn response_body(
    hub: SharedHub,
    session: &str,
    params: &Value,
) -> Result<Value, Failure> {
    let request_id = string(params, "requestId")?.to_string();
    let engine = {
        let hub = hub.lock().await;
        let page = page_of(&hub, session)?;
        // A document was told under its loader id; the engine knows the row.
        let request_id = page
            .doc_rows
            .get(&request_id)
            .cloned()
            .unwrap_or(request_id);
        (page.engine_session.clone(), request_id)
    };
    let (engine, request_id) = engine;
    let kept = send_command(&engine, |reply| SessionCommand::ResponseBody {
        request_id,
        reply,
    })
    .await
    .map_err(|err| Failure::from(session_msg(err)))?;
    match kept {
        Some(body) => Ok(json!({ "body": body.body, "base64Encoded": body.base64_encoded })),
        None => Err(Failure::from("No resource with given identifier found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_engine_knows_of_a_failure_is_chromes_word_for_it() {
        assert_eq!(
            failed(&NetFailure::ConnectionRefused).error_text,
            "net::ERR_CONNECTION_REFUSED"
        );
        assert_eq!(
            failed(&NetFailure::NameNotResolved).error_text,
            "net::ERR_NAME_NOT_RESOLVED"
        );
        assert_eq!(
            failed(&NetFailure::UnsafePort).error_text,
            "net::ERR_UNSAFE_PORT"
        );
        for (failure, chrome) in [
            (NetFailure::EmptyResponse, "net::ERR_EMPTY_RESPONSE"),
            (NetFailure::ConnectionReset, "net::ERR_CONNECTION_RESET"),
            (
                NetFailure::ContentLengthMismatch,
                "net::ERR_CONTENT_LENGTH_MISMATCH",
            ),
            (
                NetFailure::IncompleteChunkedEncoding,
                "net::ERR_INCOMPLETE_CHUNKED_ENCODING",
            ),
        ] {
            assert_eq!(failed(&failure).error_text, chrome, "{failure:?}");
        }
        // Told apart by the engine, not yet by Chrome's probe.
        for unread in [NetFailure::TimedOut, NetFailure::Other] {
            assert_eq!(failed(&unread).error_text, "net::ERR_FAILED", "{unread:?}");
        }
    }

    #[test]
    fn a_url_blocked_from_the_inspector_has_no_error_text() {
        let blocked = failed(&NetFailure::BlockedByInspector);
        assert_eq!(blocked.error_text, "");
        assert_eq!(blocked.blocked_reason, Some("inspector"));
        assert_eq!(blocked.cors, None);
    }

    #[test]
    fn a_cors_refusal_says_whether_the_header_was_missing_or_wrong() {
        let missing = failed(&NetFailure::CorsMissingAllowOrigin);
        assert_eq!(missing.error_text, "net::ERR_FAILED");
        assert_eq!(
            missing.cors,
            Some(("MissingAllowOriginHeader", String::new()))
        );
        let wrong = failed(&NetFailure::CorsAllowOriginMismatch(
            "http://example.test".into(),
        ));
        assert_eq!(
            wrong.cors,
            Some(("AllowOriginMismatch", "http://example.test".to_string()))
        );
    }

    #[test]
    fn a_failed_navigation_is_always_a_net_error() {
        assert_eq!(
            navigation_error(&NetFailure::BlockedByInspector),
            "net::ERR_FAILED"
        );
        assert_eq!(
            navigation_error(&NetFailure::ConnectionRefused),
            "net::ERR_CONNECTION_REFUSED"
        );
    }
}
