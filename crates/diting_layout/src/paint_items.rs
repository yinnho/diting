//! Paint-list types: `PaintItem`, form widgets, and text gradients.
//!
//! Split from the layout hub. The public names stay re-exported from
//! the crate root. Pure move.

use super::*;

/// How a form control lays its text run out inside the replaced box (form
/// paint polish batch): Chrome's control padding plus vertical centering
/// for the single-line controls, top-anchored for textarea, and the
/// select's reserved dropdown-arrow zone. Resolved at collect time so
/// paint stays tree-free, the same posture as [`FormWidget`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormRun {
    /// Text-like input: 2px side padding, vertically centered run.
    Input,
    /// Button-like input: light button-face field, label centered both axes.
    Button,
    /// Textarea: 2px padding, top-anchored — a multiline run centered
    /// vertically would jump as it grows.
    Textarea,
    /// Select: like Input plus a 16px arrow zone reserved at the right,
    /// where the executor paints the closed control's ▼ mark.
    Select,
}

/// A checkable input's native widget (form paint batch): the replaced box
/// draws the control itself — a bordered square with a ✓ for checkboxes, a
/// ring with an inner dot for radios. `checked` is resolved at collect time
/// (the live_checked mirror, else the parsed `checked` attribute) so paint
/// stays tree-free, the same posture as the alt run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormWidget {
    Checkbox {
        checked: bool,
    },
    Radio {
        checked: bool,
    },
    /// A range slider's static geometry (blitz#456): the thumb's position
    /// along the track as a 0..1 fraction of the inset track span, read
    /// from the live value at widget-construction time.
    Range {
        fraction: f32,
    },
}

