//! DOM → box tree → geometry: diting's layout stage (screenshot feature).
//!
//! The crate root keeps `Rect` and re-exports the public entry points.
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
pub use paint_items::{FormRun, FormWidget, PaintItem, TextGradient};
use sticky::establishes_stacking_context;
pub use sticky::LayoutCollect;
pub use sticky::{apply_sticky_to_items, sticky_axis_shift, sticky_inset_px, StickySpan};
use table::{build_table, place_table_spans, TableBuildMeta};
pub use text::FontBook;
use text_src::trim_run_edge_whitespace;
mod cascade;
mod composed;
mod container_units;
mod element_build;
mod float_zone;
mod flow_column;
mod layout_collect;
mod layout_solve;
mod paint_items;
mod replaced;
mod solve_fixup;
mod style_map;
mod style_memo;
#[cfg(test)]
mod style_memo_tests;
mod text_run;
pub use cascade::{
    compute_styles, compute_styles_gated, compute_styles_timed, compute_styles_timed_within,
    container_plan, ContainerPlan,
};
use composed::render_children;
pub use composed::shadow_style_texts;
use container_units::bake_container_units;
use element_build::{build_element, build_normal_sibling, strut_descent_for, RunSeg};
use float_zone::build_float_zone;
use flow_column::{build_flow_column, subtree_paints_nothing};
pub use layout_collect::{
    body_overflow_propagates, effective_viewport_overflow, layout_collect,
    layout_collect_with_images,
};
use layout_solve::{expand_wrapped_leaves, map_rect_arr};
pub use layout_solve::{
    layout_dom, layout_dom_with_paint, layout_dom_with_paint_and_images,
    layout_dom_with_paint_order_and_images, layout_solve, layout_solve_rooted, SolvedGeometry,
};
use replaced::{
    build_replaced_leaf, bust_subtree_cache, is_replaced_tag, pct_height_resolves, taffy_depth,
};
pub use replaced::{media_matches_width, object_paint_rect, resolve_img_source};
use solve_fixup::{
    continue_floats, harvest_static_positions, repair_box_sizes, repair_calc_widths,
    repair_wrap_host_intrinsics, reparent_out_of_flow, SolveAborted,
};
use style_map::{resolve_sizing_keywords, side_px, to_taffy_style};
pub use style_memo::clear_style_memo;
use text_run::{
    blockifies_for_block_child, br_strut_leaf, build_word_leaves, clamp_corner_radii,
    collapse_adjacent_sibling_margins, color_context, decoration_context, effective_line_height,
    fix_flex_basis_carry, font_context, form_control_run, han_context, ifc_white_space,
    is_alignment_promote, leaf_descent, measure_text_leaf, mono_context, own_decorations,
    pin_leaf_widths, run_tokens, run_wrapper_style, selected_option_label, shift_pad,
    small_caps_context, stretch_block_children_in_aligned_parent, text_transform_context,
    tokenize_ws, valign_shift, word_spacing_context, ws_rank, TextLeaf,
};

/// Absolute (page-relative) border-box rect of a DOM element after layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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
