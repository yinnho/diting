//! WebIDL 真值电池 knife88-120
use super::common::setup_runtime;

/// 刀88 — typed-om unit family (Chrome 145 truth: k88-typedom / k88-mini).
/// The CSS namespace carries the full 69-key surface: 64 unit factories whose
/// canonical unit is the LOWERCASED factory name (Hz→hz, kHz→khz, Q→q),
/// each a plain double coercion ("The provided double value is non-finite."
/// for NaN/'x'/{} — CSS.px(CSS.px(3)) too, 145 has no numeric bypass on the
/// factories); CSSUnitValue is a real constructible value object (2-arg
/// ladder, "Invalid unit: X", value rw / unit ro as prototype accessors,
/// zero own keys, instanceof chain through CSSNumericValue/CSSStyleValue);
/// serialization rides CSSStyleValue.prototype.toString (number bare,
/// percent %, else value+unit); registerProperty reads `inherits` before
/// `name` with the property-read message form; CSS.highlights is an empty
/// HighlightRegistry (size 0), CSS.paintWorklet a bare Worklet.
#[tokio::test(flavor = "current_thread")]
async fn knife88_css_unit_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : (v && typeof v === 'object') ? (v.constructor && v.constructor.name) : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        T('keys', () => Object.getOwnPropertyNames(CSS).length);
        T('px', () => { const v = CSS.px(1); return v.value + '/' + v.unit; });
        T('px:nan', () => CSS.px(NaN));
        T('px:str', () => CSS.px('x'));
        T('px:0', () => CSS.px());
        T('px:5', () => CSS.px('5').value);
        T('px:self', () => CSS.px(CSS.px(3)));
        T('u:Hz', () => CSS.Hz(1).unit);
        T('u:kHz', () => CSS.kHz(1).unit);
        T('u:Q', () => CSS.Q(1).unit);
        T('u:cqw', () => CSS.cqw(1).unit);
        T('u:number', () => CSS.number(2).unit);
        T('uv:0', () => new CSSUnitValue());
        T('uv:1', () => new CSSUnitValue(1));
        T('uv:bogus', () => new CSSUnitValue(1, 'bogus'));
        T('uv:nan', () => new CSSUnitValue(NaN, 'px'));
        T('uv:ok', () => new CSSUnitValue(1, 'px').unit);
        T('uv:rw', () => { const v = new CSSUnitValue(1, 'px'); v.value = 5; v.unit = 'em'; return v.value + '/' + v.unit; });
        T('uv:own', () => Object.getOwnPropertyNames(new CSSUnitValue(1, 'px')).length);
        T('uv:i1', () => new CSSUnitValue(1, 'px') instanceof CSSUnitValue);
        T('uv:i2', () => new CSSUnitValue(1, 'px') instanceof CSSNumericValue);
        T('uv:i3', () => new CSSUnitValue(1, 'px') instanceof CSSStyleValue);
        T('str:px', () => String(CSS.px(1)));
        T('str:number', () => String(CSS.number(2)));
        T('str:percent', () => String(CSS.percent(50)));
        T('str:neg', () => String(CSS.px(-1.5)));
        T('str:deg', () => String(CSS.deg(90)));
        T('hl', () => CSS.highlights instanceof HighlightRegistry);
        T('hl:size', () => CSS.highlights.size);
        T('pw', () => Object.getOwnPropertyNames(CSS.paintWorklet).length);
        T('rp:0', () => CSS.registerProperty());
        T('rp:empty', () => CSS.registerProperty({}));
        T('rp:ok', () => CSS.registerProperty({ name: '--x88', syntax: '*', inherits: false }));
        T('rp:dup', () => CSS.registerProperty({ name: '--x88', syntax: '*', inherits: false }));
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(50).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            ["keys", "ok", "69"],
            ["px", "ok", "1/px"],
            [
                "px:nan",
                "TypeError",
                "Failed to execute 'px' on 'CSS': The provided double value is non-finite."
            ],
            [
                "px:str",
                "TypeError",
                "Failed to execute 'px' on 'CSS': The provided double value is non-finite."
            ],
            [
                "px:0",
                "TypeError",
                "Failed to execute 'px' on 'CSS': 1 argument required, but only 0 present."
            ],
            ["px:5", "ok", "5"],
            [
                "px:self",
                "TypeError",
                "Failed to execute 'px' on 'CSS': The provided double value is non-finite."
            ],
            ["u:Hz", "ok", "hz"],
            ["u:kHz", "ok", "khz"],
            ["u:Q", "ok", "q"],
            ["u:cqw", "ok", "cqw"],
            ["u:number", "ok", "number"],
            [
                "uv:0",
                "TypeError",
                "Failed to construct 'CSSUnitValue': 2 arguments required, but only 0 present."
            ],
            [
                "uv:1",
                "TypeError",
                "Failed to construct 'CSSUnitValue': 2 arguments required, but only 1 present."
            ],
            [
                "uv:bogus",
                "TypeError",
                "Failed to construct 'CSSUnitValue': Invalid unit: bogus"
            ],
            [
                "uv:nan",
                "TypeError",
                "Failed to construct 'CSSUnitValue': The provided double value is non-finite."
            ],
            ["uv:ok", "ok", "px"],
            ["uv:rw", "ok", "5/px"],
            ["uv:own", "ok", "0"],
            ["uv:i1", "ok", "true"],
            ["uv:i2", "ok", "true"],
            ["uv:i3", "ok", "true"],
            ["str:px", "ok", "1px"],
            ["str:number", "ok", "2"],
            ["str:percent", "ok", "50%"],
            ["str:neg", "ok", "-1.5px"],
            ["str:deg", "ok", "90deg"],
            ["hl", "ok", "true"],
            ["hl:size", "ok", "0"],
            ["pw", "ok", "0"],
            [
                "rp:0",
                "TypeError",
                "Failed to execute 'registerProperty' on 'CSS': 1 argument required, but only 0 present."
            ],
            [
                "rp:empty",
                "TypeError",
                "Failed to execute 'registerProperty' on 'CSS': Failed to read the 'inherits' property from 'PropertyDefinition': Required member is undefined."
            ],
            ["rp:ok", "ok", "undef"],
            [
                "rp:dup",
                "InvalidModificationError",
                "Failed to execute 'registerProperty' on 'CSS': The name provided has already been registered."
            ]
        ]),
        "unit factories lowercase the name, CSSUnitValue rw value / ro unit, \
         serialization via CSSStyleValue.toString, registerProperty inherits-first"
    );
}

/// 刀89 — typed-om arithmetic family (Chrome 145 truth: k89-math, 81 cases).
/// Same-unit add/sub/mul/div fold to CSSUnitValue; mixed units mint CSSMathSum
/// with FLATTENING chained adds (Sum.add(px3) appends a third term) and the
/// structural minus-plus form "calc(1px + -2em)"; mul by a unit value never
/// folds, div by one serializes in div mode "calc(4px / 2px)"; min/max fold
/// only on uniform units; equals is strict unit+value with no-args true;
/// to() carries Chrome's pinned float artifacts (96px→cm
/// 2.5400000000000005, 1in→mm 25.400000000000006, 1cm→Q 40.00000000000001)
/// and rejects relative units / cross-families with distinct messages;
/// CSSMath value/values/lower/upper/operator read the ctor-minted kids;
/// CSSNumericArray owns indexed keys with a prototype length getter and zero
/// other own properties; parse() evaluates single-unit calc() to Sum[Unit]
/// while mixed units keep the SOURCE string; CSSStyleValue.parse splits
/// keywords (CSSKeywordValue) / numerics (CSSUnitValue) / plain values.
#[tokio::test(flavor = "current_thread")]
async fn knife89_css_numeric_arithmetic() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : (v && typeof v === 'object') ? (v.constructor && v.constructor.name) : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        const N = (v) => v && v.constructor && v.constructor.name === 'CSSUnitValue' ? (v.value + '/' + v.unit) : String(v);
        const NS = (f) => { const v = f(); return v.constructor.name + '/' + String(v); };
        T('add:fold', () => N(CSS.px(1).add(CSS.px(2))));
        T('add:0', () => CSS.px(1).add().constructor.name);
        T('add:num', () => CSS.px(1).add(2));
        T('add:str', () => CSS.px(1).add('x'));
        T('add:mix', () => NS(() => CSS.px(1).add(CSS.em(2))));
        T('add:node', () => { const v = CSS.px(1).add(CSS.em(2)).add(CSS.px(3)); return v.constructor.name + '/' + String(v) + '/' + v.values.length; });
        T('sub:fold', () => N(CSS.px(5).sub(CSS.px(2))));
        T('sub:mix', () => NS(() => CSS.px(1).sub(CSS.em(2))));
        T('mul:num', () => N(CSS.px(2).mul(CSS.number(3))));
        T('mul:unit', () => NS(() => CSS.px(2).mul(CSS.px(3))));
        T('div:num', () => N(CSS.px(4).div(CSS.number(2))));
        T('div:unit', () => String(CSS.px(4).div(CSS.px(2))));
        T('min:same', () => N(CSS.px(1).min(CSS.px(2))));
        T('max:same', () => N(CSS.px(1).max(CSS.px(2))));
        T('min:mix', () => NS(() => CSS.px(1).min(CSS.em(2))));
        T('eq:same', () => CSS.px(1).equals(CSS.px(1)));
        T('eq:diff', () => CSS.px(1).equals(CSS.em(1)));
        T('eq:num', () => CSS.px(1).equals(1));
        T('eq:0', () => CSS.px(1).equals());
        T('to:cm', () => N(CSS.px(96).to('cm')));
        T('to:mm', () => N(CSS.in(1).to('mm')));
        T('to:Q', () => N(CSS.cm(1).to('Q')));
        T('to:pt', () => N(CSS.px(96).to('pt')));
        T('to:rad', () => N(CSS.deg(180).to('rad')));
        T('to:grad', () => N(CSS.deg(90).to('grad')));
        T('to:s', () => N(CSS.ms(1000).to('s')));
        T('to:dpi', () => N(CSS.dppx(1).to('dpi')));
        T('to:0', () => CSS.px(1).to());
        T('to:bogus', () => CSS.px(1).to('bogus'));
        T('to:incompat', () => CSS.px(1).to('deg'));
        T('to:em', () => CSS.px(1).to('em'));
        T('toSum:1', () => NS(() => CSS.px(1).toSum('px')));
        T('toSum:2', () => { const v = CSS.in(1).toSum('px', 'in'); return v.constructor.name + '/' + String(v) + '/' + v.values.length; });
        T('inv:val', () => N(new CSSMathInvert(CSS.px(1)).value));
        T('inv:str', () => String(new CSSMathInvert(CSS.px(1))));
        T('neg:str', () => String(new CSSMathNegate(CSS.px(1))));
        T('sum:vals', () => { const a = new CSSMathSum(CSS.px(1), CSS.px(2)).values; return a.constructor.name + '/' + a.length + '/' + N(a[0]) + '/' + N(a[1]); });
        T('sum:str', () => String(new CSSMathSum(CSS.px(1), CSS.px(2))));
        T('prod:str', () => String(new CSSMathProduct(CSS.px(1), CSS.em(2))));
        T('clamp:m', () => { const c = new CSSMathClamp(CSS.px(1), CSS.px(2), CSS.px(3)); return N(c.lower) + '/' + N(c.value) + '/' + N(c.upper); });
        T('clamp:op', () => new CSSMathClamp(CSS.px(1), CSS.px(2), CSS.px(3)).operator);
        T('sum:op', () => new CSSMathSum(CSS.px(1)).operator);
        T('na:own', () => Object.getOwnPropertyNames(new CSSMathSum(CSS.px(1), CSS.px(2)).values).join(','));
        T('na:for', () => { let s = ''; for (const v of new CSSMathSum(CSS.px(1), CSS.px(2)).values) s += v.unit; return s; });
        T('parse:1px', () => N(CSSNumericValue.parse('1px')));
        T('parse:pct', () => N(CSSNumericValue.parse('50%')));
        T('parse:calc', () => NS(() => CSSNumericValue.parse('calc(1px + 2px)')));
        T('parse:calc2', () => NS(() => CSSNumericValue.parse('calc(100% - 10px)')));
        T('parse:calc3', () => NS(() => CSSNumericValue.parse('calc(2 * 3px)')));
        T('parse:max', () => NS(() => CSSNumericValue.parse('max(1px, 2em)')));
        T('parse:clamp', () => NS(() => CSSNumericValue.parse('clamp(1px, 2vw, 3px)')));
        T('parse:empty', () => CSSNumericValue.parse(''));
        T('parse:0', () => CSSNumericValue.parse());
        T('sparse', () => NS(() => CSSStyleValue.parse('width', '1px')));
        T('sparse:color', () => NS(() => CSSStyleValue.parse('color', 'red')));
        T('sparse:unset', () => NS(() => CSSStyleValue.parse('width', 'inherit')));
        T('sparse:0', () => CSSStyleValue.parse());
        T('type:deg', () => JSON.stringify(CSS.deg(1).type()));
        T('type:px', () => CSS.px(1).type().constructor.name);
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(50).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    // (split assertion — a single json! over 59 nested rows blows the macro
    // recursion limit; 刀88's 35 was the working ceiling)
    assert_eq!(
        log.as_array().unwrap()[..31],
        serde_json::json!([
            ["add:fold", "ok", "3/px"],
            ["add:0", "ok", "CSSUnitValue"],
            ["add:num", "TypeError", "Failed to execute 'add' on 'CSSNumericValue': Incompatible types"],
            ["add:str", "TypeError", "Failed to execute 'add' on 'CSSNumericValue': The provided double value is non-finite."],
            ["add:mix", "ok", "CSSMathSum/calc(1px + 2em)"],
            ["add:node", "ok", "CSSMathSum/calc(1px + 2em + 3px)/3"],
            ["sub:fold", "ok", "3/px"],
            ["sub:mix", "ok", "CSSMathSum/calc(1px + -2em)"],
            ["mul:num", "ok", "6/px"],
            ["mul:unit", "ok", "CSSMathProduct/calc(2px * 3px)"],
            ["div:num", "ok", "2/px"],
            ["div:unit", "ok", "calc(4px / 2px)"],
            ["min:same", "ok", "1/px"],
            ["max:same", "ok", "2/px"],
            ["min:mix", "ok", "CSSMathMin/min(1px, 2em)"],
            ["eq:same", "ok", "true"],
            ["eq:diff", "ok", "false"],
            ["eq:num", "ok", "false"],
            ["eq:0", "ok", "true"],
            ["to:cm", "ok", "2.5400000000000005/cm"],
            ["to:mm", "ok", "25.400000000000006/mm"],
            ["to:Q", "ok", "40.00000000000001/q"],
            ["to:pt", "ok", "72/pt"],
            ["to:rad", "ok", "3.141592653589793/rad"],
            ["to:grad", "ok", "100/grad"],
            ["to:s", "ok", "1/s"],
            ["to:dpi", "ok", "96/dpi"],
            ["to:0", "TypeError", "Failed to execute 'to' on 'CSSNumericValue': 1 argument required, but only 0 present."],
            ["to:bogus", "SyntaxError", "Failed to execute 'to' on 'CSSNumericValue': Invalid unit for conversion"],
            ["to:incompat", "TypeError", "Failed to execute 'to' on 'CSSNumericValue': Cannot convert to deg"],
            ["to:em", "TypeError", "Failed to execute 'to' on 'CSSNumericValue': Cannot convert to em"]
        ])
        .as_array()
        .unwrap()[..],
        "same-unit folds, Sum flatten/minus-plus, div-mode Product, pinned to() artifacts"
    );
    assert_eq!(
        log.as_array().unwrap()[31..],
        serde_json::json!([
            ["toSum:1", "ok", "CSSMathSum/calc(1px)"],
            ["toSum:2", "ok", "CSSMathSum/calc(96px + 0in)/2"],
            ["inv:val", "ok", "1/px"],
            ["inv:str", "ok", "calc(1 / 1px)"],
            ["neg:str", "ok", "calc(-1px)"],
            ["sum:vals", "ok", "CSSNumericArray/2/1/px/2/px"],
            ["sum:str", "ok", "calc(1px + 2px)"],
            ["prod:str", "ok", "calc(1px * 2em)"],
            ["clamp:m", "ok", "1/px/2/px/3/px"],
            ["clamp:op", "ok", "clamp"],
            ["sum:op", "ok", "sum"],
            ["na:own", "ok", "0,1"],
            ["na:for", "ok", "pxpx"],
            ["parse:1px", "ok", "1/px"],
            ["parse:pct", "ok", "50/percent"],
            ["parse:calc", "ok", "CSSMathSum/calc(3px)"],
            ["parse:calc2", "ok", "CSSMathSum/calc(100% - 10px)"],
            ["parse:calc3", "ok", "CSSMathSum/calc(6px)"],
            ["parse:max", "ok", "CSSMathMax/max(1px, 2em)"],
            ["parse:clamp", "ok", "CSSMathClamp/clamp(1px, 2vw, 3px)"],
            ["parse:empty", "SyntaxError", "Failed to execute 'parse' on 'CSSNumericValue': Invalid math expression"],
            ["parse:0", "TypeError", "Failed to execute 'parse' on 'CSSNumericValue': 1 argument required, but only 0 present."],
            ["sparse", "ok", "CSSUnitValue/1px"],
            ["sparse:color", "ok", "CSSStyleValue/red"],
            ["sparse:unset", "ok", "CSSKeywordValue/inherit"],
            ["sparse:0", "TypeError", "Failed to execute 'parse' on 'CSSStyleValue': 2 arguments required, but only 0 present."],
            ["type:deg", "ok", "{\"angle\":1}"],
            ["type:px", "ok", "Object"]
        ])
        .as_array()
        .unwrap()[..],
        "ctor-kid members, zero-own CSSNumericArray, parse evaluation vs \
         source-string retention"
    );
}

