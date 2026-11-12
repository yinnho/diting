//! innerText/line-clamp/文本变换排宽/字体面（fonts 家族并此）
use super::super::*;
use super::common::setup_runtime;

/// #208: FontFaceSet.forEach shadowed Array.prototype.forEach with a
/// function that called itself — every call was a RangeError. The
/// replacement iterates by index and keeps Set callback semantics
/// (value, value, set) with thisArg pass-through. (Chrome's set is EMPTY
/// until a page adds faces - M4 knife 52 - so the test adds one first.)
#[test]
fn fonts_for_each_iterates_without_recursing() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = "(function() { \
            var seen = 0, self2nd = null, set3rd = null, thisOk = false; \
            var ctx = { tag: 'ctx' }; \
            document.fonts.add(new FontFace('T', 'local(Arial)')); \
            document.fonts.forEach(function (v, v2, s) { \
                seen++; self2nd = (v === v2); set3rd = s; \
                if (this === ctx) thisOk = true; \
            }, ctx); \
            return JSON.stringify({ seen: seen, selfSecond: self2nd, \
                setIsFontFaceSet: set3rd === document.fonts, thisOk: thisOk, \
                identity: document.fonts === document.fonts, \
                repeat: (function(){ var n = 0; document.fonts.forEach(function(){ n++; }); return n; })() }); \
        })()";
    let out = rt.evaluate(probe).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert!(
        v["seen"].as_u64().unwrap() > 0,
        "forEach must visit the faces"
    );
    assert_eq!(
        v["selfSecond"],
        serde_json::json!(true),
        "Set semantics: 2nd arg is the value"
    );
    assert_eq!(
        v["setIsFontFaceSet"],
        serde_json::json!(true),
        "3rd arg is the set"
    );
    assert_eq!(
        v["identity"],
        serde_json::json!(true),
        "document.fonts must be the same object across accesses"
    );
    assert_eq!(v["thisOk"], serde_json::json!(true), "thisArg pass-through");
    assert_eq!(
        v["repeat"].as_u64().unwrap(),
        v["seen"].as_u64().unwrap(),
        "repeat calls must stay stable (no state corruption)"
    );
}

/// obscura#828 lineage: innerText is rendered text, not textContent —
/// script/style/template/noscript bodies, display:none subtrees and
/// visibility:hidden text contribute nothing; block boxes break lines;
/// collapsible whitespace collapses. The old getter was a textContent
/// passthrough, interleaving script source with visible text (~100x
/// bloat on script-heavy pages).
#[test]
fn inner_text_excludes_script_and_style_bodies() {
    let mut rt = setup_runtime(
        "<html><body><p>visible</p><script>var secret = 'leakme';</script>\
             <style>.x { color: red }</style><noscript>nojs</noscript></body></html>",
    );
    let t = rt.evaluate("document.body.innerText").unwrap();
    assert_eq!(t, serde_json::json!("visible"), "got: {t}");
}

#[test]
fn inner_text_skips_display_none_subtree_and_hidden_text() {
    let mut rt = setup_runtime(
        "<html><body>\
             <div style=\"display:none\">gone</div>\
             <div style=\"visibility:hidden\">veiled</div>\
             <div hidden>also-gone</div>\
             kept</body></html>",
    );
    let t = rt.evaluate("document.body.innerText").unwrap();
    assert_eq!(t, serde_json::json!("kept"), "got: {t}");
}

#[test]
fn inner_text_breaks_lines_on_blocks_and_collapses_whitespace() {
    let mut rt = setup_runtime(
        "<html><body><p>hello   world</p><p>second</p>\
             <div><span>a</span><span>b</span> c</div>tail</body></html>",
    );
    let t = rt.evaluate("document.body.innerText").unwrap();
    // Two blocks -> single newline between; inline flow stays inline with
    // collapsed internal whitespace; text after a closed block starts a
    // new line.
    assert_eq!(
        t,
        serde_json::json!("hello world\nsecond\nab c\ntail"),
        "got: {t}"
    );
}

#[test]
fn inner_text_preserves_pre_content_verbatim() {
    let mut rt = setup_runtime("<html><body><pre>  keep\n   me  </pre><p>after</p></body></html>");
    let t = rt.evaluate("document.body.innerText").unwrap();
    assert_eq!(t, serde_json::json!("  keep\n   me  \nafter"), "got: {t}");
}

