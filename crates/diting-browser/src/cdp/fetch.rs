//! The `Fetch` domain: request interception. `Fetch.enable` arms the
//! page's interception channel; every paused request surfaces here as an
//! `InterceptedRequest`, is announced as `Fetch.requestPaused` to the
//! sessions that enabled the domain, and is answered by
//! `Fetch.continueRequest` / `fulfillRequest` / `failRequest` — resolved
//! straight from the pending map, never through the session actor: a
//! paused document request is what the navigate command itself waits on,
//! so routing the answer through the same serial channel would deadlock.
//!
//! Only the `Request` stage is honored: patterns asking for `Response`
//! pauses are accepted and simply never match (Chrome pauses after the
//! response arrived; the engine's hook sits before the send).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use base64::Engine as _;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use diting::diting_js::ops::InterceptedRequest;
use diting::diting_net::{url_pattern_matches, InterceptResolution};

use super::params::{invalid, kinds, optional_boolean, optional_string, string, Kind};
use super::{ack, deliver, page_of, page_of_mut, Failure, Out, SharedHub};
use crate::session::send_command;

/// One `Fetch.enable` pattern, already read for kind. `request_stage` is
/// Chrome's word; only `"Request"` (the default) can pause here.
#[derive(Clone)]
struct FetchPattern {
    url_pattern: String,
    request_stage: String,
}

/// The page-level interception state the hub keeps while any session has
/// `Fetch` enabled: paused requests waiting for their answer, and the
/// patterns the drainer matches against (shared so a re-`enable` with new
/// patterns takes effect without re-arming the engine).
pub(super) struct FetchSlot {
    pending: Arc<AsyncMutex<HashMap<String, InterceptedRequest>>>,
    patterns: Arc<RwLock<Vec<FetchPattern>>>,
}

/// `Fetch.enable`: read the patterns, mark the session, arm the page's
/// interception channel on first use, and spawn the drainer that turns
/// engine pauses into `Fetch.requestPaused` events.
pub(super) async fn enable(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    let patterns = patterns(params)?;
    optional_boolean(params, "handleAuthRequests")?;

    // Sessions on the page that asked for pauses — the events go to all of
    // them; the first answer to a requestId resolves it (Chrome gives each
    // enabling session its own interception; the engine's channel is
    // page-level, so one pause is shared — the single-session case every
    // real client uses is exact).
    let (engine, target, spawn) = {
        let mut hub = hub.lock().await;
        hub.sessions
            .get_mut(session)
            .ok_or_else(|| Failure::from(format!("no such session {session}")))?
            .fetch_on = true;
        let page = page_of_mut(&mut hub, session)?;
        let patterns = Arc::new(RwLock::new(patterns));
        match &mut page.fetch {
            Some(slot) => {
                *slot.patterns.write().unwrap_or_else(|e| e.into_inner()) =
                    patterns.read().unwrap().clone();
                (None, None, None)
            }
            None => {
                let pending = Arc::new(AsyncMutex::new(HashMap::new()));
                let slot = FetchSlot {
                    pending: pending.clone(),
                    patterns: patterns.clone(),
                };
                page.fetch = Some(slot);
                (
                    Some(page.engine_session.clone()),
                    Some(page.target_id.clone()),
                    Some((pending, patterns)),
                )
            }
        }
    };
    if let (Some(engine), Some(target), Some((pending, patterns))) = (engine, target, spawn) {
        let (tx, rx) = mpsc::unbounded_channel();
        send_command(&engine, |reply| crate::session::SessionCommand::Page {
            op: crate::session::PageOp::FetchIntercept { tx: Some(tx) },
            reply,
        })
        .await
        .map_err(|err| super::runtime::engine_failure(super::session_msg(err)))?;
        drain(hub.clone(), target, rx, pending, patterns);
    }
    Ok(ack(id, session))
}

