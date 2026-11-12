//! 流布局：float/flex/table/行内/anonymous box/checkVisibility
use super::super::*;
use super::common::setup_runtime;

/// Issue #245: content-box px width + percent padding. The style-map
/// carry-over only adds px padding (percent has no CB at style-map
/// time), so `width:400px; padding:4%` in a 500px host used to solve
/// as border-box 400 (content 360) — Chrome keeps the authored 400 as
/// the CONTENT width (border box 440, padding 20/side), and children
/// re-resolve their own percents against the honest 400.
#[test]
fn content_box_px_width_with_percent_padding_sizes_to_chrome() {
    let mut rt = setup_runtime(
        "<html><head><style>body { margin: 0 }\
             #host { width: 500px }\
             #nested { width: 400px; padding: 4% }\
             #kid { padding-top: 25% }\
             </style></head><body>\
             <div id='host'><div id='nested'><div id='kid'>x</div></div></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var n = document.getElementById('nested');
            var kid = document.getElementById('kid');
            var r = n.getBoundingClientRect();
            [getComputedStyle(n).paddingTop,
             getComputedStyle(n).paddingLeft,
             getComputedStyle(kid).paddingTop,
             Math.round(r.width), n.offsetWidth, n.clientWidth,
             Math.round(kid.getBoundingClientRect().width)].join('|');
        "#,
        )
        .unwrap();
    // #nested: padding 4% of the host's 500 = 20/side, authored 400
    // stays the CONTENT width → border box / offsetWidth / clientWidth
    // all 440. #kid: a block fills the content box (400 wide), and its
    // own 25% padding resolves against that 400 → 100px (the old broken
    // basis answered 90 = 25% of 360).
    assert_eq!(
        out.as_str().unwrap(),
        "20px|20px|100px|440|440|440|400",
        "content-box px width + percent padding keeps the authored px as content width"
    );
}

/// obscura#1050 class: nested flex containers sized only by intrinsic
/// content (no explicit heights, `min-height:auto` items) must not
/// collapse to 0 height, and an open `<dialog>` must box its content
/// instead of a hairline. Pins taffy's intrinsic auto-sizing through two
/// nested flex levels plus dialog layout.
#[cfg(feature = "screenshot")]
#[test]
fn nested_flex_autosize_and_open_dialog_do_not_collapse() {
    let mut rt = setup_runtime(
        r#"
            <div id="outer" style="display:flex">
              <div id="mid" style="display:flex;flex-direction:column">
                <div id="leaf" style="height:40px;width:120px"></div>
              </div>
            </div>
            <dialog id="d" open><span id="dspan" style="display:block;height:30px;width:100px">x</span></dialog>
        "#,
    );
    let result = rt
        .evaluate(
            r#"
            const q = (id) => document.getElementById(id).getBoundingClientRect();
            const mid = q('mid'), outer = q('outer'), leaf = q('leaf'), d = q('d');
            return [leaf.height, mid.height, outer.height, d.height, d.width];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(40),
        "leaf keeps its explicit height"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(40),
        "column flex item wraps its content — no 0px collapse"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(40),
        "outer flex row height tracks the column"
    );
    assert!(
        parts[3].as_f64().unwrap() >= 30.0,
        "open dialog boxes its content, got {}",
        parts[3]
    );
    assert!(
        parts[4].as_f64().unwrap() >= 100.0,
        "dialog width tracks content, got {}",
        parts[4]
    );
}

/// Containing-block resolution for out-of-flow boxes (blitz#764 family):
/// an absolute box reparents to the nearest POSITIONED ancestor's padding
/// box (not its DOM parent's flow position), a transformed ancestor is a
/// containing block for BOTH absolute and fixed descendants (CSS
/// Transforms — positioned ancestors don't pin fixed), and fixed with no
/// such ancestor stays viewport-anchored on the ICB.
#[cfg(feature = "screenshot")]
#[test]
fn abspos_containing_block_resolution() {
    let mut rt = setup_runtime(
        r#"<body style="margin:8px">
              <div id="outer" style="position:relative">
                <div id="mid" style="margin-left:100px">
                  <div id="a" style="position:absolute; left:0; top:0">X</div>
                </div>
              </div>
              <div id="tf" style="transform:translateX(50px)">
                <div id="f" style="position:fixed; left:0; top:0">F</div>
              </div>
              <div id="tg" style="transform:translateX(30px)">
                <div><div id="a2" style="position:absolute; left:0; top:0">A2</div></div>
              </div>
              <div id="pos" style="position:relative; margin-left:200px">
                <div id="tr2" style="transform:translateX(10px)">
                  <div id="f3" style="position:fixed; left:0; top:0">F3</div>
                </div>
              </div>
              <div id="f2" style="position:fixed; left:0; top:0">F2</div>
              <div id="outer2" style="position:relative; padding:20px">
                <div style="height:10px"></div>
                <div id="b" style="position:absolute">B</div>
              </div>
              <div id="wrap" style="transform:translate(50px,100px); width:300px; height:400px">
                <div id="pc" style="position:fixed; top:10%; left:20%">PC</div>
                <div id="pw" style="position:fixed; top:0; left:0; width:50%">PW</div>
                <div id="psx" style="position:fixed; top:25%">PSX</div>
                <div id="psy" style="position:fixed; left:50%">PSY</div>
              </div>
              <div id="flt" style="filter:blur(0px); width:300px; height:400px">
                <div id="h1" style="position:fixed; left:0; top:0">H1</div>
              </div>
              <div id="psp" style="perspective:800px; width:300px; height:400px; margin-top:20px">
                <div id="h2" style="position:fixed; top:10%; left:20%">H2</div>
              </div>
            </body>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const g = (id) => {
                const b = document.getElementById(id).getBoundingClientRect();
                return [Math.round(b.x), Math.round(b.y)];
            };
            return {
                a: g('a'),
                f: g('f'),
                a2: g('a2'),
                f3: g('f3'),
                f2: g('f2'),
                b: g('b'),
                pc: g('pc'),
                pw: (() => {
                    const b = document.getElementById('pw').getBoundingClientRect();
                    return [Math.round(b.x), Math.round(b.y), Math.round(b.width)];
                })(),
                psx: g('psx'),
                psy: g('psy'),
                // Host origins read live — the flow slot above them is this
                // test's business, not theirs.
                h1: (() => {
                    const f = g('flt'), h = g('h1');
                    return [h[0] - f[0], h[1] - f[1]];
                })(),
                h2: (() => {
                    const f = g('psp'), h = g('h2');
                    return [h[0] - f[0], h[1] - f[1]];
                })(),
            };
        "#,
        )
        .unwrap();
    let v = result;
    assert_eq!(
        v["a"],
        serde_json::json!([8, 8]),
        "absolute anchors to the positioned ancestor's padding box, not the DOM parent's flow spot"
    );
    assert_eq!(
        v["f"],
        serde_json::json!([58, 8]),
        "fixed under a transformed ancestor anchors to that ancestor (8 + translateX(50))"
    );
    assert_eq!(
        v["a2"],
        serde_json::json!([38, 8]),
        "a transformed (non-positioned) ancestor is the absolute containing block"
    );
    assert_eq!(
        v["f3"],
        serde_json::json!([218, 8]),
        "fixed skips the positioned ancestor but still pins to the farther transformed one"
    );
    assert_eq!(
        v["f2"],
        serde_json::json!([0, 0]),
        "fixed with no transformed ancestor stays viewport-anchored (ICB)"
    );
    assert_eq!(
        v["b"],
        serde_json::json!([28, 38]),
        "auto insets resolve at the static position (body margin + padding + sibling)"
    );
    assert_eq!(
        v["pc"],
        serde_json::json!([118, 198]),
        "percent insets of a fixed box under a transformed ancestor fold against that \
             ancestor (58 + 20%*300, 158 + 10%*400), not the viewport (#223)"
    );
    assert_eq!(
        v["pw"],
        serde_json::json!([58, 158, 150]),
        "a percent WIDTH folds against the transformed containing block too (50% of 300)"
    );
    assert_eq!(
        v["psx"],
        serde_json::json!([58, 258]),
        "the auto axis takes the static position at the TRANSFORMED origin (x=58), the \
             percent axis still folds (158 + 25%*400)"
    );
    assert_eq!(
        v["h1"],
        serde_json::json!([0, 0]),
        "a filter ancestor (even blur(0px)) pins fixed descendants to itself (#316)"
    );
    assert_eq!(
        v["h2"],
        serde_json::json!([60, 40]),
        "a perspective ancestor pins fixed too, percent insets folding against it \
             (20% of 300, 10% of 400)"
    );
    assert_eq!(
        v["psy"],
        serde_json::json!([208, 158]),
        "mirror of psx: left folds (58 + 50%*300), the auto top sits at the transformed \
             origin (158)"
    );
}

/// Float + percentage width + margin (obscura #757 family): a float's
/// percentage width must survive adding a margin (percent or px) — the
/// combination is the Bootstrap 3 offset grid. Chrome keeps every column
/// at 83.333% of the 1600px row regardless of the margin.
#[cfg(feature = "screenshot")]
#[test]
fn float_percentage_width_survives_margin() {
    let mut rt = setup_runtime(
        r#"<style>
              body{margin:0}
              .row{width:1600px}
              .inner{width:370px}
            </style>
            <div class="row"><div id="c1" style="float:left;width:83.33333333%"><div class="inner">1</div></div></div>
            <div class="row"><div id="c2" style="float:left;width:83.33333333%;margin-left:8.33333333%"><div class="inner">2</div></div></div>
            <div class="row"><div id="c3" style="float:left;width:83.33333333%;margin-left:133px"><div class="inner">3</div></div></div>
            <div class="row"><div id="c4" style="width:83.33333333%;margin-left:8.33333333%"><div class="inner">4</div></div></div>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const g = (id) => {
                const b = document.getElementById(id).getBoundingClientRect();
                return [Math.round(b.x), Math.round(b.width)];
            };
            return { c1: g('c1'), c2: g('c2'), c3: g('c3'), c4: g('c4') };
        "#,
        )
        .unwrap();
    let v = result;
    // Since #243 the rect map carries taffy's UNROUNDED sizes, so every
    // column reads the true fractional 1333.33 → Math.round → 1333 —
    // the old 1333/1334 wobble (gBCR = round(right) − round(left) under
    // the integral-coordinate posture, obscura #576) is gone. The
    // property under test stays NO collapse — upstream #757 dropped the
    // column to 420/873 the moment a margin appeared.
    assert_eq!(
        v["c1"],
        serde_json::json!([0, 1333]),
        "bare float percentage width resolves against the row"
    );
    assert_eq!(
        v["c2"],
        serde_json::json!([133, 1333]),
        "percentage margin must not collapse the float's percentage width"
    );
    assert_eq!(
        v["c3"],
        serde_json::json!([133, 1333]),
        "px margin must not collapse the float's percentage width either"
    );
    assert_eq!(
        v["c4"],
        serde_json::json!([133, 1333]),
        "in-flow control: percentage width + percentage margin without float"
    );
}

#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_float_wrap_lines_start_after_the_float_then_return_to_the_edge() {
    // Chrome truth (headless Chrome, 2026-10-22): container 200px, a
    // 50x50 left float, then a p of short words at line-height 20px. The
    // p BOX starts at x=0 and overlaps the float; its line boxes start at
    // 50, 50, 50 (their tops are above the float's bottom), then 0.
    diting_layout::float_flow::force(Some(true));
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0;font:16px/20px serif} p{margin:0}</style></head><body>
              <div id=c style=\"width:200px\"><div style=\"float:left;width:50px;height:50px\"></div><p id=p>word word word word word word word word word word word word word word word word word word word word</p></div>
             </body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const p = document.getElementById('p'); const b = p.getBoundingClientRect();
                  const r = document.createRange(); r.selectNodeContents(p);
                  const lefts = [...r.getClientRects()].map(x => Math.round(x.left));
                  return JSON.stringify([Math.round(b.left), Math.round(b.width), lefts.slice(0, 5), Math.round(b.height) % 20]); })()",
            )
            .unwrap();
    diting_layout::float_flow::force(None);
    assert_eq!(v, "[0,200,[50,50,50,0,0],0]");
}

#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_float_wrap_clearfix_and_clear_siblings() {
    // Chrome truth (headless Chrome, 2026-10-22): a clearfix row holds its
    // floats (height 60 = the taller right float), the next paragraph
    // starts below it, and a `clear:left` sibling drops under the float.
    diting_layout::float_flow::force(Some(true));
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0;font:16px/20px serif}
              .row::after{content:\"\";display:table;clear:both}
              .l{float:left;width:50px;height:40px}.r{float:right;width:30px;height:60px} p{margin:0}</style></head><body>
              <div class=row id=row style=\"width:300px\"><div class=l></div><div class=r></div></div>
              <p id=after>after</p>
              <div id=host style=\"width:300px\"><div class=l></div><div id=cl style=\"clear:left;height:10px\"></div><p id=below>below</p></div>
             </body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const t = id => { const b = document.getElementById(id).getBoundingClientRect(); return [Math.round(b.top), Math.round(b.height)]; };
                  return JSON.stringify({ row: t('row'), after: t('after'), cl: t('cl'), below: t('below'), host: t('host') }); })()",
            )
            .unwrap();
    diting_layout::float_flow::force(None);
    assert_eq!(
        v,
        "{\"row\":[0,60],\"after\":[60,20],\"cl\":[120,10],\"below\":[130,20],\"host\":[80,70]}"
    );
}

