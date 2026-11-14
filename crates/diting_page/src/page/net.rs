//! Network bookkeeping: event recording (page-side and drained script-side
//! rows), retained response bodies, scoped on_request/on_response callbacks
//! and Network.setBlockedURLs, plus the identity overrides (extra headers,
//! user-agent override) CDP's Network/Emulation domains drive. Split from
//! page/mod.rs (ARCHITECTURE.md P2 batch 3); behavior unchanged. This
//! module names the page log and the body-size caps it stores against.

use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use super::Page;

/// One recorded network exchange (CDP `Network.requestWillBeSent` /
/// `responseReceived` shape). Recorded for every document + subresource
/// fetch and drained from the JS runtime for script-initiated requests;
/// read by the session /network and /har surfaces.
#[derive(Debug, Clone)]
pub struct NetworkEvent {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    /// Navigation generation this row belongs to (#101): the epoch that
    /// was live when the request was issued. `Page.navigation_epoch` is
    /// the current generation — rows whose `nav` is lower are leftovers of
    /// earlier navigation attempts (a timed-out attempt's log is NOT
    /// dropped until the next navigation resets it).
    pub nav: u64,
    pub status: u16,
    /// The REQUEST's outbound header set, lowercased (#97) — what the
    /// CDP `Network.requestWillBeSent` params and the HAR `request.headers`
    /// read. Populated from the transport's wire snapshot (document and
    /// subresource rows) or the fetch walk's hop set (script rows).
    pub headers: std::collections::HashMap<String, String>,
    pub response_headers: Arc<std::collections::HashMap<String, String>>,
    pub body_size: usize,
    pub timestamp: f64,
    /// Set on `status: 0` script-initiated rows that never produced a
    /// servable response (SSRF block, CORS refusal, transport failure).
    /// Navigation/subresource rows leave it `None`.
    pub error: Option<String>,
    /// Where a redirected request actually landed (#203). `url` keeps the
    /// REQUESTED address on every row — a taobao submit that bounced to
    /// error.taobao.com used to read as "never issued" because failure rows
    /// carried only the final target. Empty (or equal to `url`) when the
    /// request never redirected; navigation/subresource rows leave it empty
    /// (their face is the page-level `redirect_chain`, #102).
    pub final_url: String,
    /// The redirect hops followed, in order, each with the 3xx status that
    /// sent it there (#203). Empty when the request went straight through.
    pub redirects: Vec<diting_net::RedirectHop>,
    /// When the request was DISPATCHED (#229). `None` on
    /// navigation/subresource rows (their `timestamp` already stamps early
    /// in the lifecycle); `Some` on script rows, whose `timestamp` is
    /// response completion. The HAR face reads the pair as
    /// startedDateTime + time.
    pub started: Option<f64>,
}

/// A response body retained for `get_response_body` (upstream #360). Bodies
/// are classified with the Chromium DevTools policy (see
/// `diting_net::decode_devtools_body`): replacement-free text is stored as a
/// string (`base64_encoded = false`, declared-GBK pages included, matching
/// Chrome); opaque or undecodable bodies are stored base64 so
/// `take_response_body_raw` is byte-exact.
#[derive(Debug, Clone)]
pub struct StoredResponseBody {
    pub body: String,
    pub base64_encoded: bool,
}