/// `Fetch.disable`: the session stops hearing pauses; when no session on
/// the page still wants them, the engine channel is disarmed and every
/// pending pause falls through to the real request (Continue semantics —
/// dropping the resolver reads as no answer).
pub(super) async fn disable(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    kinds(params, &[])?;
    let (engine, disarm) = {
        let mut hub = hub.lock().await;
        let slot = hub
            .sessions
            .get_mut(session)
            .ok_or_else(|| Failure::from(format!("no such session {session}")))?;
        slot.fetch_on = false;
        let target = match &slot.attached {
            super::Attached::Page(target) => target.clone(),
            super::Attached::Browser(_) => {
                return Err(Failure::from("Fetch domain needs a page session"));
            }
        };
        let still_on = hub
            .sessions
            .values()
            .any(|other| other.id != session && other.on(&target) && other.fetch_on);
        if still_on {
            (None, false)
        } else {
            let page = page_of_mut(&mut hub, session)?;
            page.fetch = None;
            (Some(page.engine_session.clone()), true)
        }
    };
    if disarm {
        if let Some(engine) = engine {
            send_command(&engine, |reply| crate::session::SessionCommand::Page {
                op: crate::session::PageOp::FetchIntercept { tx: None },
                reply,
            })
            .await
            .map_err(|err| super::runtime::engine_failure(super::session_msg(err)))?;
        }
    }
    Ok(ack(id, session))
}

/// `Fetch.continueRequest` / `fulfillRequest` / `failRequest`: look the
/// pause up in the page's pending map and hand it the client's answer.
/// The map lives on the hub, so this never touches the session actor —
/// the whole point of the bypass.
pub(super) async fn resolve(
    hub: SharedHub,
    id: &Value,
    session: &str,
    method: &str,
    params: &Value,
) -> Result<Out, Failure> {
    let request_id = string(params, "requestId")?;
    let resolution = match method {
        "Fetch.continueRequest" => {
            optional_string(params, "url")?;
            optional_string(params, "method")?;
            optional_string(params, "postData")?;
            let headers = header_entries(params)?;
            optional_boolean(params, "interceptResponse")?;
            InterceptResolution::Continue {
                url: optional_string(params, "url")?.map(str::to_string),
                method: optional_string(params, "method")?.map(str::to_string),
                headers,
                body: post_data(params)?,
            }
        }
        "Fetch.fulfillRequest" => {
            let code = super::params::integer(params, "responseCode")?;
            if !(100..=999).contains(&code) {
                return Err(invalid("Invalid parameters"));
            }
            optional_string(params, "responsePhrase")?;
            let headers = header_entries(params)?;
            InterceptResolution::Fulfill {
                status: code as u16,
                headers: headers.unwrap_or_default(),
                body: post_data(params)?.unwrap_or_default(),
            }
        }
        "Fetch.failRequest" => {
            let reason = string(params, "errorReason")?;
            kinds(params, &[("errorReason", Kind::Text)])?;
            InterceptResolution::Fail {
                reason: reason.to_string(),
            }
        }
        _ => return Err(Failure::NotFound(method.to_string())),
    };
    let pending = {
        let hub = hub.lock().await;
        page_of(&hub, session)?
            .fetch
            .as_ref()
            .map(|slot| slot.pending.clone())
            .ok_or_else(|| Failure::from("Invalid InterceptionId.".to_string()))?
    };
    let request = pending.lock().await.remove(request_id);
    match request {
        Some(request) => {
            let _ = request.resolver.send(resolution);
            Ok(ack(id, session))
        }
        None => Err(Failure::from("Invalid InterceptionId.".to_string())),
    }
}

