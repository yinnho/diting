//! Float zones: a float and the in-flow siblings that share its band.
//!
//! Split from the layout hub. `build_element_inner` still decides when a
//! zone applies and calls `build_float_zone`. Pure move: these functions
//! see the same crate namespace they grew up in.

use super::*;

/// Rough content-height estimate for a float (8g; upstream
/// estimate_float_height): explicit px heights when present, else one line
/// per structural row (p/li/tr/hN) plus a character-count text estimate.
/// Deliberately rough — it only decides how much flow shares the float's
/// band. The 200px floor covers the no-information case (an empty/icon
/// float); an explicit or content-derived estimate is almost always better
/// than a generous floor.
fn estimate_float_height(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    id: NodeId,
) -> f32 {
    let mut est: f32 = styles
        .get(&id)
        .and_then(|s| s.height)
        .map(|h| match h {
            diting_css::Length::Px(v) => v,
            _ => 0.0,
        })
        .unwrap_or(0.0);
    fn estimate_into(
        tree: &DomTree,
        id: NodeId,
        styles: &HashMap<NodeId, ComputedStyle>,
        est: &mut f32,
    ) {
        let tag = tree
            .with_node(id, |n| n.as_element().map(|e| e.local.to_string()))
            .flatten()
            .unwrap_or_default();
        let st = styles.get(&id);
        if let Some(diting_css::Length::Px(v)) = st.and_then(|s| s.height) {
            *est += v;
            return;
        }
        if matches!(
            tag.as_str(),
            "li" | "tr"
                | "dt"
                | "dd"
                | "p"
                | "figcaption"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
        ) {
            *est += st.and_then(|s| s.font_size).unwrap_or(16.0) * 1.2;
        }
        let text_len = tree
            .with_node(id, |n| {
                n.text_content_of_text_node()
                    .map(|t| t.chars().filter(|c| !c.is_whitespace()).count())
                    .unwrap_or(0)
            })
            .unwrap_or(0) as f32;
        if text_len > 0.0 {
            let fsize = st.and_then(|s| s.font_size).unwrap_or(16.0);
            // ~0.55em average glyph advance at an assumed 280px band.
            let chars_per_line = (280.0 / (fsize * 0.55)).max(1.0);
            *est += (text_len / chars_per_line).ceil() * fsize * 1.2 + 16.0;
        }
        for child in render_children(tree, id) {
            estimate_into(tree, child, styles, est);
        }
    }
    for child in render_children(tree, id) {
        estimate_into(tree, child, styles, &mut est);
    }
    if est <= 0.0 {
        est = 200.0;
    }
    est
}

/// Where a float's zone ends (8b/8i): the first sibling from `start` that
/// clears `run_side` (the clearfix idiom), the next float, or the sibling
/// whose flow estimate completes the float's height budget (8g) — real float
/// reflow ends when normal flow passes the float's bottom edge. Without the
/// budget a short float drags every following section into the narrow
/// column.
fn zone_end_at_budget(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    child_ids: &[NodeId],
    start: usize,
    budget: f32,
    run_side: Option<diting_css::FloatSide>,
) -> usize {
    let is_float = |cid: &NodeId| -> bool {
        styles
            .get(cid)
            .is_some_and(|s| s.float_side.is_some() && s.display != Some(CssDisplay::None))
    };
    let mut zone_end = child_ids.len();
    let mut flow_estimate = 0.0f32;
    const ASSUMED_FLOW_WIDTH: f32 = 500.0;
    for (i, cid) in child_ids.iter().enumerate().skip(start) {
        let clears_this = styles
            .get(cid)
            .and_then(|s| s.clear_side)
            .is_some_and(|c| match c {
                diting_css::ClearSide::Both => true,
                // Logical keywords resolve against this engine's LTR-only mode.
                diting_css::ClearSide::InlineStart => run_side == Some(diting_css::FloatSide::Left),
                diting_css::ClearSide::InlineEnd => run_side == Some(diting_css::FloatSide::Right),
                diting_css::ClearSide::Left => run_side == Some(diting_css::FloatSide::Left),
                diting_css::ClearSide::Right => run_side == Some(diting_css::FloatSide::Right),
            });
        if clears_this || is_float(cid) {
            zone_end = i;
            break;
        }
        // Rough per-sibling height contribution at the assumed width:
        // explicit px height wins, else structural-row lines plus a
        // character-count wrap estimate (same heuristic as the float side).
        let st = styles.get(cid);
        // Whitespace-only text between blocks contributes nothing.
        let ws_text = tree
            .with_node(*cid, |n| {
                !n.is_element()
                    && n.text_content_of_text_node()
                        .is_some_and(|t| t.trim().is_empty())
            })
            .unwrap_or(false);
        if ws_text {
            continue;
        }
        let mut contrib = st
            .and_then(|s| s.height)
            .map(|h| match h {
                diting_css::Length::Px(v) => v.max(0.0),
                _ => 0.0,
            })
            .unwrap_or(0.0);
        let fsize = st.and_then(|s| s.font_size).unwrap_or(16.0);
        let mut chars = 0.0f32;
        fn count_text(tree: &DomTree, id: NodeId, chars: &mut f32) {
            *chars += tree
                .with_node(id, |n| {
                    n.text_content_of_text_node()
                        .map(|t| t.chars().filter(|c| !c.is_whitespace()).count() as f32)
                        .unwrap_or(0.0)
                })
                .unwrap_or(0.0);
            for c in render_children(tree, id) {
                count_text(tree, c, chars);
            }
        }
        count_text(tree, *cid, &mut chars);
        if chars > 0.0 {
            let chars_per_line = (ASSUMED_FLOW_WIDTH / (fsize * 0.55)).max(1.0);
            contrib += (chars / chars_per_line).ceil() * fsize * 1.2;
        }
        if contrib <= 0.0 {
            contrib = fsize * 1.2;
        }
        flow_estimate += contrib;
        if flow_estimate >= budget {
            zone_end = i + 1;
            break;
        }
    }
    zone_end
}

