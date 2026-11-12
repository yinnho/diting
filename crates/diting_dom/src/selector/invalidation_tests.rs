//! DOM-mutation invalidation: pins and the x.com-shaped bench.

use super::match_index::{LAST_SYNC_AFFECTED, LAST_SYNC_SCOPED};
use crate::tree::{Attribute, DomTree, NodeData, NodeId};
use crate::tree_sink::parse_html;
use diting_html5ever::{ns, LocalName, Namespace, QualName};

pub(super) struct Rng(pub(super) u64);
impl Rng {
    pub(super) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub(super) fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// react-native-web's shape: thousands of single-class atomic rules
/// (`.r-1abc { … }`), a `css-` base class per element, a few hundred
/// combinator/state/attribute rules on top.
fn xcom_sheet(rng: &mut Rng) -> Vec<String> {
    let mut rules = Vec::new();
    for i in 0..3200 {
        rules.push(format!(".r-{i:x}"));
    }
    for i in 0..40 {
        rules.push(format!(".css-{i}"));
    }
    for i in 0..260 {
        let a = rng.below(3200);
        let b = rng.below(3200);
        rules.push(match i % 13 {
            0 => format!(".r-{a:x}:hover"),
            1 => format!(".r-{a:x}:focus-visible"),
            2 => format!("[dir=\"rtl\"] .r-{a:x}"),
            3 => format!(".css-{} > .r-{a:x}", i % 40),
            4 => format!(".r-{a:x} .r-{b:x}"),
            5 => format!("[data-testid=\"t{}\"]", i % 50),
            6 => format!(".r-{a:x}[aria-selected=\"true\"]"),
            7 => format!("input.r-{a:x}::placeholder"),
            8 => format!(".r-{a:x}:not(.r-{b:x})"),
            9 => format!(".r-{a:x} + .r-{b:x}"),
            10 => "body".to_string(),
            11 => format!("a.r-{a:x}:visited"),
            _ => format!(".css-{}:first-child", i % 40),
        });
    }
    rules
}

fn xcom_tree(rng: &mut Rng) -> DomTree {
    fn node(rng: &mut Rng, depth: usize, count: &mut usize, out: &mut String) {
        if *count >= 2500 {
            return;
        }
        *count += 1;
        let mut class = format!("css-{}", rng.below(40));
        for _ in 0..(4 + rng.below(10)) {
            class.push_str(&format!(" r-{:x}", rng.below(3200)));
        }
        let tag = ["div", "div", "div", "span", "a", "button"][rng.below(6)];
        let testid = if rng.below(20) == 0 {
            format!(r#" data-testid="t{}""#, rng.below(50))
        } else {
            String::new()
        };
        out.push_str(&format!(r#"<{tag} class="{class}"{testid}>"#));
        if depth < 12 {
            for _ in 0..(1 + rng.below(4)) {
                node(rng, depth + 1, count, out);
            }
        } else {
            out.push_str("text");
        }
        out.push_str(&format!("</{tag}>"));
    }
    let mut body = String::new();
    let mut count = 0;
    while count < 2500 {
        node(rng, 0, &mut count, &mut body);
    }
    parse_html(&format!(
        "<!DOCTYPE html><html dir=\"ltr\"><body>{body}</body></html>"
    ))
}

fn elements(tree: &DomTree) -> Vec<NodeId> {
    tree.descendants(tree.document())
        .into_iter()
        .filter(|&n| tree.with_node(n, |x| x.is_element()).unwrap_or(false))
        .collect()
}

pub(super) fn element(tag: &str, class: &str) -> NodeData {
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

/// One run's worth of x.com compose-page churn at a given write volume.
fn churn(tree: &DomTree, rng: &mut Rng, els: &[NodeId], writes: usize) {
    for _ in 0..writes {
        let n = els[rng.below(els.len())];
        let class = format!(
            "css-{} r-{:x} r-{:x}",
            rng.below(40),
            rng.below(3200),
            rng.below(3200)
        );
        set_attr(tree, n, "class", &class);
    }
    let n = els[rng.below(els.len())];
    match rng.below(3) {
        0 => set_attr(
            tree,
            n,
            "aria-selected",
            if rng.below(2) == 0 { "true" } else { "false" },
        ),
        1 => set_attr(tree, n, "style", &format!("width: {}px", rng.below(500))),
        _ => set_attr(tree, n, "data-testid", &format!("t{}", rng.below(50))),
    }
    if rng.below(3) == 0 {
        let parent = els[rng.below(els.len())];
        let d = tree.new_node(element("div", &format!("css-1 r-{:x}", rng.below(3200))));
        let t = tree.new_node(NodeData::Text {
            contents: "x".into(),
        });
        tree.append_child(d, t);
        tree.append_child(parent, d);
    }
}

/// Attribute write through the ops-path shape (old value read first).
pub(super) fn set_attr(tree: &DomTree, n: NodeId, name: &str, value: &str) {
    tree.set_attribute_noted(n, name, value);
}

/// x.com-shaped bench (not a gate):
/// `cargo test -p diting_dom --release --lib xcom_match_bench -- --ignored --nocapture`.
/// Two churn levels: light (3 writes/round) and x.com-realistic (30 — the
/// live compose page flips ~30 class/attr pairs per round), plus an idle leg
/// (no churn: the fixed per-sync hand-off cost). The #111 side-split zones
/// measured ~100µs idle / 170µs light / 386-410µs xcom on this rig with the
/// same seed, vs 358µs/786µs for the 88b1eacb first cut (whole-parent-zone
/// re-match, no side split) and 7.6ms cold.
#[test]
#[ignore]
fn xcom_match_bench() {
    let mut rng = Rng(0x7c0_3a11);
    let rules = xcom_sheet(&mut rng);
    let sels: Vec<&str> = rules.iter().map(String::as_str).collect();
    let tree = xcom_tree(&mut rng);
    let els = elements(&tree);
    tree.rule_match_sets_incremental(&sels, 1);
    let full = std::time::Instant::now();
    for _ in 0..5 {
        tree.rule_match_sets(&sels);
    }
    let full = full.elapsed() / 5;
    // No churn between syncs: the fixed per-sync overhead (cache hand-off,
    // hit-map clone-out), independent of any zone work.
    let mut idle = std::time::Duration::ZERO;
    for _ in 0..60 {
        let t = std::time::Instant::now();
        tree.rule_match_sets_incremental(&sels, 1);
        idle += t.elapsed();
    }
    eprintln!("[xcom-match-bench/idle] per-run={:?}", idle / 60);
    for &(tag, writes) in &[("light", 3usize), ("xcom", 30)] {
        let passes = 60u32;
        let mut total = std::time::Duration::ZERO;
        for _ in 0..passes {
            churn(&tree, &mut rng, &els, writes);
            let t = std::time::Instant::now();
            tree.rule_match_sets_incremental(&sels, 1);
            total += t.elapsed();
        }
        eprintln!(
            "[xcom-match-bench/{tag}] elements={} rules={} writes={} full={full:?} incremental per-run={:?}",
            els.len(),
            sels.len(),
            writes,
            total / passes
        );
    }
}

/// An attr-only sync must take the ZONED path (affected rules re-match
/// inside the write's parent subtree + ancestor chain, not the document),
/// and the zone must be complete: the differential fuzz pins the values,
/// this pins the path.
#[test]
fn attr_only_sync_rematches_inside_the_zone() {
    let mut pad = String::new();
    for _ in 0..200 {
        pad.push_str("<p>x</p>");
    }
    let tree = parse_html(&format!(
        r#"<!DOCTYPE html><html><body><div id="p"><section class="s1"><i class="a"></i><b class="b"></b></section></div>{pad}</body></html>"#
    ));
    let sels: Vec<&str> = vec![
        ".a",
        ".b",
        ".a + .b",
        "section:has(.b)",
        ".s1 .b",
        "#p .a",
        "p",
        ".q",
        ".r",
        ".pad-0",
        ".pad-1",
        ".pad-2",
        ".pad-3",
        ".pad-4",
        ".pad-5",
        ".pad-6",
        ".pad-7",
        ".pad-8",
    ];
    tree.rule_match_sets_incremental(&sels, 1);
    let i = tree.query_selector_all(".a").unwrap()[0];
    set_attr(&tree, i, "class", "b");
    let inc = tree.rule_match_sets_incremental(&sels, 1);
    assert_eq!(*inc, tree.rule_match_sets(&sels));
    assert!(
        LAST_SYNC_AFFECTED.with(|c| c.get()).is_some(),
        "stays incremental"
    );
    let zone = LAST_SYNC_SCOPED
        .with(|c| c.get())
        .expect("attr-only sync must be zoned");
    assert!(
        zone < 60,
        "zone covers the write's parent subtree + ancestors, not 200+ padding elements (got {zone})"
    );
    // Structural mutations zone the same way (siblings under the parent,
    // :empty on it).
    let em = tree.new_node(element("em", ""));
    tree.append_child(tree.query_selector_all("#p").unwrap()[0], em);
    let inc = tree.rule_match_sets_incremental(&sels, 1);
    assert_eq!(*inc, tree.rule_match_sets(&sels));
    assert!(
        LAST_SYNC_SCOPED.with(|c| c.get()).is_some(),
        "an insert zones at its parent like an attr write"
    );
    // State moves flip :hover compounds up and down the document's chains
    // — document-wide.
    tree.set_hovered_node(Some(tree.query_selector_all("p").unwrap()[0]));
    let inc = tree.rule_match_sets_incremental(&sels, 1);
    assert_eq!(*inc, tree.rule_match_sets(&sels));
    assert!(
        LAST_SYNC_SCOPED.with(|c| c.get()).is_none(),
        "state degrades to document-wide"
    );
}

/// Pre-fix hole: the old sibling-scope re-probe covered the parent's
/// children but not their subtrees, so inserting between `.a` and `.b`
/// left `.a + .b span` matching the span. The sheet pads with unrelated
/// rules so the affected-ratio gate keeps this sync on the incremental
/// path (2 structural rules of 3 would rebuild — legal, but this pin is
/// about pass 2).
#[test]
fn sibling_insert_reaches_descendants_of_later_siblings() {
    let mut pad = String::new();
    for _ in 0..60 {
        pad.push_str("<p>x</p>");
    }
    let tree = parse_html(&format!(
        r#"<!DOCTYPE html><html><body><div id="p"><i class="a"></i><b class="b"><span>s</span></b></div>{pad}</body></html>"#
    ));
    let sels: Vec<&str> = [".a + .b span", ".a ~ .b span", "li"]
        .into_iter()
        .chain([".pad-0", ".pad-1", ".pad-2", ".pad-3", ".pad-4"])
        .collect();
    tree.rule_match_sets_incremental(&sels, 1);
    let b = tree.query_selector_all(".b").unwrap()[0];
    let x = tree.new_node(element("em", ""));
    tree.insert_before(b, x);
    assert_eq!(
        *tree.rule_match_sets_incremental(&sels, 1),
        tree.rule_match_sets(&sels)
    );
    assert!(
        LAST_SYNC_AFFECTED.with(|c| c.get()).is_some(),
        "must stay incremental"
    );
}

/// Selector-heavy pool: every invalidation key gets readers — classes,
/// ids, attribute names (exists/value/prefix, nested in :not/:is/:has),
/// every combinator, :nth-*/:first/:last/:only/:empty/:root, relative
/// :has, :nth-child(of …), state, live control state, pseudo routing,
/// quirks case twins, a parse failure.
const POOL: &[&str] = &[
    ".a",
    ".b",
    ".c",
    "#x",
    "#y",
    "[data-k]",
    "[data-k='1']",
    "[data-k^='v']",
    "[aria-selected='true']",
    "[title]",
    "div",
    "span",
    ".a .b",
    ".a > .b",
    ".a + .b",
    ".a ~ .b",
    ".a + .b span",
    ".a ~ .b .c",
    "div:nth-child(2n+1)",
    "li:nth-of-type(2)",
    "li:last-child",
    ":first-child",
    ".c:only-child",
    "div:empty",
    "p:not(.a)",
    ":is(.a, #x) span",
    ":where(.b) > em",
    "section:has(.c)",
    "section:has(> .a) .b",
    "li:has(+ .b)",
    ":nth-child(2 of .a)",
    ".a:hover",
    ".b:hover span",
    ":focus",
    "div:focus-within",
    "input:checked",
    "input:checked + label",
    ":root .a",
    "html .b",
    "#x .a",
    "[dir=rtl] .a",
    ":dir(rtl)",
    ".A",
    "#X",
    ".a::before",
    ".b:after, .c",
    "em, strong",
    "*",
    "div >",
    ":not([data-k]) > span",
    "[style*='red'] em",
];

fn fuzz_fixture(quirks: bool) -> String {
    let mut body = String::new();
    for s in 0..6 {
        body.push_str(&format!(
            r#"<section class="{}">"#,
            ["a", "b", "", "c", "a b", ""][s]
        ));
        for i in 0..5 {
            let class = ["a", "b", "c", "a b", "", "A"][(s + i) % 6];
            body.push_str(&format!(
                r#"<div class="{class}" data-k="v{i}"><span>t</span><em>e</em><p class="a">p</p><i></i></div>"#
            ));
        }
        body.push_str(r#"<ul><li class="a">1</li><li class="b">2</li><li>3</li></ul>"#);
        body.push_str(r#"<input type="checkbox"><label>l</label><div></div></section>"#);
    }
    let doctype = if quirks { "" } else { "<!DOCTYPE html>" };
    format!(
        r#"{doctype}<html><body><div id="x">{body}</div><div id="y"><span>y</span></div></body></html>"#
    )
}

fn all_elements(tree: &DomTree) -> Vec<NodeId> {
    elements(tree)
        .into_iter()
        .filter(|&n| {
            !tree
                .with_node(n, |x| {
                    x.as_element()
                        .is_some_and(|e| matches!(&*e.local, "html" | "head" | "body"))
                })
                .unwrap_or(true)
        })
        .collect()
}

fn check_attached(tree: &DomTree, sels: &[&str], stage: &str) {
    let inc = tree.rule_match_sets_incremental(sels, 1);
    let full = tree.rule_match_sets(sels);
    assert_eq!(inc.specificity, full.specificity, "{stage}: specificity");
    let mut at = vec![false; tree.node_slot_count()];
    for d in tree.descendants(tree.document()) {
        at[d.index()] = true;
    }
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
                "{stage}: rule {ri} `{sel}` pseudo {slot}"
            );
        }
    }
}

/// One random mutation of every kind the stamps describe.
fn mutate(tree: &DomTree, rng: &mut Rng, parked: &mut Vec<NodeId>) -> &'static str {
    let els = all_elements(tree);
    // Remove churn can empty the fixture; nothing left to mutate.
    if els.is_empty() {
        return "gone";
    }
    let pick = |rng: &mut Rng| els[rng.below(els.len())];
    match rng.below(16) {
        0 | 1 => {
            let n = pick(rng);
            let v = ["a", "b", "c", "a b", "", "A", "b c a"][rng.below(7)];
            set_attr(tree, n, "class", v);
            "class"
        }
        2 => {
            let n = pick(rng);
            let name = ["data-k", "aria-selected", "title", "dir", "style"][rng.below(5)];
            let v = ["1", "v2", "true", "rtl", "color: red", ""][rng.below(6)];
            set_attr(tree, n, name, v);
            "attr"
        }
        3 => {
            let n = pick(rng);
            let v = ["x", "y", "X", "z"][rng.below(4)];
            set_attr(tree, n, "id", v);
            "id"
        }
        4 => {
            let n = pick(rng);
            let name = ["class", "data-k", "id", "title"][rng.below(4)];
            let old = tree
                .with_node(n, |x| x.get_attribute(name).map(str::to_string))
                .flatten();
            tree.with_node_mut(n, |x| {
                if let NodeData::Element { attrs, .. } = &mut x.data {
                    attrs.retain(|a| a.name.local.as_str() != name);
                }
            });
            tree.note_attr_change(n, name, Some(old.as_deref().unwrap_or("")), None);
            "remove-attr"
        }
        5 | 6 => {
            let tag = ["div", "span", "li", "em", "section", "input", "label", "p"][rng.below(8)];
            let class = ["a", "b", "c", "", "a b"][rng.below(5)];
            let n = tree.new_node(element(tag, class));
            if rng.below(2) == 0 {
                let t = tree.new_node(NodeData::Text {
                    contents: "n".into(),
                });
                tree.append_child(n, t);
            }
            let at = pick(rng);
            if rng.below(2) == 0 {
                tree.append_child(at, n);
            } else {
                tree.insert_before(at, n);
            }
            "insert"
        }
        7 => {
            tree.remove(pick(rng));
            "remove"
        }
        8 => {
            let n = pick(rng);
            tree.remove_child(n);
            parked.push(n);
            "detach"
        }
        9 => {
            if let Some(n) = parked.pop() {
                let at = pick(rng);
                if tree.ancestors(at).contains(&n) || at == n {
                    parked.push(n);
                } else {
                    tree.append_child(at, n);
                }
            }
            "reattach"
        }
        10 => {
            let (n, dest) = (pick(rng), pick(rng));
            if dest != n && !tree.ancestors(dest).contains(&n) {
                tree.detach(n);
                tree.append_child(dest, n);
            }
            "move"
        }
        11 => {
            // Text edits, to and from empty (:empty on the parent).
            let n = pick(rng);
            let kids = tree.children(n);
            let v = ["", "t", ""][rng.below(3)];
            for k in kids {
                tree.with_node_mut(k, |x| {
                    if let NodeData::Text { contents } = &mut x.data {
                        *contents = v.to_string();
                    }
                });
                tree.note_text_change(k);
            }
            "text"
        }
        12 => {
            let n = if rng.below(4) == 0 {
                None
            } else {
                Some(pick(rng))
            };
            tree.set_hovered_node(n);
            "hover"
        }
        13 => {
            let n = if rng.below(4) == 0 {
                None
            } else {
                Some(pick(rng))
            };
            tree.set_focused_node(n);
            "focus"
        }
        14 => {
            let inputs = tree.query_selector_all("input").unwrap_or_default();
            if !inputs.is_empty() {
                let n = inputs[rng.below(inputs.len())];
                let on = rng.below(2) == 0;
                tree.with_node_mut(n, |x| x.set_live_checked(on));
                tree.note_live_change();
            }
            "live"
        }
        _ => {
            // Same-value rewrite: nothing a selector can see changed.
            let n = pick(rng);
            let cur = tree
                .with_node(n, |x| x.get_attribute("class").map(str::to_string))
                .flatten()
                .unwrap_or_default();
            set_attr(tree, n, "class", &cur);
            "noop"
        }
    }
}

#[test]
fn scoped_invalidation_matches_cold_under_random_mutations() {
    let mut incremental = 0;
    let mut steps = 0;
    for seed in 1..=32u64 {
        let tree = parse_html(&fuzz_fixture(seed % 2 == 0));
        let mut rng = Rng(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) | 1);
        let sels: Vec<&str> = (0..30).map(|_| POOL[rng.below(POOL.len())]).collect();
        check_attached(&tree, &sels, "prime");
        let mut parked = Vec::new();
        for step in 0..80 {
            let kind = mutate(&tree, &mut rng, &mut parked);
            // Sometimes two mutations land in one sync.
            let kind2 = if rng.below(3) == 0 {
                mutate(&tree, &mut rng, &mut parked)
            } else {
                ""
            };
            check_attached(
                &tree,
                &sels,
                &format!("seed {seed} step {step} {kind} {kind2}"),
            );
            steps += 1;
            if LAST_SYNC_AFFECTED.with(|c| c.get()).is_some() {
                incremental += 1;
            }
        }
    }
    // The point: most syncs stay on the scoped path.
    assert!(
        incremental * 10 > steps * 7,
        "only {incremental}/{steps} syncs stayed incremental"
    );
}
