//! Parsers for `box-shadow`, `text-shadow`, `mix-blend-mode`, backdrop blur.
//!
//! Split from the css hub. The public names stay re-exported from the
//! crate root. Pure move.

use super::*;

/// `box-shadow` v1 parser (blitz#349 family): comma-separated `<shadow>`
/// layers, each `<color>? && <length>{2,4} && inset?` in any order. Two
/// lengths minimum (dx, dy, then blur, then spread); a negative blur
/// invalidates the layer, a negative spread is legal. A missing color folds
/// against the `color` value cascaded so far (the spec's order-dependent
/// currentColor rule). `inset` layers parse and report through the CSSOM
/// but never reach the paint half — documented v1 divergence.
pub fn parse_box_shadow(
    value: &str,
    current_color: Option<Color>,
    fonts: &FontCtx,
) -> Option<Vec<BoxShadow>> {
    let mut layers = Vec::new();
    for part in split_top_level_commas(value) {
        let mut inset = false;
        let mut color: Option<Color> = None;
        let mut lengths: Vec<f32> = Vec::new();
        for tok in split_top_level_units(&part) {
            if tok.eq_ignore_ascii_case("inset") {
                if inset {
                    return None;
                }
                inset = true;
            } else if let Some(c) = parse_color(tok) {
                if color.is_some() {
                    return None;
                }
                color = Some(c);
            } else {
                if lengths.len() >= 4 {
                    return None;
                }
                lengths.push(resolve_shadow_len(tok, fonts)?);
            }
        }
        if lengths.len() < 2 {
            return None;
        }
        let blur = lengths.get(2).copied().unwrap_or(0.0);
        if blur < 0.0 {
            return None;
        }
        layers.push(BoxShadow {
            dx: lengths[0],
            dy: lengths[1],
            blur,
            spread: lengths.get(3).copied().unwrap_or(0.0),
            color: color.unwrap_or_else(|| current_color.unwrap_or(Color(0, 0, 0, 255))),
            inset,
        });
    }
    Some(layers)
}

/// `text-shadow` layers (blitz#271 family): `<color>? && <length>{2,3}` per
/// comma-separated layer — no 4th length, no `inset` (either is an invalid
/// declaration, not a parse-through), negative blur invalid, missing color
/// folds to currentColor like box-shadow.
pub fn parse_text_shadow(
    value: &str,
    current_color: Option<Color>,
    fonts: &FontCtx,
) -> Option<Vec<TextShadow>> {
    let mut layers = Vec::new();
    for part in split_top_level_commas(value) {
        let mut color: Option<Color> = None;
        let mut lengths: Vec<f32> = Vec::new();
        for tok in split_top_level_units(&part) {
            if tok.eq_ignore_ascii_case("inset") {
                return None;
            } else if let Some(c) = parse_color(tok) {
                if color.is_some() {
                    return None;
                }
                color = Some(c);
            } else {
                if lengths.len() >= 3 {
                    return None;
                }
                lengths.push(resolve_shadow_len(tok, fonts)?);
            }
        }
        if lengths.len() < 2 {
            return None;
        }
        let blur = lengths.get(2).copied().unwrap_or(0.0);
        if blur < 0.0 {
            return None;
        }
        layers.push(TextShadow {
            dx: lengths[0],
            dy: lengths[1],
            blur,
            color: color.unwrap_or_else(|| current_color.unwrap_or(Color(0, 0, 0, 255))),
        });
    }
    Some(layers)
}

/// `backdrop-filter` v1: exactly one `blur(<length>)` function or `none`
/// (handled at the declaration arm). A filter-function list or any other
/// filter function is an invalid declaration, not a parse-through — the
/// prior computed value survives, matching the other shadow parsers.
/// `mix-blend-mode` (#222): one of the 16 spec keywords, else a parse
/// error (the prior computed value stands). Case-sensitive like the
/// keywords' lowercase spellings in Chrome's parser.
pub fn parse_mix_blend_mode(value: &str) -> Option<MixBlendMode> {
    Some(match value {
        "normal" => MixBlendMode::Normal,
        "multiply" => MixBlendMode::Multiply,
        "screen" => MixBlendMode::Screen,
        "overlay" => MixBlendMode::Overlay,
        "darken" => MixBlendMode::Darken,
        "lighten" => MixBlendMode::Lighten,
        "color-dodge" => MixBlendMode::ColorDodge,
        "color-burn" => MixBlendMode::ColorBurn,
        "hard-light" => MixBlendMode::HardLight,
        "soft-light" => MixBlendMode::SoftLight,
        "difference" => MixBlendMode::Difference,
        "exclusion" => MixBlendMode::Exclusion,
        "hue" => MixBlendMode::Hue,
        "saturation" => MixBlendMode::Saturation,
        "color" => MixBlendMode::Color,
        "luminosity" => MixBlendMode::Luminosity,
        _ => return None,
    })
}

pub fn parse_backdrop_blur(value: &str, fonts: &FontCtx) -> Option<f32> {
    let v = value.trim();
    let open = v.find('(')?;
    if !v[..open].eq_ignore_ascii_case("blur") || !v.ends_with(')') {
        return None;
    }
    let inner = &v[open + 1..v.len() - 1];
    if inner.trim() != inner || inner.trim().is_empty() {
        return None;
    }
    let px = resolve_shadow_len(inner, fonts)?;
    if px < 0.0 {
        return None;
    }
    Some(px)
}

/// Shadow lengths resolve em/rem against the cascade fonts and fold calc();
/// % is rejected (it would need the shadow receiver's box).
fn resolve_shadow_len(val: &str, fonts: &FontCtx) -> Option<f32> {
    if let Some(Length::Px(px)) = eval_math_call(val, fonts) {
        return Some(px);
    }
    match parse_css_length(val.trim()).map(|l| resolve_len(l, fonts)) {
        Some(Length::Px(px)) => Some(px),
        _ => None,
    }
}

/// Whitespace split at parenthesis depth 0 — the functional token
/// (`rgb(0 0 0 / 50%)`) stays whole, its inner spaces unsplit.
fn split_top_level_units(value: &str) -> Vec<&str> {
    let mut units = Vec::new();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    for (i, b) in value.bytes().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            _ if b.is_ascii_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    units.push(&value[s..i]);
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(i);
                }
            }
        }
    }
    if let Some(s) = start {
        units.push(&value[s..]);
    }
    units
}