/// The 8h document-order zone walk of `build_float_zone` (moved out
/// verbatim): normal siblings append to `direct`, float runs and their
/// following anonymous siblings become zones, `clear` ends a zone.
#[allow(clippy::too_many_arguments)]
fn float_zone_walk(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    flattened: &mut HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    meta: &mut TableBuildMeta,
    style: &ComputedStyle,
    child_ids: &[NodeId],
    atomic_container: bool,
    font_size: f32,
    lh_elem: f32,
    is_float_child: &impl Fn(&NodeId) -> bool,
    is_whitespace_text: &impl Fn(&NodeId) -> bool,
    is_empty_bridge: &impl Fn(&NodeId) -> bool,
    direct: &mut Vec<diting_taffy::tree::NodeId>,
) {
    // --- 8h: the document-order zone walk --------------------------------
    // The cursor walks child_ids once. Normal siblings append to direct
    // where they stand; a float opens its zone AT its document position
    // and the walk resumes after the zone. This is what keeps multiple
    // zones (infobox → sidebars → prose) in source order, and stops
    // later floats from falling into build_normal_sibling's plain-block
    // path (the bug that parked wikipedia's sidebar tables above the
    // lead prose).
    let mut cursor = 0usize;
    while cursor < child_ids.len() {
        let child = child_ids[cursor];
        if !is_float_child(&child) {
            build_normal_sibling(
                child,
                tree,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
                atomic_container,
                font_size,
                lh_elem,
                direct,
            );
            cursor += 1;
            continue;
        }
        let float_idx = cursor;
        let run_side = styles.get(&child_ids[float_idx]).and_then(|s| s.float_side);
        // Extend the run across consecutive same-side floats, skipping
        // formatting whitespace between them.
        let mut run_end = float_idx + 1;
        while run_end < child_ids.len() {
            let rc = child_ids[run_end];
            let extends_run =
                is_float_child(&rc) && styles.get(&rc).and_then(|s| s.float_side) == run_side;
            if extends_run || is_whitespace_text(&rc) {
                run_end += 1;
            } else {
                break;
            }
        }
        let run_len = (float_idx..run_end)
            .filter(|&i| is_float_child(&child_ids[i]))
            .count();

        // --- 8d: opposing float pair on one band -------------------------
        // The classic left-logo / right-tagline header: a float followed
        // (possibly across empty bridge siblings) by an OPPOSITE-side float
        // shares one band — the left float hugs the left edge, the right
        // float the right edge. Reified as a space-between row; a multi-
        // float run packs into an inner wrapping row first.
        let opposite_side = match run_side {
            Some(diting_css::FloatSide::Left) => Some(diting_css::FloatSide::Right),
            Some(diting_css::FloatSide::Right) => Some(diting_css::FloatSide::Left),
            None => None,
        };
        let mut bridge_end = run_end;
        while bridge_end < child_ids.len() && is_empty_bridge(&child_ids[bridge_end]) {
            bridge_end += 1;
        }
        let opposite_at = (bridge_end < child_ids.len()
            && is_float_child(&child_ids[bridge_end])
            && styles
                .get(&child_ids[bridge_end])
                .and_then(|s| s.float_side)
                == opposite_side)
            .then_some(bridge_end);
        if let Some(opp_idx) = opposite_at {
            let mut pair_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
            if run_len >= 2 {
                let mut inner: Vec<diting_taffy::tree::NodeId> = Vec::new();
                for cid in &child_ids[float_idx..run_end] {
                    if !is_float_child(cid) {
                        continue;
                    }
                    if let Some(f) = build_element(
                        tree,
                        *cid,
                        styles,
                        images,
                        fonts,
                        taffy_tree,
                        node_map,
                        flattened,
                        run_wrappers,
                        meta,
                    ) {
                        inner.push(f);
                    }
                }
                let inner_style = Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FLEX_START,
                    ..Default::default()
                };
                if let Ok(row) = taffy_tree.new_with_children(inner_style, &inner) {
                    pair_children.push(row);
                }
            } else if let Some(f) = build_element(
                tree,
                child_ids[float_idx],
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                pair_children.push(f);
            }
            if let Some(o) = build_element(
                tree,
                child_ids[opp_idx],
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                pair_children.push(o);
            }
            let pair_style = Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SPACE_BETWEEN,
                align_items: AlignItems::FLEX_START,
                size: Size {
                    width: percent(1.0),
                    height: auto(),
                },
                ..Default::default()
            };
            if let Ok(row) = taffy_tree.new_with_children(pair_style, &pair_children) {
                direct.push(row);
            }
            // The walk resumes after the pair: earlier siblings already
            // appended above, later content (another float included) takes
            // the next loop turn.
            cursor = opp_idx + 1;
            continue;
        }

        // --- 8i: the same-side float rail ---------------------------------
        // Same-side floats separated ONLY by empty bridges — wikipedia
        // parks mw-empty-elt spans between its infobox and the sidebar
        // tables, whose CSS stacks them with clear. ONE vertical rail at the
        // inline end plus ONE flow column spanning the whole rail: the lead
        // text starts beside the FIRST float's band, like a real browser's
        // float rail, instead of stacking each float in its own zone row
        // above the text.
        if opposite_at.is_none()
            && bridge_end < child_ids.len()
            && is_float_child(&child_ids[bridge_end])
            && styles
                .get(&child_ids[bridge_end])
                .and_then(|s| s.float_side)
                == run_side
        {
            // Keep absorbing bridge-separated same-side floats into the rail.
            let mut rail_end = bridge_end + 1;
            loop {
                let mut b = rail_end;
                while b < child_ids.len() && is_empty_bridge(&child_ids[b]) {
                    b += 1;
                }
                if b < child_ids.len()
                    && is_float_child(&child_ids[b])
                    && styles.get(&child_ids[b]).and_then(|s| s.float_side) == run_side
                {
                    rail_end = b + 1;
                } else {
                    break;
                }
            }
            let rail_idx: Vec<usize> = (float_idx..rail_end)
                .filter(|&i| is_float_child(&child_ids[i]))
                .collect();
            if rail_idx.len() >= 2 {
                // The budget spans the WHOLE rail: the flow column shares
                // the band with every rail float combined.
                let budget: f32 = rail_idx
                    .iter()
                    .map(|&i| estimate_float_height(tree, styles, child_ids[i]))
                    .sum();
                let zone_end = zone_end_at_budget(
                    tree,
                    styles,
                    child_ids,
                    rail_idx[rail_idx.len() - 1] + 1,
                    budget,
                    run_side,
                );
                let flow_children = build_flow_column(
                    &child_ids[rail_end..zone_end],
                    tree,
                    styles,
                    images,
                    fonts,
                    taffy_tree,
                    node_map,
                    flattened,
                    run_wrappers,
                    meta,
                    font_size,
                    lh_elem,
                    style.white_space.unwrap_or(WhiteSpace::Normal),
                );
                let float_right = run_side == Some(diting_css::FloatSide::Right);
                let mut rail_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
                for &i in &rail_idx {
                    if let Some(f) = build_element(
                        tree,
                        child_ids[i],
                        styles,
                        images,
                        fonts,
                        taffy_tree,
                        node_map,
                        flattened,
                        run_wrappers,
                        meta,
                    ) {
                        rail_children.push(f);
                    }
                }
                let column_style = Style {
                    display: Display::Block,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    // #176: content-based, not zero — a zero hypothetical
                    // main size makes every intrinsic-width read (an abspos
                    // shrink-to-fit container's measure) size the float row
                    // to the float alone, dropping the flow content's width.
                    // Definite-width geometry is unchanged: grow fills the
                    // leftover either way, and with the float/rail at shrink
                    // 0 all shrinkage lands here.
                    flex_basis: Dimension::auto(),
                    min_size: Size {
                        width: LengthPercentageAuto::length(0.0),
                        height: LengthPercentageAuto::auto(),
                    },
                    ..Default::default()
                };
                let column = (!flow_children.is_empty())
                    .then(|| {
                        taffy_tree
                            .new_with_children(column_style, &flow_children)
                            .ok()
                    })
                    .flatten();
                if let Ok(rail) = taffy_tree.new_with_children(
                    Style {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        // #176: the rail holds floats and never shrinks —
                        // with the flow column's content-based flex-basis,
                        // negative free space in the row must compress the
                        // column (wrapping), not the floats' widths.
                        flex_shrink: 0.0,
                        // Inline-end alignment: every float hugs the rail's
                        // outer edge (right floats hug right).
                        align_items: if float_right {
                            AlignItems::FLEX_END
                        } else {
                            AlignItems::FLEX_START
                        },
                        ..Default::default()
                    },
                    &rail_children,
                ) {
                    let row_children: Vec<diting_taffy::tree::NodeId> = match (float_right, column)
                    {
                        (true, Some(col)) => vec![col, rail],
                        (true, None) => vec![rail],
                        (false, Some(col)) => vec![rail, col],
                        (false, None) => vec![rail],
                    };
                    let row_style = Style {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FLEX_START,
                        size: Size {
                            width: percent(1.0),
                            height: auto(),
                        },
                        ..Default::default()
                    };
                    if let Ok(row) = taffy_tree.new_with_children(row_style, &row_children) {
                        direct.push(row);
                    } else {
                        for n in row_children {
                            direct.push(n);
                        }
                    }
                } else {
                    // Degenerate rail failure: never lose content.
                    for n in rail_children {
                        direct.push(n);
                    }
                    for n in flow_children {
                        direct.push(n);
                    }
                }
                cursor = zone_end;
                continue;
            }
        }

        if run_len >= 2 {
            // --- 8c: the wrapping float-grid row -------------------------
            let mut row_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
            for cid in &child_ids[float_idx..run_end] {
                if !is_float_child(cid) {
                    continue;
                }
                if let Some(f) = build_element(
                    tree,
                    *cid,
                    styles,
                    images,
                    fonts,
                    taffy_tree,
                    node_map,
                    flattened,
                    run_wrappers,
                    meta,
                ) {
                    row_children.push(f);
                }
            }
            let row_style = Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::FLEX_START,
                // The row IS the band's inline size: definite width so
                // percentage-width floats wrap against the real container,
                // not an intrinsic pre-stretch guess.
                size: Size {
                    width: percent(1.0),
                    height: auto(),
                },
                ..Default::default()
            };
            if let Ok(row) = taffy_tree.new_with_children(row_style, &row_children) {
                direct.push(row);
            }
            // The walk resumes after the run: another float starts its own
            // zone/run, cleared or plain siblings take normal flow.
            cursor = run_end;
            continue;
        }

        // --- 8b: single float + flow column ------------------------------
        // Zone end: the first sibling that clears this float's side (the
        // clearfix idiom), or the next float, or the point where the flow
        // siblings have already filled an ESTIMATE of the float's height
        // (8g) — see zone_end_at_budget.
        let float_height_budget = estimate_float_height(tree, styles, child_ids[float_idx]);
        let zone_end = zone_end_at_budget(
            tree,
            styles,
            child_ids,
            float_idx + 1,
            float_height_budget,
            run_side,
        );
        // Build the float itself (blockified into the row's first item).
        let float_dom = child_ids[float_idx];
        let float_taffy = build_element(
            tree,
            float_dom,
            styles,
            images,
            fonts,
            taffy_tree,
            node_map,
            flattened,
            run_wrappers,
            meta,
        );
        // The flow column: an ANONYMOUS block wrapper around every in-zone
        // sibling built normally inside it (see build_flow_column).
        let flow_children = build_flow_column(
            &child_ids[float_idx + 1..zone_end],
            tree,
            styles,
            images,
            fonts,
            taffy_tree,
            node_map,
            flattened,
            run_wrappers,
            meta,
            font_size,
            lh_elem,
            style.white_space.unwrap_or(WhiteSpace::Normal),
        );
        let float_right =
            styles.get(&float_dom).and_then(|s| s.float_side) == Some(diting_css::FloatSide::Right);
        let mut row_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
        if !flow_children.is_empty() {
            if let Ok(column) = taffy_tree.new_with_children(
                Style {
                    display: Display::Block,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    // #176: content-based, not zero — a zero hypothetical
                    // main size makes every intrinsic-width read (an abspos
                    // shrink-to-fit container's measure) size the float row
                    // to the float alone, dropping the flow content's width.
                    // Definite-width geometry is unchanged: grow fills the
                    // leftover either way, and with the float/rail at shrink
                    // 0 all shrinkage lands here.
                    flex_basis: Dimension::auto(),
                    min_size: Size {
                        width: LengthPercentageAuto::length(0.0),
                        height: LengthPercentageAuto::auto(),
                    },
                    ..Default::default()
                },
                &flow_children,
            ) {
                row_children.push(column);
            }
        }
        // A right float hugs the container's right edge — the row's LAST
        // item (CSS places right floats at the inline-end).
        if float_right {
            row_children.extend(float_taffy);
        } else {
            for f in float_taffy.into_iter().rev() {
                row_children.insert(0, f);
            }
        }
        let row_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FLEX_START,
            // With an empty flow column the float is the row's ONLY item —
            // flex-end keeps a lone right float at the inline-end instead of
            // parking it at flex-start (column-present rows have no free
            // space, so this is a no-op there).
            justify_content: if float_right {
                JustifyContent::FLEX_END
            } else {
                JustifyContent::NORMAL
            },
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..Default::default()
        };
        if let Ok(row) = taffy_tree.new_with_children(row_style, &row_children) {
            direct.push(row);
        }
        // The walk resumes after the zone: zone-external siblings (clear,
        // later floats) take the next loop turn at their document position.
        cursor = zone_end;
    }
}

