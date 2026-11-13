//! Post-solve passes of `layout_solve_rooted`.
//!
//! Static-position harvest, out-of-flow reparent, float continuation,
//! calc-width repair, box-size repair and wrap-host intrinsic repair.
//! Split from the layout hub. `layout_solve_rooted` still calls them.
//! Pure move: these functions see the same crate namespace they grew up in.

use super::*;

/// A re-solve inside one of `layout_solve_rooted`'s passes failed; the
/// solve answers [`SolvedGeometry::aborted`].
pub(super) struct SolveAborted;

/// Static-position harvest of `layout_solve_rooted` (moved out verbatim):
/// out-of-flow boxes with both insets auto on an axis resolve at their flow
/// spot, measured by a pre-solve before the reparent pass moves them.
pub(super) fn harvest_static_positions(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    icb_node: diting_taffy::tree::NodeId,
    available: Size<AvailableSpace>,
) -> HashMap<NodeId, (f32, f32)> {
    // --- static-position harvest (blitz#764 review point) ----------------
    // An out-of-flow box with BOTH insets auto on an axis resolves at its
    // static position: where it would have been in its original flow. The
    // reparent pass below moves the box to its containing block first, and
    // taffy's auto-inset fallback then uses the flow position inside its
    // CURRENT taffy parent - the CB - so the box lands after the CB's last
    // in-flow child instead of at its DOM-parent flow spot. Harvest by
    // laying the tree out once BEFORE reparenting: with the boxes still
    // absolute children of their DOM parents, taffy's auto-inset fallback
    // IS the CSS static position (its flow spot among the original
    // siblings, which per CSS is computed with every OTHER out-of-flow box
    // still out of flow - exactly what an absolute child sees). No style
    // mutation, so the main layout below starts from a clean cache.
    let mut static_pos: HashMap<NodeId, (f32, f32)> = HashMap::new();
    {
        let needs_static: Vec<(diting_taffy::tree::NodeId, NodeId)> = node_map
            .iter()
            .filter(|(_, dom_id)| {
                styles.get(*dom_id).is_some_and(|s| {
                    matches!(
                        s.position,
                        Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
                    ) && ((s.left.is_none() && s.right.is_none())
                        || (s.top.is_none() && s.bottom.is_none()))
                })
            })
            .map(|(t, d)| (*t, *d))
            .collect();
        if !needs_static.is_empty() {
            let laid_out = taffy_tree
                .compute_layout_with_block_measure(
                    icb_node,
                    available,
                    |inputs, id, ctx, style, block| {
                        float_lines::measure_node(fonts, inputs, id, ctx, style, block)
                    },
                )
                .is_ok();
            if laid_out {
                let mut now: HashMap<diting_taffy::tree::NodeId, Rect> = HashMap::new();
                abs_rects::collect(
                    taffy_tree,
                    styles,
                    node_map,
                    icb_node,
                    (0.0, 0.0),
                    (0.0, 0.0),
                    &mut now,
                );
                for (tnid, dom_id) in &needs_static {
                    if let Some(r) = now.get(tnid) {
                        static_pos.insert(*dom_id, (r.x, r.y));
                    }
                }

                // #188-2 (blitz#977): the SELF-alignment of the out-of-flow
                // box. This taffy rev (pre-OofItemStyle, taffy#1206) places
                // an absolute child of a flex container using the
                // container's align-items only — the child's own align-self
                // is never consulted — and a grid container doesn't place
                // its abspos children at all (everything lands at 0,0).
                // CSS resolves an out-of-flow child's static position as
                // "the sole item of its flow parent" (css-flexbox §4.1;
                // css-grid §9.2: a grid area spanning the container's
                // content edges), so the child's own self-alignment resolves
                // against the parent's content box. Recompute each axis
                // whose insets are both auto; the harvested position stands
                // everywhere else.
                let dom_of: HashMap<NodeId, diting_taffy::tree::NodeId> =
                    node_map.iter().map(|(k, v)| (*v, *k)).collect();
                for (tnid, dom_id) in &needs_static {
                    let (Some(style), Some(parent_dom), Some(entry)) = (
                        styles.get(dom_id),
                        tree.with_node(*dom_id, |n| n.parent).flatten(),
                        static_pos.get_mut(dom_id),
                    ) else {
                        continue;
                    };
                    let parent = styles.get(&parent_dom);
                    let is_flex = matches!(parent.and_then(|p| p.display), Some(CssDisplay::Flex));
                    let is_grid = matches!(parent.and_then(|p| p.display), Some(CssDisplay::Grid));
                    if !is_flex && !is_grid {
                        continue;
                    }
                    let Some(&parent_tnid) = dom_of.get(&parent_dom) else {
                        continue;
                    };
                    let (Some(prect), Some(crect), Ok(playout), Ok(clayout)) = (
                        now.get(&parent_tnid),
                        now.get(tnid),
                        taffy_tree.layout(parent_tnid),
                        taffy_tree.layout(*tnid),
                    ) else {
                        continue;
                    };
                    // The parent's content box in absolute coordinates, from
                    // taffy's resolved border/padding (Rect<f32> in Layout).
                    let (pb, pp) = (playout.border, playout.padding);
                    let ox = prect.x + pb.left + pp.left;
                    let oy = prect.y + pb.top + pp.top;
                    let cw =
                        (playout.size.width - pb.left - pb.right - pp.left - pp.right).max(0.0);
                    let ch =
                        (playout.size.height - pb.top - pb.bottom - pp.top - pp.bottom).max(0.0);
                    let m = clayout.margin;
                    let x_auto = style.left.is_none() && style.right.is_none();
                    let y_auto = style.top.is_none() && style.bottom.is_none();

                    if is_grid {
                        // Grid: the static-position area is the container's
                        // whole content box (css-grid §9.2); each axis
                        // resolves the child's self-alignment first, falling
                        // back to the container's items-level default
                        // (justify-items inline, align-items block; stretch
                        // ≈ start under §10.3.7).
                        if x_auto {
                            let mode = style
                                .justify_self
                                .or(parent.and_then(|p| p.justify_items))
                                .unwrap_or(JustifyMode::FlexStart);
                            let free = cw - m.left - m.right - crect.width;
                            entry.0 = ox + m.left + mode.static_offset(free);
                        }
                        if y_auto {
                            let mode = style
                                .align_self
                                .or(parent.and_then(|p| p.align_items))
                                .unwrap_or(AlignMode::Stretch);
                            let free = ch - m.top - m.bottom - crect.height;
                            entry.1 = oy + m.top + mode.static_offset(free);
                        }
                    } else {
                        // Flex: justify-self does not apply to flex items
                        // (css-flexbox §4) — the main axis keeps taffy's
                        // placement, which already carries the container's
                        // justify-content. Only the cross axis re-resolves,
                        // and only when the child states its own align-self
                        // (unset defers to the container's align-items,
                        // already honored by the harvest above).
                        let column = matches!(
                            parent.and_then(|p| p.flex_direction),
                            Some(CssFlexDirection::Column) | Some(CssFlexDirection::ColumnReverse)
                        );
                        if column && x_auto {
                            if let Some(mode) = style.align_self {
                                let free = cw - m.left - m.right - crect.width;
                                entry.0 = ox + m.left + mode.static_offset(free);
                            }
                        }
                        if !column && y_auto {
                            if let Some(mode) = style.align_self {
                                let free = ch - m.top - m.bottom - crect.height;
                                entry.1 = oy + m.top + mode.static_offset(free);
                            }
                        }
                    }
                }
            }
        }
    }

    static_pos
}

