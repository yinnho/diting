//! The box-paint passes of `element_box` (background, border, replaced
//! content) — split from the module root (god-file ratchet).

use super::*;

/// Background pass of `element_box` (moved out verbatim): color, layered
/// images/gradients, `background-clip: text` (which hands its gradient to
/// the descendant text through `text_gradient`).
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_background(
    cx: &Ctx,
    out: &mut Out,
    dom_id: &NodeId,
    alpha: f32,
    bg: Option<diting_css::Color>,
    bg_rect: Rect,
    child_xf: Xf,
    radii: [(f32, f32); 4],
    rect: Rect,
    text_gradient: &mut Option<(TextGradient, [f32; 6])>,
) {
    let Ctx { styles, .. } = *cx;
    if alpha > 0.0 && rect.width > 0.0 && rect.height > 0.0 {
        // backdrop-filter filters everything beneath this element,
        // so it must emit BEFORE any of the element's own ink
        // (shadows, background, border) — first item in the block.
        if let Some(blur) = styles
            .get(dom_id)
            .and_then(|s| s.backdrop_blur)
            .filter(|b| *b > 0.0)
        {
            out.items.push(PaintItem::BackdropFilter {
                rect: bg_rect,
                radii,
                blur,
            });
        }
        // box-shadow below everything: CSS stacks shadows under the
        // background, first-declared layer on top, so emit reversed.
        if let Some(shadows) = styles.get(dom_id).and_then(|s| s.box_shadow.as_ref()) {
            for sh in shadows.iter().rev().filter(|sh| !sh.inset) {
                out.items.push(PaintItem::BoxShadow {
                    rect: bg_rect,
                    color: with_alpha([sh.color.0, sh.color.1, sh.color.2, sh.color.3], alpha),
                    radii,
                    dx: sh.dx,
                    dy: sh.dy,
                    blur: sh.blur,
                    spread: sh.spread,
                    inset: false,
                });
            }
        }
        // background-color: the bottom CSS layer.
        if let Some(c) = bg.filter(|c| c.3 != 0) {
            let color = with_alpha([c.0, c.1, c.2, c.3], alpha);
            let uniform = radii.iter().all(|r| *r == radii[0]);
            if uniform {
                out.items.push(PaintItem::Bg {
                    rect: bg_rect,
                    color,
                    radius: radii[0].0,
                });
            } else {
                out.items.push(PaintItem::BgCorner {
                    rect: bg_rect,
                    color,
                    radii,
                });
            }
        }
        // background-image above it: v1 takes a parseable
        // linear-gradient for the whole box (url() images stay
        // unpainted — they are the Image item's job, not a fill).
        if let Some(g) = styles
            .get(dom_id)
            .and_then(|s| s.background_image.as_deref())
            .and_then(diting_css::parse_linear_gradient)
        {
            if styles.get(dom_id).is_some_and(|s| s.background_clip_text) {
                // background-clip: text: the gradient never fills
                // the box (the opaque-block-covering-the-text bug) —
                // it becomes the subtree's glyph fill, threaded down
                // like alpha; the xf array guards the space (see the
                // attach filter below).
                *text_gradient = Some((
                    TextGradient {
                        area: bg_rect,
                        stops: g
                            .stops
                            .iter()
                            .map(|(p, c)| (*p, [c.0, c.1, c.2, c.3]))
                            .collect(),
                        css_deg: g.css_deg,
                        clone_per_line: styles.get(dom_id).is_some_and(|s| s.box_decoration_clone),
                        clone_lines: Vec::new(),
                    },
                    child_xf.to_array(),
                ));
            } else {
                let stops = g
                    .stops
                    .iter()
                    .map(|(p, c)| (*p, with_alpha([c.0, c.1, c.2, c.3], alpha)))
                    .collect();
                out.items.push(PaintItem::BgGradient {
                    rect: bg_rect,
                    stops,
                    css_deg: g.css_deg,
                    radii,
                });
            }
        }
        // inset layers above the background (and gradient) but
        // under the border, same reversed first-on-top order —
        // Chrome's inner-shadow phase sits between the two.
        if let Some(shadows) = styles.get(dom_id).and_then(|s| s.box_shadow.as_ref()) {
            for sh in shadows.iter().rev().filter(|sh| sh.inset) {
                out.items.push(PaintItem::BoxShadow {
                    rect: bg_rect,
                    color: with_alpha([sh.color.0, sh.color.1, sh.color.2, sh.color.3], alpha),
                    radii,
                    dx: sh.dx,
                    dy: sh.dy,
                    blur: sh.blur,
                    spread: sh.spread,
                    inset: true,
                });
            }
        }
    }
}

