//! The rule-hash match index: per-rule match sets for `compute_styles`,
//! built by bucketing rules on their rightmost compound key and probing
//! elements against the plausible buckets (batch 191), plus the
//! dirty-stamp incremental sync that keeps those sets coherent across DOM
//! mutations (batches 191/199, #111/#123). Split out of `selector.rs` in
//! batch 202 to ride under the layering audit's god-file cap — pure index
//! and sync logic; the parse/adapter face stays in the parent module.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use diting_selectors::context::QuirksMode;
use diting_selectors::matching::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags,
};
use diting_selectors::parser::{self, SelectorList};

use super::rule_deps::RuleDeps;
use super::{parse_selector, DitingSelector, DomElement};
use crate::split_top_level_commas;
use crate::tree::{DomTree, NodeId};

impl DomTree {
    /// Build [`RuleMatchSets`] for one stylesheet's rule selectors: the
    /// per-rule document match sets `compute_styles` needs, without one
    /// full-document querySelectorAll per rule.
    ///
    /// A rule selector can only match an element that carries the
    /// rightmost compound's id/class/tag, so rules are bucketed by that
    /// key and each element's keys probe only the plausible buckets; the
    /// servo matcher then confirms candidates exactly as the
    /// querySelector path would. A selector that fails to parse yields no
    /// bucket and no hits, the "never matches" outcome the old per-rule
    /// qSA error path produced.
    pub fn rule_match_sets(&self, rule_selectors: &[&str]) -> RuleMatchSets {
        // Rule matching is document-rooted (the same scope posture as a
        // document-rooted querySelectorAll): no :scope binding here. Shadow
        // descendants join the probe set so shadow `<style>` rules can match
        // shadow elements — the cascade's global-rule-pool approximation
        // (spec-scoped styles are a v3 concern). Combinators still stop at
        // the shadow root: the matcher climbs ordinary parent links, and a
        // shadow root has none.
        let mut probe_ids: Vec<NodeId> = self.descendants(self.document());
        for root in self.shadow_roots() {
            probe_ids.extend(self.descendants(root));
        }
        self.rule_match_sets_probing(rule_selectors, probe_ids)
    }

    /// Subtree-scoped variant (fabricated iframe documents): probe only the
    /// descendants of `roots`. The document-rooted probe never reaches an
    /// orphan tree, so reusing it there would silently match no rule.
    pub fn rule_match_sets_within(
        &self,
        rule_selectors: &[&str],
        roots: &[NodeId],
    ) -> RuleMatchSets {
        let mut probe_ids: Vec<NodeId> = Vec::new();
        for root in roots {
            probe_ids.extend(self.descendants(*root));
        }
        self.rule_match_sets_probing(rule_selectors, probe_ids)
    }

    fn rule_match_sets_probing(
        &self,
        rule_selectors: &[&str],
        probe_ids: Vec<NodeId>,
    ) -> RuleMatchSets {
        let index = self.compile_rule_index(rule_selectors);
        let mut hits = HashMap::new();
        let mut pseudo_hits = HashMap::new();
        self.probe_elements_into(&index, probe_ids, &mut hits, &mut pseudo_hits);
        RuleMatchSets {
            hits,
            specificity: index.specificity,
            pseudo_hits,
        }
    }

    /// Selector-side compilation shared by the full and incremental faces:
    /// parsed selector lists, specificities, pseudo routing, and the
    /// rightmost-compound buckets. Cheap to rebuild (one parse per rule) but
    /// far from free on real sheets (~7ms for 1.7MB), so the incremental
    /// path parks it in [`MatchCacheBox`] with the selector texts — and a
    /// changed rule list re-parses only the selectors it has not seen.
    fn compile_rule_index(&self, rule_selectors: &[&str]) -> RuleIndex {
        self.assemble_rule_index(rule_selectors.iter().map(|s| compile_rule(s)).collect())
    }

    /// Build the buckets and `:has` subject sets over per-rule compiled
    /// selectors. Quirks enters only here (both spellings bucketed), so a
    /// [`CompiledRule`] is reusable across a quirks flip.
    fn assemble_rule_index(&self, rules: Vec<CompiledRule>) -> RuleIndex {
        let mut by_id: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_class: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_tag: HashMap<String, Vec<usize>> = HashMap::new();
        // Rules where no sub-selector has an id/class/tag key (universal,
        // attr-only, pseudo-only compounds) stay candidates for every
        // element.
        let mut unkeyed: Vec<usize> = Vec::new();
        // In quirks mode class/id matching is ASCII-case-insensitive, so a
        // probe must reach rules keyed in the other case: bucket and probe
        // both spellings there. Over-bucketing is always safe (the matcher
        // has the final word); a bucket the probe cannot reach never is.
        let quirks = self.selector_quirks_mode() == QuirksMode::Quirks;
        let mut dep_class: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_id: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_attr: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_class_desc: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_id_desc: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_attr_desc: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_class_sib: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_id_sib: HashMap<String, Vec<usize>> = HashMap::new();
        let mut dep_attr_sib: HashMap<String, Vec<usize>> = HashMap::new();
        let (mut struct_rules, mut state_rules, mut wild_rules, mut zone_unsafe_rules) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (ri, rule) in rules.iter().enumerate() {
            let mut bucketed = false;
            for key in &rule.keys {
                match key {
                    Some(RuleKey::Id(id)) => {
                        by_id.entry(id.clone()).or_default().push(ri);
                        if quirks {
                            by_id.entry(id.to_ascii_lowercase()).or_default().push(ri);
                        }
                        bucketed = true;
                    }
                    Some(RuleKey::Class(class)) => {
                        by_class.entry(class.clone()).or_default().push(ri);
                        if quirks {
                            by_class
                                .entry(class.to_ascii_lowercase())
                                .or_default()
                                .push(ri);
                        }
                        bucketed = true;
                    }
                    Some(RuleKey::Tag(tag)) => {
                        by_tag.entry(tag.clone()).or_default().push(ri);
                        bucketed = true;
                    }
                    None => {}
                }
            }
            if !bucketed {
                unkeyed.push(ri);
            }
            let d = &rule.deps;
            for (table, keys) in [
                (&mut dep_class, &d.classes),
                (&mut dep_id, &d.ids),
                (&mut dep_attr, &d.attrs),
                (&mut dep_class_desc, &d.classes_desc),
                (&mut dep_id_desc, &d.ids_desc),
                (&mut dep_attr_desc, &d.attrs_desc),
                (&mut dep_class_sib, &d.classes_sib),
                (&mut dep_id_sib, &d.ids_sib),
                (&mut dep_attr_sib, &d.attrs_sib),
            ] {
                for k in keys {
                    table.entry(k.clone()).or_default().push(ri);
                }
            }
            if d.structural {
                struct_rules.push(ri);
            }
            if d.state {
                state_rules.push(ri);
            }
            if d.wild {
                wild_rules.push(ri);
            }
            if d.zone_unsafe {
                zone_unsafe_rules.push(ri);
            }
        }
        RuleIndex {
            specificity: rules.iter().map(|r| r.specificity).collect(),
            rules,
            by_id,
            by_class,
            by_tag,
            unkeyed,
            quirks,
            dep_class,
            dep_id,
            dep_attr,
            dep_class_desc,
            dep_id_desc,
            dep_attr_desc,
            dep_class_sib,
            dep_id_sib,
            dep_attr_sib,
            struct_rules,
            state_rules,
            wild_rules,
            zone_unsafe_rules,
        }
    }

