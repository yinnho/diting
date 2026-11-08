//! Generated-content pseudo boxes (::before/::after) — the box-builder
//! half of the generated-content family, split from mod.rs at the
//! god-file ratchet's demand (#19 era). The cascade half (pseudo_styles,
//! content resolution into `content_resolved`) stays in mod.rs where the
//! counter walk lives; this file turns the resolved pseudo styles into
//! taffy leaves spliced into the host's child list.

use std::collections::HashMap;

use diting_taffy::{Display, FlexDirection, Style};

use diting_dom::tree::{DomTree, NodeId};

use super::flow_tree::FlowTree as TaffyTree;
use super::{
    effective_line_height, float_flow, is_replaced_tag, own_decorations, run_wrapper_style,
    to_taffy_style, ComputedStyle, CssDisplay, FontBook, TextAlign, TextLeaf, WhiteSpace,
};

/// The ComputedStyle of a [`TextLeaf::Pseudo`] leaf — resolved through its
/// HOST's styles entry, the `before` flag picking which side of the pair.
/// Any other node returns None.
pub(super) fn pseudo_leaf_style<'a>(
    taffy_tree: &TaffyTree<TextLeaf>,
    styles: &'a HashMap<NodeId, ComputedStyle>,
    node: diting_taffy::tree::NodeId,
) -> Option<&'a ComputedStyle> {
    match taffy_tree.get_node_context(node) {
        Some(TextLeaf::Pseudo { host, before }) => {
            styles.get(host)?.pseudos.as_deref().and_then(|pair| {
                if *before {
                    pair.before.as_ref()
                } else {
                    pair.after.as_ref()
                }
            })
        }
        _ => None,
    }
}

