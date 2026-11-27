//! Small shared SVG/text helpers for the docgen family adapters.
//!
//! The adapters differ in geometry and semantics; byte escaping and the
//! fixed-column text fit law do not. Keeping those two here stops four
//! family renderers from drifting on identical utility code.

use super::spec::text_units;

/// 0.6px advance per text unit per px of font size, in tenths.
pub(super) const TEXT_WIDTH_FACTOR: i32 = 6;

/// Fit a single text line inside `available` tenths-of-pixels, clamped to
/// the family's preferred/minimum font sizes.
pub(super) fn fitted_font(text: &str, available: i32, preferred: i32, minimum: i32) -> i32 {
    let units = text_units(text) as i32;
    let by_width = available * 10 / (units * TEXT_WIDTH_FACTOR);
    preferred.min(by_width).max(minimum)
}

/// XML-escape text after flattening control whitespace to spaces.
pub(super) fn esc(s: &str) -> String {
    let flat = s.replace(['\n', '\r', '\t'], " ");
    flat.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
