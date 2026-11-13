//! `ComputedStyle`: the cascade's per-element result.
//!
//! Split from the css hub. The type stays re-exported from the crate root.
//! Pure move.

use super::*;

/// The computed values this slice models. Deliberately tiny: enough to lock
/// cascade ordering, specificity, inline override, and inheritance semantics.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComputedStyle {
    /// Outer display role. `None` means "not declared anywhere".
    pub display: Option<Display>,
    /// True while `display` still holds the cascade's UA-default fill
    /// (`Some(ua_display(tag))`). The CSSOM layer reports Chrome's per-tag
    /// UA display for those tags (table→table, li→list-item, ...) without
    /// the layout engine needing table/list box types (obscura #771).
    pub display_from_ua: bool,
    pub color: Option<Color>,
    /// Authored-space twins of the three colour slots (#17): Chrome's
    /// computed value keeps modern colour functions in their own space
    /// (`oklch(0.7 0.1 200)`), so the CSSOM face needs the tag alongside
    /// the sRGB bytes. `None` = legacy spelling (rgb()/rgba()).
    pub color_modern: Option<ModernColor>,
    /// `color-scheme` (#18): authored tokens (`light dark`), lowercased,
    /// non-inherited per spec; `None` = undeclared (gCS reports `normal`).
    pub color_scheme: Option<String>,
    /// The ancestor-resolved used scheme for this element HAD it declared no
    /// `color-scheme` of its own (#18): `light-dark()` with no own
    /// declaration walks the tree (Chrome: a root `dark` flips descendants),
    /// while the page default is LIGHT — prefers-color-scheme never reaches
    /// a `normal` page (probed Chrome 145, 2026-11-06).
    pub scheme_dark_hint: bool,
    /// SVG paint properties (svg v1). Deliberately NON-inherited here: the
    /// svg subtree compiler does paint inheritance itself, because CSS
    /// presentation-attribute priority (own CSS > own attr > inherited CSS)
    /// needs to distinguish "declared on this element" from "inherited",
    /// which a merged ComputedStyle can't. `None` = nothing declared on this
    /// element; the svg compiler seeds unset chains with the SVG initial
    /// values (fill black, stroke none, width 1).
    pub svg_fill: Option<SvgPaint>,
    pub svg_stroke: Option<SvgPaint>,
    pub svg_stroke_width: Option<f32>,
    pub svg_dasharray: Option<Vec<f32>>,
    /// `stroke-dashoffset` (animation input): the dash phase shift, in the
    /// same user units as the dasharray. The svg compiler folds it into the
    /// stroke so `stroke-dashoffset` keyframes produce the self-draw effect.
    pub svg_dashoffset: Option<f32>,
    pub background_color: Option<Color>,
    pub background_color_modern: Option<ModernColor>,
    /// `box-shadow` layers (blitz#349 family, v1), non-inherited; first
    /// layer paints on top (CSS paint order). `None` = `none`.
    pub box_shadow: Option<Vec<BoxShadow>>,
    /// `text-shadow` layers (blitz#271 family), INHERITED (copied down in
    /// `cascade_element` like `color`); first layer paints on top.
    /// `None` = `none`.
    pub text_shadow: Option<Vec<TextShadow>>,
    /// `backdrop-filter: blur(<length>)` (blitz#901 family), v1 blur-only,
    /// NON-inherited: the px blur radius applied to whatever painted
    /// beneath the element's border box before its own background. `None`
    /// = `none`.
    pub backdrop_blur: Option<f32>,
    /// `filter` (raw value, v1 position face): non-inherited. A non-none
    /// filter makes the element the containing block for its fixed AND
    /// absolute descendants (#316), like a transform; rendering the effects
    /// is later work. `None` = `none`.
    pub filter: Option<String>,
    /// `perspective` (raw value, v1 position face): same hijack semantics
    /// as filter. `None` = `none`.
    pub perspective: Option<String>,
    /// `mix-blend-mode` (#222), non-inherited: `None` = `normal`. A
    /// non-normal value establishes a stacking context and opens a blend
    /// bracket around the element's whole rendering at collect time.
    pub mix_blend_mode: Option<MixBlendMode>,
    /// Shorthand sides in CSS order (top right bottom left), already expanded.
    pub margin: Sides,
    pub padding: Sides,
    /// Uniform border (batch 4b): per-side widths, ONE color and ONE style
    /// for all sides — per-side colors/styles are later batches. A border
    /// lays out (content inset) and paints only when `border_style` is set
    /// to a line style; `none`/`hidden` (or unset, the CSS initial) mean no
    /// border. Widths accept the keywords thin/medium/thick (1/3/5px).
    pub border_width: Sides,
    pub border_color: Option<Color>,
    pub border_color_modern: Option<ModernColor>,
    pub border_style: Option<BorderStyle>,
    /// Authored box size (non-inherited): px (em/rem resolved) or %.
    /// Interpretation (content-box vs border-box edges) is `box_sizing`'s
    /// call — the layout layer does the edge mapping at taffy hand-off.
    pub width: Option<Length>,
    /// Percent-bearing math for width/min-width/max-width that only layout
    /// can evaluate (see `calc::pct_math`); the Length slot holds a placeholder.
    pub width_expr: Option<std::sync::Arc<str>>,
    pub min_width_expr: Option<std::sync::Arc<str>>,
    pub max_width_expr: Option<std::sync::Arc<str>>,
    pub height: Option<Length>,
    /// `box-sizing` (non-inherited; the `*` reset idiom sets it per element).
    /// `None` = the CSS initial `content-box`.
    pub box_sizing: Option<BoxSizing>,
    /// `background-clip: text` (non-inherited; real pages almost always ship
    /// the `-webkit-background-clip` alias, both land here). The background
    /// paints only inside the foreground glyphs — with a gradient this is the
    /// gradient-text idiom. `false` (the initial box fill) covers every box
    /// keyword; our background layers paint the border box regardless.
    pub background_clip_text: bool,
    /// `box-decoration-break: clone` (non-inherited, initial `slice`).
    /// V1 consumer: the inline background-clip: text fill, where clone
    /// restarts the gradient at every wrapped line (each line fragment is
    /// its own box). Block fragmentation effects are out of scope.
    pub box_decoration_clone: bool,
    /// Font size in px (absolute keywords/units resolved by the caller's sheet
    /// context; here we accept px/em/% where em resolves against parent).
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    /// Inherited: nearest ancestor's declared value, `None` = `normal`.
    pub line_height: Option<LineHeightSpec>,
    /// Inherited: extra space added to each rendered word-separator (the
    /// collapsed " " token's advance, CSS Text §7.1). `None` = `normal`
    /// (no extra); negative values tighten.
    pub word_spacing: Option<f32>,
    /// Inherited: `font-variant-caps` synthesis (`small-caps`). `None` =
    /// unset (inherits as `normal`). Synthesized small caps render lowercase
    /// runs as uppercase glyphs at 70% of the font size (Blink's synthesis
    /// ratio); true capitals and caseless chars keep the full size.
    pub font_variant_caps: Option<bool>,
    /// Inherited: `text-transform` case mapping applied to the rendered text
    /// runs — measure and paint both consume the same transformed string,
    /// while the DOM keeps the original (textContent is untouched, Chrome's
    /// contract). `None` = unset (inherits as `none`); a declared `None`
    /// variant resets an inherited transform.
    pub text_transform: Option<TextTransform>,
    /// Queryable-container marker + name (css-conditional-5). Marker only —
    /// no containment behavior is modeled.
    pub container_type: ContainerType,
    pub container_name: Option<String>,
    /// Inherited: `nowrap` disables wrapping for inline runs inside this box.
    /// `None` = `normal`.
    pub white_space: Option<WhiteSpace>,
    /// Inherited, attribute-derived (#139): the nearest ancestor's
    /// `lang`/`xml:lang` attribute. Not a cascade property — author rules
    /// never write it; the text layer maps it to the run's Han-unification
    /// font slot (`diting_layout::text::han`).
    pub lang: Option<String>,
    /// `text-overflow` (non-inherited): the marker a clipping box draws over
    /// overflowing inline content. `None` = `clip`.
    pub text_overflow: Option<TextOverflow>,
    /// `line-clamp` / `-webkit-line-clamp` (non-inherited, both names write
    /// this slot): keep at most N line boxes, ellipsize the Nth. Count-only
    /// by design (takumi #1965): the clamp never consults the box's content
    /// height, so N lines that sum a hair past a rounded height all stay.
    /// `None` = `none`. Chrome only HONORS the clamp on the legacy
    /// `-webkit-box` display with vertical orient — see [`Self::clamp_lines`].
    pub line_clamp: Option<u32>,
    /// `display: -webkit-box` / `-webkit-inline-box` (#275), non-inherited:
    /// which legacy-flexbox display the cascade declared. Layout maps both
    /// to plain Block/InlineBlock geometry, but the flag arms the clamp.
    pub webkit_box: Option<WebkitBoxKind>,
    /// `-webkit-box-orient: vertical` (or its `block-axis` synonym), parsed
    /// independently of `display` so declaration order never matters — the
    /// clamp gate reads the conjunction at layout time.
    pub webkit_box_vertical: bool,
    pub text_align: Option<TextAlign>,
    /// Inline base direction (#188-3, blitz#981): `direction: ltr|rtl`,
    /// inherited; the `dir` attribute hint fills the same slot below author
    /// declarations. `None` = ltr (the initial value), never "unset" —
    /// an inherited rtl survives as Some(Rtl) down the subtree.
    pub direction: Option<TextDirection>,
    /// Per-axis overflow (css-overflow-3). `None` = not declared (visible
    /// initial); the `overflow` shorthand writes both axes, so it clobbers
    /// prior longhands the way a real shorthand does. The §3.1 pair rule
    /// (visible + non-visible coerces the visible side to auto) applies on
    /// read in [`ComputedStyle::resolved_overflow`].
    pub overflow_x: Option<Overflow>,
    pub overflow_y: Option<Overflow>,
    /// `text-align-last` (#214, blitz#998), inherited; None = the initial
    /// `auto` (defer to text-align). Paint consumes it via
    /// [`ComputedStyle::physical_text_align_last`] as a per-run last-line
    /// offset.
    pub text_align_last: Option<TextAlignLast>,
    // --- flex/grid pass-through (batch 2c): px/fr-only, non-inherited ---
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrapMode>,
    /// Real flex/grid alignment — separate from `text_align`, which only
    /// addresses inline content (upstream keeps them distinct too).
    pub justify_content: Option<JustifyMode>,
    pub align_items: Option<AlignMode>,
    /// `align-content` — cross-axis distribution of wrapped lines (flex) or
    /// tracks (grid). Mapped only for flex/grid containers, where Chrome
    /// applies it; block containers ignore it. `None` = unset (`normal`,
    /// which computes stretch-like on flex/grid) — only an author
    /// declaration yields a keyword.
    pub align_content: Option<AlignContentMode>,
    /// Item-side self-alignment (#188-2, blitz#977): `align-self` and
    /// `justify-self` on the item. `None` = auto/normal/unset — defer to the
    /// container's items-level default. Consumed by the out-of-flow
    /// static-position pass (an abspos box is "the sole item of its flow
    /// parent", so its own self-alignment resolves there); in-flow items
    /// ride the container's `align_items`/`justify_content` as before.
    pub align_self: Option<AlignMode>,
    pub justify_self: Option<JustifyMode>,
    /// `justify-items` on a grid container — the inline-axis default an
    /// item's `justify-self: auto` falls back to. Flexbox ignores it.
    pub justify_items: Option<JustifyMode>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    /// Length-percentage (`auto` stays None — the initial value).
    pub flex_basis: Option<Length>,
    pub column_gap: Option<Length>,
    pub row_gap: Option<Length>,
    /// Track list (px / rem / fr / auto / minmax). `None` = not declared.
    pub grid_template_columns: Option<Vec<GridTrack>>,
    pub grid_template_rows: Option<Vec<GridTrack>>,
    /// `grid-template-areas` cell matrix, rows of names (`.` = null cell).
    /// Non-rectangular declarations are dropped — the spec makes them
    /// invalid — so the layout side can trust the shape.
    pub grid_template_areas: Option<Vec<Vec<String>>>,
    /// Named area an item is placed into (`grid-area: <name>`); resolves to
    /// the area's implicit `<name>-start`/`<name>-end` grid lines.
    pub grid_area: Option<String>,
    // --- positioning + size clamps (batch 2d), non-inherited ---
    pub position: Option<PositionMode>,
    /// Inset offsets (top/right/bottom/left): px or %.
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_height: Option<Length>,
    /// Declared aspect ratio (width/height); `auto` stays None.
    pub aspect_ratio: Option<f32>,
    /// Replaced-content fit (batch 5c), non-inherited; initial Fill.
    pub object_fit: Option<ObjectFit>,
    /// (x, y) object-position parts, non-inherited; initial 50%/50%.
    pub object_position: Option<(ObjectPositionPart, ObjectPositionPart)>,
    /// `z-index` (batch 6a), non-inherited; None = auto. Only meaningful on
    /// positioned elements (flex/grid-item support is a later batch).
    pub z_index: Option<i32>,
    /// `zoom` (#120): the element's computed zoom factor — declared here or
    /// inherited (Chrome makes zoom an inherited property); `None` = the
    /// initial 1. Non-standard, but Tmall-class publish UIs size their whole
    /// panel in design coordinates and set `zoom: viewport/1280` on the
    /// wrapper; without honoring it every box renders at design size and the
    /// form overflows the viewport.
    pub zoom: Option<f32>,
    /// The zoom THIS element declares (never inherited; `None` = no
    /// declaration of its own). The accumulated factor a subtree renders
    /// at is the ancestors' product times this — NOT `zoom` above, which
    /// already carries the inherited value and would double-count it.
    pub zoom_declared: Option<f32>,
    /// Accumulated zoom factor for THIS node (ancestors' product × own),
    /// filled by compute_styles' tree walk — 1.0 outside zoom subtrees.
    /// Geometry px in this ComputedStyle are ALREADY scaled by it (the same
    /// walk), so layout/paint read px as-is; only sizes that never pass
    /// through the cascade (replaced-element naturals) multiply by it
    /// themselves.
    pub zoom_accum: f32,
    /// `opacity` (animation batch A), non-inherited; None = 1 (the initial
    /// value) so untouched subtrees skip alpha work entirely. The paint pass
    /// multiplies it into everything the element's subtree emits — color
    /// alphas for Bg/Border/Text, an explicit item alpha for Image/Svg/
    /// Replaced — which matches Chrome's group compositing for fades.
    pub opacity: Option<f32>,
    /// `transform` function list (animation batch B; obscura #740 lineage,
    /// widened in the affine batch): translate/translate3d/translateX/
    /// translateY, scale/scaleX/scaleY, rotate/skewX/skewY and matrix
    /// composed into one 2D affine. Percentages resolve against the
    /// element's own border-box size at collect time; the map moves paint,
    /// gBCR and hit testing but NOT the layout box (Chrome semantics:
    /// transforms never affect layout).
    pub transform: Option<Transform2D>,
    /// CSS Transforms L2 individual `translate` property (#140): (x, y) with
    /// px/percent/0 slots — percent resolves against the element's own border
    /// box at collect time, same as the transform-list translate. v1 is 2D:
    /// a third (z) component invalidates the declaration.
    pub translate_prop: Option<(Length, Length)>,
    /// Individual `rotate` property (#140): Z-axis angle in degrees. 3D axis
    /// forms (`rotate: x 45deg`) are dropped — same 2D posture as the
    /// transform list.
    pub rotate_prop: Option<f32>,
    /// Individual `scale` property (#140): (x, y); `scale: 2` is (2, 2).
    pub scale_prop: Option<(f32, f32)>,
    /// `transform-origin` (#244): (x, y), keywords normalized to percentages
    /// at parse (left/top → 0%, center → 50%, right/bottom → 100%), a bare x
    /// defaults y to center. Percentages resolve against the element's own
    /// border box at collect time; the pivot default is the box center
    /// (50% 50% — the pre-#244 hardcoded behavior). The z component stays
    /// unparsed — same 2D posture as the transform list.
    pub transform_origin: Option<(Length, Length)>,
    /// Parsed `animation` shorthand (CSS animations batch). The cascade
    /// only stores it; `sample_css_animation` resolves it against the
    /// stylesheet's `@keyframes` table at a time the caller supplies, then
    /// overrides opacity/transform/stroke-dashoffset in place.
    pub animation: Option<AnimationSpec>,
    /// Parsed `transition` shorthand (CSS transitions batch). Like the
    /// animation shorthand the cascade only stores it; the JS face diffs a
    /// tracked property and registers a `CssTransition` with the engine,
    /// and `sample_css_transitions` resolves registered entries against the
    /// virtual clock. Comma lists collapse to the first entry.
    pub transition: Option<TransitionSpec>,
    /// Uniform circular `border-radius` (batch 6b): ONE length/percentage
    /// applied to all four corners (the 1-value syntax — by far the most
    /// common form). Percentages resolve against the box width. Per-corner
    /// and elliptical (`rx ry`) radii are a later batch.
    pub border_radius: Option<Length>,
    /// Per-corner radii (batch 7c), CSS corner order (top-left, top-right,
    /// bottom-right, bottom-left), each an (rx, ry) pair — rx resolves
    /// against the box width, ry against its height (the elliptical form).
    /// `None` when no border-radius is declared; the uniform 1-value case
    /// fills all four pairs identically.
    pub corner_radii: Option<[(Length, Length); 4]>,
    /// `float` (batch 8a), non-inherited; None = none (the initial value).
    pub float_side: Option<FloatSide>,
    /// `clear`, non-inherited; None = none.
    pub clear_side: Option<ClearSide>,
    /// `background-image` longhand, kept as the raw (already var()-substituted)
    /// token stream — gradients/urls don't feed layout, so there is no parsed
    /// form; this exists so the CSSOM reports author-set images instead of
    /// the `none` initial. The `background` shorthand does NOT fill it (v1).
    pub background_image: Option<String>,
    /// `mask-image`, same raw-token posture as `background_image` (#22): the
    /// paint slice doesn't rasterize masks, the field exists so the CSSOM
    /// reports the computed spelling (stops re-serialized #17-style) instead
    /// of echoing the declared inline text. The `mask` shorthand fills it
    /// (gradient tokens only, like `background`).
    pub mask_image: Option<String>,
    /// `clip-path`, raw-token posture again (#24): no clip rasterization
    /// behind it, the field exists so the CSSOM reports the declared shape
    /// and answers the `none` initial instead of the echo's empty string.
    pub clip_path: Option<String>,
    /// `font-family`, raw author token stream (same posture as
    /// `background_image`): this slice resolves fonts at raster time from
    /// the default stack, so there is no parsed form — the field exists so
    /// the CSSOM reports the author's stack (inherited, like the spec) and
    /// downstream exporters can name real typefaces.
    pub font_family: Option<String>,
    /// `border-collapse` (table layout): collapse = adjacent cell borders
    /// merge (we realize this as zero cell gaps), separate = the HTML
    /// default 2px `border-spacing`. `None` = not declared (separate).
    pub border_collapse: Option<BorderCollapse>,
    /// `table-layout` (table layout): `fixed` = column widths come from
    /// authored sources only (colgroup/col attributes, first-row cell
    /// widths) and later-row content never widens a column; `auto` = the
    /// initial content-measured layout. `None` = not declared (auto).
    pub table_layout: Option<TableLayout>,
    /// `list-style-type`, INHERITED (every browser UA sheet declares it on
    /// the list containers and li picks it up through inheritance). `None`
    /// = nothing declared anywhere — the CSS initial `disc` applies at the
    /// marker use site. `Some(None)` is the author reset (`list-style:
    /// none`), which suppresses the marker.
    pub list_style_type: Option<ListStyleType>,
    /// `vertical-align` on table cells (blitz#508); None = not declared
    /// (the UA middle default applies at the cell alignment site). The
    /// valign attribute feeds the same slot as a presentational hint, so
    /// an author declaration outranks the attribute by construction.
    pub vertical_align: Option<VerticalAlign>,
    /// `text-decoration-line` declared on THIS element (never inherited —
    /// propagation to inline descendants happens at paint-collect time).
    /// `Some(empty set)` is meaningful: `text-decoration: none` on a u/s/a
    /// clears the UA decoration.
    pub text_decoration_line: Option<TextDecorations>,
    /// Custom properties (`--*`), which DO inherit: the var() substitution
    /// source. Values are stored raw (author tokens) — !important stripped at
    /// insertion; resolution to colors/lengths happens at use sites. The map
    /// is Arc-shared: inheritance bumps the refcount instead of deep-copying
    /// (a 500-var design system made that copy the dominant cost of a
    /// whole-document style pass), and a re-declaring element clones-on-write
    /// via `Arc::make_mut` at the declaration site.
    pub custom: std::sync::Arc<std::collections::HashMap<String, String>>,
    /// `content` (generated content batch): only pseudo boxes read it.
    /// `attr()` stays unresolved here — the host's attributes live outside
    /// this module — and resolves at the cascade's visit site.
    pub content: Option<ContentValue>,
    /// The flattened text of `content` after the cascade's visit resolved
    /// counters/quotes/attr() against the walk state — the box builder's
    /// face. The symbolic `content` above stays untouched for the CSSOM
    /// computed-value face (#19: Chrome's gCS ::before serves
    /// `counter(c) ". "` while the rendered box shows `2. `).
    pub content_resolved: Option<String>,
    /// `counter-reset: <name> [<int>]? ...` — non-inherited; empty vector is
    /// also what `none` parses to.
    pub counter_reset: Vec<(String, i32)>,
    /// `counter-increment: <name> [<int>]? ...` — defaults to +1 per name.
    pub counter_increment: Vec<(String, i32)>,
    /// `quotes: <pair>+` flattened `[open, close, open, close, ...]` —
    /// inherited. `None` = the UA default pair; `Some(empty)` = `quotes:
    /// none` (open-quote/close-quote render nothing).
    pub quotes: Option<Vec<String>>,
    /// Generated-content pseudos keyed on the HOST element (the cascade's
    /// visit fills them; layout synthesizes the boxes). `None` = no matching
    /// pseudo rule — the overwhelmingly common case, zero cost.
    pub pseudos: Option<Box<PseudoPair>>,
}

