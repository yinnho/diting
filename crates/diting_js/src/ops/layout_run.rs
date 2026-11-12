// 布局执行族：整树 layout run + iframe 视口 + 吸附/滚动器分类 + 墨迹范围 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// One style+layout run over the live tree (see [`JsState::layout_cache`]):
/// every element's border-box rect, the paint order, the cascaded
/// ComputedStyle per element, the flat paint-item list in paint order
/// (band paint's input — computed anyway, so caching it is free), and the
/// per-element LOCAL geometry pair (pre-map border box + total accumulated
/// map) backing the event-coordinate surface (`local_geom`/`event_offset`
/// ops, blitz #663 family).
#[cfg(feature = "screenshot")]
pub(super) type LayoutRun = (
    HashMap<NodeId, [f32; 4]>,
    Vec<NodeId>,
    HashMap<NodeId, diting_css::ComputedStyle>,
    Vec<diting_layout::PaintItem>,
    HashMap<NodeId, ([f32; 4], [f32; 6])>,
    // Sticky subtree item spans (root-scroller v1): (node, start, end) into
    // the items vec — see layout_collect. Readers pair them with the
    // scroll-dependent shift map (sticky_shifts).
    Vec<diting_layout::StickySpan>,
    // Scroller subtree item spans (sticky v2): same shape, one per real
    // scroll container, recorded after its Clip and closed before its
    // PopClip — the read-time shift walk translates the span when the
    // container's scrollTop/scrollLeft is non-zero.
    Vec<diting_layout::StickySpan>,
);

