//! Link and unlink nodes.
//!
//! `append_child` and `insert_before` attach a node. `detach` and
//! `remove_child` take it out of the parent without freeing the slot.
//! `remove` also frees the slot and every shadow tree the subtree owns.
//! A self-cycle is a no-op so a later walk cannot hang.

use std::collections::HashSet;

use super::{DomTree, NodeId};

impl DomTree {
    pub fn append_child(&self, parent_id: NodeId, child_id: NodeId) {
        // Per DOM spec, appending a node to itself is a HierarchyRequestError;
        // here we treat it as a no-op rather than panic. Without this the
        // sibling-pointer fixup below sets the node's prev_sibling to itself
        // and every later child-walk loops forever (same failure mode that
        // insert_before's self-cycle guard was added to prevent).
        if parent_id == child_id {
            return;
        }
        // Appending a node to itself is a HierarchyRequestError treated as a
        // no-op (self-cycle guard). Appending an inclusive ancestor of the
        // parent — including across root→host edges, which is why this is not
        // the plain parent walk it replaced — makes the graph cyclic, and every
        // later descendants()/children()/textContent walk would loop forever,
        // pinning the thread in native Rust where neither tokio nor the V8
        // watchdog can interrupt it. A ShadowRoot is also never itself an
        // ordinary child. Leaf appends (the common case) skip the ancestor
        // walk inside the helper, keeping the hot path O(1).
        {
            let inner = self.inner.borrow();
            let parent_exists = inner
                .nodes
                .get(parent_id.index())
                .is_some_and(Option::is_some);
            let child_exists = inner
                .nodes
                .get(child_id.index())
                .is_some_and(Option::is_some);
            if !parent_exists
                || !child_exists
                || inner.shadow_roots.contains_key(&child_id)
                || Self::would_create_host_including_cycle(&inner, parent_id, child_id)
            {
                return;
            }
        }
        self.detach(child_id);

        let mut inner = self.inner.borrow_mut();

        let old_last = inner
            .nodes
            .get(parent_id.index())
            .and_then(|n| n.as_ref())
            .and_then(|n| n.last_child);

        if let Some(Some(child)) = inner.nodes.get_mut(child_id.index()) {
            child.parent = Some(parent_id);
            child.prev_sibling = old_last;
            child.next_sibling = None;
        }

        if let Some(old_last_id) = old_last {
            if let Some(Some(old_last_node)) = inner.nodes.get_mut(old_last_id.index()) {
                old_last_node.next_sibling = Some(child_id);
            }
        }

        if let Some(Some(parent)) = inner.nodes.get_mut(parent_id.index()) {
            if parent.first_child.is_none() {
                parent.first_child = Some(child_id);
            }
            parent.last_child = Some(child_id);
        }

        // Incremental match stamp (#111): the moved subtree re-probes and
        // structural rules re-match. The detach above already stamped the
        // removal side. A parked state anchor returning with the subtree
        // flips `:hover`/`:focus` compounds on its new ancestors — state
        // rules re-match (the detach stamp may be one mutation ago).
        if super::state::state_anchor_within(&inner, child_id) {
            inner.match_dirty.state = true;
        }
        inner.match_dirty.parent_zone(parent_id);
        inner.match_dirty.inserted(child_id);
        inner.mutate_gen = inner.mutate_gen.wrapping_add(1);
    }

