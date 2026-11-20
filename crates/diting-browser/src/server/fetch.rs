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
use std::sync::Arc;

/// `/fetch` read ceiling. Interactive navigation stays at 30s so a hung
/// CDP session cannot wedge the dispatcher. This path holds no session
/// lock, and the dynamic-column client waits 100s. Angular Material's
/// autocomplete page commits, then spends the rest of a ~50s debug-engine
/// load on the module compile and the lazy route wave; the 30s ceiling
/// cancels that work and the live read is the title shell.
const FETCH_NAV_TIMEOUT_MS: u64 = 60_000;

/// Page-scoped navigation ceiling for one `/fetch`. `None` leaves the
/// process default, so an explicit `DITING_NAV_TIMEOUT_MS` still wins.
fn fetch_nav_timeout_override(env_nav_timeout: Option<&str>) -> Option<u64> {
    match env_nav_timeout.map(str::trim).filter(|s| !s.is_empty()) {
        Some(_) => None,
        None => Some(FETCH_NAV_TIMEOUT_MS),
    }
}

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
        if ((!rendered_text_is_short(&text) && !outlet_empty) || started.elapsed() >= grace) {
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
///
/// `document.body.innerText` carries labels but never destinations, yet
/// markdown consumers (agents, list/detail extraction) score named
/// `[label](url)` records — and scoped contracts only count records inside
/// their own page section, so a links appendix at the end of the content
/// scores nothing. This pass walks `a[href]` in DOM order (visible anchors
/// only, computed-style display/visibility memoized up the ancestor chain),
/// finds each label's own position in the body text (whitespace-collapsed
/// view with an index map back into the raw string), and splices the
/// markdown link in place. Labels with no position in the text fall back to
/// a tail appendix.
/// Markdown face of a rendered page, spliced into `b` (the body innerText)
/// at each element's own text position — see `rendered_text`.
///
/// Two splice families share one edit list:
/// - `a[href]` anchors become `[label](url)` at the label's own position, so
///   scoped list sections keep owning their records. Labels drop
///   screen-reader-only runs (`(external, opens in a new tab)`) and hidden
///   anchors are skipped (computed-style walk, memoized with `__dlv`).
/// - `<table>`s become pipe tables that REPLACE the table's own text span
///   (anchored on the collapsed header-row and last-row text), because
///   innerText's cell-per-line face scores as neither a markdown table nor
///   a plain row — and appending them at the end puts them outside the
///   start/end scope most table contracts cut. A table whose span cannot
///   be located falls back to the tail appendix.
///
/// Edits apply back-to-front by raw offset (see the sort); table spans
/// occupy their interval first so link matches inside a table are not
/// spliced into its cells (the pipe table already carries the cell text).
const MARKDOWN_SPLICE_JS: &str = r#"
try{window.__dsOrig=b;}catch(e){}
function __srText(n){
  // Parts join with no separator: innerText concatenates inline siblings
  // ("Lana24" + sr-only "(external…)" renders "Lana24(external…)"), and a
  // space-joined anchor can never match the marker-joined text.
  var parts=[];
  (function w(x){for(var i=0;i<x.childNodes.length;i++){var c=x.childNodes[i];
    if(c.nodeType===3){parts.push(c.nodeValue);continue;}
    if(c.nodeType!==1)continue;
    var cl=c.className;if(cl&&cl.baseVal!==undefined)cl=cl.baseVal;
    if(/sr-only|screen.?reader|visually-hidden/i.test(cl||'')||c.getAttribute('aria-hidden')==='true')continue;
    w(c);}})(n);
  return parts.join('');
}
function __allText(n){
  // Every text node, sr-only included: the span window must budget what raw
  // innerText actually holds. EUI rows carry ~45 chars of sr-only copy text
  // per link cell; a DOM-visible-only estimate undershoots by hundreds of
  // chars over a 10-row table and the tail anchor then falls outside the
  // window (table gets tailed instead of replaced in place).
  var parts=[];
  (function w(x){for(var i=0;i<x.childNodes.length;i++){var c=x.childNodes[i];
    if(c.nodeType===3){parts.push(c.nodeValue);continue;}
    if(c.nodeType===1)w(c);}})(n);
  return parts.join('');
}
var __marks=[],__tables=[];
var __all=document.querySelectorAll('a[href]');
var __an=Math.min(__all.length,500);
for(var __i=0;__i<__an;__i++){
  var __a=__all[__i];
  var __h='';try{__h=__a.href||'';}catch(e){}
  if(__h.indexOf('http://')!==0&&__h.indexOf('https://')!==0){__h=(__a.getAttribute('href')||'').trim();if(!__h)continue;}
  var __lab=__srText(__a).replace(/\s+/g,' ').trim();
  if(!__lab)continue;
  var __el=__a,__hid=false;
  while(__el&&__el.nodeType===1){
    if(__el.__dlv===1)break;
    if(__el.__dlv===2){__hid=true;break;}
    var __cs=null;try{__cs=getComputedStyle(__el);}catch(e){}
    if(__cs&&(__cs.display==='none'||__cs.visibility==='hidden'||__cs.visibility==='collapse')){__el.__dlv=2;__hid=true;break;}
    __el=__el.parentElement;
  }
  if(__hid)continue;
  if(__lab.length>160)__lab=__lab.slice(0,160);
  __marks.push([__lab,__h]);
}
var __TRIM=/^[\s←-⇿⌀-⏿■-◿☀-➿︎️​-‏]+|[\s←-⇿⌀-⏿■-◿☀-➿︎️​-‏]+$/g;
function __cellText(el){return __srText(el).replace(/\s+/g,' ').trim().replace(__TRIM,'').replace(/\|/g,'\\|');}
function __rowText(cells,trim){var p=[];for(var i=0;i<cells.length;i++){var t=__srText(cells[i]).replace(/\s+/g,' ').trim();p.push(trim?t.replace(__TRIM,''):t);}return p.join(' ');}
function __hiddenTree(el){
  while(el&&el.nodeType===1){
    var cs=null;try{cs=getComputedStyle(el);}catch(e){}
    if(cs&&(cs.display==='none'||cs.visibility==='hidden'||cs.visibility==='collapse'))return true;
    el=el.parentElement;
  }
  return false;
}
function __ownCells(row){
  var cells=[];
  var all=row.querySelectorAll('th,td,[role=cell],[role=columnheader]');
  for(var i=0;i<all.length&&cells.length<40;i++){
    var c=all[i],p=c.parentElement,nested=false;
    while(p&&p!==row){
      if(p.tagName==='TD'||p.tagName==='TH'||p.getAttribute('role')==='cell'){nested=true;break;}
      p=p.parentElement;
    }
    if(!nested)cells.push(c);
  }
  return cells;
}
var __tabs=document.querySelectorAll('table,[role=table],[role=grid]');
var __emitted=0;
for(var __t=0;__t<__tabs.length&&__emitted<20;__t++){
  var __tb=__tabs[__t],__p=__tb.parentElement,__nested=false;
  while(__p){
    if(__p.tagName==='TABLE'||__p.getAttribute('role')==='table'||__p.getAttribute('role')==='grid'){__nested=true;break;}
    __p=__p.parentElement;
  }
  if(__nested||__hiddenTree(__tb))continue;
  var __rows=__tb.querySelectorAll('tr,[role=row]');
  if(!__rows.length)continue;
  var __headCells=__ownCells(__rows[0]);
  var __headers=[];
  for(var __c=0;__c<__headCells.length;__c++)__headers.push(__cellText(__headCells[__c]));
  if(!__headers.join('').trim())continue;
  var __lines=['| '+__headers.join(' | ')+' |'];
  var __sep=[];for(var __s=0;__s<__headers.length;__s++)__sep.push('---');
  __lines.push('| '+__sep.join(' | ')+' |');
  var __data=0,__span=__allText(__rows[0]).length+600,__lastCells=null;
  for(var __r=1;__r<__rows.length&&__data<100;__r++){
    var __tr=__rows[__r],__q=__tr.parentElement,__inHead=false;
    while(__q&&__q!==__tb){
      if(__q.tagName==='THEAD'){__inHead=true;break;}
      __q=__q.parentElement;
    }
    if(__inHead)continue;
    var __cells=__ownCells(__tr);
    if(!__cells.length)continue;
    var __vals=[];
    for(var __v=0;__v<__cells.length;__v++)__vals.push(__cellText(__cells[__v]));
    __lines.push('| '+__vals.join(' | ')+' |');
    __span+=__allText(__tr).length+32;
    __lastCells=__cells;__data++;
  }
  if(__data>0&&__lastCells){
    __tables.push({md:__lines.join('\n'),
      head:__rowText(__headCells,false).trim(),head2:__rowText(__headCells,true).trim(),
      tail:__rowText(__lastCells,false).trim(),tail2:__rowText(__lastCells,true).trim(),
      span:__span});
    __emitted++;
  }
}
if((__marks.length||__tables.length)&&b.length<500000){
  // The match view collapses whitespace runs to single spaces. EUI-class
  // grids also plant sr-only copy markers (↦ between cells, ↵ after rows)
  // that innerText carries — collapse those as separators too, or a
  // space-joined anchor can never match the marker-joined text.
  var __nb='',__map=[],__run=false;
  for(var __j=0;__j<b.length;__j++){
    var __c2=b.charAt(__j);
    var __w=/[\s↦↵]/.test(__c2);
    if(__w){if(__run)continue;__run=true;}else __run=false;
    __nb+=__w?' ':__c2;__map.push(__j);
  }
  var __used=[],__edit=[],__tail=[],__seen={},__tabFrom=0;
  for(var __ti=0;__ti<__tables.length;__ti++){
    var __t2=__tables[__ti];
    var __h0=__nb.indexOf(__t2.head,__tabFrom);
    if(__h0<0)__h0=__nb.indexOf(__t2.head2,__tabFrom);
    var __tEnd=-1,__tLen=0;
    if(__h0>=0){
      var __lim=Math.min(__nb.length,__h0+__t2.span);
      __tEnd=__nb.lastIndexOf(__t2.tail,__lim);
      if(__tEnd>__h0)__tLen=__t2.tail.length;
      if(__tEnd<0){__tEnd=__nb.lastIndexOf(__t2.tail2,__lim);if(__tEnd>__h0)__tLen=__t2.tail2.length;}
    }
    if(__h0>=0&&__tEnd>__h0){
      var __ts0=__map[__h0],__ts1=__map[__tEnd+__tLen-1]+1;
      // Swallow the row-end marker run right after the last cell, and
      // verify the replaced slice really starts/ends with the anchors —
      // an offset mismatch degrades to the tail appendix, never a splice
      // torn out of neighbouring text.
      while(__ts1<b.length&&/[\s↦↵]/.test(b.charAt(__ts1)))__ts1++;
      var __chk=b.slice(__ts0,__ts1).replace(/[\s↦↵]+/g,' ').replace(/^\s+|\s+$/g,'');
      if(__chk.indexOf(__t2.head)===0&&__chk.slice(-__t2.tail.length)===__t2.tail){
        __edit.push([__ts0,__ts1,__t2.md,null]);
        __used.push([__h0,__tEnd+__tLen]);
        __tabFrom=__tEnd+__tLen;
        continue;
      }
    }
    __tail.push(__t2.md);
  }
  for(var __m=0;__m<__marks.length;__m++){
    var __lab2=__marks[__m][0],__u=__marks[__m][1],__k=__lab2+'\u0000'+__u;
    if(__seen[__k])continue;__seen[__k]=1;
    var __from=0,__idx=-1;
    while(true){
      __idx=__nb.indexOf(__lab2,__from);
      if(__idx<0)break;
      var __ok=true;
      for(var __q2=0;__q2<__used.length;__q2++){
        if(__idx<__used[__q2][1]&&__used[__q2][0]<__idx+__lab2.length){__ok=false;break;}
      }
      if(__ok)break;
      __from=__idx+1;
    }
    if(__idx<0){__tail.push('['+__lab2.replace(/([\\[\]])/g,'\\$1')+']('+__u.replace(/([()])/g,'\\$1')+')');continue;}
    __used.push([__idx,__idx+__lab2.length]);
    __edit.push([__map[__idx],__map[__idx+__lab2.length-1]+1,__lab2,__u]);
  }
  // `__marks`/`__tables` follow DOM order, but `b` follows innerText order —
  // they diverge on flex `order`/floats, and back-to-front splicing with a
  // stale offset lands one link inside another's URL (unparseable markdown).
  __edit.sort(function(__p,__q){return __p[0]-__q[0];});
  for(var __e=__edit.length-1;__e>=0;__e--){
    var __ed=__edit[__e];
    // Table replacements carry their own trailing newline — the swallowed
    // marker run was the separator to whatever follows the table.
    var __rep=__ed[3]===null?__ed[2]+'\n':'['+__ed[2].replace(/([\\[\]])/g,'\\$1')+']('+__ed[3].replace(/([()])/g,'\\$1')+')';
    b=b.slice(0,__ed[0])+__rep+b.slice(__ed[1]);
  }
  if(__tail.length)b+='\n'+__tail.join('\n');
}
"#;

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
            //
            // With `links` (the markdown format), MARKDOWN_SPLICE_JS runs
            // right after the body read so `[label](url)` lands at each
            // label's own position and `<table>`s replace their own text
            // span with a pipe table — scoped list sections keep owning
            // their records — before the container/nav/footer appends
            // below.
            let mut js = "(function(){var b=document.body?document.body.innerText:'';".to_string();
            if links {
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
             var fm=feet.length<3?feet.length:3;\
             for(var f=0;f<fm;f++){var ft=(feet[f].textContent||'').replace(/[ \\t\\n\\r\\f]+/g,' ').trim();\
             if(!ft)continue;var head=ft.slice(0,60);var tail=ft.slice(Math.max(0,ft.length-60));\
             if(b.indexOf(head)<0||(tail!==head&&b.indexOf(tail)<0))b+='\\n'+ft;}\
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
            let browser = build_browser(req.use_proxy, &req.url, req.tls_fingerprint.as_deref())?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            if let Some(ms) =
                fetch_nav_timeout_override(std::env::var("DITING_NAV_TIMEOUT_MS").ok().as_deref())
            {
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
                if !e.to_string().contains("document committed") {
                    return Err(e.into());
                }
                tracing::warn!(
                    url = %req.url,
                    "fetch: navigation deadline after the document committed; reading the live page"
                );
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

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
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
    use super::{collapse_whitespace, fetch_nav_timeout_override, rendered_text_is_short};

    #[test]
    fn fetch_read_extends_the_nav_ceiling_unless_the_operator_set_one() {
        assert_eq!(fetch_nav_timeout_override(None), Some(60_000));
        assert_eq!(fetch_nav_timeout_override(Some("")), Some(60_000));
        assert_eq!(fetch_nav_timeout_override(Some("   ")), Some(60_000));
        assert_eq!(fetch_nav_timeout_override(Some("30000")), None);
        assert_eq!(fetch_nav_timeout_override(Some(" 45000 ")), None);
    }

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
    fn table_cell_tabs_become_markdown_pipes() {
        let raw = "Company\t Contact\t Country\nAlfreds Futterkiste\t Maria Anders\t Germany\n";
        let text = collapse_whitespace(raw);
        assert!(text.contains("Company | Contact | Country"), "got {text:?}");
        assert!(text.contains("Alfreds Futterkiste | Maria Anders | Germany"));
        assert!(!text.contains('\t'));
    }
}