#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_float_wrap_inline_elements_take_the_slots_too() {
    // Chrome truth (headless Chrome, 2026-10-22): a paragraph with <b>,
    // <a> and <i> beside a 50x50 left float: box left 0, and the leftmost
    // fragment of each line sits at 50, 50, 50, 0, 0.
    diting_layout::float_flow::force(Some(true));
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0;font:16px/20px serif}</style></head><body>
              <div id=c style=\"width:200px\"><div style=\"float:left;width:50px;height:50px\"></div><p id=p style=\"margin:0\">word word <b>bold</b> word word <a href=\"#\">link text</a> word word word word <i>ital</i> word word word word word word word word word word</p></div>
             </body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const p = document.getElementById('p'); const r = document.createRange(); r.selectNodeContents(p);
                  const by = {}; for (const x of r.getClientRects()) { const k = Math.floor((x.top + 2) / 20); by[k] = Math.min(by[k] ?? 1e9, Math.round(x.left)); }
                  return JSON.stringify({ boxLeft: Math.round(p.getBoundingClientRect().left), lines: Object.keys(by).sort((a, c) => a - c).map(k => by[k]).slice(0, 5) }); })()",
            )
            .unwrap();
    diting_layout::float_flow::force(None);
    assert_eq!(v, "{\"boxLeft\":0,\"lines\":[50,50,50,0,0]}");
}

/// #300 graduation battery (Chrome truth re-pinned 2026-10-23, width-
/// independent): seven realistic faces — a floated IMG, a right float,
/// two same-side floats, a float declared AFTER some text, list items,
/// a nested plain block, and a float with margins — with clear:both
/// separators between cases. Every pinned number is a LEFT, a TOP, a
/// box size or a line COUNT (per-line Range lefts), never a text width:
/// our serif words run ~15% wider than Chrome's, so line widths are not
/// comparable cross-engine while lefts/counts/heights are. Escaped
/// floats (A's img outliving ca by 10px, B's right float by 10px) and
/// the separators clearing below them are part of the pin: cb top=50,
/// cc top=100 come from the clears, not the boxes.
#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_float_wrap_graduation_battery() {
    diting_layout::float_flow::force(Some(true));
    let mut rt = setup_runtime(
        r#"<html><head><style>body{margin:0;font:16px/20px serif} p{margin:0} .f{float:left} .fr{float:right} .sep{clear:both;height:0}</style></head><body>
              <div id=ca style="width:300px"><img id=im src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" style="float:left;width:50px;height:50px"><p id=pa>word word word word word word word word word word word word</p></div>
              <div class=sep></div>
              <div id=cb style="width:300px"><div id=frb class=fr style="width:50px;height:50px"></div><p id=pb>word word word word word word word word word word word word</p></div>
              <div class=sep></div>
              <div id=cc style="width:300px"><div id=f1 class=f style="width:50px;height:40px"></div><div id=f2 class=f style="width:50px;height:60px"></div><p id=pc>word word word word word word word word word word word word</p></div>
              <div class=sep></div>
              <div id=cd style="width:300px"><p id=pd>word word word</p><div class=f id=fd style="width:50px;height:50px"></div><p id=pd2>word word word word word word word word word</p></div>
              <div class=sep></div>
              <div id=ce style="width:300px"><div class=f style="width:50px;height:50px"></div><ul id=ue style="margin:0;padding:0"><li id=li1>word word word</li><li id=li2>word word word</li><li id=li3>word word word</li></ul></div>
              <div class=sep></div>
              <div id=cf style="width:300px"><div class=f style="width:50px;height:70px"></div><div id=inner><p id=pf>word word word word word word word word</p></div></div>
              <div class=sep></div>
              <div id=cg style="width:300px"><div class=f id=fg style="width:50px;height:50px;margin:10px"></div><p id=pg>word word word word word word word word word word</p></div>
             </body></html>"#,
    );
    let v = rt
            .evaluate(
                r#"(() => {
                  const lines = (pid) => { const p = document.getElementById(pid); const r = document.createRange(); r.selectNodeContents(p); return [...r.getClientRects()].map(x => Math.round(x.left)); };
                  const box = (pid) => { const b = document.getElementById(pid).getBoundingClientRect(); return [Math.round(b.left), Math.round(b.top), Math.round(b.width), Math.round(b.height)]; };
                  return JSON.stringify({
                    A: { paLefts: lines('pa'), im: box('im'), ca: box('ca') },
                    B: { pbLefts: lines('pb'), frb: box('frb'), cb: box('cb') },
                    C: { pcLefts: lines('pc'), f1: box('f1'), f2: box('f2'), cc: box('cc') },
                    D: { pdLefts: lines('pd'), pd2Lefts: lines('pd2'), fd: box('fd'), cd: box('cd') },
                    E: { li1Lefts: lines('li1'), li3Lefts: lines('li3'), ce: box('ce') },
                    F: { pfLefts: lines('pf'), inner: box('inner'), cf: box('cf') },
                    G: { pgLefts: lines('pg'), fg: box('fg'), cg: box('cg') }
                  }); })()"#,
            )
            .unwrap();
    diting_layout::float_flow::force(None);
    let got: serde_json::Value =
        serde_json::from_str(v.as_str().expect("battery probe returns JSON")).unwrap();
    let want: serde_json::Value = serde_json::from_str(
            r#"{"A":{"paLefts":[50,50],"im":[0,0,50,50],"ca":[0,0,300,40]},
                "B":{"pbLefts":[0,0],"frb":[250,50,50,50],"cb":[0,50,300,40]},
                "C":{"pcLefts":[100,100,100],"f1":[0,100,50,40],"f2":[50,100,50,60],"cc":[0,100,300,60]},
                "D":{"pdLefts":[0],"pd2Lefts":[50,50],"fd":[0,180,50,50],"cd":[0,160,300,60]},
                "E":{"li1Lefts":[50],"li3Lefts":[50],"ce":[0,230,300,60]},
                "F":{"pfLefts":[50,50],"inner":[0,290,300,40],"cf":[0,290,300,40]},
                "G":{"pgLefts":[70,70],"fg":[10,370,50,50],"cg":[0,360,300,40]}}"#,
        )
        .unwrap();
    assert_eq!(got, want, "float wrap graduation battery vs Chrome 155");
}

#[tokio::test(flavor = "current_thread")]
async fn test_collapsed_display_table_with_only_anonymous_cells_lays_out() {
    // docs.python.org: a `display:table` box (no <table>) with
    // border-collapse:collapse whose children become anonymous cells. The
    // collapsed-border gutters were sized from real cells only, so the
    // anonymous cell indexed past them and every layout panicked (#301).
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0} #t{display:table;border-collapse:collapse;width:200px}</style></head><body>
              <div id=t><div id=c>cell text</div></div><p id=after>after</p></body></html>",
        );
    let v = rt
            .evaluate("(() => { const b = document.getElementById('after').getBoundingClientRect(); return (Math.round(b.width) === innerWidth) + ':' + (b.top > 0); })()")
            .unwrap();
    assert_eq!(v, "true:true");
}

#[tokio::test(flavor = "current_thread")]
async fn test_table_caption_sizes_the_table_by_its_longest_word_only() {
    // Chrome truth (headless Chrome, 2026-10-22): a floated display:table
    // figure with a long display:table-caption is as wide as its content
    // (100), the caption fills that width; a caption holding one long
    // unbreakable word widens the table to that word (292). Wikipedia's
    // thumbnails are exactly this shape (#303).
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0;font:16px/20px serif}</style></head><body>
              <figure id=f style=\"display:table;float:right;margin:0\"><div style=\"width:100px;height:50px\"></div><figcaption id=c style=\"display:table-caption;caption-side:bottom\">This is a fairly long caption sentence that would be much wider than the image if it counted toward the width of the table</figcaption></figure>
              <figure id=g style=\"display:table;float:right;clear:right;margin:0\"><div style=\"width:30px;height:20px\"></div><figcaption id=h style=\"display:table-caption\">Unbreakableextraordinarilylongcaptionword</figcaption></figure>
             </body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const w = id => Math.round(document.getElementById(id).getBoundingClientRect().width);
                  return JSON.stringify([w('f'), w('c'), w('g') > 100 && w('h') === w('g'), getComputedStyle(document.getElementById('c')).display]); })()",
            )
            .unwrap();
    assert_eq!(v, "[100,100,true,\"table-caption\"]");
}

#[tokio::test(flavor = "current_thread")]
async fn test_hidden_children_do_not_cut_the_text_run() {
    // Chrome truth (headless Chrome, 2026-10-22): `<link>`, `<meta>` and a
    // display:none <br> between words build no box, so the paragraph wraps
    // exactly like the same text without them (2 lines at 500px); the
    // engine broke the line at each one (5 lines) - Wikipedia's category
    // <link>s sit inside paragraphs (#304).
    let mut rt = setup_runtime(
            "<html><head><style>body{margin:0;font:16px/20px serif} p{width:500px;margin:0} .h{display:none}</style></head><body>
              <p id=a>alpha beta gamma <link rel=x href=./a> delta epsilon <meta name=y> zeta eta theta <br class=h> iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega</p>
              <p id=b>alpha beta gamma  delta epsilon  zeta eta theta  iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega</p>
             </body></html>",
        );
    let v = rt
            .evaluate("(() => { const h = id => Math.round(document.getElementById(id).getBoundingClientRect().height); return h('a') + ':' + h('b'); })()")
            .unwrap();
    let (a, b) = v.as_str().unwrap().split_once(':').unwrap();
    assert_eq!(a, b, "hidden children must not add lines (a={a}, b={b})");
}

#[tokio::test(flavor = "current_thread")]
async fn test_nested_percent_width_tables_in_rowspan_cells_keep_their_width() {
    // Wikipedia's clade trees: each level is a `td rowspan=2` holding a
    // `width:100%` table. The rowspan placeholder added nothing to the
    // row's intrinsic width, so every level squeezed the next (the
    // innermost table collapsed to 0px, #305). Rowspan on or off, the
    // widths must agree (Chrome: both 232/169/73 with its fonts).
    let table = |rs: &str, ids: &str| {
        format!(
            "<table id=o{ids}><tr><td style=\"white-space:nowrap\">Felidae</td><td {rs}><div><table id=a{ids} style=\"width:100%\"><tr><td style=\"white-space:nowrap\">Pantherinae</td><td {rs}><div><table id=b{ids} style=\"width:100%\"><tr><td style=\"white-space:nowrap\">Jungle cat</td></tr></table></div></td></tr><tr><td></td></tr></table></div></td></tr><tr><td></td></tr></table>"
        )
    };
    let html = format!(
        "<html><head><style>body{{margin:0;font:16px/20px serif}} table{{border-spacing:0;margin:0}} td{{padding:0 4px;vertical-align:top}}</style></head><body>{}{}</body></html>",
        table("rowspan=2", "1"),
        table("", "2")
    );
    let mut rt = setup_runtime(&html);
    let v = rt
            .evaluate("(() => { const w = id => Math.round(document.getElementById(id).getBoundingClientRect().width); return [w('a1') - w('a2'), w('b1') - w('b2'), w('b1') > 60].join(','); })()")
            .unwrap();
    assert_eq!(v, "0,0,true");
}

#[tokio::test(flavor = "current_thread")]
async fn test_auto_inset_absolute_boxes_sit_at_their_static_position() {
    // Chrome truth (headless Chrome, 2026-10-22): an absolute box with
    // top/left auto sits where it would have been in flow - here the top of
    // its static parent - however far down the page that is; the page is
    // 3500 tall (the rotated box's overflow aside). The harvest added the parent chain on top of the
    // containing-block-relative location, so it landed at 2x (6000) and
    // doubled the page (#306: Wikipedia's navbox labels).
    let mut rt = setup_runtime(
            "<html><body style=\"margin:0\"><div style=\"height:3000px\"></div>
              <div id=c1 style=\"height:100px\"><div id=d1 style=\"position:absolute\">abs</div>x</div>
              <div id=c2 style=\"height:100px\"><div id=d2 style=\"position:absolute;transform:rotate(270deg)\">rot</div>x</div>
              <div id=c3 style=\"height:100px;position:relative\"><div id=d3 style=\"position:absolute\">rel</div>x</div>
              <div style=\"height:200px\"></div></body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const t = id => Math.round(document.getElementById(id).getBoundingClientRect().top + scrollY);
                  return [t('d1') - t('c1'), Math.abs(t('d2') - t('c2')) < 80, t('d3') - t('c3'), document.documentElement.scrollHeight < 3700].join(','); })()",
            )
            .unwrap();
    assert_eq!(v, "0,true,0,true");
}

// ── Issue #45 (Playwright actionability) regression tests ────────────────
// Kept at the end of the module so they don't share textual context with
// unrelated test additions in other branches (avoids spurious merge
// conflicts when both this branch and an unrelated bootstrap.js change
// add tests near the start of `mod tests`).

