//! The anonymous block that holds the in-flow siblings of a float zone.
//!
//! Split from the layout hub. `float_zone` still calls these. Pure move.

use super::*;

/// The anonymous flow column of a float zone (8b/8i): every in-zone sibling
/// built normally inside one block wrapper. Not in node_map — it has no DOM
/// identity, so collect skips it and paints walk straight through to the
/// real children.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_flow_column(
    flow_dom: &[NodeId],
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    flattened: &mut HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    meta: &mut TableBuildMeta,
    font_size: f32,
    lh_elem: f32,
    container_ws: WhiteSpace,
) -> Vec<diting_taffy::tree::NodeId> {
    let mut flow_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
    let mut run: Vec<RunSeg> = Vec::new();
    let mut run_clip: Option<NodeId> = None;
    let run_frame = text_src::new_frame();
    let flush_run = |run: &mut Vec<RunSeg>,
                     flow_children: &mut Vec<diting_taffy::tree::NodeId>,
                     taffy_tree: &mut TaffyTree<TextLeaf>,
                     run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
                     clip: Option<NodeId>| {
        if run.is_empty() {
            return;
        }
        let segs = std::mem::take(run);
        let pending = text_src::drain_pending(run_frame);
        let mut pending_it = pending.iter().copied();
        let seg_lens: Vec<u32> = segs
            .iter()
            .filter_map(|s| match s {
                RunSeg::Text(t, ..) => Some(t.chars().count() as u32),
                _ => None,
            })
            .collect();
        if segs.iter().all(|s| matches!(s, RunSeg::Text(..))) {
            let text = segs
                .iter()
                .map(|s| match s {
                    RunSeg::Text(t, ..) => t.as_str(),
                    _ => "",
                })
                .collect::<String>();
            if !text.trim().is_empty()
                || {
                    let any_preserve = segs.iter().any(|s| matches!(s, RunSeg::Text(.., w, _, _) if w.preserves_spaces() || w.preserves_newlines()));
                    any_preserve
                }
            {
                let RunSeg::Text(_, fs, bold, color, lh, deco, vs, mono, wsp, _, sc, han) =
                    &segs[0]
                else {
                    unreachable!()
                };
                // white-space inherits, so the preserve-wins winner across
                // segments rides the whole run (the "any nowrap pins the
                // run" precedent, ranked across the family).
                let run_ws = segs.iter().fold(WhiteSpace::Normal, |acc, s| match s {
                    RunSeg::Text(.., w, _, _) if ws_rank(*w) > ws_rank(acc) => *w,
                    _ => acc,
                });
                // Anonymous flow columns own no element style; text-overflow
                // and line-clamp are non-inherited, so their owner is
                // unknown here — v1-off. text-align IS inherited but the
                // zone path aligns through the host's promote, so the run
                // stays Left here too.
                let run_ellipsis = false;
                let run_clamp = None;
                if let Ok(leaf) = taffy_tree.new_leaf_with_context(
                    float_flow::run_style(),
                    TextLeaf::Run {
                        text,
                        font_size: *fs,
                        bold: *bold,
                        color: *color,
                        line_height: *lh,
                        decorations: *deco,
                        baseline_shift: *vs,
                        mono: *mono,
                        word_spacing: *wsp,
                        ws: run_ws,
                        atomic: false,
                        align: TextAlign::Left,
                        ellipsis: run_ellipsis,
                        line_clamp: run_clamp,
                        tokens: std::cell::RefCell::new(None),
                        small_caps: *sc,
                        han: *han,
                        clip_src: clip,
                    },
                ) {
                    text_src::register(leaf, text_src::seg_ranges(&pending, &seg_lens));
                    flow_children.push(leaf);
                }
            }
            return;
        }
        // #158: the wrapper stops wrapping when no segment (nor the
        // container) leaves a soft wrap opportunity — the same fold the
        // pure-text branch runs, plus the column's container value for
        // runs of pure atomic inlines that carry no ws of their own.
        let ws = segs.iter().fold(container_ws, |acc, s| match s {
            RunSeg::Text(.., w, _, _) if ws_rank(*w) > ws_rank(acc) => *w,
            _ => acc,
        });
        let mut leaves: Vec<diting_taffy::tree::NodeId> = Vec::new();
        for seg in segs {
            match seg {
                RunSeg::Text(text, fs, bold, color, lh, deco, vs, mono, wsp, _, sc, han) => {
                    text_src::set_cur(pending_it.next());
                    leaves.extend(build_word_leaves(
                        &text, fs, bold, color, lh, deco, vs, mono, wsp, sc, han, fonts, taffy_tree,
                    ));
                    text_src::set_cur(None);
                }
                RunSeg::Nodes(nodes) => leaves.extend(nodes),
            }
        }
        trim_run_edge_whitespace(taffy_tree, &mut leaves);
        if !leaves.is_empty() {
            if ws.no_soft_wrap() {
                pin_leaf_widths(taffy_tree, &leaves);
            }
            if let Ok(wrapper) = taffy_tree.new_with_children(run_wrapper_style(ws), &leaves) {
                run_wrappers.push(wrapper);
                flow_children.push(wrapper);
            }
        }
    };
    for child in flow_dom.iter().copied() {
        let is_text = tree.with_node(child, |n| n.is_text()).unwrap_or(false);
        let child_tag = tree
            .with_node(child, |n| n.as_element().map(|e| e.local.to_string()))
            .flatten()
            .unwrap_or_default();
        let child_display = styles.get(&child).and_then(|s| s.display).or(if is_text {
            Some(CssDisplay::Inline)
        } else {
            Some(CssDisplay::Block)
        });
        let inline_level = matches!(child_display, Some(CssDisplay::Inline));
        if !is_text && child_display == Some(CssDisplay::None) {
            continue;
        } // hidden: no box, must not cut the run
        let out_of_flow = styles.get(&child).is_some_and(|s| {
            matches!(
                s.position,
                Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
            )
        });
        if child_tag == "br" && !out_of_flow {
            // #146: same trailing-br accounting as the main walk (see
            // build_element_inner) — flush, then a strut only for an EMPTY
            // ended line.
            let line_has_content = run.iter().any(|seg| match seg {
                RunSeg::Text(t, ..) => !t.trim().is_empty(),
                RunSeg::Nodes(n) => !n.is_empty(),
            });
            flush_run(
                &mut run,
                &mut flow_children,
                taffy_tree,
                run_wrappers,
                run_clip.take(),
            );
            if !line_has_content && child_display != Some(CssDisplay::None) {
                let (_, _, lh) = font_context(tree, child, styles, fonts);
                if let Some(strut) = br_strut_leaf(taffy_tree, lh) {
                    flow_children.push(strut);
                }
            }
            continue;
        }
        if !is_text && is_replaced_tag(&child_tag) {
            // Inline-flavored replaced elements (UA inline-block form
            // controls included) join the text run; the rest stay block
            // siblings (current shipped behavior for img/video).
            let inline_atom = inline_level || child_display == Some(CssDisplay::InlineBlock);
            let sd = if inline_atom && !out_of_flow {
                strut_descent_for(tree, child, styles, fonts)
            } else {
                0.0
            };
            let leaf = build_replaced_leaf(tree, child, styles, images, taffy_tree, node_map, sd);
            if let Some(leaf) = leaf {
                if inline_atom && !out_of_flow {
                    run.push(RunSeg::Nodes(vec![leaf]));
                } else {
                    flush_run(
                        &mut run,
                        &mut flow_children,
                        taffy_tree,
                        run_wrappers,
                        run_clip.take(),
                    );
                    flow_children.push(leaf);
                }
            }
            continue;
        }
        if is_text {
            let text = tree
                .with_node(child, |n| {
                    n.text_content_of_text_node().unwrap_or("").to_string()
                })
                .unwrap_or_default();
            // text-transform folds into the string before the run sees it
            // (批240) — per segment is exact for upper/lower (stateless) and
            // per-node for capitalize (cross-element words are a documented
            // v1 divergence).
            let text = {
                let tt = text_transform_context(tree, child, styles);
                if tt == TextTransform::None {
                    text
                } else {
                    text::apply_text_transform(&text, tt)
                }
            };
            let (fs, b, lh) = font_context(tree, child, styles, fonts);
            let fs = if styles.get(&child).is_some() {
                fs
            } else {
                font_size
            };
            let lh = if styles.get(&child).is_some() {
                lh
            } else {
                lh_elem
            };
            let col = color_context(tree, child, styles);
            let deco = decoration_context(tree, child, styles);
            let vs = valign_shift(tree, child, styles, fonts);
            let mono = mono_context(tree, child, styles);
            let ws = word_spacing_context(tree, child, styles);
            let nws = tree
                .with_node(child, |n| n.parent)
                .flatten()
                .and_then(|p| styles.get(&p))
                .and_then(|s| s.white_space)
                .unwrap_or(WhiteSpace::Normal);
            let sc = small_caps_context(tree, child, styles);
            let han = han_context(tree, child, styles);
            run_clip = Some(child);
            text_src::note_seg(run_frame, child);
            run.push(RunSeg::Text(
                text, fs, b, col, lh, deco, vs, mono, ws, nws, sc, han,
            ));
        } else if child_display == Some(CssDisplay::InlineBlock) && !out_of_flow {
            // Atomic inline-level box (obscura#750 family): keeps its own
            // subtree box, joins the run as one shrink-to-fit unit.
            if let Some(sub) = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                run.push(RunSeg::Nodes(vec![sub]));
            }
        } else if inline_level && !out_of_flow && !blockifies_for_block_child(tree, child, styles) {
            let sub = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            );
            if let Some(sub) = sub {
                let mut sub_children: Vec<_> =
                    taffy_tree.children(sub).unwrap_or_default().to_vec();
                wrap_q_quotes(
                    tree,
                    child,
                    &child_tag,
                    styles,
                    fonts,
                    taffy_tree,
                    &mut sub_children,
                );
                run.push(RunSeg::Nodes(sub_children.clone()));
                node_map.remove(&sub);
                let _ = taffy_tree.remove(sub);
                // Boxless after hoisting — union pass rebuilds the rect.
                if !sub_children.is_empty() {
                    flattened.insert(child, sub_children);
                }
            }
        } else {
            flush_run(
                &mut run,
                &mut flow_children,
                taffy_tree,
                run_wrappers,
                run_clip.take(),
            );
            if let Some(node) = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                flow_children.push(node);
            }
        }
    }
    flush_run(
        &mut run,
        &mut flow_children,
        taffy_tree,
        run_wrappers,
        run_clip.take(),
    );
    flow_children
}

