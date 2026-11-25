//! Script execution pipeline for the loaded document: inline/external scripts,
//! ES module graphs, module eval budgets and the env init hook. Split from
//! page/mod.rs (ARCHITECTURE.md P2 batch 3); behavior unchanged. This
//! module names the fetch policy and the eval budget a script run uses.

use url::Url;

use super::load::{decode_data_uri, subresource_allowed};
use super::Page;

/// Module evaluation budget (ms): the timeout driving a module's load + eval
/// to completion during the script phase. Default 10s;
/// `DITING_MODULE_EVAL_TIMEOUT_MS` raises it for slow module graphs
/// (dev-server HMR clients — obscura#531).
fn module_eval_budget_ms() -> u64 {
    module_eval_budget_from(
        std::env::var("DITING_MODULE_EVAL_TIMEOUT_MS")
            .ok()
            .as_deref(),
    )
}

pub(super) fn module_eval_budget_from(raw: Option<&str>) -> u64 {
    raw.and_then(|s| s.parse().ok()).unwrap_or(10_000)
}

fn resolve_module_specifier(src: &str, base_url: &str) -> String {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        src.to_string()
    } else {
        Url::parse(base_url)
            .ok()
            .and_then(|base| base.join(src).ok())
            .map(|u| u.to_string())
            .unwrap_or_else(|| src.to_string())
    }
}

/// Engine-side init hook: `DITING_INIT_SCRIPT=<path>` runs its file
/// contents before any page script on every navigation (same slot as CDP
/// preloads) — the pre-hydration instrumentation point for debugging apps
/// that bind during module eval.
fn env_init_script_source() -> Option<String> {
    env_init_script_from(std::env::var("DITING_INIT_SCRIPT").ok().as_deref())
}

pub(super) fn env_init_script_from(path: Option<&str>) -> Option<String> {
    let source = path.and_then(|p| std::fs::read_to_string(p).ok())?;
    if source.trim().is_empty() {
        None
    } else {
        Some(source)
    }
}

/// Engine-side signer-wait gate: `DITING_SIGNER_GATE=<url-regex>;<js-expr>;<wait-ms>`
/// arms the engine's own XHR send delegation (xhr.js `__ditingSignerGateHold`)
/// to hold matching requests until the expression resolves to the page's
/// signing function, then re-sign through that function. Apps whose boot
/// races their signing SDK (xiaohongshu ark: publish_render fires <1s in,
/// `proxy._webmsxyw` mounts ~6s later) 406-crash on every cold boot
/// otherwise. Zero page surface: the config global is filtered by the
/// internal-name enumeration boundary like every `__diting*` global.
fn env_signer_gate_source() -> Option<String> {
    signer_gate_source_from(std::env::var("DITING_SIGNER_GATE").ok().as_deref())
}

pub(super) fn signer_gate_source_from(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.split(';');
    let pattern = parts.next().unwrap_or("").trim();
    let signer = parts.next().unwrap_or("").trim();
    let wait_ms: u64 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(12_000);
    if pattern.is_empty() || signer.is_empty() {
        return None;
    }
    // JSON string literals are valid JS literals; the signer expression is
    // spliced in as a standalone literal (never re-embedded inside another
    // string — that would quote it into a constant). A bad regex dies in
    // the realm-side catch and leaves the gate off (engine default).
    let pattern = serde_json::to_string(pattern).ok()?;
    let signer = serde_json::to_string(signer).ok()?;
    Some(format!(
        "try{{Object.defineProperty(globalThis,'__ditingSignerGate',{{value:{{\
re:new RegExp({p}),signer:new Function(\"return (\" + {s} + \");\"),\
waitMs:{w},settleMs:2000,pollMs:50,seenAt:0,signed:0,\
resigned:0,passthrough:0,softfail:0,dropped:0,unsigned:0\
}},writable:true,configurable:true,enumerable:false}});}}catch(e){{}}",
        p = pattern,
        s = signer,
        w = wait_ms
    ))
}

impl Page {
    /// Resolve the document base URL per HTML spec:
    /// https://html.spec.whatwg.org/multipage/urls-and-fetching.html#document-base-url
    /// Falls back to self.url when no <base href> exists.
    pub(super) fn resolve_base_url(&self) -> Option<url::Url> {
        let doc_url = self.url.as_ref()?;
        let base_href: Option<String> = self.js.as_ref().and_then(|js| {
            js.with_dom(|dom| match dom.query_selector("base[href]") {
                Ok(Some(nid)) => dom
                    .get_node(nid)
                    .and_then(|n| n.get_attribute("href").map(|s| s.to_string())),
                _ => None,
            })
            .flatten()
        });
        match base_href {
            Some(href) => doc_url.join(&href).ok(),
            None => Some(doc_url.clone()),
        }
    }

    /// Store the caller-registered document-start group. Does not change
    /// the group that runs ([`Self::set_preload_scripts`]).
    pub fn set_user_preload_scripts(&mut self, scripts: Vec<String>) {
        self.user_preload_scripts = scripts;
    }

    pub fn user_preload_scripts(&self) -> &[String] {
        &self.user_preload_scripts
    }

    pub fn set_preload_scripts(&mut self, scripts: Vec<String>) {
        self.preload_scripts = scripts;
    }

    /// Register one more document-start script (CDP
    /// `Page.addScriptToEvaluateOnNewDocument`). Both groups move: the
    /// registered group is what the session recomputes the running group
    /// from at its next navigation, and the running group is what runs if
    /// the page navigates itself first — nothing recomputes that path.
    pub fn add_preload_script(&mut self, script: String) {
        self.user_preload_scripts.push(script.clone());
        self.preload_scripts.push(script);
    }

