//! Rendered-text read for `/fetch`: how long the browser tier keeps pumping
//! a page whose text is still empty, the SPA-shell probes, and the innerText
//! read itself.

use super::fetch_splice::{collapse_whitespace, MARKDOWN_SPLICE_JS};

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

pub(super) async fn wait_for_rendered_text(
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
pub(super) fn rendered_text(
    page: &mut crate::page::Page,
    selector: Option<&str>,
    links: bool,
) -> String {
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

#[cfg(test)]
mod tests {
    use super::rendered_text_is_short;

    #[test]
    fn chinese_nav_shell_is_still_short() {
        let shell = "开发指南".repeat(200);
        assert!(shell.len() > 1_200);
        assert!(shell.chars().count() < 1_200);
        assert!(rendered_text_is_short(&shell));
        assert!(!rendered_text_is_short(&"a".repeat(1_200)));
    }
}
