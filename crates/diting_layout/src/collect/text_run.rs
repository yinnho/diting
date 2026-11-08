//! The text-run paint pass — split from the module root (god-file ratchet).

use super::*;

/// Text-run pass of one `collect` frame (moved out verbatim): shaped run
/// glyphs with decorations, ellipsis/line-clamp, shadows and the inherited
/// clip-text gradient.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_text_run(
    cx: &Ctx,
    out: &mut Out,
    node: diting_taffy::tree::NodeId,
    abs: (f32, f32),
    alpha: f32,
    xf: Xf,
    run_text_shadow: &Option<Vec<diting_css::TextShadow>>,
    text_gradient_fill: &Option<TextGradient>,
) {
    let Ctx {
        tree,
        taffy_tree,
        node_map,
        styles,
        fonts,
        viewport_width,
        ..
    } = *cx;
    if let Some(TextLeaf::Run {
        text,
        font_size,
        bold,
        color,
        line_height,
        decorations,
        mono,
        word_spacing,
        ws,
        ellipsis,
        line_clamp,
        tokens,
        small_caps,
        han,
        clip_src,
        ..
    }) = taffy_tree.get_node_context(node)
    {
        // text-align-last (#214, blitz#998): inherited, so the run's
        // OWNING block carries the computed value — walk taffy ancestors
        // to the first boxed element (pure runs hang directly off their
        // block; mixed-run wrappers sit a level deeper, but a mixed run
        // wraps via word leaves and never reaches here as one Run).
        // Physical resolution folds direction in.
        let last_line_align = {
            let mut anc = taffy_tree.parent(node);
            let mut resolved = None;
            while let Some(a) = anc {
                if let Some(dom) = node_map.get(&a) {
                    resolved = styles.get(dom).and_then(|s| s.physical_text_align_last());
                    break;
                }
                anc = taffy_tree.parent(a);
            }
            resolved
        };
        // The wrap width the containing block offered at measure time:
        // the direct taffy parent's content box (the run wrapper for
        // mixed runs, the block itself for pure runs — same width).
        // Scaled geometry (animation batch B): position maps through
        // the affine, font metrics ride d, the wrap width a — exact
        // for uniform scales, the documented approximation otherwise.
        // Under a non-diagonal map (affine batch) the leaf emits RAW
        // local geometry and the canvas-side SetXf bracket maps the
        // rasterized tile per pixel.
        // Inline clip:text fill (#218): a span owns no box — rebuilt from its line geometry.
        let toks = run_tokens(
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
        // line-clamp (#271): bake the clamp into the run's visible
        // token stream at the unscaled wrap width, and swap the item's
        // text to the clamped string — every downstream re-wrap (raster,
        // decorations, PDF, band estimates) then reproduces the same N
        // lines for free, and the RasterCache keys clamped and full runs
        // apart. Scaled (d≠1) runs lose the token memo as before and
        // re-shape the clamped string; a rotated clamp can re-break the
        // last line (wrap width maps through a), the documented v1 edge.
        let clamp_wrap = taffy_tree
            .parent(node)
            .and_then(|p| taffy_tree.layout(p).ok())
            .map(|l| l.content_box_width())
            .unwrap_or(viewport_width);
        let clamped = (*line_clamp).filter(|_| !ws.no_soft_wrap()).and_then(|n| {
            text::line_clamp_bake(
                &toks,
                n,
                clamp_wrap,
                *ws,
                *font_size,
                *bold,
                fonts,
                *mono,
                *word_spacing,
                *han,
            )
        });
        let full_text = text.clone();
        let toks: std::rc::Rc<[text::Token]> = match &clamped {
            Some((t, _)) => std::rc::Rc::from(t.as_slice()),
            None => toks,
        };
        let text: std::borrow::Cow<'_, str> = match &clamped {
            Some((_, s)) => s.as_str().into(),
            None => full_text.into(),
        };
        let clip_fill = |x: f32, y: f32, wrap_at: f32, lh: f32, s: (f32, f32)| {
            clip_text::run_fill(
                text_gradient_fill.clone(),
                *clip_src,
                tree,
                styles,
                x,
                y,
                wrap_at,
                lh,
                &toks,
                *ws,
                last_line_align,
                alpha,
                s,
            )
        };
        // Beside a float the lines have different widths and starts: one
        // item per line, so the painter needs no per-line state.
        let slotted = if xf.is_diagonal()
            && clamped.is_none()
            && !matches!(
                last_line_align,
                Some(TextAlign::Center) | Some(TextAlign::Right)
            ) {
            float_lines::slots_of(node)
        } else {
            None
        };
        if let Some(slots) = slotted {
            let src = text_src::lookup(node);
            for line in float_lines::split(&text, &toks, *ws, *line_height, &slots) {
                let (lx, ly) = (
                    (abs.0 + line.slot.x) * xf.a + xf.e,
                    (abs.1 + line.slot.y) * xf.d + xf.f,
                );
                out.items.push(PaintItem::Text {
                    text: line.text,
                    font_size: font_size * xf.d,
                    bold: *bold,
                    color: with_alpha(*color, alpha),
                    line_height: line_height * xf.d,
                    x: lx,
                    y: ly,
                    wrap_at: f32::INFINITY,
                    gradient: clip_fill(
                        lx,
                        ly,
                        line.slot.width * xf.a,
                        *line_height * xf.d,
                        (xf.a, xf.d),
                    ),
                    decorations: *decorations,
                    mono: *mono,
                    word_spacing: word_spacing * xf.d,
                    truncate_at: None,
                    tokens: if xf.d == 1.0 { Some(line.tokens) } else { None },
                    ws: *ws,
                    text_shadow: run_text_shadow.clone(),
                    small_caps: *small_caps,
                    han: *han,
                    last_line_align: None,
                    src: src.as_ref().map(|s| text_src::slice(s, line.from, line.to)),
                });
            }
        } else if xf.is_diagonal() {
            let wrap_at = taffy_tree
                .parent(node)
                .and_then(|p| taffy_tree.layout(p).ok())
                .map(|l| l.content_box_width())
                .unwrap_or(viewport_width)
                * xf.a;
            // no-soft-wrap: wrap at +inf (a distinct RasterKey), ellipsis
            // truncates at the real box width.
            let truncate_at = if *ellipsis && ws.no_soft_wrap() {
                Some(wrap_at)
            } else {
                None
            };
            let wrap_at = if ws.no_soft_wrap() {
                f32::INFINITY
            } else {
                wrap_at
            };
            out.items.push(PaintItem::Text {
                text: text.clone().into_owned(),
                font_size: font_size * xf.d,
                bold: *bold,
                color: with_alpha(*color, alpha),
                line_height: line_height * xf.d,
                x: abs.0 * xf.a + xf.e,
                y: abs.1 * xf.d + xf.f,
                wrap_at,
                gradient: clip_fill(
                    abs.0 * xf.a + xf.e,
                    abs.1 * xf.d + xf.f,
                    wrap_at,
                    *line_height * xf.d,
                    (xf.a, xf.d),
                ),
                decorations: *decorations,
                mono: *mono,
                word_spacing: word_spacing * xf.d,
                truncate_at,
                // obscura#983 paint half: hand the leaf's measure-time
                // wrap tokens to the item (unscaled only — a diagonal
                // scale folds d and the memo no longer matches).
                tokens: if xf.d == 1.0 {
                    Some(toks.clone())
                } else {
                    None
                },
                ws: *ws,
                text_shadow: run_text_shadow.clone(),
                small_caps: *small_caps,
                han: *han,
                last_line_align,
                src: text_src::lookup(node),
            });
        } else {
            let wrap_at = taffy_tree
                .parent(node)
                .and_then(|p| taffy_tree.layout(p).ok())
                .map(|l| l.content_box_width())
                .unwrap_or(viewport_width);
            let truncate_at = if *ellipsis && ws.no_soft_wrap() {
                Some(wrap_at)
            } else {
                None
            };
            let wrap_at = if ws.no_soft_wrap() {
                f32::INFINITY
            } else {
                wrap_at
            };
            out.items.push(PaintItem::Text {
                text: text.clone().into_owned(),
                font_size: *font_size,
                bold: *bold,
                color: with_alpha(*color, alpha),
                line_height: *line_height,
                x: abs.0,
                y: abs.1,
                wrap_at,
                gradient: clip_fill(abs.0, abs.1, wrap_at, *line_height, (1.0, 1.0)),
                decorations: *decorations,
                mono: *mono,
                word_spacing: *word_spacing,
                truncate_at,
                tokens: Some(toks),
                ws: *ws,
                text_shadow: run_text_shadow.clone(),
                small_caps: *small_caps,
                han: *han,
                last_line_align,
                src: text_src::lookup(node),
            });
        }
    }
}