impl ComputedStyle {
    /// The line-clamp face Chrome actually ships (#275, CDP-pinned on 155):
    /// `-webkit-line-clamp` only acts on the legacy `-webkit-box` display
    /// with `-webkit-box-orient: vertical` — plain blocks, flex containers
    /// and horizontal boxes all ignore the clamp, and `overflow` plays no
    /// part. Returns the clamped line count when that face is armed.
    pub fn clamp_lines(&self) -> Option<u32> {
        self.line_clamp
            .filter(|_| self.webkit_box.is_some() && self.webkit_box_vertical)
    }

    /// css-overflow-3 §3.1 pair rule: when one axis is `visible` and the
    /// other is not, the visible side computes to `auto`. Chrome reports the
    /// coerced pair (body{overflow-x:hidden} → y reads "auto"), so the
    /// getComputedStyle face and the internal consumers both read through
    /// this.
    pub fn resolved_overflow(&self) -> (Overflow, Overflow) {
        let x = self.overflow_x.unwrap_or(Overflow::Visible);
        let y = self.overflow_y.unwrap_or(Overflow::Visible);
        match (x, y) {
            (Overflow::Visible, o) if o != Overflow::Visible => (Overflow::Auto, o),
            (o, Overflow::Visible) if o != Overflow::Visible => (o, Overflow::Auto),
            pair => pair,
        }
    }

