//! Table layout (`display: table` family).
//!
//! Split from the layout hub. `build_table` reifies rows as flex rows of
//! cells; `place_table_spans` pins lifted rowspan/colspan cells onto the
//! grid the solve just produced. Pure move: these functions see the same
//! crate namespace they grew up in.

use super::*;

/// A `rowspan > 1` cell lifted out of its row wrapper. Its grid slot stays
/// occupied by an invisible placeholder leaf (no `node_map` entry, so paint
/// and hit-testing skip it) that keeps the column-group geometry, while the
/// cell itself becomes an absolutely-positioned child of the table node —
/// the post-layout fixup pass resolves its insets from the final row and
/// placeholder boxes.
struct SpanCell {
    taffy: diting_taffy::tree::NodeId,
    placeholder: diting_taffy::tree::NodeId,
    dom: NodeId,
    col: usize,
    col_span: usize,
    row: usize,
    row_span: usize,
}

/// One table's build-side record for the post-layout span fixup: the row
/// wrappers tagged with their GRID row index (rows without cells generate
/// no wrapper) and the border inset the absolute offsets resolve against.
struct TableSpans {
    rows: Vec<(usize, diting_taffy::tree::NodeId)>,
    cells: Vec<SpanCell>,
    origin: (f32, f32),
}

/// Build-time accumulators threaded through `build_element` for the table
/// engine: the fixup jobs (consumed after the main layout) and the
/// collapsed-border edge marks (consumed by the paint walk).
#[derive(Default)]
pub(crate) struct TableBuildMeta {
    span_jobs: Vec<TableSpans>,
    /// dom id → [top, right, bottom, left]: that border side sits on a
    /// shared grid line under `border-collapse` → paint it at half width
    /// so two adjacent 2px borders read as one 2px line (Chrome centers
    /// the resolved border on the grid line; halves reproduce that for
    /// equal widths. Width conflicts keep proportional halves, not
    /// winner-takes-all — a documented v2 approximation).
    pub(super) collapsed_edges: HashMap<NodeId, [bool; 4]>,
}

/// The `vertical-align` slot (declaration or legacy `valign` attribute,
/// already merged in the cascade) as the cell-content justify rule.
fn cell_valign_justify(styles: &HashMap<NodeId, ComputedStyle>, dom: &NodeId) -> JustifyContent {
    match styles.get(dom).and_then(|s| s.vertical_align) {
        Some(diting_css::VerticalAlign::Top) | Some(diting_css::VerticalAlign::Baseline) => {
            JustifyContent::FLEX_START
        }
        Some(diting_css::VerticalAlign::Bottom) => JustifyContent::FLEX_END,
        // Absent/unknown = Chrome's UA middle default.
        _ => JustifyContent::CENTER,
    }
}

// Rows in document order, row groups flattened; CSS `display: table-row`
// children count as rows too (the CSS-authored minimum). The caption and
// the colgroup/col width sources ride the same scan. `row_group` keeps
// each row's thead/tbody/tfoot ancestor (parallel to `rows`) so the
// wrapper assembly can re-group what this scan flattens — a group with
// a real (stand-in) box gets rects/gBCR/sticky for free, and the box is
// what `position: sticky` on a row group keys off in Chrome.
//
// CSS2.2 §17.2.1's anonymous-ROW half: a run of consecutive table
// children that are not rows — table-cells most of all (Fusion's
// `.next-input` puts `display: table-cell` spans directly inside its
// `display: inline-table` root), stray elements, or non-blank text —
// wraps into ONE anonymous row. Previously those children matched no
// arm and vanished, so an inline-table laid out empty.
enum RowSrc {
    Tr(NodeId),
    Anon(Vec<NodeId>),
}

fn flush_anon(
    pending: &mut Vec<NodeId>,
    rows: &mut Vec<RowSrc>,
    row_group: &mut Vec<Option<NodeId>>,
) {
    if !pending.is_empty() {
        rows.push(RowSrc::Anon(std::mem::take(pending)));
        row_group.push(None);
    }
}

// Grid placement (spans batch): each cell claims the leftmost run of
// free column slots wide enough for its colspan; a rowspan cell also
// occupies those slots in the FOLLOWING rows, so later rows skip under
// it — HTML table formatting. Ragged rows keep the leftmost-placement
// behavior of the pre-span engine (no spans ⇒ column == cell index,
// byte-identical attribution to the old path).
struct CellSlot {
    taffy: diting_taffy::tree::NodeId,
    dom: NodeId,
    col: usize,
    col_span: usize,
    row: usize,
    row_span: usize,
    /// An invisible slot-holder for a lifted rowspan cell — carries the
    /// column basis but paints nothing and takes no valign treatment.
    phantom: bool,
    /// A synthesized anonymous cell (CSS2.2 §17.2.1): no DOM box of its
    /// own, so it takes no collapsed-edge marks and never pins a fixed
    /// column from a member's style.
    anon: bool,
}

// CSS2.2 §17.2.1 anonymous cell synthesis: a run of consecutive row
// children that are not table-cells — bare text most of all (a real
// HTML parser foster-parents it out of the table; ours keeps it in
// place, and skipping it here rendered the content invisible) or stray
// elements — wraps into ONE anonymous cell box that claims a column
// slot like any td. Whitespace-only text, comments, and display:none
// elements generate no box.
enum RowItem {
    Cell(NodeId),
    Anon(Vec<NodeId>),
}

