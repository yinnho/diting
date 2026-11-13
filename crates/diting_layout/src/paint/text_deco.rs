//! Text decorations and gradient-clipped text recolor.
//!
//! Split from `paint.rs`. The band replay still calls these. Pure move.

use super::*;

/// background-clip: text fill (gradient-text batch): rewrite an
/// opaque-white text raster in place into the gradient — each covered
/// pixel samples at its own position `(x + gx, y + top + gy)` in the item's
/// coordinate space, final alpha = glyph coverage × stop alpha. The
/// projection math mirrors [`Canvas::fill_gradient`] (same CSS gradient
/// line convention), minus the shape test: coverage IS the shape.
/// `box-decoration-break: clone` (#218) swaps the one `area` for the
/// pixel's own wrapped-line fragment box, so every line restarts the
/// gradient — the line index comes from the row's y against the item's
/// line boxes, exactly where the raster laid them.
pub(super) fn recolor_gradient_text(
    r: &mut TextRaster,
    x: f32,
    y: f32,
    line_height: f32,
    g: &TextGradient,
) {
    if r.width == 0 || r.height == 0 || g.stops.len() < 2 {
        return;
    }
    let rad = g.css_deg.to_radians() as f64;
    let (dux, duy) = (rad.sin(), -rad.cos());
    // Pixel row 0's y in the item space (the compositor blits at
    // (x, y + top)) — row_base per row, fused multiply-add per pixel.
    let tile_top = y as f64 + r.top as f64;
    for gy in 0..r.height {
        let (bx, by, bw, bh) = if g.clone_lines.is_empty() {
            (g.area.x, g.area.y, g.area.width, g.area.height)
        } else {
            let idx = (((r.top + gy as f32) / line_height).floor().max(0.0) as usize)
                .min(g.clone_lines.len() - 1);
            let (lox, lw) = g.clone_lines[idx];
            (x + lox, y + idx as f32 * line_height, lw, line_height)
        };
        let len = (bw as f64 * dux.abs() + bh as f64 * duy.abs()).max(1.0);
        let (ccx, ccy) = (bx as f64 + bw as f64 / 2.0, by as f64 + bh as f64 / 2.0);
        let col_a = dux / len;
        let row_base = (tile_top + gy as f64 - ccy) * duy / len + 0.5;
        for gx in 0..r.width {
            let i = (gy * r.width + gx) * 4;
            let coverage = r.data[i + 3];
            if coverage == 0 {
                continue;
            }
            let t = (((x as f64 + gx as f64) - ccx) * col_a + row_base).clamp(0.0, 1.0) as f32;
            let c = gradient_stop_color(&g.stops, t);
            r.data[i] = c[0];
            r.data[i + 1] = c[1];
            r.data[i + 2] = c[2];
            r.data[i + 3] = ((c[3] as u32 * coverage as u32 + 127) / 255) as u8;
        }
    }
}