    /// Drop the newest registration whose source is exactly `script` (CDP
    /// `Page.removeScriptToEvaluateOnNewDocument`), from both groups.
    /// False when no registration holds that source.
    pub fn remove_preload_script(&mut self, script: &str) -> bool {
        let dropped = match self.user_preload_scripts.iter().rposition(|s| s == script) {
            Some(i) => {
                self.user_preload_scripts.remove(i);
                true
            }
            None => false,
        };
        if let Some(i) = self.preload_scripts.iter().rposition(|s| s == script) {
            self.preload_scripts.remove(i);
        }
        dropped
    }

    pub(super) async fn execute_scripts(&mut self) {
        tracing::info!(
            "execute_scripts called, js runtime exists: {}",
            self.js.is_some()
        );
        // Compute document base URL, respecting <base href>.
        let document_base = self.resolve_base_url();
        // Soft deadline on the fetch phase of script execution. Heavy SPAs
        // (GitHub, Linear, CodeSandbox) ship 50+ scripts and our serial
        // fetch + execute loop can blow past a 25s Puppeteer goto timeout.
        // Override via DITING_SCRIPT_DEADLINE_MS for slow networks.
        // The execution phase gets the same budget AGAIN, armed after the
        // fetch phase (#79) — a network stall that eats the whole fetch
        // deadline must not also behead the CPU phase.
        let script_deadline_ms: u64 = std::env::var("DITING_SCRIPT_DEADLINE_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000);
        let script_deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_millis(script_deadline_ms);

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ScriptKind {
            Classic,
            Module,
            ImportMap,
        }

        #[derive(Debug)]
        struct ScriptInfo {
            src: Option<String>,
            inline: String,
            is_defer: bool,
            is_async: bool,
            kind: ScriptKind,
            nid: u32,
            /// Document base URL at this element's parser encounter point.
            /// A later <base href> must not rebase an earlier import map
            /// (upstream 34373c3 temporal-base semantics).
            base_url: String,
        }

        let all_scripts = match &self.js {
            Some(js) => {
                let document_url = self.url_string();
                js.with_dom(|dom| {
                    let script_ids = dom.query_selector_all("script").unwrap_or_default();
                    // Walk the tree once tracking the <base href> in effect at
                    // each script's encounter position.
                    let mut bases_at_script = std::collections::HashMap::new();
                    let mut active_base = Url::parse(&document_url).ok();
                    let mut found_base = false;
                    if let Some(root) = Some(dom.document()) {
                        for nid in dom.descendants(root) {
                            let Some(node) = dom.get_node(nid) else {
                                continue;
                            };
                            let Some(name) = node.as_element() else {
                                continue;
                            };
                            if name.local.as_str() == "base" && !found_base {
                                if let Some(href) = node.get_attribute("href") {
                                    found_base = true;
                                    if let Some(resolved) =
                                        active_base.as_ref().and_then(|base| base.join(href).ok())
                                    {
                                        active_base = Some(resolved);
                                    }
                                }
                            } else if name.local.as_str() == "script" {
                                bases_at_script.insert(
                                    nid.raw(),
                                    active_base
                                        .as_ref()
                                        .map(ToString::to_string)
                                        .unwrap_or_else(|| document_url.clone()),
                                );
                            }
                        }
                    }
                    let mut scripts = Vec::new();

                    for sid in script_ids {
                        if let Some(node) = dom.get_node(sid) {
                            let src = node.get_attribute("src").map(|s| s.to_string());
                            let script_type = node
                                .get_attribute("type")
                                .unwrap_or("")
                                .trim()
                                .to_ascii_lowercase();
                            let is_defer = node.get_attribute("defer").is_some();
                            let is_async = node.get_attribute("async").is_some();
                            let kind = match script_type.as_str() {
                                "module" => ScriptKind::Module,
                                "importmap" => ScriptKind::ImportMap,
                                "" | "text/javascript" | "application/javascript" => {
                                    ScriptKind::Classic
                                }
                                _ => continue,
                            };

                            let inline_code = if src.is_none() {
                                dom.text_content(sid)
                            } else {
                                String::new()
                            };

                            if matches!(kind, ScriptKind::ImportMap)
                                || src.is_some()
                                || !inline_code.trim().is_empty()
                            {
                                scripts.push(ScriptInfo {
                                    src,
                                    inline: inline_code,
                                    is_defer,
                                    is_async,
                                    kind,
                                    nid: sid.raw(),
                                    base_url: bases_at_script
                                        .get(&sid.raw())
                                        .cloned()
                                        .unwrap_or_else(|| document_url.clone()),
                                });
                            }
                        }
                    }
                    scripts
                })
                .unwrap_or_default()
            }
            None => return,
        };

        // Parser-order script visibility (#120): Chrome executes a classic
        // script before the parser has inserted any LATER script elements, so
        // self-locating loaders (KISSY reverse-scans
        // getElementsByTagName('script') for their own tag) must not see them.
        // Our pipeline parses the whole document first, so we emulate the
        // contract with a JS-side ceiling: script queries drop parser scripts
        // positioned after the running one (bootstrap.js _parserVisibleIds).
        // The order list comes from the DOM script query — every <script>
        // element in the document, including empty/foreign-type ones the
        // execution list below legitimately skips.
        let parser_order: Vec<u32> = match &self.js {
            Some(js) => js
                .with_dom(|dom| {
                    dom.query_selector_all("script")
                        .unwrap_or_default()
                        .into_iter()
                        .map(|sid| sid.raw())
                        .collect::<Vec<u32>>()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let parser_idx: std::collections::HashMap<u32, usize> = parser_order
            .iter()
            .enumerate()
            .map(|(i, nid)| (*nid, i))
            .collect();

        // Import maps register before any module graph starts (upstream
        // 34373c3). Parser-discovered maps merge in encounter order using the
        // base URL in effect at each element; a later map cannot rebind a
        // specifier an earlier resolution already observed.
        for script in &all_scripts {
            if script.kind == ScriptKind::ImportMap {
                if script.src.is_some() {
                    tracing::warn!("External import maps are not supported");
                    continue;
                }
                if let Some(js) = &self.js {
                    if let Err(error) = js.add_import_map(&script.inline, &script.base_url) {
                        tracing::warn!("Ignoring invalid import map: {}", error);
                    }
                }
            }
        }

        let mut regular = Vec::new();
        let mut deferred = Vec::new();
        let mut async_scripts = Vec::new();

        let mut module_scripts: Vec<ScriptInfo> = Vec::new();

        for script in all_scripts {
            match script.kind {
                ScriptKind::Module => module_scripts.push(script),
                ScriptKind::ImportMap => continue,
                ScriptKind::Classic => {
                    if script.is_defer {
                        deferred.push(script);
                    } else if script.is_async {
                        async_scripts.push(script);
                    } else {
                        regular.push(script);
                    }
                }
            }
        }

        let scripts = regular;

        tracing::info!(
            "Found {} regular + {} deferred + {} async scripts",
            scripts.len(),
            deferred.len(),
            async_scripts.len()
        );
        let all_to_execute: Vec<ScriptInfo> = scripts
            .into_iter()
            .chain(deferred)
            .chain(async_scripts)
            .collect();

        let mut resolved: Vec<(usize, String)> = Vec::new();
        let mut fetch_tasks: Vec<(usize, String)> = Vec::new();

        for (i, script) in all_to_execute.iter().enumerate() {
            if let Some(src_url) = &script.src {
                let full_url = if src_url.starts_with("http://") || src_url.starts_with("https://")
                {
                    src_url.clone()
                } else if let Some(base) = &document_base {
                    base.join(src_url)
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| src_url.clone())
                } else {
                    src_url.clone()
                };

                if !subresource_allowed(self.url.as_ref(), &full_url) {
                    // Block file://, data:, javascript:, and other
                    // off-origin schemes from being injected as a
                    // <script src>. Without this an http page can
                    // include <script src="file:///etc/passwd"> and
                    // see the body parsed as JS source.
                    tracing::warn!(
                        "blocking cross-scheme <script src>: page={} src={}",
                        self.url_string(),
                        full_url,
                    );
                    continue;
                }
                if self.url_blocked(&full_url) {
                    tracing::info!("Blocked script by Network.setBlockedURLs: {}", full_url);
                    continue;
                }
                resolved.push((i, full_url.clone()));
                fetch_tasks.push((i, full_url));
            }
        }

        let client = self.http_client.clone();
        // Stealth pages already fetched the document on this client. Classic
        // script src joins it so load.js does not open a second HTTP stack.
        let stealth = self.stealth_client.clone();
        let script_callbacks = self.callbacks.clone();
        let doc_referrer = self.url.as_ref().map(|u| u.to_string());
        let fetch_futures: Vec<_> = fetch_tasks
            .iter()
            .map(|(idx, url)| {
                let client = client.clone();
                let stealth = stealth.clone();
                let script_callbacks = script_callbacks.clone();
                let url = url.clone();
                let idx = *idx;
                let doc_referrer = doc_referrer.clone();
                async move {
                    let parsed =
                        Url::parse(&url).unwrap_or_else(|_| Url::parse("about:blank").unwrap());
                    if parsed.scheme() == "data" {
                        // data: URIs are inline; decode locally, no network fetch.
                        // Instagram and other Meta properties serve their bootstrap
                        // as <script src="data:application/x-javascript;base64,...">.
                        let body = decode_data_uri(&url).unwrap_or_default();
                        let content_type = url
                            .strip_prefix("data:")
                            .and_then(|s| s.split(',').next())
                            .unwrap_or("application/javascript")
                            .split(';')
                            .next()
                            .unwrap_or("application/javascript")
                            .to_string();
                        let mut headers = std::collections::HashMap::new();
                        headers.insert("content-type".to_string(), content_type);
                        let resp = diting_net::Response {
                            url: parsed,
                            status: 200,
                            headers,
                            body,
                            redirected_from: Vec::new(),
                            redirect_hops: Vec::new(),
                            request_headers: std::collections::HashMap::new(),
                        };
                        return Some((idx, url, resp, None));
                    }
                    // #126: monotonic anchor for the live-fetch duration below.
                    let rt_t0 = std::time::Instant::now();
                    let fetched = if let Some(stealth) = stealth.as_ref() {
                        stealth.fetch_script(&parsed, doc_referrer.as_deref()).await
                    } else {
                        client
                            .fetch_with_callbacks(
                                &parsed,
                                Some(script_callbacks.as_ref()),
                                diting_net::ResourceType::Script,
                                doc_referrer.as_deref(),
                            )
                            .await
                    };
                    match fetched {
                        Ok(resp) => {
                            // The plain path already notified observers. The
                            // wreq path has no callback hook, so fire them here
                            // with the headers that actually went out.
                            if stealth.is_some()
                                && (script_callbacks.has_request_callbacks().await
                                    || script_callbacks.has_response_callbacks().await)
                            {
                                let info = diting_net::RequestInfo {
                                    url: parsed.clone(),
                                    method: "GET".to_string(),
                                    headers: resp.request_headers.clone(),
                                    resource_type: diting_net::ResourceType::Script,
                                };
                                script_callbacks.fire_request(&info).await;
                                script_callbacks.fire_response(&info, &resp).await;
                            }
                            // #126: real timing around the live fetch only —
                            // data: URIs never touched the wire, so they carry
                            // no record. Epoch stamp feeds the JS resource
                            // clock; Instant gives the monotonic duration.
                            let rt_start_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs_f64() * 1000.0)
                                .unwrap_or(0.0);
                            let dur_ms = rt_t0.elapsed().as_secs_f64() * 1000.0;
                            Some((idx, url, resp, Some((rt_start_ms, dur_ms))))
                        }
                        Err(e) => {
                            tracing::warn!("Failed to fetch script {}: {}", url, e);
                            None
                        }
                    }
                }
            })
            .collect();

        // Bound concurrency: a page with 100 external scripts would
        // otherwise open 100 sockets at once, exhausting the connection
        // pool / ephemeral ports and triggering OS-level backpressure.
        // 16 is well above the per-host pool ceiling most browsers use
        // and matches what real Chrome does for a given origin.
        use futures::StreamExt as _;
        let external_total = fetch_futures.len();
        // Per fetch: does it block parsing (classic sync / defer — Chrome
        // waits for these however long the network takes), and has its
        // response started (headers back = a live transfer, not a dead
        // server)? See the deadline rule on the loop below.
        let blocking: Vec<bool> = fetch_tasks
            .iter()
            .map(|(idx, _)| !all_to_execute[*idx].is_async)
            .collect();
        let started: Vec<std::sync::Arc<std::sync::atomic::AtomicBool>> = (0..external_total)
            .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
            .collect();
        let fetch_futures: Vec<_> = fetch_futures
            .into_iter()
            .enumerate()
            .map(|(k, f)| {
                let flag = started[k].clone();
                async move { (k, diting_net::progress::track_response_start(flag, f).await) }
            })
            .collect();
        let mut settled = vec![false; external_total];
        // #79: count arrivals so a fetch-phase overrun can say how much of
        // the page's script surface landed before the deadline — the item-
        // wise loop below keeps those bodies, and the warn names the count.
        // The count lives on the page's shared `NavProgress` cell, not a
        // local: the /fetch starvation checkpoint polls it while this
        // phase's futures are still in flight, and a local counter would
        // be invisible (and dead the moment the navigation future is
        // dropped).
        self.nav_progress.note_script_phase(external_total);
        let arrivals_counter = std::sync::Arc::clone(&self.nav_progress);
        let fetch_stream = futures::stream::iter(fetch_futures)
            .buffer_unordered(16)
            .inspect(move |(_, result)| {
                if result.is_some() {
                    arrivals_counter.note_script_arrival();
                }
            });
        // #79: drive the stream item-wise instead of collect() — collect is
        // all-or-nothing, so one hanging URL used to discard the bodies that
        // had already landed. On deadline we keep whatever arrived; the
        // execution phase below runs those bodies plus every inline script
        // on a fresh budget.
        //
        // The deadline cuts DEAD fetches, not slow ones: past it we keep
        // waiting while some parse-blocking script's response has started
        // (its body is still flowing — the transport's own read-stall
        // timeout ends it if it stops), and give up on the rest: fetches
        // with no response yet (the hung server #79 pinned) and async
        // scripts (Chrome doesn't block on them). x.com behind a slow proxy
        // was the case: its vendor bundle was live at 10s, got dropped, main
        // ran without it and the app reported /i/script-load-failure.
        // SCRIPT_FETCH_BACKSTOP bounds a body that trickles forever.
        const SCRIPT_FETCH_BACKSTOP: tokio::time::Duration = tokio::time::Duration::from_secs(120);
        let fetch_backstop = script_deadline
            + SCRIPT_FETCH_BACKSTOP
                .saturating_sub(tokio::time::Duration::from_millis(script_deadline_ms));
        let mut fetch_stream = std::pin::pin!(fetch_stream);
        let mut fetch_results: Vec<_> = Vec::with_capacity(external_total);
        let mut cut = false;
        loop {
            let now = tokio::time::Instant::now();
            if now >= script_deadline {
                let live = (0..external_total).any(|k| {
                    blocking[k]
                        && !settled[k]
                        && started[k].load(std::sync::atomic::Ordering::Relaxed)
                });
                if !live || now >= fetch_backstop {
                    cut = true;
                    break;
                }
            }
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), fetch_stream.next())
                .await
            {
                Ok(Some((k, item))) => {
                    settled[k] = true;
                    fetch_results.push(item);
                }
                // Stream drained: every fetch future settled or errored out.
                Ok(None) => break,
                // Slice elapsed with work still in flight — re-check the
                // deadline at the top of the loop.
                Err(_) => {}
            }
        }
        if cut {
            tracing::warn!(
                "execute_scripts: fetch deadline reached after {}/{} script bodies \
                 arrived; the {} arrived bodies and all inline scripts still execute \
                 on a fresh budget — raise via DITING_SCRIPT_DEADLINE_MS",
                self.nav_progress.script_arrivals(),
                external_total,
                self.nav_progress.script_arrivals(),
            );
        }

        let mut fetched: std::collections::HashMap<usize, (String, String, diting_net::Response)> =
            std::collections::HashMap::new();
        let mut script_timings: Vec<diting_js::ops::ResourceTimingRecord> = Vec::new();
        for (idx, url, resp, rt) in fetch_results.into_iter().flatten() {
            // Script bodies: only the HTTP Content-Type charset matters
            // (no in-band meta-charset for JS).
            let code = diting_net::decode_non_html(&resp.body, resp.content_type());
            // #126: real per-script timing — only wire fetches carry a
            // sample, and the blocking posture comes from the element
            // itself (defer/async scripts don't block the parser).
            if let Some((start_ms, dur_ms)) = rt {
                let s = all_to_execute.get(idx);
                script_timings.push(diting_js::ops::ResourceTimingRecord {
                    name: url.clone(),
                    initiator: "script",
                    start_epoch_ms: start_ms,
                    duration_ms: dur_ms,
                    status: resp.status,
                    body_size: resp.body.len(),
                    render_blocking: !s.is_some_and(|s| s.is_defer || s.is_async),
                });
            }
            fetched.insert(idx, (url, code, resp));
        }
        if let Some(js) = &mut self.js {
            js.push_resource_timings(script_timings);
        }

        // Spec: readyState is "loading" while parser-discovered scripts execute.
        // Scripts that check readyState === 'loading' will register DOMContentLoaded
        // listeners instead of calling their callback immediately.
        if let Some(js) = &mut self.js {
            let _ = js.execute_script(
                "<ready-state>",
                "globalThis.__documentReadyState__ = 'loading';",
            );
            // #48: window named access must be live before ANY page script
            // runs — Chrome resolves id/name elements onto the global before
            // the first script, and a script's first statement may be a bare
            // identifier (`hero.focus()`), which never touches `document`.
            // The JS side is idempotent (_namedScanned), so this is a no-op
            // after the first navigation script phase.
            let _ = js.execute_script(
                "<named-boot>",
                "globalThis._namedBoot&&globalThis._namedBoot();",
            );
            // The document URL (including its fragment) is stamped after the
            // JS snapshot boots, so :target is still empty until this runs.
            // Page scripts and the load listener both read it.
            let _ = js.execute_script(
                "<css-target>",
                "if (typeof __diting_syncCssTarget === 'function') __diting_syncCssTarget();",
            );
            // Parser-built stylesheets never pass through the JS insertion
            // hooks (js/bootstrap.js), so their load events only fire if we
            // enumerate them here — before the script loop, so inline scripts
            // that register listeners still catch the queued tasks, matching
            // Chrome's task-after-sheet-applies ordering.
            let _ = js.execute_script(
                "<initial-sheets>",
                "if (typeof __prepareInitialStylesheets === 'function') __prepareInitialStylesheets();",
            );
        }

        // CDP `Page.addScriptToEvaluateOnNewDocument` contract: preload
        // sources must run BEFORE any of the page's own scripts. This is
        // also where puppeteer's `exposeFunction` wrapper installs itself —
        // if preload runs after page scripts, every early binding call
        // hits an undefined function and silently no-ops.
        let preload_sources = self.preload_scripts.clone();
        static ENV_INIT_SCRIPT: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        let env_init = ENV_INIT_SCRIPT.get_or_init(env_init_script_source);
        // Same slot, different lifecycle: the signer gate's realm state
        // (seenAt/signed counters) must reset per navigation, so this one is
        // NOT a OnceLock — the source is, the object it defines is fresh
        // every page.
        static SIGNER_GATE_SOURCE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        let signer_gate_src = SIGNER_GATE_SOURCE
            .get_or_init(env_signer_gate_source)
            .clone();
        if let Some(js) = &mut self.js {
            if let Some(source) = signer_gate_src.as_deref() {
                let _ = js.execute_script("<signer-gate>", source);
            }
            for source in &preload_sources {
                if let Err(e) = js.execute_script_guarded("<preload>", source.as_str()) {
                    tracing::debug!("Preload script error: {}", e);
                }
            }
            if let Some(source) = env_init.as_deref() {
                if let Err(e) = js.execute_script_guarded("<init-script>", source) {
                    tracing::debug!("Init script error: {}", e);
                }
            }
            // #120: hand bootstrap.js the document-order parser script list.
            // The ceiling starts at -1 (nothing hidden) and each classic
            // script's prologue bumps it to its own index.
            if let Ok(order) = serde_json::to_string(&parser_order) {
                let _ = js.execute_script(
                    "<parser-order>",
                    &format!(
                        "globalThis.__parserScriptOrder={o};\
                         globalThis.__parserScriptIdx=new Map(globalThis.__parserScriptOrder.map(function(n,i){{return[n,i];}}));\
                         globalThis.__parserScriptCeiling=-1;",
                        o = order
                    ),
                );
            }
        }

        // #79: execution gets its own budget, armed AFTER the fetch phase.
        // The soft deadline bounds the serial classic loop; the watchdog is
        // the hard backstop for inline scripts, which run back-to-back with
        // no await between them, so neither the soft deadline (only checked
        // between scripts) nor the per-script guard can interrupt a page
        // burning the budget across many synchronous scripts (the real-world
        // SPA / anti-bot busy-loop hang — the xiaohongshu jsvmp-scan shape
        // that filed this issue).
        let exec_deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_millis(script_deadline_ms);
        let mut exec_wd = self
            .js
            .as_mut()
            .map(|js| js.arm_watchdog(std::time::Duration::from_millis(script_deadline_ms + 1000)));

        for (i, script) in all_to_execute.iter().enumerate() {
            // Both exit conditions matter: the wall-clock deadline stops the
            // serial classic phase from eating the whole navigation budget,
            // and a watchdog that already fired means the isolate carries a
            // pending termination — every further guarded execution in this
            // loop would instantly fail, so stop burning scripts on it.
            if tokio::time::Instant::now() >= exec_deadline
                || exec_wd.as_ref().is_some_and(|t| t.fired())
            {
                // #79: name the script that consumed the budget — it is the
                // one just before the first skipped index. "skipping N" alone
                // left a xiaohongshu jsvmp-scan overrun unattributed for an
                // evening of manual bisecting.
                let burner = if i > 0 {
                    all_to_execute
                        .get(i - 1)
                        .map(|s| match s.src.as_deref() {
                            Some(url) => format!("external {url}"),
                            None => format!("inline [nid {}, {} bytes]", s.nid, s.inline.len()),
                        })
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    "the fetch phase alone (network)".to_string()
                };
                tracing::warn!(
                    "execute_scripts: classic budget ({}ms, DITING_SCRIPT_DEADLINE_MS) \
                     exhausted after {}; skipping {} remaining scripts",
                    script_deadline_ms,
                    burner,
                    all_to_execute.len() - i,
                );
                break;
            }
            if script.src.is_some() {
                if let Some((url, code, resp)) = fetched.remove(&i) {
                    tracing::info!("Executing script ({} bytes): {}", code.len(), url);
                    self.record_network_event_with_body(
                        &url,
                        "GET",
                        "Script",
                        resp.status,
                        &resp.headers,
                        &resp.request_headers,
                        &resp.body,
                    );
                    if let Some(js) = &mut self.js {
                        // #120: classic sync scripts run at their parser
                        // position — later parser scripts are invisible.
                        // Deferred/async run after parsing completes: ceiling
                        // -1 keeps everything visible for them.
                        let ceiling = if script.is_defer || script.is_async {
                            -1
                        } else {
                            parser_idx.get(&script.nid).map(|i| *i as i64).unwrap_or(-1)
                        };
                        let _ = js.execute_script("<current-script>", &format!(
                            "globalThis.__currentScriptNid={nid};globalThis.__parserScriptCeiling={ceiling};",
                            nid = script.nid, ceiling = ceiling));
                        let script_t0 = std::time::Instant::now();
                        if let Err(e) = js.execute_script_guarded(&url, &code) {
                            tracing::warn!("Script error ({}): {}", url, e);
                            // Chrome parity (#17): the uncaught throw must
                            // reach the window error hooks (onerror +
                            // ErrorEvent('error')) and the console, not just
                            // this host-side log line.
                            let msg = e.strip_prefix("JS error: ").unwrap_or(&e);
                            if let (Ok(m), Ok(s)) = (
                                serde_json::to_string(msg),
                                serde_json::to_string(url.as_str()),
                            ) {
                                let _ = js.execute_script(
                                    "<window-error>",
                                    &format!(
                                        "globalThis.__diting_reportUncaught({m}, {s}, 0, null);"
                                    ),
                                );
                            }
                        }
                        let exec_ms = script_t0.elapsed().as_millis();
                        if exec_ms >= 100 {
                            // #29: name the CPU hogs eating the classic budget
                            // — cumulative 10s exhaustion skips the rest of the
                            // SPA's scripts and the page never finishes booting.
                            tracing::warn!(
                                "slow script: {url} took {exec_ms}ms (classic budget {script_deadline_ms}ms)"
                            );
                        }
                        let _ = js
                            .execute_script("<current-script>", "globalThis.__currentScriptNid=0;");
                        // Chrome parity: an executed parser <script src> fires
                        // its element 'load' as a task. doudian's SSR bootstrap
                        // hooks `document.currentScript.previousSibling
                        // .addEventListener('load', ...)` and resolves its
                        // firstReadyDefer from there — without this event the
                        // publish sub-app never mounts (#203).
                        let _ = js.execute_script(
                            "<script-load-event>",
                            &format!(
                                "globalThis.__fireParserScriptEvent&&__fireParserScriptEvent({nid},'load');",
                                nid = script.nid
                            ),
                        );
                    }
                } else {
                    // Fetch failed or was blocked: Chrome fires 'error' on the
                    // element. Listeners gating on load/error must not hang.
                    if let Some(js) = &mut self.js {
                        let _ = js.execute_script(
                            "<script-error-event>",
                            &format!(
                                "globalThis.__fireParserScriptEvent&&__fireParserScriptEvent({nid},'error');",
                                nid = script.nid
                            ),
                        );
                    }
                }
            } else if !script.inline.is_empty() {
                let doc_url = self.url_string();
                if let Some(js) = &mut self.js {
                    tracing::info!(
                        "Executing inline script ({} bytes) [nid {}]",
                        script.inline.len(),
                        script.nid
                    );
                    let _ = js.execute_script(
                        "<current-script>",
                        &format!(
                            "globalThis.__currentScriptNid={nid};globalThis.__parserScriptCeiling={ceiling};",
                            nid = script.nid,
                            ceiling = if script.is_defer || script.is_async {
                                -1
                            } else {
                                parser_idx.get(&script.nid).map(|i| *i as i64).unwrap_or(-1)
                            }
                        ),
                    );
                    let inline_t0 = std::time::Instant::now();
                    if let Err(e) = js.execute_script_guarded("<inline>", &script.inline) {
                        tracing::warn!("Inline script error: {}", e);
                        // Same window-error reporting as external scripts
                        // (#17); Chrome attributes inline throws to the
                        // document URL.
                        let msg = e.strip_prefix("JS error: ").unwrap_or(&e);
                        if let (Ok(m), Ok(s)) = (
                            serde_json::to_string(msg),
                            serde_json::to_string(doc_url.as_str()),
                        ) {
                            let _ = js.execute_script(
                                "<window-error>",
                                &format!("globalThis.__diting_reportUncaught({m}, {s}, 0, null);"),
                            );
                        }
                    }
                    let inline_ms = inline_t0.elapsed().as_millis();
                    if inline_ms >= 100 {
                        tracing::warn!(
                            "slow inline script [nid {}] took {inline_ms}ms (classic budget {script_deadline_ms}ms)",
                            script.nid
                        );
                    }
                    let _ =
                        js.execute_script("<current-script>", "globalThis.__currentScriptNid=0;");
                }
            }
        }

        // #120: parsing is "done" for every later phase — deferred scripts,
        // module graphs, DCL/load handlers and the settle loop all run with
        // the full document visible. Also covers the budget-break path, where
        // the last classic script's ceiling would otherwise stay armed.
        if let Some(js) = &mut self.js {
            let _ = js.execute_script("<parser-clear>", "globalThis.__parserScriptCeiling=-1;");
        }

        // Retire the classic-phase watchdog at the phase boundary. When the
        // phase overran (slow scripts, or a spinner the 5s guard killed),
        // the watchdog has already called terminate_execution() and the
        // isolate carries a pending termination: every subsequent
        // execute_script fails instantly, the module phase silently no-ops,
        // and the <load-events> script below never flips readyState or
        // fires DOMContentLoaded — the page is then wedged in "loading"
        // forever (weixin article pages; v0.3.1 Windows report P1-3).
        // disarm_watchdog cancels the termination, healing the isolate for
        // the phases that follow. Chrome semantics: a killed script ends
        // the script, never the document's load lifecycle.
        if let Some(token) = exec_wd.take() {
            if let Some(js) = self.js.as_mut() {
                if js.disarm_watchdog(token) {
                    tracing::warn!(
                        "execute_scripts: classic script phase overran the watchdog; \
                         module + load phases continue on a recovered isolate"
                    );
                }
            }
        }

        // Module scripts download in parallel, then evaluate in document
        // order. A Vite page lists every chunk as its own <script
        // type=module>, entry last. The old loop fetched each one inside
        // the same 10s that also evaluated it, so the entry was skipped
        // once the deadline passed (AdaptTable's RTL demos: 0 table
        // headers). Classic scripts already split the two phases (#79).
        let mut module_sources: std::collections::HashMap<String, Result<String, String>> =
            std::collections::HashMap::new();
        let mut network_urls: Vec<String> = Vec::new();
        for script in &module_scripts {
            let Some(src) = script.src.as_deref() else {
                continue;
            };
            let full_url = resolve_module_specifier(src, &script.base_url);
            if !subresource_allowed(self.url.as_ref(), &full_url) {
                module_sources.insert(
                    full_url.clone(),
                    Err(format!("blocked cross-scheme module src {full_url}")),
                );
                continue;
            }
            if self.url_blocked(&full_url) {
                module_sources.insert(
                    full_url.clone(),
                    Err(format!("blocked by Network.setBlockedURLs: {full_url}")),
                );
                continue;
            }
            if let Some(body) = decode_data_uri(&full_url) {
                module_sources.insert(full_url, Ok(String::from_utf8_lossy(&body).into_owned()));
                continue;
            }
            if full_url.starts_with("http://") || full_url.starts_with("https://") {
                network_urls.push(full_url);
            } else {
                module_sources.insert(
                    full_url.clone(),
                    Err(format!("unsupported module url {full_url}")),
                );
            }
        }
        network_urls.sort();
        network_urls.dedup();
        let referrer = self.url.as_ref().map(|u| u.to_string());
        module_sources.extend(
            prefetch_module_sources(
                self.http_client.clone(),
                network_urls,
                referrer,
                script_deadline_ms,
            )
            .await,
        );
        let module_deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_millis(script_deadline_ms);
        for module_script in &module_scripts {
            if tokio::time::Instant::now() >= module_deadline {
                tracing::warn!(
                    "execute_scripts: module-phase deadline reached, skipping remaining module scripts"
                );
                break;
            }
            if let Some(ref src) = module_script.src {
                let full_url = resolve_module_specifier(src, &module_script.base_url);
                tracing::info!("Loading ES module: {}", full_url);
                let Some(source) = module_sources.get(&full_url) else {
                    tracing::warn!(
                        "ES module skipped ({}): not downloaded before the fetch deadline",
                        full_url
                    );
                    continue;
                };
                let source = match source {
                    Ok(code) => code.clone(),
                    Err(e) => {
                        tracing::warn!("ES module error ({}): {}", full_url, e);
                        continue;
                    }
                };
                if let Some(js) = &mut self.js {
                    match js
                        .load_module_from_source(&full_url, source, module_eval_budget_ms())
                        .await
                    {
                        Ok(()) => {
                            tracing::info!("ES module loaded: {}", full_url);
                            // The prefetch already did the wire fetch. This
                            // row is the success marker, no outbound set.
                            self.record_network_event(
                                &full_url,
                                "GET",
                                "Script",
                                200,
                                &std::collections::HashMap::new(),
                                &std::collections::HashMap::new(),
                                0,
                            );
                        }
                        Err(e) => {
                            tracing::warn!("ES module error ({}): {}", full_url, e);
                        }
                    }
                }
            } else if !module_script.inline.is_empty() {
                let base = module_script.base_url.clone();
                if let Some(js) = &mut self.js {
                    if let Err(e) = js
                        .load_inline_module(&module_script.inline, &base, module_eval_budget_ms())
                        .await
                    {
                        tracing::warn!("Inline ES module error: {}", e);
                    }
                }
            }
        }

        if let Some(js) = &mut self.js {
            // Spec order: readyState -> interactive, fire DOMContentLoaded on
            // both document and window. The window `load` event intentionally
            // does NOT fire here: script elements — parser-inserted and
            // dynamically-inserted alike — delay the load event until they
            // execute (#44), so load waits for the settle loop below. A
            // listener registered by a late-arriving dynamic script
            // (proxydetect-style visual suites hang their whole scheduler on
            // `window.addEventListener("load", …)`) must catch the event.
            // A page's DOMContentLoaded listener can spin exactly like a
            // script can — without a bound it would pin this synchronous
            // execute_script (and the whole session thread) forever. 5s
            // matches the per-script guard.
            let dcl_wd = js.arm_watchdog(std::time::Duration::from_secs(5));
            let _ = js.execute_script("<dcl-events>",
                "globalThis.__documentReadyState__ = 'interactive';\n\
                 try { document.dispatchEvent(new Event('DOMContentLoaded', {bubbles:false,cancelable:false})); } catch(e) {}\n\
                 try { window.dispatchEvent(new Event('DOMContentLoaded', {bubbles:false,cancelable:false})); } catch(e) {}");
            js.disarm_watchdog(dcl_wd);
        }

        // (#44) Give dynamically-inserted scripts their landing window before
        // load fires (script elements delay the load event until they
        // execute, parser-inserted or not).
        self.settle_pending_work().await;

        if let Some(js) = &mut self.js {
            // (#44) Now that the settle loop has let dynamically-inserted
            // scripts land and execute, the load lifecycle closes: readyState
            // flips to complete and load fires. The dispatch fires every
            // handler path exactly once, with a real Event: the window.onload
            // property via __windowOnHandlers, and the `<body onload="...">`
            // content attribute via the body-reflecting fallback in the
            // window dispatch wrapper (the #37 contract — byte-WAF challenge
            // pages drive their whole PoW from `<body onload="readygo()">`).
            // A load listener can spin exactly like a script can, so the 5s
            // watchdog bound carries over.
            let load_wd = js.arm_watchdog(std::time::Duration::from_secs(5));
            let _ = js.execute_script("<load-events>",
                "globalThis.__documentReadyState__ = 'complete';\n\
                 try { window.dispatchEvent(new Event('load', {bubbles:false,cancelable:false})); } catch(e) {}");
            js.disarm_watchdog(load_wd);
        }

        // Load handlers routinely kick off their own async work: the
        // byte-WAF PoW spins a setInterval from `<body onload="readygo()">`
        // that ends in a location.reload(), analytics fire fetches. Before
        // the #44 reorder this work was pumped by the settle loop above
        // (load fired first); now it needs its own bounded window or the
        // JS-triggered navigation chain started from a load handler never
        // lands before the navigation call returns.
        self.settle_pending_work().await;
    }
}