/// Per-row cell build of `build_table` (moved out verbatim): grid placement
/// of each cell (rowspan/colspan claims, placeholders owed to rows below),
/// anonymous cell synthesis and the row wrappers.
#[allow(clippy::too_many_arguments)]
fn build_table_rows(
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
    rows: &[RowSrc],
    col_authored: &mut Vec<Option<f32>>,
    fixed: bool,
    gap: f32,
    n_rows: usize,
    span_attr: &impl Fn(NodeId, &str) -> usize,
    occupied: &mut [Vec<bool>],
    pend_ph: &mut [Vec<(diting_taffy::tree::NodeId, NodeId, usize, usize)>],
    row_wrappers: &mut Vec<(usize, diting_taffy::tree::NodeId, Vec<CellSlot>)>,
    span_cells: &mut Vec<SpanCell>,
) {
    for (row_idx, row_src) in rows.iter().enumerate() {
        // An anonymous row has no DOM node: its members are the table's own
        // children, and font context inherits from the table itself.
        let (row_dom, members): (NodeId, Vec<NodeId>) = match row_src {
            RowSrc::Tr(rid) => (*rid, tree.children(*rid)),
            RowSrc::Anon(members) => (id, members.clone()),
        };
        let mut items: Vec<RowItem> = Vec::new();
        for cid in members {
            if styles.get(&cid).and_then(|s| s.display) == Some(CssDisplay::TableCell) {
                items.push(RowItem::Cell(cid));
                continue;
            }
            if tree.with_node(cid, |n| n.is_text()).unwrap_or(false) {
                let blank = tree
                    .with_node(cid, |n| {
                        n.text_content_of_text_node()
                            .unwrap_or("")
                            .trim()
                            .is_empty()
                    })
                    .unwrap_or(true);
                if blank {
                    continue;
                }
            } else if tree
                .with_node(cid, |n| n.as_element().map(|_| ()))
                .flatten()
                .is_none()
                || styles.get(&cid).and_then(|s| s.display) == Some(CssDisplay::None)
            {
                continue;
            }
            match items.last_mut() {
                Some(RowItem::Anon(group)) => group.push(cid),
                _ => items.push(RowItem::Anon(vec![cid])),
            }
        }
        let (row_fs, _, row_lh) = font_context(tree, row_dom, styles, fonts);
        let mut cells: Vec<CellSlot> = Vec::new();
        for item in items {
            let (cell_dom, col_span, anon) = match &item {
                RowItem::Cell(cid) => (*cid, span_attr(*cid, "colspan").min(1000), false),
                RowItem::Anon(group) => (group[0], 1, true),
            };
            let row_span = if anon {
                1
            } else {
                span_attr(cell_dom, "rowspan")
                    .min(65534)
                    .min(n_rows - row_idx)
                    .max(1)
            };
            // Leftmost run of col_span free slots in this row.
            let mut col = 0usize;
            loop {
                while occupied[row_idx].len() < col + col_span {
                    occupied[row_idx].push(false);
                }
                if (col..col + col_span).all(|i| !occupied[row_idx][i]) {
                    break;
                }
                col += 1;
            }
            let Some(cell_node) = (match &item {
                RowItem::Cell(cid) => build_element(
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
                ),
                RowItem::Anon(group) => {
                    // Members build through the normal sibling path (text
                    // runs, inline flattening, replaced atoms included).
                    // An all-inline group sits on one shared line (the same
                    // flex row wrap a text run rides); a block-level member
                    // stacks the group vertically — the block-in-inline
                    // split reads top to bottom.
                    let mut kids: Vec<diting_taffy::tree::NodeId> = Vec::new();
                    for &mid in group {
                        build_normal_sibling(
                            mid,
                            tree,
                            styles,
                            images,
                            fonts,
                            taffy_tree,
                            node_map,
                            flattened,
                            run_wrappers,
                            meta,
                            false,
                            row_fs,
                            row_lh,
                            &mut kids,
                        );
                    }
                    if kids.is_empty() {
                        None
                    } else {
                        let any_block = group.iter().any(|&mid| {
                            if tree.with_node(mid, |n| n.is_text()).unwrap_or(false) {
                                return false;
                            }
                            // Same default as build_normal_sibling: an element
                            // with no computed display is block-level.
                            !matches!(
                                styles.get(&mid).and_then(|s| s.display),
                                Some(CssDisplay::Inline) | Some(CssDisplay::InlineBlock)
                            )
                        });
                        // Two-level shape, mirroring a real cell: the OUTER
                        // node is the cell the pin pass restyles into a
                        // valign flex-COLUMN (a single-level row-wrap container
                        // would get clobbered into a column and stack its run
                        // wrappers); the inner node carries the content.
                        let inner = if any_block {
                            // Block-level member: the group stacks vertically
                            // — the block-in-inline split reads top to bottom.
                            taffy_tree
                                .new_with_children(
                                    Style {
                                        display: Display::Block,
                                        ..Default::default()
                                    },
                                    &kids,
                                )
                                .ok()
                        } else {
                            // All-inline: merge the per-member run wrappers
                            // into ONE wrapper, the same shape a real cell's
                            // mixed run builds — nested wrappers inside a
                            // wrap container wrap at exact-fit widths. Kids
                            // are reparented before the empty shells drop
                            // (the span-flatten order at the sibling path).
                            // ONLY true run wrappers are shells: a bare
                            // member in `kids` — an out-of-flow replaced
                            // atom leaf or abs-positioned inline box pushed
                            // straight to `direct` — owns its node_map
                            // entry; unwrapping-and-freeing it would strand
                            // a stale SlotMap key and panic the reparent
                            // pass on every rebuild (#119).
                            let mut leaves: Vec<diting_taffy::tree::NodeId> = Vec::new();
                            let mut shells: Vec<diting_taffy::tree::NodeId> = Vec::new();
                            for w in &kids {
                                if run_wrappers.contains(w) {
                                    shells.push(*w);
                                    leaves.extend(
                                        taffy_tree.children(*w).unwrap_or_default().to_vec(),
                                    );
                                } else {
                                    leaves.push(*w);
                                }
                            }
                            for w in shells {
                                run_wrappers.retain(|r| r != &w);
                                let _ = taffy_tree.remove(w);
                            }
                            // #158: the merged run inherits the members' most
                            // restrictive white-space — a nowrap inline split
                            // by a block keeps its pieces on one line.
                            let ws = group.iter().fold(WhiteSpace::Normal, |acc, &mid| {
                                let is_text = tree.with_node(mid, |n| n.is_text()).unwrap_or(false);
                                let w = ifc_white_space(tree, styles, mid, is_text);
                                if ws_rank(w) > ws_rank(acc) {
                                    w
                                } else {
                                    acc
                                }
                            });
                            if ws.no_soft_wrap() {
                                pin_leaf_widths(taffy_tree, &leaves);
                            }
                            match taffy_tree.new_with_children(run_wrapper_style(ws), &leaves) {
                                Ok(wrapper) => {
                                    run_wrappers.push(wrapper);
                                    Some(wrapper)
                                }
                                Err(_) => None,
                            }
                        };
                        match inner {
                            Some(inner) => taffy_tree
                                .new_with_children(Style::default(), &[inner])
                                .ok(),
                            None => None,
                        }
                    }
                }
            }) else {
                continue; // display:none builds no box and claims no slot
            };
            // Fixed layout: a first-row single-column cell pins its column
            // (the computed width already folds the td/th `width` attribute
            // hint in below every author declaration). A `<col>` width
            // outranks it — CSS2.2 §17.5.2.1: the col element sets the
            // column, the first-row cell only fills a column the cols left
            // auto.
            if fixed && !anon && row_idx == 0 && col_span == 1 {
                let w = styles
                    .get(&cell_dom)
                    .and_then(|s| s.width)
                    .and_then(|l| match l {
                        diting_css::Length::Px(px) => Some(px),
                        _ => None,
                    })
                    .filter(|w| *w > 0.0);
                if let Some(px) = w {
                    if col_authored.len() < col + 1 {
                        col_authored.resize(col + 1, None);
                    }
                    if col_authored[col].is_none() {
                        col_authored[col] = Some(px);
                    }
                }
            }
            // Claim the slots in this row and (for a rowspan) the rows below.
            for occ in &mut occupied[row_idx..row_idx + row_span] {
                occ.resize(col + col_span, false);
                occ[col..col + col_span].fill(true);
            }
            if row_span > 1 {
                // Lift the cell out of the flow: an invisible placeholder
                // holds its slot in this row (sized later to the column
                // group, so the row keeps its geometry), the real box joins
                // the table node as an absolute child once the fixup pass
                // knows the final bands. Every continuation row gets its own
                // placeholder too — same slot, same column basis.
                if let Ok(placeholder) = taffy_tree.new_leaf(Style::default()) {
                    span_cells.push(SpanCell {
                        taffy: cell_node,
                        placeholder,
                        dom: cell_dom,
                        col,
                        col_span,
                        row: row_idx,
                        row_span,
                    });
                    cells.push(CellSlot {
                        taffy: placeholder,
                        dom: cell_dom,
                        col,
                        col_span,
                        row: row_idx,
                        row_span,
                        phantom: true,
                        anon,
                    });
                    for pend in &mut pend_ph[row_idx + 1..row_idx + row_span] {
                        if let Ok(ph) = taffy_tree.new_leaf(Style::default()) {
                            pend.push((ph, cell_dom, col, col_span));
                        }
                    }
                } else {
                    cells.push(CellSlot {
                        taffy: cell_node,
                        dom: cell_dom,
                        col,
                        col_span,
                        row: row_idx,
                        row_span,
                        phantom: false,
                        anon,
                    });
                }
            } else {
                cells.push(CellSlot {
                    taffy: cell_node,
                    dom: cell_dom,
                    col,
                    col_span,
                    row: row_idx,
                    row_span: 1,
                    phantom: false,
                    anon,
                });
            }
        }
        // Slot order is column order: with rowspans above, a later DOM cell
        // can land LEFT of an earlier one (the earlier one's leftmost free
        // run jumped past occupied slots). Stable sort keeps DOM order
        // within a column (impossible — slots are exclusive — but cheap).
        cells.extend(
            pend_ph[row_idx]
                .drain(..)
                .map(|(ph, dom, col, col_span)| CellSlot {
                    taffy: ph,
                    dom,
                    col,
                    col_span,
                    row: row_idx,
                    row_span: 1,
                    phantom: true,
                    anon: false,
                }),
        );
        cells.sort_by_key(|c| c.col);
        if cells.is_empty() {
            continue;
        }
        // The `height` attribute on tr (blitz#507) landed in the tr's
        // computed style as a px presentational hint; the wrapper is
        // synthetic, so carry it across here. Spec: it's a MINIMUM row
        // height — the row still grows for taller cells (STRETCH below).
        // An anonymous row has no tr: no authored height to carry.
        let row_height = match row_src {
            RowSrc::Tr(rid) => styles
                .get(rid)
                .and_then(|s| s.height)
                .and_then(|l| match l {
                    diting_css::Length::Px(px) => Some(px),
                    _ => None,
                }),
            RowSrc::Anon(_) => None,
        };
        let row_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::STRETCH,
            flex_wrap: FlexWrap::NoWrap,
            gap: diting_taffy::geometry::Size {
                width: LengthPercentage::length(gap),
                height: LengthPercentage::length(0.0),
            },
            size: diting_taffy::geometry::Size {
                width: auto(),
                height: row_height.map(Dimension::length).unwrap_or_else(auto),
            },
            ..Default::default()
        };
        let cell_nodes: Vec<diting_taffy::tree::NodeId> = cells.iter().map(|c| c.taffy).collect();
        if let Ok(row_node) = taffy_tree.new_with_children(row_style, &cell_nodes) {
            // Only a real tr owns a DOM node to attribute the wrapper to;
            // an anonymous row's wrapper stays a pure layout stand-in.
            if let RowSrc::Tr(rid) = row_src {
                node_map.insert(row_node, *rid);
            }
            row_wrappers.push((row_idx, row_node, cells));
        }
    }
}