    /// Probe elements against the compiled index, appending matches to the
    /// (possibly pre-populated, possibly purge-filtered) hit maps. Shared by
    /// the full build and the incremental sync so the two cannot drift.
    fn probe_elements_into(
        &self,
        index: &RuleIndex,
        probe_ids: impl IntoIterator<Item = NodeId>,
        hits: &mut HashMap<usize, Vec<usize>>,
        pseudo_hits: &mut HashMap<usize, [Vec<usize>; 2]>,
    ) {
        let mut caches = diting_selectors::context::SelectorCaches::default();
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut caches,
            self.selector_quirks_mode(),
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        let mut candidates: Vec<usize> = Vec::new();
        for desc_id in probe_ids {
            let Some((local, id, class)) = self
                .with_node(desc_id, |n| {
                    let name = n.as_element()?;
                    Some((
                        name.local.as_str().to_string(),
                        n.get_attribute("id").map(|s| s.to_string()),
                        n.get_attribute("class").map(|s| s.to_string()),
                    ))
                })
                .flatten()
            else {
                continue;
            };
            candidates.clear();
            candidates.extend_from_slice(&index.unkeyed);
            let mut probe = |bucket: &HashMap<String, Vec<usize>>, key: &str| {
                if let Some(rules) = bucket.get(key) {
                    candidates.extend_from_slice(rules);
                }
            };
            // Tag selectors bucket under the parser's lower_name; probing
            // the element's lowercased local name is the shared spelling.
            // Foreign-element tags keep camelCase local names in the DOM
            // (SVG clipPath and friends), and lowercasing the probe covers
            // them too.
            probe(&index.by_tag, &local.to_ascii_lowercase());
            if let Some(id) = &id {
                probe(&index.by_id, id);
                if index.quirks {
                    probe(&index.by_id, &id.to_ascii_lowercase());
                }
            }
            if let Some(class) = &class {
                // ASCII whitespace only, same as has_class (takumi #2400).
                for c in class.split_ascii_whitespace() {
                    probe(&index.by_class, c);
                    if index.quirks {
                        probe(&index.by_class, &c.to_ascii_lowercase());
                    }
                }
            }
            candidates.sort_unstable();
            candidates.dedup();
            let element = DomElement::new(self, desc_id);
            for ri in candidates.drain(..) {
                if let Some(list) = index.rules[ri].plain.as_ref() {
                    if diting_selectors::matching::matches_selector_list(
                        list,
                        element,
                        &mut context,
                    ) {
                        hits.entry(ri).or_default().push(desc_id.index());
                    }
                }
                if let Some(routing) = index.rules[ri].routing.as_ref() {
                    let slots = [routing.before.as_ref(), routing.after.as_ref()];
                    for (slot, list) in slots.into_iter().enumerate() {
                        if let Some(list) = list {
                            if diting_selectors::matching::matches_selector_list(
                                list,
                                element,
                                &mut context,
                            ) {
                                pseudo_hits.entry(ri).or_default()[slot].push(desc_id.index());
                            }
                        }
                    }
                }
            }
        }
        // The walk is document order, not node-index order; the cascade
        // binary-searches these, so each rule's hit set ends ascending.
        for rule_hits in hits.values_mut().chain(
            pseudo_hits
                .values_mut()
                .flat_map(|rule_slots| rule_slots.iter_mut()),
        ) {
            rule_hits.sort_unstable();
        }
    }

    /// The full probe set: document descendants plus every shadow tree's,
    /// the same population `rule_match_sets` covers.
    fn match_probe_all_ids(&self) -> Vec<NodeId> {
        let mut probe_ids: Vec<NodeId> = self.descendants(self.document());
        for root in self.shadow_roots() {
            probe_ids.extend(self.descendants(root));
        }
        probe_ids
    }

    /// Whether `nid` is reachable from the document or a shadow root — the
    /// population a fresh full match would probe. Detached subtrees (removed
    /// but not freed, or awaiting re-insertion) must not contribute hits a
    /// full rebuild would not produce.
    fn match_probe_attached(&self, nid: NodeId) -> bool {
        let inner = self.borrow_inner();
        let mut current = Some(nid);
        for _ in 0..=inner.nodes.len() {
            let Some(c) = current else { return false };
            if c == inner.document || inner.shadow_roots.contains_key(&c) {
                return true;
            }
            current = inner
                .nodes
                .get(c.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.parent);
        }
        false
    }