/// Run the full diting style + layout pipeline over the live DOM tree and
/// return every element's border-box rect, the paint order (see
/// `layout_dom_with_paint_order_and_images`) and the cascaded ComputedStyle
/// per element (for the `computed_style` op). Styles are re-collected each
/// run: attribute-level mutations (style/class writes) don't bump the tree
/// epoch, so memoizing computed styles alongside the rects would serve
/// stale geometry.
#[cfg(feature = "screenshot")]
pub(super) fn layout_run_all(gs: &JsState, dom: &DomTree) -> LayoutRun {
    // Debug knob (DITING_LAYOUT_TRACE=1): phase timings for a full
    // layout run. The interrupt-sampler evidence on the WeChat article pages
    // shows multi-second stretches with no V8 stack check — candidates are
    // the Rust phases below, each re-done from scratch on every epoch bump.
    let trace = std::env::var("DITING_LAYOUT_TRACE").is_ok();
    let t0 = std::time::Instant::now();
    // Same viewport the persona publishes to window.innerWidth/innerHeight,
    // so geometry agrees with what scripts read off `window` (and the ICB
    // has a definite size for fixed-box inset resolution — obscura#675).
    let (viewport_width, viewport_height) = gs.viewport;
    let mut css = String::new();
    // Sheets join in DOCUMENT order (a comma query returns exactly that):
    // an earlier <link> loses equal-specificity ties to a later <style>,
    // like Chrome's cascade. External sheet bodies come from the
    // navigation-time fetch table (JsState::ext_sheets); a <link> whose
    // fetch failed (or was blocked) contributes nothing, matching a
    // browser that leaves the sheet empty rather than dropping the page's
    // other styling.
    if let Ok(els) = dom.query_selector_all("style, link") {
        for el in els {
            let meta = dom.with_node(el, |n| {
                let tag = n.as_element()?.local.to_ascii_lowercase();
                Some((
                    tag,
                    n.get_attribute("rel").map(|v| v.to_ascii_lowercase()),
                    n.get_attribute("href").map(|v| v.to_string()),
                ))
            });
            let Some((tag, rel, href)) = meta.flatten() else {
                continue;
            };
            match &*tag {
                "style" => {
                    // A sheet edited through CSSOM (insertRule on style.sheet — how
                    // CSS-in-JS libraries style) overrides the element's text.
                    match gs
                        .ext_sheets
                        .borrow()
                        .get(&format!("\u{0}style:{}", el.index()))
                        .and_then(|v| v.strip_prefix('\u{2}'))
                    {
                        Some(edited) => css.push_str(edited),
                        None => css.push_str(&dom.text_content(el)),
                    }
                    css.push('\n');
                }
                "link" => {
                    let is_sheet = rel
                        .as_deref()
                        .is_some_and(|r| r.split_ascii_whitespace().any(|t| t == "stylesheet"));
                    if !is_sheet {
                        continue;
                    }
                    let Some(href) = href.filter(|h| !h.is_empty()) else {
                        continue;
                    };
                    let abs = Url::parse(&gs.url)
                        .ok()
                        .and_then(|base| base.join(&href).ok())
                        .map(|u| u.to_string());
                    let Some(abs) = abs else { continue };
                    if let Some(body) = gs.ext_sheets.borrow().get(&abs) {
                        css.push_str(body);
                        css.push('\n');
                    }
                }
                _ => {}
            }
        }
    }
    // Shadow trees are invisible to the document-rooted query above, but
    // their <style> blocks style shadow content (composed rendering, phase
    // 2). They join the pool after the light sheets — same global-pool
    // approximation the rule matcher uses.
    for text in diting_layout::shadow_style_texts(dom) {
        css.push_str(&text);
        css.push('\n');
    }
    if let Some(adopted) = gs.ext_sheets.borrow().get("\u{0}adopted") {
        css.push_str(adopted);
    }
    let t_css = t0.elapsed();
    // Identity of the collected css bytes PLUS everything else that shapes
    // the parsed rules (media emulation re-filters @media arms with the
    // bytes unchanged, viewport re-folds vw/vh): unchanged key + tracked
    // DOM mutations lets the rule match sets sync incrementally instead of
    // re-matching the whole sheet (#111 — kissy's mount loop re-ran the
    // 17k-rule match six times in one React commit).
    let css_key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        css.hash(&mut hasher);
        format!(
            "{:?}|{:?}|{:?}",
            gs.viewport, gs.media_type, gs.media_overrides
        )
        .hash(&mut hasher);
        hasher.finish()
    };
    let (mut rules, keyframes, containers) = diting_css::parse_stylesheet_full(
        &css,
        (viewport_width, viewport_height),
        gs.media_type,
        &gs.media_overrides,
    );
    let t_parse = t0.elapsed();
    let mut styles_map = diting_layout::compute_styles_timed(
        dom,
        &rules,
        &keyframes,
        gs.css_time,
        &gs.css_transitions.borrow(),
        (viewport_width, viewport_height),
        Some(css_key),
    );
    // @container stage (moli#282): conditions answer against ancestor
    // container geometry, which only exists after a solve — so the arms
    // cascade in a second gated pass. Pass 1 styled the base rules; probe
    // boxes come from the cached solve when it is fresh (any previous run's
    // final geometry is a fine container-size estimate) and from one extra
    // solve otherwise. Pages without @container skip all of this.
    let mut styles_bumped = false;
    if !containers.is_empty() {
        let base_len = rules.len();
        let probe_boxes = match gs.geometry_cache.borrow().as_ref() {
            Some((e, solved)) if *e == dom.epoch() => solved.dom_boxes(),
            _ => {
                let fonts_probe = diting_layout::diting_fonts::font_book();
                let bytes_map = gs.image_bytes.borrow();
                let network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>> =
                    if bytes_map.is_empty() {
                        None
                    } else {
                        Some(&bytes_map)
                    };
                let probe = diting_layout::layout_solve(
                    dom,
                    &styles_map,
                    &fonts_probe,
                    viewport_width,
                    viewport_height,
                    network_bytes,
                    Some(gs.url.as_str()),
                );
                probe.dom_boxes()
            }
        };
        let plan =
            diting_layout::container_plan(dom, &styles_map, &probe_boxes, &containers, base_len);
        if !plan.extra_rules.is_empty() {
            rules.extend(plan.extra_rules);
            styles_map = diting_layout::compute_styles_gated(
                dom,
                &rules,
                &keyframes,
                gs.css_time,
                &gs.css_transitions.borrow(),
                base_len,
                &plan.gates,
                (viewport_width, viewport_height),
            );
            styles_bumped = true;
        }
    }
    // Refresh the attribute-selector name pool the write path consults for
    // inert-attribute invalidation skips (obscura#983). Sits after the
    // container stage so @container inner rules join the pool too.
    *gs.attr_selector_names.borrow_mut() = Some(diting_css::collect_selector_attr_names(&rules));
    let t_styles = t0.elapsed();
    // Finite animations span delay + duration * iterations. An endless one
    // can't pin a length on its own, so it contributes nothing unless it is
    // the only animation — then one full cycle keeps the extent the
    // single-cycle sampler used to report (a -t pin overrides anyway).
    // Registered transitions join the extent: a clip must run long enough
    // to show them finish.
    let mut css_extent = 0.0f64;
    let mut endless_cycle = 0.0f64;
    for a in styles_map.values().filter_map(|cs| cs.animation.as_ref()) {
        if a.iterations.is_finite() {
            css_extent = css_extent.max((a.delay + a.duration * a.iterations) as f64);
        } else {
            endless_cycle = endless_cycle.max((a.delay + a.duration) as f64);
        }
    }
    for tr in gs.css_transitions.borrow().iter() {
        css_extent = css_extent.max(tr.start + (tr.delay + tr.duration) as f64);
    }
    if css_extent <= 0.0 {
        css_extent = endless_cycle;
    }
    gs.css_extent.set(css_extent);
    let fonts = diting_layout::diting_fonts::font_book();
    // Solve-vs-collect split (#395): the taffy solve is cached keyed by the
    // tree epoch. A paint-only style write (transform/opacity — the choke
    // point in op_dom_inner) dropped the collected cache but kept this one,
    // so the run below re-collects against the cached tree. Anything
    // structural (tree mutation, class change, image bytes landing,
    // viewport move) drops both caches and re-solves.
    let epoch = dom.epoch();
    // A gated container re-cascade changed styles without bumping the epoch:
    // the cached solve (if any) was built against pass-1 styles, so it must
    // not be reused — force the fresh solve against the final cascade.
    let reuse = !styles_bumped
        && gs
            .geometry_cache
            .borrow()
            .as_ref()
            .is_some_and(|(e, _)| *e == epoch);
    let solve_src = if reuse { "cached" } else { "full" };
    let (rects, items, paint_order, local_geom, sticky_spans, scroller_spans) = if reuse {
        // Borrow held only across layout_collect, which never touches
        // JsState — nothing else can interleave on this single thread.
        let guard = gs.geometry_cache.borrow();
        let (_, solved) = guard.as_ref().expect("freshness checked above");
        let canvases = gs.canvas_images.borrow();
        diting_layout::layout_collect_with_images(
            dom,
            &styles_map,
            &fonts,
            solved,
            viewport_width,
            Some(&canvases),
        )
    } else {
        // The byte table the run resolves http(s) img sources against.
        // Empty → None keeps the all-placeholder path byte-identical to
        // before (and lets the caller decide when rasters are worth
        // fetching).
        let bytes_map = gs.image_bytes.borrow();
        let network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>> =
            if bytes_map.is_empty() {
                None
            } else {
                Some(&bytes_map)
            };
        let solved = diting_layout::layout_solve(
            dom,
            &styles_map,
            &fonts,
            viewport_width,
            viewport_height,
            network_bytes,
            Some(gs.url.as_str()),
        );
        drop(bytes_map);
        gs.solves.set(gs.solves.get() + 1);
        let canvases = gs.canvas_images.borrow();
        let run = diting_layout::layout_collect_with_images(
            dom,
            &styles_map,
            &fonts,
            &solved,
            viewport_width,
            Some(&canvases),
        );
        drop(canvases);
        *gs.geometry_cache.borrow_mut() = Some((epoch, solved));
        run
    };
    if trace {
        let t_layout = t0.elapsed();
        eprintln!(
            "[layout-trace] total={:?} solve={} css_collect={:?} css_parse={:?} compute_styles={:?} layout={:?} css_bytes={} rects={}",
            t_layout,
            solve_src,
            t_css,
            t_parse - t_css,
            t_styles - t_parse,
            t_layout - t_styles,
            css.len(),
            rects.len(),
        );
    }
    (
        rects
            .into_iter()
            .map(|(id, r)| (id, [r.x, r.y, r.width, r.height]))
            .collect(),
        paint_order,
        styles_map,
        items,
        local_geom
            .into_iter()
            .map(|(id, (r, m))| (id, ([r.x, r.y, r.width, r.height], m)))
            .collect(),
        sticky_spans,
        scroller_spans,
    )
}

