//! Collect half of a layout run, plus viewport overflow propagation.
//!
//! Split from the layout hub. The public entry points stay re-exported
//! from the crate root. Pure move: this walk sees the same crate
//! namespace it grew up in.

use super::*;

/// css-overflow-3 §3.3 overflow propagation (blitz#880): overflow on the
/// root element applies to the VIEWPORT itself; when that value is
/// `visible`, the first `body` child that generates a box hands ITS overflow
/// to the viewport instead (a `display:none` body generates no box and
/// propagates nothing). `hidden`/`clip` arriving there makes the page
/// unscrollable — the scrolling area collapses to exactly the viewport.
/// Every reader of the root scroll range (`scroll_extent`, `band_frame`)
/// must consult this or they disagree on the scrollable range.
pub fn effective_viewport_overflow(
    dom: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    root: NodeId,
    has_box: impl Fn(NodeId) -> bool,
) -> Overflow {
    let ov = |id: NodeId| {
        styles
            .get(&id)
            .map(|s| s.effective_overflow())
            .unwrap_or(Overflow::Visible)
    };
    let local = |id: NodeId| -> Option<String> {
        dom.get_node(id).and_then(|n| {
            n.as_element()
                .map(|e| e.local.to_ascii_lowercase().as_str().to_string())
        })
    };
    // The viewport-carrying element: a body query climbs to its parent (the
    // root element in a normal document); anything else is taken as-is.
    let root_elem = if local(root).as_deref() == Some("body") {
        dom.get_node(root).and_then(|n| n.parent).unwrap_or(root)
    } else {
        root
    };
    let root_ov = ov(root_elem);
    if root_ov != Overflow::Visible {
        return root_ov;
    }
    for child in dom.children(root_elem) {
        if local(child).as_deref() == Some("body") {
            // First body child only: no box → no propagation, and no
            // fallback to a later body (WPT overflow-body-propagation-016).
            return if has_box(child) { ov(child) } else { root_ov };
        }
    }
    root_ov
}

/// Whether `body`'s overflow propagated to the viewport (css-overflow-3
/// §3.3, blitz#880): its used value then flips back to `visible`, so the
/// body must not clip its own descendants in paint.
pub fn body_overflow_propagates(
    dom: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    body: NodeId,
    has_box: impl Fn(NodeId) -> bool,
) -> bool {
    let Some(html) = dom.get_node(body).and_then(|n| n.parent) else {
        return false;
    };
    let html_ov = styles
        .get(&html)
        .map(|s| s.effective_overflow())
        .unwrap_or(Overflow::Visible);
    if html_ov != Overflow::Visible {
        return false;
    }
    // Propagation only ever happens for the first body child of the root
    // element, and only when it generates a box and carries a non-visible
    // value (visible hands up nothing).
    let first_body = dom.children(html).into_iter().find(|c| {
        dom.get_node(*c)
            .and_then(|n| {
                n.as_element()
                    .map(|e| e.local.to_ascii_lowercase().as_str() == "body")
            })
            .unwrap_or(false)
    });
    first_body == Some(body)
        && has_box(body)
        && styles
            .get(&body)
            .map(|s| s.effective_overflow())
            .unwrap_or(Overflow::Visible)
            != Overflow::Visible
}

/// Re-run ONLY the collect half against a cached solve. Layout splits in
/// two halves for #395: the taffy solve (build, static-position harvest,
/// reparent, main solve, float continuation, calc repair, table placement)
/// is blind to paint-only properties — transform and opacity never reach
/// `to_taffy_style` — so this is the half a paint-only style write must
/// re-run, against the cached solve, instead of paying for a fresh taffy
/// pass (43 solves × 114ms on the probe page).
pub fn layout_collect(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    solved: &SolvedGeometry,
    viewport_width: f32,
) -> LayoutCollect {
    layout_collect_with_images(tree, styles, fonts, solved, viewport_width, None)
}

