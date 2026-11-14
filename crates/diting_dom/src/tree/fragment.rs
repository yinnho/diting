//! Parsed fragments and template documents.
//!
//! `fragment_root` is the node whose children are the fragment as written.
//! `template_contents` is the separate document behind a `<template>`.

use super::{DomTree, NodeData, NodeId};

impl DomTree {
    /// The node whose children are a parsed fragment's top-level nodes: the
    /// synthetic root `<html>` element html5ever wraps a fragment in, or the
    /// document if there is none. Importing this node's children reproduces the
    /// fragment as written.
    ///
    /// This must NOT descend into a synthesized `<body>`. A `<body>` child of
    /// the root only appears when the fragment is parsed in the `<html>`
    /// context (documentElement.innerHTML), where html5ever's "before head"
    /// mode synthesizes both `<head>` and `<body>`; returning the body there
    /// dropped the head siblings. Every other element context leaves the
    /// parsed content directly under the root, so it is unaffected.
    pub fn fragment_root(&self) -> NodeId {
        let doc = self.document();
        for child in self.children(doc) {
            if let Some(n) = self.get_node(child) {
                if n.as_element()
                    .map(|name| name.local.as_str() == "html")
                    .unwrap_or(false)
                {
                    return child;
                }
            }
        }
        doc
    }

    /// Return the template contents document for a `<template>` element,
    /// allocating one on demand for created templates — `.content` must be
    /// usable whether or not the parser pre-created it.
    ///
    /// Returns `None` for a non-element node.
    pub fn template_contents(&self, node_id: NodeId) -> Option<NodeId> {
        {
            let inner = self.inner.borrow();
            let node = inner.nodes.get(node_id.index())?.as_ref()?;
            match &node.data {
                NodeData::Element {
                    template_contents, ..
                } => {
                    if let Some(existing) = *template_contents {
                        return Some(existing);
                    }
                }
                _ => return None,
            }
        }

        // Borrow released above: `new_node` takes its own mutable borrow.
        // Matches what the tree sink allocates for a parsed template.
        let contents = self.new_node(NodeData::Document);
        let mut inner = self.inner.borrow_mut();
        if let Some(Some(node)) = inner.nodes.get_mut(node_id.index()) {
            if let NodeData::Element {
                template_contents, ..
            } = &mut node.data
            {
                *template_contents = Some(contents);
                return Some(contents);
            }
        }
        None
    }
}