/// Run one layout pass only when the memoized run is stale for the tree's
/// current epoch; callers then read their slice from the (now fresh) cache.
///
/// Freshness must be checked separately from membership: a nid absent from
/// the rects map is a legitimate miss on a FRESH cache (boxless element —
/// <head>, display:none, svg children under the svg v1 one-box model).
/// Treating "absent" as "stale" re-ran the whole page layout per boxless
/// lookup; on svg-heavy pages (archify artifacts: ~530 boxless svg nodes) a
/// single elementFromPoint walk re-laid-out the page hundreds of times and
/// tripped the 10s eval watchdog — surfacing as silent `null` eval results
/// and dead clicks.
#[cfg(feature = "screenshot")]
pub(super) fn ensure_layout_run(gs: &JsState, dom: &DomTree, epoch: u64) {
    let stale = gs.layout_cache.borrow().as_ref().map(|(e, _)| *e == epoch) != Some(true);
    if stale {
        let run = layout_run_all(gs, dom);
        *gs.layout_cache.borrow_mut() = Some((epoch, run));
    }
}

/// Parse the "w,h" viewport the JS side measured for the iframe element on
/// the host page (arg2 of the iframe layout ops). Anything unparseable or
/// non-positive falls back to the CSS default replaced box 300x150 — the
/// same box the fabricated `_IframeWindow` publishes before layout exists.
#[cfg(feature = "screenshot")]
pub(super) fn parse_iframe_viewport(arg2: &str) -> (f32, f32) {
    let mut it = arg2.split(',');
    let w = it.next().and_then(|s| s.parse::<f32>().ok());
    let h = it.next().and_then(|s| s.parse::<f32>().ok());
    match (w, h) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => (w, h),
        _ => (300.0, 150.0),
    }
}