/// Playwright >= 1.25 calls `element.checkVisibility(...)` before every
/// input event; agent snapshot layers (browser-use style) filter on it.
/// Real implementation walks the ancestor chain over computed style —
/// see the contract tests below (issue #45).
#[test]
fn element_check_visibility_is_callable() {
    let mut rt = setup_runtime(r#"<div id="x">x</div>"#);
    let result = rt
        .evaluate("document.getElementById('x').checkVisibility({checkOpacity: true})")
        .unwrap();
    assert_eq!(result, serde_json::json!(true));

    let typeof_method = rt
        .evaluate("typeof document.getElementById('x').checkVisibility")
        .unwrap();
    assert_eq!(typeof_method, serde_json::json!("function"));
}

/// #45: display:none kills visibility by default (no flags needed) —
/// on the element itself and on any ancestor (no box).
#[test]
fn check_visibility_display_none_self_and_ancestor() {
    let mut rt = setup_runtime(
        r#"<div id="hidden" style="display:none">h</div>
               <div style="display:none"><span id="in-hidden">x</span></div>
               <div id="shown">s</div>"#,
    );
    let self_none = rt
        .evaluate("document.getElementById('hidden').checkVisibility()")
        .unwrap();
    assert_eq!(self_none, serde_json::json!(false));

    let ancestor_none = rt
            .evaluate("document.getElementById('in-hidden').checkVisibility({checkOpacity:true,checkVisibilityCSS:true})")
            .unwrap();
    assert_eq!(ancestor_none, serde_json::json!(false));

    let plain = rt
        .evaluate("document.getElementById('shown').checkVisibility()")
        .unwrap();
    assert_eq!(plain, serde_json::json!(true));
}

/// #45: visibility:hidden only fails with checkVisibilityCSS — Chrome
/// parity says the default answer is true (the box still exists).
#[test]
fn check_visibility_css_flag_gates_visibility_chain() {
    let mut rt = setup_runtime(
        r#"<div id="v" style="visibility:hidden">v</div>
               <div style="visibility:hidden"><span id="in-v">x</span></div>"#,
    );
    let default_answer = rt
        .evaluate("document.getElementById('v').checkVisibility()")
        .unwrap();
    assert_eq!(default_answer, serde_json::json!(true));

    let flagged = rt
        .evaluate("document.getElementById('v').checkVisibility({checkVisibilityCSS:true})")
        .unwrap();
    assert_eq!(flagged, serde_json::json!(false));

    // Inherited through the ancestor chain, not just the element.
    let ancestor_hidden = rt
        .evaluate("document.getElementById('in-v').checkVisibility({checkVisibilityCSS:true})")
        .unwrap();
    assert_eq!(ancestor_hidden, serde_json::json!(false));
}

/// #45: the opacity check composes multiplicatively down the ancestor
/// chain — 0.5 × 0.5 stays visible, anything × 0 does not.
#[test]
fn check_visibility_opacity_flag_composes_ancestors() {
    let mut rt = setup_runtime(
        r#"<div style="opacity:0"><span id="in-zero">x</span></div>
               <div style="opacity:0.5"><div style="opacity:0.5"><span id="quarter">x</span></div></div>
               <div style="opacity:0"><span id="in-zero-plain">x</span></div>"#,
    );
    let zero_chain = rt
        .evaluate("document.getElementById('in-zero').checkVisibility({checkOpacity:true})")
        .unwrap();
    assert_eq!(zero_chain, serde_json::json!(false));

    let quarter = rt
        .evaluate("document.getElementById('quarter').checkVisibility({checkOpacity:true})")
        .unwrap();
    assert_eq!(quarter, serde_json::json!(true));

    // Without the flag an opacity:0 ancestor changes nothing.
    let plain = rt
        .evaluate("document.getElementById('in-zero-plain').checkVisibility()")
        .unwrap();
    assert_eq!(plain, serde_json::json!(true));
}

/// #45: a detached element has no box — Chrome answers false regardless
/// of flags.
#[test]
fn check_visibility_detached_element_is_false() {
    let mut rt = setup_runtime(r#"<div id="x">x</div>"#);
    let result = rt
        .evaluate(
            "var d = document.createElement('div'); d.style.color='red'; \
                 d.checkVisibility({checkOpacity:true,checkVisibilityCSS:true})",
        )
        .unwrap();
    assert_eq!(result, serde_json::json!(false));
}

#[tokio::test(flavor = "current_thread")]
async fn taffy_main_rev_flex_percent_padding_and_intrinsic_column() {
    // #216: taffy c17b313 → 22f941e absorbs the upstream flexbox fixes
    // (percentage padding resolved against the container, taffy #1209;
    // cross-clamped intrinsic measurement, taffy #1208), plus our own
    // flex-basis content-box carry-over (the size slots pre-bake padding
    // into border-box taffy sizes; the basis slot didn't, so
    // `flex-basis:50px;padding:20px` sized to 50 instead of Chrome's 90).
    // Chrome oracle: padding:10% in a 400px row is a 40px ring = an
    // 80x80 empty box; basis 50 + padding 20 is 90 tall; a width:100%
    // child clamps at max-width.
    let mut rt = setup_runtime(
        r#"<html><body><div id="row" style="display:flex;width:400px"><div id="pad" style="padding:10%"></div></div><div id="col" style="display:flex;flex-direction:column;width:400px"><div id="it" style="flex-basis:50px;padding:20px"></div></div><div id="clamp" style="display:flex;flex-direction:column;width:300px"><div id="w" style="width:100%;max-width:200px"></div></div></body></html>"#,
    );
    let script = r#"async () => {
            const g = id => { const r = document.getElementById(id).getBoundingClientRect(); return [Math.round(r.width), Math.round(r.height)]; };
            return [g('pad'), Math.round(g('it')[1]), g('w')];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([[80, 80], 90, [200, 0]]),
        "flex % padding resolves against the container; basis carries padding; max-width clamps"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn line_height_normal_persona_and_vertical_mode_gap() {
    // #21 verify-and-pin (battery faces, both verdicts documented here):
    //
    // Face 1 — the 1px line-height diff is the bundled face's persona,
    // not a formula bug. normal = ascent+descent+line_gap from the
    // first face's hhea, which IS Chrome's formula (Times cross-check:
    // 1.1504 × 16 = 18.4 ≈ Chrome's 18). The bundled diting-cjk face
    // carries hhea 1160/-288/0 at upem 1000 → 1.448em → 23.168px at
    // 16px, where Chrome-mac's default PingFang SC is 1.375em → 22px.
    // Re-metricing the default face is a product decision (it moves
    // every text box), so the diff parks as a persona artifact in the
    // same ledger as matchMedia/light-dark. Every declared family also
    // maps to this one face (Times/mono measure the same height);
    // Chrome varies 18/17/15.
    //
    // Face 2 — writing-mode:vertical-rl parses and serves in gCS
    // (fingerprint-first, pages don't gate layout on it) but layout
    // stays horizontal: word leaves carry horizontal advances
    // (build_word_leaves), taffy flows block vertically, paint draws
    // horizontal runs — vertical typesetting is a campaign (upright CJK
    // columns stacking right-to-left, transposed baselines, rotated
    // Latin), not a flip. Chrome's vertical box: width 22 (one column
    // = line-height), content growing down columns. Pinned so a future
    // vertical campaign flips this test consciously.
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
<p id="a" style="margin:0;font-size:16px">abc</p>
<p id="v" style="margin:0;font-size:16px;writing-mode:vertical-rl">abc</p>
<p id="mo" style="margin:0;font-size:16px;font-family:monospace">abc</p>
</body></html>"#,
    );
    let script = r#"async () => {
            const h = id => Math.round(document.getElementById(id).getBoundingClientRect().height * 100) / 100;
            const v = document.getElementById('v').getBoundingClientRect();
            return {
                a: h('a'),
                mo: h('mo'),
                vg: getComputedStyle(document.getElementById('v')).writingMode,
                vh: Math.round(v.height * 100) / 100,
                vfullwidth: v.width > 100,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["a"],
        serde_json::json!(23.17),
        "normal line-height = bundled face hhea (1160-(-288)+0)/1000 × 16 = 23.168; Chrome-mac PingFang gives 22 — persona, not formula"
    );
    assert_eq!(
        v["mo"],
        serde_json::json!(23.17),
        "monospace line box measures the same bundled face (Chrome: 15) — one-face persona"
    );
    assert_eq!(
        v["vg"],
        serde_json::json!("vertical-rl"),
        "writing-mode serves in gCS — fingerprint-first"
    );
    assert_eq!(
        v["vh"],
        serde_json::json!(23.17),
        "vertical-rl still lays out as one horizontal line (Chrome: columns of line-height width); flip together with the vertical campaign"
    );
    assert_eq!(
        v["vfullwidth"],
        serde_json::json!(true),
        "vertical-rl block keeps full-width horizontal flow (Chrome: width collapses to 22)"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn flex_basis_padding_carry_survives_longhand_and_incremental() {
    // #220: the #216 carry-over (basis + main-axis padding on the
    // border-box measure) worked for the one-value shorthand but the
    // issue matrix saw longhand / multi-value padding lose it, with
    // non-deterministic heights across rounds — suspected the rect
    // matching or incremental path, not to_taffy_style (FEEDPROBE proved
    // the feed correct). Probe all padding spellings in one container,
    // then rewrite each item's padding through CSSOM longhands and
    // re-measure — the incremental relayout is where the matrix's
    // two-rounds-different-heights lived.
    let mut rt = setup_runtime(
        r#"<html><body><div id="col" style="display:flex;flex-direction:column;width:400px">
<div id="sh1" style="flex-basis:50px;padding:20px"></div>
<div id="lh_top" style="flex-basis:50px;padding-top:40px"></div>
<div id="lh_bot" style="flex-basis:50px;padding-bottom:20px"></div>
<div id="sh4" style="flex-basis:50px;padding:0 0 20px 0"></div>
<div id="lh2" style="flex-basis:50px;padding-top:40px;padding-bottom:20px"></div>
<div id="sh42" style="flex-basis:50px;padding:40px 0 20px 0"></div>
</div></body></html>"#,
    );
    let script = r#"async () => {
            const h = id => Math.round(document.getElementById(id).getBoundingClientRect().height);
            const first = ['sh1','lh_top','lh_bot','sh4','lh2','sh42'].map(h);
            // Incremental pass: same items, padding rewritten through CSSOM
            // longhands to the SAME values (a no-op rewrite still exercises
            // the incremental match + relayout), then to a new value.
            for (const id of ['sh1','lh_top','lh_bot','sh4','lh2','sh42']) {
                const el = document.getElementById(id);
                const cs = getComputedStyle(el);
                el.style.paddingTop = cs.paddingTop;
                el.style.paddingBottom = cs.paddingBottom;
            }
            const noop = ['sh1','lh_top','lh_bot','sh4','lh2','sh42'].map(h);
            for (const id of ['sh1','lh_top','lh_bot','sh4','lh2','sh42']) {
                document.getElementById(id).style.paddingTop = '30px';
            }
            const cssom = ['sh1','lh_top','lh_bot','sh4','lh2','sh42'].map(h);
            for (const id of ['sh1','lh_top','lh_bot','sh4','lh2','sh42']) {
                const el = document.getElementById(id);
                const cs = getComputedStyle(el);
                el.setAttribute('style', 'flex-basis:50px;padding-top:30px;padding-bottom:' + cs.paddingBottom);
            }
            const attr = ['sh1','lh_top','lh_bot','sh4','lh2','sh42'].map(h);
            return [first, noop, cssom, attr];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    // Chrome oracle, content-box: basis 50 + main-axis (vertical) padding.
    assert_eq!(
        v[0],
        serde_json::json!([90, 90, 70, 70, 110, 110]),
        "initial pass: every padding spelling carries onto the basis: {:?}",
        v[0]
    );
    assert_eq!(
        v[1],
        serde_json::json!([90, 90, 70, 70, 110, 110]),
        "CSSOM no-op rewrite keeps the carry: {:?}",
        v[1]
    );
    // The noop round leaves every item's padding-bottom materialized as
    // a longhand (0px where it was absent), so bottoms are 20/0/20/20/20/20
    // and the 30px tops give 50+30+bottom everywhere.
    assert_eq!(
        v[2],
        serde_json::json!([100, 80, 100, 100, 100, 100]),
        "CSSOM value rewrite re-solves (was the stale-solve hole): {:?}",
        v[2]
    );
    assert_eq!(
        v[3],
        serde_json::json!([100, 80, 100, 100, 100, 100]),
        "setAttribute rewrite agrees with the CSSOM round: {:?}",
        v[3]
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_inert_attribute_writes_keep_the_layout_cache() {
    // obscura#983: a docs theme writing tabindex on every heading used
    // to drop the layout cache per write, so geometry reads alternated
    // with those writes each paid a full re-layout.
    let mut rt = setup_runtime(
        r#"<html><head><style>[data-x] { color: red }</style></head>
               <body><h1 id="h">heading</h1></body></html>"#,
    );
    let force = r#"() => document.getElementById('h').getBoundingClientRect().height"#;
    rt.call_function_on_remote(force, None, &[], true, true)
        .await
        .unwrap();
    let rev0 = rt.with_state(|st| st.layout_rev.get());

    // tabindex is layout-inert and no rule selects on it: no cache drop.
    let write = r#"() => { document.getElementById('h').setAttribute('tabindex', '0'); }"#;
    rt.call_function_on_remote(write, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        rt.with_state(|st| st.layout_rev.get()),
        rev0,
        "tabindex write keeps the cache"
    );
    rt.call_function_on_remote(force, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        rt.with_state(|st| st.layout_rev.get()),
        rev0,
        "geometry read after inert write"
    );

    // Same for data-* when NO rule references the name — but the
    // [data-x] rule in this document does, so that write must drop.
    let write = r#"() => { document.getElementById('h').setAttribute('data-y', '1'); }"#;
    rt.call_function_on_remote(write, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        rt.with_state(|st| st.layout_rev.get()),
        rev0,
        "unreferenced data-* write keeps the cache"
    );
    let write = r#"() => { document.getElementById('h').setAttribute('data-x', '1'); }"#;
    rt.call_function_on_remote(write, None, &[], true, true)
        .await
        .unwrap();
    assert!(
        rt.with_state(|st| st.layout_rev.get()) > rev0,
        "selector-referenced attr invalidates"
    );

    // class always invalidates (selector matching depends on it).
    let rev1 = rt.with_state(|st| st.layout_rev.get());
    let write = r#"() => { document.getElementById('h').setAttribute('class', 'c'); }"#;
    rt.call_function_on_remote(write, None, &[], true, true)
        .await
        .unwrap();
    assert!(
        rt.with_state(|st| st.layout_rev.get()) > rev1,
        "class write invalidates"
    );
}

/// blitz#840 shape (narrowed to what applies without transitions): an
/// out-of-flow element inside an INLINE-LEVEL container must still carry
/// its transform — in gBCR here, and the paint side reads the same
/// resolved matrix. Upstream lost the transform entirely when the subtree
/// was pruned at an inline ancestor; the probe pins the abspos-in-span
/// arrangement plus a trailing sibling, their exact three conditions.
#[cfg(feature = "screenshot")]
#[test]
fn test_abspos_child_in_inline_container_keeps_transform() {
    let mut rt = setup_runtime(
        "<html><body style=\"margin:0\">\
             <span class=\"box\" style=\"position:relative;width:42px;height:22px\">\
             <span id=\"dot\" style=\"position:absolute;top:0;left:2px;width:18px;height:18px;\
             transform:translateX(20px)\"></span></span>\
             <div></div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const dot = document.getElementById("dot");
            const r = dot.getBoundingClientRect();
            return [Math.round(r.x), Math.round(r.width)];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(22),
        "abspos dot at left:2 + translateX(20) reports x=22 through the inline container"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(18),
        "transform does not resize the box"
    );
}

// vertical-align authored lengths/percentages are baseline raises on the
// line (CSS2 §10.8.1): positive lifts, negative drops, % of the element's
// own line-height. Mixed shifts on one line keep the RELATIVE offsets —
// the whole line grows so the extreme baseline still fits.
#[test]
#[cfg(feature = "screenshot")]
fn test_vertical_align_length_percent_shift() {
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
              <p style="margin:0;line-height:40px">
                <span id="plain">ab</span>
                <span id="up" style="vertical-align:10px">ab</span>
                <span id="pct" style="vertical-align:50%">ab</span>
                <span id="down" style="vertical-align:-8px">ab</span>
              </p>
            </body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const g = (id) => document.getElementById(id).getBoundingClientRect();
                    const cs = (id) => getComputedStyle(document.getElementById(id)).verticalAlign;
                    const plain = g('plain').top;
                    return {
                        up: plain - g('up').top,
                        pct: plain - g('pct').top,
                        down: g('down').top - plain,
                        csUp: cs('up'),
                        csPct: cs('pct'),
                        csDown: cs('down'),
                    };
                })())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let close = |key: &str, want: f64| -> bool {
        d[key]
            .as_f64()
            .map(|got| (got - want).abs() < 2.5)
            .unwrap_or(false)
    };
    assert!(
        close("up", 10.0),
        "vertical-align:10px must lift 10px (diag={d})"
    );
    assert!(
        close("pct", 20.0),
        "50% of line-height:40px must lift 20px (diag={d})"
    );
    assert!(
        close("down", 8.0),
        "vertical-align:-8px must drop 8px (diag={d})"
    );
    assert_eq!(d["csUp"].as_str().unwrap(), "10px");
    assert_eq!(d["csPct"].as_str().unwrap(), "50%");
    assert_eq!(d["csDown"].as_str().unwrap(), "-8px");
}

/// Anon-cell batch: children of a table-row box that are not table-cells
/// wrap into ONE anonymous cell per consecutive run (CSS2.2 §17.2.1),
/// claiming a column slot like any td. Static HTML never reaches here —
/// the parser foster-parents bare text out of the table (verified:
/// `<tr>AAA<td>` puts "AAA" on body before the table, Chrome's exact
/// behavior) — so the two real entries are CSS-authored rows
/// (display:table-row with non-cell content) and JS tree mutations.
#[test]
fn test_table_anonymous_cell_synthesis() {
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0;font-size:16px">
              <div style="display:table">
                <div style="display:table-row">AAA<div id="b1" style="display:table-cell">B</div></div>
                <div style="display:table-row"><div id="c2" style="display:table-cell">C</div><div id="d2" style="display:table-cell">D</div></div>
              </div>
              <div id="t2" style="display:table">
                <div style="display:table-row">AA<span>BB</span>CC<div id="x1" style="display:table-cell">x</div></div>
              </div>
              <table><tr id="r3"><td id="y1">Y</td></tr></table>
              <table id="t4"><tr><td id="m1">AA<span>BB</span>CC</td><td id="m2">x</td></tr></table>
            </body></html>"#,
    );
    let v = rt
            .evaluate(
                r#"JSON.stringify((() => {
                    const r = (id) => { const b = document.getElementById(id).getBoundingClientRect();
                                        return { x: b.x, w: b.width }; };
                    const y1Before = r('y1').x;
                    document.getElementById('r3').insertBefore(document.createTextNode('ZZ'), document.getElementById('y1'));
                    return { b1: r('b1'), c2: r('c2'), d2: r('d2'), x1: r('x1'),
                             t2h: document.getElementById('t2').getBoundingClientRect().height,
                             m1: r('m1'), m2x: r('m2').x, t4h: document.getElementById('t4').getBoundingClientRect().height,
                             y1Before, y1After: r('y1').x };
                })())"#,
            )
            .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let (b1x, c2x, d2x) = (
        d["b1"]["x"].as_f64().unwrap(),
        d["c2"]["x"].as_f64().unwrap(),
        d["d2"]["x"].as_f64().unwrap(),
    );
    assert!(
        b1x > c2x + 5.0,
        "the anon cell claims column 0, pushing B right (diag={d})"
    );
    assert!(
        (b1x - d2x).abs() < 1.0,
        "B and D share column 1 across rows (diag={d})"
    );
    assert!(
        d["x1"]["x"].as_f64().unwrap() > 5.0,
        "the inline group claims its column (diag={d})"
    );
    let t2h = d["t2h"].as_f64().unwrap();
    let t4h = d["t4h"].as_f64().unwrap();
    let (x1x, m2x) = (d["x1"]["x"].as_f64().unwrap(), d["m2x"].as_f64().unwrap());
    assert!(
        (t2h - t4h).abs() <= 1.0 && (x1x - m2x).abs() < 1.0,
        "anon cell matches a real cell with identical content (t2h={t2h} t4h={t4h} x1x={x1x} m2x={m2x}, diag={d})"
    );
    assert!(
        t4h < 30.0,
        "the mixed-run cell fits on ONE line at its max-content pin (no fractional-rounding wrap) (t4h={t4h}, diag={d})"
    );
    let (yb, ya) = (
        d["y1Before"].as_f64().unwrap(),
        d["y1After"].as_f64().unwrap(),
    );
    assert!(
        ya > yb + 5.0,
        "JS-inserted text synthesizes a cell on re-layout ({yb} -> {ya})"
    );
}