/// Download external module-script sources together. The deadline keeps
/// bodies that already arrived and drops the rest; one hung URL must not
/// throw away the entry chunk that finished beside it.
async fn prefetch_module_sources(
    client: std::sync::Arc<diting_net::HttpClient>,
    urls: Vec<String>,
    referrer: Option<String>,
    budget_ms: u64,
) -> std::collections::HashMap<String, Result<String, String>> {
    let total = urls.len();
    if total == 0 {
        return std::collections::HashMap::new();
    }
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(budget_ms);
    let fetches = urls.into_iter().map(|url| {
        let client = client.clone();
        let referrer = referrer.clone();
        async move {
            let result = match Url::parse(&url) {
                Ok(spec) => match client
                    .fetch_with_callbacks(
                        &spec,
                        None,
                        diting_net::ResourceType::ModuleScript,
                        referrer.as_deref(),
                    )
                    .await
                {
                    Ok(resp) if (200..=299).contains(&resp.status) => {
                        Ok(diting_net::decode_non_html(&resp.body, resp.content_type()))
                    }
                    Ok(resp) => Err(format!("Module {url} returned HTTP {}", resp.status)),
                    Err(e) => Err(format!("Module fetch failed ({url}): {e}")),
                },
                Err(e) => Err(format!("Invalid module URL {url}: {e}")),
            };
            (url, result)
        }
    });
    use futures::StreamExt as _;
    let mut stream = std::pin::pin!(futures::stream::iter(fetches).buffer_unordered(16));
    let mut out = std::collections::HashMap::with_capacity(total);
    let mut cut = false;
    loop {
        if tokio::time::Instant::now() >= deadline {
            cut = true;
            break;
        }
        match tokio::time::timeout(tokio::time::Duration::from_millis(50), stream.next()).await {
            Ok(Some((url, result))) => {
                out.insert(url, result);
            }
            Ok(None) => break,
            Err(_) => {}
        }
    }
    if cut {
        tracing::warn!(
            "execute_scripts: module fetch deadline reached after {}/{} bodies arrived; \
             arrived modules still execute on a fresh budget",
            out.len(),
            total
        );
    }
    out
}