/// One style+layout run over an ORPHAN SUBTREE — a fabricated iframe
/// document (obscura #976 family), memoized per (epoch, root, viewport) in
/// [`JsState::iframe_layout_cache`]. The sub-run anchors to the iframe
/// element's laid-out size on the host page (the iframe's own viewport, per
/// Chrome's gBCR semantics — the JS side passes it through arg2; the 300x150
/// default box stands when the host layout has no box for the element) and
/// never touches the main page's layout caches. Inline <style> sheets only
/// (v1 boundary): external <link> sheets inside a fabricated iframe doc
/// would need the navigation-time fetch cascade the main document owns.
#[cfg(feature = "screenshot")]
pub(super) fn iframe_layout_run(
    gs: &JsState,
    dom: &DomTree,
    root: NodeId,
    vw: f32,
    vh: f32,
) -> std::rc::Rc<LayoutRun> {
    let epoch = dom.epoch();
    let (vwb, vhb) = (vw.to_bits(), vh.to_bits());
    if let Some(hit) = gs
        .iframe_layout_cache
        .borrow()
        .as_ref()
        .and_then(|(e, r, cw, ch, run)| {
            (*e == epoch && *r == root && *cw == vwb && *ch == vhb).then(|| std::rc::Rc::clone(run))
        })
    {
        return hit;
    }
    let mut css = String::new();
    let mut stack = dom.children(root);
    while let Some(cur) = stack.pop() {
        stack.extend(dom.children(cur));
        let is_style = dom
            .with_node(cur, |n| {
                n.as_element()
                    .map(|e| *e.local.to_ascii_lowercase() == *"style")
            })
            .flatten()
            .unwrap_or(false);
        if is_style {
            css.push_str(&dom.text_content(cur));
            css.push('\n');
        }
    }
    let (rules, keyframes) =
        diting_css::parse_stylesheet_timed_with(&css, (vw, vh), gs.media_type, &gs.media_overrides);
    let styles_map = diting_layout::compute_styles_timed_within(
        dom,
        &rules,
        &keyframes,
        None,
        root,
        // Transitions are registered against main-document node ids; the
        // subtree id space here is separate, so the empty slice stands.
        &[],
        (vw, vh),
    );
    let fonts = diting_layout::diting_fonts::font_book();
    let solved = diting_layout::layout_solve_rooted(
        dom,
        &styles_map,
        &fonts,
        vw,
        vh,
        None,
        None,
        Some(root),
    );
    let canvases = gs.canvas_images.borrow();
    let (rects, items, paint_order, local_geom, sticky_spans, scroller_spans) =
        diting_layout::layout_collect_with_images(
            dom,
            &styles_map,
            &fonts,
            &solved,
            vw,
            Some(&canvases),
        );
    drop(canvases);
    let run = std::rc::Rc::new((
        rects
            .into_iter()
            .map(|(id, r)| (id, [r.x, r.y, r.width, r.height]))
            .collect(),
        paint_order,
        styles_map,
        items,
        local_geom
            .into_iter()
            .map(|(id, (r, m))| (id, ([r.x, r.y, r.width, r.height], m)))
            .collect(),
        sticky_spans,
        scroller_spans,
    ));
    *gs.iframe_layout_cache.borrow_mut() = Some((epoch, root, vwb, vhb, std::rc::Rc::clone(&run)));
    run
}