/// Whether `id`'s subtree can never paint anything: no non-whitespace text
/// outside metadata elements. MediaWiki parks
/// `<span class="mw-empty-elt"><link/><link/></span>` wrappers between its
/// floated infobox and the sidebar tables — the links are ResourceLoader
/// hints, not content — so a bare has-children check misreads the wrappers
/// as flow content and stops the same-side float rail from forming.
pub(super) fn subtree_paints_nothing(tree: &DomTree, id: NodeId) -> bool {
    enum Step {
        Paints,
        Skip,
        Descend,
    }
    let mut stack = vec![id];
    while let Some(cur) = stack.pop() {
        let Some(step) = tree.with_node(cur, |n| match &n.data {
            diting_dom::NodeData::Text { contents } => {
                if contents.trim().is_empty() {
                    Step::Skip
                } else {
                    Step::Paints
                }
            }
            diting_dom::NodeData::Element { name, .. } => {
                if matches!(
                    name.local.as_str(),
                    "link" | "meta" | "style" | "script" | "template"
                ) {
                    Step::Skip
                } else {
                    Step::Descend
                }
            }
            _ => Step::Skip,
        }) else {
            return false;
        };
        match step {
            Step::Paints => return false,
            Step::Skip => {}
            Step::Descend => {
                for child in render_children(tree, cur) {
                    stack.push(child);
                }
            }
        }
    }
    true
}
