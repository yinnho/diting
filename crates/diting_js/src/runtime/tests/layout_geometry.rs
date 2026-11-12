//! 几何读数：gbcr/offset/from_point 命中/z 序/transform
use super::super::*;
use super::common::setup_runtime;

/// #243: gBCR serves Chrome-style subpixel geometry while the IDL
/// integer accessors stay rounded. taffy's round_layout pins paint boxes
/// to the integer grid; the JS-facing rect map now keeps taffy's
/// UNROUNDED border-box size, and offset*/client* Math.round on the JS
/// side — so `height: 50.6px; border: 1px` reads 52.6 from gBCR and 53
/// from offsetHeight, exactly Chrome's split.
#[test]
fn gbcr_keeps_subpixel_geometry_while_integer_accessors_round() {
    let mut rt = setup_runtime(
        "<html><body style='margin: 0'>\
             <div id='f' style='height: 50.6px; border: 1px solid; width: 100px'></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var f = document.getElementById('f');
            var r = f.getBoundingClientRect();
            [r.height, r.width, r.top, r.bottom, f.offsetHeight, f.offsetWidth].join('|');
        "#,
        )
        .unwrap();
    // height 50.6 + 2px border = 52.6 unrounded (was 53); width 100 + 2px
    // border = 102 (content-box); top edge is integral either way (body
    // margin 0); offset* keep rounding.
    assert_eq!(
        out.as_str().unwrap(),
        "52.6|102|0|52.6|53|102",
        "gBCR returns float geometry; offset* round to integers"
    );
}

/// 批234 (blitz#940): the UA form-control font surfaces through the
/// computed-style face exactly as Chrome reports it — 13.3333px, sans
/// family (monospace for textarea), weight 400 — inside a parent pushing
/// all four the other way; author longhands still win (the
/// `input { font: inherit }` reset contract).
#[test]
fn computed_face_reports_form_control_ua_font() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             #p { font: 700 24px/3 monospace; }\
             #i2 { font-family: inherit; font-size: inherit; }\
             </style></head><body>\
             <div id='p'><input id='i'><textarea id='t'></textarea><input id='i2'></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var i = getComputedStyle(document.getElementById('i'));
            var t = getComputedStyle(document.getElementById('t'));
            var i2 = getComputedStyle(document.getElementById('i2'));
            return [i.fontSize, i.fontWeight, i.fontFamily, t.fontFamily, i2.fontSize].join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        "13.3333px|400|sans-serif|monospace|24px",
        "control carries its own UA font; authored inherit falls back to the parent's"
    );
}

#[test]
fn test_element_from_point_is_function() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let kind = rt.evaluate("typeof document.elementFromPoint").unwrap();
    assert_eq!(kind, serde_json::json!("function"));
    let kind2 = rt.evaluate("typeof document.elementsFromPoint").unwrap();
    assert_eq!(kind2, serde_json::json!("function"));
}

// Regression (obscura #738): hit-testing among overlapping positioned
// siblings must follow paint order, not document order. The close
// button (z-index 1002) precedes the loading overlay (z-index 1001) in
// the DOM; the old nid tie-break handed the click to the overlay — a
// coordinate click aimed at the visibly topmost element was delivered
// to the z-index-below one. Feature-gated: real rects need the layout
// stack (as above).
#[test]
#[cfg(feature = "screenshot")]
fn test_element_from_point_respects_z_index_among_positioned_siblings() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .dialog { position: absolute; top: 100px; left: 100px; width: 400px; height: 200px; }
          .close-button { position: absolute; top: 10px; left: 350px; width: 32px; height: 32px; z-index: 1002; }
          .loading-overlay { position: absolute; top: 0; left: 0; width: 400px; height: 200px; z-index: 1001; }
        </style></head><body>
        <div class="dialog"><button class="close-button">x</button><div class="loading-overlay"></div></div>
        </body></html>"#,
    );
    let diag = rt
            .evaluate(
                r#"JSON.stringify((() => {
                    const btn = document.querySelector('.close-button');
                    const r = btn.getBoundingClientRect();
                    return {
                        z: getComputedStyle(btn).zIndex,
                        overlayZ: getComputedStyle(document.querySelector('.loading-overlay')).zIndex,
                        pos: getComputedStyle(btn).position,
                        rect: [r.left, r.top, r.width, r.height],
                    };
                })())"#,
            )
            .unwrap();
    println!("z-index hit-test diagnostics: {diag}");
    let hit = rt
            .evaluate(
                r#"(function() {
                    const btn = document.querySelector('.close-button');
                    const r = btn.getBoundingClientRect();
                    return document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)?.className;
                })()"#,
            )
            .unwrap();
    assert_eq!(
        hit,
        serde_json::json!("close-button"),
        "topmost z-index must win the hit, got {hit} (diag: {diag})"
    );
    // elementsFromPoint mirrors the ranking: front-to-back, the button
    // before the overlay it visually sits on.
    let stack = rt
        .evaluate(
            r#"(function() {
                    const btn = document.querySelector('.close-button');
                    const r = btn.getBoundingClientRect();
                    return document
                        .elementsFromPoint(r.left + r.width / 2, r.top + r.height / 2)
                        .slice(0, 2)
                        .map((el) => el.className)
                        .join(',');
                })()"#,
        )
        .unwrap();
    assert_eq!(
        stack,
        serde_json::json!("close-button,loading-overlay"),
        "elementsFromPoint must be front-to-back by paint order, got {stack}"
    );
}

/// Chrome ends every elementsFromPoint stack with `<body>` then `<html>`;
/// both span the viewport, so they are appended after the ranked
/// descendants, never ranked among them — ranking a viewport-spanning
/// box would shadow every real descendant in elementFromPoint
/// (absorbing obscura PR #848's append-not-rank resolution).
#[test]
#[cfg(feature = "screenshot")]
fn test_elements_from_point_appends_body_and_html() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .dialog { position: absolute; top: 100px; left: 100px; width: 400px; height: 200px; }
        </style></head><body>
        <div class="dialog"></div>
        </body></html>"#,
    );
    let stack = rt
        .evaluate(
            r#"(function() {
                    const d = document.querySelector('.dialog');
                    const r = d.getBoundingClientRect();
                    return document
                        .elementsFromPoint(r.left + r.width / 2, r.top + r.height / 2)
                        .map((el) => el.tagName + (el.className ? '.' + el.className : ''))
                        .join('|');
                })()"#,
        )
        .unwrap();
    assert_eq!(
        stack,
        serde_json::json!("DIV.dialog|BODY|HTML"),
        "stack must end with BODY then HTML, got {stack}"
    );
}

