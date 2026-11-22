//! Stateless page acquire: fetch / click / eval + challenge ride-out.
//!
//! Challenge heuristics here are acquisition-path *shape* detection (Cloudflare
//! Turnstile, ByteDance WAF PoW) — not platform business. Site policy stays in
//! `workflow/`.

use super::browser::{build_browser, inject_cookies, run_on_local_runtime};
use super::fetch_splice::{collapse_whitespace, MARKDOWN_SPLICE_JS};
use super::route_fallback::{
    nav_deadline_ms, navigation_uses_direct, note_direct_navigation_outcome,
    note_proxy_fallback_started, proxy_retry_pair, starve_bail_ms, wait_for_load_or_deadline,
    FETCH_NAV_TIMEOUT_MS,
};
use crate::{
    ClickRequest, ClickResponse, EvalRequest, EvalResponse, FetchRequest, FetchResponse,
    OutputFormat,
};
use anyhow::{Context, Result};
use std::sync::Arc;

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

/// Check if the current page is an Anubis proof-of-work interstitial
/// (TecharoHQ anubis — winehq / kernel.org / fedora-docs class). The
/// challenge HTML carries `script[src*="/.within.website/x/cmd/anubis/"]`
/// plus a `Making sure you're not a bot!` title; both markers vanish the
/// moment the pass-challenge navigation replaces the document, which is
/// exactly the window this ride-out covers. Gray / deploy-facing, same
/// stance as the byte-WAF shape above: acquisition-path detection only.
fn is_anubis_challenge(page: &mut crate::page::Page) -> bool {
    let val = page.evaluate(
        "(function(){\
            try {\
                if (document.querySelector('script[src*=\"/.within.website/x/cmd/anubis/\"]')) return true;\
                return (document.title || '').indexOf(\"Making sure you're not a bot\") >= 0;\
            } catch (e) { return false; }\
        })()",
    );
    val.as_bool().unwrap_or(false)
}