/// The drainer: engine pauses in, `Fetch.requestPaused` events out, and
/// the pending map the resolution commands answer from. Ends when the
/// page's interception channel closes (page or session teardown) — the
/// pending map then drops with it and in-flight pauses fall through.
fn drain(
    hub: SharedHub,
    target: String,
    mut rx: mpsc::UnboundedReceiver<InterceptedRequest>,
    pending: Arc<AsyncMutex<HashMap<String, InterceptedRequest>>>,
    patterns: Arc<RwLock<Vec<FetchPattern>>>,
) {
    tokio::spawn(async move {
        let mut counter = 0u64;
        while let Some(request) = rx.recv().await {
            let pats = patterns.read().unwrap_or_else(|e| e.into_inner()).clone();
            let matched = pats.iter().any(|p| {
                p.request_stage == "Request" && url_pattern_matches(&p.url_pattern, &request.url)
            });
            if !matched {
                // Not this client's business: continue untouched, without
                // ever announcing a pause Chrome would not have sent.
                let _ = request.resolver.send(InterceptResolution::Continue {
                    url: None,
                    method: None,
                    headers: None,
                    body: None,
                });
                continue;
            }
            counter += 1;
            let request_id = format!("interception-job-{counter}.0");
            let engine_row_id = request.request_id.clone();
            let resource_type = request.resource_type.clone();
            let mut paused = json!({
                "requestId": request_id,
                "request": {
                    "url": request.url,
                    "method": request.method,
                    "headers": request.headers,
                },
                "frameId": target,
                "resourceType": resource_type,
                "requestStage": "Request",
            });
            if let Some(body) = &request.post_data {
                use base64::Engine as _;
                paused["request"]["postData"] =
                    json!(base64::engine::general_purpose::STANDARD.encode(body));
            }
            let mut out = Out::default();
            {
                let mut hub = hub.lock().await;
                if !hub.pages.contains_key(&target) {
                    drop(hub);
                    pending.lock().await.remove(&request_id);
                    let _ = request.resolver.send(InterceptResolution::Continue {
                        url: None,
                        method: None,
                        headers: None,
                        body: None,
                    });
                    continue;
                }
                // Playwright routes a pause only once it can pair it with the
                // request's `Network.requestWillBeSent` (`networkId` is the
                // key) — a pause without one is auto-continued, unrouted. The
                // row that will carry this request is written when the fetch
                // *lands*, which the pause itself blocks, so the pairing frame
                // cannot come from the row: it is told here, ahead of the
                // pause, in the shape `network_frames` would use, and the row
                // later skips its own copy (its wire id sits in `announced`).
                // A document's wire id is its loader's — the loader of the
                // navigation in flight is the page's next, since it numbers
                // when the navigation completes.
                let page = &hub.pages[&target];
                let network_id = if resource_type == "Document" {
                    format!("ldr{}", page.loader + 1)
                } else {
                    engine_row_id.clone()
                };
                if !network_id.is_empty() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64();
                    let loader = if resource_type == "Document" {
                        network_id.clone()
                    } else {
                        format!("ldr{}", page.loader)
                    };
                    let document_url = page.url.clone();
                    let initiator = match resource_type.as_str() {
                        "Fetch" | "XHR" => {
                            json!({ "type": "script", "stack": { "callFrames": [] } })
                        }
                        _ => json!({ "type": "other" }),
                    };
                    let mut will_be_sent = json!({
                        "requestId": network_id,
                        "loaderId": loader,
                        "documentURL": document_url,
                        "request": paused["request"].clone(),
                        "timestamp": now,
                        "wallTime": now,
                        "initiator": initiator,
                        "redirectHasExtraInfo": false,
                        "type": resource_type,
                        "frameId": target,
                        "hasUserGesture": false,
                    });
                    if let Some(req) = will_be_sent["request"].as_object_mut() {
                        req.insert("initialPriority".to_string(), json!("High"));
                        req.insert(
                            "referrerPolicy".to_string(),
                            json!("strict-origin-when-cross-origin"),
                        );
                        req.insert("mixedContentType".to_string(), json!("none"));
                    }
                    paused["networkId"] = json!(network_id);
                    let page = hub.pages.get_mut(&target).expect("checked above");
                    page.announced.insert(network_id.clone());
                    page.announced_order.push_back(network_id.clone());
                    while page.announced_order.len() > 1024 {
                        if let Some(oldest) = page.announced_order.pop_front() {
                            page.announced.remove(&oldest);
                        }
                    }
                    let sessions: Vec<(u64, Option<String>)> = hub
                        .sessions
                        .values()
                        .filter(|slot| slot.on(&target) && slot.fetch_on)
                        .map(|slot| (slot.client, hub.level(Some(&slot.id))))
                        .collect();
                    for (client, level) in sessions {
                        deliver(
                            &hub,
                            &mut out,
                            0,
                            (client, level.clone()),
                            "Network.requestWillBeSent",
                            will_be_sent.clone(),
                        );
                        deliver(
                            &hub,
                            &mut out,
                            0,
                            (client, level),
                            "Fetch.requestPaused",
                            paused.clone(),
                        );
                    }
                } else {
                    // No row id to pair with (transport-level pauses do not
                    // pre-mint one): the pause goes out bare and clients that
                    // require correlation auto-continue it — unrouted but
                    // never stuck.
                    let sessions: Vec<(u64, Option<String>)> = hub
                        .sessions
                        .values()
                        .filter(|slot| slot.on(&target) && slot.fetch_on)
                        .map(|slot| (slot.client, hub.level(Some(&slot.id))))
                        .collect();
                    for (client, level) in sessions {
                        deliver(
                            &hub,
                            &mut out,
                            0,
                            (client, level),
                            "Fetch.requestPaused",
                            paused.clone(),
                        );
                    }
                }
            }
            pending.lock().await.insert(request_id.clone(), request);
            for (outbox, frame) in out.spill {
                let _ = outbox.send(frame);
            }
        }
    });
}

