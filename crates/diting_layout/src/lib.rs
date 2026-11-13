//! DOM → box tree → geometry: diting's layout stage (screenshot feature).
//!
//! Entry points: `compute_styles*` (`cascade.rs`) runs the cascade over a `DomTree` into
//! per-node `ComputedStyle`; `layout_solve*` maps that onto our taffy tree
//! (`flow_tree.rs`, vendored `diting_taffy`) and solves it; `layout_collect*`
//! / `layout_dom*` walk the solved tree into paint items (`paint.rs`) with
//! text measured by swash (`text.rs`) and decoded images (`image.rs`).
//! Floats, tables, positioned/sticky boxes, forms, SVG and pseudo-element
//! content each live in their own submodule below.
//!
//! The taffy fork-delta regression ports (obscura's forked taffy vs ours)
//! are test-only and live in `fork_deltas.rs` / `fork_table_deltas.rs`.

use std::collections::{HashMap, HashSet};

use diting_taffy::prelude::*;
// Our own tree (flow_tree.rs) answers to the stock tree's name and methods.
use flow_tree::FlowTree as TaffyTree;

use diting_css::{
    AlignContentMode, AlignMode, ComputedStyle, Display as CssDisplay,
    FlexDirection as CssFlexDirection, FlexWrapMode, GridTrack, JustifyMode, ObjectFit,
    ObjectPositionPart, Overflow, PositionMode, TextAlign, TextDecorations, TextDirection,
    TextOverflow, TextShadow, TextTransform, WhiteSpace,
};
use diting_dom::tree::{DomTree, NodeId};

// Bundled CJK font supply for /screenshot determinism (batch 3c). Lives in
// this crate since #58: fonts and layout reference each other (FontBook /
// FaceBytes / Han slots), so they compile as one unit.
mod abs_rects;
mod clip_text;
mod collect;
pub mod diting_fonts;
use collect::{collect, Xf, ZEscape};
mod counters;
mod pseudo;
use pseudo::{append_pseudo_leaves, pseudo_leaf_style};
mod face_bytes;
pub mod float_flow;
mod float_lines;
#[cfg(test)]
mod float_probe;
mod flow_tree;
mod forms;
pub mod image;
mod inline_flow;
pub mod paint;
pub mod sticky;
pub mod svg;
mod table;
pub mod text;
pub mod text_geom;
mod text_src;
#[cfg(test)]
pub(crate) use counters::q_quote_pair;
use counters::{
    apply_counter_modifiers, pop_out_of_scope, resolve_content_into, wrap_q_quotes, CounterState,
};
pub use face_bytes::FaceBytes;
pub use image::DecodedImage;
use sticky::establishes_stacking_context;
pub use sticky::LayoutCollect;
pub use sticky::{apply_sticky_to_items, sticky_axis_shift, sticky_inset_px, StickySpan};
use table::{build_table, place_table_spans, TableBuildMeta};
pub use text::FontBook;
use text_src::trim_run_edge_whitespace;
mod cascade;
mod container_units;
mod element_build;
mod float_zone;
mod layout_collect;
mod solve_fixup;
mod style_map;
mod style_memo;
#[cfg(test)]
mod style_memo_tests;
pub use cascade::{
    compute_styles, compute_styles_gated, compute_styles_timed, compute_styles_timed_within,
    container_plan, ContainerPlan,
};
use container_units::bake_container_units;
use element_build::{build_element, RunSeg};
use float_zone::build_float_zone;
pub use layout_collect::{
    body_overflow_propagates, effective_viewport_overflow, layout_collect,
    layout_collect_with_images,
};
use solve_fixup::{
    continue_floats, harvest_static_positions, repair_box_sizes, repair_calc_widths,
    repair_wrap_host_intrinsics, reparent_out_of_flow, SolveAborted,
};
use style_map::{resolve_sizing_keywords, side_px, to_taffy_style};
pub use style_memo::clear_style_memo;

