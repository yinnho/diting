//! Stateless page acquire: fetch / click / eval. The helpers live one module
//! over, by job: [`super::challenge`] (challenge ride-out),
//! [`super::fetch_text`] (rendered-text read), [`super::fetch_documents`]
//! (PDF/DOCX bytes → text), [`super::fetch_facts`] (media / layout / resources
//! facts), [`super::fetch_splice`], [`super::route_fallback`] (route seams).

use super::browser::{build_browser, inject_cookies, run_on_local_runtime};
use super::challenge::{maybe_bypass_anubis, maybe_bypass_byte_waf, maybe_bypass_challenge};
use super::fetch_documents::{binary_document_text, embedded_documents_text};
use super::fetch_facts::{
    media_from_eval, parse_layout_facts, prefetch_resource_facts, MEDIA_PROBE,
};
use super::fetch_text::{rendered_text, wait_for_rendered_text};
use super::route_fallback::{
    fetch_nav_ceiling_ms, nav_deadline_hit_before_commit, navigation_uses_direct,
    note_direct_navigation_outcome, note_proxy_fallback_started, proxy_retry_pair, starve_bail_ms,
    wait_for_load_or_deadline,
};
use crate::{
    ClickRequest, ClickResponse, EvalRequest, EvalResponse, FetchRequest, FetchResponse,
    OutputFormat,
};
use anyhow::{Context, Result};

/// Geo-block stub probe. netflix-class sites answer a direct (CN-exit)
/// request with a bare server-rendered shell — netflix's is literally
/// "Not Available" at 13 chars — while the same URL through the proxy
/// carries the full page. The worker never sets `use_proxy`, so the fetch
/// path detects the stub by phrase and re-navigates through the tunnel
/// once. Phrase-matched (not just "short") so legitimately tiny pages
/// (example.com) and still-empty SPA shells never pay the retry.
const GEO_STUB_PROBE_JS: &str = r#"(function(){var t=(document.body?document.body.innerText:'').trim();
if(t.length>300)return false;t=t.toLowerCase();
var m=['not available','unavailable in your','unavailable in your country','blocked in your country','geo-restricted','geo restricted','access denied','currently unavailable'];
for(var i=0;i<m.length;i++){if(t.indexOf(m[i])>=0)return true;}
return false;})()"#;

/// True when the live page reads as a geo-block stub (see
/// [`GEO_STUB_PROBE_JS`]). Server-rendered stubs are readable before any
/// settle, so this costs one innerText walk on the committed document.
fn geo_block_stub(page: &mut crate::page::Page) -> bool {
    page.evaluate(GEO_STUB_PROBE_JS).as_bool().unwrap_or(false)
}