/// The `#status` line of an Anubis interstitial: `Loading…` until
/// make-challenge answers, then `Speed: …kH/s` while the workers race,
/// `Success!` when a nonce lands.
fn anubis_status(page: &mut crate::page::Page) -> String {
    page.evaluate("((document.getElementById('status')||{}).textContent||'').trim()")
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// Ride out an Anubis PoW interstitial. The challenge solves itself
/// in-page (8 workers racing SHA-256 to difficulty 4, ~65k hashes median —
/// ~10s in this engine with an unlucky tail past 20s), then
/// `location.replace()`s to pass-challenge. That navigation lands in
/// `pending_navigation`, and nothing else in the stateless fetch path
/// drains it after the initial goto chain closed — the exact shape the
/// byte-WAF reload had. Pump in slices, drain the navigation when it
/// lands, and bail early when the status never leaves `Loading…` (the
/// module never executed — engine module-referrer bug class), so a dead
/// challenge doesn't burn the full budget.
pub(crate) async fn maybe_bypass_anubis(page: &mut crate::page::Page) -> Result<()> {
    if !is_anubis_challenge(page) {
        return Ok(());
    }
    let url = page.url();
    tracing::info!(
        "anubis PoW challenge detected at {}, riding out the proof-of-work",
        url
    );

    const SLICE_MS: u64 = 500;
    // Difficulty-4 PoW is ~65k hashes median — ~12s in this engine — with
    // an unlucky tail (winehq's IP-deterministic challenge needs 136k ≈
    // 20-25s). The fetch worker wall is 27s: goto ~4s + this budget +
    // extraction ~2s. 22s buys the median twice over and most of the tail;
    // past it the case is lost either way.
    const BUDGET_MS: u64 = 22_000;
    // make-challenge through a slow tunnel can take seconds; 8s of a
    // status frozen at the initial value means the module is dead, not slow.
    const LOADING_STALL_MS: u64 = 8_000;
    let started = std::time::Instant::now();
    let mut loading_since: Option<std::time::Instant> = None;
    loop {
        page.settle(SLICE_MS).await;
        // Drain a landed pass-challenge navigation (or any JS navigation
        // the settle woke up); navigation failure keeps the challenge page,
        // which the caller extracts as-is.
        match page.process_pending_navigation().await {
            Ok(true) => {
                // Real document (or SPA route sync): give the landing page
                // a beat before re-testing for a re-challenge below.
                page.settle(2_000).await;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("anubis pass-challenge navigation failed: {}", e);
            }
        }
        if !is_anubis_challenge(page) {
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "anubis challenge cleared at {}",
                url
            );
            return Ok(());
        }
        if started.elapsed().as_millis() as u64 >= BUDGET_MS {
            break;
        }
        let status = anubis_status(page);
        if status == "Loading..." {
            let since = *loading_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed().as_millis() as u64 >= LOADING_STALL_MS {
                tracing::warn!(
                    "anubis status stalled at 'Loading...' for {}ms at {} — module never executed",
                    since.elapsed().as_millis() as u64,
                    url
                );
                return Ok(());
            }
        } else {
            loading_since = None;
        }
    }
    tracing::warn!(
        "anubis challenge at {} still present after {}ms",
        url,
        BUDGET_MS
    );
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
/// Characters. ASCII pages under this are "short"; a longer body is done.
/// Counted in chars, not bytes: a Chinese nav shell is ~1100 characters
/// and ~1800 UTF-8 bytes, and the byte check used to treat that shell as
/// the finished document (Arco's component page).
const SHORT_TEXT_CHARS: usize = 1_200;
/// A title-only shell. Doc sites (EUI, PrimeNG) used to be read after the
/// app painted; giving up on the first empty sample is what made them
/// regress. Anything longer than a title is a finished short page, unless
/// an Angular `router-outlet` is still empty — the toolbar alone crosses
/// this line (Angular Material's is ~246 chars) before the route mounts.
const SHELL_BYTES: usize = 200;
/// Extra grace while that outlet is empty and a module or timer can still
/// fill it. The 6s short-text grace returns the toolbar if the examples
/// import starts during the gap after the sidenav graph evaluates.
const ROUTE_SHELL_GRACE_MS: u64 = 15_000;
/// Grace for the long-shell trap: doc sites (docusaurus class — EUI) paint
/// a multi-KB sidebar/nav first, so the body passes the "not short" bar
/// while `main` is still an empty route outlet. An empty main is not by
/// itself a work signal (a wedged realm must not idle the whole route
/// grace), so it pumps this long and then accepts the shell. Docusaurus
/// pages whose SSR HTML fails hydration (React #418/#423) fall back to a
/// full client render — cold loads through a proxy need the full window.
const MAIN_OUTLET_GRACE_MS: u64 = 8_000;
/// `settle_quiet` window. A 3s delayed paint is still pending; a 5s
/// heartbeat is not, so the grace does not idle out on analytics timers.
const SOON_WORK_MS: u64 = 3_500;

/// A body under [`SHORT_TEXT_CHARS`]. Byte length is only a fast path:
/// anything past four bytes per character cannot still be short, and a
/// pure-ASCII page never pays for a char walk.
fn rendered_text_is_short(text: &str) -> bool {
    if text.len() < SHORT_TEXT_CHARS {
        return true;
    }
    if text.len() >= SHORT_TEXT_CHARS * 4 {
        return false;
    }
    text.chars().count() < SHORT_TEXT_CHARS
}

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

fn page_has_soon_work(page: &mut crate::page::Page) -> bool {
    // A route `import()` or a stylesheet preload is invisible to the
    // in-page fetch map. Arco's table doc is that chunk: the shell is
    // already ~1KB, and a quiet probe used to return it before the
    // README module arrived.
    if page
        .inner
        .js
        .as_ref()
        .is_some_and(|js| js.has_pending_module_load())
    {
        return true;
    }
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

/// Angular inserts the routed component as the next element sibling of
/// `<router-outlet>`. Every outlet still empty means the shell has
/// painted and the route has not. No outlet at all is not this case.
fn router_outlet_unfilled(page: &mut crate::page::Page) -> bool {
    page.evaluate(
        "(function(){\
            var list=document.querySelectorAll('router-outlet');\
            if(!list.length) return false;\
            for(var i=0;i<list.length;i++){\
                var n=list[i].nextElementSibling;\
                if(n && n.tagName!=='ROUTER-OUTLET') return false;\
            }\
            return true;\
        })()",
    )
    .as_bool()
    .unwrap_or(false)
}

/// The docusaurus-class shell: a long sidebar makes the body read as
/// "done" while the routed document has not mounted yet. `main` (or
/// `[role=main]`) exists but carries (almost) no text. No main element is
/// not this case — pages that use neither keep their old behavior.
fn main_outlet_unfilled(page: &mut crate::page::Page) -> bool {
    page.evaluate(
        "(function(){\
            var m=document.querySelector('main,[role=main]');\
            if(!m) return false;\
            var t=(m.innerText||'').replace(/\\s+/g,' ').trim();\
            return t.length<40;\
        })()",
    )
    .as_bool()
    .unwrap_or(false)
}

async fn wait_for_rendered_text(
    page: &mut crate::page::Page,
    selector: Option<&str>,
    links: bool,
) -> String {
    let started = std::time::Instant::now();
    loop {
        // Keep this string. A second innerText walk of the same document
        // (GitHub, WHATWG) is what pushes the response past the client
        // read timeout after the navigation deadline already fired.
        let text = rendered_text(page, selector, links);
        let outlet_empty = main_outlet_unfilled(page);
        let unfilled = router_outlet_unfilled(page) || outlet_empty;
        let soon = page_has_soon_work(page);
        let grace = if unfilled && soon {
            std::time::Duration::from_millis(ROUTE_SHELL_GRACE_MS)
        } else if outlet_empty {
            std::time::Duration::from_millis(MAIN_OUTLET_GRACE_MS)
        } else {
            std::time::Duration::from_millis(EMPTY_TEXT_GRACE_MS)
        };
        // A long body is done — unless its main outlet is still empty and
        // the route grace has not expired: that body is the sidebar, not
        // the document.
        if (!rendered_text_is_short(&text) && !outlet_empty) || started.elapsed() >= grace {
            return text;
        }
        // Title-only shells keep pumping for the whole grace. A probe that
        // fails on a wedged realm must not look like "nothing is coming".
        // A toolbar past SHELL_BYTES with an empty outlet is still that shell.
        if text.len() >= SHELL_BYTES && !soon && !unfilled {
            return text;
        }
        page.pump_event_loop_slice(250).await;
    }
}

/// Inline link splice for the markdown body read (see `rendered_text`).
fn rendered_text(page: &mut crate::page::Page, selector: Option<&str>, links: bool) -> String {
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
            format!(
                "(function(){{var el=document.querySelector(`{escaped}`);return el?el.innerText:'';}})()"
            )
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
            //
            // With `links` (the markdown format), MARKDOWN_SPLICE_JS runs
            // right after the body read so `[label](url)` lands at each
            // label's own position and `<table>`s replace their own text
            // span with a pipe table — scoped list sections keep owning
            // their records — before the container/nav/footer appends
            // below.
            let mut js = "(function(){var b=document.body?document.body.innerText:'';".to_string();
            if links {
                // Pristine innerText snapshot for the containment probes
                // below (nav/footer/container appends): ATX prefixes and
                // [label](url) splices both break substring containment, so
                // comparing a raw probe against the rewritten body would
                // append whole <main>/<footer> duplicates of content that is
                // already there. Captured BEFORE the ATX pass for the same
                // reason, and overwritten on every evaluate so a retry on a
                // mutated DOM never probes against a stale snapshot.
                js.push_str("try{window.__dsOrig=b;}catch(e){}");
                // ATX headings: innerText flattens h1-h6 into plain lines, so
                // a markdown consumer loses the document outline entirely —
                // card-shaped contracts ("^#{2,3}\\s+(?<title>.+)$") then find
                // zero records on a page whose tiles are all headings
                // (apple.com). Prefix every body line that IS a heading's own
                // text with its level markers, BEFORE the splice so a heading
                // that is itself a link still becomes "## [Title](url)". Full
                // line equality (whitespace-collapsed) — partial matches stay
                // untouched; plain-text format keeps its bare lines.
                js.push_str(
                    "\
                 var __hs=document.querySelectorAll('h1,h2,h3,h4,h5,h6');\
                 if(__hs.length&&__hs.length<=400){var __hm={};\
                 for(var __hi=0;__hi<__hs.length;__hi++){\
                 var __ht=(__hs[__hi].innerText||'').replace(/\\s+/g,' ').trim();\
                 if(__ht&&__ht.length<=200&&!__hm[__ht])__hm[__ht]=parseInt(__hs[__hi].tagName.slice(1),10);}\
                 var __bl=b.split('\\n'),__ol=[];\
                 for(var __li=0;__li<__bl.length;__li++){\
                 var __lk=__bl[__li].replace(/\\s+/g,' ').trim();\
                 if(__lk&&__hm[__lk]&&!/^#{1,6}\\s/.test(__lk)){\
                 __ol.push(new Array(__hm[__lk]+1).join('#')+' '+__lk);}\
                 else{__ol.push(__bl[__li]);}}\
                 b=__ol.join('\\n');}",
                );
                js.push_str(MARKDOWN_SPLICE_JS);
            }
            js.push_str(
                "\
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
             var fm=feet.length<3?feet.length:3;var nb=null;\
             for(var f=0;f<fm;f++){\
             var ft=(feet[f].innerText||feet[f].textContent||'').replace(/^[ \\t\\r\\n\\f]+|[ \\t\\r\\n\\f]+$/g,'');\
             if(!ft)continue;\
             if(nb===null)nb=((typeof window!=='undefined'&&window.__dsOrig)||b).replace(/[ \\t\\n\\r\\f]+/g,' ').toLowerCase();\
             var fn=ft.replace(/[ \\t\\n\\r\\f]+/g,' ').toLowerCase();\
             var head=fn.slice(0,60);var tail=fn.slice(Math.max(0,fn.length-60));\
             if(nb.indexOf(head)<0||(tail!==head&&nb.indexOf(tail)<0))b+='\\n'+ft;}\
             var nodes=document.querySelectorAll('article.markdown-body,.markdown-body,#readme,#js_content,article,main,.theme-doc-markdown');\
             if(nodes.length>8)nodes=Array.prototype.slice.call(nodes,0,8);\
             var orig=(typeof window!=='undefined'&&window.__dsOrig)||null;\
             for(var i=0;i<nodes.length;i++){var a=nodes[i].innerText||'';var probe=a.trim().slice(0,80);\
             if(probe&&b.indexOf(probe)<0&&(!orig||orig.indexOf(probe)<0))b+='\\n'+a;}\
             var title=(document.title||'').trim();\
             if(title&&b.indexOf(title)<0)b=title+'\\n'+b;\
             return b;})()",
            );
            js
        }
    };
    let raw = page.evaluate(&js).as_str().unwrap_or("").to_string();
    // Collapse runs of whitespace (heavy SPA pages produce lots of blank
    // lines from empty layout containers) — keeps the output tight.
    collapse_whitespace(&raw)
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
            // Starvation bail deadline: only a true direct leg with a proxy
            // behind it, and only when the operator has not pinned an
            // explicit DITING_NAV_TIMEOUT_MS (nav_deadline_ms ranks: env >
            // starve bail > 60s default).
            let starve_ms = if direct_leg && proxy_configured {
                starve_bail_ms(std::env::var("DITING_STARVE_BAIL_MS").ok().as_deref())
            } else {
                0
            };
            if let Some(ms) = nav_deadline_ms(
                std::env::var("DITING_NAV_TIMEOUT_MS").ok().as_deref(),
                starve_ms,
            ) {
                page.inner.set_navigation_timeout(Some(ms));
            }
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
                let msg = e.to_string();
                if msg.contains("document committed") {
                    // Early deadline on a direct leg: distinguish a starving
                    // download (remote scripts still pending = throttled CDN
                    // trickle) from JS-bound work (PoW challenge, hydration
                    // — nothing left to download). Starving legs jump ship
                    // for the proxy while the worker's retry budget is
                    // still alive; JS-bound pages get their original budget
                    // back below.
                    if starve_ms > 0
                        && direct_leg
                        && super::route_fallback::pending_remote_scripts(&mut page).unwrap_or(0) > 0
                    {
                        tracing::info!(
                            url = %req.url,
                            "fetch: direct leg starving (remote scripts still pending); retrying through the proxy"
                        );
                        note_direct_navigation_outcome(&req.url, direct_leg, false);
                        note_proxy_fallback_started(&req.url, |host| {
                            diting::route_table::note_proxy_pending(host);
                        });
                        drop(page);
                        drop(browser);
                        let (page2, browser2) = proxy_retry_pair(
                            &req,
                            &format!("direct navigation starved after {starve_ms}ms"),
                        )
                        .await?;
                        page = page2;
                        browser = browser2;
                        direct_leg = false;
                        // The live proxy pair replaces the dropped direct
                        // pair: the geo-stub retry below drops `browser`
                        // again, so the binding stays owned across this
                        // fallback, exactly like the transport-death arm.
                    } else {
                        tracing::warn!(
                            url = %req.url,
                            "fetch: navigation deadline after the document committed; reading the live page"
                        );
                        note_direct_navigation_outcome(&req.url, direct_leg, true);
                        // The early deadline only shortens the wait when the
                        // page is JS-bound: hand the original budget back
                        // before reading the live page.
                        if starve_ms > 0 {
                            wait_for_load_or_deadline(
                                &mut page,
                                started + std::time::Duration::from_millis(FETCH_NAV_TIMEOUT_MS),
                            );
                        }
                    }
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

/// Byte-fetch a binary document (PDF / DOCX) and pull its text out with the
/// salvage extractor ([`crate::server::pdf_text`]). `None` on any failure —
/// the caller then keeps the binary-document refusal, so a parser miss
/// degrades to the honest error rather than a half-empty answer. Mirrors
/// `http_fetch`'s proxy decision instead of the browser's because the bytes
/// are the same origin the navigation just reached; no rate re-check (the
/// navigation already counted this request).
async fn binary_document_text(url: &str, use_proxy: bool) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let use_proxy = use_proxy || crate::config::should_auto_proxy(url);
    let proxy = if use_proxy {
        crate::config::proxy_from_env()
    } else {
        None
    };
    let jar = Arc::new(diting::diting_net::CookieJar::new());
    let mut client =
        diting::diting_net::HttpClient::with_full_options(jar, proxy.as_deref(), false);
    client.emulate_chrome = true;
    let resp = client.fetch(&parsed).await.ok()?;
    if resp.status != 200 {
        return None;
    }
    let text = if resp.body.starts_with(b"%PDF") {
        crate::server::pdf_text::pdf_to_text(&resp.body)
    } else if resp.body.starts_with(b"PK\x03\x04") {
        crate::server::docx_text::docx_to_text(&resp.body)
    } else {
        None
    }?;
    // A parse that yielded soup or nothing is not an answer.
    if text.trim().len() < 200 {
        return None;
    }
    Some(text)
}