/// obscura #767 probe: width:calc(100% - 32px) inside a flex subtree must
/// resolve against the flex container's content box (400px → 368), same as
/// the identical block-level control.
#[cfg(feature = "screenshot")]
#[test]
fn calc_width_inside_flex_subtree() {
    let mut rt = setup_runtime(
        r#"<style>
          #flex{display:flex;width:400px}
          #item{width:calc(100% - 32px);height:10px}
          #wrap{width:400px}
          #blk{width:calc(100% - 32px);height:10px}
        </style>
        <div id="flex"><div id="item"></div></div>
        <div id="wrap"><div id="blk"></div></div>"#,
    );
    let w_of = |rt: &mut JsRuntime, sel: &str| {
        rt.evaluate(&format!(
            "Math.round(document.querySelector('{}').getBoundingClientRect().width)",
            sel
        ))
        .unwrap()
    };
    let flex_w = w_of(&mut rt, "#item").as_f64().unwrap_or(0.0);
    let blk_w = w_of(&mut rt, "#blk").as_f64().unwrap_or(0.0);
    assert!(
        (flex_w - 368.0).abs() <= 1.0,
        "calc() width collapsed in flex subtree: {} (block control {})",
        flex_w,
        blk_w
    );
}

/// Batch 99 follow-up probe: the Bootstrap 3 clearfix. A host whose only
/// child is a float must reach the float's height once its ::after runs
/// `content:"";display:table;clear:both` — the pseudo leaf clears below the
/// float zone and the host's height accounts for the pushed-out box.
#[cfg(feature = "screenshot")]
#[test]
fn clearfix_pseudo_clears_float_zone() {
    let mut rt = setup_runtime(
        r#"<style>
          .row::after{content:"";display:table;clear:both}
          .col{float:left;width:50px;height:40px}
        </style>
        <div class="row" id="row"><div class="col"></div></div>
        <div id="plain" style="position:relative"><div class="col"></div></div>"#,
    );
    let h_of = |rt: &mut JsRuntime, sel: &str| {
        rt.evaluate(&format!(
            "Math.round(document.querySelector('{}').getBoundingClientRect().height)",
            sel
        ))
        .unwrap()
    };
    let row_h = h_of(&mut rt, "#row").as_f64().unwrap_or(0.0);
    let plain_h = h_of(&mut rt, "#plain").as_f64().unwrap_or(0.0);
    assert!(
        (row_h - 40.0).abs() <= 1.0,
        "clearfix row collapsed to {} (plain float host {}); ::after clear:both must end the float zone",
        row_h,
        plain_h
    );
    // Posture pin, graduated with the #300 default flip (after the #312
    // comma-list fix made the clearfix ::after load-bearing): Chrome's
    // exact float semantics — a plain host (position:relative creates no
    // BFC) is NOT sized by its floats and collapses to 0; only the
    // clearfix row reaches the float's height. The old engine contained
    // floats unconditionally, which masked dead clearfix rules.
    assert!(
        (plain_h - 0.0).abs() <= 1.0,
        "plain float host must collapse to 0 under Chrome float semantics (got {}); \
         containment belongs to the ::after clear, pinned above",
        plain_h
    );
}

/// #313 eaten faces, Chrome-probed headless (multi-host page): abspos
/// children ride the wrap path — inset box pinned to [0,140], no-inset box
/// at its static position [0,160] — while the block sibling overlaps the
/// float (box stays full-width, only the line boxes wrap inside). The hosts
/// are not BFCs, so h1's float escapes and the left floats stack into a
/// horizontal chain (0/120/240); the flex/grid children narrow as
/// independent formatting contexts around BOTH floats in their band
/// ([240, 560]). A ::before with real content no longer routes the host
/// back onto the zone path.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_abspos_flexgrid_pseudo_faces() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .f{float:left;width:120px;height:80px}
          .h{position:relative;width:800px}
          .box{margin:0;height:20px}
          #abs{position:absolute;left:0;top:0;width:140px}
          #static{position:absolute;width:160px}
          #fig{display:flex}
          #grid{display:grid}
          #h4::before{content:"x"}
        </style>
        <div class="h" id="h1"><div class="f"></div>
          <div class="box" id="abs">a</div>
          <div class="box" id="static">s</div>
          <p class="box" id="p1">t</p>
        </div>
        <div class="h" id="h2"><div class="f"></div>
          <div class="box" id="fig"><div>t</div></div>
          <div class="box" id="grid"><div>t</div></div>
          <p class="box" id="p2">t</p>
        </div>
        <div class="h" id="h4"><div class="f"></div>
          <p class="box" id="p4">t</p>
        </div>"#,
    );
    let px = |rt: &mut JsRuntime, sel: &str, prop: &str| {
        rt.evaluate(&format!(
            "Math.round(document.querySelector('{}').getBoundingClientRect().{})",
            sel, prop
        ))
        .unwrap()
        .as_f64()
        .unwrap_or(f64::NAN)
    };
    // Abspos face: out-of-flow children are unaffected by the wrap decision.
    assert!(
        (px(&mut rt, "#abs", "left") - 0.0).abs() <= 1.0
            && (px(&mut rt, "#abs", "width") - 140.0).abs() <= 1.0,
        "inset abs child at [{},{}] — insets must keep pinning it",
        px(&mut rt, "#abs", "left"),
        px(&mut rt, "#abs", "width")
    );
    assert!(
        (px(&mut rt, "#static", "left") - 0.0).abs() <= 1.0
            && (px(&mut rt, "#static", "width") - 160.0).abs() <= 1.0,
        "static-position abs child at [{},{}] — no insets means static spot",
        px(&mut rt, "#static", "left"),
        px(&mut rt, "#static", "width")
    );
    assert!(
        (px(&mut rt, "#p1", "left") - 0.0).abs() <= 1.0
            && (px(&mut rt, "#p1", "width") - 800.0).abs() <= 1.0,
        "block sibling at [{},{}] — the box overlaps the float, only its lines wrap",
        px(&mut rt, "#p1", "left"),
        px(&mut rt, "#p1", "width")
    );
    // Flex/grid face: independent formatting contexts narrow beside the
    // floats in their band. The band holds two — h1's escaped float (hosts
    // are not BFCs) and h2's own, which stacked beside it — so the free
    // slot starts at 240. Width-independent: 2 × float width from the live
    // host width.
    let hw = px(&mut rt, "#h2", "width");
    for sel in ["#fig", "#grid"] {
        assert!(
            (px(&mut rt, sel, "left") - 240.0).abs() <= 1.0
                && (px(&mut rt, sel, "width") - (hw - 240.0)).abs() <= 1.0,
            "{} at [{},{}] — must narrow into the free band [240, {}]",
            sel,
            px(&mut rt, sel, "left"),
            px(&mut rt, sel, "width"),
            hw - 240.0
        );
    }
    assert!(
        (px(&mut rt, "#p2", "left") - 0.0).abs() <= 1.0
            && (px(&mut rt, "#p2", "width") - hw).abs() <= 1.0,
        "block sibling after flex at [{},{}] — stays full-width",
        px(&mut rt, "#p2", "left"),
        px(&mut rt, "#p2", "width")
    );
    // Generated-content face: the host with a real-content ::before wraps
    // like any plain host.
    assert!(
        (px(&mut rt, "#p4", "left") - 0.0).abs() <= 1.0
            && (px(&mut rt, "#p4", "width") - px(&mut rt, "#h4", "width")).abs() <= 1.0,
        "p under ::before host at [{},{}] — generated content must not route to the zone",
        px(&mut rt, "#p4", "left"),
        px(&mut rt, "#p4", "width")
    );
}