/// Meta-refresh stub probe (hao123 class): the bare domain 301s to a page
/// whose only payload is `<meta http-equiv="refresh" content="0.5;url=…">`,
/// and nothing in the page lifetime acts on it. Returns the resolved target
/// only for a true stub — collapsed body text under 400 chars and delay at
/// most 10s — so a real page carrying a stray refresh meta is never
/// hijacked. Empty string means "not a stub, stay".
const META_REFRESH_PROBE_JS: &str = r#"(function(){
var metas=document.querySelectorAll('meta');
for(var i=0;i<metas.length;i++){
  var eq=(metas[i].getAttribute('http-equiv')||'').toLowerCase();
  if(eq!=='refresh')continue;
  var m=/^\s*(\d+(?:\.\d+)?)\s*;\s*url\s*=\s*(.+?)\s*$/i.exec(metas[i].getAttribute('content')||'');
  if(!m||parseFloat(m[1])>10)continue;
  var t=(document.body?document.body.innerText:'').replace(/\s+/g,'');
  if(t.length>400)continue;
  try{return new URL(m[2].replace(/^['"]+|['"]+$/g,''),location.href).href;}catch(e){return '';}
}
return '';})()"#;

/// Same as fetch_url_text but injects search-session cookies before navigation.
/// Needed for sogou WeChat /link redirect URLs which require the sogou session
/// cookie to pass the antispider check.
pub(crate) fn fetch_url_text_with_cookies(
    url: String,
    use_proxy: bool,
    wait_secs: u64,
    max_chars: usize,
    cookies: &[String],
) -> Result<(String, bool, String)> {
    let cookies = cookies.to_vec(); // Clone so the closure owns the data.
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            let browser = build_browser(use_proxy, &url, None)?;
            if !cookies.is_empty() {
                inject_cookies(&browser, &cookies, &url);
            }
            let mut page = browser.new_page().await?;
            page.goto(&url).await?;

            // Auto-bypass Cloudflare Turnstile challenge if detected.
            maybe_bypass_challenge(&mut page).await?;

            // Ride out bytedance WAF JS challenges (juejin.cn class).
            maybe_bypass_byte_waf(&mut page).await?;

            if wait_secs > 0 {
                page.settle(wait_secs * 1000).await;
            }

            // Check if we landed on an antispider/CAPTCHA page.
            let final_url = page.url();
            tracing::info!("fetch_url_text: {} -> final_url={}", url, final_url);
            let is_antispider = final_url.contains("/antispider")
                || final_url.contains("wappass.baidu.com")
                || final_url.contains("sorry.google.com")
                || final_url.contains("challenge-platform");
            let mut content = rendered_text(&mut page, None, false);
            // Search bodies always get the stripper — there is no per-item
            // knob to turn it off, and a result list is exactly where a
            // poisoned page meets a model with no human in the loop.
            let hidden = crate::sanitize::parse_hidden_spans(
                &page.evaluate(crate::sanitize::HIDDEN_SPAN_PROBE),
            );
            content = crate::sanitize::sanitize_text(&content, &hidden).0;

            // If we landed on an antispider/CAPTCHA page, treat it as an error
            // rather than returning the CAPTCHA page content as search result body.
            if is_antispider {
                return Err(anyhow::anyhow!(
                    "CAPTCHA/antispider page detected at {}",
                    final_url
                ));
            }

            let (content, truncated) = if max_chars > 0 && content.chars().count() > max_chars {
                (content.chars().take(max_chars).collect::<String>(), true)
            } else {
                (content, false)
            };
            // The post-redirect effective URL rides along so the caller can
            // run the robots re-check (#175) on its own async runtime — the
            // robots cache lock belongs to the axum runtime, not this one.
            Ok((content, truncated, final_url))
        })
    })
}

/// Evaluate a JS expression on a page, retrying until it returns non-null
/// or the timeout expires. Used for extracting `window.__INITIAL_STATE__`
/// and similar JS globals from SPA pages.
pub(crate) fn extract_js_global(
    page: &mut crate::page::Page,
    expression: &str,
    timeout_ms: u64,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let mut interval = 200u64;
    loop {
        let js = format!(
            "(function() {{ try {{ var r = {}; return r == null ? null : (typeof r === 'object' ? JSON.stringify(r) : r); }} catch(e) {{ return null; }} }})()",
            expression
        );
        let val = page.evaluate(&js);
        if !val.is_null() {
            // If the value is a string containing JSON, parse it.
            if let Some(s) = val.as_str() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                    return parsed;
                }
                return serde_json::Value::String(s.to_string());
            }
            return val;
        }
        if std::time::Instant::now() >= deadline {
            return serde_json::Value::Null;
        }
        // Synchronous sleep — we're inside run_on_local_runtime.
        std::thread::sleep(std::time::Duration::from_millis(interval));
        interval = (interval * 2).min(2000);
        // Also pump the JS event loop.
        let _ = page.evaluate("1+1");
    }
}

