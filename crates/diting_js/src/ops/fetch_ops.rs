// fetch op 面：请求拼装/凭据门 + 结算窗 + 同步 XHR + glob 门 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// Cap on the number of redirect hops op_fetch_url will follow.
///
/// The Fetch standard fixes the number at 20: HTTP-redirect fetch returns a
/// network error as soon as a request's redirect count *reaches* 20, so the
/// twentieth hop still succeeds and the twenty-first fails.
/// https://fetch.spec.whatwg.org/#http-redirect-fetch
///
/// A client's usual default of 10 does not apply here: redirects are followed by
/// hand in this file, one hop per loop iteration, so each hop is re-checked
/// against the SSRF rules (upstream 4b90ec3).
pub(crate) const FETCH_REDIRECT_LIMIT: usize = 20;

/// RequestCredentials from the Fetch standard: whether cookies may be sent to
/// (and stored from) a request's URL (upstream b744b9b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FetchCredentials {
    Omit,
    SameOrigin,
    Include,
}

impl FetchCredentials {
    fn parse(value: &str) -> Self {
        match value {
            "omit" => Self::Omit,
            "include" => Self::Include,
            _ => Self::SameOrigin,
        }
    }

    pub(super) fn allows(self, page_origin: &str, request_url: &str) -> bool {
        match self {
            Self::Omit => false,
            Self::Include => true,
            Self::SameOrigin => request_origin(request_url)
                .map(|origin| origin == page_origin)
                .unwrap_or(false),
        }
    }
}

/// OpState-cloned inputs the fetch front half gathers before any network
/// I/O, shared by both drivers.
pub(super) struct FetchRequestInit {
    pub(super) cookie_jar: Option<Arc<CookieJar>>,
    pub(super) in_flight: Option<Arc<std::sync::atomic::AtomicU32>>,
    pub(super) in_flight_list:
        Option<std::sync::Arc<std::sync::Mutex<Vec<diting_net::InFlightScripted>>>>,
    pub(super) proxy_url: Option<String>,
    pub(super) http_client: Option<Arc<HttpClient>>,
    pub(super) callbacks: Option<std::sync::Arc<diting_net::CallbackRegistry>>,
    pub(super) document_url: String,
    pub(super) intercept_tx: Option<tokio::sync::mpsc::UnboundedSender<InterceptedRequest>>,
}