// Event-coordinate surface under transforms (blitz #663 family).
// pageX/pageY ride the client point plus root scroll; x/y alias
// clientX/clientY; offsetX/offsetY inverse-map the hit point through
// the element's TOTAL paint transform into its local space. The box is
// 100x100 at doc (100,100) rotated 45° about its center (150,150); a
// hit at (150,110) inverse-maps to local (121.716, 121.716), i.e.
// offset (21.716, 21.716) from the box origin.
#[test]
#[cfg(feature = "screenshot")]
fn test_mouse_event_coordinates_under_rotation() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
            body { margin: 0; }
            #rot { position: absolute; left: 100px; top: 100px; width: 100px; height: 100px;
                   transform: rotate(45deg); }
          </style></head><body><div id="rot"></div></body></html>"#,
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((function() {
                    const d = document.getElementById('rot');
                    const ev = new MouseEvent('click', { clientX: 150, clientY: 110 });
                    d.dispatchEvent(ev);
                    const r = d.getBoundingClientRect();
                    return {
                        ox: +ev.offsetX.toFixed(3), oy: +ev.offsetY.toFixed(3),
                        px: ev.pageX, py: ev.pageY, x: ev.x, y: ev.y,
                        gleft: +r.left.toFixed(2), gtop: +r.top.toFixed(2),
                        gwidth: +r.width.toFixed(2),
                        offsetLeft: d.offsetLeft, offsetTop: d.offsetTop,
                    };
                })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["px"],
        serde_json::json!(150),
        "pageX = clientX + root scroll (0 here)"
    );
    assert_eq!(v["py"], serde_json::json!(110));
    assert_eq!(v["x"], serde_json::json!(150), "x aliases clientX");
    assert_eq!(v["y"], serde_json::json!(110));
    let ox = v["ox"].as_f64().unwrap();
    let oy = v["oy"].as_f64().unwrap();
    assert!(
        (ox - 21.716).abs() < 0.05 && (oy - 21.716).abs() < 0.05,
        "offset must be the inverse-mapped local point (21.716,21.716), got ({ox},{oy})"
    );
    // gBCR carries the mapped bounding box (a rotated square's AABB is
    // the 141.42 diagonal box); offsetLeft/Top stay layout-true.
    assert_eq!(
        v["offsetLeft"],
        serde_json::json!(100),
        "offsetLeft ignores the transform"
    );
    assert_eq!(v["offsetTop"], serde_json::json!(100));
    assert_eq!(
        v["gleft"],
        serde_json::json!(79.29),
        "gBCR is the mapped AABB"
    );
    assert_eq!(v["gtop"], serde_json::json!(79.29));
    assert_eq!(v["gwidth"], serde_json::json!(141.42));
}

// Exact-shape hit testing (blitz #663 family): the corner of a rotated
// box's AABB covers no pixel of the element, so elementFromPoint there
// must NOT hand back the rotated element — while its center still hits.
#[test]
#[cfg(feature = "screenshot")]
fn test_element_from_point_rejects_rotated_aabb_corners() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
            body { margin: 0; }
            #rot { position: absolute; left: 100px; top: 100px; width: 100px; height: 100px;
                   transform: rotate(45deg); }
          </style></head><body><div id="rot"></div></body></html>"#,
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((function() {
                    return {
                        center: document.elementFromPoint(150, 110)?.id || null,
                        corner: document.elementFromPoint(85, 85)?.id || null,
                    };
                })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["center"],
        serde_json::json!("rot"),
        "a point inside the rotated shape must hit the element"
    );
    assert_ne!(
        v["corner"],
        serde_json::json!("rot"),
        "the AABB corner (85,85) is outside the rotated diamond — must not hit"
    );
}

/// A `display: none` element generates no box, so it cannot be hit however
/// high its z-index: the zero-size rect filter runs before ranking ever
/// sees it. Pinning this because a boxless element entering the ranking
/// would win by accident (obscura PR #848's second test, same shape).
#[test]
#[cfg(feature = "screenshot")]
fn test_element_from_point_ignores_display_none_sibling() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .real { position: absolute; top: 100px; left: 100px; width: 200px; height: 100px; }
          .hidden { position: absolute; top: 100px; left: 100px; width: 200px; height: 100px; z-index: 9999; display: none; }
        </style></head><body>
        <div class="hidden"></div><div class="real"></div>
        </body></html>"#,
    );
    let hit = rt
            .evaluate(
                r#"(function() {
                    const r = document.querySelector('.real').getBoundingClientRect();
                    return document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)?.className;
                })()"#,
            )
            .unwrap();
    assert_eq!(
        hit,
        serde_json::json!("real"),
        "a display:none sibling must never take the hit, got {hit}"
    );
}

/// A nid absent from the rects map is a legitimate miss on a fresh cache
/// (<head>, display:none subtrees, svg children under the svg v1 one-box
/// model) — not a staleness signal. Conflating the two re-ran the
/// whole-page layout once per boxless lookup; on svg-heavy pages (archify
/// workflow artifacts: ~530 boxless svg nodes) a single elementFromPoint
/// walk re-laid-out the tree hundreds of times and tripped the 10s eval
/// watchdog, surfacing as silent `null` eval results and dead clicks.
/// The timing itself isn't unit-assertable without wall-clock flakiness,
/// so this pins the semantics: hit testing stays correct on a
/// boxless-heavy DOM, both on first layout and after an epoch bump.
#[test]
#[cfg(feature = "screenshot")]
fn test_element_from_point_on_boxless_heavy_dom() {
    let mut circles = String::new();
    for i in 0..200 {
        circles.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"4\"/>\n",
            20 + (i % 20) * 19,
            20 + (i / 20) * 19
        ));
    }
    let html = format!(
        r#"<html><head><style>
          .panel {{ position: absolute; top: 40px; left: 40px; width: 200px; height: 100px; }}
          .ghost {{ position: absolute; top: 40px; left: 40px; width: 200px; height: 100px; display: none; }}
          svg {{ position: absolute; top: 0; left: 0; }}
        </style></head><body>
        <svg width="400" height="400" viewBox="0 0 400 400">{circles}</svg>
        <div class="ghost"></div><div class="panel"></div>
        </body></html>"#
    );
    let mut rt = setup_runtime(&html);
    let probe = |rt: &mut JsRuntime| {
        let v = rt
                .evaluate(
                    r#"(function() {
                    const p = document.querySelector('.panel').getBoundingClientRect();
                    const hit = document.elementFromPoint(p.left + p.width / 2, p.top + p.height / 2);
                    const circles = document.querySelectorAll('circle').length;
                    // boxless elements must not poison the walk: 200 svg
                    // children + a display:none sibling all lack rects
                    const second = document.elementFromPoint(370, 380);
                    return JSON.stringify({
                        hit: hit ? hit.className || hit.tagName : null,
                        circles,
                        svgArea: second ? second.tagName : null,
                    });
                })()"#,
                )
                .unwrap();
        serde_json::from_str::<serde_json::Value>(v.as_str().unwrap()).unwrap()
    };
    let first = probe(&mut rt);
    assert_eq!(first["circles"], serde_json::json!(200));
    assert_eq!(first["hit"], serde_json::json!("panel"));
    // SVG-namespace elements keep their source-case tag name (Chrome
    // tagName uppercases only the HTML namespace).
    assert_eq!(first["svgArea"], serde_json::json!("svg"));

    // Mutate: the epoch bumps, the memoized run must be re-run (not
    // served from the stale cache), and the panel must move accordingly.
    rt.evaluate(r#"document.querySelector('.panel').style.top = '300px'; 'ok'"#)
        .unwrap();
    let after = probe(&mut rt);
    assert_eq!(after["hit"], serde_json::json!("panel"));
}