/// Collapsed-border gutters of `build_table` (blitz #1005 absorption, #205;
/// moved out verbatim): under collapse the cell borders are the grid lines,
/// each cell carries half its gutters as padding. Returns (column, row)
/// gutter widths.
fn collapsed_gutters(
    style: &ComputedStyle,
    styles: &HashMap<NodeId, ComputedStyle>,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    n_rows: usize,
    row_wrappers: &[(usize, diting_taffy::tree::NodeId, Vec<CellSlot>)],
    span_cells: &[SpanCell],
) -> (Vec<f32>, Vec<f32>) {
    // --- collapsed-border gutters (blitz #1005 absorption, #205) ---------
    // Under collapse the cell borders ARE the grid lines: their space is a
    // shared gutter between cells, not per-cell border-box inflation.
    // Chrome lays `2×(width:100;border:10)` out at 220px; the per-cell
    // border model computed 240 (#205). Model: every grid line's gutter =
    // the max of the adjacent cell borders on it; each cell side reserves
    // HALF its gutters as padding, the layout border drops to zero, and
    // the paint walk's collapsed-edge halving centers internal lines on
    // the seam (each neighbor paints its half inside its own gutter).
    // Outer lines read at half weight — Chrome centers them on the table
    // edge with the outer half outside the box; v1 keeps them inside.
    let mut col_gutter: Vec<f32> = Vec::new();
    let mut row_gutter: Vec<f32> = vec![0.0; n_rows + 1];
    if matches!(
        style.border_collapse,
        Some(diting_css::BorderCollapse::Collapse)
    ) {
        let edge_px = |dom: NodeId| -> [f32; 4] {
            styles
                .get(&dom)
                .map(|s| {
                    if s.border_style.is_some() {
                        [
                            side_px(s.border_width.top),
                            side_px(s.border_width.right),
                            side_px(s.border_width.bottom),
                            side_px(s.border_width.left),
                        ]
                    } else {
                        [0.0; 4]
                    }
                })
                .unwrap_or([0.0; 4])
        };
        // (edges, col, col_span, row, row_span) — real cells only; anon
        // cells own no border style and phantoms paint nothing.
        let mut all: Vec<([f32; 4], usize, usize, usize, usize)> = Vec::new();
        let mut total_cols = 0usize;
        for (_, _, cells) in row_wrappers.iter() {
            for cell in cells {
                if cell.phantom || cell.anon {
                    continue;
                }
                total_cols = total_cols.max(cell.col + cell.col_span);
                all.push((
                    edge_px(cell.dom),
                    cell.col,
                    cell.col_span,
                    cell.row,
                    cell.row_span,
                ));
            }
        }
        for cell in span_cells.iter() {
            total_cols = total_cols.max(cell.col + cell.col_span);
            all.push((
                edge_px(cell.dom),
                cell.col,
                cell.col_span,
                cell.row,
                cell.row_span,
            ));
        }
        col_gutter = vec![0.0; total_cols + 1];
        for (e, col, cspan, row, rspan) in &all {
            col_gutter[*col] = col_gutter[*col].max(e[3]);
            col_gutter[*col + *cspan] = col_gutter[*col + *cspan].max(e[1]);
            row_gutter[*row] = row_gutter[*row].max(e[0]);
            row_gutter[*row + *rspan] = row_gutter[*row + *rspan].max(e[2]);
        }
        let patch_cell = |taffy_tree: &mut TaffyTree<TextLeaf>,
                          node: diting_taffy::tree::NodeId,
                          cell_dom: Option<NodeId>,
                          col: usize,
                          cspan: usize,
                          row: usize,
                          rspan: usize| {
            let Ok(mut st) = taffy_tree.style(node).cloned() else {
                return;
            };
            // Anonymous / phantom cells own no border, so the gutters (sized
            // from real cells) may not reach them.
            let half = |g: &[f32], i: usize| g.get(i).copied().unwrap_or(0.0) / 2.0;
            let gl = half(&col_gutter, col);
            let gr = half(&col_gutter, col + cspan);
            let gt = half(&row_gutter, row);
            let gb = half(&row_gutter, row + rspan);
            st.border = diting_taffy::geometry::Rect {
                top: LengthPercentage::length(0.0),
                right: LengthPercentage::length(0.0),
                bottom: LengthPercentage::length(0.0),
                left: LengthPercentage::length(0.0),
            };
            let widen = |cur: &LengthPercentage, half: f32| -> LengthPercentage {
                match cur.expand() {
                    diting_taffy::style::ExpandedLengthPercentage::Length(v) => {
                        LengthPercentage::length(v + half)
                    }
                    _ => *cur,
                }
            };
            st.padding.top = widen(&st.padding.top, gt);
            st.padding.right = widen(&st.padding.right, gr);
            st.padding.bottom = widen(&st.padding.bottom, gb);
            st.padding.left = widen(&st.padding.left, gl);
            // A content-box authored px size was mapped over by the OLD
            // border and padding at build time — retarget it at the new
            // gutter halves so the authored CONTENT size survives.
            if let Some(dom) = cell_dom {
                if let Some(cs) = styles.get(&dom) {
                    let border_box =
                        matches!(cs.box_sizing, Some(diting_css::BoxSizing::BorderBox));
                    let e = edge_px(dom);
                    if !border_box && matches!(cs.width, Some(diting_css::Length::Px(_))) {
                        if let diting_taffy::style::ExpandedDimension::Length(w) =
                            st.size.width.expand()
                        {
                            st.size.width = Dimension::length(w - e[1] - e[3] + gl + gr);
                        }
                    }
                    if !border_box && matches!(cs.height, Some(diting_css::Length::Px(_))) {
                        if let diting_taffy::style::ExpandedDimension::Length(h) =
                            st.size.height.expand()
                        {
                            st.size.height = Dimension::length(h - e[0] - e[2] + gt + gb);
                        }
                    }
                }
            }
            let _ = taffy_tree.set_style(node, st);
        };
        for (_, _, cells) in row_wrappers.iter() {
            for cell in cells {
                if cell.phantom {
                    continue;
                }
                patch_cell(
                    taffy_tree,
                    cell.taffy,
                    (!cell.anon).then_some(cell.dom),
                    cell.col,
                    cell.col_span,
                    cell.row,
                    cell.row_span,
                );
            }
        }
        for cell in span_cells.iter() {
            patch_cell(
                taffy_tree,
                cell.taffy,
                Some(cell.dom),
                cell.col,
                cell.col_span,
                cell.row,
                cell.row_span,
            );
        }
    }

    (col_gutter, row_gutter)
}

