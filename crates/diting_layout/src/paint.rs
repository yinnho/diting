//! Raster paint: [`execute`] replays the [`PaintItem`] list that
//! [`layout_dom_with_paint`](super::layout_dom_with_paint) and its siblings
//! produce (backgrounds incl. gradients, borders/radius, text tiles, images,
//! svg ops) onto an RGBA [`Canvas`], in the stacking order layout computed.
//! Blend modes live in `paint/blend.rs`.

use super::forms::{paint_form_control, paint_form_widget};
use super::text::{
    baseline_offset, greedy_wrap, last_line_offset, tokens_of, truncate_tokens, HanSlot, PdfGlyph,
    ScaledMetrics, TextRaster, Token,
};
use super::{FontBook, PaintItem, Rect, TextGradient};
use diting_css::{MixBlendMode, TextDecorations, TextShadow, WhiteSpace};
use std::collections::HashSet;

/// Rough advance width: CJK/fullwidth ≈ 1em, everything else ≈ 0.6em.
pub(crate) fn est_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| if c > '\u{2E80}' { 1.0 } else { 0.6 })
        .sum::<f32>()
        * font_size
}

/// The page-space extent of the `Text` items alone, from the same wrap model
/// the band pre-filter and `rasterize_wrapped` use (height = y + lines ×
/// line-height, width = one wrapped line). Bare text owns no element box —
/// html/body stretch to the viewport — so this is the only place its true
/// extent exists: the scroll-union in `band_frame` and the print pump's page
/// count both read it. Errs high, never low.
/// Fold the wrap-model ink extent of every Text item, in CANVAS space
/// (blitz#841 transform half). Under a non-diagonal transform the collect
/// walk brackets the subtree's items in LOCAL coordinates (`SetXf`); a
/// naive union of raw x/y then overestimates the scrollable region — a
/// rotated long line reports its unrotated width as horizontal ink.
/// Track the bracket exactly like the paint pass (SetXf composes on top of
/// the open map, SetXfCanvas cancels it, ClearXf pops) and map each item's
/// ink box through the active map before it joins the union. Prebaked
/// (diagonal) chains carry no bracket, so their items — already in canvas
/// space — pass through an identity map and the fold is bit-identical to
/// the pre-transform behavior.
pub fn text_ink_extent(items: &[PaintItem]) -> (f32, f32) {
    let mut w = 0.0f32;
    let mut h = 0.0f32;
    let mut cur = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut open: Vec<[f32; 6]> = Vec::new();
    for item in items {
        match item {
            PaintItem::SetXf { xf } => {
                open.push(cur);
                cur = compose_arr(cur, *xf);
            }
            PaintItem::SetXfCanvas => {
                open.push(cur);
                cur = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
            }
            PaintItem::ClearXf => {
                if let Some(prev) = open.pop() {
                    cur = prev;
                }
            }
            PaintItem::Text {
                text,
                font_size,
                line_height,
                x,
                y,
                wrap_at,
                ..
            } => {
                let wrap = wrap_at.max(1.0);
                let est = est_width(text, *font_size);
                let lines = (est / wrap).ceil().max(1.0);
                let local = Rect {
                    x: *x,
                    y: *y,
                    width: est.min(wrap),
                    height: lines * line_height,
                };
                let r = super::map_rect_arr(cur, local);
                w = w.max(r.x + r.width);
                h = h.max(r.y + r.height);
            }
            _ => {}
        }
    }
    (w, h)
}