#[test]
#[cfg(feature = "screenshot")]
fn test_element_from_point_in_viewport_returns_body() {
    let mut rt = setup_runtime("<html><body><h1>Hi</h1></body></html>");
    // With diting-layout rects backing getBoundingClientRect, hit testing
    // is real. The h1's UA box (body 8px margin + h1 .67em margin-top,
    // then the 2em line box) starts around y≈30, so (10,40) lands on it;
    // (10,10) sits in the h1's margin — body territory, like Chrome.
    //
    // Feature-gated: the layout_rects_all pipeline (and the
    // _domRaw("layout_rect") op behind gBCR) only exists under the
    // `screenshot` feature; without it gBCR falls back to bootstrap.js's
    // synthetic nid-hashed grid (h1 lands at 450,190), which this test
    // was never meant to pin. The long-standing bare-`cargo test` red
    // was exactly that gate, not a layout regression — bisect pointed at
    // 83129c5 only because that's when the test started asserting real
    // rects.
    let tag = rt
        .evaluate("document.elementFromPoint(10, 40)?.tagName")
        .unwrap();
    assert_eq!(tag, serde_json::json!("H1"));
    let margin_area = rt
        .evaluate("document.elementFromPoint(10, 10)?.tagName")
        .unwrap();
    assert_eq!(margin_area, serde_json::json!("BODY"));
    // Below the h1's line box the point falls through to the body.
    let below = rt
        .evaluate("document.elementFromPoint(10, 500)?.tagName")
        .unwrap();
    assert_eq!(below, serde_json::json!("BODY"));
}

// #217 / obscura #1164: hit-test uses half-open [left,right)×[top,bottom).
// Points on the right or bottom edge must NOT hit the box (adjacent
// siblings would otherwise both claim the shared edge).
#[test]
#[cfg(feature = "screenshot")]
fn test_element_from_point_excludes_right_and_bottom_edges() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          #box { position:absolute; left:100px; top:100px; width:50px; height:50px; background:#000; }
        </style></head><body><div id="box"></div></body></html>"#,
    );
    let diag = rt
            .evaluate(
                r#"JSON.stringify((() => {
                    const r = document.getElementById('box').getBoundingClientRect();
                    const idAt = (x, y) => {
                      const el = document.elementFromPoint(x, y);
                      return el && el.id ? el.id : (el && el.tagName) || null;
                    };
                    return {
                      rect: { left: r.left, top: r.top, right: r.right, bottom: r.bottom, w: r.width, h: r.height },
                      center: idAt(r.left + r.width / 2, r.top + r.height / 2),
                      leftTop: idAt(r.left, r.top),
                      rightEdge: idAt(r.right, r.top + r.height / 2),
                      bottomEdge: idAt(r.left + r.width / 2, r.bottom),
                      justInsideBR: idAt(r.right - 0.5, r.bottom - 0.5),
                    };
                })())"#,
            )
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(diag.as_str().unwrap()).unwrap();
    assert_eq!(v["center"], "box", "center must hit: {v}");
    assert_eq!(v["leftTop"], "box", "left/top inclusive: {v}");
    assert_ne!(
        v["rightEdge"], "box",
        "right edge exclusive (obscura #1164): {v}"
    );
    assert_ne!(
        v["bottomEdge"], "box",
        "bottom edge exclusive (obscura #1164): {v}"
    );
    assert_eq!(v["justInsideBR"], "box", "just inside BR still hits: {v}");
}

// Regression (obscura #740): translate percentages resolve against the
// element's own box — translate(-50%, -50%) centers a top:50% box. The
// Y axis used to come out wrong while X was right. Feature-gated: real
// geometry needs the layout stack.
#[test]
#[cfg(feature = "screenshot")]
fn test_translate_negative_percent_centers_the_box() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .dialog { position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); width: 400px; height: 140px; }
        </style></head><body>
        <div class="dialog"></div>
        </body></html>"#,
    );
    let diag = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const r = document.querySelector('.dialog').getBoundingClientRect();
                    return { top: r.top, left: r.left, w: r.width, h: r.height,
                             vw: window.innerWidth, vh: window.innerHeight };
                })())"#,
        )
        .unwrap();
    println!("translate diagnostics: {diag}");
    let v: serde_json::Value = serde_json::from_str(diag.as_str().unwrap()).unwrap();
    let vh = v["vh"].as_f64().unwrap();
    let vw = v["vw"].as_f64().unwrap();
    let top = v["top"].as_f64().unwrap();
    let left = v["left"].as_f64().unwrap();
    assert_eq!(v["w"].as_f64().unwrap(), 400.0);
    assert_eq!(v["h"].as_f64().unwrap(), 140.0);
    // top:50% of the viewport, minus half the box: dead center.
    assert_eq!(
        top,
        vh / 2.0 - 70.0,
        "Y must be viewport-centered, diag: {diag}"
    );
    assert_eq!(
        left,
        vw / 2.0 - 200.0,
        "X must be viewport-centered, diag: {diag}"
    );
}

// Nested translate (px form): the ancestor's translate carries the
// whole subtree, the child's own translate stacks on top — and the
// layout-internal position of the child inside the ancestor is
// unchanged (transforms never feed back into layout).
#[test]
#[cfg(feature = "screenshot")]
fn test_translate_px_stacks_down_the_subtree() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .outer { transform: translate(10px, 20px); width: 200px; height: 100px; }
          .inner { transform: translate(0, -5px); width: 50px; height: 50px; }
        </style></head><body>
        <div class="outer"><div class="inner"></div></div>
        </body></html>"#,
    );
    let diag = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const o = document.querySelector('.outer').getBoundingClientRect();
                    const i = document.querySelector('.inner').getBoundingClientRect();
                    return { dx: i.left - o.left, dy: i.top - o.top,
                             ow: o.width, oh: o.height, iw: i.width, ih: i.height };
                })())"#,
        )
        .unwrap();
    println!("translate px diagnostics: {diag}");
    let v: serde_json::Value = serde_json::from_str(diag.as_str().unwrap()).unwrap();
    // Layout kept the child at the ancestor's content origin; only the
    // two translates move it: 0 + 0 = 0 in X, 0 + (-5) = -5 in Y.
    assert_eq!(v["dx"].as_f64().unwrap(), 0.0, "diag: {diag}");
    assert_eq!(v["dy"].as_f64().unwrap(), -5.0, "diag: {diag}");
    assert_eq!(v["ow"].as_f64().unwrap(), 200.0);
    assert_eq!(v["iw"].as_f64().unwrap(), 50.0);
}

