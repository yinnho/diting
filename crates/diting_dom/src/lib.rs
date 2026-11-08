pub mod selector;
pub mod serialize;
pub mod tree;
pub mod tree_sink;

pub use tree::{AttachShadowError, DomTree, NodeData, NodeId, ShadowRootMode};
pub use tree_sink::{parse_fragment, parse_fragment_with_context, parse_html};

#[cfg(test)]
mod tests;

/// Split on commas outside any parenthesis or quoted string — `rgb(1, 2, 3)`
/// stays one piece, `counters(x, ", ")` keeps its separator whole. Also the
/// selector-side splitter for the match index's pseudo-element routing.
pub fn split_top_level_commas(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut quote: Option<u8> = None;
    let bytes = value.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' => quote = Some(b),
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                parts.push(value[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(value[start..].trim().to_string());
    parts.retain(|p| !p.is_empty());
    parts
}