/// M4 刀90: Selection/Range across shadow boundaries + Selection.toString
/// block newlines (k90-xshadow/str4/text7/8/9 vs Chrome 145). Range keeps
/// later-set-endpoint-wins on cross-root setStart/setEnd (already in since
/// 刀73); this knife pins the String() extraction: Selection.toString is
/// layout-flavored (margin blocks "\n\n", plain blocks "\n", BR "\n", tail
/// emits only when the end container still has later children), while
/// Range.toString stays pure concatenation. Cross-tree setBaseAndExtent
/// keeps the raw base/extent in a module WeakMap — displayed collapsed,
/// String() walks the BASE's tree toward the boundary on the extent's side;
/// extend() and addRange collapse for real (empty String).
#[tokio::test]
async fn knife90_cross_shadow_selection_and_plain_text() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        const J = (v) => JSON.stringify(String(v));
        const w = document.createElement('div');
        w.innerHTML = '<div id=da>aaa</div><div id=db>bbb</div><p id=pa>ccc</p><p id=pb>ddd</p><span id=sa>eee</span><span id=sb>fff</span><div id=bc>ab<br>cd</div><div id=nw><p id=np>ab</p></div>';
        document.body.appendChild(w);
        const $ = (i) => w.querySelector('#' + i);
        const sel = getSelection();
        const S = (r) => { sel.removeAllRanges(); sel.addRange(r); return J(sel); };
        const P = (x, y) => { const r = document.createRange(); r.setStart(x.firstChild, 1); r.setEnd(y.firstChild, 2); return S(r); };
        T('blk:div', () => P($('da'), $('db')));
        T('blk:p', () => P($('pa'), $('pb')));
        T('blk:span', () => P($('sa'), $('sb')));
        T('blk:br', () => { const r = document.createRange(); r.selectNodeContents($('bc')); return S(r); });
        T('blk:n1', () => { const r = document.createRange(); r.selectNodeContents($('nw')); return S(r); });
        T('blk:n6', () => { const r = document.createRange(); r.setStart($('pa').firstChild, 0); r.setEndBefore($('pb')); return S(r); });
        T('blk:full2', () => { const r = document.createRange(); r.setStartBefore($('pa')); r.setEndAfter($('pa')); return S(r); });
        T('blk:tailin', () => { const r = document.createRange(); r.setStart($('pa').firstChild, 0); r.setEnd($('pa').firstChild, 3); return S(r); });
        T('range:plain', () => { const r = document.createRange(); r.setStart($('da').firstChild, 1); r.setEnd($('db').firstChild, 2); return J(r); });
        const lh = document.createElement('div'); lh.textContent = 'light text';
        const host = document.createElement('div');
        document.body.appendChild(lh); document.body.appendChild(host);
        const sr = host.attachShadow({ mode: 'open' });
        sr.innerHTML = '<p id=sp>shadow text</p>';
        const lt = lh.firstChild, st = sr.getElementById('sp').firstChild;
        T('x:anchor', () => { sel.setBaseAndExtent(lt, 1, st, 3); return (sel.anchorNode === st) + '/' + sel.anchorOffset + '/' + sel.direction; });
        T('x:fwd', () => { sel.setBaseAndExtent(lt, 1, st, 3); return J(sel); });
        T('x:rev', () => { sel.setBaseAndExtent(st, 3, lt, 1); return (sel.anchorNode === st) + '/' + J(sel); });
        T('x:add', () => { const r = document.createRange(); r.setStart(lt, 1); r.setEnd(st, 3); sel.removeAllRanges(); sel.addRange(r); return (sel.anchorNode === st) + '/' + J(sel); });
        T('x:ext', () => { sel.collapse(lt, 1); sel.extend(st, 3); return J(sel); });
        T('rg:set', () => { const r = document.createRange(); r.setStart(lt, 1); r.setEnd(st, 3); return (r.startContainer === st) + '/' + r.collapsed + '/' + r.cloneContents().childNodes.length; });
        T('rg:rev', () => { const r = document.createRange(); r.setStart(st, 3); r.setEnd(lt, 1); return r.startContainer === lt; });
        T('rg:cmp', () => { const r = document.createRange(); r.setStart(lt, 1); r.setEnd(st, 3); return r.compareBoundaryPoints(Range.START_TO_START, r); });
        T('gcr:delta', () => { sel.collapse(st, 1); const a = sel.getComposedRanges(); const g = a[0]; const hi = Array.from(document.body.childNodes).indexOf(host); return a.length + '/body@+' + (g.startOffset - hi) + '>body@+' + (g.endOffset - hi); });
        const h2 = document.createElement('div'); document.body.appendChild(h2);
        const sr2 = h2.attachShadow({ mode: 'closed' });
        sr2.innerHTML = '<p>closed text</p>';
        const st2 = sr2.querySelector('p').firstChild;
        T('cs:anchor', () => { sel.setBaseAndExtent(st2, 0, st2, 3); return (sel.anchorNode === st2) + '/' + sel.anchorOffset; });
        T('cs:dict', () => { const a = sel.getComposedRanges({ shadowRoots: [sr2] }); const g = a[0]; return (g.startContainer === st2) + '/' + g.startOffset + '>' + g.endOffset; });
        const h3 = document.createElement('div'); document.body.appendChild(h3);
        const s3 = h3.attachShadow({ mode: 'open' }); s3.innerHTML = '<p>one</p>';
        const n1 = s3.querySelector('p').firstChild;
        T('two:shadow', () => { sel.setBaseAndExtent(n1, 0, st2, 1); return (sel.anchorNode === n1) + '/' + sel.direction; });
        T('two:rev', () => { sel.setBaseAndExtent(st2, 1, n1, 0); return (sel.anchorNode === n1) + '/' + sel.direction; });
        T('own', () => { sel.setBaseAndExtent(lt, 1, st, 3); sel.extend(st, 3); return Reflect.ownKeys(sel).length; });
    "#,
    )
    .unwrap();
    let v = rt.evaluate("JSON.stringify(globalThis.__log)").unwrap();
    let log: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            ["blk:div", "ok", "\"aa\\nbb\""],
            ["blk:p", "ok", "\"cc\\n\\ndd\""],
            ["blk:span", "ok", "\"eeff\""],
            ["blk:br", "ok", "\"ab\\ncd\""],
            ["blk:n1", "ok", "\"ab\""],
            ["blk:n6", "ok", "\"ccc\\n\\n\""],
            ["blk:full2", "ok", "\"ccc\\n\\n\""],
            ["blk:tailin", "ok", "\"ccc\""],
            ["range:plain", "ok", "\"aabb\""],
            ["x:anchor", "ok", "true/3/forward"],
            ["x:fwd", "ok", "\"ight text\\n\""],
            ["x:rev", "ok", "true/\"sha\""],
            ["x:add", "ok", "true/\"\""],
            ["x:ext", "ok", "\"\""],
            ["rg:set", "ok", "true/true/0"],
            ["rg:rev", "ok", "true"],
            ["rg:cmp", "ok", "0"],
            ["gcr:delta", "ok", "1/body@+0>body@+1"],
            ["cs:anchor", "ok", "true/0"],
            ["cs:dict", "ok", "true/0>3"],
            ["two:shadow", "ok", "true/backward"],
            ["two:rev", "ok", "true/forward"],
            ["own", "ok", "0"]
        ]),
        "block newlines (margin=p double, plain div single, span none, br one, tail by end-container children), cross-tree raw String in the base tree, Range later-endpoint collapse, normalized composed lift"
    );
}

#[tokio::test]
async fn knife91_svg_point_lists() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        const poly = document.createElementNS('http://www.w3.org/2000/svg', 'polygon');
        poly.setAttribute('points', '10,20 30,40 50,60');
        svg.appendChild(poly); document.body.appendChild(svg);
        const P = poly.points, AP = poly.animatedPoints;
        const pt = (x, y) => { const s = svg.createSVGPoint(); s.x = x; s.y = y; return s; };
        T('shape', () => [P.constructor.name, P.length, P.numberOfItems, P.getItem(2).x + ',' + P.getItem(2).y].join('/'));
        T('cache', () => [poly.points === P, P === AP, poly.animatedPoints === AP].join('/'));
        T('fresh', () => [P.getItem(0) === P.getItem(0), P[0] === P.getItem(0), P[0].x].join('/'));
        T('ownkeys', () => Object.getOwnPropertyNames(P).join(','));
        T('writethru', () => { const p = P.getItem(0); p.x = 99; return JSON.stringify(poly.getAttribute('points')); });
        T('live', () => { poly.setAttribute('points', '5,6 7,8'); return P.getItem(1).x + '/' + Object.getOwnPropertyNames(P).join(','); });
        T('reattr', () => { const p = P.getItem(0); poly.setAttribute('points', '9,9 8,8'); p.x = 77; return p.x + '/' + P.getItem(0).x + '/' + JSON.stringify(poly.getAttribute('points')); });
        T('oor-gt', () => P.getItem(5));
        T('oor-neg', () => P.getItem(-1));
        T('oor-eq', () => P.getItem(2));
        T('argc', () => P.getItem());
        T('type', () => P.appendItem({ x: 1, y: 2 }));
        T('clamp', () => { const r = P.insertItemBefore(pt(1, 1), -1); return r.x + '/' + P.numberOfItems + '/' + P.getItem(P.numberOfItems - 1).x; });
        T('alias', () => { const s = pt(42, 43); P.insertItemBefore(s, 0); s.x = 55; return P.getItem(0).x; });
        T('mut', () => { const r = P.replaceItem(pt(7, 7), 0); return [r.x, P.removeItem(0).x, P.initialize(pt(5, 5)).x, P.numberOfItems].join('/'); });
        T('clear', () => { P.clear(); return P.numberOfItems + '/' + JSON.stringify(poly.getAttribute('points')) + '/' + Object.getOwnPropertyNames(P).join(','); });
        T('parse', () => { poly.setAttribute('points', '1,x 3 4'); const a = P.numberOfItems; poly.setAttribute('points', '1,2 3'); const b = P.numberOfItems;
                           poly.setAttribute('points', ' 1e2,3 +4,5'); return a + '/' + b + '/' + P.numberOfItems + '/' + P.getItem(1).x; });
        T('ap-ro', () => AP.appendItem(pt(1, 1)));
        T('ap-item', () => { const p = AP.getItem(0); try { p.x = 9; return 'no-throw'; } catch (e) { return e.name; } });
        T('ap-read', () => AP.numberOfItems + '/' + AP.getItem(0).x);
        T('serialize', () => { poly.setAttribute('points', '1,2'); P.appendItem(pt(1.5, 2.5)); return JSON.stringify(poly.getAttribute('points')); });
        T('iter', () => [...P].length + '/' + [...P][0].x);
        T('polyline', () => { const pl = document.createElementNS('http://www.w3.org/2000/svg', 'polyline'); pl.setAttribute('points', '1,2 3,4');
                              return [pl.points.numberOfItems, pl.animatedPoints.numberOfItems, pl.points === pl.animatedPoints].join('/'); });
        T('noattr', () => document.createElementNS('http://www.w3.org/2000/svg', 'polygon').points.numberOfItems);
        T('desc', () => { const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(poly), 'points');
                          const e = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(P), 'numberOfItems');
                          return [d.get.name, !!d.set, d.enumerable, d.configurable, e.get.name].join('/'); });
        T('ctorface', () => [SVGPoint.length, SVGPoint.name, (() => { try { new SVGPoint(1, 2); return 'no-throw'; } catch (e2) { return e2.name; } })(),
                             pt(1, 1) instanceof SVGPoint, svg.createSVGPoint().constructor.name].join('/'));
        T('mt', () => { const s = pt(3, 4); const m = svg.createSVGMatrix(); return s.matrixTransform(m).x; });
        T('brand', () => { const g = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(P), 'getItem').get; try { g.call({}); return 'no-throw'; } catch (e) { return e.name; } });
        return JSON.stringify(__log);
    "#).unwrap();
    let out = rt.evaluate("JSON.stringify(__log)").unwrap();
    let log: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            ["shape", "ok", "SVGPointList/3/3/50,60"],
            ["cache", "ok", "true/false/true"],
            ["fresh", "ok", "false/false/10"],
            ["ownkeys", "ok", "0,1,2"],
            ["writethru", "ok", "\"99 20 30 40 50 60\""],
            ["live", "ok", "7/0,1"],
            ["reattr", "ok", "77/9/\"9 9 8 8\""],
            [
                "oor-gt",
                "IndexSizeError",
                "Failed to execute 'getItem' on 'SVGPointList': The index provided (5) is greater than the maximum bound (2)."
            ],
            [
                "oor-neg",
                "IndexSizeError",
                "Failed to execute 'getItem' on 'SVGPointList': The index provided (4294967295) is greater than the maximum bound (2)."
            ],
            [
                "oor-eq",
                "IndexSizeError",
                "Failed to execute 'getItem' on 'SVGPointList': The index provided (2) is greater than or equal to the maximum bound (2)."
            ],
            [
                "argc",
                "TypeError",
                "Failed to execute 'getItem' on 'SVGPointList': 1 argument required, but only 0 present."
            ],
            [
                "type",
                "TypeError",
                "Failed to execute 'appendItem' on 'SVGPointList': parameter 1 is not of type 'SVGPoint'."
            ],
            ["clamp", "ok", "1/3/1"],
            ["alias", "ok", "55"],
            ["mut", "ok", "7/7/5/1"],
            ["clear", "ok", "0/null/"],
            ["parse", "ok", "0/1/2/4"],
            [
                "ap-ro",
                "NoModificationAllowedError",
                "Failed to execute 'appendItem' on 'SVGPointList': This object is read-only."
            ],
            ["ap-item", "ok", "NoModificationAllowedError"],
            ["ap-read", "ok", "2/100"],
            ["serialize", "ok", "\"1 2 1.5 2.5\""],
            ["iter", "ok", "2/1"],
            ["polyline", "ok", "2/2/false"],
            ["noattr", "ok", "0"],
            ["desc", "ok", "get points/false/true/true/get numberOfItems"],
            ["ctorface", "ok", "0/SVGPoint/TypeError/true/SVGPoint"],
            ["mt", "ok", "3"],
            ["brand", "ok", "TypeError"]
        ]),
        "SVGPointList live attribute view: fresh wrappers over shared stores, two OOB message variants, clamp inserts, store aliasing, clear removes the attribute, all-or-nothing parse, animatedPoints read-only + ro item setters, ctor always Illegal constructor"
    );
}