/// `Fetch.enable`'s `patterns` field: an array of
/// `{urlPattern, requestStage?, resourceType?}` objects. Every declared
/// field is kind-checked; entries that are not objects or lack a string
/// `urlPattern` are `-32602`, as Chrome reads them.
fn patterns(params: &Value) -> Result<Vec<FetchPattern>, Failure> {
    if params.is_null() {
        return Ok(Vec::new());
    }
    if !params.is_object() {
        return Err(super::params::invalid_params());
    }
    let Some(value) = params.get("patterns") else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(super::params::invalid_params());
    };
    let mut out = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            return Err(super::params::invalid_params());
        };
        let url_pattern = obj
            .get("urlPattern")
            .and_then(Value::as_str)
            .ok_or_else(super::params::invalid_params)?
            .to_string();
        let request_stage = obj
            .get("requestStage")
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(super::params::invalid_params)
            })
            .transpose()?
            .unwrap_or_else(|| "Request".to_string());
        if let Some(rt) = obj.get("resourceType") {
            if !rt.is_string() {
                return Err(super::params::invalid_params());
            }
        }
        out.push(FetchPattern {
            url_pattern,
            request_stage,
        });
    }
    Ok(out)
}

/// A `{name, value}` header list — `Fetch.continueRequest.headers` and
/// `Fetch.fulfillRequest.responseHeaders` share the shape. `None` when the
/// field was left out; entries of another kind are `-32602`.
fn header_entries(params: &Value) -> Result<Option<HashMap<String, String>>, Failure> {
    let Some(value) = params
        .get("headers")
        .or_else(|| params.get("responseHeaders"))
    else {
        return Ok(None);
    };
    let Some(items) = value.as_array() else {
        return Err(super::params::invalid_params());
    };
    let mut map = HashMap::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(super::params::invalid_params)?;
        let val = item
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(super::params::invalid_params)?;
        map.insert(name.to_string(), val.to_string());
    }
    Ok(Some(map))
}

/// The base64 `postData` / `body` field, decoded. `None` when absent.
fn post_data(params: &Value) -> Result<Option<Vec<u8>>, Failure> {
    let field = if params.get("postData").is_some() {
        "postData"
    } else {
        "body"
    };
    let Some(value) = params.get(field) else {
        return Ok(None);
    };
    let Some(text) = value.as_str() else {
        return Err(super::params::invalid_params());
    };
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map(Some)
        .map_err(|_| super::params::invalid_params())
}
