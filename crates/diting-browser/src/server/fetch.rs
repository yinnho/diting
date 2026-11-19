//! Stateless page acquire: fetch / click / eval + challenge ride-out.
//!
//! Challenge heuristics here are acquisition-path *shape* detection (Cloudflare
//! Turnstile, ByteDance WAF PoW) — not platform business. Site policy stays in
//! `workflow/`.

use super::browser::{build_browser, inject_cookies, run_on_local_runtime};
use crate::{
    ClickRequest, ClickResponse, EvalRequest, EvalResponse, FetchRequest, FetchResponse,
    OutputFormat,
};
use anyhow::{Context, Result};

/// Check if the current page is a Cloudflare challenge.
fn is_cloudflare_challenge(page: &mut crate::page::Page) -> bool {
    let title_val = page.evaluate("document.title");
    let title = title_val.as_str().unwrap_or("");
    if title.contains("Just a moment") || title.contains("Attention Required") {
        return true;
    }
    let has_turnstile_val =
        page.evaluate(r#"!!document.querySelector('iframe[src*="challenges.cloudflare.com"]')"#);
    has_turnstile_val.as_bool().unwrap_or(false)
}

/// After goto(), detect and auto-bypass Cloudflare Turnstile challenges.
/// Waits for `cf_clearance` cookie, then re-navigates if the page hasn't
/// auto-redirected.
pub(crate) async fn maybe_bypass_challenge(page: &mut crate::page::Page) -> Result<()> {
    if !is_cloudflare_challenge(page) {
        return Ok(());
    }
    let url = page.url();
    tracing::info!(
        "Cloudflare challenge detected at {}, auto-bypassing...",
        url
    );

    // Give Turnstile JS time to execute (managed challenge auto-completes).
    page.settle(5000).await;

    // Wait for cf_clearance cookie (the signal that Turnstile passed).
    match page
        .wait_for_cookie("cf_clearance", std::time::Duration::from_secs(25))
        .await
    {
        Ok(()) => {
            tracing::info!("cf_clearance cookie received, challenge passed");
            // If the page didn't auto-redirect, re-navigate.
            if is_cloudflare_challenge(page) {
                tracing::info!("Re-navigating to {} after challenge pass", url);
                page.goto(&url).await?;
                page.settle(3000).await;
            }
        }
        Err(e) => {
            tracing::warn!("cf_clearance timeout: {}", e);
            // Don't fail hard — the page might still have usable content
            // (e.g. invisible challenge that completed without cookie).
        }
    }
    Ok(())
}

/// Check if the current page is a bytedance WAF JS challenge (juejin.cn
/// class): a tiny stub whose `<body onload="readygo()">` brute-forces a
/// SHA256 PoW over 1ms ticks and reloads with a `_wafchallengeid` answer
/// cookie. Signature: a global `readygo` function + almost no visible text
/// (real pages don't define that global; challenge stubs carry no content).
///
/// **Gray / deploy-facing (not platform business):** this is acquisition-path
/// challenge *shape* detection for `/fetch` (and kin) so a PoW interstitial
/// does not look like empty content. It is not a seller/login flow and must
/// not grow into site-specific publish logic — leave WAF ride-out here;
/// site policy stays in `workflow/`. See architecture note: core = generic
/// engine, platform compat → flow.
fn is_byte_waf_challenge(page: &mut crate::page::Page) -> bool {
    let val = page.evaluate(
        "(function() {\
            try {\
                if (!document.body) return false;\
                var text = (document.body.innerText || '').replace(/\\s+/g, '');\
                return text.length < 100 && typeof window.readygo === 'function';\
            } catch (e) { return false; }\
        })()",
    );
    val.as_bool().unwrap_or(false)
}

/// After goto(), ride out the bytedance WAF JS challenge. The challenge
/// solves itself in-page — readygo() sets the answer cookie, then calls
/// location.reload() — but the reload is recorded as a pending JS
/// navigation that nobody in the stateless fetch path drains (sessions
/// have a command-loop pump; do_fetch doesn't), and the PoW ticks need
/// event-loop pumping that the caller's default 0s settle never gives.
/// So: pump, drain the reload, settle the real page.
pub(crate) async fn maybe_bypass_byte_waf(page: &mut crate::page::Page) -> Result<()> {
    if !is_byte_waf_challenge(page) {
        return Ok(());
    }
    let url = page.url();
    tracing::info!(
        "byte-WAF challenge detected at {}, waiting for PoW + reload",
        url
    );

    // Pump so readygo's 1ms interval ticks (the PoW is trivially small —
    // observed answers are single/low double digits) and the reload lands
    // in pending_navigation.
    page.settle(2000).await;

    // Drain the JS-initiated reload; the re-navigation runs load events
    // again, this time with the answer cookie attached by the client.
    match page.process_pending_navigation().await {
        Ok(true) => {
            page.settle(1500).await;
        }
        Ok(false) => {
            // No reload recorded — either it fired inside settle's own
            // navigation handling or the solver failed. Re-check below.
        }
        Err(e) => {
            tracing::warn!("byte-WAF reload navigation failed: {}", e);
        }
    }

    if is_byte_waf_challenge(page) {
        tracing::warn!("byte-WAF challenge at {} still present after settle", url);
    }
    Ok(())
}

/// Read the rendered text content from the live DOM (after JS has run).
/// When `selector` is given, return that element's innerText; otherwise the
/// whole body. This reflects JS-filled content (WeChat/SPA), unlike parsing
/// the initial HTML snapshot.
/// #66: how long the browser tier keeps pumping a page whose text is still
/// empty. crates.io (an Ember SPA) returns from navigation with a bare shell
/// — body.innerText "" — and renders its ~5k chars a few seconds later; the
/// extraction ran in between and answered 200 with an empty body, which an
/// agent can only treat as a failure. A long body returns at once. A short
/// shell keeps pumping only while a timer or fetch can still land inside
/// the next few seconds (quotes.toscrape.com `?delay=3000` paints after
/// load). A finished shell, or a page whose next timer is further out,
/// pays nothing.
const EMPTY_TEXT_GRACE_MS: u64 = 6_000;
/// Bytes. ASCII pages under this are "short"; a longer body is done.
const SHORT_TEXT_BYTES: usize = 1_200;
/// A title-only shell. Doc sites (EUI, PrimeNG) used to be read after the
/// app painted; giving up on the first empty sample is what made them
/// regress. Anything longer than a title is a finished short page.
const SHELL_BYTES: usize = 200;
/// `settle_quiet` window. A 3s delayed paint is still pending; a 5s
/// heartbeat is not, so the grace does not idle out on analytics timers.
const SOON_WORK_MS: u64 = 3_500;

fn page_has_soon_work(page: &mut crate::page::Page) -> bool {
    // Missing probe reads as "nothing pending" so a realm without the
    // helper does not sit out the whole grace.
    let expr = format!(
        "globalThis.__diting_settle_quiet&&globalThis.__diting_settle_quiet({SOON_WORK_MS})"
    );
    match page.evaluate(&expr).as_bool() {
        Some(quiet) => !quiet,
        None => false,
    }
}

async fn wait_for_rendered_text(page: &mut crate::page::Page, selector: Option<&str>) -> String {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(EMPTY_TEXT_GRACE_MS);
    loop {
        // Keep this string. A second innerText walk of the same document
        // (GitHub, WHATWG) is what pushes the response past the client
        // read timeout after the navigation deadline already fired.
        let text = rendered_text(page, selector);
        if text.len() >= SHORT_TEXT_BYTES || std::time::Instant::now() >= deadline {
            return text;
        }
        // Title-only shells keep pumping for the whole grace. A probe that
        // fails on a wedged realm must not look like "nothing is coming".
        if text.len() >= SHELL_BYTES && !page_has_soon_work(page) {
            return text;
        }
        page.pump_event_loop_slice(250).await;
    }
}

fn rendered_text(page: &mut crate::page::Page, selector: Option<&str>) -> String {
    // innerText now carries rendered-text semantics itself (script/style/
    // display:none excluded, block boundaries as newlines), so no DOM
    // mutation is needed here — the old workaround blanked script text by
    // destructively rewriting textContent before reading.
    let js = match selector {
        Some(sel) => {
            let escaped = sel
                .replace('\\', "\\\\")
                .replace('`', "\\`")
                .replace('$', "\\$");
            format!("(function(){{var el=document.querySelector(`{escaped}`);return el?el.innerText:'';}})()")
        }
        None => {
            // WeChat articles ship the full body server-rendered inside
            // #js_content but keep it visibility:hidden until their module JS
            // reveals it — body.innerText is then just the title/byline shell
            // (v0.3.1 report P1-3: 133 chars where the article is 981). Per
            // spec a not-rendered element's own innerText is its textContent,
            // so the container read still carries the article. GitHub does
            // the same with `.markdown-body`: the README is in the DOM, an
            // ancestor is still hidden, and body.innerText is only the nav.
            // Append any such container the body text does not already
            // contain, and put document.title on the front when the heading
            // never made it into the rendered text (example.com's h1).
            "(function(){var b=document.body?document.body.innerText:'';\
             function addFrames(doc,depth){if(!doc||depth>3)return;\
             var frames=doc.querySelectorAll('iframe');var n=frames.length<8?frames.length:8;\
             for(var i=0;i<n;i++){var child=null;try{child=frames[i].contentDocument;}catch(e){}\
             if(!child||!child.body)continue;var a=child.body.innerText||'';\
             var probe=a.trim().slice(0,80);if(probe&&b.indexOf(probe)<0)b+='\\n'+a;\
             addFrames(child,depth+1);}}\
             addFrames(document,0);\
             var links=document.querySelectorAll('nav a, aside a');\
             var lim=links.length<400?links.length:400;var extra=[];\
             for(var k=0;k<lim;k++){var t=(links[k].textContent||'').replace(/[ \\t\\n\\r\\f]+/g,' ').trim();\
             if(t.length>1&&t.length<80&&b.indexOf(t)<0)extra.push(t);}\
             if(extra.length)b+='\\n'+extra.join('\\n');\
             var feet=document.querySelectorAll('footer');\
             var fm=feet.length<3?feet.length:3;\
             for(var f=0;f<fm;f++){var ft=(feet[f].textContent||'').replace(/[ \\t\\n\\r\\f]+/g,' ').trim();\
             if(!ft)continue;var head=ft.slice(0,60);var tail=ft.slice(Math.max(0,ft.length-60));\
             if(b.indexOf(head)<0||(tail!==head&&b.indexOf(tail)<0))b+='\\n'+ft;}\
             var nodes=document.querySelectorAll('article.markdown-body,.markdown-body,#readme,#js_content,article,main,.theme-doc-markdown');\
             if(nodes.length>8)nodes=Array.prototype.slice.call(nodes,0,8);\
             for(var i=0;i<nodes.length;i++){var a=nodes[i].innerText||'';var probe=a.trim().slice(0,80);\
             if(probe&&b.indexOf(probe)<0)b+='\\n'+a;}\
             var title=(document.title||'').trim();\
             if(title&&b.indexOf(title)<0)b=title+'\\n'+b;\
             return b;})()"
                .to_string()
        }
    };
    let raw = page.evaluate(&js).as_str().unwrap_or("").to_string();
    // Collapse runs of whitespace (heavy SPA pages produce lots of blank
    // lines from empty layout containers) — keeps the output tight.
    collapse_whitespace(&raw)
}

/// Drop blank lines, turn a no-break space into a normal space, and write
/// table-cell tabs as markdown pipes.
///
/// CSS 2.1's `<title>` is `CSS&nbsp;2.1`. The decoded heading contains
/// U+00A0, so a reader looking for `CSS 2.1` misses the title that is
/// already on the page. Rendered tables separate cells with a tab plus
/// the space the walk leaves between cells (`Company\t Contact\t
/// Country`); the readable row is `Company | Contact | Country`.
fn collapse_whitespace(s: &str) -> String {
    s.lines()
        .map(|l| normalize_fetch_line(l.trim()))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_fetch_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{00a0}' | '\u{202f}' | '\u{2007}' | '\u{feff}' => out.push(' '),
            '\t' => {
                if matches!(chars.peek(), Some(' ' | '\u{00a0}')) {
                    chars.next();
                }
                out.push_str(" | ");
            }
            other => out.push(other),
        }
    }
    out
}

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
            let mut content = rendered_text(&mut page, None);
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
            let browser = build_browser(req.use_proxy, &req.url, req.tls_fingerprint.as_deref())?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
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
            if let Err(e) = page.goto(&req.url).await {
                if !e.to_string().contains("document committed") {
                    return Err(e.into());
                }
                tracing::warn!(
                    url = %req.url,
                    "fetch: navigation deadline after the document committed; reading the live page"
                );
            }

            // Auto-bypass Cloudflare Turnstile challenge if detected.
            if req.auto_bypass_challenge {
                maybe_bypass_challenge(&mut page).await?;
            }

            // Ride out bytedance WAF JS challenges (juejin.cn class) —
            // unconditional, like the Cloudflare path in search fetches.
            maybe_bypass_byte_waf(&mut page).await?;

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
            }
            // #66: text formats only — the HTML format returns the document
            // itself, never empty. The string is kept: the content match
            // below must not walk innerText a second time.
            let rendered = if matches!(req.format, OutputFormat::Html) {
                String::new()
            } else {
                let text = wait_for_rendered_text(&mut page, req.selector.as_deref()).await;
                if text.trim().is_empty() {
                    tracing::warn!(
                        url = %req.url,
                        html_bytes = page.content().len(),
                        "browser tier: page still has no text after {}ms of grace",
                        EMPTY_TEXT_GRACE_MS
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

/// Media-manifest probe (#232). Runs after the page settles, so img
/// `currentSrc` is the post-lazyload live value. Mirrors the attribute set
/// of render.rs::collect_media (the Tier-1 static twin) — keep the two in
/// sync when adding a source.
const MEDIA_PROBE: &str = r#"(function(){
var out=[],seen={};
function add(u){if(!u)return;u=String(u).trim();if(!u||u.indexOf('data:')===0)return;if(seen[u])return;seen[u]=1;out.push(u)}
function plain(el,names){for(var i=0;i<names.length;i++){var v=el.getAttribute(names[i]);if(v)add(v)}}
function first(el,names){for(var i=0;i<names.length;i++){var v=el.getAttribute(names[i]);if(v){add(v.split(',')[0].trim().split(/\s+/)[0])}}}
var imgs=document.querySelectorAll('img');
for(var i=0;i<imgs.length;i++){var im=imgs[i];add(im.currentSrc);plain(im,['src','data-src','data-original','data-lazy-src']);first(im,['srcset','data-srcset'])}
var ms=document.querySelectorAll('meta[property="og:image"],meta[name="og:image"]');
for(var i=0;i<ms.length;i++){add(ms[i].getAttribute('content'))}
var vs=document.querySelectorAll('video,audio,video source,audio source');
for(var i=0;i<vs.length;i++){var v=vs[i];add(v.src);plain(v,['data-src','poster'])}
return JSON.stringify(out)})()"#;

/// Parse the MEDIA_PROBE result (a JSON string of URLs) into the response's
/// `media` list. Any probe hiccup yields an empty list — media is a lead
/// surface, never worth failing the fetch over.
pub(crate) fn media_from_eval(v: serde_json::Value) -> Vec<String> {
    let raw = match v.as_str() {
        Some(s) => s,
        None => return Vec::new(),
    };
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(urls) => crate::render::finalize_media(urls),
        Err(_) => Vec::new(),
    }
}