#[test]
fn inner_text_br_forces_line_break() {
    let mut rt = setup_runtime("<html><body><div>one<br>two</div></body></html>");
    let t = rt.evaluate("document.body.innerText").unwrap();
    assert_eq!(t, serde_json::json!("one\ntwo"), "got: {t}");
}

/// obscura#1106 absorption: three rendered-text contracts the old walk
/// missed — a DECLARED visibility:visible descendant restores text under
/// a hidden ancestor (undeclared descendants stay hidden); table cells
/// separate with tabs (one per boundary, an empty cell keeps its own,
/// rows break with newlines and eat the trailing tab); an unrendered
/// root — display:none or detached — falls back to textContent exactly
/// like Chrome instead of "".
#[test]
fn inner_text_restores_visible_descendants_tables_and_unrendered_roots() {
    let mut rt = setup_runtime(
        "<html><body>\
             <div style=\"visibility:hidden\">veiled<span style=\"visibility:visible\">shown</span><i>still-hidden</i></div>\
             <table><tr><td>a</td><td>b</td><td></td><td>c</td></tr><tr><td>d</td></tr></table>\
             <div id=\"cloaked\" style=\"display:none\">raw<script>var x = 1;</script>text</div>\
             </body></html>",
    );
    let t = rt.evaluate("document.body.innerText").unwrap();
    assert_eq!(t, serde_json::json!("shown\na\tb\t\tc\nd"), "got: {t}");
    // Unrendered root: display:none reads back raw textContent (script
    // source included, whitespace uncollapsed) — the classic scraper
    // shape where innerText on a hidden node still yields the text.
    let t = rt
        .evaluate("document.getElementById('cloaked').innerText")
        .unwrap();
    assert_eq!(t, serde_json::json!("rawvar x = 1;text"), "got: {t}");
    // Detached roots take the same fallback, not the collapsed walk.
    let t = rt
        .evaluate(
            "(() => { const d = document.createElement('div');\
                 d.textContent = '  detached  '; return d.innerText; })()",
        )
        .unwrap();
    assert_eq!(t, serde_json::json!("  detached  "), "got: {t}");
}

// UA `q` marks (Chrome q::before/::after open-quote/close-quote): diting
// has no generated content, so the layout synthesizes the quote leaves
// around flattened q content. They must take layout space in front of
// the q's own children and give the (boxless-union) q a wider rect.
#[test]
#[cfg(feature = "screenshot")]
fn test_q_element_quote_marks_take_layout_space() {
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
              <p style="margin:0"><span id="plain">hi</span></p>
              <p style="margin:0"><q><span id="quoted">hi</span></q></p>
              <p style="margin:0"><q id="empty"></q><span id="after">x</span></p>
            </body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const g = (id) => document.getElementById(id).getBoundingClientRect();
                    const plain = g('plain');
                    const quoted = g('quoted');
                    const q = document.querySelector('q').getBoundingClientRect();
                    const emptyQ = g('empty');
                    const after = g('after');
                    return {
                        shift: quoted.left - plain.left,
                        qWider: q.width > quoted.width,
                        emptyW: emptyQ.width,
                        afterShift: after.left - emptyQ.left,
                    };
                })())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let shift = d["shift"].as_f64().unwrap();
    assert!(
        shift > 1.0,
        "open quote must push the q's content right (shift={shift}, diag={d})"
    );
    assert!(
        d["qWider"].as_bool().unwrap(),
        "q union rect must include the marks (diag={d})"
    );
    let empty_w = d["emptyW"].as_f64().unwrap();
    assert!(
        empty_w > 0.0,
        "empty <q></q> still renders both marks (w={empty_w})"
    );
    let after_shift = d["afterShift"].as_f64().unwrap();
    assert!(
        after_shift > 1.0,
        "close quote must occupy space before the next sibling (shift={after_shift})"
    );
}

