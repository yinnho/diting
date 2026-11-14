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

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use diting_taffy::prelude::*;

#[cfg(test)]
use diting_css::TextDecorations;
#[cfg(test)]
use diting_css::{TextAlign, WhiteSpace};
#[cfg(test)]
use diting_dom::tree::DomTree;
#[cfg(test)]
use diting_dom::tree::NodeId;
#[cfg(test)]
use flow_tree::FlowTree as TaffyTree;

// Bundled CJK font supply for /screenshot determinism (batch 3c). Lives in
// this crate since #58: fonts and layout reference each other (FontBook /
// FaceBytes / Han slots), so they compile as one unit.
mod abs_rects;
mod clip_text;
mod collect;
mod counters;
pub mod diting_fonts;
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
mod pseudo;
pub mod sticky;
pub mod svg;
mod table;
pub mod text;
pub mod text_geom;
mod text_src;
#[cfg(test)]
pub(crate) use counters::q_quote_pair;
pub use face_bytes::FaceBytes;
pub use image::DecodedImage;
pub use paint_items::{FormRun, FormWidget, PaintItem, TextGradient};
pub use sticky::LayoutCollect;
pub use sticky::{apply_sticky_to_items, sticky_axis_shift, sticky_inset_px, StickySpan};
pub use text::FontBook;
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
pub use composed::shadow_style_texts;
#[cfg(test)]
use container_units::bake_container_units;
pub use layout_collect::{
    body_overflow_propagates, effective_viewport_overflow, layout_collect,
    layout_collect_with_images,
};
pub use layout_solve::{
    layout_dom, layout_dom_with_paint, layout_dom_with_paint_and_images,
    layout_dom_with_paint_order_and_images, layout_solve, layout_solve_rooted, SolvedGeometry,
};
pub use replaced::{media_matches_width, object_paint_rect, resolve_img_source};
pub use style_memo::clear_style_memo;
#[cfg(test)]
use text_run::{
    build_word_leaves, leaf_descent, measure_text_leaf, run_tokens, shift_pad, TextLeaf,
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