    /// Incremental face for repeated layout runs against the same stylesheet
    /// (#111): the hit sets persist in the tree's [`MatchCacheBox`] with the
    /// selector texts they were matched for, and DOM mutations recorded in [`MatchDirty`] since the
    /// last sync only re-probe the elements they could have affected. The
    /// result is bit-for-bit what a fresh full match produces — the
    /// correctness argument is stamp completeness: an insert/re-parent only
    /// changes matches inside the moved subtree (ancestor chains) plus the
    /// two child lists it touches (sibling combinators, :nth-child) plus the
    /// parent chain (:has anchoring); an attribute write only changes the
    /// element, its subtree, its siblings, and its ancestor chain. Anything
    /// the registry cannot express falls back to a full rebuild. A changed
    /// rule list (insertRule) carries every rule whose selector text
    /// survived and matches only the new ones (`remap_match_cache`);
    /// `css_key` only labels the trace lines.
    pub fn rule_match_sets_incremental(
        &self,
        rule_selectors: &[&str],
        css_key: u64,
    ) -> Arc<RuleMatchSets> {
        let trace = std::env::var("DITING_MATCH_TRACE").is_ok();
        let dirty = self.take_match_dirty();
        let cache = match self.take_match_cache() {
            // Same rule list (whatever the key says): reuse as-is.
            Some(cache)
                if cache
                    .selectors
                    .iter()
                    .map(String::as_str)
                    .eq(rule_selectors.iter().copied()) =>
            {
                cache
            }
            // The css changed (insertRule, a new <style>): carry over every
            // rule whose selector text is unchanged — a hit set is a pure
            // function of selector text and tree — and probe only the
            // rest. The dirty sync below then brings the carried sets up
            // to the current tree like any other run. A quirks flip needs no
            // guard here: the remap compiles under the current mode and
            // set_quirks stamps a full re-probe.
            Some(cache) => self.remap_match_cache(cache, rule_selectors, css_key, trace),
            None => {
                // First run: full build against fresh
                // buckets, then park the result for the next sync.
                let index = self.compile_rule_index(rule_selectors);
                let mut hits = HashMap::new();
                let mut pseudo_hits = HashMap::new();
                self.probe_elements_into(
                    &index,
                    self.match_probe_all_ids(),
                    &mut hits,
                    &mut pseudo_hits,
                );
                let sets = Arc::new(RuleMatchSets {
                    hits,
                    specificity: index.specificity.clone(),
                    pseudo_hits,
                });
                self.set_match_cache(MatchCacheBox {
                    selectors: rule_selectors.iter().map(|s| s.to_string()).collect(),
                    index,
                    sets: sets.clone(),
                });
                if trace {
                    eprintln!(
                        "[match-trace] FULL-BUILD key={css_key:#x} rules={} elements={}",
                        rule_selectors.len(),
                        sets.hits.values().map(Vec::len).sum::<usize>(),
                    );
                }
                return sets;
            }
        };

        let rebuild = |tree: &Self, cache: MatchCacheBox| -> Arc<RuleMatchSets> {
            #[cfg(test)]
            LAST_SYNC_AFFECTED.with(|c| c.set(None));
            #[cfg(test)]
            LAST_SYNC_SCOPED.with(|c| c.set(None));
            let mut hits = HashMap::new();
            let mut pseudo_hits = HashMap::new();
            tree.probe_elements_into(
                &cache.index,
                tree.match_probe_all_ids(),
                &mut hits,
                &mut pseudo_hits,
            );
            let sets = Arc::new(RuleMatchSets {
                hits,
                specificity: cache.index.specificity.clone(),
                pseudo_hits,
            });
            tree.set_match_cache(MatchCacheBox {
                sets: sets.clone(),
                ..cache
            });
            sets
        };

        if dirty.full {
            if trace {
                eprintln!("[match-trace] REBUILD(dirty.full) key={css_key:#x}");
            }
            return rebuild(self, cache);
        }

        let slots = self.node_slot_count();
        let mut seen = vec![false; slots];
        let mut mark = |nid: NodeId| {
            let i = nid.index();
            if i < slots && !seen[i] {
                seen[i] = true;
                true
            } else {
                false
            }
        };
        // Fast gate: the stamp count is a lower bound on the candidate
        // count (every root contributes at least itself), so a mass
        // mutation (initial parse, big innerHTML) skips candidate
        // resolution entirely and rebuilds — the pre-incremental cost.
        let stamp_count = dirty.roots.len() + dirty.purge_roots.len();
        if stamp_count.saturating_mul(4) > slots {
            if trace {
                eprintln!(
                    "[match-trace] REBUILD(stamp-gate) key={css_key:#x} stamps={stamp_count} slots={slots}"
                );
            }
            return rebuild(self, cache);
        }

        // Pass 1 — element-scoped: inserted, moved and removed subtrees.
        // Their members re-probe against every rule (new nodes were never
        // matched; moved ones have new ancestors). Slots is a safe index
        // ceiling: the arena never shrinks, and freed slots that come back
        // carry their own insert stamps.
        let mut candidates: Vec<NodeId> = Vec::new();
        for root in dirty
            .roots
            .iter()
            .copied()
            .chain(dirty.purge_roots.iter().copied())
        {
            if mark(root) {
                candidates.push(root);
            }
            for d in self.descendants(root) {
                if mark(d) {
                    candidates.push(d);
                }
            }
        }
        // A mass mutation (initial parse, big innerHTML) costs more to
        // resolve than a full rebuild — same as the pre-incremental world.
        if candidates.len() * 4 > slots {
            if trace {
                eprintln!(
                    "[match-trace] REBUILD(candidate-gate) key={css_key:#x} candidates={} slots={slots}",
                    candidates.len()
                );
            }
            return rebuild(self, cache);
        }

        // Pass 2 — rule-scoped: everything else a mutation can flip lives
        // OUTSIDE those subtrees only through a rule that depends on what
        // changed (see RuleDeps): the changed class tokens / ids /
        // attribute names, hover-focus state, sibling structure, or a
        // pseudo-class we do not scope. Those rules (usually a handful)
        // re-match inside the mutations' zone; every other rule's hits
        // outside the pass-1 subtrees cannot have moved.
        let mut affected: Vec<usize> = Vec::new();
        let idx = &cache.index;
        if dirty.structure {
            affected.extend_from_slice(&idx.struct_rules);
            // Slot/shadow/`&`/invalid rules re-match on structure too —
            // their flip surface is not describable with dep keys, and an
            // inserted subtree can rewire slot assignment around itself.
            affected.extend_from_slice(&idx.zone_unsafe_rules);
        }
        if dirty.state {
            affected.extend_from_slice(&idx.state_rules);
        }
        // A descendant- or sibling-side dep firing decides the re-match
        // zone (subtree / parent); a subject-side firing alone only ever
        // flips the written element itself.
        let mut fired_desc = vec![false; idx.rules.len()];
        let mut fired_sib = vec![false; idx.rules.len()];
        for (subj, desc, sib, keys) in [
            (
                &idx.dep_class,
                &idx.dep_class_desc,
                &idx.dep_class_sib,
                &dirty.classes,
            ),
            (&idx.dep_id, &idx.dep_id_desc, &idx.dep_id_sib, &dirty.ids),
            (
                &idx.dep_attr,
                &idx.dep_attr_desc,
                &idx.dep_attr_sib,
                &dirty.attrs,
            ),
        ] {
            for k in keys {
                if let Some(rules) = desc.get(k) {
                    for &r in rules {
                        fired_desc[r] = true;
                    }
                    affected.extend_from_slice(rules);
                }
                if let Some(rules) = sib.get(k) {
                    for &r in rules {
                        fired_sib[r] = true;
                    }
                    affected.extend_from_slice(rules);
                }
                if let Some(rules) = subj.get(k) {
                    affected.extend_from_slice(rules);
                }
            }
        }
        // Live control state can sit on any element; an attr write can
        // flip a wild pseudo on the written element or below it (a `slot=`
        // reassignment, inherited `:dir`). A pure structure/state window
        // cannot — the inserted subtree re-probes in pass 1, and
        // hover/focus on untouched elements did not move.
        if dirty.live
            || !dirty.classes.is_empty()
            || !dirty.ids.is_empty()
            || !dirty.attrs.is_empty()
        {
            affected.extend_from_slice(&idx.wild_rules);
        }
        affected.sort_unstable();
        affected.dedup();
        // Re-matching most of the sheet document-wide is a rebuild anyway.
        if affected.len() * 2 > idx.rules.len() {
            if trace {
                eprintln!(
                    "[match-trace] REBUILD(rule-gate) key={css_key:#x} affected={} rules={}",
                    affected.len(),
                    idx.rules.len()
                );
            }
            return rebuild(self, cache);
        }

        // The hit sets hand out and park behind a shared Arc: the style run
        // consumes its handle and drops it, so the unwrap below moves in
        // place — the clone-out this replaced was a per-sync cost the size
        // of the whole hit map, dwarfing the incremental work itself. A
        // caller that holds its handle across syncs pays one copy here,
        // exactly the old behavior.
        let mut sets = Arc::unwrap_or_clone(cache.sets);
        let cache_index = &cache.index;

        // Purge every candidate index from the hit sets, then re-probe the
        // attached ones. Detached subtree members stay purged (a full
        // rebuild probes only document+shadow descendants), and a freed
        // slot's stale hits die here too.
        let candidates_len = candidates.len();
        for rule_hits in sets
            .hits
            .values_mut()
            .chain(sets.pseudo_hits.values_mut().flat_map(|s| s.iter_mut()))
        {
            rule_hits.retain(|&i| !(i < slots && seen[i]));
        }
        let attached: Vec<NodeId> = candidates
            .into_iter()
            .filter(|&c| self.match_probe_attached(c))
            .collect();
        if trace {
            eprintln!(
                "[match-trace] INCREMENTAL key={css_key:#x} roots={} purge={} candidates={} attached={} affected-rules={}",
                dirty.roots.len(),
                dirty.purge_roots.len(),
                candidates_len,
                attached.len(),
                affected.len()
            );
        }
        self.probe_elements_into(cache_index, attached, &mut sets.hits, &mut sets.pseudo_hits);
        if !affected.is_empty() {
            // The re-match stays inside the mutations' zone instead of the
            // whole document, with the zone shape chosen per rule by which
            // of its deps actually fired (see RuleDeps' side split):
            // subject-side only — the written elements; descendant-side or
            // a wild pseudo — the written elements' subtrees; structural or
            // sibling-side — the change parents' subtrees. Rules whose
            // matching crosses boundaries the zone walks cannot see (:has
            // matches elements other than the subject; slots, shadow, `&`),
            // and state/live windows, re-match document-wide, as does any
            // zone that grew to the document.
            fn reprobe_rules(
                tree: &DomTree,
                src_rules: &[CompiledRule],
                sets: &mut RuleMatchSets,
                rules: &[usize],
                document_wide: bool,
                zone_seen: &[bool],
                zone_ids: &[NodeId],
            ) {
                if rules.is_empty() {
                    return;
                }
                if document_wide {
                    for r in rules {
                        sets.hits.remove(r);
                        sets.pseudo_hits.remove(r);
                    }
                } else {
                    // Outside the zone the affected rules' hits cannot have
                    // moved (that is the zone invariant) — keep them;
                    // inside, purge and re-probe.
                    let in_zone =
                        |v: &mut Vec<usize>| v.retain(|&i| !(i < zone_seen.len() && zone_seen[i]));
                    for r in rules {
                        if let Some(v) = sets.hits.get_mut(r) {
                            in_zone(v);
                        }
                        if let Some(s) = sets.pseudo_hits.get_mut(r) {
                            for v in s.iter_mut() {
                                in_zone(v);
                            }
                        }
                    }
                }
                let sub =
                    tree.assemble_rule_index(rules.iter().map(|&r| src_rules[r].clone()).collect());
                let mut sub_hits = HashMap::new();
                let mut sub_pseudo = HashMap::new();
                tree.probe_elements_into(
                    &sub,
                    if document_wide {
                        tree.match_probe_all_ids()
                    } else {
                        zone_ids.to_vec()
                    },
                    &mut sub_hits,
                    &mut sub_pseudo,
                );
                for (k, v) in sub_hits {
                    sets.hits.entry(rules[k]).or_default().extend(v);
                }
                for (k, v) in sub_pseudo {
                    let [a, b] = v;
                    let t = sets.pseudo_hits.entry(rules[k]).or_default();
                    t[0].extend(a);
                    t[1].extend(b);
                }
                if !document_wide {
                    // A cold build's hit vecs come in document order (the
                    // probe walks the tree); the zone merge interleaves
                    // survivors with zone-order hits. Same set, different
                    // order — sort back.
                    for r in rules {
                        if let Some(v) = sets.hits.get_mut(r) {
                            v.sort_unstable();
                        }
                        if let Some(s) = sets.pseudo_hits.get_mut(r) {
                            for v in s.iter_mut() {
                                v.sort_unstable();
                            }
                        }
                    }
                }
            }
            /// How far a zone reaches below its anchors — the per-rule
            /// choice is the whole point of the side split.
            #[derive(Clone, Copy, PartialEq)]
            enum ZoneReach {
                /// The anchors only: a subject-side dep flip lands on the
                /// written element itself.
                Anchors,
                /// Anchor plus its children: sibling-position rules
                /// (`:first-child`, `.a + .b` subjects) live here.
                Children,
                /// Anchor plus its subtree: descendant-side deps and
                /// sibling rules that continue downward.
                Subtree,
            }
            fn build_zone(
                tree: &DomTree,
                anchors: &[NodeId],
                slots: usize,
                reach: ZoneReach,
            ) -> Option<(Vec<bool>, Vec<NodeId>)> {
                // The demotion gate is ids.len()*4 > slots: walking a huge
                // zone just to demote it afterwards wastes the walk, so the
                // budget comes in here — bail the moment it is crossed.
                let budget = slots / 4;
                let mut seen = vec![false; slots];
                let mut ids = Vec::new();
                let mark = |c: NodeId, seen: &mut Vec<bool>, ids: &mut Vec<NodeId>| {
                    if ids.len() > budget {
                        return;
                    }
                    if c.index() < seen.len() && !seen[c.index()] {
                        seen[c.index()] = true;
                        ids.push(c);
                    }
                };
                for &a in anchors {
                    // Attachment is inherited downward: a detached anchor's
                    // subtree is detached with it, so skip it wholesale —
                    // probing it would add hits a cold build
                    // (attached-only) never produces, while its stale hits
                    // already died in pass 1's purge.
                    if !tree.match_probe_attached(a) {
                        continue;
                    }
                    mark(a, &mut seen, &mut ids);
                    if ids.len() > budget {
                        return None;
                    }
                    match reach {
                        ZoneReach::Anchors => {}
                        ZoneReach::Children => {
                            let mut child = tree.with_node(a, |n| n.first_child).flatten();
                            while let Some(c) = child {
                                mark(c, &mut seen, &mut ids);
                                if ids.len() > budget {
                                    return None;
                                }
                                child = tree.with_node(c, |n| n.next_sibling).flatten();
                            }
                        }
                        ZoneReach::Subtree => {
                            for d in tree.descendants(a) {
                                mark(d, &mut seen, &mut ids);
                                if ids.len() > budget {
                                    return None;
                                }
                            }
                        }
                    }
                }
                Some((seen, ids))
            }
            let mut wide: Vec<usize> = Vec::new();
            let mut elem: Vec<usize> = Vec::new();
            let mut subtree: Vec<usize> = Vec::new();
            let mut parent: Vec<usize> = Vec::new();
            let mut parent_sub: Vec<usize> = Vec::new();
            for &r in &affected {
                let deps = &cache_index.rules[r].deps;
                if deps.zone_unsafe {
                    wide.push(r);
                } else if dirty.structure && deps.structural || fired_sib[r] {
                    // A structural flip lands on the change parent's
                    // children (sibling subjects, `:empty` on the parent
                    // itself); rules that keep walking down from those
                    // children (`.a + .b .c`) need the whole subtree.
                    let desc_side = !(deps.classes_desc.is_empty()
                        && deps.ids_desc.is_empty()
                        && deps.attrs_desc.is_empty());
                    if desc_side {
                        parent_sub.push(r);
                    } else {
                        parent.push(r);
                    }
                } else if fired_desc[r] || (deps.wild && deps.wild_inherited) {
                    subtree.push(r);
                } else {
                    // Subject-side fire only — including structural rules
                    // whose sibling conditions did not move (a class write
                    // on `:first-child`'s subject) and subject pseudos
                    // (`:visited`, `:checked`): the flip is at the written
                    // element itself.
                    elem.push(r);
                }
            }
            // A zone is only built for the shapes that have rules waiting;
            // state/live windows and an over-grown zone demote theirs to a
            // document-wide re-match instead.
            let forced_wide = dirty.state || dirty.live || dirty.zone_full;
            let demote = |want_wide: bool,
                          anchors: &[NodeId],
                          rules: &[usize],
                          reach: ZoneReach|
             -> (bool, Vec<bool>, Vec<NodeId>) {
                if want_wide || rules.is_empty() || anchors.is_empty() {
                    return (true, Vec::new(), Vec::new());
                }
                match build_zone(self, anchors, slots, reach) {
                    Some((seen, ids)) => (false, seen, ids),
                    None => (true, Vec::new(), Vec::new()),
                }
            };
            let (elem_wide, elem_seen, elem_ids) =
                demote(forced_wide, &dirty.elem_zones, &elem, ZoneReach::Anchors);
            let (subtree_wide, subtree_seen, subtree_ids) =
                demote(forced_wide, &dirty.elem_zones, &subtree, ZoneReach::Subtree);
            let (parent_wide, parent_seen, parent_ids) = demote(
                forced_wide,
                &dirty.parent_zones,
                &parent,
                ZoneReach::Children,
            );
            let (parent_sub_wide, parent_sub_seen, parent_sub_ids) = demote(
                forced_wide,
                &dirty.parent_zones,
                &parent_sub,
                ZoneReach::Subtree,
            );
            if elem_wide {
                wide.append(&mut elem);
            }
            if subtree_wide {
                wide.append(&mut subtree);
            }
            if parent_wide {
                wide.append(&mut parent);
            }
            if parent_sub_wide {
                wide.append(&mut parent_sub);
            }
            #[cfg(test)]
            let scoped_rules = elem.len() + subtree.len() + parent.len() + parent_sub.len();
            if std::env::var("DITING_ZONE_DEBUG").is_ok() {
                eprintln!(
                    "[zone-debug] affected={} wide={} elem={} ({} ids) subtree={} ({} ids) parent={} ({} ids) parent_sub={} ({} ids) elem_anchors={} parent_anchors={}",
                    affected.len(),
                    wide.len(),
                    elem.len(),
                    elem_ids.len(),
                    subtree.len(),
                    subtree_ids.len(),
                    parent.len(),
                    parent_ids.len(),
                    parent_sub.len(),
                    parent_sub_ids.len(),
                    dirty.elem_zones.len(),
                    dirty.parent_zones.len(),
                );
            }
            reprobe_rules(self, &cache_index.rules, &mut sets, &wide, true, &[], &[]);
            reprobe_rules(
                self,
                &cache_index.rules,
                &mut sets,
                &elem,
                false,
                &elem_seen,
                &elem_ids,
            );
            reprobe_rules(
                self,
                &cache_index.rules,
                &mut sets,
                &subtree,
                false,
                &subtree_seen,
                &subtree_ids,
            );
            reprobe_rules(
                self,
                &cache_index.rules,
                &mut sets,
                &parent,
                false,
                &parent_seen,
                &parent_ids,
            );
            reprobe_rules(
                self,
                &cache_index.rules,
                &mut sets,
                &parent_sub,
                false,
                &parent_sub_seen,
                &parent_sub_ids,
            );
            #[cfg(test)]
            LAST_SYNC_SCOPED.with(|c| {
                c.set(if scoped_rules == 0 {
                    None
                } else {
                    Some(
                        elem_ids.len()
                            + subtree_ids.len()
                            + parent_ids.len()
                            + parent_sub_ids.len(),
                    )
                })
            });
        } else {
            #[cfg(test)]
            LAST_SYNC_SCOPED.with(|c| c.set(None));
        }
        // A fresh build only keys rules that matched at least one element;
        // drop the rule entries the purge emptied so the shapes stay
        // identical.
        sets.hits.retain(|_, v| !v.is_empty());
        sets.pseudo_hits
            .retain(|_, s| s.iter().any(|v| !v.is_empty()));
        #[cfg(test)]
        LAST_SYNC_AFFECTED.with(|c| c.set(Some(affected.len())));
        let out = Arc::new(sets);
        self.set_match_cache(MatchCacheBox {
            sets: out.clone(),
            ..cache
        });
        out
    }
}

