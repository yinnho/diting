//! Differential pins for the cascade memo: after every DOM mutation the
//! memoized document pass (css_key run, memo persisted on the tree) must
//! return exactly the map a cold full recompute returns (css_key None: no
//! memo, full selector re-match).

use crate::style_memo::last_stats;
use crate::{compute_styles_gated, compute_styles_timed};
use diting_css::{parse_stylesheet_full, CssMediaType, CssTransition, KeyframesMap, ParsedRule};
use diting_dom::tree::{Attribute, DomTree, NodeData, NodeId};
use diting_dom::tree_sink::parse_html;
use diting_html5ever::ns;
use std::collections::HashMap;

const VP: (f32, f32) = (1024.0, 768.0);

const SHEET: &str = r#"
@property --gap { syntax: "<length>"; inherits: true; initial-value: 2px; }
@layer base, theme;
@layer base { div { margin: 1px; color: rgb(10, 10, 10); } }
@layer theme { .hot { color: rgb(200, 0, 0) !important; } div { margin: 2px; } }
:root { --accent: rgb(1, 2, 3); font-size: 15px; }
body { line-height: 1.4; font-family: serif; }
section { font-size: 1.1em; padding: var(--gap); }
section > div { border-left: 1px solid var(--accent); }
.big { font-size: 1.5em; }
.small { font-size: 0.8rem; }
.hot { color: rgb(255, 0, 0); }
.cool { color: rgb(0, 0, 255); --accent: rgb(9, 9, 9); }
.flex { display: flex; gap: 4px; }
.hide { display: none; }
.wide { width: 50%; }
.em { width: 10em; margin-left: 2em; }
div div span { text-decoration: underline; }
p + p { margin-top: 3px; }
li:nth-child(2n) { color: rgb(0, 128, 0); }
[data-x="1"] { opacity: 0.5; }
[lang|="ja"] { font-family: monospace; }
article:has(.hot) { background-color: rgb(250, 250, 0); }
ol { counter-reset: item; }
li { counter-increment: item; }
li::before { content: counter(item) ". " attr(data-label); }
.q::after { content: close-quote; }
.q::before { content: open-quote; }
.spin { animation: spin 2s linear infinite; }
.fade { animation: fade 1s 0.5s both; }
@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
@keyframes fade { from { opacity: 0; } to { opacity: 1; } }
.zoomed { zoom: 2; }
td { padding: 2px; }
.rtl { direction: rtl; }
"#;