/// [`layout_collect`] with host-supplied replaced-element bitmaps layered
/// over the solve's own `<img>` map (canvas compositing, #210): the JS
/// realm owns canvas pixels, so the page host pushes them in per render
/// instead of the solve resolving them from resource bytes. Entries here
/// win over the same-nid img entry (a node is one or the other).
pub fn layout_collect_with_images(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    solved: &SolvedGeometry,
    viewport_width: f32,
    extra_images: Option<&HashMap<NodeId, DecodedImage>>,
) -> LayoutCollect {
    let mut rects = HashMap::new();
    let mut items: Vec<PaintItem> = Vec::new();
    // Paint sequence of the boxed elements (obscura #738): filled by the
    // `collect` walk, sibling bands already z-sorted. See the doc on
    // [`layout_dom_with_paint_order_and_images`].
    let mut paint_order: Vec<NodeId> = Vec::new();
    // Per-element LOCAL geometry for the event-coordinate surface (blitz
    // #663 family): the PRE-map border box plus the element's TOTAL
    // accumulated map (own linear part composed over ancestors — the same
    // map gBCR's `rects` entry was produced with). offsetX/Y inverse-maps
    // the hit point through it into the element's local space; hit testing
    // uses it to keep rotate/skew elements from swallowing corners of their
    // mapped bounding box. Keyed by DOM nid directly; the inline-band pass's
    // taffy-keyed `local_by_node` keeps its own shape untouched.
    let mut local_geom: HashMap<NodeId, (Rect, [f32; 6])> = HashMap::new();
    let mut sticky_spans: Vec<StickySpan> = Vec::new();
    // Scroller spans (sticky v2): one per real scroll container, recorded
    // unconditionally at layout time — scroll offsets are read-time paint
    // state that changes without a layout epoch bump, so the spans cannot
    // be gated on the current offset.
    let mut scroller_spans: Vec<StickySpan> = Vec::new();
    let Some(icb_node) = solved.icb_node else {
        return (
            rects,
            items,
            paint_order,
            local_geom,
            sticky_spans,
            scroller_spans,
        );
    };
    let _src_armed = text_src::arm(solved.text_src.clone());
    let SolvedGeometry {
        taffy_tree,
        node_map,
        images,
        static_pos,
        baseline_shifts,
        run_wrappers,
        flattened,
        collapsed_edges,
        ..
    } = solved;
    // Host-supplied bitmaps (#210) shadow the solve's img map by nid. The
    // merged map is owned here and lives to the end of the walk.
    let merged_images: Option<HashMap<NodeId, DecodedImage>> = extra_images.map(|extra| {
        let mut m: HashMap<NodeId, DecodedImage> =
            images.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (k, v) in extra.iter() {
            m.insert(*k, v.clone());
        }
        m
    });
    let images: &HashMap<NodeId, DecodedImage> = merged_images.as_ref().unwrap_or(images);

    let mut abs_by_node: HashMap<diting_taffy::tree::NodeId, Rect> = HashMap::new();
    let mut local_by_node: HashMap<diting_taffy::tree::NodeId, (Rect, [f32; 6])> = HashMap::new();
    let mut node_first_item: HashMap<diting_taffy::tree::NodeId, usize> = HashMap::new();
    // The ICB is not in node_map → always a barrier, so the deferred band
    // never survives the walk — asserted rather than assumed.
    let mut root_escapes: Vec<ZEscape> = Vec::new();
    {
        let cx = collect::Ctx {
            tree,
            taffy_tree,
            node_map,
            styles,
            fonts,
            images,
            static_pos,
            baseline_shifts,
            collapsed_edges,
            viewport_width,
        };
        let mut out = collect::Out {
            rects: &mut rects,
            local_geom: &mut local_geom,
            abs_by_node: &mut abs_by_node,
            local_by_node: &mut local_by_node,
            node_first_item: &mut node_first_item,
            sticky_spans: &mut sticky_spans,
            scroller_spans: &mut scroller_spans,
            items: &mut items,
            paint_order: &mut paint_order,
        };
        collect(
            &cx,
            &mut out,
            &mut root_escapes,
            icb_node,
            (0.0, 0.0),
            Xf::IDENTITY,
            1.0,
            None,
        );
    }
    debug_assert!(root_escapes.is_empty());
    // Flattened inline wrappers (span/label/a/… — obscura#722 lineage) own no
    // taffy box: the run hoisted their children. getBoundingClientRect still
    // owes them a rect, so union the hoisted kids' absolute boxes into one
    // bounding box. CSS unions the element's own fragments; kids approximate
    // that closely for text content (exact for the common cases).
    for (dom, kids) in flattened {
        if rects.contains_key(dom) {
            continue;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut any = false;
        for k in kids {
            let Some(r) = abs_by_node.get(k) else {
                continue;
            };
            any = true;
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x + r.width);
            max_y = max_y.max(r.y + r.height);
        }
        if any {
            // Strut (obscura#722 residual): the inline's own box carries its
            // line-height even when its content is shorter — a replaced-only
            // inline (`<a><img 8x8></a>`) or a smaller-font descendant unions
            // below the line box Chrome reports. Grow the union vertically to
            // the element's effective line-height, split like half-leading.
            // Sub-pixel shortfalls (< 1px) are run-metric rounding, not a
            // missing strut — growing there just pushes the first line's
            // inline above y=0, which Chrome never reports.
            let (_, _, lh) = font_context(tree, *dom, styles, fonts);
            let h = max_y - min_y;
            if lh - h > 1.0 {
                let grow = (lh - h) / 2.0;
                min_y -= grow;
                max_y += grow;
            }
            rects.insert(
                *dom,
                Rect {
                    x: min_x,
                    y: min_y,
                    width: max_x - min_x,
                    height: max_y - min_y,
                },
            );
        }
    }
    // Inline backgrounds (blitz#340 family): a flattened inline element has
    // no taffy box, so the collect walk never reaches the boxed-Bg emission
    // for it and the background-color silently vanished (nicoburns'
    // diagnosis in that issue is the same shape). Paint it as per-line bands
    // over the hoisted content's absolute rects, spliced at the content's
    // first paint item so the color lands under the element's own ink.
    // Nested inlines record a later first item, so their bands splice after
    // the outer's and overpaint it — outer bg under inner bg under ink, the
    // CSS inline paint order.
    if !flattened.is_empty() {
        // A rotated ancestor's SetXf bracket maps everything inside it, and
        // the band rects below are CANVAS-space (each line band maps through
        // the leaf's recorded map) — splicing them raw there would
        // double-map. Bands falling inside a bracket get wrapped in a
        // SetXfCanvas/ClearXf pair that cancels the enclosing map (see the
        // enum doc), so the rotated inline background paints at its canvas
        // rect, still under the leaf's own ink.
        let mut inside_xf = vec![false; items.len()];
        {
            let mut depth = 0usize;
            for (i, it) in items.iter().enumerate() {
                inside_xf[i] = depth > 0;
                match it {
                    PaintItem::SetXf { .. } => depth += 1,
                    PaintItem::ClearXf => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        let wrapper_set: std::collections::HashSet<_> = run_wrappers.iter().copied().collect();
        let mut inserts: Vec<(usize, usize, Vec<PaintItem>)> = Vec::new();
        for (dom, kids) in flattened {
            // Paint-contributing decoration on a flattened inline (#21):
            // the solid per-line bands this pass always had, a
            // background-image gradient, or a border. The old guard skipped
            // on missing bg-COLOR, silently dropping gradient-only and
            // border-only spans entirely.
            let st = styles.get(dom);
            let bg = st.and_then(|s| s.background_color).filter(|c| c.3 != 0);
            // clip:text spans: the band gradient moves into the glyphs (#218).
            let grad = st
                .filter(|s| !s.background_clip_text)
                .and_then(|s| s.background_image.as_deref())
                .and_then(diting_css::parse_linear_gradient);
            // (#26): box-shadow layers paint even on a span with no
            // background, gradient, or border at all — shadow-only spans
            // previously fell straight through this guard.
            let shadows = st
                .and_then(|s| s.box_shadow.clone())
                .filter(|v| !v.is_empty());
            let border = st.filter(|s| s.border_style.is_some()).and_then(|s| {
                let widths = [
                    side_px(s.border_width.top),
                    side_px(s.border_width.right),
                    side_px(s.border_width.bottom),
                    side_px(s.border_width.left),
                ];
                widths.iter().any(|w| *w > 0.0).then(|| {
                    let color = s
                        .border_color
                        .or(s.color)
                        .map(|c| [c.0, c.1, c.2, c.3])
                        .unwrap_or([0, 0, 0, 255]);
                    (widths, color)
                })
            });
            if bg.is_none() && grad.is_none() && border.is_none() && shadows.is_none() {
                continue;
            }
            // The recorded kids can be leaves directly or run wrappers
            // holding them at any depth (nested inlines flatten wrapper
            // into wrapper); descend through wrappers so bands measure
            // text geometry, not wrapper spans.
            let mut pieces: Vec<Rect> = Vec::new();
            let mut owners: Vec<diting_taffy::tree::NodeId> = Vec::new();
            for k in kids {
                expand_wrapped_leaves(
                    *k,
                    taffy_tree,
                    &wrapper_set,
                    &local_by_node,
                    fonts,
                    &mut pieces,
                    &mut owners,
                );
            }
            let Some(&idx) = owners.iter().filter_map(|n| node_first_item.get(n)).min() else {
                continue;
            };
            let inside = inside_xf.get(idx).copied().unwrap_or(false);
            // Group pieces into line bands by vertical overlap: same-line
            // leaves share the line box even where baseline shifts split
            // their tops; different lines never overlap.
            pieces.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
            let mut bands: Vec<Rect> = Vec::new();
            for r in pieces {
                match bands.last_mut() {
                    Some(last) if r.y < last.y + last.height => {
                        let bottom = (last.y + last.height).max(r.y + r.height);
                        let right = (last.x + last.width).max(r.x + r.width);
                        last.y = last.y.min(r.y);
                        last.x = last.x.min(r.x);
                        last.width = right - last.x;
                        last.height = bottom - last.y;
                    }
                    _ => bands.push(r),
                }
            }
            let mut band_items: Vec<PaintItem> = Vec::new();
            // (#26) slice corner radii: resolve the span's corner_radii
            // against each band box, then box-decoration-break: slice — the
            // FIRST fragment keeps the left corners (TL/BL), the LAST the
            // right (TR/BR), middles are square (Chrome headless evidence on
            // the issue). Per-band clamping shrinks oversized radii the way
            // Chrome's proportional reduction turns a too-tall 16px radius
            // into the elliptical arc on a short line fragment.
            let corners = st.and_then(|s| s.corner_radii);
            let res = |l: &diting_css::Length, basis: f32| match l {
                diting_css::Length::Px(v) => *v,
                diting_css::Length::Percent(p) => p * basis / 100.0,
                diting_css::Length::Calc { percent, .. } => percent * basis / 100.0,
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent => 0.0,
            };
            let n_bands = bands.len();
            let frag_radii = |i: usize, band: &Rect| -> [(f32, f32); 4] {
                corners
                    .map(|cs| {
                        let mut rs = std::array::from_fn(|k| {
                            (res(&cs[k].0, band.width), res(&cs[k].1, band.height))
                        });
                        if n_bands > 1 {
                            if i == 0 {
                                rs[1] = (0.0, 0.0);
                                rs[2] = (0.0, 0.0);
                            } else if i + 1 == n_bands {
                                rs[0] = (0.0, 0.0);
                                rs[3] = (0.0, 0.0);
                            } else {
                                rs = [(0.0, 0.0); 4];
                            }
                        }
                        clamp_corner_radii(rs, band.width, band.height)
                    })
                    .unwrap_or([(0.0, 0.0); 4])
            };
            // Shadow geometry rides the fragment border box: the band grown
            // outward by the border widths (left edge only on the first
            // fragment, right only on the last — same slice rule the border
            // strips below follow).
            let [bt, br_, bb, bl] = border.as_ref().map(|(w, _)| *w).unwrap_or([0.0; 4]);
            let frag_rect = |i: usize, band: &Rect| Rect {
                x: if i == 0 { band.x - bl } else { band.x },
                y: band.y - bt,
                width: (if i + 1 == n_bands {
                    band.x + band.width + br_
                } else {
                    band.x + band.width
                }) - (if i == 0 { band.x - bl } else { band.x }),
                height: band.height + bt + bb,
            };
            // Layer order mirrors the block walk: outer shadows below the
            // background, bg-color, bg-image gradient, inset shadows above
            // the background, border strips on top.
            if let Some(ref list) = shadows {
                for sh in list.iter().rev().filter(|sh| !sh.inset) {
                    for (i, band) in bands.iter().enumerate() {
                        band_items.push(PaintItem::BoxShadow {
                            rect: frag_rect(i, band),
                            color: [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
                            radii: frag_radii(i, band),
                            dx: sh.dx,
                            dy: sh.dy,
                            blur: sh.blur,
                            spread: sh.spread,
                            inset: false,
                        });
                    }
                }
            }
            if let Some(color) = bg {
                let color = [color.0, color.1, color.2, color.3];
                for (i, rect) in bands.iter().enumerate() {
                    let radii = frag_radii(i, rect);
                    if radii.iter().all(|r| *r == radii[0]) {
                        band_items.push(PaintItem::Bg {
                            rect: *rect,
                            color,
                            radius: radii[0].0,
                        });
                    } else {
                        band_items.push(PaintItem::BgCorner {
                            rect: *rect,
                            color,
                            radii,
                        });
                    }
                }
            }
            if let Some(g) = grad {
                // Continuous strip (Blink PaintRectForImageStrip): every
                // fragment samples the gradient defined over the UNION of
                // the bands, clipped to its own band — a to-right gradient
                // never restarts on later lines.
                let ux = bands.iter().map(|r| r.x).fold(f32::INFINITY, f32::min);
                let uy = bands.iter().map(|r| r.y).fold(f32::INFINITY, f32::min);
                let ur = bands
                    .iter()
                    .map(|r| r.x + r.width)
                    .fold(f32::NEG_INFINITY, f32::max);
                let ub = bands
                    .iter()
                    .map(|r| r.y + r.height)
                    .fold(f32::NEG_INFINITY, f32::max);
                if ur > ux && ub > uy {
                    let union = Rect {
                        x: ux,
                        y: uy,
                        width: ur - ux,
                        height: ub - uy,
                    };
                    let stops: Vec<(f32, [u8; 4])> = g
                        .stops
                        .iter()
                        .map(|(p, c)| (*p, [c.0, c.1, c.2, c.3]))
                        .collect();
                    for (i, band) in bands.iter().enumerate() {
                        band_items.push(PaintItem::Clip { rect: *band });
                        band_items.push(PaintItem::BgGradient {
                            rect: union,
                            stops: stops.clone(),
                            css_deg: g.css_deg,
                            radii: frag_radii(i, band),
                        });
                        band_items.push(PaintItem::PopClip);
                    }
                }
            }
            if let Some(ref list) = shadows {
                for sh in list.iter().rev().filter(|sh| sh.inset) {
                    for (i, band) in bands.iter().enumerate() {
                        band_items.push(PaintItem::BoxShadow {
                            rect: frag_rect(i, band),
                            color: [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
                            radii: frag_radii(i, band),
                            dx: sh.dx,
                            dy: sh.dy,
                            blur: sh.blur,
                            spread: sh.spread,
                            inset: true,
                        });
                    }
                }
            }
            if let Some((widths, color)) = border {
                // box-decoration-break: slice — top/bottom edges ride every
                // fragment; left only the first, right only the last. The
                // strips sit OUTSIDE the text bands (Chrome grows the
                // fragment border-box outward), so they never cover glyphs.
                let [t, r, b, l] = widths;
                for (i, band) in bands.iter().enumerate() {
                    let x0 = if i == 0 { band.x - l } else { band.x };
                    let x1 = if i + 1 == bands.len() {
                        band.x + band.width + r
                    } else {
                        band.x + band.width
                    };
                    if t > 0.0 {
                        band_items.push(PaintItem::Bg {
                            rect: Rect {
                                x: x0,
                                y: band.y - t,
                                width: x1 - x0,
                                height: t,
                            },
                            color,
                            radius: 0.0,
                        });
                    }
                    if b > 0.0 {
                        band_items.push(PaintItem::Bg {
                            rect: Rect {
                                x: x0,
                                y: band.y + band.height,
                                width: x1 - x0,
                                height: b,
                            },
                            color,
                            radius: 0.0,
                        });
                    }
                    if i == 0 && l > 0.0 {
                        band_items.push(PaintItem::Bg {
                            rect: Rect {
                                x: band.x - l,
                                y: band.y - t,
                                width: l,
                                height: band.height + t + b,
                            },
                            color,
                            radius: 0.0,
                        });
                    }
                    if i + 1 == bands.len() && r > 0.0 {
                        band_items.push(PaintItem::Bg {
                            rect: Rect {
                                x: band.x + band.width,
                                y: band.y - t,
                                width: r,
                                height: band.height + t + b,
                            },
                            color,
                            radius: 0.0,
                        });
                    }
                }
            }
            if inside {
                band_items.insert(0, PaintItem::SetXfCanvas);
                band_items.push(PaintItem::ClearXf);
            }
            inserts.push((idx, tree.ancestors(*dom).len(), band_items));
        }
        // Later splice points first, deeper element first on ties, so each
        // batch lands under everything recorded after it.
        inserts.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        // By reference: the sticky-span adjustment below re-reads the
        // pre-splice indices and band sizes after this loop.
        for (idx, _, band_items) in &inserts {
            items.splice(*idx..*idx, band_items.iter().cloned());
        }
        // Sticky spans were recorded pre-splice, and the insert indices are
        // pre-splice too: shift each endpoint by every insert landing inside
        // it. A start counts an insert AT the boundary (a band spliced at
        // the sticky node's own first item is its inline background); an
        // end is exclusive, so only strictly-inside inserts count.
        if !sticky_spans.is_empty() {
            for (_, start, end) in sticky_spans.iter_mut() {
                let (s0, e0) = (*start, *end);
                *start = s0
                    + inserts
                        .iter()
                        .map(|(idx, _, band)| usize::from(*idx <= s0) * band.len())
                        .sum::<usize>();
                *end = e0
                    + inserts
                        .iter()
                        .map(|(idx, _, band)| usize::from(*idx < e0) * band.len())
                        .sum::<usize>();
            }
        }
    }
    (
        rects,
        items,
        paint_order,
        local_geom,
        sticky_spans,
        scroller_spans,
    )
}
