//! getComputedStyle 读面 + presentational 反射 + 简写展开
use super::super::*;
use super::common::setup_runtime;

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
    // transform-origin: default "50% 50% 0px" (Chrome's string from the
    // issue probe), declared "left top" normalizes to 0% 0%, unset
    // logical margin answers 0px like Chrome.
    assert_eq!(
        out.as_str().unwrap(),
        "10px|96px|4px|8px|50% 50% 0px|0% 0% 0px|0px",
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
    let mut rt = setup_runtime(r#"<html><body><div id="d">x</div></body></html>"#);
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
    // "none", webkitMaskImage initial → "none". webkitTextFillColor needs
    // color-alias modeling and stays '' (follow-up noted in the issue).
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
<p id="p" style="-webkit-line-clamp:3">x</p>
<p id="r">x</p>
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
        serde_json::json!(""),
        "webkitTextFillColor still awaits color-alias modeling (Chrome: rgb(0, 0, 0)) — flip when that lands"
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
