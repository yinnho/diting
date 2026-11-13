//! Shadow roots and host-including walks.
//!
//! Split from `tree.rs`. These stay methods on `DomTree`. Pure move.

use super::*;

impl DomTree {
    /// Create and attach a native shadow-root node to `host`.
    pub fn attach_shadow_root(
        &self,
        host: NodeId,
        mode: ShadowRootMode,
    ) -> Result<NodeId, AttachShadowError> {
        {
            let inner = self.inner.borrow();
            let host_is_element = inner
                .nodes
                .get(host.index())
                .and_then(|node| node.as_ref())
                .is_some_and(Node::is_element);
            if !host_is_element {
                return Err(AttachShadowError::HostIsNotElement);
            }
            if inner.shadow_roots_by_host.contains_key(&host) {
                return Err(AttachShadowError::HostAlreadyHasShadowRoot);
            }
        }

        let root = self.new_node(NodeData::Document);
        // A freshly allocated document-fragment backing node satisfies every
        // invariant below. If this ever fails, remove it so the arena does not
        // retain an unreachable allocation.
        if let Err(error) = self.attach_shadow_root_node(host, root, mode) {
            self.remove(root);
            return Err(error);
        }
        Ok(root)
    }

    /// Attach an existing detached fragment node as a shadow root. The node
    /// must be an unregistered Document-data node with no ordinary links.
    pub(crate) fn attach_shadow_root_node(
        &self,
        host: NodeId,
        root: NodeId,
        mode: ShadowRootMode,
    ) -> Result<(), AttachShadowError> {
        let mut inner = self.inner.borrow_mut();
        let host_is_element = inner
            .nodes
            .get(host.index())
            .and_then(|node| node.as_ref())
            .is_some_and(Node::is_element);
        if !host_is_element {
            return Err(AttachShadowError::HostIsNotElement);
        }
        if inner.shadow_roots_by_host.contains_key(&host) {
            return Err(AttachShadowError::HostAlreadyHasShadowRoot);
        }

        let valid_root = root != inner.document
            && !inner.shadow_roots.contains_key(&root)
            && inner
                .nodes
                .get(root.index())
                .and_then(|node| node.as_ref())
                .is_some_and(|node| {
                    matches!(node.data, NodeData::Document)
                        && node.parent.is_none()
                        && node.prev_sibling.is_none()
                        && node.next_sibling.is_none()
                });
        if !valid_root {
            return Err(AttachShadowError::InvalidShadowRoot);
        }

        let info = ShadowRoot {
            id: root,
            host,
            mode,
        };
        inner.shadow_roots.insert(root, info);
        inner.shadow_roots_by_host.insert(host, root);
        Ok(())
    }

    /// Return the native root hosted by `host`, including closed roots. Web API
    /// visibility is intentionally left to the caller.
    pub fn shadow_root(&self, host: NodeId) -> Option<NodeId> {
        self.inner.borrow().shadow_roots_by_host.get(&host).copied()
    }

    /// Every registered shadow-root node, in no particular order. The
    /// rendering passes (styles, layout, image prefetch) walk these in
    /// addition to the ordinary tree so shadow content paints; ordinary DOM
    /// traversal must stay tree-scoped.
    pub fn shadow_roots(&self) -> Vec<NodeId> {
        self.inner.borrow().shadow_roots.keys().copied().collect()
    }

    pub fn shadow_root_info(&self, root: NodeId) -> Option<ShadowRoot> {
        self.inner.borrow().shadow_roots.get(&root).copied()
    }

    pub fn is_shadow_root(&self, node: NodeId) -> bool {
        self.inner.borrow().shadow_roots.contains_key(&node)
    }

    /// Return the root of `node`'s local tree scope. This follows ordinary
    /// parent links only, so a shadow descendant resolves to its ShadowRoot and
    /// a light descendant resolves to its document or detached subtree root.
    pub fn tree_scope_root(&self, node: NodeId) -> Option<NodeId> {
        let inner = self.inner.borrow();
        let mut current = node;
        for _ in 0..=inner.nodes.len() {
            let current_node = inner.nodes.get(current.index())?.as_ref()?;
            match current_node.parent {
                Some(parent) => current = parent,
                None => return Some(current),
            }
        }
        None
    }

    pub fn containing_shadow_root(&self, node: NodeId) -> Option<NodeId> {
        let root = self.tree_scope_root(node)?;
        self.is_shadow_root(root).then_some(root)
    }

    /// Return the topmost root after crossing ShadowRoot-to-host edges. This is
    /// the native counterpart of `getRootNode({ composed: true })`.
    pub fn shadow_including_root(&self, node: NodeId) -> Option<NodeId> {
        let inner = self.inner.borrow();
        let mut current = node;
        for _ in 0..=inner.nodes.len() {
            let current_node = inner.nodes.get(current.index())?.as_ref()?;
            if let Some(parent) = current_node.parent {
                current = parent;
            } else if let Some(root) = inner.shadow_roots.get(&current) {
                current = root.host;
            } else {
                return Some(current);
            }
        }
        None
    }

    fn host_including_parent(inner: &DomTreeInner, node: NodeId) -> Option<NodeId> {
        inner
            .nodes
            .get(node.index())
            .and_then(|entry| entry.as_ref())
            .and_then(|entry| entry.parent)
            .or_else(|| inner.shadow_roots.get(&node).map(|root| root.host))
    }

    /// DOM insertion rejects a node when it is a host-including inclusive
    /// ancestor of the destination parent. Ordinary parent links are not enough
    /// for this check because a ShadowRoot's parent is intentionally null.
    pub(crate) fn would_create_host_including_cycle(
        inner: &DomTreeInner,
        parent: NodeId,
        child: NodeId,
    ) -> bool {
        let child_can_be_ancestor = inner
            .nodes
            .get(child.index())
            .and_then(|entry| entry.as_ref())
            .is_some_and(|entry| entry.first_child.is_some())
            || inner.shadow_roots_by_host.contains_key(&child);
        if !child_can_be_ancestor {
            return false;
        }

        let mut current = Some(parent);
        for _ in 0..=inner.nodes.len() {
            let node = match current {
                Some(node) => node,
                None => return false,
            };
            if node == child {
                return true;
            }
            current = Self::host_including_parent(inner, node);
        }

        // A valid host-including chain cannot be longer than the arena. Refuse
        // mutation if pre-existing corruption ever violates that invariant.
        true
    }
}
