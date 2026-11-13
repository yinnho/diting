//! Element box build: `build_element` and the flow-child walk under it.
//!
//! Split from the layout hub. Tables, float zones, the flow column and
//! `layout_solve_rooted` still call `build_element`. Pure move: this walk
//! sees the same crate namespace it grew up in.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_element(
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
) -> Option<diting_taffy::tree::NodeId> {
    // The float environment is a parent-to-child fact: inner reads what
    // the parent decided for THIS element and re-sets it for its own
    // children — restored here so a later sibling never inherits it.
    let env_saved = float_flow::in_env();
    let node = build_element_inner(
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
    );
    float_flow::set_env(env_saved);
    let node = node?;
    append_pseudo_leaves(tree, id, styles, node, fonts, taffy_tree);
    Some(node)
}

// Partition children into block-level elements (direct taffy children)
// and inline runs (text + inline elements). A PURE-text run (only text
// nodes) becomes ONE measured leaf — the same shape blitz gives its
// parley text nodes, whose observable behavior (edge-whitespace collapse,
// greedy wrap, ceiled width) we reproduce in measure_text_leaf. Mixed
// runs fall back to the batch-2b wrapping flex row of word leaves.
pub(super) enum RunSeg {
    Text(
        String,
        f32,
        bool,
        [u8; 4],
        f32,
        TextDecorations,
        f32,
        bool,
        f32,
        WhiteSpace,
        bool,
        Option<text::HanSlot>,
    ),
    Nodes(Vec<diting_taffy::tree::NodeId>),
}

