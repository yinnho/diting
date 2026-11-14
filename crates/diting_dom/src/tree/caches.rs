//! Persisted per-tree caches the style pipeline parks between runs: the
//! incremental selector match state (#111) and the layout crate's
//! per-element cascade memo. Split from tree.rs (god-file ratchet,
//! ARCHITECTURE.md §6 P2).

use super::{DomTree, MatchCacheBox, MatchDirty, NodeId};

impl DomTree {
    /// Mark an element's selector matches stale without saying what
    /// changed: the next sync rebuilds. Writers that know what they changed
    /// use the scoped hooks instead (`note_attr_change` and friends).
    pub fn note_restyle(&self, _id: NodeId) {
        self.inner.borrow_mut().match_dirty.full = true;
    }

    /// Escape hatch for mutation paths the dirty registry cannot describe:
    /// the next incremental match sync rebuilds from scratch. No production
    /// mutation path needs it yet; the dirty-registry tests drive it.
    #[cfg(test)]
    pub(crate) fn note_match_full(&self) {
        self.inner.borrow_mut().match_dirty.full = true;
    }

    pub(crate) fn take_match_dirty(&self) -> MatchDirty {
        std::mem::take(&mut self.inner.borrow_mut().match_dirty)
    }

    /// Take the persisted match state; the caller decides whether it still
    /// fits the current rule list (see `rule_match_sets_incremental`).
    pub(crate) fn take_match_cache(&self) -> Option<MatchCacheBox> {
        self.inner.borrow_mut().match_cache.take()
    }

    pub(crate) fn set_match_cache(&self, cache: MatchCacheBox) {
        self.inner.borrow_mut().match_cache = Some(cache);
    }

    /// Hand out the persisted cascade memo — opaque here, the dom crate
    /// cannot name ComputedStyle; every entry is keyed on its full inputs,
    /// so no mutation path needs to invalidate it. The caller puts it back
    /// with [`DomTree::set_style_memo`] after the run.
    pub fn take_style_memo(&self) -> Option<Box<dyn std::any::Any>> {
        self.inner.borrow_mut().style_memo.take()
    }

    pub fn set_style_memo(&self, memo: Box<dyn std::any::Any>) {
        self.inner.borrow_mut().style_memo = Some(memo);
    }

    /// An attribute write from the ops layer (`Node::set_attribute` has no
    /// tree handle), old value read before the write: only rules reading
    /// the changed class tokens / id / attribute name re-match, inside a
    /// zone anchored at the written element's parent.
    pub fn note_attr_change(&self, id: NodeId, name: &str, old: Option<&str>, new: Option<&str>) {
        let parent = self.with_node(id, |n| n.parent).flatten();
        let mut inner = self.inner.borrow_mut();
        inner.match_dirty.attr_change(name, old, new);
        // A write on a detached element flips nothing attached — its own
        // hits died with the detach — but the zone build skips detached
        // anchors anyway, so stamping is harmless.
        inner.match_dirty.attr_zone(id, parent);
    }

    /// Write an attribute and stamp exactly what changed; returns the old
    /// value (the ops layer's id index wants it).
    pub fn set_attribute_noted(&self, id: NodeId, name: &str, value: &str) -> Option<String> {
        let old = self
            .with_node(id, |n| n.get_attribute(name).map(str::to_string))
            .flatten();
        self.with_node_mut(id, |n| n.set_attribute(name, value.to_string()));
        self.note_attr_change(id, name, old.as_deref(), Some(value));
        old
    }

    /// Hover/active/focus moved (tree/state.rs): only rules using those
    /// pseudo-classes re-match, document-wide (`:focus-within`, `:hover`
    /// up the chain and every descendant-combinator subject under it).
    pub(crate) fn note_state_change(&self) {
        self.inner.borrow_mut().match_dirty.state = true;
    }

    /// A text node's data changed in place (`:empty` on its parent).
    pub fn note_text_change(&self, id: NodeId) {
        let parent = self.with_node(id, |n| n.parent).flatten();
        let mut inner = self.inner.borrow_mut();
        if let Some(p) = parent {
            inner.match_dirty.parent_zone(p);
        }
        inner.match_dirty.structure = true;
    }

    /// A form control's live value / checkedness / indeterminate state
    /// changed (`:checked`, `:indeterminate` and the other wild rules).
    pub fn note_live_change(&self) {
        self.inner.borrow_mut().match_dirty.live = true;
    }
}
