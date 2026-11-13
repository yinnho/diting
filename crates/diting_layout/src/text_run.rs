//! Measured text leaves and the inline-run helpers that build them.
//!
//! Split from the layout hub. Callers still see these names on the crate
//! root. Pure move.

use super::*;

/// CJK classification for the tokenizer (per-glyph line breaks) — kept from
/// the pre-3a deterministic metrics era; `is_cjk` no longer feeds widths.
pub(super) fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F        // Hangul Jamo
        | 0x2E80..=0x9FFF      // CJK Radicals .. CJK Unified
        | 0xA960..=0xA97F      // Hangul Extended-A
        | 0xAC00..=0xD7FF      // Hangul Syllables + Compatibility
        | 0xF900..=0xFAFF      // CJK Compatibility Ideographs
        | 0xFE30..=0xFE4F      // CJK Compatibility Forms
        | 0xFF00..=0xFF60      // Fullwidth Forms
        | 0xFFE0..=0xFFE6      // Fullwidth Signs
    )
}

/// Node context for measured taffy leaves (batch 3a). A pure-text run is ONE
/// leaf measured the way blitz measures its parley text nodes — see
/// [`measure_text_leaf`]; mixed runs (text + inline elements) keep the
/// flex-row-of-word-leaves fallback from batch 2b.
#[derive(Clone)]
pub(super) enum TextLeaf {
    Run {
        text: String,
        font_size: f32,
        bold: bool,
        color: [u8; 4],
        line_height: f32,
        decorations: TextDecorations,
        baseline_shift: f32,
        mono: bool,
        word_spacing: f32,
        /// The run's white-space mode (nowrap plus the pre family):
        /// `no_soft_wrap()` drops the wrap width (one line, max-content;
        /// scrollWidth grows for free), paint wraps at +inf; `pre`/
        /// `pre-line` add hard breaks, `pre*` preserve space runs.
        ws: WhiteSpace,
        /// This run is an inline ELEMENT's box (its leaves were hoisted
        /// from one), not a bare text run. Beside a float the two differ
        /// (#313): bare nowrap/pre text overflows its band in place, an
        /// unbreakable element box too wide for the band drops below the
        /// float (float_lines walks it past the segment).
        atomic: bool,
        /// The run's physical `text-align` (inherited from the owning
        /// block). Line alignment normally rides the block→flex promote
        /// (style_map); the float wrap path keeps its host a block
        /// container — floats only exist in taffy's block algorithm — so
        /// there the lines center/right-shift inside their float slots
        /// instead (`float_lines`, #313).
        align: TextAlign,
        /// `text-overflow: ellipsis` on the owning element (non-inherited,
        /// so captured from the block that owns the run). Only acted on
        /// together with `nowrap` at paint time — layout rects, scroll
        /// extents and selection keep the full text, per spec.
        ellipsis: bool,
        /// `line-clamp`/`-webkit-line-clamp` on the owning element (#271,
        /// non-inherited): at most N line boxes. Measure clamps its line
        /// COUNT (count-only, takumi #1965 — never the content height);
        /// the paint item carries the clamped token stream so every
        /// re-wrap path reproduces the same N lines.
        line_clamp: Option<u32>,
        /// Shaped wrap tokens, memoized per leaf (obscura#983's measure
        /// half): taffy probes a run leaf several times per solve
        /// (min/max-content, definite widths, repair passes) and shaping
        /// used to re-run on every probe. Tokens are a pure function of
        /// text+style, which the leaf owns for its whole lifetime — shape
        /// once via [`run_tokens`], clone the Rc on every later probe.
        tokens: std::cell::RefCell<Option<std::rc::Rc<[text::Token]>>>,
        /// `font-variant-caps: small-caps` synthesis: lowercase runs shape
        /// as uppercase glyphs at 70% size (case-run token segmentation).
        small_caps: bool,
        /// #139 (Han unification): the run's Han slot from its lang chain —
        /// unified ideographs route to the first same-slot face (see
        /// `text::han`). `None` = plain cascade.
        han: Option<text::HanSlot>,
        /// #218: source DOM text node — flattened spans own no box; the clip chain walks from it.
        clip_src: Option<NodeId>,
    },
    /// One word/glyph of a MIXED run (text around inline elements): the
    /// batch-2b word-leaf fallback, now carrying paint context (batch 4d).
    /// Layout is still style-driven — the measure closure passes Word
    /// leaves straight through to taffy's own style sizing.
    Word {
        text: String,
        font_size: f32,
        bold: bool,
        color: [u8; 4],
        line_height: f32,
        decorations: TextDecorations,
        baseline_shift: f32,
        mono: bool,
        han: Option<text::HanSlot>,
    },
    /// An inline-level replaced atom rides the strut (CSS: the strut is a
    /// zero-width glyph of the container's font, so the LINE box spans its
    /// full ascent+descent): the leaf grows by the surrounding font's
    /// descent, and collect shrinks the recorded rect back to the element
    /// box — gBCR/offsetHeight stay element-sized while the container line
    /// gains the descent below the atom.
    Replaced { strut_descent: f32 },
    /// An EMPTY-content pseudo box (::before/::after with `content:""` —
    /// background-only decoration: overlays, bars, clearfixes). The leaf
    /// has no DOM id of its own, so it carries its HOST's DOM id plus
    /// which pseudo it is; the paint walk claims the pseudo's own
    /// background layers from the host's styles entry (#172).
    Pseudo { host: NodeId, before: bool },
}

/// Used line height for a text run: the element's declared `line-height`
/// (unitless multiplier against its own font-size, or absolute px).
/// `normal`/unset derives from the face's vertical metrics —
/// ascent+descent+line gap — like a real browser, not a flat 1.2×
/// (#16, blitz#878; the ratio is memoized inside the FontBook).
pub(super) fn effective_line_height(
    fonts: &FontBook,
    spec: Option<&diting_css::LineHeightSpec>,
    font_size: f32,
    bold: bool,
) -> f32 {
    match spec {
        Some(diting_css::LineHeightSpec::Number(n)) => font_size * n,
        Some(diting_css::LineHeightSpec::Px(px)) => *px,
        Some(diting_css::LineHeightSpec::Normal) | None => {
            fonts.normal_line_height(font_size, bold)
        }
    }
}