    pub fn insert_before(&self, existing_id: NodeId, new_sibling_id: NodeId) {
        // Per DOM spec: if the node being inserted IS the reference node,
        // the operation is a no-op (the node is already in its target
        // position). Without this, the linked-list fixup below sets the
        // node's prev_sibling and next_sibling to itself, creating a cycle
        // -- every later traversal (childNodes, querySelectorAll, etc) then
        // loops forever and the test page hangs while the engine burns RAM.
        if existing_id == new_sibling_id {
            return;
        }
        let parent_id = {
            let inner = self.inner.borrow();
            match inner
                .nodes
                .get(existing_id.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.parent)
            {
                Some(p) => p,
                None => return,
            }
        };

        // Inserting the parent itself, or any ancestor of the parent, as a child
        // of that parent would create a cycle (same non-terminating-walk hang as
        // append_child). Reject it, matching the self-insert guard above —
        // across root→host edges too, because a ShadowRoot's null parent must
        // not hide the host below it. Leaf inserts skip the walk inside the
        // helper, so the common case stays O(1). A registered shadow root can
        // never become an ordinary child either.
        {
            let inner = self.inner.borrow();
            let new_exists = inner
                .nodes
                .get(new_sibling_id.index())
                .is_some_and(Option::is_some);
            if !new_exists
                || inner.shadow_roots.contains_key(&new_sibling_id)
                || Self::would_create_host_including_cycle(&inner, parent_id, new_sibling_id)
            {
                return;
            }
        }

        self.detach(new_sibling_id);

        // Read existing's prev AFTER detaching new. If new was existing's
        // immediate previous sibling, detach moved that pointer; using the
        // pre-detach value would splice new.next_sibling = new (a self-cycle)
        // and hang every later sibling walk. This is what hung ebay.com.
        let prev_id = {
            let inner = self.inner.borrow();
            inner
                .nodes
                .get(existing_id.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.prev_sibling)
        };

        let mut inner = self.inner.borrow_mut();

        if let Some(Some(node)) = inner.nodes.get_mut(new_sibling_id.index()) {
            node.parent = Some(parent_id);
            node.prev_sibling = prev_id;
            node.next_sibling = Some(existing_id);
        }

        if let Some(Some(node)) = inner.nodes.get_mut(existing_id.index()) {
            node.prev_sibling = Some(new_sibling_id);
        }

        if let Some(prev) = prev_id {
            if let Some(Some(node)) = inner.nodes.get_mut(prev.index()) {
                node.next_sibling = Some(new_sibling_id);
            }
        } else if let Some(Some(parent)) = inner.nodes.get_mut(parent_id.index()) {
            parent.first_child = Some(new_sibling_id);
        }

        // Incremental match stamp, mirroring append_child's (state anchor
        // re-entering the document included).
        if super::state::state_anchor_within(&inner, new_sibling_id) {
            inner.match_dirty.state = true;
        }
        inner.match_dirty.parent_zone(parent_id);
        inner.match_dirty.inserted(new_sibling_id);
        inner.mutate_gen = inner.mutate_gen.wrapping_add(1);
    }

    pub fn detach(&self, node_id: NodeId) {
        let mut inner = self.inner.borrow_mut();

        // The document and registered shadow roots have no ordinary parent and
        // cannot be detached through light-tree mutation APIs.
        if node_id == inner.document || inner.shadow_roots.contains_key(&node_id) {
            return;
        }

        let (parent_id, prev_id, next_id) =
            match inner.nodes.get(node_id.index()).and_then(|n| n.as_ref()) {
                Some(node) => (node.parent, node.prev_sibling, node.next_sibling),
                None => return,
            };

        if let Some(prev) = prev_id {
            if let Some(Some(node)) = inner.nodes.get_mut(prev.index()) {
                node.next_sibling = next_id;
            }
        } else if let Some(parent_id) = parent_id {
            if let Some(Some(parent)) = inner.nodes.get_mut(parent_id.index()) {
                parent.first_child = next_id;
            }
        }

        if let Some(next) = next_id {
            if let Some(Some(node)) = inner.nodes.get_mut(next.index()) {
                node.prev_sibling = prev_id;
            }
        } else if let Some(parent_id) = parent_id {
            if let Some(Some(parent)) = inner.nodes.get_mut(parent_id.index()) {
                parent.last_child = prev_id;
            }
        }

        if let Some(Some(node)) = inner.nodes.get_mut(node_id.index()) {
            node.parent = None;
            node.prev_sibling = None;
            node.next_sibling = None;
        }

        // Incremental match stamp: the detached subtree's hits must purge
        // (it may be re-inserted later, which carries its own insert
        // stamp), and structural rules re-match (sibling combinators close
        // the gap). A state anchor (hover/focus/active) inside the subtree
        // leaves the document with it: `:hover` compounds on ancestors
        // OUTSIDE the subtree flip with no structure change of their own,
        // so the state rules re-match too.
        if let Some(parent_id) = parent_id {
            if super::state::state_anchor_within(&inner, node_id) {
                inner.match_dirty.state = true;
            }
            inner.match_dirty.parent_zone(parent_id);
            inner.match_dirty.detached(node_id);
        }
        inner.mutate_gen = inner.mutate_gen.wrapping_add(1);
    }

