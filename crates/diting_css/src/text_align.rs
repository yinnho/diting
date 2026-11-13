//! Text alignment, direction, and transform keywords.
//!
//! Split from the css hub. The public names stay re-exported from the
//! crate root. Pure move.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    /// Logical edges (css-text-3): resolved against the element's own
    /// [`TextDirection`] at consume time. `Start` is also the property's
    /// initial value, so an undeclared text-align rides the same
    /// resolution (#188-3).
    Start,
    End,
    /// Parsed and reported by the computed face, but line stretching is
    /// NOT painted (#214): layout consumes justify as start (no promote),
    /// which is also exactly the canvas `text-align-last` needs to move
    /// the last line off.
    Justify,
}

/// `text-align-last` (css-text-3 §8.2, #214, blitz#998), inherited; the
/// initial value `auto` defers to `text-align` (no override). Consumed
/// paint-side as a per-run last-line x offset — see
/// [`ComputedStyle::physical_text_align_last`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignLast {
    Auto,
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

/// CSS Writing Modes §2.1: the inline base direction (`direction`),
/// inherited. Consumed faces (#188-3, blitz#981): logical text-align
/// resolution and RTL anchoring of text-entry control values. No UAX #9
/// glyph reordering happens — same-direction content lands exactly, mixed
/// runs keep logical order; `unicode-bidi` is not modeled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    Ltr,
    Rtl,
}

/// `text-transform` case mapping (CSS Text §6.1) — rendering-only; the DOM
/// text is never rewritten. The layout-side context walker resolves
/// inheritance (`small_caps_context` precedent), so the `None` variant
/// carries "declared `none`", which resets an ancestor's transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}