/// #313 center/right hosts of bare text wrap: the promote is suppressed
/// (floats only exist in taffy's block algorithm) and float_lines shifts
/// each line inside its float slot. Width-independent pins: a line beside
/// the float centers in [float_w, host_w], a line below it centers in the
/// full width, right-aligned lines end at the host's right edge, and the
/// float itself stays pinned left. A center host whose inline content is
/// one plain inline element mounts directly (#314) and centers the same
/// way; hosts with a mixed run keep the zone posture — lines never overlap
/// the float.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_center_right_hosts() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;font:16px/20px monospace}
          .f{float:left;width:100px;height:60px;background:#fa0}
        </style>
        <div class="h" id="c" style="text-align:center"><div class="f" id="fc"></div>the quick brown fox jumps over the lazy dog again</div>
        <div class="h" id="r" style="text-align:right"><div class="f" id="fr"></div>the quick brown fox jumps over the lazy dog again</div>
        <div class="h" id="s" style="text-align:center"><div class="f" id="fs"></div><span>the quick brown fox jumps over the lazy dog again</span></div>
        <div class="h" id="l"><div class="f" id="fl"></div><span>the quick brown fox jumps over the lazy dog again</span></div>"#,
    );
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<(f64, f64, f64)> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{
                  const t = [...document.querySelector('{}').childNodes].find(n => n.nodeType === 3);
                  const r = document.createRange();
                  r.setStart(t, 0); r.setEnd(t, t.textContent.length);
                  return JSON.stringify([...r.getClientRects()].map(b=>[Math.round(b.left),Math.round(b.top),Math.round(b.width)]));}})()"#,
                sel
            ))
            .unwrap();
        let s = v.as_str().unwrap_or("[]");
        serde_json::from_str::<Vec<Vec<f64>>>(s)
            .unwrap_or_default()
            .into_iter()
            .map(|x| (x[0], x[1], x[2]))
            .collect()
    };
    let left_of = |rt: &mut JsRuntime, sel: &str| {
        rt.evaluate(&format!(
            "Math.round(document.querySelector('{}').getBoundingClientRect().left)",
            sel
        ))
        .unwrap()
        .as_f64()
        .unwrap_or(f64::NAN)
    };
    // The host's text-align never moves the floats themselves.
    assert!(
        (left_of(&mut rt, "#fc") - 0.0).abs() <= 1.0,
        "center host float left {}",
        left_of(&mut rt, "#fc")
    );
    // Band-relative centering: lines beside the 100px float center in
    // [100, 400], lines below it (top ≥ 60) center in [0, 400].
    let host_top = rt
        .evaluate("Math.round(document.querySelector('#c').getBoundingClientRect().top)")
        .unwrap()
        .as_f64()
        .unwrap_or(0.0);
    for (l, t, w) in lines(&mut rt, "#c") {
        let band = if t - host_top < 60.0 {
            (100.0, 400.0)
        } else {
            (0.0, 400.0)
        };
        let expect = band.0 + (band.1 - band.0 - w) / 2.0;
        assert!(
            (l - expect).abs() <= 1.5,
            "center line at left {} width {} must sit at {} in band {:?}",
            l,
            w,
            expect,
            band
        );
    }
    // Right alignment: every line ends at the host's right edge.
    for (l, _t, w) in lines(&mut rt, "#r") {
        assert!(
            (l + w - 400.0).abs() <= 1.5,
            "right line at [{},{}] must end at 400",
            l,
            l + w
        );
    }
    // #314: a lone plain inline element mounts directly beside the float,
    // so the span host wraps too — same band-relative centering as the
    // bare-text host. Floats escape into the shared context and chain, so
    // the band's left edge is read off the laid-out float, not assumed.
    let span_host_top = rt
        .evaluate("Math.round(document.querySelector('#s').getBoundingClientRect().top)")
        .unwrap()
        .as_f64()
        .unwrap_or(0.0);
    let span_float_left = left_of(&mut rt, "#fs");
    for (l, t, w) in lines(&mut rt, "#s span") {
        let band = if t - span_host_top < 60.0 {
            (span_float_left + 100.0, 400.0)
        } else {
            (0.0, 400.0)
        };
        let expect = band.0 + (band.1 - band.0 - w) / 2.0;
        assert!(
            (l - expect).abs() <= 1.5,
            "span center line at left {} width {} must sit at {} in band {:?}",
            l,
            w,
            expect,
            band
        );
    }
    // The left-aligned span host (#314's original face: span text used to
    // ride the wrapper as one max-content atom straight across the float):
    // beside the float the lines start at its right edge, below it at 0.
    let l_host_top = rt
        .evaluate("Math.round(document.querySelector('#l').getBoundingClientRect().top)")
        .unwrap()
        .as_f64()
        .unwrap_or(0.0);
    let l_float_left = left_of(&mut rt, "#fl");
    for (l, t, _w) in lines(&mut rt, "#l span") {
        let expect = if t - l_host_top < 60.0 {
            l_float_left + 100.0
        } else {
            0.0
        };
        assert!(
            (l - expect).abs() <= 1.5,
            "span line at left {} must sit at {}",
            l,
            expect
        );
    }
}

/// Float widths of every authored shape ride the wrap path (#313): px/%
/// as-is, em/rem folded at cascade, calc repaired between two solves (the
/// repair busts the layout cache when the fixup is a float — taffy's
/// cache key can't see a sibling float's new collision extent, so the
/// text leaf would keep slots wrapped against the old extent and paint
/// across the repaired box), sizing keywords measured at build. Chrome
/// truth (headless, 2026-10-23): calc(25%+40px) = 140 in a 400px host,
/// min-content of "FFF" = 29; lines beside sit at the float's right edge
/// and the first line below returns to the host's left edge.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_calc_and_keyword_widths() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;font:16px/20px monospace}
          .f{float:left;height:40px;background:#fa0}
        </style>
        <div class="h" id="a"><div class="f" id="fa" style="width:calc(25% + 40px)"></div><p style="margin:0" id="pa">the quick brown fox jumps over the lazy dog again and again more text to force a fourth line here yes</p></div>
        <div class="h" id="b"><div class="f" id="fb" style="width:min-content">FFF</div><p style="margin:0" id="pb">the quick brown fox jumps over the lazy dog again and again more text to force a fourth line here yes</p></div>"#,
    );
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<Vec<f64>> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const p=document.querySelector('{sel}');const r=document.createRange();r.selectNodeContents(p);return JSON.stringify([...r.getClientRects()].map(b=>[Math.round(b.left),Math.round(b.top)]).slice(0,4));}})()"#
            ))
            .unwrap();
        serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default()
    };
    let num =
        |rt: &mut JsRuntime, expr: &str| rt.evaluate(expr).unwrap().as_f64().unwrap_or(f64::NAN);
    // calc(25% + 40px) in a 400px host = 140 (Chrome: 140).
    assert!(
        (num(
            &mut rt,
            "Math.round(document.getElementById('fa').getBoundingClientRect().width)"
        ) - 140.0)
            .abs()
            <= 1.0
    );
    let la = lines(&mut rt, "#pa");
    assert!(la.len() >= 3, "calc float text wraps to at least 3 lines");
    assert!(
        (la[0][0] - 140.0).abs() <= 1.0,
        "first line sits at the calc float's right edge, got {}",
        la[0][0]
    );
    // The 40px float spans two lines; the third returns to the left edge.
    assert!(
        (la[2][0] - 0.0).abs() <= 1.0,
        "line below the calc float returns to the edge, got {}",
        la[2][0]
    );
    // min-content of "FFF" = 29 (Chrome: 29).
    assert!(
        (num(
            &mut rt,
            "Math.round(document.getElementById('fb').getBoundingClientRect().width)"
        ) - 29.0)
            .abs()
            <= 1.0
    );
    let lb = lines(&mut rt, "#pb");
    assert!(
        (lb[0][0] - 29.0).abs() <= 1.0,
        "first line sits at the min-content float's right edge, got {}",
        lb[0][0]
    );
    assert!(
        (lb[2][0] - 0.0).abs() <= 1.0,
        "line below the min-content float returns to the edge, got {}",
        lb[2][0]
    );
}

/// nowrap/pre runs beside floats (#313): a nowrap line stays on the
/// float's line — it starts at the band and overflows right instead of
/// dropping below — and a pre line below the float returns to the edge
/// (Chrome parity, 2026-10-23). A center host of bare nowrap text centers
/// its single line inside the band.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_nowrap_pre_lines() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;height:60px;font:16px/20px monospace}
          .f{float:left;width:100px;height:40px;background:#fa0}
          p{margin:0}
        </style>
        <div class="h" id="a"><div class="f"></div><p style="white-space:nowrap">abc</p></div>
        <div class="h" id="b"><div class="f"></div><p style="white-space:nowrap">the quick brown fox jumps over the lazy dog again and again more</p></div>
        <div class="h" id="c"><div class="f"></div><p style="white-space:pre">one two
three four five
six</p></div>
        <div class="h" id="d"><div class="f"></div><p><span style="white-space:nowrap">abc def</span></p></div>
        <div class="h" id="f"><div class="f"></div><p style="white-space:nowrap;text-align:center">abc</p></div>
        <div class="h" id="ref"><p style="white-space:nowrap">abc</p></div>"#,
    );
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<Vec<f64>> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const p=document.querySelector('{sel}');const r=document.createRange();r.selectNodeContents(p);return JSON.stringify([...r.getClientRects()].filter(b=>b.width>1).map(b=>[Math.round(b.left*10)/10,Math.round(b.top*10)/10,Math.round(b.width*10)/10]));}})()"#
            ))
            .unwrap();
        serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default()
    };
    let num =
        |rt: &mut JsRuntime, expr: &str| rt.evaluate(expr).unwrap().as_f64().unwrap_or(f64::NAN);
    // nowrap beside a float: one line, at the band start, as wide as the
    // same text without a float (no fold into the band).
    let la = lines(&mut rt, "#a p");
    let wref = lines(&mut rt, "#ref p")[0][2];
    assert_eq!(
        la.len(),
        1,
        "nowrap beside a float stays one line, got {:?}",
        la
    );
    assert!(
        (la[0][0] - 100.0).abs() <= 1.0,
        "nowrap line starts at the float's edge, got {}",
        la[0][0]
    );
    assert!(
        (la[0][2] - wref).abs() <= 1.0,
        "nowrap width is the unwrapped width, got {} vs {}",
        la[0][2],
        wref
    );
    // A line too wide for the band overflows right — it does not drop
    // below the float.
    let lb = lines(&mut rt, "#b p");
    assert_eq!(
        lb.len(),
        1,
        "a wide nowrap run stays on the float's line, got {:?}",
        lb
    );
    assert!(
        (lb[0][0] - 100.0).abs() <= 1.0,
        "wide nowrap line starts at the band, got {}",
        lb[0][0]
    );
    assert!(
        lb[0][0] + lb[0][2] > 401.0,
        "wide nowrap overflows the host, right edge {}",
        lb[0][0] + lb[0][2]
    );
    let btop = num(
        &mut rt,
        "document.getElementById('b').getBoundingClientRect().top",
    );
    assert!(
        lb[0][1] - btop < 20.0,
        "the nowrap line is beside the float, not below: {}",
        lb[0][1] - btop
    );
    // pre: each hard line takes its band; below the float it returns.
    let lc = lines(&mut rt, "#c p");
    let lefts: Vec<f64> = lc.iter().map(|l| l[0]).collect();
    assert_eq!(
        lc.len(),
        3,
        "pre breaks one line per newline, got {:?}",
        lefts
    );
    assert!(
        lc.iter()
            .zip([100.0, 100.0, 0.0])
            .all(|(l, e)| (l[0] - e).abs() <= 1.0),
        "pre lines band/band/edge, got {:?}",
        lefts
    );
    // A nowrap inline element rides the wrapper atom into the band. The
    // range reports the atom's box and its inner line box — identical
    // geometry, so every rect must sit at one band start.
    let ld = lines(&mut rt, "#d p");
    assert!(
        !ld.is_empty() && ld.iter().all(|l| (l[0] - 100.0).abs() <= 1.0),
        "nowrap span sits at the band start, got {:?}",
        ld
    );
    // Center host of bare nowrap text: the line centers inside the band.
    let lf = lines(&mut rt, "#f p");
    assert_eq!(lf.len(), 1);
    let expect = 100.0 + (300.0 - lf[0][2]) / 2.0;
    assert!(
        (lf[0][0] - expect).abs() <= 1.5,
        "centered nowrap in the band: {} vs {}",
        lf[0][0],
        expect
    );
}