/// Absolute (page-relative) border-box rect of a DOM element after layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// CJK classification for the tokenizer (per-glyph line breaks) — kept from
/// the pre-3a deterministic metrics era; `is_cjk` no longer feeds widths.
fn is_cjk(c: char) -> bool {
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
enum TextLeaf {
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
fn effective_line_height(
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
fn block_level_child(tree: &DomTree, cid: NodeId, styles: &HashMap<NodeId, ComputedStyle>) -> bool {
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
fn blockifies_for_block_child(
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
fn fix_flex_basis_carry(
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
fn clamp_corner_radii(radii: [(f32, f32); 4], w: f32, h: f32) -> [(f32, f32); 4] {
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
fn font_context(
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
fn color_context(tree: &DomTree, id: NodeId, styles: &HashMap<NodeId, ComputedStyle>) -> [u8; 4] {
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
fn mono_context(tree: &DomTree, id: NodeId, styles: &HashMap<NodeId, ComputedStyle>) -> bool {
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
fn word_spacing_context(
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
fn small_caps_context(tree: &DomTree, id: NodeId, styles: &HashMap<NodeId, ComputedStyle>) -> bool {
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
fn text_transform_context(
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
fn han_context(
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
fn decoration_context(
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
fn own_decorations(p: &ComputedStyle) -> TextDecorations {
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
fn valign_shift(
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
fn leaf_descent(fonts: &FontBook, fs: f32, bold: bool) -> f32 {
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
fn shift_pad(shift: f32, descent: f32) -> f32 {
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
fn selected_option_label(tree: &DomTree, root: NodeId) -> Option<String> {
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
fn form_control_run(
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
fn is_alignment_promote(s: &Style) -> bool {
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
fn stretch_block_children_in_aligned_parent(
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

fn collapse_adjacent_sibling_margins(
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
fn build_word_leaves(
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
fn run_tokens(
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
fn ws_rank(ws: WhiteSpace) -> u8 {
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
fn measure_text_leaf(
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
fn run_wrapper_style(ws: WhiteSpace) -> Style {
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
fn pin_leaf_widths(taffy_tree: &mut TaffyTree<TextLeaf>, leaves: &[diting_taffy::tree::NodeId]) {
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
fn ifc_white_space(
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
fn br_strut_leaf(
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

/// Tags whose layout box is a replaced leaf: intrinsic size + aspect ratio,
/// no children in the layout tree. `input`/`textarea` join the img/canvas
/// family: an unstyled control used to fall through to the plain-block path
/// and measure 0×N (no content to lay out), which reads as "invisible" to
/// every rect-based consumer — Playwright's actionability loop waits forever
/// on a zero-height input (obscura#807 class). `svg` joins for the same
/// reason plus a stronger one: its children are shapes/text in viewBox
/// user units, not flow content — flowing them is the text-leak bug the
/// AginxOS svg batch starts from. The subtree compiles to paint ops at
/// collect time instead (see [`svg::compile_svg`]).
fn is_replaced_tag(tag: &str) -> bool {
    matches!(
        tag,
        "img"
            | "video"
            | "iframe"
            | "canvas"
            | "object"
            | "embed"
            | "input"
            | "textarea"
            | "select"
            | "svg"
    )
}

/// Canonicalize an `<img>`-shaped URL against the document base. The byte
/// tables both fetch paths key (`ImageCache::network_bytes`, the screenshot
/// prefetch, the band-pump's `image_bytes`) hold ABSOLUTE URLs, so a
/// relative `src="dot.png"` that passes through raw never matches its own
/// fetched body — the img paints a placeholder while the bytes sit in the
/// table. Resolution follows the fetchers' own join (`base.join(raw)`), so
/// both sides produce the same canonical `Url` string. data:/blob: are
/// self-contained and pass through; an already-absolute src canonicalizes
/// through `Url::parse` (same normalizer `join` applies); without a base —
/// or against an unparseable base — the raw string survives unchanged,
/// preserving the pre-base pass-through (miss → placeholder).
fn absolutize_img_src(raw: &str, base_url: Option<&str>) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("data:") || raw.starts_with("blob:") {
        return raw.to_string();
    }
    if let Ok(u) = url::Url::parse(raw) {
        return u.to_string();
    }
    match base_url
        .and_then(|b| url::Url::parse(b).ok())
        .and_then(|b| b.join(raw).ok())
    {
        Some(joined) => joined.to_string(),
        None => raw.to_string(),
    }
}

/// The URL an `<img>` should load: its `<picture>` parent's first matching
/// `<source>` candidate when present (media query evaluated at the layout
/// viewport), else the img's own srcset selection, else plain `src`
/// (HTML §4.8.4.3.9, product-simplified). `base_url` is the owner
/// document's URL — relative sources absolutize against it (see
/// [`absolutize_img_src`]) so the returned string is the key the fetched
/// byte tables use. None → the placeholder path.
pub fn resolve_img_source(
    tree: &DomTree,
    img: NodeId,
    viewport_width: f32,
    base_url: Option<&str>,
) -> Option<String> {
    let attr = |id: NodeId, name: &str| {
        tree.with_node(id, |n| n.get_attribute(name).map(|v| v.to_string()))
            .flatten()
    };

    // A <picture> parent contributes candidates only from <source> children
    // that precede the img in document order; the img itself terminates the
    // scan.
    if let Some(parent) = tree.with_node(img, |n| n.parent).flatten() {
        let parent_is_picture = tree
            .with_node(parent, |n| {
                n.as_element().map(|e| e.local.to_string() == "picture")
            })
            .flatten()
            .unwrap_or(false);
        if parent_is_picture {
            for child in tree.children(parent) {
                let tag = tree
                    .with_node(child, |n| n.as_element().map(|e| e.local.to_string()))
                    .flatten()
                    .unwrap_or_default();
                if child == img {
                    break;
                }
                if tag != "source" {
                    continue;
                }
                // media="(min-width: …)" / absent media both evaluate here;
                // a non-matching source is skipped to the next sibling.
                if !media_matches_width(attr(child, "media").as_deref(), viewport_width) {
                    continue;
                }
                if let Some(srcset) = attr(child, "srcset") {
                    let cands = image::parse_srcset(&srcset);
                    if let Some(c) = image::select_srcset_candidate(&cands, viewport_width) {
                        return Some(absolutize_img_src(&c.url, base_url));
                    }
                }
                // type="image/webp" etc.: without the candidate set matching
                // we fall through — a source with no usable srcset never wins.
            }
        }
    }

    if let Some(srcset) = attr(img, "srcset") {
        let cands = image::parse_srcset(&srcset);
        if let Some(c) = image::select_srcset_candidate(&cands, viewport_width) {
            return Some(absolutize_img_src(&c.url, base_url));
        }
    }

    attr(img, "src").map(|s| absolutize_img_src(&s, base_url))
}

/// Minimal media-query width gate for `<source media>`: supports the two
/// real-world forms `(min-width: Npx)` / `(max-width: Npx)` joined by
/// `and`, plus absent/`all`. Anything else declines (conservative: falls
/// through to the next source or the fallback img).
pub fn media_matches_width(media: Option<&str>, vw: f32) -> bool {
    let Some(media) = media.map(str::trim).filter(|m| !m.is_empty()) else {
        return true;
    };
    if media.eq_ignore_ascii_case("all") {
        return true;
    }
    // NOT-prefixed or print media queries: decline rather than guess.
    if media.contains("not ") || media.contains("print") {
        return false;
    }
    let mut ok = true;
    for clause in media.split(" and ") {
        let clause = clause.trim().trim_start_matches('(').trim_end_matches(')');
        if let Some(w) = clause
            .strip_prefix("min-width:")
            .and_then(|v| v.trim().strip_suffix("px"))
            .and_then(|v| v.trim().parse::<f32>().ok())
        {
            ok &= vw >= w;
        } else if let Some(w) = clause
            .strip_prefix("max-width:")
            .and_then(|v| v.trim().strip_suffix("px"))
            .and_then(|v| v.trim().parse::<f32>().ok())
        {
            ok &= vw <= w;
        }
        // Unknown feature inside a conjunction: leave `ok` untouched (the
        // common case is a single min/max-width clause anyway).
    }
    ok
}

/// The rect replaced content paints into, per object-fit/object-position
/// (batch 5c) — the port of blitz-paint's sizing.rs + render.rs draw_image
/// offset math. `box_rect` is the content box, (nw, nh) the natural size:
///
/// - Fill: the whole box (the pre-5c behavior).
/// - Contain / Cover: scale by min/max of the two axis ratios; blitz's
///   four-arm `(x<1, y<1)` match reduces to exactly that.
/// - None: natural size. ScaleDown: Contain unless natural is already
///   smaller than the contain result on width (then natural).
/// - Offset: each object-position part resolves against the free space
///   `box − paint` — percentages scale it, px lengths use it directly;
///   the initial 50%/50% centers. Cover offsets go negative (overflow).
pub fn object_paint_rect(
    box_rect: Rect,
    nw: f32,
    nh: f32,
    fit: ObjectFit,
    pos: (ObjectPositionPart, ObjectPositionPart),
) -> Rect {
    let (bw, bh) = (box_rect.width, box_rect.height);
    let paint = |w: f32, h: f32| Rect {
        x: 0.0,
        y: 0.0,
        width: w,
        height: h,
    };
    let size = if nw <= 0.0 || nh <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        match fit {
            ObjectFit::None => paint(nw, nh),
            _ => paint(bw, bh),
        }
    } else {
        let (xr, yr) = (bw / nw, bh / nh);
        match fit {
            ObjectFit::Fill => paint(bw, bh),
            ObjectFit::Contain => paint(nw * xr.min(yr), nh * xr.min(yr)),
            ObjectFit::Cover => paint(nw * xr.max(yr), nh * xr.max(yr)),
            ObjectFit::None => paint(nw, nh),
            ObjectFit::ScaleDown => {
                let (cw, ch) = (nw * xr.min(yr), nh * xr.min(yr));
                if nw < cw {
                    paint(nw, nh)
                } else {
                    paint(cw, ch)
                }
            }
        }
    };
    let resolve = |part: ObjectPositionPart, free: f32| match part {
        ObjectPositionPart::Percent(p) => free * p / 100.0,
        ObjectPositionPart::Px(x) => x,
    };
    Rect {
        x: box_rect.x + resolve(pos.0, bw - size.width),
        y: box_rect.y + resolve(pos.1, bh - size.height),
        width: size.width,
        height: size.height,
    }
}

/// CSS 2.1 §10.5: a percentage height resolves only when the containing
/// block's height is specified EXPLICITLY; against a content-sized
/// (indefinite) ancestor the percentage behaves as `auto`. Walk the DOM
/// ancestors of `id`: Px (or a viewport unit already folded to px at
/// cascade) is definite; Percent/Calc defers the question one level up;
/// auto/keywords/nothing stops the walk as indefinite. The document root
/// counts as definite — its percentage resolves against the viewport.
fn cb_height_definite(tree: &DomTree, styles: &HashMap<NodeId, ComputedStyle>, id: NodeId) -> bool {
    let mut cur = tree.with_node(id, |n| n.parent).flatten();
    while let Some(nid) = cur {
        if tree.with_node(nid, |n| n.parent).flatten().is_none() {
            return true;
        }
        match styles.get(&nid).and_then(|s| s.height) {
            Some(diting_css::Length::Px(_)) => return true,
            Some(diting_css::Length::Percent(_)) | Some(diting_css::Length::Calc { .. }) => {
                cur = tree.with_node(nid, |n| n.parent).flatten();
            }
            _ => return false,
        }
    }
    false
}

/// Empty every cached layout in `root`'s subtree (taffy's `mark_dirty`
/// propagates only to ancestors). The two post-layout repair passes use it
/// when a fixup touched a float: a repaired float's new collision extent is
/// invisible to its siblings' cache keys — (style, constraints) — so a
/// cached text leaf would keep its float_lines slots wrapped against the
/// float's old extent and paint across the repaired box.
fn bust_subtree_cache(node: diting_taffy::tree::NodeId, tree: &mut TaffyTree<TextLeaf>) {
    let _ = tree.mark_dirty(node);
    for c in tree.children(node).unwrap_or_default().to_vec() {
        bust_subtree_cache(c, tree);
    }
}

/// Depth of a taffy node below the ICB — the post-layout repair passes sort
/// their fixups shallow-first so an inner fixup feeds an outer one's basis.
fn taffy_depth(
    taffy_tree: &TaffyTree<TextLeaf>,
    node: diting_taffy::tree::NodeId,
    icb: diting_taffy::tree::NodeId,
) -> usize {
    let (mut d, mut cur) = (0usize, node);
    while cur != icb {
        match taffy_tree.parent(cur) {
            Some(p) => {
                cur = p;
                d += 1;
            }
            None => break,
        }
    }
    d
}

/// Whether `height: <percent>` on element `id` resolves, or folds to `auto`
/// per CSS 2.1 §10.5: it resolves when the containing-block chain is definite
/// (see [`cb_height_definite`]) OR the element is absolutely positioned (the
/// §10.5 exception — its containing block is the nearest positioned
/// ancestor's padding box, definite by construction once that ancestor has
/// laid out).
fn pct_height_resolves(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    id: NodeId,
) -> bool {
    let abspos = matches!(
        styles.get(&id).and_then(|s| s.position),
        Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
    );
    abspos || cb_height_definite(tree, styles, id)
}

/// Build the taffy leaf for a replaced element. Per-tag natural-size
/// semantics (batch 7a), mirroring blitz-dom layout/mod.rs:
///
/// - `img`: a decoded image gives the natural size/ratio; attributes are
///   presentational hints overriding per-axis with the image ratio
///   back-filling; nothing at all → the CSS default replaced box 300×150.
/// - `canvas`: its width/height attributes ARE the intrinsic size
///   (defaulting 300×150) and it carries an aspect RATIO — a missing axis
///   transfers through it.
/// - `video`/`iframe`/`embed`: attribute-or-300×150 per axis, NO ratio —
///   `<video width=600>` lays out 600×150, height does not transfer.
fn build_replaced_leaf(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    strut_descent: f32,
) -> Option<diting_taffy::tree::NodeId> {
    // #196: replaced elements skip build_element_inner's display gate (they
    // attach from three run-attach sites that call here directly), so a
    // display:none textarea — baidu/sina ship page templates as hidden
    // textareas — used to lay out as a real control and paint its value
    // (the raw `<style …>` template text) as visible content. One gate
    // here closes all three attach sites at once.
    if styles.get(&id).and_then(|s| s.display) == Some(CssDisplay::None) {
        return None;
    }
    let style = styles.get(&id).cloned().unwrap_or_default();
    let tag = tree
        .with_node(id, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    let attr = |name: &str| {
        tree.with_node(id, |n| n.get_attribute(name).map(|v| v.to_string()))
            .flatten()
            .and_then(|v| v.parse::<f32>().ok())
    };
    let (aw, ah) = (attr("width"), attr("height"));
    // Natural size AND whether a missing axis derives from the ratio.
    let (nat_w, nat_h, ratio_transfer) = match tag.as_str() {
        // Form controls (obscura#807 class): Chrome-shaped default boxes.
        // Text-like inputs ≈ the size=20 default column; checkbox/radio are
        // square 13px widgets; button-like inputs approximate label width
        // from the value attribute (input has no laid-out text of its own);
        // textarea is the cols=20/rows=2 default box. All three axes are
        // overridable per-axis by CSS/attribute width & height below.
        "input" => {
            let ty = tree
                .with_node(id, |n| {
                    n.get_attribute("type").map(|v| v.to_ascii_lowercase())
                })
                .flatten()
                .unwrap_or_default();
            match ty.as_str() {
                "checkbox" | "radio" => (13.0, 13.0, false),
                // Chrome's default slider box (blitz#456).
                "range" => (129.0, 16.0, false),
                "button" | "submit" | "reset" => {
                    // Same precedence as the paint run: dirty value, then the
                    // parsed value attribute, then Chrome's default "Submit".
                    let label = tree
                        .with_node(id, |n| {
                            n.live_value()
                                .map(|v| v.to_string())
                                .or_else(|| n.get_attribute("value").map(|v| v.to_string()))
                        })
                        .flatten()
                        .unwrap_or_default();
                    let label: String = if label.is_empty() && ty == "submit" {
                        "Submit".to_string()
                    } else {
                        label
                    };
                    (label.chars().count() as f32 * 8.8 + 20.0, 22.0, false)
                }
                _ => (177.0, 22.0, false),
            }
        }
        "textarea" => (177.0, 38.0, false),
        // An atomic replaced box like the others: options never lay out as
        // page text (a closed select paints one label). Width tracks the
        // label it will paint — the same precedence form_control_run
        // resolves — with control padding and dropdown-arrow room added,
        // matching how the button arm sizes from its label.
        "select" => {
            let label = tree
                .with_node(id, |n| n.live_value().map(|v| v.to_string()))
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| selected_option_label(tree, id))
                .unwrap_or_default();
            (label.chars().count() as f32 * 8.8 + 36.0, 22.0, false)
        }
        "canvas" => (aw.unwrap_or(300.0), ah.unwrap_or(150.0), true),
        "video" | "iframe" | "embed" | "object" => {
            (aw.unwrap_or(300.0), ah.unwrap_or(150.0), false)
        }
        // svg: the viewBox IS the intrinsic size/ratio — the element box
        // maps onto it at paint (that map is the zoom semantics of this
        // batch). Attr w/h override per-axis and back-fill through the
        // viewBox ratio; no viewBox: attr-or-300×150 like the others.
        "svg" => {
            let vb = tree
                .with_node(id, |n| n.get_attribute("viewBox").map(|v| v.to_string()))
                .flatten()
                .and_then(|v| svg::parse_view_box(&v));
            match vb {
                Some((_, _, vw, vh)) => {
                    let r = vw / vh;
                    let (w, h) = match (aw, ah) {
                        (Some(w), Some(h)) if h > 0.0 => (w, h),
                        (Some(w), None) => (w, w / r),
                        (None, Some(h)) => (h * r, h),
                        _ => (vw, vh),
                    };
                    (w, h, true)
                }
                None => (aw.unwrap_or(300.0), ah.unwrap_or(150.0), false),
            }
        }
        // img: attrs override the decoded image per-axis, the image ratio
        // back-fills a free axis; with nothing at all, 300×150.
        _ => {
            let img_ratio = images.get(&id).map(|i| i.width as f32 / i.height as f32);
            let (w, h) = match (aw, ah) {
                (Some(w), Some(h)) if h > 0.0 => (w, h),
                (Some(w), None) => (w, w / img_ratio.unwrap_or(2.0)),
                (None, Some(h)) => (h * img_ratio.unwrap_or(2.0), h),
                (None, None) => images
                    .get(&id)
                    .map(|i| (i.width as f32, i.height as f32))
                    .unwrap_or((300.0, 150.0)),
                // Degenerate attrs (e.g. height="0"): the CSS default box.
                _ => (300.0, 150.0),
            };
            (w, h, true)
        }
    };
    // #120 zoom: the natural box is a declaration-class size — attrs are
    // presentational hints, decoded image dimensions and the form-control
    // default boxes are intrinsic — and Chrome renders all of them scaled
    // inside a zoom subtree. Scaling here (after the match) keeps the ratio
    // arithmetic above in unscaled coordinates, so ratio_transfer is
    // unaffected (both axes scale together).
    let (nat_w, nat_h) = (nat_w * style.zoom_accum, nat_h * style.zoom_accum);

    // Attrs block ratio transfer for img and svg: width/height attrs are
    // presentational-hint DECLARATIONS, so a CSS override of one axis does
    // not re-derive the other (#cssw stays 100×200, not 100×50). Canvas
    // attrs ARE the intrinsic size and never block.
    let (attr_dw, attr_dh) = if tag == "img" || tag == "svg" {
        (aw.is_some(), ah.is_some())
    } else {
        (false, false)
    };
    let ratio = ratio_transfer.then(|| nat_w / nat_h);
    let css_w_px = match style.width {
        Some(diting_css::Length::Px(w)) => Some(w),
        _ => None,
    };
    let css_h_px = match style.height {
        Some(diting_css::Length::Px(h)) => Some(h),
        _ => None,
    };
    // Build-time ratio transfer: with exactly one authored CSS px axis and
    // nothing declaring the other, the free axis derives through the
    // natural ratio. taffy's aspect_ratio only re-derives in block/column
    // flows — not the flex-row run wrapper inline atoms now join — while
    // Chrome transfers in every container.
    let derived_h = match (ratio, css_w_px) {
        (Some(r), Some(w)) if !attr_dh => Some(w / r),
        _ => None,
    };
    let derived_w = match (ratio, css_h_px) {
        (Some(r), Some(h)) if !attr_dw => Some(h * r),
        _ => None,
    };

    // Positioning rides into replaced leaves too (to_taffy_style does this
    // for every boxed element): without it an absolutely-positioned
    // input/img/select kept its taffy-relative style, the reparent pass
    // still moved it to the containing block, and it then stacked in flow
    // there ignoring its insets.
    let position = match style.position {
        Some(diting_css::PositionMode::Absolute) | Some(diting_css::PositionMode::Fixed) => {
            Position::Absolute
        }
        _ => Position::Relative,
    };
    let mut s = Style {
        item_is_replaced: true,
        position,
        ..Default::default()
    };
    // Inset/clamp unset values are CSS `auto` (same mapping as
    // to_taffy_style's local lpa_auto).
    let lpa_auto = |v: Option<diting_css::Length>| match v {
        Some(diting_css::Length::Px(px)) => LengthPercentageAuto::length(px),
        Some(diting_css::Length::Percent(p)) => LengthPercentageAuto::percent(p / 100.0),
        Some(diting_css::Length::Calc { percent, .. }) => {
            LengthPercentageAuto::percent(percent / 100.0)
        }
        Some(
            diting_css::Length::Auto
            | diting_css::Length::MinContent
            | diting_css::Length::MaxContent
            | diting_css::Length::FitContent,
        )
        | None => LengthPercentageAuto::auto(),
    };
    if style.top.is_some()
        || style.right.is_some()
        || style.bottom.is_some()
        || style.left.is_some()
    {
        s.inset = diting_taffy::geometry::Rect {
            top: lpa_auto(style.top),
            right: lpa_auto(style.right),
            bottom: lpa_auto(style.bottom),
            left: lpa_auto(style.left),
        };
    }
    // UA/author border lays out on replaced boxes too (batch 7a): the
    // iframe's UA `2px inset` makes a width=600 attr box come out 604.
    // taffy sizes are border-box, so the attr/CSS size gets the widths
    // added (content-box semantics of the HTML attributes).
    let bline = style.border_style.is_some();
    let bw = |which: f32| -> LengthPercentage {
        LengthPercentage::length(if bline { which } else { 0.0 })
    };
    let (bt, br, bb, bl) = (
        side_px(style.border_width.top),
        side_px(style.border_width.right),
        side_px(style.border_width.bottom),
        side_px(style.border_width.left),
    );
    s.border = diting_taffy::geometry::Rect {
        top: bw(bt),
        right: bw(br),
        bottom: bw(bb),
        left: bw(bl),
    };
    // The strut descent bakes into the leaf's DEFINITE height (line-box
    // bookkeeping; collect subtracts it back from the rect). taffy's ratio
    // transfer off that height — the cyclic-percent case, where a percent
    // width against a content-sized ancestor can't resolve and the width
    // derives from the height — must land on nat_w, not nat_w + strut×ratio
    // (the +9px hero width). Fold the strut into the denominator instead.
    // Both-definite leaves never transfer (the ratio is inert there) and the
    // width→height direction can't fire for inline atoms — their height arm
    // is always the baked definite length — so the compensated ratio only
    // ever feeds the height→width transfer. Block-level callers pass strut
    // 0 and keep the raw natural ratio.
    s.aspect_ratio = ratio_transfer.then(|| nat_w / (nat_h + strut_descent));
    // CSS width/height win per axis; missing axis derives from the ratio.
    // Percent CSS sizes pass through (the CB resolves them; the natural
    // ratio only backfills auto axes). Px sizes are content-box per the
    // attribute semantics, so border widths ride on top — except under
    // authored `box-sizing: border-box`, where the px already measures to
    // the border edge.
    let border_box = matches!(style.box_sizing, Some(diting_css::BoxSizing::BorderBox));
    // The strut descent (inline atoms only — block-level callers pass 0)
    // extends the LEAF below the element box so the line box spans the
    // strut's full extent; collect subtracts it back from the recorded
    // rect, so percent/calc arms (CB-resolved approximations) skip the pad
    // rather than grow an axis the rect shrink could not undo exactly.
    s.size = Size {
        width: match style.width {
            Some(diting_css::Length::Px(w)) => Dimension::length(if border_box {
                w
            } else {
                w + if bline { bl + br } else { 0.0 }
            }),
            Some(diting_css::Length::Percent(p)) => Dimension::percent(p / 100.0),
            // Percent-only placeholder (same convention as to_taffy_style).
            Some(diting_css::Length::Calc { percent, .. }) => Dimension::percent(percent / 100.0),
            // Sizing keywords on replaced boxes: intrinsic = natural size
            // (css-sizing-3), same fallback as auto.
            Some(
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent,
            )
            | None => {
                Dimension::length(derived_w.unwrap_or(nat_w) + if bline { bl + br } else { 0.0 })
            }
        },
        height: match style.height {
            Some(diting_css::Length::Px(h)) => Dimension::length(
                (if border_box {
                    h
                } else {
                    h + if bline { bt + bb } else { 0.0 }
                }) + strut_descent,
            ),
            Some(diting_css::Length::Percent(p)) if pct_height_resolves(tree, styles, id) => {
                Dimension::percent(p / 100.0)
            }
            Some(diting_css::Length::Calc { percent, .. })
                if pct_height_resolves(tree, styles, id) =>
            {
                Dimension::percent(percent / 100.0)
            }
            // Percent (or calc with a percent part) against a content-sized
            // ancestor behaves as `auto` (§10.5) — for a replaced box its
            // natural height. Taffy would resolve the percent against an
            // indefinite CB to 0, collapsing `height:100%` iframes inside
            // auto-height wrappers (the webtop .window-body case) to zero.
            Some(
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent
                | diting_css::Length::Percent(_)
                | diting_css::Length::Calc { .. },
            )
            | None => Dimension::length(
                derived_h.unwrap_or(nat_h) + if bline { bt + bb } else { 0.0 } + strut_descent,
            ),
        },
    };

    let node = taffy_tree
        .new_leaf_with_context(s, TextLeaf::Replaced { strut_descent })
        .ok()?;
    node_map.insert(node, id);
    Some(node)
}

/// Build the taffy subtree for one element. Returns None for display:none
/// (subtree skipped) and for the document node's non-element parts.
/// Strut descent for an inline-level replaced atom: the descent of the
/// element's inherited font context (the strut IS a zero-width glyph of the
/// line's font). Block-level replaced boxes pass 0.
fn strut_descent_for(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
) -> f32 {
    let (fs, bold, _) = font_context(tree, id, styles, fonts);
    // Whole-pixel pad so the padded leaf height survives taffy's integer
    // rounding exactly and collect's subtract-back lands on the true box.
    leaf_descent(fonts, fs, bold).ceil()
}

/// Build one child into its parent's normal-flow child list — the same
/// replaced/text/inline-run/block dispatch as build_element's main loop,
/// factored out so the float-zone branches (8b/8c) can append zone-external
/// siblings without duplicating the body. Runs are flushed internally (the
/// float branches interleave zone rows between siblings, so a shared pending
/// run across calls would misorder).
#[allow(clippy::too_many_arguments)]
fn build_normal_sibling(
    child: NodeId,
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    flattened: &mut HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    meta: &mut TableBuildMeta,
    atomic_container: bool,
    font_size: f32,
    line_height: f32,
    direct: &mut Vec<diting_taffy::tree::NodeId>,
) {
    let is_text = tree.with_node(child, |n| n.is_text()).unwrap_or(false);
    let child_tag = tree
        .with_node(child, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    let child_display = styles.get(&child).and_then(|s| s.display).or(if is_text {
        Some(CssDisplay::Inline)
    } else {
        Some(CssDisplay::Block)
    });
    let inline_level = matches!(child_display, Some(CssDisplay::Inline));
    if !is_text && child_display == Some(CssDisplay::None) {
        return;
    } // hidden: no box, must not cut the run
    let out_of_flow = styles.get(&child).is_some_and(|s| {
        matches!(
            s.position,
            Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
        )
    });
    if !is_text && is_replaced_tag(&child_tag) {
        // A replaced element whose display resolves to any inline flavor
        // (UA-sheet inline-block for form controls included) is an inline
        // atom: it joins the text run instead of becoming a block sibling.
        let inline_atom = inline_level || child_display == Some(CssDisplay::InlineBlock);
        if inline_atom && !atomic_container && !out_of_flow {
            let sd = strut_descent_for(tree, child, styles, fonts);
            if let Some(leaf) =
                build_replaced_leaf(tree, child, styles, images, taffy_tree, node_map, sd)
            {
                // A lone inline atom still gets the wrapping-run stand-in so
                // it lays out on the text baseline path like the main loop.
                if let Ok(wrapper) = taffy_tree.new_with_children(
                    run_wrapper_style(ifc_white_space(tree, styles, child, false)),
                    &[leaf],
                ) {
                    run_wrappers.push(wrapper);
                    direct.push(wrapper);
                }
            }
        } else if let Some(leaf) =
            build_replaced_leaf(tree, child, styles, images, taffy_tree, node_map, 0.0)
        {
            direct.push(leaf);
        }
        return;
    }
    if !is_text
        && child_display == Some(CssDisplay::InlineBlock)
        && !atomic_container
        && !out_of_flow
    {
        // Atomic inline-level box (obscura#750 family): keeps its own subtree
        // box and gets the wrapping-run stand-in so it sits on the text line
        // path like a replaced atom, sized shrink-to-fit by the parent run.
        if let Some(sub) = build_element(
            tree,
            child,
            styles,
            images,
            fonts,
            taffy_tree,
            node_map,
            flattened,
            run_wrappers,
            meta,
        ) {
            if let Ok(wrapper) = taffy_tree.new_with_children(
                run_wrapper_style(ifc_white_space(tree, styles, child, false)),
                &[sub],
            ) {
                run_wrappers.push(wrapper);
                direct.push(wrapper);
            }
        }
        return;
    }
    if is_text
        || (inline_level
            && !atomic_container
            && !out_of_flow
            && !blockifies_for_block_child(tree, child, styles))
    {
        // Text and flattenable inlines become their own measured run here:
        // single-segment runs are the overwhelmingly common case at
        // zone boundaries, and build_word_leaves + the run wrapper reproduce
        // the mixed-run path exactly.
        let mut leaves: Vec<diting_taffy::tree::NodeId> = Vec::new();
        if is_text {
            let text = tree
                .with_node(child, |n| {
                    n.text_content_of_text_node().unwrap_or("").to_string()
                })
                .unwrap_or_default();
            // text-transform folds into the string here so measure and paint
            // agree (批240); the DOM keeps the original text.
            let text = {
                let tt = text_transform_context(tree, child, styles);
                if tt == TextTransform::None {
                    text
                } else {
                    text::apply_text_transform(&text, tt)
                }
            };
            // Formatting-whitespace-only text generates no box (CSS white-
            // space processing — keeps adjoining block margins adjacent).
            if text.trim().is_empty() {
                return;
            }
            let (fs, b, lh) = font_context(tree, child, styles, fonts);
            let fs = if styles.get(&child).is_some() {
                fs
            } else {
                font_size
            };
            let lh = if styles.get(&child).is_some() {
                lh
            } else {
                line_height
            };
            let col = color_context(tree, child, styles);
            let deco = decoration_context(tree, child, styles);
            let vs = valign_shift(tree, child, styles, fonts);
            let mono = mono_context(tree, child, styles);
            let ws = word_spacing_context(tree, child, styles);
            let sc = small_caps_context(tree, child, styles);
            let han = han_context(tree, child, styles);
            text_src::set_cur(Some(child));
            leaves.extend(build_word_leaves(
                &text, fs, b, col, lh, deco, vs, mono, ws, sc, han, fonts, taffy_tree,
            ));
            text_src::set_cur(None);
        } else {
            let sub = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            );
            if let Some(sub) = sub {
                let mut sub_children: Vec<_> =
                    taffy_tree.children(sub).unwrap_or_default().to_vec();
                wrap_q_quotes(
                    tree,
                    child,
                    &child_tag,
                    styles,
                    fonts,
                    taffy_tree,
                    &mut sub_children,
                );
                leaves.extend(sub_children.clone());
                // Flattening removes the sub's taffy node (invalidating its
                // SlotMap key) — drop the stale node_map entry with it.
                node_map.remove(&sub);
                let _ = taffy_tree.remove(sub);
                // The element keeps its DOM identity boxless — the union
                // pass after the collect walk rebuilds a rect from the kids.
                if !sub_children.is_empty() {
                    flattened.insert(child, sub_children);
                }
            }
        }
        if !leaves.is_empty() {
            let ws = ifc_white_space(tree, styles, child, is_text);
            if ws.no_soft_wrap() {
                pin_leaf_widths(taffy_tree, &leaves);
            }
            if let Ok(wrapper) = taffy_tree.new_with_children(run_wrapper_style(ws), &leaves) {
                run_wrappers.push(wrapper);
                direct.push(wrapper);
            }
        }
        return;
    }
    if let Some(node) = build_element(
        tree,
        child,
        styles,
        images,
        fonts,
        taffy_tree,
        node_map,
        flattened,
        run_wrappers,
        meta,
    ) {
        direct.push(node);
    }
}

/// The anonymous flow column of a float zone (8b/8i): every in-zone sibling
/// built normally inside one block wrapper. Not in node_map — it has no DOM
/// identity, so collect skips it and paints walk straight through to the
/// real children.
#[allow(clippy::too_many_arguments)]
fn build_flow_column(
    flow_dom: &[NodeId],
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    fonts: &FontBook,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    flattened: &mut HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
    meta: &mut TableBuildMeta,
    font_size: f32,
    lh_elem: f32,
    container_ws: WhiteSpace,
) -> Vec<diting_taffy::tree::NodeId> {
    let mut flow_children: Vec<diting_taffy::tree::NodeId> = Vec::new();
    let mut run: Vec<RunSeg> = Vec::new();
    let mut run_clip: Option<NodeId> = None;
    let run_frame = text_src::new_frame();
    let flush_run = |run: &mut Vec<RunSeg>,
                     flow_children: &mut Vec<diting_taffy::tree::NodeId>,
                     taffy_tree: &mut TaffyTree<TextLeaf>,
                     run_wrappers: &mut Vec<diting_taffy::tree::NodeId>,
                     clip: Option<NodeId>| {
        if run.is_empty() {
            return;
        }
        let segs = std::mem::take(run);
        let pending = text_src::drain_pending(run_frame);
        let mut pending_it = pending.iter().copied();
        let seg_lens: Vec<u32> = segs
            .iter()
            .filter_map(|s| match s {
                RunSeg::Text(t, ..) => Some(t.chars().count() as u32),
                _ => None,
            })
            .collect();
        if segs.iter().all(|s| matches!(s, RunSeg::Text(..))) {
            let text = segs
                .iter()
                .map(|s| match s {
                    RunSeg::Text(t, ..) => t.as_str(),
                    _ => "",
                })
                .collect::<String>();
            if !text.trim().is_empty()
                || {
                    let any_preserve = segs.iter().any(|s| matches!(s, RunSeg::Text(.., w, _, _) if w.preserves_spaces() || w.preserves_newlines()));
                    any_preserve
                }
            {
                let RunSeg::Text(_, fs, bold, color, lh, deco, vs, mono, wsp, _, sc, han) =
                    &segs[0]
                else {
                    unreachable!()
                };
                // white-space inherits, so the preserve-wins winner across
                // segments rides the whole run (the "any nowrap pins the
                // run" precedent, ranked across the family).
                let run_ws = segs.iter().fold(WhiteSpace::Normal, |acc, s| match s {
                    RunSeg::Text(.., w, _, _) if ws_rank(*w) > ws_rank(acc) => *w,
                    _ => acc,
                });
                // Anonymous flow columns own no element style; text-overflow
                // and line-clamp are non-inherited, so their owner is
                // unknown here — v1-off. text-align IS inherited but the
                // zone path aligns through the host's promote, so the run
                // stays Left here too.
                let run_ellipsis = false;
                let run_clamp = None;
                if let Ok(leaf) = taffy_tree.new_leaf_with_context(
                    float_flow::run_style(),
                    TextLeaf::Run {
                        text,
                        font_size: *fs,
                        bold: *bold,
                        color: *color,
                        line_height: *lh,
                        decorations: *deco,
                        baseline_shift: *vs,
                        mono: *mono,
                        word_spacing: *wsp,
                        ws: run_ws,
                        atomic: false,
                        align: TextAlign::Left,
                        ellipsis: run_ellipsis,
                        line_clamp: run_clamp,
                        tokens: std::cell::RefCell::new(None),
                        small_caps: *sc,
                        han: *han,
                        clip_src: clip,
                    },
                ) {
                    text_src::register(leaf, text_src::seg_ranges(&pending, &seg_lens));
                    flow_children.push(leaf);
                }
            }
            return;
        }
        // #158: the wrapper stops wrapping when no segment (nor the
        // container) leaves a soft wrap opportunity — the same fold the
        // pure-text branch runs, plus the column's container value for
        // runs of pure atomic inlines that carry no ws of their own.
        let ws = segs.iter().fold(container_ws, |acc, s| match s {
            RunSeg::Text(.., w, _, _) if ws_rank(*w) > ws_rank(acc) => *w,
            _ => acc,
        });
        let mut leaves: Vec<diting_taffy::tree::NodeId> = Vec::new();
        for seg in segs {
            match seg {
                RunSeg::Text(text, fs, bold, color, lh, deco, vs, mono, wsp, _, sc, han) => {
                    text_src::set_cur(pending_it.next());
                    leaves.extend(build_word_leaves(
                        &text, fs, bold, color, lh, deco, vs, mono, wsp, sc, han, fonts, taffy_tree,
                    ));
                    text_src::set_cur(None);
                }
                RunSeg::Nodes(nodes) => leaves.extend(nodes),
            }
        }
        trim_run_edge_whitespace(taffy_tree, &mut leaves);
        if !leaves.is_empty() {
            if ws.no_soft_wrap() {
                pin_leaf_widths(taffy_tree, &leaves);
            }
            if let Ok(wrapper) = taffy_tree.new_with_children(run_wrapper_style(ws), &leaves) {
                run_wrappers.push(wrapper);
                flow_children.push(wrapper);
            }
        }
    };
    for child in flow_dom.iter().copied() {
        let is_text = tree.with_node(child, |n| n.is_text()).unwrap_or(false);
        let child_tag = tree
            .with_node(child, |n| n.as_element().map(|e| e.local.to_string()))
            .flatten()
            .unwrap_or_default();
        let child_display = styles.get(&child).and_then(|s| s.display).or(if is_text {
            Some(CssDisplay::Inline)
        } else {
            Some(CssDisplay::Block)
        });
        let inline_level = matches!(child_display, Some(CssDisplay::Inline));
        if !is_text && child_display == Some(CssDisplay::None) {
            continue;
        } // hidden: no box, must not cut the run
        let out_of_flow = styles.get(&child).is_some_and(|s| {
            matches!(
                s.position,
                Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
            )
        });
        if child_tag == "br" && !out_of_flow {
            // #146: same trailing-br accounting as the main walk (see
            // build_element_inner) — flush, then a strut only for an EMPTY
            // ended line.
            let line_has_content = run.iter().any(|seg| match seg {
                RunSeg::Text(t, ..) => !t.trim().is_empty(),
                RunSeg::Nodes(n) => !n.is_empty(),
            });
            flush_run(
                &mut run,
                &mut flow_children,
                taffy_tree,
                run_wrappers,
                run_clip.take(),
            );
            if !line_has_content && child_display != Some(CssDisplay::None) {
                let (_, _, lh) = font_context(tree, child, styles, fonts);
                if let Some(strut) = br_strut_leaf(taffy_tree, lh) {
                    flow_children.push(strut);
                }
            }
            continue;
        }
        if !is_text && is_replaced_tag(&child_tag) {
            // Inline-flavored replaced elements (UA inline-block form
            // controls included) join the text run; the rest stay block
            // siblings (current shipped behavior for img/video).
            let inline_atom = inline_level || child_display == Some(CssDisplay::InlineBlock);
            let sd = if inline_atom && !out_of_flow {
                strut_descent_for(tree, child, styles, fonts)
            } else {
                0.0
            };
            let leaf = build_replaced_leaf(tree, child, styles, images, taffy_tree, node_map, sd);
            if let Some(leaf) = leaf {
                if inline_atom && !out_of_flow {
                    run.push(RunSeg::Nodes(vec![leaf]));
                } else {
                    flush_run(
                        &mut run,
                        &mut flow_children,
                        taffy_tree,
                        run_wrappers,
                        run_clip.take(),
                    );
                    flow_children.push(leaf);
                }
            }
            continue;
        }
        if is_text {
            let text = tree
                .with_node(child, |n| {
                    n.text_content_of_text_node().unwrap_or("").to_string()
                })
                .unwrap_or_default();
            // text-transform folds into the string before the run sees it
            // (批240) — per segment is exact for upper/lower (stateless) and
            // per-node for capitalize (cross-element words are a documented
            // v1 divergence).
            let text = {
                let tt = text_transform_context(tree, child, styles);
                if tt == TextTransform::None {
                    text
                } else {
                    text::apply_text_transform(&text, tt)
                }
            };
            let (fs, b, lh) = font_context(tree, child, styles, fonts);
            let fs = if styles.get(&child).is_some() {
                fs
            } else {
                font_size
            };
            let lh = if styles.get(&child).is_some() {
                lh
            } else {
                lh_elem
            };
            let col = color_context(tree, child, styles);
            let deco = decoration_context(tree, child, styles);
            let vs = valign_shift(tree, child, styles, fonts);
            let mono = mono_context(tree, child, styles);
            let ws = word_spacing_context(tree, child, styles);
            let nws = tree
                .with_node(child, |n| n.parent)
                .flatten()
                .and_then(|p| styles.get(&p))
                .and_then(|s| s.white_space)
                .unwrap_or(WhiteSpace::Normal);
            let sc = small_caps_context(tree, child, styles);
            let han = han_context(tree, child, styles);
            run_clip = Some(child);
            text_src::note_seg(run_frame, child);
            run.push(RunSeg::Text(
                text, fs, b, col, lh, deco, vs, mono, ws, nws, sc, han,
            ));
        } else if child_display == Some(CssDisplay::InlineBlock) && !out_of_flow {
            // Atomic inline-level box (obscura#750 family): keeps its own
            // subtree box, joins the run as one shrink-to-fit unit.
            if let Some(sub) = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                run.push(RunSeg::Nodes(vec![sub]));
            }
        } else if inline_level && !out_of_flow && !blockifies_for_block_child(tree, child, styles) {
            let sub = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            );
            if let Some(sub) = sub {
                let mut sub_children: Vec<_> =
                    taffy_tree.children(sub).unwrap_or_default().to_vec();
                wrap_q_quotes(
                    tree,
                    child,
                    &child_tag,
                    styles,
                    fonts,
                    taffy_tree,
                    &mut sub_children,
                );
                run.push(RunSeg::Nodes(sub_children.clone()));
                node_map.remove(&sub);
                let _ = taffy_tree.remove(sub);
                // Boxless after hoisting — union pass rebuilds the rect.
                if !sub_children.is_empty() {
                    flattened.insert(child, sub_children);
                }
            }
        } else {
            flush_run(
                &mut run,
                &mut flow_children,
                taffy_tree,
                run_wrappers,
                run_clip.take(),
            );
            if let Some(node) = build_element(
                tree,
                child,
                styles,
                images,
                fonts,
                taffy_tree,
                node_map,
                flattened,
                run_wrappers,
                meta,
            ) {
                flow_children.push(node);
            }
        }
    }
    flush_run(
        &mut run,
        &mut flow_children,
        taffy_tree,
        run_wrappers,
        run_clip.take(),
    );
    flow_children
}

/// Whether `id`'s subtree can never paint anything: no non-whitespace text
/// outside metadata elements. MediaWiki parks
/// `<span class="mw-empty-elt"><link/><link/></span>` wrappers between its
/// floated infobox and the sidebar tables — the links are ResourceLoader
/// hints, not content — so a bare has-children check misreads the wrappers
/// as flow content and stops the same-side float rail from forming.
fn subtree_paints_nothing(tree: &DomTree, id: NodeId) -> bool {
    enum Step {
        Paints,
        Skip,
        Descend,
    }
    let mut stack = vec![id];
    while let Some(cur) = stack.pop() {
        let Some(step) = tree.with_node(cur, |n| match &n.data {
            diting_dom::NodeData::Text { contents } => {
                if contents.trim().is_empty() {
                    Step::Skip
                } else {
                    Step::Paints
                }
            }
            diting_dom::NodeData::Element { name, .. } => {
                if matches!(
                    name.local.as_str(),
                    "link" | "meta" | "style" | "script" | "template"
                ) {
                    Step::Skip
                } else {
                    Step::Descend
                }
            }
            _ => Step::Skip,
        }) else {
            return false;
        };
        match step {
            Step::Paints => return false,
            Step::Skip => {}
            Step::Descend => {
                for child in render_children(tree, cur) {
                    stack.push(child);
                }
            }
        }
    }
    true
}

/// The composed-tree children that render under `id` (shadow DOM phase 2):
/// a shadow host renders its shadow tree's children instead of its light
/// children, and a `<slot>` renders its assigned light children — or, with
/// nothing assigned, its own fallback children. The slot element itself
/// never builds a box (display:contents equivalent). Everything else passes
/// through unchanged; light-tree walks elsewhere must stay tree-scoped.
fn render_children(tree: &DomTree, id: NodeId) -> Vec<NodeId> {
    let raw: Vec<NodeId> = match tree.shadow_root(id) {
        Some(root) => tree.children(root),
        None => tree.children(id),
    };
    let mut out = Vec::with_capacity(raw.len());
    for child in raw {
        if tree.is_html_slot_element(child) {
            match tree.assigned_nodes(child) {
                Some(assigned) if !assigned.is_empty() => out.extend(assigned),
                _ => out.extend(render_children(tree, child)),
            }
        } else {
            out.push(child);
        }
    }
    out
}

/// Text of every `<style>` element inside a shadow tree. Serialized HTML
/// never contains shadow trees, so only the live-tree CSS collection
/// (ops.rs) calls this. Hosts are visited in document order so
/// equal-specificity shadow rules stay deterministic; a HashMap iteration
/// over the registry would reshuffle them per run.
pub fn shadow_style_texts(tree: &diting_dom::DomTree) -> Vec<String> {
    fn collect_into(tree: &diting_dom::DomTree, id: diting_dom::NodeId, out: &mut Vec<String>) {
        if let Some(root) = tree.shadow_root(id) {
            for desc in tree.descendants(root) {
                let is_style = tree
                    .with_node(desc, |n| {
                        n.as_element().is_some_and(|e| e.local.as_str() == "style")
                    })
                    .unwrap_or(false);
                if is_style {
                    out.push(tree.text_content(desc));
                }
                // Nested hosts inside this shadow tree.
                if tree.shadow_root(desc).is_some() {
                    collect_into(tree, desc, out);
                }
            }
        }
        for child in tree.children(id) {
            collect_into(tree, child, out);
        }
    }
    let mut out = Vec::new();
    collect_into(tree, tree.document(), &mut out);
    out
}

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

/// Lay a DOM tree out at a fixed viewport size and return each element's
/// absolute border-box rect. Elements are keyed by diting_dom NodeId; the
/// root's containing block is the viewport (a definite-sized root taffy
/// node, so bottom/percentage insets on fixed boxes resolve against it —
/// obscura#675 lineage fix).
pub fn layout_dom(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
) -> HashMap<NodeId, Rect> {
    layout_dom_with_paint(tree, styles, fonts, viewport_width, viewport_height).0
}

/// [`layout_dom`] plus the paint item list in document order (an element's
/// `Bg` precedes the items of its subtree, so a solid background lands
/// under its descendants' ink).
pub fn layout_dom_with_paint(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
) -> (HashMap<NodeId, Rect>, Vec<PaintItem>) {
    layout_dom_with_paint_and_images(
        tree,
        styles,
        fonts,
        viewport_width,
        viewport_height,
        None,
        None,
    )
}

/// [`layout_dom_with_paint`] with an injected table of fetched image bodies
/// (batch 6c): absolute `http(s)`/`file` URL → response body. `<img src>`
/// entries pointing at those URLs decode like data: URLs (PNG only); misses
/// keep the placeholder. The fetch itself is the caller's job — the screenshot
/// prefetch pass fills this from diting_net. `base_url` is the document URL
/// relative srcs resolve against to reach the table's absolute keys (same
/// join the fetchers used).
#[allow(clippy::too_many_arguments)]
pub fn layout_dom_with_paint_and_images(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
) -> (HashMap<NodeId, Rect>, Vec<PaintItem>) {
    let (rects, items, _order, _local_geom, _sticky_spans, _scroller_spans) =
        layout_dom_with_paint_order_and_images(
            tree,
            styles,
            fonts,
            viewport_width,
            viewport_height,
            network_bytes,
            base_url,
        );
    (rects, items)
}

/// Map a rect through a CSS-order affine array [a,b,c,d,e,f]: the diagonal
/// fast path keeps the historical two-corner normalize (negative scales
/// mirror); anything else maps all four corners and unions them — Chrome's
/// gBCR bounding-box behavior under rotation. Free-standing so
/// [`expand_wrapped_leaves`] can map through a recorded array — the walk's
/// `Xf` type is fn-local.
fn map_rect_arr(m: [f32; 6], r: Rect) -> Rect {
    if m[1] == 0.0 && m[2] == 0.0 {
        let (x1, x2) = (r.x * m[0] + m[4], (r.x + r.width) * m[0] + m[4]);
        let (y1, y2) = (r.y * m[3] + m[5], (r.y + r.height) * m[3] + m[5]);
        Rect {
            x: x1.min(x2),
            y: y1.min(y2),
            width: (x2 - x1).abs(),
            height: (y2 - y1).abs(),
        }
    } else {
        let pt = |x: f32, y: f32| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (x0, y0) = pt(r.x, r.y);
        let (x1, y1) = pt(r.x + r.width, r.y);
        let (x2, y2) = pt(r.x, r.y + r.height);
        let (x3, y3) = pt(r.x + r.width, r.y + r.height);
        let xs = [x0, x1, x2, x3];
        let ys = [y0, y1, y2, y3];
        let min_x = xs.iter().cloned().fold(f32::MAX, f32::min);
        let max_x = xs.iter().cloned().fold(f32::MIN, f32::max);
        let min_y = ys.iter().cloned().fold(f32::MAX, f32::min);
        let max_y = ys.iter().cloned().fold(f32::MIN, f32::max);
        Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }
}

/// Collect the canvas-space rects of everything a flattened inline hoisted:
/// descend through run wrappers (nested flattening wraps wrapper into
/// wrapper) and stop at leaves / boxed nodes, which carry their LOCAL rect
/// plus the accumulated map in `local_by_node` and a first-item index in
/// `node_first_item`. A wrapped run leaf is one taffy box covering every
/// line it broke into — split it with the same greedy-wrap truth the paint
/// path uses (in LOCAL space, where the wrap width still means what the
/// paint item's `wrap_at` means), then map each line band to canvas —
/// under rotation a horizontal line band is a vertical stripe, and the
/// mapped bbox of each band is the closest an axis-aligned Bg can get.
fn expand_wrapped_leaves(
    node: diting_taffy::tree::NodeId,
    taffy_tree: &TaffyTree<TextLeaf>,
    wrapper_set: &std::collections::HashSet<diting_taffy::tree::NodeId>,
    local_by_node: &HashMap<diting_taffy::tree::NodeId, (Rect, [f32; 6])>,
    fonts: &FontBook,
    pieces: &mut Vec<Rect>,
    owners: &mut Vec<diting_taffy::tree::NodeId>,
) {
    if wrapper_set.contains(&node) {
        for c in taffy_tree.children(node).unwrap_or_default() {
            expand_wrapped_leaves(
                c,
                taffy_tree,
                wrapper_set,
                local_by_node,
                fonts,
                pieces,
                owners,
            );
        }
        return;
    }
    let Some((r, m)) = local_by_node.get(&node) else {
        return;
    };
    if let Some(TextLeaf::Run {
        text,
        font_size,
        bold,
        line_height,
        mono,
        word_spacing,
        ws,
        line_clamp,
        tokens,
        small_caps,
        han,
        ..
    }) = taffy_tree.get_node_context(node)
    {
        let tokens = run_tokens(
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
        // no-soft-wrap modes keep their single line here too: the affine
        // path expands a leaf into per-line tiles, and wrapping such a run
        // at the box width would split ink the rasterizer lays on one line.
        let mut lines = text::greedy_wrap(
            &tokens,
            if ws.no_soft_wrap() {
                None
            } else {
                Some(r.width.max(0.0))
            },
            *ws,
        );
        // line-clamp (#271): band pieces stop at the Nth line — the same
        // count-only rule the measure path applies, so an inline span's
        // background never bands over the clamped-out lines.
        if let Some(n) = line_clamp.filter(|n| lines.len() > *n as usize) {
            lines.truncate(n as usize);
        }
        for (i, line) in lines.iter().enumerate() {
            if line.width <= 0.0 {
                continue;
            }
            pieces.push(map_rect_arr(
                *m,
                Rect {
                    x: r.x,
                    y: r.y + i as f32 * line_height,
                    width: line.width,
                    height: *line_height,
                },
            ));
            owners.push(node);
        }
        return;
    }
    pieces.push(map_rect_arr(*m, *r));
    owners.push(node);
}

/// Baseline of a text line box below its top edge, from real font metrics
/// (the same quantized model the paint path rasterizes against).
fn text_baseline(fonts: &FontBook, font_size: f32, bold: bool, line_height: f32) -> f32 {
    let m = fonts
        .metrics(font_size, bold)
        .unwrap_or(text::ScaledMetrics {
            ascent: font_size,
            descent: font_size * 0.2,
            line_gap: 0.0,
        });
    text::baseline_offset(m.ascent, m.descent, line_height)
}

/// The LAST in-flow line baseline inside an atomic inline-level box (CSS2:
/// an inline-block baseline-aligns to the baseline of its last in-flow line
/// box), as a y offset from the box's border-box top. `rel_y` accumulates
/// taffy locations plus already-computed baseline shifts (inner runs are
/// processed before outer ones). Returns None when the subtree carries no
/// text — the caller then falls back to the bottom edge, which is the CSS
/// behavior for a box with no in-flow line boxes.
fn subtree_last_baseline(
    taffy_tree: &TaffyTree<TextLeaf>,
    node: diting_taffy::tree::NodeId,
    rel_y: f32,
    shifts: &HashMap<diting_taffy::tree::NodeId, f32>,
    fonts: &FontBook,
    best: &mut Option<f32>,
) {
    let Ok(layout) = taffy_tree.layout(node) else {
        return;
    };
    let rel_y = rel_y + shifts.get(&node).copied().unwrap_or(0.0);
    match taffy_tree.get_node_context(node) {
        Some(TextLeaf::Word {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => {
            if layout.size.height > 0.0 {
                let b =
                    rel_y + text_baseline(fonts, *font_size, *bold, *line_height) - baseline_shift;
                *best = Some(best.map_or(b, |x: f32| x.max(b)));
            }
            return;
        }
        Some(TextLeaf::Run {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => {
            // A wrapped run's last line sits one line box above its bottom;
            // the shift pad is empty reservation, not a text line.
            let pad = shift_pad(*baseline_shift, leaf_descent(fonts, *font_size, *bold));
            let b = rel_y + layout.size.height - pad - line_height
                + text_baseline(fonts, *font_size, *bold, *line_height)
                - baseline_shift;
            *best = Some(best.map_or(b, |x: f32| x.max(b)));
            return;
        }
        Some(TextLeaf::Replaced { strut_descent }) => {
            // Baseline = the element box's bottom edge (the pad below it is
            // strut reservation, not element box).
            let b = rel_y + layout.size.height - strut_descent;
            *best = Some(best.map_or(b, |x: f32| x.max(b)));
            return;
        }
        // A pseudo box is textless (#172): its children (none) hold no
        // baseline, the bottom-edge fallback applies.
        Some(TextLeaf::Pseudo { .. }) | None => {}
    }
    for child in taffy_tree.children(node).unwrap_or_default() {
        let Ok(cl) = taffy_tree.layout(child) else {
            continue;
        };
        subtree_last_baseline(
            taffy_tree,
            child,
            rel_y + cl.location.y,
            shifts,
            fonts,
            best,
        );
    }
}

/// Baseline alignment for inline runs (blitz#750 family): CSS aligns the
/// boxes of one line by their baselines, but taffy leaves cannot report
/// baselines (the measure closure returns Size only — still true at the
/// taffy rev blitz's own baseline PR pins), so run wrappers lay out
/// FLEX_START and we shift afterwards. Wrapping is width-driven, so the
/// flex lines taffy already formed ARE the CSS line boxes; per line every
/// item drops by (line max baseline − its own baseline). For uniform-font
/// runs every dy is zero, leaving existing geometry bit-identical — only
/// runs mixing font metrics or atomic boxes move.
fn compute_baseline_shifts(
    taffy_tree: &TaffyTree<TextLeaf>,
    run_wrappers: &[diting_taffy::tree::NodeId],
    fonts: &FontBook,
) -> HashMap<diting_taffy::tree::NodeId, f32> {
    let mut shifts: HashMap<diting_taffy::tree::NodeId, f32> = HashMap::new();
    // First-line baselines of processed wrappers: a nested wrapper (flattened
    // inline content) joins an outer run as ONE item and, as a flex
    // container, contributes its FIRST baseline (css-align, blitz#750's
    // Display::Flex arm).
    let mut first_baselines: HashMap<diting_taffy::tree::NodeId, f32> = HashMap::new();
    // Inner wrappers first: outer runs read their shifts and first baselines.
    let depth = |mut n: diting_taffy::tree::NodeId| {
        let mut d = 0usize;
        while let Some(p) = taffy_tree.parent(n) {
            d += 1;
            n = p;
        }
        d
    };
    let mut ordered = run_wrappers.to_vec();
    ordered.sort_by_key(|w| std::cmp::Reverse(depth(*w)));

    for wrapper in ordered {
        let children = taffy_tree.children(wrapper).unwrap_or_default().to_vec();
        if children.len() < 2 {
            // A lone item aligns to itself; nothing to shift. Still record
            // the first baseline for an enclosing run.
            if let Some(&only) = children.first() {
                if let Ok(l) = taffy_tree.layout(only) {
                    if l.size.height > 0.0 {
                        let b = item_baseline(
                            taffy_tree,
                            only,
                            &shifts,
                            &first_baselines,
                            fonts,
                            l.size.height,
                        );
                        first_baselines.insert(wrapper, b);
                    }
                }
            }
            continue;
        }
        // Group into flex lines by y (taffy rounds locations to the pixel
        // grid, so same-line items share a y).
        let mut lines: Vec<(f32, Vec<(diting_taffy::tree::NodeId, f32)>)> = Vec::new();
        for child in children {
            let Ok(l) = taffy_tree.layout(child) else {
                continue;
            };
            // Whitespace-only word leaves carry no height and no ink.
            if l.size.height <= 0.0 {
                continue;
            }
            let b = item_baseline(
                taffy_tree,
                child,
                &shifts,
                &first_baselines,
                fonts,
                l.size.height,
            );
            match lines
                .iter_mut()
                .find(|(y, _)| (*y - l.location.y).abs() < 0.5)
            {
                Some((_, items)) => items.push((child, b)),
                None => lines.push((l.location.y, vec![(child, b)])),
            }
        }
        lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((_, items)) = lines.first() {
            let b = items.iter().fold(0.0f32, |m, (_, b)| m.max(*b));
            first_baselines.insert(wrapper, b);
        }
        for (_, items) in lines {
            let line_b = items.iter().fold(0.0f32, |m, (_, b)| m.max(*b));
            for (child, b) in items {
                let dy = line_b - b;
                if dy > 0.01 {
                    shifts.insert(child, dy);
                }
            }
        }
    }
    shifts
}

/// Baseline of one run item below its border-box top.
fn item_baseline(
    taffy_tree: &TaffyTree<TextLeaf>,
    child: diting_taffy::tree::NodeId,
    shifts: &HashMap<diting_taffy::tree::NodeId, f32>,
    first_baselines: &HashMap<diting_taffy::tree::NodeId, f32>,
    fonts: &FontBook,
    height: f32,
) -> f32 {
    match taffy_tree.get_node_context(child) {
        Some(TextLeaf::Word {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => text_baseline(fonts, *font_size, *bold, *line_height) - baseline_shift,
        Some(TextLeaf::Run {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => {
            let pad = shift_pad(*baseline_shift, leaf_descent(fonts, *font_size, *bold));
            height - pad - line_height + text_baseline(fonts, *font_size, *bold, *line_height)
                - baseline_shift
        }
        Some(TextLeaf::Replaced { strut_descent }) => height - strut_descent,
        // A pseudo box is textless (#172): bottom-edge fallback, same as a
        // textless box.
        Some(TextLeaf::Pseudo { .. }) | None => {
            if let Some(b) = first_baselines.get(&child) {
                return *b;
            }
            if taffy_tree.children(child).map_or(0, |c| c.len()) > 0 {
                // Atomic inline-level box: last in-flow line baseline.
                let mut best = None;
                subtree_last_baseline(taffy_tree, child, 0.0, shifts, fonts, &mut best);
                if let Some(b) = best {
                    return b;
                }
            }
            // Textless box: bottom edge (CSS bottom margin-edge fallback —
            // run items carry no vertical margins in this model).
            height
        }
    }
}

/// [`layout_dom_with_paint_and_images`] plus the paint order: every boxed
/// element in the sequence the flat item list paints it, so the LAST entry
/// whose rect contains a point is the element a pixel there shows. This is
/// the ranking `elementFromPoint` needs — document order is not paint order
/// once positioned siblings with z-index hoist out of it (obscura #738).
/// Boxless flattened inline wrappers (span/a/label — obscura#722 lineage)
/// are absent; hit testing ranks them with their nearest boxed ancestor,
/// which paints their hoisted ink in-flow anyway.
pub fn layout_dom_with_paint_order_and_images(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
) -> LayoutCollect {
    let solved = layout_solve(
        tree,
        styles,
        fonts,
        viewport_width,
        viewport_height,
        network_bytes,
        base_url,
    );
    layout_collect(tree, styles, fonts, &solved, viewport_width)
}

/// The solved half of a layout run, split out for #395: the taffy tree after
/// every pass (build, static-position harvest, reparent, main solve, float
/// continuation, calc repair, table span placement) plus the auxiliary maps
/// the collect walk consumes. Paint-only property writes — transform and
/// opacity, everything a GSAP-style timeline seek lands — are invisible to
/// all of it: `to_taffy_style` maps only geometry families, the solve passes
/// consume only those, and baseline shifts read pure layout. A fresh
/// [`layout_collect`] over a cached [`SolvedGeometry`] is therefore
/// bit-identical to re-running the whole pipeline — which is what lets the
/// video pump repaint per-seek frames without the full-page relayout it used
/// to pay (43 solves × 114ms on the probe page).
pub struct SolvedGeometry {
    taffy_tree: TaffyTree<TextLeaf>,
    node_map: HashMap<diting_taffy::tree::NodeId, NodeId>,
    images: HashMap<NodeId, DecodedImage>,
    static_pos: HashMap<NodeId, (f32, f32)>,
    baseline_shifts: HashMap<diting_taffy::tree::NodeId, f32>,
    run_wrappers: Vec<diting_taffy::tree::NodeId>,
    collapsed_edges: HashMap<NodeId, [bool; 4]>,
    flattened: HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    /// The synthetic ICB the walk roots at; `None` marks an aborted solve
    /// (no element root, or a taffy error) — collect returns the same empty
    /// outputs the undivided pipeline returned on those paths.
    icb_node: Option<diting_taffy::tree::NodeId>,
    /// Which DOM text node(s) each text leaf carries (range / client-rect queries).
    text_src: text_src::SrcMap,
}

impl SolvedGeometry {
    /// DOM-side solved border boxes (DOM id → (w, h)): the container-query
    /// pass reads candidate container widths from here without a full
    /// collect. Display:none nodes have no taffy box and are absent — a
    /// missing box means "not queryable", matching the containment contract.
    pub fn dom_boxes(&self) -> HashMap<NodeId, (f32, f32)> {
        self.node_map
            .iter()
            .filter_map(|(t, d)| {
                self.taffy_tree
                    .layout(*t)
                    .ok()
                    .map(|l| (*d, (l.size.width, l.size.height)))
            })
            .collect()
    }

    /// The empty solve the early-return paths hand back: nothing walked,
    /// nothing collected.
    fn aborted() -> Self {
        SolvedGeometry {
            taffy_tree: TaffyTree::new(),
            node_map: HashMap::new(),
            images: HashMap::new(),
            static_pos: HashMap::new(),
            baseline_shifts: HashMap::new(),
            run_wrappers: Vec::new(),
            collapsed_edges: HashMap::new(),
            flattened: HashMap::new(),
            icb_node: None,
            text_src: Default::default(),
        }
    }
}

/// Solve the taffy tree for the live DOM (the first half of what
/// [`layout_dom_with_paint_order_and_images`] used to do in one body): build
/// the node tree, harvest static positions, reparent out-of-flow boxes onto
/// their containing blocks, run the block/flex/grid solve, then the
/// float-continuation, calc-repair and table-span fixups that re-solve when
/// they adjust geometry. See [`SolvedGeometry`] for why the result is worth
/// caching separately from the collected items.
pub fn layout_solve(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
) -> SolvedGeometry {
    text_src::reset();
    float_lines::reset();
    // The document node is not an element; lay out from the first element
    // descendant (the <html> root), like upstream.
    let root = tree
        .children(tree.document())
        .into_iter()
        .find(|id| tree.with_node(*id, |n| n.is_element()).unwrap_or(false));
    layout_solve_rooted(
        tree,
        styles,
        fonts,
        viewport_width,
        viewport_height,
        network_bytes,
        base_url,
        root,
    )
}

/// Explicit-root variant (fabricated iframe documents): `root` is the
/// orphan subtree's topmost element — the whole downstream build
/// (build_element walk, layout_collect) runs from it, and the ICB is that
/// subtree's own viewport.
#[allow(clippy::too_many_arguments)]
pub fn layout_solve_rooted(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
    root: Option<NodeId>,
) -> SolvedGeometry {
    let mut taffy_tree = TaffyTree::new();
    let mut node_map: HashMap<diting_taffy::tree::NodeId, NodeId> = HashMap::new();

    // Pre-pass (batches 5b + 6c, parallelized by #112): resolve every img
    // src through the ImageCache — data: URLs decode inline, http(s)/file
    // URLs consult the fetched-byte table; results are cached so repeated
    // srcs and layout re-runs decode once. Unresolvable imgs keep the
    // batch-5a placeholder. #112: the decodes used to run inline in
    // scan_images — serial, one image at a time on the layout thread, and
    // invisible to the layout trace (which times styles/taffy, not this
    // pass) — so an image-heavy page's screenshot wall clock was ~all
    // decode. Collect the page's unique srcs first, decode them across the
    // cores (ImageCache::prefetch), and the walk below only clones cached
    // Arcs.
    let empty: HashMap<String, std::sync::Arc<Vec<u8>>> = HashMap::new();
    let cache = image::ImageCache::with_network(network_bytes.unwrap_or(&empty));
    let mut images: HashMap<NodeId, DecodedImage> = HashMap::new();
    fn collect_img_srcs(
        tree: &DomTree,
        id: NodeId,
        srcs: &mut HashSet<String>,
        viewport_width: f32,
        base_url: Option<&str>,
    ) {
        let is_img = tree
            .with_node(id, |n| n.as_element().map(|e| e.local.to_string() == "img"))
            .flatten()
            .unwrap_or(false);
        if is_img {
            if let Some(src) = resolve_img_source(tree, id, viewport_width, base_url) {
                srcs.insert(src);
            }
        }
        for child in render_children(tree, id) {
            collect_img_srcs(tree, child, srcs, viewport_width, base_url);
        }
    }
    fn scan_images(
        tree: &DomTree,
        id: NodeId,
        cache: &image::ImageCache,
        images: &mut HashMap<NodeId, DecodedImage>,
        viewport_width: f32,
        base_url: Option<&str>,
    ) {
        let is_img = tree
            .with_node(id, |n| n.as_element().map(|e| e.local.to_string() == "img"))
            .flatten()
            .unwrap_or(false);
        if is_img {
            let src = resolve_img_source(tree, id, viewport_width, base_url);
            if let Some(img) = src.as_deref().and_then(|s| cache.resolve(s)) {
                images.insert(id, (*img).clone());
            }
        }
        for child in render_children(tree, id) {
            scan_images(tree, child, cache, images, viewport_width, base_url);
        }
    }
    if let Some(root_id) = &root {
        let mut srcs: HashSet<String> = HashSet::new();
        collect_img_srcs(tree, *root_id, &mut srcs, viewport_width, base_url);
        cache.prefetch(&srcs.into_iter().collect::<Vec<_>>());
        scan_images(
            tree,
            *root_id,
            &cache,
            &mut images,
            viewport_width,
            base_url,
        );
    }

    // Flattened inline wrappers (see build_element) recorded as
    // dom id → hoisted taffy children, for the union pass after the walk.
    let mut flattened: HashMap<NodeId, Vec<diting_taffy::tree::NodeId>> = HashMap::new();
    // Run wrappers (the IFC stand-in flex rows) recorded at assembly for the
    // post-layout baseline-alignment pass (blitz#750 family) — they can't be
    // re-identified structurally (inline-block boxes share the same taffy
    // style), so the builders log them as they create them.
    let mut run_wrappers: Vec<diting_taffy::tree::NodeId> = Vec::new();
    // Table span bookkeeping (rowspan/colspan): build_table logs each spanned
    // table here; the post-layout fixup reads the placeholder geometry and
    // places the lifted absolute cells over their spanned band.
    let mut table_meta = TableBuildMeta::default();
    let Some(root_id) = root else {
        return SolvedGeometry::aborted();
    };
    let Some(root_node) = build_element(
        tree,
        root_id,
        styles,
        &images,
        fonts,
        &mut taffy_tree,
        &mut node_map,
        &mut flattened,
        &mut run_wrappers,
        &mut table_meta,
    ) else {
        return SolvedGeometry::aborted();
    };

    // The initial containing block (obscura#675 lineage fix): CSS anchors a
    // fixed box's insets — and every percentage that bottoms out at the root
    // — to the VIEWPORT, a notional box DISTINCT from the root element's own
    // content-driven box (blitz keeps html at content height too; conflating
    // the two made the cross-check read html=600 vs blitz=140). Wrap the
    // root element in a synthetic definite viewport-sized taffy node; the
    // reparent pass below lands fixed boxes — and ancestor-less absolutes,
    // whose containing block is also the ICB — on it, so taffy's inset math
    // has a real viewport to resolve against. The wrapper has no DOM node,
    // so collect/paint walks keyed by node_map skip it. Children taller
    // than the viewport overflow visibly; taffy doesn't clip.
    let icb_node = taffy_tree
        .new_leaf(Style {
            // Block, not taffy's Flex default: the root element must stay a
            // stretching BLOCK child (a flex item would shrink html to
            // max-content — probe: 500px child → html=500 instead of VW).
            display: Display::Block,
            size: Size {
                width: diting_taffy::style::Dimension::length(viewport_width),
                height: diting_taffy::style::Dimension::length(viewport_height),
            },
            ..Default::default()
        })
        .expect("synthetic ICB node");
    let _ = taffy_tree.add_child(icb_node, root_node);

    let available = Size {
        width: AvailableSpace::Definite(viewport_width),
        height: AvailableSpace::MaxContent,
    };

    let static_pos = harvest_static_positions(
        tree,
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    );
    reparent_out_of_flow(tree, styles, &mut taffy_tree, &node_map, icb_node, root);
    // Measured-leaf dispatch (batch 3a): pure-text runs carry a TextLeaf
    // context and measure through the FontBook; every other leaf must fall
    // through to taffy's own style-based sizing — the closure fires for ALL
    // childless nodes, so returning HIDDEN here would zero plain leaves
    // (that's exactly what stock compute_layout does below via the same fn).
    let measured = taffy_tree.compute_layout_with_block_measure(
        icb_node,
        available,
        |inputs, id, ctx, style, block| {
            float_lines::measure_node(fonts, inputs, id, ctx, style, block)
        },
    );
    if measured.is_err() {
        return SolvedGeometry::aborted();
    }

    if continue_floats(
        tree,
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    )
    .is_err()
    {
        return SolvedGeometry::aborted();
    }
    let Ok(mut repaired) = repair_calc_widths(
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    ) else {
        return SolvedGeometry::aborted();
    };
    if repair_box_sizes(
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
        &mut repaired,
    )
    .is_err()
    {
        return SolvedGeometry::aborted();
    }
    if repair_wrap_host_intrinsics(
        tree,
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    )
    .is_err()
    {
        return SolvedGeometry::aborted();
    }
    if place_table_spans(fonts, &mut taffy_tree, icb_node, available, &table_meta).is_err() {
        return SolvedGeometry::aborted();
    }
    // Both trees round to the pixel grid inside compute_layout (taffy's
    // use_rounding defaults on; blitz rounds via the same path), so the
    // rect comparisons assume integer edges on both sides.

    // Baseline alignment of inline runs (blitz#750 family): per-line shifts
    // computed against the FINAL layout, applied during the collect walk.
    let baseline_shifts = compute_baseline_shifts(&taffy_tree, &run_wrappers, fonts);
    // Layout is final: the result caches only cost memory from here on.
    taffy_tree.drop_caches();

    SolvedGeometry {
        taffy_tree,
        node_map,
        images,
        static_pos,
        baseline_shifts,
        run_wrappers,
        collapsed_edges: table_meta.collapsed_edges,
        flattened,
        icb_node: Some(icb_node),
        text_src: text_src::take(),
    }
}

#[cfg(test)]
#[cfg(test)]
mod absorption_pins;
#[cfg(test)]
mod fork_deltas;
#[cfg(test)]
mod fork_table_deltas;

// (The dual-engine bridge_cross_check tests lived in the product crate until
// the blitz reference pipeline was deleted 2026-11-06 — diting carries zero
// blitz references and never did import any; ARCHITECTURE.md §2 rule R2.)

#[cfg(test)]
mod tests;