// obscura #937 family: transforms only apply to transformable boxes
// (block-level or atomic inline-level). A non-replaced inline box is
// not transformable — Chrome ignores its transform for gBCR,
// hit-testing AND ink extent (paint already ignores it here).
#[test]
#[cfg(feature = "screenshot")]
fn transform_on_non_replaced_inline_is_ignored_by_gbcr() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          #i1, #i2 { display: inline; }
          #i2 { transform: translateX(100px); }
          #b1, #b2 { display: inline-block; }
          #b2 { transform: translateX(100px); }
          .row { width: 50px; }
        </style></head><body>
        <div class="row"><span id="i1">hi</span></div>
        <div class="row"><span id="i2">hi</span></div>
        <div><span id="b1">yo</span></div>
        <div><span id="b2">yo</span></div>
        </body></html>"#,
    );
    let diag = rt
            .evaluate(
                r#"JSON.stringify((() => {
                    try {
                        const e = id => document.getElementById(id);
                        const g = id => e(id).getBoundingClientRect();
                        const i1 = g('i1'), i2 = g('i2'), b1 = g('b1'), b2 = g('b2');
                        const midY = i2.top + i2.height / 2;
                        const atStatic = document.elementFromPoint(i2.left + i2.width / 2, midY);
                        const atShifted = document.elementFromPoint(i2.left + 100 + i2.width / 2, midY);
                        return { iDx: i2.left - i1.left, bDx: b2.left - b1.left,
                                 atStatic: atStatic ? atStatic.id : null,
                                 atShifted: atShifted ? atShifted.id : null,
                                 sw1: e('i1').parentElement.scrollWidth,
                                 sw2: e('i2').parentElement.scrollWidth };
                    } catch (err) { return { error: String(err) }; }
                })())"#,
            )
            .unwrap();
    println!("inline transform diagnostics: {diag}");
    let v: serde_json::Value = serde_json::from_str(diag.as_str().unwrap()).unwrap();
    assert_eq!(
        v["bDx"].as_f64().unwrap(),
        100.0,
        "control: inline-block IS transformable, must shift: {diag}"
    );
    assert_eq!(
        v["iDx"].as_f64().unwrap(),
        0.0,
        "non-replaced inline is not transformable — gBCR must ignore the transform: {diag}"
    );
    let static_hit = v["atStatic"].as_str().unwrap_or("(null)");
    let shifted_hit = v["atShifted"].as_str().unwrap_or("(null)");
    assert_eq!(
        static_hit, "i2",
        "hit-testing must also ignore the inline transform: {diag}"
    );
    assert_ne!(
        shifted_hit, "i2",
        "the shifted position must not hit the inline box: {diag}"
    );
    assert_eq!(
        v["sw1"].as_u64().unwrap(),
        v["sw2"].as_u64().unwrap(),
        "ink extent must ignore the inline transform too: {diag}"
    );
}

#[test]
fn test_element_from_point_out_of_viewport_returns_null() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let neg_x = rt.evaluate("document.elementFromPoint(-1, 10)").unwrap();
    assert_eq!(neg_x, serde_json::Value::Null);
    let neg_y = rt.evaluate("document.elementFromPoint(10, -1)").unwrap();
    assert_eq!(neg_y, serde_json::Value::Null);
    let huge = rt
        .evaluate("document.elementFromPoint(99999, 99999)")
        .unwrap();
    assert_eq!(huge, serde_json::Value::Null);
}

#[test]
fn test_elements_from_point_returns_array() {
    // Length 2, not 1: Chrome answers [BODY, HTML] on a bare page (the
    // old expectation of 1 encoded our pre-append behaviour, not the
    // web's — same correction as obscura PR #848, measured against
    // headless Chrome rather than assumed).
    let mut rt = setup_runtime("<html><body></body></html>");
    let len_in = rt
        .evaluate("document.elementsFromPoint(10, 10).length")
        .unwrap();
    assert_eq!(len_in.as_f64().unwrap() as i64, 2);
    let len_out = rt
        .evaluate("document.elementsFromPoint(-1, -1).length")
        .unwrap();
    assert_eq!(len_out.as_f64().unwrap() as i64, 0);
}

#[test]
fn test_element_from_point_non_numeric_returns_null() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let nan = rt.evaluate("document.elementFromPoint(NaN, 10)").unwrap();
    assert_eq!(nan, serde_json::Value::Null);
    let inf = rt
        .evaluate("document.elementFromPoint(Infinity, 10)")
        .unwrap();
    assert_eq!(inf, serde_json::Value::Null);
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_header_pins_gbcr_and_hit_test_under_root_scroll() {
    // #434 sticky v1: sticky vs the ROOT scroller. gBCR serves
    // CLIENT-space boxes (doc-space layout rects minus the root
    // scroll, sticky shift folded in), so a top:0 header that has
    // stuck reads top ≈ 0 however deep the page scrolls;
    // elementFromPoint is a client-space query over those same
    // rects, so the stuck header must win at client y=5. The tall
    // later static sibling is the load-bearing part of the hit
    // assertion: its box covers the stuck band, so the header only
    // wins because sticky is a POSITIONED paint level (App. E step
    // 8), not document order.
    let mut rt = setup_runtime(
        r#"<html><body><div id="head" style="position:sticky; top:0; height:40px; background:#ccc">H</div><div id="tall" style="height:4000px; background:#eee">filler</div></body></html>"#,
    );
    let script = r#"async () => {
            const head = document.getElementById('head');
            const pos = getComputedStyle(head).position;
            const inFlow = head.getBoundingClientRect().top;
            window.scrollTo(0, 300);
            const stuck = head.getBoundingClientRect().top;
            const hitEl = document.elementFromPoint(10, 5);
            const hitId = hitEl ? (hitEl.id || '') : 'null';
            window.scrollTo(0, 0);
            await new Promise(r => setTimeout(r, 10));
            const back = head.getBoundingClientRect().top;
            return [
                pos,
                inFlow < 50,
                Math.abs(stuck) < 0.5,
                hitId,
                Math.abs(back - inFlow) < 0.5,
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["sticky", true, true, "head", true]),
        "computed face, in-flow rest, stuck client top == 0, client hit lands on the stuck header, back to in-flow"
    );
}

// Cross-parent z (the narrow slice): a positioned z>0 child of a
// non-stacking-context parent hoists to the nearest context ancestor.
// Chrome ground truth: elementFromPoint(50,50) is B, not the later
// sibling C(z:1) — diting used to answer C because B never left A's
// per-parent band.
#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_cross_parent_z_hoists_to_stacking_context() {
    let mut rt = setup_runtime(
        r#"<!doctype html><html><body style="margin:0">
    <div id="A" style="position:relative;height:100px"><div id="B" style="position:absolute;z-index:5;top:0;left:0;width:100px;height:100px;background:red"></div></div>
    <div id="C" style="position:relative;z-index:1;margin-top:-100px;width:100px;height:100px;background:blue"></div>
    </body></html>"#,
    );
    let script = r#"() => {
          const el = document.elementFromPoint(50, 50);
          return el ? el.id : 'none';
        }"#;
    let out = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(out.value.unwrap(), serde_json::json!("B"));
}