/// Fetch a page and return content in the requested format.
pub fn do_fetch(req: FetchRequest) -> Result<FetchResponse> {
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            // Request clock: the content-stability tail below budgets itself
            // against what navigation already spent of the caller's deadline.
            let started = std::time::Instant::now();
            // Freeze the route decision before build_browser: an explicit
            // use_proxy=false may still be auto-proxied by a domain rule or
            // an existing route-table sentence.
            let proxy_configured = crate::config::proxy_from_env().is_some();
            let mut direct_leg = navigation_uses_direct(&req.url, req.use_proxy, proxy_configured);
            let mut browser =
                build_browser(req.use_proxy, &req.url, req.tls_fingerprint.as_deref())?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            // Starvation checkpoint window: only a true direct leg with a
            // proxy behind it. The nav ceiling itself stays at the full
            // default (or the operator's explicit DITING_NAV_TIMEOUT_MS) —
            // the checkpoint below is an observer, not a deadline.
            let starve_ms = if direct_leg && proxy_configured {
                starve_bail_ms(std::env::var("DITING_STARVE_BAIL_MS").ok().as_deref())
            } else {
                0
            };
            let nav_ceiling_ms = fetch_nav_ceiling_ms(
                std::env::var("DITING_NAV_TIMEOUT_MS").ok().as_deref(),
                &req.url,
            );
            page.inner.set_navigation_timeout(Some(nav_ceiling_ms));
            // Geometry facts are the caller's opt-in. The page stays Mock
            // until then, so scrollHeight is the synthetic grid.
            if req.layout {
                page.inner
                    .set_layout_policy(diting::diting_browser::LayoutPolicy::OnDemand);
            }
            // GitHub-class pages commit the document and then keep scripts
            // running past the navigation deadline. The live document is the
            // page; dropping it turns a finished read into a 500. A deadline
            // before commit is still a hard error.
            //
            // Starvation checkpoint (starve_ms > 0): poll the page's shared
            // NavProgress cell at the window WITHOUT consuming the
            // navigation future. The engine-side counters are the only
            // honest progress signal — a JS probe cannot see deliveries on
            // a live navigation (our resource timings are only pushed when
            // a whole fetch phase completes), and the old design that
            // SWAPPED the nav ceiling down to the bail dropped the whole
            // goto future at the deadline, killing every in-flight script
            // fetch with it, so a progressing trickle (run14 melon: 19/20
            // script bodies landed by 10s, the last at ~17s; microsoft
            // ~21.6s; shopify's stylesheet wave ~17.2s) could never finish
            // on the leg that was already carrying it. Zero progress —
            // the document never committed, or a discovered script wave
            // delivered not one body (petstore3's single 420KB bundle
            // trickling at ~11KB/s) — still jumps ship for the proxy while
            // the worker's retry budget is alive; anything else resumes
            // the SAME future under the full ceiling.
            let nav_progress = page.inner.nav_progress();
            let nav_result = if starve_ms > 0 {
                let mut goto_fut = Box::pin(page.goto(&req.url));
                let finished = tokio::select! {
                    r = goto_fut.as_mut() => Some(r),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(starve_ms)) => None,
                };
                match finished {
                    Some(r) => r,
                    None if !nav_progress.committed() || nav_progress.script_zero_progress() => {
                        let why = if !nav_progress.committed() {
                            "document not committed"
                        } else {
                            "script wave delivered no bodies"
                        };
                        tracing::info!(
                            url = %req.url,
                            "fetch: direct leg starving at the {starve_ms}ms checkpoint ({why}); retrying through the proxy"
                        );
                        note_direct_navigation_outcome(&req.url, direct_leg, false);
                        note_proxy_fallback_started(&req.url, |host| {
                            diting::route_table::note_proxy_pending(host);
                        });
                        // Drop the pinned future first: it holds the &mut
                        // borrow of `page`, and the direct pair must be
                        // fully gone before the proxied pair exists (V8
                        // isolates drop in reverse creation order).
                        drop(goto_fut);
                        drop(page);
                        drop(browser);
                        let (page2, browser2) = proxy_retry_pair(
                            &req,
                            &format!(
                                "direct navigation starved {starve_ms}ms at the checkpoint ({why})"
                            ),
                        )
                        .await?;
                        page = page2;
                        browser = browser2;
                        direct_leg = false;
                        Ok(())
                    }
                    None => {
                        tracing::info!(
                            url = %req.url,
                            "fetch: direct leg progressing at the {starve_ms}ms checkpoint; holding the navigation to the full ceiling"
                        );
                        goto_fut.await
                    }
                }
            } else {
                page.goto(&req.url).await
            };
            if let Err(e) = nav_result {
                let msg = e.to_string();
                if msg.contains("document committed") {
                    // Full-ceiling deadline after the document committed:
                    // the live document is the page (GitHub-class, a PoW
                    // challenge riding inline scripts, or a progressing
                    // trickle that outlived the ceiling) — read it.
                    tracing::warn!(
                        url = %req.url,
                        "fetch: navigation deadline after the document committed; reading the live page"
                    );
                    note_direct_navigation_outcome(&req.url, direct_leg, true);
                    // Hand the SAME ceiling to the wait pump before
                    // reading: a cancelled pipeline reports readyState
                    // "complete" long before its subresources land, so
                    // the pump also waits out pending scripts and
                    // stylesheets (run14 shopify's stylesheet wave) — but
                    // never past the ceiling that already cut goto (the
                    // pump deadline and the page timeout are one number,
                    // see fetch_nav_ceiling_ms).
                    if starve_ms > 0 {
                        wait_for_load_or_deadline(
                            &mut page,
                            started + std::time::Duration::from_millis(nav_ceiling_ms),
                        );
                    }
                } else if starve_ms > 0 && direct_leg && nav_deadline_hit_before_commit(&msg) {
                    // The starvation bail's pre-commit face, after the fact:
                    // the deadline fired before the document committed (slow
                    // TTFB, first bytes trickling). The checkpoint above
                    // usually catches this face at `starve_ms` with the
                    // engine-side progress cell; this arm remains for the
                    // one window it cannot cover — an operator ceiling
                    // (`DITING_NAV_TIMEOUT_MS`) shorter than the checkpoint.
                    // `about:blank` gives nothing to read and the transport
                    // arm below does not match a deadline error, so without
                    // this arm the fetch is a bare 500. Same cure: jump for
                    // the proxy while the worker's retry budget is alive.
                    tracing::info!(
                        url = %req.url,
                        "fetch: direct leg starved before the document committed; retrying through the proxy"
                    );
                    note_direct_navigation_outcome(&req.url, direct_leg, false);
                    note_proxy_fallback_started(&req.url, |host| {
                        diting::route_table::note_proxy_pending(host);
                    });
                    drop(page);
                    drop(browser);
                    let (page2, browser2) = proxy_retry_pair(
                        &req,
                        &format!(
                            "direct navigation starved {starve_ms}ms before the document committed"
                        ),
                    )
                    .await?;
                    page = page2;
                    browser = browser2;
                    direct_leg = false;
                    // The live proxy pair replaces the dropped direct pair:
                    // the geo-stub retry below drops `browser` again, so the
                    // binding stays owned across this fallback, like the two
                    // arms above.
                } else {
                    // Transport-class death (connect/timeout/refused) on the
                    // direct route: the wall is a property of the box, not of
                    // the request — note it in the route table, then retry the
                    // navigation through the tunnel once when a proxy exists
                    // and this leg wasn't already proxied. A proxy leg that
                    // carries the page sentences the host (note_proxy_win), so
                    // the next fetch goes straight to the proxy. The geo-stub
                    // retry below is the content-level sibling of this block.
                    note_direct_navigation_outcome(&req.url, direct_leg, false);
                    let transport = msg.contains("Connect")
                        || msg.contains("TimedOut")
                        || msg.contains("timed out")
                        || msg.contains("Connection refused")
                        || msg.contains("error sending request");
                    if !transport || !direct_leg || !proxy_configured {
                        return Err(e.into());
                    }
                    tracing::info!(url = %req.url, "fetch: direct navigation died at transport; retrying through the proxy");
                    note_proxy_fallback_started(&req.url, |host| {
                        diting::route_table::note_proxy_pending(host);
                    });
                    drop(page);
                    drop(browser);
                    let cause = format!("direct navigation failed ({msg})");
                    let (page2, browser2) = proxy_retry_pair(&req, &cause).await?;
                    page = page2;
                    browser = browser2;
                    direct_leg = false;
                    // The live proxy pair replaces the dropped direct pair:
                    // the geo-stub retry below drops `browser` again, so the
                    // binding must stay owned across this fallback.
                }
            } else {
                note_direct_navigation_outcome(&req.url, direct_leg, true);
            }

            // Non-text documents (PDF/DOCX/…) must not reach the renderer:
            // the HTML parser accepts the binary and every text face then
            // carries megabytes of soup — downstream markdown consumers
            // choke on it (the LexBench scorer's HTML tokenizer spins for
            // CPU-hours on a 2.7MB single-line PDF body). PDF/DOCX asked
            // back as text go through the salvage extractor below; anything
            // else is refused — bytes belong to /download. The allow list
            // mirrors navigate.rs `renders_as_text_document` plus the
            // html/xhtml family; an unknown (empty) type keeps the old
            // lenient path.
            let content_type = page.inner.content_type();
            if !content_type.is_empty()
                && !content_type.starts_with("text/")
                && content_type != "text/html"
                && content_type != "application/xhtml+xml"
                && content_type != "application/json"
                && !content_type.ends_with("+json")
                && content_type != "application/xml"
                && !content_type.ends_with("+xml")
                && content_type != "application/javascript"
                && content_type != "text/javascript"
            {
                // One exception: text-back requests for binary documents get
                // the salvage extractor (pdf_text.rs) on bytes re-fetched
                // over plain HTTP — same origin, same proxy decision as the
                // navigation that discovered the type. Routed by content
                // type AND magic bytes: raw.githubusercontent serves PDFs as
                // application/octet-stream, so the sniff, not the mime, has
                // the final word. Anything unextractable stays refused:
                // bytes belong to /download.
                if matches!(req.format, OutputFormat::Markdown | OutputFormat::Text)
                    && (content_type.contains("pdf")
                        || content_type.contains("wordprocessingml")
                        || content_type == "application/octet-stream")
                {
                    if let Some(text) = binary_document_text(&req.url, req.use_proxy).await {
                        tracing::info!(
                            url = %req.url,
                            chars = text.len(),
                            "fetch: pdf text extracted via salvage parser"
                        );
                        return Ok(FetchResponse {
                            url: page.url(),
                            title: None,
                            content: text,
                            truncated: false,
                            captcha_event: None,
                            js_extract_result: None,
                            tier: Some("browser"),
                            redirected_from: Vec::new(),
                            content_hash: None,
                            changed_since_prev: None,
                            sanitize_report: None,
                            xhr: Vec::new(),
                            media: Vec::new(),
                            layout: None,
                            resources: Vec::new(),
                        });
                    }
                }
                return Err(anyhow::anyhow!(
                    "binary document ({content_type}): {} is not text-extractable; use /download for the bytes",
                    req.url
                ));
            }

            // Auto-bypass Cloudflare Turnstile challenge if detected.
            if req.auto_bypass_challenge {
                maybe_bypass_challenge(&mut page).await?;
            }

            // Ride out bytedance WAF JS challenges (juejin.cn class) —
            // unconditional, like the Cloudflare path in search fetches.
            maybe_bypass_byte_waf(&mut page).await?;

            // Ride out Anubis PoW interstitials (winehq class) — pump the
            // workers, drain the pass-challenge navigation. Same
            // acquisition-path stance as the two above.
            maybe_bypass_anubis(&mut page).await?;

            // Geo-block stub (netflix class): the direct route landed a
            // bare phrase-matched shell, the proxy route carries the real
            // page. The direct browser and its page are dropped FIRST —
            // V8 isolates must be dropped in reverse creation order, so a
            // second browser may not be built while the first one's page
            // is still alive (v8::OwnedIsolate panics otherwise). The
            // stub is worthless anyway; if the proxy leg fails the error
            // says what happened.
            if direct_leg && proxy_configured && geo_block_stub(&mut page) {
                tracing::info!(
                    url = %req.url,
                    "fetch: geo-block stub on the direct route; retrying through the proxy"
                );
                note_proxy_fallback_started(&req.url, |host| {
                    diting::route_table::note_proxy_pending(host);
                });
                drop(page);
                drop(browser);
                let (page2, _browser2) = proxy_retry_pair(&req, "geo-block stub").await?;
                page = page2;
                // _browser2 may drop here: Page owns its Arc<BrowserContext>,
                // the page outlives the factory that built it.
            }

            // Meta-refresh stub (hao123 class): a bare redirect page whose
            // only payload is the refresh directive. Follow it explicitly —
            // same route, at most two hops, self-loop guarded. A failed hop
            // keeps the stub: it scores nothing either way, and an error
            // would only hide what actually happened.
            for _ in 0..2 {
                let target = page.evaluate(META_REFRESH_PROBE_JS);
                let target = target.as_str().unwrap_or("").trim().to_string();
                if target.is_empty() || target == req.url {
                    break;
                }
                tracing::info!(url = %req.url, %target, "fetch: following meta-refresh stub");
                if let Err(e) = page.goto(&target).await {
                    if !e.to_string().contains("document committed") {
                        tracing::warn!(%target, "fetch: meta-refresh hop failed: {e}");
                        break;
                    }
                }
            }

            if let Some(wait) = req.wait_secs {
                // Floor, not ceiling: late hydration work (chunk fetches
                // through a slow tunnel) keeps mutating the page after a
                // by-duration settle goes idle — wait until content stops
                // changing (see Page::settle_content_stable). But the tail
                // is budget-aware: the harness worker kills a /fetch at 27s,
                // and pages whose navigation alone ran 23-26s (kubernetes.io
                // family through the CN tunnel) had the tail's fresh 9s
                // pushed them over — three run3 passes flipped to timeouts.
                // Spend only what's left of a 23s overall budget, leaving
                // extraction+transfer room; 0 left means the floor alone
                // decides (an explicit wait_secs=30 still gets its 30s —
                // the caller's floor is never cut, only our extra tail).
                const FETCH_TAIL_DEADLINE_MS: u64 = 23_000;
                let spent = started.elapsed().as_millis() as u64;
                let tail_budget = FETCH_TAIL_DEADLINE_MS.saturating_sub(spent);
                page.settle_content_stable(wait * 1000, tail_budget).await;
            }
            // #66: text formats only — the HTML format returns the document
            // itself, never empty. The string is kept: the content match
            // below must not walk innerText a second time.
            let rendered = if matches!(req.format, OutputFormat::Html) {
                String::new()
            } else {
                // Markdown carries destinations inline and tables as in-place
                // pipe tables (MARKDOWN_SPLICE_JS in rendered_text) —
                // innerText alone scores as neither.
                let text = wait_for_rendered_text(
                    &mut page,
                    req.selector.as_deref(),
                    req.format == OutputFormat::Markdown,
                )
                .await;
                if text.trim().is_empty() {
                    tracing::warn!(
                        url = %req.url,
                        html_bytes = page.content().len(),
                        "browser tier: page still has no text after grace"
                    );
                }
                text
            };

            // Cost knobs (#279) — the explicit opt-in for expensive work,
            // logged so the bill is observable. Layout first: the forced
            // pass warms geometry before js_extract runs below, so a probe
            // reading gBCR doesn't spend its timeout window on first-build.
            let layout = if req.layout {
                let t0 = std::time::Instant::now();
                let facts = parse_layout_facts(&page.evaluate(
                    "JSON.stringify({h:document.documentElement.scrollHeight,w:document.documentElement.scrollWidth})",
                ));
                match &facts {
                    Some(f) => tracing::info!(
                        "fetch layout forced: {}x{} in {:?}",
                        f.scroll_width,
                        f.scroll_height,
                        t0.elapsed()
                    ),
                    None => tracing::warn!(
                        "fetch layout forced: probe returned no geometry in {:?}",
                        t0.elapsed()
                    ),
                }
                facts
            } else {
                None
            };

            // Title: prefer a visible article-title element (WeChat's
            // #activity-name), then document.title, then og:title meta.
            let title = page
                .evaluate(
                    "((document.querySelector('#activity-name,h1,.article-title')||{}).textContent||'').trim()\
                     || document.title\
                     || (document.querySelector('meta[property=\"og:title\"]')||{}).content\
                     || ''",
                )
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            // Media manifest (#232): one probe over the rendered DOM —
            // img currentSrc is the post-lazyload live value, the data-*
            // attributes cover images that never fired, srcset keeps only
            // its first candidate, video/audio carry sources + poster.
            // Research loops get body + asset leads in a single /fetch.
            let media = media_from_eval(page.evaluate(MEDIA_PROBE));

            // The `resources:true` pass (#279): fetch the subresource bodies
            // through the same bounded machinery the screenshot path uses,
            // then report size + decoded dimensions per fetched asset. The
            // bodies themselves stay out of the response — `media` carries
            // the leads, this says which leads are real and how big.
            let resources = if req.resources {
                prefetch_resource_facts(&mut page).await
            } else {
                Vec::new()
            };

            // Source the content from the RENDERED DOM, not the initial HTML
            // snapshot. On heavy SPA pages (WeChat: 6.6MB shell) the article
            // body is filled in by JS and sits deep in document.documentElement
            // .outerHTML — converting the whole shell to markdown then
            // truncating to max_chars would cut the body off entirely.
            // body.innerText (after settle/wait) is the already-rendered text.
            // Content extraction, with the injection stripper on the
            // text/markdown path (raw HTML is never sanitized — the caller
            // asked for the bytes as they are).
            let (content, sanitize_report) = match req.format {
                OutputFormat::Html => (page.content(), None),
                OutputFormat::Text | OutputFormat::Markdown => {
                    let raw = rendered;
                    if !req.sanitize {
                        (raw, None)
                    } else {
                        // Probe while the page is still alive: text that a
                        // human can't see (opacity 0 / sub-4px font) but
                        // innerText happily carries.
                        let hidden = crate::sanitize::parse_hidden_spans(
                            &page.evaluate(crate::sanitize::HIDDEN_SPAN_PROBE),
                        );
                        let (clean, report) = crate::sanitize::sanitize_text(&raw, &hidden);
                        let report = if report.is_clean() {
                            None
                        } else {
                            Some(report)
                        };
                        (clean, report)
                    }
                }
            };

            // Embedded-document rescue (pdf.js viewers, PDFObject-style
            // embeds): the PDF bytes never ride the navigation response and
            // never land in the DOM's text, so both the content-type gate
            // above and innerText miss them. Probe the live page for
            // document URLs, byte-fetch them, splice the extracted text in
            // BEFORE max_chars truncation so the appended body rides the
            // same knob as the page's own text. Recall-only upside: the
            // scorer gates on keyword recall, not precision.
            let content = if matches!(req.format, OutputFormat::Text | OutputFormat::Markdown) {
                match embedded_documents_text(&req, &mut page, started).await {
                    Some(extra) => format!("{content}\n\n{extra}"),
                    None => content,
                }
            } else {
                content
            };

            // Truncate to max_chars (0 = unlimited). Keeps huge pages from
            // blowing up a downstream LLM context window.
            let (content, truncated) =
                if req.max_chars > 0 && content.chars().count() > req.max_chars {
                    let cut: String = content.chars().take(req.max_chars).collect();
                    (cut, true)
                } else {
                    (content, false)
                };

            // JS extraction: evaluate the user-specified expression after page
            // has settled and content is extracted.
            let js_extract_result = req
                .js_extract
                .as_ref()
                .map(|cfg| extract_js_global(&mut page, &cfg.expression, cfg.timeout_ms));

            // CAPTCHA detection and optional auto-solve.
            let captcha_event = {
                let final_url = page.url();
                // A challenge interstitial is a short document. Serializing
                // outerHTML of a page that already yielded a long body
                // (GitHub, an RFC) only to scan for a widget marker holds
                // the response past the client's read timeout. URL checks
                // still run against an empty body.
                let html_snapshot = if content.len() > 8_000 {
                    String::new()
                } else {
                    page.content()
                };
                crate::captcha::detect_and_maybe_solve(&final_url, &html_snapshot).await
            };

            // Background API capture (capture_xhr): the page's own XHR/fetch
            // responses — usually the clean structured face, an order of
            // magnitude cheaper to read than the rendered DOM. Per-body cap
            // rides `max_chars` (the LLM-context knob), bounded so 20 bodies
            // can't each claim the full default.
            let xhr = if let Some(filters) = req.capture_xhr.as_ref() {
                let record = page.inner.navigation_record();
                let body_cap = req.max_chars.min(8_000);
                // #97: the capture_xhr surface exists to read what the page's
                // JS put on its API calls — the signed/custom outbound headers
                // (x-s class) are half of that story, so rows carry them.
                crate::har::xhr_bodies(&record.events, filters, body_cap, true, &|rid| {
                    page.inner.get_response_body(rid)
                })
            } else {
                Vec::new()
            };

            Ok(FetchResponse {
                url: page.url(),
                title,
                content,
                truncated,
                captcha_event,
                js_extract_result,
                tier: Some("browser"),
                redirected_from: Vec::new(),
                content_hash: None,
                changed_since_prev: None,
                sanitize_report,
                xhr,
                media,
                layout,
                resources,
            })
        })
    })
}