/// Shared front half of op_fetch_url and op_fetch_url_sync: the SSRF gate,
/// the base64-body marker probe, and the OpState clones both drivers hand
/// to the walk. Returns Err(the exact JSON the op must return) for the two
/// short-circuits (SSRF reject, setBlockedURLs pattern), each recorded into
/// the network-event log first.
pub(super) fn gather_fetch_parts(
    state: &OpState,
    url: &str,
    method: &str,
    headers_json: &str,
    started: f64,
) -> Result<(FetchRequestInit, bool), String> {
    if let Ok(parsed_url) = url::Url::parse(url) {
        if let Err(e) = validate_fetch_url(&parsed_url) {
            record_failed_fetch(
                state,
                &FetchFailure {
                    url: url.to_string(),
                    method: method.to_string(),
                    reason: e.clone(),
                    final_url: url.to_string(),
                    redirects: Vec::new(),
                    started,
                },
            );
            return Err(serde_json::json!({
                "status": 0,
                "body": "",
                "url": url,
                "headers": {},
                "blocked": true,
                "error": e,
            })
            .to_string());
        }
    } else {
        // Fail closed (#252): an unparseable URL used to skip the gate
        // entirely — harmless while the client re-parsed the same string with
        // the same url crate, a hole the moment a transport disagrees. The
        // JS fetch entry throws Chrome's TypeError face before the op call,
        // so this is the backstop for direct op callers (module loader,
        // XHR's shared closure).
        let reason = "invalid URL (failed to parse)".to_string();
        record_failed_fetch(
            state,
            &FetchFailure {
                url: url.to_string(),
                method: method.to_string(),
                reason: reason.clone(),
                final_url: url.to_string(),
                redirects: Vec::new(),
                started,
            },
        );
        return Err(serde_json::json!({
            "status": 0,
            "body": "",
            "url": url,
            "headers": {},
            "blocked": true,
            "error": reason,
        })
        .to_string());
    }

    // The JS shim sets this out-of-band header to signal that `body` is
    // base64-encoded raw bytes (upstream obscura #716). Detected from the
    // original headers_json, before any interception rewrite, since a
    // Continue rewrite supplies a plain-text body, never the base64 wire form.
    let body_is_base64 = serde_json::from_str::<serde_json::Value>(headers_json)
        .ok()
        .and_then(|v| {
            v.get("__diting_body_b64")
                .and_then(|x| x.as_str())
                .map(|s| s == "1")
        })
        .unwrap_or(false);

    let (init, blocked_pattern) = {
        let gs = state.borrow::<SharedState>().clone();
        let gs = gs.borrow_mut();
        let blocked_pattern = gs
            .blocked_urls
            .iter()
            .find(|p| *p == "*" || url.contains(p.as_str()) || glob_match(p, url))
            .cloned();
        tracing::debug!(
            "op_fetch_url: intercept_enabled={}, has_tx={}",
            gs.intercept_enabled,
            gs.intercept_tx.is_some()
        );
        let init = FetchRequestInit {
            cookie_jar: gs.cookie_jar.clone(),
            in_flight: gs.http_client.as_ref().map(|c| c.in_flight.clone()),
            in_flight_list: gs.http_client.as_ref().map(|c| c.in_flight_list.clone()),
            // #139: thread the configured proxy through to the per-request
            // diting_net::wire::Client. Without this, op_fetch_url silently bypasses
            // BrowserContext.proxy_url for every JS fetch() / XHR call.
            proxy_url: gs
                .http_client
                .as_ref()
                .and_then(|c| c.proxy_url().map(|s| s.to_string())),
            http_client: gs.http_client.clone(),
            callbacks: gs.callbacks.clone(),
            document_url: gs.document.url(),
            intercept_tx: if gs.intercept_enabled {
                gs.intercept_tx.clone()
            } else {
                None
            },
        };
        (init, blocked_pattern)
    };
    // Recorded after the scope closes — record_failed_fetch re-borrows
    // this state, which cannot nest under the held borrow_mut.
    if let Some(pattern) = blocked_pattern {
        record_failed_fetch(
            state,
            &FetchFailure {
                url: url.to_string(),
                method: method.to_string(),
                reason: format!("blocked by Network.setBlockedURLs pattern: {pattern}"),
                final_url: url.to_string(),
                redirects: Vec::new(),
                started,
            },
        );
        return Err(serde_json::json!({
            "status": 0,
            "body": "",
            "url": url,
            "headers": {},
            "blocked": true,
        })
        .to_string());
    }
    Ok((init, body_is_base64))
}