/// A nowrap/pre inline ELEMENT's box drops below a float it cannot fit
/// (#313): bare nowrap text stays on the float's line and overflows right
/// (the test above), but an unbreakable element box — a lone span mounted
/// directly, `atomic` on its run leaf — walks float_lines' slots past the
/// band, and a pre's later hard lines keep stacking below (Chrome parity,
/// 2026-10-24: the wide span at (0, 40) in left and center hosts, the
/// narrow span at its band start, the pre's lines at 40/60/80 all x=0).
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_nowrap_element_box_drops_below() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;height:60px;font:16px/20px monospace}
          .tall{height:120px}
          .f{float:left;width:100px;height:40px;background:#fa0}
          p{margin:0}
          span{white-space:nowrap}
        </style>
        <div class="h" id="s"><div class="f"></div><span>alpha beta gamma delta epsilon zeta eta the</span></div>
        <div class="h" id="sf"><div class="f"></div><span>abc def</span></div>
        <div class="h" id="c" style="text-align:center"><div class="f"></div><span>alpha beta gamma delta epsilon zeta eta the</span></div>
        <div class="h" id="cf" style="text-align:center"><div class="f"></div><span>abc def</span></div>
        <div class="h tall" id="pw"><div class="f"></div><span style="white-space:pre">alpha beta gamma delta epsilon zeta eta wide hard line
two
three</span></div>"#,
    );
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<Vec<f64>> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const h=document.querySelector('{sel}');const ht=h.getBoundingClientRect().top;const el=h.querySelector('span');const r=document.createRange();r.selectNodeContents(el);return JSON.stringify([...r.getClientRects()].filter(b=>b.width>1).map(b=>[Math.round(b.left*10)/10,Math.round((b.top-ht)*10)/10,Math.round(b.width*10)/10]));}})()"#
            ))
            .unwrap();
        serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default()
    };
    // A nowrap span too wide for the band drops below the float (40px
    // tall) instead of overflowing the band in place like bare text.
    for sel in ["#s", "#c"] {
        let l = lines(&mut rt, sel);
        assert_eq!(l.len(), 1, "{}: one unbreakable line, got {:?}", sel, l);
        assert!(
            (l[0][0] - 0.0).abs() <= 1.0,
            "{}: dropped line returns to the edge, got {}",
            sel,
            l[0][0]
        );
        assert!(
            (l[0][1] - 40.0).abs() <= 2.5,
            "{}: dropped line sits below the float, got {}",
            sel,
            l[0][1]
        );
    }
    // A nowrap span that fits stays beside the float — at the band start
    // in a left host, centered inside the band in a center host.
    let lf = lines(&mut rt, "#sf");
    assert_eq!(lf.len(), 1);
    assert!(
        (lf[0][0] - 100.0).abs() <= 1.0,
        "fitting span at the band start, got {}",
        lf[0][0]
    );
    assert!(
        lf[0][1] < 20.0,
        "fitting span stays beside the float, got {}",
        lf[0][1]
    );
    let lcf = lines(&mut rt, "#cf");
    assert_eq!(lcf.len(), 1);
    let expect = 100.0 + (300.0 - lcf[0][2]) / 2.0;
    assert!(
        (lcf[0][0] - expect).abs() <= 1.5,
        "centered fitting span: {} vs {}",
        lcf[0][0],
        expect
    );
    // A pre element's wide first hard line drops, and the later hard
    // lines keep stacking below the float at the edge.
    let lp = lines(&mut rt, "#pw");
    assert_eq!(lp.len(), 3, "pre breaks one line per newline, got {:?}", lp);
    for (i, l) in lp.iter().enumerate() {
        assert!(
            (l[0] - 0.0).abs() <= 1.0,
            "pre line {} at the edge, got {}",
            i,
            l[0]
        );
        let expect = 40.0 + i as f64 * 20.0;
        assert!(
            (l[1] - expect).abs() <= 2.5,
            "pre line {} below the float ({}), got {}",
            i,
            expect,
            l[1]
        );
    }
}

/// A pre element among OTHER inline content (#313): Chrome breaks its
/// newlines as real line breaks in the parent's IFC — lead shares the
/// first hard line, tail continues on the next (Chrome 2026-10-24:
/// hostH=40, two band lines; center/right shift each line inside the
/// band). The wrapper used to fold the pre into one atom; hoist now
/// explodes at hard breaks like `<br>`. Lead/tail may split into per-word
/// rects (engine word leaves); asserts are width-independent.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_pre_mixed_run_breaks() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;font:16px/20px monospace}
          .f{float:left;width:150px;height:40px;background:#fa0}
        </style>
        <div class="h" id="m"><div class="f"></div>lead lead <span style="white-space:pre">alpha
beta gamma</span> tail tail</div>
        <div class="h" id="mc" style="text-align:center"><div class="f"></div>lead lead <span style="white-space:pre">alpha
beta gamma</span> tail tail</div>
        <div class="h" id="mr" style="text-align:right"><div class="f"></div>lead lead <span style="white-space:pre">alpha
beta gamma</span> tail tail</div>"#,
    );
    let v = rt
        .evaluate(
            r##"(()=>{const out={};for(const [k,sel] of [["m","#m"],["mc","#mc"],["mr","#mr"]]){const h=document.querySelector(sel);const ht=h.getBoundingClientRect().top;const R=n=>{const r=document.createRange();r.selectNodeContents(n);return [...r.getClientRects()].filter(b=>b.width>1).map(b=>[Math.round(b.left*10)/10,Math.round((b.top-ht)*10)/10,Math.round(b.width*10)/10]);};const texts=[...h.childNodes].filter(n=>n.nodeType===3&&n.textContent.trim());out[k]={lead:R(texts[0]),sp:R(h.querySelector("span")),tail:R(texts[1]),hostH:h.getBoundingClientRect().height};}return JSON.stringify(out);})()"##,
        )
        .unwrap();
    type Faces = Vec<Vec<f64>>;
    #[allow(clippy::type_complexity)]
    let out: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(v.as_str().unwrap_or("{}")).unwrap_or_default();
    let face = |k: &str, f: &str| -> Faces {
        serde_json::from_value(out[k][f].clone()).unwrap_or_default()
    };
    let host_h = |k: &str| out[k]["hostH"].as_f64().unwrap_or(f64::NAN);
    // Left: lead + first hard line share line 0 in the band; second hard
    // line + tail share line 1 at the band start. Host stays 40.
    let (lead, sp, tail) = (face("m", "lead"), face("m", "sp"), face("m", "tail"));
    assert!(
        (host_h("m") - 40.0).abs() <= 2.5,
        "left hostH two band lines, got {}",
        host_h("m")
    );
    assert!(
        !lead.is_empty() && lead.iter().all(|r| r[0] >= 149.0 && r[1].abs() <= 2.5),
        "lead on line 0 in band, got {:?}",
        lead
    );
    assert_eq!(sp.len(), 2, "pre splits into two hard lines, got {:?}", sp);
    let lead_end = lead.last().map(|r| r[0] + r[2]).unwrap_or(0.0);
    assert!(
        sp[0][0] >= lead_end - 1.0 && sp[0][0] <= lead_end + 12.0 && sp[0][1].abs() <= 2.5,
        "first hard line continues after lead (space gap ok), got {:?} after {:?}",
        sp[0],
        lead
    );
    assert!(
        (sp[1][0] - 150.0).abs() <= 1.5 && (sp[1][1] - 20.0).abs() <= 2.5,
        "second hard line at band start on line 1, got {:?}",
        sp[1]
    );
    assert!(
        !tail.is_empty()
            && tail
                .iter()
                .all(|r| (r[1] - 20.0).abs() <= 2.5 && r[0] >= 149.0),
        "tail on line 1 in band, got {:?}",
        tail
    );
    let sp1_end = sp[1][0] + sp[1][2];
    assert!(
        tail[0][0] >= sp1_end - 1.0 && tail[0][0] <= sp1_end + 12.0,
        "tail continues after the second hard line (space gap ok), got {:?} after {:?}",
        tail[0],
        sp[1]
    );
    // Center: each parent line centers inside the band.
    let (lead, sp, tail) = (face("mc", "lead"), face("mc", "sp"), face("mc", "tail"));
    assert!(
        (host_h("mc") - 40.0).abs() <= 2.5,
        "center hostH, got {}",
        host_h("mc")
    );
    assert_eq!(sp.len(), 2, "center pre two hard lines, got {:?}", sp);
    let l1_left = lead[0][0];
    let l1_right = sp[0][0] + sp[0][2];
    let l1w = l1_right - l1_left;
    let expect1 = 150.0 + (250.0 - l1w) / 2.0;
    assert!(
        (l1_left - expect1).abs() <= 2.5,
        "center line1 in band: {} vs {} (w={})",
        l1_left,
        expect1,
        l1w
    );
    let l2_left = sp[1][0];
    let l2_right = tail.last().map(|r| r[0] + r[2]).unwrap_or(l2_left);
    let l2w = l2_right - l2_left;
    let expect2 = 150.0 + (250.0 - l2w) / 2.0;
    assert!(
        (l2_left - expect2).abs() <= 2.5,
        "center line2 in band: {} vs {} (w={})",
        l2_left,
        expect2,
        l2w
    );
    // Right: each parent line flush right in the band.
    let (_lead, sp, tail) = (face("mr", "lead"), face("mr", "sp"), face("mr", "tail"));
    assert!(
        (host_h("mr") - 40.0).abs() <= 2.5,
        "right hostH, got {}",
        host_h("mr")
    );
    assert_eq!(sp.len(), 2, "right pre two hard lines, got {:?}", sp);
    assert!(
        (sp[0][0] + sp[0][2] - 400.0).abs() <= 2.0 && sp[0][1].abs() <= 2.5,
        "right line1 flush right in band, got {:?}",
        sp[0]
    );
    let tail_right = tail.last().map(|r| r[0] + r[2]).unwrap_or(0.0);
    assert!(
        (tail_right - 400.0).abs() <= 2.0 && tail.iter().all(|r| (r[1] - 20.0).abs() <= 2.5),
        "right line2 flush right in band, got {:?}",
        tail
    );
}

/// #313 flex/grid ITEM hosts wrap beside floats: a definite-width item
/// matches a plain block host (band lefts/tops), and an auto-width item
/// takes the wrap path too — the #315 intrinsic repair sums float +
/// caption the same way it does for floated/abspos hosts (Chrome
/// 2026-10-24: 100px float / 400px definite host → two band lines at
/// x=100, hostH=40; auto-width flex/grid item → hostW = 150 + caption).
/// Glyph tops carry the usual −1.6 bias. Inline-block/flex/grid children
/// of a content-sized host sum via the same repair (#315); plain blocks do not.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_flex_grid_item_hosts() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .row{display:flex;width:600px}
          .grid{display:grid;grid-template-columns:400px 200px;width:600px}
          .gauto{display:grid;grid-template-columns:max-content;width:600px}
          .h{width:400px;font:16px/20px monospace}
          .f{float:left;width:100px;height:40px;background:#fa0}
        </style>
        <div class="row"><div class="h" id="fx"><div class="f"></div>alpha beta gamma delta epsilon zeta eta theta iota kappa</div></div>
        <div class="grid"><div class="h" id="gx"><div class="f"></div>alpha beta gamma delta epsilon zeta eta theta iota kappa</div><div></div></div>
        <div class="row"><div id="stf" style="font:16px/20px monospace"><div class="f" style="width:150px"></div>caption</div></div>
        <div class="gauto"><div id="gstf" style="font:16px/20px monospace"><div class="f" style="width:150px"></div>caption</div></div>
        <div class="p" id="ref" style="font:16px/20px monospace;width:400px">caption</div>"#,
    );
    let dump = |rt: &mut JsRuntime, sel: &str| -> (f64, f64, Vec<Vec<f64>>) {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const h=document.querySelector('{sel}');const ht=h.getBoundingClientRect();const f=h.querySelector('.f').getBoundingClientRect();const r=document.createRange();r.selectNodeContents(h);const lines=[];for(const b of r.getClientRects()){{if(b.width<2)continue;if(Math.abs(b.left-f.left)<1&&Math.abs(b.top-f.top)<1&&Math.abs(b.width-f.width)<1)continue;lines.push([+b.left.toFixed(1),+(b.top-ht.top).toFixed(1),+b.width.toFixed(1)]);}}return JSON.stringify({{hostW:+ht.width.toFixed(1),hostH:+ht.height.toFixed(1),lines}});}})()"#
            ))
            .unwrap();
        let o: serde_json::Value =
            serde_json::from_str(v.as_str().unwrap_or("{}")).unwrap_or_default();
        let lines: Vec<Vec<f64>> = serde_json::from_value(o["lines"].clone()).unwrap_or_default();
        (
            o["hostW"].as_f64().unwrap_or(f64::NAN),
            o["hostH"].as_f64().unwrap_or(f64::NAN),
            lines,
        )
    };
    for sel in ["#fx", "#gx"] {
        let (_w, host_h, lines) = dump(&mut rt, sel);
        assert!(
            (host_h - 40.0).abs() <= 2.5,
            "{sel} hostH two band lines, got {host_h}"
        );
        assert_eq!(
            lines.len(),
            2,
            "{sel} two lines beside the float, got {lines:?}"
        );
        assert!(
            lines.iter().all(|r| (r[0] - 100.0).abs() <= 1.5),
            "{sel} every line starts at the band, got {lines:?}"
        );
        assert!(
            (lines[0][1] + 1.6).abs() <= 2.5 && (lines[1][1] - 18.4).abs() <= 2.5,
            "{sel} line tops at 0/−1.6 then 20/18.4, got {lines:?}"
        );
    }
    // #315: auto-width flex/grid-item hosts sum float + caption on the
    // wrap path (same repair as floated/abspos hosts). Text width from a
    // float-free reference — monospace font widths vary by backend.
    let caption_w = {
        let v = rt
            .evaluate(r#"(()=>{const r=document.createRange();r.selectNodeContents(document.querySelector('#ref'));const rs=[...r.getClientRects()].filter(b=>b.width>1);return JSON.stringify(rs[0].width);})()"#)
            .unwrap();
        v.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::NAN)
    };
    for sel in ["#stf", "#gstf"] {
        let (stf_w, stf_h, stf_lines) = dump(&mut rt, sel);
        let expect = 150.0 + caption_w;
        assert!(
            (stf_w - expect).abs() <= 1.5 && (stf_h - 40.0).abs() <= 2.5,
            "{sel} must sum float+caption ({expect}), got w={stf_w} h={stf_h} lines={stf_lines:?}"
        );
        assert_eq!(
            stf_lines.len(),
            1,
            "{sel} caption beside the float, got {stf_lines:?}"
        );
        assert!(
            (stf_lines[0][0] - 150.0).abs() <= 1.5 && (stf_lines[0][1] + 1.6).abs() <= 2.5,
            "{sel} caption at band start, got {stf_lines:?}"
        );
    }
}

