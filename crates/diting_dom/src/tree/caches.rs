//! Persisted per-tree caches the style pipeline parks between runs: the
//! incremental selector match state (#111) and the layout crate's
//! per-element cascade memo. Split from tree.rs (god-file ratchet,
//! ARCHITECTURE.md §6 P2).

use super::{DomTree, MatchCacheBox};

impl DomTree {
    /// Take the persisted match state when the css bytes behind `key` are
    /// unchanged; drop it otherwise (stale buckets must never be probed).
    pub(crate) fn take_match_cache(&self, key: u64) -> Option<MatchCacheBox> {
        let mut inner = self.inner.borrow_mut();
        if inner.match_cache.as_ref().is_some_and(|c| c.key == key) {
            inner.match_cache.take()
        } else {
            inner.match_cache = None;
            None
        }
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
}
