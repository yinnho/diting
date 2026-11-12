use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::rc::Rc;
use std::sync::{Arc, PoisonError};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use diting_dom::{AttachShadowError, DomTree, NodeData, NodeId, ShadowRootMode};
use diting_net::{CookieJar, HttpClient};
use diting_ops::diting_op;
use diting_runtime::state::OpState;
use url::Url;
#[cfg(feature = "screenshot")]
pub use layout_run::text_ink_extent;
pub use state::{InterceptResolution, InterceptedRequest, INTERCEPT_RESOLUTION_TIMEOUT_MS, ResourceTimingRecord, JsState, JsNetworkEvent, StoredNetworkResponseBody, SharedState};
pub(crate) use state::{BlobStoreMap, fetch_body_byte_limit};
use state::{response_body_entry_limit, response_body_byte_limit};
use script_start::{mark_script_subtree_started, propagate_script_start_state};
use dom::{parse_nid, DomCmd};
use dom_cmds::{dom_cmd_attr_state, dom_cmd_create, dom_cmd_document, dom_cmd_mutate, dom_cmd_query, dom_cmd_tree};
use dom_style::{dom_cmd_geometry, dom_cmd_scroll_style, dom_cmd_sheets};
#[cfg(feature = "screenshot")]
use layout_run::{ensure_layout_run, iframe_layout_run, is_element_scroller, is_viewport_fixed, parse_iframe_viewport, sticky_shifts, LayoutRun};
#[cfg(feature = "screenshot")]
use dom_style::{attr_write_is_layout_inert, style_write_is_paint_only};

/// Mirror a `URL.createObjectURL` registration into the Rust-side blob
/// table. The JS-side `__blobObjs` map is unreachable from the module
/// loader (which resolves outside the realm), so `import("blob:…")` and
/// `<script type=module src="blob:…">` resolve through this mirror.
#[diting_op]
fn op_blob_register(
    state: &OpState,
    #[string] id: String,
    #[buffer] bytes: &[u8],
    #[string] mime: String,
) {
    let shared = state.borrow::<SharedState>().clone();
    shared
        .borrow()
        .blob_store
        .borrow_mut()
        .insert(id, (bytes.to_vec(), mime));
}

/// Drop a mirrored blob registration (`URL.revokeObjectURL`).
#[diting_op]
fn op_blob_revoke(state: &OpState, #[string] id: String) {
    let shared = state.borrow::<SharedState>().clone();
    shared.borrow().blob_store.borrow_mut().remove(&id);
}

mod band;
mod canvas;
mod css;
mod ecdsa;
mod ed25519;
mod encoding;
mod fetch_gate;
mod fetch_walk;
mod node_order;
mod props;
mod text_geom;

#[cfg(feature = "screenshot")]
pub use band::BandFrame;
#[cfg(feature = "screenshot")]
pub use band::{
    band_frame, band_frame_cut, band_frame_cut_with_text, band_frame_scaled, band_frame_with_text,
};
pub(crate) use canvas::{op_canvas_jpeg, op_canvas_png};
#[cfg(feature = "screenshot")]
pub(crate) use canvas::{op_canvas_store, op_canvas_text};
pub(crate) use ecdsa::{
    op_subtle_ecdh_derive_bits, op_subtle_ecdsa_generate, op_subtle_ecdsa_import_private,
    op_subtle_ecdsa_import_public, op_subtle_ecdsa_sign, op_subtle_ecdsa_verify,
};
pub(crate) use ed25519::{
    op_subtle_ed25519_check, op_subtle_ed25519_public, op_subtle_ed25519_sign,
    op_subtle_ed25519_verify,
};
pub(crate) use fetch_walk::{fetch_url_walk, FetchFailure, FetchNetworkEvent, FetchWalkDeps};
use node_order::{compare_node_order, node_child_index};
mod cors;
mod crypto_secret;
mod referrer;
use referrer::resolve_document_referrer_policy;
pub use referrer::{fetch_referer, last_valid_referrer_token, resolve_referrer_policy_from};
mod fetch_client;
#[cfg(feature = "screenshot")]
use cssom_value::{cb_inline_for, computed_style_value};
pub use fetch_client::cached_request_client;
pub(crate) use fetch_client::select_request_client;
mod cssom_value;
mod image_info;
pub(crate) use cors::request_origin;
pub(crate) use crypto_secret::{
    crypto_err, op_random_bytes, op_subtle_aes_cbc, op_subtle_aes_ctr, op_subtle_aes_gcm,
    op_subtle_aes_kw, op_subtle_digest, op_subtle_hkdf, op_subtle_hmac, pbkdf2_derive,
};
pub use fetch_gate::validate_fetch_url;
pub(crate) use image_info::op_image_info;
mod video_info;
pub(crate) use video_info::op_video_info;
mod crypto_rsa;
pub(crate) use crypto_rsa::{
    op_subtle_rsa_import_pkcs8, op_subtle_rsa_import_private, op_subtle_rsa_import_public,
    op_subtle_rsa_import_spki, op_subtle_rsa_pkcs1v15_verify, op_subtle_rsa_pss_verify,
    rsa_generate_flat, rsa_oaep, rsa_pkcs1v15_sign, rsa_pss_sign,
};