    /// Detach a node from its parent AND remove it (and all descendants)
    /// from the id-index so that `getElementById` no longer returns them.
    /// Unlike `remove()`, this does NOT free the nodes — the JS side may
    /// still hold references to the wrappers.
    pub fn remove_child(&self, node_id: NodeId) {
        // Collect all id attribute values in the subtree. We snapshot them
        // before detaching so `get_attribute` can still see the tree.
        let ids_to_remove: Vec<String> = {
            let descendants = self.descendants(node_id);
            let inner = self.inner.borrow();
            let mut ids: Vec<String> = Vec::new();
            if let Some(Some(node)) = inner.nodes.get(node_id.index()) {
                if let Some(id_val) = node.get_attribute("id") {
                    ids.push(id_val.to_string());
                }
            }
            for desc_id in &descendants {
                if let Some(Some(node)) = inner.nodes.get(desc_id.index()) {
                    if let Some(id_val) = node.get_attribute("id") {
                        ids.push(id_val.to_string());
                    }
                }
            }
            ids
        };

        self.detach(node_id);

        let mut inner = self.inner.borrow_mut();
        for id_str in &ids_to_remove {
            inner.id_index.remove(id_str);
        }
    }

    /// Collect an ordinary subtree plus every shadow tree owned by a host in
    /// that subtree. This is used only by the arena-freeing path; normal DOM
    /// traversal must remain tree-scoped and therefore never follows host edges.
    fn inclusive_owned_subtrees(&self, node_id: NodeId) -> Vec<NodeId> {
        let inner = self.inner.borrow();
        if !inner
            .nodes
            .get(node_id.index())
            .is_some_and(Option::is_some)
        {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![node_id];
        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            let Some(node) = inner
                .nodes
                .get(current.index())
                .and_then(|entry| entry.as_ref())
            else {
                continue;
            };
            result.push(current);

            if let Some(root) = inner.shadow_roots_by_host.get(&current) {
                stack.push(*root);
            }

            let mut children = Vec::new();
            let mut child = node.first_child;
            while let Some(child_id) = child {
                children.push(child_id);
                if children.len() > inner.nodes.len() {
                    break;
                }
                child = inner
                    .nodes
                    .get(child_id.index())
                    .and_then(|entry| entry.as_ref())
                    .and_then(|entry| entry.next_sibling);
            }
            stack.extend(children.into_iter().rev());
        }
        result
    }

    pub fn remove(&self, node_id: NodeId) {
        let nodes_to_remove = self.inclusive_owned_subtrees(node_id);
        if nodes_to_remove.is_empty() {
            return;
        }
        self.detach(node_id);
        let mut inner = self.inner.borrow_mut();
        // Every removal frees at least the root's slot (the guard above
        // returned for dead nodes), so the epoch always moves (#124).
        inner.epoch_gen = inner.epoch_gen.wrapping_add(1);

        let mut ids_to_remove = Vec::new();
        for &id in &nodes_to_remove {
            if let Some(Some(node)) = inner.nodes.get(id.index()) {
                if let Some(id_val) = node.get_attribute("id") {
                    ids_to_remove.push(id_val.to_string());
                }
            }
        }

        for id_str in ids_to_remove {
            inner.id_index.remove(&id_str);
        }

        // Remove both shadow-registry directions before freeing any arena
        // slot. Otherwise a reused NodeId could inherit an old host/root
        // relationship.
        for &id in &nodes_to_remove {
            if let Some(root) = inner.shadow_roots.remove(&id) {
                if inner.shadow_roots_by_host.get(&root.host) == Some(&id) {
                    inner.shadow_roots_by_host.remove(&root.host);
                }
            }
            if let Some(root_id) = inner.shadow_roots_by_host.remove(&id) {
                inner.shadow_roots.remove(&root_id);
            }
        }

        // Only free slots that are currently live. Freeing an out-of-range id
        // panics on direct indexing, and freeing an already-freed slot pushes it
        // onto the free list a second time — later handing the same NodeId to
        // two live nodes (aliasing). Double-remove and remove-after-remove are
        // reachable from the JS DOM API, so treat them as no-ops here.
        for id in nodes_to_remove {
            if matches!(inner.nodes.get(id.index()), Some(Some(_))) {
                inner.nodes[id.index()] = None;
                inner.free_list.push(id.0);
                // Slot recycle safety: the freed NodeId can come back as a
                // different node — its scroll offset must not follow it.
                inner.scroll_offsets.remove(&id);
            }
        }
    }
}
