//! PDF text operators: vectorized runs and their clip brackets.
//!
//! Split from `paint.rs`. The public names stay re-exported from `paint`.
//! Pure move.

use super::*;

/// One vectorizable text run for the PDF text layer (vector-text batch):
/// shaped glyphs in absolute page px plus the decoration strokes, in the
/// item's own color. The writer embeds the same face bytes the shaper used,
/// so glyph ids pass through with no remap.
#[derive(Debug)]
pub struct PdfLine {
    pub font_size: f32,
    pub color: [u8; 4],
    pub glyphs: Vec<PdfGlyph>,
    /// (x, y_top, width) rects in page px; height is the decoration
    /// thickness derived from font_size at write time.
    pub strokes: Vec<(f32, f32, f32)>,
}

/// A clip (rect only) or text run, in document order — the PDF text layer's
/// mini-stream. Rounded clips don't appear here: a run inside one stays
/// raster (the vector layer can't express the corner cut), so only rect
/// clips need to bracket the runs.
#[derive(Debug)]
pub enum PdfOp {
    Clip { x: f32, y: f32, w: f32, h: f32 },
    PopClip,
    Line(PdfLine),
}

/// The text-layer half of a paint list: every `Text` item that can be
/// re-expressed as positioned glyphs, plus the rect-clip brackets it sits
/// under. Returns the ops in document order and the indexes of items the
/// caller must DROP from its raster pass (vectorized text painted twice
/// would double the antialiasing). Mirrors the raster walk exactly —
/// same token/pre-shaped path, same truncate-then-marker model, same
/// greedy wrap and per-line baselines — so vector and raster ink land on
/// the same pixels.
pub fn pdf_text_ops(items: &[PaintItem], fonts: &FontBook) -> (Vec<PdfOp>, HashSet<usize>) {
    let mut ops: Vec<PdfOp> = Vec::new();
    let mut vectorized: HashSet<usize> = HashSet::new();
    let mut clip_stack: Vec<bool> = Vec::new();
    let mut xf_depth = 0usize;
    for (idx, item) in items.iter().enumerate() {
        match item {
            PaintItem::Clip { rect } => {
                ops.push(PdfOp::Clip {
                    x: rect.x,
                    y: rect.y,
                    w: rect.width,
                    h: rect.height,
                });
                clip_stack.push(true);
            }
            PaintItem::ClipRounded { .. } => clip_stack.push(false),
            PaintItem::PopClip => {
                if clip_stack.pop() == Some(true) {
                    ops.push(PdfOp::PopClip);
                }
            }
            PaintItem::SetXf { .. } | PaintItem::SetXfCanvas => xf_depth += 1,
            PaintItem::ClearXf => xf_depth = xf_depth.saturating_sub(1),
            PaintItem::Text {
                text,
                font_size,
                bold,
                color,
                line_height,
                x,
                y,
                wrap_at,
                gradient,
                decorations,
                mono,
                word_spacing,
                truncate_at,
                tokens,
                ws,
                text_shadow,
                small_caps,
                han,
                last_line_align,
                ..
            } => {
                // Vector gate: the shaper must cover every segment (fallback
                // faces are emoji color bitmaps with no outlines), the fill
                // must be plain opaque (gradients paint per-pixel through
                // the glyphs, shadows layer under them, subtree opacity rides
                // color[3]), and the item must sit in page space — under a
                // transform bracket the coordinates are LOCAL, and under a
                // rounded clip the ink is corner-cut. Any miss keeps the item
                // raster, where the exact painting already exists.
                if text.trim().is_empty()
                    || gradient.is_some()
                    || text_shadow.is_some()
                    || color[3] != 255
                    || xf_depth != 0
                    || clip_stack.contains(&false)
                {
                    continue;
                }
                let Some(line) = pdf_vectorize_line(
                    text,
                    *font_size,
                    *bold,
                    *color,
                    *line_height,
                    *x,
                    *y,
                    *wrap_at,
                    *decorations,
                    *mono,
                    *word_spacing,
                    *truncate_at,
                    *ws,
                    *small_caps,
                    *han,
                    tokens.as_deref(),
                    fonts,
                    *last_line_align,
                ) else {
                    continue;
                };
                vectorized.insert(idx);
                ops.push(PdfOp::Line(line));
            }
            _ => {}
        }
    }
    (ops, vectorized)
}