/// Sizing of `build_table` (moved out verbatim): max-content pre-measure,
/// per-column attribution and pinned bases, rowspan row heights, collapsed
/// edge marks, the lifted rowspan cells and shrink-to-fit.
#[allow(clippy::too_many_arguments)]
fn size_table(
    style: &ComputedStyle,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    meta: &mut TableBuildMeta,
    col_authored: &[Option<f32>],
    fixed: bool,
    gap: f32,
    n_rows: usize,
    row_wrappers: &[(usize, diting_taffy::tree::NodeId, Vec<CellSlot>)],
    span_cells: &mut Vec<SpanCell>,
    caption_node: Option<diting_taffy::tree::NodeId>,
    table_node: diting_taffy::tree::NodeId,
    col_gutter: &[f32],
    row_gutter: &[f32],
) {
    // Pre-measure each row wrapper at max-content and harvest per-column
    // maxima. The measured closure is the same TextLeaf dispatch as the
    // root pass — a plain compute_layout would zero the text runs.
    let measure_pass = |taffy_tree: &mut TaffyTree<TextLeaf>,
                        node: diting_taffy::tree::NodeId|
     -> Option<(f32, f32)> {
        let space = diting_taffy::geometry::Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        };
        // Rounding OFF for the pre-measure: word-leaf widths are fractional
        // advances, and taffy's round-to-nearest would drop the fractional
        // residue of a run's sum (61.4 -> 61) — the pinned column then sits
        // BELOW the content width and the final layout wraps a word
        // (batch-58 probe: `AA<span>BB</span>CC` needed 61.4, measured 61,
        // wrapped to two lines). Heights keep the old rounded behavior via
        // .round() at the harvest sites. The layout MUST be read before
        // re-enabling: with rounding off `layout()` reads unrounded_layout
        // (fresh), otherwise it reads final_layout, which the skipped
        // round_layout never wrote — i.e. stale values.
        taffy_tree.disable_rounding();
        let _ = taffy_tree.compute_layout_with_block_measure(
            node,
            space,
            |inputs, id, ctx, style, block| {
                float_lines::measure_node(fonts, inputs, id, ctx, style, block)
            },
        );
        let out = taffy_tree
            .layout(node)
            .ok()
            .map(|l| (l.size.width, l.size.height));
        taffy_tree.enable_rounding();
        out
    };
    // A lifted (rowspan) cell sits in no row wrapper: measure it standalone
    // at the given width availability.
    let measure_cell =
        |taffy_tree: &mut TaffyTree<TextLeaf>,
         node: diting_taffy::tree::NodeId,
         width: AvailableSpace|
         -> (f32, f32) {
            let space = diting_taffy::geometry::Size {
                width,
                height: AvailableSpace::MaxContent,
            };
            taffy_tree.disable_rounding();
            let _ =
                taffy_tree.compute_layout_with_measure(node, space, |inputs, _id, ctx, style| {
                    match ctx {
                        Some(TextLeaf::Run {
                            text,
                            font_size,
                            bold,
                            line_height,
                            baseline_shift,
                            mono,
                            word_spacing,
                            ws,
                            tokens,
                            small_caps,
                            han,
                            line_clamp,
                            ..
                        }) => {
                            let pad =
                                shift_pad(*baseline_shift, leaf_descent(fonts, *font_size, *bold));
                            let shaped = run_tokens(
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
                            measure_text_leaf(&shaped, *line_height, pad, &inputs, *ws, *line_clamp)
                        }
                        Some(TextLeaf::Word { .. })
                        | Some(TextLeaf::Replaced { .. })
                        | Some(TextLeaf::Pseudo { .. })
                        | None => diting_taffy::compute_leaf_layout(
                            inputs,
                            style,
                            |_, _| 0.0,
                            |_, _| Size::ZERO,
                        ),
                    }
                });
            let out = taffy_tree
                .layout(node)
                .map(|l| (l.size.width, l.size.height))
                .unwrap_or((0.0, 0.0));
            taffy_tree.enable_rounding();
            out
        };
    // Column attribution in two passes: single-column cells land wholly in
    // their column; spanning cells then distribute their DEFICIT equally
    // across the spanned columns (the simple spec algorithm — a group
    // already wide enough takes nothing). Document order within each pass.
    let lifted: HashMap<(usize, usize), diting_taffy::tree::NodeId> = span_cells
        .iter()
        .map(|c| ((c.row, c.col), c.taffy))
        .collect();
    let mut col_max: Vec<f32> = Vec::new();
    let mut pass_b: Vec<(usize, usize, f32)> = Vec::new();
    for (row_idx, row_node, cells) in row_wrappers.iter() {
        if measure_pass(taffy_tree, *row_node).is_none() {
            continue;
        }
        for cell in cells {
            let w = if let Some(real) = lifted.get(&(*row_idx, cell.col)) {
                measure_cell(taffy_tree, *real, AvailableSpace::MaxContent)
                    .0
                    .ceil()
            } else {
                taffy_tree.unrounded_layout(cell.taffy).size.width.ceil()
            };
            if cell.col_span > 1 {
                pass_b.push((cell.col, cell.col_span, w));
            } else {
                if cell.col >= col_max.len() {
                    col_max.resize(cell.col + 1, 0.0);
                }
                col_max[cell.col] = col_max[cell.col].max(w);
            }
        }
    }
    for (col, span, w) in pass_b {
        if col + span > col_max.len() {
            col_max.resize(col + span, 0.0);
        }
        let group = col_max[col..col + span].iter().sum::<f32>() + gap * (span - 1) as f32;
        if w > group {
            let share = (w - group) / span as f32;
            for m in &mut col_max[col..col + span] {
                *m += share;
            }
        }
    }

    // Pin every cell to its column(-group) max: uniform bases per column
    // keep the columns aligned under any final width; a spanning cell's
    // basis is the summed group so surplus and deficit move the group as a
    // unit. grow = basis makes surplus table width distribute across
    // columns proportional to their content width (the auto-layout
    // behavior); a zero-content column gets the minimal grow of 1 so it
    // still absorbs its share of an authored table width (Chrome: a single
    // empty column in `width:100px` is 100px wide). Shrink 1 takes the
    // deficit back proportionally. Both preserve alignment because the
    // ratios are uniform per column.
    let group_width = |col: usize, span: usize| -> f32 {
        let end = (col + span).min(col_max.len());
        let mut w = if col < end {
            col_max[col..end].iter().sum::<f32>()
        } else {
            0.0
        };
        if span > 1 {
            w += gap * (span - 1) as f32;
        }
        w
    };
    // Fixed-layout authored group: the summed spanned authored widths plus
    // interior gaps; None when any spanned column is auto (that group then
    // grows for its equal per-column share of the leftover instead).
    let authored_group = |col: usize, span: usize| -> Option<f32> {
        let end = (col + span).min(col_authored.len());
        (col < end && (col..end).all(|i| col_authored[i].is_some())).then(|| {
            col_authored[col..end]
                .iter()
                .map(|w| w.unwrap_or(0.0))
                .sum::<f32>()
                + if span > 1 {
                    gap * (span - 1) as f32
                } else {
                    0.0
                }
        })
    };
    for (_, _, cells) in row_wrappers.iter() {
        for cell in cells {
            // Fixed: an authored group is exact and frozen (no grow, no
            // shrink); an auto group takes zero basis and grows by its
            // spanned-column count — every row then distributes the same
            // leftover over the same per-column grow total, keeping columns
            // aligned (authored-ness is a property of the column, so rows
            // that cover all columns freeze the same total). Auto keeps the
            // content-measured basis with proportional grow/shrink.
            let (basis, grow, shrink) = if fixed {
                match authored_group(cell.col, cell.col_span) {
                    Some(w) => (w, 0.0, 0.0),
                    None => (0.0, cell.col_span as f32, 0.0),
                }
            } else {
                let basis = group_width(cell.col, cell.col_span);
                (basis, if basis > 0.0 { basis } else { 1.0 }, 1.0)
            };
            if let Ok(mut st) = taffy_tree.style(cell.taffy).cloned() {
                st.flex_basis = Dimension::length(basis);
                st.flex_grow = grow;
                st.flex_shrink = shrink;
                if fixed {
                    // Fixed columns don't clamp to their content: a long cell
                    // wraps or overflows its column instead of widening it.
                    st.min_size.width = LengthPercentageAuto::length(0.0);
                } else if cell.phantom {
                    // A rowspan placeholder holds the lifted cell's width: it counts
                    // toward the row's intrinsic width (nested tables collapsed, #305).
                    st.min_size.width = LengthPercentageAuto::length(basis);
                }
                // vertical-align (blitz#508): the declaration or the legacy
                // valign attribute (same computed slot) moves cell CONTENT,
                // not the box — the cell still fills the row via the
                // wrapper's STRETCH. A flex-column cell plus justify_content
                // models it; absent/unknown values take Chrome's UA middle
                // default (every browser's vertical-align:middle on cells).
                // Placeholders only carry the basis — no content to move.
                if !cell.phantom {
                    st.display = Display::Flex;
                    st.flex_direction = FlexDirection::Column;
                    st.justify_content = cell_valign_justify(styles, &cell.dom);
                }
                let _ = taffy_tree.set_style(cell.taffy, st);
            }
        }
    }

    // --- row heights under rowspan (spans batch) -------------------------
    // Natural heights at the pinned bases (the lifted cells are out of the
    // rows, so rows size to their remaining content); each lifted cell's
    // height need then spreads across its spanned rows — a deficit over
    // their natural sum distributes equally, applied as row min-heights so
    // taller content can still grow a row.
    if !span_cells.is_empty() {
        let mut row_h: Vec<f32> = vec![0.0; n_rows];
        let mut raised: Vec<bool> = vec![false; n_rows];
        for (grid_row, row_node, _) in row_wrappers.iter() {
            if let Some((_, h)) = measure_pass(taffy_tree, *row_node) {
                row_h[*grid_row] = h.round();
            }
        }
        for cell in span_cells.iter() {
            // Under fixed the lifted cell's real width is its authored group;
            // the content-measured group is the fallback for auto groups (the
            // final auto width only exists post-layout — the estimate only
            // shapes this row-height pre-pass).
            let group_w = if fixed {
                authored_group(cell.col, cell.col_span)
                    .unwrap_or_else(|| group_width(cell.col, cell.col_span))
            } else {
                group_width(cell.col, cell.col_span)
            };
            let (_, ch) = measure_cell(taffy_tree, cell.taffy, AvailableSpace::Definite(group_w));
            let ch = ch.round();
            let spanned: Vec<usize> = (cell.row..(cell.row + cell.row_span).min(n_rows)).collect();
            if spanned.is_empty() {
                continue;
            }
            let cur: f32 = spanned.iter().map(|r| row_h[*r]).sum();
            if ch > cur {
                let share = (ch - cur) / spanned.len() as f32;
                for r in spanned {
                    row_h[r] += share;
                    raised[r] = true;
                }
            }
        }
        for (grid_row, row_node, _) in row_wrappers.iter() {
            if raised[*grid_row] {
                if let Ok(mut st) = taffy_tree.style(*row_node).cloned() {
                    st.min_size.height = LengthPercentageAuto::length(row_h[*grid_row]);
                    let _ = taffy_tree.set_style(*row_node, st);
                }
            }
        }
    }

    // --- collapsed-border edge marks (spans batch) -----------------------
    // Under collapse each cell side paints at half width inside its own
    // gutter padding: internal lines from the two neighbors meet centered
    // on the grid line (Chrome's resolved-border rendering for equal
    // widths; unequal widths keep the documented proportional-halves
    // approximation), and outer sides stay inside the table box. A side
    // with no gutter paints nothing — there is no reserved space for it.
    // Marks key off the gutter widths, not grid indices.
    if matches!(
        style.border_collapse,
        Some(diting_css::BorderCollapse::Collapse)
    ) {
        for (_, _, cells) in row_wrappers.iter() {
            for cell in cells {
                // Phantoms paint nothing (no node_map entry); the lifted cell
                // itself carries its true grid extent from its origin row.
                // Anonymous cells own no DOM box either — the mark would land
                // on a member element and halve its authored border.
                if cell.phantom || cell.anon {
                    continue;
                }
                let cend = (cell.col + cell.col_span).min(col_gutter.len().saturating_sub(1));
                let rend = (cell.row + cell.row_span).min(n_rows);
                meta.collapsed_edges.insert(
                    cell.dom,
                    [
                        row_gutter.get(cell.row).copied().unwrap_or(0.0) > 0.0,
                        col_gutter.get(cend).copied().unwrap_or(0.0) > 0.0,
                        row_gutter.get(rend).copied().unwrap_or(0.0) > 0.0,
                        col_gutter.get(cell.col).copied().unwrap_or(0.0) > 0.0,
                    ],
                );
            }
        }
    }

    // Lift the rowspan cells onto the table node as absolute children; the
    // post-layout fixup pass resolves their insets from the final row and
    // placeholder boxes. Taffy resolves a `left/top` inset against the
    // parent's border-box origin plus its border width (flexbox.rs
    // `offset_main = start + border`), so carry the effective border — the
    // row wrappers' own locations already include the table's padding.
    if !span_cells.is_empty() {
        for cell in span_cells.iter() {
            if let Ok(mut st) = taffy_tree.style(cell.taffy).cloned() {
                st.position = Position::Absolute;
                st.flex_grow = 0.0;
                st.flex_shrink = 0.0;
                st.display = Display::Flex;
                st.flex_direction = FlexDirection::Column;
                st.justify_content = cell_valign_justify(styles, &cell.dom);
                let _ = taffy_tree.set_style(cell.taffy, st);
            }
            let _ = taffy_tree.add_child(table_node, cell.taffy);
        }
        meta.span_jobs.push(TableSpans {
            rows: row_wrappers.iter().map(|(g, n, _)| (*g, *n)).collect(),
            cells: std::mem::take(span_cells),
            origin: (
                if style.border_style.is_some() {
                    side_px(style.border_width.left)
                } else {
                    0.0
                },
                if style.border_style.is_some() {
                    side_px(style.border_width.top)
                } else {
                    0.0
                },
            ),
        });
    }

    // Shrink-to-fit: without an authored width (an explicit `width: auto`
    // parses to None too), the table sizes to the summed column maxima
    // plus gaps, capped at the containing block width by a 100% max clamp
    // (the same fit-content idiom resolve_sizing_keywords uses).
    if style.width.is_none() && !col_max.is_empty() {
        let gaps = gap * col_max.len().saturating_sub(1) as f32;
        // The caption's longest word is a floor: a table is never narrower.
        let cap_min = caption_node.map_or(0.0, |c| {
            measure_cell(taffy_tree, c, AvailableSpace::MinContent).0
        });
        let shrink = (col_max.iter().sum::<f32>() + gaps).max(cap_min);
        if shrink > 0.0 {
            if let Ok(mut st) = taffy_tree.style(table_node).cloned() {
                st.size.width = Dimension::length(shrink);
                st.max_size.width = LengthPercentageAuto::percent(1.0);
                let _ = taffy_tree.set_style(table_node, st);
            }
        }
    }
}

