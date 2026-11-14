//! Parent, child, and ancestor walks.
//!
//! Every walk is iterative and capped at the node-slot count. A cycle in
//! the parent or sibling links stops the walk instead of hanging the engine.

use super::{DomTree, NodeId};

impl DomTree {
    pub fn children(&self, node_id: NodeId) -> Vec<NodeId> {
        let inner = self.inner.borrow();
        let mut result = Vec::new();
        let mut current = inner
            .nodes
            .get(node_id.index())
            .and_then(|n| n.as_ref())
            .and_then(|n| n.first_child);
        while let Some(child_id) = current {
            result.push(child_id);
            // Defense in depth, same bound as descendants(): a valid sibling
            // chain is at most nodes.len() long. Exceeding it means
            // next_sibling forms a cycle; stop rather than loop forever.
            if result.len() > inner.nodes.len() {
                eprintln!(
                    "diting: children() cap hit at node {} - cycle",
                    node_id.index()
                );
                break;
            }
            current = inner
                .nodes
                .get(child_id.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.next_sibling);
        }
        result
    }

    pub fn descendants(&self, node_id: NodeId) -> Vec<NodeId> {
        let inner = self.inner.borrow();
        let mut result = Vec::new();
        let mut stack = Vec::new();

        let mut first = inner
            .nodes
            .get(node_id.index())
            .and_then(|n| n.as_ref())
            .and_then(|n| n.first_child);
        let mut children_to_push = Vec::new();
        while let Some(child_id) = first {
            children_to_push.push(child_id);
            if children_to_push.len() > inner.nodes.len() {
                eprintln!(
                    "diting: sibling-chain cap hit at node {} - cycle",
                    node_id.index()
                );
                break;
            }
            first = inner
                .nodes
                .get(child_id.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.next_sibling);
        }
        for child_id in children_to_push.into_iter().rev() {
            stack.push(child_id);
        }

        while let Some(current) = stack.pop() {
            result.push(current);
            // Defense in depth: a well-formed subtree has at most nodes.len()
            // descendants. Exceeding that means the parent/child graph is cyclic
            // (which the append_child / insert_before guards prevent); stop rather
            // than grow the stack and result forever and wedge the engine. On a
            // valid tree this bound is never reached, so the hot path is unchanged.
            if result.len() > inner.nodes.len() {
                eprintln!(
                    "diting: descendants() cap hit at node {} ({} nodes) - tree has a cycle",
                    node_id.index(),
                    inner.nodes.len()
                );
                break;
            }

            let mut child = inner
                .nodes
                .get(current.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.first_child);
            let mut children_to_push = Vec::new();
            while let Some(child_id) = child {
                children_to_push.push(child_id);
                if children_to_push.len() > inner.nodes.len() {
                    eprintln!(
                        "diting: sibling-chain cap hit at node {} - cycle",
                        current.index()
                    );
                    break;
                }
                child = inner
                    .nodes
                    .get(child_id.index())
                    .and_then(|n| n.as_ref())
                    .and_then(|n| n.next_sibling);
            }
            for child_id in children_to_push.into_iter().rev() {
                stack.push(child_id);
            }
        }

        result
    }

    pub fn ancestors(&self, node_id: NodeId) -> Vec<NodeId> {
        let inner = self.inner.borrow();
        let mut result = Vec::new();
        let mut current = inner
            .nodes
            .get(node_id.index())
            .and_then(|n| n.as_ref())
            .and_then(|n| n.parent);
        while let Some(parent_id) = current {
            result.push(parent_id);
            // Defense in depth: a valid parent chain is at most nodes.len()
            // long. Exceeding it means parent forms a cycle; stop rather
            // than loop forever.
            if result.len() > inner.nodes.len() {
                eprintln!(
                    "diting: ancestors() cap hit at node {} - cycle",
                    node_id.index()
                );
                break;
            }
            current = inner
                .nodes
                .get(parent_id.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.parent);
        }
        result
    }

    /// True when walking the parent chain from `node_id` reaches the document node (light tree only).
    pub fn is_in_document(&self, node_id: NodeId) -> bool {
        let inner = self.inner.borrow();
        let mut current = node_id;
        for _ in 0..=inner.nodes.len() {
            if current == inner.document {
                return true;
            }
            match inner
                .nodes
                .get(current.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.parent)
            {
                Some(parent) => current = parent,
                None => return false,
            }
        }
        false
    }
}