#[cfg(test)]
thread_local! {
    /// Fresh-rule count of the last remap on this thread — test probe.
    pub(crate) static LAST_REMAP_FRESH: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
    /// The last sync's path: `Some(affected rule count)` when it stayed
    /// incremental, `None` when it rebuilt — test probe.
    pub(crate) static LAST_SYNC_AFFECTED: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
    /// The last sync's re-match domain: `Some(zone size)` when the
    /// affected rules re-matched inside the attr-write zone, `None` when
    /// document-wide or rebuilt — test probe.
    pub(crate) static LAST_SYNC_SCOPED: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

impl DomTree {
    /// Rebuild the persisted match state for a changed rule list without
    /// re-matching the unchanged rules. New rule `j` takes the hit sets of
    /// an old rule whose selector text is byte-equal (which old index, if
    /// several, does not matter: equal text, equal sets); rules with no
    /// such twin are probed against every attached element. The carried
    /// sets still describe the tree as of the last sync — the caller's
    /// dirty sync runs next, exactly as on an unchanged sheet.
    fn remap_match_cache(
        &self,
        old: MatchCacheBox,
        rule_selectors: &[&str],
        css_key: u64,
        trace: bool,
    ) -> MatchCacheBox {
        let mut old_pos: HashMap<&str, usize> = HashMap::with_capacity(old.selectors.len());
        for (i, sel) in old.selectors.iter().enumerate() {
            old_pos.entry(sel.as_str()).or_insert(i);
        }
        // Equal text, equal compiled selector: only unseen texts re-parse.
        let compiled: Vec<CompiledRule> = rule_selectors
            .iter()
            .map(|sel| match old_pos.get(sel) {
                Some(&i) => old.index.rules[i].clone(),
                None => compile_rule(sel),
            })
            .collect();
        let index = self.assemble_rule_index(compiled);
        let mut hits = HashMap::new();
        let mut pseudo_hits = HashMap::new();
        let mut fresh: Vec<usize> = Vec::new();
        for (j, sel) in rule_selectors.iter().enumerate() {
            match old_pos.get(sel) {
                Some(&i) => {
                    if let Some(h) = old.sets.hits.get(&i) {
                        hits.insert(j, h.clone());
                    }
                    if let Some(p) = old.sets.pseudo_hits.get(&i) {
                        pseudo_hits.insert(j, p.clone());
                    }
                }
                None => fresh.push(j),
            }
        }
        let fresh_count = fresh.len();
        if fresh_count > 0 {
            // A sub-index of just the fresh rules: probing the full index
            // and filtering would still gather every bucket's candidates
            // (thousands of `... span` rules per span on atomic sheets).
            let sub =
                self.assemble_rule_index(fresh.iter().map(|&j| index.rules[j].clone()).collect());
            let mut sub_hits = HashMap::new();
            let mut sub_pseudo = HashMap::new();
            self.probe_elements_into(
                &sub,
                self.match_probe_all_ids(),
                &mut sub_hits,
                &mut sub_pseudo,
            );
            hits.extend(sub_hits.into_iter().map(|(k, v)| (fresh[k], v)));
            pseudo_hits.extend(sub_pseudo.into_iter().map(|(k, v)| (fresh[k], v)));
        }
        #[cfg(test)]
        LAST_REMAP_FRESH.with(|c| c.set(Some(fresh_count)));
        if trace {
            eprintln!(
                "[match-trace] REMAP key={css_key:#x} rules {}->{} fresh={fresh_count}",
                old.selectors.len(),
                rule_selectors.len()
            );
        }
        MatchCacheBox {
            selectors: rule_selectors.iter().map(|s| s.to_string()).collect(),
            sets: Arc::new(RuleMatchSets {
                hits,
                specificity: index.specificity.clone(),
                pseudo_hits,
            }),
            index,
        }
    }
}

/// One rule's quirks-independent compiled selector state: the plain list,
/// the pseudo routing, the specificity, each sub-selector's rightmost key
/// (`None` = keyless), and whether any sub-selector bears `:has()`.
#[derive(Clone)]
struct CompiledRule {
    plain: Option<SelectorList<DitingSelector>>,
    routing: Option<PseudoRouting>,
    specificity: Option<u32>,
    keys: Vec<Option<RuleKey>>,
    deps: RuleDeps,
}

fn compile_rule(selector: &str) -> CompiledRule {
    // A trailing pseudo-element suffix routes that sub-selector to
    // the pseudo cascade instead of the normal one — decided per
    // SUB-SELECTOR, because comma lists mix them freely
    // (`.row:after,.row:before` is every legacy clearfix,
    // Bootstrap 3's whole grid). Each part is split, its
    // `::before`/`::after` suffix stripped, and the plain
    // remainder keeps cascading normally: `.d3 p,.d3:after`
    // styles `p` exactly like Chrome and routes only the ::after.
    let mut plain_parts: Vec<String> = Vec::new();
    let mut before_parts: Vec<String> = Vec::new();
    let mut after_parts: Vec<String> = Vec::new();
    for part in split_top_level_commas(selector) {
        let p = part.trim();
        let lower = p.to_ascii_lowercase();
        // ::before strips before :before, or `.x::before` would
        // leave a dangling colon on the base.
        let stripped = lower
            .strip_suffix("::before")
            .map(|rest| (rest.len(), 0))
            .or_else(|| lower.strip_suffix("::after").map(|rest| (rest.len(), 1)))
            .or_else(|| lower.strip_suffix(":before").map(|rest| (rest.len(), 0)))
            .or_else(|| lower.strip_suffix(":after").map(|rest| (rest.len(), 1)));
        // to_ascii_lowercase is byte-for-byte, so slicing the
        // original by the lowered length keeps the author's case.
        match stripped {
            Some((base_len, slot)) => match p.get(..base_len) {
                Some(base) if !base.trim().is_empty() => {
                    if slot == 0 {
                        before_parts.push(base.to_string());
                    } else {
                        after_parts.push(base.to_string());
                    }
                }
                // A bare pseudo (base empty) parses as nothing;
                // let it die in the plain pile like a parse error.
                _ => plain_parts.push(p.to_string()),
            },
            None => plain_parts.push(p.to_string()),
        }
    }
    let routing = (!before_parts.is_empty() || !after_parts.is_empty()).then(|| PseudoRouting {
        before: parse_selector(&before_parts.join(",")).ok(),
        after: parse_selector(&after_parts.join(",")).ok(),
    });
    let plain = if plain_parts.is_empty() {
        None
    } else {
        parse_selector(&plain_parts.join(",")).ok()
    };
    // Base-spec semantics, unchanged: the rule keeps one specificity
    // from its first surviving list (the pseudo bit itself stays
    // uncounted).
    let first_list = plain
        .as_ref()
        .or(routing.as_ref().and_then(|r| r.before.as_ref()))
        .or(routing.as_ref().and_then(|r| r.after.as_ref()));
    let specificity = first_list.and_then(|l| l.slice().first().map(|s| s.specificity()));
    // A comma list matches if ANY sub-selector matches, so every
    // sub-selector of every list — plain and pseudo bases alike —
    // contributes its own key.
    let mut lists: Vec<&SelectorList<DitingSelector>> = Vec::new();
    if let Some(list) = plain.as_ref() {
        lists.push(list);
    }
    if let Some(r) = routing.as_ref() {
        if let Some(list) = r.before.as_ref() {
            lists.push(list);
        }
        if let Some(list) = r.after.as_ref() {
            lists.push(list);
        }
    }
    let keys = lists
        .iter()
        .flat_map(|l| l.slice())
        .map(rightmost_key)
        .collect();
    let mut deps = RuleDeps::default();
    for sel in lists.iter().flat_map(|l| l.slice()) {
        deps.add_selector(sel);
    }
    for v in [&mut deps.classes, &mut deps.ids, &mut deps.attrs] {
        v.sort_unstable();
        v.dedup();
    }
    CompiledRule {
        specificity,
        keys,
        deps,
        plain,
        routing,
    }
}

/// The rightmost-compound key a rule selector's subject element must
/// carry, ranked by selectivity (id > class > tag). Extracted per
/// sub-selector for [`DomTree::rule_match_sets`]'s buckets; a compound
/// with none of the three keys nothing, so its rule is tested against
/// every element.
#[derive(Clone)]
enum RuleKey {
    Tag(String),
    Class(String),
    Id(String),
}

impl RuleKey {
    fn rank(&self) -> u8 {
        match self {
            RuleKey::Tag(_) => 0,
            RuleKey::Class(_) => 1,
            RuleKey::Id(_) => 2,
        }
    }
}

/// Extract the key from one parsed selector's rightmost compound. Servo's
/// `Selector::iter()` yields components right-to-left starting at the
/// rightmost compound; the iterator stops at the compound boundary, which
/// is exactly the span that constrains the subject element (anything left
/// of a combinator describes ancestors, not the subject).
fn rightmost_key(selector: &parser::Selector<DitingSelector>) -> Option<RuleKey> {
    let mut best: Option<RuleKey> = None;
    for component in selector.iter() {
        let key = match component {
            parser::Component::ID(id) => Some(RuleKey::Id(id.0.clone())),
            parser::Component::Class(class) => Some(RuleKey::Class(class.0.clone())),
            // The DOM-side probe reaches this bucket through the element's
            // lowercased local name, so the parser's lower_name is the
            // spelling both sides agree on.
            parser::Component::LocalName(name) => Some(RuleKey::Tag(name.lower_name.0.to_string())),
            _ => None,
        };
        if let Some(key) = key {
            if best.as_ref().is_none_or(|b| key.rank() > b.rank()) {
                best = Some(key);
            }
        }
    }
    best
}

/// Chrome-style rule-hash match sets for stylesheet application, built by
/// [`DomTree::rule_match_sets`].
///
/// `compute_styles` used to precompute each rule's document match set
/// with one full-document `querySelectorAll` per rule: O(rules x docsize)
/// selector matches per layout run, re-paid on every epoch bump. On the
/// WeChat article pages (6912 rules, 483 elements, 3 MB of CSS) that
/// phase alone was ~3.7s of EVERY layout run, and page scripts'
/// write-then-read layout thrashing re-triggered it several times per
/// navigation, the engine-side root cause behind the appmsg.js
/// synchronous "dead spin" that blew the nav deadline (a V8 terminate
/// cannot land inside a long Rust phase).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleMatchSets {
    /// Rule index to matching element node indices, ascending: ready for
    /// the cascade's per-element binary-search membership test (which the
    /// old code documented but then did linearly).
    pub hits: HashMap<usize, Vec<usize>>,
    /// Rule index to the rule selector's specificity, parsed once here
    /// instead of re-parsed per matched element (the old cascade called
    /// `compile_rule_selector` per element x matched rule).
    pub specificity: Vec<Option<u32>>,
    /// Pseudo-element rule hit sets, slot-indexed (0 = `::before`, 1 =
    /// `::after`): each rule's comma list routes its pseudo sub-selectors
    /// per slot, so `.row:after,.row:before` fills both slots from one
    /// rule while `.d3 p,.d3:after` feeds the normal cascade its `p`.
    pub pseudo_hits: HashMap<usize, [Vec<usize>; 2]>,
}

