//! Navigation and settle: the navigate family (document fetch, redirect
//! chains, referrer/history bookkeeping) plus the event-loop settle/pump
//! family sessions drive between actions. Split from page/mod.rs
//! (ARCHITECTURE.md P2 batch 3); behavior unchanged. This module names the
//! page helpers a navigation calls.

use diting_dom::{parse_html, DomTree};
use diting_net::{NetError, Response};
use url::Url;

use super::load::{decode_data_uri, subresource_allowed};
use super::{Page, PageError};

/// Returns true when a JS-initiated navigation would step from a
/// non-file scheme into a file: URL. We treat that move as an SOP
/// violation because the existing realm survives the navigation and
/// can read the new document's body.
pub(super) fn cross_scheme_to_file(from: &str, to: &str) -> bool {
    let to_is_file = Url::parse(to)
        .map(|u| u.scheme().eq_ignore_ascii_case("file"))
        .unwrap_or(false);
    if !to_is_file {
        return false;
    }
    Url::parse(from)
        .map(|u| !u.scheme().eq_ignore_ascii_case("file"))
        .unwrap_or(true)
}

/// Document base for a commit-time stylesheet. The first `<base href>` in
/// the parse tree, resolved against the document URL; otherwise the
/// document URL. These sheets are fetched before the JS realm owns the
/// DOM, so this reads the parse tree directly — the same rule as
/// [`Page::resolve_base_url`].
fn stylesheet_base(doc_url: &Url, dom: &DomTree) -> Url {
    // Tag walk, not `base[href]`: the attribute form does not match on this
    // parse tree, so a present <base href="/"> was ignored and the sheet
    // was joined against the document path.
    let href = dom
        .query_selector_all("base")
        .ok()
        .into_iter()
        .flatten()
        .find_map(|nid| {
            dom.get_node(nid).and_then(|n| {
                n.get_attribute("href")
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
            })
        });
    match href {
        Some(href) => doc_url.join(&href).unwrap_or_else(|_| doc_url.clone()),
        None => doc_url.clone(),
    }
}

/// Chrome renders a response whose MIME says "this is not HTML" as a plain
/// text document: body is a single `<pre>` holding the source verbatim, so
/// newlines survive and markup characters stay inert. Feeding the bytes to
/// the HTML parser instead collapses the formatting and interprets any
/// `<tag>`-looking text as real elements.
pub(super) fn renders_as_text_document(mime: &str) -> bool {
    if mime == "text/html" || mime == "application/xhtml+xml" || mime.starts_with("image/") {
        return false;
    }
    mime.starts_with("text/")
        || mime == "application/json"
        || mime == "application/javascript"
        || mime.ends_with("+json")
        || mime == "application/xml"
        || mime.ends_with("+xml")
}

pub(super) fn plain_text_document(body: &str) -> DomTree {
    let mut escaped = String::with_capacity(body.len());
    for ch in body.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    parse_html(&format!("<pre>{escaped}</pre>"))
}

