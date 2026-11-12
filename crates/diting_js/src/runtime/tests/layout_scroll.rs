//! 滚动与视口：scroller 级联/overflow 传播/scrollend/sticky
use super::super::*;
use super::common::setup_runtime;

#[tokio::test(flavor = "current_thread")]
async fn test_element_scroll_offsets_and_scroll_event_coalescing() {
    // Upstream 29e20ae + 1c7402d: scrollTop/scrollLeft round-trip, direct
    // assignment fires a scroll event (only on change), and scroll
    // operations coalesce to one event per call.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const el = document.createElement('div');
            document.body.appendChild(el);
            let events = 0;
            el.addEventListener('scroll', () => events++);
            el.scrollTop = 100;          // changed -> 1 event
            el.scrollTop = 100;          // unchanged -> no event
            el.scrollTo(0, 250);         // one coalesced event
            el.scrollBy({ left: 30, top: 50 });
            el.scroll(0, -5);            // clamps both axes back to 0, 1 event
            const offsets = [el.scrollTop, el.scrollLeft];
            await new Promise(r => setTimeout(r, 10));
            return [offsets, events];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([[0, 0], 4]));
}

#[tokio::test(flavor = "current_thread")]
async fn test_window_scroll_moves_page_offset_shared_with_scrolling_element() {
    // Upstream f6ca133: window scroll methods move the page offset stored
    // on the scrolling element; scrollX/scrollY/pageXOffset/pageYOffset are
    // views of it, and a window scroll reaches document AND window listeners.
    // Tall page: the root scroller clamps to the real scroll range now, so
    // the offsets below must stay inside it to actually move.
    let mut rt = setup_runtime(
        r#"<html><body><div id="d"></div><div style="height:5000px"></div><div style="width:5000px;height:1px"></div></body></html>"#,
    );
    let script = r#"async () => {
            const isDocEl = document.scrollingElement === document.documentElement;
            window.scrollTo(0, 500);
            const afterTo = [window.scrollX, window.scrollY];
            window.scrollBy(0, 200);
            const afterBy = [window.pageXOffset, window.pageYOffset];
            window.scrollTo({ left: 10, top: 40 });
            const afterOptions = [window.scrollX, window.scrollY];
            window.scrollTo(0, -100);
            const afterClamp = window.scrollY;
            document.scrollingElement.scrollTop = 90;
            const viaWindow = window.scrollY;
            let win = 0, doc = 0;
            window.addEventListener('scroll', () => win++);
            document.addEventListener('scroll', () => doc++);
            window.scrollBy(0, 400);
            await new Promise(r => setTimeout(r, 10));
            // Five window scroll ops ran in total (four above the listeners
            // plus the final scrollBy); each fires exactly one scroll at the
            // document and one at the window, all drained by the await.
            return [isDocEl, afterTo, afterBy, afterOptions, afterClamp, viaWindow, win, doc, window.scrollY];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([true, [0, 500], [0, 700], [10, 40], 0, 90, 5, 5, 490])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_scrollend_fires_after_scroll_completes_and_not_without_translation() {
    // Blitz#354: scrollend trails scroll once scrolling finishes (same
    // tick here — our scrolls are instant) and, unlike scroll, bubbles —
    // so the element scrollend reaches document AND window listeners up
    // the chain (as in Chrome, via the M3 native core's document →
    // window path edge), while the window-path fire adds one more at
    // each. The negative clause holds too: a scroll op that translated
    // nothing fires neither scroll nor scrollend. Tall page so the
    // window scroll has a real range to move in (the root scroller
    // clamps to it now).
    let mut rt = setup_runtime("<html><body><div style=\"height:5000px\"></div></body></html>");
    let script = r#"async () => {
            const el = document.createElement('div');
            document.body.appendChild(el);
            let se = 0, wse = 0, dse = 0;
            el.addEventListener('scrollend', () => se++);
            window.addEventListener('scrollend', () => wse++);
            document.addEventListener('scrollend', () => dse++);
            el.scrollTop = 100;       // moved -> element scrollend, bubbles to document + window
            el.scrollTo(0, 100);      // same position -> no move -> nothing
            window.scrollTo(0, 400);  // moved -> document + window scrollend
            window.scrollTo(0, 400);  // same position -> no move -> nothing
            await new Promise(r => setTimeout(r, 10));
            return [se, wse, dse];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([1, 2, 2]));
}