/// Per-rule pseudo-element routing after comma-list splitting: the bases
/// of the rule's `::before`/`::after` sub-selectors with their suffixes
/// stripped, kept apart from [`CompiledRule::plain`]'s sub-selectors so
/// the normal cascade never applies a pseudo base to real elements.
#[derive(Clone)]
struct PseudoRouting {
    before: Option<SelectorList<DitingSelector>>,
    after: Option<SelectorList<DitingSelector>>,
}

/// The compiled selector-side state a match run probes against: parsed
/// lists, specificities, pseudo routing, and the rightmost-compound
/// buckets (see [`DomTree::rule_match_sets`]). Lives in
/// [`MatchCacheBox`] across incremental runs, with its selector texts.
pub(crate) struct RuleIndex {
    /// Per-rule compiled selectors, rule order.
    rules: Vec<CompiledRule>,
    specificity: Vec<Option<u32>>,
    by_id: HashMap<String, Vec<usize>>,
    by_class: HashMap<String, Vec<usize>>,
    by_tag: HashMap<String, Vec<usize>>,
    unkeyed: Vec<usize>,
    quirks: bool,
    /// Rule-scoped invalidation tables (see [`RuleDeps`]): lowercased
    /// class/id/attribute-name keys to the rules that read them, plus the
    /// structural, state and wild rule lists.
    dep_class: HashMap<String, Vec<usize>>,
    dep_id: HashMap<String, Vec<usize>>,
    dep_attr: HashMap<String, Vec<usize>>,
    dep_class_desc: HashMap<String, Vec<usize>>,
    dep_id_desc: HashMap<String, Vec<usize>>,
    dep_attr_desc: HashMap<String, Vec<usize>>,
    dep_class_sib: HashMap<String, Vec<usize>>,
    dep_id_sib: HashMap<String, Vec<usize>>,
    dep_attr_sib: HashMap<String, Vec<usize>>,
    struct_rules: Vec<usize>,
    state_rules: Vec<usize>,
    wild_rules: Vec<usize>,
    zone_unsafe_rules: Vec<usize>,
}