/// Absolute/fixed reparent pass of `layout_solve_rooted` (moved out
/// verbatim): out-of-flow boxes move under their containing block (a
/// positioned or transformed ancestor, else the ICB).
pub(super) fn reparent_out_of_flow(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    icb_node: diting_taffy::tree::NodeId,
    root: Option<NodeId>,
) {
    // Absolute/fixed reparent pass (upstream's containing-block fix-up):
    // taffy resolves an absolute child against its DIRECT taffy parent, so
    // move each out-of-flow box to its CSS containing block — the nearest
    // ancestor with position != static OR a non-none transform (CSS
    // Transforms §: a transformed ancestor is a containing block for both
    // absolute and fixed descendants). Fixed with no such ancestor —
    // positioned ancestors don't pin it — resolves to the root = the
    // initial containing block stand-in.
    {
        let dom_of: HashMap<NodeId, diting_taffy::tree::NodeId> =
            node_map.iter().map(|(k, v)| (*v, *k)).collect();
        let mut reparents: Vec<(diting_taffy::tree::NodeId, diting_taffy::tree::NodeId)> = node_map
            .iter()
            .filter_map(|(taffy_nid, dom_id)| {
                let style = styles.get(dom_id)?;
                let fixed = style.position == Some(PositionMode::Fixed);
                if style.position != Some(PositionMode::Absolute) && !fixed {
                    return None;
                }
                let target_dom = {
                    let mut cur = tree.with_node(*dom_id, |n| n.parent).flatten();
                    while let Some(nid) = cur {
                        let ancestor = styles.get(&nid);
                        // CSS Transforms: a non-none transform makes an
                        // ancestor the containing block for BOTH absolute
                        // and fixed descendants, and so do `filter` and
                        // `perspective` (filter-effects-1 § containing
                        // block; transform-style perspective). For fixed
                        // those are the only ancestor kinds that override
                        // the viewport — a merely positioned ancestor
                        // doesn't pin it.
                        let transformed = ancestor.is_some_and(|s| {
                            s.effective_transform().is_some()
                                || s.filter.is_some()
                                || s.perspective.is_some()
                        });
                        if transformed
                            || (!fixed
                                && ancestor.is_some_and(|s| {
                                    matches!(
                                        s.position,
                                        Some(PositionMode::Relative)
                                            | Some(PositionMode::Absolute)
                                            | Some(PositionMode::Fixed)
                                    )
                                }))
                        {
                            break;
                        }
                        cur = tree.with_node(nid, |n| n.parent).flatten();
                    }
                    cur
                };
                let target = target_dom
                    .and_then(|d| dom_of.get(&d))
                    .copied()
                    .unwrap_or(icb_node);
                let current = taffy_tree.parent(*taffy_nid)?;
                if current != target {
                    Some((*taffy_nid, target))
                } else {
                    None
                }
            })
            .collect();
        // Append order must be DOCUMENT order: node_map is a HashMap, and
        // add_child appends — iterating it directly scrambles sibling order
        // after reparenting (paint order then diverges from both blitz and
        // the DOM). Sort the reparented nodes by their pre-order rank.
        let mut rank: HashMap<NodeId, usize> = HashMap::new();
        fn preorder(
            tree: &DomTree,
            id: NodeId,
            rank: &mut HashMap<NodeId, usize>,
            ctr: &mut usize,
        ) {
            rank.insert(id, *ctr);
            *ctr += 1;
            for child in render_children(tree, id) {
                preorder(tree, child, rank, ctr);
            }
        }
        {
            let mut ctr = 0usize;
            if let Some(root_id) = root {
                preorder(tree, root_id, &mut rank, &mut ctr);
            }
        }
        reparents.sort_by_key(|(taffy_nid, _)| {
            node_map
                .get(taffy_nid)
                .and_then(|d| rank.get(d))
                .copied()
                .unwrap_or(usize::MAX)
        });
        for (node, target) in reparents {
            if let Some(old) = taffy_tree.parent(node) {
                let _ = taffy_tree.remove_child(old, node);
            }
            let _ = taffy_tree.add_child(target, node);
        }
    }
}