/// Escape a value for safe inclusion inside a JavaScript template
/// literal. The previous implementation only escaped `\`, `` ` `` and
/// `${`; that left U+2028 / U+2029 (the JS-specific line terminators)
/// and other control characters as breakout vectors. Done at the
/// callsite means future tweaks come back to one function.
pub(super) fn escape_for_js_template_literal(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '$' => out.push_str("\\$"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            '\u{0000}' => out.push_str("\\0"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}
use crate::lifecycle::LifecycleState;

impl Page {
    /// Fetch the main document. Stealth mode bypasses the tracing client —
    /// the wreq-backed stealth transport has no callback hook (and stealth
    /// pages are exactly the ones whose observers should not double-fire on
    /// a side channel); observers still see scripts/stylesheets/fetches.
    pub(super) async fn fetch_document(&self, url: &Url) -> Result<Response, NetError> {
        // CDP Fetch interception covers navigation documents too (Playwright
        // `page.route` / Puppeteer `setRequestInterception`). The document
        // request parks on the same resolution channel as a script-initiated
        // fetch; the load task runs spawned off the CDP loop, so the bridge
        // can answer it from a later command or the idle pump. No answer
        // inside the shared resolution timeout → fall through to the real
        // request (Continue semantics). Intercept resolution `Continue`
        // overrides (url/method/headers rewrites) are not applied to
        // documents — clients rewrite the bare `route.continue_()` case in
        // practice, and honored rewrites would need re-running the SSRF gate
        // on the substituted URL before commit.
        if let Some(tx) = &self.intercept_tx {
            let (resolver, rx) = tokio::sync::oneshot::channel();
            let mut headers = std::collections::HashMap::new();
            if let Ok(ua) = self.http_client.user_agent.try_read() {
                headers.insert("User-Agent".to_string(), ua.clone());
            }
            let request = diting_js::ops::InterceptedRequest {
                url: url.to_string(),
                method: "GET".to_string(),
                headers,
                post_data: None,
                resource_type: "Document".to_string(),
                request_id: String::new(),
                resolver,
            };
            if tx.send(request).is_ok() {
                let timeout_ms = diting_js::ops::INTERCEPT_RESOLUTION_TIMEOUT_MS
                    .load(std::sync::atomic::Ordering::Relaxed);
                match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx).await {
                    Ok(Ok(diting_js::ops::InterceptResolution::Fulfill {
                        status,
                        headers,
                        body,
                    })) => {
                        return Ok(Response {
                            url: url.clone(),
                            status,
                            headers,
                            body,
                            redirected_from: Vec::new(),
                            redirect_hops: Vec::new(),
                            request_headers: std::collections::HashMap::new(),
                        });
                    }
                    Ok(Ok(diting_js::ops::InterceptResolution::Fail { reason })) => {
                        return Err(NetError::Network(format!(
                            "request blocked by client, reason: {reason}"
                        )));
                    }
                    // Continue / dropped resolver / timeout: real request below.
                    Ok(Ok(_)) | Ok(Err(_)) | Err(_) => {}
                }
            }
        }
        // A JS/link-initiated navigation carries the referring document's
        // URL per browser semantics; direct automation navigations leave
        // the staged referrer empty (edb1785) and go out bare. `facts`
        // prefers the in-flight draft: the live document still belongs to
        // the outgoing realm.
        let staged_referrer = self.facts().referrer();
        let referrer = if staged_referrer.is_empty() {
            None
        } else {
            Some(staged_referrer.as_str())
        };
        if let Some(ref stealth) = self.stealth_client {
            return stealth.fetch_navigation(url, referrer).await;
        }
        self.http_client
            .fetch_with_callbacks(
                url,
                Some(&self.callbacks),
                diting_net::ResourceType::Document,
                referrer,
            )
            .await
    }

    pub fn take_pending_navigation(&self) -> Option<(String, String, String)> {
        if let Some(js) = &self.js {
            js.take_pending_navigation()
        } else {
            None
        }
    }

    /// Non-consuming peek: does the page have a JS-initiated navigation
    /// (location/form) queued and undrained? Settle loops ask this before
    /// paying for the drain attempt.
    pub fn has_pending_navigation(&self) -> bool {
        self.js
            .as_ref()
            .is_some_and(|js| js.has_pending_navigation())
    }

    /// Session-history jump queued by page JS (`history.back()` past this
    /// document's first entry, `forward()` into seeded forward entries) —
    /// an absolute index into `self.history`.
    /// Whether the page asked to step through session history across
    /// documents (`history.back()` past this document's own entries).
    pub fn has_pending_history_jump(&self) -> bool {
        self.js
            .as_ref()
            .is_some_and(|js| js.has_pending_history_jump())
    }

    pub fn take_pending_history_jump(&self) -> Option<i32> {
        if let Some(js) = &self.js {
            js.take_pending_history_jump()
        } else {
            None
        }
    }

    pub fn take_pending_write_nav(&self) -> bool {
        match &self.js {
            Some(js) => js.take_pending_write_nav(),
            None => false,
        }
    }

    pub async fn process_pending_navigation(&mut self) -> Result<bool, PageError> {
        // Session-history jump (history.back/forward past this document's
        // boundaries — obscura#1105): navigate to the EXISTING entry, never
        // append. A stale or out-of-range target (hostile page, raced CDP
        // edit) falls through to the regular pending-navigation arm.
        let history_jump = self
            .take_pending_history_jump()
            .filter(|&t| t >= 0)
            .map(|t| t as usize)
            .filter(|&t| t != self.history_index && t < self.history.len());
        if let Some(t) = history_jump {
            let url = self.history[t].clone();
            let prev_index = self.history_index;
            // Move the cursor FIRST: init_js seeds the landing document's JS
            // history from the cursor position, and push_history's
            // consecutive-dedupe then sees the same URL and early-returns —
            // the jump MOVES the session history instead of appending.
            self.history_index = t;
            let result = self
                .navigate_with_wait_post_ref(&url, crate::lifecycle::WaitUntil::Load, "GET", "", "")
                .await;
            if result.is_err() {
                // The outgoing document is still live (obscura#920 failed-nav
                // semantics): restore the cursor it was seeded with.
                self.history_index = prev_index;
            }
            return result.map(|_| true);
        }
        if let Some((url, method, body)) = self.take_pending_navigation() {
            // A navigation the page asked for itself is document-initiated:
            // the first hop carries a referrer per
            // strict-origin-when-cross-origin, unlike direct automation
            // navigations which send none (upstream parity).
            let source_url = self
                .url
                .as_ref()
                .and_then(|source| {
                    Url::parse(&url).ok().map(|target| {
                        diting_net::client::HttpClient::navigation_referrer(source, &target)
                    })
                })
                .unwrap_or_default();
            self.navigate_with_wait_post_ref(
                &url,
                crate::lifecycle::WaitUntil::Load,
                &method,
                &body,
                &source_url,
            )
            .await?;
            Ok(true)
        } else {
            // A page that routed itself through history has still navigated
            // (SPA pushState adoption — see fork_virtual_url.rs).
            Ok(self.sync_virtual_url())
        }
    }

    /// Pump the event loop until pending work settles: dynamic external
    /// script fetches land and execute, inflight XHR/fetch resolve, and
    /// timers tick. A page with nothing pending leaves on the 500ms fast
    /// path (upstream a6bb741). `DITING_DYNAMIC_SCRIPT_SETTLE_MS`
    /// (default 3s) still caps a dynamic script or stylesheet fetch. A
    /// live ES `import()` keeps the loop turning until that graph finishes;
    /// synchronous work inside the pump is bounded separately per task.
    pub(super) async fn settle_pending_work(&mut self) {
        let Some(js) = self.js.as_mut() else { return };
        let dynamic_settle_ms = std::env::var("DITING_DYNAMIC_SCRIPT_SETTLE_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3_000)
            .max(500);
        // Synchronous work inside each pump is bounded by the pump family's
        // per-task watchdog (#224/#100): legitimate commerce-SPA commits may
        // chain for minutes at task boundaries, while one never-ending task
        // is still cut. `dynamic_settle_ms` remains an ASYNC module/script
        // patience below; it is no longer also miscast as a sync wall.
        let started = tokio::time::Instant::now();
        let deadline = started + tokio::time::Duration::from_millis(500);
        let dynamic_deadline = started + tokio::time::Duration::from_millis(dynamic_settle_ms);
        let mut idle_count = 0u32;
        loop {
            let now = tokio::time::Instant::now();
            // ES import() only. A dead <link rel=stylesheet> re-fetch also
            // bumps the dynamic-script counter; treating that as "keep
            // pumping" held navigation until the 30s deadline (#203).
            let import_pending = js.has_pending_dynamic_imports();
            // 500ms fast path when nothing is in flight. A dynamic script
            // fetch still stops at `dynamic_settle_ms`. A live import() does
            // not: Angular's component route downloads the sidenav graph,
            // then starts the examples import only after that graph
            // evaluates, and the 3s cap returned the toolbar first.
            if now >= deadline
                && !import_pending
                && (now >= dynamic_deadline || !js.has_pending_module_load())
            {
                break;
            }
            let idle = js.run_event_loop_until_idle(10).await;
            if idle {
                if self.http_client.active_requests() == 0 && !js.has_pending_module_load() {
                    idle_count += 1;
                    if idle_count >= 2 {
                        break;
                    }
                    tokio::task::yield_now().await;
                } else {
                    idle_count = 0;
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                }
            } else {
                idle_count = 0;
                // The loop is parked on a far-off timer, not on work:
                // when nothing is in flight and nothing fires inside
                // the rest of the fast-path window, waiting it out
                // only adds latency.
                if self.http_client.active_requests() == 0 && !js.has_pending_module_load() {
                    let left = deadline
                        .saturating_duration_since(tokio::time::Instant::now())
                        .as_millis() as u64;
                    if js.settle_quiet(left) {
                        break;
                    }
                }
            }
        }
        js.trim_heap();
    }

    pub async fn navigate(&mut self, url_str: &str) -> Result<(), PageError> {
        self.navigate_with_wait(url_str, crate::lifecycle::WaitUntil::Load)
            .await
    }

    pub async fn navigate_with_wait(
        &mut self,
        url_str: &str,
        wait_until: crate::lifecycle::WaitUntil,
    ) -> Result<(), PageError> {
        self.navigate_with_wait_post(url_str, wait_until, "GET", "")
            .await
    }

    pub async fn navigate_with_wait_post(
        &mut self,
        url_str: &str,
        wait_until: crate::lifecycle::WaitUntil,
        method: &str,
        body: &str,
    ) -> Result<(), PageError> {
        // Direct automation navigations carry no referrer (upstream edb1785:
        // only document-initiated navigations set one; the JS-triggered chain
        // inside the inner loop stamps each subsequent hop).
        self.navigate_with_wait_post_ref(url_str, wait_until, method, body, "")
            .await
    }

    /// Page-scoped navigation deadline: `set_navigation_timeout` when the
    /// automation request carries an explicit timeout, else
    /// `DITING_NAV_TIMEOUT_MS` (default 30s). Complements the env var the
    /// way upstream does (structured field over process-wide default).
    pub(super) fn navigation_timeout(&self) -> tokio::time::Duration {
        let ms = self.navigation_timeout_ms.unwrap_or_else(|| {
            std::env::var("DITING_NAV_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30_000)
        });
        tokio::time::Duration::from_millis(ms)
    }

    /// Set a page-scoped navigation deadline in milliseconds; `None` restores
    /// the process-wide default.
    pub fn set_navigation_timeout(&mut self, ms: Option<u64>) {
        self.navigation_timeout_ms = ms;
    }

    pub(super) async fn navigate_with_wait_post_ref(
        &mut self,
        url_str: &str,
        wait_until: crate::lifecycle::WaitUntil,
        method: &str,
        body: &str,
        initial_referrer: &str,
    ) -> Result<(), PageError> {
        // The initiating document's contribution to `document.referrer` on
        // the first hop. Empty for direct automation navigations; the page's
        // own pending navigation (process_pending_navigation) passes a
        // strict-origin-when-cross-origin value here. Subsequent hops of a
        // JS-triggered chain are stamped inside the inner loop.
        self.draft().set_referrer(initial_referrer);
        // Hard ceiling on a single end-to-end navigation. Without this a slow
        // primary fetch or a runaway settle loop can hold the V8 lock for
        // arbitrarily long (we've measured 60+ seconds on JS-heavy news
        // sites), wedging every other in-flight CDP request because the
        // dispatcher holds the lock across the entire handler. 30 seconds
        // matches the clients' 30s read timeout — the worst case is
        // one slow primary GET plus one slow JS-redirect chain step. Override
        // with `DITING_NAV_TIMEOUT_MS=NN`, or set a page-scoped deadline when
        // the automation request already has an explicit timeout.
        let nav_timeout = self.navigation_timeout();
        let nav_timeout_ms = nav_timeout.as_millis() as u64;
        // Fresh progress cell contents for this attempt: the observer (the
        // /fetch starvation checkpoint) may have cloned the Arc before
        // calling in, and must not see the previous attempt's commit or
        // script counts.
        self.nav_progress.reset();

        let result = match tokio::time::timeout(
            nav_timeout,
            self.navigate_with_wait_post_inner(url_str, wait_until, method, body),
        )
        .await
        {
            Ok(r) => r,
            Err(_) => {
                // #101: one deadline, four different failures. Name the
                // phase and the document that is actually live — computed
                // BEFORE `Failed` overwrites the lifecycle evidence.
                let epoch = self.navigation_epoch;
                let committed = self.realm_epoch == epoch;
                let doc_received = self
                    .network_events
                    .iter()
                    .any(|e| e.nav == epoch && e.resource_type == "Document");
                let active = if committed {
                    self.url_string()
                } else {
                    // The attempt never committed: the outgoing document is
                    // still the live page. Roll the optimistic `url` back
                    // with it, so /network's url, /state's location.href
                    // and eval's realm all describe the same document
                    // (#101's three-faces complaint).
                    self.url = Url::parse(&self.carried_network_url).ok();
                    if self.carried_network_url.is_empty() {
                        "about:blank".to_string()
                    } else {
                        self.carried_network_url.clone()
                    }
                };
                let phase = if committed {
                    match self.lifecycle {
                        crate::lifecycle::LifecycleState::Loading => {
                            "document committed, scripts/settle still running"
                        }
                        _ => "loaded after the deadline raced",
                    }
                } else if doc_received {
                    "document received, not yet committed"
                } else {
                    "document fetch in flight"
                };
                self.lifecycle = crate::lifecycle::LifecycleState::Failed;
                Err(PageError::NetworkError(format!(
                    "navigation exceeded {nav_timeout_ms}ms deadline ({phase}; \
                     active document: {active}; requested: {url_str}; navigation #{epoch} — \
                     /network rows carry the generation as \"nav\")"
                )))
            }
        };
        if result.is_err() {
            // The attempt did not commit this draft (or a later chain hop
            // staged a referrer it never navigated). Drop it so the live
            // document stays the one the realm shows.
            self.pending_document = None;
        }
        if result.is_ok() {
            self.push_history(self.url_string());
        }
        result
    }

    /// Drive the JS event loop after navigation so deferred work can run:
    /// pending timers (setTimeout / setInterval), queued microtasks, in-flight
    /// fetches, and completion callbacks such as testharness's
    /// `add_completion_callback`. Returns as soon as the loop goes idle, or
    /// after `max_ms`. Without this the page is observed exactly as it stood at
    /// the load event, before any async work settles, which silently strands
    /// timer-driven tests and dynamic pages.
    pub async fn settle(&mut self, max_ms: u64) {
        if max_ms == 0 {
            return;
        }
        if let Some(js) = &mut self.js {
            // Bounded against both async idle and synchronous microtask storms:
            // a plain tokio timeout cannot preempt a page that pins the thread
            // inside V8 (the real-world SPA hang), so settle drives the loop
            // through the watchdog-guarded path.
            let _ = js.run_event_loop_bounded(max_ms).await;
        }
    }

    /// Drive the JS event loop until it goes idle, capped at `max_ms`. Use
    /// after interactions that kick off async work whose completion matters
    /// (client-side route transitions: the RSC fetch → flight parse → render →
    /// pushState chain). Returns `true` if the page quiesced within the
    /// budget; `false` means still busy (or capped by an interval timer).
    pub async fn settle_until_idle(&mut self, max_ms: u64) -> bool {
        if max_ms == 0 {
            return true;
        }
        if let Some(js) = &mut self.js {
            let fired_before = js.watchdog_fired_total();
            let idle = js.run_event_loop_until_idle(max_ms).await;
            if js.watchdog_fired_total() > fired_before {
                self.storm_backoff_ms = (self.storm_backoff_ms.max(200) * 2).min(5000);
                self.storm_hot_until = Some(
                    tokio::time::Instant::now()
                        + tokio::time::Duration::from_millis(self.storm_backoff_ms),
                );
            } else if idle {
                self.storm_backoff_ms = 0;
                self.storm_hot_until = None;
            }
            return idle;
        }
        true
    }

    /// Wait until the page stops producing content. `settle(max_ms)` pumps
    /// by duration, and on a slow tunnel the event loop goes idle between
    /// chunk fetches (each 3-8s in flight with nothing to run), so a
    /// short-wait extract caught pages mid-hydration — eui's demo tables
    /// read "Loading..." at wait_secs≤5 but all render by 8s. Stability is
    /// two signals agreeing across a whole quiet window: nothing in flight
    /// (HTTP requests, module loads) and the body text length unchanged.
    /// `min_ms` is the caller's floor (`wait_secs`) — never cut, whatever
    /// the clocks say. The open-ended tail is capped at `TAIL_MS`, further
    /// clamped by `tail_budget_ms` so a page whose navigation already ate
    /// the caller's overall deadline (benchmark worker kills at 27s) spends
    /// only what's left, not a fresh 9s on top.
    pub async fn settle_content_stable(&mut self, min_ms: u64, tail_budget_ms: u64) {
        const TAIL_MS: u64 = 9_000;
        const WINDOW_MS: u64 = 600;
        self.settle(min_ms).await;
        let tail = TAIL_MS.min(tail_budget_ms);
        let cap = tokio::time::Instant::now() + tokio::time::Duration::from_millis(tail);
        let window = tokio::time::Duration::from_millis(WINDOW_MS);
        let mut last = self.body_text_len().await;
        let mut quiet_since: Option<tokio::time::Instant> = None;
        loop {
            if tokio::time::Instant::now() >= cap {
                return;
            }
            self.settle(WINDOW_MS).await;
            // A JS navigation queued during the settle (delayed redirect,
            // challenge pass-through) must not rot in the queue: nothing
            // downstream of this loop drains it in the stateless fetch
            // path. Running it here lets the landing document join the
            // same stability measurement; its own text length resets the
            // quiet window. The navigation carries the page-scoped nav
            // timeout for hang protection, like the byte-WAF drain.
            if self.has_pending_navigation() {
                match self.process_pending_navigation().await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("settle tail: JS navigation failed: {}", e);
                    }
                }
                last = self.body_text_len().await;
                quiet_since = None;
                continue;
            }
            // Script graph pending = content is on its way (webpack chunk
            // fetches). Arbitrary network activity is NOT a hold signal —
            // analytics beacons keep python.org's request count above zero
            // for the whole tail while its text stopped changing at 1s.
            let busy = self
                .js
                .as_ref()
                .is_some_and(|js| js.has_pending_module_load());
            let len = self.body_text_len().await;
            let now = tokio::time::Instant::now();
            if len != last || busy {
                last = len;
                quiet_since = None;
                continue;
            }
            match quiet_since {
                None => quiet_since = Some(now),
                Some(t) if now.duration_since(t) >= window => return,
                Some(_) => {}
            }
        }
    }

    /// `document.body.innerText.length` as a cheap content fingerprint for
    /// [`settle_content_stable`]. Errors (no body / wedged realm) read as 0,
    /// which is as comparable as any other constant.
    async fn body_text_len(&mut self) -> usize {
        self.evaluate_remote(
            "(document.body && document.body.innerText || '').length",
            true,
            false,
        )
        .await
        .value
        .and_then(|v| v.as_u64().map(|n| n as usize))
        .unwrap_or(0)
    }

    async fn navigate_with_wait_post_inner(
        &mut self,
        url_str: &str,
        wait_until: crate::lifecycle::WaitUntil,
        method: &str,
        body: &str,
    ) -> Result<(), PageError> {
        let mut current_url = url_str.to_string();
        let mut current_method = method.to_string();
        let mut current_body = body.to_string();
        // This cap counts documents in a JS-initiated navigation chain
        // (location/form hops), not HTTP 3xx redirects — those are budgeted
        // separately (20) by the net client. The low default is right: it is
        // what stops a page that resets `location` on every load. But a
        // legitimate long chain (SSO handover across providers) must be
        // raisable by the operator — env knob in the shape of
        // DITING_NAV_TIMEOUT_MS (obscura#664).
        let chain_limit = std::env::var("DITING_NAV_CHAIN_LIMIT")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(10);
        for chain in 0..chain_limit {
            self.navigate_single(&current_url, wait_until, &current_method, &current_body)
                .await?;
            if let Some((next_url, next_method, next_body)) = self.take_pending_navigation() {
                if cross_scheme_to_file(&current_url, &next_url) {
                    // SOP gate. A web page must not be able to drive
                    // a navigation to file:// and then read the loaded
                    // document. Without this an http(s) page sets
                    // window.onload, calls location.href = "file:..."
                    // and harvests document.body from a local file
                    // once the new document loads.
                    tracing::warn!(
                        "blocking JS-initiated cross-scheme navigation to file: {} -> {}",
                        current_url,
                        next_url,
                    );
                    break;
                }
                tracing::info!(
                    "JS-triggered navigation chain: {} {} -> {}",
                    current_method,
                    current_url,
                    next_url
                );
                // The chain step is document-initiated: the new document sees
                // the strict-origin-when-cross-origin referrer of this one.
                let referrer = Url::parse(&current_url)
                    .ok()
                    .and_then(|src| {
                        Url::parse(&next_url).ok().map(|dst| {
                            diting_net::client::HttpClient::navigation_referrer(&src, &dst)
                        })
                    })
                    .unwrap_or_default();
                self.draft().set_referrer(&referrer);
                current_url = next_url;
                current_method = next_method;
                current_body = next_body;
                if chain + 1 == chain_limit {
                    // Hit the cap and the page still wants to keep
                    // chaining. Surface that as an error instead of
                    // returning Ok(()) so callers can distinguish a
                    // successful load from a redirect storm.
                    return Err(PageError::TooManyClientNavigations(chain_limit));
                }
                continue;
            }
            break;
        }
        Ok(())
    }

    async fn navigate_single(
        &mut self,
        url_str: &str,
        wait_until: crate::lifecycle::WaitUntil,
        method: &str,
        body: &str,
    ) -> Result<(), PageError> {
        let url = Url::parse(url_str).map_err(|e| PageError::InvalidUrl(e.to_string()))?;
        // Navigation phase clocks (DITING_NAV_TIMING): permanent forensics
        // for the navigation-lifecycle campaign — which phase the fixed
        // per-navigation cost actually lives in.
        let t_nav = std::time::Instant::now();

        // The outgoing document's events must outlive this navigation: the
        // CDP drain that emits Network events only runs after the new
        // document settles, `network_events` is reset below, and `init_js`
        // swaps the runtime whose queue still holds script-initiated
        // fetch/XHR events. Sync those into the page-side list now and
        // carry the whole thing across (obscura #920 shape — and broader
        // than the history-jump case there: every navigation dropped them).
        self.sync_js_network_events();
        self.carried_network_url = self.url_string();
        let mut outgoing = std::mem::take(&mut self.network_events);
        self.carried_network_events.append(&mut outgoing);

        // A new navigation attempt begins: bump the generation so every
        // row recorded below attributes to it (#101). After the sync above,
        // which stamps the outgoing document's drained rows with the old
        // epoch.
        self.navigation_epoch += 1;

        self.lifecycle = LifecycleState::Loading;
        self.url = Some(url.clone());
        // A new attempt invalidates the previous document's redirect trail
        // (#102): the field describes the CURRENT document's fetch, and a
        // failed attempt must not leave its trail behind.
        self.redirect_chain.clear();

        if url.scheme() == "about" {
            self.navigate_blank();
            // Lazy blank realm: about:blank has no scripts of its own, so
            // its realm is pure cost until something touches JS — and the
            // dominant shape (puppeteer's `newPage()` → immediate `goto()`)
            // would build the blank realm and throw it away within
            // milliseconds. Mark the deferral instead; the first eval-side
            // operation materializes realm + preloads through
            // `materialize_deferred_realm` (the exact wiring an eager
            // blank used to get, preloads included — exposeFunction on a
            // fresh page must still exist before the caller's first
            // evaluate). A real navigation's init_js clears the flag, so
            // a deferred blank that is navigated away never builds
            // anything at all.
            self.deferred_blank_realm = true;
            return Ok(());
        }

        let t_fetch = std::time::Instant::now();
        let response = if url.scheme() == "data" {
            let content_type = url_str
                .strip_prefix("data:")
                .and_then(|s| s.split(',').next())
                .unwrap_or("text/html")
                .split(';')
                .next()
                .unwrap_or("text/html")
                .to_string();
            let body_bytes = decode_data_uri(url_str).unwrap_or_default();
            let mut headers = std::collections::HashMap::new();
            headers.insert("content-type".to_string(), content_type);
            Ok(diting_net::Response {
                url: url.clone(),
                status: 200,
                headers,
                body: body_bytes,
                redirected_from: Vec::new(),
                redirect_hops: Vec::new(),
                request_headers: std::collections::HashMap::new(),
            })
        } else if method == "POST" {
            // The submitting document initiates the POST: it is both the
            // Referer (policy-trimmed per hop) and the Origin source.
            let doc_referrer = self.url.as_ref().map(|u| u.to_string());
            self.http_client
                .post_form_with_callbacks(
                    &url,
                    body,
                    Some(&self.callbacks),
                    diting_net::ResourceType::Document,
                    doc_referrer.as_deref(),
                )
                .await
        } else {
            self.fetch_document(&url).await
        }
        .map_err(|e| {
            self.lifecycle = LifecycleState::Failed;
            // The document never came: the log says so, as a browser's does.
            self.record_failed_network_event(
                url.as_str(),
                method,
                "Document",
                e.to_string(),
                e.failure(),
            );
            PageError::from(e)
        })?;
        let fetch_ms = t_fetch.elapsed().as_micros();
        let t_parse = std::time::Instant::now();

        self.record_network_event_with_body(
            url.as_str(),
            method,
            "Document",
            response.status,
            &response.headers,
            &response.request_headers,
            &response.body,
        );
        if !response.redirect_hops.is_empty() {
            self.fill_redirect_trail(response.url.to_string(), response.redirect_hops.clone());
        }

        // #102: keep the hop trail for the session face — a login bounce,
        // a rewritten parameter error and a plain landing are
        // distinguishable only by how the request wandered before the
        // document arrived.
        self.redirect_chain = response
            .redirected_from
            .iter()
            .map(|u| u.to_string())
            .collect();
        if !response.redirected_from.is_empty() {
            self.url = Some(response.url.clone());
        }

        // Honor the response charset: HTTP Content-Type → <meta charset> sniff
        // in the first 1KB → UTF-8 fallback. Without this, every non-UTF-8
        // page (GBK, Big5, Shift-JIS, Windows-125x, EUC-KR, ISO-8859-x)
        // came through as replacement characters.
        let (body_text, encoding_name) =
            diting_net::decode_response_with_name(&response.body, response.content_type());
        let draft = self.draft();
        draft.set_encoding(encoding_name);
        // Main-response MIME (lowercased, parameters stripped) backs
        // `document.contentType`. An absent Content-Type stays empty so the
        // JS layer keeps its URL-sniffing fallback. The local is what the
        // svg / plain-text branch below reads — the draft is not committed
        // yet, and the live document must not move.
        let content_type = response
            .content_type()
            .map(|ct| ct.split(';').next().unwrap_or("").trim().to_lowercase())
            .filter(|ct| !ct.is_empty());
        draft.set_content_type(content_type.as_deref().unwrap_or(""));
        // Referrer Policy §"Determine request's Referrer Policy": a policy
        // delivered via the response header wins outright over <meta>. Keep
        // only the last valid comma token (invalid ones are skipped).
        draft.set_referrer_policy(
            &response
                .header("referrer-policy")
                .and_then(diting_js::ops::last_valid_referrer_token)
                .unwrap_or_default(),
        );
        // A bare SVG document navigated as a document (content-type
        // image/svg+xml; the html5 parser wraps it in html>body>svg):
        // Chrome sizes the root svg at 100%x100% of the initial containing
        // block with no body UA margin. Reproduce that by injecting the
        // effective viewport as width/height attributes on the root svg
        // (author attributes win — only absent ones are filled in) plus a
        // head style resetting the body margin. The replaced-leaf intrinsic
        // arm and the viewBox meet scaling (svg.rs) do the rest. Layout
        // only runs in the screenshot pipeline, so the whole branch gates
        // with it.
        #[cfg(feature = "screenshot")]
        let dom = {
            let is_svg_doc = response
                .content_type()
                .is_some_and(|ct| ct.starts_with("image/svg"));
            if is_svg_doc {
                let dom = parse_html(&format!(
                    "<style>html,body{{margin:0;padding:0}}</style>{body_text}"
                ));
                if let Some(svg_id) = dom.query_selector("svg").ok().flatten() {
                    let (vw, vh) = self.effective_viewport();
                    dom.with_node_mut(svg_id, |n| {
                        if n.get_attribute("width").is_none() {
                            n.set_attribute("width", vw.to_string());
                        }
                        if n.get_attribute("height").is_none() {
                            n.set_attribute("height", vh.to_string());
                        }
                    });
                }
                dom
            } else if content_type
                .as_deref()
                .is_some_and(renders_as_text_document)
            {
                plain_text_document(&body_text)
            } else {
                parse_html(&body_text)
            }
        };
        #[cfg(not(feature = "screenshot"))]
        let dom = if content_type
            .as_deref()
            .is_some_and(renders_as_text_document)
        {
            plain_text_document(&body_text)
        } else {
            parse_html(&body_text)
        };

        let parse_ms = t_parse.elapsed().as_micros();
        let t_css = std::time::Instant::now();

        let title = dom
            .query_selector("title")
            .ok()
            .flatten()
            .map(|title_id| dom.text_content(title_id))
            .unwrap_or_default();
        draft.set_title(&title);

        let stylesheet_urls: Vec<String> = dom
            .query_selector_all("link")
            .unwrap_or_default()
            .iter()
            .filter_map(|&nid| {
                let node = dom.get_node(nid)?;
                let rel = node.get_attribute("rel")?;
                if rel.to_lowercase() != "stylesheet" {
                    return None;
                }
                node.get_attribute("href").map(|s| s.to_string())
            })
            .collect();

        let sheet_base = self.url.as_ref().map(|doc| stylesheet_base(doc, &dom));
        let mut css_fetch_urls: Vec<String> = Vec::new();
        for href in &stylesheet_urls {
            let full_url = if href.starts_with("http://") || href.starts_with("https://") {
                href.clone()
            } else if let Some(base) = &sheet_base {
                base.join(href)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| href.clone())
            } else {
                href.clone()
            };
            if !subresource_allowed(self.url.as_ref(), &full_url) {
                tracing::warn!(
                    "blocking cross-scheme <link rel=stylesheet href>: page={} href={}",
                    self.url_string(),
                    full_url,
                );
                continue;
            }
            if self.url_blocked(&full_url) {
                tracing::info!("Blocked stylesheet by Network.setBlockedURLs: {}", full_url);
                continue;
            }
            css_fetch_urls.push(full_url);
        }

        let client = self.http_client.clone();
        let css_callbacks = self.callbacks.clone();
        let doc_referrer = self.url.as_ref().map(|u| u.to_string());
        // A stylesheet whose connection silently dies must not hold the
        // commit hostage: the collect() below waits for every sheet, and a
        // black-holed fetch parks the whole navigation in "document
        // received, not yet committed" until the 30s nav deadline — or, on
        // a page-initiated hop, longer (#203 retest shape). Real browsers
        // commit the DOM regardless and let a dead sheet fail quietly.
        let sheet_budget_ms = std::env::var("DITING_STYLESHEET_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&ms| ms > 0)
            .unwrap_or(10_000);
        let css_futures: Vec<_> = css_fetch_urls
            .iter()
            .map(|full_url| {
                let client = client.clone();
                let css_callbacks = css_callbacks.clone();
                let url_str = full_url.clone();
                let doc_referrer = doc_referrer.clone();
                async move {
                    let parsed =
                        Url::parse(&url_str).unwrap_or_else(|_| Url::parse("about:blank").unwrap());
                    // #126: real resource-timing capture — epoch stamp for the
                    // resource clock, monotonic Instant for the duration. The
                    // record only lands on a completed fetch; failures stay out.
                    let rt_start_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs_f64() * 1000.0)
                        .unwrap_or(0.0);
                    let rt_t0 = std::time::Instant::now();
                    let fetched = tokio::time::timeout(
                        tokio::time::Duration::from_millis(sheet_budget_ms),
                        client.fetch_with_callbacks(
                            &parsed,
                            Some(css_callbacks.as_ref()),
                            diting_net::ResourceType::Stylesheet,
                            doc_referrer.as_deref(),
                        ),
                    )
                    .await;
                    match fetched {
                        Ok(Ok(resp)) => {
                            let dur_ms = rt_t0.elapsed().as_secs_f64() * 1000.0;
                            Some((url_str, resp, rt_start_ms, dur_ms))
                        }
                        Ok(Err(e)) => {
                            tracing::debug!("Failed to fetch stylesheet {}: {}", url_str, e);
                            None
                        }
                        Err(_) => {
                            tracing::warn!(
                                "stylesheet {} exceeded {}ms budget — committing without it",
                                url_str,
                                sheet_budget_ms
                            );
                            None
                        }
                    }
                }
            })
            .collect();

        // Same concurrency cap as script fetches.
        use futures::StreamExt as _;
        let css_results: Vec<_> = futures::stream::iter(css_futures)
            .buffer_unordered(16)
            .collect()
            .await;
        let mut css_sources: Vec<(String, String)> = Vec::new();
        let mut css_timings: Vec<diting_js::ops::ResourceTimingRecord> = Vec::new();
        for (url_str, resp, rt_start_ms, dur_ms) in css_results.into_iter().flatten() {
            // CSS bodies: honor the Content-Type charset; CSS @charset is
            // out of scope for the current scrape-focused pipeline.
            let css = diting_net::decode_non_html(&resp.body, resp.content_type());
            self.record_network_event_with_body(
                &url_str,
                "GET",
                "Stylesheet",
                resp.status,
                &resp.headers,
                &resp.request_headers,
                &resp.body,
            );
            css_timings.push(diting_js::ops::ResourceTimingRecord {
                name: url_str.clone(),
                initiator: "link",
                start_epoch_ms: rt_start_ms,
                duration_ms: dur_ms,
                status: resp.status,
                body_size: resp.body.len(),
                // Stylesheets always block first render in this pipeline.
                render_blocking: true,
            });
            css_sources.push((url_str, css));
        }

        let css_ms = t_css.elapsed().as_micros();
        let t_realm = std::time::Instant::now();
        self.dom = Some(dom);
        self.init_js();
        let realm_ms = t_realm.elapsed().as_micros();
        let t_scripts = std::time::Instant::now();

        // Hand the fetched sheet bodies to the JS state natively: the
        // layout run joins them into the cascade (getComputedStyle /
        // getBoundingClientRect / elementFromPoint see authored styles, not
        // initial values) and document.styleSheets builds real rule lists
        // from them. Has to happen before scripts run, regardless of
        // waitUntil, so handlers that read geometry or cssRules mid-boot
        // see the styled document.
        if let Some(js) = &mut self.js {
            let sheets: std::collections::HashMap<String, String> =
                css_sources.iter().cloned().collect();
            js.set_ext_sheets(sheets);
            // #126: hand the real fetch timings over with the sheet bodies.
            js.push_resource_timings(css_timings);
        }
        // Inject CSS as a global so any CSS-aware page shim can read the
        // joined text directly (window.__diting_css).
        if !css_sources.is_empty() {
            if let Some(js) = &mut self.js {
                let combined_css = css_sources
                    .iter()
                    .map(|(_, c)| c.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                // Use the thorough template-literal escape that
                // covers U+2028 / U+2029 and other control chars.
                // The previous escaper only handled `, \, and ${,
                // letting attacker-controlled CSS containing a raw
                // U+2028 break out of the template literal and run
                // arbitrary JS in the page's V8 realm.
                let escaped = escape_for_js_template_literal(&combined_css);
                let code = format!("globalThis.__diting_css = `{}`;", escaped);
                let _ = js.execute_script("<css>", &code);
            }
        }
        if let Some(js) = &mut self.js {
            let _ = js.execute_script("<iframe-load>",
                "(function() { var iframes = document.querySelectorAll('iframe[src],iframe[srcdoc]'); for (var i = 0; i < iframes.length; i++) { if (iframes[i].hasAttribute('srcdoc')) { iframes[i]._loadIframeSrc('about:srcdoc'); continue; } var src = iframes[i].getAttribute('src'); if (src && src !== 'about:blank') iframes[i]._loadIframeSrc(src); } })()");
        }

        // Spec: DOMContentLoaded fires AFTER parser-blocking scripts run,
        // not before. Skipping execute_scripts() on the DCL path meant
        // every inline <script> in the page was silently dropped: form
        // listeners never registered, frameworks never bootstrapped,
        // page.click() handlers were no-ops. Now scripts run regardless
        // of waitUntil and DCL means "DOM parsed AND scripts executed".
        self.execute_scripts().await;
        let scripts_ms = t_scripts.elapsed().as_micros();

        // One structured line per navigation (DITING_NAV_TIMING). The
        // settled/lifecycle split lives inside `scripts` — the dynamic
        // settle loops run as part of execute_scripts.
        let log_nav_timing = |netidle_ms: u128| {
            if diting_env::env_knobs::nav_timing() {
                tracing::info!(
                    "nav timing [{url_str}]: fetch {}us parse {}us css {}us realm {}us \
                     scripts {}us netidle {}us total {}us",
                    fetch_ms,
                    parse_ms,
                    css_ms,
                    realm_ms,
                    scripts_ms,
                    netidle_ms,
                    t_nav.elapsed().as_micros(),
                );
            }
        };

        self.lifecycle = LifecycleState::DomContentLoaded;

        if wait_until == crate::lifecycle::WaitUntil::DomContentLoaded {
            log_nav_timing(0);
            return Ok(());
        }

        if let Some(js) = &mut self.js {
            if let Ok(new_title) = js.evaluate("document.title") {
                if let Some(t) = new_title.as_str() {
                    self.document.set_title(t);
                }
            }
        }

        self.lifecycle = LifecycleState::Loaded;

        let t_netidle = std::time::Instant::now();
        if matches!(
            wait_until,
            crate::lifecycle::WaitUntil::NetworkIdle0 | crate::lifecycle::WaitUntil::NetworkIdle2
        ) {
            let threshold = match wait_until {
                crate::lifecycle::WaitUntil::NetworkIdle0 => 0,
                crate::lifecycle::WaitUntil::NetworkIdle2 => 2,
                _ => 0,
            };

            // Same hazard as the post-script settle: a synchronous commit can
            // pin the thread past the 5s network-idle deadline. Each pump is
            // bounded per task rather than against the whole idle window, so a
            // slow legitimate commit finishes while one true spin remains cut.
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
            let mut idle_since: Option<tokio::time::Instant> = None;

            loop {
                let active = self.http_client.active_requests();
                let now = tokio::time::Instant::now();

                if active <= threshold {
                    if idle_since.is_none() {
                        idle_since = Some(now);
                    }
                    if now.duration_since(idle_since.unwrap())
                        >= tokio::time::Duration::from_millis(500)
                    {
                        break;
                    }
                } else {
                    idle_since = None;
                }

                if now >= deadline {
                    tracing::debug!(
                        "Network idle timeout reached with {} active requests",
                        active
                    );
                    break;
                }

                if let Some(js) = &mut self.js {
                    let _ = js.run_event_loop_until_idle(50).await;
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }

            self.lifecycle = LifecycleState::NetworkIdle;
        }

        log_nav_timing(t_netidle.elapsed().as_micros());
        Ok(())
    }

    pub fn navigate_blank(&mut self) {
        self.snapshot_session_storage();
        if let Some(js) = self.js.as_mut() {
            let calls = js.take_pending_console_calls();
            self.suspended_console.extend(calls);
        }
        self.js = None;
        self.url = Some(Url::parse("about:blank").unwrap());
        self.dom = Some(parse_html(
            "<!DOCTYPE html><html><head></head><body></body></html>",
        ));
        // about:blank keeps the previous document's encoding and referrer
        // policy — this path never cleared them. The hop's referrer was
        // staged on the draft; a direct call has no draft and only clears
        // title, content type, and url on the live document. The realm is
        // already dropped, so mutating the live document does not change
        // one JS can still see.
        if let Some(pending) = self.pending_document.take() {
            self.document.set_referrer(&pending.referrer());
        }
        self.document.set_title("");
        self.document.set_content_type("");
        self.document.set_url("about:blank");
        self.lifecycle = LifecycleState::Loaded;
    }

    pub fn url_string(&self) -> String {
        self.url
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| "about:blank".to_string())
    }
}