/// The persisted match state for [`DomTree::rule_match_sets_incremental`]:
/// the compiled index and the live hit sets for `selectors`, valid as long
/// as every tree mutation since the last sync carried a [`MatchDirty`]
/// stamp.
pub(crate) struct MatchCacheBox {
    /// The selector texts `index` was compiled from, rule order — the
    /// remap across css changes keys on exact string equality.
    selectors: Vec<String>,
    index: RuleIndex,
    /// Shared with the handle the last sync handed out: when the caller
    /// drops its handle before the next sync (the style run's shape), the
    /// sync moves the sets in place instead of copying the whole hit map.
    sets: Arc<RuleMatchSets>,
}

/// Mutations the incremental matcher needs to know about, stamped by the
/// tree at every structural change and by the `note_*` hooks at attribute,
/// state, text and live-control writes. Consumed (emptied) by the next
/// [`DomTree::rule_match_sets_incremental`] sync.
#[derive(Default)]
pub(crate) struct MatchDirty {
    /// Roots of inserted or re-parented subtrees: every element at and
    /// below re-probes against every rule.
    pub(crate) roots: Vec<NodeId>,
    /// Roots of detached subtrees: same treatment as `roots` — if the
    /// subtree was re-inserted elsewhere it also carries a `roots` stamp
    /// and re-probes; if it stayed detached the purge drops its hits; if
    /// the slot was freed and reused the new occupant carries its own
    /// insert stamp, so the purge never outlives its meaning.
    pub(crate) purge_roots: Vec<NodeId>,
    /// A child list or a text node changed somewhere: structural rules
    /// re-match inside the parent zones.
    pub(crate) structure: bool,
    /// Hover/active/focus moved: state rules re-match document-wide.
    pub(crate) state: bool,
    /// A form control's live value/checkedness changed: wild rules only.
    pub(crate) live: bool,
    /// Lowercased class tokens, ids and attribute names attribute writes
    /// changed: the rules reading them re-match inside the write zones.
    pub(crate) classes: Vec<String>,
    pub(crate) ids: Vec<String>,
    pub(crate) attrs: Vec<String>,
    /// Zone anchors for this window's mutations, bounding where a rule
    /// whose dependencies changed can have flipped (see the sync's pass 2).
    ///
    /// `elem_zones` — the elements attr writes landed on. Which slice of
    /// the tree a fired dep re-matches depends on its side in the selector
    /// (RuleDeps): subject-side deps only at the element itself, descendant
    /// deps in its subtree, sibling deps around it (the parent zone).
    ///
    /// `parent_zones` — the parents of attr-written elements, and of
    /// inserted/detached/text-edited nodes. A structurally-depending rule
    /// (sibling combinators, :nth-*, :first/last/only, :empty) can flip at
    /// the changed element's siblings and their subtrees, which the
    /// element zone cannot see.
    ///
    /// Each list is capped; an overflow degrades its rules' re-match to
    /// document-wide.
    pub(crate) elem_zones: Vec<NodeId>,
    pub(crate) parent_zones: Vec<NodeId>,
    pub(crate) zone_full: bool,
    /// Set when a mutation path the registry cannot describe ran — the
    /// next sync rebuilds from scratch.
    pub(crate) full: bool,
}