/// #315 non-summable-child face: a content-sized host whose non-float child
/// is an inline-block / flex / grid SUMS that box's preferred width with the
/// float (Chrome 2026-10-24: 150 + caption on one line, hostH=40). A plain
/// block child does NOT — the host stays the float alone and the caption
/// drops below (hostW=150, hostH=60). Glyph tops carry the usual −1.6 bias.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_nonsummable_atomic_children() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .f{float:left;width:150px;height:40px;background:#fa0}
          .h{float:left;font:16px/20px monospace}
        </style>
        <div class="h" id="ib"><div class="f"></div><span style="display:inline-block">caption</span></div>
        <div class="h" id="fx"><div class="f"></div><div style="display:flex">caption</div></div>
        <div class="h" id="gx"><div class="f"></div><div style="display:grid">caption</div></div>
        <div class="h" id="ibw"><div class="f"></div><span style="display:inline-block;width:80px">cap</span></div>
        <div class="h" id="blk"><div class="f"></div><div>caption</div></div>
        <div class="h" id="abs" style="position:absolute;left:0;top:200px"><div class="f"></div><span style="display:inline-block">caption</span></div>
        <div style="display:flex;width:600px"><div id="stf" style="font:16px/20px monospace"><div class="f"></div><div style="display:flex">caption</div></div></div>
        <div id="ref" style="font:16px/20px monospace;display:inline-block">caption</div>"#,
    );
    let caption_w = {
        let v = rt
            .evaluate(r#"(()=>{const r=document.createRange();r.selectNodeContents(document.querySelector('#ref'));const rs=[...r.getClientRects()].filter(b=>b.width>1);return JSON.stringify(rs[0].width);})()"#)
            .unwrap();
        v.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::NAN)
    };
    let dump = |rt: &mut JsRuntime, sel: &str| -> (f64, f64, Vec<Vec<f64>>) {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const h=document.querySelector('{sel}');const ht=h.getBoundingClientRect();const f=h.querySelector('.f').getBoundingClientRect();const r=document.createRange();r.selectNodeContents(h);const lines=[];for(const b of r.getClientRects()){{if(b.width<2)continue;if(Math.abs(b.left-f.left)<1&&Math.abs(b.top-f.top)<1&&Math.abs(b.width-f.width)<1)continue;lines.push([+(b.left-ht.left).toFixed(1),+(b.top-ht.top).toFixed(1),+b.width.toFixed(1)]);}}return JSON.stringify({{hostW:+ht.width.toFixed(1),hostH:+ht.height.toFixed(1),lines}});}})()"#
            ))
            .unwrap();
        let o: serde_json::Value =
            serde_json::from_str(v.as_str().unwrap_or("{}")).unwrap_or_default();
        let lines: Vec<Vec<f64>> = serde_json::from_value(o["lines"].clone()).unwrap_or_default();
        (
            o["hostW"].as_f64().unwrap_or(f64::NAN),
            o["hostH"].as_f64().unwrap_or(f64::NAN),
            lines,
        )
    };
    let expect = 150.0 + caption_w;
    for sel in ["#ib", "#fx", "#gx", "#abs", "#stf"] {
        let (w, h, lines) = dump(&mut rt, sel);
        assert!(
            (w - expect).abs() <= 1.5 && (h - 40.0).abs() <= 2.5,
            "{sel} must sum float+atomic ({expect}), got w={w} h={h} lines={lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|r| (r[0] - 150.0).abs() <= 1.5 && (r[1] + 1.6).abs() <= 2.5),
            "{sel} caption at the band, got {lines:?}"
        );
    }
    let (w, h, lines) = dump(&mut rt, "#ibw");
    assert!(
        (w - 230.0).abs() <= 1.5 && (h - 40.0).abs() <= 2.5,
        "#ibw definite ib sums 150+80, got w={w} h={h} lines={lines:?}"
    );
    let (w, h, lines) = dump(&mut rt, "#blk");
    assert!(
        (w - 150.0).abs() <= 1.5 && (h - 60.0).abs() <= 2.5,
        "#blk plain block does not sum — host stays the float, caption drops, got w={w} h={h} lines={lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|r| (r[1] - 40.0).abs() <= 2.5 || (r[1] - 38.4).abs() <= 2.5),
        "#blk caption below the float, got {lines:?}"
    );
}

/// A nowrap element among OTHER inline content (#313 mx face): the run
/// wrapper's inline_flow drops a box no band fits below the float — the
/// walk's clear rescue answers the real float bottom, the fallback slot
/// alone answers the query y — and the content AFTER it continues on the
/// dropped line, wrapping to the next one when it overflows (Chrome
/// 2026-10-24: lead stays in band, the box drops, tail follows). The
/// dropped line aligns like any other: left, centered, right against the
/// full width below the float.
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_nowrap_mixed_run_drop() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;font:16px/20px monospace}
          .f{float:left;width:150px;height:40px;background:#fa0}
          span{white-space:nowrap}
        </style>
        <div class="h" id="m"><div class="f"></div>lead lead <span>alpha beta gamma delta epsilon zeta</span> tail tail</div>
        <div class="h" id="mc" style="text-align:center"><div class="f"></div>lead lead <span>alpha beta gamma delta epsilon zeta</span> tail tail</div>
        <div class="h" id="mr" style="text-align:right"><div class="f"></div>lead lead <span>alpha beta gamma delta epsilon zeta</span> tail tail</div>"#,
    );
    let v = rt
        .evaluate(
            r##"(()=>{const out={};for(const [k,sel] of [["m","#m"],["mc","#mc"],["mr","#mr"]]){const h=document.querySelector(sel);const ht=h.getBoundingClientRect().top;const R=n=>{const r=document.createRange();r.selectNodeContents(n);return [...r.getClientRects()].filter(b=>b.width>1).map(b=>[Math.round(b.left*10)/10,Math.round((b.top-ht)*10)/10,Math.round(b.width*10)/10]);};const texts=[...h.childNodes].filter(n=>n.nodeType===3&&n.textContent.trim());out[k]={lead:R(texts[0]),sp:R(h.querySelector("span")),tail:R(texts[1])};}return JSON.stringify(out);})()"##,
        )
        .unwrap();
    type Faces = Vec<Vec<f64>>;
    #[allow(clippy::type_complexity)]
    let out: std::collections::HashMap<String, std::collections::HashMap<String, Faces>> =
        serde_json::from_str(v.as_str().unwrap_or("{}")).unwrap_or_default();
    let get = |k: &str, f: &str| -> &Faces { out.get(k).and_then(|m| m.get(f)).unwrap() };
    // Left host: lead stays beside the float, the box drops below it, tail
    // follows on the dropped line and wraps to the next one.
    let (lead, sp, tail) = (get("m", "lead"), get("m", "sp"), get("m", "tail"));
    assert!(
        lead.iter().all(|r| r[1] < 20.0 && r[0] >= 149.0),
        "lead stays in the band, got {:?}",
        lead
    );
    assert_eq!(sp.len(), 1, "one unbreakable box, got {:?}", sp);
    assert!(
        sp[0][0] <= 1.0 && (sp[0][1] - 40.0).abs() <= 2.5,
        "box drops below the float, got {:?}",
        sp[0]
    );
    assert_eq!(
        tail.len(),
        2,
        "tail wraps past the dropped line, got {:?}",
        tail
    );
    assert!(
        tail[0][1] >= 37.5 && tail[0][0] > sp[0][0] + sp[0][2] - 2.0,
        "first tail word beside the box, got {:?}",
        tail[0]
    );
    assert!(
        tail[1][0] <= 1.0 && (tail[1][1] - 60.0).abs() <= 2.5,
        "second tail word on its own line at the edge, got {:?}",
        tail[1]
    );
    // Center host: the fitting line centers inside the band, the dropped
    // line and the tail's own line center against the full width.
    let (lead, sp, tail) = (get("mc", "lead"), get("mc", "sp"), get("mc", "tail"));
    let lw = lead[1][0] + lead[1][2] - lead[0][0];
    assert!(
        (lead[0][0] - (150.0 + (250.0 - lw) / 2.0)).abs() <= 2.0,
        "lead centered in band, got {} (line {})",
        lead[0][0],
        lw
    );
    assert!(
        (sp[0][1] - 40.0).abs() <= 2.5,
        "centered box still drops, got {:?}",
        sp[0]
    );
    let dw = (tail[0][0] + tail[0][2]) - sp[0][0];
    assert!(
        (sp[0][0] - (400.0 - dw) / 2.0).abs() <= 2.5,
        "dropped line centered in full width, got {} (line {})",
        sp[0][0],
        dw
    );
    assert!(
        (tail[1][0] - (400.0 - tail[1][2]) / 2.0).abs() <= 2.0,
        "tail's own line centered, got {:?}",
        tail[1]
    );
    // Right host: fitting line flush right in the band, dropped line and
    // the tail's own line flush right in the full width.
    let (lead, _sp, tail) = (get("mr", "lead"), get("mr", "sp"), get("mr", "tail"));
    assert!(
        lead.last().is_some_and(|r| r[0] + r[2] >= 398.5),
        "lead flush right in band, got {:?}",
        lead
    );
    assert!(
        tail[0][0] + tail[0][2] >= 398.5 && (tail[0][1] - 40.0).abs() <= 2.5,
        "first tail flush right on the dropped line, got {:?}",
        tail[0]
    );
    assert!(
        (tail[1][0] - (400.0 - tail[1][2])).abs() <= 1.5 && (tail[1][1] - 60.0).abs() <= 2.5,
        "second tail flush right on its own line, got {:?}",
        tail[1]
    );
}

/// A content-sized wrap host (floated or abs/fixed, auto width) sizes
/// shrink-to-fit with the floats SUMMED (#315): taffy's intrinsic takes
/// the max over block children — the float-figure idiom sized to the
/// float alone — so the repair pass computes
/// `min(max(min_content, avail), max_content)` itself: float margin
/// boxes + the text on one line for max-content (a pre run its widest
/// hard line), the widest float against the longest word for
/// min-content, avail against the containing block (the reparent pass
/// made that the taffy parent for abs/fixed). The same round pinned the
/// §9.5 line-box drop: a band too narrow for a line's first word moves
/// that line below the float (Chrome-truth 2026-10-24).
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_content_sized_host_intrinsic() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0;font:16px/20px monospace}
          .p{width:400px}
          .h{float:left}
          .f{float:left;width:150px;height:40px;background:#fa0}
          .clear{clear:both;height:24px}
        </style>
        <div class="p" id="a"><div class="h"><div class="f"></div>hello</div></div><div class="clear"></div>
        <div class="p" style="width:180px" id="b"><div class="h"><div class="f"></div>hello</div></div><div class="clear"></div>
        <div class="p" style="width:250px" id="c"><div class="h"><div class="f" style="width:100px"></div>alpha beta gamma delta epsilon zeta eta</div></div><div class="clear"></div>
        <div class="p" id="d"><div class="h"><div class="f" style="width:60px"></div><div class="f" style="width:70px"></div>hello</div></div><div class="clear"></div>
        <div class="p" id="m"><div class="h"><div class="f" style="margin:0 5px"></div>hello</div></div><div class="clear"></div>
        <div class="p" id="pre"><div class="h"><div class="f" style="width:100px"></div><span style="white-space:pre">one two
three</span></div></div><div class="clear"></div>
        <div class="p" id="abs"><div class="h" style="position:absolute"><div class="f"></div>hello</div></div>
        <div class="p" id="ref">hello</div>
        <div class="p" id="ref2"><span style="white-space:pre">one two
three</span></div>
        <div id="n" style="width:180px;height:120px"><div class="f"></div>hello</div>
        <div id="n2" style="width:180px;height:120px"><div class="f"></div>hi there</div>"#,
    );
    let w = |rt: &mut JsRuntime, sel: &str| {
        rt.evaluate(&format!(
            "document.querySelector('{sel}').getBoundingClientRect().width"
        ))
        .unwrap()
        .as_f64()
        .unwrap_or(f64::NAN)
    };
    // Text references by Range rects — the box width of a block is its
    // stretch, not the glyphs.
    let text_w = {
        let v = rt
            .evaluate(r#"(()=>{const r=document.createRange();r.selectNodeContents(document.querySelector('#ref'));const rs=[...r.getClientRects()].filter(b=>b.width>1);return JSON.stringify(rs[0].width);})()"#)
            .unwrap();
        v.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::NAN)
    };
    let pre_widest = {
        let v = rt
            .evaluate(r#"(()=>{const r=document.createRange();r.selectNodeContents(document.querySelector('#ref2 span'));const rs=[...r.getClientRects()].filter(b=>b.width>1);return JSON.stringify(Math.max(...rs.map(b=>b.width)));})()"#)
            .unwrap();
        v.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::NAN)
    };
    // The figure sum: float + text on one line (and float + margins + text).
    let wa = w(&mut rt, "#a .h");
    assert!(
        (wa - (150.0 + text_w)).abs() <= 1.0,
        "float + caption on one line: {} vs 150+{}",
        wa,
        text_w
    );
    assert!(
        (w(&mut rt, "#m .h") - (160.0 + text_w)).abs() <= 1.0,
        "float margin box sums: {}",
        w(&mut rt, "#m .h")
    );
    // Two floats sum on the preferred line.
    assert!(
        (w(&mut rt, "#d .h") - (130.0 + text_w)).abs() <= 1.0,
        "two floats sum: {}",
        w(&mut rt, "#d .h")
    );
    // The avail clamp: min(max(min_c, avail), max_c).
    assert!(
        (w(&mut rt, "#b .h") - 180.0).abs() <= 1.0,
        "shrink-to-fit fills the available width: {}",
        w(&mut rt, "#b .h")
    );
    assert!(
        (w(&mut rt, "#c .h") - 250.0).abs() <= 1.0,
        "narrow column clamps: {}",
        w(&mut rt, "#c .h")
    );
    // A pre caption contributes its widest hard line.
    assert!(
        (w(&mut rt, "#pre .h") - (100.0 + pre_widest)).abs() <= 1.0,
        "pre widest hard line sums: {} vs 100+{}",
        w(&mut rt, "#pre .h"),
        pre_widest
    );
    // An abs/fixed host measures avail against its containing block (the
    // viewport here, wider than max-content) — same sum as the float host.
    assert!(
        (w(&mut rt, "#abs .h") - wa).abs() <= 1.0,
        "abspos host sizes to the same sum: {}",
        w(&mut rt, "#abs .h")
    );
    // §9.5: a band too narrow for the line's first word moves the line
    // below the float; a fitting word stays beside it.
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<Vec<f64>> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const h=document.querySelector('{sel}');const fb=h.querySelector('.f').getBoundingClientRect();const ht=h.getBoundingClientRect().top;const r=document.createRange();r.selectNodeContents(h);const byTop={{}};for(const b of r.getClientRects()){{if(b.width<2)continue;if(Math.abs(b.left-fb.left)<1&&Math.abs(b.top-fb.top)<1&&Math.abs(b.width-fb.width)<1)continue;const t=Math.round(b.top-ht);if(!byTop[t])byTop[t]=[b.left,b.right];else{{byTop[t][0]=Math.min(byTop[t][0],b.left);byTop[t][1]=Math.max(byTop[t][1],b.right);}}}}return JSON.stringify(Object.entries(byTop).sort((a,b)=>a[0]-b[0]).map(([t,v])=>[+t,+v[0].toFixed(1),+(v[1]-v[0]).toFixed(1)]));}})()"#
            ))
            .unwrap();
        serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default()
    };
    let ln = lines(&mut rt, "#n");
    assert_eq!(
        ln.len(),
        1,
        "the lone word drops below the narrow band, got {:?}",
        ln
    );
    assert!(
        (ln[0][1] - 0.0).abs() <= 1.0 && (ln[0][0] - 40.0).abs() <= 3.0,
        "dropped word at the edge below the float, got {:?}",
        ln[0]
    );
    let ln2 = lines(&mut rt, "#n2");
    assert_eq!(
        ln2.len(),
        2,
        "fitting word stays, long word drops, got {:?}",
        ln2
    );
    assert!(
        (ln2[0][1] - 150.0).abs() <= 1.0 && ln2[0][0] < 20.0,
        "short word keeps the band, got {:?}",
        ln2[0]
    );
    assert!(
        (ln2[1][1] - 0.0).abs() <= 1.0 && (ln2[1][0] - 40.0).abs() <= 3.0,
        "long word drops below the float, got {:?}",
        ln2[1]
    );
}