fn response_body_entry_limit() -> usize {
    std::env::var("DITING_NETWORK_BODY_BUFFER_ENTRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128)
}

fn response_body_byte_limit() -> usize {
    std::env::var("DITING_NETWORK_BODY_BUFFER_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2 * 1024 * 1024)
}

/// One navigation's request log, copied out so the caller can keep using
/// the page (response bodies, the URL) while it formats the log.
pub struct NavigationRecord {
    pub events: Vec<NetworkEvent>,
    pub epoch: u64,
    pub redirect_chain: Vec<String>,
}

impl Page {
    /// Script-initiated fetch()/XHR hops currently flying (#116): url,
    /// method, dispatch epoch-ms. The answer to "my eval timed out — is
    /// the save still executing?" — a hung request stays listed until its
    /// transport actually ends, instead of being invisible until then.
    pub fn scripted_in_flight(&self) -> Vec<diting_net::InFlightScripted> {
        self.http_client.scripted_in_flight()
    }

    /// Hard block from `Network.setBlockedURLs`: matched resources fail
    /// outright (Chrome semantics — no pause, no client round trip).
    /// `pub(crate)`: render-path fetchers outside this module (screenshot
    /// prefetch, pptx_native image export) share the same hard block.
    pub fn url_blocked(&self, url: &str) -> bool {
        if self.blocked_urls.is_empty() {
            return false;
        }
        self.blocked_urls
            .iter()
            .any(|p| diting_net::url_pattern_matches(p, url))
    }

    /// Arm / disarm the Fetch-domain interception kernel. `Some(tx)` routes
    /// every script-initiated fetch()/XHR through the CDP bridge as an
    /// `InterceptedRequest` (the bridge answers via the per-request oneshot);
    /// `None` disarms and lets in-flight resolvers fall through to the real
    /// request. Arming survives navigation — `init_js` carries the channel
    /// into each rebuilt realm.
    pub fn set_fetch_intercept(
        &mut self,
        tx: Option<tokio::sync::mpsc::UnboundedSender<diting_js::ops::InterceptedRequest>>,
    ) {
        self.intercept_tx = tx.clone();
        if let Some(js) = &self.js {
            match tx {
                Some(tx) => {
                    js.set_intercept_tx(tx);
                    js.set_intercept_enabled(true);
                }
                None => js.set_intercept_enabled(false),
            }
        }
    }

    pub fn set_blocked_urls(&mut self, patterns: Vec<String>) {
        self.blocked_urls = patterns.clone();
        if let Some(js) = &self.js {
            js.set_blocked_urls(patterns);
        }
    }

    // 8 args mirrors the CDP event shape (url/method/type/status/response
    // set/#97 request set/size) — a params struct would split one record
    // across two types for every caller.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_network_event(
        &mut self,
        url: &str,
        method: &str,
        resource_type: &str,
        status: u16,
        response_headers: &std::collections::HashMap<String, String>,
        request_headers: &std::collections::HashMap<String, String>,
        body_size: usize,
    ) -> String {
        self.network_event_counter += 1;
        let request_id = format!("{}.{}", self.id, self.network_event_counter);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.network_events.push(NetworkEvent {
            request_id: request_id.clone(),
            url: url.to_string(),
            method: method.to_string(),
            resource_type: resource_type.to_string(),
            nav: self.navigation_epoch,
            status,
            headers: request_headers.clone(),
            response_headers: Arc::new(response_headers.clone()),
            body_size,
            timestamp,
            error: None,
            // Navigation/subresource rows don't track hops here (the
            // document's trail lives on the page-level `redirect_chain`,
            // #102); script rows get theirs from sync_js_network_events.
            final_url: String::new(),
            redirects: Vec::new(),
            started: None,
        });
        request_id
    }

    /// Record the event and retain the body for `get_response_body`. The
    /// text/base64 split follows the Chromium DevTools body policy (Chrome
    /// 152 verified, obscura #791).
    #[allow(clippy::too_many_arguments)] // same record shape as above, body instead of size
    pub(super) fn record_network_event_with_body(
        &mut self,
        url: &str,
        method: &str,
        resource_type: &str,
        status: u16,
        response_headers: &std::collections::HashMap<String, String>,
        request_headers: &std::collections::HashMap<String, String>,
        body: &[u8],
    ) -> String {
        let request_id = self.record_network_event(
            url,
            method,
            resource_type,
            status,
            response_headers,
            request_headers,
            body.len(),
        );
        self.store_response_body(
            request_id.clone(),
            body,
            response_headers.get("content-type").map(|s| s.as_str()),
        );
        request_id
    }

    fn store_response_body(&mut self, request_id: String, body: &[u8], content_type: Option<&str>) {
        let max_entries = response_body_entry_limit();
        let max_bytes = response_body_byte_limit();
        if max_entries == 0 || max_bytes == 0 || body.len() > max_bytes {
            return;
        }
        let stored = match diting_net::decode_devtools_body(body, content_type) {
            Some(text) => StoredResponseBody {
                body: text,
                base64_encoded: false,
            },
            None => StoredResponseBody {
                body: BASE64.encode(body),
                base64_encoded: true,
            },
        };
        self.response_bodies.insert(request_id.clone(), stored);
        self.response_body_order.push_back(request_id);
        while self.response_body_order.len() > max_entries {
            if let Some(oldest) = self.response_body_order.pop_front() {
                self.response_bodies.remove(&oldest);
            }
        }
    }

    /// Body stored for a request id: page-side (`{page}.{N}`) or
    /// script-initiated (`fetch-{N}`, retained in the JS runtime).
    pub fn get_response_body(&self, request_id: &str) -> Option<StoredResponseBody> {
        self.response_bodies.get(request_id).cloned().or_else(|| {
            self.js
                .as_ref()?
                .get_network_response_body(request_id)
                .map(|body| StoredResponseBody {
                    body: body.body,
                    base64_encoded: body.base64_encoded,
                })
        })
    }

    /// Take a stored response body as raw bytes for CDP streaming
    /// (Fetch.takeResponseBodyAsStream). Removes it from the page-side cache
    /// and transfers ownership to the caller, so a large body is held once
    /// and freed when the stream is closed rather than lingering in this
    /// long-running process (upstream #360). Binary bodies are stored base64
    /// (byte-exact); text bodies return their UTF-8 bytes. Returns None if
    /// the body was never cached (e.g. it exceeded
    /// DITING_NETWORK_BODY_BUFFER_BYTES) or the id is unknown.
    pub fn take_response_body_raw(&mut self, request_id: &str) -> Option<Vec<u8>> {
        let stored = if let Some(body) = self.response_bodies.remove(request_id) {
            self.response_body_order.retain(|id| id != request_id);
            body
        } else {
            self.js
                .as_ref()?
                .get_network_response_body(request_id)
                .map(|b| StoredResponseBody {
                    body: b.body,
                    base64_encoded: b.base64_encoded,
                })?
        };
        if stored.base64_encoded {
            BASE64.decode(stored.body.as_bytes()).ok()
        } else {
            Some(stored.body.into_bytes())
        }
    }

    /// Make the body stored under `from_id` also retrievable under `to_id`.
    /// The main navigation resource is stored under its internal request id,
    /// but the CDP layer reports it with the navigation's loaderId as the
    /// requestId (Chrome's `requestId === loaderId` convention). Without this
    /// alias, `Network.getResponseBody(loaderId)` misses (upstream #340).
    pub fn alias_response_body(&mut self, from_id: &str, to_id: &str) {
        if from_id == to_id || self.response_bodies.contains_key(to_id) {
            return;
        }
        if let Some(body) = self.response_bodies.get(from_id).cloned() {
            self.response_bodies.insert(to_id.to_string(), body);
            self.response_body_order.push_back(to_id.to_string());
        }
    }

    pub fn clear_response_bodies(&mut self) {
        self.response_bodies.clear();
        self.response_body_order.clear();
        if let Some(js) = &self.js {
            js.clear_network_response_bodies();
        }
    }

    /// Move network events recorded for script-initiated requests
    /// (fetch/XHR) from the JS runtime into this page's `network_events`, so
    /// the CDP layer emits Network.requestWillBeSent / responseReceived for
    /// them (upstream #406). Idempotent: the runtime's queue is drained. The
    /// `fetch-{N}` request id is preserved so get_response_body resolves.
    /// The session /network and /har surfaces call this before reading.
    pub fn sync_js_network_events(&mut self) {
        let events = match self.js.as_ref() {
            Some(js) => js.take_js_network_events(),
            None => return,
        };
        for ev in events {
            self.network_events.push(NetworkEvent {
                request_id: ev.request_id,
                url: ev.url,
                method: ev.method,
                resource_type: "Fetch".to_string(),
                // Stamped at drain time with the generation then current:
                // rows the live realm issued get its epoch, and the drain
                // at the top of navigate_single (before the epoch bump)
                // attributes the outgoing document's rows to it (#101).
                nav: self.navigation_epoch,
                status: ev.status,
                headers: ev.request_headers,
                response_headers: Arc::new(ev.response_headers),
                body_size: ev.body_size,
                timestamp: ev.timestamp,
                error: ev.error,
                // #203: the requested url rides in `url`; where it landed
                // and the hops it took follow it onto the page-level rows.
                final_url: ev.final_url,
                redirects: ev.redirects,
                // #229: script rows know their dispatch time; nav rows keep
                // None (timestamp already stamps early in their lifecycle).
                started: Some(ev.started),
            });
        }
    }

    /// This navigation's request log, generation, and redirect trail.
    /// Script-issued rows are drained first, so the log matches the realm.
    pub fn navigation_record(&mut self) -> NavigationRecord {
        self.sync_js_network_events();
        NavigationRecord {
            events: self.network_events.clone(),
            epoch: self.navigation_epoch,
            redirect_chain: self.redirect_chain.clone(),
        }
    }

    /// Register a passive callback fired for every request this page's
    /// fetches (document, subresources) and its JS fetch()/XHR make, once the
    /// method/headers are known and before it is sent. Non-blocking; use
    /// `enable_interception` to mutate or block. Returns a stable id; pass it
    /// to `off_request` to detach (upstream #408). Scoped to this page: it
    /// never sees sibling pages' requests and dies with the page.
    pub fn on_request(&mut self, cb: diting_net::RequestCallback) -> u64 {
        self.callbacks.add_request(cb)
    }

    /// Register a passive callback fired with every response this page
    /// receives, including its body. Non-blocking. The main path for crawlers
    /// that need to capture API response payloads. Returns a stable id.
    /// Page-scoped like `on_request`.
    pub fn on_response(&mut self, cb: diting_net::ResponseCallback) -> u64 {
        self.callbacks.add_response(cb)
    }

    /// Detach a request observer registered with `on_request`. Returns true
    /// if one was removed.
    pub fn off_request(&mut self, id: u64) -> bool {
        self.callbacks.remove_request(id)
    }

    /// Fan `Network.setExtraHTTPHeaders` out to every transport this page
    /// can issue requests from. A stealth page fetches its main document
    /// through the wreq stealth client (`fetch_document`) while JS
    /// fetch()/XHR keep using the plain one — writing the extras to
    /// the plain client alone silently drops them from document requests,
    /// which is exactly the header a CDP client set them to control
    /// (obscura #571).
    pub async fn set_extra_headers(&self, headers: std::collections::HashMap<String, String>) {
        self.http_client.set_extra_headers(headers.clone()).await;
        if let Some(ref stealth) = self.stealth_client {
            stealth.set_extra_headers(headers).await;
        }
    }

    /// Apply a CDP `setUserAgentOverride` (Network + Emulation domains) to
    /// every surface that advertises an identity: both HTTP transports, and
    /// the live JS persona so navigator.userAgent / platform / fonts follow
    /// immediately instead of on the next navigation. One source of truth
    /// for the whole identity (obscura #481 class). The TLS fingerprint's
    /// OS is baked into the wreq client at construction and is deliberately
    /// not rebuilt here — matching Chrome, where a UA override does not
    /// re-handshake the TLS stack.
    ///
    /// `lang` carries the override's `acceptLanguage`, `platform` its
    /// `platform` field (navigator.platform only — obscura #777 class).
    /// Each sent field applies independently — a locale-only call must not
    /// touch the UA and vice versa (Chrome's only-sent-fields semantics).
    /// The language moves navigator.language immediately and the transport
    /// headers, but never re-pins the ICU default (process-global, sticky
    /// per-isolate — see [`diting_js::runtime`]).
    pub async fn set_user_agent_override(
        &mut self,
        ua: &str,
        lang: Option<&str>,
        platform: Option<&str>,
    ) {
        if !ua.is_empty() {
            self.http_client.set_user_agent(ua).await;
            if let Some(ref stealth) = self.stealth_client {
                stealth.set_user_agent(ua).await;
            }
            if let Some(ref mut rt) = self.js {
                rt.set_user_agent(ua);
            }
            // set_user_agent republished the persona viewport; put the
            // override back on top of it.
            self.apply_viewport_override();
            self.apply_emulated_media();
            self.apply_touch_override();
        }
        if let Some(lang) = lang.filter(|l| !l.is_empty()) {
            self.http_client.set_accept_language(lang).await;
            if let Some(ref stealth) = self.stealth_client {
                stealth.set_accept_language(lang).await;
            }
            if let Some(ref mut rt) = self.js {
                rt.set_navigator_language(lang);
            }
            // The explicit locale pin outranks the UA's acceptLanguage —
            // put it back on top, same posture as the viewport pin above.
            self.apply_locale_override();
        }
        if let Some(platform) = platform.filter(|p| !p.is_empty()) {
            if let Some(ref mut rt) = self.js {
                rt.set_navigator_platform(platform);
            }
        }
    }
}