/// Records a successful fetch's body + network event (upstream #406/#360):
/// keyed `fetch-{N}`, LRU-bounded, so the CDP layer can emit Network events
/// and resolve getResponseBody for fetch()/XHR traffic. Extracted from the
/// walk so the sync driver can record after its worker thread returns.
pub(super) fn record_fetch_network_event(state: &OpState, ev: &FetchNetworkEvent) -> String {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.network_response_body_counter += 1;
    let request_id = format!("fetch-{}", gs.network_response_body_counter);
    let max_entries = response_body_entry_limit();
    let max_bytes = response_body_byte_limit();
    let (stored_body, base64_encoded) = match &ev.stored_text {
        Some(text) => (text.clone(), false),
        None => (ev.resp_body_base64.clone(), true),
    };
    if max_entries > 0 && max_bytes > 0 && ev.body_size <= max_bytes {
        gs.network_response_bodies.insert(
            request_id.clone(),
            StoredNetworkResponseBody {
                body: stored_body,
                base64_encoded,
            },
        );
        gs.network_response_body_order.push_back(request_id.clone());
        while gs.network_response_body_order.len() > max_entries {
            if let Some(oldest) = gs.network_response_body_order.pop_front() {
                gs.network_response_bodies.remove(&oldest);
            }
        }
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    gs.js_network_events.push(JsNetworkEvent {
        request_id: request_id.clone(),
        url: ev.url.clone(),
        method: ev.method.clone(),
        status: ev.status,
        response_headers: ev.response_headers.clone(),
        request_headers: ev.request_headers.clone(),
        body_size: ev.body_size,
        timestamp,
        error: None,
        final_url: ev.final_url.clone(),
        redirects: ev.redirects.clone(),
        started: ev.started,
    });
    const MAX_JS_NETWORK_EVENTS: usize = 4096;
    if gs.js_network_events.len() > MAX_JS_NETWORK_EVENTS {
        let overflow = gs.js_network_events.len() - MAX_JS_NETWORK_EVENTS;
        gs.js_network_events.drain(0..overflow);
    }
    request_id
}

/// Replays the failure events the walk collected into the network-event log
/// (a sequential walk yields at most one).
pub(super) fn replay_fetch_failures(state: &OpState, failures: &[FetchFailure]) {
    for f in failures {
        record_failed_fetch(state, f);
    }
}

/// #226: mirror of async fetch completions, keyed by the JS shim's per-call
/// key. A watchdog TerminateExecution landing mid event-loop tick eats op
/// settlements silently — deno_core pops the completed batch in
/// `dispatch_event_loop_tick`, the tick (or the microtask carrying the await
/// continuation) dies, and the phase dispatchers swallow the termination as
/// `Ok(false)`, so neither side ever learns. The JS side probes this table
/// from the macrotask-recover hook (which runs on every unwind) and a slow
/// heartbeat, and settles its outer promise manually via `op_fetch_take`.
/// Byte-bounded: a lost settlement that is never probed out is exactly the
/// entry that lingers longest, and bodies can be megabyte configs.
#[derive(Default)]
pub(crate) struct FetchSettlements {
    pub(super) deque: std::collections::VecDeque<(String, String)>,
    pub(super) total_bytes: usize,
}

pub(super) const FETCH_SETTLEMENT_MAX_BYTES: usize = 8 * 1024 * 1024;
pub(super) const FETCH_SETTLEMENT_MAX_ENTRIES: usize = 64;

impl FetchSettlements {
    fn push(&mut self, key: String, payload: String) {
        self.total_bytes += payload.len();
        self.deque.push_back((key, payload));
        while self.deque.len() > FETCH_SETTLEMENT_MAX_ENTRIES
            || self.total_bytes > FETCH_SETTLEMENT_MAX_BYTES
        {
            match self.deque.pop_front() {
                Some((_, p)) => self.total_bytes = self.total_bytes.saturating_sub(p.len()),
                None => break,
            }
        }
    }

    fn take(&mut self, key: &str) -> Option<String> {
        let pos = self.deque.iter().rposition(|(k, _)| k == key)?;
        let (_, payload) = self.deque.remove(pos)?;
        self.total_bytes = self.total_bytes.saturating_sub(payload.len());
        Some(payload)
    }
}

/// Unix seconds "now" — the one clock every fetch-row stamp shares (#229:
/// op entry = request dispatch; stamps taken later, after an intercept
/// wait or gate walk, would misorder HAR rows).
pub(super) fn now_unix() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Mirror one fetch settlement onto the #226 ring. Every async-op exit —
/// the walk's success/error AND the pre-walk short-circuits (SSRF /
/// setBlockedURLs return, intercept Fulfill/Fail, rewrite blocks) — owes
/// the JS probe a settlement; an exit that skips this turns a
/// watchdog-eaten completion into a promise that never settles.
pub(super) fn push_fetch_settlement(
    state: &std::rc::Rc<std::cell::RefCell<OpState>>,
    key: &str,
    value: String,
) {
    if !key.is_empty() {
        state
            .borrow_mut()
            .borrow_mut::<FetchSettlements>()
            .push(key.to_string(), value);
    }
}

/// Probe-and-take one mirrored completion. `""` = not completed (or already
/// evicted); a payload starting with NUL is a rejection notice, anything else
/// is the op's JSON result verbatim.
#[diting_op]
#[string]
pub(super) fn op_fetch_take(state: &mut OpState, #[string] key: String) -> String {
    state
        .borrow_mut::<FetchSettlements>()
        .take(&key)
        .unwrap_or_default()
}

/// 10d host flip: the body runs on OUR Vm host — hand-called from
/// `async_glue::op_fetch_url_impl` (the raw wrapper sees `(promise_id,
/// user args...)`; the JS async stub owns the promise). The `Rc<RefCell<
/// OpState>>` face is our own state cell now; the JsErrorBox error face
/// stays (leaf crate), collapsing to a message string at the glue.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn op_fetch_url_body(
    state: Rc<RefCell<OpState>>,
    url: String,
    method: String,
    headers_json: String,
    body: String,
    origin: String,
    mode: String,
    credentials: String,
    // "policy\0init" — bundled to stay under deno_core's async op arg limit.
    referrer: String,
) -> Result<String, deno_error::JsErrorBox> {
    // deno_core async op codegen caps explicit args; the Referer pair rides one slot
    // (and, third segment, the #226 settlement key — see FetchSettlements).
    let mut referrer_segs = referrer.split('\u{0}');
    let referrer_policy = referrer_segs.next().unwrap_or("").to_string();
    let referrer_init = referrer_segs.next().unwrap_or("about:client").to_string();
    let settle_key = referrer_segs.next().unwrap_or("").to_string();
    // "" = the caller carried no explicit policy (fetch init unset, loaders,
    // XHR): fall back to the document's own policy (Referrer-Policy header or
    // <meta name=referrer>), else fetch_referer applies the spec default.
    let referrer_policy = if referrer_policy.is_empty() {
        resolve_document_referrer_policy(&state.borrow())
    } else {
        referrer_policy
    };
    tracing::debug!(
        "op_fetch_url called: {} {} (intercept check pending)",
        method,
        url
    );

    // One entry stamp, every exit's truth (#229): rows stamped later —
    // after the intercept wait or gate walk — would order behind requests
    // dispatched after them.
    let started_unix = now_unix();
    // Bind first: the `state.borrow()` temporary must drop before the arm
    // body re-borrows for the settlement push (same nesting rule gather's
    // own record_failed_fetch follows).
    let gathered = gather_fetch_parts(&state.borrow(), &url, &method, &headers_json, started_unix);
    let (init, body_is_base64) = match gathered {
        Ok(parts) => parts,
        Err(json) => {
            // Pre-walk short-circuit (SSRF / blocked pattern): still an
            // op exit, still owes the probe its settlement (#226 — the
            // mirror must cover every exit, not just the walk's).
            push_fetch_settlement(&state, &settle_key, json.clone());
            return Ok(json);
        }
    };
    let FetchRequestInit {
        cookie_jar,
        in_flight,
        in_flight_list,
        proxy_url,
        http_client,
        callbacks,
        document_url,
        intercept_tx,
    } = init;

    let mut override_url: Option<String> = None;
    let mut override_method: Option<String> = None;
    let mut override_headers: Option<HashMap<String, String>> = None;
    let mut override_body: Option<Vec<u8>> = None;
    if let Some(tx) = intercept_tx {
        let mut request_headers_view: HashMap<String, String> =
            serde_json::from_str(&headers_json).unwrap_or_default();
        // The base64-body marker is an internal transport signal, never a
        // request header — the wire path strips it; the paused view must
        // too, or an intercepting client sees a header Chrome would never
        // send (#252).
        request_headers_view.remove("__diting_body_b64");
        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel();
        let intercepted = InterceptedRequest {
            url: url.clone(),
            method: method.clone(),
            headers: request_headers_view.clone(),
            resource_type: "Fetch".to_string(),
            resolver: resolve_tx,
        };
        if tx.send(intercepted).is_ok() {
            // Bounded wait: the bridge drains pauses between commands, so a
            // resolution normally lands within one command gap. When the
            // dispatch that triggered the fetch is itself parked on this
            // future (evaluate of a fetch expression), the resolution cannot
            // be delivered and the wait must expire instead of hanging the
            // command forever.
            let timeout_ms =
                INTERCEPT_RESOLUTION_TIMEOUT_MS.load(std::sync::atomic::Ordering::Relaxed);
            match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), resolve_rx)
                .await
            {
                Ok(Ok(InterceptResolution::Fulfill {
                    status,
                    headers: h,
                    body: b,
                })) => {
                    // Chrome emits network events for fulfilled requests
                    // too — a fulfilled fetch must not read as "never
                    // happened" in the network log (#252); Fail already
                    // replays through record_failed_fetch.
                    let body_b64 = BASE64.encode(&b);
                    record_fetch_network_event(
                        &state.borrow(),
                        &FetchNetworkEvent {
                            url: url.clone(),
                            method: method.clone(),
                            status,
                            response_headers: h.clone(),
                            request_headers: request_headers_view.clone(),
                            body_size: b.len(),
                            stored_text: None,
                            resp_body_base64: body_b64.clone(),
                            final_url: url.clone(),
                            redirects: Vec::new(),
                            started: started_unix,
                        },
                    );
                    // Byte-native: the synthetic body rides the same base64
                    // channel as real network responses (binary payloads
                    // survive; text is reconstructed on the JS side).
                    let resp = serde_json::json!({
                        "status": status,
                        "bodyBase64": body_b64,
                        "url": url,
                        "headers": h,
                    })
                    .to_string();
                    push_fetch_settlement(&state, &settle_key, resp.clone());
                    return Ok(resp);
                }
                Ok(Ok(InterceptResolution::Fail { reason })) => {
                    record_failed_fetch(
                        &state.borrow(),
                        &FetchFailure {
                            url: url.clone(),
                            method: method.clone(),
                            reason: reason.clone(),
                            final_url: url.clone(),
                            redirects: Vec::new(),
                            started: started_unix,
                        },
                    );
                    let resp = serde_json::json!({
                        "status": 0,
                        "body": "",
                        "url": url,
                        "headers": {},
                        "blocked": true,
                        "error": reason,
                    })
                    .to_string();
                    push_fetch_settlement(&state, &settle_key, resp.clone());
                    return Ok(resp);
                }
                Ok(Ok(InterceptResolution::Continue {
                    url: new_url,
                    method: new_method,
                    headers: new_headers,
                    body: new_body,
                })) => {
                    override_url = new_url;
                    override_method = new_method;
                    override_headers = new_headers;
                    override_body = new_body;
                }
                // Client answered nothing in time (or dropped the resolver):
                // fall through to the real request, i.e. Continue semantics.
                Ok(Err(_)) | Err(_) => {}
            }
        }
    }

    // Apply interception overrides (shadow the params for the rest of the op).
    // A Continue rewrite of the URL must pass the same SSRF / private-network
    // gate as the original request (checked above) and as redirects (checked
    // below). Without this re-validation a rewrite to an internal address would
    // bypass validate_fetch_url entirely — and an *unparseable* rewrite must
    // fail closed (#70): passing the raw string through would hand the request
    // to the network layer with no validation at all.
    let url = match override_url {
        Some(new_url) => {
            match url::Url::parse(&new_url) {
                Ok(parsed) => {
                    if let Err(reason) = validate_fetch_url(&parsed) {
                        let error =
                            format!("Intercept rewrite to forbidden URL blocked: {}", reason);
                        record_failed_fetch(
                            &state.borrow(),
                            &FetchFailure {
                                url: new_url.clone(),
                                method: method.clone(),
                                reason: error.clone(),
                                final_url: new_url.clone(),
                                redirects: Vec::new(),
                                started: started_unix,
                            },
                        );
                        let resp = serde_json::json!({
                            "status": 0,
                            "body": "",
                            "url": new_url,
                            "blocked": true,
                            "error": error,
                        })
                        .to_string();
                        push_fetch_settlement(&state, &settle_key, resp.clone());
                        return Ok(resp);
                    }
                }
                Err(_) => {
                    let error = "Intercept rewrite to unparseable URL blocked".to_string();
                    record_failed_fetch(
                        &state.borrow(),
                        &FetchFailure {
                            url: new_url.clone(),
                            method: method.clone(),
                            reason: error.clone(),
                            final_url: new_url.clone(),
                            redirects: Vec::new(),
                            started: started_unix,
                        },
                    );
                    let resp = serde_json::json!({
                        "status": 0,
                        "body": "",
                        "url": new_url,
                        "blocked": true,
                        "error": error,
                    })
                    .to_string();
                    push_fetch_settlement(&state, &settle_key, resp.clone());
                    return Ok(resp);
                }
            }
            new_url
        }
        None => url,
    };
    let method = override_method.unwrap_or(method);
    // A Continue rewrite supplies raw request bytes (the CDP layer already
    // decoded the base64 wire form), so the base64 flag applies only to the
    // original body.
    let body_bytes: Vec<u8> = match override_body {
        Some(b) => b,
        None => {
            if body_is_base64 {
                BASE64.decode(&body).unwrap_or_default()
            } else {
                body.into_bytes()
            }
        }
    };
    let headers_json = match override_headers {
        Some(h) => serde_json::to_string(&h).unwrap_or(headers_json),
        None => headers_json,
    };

    let mut custom_headers: std::collections::HashMap<String, String> =
        serde_json::from_str(&headers_json).unwrap_or_default();
    // The out-of-band base64 marker must not leak to the wire or into the
    // preflight Access-Control-Request-Headers list.
    custom_headers.remove("__diting_body_b64");

    // url::Url::origin() normalizes default ports, so an explicit :443 still
    // compares same-origin (the old hand-rolled form did not).
    let initial_request_origin = request_origin(&url).unwrap_or_default();
    let page_origin = if origin.is_empty() {
        initial_request_origin
    } else {
        origin
    };

    let mut deps = FetchWalkDeps {
        url,
        method,
        custom_headers,
        body_bytes,
        page_origin,
        mode,
        credentials: FetchCredentials::parse(&credentials),
        cookie_jar,
        in_flight,
        in_flight_list,
        http_client,
        proxy_url,
        document_url,
        referrer_policy,
        referrer_init,
        callbacks,
        failures: Vec::new(),
        // #229: op entry = request dispatch. Stamped before the first hop
        // goes out so the HAR face can order concurrent requests by start,
        // not by which response landed first.
        started_unix,
    };

    let outcome = match fetch_url_walk(&mut deps).await {
        Ok(outcome) => outcome,
        Err(e) => {
            replay_fetch_failures(&state.borrow(), &deps.failures);
            push_fetch_settlement(&state, &settle_key, format!("\u{0}{e}"));
            return Err(e);
        }
    };
    replay_fetch_failures(&state.borrow(), &deps.failures);
    if let Some(ev) = outcome.network.as_ref() {
        let request_id = record_fetch_network_event(&state.borrow(), ev);
        // #224: status rides the line — the doudian post-mortem had 0-byte
        // completions whose status was unknowable from the log, and
        // "empty 200" vs "synthesized 0" are different bugs.
        tracing::debug!(
            "op_fetch_url completed: {} {} ({} bytes, status {}, network event {})",
            ev.method,
            ev.url,
            ev.body_size,
            ev.status,
            request_id,
        );
    }
    if !settle_key.is_empty() {
        push_fetch_settlement(&state, &settle_key, outcome.json.clone());
    }
    Ok(outcome.json)
}