/// ::before/::after boxes for a host whose taffy node was just built
/// (generated-content v1). The pseudo computed styles ride the host's entry
/// in `styles` (cascade side: pseudo_styles); here they become leaves spliced
/// into the host's child list. Replaced and table hosts are skipped in v1:
/// replaced boxes own no layout children, and table hosts reify their own
/// row structure through build_table.
pub(super) fn append_pseudo_leaves(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    node: diting_taffy::tree::NodeId,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
) {
    let Some(style) = styles.get(&id) else { return };
    let Some(pair) = style.pseudos.as_deref() else {
        return;
    };
    let tag = tree
        .with_node(id, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    if is_replaced_tag(&tag) || style.display == Some(CssDisplay::Table) {
        return;
    }
    let mut children: Vec<diting_taffy::tree::NodeId> =
        taffy_tree.children(node).unwrap_or_default().to_vec();
    let mut changed = false;
    if let Some(before) = &pair.before {
        changed |= pseudo_leaf(id, before, taffy_tree, &mut children, fonts, true);
    }
    if let Some(after) = &pair.after {
        changed |= pseudo_leaf(id, after, taffy_tree, &mut children, fonts, false);
    }
    if changed {
        let _ = taffy_tree.set_children(node, &children);
    }
}

/// Whether an inline pseudo may merge into an adjacent pure-text run: the
/// merged leaf carries the RUN's paint style, so the pseudo's computed text
/// style must agree on every axis the merge would override (#172). A None
/// axis is inherited from the host and equals the run's value by
/// construction, so only explicitly diverging declarations unmerge.
fn pseudo_text_style_matches(
    p: &ComputedStyle,
    run_color: [u8; 4],
    run_font_size: f32,
    run_bold: bool,
    run_mono: bool,
) -> bool {
    p.color.is_none_or(|c| [c.0, c.1, c.2, c.3] == run_color)
        && p.font_size.is_none_or(|f| (f - run_font_size).abs() < 0.01)
        && p.font_weight.is_none_or(|w| (w >= 600) == run_bold)
        && p.font_family
            .as_deref()
            .is_none_or(|f| diting_css::wants_monospace(f) == run_mono)
}

/// Build one inline pseudo text leaf carrying the pseudo's OWN computed
/// text style — #172's split path (the adjacent-run merge only fits a
/// pseudo whose style matches the run). Same construction the standalone
/// inline path has always used.
fn pseudo_inline_run_leaf(
    taffy_tree: &mut TaffyTree<TextLeaf>,
    p: &ComputedStyle,
    fonts: &FontBook,
    content: &str,
    ws: WhiteSpace,
) -> Option<diting_taffy::tree::NodeId> {
    let fs = p.font_size.unwrap_or(diting_css::DEFAULT_ROOT_FONT_SIZE);
    let bold = p.font_weight.is_some_and(|w| w >= 600);
    taffy_tree
        .new_leaf_with_context(
            Style::default(),
            TextLeaf::Run {
                text: content.to_string(),
                font_size: fs,
                bold,
                color: p.color.map_or([0, 0, 0, 255], |c| [c.0, c.1, c.2, c.3]),
                line_height: effective_line_height(fonts, p.line_height.as_ref(), fs, bold),
                decorations: own_decorations(p),
                baseline_shift: 0.0,
                mono: p
                    .font_family
                    .as_deref()
                    .is_some_and(diting_css::wants_monospace),
                word_spacing: p.word_spacing.unwrap_or(0.0),
                ws,
                atomic: false,
                align: p.physical_text_align().unwrap_or(TextAlign::Left),
                ellipsis: false,
                line_clamp: None,
                clip_src: None,
                tokens: std::cell::RefCell::new(None),
                small_caps: p.font_variant_caps.unwrap_or(false),
                han: p.lang.as_deref().and_then(crate::text::han_slot_for_lang),
            },
        )
        .ok()
}

/// Build one pseudo box and splice it into the host's child list; returns
/// whether the list changed. Block-ish pseudos become leaves carrying the
/// pseudo's taffy style — an EMPTY content box stays a plain style leaf so
/// padding/border/background/height survive (the clearfix shape), tagged
/// `TextLeaf::Pseudo` so paint claims its background (#172). Inline
/// pseudos join the host's text: when the adjacent leaf is a pure-text run
/// their texts merge onto one leaf, so `li:before { content: "• " }` shares
/// the line with the label (Chrome's inline generated boxes take part in the
/// host's first/last line box). The merge only happens while the pseudo's
/// own text style matches the run's (#172) — a diverging pseudo becomes its
/// own leaf and paints in its declared style.
fn pseudo_leaf(
    host: NodeId,
    p: &ComputedStyle,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    children: &mut Vec<diting_taffy::tree::NodeId>,
    fonts: &FontBook,
    is_before: bool,
) -> bool {
    if p.display == Some(CssDisplay::None) {
        return false;
    }
    // The flattened walk-resolved text (#19); `content` itself stays
    // symbolic for the gCS face.
    let Some(content) = &p.content_resolved else {
        return false;
    };
    // CSS display-change: absolute/fixed positioning blockifies the pseudo
    // — the overlay idiom `::before { content:""; position:absolute;
    // inset:0 }` declares no display at all, and Chrome generates a block
    // box for it. Without this the None display falls into the inline
    // branch, whose empty-content gate drops the box entirely (#172).
    let effective_display = match p.position {
        Some(diting_css::PositionMode::Absolute) | Some(diting_css::PositionMode::Fixed)
            if matches!(p.display, None | Some(CssDisplay::Inline)) =>
        {
            Some(CssDisplay::Block)
        }
        _ => p.display,
    };
    let insert = |leaf: diting_taffy::tree::NodeId, children: &mut Vec<diting_taffy::tree::NodeId>| {
        if is_before {
            children.insert(0, leaf);
        } else {
            children.push(leaf);
        }
    };
    match effective_display {
        None | Some(CssDisplay::Inline) => {
            if content.trim().is_empty() {
                return false;
            }
            // Merge into the adjacent pure-text run when there is one
            // (first child for ::before, last for ::after) — but only when
            // the pseudo's own text style agrees with the run's (#172): a
            // merged leaf paints with the RUN's metrics and color, so
            // `.row::before { content: "▸ "; color: accent }` used to lose
            // its accent and paint in the host's color. A diverging pseudo
            // becomes its own leaf spliced INTO the inline flow, so it
            // still shares the host's line like Chrome's generated box.
            let adj_idx = if is_before {
                0
            } else {
                children.len().saturating_sub(1)
            };
            if let Some(&adj) = children.get(adj_idx) {
                if let Some(TextLeaf::Run {
                    text,
                    font_size,
                    bold,
                    color,
                    line_height,
                    decorations,
                    baseline_shift,
                    mono,
                    word_spacing,
                    ws,
                    align,
                    ellipsis,
                    line_clamp,
                    small_caps,
                    han,
                    clip_src,
                    ..
                }) = taffy_tree.get_node_context(adj)
                {
                    let ws_run = *ws;
                    if pseudo_text_style_matches(p, *color, *font_size, *bold, *mono) {
                        let merged = if is_before {
                            format!("{}{}", content, text)
                        } else {
                            format!("{}{}", text, content)
                        };
                        let leaf = taffy_tree
                            .new_leaf_with_context(
                                Style::default(),
                                TextLeaf::Run {
                                    text: merged,
                                    font_size: *font_size,
                                    bold: *bold,
                                    color: *color,
                                    line_height: *line_height,
                                    decorations: *decorations,
                                    baseline_shift: *baseline_shift,
                                    mono: *mono,
                                    word_spacing: *word_spacing,
                                    ws: *ws,
                                    atomic: false,
                                    align: *align,
                                    ellipsis: *ellipsis,
                                    line_clamp: *line_clamp,
                                    tokens: std::cell::RefCell::new(None),
                                    small_caps: *small_caps,
                                    han: *han,
                                    clip_src: *clip_src,
                                },
                            )
                            .ok();
                        if let Some(leaf) = leaf {
                            let _ = taffy_tree.remove(adj);
                            children[adj_idx] = leaf;
                            return true;
                        }
                        return false;
                    }
                    // Style diverges — build the pseudo's own leaf and keep
                    // it on the same line: when the neighbor is a mixed
                    // inline run-wrapper splice into its boundary, else
                    // (bare run) regroup [pseudo, run] into a wrapping flex
                    // row at the run's slot. A bare sibling leaf would stack
                    // as its own block line under a block host.
                    if let Some(leaf) =
                        pseudo_inline_run_leaf(taffy_tree, p, fonts, content, ws_run)
                    {
                        let wrapper_shape = taffy_tree.get_node_context(adj).is_none()
                            && taffy_tree.style(adj).is_ok_and(|s| {
                                s.display == Display::Flex && s.flex_direction == FlexDirection::Row
                            })
                            && taffy_tree.children(adj).is_ok_and(|kids| !kids.is_empty());
                        if wrapper_shape {
                            let mut kids = taffy_tree.children(adj).unwrap_or_default().to_vec();
                            if is_before {
                                kids.insert(0, leaf);
                            } else {
                                kids.push(leaf);
                            }
                            let _ = taffy_tree.set_children(adj, &kids);
                            return true;
                        }
                        let pair = if is_before {
                            vec![leaf, adj]
                        } else {
                            vec![adj, leaf]
                        };
                        if let Ok(wrapper) =
                            taffy_tree.new_with_children(run_wrapper_style(ws_run), &pair)
                        {
                            children[adj_idx] = wrapper;
                            return true;
                        }
                    }
                    return false;
                }
            }
            match pseudo_inline_run_leaf(
                taffy_tree,
                p,
                fonts,
                content,
                p.white_space.unwrap_or(WhiteSpace::Normal),
            ) {
                Some(leaf) => {
                    insert(leaf, children);
                    true
                }
                None => false,
            }
        }
        _ => {
            // Pseudo-element content leaf: no DOM id to walk a containing-
            // block chain from, and content strings aren't height-sized —
            // keep percent passthrough (the pre-fold behavior).
            let mut taffy_style = to_taffy_style(p, true);
            if float_flow::enabled() && p.clear_side.is_some() && content.is_empty() {
                // A clearfix box sits in its host's block flow and clears.
                taffy_style.display = Display::Block;
                taffy_style.clear = float_flow::clear_of(p.clear_side);
            }
            let leaf = if content.trim().is_empty() {
                // The empty decoration box (#172): a tagged leaf, so the
                // paint walk can find the pseudo's own ComputedStyle
                // through the host and paint its background layers.
                taffy_tree
                    .new_leaf_with_context(
                        taffy_style,
                        TextLeaf::Pseudo {
                            host,
                            before: is_before,
                        },
                    )
                    .ok()
            } else {
                taffy_tree
                    .new_leaf_with_context(taffy_style, {
                        let fs = p.font_size.unwrap_or(diting_css::DEFAULT_ROOT_FONT_SIZE);
                        let bold = p.font_weight.is_some_and(|w| w >= 600);
                        TextLeaf::Run {
                            text: content.clone(),
                            font_size: fs,
                            bold,
                            color: p.color.map_or([0, 0, 0, 255], |c| [c.0, c.1, c.2, c.3]),
                            line_height: effective_line_height(
                                fonts,
                                p.line_height.as_ref(),
                                fs,
                                bold,
                            ),
                            decorations: own_decorations(p),
                            baseline_shift: 0.0,
                            mono: p
                                .font_family
                                .as_deref()
                                .is_some_and(diting_css::wants_monospace),
                            word_spacing: p.word_spacing.unwrap_or(0.0),
                            ws: p.white_space.unwrap_or(WhiteSpace::Normal),
                            atomic: false,
                            align: p.physical_text_align().unwrap_or(TextAlign::Left),
                            ellipsis: false,
                            line_clamp: None,
                            tokens: std::cell::RefCell::new(None),
                            small_caps: p.font_variant_caps.unwrap_or(false),
                            han: p.lang.as_deref().and_then(crate::text::han_slot_for_lang),
                            clip_src: None,
                        }
                    })
                    .ok()
            };
            match leaf {
                Some(leaf) => {
                    insert(leaf, children);
                    true
                }
                None => false,
            }
        }
    }
}
