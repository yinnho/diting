//! getComputedStyle 读面 + presentational 反射 + 简写展开
use super::super::*;
use super::common::setup_runtime;

/// Body's UA margin is 8px on every side. Chrome's computed `margin`
/// shorthand collapses that to "8px", and drops repeated sides.
#[test]
fn margin_shorthand_collapses_like_chrome() {
    let mut rt = setup_runtime(
        "<html><body>\
             <div id='z'></div>\
             <div id='v' style='margin:1px 2px'></div>\
             <div id='a' style='margin:1px 2px 3px 4px'></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify({
              body: getComputedStyle(document.body).margin,
              top: getComputedStyle(document.body).marginTop,
              zero: getComputedStyle(document.getElementById('z')).margin,
              pair: getComputedStyle(document.getElementById('v')).margin,
              quad: getComputedStyle(document.getElementById('a')).margin
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["body"], serde_json::json!("8px"));
    assert_eq!(v["top"], serde_json::json!("8px"));
    assert_eq!(v["zero"], serde_json::json!("0px"));
    assert_eq!(v["pair"], serde_json::json!("1px 2px"));
    assert_eq!(v["quad"], serde_json::json!("1px 2px 3px 4px"));
}

#[test]
fn border_and_outline_shorthands_match_chrome() {
    let mut rt = setup_runtime(
        "<html><body>\
             <div id='z'></div>\
             <div id='t' style='border-top:2px solid red'></div>\
             <div id='o' style='outline:2px dashed blue'></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify({
              border: getComputedStyle(document.getElementById('z')).border,
              top: getComputedStyle(document.getElementById('z')).borderTop,
              outline: getComputedStyle(document.getElementById('z')).outline,
              uneven: getComputedStyle(document.getElementById('t')).border,
              unevenTop: getComputedStyle(document.getElementById('t')).borderTop,
              painted: getComputedStyle(document.getElementById('o')).outline
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["border"], serde_json::json!("0px none rgb(0, 0, 0)"));
    assert_eq!(v["top"], serde_json::json!("0px none rgb(0, 0, 0)"));
    assert_eq!(v["outline"], serde_json::json!("rgb(0, 0, 0) none 3px"));
    assert_eq!(v["uneven"], serde_json::json!(""));
    assert_eq!(
        v["unevenTop"],
        serde_json::json!("2px solid rgb(255, 0, 0)")
    );
    assert_eq!(v["painted"], serde_json::json!("rgb(0, 0, 255) dashed 2px"));
}

#[test]
fn outline_stylesheet_matches_chrome() {
    // Stylesheet outline used to fall through to the initial
    // "rgb(0, 0, 0) none 3px". Chrome 157 expands the shorthand and snaps
    // a non-zero sub-pixel length to ±1px, then truncates toward zero.
    let mut rt = setup_runtime(
        "<html><head><style>\
         #sa { outline: 2px dashed blue }\
         #sb { color: blue; outline: solid }\
         #sc { outline-style: dotted }\
         #sd { outline-width: thick }\
         #se { outline-color: red }\
         #sf { outline-offset: 4px }\
         #sg { outline: 2px }\
         #sh { font-size: 20px; outline-width: 2em }\
         #si { outline-style: auto }\
         #sj { outline-style: hidden }\
         #sk { outline: thin dotted green }\
         #sl { outline: 2px solid red; outline-width: 5px }\
         #sm { outline-width: calc(4px + 1px) }\
         #sn { color: green; outline: none }\
         #so { outline: rgb(0, 0, 255) }\
         #sp { outline-color: transparent; outline-style: solid }\
         #sq { outline-width: 1.6px }\
         #sr { outline-offset: -0.4px }\
         #ss { outline-offset: -2px }\
         </style></head><body>\
         <div id='sa'></div><div id='sb'></div><div id='sc'></div>\
         <div id='sd'></div><div id='se'></div><div id='sf'></div>\
         <div id='sg'></div><div id='sh'></div><div id='si'></div>\
         <div id='sj'></div><div id='sk'></div><div id='sl'></div>\
         <div id='sm'></div><div id='sn'></div><div id='so'></div>\
         <div id='sp'></div><div id='sq'></div><div id='sr'></div>\
         <div id='ss'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => {
                const cs = getComputedStyle(document.getElementById(id));
                return { o: cs.outline, s: cs.outlineStyle, w: cs.outlineWidth,
                         c: cs.outlineColor, f: cs.outlineOffset };
              };
              return { sa: g('sa'), sb: g('sb'), sc: g('sc'), sd: g('sd'),
                       se: g('se'), sf: g('sf'), sg: g('sg'), sh: g('sh'),
                       si: g('si'), sj: g('sj'), sk: g('sk'), sl: g('sl'),
                       sm: g('sm'), sn: g('sn'), so: g('so'), sp: g('sp'),
                       sq: g('sq'), sr: g('sr'), ss: g('ss') };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["sa"]["o"], serde_json::json!("rgb(0, 0, 255) dashed 2px"));
    assert_eq!(v["sb"]["o"], serde_json::json!("rgb(0, 0, 255) solid 3px"));
    assert_eq!(v["sc"]["o"], serde_json::json!("rgb(0, 0, 0) dotted 3px"));
    assert_eq!(v["sc"]["s"], serde_json::json!("dotted"));
    assert_eq!(v["sd"]["w"], serde_json::json!("5px"));
    assert_eq!(v["sd"]["o"], serde_json::json!("rgb(0, 0, 0) none 5px"));
    assert_eq!(v["se"]["c"], serde_json::json!("rgb(255, 0, 0)"));
    assert_eq!(v["se"]["o"], serde_json::json!("rgb(255, 0, 0) none 3px"));
    assert_eq!(v["sf"]["f"], serde_json::json!("4px"));
    assert_eq!(v["sf"]["o"], serde_json::json!("rgb(0, 0, 0) none 3px"));
    assert_eq!(v["sg"]["o"], serde_json::json!("rgb(0, 0, 0) none 2px"));
    assert_eq!(v["sh"]["w"], serde_json::json!("40px"));
    assert_eq!(v["si"]["s"], serde_json::json!("auto"));
    assert_eq!(v["si"]["o"], serde_json::json!("rgb(0, 0, 0) auto 3px"));
    assert_eq!(v["sj"]["s"], serde_json::json!("none"));
    assert_eq!(v["sk"]["o"], serde_json::json!("rgb(0, 128, 0) dotted 1px"));
    assert_eq!(v["sl"]["o"], serde_json::json!("rgb(255, 0, 0) solid 5px"));
    assert_eq!(v["sm"]["w"], serde_json::json!("5px"));
    assert_eq!(v["sn"]["o"], serde_json::json!("rgb(0, 128, 0) none 3px"));
    assert_eq!(v["so"]["o"], serde_json::json!("rgb(0, 0, 255) none 3px"));
    assert_eq!(
        v["sp"]["o"],
        serde_json::json!("rgba(0, 0, 0, 0) solid 3px")
    );
    assert_eq!(v["sq"]["w"], serde_json::json!("1px"));
    assert_eq!(v["sr"]["f"], serde_json::json!("-1px"));
    assert_eq!(v["ss"]["f"], serde_json::json!("-2px"));
}