/// Float continuation of `layout_solve_rooted` (batch 8g, moved out
/// verbatim): a float excludes in-flow content beyond its direct siblings.
pub(super) fn continue_floats(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    icb_node: diting_taffy::tree::NodeId,
    available: Size<AvailableSpace>,
) -> Result<(), SolveAborted> {
    // --- float continuation (batch 8g) -----------------------------------
    // A float excludes in-flow content not only among its DIRECT siblings:
    // blocks further down the DOM (later siblings of its parent, and of
    // grand-parents, up to the nearest block formatting context) must also
    // shorten their lines where they intersect the float's band. The tree
    // build can't know those rectangles yet, so — like upstream
    // apply_float_continuations — this runs after a first layout: collect
    // every float's real band, narrow the intersecting LATER blocks by
    // clamping their max-width (left float: shrink the right side; right
    // float: push the left edge past the float), then lay out again.
    {
        let dom_of: HashMap<NodeId, diting_taffy::tree::NodeId> =
            node_map.iter().map(|(k, v)| (*v, *k)).collect();
        let mut rects_now: HashMap<diting_taffy::tree::NodeId, Rect> = HashMap::new();
        fn abs_rects(
            taffy_tree: &TaffyTree<TextLeaf>,
            node: diting_taffy::tree::NodeId,
            offset: (f32, f32),
            out: &mut HashMap<diting_taffy::tree::NodeId, Rect>,
        ) {
            let Ok(layout) = taffy_tree.layout(node) else {
                return;
            };
            let abs = (offset.0 + layout.location.x, offset.1 + layout.location.y);
            out.insert(
                node,
                Rect {
                    x: abs.0,
                    y: abs.1,
                    width: layout.size.width,
                    height: layout.size.height,
                },
            );
            for child in taffy_tree.children(node).unwrap_or_default() {
                abs_rects(taffy_tree, child, abs, out);
            }
        }
        abs_rects(taffy_tree, icb_node, (0.0, 0.0), &mut rects_now);

        // Float bands with the DOM node that owns each float.
        let mut bands: Vec<(NodeId, diting_taffy::tree::NodeId, Rect)> = Vec::new();
        for (tnid, dom_id) in node_map.iter() {
            let is_float = styles.get(dom_id).is_some_and(|s| s.float_side.is_some());
            if !is_float {
                continue;
            }
            if let Some(r) = rects_now.get(tnid) {
                bands.push((*dom_id, *tnid, *r));
            }
        }
        if !bands.is_empty() {
            let mut changed = false;
            for (float_dom, fnode, fr) in &bands {
                let side = styles
                    .get(float_dom)
                    .and_then(|s| s.float_side)
                    .unwrap_or(diting_css::FloatSide::Left);
                let Some(parent_dom) = tree.with_node(*float_dom, |n| n.parent).flatten() else {
                    continue;
                };
                let float_taffy_parent = taffy_tree.parent(*fnode);
                // Walk from the float's parent upward; narrow every LATER
                // sibling block until we hit a BFC stand-in. Blocks inside
                // the float's OWN zone row (the 8b flow column) share the
                // float's taffy parent — they are the content the float
                // already excludes by construction, so skip them.
                let mut below = *float_dom;
                let mut cur = parent_dom;
                loop {
                    // A grid/flex container establishes an independent
                    // formatting context: its children are grid/flex items
                    // that CSS floats can never displace. Vector 2022 parks
                    // floated navboxes deep inside the article's mw-body
                    // grid — without this stop the climb escapes the grid
                    // and narrows the page's TOC column to max-width 0.
                    if styles.get(&cur).is_some_and(|s| {
                        matches!(s.display, Some(CssDisplay::Grid) | Some(CssDisplay::Flex))
                    }) {
                        break;
                    }
                    // cur's taffy box shares the float's taffy parent — both
                    // sit inside the same synthetic 8b/8c/8d row, whose
                    // contents the float already excludes. Stop climbing.
                    if let Some(&cnid) = dom_of.get(&cur) {
                        if taffy_tree.parent(cnid) == float_taffy_parent {
                            break;
                        }
                    }
                    let siblings = tree.children(cur);
                    // Floats displace only content that FOLLOWS them in
                    // document order: siblings BEFORE the float's branch
                    // keep their full width (their boxes predate the float's
                    // band). Narrowing them would, e.g., shrink every
                    // section above an article-bottom navbox float.
                    let after = match siblings.iter().position(|s| *s == below) {
                        Some(i) => &siblings[i + 1..],
                        None => &siblings[..],
                    };
                    for sib in after {
                        let Some(&snid) = dom_of.get(sib) else {
                            continue;
                        };
                        // Anything whose taffy ANCESTRY runs through the
                        // float's own synthetic row (the 8b flow column
                        // wraps the zone's blocks) is content the float
                        // already excludes — never narrow it.
                        let mut anc = taffy_tree.parent(snid);
                        let mut inside_zone_row = false;
                        while let Some(a) = anc {
                            if Some(a) == float_taffy_parent {
                                inside_zone_row = true;
                                break;
                            }
                            anc = taffy_tree.parent(a);
                        }
                        if inside_zone_row {
                            continue;
                        }
                        let Some(sr) = rects_now.get(&snid) else {
                            continue;
                        };
                        // Intersect test against this float's vertical band.
                        let overlaps_y = sr.y < fr.y + fr.height && sr.y + sr.height > fr.y;
                        if !overlaps_y || sr.width <= 0.0 {
                            continue;
                        }
                        let is_float_sib = styles.get(sib).is_some_and(|s| s.float_side.is_some());
                        if is_float_sib {
                            continue;
                        }
                        // Out-of-flow boxes are never pushed by a float
                        // (CSS: floats only displace in-flow content).
                        let out_of_flow = styles.get(sib).is_some_and(|s| {
                            matches!(
                                s.position,
                                Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
                            )
                        });
                        if out_of_flow {
                            continue;
                        }
                        // CSS 2.1 §9.5: a float's band moves the BORDER BOX
                        // only of a later in-flow box that establishes an
                        // independent formatting context (an overflow-
                        // clipping box, a table, a flex/grid container, an
                        // atomic inline). A plain overflow:visible block
                        // keeps its full width — the float shortens just
                        // its inner line boxes, which the 8b zone owns.
                        // Clamping plain blocks collapsed real pages: the
                        // tmall publish form's width:auto cards were pinned
                        // to a feedback float's leftover sliver when its
                        // band stuck out of a shrunk parent (#127).
                        // overflow:clip is paint-only and does NOT contain
                        // floats (css-overflow-3), so it stays plain here.
                        let dodges = styles.get(sib).is_some_and(|s| {
                            matches!(
                                s.effective_overflow(),
                                Overflow::Hidden | Overflow::Auto | Overflow::Scroll
                            ) || matches!(
                                s.display,
                                Some(CssDisplay::Flex)
                                    | Some(CssDisplay::Grid)
                                    | Some(CssDisplay::Table)
                                    | Some(CssDisplay::InlineBlock)
                            )
                        });
                        if !dodges {
                            continue;
                        }
                        match side {
                            diting_css::FloatSide::Left => {
                                let want_right = fr.x + fr.width;
                                let inset = (want_right - sr.x).max(0.0);
                                let avail = (sr.width - inset).max(0.0);
                                if avail < sr.width - 0.5 {
                                    if let Ok(mut st) = taffy_tree.style(snid).cloned() {
                                        st.max_size.width =
                                            LengthPercentageAuto::length(avail.max(0.0));
                                        let _ = taffy_tree.set_style(snid, st);
                                        changed = true;
                                    }
                                }
                            }
                            diting_css::FloatSide::Right => {
                                let float_left = fr.x;
                                let inset = (sr.x + sr.width - float_left).max(0.0);
                                let avail = (sr.width - inset).max(0.0);
                                if avail < sr.width - 0.5 {
                                    if let Ok(mut st) = taffy_tree.style(snid).cloned() {
                                        st.margin.left = LengthPercentageAuto::length(fr.x - sr.x);
                                        st.max_size.width =
                                            LengthPercentageAuto::length(avail.max(0.0));
                                        let _ = taffy_tree.set_style(snid, st);
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                    // The level just processed narrowed inside `cur`. If
                    // `cur` itself establishes an independent formatting
                    // context, every float inside it is CONTAINED — its
                    // band can never reach `cur`'s own later siblings, so
                    // the climb stops here instead of escaping (grid/flex
                    // already stopped at the top; overflow:clip is paint-
                    // only and does not contain floats).
                    if styles.get(&cur).is_some_and(|s| {
                        matches!(
                            s.effective_overflow(),
                            Overflow::Hidden | Overflow::Auto | Overflow::Scroll
                        ) || matches!(
                            s.display,
                            Some(CssDisplay::Table) | Some(CssDisplay::InlineBlock)
                        )
                    }) {
                        break;
                    }
                    below = cur;
                    let Some(grand) = tree.with_node(cur, |n| n.parent).flatten() else {
                        break;
                    };
                    cur = grand;
                }
            }
            if changed {
                let re = taffy_tree.compute_layout_with_block_measure(
                    icb_node,
                    available,
                    |inputs, id, ctx, style, block| {
                        float_lines::measure_node(fonts, inputs, id, ctx, style, block)
                    },
                );
                if re.is_err() {
                    return Err(SolveAborted);
                }
            }
        }
    }
    // calc() repair pass (obscura#767 family): a mixed percent+px calc on
    // width/min/max rode in as a percent-only placeholder (taffy has no
    // "percent + px" Dimension shape). Now that the layout has settled,
    // resolve each placeholder exactly ONCE against its containing block's
    // content-box width — CSS never re-samples a resolved value against a
    // re-sampled basis, so this single deterministic pass replaces the
    // value instead of feeding it back through another percent resolution.
    Ok(())
}

/// calc() width repair of `layout_solve_rooted` (moved out verbatim):
/// resolved shallow-first so a calc width inside a calc-width parent
/// cascades; returns the repaired nodes for the #245 pass.
pub(super) fn repair_calc_widths(
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    icb_node: diting_taffy::tree::NodeId,
    available: Size<AvailableSpace>,
) -> Result<HashMap<diting_taffy::tree::NodeId, f32>, SolveAborted> {
    // Sorted shallow-first, a calc width inside a calc-width parent cascades
    // through the `repaired` map (also read by the #245 pass below) without
    // another round-trip. height and margin/padding/inset keep the
    // percent-only placeholder (documented approximation).
    let mut repaired: HashMap<diting_taffy::tree::NodeId, f32> = HashMap::new();
    {
        // A content-box percent width with padding/border is also repaired:
        // the box is pct * containing-block + the px edges (taffy resolves a
        // percent size as the border box), which is what Chrome lays out.
        let edges_px = |s: &diting_css::ComputedStyle| {
            side_px(s.padding.left)
                + side_px(s.padding.right)
                + if s.border_style.is_some() {
                    side_px(s.border_width.left) + side_px(s.border_width.right)
                } else {
                    0.0
                }
        };
        let is_pct =
            |l: Option<diting_css::Length>| matches!(l, Some(diting_css::Length::Percent(_)));
        let has_calc_width = |s: &diting_css::ComputedStyle| {
            matches!(s.width, Some(diting_css::Length::Calc { .. }))
                || matches!(s.min_width, Some(diting_css::Length::Calc { .. }))
                || matches!(s.max_width, Some(diting_css::Length::Calc { .. }))
                || ((is_pct(s.width) || is_pct(s.min_width) || is_pct(s.max_width))
                    && !matches!(s.box_sizing, Some(diting_css::BoxSizing::BorderBox))
                    && edges_px(s) > 0.0)
        };
        let mut fixups: Vec<(diting_taffy::tree::NodeId, NodeId)> = node_map
            .iter()
            .filter(|(_, d)| styles.get(*d).is_some_and(has_calc_width))
            .map(|(t, d)| (*t, *d))
            .collect();
        if !fixups.is_empty() {
            fixups.sort_by_key(|(t, _)| taffy_depth(taffy_tree, *t, icb_node));
            let mut float_fixups = false;
            for (tnid, dom_id) in fixups {
                let Some(st) = styles.get(&dom_id) else {
                    continue;
                };
                float_fixups |= st.float_side.is_some();
                // Containing block = the taffy parent's settled content box,
                // or the arithmetic repair when the parent itself was a
                // calc-width fixup.
                let Some(cbw) = taffy_tree.parent(tnid).and_then(|p| {
                    if let Some(w) = repaired.get(&p) {
                        return Some(*w);
                    }
                    let l = taffy_tree.layout(p).ok()?;
                    Some(
                        l.size.width
                            - l.padding.left
                            - l.padding.right
                            - l.border.left
                            - l.border.right,
                    )
                }) else {
                    continue;
                };
                // content-box → taffy border-box carry-over, same as
                // to_taffy_style's px arms — skipped under authored
                // `box-sizing: border-box`, where the calc result already
                // measures to the border edge.
                let pb = side_px(st.padding.left)
                    + side_px(st.padding.right)
                    + if st.border_style.is_some() {
                        side_px(st.border_width.left) + side_px(st.border_width.right)
                    } else {
                        0.0
                    };
                let border_box = matches!(st.box_sizing, Some(diting_css::BoxSizing::BorderBox));
                let infl = if border_box { 0.0 } else { pb };
                let Some(mut ts) = taffy_tree.style(tnid).ok().cloned() else {
                    continue;
                };
                let as_calc = |l: Option<diting_css::Length>| match l {
                    Some(diting_css::Length::Calc { percent, px }) => Some((percent, px)),
                    Some(diting_css::Length::Percent(p)) if infl > 0.0 => Some((p, 0.0)),
                    _ => None,
                };
                let calc_px = |e: Option<&str>, percent: f32, px: f32| {
                    e.and_then(|e| diting_css::eval_width_expr(e, cbw))
                        .unwrap_or(cbw * percent / 100.0 + px)
                        .max(0.0)
                };
                if let Some((percent, px)) = as_calc(st.width) {
                    let content = calc_px(st.width_expr.as_deref(), percent, px);
                    ts.size.width = Dimension::length(content + infl);
                    // Children resolve percents against this element's CONTENT
                    // box — under border-box the calc result is the border
                    // box, so strip padding+border for the child basis.
                    repaired.insert(
                        tnid,
                        if border_box {
                            (content - pb).max(0.0)
                        } else {
                            content
                        },
                    );
                }
                if let Some((percent, px)) = as_calc(st.min_width) {
                    let content = calc_px(st.min_width_expr.as_deref(), percent, px);
                    ts.min_size.width = LengthPercentageAuto::length(content + infl);
                }
                if let Some((percent, px)) = as_calc(st.max_width) {
                    let content = calc_px(st.max_width_expr.as_deref(), percent, px);
                    ts.max_size.width = LengthPercentageAuto::length(content + infl);
                }
                let _ = taffy_tree.set_style(tnid, ts);
            }
            // A repaired size changes the collision geometry floats see,
            // but taffy's cache key is (style, constraints) — a sibling
            // text leaf beside a repaired float keeps its cache entry
            // (wrapped against the float's OLD extent) through the
            // re-solve, and its stale float_lines slots would cross the
            // repaired float. Non-float fixups need no bust: their
            // descendants re-measure through the parent-width cache miss,
            // and block siblings do not interact. Bust the subtree only
            // when a float was among the fixups.
            if float_fixups {
                bust_subtree_cache(icb_node, taffy_tree);
            }
            let re = taffy_tree.compute_layout_with_block_measure(
                icb_node,
                available,
                |inputs, id, ctx, style, block| {
                    float_lines::measure_node(fonts, inputs, id, ctx, style, block)
                },
            );
            if re.is_err() {
                return Err(SolveAborted);
            }
        }
    }
    Ok(repaired)
}

/// #245 repair pass of `layout_solve_rooted` (moved out verbatim):
/// content-box px sizes and percent padding.
#[allow(clippy::too_many_arguments)]
pub(super) fn repair_box_sizes(
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    icb_node: diting_taffy::tree::NodeId,
    available: Size<AvailableSpace>,
    repaired: &mut HashMap<diting_taffy::tree::NodeId, f32>,
) -> Result<(), SolveAborted> {
    // #245 repair pass: content-box px sizes + percent padding. The
    // style-map carry-over (to_taffy_style) only adds px-VALUED padding —
    // percent padding has no containing block to resolve against at
    // style-map time, so `width:400px; padding:4%` solved as border-box
    // 400 (content 360) where Chrome keeps the authored 400 as the
    // content width (border box 440). The CB content width is settled
    // now, so resolve the percent paddings against it once and carry
    // the authored px sizes over by the true sums — the same
    // single-pass-then-one-re-solve shape as the calc repair, reading
    // the shared `repaired` arithmetic basis for a parent fixed by
    // either pass (the converse order — a calc child under a parent
    // fixed only here — keeps the stale-layout basis; rare×rare and
    // self-consistent with the re-solve). Border-box sizes skip: the
    // authored value already measures to the border edge.
    {
        let side_used = |l: Option<diting_css::Length>, cbw: f32| -> f32 {
            match l {
                Some(diting_css::Length::Px(v)) => v,
                Some(diting_css::Length::Percent(p)) => cbw * p / 100.0,
                Some(diting_css::Length::Calc { percent, px }) => cbw * percent / 100.0 + px,
                _ => 0.0,
            }
        };
        let pct = |l: Option<diting_css::Length>| {
            matches!(
                l,
                Some(diting_css::Length::Percent(_)) | Some(diting_css::Length::Calc { .. })
            )
        };
        let px = |l: Option<diting_css::Length>| matches!(l, Some(diting_css::Length::Px(_)));
        let needs = |s: &diting_css::ComputedStyle| {
            (pct(s.padding.top)
                || pct(s.padding.right)
                || pct(s.padding.bottom)
                || pct(s.padding.left))
                && !matches!(s.box_sizing, Some(diting_css::BoxSizing::BorderBox))
                && (px(s.width)
                    || px(s.height)
                    || px(s.min_width)
                    || px(s.max_width)
                    || px(s.min_height)
                    || px(s.max_height))
        };
        let mut fixups: Vec<(diting_taffy::tree::NodeId, NodeId)> = node_map
            .iter()
            .filter(|(_, d)| styles.get(*d).is_some_and(needs))
            .map(|(t, d)| (*t, *d))
            .collect();
        if !fixups.is_empty() {
            fixups.sort_by_key(|(t, _)| taffy_depth(taffy_tree, *t, icb_node));
            let mut float_fixups = false;
            for (tnid, dom_id) in fixups {
                let Some(st) = styles.get(&dom_id) else {
                    continue;
                };
                float_fixups |= st.float_side.is_some();
                // Percent padding resolves against the CB's inline size —
                // the parent's settled content box, or either repair pass's
                // arithmetic basis while the taffy layout is still stale.
                let Some(cbw) = taffy_tree.parent(tnid).and_then(|p| {
                    if let Some(w) = repaired.get(&p) {
                        return Some(*w);
                    }
                    let l = taffy_tree.layout(p).ok()?;
                    Some(
                        l.size.width
                            - l.padding.left
                            - l.padding.right
                            - l.border.left
                            - l.border.right,
                    )
                }) else {
                    continue;
                };
                let (pt, pr, pb, pl) = (
                    side_used(st.padding.top, cbw),
                    side_used(st.padding.right, cbw),
                    side_used(st.padding.bottom, cbw),
                    side_used(st.padding.left, cbw),
                );
                let (bt, br, bb, bl) = if st.border_style.is_some() {
                    (
                        side_px(st.border_width.top),
                        side_px(st.border_width.right),
                        side_px(st.border_width.bottom),
                        side_px(st.border_width.left),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };
                let Some(mut ts) = taffy_tree.style(tnid).ok().cloned() else {
                    continue;
                };
                if let Some(diting_css::Length::Px(w)) = st.width {
                    ts.size.width = Dimension::length(w + pl + pr + bl + br);
                    // Children resolve their percents against this
                    // element's content box — the authored px IS the
                    // content width under content-box.
                    repaired.insert(tnid, w);
                }
                if let Some(diting_css::Length::Px(h)) = st.height {
                    ts.size.height = Dimension::length(h + pt + pb + bt + bb);
                }
                if let Some(diting_css::Length::Px(w)) = st.min_width {
                    ts.min_size.width = LengthPercentageAuto::length(w + pl + pr + bl + br);
                }
                if let Some(diting_css::Length::Px(w)) = st.max_width {
                    ts.max_size.width = LengthPercentageAuto::length(w + pl + pr + bl + br);
                }
                if let Some(diting_css::Length::Px(h)) = st.min_height {
                    ts.min_size.height = LengthPercentageAuto::length(h + pt + pb + bt + bb);
                }
                if let Some(diting_css::Length::Px(h)) = st.max_height {
                    ts.max_size.height = LengthPercentageAuto::length(h + pt + pb + bt + bb);
                }
                let _ = taffy_tree.set_style(tnid, ts);
            }
            // Same stale-sibling hazard as the calc repair above: a repaired
            // float's new extent is invisible to its siblings' cache keys.
            if float_fixups {
                bust_subtree_cache(icb_node, taffy_tree);
            }
            let re = taffy_tree.compute_layout_with_block_measure(
                icb_node,
                available,
                |inputs, id, ctx, style, block| {
                    float_lines::measure_node(fonts, inputs, id, ctx, style, block)
                },
            );
            if re.is_err() {
                return Err(SolveAborted);
            }
        }
    }
    Ok(())
}

/// #315 intrinsic repair of `layout_solve_rooted` (moved out verbatim):
/// content-sized wrap hosts get the float+text sums the solve settled.
pub(super) fn repair_wrap_host_intrinsics(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    icb_node: diting_taffy::tree::NodeId,
    available: Size<AvailableSpace>,
) -> Result<(), SolveAborted> {
    // --- #315 intrinsic repair: content-sized wrap hosts --------------------
    // A floated, abs/fixed, or auto-width flex/grid-ITEM wrap host sizes
    // shrink-to-fit, but taffy's intrinsic takes the MAX over block
    // children — the float-figure idiom (float + caption) sized to the
    // float alone and the caption overflowed it. CSS wants
    // min(max(min_content, avail), max_content) where the preferred width
    // SUMS the float margin boxes with the text on one line, and
    // min-content STACKS the floats (widest one) against the longest
    // word (Chrome-truth probes, 2026-10-24: float+margins+caption on
    // one line; pre contributes its widest hard line to the preferred
    // width; abs/fixed measure avail against their containing block,
    // which the reparent pass already made the taffy parent; a flex/grid
    // item's avail is the flex/grid parent's content box, and setting a
    // definite width on the re-solve makes the item contribute that sum).
    // The solve above settled every child; compute the sums, set the
    // width, re-solve once — the same shape as the two repair passes
    // above. eligible admits floats + inline runs + block/ib/flex/grid;
    // the walk sums floats, runs, and ib/flex/grid preferred boxes, and
    // skips plain blocks (Chrome: host preferred width = float alone).
    // Authored min/max stay in the taffy style and clamp on the re-solve.
    {
        let content_sized = |s: &diting_css::ComputedStyle, dom_id: NodeId| {
            if s.width.is_some() {
                return false;
            }
            if s.float_side.is_some()
                || matches!(
                    s.position,
                    Some(diting_css::PositionMode::Absolute)
                        | Some(diting_css::PositionMode::Fixed)
                )
            {
                return true;
            }
            tree.with_node(dom_id, |n| n.parent)
                .flatten()
                .and_then(|p| styles.get(&p))
                .is_some_and(|ps| {
                    matches!(
                        ps.display.unwrap_or(diting_css::Display::Block),
                        diting_css::Display::Flex | diting_css::Display::Grid
                    )
                })
        };
        let mut fixups: Vec<(diting_taffy::tree::NodeId, NodeId)> = node_map
            .iter()
            .filter(|(_, d)| {
                let Some(st) = styles.get(*d) else {
                    return false;
                };
                content_sized(st, **d)
                    && float_flow::eligible(tree, styles, **d, st, &render_children(tree, **d))
            })
            .map(|(t, d)| (*t, *d))
            .collect();
        if !fixups.is_empty() {
            fixups.sort_by_key(|(t, _)| taffy_depth(taffy_tree, *t, icb_node));
            let mut float_fixups = false;
            // host → final border-box width: a nested figure's outer sum
            // reads the inner host's repaired size, not the stale solve.
            let mut sized: HashMap<diting_taffy::tree::NodeId, f32> = HashMap::new();
            let dom_of = |t: diting_taffy::tree::NodeId| node_map.get(&t).copied();
            for (tnid, dom_id) in fixups {
                let Some(st) = styles.get(&dom_id) else {
                    continue;
                };
                float_fixups |= st.float_side.is_some();
                let Some(avail) = taffy_tree.parent(tnid).and_then(|p| {
                    let l = taffy_tree.layout(p).ok()?;
                    Some(
                        l.size.width
                            - l.padding.left
                            - l.padding.right
                            - l.border.left
                            - l.border.right,
                    )
                }) else {
                    continue;
                };
                let mut floats_sum = 0.0f32;
                let mut widest_float = 0.0f32;
                let mut text_max = 0.0f32;
                let mut longest_word = 0.0f32;
                // Walk host children (and anonymous run wrappers): floats and
                // shaped runs sum as before; an inline-block/flex/grid box
                // contributes its preferred width (definite px, else the
                // max-content of runs under it + padding/border) — Chrome
                // places those BFCs beside the float on the preferred line
                // (#315). A plain block contributes nothing.
                #[allow(clippy::too_many_arguments)] // shape mirrors run_tokens'
                fn run_pref(
                    text: &str,
                    font_size: f32,
                    bold: bool,
                    fonts: &FontBook,
                    mono: bool,
                    word_spacing: f32,
                    ws: WhiteSpace,
                    tokens: &std::cell::RefCell<Option<std::rc::Rc<[text::Token]>>>,
                    small_caps: bool,
                    han: Option<text::HanSlot>,
                ) -> (f32, f32) {
                    let shaped = run_tokens(
                        text,
                        font_size,
                        bold,
                        fonts,
                        mono,
                        word_spacing,
                        ws,
                        tokens,
                        small_caps,
                        han,
                    );
                    let (mut line, mut widest_line) = (0.0f32, 0.0f32);
                    for tok in shaped.iter() {
                        if tok.is_break {
                            widest_line = widest_line.max(line);
                            line = 0.0;
                        } else {
                            line += tok.width;
                        }
                    }
                    let longest = shaped
                        .iter()
                        .filter(|tok| !tok.is_space && !tok.is_break)
                        .map(|tok| tok.width)
                        .fold(0.0, f32::max);
                    (widest_line.max(line), longest)
                }
                fn runs_under(
                    taffy_tree: &TaffyTree<TextLeaf>,
                    node: diting_taffy::tree::NodeId,
                    fonts: &FontBook,
                ) -> (f32, f32) {
                    if let Some(TextLeaf::Run {
                        text,
                        font_size,
                        bold,
                        mono,
                        word_spacing,
                        ws,
                        tokens,
                        small_caps,
                        han,
                        ..
                    }) = taffy_tree.get_node_context(node)
                    {
                        return run_pref(
                            text,
                            *font_size,
                            *bold,
                            fonts,
                            *mono,
                            *word_spacing,
                            *ws,
                            tokens,
                            *small_caps,
                            *han,
                        );
                    }
                    let (mut max_c, mut min_c) = (0.0f32, 0.0f32);
                    for c in taffy_tree.children(node).unwrap_or_default() {
                        let (a, b) = runs_under(taffy_tree, c, fonts);
                        max_c += a;
                        min_c = min_c.max(b);
                    }
                    (max_c, min_c)
                }
                #[allow(clippy::too_many_arguments)] // one &mut per tracked min/max
                fn contribute(
                    taffy_tree: &TaffyTree<TextLeaf>,
                    node: diting_taffy::tree::NodeId,
                    fonts: &FontBook,
                    styles: &HashMap<NodeId, ComputedStyle>,
                    dom_of: &dyn Fn(diting_taffy::tree::NodeId) -> Option<NodeId>,
                    sized: &HashMap<diting_taffy::tree::NodeId, f32>,
                    floats_sum: &mut f32,
                    widest_float: &mut f32,
                    text_max: &mut f32,
                    longest_word: &mut f32,
                ) {
                    let is_float = taffy_tree
                        .style(node)
                        .ok()
                        .is_some_and(|s| s.float != diting_taffy::Float::None);
                    if let Some(TextLeaf::Run {
                        text,
                        font_size,
                        bold,
                        mono,
                        word_spacing,
                        ws,
                        tokens,
                        small_caps,
                        han,
                        ..
                    }) = taffy_tree.get_node_context(node)
                    {
                        let (mx, lw) = run_pref(
                            text,
                            *font_size,
                            *bold,
                            fonts,
                            *mono,
                            *word_spacing,
                            *ws,
                            tokens,
                            *small_caps,
                            *han,
                        );
                        *text_max += mx;
                        *longest_word = longest_word.max(lw);
                        return;
                    }
                    if is_float {
                        let outer = sized.get(&node).copied().unwrap_or_else(|| {
                            taffy_tree
                                .layout(node)
                                .ok()
                                .map(|l| l.size.width)
                                .unwrap_or(0.0)
                        }) + dom_of(node)
                            .and_then(|d| styles.get(&d))
                            .map(|s| side_px(s.margin.left) + side_px(s.margin.right))
                            .unwrap_or(0.0);
                        *floats_sum += outer;
                        *widest_float = widest_float.max(outer);
                        return;
                    }
                    if let Some(dom) = dom_of(node) {
                        if let Some(st) = styles.get(&dom) {
                            let display = st.display.unwrap_or(CssDisplay::Block);
                            if matches!(
                                display,
                                CssDisplay::InlineBlock | CssDisplay::Flex | CssDisplay::Grid
                            ) {
                                let margins = side_px(st.margin.left) + side_px(st.margin.right);
                                let pb = side_px(st.padding.left)
                                    + side_px(st.padding.right)
                                    + if st.border_style.is_some() {
                                        side_px(st.border_width.left)
                                            + side_px(st.border_width.right)
                                    } else {
                                        0.0
                                    };
                                let border_box =
                                    matches!(st.box_sizing, Some(diting_css::BoxSizing::BorderBox));
                                let (pref, min_w) = match st.width {
                                    Some(diting_css::Length::Px(w)) => {
                                        let border = if border_box { w } else { w + pb };
                                        (border, border)
                                    }
                                    _ => {
                                        let (mx, lw) = runs_under(taffy_tree, node, fonts);
                                        (mx + pb, lw + pb)
                                    }
                                };
                                *text_max += pref + margins;
                                *longest_word = longest_word.max(min_w + margins);
                                return;
                            }
                            if matches!(display, CssDisplay::Block) {
                                // Plain block: Chrome's shrink-to-fit ignores
                                // it (auto width fills the host → circular).
                                return;
                            }
                        }
                    }
                    for c in taffy_tree.children(node).unwrap_or_default() {
                        contribute(
                            taffy_tree,
                            c,
                            fonts,
                            styles,
                            dom_of,
                            sized,
                            floats_sum,
                            widest_float,
                            text_max,
                            longest_word,
                        );
                    }
                }
                for c in taffy_tree.children(tnid).unwrap_or_default().to_vec() {
                    contribute(
                        taffy_tree,
                        c,
                        fonts,
                        styles,
                        &dom_of,
                        &sized,
                        &mut floats_sum,
                        &mut widest_float,
                        &mut text_max,
                        &mut longest_word,
                    );
                }
                let min_c = longest_word.max(widest_float);
                // Ceil the preferred sum: an atomic ib/flex/grid beside the
                // float is unbreakable, and a band that matches max-content
                // only to the fraction drops it (§9.5 / nowrap face). Chrome
                // fits on the exact sum; we land a sub-pixel short. Existing
                // figure-idiom pins allow ±1px.
                let max_c = (floats_sum + text_max).ceil();
                let content = min_c.max(avail).min(max_c);
                let pb = side_px(st.padding.left)
                    + side_px(st.padding.right)
                    + if st.border_style.is_some() {
                        side_px(st.border_width.left) + side_px(st.border_width.right)
                    } else {
                        0.0
                    };
                let border_box = matches!(st.box_sizing, Some(diting_css::BoxSizing::BorderBox));
                let infl = if border_box { 0.0 } else { pb };
                sized.insert(tnid, content + infl);
                if let Ok(mut ts) = taffy_tree.style(tnid).cloned() {
                    ts.size.width = Dimension::length(content + infl);
                    let _ = taffy_tree.set_style(tnid, ts);
                }
            }
            if float_fixups {
                bust_subtree_cache(icb_node, taffy_tree);
            }
            let re = taffy_tree.compute_layout_with_block_measure(
                icb_node,
                available,
                |inputs, id, ctx, style, block| {
                    float_lines::measure_node(fonts, inputs, id, ctx, style, block)
                },
            );
            if re.is_err() {
                return Err(SolveAborted);
            }
        }
    }
    Ok(())
}
