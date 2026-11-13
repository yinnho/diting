//! Viewport-band replay of a [`PaintItem`] list.
//!
//! Split from `paint.rs`. `execute` is the unshifted face and calls
//! `execute_band`. Pure move.

use super::*;

/// Replay the paint items shifted by `(-dx, -dy)` — the viewport-band paint:
/// with `dy` at the band's page-space top, only the band's rows land on the
/// canvas and everything else falls outside the bounds (every primitive's
/// pixel loop intersects with `allowed()`, so off-band rects/borders/images
/// cost an empty range). `Text`/`Replaced` rasterize at paint time, so they
/// get a cheap bounds estimate first — a skip can only ever drop ink that
/// the estimate put outside the band, and the estimate errs high.
pub fn execute_band(items: &[PaintItem], fonts: &FontBook, out: &mut Canvas, dx: f32, dy: f32) {
    // Open blend groups (#222): the saved canvas is the backdrop layer,
    // the redirected `out` collects the group in isolation until BlendEnd
    // blends it back. Empty on pages without mix-blend-mode — zero cost.
    let mut blend_stack: Vec<(Canvas, MixBlendMode)> = Vec::new();
    // Whether a text tile's ink can reach the band. Line count comes from
    // the same wrap model rasterize_wrapped uses; `top` can lift ink above
    // the line-box top by up to a line's leading, so the top edge gets a
    // full line of slack.
    #[allow(clippy::too_many_arguments)]
    fn text_reaches_band(
        y: f32,
        text: &str,
        font_size: f32,
        wrap_at: f32,
        line_height: f32,
        dy: f32,
        band_h: i64,
        pad: f32,
    ) -> bool {
        let wrap = wrap_at.max(1.0);
        let lines = (est_width(text, font_size) / wrap).ceil().max(1.0);
        let top = y - line_height;
        let bottom = y + lines * line_height;
        bottom + pad > dy && top - pad < dy + band_h as f32
    }

    for item in items {
        match item {
            PaintItem::SetXf { xf } => {
                // The stored map is in page space; the band shift folds in
                // here — the canvas-space total is T(−dx,−dy)∘M, whose e/f
                // are exactly M.e − dx / M.f − dy.
                out.push_xf([
                    xf[0] as f64,
                    xf[1] as f64,
                    xf[2] as f64,
                    xf[3] as f64,
                    xf[4] as f64 - dx as f64,
                    xf[5] as f64 - dy as f64,
                ]);
            }
            PaintItem::SetXfCanvas => {
                // Canvas-space emission (the inline-band splice): replace
                // the open map with the band shift alone — identity in the
                // full-page execute — so the bracket's transform cancels.
                out.push_xf_canvas([1.0, 0.0, 0.0, 1.0, -dx as f64, -dy as f64]);
            }
            PaintItem::ClearXf => {
                out.pop_xf();
            }
            PaintItem::Clip { rect } => {
                if out.xf().is_some() {
                    out.push_clip_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        [(0.0, 0.0); 4],
                    );
                } else {
                    out.push_clip(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        (rect.x + rect.width - dx).round() as i64,
                        (rect.y + rect.height - dy).round() as i64,
                    );
                }
            }
            PaintItem::ClipRounded { rect, radii } => {
                if out.xf().is_some() {
                    out.push_clip_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        *radii,
                    );
                } else {
                    out.push_rounded_clip(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        (rect.x + rect.width - dx).round() as i64,
                        (rect.y + rect.height - dy).round() as i64,
                        *radii,
                    );
                }
            }
            PaintItem::PopClip => {
                out.pop_clip();
            }
            PaintItem::BlendBegin { mode } => {
                blend::redirect(out, &mut blend_stack, *mode);
            }
            PaintItem::BlendEnd => {
                blend::restore(out, &mut blend_stack);
            }
            PaintItem::BgCorner {
                rect, color, radii, ..
            } => {
                if out.xf().is_some() {
                    out.fill_shape_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        *radii,
                        *color,
                    );
                } else {
                    out.fill_corner_rect(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        rect.width.round() as i64,
                        rect.height.round() as i64,
                        *radii,
                        *color,
                    );
                }
            }
            PaintItem::BackdropFilter { rect, radii, blur } => {
                // v1: skipped under an open transform bracket (no
                // canvas-space snapshot on the affine path); axis-aligned
                // glass is the blitz#901 shape.
                if out.xf().is_none() && *blur > 0.0 {
                    out.blur_backdrop(
                        rect.x - dx,
                        rect.y - dy,
                        rect.width,
                        rect.height,
                        *radii,
                        *blur,
                    );
                }
            }
            PaintItem::BoxShadow {
                rect,
                color,
                radii,
                dx: sdx,
                dy: sdy,
                blur,
                spread,
                inset,
            } => {
                if out.xf().is_some() {
                    out.fill_shadow_affine(
                        rect, *color, *radii, *sdx, *sdy, *blur, *spread, *inset,
                    );
                } else {
                    // No bracket: the collect walk's paint translation still
                    // applies (the affine path folds it into the matrix).
                    let moved = Rect {
                        x: rect.x - dx,
                        y: rect.y - dy,
                        width: rect.width,
                        height: rect.height,
                    };
                    out.fill_box_shadow(&moved, *color, *radii, *sdx, *sdy, *blur, *spread, *inset);
                }
            }
            PaintItem::Bg {
                rect,
                color,
                radius,
                ..
            } => {
                if out.xf().is_some() {
                    out.fill_shape_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        [(*radius, *radius); 4],
                        *color,
                    );
                } else {
                    out.fill_rounded_rect(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        rect.width.round() as i64,
                        rect.height.round() as i64,
                        *radius,
                        *color,
                    );
                }
            }
            PaintItem::BgGradient {
                rect,
                stops,
                css_deg,
                radii,
            } => {
                if out.xf().is_some() {
                    out.fill_gradient_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        stops,
                        *css_deg,
                        *radii,
                    );
                } else {
                    out.fill_gradient(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        rect.width.round() as i64,
                        rect.height.round() as i64,
                        stops,
                        *css_deg,
                        *radii,
                    );
                }
            }
            PaintItem::Border {
                rect,
                widths,
                color,
                radii,
            } => {
                if out.xf().is_some() {
                    out.border_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        *widths,
                        *radii,
                        *color,
                    );
                } else if radii.iter().all(|r| r.0 <= 0.0 && r.1 <= 0.0) {
                    // Four bands, square corners: top/bottom span the full
                    // border-box width (they own the corners), left/right inset
                    // by the top/bottom widths — the classic rectangular-border
                    // paint browsers produce with radius 0 (the historical
                    // fast path, bit-for-bit).
                    let [t, r, b, l] = *widths;
                    let (x, y) = ((rect.x - dx).round() as i64, (rect.y - dy).round() as i64);
                    let (w, h) = (rect.width.round() as i64, rect.height.round() as i64);
                    // #38: widths arrive fractional in the wild (border:
                    // 0.8px) and the old `as i64` cast truncated every side
                    // below 1px to zero — the border vanished entirely. Snap
                    // each side to the nearest device pixel and scale that
                    // side's alpha by the coverage (0.8px -> 1px at 0.8
                    // alpha), the integer-canvas stand-in for Chrome's
                    // antialiasing. Exact-zero sides still paint nothing;
                    // integer widths stay bit-for-bit the old bands.
                    let band = |cw: f32| -> (i64, [u8; 4]) {
                        if cw <= 0.0 {
                            return (0, *color);
                        }
                        let pix = (cw.round() as i64).max(1);
                        let cov = (cw / pix as f32).clamp(0.0, 1.0);
                        let mut c = *color;
                        c[3] = ((c[3] as f32) * cov).round() as u8;
                        (pix, c)
                    };
                    let (tp, tc) = band(t);
                    let (bp, bc) = band(b);
                    let (lp, lc) = band(l);
                    let (rp, rc) = band(r);
                    out.fill_rect(x, y, w, tp, tc);
                    out.fill_rect(x, y + h - bp, w, bp, bc);
                    out.fill_rect(x, y + tp, lp, h - tp - bp, lc);
                    out.fill_rect(x + w - rp, y + tp, rp, h - tp - bp, rc);
                } else {
                    // Rounded ring: outer rounded box minus the widths-inset
                    // inner rounded box — Chrome's border shape when
                    // border-radius meets a border.
                    out.fill_border_ring(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        rect.width.round() as i64,
                        rect.height.round() as i64,
                        *widths,
                        *radii,
                        *color,
                    );
                }
            }
            PaintItem::Image {
                rect,
                paint_rect,
                image,
                alpha,
            } => {
                if out.xf().is_some() {
                    // Replaced content still clips to the element box — the
                    // clip rides the same bracket.
                    out.push_clip_affine(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                        [(0.0, 0.0); 4],
                    );
                    out.blit_image_affine(
                        image,
                        paint_rect.x as f64,
                        paint_rect.y as f64,
                        paint_rect.width as f64,
                        paint_rect.height as f64,
                        *alpha,
                    );
                    out.pop_clip();
                } else {
                    // Replaced content is always clipped to the element box
                    // (upstream clips image elements regardless of overflow);
                    // object-fit cover/object-position can push paint_rect past
                    // `rect`, so clip the blit to the box.
                    out.push_clip(
                        (rect.x - dx).round() as i64,
                        (rect.y - dy).round() as i64,
                        (rect.x + rect.width - dx).round() as i64,
                        (rect.y + rect.height - dy).round() as i64,
                    );
                    out.blit_image(
                        image,
                        (paint_rect.x - dx).round() as i64,
                        (paint_rect.y - dy).round() as i64,
                        paint_rect.width.round() as i64,
                        paint_rect.height.round() as i64,
                        *alpha,
                    );
                    out.pop_clip();
                }
            }
            PaintItem::Replaced {
                rect,
                alt,
                fill_placeholder,
                widget,
                form,
                alpha,
                caret,
                form_rtl,
            } => {
                if out.xf().is_some() {
                    // Rasterize the placeholder + alt into a transparent
                    // LOCAL scratch at raw metrics (the bracket maps the
                    // tile; alt text must not bake the scale in), then blit
                    // it through the map.
                    let (w, h) = (rect.width.round() as i64, rect.height.round() as i64);
                    if w <= 0 || h <= 0 {
                        continue;
                    }
                    let (bx0, by0, bx1, by1) = out.mapped_xf_bounds(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                    );
                    if bx1 <= 0 || by1 <= 0 || bx0 >= out.width as i64 || by0 >= out.height as i64 {
                        continue;
                    }
                    let (w, h) = (w as usize, h as usize);
                    let mut scratch = Canvas::new_transparent(w, h);
                    if let Some(widget) = widget {
                        paint_form_widget(
                            &mut scratch,
                            0,
                            0,
                            w as i64,
                            h as i64,
                            *widget,
                            fonts,
                            *alpha,
                        );
                    } else if let Some(form) = form {
                        paint_form_control(
                            &mut scratch,
                            0,
                            0,
                            w as i64,
                            h as i64,
                            alt.as_ref(),
                            *form,
                            *fill_placeholder,
                            fonts,
                            *alpha,
                            *caret,
                            *form_rtl,
                        );
                    } else {
                        if *fill_placeholder {
                            scratch.fill_rect(
                                0,
                                0,
                                w as i64,
                                h as i64,
                                alpha_color([224, 224, 224, 255], *alpha),
                            );
                        }
                        if let Some((text, font_size, bold, line_height, color)) = alt {
                            if !text.trim().is_empty() {
                                // Ink clips to the box (batch 6e), now in the
                                // scratch's own coordinates.
                                scratch.push_clip(0, 0, w as i64, h as i64);
                                let r = fonts.rasterize_wrapped(
                                    text,
                                    *font_size,
                                    *bold,
                                    alpha_color(*color, *alpha),
                                    w as f32,
                                    *line_height,
                                    false,
                                    0.0,
                                    None,
                                    WhiteSpace::Normal,
                                    false,
                                    None,
                                    None,
                                );
                                scratch.blit_text(&r, 0, r.top.round() as i64);
                                scratch.pop_clip();
                            }
                        }
                    }
                    out.blit_rgba_affine(&scratch.data, w, h, rect.x as f64, rect.y as f64);
                } else {
                    if rect.y + rect.height <= dy || rect.y >= dy + out.height as f32 {
                        continue;
                    }
                    let (x, y) = ((rect.x - dx).round() as i64, (rect.y - dy).round() as i64);
                    let (w, h) = (rect.width.round() as i64, rect.height.round() as i64);
                    if let Some(widget) = widget {
                        if w > 0 && h > 0 {
                            paint_form_widget(out, x, y, w, h, *widget, fonts, *alpha);
                        }
                    } else if let Some(form) = form {
                        paint_form_control(
                            out,
                            x,
                            y,
                            w,
                            h,
                            alt.as_ref(),
                            *form,
                            *fill_placeholder,
                            fonts,
                            *alpha,
                            *caret,
                            *form_rtl,
                        );
                    } else {
                        if *fill_placeholder && w > 0 && h > 0 {
                            out.fill_rect(x, y, w, h, alpha_color([224, 224, 224, 255], *alpha));
                        }
                        if let Some((text, font_size, bold, line_height, color)) = alt {
                            if !text.trim().is_empty() {
                                // The alt run wraps at the box width; the tile's
                                // `top` offsets ink above the box top exactly like
                                // any other text tile (cramped-CJK leading). The
                                // ink clips to the box (batch 6e): a broken-image
                                // alt that wraps past the bottom is cut there,
                                // like every browser.
                                out.push_clip(x, y, x + w, y + h);
                                let r = fonts.rasterize_wrapped(
                                    text,
                                    *font_size,
                                    *bold,
                                    alpha_color(*color, *alpha),
                                    w.max(0) as f32,
                                    *line_height,
                                    false,
                                    0.0,
                                    None,
                                    WhiteSpace::Normal,
                                    false,
                                    None,
                                    None,
                                );
                                out.blit_text(&r, x, (y as f32 + r.top).round() as i64);
                                out.pop_clip();
                            }
                        }
                    }
                }
            }
            PaintItem::Svg {
                rect,
                render,
                alpha,
            } => {
                if out.xf().is_some() {
                    let (w, h) = (rect.width.round() as i64, rect.height.round() as i64);
                    if w <= 0 || h <= 0 {
                        continue;
                    }
                    let (bx0, by0, bx1, by1) = out.mapped_xf_bounds(
                        rect.x as f64,
                        rect.y as f64,
                        rect.width as f64,
                        rect.height as f64,
                    );
                    if bx1 <= 0 || by1 <= 0 || bx0 >= out.width as i64 || by0 >= out.height as i64 {
                        continue;
                    }
                    // Rasterize the subtree into a transparent LOCAL scratch
                    // (dx/dy = the local origin lands it at (0, 0)), then
                    // blit through the bracket.
                    let (w, h) = (w as usize, h as usize);
                    let mut scratch = Canvas::new_transparent(w, h);
                    crate::svg::paint_svg(
                        render,
                        rect,
                        fonts,
                        &mut scratch,
                        rect.x,
                        rect.y,
                        *alpha,
                    );
                    out.blit_rgba_affine(&scratch.data, w, h, rect.x as f64, rect.y as f64);
                } else {
                    // Same band prefilter as Replaced: the svg painter clips to
                    // the element box anyway, this just skips rasterizing an
                    // off-band subtree.
                    if rect.y + rect.height <= dy || rect.y >= dy + out.height as f32 {
                        continue;
                    }
                    crate::svg::paint_svg(render, rect, fonts, out, dx, dy, *alpha);
                }
            }
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
                // background-clip: text: the fill color is ignored entirely
                // (CSS paints the background through the glyphs; the
                // transparent-text-fill half of the idiom is free by
                // construction) — rasterize an opaque-white mask and rewrite
                // each covered pixel with the gradient sampled at its own
                // position. Both blit paths stay untouched: the band path's
                // page coords and the bracket's local coords are exactly the
                // space `area` was captured in, so the gradient rides the
                // affine through rotations too.
                let fill = if gradient.is_some() {
                    [255, 255, 255, 255]
                } else {
                    *color
                };
                // Shadow extent (blur feather + offset) for prefilter
                // widening; shadows ride under the glyphs, first-declared
                // layer on top (blitz#271 family).
                let shadow_pad = text_shadow.as_ref().and_then(|s| {
                    s.iter()
                        .map(|sh| sh.blur.ceil().max(sh.dx.abs()).max(sh.dy.abs()) + 1.0)
                        .fold(None::<f32>, |acc, v| Some(acc.map_or(v, |m| m.max(v))))
                });
                if out.xf().is_some() {
                    // Rasterize at RAW local metrics — the bracket maps the
                    // tile, so no scale folds into font metrics — prefiltered
                    // by the mapped bbox of the same estimated tile box the
                    // band check below uses, widened by the shadow extent so
                    // an offset/blur reaching the canvas isn't skipped.
                    let pad = shadow_pad.unwrap_or(0.0) as i64;
                    let est_w = est_width(text, *font_size).max(*wrap_at);
                    let lines = (est_width(text, *font_size) / wrap_at.max(1.0))
                        .ceil()
                        .max(1.0);
                    let (bx0, by0, bx1, by1) = out.mapped_xf_bounds(
                        *x as f64,
                        (*y - *line_height) as f64,
                        est_w as f64,
                        ((lines + 1.0) * *line_height) as f64,
                    );
                    if bx1 + pad <= 0
                        || by1 + pad <= 0
                        || bx0 - pad >= out.width as i64
                        || by0 - pad >= out.height as i64
                    {
                        continue;
                    }
                    if let Some(shadows) = text_shadow {
                        for sh in shadows.iter().rev() {
                            stamp_text_shadow(
                                out,
                                fonts,
                                text,
                                *font_size,
                                *bold,
                                *wrap_at,
                                *line_height,
                                *mono,
                                *word_spacing,
                                *truncate_at,
                                *ws,
                                *small_caps,
                                *han,
                                tokens.as_deref(),
                                sh,
                                true,
                                (*x + sh.dx) as f64,
                                (*y + sh.dy) as f64,
                                *last_line_align,
                            );
                            // Chrome shadows the decorations with the text
                            // (blitz#984): each hard layer restamps the
                            // strokes in the shadow color at the same offset.
                            // Blurred layers skip the stroke copy — a hard
                            // line beside a feathered glyph reads as a bug,
                            // not a shadow.
                            if sh.blur <= 0.0 {
                                paint_text_decorations(
                                    out,
                                    fonts,
                                    text,
                                    *font_size,
                                    *bold,
                                    [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
                                    *line_height,
                                    *x + sh.dx,
                                    *y + sh.dy,
                                    *wrap_at,
                                    *decorations,
                                    None,
                                    *mono,
                                    *word_spacing,
                                    *truncate_at,
                                    *ws,
                                    *small_caps,
                                    *han,
                                    tokens.as_deref(),
                                    0.0,
                                    0.0,
                                    *last_line_align,
                                );
                            }
                        }
                    }
                    let r = fonts.rasterize_wrapped_with(
                        text,
                        *font_size,
                        *bold,
                        fill,
                        *wrap_at,
                        *line_height,
                        *mono,
                        *word_spacing,
                        *truncate_at,
                        *ws,
                        tokens.clone(),
                        *small_caps,
                        *han,
                        *last_line_align,
                    );
                    // Gradient recolor rewrites pixels in place — the cache
                    // hands out Arcs, so that path clones first (#399).
                    let mut owned;
                    let r = if let Some(g) = gradient {
                        owned = (*r).clone();
                        recolor_gradient_text(&mut owned, *x, *y, *line_height, g);
                        &owned
                    } else {
                        &r
                    };
                    out.blit_rgba_affine(
                        &r.data,
                        r.width,
                        r.height,
                        *x as f64,
                        (*y + r.top) as f64,
                    );
                    paint_text_decorations(
                        out,
                        fonts,
                        text,
                        *font_size,
                        *bold,
                        *color,
                        *line_height,
                        *x,
                        *y,
                        *wrap_at,
                        *decorations,
                        decorations.pierce.then_some(gradient.as_ref()).flatten(),
                        *mono,
                        *word_spacing,
                        *truncate_at,
                        *ws,
                        *small_caps,
                        *han,
                        tokens.as_deref(),
                        0.0,
                        0.0,
                        *last_line_align,
                    );
                } else {
                    let pad = shadow_pad.unwrap_or(0.0);
                    if !text_reaches_band(
                        *y,
                        text,
                        *font_size,
                        *wrap_at,
                        *line_height,
                        dy,
                        out.height as i64,
                        pad,
                    ) {
                        continue;
                    }
                    if let Some(shadows) = text_shadow {
                        for sh in shadows.iter().rev() {
                            stamp_text_shadow(
                                out,
                                fonts,
                                text,
                                *font_size,
                                *bold,
                                *wrap_at,
                                *line_height,
                                *mono,
                                *word_spacing,
                                *truncate_at,
                                *ws,
                                *small_caps,
                                *han,
                                tokens.as_deref(),
                                sh,
                                false,
                                (*x + sh.dx - dx) as f64,
                                (*y + sh.dy - dy) as f64,
                                *last_line_align,
                            );
                            if sh.blur <= 0.0 {
                                paint_text_decorations(
                                    out,
                                    fonts,
                                    text,
                                    *font_size,
                                    *bold,
                                    [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
                                    *line_height,
                                    *x + sh.dx,
                                    *y + sh.dy,
                                    *wrap_at,
                                    *decorations,
                                    None,
                                    *mono,
                                    *word_spacing,
                                    *truncate_at,
                                    *ws,
                                    *small_caps,
                                    *han,
                                    tokens.as_deref(),
                                    dx,
                                    dy,
                                    *last_line_align,
                                );
                            }
                        }
                    }
                    let r = fonts.rasterize_wrapped_with(
                        text,
                        *font_size,
                        *bold,
                        fill,
                        *wrap_at,
                        *line_height,
                        *mono,
                        *word_spacing,
                        *truncate_at,
                        *ws,
                        tokens.clone(),
                        *small_caps,
                        *han,
                        *last_line_align,
                    );
                    let mut owned;
                    let r = if let Some(g) = gradient {
                        owned = (*r).clone();
                        recolor_gradient_text(&mut owned, *x, *y, *line_height, g);
                        &owned
                    } else {
                        &r
                    };
                    // Tile row 0 sits `top` px above the leaf's line-box top.
                    out.blit_text(r, (x - dx).round() as i64, (y - dy + r.top).round() as i64);
                    paint_text_decorations(
                        out,
                        fonts,
                        text,
                        *font_size,
                        *bold,
                        *color,
                        *line_height,
                        *x,
                        *y,
                        *wrap_at,
                        *decorations,
                        decorations.pierce.then_some(gradient.as_ref()).flatten(),
                        *mono,
                        *word_spacing,
                        *truncate_at,
                        *ws,
                        *small_caps,
                        *han,
                        tokens.as_deref(),
                        dx,
                        dy,
                        *last_line_align,
                    );
                }
            }
        }
    }
}

