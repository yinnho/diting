//! Cascade over a `DomTree`: `compute_styles*` and the `@container` plan.
//!
//! Split from the layout hub. The public entry points stay re-exported
//! from the crate root. Pure move: this pass sees the same crate
//! namespace it grew up in.

use super::*;

/// Compute every element's cascade result for a parsed tree + stylesheet:
/// the production style-resolution pass feeding [`layout_dom`]. Walks the
/// tree once; each element matches the full rule set (O(rules × elements) —
/// fine for page-sized inputs, a matching index is future work), chains
/// inherited properties from the parent's computed style, and applies the
/// inline `style` attribute last.
/// Per-element gating for `@container` arms (moli#282): the container
/// conditions answer against the element's nearest qualifying container
/// ancestor, which only exists after a layout pass — so the plan is built
/// between a probe solve and the final cascade. `gates` is keyed by
/// ABSOLUTE rule index in the extended rule slice (`container arms sit at
/// index >= base_len`); values are ascending DOM node indexes ready for
/// the cascade's binary-search membership test.
pub struct ContainerPlan {
    pub extra_rules: Vec<diting_css::ParsedRule>,
    pub gates: HashMap<usize, Vec<usize>>,
}

struct ContainerStackEntry {
    name: Option<String>,
    w: f32,
    h: f32,
}

/// Evaluate every container rule's inner selectors against the tree and
/// record which elements they reach through a passing container. Rules with
/// container-query units (cqw/cqh/cqi/cqb) in their declarations are split
/// per distinct container size so the units bake to concrete px per group.
pub fn container_plan(
    tree: &DomTree,
    styles: &HashMap<NodeId, diting_css::ComputedStyle>,
    boxes: &HashMap<NodeId, (f32, f32)>,
    container_rules: &[diting_css::ContainerRule],
    base_len: usize,
) -> ContainerPlan {
    use diting_css::ContainerType;
    let mut entries: Vec<(
        diting_css::ParsedRule,
        Option<String>,
        Option<diting_css::ContainerCondition>,
    )> = Vec::new();
    for cr in container_rules {
        for r in &cr.rules {
            entries.push((r.clone(), cr.name.clone(), cr.condition.clone()));
        }
    }
    if entries.is_empty() {
        return ContainerPlan {
            extra_rules: Vec::new(),
            gates: HashMap::new(),
        };
    }
    let selectors: Vec<&str> = entries
        .iter()
        .map(|(r, _, _)| r.selector.as_str())
        .collect();
    let sets = tree.rule_match_sets(&selectors);
    // Per inner rule: (node index, container w, container h) in document order.
    let mut acc: Vec<Vec<(usize, f32, f32)>> = vec![Vec::new(); entries.len()];
    #[allow(clippy::too_many_arguments)]
    fn dfs(
        tree: &DomTree,
        styles: &HashMap<NodeId, diting_css::ComputedStyle>,
        boxes: &HashMap<NodeId, (f32, f32)>,
        sets: &diting_dom::selector::RuleMatchSets,
        entries: &[(
            diting_css::ParsedRule,
            Option<String>,
            Option<diting_css::ContainerCondition>,
        )],
        acc: &mut [Vec<(usize, f32, f32)>],
        nid: NodeId,
        stack: &mut Vec<ContainerStackEntry>,
    ) {
        let is_element = tree.with_node(nid, |n| n.is_element()).unwrap_or(false);
        if is_element {
            // The element's own container-type cannot satisfy its own query:
            // gate evaluation runs before the self-push below.
            if !stack.is_empty() {
                for (ri, (_, name, condition)) in entries.iter().enumerate() {
                    if sets
                        .hits
                        .get(&ri)
                        .is_none_or(|h| h.binary_search(&nid.index()).is_err())
                    {
                        continue;
                    }
                    let target = stack.iter().rev().find(|c| {
                        name.as_ref()
                            .is_none_or(|n| c.name.as_deref() == Some(n.as_str()))
                    });
                    if let Some(c) = target {
                        if condition.as_ref().is_none_or(|cond| cond.matches(c.w, c.h)) {
                            acc[ri].push((nid.index(), c.w, c.h));
                        }
                    }
                }
            }
            if let Some(cs) = styles.get(&nid) {
                if cs.container_type != ContainerType::Normal {
                    if let Some(&(w, h)) = boxes.get(&nid) {
                        stack.push(ContainerStackEntry {
                            name: cs.container_name.clone(),
                            w,
                            h,
                        });
                    }
                }
            }
        }
        for child in tree.children(nid) {
            dfs(tree, styles, boxes, sets, entries, acc, child, stack);
        }
        if is_element {
            if let Some(cs) = styles.get(&nid) {
                if cs.container_type != ContainerType::Normal && boxes.contains_key(&nid) {
                    stack.pop();
                }
            }
        }
    }
    let mut stack = Vec::new();
    dfs(
        tree,
        styles,
        boxes,
        &sets,
        &entries,
        &mut acc,
        tree.document(),
        &mut stack,
    );

    let mut extra_rules = Vec::new();
    let mut gates: HashMap<usize, Vec<usize>> = HashMap::new();
    for (ri, (rule, _, _)) in entries.iter().enumerate() {
        let hits = &acc[ri];
        if hits.is_empty() {
            continue;
        }
        let has_cq_units = ["cqw", "cqh", "cqi", "cqb"]
            .iter()
            .any(|u| rule.declarations.contains(u));
        if !has_cq_units {
            let idx = base_len + extra_rules.len();
            let mut nodes: Vec<usize> = hits.iter().map(|(n, _, _)| *n).collect();
            nodes.sort_unstable();
            nodes.dedup();
            extra_rules.push(rule.clone());
            gates.insert(idx, nodes);
        } else {
            // Same selector may answer through different containers → the
            // unit bake differs per container. Group by container size.
            let mut groups: std::collections::HashMap<(i64, i64), (f32, f32, Vec<usize>)> =
                std::collections::HashMap::new();
            for (n, w, h) in hits {
                let key = ((w * 100.0) as i64, (h * 100.0) as i64);
                groups.entry(key).or_insert((*w, *h, Vec::new())).2.push(*n);
            }
            for (_, (w, h, mut nodes)) in groups {
                let idx = base_len + extra_rules.len();
                nodes.sort_unstable();
                nodes.dedup();
                extra_rules.push(diting_css::ParsedRule {
                    declarations: bake_container_units(&rule.declarations, w, h),
                    ..rule.clone()
                });
                gates.insert(idx, nodes);
            }
        }
    }
    ContainerPlan { extra_rules, gates }
}