#[allow(clippy::too_many_arguments)]
fn flush_run_into(
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    id: NodeId,
    style: &ComputedStyle,
    run_frame: u32,
    float_host: bool,
    run: &mut Vec<RunSeg>,
    direct: &mut Vec<diting_taffy::tree::NodeId>,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    clip: Option<NodeId>,
) {
    if run.is_empty() {
        return;
    }
    let segs = std::mem::take(run);
    let pending = text_src::drain_pending(run_frame);
    let mut pending_it = pending.iter().copied();
    // All-text run → one measured leaf (adjacent DOM text nodes
    // concatenate, which is also how CSS joins them). A run that is
    // ONLY formatting whitespace generates no box at all (CSS white-
    // space processing: it would otherwise sit between two blocks and
    // physically separate their adjoining margins, breaking collapse).
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
        if text.trim().is_empty()
            && !text.contains('\u{2028}')
            && !segs.iter().any(|s| matches!(s, RunSeg::Text(.., w, _, _) if w.preserves_spaces() || w.preserves_newlines()))
        {
            return;
        }
        let RunSeg::Text(_, fs, bold, color, lh, deco, vs, mono, wsp, _, sc, han) = &segs[0] else {
            unreachable!()
        };
        // white-space inherits, so the preserve-wins winner across
        // segments rides the whole run (the "any nowrap pins the run"
        // precedent, ranked across the family).
        let run_ws = segs.iter().fold(WhiteSpace::Normal, |acc, s| match s {
            RunSeg::Text(.., w, _, _) if ws_rank(*w) > ws_rank(acc) => *w,
            _ => acc,
        });
        // text-overflow is non-inherited and acts on the OWNING block's
        // inline overflow: the run truncates only when its owner also
        // clips — the classic nowrap + hidden + ellipsis pattern.
        // line-clamp shares the owner-block precedent (#271).
        let run_ellipsis = styles.get(&id).is_some_and(|s| {
            s.clips_descendants() && s.text_overflow == Some(TextOverflow::Ellipsis)
        });
        // line-clamp shares the owner-block precedent (#271) — but only
        // on the legacy -webkit-box face Chrome honors it on (#275).
        let run_clamp = styles.get(&id).and_then(|s| s.clamp_lines());
        // text-align is inherited, so the run carries the OWNING
        // block's physical value — the wrap path (block host) bakes
        // band-relative line alignment from it (#313); promoted hosts
        // never reach that code (flex parents hand no block ctx).
        let run_align = styles
            .get(&id)
            .and_then(|s| s.physical_text_align())
            .unwrap_or(TextAlign::Left);
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
                // An inline ELEMENT's run is the element's box (the box
                // dissolves at hoisting): beside a float an unbreakable
                // box drops below the band it cannot fit — bare text
                // overflows in place (#313).
                atomic: style.display == Some(CssDisplay::Inline),
                align: run_align,
                ellipsis: run_ellipsis,
                line_clamp: run_clamp,
                tokens: std::cell::RefCell::new(None),
                small_caps: *sc,
                han: *han,
                clip_src: clip,
            },
        ) {
            text_src::register(leaf, text_src::seg_ranges(&pending, &seg_lens));
            direct.push(leaf);
            return;
        }
    }
    // A lone plain inline element around the whole run (#314, float
    // hosts only): the hoist left exactly one Run leaf in Nodes — a
    // plain text run like the pure branch above, so mount it directly
    // instead of wrapping it. In the wrapper it is one max-content
    // atom: beside a float it cannot take a slot (drops past the
    // float) and the painter re-folds it across the float; mounted
    // directly it rides the same float_lines machinery as bare text.
    // Not beside floats the wrapper stays — it keeps the span's box
    // content-driven (its width, its overflow) and pins nowrap/pre
    // runs; inline-block boxes and mixed content are atomic by design.
    // Formatting whitespace around the element contributes no glyphs
    // (the pure branch's call for all-whitespace runs), and
    // pretty-printed markup always has some: `\n  <span>x</span>\n`
    // mounts directly.
    let content_segs: Vec<&RunSeg> = segs
        .iter()
        .filter(|s| {
            !matches!(s, RunSeg::Text(t, .., w, _, _) if t.trim().is_empty() && !w.preserves_spaces() && !w.preserves_newlines())
        })
        .collect();
    if float_host || float_flow::in_env() {
        if let [RunSeg::Nodes(children)] = &content_segs[..] {
            if children.len() == 1
                && matches!(
                    taffy_tree.get_node_context(children[0]),
                    // Foldable mounts beside the host's own floats; a
                    // nowrap/pre element mounts in ANY float environment
                    // — an inherited band counts too (a p inside the
                    // host) — because only float_lines can walk its
                    // unbreakable lines into the slots.
                    Some(TextLeaf::Run { ws, .. })
                        if float_host || ws.no_soft_wrap() || ws.preserves_newlines()
                )
            {
                direct.push(children[0]);
                return;
            }
        }
    }
    // #158: the element's own white-space seeds the fold — the
    // container governs the breaks between its inline content, and
    // segment values only pin further (the ranked "any nowrap pins
    // the run" precedent).
    let container_ws = styles
        .get(&id)
        .and_then(|s| s.white_space)
        .unwrap_or(WhiteSpace::Normal);
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
        let mut wstyle = run_wrapper_style(ws);
        // #313: the host's center/right text-align rides the wrapper's
        // justify — beside a float inline_flow shifts each line inside
        // its slot from it (the host's flex promote is suppressed on
        // the wrap path); away from floats it packs the flex row's
        // lines, the promote's box-centering posture as before.
        match styles.get(&id).and_then(|s| s.physical_text_align()) {
            Some(TextAlign::Center) => wstyle.justify_content = JustifyContent::CENTER,
            Some(TextAlign::Right) => wstyle.justify_content = JustifyContent::FLEX_END,
            _ => {}
        }
        if let Ok(wrapper) = taffy_tree.new_with_children(wstyle, &leaves) {
            run_wrappers.push(wrapper);
            direct.push(wrapper);
        }
    }
}