/// The seven Referrer Policy tokens ("" is the "unset" marker, not a policy).
pub(super) const REFERRER_POLICY_TOKENS: [&str; 7] = [
    "no-referrer",
    "no-referrer-when-downgrade",
    "origin",
    "origin-when-cross-origin",
    "strict-origin",
    "strict-origin-when-cross-origin",
    "unsafe-url",
];

pub(crate) fn glob_match(pattern: &str, url: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(mid) = pattern.strip_prefix('*').and_then(|p| p.strip_suffix('*')) {
        return url.contains(mid);
    }
    if let Some(tail) = pattern.strip_prefix('*') {
        return url.ends_with(tail);
    }
    if let Some(head) = pattern.strip_suffix('*') {
        return url.starts_with(head);
    }
    url == pattern
}

/// Record a `status: 0` network event for a fetch that never produced a
/// servable response — SSRF-blocked, URL-blocklisted, CORS-refused, or dead
/// at the transport layer. The success path records via
/// `record_fetch_network_event`; without this companion every early exit
/// vanished from the session /network log, and a page whose API calls all
/// died here read as "never issued a request" — exactly the ghost the taobao
/// shop-SPA report chased (punished mtop XHRs are CORS-refused after the
/// body arrives, so the whole call site went dark).
pub(super) fn record_failed_fetch(state: &OpState, f: &FetchFailure) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.network_response_body_counter += 1;
    // Same `fetch-{N}` id space as the success path (no body is stored under
    // it — there is none to retrieve); keeps ids unique across interleaved
    // failures and successes.
    let request_id = format!("fetch-{}", gs.network_response_body_counter);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    gs.js_network_events.push(JsNetworkEvent {
        request_id,
        url: f.url.clone(),
        method: f.method.clone(),
        status: 0,
        response_headers: HashMap::new(),
        request_headers: HashMap::new(),
        body_size: 0,
        timestamp,
        error: Some(f.reason.clone()),
        final_url: f.final_url.clone(),
        redirects: f.redirects.clone(),
        started: f.started,
    });
    const MAX_JS_NETWORK_EVENTS: usize = 4096;
    if gs.js_network_events.len() > MAX_JS_NETWORK_EVENTS {
        let overflow = gs.js_network_events.len() - MAX_JS_NETWORK_EVENTS;
        gs.js_network_events.drain(0..overflow);
    }
}