#[tokio::test]
async fn knife92_webgl_ext_behavior() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        const gl = document.createElement('canvas').getContext('webgl');
        const gl2 = document.createElement('canvas').getContext('webgl2');
        const vao = gl.getExtension('OES_vertex_array_object');
        const v1 = vao.createVertexArrayOES();
        T('vao.mint', () => [Object.prototype.toString.call(v1), Reflect.ownKeys(v1).length,
                              Object.prototype.toString.call(Object.getPrototypeOf(v1)),
                              Object.prototype.toString.call(Object.getPrototypeOf(Object.getPrototypeOf(v1))),
                              Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(v1))) === Object.prototype].join('/'));
        T('vao.is0', () => vao.isVertexArrayOES(v1));
        T('vao.is0args', () => vao.isVertexArrayOES());
        T('vao.isnull', () => vao.isVertexArrayOES(null));
        T('vao.isbad', () => vao.isVertexArrayOES({}));
        T('vao.bind', () => { vao.bindVertexArrayOES(v1); return vao.isVertexArrayOES(v1); });
        T('vao.bindnull', () => { vao.bindVertexArrayOES(null); return gl.getError(); });
        T('vao.bindbad', () => vao.bindVertexArrayOES({}));
        T('vao.del', () => { vao.deleteVertexArrayOES(v1); return [vao.isVertexArrayOES(v1), vao.isVertexArrayOES(null)].join('/'); });
        T('vao.argc', () => vao.createVertexArrayOES());
        const tq = gl.getExtension('EXT_disjoint_timer_query');
        const q = tq.createQueryEXT();
        T('tq.is0', () => tq.isQueryEXT(q));
        T('tq.beginBad', () => { tq.beginQueryEXT(9999, q); return gl.getError(); });
        T('tq.beginNull', () => tq.beginQueryEXT(tq.TIME_ELAPSED_EXT, null));
        T('tq.beginNotQ', () => tq.beginQueryEXT(tq.TIME_ELAPSED_EXT, {}));
        T('tq.flow', () => { tq.beginQueryEXT(tq.TIME_ELAPSED_EXT, q);
                             const cur = tq.getQueryEXT(tq.TIME_ELAPSED_EXT, tq.CURRENT_QUERY_EXT) === q;
                             tq.endQueryEXT(tq.TIME_ELAPSED_EXT);
                             return [cur, tq.isQueryEXT(q), tq.getQueryEXT(tq.TIME_ELAPSED_EXT, tq.CURRENT_QUERY_EXT),
                                     tq.getQueryEXT(tq.TIMESTAMP_EXT, tq.QUERY_COUNTER_BITS_EXT)].join('/'); });
        T('tq.bits', () => tq.getQueryEXT(tq.TIME_ELAPSED_EXT, tq.QUERY_COUNTER_BITS_EXT));
        T('tq.obj', () => { gl.getError(); const av = tq.getQueryObjectEXT(q, tq.QUERY_RESULT_AVAILABLE_EXT);
                            const rs = tq.getQueryObjectEXT(q, tq.QUERY_RESULT_EXT); return [av, rs, gl.getError()].join('/'); });
        T('tq.objBadPname', () => { gl.getError(); const v = tq.getQueryObjectEXT(q, 9999); return [v, gl.getError()].join('/'); });
        T('tq.objBadType', () => tq.getQueryObjectEXT({}, 0));
        T('tq.counter', () => { const q2 = tq.createQueryEXT(); gl.getError(); tq.queryCounterEXT(q2, tq.TIMESTAMP_EXT);
                                const e1 = gl.getError(); tq.queryCounterEXT(q2, 9999);
                                return [e1, gl.getError(), tq.isQueryEXT(q2)].join('/'); });
        T('tq.beginTwice', () => { tq.beginQueryEXT(tq.TIME_ELAPSED_EXT, q); gl.getError(); const q3 = tq.createQueryEXT();
                                   tq.beginQueryEXT(tq.TIME_ELAPSED_EXT, q3); const e = gl.getError(); tq.endQueryEXT(tq.TIME_ELAPSED_EXT); return e; });
        T('tq.gpuDisjoint', () => { gl.getError(); return [gl.getParameter(tq.GPU_DISJOINT_EXT), gl.getError()].join('/'); });
        T('tq.delete', () => { tq.deleteQueryEXT(null); tq.deleteQueryEXT(q); return [tq.isQueryEXT(q)].join('/'); });
        const ds = gl.getExtension('WEBGL_debug_shaders');
        T('ds.uncompiled', () => { const s = gl.createShader(gl.VERTEX_SHADER); gl.shaderSource(s, 'void main(){}'); return ds.getTranslatedShaderSource(s); });
        T('ds.compiled', () => { const s = gl.createShader(gl.VERTEX_SHADER); gl.shaderSource(s, 'void main(){}'); gl.compileShader(s);
                                 const t = ds.getTranslatedShaderSource(s); return [typeof t, t.length].join('/'); });
        T('ds.badtype', () => ds.getTranslatedShaderSource({}));
        T('astc', () => gl.getExtension('WEBGL_compressed_texture_astc').getSupportedProfiles().join(','));
        const ia = gl.getExtension('ANGLE_instanced_arrays');
        T('ia.draw', () => { ia.drawArraysInstancedANGLE(gl.TRIANGLES, 0, 3, 1); return gl.getError(); });
        T('ia.divisorOOB', () => { ia.vertexAttribDivisorANGLE(9999, 1); return gl.getError(); });
        T('ia.divisorNeg', () => { ia.vertexAttribDivisorANGLE(-1, 1); return gl.getError(); });
        T('ia.argc', () => ia.drawArraysInstancedANGLE());
        const db = gl.getExtension('WEBGL_draw_buffers');
        T('db.ok', () => { db.drawBuffersWEBGL([gl.BACK]); return gl.getError(); });
        T('db.bad', () => { db.drawBuffersWEBGL([9999]); return gl.getError(); });
        T('db.empty', () => { db.drawBuffersWEBGL([]); return gl.getError(); });
        T('db.double', () => { db.drawBuffersWEBGL([gl.BACK, gl.BACK]); return gl.getError(); });
        T('db.seq', () => db.drawBuffersWEBGL('ab'));
        const oi = gl2.getExtension('OES_draw_buffers_indexed');
        T('oi.ok', () => { oi.blendFunciOES(0, gl2.ONE, gl2.ZERO); return gl2.getError(); });
        T('oi.oob', () => { oi.blendFunciOES(99, gl2.ONE, gl2.ZERO); return gl2.getError(); });
        T('oi.badCap', () => { oi.enableiOES(9999, 0); return gl2.getError(); });
        const cc = gl2.getExtension('EXT_clip_control');
        T('cc.gate', () => gl2.getParameter(37724));
        T('cc.set', () => { cc.clipControlEXT(36002, 37727); return [gl2.getParameter(37724), gl2.getParameter(37725), gl2.getError()].join('/'); });
        T('cc.bad', () => { cc.clipControlEXT(9999, 37726); return gl2.getError(); });
        const pm = gl2.getExtension('WEBGL_polygon_mode');
        T('pm.face', () => { pm.polygonModeWEBGL(gl2.FRONT, pm.FILL_WEBGL); return gl2.getError(); });
        T('pm.ok', () => { pm.polygonModeWEBGL(gl2.FRONT_AND_BACK, pm.LINE_WEBGL); return [gl2.getError(), gl2.getParameter(2880)].join('/'); });
        const pv = gl2.getExtension('WEBGL_provoking_vertex');
        T('pv', () => { pv.provokingVertexWEBGL(36430); return [gl2.getError(), gl2.getParameter(36431)].join('/'); });
        const md = gl2.getExtension('WEBGL_multi_draw');
        T('md', () => { md.multiDrawArraysWEBGL(gl2.TRIANGLES, new Int32Array([0]), 0, new Int32Array([3]), 0, 1); return gl2.getError(); });
        T('md.argc', () => md.multiDrawArraysWEBGL());
        const tq2 = gl2.getExtension('EXT_disjoint_timer_query_webgl2');
        T('tq2.core', () => { const q2 = gl2.createQuery(); gl2.getError(); tq2.queryCounterEXT(q2, tq2.TIMESTAMP_EXT);
                              return [gl2.getError(), Object.prototype.toString.call(q2)].join('/'); });
        T('tq2.bad', () => tq2.queryCounterEXT({}, tq2.TIMESTAMP_EXT));
        T('tq2.argc', () => tq2.queryCounterEXT());
        T('rx.getParam', () => WebGLRenderingContext.prototype.getParameter.call({}, 1));
        T('rx.getExt', () => WebGLRenderingContext.prototype.getExtension.call({}, 'x'));
        T('rx.getErr', () => WebGLRenderingContext.prototype.getError.call({}));
        T('rx.extMeth', () => vao.isVertexArrayOES.call({}, null));
        T('ext.argc', () => vao.bindVertexArrayOES());
        T('aniso.gate', () => [gl.getParameter(34047), gl2.getParameter(34047)].join('/'));
        T('aniso.on', () => { gl.getExtension('EXT_texture_filter_anisotropic'); return gl.getParameter(34047); });
        T('ev.ctor', () => { const e = new WebGLContextEvent('webglcontextlost', { cancelable: true, statusMessage: 'x' });
                             return [typeof WebGLContextEvent, e.constructor.name, e.cancelable, e.statusMessage,
                                     Object.getOwnPropertyNames(Object.getPrototypeOf(e)).join(','),
                                     Object.prototype.toString.call(e)].join('/'); });
        T('ev.length', () => WebGLContextEvent.length);
        T('wo', () => [Object.prototype.toString.call(WebGLObject.prototype), Reflect.ownKeys(WebGLObject.prototype).filter((x) => typeof x === 'string').join(','), !!WebGLObject.prototype[Symbol.toStringTag]].join('/'));
        // context-loss machine (sync flags now, events pumped below)
        const lc = document.createElement('canvas');
        globalThis.__lgl = lc.getContext('webgl');
        globalThis.__evts = [];
        lc.addEventListener('webglcontextlost', (e) => { e.preventDefault(); __evts.push('lost'); }, false);
        lc.addEventListener('webglcontextrestored', () => __evts.push('restored'), false);
        const wlc = __lgl.getExtension('WEBGL_lose_context');
        wlc.loseContext();
        T('lost.sync', () => [__lgl.isContextLost(), __lgl.getExtension('OES_element_index_uint'), __lgl.getParameter(__lgl.VERSION),
                              __lgl.getError(), __lgl.getSupportedExtensions().length].join('/'));
        globalThis.__wlc = wlc;
        return JSON.stringify(__log);
    "#).unwrap();
    let out = rt.evaluate("JSON.stringify(__log)").unwrap();
    let log: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let arr = log.as_array().unwrap();
    assert_eq!(
        serde_json::Value::Array(arr[..30].to_vec()),
        serde_json::json!([
            [
                "vao.mint",
                "ok",
                "[object WebGLVertexArrayObjectOES]/0/[object WebGLVertexArrayObjectOES]/[object WebGLObject]/true"
            ],
            ["vao.is0", "ok", "false"],
            ["vao.is0args", "ok", "false"],
            ["vao.isnull", "ok", "false"],
            [
                "vao.isbad",
                "TypeError",
                "Failed to execute 'isVertexArrayOES' on 'OESVertexArrayObject': parameter 1 is not of type 'WebGLVertexArrayObjectOES'."
            ],
            ["vao.bind", "ok", "true"],
            ["vao.bindnull", "ok", "0"],
            [
                "vao.bindbad",
                "TypeError",
                "Failed to execute 'bindVertexArrayOES' on 'OESVertexArrayObject': parameter 1 is not of type 'WebGLVertexArrayObjectOES'."
            ],
            ["vao.del", "ok", "false/false"],
            ["vao.argc", "ok", "[object WebGLVertexArrayObjectOES]"],
            ["tq.is0", "ok", "false"],
            ["tq.beginBad", "ok", "1280"],
            [
                "tq.beginNull",
                "TypeError",
                "Failed to execute 'beginQueryEXT' on 'EXTDisjointTimerQuery': parameter 2 is not of type 'WebGLTimerQueryEXT'."
            ],
            [
                "tq.beginNotQ",
                "TypeError",
                "Failed to execute 'beginQueryEXT' on 'EXTDisjointTimerQuery': parameter 2 is not of type 'WebGLTimerQueryEXT'."
            ],
            ["tq.flow", "ok", "true/true//0"],
            ["tq.bits", "ok", "64"],
            ["tq.obj", "ok", "false/0/0"],
            ["tq.objBadPname", "ok", "/1280"],
            [
                "tq.objBadType",
                "TypeError",
                "Failed to execute 'getQueryObjectEXT' on 'EXTDisjointTimerQuery': parameter 1 is not of type 'WebGLTimerQueryEXT'."
            ],
            ["tq.counter", "ok", "0/1280/true"],
            ["tq.beginTwice", "ok", "1282"],
            ["tq.gpuDisjoint", "ok", "false/0"],
            ["tq.delete", "ok", "false"],
            ["ds.uncompiled", "ok", ""],
            ["ds.compiled", "ok", "string/13"],
            [
                "ds.badtype",
                "TypeError",
                "Failed to execute 'getTranslatedShaderSource' on 'WebGLDebugShaders': parameter 1 is not of type 'WebGLShader'."
            ],
            ["astc", "ok", "ldr,hdr"],
            ["ia.draw", "ok", "1282"],
            ["ia.divisorOOB", "ok", "1281"],
            ["ia.divisorNeg", "ok", "1281"],
        ]),
        "WebGL extension method behavior (刀92) part 1: mint chains through [object WebGLObject], VAO activation semantics, timer-query family (begin/end/counter activation, error ladder, never-completing results), debug shaders compile gate, ANGLE draw/divisor errors"
    );
    assert_eq!(
        serde_json::Value::Array(arr[30..].to_vec()),
        serde_json::json!([
            [
                "ia.argc",
                "TypeError",
                "Failed to execute 'drawArraysInstancedANGLE' on 'ANGLEInstancedArrays': 4 arguments required, but only 0 present."
            ],
            ["db.ok", "ok", "0"],
            ["db.bad", "ok", "1282"],
            ["db.empty", "ok", "1282"],
            ["db.double", "ok", "1280"],
            [
                "db.seq",
                "TypeError",
                "Failed to execute 'drawBuffersWEBGL' on 'WebGLDrawBuffers': The provided value cannot be converted to a sequence."
            ],
            ["oi.ok", "ok", "0"],
            ["oi.oob", "ok", "1281"],
            ["oi.badCap", "ok", "1280"],
            ["cc.gate", "ok", "36001"],
            ["cc.set", "ok", "36002/37727/0"],
            ["cc.bad", "ok", "1280"],
            ["pm.face", "ok", "1280"],
            ["pm.ok", "ok", "0/6913"],
            ["pv", "ok", "0/36430"],
            ["md", "ok", "1282"],
            [
                "md.argc",
                "TypeError",
                "Failed to execute 'multiDrawArraysWEBGL' on 'WebGLMultiDraw': 6 arguments required, but only 0 present."
            ],
            ["tq2.core", "ok", "0/[object WebGLQuery]"],
            [
                "tq2.bad",
                "TypeError",
                "Failed to execute 'queryCounterEXT' on 'EXTDisjointTimerQueryWebGL2': parameter 1 is not of type 'WebGLQuery'."
            ],
            [
                "tq2.argc",
                "TypeError",
                "Failed to execute 'queryCounterEXT' on 'EXTDisjointTimerQueryWebGL2': 2 arguments required, but only 0 present."
            ],
            ["rx.getParam", "TypeError", "Illegal invocation"],
            ["rx.getExt", "TypeError", "Illegal invocation"],
            ["rx.getErr", "TypeError", "Illegal invocation"],
            ["rx.extMeth", "TypeError", "Illegal invocation"],
            ["ext.argc", "ok", "undef"],
            ["aniso.gate", "ok", "/"],
            ["aniso.on", "ok", "16"],
            [
                "ev.ctor",
                "ok",
                "function/WebGLContextEvent/true/x/statusMessage,constructor/[object WebGLContextEvent]"
            ],
            ["ev.length", "ok", "1"],
            ["wo", "ok", "[object WebGLObject]/constructor/true"],
            ["lost.sync", "ok", "true///37442/0"]
        ]),
        "part 2: drawBuffers length-before-element, blend-index family, clip/polygon/provoking state + enable-on-getExtension gating, multiDraw 1282, webgl2 timer typed against core WebGLQuery, receiver strictness (bare Illegal invocation), constructible WebGLContextEvent, lose/restore machine sync flags"
    );
    // pump 1: async lost event dispatch -> preventDefault observed -> restore legal (restoreContext before the
    // canceled lost event is a no-op, chrome truth k92-p3evt2)
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run_event_loop()).await;
    let out2 = rt
        .evaluate("JSON.stringify([JSON.stringify(__evts), __lgl.isContextLost()])")
        .unwrap();
    assert_eq!(out2.as_str().unwrap(), "[\"[\\\"lost\\\"]\",true]");
    // restore stays async: sync isContextLost still true, restored event lands on the next pump
    let out3 = rt
        .evaluate("__wlc.restoreContext(); String(__lgl.isContextLost())")
        .unwrap();
    assert_eq!(out3.as_str().unwrap(), "true");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run_event_loop()).await;
    let out4 = rt
        .evaluate("JSON.stringify([JSON.stringify(__evts), __lgl.isContextLost(), __lgl.getSupportedExtensions().length > 0, __lgl.getError()])")
        .unwrap();
    assert_eq!(
        out4.as_str().unwrap(),
        "[\"[\\\"lost\\\",\\\"restored\\\"]\",false,true,0]"
    );
}