/// One paint primitive in document order (batch 4a) — the minimal output
/// contract between layout and paint. `Bg` is an element's solid
/// background-color over its border-box (rounded per border-radius, batch
/// 6b); `Text` is a text run at its leaf origin, re-wrapped at the width
/// its containing block gave it at measure time (mixed runs paint
/// per-word leaves at their own boxes, batch 4d). Shadows and elliptical/
/// per-corner radii don't exist yet in this slice.
#[derive(Debug, Clone)]
pub enum PaintItem {
    Bg {
        rect: Rect,
        color: [u8; 4],
        radius: f32,
    },
    /// Per-corner radii variant of `Bg` (batch 7c): CSS corner order
    /// (TL TR BR BL), each (rx, ry) already resolved to px.
    BgCorner {
        rect: Rect,
        color: [u8; 4],
        radii: [(f32, f32); 4],
    },
    /// A `box-shadow` layer (blitz#349 family): `rect` is the element's own
    /// box, `radii` the same clamped per-corner values a `Bg` on this box
    /// gets, and dx/dy/blur/spread are the layer's px lengths. Outer layers
    /// paint BEFORE the element's `Bg` so the background covers the shadow
    /// inside the box edge; inset layers (`inset: true`) paint AFTER the
    /// background (and gradient) but BEFORE the `Border`, hard-clipped to
    /// the box — Chrome's inner-shadow phase.
    BoxShadow {
        rect: Rect,
        color: [u8; 4],
        radii: [(f32, f32); 4],
        dx: f32,
        dy: f32,
        blur: f32,
        spread: f32,
        inset: bool,
    },
    /// A `backdrop-filter: blur()` region (blitz#901 family): the first
    /// item a glass element emits, so it filters everything painted
    /// beneath it so far. `rect` is the element's border box and `radii`
    /// the same clamped per-corner values a `Bg` on this box gets. The
    /// blur may SAMPLE outside the box (a blur needs input) but the
    /// composited output clips to the ROUNDED border shape, not the
    /// axis-aligned rect — the exact bug upstream blitz#901 hit.
    BackdropFilter {
        rect: Rect,
        radii: [(f32, f32); 4],
        blur: f32,
    },
    /// A `background-image: linear-gradient(...)` fill (gradient batch).
    /// `stops` are (0..1 position, straight RGBA) ascending, the CSS angle
    /// is in degrees (0 = to top, clockwise), and `radii` are the same
    /// per-corner values a solid `Bg` would clip to — the fill follows the
    /// rounded box. Paints AFTER the element's `Bg` (background-color is
    /// the bottom layer in CSS); element-subtree opacity is already folded
    /// into the stop alphas at emission.
    BgGradient {
        rect: Rect,
        stops: Vec<(f32, [u8; 4])>,
        css_deg: f32,
        radii: [(f32, f32); 4],
    },
    /// A decoded raster image blitted into the replaced box (batch 5b):
    /// sized per object-fit and offset per object-position (batch 5c) —
    /// see [`object_paint_rect`]. `rect` is the element box and doubles as
    /// the clip (replaced content never escapes it); `paint_rect` is the
    /// computed blit destination.
    Image {
        rect: Rect,
        paint_rect: Rect,
        image: DecodedImage,
        /// Element-subtree opacity (animation batch A) multiplied into the
        /// source pixels at blit time — raster colors can't fold a group
        /// alpha any other way.
        alpha: f32,
    },
    /// A replaced element's placeholder (batch 5a): an optional gray box
    /// (skipped when the author styled a background — that already shows)
    /// plus the alt text run, already resolved to font/color context and
    /// wrapped inside the box at paint time. Upstream blitz paints
    /// nothing for an unloaded img (`draw_image` is a no-op without
    /// raster data), so the placeholder is OUR no-network product policy,
    /// locked by structural tests; the cross-checked part is the shared
    /// box geometry (batch 2 rects) and the img's own CSS background.
    Replaced {
        rect: Rect,
        /// (text, font_size, bold, line_height, color) of the alt run; None
        /// without an alt attribute (present-but-empty paints box-only, like
        /// alt="").
        alt: Option<(String, f32, bool, f32, [u8; 4])>,
        fill_placeholder: bool,
        /// Element-subtree opacity (animation batch A) applied to the
        /// placeholder fill and the alt run's color at paint time.
        alpha: f32,
        /// A checkable input's native widget (form paint batch): when set,
        /// paint draws the control itself instead of text — the checkedness
        /// resolved here at collect time (live_checked mirror, else the
        /// parsed attribute), exactly the JS getter's precedence.
        widget: Option<FormWidget>,
        /// The control's run layout (form paint polish batch): when set,
        /// paint draws Chrome's default control shell (1px gray ring, white
        /// field — only while the author styled no background of their own)
        /// and insets/centers the text run per kind. Mutually exclusive with
        /// `widget` in practice: checkables resolve None here.
        form: Option<FormRun>,
        /// Caret (typing-cursor batch): (char offset, ink) in the focused
        /// text-entry control, resolved at collect time — the offset is the
        /// recorded selection's anchor (min(start, end)) clamped to the
        /// value's char count, the ink the control's color context (never
        /// the placeholder gray). Some only while this node holds focus AND
        /// a selection was recorded for it; paint keeps no tree access, so
        /// the whole condition is evaluated here. Deterministic always-on
        /// (no blink) — a screenshot must show it.
        caret: Option<(usize, [u8; 4])>,
        /// RTL value anchoring (#188-3, blitz#981): the control's base
        /// direction resolved at collect time — an Input/Textarea whose
        /// direction is rtl anchors each value line at the RIGHT edge (an
        /// RTL paragraph's start side). Select/Button stay physical; runs
        /// keep logical order (no UAX #9 reordering).
        form_rtl: bool,
    },
    /// A compiled svg subtree painted into its replaced box (svg v1): the
    /// op list is in viewBox user units with group transforms pre-flattened
    /// at collect time; paint maps it through the box (element sizing IS
    /// the zoom). Arc because the item list is cloned per band paint and
    /// the op list is immutable from here on.
    Svg {
        rect: Rect,
        render: std::sync::Arc<svg::SvgRender>,
        /// Element-subtree opacity (animation batch A) multiplied into every
        /// op color at paint time — the compiled op list is shared/immutable.
        alpha: f32,
    },
    /// Begin clipping descendants to `rect` (the padding box of an
    /// overflow-clipping element) until the matching [`PaintItem::PopClip`].
    /// The flat item list carries the tree's clip structure in document
    /// order; nesting is an intersection.
    Clip {
        rect: Rect,
    },
    /// Rounded variant of `Clip` (batch 7d): the clipping box's own radii
    /// cut descendants' ink along the curve, like upstream's rounded
    /// padding-box BezPath clip.
    ClipRounded {
        rect: Rect,
        radii: [(f32, f32); 4],
    },
    PopClip,
    /// Begin a non-diagonal transform bracket (affine batch): everything
    /// between here and the matching [`PaintItem::ClearXf`] was emitted in
    /// LOCAL coordinates and maps through `xf` (CSS matrix order
    /// a b c d e f: x' = a·x + c·y + e, y' = b·x + d·y + f). Diagonal
    /// transforms (translate/scale) never use this — they pre-bake their
    /// geometry into item fields at collect time, so untouched pages keep
    /// the exact pre-batch path. CSS makes a transform a stacking context,
    /// so the bracket wraps the element's whole subtree by construction.
    SetXf {
        xf: [f32; 6],
    },
    /// End the nearest open [`PaintItem::SetXf`] bracket.
    ClearXf,
    /// Paint in CANVAS coordinates until the matching [`PaintItem::ClearXf`]
    /// (affine residuals batch): the pushed map REPLACES the open bracket's
    /// total instead of composing onto it, cancelling the enclosing
    /// transform. Emitted by the inline-band splice — its band rects come
    /// from `abs_by_node`, which already maps through ancestor transforms,
    /// so splicing them raw inside a bracket would double-map. The pair is
    /// self-contained; paint after the ClearXf returns to the bracket.
    SetXfCanvas,
    /// Begin a mix-blend-mode group (#222): everything until the matching
    /// [`PaintItem::BlendEnd`] paints into an offscreen transparent canvas,
    /// and `BlendEnd` composites the group onto the backdrop below with the
    /// mode — the W3C compositing formula per channel, with color-dodge and
    /// color-burn in exact integer arithmetic (takumi #1791: a float divide
    /// lands off-by-one through the gradient transition bands; integers
    /// can't). The bracket opens before the element's own first item and
    /// closes after its whole subtree — mix-blend-mode establishes a
    /// stacking context, so a hoisted z>0 child can never land inside a
    /// foreign blend bracket (same barrier rule as the xf bracket).
    BlendBegin {
        mode: diting_css::MixBlendMode,
    },
    /// Close the nearest open [`PaintItem::BlendBegin`] bracket.
    BlendEnd,
    /// A uniform solid border: four bands on the border-box edges (rounded
    /// ring when `radii` are set), painted AFTER the element's Bg
    /// (background-clip: border-box draws the bg beneath the border) and
    /// before the subtree. Widths in CSS order (top right bottom left), px;
    /// `radii` are the same per-corner values a `BgCorner` fill would use —
    /// the ring is the outer rounded box minus the widths-inset inner box
    /// (inner radii shrink by the adjacent border widths). All-zero radii
    /// keep the historical four-band fast path.
    Border {
        rect: Rect,
        widths: [f32; 4],
        color: [u8; 4],
        radii: [(f32, f32); 4],
    },
    Text {
        text: String,
        font_size: f32,
        bold: bool,
        color: [u8; 4],
        /// Used line-height (px) — the leaf's own measure-time value, so the
        /// paint baselines land exactly where layout placed the line boxes.
        line_height: f32,
        /// Leaf origin (line-box top-left), page px.
        x: f32,
        y: f32,
        /// The wrap width the containing block offered at measure time.
        wrap_at: f32,
        /// `background-clip: text` fill (gradient-text batch): when the
        /// nearest boxed ancestor established one, glyph coverage samples
        /// this gradient across `area` instead of filling with `color` —
        /// the run's color is ignored entirely (CSS paints the background
        /// through the glyphs; `-webkit-text-fill-color: transparent` hides
        /// the normal fill, which we get by construction).
        gradient: Option<TextGradient>,
        /// Propagated `text-decoration-line` set (CSS 2.1 §16.3.1 — the
        /// decoration an ancestor declared paints across this inline's text
        /// in the TEXT's own color, so it rides the leaf, not the ancestor).
        decorations: TextDecorations,
        /// The run rides the monospace face (mono batch): a `monospace`
        /// member anywhere in the nearest ancestor's font-family list.
        mono: bool,
        /// Inherited `word-spacing` (CSS Text §7.1), px — folded into each
        /// space token's advance at measure AND paint so wrap, glyphs and
        /// decorations all separate by the same amount.
        word_spacing: f32,
        /// `text-overflow: ellipsis` limit (nowrap+ellipsis batch): when
        /// Some, paint drops trailing tokens and appends "…" so the ink fits
        /// this width; layout rects, scroll extents and selection keep the
        /// full text (the marker is a rendering effect, CSS UI §5.2).
        truncate_at: Option<f32>,
        /// The run's wrap tokens, pre-shaped from the leaf's measure-time
        /// memo (obscura#983's paint half): the glyph-pixel RasterCache
        /// holds no wrap geometry, so the decorations painter and a
        /// first-raster cache miss would re-shape the run on every repaint
        /// without this. Some only when the item's font params are the
        /// leaf's UNSCALED ones (identity d — a diagonal scale folds d into
        /// font_size/word_spacing and the memo no longer matches); word
        /// leaves carry None.
        tokens: Option<std::rc::Rc<[text::Token]>>,
        /// The run's white-space mode (pre family): selects the tokenizer
        /// and wrap regime at raster time — wrap_at=+inf alone can't tell
        /// `nowrap` (collapse, one line) from `pre` (preserve, hard breaks).
        ws: WhiteSpace,
        /// `text-shadow` layers (blitz#271 family, inherited), element
        /// opacity already folded into each color: painted UNDER the glyphs
        /// (and decorations), first-declared layer on top.
        text_shadow: Option<Vec<TextShadow>>,
        /// Synthesized small-caps (small-caps batch, inherited): lowercase
        /// runs render as uppercase glyphs at 70% size. Word leaves carry
        /// false — their segments are pre-uppercased at build time.
        small_caps: bool,
        /// #139 (Han unification): the run's Han slot (nearest ancestor
        /// lang) — rides the item so measure and paint segment identically.
        han: Option<text::HanSlot>,
        /// `text-align-last` (#214, blitz#998), resolved PHYSICAL against
        /// the owning block's direction at collect time: only Center/Right
        /// arrive (auto/start/left fold to None). Paint offsets the run's
        /// LAST line inside its tile; word leaves carry None — the flex row
        /// breaks mixed runs, and per-line alignment there is a later face.
        last_line_align: Option<TextAlign>,
        /// The DOM text node(s) (node, start char, char length) this item's text
        /// carries — range / client-rect queries map fragments back through it.
        src: Option<text_src::Segs>,
    },
}

/// A `background-clip: text` fill (gradient-text batch): the gradient a
/// clip:text element suppressed from its own box, re-aimed at the glyphs of
/// its subtree. Coverage samples the gradient at each pixel's own position
/// in the text item's coordinate space, so the fill tracks the gradient's
/// angle across the whole element, not per glyph.
#[derive(Debug, Clone)]
pub struct TextGradient {
    /// The clip element's background box (exactly what a `BgGradient` would
    /// have filled) in the text item's coordinate space — page space on the
    /// prebaked diagonal path, local inside a transform bracket, where the
    /// affine then carries the gradient through the rotation.
    pub area: Rect,
    /// Raw stops — the text item folds its own inherited alpha at attach
    /// time, so opacity between the clip element and the text composes.
    pub stops: Vec<(f32, [u8; 4])>,
    /// linear-gradient angle in CSS degrees (0 = to top, clockwise), same
    /// convention `BgGradient` carries.
    pub css_deg: f32,
    /// `box-decoration-break: clone` (#218): restart per line; the
    /// per-line fragment boxes (x offset, advance).
    pub clone_per_line: bool,
    pub clone_lines: Vec<(f32, f32)>,
}