/// Table layout (display:table family), v1 model:
///
/// The table becomes a taffy flex COLUMN of row wrappers; each row wrapper
/// is a non-wrapping flex ROW of its cells (cells themselves build through
/// the normal element paths, so every inline/block behavior inside a cell
/// keeps working); thead/tbody/tfoot flatten into their tr children.
///
/// Column alignment comes from a pre-measure pass — each row wrapper is laid
/// out at max-content width, per-column maxima are collected, then every
/// cell's flex-basis is pinned to its column max (grow 0 / shrink 1). With
/// a uniform basis per column, taffy's proportional shrink/grow preserves
/// alignment under ANY final table width (authored or shrink-to-fit), so
/// columns line up across rows the way a real table algorithm produces.
///
/// Width: authored table width passes through; otherwise the table
/// shrink-to-fits the summed column maxima with a 100%-of-containing-block
/// max clamp (the same fit-content idiom `resolve_sizing_keywords` uses).
/// `border-collapse: collapse` realizes as zero gaps between rows/cells;
/// the separate initial gets Chrome's default 2px border-spacing.
///
/// v1 known limits: caption renders top-only
/// (caption-side:bottom unstyled); colgroup/col contribute widths only;
/// `table-layout: fixed` requires an authored table width (an auto-width
/// fixed table falls back to the auto algorithm).
#[allow(clippy::too_many_arguments)]
pub(super) fn build_table(
    tree: &DomTree,
    id: NodeId,
    style: &ComputedStyle,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    flattened: &mut HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    meta: &mut TableBuildMeta,
) -> Option<diting_taffy::tree::NodeId> {
    let tag_of = |nid: NodeId| -> String {
        tree.with_node(nid, |n| n.as_element().map(|e| e.local.to_string()))
            .flatten()
            .unwrap_or_default()
    };
    let mut rows: Vec<RowSrc> = Vec::new();
    let mut row_group: Vec<Option<NodeId>> = Vec::new();
    let mut caption_id: Option<NodeId> = None;
    let mut col_nodes: Vec<NodeId> = Vec::new();
    let mut anon_pending: Vec<NodeId> = Vec::new();
    for child in tree.children(id) {
        // Box-less children never open an anonymous row: blank text and
        // display:none (same skips as the per-row cell scan below).
        let generates_box = if tree.with_node(child, |n| n.is_text()).unwrap_or(false) {
            tree.with_node(child, |n| {
                n.text_content_of_text_node().map(|t| !t.trim().is_empty())
            })
            .flatten()
            .unwrap_or(false)
        } else {
            styles.get(&child).and_then(|s| s.display) != Some(CssDisplay::None)
        };
        match tag_of(child).as_str() {
            "tr" => {
                flush_anon(&mut anon_pending, &mut rows, &mut row_group);
                rows.push(RowSrc::Tr(child));
                row_group.push(None);
            }
            "caption" => caption_id = caption_id.or(Some(child)),
            _ if styles.get(&child).and_then(|s| s.display) == Some(CssDisplay::TableCaption) => {
                caption_id = caption_id.or(Some(child))
            }
            "colgroup" | "col" => col_nodes.push(child),
            "thead" | "tbody" | "tfoot" => {
                flush_anon(&mut anon_pending, &mut rows, &mut row_group);
                for gc in tree
                    .children(child)
                    .into_iter()
                    .filter(|gc| tag_of(*gc) == "tr")
                {
                    rows.push(RowSrc::Tr(gc));
                    row_group.push(Some(child));
                }
            }
            _ => {
                if styles.get(&child).and_then(|s| s.display) == Some(CssDisplay::TableRow) {
                    flush_anon(&mut anon_pending, &mut rows, &mut row_group);
                    rows.push(RowSrc::Tr(child));
                    row_group.push(None);
                } else if generates_box {
                    anon_pending.push(child);
                }
            }
        }
    }
    flush_anon(&mut anon_pending, &mut rows, &mut row_group);

    // `collapse` = shared borders → zero gaps; separate (the UA initial)
    // gets Chrome's default 2px border-spacing. Same value on both axes:
    // vertical between row wrappers, horizontal between cells.
    let gap = match style.border_collapse {
        Some(diting_css::BorderCollapse::Collapse) => 0.0,
        _ => 2.0,
    };

    let n_rows = rows.len();
    let mut occupied: Vec<Vec<bool>> = vec![Vec::new(); n_rows];
    let span_attr = |cid: NodeId, name: &str| -> usize {
        tree.with_node(cid, |n| n.get_attribute(name).map(|v| v.to_string()))
            .flatten()
            .and_then(|v: String| v.trim().parse::<usize>().ok())
            .filter(|v| *v >= 1)
            .unwrap_or(1)
    };
    // `table-layout: fixed` needs an authored table width (an auto-width
    // fixed table falls back to the auto algorithm — the browser guidance
    // posture for "fixed needs width"). Under fixed, column widths come
    // from authored sources only: colgroup/col `width` attributes and
    // first-row cells; later-row content never widens a column. The
    // content-measurement passes still run (row heights under rowspan read
    // them), but the pinning below ignores their column maxima.
    let fixed =
        matches!(style.table_layout, Some(diting_css::TableLayout::Fixed)) && style.width.is_some();
    // Per-column authored widths for the fixed layout. None = auto column
    // (its share of the leftover width comes from flex-grow at layout).
    let mut col_authored: Vec<Option<f32>> = Vec::new();
    if fixed && !col_nodes.is_empty() {
        let px_attr = |nid: NodeId| -> Option<f32> {
            tree.with_node(nid, |n| {
                n.get_attribute("width").map(|v| v.trim().to_string())
            })
            .flatten()
            .and_then(|v| {
                // HTML bare number; tolerate a px suffix (pages write
                // width="100px" against the spec). Percentages stay
                // unparsed — no resolved table width to fold against.
                let t = v.trim();
                let t = t.strip_suffix("px").map(str::trim).unwrap_or(t);
                t.parse::<f32>().ok().filter(|w| *w > 0.0)
            })
        };
        for cn in &col_nodes {
            // A colgroup's own width covers its span when it carries no col
            // children; each col child carries its own (span spreads one
            // width over a run of columns).
            let cols: Vec<NodeId> = tree
                .children(*cn)
                .into_iter()
                .filter(|gc| tag_of(*gc) == "col")
                .collect();
            let sources: Vec<(NodeId, usize)> = if cols.is_empty() {
                vec![(*cn, span_attr(*cn, "span"))]
            } else {
                cols.into_iter()
                    .map(|c| (c, span_attr(c, "span")))
                    .collect()
            };
            for (c, span) in sources {
                let authored = px_attr(c);
                let start = col_authored.len();
                col_authored.resize(start + span, None);
                if let Some(w) = authored {
                    for slot in &mut col_authored[start..start + span] {
                        *slot = Some(w);
                    }
                }
            }
        }
    }
    let mut span_cells: Vec<SpanCell> = Vec::new();
    // Placeholders owed to rows below by rowspan cells processed above: each
    // continuation row needs its own slot-holder leaf (a taffy node has one
    // parent), else its own cells collapse to x=0.
    let mut pend_ph: Vec<Vec<(diting_taffy::tree::NodeId, NodeId, usize, usize)>> =
        vec![Vec::new(); n_rows];
    // (grid row, wrapper node, cells) — wrappers only exist for rows that
    // produced at least one cell box.
    let mut row_wrappers: Vec<(usize, diting_taffy::tree::NodeId, Vec<CellSlot>)> = Vec::new();
    build_table_rows(
        tree,
        id,
        styles,
        images,
        fonts,
        taffy_tree,
        node_map,
        flattened,
        run_wrappers,
        meta,
        &rows,
        &mut col_authored,
        fixed,
        gap,
        n_rows,
        &span_attr,
        &mut occupied,
        &mut pend_ph,
        &mut row_wrappers,
        &mut span_cells,
    );
    // The caption box rides first: a block-flow first child of the table,
    // spanning its width (caption-side top is the UA default; bottom is out
    // of scope for v1). Previously the element was dropped entirely — no box
    // built — so wikipedia infobox captions were invisible.
    let mut table_children: Vec<diting_taffy::tree::NodeId> =
        Vec::with_capacity(row_wrappers.len() + 1);
    let mut caption_node: Option<diting_taffy::tree::NodeId> = None;
    if let Some(cap) = caption_id {
        if let Some(cap_node) = build_element(
            tree,
            cap,
            styles,
            images,
            fonts,
            taffy_tree,
            node_map,
            flattened,
            run_wrappers,
            meta,
        ) {
            // A caption sizes the table by its longest word, not its line.
            taffy_tree.mark_min_as_max(cap_node);
            caption_node = Some(cap_node);
            table_children.push(cap_node);
        }
    }
    // Re-group consecutive rows that share a thead/tbody/tfoot ancestor
    // into a group wrapper (flex column, same border-spacing gap, keyed to
    // the group's DOM id). Geometry is unchanged: the table's own gap still
    // spaces the top-level children, and each group's internal gap spaces
    // its rows, so the total inter-row gaps are exactly the pre-grouping
    // N-1. What the wrapper BUYS: the group lands in node_map, so the
    // harvest gives it a rect — gBCR, backgrounds, and `position: sticky`
    // on row groups (Chrome sticks the whole group) all inherit the box.
    {
        let mut gi = 0usize;
        while gi < row_wrappers.len() {
            let (row_idx, row_node, _) = &row_wrappers[gi];
            match row_group.get(*row_idx).copied().flatten() {
                None => {
                    table_children.push(*row_node);
                    gi += 1;
                }
                Some(group_dom) => {
                    let mut members = vec![*row_node];
                    gi += 1;
                    while gi < row_wrappers.len()
                        && row_group.get(row_wrappers[gi].0).copied().flatten() == Some(group_dom)
                    {
                        members.push(row_wrappers[gi].1);
                        gi += 1;
                    }
                    let group_style = Style {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::STRETCH,
                        flex_wrap: FlexWrap::NoWrap,
                        gap: diting_taffy::geometry::Size {
                            width: LengthPercentage::length(0.0),
                            height: LengthPercentage::length(gap),
                        },
                        ..Default::default()
                    };
                    if let Ok(group_node) = taffy_tree.new_with_children(group_style, &members) {
                        node_map.insert(group_node, group_dom);
                        table_children.push(group_node);
                    } else {
                        table_children.extend(members);
                    }
                }
            }
        }
    }
    let table_node = if table_children.is_empty() {
        taffy_tree
            .new_leaf(to_taffy_style(style, pct_height_resolves(tree, styles, id)))
            .ok()?
    } else {
        taffy_tree
            .new_with_children(
                to_taffy_style(style, pct_height_resolves(tree, styles, id)),
                &table_children,
            )
            .ok()?
    };
    node_map.insert(table_node, id);

    if row_wrappers.is_empty() {
        return Some(table_node);
    }

    let (col_gutter, row_gutter) = collapsed_gutters(
        style,
        styles,
        taffy_tree,
        n_rows,
        &row_wrappers,
        &span_cells,
    );
    size_table(
        style,
        styles,
        fonts,
        taffy_tree,
        meta,
        &col_authored,
        fixed,
        gap,
        n_rows,
        &row_wrappers,
        &mut span_cells,
        caption_node,
        table_node,
        &col_gutter,
        &row_gutter,
    );
    Some(table_node)
}