#[tokio::test]
async fn knife93_unload_dead_and_webkit_handler_attributes() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        const div = document.createElement('div');
        // ---- synthetic unload fires NOTHING: listeners AND handlers, every target (k93-fire/fire2/fire3) ----
        T('ul.winLsn', () => { let n = 0; const f = () => n++; window.addEventListener('unload', f);
                               window.dispatchEvent(new Event('unload')); window.removeEventListener('unload', f); return n; });
        T('ul.winHdl', () => { let n = 0; window.onunload = () => n++; window.dispatchEvent(new Event('unload'));
                               window.onunload = null; return n; });
        T('ul.docLsn', () => { let n = 0; const f = () => n++; document.addEventListener('unload', f);
                               document.dispatchEvent(new Event('unload')); document.removeEventListener('unload', f); return n; });
        T('ul.docHdl', () => { let n = 0; document.onunload = () => n++; document.dispatchEvent(new Event('unload'));
                               document.onunload = null; return n; });
        T('ul.bodyHdl', () => { let n = 0; document.body.onunload = () => n++;
                                document.body.dispatchEvent(new Event('unload', { bubbles: true })); document.body.onunload = null; return n; });
        T('ul.divLsn', () => { let n = 0; const f = () => n++; div.addEventListener('unload', f);
                               div.dispatchEvent(new Event('unload')); div.removeEventListener('unload', f); return n; });
        T('ul.etLsn', () => { const et = new EventTarget(); let n = 0; const f = () => n++;
                              et.addEventListener('unload', f); et.dispatchEvent(new Event('unload')); return n; });
        T('ul.spanLsn', () => { const sp = document.createElement('span'); let n = 0; const f = () => n++;
                                sp.addEventListener('unload', f); sp.dispatchEvent(new Event('unload')); return n; });
        T('ul.ret', () => window.dispatchEvent(new Event('unload')));
        T('ul.bubBodyToWin', () => { let n = 0; window.addEventListener('unload', () => n++);
                                     document.body.dispatchEvent(new Event('unload', { bubbles: true })); return n; });
        // neighbors stay alive: beforeunload/pagehide fire on synthetic (the pipeline-only ones are error+unload only)
        T('bu.hdl', () => { let n = 0; window.onbeforeunload = () => n++; window.dispatchEvent(new Event('beforeunload'));
                            window.onbeforeunload = null; return n; });
        T('ph.hdl', () => { let n = 0; window.onpagehide = () => n++; window.dispatchEvent(new Event('pagehide'));
                            window.onpagehide = null; return n; });
        T('body.click', () => { let n = 0; document.body.onclick = () => n++;
                                document.body.dispatchEvent(new Event('click', { bubbles: true })); document.body.onclick = null; return n; });
        // ---- the four webkit-prefixed handler attributes: stored but NEVER fired (listener of the type still fires) ----
        T('wk.hdl', () => { let n = 0; div.onwebkitanimationend = () => n++; div.dispatchEvent(new Event('webkitanimationend'));
                            div.onwebkitanimationend = null; return n; });
        T('wk.lsn', () => { let n = 0; const f = () => n++; div.addEventListener('webkitanimationend', f);
                            div.dispatchEvent(new Event('webkitanimationend')); div.removeEventListener('webkitanimationend', f); return n; });
        T('wk.cross', () => { let n = 0; div.onwebkitanimationend = () => n++; div.dispatchEvent(new Event('animationend'));
                              div.onwebkitanimationend = null; return n; });
        T('wk.store', () => { const f = function zz() {}; div.onwebkitanimationend = f;
                              const back = div.onwebkitanimationend === f; div.onwebkitanimationend = null;
                              return [back, div.onwebkitanimationend === null].join('/'); });
        T('wk.win', () => { let n = 0; window.onwebkitanimationstart = () => n++;
                            window.dispatchEvent(new Event('webkitanimationstart')); window.onwebkitanimationstart = null; return n; });
        T('wk.doc', () => { let n = 0; document.onwebkitanimationiteration = () => n++;
                            document.dispatchEvent(new Event('webkitanimationiteration'));
                            document.onwebkitanimationiteration = null; return n; });
        T('wktrans', () => { let n = 0; div.onwebkittransitionend = () => n++;
                             div.dispatchEvent(new Event('webkittransitionend')); div.onwebkittransitionend = null; return n; });
        T('wk.desc', () => { const d = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'onwebkitanimationend');
                             return d ? [!!d.get, !!d.set, d.enumerable, d.configurable].join('/') : 'MISSING'; });
        // std handlers keep firing (the tail at large is alive — k93-fire div/document/video/svg/window sweeps)
        T('std.anim', () => { let n = 0; div.onanimationend = () => n++; div.dispatchEvent(new Event('animationend'));
                              div.onanimationend = null; return n; });
        T('std.video', () => { const v = document.createElement('video'); let n = 0; v.onpause = () => n++;
                               v.dispatchEvent(new Event('pause')); v.onpause = null; return n; });
        T('std.win', () => { let n = 0; window.onresize = () => n++; window.dispatchEvent(new Event('resize'));
                             window.onresize = null; return n; });
        return JSON.stringify(__log);
    "#).unwrap();
    let log = rt.evaluate("JSON.stringify(__log)").unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["ul.winLsn", "ok", "0"],
            ["ul.winHdl", "ok", "0"],
            ["ul.docLsn", "ok", "0"],
            ["ul.docHdl", "ok", "0"],
            ["ul.bodyHdl", "ok", "0"],
            ["ul.divLsn", "ok", "0"],
            ["ul.etLsn", "ok", "0"],
            ["ul.spanLsn", "ok", "0"],
            ["ul.ret", "ok", "true"],
            ["ul.bubBodyToWin", "ok", "0"],
            ["bu.hdl", "ok", "1"],
            ["ph.hdl", "ok", "1"],
            ["body.click", "ok", "1"],
            ["wk.hdl", "ok", "0"],
            ["wk.lsn", "ok", "1"],
            ["wk.cross", "ok", "0"],
            ["wk.store", "ok", "true/true"],
            ["wk.win", "ok", "0"],
            ["wk.doc", "ok", "0"],
            ["wktrans", "ok", "0"],
            ["wk.desc", "ok", "true/true/true/true"],
            ["std.anim", "ok", "1"],
            ["std.video", "ok", "1"],
            ["std.win", "ok", "1"]
        ]),
        "knife93: synthetic unload fires nothing on any target; onwebkit* four attributes stored-but-dead; the rest of the tail stays alive"
    );
}

#[tokio::test]
async fn knife94_highlight_registry_map_like() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', v === undefined ? 'undef' : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        const reg = CSS.highlights;
        const rg = document.createRange();
        // ---- Highlight: Set-like of AbstractRanges ----
        T('h.ctor', () => [new Highlight(rg).size, new Highlight().size, new Highlight(rg, document.createRange()).size].join('/'));
        T('h.ctorBad', () => new Highlight(rg, {}));
        T('h.ctorBad1', () => new Highlight({}));
        T('h.add', () => { const h = new Highlight(); h.add(rg); return [h.size, h.has(rg), h.add(rg) === h, h.size].join('/'); });
        T('h.addBad', () => { const h = new Highlight(); h.add({}); return 'no'; });
        T('h.hasBad', () => { const h = new Highlight(rg); return h.has(999); });
        T('h.del', () => { const h = new Highlight(rg); return [h.delete(rg), h.size, h.delete(rg)].join('/'); });
        T('h.clear', () => { const h = new Highlight(rg); h.clear(); return h.size; });
        T('h.type', () => { const h = new Highlight(rg); const t0 = h.type; h.type = 'text-highlight'; return [t0, h.type].join('/'); });
        T('h.priority', () => { const h = new Highlight(rg); const p0 = h.priority; h.priority = 7; return [p0, h.priority].join('/'); });
        T('h.iter', () => { const h = new Highlight(rg); let s = ''; for (const x of h) s += (x === rg); return s; });
        T('h.symIsValues', () => Highlight.prototype[Symbol.iterator] === Highlight.prototype.values);
        T('h.rx', () => Highlight.prototype.add.call({}, rg));
        // ---- HighlightRegistry: Map-like ----
        const h = new Highlight(rg);
        T('reg.setGet', () => { reg.set('k1', h); return [reg.size, reg.get('k1') === h, reg.has('k1')].join('/'); });
        T('reg.overwrite', () => { const h2 = new Highlight(); reg.set('k1', h2); return [reg.size, reg.get('k1') === h2].join('/'); });
        T('reg.order', () => { reg.set('b', h); reg.set('a', h); const ks = [...reg.keys()].join(','); reg.delete('k1'); reg.delete('b'); reg.delete('a'); return ks; });
        T('reg.miss', () => [reg.get('nosuch'), reg.has('nosuch'), reg.delete('nosuch')].join('/'));
        T('reg.clear', () => { reg.set('x', h); reg.clear(); return reg.size; });
        T('reg.iter', () => { reg.set('i1', h); let s = ''; for (const [k, v] of reg) s += k + '=' + (v === h) + ';'; reg.delete('i1'); return s; });
        T('reg.forEach', () => { reg.set('fa', h); let out = ''; reg.forEach((v, k, m) => { out = [v === h, k, m === reg].join('/'); }); reg.delete('fa'); return out; });
        T('reg.symIsEntries', () => reg[Symbol.iterator] === reg.entries);
        T('reg.hfp', () => { reg.set('fp', h); const v = reg.highlightsFromPoint(0, 0); reg.delete('fp'); return Array.isArray(v) ? 'arr:' + v.length : typeof v; });
        T('reg.rx', () => HighlightRegistry.prototype.get.call({}, 'x'));
        T('reg.setBadVal', () => reg.set('x', {}));
        T('reg.set1', () => reg.set('a'));
        T('reg.get0', () => reg.get());
        return JSON.stringify(__log);
    "#).unwrap();
    let log = rt.evaluate("JSON.stringify(__log)").unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["h.ctor", "ok", "1/0/2"],
            [
                "h.ctorBad",
                "TypeError",
                "Failed to construct 'Highlight': parameter 2 is not of type 'AbstractRange'."
            ],
            [
                "h.ctorBad1",
                "TypeError",
                "Failed to construct 'Highlight': parameter 1 is not of type 'AbstractRange'."
            ],
            ["h.add", "ok", "1/true/true/1"],
            [
                "h.addBad",
                "TypeError",
                "Failed to execute 'add' on 'Highlight': parameter 1 is not of type 'AbstractRange'."
            ],
            [
                "h.hasBad",
                "TypeError",
                "Failed to execute 'has' on 'Highlight': parameter 1 is not of type 'AbstractRange'."
            ],
            ["h.del", "ok", "true/0/false"],
            ["h.clear", "ok", "0"],
            ["h.type", "ok", "highlight/highlight"],
            ["h.priority", "ok", "0/7"],
            ["h.iter", "ok", "true"],
            ["h.symIsValues", "ok", "true"],
            ["h.rx", "TypeError", "Illegal invocation"],
            ["reg.setGet", "ok", "1/true/true"],
            ["reg.overwrite", "ok", "1/true"],
            ["reg.order", "ok", "k1,b,a"],
            ["reg.miss", "ok", "/false/false"],
            ["reg.clear", "ok", "0"],
            ["reg.iter", "ok", "i1=true;"],
            ["reg.forEach", "ok", "true/fa/true"],
            ["reg.symIsEntries", "ok", "true"],
            ["reg.hfp", "ok", "arr:0"],
            ["reg.rx", "TypeError", "Illegal invocation"],
            [
                "reg.setBadVal",
                "TypeError",
                "Failed to execute 'set' on 'HighlightRegistry': parameter 2 is not of type 'Highlight'."
            ],
            [
                "reg.set1",
                "TypeError",
                "Failed to execute 'set' on 'HighlightRegistry': 2 arguments required, but only 1 present."
            ],
            [
                "reg.get0",
                "TypeError",
                "Failed to execute 'get' on 'HighlightRegistry': 1 argument required, but only 0 present."
            ]
        ]),
        "knife94: HighlightRegistry Map-like + Highlight Set-like real bodies (Chrome 145 truth)"
    );
}