/// Parse the `layout:true` geometry probe: the eval returns a JSON string
/// `{"h":<scrollHeight>,"w":<scrollWidth>}`. Any hiccup (null, non-JSON,
/// NaN-ish values) is `None` — the knob reports what layout measured, not
/// an error page's zeros.
pub(crate) fn parse_layout_facts(v: &serde_json::Value) -> Option<crate::LayoutFacts> {
    let raw = v.as_str()?;
    let obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(raw).ok()?;
    Some(crate::LayoutFacts {
        scroll_height: obj.get("h")?.as_i64()?,
        scroll_width: obj.get("w")?.as_i64()?,
    })
}

/// Turn the prefetched bodies into `resources` response rows: size always,
/// pixel dimensions when the bytes decode (magic sniff, PNG/JPEG/WebP/GIF).
/// Bodies stay out of the response — the caller has the URLs in `media`.
/// The `resources:true` body pass, feature-split: with `screenshot` it
/// prefetches through the bounded machinery the screenshot path uses
/// (#279); without, the knob answers an empty table (the capability
/// rides the feature, like /screenshot itself).
async fn prefetch_resource_facts(
    // Only the `screenshot` build reads the page; the bare build answers
    // an empty table without touching it.
    #[cfg_attr(not(feature = "screenshot"), allow(unused_variables))] page: &mut crate::page::Page,
) -> Vec<crate::ResourceFact> {
    #[cfg(feature = "screenshot")]
    {
        let t0 = std::time::Instant::now();
        let html = page.content();
        let url = page.url();
        let fetched = crate::screenshot::prefetch_render_resources(page, &url, &html, 1280.0).await;
        let facts = resource_facts(&fetched);
        tracing::info!(
            "fetch resources forced: {} bodies, {} with dimensions, in {:?}",
            facts.len(),
            facts.iter().filter(|f| f.width.is_some()).count(),
            t0.elapsed()
        );
        facts
    }
    #[cfg(not(feature = "screenshot"))]
    {
        Vec::new()
    }
}