/// Effective text-shadow blur ceiling. Past this a shadow is visually
/// indistinguishable wash — the clamp bounds the scratch tile without
/// costing any real rendering (#76).
pub(super) const MAX_SHADOW_BLUR: f32 = 256.0;
/// Padded-tile ceiling in pixels (~16 MP: a 4000×4000 tile; an 8000-px
/// text line at the clamped 256-px radius still fits). Alpha plane + RGBA
/// copy at this size stay under ~80 MB; anything bigger paints the
/// unfeathered shadow instead of allocating.
const MAX_SHADOW_SCRATCH_PX: usize = 16 * 1024 * 1024;

/// One text-shadow layer stamped UNDER the glyphs (blitz#271 family):
/// re-rasterize the run in the layer color (the RasterCache keys on color,
/// so shadow layers memo independently), box-blur its alpha when the layer
/// asks for a blur, and blit at the offset. `affine` selects the
/// transformed blit (page space; the open bracket folds dx/dy) vs the
/// band-space rounded blit.
///
/// The blur radius feeds the padded scratch tile's size straight from CSS —
/// `text-shadow: 0 0 100000px` on a short run asks for tens of GB (#76,
/// obscura #1059 family). Two guards below: clamp the effective radius
/// (past 256 px a shadow is visually indistinguishable wash) and cap the
/// padded tile at a scratch budget; an oversized tile paints the
/// unfeathered shadow instead of allocating. Small-blur rendering is
/// byte-identical to before (blur well under budget ⇒ same path).
#[allow(clippy::too_many_arguments)]
fn stamp_text_shadow(
    out: &mut Canvas,
    fonts: &FontBook,
    text: &str,
    font_size: f32,
    bold: bool,
    wrap_at: f32,
    line_height: f32,
    mono: bool,
    word_spacing: f32,
    truncate_at: Option<f32>,
    ws: WhiteSpace,
    small_caps: bool,
    han: Option<HanSlot>,
    tokens: Option<&[Token]>,
    sh: &TextShadow,
    affine: bool,
    ox: f64,
    oy: f64,
    last_line_align: Option<diting_css::TextAlign>,
) {
    let r = fonts.rasterize_wrapped_with(
        text,
        font_size,
        bold,
        [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
        wrap_at,
        line_height,
        mono,
        word_spacing,
        truncate_at,
        ws,
        tokens.map(std::rc::Rc::from),
        small_caps,
        han,
        last_line_align,
    );
    if sh.blur <= 0.0 {
        if affine {
            out.blit_rgba_affine(&r.data, r.width, r.height, ox, oy + r.top as f64);
        } else {
            out.blit_text(&r, ox.round() as i64, (oy + r.top as f64).round() as i64);
        }
        return;
    }
    // Soft layer: blur the raster's ALPHA channel in place of a renderer
    // blur filter (upstream blitz#271 blocks blur on renderer support; the
    // per-pixel path here just does it). The padded buffer gives the
    // feather room so the run tile's edge can't clip it.
    let blur = sh.blur.min(MAX_SHADOW_BLUR);
    let pad = blur.ceil() as usize;
    let radius = ((blur * 0.5).round() as usize).max(1);
    let pw = r.width.saturating_add(pad).saturating_add(pad);
    let ph = r.height.saturating_add(pad).saturating_add(pad);
    if pw
        .checked_mul(ph)
        .is_none_or(|px| px > MAX_SHADOW_SCRATCH_PX)
    {
        // Even clamped, the tile overshoots the scratch budget (enormous
        // run + full-radius blur): paint the unfeathered shadow — same
        // shape the `blur <= 0` path draws — instead of a giant allocation.
        if affine {
            out.blit_rgba_affine(&r.data, r.width, r.height, ox, oy + r.top as f64);
        } else {
            out.blit_text(&r, ox.round() as i64, (oy + r.top as f64).round() as i64);
        }
        return;
    }
    let mut alpha = vec![0u8; pw * ph];
    for (row, src_row) in r.data.chunks_exact(r.width * 4).enumerate() {
        for (col, p) in src_row.as_chunks::<4>().0.iter().enumerate() {
            alpha[(row + pad) * pw + col + pad] = p[3];
        }
    }
    for round in 0..2 {
        alpha = box_blur_alpha(&alpha, pw, ph, radius, round == 0);
    }
    let mut data = Vec::with_capacity(pw * ph * 4);
    for a in &alpha {
        data.extend_from_slice(&[sh.color.0, sh.color.1, sh.color.2, *a]);
    }
    let blurred = TextRaster {
        width: pw,
        height: ph,
        baseline: 0.0,
        top: 0.0,
        data,
    };
    // Row 0 of the padded tile is `pad` px above the run tile's row 0,
    // which itself sits `r.top` px above the line box top.
    if affine {
        out.blit_rgba_affine(
            &blurred.data,
            blurred.width,
            blurred.height,
            ox - pad as f64,
            oy + r.top as f64 - pad as f64,
        );
    } else {
        out.blit_text(
            &blurred,
            (ox - pad as f64).round() as i64,
            (oy + r.top as f64 - pad as f64).round() as i64,
        );
    }
}

/// One separable box-blur pass (H or V) over an alpha plane with
/// edge-clamped windows. Two rounds ≈ a triangular kernel — the same
/// extent convention as the box-shadow linear feather (support ≈ blur).
pub(super) fn box_blur_alpha(
    src: &[u8],
    w: usize,
    h: usize,
    r: usize,
    horizontal: bool,
) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    if horizontal {
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let lo = x.saturating_sub(r);
                let hi = (x + r + 1).min(w);
                let mut sum = 0u32;
                for k in lo..hi {
                    sum += src[row + k] as u32;
                }
                out[row + x] = (sum / (hi - lo) as u32) as u8;
            }
        }
    } else {
        for x in 0..w {
            for y in 0..h {
                let lo = y.saturating_sub(r);
                let hi = (y + r + 1).min(h);
                let mut sum = 0u32;
                for k in lo..hi {
                    sum += src[k * w + x] as u32;
                }
                out[y * w + x] = (sum / (hi - lo) as u32) as u8;
            }
        }
    }
    out
}