/// Table span placement of `layout_solve_rooted` (moved out verbatim):
/// lifted rowspan/colspan cells take their grid-derived boxes.
pub(super) fn place_table_spans(
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    icb_node: diting_taffy::tree::NodeId,
    available: Size<AvailableSpace>,
    table_meta: &TableBuildMeta,
) -> Result<(), SolveAborted> {
    // --- table span placement (spans batch) --------------------------------
    // The lifted rowspan cells sit on their table node as absolute children
    // with auto insets; now that the final layout exists, read each
    // placeholder's slot geometry and the spanned row wrappers' bands, and
    // pin the real cell over them (width = slot, height = first-row top to
    // last-row bottom). Registry order is inner-table-first (children build
    // before their parent table finishes), so walk it in REVERSE: an outer
    // table's lift re-runs layout before an inner table inside the lifted
    // cell reads its own rows, keeping nested spanned tables honest.
    if !table_meta.span_jobs.is_empty() {
        for job in table_meta.span_jobs.iter().rev() {
            let mut dirty = false;
            for cell in &job.cells {
                let Ok(ph) = taffy_tree.layout(cell.placeholder) else {
                    continue;
                };
                let Some(row_layout) = job
                    .rows
                    .iter()
                    .find(|(g, _)| *g == cell.row)
                    .and_then(|(_, n)| taffy_tree.layout(*n).ok())
                else {
                    continue;
                };
                // Band bottom: the last row wrapper inside the span (clamped
                // when the rowspan ran past the table's last row at build).
                let last = job
                    .rows
                    .iter()
                    .rfind(|(g, _)| *g < cell.row + cell.row_span)
                    .or_else(|| job.rows.last());
                let Some(last_layout) = last.and_then(|(_, n)| taffy_tree.layout(*n).ok()) else {
                    continue;
                };
                // Slot x and band y are table-border-box coordinates: the row
                // wrapper's location already folds the table's border+padding
                // in, the placeholder's location the row wrapper's.
                let x = row_layout.location.x + ph.location.x;
                let y_top = row_layout.location.y;
                let y_bot = last_layout.location.y + last_layout.size.height;
                if let Ok(mut st) = taffy_tree.style(cell.taffy).cloned() {
                    st.position = Position::Absolute;
                    st.inset = diting_taffy::geometry::Rect {
                        left: LengthPercentageAuto::length((x - job.origin.0).max(0.0)),
                        top: LengthPercentageAuto::length((y_top - job.origin.1).max(0.0)),
                        right: LengthPercentageAuto::AUTO,
                        bottom: LengthPercentageAuto::AUTO,
                    };
                    st.size = diting_taffy::geometry::Size {
                        width: Dimension::length(ph.size.width.max(0.0)),
                        height: Dimension::length((y_bot - y_top).max(0.0)),
                    };
                    let _ = taffy_tree.set_style(cell.taffy, st);
                    dirty = true;
                }
            }
            if dirty {
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
    Ok(())
}
