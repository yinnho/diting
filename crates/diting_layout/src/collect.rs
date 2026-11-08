//! The paint-collection walk of `layout_collect_with_images` (moved out
//! of its body verbatim): the `Xf` affine type, z-escape records, the
//! list-marker emitter and the recursive `collect` pre-order walk.

use super::*;

// Accumulate locations down the taffy tree: child location already
// includes the parent's border+padding offset, so a plain sum is the
// absolute border-box origin (same accumulation blitz-paint performs).
// The same pre-order walk emits paint items: an element's Bg when its
// box is recorded, a run's Text at its leaf — document order, parents
// before children.
/// Resolved 2D affine threaded down the collect walk (animation batch B,
/// widened in the affine batch): x' = a·x + c·y + e, y' = b·x + d·y + f,
/// CSS matrix convention. Identity unless some ancestor declared a
/// transform with a part beyond translate — pure translates fold into
/// `offset` and never appear here, so untouched trees take the exact
/// pre-batch path. Diagonal maps pre-bake into item fields at collect
/// time; a non-diagonal map (rotate/skew) flips descendants into the
/// SetXf/ClearXf local-coordinate paint path.
#[derive(Clone, Copy)]
pub(super) struct Xf {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl Xf {
    pub(super) const IDENTITY: Xf = Xf {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Whether the linear part is diagonal (translate/scale lineage
    /// only) — the collect walk pre-bakes geometry on that path.
    fn is_diagonal(&self) -> bool {
        self.b == 0.0 && self.c == 0.0
    }

    /// Compose two maps, `m` outer and `n` inner (matrix product m·n).
    fn compose(m: Xf, n: Xf) -> Xf {
        Xf {
            a: m.a * n.a + m.c * n.b,
            b: m.b * n.a + m.d * n.b,
            c: m.a * n.c + m.c * n.d,
            d: m.b * n.c + m.d * n.d,
            e: m.a * n.e + m.c * n.f + m.e,
            f: m.b * n.e + m.d * n.f + m.f,
        }
    }

    /// Map a rect: the diagonal fast path keeps the historical two-corner
    /// normalize (negative scales mirror); anything else maps all four
    /// corners and unions them — Chrome's gBCR bounding-box behavior
    /// under rotation.
    fn map_rect(&self, r: Rect) -> Rect {
        map_rect_arr(self.to_array(), r)
    }

    fn to_array(self) -> [f32; 6] {
        [self.a, self.b, self.c, self.d, self.e, self.f]
    }
}

/// A positioned z>0 child deferred past a non-context parent: emitted
/// at the nearest stacking-context ancestor instead (cross-parent z,
/// the narrow slice). Carries the deferring frame's walk env — geometry
/// is recursion-order-free, so the subtree paints pixel-identical at
/// its z slot; only the sequence in the flat item list moves. Lives
/// beside `Xf` because that map type is fn-scoped.
pub(super) struct ZEscape {
    z: i32,
    child: diting_taffy::tree::NodeId,
    offset: (f32, f32),
    xf: Xf,
    alpha: f32,
    gradient: Option<(TextGradient, [f32; 6])>,
}

/// Multiply a straight-alpha color's alpha channel (animation batch A):
/// the paint pipeline is straight-alpha source-over throughout, so
/// folding element opacity into item colors composites the subtree.
pub(super) fn with_alpha(c: [u8; 4], a: f32) -> [u8; 4] {
    if a >= 1.0 {
        c
    } else {
        [c[0], c[1], c[2], (c[3] as f32 * a).round() as u8]
    }
}

/// #206 (blitz #1003 absorption, marker v1): an `li` paints its list
/// marker OUTSIDE the border box, hanging off the outer left edge (the
/// outer right edge under rtl) and sharing the first text line's
/// typography so the baselines align. Layout never sees the marker —
/// Chrome's outside marker box takes no space in the principal box, and
/// the ul's UA padding is what visually hosts the bullets. The index is
/// the li's position among same-parent li siblings (css-lists item
/// counter); the typography comes from the first Run/Word leaf in the
/// laid-out subtree. Called only on the prebaked (diagonal) path — a
/// transformed li under a SetXf bracket skips its marker.
#[allow(clippy::too_many_arguments)]
pub(super) fn push_list_marker(
    tree: &DomTree,
    taffy_tree: &TaffyTree<TextLeaf>,
    node: diting_taffy::tree::NodeId,
    dom_id: NodeId,
    cs: &diting_css::ComputedStyle,
    rect: Rect,
    fonts: &FontBook,
    alpha: f32,
    xf: Xf,
    items: &mut Vec<PaintItem>,
) {
    let is_li = tree
        .with_node(dom_id, |n| {
            n.as_element().is_some_and(|e| &*e.local == "li")
        })
        .unwrap_or(false);
    if !is_li {
        return;
    }
    // Author display changes de-list the item (display:flex/inline li
    // generates no marker in Chrome); the UA Block keeps one.
    if !matches!(cs.display, Some(CssDisplay::Block)) {
        return;
    }
    let ty = cs
        .list_style_type
        .unwrap_or(diting_css::ListStyleType::Disc);
    if ty == diting_css::ListStyleType::None {
        return;
    }
    let mut index = 1usize;
    let mut sib = tree.with_node(dom_id, |n| n.prev_sibling).flatten();
    while let Some(s) = sib {
        if tree
            .with_node(s, |n| n.as_element().is_some_and(|e| &*e.local == "li"))
            .unwrap_or(false)
        {
            index += 1;
        }
        sib = tree.with_node(s, |n| n.prev_sibling).flatten();
    }
    let text = ty.marker_text(index);
    // First text leaf in the laid-out subtree: its line-box top and
    // font params carry the marker (baseline alignment for free). An
    // li with no text falls back to the content box's first line.
    let mut fs = cs.font_size.unwrap_or(diting_css::DEFAULT_ROOT_FONT_SIZE);
    let mut lh = fs * 1.2;
    let mut bold = cs.font_weight.unwrap_or(400) >= 600;
    let mut mono = false;
    let mut han = None;
    let mut first_line_y = rect.y + side_px(cs.border_width.top) + side_px(cs.padding.top);
    let mut stack: Vec<(diting_taffy::tree::NodeId, f32, f32)> = vec![(node, 0.0, 0.0)];
    while let Some((n, ox, oy)) = stack.pop() {
        let kids = taffy_tree.children(n).unwrap_or_default();
        for &c in kids.iter().rev() {
            let Ok(l) = taffy_tree.layout(c) else {
                continue;
            };
            let (cx, cy) = (ox + l.location.x, oy + l.location.y);
            let leaf = match taffy_tree.get_node_context(c) {
                Some(leaf @ TextLeaf::Run { .. }) | Some(leaf @ TextLeaf::Word { .. }) => {
                    Some(leaf)
                }
                _ => None,
            };
            if let Some(leaf) = leaf {
                match leaf {
                    TextLeaf::Run {
                        font_size,
                        line_height,
                        bold: b,
                        mono: m,
                        han: h,
                        ..
                    }
                    | TextLeaf::Word {
                        font_size,
                        line_height,
                        bold: b,
                        mono: m,
                        han: h,
                        ..
                    } => {
                        fs = *font_size;
                        lh = *line_height;
                        bold = *b;
                        mono = *m;
                        han = *h;
                    }
                    _ => unreachable!(),
                }
                first_line_y = rect.y + cy;
                stack.clear();
                break;
            }
            stack.push((c, cx, cy));
        }
    }
    if text.is_empty() || fs <= 0.0 {
        return;
    }
    let advance = fonts.advance_width(&text, fs, bold, mono, han);
    let gap = fs * 0.25;
    let rtl = matches!(cs.direction, Some(diting_css::TextDirection::Rtl));
    let x = if rtl {
        rect.x + rect.width + gap
    } else {
        rect.x - advance - gap
    };
    let color = cs
        .color
        .map(|c| [c.0, c.1, c.2, c.3])
        .unwrap_or([0, 0, 0, 255]);
    items.push(PaintItem::Text {
        text,
        font_size: fs * xf.d,
        bold,
        color: with_alpha(color, alpha),
        line_height: lh * xf.d,
        x: x * xf.a + xf.e,
        y: first_line_y * xf.d + xf.f,
        wrap_at: f32::INFINITY,
        gradient: None,
        decorations: diting_css::TextDecorations::default(),
        mono,
        word_spacing: 0.0,
        truncate_at: None,
        tokens: None,
        ws: WhiteSpace::Normal,
        text_shadow: None,
        small_caps: false,
        han,
        // A marker is a single line anchored at its own x — alignment
        // inside the tile never applies.
        last_line_align: None,
        src: None,
    });
}

/// Per-frame state the element-box pass opens and `collect`'s tail closes
/// (clip / transform / blend brackets, the scroller span, the map and
/// text gradient handed to descendants).
pub(super) struct BoxState {
    clips: bool,
    xf_bracket: bool,
    blend_bracket: bool,
    scroll_span: Option<(NodeId, usize)>,
    child_xf: Xf,
    text_gradient: Option<(TextGradient, [f32; 6])>,
}

/// Background pass of `element_box` (moved out verbatim): color, layered
/// images/gradients, `background-clip: text` (which hands its gradient to
/// the descendant text through `text_gradient`).
#[allow(clippy::too_many_arguments)]
fn paint_background(
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
fn paint_border(
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
fn paint_replaced(
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

/// The element-box pass of one `collect` frame (moved out of its body
/// verbatim): backgrounds, borders, replaced content, clip/transform/blend
/// brackets and scroller spans for the boxed DOM element `dom_id`.
#[allow(clippy::too_many_arguments)]
fn element_box(
    cx: &Ctx,
    out: &mut Out,
    dom_id: &NodeId,
    node: diting_taffy::tree::NodeId,
    layout: &diting_taffy::tree::Layout,
    abs: (f32, f32),
    alpha: f32,
    strut_pad: f32,
    t_lin: (f32, f32, f32, f32),
    xf: Xf,
    st: BoxState,
) -> BoxState {
    let Ctx {
        tree,
        taffy_tree,
        styles,
        fonts,
        static_pos,
        ..
    } = *cx;
    // Every path through the pass decides `clips` before reading it.
    let mut clips;
    let BoxState {
        clips: _,
        mut xf_bracket,
        mut blend_bracket,
        mut scroll_span,
        mut child_xf,
        mut text_gradient,
    } = st;
    let mut rect = Rect {
        x: abs.0,
        y: abs.1,
        width: layout.size.width,
        height: layout.size.height - strut_pad,
    };
    // Static-position override (the harvest pass above): a
    // both-auto axis of an out-of-flow box takes its ORIGINAL flow
    // coordinate, not the post-reparent CB flow tail taffy fell
    // back to. Sizes stay taffy's (shrink-to-fit against the CB is
    // correct per CSS).
    if let Some((sx, sy)) = static_pos.get(dom_id) {
        let s = styles.get(dom_id);
        let x_auto = s.is_none_or(|s| s.left.is_none() && s.right.is_none());
        let y_auto = s.is_none_or(|s| s.top.is_none() && s.bottom.is_none());
        if x_auto {
            rect.x = *sx;
        }
        if y_auto {
            rect.y = *sy;
        }
    }
    // The transform's linear part pivots on the element's own
    // (already translate-folded) box: the center (50% 50%) by
    // default, or the declared transform-origin (#244) — keywords
    // normalized to percentages at parse, percentages resolving
    // against the element's own border box here. Pivoting the
    // accumulated function list there equals CSS transform-origin
    // composition (the translate rode the same absolute axes the
    // offset fold used, so both list orders come out exact).
    // Ancestors wrap outside via compose. Diagonal linear parts
    // keep pre-baking geometry into item fields (the historical
    // path); a rotate/skew flips this element's whole item range
    // into local coordinates bracketed by SetXf/ClearXf, and paint
    // inverse-maps per pixel.
    // mix-blend-mode (#222): a non-normal mode opens the blend
    // bracket BEFORE the element's own first item — background,
    // pseudo boxes, text runs and children all paint into the
    // group, exactly the slice Chrome composites. A blending
    // element is a stacking context (establishes_stacking_context
    // above), so z-band hoisting can never tear the pair.
    if let Some(mode) = styles
        .get(dom_id)
        .and_then(|s| s.mix_blend_mode)
        .filter(|m| *m != diting_css::MixBlendMode::Normal)
    {
        out.items.push(PaintItem::BlendBegin { mode });
        blend_bracket = true;
    }
    let prebake = if t_lin != (1.0f32, 0.0, 0.0, 1.0) {
        let origin = |l: diting_css::Length, basis: f32| match l {
            diting_css::Length::Px(v) => v,
            diting_css::Length::Percent(p) => p / 100.0 * basis,
            diting_css::Length::Calc { percent, .. } => percent / 100.0 * basis,
            _ => 0.0,
        };
        let (ox, oy) = match styles.get(dom_id).and_then(|s| s.transform_origin) {
            Some((lx, ly)) => (origin(lx, rect.width), origin(ly, rect.height)),
            None => (rect.width / 2.0, rect.height / 2.0),
        };
        let (rcx, rcy) = (rect.x + ox, rect.y + oy);
        let own = Xf {
            a: t_lin.0,
            b: t_lin.1,
            c: t_lin.2,
            d: t_lin.3,
            e: rcx - (t_lin.0 * rcx + t_lin.2 * rcy),
            f: rcy - (t_lin.1 * rcx + t_lin.3 * rcy),
        };
        child_xf = Xf::compose(xf, own);
        if !xf.is_diagonal() {
            // Inside an open bracket the canvas top already equals
            // xf, so only the own map composes on top of it.
            out.items.push(PaintItem::SetXf { xf: own.to_array() });
            xf_bracket = true;
            false
        } else if !child_xf.is_diagonal() {
            // First non-diagonal map on a prebaked chain: the canvas
            // top is identity here, so the bracket carries the FULL
            // accumulated map.
            out.items.push(PaintItem::SetXf {
                xf: child_xf.to_array(),
            });
            xf_bracket = true;
            false
        } else {
            true
        }
    } else {
        xf.is_diagonal()
    };
    // gBCR/hit-testing rect: the TRANSFORMED box (Chrome reports the
    // mapped bounds — a 4-corner bounding box under rotation;
    // translate is already in `rect` via the offset fold).
    let mrect = child_xf.map_rect(rect);
    // #243: the JS-facing geometry maps keep taffy's UNROUNDED size —
    // Chrome's gBCR serves subpixel border boxes (height:50.6px +
    // 2px border reads 52.6, not 53). Paint below keeps the rounded
    // box (whole-pixel text/bands are load-bearing), and the IDL
    // integer accessors (offset*/client*/scroll*) round in JS
    // (_ditingExtent's Math.round) — only the DOMRect family wants
    // the fractional truth. Origins stay the walk's rounded
    // positions: taffy's rounder pins edges to the integer grid, and
    // authored origins are integral outside zoom/DPR pages.
    let raw_rect = {
        let ul = taffy_tree.unrounded_layout(node);
        Rect {
            x: rect.x,
            y: rect.y,
            width: ul.size.width,
            height: ul.size.height - strut_pad,
        }
    };
    out.rects.insert(*dom_id, child_xf.map_rect(raw_rect));
    // The un-mapped twin plus the total map (event-coordinate
    // surface, blitz #663 family): offsetX/Y inverse-maps the hit
    // point through the map and subtracts this box's padding edge.
    out.local_geom
        .insert(*dom_id, (raw_rect, child_xf.to_array()));
    // Items under a SetXf bracket paint in LOCAL coordinates (raw
    // `rect`); the prebaked path uses the mapped box.
    let bg_rect = if prebake { mrect } else { rect };
    // Outside list marker (#206): laid out with the box, painted
    // hanging off the border box's outer edge.
    if prebake {
        if let Some(cs) = styles.get(dom_id) {
            push_list_marker(
                tree, taffy_tree, node, *dom_id, cs, rect, fonts, alpha, child_xf, out.items,
            );
        }
    }
    // CSS2.1 paints cell > row > row-group backgrounds. Row groups
    // have their own wrapper boxes since the sticky-group batch, so
    // the group's bg paints on its own band (under the rows, which
    // are its taffy children) — the old paint-time DOM climb that
    // faked a group band onto the row is gone with the boxless
    // group. (If the group wrapper fails to build, the group's bg
    // is dropped — error fallback only.)
    let bg = styles.get(dom_id).and_then(|s| s.background_color);
    // Radii resolve per-axis against the LOCAL box (rx width, ry
    // height — the elliptical form), then ride the affine. Hoisted
    // above the fills so the color and gradient layers share them.
    let res = |l: &diting_css::Length, basis: f32| match l {
        diting_css::Length::Px(v) => *v,
        diting_css::Length::Percent(p) => p * basis / 100.0,
        diting_css::Length::Calc { percent, .. } => percent * basis / 100.0,
        diting_css::Length::Auto
        | diting_css::Length::MinContent
        | diting_css::Length::MaxContent
        | diting_css::Length::FitContent => 0.0,
    };
    let radii: [(f32, f32); 4] = styles
        .get(dom_id)
        .and_then(|s| s.corner_radii.as_ref())
        .map(|corners| {
            let rs = std::array::from_fn(|i| {
                // Radii ride the affine only on the prebaked path
                // (each axis scales by its diagonal entry); under a
                // SetXf bracket they stay local and the affine
                // rasterizer curves them.
                if prebake {
                    (
                        res(&corners[i].0, rect.width) * child_xf.a,
                        res(&corners[i].1, rect.height) * child_xf.d,
                    )
                } else {
                    (
                        res(&corners[i].0, rect.width),
                        res(&corners[i].1, rect.height),
                    )
                }
            });
            clamp_corner_radii(rs, bg_rect.width, bg_rect.height)
        })
        .unwrap_or([(0.0, 0.0); 4]);
    paint_background(
        cx,
        out,
        dom_id,
        alpha,
        bg,
        bg_rect,
        child_xf,
        radii,
        rect,
        &mut text_gradient,
    );
    // A border exists only with a line style; its color defaults to
    // currentColor = the element's own computed (inherited) color.
    let st = styles.get(dom_id);
    paint_border(
        cx, out, dom_id, alpha, bg_rect, child_xf, prebake, radii, st,
    );
    // A replaced box paints either its decoded image (batch 5b,
    // object-fit: fill over the content box — equals the border box
    // until replaced elements model border/padding) or the batch-5a
    // placeholder over its bg/border. The alt run resolves against
    // the img's inherited font/color context here so paint stays
    // style-free.
    let replaced = tree
        .with_node(*dom_id, |n| {
            n.as_element().map(|e| is_replaced_tag(&e.local))
        })
        .flatten()
        .unwrap_or(false);
    if replaced {
        paint_replaced(cx, out, dom_id, alpha, bg_rect, child_xf, prebake, rect);
    }
    // A clipping element constrains its DESCENDANTS' paint (its own
    // bg/border above are not clipped) to the padding box — the
    // border box inset by the border widths. Text runs are taffy
    // children here, so they land inside the clip pair too.
    let style = styles.get(dom_id);
    clips = style.is_some_and(|s| s.clips_descendants());
    if clips {
        // blitz#880 (css-overflow-3 §3.3): html's overflow belongs to
        // the viewport — the canvas bounds are the clip, never an
        // html clip push. A body whose overflow propagated to the
        // viewport has its used value flipped back to `visible` and
        // must not clip either.
        let tag = tree.get_node(*dom_id).and_then(|n| {
            n.as_element()
                .map(|e| e.local.to_ascii_lowercase().as_str().to_string())
        });
        if tag.as_deref() == Some("html")
            || (tag.as_deref() == Some("body")
                && body_overflow_propagates(tree, styles, *dom_id, |id| {
                    out.rects.contains_key(&id)
                }))
        {
            clips = false;
        }
    }
    if clips {
        let st = style.expect("checked above");
        let bline = st.border_style.is_some();
        let (bt, br, bb, bl) = (
            if bline {
                side_px(st.border_width.top)
            } else {
                0.0
            },
            if bline {
                side_px(st.border_width.right)
            } else {
                0.0
            },
            if bline {
                side_px(st.border_width.bottom)
            } else {
                0.0
            },
            if bline {
                side_px(st.border_width.left)
            } else {
                0.0
            },
        );
        // The clip rect is the padding box; the element's own radii
        // ride along (batch 7d) so descendants cut at the curve —
        // upstream clips through the rounded padding_box_path.
        let res = |l: &diting_css::Length, basis: f32| match l {
            diting_css::Length::Px(v) => *v,
            diting_css::Length::Percent(p) => p * basis / 100.0,
            diting_css::Length::Calc { percent, .. } => percent * basis / 100.0,
            diting_css::Length::Auto
            | diting_css::Length::MinContent
            | diting_css::Length::MaxContent
            | diting_css::Length::FitContent => 0.0,
        };
        let pad_w = (rect.width - bl - br).max(0.0);
        let pad_h = (rect.height - bt - bb).max(0.0);
        let radii: Option<[(f32, f32); 4]> = st
            .corner_radii
            .map(|cs| {
                let mut out = [(0.0f32, 0.0f32); 4];
                for (slot, (rx, ry)) in out.iter_mut().zip(cs.iter()) {
                    *slot = (res(rx, rect.width), res(ry, rect.height));
                }
                out
            })
            .or_else(|| {
                // Legacy uniform shortcut only when no per-corner
                // form was parsed at all.
                st.border_radius.map(|r| {
                    let v = match r {
                        diting_css::Length::Px(v) => v,
                        diting_css::Length::Percent(p) => p * rect.width / 100.0,
                        diting_css::Length::Calc { percent, .. } => percent * rect.width / 100.0,
                        diting_css::Length::Auto
                        | diting_css::Length::MinContent
                        | diting_css::Length::MaxContent
                        | diting_css::Length::FitContent => 0.0,
                    };
                    [(v, v); 4]
                })
            })
            .map(|rs| clamp_corner_radii(rs, pad_w, pad_h))
            .map(|rs| {
                // Descendants paint through child_xf; on the prebaked
                // path the clip (and its radii) rides the same
                // affine, under a bracket it stays local.
                if prebake {
                    rs.map(|(rx, ry)| (rx * child_xf.a, ry * child_xf.d))
                } else {
                    rs
                }
            });
        // The clip lives in child space (it bounds descendants) —
        // mapped through child_xf on the prebaked path, raw local
        // geometry under a bracket. The element's own bg/border
        // above are already painted and stay unclipped either way.
        let pad_local = Rect {
            x: rect.x + bl,
            y: rect.y + bt,
            width: pad_w,
            height: pad_h,
        };
        let pad_rect = if prebake {
            child_xf.map_rect(pad_local)
        } else {
            pad_local
        };
        let clip_item = match radii {
            Some(radii) if radii.iter().any(|r| r.0 > 0.0 && r.1 > 0.0) => PaintItem::ClipRounded {
                rect: pad_rect,
                radii,
            },
            _ => PaintItem::Clip { rect: pad_rect },
        };
        out.items.push(clip_item);
        // Sticky v2: a scroll container's span opens after its clip
        // — the clip rect (and the element's own bg/border above)
        // stay fixed while the scrolled subtree translates under
        // them. html/body never get here (viewport owns their
        // overflow, `clips` was flipped back to false above).
        if st.effective_overflow() != Overflow::Clip {
            scroll_span = Some((*dom_id, out.items.len()));
        }
    }
    BoxState {
        clips,
        xf_bracket,
        blend_bracket,
        scroll_span,
        child_xf,
        text_gradient,
    }
}

/// The walk's read-only inputs, shared by every frame of the recursion.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) tree: &'a DomTree,
    pub(super) taffy_tree: &'a TaffyTree<TextLeaf>,
    pub(super) node_map: &'a HashMap<diting_taffy::tree::NodeId, NodeId>,
    pub(super) styles: &'a HashMap<NodeId, ComputedStyle>,
    pub(super) fonts: &'a FontBook,
    pub(super) images: &'a HashMap<NodeId, DecodedImage>,
    pub(super) static_pos: &'a HashMap<NodeId, (f32, f32)>,
    pub(super) baseline_shifts: &'a HashMap<diting_taffy::tree::NodeId, f32>,
    pub(super) collapsed_edges: &'a HashMap<NodeId, [bool; 4]>,
    pub(super) viewport_width: f32,
}

/// What the walk accumulates (geometry maps, paint items, spans).
pub(super) struct Out<'a> {
    pub(super) rects: &'a mut HashMap<NodeId, Rect>,
    pub(super) local_geom: &'a mut HashMap<NodeId, (Rect, [f32; 6])>,
    pub(super) abs_by_node: &'a mut HashMap<diting_taffy::tree::NodeId, Rect>,
    pub(super) local_by_node: &'a mut HashMap<diting_taffy::tree::NodeId, (Rect, [f32; 6])>,
    pub(super) node_first_item: &'a mut HashMap<diting_taffy::tree::NodeId, usize>,
    pub(super) sticky_spans: &'a mut Vec<StickySpan>,
    pub(super) scroller_spans: &'a mut Vec<StickySpan>,
    pub(super) items: &'a mut Vec<PaintItem>,
    pub(super) paint_order: &'a mut Vec<NodeId>,
}

/// Text-run pass of one `collect` frame (moved out verbatim): shaped run
/// glyphs with decorations, ellipsis/line-clamp, shadows and the inherited
/// clip-text gradient.
#[allow(clippy::too_many_arguments)]
fn paint_text_run(
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

/// Stacking-band split of one `collect` frame's children (moved out
/// verbatim): each child lands in the negative / in-flow / positive z band,
/// or is deferred as a cross-parent escape when this frame is no barrier.
#[allow(clippy::too_many_arguments)]
fn band_children(
    cx: &Ctx,
    children: &[diting_taffy::tree::NodeId],
    abs: (f32, f32),
    alpha: f32,
    barrier: bool,
    child_xf: Xf,
    text_gradient: &Option<(TextGradient, [f32; 6])>,
    neg: &mut Vec<(i32, usize)>,
    mid: &mut Vec<(i32, usize)>,
    pos: &mut Vec<(i32, usize)>,
    pending: &mut Vec<ZEscape>,
) {
    let Ctx {
        tree,
        taffy_tree,
        node_map,
        styles,
        ..
    } = *cx;
    for (i, &child) in children.iter().enumerate() {
        let child_style = node_map
            .get(&child)
            .and_then(|d| styles.get(d))
            // A pseudo box leaf classifies by its own ComputedStyle
            // (#172): a positioned overlay ::before joins the
            // positioned band and out-paints in-flow content, like
            // Chrome's generated box.
            .or_else(|| pseudo_leaf_style(taffy_tree, styles, child));
        // Sticky counts as positioned here (CSS App. E step 8): a stuck
        // overlay must out-paint — and so out-rank in elementFromPoint,
        // which sorts by this order — any later static sibling whose tall
        // box covers the stuck band (uv-docs' .md-header case, #434).
        let mut positioned = child_style.is_some_and(|s| {
            matches!(
                s.position,
                Some(PositionMode::Relative)
                    | Some(PositionMode::Absolute)
                    | Some(PositionMode::Fixed)
                    | Some(PositionMode::Sticky)
            )
        });
        let mut member_z_override: Option<i32> = None;
        // Row-group wrappers (thead/tbody/tfoot stand-ins) establish no
        // box in CSS, so a sticky ROW inside one must still join the
        // table's positioned band — confining it to the group frame lets
        // a later tbody's tall static box cover the stuck header. The
        // group's own sticky style already classifies it directly; this
        // look-through only fires for a static group with positioned
        // member rows (z≠0 members stay per-parent, pre-existing).
        if !positioned {
            let is_group = node_map
                .get(&child)
                .and_then(|d| {
                    tree.with_node(*d, |n| {
                        n.as_element()
                            .map(|e| {
                                matches!(
                                    e.local.to_ascii_lowercase().as_str(),
                                    "thead" | "tbody" | "tfoot"
                                )
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if is_group {
                let mut any_pos = false;
                let mut member_max_z: i32 = 0;
                let mut member_min_z: i32 = 0;
                for m in taffy_tree.children(child).unwrap_or_default() {
                    if let Some(s) = node_map.get(&m).and_then(|d| styles.get(d)) {
                        if matches!(
                            s.position,
                            Some(PositionMode::Relative)
                                | Some(PositionMode::Absolute)
                                | Some(PositionMode::Fixed)
                                | Some(PositionMode::Sticky)
                        ) {
                            any_pos = true;
                            let mz = s.z_index.unwrap_or(0);
                            member_max_z = member_max_z.max(mz);
                            member_min_z = member_min_z.min(mz);
                        }
                    }
                }
                if any_pos {
                    positioned = true;
                    // A member carrying a nonzero z drags the whole
                    // group into its hoisted band (a sticky row with
                    // z-index must still out-rank a positioned z:2
                    // sibling of the table); signless members keep the
                    // plain positioned band below.
                    if member_max_z > 0 {
                        member_z_override = Some(member_max_z);
                    } else if member_min_z < 0 {
                        member_z_override = Some(member_min_z);
                    }
                }
            }
        }
        let floated = child_style.is_some_and(|s| s.float_side.is_some());
        let z =
            member_z_override.unwrap_or_else(|| child_style.and_then(|s| s.z_index).unwrap_or(0));
        if z != 0 && positioned {
            // Hoisted band: painted before (z<0) / after (z>0) the
            // middle band, ascending within the band.
            if z < 0 {
                neg.push((z, i));
            } else if barrier {
                pos.push((z, i));
            } else {
                // Defer past this non-context parent, carrying the walk
                // env the direct call would have received — geometry is
                // recursion-order-free, so the subtree paints identical
                // wherever its slot lands; the barrier emits it in ITS
                // positive band below.
                pending.push(ZEscape {
                    z,
                    child,
                    offset: abs,
                    xf: child_xf,
                    alpha,
                    gradient: text_gradient.clone(),
                });
            }
        } else if positioned {
            mid.push((2, i)); // paint level 2: above in-flow content
        } else if floated {
            mid.push((1, i)); // paint level 1: above static, below positioned
        } else {
            mid.push((0, i));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect(
    cx: &Ctx,
    out: &mut Out,
    escaped: &mut Vec<ZEscape>,
    node: diting_taffy::tree::NodeId,
    offset: (f32, f32),
    xf: Xf,
    alpha: f32,
    text_gradient: Option<&(TextGradient, [f32; 6])>,
) {
    let Ctx {
        tree,
        taffy_tree,
        node_map,
        styles,
        baseline_shifts,
        ..
    } = *cx;
    let Ok(layout) = taffy_tree.layout(node) else {
        return;
    };
    let item0 = out.items.len();
    // background-clip: text (gradient-text batch): inherited from the
    // nearest boxed clip:text ancestor, shadowed locally so a nested
    // clip:text element replaces it for its own subtree only — siblings
    // never see each other's gradient.
    let mut text_gradient = text_gradient.cloned();
    // Hit testing (obscura #738): record this element's slot in the flat
    // paint sequence as the walk reaches it. Because `collect` pushes a
    // node's own items before recursing and sorts children into the
    // z-index bands below, the resulting vector IS the paint order —
    // descendants after ancestors, hoisted z>0 siblings after the flow.
    // Text leaves and the synthetic ICB root never enter `node_map`, so
    // only boxed elements land here.
    if let Some(dom_id) = node_map.get(&node) {
        out.paint_order.push(*dom_id);
    }
    // transform (obscura #740 lineage, widened twice): the translate
    // part resolves against the element's own border box (percent
    // included) and folds into the offset — the element's rect, paint
    // items and whole subtree move together, layout never sees it
    // (Chrome: transforms don't affect layout). The linear part waits
    // for the box below: it pivots on the box center.
    let mut offset = offset;
    let mut t_lin = (1.0f32, 0.0f32, 0.0f32, 1.0f32);
    if let Some(dom_id) = node_map.get(&node) {
        if let Some(t) = styles.get(dom_id).and_then(|s| s.effective_transform()) {
            let resolve = |l: diting_css::Length, basis: f32| match l {
                diting_css::Length::Px(v) => v,
                diting_css::Length::Percent(p) => p / 100.0 * basis,
                // Percent-only approximation, same as to_taffy_style.
                diting_css::Length::Calc { percent, .. } => percent / 100.0 * basis,
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent => 0.0,
            };
            offset.0 += resolve(t.tx, layout.size.width);
            offset.1 += resolve(t.ty, layout.size.height);
            t_lin = (t.a, t.b, t.c, t.d);
        }
    }
    // opacity (animation batch A): non-inherited, multiplies down the
    // subtree — a faded parent fades everything inside it, matching
    // Chrome's group compositing for fades.
    let alpha = alpha
        * node_map
            .get(&node)
            .and_then(|d| styles.get(d))
            .and_then(|s| s.opacity)
            .unwrap_or(1.0);
    // Baseline alignment (blitz#750 family): run items drop onto their
    // line's baseline. Folded into the offset like the translate above,
    // so the item's rect, paint and whole subtree move together.
    if let Some(dy) = baseline_shifts.get(&node) {
        offset.1 += dy;
    }
    // Strut descent (replaced inline atoms): the taffy leaf is taller
    // than the element box by the strut's descent so the line box spans
    // it; every recorded box (rect/local/abs — gBCR, paint, hit test)
    // reports the element box.
    let strut_pad = match taffy_tree.get_node_context(node) {
        Some(TextLeaf::Replaced { strut_descent }) => *strut_descent,
        _ => 0.0,
    };
    let abs = (offset.0 + layout.location.x, offset.1 + layout.location.y);
    // The map handed to descendants (animation batch B): upgraded below
    // once this element's own box is final, since the scale part pivots
    // on the box center.
    let mut child_xf = xf;
    // Every visited node's absolute border box — the union pass after
    // the walk rebuilds rects for flattened inline wrappers from kids.
    // The local-space twin (affine residuals batch ②) records the
    // PRE-map box plus the incoming map: the inline-band pass splits
    // text runs into line bands where the wrap width still means what
    // the paint item's `wrap_at` means, then maps each band.
    out.local_by_node.insert(
        node,
        (
            Rect {
                x: abs.0,
                y: abs.1,
                width: layout.size.width,
                height: layout.size.height - strut_pad,
            },
            xf.to_array(),
        ),
    );
    out.abs_by_node.insert(
        node,
        xf.map_rect(Rect {
            x: abs.0,
            y: abs.1,
            width: layout.size.width,
            height: layout.size.height - strut_pad,
        }),
    );
    let mut clips = false;
    let mut xf_bracket = false;
    let mut blend_bracket = false;
    // Sticky v2: while `clips` is true and the used overflow is
    // scrollable (anything but `clip` — `hidden` IS a scroll container
    // per css-overflow), this element is a scroll container. Its subtree
    // items form a span recorded AFTER the Clip push (the clip rect
    // itself must not translate when the subtree is scrolled) and closed
    // BEFORE the PopClip push.
    let mut scroll_span: Option<(NodeId, usize)> = None;
    if let Some(dom_id) = node_map.get(&node) {
        let state = element_box(
            cx,
            out,
            dom_id,
            node,
            layout,
            abs,
            alpha,
            strut_pad,
            t_lin,
            xf,
            BoxState {
                clips,
                xf_bracket,
                blend_bracket,
                scroll_span,
                child_xf,
                text_gradient,
            },
        );
        BoxState {
            clips,
            xf_bracket,
            blend_bracket,
            scroll_span,
            child_xf,
            text_gradient,
        } = state;
    } else if taffy_tree
        .get_node_context(node)
        .is_some_and(|l| matches!(l, TextLeaf::Pseudo { .. }))
    {
        // An empty-content pseudo box (#172): the leaf has no DOM id,
        // its ComputedStyle rides the HOST's styles entry — claim the
        // pseudo's own decoration layers (background, gradient,
        // shadows, border). No host-keyed bookkeeping: rects/
        // paint_order/clip stay element-side, a pseudo generates no
        // DOM box.
        if let Some(ps) = pseudo_leaf_style(taffy_tree, styles, node) {
            let alpha = alpha * ps.opacity.unwrap_or(1.0);
            let rect = Rect {
                x: abs.0,
                y: abs.1,
                width: layout.size.width,
                height: layout.size.height,
            };
            // Same prebake split as the element path: geometry maps
            // through the accumulated affine when diagonal, stays in
            // local coordinates under a SetXf bracket.
            let prebake = xf.is_diagonal();
            let bg_rect = if prebake { xf.map_rect(rect) } else { rect };
            let res = |l: &diting_css::Length, basis: f32| match l {
                diting_css::Length::Px(v) => *v,
                diting_css::Length::Percent(p) => p * basis / 100.0,
                diting_css::Length::Calc { percent, .. } => percent * basis / 100.0,
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent => 0.0,
            };
            let radii: [(f32, f32); 4] = ps
                .corner_radii
                .as_ref()
                .map(|corners| {
                    let rs = std::array::from_fn(|i| {
                        if prebake {
                            (
                                res(&corners[i].0, rect.width) * xf.a,
                                res(&corners[i].1, rect.height) * xf.d,
                            )
                        } else {
                            (
                                res(&corners[i].0, rect.width),
                                res(&corners[i].1, rect.height),
                            )
                        }
                    });
                    clamp_corner_radii(rs, bg_rect.width, bg_rect.height)
                })
                .unwrap_or([(0.0, 0.0); 4]);
            if alpha > 0.0 && rect.width > 0.0 && rect.height > 0.0 {
                if let Some(blur) = ps.backdrop_blur.filter(|b| *b > 0.0) {
                    out.items.push(PaintItem::BackdropFilter {
                        rect: bg_rect,
                        radii,
                        blur,
                    });
                }
                if let Some(shadows) = ps.box_shadow.as_ref() {
                    for sh in shadows.iter().rev().filter(|sh| !sh.inset) {
                        out.items.push(PaintItem::BoxShadow {
                            rect: bg_rect,
                            color: with_alpha(
                                [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
                                alpha,
                            ),
                            radii,
                            dx: sh.dx,
                            dy: sh.dy,
                            blur: sh.blur,
                            spread: sh.spread,
                            inset: false,
                        });
                    }
                }
                if let Some(c) = ps.background_color.filter(|c| c.3 != 0) {
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
                // An EMPTY box has no glyphs — background-clip:text
                // degrades to painting the box (Chrome never generates
                // the pseudo without content text to fill).
                if let Some(g) = ps
                    .background_image
                    .as_deref()
                    .and_then(diting_css::parse_linear_gradient)
                {
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
                if let Some(shadows) = ps.box_shadow.as_ref() {
                    for sh in shadows.iter().rev().filter(|sh| sh.inset) {
                        out.items.push(PaintItem::BoxShadow {
                            rect: bg_rect,
                            color: with_alpha(
                                [sh.color.0, sh.color.1, sh.color.2, sh.color.3],
                                alpha,
                            ),
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
            // Border outside the alpha/size guard, same stance as the
            // element path; currentColor falls back to the pseudo's
            // inherited color.
            if ps.border_style.is_some() {
                let style = ps;
                let (kx, ky) = if prebake { (xf.a, xf.d) } else { (1.0, 1.0) };
                let widths = [
                    side_px(style.border_width.top) * ky,
                    side_px(style.border_width.right) * kx,
                    side_px(style.border_width.bottom) * ky,
                    side_px(style.border_width.left) * kx,
                ];
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
    }
    // background-clip: text attach: only while this leaf still paints
    // in the captured space (transform mismatch → solid), this leaf's
    // inherited opacity folded into the stops.
    let text_gradient_fill = text_gradient
        .as_ref()
        .filter(|(_, at)| *at == xf.to_array())
        .map(|(g, _)| TextGradient {
            area: g.area,
            stops: g
                .stops
                .iter()
                .map(|(p, c)| (*p, with_alpha(*c, alpha)))
                .collect(),
            css_deg: g.css_deg,
            clone_per_line: g.clone_per_line,
            clone_lines: g.clone_lines.clone(),
        });
    // Inherited text-shadow layers with this element's opacity folded
    // into each color (the item colors carry alpha, shadows ride along).
    // Text leaves sit outside `node_map` — walk up to the nearest boxed
    // ancestor, whose style already carries the inherited layer list.
    let mut shadow_dom = node_map.get(&node).copied();
    let mut shadow_up = taffy_tree.parent(node);
    while shadow_dom.is_none() {
        match shadow_up {
            Some(p) => {
                shadow_dom = node_map.get(&p).copied();
                shadow_up = taffy_tree.parent(p);
            }
            None => break,
        }
    }
    let run_text_shadow = shadow_dom
        .and_then(|dom_id| styles.get(&dom_id))
        .and_then(|s| s.text_shadow.clone())
        .map(|layers| {
            layers
                .into_iter()
                .map(|mut sh| {
                    let c = with_alpha([sh.color.0, sh.color.1, sh.color.2, sh.color.3], alpha);
                    sh.color = diting_css::Color(c[0], c[1], c[2], c[3]);
                    sh
                })
                .collect()
        });
    paint_text_run(
        cx,
        out,
        node,
        abs,
        alpha,
        xf,
        &run_text_shadow,
        &text_gradient_fill,
    );
    if let Some(TextLeaf::Word {
        text,
        font_size,
        bold,
        color,
        line_height,
        decorations,
        mono,
        han,
        ..
    }) = taffy_tree.get_node_context(node)
    {
        // A word leaf paints at its own box — the enclosing flex row
        // already did the line breaking (leaf-level wrap). Single-token
        // text can never break, so wrap_at just equals the leaf width.
        // Same diagonal/non-diagonal split as the Run leaf above.
        if xf.is_diagonal() {
            out.items.push(PaintItem::Text {
                text: text.clone(),
                font_size: font_size * xf.d,
                bold: *bold,
                color: with_alpha(*color, alpha),
                line_height: line_height * xf.d,
                x: abs.0 * xf.a + xf.e,
                y: abs.1 * xf.d + xf.f,
                wrap_at: layout.size.width * xf.a,
                gradient: text_gradient_fill.clone(),
                decorations: *decorations,
                mono: *mono,
                word_spacing: 0.0,
                truncate_at: None,
                tokens: None,
                ws: WhiteSpace::Normal,
                text_shadow: run_text_shadow.clone(),
                // Word leaves are pre-uppercased at build time.
                small_caps: false,
                han: *han,
                last_line_align: None,
                src: text_src::lookup(node),
            });
        } else {
            out.items.push(PaintItem::Text {
                text: text.clone(),
                font_size: *font_size,
                bold: *bold,
                color: with_alpha(*color, alpha),
                line_height: *line_height,
                x: abs.0,
                y: abs.1,
                wrap_at: layout.size.width,
                gradient: text_gradient_fill.clone(),
                decorations: *decorations,
                mono: *mono,
                word_spacing: 0.0,
                truncate_at: None,
                tokens: None,
                ws: WhiteSpace::Normal,
                text_shadow: run_text_shadow.clone(),
                // Word leaves are pre-uppercased at build time.
                small_caps: false,
                han: *han,
                last_line_align: None,
                src: text_src::lookup(node),
            });
        }
    }
    // Inline background bands (blitz#340 family) splice at the first
    // paint item of the band owner's content — record this node's first
    // item index while the walk is here. +1 past a SetXf pushed at
    // item0, so splices land inside the bracket (and are skipped there
    // — see the splice site).
    let rec = item0 + usize::from(xf_bracket);
    if out.items.len() > rec {
        out.node_first_item.entry(node).or_insert(rec);
    }
    // Stacking order (batch 6a, float level in 8f), the blitz-dom
    // damage.rs model per parent: children with z-index ≠ 0 that are
    // positioned hoist out of document order into the negative band
    // (painted first, sorted by z ascending) and the positive band
    // (last, ascending); the rest paint between them in a stable sort
    // by paint level — in-flow (static) 0 first, floats 1 (CSS 2.1
    // App. E step 5: a float paints above the in-flow blocks and text
    // of its own band), positioned z-auto 2 above. Same-z ties keep
    // tree order; text leaves are always in-flow.
    let children = taffy_tree.children(node).unwrap_or_default().to_vec();
    // Cross-parent z (the narrow slice): a positioned z>0 child of a
    // NON-stacking-context parent must hoist past it — in Chrome,
    // A{relative}>B{absolute z:5} out-paints A's later sibling
    // C{relative z:1}. This frame is a BARRIER when it consumes such
    // children locally: a stacking context, a clipper (a hoisted range
    // would tear its Clip/PopClip pair), or a sticky subtree (its
    // shift span must stay contiguous). Every bracket producer implies
    // a barrier condition — xf_bracket needs a transform, clips needs
    // clips_descendants — so a hoisted subtree can never land inside
    // someone else's brackets. Negative z stays per-parent: the linear
    // walk cannot emit before this node's own background, and a neg-z
    // child already paints above it today (pre-existing inaccuracy,
    // out of this slice). The synthetic ICB is not in node_map →
    // always a barrier.
    // Row-group wrappers hoisted into a z band by their member rows
    // (look-through below) carry positioned subtrees without being
    // stacking contexts themselves — they must stop escapes at the
    // band slot, or their recursion would append back into the
    // consuming barrier's `below` after it drained.
    let self_is_group = node_map
        .get(&node)
        .and_then(|d| {
            tree.with_node(*d, |n| {
                n.as_element()
                    .map(|e| {
                        matches!(
                            e.local.to_ascii_lowercase().as_str(),
                            "thead" | "tbody" | "tfoot"
                        )
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let barrier = self_is_group
        || match node_map.get(&node).and_then(|d| styles.get(d)) {
            None => true,
            Some(s) => {
                establishes_stacking_context(s)
                    || s.clips_descendants()
                    || s.position == Some(PositionMode::Sticky)
            }
        };
    let mut neg: Vec<(i32, usize)> = Vec::new();
    let mut mid: Vec<(i32, usize)> = Vec::new();
    let mut pos: Vec<(i32, usize)> = Vec::new();
    // z>0 children this non-barrier frame defers up to the barrier.
    let mut pending: Vec<ZEscape> = Vec::new();
    band_children(
        cx,
        &children,
        abs,
        alpha,
        barrier,
        child_xf,
        &text_gradient,
        &mut neg,
        &mut mid,
        &mut pos,
        &mut pending,
    );
    neg.sort_by_key(|(z, _)| *z);
    mid.sort_by_key(|(lvl, _)| *lvl);
    pos.sort_by_key(|(z, _)| *z);
    // Emission, neg → mid → pos as before. `below` catches escapes
    // bubbled up from non-context mid children — neg/pos children carry
    // z≠0, are themselves contexts, and consume locally, so only the
    // mid band can hand escapes up.
    let mut below: Vec<ZEscape> = Vec::new();
    macro_rules! emit {
        ($n:expr, $o:expr, $x:expr, $a:expr, $g:expr) => {
            collect(cx, out, &mut below, $n, $o, $x, $a, $g)
        };
    }
    for (_, i) in neg {
        emit!(children[i], abs, child_xf, alpha, text_gradient.as_ref());
    }
    for (_, i) in mid {
        emit!(children[i], abs, child_xf, alpha, text_gradient.as_ref());
    }
    // Positive band: own z>0 children merged with the bubbled escapes,
    // stable-sorted by z — own children precede escapes on equal z
    // (direct children ahead of hoisted descendants). ONLY a barrier
    // may consume escapes here; a non-barrier frame hands `below` up
    // untouched with its own pending, or a nephew would settle at this
    // frame's level instead of the real stacking context.
    enum PosSlot {
        Own(usize),
        Up(ZEscape),
    }
    let mut seq: Vec<(i32, PosSlot)> = pos.into_iter().map(|(z, i)| (z, PosSlot::Own(i))).collect();
    if barrier {
        seq.extend(below.drain(..).map(|e| (e.z, PosSlot::Up(e))));
    }
    seq.sort_by_key(|(z, _)| *z);
    for (_, slot) in seq {
        match slot {
            PosSlot::Own(i) => {
                emit!(children[i], abs, child_xf, alpha, text_gradient.as_ref())
            }
            PosSlot::Up(e) => emit!(e.child, e.offset, e.xf, e.alpha, e.gradient.as_ref()),
        }
    }
    if barrier {
        // Pos-band emission targets are all contexts, so `below` is
        // empty again here — asserted rather than assumed.
        debug_assert!(below.is_empty());
    } else {
        // Hand the deferred band to the parent frame's walk: own
        // pending children first, then bubbled nephews, matching the
        // own-before-escapes tie rule above.
        escaped.append(&mut pending);
        escaped.append(&mut below);
    }
    // Sticky v2: the scroller's span closes before the PopClip — the
    // closing bracket itself must not translate. Same span shape as the
    // sticky spans above, consumed by the same apply_sticky_to_items.
    if let Some((sid, s0)) = scroll_span {
        out.scroller_spans.push((sid, s0, out.items.len()));
    }
    if clips {
        out.items.push(PaintItem::PopClip);
    }
    if xf_bracket {
        out.items.push(PaintItem::ClearXf);
    }
    // #222: the blend bracket closes OUTSIDE the xf bracket — the
    // transform composes the group's content first, then the whole
    // group blends (CSS order: transform is part of the element's
    // rendering that mix-blend-mode composites).
    if blend_bracket {
        out.items.push(PaintItem::BlendEnd);
    }
    // Sticky spans (root-scroller v1): a sticky subtree's items are a
    // contiguous run — the walk emits the node's own band, then its
    // children (z-sorted), then the closing brackets — so the span is
    // exactly [item0, len) recorded here, pre-splice. Item0 was
    // captured at entry, before this node pushed anything.
    if let Some(dom_id) = node_map.get(&node) {
        if styles
            .get(dom_id)
            .is_some_and(|s| s.position == Some(PositionMode::Sticky))
        {
            out.sticky_spans.push((*dom_id, item0, out.items.len()));
        }
    }
}