/// A centered host of a MIXED run — text, an inline element, text — wraps
/// with each line shifting inside its float slot (#314 tail, #313): the run
/// wrapper's inline_flow placement reads the host's align from its justify
/// and centers line by line, trailing spaces hanging (Chrome parity,
/// 2026-10-23: band lines at 105.5, the below-float line at 175.9).
#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_center_mixed_run() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;height:90px;font:16px/20px monospace}
          .f{float:left;width:100px;height:40px;background:#fa0}
          p{margin:0}
        </style>
        <div class="h" id="m" style="text-align:center"><div class="f"></div>alpha beta <span>gamma delta epsilon</span> zeta eta theta iota kappa more words</div>
        <div class="h" id="l"><div class="f"></div>alpha beta <span>gamma delta epsilon</span> zeta eta theta iota kappa more words</div>"#,
    );
    // Lines are the union of one top's rects, minus the float's own box
    // (the 100px child) and the run's element edges.
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<Vec<f64>> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const h=document.querySelector('{sel}');const fb=h.querySelector('.f').getBoundingClientRect();const r=document.createRange();r.selectNodeContents(h);const byTop={{}};for(const b of r.getClientRects()){{if(b.width<2)continue;if(Math.abs(b.left-fb.left)<1&&Math.abs(b.top-fb.top)<1&&Math.abs(b.width-fb.width)<1)continue;const t=Math.round(b.top);if(!byTop[t])byTop[t]=[b.left,b.right];else{{byTop[t][0]=Math.min(byTop[t][0],b.left);byTop[t][1]=Math.max(byTop[t][1],b.right);}}}}return JSON.stringify(Object.entries(byTop).sort((a,b)=>a[0]-b[0]).map(([t,v])=>[+(v[0]).toFixed(1),+(v[1]-v[0]).toFixed(1)]));}})()"#
            ))
            .unwrap();
        serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default()
    };
    // Center host: every line centers in ITS band — the first two in the
    // float's band (100..400), the below-float line in the full width.
    let lm = lines(&mut rt, "#m");
    assert!(lm.len() == 3, "three lines across the float, got {:?}", lm);
    for (i, l) in lm.iter().take(2).enumerate() {
        let expect = 100.0 + (300.0 - l[1]) / 2.0;
        assert!(
            (l[0] - expect).abs() <= 1.5,
            "center line {} in the band: {} vs {}",
            i,
            l[0],
            expect
        );
    }
    assert!(
        lm[0][0] > 101.5,
        "the band lines shift past the float edge (trailing space hangs), got {}",
        lm[0][0]
    );
    let expect = (400.0 - lm[2][1]) / 2.0;
    assert!(
        (lm[2][0] - expect).abs() <= 1.5,
        "below-float line centers in the full width: {} vs {}",
        lm[2][0],
        expect
    );
    // Left host: banded left, returning to the edge below the float.
    let ll = lines(&mut rt, "#l");
    assert!(ll.len() == 3, "three left lines, got {:?}", ll);
    assert!(
        (ll[0][0] - 100.0).abs() <= 1.0 && (ll[1][0] - 100.0).abs() <= 1.0,
        "left lines at the band start, got {:?}",
        ll
    );
    assert!(
        (ll[2][0] - 0.0).abs() <= 1.0,
        "left host returns to the edge, got {}",
        ll[2][0]
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn float_wrap_center_block_child_band() {
    // The p face (#313/#314): a plain block child of a float host keeps
    // the flex-column promote (it has no floats of its own), which cuts
    // its lines out of the band. The promote demotes inside the host's
    // block flow: the p's lines owe the ancestor floats a band, and its
    // alignment rides the run wrapper's justify (#314 tail).
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .h{width:400px;height:90px;font:16px/20px monospace}
          .f{float:left;width:100px;height:40px;background:#fa0}
          p{margin:0}
        </style>
        <div class="h" id="m" style="text-align:center"><div class="f"></div><p>alpha beta <span>gamma delta epsilon</span> zeta eta theta iota kappa more words</p></div>
        <div class="h" id="n" style="text-align:center"><div class="f"></div><div><p>alpha beta gamma delta epsilon zeta eta theta iota kappa more words</p></div></div>
        <div class="h" id="pl"><div class="f"></div><p>alpha beta gamma delta epsilon zeta eta theta iota kappa more words</p></div>"#,
    );
    let lines = |rt: &mut JsRuntime, sel: &str| -> Vec<Vec<f64>> {
        let v = rt
            .evaluate(&format!(
                r#"(()=>{{const el=document.querySelector('{sel} p');const r=document.createRange();r.selectNodeContents(el);const byTop={{}};for(const b of r.getClientRects()){{if(b.width<2)continue;const t=Math.round(b.top);if(!byTop[t])byTop[t]=[b.left,b.right];else{{byTop[t][0]=Math.min(byTop[t][0],b.left);byTop[t][1]=Math.max(byTop[t][1],b.right);}}}}return JSON.stringify(Object.entries(byTop).sort((a,b)=>a[0]-b[0]).map(([t,v])=>[+(v[0]).toFixed(1),+(v[1]-v[0]).toFixed(1)]));}})()"#
            ))
            .unwrap();
        serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default()
    };
    // Center p with a mixed run: the same band-relative centering as the
    // direct-mount host (float_wrap_center_mixed_run).
    let lm = lines(&mut rt, "#m");
    assert!(lm.len() == 3, "three lines across the float, got {:?}", lm);
    for (i, l) in lm.iter().take(2).enumerate() {
        let expect = 100.0 + (300.0 - l[1]) / 2.0;
        assert!(
            (l[0] - expect).abs() <= 1.5,
            "center line {} in the band: {} vs {}",
            i,
            l[0],
            expect
        );
    }
    let expect = (400.0 - lm[2][1]) / 2.0;
    assert!(
        (lm[2][0] - expect).abs() <= 1.5,
        "below-float line centers in the full width: {} vs {}",
        lm[2][0],
        expect
    );
    // The environment passes through a plain block — a nested div keeps
    // the p in the band (Chrome: only a BFC boundary stops it).
    let ln = lines(&mut rt, "#n");
    assert!(ln.len() == 3, "three nested lines, got {:?}", ln);
    for (i, l) in ln.iter().take(2).enumerate() {
        let expect = 100.0 + (300.0 - l[1]) / 2.0;
        assert!(
            (l[0] - expect).abs() <= 1.5,
            "nested center line {} in the band: {} vs {}",
            i,
            l[0],
            expect
        );
    }
    // A left p keeps its band-left posture, returning to the edge below.
    let lp = lines(&mut rt, "#pl");
    assert!(lp.len() == 3, "three left lines, got {:?}", lp);
    assert!(
        (lp[0][0] - 100.0).abs() <= 1.0 && (lp[1][0] - 100.0).abs() <= 1.0,
        "left lines at the band start, got {:?}",
        lp
    );
    assert!(
        (lp[2][0] - 0.0).abs() <= 1.0,
        "left p returns to the edge, got {}",
        lp[2][0]
    );
}

/// #160 / obscura#1067: the deprecated IDL spellings — visibilityProperty
/// / opacityProperty — answer the same as checkVisibilityCSS/checkOpacity.
/// Playwright 1.25-era call sites pass the old names.
#[test]
fn check_visibility_answers_deprecated_aliases() {
    let mut rt = setup_runtime(
        r#"<div style="visibility:hidden"><span id="in-v">x</span></div>
               <div style="opacity:0"><span id="in-zero">x</span></div>
               <div id="plain">s</div>"#,
    );
    let vis_alias = rt
        .evaluate("document.getElementById('in-v').checkVisibility({visibilityProperty:true})")
        .unwrap();
    assert_eq!(
        vis_alias,
        serde_json::json!(false),
        "visibilityProperty behaves as checkVisibilityCSS"
    );

    let vis_alias_off = rt
        .evaluate("document.getElementById('in-v').checkVisibility({visibilityProperty:false})")
        .unwrap();
    assert_eq!(
        vis_alias_off,
        serde_json::json!(true),
        "explicit false opts out"
    );

    let op_alias = rt
        .evaluate("document.getElementById('in-zero').checkVisibility({opacityProperty:true})")
        .unwrap();
    assert_eq!(
        op_alias,
        serde_json::json!(false),
        "opacityProperty behaves as checkOpacity"
    );

    let plain = rt
            .evaluate("document.getElementById('plain').checkVisibility({visibilityProperty:true,opacityProperty:true})")
            .unwrap();
    assert_eq!(plain, serde_json::json!(true));
}

#[tokio::test(flavor = "current_thread")]
async fn test_computed_width_height_and_percent_box_model() {
    // Chrome truth (headless Chrome, 2026-10-22): getComputedStyle width/
    // height are the USED content-box size (border-box under
    // box-sizing:border-box), untransformed; display:none has no box; and
    // a content-box percent width adds its padding+border on top.
    let mut rt = setup_runtime(
        "<html><head><style>body{margin:0}
              #w{width:600px}
              #a{width:100px;padding:10px;border:2px solid red;height:30px}
              #c{box-sizing:border-box;width:100px;padding:10px;border:2px solid red;height:30px}
              #e{width:100px;height:20px;transform:scale(2)}
              #f{display:none;width:70px}
              #p{width:50%;padding:0 5px;border:3px solid red;height:10px}
              #q{width:50%;padding:0 5px;border:3px solid red;height:10px;box-sizing:border-box}
              #m{max-width:50%;padding:10px;height:10px}
             </style></head><body><div id=w>
              <div id=a>a</div><div id=c>c</div><div id=e>e</div><div id=f>f</div>
              <div id=h style=\"width:calc(100% - 10px);height:calc(2*10px)\">h</div>
              <div id=p></div><div id=q></div><div id=m></div></div></body></html>",
    );
    let v = rt
            .evaluate(
                "(() => { const cs = (i) => { const c = getComputedStyle(document.getElementById(i)); return c.width + ' ' + c.height; };
                  const bw = (i) => Math.round(document.getElementById(i).getBoundingClientRect().width * 100) / 100;
                  return [cs('a'), cs('c'), cs('e'), cs('f'), cs('h'), bw('p'), bw('q'), bw('m')].join('|'); })()",
            )
            .unwrap();
    assert_eq!(
        v,
        "100px 30px|100px 30px|100px 20px|auto auto|590px 20px|316|300|320"
    );
}
