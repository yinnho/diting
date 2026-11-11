//! Per-element cascade memo (#66 follow-up). x.com's compose page re-ran
//! the whole cascade 79 times in 45s (81ms each, ~2300 elements) while each
//! DOM epoch touched a handful of nodes. The memo hands an element its
//! previous `cascade_element` output when every input that function reads
//! is unchanged:
//!
//! - per run (any change clears every entry): the rule table itself, each
//!   rule's layer rank (per-thread parse state), the `@property` registry,
//!   the viewport;
//! - per element: tag, the full attribute list (inline style, lang, dir,
//!   href, the td/th presentational hints all live there), the matched
//!   rule indices + specificity after @container gating, the root font
//!   size, and the parent's face *version*.
//!
//! A face is the style an element hands its children (cascade output after
//! the animation sample). Its version stays put while the face compares
//! equal to the previous run's and takes a fresh, never-reused number
//! otherwise — so a child hits only when its parent handed down exactly
//! the value it saw last time, whatever happened in between (removal,
//! reinsertion, slot reuse). `dir=auto` reads the subtree's text and
//! slot-assigned children inherit from a post-processed style; both always
//! recompute. Pseudo-elements, counters, animation sampling, zoom scaling
//! and transitions run every pass on top of the (memoized) cascade output.

use diting_css::{ComputedStyle, ParsedRule};
use diting_dom::tree::{Attribute, DomTree, NodeId};

/// Parent version of a root element (no parent face). Real versions start
/// at 1.
pub(crate) const ROOT_PARENT: u64 = 0;
/// Recorded for elements whose parent face is not memo-tracked: no live
/// version ever equals it.
const UNTRACKED_PARENT: u64 = u64::MAX;

#[derive(Default)]
struct StyleMemo {
    /// Slot 0: the plain document pass; slot 1: the @container gated pass
    /// (a different rule table every epoch would thrash a shared slot).
    faces: [Option<FaceMemo>; 2],
    next_version: u64,
    /// (hits, misses) of the last finished run — test probe.
    last_stats: (usize, usize),
}

struct FaceMemo {
    rules: Vec<ParsedRule>,
    layer_ranks: Vec<u32>,
    registry: diting_css::props::RegistrySnapshot,
    viewport: (u32, u32),
    entries: Vec<Option<Entry>>,
}

struct Entry {
    tag: String,
    attrs: Vec<Attribute>,
    matched: Vec<(usize, u32)>,
    parent_version: u64,
    root_fs: u32,
    cacheable: bool,
    raw: ComputedStyle,
    /// The face when the animation sample changed it; `None` = same as raw.
    face: Option<ComputedStyle>,
    version: u64,
}

impl Entry {
    fn face(&self) -> &ComputedStyle {
        self.face.as_ref().unwrap_or(&self.raw)
    }
}

/// One cascade run's view of the memo; [`MemoRun::finish`] stores it back.
pub(crate) struct MemoRun {
    slot: usize,
    face: FaceMemo,
    next_version: u64,
    pub(crate) hits: usize,
    pub(crate) misses: usize,
}

/// Element-level inputs gathered by the visit before it cascades.
pub(crate) struct ElemKey<'a> {
    pub(crate) nid: NodeId,
    pub(crate) tag: &'a str,
    pub(crate) matched: &'a [(usize, u32)],
    /// `None` when the parent face is not memo-tracked (slot inheritance).
    pub(crate) parent_version: Option<u64>,
    pub(crate) root_fs: f32,
}

impl MemoRun {
    pub(crate) fn begin(
        tree: &DomTree,
        slot: usize,
        rules: &[ParsedRule],
        viewport: (f32, f32),
    ) -> MemoRun {
        let mut memo = tree
            .take_style_memo()
            .and_then(|b| b.downcast::<StyleMemo>().ok())
            .map(|b| *b)
            .unwrap_or_default();
        let layer_ranks = diting_css::rule_layer_ranks(rules);
        let registry = diting_css::props::registry_snapshot();
        let viewport = (viewport.0.to_bits(), viewport.1.to_bits());
        let face = match memo.faces[slot].take() {
            Some(f)
                if f.viewport == viewport
                    && f.layer_ranks == layer_ranks
                    && f.registry == registry
                    && f.rules == rules =>
            {
                f
            }
            _ => FaceMemo {
                rules: rules.to_vec(),
                layer_ranks,
                registry,
                viewport,
                entries: Vec::new(),
            },
        };
        let mut run = MemoRun {
            slot,
            face,
            next_version: memo.next_version.max(1),
            hits: 0,
            misses: 0,
        };
        run.face
            .entries
            .resize_with(tree.node_slot_count(), || None);
        tree.set_style_memo(Box::new(memo));
        run
    }