/// Insert a fetched image body into [`JsState::image_bytes`] (FIFO eviction
/// at the entry cap) and drop `layout_cache`: placeholder boxes and real
/// intrinsic sizes can reflow differently, so memoized geometry from the
/// pre-fetch run must not survive the insert.
#[cfg(feature = "screenshot")]
pub fn store_image_bytes(gs: &mut JsState, url: String, bytes: Vec<u8>) {
    const IMAGE_TABLE_CAP: usize = 64;
    let mut map = gs.image_bytes.borrow_mut();
    if map.contains_key(&url) {
        return;
    }
    if map.len() >= IMAGE_TABLE_CAP {
        if let Some(oldest) = gs.image_order.borrow_mut().pop_front() {
            map.remove(&oldest);
        }
    }
    map.insert(url.clone(), std::sync::Arc::new(bytes));
    drop(map);
    gs.image_order.borrow_mut().push_back(url);
    gs.drop_layout();
}

#[diting_op]
fn op_console_msg(state: &OpState, #[string] level: &str, #[string] msg: &str) {
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
fn op_dialog(state: &OpState, #[string] kind: &str, #[string] message: &str) -> String {
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

    fn allows(self, page_origin: &str, request_url: &str) -> bool {
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
struct FetchRequestInit {
    cookie_jar: Option<Arc<CookieJar>>,
    in_flight: Option<Arc<std::sync::atomic::AtomicU32>>,
    in_flight_list: Option<std::sync::Arc<std::sync::Mutex<Vec<diting_net::InFlightScripted>>>>,
    proxy_url: Option<String>,
    http_client: Option<Arc<HttpClient>>,
    callbacks: Option<std::sync::Arc<diting_net::CallbackRegistry>>,
    document_url: String,
    intercept_tx: Option<tokio::sync::mpsc::UnboundedSender<InterceptedRequest>>,
}

/// Shared front half of op_fetch_url and op_fetch_url_sync: the SSRF gate,
/// the base64-body marker probe, and the OpState clones both drivers hand
/// to the walk. Returns Err(the exact JSON the op must return) for the two
/// short-circuits (SSRF reject, setBlockedURLs pattern), each recorded into
/// the network-event log first.
fn gather_fetch_parts(
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
            document_url: gs.url.clone(),
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
fn record_fetch_network_event(state: &OpState, ev: &FetchNetworkEvent) -> String {
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
fn replay_fetch_failures(state: &OpState, failures: &[FetchFailure]) {
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
    deque: std::collections::VecDeque<(String, String)>,
    total_bytes: usize,
}

const FETCH_SETTLEMENT_MAX_BYTES: usize = 8 * 1024 * 1024;
const FETCH_SETTLEMENT_MAX_ENTRIES: usize = 64;

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
fn now_unix() -> f64 {
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
fn push_fetch_settlement(
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
fn op_fetch_take(state: &mut OpState, #[string] key: String) -> String {
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
const REFERRER_POLICY_TOKENS: [&str; 7] = [
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
fn record_failed_fetch(state: &OpState, f: &FetchFailure) {
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
fn sync_xhr_timeout_ms() -> u64 {
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
fn op_fetch_url_sync(
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

#[diting_op]
#[string]
fn op_get_cookies(state: &OpState) -> String {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    let jar = match &gs.cookie_jar {
        Some(j) => j,
        None => return String::new(),
    };
    let url = match url::Url::parse(&gs.url) {
        Ok(u) => u,
        Err(_) => return String::new(),
    };
    jar.get_js_visible_cookies(&url)
}

#[diting_op]
fn op_set_cookie(state: &OpState, #[string] cookie_str: &str) {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    let jar = match &gs.cookie_jar {
        Some(j) => j,
        None => return,
    };
    let url = match url::Url::parse(&gs.url) {
        Ok(u) => u,
        Err(_) => return,
    };
    jar.set_cookie_from_js(cookie_str, &url);
}

// localStorage persistence (obscura#629 class): the bootstrap store is a
// plain JS map, so logins/flags a page keeps in localStorage died with the
// process. The store now loads through op_storage_read on first access and
// flushes (debounced JS-side) through op_storage_write; the authoritative
// copy lives in this process-global map, mirrored to one JSON file per
// origin under DITING_STORAGE_DIR (falling back to the cookie store
// dir). sessionStorage deliberately stays memory-only — per-tab lifetime is
// the spec'd behavior. Both locks recover from poison (#336): they are
// process-wide, and the guarded sections are single insert/get calls.
static LOCAL_STORAGE: std::sync::LazyLock<
    std::sync::Mutex<HashMap<String, HashMap<String, String>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
static LOCAL_STORAGE_LOADED: std::sync::LazyLock<std::sync::Mutex<HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));

fn storage_file(origin: &str) -> Option<std::path::PathBuf> {
    if diting_env::env_knobs::ephemeral() {
        return None;
    }
    let dir = std::env::var("DITING_STORAGE_DIR")
        .ok()
        .or_else(|| std::env::var("DITING_COOKIE_STORE_DIR").ok())
        .or_else(|| diting_env::env_knobs::app_data_dir().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| ".".to_string());
    let mut sanitized = String::with_capacity(origin.len());
    for c in origin.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }
    // FNV-1a suffix: sanitized names collide ("a:b" vs "a_b"); the hash does not.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in origin.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(
        std::path::Path::new(&dir)
            .join("localStorage")
            .join(format!("{sanitized}-{:016x}.json", hash)),
    )
}

fn storage_load_locked(origin: &str) {
    let mut loaded = LOCAL_STORAGE_LOADED
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if !loaded.insert(origin.to_string()) {
        return;
    }
    let Some(path) = storage_file(origin) else {
        return;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(entries) = serde_json::from_str::<HashMap<String, String>>(&raw) else {
        return;
    };
    LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(origin.to_string(), entries);
}

#[diting_op]
#[string]
fn op_storage_read(#[string] origin: &str) -> String {
    storage_load_locked(origin);
    let map = LOCAL_STORAGE.lock().unwrap_or_else(PoisonError::into_inner);
    let entries = map.get(origin);
    serde_json::to_string(&entries.unwrap_or(&HashMap::new())).unwrap_or_else(|_| "{}".to_string())
}

#[diting_op]
fn op_storage_write(#[string] origin: &str, #[string] json: &str) {
    let Ok(entries) = serde_json::from_str::<HashMap<String, String>>(json) else {
        return;
    };
    // Load-first: a write for a never-read origin must not be clobbered by
    // the on-disk copy when the reader side initializes later.
    storage_load_locked(origin);
    LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(origin.to_string(), entries);
    let Some(path) = storage_file(origin) else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            // An unwritable store must be loud (obscura#855 item 1 shape):
            // silently returning here loses every subsequent localStorage
            // write with zero diagnostics.
            tracing::warn!(
                "localStorage persist: create_dir_all({}) failed: {}",
                parent.display(),
                e
            );
            return;
        }
    }
    let payload = LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(origin)
        .map(|entries| serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string()));
    let Some(payload) = payload else { return };
    // tmp + rename so a mid-write kill never leaves a truncated store;
    // 0600 to match the cookie store — this file carries login tokens too
    // (the xinzao-class session shape).
    let tmp = path.with_extension("tmp");
    #[cfg(unix)]
    let write_ok = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .and_then(|mut f| f.write_all(payload.as_bytes()))
            .is_ok()
    };
    // Windows has no mode bits; the file lives in the per-user app-data
    // directory (config::app_data_dir), whose ACLs already scope it to the
    // user.
    #[cfg(not(unix))]
    let write_ok = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(payload.as_bytes()))
        .is_ok();
    if write_ok {
        if let Err(e) = std::fs::rename(&tmp, &path) {
            tracing::warn!(
                "localStorage persist: rename to {} failed: {}",
                path.display(),
                e
            );
        }
    } else {
        tracing::warn!("localStorage persist: write to {} failed", tmp.display());
    }
}

#[cfg(test)]
pub(crate) fn reset_local_storage_for_tests() {
    LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    LOCAL_STORAGE_LOADED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

#[diting_op]
fn op_navigate(state: &OpState, #[string] url: &str, #[string] method: &str, #[string] body: &str) {
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
fn op_history_jump_to(state: &OpState, target: i32) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.pending_history_jump = Some(target);
}

/// Free body of `op_sleep` — the hand glue (`diting_js::async_glue`)
/// calls this directly off our Vm host (10d swap; the #[op2] wrapper died
/// with the deno host).
pub(crate) async fn op_sleep_body(millis: u64) {
    // #230: tokio's timer wheel ticks at 1ms granularity — even a
    // zero-length sleep parks until the next wheel tick before its
    // deadline reads as due, taxing every delay-0 round-trip with ~1ms of
    // pure wheel latency (measured 1154µs/trip while the JS work under it
    // cost 55ns/item: the macrotask trampoline takes one trip per 64
    // items, so a 340k-timer fxg boot drained at ~5.7k/s = 59s). A zero
    // sleep completes at the first drive instead — settlement still rides
    // the settle queue into the next loop pass, so the task-boundary
    // discipline (#37/#327 line: never settle on the JS stack) is intact;
    // it just doesn't wait for a wheel tick to say "due".
    if millis == 0 {
        return;
    }
    // Reactor-less contexts (plain #[test] isolates): a tokio timer would
    // panic mid-poll inside v8, which aborts the whole test binary. Resolve
    // at the next event-loop checkpoint instead — production always runs
    // under a reactor and never takes this path.
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
}

// Records a binding call from page JS. The CDP layer drains this queue
// after every dispatch and emits one `Runtime.bindingCalled` event per
// entry, that's how puppeteer's `page.exposeFunction` callbacks fire.
#[diting_op]
fn op_binding_called(state: &OpState, #[string] name: &str, #[string] payload: &str) {
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

/// Serialize a parsed URL into the WHATWG IDL component shape consumed by the
/// `URL` class in bootstrap.js. Getters read these fields directly so no op
/// call happens per property access.
fn url_components(u: &url::Url) -> serde_json::Value {
    let port = u.port().map(|p| p.to_string()).unwrap_or_default();
    let hostname = u.host_str().unwrap_or("").to_string();
    let host = if hostname.is_empty() {
        String::new()
    } else if port.is_empty() {
        hostname.clone()
    } else {
        format!("{hostname}:{port}")
    };
    // WHATWG search/hash getters return "" for a null OR empty component.
    let search = match u.query() {
        Some(q) if !q.is_empty() => format!("?{q}"),
        _ => String::new(),
    };
    let hash = match u.fragment() {
        Some(f) if !f.is_empty() => format!("#{f}"),
        _ => String::new(),
    };
    serde_json::json!({
        "ok": true,
        "href": u.as_str(),
        "protocol": format!("{}:", u.scheme()),
        "username": u.username(),
        "password": u.password().unwrap_or(""),
        "host": host,
        "hostname": hostname,
        "port": port,
        "pathname": u.path(),
        "search": search,
        "hash": hash,
        "origin": u.origin().ascii_serialization(),
    })
}

/// Parse `href` (optionally resolved against `base`) with the WHATWG-compliant
/// `url` crate. Returns the component JSON, or `{"ok":false}` when the input is
/// not a valid URL (the JS side turns that into a TypeError, per spec).
#[diting_op]
#[string]
fn op_url_parse(#[string] href: &str, #[string] base: &str) -> String {
    // The url crate can panic on a few pathological inputs (internal range
    // slicing); catch it so a bad URL never aborts the process.
    std::panic::catch_unwind(|| {
        let parsed = if base.is_empty() {
            url::Url::parse(href)
        } else {
            url::Url::parse(base).and_then(|b| b.join(href))
        };
        match parsed {
            Ok(u) => url_components(&u).to_string(),
            Err(_) => "{\"ok\":false}".to_string(),
        }
    })
    .unwrap_or_else(|_| "{\"ok\":false}".to_string())
}

/// Apply a WHATWG URL setter (`part` = href/protocol/username/password/host/
/// hostname/port/pathname/search/hash) to `href` and return the new components.
fn url_set_inner(href: &str, part: &str, value: &str) -> Option<serde_json::Value> {
    let mut u = url::Url::parse(href).ok()?;
    match part {
        "href" => {
            let nu = url::Url::parse(value).ok()?;
            return Some(url_components(&nu));
        }
        "protocol" => {
            let _ = u.set_scheme(value.trim_end_matches(':'));
        }
        "username" => {
            let _ = u.set_username(value);
        }
        "password" => {
            let _ = u.set_password(if value.is_empty() { None } else { Some(value) });
        }
        "host" => set_host_port(&mut u, value),
        "hostname" => {
            if !value.is_empty() {
                let _ = u.set_host(Some(value));
            }
        }
        "port" => {
            // WHATWG port state consumes the leading ASCII digits and stops
            // at the first non-digit, so "8080abc" sets 8080; no digits or a
            // number above 65535 leaves the port untouched.
            if value.is_empty() {
                let _ = u.set_port(None);
            } else {
                let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(p) = digits.parse::<u32>() {
                    if p <= 65535 {
                        let _ = u.set_port(Some(p as u16));
                    }
                }
            }
        }
        "pathname" => u.set_path(value),
        "search" => {
            // Null only when the ORIGINAL value is the empty string; a bare
            // "?" sets an empty query that still serializes its delimiter
            // (href ends in "?").
            let q = value.strip_prefix('?').unwrap_or(value);
            u.set_query(if value.is_empty() { None } else { Some(q) });
        }
        "hash" => {
            let f = value.strip_prefix('#').unwrap_or(value);
            u.set_fragment(if value.is_empty() { None } else { Some(f) });
        }
        _ => {}
    }
    Some(url_components(&u))
}

#[diting_op]
#[string]
fn op_url_set(#[string] href: &str, #[string] part: &str, #[string] value: &str) -> String {
    // Some url-crate setters panic on pathological inputs (the url-setters WPT
    // tests exercise these). Catch the unwind and treat it as a no-op setter,
    // returning the URL unchanged, which matches WHATWG "do nothing on invalid".
    match std::panic::catch_unwind(|| url_set_inner(href, part, value)) {
        Ok(Some(v)) => v.to_string(),
        _ => match url::Url::parse(href) {
            Ok(u) => url_components(&u).to_string(),
            Err(_) => "{\"ok\":false}".to_string(),
        },
    }
}

/// Best-effort `host` setter: split `host[:port]` (handling bracketed IPv6) and
/// apply hostname and port separately, since `url::Url::set_host` rejects a port.
fn set_host_port(u: &mut url::Url, value: &str) {
    // IPv6 literals are bracketed; never split inside the brackets.
    if value.starts_with('[') {
        if let Some(close) = value.find(']') {
            let host = &value[..=close];
            let rest = &value[close + 1..];
            if u.set_host(Some(host)).is_ok() {
                if let Some(p) = rest.strip_prefix(':') {
                    if let Ok(pn) = p.parse::<u16>() {
                        let _ = u.set_port(Some(pn));
                    }
                }
            }
            return;
        }
    }
    if let Some(idx) = value.rfind(':') {
        let (h, p) = (&value[..idx], &value[idx + 1..]);
        if p.is_empty() || p.chars().all(|c| c.is_ascii_digit()) {
            if u.set_host(Some(h)).is_ok() {
                if p.is_empty() {
                    let _ = u.set_port(None);
                } else if let Ok(pn) = p.parse::<u16>() {
                    let _ = u.set_port(Some(pn));
                }
            }
            return;
        }
    }
    let _ = u.set_host(Some(value));
}

/// Resolve `href` against optional `base` and return only the serialized
/// absolute URL (no component breakdown). Used by the hot `a.href`/`area.href`
/// getter, which only needs the resolved string, so it avoids building and
/// re-parsing the full component JSON. Returns "" when the input is invalid.
#[diting_op]
#[string]
fn op_url_resolve(#[string] href: &str, #[string] base: &str) -> String {
    std::panic::catch_unwind(|| {
        let parsed = if base.is_empty() {
            url::Url::parse(href)
        } else {
            url::Url::parse(base).and_then(|b| b.join(href))
        };
        parsed.map(|u| u.as_str().to_string()).unwrap_or_default()
    })
    .unwrap_or_default()
}

/// Parse and merge an inline document import map (upstream 34373c3). Returns
/// "" on success or the parse/merge error message, so the bootstrap caller can
/// surface it as a script error event without a rejected-op round-trip.
#[diting_op]
#[string]
fn op_add_import_map(
    state: &OpState,
    #[string] source: String,
    #[string] base_url: String,
) -> String {
    let shared = state.borrow::<SharedState>().clone();
    let import_map = shared.borrow().import_map.clone();
    let parsed = match crate::import_map::ImportMap::parse(&source, &base_url) {
        Ok(map) => map,
        Err(error) => return error,
    };
    let result = match import_map.try_borrow_mut() {
        Ok(mut current) => {
            current.merge(parsed);
            String::new()
        }
        Err(_) => "Import map is already borrowed".to_string(),
    };
    result
}

/// 批240 / obscura#1100: SVGGeometryElement.getTotalLength() backing. Chrome
/// measures through SkPathMeasure (circle r=25 → 156.0674, not 2πr), so these
/// numbers line up with Chrome (conic chord measure for ovals, analytic for
/// explicit d beziers). Non-geometry nodes / missing geometry read 0.
/// Anti-panic boundary: the path data comes from the page, and a parser panic
/// must degrade to 0 (the op_dom posture) rather than unwind through the FFI
/// frame. Measurement lives in the layout crate, so the no-screenshot build
/// reads 0 — geometry measurement without the layout engine is meaningless
/// anyway.
#[cfg(feature = "screenshot")]
#[diting_op]
fn op_svg_path_len(state: &OpState, #[smi] nid: u32) -> f64 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        svg_measure(state, nid)
            .map(|m| m.total() as f64)
            .unwrap_or(0.0)
    }))
    .unwrap_or(0.0)
}

#[cfg(not(feature = "screenshot"))]
#[diting_op]
fn op_svg_path_len(_: &OpState, #[smi] _: u32) -> f64 {
    0.0
}

/// getPointAtLength backing — "x\0y" (cheap unambiguous pair; the bootstrap
/// side splits and wraps it in an SVGPoint). Negative/overlong targets
/// clamp to the endpoints, Chrome's contract.
#[cfg(feature = "screenshot")]
#[diting_op]
#[string]
fn op_svg_path_point(state: &OpState, #[smi] nid: u32, target: f64) -> String {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        svg_measure(state, nid).map(|m| m.point_at(target as f32))
    }))
    .unwrap_or(None);
    match r {
        Some((x, y)) if x.is_finite() && y.is_finite() => format!("{x}\0{y}"),
        _ => "0\u{0}0".to_string(),
    }
}

