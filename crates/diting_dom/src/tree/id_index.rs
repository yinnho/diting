//! Element id index.
//!
//! `update_id_index` records an id when an attribute changes.
//! `get_element_by_id` trusts that entry only when the node is still in
//! the light tree; otherwise it rescans document order and refreshes it.

use super::{DomTree, NodeId};

impl DomTree {
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        let indexed = self.inner.borrow().id_index.get(id).copied();
        // The index is keep-first and set_inner_html only detaches the old children, so it can point at a node that is no longer in
        // the document (and then never learns about the new element with the same id). getElementById only finds nodes IN the
        // tree, so an indexed hit must be connected to the document; otherwise rescan and refresh the index below.
        if indexed.is_some_and(|node| {
            self.containing_shadow_root(node).is_none() && self.is_in_document(node)
        }) {
            return indexed;
        }

        // Creation happens before insertion, so the O(1) best-effort index can
        // point at a shadow descendant (ids register at new_node, before the
        // node lands in any tree). Never expose that node through
        // document.getElementById; recover the first matching light-tree
        // element in document order instead. Detached and template-content
        // nodes retain the legacy best-effort lookup behavior.
        let found = self
            .descendants(self.document())
            .into_iter()
            .find(|node_id| {
                self.with_node(*node_id, |node| node.get_attribute("id") == Some(id))
                    .unwrap_or(false)
            });
        if let Some(node) = found {
            self.inner
                .borrow_mut()
                .id_index
                .insert(id.to_string(), node);
        }
        found
    }

    pub fn update_id_index(&self, node_id: NodeId, old_id: Option<&str>, new_id: Option<&str>) {
        let mut inner = self.inner.borrow_mut();
        if let Some(old) = old_id {
            inner.id_index.remove(old);
        }
        if let Some(new) = new_id {
            inner.id_index.insert(new.to_string(), node_id);
        }
    }
}