fn fixture() -> String {
    let mut body = String::new();
    for s in 0..6 {
        body.push_str(&format!(
            r#"<section id="s{s}" class="{}">"#,
            ["", "big", "cool", "flex", "small", "zoomed"][s]
        ));
        for a in 0..4 {
            body.push_str(&format!(r#"<article id="a{s}_{a}"><div class="em">"#));
            for d in 0..3 {
                body.push_str(&format!(
                    r#"<div id="d{s}_{a}_{d}" class="{}" data-x="{}"><span>t{d}</span><p>p</p><p class="q">q</p></div>"#,
                    ["hot", "wide", ""][d],
                    d % 2
                ));
            }
            body.push_str("</div></article>");
        }
        body.push_str(&format!(r#"<ol id="ol{s}">"#));
        for i in 0..5 {
            body.push_str(&format!(
                r#"<li data-label="L{i}" class="{}">item</li>"#,
                ["", "spin", "fade", "", "hot"][i]
            ));
        }
        body.push_str("</ol></section>");
    }
    body.push_str(
        r#"<table><tr><td id="td1" width="80" valign="top" nowrap>x</td><td height="30">y</td></tr></table>
<p id="auto" dir="auto">hello</p><p dir="rtl" lang="ja-JP">r</p><a href="/x">link</a><a>anchor</a>
<div id="host"><template shadowrootmode="open"><div class="hot"><slot name="s"></slot></div><slot></slot></template><span slot="s" id="slotted">s</span><em>dflt</em></div>"#,
    );
    format!("<html><body>{body}</body></html>")
}

struct Rig {
    tree: DomTree,
    rules: Vec<ParsedRule>,
    keyframes: KeyframesMap,
    vp: (f32, f32),
    css_time: Option<f64>,
    /// The css_key contract: a new rule table gets a new key.
    css_key: u64,
    transitions: Vec<CssTransition>,
}

impl Rig {
    fn new() -> Rig {
        let tree = parse_html(&fixture());
        let (rules, keyframes, _) =
            parse_stylesheet_full(SHEET, VP, CssMediaType::Screen, &Default::default());
        Rig {
            tree,
            rules,
            keyframes,
            vp: VP,
            css_time: None,
            transitions: Vec::new(),
            css_key: 7,
        }
    }

    fn memoized(&self) -> HashMap<NodeId, diting_css::ComputedStyle> {
        compute_styles_timed(
            &self.tree,
            &self.rules,
            &self.keyframes,
            self.css_time,
            &self.transitions,
            self.vp,
            Some(self.css_key),
        )
    }

    fn cold(&self) -> HashMap<NodeId, diting_css::ComputedStyle> {
        compute_styles_timed(
            &self.tree,
            &self.rules,
            &self.keyframes,
            self.css_time,
            &self.transitions,
            self.vp,
            None,
        )
    }

    /// Memoized pass == cold pass, element by element. Returns the memo
    /// stats of the memoized run.
    fn check(&self, step: &str) -> (usize, usize) {
        let warm = self.memoized();
        let stats = last_stats(&self.tree);
        let cold = self.cold();
        assert_same(&self.tree, &warm, &cold, step);
        stats
    }

    fn elements(&self) -> Vec<NodeId> {
        let doc = self.tree.document();
        let mut out = Vec::new();
        let mut stack = vec![doc];
        while let Some(n) = stack.pop() {
            if self.tree.with_node(n, |x| x.is_element()).unwrap_or(false) {
                out.push(n);
            }
            if let Some(root) = self.tree.shadow_root(n) {
                stack.push(root);
            }
            stack.extend(self.tree.children(n));
        }
        out
    }

    fn set_attr(&self, n: NodeId, name: &str, value: &str) {
        self.tree.set_attribute_noted(n, name, value);
    }
}

fn assert_same(
    tree: &DomTree,
    warm: &HashMap<NodeId, diting_css::ComputedStyle>,
    cold: &HashMap<NodeId, diting_css::ComputedStyle>,
    step: &str,
) {
    let mut wk: Vec<_> = warm.keys().copied().collect();
    let mut ck: Vec<_> = cold.keys().copied().collect();
    wk.sort_by_key(|n| n.index());
    ck.sort_by_key(|n| n.index());
    assert_eq!(wk, ck, "{step}: styled element sets differ");
    for n in ck {
        let (w, c) = (&warm[&n], &cold[&n]);
        if w != c {
            let tag = tree
                .with_node(n, |x| x.as_element().map(|e| e.local.to_string()))
                .flatten();
            panic!("{step}: {n} <{tag:?}> differs\nmemo: {w:?}\ncold: {c:?}");
        }
    }
}

fn element(tag: &str, attrs: &[(&str, &str)]) -> NodeData {
    let q = |l: &str, n| diting_html5ever::QualName {
        prefix: None,
        ns: n,
        local: diting_html5ever::LocalName::from(l),
    };
    NodeData::Element {
        name: q(tag, ns!(html)),
        attrs: attrs
            .iter()
            .map(|(k, v)| Attribute {
                name: q(k, ns!()),
                value: v.to_string(),
            })
            .collect(),
        live_value: None,
        live_checked: Some(false),
        live_indeterminate: false,
        mathml_annotation_xml_integration_point: false,
        template_contents: None,
    }
}

/// Deterministic xorshift — the fuzz below must replay identically.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[(self.next() % xs.len() as u64) as usize]
    }
}

#[test]
fn memo_matches_cold_cascade_under_random_mutations() {
    let rig = Rig::new();
    let mut rig = rig;
    let (_, first_misses) = rig.check("initial");
    assert!(first_misses > 200, "fixture too small: {first_misses}");
    // Unchanged tree: every element hits.
    let (hits, misses) = rig.check("rerun");
    // Only the bypassed elements recompute: the two slot-assigned light
    // children (they inherit from the slot's finished style) and the
    // dir=auto paragraph (it reads its text).
    assert_eq!(misses, 3, "an untouched tree must not recompute");
    assert!(hits > 200);

    let classes = [
        "",
        "hot",
        "cool",
        "big",
        "small",
        "flex",
        "hide",
        "spin fade",
        "zoomed",
        "rtl em",
    ];
    let styles = [
        "",
        "color: rgb(1, 1, 1)",
        "font-size: 2em",
        "--accent: rgb(7, 7, 7)",
        "--gap: 9px",
        "zoom: 3",
        "animation: fade 1s",
        "line-height: 20px !important",
    ];
    let tags = ["div", "span", "p", "li", "section", "a", "td"];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut localized = 0;
    for step in 0..400 {
        let els = rig.elements();
        let pickable: Vec<NodeId> = els
            .iter()
            .copied()
            .filter(|&n| {
                let t = rig
                    .tree
                    .with_node(n, |x| x.as_element().map(|e| e.local.to_string()))
                    .flatten()
                    .unwrap_or_default();
                !matches!(t.as_str(), "html" | "head" | "body")
            })
            .collect();
        let target = rng.pick(&pickable);
        let op = rng.next() % 11;
        let label = format!("step {step} op {op} on {target}");
        match op {
            0 => rig.set_attr(target, "class", rng.pick(&classes)),
            1 => rig.set_attr(target, "style", rng.pick(&styles)),
            2 => {
                let tag = rng.pick(&tags);
                let class = rng.pick(&classes);
                let n = rig.tree.new_node(element(tag, &[("class", class)]));
                let t = rig.tree.new_node(NodeData::Text {
                    contents: "new".into(),
                });
                rig.tree.append_child(n, t);
                if rng.next().is_multiple_of(2) {
                    rig.tree.append_child(target, n);
                } else {
                    rig.tree.insert_before(target, n);
                }
            }
            3 => {
                if pickable.len() > 150 {
                    rig.tree.remove(target);
                }
            }
            4 => {
                // Move under another element that is not inside target.
                let dest = rng.pick(&pickable);
                let inside = dest == target || rig.tree.ancestors(dest).contains(&target);
                if !inside && !rig.tree.is_shadow_root(dest) {
                    rig.tree.detach(target);
                    rig.tree.append_child(dest, target);
                }
            }
            5 => {
                // Text edit — dir=auto reads it.
                let auto = rig.tree.get_element_by_id("auto").unwrap_or(target);
                let words = ["hello", "שלום", "مرحبا", "123", "abc"];
                for c in rig.tree.children(auto) {
                    rig.tree.with_node_mut(c, |x| {
                        if let NodeData::Text { contents } = &mut x.data {
                            *contents = rng.pick(&words).to_string();
                        }
                    });
                }
            }
            6 => rig.set_attr(target, "lang", rng.pick(&["ja", "en", "zh-CN", ""])),
            7 => rig.set_attr(target, "dir", rng.pick(&["rtl", "ltr", "auto", ""])),
            8 => {
                // Rewrite an attribute with its own value (no-op write).
                let cur = rig
                    .tree
                    .with_node(target, |x| x.get_attribute("class").map(str::to_string))
                    .flatten()
                    .unwrap_or_default();
                rig.set_attr(target, "class", &cur);
            }
            9 => {
                rig.css_time = rng.pick(&[None, Some(0.25), Some(0.8), Some(1.7), Some(5.0)]);
            }
            _ => rig.set_attr(
                target,
                rng.pick(&["data-x", "data-label", "width", "href"]),
                rng.pick(&["1", "0", "80", "z"]),
            ),
        }
        let (hits, misses) = rig.check(&label);
        if misses * 4 < hits {
            localized += 1;
        }
    }
    // The point of the memo: most single mutations recompute a small slice.
    assert!(
        localized > 200,
        "only {localized}/400 steps stayed localized"
    );
}

#[test]
fn memo_tracks_whole_run_inputs() {
    let mut rig = Rig::new();
    rig.check("initial");

    // Viewport (vw/% fold) — every entry must drop.
    rig.vp = (800.0, 600.0);
    let (hits, _) = rig.check("viewport");
    assert_eq!(hits, 0);

    // A grown rule table (a new rule appended): the memo remaps onto the
    // surviving rules — spans gain a match and recompute, the rest stay.
    let (rules, kf, _) = parse_stylesheet_full(
        &format!("{SHEET} span {{ color: rgb(3, 3, 3); }}"),
        rig.vp,
        CssMediaType::Screen,
        &Default::default(),
    );
    rig.rules = rules;
    rig.css_key += 1;
    rig.keyframes = kf;
    let (hits, misses) = rig.check("rules");
    assert!(hits > 0 && misses > 3, "remap: hits={hits} misses={misses}");

    // Same rule shape, different declarations: every matched index list
    // is unchanged, only `.big`'s text moved — its elements (and their
    // inheriting subtrees) must recompute even though their indexes match.
    let (rules, kf, _) = parse_stylesheet_full(
        &format!("{SHEET} span {{ color: rgb(3, 3, 3); }}")
            .replace("font-size: 1.5em", "font-size: 3em"),
        rig.vp,
        CssMediaType::Screen,
        &Default::default(),
    );
    rig.rules = rules;
    rig.keyframes = kf;
    rig.css_key += 1;
    let (hits, misses) = rig.check("rule text");
    assert!(hits > 0 && misses > 3, "remap: hits={hits} misses={misses}");

    // Edit the FIRST rule in place: its old index has no twin, and an
    // unmapped index must never alias a live one (index 0 included).
    assert!(rig.rules[0].declarations.contains("rgb(10, 10, 10)"));
    let (rules, kf, _) = parse_stylesheet_full(
        &format!("{SHEET} span {{ color: rgb(3, 3, 3); }}")
            .replace("font-size: 1.5em", "font-size: 3em")
            .replace("color: rgb(10, 10, 10)", "color: rgb(11, 10, 10)"),
        rig.vp,
        CssMediaType::Screen,
        &Default::default(),
    );
    rig.rules = rules;
    rig.keyframes = kf;
    rig.css_key += 1;
    rig.check("first rule edited");

    // Layer ranks are per-thread parse state: another document's parse
    // (`a.b` post-orders before `a`) flips ids 1/2 under identical rules.
    // The probe re-declares SHEET's @property so the registry key stays
    // equal and only the ranks move.
    let probe = r#"@property --gap { syntax: "<length>"; inherits: true; initial-value: 2px; } @layer a.b, c;"#;
    let _ = parse_stylesheet_full(probe, VP, CssMediaType::Screen, &Default::default());
    let (_, misses) = rig.check("layer ranks");
    assert!(misses > 3, "re-ranked layered rules must recompute");

    // rem against a new root size while body pins its own px size: body's
    // face is unchanged, so only the root_fs key catches its rem children.
    let body = rig.tree.children(rig.elements()[0]).into_iter().find(|&n| {
        rig.tree
            .with_node(n, |x| x.as_element().is_some_and(|e| &*e.local == "body"))
            .unwrap_or(false)
    });
    let html = rig.elements()[0];
    rig.set_attr(body.unwrap(), "style", "font-size: 16px");
    rig.check("body px");
    rig.set_attr(html, "style", "font-size: 30px");
    rig.check("root font-size");

    // JS-registered property: inheritance semantics change under the same
    // rules.
    diting_css::props::register_js("--accent", "<color>", false, Some("rgb(0, 0, 0)"));
    let (hits, _) = rig.check("registerProperty");
    diting_css::props::clear_js();
    assert_eq!(hits, 0);
    rig.check("registry cleared");

    // Animations: the raw cascade is memoized, the sample is not.
    for t in [Some(0.1), Some(0.6), Some(1.2), None, Some(3.3)] {
        rig.css_time = t;
        let (_, misses) = rig.check("css_time");
        assert_eq!(misses, 3, "time alone must not recompute the cascade");
    }

    // Transitions are an exit-phase overlay on the output.
    let li = rig
        .elements()
        .into_iter()
        .find(|&n| {
            rig.tree
                .with_node(n, |x| x.get_attribute("data-label") == Some("L1"))
                .unwrap_or(false)
        })
        .unwrap();
    rig.transitions.push(CssTransition {
        nid: li.index(),
        property: "opacity".into(),
        from: diting_css::TransitionValue::Opacity(0.0),
        to: diting_css::TransitionValue::Opacity(1.0),
        start: 0.0,
        duration: 2.0,
        delay: 0.0,
        easing: diting_css::Easing::Linear,
    });
    rig.css_time = Some(0.5);
    rig.check("transition");
}

#[test]
fn counters_and_pseudos_follow_structure() {
    let rig = Rig::new();
    rig.check("initial");
    let ol = rig.tree.get_element_by_id("ol0").unwrap();
    let items = rig.tree.children(ol);
    // Drop the first li: every later ::before renumbers, while the li
    // hosts themselves keep their memoized cascade.
    rig.tree.remove(items[0]);
    let (hits, misses) = rig.check("remove li");
    assert!(hits > misses);
    let out = rig.memoized();
    let before = out[&items[1]]
        .pseudos
        .as_ref()
        .and_then(|p| p.before.as_ref())
        .and_then(|b| b.content_resolved.clone());
    assert_eq!(before.as_deref(), Some("1. L1"));
    // counter-reset moved off the ol: the next walk numbers globally.
    rig.set_attr(ol, "style", "counter-reset: none");
    rig.check("reset gone");
    // A quote opened before the children closes after them.
    let d = rig.tree.get_element_by_id("d0_0_0").unwrap();
    rig.set_attr(d, "class", "q");
    rig.check("quotes");
}

#[test]
fn gated_container_face_matches_cold() {
    let rig = Rig::new();
    let base = rig.rules.len();
    let mut rules = rig.rules.clone();
    rules.push(ParsedRule {
        selector: "div".into(),
        declarations: "color: rgb(77, 77, 77); padding: 1em".into(),
        layer: 0,
    });
    let divs: Vec<usize> = rig
        .elements()
        .into_iter()
        .filter(|&n| {
            rig.tree
                .with_node(n, |x| x.as_element().is_some_and(|e| &*e.local == "div"))
                .unwrap_or(false)
        })
        .map(|n| n.index())
        .collect();
    let gated = |gates: &HashMap<usize, Vec<usize>>| {
        compute_styles_gated(
            &rig.tree,
            &rules,
            &rig.keyframes,
            None,
            &[],
            base,
            gates,
            VP,
        )
    };
    let mut gates = HashMap::new();
    gates.insert(base, divs.clone());
    gated(&gates);
    for (i, cut) in [0usize, 10, 3, 3, 25, 25].into_iter().enumerate() {
        let g: Vec<usize> = divs.iter().copied().skip(cut).step_by(2).collect();
        gates.insert(base, g);
        if i % 2 == 1 {
            let d = rig.tree.get_element_by_id("d1_1_1").unwrap();
            rig.set_attr(d, "class", ["hot", "cool", "big"][i / 2]);
        }
        let warm = gated(&gates);
        let (hits, _) = last_stats(&rig.tree);
        if i > 0 {
            assert!(hits > 0, "gated step {i} never hit");
        }
        // Cold reference on the same tree: park the memo, recompute from
        // nothing, put the memo back.
        let parked = rig.tree.take_style_memo();
        let cold = gated(&gates);
        if let Some(m) = parked {
            rig.tree.set_style_memo(m);
        }
        assert_same(&rig.tree, &warm, &cold, &format!("gated step {i}"));
    }
    // Plain and gated faces keep separate slots: alternating does not thrash.
    rig.memoized();
    compute_styles_gated(
        &rig.tree,
        &rules,
        &rig.keyframes,
        None,
        &[],
        base,
        &gates,
        VP,
    );
    rig.memoized();
    let (_, misses) = last_stats(&rig.tree);
    assert_eq!(misses, 3);
}

/// Synthetic large-page bench (not a gate): ~2000 elements, a ~3000-rule
/// sheet, one class flip per pass — memo on vs DITING_STYLE_MEMO=0 on the
/// same css-keyed path. Run with
/// `cargo test -p diting_layout --release --lib style_memo_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn style_memo_bench() {
    let mut css =
        String::from(":root { font-size: 16px; --c: rgb(1, 2, 3); } body { line-height: 1.5; }\n");
    for i in 0..3000 {
        css.push_str(&format!(
            ".c{} {{ color: rgb({}, 0, 0); margin: {}px; padding: 0.{}em; border-left: 1px solid var(--c); }}\n.w{} > .c{} span {{ font-size: 1.{}em; }}\n",
            i % 400,
            i % 255,
            i % 7,
            i % 9,
            i % 50,
            i % 400,
            i % 5
        ));
    }
    let mut body = String::new();
    for w in 0..100 {
        body.push_str(&format!(r#"<div class="w{w}" id="w{w}">"#));
        for i in 0..6 {
            body.push_str(&format!(
                r#"<div class="c{} x"><span>a</span><p class="c{}">b<em>e</em></p></div>"#,
                (w * 6 + i) % 400,
                (w + i) % 400
            ));
        }
        body.push_str("</div>");
    }
    let tree = parse_html(&format!("<html><body>{body}</body></html>"));
    let (rules, kf, _) = parse_stylesheet_full(&css, VP, CssMediaType::Screen, &Default::default());
    let run = || compute_styles_timed(&tree, &rules, &kf, None, &[], VP, Some(1));
    let n = run().len();
    let passes = 60;
    let mut timings = Vec::new();
    for memo_on in [false, true] {
        if memo_on {
            std::env::remove_var("DITING_STYLE_MEMO");
        } else {
            std::env::set_var("DITING_STYLE_MEMO", "0");
        }
        run();
        let t = std::time::Instant::now();
        // Two writes per pass, fresh values in each phase: a wrapper's class
        // (re-targets its `.wN > .cX span` descendants) and one child's
        // class (new color/padding inherited by its subtree).
        let phase = memo_on as usize;
        for p in 0..passes {
            let w = tree.get_element_by_id(&format!("w{}", p % 100)).unwrap();
            let class = format!("w{}", (p * 7 + phase * 13) % 50);
            tree.set_attribute_noted(w, "class", &class);
            let child = tree.children(w)[p % 6];
            let class = format!("c{} x", (p * 11 + phase * 3) % 400);
            tree.set_attribute_noted(child, "class", &class);
            run();
        }
        timings.push(t.elapsed() / passes as u32);
    }
    std::env::remove_var("DITING_STYLE_MEMO");
    eprintln!(
        "[style-memo-bench] elements={n} rules={} per-pass: full={:?} memo={:?} ({:.1}x)",
        rules.len(),
        timings[0],
        timings[1],
        timings[0].as_secs_f64() / timings[1].as_secs_f64()
    );
}

/// insertRule churn (react-native-web inserts at chosen indexes to keep its
/// precedence groups): the memo remaps onto equal rules at their new
/// indexes. Rule insertions anywhere — including new @layer blocks that
/// shift existing layers' ranks — plus edits, removals and DOM writes; the
/// memoized pass must equal the cold pass after every step.
#[test]
fn memo_survives_rule_insertions() {
    let pool = [
        ".hot { color: rgb(9, 0, 0); }",
        ".r-a { padding: 3px; }",
        ".r-b { font-size: 1.2em; }",
        "div { line-height: 18px; }",
        "span { color: rgb(0, 9, 0); }",
        ".big { font-size: 2em; }",
        "li::before { content: \"*\"; }",
        "@layer early { div { margin: 7px; } }",
        "@layer late { div { margin: 8px; } .hot { color: rgb(1, 1, 9); } }",
        "@layer early.inner { span { padding: 1px; } }",
        "section > div { border-left: 2px solid red; }",
        ".cool span { text-decoration: underline; }",
        "p + p { margin-top: 9px; }",
        ".hot { color: rgb(9, 0, 0); }",
    ];
    let mut rig = Rig::new();
    let mut sheet: Vec<String> = SHEET.split_inclusive("}\n").map(str::to_string).collect();
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let reparse = |rig: &mut Rig, sheet: &[String]| {
        let (rules, kf, _) = parse_stylesheet_full(
            &sheet.concat(),
            VP,
            CssMediaType::Screen,
            &Default::default(),
        );
        rig.rules = rules;
        rig.keyframes = kf;
        rig.css_key += 1;
    };
    reparse(&mut rig, &sheet);
    rig.check("initial");
    let mut kept = 0;
    for step in 0..120 {
        match rng.next() % 8 {
            0..=4 => {
                let at = (rng.next() % (sheet.len() as u64 + 1)) as usize;
                sheet.insert(at, format!("{}\n", rng.pick(&pool)));
            }
            5 => {
                let at = (rng.next() % sheet.len() as u64) as usize;
                sheet.remove(at);
            }
            6 => {
                let at = (rng.next() % sheet.len() as u64) as usize;
                sheet[at] = format!("{}\n", rng.pick(&pool));
            }
            _ => {}
        }
        if rng.next().is_multiple_of(3) {
            let els = rig.elements();
            let n = els[(rng.next() % els.len() as u64) as usize];
            rig.set_attr(n, "class", rng.pick(&["hot", "r-a r-b", "big", "cool", ""]));
        }
        reparse(&mut rig, &sheet);
        let (hits, misses) = rig.check(&format!("insert step {step}"));
        if hits > misses * 2 {
            kept += 1;
        }
    }
    // Broad pool rules (`div {}`, new @layer blocks re-ranking every
    // layered rule) legitimately invalidate most elements; the narrow ones
    // must not. Seeded: 36/120 at the time of writing.
    assert!(
        kept > 25,
        "memo kept its hits in only {kept}/120 rule edits"
    );
}

/// The x.com shape in miniature: one insertRule of a fresh atomic class
/// that nothing matches yet keeps every element on the memo.
#[test]
fn unrelated_insert_rule_keeps_every_entry() {
    let mut rig = Rig::new();
    rig.check("initial");
    let (rules, kf, _) = parse_stylesheet_full(
        &SHEET.replacen(".big {", ".r-zz9 { width: 4px; }\n.big {", 1),
        VP,
        CssMediaType::Screen,
        &Default::default(),
    );
    assert_eq!(rules.len(), rig.rules.len() + 1);
    rig.rules = rules;
    rig.keyframes = kf;
    rig.css_key += 1;
    let (_, misses) = rig.check("insertRule");
    assert_eq!(misses, 3, "only the always-bypassed elements recompute");
}

/// insertRule bench (not a gate): the synthetic page above, but every pass
/// inserts fresh atomic rules at chosen indexes (react-native-web's shape)
/// plus one class write. Reports compute_styles per pass for 1 and 5
/// inserts. Same invocation as `style_memo_bench`.
#[test]
#[ignore]
fn insert_rule_bench() {
    let mut sheet: Vec<String> =
        vec![":root { font-size: 16px; } body { line-height: 1.5; }\n".into()];
    for i in 0..3000 {
        sheet.push(format!(
            ".c{} {{ color: rgb({}, 0, 0); margin: {}px; padding: 0.{}em; }}\n",
            i % 400,
            i % 255,
            i % 7,
            i % 9
        ));
        sheet.push(format!(
            ".w{} > .c{} span {{ font-size: 1.{}em; }}\n",
            i % 50,
            i % 400,
            i % 5
        ));
    }
    let mut body = String::new();
    for w in 0..100 {
        body.push_str(&format!(r#"<div class="w{w}" id="w{w}">"#));
        for i in 0..6 {
            body.push_str(&format!(
                r#"<div class="c{} x"><span>a</span><p class="c{}">b<em>e</em></p></div>"#,
                (w * 6 + i) % 400,
                (w + i) % 400
            ));
        }
        body.push_str("</div>");
    }
    let tree = parse_html(&format!("<html><body>{body}</body></html>"));
    let mut rng = Rng(0xbe9c_0001);
    let mut key = 1u64;
    let (rules, kf, _) = parse_stylesheet_full(
        &sheet.concat(),
        VP,
        CssMediaType::Screen,
        &Default::default(),
    );
    compute_styles_timed(&tree, &rules, &kf, None, &[], VP, Some(key));
    for inserts in [1usize, 5] {
        let passes = 40u32;
        let mut total = std::time::Duration::ZERO;
        for p in 0..passes {
            for _ in 0..inserts {
                let at = (rng.next() % sheet.len() as u64) as usize;
                let n = rng.next() % 1_000_000;
                sheet.insert(at, format!(".r-{n:x} {{ width: {}px; }}\n", n % 300));
            }
            let w = tree.get_element_by_id(&format!("w{}", p % 100)).unwrap();
            let class = format!("w{}", (p * 7 + inserts as u32) % 50);
            tree.set_attribute_noted(w, "class", &class);
            let (rules, kf, _) = parse_stylesheet_full(
                &sheet.concat(),
                VP,
                CssMediaType::Screen,
                &Default::default(),
            );
            key += 1;
            let t = std::time::Instant::now();
            compute_styles_timed(&tree, &rules, &kf, None, &[], VP, Some(key));
            total += t.elapsed();
        }
        eprintln!(
            "[insert-rule-bench] inserts/pass={inserts} rules={} per-pass compute_styles={:?}",
            sheet.len(),
            total / passes
        );
    }
}