// 批240 / blitz#924: innerText REFLECTS text-transform while textContent
// keeps the source (Chrome reads back the rendered casing); a declared
// `none` on a descendant resets the inherited transform; getComputedStyle
// reports the declared value (and 'none' for elements with no
// declaration).
#[test]
fn inner_text_reflects_text_transform() {
    let mut rt = setup_runtime(
        r#"<html><body>
              <div style="text-transform:uppercase" id="u"><span>hello</span> world</div>
              <div style="text-transform:capitalize" id="cap">don't stop</div>
              <div style="text-transform:uppercase"><span style="text-transform:none" id="n">quiet</span></div>
              <div id="plain">plain</div>
            </body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const g = (el) => getComputedStyle(el).getPropertyValue('text-transform');
                    return {
                        inner: document.getElementById('u').innerText,
                        text: document.getElementById('u').textContent,
                        cap: document.getElementById('cap').innerText,
                        nested: document.getElementById('n').innerText,
                        gcsU: g(document.getElementById('u')),
                        gcsN: g(document.getElementById('n')),
                        gcsPlain: g(document.getElementById('plain')),
                    };
                })())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(d["inner"], serde_json::json!("HELLO WORLD"), "diag={d}");
    assert_eq!(
        d["text"],
        serde_json::json!("hello world"),
        "textContent keeps the source"
    );
    assert_eq!(
        d["cap"],
        serde_json::json!("Don't Stop"),
        "capitalize at word starts; apostrophe joins"
    );
    assert_eq!(
        d["nested"],
        serde_json::json!("quiet"),
        "declared none resets the inherited transform"
    );
    assert_eq!(d["gcsU"], serde_json::json!("uppercase"), "diag={d}");
    assert_eq!(d["gcsN"], serde_json::json!("none"));
    assert_eq!(
        d["gcsPlain"],
        serde_json::json!("none"),
        "no declaration reads the initial value"
    );
}

// text-transform rides the layout measure: an uppercased span is exactly
// as wide as its literal-uppercase twin (Chrome: uppercase glyphs, not
// the lowercase source, decide the box).
#[test]
#[cfg(feature = "screenshot")]
fn text_transform_uppercase_widens_layout() {
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
              <p style="margin:0"><span id="up" style="text-transform:uppercase">hello</span></p>
              <p style="margin:0"><span id="lit">HELLO</span></p>
              <p style="margin:0"><span id="low">hello</span></p>
            </body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => ({
                    upW: document.getElementById('up').getBoundingClientRect().width,
                    litW: document.getElementById('lit').getBoundingClientRect().width,
                    lowW: document.getElementById('low').getBoundingClientRect().width,
                }))())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let up = d["upW"].as_f64().unwrap();
    let lit = d["litW"].as_f64().unwrap();
    let low = d["lowW"].as_f64().unwrap();
    assert!(up > 0.0, "transformed span must lay out (diag={d})");
    assert!(
        (up - lit).abs() < 0.51,
        "uppercase transform width == literal uppercase width (up={up}, lit={lit})"
    );
    assert!(
        up > low + 0.5,
        "HELLO is wider than hello — the transform really rode the measure (up={up}, low={low})"
    );
}

/// Mono batch: a UA-monospace element (<code>) shapes its ASCII on the
/// bundled Noto Sans Mono — fixed 0.6em advance, Chrome's parity — while
/// a plain span stays proportional, an explicit `font-family: monospace`
/// on any element routes the same way, and CJK inside the run keeps the
/// CJK face's full-em advance (per-char fallback).
#[test]
fn test_monospace_face_routes_ascii_runs() {
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0;font-size:20px">
              <code id="c">0000000000</code>
              <span id="p">0000000000</span>
              <span id="m" style="font-family:monospace">0000000000</span>
              <code id="cjk">汉字</code>
            </body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const w = (id) => document.getElementById(id).getBoundingClientRect().width;
                    return { code: w('c'), plain: w('p'), explicit: w('m'), cjk: w('cjk') };
                })())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let w = |key: &str| d[key].as_f64().unwrap();
    assert!(
        (w("code") - 120.0).abs() < 1.5,
        "10 chars × 0.6em @20px = 120 (diag={d})"
    );
    assert!(
        (w("explicit") - 120.0).abs() < 1.5,
        "font-family:monospace routes the same (diag={d})"
    );
    assert!(
        (w("plain") - w("code")).abs() > 1.0,
        "proportional digits differ from mono (diag={d})"
    );
    assert!(
        (w("cjk") - 40.0).abs() < 1.5,
        "CJK in a mono run keeps full-em (diag={d})"
    );
}