// The escape bubbles through a CHAIN of non-context frames: two
// relative z-auto intermediates both refuse to consume B(z:5), so it
// lands at the root context and out-paints C(z:1) — Chrome parity,
// since relative z:auto establishes no context.
#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_cross_parent_z_bubbles_through_two_non_context_levels() {
    let mut rt = setup_runtime(
        r#"<!doctype html><html><body style="margin:0">
    <div id="A" style="position:relative;height:100px"><div id="M" style="position:relative;height:50px"><div id="B" style="position:absolute;z-index:5;top:0;left:0;width:100px;height:100px;background:red"></div></div></div>
    <div id="C" style="position:relative;z-index:1;margin-top:-100px;width:100px;height:100px;background:blue"></div>
    </body></html>"#,
    );
    let script = r#"() => {
          const el = document.elementFromPoint(50, 50);
          return el ? el.id : 'none';
        }"#;
    let out = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(out.value.unwrap(), serde_json::json!("B"));
}

// A context intermediate consumes the hoist: opacity<1 makes A its own
// stacking context, so B(z:5) is confined inside A's group and the
// later C(z:1) out-paints the whole group — Chrome parity.
#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_cross_parent_z_context_intermediate_consume() {
    let mut rt = setup_runtime(
        r#"<!doctype html><html><body style="margin:0">
    <div id="A" style="position:relative;height:100px;opacity:0.5"><div id="B" style="position:absolute;z-index:5;top:0;left:0;width:100px;height:100px;background:red"></div></div>
    <div id="C" style="position:relative;z-index:1;margin-top:-100px;width:100px;height:100px;background:blue"></div>
    </body></html>"#,
    );
    let script = r#"() => {
          const el = document.elementFromPoint(50, 50);
          return el ? el.id : 'none';
        }"#;
    let out = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(out.value.unwrap(), serde_json::json!("C"));
}