    /// The single-value merge the coarse internal consumers classify with
    /// (any-axis clip, viewport propagation): the strongest axis wins
    /// (scroll > auto > hidden > clip > visible).
    pub fn effective_overflow(&self) -> Overflow {
        let (x, y) = self.resolved_overflow();
        let rank = |o: Overflow| match o {
            Overflow::Scroll => 4,
            Overflow::Auto => 3,
            Overflow::Hidden => 2,
            Overflow::Clip => 1,
            Overflow::Visible => 0,
        };
        if rank(x) >= rank(y) {
            x
        } else {
            y
        }
    }

    /// True when either axis clips descendants' paint (the paint clip gates
    /// read through this).
    pub fn clips_descendants(&self) -> bool {
        let (x, y) = self.resolved_overflow();
        x != Overflow::Visible || y != Overflow::Visible
    }
}

/// The final transform of an element folds the three CSS Transforms L2
/// individual properties around the `transform` list (#140):
/// `translate ∘ rotate ∘ scale ∘ transform` — the individual properties are
/// applied AFTER the list, exactly the spec's concatenation order.
///
/// Composes outward from the list's affine (each individual property wraps
/// the accumulated matrix): scale multiplies the linear part and the slots
/// stay symbolic; rotate mixes the slots' axes, so a percent slot — which
/// can't ride a rotated map — makes the whole composition give up on the
/// individual properties and keep the plain list (a rare combination; the
/// honest alternative is dropping the declaration, which is worse).
/// translate lands outermost and just adds.
impl ComputedStyle {
    /// Consume-time physical alignment (#188-3, blitz#981): start/end —
    /// and the undeclared initial, which IS start (css-text-3) — resolve
    /// against the element's own base direction. The computed style keeps
    /// the logical keyword (Chrome reports start/end verbatim); only
    /// layout's consume sites (to_taffy_style) call this.
    pub fn physical_text_align(&self) -> Option<TextAlign> {
        let rtl = self.direction == Some(TextDirection::Rtl);
        match self.text_align {
            Some(TextAlign::Start) | None => Some(if rtl {
                TextAlign::Right
            } else {
                TextAlign::Left
            }),
            Some(TextAlign::End) => Some(if rtl {
                TextAlign::Left
            } else {
                TextAlign::Right
            }),
            other => other,
        }
    }