/// Border pass of `element_box` (moved out verbatim): per-side widths,
/// styles and colors, rounded by the box radii.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_border(
    cx: &Ctx,
    out: &mut Out,
    dom_id: &NodeId,
    alpha: f32,
    bg_rect: Rect,
    child_xf: Xf,
    prebake: bool,
    radii: [(f32, f32); 4],
    st: Option<&ComputedStyle>,
) {
    let Ctx {
        collapsed_edges, ..
    } = *cx;
    if let Some(style) = st.filter(|s| s.border_style.is_some()) {
        // Top/bottom are horizontal strips (vertical thickness rides
        // d), left/right vertical strips (a) — the affine maps each
        // edge's thickness with its own axis; a bracket keeps the
        // widths local.
        let (kx, ky) = if prebake {
            (child_xf.a, child_xf.d)
        } else {
            (1.0, 1.0)
        };
        // Collapsed table cell borders (spans batch): an edge on a
        // shared grid line paints at HALF width so two adjacent 2px
        // borders read as one 2px line centered on the line —
        // Chrome's resolved-border rendering for equal widths.
        // Index-based marks from build_table (top/right/bottom/left).
        let halve = collapsed_edges.get(dom_id);
        let mut widths = [
            side_px(style.border_width.top) * ky,
            side_px(style.border_width.right) * kx,
            side_px(style.border_width.bottom) * ky,
            side_px(style.border_width.left) * kx,
        ];
        if let Some(h) = halve {
            for (i, w) in widths.iter_mut().enumerate() {
                if h[i] {
                    *w /= 2.0;
                }
            }
        }
        if widths.iter().any(|w| *w > 0.0) {
            let color = style
                .border_color
                .or(style.color)
                .map(|c| with_alpha([c.0, c.1, c.2, c.3], alpha))
                .unwrap_or([0, 0, 0, 255]);
            out.items.push(PaintItem::Border {
                rect: bg_rect,
                widths,
                color,
                radii,
            });
        }
    }
}

