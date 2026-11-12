//! Differential pins for the rule-list remap (insertRule): after every rule
//! insertion/removal/edit and every DOM mutation, the incremental match sets
//! must equal a cold full match on every attached element (the storm test's
//! contract — stale hits on detached slots never reach the cascade and die
//! at reuse/reattach), and with nothing detached they must equal it exactly.

use super::match_index::LAST_REMAP_FRESH;
use crate::tree::{Attribute, DomTree, NodeData, NodeId};
use crate::tree_sink::parse_html;
use diting_html5ever::{ns, LocalName, Namespace, QualName};

/// Selector pool rule lists draw from: keyed, unkeyed, combinators,
/// structural pseudos, :has, pseudo-element routing, quirks-case twins, a
/// parse failure, and exact duplicates.
const POOL: &[&str] = &[
    "div",
    ".a",
    ".b",
    ".c",
    "#w",
    "span",
    "li",
    "div .a",
    "div > span",
    ".a + .b",
    ".a ~ li",
    "li:nth-child(2n)",
    "li:first-child",
    "[data-k]",
    "[data-k='1']",
    "*",
    ":not(.a)",
    "section:has(.c)",
    "section:has(.c) span",
    ".a::before",
    ".b:after, .c",
    ".A",
    "#W",
    "div >",
    "ul li.a",
    ".r-1abc",
    ".r-2def.r-3ghi",
    "em, strong",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn fixture() -> String {
    let mut body = String::from(r#"<div id="w" class="a">"#);
    for s in 0..8 {
        body.push_str(r#"<section><ul>"#);
        for i in 0..6 {
            let class = ["a", "b", "c", "a b", "r-1abc", "r-2def r-3ghi"][(s + i) % 6];
            body.push_str(&format!(
                r#"<li class="{class}" data-k="{}"><span>x</span><em>e</em></li>"#,
                i % 3
            ));
        }
        body.push_str("</ul><div><span class=\"b\">s</span></div></section>");
    }
    body.push_str("</div>");
    // Quirks mode (no doctype): the case-folded bucket spellings ride along.
    format!("<html><body>{body}</body></html>")
}

fn element(rng: &mut Rng) -> NodeData {
    let tag = ["span", "div", "li", "em", "section"][rng.below(5)];
    let class = ["a", "b", "c", "A", "", "r-1abc"][rng.below(6)];
    NodeData::Element {
        name: QualName::new(None, ns!(html), LocalName::from(tag)),
        attrs: vec![Attribute {
            name: QualName::new(None, Namespace::default(), LocalName::from("class")),
            value: class.into(),
        }],
        template_contents: None,
        mathml_annotation_xml_integration_point: false,
        live_value: None,
        live_checked: None,
        live_indeterminate: false,
    }
}

fn attached(tree: &DomTree) -> Vec<bool> {
    let mut at = vec![false; tree.node_slot_count()];
    for d in tree.descendants(tree.document()) {
        at[d.index()] = true;
    }
    at
}

/// Incremental (keyed run) vs cold full match, on attached elements.
fn check(tree: &DomTree, sels: &[&str], key: u64, stage: &str) {
    let inc = tree.rule_match_sets_incremental(sels, key);
    let full = tree.rule_match_sets(sels);
    assert_eq!(inc.specificity, full.specificity, "{stage}: specificity");
    let at = attached(tree);
    let mask = |v: Option<&Vec<usize>>| -> Vec<usize> {
        v.map(|v| v.iter().copied().filter(|&i| at[i]).collect())
            .unwrap_or_default()
    };
    for (ri, sel) in sels.iter().enumerate() {
        assert_eq!(
            mask(inc.hits.get(&ri)),
            mask(full.hits.get(&ri)),
            "{stage}: rule {ri} `{sel}`"
        );
        for slot in 0..2 {
            assert_eq!(
                mask(inc.pseudo_hits.get(&ri).map(|s| &s[slot])),
                mask(full.pseudo_hits.get(&ri).map(|s| &s[slot])),
                "{stage}: rule {ri} `{sel}` pseudo slot {slot}"
            );
        }
    }
}

fn pick_element(tree: &DomTree, rng: &mut Rng) -> NodeId {
    let w = tree.get_element_by_id("w").unwrap();
    let pool: Vec<NodeId> = tree
        .descendants(w)
        .into_iter()
        .filter(|&n| tree.with_node(n, |x| x.is_element()).unwrap_or(false))
        .collect();
    pool[rng.below(pool.len())]
}

#[test]
fn remap_matches_cold_under_rule_and_dom_churn() {
    for seed in 1..=24u64 {
        let tree = parse_html(&fixture());
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let mut rules: Vec<&str> = (0..10).map(|_| POOL[rng.below(POOL.len())]).collect();
        let mut key = 1u64;
        check(&tree, &rules, key, "prime");
        for step in 0..60 {
            let stage = format!("seed {seed} step {step}");
            // Rule churn: mostly insertRule at a chosen index (RNW keeps
            // precedence groups that way), sometimes delete or edit.
            match rng.below(6) {
                0..=2 => {
                    let at = rng.below(rules.len() + 1);
                    rules.insert(at, POOL[rng.below(POOL.len())]);
                }
                3 => {
                    if rules.len() > 3 {
                        rules.remove(rng.below(rules.len()));
                    }
                }
                4 => {
                    let at = rng.below(rules.len());
                    rules[at] = POOL[rng.below(POOL.len())];
                }
                _ => {}
            }
            // DOM churn alongside.
            match rng.below(5) {
                0 => {
                    let n = pick_element(&tree, &mut rng);
                    let class = ["a", "b", "c", "a c", "", "r-1abc"][rng.below(6)];
                    tree.set_attribute_noted(n, "class", class);
                }
                1 => {
                    let parent = pick_element(&tree, &mut rng);
                    let n = tree.new_node(element(&mut rng));
                    tree.append_child(parent, n);
                }
                2 => {
                    let n = pick_element(&tree, &mut rng);
                    tree.remove(n);
                }
                3 => {
                    let n = pick_element(&tree, &mut rng);
                    tree.set_attribute_noted(n, "data-k", "1");
                }
                _ => {}
            }
            key += 1;
            check(&tree, &rules, key, &stage);
        }
    }
}

/// With no detached debris, the remapped sets are exactly the cold sets —
/// and only the inserted selectors were probed.
#[test]
fn insert_rule_probes_only_new_selectors() {
    let tree = parse_html(&fixture());
    let mut rules: Vec<&str> = POOL[..20].to_vec();
    tree.rule_match_sets_incremental(&rules, 1);
    // Two insertRule calls at chosen indexes, one of them a duplicate of a
    // rule already present (its text has a twin: nothing to probe).
    rules.insert(3, ".r-2def.r-3ghi");
    rules.insert(0, ".a");
    LAST_REMAP_FRESH.with(|c| c.set(None));
    let inc = tree.rule_match_sets_incremental(&rules, 2);
    assert_eq!(LAST_REMAP_FRESH.with(|c| c.get()), Some(1));
    assert_eq!(*inc, tree.rule_match_sets(&rules));
    // A DOM write between two insertions: the carried sets still sync.
    let li = tree.query_selector_all("li").unwrap()[4];
    tree.set_attribute_noted(li, "class", "c");
    rules.insert(10, "em");
    let inc = tree.rule_match_sets_incremental(&rules, 3);
    assert_eq!(LAST_REMAP_FRESH.with(|c| c.get()), Some(1));
    assert_eq!(*inc, tree.rule_match_sets(&rules));
    // Unchanged list under a new key: plain reuse, no remap.
    LAST_REMAP_FRESH.with(|c| c.set(None));
    let inc = tree.rule_match_sets_incremental(&rules, 4);
    assert_eq!(LAST_REMAP_FRESH.with(|c| c.get()), None);
    assert_eq!(*inc, tree.rule_match_sets(&rules));
}

/// A quirks flip changes what every selector matches: the carried sets
/// must not survive it (set_quirks stamps a full re-probe).
#[test]
fn quirks_flip_never_carries_hits() {
    let tree = parse_html(&fixture());
    let mut rules = vec![".A", "#W", "li"];
    tree.rule_match_sets_incremental(&rules, 1);
    tree.set_quirks(false);
    rules.push("span");
    let inc = tree.rule_match_sets_incremental(&rules, 2);
    assert_eq!(*inc, tree.rule_match_sets(&rules));
    assert!(
        !inc.hits.contains_key(&0),
        "no-quirks `.A` must not match class a"
    );
}