/// Whether `node` is a position:fixed box whose containing block is the
/// VIEWPORT — no ancestor between it and `root` carries a transform. CSS
/// transforms establish a new containing block for fixed descendants, so a
/// fixed box under a transformed ancestor re-anchors into that subtree and
/// DOES contribute to its scrollable overflow; only the viewport-anchored
/// kind is exempt (blitz#841: upstream's viewport became scrollable because
/// a translated fixed element entered the overflow walk).
#[cfg(feature = "screenshot")]
pub(super) fn is_viewport_fixed(
    dom: &DomTree,
    styles: &HashMap<NodeId, diting_css::ComputedStyle>,
    node: NodeId,
    root: NodeId,
) -> bool {
    if !styles
        .get(&node)
        .is_some_and(|st| st.position == Some(diting_css::PositionMode::Fixed))
    {
        return false;
    }
    let mut cur = dom.get_node(node).and_then(|n| n.parent);
    while let Some(id) = cur {
        if id == root {
            return true;
        }
        if styles
            .get(&id)
            .is_some_and(|st| st.effective_transform().is_some())
        {
            return false;
        }
        cur = dom.get_node(id).and_then(|n| n.parent);
    }
    true
}

/// A real element scroll container for sticky v2: boxed, clipping,
/// scrollable used overflow (hidden IS a scroll container per css-overflow,
/// clip is not), and not one the viewport owns by propagation — html's
/// overflow always propagates to the viewport, body's when html is visible
/// (body_overflow_propagates, the same predicate the clip-emission walk
/// used, so the walks agree on what the viewport owns). Shared by the
/// `is_scroll_container` op and the sticky_shifts source filter: what JS
/// may write and what paint consumes cannot drift apart.
#[cfg(feature = "screenshot")]
pub(super) fn is_element_scroller(
    dom: &DomTree,
    rects: &HashMap<NodeId, [f32; 4]>,
    styles: &HashMap<NodeId, diting_css::ComputedStyle>,
    id: NodeId,
) -> bool {
    if !rects.contains_key(&id) {
        return false;
    }
    let Some(s) = styles.get(&id) else {
        return false;
    };
    if !(s.clips_descendants() && s.effective_overflow() != diting_css::Overflow::Clip) {
        return false;
    }
    let tag = dom
        .with_node(id, |n| {
            n.as_element()
                .map(|e| e.local.to_ascii_lowercase().to_string())
        })
        .flatten();
    match tag.as_deref() {
        Some("html") => false,
        Some("body") => {
            !diting_layout::body_overflow_propagates(dom, styles, id, |x| rects.contains_key(&x))
        }
        _ => true,
    }
}