#[tokio::test]
async fn knife95_instance_method_ladders() {
    // (刀95) required-argument ladders reach the INSTANCE-REQUIRING interfaces
    // (elements/document/window/storage/observers/2d-canvas…): every message
    // below is a k95 Chrome-probe truth row. Plus the three hand finds: the
    // forEach pre-iteration throw, the eight Chrome length-0 window methods,
    // webkitURL's new-check-first message, the trustedTypes prototype link
    // and scheduler's prototype landing with Chrome's descriptor shape.
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const L = [];
        const T = (k, f) => { try { const v = f(); L.push([k, 'ok', v === undefined ? '' : String(v)]); } catch (e) { L.push([k, 'E', e.name + ':' + e.message]); } };
        const div = document.createElement('div');
        T('el.setAttribute', () => div.setAttribute());
        T('el.closest', () => div.closest());
        T('el.animate', () => div.animate());
        T('el.attachShadow', () => div.attachShadow());
        T('doc.createElement', () => document.createElement());
        T('doc.querySelector', () => document.querySelector());
        T('doc.createElementNS', () => document.createElementNS());
        T('node.appendChild', () => div.appendChild());
        T('node.insertBefore', () => div.insertBefore());
        T('cd.appendData', () => document.createTextNode('x').appendData());
        T('sel.collapse', () => getSelection().collapse());
        T('sel.setBaseAndExtent', () => getSelection().setBaseAndExtent());
        T('hist.pushState', () => history.pushState());
        T('ctx.measureText', () => document.createElement('canvas').getContext('2d').measureText());
        T('ctx.drawImage', () => document.createElement('canvas').getContext('2d').drawImage());
        T('nav.getUserMedia', () => navigator.getUserMedia());
        T('mo.observe', () => new MutationObserver(function(){}).observe());
        T('win.setTimeout', () => setTimeout());
        T('win.matchMedia', () => matchMedia());
        T('win.getComputedStyle', () => getComputedStyle());
        T('loc.assign', () => location.assign());
        T('port.postMessage', () => new MessageChannel().port1.postMessage());
        T('svg.getPointAtLength', () => document.createElementNS('http://www.w3.org/2000/svg', 'path').getPointAtLength());
        T('classList.forEach', () => document.body.classList.forEach());
        L.push(['winLen0', 'ok', [alert, confirm, prompt, scroll, scrollBy, scrollTo, clearInterval, clearTimeout].map((f) => f.length).join(',')]);
        T('webkitURL', () => webkitURL());
        L.push(['ttpf', 'ok', [trustedTypes instanceof TrustedTypePolicyFactory, (function(){try{trustedTypes.createPolicy();return 'no'}catch(e){return e.name}})()].join('/')]);
        L.push(['sched', 'ok', [scheduler instanceof Scheduler, Scheduler.prototype.postTask.length, Object.getOwnPropertyDescriptor(Scheduler.prototype, 'postTask').enumerable].join('/')]);
        globalThis.__r95 = null;
        Promise.all([fetch().catch((e) => 'F:' + e.name + ':' + e.message),
                     document.requestStorageAccessFor().catch((e) => 'R:' + e.name + ':' + e.message),
                     createImageBitmap().catch((e) => 'C:' + e.name + ':' + e.message)])
          .then((a) => { globalThis.__r95 = a; });
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            [
                "el.setAttribute",
                "E",
                "TypeError:Failed to execute 'setAttribute' on 'Element': 2 arguments required, but only 0 present."
            ],
            [
                "el.closest",
                "E",
                "TypeError:Failed to execute 'closest' on 'Element': 1 argument required, but only 0 present."
            ],
            [
                "el.animate",
                "E",
                "TypeError:Failed to execute 'animate' on 'Element': 1 argument required, but only 0 present."
            ],
            [
                "el.attachShadow",
                "E",
                "TypeError:Failed to execute 'attachShadow' on 'Element': 1 argument required, but only 0 present."
            ],
            [
                "doc.createElement",
                "E",
                "TypeError:Failed to execute 'createElement' on 'Document': 1 argument required, but only 0 present."
            ],
            [
                "doc.querySelector",
                "E",
                "TypeError:Failed to execute 'querySelector' on 'Document': 1 argument required, but only 0 present."
            ],
            [
                "doc.createElementNS",
                "E",
                "TypeError:Failed to execute 'createElementNS' on 'Document': 2 arguments required, but only 0 present."
            ],
            [
                "node.appendChild",
                "E",
                "TypeError:Failed to execute 'appendChild' on 'Node': 1 argument required, but only 0 present."
            ],
            [
                "node.insertBefore",
                "E",
                "TypeError:Failed to execute 'insertBefore' on 'Node': 2 arguments required, but only 0 present."
            ],
            [
                "cd.appendData",
                "E",
                "TypeError:Failed to execute 'appendData' on 'CharacterData': 1 argument required, but only 0 present."
            ],
            [
                "sel.collapse",
                "E",
                "TypeError:Failed to execute 'collapse' on 'Selection': 1 argument required, but only 0 present."
            ],
            [
                "sel.setBaseAndExtent",
                "E",
                "TypeError:Failed to execute 'setBaseAndExtent' on 'Selection': 4 arguments required, but only 0 present."
            ],
            [
                "hist.pushState",
                "E",
                "TypeError:Failed to execute 'pushState' on 'History': 2 arguments required, but only 0 present."
            ],
            [
                "ctx.measureText",
                "E",
                "TypeError:Failed to execute 'measureText' on 'CanvasRenderingContext2D': 1 argument required, but only 0 present."
            ],
            [
                "ctx.drawImage",
                "E",
                "TypeError:Failed to execute 'drawImage' on 'CanvasRenderingContext2D': 3 arguments required, but only 0 present."
            ],
            [
                "nav.getUserMedia",
                "E",
                "TypeError:Failed to execute 'getUserMedia' on 'Navigator': 3 arguments required, but only 0 present."
            ],
            [
                "mo.observe",
                "E",
                "TypeError:Failed to execute 'observe' on 'MutationObserver': 1 argument required, but only 0 present."
            ],
            [
                "win.setTimeout",
                "E",
                "TypeError:Failed to execute 'setTimeout' on 'Window': 1 argument required, but only 0 present."
            ],
            [
                "win.matchMedia",
                "E",
                "TypeError:Failed to execute 'matchMedia' on 'Window': 1 argument required, but only 0 present."
            ],
            [
                "win.getComputedStyle",
                "E",
                "TypeError:Failed to execute 'getComputedStyle' on 'Window': 1 argument required, but only 0 present."
            ],
            [
                "loc.assign",
                "E",
                "TypeError:Failed to execute 'assign' on 'Location': 1 argument required, but only 0 present."
            ],
            [
                "port.postMessage",
                "E",
                "TypeError:Failed to execute 'postMessage' on 'MessagePort': 1 argument required, but only 0 present."
            ],
            [
                "svg.getPointAtLength",
                "E",
                "TypeError:Failed to execute 'getPointAtLength' on 'SVGGeometryElement': 1 argument required, but only 0 present."
            ],
            [
                "classList.forEach",
                "E",
                "TypeError:undefined is not a function"
            ],
            ["winLen0", "ok", "0,0,0,0,0,0,0,0"],
            [
                "webkitURL",
                "E",
                "TypeError:Failed to construct 'URL': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
            ],
            ["ttpf", "ok", "true/TypeError"],
            ["sched", "ok", "true/1/true"]
        ])
    );
    // promise legs: Chrome REJECTS with the same argc TypeErrors
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run_event_loop()).await;
    let out = rt.evaluate("JSON.stringify(__r95)").unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        r#"["F:TypeError:Failed to execute 'fetch' on 'Window': 1 argument required, but only 0 present.","R:TypeError:Failed to execute 'requestStorageAccessFor' on 'Document': 1 argument required, but only 0 present.","C:TypeError:Failed to execute 'createImageBitmap' on 'Window': 1 argument required, but only 0 present."]"#
    );
}

#[tokio::test]
async fn knife96_legacy_alias_identity() {
    // (刀96) [LegacyAlias] names are the SAME interface object in Chrome:
    // webkitURL === URL, WebKitCSSMatrix === DOMMatrix (k96 sweep found
    // exactly these two missing; the webkitMediaStream/Speech family and
    // WebKitMutationObserver were already true, Image/Audio/Option stay
    // separate in both engines). The URL function ctor wraps the class so
    // the no-new call throws Chrome's DOM new-check message, and the
    // WebKitCSSMatrix legacy-factory twin is deleted — DOMMatrix's ctor
    // already parses transform-list strings AND sequences with the unified
    // 'DOMMatrix' error context.
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const L = [];
        const T = (k, f) => { try { const v = f(); L.push([k, 'ok', v === undefined ? '' : String(v)]); } catch (e) { L.push([k, 'E', e.name + ':' + e.message]); } };
        T('alias.url', () => webkitURL === URL);
        T('alias.cssm', () => WebKitCSSMatrix === DOMMatrix);
        T('alias.ms', () => webkitMediaStream === MediaStream);
        T('alias.mo', () => WebKitMutationObserver === MutationObserver);
        T('url.call', () => URL('x'));
        T('wkurl.call', () => webkitURL('x'));
        T('url.new0', () => new URL());
        T('wkurl.str', () => new webkitURL('http://a.b/p').href);
        T('wkurl.statics', () => Object.getOwnPropertyNames(webkitURL).filter((n) => n !== 'name' && n !== 'length' && n !== 'prototype').join(','));
        T('url.len', () => URL.length + '/' + URL.name + '/' + (URL.prototype.constructor === URL) + '/' + String(URL).slice(0, 30));
        T('cssm.seq', () => { const m = new WebKitCSSMatrix([1, 2, 3, 4, 5, 6]); return m.a + ',' + m.f; });
        T('cssm.bad', () => new WebKitCSSMatrix('garbage('));
        T('cssm.num', () => new WebKitCSSMatrix(5));
        T('cssm.empty', () => new WebKitCSSMatrix().toString());
        T('cssm.statics', () => Object.getOwnPropertyNames(WebKitCSSMatrix).filter((n) => n !== 'name' && n !== 'length' && n !== 'prototype').join(','));
        T('url.sub', () => { class X extends URL {}; const x = new X('http://a.b/p'); return x.href + '/' + (x instanceof X) + '/' + (x instanceof URL); });
        T('wkurl.parse', () => webkitURL.parse('http://a.b/') !== null);
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["alias.url", "ok", "true"],
            ["alias.cssm", "ok", "true"],
            ["alias.ms", "ok", "true"],
            ["alias.mo", "ok", "true"],
            [
                "url.call",
                "E",
                "TypeError:Failed to construct 'URL': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
            ],
            [
                "wkurl.call",
                "E",
                "TypeError:Failed to construct 'URL': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
            ],
            [
                "url.new0",
                "E",
                "TypeError:Failed to construct 'URL': 1 argument required, but only 0 present."
            ],
            ["wkurl.str", "ok", "http://a.b/p"],
            [
                "wkurl.statics",
                "ok",
                "canParse,parse,createObjectURL,revokeObjectURL"
            ],
            ["url.len", "ok", "1/URL/true/function URL() { [native code]"],
            ["cssm.seq", "ok", "1,6"],
            [
                "cssm.bad",
                "E",
                "SyntaxError:Failed to construct 'DOMMatrix': Failed to parse 'garbage('."
            ],
            [
                "cssm.num",
                "E",
                "SyntaxError:Failed to construct 'DOMMatrix': Failed to parse '5'."
            ],
            ["cssm.empty", "ok", "matrix(1, 0, 0, 1, 0, 0)"],
            [
                "cssm.statics",
                "ok",
                "fromFloat32Array,fromFloat64Array,fromMatrix"
            ],
            ["url.sub", "ok", "http://a.b/p/true/true"],
            ["wkurl.parse", "ok", "true"]
        ]),
        "knife96: LegacyAlias identity (webkitURL/WebKitCSSMatrix) + unified ctor faces (Chrome 145 truth)"
    );
}

#[tokio::test]
async fn knife97_font_metrics_model() {
    // (刀97) canvas measureText = real per-glyph advances from the captured
    // Chrome 145 tables (font_metrics.js): per-font ASCII rows, EXT rows for
    // non-ASCII (·/—/…/quotes), CJK exactly 1em/char by code point (astral
    // pairs count once), Hangul 0.865em, system-ui = SF Pro's continuous
    // optical axis (interpolated per-char em over knots — H is WIDER at small
    // sizes), Apple Color Emoji's 13 keycap-source chars take the bitmap
    // integer ladder below 24px, generic families resolve unquoted
    // (monospace=Menlo, vert 15/4) while quoted "monospace" keeps the hidden
    // face rows, absent families fall back to the default table, and
    // fontBoundingBox comes from per-font asc/desc ratios with per-size
    // overrides. Kerning is NOT modeled (documented residue; the un-kerned
    // rows here are exact Chrome parity, verified against the live browser).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const L = [];
        const T = (k, f) => { try { const v = f(); L.push([k, 'ok', v === undefined ? '' : String(v)]); } catch (e) { L.push([k, 'E', e.name + ':' + e.message]); } };
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        const V = (font) => { x.font = font; const m = x.measureText('Hg'); return m.fontBoundingBoxAscent + '/' + m.fontBoundingBoxDescent; };
        T('menlo.lin10', () => W('10px Menlo', 'Hello World gjpq 123'));
        T('courier.lin16', () => W('16px "Courier New"', 'Hello World gjpq 123'));
        T('mono.face', () => W('100px monospace', 'mnmn_mnmn'));
        T('menlo.face', () => W('100px Menlo', 'mnmn_mnmn'));
        T('generic.equal', () => W('100px sans-serif', 'Hello') === W('100px -apple-system', 'Hello') && W('100px ui-sans-serif', 'Hello') === W('100px __absent__', 'Hello') && W('100px ui-monospace', 'Hello') === W('100px __absent__', 'Hello'));
        T('systemui.differs', () => W('100px system-ui', 'Hello') !== W('100px sans-serif', 'Hello'));
        T('systemui.H10', () => W('10px system-ui', 'H'));
        T('systemui.H100', () => W('100px system-ui', 'H'));
        T('emoji.one16', () => W('16px "Apple Color Emoji"', '1'));
        T('emoji.one32', () => W('32px "Apple Color Emoji"', '1'));
        T('emoji.H16', () => W('16px "Apple Color Emoji"', 'H'));
        T('ext.middot.arial', () => W('100px Arial', '·'));
        T('ext.middot.def', () => W('100px __absent__', '·'));
        T('ext.quote.arial', () => W('100px Arial', '“'));
        T('ext.emdash', () => W('100px Times New Roman', '—'));
        T('cjk', () => W('100px __absent__', '你好'));
        T('hangul', () => W('100px __absent__', '한국'));
        T('astral', () => W('100px __absent__', '𠀀'));
        T('missing.font', () => W('100px NoSuchFontHere', 'Hello') === W('100px __absent__', 'Hello'));
        T('pingfang.default', () => W('72px "PingFang SC"', 'Hello World gjpq 123') === W('72px __absent__', 'Hello World gjpq 123'));
        T('zapfino.differs', () => W('72px Zapfino', 'Hello World gjpq 123') !== W('72px __absent__', 'Hello World gjpq 123'));
        T('vert.menlo16', () => V('16px Menlo'));
        T('vert.mono16', () => V('16px monospace'));
        T('vert.arial32', () => V('32px Arial'));
        T('vert.emoji100', () => V('100px "Apple Color Emoji"'));
        T('vert.systemui10', () => V('10px system-ui'));
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["menlo.lin10", "ok", "120.4102"],
            ["courier.lin16", "ok", "192.0314"],
            ["mono.face", "ok", "541.8459"],
            ["menlo.face", "ok", "541.8459"],
            ["generic.equal", "ok", "true"],
            ["systemui.differs", "ok", "true"],
            ["systemui.H10", "ok", "7.5391"],
            ["systemui.H100", "ok", "69.9219"],
            ["emoji.one16", "ok", "20"],
            ["emoji.one32", "ok", "32"],
            ["emoji.H16", "ok", "11.52"],
            ["ext.middot.arial", "ok", "33.3008"],
            ["ext.middot.def", "ok", "50"],
            ["ext.quote.arial", "ok", "33.3008"],
            ["ext.emdash", "ok", "100"],
            ["cjk", "ok", "200"],
            ["hangul", "ok", "173"],
            ["astral", "ok", "100"],
            ["missing.font", "ok", "true"],
            ["pingfang.default", "ok", "true"],
            ["zapfino.differs", "ok", "true"],
            ["vert.menlo16", "ok", "15/4"],
            ["vert.mono16", "ok", "15/4"],
            ["vert.arial32", "ok", "29/7"],
            ["vert.emoji100", "ok", "100/31"],
            ["vert.systemui10", "ok", "10/2"]
        ]),
        "knife97: per-glyph measureText model (Chrome 145 font-metrics truth)"
    );
}