/// The driver's give-up horizon comes from the env knob
/// `DITING_SYNC_XHR_TIMEOUT_MS` with a 120 000ms default and 50ms
/// floor — shrinkable so the abandonment row is testable in CI.
pub(super) fn sync_xhr_timeout_ms() -> u64 {
    diting_env::env_knobs::sync_xhr_timeout_ms()
}

/// Synchronous twin of op_fetch_url (obscura#908): XHR.open(..., false) +
/// send() must issue the request and return with status populated, with no
/// event-loop turn in between. deno_core async ops only resolve when the
/// embedding pumps the event loop after the eval returns — impossible while
/// JS holds the thread inside send() — so the shared walk runs on a worker
/// thread with its own current-thread tokio runtime and this op blocks on
/// the reply channel.
///
/// Two divergences from the async path, both inherent to holding the JS
/// thread:
/// - CDP Fetch interception is skipped: a resolution cannot be delivered
///   while the dispatch that raised the request is parked here (the same
///   reason the async path's interception wait is bounded).
/// - The shared fetch client is touched from a second runtime.
#[diting_op]
#[string]
pub(super) fn op_fetch_url_sync(
    state: &OpState,
    #[string] url: String,
    #[string] method: String,
    #[string] headers_json: String,
    #[string] body: String,
    #[string] origin: String,
    #[string] mode: String,
    #[string] credentials: String,
) -> Result<String, deno_error::JsErrorBox> {
    // Same #229 entry stamp as the async driver — gather's short-circuit
    // rows and the walk's deps both read the dispatch time.
    let started_unix = now_unix();
    let (init, body_is_base64) =
        gather_fetch_parts(state, &url, &method, &headers_json, started_unix)
            .map_err(deno_error::JsErrorBox::generic)?;
    let FetchRequestInit {
        cookie_jar,
        in_flight,
        in_flight_list,
        proxy_url,
        http_client,
        callbacks,
        document_url,
        ..
    } = init;

    let body_bytes = if body_is_base64 {
        BASE64.decode(&body).unwrap_or_default()
    } else {
        body.into_bytes()
    };
    let mut custom_headers: std::collections::HashMap<String, String> =
        serde_json::from_str(&headers_json).unwrap_or_default();
    custom_headers.remove("__diting_body_b64");

    let initial_request_origin = request_origin(&url).unwrap_or_default();
    let page_origin = if origin.is_empty() {
        initial_request_origin
    } else {
        origin
    };

    let mut deps = FetchWalkDeps {
        url,
        method,
        custom_headers,
        body_bytes,
        page_origin,
        mode,
        credentials: FetchCredentials::parse(&credentials),
        cookie_jar,
        in_flight,
        in_flight_list,
        http_client,
        proxy_url,
        document_url,
        // XHR has no referrerPolicy surface (Fetch-only RequestInit); the
        // document's own policy (header / <meta name=referrer>) applies.
        referrer_policy: resolve_document_referrer_policy(state),
        referrer_init: "about:client".to_string(),
        callbacks,
        failures: Vec::new(),
        // #229: op entry = request dispatch. Stamped before the first hop
        // goes out so the HAR face can order concurrent requests by start,
        // not by which response landed first.
        started_unix,
    };

    let (tx, rx) = std::sync::mpsc::channel();
    // The driver can give up before the walk does: recv_timeout below fires
    // while the worker is still mid-walk (multi-hop slow hops outlive it),
    // and the worker's eventual failure replay then dies with the thread —
    // the request would vanish from in_flight with no terminal row (#84
    // observability: hung/failed requests must leave a trace). Captured
    // here so the abandonment row can be recorded on the timeout branch.
    let abandon_url = deps.url.clone();
    let abandon_method = deps.method.clone();
    let abandon_started = deps.started_unix;
    let timeout_ms = sync_xhr_timeout_ms();
    std::thread::Builder::new()
        .name("sync-xhr".to_string())
        .spawn(move || {
            let result = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt.block_on(fetch_url_walk(&mut deps)),
                Err(e) => Err(deno_error::JsErrorBox::generic(format!(
                    "sync XHR transport runtime: {}",
                    e
                ))),
            };
            let failures = deps.failures;
            let _ = tx.send((result, failures));
        })
        .map_err(|e| deno_error::JsErrorBox::generic(format!("sync XHR worker: {}", e)))?;

    let (result, failures) = match rx.recv_timeout(std::time::Duration::from_millis(timeout_ms)) {
        Ok(pair) => pair,
        Err(_) => {
            // #84: the walk may still be running on the worker — its own
            // failure replay will never be read (the channel's receiver
            // is gone). Record the abandonment as the request's terminal
            // state so /network keeps a row for it.
            record_failed_fetch(
                state,
                &FetchFailure {
                    url: abandon_url.clone(),
                    method: abandon_method.clone(),
                    reason: format!(
                        "synchronous XHR driver timed out after {timeout_ms}ms — \
                         the worker's late result is not logged"
                    ),
                    final_url: abandon_url,
                    redirects: Vec::new(),
                    started: abandon_started,
                },
            );
            return Err(deno_error::JsErrorBox::generic(
                "Synchronous XMLHttpRequest timed out or its worker died",
            ));
        }
    };
    replay_fetch_failures(state, &failures);
    let outcome = result?;
    if let Some(ev) = outcome.network.as_ref() {
        record_fetch_network_event(state, ev);
    }
    Ok(outcome.json)
}