/// Click an element by CSS selector using JS `element.click()`.
pub fn do_click(req: ClickRequest) -> Result<ClickResponse> {
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            crate::rate::check_domain(&req.url).map_err(anyhow::Error::msg)?;
            let browser = build_browser(req.use_proxy, &req.url, req.tls_fingerprint.as_deref())?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            page.goto(&req.url).await?;

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
            }

            let clicked = if let Some(element) = page.query_selector(&req.selector) {
                page.click_element(element)
                    .context("element.click() failed")?;
                true
            } else {
                false
            };

            page.settle(500).await;
            // A submit click routes through the bootstrap form glue and
            // lands as a pending JS navigation; draining it fires the POST
            // and moves the page to the action URL (obscura #618 class —
            // the CDP layer drains after clicks, this is the stateless
            // surface's equivalent).
            if page.process_pending_navigation().await.unwrap_or(false) {
                // The landed document needs its own event-loop slice before
                // url/innerText are read.
                page.settle(800).await;
            }
            let text_after = page
                .evaluate("document.body.innerText")
                .as_str()
                .map(|s| s.to_string());

            Ok(ClickResponse {
                url: page.url(),
                selector: req.selector,
                clicked,
                text_after,
            })
        })
    })
}

/// Evaluate arbitrary JavaScript on the page.
pub fn do_eval(req: EvalRequest) -> Result<EvalResponse> {
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            crate::rate::check_domain(&req.url).map_err(anyhow::Error::msg)?;
            let browser = build_browser(req.use_proxy, &req.url, req.tls_fingerprint.as_deref())?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            page.goto(&req.url).await?;

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
            }

            let result = page.evaluate_async(&req.script).await;
            // A location.href-style navigation in the script lands as a
            // pending JS navigation — drain so the reported url follows it
            // (mirrors the CDP Runtime.evaluate path, which drains post-eval).
            if page.process_pending_navigation().await.unwrap_or(false) {
                page.settle(500).await;
            }

            Ok(EvalResponse {
                url: page.url(),
                result,
            })
        })
    })
}