    pub(crate) fn finish(self, tree: &DomTree) {
        let mut memo = tree
            .take_style_memo()
            .and_then(|b| b.downcast::<StyleMemo>().ok())
            .map(|b| *b)
            .unwrap_or_default();
        memo.faces[self.slot] = Some(self.face);
        memo.next_version = self.next_version;
        memo.last_stats = (self.hits, self.misses);
        tree.set_style_memo(Box::new(memo));
    }

    /// The previous cascade output when every input matches.
    pub(crate) fn lookup(&self, tree: &DomTree, k: &ElemKey) -> Option<ComputedStyle> {
        let pv = k.parent_version?;
        let e = self.face.entries.get(k.nid.index())?.as_ref()?;
        let same = e.cacheable
            && e.parent_version == pv
            && e.root_fs == k.root_fs.to_bits()
            && e.tag == k.tag
            && e.matched == k.matched
            && tree
                .with_node(k.nid, |n| n.attrs() == Some(&e.attrs[..]))
                .unwrap_or(false);
        same.then(|| e.raw.clone())
    }

    /// Record this run's result and return the element's face version for
    /// its children. `raw` is `Some` on a miss (the fresh cascade output),
    /// `None` on a hit (the entry already holds it).
    pub(crate) fn record(
        &mut self,
        tree: &DomTree,
        k: &ElemKey,
        raw: Option<ComputedStyle>,
        face: &ComputedStyle,
    ) -> u64 {
        let idx = k.nid.index();
        if idx >= self.face.entries.len() {
            self.face.entries.resize_with(idx + 1, || None);
        }
        let slot = &mut self.face.entries[idx];
        let old_version = slot.as_ref().map(|e| e.version);
        let stable;
        match raw {
            None => {
                self.hits += 1;
                let e = slot.as_mut().expect("memo hit without an entry");
                stable = if face.animation.is_none() && e.face.is_none() {
                    // No sampler on either side: face == raw == last face.
                    true
                } else {
                    face == e.face()
                };
                e.face = if face.animation.is_some() && *face != e.raw {
                    Some(face.clone())
                } else {
                    None
                };
            }
            Some(raw) => {
                self.misses += 1;
                stable = slot.as_ref().is_some_and(|e| e.face() == face);
                let (attrs, cacheable) = tree
                    .with_node(k.nid, |n| {
                        let dir_auto = n
                            .get_attribute("dir")
                            .is_some_and(|d| d.trim().eq_ignore_ascii_case("auto"));
                        (n.attrs().map(<[_]>::to_vec).unwrap_or_default(), !dir_auto)
                    })
                    .unwrap_or_default();
                let face_copy = (face.animation.is_some() && *face != raw).then(|| face.clone());
                *slot = Some(Entry {
                    tag: k.tag.to_string(),
                    attrs,
                    matched: k.matched.to_vec(),
                    parent_version: k.parent_version.unwrap_or(UNTRACKED_PARENT),
                    root_fs: k.root_fs.to_bits(),
                    cacheable,
                    raw,
                    face: face_copy,
                    version: 0,
                });
            }
        }
        let version = match (stable, old_version) {
            (true, Some(v)) => v,
            _ => {
                let v = self.next_version;
                self.next_version += 1;
                v
            }
        };
        if let Some(e) = self.face.entries[idx].as_mut() {
            e.version = version;
        }
        version
    }
}

/// Drop the tree's cascade memo (the next run recomputes every element).
pub fn clear_style_memo(tree: &DomTree) {
    drop(tree.take_style_memo());
}

/// (hits, misses) of the tree's last memoized cascade run.
#[cfg(test)]
pub(crate) fn last_stats(tree: &DomTree) -> (usize, usize) {
    let memo = tree.take_style_memo();
    let stats = memo
        .as_ref()
        .and_then(|b| b.downcast_ref::<StyleMemo>())
        .map_or((0, 0), |m| m.last_stats);
    if let Some(b) = memo {
        tree.set_style_memo(b);
    }
    stats
}