/// The sticky shift map for the CURRENT read (sticky v2): one [dx, dy]
/// per node inside a shifted subtree — sticky boxes AND element-scroller
/// contents. A sticky's value is its constraint shift composed over
/// enclosing sources; a scroller's descendants carry the negated scroll
/// offset the same way. Constraints are computed HERE, not at layout
/// time: they depend on the scroll offsets, which are orthogonal to the
/// layout epoch (scroll-blind layout — the same invariant that lets the
/// band pump re-blit without re-solving). Memoized per (epoch,
/// layout_rev, root scroll, viewport, scroll_gen) in
/// [`JsState::sticky_shift_cache`].
///
/// Sources — sticky nodes and scrolled element scrollers — are processed
/// SHALLOW-FIRST with one composition rule: a source's total is its own
/// contribution plus whatever shallower sources already wrote onto it
/// (out[nid] IS the nearest enclosing source's total, because every
/// shallower subtree write covered this node). Each source then writes
/// its folded total across its subtree, which composes
/// sticky-in-scroller, scroller-in-sticky, and a node that is BOTH — its
/// sticky branch runs first (covers self), its scroller branch bases on
/// that and rewrites only the descendants, exactly the two nested paint
/// spans its element records.
///
/// Gates (documented Chrome divergences, each degrades to plain relative
/// or unscrolled paint):
/// - A source under a position:fixed or transformed ANCESTOR gets no
///   shift: the fixed subtree has no scroller relation, and a
///   transformed ancestor brackets its subtree's paint in local coords,
///   where a uniform document-space translate can't compose from inside
///   the span. This also gates a transformed element scroller and sticky
///   inside one (accepted v2 approximation). The source's OWN transform
///   is fine — its bracket sits inside its span and
///   apply_sticky_to_items folds the shift into it.
/// - Sticky scrollport = the nearest element scroller's padding box plus
///   that scroller's OWN offset (outer scrollers cancel: port and
///   content ride them together); with none, the root scroller /
///   viewport (v1). html/body never count as element scrollers — their
///   overflow propagates to the viewport (css-overflow §3.3;
///   body_overflow_propagates is the same predicate the clip-emission
///   walk uses, so the two walks agree on what the viewport owns).
/// - Containing block = nearest boxed ancestor's border box (Blink uses
///   margin-box/content-box refinements).
/// - Boxless sticky (display:contents, inline wrappers) is skipped.
#[cfg(feature = "screenshot")]
pub(super) fn sticky_shifts(gs: &JsState, dom: &DomTree) -> std::rc::Rc<HashMap<NodeId, [f32; 2]>> {
    use diting_css::{Length, PositionMode};

    let epoch = dom.epoch();
    let (sx, sy) = gs.scroll_offset;
    let (vw, vh) = gs.viewport;
    // scroll_gen joins the key: element-scroller offsets change WITHOUT an
    // epoch or layout_rev bump (read-time paint state), so without it a
    // scrollTop write would keep serving the stale map.
    let key = (epoch, gs.layout_rev.get(), sx, sy, vw, vh, dom.scroll_gen());
    if let Some((k, v)) = gs.sticky_shift_cache.borrow().as_ref() {
        if *k == key {
            return std::rc::Rc::clone(v);
        }
    }
    ensure_layout_run(gs, dom, epoch);
    let guard = gs.layout_cache.borrow();
    let empty = std::rc::Rc::new(HashMap::new());
    let Some((_, (rects, _, styles, _, _, _, _))) = guard.as_ref().filter(|(e, _)| *e == epoch)
    else {
        return empty;
    };

    // Unified shift sources: (node, is_scroller). Sticky entries are
    // appended before scroller entries, so at equal depth a node that is
    // both runs its sticky branch first — the composition order its two
    // nested paint spans resolve in. NO dedup: dedup would drop the
    // second entry of a both-node and lose its scroll.
    let scrolls = dom.scroll_offsets();
    let mut sources: Vec<(NodeId, bool)> = styles
        .iter()
        .filter(|(_, s)| s.position == Some(PositionMode::Sticky))
        .map(|(n, _)| (*n, false))
        .filter(|(n, _)| rects.contains_key(n))
        .collect();
    sources.extend(
        scrolls
            .iter()
            .filter(|(_n, o)| o[0] != 0.0 || o[1] != 0.0)
            .map(|(n, _)| (*n, true))
            .filter(|(n, _)| is_element_scroller(dom, rects, styles, *n)),
    );
    sources.sort_by_key(|(n, _)| {
        let mut d = 0usize;
        let mut cur = dom.get_node(*n).and_then(|nd| nd.parent);
        while let Some(p) = cur {
            d += 1;
            cur = dom.get_node(p).and_then(|nd| nd.parent);
        }
        d
    });

    let resolve =
        |l: &Option<Length>, port: f32| l.as_ref().map(|l| diting_layout::sticky_inset_px(l, port));
    // Border widths count only with a border-style — the clip walk's
    // side_px rule; the scrollport is the padding box.
    let side_px = |l: &Option<Length>| match l {
        Some(Length::Px(v)) => *v,
        _ => 0.0,
    };

    let mut out: HashMap<NodeId, [f32; 2]> = HashMap::new();
    for (nid, scroller_source) in sources {
        // One ancestor walk, three uses: gate on fixed/transformed (no
        // shift), first boxed ancestor's border box (the CB), nearest
        // element scroller (the sticky's scrollport). The gate must see
        // the WHOLE chain — a transform above the CB still brackets the
        // paint.
        let mut gated = false;
        let mut cb: Option<(NodeId, [f32; 4])> = None;
        let mut port: Option<([f32; 4], [f32; 2])> = None;
        let mut cur = dom.get_node(nid).and_then(|nd| nd.parent);
        while let Some(p) = cur {
            if let Some(st) = styles.get(&p) {
                if st.position == Some(PositionMode::Fixed) || st.effective_transform().is_some() {
                    gated = true;
                    break;
                }
            }
            if cb.is_none() {
                // Table-internal stand-in boxes never establish the sticky
                // containing block: per CSS2.1 the CB of a table row (and
                // of a cell) is the TABLE, so row groups (thead/tbody/
                // tfoot — UA-mapped to TableRow, same as tr) and rows are
                // walked past, not clamped to. Without the skip a sticky
                // tr clamps to its 30px-tall thead box and never moves.
                let table_internal =
                    styles.get(&p).and_then(|st| st.display) == Some(diting_css::Display::TableRow);
                if !table_internal {
                    if let Some(&r) = rects.get(&p) {
                        cb = Some((p, r));
                    }
                }
            }
            if port.is_none() && is_element_scroller(dom, rects, styles, p) {
                let [px, py, pw, ph] = rects[&p];
                let st = &styles[&p];
                // Padding box = border box inset by border widths, which
                // apply only with a border-style.
                let (bt, br, bb, bl) = if st.border_style.is_some() {
                    (
                        side_px(&st.border_width.top),
                        side_px(&st.border_width.right),
                        side_px(&st.border_width.bottom),
                        side_px(&st.border_width.left),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };
                port = Some((
                    [
                        px + bl,
                        py + bt,
                        (pw - bl - br).max(0.0),
                        (ph - bt - bb).max(0.0),
                    ],
                    scrolls.get(&p).copied().unwrap_or([0.0, 0.0]),
                ));
            }
            cur = dom.get_node(p).and_then(|nd| nd.parent);
        }
        if gated {
            continue;
        }
        // A scroll container's rect in the map is its CLIP box, but a
        // sticky inside it travels the whole scrollable content — the
        // clamp range is the subtree union, the same extent scrollHeight
        // reports. v1 never noticed: at the root the body box already
        // spans the full document, so union == rect there.
        if let Some((cb_node, cb_rect)) = cb.as_mut() {
            if is_element_scroller(dom, rects, styles, *cb_node) {
                let [bx, by, _, _] = *cb_rect;
                let mut stack = dom.children(*cb_node);
                while let Some(c) = stack.pop() {
                    if let Some(&[rx, ry, rw, rh]) = rects.get(&c) {
                        cb_rect[2] = cb_rect[2].max(rx + rw - bx);
                        cb_rect[3] = cb_rect[3].max(ry + rh - by);
                    }
                    stack.extend(dom.children(c));
                }
            }
        }
        // Base = the total already written on this node by shallower
        // sources; out[nid] is the nearest enclosing source's total
        // because every shallower subtree write covered this node.
        let base = out.get(&nid).copied().unwrap_or([0.0, 0.0]);
        let (own, covers_self) = if scroller_source {
            // Content translates UP by the scroll offset; the scroller's
            // own box/border/clip stay fixed (its span opens after its
            // clip, the map write skips the node itself).
            let o = scrolls.get(&nid).copied().unwrap_or([0.0, 0.0]);
            ([-o[0], -o[1]], false)
        } else {
            let Some(&[x, y, w, h]) = rects.get(&nid) else {
                continue;
            };
            let Some((_, [cx, cy, cw, ch])) = cb else {
                continue;
            };
            let st = &styles[&nid];
            // Port: nearest element scroller (v2) with its OWN offset —
            // outer scrollers cancel because the port rides them with the
            // content. No scroller ancestor: the root scroller (v1).
            // (py + oy) against the UNSCROLLED y is the pinned read: the
            // port rides every outer source with the content, so only the
            // port's own scroll distinguishes them.
            let (dy, dx) = if let Some(([px, py, pw, ph], [ox, oy])) = port {
                (
                    diting_layout::sticky_axis_shift(
                        y,
                        h,
                        resolve(&st.top, ph),
                        resolve(&st.bottom, ph),
                        py + oy,
                        ph,
                        cy,
                        cy + ch,
                    ),
                    diting_layout::sticky_axis_shift(
                        x,
                        w,
                        resolve(&st.left, pw),
                        resolve(&st.right, pw),
                        px + ox,
                        pw,
                        cx,
                        cx + cw,
                    ),
                )
            } else {
                (
                    diting_layout::sticky_axis_shift(
                        y,
                        h,
                        resolve(&st.top, vh),
                        resolve(&st.bottom, vh),
                        sy,
                        vh,
                        cy,
                        cy + ch,
                    ),
                    diting_layout::sticky_axis_shift(
                        x,
                        w,
                        resolve(&st.left, vw),
                        resolve(&st.right, vw),
                        sx,
                        vw,
                        cx,
                        cx + cw,
                    ),
                )
            };
            if dx == 0.0 && dy == 0.0 {
                // Still in flow at this scroll — descendants read no base
                // shift from us, and this subtree keeps any ancestor total.
                continue;
            }
            ([dx, dy], true)
        };
        let total = [own[0] + base[0], own[1] + base[1]];
        // Write the total over the subtree (self included for a sticky —
        // its own box moves; excluded for a scroller — only its content
        // does). Every read point (gBCR, hit testing, band paint) shifts
        // uniformly; a deeper source later overwrites its own subtree
        // with its total, which already folded this one via the base
        // lookup above.
        let mut stack = if covers_self {
            vec![nid]
        } else {
            dom.children(nid)
        };
        while let Some(cur) = stack.pop() {
            out.insert(cur, total);
            stack.extend(dom.children(cur));
        }
    }
    drop(guard);
    let rc = std::rc::Rc::new(out);
    *gs.sticky_shift_cache.borrow_mut() = Some((key, std::rc::Rc::clone(&rc)));
    rc
}

#[cfg(feature = "screenshot")]
/// The page's text-ink extent alone ((width, height) in CSS px), from the
/// same memoized layout run every band paint shares. For bare-text bodies —
/// no element boxes past the stretched html/body — this is the only true
/// extent; the print pump reads it as its content-height fallback when
/// scrollHeight sits at the viewport clamp.
#[cfg(feature = "screenshot")]
pub fn text_ink_extent(gs: &JsState) -> Option<(f32, f32)> {
    let dom = gs.dom.as_ref()?;
    let epoch = dom.epoch();
    ensure_layout_run(gs, dom, epoch);
    let guard = gs.layout_cache.borrow();
    let (_, (_, _, _, items, _, _, _)) = guard.as_ref().filter(|(e, _)| *e == epoch)?;
    Some(diting_layout::paint::text_ink_extent(items))
}