#[tokio::test(flavor = "current_thread")]
async fn test_focus_and_scrollintoview_move_viewport() {
    // #215 (obscura #1133): focus() implies scrollIntoView
    // nearest/nearest, and explicit scrollIntoView modes align the box
    // in the scrollport. Tall page so the root scroller has a real range.
    let mut rt = setup_runtime(
        r#"<html><body><div style="height:4000px"></div><input id="f"><div style="height:6000px"></div></body></html>"#,
    );
    let script = r#"async () => {
            const f = document.getElementById('f');
            f.focus();
            const r1 = f.getBoundingClientRect();
            const nearestBottom = Math.abs(r1.bottom - innerHeight) < 2;
            f.scrollIntoView({block: 'center'});
            const r2 = f.getBoundingClientRect();
            const centered = Math.abs((r2.top + r2.bottom) / 2 - innerHeight / 2) < 2;
            f.scrollIntoView(true);
            const r3 = f.getBoundingClientRect();
            const atTop = Math.abs(r3.top) < 2;
            f.scrollIntoView(false);
            const r4 = f.getBoundingClientRect();
            const atBottom = Math.abs(r4.bottom - innerHeight) < 2;
            window.scrollTo(0, 0);
            f.blur();
            f.focus({preventScroll: true});
            const afterPrevent = window.scrollY;
            f.blur();
            f.focus();
            const afterRefocus = window.scrollY;
            return [nearestBottom, centered, atTop, atBottom, afterPrevent, afterRefocus > 0];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
            result.value.unwrap(),
            serde_json::json!([true, true, true, true, 0, true]),
            "focus scrolls nearest, block:center/start/end align, preventScroll opts out, refocus scrolls again"
        );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_root_scroll_mirrors_to_native_band_paint_state() {
    // AginxOS P0: the CDP band painter reads the root scroll offset from
    // JsState. window.scrollTo and direct root writes (through either the
    // html or the body wrapper — CSSOM View proxies body to the scrolling
    // element) must all land in that one native pair.
    // Tall AND wide: the root scroller clamps to the real scroll range on
    // both axes now, so the horizontal writes below need a wide document
    // to stay inside the range (a viewport-wide page pins scrollLeft at 0,
    // the same way Chrome does).
    let mut rt = setup_runtime(
        r#"<html><body><div style="height:5000px"></div><div style="width:5000px;height:1px"></div></body></html>"#,
    );
    let script = r#"async () => {
            window.scrollTo(0, 300);
            document.documentElement.scrollLeft = 20;
            document.body.scrollTop = 1000;   // proxies to the scrolling element
            await new Promise(r => setTimeout(r, 10));
            return [
                window.scrollY,
                document.body.scrollTop,
                document.documentElement.scrollTop,
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([1000, 1000, 1000]));
    let (sx, sy) = rt.with_state(|st| st.scroll_offset);
    assert_eq!((sx, sy), (20.0, 1000.0));
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_with_z_index_outranks_later_static_content() {
    // #434 second half: a sticky with an explicit z-index joins the
    // hoisted positive band. On uv-docs (mkdocs-material) the stuck
    // .md-header carries z-index:4 while a later static .md-container
    // spans the whole scroll — before sticky counted as positioned, the
    // header ranked static (level 0) and lost both paint and hit order
    // to that sibling. (Cross-level ordering vs an absolute overlay
    // parented at the ICB stays out of scope: per-parent bands are the
    // engine's documented approximation.)
    let mut rt = setup_runtime(
        r#"<html><body><div id="head" style="position:sticky; top:0; height:40px; background:#ccc; z-index:4">H</div><div id="tall" style="height:4000px; background:#eee">filler</div></body></html>"#,
    );
    let script = r#"async () => {
            window.scrollTo(0, 300);
            await new Promise(r => setTimeout(r, 10));
            const pick = (y) => {
                const el = document.elementFromPoint(10, y);
                return el ? (el.id || el.tagName) : 'null';
            };
            const stack = document.elementsFromPoint(10, 5)
                .slice(0, 2).map(el => el.id || el.tagName);
            const out = [pick(5), pick(45), stack];
            window.scrollTo(0, 0);
            return out;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["head", "tall", ["head", "tall"]]),
        "stuck z:4 header owns its band; past the band the static sibling answers again"
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_header_stops_at_containing_block_end() {
    // #434: the sticky shift clamps to the containing block — a 40px
    // header inside a 200px wrap never scrolls past wrap.bottom - 40,
    // however deep the document scrolls.
    let mut rt = setup_runtime(
        r#"<html><body><div id="wrap" style="height:200px"><div id="head" style="position:sticky; top:0; height:40px; background:#ccc">H</div></div><div style="height:4000px"></div></body></html>"#,
    );
    let script = r#"async () => {
            const head = document.getElementById('head');
            const wrap = document.getElementById('wrap');
            window.scrollTo(0, 300);
            await new Promise(r => setTimeout(r, 10));
            const stuck = head.getBoundingClientRect().top;
            const stop = wrap.getBoundingClientRect().top + wrap.getBoundingClientRect().height - 40;
            return [Math.abs(stuck - stop) < 0.5, stuck];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let value = result.value.unwrap();
    assert_eq!(
        value.as_array().unwrap().first().unwrap(),
        &serde_json::json!(true),
        "stuck top == wrap.bottom - 40 (raw value: {value})"
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_element_scroller_clamps_and_shifts_descendant_gbcr() {
    // sticky v2: an overflow:auto element is a real scroll container.
    // scrollTop clamps to extent-client (1000-200), the write mirrors
    // into the native scroll table, and the child's gBCR travels by
    // exactly the scroll — gBCR and paint read the same shift map.
    let mut rt = setup_runtime(
        r#"<html><body><div id="box" style="height:200px; overflow:auto; margin:0; padding:0; border:none"><div id="kid" style="height:1000px">x</div></div></body></html>"#,
    );
    let script = r#"async () => {
            const box = document.getElementById('box');
            const kid = document.getElementById('kid');
            const before = kid.getBoundingClientRect().top;
            box.scrollTop = 99999;
            await new Promise(r => setTimeout(r, 10));
            const after = kid.getBoundingClientRect().top;
            const sh = box.scrollHeight;
            return [box.scrollTop, Math.round(before - after), sh >= 1000 && sh <= 1016];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([800, 800, true]),
        "clamped to extent-client, child gBCR drops by the scroll, scrollHeight reflects content"
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_nested_element_scrollers_compose_shifts() {
    // Two stacked scrollers: outer=100 shifts the inner BOX (and its
    // subtree) by -100; inner=50 shifts the leaf another -50. The
    // nearest-source base telescopes, so the leaf's gBCR drop is the
    // SUM, 150 — outer's later write must not clobber inner's more
    // specific descendant shift.
    let mut rt = setup_runtime(
        r#"<html><body><div id="outer" style="height:200px; overflow:auto; margin:0; padding:0; border:none"><div id="spacer" style="height:500px"></div><div id="inner" style="height:150px; overflow:auto; margin:0; padding:0; border:none"><div id="leaf" style="height:600px">L</div></div></div></body></html>"#,
    );
    let script = r#"async () => {
            const outer = document.getElementById('outer');
            const inner = document.getElementById('inner');
            const leaf = document.getElementById('leaf');
            const before = leaf.getBoundingClientRect().top;
            outer.scrollTop = 100;
            inner.scrollTop = 50;
            await new Promise(r => setTimeout(r, 10));
            const after = leaf.getBoundingClientRect().top;
            return [outer.scrollTop, inner.scrollTop, Math.round(before - after)];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([100, 50, 150]),
        "both writes round-trip clamped, leaf drop = outer + inner"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_intersection_observer_drives_infinite_scroll_to_completion() {
    // The obstacle-course `observer-intersection` pattern (obscura-benchmark,
    // discussed upstream in #671): a sentinel is observed, each intersection
    // appends a batch, and the feed must reach the cap and set the done flag.
    // Our IO is geometry-naive (always intersecting) but re-fires on DOM
    // mutations + a burst schedule, so the pattern completes regardless of
    // font-metric-driven card heights (real-geometry engines flip pass/fail
    // on the default 18px line height).
    let mut rt =
        setup_runtime("<html><body><div id=feed></div><div id=sentinel></div></body></html>");
    let script = r#"async () => {
            const feed = document.getElementById('feed');
            let loaded = 0;
            // 30, not the fixture's 50: completion is driven by the burst
            // schedule (120/500/1500/3500/7000ms), and a unit test should
            // not wait out the late bursts. The invariant is "completes and
            // terminates", not "reaches 50".
            const BATCH = 10, MAX = 30;
            const io = new IntersectionObserver((entries) => {
                for (const e of entries) {
                    if (e.isIntersecting && loaded < MAX) {
                        for (let i = 0; i < BATCH && loaded < MAX; i++) {
                            const card = document.createElement('div');
                            card.className = 'card';
                            feed.appendChild(card);
                            loaded++;
                        }
                        if (loaded >= MAX) { io.disconnect(); window.__done = 'io:' + loaded; }
                    }
                }
            });
            io.observe(document.getElementById('sentinel'));
            // The chain advances one batch per loop turn (mutation hop or
            // burst timer), so poll until the flag lands rather than a
            // single fixed sleep.
            for (let k = 0; k < 40 && !window.__done; k++) {
                await new Promise(r => setTimeout(r, 25));
            }
            return [loaded, String(window.__done || '')];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([30, "io:30"]));
}

#[cfg(feature = "screenshot")]
#[test]
fn test_scroll_height_reports_overflow_extent_not_own_box() {
    // blitz#444 family: scrollHeight must be the scrollable overflow
    // extent (union of the laid-out subtree), not the element's own box
    // height — otherwise scrollHeight - clientHeight is always 0 and
    // neither our wheel helper nor a page's lazy loader can detect
    // scrollability. client* keep the box contract; scroll* must move.
    let mut rt = setup_runtime(
            "<html><body><div id=\"box\" style=\"width:200px;height:100px;overflow:auto\"><div style=\"height:500px\"></div></div></body></html>",
        );
    let result = rt
        .evaluate(
            r#"
            const el = document.getElementById("box");
            return [el.scrollHeight, el.clientHeight,
                    el.scrollWidth, el.clientWidth,
                    el.scrollHeight - el.clientHeight > 0];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(500),
        "scrollHeight serves the descendant overflow extent, not 100"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(100),
        "clientHeight stays the box"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(200),
        "no horizontal overflow -> scrollWidth equals the box width"
    );
    assert_eq!(parts[3], serde_json::json!(200));
    assert_eq!(
        parts[4],
        serde_json::json!(true),
        "scrollability is now detectable via scrollHeight - clientHeight"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_document_scroll_height_tracks_page_extent_with_viewport_floor() {
    // The document scrolling area must track real page extent (tall page
    // -> scrollHeight > innerHeight) and clamp UP to the viewport on
    // short pages — Chrome never lets the root scrolling area be smaller
    // than the window. clientHeight keeps the viewport contract either
    // way, so scrollHeight - clientHeight stays a working overflow probe.
    // 5000px, not 2000px: the stealth persona draws innerHeight from its
    // screen pool (a 4K panel gives 2080), so a "tall" page must be tall
    // against the largest plausible persona viewport, not just 800.
    let mut rt = setup_runtime("<html><body><div style=\"height:5000px\"></div></body></html>");
    let result = rt
        .evaluate(
            r#"
            return [document.documentElement.scrollHeight > innerHeight,
                    document.body.scrollHeight > innerHeight,
                    document.documentElement.scrollHeight >= document.body.scrollHeight];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "documentElement.scrollHeight reports the real page extent"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "body.scrollHeight reports the real content extent"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "html extent contains the body extent"
    );

    let mut rt = setup_runtime("<html><body><p>short</p></body></html>");
    let result = rt
        .evaluate(
            r#"
            return [document.documentElement.scrollHeight === innerHeight,
                    document.body.scrollHeight === innerHeight,
                    document.documentElement.clientHeight === innerHeight];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "short page clamps the root scrolling area up to the viewport"
    );
    assert_eq!(parts[1], serde_json::json!(true));
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "clientHeight keeps the viewport contract"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_body_overflow_hidden_propagates_to_viewport() {
    // blitz#880 (css-overflow-3 §3.3): body's overflow hands up to the
    // viewport when html is visible — the scrolling area collapses to
    // exactly the viewport (window.scrollTo pins at 0), while body's own
    // scrollHeight keeps its content extent (its used overflow flipped
    // back to `visible`). Before the fix the viewport scrolled freely:
    // the extent walk never consulted overflow at all.
    let mut rt = setup_runtime(
        "<html><body style=\"overflow:hidden\"><div style=\"height:5000px\"></div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            window.scrollTo(0, 1000);
            return [document.documentElement.scrollHeight === innerHeight,
                    window.scrollY === 0,
                    document.body.scrollHeight > innerHeight];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "propagated hidden collapses scrollingElement.scrollHeight to the viewport"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "window scrolling is pinned at 0 for a hidden viewport"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "body keeps its content extent (used overflow became visible)"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_html_overflow_hidden_collapses_scroll_range() {
    // Same propagation rule for html's own overflow: it belongs to the
    // viewport, so hidden there pins the scroll range at the viewport
    // instead of leaving html/body clamping up to a 5000px content
    // extent. A visible body does NOT propagate over it — html's own
    // non-visible value wins either way.
    let mut rt = setup_runtime(
        "<html style=\"overflow:hidden\"><body><div style=\"height:5000px\"></div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            window.scrollTo(0, 1000);
            return [document.documentElement.scrollHeight === innerHeight,
                    window.scrollY === 0];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(parts[0], serde_json::json!(true));
    assert_eq!(parts[1], serde_json::json!(true));
}

#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_body_overflow_scroll_still_scrollable() {
    // Regression guard for the propagation fix: an explicit scrollable
    // value on body propagates too (viewport overflow = auto/scroll) and
    // the viewport must keep scrolling — the collapse only fires for
    // hidden/clip.
    let mut rt = setup_runtime(
        "<html><body style=\"overflow:scroll\"><div style=\"height:5000px\"></div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            window.scrollTo(0, 1000);
            return [document.documentElement.scrollHeight > innerHeight,
                    window.scrollY === 1000];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(parts[0], serde_json::json!(true));
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "propagated scroll keeps the viewport scrollable"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_bare_text_body_scroll_height_serves_text_extent() {
    // blitz#444's text-only residual: a body holding nothing but text
    // nodes has no element box past html/body — and those stretch to the
    // viewport — so the element walk sees no overflow and the page could
    // never scroll past its first screenful. The Text paint items carry
    // the true extent (wrap-model estimate, errs high); the root fold
    // lifts scrollHeight past innerHeight. 1000 chars at width 20 ≈ 9600
    // estimated px, safely above the largest persona viewport (2080).
    let html = format!(
        "<html><body style=\"width:20px;font-size:16px;line-height:20px\">{}</body></html>",
        "x".repeat(1000)
    );
    let mut rt = setup_runtime(&html);
    let result = rt
        .evaluate(
            r#"
            return [document.documentElement.scrollHeight > innerHeight,
                    document.body.scrollHeight > innerHeight,
                    document.documentElement.scrollHeight >= document.body.scrollHeight];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "bare text taller than the viewport must make the root scrollable"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "body scrollHeight must serve the text extent, not its stretched box"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "html extent contains the body extent"
    );
}