/// Whether `cid` acts as an in-flow block-level ELEMENT child toward its
/// parent — the trigger half of the block-in-inline split (CSS2.1 §9.2.1.1).
/// An inline child that itself carries a block child counts too: it
/// blockifies (see [`blockifies_for_block_child`]) and presents block-level
/// outward — how the spec's split propagates through nested inlines.
/// Replaced children never trigger — they stay atomic run members either
/// way (Chrome does not split an inline around
/// an inline-level replaced box). Floats and out-of-flow boxes are
/// excluded v1: they ride the float-zone/reparent machinery, not the split.
pub(super) fn block_level_child(
    tree: &DomTree,
    cid: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> bool {
    if tree.with_node(cid, |n| n.is_text()).unwrap_or(false) {
        return false;
    }
    let tag = tree
        .with_node(cid, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    if is_replaced_tag(&tag) {
        return false;
    }
    let Some(cs) = styles.get(&cid) else {
        return false;
    };
    if matches!(
        cs.position,
        Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
    ) || cs.float_side.is_some()
    {
        return false;
    }
    match cs.display {
        // Unset display follows the same Block default the attach sites use.
        None
        | Some(CssDisplay::Block)
        | Some(CssDisplay::Flex)
        | Some(CssDisplay::Grid)
        | Some(CssDisplay::Table) => true,
        Some(CssDisplay::Inline) => blockifies_for_block_child(tree, cid, styles),
        _ => false,
    }
}

/// Whether an inline element has an in-flow block-level element child —
/// the block-in-inline shape. The spec answer splits the inline into
/// anonymous blocks around it; our approximation blockifies the element
/// whole (element granularity, one box). Left as the IFC flex-row
/// stand-in, the block child becomes a shrink-to-fit ROW item: the
/// shobserver export pages (#186) had ARTICLE inside inline A#source
/// measure itself to 1092px inside a 740px .container and clip the
/// article's right edge off-viewport.
pub(super) fn blockifies_for_block_child(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> bool {
    if styles.get(&id).and_then(|s| s.display) != Some(CssDisplay::Inline) {
        return false;
    }
    render_children(tree, id)
        .into_iter()
        .any(|cid| block_level_child(tree, cid, styles))
}

/// The #220 flex-basis content-box carry-over. flex-basis measures along
/// the PARENT's main axis, but to_taffy_style only sees the item's own
/// style — its carry keyed on the item's flex_direction, which is Row for
/// a plain item (the default), so items in a column container carried
/// their LEFT/RIGHT padding onto the basis while Chrome adds top/bottom.
/// One-value shorthand padding masked it (uniform sides carry the same
/// sum either way); longhand or multi-value padding exposed it (basis 50 +
/// padding-top 40 measured 50, and 40+20 measured 60 — the latter being
/// taffy clamping the border-box basis up to the padding sum, not a
/// carry). Runs at the flex parent's tail, where the parent's axis and
/// the final child list are both known; border-box items already measure
/// border-edge and synthetic children (no node_map entry) are skipped.
pub(super) fn fix_flex_basis_carry(
    taffy_tree: &mut TaffyTree<TextLeaf>,
    parent: &ComputedStyle,
    styles: &HashMap<NodeId, ComputedStyle>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    children: &[diting_taffy::tree::NodeId],
) {
    if parent.display != Some(CssDisplay::Flex) {
        return;
    }
    let vertical = matches!(
        parent.flex_direction,
        Some(diting_css::FlexDirection::Column | diting_css::FlexDirection::ColumnReverse)
    );
    for &child in children {
        let Some(&dom) = node_map.get(&child) else {
            continue;
        };
        let Some(cs) = styles.get(&dom) else { continue };
        let Some(diting_css::Length::Px(px)) = cs.flex_basis else {
            continue;
        };
        if matches!(cs.box_sizing, Some(diting_css::BoxSizing::BorderBox)) {
            continue;
        }
        let bline = cs.border_style.is_some();
        let bw = |v: Option<diting_css::Length>| if bline { side_px(v) } else { 0.0 };
        let carry = if vertical {
            side_px(cs.padding.top)
                + side_px(cs.padding.bottom)
                + bw(cs.border_width.top)
                + bw(cs.border_width.bottom)
        } else {
            side_px(cs.padding.left)
                + side_px(cs.padding.right)
                + bw(cs.border_width.left)
                + bw(cs.border_width.right)
        };
        if carry == 0.0 {
            continue;
        }
        if let Ok(mut ts) = taffy_tree.style(child).cloned() {
            ts.flex_basis = Dimension::length(px + carry);
            let _ = taffy_tree.set_style(child, ts);
        }
    }
}

/// CSS BG3 §5.4 corner-overlap rule: when adjacent corner radii sum past
/// their shared edge, ALL radii scale down by the tightest ratio (Chrome's
/// behavior) — an unclamped 999px pill radius otherwise paints a stray arc
/// far above the box (slides-deck probe). Corner order is [TL, TR, BR, BL]
/// (diting_css border-radius expand). The ratio is invariant under diagonal
/// affine scaling, so clamping pre- or post-prebake is equivalent.
pub(super) fn clamp_corner_radii(radii: [(f32, f32); 4], w: f32, h: f32) -> [(f32, f32); 4] {
    let mut f = 1.0f32;
    let edges = [
        (w, radii[0].0 + radii[1].0), // top: TL.x + TR.x
        (w, radii[3].0 + radii[2].0), // bottom: BL.x + BR.x
        (h, radii[0].1 + radii[3].1), // left: TL.y + BL.y
        (h, radii[1].1 + radii[2].1), // right: TR.y + BR.y
    ];
    for (edge, sum) in edges {
        if sum > 0.0 {
            f = f.min(edge / sum);
        }
    }
    if f >= 1.0 {
        return radii;
    }
    radii.map(|(rx, ry)| (rx * f, ry * f))
}

/// Effective font context for a text leaf: nearest ancestor's font-size /
/// weight / line-height spec (defaults 16px / 400 / normal). Since batch 2e
/// every cascaded element carries a resolved font-size, so the walk stops at
/// the NEAREST value instead of relying on outer ancestors being None.
/// Returns (font_size, bold, used_line_height): a Number spec multiplies the
/// RUN's font-size (CSS computed-value semantics — the multiplier applies to
/// whichever font-size the text actually uses), px passes through absolute.
pub(super) fn font_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
) -> (f32, bool, f32) {
    let mut font_size: Option<f32> = None;
    let mut bold: Option<bool> = None;
    let mut lh_spec: Option<diting_css::LineHeightSpec> = None;
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(style) = styles.get(&nid) {
            if font_size.is_none() {
                if let Some(fs) = style.font_size {
                    font_size = Some(fs);
                }
            }
            if bold.is_none() {
                if let Some(fw) = style.font_weight {
                    bold = Some(fw >= 600);
                }
            }
            if lh_spec.is_none() {
                lh_spec = style.line_height;
            }
            if font_size.is_some() && bold.is_some() && lh_spec.is_some() {
                break;
            }
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    let fs = font_size.unwrap_or(16.0);
    let b = bold.unwrap_or(false);
    (fs, b, effective_line_height(fonts, lh_spec.as_ref(), fs, b))
}

/// Inherited text color for a node — the same nearest-set ancestor walk as
/// [`font_context`] (the cascade inherits `color`, but text NODES carry no
/// ComputedStyle, so the paint side resolves it here). Defaults to black.
pub(super) fn color_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> [u8; 4] {
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(c) = styles.get(&nid).and_then(|s| s.color) {
            return [c.0, c.1, c.2, c.3];
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    [0, 0, 0, 255]
}

/// Whether a text node rides the monospace face (mono batch): the same
/// nearest-set ancestor walk as [`color_context`] over `font-family` — the
/// cascade inherits it, but text nodes carry no ComputedStyle. Any family
/// list member naming monospace/ui-monospace selects the face (per-char
/// fallback keeps CJK on the primary pair). Defaults to false.
pub(super) fn mono_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> bool {
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(family) = styles.get(&nid).and_then(|s| s.font_family.as_deref()) {
            return diting_css::wants_monospace(family);
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    false
}

/// Inherited `word-spacing` for a text node (px): the same nearest-set
/// ancestor walk as [`mono_context`] — the cascade inherits it, but text
/// nodes carry no ComputedStyle of their own. Defaults to 0.0 (`normal`).
pub(super) fn word_spacing_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> f32 {
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(ws) = styles.get(&nid).and_then(|s| s.word_spacing) {
            return ws;
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    0.0
}

/// `font-variant-caps` inherits, so a text node's synthesis flag walks the
/// ancestor chain like word-spacing does (None = unset → no synthesis).
pub(super) fn small_caps_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> bool {
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(sc) = styles.get(&nid).and_then(|s| s.font_variant_caps) {
            return sc;
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    false
}

/// 批240: `text-transform` context (inherited, the `small_caps_context`
/// precedent) — nearest styled ancestor's declared value. A declared
/// `TextTransform::None` resets an inherited transform; no declaration
/// anywhere falls through to no transform.
pub(super) fn text_transform_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> TextTransform {
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(tt) = styles.get(&nid).and_then(|s| s.text_transform) {
            return tt;
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    TextTransform::None
}

/// #139 (Han unification): a text node's Han slot comes from its nearest
/// styled ancestor's resolved `lang` — the cascade already carried the
/// nearest ancestor's lang attribute down (see `cascade_element`), so the
/// FIRST style hit is the answer; non-CJK languages map to None (the plain
/// per-char cascade, the pre-#139 order).
pub(super) fn han_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> Option<text::HanSlot> {
    let mut current = Some(id);
    while let Some(nid) = current {
        if let Some(cs) = styles.get(&nid) {
            return cs.lang.as_deref().and_then(text::han_slot_for_lang);
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    None
}

/// text-decoration resolution for a text node: the property PROPAGATES
/// (CSS 2.1 §16.3.1) rather than inheriting, so every ancestor's declared
/// set unions in — `<u><s>` paints both lines. Propagation stops at an
/// atomic inline (inline-block) or out-of-flow (absolute/fixed/float)
/// boundary: the boundary element's own text still carries its own
/// declaration, but nothing crosses such an element inward OR outward.
pub(super) fn decoration_context(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> TextDecorations {
    let mut out = TextDecorations::default();
    let mut current = Some(id);
    while let Some(nid) = current {
        let boundary = if let Some(s) = styles.get(&nid) {
            if let Some(d) = s.text_decoration_line {
                out = out.union(d);
                // #218: clip:text on the DECLARER pierces the strokes.
                if s.background_clip_text {
                    out.pierce = true;
                }
            }
            nid != id
                && (s.display == Some(CssDisplay::InlineBlock)
                    || matches!(
                        s.position,
                        Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
                    )
                    || s.float_side.is_some())
        } else {
            false
        };
        if boundary {
            break;
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    out
}

/// An own-element decoration set (pseudo leaves): the decorator IS the
/// element, so clip:text on it pierces the strokes (#218).
pub(super) fn own_decorations(p: &ComputedStyle) -> TextDecorations {
    let mut d = p.text_decoration_line.unwrap_or_default();
    if p.background_clip_text {
        d.pierce = true;
    }
    d
}

/// `vertical-align: sub|super` as a baseline shift on inline text (CSS 2.1
/// §10.8.1 keyword model, Chromium constants): sub drops 20% of the PARENT's
/// font-size below the baseline, super lifts 40% above it — the Chrome probes
/// (a 20px-parent `<sup>` and an explicit 20px `super` both grow the line by
/// exactly 8px). Shifts compose up the ancestor chain (nested sup compounds).
/// The walk stops at an atomic-inline (inline-block) or out-of-flow boundary
/// WITHOUT applying the boundary's own declaration — vertical-align on an
/// inline-block moves the BOX. Down-positive: sub → +0.2×parent_fs, super →
/// −0.4×parent_fs; an authored length flips sign (CSS: positive raises); a
/// percentage resolves against the element's own line-height; the baseline/
/// top/middle/bottom keywords are 0 (the line-baseline machinery owns them).
pub(super) fn valign_shift(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
) -> f32 {
    let mut shift = 0.0f32;
    let mut current = Some(id);
    while let Some(nid) = current {
        let boundary = if let Some(s) = styles.get(&nid) {
            if s.display == Some(CssDisplay::Inline) {
                match s.vertical_align {
                    Some(diting_css::VerticalAlign::Sub)
                    | Some(diting_css::VerticalAlign::Super) => {
                        let parent = tree.with_node(nid, |n| n.parent).flatten();
                        let parent_fs = parent
                            .map(|p| font_context(tree, p, styles, fonts).0)
                            .unwrap_or(16.0);
                        shift += if s.vertical_align == Some(diting_css::VerticalAlign::Sub) {
                            0.2 * parent_fs
                        } else {
                            -0.4 * parent_fs
                        };
                    }
                    Some(diting_css::VerticalAlign::Length(px)) => {
                        shift -= px;
                    }
                    Some(diting_css::VerticalAlign::Percent(p)) => {
                        let own_lh = font_context(tree, nid, styles, fonts).2;
                        shift -= p / 100.0 * own_lh;
                    }
                    _ => {}
                }
            }
            nid != id
                && (s.display == Some(CssDisplay::InlineBlock)
                    || matches!(
                        s.position,
                        Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
                    )
                    || s.float_side.is_some())
        } else {
            false
        };
        if boundary {
            break;
        }
        current = tree.with_node(nid, |n| n.parent).flatten();
    }
    shift
}

/// Font descent at a size/weight, FontBook fallback fs×0.2 — the same
/// fallback text_baseline rides.
pub(super) fn leaf_descent(fonts: &FontBook, fs: f32, bold: bool) -> f32 {
    fonts
        .metrics(fs, bold)
        .map(|m| m.descent)
        .unwrap_or(fs * 0.2)
}

/// Extra leaf height a baseline shift needs so the line box contains the
/// shifted glyph: a lift (super) pads by the lift itself; a drop (sub) pads
/// by the drop plus the glyph's descent, which would otherwise poke out the
/// line's bottom. Whole-pixel: taffy's round_layout pass sizes nodes on the
/// cumulative pixel grid, so a fractional pad would be unrecoverable from
/// the rounded height and the error would land on the element's own box.
pub(super) fn shift_pad(shift: f32, descent: f32) -> f32 {
    let raw = if shift < 0.0 {
        -shift
    } else if shift > 0.0 {
        shift + descent
    } else {
        0.0
    };
    raw.ceil()
}

/// The label a select displays: the first option whose selectedness is on
/// — the parsed `selected` attribute, since JS-side dirtiness rides the
/// live_value mirror instead — else the first option, the same precedence
/// the bootstrap's select.value getter resolves with (`opts[i].selected`
/// falls back to the attribute there). An option's display text is its
/// `label` attribute when present, else its content, trimmed.
pub(super) fn selected_option_label(tree: &DomTree, root: NodeId) -> Option<String> {
    // Document order over the subtree: options are children of the select
    // or of optgroups, so an explicit LIFO stack of reversed child lists
    // walks exactly the order querySelectorAll('option') would match.
    let children_rev = |id: NodeId| -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = tree.with_node(id, |n| n.first_child).flatten();
        while let Some(c) = cur {
            out.push(c);
            cur = tree.with_node(c, |n| n.next_sibling).flatten();
        }
        out.reverse();
        out
    };
    let label_of = |id: NodeId| -> String {
        tree.with_node(id, |n| n.get_attribute("label").map(|v| v.to_string()))
            .flatten()
            .unwrap_or_else(|| tree.text_content(id))
            .trim()
            .to_string()
    };
    let mut first: Option<String> = None;
    let mut stack = children_rev(root);
    while let Some(nid) = stack.pop() {
        let is_option = tree
            .with_node(nid, |n| {
                n.as_element().map(|e| e.local.as_str() == "option")
            })
            .flatten()
            .unwrap_or(false);
        if is_option {
            let selected = tree
                .with_node(nid, |n| n.get_attribute("selected").is_some())
                .unwrap_or(false);
            if selected {
                return Some(label_of(nid));
            }
            // The first option stays the fallback even when its label is
            // empty (Chrome shows an empty box then, not option #2).
            if first.is_none() {
                first = Some(label_of(nid));
            }
        } else {
            // Only optgroup (and, in weird markup, anything else) can wrap
            // options; text nodes carry no options.
            stack.extend(children_rev(nid));
        }
    }
    first.filter(|s| !s.is_empty())
}

/// The text run a form control paints inside its replaced box: the dirty
/// value (live_value, mirrored from `el.value = x` by the bootstrap) wins,
/// then the parsed default — value attribute for input, textContent for
/// textarea (the same precedence the JS value getter uses, so what paints
/// is what `el.value` reads). A select paints the label of the option its
/// value getter would report (the bootstrap's value/selectedIndex writes
/// land that label as live_value on the select). An empty text-like
/// control paints its placeholder in Chrome's gray; button-ish inputs
/// label from their value like their sizing does. Checkable inputs draw
/// no run — the widget itself carries the state (see [`FormWidget`]).
/// None for everything else.
pub(super) fn form_control_run(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
) -> Option<(String, f32, bool, f32, [u8; 4])> {
    let tag = tree
        .with_node(id, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    if tag != "input" && tag != "textarea" && tag != "select" {
        return None;
    }
    let (font_size, bold, lh) = font_context(tree, id, styles, fonts);
    let ink = color_context(tree, id, styles);
    let attr = |name: &str| {
        tree.with_node(id, |n| n.get_attribute(name).map(|v| v.to_string()))
            .flatten()
    };
    let live = tree
        .with_node(id, |n| n.live_value().map(|v| v.to_string()))
        .flatten();
    let run = |text: String, color: [u8; 4]| Some((text, font_size, bold, lh, color));
    if tag == "select" {
        let label = live
            .filter(|s| !s.trim().is_empty())
            .or_else(|| selected_option_label(tree, id))?;
        return run(label.trim().to_string(), ink);
    }
    if tag == "textarea" {
        let text = live.unwrap_or_else(|| tree.text_content(id));
        return if text.is_empty() {
            attr("placeholder").and_then(|p| run(p, [117, 117, 117, 255]))
        } else {
            run(text, ink)
        };
    }
    let ty = attr("type").unwrap_or_default().to_ascii_lowercase();
    match ty.as_str() {
        // Checkbox/radio are drawn as widgets; range's value is the thumb
        // position, not text (blitz#456).
        "checkbox" | "radio" | "range" => None,
        "button" | "submit" | "reset" => {
            let label = live
                .or_else(|| attr("value"))
                .filter(|v| !v.is_empty())
                .or_else(|| (ty == "submit").then(|| "Submit".to_string()));
            label.and_then(|t| run(t, ink))
        }
        _ => {
            let text = live.or_else(|| attr("value")).unwrap_or_default();
            if text.is_empty() {
                attr("placeholder").and_then(|p| run(p, [117, 117, 117, 255]))
            } else {
                run(text, ink)
            }
        }
    }
}

/// Split text into layout tokens. Whitespace runs collapse to a single space
/// token (CSS text processing) that keeps its width but contributes no
/// height; CJK chars break per-glyph — UAX#14 allows a break after every
/// ideograph, and without this a CJK paragraph would be one unbreakable
/// "word".
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            // CSS §16.6.1: a whitespace run collapses to ONE space, not one
            // leaf per char — the newline+indent node between inline
            // siblings used to measure as a dozen-plus spaces.
            if !tokens.last().is_some_and(|t| t.trim().is_empty()) {
                tokens.push(" ".to_string());
            }
        } else if is_cjk(ch) {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(ch.to_string());
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

/// white-space-aware tokenizer for the pre family. Three regimes:
/// - collapse (normal/nowrap): the plain [`tokenize`] path, edges trimmed.
/// - pre-line: collapse spaces per newline-separated segment but keep the
///   newlines themselves as hard-break tokens.
/// - preserve (pre/pre-wrap/break-spaces): keep every space and newline; CRLF
///   normalizes to LF first (UA behaviour); tab becomes an 8-space run
///   (default tab-size).
pub(crate) fn tokenize_ws(text: &str, ws: diting_css::WhiteSpace) -> Vec<String> {
    use diting_css::WhiteSpace as Ws;
    match ws {
        Ws::Normal | Ws::Nowrap => tokenize(text.trim()),
        Ws::PreLine => {
            let mut acc = Vec::new();
            let segs: Vec<&str> = text.split('\n').collect();
            for (i, seg) in segs.iter().enumerate() {
                let seg = seg.strip_suffix('\r').unwrap_or(seg);
                acc.append(&mut tokenize(seg.trim()));
                if i + 1 < segs.len() {
                    acc.push("\n".to_string());
                }
            }
            acc
        }
        Ws::Pre | Ws::PreWrap | Ws::BreakSpaces => {
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            let mut tokens = Vec::new();
            let mut word = String::new();
            for ch in text.chars() {
                match ch {
                    '\n' => {
                        if !word.is_empty() {
                            tokens.push(std::mem::take(&mut word));
                        }
                        tokens.push("\n".to_string());
                    }
                    '\t' => {
                        if !word.is_empty() {
                            tokens.push(std::mem::take(&mut word));
                        }
                        tokens.push("        ".to_string());
                    }
                    _ if ch.is_whitespace() => {
                        if !word.is_empty() {
                            tokens.push(std::mem::take(&mut word));
                        }
                        tokens.push(ch.to_string());
                    }
                    _ if is_cjk(ch) => {
                        if !word.is_empty() {
                            tokens.push(std::mem::take(&mut word));
                        }
                        tokens.push(ch.to_string());
                    }
                    _ => word.push(ch),
                }
            }
            if !word.is_empty() {
                tokens.push(word);
            }
            tokens
        }
    }
}

/// Adjacent-sibling margin collapse (CSS 2.1 §8.3.1). Two regimes coexist in
/// the bridge:
///
/// - Taffy's native BLOCK layout already collapses sibling margins to max —
///   but unconditionally, WITHOUT the "border/padding on the touching edges
///   separates the boxes" rule (its `has_styles_preventing_being_collapsed_
///   through` only gates a node collapsing through ITSELF, not the pair).
/// - Flex/grid containers SUM margins.
///
/// This pass runs over the built child list and rewrites the pair so BOTH
/// engines land on the CSS geometry: touching edges clean → encode the
/// collapsed max (zeroing prev.bottom, inflating next.top) for flex/grid
/// parents, and leave block parents' native collapse alone; separated edges
/// → encode the SUM (prev.bottom=0, next.top=a+b), which makes taffy's
/// unconditional block-side max a no-op (max(a+b applied once)) while giving
/// flex/grid the correct sum.
///
/// Out of scope (documented approximations): %-margin pairs (the collapsed
/// max is unknowable at build time), collapse-through of empty
/// self-collapsing blocks, and first/last-child collapse with the parent.
/// Whether this taffy style is the flex-column alignment stand-in for a
/// block's `text-align: center/right` (see `to_taffy_style`). Tables share
/// Column but stretch; rows are Row — both stay outside this predicate.
pub(super) fn is_alignment_promote(s: &Style) -> bool {
    s.display == Display::Flex
        && s.flex_direction == FlexDirection::Column
        && matches!(s.align_items, AlignItems::CENTER | AlignItems::FLEX_END)
}

/// (#169) CSS `text-align` aligns the INLINE content only; block-level
/// children of a centered block still fill the container width (§10.3.3),
/// centering their own inline content through inheritance. The flex-column
/// stand-in instead shrinks EVERY item to content width — on the 1688 punish
/// page `body, p { text-align: center }` collapsed the whole nc-slider chain
/// (`#nocaptcha` … `.nc_scale`) to 0 width. Patch block-level element
/// children with `align_self: STRETCH` before assembly: only real element
/// children carry a node_map entry (runs, float zones, bars and anonymous
/// wrappers don't), and tables/images keep the centered shrink-to-fit Chrome
/// gives them under text-align.
pub(super) fn stretch_block_children_in_aligned_parent(
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    styles: &HashMap<NodeId, ComputedStyle>,
    children: &[diting_taffy::tree::NodeId],
) {
    for &n in children {
        let Some(&dom) = node_map.get(&n) else {
            continue;
        };
        let Some(cs) = styles.get(&dom) else { continue };
        if !matches!(
            cs.display,
            Some(CssDisplay::Block)
                | Some(CssDisplay::Flex)
                | Some(CssDisplay::Grid)
                | Some(CssDisplay::TableCell)
        ) {
            continue;
        }
        if let Ok(mut st) = taffy_tree.style(n).cloned() {
            st.align_self = Some(AlignSelf::STRETCH);
            let _ = taffy_tree.set_style(n, st);
        }
    }
}

pub(super) fn collapse_adjacent_sibling_margins(
    taffy_tree: &mut TaffyTree<TextLeaf>,
    styles: &HashMap<NodeId, ComputedStyle>,
    node_map: &HashMap<diting_taffy::tree::NodeId, NodeId>,
    direct: &[diting_taffy::tree::NodeId],
) {
    let px = |v: Option<diting_css::Length>| match v {
        Some(diting_css::Length::Px(px)) => Some(px),
        _ => None,
    };
    for pair in direct.windows(2) {
        let (prev, next) = (pair[0], pair[1]);
        let (Some(&dprev), Some(&dnext)) = (node_map.get(&prev), node_map.get(&next)) else {
            continue; // run wrapper / synthetic node — not an element pair
        };
        let (Some(sp), Some(sn)) = (styles.get(&dprev), styles.get(&dnext)) else {
            continue;
        };
        // Both in-flow block-level: flex/grid containers, floats, and
        // positioned boxes never collapse (CSS §8.3.1).
        if !matches!(sp.display, Some(CssDisplay::Block) | None)
            || !matches!(sn.display, Some(CssDisplay::Block) | None)
        {
            continue;
        }
        let out_of_flow = |s: &ComputedStyle| {
            matches!(
                s.position,
                Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
            )
        };
        if out_of_flow(sp) || out_of_flow(sn) || sp.float_side.is_some() || sn.float_side.is_some()
        {
            continue;
        }
        let (Some(mp), Some(mn)) = (px(sp.margin.bottom), px(sn.margin.top)) else {
            continue; // % margins: keep the summed gap (approximation above)
        };
        if mp == 0.0 && mn == 0.0 {
            continue;
        }
        // Border/padding on the touching edges separates the boxes: the
        // margins stop adjoining and SUM instead of collapsing to max
        // (CSS §8.3.1). Either way the pair is re-encoded as
        // [prev.bottom=0 | next.top=gap] — see the doc comment for why
        // both engine regimes land on `gap`.
        let separated = (sp.border_style.is_some() && side_px(sp.border_width.bottom) > 0.0)
            || side_px(sp.padding.bottom) > 0.0
            || (sn.border_style.is_some() && side_px(sn.border_width.top) > 0.0)
            || side_px(sn.padding.top) > 0.0;
        let gap = if separated { mp + mn } else { mp.max(mn) };
        if let (Ok(mut prev_style), Ok(mut next_style)) = (
            taffy_tree.style(prev).cloned(),
            taffy_tree.style(next).cloned(),
        ) {
            prev_style.margin.bottom = diting_taffy::style::LengthPercentageAuto::length(0.0);
            next_style.margin.top = diting_taffy::style::LengthPercentageAuto::length(gap);
            let _ = taffy_tree.set_style(prev, prev_style);
            let _ = taffy_tree.set_style(next, next_style);
        }
    }
}

/// One taffy leaf per word (upstream's per-word leaf model). Intrinsic size
/// from real shaped advances (batch 3a); wrapping happens in the enclosing
/// flex-wrap run container. Since batch 4d the leaf carries paint context —
/// mixed runs paint their words.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_word_leaves(
    text: &str,
    font_size: f32,
    bold: bool,
    color: [u8; 4],
    line_height: f32,
    decorations: TextDecorations,
    shift: f32,
    mono: bool,
    word_spacing: f32,
    small_caps: bool,
    han: Option<text::HanSlot>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
) -> Vec<diting_taffy::tree::NodeId> {
    let src_node = text_src::cur();
    let cur_range = std::cell::Cell::new((0u32, 0u32));
    let mut word_leaf = |seg_text: &str, seg_size: f32, is_space: bool| {
        // The separator token's advance carries the inherited
        // word-spacing (same token model measurement uses), so
        // taffy positions the following word leaves accordingly.
        let width = fonts.advance_width(seg_text, seg_size, bold, mono, han)
            + if is_space { word_spacing } else { 0.0 };
        // Pure-whitespace tokens contribute no height (they sit between
        // block siblings without adding a spurious blank row).
        let height = if is_space {
            0.0
        } else {
            line_height + shift_pad(shift, leaf_descent(fonts, seg_size, bold))
        };
        let leaf = TextLeaf::Word {
            text: seg_text.to_string(),
            font_size: seg_size,
            bold,
            color,
            line_height,
            decorations,
            baseline_shift: shift,
            mono,
            han,
        };
        let id = Some(taffy_tree.new_word(width.max(0.0), height.max(0.0), leaf));
        if let (Some(id), Some(node)) = (id, src_node) {
            // The leaf paints one token; the node's chars begin `a` chars
            // before it, so node-local = leaf char + a.
            let (a, _) = cur_range.get();
            text_src::register(id, vec![(node, -(a as i32), text.chars().count() as u32)]);
        }
        id
    };
    let toks = tokenize(text);
    let ranges = if src_node.is_some() {
        text_src::token_ranges(text, &toks)
    } else {
        Vec::new()
    };
    toks.into_iter()
        .enumerate()
        .flat_map(|(ti, token)| {
            cur_range.set(ranges.get(ti).copied().unwrap_or((0, 0)));
            let is_space = token.trim().is_empty();
            if small_caps && !is_space && token.chars().any(|c| c.is_lowercase()) {
                // Synthesized small-caps: lowercase runs render as
                // uppercase glyphs at 70% size, mixed case stays
                // full-size (Blink's model). Each segment is its own
                // leaf so taffy positions them by shaped width.
                text::caps_case_runs(&token)
                    .into_iter()
                    .filter_map(|(seg, lower)| {
                        let seg_size = if lower {
                            font_size * text::SMALL_CAPS_RATIO
                        } else {
                            font_size
                        };
                        // Synthesis uppercases the glyphs (`ß` → "SS");
                        // already-uppercase segments are unaffected.
                        word_leaf(&seg.to_uppercase(), seg_size, false)
                    })
                    .collect::<Vec<_>>()
            } else {
                word_leaf(&token, font_size, is_space)
                    .into_iter()
                    .collect::<Vec<_>>()
            }
        })
        .collect()
}

/// Shaped-token memo accessor for a run leaf: shape once, then hand every
/// later probe a cheap Rc clone (obscura#983's measure half). The leaf owns
/// its text+style for its whole lifetime, so the memo needs no key — unlike
/// upstream, which keys a 16-entry per-item cache by (width, Wrap).
#[allow(clippy::too_many_arguments)]
pub(super) fn run_tokens(
    text: &str,
    font_size: f32,
    bold: bool,
    fonts: &FontBook,
    mono: bool,
    word_spacing: f32,
    ws: WhiteSpace,
    memo: &std::cell::RefCell<Option<std::rc::Rc<[text::Token]>>>,
    small_caps: bool,
    han: Option<text::HanSlot>,
) -> std::rc::Rc<[text::Token]> {
    memo.borrow_mut()
        .get_or_insert_with(|| {
            // #275: a <br> folded into a clamped run rides as U+2028
            // markers. They must never reach tokenize_ws — under Normal
            // the marker is Unicode whitespace and would collapse to a
            // space — so split on it first, tokenize each span under the
            // run's own white-space, and splice zero-width forced-break
            // tokens between (the same is_break the pre family rides).
            if !text.contains('\u{2028}') {
                text::tokens_of(
                    text,
                    font_size,
                    bold,
                    fonts,
                    mono,
                    word_spacing,
                    ws,
                    small_caps,
                    han,
                )
                .into()
            } else {
                let mut out: Vec<text::Token> = Vec::new();
                for (i, piece) in text.split('\u{2028}').enumerate() {
                    if i > 0 {
                        out.push(text::Token {
                            text: "\u{2028}".to_string(),
                            width: 0.0,
                            is_space: false,
                            is_break: true,
                            scale: 1.0,
                            is_ellipsis: false,
                        });
                    }
                    out.extend(text::tokens_of(
                        piece,
                        font_size,
                        bold,
                        fonts,
                        mono,
                        word_spacing,
                        ws,
                        small_caps,
                        han,
                    ));
                }
                out.into()
            }
        })
        .clone()
}

/// Preserve-wins merge for a mixed white-space run: white-space inherits, so
/// one `pre` segment makes the whole run preserve (the same "any nowrap
/// pins the run" precedent, ranked across the family).
pub(super) fn ws_rank(ws: WhiteSpace) -> u8 {
    match ws {
        WhiteSpace::Pre => 5,
        WhiteSpace::BreakSpaces => 4,
        WhiteSpace::PreWrap => 3,
        WhiteSpace::PreLine => 2,
        WhiteSpace::Nowrap => 1,
        WhiteSpace::Normal => 0,
    }
}

/// Taffy measure function for a pure-text run leaf (batch 3a). Reproduces
/// the observable behavior of blitz's parley-measured text nodes:
///
/// - `tokens` arrive pre-shaped (once per leaf, via [`run_tokens`]); an
///   empty slice means the run trims to nothing and measures HIDDEN;
/// - collapsible whitespace at run EDGES contributes nothing (probe:
///   `"hello "` and `" hello"` both measure as `"hello"`, `" "` as zero);
/// - greedy line breaking over exact shaped advances; a space before a
///   break point is dropped (CSS trailing-whitespace removal);
/// - the block's width is `ceil(max line advance)` — parley rounds the text
///   run's size UP so nothing overflows the box, which taffy's own
///   round-to-nearest would not reproduce (probe: "hello" 37.36 → 38);
/// - height = line count × used line-height (blitz pins `normal` at
///   1.2×fs) plus the baseline-shift pad (a sub/sup run grows its line box
///   by exactly the shift's extent).
pub(super) fn measure_text_leaf(
    tokens: &[text::Token],
    line_height: f32,
    pad: f32,
    inputs: &diting_taffy::tree::LayoutInput,
    ws: WhiteSpace,
    line_clamp: Option<u32>,
) -> diting_taffy::tree::LayoutOutput {
    let known = inputs.known_dimensions;
    let lh = line_height;
    if tokens.is_empty() {
        return diting_taffy::tree::LayoutOutput::HIDDEN;
    }
    let widest_token = tokens.iter().map(|t| t.width).fold(0.0, f32::max);

    let wrap_at = if ws.no_soft_wrap() {
        None
    } else {
        match inputs.available_space.width {
            diting_taffy::AvailableSpace::Definite(w) => Some(w),
            _ => None,
        }
    };
    // Greedy wrap over the shared breaker (batch 4a): measure and paint see
    // the same lines by construction.
    let mut lines = text::greedy_wrap(tokens, wrap_at, ws);
    // line-clamp (#271): limit by line COUNT only — never the content
    // height. N lines summing a hair past a rounded box height all stay
    // (takumi #1965's exactly-fits edge); exactly N wrapped lines is not a
    // clamp at all, so a run that fits keeps every line and its height.
    if let Some(n) = line_clamp.filter(|n| lines.len() > *n as usize) {
        lines.truncate(n as usize);
    }
    let min_content = matches!(
        inputs.available_space.width,
        diting_taffy::AvailableSpace::MinContent
    );
    // no-soft-wrap modes remove every soft wrap opportunity (CSS Text §5):
    // min-content rises to the full single line, same as max-content.
    let max_line = if min_content && !ws.no_soft_wrap() {
        widest_token
    } else {
        lines.iter().map(|l| l.width).fold(0.0, f32::max)
    };
    let size = diting_taffy::geometry::Size {
        width: known.width.unwrap_or(max_line.ceil()),
        height: known.height.unwrap_or(lines.len() as f32 * lh + pad),
    };
    // taffy >= 1b918ba replaced the `content_size: Size` second argument
    // with a scrollable-overflow `Rect`; a text leaf's content is exactly
    // its own box, so the rect's extent equals the measured size.
    diting_taffy::tree::LayoutOutput::from_sizes(
        size,
        // normal scroll origins: the leaf overflows only toward the inline end/block end
        diting_taffy::tree::ScrollableOverflow::new(
            diting_taffy::geometry::Rect {
                left: 0.0,
                top: 0.0,
                right: size.width,
                bottom: size.height,
            },
            diting_taffy::geometry::Point { x: false, y: false },
        ),
    )
}

/// The inline-formatting-context stand-in around a run of inline content:
/// a wrapping flex row (upstream run_wrapper_style / outer_style model).
/// `ws` is the white-space in effect on the run's formatting context
/// (#158): a mode with no soft wrap opportunities (nowrap/pre) removes
/// the break between atomic inlines too — the row stops wrapping and its
/// items stop shrinking (a nowrap run overflows its box like Chrome's, it
/// never compresses its atoms to fit).
pub(super) fn run_wrapper_style(ws: WhiteSpace) -> Style {
    Style {
        // With float wrap on, a wrapping row is a block-level box the tree
        // lays out as a flex row (or, beside a float, into the float's slots).
        display: if float_flow::enabled() && !ws.no_soft_wrap() {
            Display::Block
        } else {
            Display::Flex
        },
        flex_direction: FlexDirection::Row,
        flex_wrap: if ws.no_soft_wrap() {
            FlexWrap::NoWrap
        } else {
            FlexWrap::Wrap
        },
        align_items: AlignItems::FLEX_START,
        ..Style::default()
    }
}

/// #158 companion: leaves entering a no-soft-wrap run keep their measured
/// width — flex's default shrink would squeeze them onto the one line,
/// where Chrome lets the unbreakable content overflow instead.
pub(super) fn pin_leaf_widths(
    taffy_tree: &mut TaffyTree<TextLeaf>,
    leaves: &[diting_taffy::tree::NodeId],
) {
    for &l in leaves {
        taffy_tree.pin_shrink(l);
    }
}

/// The effective white-space governing `child`'s participation in an
/// inline formatting context: the fold (most-restrictive wins, the
/// documented "any nowrap pins the run" precedent) of the CONTAINER's
/// resolved value — the parent, white-space being inherited — and the
/// child's own when it is an element that declared one. Text nodes have
/// no style entry; their parent read mirrors the nws lookup the run
/// builder uses.
pub(super) fn ifc_white_space(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    child: NodeId,
    is_text: bool,
) -> WhiteSpace {
    let parent_ws = tree
        .with_node(child, |n| n.parent)
        .flatten()
        .and_then(|h| styles.get(&h))
        .and_then(|s| s.white_space);
    let own_ws = if is_text {
        None
    } else {
        styles.get(&child).and_then(|s| s.white_space)
    };
    match (parent_ws, own_ws) {
        (Some(p), Some(o)) => {
            if ws_rank(o) > ws_rank(p) {
                o
            } else {
                p
            }
        }
        (Some(p), None) | (None, Some(p)) => p,
        (None, None) => WhiteSpace::Normal,
    }
}

/// The line box a `<br>` owns when the line it ends is EMPTY (#146): a
/// zero-width, line-height-tall strut. Chrome's trailing-br accounting —
/// verified against headless Chrome — is that a forced break ends the
/// current line and contributes a line box only when that line is empty
/// (block start, after another `<br>`, after a block sibling); a trailing
/// break after content adds no height, and the line a break opens for
/// following content is that content's own line. Not in `node_map` — the
/// strut paints nothing and owns no DOM identity.
pub(super) fn br_strut_leaf(
    taffy_tree: &mut TaffyTree<TextLeaf>,
    lh: f32,
) -> Option<diting_taffy::tree::NodeId> {
    taffy_tree
        .new_leaf(Style {
            size: Size {
                width: Dimension::length(0.0),
                height: Dimension::length(lh.max(0.0)),
            },
            ..Style::default()
        })
        .ok()
}