#[cfg(feature = "screenshot")]
pub(crate) fn resource_facts(
    fetched: &crate::screenshot::PrefetchedResources,
) -> Vec<crate::ResourceFact> {
    let mut facts: Vec<crate::ResourceFact> = fetched
        .iter()
        .map(|(url, body)| {
            let decoded = diting::diting_layout::image::decode_bytes(body);
            crate::ResourceFact {
                url: url.clone(),
                bytes: body.len(),
                width: decoded.as_ref().map(|d| d.width),
                height: decoded.as_ref().map(|d| d.height),
            }
        })
        .collect();
    // Stable order across runs — HashMap iteration is not.
    facts.sort_by(|a, b| a.url.cmp(&b.url));
    facts
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

            let clicked = if let Some(el) = page.query_selector(&req.selector) {
                el.click().context("element.click() failed")?;
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

#[cfg(test)]
mod tests {
    use super::collapse_whitespace;

    #[test]
    fn nbsp_in_a_title_reads_as_a_normal_space() {
        let raw = "Cascading Style Sheets Level 2 Revision 1 (CSS\u{00a0}2.1) Specification\n";
        let text = collapse_whitespace(raw);
        assert!(
            text.contains("(CSS 2.1) Specification"),
            "nbsp must fold to a space, got {text:?}"
        );
        assert!(!text.contains('\u{00a0}'));
    }

    #[test]
    fn table_cell_tabs_become_markdown_pipes() {
        let raw = "Company\t Contact\t Country\nAlfreds Futterkiste\t Maria Anders\t Germany\n";
        let text = collapse_whitespace(raw);
        assert!(text.contains("Company | Contact | Country"), "got {text:?}");
        assert!(text.contains("Alfreds Futterkiste | Maria Anders | Germany"));
        assert!(!text.contains('\t'));
    }
}