/// Compose two CSS-order affine arrays, `m` outer and `n` inner (m·n) —
/// array twin of the collect walk's fn-local `Xf::compose`.
fn compose_arr(m: [f32; 6], n: [f32; 6]) -> [f32; 6] {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

/// Replay the paint items onto `out`. `Bg` rects come from taffy's rounded
/// layout so the fill lands on whole pixels; each `Text` re-rasterizes
/// wrapped at the width its containing block offered at measure time, so
/// the tile's line structure is the measure function's own.
pub fn execute(items: &[PaintItem], fonts: &FontBook, out: &mut Canvas) {
    execute_band(items, fonts, out, 0.0, 0.0);
}

/// Fold a device-pixel-ratio scale into the item stream (#185): the caller
/// allocates the canvas at `vw·s × vh·s` and passes `dx·s, dy·s` to
/// [`execute_band`]; this pass multiplies every page-space geometry field by
/// `s`, so the pixel loops resolve edges and gradients at device resolution
/// and text re-shapes at the scaled font params instead of nearest-neighboring
/// a 1× bitmap up (the DPR bug's blurry face). `Text` DROPS its wrap-token
/// memo — the memo only matches unscaled shaping, the same rule the
/// collect-time diagonal fold follows. Items inside a `SetXf` bracket are
/// LOCAL coordinates: the bracket matrix alone takes the scale (S·M — execute
/// then folds the band shift into e/f as usual), scaling the locals too would
/// double-apply. `SetXfCanvas` regions are canvas-space and scale normally.
pub fn scale_items(items: &[PaintItem], s: f32) -> Vec<PaintItem> {
    if (s - 1.0).abs() < f32::EPSILON {
        return items.to_vec();
    }
    // Suffixed `_s` because the pattern-bound field names (`rect`,
    // `radii`) shadow any plain-named helper inside the arms.
    fn rect_s(r: &mut Rect, s: f32) {
        r.x *= s;
        r.y *= s;
        r.width *= s;
        r.height *= s;
    }
    fn radii_s(rs: &mut [(f32, f32); 4], s: f32) {
        for (rx, ry) in rs.iter_mut() {
            *rx *= s;
            *ry *= s;
        }
    }
    let mut out = Vec::with_capacity(items.len());
    // One entry per open bracket: true = local-coordinate span (scale rides
    // the matrix, item fields stay raw). SetXfCanvas hijacks the top bracket
    // into a canvas-space span.
    let mut stack: Vec<bool> = Vec::new();
    for item in items {
        let mut it = item.clone();
        match &mut it {
            PaintItem::SetXf { xf } => {
                for v in xf.iter_mut() {
                    *v *= s;
                }
                stack.push(true);
            }
            PaintItem::SetXfCanvas => {
                if let Some(top) = stack.last_mut() {
                    *top = false;
                } else {
                    stack.push(false);
                }
            }
            PaintItem::ClearXf => {
                stack.pop();
            }
            // Blend brackets carry no geometry (#222) — the group is the
            // span itself, scaled through whatever its items carry.
            PaintItem::BlendBegin { .. } | PaintItem::BlendEnd => {}
            _ if stack.last().copied().unwrap_or(false) => {}
            PaintItem::Bg { rect, radius, .. } => {
                rect_s(rect, s);
                *radius *= s;
            }
            PaintItem::BgCorner { rect, radii, .. } => {
                rect_s(rect, s);
                radii_s(radii, s);
            }
            PaintItem::BoxShadow {
                rect,
                radii,
                dx,
                dy,
                blur,
                spread,
                ..
            } => {
                rect_s(rect, s);
                radii_s(radii, s);
                *dx *= s;
                *dy *= s;
                *blur *= s;
                *spread *= s;
            }
            PaintItem::BackdropFilter { rect, radii, blur } => {
                rect_s(rect, s);
                radii_s(radii, s);
                *blur *= s;
            }
            PaintItem::BgGradient { rect, radii, .. } => {
                rect_s(rect, s);
                radii_s(radii, s);
            }
            PaintItem::Image {
                rect, paint_rect, ..
            } => {
                rect_s(rect, s);
                rect_s(paint_rect, s);
            }
            PaintItem::Replaced { rect, alt, .. } => {
                rect_s(rect, s);
                if let Some((_, font_size, _, line_height, _)) = alt {
                    *font_size *= s;
                    *line_height *= s;
                }
            }
            PaintItem::Svg { rect, .. } => rect_s(rect, s),
            PaintItem::Clip { rect } => rect_s(rect, s),
            PaintItem::ClipRounded { rect, radii } => {
                rect_s(rect, s);
                radii_s(radii, s);
            }
            PaintItem::PopClip => {}
            PaintItem::Border {
                rect,
                widths,
                radii,
                ..
            } => {
                rect_s(rect, s);
                for w in widths.iter_mut() {
                    *w *= s;
                }
                radii_s(radii, s);
            }
            PaintItem::Text {
                font_size,
                line_height,
                x,
                y,
                wrap_at,
                gradient,
                word_spacing,
                truncate_at,
                tokens,
                text_shadow,
                ..
            } => {
                *font_size *= s;
                *line_height *= s;
                *x *= s;
                *y *= s;
                *wrap_at *= s;
                *word_spacing *= s;
                if let Some(limit) = truncate_at {
                    *limit *= s;
                }
                // Scaled font params no longer match the leaf's measure-time
                // shaping — the paint re-shapes (same posture as the
                // collect-time diagonal fold).
                *tokens = None;
                if let Some(g) = gradient.as_mut() {
                    rect_s(&mut g.area, s);
                }
                if let Some(shadows) = text_shadow.as_mut() {
                    for sh in shadows.iter_mut() {
                        sh.dx *= s;
                        sh.dy *= s;
                        sh.blur *= s;
                    }
                }
            }
        }
        out.push(it);
    }
    out
}

/// Scale a straight-alpha color's alpha channel (animation batch A): the
/// pipeline composites straight-alpha source-over, so a group opacity folds
/// into item colors directly.
pub(crate) fn alpha_color(c: [u8; 4], a: f32) -> [u8; 4] {
    if a >= 1.0 {
        c
    } else {
        [c[0], c[1], c[2], (c[3] as f32 * a).round() as u8]
    }
}

// The colocated contract suite (batch 219: moved out with the file riding
// the god-file ratchet cap — the layering audit exempts tests.rs).
mod band;
mod pdf_text;
mod text_deco;
use band::box_blur_alpha;
pub use band::execute_band;
pub use pdf_text::{pdf_ops_translate, pdf_text_ops, PdfLine, PdfOp};
use text_deco::{paint_text_decorations, recolor_gradient_text};
mod canvas;
#[cfg(test)]
use band::MAX_SHADOW_BLUR;
use canvas::gradient_stop_color;
pub use canvas::Canvas;
#[cfg(test)]
use canvas::{lerp_premultiplied, sd_rounded_box};
mod blend;
#[cfg(test)]
mod tests;