/// Vectorize one `Text` item: shape its wrapped lines into positioned
/// glyphs (page px, absolute ink y) and collect the decoration strokes.
/// None = "can't express this as vector" (a fallback face somewhere, or
/// nothing would paint at all) — the caller keeps the item raster.
#[allow(clippy::too_many_arguments)]
fn pdf_vectorize_line(
    text: &str,
    font_size: f32,
    bold: bool,
    color: [u8; 4],
    line_height: f32,
    x: f32,
    y: f32,
    wrap_at: f32,
    decorations: TextDecorations,
    mono: bool,
    word_spacing: f32,
    truncate_at: Option<f32>,
    ws: WhiteSpace,
    small_caps: bool,
    han: Option<HanSlot>,
    pre_shaped: Option<&[Token]>,
    fonts: &FontBook,
    last_line_align: Option<diting_css::TextAlign>,
) -> Option<PdfLine> {
    let owned;
    let tokens: &[Token] = match pre_shaped {
        Some(t) => t,
        None => {
            owned = tokens_of(
                text,
                font_size,
                bold,
                fonts,
                mono,
                word_spacing,
                ws,
                small_caps,
                han,
            );
            &owned
        }
    };
    // Painted tokens: kept + the U+2026 marker appended, exactly the model
    // `rasterize_wrapped_uncached` paints; decorations wrap the kept set
    // only (the marker is undecorated, Chrome).
    let truncated;
    let painted: &[Token] = match truncate_at.and_then(|limit| {
        truncate_tokens(
            tokens,
            limit,
            font_size,
            bold,
            fonts,
            mono,
            word_spacing,
            ws,
            han,
        )
    }) {
        Some((mut kept, marker)) => {
            kept.extend(marker);
            truncated = kept;
            &truncated
        }
        None => tokens,
    };
    // #271: same markerless set as paint_text_decorations — strokes span
    // the kept tokens only (the clamp's "…" is ink, not decorated text).
    let stripped;
    let markerless: &[Token] = if tokens.iter().any(|t| t.is_ellipsis) {
        stripped = tokens
            .iter()
            .filter(|t| !t.is_ellipsis)
            .cloned()
            .collect::<Vec<_>>();
        &stripped
    } else {
        tokens
    };
    let truncated_kept;
    let kept: &[Token] = match truncate_at.and_then(|limit| {
        truncate_tokens(
            markerless,
            limit,
            font_size,
            bold,
            fonts,
            mono,
            word_spacing,
            ws,
            han,
        )
    }) {
        Some((k, _)) => {
            truncated_kept = k;
            &truncated_kept
        }
        None => markerless,
    };
    let lines = greedy_wrap(painted, Some(wrap_at.max(0.0)), ws);
    if lines.iter().all(|l| l.width <= 0.0) {
        return None;
    }
    let m = fonts.metrics(font_size, bold).unwrap_or(ScaledMetrics {
        ascent: font_size,
        descent: font_size * 0.2,
        line_gap: 0.0,
    });
    let b0 = baseline_offset(m.ascent, m.descent, line_height);

    let mut glyphs: Vec<PdfGlyph> = Vec::new();
    let shape_run = |run: &str, pen: f32, baseline: f32, out: &mut Vec<PdfGlyph>| -> bool {
        // Shaped advances/glyph ids, pen-relative → page space. The whole-line
        // string at pen 0 is the word-spacing==0 paint model; per non-space
        // token at the cumulative pen is the word-spacing>0 one (spaces only
        // advance, mirroring the raster token walk).
        let Some(gs) = fonts.pdf_shape(run, font_size, bold, mono, han) else {
            return false;
        };
        out.extend(gs.into_iter().map(|mut g| {
            g.x += x + pen;
            g.y += baseline;
            g
        }));
        true
    };
    if word_spacing == 0.0 {
        for (li, line) in lines.iter().enumerate() {
            if line.token_idx.is_empty() {
                continue;
            }
            let s: String = line
                .token_idx
                .iter()
                .map(|&i| painted[i].text.as_str())
                .collect();
            let baseline = y + (li as f32 * line_height).round() + b0;
            if !shape_run(
                &s,
                last_line_offset(&lines, li, last_line_align),
                baseline,
                &mut glyphs,
            ) {
                return None;
            }
        }
    } else {
        for (li, line) in lines.iter().enumerate() {
            let mut pen = last_line_offset(&lines, li, last_line_align);
            let baseline = y + (li as f32 * line_height).round() + b0;
            for &i in &line.token_idx {
                let t = &painted[i];
                if !t.is_space && !shape_run(&t.text, pen, baseline, &mut glyphs) {
                    return None;
                }
                pen += t.width;
            }
        }
    }
    if glyphs.is_empty() && decorations.is_empty() {
        return None;
    }

    let mut strokes: Vec<(f32, f32, f32)> = Vec::new();
    if !decorations.is_empty() {
        // Mirror paint_text_decorations to the pixel: kept-token wrap, same
        // baseline steps. Stroke height (1px per 16px font) is the writer's
        // business — it lives in the PDF op stream, not here.
        let dlines = greedy_wrap(kept, Some(wrap_at.max(0.0)), ws);
        for (i, line) in dlines.iter().enumerate() {
            if line.width <= 0.0 {
                continue;
            }
            let baseline = y + (i as f32 * line_height).round() + b0;
            let sx = x + last_line_offset(&dlines, i, last_line_align);
            if decorations.underline {
                strokes.push((sx, baseline + (m.descent * 0.5).max(1.0), line.width));
            }
            if decorations.overline {
                strokes.push((sx, baseline - m.ascent, line.width));
            }
            if decorations.line_through {
                strokes.push((sx, baseline - font_size * 0.28, line.width));
            }
        }
    }
    Some(PdfLine {
        font_size,
        color,
        glyphs,
        strokes,
    })
}

/// Bake page-space pdf ops into a band's local coordinates (band origin
/// subtraction) — the writer then needs no per-band offset.
pub fn pdf_ops_translate(ops: &mut [PdfOp], dx: f32, dy: f32) {
    for op in ops.iter_mut() {
        match op {
            PdfOp::Clip { x, y, .. } => {
                *x -= dx;
                *y -= dy;
            }
            PdfOp::Line(l) => {
                for g in l.glyphs.iter_mut() {
                    g.x -= dx;
                    g.y -= dy;
                }
                for (sx, sy, _) in l.strokes.iter_mut() {
                    *sx -= dx;
                    *sy -= dy;
                }
            }
            PdfOp::PopClip => {}
        }
    }
}
