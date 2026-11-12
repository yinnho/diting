//! Live interaction state: focus, hover, active, text selection, and
//! element scroll offsets — the per-document fields that die with the tree
//! on navigation (Chrome resets activeElement and hover the same way).
//! Split from tree.rs (god-file ratchet, ARCHITECTURE.md §6 P2); behavior
//! unchanged.

use std::collections::HashMap;

use super::{DomTree, DomTreeInner, NodeId};

/// Whether a hover/focus/active anchor sits at or below `root` — a subtree
/// entering or leaving the document then carries the anchor with it, and the
/// `:hover`/`:focus`/`:active`/`:focus-within` compounds on ancestors
/// OUTSIDE the subtree flip with no attribute or structure change of their
/// own. Page steady state has no anchors, so the common case is three None
/// checks. Walk bounded like the other chain walks: a corrupted parent link
/// degrades to a long chain, not a hang.
pub(super) fn state_anchor_within(inner: &DomTreeInner, root: NodeId) -> bool {
    for anchor in [inner.hovered_node, inner.focused_node, inner.active_node]
        .into_iter()
        .flatten()
    {
        let mut cur = Some(anchor);
        for _ in 0..=inner.nodes.len() {
            let Some(c) = cur else { break };
            if c == root {
                return true;
            }
            cur = inner
                .nodes
                .get(c.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.parent);
        }
    }
    false
}

impl DomTree {
    /// The currently focused element, if any (blitz#839: :focus and friends
    /// must match live focus, not a static snapshot). Tree-level on purpose:
    /// focus dies with the document — navigation builds a new tree, and
    /// Chrome resets activeElement to body on navigation the same way.
    pub fn focused_node(&self) -> Option<NodeId> {
        self.inner.borrow().focused_node
    }

    pub fn set_focused_node(&self, id: Option<NodeId>) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.focused_node == id {
                return;
            }
            inner.focused_node = id;
            // A pseudo-state flip changes computed styles (:focus, :focus-within)
            // without touching tree shape, and the style-carrying caches
            // (layout_cache behind computed_style, solved boxes) only re-key
            // on the epoch — so the flip must move it. Same for the hover and
            // active setters below (#152); an attribute write still doesn't
            // bump, those go through note_restyle's own consumers.
            inner.epoch_gen = inner.epoch_gen.wrapping_add(1);
        }
        // :focus/:focus-visible/:focus-within rules re-match document-wide
        // on the next sync (the old always-full match picked this up
        // implicitly).
        self.note_state_change();
    }

    /// The element under the pointer, if any (#152). An element matches
    /// `:hover` when it IS this node or contains it, so a hover move can
    /// flip `:hover` compounds on the whole ancestor chain of either end
    /// — and with descendant combinators, on subjects arbitrarily deep
    /// under any chain element. The state stamp re-matches every rule
    /// that uses a state pseudo-class document-wide. Tree-level like focus:
    /// hover dies with the document on navigation, same as Chrome.
    pub fn hovered_node(&self) -> Option<NodeId> {
        self.inner.borrow().hovered_node
    }

    pub fn set_hovered_node(&self, id: Option<NodeId>) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.hovered_node == id {
                return;
            }
            inner.hovered_node = id;
            inner.epoch_gen = inner.epoch_gen.wrapping_add(1);
        }
        self.note_state_change();
    }

    /// The element mid-press (#152): `:active` matches it and its ancestor
    /// chain from the mousedown until the mouseup — Chrome clears the state
    /// before dispatching pointerup (a gCS read inside a pointerup handler
    /// already shows the un-active style). Same chain-stamped setter shape
    /// as hover.
    pub fn active_node(&self) -> Option<NodeId> {
        self.inner.borrow().active_node
    }

    pub fn set_active_node(&self, id: Option<NodeId>) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.active_node == id {
                return;
            }
            inner.active_node = id;
            inner.epoch_gen = inner.epoch_gen.wrapping_add(1);
        }
        self.note_state_change();
    }

    /// The recorded text-entry selection as (node, start, end), if a
    /// script wrote one (setSelectionRange, selectionStart/End setters,
    /// value writes, focus). Offsets are the JS numbers passed through
    /// as-is — UTF-16 units; paint clamps to the value's char count, so
    /// astral characters diverge (accepted). Tree-level like focus: the
    /// record dies with the document on navigation. The caret paints only
    /// while this node ALSO holds focus — blur keeps the record, matching
    /// Chrome's hidden caret on an unfocused control.
    pub fn selection(&self) -> Option<(NodeId, usize, usize)> {
        self.inner.borrow().selection
    }

    pub fn set_selection(&self, sel: Option<(NodeId, usize, usize)>) {
        self.inner.borrow_mut().selection = sel;
    }

    /// Record one element-scroller's (scrollLeft, scrollTop). Write-through
    /// from the bootstrap's scrollTop/scrollLeft setters (sticky v2); the
    /// JS wrapper keeps its own copy for reads, so this is the paint-side
    /// truth only. Does NOT bump the tree epoch — scroll is read-time paint
    /// state; `scroll_gen` is the fingerprint shift-dependent caches key on.
    pub fn set_node_scroll(&self, id: NodeId, x: f32, y: f32) {
        let mut inner = self.inner.borrow_mut();
        let v = [x.max(0.0), y.max(0.0)];
        if inner.scroll_offsets.get(&id) != Some(&v) {
            inner.scroll_offsets.insert(id, v);
            inner.scroll_gen += 1;
        }
    }

    pub fn node_scroll(&self, id: NodeId) -> [f32; 2] {
        self.inner
            .borrow()
            .scroll_offsets
            .get(&id)
            .copied()
            .unwrap_or([0.0, 0.0])
    }

    /// The live element-scroller offsets, for the read-time shift walks.
    pub fn scroll_offsets(&self) -> HashMap<NodeId, [f32; 2]> {
        self.inner.borrow().scroll_offsets.clone()
    }

    pub fn scroll_gen(&self) -> u64 {
        self.inner.borrow().scroll_gen
    }
}
