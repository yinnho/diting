// dom_cmd 样式三族：sheets/geometry/scroll_style + style 写路径判定 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// `op_dom` family: external stylesheets, resource timings, CSS rule parsing.
pub(super) fn dom_cmd_sheets(gs: &JsState, _dom: &DomTree, cmd: &str, arg1: String, arg2: String) -> DomCmd {
    Ok(match cmd {
        // External stylesheet bodies (absolute URL → CSS text) fetched at
        // navigation. document.styleSheets reads this to build real
        // CSSStyleSheet entries for <link rel=stylesheet> elements — the
        // fetch itself lives in Page's navigation pipeline, this is the
        // retention side.
        "ext_sheets" => {
            let map = gs.ext_sheets.borrow();
            let mut obj = serde_json::Map::with_capacity(map.len());
            for (k, v) in map.iter() {
                obj.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            serde_json::Value::Object(obj).to_string()
        }
        // Real per-request resource timings (#126): epoch-stamped fetch
        // records from the navigation pipeline. The JS side converts them
        // into the resource clock and merges with its own fetch/XHR
        // entries; requests without a real measurement never appear.
        "resource_timings" => {
            let list = gs.resource_timings.borrow();
            serde_json::to_string(
                &list
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "name": r.name,
                            "initiatorType": r.initiator,
                            "start": r.start_epoch_ms,
                            "duration": r.duration_ms,
                            "status": r.status,
                            "bodySize": r.body_size,
                            "renderBlocking": r.render_blocking,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[]".to_string())
        }
        // clearResourceTimings(): the buffer spans BOTH faces (Rust-recorded
        // subresources + JS-recorded fetch/XHR), so the clear crosses too.
        "resource_timings_clear" => {
            gs.resource_timings.borrow_mut().clear();
            "true".to_string()
        }
        // Stylesheet body for a link that entered the live document (or
        // re-href'd inside it) after navigation — fetched by the bootstrap's
        // dynamic-link loader and parked here so both layout_run_all's
        // cascade join and document.styleSheets see it. The layout cache
        // must drop: a new sheet can flip any rule match, which the tree
        // epoch cannot express (same reasoning as the attribute-write
        // commands at the top of this dispatcher).
        "ext_sheet_put" => {
            gs.ext_sheets.borrow_mut().insert(arg1, arg2);
            #[cfg(feature = "screenshot")]
            {
                gs.drop_layout();
            }
            "null".to_string()
        }
        // Parse CSS text into rule records for CSSOM: real selector +
        // declaration text from the same parser the cascade uses, so
        // sheet.cssRules agrees with what layout actually applies.
        "parse_css_rules" => {
            let rules = diting_css::parse_stylesheet(&arg1);
            let arr: Vec<serde_json::Value> = rules
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "selectorText": r.selector,
                        "cssText": format!("{}{{{}}}", r.selector, r.declarations),
                        "declarations": r.declarations,
                    })
                })
                .collect();
            serde_json::Value::Array(arr).to_string()
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: layout geometry: rects, paint order, local geometry, event offsets.
pub(super) fn dom_cmd_geometry(gs: &JsState, dom: &DomTree, cmd: &str, arg1: String, arg2: String) -> DomCmd {
    Ok(match cmd {
        "text_rects" | "elem_rects" => text_geom::run(gs, dom, cmd, &arg1, &arg2),
        // Real layout geometry for one element, from the diting_css +
        // diting_layout pipeline. Memoized per tree epoch; a stale epoch
        // (any node allocation/free since the last run) re-lays-out. Returns
        // "[x,y,width,height]" or "null". Gated behind the screenshot
        // feature because diting_layout pulls taffy/swash; without it the
        // bootstrap falls back to its synthetic hit-test grid.
        #[cfg(feature = "screenshot")]
        "layout_rect" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let rect = gs.layout_cache.borrow().as_ref().and_then(|(e, (m, ..))| {
                if *e == epoch {
                    m.get(&nid).copied()
                } else {
                    None
                }
            });
            match rect {
                // Sticky v1: the border-box rect carries the scroll-dependent
                // shift so gBCR/offsetTop track the visual position (the
                // same in-flow + sticky pairing the paint side applies).
                // Plain elements read a zero entry — bytes unchanged.
                Some([x, y, w, h]) => {
                    let sh = *sticky_shifts(gs, dom).get(&nid).unwrap_or(&[0.0, 0.0]);
                    format!("[{},{},{},{}]", x + sh[0], y + sh[1], w, h)
                }
                None => {
                    // A valid layout run for this epoch says the element has
                    // NO box (display:none, detached, or composed-tree-hidden:
                    // an unslotted light child / a slot element) — Chrome
                    // answers an all-zero DOMRect there. "null" stays reserved
                    // for an invalid nid or a failed layout run, the cases
                    // where bootstrap's synthetic grid is the fallback.
                    if gs
                        .layout_cache
                        .borrow()
                        .as_ref()
                        .is_some_and(|(e, _)| *e == epoch)
                    {
                        "[0,0,0,0]".into()
                    } else {
                        "null".into()
                    }
                }
            }
        }
        // Elements in paint order (ascending), from the same layout run as
        // `layout_rect`. Backs elementFromPoint: document order is not paint
        // order once positioned z-index siblings hoist out of it (obscura
        // #738). Boxless inline wrappers are absent — the JS side ranks
        // them with their nearest boxed ancestor.
        #[cfg(feature = "screenshot")]
        "paint_order" => {
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let order = gs
                .layout_cache
                .borrow()
                .as_ref()
                .and_then(
                    |(e, (_, o, ..))| {
                        if *e == epoch {
                            Some(o.clone())
                        } else {
                            None
                        }
                    },
                );
            let order = order.unwrap_or_default();
            let mut s = String::with_capacity(order.len() * 8 + 2);
            s.push('[');
            for (i, id) in order.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&id.index().to_string());
            }
            s.push(']');
            s
        }
        // Per-element LOCAL geometry (blitz #663 family): the pre-map
        // border box and the element's TOTAL accumulated paint map as
        // "[x,y,w,h,a,b,c,d,e,f]", or "null" for a boxless element or a
        // failed run. Backs offsetX/Y inverse mapping and exact-shape hit
        // testing — the JS side rejects hit points that fall inside the
        // element's axis-aligned bounding box but outside the true
        // transformed shape.
        #[cfg(feature = "screenshot")]
        "local_geom" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let guard = gs.layout_cache.borrow();
            let geom = guard.as_ref().and_then(|(e, run)| {
                if *e == epoch {
                    run.4.get(&nid).copied()
                } else {
                    None
                }
            });
            drop(guard);
            match geom {
                // Sticky v1: shift the border box and prepend the shift onto
                // the TOTAL map's e/f (T·M — a document-space translate, the
                // same fold the paint side composes into a bracket), so
                // offset inverse-mapping and exact-shape hit tests land on
                // the visual geometry.
                Some(([x, y, w, h], [a, b, c, d, e, f])) => {
                    let sh = *sticky_shifts(gs, dom).get(&nid).unwrap_or(&[0.0, 0.0]);
                    format!(
                        "[{},{},{},{},{},{},{},{},{},{}]",
                        x + sh[0],
                        y + sh[1],
                        w,
                        h,
                        a,
                        b,
                        c,
                        d,
                        e + sh[0],
                        f + sh[1]
                    )
                }
                None => "null".into(),
            }
        }
        // getBoundingClientRect / getClientRects inside a fabricated iframe
        // document (obscura #976): a sync-created contentDocument is an
        // orphan Rust DOM subtree the main run's rect map never covers, so
        // the main-document `layout_rect` answered all zeros. This sub-run
        // answers in the IFRAME'S OWN viewport (the 300x150 default box the
        // fabricated _IframeWindow publishes), matching Chrome — which
        // measures iframe-doc elements against the iframe's viewport with no
        // coordinate stitching into the host page. arg1 must belong to an
        // orphan subtree; the walk reaching the document node means a
        // main-document element was misrouted here (the JS side only calls
        // this through the iframe-doc marker) and gets "null".
        #[cfg(feature = "screenshot")]
        "iframe_layout_rect" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            let mut root = nid;
            while let Some(p) = dom.get_node(root).and_then(|n| n.parent) {
                root = p;
            }
            if root == dom.document() {
                return Ok("null".into());
            }
            let (vw, vh) = parse_iframe_viewport(&arg2);
            let run = iframe_layout_run(gs, dom, root, vw, vh);
            match run.0.get(&nid) {
                Some(&[x, y, w, h]) => format!("[{},{},{},{}]", x, y, w, h),
                // A fresh run without the nid = boxless element in the iframe
                // doc (display:none, …) — the same all-zero DOMRect contract
                // the main-document `layout_rect` serves.
                None => "[0,0,0,0]".into(),
            }
        }
        // The event-coordinate surface (offsetX/Y inverse mapping, exact
        // hit shapes) inside a fabricated iframe document — same 10-tuple
        // wire format as `local_geom`, served from the iframe-subtree run.
        #[cfg(feature = "screenshot")]
        "iframe_local_geom" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            let mut root = nid;
            while let Some(p) = dom.get_node(root).and_then(|n| n.parent) {
                root = p;
            }
            if root == dom.document() {
                return Ok("null".into());
            }
            let (vw, vh) = parse_iframe_viewport(&arg2);
            let run = iframe_layout_run(gs, dom, root, vw, vh);
            match run.4.get(&nid) {
                Some(&([x, y, w, h], [a, b, c, d, e, f])) => {
                    format!(
                        "[{},{},{},{},{},{},{},{},{},{}]",
                        x, y, w, h, a, b, c, d, e, f
                    )
                }
                None => "null".into(),
            }
        }
        // offsetX/offsetY for a hit at a document-space point (arg2
        // "docX,docY" — client + scroll) on element arg1 (blitz #663
        // family): the point inverse-maps through the element's TOTAL
        // accumulated map into its local space, minus the padding-edge
        // origin. Singular or non-finite maps answer "null"; the JS side
        // keeps its constructor defaults there.
        #[cfg(feature = "screenshot")]
        "event_offset" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            let Some((px, py)) = arg2.split_once(',').and_then(|(xs, ys)| {
                let x = xs.trim().parse::<f32>().ok()?;
                let y = ys.trim().parse::<f32>().ok()?;
                Some((x, y))
            }) else {
                return Ok("null".into());
            };
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let guard = gs.layout_cache.borrow();
            let Some((_, run)) = guard.as_ref().filter(|(e, _)| *e == epoch) else {
                return Ok("null".into());
            };
            let Some(&([bx, by, _, _], [a, b, c, d, ex, ey])) = run.4.get(&nid) else {
                return Ok("null".into());
            };
            let det = a * d - b * c;
            if det.abs() < 1e-9 {
                return Ok("null".into());
            }
            let lx = (d * (px - ex) - c * (py - ey)) / det;
            let ly = (a * (py - ey) - b * (px - ex)) / det;
            if !lx.is_finite() || !ly.is_finite() {
                return Ok("null".into());
            }
            // Padding edge = border box origin + border + padding on each
            // axis. Px dominates click targets; percent/calc sides answer
            // 0 there (v1 boundary — those elements keep working, just
            // without the sub-box nudge).
            let edge = |len: &Option<diting_css::Length>| match len {
                Some(diting_css::Length::Px(v)) => *v,
                _ => 0.0,
            };
            if let Some(style) = run.2.get(&nid) {
                let ox = lx - bx - edge(&style.border_width.left) - edge(&style.padding.left);
                let oy = ly - by - edge(&style.border_width.top) - edge(&style.padding.top);
                format!("[{},{}]", ox, oy)
            } else {
                format!("[{},{}]", lx - bx, ly - by)
            }
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: scroll extents/containers and computed style.
#[cfg_attr(not(feature = "screenshot"), allow(unused_variables))] // only the null stubs remain
pub(super) fn dom_cmd_scroll_style(
    gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        // CSSOM scrollWidth/scrollHeight for one element as "[w,h]", from
        // the same layout run as `layout_rect`: the element's own box
        // unioned with every laid-out DOM descendant's overflow extent
        // relative to the element's origin (blitz #444 — scroll ranges that
        // ignore overflowing content clamp agents out of the bottom of the
        // page). html/body additionally clamp up to the viewport: Chrome's
        // scrolling area is never smaller than the window, while its
        // client* stay the viewport contract. Element boxes carry the
        // content extent (blocks include their text, abspos kids have their
        // own rects), so the element-rect walk approximates the CSSOM
        // scrollable overflow region without per-margin-box geometry.
        #[cfg(feature = "screenshot")]
        "scroll_extent" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let guard = gs.layout_cache.borrow();
            let Some((_, (rects, _, styles, items, _, _, _))) =
                guard.as_ref().filter(|(e, _)| *e == epoch)
            else {
                return Ok("null".into());
            };
            let Some(&[ox, oy, ow, oh]) = rects.get(&nid) else {
                return Ok("null".into());
            };
            let is_root = dom
                .with_node(nid, |n| {
                    n.as_element()
                        .map(|e| matches!(e.local.to_ascii_lowercase().as_str(), "html" | "body"))
                })
                .flatten()
                .unwrap_or(false);
            let mut max_w = ow;
            let mut max_h = oh;
            let mut stack = dom.children(nid);
            while let Some(cur) = stack.pop() {
                // blitz#841: a viewport-fixed box never contributes to any
                // scroller's overflow — its containing block is the viewport,
                // so its (possibly transformed) rect must stay out of the
                // union. The whole subtree skips: everything under it is
                // pinned with it.
                if is_viewport_fixed(dom, styles, cur, nid) {
                    continue;
                }
                stack.extend(dom.children(cur));
                if let Some(&[x, y, w, h]) = rects.get(&cur) {
                    max_w = max_w.max((x + w - ox).max(0.0));
                    max_h = max_h.max((y + h - oy).max(0.0));
                }
            }
            if is_root {
                let (vw, vh) = gs.viewport;
                max_w = max_w.max(vw);
                max_h = max_h.max(vh);
                // The element walk can't see bare text — html/body stretch
                // to the viewport, so a text-only body's ink never lifts the
                // union past vh and the page can't scroll past its first
                // screenful. Fold the Text items' wrap-model extent in (the
                // same fold `band_frame` applies, so JS scrollHeight and the
                // pump's clamp agree on the range).
                let (ink_w, ink_h) = diting_layout::paint::text_ink_extent(items);
                max_w = max_w.max(ink_w);
                max_h = max_h.max(ink_h);
            }
            // Viewport overflow propagation (blitz#880, css-overflow-3
            // §3.3): hidden/clip carried by the root element — its own or
            // handed up by the first body child — makes the viewport itself
            // unscrollable, so the scrolling area collapses to exactly the
            // viewport (Chrome: scrollingElement.scrollHeight ==
            // clientHeight there). Only the root-element query collapses;
            // body keeps its content extent (its used overflow became
            // `visible`). Same clamp `band_frame` applies, so the JS range
            // and the pump's stay in agreement.
            let is_html = dom
                .with_node(nid, |n| {
                    n.as_element()
                        .map(|e| e.local.to_ascii_lowercase().as_str() == "html")
                })
                .flatten()
                .unwrap_or(false);
            if is_html {
                let eff = diting_layout::effective_viewport_overflow(dom, styles, nid, |id| {
                    rects.contains_key(&id)
                });
                if matches!(
                    eff,
                    diting_css::Overflow::Hidden | diting_css::Overflow::Clip
                ) {
                    let (vw, vh) = gs.viewport;
                    max_w = vw;
                    max_h = vh;
                }
            }
            format!("[{},{}]", max_w, max_h)
        }
        // Sticky v2: gate for the bootstrap's scrollTop/scrollLeft setters —
        // is this element a real scroll container the shift walk will
        // honor? "1"/"0"; bare builds answer "null" (falls through to the
        // catch-all below) and the JS side treats that as not-a-scroller.
        #[cfg(feature = "screenshot")]
        "is_scroll_container" => {
            let Some(nid) = parse_nid(&arg1) else {
                return Ok("0".into());
            };
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let guard = gs.layout_cache.borrow();
            let Some((_, (rects, _, styles, _, _, _, _))) =
                guard.as_ref().filter(|(e, _)| *e == epoch)
            else {
                return Ok("0".into());
            };
            if is_element_scroller(dom, rects, styles, nid) {
                "1".into()
            } else {
                "0".into()
            }
        }
        // Sticky v2 write-through: record one element scroller's offset.
        // arg1 = nid, arg2 = "left\0top"; the JS setter clamped to the
        // scrollable range before calling. scroll_gen (not the epoch)
        // invalidates the shift caches, so scrolling never re-lays-out.
        #[cfg(feature = "screenshot")]
        "set_node_scroll" => {
            let Some(nid) = parse_nid(&arg1) else {
                return Ok("ok".into());
            };
            if let Some((x, y)) = arg2
                .split_once('\0')
                .and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))
            {
                if x.is_finite() && y.is_finite() {
                    dom.set_node_scroll(nid, x, y);
                }
            }
            "ok".into()
        }
        // Cascaded computed values for one element as a single JSON object,
        // from the same style+layout run as layout_rect — the layer
        // getComputedStyle was missing: only inline styles were consulted,
        // so values from a <style> block (z-index, position, …) read back
        // as the initial value (obscura #738's companion trap). Snapshot
        // shape follows upstream's op_computed_style: one call returns the
        // whole table, so a style object costs one native round-trip
        // instead of one per property. Covers the layout-decision
        // properties the ComputedStyle carries (inline style attr is
        // folded in by compute_styles); unknown properties stay absent and
        // the JS caller falls through to its own tables. Used value for
        // width/height stays the bounding rect's job, so those are
        // deliberately not in the table.
        #[cfg(feature = "screenshot")]
        "computed_style" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("null".into()),
            };
            // Pseudo-element face (batch 102): arg2 routes to the host's
            // cascaded ::before/::after styles. The JS wrapper validates the
            // argument (TypeError for anything not `::name` or a legacy
            // single-colon form), so anything non-empty reaching here answers
            // either a pseudo cascade or — for pseudos with no matching rule
            // and for `::name` forms the engine doesn't model — an
            // initial-value table, matching Chrome's no-throw posture.
            let pseudo_kind = match arg2.trim().to_ascii_lowercase().as_str() {
                "" => 0u8,
                "::before" | ":before" => 1,
                "::after" | ":after" => 2,
                _ => 3,
            };
            let cssom_tag = dom
                .with_node(nid, |n| {
                    n.as_element()
                        .map(|e| e.local.as_str().to_ascii_lowercase())
                })
                .flatten();
            let epoch = dom.epoch();
            ensure_layout_run(gs, dom, epoch);
            let style_and_cb = gs.layout_cache.borrow().as_ref().and_then(|(e, r)| {
                if *e != epoch {
                    return None;
                }
                let (rects, _, styles, _, _, _, _) = r;
                let style = styles.get(&nid).cloned();
                // #242: percentage padding/margin resolves to used px against
                // the containing block — Chrome's gCS posture. Pseudos share
                // the host's containing block.
                let cb = cb_inline_for(dom, rects, styles, nid, 8);
                style.map(|s| (s, cb))
            });
            match style_and_cb {
                Some((s, cb)) => {
                    // The pseudo cascade lives on the host's ComputedStyle;
                    // a pseudo with no rule serializes the initial-value
                    // table rather than an empty declaration.
                    let pseudo_style = match pseudo_kind {
                        1 => s.pseudos.as_ref().and_then(|p| p.before.as_ref()).cloned(),
                        2 => s.pseudos.as_ref().and_then(|p| p.after.as_ref()).cloned(),
                        _ => None,
                    };
                    let (target, tag) = match pseudo_style {
                        Some(ps) => (ps, None),
                        None if pseudo_kind == 0 => (s, cssom_tag),
                        None => (diting_css::ComputedStyle::default(), None),
                    };
                    let mut obj = serde_json::Map::with_capacity(
                        cssom_value::COMPUTED_STYLE_PROPS.len() + target.custom.len(),
                    );
                    for prop in cssom_value::COMPUTED_STYLE_PROPS {
                        if let Some(v) = computed_style_value(&target, prop, tag.as_deref(), cb) {
                            obj.insert((*prop).to_string(), serde_json::Value::String(v));
                        }
                    }
                    // Custom properties ride the same snapshot (case-sensitive
                    // keys — the JS lookup skips its kebab-lowercase step for
                    // `--` names): getComputedStyle(el).getPropertyValue('--x').
                    for (k, v) in target.custom.iter() {
                        obj.insert(k.clone(), serde_json::Value::String(v.clone()));
                    }
                    for (k, v) in diting_css::props::missing_initials(&target.custom) {
                        obj.insert(k, serde_json::Value::String(v));
                    }
                    serde_json::Value::Object(obj).to_string()
                }
                None => "null".into(),
            }
        }
        #[cfg(not(feature = "screenshot"))]
        "computed_style" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "layout_rect" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "paint_order" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "scroll_extent" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "local_geom" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "event_offset" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "iframe_layout_rect" => "null".into(),
        #[cfg(not(feature = "screenshot"))]
        "iframe_local_geom" => "null".into(),
        _ => return Err((arg1, arg2)),
    })
}