#[cfg(not(feature = "screenshot"))]
#[diting_op]
#[string]
fn op_svg_path_point(_: &OpState, #[smi] _: u32, _: f64) -> String {
    "0\u{0}0".to_string()
}

#[cfg(feature = "screenshot")]
fn svg_measure(state: &OpState, nid: u32) -> Option<diting_layout::svg::PathMeasure> {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    // `&gs.dom?` is the lint's shape but moves out of the RefCell borrow;
    // `as_ref()` keeps the Option borrowed.
    let dom = gs.dom.as_ref()?;
    let node = dom.get_node(NodeId::new(nid))?;
    let tag = match &node.data {
        diting_dom::NodeData::Element { name, .. } => name.local.as_str(),
        _ => return None,
    };
    diting_layout::svg::build_path_measure(tag, &|a| node.get_attribute(a).map(|s| s.to_string()))
}

/// (#78) Sub-ms wall clock for performance.now(). Chrome coarsens the timer
/// to 100µs (non-crossOriginIsolated) and reports floats; an integral
/// Date.now()-based value is a timing-entropy tell on the jsvmp environment
/// scan path. Quantized to 0.1ms steps — epoch-ms times ten stays well under
/// 2^53, so the f64 rounding is exact (finer quantization would not be).
#[diting_op]
fn op_clock_ms() -> f64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0;
    (ms * 10.0).round() / 10.0
}