/// Paint a text item's `text-decoration-line` set — one rect per wrapped
/// line, in the text's own color. Line breaks and baselines replay the
/// exact `rasterize_wrapped` math (same tokens, same `baseline_offset`), so
/// the strokes sit where the glyphs are regardless of wrap width. Geometry
/// is font-metric derived (CSS Text Decoration 4's `auto` position/thickness
/// family): underline rides half the descent under the baseline,
/// line-through sits at ~x-height, overline caps the ascent; thickness
/// scales with font size at 1px per 16px. #218 (takumi #1802): when the
/// DECORATING element itself clips a gradient through its text, Chrome
/// strokes with the gradient (`text-decoration-color` loses) — `gradient`
/// carries that fill and each column samples it in page space.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_text_decorations(
    out: &mut Canvas,
    fonts: &FontBook,
    text: &str,
    font_size: f32,
    bold: bool,
    color: [u8; 4],
    line_height: f32,
    x: f32,
    y: f32,
    wrap_at: f32,
    decorations: TextDecorations,
    gradient: Option<&TextGradient>,
    mono: bool,
    word_spacing: f32,
    truncate_at: Option<f32>,
    ws: WhiteSpace,
    small_caps: bool,
    han: Option<HanSlot>,
    // The item's wrap tokens, pre-shaped when the item carries the run
    // leaf's memo (unscaled font params): the decorations painter needs the
    // wrap LINES, which the glyph-pixel RasterCache doesn't hold, so
    // without this every repaint re-shaped the run here.
    pre_shaped: Option<&[Token]>,
    dx: f32,
    dy: f32,
    // #214: the item's text-align-last — the last line's stroke follows the
    // glyphs' offset, else an underlined last line would keep its stroke on
    // the start edge while the ink moved.
    last_line_align: Option<diting_css::TextAlign>,
) {
    if decorations.is_empty() || text.trim().is_empty() {
        return;
    }
    let owned;
    let tokens = match pre_shaped {
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
    // The ellipsis marker is undecorated (Chrome): strokes span the kept
    // tokens only, so underline/line-through end at the truncation cut.
    let kept;
    let tokens: &[Token] = match truncate_at.and_then(|limit| {
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
        Some((t, _marker)) => {
            kept = t;
            &kept
        }
        None => tokens,
    };
    // #271: the clamp's "…" marker rides the token stream but is
    // undecorated (Chrome) — wrap a markerless copy so strokes stop at
    // the cut, never under the ellipsis.
    let stripped;
    let tokens: &[Token] = if tokens.iter().any(|t| t.is_ellipsis) {
        stripped = tokens
            .iter()
            .filter(|t| !t.is_ellipsis)
            .cloned()
            .collect::<Vec<_>>();
        &stripped
    } else {
        tokens
    };
    let lines = greedy_wrap(tokens, Some(wrap_at.max(0.0)), ws);
    let m = fonts.metrics(font_size, bold).unwrap_or(ScaledMetrics {
        ascent: font_size,
        descent: font_size * 0.2,
        line_gap: 0.0,
    });
    let b0 = baseline_offset(m.ascent, m.descent, line_height);
    let thickness = (font_size / 16.0).round().max(1.0);
    let mut stroke = |lx: f32, ly: f32, w: f32, li: usize| {
        if w <= 0.0 {
            return;
        }
        if let Some(g) = gradient.filter(|g| g.stops.len() >= 2) {
            // Same sampling math as `recolor_gradient_text`: the gradient
            // area is the whole run's (or the clone line's) box, and the
            // stroke's page-space x drives the color while the fill itself
            // still lands at the band-shifted canvas x.
            let (bx, by, bw, bh) = if g.clone_lines.is_empty() {
                (g.area.x, g.area.y, g.area.width, g.area.height)
            } else {
                let (lox, lw) = g.clone_lines[li.min(g.clone_lines.len() - 1)];
                (x + lox, y + li as f32 * line_height, lw, line_height)
            };
            let rad = g.css_deg.to_radians() as f64;
            let (dux, duy) = (rad.sin(), -rad.cos());
            let len = (bw as f64 * dux.abs() + bh as f64 * duy.abs()).max(1.0);
            let (ccx, ccy) = (bx as f64 + bw as f64 / 2.0, by as f64 + bh as f64 / 2.0);
            let row = (ly as f64 + thickness as f64 / 2.0 - ccy) * duy / len + 0.5;
            let tw = w.round().max(1.0) as usize;
            let th = thickness as usize;
            let mut tile = vec![0u8; tw * th * 4];
            for c in 0..tw {
                let t = (((lx + c as f32) as f64 - ccx) * dux / len + row).clamp(0.0, 1.0);
                let col = gradient_stop_color(&g.stops, t as f32);
                for r in 0..th {
                    tile[(r * tw + c) * 4..(r * tw + c) * 4 + 4].copy_from_slice(&col);
                }
            }
            if out.xf().is_some() {
                out.blit_rgba_affine(&tile, tw, th, lx as f64, ly as f64);
            } else {
                let (x0, y0) = ((lx - dx).round() as i64, (ly - dy).round() as i64);
                for c in 0..tw {
                    out.fill_rect(
                        x0 + c as i64,
                        y0,
                        1,
                        th as i64,
                        tile[c * 4..c * 4 + 4].try_into().unwrap(),
                    );
                }
            }
            return;
        }
        if out.xf().is_some() {
            // fill_rect paints raw canvas coords — under a transform bracket
            // the stroke needs the affine too, so blit a solid tile instead.
            let tw = w.ceil().max(1.0) as usize;
            let th = thickness as usize;
            let mut tile = vec![0u8; tw * th * 4];
            for px in tile.as_chunks_mut::<4>().0 {
                px.copy_from_slice(&color);
            }
            out.blit_rgba_affine(&tile, tw, th, lx as f64, ly as f64);
        } else {
            out.fill_rect(
                (lx - dx).round() as i64,
                (ly - dy).round() as i64,
                w.round().max(1.0) as i64,
                thickness as i64,
                color,
            );
        }
    };
    for (i, line) in lines.iter().enumerate() {
        if line.width <= 0.0 {
            continue;
        }
        let baseline = y + (i as f32 * line_height).round() + b0;
        let lx = x + last_line_offset(&lines, i, last_line_align);
        if decorations.underline {
            stroke(lx, baseline + (m.descent * 0.5).max(1.0), line.width, i);
        }
        if decorations.overline {
            stroke(lx, baseline - m.ascent, line.width, i);
        }
        if decorations.line_through {
            stroke(lx, baseline - font_size * 0.28, line.width, i);
        }
    }
}