// DOCUMENTED DEVIATION (narrow slice): a clipping intermediate pins the
// z child locally (a hoisted range would tear the Clip/PopClip pair).
// Chrome hoists AND keeps the clip, answering B here; diting answers C
// until clip-pair surgery makes hoisting clip-aware — flip this pin
// consciously when that lands.
#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_cross_parent_z_clip_intermediate_pins_locally() {
    let mut rt = setup_runtime(
        r#"<!doctype html><html><body style="margin:0">
    <div id="A" style="position:relative;height:100px;overflow:hidden"><div id="B" style="position:absolute;z-index:5;top:0;left:0;width:100px;height:100px;background:red"></div></div>
    <div id="C" style="position:relative;z-index:1;margin-top:-100px;width:100px;height:100px;background:blue"></div>
    </body></html>"#,
    );
    let script = r#"() => {
          const el = document.elementFromPoint(50, 50);
          return el ? el.id : 'none';
        }"#;
    let out = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(out.value.unwrap(), serde_json::json!("C"));
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_pins_inside_element_scroller() {
    // sticky v2's composition case: a sticky header inside a scrolled
    // container reads TOTAL zero (its own +400 pin cancels the
    // scroller's -400 base) yet must stay pinned at the port top in
    // gBCR — the delta-liveness semantics apply_sticky_to_items pins.
    let mut rt = setup_runtime(
        r#"<html><body><div id="box" style="height:300px; overflow:auto; margin:0; padding:0; border:none"><div id="head" style="position:sticky; top:0; height:30px">H</div><div id="tall" style="height:1500px">filler</div></div></body></html>"#,
    );
    let script = r#"async () => {
            const box = document.getElementById('box');
            const head = document.getElementById('head');
            const tall = document.getElementById('tall');
            const boxTop = box.getBoundingClientRect().top;
            const rest = head.getBoundingClientRect().top;
            const tallBefore = tall.getBoundingClientRect().top;
            box.scrollTop = 400;
            await new Promise(r => setTimeout(r, 10));
            const mid = box.scrollTop;
            const stuck = head.getBoundingClientRect().top;
            const tallAfter = tall.getBoundingClientRect().top;
            box.scrollTop = 0;
            await new Promise(r => setTimeout(r, 10));
            const back = head.getBoundingClientRect().top;
            return [
                mid,
                Math.abs(stuck - boxTop) < 0.5,
                Math.abs((tallBefore - tallAfter) - 400) < 0.5,
                Math.abs(back - rest) < 0.5,
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([400, true, true, true]),
        "head stays at the scroller's port top, tall travels with the scroll, reset restores in-flow"
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_table_row_pins_under_root_scroll() {
    // blitz#887 收官口子：boxless 表格行 sticky。我们的行 wrapper 在
    // node_map 里就键在 tr 的 DOM id 上（批57 表格引擎），rects 天然
    // 有行盒——sticky 全机制（shift map + paint span + 命中）直接继承。
    // 钉死三面：gBCR 黏住、cell 后代同步黏、client 命中落在黏住的行上。
    let mut rt = setup_runtime(
        r#"<html><body><div style="height:100px"></div><table style="border-collapse:collapse"><thead><tr id="head" style="position:sticky; top:0; height:30px; background:#ccc"><th id="hc" style="width:100px">H</th></tr></thead><tbody><tr><td><div style="height:4000px">r1</div></td></tr></tbody></table><div style="height:200px"></div></body></html>"#,
    );
    let script = r#"async () => {
            const head = document.getElementById('head');
            const cell = document.getElementById('hc');
            const pos = getComputedStyle(head).position;
            const rest = head.getBoundingClientRect().top;
            window.scrollTo(0, 300);
            await new Promise(r => setTimeout(r, 10));
            const stuck = head.getBoundingClientRect().top;
            const cellStuck = cell.getBoundingClientRect().top;
            const hit = document.elementFromPoint(10, 5);
            const hitId = hit ? (hit.id || hit.tagName) : 'null';
            window.scrollTo(0, 0);
            await new Promise(r => setTimeout(r, 10));
            const back = head.getBoundingClientRect().top;
            return [
                pos,
                Math.round(rest),
                Math.round(stuck),
                Math.round(cellStuck),
                hitId,
                Math.round(back),
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["sticky", 108, 0, 0, "hc", 108]),
        "row sticks at top:0 (client-space gBCR == 0), cell inherits the shift, hit lands on the stuck header, reset restores"
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_table_header_group_and_cell_pin() {
    // Same boxless family, two more members: sticky on thead (row group,
    // Chrome sticks the whole group) and sticky on a th cell (frozen
    // header cell). Both ride the same node_map keying as the row.
    let mut rt = setup_runtime(
        r#"<html><body><div style="height:100px"></div><table style="border-collapse:collapse"><thead id="hg" style="position:sticky; top:0"><tr style="height:30px"><th id="c1" style="width:100px">H</th></tr></thead><tbody><tr><td><div style="height:4000px">r1</div></td></tr></tbody></table><div style="height:200px"></div></body></html>"#,
    );
    let script = r#"async () => {
            const group = document.getElementById('hg');
            const cell = document.getElementById('c1');
            window.scrollTo(0, 300);
            await new Promise(r => setTimeout(r, 10));
            return [
                Math.round(group.getBoundingClientRect().top),
                Math.round(cell.getBoundingClientRect().top),
                (document.elementFromPoint(10, 5) || {}).id || 'null',
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([0, 0, "c1"]),
        "group sticks at top:0 (client-space gBCR), cell inherits the shift, hit lands on the stuck header cell"
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_sticky_table_row_z_index_outranks_later_positioned() {
    // The z≠0 half of the group look-through: a sticky row carrying
    // z-index:5 inside a STATIC thead must ride the hoisted band at z=5
    // — a fixed z:2 overlay on top of the viewport must NOT cover the
    // stuck header. (Group classified z-auto would sit in the middle
    // band and lose to the overlay's positive band.)
    let mut rt = setup_runtime(
        r#"<html><body><div style="height:100px"></div><table style="border-collapse:collapse"><thead><tr id="head" style="position:sticky; top:0; z-index:5; height:30px; background:#ccc"><th id="hc" style="width:100px">H</th></tr></thead><tbody><tr><td><div style="height:4000px">r1</div></td></tr></tbody></table><div id="over" style="position:fixed; top:0; left:0; width:100%; height:100px; z-index:2">over</div></body></html>"#,
    );
    let script = r#"async () => {
            window.scrollTo(0, 300);
            await new Promise(r => setTimeout(r, 10));
            const head = document.getElementById('head');
            const hit = document.elementFromPoint(10, 5);
            return [
                Math.round(head.getBoundingClientRect().top),
                hit ? (hit.id || hit.tagName) : 'null',
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([0, "hc"]),
        "sticky z:5 row in a static thead outranks the fixed z:2 overlay in hit-testing (client-space stuck top == 0)"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_layout_rect_returns_real_geometry_through_get_bounding_client_rect() {
    // Task #108: getBoundingClientRect serves diting-layout geometry, not
    // the synthetic hit-test grid. Two block siblings must land at
    // distinct stacked y positions with full-content width — the grid
    // scatter would give them unrelated (x, y) cells of 100x20.
    let mut rt = setup_runtime(
        "<html><body><div id=\"a\">alpha</div><div id=\"b\">bravo</div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const a = document.getElementById("a").getBoundingClientRect();
            const b = document.getElementById("b").getBoundingClientRect();
            return [a.x, a.y, a.width, a.height, b.y > a.y + a.height - 1, b.x === a.x,
                    a.width === innerWidth - 16];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(8),
        "block x at body's 8px UA content edge"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(8),
        "first block at body content top"
    );
    // Width agrees with the PERSONA viewport minus body's 8px UA margins
    // (set_viewport publishes it to the layout layer; the old hard-coded
    // 1920 broke whenever the persona pool drew a narrower screen).
    assert_eq!(
        parts[6],
        serde_json::json!(true),
        "block spans viewport width minus body margins"
    );
    assert_eq!(
        parts[4],
        serde_json::json!(true),
        "second block stacks below first"
    );
    assert_eq!(
        parts[5],
        serde_json::json!(true),
        "siblings share left edge"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_layout_rect_cache_invalidates_on_mutation() {
    // A node allocation bumps the tree epoch; the next rect read must
    // reflect the mutated tree (the inserted sibling pushes #b down),
    // not the memoized pre-insert layout.
    let mut rt = setup_runtime(
        "<html><body><div id=\"a\" style=\"height:50px\">a</div><div id=\"b\">b</div></body></html>",
    );
    let before = rt
        .evaluate("document.getElementById('b').getBoundingClientRect().y")
        .unwrap();
    rt.evaluate(
            "const d = document.createElement('div'); d.style.height = '30px'; document.body.insertBefore(d, document.getElementById('b'))",
        )
        .unwrap();
    let after = rt
        .evaluate("document.getElementById('b').getBoundingClientRect().y")
        .unwrap();
    assert_ne!(
        before, after,
        "inserting a 30px block above #b must push it down"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_offset_geometry_matches_real_layout_rect() {
    // offsetWidth/Height/Top/Left and clientWidth/Height read the same
    // taffy-layout source as getBoundingClientRect (rounded): a script
    // sizing a container via offsetWidth (map-lib init) must not see the
    // 100x20 stub while gBCR reports the real box.
    let mut rt = setup_runtime(
        "<html><body><div id=\"box\" style=\"width:300px;height:120px;margin:10px\">x</div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const el = document.getElementById("box");
            const r = el.getBoundingClientRect();
            return [el.offsetWidth === 300, el.offsetHeight === 120,
                    el.offsetWidth === r.width, el.offsetHeight === r.height,
                    el.offsetTop === Math.round(r.y), el.offsetLeft === Math.round(r.x),
                    el.clientWidth === r.width, el.clientHeight === r.height,
                    document.body.clientWidth === innerWidth];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "offsetWidth serves real layout width"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "offsetHeight serves real layout height"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "offsetWidth matches gBCR width"
    );
    assert_eq!(
        parts[3],
        serde_json::json!(true),
        "offsetHeight matches gBCR height"
    );
    assert_eq!(
        parts[4],
        serde_json::json!(true),
        "offsetTop matches gBCR y (document coords)"
    );
    assert_eq!(
        parts[5],
        serde_json::json!(true),
        "offsetLeft matches gBCR x"
    );
    assert_eq!(
        parts[6],
        serde_json::json!(true),
        "clientWidth matches gBCR width"
    );
    assert_eq!(
        parts[7],
        serde_json::json!(true),
        "clientHeight matches gBCR height"
    );
    assert_eq!(
        parts[8],
        serde_json::json!(true),
        "body stays a viewport root"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_offset_parent_is_null_terminated_cssom_chain() {
    // #128: Fusion-style accumulators walk `e = e.offsetParent;
    // while (null !== e && e !== anchor)` — the exit test only accepts
    // null, so an undefined return (the property simply not being
    // reflected) spins into `.offsetTop` of undefined and kills the
    // component mid-mount. Chrome semantics pinned: nearest positioned
    // or table ancestor, body fallback, null for html/body/fixed/
    // display:none chains/detached — never undefined.
    let mut rt = setup_runtime(
        "<html><head><style>#fixed { position: fixed; top: 0; left: 0; width: 50px; height: 20px; }\
             .rel { position: relative; }</style></head>\
             <body><div class=\"rel\" id=\"rel\"><div id=\"plain\"></div></div><div id=\"top\"></div>\
             <div id=\"fixed\"></div><div id=\"hider\" style=\"display:none\"><div id=\"hiddenchild\"></div></div>\
             <table><tr><td id=\"cell\"><span id=\"incell\"></span></td></tr></table></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const byId = (id) => document.getElementById(id);
            // The exact Fusion loop shape: undefined would never exit.
            const walk = (el) => {
                let e = el, hops = 0;
                do { e = e.offsetParent; hops++; } while (null !== e && hops < 50);
                return [e === null, hops];
            };
            const detached = document.createElement("div");
            return [
                walk(byId("plain")),
                byId("plain").offsetParent === byId("rel"),
                byId("top").offsetParent === document.body,
                byId("incell").offsetParent === byId("cell"),
                byId("fixed").offsetParent === null,
                byId("hiddenchild").offsetParent === null,
                document.body.offsetParent === null,
                document.documentElement.offsetParent === null,
                detached.offsetParent === null,
                typeof byId("top").offsetParent,
            ];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!([true, 3]),
        "walk exits at null in ≤3 hops (el→rel→body)"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "nearest positioned ancestor wins"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "static top-level falls back to body"
    );
    assert_eq!(
        parts[3],
        serde_json::json!(true),
        "table cell is an offset parent"
    );
    assert_eq!(parts[4], serde_json::json!(true), "position:fixed has none");
    assert_eq!(
        parts[5],
        serde_json::json!(true),
        "display:none chain has none"
    );
    assert_eq!(parts[6], serde_json::json!(true), "body has none");
    assert_eq!(parts[7], serde_json::json!(true), "html has none");
    assert_eq!(
        parts[8],
        serde_json::json!(true),
        "detached element has none"
    );
    assert_eq!(
        parts[9],
        serde_json::json!("object"),
        "never undefined: Element or null only"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn test_offset_geometry_zero_while_hidden_real_after_show() {
    // CSSOM: offset*/client*/scroll* of a display:none element are 0 —
    // the init-in-hidden-container pattern (map libs size while hidden,
    // re-measure on show) must read 0 while hidden and the real box once
    // shown. Hidden via stylesheet to cover the cascade path, not just
    // inline style; the show write must invalidate the memoized layout
    // run (set_attribute drops it), or the re-measure would serve the
    // stale cached "none".
    let mut rt = setup_runtime(
        "<html><head><style>#h { display: none; }</style></head><body><div id=\"h\" style=\"width:300px;height:80px\">h</div><div id=\"s\">s</div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const el = document.getElementById("h");
            const hidden = [el.offsetWidth, el.offsetHeight, el.clientWidth,
                            el.clientHeight, el.scrollWidth, el.scrollHeight];
            el.style.display = "block";
            const shown = [el.offsetWidth, el.offsetHeight];
            return [hidden, shown, el.offsetWidth === 300];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!([0, 0, 0, 0, 0, 0]),
        "all six box metrics read 0 while display:none"
    );
    assert_eq!(
        parts[1],
        serde_json::json!([300, 80]),
        "unhiding serves the real layout box, not a stale cached none"
    );
    assert_eq!(parts[2], serde_json::json!(true));
}

#[cfg(feature = "screenshot")]
#[test]
fn test_rotated_text_ink_does_not_fake_horizontal_scroll() {
    // blitz#841 transform half: the root ink-extent fold must map
    // bracketed text through its transform. A nowrap rotate(90deg) line
    // is a ~14px-wide vertical stripe on screen; folding its raw LOCAL
    // x + est width instead reports ~8000px of horizontal ink and fakes
    // a scrollable page. The element box is width:100 at left:0, so its
    // mapped stripe (x within ~[43,57]) sits inside every persona
    // viewport — the ONLY thing that can push scrollWidth past
    // innerWidth is the stale untransformed ink fold. 8000px of text
    // overshoots every screen the persona can draw, so the assertion
    // holds relative to whatever viewport it picked. gBCR already
    // serves the mapped AABB (the element-box half of the walk is
    // transform-correct), so the text fold was the only stale source.
    let html = format!(
        "<html><body><div id=\"r\" style=\"position:absolute;left:0px;top:40px;width:100px;height:14px;font-size:10px;white-space:nowrap;transform:rotate(90deg)\">{}</div></body></html>",
        "字".repeat(800)
    );
    let mut rt = setup_runtime(&html);
    let result = rt
        .evaluate(
            r#"
            const r = document.getElementById("r").getBoundingClientRect();
            return [document.documentElement.scrollWidth === window.innerWidth,
                    r.width < 50,
                    document.documentElement.scrollHeight > 7500];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "rotated text must not leak its unrotated width into scrollWidth"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "gBCR keeps serving the mapped (vertical stripe) box"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "the rotated length lands vertically in the mapped box"
    );
}

/// blitz#841 shape: a `position: fixed` element never contributes to the
/// root scrollable overflow — not even when a transform pushes its paint
/// far past the viewport edge. Upstream's viewport became scrollable
/// because the fixed element's (transformed) box entered the overflow
/// walk; Chrome keeps scrollWidth pinned to the viewport for fixed boxes.
#[cfg(feature = "screenshot")]
#[test]
fn test_fixed_transformed_element_does_not_expand_scroll_extent() {
    let mut rt = setup_runtime(
        "<html><body><p>body text</p>\
             <div id=\"fx\" style=\"position:fixed;left:0;top:0;width:50px;height:50px;\
             transform:translateX(2500px)\"></div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            return [document.documentElement.scrollWidth <= innerWidth + 50,
                    document.body.scrollWidth <= innerWidth + 50];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "a fixed box translated past the viewport must not widen the root scroll area"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "nor the body's scroll area — fixed is out of every scroller's flow"
    );
}

/// obscura #722 probe: a plain inline element sharing a line with an atomic
/// inline (inline-block sibling) or a float must still report real gBCR
/// geometry — 0x0 there means the run refused to fold and dropped the box.
#[cfg(feature = "screenshot")]
#[test]
fn inline_next_to_atomic_inline_has_rect() {
    let mut rt = setup_runtime(
        r#"<style>p{margin:0;font-size:16px;line-height:20px}</style>
        <p><span style="display:inline-block;width:50px;height:20px"></span><b id="b">hi</b></p>
        <p><i id="alone">solo</i></p>"#,
    );
    let box_of = |rt: &mut JsRuntime, sel: &str| {
        rt.evaluate(&format!(
            "(function(){{const r=document.querySelector('{}').getBoundingClientRect();\
              return {{w:Math.round(r.width),h:Math.round(r.height),x:Math.round(r.x)}};}})()",
            sel
        ))
        .unwrap()
    };
    let b = box_of(&mut rt, "#b");
    let alone = box_of(&mut rt, "#alone");
    assert!(
        b["w"].as_f64().unwrap_or(0.0) > 0.0 && b["h"].as_f64().unwrap_or(0.0) > 0.0,
        "inline beside an atomic inline reports 0x0: {:?} (control {:?})",
        b,
        alone
    );
    assert!(
        alone["w"].as_f64().unwrap_or(0.0) > 0.0,
        "control inline alone must have a rect"
    );
}

/// #129: page JS can compute a NaN and write `style.left = 'NaNpx'`.
/// The Rust parser drops the declaration (no slot, no layout poison),
/// and the computed face answers the used value from geometry — the
/// inline echo must not read the dropped declaration back out.
#[cfg(feature = "screenshot")]
#[test]
fn computed_box_offsets_never_echo_dropped_nan_decl() {
    let mut rt = setup_runtime(
        "<html><head><style>#o { position: absolute; left: 40px; width: 100px; }</style>\
             </head><body><div id='o'>x</div></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var el = document.getElementById('o');
            var before = getComputedStyle(el).left;
            el.style.left = 'NaNpx';
            var after = getComputedStyle(el).left;
            var rect = el.getBoundingClientRect().left;
            // `left: auto` is legal and DOES reset — back to the static
            // position (body margin 8px), unlike the dropped NaN write.
            el.style.left = 'auto';
            var autoLeft = getComputedStyle(el).left;
            return [before, after, String(rect), autoLeft].join('|');
        "#,
        )
        .unwrap();
    let got = out.as_str().unwrap().to_string();
    assert!(
        !got.contains("NaN"),
        "computed left may never read back a dropped NaN declaration: {got}"
    );
    // The valid declared offset still resolves, and after the rejected
    // write it keeps answering geometry. `auto` then resets — away from
    // 40px to the static position (whose exact arithmetic is the
    // layout layer's, not this face contract).
    let parts: Vec<&str> = got.split('|').collect();
    assert_eq!(parts[0..3].join("|"), "40px|40px|40");
    assert_ne!(parts[3], "40px", "left: auto must reset the offset");
}

#[tokio::test(flavor = "current_thread")]
async fn test_range_client_rects_follow_the_painted_lines() {
    // Chrome truth (headless Chrome, 2026-10-22): a wrapped block's text
    // range yields one rect per line (equal line pitch, shared left edge),
    // a sub-range sits inside its line, a collapsed range is a zero-width
    // caret, selectNode(inline) = the element's per-line boxes PLUS its
    // text rects, and a wrapped inline reports one box per line.
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0;font:16px/24px serif} #w{width:200px;padding:10px}</style></head><body>
              <div id=w>aaa bbb ccc ddd eee fff ggg hhh iii jjj kkk lll mmm nnn ooo ppp qqq</div>
              <p id=p3 style=\"width:300px;margin:0\">short line</p>
              <div id=w2 style=\"width:200px\">start <span id=sp>one two three four five six seven eight nine ten eleven twelve thirteen</span> end</div>
             </body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const r = []; const f = x => Math.round(x * 10) / 10;
                  const t = document.getElementById('w').firstChild;
                  const rg = document.createRange(); rg.selectNodeContents(document.getElementById('w'));
                  const rs = [...rg.getClientRects()];
                  r.push(rs.length >= 3);
                  r.push(rs.every((x, i) => i === 0 || Math.abs((x.y - rs[i - 1].y) - 24) < 0.6));
                  r.push([...new Set(rs.map(x => f(x.x)))].join('/'));
                  const rg2 = document.createRange(); rg2.setStart(t, 4); rg2.setEnd(t, 7); const p = rg2.getClientRects();
                  r.push(p.length + ':' + (p[0].x > 10) + ':' + (p[0].width > 0 && p[0].width < 60));
                  const p3 = document.getElementById('p3').firstChild;
                  const rg5 = document.createRange(); rg5.setStart(p3, 6); rg5.setEnd(p3, 10); const c = rg5.getClientRects();
                  const rg6 = document.createRange(); rg6.setStart(p3, 3); rg6.collapse(true);
                  r.push(c.length + ':' + (c[0].width > 0) + ':' + rg6.getClientRects().length + ':' + f(rg6.getBoundingClientRect().width));
                  const sp = document.getElementById('sp'); const sr = [...sp.getClientRects()];
                  const rg4 = document.createRange(); rg4.selectNode(sp);
                  r.push((sr.length >= 3) + ':' + (rg4.getClientRects().length === 2 * sr.length) + ':' + (sp.getBoundingClientRect().height > 24));
                  r.push(document.getElementById('w').getClientRects().length);
                  return r.join('|'); })()",
            )
            .unwrap();
    assert_eq!(v, "true|true|10|1:true:true|1:true:1:0|true:true:true|1");
}

/// JS-SURFACE-PLAN step 5 (Blitz #1304 cross-check): the hit-test cold path.
/// Before any explicit layout read, elementFromPoint must already answer
/// from on-demand layout, skip display:none / template / head / detached
/// nodes, and — WebIDL `double` — throw TypeError (not answer null) on a
/// missing or non-finite coordinate, exactly like Chrome.
#[test]
fn hit_test_cold_path_and_webidl_double_args() {
    let mut rt = setup_runtime(
        "<html><head><title>t</title><style>#hid{display:none}</style></head><body>\
         <div id='a' style='width:100px;height:50px'>A</div>\
         <div id='hid'><span id='inner'>x</span></div>\
         <template id='tp'><b id='tb'>t</b></template></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
        (function () {
            function msg(f) { try { f(); return 'no-throw'; } catch (e) { return e.constructor.name + ': ' + e.message; } }
            var d = document.createElement('div'); document.body.appendChild(d); d.remove();
            var stack = document.elementsFromPoint(5, 5);
            return JSON.stringify({
                far: document.elementFromPoint(5000, 5000),
                negative: document.elementFromPoint(-1, -1),
                skipped: stack.some(function (e) {
                    return ['hid', 'inner', 'tp', 'tb'].indexOf(e.id) >= 0 ||
                           ['HEAD', 'TITLE', 'STYLE'].indexOf(e.tagName) >= 0 || e === d;
                }),
                endsHtml: stack.length ? stack[stack.length - 1].tagName : null,
                nan: msg(function () { document.elementFromPoint(NaN, 1); }),
                inf: msg(function () { document.elementsFromPoint(1, Infinity); }),
                arity: msg(function () { document.elementFromPoint(1); }),
                ok: msg(function () { document.elementFromPoint('5', '5'); }),
            });
        })()
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert!(v["far"].is_null() && v["negative"].is_null(), "{v}");
    assert_eq!(v["skipped"], false, "{v}");
    assert_eq!(v["endsHtml"], "HTML");
    assert_eq!(
        v["nan"],
        "TypeError: Failed to execute 'elementFromPoint' on 'Document': The provided double value is non-finite."
    );
    assert_eq!(
        v["inf"],
        "TypeError: Failed to execute 'elementsFromPoint' on 'Document': The provided double value is non-finite."
    );
    assert_eq!(
        v["arity"],
        "TypeError: Failed to execute 'elementFromPoint' on 'Document': 2 arguments required, but only 1 present."
    );
    assert_eq!(v["ok"], "no-throw", "numeric strings coerce like Chrome");
}
