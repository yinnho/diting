//! `@supports` condition evaluation.
//!
//! Split from the css hub. `supports_condition_applies` stays re-exported
//! from the crate root. Pure move.

use super::*;

// ---------------------------------------------------------------------------
// @supports evaluation
// ---------------------------------------------------------------------------

/// Evaluate an `@supports` condition: `prop: value` probes checked against
/// our supported-declaration set, combined with not/and/or per CSS Conditional.
pub fn supports_condition_applies(condition: &str) -> bool {
    let condition = condition.trim();
    let condition = condition
        .strip_prefix("@supports")
        .or_else(|| condition.strip_prefix("supports"))
        .unwrap_or(condition)
        .trim();
    eval_supports(condition).unwrap_or(false)
}

fn eval_supports(condition: &str) -> Option<bool> {
    let condition = condition.trim();
    if condition.is_empty() {
        return None;
    }
    // Unwrap one enclosing parenthesized group: `(...)` or `((a) and (b))`.
    if condition.starts_with('(') && condition.ends_with(')') && balanced_inner_is_whole(condition)
    {
        return eval_supports(&condition[1..condition.len() - 1]);
    }
    if let Some(rest) = strip_keyword(condition, "not") {
        return eval_supports(rest).map(|r| !r);
    }
    if let Some(parts) = split_top_level_keyword(condition, "or") {
        let results: Option<Vec<bool>> = parts.iter().map(|p| eval_supports(p)).collect();
        return results.map(|rs| rs.iter().any(|&b| b));
    }
    if let Some(parts) = split_top_level_keyword(condition, "and") {
        let results: Option<Vec<bool>> = parts.iter().map(|p| eval_supports(p)).collect();
        return results.map(|rs| rs.iter().all(|&b| b));
    }
    // Leaf: selector probes. `CSS.supports(':hover')` / `selector(:has(a))`
    // are true in Chrome for every pseudo this engine matches; an unknown
    // pseudo fails like a real browser.
    let probe = condition.trim();
    let selector_probe = probe
        .strip_prefix("selector(")
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
        .unwrap_or(probe);
    if let Some(pseudo) = selector_probe.strip_prefix(':') {
        let head = pseudo.split('(').next().unwrap_or(pseudo);
        let known = supports::pseudo_known(head.trim_start_matches(':'));
        return Some(known);
    }
    // Leaf: `(prop: value)` declaration probe.
    let probe = probe.trim_start_matches('(').trim_end_matches(')').trim();
    let (name, value) = probe.split_once(':')?;
    Some(supports_declaration(name.trim(), value.trim()))
}

/// Is the content between the outer parens one balanced group (not `(a)+(b)`)?
fn balanced_inner_is_whole(condition: &str) -> bool {
    let mut depth = 0i32;
    for (i, c) in condition.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && i != condition.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn strip_keyword<'a>(condition: &'a str, keyword: &str) -> Option<&'a str> {
    if condition
        .get(..keyword.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(keyword))
        && condition
            .as_bytes()
            .get(keyword.len())
            .is_some_and(u8::is_ascii_whitespace)
    {
        Some(condition[keyword.len()..].trim())
    } else {
        None
    }
}

/// Split on a top-level (depth-0, outside parens) boolean keyword.
fn split_top_level_keyword(condition: &str, keyword: &str) -> Option<Vec<String>> {
    let bytes = condition.as_bytes();
    let kw = keyword.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut i = 0usize;
    let mut found = false;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = (depth - 1).max(0),
            _ if depth == 0 && i + kw.len() <= bytes.len() && bytes[i..].starts_with(kw) => {
                let before_ok =
                    i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b')';
                let after_idx = i + kw.len();
                let after_ok = after_idx == bytes.len()
                    || bytes[after_idx].is_ascii_whitespace()
                    || bytes[after_idx] == b'(';
                let word_boundary_before = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                if before_ok && after_ok && word_boundary_before {
                    // Avoid matching "and" inside "not": check no preceding space-less letter.
                    parts.push(condition[start..i].trim().to_string());
                    found = true;
                    i = after_idx;
                    start = i;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if !found {
        return None;
    }
    parts.push(condition[start..].trim().to_string());
    Some(parts)
}
