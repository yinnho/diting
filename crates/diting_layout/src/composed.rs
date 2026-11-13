//! Composed-tree walks: shadow hosts, slots, and shadow `<style>` text.
//!
//! Split from the layout hub. `shadow_style_texts` stays re-exported from
//! the crate root. Pure move.

use super::*;

/// The composed-tree children that render under `id` (shadow DOM phase 2):
/// a shadow host renders its shadow tree's children instead of its light
/// children, and a `<slot>` renders its assigned light children — or, with
/// nothing assigned, its own fallback children. The slot element itself
/// never builds a box (display:contents equivalent). Everything else passes
/// through unchanged; light-tree walks elsewhere must stay tree-scoped.
pub(super) fn render_children(tree: &DomTree, id: NodeId) -> Vec<NodeId> {
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
