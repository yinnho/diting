//! `@media` query evaluation.
//!
//! `media_query_applies*` stay re-exported from the crate root. Preference
//! defaults live here, next to the query that reads them.

use crate::len_parse::CssF32;
use crate::supports_cond::split_top_level_keyword;

/// Which media a stylesheet evaluation targets. `print` rules only apply
/// inside `@media print` bodies; `screen` rules apply outside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CssMediaType {
    #[default]
    Screen,
    Print,
}

/// Emulated media features pushed via CDP `Emulation.setEmulatedMedia`
/// (Playwright's `page.emulateMedia`): (name, value) pairs such as
/// `("prefers-reduced-motion", "reduce")`. A feature present here answers
/// every query for that name from its emulated value; the rest keep the
/// persona defaults `media_pref_default` holds — the same table the JS
/// `matchMedia` publishes, so the cascade and the scripts agree (#29).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MediaOverrides {
    pub features: Vec<(String, String)>,
}

/// The un-emulated value a preference feature holds — mirrors the
/// `_MQ_PERSONA_BOOL` desktop-light persona in the bootstrap. Features not
/// listed have no default and stay false in both faces until emulated.
fn media_pref_default(name: &str) -> Option<&'static str> {
    match name {
        "prefers-color-scheme" => Some("light"),
        "prefers-reduced-motion" => Some("no-preference"),
        "prefers-reduced-transparency" => Some("no-preference"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// @media evaluation
// ---------------------------------------------------------------------------

/// A media-query list is an OR of comma-separated arms (commas inside
/// functions are not separators). Evaluate each arm independently.
pub fn media_query_applies(query: &str, viewport: (f32, f32), media_type: CssMediaType) -> bool {
    media_query_applies_with(query, viewport, media_type, &MediaOverrides::default())
}

/// Same, with emulated media features (`Emulation.setEmulatedMedia`) consulted
/// before the persona defaults — the cascade face of `matchMedia`'s truth.
pub fn media_query_applies_with(
    query: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
) -> bool {
    split_media_query_list(query)
        .into_iter()
        .any(|q| single_media_query_applies(q, viewport, media_type, overrides))
}

/// Split on top-level commas only: `rgb(1,2,3)` keeps its commas inside one arm.
fn split_media_query_list(query: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in query.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            ',' if depth == 0 => {
                parts.push(&query[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&query[start..]);
    parts
}

fn single_media_query_applies(
    query: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
) -> bool {
    let query = query.trim().strip_prefix("@media").unwrap_or(query).trim();
    let compact: String = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();

    // Leading `not` negates the whole arm. Only strip the `not` token itself;
    // the media type after it must stay intact (`not print` → evaluate
    // `print`, not an empty string that would wrongly imply `all`).
    if let Some(rest) = compact.strip_prefix("not") {
        return !single_media_query_applies_compact(rest, viewport, media_type, overrides);
    }
    single_media_query_applies_compact(&compact, viewport, media_type, overrides)
}

fn single_media_query_applies_compact(
    compact: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
) -> bool {
    // A bare feature list (`(min-width: 768px)`) implies media type `all`;
    // evaluate its features directly instead of walking the type-token path.
    if compact.starts_with('(') || compact.is_empty() {
        return and_chain(compact)
            .iter()
            .all(|feature| media_feature_applies(feature, viewport, overrides));
    }

    // Media type check first; features then refine. An unknown type fails the arm.
    let (type_matches, rest) = if let Some(r) = compact.strip_prefix("print") {
        (media_type == CssMediaType::Print, Some(r))
    } else if let Some(r) = compact.strip_prefix("screen") {
        (media_type == CssMediaType::Screen, Some(r))
    } else if let Some(r) = compact.strip_prefix("all") {
        // `all` applies under every media type — including emulated print.
        (true, Some(r))
    } else if let Some(r) = compact.strip_prefix("onlyscreen") {
        // `only screen` legacy prefix: skip the `only` token.
        (media_type == CssMediaType::Screen, Some(r))
    } else {
        (false, None)
    };
    if !type_matches {
        return false;
    }
    let rest = rest.unwrap_or("");
    if rest.is_empty() {
        return true;
    }
    // Features must be `and`-chained after the type token; anything else is
    // unparseable — be conservative and fail.
    let Some(feature_expr) = rest.strip_prefix("and") else {
        return false;
    };
    and_chain(feature_expr)
        .iter()
        .all(|feature| media_feature_applies(feature, viewport, overrides))
}

/// An `and`-chain of features, cut on the keyword outside the brackets.
/// The query is compacted first — no whitespace left — so a cut on the
/// bare substring would also cut `landscape` in half (`l-and-scape`).
/// `@supports` conditions are cut by the same rule.
fn and_chain(compact: &str) -> Vec<String> {
    split_top_level_keyword(compact, "and").unwrap_or_else(|| vec![compact.to_string()])
}

/// One `(min-width: 768px)`-style expression. Width/height bounds compare
/// against the viewport; `prefers-*` preferences answer from the emulated
/// overrides first, then the persona defaults — evaluated before the px parse
/// so a non-numeric value like `reduce` isn't discarded as unparseable.
/// `orientation` reads the viewport's shape: a square hangs portrait, the
/// height not below the width — the same reading the script face
/// (`matchMedia`) gives, so the two cannot disagree.
fn media_feature_applies(feature: &str, viewport: (f32, f32), overrides: &MediaOverrides) -> bool {
    let feature = feature
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let Some((name, value)) = feature.split_once(':') else {
        // Boolean feature without a value: unsupported → conservative false.
        return false;
    };
    let name = name.trim();
    let value = value.trim();
    if let Some(answer) = media_pref_answer(name, value, overrides) {
        return answer;
    }
    if name.eq_ignore_ascii_case("orientation") {
        let hung = if viewport.1 >= viewport.0 {
            "portrait"
        } else {
            "landscape"
        };
        return value.eq_ignore_ascii_case(hung);
    }
    let value_px = parse_px(value);
    let value_px = match value_px {
        Some(v) => v,
        None => return false,
    };
    match name {
        "min-width" => viewport.0 >= value_px,
        "max-width" => viewport.0 <= value_px,
        "min-height" => viewport.1 >= value_px,
        "max-height" => viewport.1 <= value_px,
        "width" => (viewport.0 - value_px).abs() < f32::EPSILON,
        "height" => (viewport.1 - value_px).abs() < f32::EPSILON,
        _ => false,
    }
}

/// Resolve a `prefers-*` (or any keyword-valued preference) expression:
/// emulated value wins, persona default second. `None` = not a preference
/// feature we model — the caller falls through to the numeric path.
fn media_pref_answer(name: &str, value: &str, overrides: &MediaOverrides) -> Option<bool> {
    let emulated = overrides
        .features
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v);
    let truth = match emulated {
        Some(v) => Some(v.as_str()),
        None => media_pref_default(name),
    }?;
    Some(truth.eq_ignore_ascii_case(value))
}

pub(crate) fn parse_px(value: &str) -> Option<f32> {
    let v = value.trim().strip_suffix("px").unwrap_or(value.trim());
    v.css_f32()
}