impl MatchDirty {
    pub(crate) fn inserted(&mut self, root: NodeId) {
        self.roots.push(root);
        self.structure = true;
    }

    pub(crate) fn detached(&mut self, root: NodeId) {
        self.purge_roots.push(root);
        self.structure = true;
    }

    /// Anchor the element zone at a written element and the parent zone at
    /// its parent — see [`MatchDirty::elem_zones`]. The caller resolves
    /// the parent while it still can (a detach unlinks first).
    pub(crate) fn attr_zone(&mut self, elem: NodeId, parent: Option<NodeId>) {
        if self.elem_zones.len() >= 256 {
            self.zone_full = true;
        } else {
            self.elem_zones.push(elem);
        }
        if let Some(p) = parent {
            self.parent_zone(p);
        }
    }

    /// Anchor the parent zone at `parent` (inserts, detaches, text edits).
    pub(crate) fn parent_zone(&mut self, parent: NodeId) {
        if self.parent_zones.len() >= 256 {
            self.zone_full = true;
        } else {
            self.parent_zones.push(parent);
        }
    }

    /// One attribute write, old value read before it landed. A
    /// byte-identical rewrite changes nothing a selector can see.
    pub(crate) fn attr_change(&mut self, name: &str, old: Option<&str>, new: Option<&str>) {
        if old == new {
            return;
        }
        let lower = name.to_ascii_lowercase();
        // A qualified name (`xlink:href`) is matched by its local part.
        if let Some((_, local)) = lower.rsplit_once(':') {
            self.attrs.push(local.to_string());
        }
        match lower.as_str() {
            "class" => {
                let toks = |v: Option<&str>| -> HashSet<String> {
                    v.unwrap_or("")
                        .split_ascii_whitespace()
                        .map(str::to_string)
                        .collect()
                };
                let (a, b) = (toks(old), toks(new));
                self.classes
                    .extend(a.symmetric_difference(&b).map(|t| t.to_ascii_lowercase()));
            }
            "id" => self.ids.extend(
                [old, new]
                    .into_iter()
                    .flatten()
                    .map(|v| v.to_ascii_lowercase()),
            ),
            _ => {}
        }
        self.attrs.push(lower);
    }
}
