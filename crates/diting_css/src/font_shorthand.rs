//! The `font` shorthand and the weight parser it shares.
//!
//! Split from the css hub. Callers still see the names on the crate root.
//! Pure move.

use super::*;

/// Quote- and paren-aware token split for the `font` shorthand: quoted
/// family names ("Helvetica Neue", Arial) and calc() bodies stay one token.
fn split_font_tokens(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut start: Option<usize> = None;
    for (idx, ch) in value.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                if start.is_none() {
                    start = Some(idx);
                }
            }
            '(' => {
                depth += 1;
                if start.is_none() {
                    start = Some(idx);
                }
            }
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 && c.is_whitespace() => {
                if let Some(s) = start.take() {
                    out.push(&value[s..idx]);
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(idx);
                }
            }
        }
    }
    if let Some(s) = start {
        out.push(&value[s..]);
    }
    out
}

pub(super) struct FontShorthand {
    pub(super) weight: u16,
    pub(super) size_px: Option<f32>,
    pub(super) line_height: Option<LineHeightSpec>,
    pub(super) family: String,
    pub(super) small_caps: bool,
}

/// Parse the `font` shorthand grammar (system font keywords are not
/// modeled): leading style/variant/weight/stretch tokens in any order, then
/// the mandatory size (which may carry a `/line-height` with or without
/// surrounding whitespace), then the mandatory family as the verbatim
/// remainder. em font-size folds against the current context size — exact
/// when the shorthand is the only font-size declaration (own == inherited),
/// an approximation when an earlier declaration already set a size.
pub(super) fn parse_font_shorthand(v: &str, fonts: &FontCtx) -> Option<FontShorthand> {
    let v = v.trim();
    let first = v.split_whitespace().next()?.to_ascii_lowercase();
    if matches!(
        first.as_str(),
        "caption"
            | "icon"
            | "menu"
            | "message-box"
            | "small-caption"
            | "status-bar"
            | "inherit"
            | "initial"
            | "unset"
            | "revert"
    ) {
        return None;
    }
    let toks = split_font_tokens(v);
    let mut weight = 400u16;
    let mut small_caps = false;
    let mut idx = 0usize;
    while idx < toks.len() {
        let t = toks[idx];
        if let Some(w) = parse_font_weight(t) {
            weight = w;
            idx += 1;
            continue;
        }
        match t.to_ascii_lowercase().as_str() {
            "small-caps" => {
                small_caps = true;
                idx += 1;
            }
            "italic" | "oblique" | "normal" | "semi-condensed" | "condensed"
            | "extra-condensed" | "ultra-condensed" | "semi-expanded" | "expanded"
            | "extra-expanded" | "ultra-expanded" | "bolder" | "lighter" => idx += 1,
            _ => break,
        }
    }
    // Mandatory size — may carry an attached `/line-height` (`20px/1.5`).
    // An unparseable size makes the whole shorthand invalid.
    let size_tok = toks.get(idx)?;
    let (size_str, mut next) = match size_tok.split_once('/') {
        Some((s, lh)) if !s.is_empty() => (s, Some(lh)),
        _ => (*size_tok, None),
    };
    let size = size_px(size_str, fonts)?;
    idx += 1;
    // Detached `/line-height`: `20px / 1.5` or `20px /1.5`.
    if next.is_none() {
        if let Some(t) = toks.get(idx) {
            if *t == "/" {
                next = Some(*toks.get(idx + 1)?);
                idx += 2;
            } else if let Some(rest) = t.strip_prefix('/') {
                next = Some(rest);
                idx += 1;
            }
        }
    }
    let line_height = next.and_then(parse_line_height).map(|raw| match raw {
        LineHeightRaw::Normal => LineHeightSpec::Normal,
        LineHeightRaw::Number(n) => LineHeightSpec::Number(n),
        LineHeightRaw::Len(CssLength::Px(px)) => LineHeightSpec::Px(px),
        // em folds against the shorthand's NEW size (spec: line-height
        // computes after font-size within the shorthand); % behaves like a
        // number multiplier, matching the line-height longhand arm.
        LineHeightRaw::Len(CssLength::Em(n)) => LineHeightSpec::Px(n * size),
        LineHeightRaw::Len(CssLength::Rem(n)) => LineHeightSpec::Px(n * fonts.root),
        LineHeightRaw::Len(CssLength::Percent(p)) => LineHeightSpec::Number(p / 100.0),
        LineHeightRaw::Len(CssLength::Vw(n)) => LineHeightSpec::Px(n * fonts.viewport_w / 100.0),
        LineHeightRaw::Len(CssLength::Vh(n)) => LineHeightSpec::Px(n * fonts.viewport_h / 100.0),
        LineHeightRaw::Len(CssLength::Vmin(n)) => {
            LineHeightSpec::Px(n * fonts.viewport_w.min(fonts.viewport_h) / 100.0)
        }
        LineHeightRaw::Len(CssLength::Vmax(n)) => {
            LineHeightSpec::Px(n * fonts.viewport_w.max(fonts.viewport_h) / 100.0)
        }
        // ch/ex fold at the 0.5em fallback, basis = the shorthand's new size.
        LineHeightRaw::Len(CssLength::Ch(n)) | LineHeightRaw::Len(CssLength::Ex(n)) => {
            LineHeightSpec::Px(n * 0.5 * size)
        }
    });
    // Mandatory family: the verbatim remainder (quoted names included).
    if idx >= toks.len() {
        return None;
    }
    let family = toks[idx..].join(" ");
    Some(FontShorthand {
        weight,
        size_px: Some(size),
        line_height,
        family,
        small_caps,
    })
}

/// Fold a font-size token from the shorthand against the context. `None`
/// when the token is not a parseable size (the whole shorthand is invalid
/// without one).
fn size_px(size_str: &str, fonts: &FontCtx) -> Option<f32> {
    parse_font_size_len(size_str).map(|l| {
        font_size_px(
            l,
            fonts.own,
            fonts.root,
            (fonts.viewport_w, fonts.viewport_h),
        )
    })
}

pub(super) fn parse_font_weight(v: &str) -> Option<u16> {
    match v.trim() {
        "normal" => Some(400),
        "bold" => Some(700),
        other => {
            let n = other.parse::<u16>().ok()?;
            (1..=9).contains(&(n / 100)).then_some(n)
        }
    }
}