/// The non-float child partition of `build_element_inner` (moved out
/// verbatim): block-level children mount directly, inline content gathers
/// into runs flushed as measured leaves.
#[allow(clippy::too_many_arguments)]
fn build_flow_children(
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
    child_ids: &[NodeId],
    atomic_container: bool,
    float_host: bool,
    font_size: f32,
    lh_elem: f32,
    wrap_v2: bool,
    run_frame: u32,
    flush_run: &impl Fn(
        &mut Vec<RunSeg>,
        &mut Vec<diting_taffy::tree::NodeId>,
        &mut TaffyTree<TextLeaf>,
        &mut Vec<diting_taffy::tree::NodeId>,
        Option<NodeId>,
    ),
    direct: &mut Vec<diting_taffy::tree::NodeId>,
    run: &mut Vec<RunSeg>,
    run_clip: &mut Option<NodeId>,
) {
    for &child in child_ids {
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
          // Out-of-flow children (CSS blockification of abspos) never flatten
          // into an inline run — the reparent pass in layout_dom will move them
          // to their containing block anyway.
        let out_of_flow = styles.get(&child).is_some_and(|s| {
            matches!(
                s.position,
                Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
            )
        });
        if child_tag == "br" && !out_of_flow {
            // #275: inside an armed clamp the br folds into the run as a
            // U+2028 marker instead of flushing. The flush model splits
            // `ab<br>cd<br>ef` into three 1-line leaves, and a per-leaf
            // clamp can never see past one leaf — one run means one line
            // stack the clamp counts across, like Chrome. run_tokens
            // translates the marker to a zero-width forced break; the
            // params mirror the text-node branch so a LEADING br (marker
            // as first segment) still donates the run's real font. The
            // unclamped path keeps the block-model flush below (#146).
            if styles.get(&id).is_some_and(|s| s.clamp_lines().is_some()) {
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
                let nws = tree
                    .with_node(child, |n| n.parent)
                    .flatten()
                    .and_then(|p| styles.get(&p))
                    .and_then(|s| s.white_space)
                    .unwrap_or(WhiteSpace::Normal);
                text_src::note_seg(run_frame, child);
                run.push(RunSeg::Text(
                    "\u{2028}".to_string(),
                    fs,
                    b,
                    color_context(tree, child, styles),
                    lh,
                    decoration_context(tree, child, styles),
                    valign_shift(tree, child, styles, fonts),
                    mono_context(tree, child, styles),
                    word_spacing_context(tree, child, styles),
                    nws,
                    small_caps_context(tree, child, styles),
                    han_context(tree, child, styles),
                ));
                continue;
            }
            // #146: the forced break ends the current line. Flush as the
            // block model always did (the following content is its own
            // line), and give the br a line box only when the line it ends
            // is EMPTY — whitespace-only text and hoisted empty inline
            // boxes are not content. `display:none` br (the reset idiom)
            // breaks nothing and owns no line.
            let line_has_content = run.iter().any(|seg| match seg {
                RunSeg::Text(t, ..) => !t.trim().is_empty(),
                RunSeg::Nodes(n) => !n.is_empty(),
            });
            flush_run(run, direct, taffy_tree, run_wrappers, run_clip.take());
            if !line_has_content && child_display != Some(CssDisplay::None) {
                let (_, _, lh) = font_context(tree, child, styles, fonts);
                if let Some(strut) = br_strut_leaf(taffy_tree, lh) {
                    direct.push(strut);
                }
            }
            continue;
        }
        if !is_text && is_replaced_tag(&child_tag) {
            // Replaced elements are atomic: an inline-level box inside a run
            // (like a fat word), a direct item inside flex/grid or when the
            // UA/author made it block-level (our ua_display keeps img block).
            let inline_atom = inline_level || child_display == Some(CssDisplay::InlineBlock);
            // A floated replaced element is a float first (CSS float
            // blockifies): never run-inline, and on the wrap_v2 path it must
            // carry taffy's Float like its element siblings (mark), or it
            // stacks as a plain block above the text. Non-wrap_v2 containers
            // never reach here with float children — the zone model takes
            // them at the is_float_child branch above.
            let floated = wrap_v2
                && styles
                    .get(&child)
                    .is_some_and(|s| s.float_side.is_some() || s.clear_side.is_some());
            let sd = if inline_atom && !atomic_container && !out_of_flow && !floated {
                strut_descent_for(tree, child, styles, fonts)
            } else {
                0.0
            };
            let leaf = build_replaced_leaf(tree, child, styles, images, taffy_tree, node_map, sd);
            if let Some(leaf) = leaf {
                if inline_atom && !atomic_container && !out_of_flow && !floated {
                    run.push(RunSeg::Nodes(vec![leaf]));
                } else {
                    flush_run(run, direct, taffy_tree, run_wrappers, run_clip.take());
                    direct.push(leaf);
                    if floated {
                        float_flow::mark(taffy_tree, leaf, styles.get(&child));
                    }
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
            // (批240) — same per-segment posture as the other run builder.
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
            // First segment's node donates the whole run's color — the same
            // first-segment approximation fs/bold already use.
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
            *run_clip = Some(child);
            text_src::note_seg(run_frame, child);
            run.push(RunSeg::Text(
                text, fs, b, col, lh, deco, vs, mono, ws, nws, sc, han,
            ));
        } else if child_display == Some(CssDisplay::InlineBlock)
            && !atomic_container
            && !out_of_flow
        {
            // An inline-level block is ATOMIC (Chrome line-box model): it keeps
            // its own subtree box and joins the run as one unit, like a replaced
            // leaf. Flattening it would let its content wrap across the parent's
            // line; keeping the box makes taffy size it shrink-to-fit
            // (min(max-content, available)) against the run — and our wrapping
            // runs can't build the overflow bomb upstream's NoWrap rows did
            // (obscura#750).
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
        } else if inline_level
            && !atomic_container
            && !out_of_flow
            && !blockifies_for_block_child(tree, child, styles)
        {
            // A plain inline wrapper flattens into the enclosing run (upstream
            // is_flattenable_inline): the words wrap at the real block level.
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
                // #313: a pre* inline among OTHER content — Chrome breaks its
                // newlines as real line breaks in the parent's IFC. Hoisting
                // the whole run as one atom folds them into an internal
                // multi-line box (lead and the first hard line share no
                // parent line; tail never continues on the next). Beside a
                // float, explode at hard breaks and flush between — the
                // same shape as <br> — so lead shares the first hard line
                // and tail continues on the next. Away from floats the
                // atom posture stays (zone/paint already match).
                // Only among OTHER inline content — a lone pre still mounts
                // as one atom (float_lines walks its hard lines; #315's
                // content-sized sum uses the widest hard line).
                let has_lead = run.iter().any(|seg| match seg {
                    RunSeg::Text(tx, .., w, _, _) => {
                        !tx.trim().is_empty() || w.preserves_spaces() || w.preserves_newlines()
                    }
                    RunSeg::Nodes(n) => !n.is_empty(),
                });
                let has_tail = {
                    let mut past = false;
                    child_ids.iter().any(|&sib| {
                        if sib == child {
                            past = true;
                            return false;
                        }
                        if !past {
                            return false;
                        }
                        if tree.with_node(sib, |n| n.is_text()).unwrap_or(false) {
                            tree.with_node(sib, |n| {
                                n.text_content_of_text_node()
                                    .is_some_and(|tx| !tx.trim().is_empty())
                            }) == Some(true)
                        } else {
                            styles.get(&sib).is_some_and(|s| {
                                s.display.unwrap_or(CssDisplay::Block) == CssDisplay::Inline
                            })
                        }
                    })
                };
                let explode = (float_host || float_flow::in_env())
                    && (has_lead || has_tail)
                    && styles
                        .get(&child)
                        .and_then(|s| s.white_space)
                        .is_some_and(|w| w.preserves_newlines())
                    && sub_children.len() == 1
                    && matches!(
                        taffy_tree.get_node_context(sub_children[0]),
                        Some(TextLeaf::Run { ws, text, .. })
                            if ws.preserves_newlines() && (text.contains('\n') || text.contains('\r'))
                    );
                if explode {
                    let run_leaf = sub_children[0];
                    let ctx = taffy_tree.get_node_context(run_leaf).cloned();
                    let Some(TextLeaf::Run {
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
                        atomic,
                        align,
                        ellipsis,
                        line_clamp,
                        small_caps,
                        han,
                        clip_src,
                        ..
                    }) = ctx
                    else {
                        unreachable!("explode gated on Run");
                    };
                    // Pull the original leaf's text-src segs before it dies so
                    // Range/getClientRects on the span still resolve.
                    let orig_segs = text_src::take_building(run_leaf);
                    let _ = taffy_tree.remove(run_leaf);
                    node_map.remove(&sub);
                    let _ = taffy_tree.remove(sub);
                    let normalized = text.replace("\r\n", "\n").replace("\r", "\n");
                    let lines: Vec<&str> = normalized.split('\n').collect();
                    let mut flat: Vec<diting_taffy::tree::NodeId> = Vec::new();
                    let mut char_at = 0u32;
                    for (i, line) in lines.iter().enumerate() {
                        if i > 0 {
                            flush_run(run, direct, taffy_tree, run_wrappers, run_clip.take());
                            char_at += 1; // the newline between hard lines
                        }
                        let line_len = line.chars().count() as u32;
                        if let Ok(leaf) = taffy_tree.new_leaf_with_context(
                            float_flow::run_style(),
                            TextLeaf::Run {
                                text: (*line).to_string(),
                                font_size,
                                bold,
                                color,
                                line_height,
                                decorations,
                                baseline_shift,
                                mono,
                                word_spacing,
                                ws,
                                atomic,
                                align,
                                ellipsis,
                                line_clamp,
                                tokens: std::cell::RefCell::new(None),
                                small_caps,
                                han,
                                clip_src,
                            },
                        ) {
                            if let Some(ref segs) = orig_segs {
                                // Re-base + clamp the original run's segs onto
                                // this hard line's own text (leaf coords 0..line_len).
                                let from = char_at as i64;
                                let to = from + line_len as i64;
                                let sliced: Vec<(NodeId, i32, u32)> = segs
                                    .iter()
                                    .filter_map(|(n, s, l)| {
                                        let seg_a = *s as i64;
                                        let seg_b = seg_a + *l as i64;
                                        if seg_a >= to || seg_b <= from {
                                            return None;
                                        }
                                        let a = (seg_a - from).max(0) as i32;
                                        let b = (seg_b.min(to) - from) as i32;
                                        let len = (b - a).max(0) as u32;
                                        (len > 0).then_some((*n, a, len))
                                    })
                                    .collect();
                                text_src::register(leaf, sliced);
                            }
                            flat.push(leaf);
                            run.push(RunSeg::Nodes(vec![leaf]));
                        }
                        char_at += line_len;
                    }
                    if !flat.is_empty() {
                        flattened.insert(child, flat);
                    }
                } else {
                    run.push(RunSeg::Nodes(sub_children.clone()));
                    node_map.remove(&sub);
                    let _ = taffy_tree.remove(sub);
                    // Boxless after hoisting — union pass rebuilds the rect.
                    if !sub_children.is_empty() {
                        flattened.insert(child, sub_children);
                    }
                }
            }
        } else {
            flush_run(run, direct, taffy_tree, run_wrappers, run_clip.take());
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
                // wrap_v2 children carry their float/clear into taffy. A
                // plain-path container (no direct float children, so no zone
                // reification) still sees floats ESCAPED from descendants in
                // the shared BFC — its clear children must drop below them
                // (the case-div separators of #300's battery). Zone-model
                // containers returned earlier, so with the gate on marking
                // here is always safe; gate-off never marks, byte-identical.
                if float_flow::enabled() {
                    float_flow::mark(taffy_tree, node, styles.get(&child));
                }
                direct.push(node);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_element_inner(
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
) -> Option<diting_taffy::tree::NodeId> {
    // #187: an element missing from the styles map used to fall to
    // ComputedStyle::default(), whose display is None ≠ Some(None) — the
    // gate below passed and raw-text elements (<style>/<script>) laid
    // their text as visible runs (baidu homepage's injected styles
    // surfaced as body text at the top-left). A coverage gap now renders
    // hidden: a blank box beats leaked CSS text. The author-override path
    // (an explicit display present in the map) is untouched.
    let style = {
        let s = styles.get(&id)?;
        s.clone()
    };
    if style.display == Some(CssDisplay::None) {
        return None;
    }
    // #186 block-in-inline: an inline with an in-flow block-level child is
    // blockified whole (§9.2.1.1's anonymous split, approximated at element
    // granularity) so the block child fills the container width as a BFC
    // sibling instead of becoming a shrink-to-fit row item. The parent-side
    // attach decisions consult the same predicate so this element is NOT
    // flattened into a run.
    let style = if style.display == Some(CssDisplay::Inline)
        && blockifies_for_block_child(tree, id, styles)
    {
        let mut blockified = style;
        blockified.display = Some(CssDisplay::Block);
        blockified
    } else {
        style
    };
    let tag = tree
        .with_node(id, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    // Reached directly (root, or a replaced element someone recursed into):
    // replaced boxes own no layout children.
    if is_replaced_tag(&tag) {
        return build_replaced_leaf(tree, id, styles, images, taffy_tree, node_map, 0.0);
    }

    // --- table layout (display:table family) ------------------------------
    // A table dispatches here BEFORE the block/inline partition: its rows
    // are reified as flex rows of cells, never run through the flow logic.
    if style.display == Some(CssDisplay::Table) {
        return build_table(
            tree,
            id,
            &style,
            styles,
            images,
            fonts,
            taffy_tree,
            node_map,
            flattened,
            run_wrappers,
            meta,
        );
    }

    // Shadow hosts render their shadow tree here (composed-tree children;
    // unassigned light children and slot boxes never build).
    // #313: a plain block inside a wrap host's block flow inherits its
    // floats — in CSS only BFC boundaries stop line boxes from owing the
    // ancestor floats a band. A static, non-floated block is such a
    // passthrough; everything else (flex/grid/table, abspos/fixed,
    // floats) starts a fresh context.
    let in_float_env = float_flow::in_env()
        && style.display == Some(CssDisplay::Block)
        && !matches!(
            style.position,
            Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
        )
        && style.float_side.is_none();
    let child_ids: Vec<NodeId> = render_children(tree, id);
    let wrap_v2 = float_flow::eligible(tree, styles, id, &style, &child_ids);
    // Children see the host's floats — or the environment the host itself
    // inherited as a plain block.
    float_flow::set_env(wrap_v2 || in_float_env);
    // #314: beside real floats a lone plain inline element mounts directly
    // (its run leaf then takes the float slots like bare text). Anywhere
    // else the wrapper keeps the span's box content-driven — its own
    // width, scrollWidth overflow — which is the geometry spans owe.
    let float_host = wrap_v2
        && child_ids.iter().any(|cid| {
            styles
                .get(cid)
                .is_some_and(|s| s.float_side.is_some() && s.display != Some(CssDisplay::None))
        });

    // In a flex/grid container every element child is blockified into its own
    // item (CSS flex-item blockification); runs only form in block/inline
    // formatting contexts.
    let atomic_container = matches!(
        style.display,
        Some(CssDisplay::Flex) | Some(CssDisplay::Grid)
    );

    let mut direct: Vec<diting_taffy::tree::NodeId> = Vec::new();
    let mut run: Vec<RunSeg> = Vec::new();
    let mut run_clip: Option<NodeId> = None;
    let run_frame = text_src::new_frame();
    let flush_run = |run: &mut Vec<RunSeg>,
                     direct: &mut Vec<diting_taffy::tree::NodeId>,
                     taffy_tree: &mut TaffyTree<TextLeaf>,
                     run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
                     clip: Option<NodeId>| {
        flush_run_into(
            styles,
            fonts,
            id,
            &style,
            run_frame,
            float_host,
            run,
            direct,
            taffy_tree,
            run_wrappers,
            clip,
        )
    };

    let (font_size, _bold, lh_elem) = font_context(tree, id, styles, fonts);

    // --- float zone (batch 8b/8c/8h) -------------------------------------
    // taffy has no floats, so float shapes are REIFIED at tree-build time
    // (upstream obscura-render's build_children_with_float_zone):
    // - A RUN of ≥2 consecutive same-side floats (whitespace between them
    //   allowed) becomes ONE wrapping flex row (8c).
    // - A single float plus following siblings becomes [float | anonymous
    //   flow column]: the column takes the remaining width (8b).
    // - A `clear` sibling ends a zone and stays in normal flow after it.
    // - Zones interleave with normal siblings in DOCUMENT order (8h).
    let is_float_child = |cid: &NodeId| -> bool {
        styles
            .get(cid)
            .is_some_and(|s| s.float_side.is_some() && s.display != Some(CssDisplay::None))
    };
    if !wrap_v2 && child_ids.iter().any(is_float_child) {
        if let Some(done) = build_float_zone(
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
            &style,
            &child_ids,
            atomic_container,
            font_size,
            lh_elem,
            &is_float_child,
            &mut direct,
        ) {
            return Some(done);
        }
    } else {
        build_flow_children(
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
            &child_ids,
            atomic_container,
            float_host,
            font_size,
            lh_elem,
            wrap_v2,
            run_frame,
            &flush_run,
            &mut direct,
            &mut run,
            &mut run_clip,
        );
    }
    flush_run(
        &mut run,
        &mut direct,
        taffy_tree,
        run_wrappers,
        run_clip.take(),
    );
    // Sibling margin normalization runs over the FINAL child list of every
    // container (see collapse_adjacent_sibling_margins: fixes the
    // border/padding separation rule taffy's native block collapse lacks,
    // and supplies collapsing for flex stand-in parents).
    collapse_adjacent_sibling_margins(taffy_tree, styles, node_map, &direct);
    // #220: the flex-basis px carry needs this container's main axis —
    // only the parent knows it (see fix_flex_basis_carry).
    fix_flex_basis_carry(taffy_tree, &style, styles, node_map, &direct);

    let mut taffy_style = to_taffy_style(&style, pct_height_resolves(tree, styles, id));
    // #313: the wrap path keeps the host a plain block container — taffy
    // floats only exist in its block algorithm, and the flex-column promote
    // would demote them to misaligned flex items. Line alignment moves into
    // float_lines: the host's runs carry the physical text-align and shift
    // inside their float slots. A plain block AROUND floats (a centered p
    // beside the host's float, #314 tail) demotes for the same reason:
    // taffy hands the BlockContext only down a block chain, so the promote
    // would cut its lines out of the band — its alignment rides the run
    // wrapper's justify instead.
    if (wrap_v2 || in_float_env) && is_alignment_promote(&taffy_style) {
        taffy_style.display = Display::Block;
        taffy_style.flex_direction = FlexDirection::default();
        taffy_style.align_items = AlignItems::default();
    }
    if is_alignment_promote(&taffy_style) {
        stretch_block_children_in_aligned_parent(taffy_tree, node_map, styles, &direct);
    }
    let node = if direct.is_empty() {
        taffy_tree.new_leaf(taffy_style).ok()?
    } else {
        taffy_tree.new_with_children(taffy_style, &direct).ok()?
    };
    node_map.insert(node, id);
    resolve_sizing_keywords(taffy_tree, node, &style, fonts);
    Some(node)
}
