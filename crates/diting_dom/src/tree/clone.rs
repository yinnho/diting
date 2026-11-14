//! Clone and import nodes.
//!
//! `clone_node` copies within one tree. `import_node_from` copies out of
//! another tree. Shadow roots are not cloned; template contents are a
//! separate document node and get their own copy.

use super::{DomTree, NodeData, NodeId};

impl DomTree {
    /// Clone one node within this tree without attaching the clone.
    ///
    /// This operates on node data directly instead of serializing and parsing
    /// HTML. Besides avoiding context-sensitive fragment parsing (`<html>`,
    /// table children, and foreign content), it preserves the cloned root's
    /// element type and namespace. Template contents are stored in a separate
    /// document node and therefore need their own remapped clone.
    pub fn clone_node(&self, source_node_id: NodeId, deep: bool) -> Option<NodeId> {
        // DOM cloneNode is not defined for ShadowRoot nodes. A host clone keeps
        // its light subtree only; the separate registry means the shadow root
        // is naturally omitted from that traversal.
        if self.is_shadow_root(source_node_id) {
            return None;
        }
        let source_data = self.get_node(source_node_id)?.data;
        let cloned_root = self.new_node(source_data);
        let mut stack = Vec::new();
        self.prepare_cloned_children(source_node_id, cloned_root, deep, &mut stack);

        while let Some((dest_parent, source_node)) = stack.pop() {
            let source_data = match self.get_node(source_node) {
                Some(node) => node.data,
                None => continue,
            };
            let cloned_node = self.new_node(source_data);
            self.append_child(dest_parent, cloned_node);
            self.prepare_cloned_children(source_node, cloned_node, true, &mut stack);
        }

        Some(cloned_root)
    }

    fn prepare_cloned_children(
        &self,
        source_node: NodeId,
        cloned_node: NodeId,
        deep: bool,
        stack: &mut Vec<(NodeId, NodeId)>,
    ) {
        let source_contents = self
            .with_node(source_node, |node| match &node.data {
                NodeData::Element {
                    template_contents, ..
                } => *template_contents,
                _ => None,
            })
            .flatten();

        if let Some(source_contents) = source_contents {
            let cloned_contents = self.new_node(NodeData::Document);
            self.with_node_mut(cloned_node, |node| {
                if let NodeData::Element {
                    template_contents, ..
                } = &mut node.data
                {
                    *template_contents = Some(cloned_contents);
                }
            });
            if deep {
                for child in self.children(source_contents).into_iter().rev() {
                    stack.push((cloned_contents, child));
                }
            }
        }

        if deep {
            for child in self.children(source_node).into_iter().rev() {
                stack.push((cloned_node, child));
            }
        }
    }

    pub fn import_children_from(&self, parent_id: NodeId, source: &DomTree, source_node: NodeId) {
        let source_children = source.children(source_node);
        for source_child_id in source_children {
            let _ = self.import_node_from(parent_id, source, source_child_id);
        }
    }

    /// Copies a node together with its subtree from `source` and attaches it to
    /// `parent_id`. Returns the copy of `source_node_id` itself, so a caller
    /// that keeps parsing into `source` (the document.write input stream) can
    /// map the source node to its copy.
    pub fn import_node_from(
        &self,
        parent_id: NodeId,
        source: &DomTree,
        source_node_id: NodeId,
    ) -> Option<NodeId> {
        // Iterative DFS with an explicit (dest_parent, source_node) stack so a
        // deeply nested source tree cannot overflow the thread stack. Children
        // are pushed in reverse so they are appended in document order.
        let mut imported_root = None;
        let mut stack = vec![(parent_id, source_node_id)];
        while let Some((dest_parent, src_id)) = stack.pop() {
            let node_data = {
                let source_inner = source.inner.borrow();
                match source_inner.nodes.get(src_id.index()) {
                    Some(Some(node)) => node.data.clone(),
                    _ => continue,
                }
            };

            let new_id = self.new_node(node_data);
            self.append_child(dest_parent, new_id);
            // The first node off the stack is source_node_id itself.
            if imported_root.is_none() {
                imported_root = Some(new_id);
            }

            // A <template>'s children hang off a separate contents document, so
            // the child walk below never reaches them. Worse, the cloned data
            // carries the *source* tree's contents NodeId, which here indexes
            // whatever unrelated node occupies that slot. Allocate a real
            // contents node and queue the source contents' children into it, so
            // the reference is remapped rather than left dangling.
            let src_contents = {
                let inner = self.inner.borrow();
                match inner.nodes.get(new_id.index()).and_then(|n| n.as_ref()) {
                    Some(node) => match &node.data {
                        NodeData::Element {
                            template_contents, ..
                        } => *template_contents,
                        _ => None,
                    },
                    None => None,
                }
            };
            if let Some(src_contents) = src_contents {
                let dest_contents = self.new_node(NodeData::Document);
                {
                    let mut inner = self.inner.borrow_mut();
                    if let Some(Some(node)) = inner.nodes.get_mut(new_id.index()) {
                        if let NodeData::Element {
                            template_contents, ..
                        } = &mut node.data
                        {
                            *template_contents = Some(dest_contents);
                        }
                    }
                }
                for child_id in source.children(src_contents).into_iter().rev() {
                    stack.push((dest_contents, child_id));
                }
            }

            for child_id in source.children(src_id).into_iter().rev() {
                stack.push((new_id, child_id));
            }
        }
        imported_root
    }
}
