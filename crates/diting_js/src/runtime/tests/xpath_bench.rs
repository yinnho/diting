//! Per-call cost of document.evaluate under the native bridge: every call
//! snapshots the live tree in Rust before evaluating. Manual-run (#[ignore])
//! ledger measurement, not a CI gate — sizes chosen to bracket heavy SPAs
//! (抖店/天猫 sit in the 20k-80k element range).

use super::common::setup_runtime;

fn big_html(elements: usize) -> String {
    // realistic scraping-target shape: nested rows with several attributes,
    // class lists, data-* and text content
    let mut out = String::with_capacity(elements * 120);
    out.push_str("<html><body><div id=root>");
    for i in 0..elements {
        out.push_str(&format!(
            "<div class='item row x{}' data-n='{}' id='i{}'><a href='#{}'><span class='t'>text {}</span><b>bold</b></a><input value='v{}'></div>",
            i % 7, i, i, i, i, i
        ));
    }
    out.push_str("</div></body></html>");
    out
}

#[test]
#[ignore]
fn xpath_first_vs_warm_call() {
    // cold = first evaluate after a mutation (full snapshot rebuild);
    // warm = repeat evaluate on an unmutated tree (cache hit). The split
    // shows which side dominates: snapshot rebuild or moli evaluation.
    for elements in [1000usize, 5000, 20000] {
        let mut rt = setup_runtime(&big_html(elements));
        for (label, expr, ty) in [
            ("nodeset snapshot", "//div[@data-n]", 7),
            ("scalar count", "count(//div)", 0),
            (
                "deep predicate",
                "//div[@data-n>100 and contains(@class,'row')]/a/span",
                7,
            ),
        ] {
            let js = format!(
                "(function(){{try{{var out=[];for(var i=0;i<20;i++){{if(i>0){{document.body.setAttribute('data-bump',''+i)}};var t0=Date.now();var r=document.evaluate({q},document,null,{t},null);if(r.resultType===7)r.snapshotItem(0);var cold=Date.now()-t0;t0=Date.now();r=document.evaluate({q},document,null,{t},null);if(r.resultType===7)r.snapshotItem(0);var warm=Date.now()-t0;out.push([cold,warm])}};return JSON.stringify(out)}}catch(err){{return 'threw: '+err.message}}}})()",
                q = serde_json::to_string(expr).unwrap(),
                t = ty
            );
            let out = rt.evaluate(&js).unwrap();
            let Some(txt) = out.as_str() else {
                panic!("{elements} elems | {label}: non-string out {out:?}");
            };
            if txt.starts_with("threw:") {
                panic!("{elements} elems | {label}: {txt}");
            }
            let rows: Vec<(f64, f64)> = serde_json::from_str(txt).unwrap();
            let cold: f64 = rows.iter().map(|r| r.0).sum::<f64>() / rows.len() as f64;
            let warm: f64 = rows.iter().map(|r| r.1).sum::<f64>() / rows.len() as f64;
            println!("{elements} elems | {label}: cold {cold:.2}ms warm {warm:.2}ms");
        }
    }
    panic!("ledger: see stdout");
}

#[test]
#[ignore]
fn xpath_evaluate_cost_scaling() {
    for elements in [1000usize, 5000, 20000] {
        let mut rt = setup_runtime(&big_html(elements));
        // measure inside JS so the op roundtrip is included exactly once per
        // call, as in production; 30 iterations per shape after 5 warmups
        for (label, expr, ty) in [
            ("nodeset snapshot", "//div[@data-n]", 7),
            ("scalar count", "count(//div)", 0),
            (
                "deep predicate",
                "//div[@data-n>100 and contains(@class,'row')]/a/span",
                7,
            ),
        ] {
            let js = format!(
                "(function(){{try{{for(var w=0;w<5;w++){{document.evaluate({q},document,null,{t},null)}};var t0=Date.now();for(var i=0;i<30;i++){{var r=document.evaluate({q},document,null,{t},null);if(r.resultType===7)r.snapshotItem(0)}};return Date.now()-t0}}catch(err){{return 'threw: '+err.message}}}})()",
                q = serde_json::to_string(expr).unwrap(),
                t = ty
            );
            let out = rt.evaluate(&js).unwrap();
            if let Some(msg) = out.as_str() {
                panic!("{elements} elems | {label}: {msg}");
            }
            let total_ms = out.as_f64().unwrap_or(f64::NAN);
            println!(
                "{elements} elems | {label}: {:.2}ms/call ({} calls)",
                total_ms / 30.0,
                30
            );
        }
    }
    panic!("ledger: see stdout");
}