/// line-clamp / -webkit-line-clamp (#271): the clamp caps the box's HEIGHT
/// by line count — count-only, never the content height (takumi #1965's
/// exactly-fits edge: a run wrapping to exactly N lines keeps all of them),
/// the computed style serializes "2"/"none", and it does not inherit.
/// #275 moved the gate onto Chrome's real face: the clamp only acts on
/// `display:-webkit-box` + `-webkit-box-orient:vertical`, so the clamped
/// boxes here wear the full legacy idiom card templates ship, and the
/// plain-block twin pins that the same declaration WITHOUT that display
/// no longer clamps (it never did in Chrome).
#[cfg(feature = "screenshot")]
#[test]
fn line_clamp_caps_height_by_line_count() {
    let mut rt = setup_runtime(
        r#"<div style="width:120px;line-height:20px;font-size:16px">
             <div id="clamped" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical">alpha beta gamma delta epsilon zeta eta theta</div>
             <div id="exact" style="line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical">alpha beta gamma</div>
             <div id="plain" style="-webkit-line-clamp:2">alpha beta gamma delta epsilon zeta eta theta</div>
             <div id="free">alpha beta gamma delta epsilon zeta eta theta</div>
             <div id="kid"><span>alpha beta gamma</span></div>
           </div>"#,
    );
    let parts = rt
        .evaluate(
            r#"
            const clamped = document.getElementById("clamped"),
                  exact = document.getElementById("exact"),
                  plain = document.getElementById("plain"),
                  free = document.getElementById("free"),
                  kid = document.getElementById("kid");
            return [
                getComputedStyle(clamped).getPropertyValue("-webkit-line-clamp"),
                getComputedStyle(clamped).getPropertyValue("line-clamp"),
                getComputedStyle(exact).getPropertyValue("line-clamp"),
                getComputedStyle(kid).getPropertyValue("line-clamp"),
                clamped.getBoundingClientRect().height,
                exact.getBoundingClientRect().height,
                plain.getBoundingClientRect().height,
                free.getBoundingClientRect().height,
            ];
        "#,
        )
        .unwrap();
    let parts = parts.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!("2"),
        "-webkit-line-clamp reads back"
    );
    assert_eq!(
        parts[1],
        serde_json::json!("2"),
        "the standard spelling answers from the same slot (alias)"
    );
    assert_eq!(
        parts[2],
        serde_json::json!("2"),
        "standard line-clamp reads back"
    );
    assert_eq!(
        parts[3],
        serde_json::json!("none"),
        "line-clamp does not inherit"
    );
    let (h_clamped, h_exact, h_plain, h_free) = (
        parts[4].as_f64().unwrap(),
        parts[5].as_f64().unwrap(),
        parts[6].as_f64().unwrap(),
        parts[7].as_f64().unwrap(),
    );
    assert!(
        (h_clamped - 40.0).abs() < 2.0,
        "clamped box is exactly 2 lines tall ({h_clamped})"
    );
    assert!(
        (h_exact - 40.0).abs() < 2.0,
        "exactly-2-line text is NOT clamped shorter ({h_exact}) — takumi #1965"
    );
    assert!(
        (h_plain - h_free).abs() < 2.0,
        "plain block ignores the clamp without -webkit-box ({h_plain} vs {h_free}) — Chrome parity (#275)"
    );
    assert!(h_free > 60.0, "unclamped sibling wraps taller ({h_free})");
    // The count-only rule: the clamped box is 2 lines even though its
    // content height (h_free) is taller — clamped == 40 no matter what.
    assert!(
        h_clamped < h_free - 20.0,
        "clamp cut lines away ({h_clamped} < {h_free})"
    );
}