#[tokio::test]
async fn knife98_kern_matrix_model() {
    // (刀98) measureText kern: the full pair matrix (28,907 pairs over 39
    // fonts, px@100 exactly linear for fixed fonts — Arial AV -7.42@100 =
    // -1.1872@16), system-ui's optical axis on TOP of it (knots em@16/20/24,
    // flat <=16, blends to exactly-linear >=28: Ta -13.18/-1.64/-3.6904 at
    // 100/16/28) including the 2,362 optical-only pairs that kern ~0 at 100
    // but track at text sizes ('wo' -0.313px@16), GPOS device-table bands
    // (px*100 at integer sizes 6..23 for every kern/optical pair — '¬Ý' at 7
    // is exact to 4dp), ligature pairs absorbed as ordinary pairs (quoted
    // "monospace" ff -6, Songti/Kaiti fi/fl), and the triple matrix
    // (36,943 contextual exceptions over 51 fonts: Zapfino ffi +10.25 / 0th
    // -24.25, Marker Felt fib -4.1 and script-join chains, default fff +6,
    // system-ui's exotic punctuation chains px@100-linear with optical
    // residue <=0.47px). CJK/Hangul kern and triples = 0 (widths are pure
    // per-glyph sums). xk knot floor extended to 5px (H 3.9111@5). All rows
    // (刀100 repin) 14 rows moved with the 刀100 truth upgrades (composite full-
    // matrix kern, su triple baseline, 4dp S rows) and were re-verified against
    // live Chrome 2026-11-01 — 13 within 0.0051 (several now exact);
    // tri.suExotic16 stayed 0.23 off under live Chrome (su small-px
    // non-linearity, 刀101 backlog) until 刀103's triSpot anchors landed.
    // verified against live Chrome 145 within 0.0405px (worst leg: 24px
    // knot-region text); expectations pin the model, not Chrome's float.
    // (刀101 repin) six rows moved with the kb 0.002 re-gate + as 0.002 gate
    // and were re-verified against live Chrome (all ≤0.0012; suTa16's new
    // -1.64 IS chrome to 0.0006); (刀103 repin) tri.suExotic16 24.4092 →
    // 24.6433 — the triSpot curve IS chrome exactly; the 0.23 model pin
    // retired with the backlog it was holding.
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const L = [];
        const T = (k, f) => { try { const v = f(); L.push([k, 'ok', v === undefined ? '' : String(v)]); } catch (e) { L.push([k, 'E', e.name + ':' + e.message]); } };
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        const R = (font, a, b) => +(W(font, a + b) - W(font, a) - W(font, b)).toFixed(4);
        T('pair.arial100', () => R('100px Arial', 'A', 'V'));
        T('pair.arial16', () => R('16px Arial', 'A', 'V'));
        T('pair.suTa100', () => R('100px system-ui', 'T', 'a'));
        T('pair.suTa16', () => R('16px system-ui', 'T', 'a'));
        T('pair.suTa28', () => R('28px system-ui', 'T', 'a'));
        T('pair.monoFF', () => R('100px "monospace"', 'f', 'f'));
        T('su.tavern16', () => W('16px system-ui', 'Tavern'));
        T('su.avatar18', () => W('18px system-ui', 'AVATAR'));
        T('su.tavern24', () => W('24px system-ui', 'Tavern'));
        T('su.optical8', () => W('8px system-ui', 'Wa wo'));
        T('su.band7', () => W('7px system-ui', '¬Ý'));
        T('su.band17', () => W('17px system-ui', '&V'));
        T('su.band23', () => W('23px system-ui', '"Æ'));
        T('su.s67', () => W('5px system-ui', 'H') + '/' + W('6px system-ui', 'H') + '/' + W('7px system-ui', 'H'));
        T('lig.songti16', () => W('16px "Songti SC"', 'office fluff waffle'));
        T('lig.kaiti24', () => W('24px "Kaiti SC"', 'office fluff'));
        T('tri.zapf32', () => W('32px Zapfino', 'ffi ffl Age Te. office'));
        T('tri.zap0th100', () => W('100px Zapfino', '0th'));
        T('tri.zap0th16', () => W('16px Zapfino', '0th'));
        T('tri.bask16', () => W('16px Baskerville', 'office ffl ffi'));
        T('tri.mf16', () => W('16px "Marker Felt"', 'a fib flub office "fi fi!'));
        T('tri.mf40', () => W('40px "Marker Felt"', 'a fib flub office "fi fi!'));
        T('tri.fff100', () => W('100px __absent__', 'waffle office fff'));
        T('tri.suExotic100', () => W('100px system-ui', '⁏²Æ'));
        T('tri.suExotic16', () => W('16px system-ui', '⁏²Æ'));
        T('cjk.zero', () => W('100px "PingFang SC"', '你好吗'));
        T('hangul.zero', () => W('16px "PingFang SC"', '한국'));
        T('monoQ.flat8', () => W('8px "monospace"', 'Tavern AV ff fi'));
        T('monoQ.flat100', () => W('100px "monospace"', 'Tavern AV ff fi'));
        T('fix.georgia33', () => W('33px Georgia', 'AVATAR. To. wo'));
        T('fix.times32', () => W('32px "Times New Roman"', 'A’’s barn —— To.'));
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["pair.arial100", "ok", "-7.4219"],
            ["pair.arial16", "ok", "-1.1876"],
            ["pair.suTa100", "ok", "-13.1836"],
            ["pair.suTa16", "ok", "-1.6406"],
            ["pair.suTa28", "ok", "-3.6914"],
            ["pair.monoFF", "ok", "-6"],
            ["su.tavern16", "ok", "48.2266"],
            ["su.avatar18", "ok", "63.497"],
            ["su.tavern24", "ok", "68.3251"],
            ["su.optical8", "ok", "25.8103"],
            ["su.band7", "ok", "9.4564"],
            ["su.band17", "ok", "21.6069"],
            ["su.band23", "ok", "29.4162"],
            ["su.s67", "ok", "3.9111/4.6934/5.4255"],
            ["lig.songti16", "ok", "111.664"],
            ["lig.kaiti24", "ok", "101.664"],
            ["tri.zapf32", "ok", "427.6"],
            ["tri.zap0th100", "ok", "196.75"],
            ["tri.zap0th16", "ok", "31.48"],
            ["tri.bask16", "ok", "67.3357"],
            ["tri.mf16", "ok", "118.688"],
            ["tri.mf40", "ok", "296.72"],
            ["tri.fff100", "ok", "712.4"],
            ["tri.suExotic100", "ok", "152.4582"],
            ["tri.suExotic16", "ok", "24.6433"],
            ["cjk.zero", "ok", "300"],
            ["hangul.zero", "ok", "27.68"],
            ["monoQ.flat8", "ok", "53.824"],
            ["monoQ.flat100", "ok", "672.8"],
            ["fix.georgia33", "ok", "246.0175"],
            ["fix.times32", "ok", "235.3594"]
        ]),
        "knife98: full kern matrix model (Chrome 145 fonts-kern truth)"
    );
}

#[tokio::test]
async fn knife99_face_axis_model() {
    // (刀99) ctx.font face axis: the style/variant/weight tokens before the
    // size resolve to per-font FACE tables (B/I/Z/M/S/100/200/300/600/800/900)
    // — pure lookup, no synthesis. Quoted generics are hidden faces
    // (quoted bold monospace = DEFBOLD PingFang Semibold 814.6; quoted serif
    // italic == system-ui italic 790.7) while unquoted generics resolve
    // through the generic table (bold serif = Songti bold 853.3). Near-R
    // numeric weights inherit the R table + R kern + ext overrides (bold
    // Zapfino == regular 117.09, w100 Menlo 144.4922). Small-caps quantizes:
    // advance spots 6..96 (non-multiples of 10) and kern bands 6..32
    // (scKernBands 2,520 pairs, band-length-driven cap in kernOf — 'lavatory
    // fi' at 24px is band-exact 108.0699). su numeric weights are real faces
    // with own kk knots + v100 + GPOS band overrides at the 0.008px deviation
    // gate (600@18 = 169.774, incl. fractional 14.5/10.5px), and band-only
    // pairs (zero at the old knot capture's cutoff, e.g. 900 'st' knots
    // [-8,-6,-1]) are swept in. kernFaces tables carry 4dp @100 anchors.
    // (刀100 d repin) five rows moved with the 刀100 truth upgrades (4dp S rows,
    // composite full-matrix kern, DEFBOLD index fix) and were re-verified
    // against Chrome before re-pinning — new pins are all ≤0.0006 from live.
    // All rows verified against live Chrome 145 within 0.0051px; expectations
    // pin the model, not Chrome's float.
    // (刀101 repin) three rows moved with the 刀101 truth re-pin, re-verified
    // against live Chrome (all ≤0.0014).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const L = [];
        const T = (k, f) => { try { const v = f(); L.push([k, 'ok', v === undefined ? '' : String(v)]); } catch (e) { L.push([k, 'E', e.name + ':' + e.message]); } };
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        T('face.boldArial100', () => W('bold 100px Arial', 'Hamburgefonstiv m AV P. Y,'));
        T('face.italBask100', () => W('italic 100px Baskerville', 'Hamburgefonstiv rt av'));
        T('face.boldItalTimes100', () => W('bold italic 100px Times New Roman', 'Hamburgefonstiv AV Ta'));
        T('face.scArial100', () => W('small-caps 100px Arial', 'hello world Ham'));
        T('face.w500helv32', () => W('500 32px Helvetica', 'Hamburgefonstiv'));
        T('face.w300songti24', () => W('300 24px Songti SC', 'Hamburgefonstiv'));
        T('face.w100menlo16', () => W('100 16px Menlo', 'Hamburgefonstiv'));
        T('face.w900su40', () => W('900 40px system-ui', 'Hamburgefonstiv Ta'));
        T('face.boldSu16', () => W('bold 16px system-ui', 'Ta To Tr av rt'));
        T('face.boldItalSu24', () => W('bold italic 24px system-ui', 'Hamburgefonstiv'));
        T('face.scSu16', () => W('small-caps 16px system-ui', 'hello small caps'));
        T('face.w500su16', () => W('500 16px system-ui', 'Ta Ham m'));
        T('face.boldSimSun16', () => W('bold 16px SimSun', 'Hamburgefonstiv'));
        T('face.boldZapf12', () => W('bold 12px Zapfino', 'Hamburgefonstiv'));
        T('genq.monoBold100', () => W('bold 100px "monospace"', 'Hamburgefonstiv'));
        T('genq.serifItal100', () => W('italic 100px "serif"', 'Hamburgefonstiv'));
        T('genq.cursive100', () => W('100px "cursive"', 'Hamburgefonstiv'));
        T('unq.serifBold100', () => W('bold 100px serif', 'Hamburgefonstiv Ta'));
        T('unq.monoSc100', () => W('small-caps 100px monospace', 'hello world Ham'));
        T('sc.spot8', () => W('small-caps 8px Arial', 'hello'));
        T('sc.spot32', () => W('small-caps 32px Arial', 'hello'));
        T('sc.kern24arial', () => W('small-caps 24px Arial', 'lavatory fi'));
        T('sc.kern24times', () => W('small-caps 24px Times New Roman', 'lavatory'));
        T('su.w600_18', () => W('600 18px system-ui', 'Hamburgefonstiv Ta'));
        T('su.w800_16', () => W('800 16px system-ui', 'Hamburgefonstiv Ta'));
        T('su.w900_13', () => W('900 13px system-ui', 'Hamburgefonstiv'));
        T('su.bold13', () => W('bold 13px system-ui', 'Hamburgefonstiv'));
        T('su.w900_145', () => W('900 14.5px system-ui', 'Hamburgefonstiv'));
        T('su.w800_105', () => W('800 10.5px system-ui', 'Ta av'));
        T('face.genBold', () => W('bold 100px Geneva', 'Hamburgefonstiv m'));
        T('face.w600futura100', () => W('600 100px Futura', 'Hamburgefonstiv'));
        T('face.boldMenloFix', () => W('bold 100px Menlo', '⁄ fix'));
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["face.boldArial100", "ok", "1318.8475"],
            ["face.italBask100", "ok", "757.3242"],
            ["face.boldItalTimes100", "ok", "1000.1464"],
            ["face.scArial100", "ok", "722.8859"],
            ["face.w500helv32", "ok", "241.8906"],
            ["face.w300songti24", "ok", "172.56"],
            ["face.w100menlo16", "ok", "144.4922"],
            ["face.w900su40", "ok", "392.8677"],
            ["face.boldSu16", "ok", "98.5468"],
            ["face.boldItalSu24", "ok", "196.8721"],
            ["face.scSu16", "ok", "130.625"],
            ["face.w500su16", "ok", "73.5992"],
            ["face.boldSimSun16", "ok", "130.336"],
            ["face.boldZapf12", "ok", "117.09"],
            ["genq.monoBold100", "ok", "814.6"],
            ["genq.serifItal100", "ok", "790.7"],
            ["genq.cursive100", "ok", "790.7"],
            ["unq.serifBold100", "ok", "853.3"],
            ["unq.monoSc100", "ok", "686.3385"],
            ["sc.spot8", "ok", "19.676"],
            ["sc.spot32", "ok", "72.144"],
            ["sc.kern24arial", "ok", "108.0707"],
            ["sc.kern24times", "ok", "85.9798"],
            ["su.w600_18", "ok", "169.7786"],
            ["su.w800_16", "ok", "160.4531"],
            ["su.w900_13", "ok", "117.9718"],
            ["su.bold13", "ok", "112.4401"],
            ["su.w900_145", "ok", "129.9909"],
            ["su.w800_105", "ok", "27.9372"],
            ["face.genBold", "ok", "946.0936"],
            ["face.w600futura100", "ok", "936.4"],
            ["face.boldMenloFix", "ok", "323.0204"]
        ]),
        "knife99: face axis model (Chrome 145 fonts-faces truth)"
    );
}

#[tokio::test]
async fn knife100_sc_composite_lattice() {
    // (刀100) sc×face composites: small-caps on a styled face is its OWN
    // measured table, keyed by the scLattice vkey — numeric weights are
    // distinct lattice keys ('800' ≠ bold for Gill Sans: S800 class ≠ SB
    // class), keywords b/i/z fold only from keyword tokens, and 400 always
    // collapses to plain S. Composite kern is a FULL CH95×CH95 capture per
    // representative (krS containment: absent pair = measured zero — Arial#SB
    // 'Ta' kerns 0 while R 'Ta' = −11.08; Gill#SB 'la' = −0.99 lives only on
    // the composite), empty-kern faces carry kr0 (Gill S800: all-zero kern).
    // Face tables are 4dp@100 rows (the k99 3dp S rows were the last holdout
    // — Menlo sc was +0.0062/21chars from same-sign rounding). Quoted-generic
    // sc faces quantize like their kin (q: scspot attached at 刀100 d).
    // Rows verified against live Chrome 145 within 0.0051px; the two 95-char
    // Gill S600 spot rows sit at the 4dp term-sum stack-up floor (≤0.05px —
    // per-term diffs are sub-threshold, they accumulate over ~90 terms).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r##"
        const L = [];
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        L.push(['lat.s800gill', W('800 small-caps 12px Gill Sans', 'lavatory fi')]);
        L.push(['lat.s600gill', W('600 small-caps 12px Gill Sans', 'lavatory')]);
        L.push(['lat.boldGill', W('bold small-caps 12px Gill Sans', 'lavatory')]);
        L.push(['lat.s100arial', W('100 small-caps 16px Arial', 'lavatory fi')]);
        L.push(['lat.zapfBoldSc', W('bold small-caps 100px Zapfino', 'Hamburgefonstiv AV Ta')]);
        L.push(['men.sc100', W('small-caps 100px Menlo', 'Hamburgefonstiv AV Ta')]);
        L.push(['men.isc100', W('italic small-caps 100px Menlo', 'Hamburgefonstiv AV Ta')]);
        L.push(['q.sansScFi16', W('small-caps 16px "sans-serif"', 'lavatory fi')]);
        L.push(['q.sansBoldSc16', W('bold small-caps 16px "sans-serif"', 'lavatory fi')]);
        L.push(['g.serifSc600', W('600 small-caps 24px serif', 'Hamburgefonstiv')]);
        L.push(['kern.sbArialTa', W('bold small-caps 100px Arial', 'Ta')]);
        L.push(['kern.gill800Ta', W('800 small-caps 100px Gill Sans', 'Ta')]);
        L.push(['excl.gill600', W('600 small-caps 100px Gill Sans', 'RS ST wx')]);
        L.push(['spot.gill95_16', W('600 small-caps 16px Gill Sans', ' !"#$%&\'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~')]);
        L.push(['spot.gill95_8', W('600 small-caps 8px Gill Sans', ' !"#$%&\'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~')]);
        return JSON.stringify(L);
    "##).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["lat.s800gill", 68.3481],
            ["lat.s600gill", 41.21],
            ["lat.boldGill", 46.6201],
            ["lat.s100arial", 70.0593],
            ["lat.zapfBoldSc", 1860.4],
            ["men.sc100", 993.3846],
            ["men.isc100", 993.3846],
            ["q.sansScFi16", 72.197],
            ["q.sansBoldSc16", 73.814],
            ["g.serifSc600", 177.39],
            ["kern.sbArialTa", 111.6358],
            ["kern.gill800Ta", 139.1406],
            ["excl.gill600", 418.6035],
            ["spot.gill95_16", 786.2426],
            ["spot.gill95_8", 401.9498]
        ]),
        "knife100: sc composite lattice (Chrome 145 fonts-faces truth)"
    );
}