/// DOM+app probe for embedded documents (returns a JSON array of absolute
/// URLs). Two families, both invisible to the navigation response:
/// - HTML embeds: `<embed>/<object>/<iframe>` with a `.pdf` src or an
///   `application/pdf` type (PDFObject and friends put the URL in the DOM).
/// - pdf.js viewers: no DOM trace at all — the document URL lives in the
///   app's own options (`PDFViewerApplicationOptions.get("defaultUrl")`,
///   the build default kept by virtually every deployment), the opened
///   document's `PDFViewerApplication.url`/`_downloadUrl`, or the `?file=`
///   query param. In our engine pdf.js initializes but never fetches its
///   default document, so this config read is the only signal there is.
const EMBEDDED_DOC_PROBE_JS: &str = r#"
(function(){
  var out = [], seen = {};
  function add(u){
    if (!u) return;
    try { u = new URL(u, location.href).href } catch(e){ return }
    u = u.split('#')[0];
    if (u && !seen[u]) { seen[u] = 1; out.push(u); }
  }
  document.querySelectorAll('embed,object,iframe').forEach(function(el){
    var u = (el.getAttribute('src') || el.getAttribute('data') || '').trim();
    var t = (el.getAttribute('type') || '').toLowerCase();
    if (!u || (!/\.pdf([?#]|$)/i.test(u) && t.indexOf('pdf') < 0)) return;
    add(u);
  });
  try {
    var q = new URLSearchParams(location.search).get('file');
    if (q) add(q);
  } catch(e){}
  try {
    var o = window.PDFViewerApplicationOptions;
    var d = o && o.get && o.get('defaultUrl');
    if (d) add(d);
  } catch(e){}
  try {
    var a = window.PDFViewerApplication;
    if (a) { if (a.url) add(a.url); if (a._downloadUrl) add(a._downloadUrl); }
  } catch(e){}
  return JSON.stringify(out);
})()
"#;

/// Embedded-document splicer for text/markdown fetches: probe, dedupe,
/// byte-fetch each candidate through [`binary_document_text`], join the
/// survivors under labeled headers. Guarded by the request clock — a page
/// that burned its budget on settle must not add 2× the fetch timeout on
/// top. `None` when there is nothing to append (no hits, no extractable
/// bytes, or over budget).
async fn embedded_documents_text(
    req: &FetchRequest,
    page: &mut crate::page::Page,
    started: std::time::Instant,
) -> Option<String> {
    // Same frame as the settle tail's 23s budget, but tighter: leave the
    // response-transfer room that tail comment promises.
    const EMBED_DOC_DEADLINE_MS: u64 = 18_000;
    const EMBED_DOC_MAX: usize = 2;
    let urls: Vec<String> =
        serde_json::from_str(page.evaluate(EMBEDDED_DOC_PROBE_JS).as_str()?).ok()?;
    let mut parts: Vec<String> = Vec::new();
    for url in urls {
        if parts.len() >= EMBED_DOC_MAX
            || started.elapsed().as_millis() as u64 > EMBED_DOC_DEADLINE_MS
        {
            break;
        }
        // The navigation response itself already went through the
        // content-type gate above — refetching it here would double-count
        // the request and duplicate its text.
        if url == req.url || url.is_empty() {
            continue;
        }
        let url = url.trim().to_string();
        if let Some(text) = binary_document_text(&url, req.use_proxy).await {
            tracing::info!(
                url = %url,
                chars = text.len(),
                "fetch: embedded document extracted"
            );
            parts.push(format!("[Embedded document: {url}]\n\n{text}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
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
    use super::{collapse_whitespace, rendered_text_is_short};

    #[test]
    fn chinese_nav_shell_is_still_short() {
        let shell = "开发指南".repeat(200);
        assert!(shell.len() > 1_200);
        assert!(shell.chars().count() < 1_200);
        assert!(rendered_text_is_short(&shell));
        assert!(!rendered_text_is_short(&"a".repeat(1_200)));
    }

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
    fn installment_glue_gets_its_space_back_and_nothing_else_does() {
        // <span>or 9 x</span><b>$1.21</b> renders with no whitespace node, so
        // the line arrives glued; the amount keeps its own word.
        let text = collapse_whitespace("Cropped Stay Groovy off white\n$10.90\nor 9 x$1.21\n");
        assert!(text.contains("or 9 x $1.21"), "got {text:?}");
        // Anything outside "or <digits> x$" keeps its exact bytes.
        let keep = collapse_whitespace("Box 4 x$2\nor x$1\nor 9x$1.21\nTorx$6\n");
        assert!(keep.contains("4 x$2") && keep.contains("or x$1"));
        assert!(keep.contains("or 9x$1.21") && keep.contains("Torx$6"));
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