pub fn compute_styles(
    tree: &DomTree,
    rules: &[diting_css::ParsedRule],
    viewport: (f32, f32),
) -> HashMap<NodeId, diting_css::ComputedStyle> {
    // Static renders (screenshot/svg paths): no registered transitions —
    // the cascade values stand.
    compute_styles_timed(
        tree,
        rules,
        &diting_css::KeyframesMap::new(),
        None,
        &[],
        viewport,
        None,
    )
}

/// The animated face of [`compute_styles`]: after the cascade resolves each
/// element, its `animation` shorthand is sampled against the stylesheet's
/// `@keyframes` table at `css_time`. `None` samples the end state (the
/// poster posture for static renders); the video pump feeds real seconds
/// here. Everything the sampler touches is paint-channel only
/// (opacity/transform/stroke-dashoffset), so this runs inside the
/// collect-cache (#395) invalidation domain — `set_css_time` drops the
/// paint-only caches and the next read re-cascades fresh.
#[allow(clippy::too_many_arguments)]
fn compute_styles_impl(
    tree: &DomTree,
    rules: &[diting_css::ParsedRule],
    keyframes: &diting_css::KeyframesMap,
    css_time: Option<f64>,
    within_root: Option<NodeId>,
    transitions: &[diting_css::CssTransition],
    gated: Option<(usize, &HashMap<usize, Vec<usize>>)>,
    viewport: (f32, f32),
    css_key: Option<u64>,
) -> HashMap<NodeId, diting_css::ComputedStyle> {
    // styles-trace phase accumulators (#109 walk breakdown): nanoseconds in
    // cascade_element, pseudo_styles, and children() enumeration.
    thread_local! {
        static T_CASCADE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
        static T_PSEUDO: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
        static T_CHILDREN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    T_CASCADE.with(|c| c.set(0));
    T_PSEUDO.with(|c| c.set(0));
    T_CHILDREN.with(|c| c.set(0));
    #[allow(clippy::too_many_arguments)]
    fn visit(
        tree: &DomTree,
        rules: &[diting_css::ParsedRule],
        node_rules: &[Vec<(usize, u32)>],
        node_pseudo: &[[Vec<(usize, u32)>; 2]],
        has_pseudo: bool,
        keyframes: &diting_css::KeyframesMap,
        css_time: Option<f64>,
        nid: NodeId,
        parent: Option<&diting_css::ComputedStyle>,
        root_fs: f32,
        out: &mut HashMap<NodeId, diting_css::ComputedStyle>,
        counters: &mut CounterState,
        depth: usize,
        gated: Option<(usize, &HashMap<usize, Vec<usize>>)>,
        viewport: (f32, f32),
        zoom_accum: f32,
        memo: &mut Option<style_memo::MemoRun>,
        parent_version: Option<u64>,
    ) {
        let Some(tag) = tree
            .with_node(nid, |n| n.as_element().map(|e| e.local.to_string()))
            .flatten()
        else {
            return;
        };
        let matched_idx: Vec<(usize, u32)> = node_rules[nid.index()]
            .iter()
            .copied()
            .filter(|&(ri, _)| {
                // The per-rule match sets were precomputed once (below); this
                // used to scan the ENTIRE rule table per element — with real
                // sheet sizes (tmall publish: 17,300 rules × 7,678 elements
                // ≈ 133M hash probes, 9.9s of an 11.2s layout) the membership
                // test alone was the cost (#109). The buckets below hand each
                // element its matched rules in ascending rule order, so
                // cascade order matches the old scan exactly.
                // @container arms (index >= base) cascade only on elements
                // the container plan gated through a passing container.
                match gated {
                    Some((base, gates)) => {
                        ri < base
                            || gates
                                .get(&ri)
                                .is_some_and(|v| v.binary_search(&nid.index()).is_ok())
                    }
                    None => true,
                }
            })
            .collect();
        let key = style_memo::ElemKey {
            nid,
            tag: &tag,
            matched: &matched_idx,
            parent_version,
            root_fs,
        };
        let t_c = std::time::Instant::now();
        // A memo miss keeps one copy of the raw output for the record.
        let (mut cs, fresh) = match memo.as_ref().and_then(|m| m.lookup(tree, &key)) {
            Some(remembered) => (remembered, None),
            None => {
                let matched: Vec<(&diting_css::ParsedRule, u32)> = matched_idx
                    .iter()
                    .map(|&(ri, spec)| (&rules[ri], spec))
                    .collect();
                let inline = tree
                    .with_node(nid, |n| n.get_attribute("style").map(|s| s.to_string()))
                    .flatten();
                let raw = diting_css::cascade_element(
                    &tag,
                    tree,
                    nid,
                    &matched,
                    parent,
                    inline.as_deref(),
                    root_fs,
                    viewport,
                );
                match memo {
                    Some(_) => (raw.clone(), Some(raw)),
                    None => (raw, None),
                }
            }
        };
        T_CASCADE.with(|c| c.set(c.get() + t_c.elapsed().as_nanos() as u64));
        // The memo holds raw cascade output; everything after it (animation
        // sample, counters, pseudos, zoom) re-runs each pass.
        diting_css::sample_css_animation(&mut cs, keyframes, css_time);
        let my_version = memo.as_mut().map(|m| m.record(tree, &key, fresh, &cs));
        // #120 zoom: this node's accumulated factor — ancestors' product
        // times THIS element's own declaration (`zoom_declared`, never the
        // inherited `zoom`, which already carries the ancestor factor).
        // The cascade above (and every pseudo/child below) must see
        // UNSCALED px — em/% fold against unscaled ancestors — so the
        // geometry scaling happens only at the out.insert at the bottom,
        // on a clone.
        let z = zoom_accum * cs.zoom_declared.unwrap_or(1.0);
        // CSS counters: apply this element's own reset/increment before its
        // pseudos resolve. The reset pushes a shadowing counter whose scope
        // lasts until this element's parent's subtree ends (see
        // pop_out_of_scope) — css-lists-3 scoping, not a flat clobber.
        // display:none elements generate no boxes and no counters.
        if cs.display != Some(diting_css::Display::None)
            && (!cs.counter_reset.is_empty() || !cs.counter_increment.is_empty())
        {
            apply_counter_modifiers(counters, &cs.counter_reset, &cs.counter_increment, depth);
        }
        // Generated content: pseudo-element rules cascade a synthetic span
        // against the host's own matched set and hang off the host's
        // computed style; build_element synthesizes their boxes. ::before
        // resolves before the children and ::after after — quote depth and
        // counter mutations interleave with the subtree walk (css-content-3
        // box order), so one-shot resolution would close quotes opened
        // before the children ever ran.
        let mut pseudo_pair = diting_css::PseudoPair::default();
        if has_pseudo {
            let t_p = std::time::Instant::now();
            pseudo_pair.before = pseudo_styles(
                tree,
                rules,
                &node_pseudo[nid.index()][0],
                nid,
                &cs,
                root_fs,
                counters,
                depth,
                gated,
                viewport,
            );
            T_PSEUDO.with(|c| c.set(c.get() + t_p.elapsed().as_nanos() as u64));
        }
        let child_root_fs = if parent.is_none() {
            cs.font_size.unwrap_or(diting_css::DEFAULT_ROOT_FONT_SIZE)
        } else {
            root_fs
        };
        // Composed-tree children, each paired with its flat-tree
        // inheritance parent. A shadow host's shadow children inherit from
        // the host; a slot's assigned light children inherit from the SLOT
        // (pinned to Chrome 2026-09-16: `color` on the slot colors the
        // slotted child, and with it undeclared the host's value walks
        // through) — so the slot is walked before its assigned nodes and
        // its computed style is already in `out` when they cascade. A slot
        // with nothing assigned composes nothing here; its own visit
        // enumerates the fallback children against it. Unassigned light
        // children of a host are never visited — they render nothing, so
        // no computed style either.
        let slot_consumed = tree.is_html_slot_element(nid)
            && tree.assigned_nodes(nid).is_some_and(|a| !a.is_empty());
        let t_ch = std::time::Instant::now();
        let walk: Vec<(NodeId, NodeId)> = if slot_consumed {
            Vec::new()
        } else {
            let raw: Vec<NodeId> = match tree.shadow_root(nid) {
                Some(root) => tree.children(root),
                None => tree.children(nid),
            };
            let mut w = Vec::with_capacity(raw.len());
            for child in raw {
                w.push((child, nid));
                if tree.is_html_slot_element(child) {
                    if let Some(assigned) = tree.assigned_nodes(child) {
                        for node in assigned {
                            w.push((node, child));
                        }
                    }
                }
            }
            w
        };
        T_CHILDREN.with(|c| c.set(c.get() + t_ch.elapsed().as_nanos() as u64));
        // The common branch hands each child a borrow of THIS element's
        // style — a full ComputedStyle clone per child (7678 per pass on the
        // publish page) was pure overhead when cs outlives the whole loop
        // (#109). Only slot-assigned nodes need a parent outside this frame,
        // and those clone out of `out` as before.
        for (child, inherit_from) in walk {
            if inherit_from == nid {
                visit(
                    tree,
                    rules,
                    node_rules,
                    node_pseudo,
                    has_pseudo,
                    keyframes,
                    css_time,
                    child,
                    Some(&cs),
                    child_root_fs,
                    out,
                    counters,
                    depth + 1,
                    gated,
                    viewport,
                    z,
                    memo,
                    my_version,
                );
            } else {
                // Slot-assigned nodes inherit from a style already pulled
                // from `out` — scaled, if that slot sits inside a zoom
                // subtree (#120: em folding against the scaled font size
                // double-counts the factor there; zoom+slots hasn't shipped
                // together in the wild yet).
                let parent = out
                    .get(&inherit_from)
                    .cloned()
                    .unwrap_or_else(|| cs.clone());
                visit(
                    tree,
                    rules,
                    node_rules,
                    node_pseudo,
                    has_pseudo,
                    keyframes,
                    css_time,
                    child,
                    Some(&parent),
                    child_root_fs,
                    out,
                    counters,
                    depth + 1,
                    gated,
                    viewport,
                    z,
                    memo,
                    None,
                );
            }
        }
        if has_pseudo {
            let t_p = std::time::Instant::now();
            pseudo_pair.after = pseudo_styles(
                tree,
                rules,
                &node_pseudo[nid.index()][1],
                nid,
                &cs,
                root_fs,
                counters,
                depth,
                gated,
                viewport,
            );
            T_PSEUDO.with(|c| c.set(c.get() + t_p.elapsed().as_nanos() as u64));
        }
        if pseudo_pair.before.is_some() || pseudo_pair.after.is_some() {
            cs.pseudos = Some(Box::new(pseudo_pair));
        }
        pop_out_of_scope(counters, depth);
        // #120 zoom: scale geometry px into display coordinates now that the
        // whole subtree (children inherited unscaled values, pseudos cascaded
        // against the unscaled host) has been walked. Pseudo boxes ride the
        // host's factor — they have no children of their own. Off zoom
        // subtrees (z == 1) nothing is cloned; the cascade output lands as-is.
        if (z - 1.0).abs() > f32::EPSILON {
            let mut scaled = cs.clone();
            diting_css::scale_computed_px(&mut scaled, z);
            if let Some(pp) = scaled.pseudos.as_mut() {
                if let Some(before) = pp.before.as_mut() {
                    diting_css::scale_computed_px(before, z);
                }
                if let Some(after) = pp.after.as_mut() {
                    diting_css::scale_computed_px(after, z);
                }
            }
            scaled.zoom_accum = z;
            out.insert(nid, scaled);
        } else {
            cs.zoom_accum = 1.0;
            out.insert(nid, cs);
        }
    }
    // Pseudo-element rules (::before/::after) for one host: the caller hands
    // in the host's pre-bucketed pseudo matches (slot 0 = before, 1 = after;
    // the selector layer kept pseudo rules OUT of the normal buckets), and
    // each kind cascades a synthetic span whose parent is the host —
    // undeclared properties inherit exactly like a real child's would.
    #[allow(clippy::too_many_arguments)]
    fn pseudo_styles(
        tree: &DomTree,
        rules: &[diting_css::ParsedRule],
        bucket: &[(usize, u32)],
        nid: NodeId,
        host: &diting_css::ComputedStyle,
        root_fs: f32,
        counters: &mut CounterState,
        depth: usize,
        gated: Option<(usize, &HashMap<usize, Vec<usize>>)>,
        viewport: (f32, f32),
    ) -> Option<diting_css::ComputedStyle> {
        let matched: Vec<(&diting_css::ParsedRule, u32)> = bucket
            .iter()
            .filter_map(|&(ri, spec)| {
                if let Some((base, gates)) = gated {
                    if ri >= base
                        && gates
                            .get(&ri)
                            .is_none_or(|v| v.binary_search(&nid.index()).is_err())
                    {
                        return None;
                    }
                }
                Some((&rules[ri], spec))
            })
            .collect();
        if matched.is_empty() {
            return None;
        }
        // Tag "span": the UA sheet's inline display and none of the
        // tag-gated UA branches (their attribute reads fire only on
        // a/td/th/tr hosts).
        let mut p = diting_css::cascade_element(
            "span",
            tree,
            nid,
            &matched,
            Some(host),
            None,
            root_fs,
            viewport,
        );
        // attr() resolves against the HOST's attributes; a missing
        // attribute yields the empty string. Counters and quotes resolve
        // against the walk state: the pseudo's OWN reset/increment apply
        // to the persistent state (css-lists-3 — `li::before
        // {counter-increment: item}` is the canonical pattern; the entry
        // lives at the host's depth, so it survives the host's own
        // subtree exit and dies with the host's parent). The flattened
        // text rides `content_resolved` for the box builder — `content`
        // stays symbolic so the gCS face serves Chrome's functional
        // shape (#19).
        apply_counter_modifiers(counters, &p.counter_reset, &p.counter_increment, depth);
        let quotes = p.quotes.clone();
        if let Some(cv) = p.content.as_ref() {
            let mut out = String::new();
            resolve_content_into(cv, tree, nid, counters, quotes.as_deref(), &mut out);
            p.content_resolved = Some(out);
        }
        // Clearfix box: display:table coerces to block — the generated
        // box needs flow presence (it becomes a taffy child of the host),
        // not the table walk.
        if p.display == Some(diting_css::Display::Table) {
            p.display = Some(diting_css::Display::Block);
        }
        Some(p)
    }
    // One querySelectorAll per RULE over the whole document, sorted for the
    // binary search in visit. This replaces the per-element-per-rule full-doc
    // scan that made style resolution quadratic-cubic on real pages.
    // One bucketed tree walk replaces one full-document querySelectorAll
    // per rule: the rightmost-compound rule hash (see RuleMatchSets for
    // the WeChat numbers behind this) builds every rule's document match
    // set AND each selector's specificity in a single pass, both sorted
    // once for the binary searches in visit.
    let styles_trace = std::env::var("DITING_LAYOUT_TRACE").is_ok();
    let t_styles = std::time::Instant::now();
    let rule_selectors: Vec<&str> = rules.iter().map(|r| r.selector.as_str()).collect();
    // The document run probes document(+shadow) descendants; a within-run
    // probes only the orphan root's subtree (rule_match_sets_within). A
    // css-keyed run syncs the tree's persisted hit sets incrementally
    // instead of re-matching the whole sheet (#111) — the gated and within
    // faces pass None and leave the cache untouched (the gated pass matches
    // an extended rule table; a within pass matches an orphan subtree).
    // The incremental face hands out an Arc to the parked hit sets (the
    // next sync reclaims them in place when this handle is dropped); the
    // one-shot faces wrap their fresh sets to keep the arms one type.
    let sets = match (within_root, css_key) {
        (Some(root), _) => {
            std::sync::Arc::new(tree.rule_match_sets_within(&rule_selectors, &[root]))
        }
        (None, Some(key)) => tree.rule_match_sets_incremental(&rule_selectors, key),
        (None, None) => std::sync::Arc::new(tree.rule_match_sets(&rule_selectors)),
    };
    let t_match = t_styles.elapsed();

    // Inverted match index (#109): visit used to walk the ENTIRE rule table
    // per element (a `hits` hash probe per rule per element, twice more for
    // the pseudo scans) — the membership test itself, not the cascade, was
    // the cost on real sheets. One pass over the rule→hits maps builds
    // node→matched-rule buckets; filling in ascending rule order preserves
    // cascade order exactly. Rules without a parsed specificity drop here,
    // the same None the old per-element filter_map returned for them.
    let node_slots = tree.node_slot_count();
    let mut node_rules: Vec<Vec<(usize, u32)>> = vec![Vec::new(); node_slots];
    let mut node_pseudo: Vec<[Vec<(usize, u32)>; 2]> = vec![[Vec::new(), Vec::new()]; node_slots];
    for (ri, _) in rules.iter().enumerate() {
        let spec = sets.specificity.get(ri).copied().flatten();
        if let (Some(spec), Some(hits)) = (spec, sets.hits.get(&ri)) {
            for &hi in hits {
                node_rules[hi].push((ri, spec));
            }
        }
        if let (Some(spec), Some(slots)) = (spec, sets.pseudo_hits.get(&ri)) {
            for (slot, hits) in slots.iter().enumerate() {
                for &hi in hits {
                    node_pseudo[hi][slot].push((ri, spec));
                }
            }
        }
    }
    let has_pseudo = !sets.pseudo_hits.is_empty();
    let t_buckets = t_styles.elapsed();

    // The memo rides the document faces only: a within-run styles an orphan
    // subtree against a default root, and the plain uncached face
    // (screenshot paths, css_key None) is one-shot. DITING_STYLE_MEMO=0
    // turns it off for A/B triage.
    let memo_slot = match (within_root, gated, css_key) {
        (Some(_), _, _) => None,
        (None, Some(_), _) => Some(1),
        (None, None, Some(_)) => Some(0),
        (None, None, None) => None,
    }
    .filter(|_| std::env::var("DITING_STYLE_MEMO").as_deref() != Ok("0"));
    let mut memo = memo_slot.map(|slot| style_memo::MemoRun::begin(tree, slot, rules, viewport));
    let mut out = HashMap::new();
    let mut counters = CounterState::default();
    match within_root {
        Some(root) => visit(
            tree,
            rules,
            &node_rules,
            &node_pseudo,
            has_pseudo,
            keyframes,
            css_time,
            root,
            None,
            diting_css::DEFAULT_ROOT_FONT_SIZE,
            &mut out,
            &mut counters,
            0,
            gated,
            viewport,
            1.0,
            &mut memo,
            Some(style_memo::ROOT_PARENT),
        ),
        None => {
            for child in tree.children(tree.document()) {
                visit(
                    tree,
                    rules,
                    &node_rules,
                    &node_pseudo,
                    has_pseudo,
                    keyframes,
                    css_time,
                    child,
                    None,
                    diting_css::DEFAULT_ROOT_FONT_SIZE,
                    &mut out,
                    &mut counters,
                    0,
                    gated,
                    viewport,
                    1.0,
                    &mut memo,
                    Some(style_memo::ROOT_PARENT),
                );
            }
        }
    }
    // Exit-phase transition sampling (CSS transitions batch): registered
    // entries override cascade values once per pass, after the visit.
    if !transitions.is_empty() {
        diting_css::sample_css_transitions(transitions, css_time, &mut out);
    }
    let (memo_hits, memo_misses) = memo.as_ref().map_or((0, 0), |m| (m.hits, m.misses));
    if let Some(m) = memo {
        m.finish(tree);
    }
    if styles_trace {
        let total_matched: usize = node_rules.iter().map(|v| v.len()).sum();
        let (a_cascade, a_pseudo, a_children) = (
            T_CASCADE.with(|c| c.get()),
            T_PSEUDO.with(|c| c.get()),
            T_CHILDREN.with(|c| c.get()),
        );
        eprintln!(
            "[styles-trace] rules={} elements={} memo_hits={} memo_misses={} rule_match_sets={} matched={} match={:?} buckets={:?} walk={:?} cascade={:?} pseudo={:?} children={:?} total={:?}",
            rules.len(),
            out.len(),
            memo_hits,
            memo_misses,
            sets.hits.len(),
            total_matched,
            t_match,
            t_buckets - t_match,
            t_styles.elapsed() - t_buckets,
            std::time::Duration::from_nanos(a_cascade),
            std::time::Duration::from_nanos(a_pseudo),
            std::time::Duration::from_nanos(a_children),
            t_styles.elapsed(),
        );
    }
    out
}

/// Document-rooted style resolution (main path). `css_key` identifies the
/// collected css bytes the rules were parsed from: when supplied (and
/// unchanged since the last run) the rule match sets sync incrementally
/// against the tree's dirty registry instead of re-matching the sheet.
#[allow(clippy::too_many_arguments)]
pub fn compute_styles_timed(
    tree: &DomTree,
    rules: &[diting_css::ParsedRule],
    keyframes: &diting_css::KeyframesMap,
    css_time: Option<f64>,
    transitions: &[diting_css::CssTransition],
    viewport: (f32, f32),
    css_key: Option<u64>,
) -> HashMap<NodeId, diting_css::ComputedStyle> {
    compute_styles_impl(
        tree,
        rules,
        keyframes,
        css_time,
        None,
        transitions,
        None,
        viewport,
        css_key,
    )
}

/// Second-pass face for `@container` arms (moli#282): `gates` maps ABSOLUTE
/// rule indexes (>= `base_len`, the container arms appended past the base
/// rules) to the elements that reached them through a passing container.
/// Callers build the plan from a probe pass's geometry, extend `rules` with
/// the plan's extra rules, and re-cascade through here — the gated arms
/// then match exactly those elements.
#[allow(clippy::too_many_arguments)]
pub fn compute_styles_gated(
    tree: &DomTree,
    rules: &[diting_css::ParsedRule],
    keyframes: &diting_css::KeyframesMap,
    css_time: Option<f64>,
    transitions: &[diting_css::CssTransition],
    base_len: usize,
    gates: &HashMap<usize, Vec<usize>>,
    viewport: (f32, f32),
) -> HashMap<NodeId, diting_css::ComputedStyle> {
    compute_styles_impl(
        tree,
        rules,
        keyframes,
        css_time,
        None,
        transitions,
        Some((base_len, gates)),
        viewport,
        None,
    )
}

/// Subtree variant (fabricated iframe documents, obscura #976 family): the
/// orphan root plays the root-element role — no parent cascade, default
/// root font size — and the rule match sets probe only its descendants.
pub fn compute_styles_timed_within(
    tree: &DomTree,
    rules: &[diting_css::ParsedRule],
    keyframes: &diting_css::KeyframesMap,
    css_time: Option<f64>,
    root: NodeId,
    transitions: &[diting_css::CssTransition],
    viewport: (f32, f32),
) -> HashMap<NodeId, diting_css::ComputedStyle> {
    compute_styles_impl(
        tree,
        rules,
        keyframes,
        css_time,
        Some(root),
        transitions,
        None,
        viewport,
        None,
    )
}