#[tokio::test]
async fn knife101_systemui_kern_repin() {
    // (刀101) system-ui truth re-pin, four layers. kb: the entry half-grid
    // anchors were DEAD CODE (krbM bare dicts, B.length undefined) — now
    // inner Maps with the same sparse lookup kernFace runs ('./' 0.6734 at
    // 18.5, fractional 18.75 interpolates; '——' decay legs exact to 79.5).
    // kb re-gate 0.002 + v100 FLOATS (4dp @100 px, not int-truncated).
    // suSpot fills: 8 faces × 24 standard sizes + the as gate 0.005→0.002
    // (the census arbiter is 0.003; rows@25 residues lived in the dropped
    // band). SI/SZ double-synthesis (oblique×small-caps) quantizes per-glyph
    // advances CHAOTICALLY at every unpinned size across the WHOLE axis
    // (1.5px@13.5 … 0.5px@90 on 'hello' — the believed 43..63 window was
    // just where integers were unpinned) → full 0.5-grid pin 5..99.5
    // (×2 faces; warm-captured, 24.4k overlapping cells reproduce exactly).
    // Italic small-caps KERN also curves above 32.5 where every upright face
    // is exactly linear (s1 swept 150k cells: 0 flagged) → SI/SZ curvature
    // bands 33..99.5 (691 pairs, worst 'ly' 0.0254@64), kernFace cutoff
    // 81→100. Chrome-side gotchas this knife: sliced-string ToString is
    // CORRUPT in Chrome 145 too (v1 truth rebuilt slice-free); italic
    // fallback resolution is cold/warm racy — the warm key is U+2017 in
    // face R at 21px (measured once, loads the fallback process-wide; cold
    // is sticky per face×char; the engine bakes WARM = the steady state).
    // Rows verified against live Chrome 145 within 0.0004px (most exact);
    // expectations pin the model, not Chrome's float.
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt
        .evaluate(
            r#"
        const L = [];
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        const R = (font, a, b) => +(W(font, a + b) - W(font, a) - W(font, b)).toFixed(4);
        L.push(['kb.dotslash185', R('18.5px system-ui', '.', '/')]);
        L.push(['kb.dotslash1875', R('18.75px system-ui', '.', '/')]);
        L.push(['kb.dotslash19', R('19px system-ui', '.', '/')]);
        L.push(['kb.emdash50', R('50px system-ui', '—', '—')]);
        L.push(['kb.emdash795', R('79.5px system-ui', '—', '—')]);
        L.push(['band.stSI64', R('italic small-caps 64px system-ui', 's', 't')]);
        L.push(['band.lySI64', R('italic small-caps 64px system-ui', 'l', 'y')]);
        L.push(['band.stSZ64', R('bold italic small-caps 64px system-ui', 's', 't')]);
        L.push(['band.lySZ80', R('bold italic small-caps 80px system-ui', 'l', 'y')]);
        L.push(['band.stSI435', R('italic small-caps 43.5px system-ui', 's', 't')]);
        L.push(['lin.stI100', R('italic 100px system-ui', 's', 't')]);
        L.push(['as.w800_25', W('800 25px system-ui', ' !1')]);
        L.push(['as.w900_25', W('900 25px system-ui', '$7')]);
        L.push(['as.M_25', W('500 25px system-ui', ' $')]);
        L.push(['as.S800_72', W('800 small-caps 72px system-ui', 'lavatory')]);
        L.push(['as.SB_72', W('bold small-caps 72px system-ui', 'Hamburgefonstiv')]);
        L.push(['as.SI43', W('italic small-caps 43px system-ui', 'hello small caps')]);
        L.push(['as.SI575', W('italic small-caps 57.5px system-ui', 'hello')]);
        L.push(['as.SZ61', W('bold italic small-caps 61px system-ui', 'small caps')]);
        L.push(['lad.I40', W('italic 40px system-ui', 'Hamburgefonstiv')]);
        L.push(['lad.Z55', W('bold italic 55px system-ui', 'Hamburgefonstiv')]);
        L.push(['lad.SZ23', W('bold italic small-caps 23px system-ui', 'Hamburgefonstiv')]);
        return JSON.stringify(L);
    "#,
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["kb.dotslash185", 0.6734],
            ["kb.dotslash1875", 0.674],
            ["kb.dotslash19", 0.6747],
            ["kb.emdash50", -0.3419],
            // 刀102: advance band now applies inside sum() at 79.5, shifting the
            // toFixed(4) rounding of the derived pair value; kern anchors unchanged.
            ["kb.emdash795", -0.0085],
            ["band.stSI64", -1.3184],
            ["band.lySI64", -5.7129],
            ["band.stSZ64", -0.8789],
            ["band.lySZ80", -6.6992],
            ["band.stSI435", -0.8789],
            ["lin.stI100", -0.9766],
            ["as.w800_25", 26.361],
            ["as.w900_25", 32.1045],
            ["as.M_25", 21.1425],
            ["as.S800_72", 322.078],
            ["as.SB_72", 619.1191],
            ["as.SI43", 283.0598],
            ["as.SI575", 120.2196],
            ["as.SZ61", 270.1098],
            ["lad.I40", 296.8165],
            ["lad.Z55", 446.1906],
            ["lad.SZ23", 164.5344]
        ]),
        "knife101: system-ui kern re-pin + SI/SZ grid (Chrome 145 fonts truth)"
    );
}

#[test]
fn knife102_systemui_shared_bands() {
    // (k102) 直立面共享梯包络: at unpinned sizes every upright face deviates
    // from the ladder identically per glyph in two windows — bump 32.5–39.5
    // (f peak 1.0 @36.5) and arch 64.5–99.5 (peak @80.5, max 0.1495px) — a
    // rank-1 product delta(g,s) = a[g]·f(s), residual 0.0005 = capture floor.
    // Emitted as SBAND + sb flag on the base entry and 19 upright faces (SI/SZ
    // stay OFF: their 0.5-grid pin covers everything). fixA: italic turns the
    // U+2016 amplitude OFF (I/Z run the bare ladder there — sba REPLACE rows).
    // M-class cliff tails: the deep-punctuation glyph switch extends 28.5–31.5
    // (not just ≤27.5), 84 tail rows merged into suSpot. Region B wobble:
    // 17.5–27.5 halves pinned past the 0.002 as gate; under-gate cells keep
    // the ladder (regB.w800_255H: chrome 19.2028, ladder 19.2043 — by gate).
    // as-layer quarter interpolation: on exact-pin miss, interpolate between
    // adjacent half anchors; excess-over-proportional > 0.07 = cliff, hold
    // floor flat (q.SZ_cliff6625 = the old 挂账 cell, chrome 58.9382 — cliff-
    // interior quarter not expressible from two anchors). 刀116: SI/SZ scq
    // glyphs now mask cross-level as brackets and extrapolate along the
    // level (66.0 anchor + 65.5 slope); miss 0.0026 vs the old cliff hold
    // 0.0049 — idx158@66.25 cleared, expectation re-pinned to the new model.
    // Rows verified against live Chrome 145 within the 0.002 gate (most
    // 0.0000–0.0009); expectations pin the model, not Chrome's float.
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt
        .evaluate(
            r#"
        const L = [];
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        L.push(['band.R365', W('36.5px system-ui', ' !1')]);
        L.push(['band.R805', W('80.5px system-ui', '$7')]);
        L.push(['band.R7525', W('75.25px system-ui', 'Hamburgefonstiv')]);
        L.push(['band.SB365', W('bold small-caps 36.5px system-ui', ' !1')]);
        L.push(['band.w300_805', W('300 80.5px system-ui', ' $')]);
        L.push(['edge.R405', W('40.5px system-ui', ' !1')]);
        L.push(['regB.w800_255H', W('800 25.5px system-ui', 'H')]);
        L.push(['regB.S200_225', W('200 small-caps 22.5px system-ui', 'lavatory')]);
        L.push(['tail.M_295', W('500 29.5px system-ui', 'Hamburgefonstiv')]);
        L.push(['tail.B_315', W('bold 31.5px system-ui', 'Hamburgefonstiv')]);
        L.push(['tail.w900_305', W('900 30.5px system-ui', 'Hamburgefonstiv')]);
        L.push(['fixA.I_365', W('italic 36.5px system-ui', '‖')]);
        L.push(['fixA.Z_805', W('bold italic 80.5px system-ui', '‖')]);
        L.push(['fixA.B_365', W('bold 36.5px system-ui', '‖')]);
        L.push(['q.SI_3425_H', W('italic small-caps 34.25px system-ui', 'H')]);
        L.push(['q.SI_8525_H', W('italic small-caps 85.25px system-ui', 'H')]);
        L.push(['q.SZ_3325_H', W('bold italic small-caps 33.25px system-ui', 'H')]);
        L.push(['q.SZ_cliff6625', W('bold italic small-caps 66.25px system-ui', 'ß')]);
        L.push(['u2028', W('21px system-ui', '\u2028')]);
        return JSON.stringify(L);
    "#,
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["band.R365", 34.5083],
            ["band.R805", 92.685],
            ["band.R7525", 541.6726],
            ["band.SB365", 36.2015],
            ["band.w300_805", 64.7381],
            ["edge.R405", 38.1715],
            ["regB.w800_255H", 19.2044],
            ["regB.S200_225", 86.3252],
            ["tail.M_295", 224.9027],
            ["tail.B_315", 255.0496],
            ["tail.w900_305", 261.5944],
            ["fixA.I_365", 18.25],
            ["fixA.Z_805", 40.25],
            ["fixA.B_365", 16.0182],
            ["q.SI_3425_H", 24.4029],
            ["q.SI_8525_H", 59.7749],
            ["q.SZ_3325_H", 24.8147],
            ["q.SZ_cliff6625", 58.9356],
            ["u2028", 4.9321]
        ]),
        "knife102: upright shared bump/arch bands + region B + cliff tails + quarters (Chrome 145 fonts truth)"
    );
}

#[test]
fn knife103_triple_small_size_spots() {
    // (k103) 三元组小尺寸非线性: su triple values are NOT linear-from-@100
    // below ~28px — GPOS pixel-snap families: threshold turn-on (S ⁝“a: 0 to
    // 16, fires from 17), non-monotonic peak-then-dip (900 †9…), sign-flip
    // (B ⁏&™ negative small, positive large), reverse-decay (B 7:Ì killed
    // @100 but real 5..24). 102,497/150,863 su keys deviate > 0.002 at some
    // integer size 5..28 (worst 2.11px @16, face 900 ♦^,). triSpot truth:
    // per-key integer anchor rows [px@100*100 at 5..28], emitted as k3s Maps
    // (TSP/TSPM, R on the base entry + 21 faces by entry tag); the sum()
    // kernel consults k3s BEFORE the kill path — spot beats mod/kill/inherit
    // inside [5,28], linear interp between integer anchors (half sizes
    // verified worst 0.0008 on the worst keys — no half grid needed, unlike
    // k101's kern bands), <5 clamps to v(5), >=28 falls back to the resolved
    // linear value (curves converge by ~28). Round 2 added the R-ext
    // INHERITED keys (163–637/face; face mods alone don't cover the inherit
    // universe). Residue after the layer: c3v2 37→5 bad @0.0015 (2 @0.002),
    // worst 0.0028 — all flank-context deltas (flanked 'nv‥zt' vs standalone
    // window), the c5 mid-layer's small-size extension = 刀104 candidate.
    // knife98 tri.suExotic16 re-pinned 24.4092 → 24.643 (the 0.23 model pin
    // retired with its backlog). Rows below verified against live Chrome
    // (worst 0.0018 = R@27.5 interp cell; string widths carry three glyphs'
    // 4dp accumulation).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt
        .evaluate(
            r#"
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        x.font = '21px system-ui'; x.measureText('‗').width;
        const W = (font, text) => { x.font = font; return +x.measureText(text).width.toFixed(4); };
        const L = [];
        L.push(['spot.R_5', W('5px system-ui', '⁏…“')]);
        L.push(['spot.R_16', W('16px system-ui', '⁏…“')]);
        L.push(['spot.R_275', W('27.5px system-ui', '⁏…“')]);
        L.push(['spot.R_28', W('28px system-ui', '⁏…“')]);
        L.push(['spot.R_285', W('28.5px system-ui', '⁏…“')]);
        L.push(['spot.R_32', W('32px system-ui', '⁏…“')]);
        L.push(['spot.R_4clamp', W('4px system-ui', '⁏…“')]);
        L.push(['spot.900_16', W('900 16px system-ui', '♦^,')]);
        L.push(['spot.B_inh_16', W('bold 16px system-ui', '♥*T')]);
        L.push(['spot.S_turn_12', W('small-caps 12px system-ui', '⁝“a')]);
        L.push(['spot.S_turn_16', W('small-caps 16px system-ui', '⁝“a')]);
        L.push(['spot.S_turn_21', W('small-caps 21px system-ui', '⁝“a')]);
        L.push(['spot.B_revive_16', W('bold 16px system-ui', '7:Ì')]);
        L.push(['spot.B_flip_16', W('bold 16px system-ui', '⁏&™')]);
        L.push(['spot.M_half_165', W('500 16.5px system-ui', '♦^.')]);
        L.push(['spot.100_8', W('100 8px system-ui', '⁝×T')]);
        return JSON.stringify(L);
    "#,
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["spot.R_5", 7.4987],
            ["spot.R_16", 22.0886],
            ["spot.R_275", 31.5618],
            ["spot.R_28", 32.0237],
            ["spot.R_285", 32.5892],
            ["spot.R_32", 36.5367],
            ["spot.R_4clamp", 5.999],
            ["spot.900_16", 29.336],
            ["spot.B_inh_16", 33.3516],
            ["spot.S_turn_12", 15.8458],
            ["spot.S_turn_16", 20.5027],
            ["spot.S_turn_21", 25.8536],
            ["spot.B_revive_16", 19.2345],
            ["spot.B_flip_16", 30.7577],
            ["spot.M_half_165", 27.4486],
            ["spot.100_8", 12.4558]
        ]),
        "knife103: triple small-size spot anchors (Chrome 145 fonts truth)"
    );
}