    /// Consume-time physical LAST-line alignment (#214, blitz#998): `auto`
    /// — the initial — defers to `text-align` (no override, so None);
    /// start/end resolve against direction like [`Self::physical_text_align`].
    /// `justify` consumes as None too: there is no stretch machinery to
    /// justify the last line against (the computed face still reports the
    /// keyword verbatim).
    pub fn physical_text_align_last(&self) -> Option<TextAlign> {
        let rtl = self.direction == Some(TextDirection::Rtl);
        match self.text_align_last {
            None | Some(TextAlignLast::Auto) | Some(TextAlignLast::Justify) => None,
            Some(TextAlignLast::Left) => Some(TextAlign::Left),
            Some(TextAlignLast::Right) => Some(TextAlign::Right),
            Some(TextAlignLast::Center) => Some(TextAlign::Center),
            Some(TextAlignLast::Start) => Some(if rtl {
                TextAlign::Right
            } else {
                TextAlign::Left
            }),
            Some(TextAlignLast::End) => Some(if rtl {
                TextAlign::Left
            } else {
                TextAlign::Right
            }),
        }
    }

    pub fn effective_transform(&self) -> Option<Transform2D> {
        let has_ind = self.translate_prop.is_some()
            || self.rotate_prop.is_some()
            || self.scale_prop.is_some();
        let mut m = match self.transform {
            Some(t) => t,
            None if !has_ind => return None,
            None => Transform2D {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: Length::Px(0.0),
                ty: Length::Px(0.0),
            },
        };
        if let Some((sx, sy)) = self.scale_prop {
            m.a *= sx;
            m.c *= sx;
            m.b *= sy;
            m.d *= sy;
            m.tx = mul_len(m.tx, sx);
            m.ty = mul_len(m.ty, sy);
        }
        if let Some(deg) = self.rotate_prop {
            let rad = deg.to_radians();
            let (sin, cos) = rad.sin_cos();
            // Percent slots can't ride a rotated axis map (add_len would mix
            // units) — the rare rotate+percent-translate combination keeps
            // the plain transform list instead of dropping everything.
            let (tx_px, ty_px) = match (m.tx, m.ty) {
                (Length::Px(x), Length::Px(y)) => (x, y),
                _ => return self.transform,
            };
            let (a, b, c, d) = (m.a, m.b, m.c, m.d);
            m.a = cos * a - sin * b;
            m.b = sin * a + cos * b;
            m.c = cos * c - sin * d;
            m.d = sin * c + cos * d;
            m.tx = Length::Px(cos * tx_px - sin * ty_px);
            m.ty = Length::Px(sin * tx_px + cos * ty_px);
        }
        if let Some((tx, ty)) = self.translate_prop {
            // Outermost: adds without passing through the linear part.
            // Mixed-unit slots (list px + property percent) can't compose —
            // same give-up posture as the rotate arm.
            match (add_len(m.tx, tx), add_len(m.ty, ty)) {
                (Some(x), Some(y)) => {
                    m.tx = x;
                    m.ty = y;
                }
                _ => return self.transform,
            }
        }
        Some(m)
    }
}