/// Correctness gate for the snapshot cache, NOT ignored: every content-
/// mutation path the cache keys on must invalidate it. A stale hit is a
/// silent wrong answer — worse than the rebuild cost the cache saves.
#[test]
fn xpath_cache_invalidates_on_mutation() {
    let mut rt = setup_runtime(
        "<html><body><div id=root><div class=a data-n=1><span>one</span></div></div></body></html>",
    );
    // structure: appendChild grows count(//div)
    let out = rt
        .evaluate(
            r#"(function(){
            var root = document.getElementById('root');
            var before = document.evaluate('count(//div)', document, null, 0, null).numberValue;
            var d = document.createElement('div');
            root.appendChild(d);
            var after = document.evaluate('count(//div)', document, null, 0, null).numberValue;
            return [before, after].join(',');
        })()"#,
        )
        .unwrap();
    assert_eq!(out.as_str().unwrap(), "2,3", "appendChild must invalidate");

    // attribute write: setAttribute flips a predicate match
    let out = rt
        .evaluate(
            r#"(function(){
            var before = document.evaluate('count(//div[@data-n=77])', document, null, 0, null).numberValue;
            document.querySelector('.a').setAttribute('data-n', '77');
            var after = document.evaluate('count(//div[@data-n=77])', document, null, 0, null).numberValue;
            return [before, after].join(',');
        })()"#,
        )
        .unwrap();
    assert_eq!(out.as_str().unwrap(), "0,1", "setAttribute must invalidate");

    // text write: string() content must refresh
    let out = rt
        .evaluate(
            r#"(function(){
            var before = document.evaluate('string(//span)', document, null, 0, null).stringValue;
            document.querySelector('span').textContent = 'two';
            var after = document.evaluate('string(//span)', document, null, 0, null).stringValue;
            return before + '|' + after;
        })()"#,
        )
        .unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        "one|two",
        "textContent must invalidate"
    );

    // removal: removeChild shrinks count
    let out = rt
        .evaluate(
            r#"(function(){
            var root = document.getElementById('root');
            var before = document.evaluate('count(//div)', document, null, 0, null).numberValue;
            root.removeChild(root.firstChild);
            var after = document.evaluate('count(//div)', document, null, 0, null).numberValue;
            return [before, after].join(',');
        })()"#,
        )
        .unwrap();
    assert_eq!(out.as_str().unwrap(), "3,2", "removeChild must invalidate");
}

/// The cache key (mutation epoch, root NodeId) collides across documents —
/// a fresh tree restarts its epoch at small values and the document node is
/// always slot 0 — so set_dom must clear it. Regression test: evaluate on
/// one document, swap in a new dom, re-evaluate; the answer must come from
/// the NEW tree.
#[test]
fn xpath_cache_dropped_on_document_swap() {
    let mut rt = setup_runtime("<html><body><div></div></body></html>");
    let first = rt
        .evaluate("(document.evaluate('count(//div)', document, null, 0, null).numberValue)")
        .unwrap();
    assert_eq!(first.as_f64().unwrap(), 1.0);
    // the navigation path: fresh dom installed through the same wiring
    let dom2 = diting_dom::parse_html("<html><body><p></p><p></p></body></html>");
    rt.set_dom(dom2);
    let second = rt
        .evaluate("(document.evaluate('count(//div)', document, null, 0, null).numberValue)")
        .unwrap();
    assert_eq!(second.as_f64().unwrap(), 0.0);
}