#[tokio::test]
async fn knife117_font_shorthand_parse_model() {
    // (刀117) ctx.font full parse/serial/render model (Chrome 145 truth):
    // invalid assignment IGNORED — getter and metrics keep the previous
    // value ('bold 700 …' double-weight and 'zzz' keep '16px Arial').
    // Canonical serial `italic bold small-caps <px>px <families>`; oblique
    // bare→italic, oblique-with-angle→style dropped (face reads regular),
    // lighter→100, 700→bold, 400 dropped. The keyword map fires only on the
    // EXACT float — 700.5 serializes its truncated int '700', never 'bold'
    // ('700.5 0.5px Arial' → '700 0.5px Arial'). Computed size is stored
    // float32: serial = toPrecision(6) of Math.fround(px) (6.7199949px →
    // '6.72px' because f32 snaps past the 6-sig half; '1e-7px' keeps the
    // exponential form verbatim '1.00000e-7px'; decimal branch trims
    // '100.000'→'100'; >10000 clamps to 10000). The RENDER size is a second,
    // independent quantization: round to 6 decimals then truncate the
    // decimal string to 2 places — 6.7199990 renders 6.71 (7th digit 0, no
    // carry) while 6.7199999 renders 6.72 (carry); 9999.999 serializes
    // '10000px' but renders 9999.99; 0.29/0.58 render at declared (string
    // trunc survives the float traps); 0.145→0.14; 0.014/0.015 plateau at
    // 0.01; 0.009→0. Face selection: only a NUMERIC weight owns a lattice
    // key when the family captured that face (刀100 never-fold); a MISSING
    // numeric ≥600 key folds to the class letters with the italic honored
    // ('700 italic' Avenir Next renders Z — the old fallback dropped the
    // italic and read B; numeric 700 roman == B because bold IS the 700
    // face). Documented non-parse gaps (metrics line): FM table lacks
    // math / Times / STIX Two Math faces, Avenir Next Demibold (600) and
    // the 600-900 italic faces; small-caps synthesis curve diverges <4px.
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const L = [];
        const S = (f) => { x.font = 'bold 20px Arial'; x.font = f; return x.font; };
        L.push(['ser.fr32w', S('6.7199949px Arial')]);
        L.push(['ser.expo', S('1e-7px Arial')]);
        L.push(['ser.clamp', S('12345678px Arial')]);
        L.push(['ser.sixsig', S('1234.5678px Arial')]);
        L.push(['ser.rnd6', S('99.99999px Arial')]);
        L.push(['ser.fround1', S('1.0000005px Arial')]);
        L.push(['ser.frac33', S('0.333333px Arial')]);
        L.push(['ser.lighter', S('lighter 16px Arial')]);
        L.push(['ser.obl', S('oblique 16px Arial')]);
        L.push(['ser.oblang', S('oblique 20deg 16px Arial')]);
        L.push(['ser.canon', S('bold italic 16px Arial')]);
        L.push(['ser.n700', S('700 16px Arial')]);
        L.push(['ser.n400', S('400 16px Arial')]);
        L.push(['ser.wfrac', S('700.5 0.5px Arial')]);
        L.push(['ser.w450', S('450.5 16px Arial')]);
        L.push(['ser.qgen', S('16px "sans-serif"')]);
        L.push(['ser.ex3', S('3ex Arial')]);
        L.push(['ser.exmono', S('3ex monospace')]);
        L.push(['ser.ch3', S('3ch Arial')]);
        L.push(['ser.rem', S('2rem Arial')]);
        L.push(['ser.em15', S('1.5em Arial')]);
        L.push(['ser.xxxl', S('xxx-large Arial')]);
        L.push(['ser.larger', S('larger Arial')]);
        L.push(['ser.multidot', S('1.5.5px Arial')]);
        x.font = '16px Arial'; x.font = 'bold 700 16px Arial';
        L.push(['ser.reject', x.font]);
        x.font = '16px Arial'; x.font = 'zzz'; x.font = 'qqq';
        L.push(['ser.reject2', x.font]);
        const W4 = (f) => { x.font = f; return +x.measureText('Hamburgefonstiv').width.toFixed(4); };
        L.push(['w.trunc6', W4('6.7199990px Arial')]);
        L.push(['w.carry6', W4('6.7199999px Arial')]);
        L.push(['w.snr', W4('9999.999px Arial')]);
        L.push(['w.clamp10k', W4('10000px Arial')]);
        L.push(['w.trap029', W4('0.29px Arial')]);
        L.push(['w.trap058', W4('0.58px Arial')]);
        L.push(['w.trunc145', W4('0.145px Arial')]);
        L.push(['w.q1', W4('1q Arial')]);
        L.push(['w.px33', W4('0.333333px Arial')]);
        L.push(['w.plateau014', W4('0.014px Arial')]);
        L.push(['w.zero009', W4('0.009px Arial')]);
        L.push(['w.rem20', W4('2rem Arial')]);
        L.push(['w.ex15', W4('1.5ex Arial')]);
        const WZ = (f, t) => { x.font = f; return +x.measureText(t).width.toFixed(4); };
        const T = 'nv⁝‹Tzt';
        L.push(['fc.Z', WZ('bold italic 100px Avenir Next', T)]);
        L.push(['fc.w700i', WZ('700 italic 100px Avenir Next', T)]);
        L.push(['fc.B', WZ('bold 100px Avenir Next', T)]);
        L.push(['fc.w700', WZ('700 100px Avenir Next', T)]);
        L.push(['fc.sc800', WZ('small-caps 800 100px Arial', T)]);
        L.push(['fc.scbold', WZ('small-caps bold 100px Arial', T)]);
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["ser.fr32w", "6.72px Arial"],
            ["ser.expo", "1.00000e-7px Arial"],
            ["ser.clamp", "10000px Arial"],
            ["ser.sixsig", "1234.57px Arial"],
            ["ser.rnd6", "100px Arial"],
            ["ser.fround1", "1px Arial"],
            ["ser.frac33", "0.333333px Arial"],
            ["ser.lighter", "100 16px Arial"],
            ["ser.obl", "italic 16px Arial"],
            ["ser.oblang", "16px Arial"],
            ["ser.canon", "italic bold 16px Arial"],
            ["ser.n700", "bold 16px Arial"],
            ["ser.n400", "16px Arial"],
            ["ser.wfrac", "700 0.5px Arial"],
            ["ser.w450", "450 16px Arial"],
            ["ser.qgen", "16px \"sans-serif\""],
            ["ser.ex3", "13.4619px Arial"],
            ["ser.exmono", "10.9378px monospace"],
            ["ser.ch3", "15px Arial"],
            ["ser.rem", "20px Arial"],
            ["ser.em15", "15px Arial"],
            ["ser.xxxl", "48px Arial"],
            ["ser.larger", "12px Arial"],
            ["ser.multidot", "1 0.5px Arial"],
            ["ser.reject", "16px Arial"],
            ["ser.reject2", "16px Arial"],
            ["w.trunc6", 50.7214],
            ["w.carry6", 50.797],
            ["w.snr", 75590.7244],
            ["w.clamp10k", 75590.8],
            ["w.trap029", 2.1921],
            ["w.trap058", 4.3843],
            ["w.trunc145", 1.0583],
            ["w.q1", 7.1055],
            ["w.px33", 2.4945],
            ["w.plateau014", 0.0756],
            ["w.zero009", 0],
            ["w.rem20", 151.1816],
            ["w.ex15", 50.8726],
            ["fc.Z", 316.7008],
            ["fc.w700i", 316.7008],
            ["fc.B", 322.1008],
            ["fc.w700", 322.1008],
            ["fc.sc800", 310.4445],
            ["fc.scbold", 310.4445]
        ]),
        "knife117: font shorthand parse/serial/render model (Chrome 145 truth)"
    );
}

#[tokio::test]
// The U+200B/C/D codepoints in the probe strings are the payload (刀118
// Cf-insert shaping truth), not stray bytes.
#[allow(clippy::invisible_characters)]
async fn knife118_cf_insert_kill_truth() {
    // (刀118) transparent-Cf insert kills, CFT Chrome 145 truth (k118a..m
    // censuses, 323,385-cell universe). Chrome elides {200C,200D,2060-2064,
    // 2066,2068,2069} before shaping: GPOS pair kern PASSES through them,
    // but the contextual kern/liga layer dies per (font, window, member
    // class) — LIGAP kills the isolated pair by REPLACING the kern term
    // (r=0 = pure drop; optical bands free), and a triple window with a
    // CFKT entry governs its junctions via its measured total delta (mask
    // 1=ZWNJ 2=WJ-class 4=ZWJ; junction precedence Z>J>W). An isolated
    // pair kill NEVER extends into a flanking TMAP window (zapfino pp:
    // 'p·C·p' kills to 110 but 'p·C·pp' stays 180.75) — and a kern-class
    // font passes the same Cf untouched (TNR ff·200C). Opaque CF_ZW
    // members (200B…) stay in-stream as junction breakers (arial ff·200B
    // = f + f, no ligature kern). ZWJ rides ONLY where the mask carries
    // bit 4 (kr=1 shared table is ZWNJ-only: -apple-system ff/fi die on
    // 200C, not 200D; AT ffi j2 dies on 200C/2060, not 200D).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt
        .evaluate(
            r#"
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const L = [];
        const W = (f, t) => { x.font = f; return +x.measureText(t).width.toFixed(3); };
        const AT = '100px "American Typewriter"';
        L.push(['at.fi200C', W(AT, 'f‌i')]);
        L.push(['at.fi2060', W(AT, 'f⁠i')]);
        L.push(['at.fi200D', W(AT, 'f‍i')]);
        L.push(['at.ffi', W(AT, 'ffi')]);
        L.push(['at.ffi.j2z', W(AT, 'ff‌i')]);
        L.push(['at.ffi.j2w', W(AT, 'ff⁠i')]);
        L.push(['at.ffi.j2j', W(AT, 'ff‍i')]);
        L.push(['at.ffi.j1z', W(AT, 'f‌fi')]);
        L.push(['at.dag.j1z', W(AT, '†‌1,')]);
        L.push(['at.dag.j2z', W(AT, '†1‌,')]);
        L.push(['kt.fi2066', W('100px "Kaiti SC"', 'f\u2066i')]);
        L.push(['kt.fi200C', W('100px "Kaiti SC"', 'f‌i')]);
        L.push(['av.fi200C', W('100px Avenir', 'f‌i')]);
        L.push(['gv.fl200C', W('100px Geneva', 'f‌l')]);
        L.push(['gs.fi200C', W('100px "Gill Sans"', 'f‌i')]);
        L.push(['tnr.ff200C', W('100px "Times New Roman"', 'f‌f')]);
        L.push(['ar.ff200B', W('100px Arial', 'f​f')]);
        L.push(['ar.ff', W('100px Arial', 'ff')]);
        L.push(['aps.ff200C', W('100px -apple-system', 'f‌f')]);
        L.push(['aps.fi200C', W('100px -apple-system', 'f‌i')]);
        L.push(['aps.ff200D', W('100px -apple-system', 'f‍f')]);
        L.push(['zp.pp200C', W('100px Zapfino', 'p‌p')]);
        L.push(['zp.pp', W('100px Zapfino', 'pp')]);
        L.push(['zp.pp200C.p', W('100px Zapfino', 'p‌pp')]);
        return JSON.stringify(L);
    "#,
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["at.fi200C", 68.4],
            ["at.fi2060", 68.4],
            ["at.fi200D", 60],
            ["at.ffi", 94.6],
            ["at.ffi.j2z", 104.1],
            ["at.ffi.j2w", 104.1],
            ["at.ffi.j2j", 94.6],
            ["at.ffi.j1z", 94.6],
            ["at.dag.j1z", 150.1],
            ["at.dag.j2z", 154.7],
            ["kt.fi2066", 61.5],
            ["kt.fi200C", 61.5],
            ["av.fi200C", 53.6],
            ["gv.fl200C", 62.5],
            ["gs.fi200C", 46.924],
            ["tnr.ff200C", 64.795],
            ["ar.ff200B", 55.566],
            ["ar.ff", 53.76],
            ["aps.ff200C", 74.6],
            ["aps.fi200C", 62.9],
            ["aps.ff200D", 68.6],
            ["zp.pp200C", 110],
            ["zp.pp", 125.75],
            ["zp.pp200C.p", 180.75]
        ]),
        "knife118: Cf-insert kill truth (Chrome 145 CFT)"
    );
}

#[tokio::test]
async fn knife119_fm_fill_truth() {
    // (刀119) 83-family FM 缺脸填平, CFT Chrome 145 truth (k119b..p
    // censuses, 983 legs engine==truth ±0.000). The fmfill truth
    // (fonts-fmfill-truth-145.json, split file per the 100MB cap) merges
    // as a post-pass so kcanon stays append-only; ext rows MUST join
    // d['ext']['adv'] BEFORE build_faces — near-R faces (I/M/Z) derive
    // their x base + xp overrides from it (pre-fix every non-override
    // ext slot read __absent__). `math` unquoted resolves the STIX Two
    // Math face; quoted "math" falls to the default generic. The 8
    // filed base_tri cells were pair-universe holes, not FM 缺脸:
    // zapfino ¡J/¡j/¡p/¿j/¿p pair kerns + baskerville fl. Residual
    // (deferred, not this knife): kernFaces gap for B/I/M/Z faces of
    // new families (81 legs) and contextual string kern/tri (15 legs).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt.evaluate(r#"
        const c = document.createElement('canvas');
        const x = c.getContext('2d');
        const L = [];
        const W = (f, t) => { x.font = f; return +x.measureText(t).width.toFixed(3); };
        const V = (f) => { x.font = f; const m = x.measureText('Hg'); return [m.fontBoundingBoxAscent, m.fontBoundingBoxDescent]; };
        const AS = 'AVWaTofiery.,72µ‑♥';
        const MM = '100px "Malayalam MN"';
        L.push(['math.kw', W('100px math', AS)]);
        L.push(['math.q', W('100px "math"', AS)]);
        L.push(['math.kw.itimes', W('100px math', 'x⁢y')]);
        L.push(['math.kw.plus.itimes', W('100px math', 'x+⁢y')]);
        L.push(['math.face', W('100px "STIX Two Math"', AS)]);
        L.push(['stix.ftm', W('100px "STIX Two Math"', 'f™')]);
        L.push(['stix.VA', W('100px "STIX Two Math"', 'VA')]);
        L.push(['times', W('100px Times', AS)]);
        L.push(['times.q', W('100px "Times"', AS)]);
        L.push(['mal.adv', W(MM, AS)]);
        L.push(['mal.ital', W('italic ' + MM, AS)]);
        L.push(['mal.w500', W('500 ' + MM, AS)]);
        L.push(['mal.bold', W('bold ' + MM, AS)]);
        const v16 = V('16px "Malayalam MN"');
        L.push(['mal.v16a', +v16[0]]);
        L.push(['mal.v16d', +v16[1]]);
        const v100 = V(MM);
        L.push(['mal.v100a', +v100[0]]);
        L.push(['mal.v100d', +v100[1]]);
        L.push(['kan.fi', W('100px "Kannada MN"', 'fi')]);
        L.push(['kan.adv', W('100px "Kannada MN"', AS)]);
        L.push(['wd.adv', W('100px Wingdings', AS)]);
        L.push(['zp.invp', W('100px Zapfino', '¡p')]);
        L.push(['zp.qmp', W('100px Zapfino', '¿p')]);
        L.push(['zp.invJ', W('100px Zapfino', '¡J')]);
        L.push(['zp.tri', W('100px Zapfino', '¡pf')]);
        L.push(['bas.fl', W('100px Baskerville', 'fl')]);
        L.push(['bas.ffl', W('100px Baskerville', 'ffl')]);
        L.push(['bas.flw', W('100px Baskerville', 'flw')]);
        return JSON.stringify(L);
    "#).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["math.kw", 863.4],
            ["math.q", 958.2],
            ["math.kw.itimes", 96.2],
            ["math.kw.plus.itimes", 168.2],
            ["math.face", 863.4],
            ["stix.ftm", 129.8],
            ["stix.VA", 129.3],
            ["times", 843.555],
            ["times.q", 843.555],
            ["mal.adv", 915.668],
            ["mal.ital", 915.668],
            ["mal.w500", 917.368],
            ["mal.bold", 958.005],
            ["mal.v16a", 16],
            ["mal.v16d", 4],
            ["mal.v100a", 100],
            ["mal.v100d", 25],
            ["kan.fi", 66.016],
            ["kan.adv", 916.693],
            ["wd.adv", 1698.144],
            ["zp.invp", 117.5],
            ["zp.qmp", 127.5],
            ["zp.invJ", 161.25],
            ["zp.tri", 163.25],
            ["bas.fl", 51.024],
            ["bas.ffl", 78.124],
            ["bas.flw", 118.749]
        ]),
        "knife119: FM 83-family fill truth (Chrome 145 CFT)"
    );
}

#[tokio::test]
async fn knife120_acc_backing_no_page_collision() {
    // (刀120) The __acc own-slot backing (`this._onloadend` etc.) collided
    // with page wrapper conventions: baxia (g.alicdn.com/sd/baxia, tmall
    // risk control) saves the original handler as `this._onloadend` before
    // wrapping `onloadend`; our setter then clobbered that saved original
    // with the wrapper itself — dispatch → wrapper → r._onloadend (= the
    // wrapper) → … RangeError, killing the page's whole module system
    // (tmall sell.publish.tmall.com category.htm, live 2026-11-02, the
    // stack entirely inside the eval'd baxiaCommon.js blob). Chrome keeps
    // the IDL attribute backing in a native slot JS cannot name, so
    // `x._onloadend` never returns the stored handler. Fix: __acc backs
    // its accessors with the 刀84 hidden-slot channel (__diting_liftSlots
    // WeakMap); __accRo keeps the own-prop face (engine-side __def
    // writers, no page wrapper family saves originals under ro names).
    let mut rt = setup_runtime("<html><body></body></html>");
    let log = rt
        .evaluate(
            r#"
        const L = [];
        // 1. Chrome parity: the page never sees the backing by name.
        const x = new XMLHttpRequest();
        x.onloadend = function orig() {};
        L.push(['xhr.privHidden', x._onloadend === undefined]);
        L.push(['xhr.propIsFn', typeof x.onloadend === 'function']);
        // 2. The baxia wrap pattern must terminate: save → wrap → dispatch.
        let calls = 0;
        const H = () => { calls++; };
        x.onloadend = H;
        x._onloadend = x.onloadend;
        x.onloadend = function W(e) { if (x._onloadend) x._onloadend(e); };
        x.dispatchEvent(new ProgressEvent('loadend'));
        L.push(['xhr.wrapHOnce', calls === 1]);
        // 3. Same face on the other __acc classes a page wraps this way.
        const fr = new FileReader();
        fr.onload = function fl() {};
        L.push(['fr.privHidden', fr._onload === undefined]);
        let bcOk = null;
        if (typeof BroadcastChannel !== 'undefined') {
          const bc = new BroadcastChannel('k120');
          bc.onmessage = function bm() {};
          bcOk = bc._onmessage === undefined;
          bc.close();
        }
        L.push(['bc.privHidden', bcOk === null || bcOk === true]);
        // 4. Round-trip through the setter still works (dispatch leg).
        let got = null;
        x.onloadend = function G(e) { got = e && e.type; };
        x.dispatchEvent(new ProgressEvent('loadend'));
        L.push(['xhr.dispatchType', got === 'loadend']);
        return JSON.stringify(L);
    "#,
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(log.as_str().unwrap()).unwrap(),
        serde_json::json!([
            ["xhr.privHidden", true],
            ["xhr.propIsFn", true],
            ["xhr.wrapHOnce", true],
            ["fr.privHidden", true],
            ["bc.privHidden", true],
            ["xhr.dispatchType", true]
        ]),
        "knife120: __acc backing unreachable by page names; baxia wrap terminates"
    );
}