/// #275: the clamp face Chrome actually ships (every height CDP-pinned on
/// Chrome 155): `-webkit-line-clamp` acts ONLY on `display:-webkit-box`
/// (inline-box included) with `-webkit-box-orient:vertical`, needs no
/// `overflow:hidden`, counts <br> lines and wrapped lines alike, and never
/// fires on plain blocks, flex, or a horizontal/unoriented box. A <br>
/// inside the armed box folds into the run as one line stack, so
/// `ab<br>cd<br>ef` clamps to 2 lines — and the br edge rules match too:
/// a leading br keeps its blank line under the clamp, a trailing br never
/// adds one.
#[cfg(feature = "screenshot")]
#[test]
fn line_clamp_webkit_box_faces() {
    let mut rt = setup_runtime(
        r#"<div style="font:20px/20px monospace">
             <div id="blk" style="-webkit-line-clamp:2;overflow:hidden">ab<br>cd<br>ef</div>
             <div id="wkb" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden">ab<br>cd<br>ef</div>
             <div id="flex" style="-webkit-line-clamp:2;display:flex;flex-direction:column;overflow:hidden">ab<br>cd<br>ef</div>
             <div id="free" style="overflow:hidden">ab<br>cd<br>ef</div>
             <div id="noorient" style="-webkit-line-clamp:2;display:-webkit-box;overflow:hidden">ab<br>cd<br>ef</div>
             <div id="horiz" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:horizontal;overflow:hidden">ab<br>cd<br>ef</div>
             <div id="ibox" style="-webkit-line-clamp:2;display:-webkit-inline-box;-webkit-box-orient:vertical;overflow:hidden">ab<br>cd<br>ef</div>
             <div id="nooverflow" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical">ab<br>cd<br>ef</div>
             <div id="wrapped" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden;width:40px">aa bb cc dd ee</div>
             <div id="wexactly" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden;width:40px">aa bb</div>
             <div id="lead" style="-webkit-line-clamp:2;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden"><br>ab<br>cd</div>
             <div id="trail" style="display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden">ab<br></div>
           </div>"#,
    );
    let out = rt
        .evaluate(
            r#"
            const g = i => document.getElementById(i).getBoundingClientRect().height;
            return ["blk","wkb","flex","free","noorient","horiz","ibox","nooverflow","wrapped","wexactly","lead","trail"].map(g);
        "#,
        )
        .unwrap();
    let h = out.as_array().expect("array result");
    // Chrome 155, same markup, same 20px/20px font: [60,40,60,60,60,60,40,40,40,40,40,20]
    let want: &[(usize, f64, &str)] = &[
        (0, 60.0, "plain block ignores the clamp"),
        (1, 40.0, "-webkit-box+vertical clamps br-mixed lines"),
        (2, 60.0, "modern flex ignores the clamp"),
        (3, 60.0, "unclamped sibling keeps 3 lines"),
        (4, 60.0, "-webkit-box without orient ignores the clamp"),
        (5, 60.0, "horizontal orient ignores the clamp"),
        (6, 40.0, "-webkit-inline-box clamps too"),
        (7, 40.0, "the clamp needs no overflow:hidden"),
        (8, 40.0, "wrapped lines count against the clamp"),
        (9, 40.0, "exactly N wrapped lines are kept, not cut"),
        (
            10,
            40.0,
            "a leading br keeps its blank line under the clamp",
        ),
        (
            11,
            20.0,
            "a trailing br adds no line (unclamped legacy face)",
        ),
    ];
    for (i, target, why) in want {
        let got = h[*i].as_f64().unwrap();
        assert!(
            (got - target).abs() < 2.0,
            "face #{i} ({why}): got {got}, want {target} — full heights {h:?}"
        );
    }
}

/// M4 knife 52 - document.fonts is a real FontFaceSet (EventTarget, insertion-ordered set of FontFace) and FontFace has Chrome's
/// full descriptor surface; fonts-probe.js, 27 keys against real Chrome 157 (157 exposes the FontFaceSet interface with a prototype
/// `constructor`, and FontFace grew `width`). `win[4]` is FontFaceSet's ABSOLUTE index in getOwnPropertyNames(window) -
/// exact since #30 put the non-configurable globals in Chrome's slots.
#[tokio::test(flavor = "current_thread")]
async fn fonts_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/fonts-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/fonts-truth-157.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// #28 - FontFace.width (CSS Fonts 4's rename of font-stretch, new between 146 and 157) and the exposed FontFaceSet
/// interface, against real Chrome 157 (fonts-width-probe.js, 15 keys): width is its OWN accessor pair over the stretch
/// slot, a descriptors dict carrying both lets `width` win (IDL converts members lexicographically), and
/// document.fonts.constructor is the window's FontFaceSet - not a global_surface stub.
#[tokio::test(flavor = "current_thread")]
async fn fontface_width_and_fontfaceset_iface_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/fonts-width-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/fonts-width-truth-157.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