/// The op table our own host installs (10d flip): every #[diting_op]
/// wrapper already returns OUR OpDecl, and the four async ops come in as
/// hand glue from `diting_js::async_glue` (op_fetch_url / op_sleep /
/// op_ws_open / op_ws_next_message). The list mirrors the retired
/// build_extension() entry-for-entry; the to_deno adapter and the
/// Extension wrapper died with the deno host.
pub fn our_decls() -> Vec<diting_runtime::op_reg::OpDecl> {
    let mut decls = vec![
        css::op_css_supports(),
        props::op_register_property(),
        dom::op_dom(),
        op_clock_ms(),
        op_svg_path_len(),
        op_svg_path_point(),
        dom::op_shadow_attach(),
        op_console_msg(),
        op_dialog(),
        script_start::op_script_mark_started(),
        script_start::op_script_try_start(),
        script_start::op_dyn_script_fetch_begin(),
        script_start::op_dyn_script_fetch_end(),
        op_blob_register(),
        op_blob_revoke(),
        op_fetch_take(),
        op_fetch_url_sync(),
        op_image_info(),
        op_video_info(),
        op_canvas_png(),
        op_canvas_jpeg(),
        #[cfg(feature = "screenshot")]
        op_canvas_store(),
        #[cfg(feature = "screenshot")]
        op_canvas_text(),
        op_get_cookies(),
        op_set_cookie(),
        op_storage_read(),
        op_storage_write(),
        op_navigate(),
        op_history_jump_to(),
        op_binding_called(),
        op_subtle_digest(),
        op_subtle_hmac(),
        op_subtle_aes_gcm(),
        op_subtle_aes_cbc(),
        op_subtle_aes_ctr(),
        op_subtle_aes_kw(),
        op_subtle_rsa_import_public(),
        op_subtle_rsa_import_spki(),
        op_subtle_rsa_import_private(),
        op_subtle_rsa_import_pkcs8(),
        op_subtle_rsa_pkcs1v15_verify(),
        op_subtle_rsa_pss_verify(),
        op_subtle_hkdf(),
        op_subtle_ecdsa_generate(),
        op_subtle_ecdsa_import_private(),
        op_subtle_ecdsa_import_public(),
        op_subtle_ecdh_derive_bits(),
        op_subtle_ecdsa_sign(),
        op_subtle_ecdsa_verify(),
        op_subtle_ed25519_check(),
        op_subtle_ed25519_public(),
        op_subtle_ed25519_sign(),
        op_subtle_ed25519_verify(),
        op_random_bytes(),
        op_url_parse(),
        op_url_set(),
        op_url_resolve(),
        op_add_import_map(),
        encoding::op_encoding_for_label(),
        encoding::op_text_decode(),
        encoding::op_url_encode_query(),
        crate::ws::op_ws_send(),
        crate::ws::op_ws_close(),
    ];
    // The async four ride the hand glue (promise stub + OpDriver spawn).
    decls.extend(crate::async_glue::all_decls());
    decls
}

mod state;
mod script_start;
mod dom;
mod dom_cmds;
mod dom_style;
mod layout_run;
#[cfg(test)]
mod tests;