/// The float-zone child partition of `build_element_inner` (moved out
/// verbatim, batch 8b/8c/8e/8h): float runs and their anonymous
/// siblings mount as zones in document order. `Some(node)` is a finished
/// element (the 8e right-float navigation bar) the caller returns as is.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_float_zone(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    flattened: &mut HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    meta: &mut TableBuildMeta,
    style: &ComputedStyle,
    child_ids: &[NodeId],
    atomic_container: bool,
    font_size: f32,
    lh_elem: f32,
    is_float_child: &impl Fn(&NodeId) -> bool,
    direct: &mut Vec<diting_taffy::tree::NodeId>,
) -> Option<diting_taffy::tree::NodeId> {
    let is_whitespace_text = |cid: &NodeId| -> bool {
        tree.with_node(*cid, |n| {
            !n.is_element()
                && n.text_content_of_text_node()
                    .is_some_and(|t| t.trim().is_empty())
        })
        .unwrap_or(false)
    };
    // An empty bridge sibling (upstream is_empty_bridge): whitespace
    // text OR an element with no authored size/margin/padding/border
    // whose subtree paints nothing — the legacy compatibility boxes real
    // pages park between the two header floats, and MediaWiki's
    // mw-empty-elt wrappers (link/meta cargo included) between float
    // runs.
    let is_empty_bridge = |cid: &NodeId| -> bool {
        if is_whitespace_text(cid) {
            return true;
        }
        if !subtree_paints_nothing(tree, *cid) {
            return false;
        }
        styles.get(cid).is_some_and(|s| {
            s.width.is_none()
                && s.height.is_none()
                && s.margin.top.is_none()
                && s.margin.right.is_none()
                && s.margin.bottom.is_none()
                && s.margin.left.is_none()
                && s.padding.top.is_none()
                && s.padding.right.is_none()
                && s.padding.bottom.is_none()
                && s.padding.left.is_none()
                && s.border_style.is_none()
        })
    };

    // --- 8e: the right-float navigation bar --------------------------
    // A container of inline-ish flow content plus >=2 RIGHT floats and
    // no left float: right floats place from the inline-end inward —
    // visual order REVERSED vs source — while ordinary content fills
    // from the band's start. Reified as [flow items | reversed
    // right-float group], an anonymous wrapping row at definite width.
    let first_float_side = child_ids
        .iter()
        .find(|cid| is_float_child(cid))
        .and_then(|cid| styles.get(cid).and_then(|s| s.float_side));
    let all_right = first_float_side == Some(diting_css::FloatSide::Right)
        && !child_ids.iter().any(|cid| {
            styles.get(cid).and_then(|s| s.float_side) == Some(diting_css::FloatSide::Left)
        });
    let flow_is_inline = child_ids.iter().all(|cid| {
        if is_float_child(cid) || is_empty_bridge(cid) {
            return true;
        }
        styles
            .get(cid)
            .is_none_or(|s| s.display != Some(CssDisplay::Block))
    });
    let right_floats: Vec<NodeId> = child_ids
        .iter()
        .copied()
        .filter(|cid| {
            styles.get(cid).is_some_and(|s| {
                s.float_side == Some(diting_css::FloatSide::Right)
                    && s.display != Some(CssDisplay::None)
            })
        })
        .collect();
    if all_right && flow_is_inline && right_floats.len() >= 2 {
        // Flow items in source order; runs of formatting whitespace
        // collapse to one representative node and drop at band edges.
        let mut flow_dom: Vec<NodeId> = Vec::new();
        let mut pending_ws: Option<NodeId> = None;
        let mut has_flow_content = false;
        for &cid in child_ids {
            if is_float_child(&cid) {
                continue;
            }
            if is_whitespace_text(&cid) {
                if has_flow_content && pending_ws.is_none() {
                    pending_ws = Some(cid);
                }
                continue;
            }
            if has_flow_content {
                if let Some(ws) = pending_ws.take() {
                    flow_dom.push(ws);
                }
            } else {
                pending_ws = None;
            }
            flow_dom.push(cid);
            has_flow_content = true;
        }
        let mut row_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
        for cid in flow_dom {
            build_normal_sibling(
                cid,
                tree,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
                atomic_container,
                font_size,
                lh_elem,
                &mut row_children,
            );
        }
        // The right-float group in REVERSED source order (CSS places
        // right floats inline-end first).
        let mut right_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
        for cid in right_floats.iter().rev() {
            if let Some(f) = build_element(
                tree,
                *cid,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                right_children.push(f);
            }
        }
        if !row_children.is_empty() && !right_children.is_empty() {
            let group_style = Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: diting_taffy::geometry::Rect {
                    top: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                    bottom: LengthPercentageAuto::length(0.0),
                    left: LengthPercentageAuto::auto(),
                },
                ..Default::default()
            };
            if let Ok(group) = taffy_tree.new_with_children(group_style, &right_children) {
                row_children.push(group);
            }
            let bar_style = Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::FLEX_START,
                size: Size {
                    width: percent(1.0),
                    height: auto(),
                },
                ..Default::default()
            };
            if let Ok(bar) = taffy_tree.new_with_children(bar_style, &row_children) {
                direct.push(bar);
            }
        } else {
            // Degenerate side empty: fall back to plain normal flow.
            for n in row_children.drain(..) {
                direct.push(n);
            }
            for f in right_children.drain(..) {
                direct.push(f);
            }
        }
        collapse_adjacent_sibling_margins(taffy_tree, styles, node_map, direct);
        let taffy_style = to_taffy_style(style, pct_height_resolves(tree, styles, id));
        if is_alignment_promote(&taffy_style) {
            stretch_block_children_in_aligned_parent(taffy_tree, node_map, styles, direct);
        }
        let node = if direct.is_empty() {
            taffy_tree.new_leaf(taffy_style).ok()?
        } else {
            taffy_tree.new_with_children(taffy_style, direct).ok()?
        };
        node_map.insert(node, id);
        return Some(node);
    }
    float_zone_walk(
        tree,
        styles,
        images,
        fonts,
        taffy_tree,
        node_map,
        flattened,
        run_wrappers,
        meta,
        style,
        child_ids,
        atomic_container,
        font_size,
        lh_elem,
        is_float_child,
        &is_whitespace_text,
        &is_empty_bridge,
        direct,
    );
    None
}