#[test]
fn font_flex_list_background_shorthands_match_chrome() {
    // Chrome 157 computed shorthands: list-style is "position image type",
    // an unset animation is "none", an unset transition is "all". Authored
    // animations drop initial pieces and put the name last.
    let mut rt = setup_runtime(
        "<html><body><div id='z'></div><b id='b'>x</b>\
         <div id='a' style='animation: spin 1s linear 2'></div>\
         <div id='t' style='transition: opacity 1s linear'></div>\
         <div id='l' style='list-style-position: inside; list-style-type: circle'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"const z = getComputedStyle(document.getElementById('z'));
            const b = getComputedStyle(document.getElementById('b'));
            const a = getComputedStyle(document.getElementById('a'));
            const t = getComputedStyle(document.getElementById('t'));
            const l = getComputedStyle(document.getElementById('l'));
            JSON.stringify({
              font: z.font, bold: b.font, flex: z.flex, flow: z.flexFlow,
              list: z.listStyle, bg: z.background, anim: z.animation, tr: z.transition,
              animOn: a.animation, trOn: t.transition, listOn: l.listStyle
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["font"], serde_json::json!("16px Times"));
    assert_eq!(v["bold"], serde_json::json!("700 16px Times"));
    assert_eq!(v["flex"], serde_json::json!("0 1 auto"));
    assert_eq!(v["flow"], serde_json::json!("row nowrap"));
    assert_eq!(v["list"], serde_json::json!("outside none disc"));
    assert_eq!(v["listOn"], serde_json::json!("inside none circle"));
    assert_eq!(
        v["bg"],
        serde_json::json!(
            "rgba(0, 0, 0, 0) none repeat scroll 0% 0% / auto padding-box border-box"
        )
    );
    assert_eq!(v["anim"], serde_json::json!("none"));
    assert_eq!(v["animOn"], serde_json::json!("1s linear 2 spin"));
    assert_eq!(v["tr"], serde_json::json!("all"));
    assert_eq!(v["trOn"], serde_json::json!("opacity 1s linear"));
}

#[test]
fn insets_and_transform_origin_match_chrome() {
    // Chrome 157: a static inset is the specified value (`auto` when unset,
    // a percentage stays a percentage). Relative autos become 0px, and one
    // specified side negates the other. Absolute/fixed insets are used px
    // against the containing block. transform-origin on a box is used px
    // and drops a zero z; display:none keeps "50% 50%".
    let mut rt = setup_runtime(
        "<html><head><style>\
         body { margin: 0 }\
         #s { left: 10px; width: 100px; height: 40px }\
         #sp { left: 10%; width: 80px; height: 20px }\
         #u { width: 100px; height: 40px }\
         #rel { position: relative; width: 100px; height: 40px }\
         #shift { position: relative; left: 10px; top: 5px; width: 100px; height: 40px }\
         #host { width: 200px }\
         #pct { position: relative; left: 10%; width: 100px; height: 40px }\
         #abs { position: absolute; left: 40px; top: 12px; width: 30px; height: 10px }\
         #stick { position: sticky; top: 8px; width: 100px; height: 40px }\
         #none { display: none; width: 100px; height: 40px }\
         #orig { width: 100px; height: 40px; transform-origin: left top }\
         </style></head><body>\
         <div id='s'></div><div id='sp'></div><div id='u'></div>\
         <div id='rel'></div><div id='shift'></div>\
         <div id='host'><div id='pct'></div></div>\
         <div id='abs'></div><div id='stick'></div><div id='none'></div>\
         <div id='orig'></div>\
         <div id='p' style='width:100px;height:40px;perspective-origin:10px 30px'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            __diting_setViewport(800, 600, false, undefined);
            const g = (id) => getComputedStyle(document.getElementById(id));
            const box = (id) => {
              const c = g(id);
              return [c.left, c.top, c.right, c.bottom, c.transformOrigin, c.perspectiveOrigin].join(' ');
            };
            JSON.stringify({
              s: box('s'), sp: box('sp'), u: box('u'), rel: box('rel'),
              shift: box('shift'), pct: box('pct'), abs: box('abs'),
              stick: box('stick'), none: box('none'), orig: box('orig'),
              p: g('p').perspectiveOrigin
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["s"],
        serde_json::json!("10px auto auto auto 50px 20px 50px 20px")
    );
    assert_eq!(
        v["sp"],
        serde_json::json!("10% auto auto auto 40px 10px 40px 10px")
    );
    assert_eq!(
        v["u"],
        serde_json::json!("auto auto auto auto 50px 20px 50px 20px")
    );
    assert_eq!(
        v["rel"],
        serde_json::json!("0px 0px 0px 0px 50px 20px 50px 20px")
    );
    assert_eq!(
        v["shift"],
        serde_json::json!("10px 5px -10px -5px 50px 20px 50px 20px")
    );
    assert_eq!(
        v["pct"],
        serde_json::json!("20px 0px -20px 0px 50px 20px 50px 20px")
    );
    assert_eq!(
        v["abs"],
        serde_json::json!("40px 12px 730px 578px 15px 5px 15px 5px")
    );
    assert_eq!(
        v["stick"],
        serde_json::json!("auto 8px auto auto 50px 20px 50px 20px")
    );
    assert_eq!(
        v["none"],
        serde_json::json!("auto auto auto auto 50% 50% 50% 50%")
    );
    assert_eq!(
        v["orig"],
        serde_json::json!("auto auto auto auto 0px 0px 50px 20px")
    );
    assert_eq!(v["p"], serde_json::json!("10px 30px"));
}

#[test]
fn relative_vertical_percent_uses_parent_content_height() {
    // Chrome 157: top/bottom percentages on a relative element resolve
    // against the parent's content-box height, including when that height
    // is auto (the laid-out content) or border-box (padding is subtracted).
    // Both sides specified stay independent; an auto side is the negation.
    let mut rt = setup_runtime(
        "<html><head><style>body{margin:0}\
         #h{height:200px} #c{position:relative;top:10%;height:20px}\
         #b{height:200px;padding:20px;box-sizing:border-box}\
         #cb{position:relative;top:10%;height:20px}\
         #a #ca{position:relative;top:10%;height:20px}\
         #both{height:200px} #bt{position:relative;top:10%;bottom:20px;height:20px}\
         </style></head><body>\
         <div id='h'><div id='c'></div></div>\
         <div id='b'><div id='cb'></div></div>\
         <div id='a'><div id='ca'></div></div>\
         <div id='both'><div id='bt'></div></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"const g = (id) => getComputedStyle(document.getElementById(id));
            JSON.stringify({
              t: g('c').top, b: g('c').bottom,
              tb: g('cb').top, bb: g('cb').bottom,
              ta: g('ca').top, ba: g('ca').bottom,
              tt: g('bt').top, bt: g('bt').bottom
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["t"], serde_json::json!("20px"));
    assert_eq!(v["b"], serde_json::json!("-20px"));
    assert_eq!(v["tb"], serde_json::json!("16px"));
    assert_eq!(v["bb"], serde_json::json!("-16px"));
    assert_eq!(v["ta"], serde_json::json!("2px"));
    assert_eq!(v["ba"], serde_json::json!("-2px"));
    assert_eq!(v["tt"], serde_json::json!("20px"));
    assert_eq!(v["bt"], serde_json::json!("20px"));
}

#[test]
fn logical_box_and_rule_shorthands_match_chrome() {
    // Chrome 157: logical margin/padding and inset collapse like the
    // physical shorthands. border-block is the shared edge, or "" when the
    // two edges differ. column-rule drops a none style. A relative box's
    // inset is 0px; a static one is auto.
    let mut rt = setup_runtime(
        "<html><body>\
         <div id='z'></div>\
         <div id='m' style='margin-block:10px 4px;padding-inline:6px'></div>\
         <div id='i' style='inset:1px 2px 3px 4px'></div>\
         <div id='r' style='position:relative'></div>\
         <div id='bb' style='border-block:2px solid red'></div>\
         <div id='bs' style='border-block-start:4px solid black'></div>\
         <div id='cr' style='column-rule:4px dotted red'></div>\
         <div id='sm' style='scroll-margin:1px 2px 3px 4px'></div>\
         <div id='sp' style='scroll-padding:10px 20px'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"const g = (id) => getComputedStyle(document.getElementById(id));
            const z = g('z'), body = getComputedStyle(document.body);
            JSON.stringify({
              bm: body.marginBlock, zm: z.marginBlock, zi: z.marginInline,
              zp: z.paddingBlock, zpi: z.paddingInline, zin: z.inset,
              zib: z.insetBlock, zii: z.insetInline, zbb: z.borderBlock,
              zbi: z.borderInline, zcr: z.columnRule, zrr: z.rowRule,
              zsm: z.scrollMargin, zsp: z.scrollPadding,
              mm: g('m').marginBlock, mp: g('m').paddingInline,
              ins: g('i').inset, ib: g('i').insetBlock, ii: g('i').insetInline,
              rel: g('r').inset,
              bb: g('bb').borderBlock, bbs: g('bb').borderBlockStart,
              bempty: g('bs').borderBlock, bstart: g('bs').borderBlockStart,
              cr: g('cr').columnRule, sm: g('sm').scrollMargin, sp: g('sp').scrollPadding
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["bm"], serde_json::json!("8px"));
    assert_eq!(v["zm"], serde_json::json!("0px"));
    assert_eq!(v["zi"], serde_json::json!("0px"));
    assert_eq!(v["zp"], serde_json::json!("0px"));
    assert_eq!(v["zpi"], serde_json::json!("0px"));
    assert_eq!(v["zin"], serde_json::json!("auto"));
    assert_eq!(v["zib"], serde_json::json!("auto"));
    assert_eq!(v["zii"], serde_json::json!("auto"));
    assert_eq!(v["zbb"], serde_json::json!("0px none rgb(0, 0, 0)"));
    assert_eq!(v["zbi"], serde_json::json!("0px none rgb(0, 0, 0)"));
    assert_eq!(v["zcr"], serde_json::json!("3px rgb(0, 0, 0)"));
    assert_eq!(v["zrr"], serde_json::json!("3px rgb(0, 0, 0)"));
    assert_eq!(v["zsm"], serde_json::json!("0px"));
    assert_eq!(v["zsp"], serde_json::json!("auto"));
    assert_eq!(v["mm"], serde_json::json!("10px 4px"));
    assert_eq!(v["mp"], serde_json::json!("6px"));
    assert_eq!(v["ins"], serde_json::json!("1px 2px 3px 4px"));
    assert_eq!(v["ib"], serde_json::json!("1px 3px"));
    assert_eq!(v["ii"], serde_json::json!("4px 2px"));
    assert_eq!(v["rel"], serde_json::json!("0px"));
    assert_eq!(v["bb"], serde_json::json!("2px solid rgb(255, 0, 0)"));
    assert_eq!(v["bbs"], serde_json::json!("2px solid rgb(255, 0, 0)"));
    assert_eq!(v["bempty"], serde_json::json!(""));
    assert_eq!(v["bstart"], serde_json::json!("4px solid rgb(0, 0, 0)"));
    assert_eq!(v["cr"], serde_json::json!("4px dotted rgb(255, 0, 0)"));
    assert_eq!(v["sm"], serde_json::json!("1px 2px 3px 4px"));
    assert_eq!(v["sp"], serde_json::json!("10px 20px"));
}

#[test]
fn emphasis_wrap_columns_stroke_shorthands_match_chrome() {
    // Chrome 157: text-emphasis keeps the color and drops the initial
    // "filled". text-wrap shows the style when it isn't auto, otherwise
    // the mode. columns is "width count", omitting auto. font-synthesis
    // lists the axes that are still on. A zero text-stroke width stays.
    let mut rt = setup_runtime(
        "<html><body>\
         <div id='z'></div>\
         <div id='e' style='text-emphasis: open circle blue'></div>\
         <div id='es' style='text-emphasis-style: triangle'></div>\
         <div id='w' style='text-wrap: nowrap'></div>\
         <div id='wb' style='text-wrap-style: stable'></div>\
         <div id='c' style='columns: 200px 3'></div>\
         <div id='cw' style='column-width: 200px'></div>\
         <div id='f' style='font-synthesis: weight style'></div>\
         <div id='fw' style='font-synthesis-weight: none'></div>\
         <div id='s' style='-webkit-text-stroke: 2px blue'></div>\
         <div id='sw' style='-webkit-text-stroke-width: 3px'></div>\
         <div id='o' style='offset-distance: 10px'></div>\
         <div id='or' style='offset-rotate: 45deg'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"const g = (id) => getComputedStyle(document.getElementById(id));
            const z = g('z');
            JSON.stringify({
              te: z.textEmphasis, tw: z.textWrap, col: z.columns,
              fs: z.fontSynthesis, stk: z.webkitTextStroke, off: z.offset, mask: z.mask,
              e: g('e').textEmphasis, es: g('es').textEmphasis,
              w: g('w').textWrap, wb: g('wb').textWrap,
              c: g('c').columns, cw: g('cw').columns,
              f: g('f').fontSynthesis, fw: g('fw').fontSynthesis,
              s: g('s').webkitTextStroke, sw: g('sw').webkitTextStroke,
              o: g('o').offset, orot: g('or').offset
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["te"], serde_json::json!("none rgb(0, 0, 0)"));
    assert_eq!(v["tw"], serde_json::json!("wrap"));
    assert_eq!(v["col"], serde_json::json!("auto"));
    assert_eq!(v["fs"], serde_json::json!("weight style small-caps"));
    assert_eq!(v["stk"], serde_json::json!("0px rgb(0, 0, 0)"));
    assert_eq!(v["off"], serde_json::json!("none 0px auto 0deg"));
    assert_eq!(v["mask"], serde_json::json!("none"));
    assert_eq!(v["e"], serde_json::json!("open circle rgb(0, 0, 255)"));
    assert_eq!(v["es"], serde_json::json!("triangle rgb(0, 0, 0)"));
    assert_eq!(v["w"], serde_json::json!("nowrap"));
    assert_eq!(v["wb"], serde_json::json!("stable"));
    assert_eq!(v["c"], serde_json::json!("200px 3"));
    assert_eq!(v["cw"], serde_json::json!("200px"));
    assert_eq!(v["f"], serde_json::json!("weight style"));
    assert_eq!(v["fw"], serde_json::json!("style small-caps"));
    assert_eq!(v["s"], serde_json::json!("2px rgb(0, 0, 255)"));
    assert_eq!(v["sw"], serde_json::json!("3px rgb(0, 0, 0)"));
    assert_eq!(v["o"], serde_json::json!("none 10px auto 0deg"));
    assert_eq!(v["orot"], serde_json::json!("none 0px 45deg"));
}

#[test]
fn columns_stylesheet_matches_chrome() {
    // Stylesheet columns used to stay "auto". Chrome folds em against the
    // element's own font-size and omits an auto leg from the shorthand.
    let mut rt = setup_runtime(
        "<html><head><style>\
         #sa { columns: 12em }\
         #sb { font-size: 20px; columns: 12em }\
         #sc { columns: 3 200px }\
         #sd { column-width: 12em }\
         #se { column-count: 3 }\
         #sf { columns: auto 3 }\
         #sg { columns: 200px auto }\
         #sh { column-width: 0 }\
         #si { column-width: calc(10px + 2px) }\
         #sj { columns: 100px 2; column-count: 4 }\
         #sk { column-count: 4; columns: 100px }\
         #sl { column-width: 1.5px }\
         </style></head><body>\
         <div id='sa'></div><div id='sb'></div><div id='sc'></div>\
         <div id='sd'></div><div id='se'></div><div id='sf'></div>\
         <div id='sg'></div><div id='sh'></div><div id='si'></div>\
         <div id='sj'></div><div id='sk'></div><div id='sl'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => {
                const cs = getComputedStyle(document.getElementById(id));
                return { c: cs.columns, w: cs.columnWidth, n: cs.columnCount };
              };
              return { sa: g('sa'), sb: g('sb'), sc: g('sc'), sd: g('sd'),
                       se: g('se'), sf: g('sf'), sg: g('sg'), sh: g('sh'),
                       si: g('si'), sj: g('sj'), sk: g('sk'), sl: g('sl') };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["sa"]["c"], serde_json::json!("192px"));
    assert_eq!(v["sa"]["w"], serde_json::json!("192px"));
    assert_eq!(v["sa"]["n"], serde_json::json!("auto"));
    assert_eq!(v["sb"]["c"], serde_json::json!("240px"));
    assert_eq!(v["sc"]["c"], serde_json::json!("200px 3"));
    assert_eq!(v["sd"]["w"], serde_json::json!("192px"));
    assert_eq!(v["sd"]["c"], serde_json::json!("192px"));
    assert_eq!(v["se"]["c"], serde_json::json!("3"));
    assert_eq!(v["se"]["n"], serde_json::json!("3"));
    assert_eq!(v["sf"]["c"], serde_json::json!("3"));
    assert_eq!(v["sg"]["c"], serde_json::json!("200px"));
    assert_eq!(v["sh"]["w"], serde_json::json!("0px"));
    assert_eq!(v["si"]["w"], serde_json::json!("12px"));
    assert_eq!(v["sj"]["c"], serde_json::json!("100px 4"));
    assert_eq!(v["sk"]["c"], serde_json::json!("100px"));
    assert_eq!(v["sk"]["n"], serde_json::json!("auto"));
    assert_eq!(v["sl"]["w"], serde_json::json!("1.5px"));
}

#[test]
fn transform_origin_z_matches_chrome() {
    // Chrome 157 keeps a non-zero z and drops 0px. x/y percentages resolve
    // against the border box; display:none keeps the specified spelling.
    // `top 10px` is invalid. `center left` is x=left, y=center.
    let mut rt = setup_runtime(
        "<html><head><style>\
         #a { width: 100px; height: 40px; transform-origin: 10px 20px 5px }\
         #b { width: 100px; height: 40px; transform-origin: 10px 20px 0px }\
         #c { width: 100px; height: 40px; transform-origin: 50% 50% 10px }\
         #d { width: 100px; height: 40px; transform-origin: left top 5px }\
         #e { display: none; width: 100px; height: 40px; transform-origin: 50% 25% 10px }\
         #f { display: none; transform-origin: 10px 20px 1em }\
         #g { width: 100px; height: 40px; transform-origin: top 10px }\
         #h { width: 100px; height: 40px; transform-origin: center left 5px }\
         #i { font-size: 20px; width: 100px; height: 40px; transform-origin: 1em 2em 0.5em }\
         #j { width: 100px; height: 40px; transform-origin: 10px 20px -3px }\
         #k { width: 100px; height: 40px; transform-origin: 10px top 8px }\
         </style></head><body>\
         <div id='a'></div><div id='b'></div><div id='c'></div><div id='d'></div>\
         <div id='e'></div><div id='f'></div><div id='g'></div><div id='h'></div>\
         <div id='i'></div><div id='j'></div><div id='k'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => getComputedStyle(document.getElementById(id)).transformOrigin;
              return { a: g('a'), b: g('b'), c: g('c'), d: g('d'), e: g('e'),
                       f: g('f'), g: g('g'), h: g('h'), i: g('i'), j: g('j'), k: g('k') };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["a"], serde_json::json!("10px 20px 5px"));
    assert_eq!(v["b"], serde_json::json!("10px 20px"));
    assert_eq!(v["c"], serde_json::json!("50px 20px 10px"));
    assert_eq!(v["d"], serde_json::json!("0px 0px 5px"));
    assert_eq!(v["e"], serde_json::json!("50% 25% 10px"));
    assert_eq!(v["f"], serde_json::json!("10px 20px 16px"));
    assert_eq!(v["g"], serde_json::json!("50px 20px"));
    assert_eq!(v["h"], serde_json::json!("0px 20px 5px"));
    assert_eq!(v["i"], serde_json::json!("20px 40px 10px"));
    assert_eq!(v["j"], serde_json::json!("10px 20px -3px"));
    assert_eq!(v["k"], serde_json::json!("10px 0px 8px"));
}

#[test]
fn sticky_percent_uses_scrollport_content_box() {
    // Chrome 157: sticky percentages resolve against the nearest scrollport's
    // content box (width for left/right, height for top/bottom). No element
    // scroller means the viewport. Unset stays auto. Relative still uses the
    // parent's content height, which is a different basis.
    let mut rt = setup_runtime(
        "<html><head><style>body{margin:0}\
         #root { position: sticky; top: 10%; left: 10%; width: 20px; height: 20px }\
         #both { position: sticky; top: 10%; bottom: 20%; width: 20px; height: 20px }\
         #px { position: sticky; top: 8px; width: 20px; height: 20px }\
         #sc { width: 250px; height: 300px; overflow: auto }\
         #d { position: sticky; top: 10%; left: 20%; height: 20px }\
         #cb { width: 250px; height: 300px; overflow: auto; padding: 20px; border: 10px solid #000 }\
         #a { position: sticky; top: 10%; left: 10%; height: 20px }\
         #bb { width: 250px; height: 300px; overflow: auto; padding: 20px;\
               border: 10px solid #000; box-sizing: border-box }\
         #b { position: sticky; top: 10%; left: 10%; height: 20px }\
         #nest { width: 400px; height: 400px; overflow: auto }\
         #mid { width: 180px; height: 120px; overflow: auto }\
         #n { position: sticky; top: 10%; left: 10%; height: 20px }\
         #rel { position: relative; top: 10%; height: 20px }\
         #host { width: 400px; height: 200px }\
         </style></head><body>\
         <div id='root'></div><div id='both'></div><div id='px'></div>\
         <div id='sc'><div id='d'></div></div>\
         <div id='cb'><div id='a'></div></div>\
         <div id='bb'><div id='b'></div></div>\
         <div id='nest'><div id='mid'><div id='n'></div></div></div>\
         <div id='host'><div id='rel'></div></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            __diting_setViewport(800, 600, false, undefined);
            const g = (id) => {
              const c = getComputedStyle(document.getElementById(id));
              return [c.top, c.left, c.right, c.bottom].join(' ');
            };
            JSON.stringify({
              root: g('root'), both: g('both'), px: g('px'), d: g('d'),
              a: g('a'), b: g('b'), n: g('n'), rel: g('rel')
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["root"], serde_json::json!("60px 80px auto auto"));
    assert_eq!(v["both"], serde_json::json!("60px auto auto 120px"));
    assert_eq!(v["px"], serde_json::json!("8px auto auto auto"));
    assert_eq!(v["d"], serde_json::json!("30px 50px auto auto"));
    assert_eq!(v["a"], serde_json::json!("30px 25px auto auto"));
    assert_eq!(v["b"], serde_json::json!("24px 19px auto auto"));
    assert_eq!(v["n"], serde_json::json!("12px 18px auto auto"));
    assert_eq!(v["rel"], serde_json::json!("20px 0px 0px -20px"));
}

#[test]
fn gap_grid_place_and_marker_shorthands_match_chrome() {
    // Chrome 157: gap is row then column, collapsed when equal. grid is
    // rows / columns / areas / auto-flow / auto-rows / auto-columns.
    // place-* is the two longhands, one value when they match. border-image
    // and marker are "none" until a side differs.
    let mut rt = setup_runtime(
        "<html><body>\
         <div id='z'></div>\
         <div id='g' style='gap: 10px 4px'></div>\
         <div id='gr' style='row-gap: 10px'></div>\
         <div id='gs' style='row-gap: 8px; column-gap: 8px'></div>\
         <div id='gc' style='grid-template-columns: 1fr 1fr'></div>\
         <div id='ac' style='align-content: center'></div>\
         <div id='jc' style='justify-content: center'></div>\
         <div id='ai' style='align-items: center'></div>\
         <div id='as' style='align-self: center'></div>\
         <div id='mk' style='marker-start: url(a)'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"const g = (id) => getComputedStyle(document.getElementById(id));
            const z = g('z');
            JSON.stringify({
              gap: z.gap, grid: z.grid, pc: z.placeContent, pi: z.placeItems,
              ps: z.placeSelf, bi: z.borderImage, mk: z.marker,
              g: g('g').gap, gr: g('gr').gap, gs: g('gs').gap,
              row: g('g').rowGap, col: g('g').columnGap,
              gc: g('gc').grid, ac: g('ac').placeContent, jc: g('jc').placeContent,
              ai: g('ai').placeItems, aself: g('as').placeSelf, mkk: g('mk').marker
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["gap"], serde_json::json!("normal"));
    assert_eq!(
        v["grid"],
        serde_json::json!("none / none / none / row / auto / auto")
    );
    assert_eq!(v["pc"], serde_json::json!("normal"));
    assert_eq!(v["pi"], serde_json::json!("normal"));
    assert_eq!(v["ps"], serde_json::json!("auto"));
    assert_eq!(v["bi"], serde_json::json!("none"));
    assert_eq!(v["mk"], serde_json::json!("none"));
    assert_eq!(v["g"], serde_json::json!("10px 4px"));
    assert_eq!(v["row"], serde_json::json!("10px"));
    assert_eq!(v["col"], serde_json::json!("4px"));
    assert_eq!(v["gr"], serde_json::json!("10px normal"));
    assert_eq!(v["gs"], serde_json::json!("8px"));
    assert_eq!(
        v["gc"],
        serde_json::json!("none / 1fr 1fr / none / row / auto / auto")
    );
    assert_eq!(v["ac"], serde_json::json!("center normal"));
    assert_eq!(v["jc"], serde_json::json!("normal center"));
    assert_eq!(v["ai"], serde_json::json!("center normal"));
    assert_eq!(v["aself"], serde_json::json!("center auto"));
    assert_eq!(v["mkk"], serde_json::json!(""));
}

#[test]
fn place_shorthands_match_chrome() {
    // Chrome 157 keeps `start` and `end`. One value sets both axes. A later
    // longhand wins. The shorthand collapses when the two axes match.
    let mut rt = setup_runtime(
        "<html><head><style>\
         #a { place-content: center }\
         #b { place-content: center start }\
         #c { place-content: center; justify-content: end }\
         #d { place-items: center stretch }\
         #e { place-items: start end }\
         #f { place-self: center }\
         #g { place-self: start end }\
         #h { place-self: flex-end }\
         #i { justify-content: start }\
         </style></head><body>\
         <div id='a'></div><div id='b'></div><div id='c'></div>\
         <div id='d'></div><div id='e'></div><div id='f'></div>\
         <div id='g'></div><div id='h'></div><div id='i'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => getComputedStyle(document.getElementById(id));
              return {
                a: g('a').placeContent, ac: g('a').alignContent, aj: g('a').justifyContent,
                b: g('b').placeContent, bj: g('b').justifyContent,
                c: g('c').placeContent,
                d: g('d').placeItems, di: g('d').alignItems, dj: g('d').justifyItems,
                e: g('e').placeItems,
                f: g('f').placeSelf, fa: g('f').alignSelf, fj: g('f').justifySelf,
                g: g('g').placeSelf,
                h: g('h').placeSelf,
                i: g('i').justifyContent, ip: g('i').placeContent
              };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["a"], serde_json::json!("center"));
    assert_eq!(v["ac"], serde_json::json!("center"));
    assert_eq!(v["aj"], serde_json::json!("center"));
    assert_eq!(v["b"], serde_json::json!("center start"));
    assert_eq!(v["bj"], serde_json::json!("start"));
    assert_eq!(v["c"], serde_json::json!("center end"));
    assert_eq!(v["d"], serde_json::json!("center stretch"));
    assert_eq!(v["di"], serde_json::json!("center"));
    assert_eq!(v["dj"], serde_json::json!("stretch"));
    assert_eq!(v["e"], serde_json::json!("start end"));
    assert_eq!(v["f"], serde_json::json!("center"));
    assert_eq!(v["fa"], serde_json::json!("center"));
    assert_eq!(v["fj"], serde_json::json!("center"));
    assert_eq!(v["g"], serde_json::json!("start end"));
    assert_eq!(v["h"], serde_json::json!("flex-end"));
    assert_eq!(v["i"], serde_json::json!("start"));
    assert_eq!(v["ip"], serde_json::json!("normal start"));
}

#[test]
fn grid_shorthand_expands_like_chrome() {
    // Chrome 157 spells `grid` as the six longhands. Lengths stay lengths.
    // A bare track list with no slash is invalid. auto-flow before the
    // slash is the row axis (`dense` alone); after the slash it is column.
    // fr on a grid container becomes used px; that resolution is not here.
    let mut rt = setup_runtime(
        "<html><head><style>\
         #a { grid: 100px / 200px }\
         #b { grid: none }\
         #c { grid: auto-flow 40px / 100px }\
         #d { grid: 100px / auto-flow dense 30px }\
         #e { grid: auto-flow dense / 100px }\
         #f { grid: 100px / 200px; grid-template-columns: 50px }\
         #g { grid: 1fr }\
         #h { grid-template-columns: repeat(4, 1fr) }\
         </style></head><body>\
         <div id='a'></div><div id='b'></div><div id='c'></div><div id='d'></div>\
         <div id='e'></div><div id='f'></div><div id='g'></div><div id='h'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => getComputedStyle(document.getElementById(id)).grid;
              return { a: g('a'), b: g('b'), c: g('c'), d: g('d'),
                       e: g('e'), f: g('f'), g: g('g'), h: g('h') };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["a"],
        serde_json::json!("100px / 200px / none / row / auto / auto")
    );
    assert_eq!(
        v["b"],
        serde_json::json!("none / none / none / row / auto / auto")
    );
    assert_eq!(
        v["c"],
        serde_json::json!("none / 100px / none / row / 40px / auto")
    );
    assert_eq!(
        v["d"],
        serde_json::json!("100px / none / none / column dense / auto / 30px")
    );
    assert_eq!(
        v["e"],
        serde_json::json!("none / 100px / none / dense / auto / auto")
    );
    assert_eq!(
        v["f"],
        serde_json::json!("100px / 50px / none / row / auto / auto")
    );
    assert_eq!(
        v["g"],
        serde_json::json!("none / none / none / row / auto / auto")
    );
    assert_eq!(
        v["h"],
        serde_json::json!("none / repeat(4, 1fr) / none / row / auto / auto")
    );
}

#[test]
fn grid_used_tracks_match_chrome() {
    // Chrome 157 resolves template tracks to used px only on a grid
    // container. `1fr`, `repeat()`, `%`, and `minmax()` become the laid-out
    // sizes. A block and `display: none` keep the specified spelling.
    // An empty explicit `1fr` row with `height: auto` is `0px`.
    let mut rt = setup_runtime(
        "<html><head><style>body { margin: 0 }</style></head><body>\
         <div id='a' style='display:grid;width:200px;grid-template-columns:1fr 1fr'></div>\
         <div id='b' style='display:grid;width:200px;height:100px;grid:1fr / 1fr'></div>\
         <div id='c' style='display:grid;width:200px;grid-template-columns:repeat(4, 1fr)'></div>\
         <div id='d' style='width:200px;grid-template-columns:repeat(4, 1fr)'></div>\
         <div id='e' style='display:grid;width:400px;grid-template-columns:25% 1fr'></div>\
         <div id='f' style='display:grid;width:200px;column-gap:20px;grid-template-columns:1fr 1fr'></div>\
         <div id='g' style='display:grid;width:200px;padding:10px;grid-template-columns:1fr 1fr'></div>\
         <div id='h' style='display:grid;box-sizing:border-box;width:200px;padding:10px;grid-template-columns:1fr 1fr'></div>\
         <div id='i' style='display:grid;width:200px;grid-template-columns:minmax(0, 1fr) 50px'></div>\
         <div id='j' style='display:grid;width:200px;height:100px;grid-template-rows:1fr'></div>\
         <div id='k' style='display:grid;width:200px;grid-template-rows:1fr'></div>\
         <div id='l' style='display:none;width:200px;grid-template-columns:1fr 1fr'></div>\
         <div id='m' style='display:inline-grid;width:200px;grid-template-columns:1fr 1fr'></div>\
         <div id='n' style='display:grid;width:201px;grid-template-columns:1fr 1fr'></div>\
         <div id='o' style='display:grid;width:200px;grid-template-columns:100px 100px'>\
           <div style='height:20px'></div><div style='height:20px'></div><div style='height:20px'></div>\
         </div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => getComputedStyle(document.getElementById(id)).grid;
              return {
                a: g('a'), b: g('b'), c: g('c'), d: g('d'), e: g('e'),
                f: g('f'), g: g('g'), h: g('h'), i: g('i'), j: g('j'),
                k: g('k'), l: g('l'), m: g('m'), n: g('n'), o: g('o')
              };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["a"],
        serde_json::json!("none / 100px 100px / none / row / auto / auto")
    );
    assert_eq!(
        v["b"],
        serde_json::json!("100px / 200px / none / row / auto / auto")
    );
    assert_eq!(
        v["c"],
        serde_json::json!("none / 50px 50px 50px 50px / none / row / auto / auto")
    );
    assert_eq!(
        v["d"],
        serde_json::json!("none / repeat(4, 1fr) / none / row / auto / auto")
    );
    assert_eq!(
        v["e"],
        serde_json::json!("none / 100px 300px / none / row / auto / auto")
    );
    assert_eq!(
        v["f"],
        serde_json::json!("none / 90px 90px / none / row / auto / auto")
    );
    assert_eq!(
        v["g"],
        serde_json::json!("none / 100px 100px / none / row / auto / auto")
    );
    assert_eq!(
        v["h"],
        serde_json::json!("none / 90px 90px / none / row / auto / auto")
    );
    assert_eq!(
        v["i"],
        serde_json::json!("none / 150px 50px / none / row / auto / auto")
    );
    assert_eq!(
        v["j"],
        serde_json::json!("100px / none / none / row / auto / auto")
    );
    assert_eq!(
        v["k"],
        serde_json::json!("0px / none / none / row / auto / auto")
    );
    assert_eq!(
        v["l"],
        serde_json::json!("none / 1fr 1fr / none / row / auto / auto")
    );
    assert_eq!(
        v["m"],
        serde_json::json!("none / 100px 100px / none / row / auto / auto")
    );
    assert_eq!(
        v["n"],
        serde_json::json!("none / 100.5px 100.5px / none / row / auto / auto")
    );
    assert_eq!(
        v["o"],
        serde_json::json!("20px 20px / 100px 100px / none / row / auto / auto")
    );
}

#[test]
fn grid_areas_shorthand_matches_chrome() {
    // Chrome 157 accepts the areas form of `grid`. Strings serialize with
    // double quotes. A missing row size is `auto`, and on a grid container
    // that auto row is `0px`. Explicit columns resolve to used px. The
    // areas themselves also create columns when the slash is omitted.
    // `grid` resets auto-flow; `grid-template` does not. A jagged matrix
    // drops the whole declaration.
    let mut rt = setup_runtime(
        "<html><head><style>body { margin: 0 }</style></head><body>\
         <div id='a' style=\"grid: 'a a' / 1fr 1fr\"></div>\
         <div id='b' style=\"display:grid;width:200px;grid: 'a a' / 1fr 1fr\"></div>\
         <div id='c' style=\"grid: 'a a' 20px / 1fr 1fr\"></div>\
         <div id='d' style=\"display:grid;width:200px;height:100px;grid: 'a a' 20px / 1fr 1fr\"></div>\
         <div id='e' style=\"grid: 'a a' 'b b' / 1fr 1fr\"></div>\
         <div id='f' style=\"display:grid;width:200px;grid: 'a a' 'b b' / 1fr 1fr\"></div>\
         <div id='g' style=\"grid: 'a a' 20px 'b b' 30px / 100px 1fr\"></div>\
         <div id='h' style=\"grid: &quot;a a&quot; / 1fr 1fr\"></div>\
         <div id='i' style=\"grid: auto-flow dense / 100px; grid-template: 'a a' / 1fr 1fr\"></div>\
         <div id='j' style=\"grid: auto-flow dense / 100px; grid: 'a a' / 1fr 1fr\"></div>\
         <div id='k' style=\"grid-template-areas: 'a a'\"></div>\
         <div id='l' style=\"grid: 'a . b' / 1fr 1fr 1fr\"></div>\
         <div id='m' style=\"grid: 'a a'\"></div>\
         <div id='n' style=\"display:grid;width:200px;grid: 'a a'\"></div>\
         <div id='o' style=\"grid: 'a a' 'b'\"></div>\
         <div id='p' style=\"grid: 'a a' 20px / 1fr 1fr; grid-template-columns: 50px\"></div>\
         <div id='q' style=\"grid-template-areas: 'a ... b'\"></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => getComputedStyle(document.getElementById(id));
              const ids = 'abcdefghijklmnopq'.split('');
              const o = {};
              for (const id of ids) o[id] = g(id).grid;
              o.iflow = g('i').gridAutoFlow;
              o.jflow = g('j').gridAutoFlow;
              return o;
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["a"],
        serde_json::json!("auto / 1fr 1fr / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["b"],
        serde_json::json!("0px / 100px 100px / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["c"],
        serde_json::json!("20px / 1fr 1fr / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["d"],
        serde_json::json!("20px / 100px 100px / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["e"],
        serde_json::json!("auto auto / 1fr 1fr / \"a a\" \"b b\" / row / auto / auto")
    );
    assert_eq!(
        v["f"],
        serde_json::json!("0px 0px / 100px 100px / \"a a\" \"b b\" / row / auto / auto")
    );
    assert_eq!(
        v["g"],
        serde_json::json!("20px 30px / 100px 1fr / \"a a\" \"b b\" / row / auto / auto")
    );
    assert_eq!(
        v["h"],
        serde_json::json!("auto / 1fr 1fr / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["i"],
        serde_json::json!("auto / 1fr 1fr / \"a a\" / dense / auto / auto")
    );
    assert_eq!(v["iflow"], serde_json::json!("dense"));
    assert_eq!(
        v["j"],
        serde_json::json!("auto / 1fr 1fr / \"a a\" / row / auto / auto")
    );
    assert_eq!(v["jflow"], serde_json::json!("row"));
    assert_eq!(
        v["k"],
        serde_json::json!("none / none / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["l"],
        serde_json::json!("auto / 1fr 1fr 1fr / \"a . b\" / row / auto / auto")
    );
    assert_eq!(
        v["m"],
        serde_json::json!("auto / none / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["n"],
        serde_json::json!("0px / 100px 100px / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["o"],
        serde_json::json!("none / none / none / row / auto / auto")
    );
    assert_eq!(
        v["p"],
        serde_json::json!("20px / 50px / \"a a\" / row / auto / auto")
    );
    assert_eq!(
        v["q"],
        serde_json::json!("none / none / \"a . b\" / row / auto / auto")
    );
}

#[test]
fn grid_track_zero_matches_chrome() {
    // Chrome 157 computes a bare zero as 0px inside minmax() and repeat().
    // 0fr and 0% stay. A grid container still resolves the track to used px.
    // display:none keeps the specified spelling. repeat() is not a value of
    // grid-auto-rows, so that declaration drops and the earlier 10px stays.
    let mut rt = setup_runtime(
        "<html><head><style>body { margin: 0 }</style></head><body>\
         <div id='a' style='grid-template-columns:minmax(0, 1fr)'></div>\
         <div id='b' style='grid-template-columns:0'></div>\
         <div id='c' style='grid-template-columns:repeat(2, minmax(0, 1fr))'></div>\
         <div id='d' style='grid-template-columns:0fr'></div>\
         <div id='e' style='grid-template-columns:0%'></div>\
         <div id='f' style='grid-template-columns:minmax(0,1fr)'></div>\
         <div id='g' style='grid-template-columns:11.5rem'></div>\
         <div id='h' style='display:none;grid-template-columns:minmax(0, 1fr)'></div>\
         <div id='i' style='display:grid;width:200px;grid-template-columns:minmax(0, 1fr)'></div>\
         <div id='j' style='grid-auto-rows:minmax(0, 1fr)'></div>\
         <div id='k' style='grid-auto-columns:0'></div>\
         <div id='l' style='grid: 100px / minmax(0, 1fr)'></div>\
         <div id='m' style='grid-auto-rows:10px;grid-auto-rows:repeat(2, 10px)'></div>\
         <div id='n' style='grid-template-columns:repeat(4, 1fr)'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              const g = (id) => getComputedStyle(document.getElementById(id)).grid;
              const ids = 'abcdefghijklmn'.split('');
              const o = {};
              for (const id of ids) o[id] = g(id);
              return o;
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["a"],
        serde_json::json!("none / minmax(0px, 1fr) / none / row / auto / auto")
    );
    assert_eq!(
        v["b"],
        serde_json::json!("none / 0px / none / row / auto / auto")
    );
    assert_eq!(
        v["c"],
        serde_json::json!("none / repeat(2, minmax(0px, 1fr)) / none / row / auto / auto")
    );
    assert_eq!(
        v["d"],
        serde_json::json!("none / 0fr / none / row / auto / auto")
    );
    assert_eq!(
        v["e"],
        serde_json::json!("none / 0% / none / row / auto / auto")
    );
    assert_eq!(
        v["f"],
        serde_json::json!("none / minmax(0px, 1fr) / none / row / auto / auto")
    );
    assert_eq!(
        v["g"],
        serde_json::json!("none / 184px / none / row / auto / auto")
    );
    assert_eq!(
        v["h"],
        serde_json::json!("none / minmax(0px, 1fr) / none / row / auto / auto")
    );
    assert_eq!(
        v["i"],
        serde_json::json!("none / 200px / none / row / auto / auto")
    );
    assert_eq!(
        v["j"],
        serde_json::json!("none / none / none / row / minmax(0px, 1fr) / auto")
    );
    assert_eq!(
        v["k"],
        serde_json::json!("none / none / none / row / auto / 0px")
    );
    assert_eq!(
        v["l"],
        serde_json::json!("100px / minmax(0px, 1fr) / none / row / auto / auto")
    );
    assert_eq!(
        v["m"],
        serde_json::json!("none / none / none / row / 10px / auto")
    );
    assert_eq!(
        v["n"],
        serde_json::json!("none / repeat(4, 1fr) / none / row / auto / auto")
    );
}

#[test]
fn grid_track_em_matches_chrome() {
    // em uses the element's own font-size, rem the root. A plain-length
    // calc folds. Viewport units use the pinned window. 1fr beside an em
    // track still becomes used px on a grid container.
    let mut rt = setup_runtime(
        "<html><head><style>\
         #em { font-size: 30px; grid-template-columns: 1em }\
         #rem { font-size: 30px; grid-template-columns: 1rem }\
         #mm { font-size: 20px; grid-template-columns: minmax(0, 2em) }\
         #calc { font-size: 20px; grid-template-columns: calc(10px + 1em) }\
         #rep { grid-template-columns: repeat(2, 1em) }\
         #up { font-size: 20px; grid-template-columns: 1EM }\
         #vw { grid-template-columns: 10vw }\
         #vh { grid-template-columns: 10vh }\
         #used { display: grid; width: 200px; font-size: 20px; grid-template-columns: 1em 1fr }\
         </style></head><body>\
         <div id='em'></div><div id='rem'></div><div id='mm'></div>\
         <div id='calc'></div><div id='rep'></div><div id='up'></div>\
         <div id='vw'></div><div id='vh'></div><div id='used'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((() => {
              __diting_setViewport(1000, 500, false, undefined);
              const g = (id) => getComputedStyle(document.getElementById(id)).grid;
              return {
                em: g('em'), rem: g('rem'), mm: g('mm'), calc: g('calc'),
                rep: g('rep'), up: g('up'), vw: g('vw'), vh: g('vh'), used: g('used')
              };
            })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["em"],
        serde_json::json!("none / 30px / none / row / auto / auto")
    );
    assert_eq!(
        v["rem"],
        serde_json::json!("none / 16px / none / row / auto / auto")
    );
    assert_eq!(
        v["mm"],
        serde_json::json!("none / minmax(0px, 40px) / none / row / auto / auto")
    );
    assert_eq!(
        v["calc"],
        serde_json::json!("none / 30px / none / row / auto / auto")
    );
    assert_eq!(
        v["rep"],
        serde_json::json!("none / repeat(2, 16px) / none / row / auto / auto")
    );
    assert_eq!(
        v["up"],
        serde_json::json!("none / 20px / none / row / auto / auto")
    );
    assert_eq!(
        v["vw"],
        serde_json::json!("none / 100px / none / row / auto / auto")
    );
    assert_eq!(
        v["vh"],
        serde_json::json!("none / 50px / none / row / auto / auto")
    );
    assert_eq!(
        v["used"],
        serde_json::json!("none / 20px 180px / none / row / auto / auto")
    );
}

#[test]
fn text_decoration_shorthand_matches_chrome() {
    // Chrome 157 omits solid, currentcolor, and auto thickness. An explicit
    // color or a non-solid style stays. Line keywords come out as
    // underline, overline, line-through. Color on the element does not
    // leak into the shorthand unless the decoration names its own color.
    let mut rt = setup_runtime(
        "<html><body>\
         <div id='z'></div>\
         <div id='a' style='text-decoration: underline'></div>\
         <div id='b' style='text-decoration: underline red'></div>\
         <div id='c' style='text-decoration: underline dotted'></div>\
         <div id='d' style='text-decoration: underline wavy blue'></div>\
         <div id='e' style='text-decoration: line-through overline'></div>\
         <div id='f' style='text-decoration-color: red'></div>\
         <div id='g' style='text-decoration-style: dotted'></div>\
         <div id='k' style='text-decoration-thickness: 2px'></div>\
         <div id='l' style='color: blue; text-decoration: underline'></div>\
         <style>\
         #sa { text-decoration: underline red }\
         #sb { text-decoration: underline wavy blue }\
         #sc { text-decoration: dotted }\
         </style>\
         <div id='sa'></div><div id='sb'></div><div id='sc'></div>\
         </body></html>",
    );
    let out = rt
        .evaluate(
            r#"const g = (id) => getComputedStyle(document.getElementById(id)).textDecoration;
            JSON.stringify({
              z: g('z'), a: g('a'), b: g('b'), c: g('c'), d: g('d'),
              e: g('e'), f: g('f'), s: g('g'), k: g('k'), l: g('l'),
              lc: getComputedStyle(document.getElementById('l')).textDecorationColor,
              sa: g('sa'), sb: g('sb'), sc: g('sc')
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["z"], serde_json::json!("none"));
    assert_eq!(v["a"], serde_json::json!("underline"));
    assert_eq!(v["b"], serde_json::json!("underline rgb(255, 0, 0)"));
    assert_eq!(v["c"], serde_json::json!("underline dotted"));
    assert_eq!(v["d"], serde_json::json!("underline wavy rgb(0, 0, 255)"));
    assert_eq!(v["e"], serde_json::json!("overline line-through"));
    assert_eq!(v["f"], serde_json::json!("rgb(255, 0, 0)"));
    assert_eq!(v["s"], serde_json::json!("dotted"));
    assert_eq!(v["k"], serde_json::json!("2px"));
    assert_eq!(v["l"], serde_json::json!("underline"));
    assert_eq!(v["lc"], serde_json::json!("rgb(0, 0, 255)"));
    assert_eq!(v["sa"], serde_json::json!("underline rgb(255, 0, 0)"));
    assert_eq!(v["sb"], serde_json::json!("underline wavy rgb(0, 0, 255)"));
    assert_eq!(v["sc"], serde_json::json!("dotted"));
}

#[test]
fn computed_keywords_and_new_initials_match_chrome() {
    // Chrome 157 (local 157.0.8103.52): medium/thin/thick rule widths are
    // px, corner-shape keywords are superellipse(), offset-rotate:auto is
    // "auto 0deg", and the longhands added since the 145 list have initials.
    let mut rt = setup_runtime(
        "<html><body><div id='z'></div>\
         <div id='k' style='column-rule-width: medium; row-rule-width: thick; \
         corner-top-left-shape: round; offset-rotate: reverse; \
         column-rule-break: auto; text-emphasis-position: over right'></div>\
         <p id='c' style='color: red'>x</p></body></html>",
    );
    let out = rt
        .evaluate(
            r#"const z = getComputedStyle(document.getElementById('z'));
            const k = getComputedStyle(document.getElementById('k'));
            const c = getComputedStyle(document.getElementById('c'));
            JSON.stringify({
              ruleW: z.columnRuleWidth, rowW: z.rowRuleWidth,
              ruleB: z.columnRuleBreak, rowB: z.rowRuleBreak,
              corner: z.cornerTopLeftShape, mask: z.maskPosition,
              rot: z.offsetRotate, anchor: z.positionAnchor,
              vis: z.positionVisibility, emph: z.textEmphasisPosition,
              slice: z.webkitMaskBoxImageSlice, trig: z.animationTrigger,
              buf: z.bufferedRendering, colH: z.columnHeight,
              flexN: z.flexLineCount, frame: z.frameSizing,
              overlap: z.ruleOverlap, axis: z.scrollAxisLock,
              drag: z.windowDrag, bimg: z.webkitBorderImage,
              mimg: z.webkitMaskBoxImage, inset: z.columnRuleInsetCapEnd,
              visItems: z.columnRuleVisibilityItems, rowVis: z.rowRuleVisibilityItems,
              src: z.timelineTriggerSource, act: z.timelineTriggerActivationRangeStart,
              active: z.timelineTriggerActiveRangeStart,
              kW: k.columnRuleWidth, kRow: k.rowRuleWidth, kCorner: k.cornerTopLeftShape,
              kRot: k.offsetRotate, kBreak: k.columnRuleBreak, kEmph: k.textEmphasisPosition,
              caret: c.caretColor
            })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["ruleW"], serde_json::json!("3px"));
    assert_eq!(v["rowW"], serde_json::json!("3px"));
    assert_eq!(v["ruleB"], serde_json::json!("normal"));
    assert_eq!(v["rowB"], serde_json::json!("normal"));
    assert_eq!(v["corner"], serde_json::json!("superellipse(1)"));
    assert_eq!(v["mask"], serde_json::json!("0% 0%"));
    assert_eq!(v["rot"], serde_json::json!("auto 0deg"));
    assert_eq!(v["anchor"], serde_json::json!("normal"));
    assert_eq!(v["vis"], serde_json::json!("anchor-visible"));
    assert_eq!(v["emph"], serde_json::json!("over"));
    assert_eq!(v["slice"], serde_json::json!("0 fill"));
    assert_eq!(v["trig"], serde_json::json!("none"));
    assert_eq!(v["buf"], serde_json::json!("auto"));
    assert_eq!(v["colH"], serde_json::json!("auto"));
    assert_eq!(v["flexN"], serde_json::json!("1"));
    assert_eq!(v["frame"], serde_json::json!("auto"));
    assert_eq!(v["overlap"], serde_json::json!("row-over-column"));
    assert_eq!(v["axis"], serde_json::json!("auto"));
    assert_eq!(v["drag"], serde_json::json!("none"));
    assert_eq!(v["bimg"], serde_json::json!("none"));
    assert_eq!(v["mimg"], serde_json::json!("none"));
    assert_eq!(v["inset"], serde_json::json!("0px"));
    assert_eq!(v["visItems"], serde_json::json!("normal"));
    assert_eq!(v["rowVis"], serde_json::json!("normal"));
    assert_eq!(v["src"], serde_json::json!("auto"));
    assert_eq!(v["act"], serde_json::json!("normal"));
    assert_eq!(v["active"], serde_json::json!("auto"));
    assert_eq!(v["kW"], serde_json::json!("3px"));
    assert_eq!(v["kRow"], serde_json::json!("5px"));
    assert_eq!(v["kCorner"], serde_json::json!("superellipse(1)"));
    assert_eq!(v["kRot"], serde_json::json!("auto 180deg"));
    assert_eq!(v["kBreak"], serde_json::json!("normal"));
    assert_eq!(v["kEmph"], serde_json::json!("over"));
    assert_eq!(v["caret"], serde_json::json!("rgb(255, 0, 0)"));
}

/// #214 (blitz#998): `text-align-last` on the computed face — declared
/// keywords round-trip verbatim, undeclared reports the css-text-3
/// initial `auto`, and inheritance rides the cascade (a child sees the
/// ancestor's value until it declares its own). `text-align: justify`
/// parses too — reported verbatim; the stretch itself is not painted
/// (consumed as start by layout).
#[test]
fn computed_style_reports_text_align_last() {
    let mut rt = setup_runtime(
        "<html><body>\
             <div id='a' style='text-align:justify;text-align-last:right'>x</div>\
             <div id='b' style='text-align-last:center'><span id='c'>y</span></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var a = getComputedStyle(document.getElementById('a'));
            var c = getComputedStyle(document.getElementById('c'));
            [a.getPropertyValue('text-align-last'),
             a.getPropertyValue('text-align'),
             c.getPropertyValue('text-align-last')].join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        "right|justify|center",
        "declared right round-trips, justify parses on text-align, the span inherits center"
    );
}

/// #207: the border style/color longhands on the computed face. Declared
/// keywords round-trip, undeclared style reports the CSS initial `none`,
/// and the color resolves currentcolor Chrome-style (declared border
/// color, else the element's color, else black).
#[test]
fn computed_style_reports_border_style_color() {
    let mut rt = setup_runtime(
        "<html><body>\
             <div id='a' style='border: 2px dashed #ff0000'>x</div>\
             <div id='b' style='color: rgb(0, 128, 0)'>y</div>\
             <div id='c'>z</div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var a = getComputedStyle(document.getElementById('a'));
            var b = getComputedStyle(document.getElementById('b'));
            var c = getComputedStyle(document.getElementById('c'));
            [a.getPropertyValue('border-top-style'),
             a.getPropertyValue('border-left-style'),
             a.getPropertyValue('border-top-color'),
             a.getPropertyValue('border-bottom-color'),
             a.getPropertyValue('border-top-width'),
             b.getPropertyValue('border-top-color'),
             c.getPropertyValue('border-top-style'),
             c.getPropertyValue('border-top-color')].join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        "dashed|dashed|rgb(255, 0, 0)|rgb(255, 0, 0)|2px|rgb(0, 128, 0)|none|rgb(0, 0, 0)",
        "declared longhands round-trip, undeclared style is none, color resolves currentcolor"
    );
}

/// #242: percentage padding/margin report Chrome's used px from
/// getComputedStyle, resolved against the containing block — in-flow
/// against the parent's content box (ancestor percentage insets resolve
/// recursively), absolutely positioned against the nearest positioned
/// ancestor's padding box.
#[test]
fn computed_style_resolves_percent_padding_margin_to_used_px() {
    let mut rt = setup_runtime(
        "<html><head><style>body { margin: 0 }\
             #host { width: 500px }\
             #nested { padding: 4%; width: 400px; box-sizing: border-box }\
             #abs { position: absolute; margin-left: 10% }\
             #pos { position: relative; width: 300px; border: 10px solid }\
             </style></head><body>\
             <div id='host'><div id='inflow' style='padding: 10%; margin-top: 5%'>x</div>\
             <div id='nested'><div id='deep' style='padding-top: 25%'>y</div></div></div>\
             <div id='pos'><div id='abs'>z</div></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var g = function (id) { return getComputedStyle(document.getElementById(id)); };
            [g('inflow').getPropertyValue('padding-top'),
             g('inflow').getPropertyValue('margin-top'),
             g('deep').getPropertyValue('padding-top'),
             g('abs').getPropertyValue('margin-left')].join('|');
        "#,
        )
        .unwrap();
    // inflow: CB = #host content box 500 → 10% = 50, 5% = 25.
    // deep: #nested is border-box 400; its padding 4% resolves against
    // #host content 500 = 20/side → #nested content 360 is #deep's CB →
    // 25% = 90. (Authored px width + % padding under content-box is the
    // style-map's known compensation gap — px sizes only map over by
    // px-valued padding/border — so border-box makes the fixture
    // unambiguous for engine and Chrome alike.)
    // abs: CB = #pos PADDING box = content 300 + padding 0 = 300 — the
    // border sits OUTSIDE the padding box, never subtracted → 10% = 30.
    assert_eq!(
        out.as_str().unwrap(),
        "50px|25px|90px|30px",
        "percent padding/margin resolve against the CSS containing block"
    );
}

/// #244: the logical box longhands and transform-origin join the served
/// gCS table. Logical names answer from the physical slots (horizontal-tb:
/// block→top/bottom, inline→left/right) with the same used-px resolution
/// as #242; transform-origin serves Chrome's computed form with the z
/// pinned to 0px. The declarations must also LAYOUT — a logical margin
/// moves the box exactly like its physical twin.
#[test]
fn logical_box_properties_and_transform_origin_serve_and_layout() {
    let mut rt = setup_runtime(
        "<html><head><style>body { margin: 0 }\
             #a { margin-block-start: 10px; padding-inline-start: 5% }\
             #host { width: 400px }\
             #b { margin-inline: 4px 8px }\
             #pct { width: 200px; height: 40px }\
             #c { transform-origin: left top; transform: rotate(90deg); width: 100px; height: 50px }\
             #d { transform: rotate(90deg); width: 100px; height: 50px }\
             </style></head><body>\
             <div id='a'>x</div>\
             <div id='host'><div id='pct'>y</div></div>\
             <div id='b'>z</div>\
             <div id='c'></div><div id='d'></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            // Pin the viewport: the stealth persona randomizes the default
            // width across common laptop sizes, and the 5% padding below
            // resolves against it.
            __diting_setViewport(1920, 1000, false, undefined);
            var g = function (id) { return getComputedStyle(document.getElementById(id)); };
            [g('a').getPropertyValue('margin-block-start'),
             g('a').getPropertyValue('padding-inline-start'),
             g('b').getPropertyValue('margin-inline-start'),
             g('b').getPropertyValue('margin-inline-end'),
             g('pct').getPropertyValue('transform-origin'),
             g('c').getPropertyValue('transform-origin'),
             g('a').getPropertyValue('margin-inline-start')].join('|');
        "#,
        )
        .unwrap();
    // #a: logical margin/padding land in the physical slots; #a's
    // padding-inline-start 5% resolves against its CB (body content —
    // margin 0, so the full pinned 1920) like any #242 face → 96px.
    // #b: the 1-2 value shorthand fills left=4 right=8.
    // transform-origin: Chrome resolves against the border box and drops
    // a zero z. #pct is 200×40 so the initial center is "100px 20px";
    // "left top" on #c is "0px 0px". Unset logical margin is 0px.
    assert_eq!(
        out.as_str().unwrap(),
        "10px|96px|4px|8px|100px 20px|0px 0px|0px",
        "logical longhands serve from the physical slots with used-px resolution"
    );
    // The declarations LAYOUT too: #a's margin-block-start: 10px offsets
    // it exactly like margin-top (gBCR y = 10), and the declared
    // transform-origin actually pivots. Chrome math for rotate(90deg)
    // (clockwise, y-down: (x,y)→(−y,x)) on a 100×50 box at flow slot
    // (0, Y): left-top pivot gives bounds x∈[−50,0], y∈[Y,Y+100];
    // center pivot gives x∈[25,75], y∈[Y−25,Y+75]. Both are 50×100.
    // #d flows BELOW #c's unrotated 50px box (transform doesn't reflow),
    // so with slots Yc and Yc+50: c.x−d.x = −50−25 = −75 and
    // c.y−d.y = Yc−(Yc+25) = −25 — the deltas isolate the pivot without
    // pinning Y itself (font-dependent line stack above).
    let geo = rt
            .evaluate(
                r#"
            var r = function (id) { return document.getElementById(id).getBoundingClientRect(); };
            var a = r('a'), c = r('c'), d = r('d');
            return [Math.round(a.y),
                    Math.round(c.x), Math.round(c.y), Math.round(c.width), Math.round(c.height),
                    Math.round(d.x), Math.round(d.y), Math.round(d.width), Math.round(d.height)].join('|');
        "#,
            )
            .unwrap();
    let nums: Vec<f64> = geo
        .as_str()
        .unwrap()
        .split('|')
        .map(|n| n.parse::<f64>().unwrap_or(0.0))
        .collect();
    assert_eq!(
        nums[0], 10.0,
        "margin-block-start: 10px must offset the box like margin-top"
    );
    // Both pivots: rotated bounds are 50 wide × 100 tall.
    assert_eq!(
        (nums[3], nums[4]),
        (50.0, 100.0),
        "left-top pivot bounds 50x100"
    );
    assert_eq!(
        (nums[7], nums[8]),
        (50.0, 100.0),
        "center pivot bounds 50x100"
    );
    // The pivot deltas (d's slot sits 50 below c's unrotated box):
    // c swings left past the slot edge, d hugs the slot center.
    assert_eq!(
        nums[1] - nums[5],
        -75.0,
        "c.x = d.x - 75 (left-top vs center pivot)"
    );
    assert_eq!(
        nums[2] - nums[6],
        -25.0,
        "c.y = d.y - 25 (center pivot raises d by 25, slot +50)"
    );
}

/// #120: the computed-style object must brand as CSSStyleDeclaration.
/// @alifd/next's getStyle gates reads on isPlainObject(holder) —
/// toString tag 'Object' + constructor === Object — and answers null
/// when it passes; place() then parseFloat(null)s the offsetParent's
/// border widths into `left/top: NaNpx` (tmall publish SKU drawer,
/// pinned stuck off-viewport because the drawer never remounts to
/// re-run place()). The tag and the constructor are exactly the two
/// identity hooks isPlainObject reads; both must answer like Chrome.
#[test]
fn computed_style_brands_as_cssstyledeclaration() {
    let mut rt = setup_runtime("<html><body><div id='d'>x</div></body></html>");
    let out = rt
        .evaluate(
            r#"
            var cs = getComputedStyle(document.getElementById('d'));
            var isPlainObject = function (e) {
                var T = Object.prototype.toString.call(e).slice(8, -1);
                if (T !== 'Object') return false;
                var c = e.constructor;
                if (typeof c !== 'function') return false;
                var p = c.prototype;
                return Object.prototype.toString.call(p).slice(8, -1) === 'Object'
                    && !!Object.prototype.hasOwnProperty.call(p, 'isPrototypeOf');
            };
            // The consumption shape straight out of the bundle's getStyle:
            // null when isPlainObject, else the property read.
            var getStyle = function (el, prop) {
                var n = el && 1 === el.nodeType ? getComputedStyle(el, null) : {};
                return isPlainObject(n) ? null : n.getPropertyValue(prop);
            };
            return [
                Object.prototype.toString.call(cs),
                cs[Symbol.toStringTag],
                cs.constructor === CSSStyleDeclaration,
                isPlainObject(cs),
                String(getStyle(document.getElementById('d'), 'border-top-width')),
            ].join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        "[object CSSStyleDeclaration]|CSSStyleDeclaration|true|false|0px",
        "tag + constructor must brand the computed face as CSSStyleDeclaration"
    );
}

/// Small-caps batch: `font-variant-caps` parses, inherits, rides both
/// shorthands (`font: small-caps …` / `font-variant: small-caps`) and
/// reads back through getComputedStyle as "small-caps"/"normal".
#[cfg(feature = "screenshot")]
#[test]
fn font_variant_caps_parse_inherit_and_computed() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             #t { font-variant-caps: small-caps; }\
             #f { font: small-caps 20px serif; }\
             #v { font-variant: small-caps; }\
             </style></head><body>\
             <div id='t'><span id='c'>x</span></div>\
             <div id='f'>y</div><div id='v'>z</div><div id='n'>w</div>\
             </body></html>",
    );
    let mut gcs = |sel: &str, expr: &str| {
        rt.evaluate(&format!(
            "getComputedStyle(document.getElementById('{}')).{}",
            sel, expr
        ))
        .unwrap()
    };
    assert_eq!(
        gcs("t", "getPropertyValue('font-variant-caps')"),
        serde_json::json!("small-caps")
    );
    assert_eq!(
        gcs("c", "fontVariantCaps"),
        serde_json::json!("small-caps"),
        "font-variant-caps inherits"
    );
    assert_eq!(
        gcs("f", "fontVariantCaps"),
        serde_json::json!("small-caps"),
        "font shorthand carries small-caps"
    );
    assert_eq!(gcs("v", "fontVariantCaps"), serde_json::json!("small-caps"));
    assert_eq!(
        gcs("v", "getPropertyValue('font-variant')"),
        serde_json::json!("small-caps"),
        "font-variant shorthand sets both faces"
    );
    assert_eq!(
        gcs("n", "fontVariantCaps"),
        serde_json::json!("normal"),
        "unset computes to normal"
    );
}

/// Issue #32: legacy reflected attributes (align on div/p/h*) are real
/// [Reflect] DOMString members on the per-tag interfaces — Chrome's
/// Object.keys(HTMLDivElement.prototype) is ["align"] — and
/// ReadableStream gains the enumerable values() member with
/// Symbol.asyncIterator aliased to it (#31's v1 boundary).
#[cfg(feature = "screenshot")]
#[test]
fn reflected_align_and_stream_values() {
    let mut rt = setup_runtime("<html><body><div id='t'>x</div></body></html>");
    let out = rt
        .evaluate(
            r#"
            var d = document.createElement('div');
            d.setAttribute('align', 'center');
            var viaGetter = d.align;
            d.align = 'right';
            var viaSetter = d.getAttribute('align');
            var p = document.createElement('p'); p.align = 'left';
            var h = document.createElement('h2'); h.align = 'top';
            var rs = new ReadableStream({ start: function(c){ c.enqueue('a'); } });
            var it = rs.values();
            return JSON.stringify({
                faces: [
                    Object.keys(HTMLDivElement.prototype),
                    Object.keys(HTMLParagraphElement.prototype),
                    Object.keys(HTMLHeadingElement.prototype),
                    Object.keys(HTMLSpanElement.prototype),
                    Object.keys(ReadableStream.prototype).slice(-1),
                ],
                align: [viaGetter, viaSetter,
                        p.getAttribute('align'), h.getAttribute('align')],
                blank: document.createElement('div').align,
                span: document.createElement('span').align,
                hasValues: typeof rs.values,
                alias: ReadableStream.prototype[Symbol.asyncIterator]
                        === ReadableStream.prototype.values,
                itNext: typeof it.next(),
                itSelf: it[Symbol.asyncIterator]() === it,
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let faces = v["faces"].as_array().unwrap();
    assert_eq!(faces[0], serde_json::json!(["align"]), "div prototype face");
    assert_eq!(faces[1], serde_json::json!(["align"]), "p prototype face");
    assert_eq!(
        faces[2],
        serde_json::json!(["align"]),
        "heading prototype face"
    );
    assert_eq!(faces[3], serde_json::json!([]), "span has no align");
    assert_eq!(
        faces[4],
        serde_json::json!(["values"]),
        "values is the newest RS member"
    );
    assert_eq!(
        v["align"],
        serde_json::json!(["center", "right", "left", "top"]),
        "align get/set round-trips through the content attribute"
    );
    assert_eq!(
        v["blank"],
        serde_json::json!(""),
        "no attribute -> empty string"
    );
    assert_eq!(
        v["span"],
        serde_json::Value::Null,
        "span.align is undefined"
    );
    assert_eq!(v["hasValues"], serde_json::json!("function"));
    assert_eq!(
        v["alias"],
        serde_json::json!(true),
        "Symbol.asyncIterator aliases values"
    );
    assert_eq!(
        v["itNext"],
        serde_json::json!("object"),
        "values().next() is a promise"
    );
    assert_eq!(
        v["itSelf"],
        serde_json::json!(true),
        "iterator is self-async-iterable"
    );
}

#[test]
fn reflected_body_color_and_table_family() {
    let mut rt = setup_runtime(
        "<html><body bgcolor='#eeeeee' text='navy'><div id='t'>x</div></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var b = document.body, t = document.createElement('table');
            var d = document.createElement('div');
            var parsed0 = [b.bgColor, b.text];
            b.text = 'red'; b.aLink = 'maroon'; b.vLink = 'purple';
            b.link = 'blue'; b.background = 'bg.png';
            t.align = 'center'; t.border = '2'; t.bgColor = '#f0f0f0';
            t.cellPadding = '4'; t.cellSpacing = '0';
            return JSON.stringify({
                bodyFace: Object.keys(HTMLBodyElement.prototype).sort(),
                tblFace: Object.keys(HTMLTableElement.prototype).sort(),
                parsed: parsed0,
                bodyAttrs: [b.getAttribute('text'), b.getAttribute('alink'),
                            b.getAttribute('vlink'), b.getAttribute('link'),
                            b.getAttribute('background')],
                tblAttrs: [t.getAttribute('align'), t.getAttribute('border'),
                           t.getAttribute('bgcolor'), t.getAttribute('cellpadding'),
                           t.getAttribute('cellspacing')],
                attrToProp: (t.setAttribute('border', '5'), t.border),
                // [LegacyNullToEmptyString] applies to null only — the
                // [Reflect] plain members coerce null -> "null".
                nullEmpty: [(b.text = null, b.getAttribute('text')),
                            (t.cellPadding = null, t.getAttribute('cellpadding'))],
                plainNull: [(t.border = null, t.getAttribute('border')),
                            (d.align = null, d.getAttribute('align'))],
                undef: (t.border = undefined, t.getAttribute('border')),
                blank: [document.createElement('table').border, b.aLink === 'maroon' && ''],
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["bodyFace"],
        serde_json::json!([
            "aLink",
            "background",
            "bgColor",
            "link",
            // (#37) the 24 window-reflecting on* members are enumerable
            // on the prototype too, exactly like Chrome 152
            "onafterprint",
            "onbeforeprint",
            "onbeforeunload",
            "onblur",
            "onerror",
            "onfocus",
            "ongamepadconnected",
            "ongamepaddisconnected",
            "onhashchange",
            "onlanguagechange",
            "onload",
            "onmessage",
            "onmessageerror",
            "onoffline",
            "ononline",
            "onpagehide",
            "onpageshow",
            "onpopstate",
            "onrejectionhandled",
            "onresize",
            "onscroll",
            "onstorage",
            "onunhandledrejection",
            "onunload",
            "text",
            "vLink"
        ]),
        "body prototype carries the legacy color family + the on* family"
    );
    assert_eq!(
        v["tblFace"],
        serde_json::json!([
            "align",
            "bgColor",
            "border",
            "caption",
            "cellPadding",
            "cellSpacing",
            "createCaption",
            "createTBody",
            "createTFoot",
            "createTHead",
            "deleteCaption",
            "deleteRow",
            "deleteTFoot",
            "deleteTHead",
            "frame",
            "insertRow",
            "rows",
            "rules",
            "summary",
            "tBodies",
            "tFoot",
            "tHead",
            "width"
        ]),
        "table prototype carries the legacy family + table DOM members"
    );
    assert_eq!(
        v["parsed"],
        serde_json::json!(["#eeeeee", "navy"]),
        "content attrs parsed into the document are readable through the accessors"
    );
    assert_eq!(
        v["bodyAttrs"],
        serde_json::json!(["red", "maroon", "purple", "blue", "bg.png"]),
        "IDL writes land under the lowercase content-attr names (alink/vlink)"
    );
    assert_eq!(
        v["tblAttrs"],
        serde_json::json!(["center", "2", "#f0f0f0", "4", "0"]),
        "table IDL writes land under cellpadding/cellspacing"
    );
    assert_eq!(
        v["attrToProp"],
        serde_json::json!("5"),
        "attr write -> accessor read"
    );
    assert_eq!(
        v["nullEmpty"],
        serde_json::json!(["", ""]),
        "[LegacyNullToEmptyString]: null setter -> empty content attribute"
    );
    assert_eq!(
        v["plainNull"],
        serde_json::json!(["null", "null"]),
        "plain [Reflect]: null setter -> \"null\" (incl. #32's div.align correction)"
    );
    assert_eq!(
        v["undef"],
        serde_json::json!("undefined"),
        "undefined -> \"undefined\" in both modes"
    );
    assert_eq!(
        v["blank"],
        serde_json::json!(["", ""]),
        "absent attribute -> empty string getter"
    );
}

#[test]
fn reflected_ulong_and_keyword_families() {
    let mut rt = setup_runtime(
        "<html><body><table><tr><td id='c' colspan='2' rowspan='3' headers='h1 h2'>x</td>\
             <th id='h' scope='col'>H</th></tr></table></body></html>",
    );
    let out = rt.evaluate(r#"
            var td = document.getElementById('c'), th = document.getElementById('h');
            var col = document.createElement('col'), cg = document.createElement('colgroup');
            var fresh = document.createElement('td');
            var defaults = [fresh.colSpan, fresh.rowSpan, fresh.headers, fresh.scope, col.span];
            var parsed = [td.colSpan, td.rowSpan, td.headers, th.scope];
            // GETTER: clamp to the ReflectRange boundaries, default on
            // missing/garbage. HTML non-negative integer parsing: leading
            // whitespace, '+', digits, stop at first non-digit.
            var g = function(el, prop, attr, val) { el.setAttribute(attr, val); return el[prop]; };
            var gets = [
                g(fresh, 'colSpan', 'colspan', '3'), g(fresh, 'colSpan', 'colspan', '0'),
                g(fresh, 'colSpan', 'colspan', '1001'), g(fresh, 'colSpan', 'colspan', '4294967296'),
                g(fresh, 'colSpan', 'colspan', 'abc'), g(fresh, 'colSpan', 'colspan', '-2'),
                g(fresh, 'colSpan', 'colspan', '  7  '), g(fresh, 'colSpan', 'colspan', '+9'),
                g(fresh, 'colSpan', 'colspan', '12abc'), g(fresh, 'colSpan', 'colspan', '3.9'),
                g(fresh, 'rowSpan', 'rowspan', '0'), g(fresh, 'rowSpan', 'rowspan', '65535'),
                g(fresh, 'rowSpan', 'rowspan', '70000'),
                g(col, 'span', 'span', '0'), g(col, 'span', 'span', '1001'),
            ];
            // SETTER: never clamps — ToUint32 (Number(v) >>> 0), written
            // verbatim inside [0, 2^31-1], default written above that.
            var s = function(el, prop, attr, v) { el[prop] = v; var a = el.getAttribute(attr); el.removeAttribute(attr); return a; };
            var sets = [
                s(fresh, 'colSpan', 'colspan', 0), s(fresh, 'colSpan', 'colspan', 5000),
                s(fresh, 'colSpan', 'colspan', -1), s(fresh, 'colSpan', 'colspan', 4294967295),
                s(fresh, 'colSpan', 'colspan', 4294967296), s(fresh, 'colSpan', 'colspan', null),
                s(fresh, 'colSpan', 'colspan', undefined), s(fresh, 'colSpan', 'colspan', '12'),
                s(fresh, 'colSpan', 'colspan', 3.9), s(fresh, 'colSpan', 'colspan', 'abc'),
                s(fresh, 'colSpan', 'colspan', true), s(fresh, 'colSpan', 'colspan', 1e10),
                s(fresh, 'rowSpan', 'rowspan', 0), s(fresh, 'rowSpan', 'rowspan', 70000),
                s(col, 'span', 'span', 0), s(col, 'span', 'span', 2000),
            ];
            // The 0 quirk: setter writes "0" (legal), getter reads 1 (clamped).
            var zeroTrip = (fresh.colSpan = 0, [fresh.getAttribute('colspan'), fresh.colSpan]);
            var overTrip = (fresh.colSpan = 1001, [fresh.getAttribute('colspan'), fresh.colSpan]);
            // scope: lowercase exact keyword match, no "auto", no trimming; setter verbatim.
            var kw = (th.setAttribute('scope', 'ROW'), th.scope);
            var kwAuto = (th.setAttribute('scope', 'auto'), th.scope);
            var kwBogus = (th.setAttribute('scope', 'bogus'), th.scope);
            var kwSpaces = (th.setAttribute('scope', ' col '), th.scope);
            th.scope = 'bogus';
            var kwSetAttr = th.getAttribute('scope');
            th.scope = null;
            var kwSetNull = th.getAttribute('scope');
            var headersNull = (fresh.headers = null, fresh.getAttribute('headers'));
            return JSON.stringify({
                defaults: defaults,
                ifaces: [col.constructor.name, cg.constructor.name],
                colFace: Object.keys(HTMLTableColElement.prototype).sort(),
                parsed: parsed,
                gets: gets, sets: sets,
                trips: [zeroTrip, overTrip],
                kw: [kw, kwAuto, kwBogus, kwSpaces, kwSetAttr, kwSetNull],
                headersNull: headersNull,
            });
        "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["defaults"],
        serde_json::json!([1, 1, "", "", 1]),
        "ReflectDefault: colSpan/rowSpan/span 1, string reflects ''"
    );
    assert_eq!(
        v["ifaces"],
        serde_json::json!(["HTMLTableColElement", "HTMLTableColElement"]),
        "col and colgroup share HTMLTableColElement"
    );
    assert_eq!(
        v["colFace"],
        serde_json::json!(["align", "ch", "chOff", "span", "vAlign", "width"]),
        "col prototype face: legacy five + ulong span"
    );
    assert_eq!(
        v["parsed"],
        serde_json::json!([2, 3, "h1 h2", "col"]),
        "parsed-in colspan/rowspan/headers/scope read through"
    );
    assert_eq!(
        v["gets"],
        serde_json::json!([3, 1, 1000, 1000, 1, 1, 7, 9, 12, 3, 0, 65534, 65534, 1, 1000]),
        "getter clamps to [min,max], defaults on missing/garbage, parses per HTML integer rules"
    );
    assert_eq!(
        v["sets"],
        serde_json::json!([
            "0",
            "5000",
            "1",
            "1",
            "0",
            "0",
            "0",
            "12",
            "3",
            "0",
            "1",
            "1410065408",
            "0",
            "70000",
            "0",
            "2000"
        ]),
        "setter never clamps: ToUint32 verbatim within [0,2^31-1], default above"
    );
    assert_eq!(
        v["trips"],
        serde_json::json!([["0", 1], ["1001", 1000]]),
        "the 0/1001 round trips: attribute keeps what was written, getter clamps"
    );
    assert_eq!(
        v["kw"],
        serde_json::json!(["row", "", "", "", "bogus", "null"]),
        "scope keyword reflect: canonical lowercase match, auto/bogus/spaces -> '', setter verbatim"
    );
    assert_eq!(
        v["headersNull"],
        serde_json::json!("null"),
        "headers is plain reflect: null -> \"null\""
    );
}

#[test]
fn reflected_long_form_control_families() {
    let mut rt = setup_runtime(
        "<html><body><input id='i' size='7' maxlength='5'>\
             <textarea id='t' rows='4' cols='6' wrap='HARD' maxlength='5'>x</textarea></body></html>",
    );
    let out = rt.evaluate(r#"
            var i = document.getElementById('i'), ta = document.getElementById('t');
            var inp = document.createElement('input'), tx = document.createElement('textarea');
            var defaults = [inp.size, inp.maxLength, inp.minLength,
                            tx.rows, tx.cols, tx.wrap, tx.maxLength, tx.minLength];
            var parsed = [i.size, i.maxLength, ta.rows, ta.cols, ta.wrap, ta.maxLength];
            // GETTER: HTML integer parse. size/rows/cols are positive-only
            // (0/garbage/overflow -> default); maxLength is signed with a -1
            // missing-value default (Chrome 152 — MDN's 0 is stale).
            var g = function(el, prop, attr, val) { el.setAttribute(attr, val); return el[prop]; };
            var gets = [
                g(inp, 'size', 'size', '0'), g(inp, 'size', 'size', 'abc'),
                g(inp, 'size', 'size', '-2'), g(inp, 'size', 'size', '  3  '),
                g(inp, 'size', 'size', '12abc'), g(inp, 'size', 'size', '+5'),
                g(inp, 'size', 'size', '2147483647'), g(inp, 'size', 'size', '2147483648'),
                g(inp, 'size', 'size', '3.9'),
                g(inp, 'maxLength', 'maxlength', '0'), g(inp, 'maxLength', 'maxlength', '-2'),
                g(inp, 'maxLength', 'maxlength', 'abc'), g(inp, 'maxLength', 'maxlength', '3.9'),
                g(inp, 'maxLength', 'maxlength', '2147483648'),
                g(tx, 'rows', 'rows', '0'), g(tx, 'rows', 'rows', 'abc'),
                g(tx, 'rows', 'rows', '-2'), g(tx, 'rows', 'rows', '10abc'),
                g(tx, 'cols', 'cols', '0'), g(tx, 'cols', 'cols', '+33'),
            ];
            // SETTER: three behaviors — size THROWS on a ToUint32 of 0,
            // maxLength/minLength THROW on a ToInt32 negative (including -1,
            // the getter's own default), rows/cols never throw and write the
            // DEFAULT string on 0 or 2^31 overflow.
            var T = function(f) { try { f(); return 'ok'; } catch(e) { return 'THROW:'+e.name; } };
            var s = function(el, prop, attr, v) {
                el.removeAttribute(attr);
                var r = T(function(){ el[prop] = v; });
                var a = el.getAttribute(attr);
                return [r, a];
            };
            var sets = [
                s(inp, 'size', 'size', 0), s(inp, 'size', 'size', 1e10),
                s(inp, 'size', 'size', -1), s(inp, 'size', 'size', 12),
                s(inp, 'maxLength', 'maxlength', 0), s(inp, 'maxLength', 'maxlength', -5),
                s(inp, 'maxLength', 'maxlength', -1), s(inp, 'maxLength', 'maxlength', 2147483648),
                s(inp, 'maxLength', 'maxlength', null), s(inp, 'maxLength', 'maxlength', '5x'),
                s(inp, 'maxLength', 'maxlength', 3.9),
                s(tx, 'minLength', 'minlength', -1), s(tx, 'minLength', 'minlength', 5),
                s(tx, 'rows', 'rows', 0), s(tx, 'rows', 'rows', null),
                s(tx, 'rows', 'rows', 1e10), s(tx, 'rows', 'rows', -2),
                s(tx, 'cols', 'cols', 0), s(tx, 'cols', 'cols', 40),
            ];
            // The throw is an IndexSizeError naming the receiving interface.
            var errMsg = (function(){ try { inp.size = 0; } catch(e) { return e.name + '|' + e.message.indexOf('HTMLInputElement'); } return '?'; })();
            // wrap is a PLAIN reflect: verbatim getter, String(v) setter.
            ta.setAttribute('wrap', ' hard ');
            var wVerbatim = ta.wrap;
            ta.wrap = 'soft';
            var wSet = ta.getAttribute('wrap');
            ta.wrap = null;
            var wSetNull = ta.getAttribute('wrap');
            // Negative maxlength attribute reads back as the default.
            ta.setAttribute('maxlength', '-5');
            var tripNeg = ta.maxLength;
            var inFace = Object.keys(HTMLInputElement.prototype);
            var taFace = Object.keys(HTMLTextAreaElement.prototype);
            return JSON.stringify({
                defaults: defaults,
                ifaces: [i.constructor.name, ta.constructor.name],
                parsed: parsed,
                gets: gets, sets: sets,
                errMsg: errMsg,
                wrap: [wVerbatim, wSet, wSetNull],
                tripNeg: tripNeg,
                faces: [
                    ['size', 'maxLength', 'minLength'].every(function(k){ return inFace.indexOf(k) >= 0; }),
                    ['rows', 'cols', 'wrap'].every(function(k){ return inFace.indexOf(k) < 0; }),
                    ['rows', 'cols', 'wrap', 'maxLength', 'minLength'].every(function(k){ return taFace.indexOf(k) >= 0; }),
                ],
            });
        "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["defaults"],
        serde_json::json!([20, -1, -1, 2, 20, "", -1, -1]),
        "ReflectDefault: size 20, maxLength/minLength -1 (not MDN's 0), rows 2, cols 20, wrap ''"
    );
    assert_eq!(
        v["ifaces"],
        serde_json::json!(["HTMLInputElement", "HTMLTextAreaElement"])
    );
    assert_eq!(
        v["parsed"],
        serde_json::json!([7, 5, 4, 6, "HARD", 5]),
        "parsed-in size/maxlength/rows/cols/wrap read through (#36)"
    );
    assert_eq!(
        v["gets"],
        serde_json::json!([
            20, 20, 20, 3, 12, 5, 2147483647, 20, 3, 0, -1, -1, 3, -1, 2, 2, 2, 10, 20, 33
        ]),
        "positive-only parse defaults on 0/garbage/overflow; maxLength keeps 0 and defaults -1"
    );
    assert_eq!(
        v["sets"],
        serde_json::json!([
            ["THROW:IndexSizeError", null],
            ["ok", "1410065408"],
            ["ok", "20"],
            ["ok", "12"],
            ["ok", "0"],
            ["THROW:IndexSizeError", null],
            ["THROW:IndexSizeError", null],
            ["THROW:IndexSizeError", null],
            ["ok", "0"],
            ["ok", "0"],
            ["ok", "3"],
            ["THROW:IndexSizeError", null],
            ["ok", "5"],
            ["ok", "2"],
            ["ok", "2"],
            ["ok", "1410065408"],
            ["ok", "2"],
            ["ok", "20"],
            ["ok", "40"]
        ]),
        "size throws on 0; maxLength throws on ToInt32 negatives; rows/cols write defaults"
    );
    assert!(
        v["errMsg"].as_str().unwrap().starts_with("IndexSizeError|"),
        "the throw is an IndexSizeError naming HTMLInputElement, got {:?}",
        v["errMsg"]
    );
    assert_eq!(
        v["wrap"],
        serde_json::json!([" hard ", "soft", "null"]),
        "wrap is a plain reflect: no keyword folding, null -> \"null\""
    );
    assert_eq!(
        v["tripNeg"],
        serde_json::json!(-1),
        "maxlength='-5' attribute reads back as the -1 default"
    );
    assert_eq!(
        v["faces"],
        serde_json::json!([true, true, true]),
        "input face gains size/maxLength/minLength only; textarea gains all five"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn computed_style_width_resolves_past_the_inline_surface() {
    let mut rt = setup_runtime(
        "<html><body><div id=\"a\">alpha</div><div id=\"b\" style=\"width: 123px\">bravo</div></body></html>",
    );
    let checks = rt
        .evaluate(
            r#"
            const a = document.getElementById("a"), b = document.getElementById("b");
            return [
                getComputedStyle(a).width,
                getComputedStyle(b).width,
                typeof getComputedStyle(a).getPropertyValue,
                typeof getComputedStyle(a).length,
            ];
        "#,
        )
        .unwrap();
    let parts = checks.as_array().expect("array result");
    let block_w = parts[0].as_str().expect("geometry-backed width string");
    assert!(
        block_w.ends_with("px") && block_w != "0px",
        "width comes from the layout rect, not the empty inline value"
    );
    assert_eq!(
        parts[1],
        serde_json::json!("123px"),
        "inline width still wins the cascade-less approximation"
    );
    assert_eq!(
        parts[2],
        serde_json::json!("function"),
        "interface methods still route to the target"
    );
    assert_eq!(
        parts[3],
        serde_json::json!("number"),
        "interface members still route to the target"
    );
}

/// getComputedStyle overflow face pinned to the Chrome probe matrix
/// (css-overflow-3): the pair form reads as "x y", single keywords stay
/// single, and the §3.1 coercion surfaces through overflowY ("auto").
#[cfg(feature = "screenshot")]
#[test]
fn computed_style_overflow_per_axis_matrix() {
    let mut rt = setup_runtime(
        r#"<body><div id="a" style="overflow: clip"></div><div id="b" style="overflow: hidden auto"></div><div id="c" style="overflow-x: hidden"></div><div id="d"></div></body>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const g = id => getComputedStyle(document.getElementById(id));
            return {
                a: g('a').overflow, aX: g('a').overflowX,
                b: g('b').overflow, bX: g('b').overflowX, bY: g('b').overflowY,
                c: g('c').overflow, cX: g('c').overflowX, cY: g('c').overflowY,
                d: g('d').overflow,
            };
        "#,
        )
        .unwrap();
    let v = result;
    assert_eq!(
        v["a"],
        serde_json::json!("clip"),
        "single keyword serializes alone"
    );
    assert_eq!(v["aX"], serde_json::json!("clip"));
    assert_eq!(
        v["b"],
        serde_json::json!("hidden auto"),
        "pair form reads back as pair"
    );
    assert_eq!(v["bX"], serde_json::json!("hidden"));
    assert_eq!(v["bY"], serde_json::json!("auto"));
    assert_eq!(
        v["c"],
        serde_json::json!("hidden auto"),
        "§3.1: visible axis coerces to auto"
    );
    assert_eq!(v["cX"], serde_json::json!("hidden"));
    assert_eq!(
        v["cY"],
        serde_json::json!("auto"),
        "Chrome reports the coerced pair"
    );
    assert_eq!(
        v["d"],
        serde_json::json!("visible"),
        "default stays visible"
    );
}

// Regression (companion to obscura #738): getComputedStyle consulted
// inline styles, dimensions and a defaults table — but never the
// stylesheet cascade, so a z-index/position set in a <style> block
// read back as "auto"/"static" no matter what the sheet said. Any
// script branching on computed layout properties (jQuery .css(),
// overlay/positioning logic) took the wrong branch silently.
#[test]
#[cfg(feature = "screenshot")]
fn test_get_computed_style_reads_the_stylesheet_cascade() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .panel { position: absolute; z-index: 42; display: flex; color: rgb(1, 2, 3); }
          .plain { color: #abcdef; }
        </style></head><body>
        <div class="panel" id="p"><span class="plain" id="s">x</span></div>
        </body></html>"#,
    );
    let out = rt
            .evaluate(
                r#"JSON.stringify({
                    pos: getComputedStyle(document.getElementById('p')).position,
                    z: getComputedStyle(document.getElementById('p')).zIndex,
                    disp: getComputedStyle(document.getElementById('p')).display,
                    color: getComputedStyle(document.getElementById('p')).color,
                    camel: getComputedStyle(document.getElementById('p')).zIndex,
                    viaCall: getComputedStyle(document.getElementById('p')).getPropertyValue('z-index'),
                })"#,
            )
            .unwrap();
    println!("gcs cascade diagnostics: {out}");
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["pos"], serde_json::json!("absolute"));
    assert_eq!(v["z"], serde_json::json!("42"));
    assert_eq!(v["camel"], serde_json::json!("42"));
    assert_eq!(v["viaCall"], serde_json::json!("42"));
    assert_eq!(v["disp"], serde_json::json!("flex"));
    assert_eq!(v["color"], serde_json::json!("rgb(1, 2, 3)"));
    // Inline still wins the cascade over the stylesheet rule.
    let inline = rt
        .evaluate(
            r#"(function() {
                    const p = document.getElementById('p');
                    p.style.zIndex = '7';
                    return getComputedStyle(p).zIndex;
                })()"#,
        )
        .unwrap();
    assert_eq!(inline, serde_json::json!("7"));
    // A property no rule targets keeps its initial value, not garbage.
    let initial = rt
        .evaluate("getComputedStyle(document.getElementById('s')).zIndex")
        .unwrap();
    assert_eq!(initial, serde_json::json!("auto"));
}

// Regression (obscura #771 wrong-value rows): getComputedStyle served the
// layout engine's Block bucket for the table family and `stretch` /
// `flex-start` for unset align-items/justify-content. Chrome answers
// table/table-row/table-cell/list-item and `normal` — an element's box
// type was otherwise unreadable over CDP. An author `display: block`
// (responsive table collapses) must still win over the UA table.
#[test]
#[cfg(feature = "screenshot")]
fn test_get_computed_style_table_family_and_flex_initials() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .flat { display: block; }
        </style></head><body>
        <table><tr><td id="cell">a</td><td class="flat" id="flat">b</td></tr></table>
        <ul><li id="item">x</li></ul>
        <div id="box"><span id="in">y</span></div>
        </body></html>"#,
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify({
                    table: getComputedStyle(document.querySelector('table')).display,
                    row: getComputedStyle(document.querySelector('tr')).display,
                    cell: getComputedStyle(document.getElementById('cell')).display,
                    authorBlock: getComputedStyle(document.getElementById('flat')).display,
                    listItem: getComputedStyle(document.getElementById('item')).display,
                    div: getComputedStyle(document.getElementById('box')).display,
                    span: getComputedStyle(document.getElementById('in')).display,
                    alignNormal: getComputedStyle(document.getElementById('box')).alignItems,
                    justifyNormal: getComputedStyle(document.getElementById('box')).justifyContent,
                })"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["table"], serde_json::json!("table"));
    assert_eq!(v["row"], serde_json::json!("table-row"));
    assert_eq!(v["cell"], serde_json::json!("table-cell"));
    assert_eq!(v["authorBlock"], serde_json::json!("block"));
    assert_eq!(v["listItem"], serde_json::json!("list-item"));
    assert_eq!(v["div"], serde_json::json!("block"));
    assert_eq!(v["span"], serde_json::json!("inline"));
    assert_eq!(v["alignNormal"], serde_json::json!("normal"));
    assert_eq!(v["justifyNormal"], serde_json::json!("normal"));
}

// Regression (obscura #771 empty-value rows): 41 computed properties read
// back '' on an unstyled element; '' is indistinguishable from "not set",
// so feature probes silently took the wrong branch. The defaults table
// now carries Chrome's initial values (audited against Chromium 147).
#[test]
fn test_get_computed_style_initial_values_not_empty() {
    let mut rt = setup_runtime(
        r#"<html><body><div id="d" style="width:100px;height:40px">x</div></body></html>"#,
    );
    let out = rt
            .evaluate(
                r#"JSON.stringify({
                    bg: getComputedStyle(document.getElementById('d')).backgroundImage,
                    bgPos: getComputedStyle(document.getElementById('d')).backgroundPosition,
                    bgRepeat: getComputedStyle(document.getElementById('d')).backgroundRepeat,
                    fontStyle: getComputedStyle(document.getElementById('d')).fontStyle,
                    flexGrow: getComputedStyle(document.getElementById('d')).flexGrow,
                    flexShrink: getComputedStyle(document.getElementById('d')).flexShrink,
                    flexBasis: getComputedStyle(document.getElementById('d')).flexBasis,
                    transProp: getComputedStyle(document.getElementById('d')).transitionProperty,
                    animName: getComputedStyle(document.getElementById('d')).animationName,
                    animIter: getComputedStyle(document.getElementById('d')).animationIterationCount,
                    animTiming: getComputedStyle(document.getElementById('d')).animationTimingFunction,
                    userSelect: getComputedStyle(document.getElementById('d')).userSelect,
                    direction: getComputedStyle(document.getElementById('d')).direction,
                    zoom: getComputedStyle(document.getElementById('d')).zoom,
                    minHeight: getComputedStyle(document.getElementById('d')).minHeight,
                    order: getComputedStyle(document.getElementById('d')).order,
                    objectFit: getComputedStyle(document.getElementById('d')).objectFit,
                    aspectRatio: getComputedStyle(document.getElementById('d')).aspectRatio,
                    outlineWidth: getComputedStyle(document.getElementById('d')).outlineWidth,
                    letterSpacing: getComputedStyle(document.getElementById('d')).letterSpacing,
                    wordBreak: getComputedStyle(document.getElementById('d')).wordBreak,
                    overflowWrap: getComputedStyle(document.getElementById('d')).overflowWrap,
                    hyphens: getComputedStyle(document.getElementById('d')).hyphens,
                    tabSize: getComputedStyle(document.getElementById('d')).tabSize,
                    listImage: getComputedStyle(document.getElementById('d')).listStyleImage,
                    bgOrigin: getComputedStyle(document.getElementById('d')).backgroundOrigin,
                    borderImage: getComputedStyle(document.getElementById('d')).borderImageSource,
                    objectPos: getComputedStyle(document.getElementById('d')).objectPosition,
                    gridFlow: getComputedStyle(document.getElementById('d')).gridAutoFlow,
                    alignContent: getComputedStyle(document.getElementById('d')).alignContent,
                    justifyItems: getComputedStyle(document.getElementById('d')).justifyItems,
                    fontStretch: getComputedStyle(document.getElementById('d')).fontStretch,
                    transformStyle: getComputedStyle(document.getElementById('d')).transformStyle,
                    touchAction: getComputedStyle(document.getElementById('d')).touchAction,
                    quotes: getComputedStyle(document.getElementById('d')).quotes,
                    verticalAlign: getComputedStyle(document.getElementById('d')).verticalAlign,
                    radius: getComputedStyle(document.getElementById('d')).borderTopLeftRadius,
                    maxWidth: getComputedStyle(document.getElementById('d')).maxWidth,
                    rotate: getComputedStyle(document.getElementById('d')).rotate,
                    scale: getComputedStyle(document.getElementById('d')).scale,
                    translate: getComputedStyle(document.getElementById('d')).translate,
                    fontOpt: getComputedStyle(document.getElementById('d')).fontOpticalSizing,
                    maskClip: getComputedStyle(document.getElementById('d')).maskClip,
                    maskComp: getComputedStyle(document.getElementById('d')).maskComposite,
                    wrapMode: getComputedStyle(document.getElementById('d')).textWrapMode,
                    colFill: getComputedStyle(document.getElementById('d')).columnFill,
                    snap: getComputedStyle(document.getElementById('d')).scrollSnapType,
                    blockW: getComputedStyle(document.getElementById('d')).borderBlockStartWidth,
                    blockS: getComputedStyle(document.getElementById('d')).borderBlockStartStyle,
                    blockC: getComputedStyle(document.getElementById('d')).borderBlockStartColor,
                    scrollM: getComputedStyle(document.getElementById('d')).scrollMarginTop,
                    scrollP: getComputedStyle(document.getElementById('d')).scrollPaddingTop,
                    justifySelf: getComputedStyle(document.getElementById('d')).justifySelf,
                    gridCol: getComputedStyle(document.getElementById('d')).gridColumnStart,
                    textRender: getComputedStyle(document.getElementById('d')).textRendering,
                    stroke: getComputedStyle(document.getElementById('d')).stroke,
                    fill: getComputedStyle(document.getElementById('d')).fill,
                    strokeW: getComputedStyle(document.getElementById('d')).strokeWidth,
                    webkitStroke: getComputedStyle(document.getElementById('d')).webkitTextStrokeWidth,
                    boxOrient: getComputedStyle(document.getElementById('d')).webkitBoxOrient,
                    boxPack: getComputedStyle(document.getElementById('d')).webkitBoxPack,
                    boxFlex: getComputedStyle(document.getElementById('d')).webkitBoxFlex,
                    animTl: getComputedStyle(document.getElementById('d')).animationTimeline,
                    vtName: getComputedStyle(document.getElementById('d')).viewTransitionName,
                    scrollTl: getComputedStyle(document.getElementById('d')).scrollTimelineAxis,
                    marker: getComputedStyle(document.getElementById('d')).markerStart,
                    pathD: getComputedStyle(document.getElementById('d')).d,
                    flood: getComputedStyle(document.getElementById('d')).floodColor,
                    mathStyle: getComputedStyle(document.getElementById('d')).mathStyle,
                    field: getComputedStyle(document.getElementById('d')).fieldSizing,
                    clip: getComputedStyle(document.getElementById('d')).clip,
                    containW: getComputedStyle(document.getElementById('d')).containIntrinsicWidth,
                    ruby: getComputedStyle(document.getElementById('d')).rubyPosition,
                    anchor: getComputedStyle(document.getElementById('d')).positionAnchor,
                    area: getComputedStyle(document.getElementById('d')).positionArea,
                    snapStop: getComputedStyle(document.getElementById('d')).scrollSnapStop,
                    inset: getComputedStyle(document.getElementById('d')).insetBlockStart,
                    cx: getComputedStyle(document.getElementById('d')).cx,
                    rx: getComputedStyle(document.getElementById('d')).rx,
                    barColor: getComputedStyle(document.getElementById('d')).scrollbarColor,
                    caretShape: getComputedStyle(document.getElementById('d')).caretShape,
                    fontPal: getComputedStyle(document.getElementById('d')).fontPalette,
                    maskSize: getComputedStyle(document.getElementById('d')).maskSize,
                    maskPos: getComputedStyle(document.getElementById('d')).maskPosition,
                    logicalR: getComputedStyle(document.getElementById('d')).borderStartStartRadius,
                    marginTrim: getComputedStyle(document.getElementById('d')).marginTrim,
                    readFlow: getComputedStyle(document.getElementById('d')).readingFlow,
                    scrollInit: getComputedStyle(document.getElementById('d')).scrollInitialTarget,
                    interp: getComputedStyle(document.getElementById('d')).interpolateSize,
                    webkitOri: getComputedStyle(document.getElementById('d')).webkitTextOrientation,
                    anchorName: getComputedStyle(document.getElementById('d')).anchorName,
                    corner: getComputedStyle(document.getElementById('d')).cornerTopLeftShape,
                    maxInline: getComputedStyle(document.getElementById('d')).maxInlineSize,
                    minInline: getComputedStyle(document.getElementById('d')).minInlineSize,
                    textWrap: getComputedStyle(document.getElementById('d')).textWrapStyle,
                    vtClass: getComputedStyle(document.getElementById('d')).viewTransitionClass,
                    maskX: getComputedStyle(document.getElementById('d')).webkitMaskPositionX,
                    reflect: getComputedStyle(document.getElementById('d')).webkitBoxReflect,
                    rowRule: getComputedStyle(document.getElementById('d')).rowRuleStyle,
                    baseSrc: getComputedStyle(document.getElementById('d')).baselineSource,
                    colWrap: getComputedStyle(document.getElementById('d')).columnWrap,
                    range: getComputedStyle(document.getElementById('d')).dynamicRangeLimit,
                    tap: getComputedStyle(document.getElementById('d')).webkitTapHighlightColor,
                    rowColor: getComputedStyle(document.getElementById('d')).rowRuleColor,
                    textFit: getComputedStyle(document.getElementById('d')).textFit,
                    markerGroup: getComputedStyle(document.getElementById('d')).scrollMarkerGroup,
                    origin: getComputedStyle(document.getElementById('d')).transformOrigin,
                })"#,
            )
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["bg"], serde_json::json!("none"));
    assert_eq!(v["bgPos"], serde_json::json!("0% 0%"));
    assert_eq!(v["bgRepeat"], serde_json::json!("repeat"));
    assert_eq!(v["fontStyle"], serde_json::json!("normal"));
    assert_eq!(v["flexGrow"], serde_json::json!("0"));
    assert_eq!(v["flexShrink"], serde_json::json!("1"));
    assert_eq!(v["flexBasis"], serde_json::json!("auto"));
    assert_eq!(v["transProp"], serde_json::json!("all"));
    assert_eq!(v["animName"], serde_json::json!("none"));
    assert_eq!(v["animIter"], serde_json::json!("1"));
    assert_eq!(v["animTiming"], serde_json::json!("ease"));
    assert_eq!(v["userSelect"], serde_json::json!("auto"));
    assert_eq!(v["direction"], serde_json::json!("ltr"));
    assert_eq!(v["zoom"], serde_json::json!("1"));
    assert_eq!(v["minHeight"], serde_json::json!("0px"));
    assert_eq!(v["order"], serde_json::json!("0"));
    assert_eq!(v["objectFit"], serde_json::json!("fill"));
    assert_eq!(v["aspectRatio"], serde_json::json!("auto"));
    // Verified against local Chromium: the computed value stays `medium`
    // (3px) even with outline-style none — not the used value 0px.
    assert_eq!(v["outlineWidth"], serde_json::json!("3px"));
    assert_eq!(v["letterSpacing"], serde_json::json!("normal"));
    assert_eq!(v["wordBreak"], serde_json::json!("normal"));
    assert_eq!(v["overflowWrap"], serde_json::json!("normal"));
    assert_eq!(v["hyphens"], serde_json::json!("manual"));
    assert_eq!(v["tabSize"], serde_json::json!("8"));
    assert_eq!(v["listImage"], serde_json::json!("none"));
    assert_eq!(v["bgOrigin"], serde_json::json!("padding-box"));
    assert_eq!(v["borderImage"], serde_json::json!("none"));
    assert_eq!(v["objectPos"], serde_json::json!("50% 50%"));
    assert_eq!(v["gridFlow"], serde_json::json!("row"));
    assert_eq!(v["alignContent"], serde_json::json!("normal"));
    assert_eq!(v["justifyItems"], serde_json::json!("normal"));
    assert_eq!(v["fontStretch"], serde_json::json!("100%"));
    assert_eq!(v["transformStyle"], serde_json::json!("flat"));
    assert_eq!(v["touchAction"], serde_json::json!("auto"));
    assert_eq!(v["quotes"], serde_json::json!("auto"));
    assert_eq!(v["verticalAlign"], serde_json::json!("baseline"));
    assert_eq!(v["radius"], serde_json::json!("0px"));
    assert_eq!(v["maxWidth"], serde_json::json!("none"));
    assert_eq!(v["rotate"], serde_json::json!("none"));
    assert_eq!(v["scale"], serde_json::json!("none"));
    assert_eq!(v["translate"], serde_json::json!("none"));
    assert_eq!(v["fontOpt"], serde_json::json!("auto"));
    assert_eq!(v["maskClip"], serde_json::json!("border-box"));
    assert_eq!(v["maskComp"], serde_json::json!("add"));
    assert_eq!(v["wrapMode"], serde_json::json!("wrap"));
    assert_eq!(v["colFill"], serde_json::json!("balance"));
    assert_eq!(v["snap"], serde_json::json!("none"));
    assert_eq!(v["blockW"], serde_json::json!("0px"));
    assert_eq!(v["blockS"], serde_json::json!("none"));
    assert_eq!(v["blockC"], serde_json::json!("rgb(0, 0, 0)"));
    assert_eq!(v["scrollM"], serde_json::json!("0px"));
    assert_eq!(v["scrollP"], serde_json::json!("auto"));
    assert_eq!(v["justifySelf"], serde_json::json!("auto"));
    assert_eq!(v["gridCol"], serde_json::json!("auto"));
    assert_eq!(v["textRender"], serde_json::json!("auto"));
    assert_eq!(v["stroke"], serde_json::json!("none"));
    assert_eq!(v["fill"], serde_json::json!("rgb(0, 0, 0)"));
    assert_eq!(v["strokeW"], serde_json::json!("1px"));
    assert_eq!(v["webkitStroke"], serde_json::json!("0px"));
    assert_eq!(v["boxOrient"], serde_json::json!("horizontal"));
    assert_eq!(v["boxPack"], serde_json::json!("start"));
    assert_eq!(v["boxFlex"], serde_json::json!("0"));
    assert_eq!(v["animTl"], serde_json::json!("auto"));
    assert_eq!(v["vtName"], serde_json::json!("none"));
    assert_eq!(v["scrollTl"], serde_json::json!("block"));
    assert_eq!(v["marker"], serde_json::json!("none"));
    assert_eq!(v["pathD"], serde_json::json!("none"));
    assert_eq!(v["flood"], serde_json::json!("rgb(0, 0, 0)"));
    assert_eq!(v["mathStyle"], serde_json::json!("normal"));
    assert_eq!(v["field"], serde_json::json!("fixed"));
    assert_eq!(v["clip"], serde_json::json!("auto"));
    assert_eq!(v["containW"], serde_json::json!("none"));
    assert_eq!(v["ruby"], serde_json::json!("over"));
    assert_eq!(v["anchor"], serde_json::json!("normal"));
    assert_eq!(v["area"], serde_json::json!("none"));
    assert_eq!(v["snapStop"], serde_json::json!("normal"));
    assert_eq!(v["inset"], serde_json::json!("auto"));
    assert_eq!(v["cx"], serde_json::json!("0px"));
    assert_eq!(v["rx"], serde_json::json!("auto"));
    assert_eq!(v["barColor"], serde_json::json!("auto"));
    assert_eq!(v["caretShape"], serde_json::json!("auto"));
    assert_eq!(v["fontPal"], serde_json::json!("normal"));
    assert_eq!(v["maskSize"], serde_json::json!("auto"));
    assert_eq!(v["maskPos"], serde_json::json!("0% 0%"));
    assert_eq!(v["logicalR"], serde_json::json!("0px"));
    assert_eq!(v["marginTrim"], serde_json::json!("none"));
    assert_eq!(v["readFlow"], serde_json::json!("normal"));
    assert_eq!(v["scrollInit"], serde_json::json!("none"));
    assert_eq!(v["interp"], serde_json::json!("numeric-only"));
    assert_eq!(v["webkitOri"], serde_json::json!("vertical-right"));
    assert_eq!(v["anchorName"], serde_json::json!("none"));
    assert_eq!(v["corner"], serde_json::json!("superellipse(1)"));
    assert_eq!(v["maxInline"], serde_json::json!("none"));
    assert_eq!(v["minInline"], serde_json::json!("0px"));
    assert_eq!(v["textWrap"], serde_json::json!("auto"));
    assert_eq!(v["vtClass"], serde_json::json!("none"));
    assert_eq!(v["maskX"], serde_json::json!("0%"));
    assert_eq!(v["reflect"], serde_json::json!("none"));
    assert_eq!(v["rowRule"], serde_json::json!("none"));
    assert_eq!(v["baseSrc"], serde_json::json!("auto"));
    assert_eq!(v["colWrap"], serde_json::json!("auto"));
    assert_eq!(v["range"], serde_json::json!("no-limit"));
    assert_eq!(v["tap"], serde_json::json!("rgba(0, 0, 0, 0.18)"));
    assert_eq!(v["rowColor"], serde_json::json!("rgb(0, 0, 0)"));
    assert_eq!(v["textFit"], serde_json::json!("none"));
    assert_eq!(v["markerGroup"], serde_json::json!("none"));
    assert_eq!(v["origin"], serde_json::json!("50px 20px"));
}

/// Upstream a0e1ba5: CSSStyleDeclaration is a real global interface — the
/// type of element.style — not merely pre-declared.
#[test]
fn cssstyledeclaration_is_a_usable_global_interface() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
            .evaluate(
                "(function(){var d=Object.getOwnPropertyDescriptor(window,'CSSStyleDeclaration');return (typeof window.CSSStyleDeclaration)+'|'+(document.body.style instanceof CSSStyleDeclaration)+'|'+(d?d.enumerable:'missing');})()",
            )
            .unwrap();
    assert_eq!(v, serde_json::json!("function|true|false"));
}

#[test]
fn test_style_in_and_object_keys_cssom_parity() {
    // el.style was a bare get/set proxy: `'color' in el.style`,
    // Object.keys(el.style), and camelCase↔dashed sync all failed.
    let mut rt = setup_runtime(r#"<div id="el"></div>"#);
    let result = rt
        .evaluate(
            r#"
            const s = document.getElementById('el').style;
            s.fontSize = '20px';
            const keys = Object.keys(s);
            return [
                'color' in s,
                'gap' in s,
                'object-fit' in s,
                s.getPropertyValue('font-size'),
                s.fontSize,
                keys.includes('color'),
                keys.includes('fontSize'),
                s.cssText,
                s.length,
                s.item(0),
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true,
            true,
            true,
            "20px",
            "20px",
            true,
            true,
            "font-size: 20px;",
            1,
            "font-size"
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn css_wide_keywords_initial_inherit_unset() {
    // #218 / takumi #1795: initial/inherit/unset used to fall through
    // every per-property parser and get dropped — invisible wherever the
    // property is naturally inherited (dropped ≈ inherit), but
    // color:initial kept the parent's red and padding:inherit lost the
    // parent's 8px. Chrome oracle: initial = the property's initial
    // value, inherit = the parent's computed value, unset = inherited
    // bit picks. var() delivery counts too (Chrome applies keywords
    // arriving through substitution).
    let mut rt = setup_runtime(
        r#"<html><body>
<div id="p" style="color: rgb(255,0,0); font-size: 24px; font-weight: 700">
<span id="inh" style="color: inherit">x</span>
<span id="ini" style="color: initial">x</span>
<span id="uns" style="color: unset">x</span>
<span id="varini" style="--k: initial; color: var(--k)">x</span><span id="allini" style="color: initial; all: initial">x</span>
<span id="padp" style="padding: 8px"><b id="padini" style="padding: initial">y</b><b id="padinh" style="padding: inherit">y</b></span>
<span id="linp" style="line-height: 40px"><b id="linini" style="line-height: initial">y</b><b id="linh" style="line-height: inherit">y</b></span>
<b id="fwini" style="font-weight: initial">y</b>
<b id="fwinh" style="font-weight: inherit">y</b>
<span id="bgp" style="background-color: rgb(255,255,0)"><b id="bginh" style="background-color: inherit">y</b></span>
</div>
</body></html>"#,
    );
    let script = r#"async () => {
            const c = id => getComputedStyle(document.getElementById(id));
            return {
                inh: c('inh').color, ini: c('ini').color, uns: c('uns').color,
                varini: c('varini').color, allini: c('allini').color,
                padini: c('padini').paddingTop, padinh: c('padinh').paddingTop,
                linini: c('linini').lineHeight, linh: c('linh').lineHeight,
                fwini: c('fwini').fontWeight, fwinh: c('fwinh').fontWeight,
                bginh: c('bginh').backgroundColor,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["inh"],
        serde_json::json!("rgb(255, 0, 0)"),
        "inherit copies the parent"
    );
    assert_eq!(
        v["ini"],
        serde_json::json!("rgb(0, 0, 0)"),
        "initial beats inheritance: black, not parent red"
    );
    assert_eq!(
        v["uns"],
        serde_json::json!("rgb(255, 0, 0)"),
        "unset on an inherited property = inherit"
    );
    // --k: initial empties the slot; var(--k) with no fallback is then
    // IACVT, and IACVT on an inherited property takes the INHERITED
    // value (css-variables §3.1) — Chrome shows the parent red here.
    assert_eq!(
        v["varini"],
        serde_json::json!("rgb(255, 0, 0)"),
        "var() over a guaranteed-invalid custom property is IACVT, not the initial keyword"
    );
    assert_eq!(
        v["allini"],
        serde_json::json!("rgb(0, 0, 0)"),
        "all: initial resets color"
    );
    assert_eq!(
        v["padini"],
        serde_json::json!("0px"),
        "padding: initial is 0"
    );
    assert_eq!(
        v["padinh"],
        serde_json::json!("8px"),
        "padding: inherit copies the parent (non-inherited prop)"
    );
    assert_eq!(
        v["linini"],
        serde_json::json!("normal"),
        "line-height: initial is normal"
    );
    assert_eq!(
        v["linh"],
        serde_json::json!("40px"),
        "line-height: inherit copies the parent"
    );
    assert_eq!(
        v["fwini"],
        serde_json::json!("400"),
        "font-weight: initial is 400"
    );
    assert_eq!(
        v["fwinh"],
        serde_json::json!("700"),
        "font-weight: inherit copies the parent"
    );
    assert_eq!(
        v["bginh"],
        serde_json::json!("rgb(255, 255, 0)"),
        "background-color: inherit copies the parent"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn mask_image_computed_spelling_and_shorthand() {
    // #22: mask-image was unmodeled, so gCS fell through to the inline echo
    // in computed_style.js and served the DECLARED spelling — the battery
    // face was `linear-gradient(black,transparent)` where Chrome re-serializes
    // every stop (the #17 computed_image_text pass, which until now only fed
    // background-image). Chrome truth captured 2026-11-06: longhand and the
    // `mask:` shorthand both normalize; the initial/unset answer is `none`
    // (not ""); an invalid image value (`red`) drops the declaration like
    // Chrome's invalid-declaration recovery.
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
<p id="p" style="mask-image:linear-gradient(black,transparent)">x</p>
<p id="q" style="mask:linear-gradient(#f00,#0000)">x</p>
<p id="r">x</p>
<p id="s" style="mask-image:none">x</p>
<p id="t" style="mask:linear-gradient(black,transparent);mask-image:radial-gradient(green,yellow)">x</p>
<p id="u" style="mask-image:red">x</p>
<p id="c" style="clip-path:circle(50%)">x</p>
</body></html>"#,
    );
    let script = r#"async () => {
            const g = id => getComputedStyle(document.getElementById(id));
            return {
                p: g('p').maskImage,
                q: g('q').maskImage,
                unset: g('r').maskImage,
                none: g('s').maskImage,
                t: g('t').maskImage,
                invalid: g('u').maskImage,
                kebab: g('c').getPropertyValue('-webkit-mask-image'),
                clip: g('c').clipPath,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["p"],
        serde_json::json!("linear-gradient(rgb(0, 0, 0), rgba(0, 0, 0, 0))"),
        "longhand stops re-serialize #17-style (Chrome: keywords expand, transparent is rgba(0, 0, 0, 0))"
    );
    assert_eq!(
        v["q"],
        serde_json::json!("linear-gradient(rgb(255, 0, 0), rgba(0, 0, 0, 0))"),
        "mask: shorthand expands into mask-image (Chrome: #f00/#0000 → rgb/rgba)"
    );
    assert_eq!(
        v["unset"],
        serde_json::json!("none"),
        "the initial is none (Chrome answers none, not '')"
    );
    assert_eq!(
        v["none"],
        serde_json::json!("none"),
        "explicit none stays none"
    );
    assert_eq!(
        v["t"],
        serde_json::json!("radial-gradient(rgb(0, 128, 0), rgb(255, 255, 0))"),
        "a later longhand overrides the shorthand's image"
    );
    assert_eq!(
        v["invalid"],
        serde_json::json!("none"),
        "mask-image: red is invalid (a color is not an image) — declaration drops"
    );
    assert_eq!(
        v["kebab"],
        serde_json::json!("none"),
        "-webkit-mask-image serves through the kebab channel (camel webkitMaskImage is the family-wide JS kebab quirk, out of scope)"
    );
    assert_eq!(
        v["clip"],
        serde_json::json!("circle(50%)"),
        "the battery probe's other half: declared clip-path echoes verbatim"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn gcs_camel_webkit_alias_prefix() {
    // #23: the lookup's kebab pass turned webkitLineClamp into
    // "webkit-line-clamp" (no leading dash), so the camel surface of every
    // vendor alias read '' — the snapshot keys all carry the dash. The
    // kebab channel (getPropertyValue('-webkit-…')) always worked. Chrome
    // truth 2026-11-06: declared -webkit-line-clamp:3 → "3", unset →
    // "none", webkitMaskImage initial → "none". webkitTextFillColor and
    // webkitTextStrokeColor follow the used color (currentcolor).
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
<p id="p" style="-webkit-line-clamp:3">x</p>
<p id="r">x</p>
<p id="c" style="color: red">x</p>
<p id="f" style="-webkit-text-fill-color: blue">x</p>
<p id="h" style="-webkit-text-fill-color:#00f">x</p>
<p id="o" style="outline-color:rgba(0,0,255,.5)">x</p>
</body></html>"#,
    );
    let script = r#"async () => {
            const g = id => getComputedStyle(document.getElementById(id));
            return {
                wlc: g('p').webkitLineClamp,
                wlcU: g('r').webkitLineClamp,
                wmiU: g('r').webkitMaskImage,
                kebab: g('r').getPropertyValue('-webkit-line-clamp'),
                wtfc: g('r').webkitTextFillColor,
                stroke: g('r').webkitTextStrokeColor,
                colored: g('c').webkitTextFillColor,
                color: g('c').color,
                explicit: g('f').webkitTextFillColor,
                hex: g('h').webkitTextFillColor,
                alpha: g('o').outlineColor,
                caret: g('r').caretColor,
                accent: g('r').accentColor,
                cols: g('r').columnCount,
                outline: g('r').outlineColor,
                outlineStyle: g('r').outlineStyle,
                outlineOff: g('r').outlineOffset,
                deco: g('r').textDecorationColor,
                decoStyle: g('r').textDecorationStyle,
                emphasis: g('r').textEmphasisStyle,
                emphasisPos: g('r').textEmphasisPosition,
                outlineColored: g('c').outlineColor,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["wlc"],
        serde_json::json!("3"),
        "camel vendor alias reaches the cascade value (was '' pre-#23)"
    );
    assert_eq!(
        v["wlcU"],
        serde_json::json!("none"),
        "unset camel alias serves the initial like Chrome"
    );
    assert_eq!(
        v["wmiU"],
        serde_json::json!("none"),
        "webkitMaskImage camel face lights up #22's arm (was '')"
    );
    assert_eq!(
        v["kebab"],
        serde_json::json!("none"),
        "the kebab channel keeps working"
    );
    assert_eq!(
        v["wtfc"],
        serde_json::json!("rgb(0, 0, 0)"),
        "unset webkitTextFillColor is the used color"
    );
    assert_eq!(
        v["stroke"],
        serde_json::json!("rgb(0, 0, 0)"),
        "unset webkitTextStrokeColor is the used color"
    );
    assert_eq!(
        v["colored"], v["color"],
        "webkitTextFillColor follows a specified color"
    );
    assert_eq!(
        v["explicit"],
        serde_json::json!("rgb(0, 0, 255)"),
        "an explicit named fill color collapses to rgb()"
    );
    assert_eq!(v["hex"], serde_json::json!("rgb(0, 0, 255)"));
    assert_eq!(v["alpha"], serde_json::json!("rgba(0, 0, 255, 0.5)"));
    assert_eq!(v["caret"], serde_json::json!("rgb(0, 0, 0)"));
    assert_eq!(v["accent"], serde_json::json!("auto"));
    assert_eq!(v["cols"], serde_json::json!("auto"));
    assert_eq!(
        v["outline"], v["wtfc"],
        "unset outline-color is the used color"
    );
    assert_eq!(v["outlineStyle"], serde_json::json!("none"));
    assert_eq!(v["outlineOff"], serde_json::json!("0px"));
    assert_eq!(
        v["deco"], v["wtfc"],
        "unset text-decoration-color is the used color"
    );
    assert_eq!(v["decoStyle"], serde_json::json!("solid"));
    assert_eq!(v["emphasis"], serde_json::json!("none"));
    assert_eq!(v["emphasisPos"], serde_json::json!("over"));
    assert_eq!(
        v["outlineColored"], v["color"],
        "outline-color follows a specified color"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn clip_path_computed_serving() {
    // #24: clip-path was unmodeled — the declared spelling only surfaced
    // through the inline echo (coincidence-parity for canonical spellings)
    // and the initial answered '' where Chrome says "none". Raw store +
    // shape-grammar gate now; Chrome truth 2026-11-06: circle(50%) and
    // polygon verbatim, unset/kebab "none", an invalid value drops.
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
<p id="p" style="clip-path:circle(50%)">x</p>
<p id="r">x</p>
<p id="s" style="clip-path:polygon(0 0,100% 0,50% 100%)">x</p>
<p id="t" style="clip-path:red">x</p>
</body></html>"#,
    );
    let script = r#"async () => {
            const g = id => getComputedStyle(document.getElementById(id));
            return {
                circle: g('p').clipPath,
                unset: g('r').clipPath,
                kebab: g('r').getPropertyValue('clip-path'),
                poly: g('s').clipPath,
                bad: g('t').clipPath,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["circle"],
        serde_json::json!("circle(50%)"),
        "declared basic shape serves verbatim"
    );
    assert_eq!(
        v["unset"],
        serde_json::json!("none"),
        "the initial is none (Chrome answers none, not '')"
    );
    assert_eq!(
        v["kebab"],
        serde_json::json!("none"),
        "kebab channel serves the same initial"
    );
    assert_eq!(
        v["poly"],
        serde_json::json!("polygon(0 0,100% 0,50% 100%)"),
        "polygon points pass through verbatim"
    );
    assert_eq!(
        v["bad"],
        serde_json::json!("none"),
        "clip-path: red is not a shape — declaration drops like Chrome"
    );
}

/// white-space / text-overflow ride the computed-style table (blitz#888):
/// the author's declaration reports back verbatim, white-space inherits
/// through the cascade while text-overflow does not, and the geometry
/// keeps the full text (scrollWidth outgrows the clip box — Chrome parity,
/// the ellipsis marker is a paint-time effect).
#[cfg(feature = "screenshot")]
#[test]
fn computed_style_and_geometry_for_nowrap_ellipsis() {
    let mut rt = setup_runtime(
        r#"<div id="clip" style="width:80px;overflow:hidden;text-overflow:ellipsis"><span id="nw" style="white-space:nowrap">alpha beta gamma delta epsilon zeta eta theta</span></div><div id="plain">hi</div>"#,
    );
    let parts = rt
        .evaluate(
            r#"
            const clip = document.getElementById("clip"),
                  nw = document.getElementById("nw"),
                  plain = document.getElementById("plain");
            const csC = getComputedStyle(clip), csN = getComputedStyle(nw), csP = getComputedStyle(plain);
            return [
                csC.getPropertyValue("white-space"), csC.getPropertyValue("text-overflow"),
                csN.whiteSpace, csN.textOverflow,
                csP.whiteSpace, csP.textOverflow,
                clip.scrollWidth, clip.clientWidth,
                nw.getBoundingClientRect().height,
            ];
        "#,
        )
        .unwrap();
    let parts = parts.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!("normal"),
        "white-space unset on the outer box"
    );
    assert_eq!(parts[1], serde_json::json!("ellipsis"));
    assert_eq!(
        parts[2],
        serde_json::json!("nowrap"),
        "white-space comes from the declaration"
    );
    assert_eq!(
        parts[3],
        serde_json::json!("clip"),
        "text-overflow does not inherit (CSS UI §5.2)"
    );
    assert_eq!(parts[4], serde_json::json!("normal"));
    assert_eq!(
        parts[5],
        serde_json::json!("clip"),
        "initial text-overflow is clip"
    );
    let (sw, cw) = (parts[6].as_f64().unwrap(), parts[7].as_f64().unwrap());
    assert!(
        sw > cw + 20.0,
        "scrollWidth keeps the full nowrap text ({sw} > {cw})"
    );
    let h = parts[8].as_f64().unwrap();
    assert!(
        h > 10.0 && h < 30.0,
        "nowrap keeps the run on one line ({h})"
    );
}

/// getComputedStyle face for `content` (batch 99): the default reads
/// "normal" (Chrome's initial), declared values re-serialize quoted,
/// attr() keeps its functional shape, and pseudo-only declarations never
/// leak onto the host element.
#[cfg(feature = "screenshot")]
#[test]
fn computed_style_content_face() {
    let mut rt = setup_runtime(
        r#"<style>
          #q::before{content:"P"}
          #a{content:attr(k)}
          #s{content:"S"}
        </style>
        <span id="q"></span><span id="a" k="v1"></span><span id="s"></span><span id="d"></span>"#,
    );
    let cs = |rt: &mut JsRuntime, sel: &str| {
        rt.evaluate(&format!(
            "getComputedStyle(document.querySelector('{}')).content",
            sel
        ))
        .unwrap()
    };
    assert_eq!(
        cs(&mut rt, "#q"),
        serde_json::json!("normal"),
        "pseudo-only rule never lands on the host"
    );
    assert_eq!(cs(&mut rt, "#a"), serde_json::json!("attr(k)"));
    assert_eq!(cs(&mut rt, "#s"), serde_json::json!("\"S\""));
    assert_eq!(
        cs(&mut rt, "#d"),
        serde_json::json!("normal"),
        "default computed content is Chrome's 'normal'"
    );
}

#[cfg(feature = "screenshot")]
#[test]
fn computed_style_pseudo_element_face() {
    let mut rt = setup_runtime(
        r#"<style>
          #q::before{content:"P";color:rgb(255, 0, 0)}
          #q{color:rgb(0, 0, 255)}
          #a::after{content:attr(k)}
        </style>
        <span id="q">host</span><span id="a" k="v1"></span><span id="n"></span>"#,
    );
    let read = |rt: &mut JsRuntime, expr: &str| -> String {
        match rt.evaluate(expr) {
            Ok(serde_json::Value::String(s)) => s,
            other => panic!("{expr} -> {other:?}"),
        }
    };
    // Pseudo face reads the pseudo cascade, not the host's.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q'),'::before').getPropertyValue('content')"
        ),
        r#""P""#
    );
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q'),'::before').color"
        ),
        "rgb(255, 0, 0)"
    );
    // Host face untouched by the pseudo declarations.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q')).color"
        ),
        "rgb(0, 0, 255)"
    );
    // Legacy single-colon spelling, ASCII case-insensitive.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q'),':BEFORE').getPropertyValue('content')"
        ),
        r#""P""#
    );
    // attr() keeps its functional shape on the computed face (#19, Chrome
    // parity) — the host attribute substitutes only into the rendered box.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('a'),'::after').getPropertyValue('content')"
        ),
        "attr(k)"
    );
    // Pseudo with no matching rule (and unmodeled `::name` forms) answers
    // the initial-value table.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q'),'::after').getPropertyValue('content')"
        ),
        "normal"
    );
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('n'),'::before').getPropertyValue('content')"
        ),
        "normal"
    );
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q'),'::fancy').getPropertyValue('content')"
        ),
        "normal"
    );
    // The pseudo generates no box: geometry reads 'auto' instead of the
    // host's bounding rect.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('q'),'::before').width"
        ),
        "auto"
    );
    // Junk arguments throw TypeError (CSSOM argument grammar: only the four
    // legacy single-colon names and `::name` forms are valid). The eval
    // harness swallows throws to null, so the probe catches in-script.
    for junk in ["fancy", ":selection"] {
        let expr = format!(
            "(function(){{try{{getComputedStyle(document.getElementById('q'),'{}');return 'no-throw';}}catch(e){{return e instanceof TypeError?'TypeError':'other:'+e;}}}})()",
            junk
        );
        assert_eq!(read(&mut rt, &expr), "TypeError", "junk arg {junk}");
    }
}