/// Replaced-content pass of `element_box` (moved out verbatim): decoded
/// images, canvas/video frames, svg and form-control faces inside the
/// box's content rect.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_replaced(
    cx: &Ctx,
    out: &mut Out,
    dom_id: &NodeId,
    alpha: f32,
    bg_rect: Rect,
    child_xf: Xf,
    prebake: bool,
    rect: Rect,
) {
    let Ctx {
        tree,
        styles,
        fonts,
        images,
        ..
    } = *cx;
    // svg: the subtree compiles to paint ops here — never the
    // image path, never the placeholder (an empty svg still
    // renders its element box, matching Chrome).
    let is_svg = tree
        .with_node(*dom_id, |n| {
            n.as_element().map(|e| e.local.to_string() == "svg")
        })
        .flatten()
        .unwrap_or(false);
    if is_svg {
        let render = std::sync::Arc::new(svg::compile_svg(tree, styles, *dom_id));
        out.items.push(PaintItem::Svg {
            rect: bg_rect,
            render,
            alpha,
        });
    } else if let Some(img) = images.get(dom_id) {
        let st = styles.get(dom_id);
        let fit = st.and_then(|s| s.object_fit).unwrap_or(ObjectFit::Fill);
        let pos = st.and_then(|s| s.object_position).unwrap_or((
            ObjectPositionPart::Percent(50.0),
            ObjectPositionPart::Percent(50.0),
        ));
        // The blit destination per object-fit/position (batch
        // 5c); `rect` stays the element box for callers that
        // want it — mapped through the same affine as mrect on
        // the prebaked path, raw under a bracket.
        let op_rect = object_paint_rect(rect, img.width as f32, img.height as f32, fit, pos);
        let paint_rect = if prebake {
            child_xf.map_rect(op_rect)
        } else {
            op_rect
        };
        out.items.push(PaintItem::Image {
            rect: bg_rect,
            paint_rect,
            image: img.clone(),
            alpha,
        });
    } else {
        // Alt text is an <img> concept only (batch 7a): video/
        // iframe/canvas placeholders are the bare box. Form
        // controls (input/textarea) paint the text they show —
        // the dirty value, parsed default, or gray placeholder.
        let is_img = tree
            .with_node(*dom_id, |n| {
                n.as_element().map(|e| e.local.to_string() == "img")
            })
            .flatten()
            .unwrap_or(false);
        let alt = if is_img {
            tree.with_node(*dom_id, |n| n.get_attribute("alt").map(|v| v.to_string()))
                .flatten()
                .map(|text| {
                    let (font_size, bold, lh) = font_context(tree, *dom_id, styles, fonts);
                    (
                        text,
                        font_size,
                        bold,
                        lh,
                        color_context(tree, *dom_id, styles),
                    )
                })
        } else {
            form_control_run(tree, *dom_id, styles, fonts)
        };
        // The gray box only when the author gave no visible
        // background — an authored bg already reads as "box here".
        let fill_placeholder = styles
            .get(dom_id)
            .and_then(|s| s.background_color)
            .is_none_or(|c| c.3 == 0);
        // Checkable inputs resolve their widget here (dirty
        // mirror beats the parsed attribute — the JS getter's
        // precedence) so the paint executor stays tree-free.
        let widget = tree
            .with_node(*dom_id, |n| {
                let is_input = n
                    .as_element()
                    .map(|e| e.local.as_str() == "input")
                    .unwrap_or(false);
                if !is_input {
                    return None;
                }
                let ty = n
                    .get_attribute("type")
                    .map(|v| v.to_ascii_lowercase())
                    .unwrap_or_default();
                let checked = n
                    .live_checked()
                    .unwrap_or_else(|| n.get_attribute("checked").is_some());
                match ty.as_str() {
                    "checkbox" => Some(FormWidget::Checkbox { checked }),
                    "radio" => Some(FormWidget::Radio { checked }),
                    "range" => {
                        // Fraction of the inset track the thumb
                        // sits at: live value (dirty mirror of a
                        // click) over the value attribute, over
                        // the spec default (min+max)/2. min/max
                        // fall back to 0/100; max<=min pins the
                        // thumb at the start (the spec clamps
                        // value to min).
                        let num = |name: &str, def: f32| {
                            n.get_attribute(name)
                                .and_then(|v| v.parse::<f32>().ok())
                                .unwrap_or(def)
                        };
                        let (min, max) = (num("min", 0.0), num("max", 100.0));
                        let value = n
                            .live_value()
                            .or_else(|| n.get_attribute("value"))
                            .and_then(|v| v.parse::<f32>().ok())
                            .unwrap_or((min + max) / 2.0);
                        let fraction = if max > min {
                            ((value - min) / (max - min)).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        Some(FormWidget::Range { fraction })
                    }
                    _ => None,
                }
            })
            .flatten();
        // The control's run layout kind: same tag/type split the
        // widget arm uses, but for the text-carrying controls —
        // checkables take the widget path instead.
        let form = if widget.is_none() {
            tree.with_node(*dom_id, |n| {
                let local = n
                    .as_element()
                    .map(|e| e.local.to_string())
                    .unwrap_or_default();
                match local.as_str() {
                    "textarea" => Some(FormRun::Textarea),
                    "select" => Some(FormRun::Select),
                    "input" => {
                        let ty = n
                            .get_attribute("type")
                            .map(|v| v.to_ascii_lowercase())
                            .unwrap_or_default();
                        match ty.as_str() {
                            "button" | "submit" | "reset" => Some(FormRun::Button),
                            // checkbox/radio never reach here
                            // (widget took them); every other type
                            // is a text-like single-line control.
                            _ => Some(FormRun::Input),
                        }
                    }
                    _ => None,
                }
            })
            .flatten()
        } else {
            None
        };
        // Caret: the focused text-entry control's typing
        // cursor, at the recorded selection's anchor. Resolved
        // here — collect time — because paint keeps no tree
        // access; same posture as checkedness and the run kind
        // above. min(start,end): a non-collapsed range paints
        // the anchor side (Chrome paints the focus node; the
        // direction mirror is not recorded, so the anchor is
        // the deterministic stand-in).
        let caret = if matches!(form, Some(FormRun::Input) | Some(FormRun::Textarea))
            && tree.focused_node() == Some(*dom_id)
            && tree.selection().is_some_and(|(nid, _, _)| nid == *dom_id)
        {
            tree.selection().map(|(_, start, end)| {
                let live = tree
                    .with_node(*dom_id, |n| n.live_value().map(|v| v.to_string()))
                    .flatten();
                // The value the JS getter would report: dirty
                // mirror first, then the parsed default
                // (attribute for input, child text for
                // textarea).
                let value = if matches!(form, Some(FormRun::Textarea)) {
                    live.unwrap_or_else(|| tree.text_content(*dom_id))
                } else {
                    live.or_else(|| {
                        tree.with_node(*dom_id, |n| n.get_attribute("value").map(|v| v.to_string()))
                            .flatten()
                    })
                    .unwrap_or_default()
                };
                let ink = color_context(tree, *dom_id, styles);
                (start.min(end).min(value.chars().count()), ink)
            })
        } else {
            None
        };
        // An empty value with no placeholder leaves the run
        // None — but a caret still needs the control's font
        // metrics to stand in a line box, so synthesize an
        // empty run for it (the rasterizer blits nothing for
        // empty text; only the caret consumes the metrics).
        let alt = if alt.is_none() && caret.is_some() {
            let (font_size, bold, lh) = font_context(tree, *dom_id, styles, fonts);
            Some((
                String::new(),
                font_size,
                bold,
                lh,
                caret.map(|(_, ink)| ink).unwrap_or_default(),
            ))
        } else {
            alt
        };
        // RTL value anchoring (#188-3, blitz#981): resolved at
        // collect time like the caret (paint keeps no tree
        // access); Select/Button stay physical.
        let form_rtl = matches!(form, Some(FormRun::Input) | Some(FormRun::Textarea))
            && styles
                .get(dom_id)
                .is_some_and(|s| s.direction == Some(TextDirection::Rtl));
        out.items.push(PaintItem::Replaced {
            rect: bg_rect,
            alt,
            fill_placeholder,
            alpha,
            widget,
            form,
            caret,
            form_rtl,
        });
    }
}