/// Inline style string as an effective property map (lowercased names,
/// trimmed values, last declaration wins; colon-less garbage stays raw).
#[cfg(feature = "screenshot")]
pub(super) fn style_property_map(style: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for decl in style.split(';') {
        let (n, v) = decl.split_once(':').unwrap_or((decl, ""));
        let n = n.trim().to_ascii_lowercase();
        if !n.is_empty() {
            map.insert(n, v.trim().to_string());
        }
    }
    map
}

/// Is a style-attribute write paint-only — every property whose effective
/// declaration differs (added, removed, or VALUE-changed) sits inside the
/// paint-only set {transform, opacity, translate, rotate, scale}? Those
/// feed no layout input, so the solve survives and only the collect half
/// re-runs. Anything else (padding, width, …) feeds the solve: a same-name
/// value rewrite used to pass a name-set diff vacuously and served the
/// stale solve (#220's two-rounds-different-heights); a missing side gets
/// the full drop.
#[cfg(feature = "screenshot")]
pub(super) fn style_write_is_paint_only(old: Option<&str>, new: Option<&str>) -> bool {
    const PAINT_ONLY: [&str; 5] = ["transform", "opacity", "translate", "rotate", "scale"];
    let (Some(old), Some(new)) = (old, new) else {
        return false;
    };
    let old = style_property_map(old);
    let new = style_property_map(new);
    old.keys()
        .chain(new.keys())
        .all(|n| old.get(n) == new.get(n) || PAINT_ONLY.contains(&n.as_str()))
}

/// Can an attribute write never change computed style or layout on its own?
/// The inert set is focus/a11y/script hooks (obscura#983); the escape hatch
/// is the rule pool — a stylesheet selecting on the name (`[data-x]`)
/// disqualifies it. `None` for the pool means no layout run has happened
/// yet, so no cache exists to invalidate and skipping is free.
#[cfg(feature = "screenshot")]
pub(super) fn attr_write_is_layout_inert(
    name: &str,
    selector_names: Option<&std::collections::HashSet<String>>,
) -> bool {
    let n = name.to_ascii_lowercase();
    let inert = matches!(
        n.as_str(),
        "tabindex" | "title" | "role" | "accesskey" | "draggable" | "spellcheck"
    ) || n.starts_with("data-")
        || n.starts_with("aria-");
    inert && !selector_names.is_some_and(|s| s.contains(n.as_str()))
}

