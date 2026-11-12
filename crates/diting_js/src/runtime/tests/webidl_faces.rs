//! 通用 WebIDL 面拍：members/stack/surface snapshot/svg/xpath
use super::super::*;
use super::common::setup_runtime;

/// `svg.viewBox` must reflect as an animated rect on the fit-to-viewbox
/// SVG elements — the archify viewer's export path reads
/// `svg.viewBox.baseVal.width` directly, so an undefined `viewBox`
/// crashes one property later (found by the artifact viewer smoke probe).
#[test]
fn svg_viewbox_reflects_animated_rect() {
    let mut rt = setup_runtime(
        r#"<svg id="a" viewBox="0 0 880 588"></svg><svg id="b"></svg><div id="h"></div>"#,
    );
    let w = rt
        .evaluate("document.getElementById('a').viewBox.baseVal.width")
        .unwrap();
    assert_eq!(w.as_f64().unwrap(), 880.0);
    let h = rt
        .evaluate("document.getElementById('a').viewBox.animVal.height")
        .unwrap();
    assert_eq!(h.as_f64().unwrap(), 588.0);

    // Missing attribute: still an animated rect, all zeros (Chrome shape).
    let absent = rt
            .evaluate("var vb = document.getElementById('b').viewBox.baseVal; vb.x + ',' + vb.y + ',' + vb.width + ',' + vb.height")
            .unwrap();
    assert_eq!(absent, serde_json::json!("0,0,0,0"));

    // Malformed attribute parses as absent, not as garbage.
    let bad = rt
            .evaluate("document.getElementById('b').setAttribute('viewBox','10 20 100'); document.getElementById('b').viewBox.baseVal.width")
            .unwrap();
    assert_eq!(bad.as_f64().unwrap(), 0.0);
    let good = rt
            .evaluate("document.getElementById('b').setAttribute('viewBox','10 20 100 50'); var v = document.getElementById('b').viewBox.baseVal; v.x + ',' + v.width")
            .unwrap();
    assert_eq!(good, serde_json::json!("10,100"));

    // HTML elements keep no viewBox (Chrome exposes it only on the
    // fit-to-viewbox SVG tags).
    let html = rt
        .evaluate("document.getElementById('h').viewBox === undefined")
        .unwrap();
    assert_eq!(html, serde_json::json!(true));
}

/// #27: the interface prototype chain must read like Chrome's —
/// HTMLDivElement → HTMLElement → Element → Node → EventTarget — with
/// per-interface prototypes that actually isolate patches, and
/// constructor.name per tag (unknown tags report HTMLElement).
#[test]
fn interface_prototype_chain_and_patch_isolation() {
    let mut rt = setup_runtime(
        r#"<body><div id="d"></div><span id="s"></span><x-foo id="x"></x-foo></body>"#,
    );
    let checks: &[(&str, bool)] = &[
        ("document.getElementById('d') instanceof HTMLElement", true),
        ("document.getElementById('d') instanceof Element", true),
        ("document.getElementById('d') instanceof Node", true),
        ("document.getElementById('d') instanceof EventTarget", true),
        (
            "Object.getPrototypeOf(HTMLElement.prototype) === Element.prototype",
            true,
        ),
        (
            "Object.getPrototypeOf(Element.prototype) === Node.prototype",
            true,
        ),
        (
            "Object.getPrototypeOf(Node.prototype) === EventTarget.prototype",
            true,
        ),
        ("document instanceof EventTarget", true),
        ("EventTarget.prototype !== Node.prototype", true),
    ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
    // constructor.name per interface; unknown tag falls back to HTMLElement.
    let names = rt
        .evaluate(
            r#"JSON.stringify([
                    document.getElementById('d').constructor.name,
                    document.getElementById('s').constructor.name,
                    document.getElementById('x').constructor.name,
                ])"#,
        )
        .unwrap();
    assert_eq!(
        names,
        serde_json::json!(r#"["HTMLDivElement","HTMLSpanElement","HTMLElement"]"#)
    );
    // #167: document's concrete interface is HTMLDocument — a real
    // global class (site SDKs reference it by name; Alibaba's AWSC
    // nc-slider init died on `HTMLDocument is not defined` and its
    // punish-page track stayed 0-wide), with document still instanceof
    // Document, and construction illegal like Chrome's.
    let html_doc = rt
            .evaluate(
                r#"(() => {
                    let threw = false;
                    try { new HTMLDocument(); } catch (e) { threw = e instanceof TypeError; }
                    const d = Object.getOwnPropertyDescriptor(window, 'HTMLDocument');
                    return JSON.stringify({
                        t: typeof HTMLDocument,
                        instDoc: document instanceof HTMLDocument,
                        instSuper: document instanceof Document,
                        name: document.constructor.name,
                        chain: Object.getPrototypeOf(Object.getPrototypeOf(document)) === Document.prototype,
                        threw,
                        ne: d ? d.enumerable === false : false,
                        xml: new DOMParser().parseFromString('<a/>', 'text/xml') instanceof HTMLDocument,
                    });
                })()"#,
            )
            .unwrap();
    assert_eq!(
        html_doc,
        serde_json::json!(
            r#"{"t":"function","instDoc":true,"instSuper":true,"name":"HTMLDocument","chain":true,"threw":true,"ne":true,"xml":false}"#
        )
    );
    // A patch on one interface's prototype reaches its own tags only.
    let isolation = rt
        .evaluate(
            r#"(() => {
                    HTMLDivElement.prototype._probe = 'div';
                    Element.prototype._probe2 = 'all';
                    const d = document.getElementById('d');
                    const s = document.getElementById('s');
                    return JSON.stringify([d._probe, s._probe === undefined, d._probe2, s._probe2]);
                })()"#,
        )
        .unwrap();
    assert_eq!(isolation, serde_json::json!(r#"["div",true,"all","all"]"#));
}

/// #27 (obscura#999 face): Web IDL operations are enumerable — zone.js
/// patchClass() discovers methods with `for (prop in instance)` and only
/// walks enumerable properties, so non-enumerable class methods made
/// Angular die with "n.observe is not a function".
#[test]
fn interface_operations_are_enumerable() {
    let mut rt = setup_runtime(r#"<body><div id="d"></div></body>"#);
    let checks: &[(&str, bool)] = &[
        (
            "Object.getOwnPropertyDescriptor(MutationObserver.prototype, 'observe').enumerable",
            true,
        ),
        (
            "Object.getOwnPropertyDescriptor(Element.prototype, 'getAttribute').enumerable",
            true,
        ),
        (
            "Object.getOwnPropertyDescriptor(Node.prototype, 'appendChild').enumerable",
            true,
        ),
        // The zone.js patchClass discovery pattern: for-in over an
        // instance must surface the interface operations as functions.
        (
            "(() => { const mo = new MutationObserver(function(){}); let saw = 0; \
                  for (const k in mo) { if (k === 'observe' && typeof mo[k] === 'function') saw++; } \
                  return saw === 1; })()",
            true,
        ),
        // window.constructor.prototype members (WindowProxy face) stay out
        // of scope: Object.prototype built-ins must NOT become enumerable.
        (
            "Object.getOwnPropertyDescriptor(Object.prototype, 'hasOwnProperty').enumerable",
            false,
        ),
    ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
}

/// #28: SVG elements wrap in their real interfaces (constructor.name,
/// instanceof, patch isolation) and read the true per-node namespace from
/// the tree — parsed SVG children inherit theirs, createElementNS keeps
/// the XML tag case, and svg/HTML <a> discriminate by namespace.
#[test]
fn svg_interface_family_and_namespace() {
    let mut rt = setup_runtime(
        r#"<body><svg id="sv"><path id="pa"></path><linearGradient id="lg"></linearGradient><a id="sa"></a></svg><a id="ha" href="/h">h</a></body>"#,
    );
    let checks: &[(&str, bool)] = &[
        // Chrome chain: SVGSVGElement → SVGGraphicsElement → SVGElement
        // → HTMLElement → Element (Blink routes SVG through HTMLElement).
        (
            "document.getElementById('sv') instanceof SVGSVGElement",
            true,
        ),
        (
            "document.getElementById('sv') instanceof SVGGraphicsElement",
            true,
        ),
        ("document.getElementById('sv') instanceof SVGElement", true),
        // (刀72) Chrome 145: the SVG chain has no HTMLElement level (the #28 claim was wrong).
        (
            "document.getElementById('sv') instanceof HTMLElement",
            false,
        ),
        ("document.getElementById('sv') instanceof Element", true),
        (
            "document.getElementById('pa') instanceof SVGGeometryElement",
            true,
        ),
        // svg is graphics but NOT geometry; shapes are both.
        (
            "document.getElementById('sv') instanceof SVGGeometryElement",
            false,
        ),
        // Namespace discrimination: svg <a> vs HTML <a> never collide.
        ("document.getElementById('sa') instanceof SVGAElement", true),
        (
            "document.getElementById('sa') instanceof HTMLAnchorElement",
            false,
        ),
        (
            "document.getElementById('ha') instanceof HTMLAnchorElement",
            true,
        ),
        (
            "document.getElementById('ha') instanceof SVGAElement",
            false,
        ),
        // The old bare alias made EVERY node instanceof SVGSVGElement.
        ("document.body instanceof SVGSVGElement", false),
        ("SVGSVGElement !== Element", true),
        (
            "Object.getPrototypeOf(SVGSVGElement.prototype) === SVGGraphicsElement.prototype",
            true,
        ),
        (
            "Object.getPrototypeOf(SVGGraphicsElement.prototype) === SVGElement.prototype",
            true,
        ),
        // (刀72) Chrome 145: SVGElement sits directly on Element.
        (
            "Object.getPrototypeOf(SVGElement.prototype) === Element.prototype",
            true,
        ),
        // Parsed SVG children inherit the namespace from the tree; HTML
        // elements keep XHTML. Case: linearGradient keeps its camelCase.
        (
            "document.getElementById('pa').namespaceURI === 'http://www.w3.org/2000/svg'",
            true,
        ),
        (
            "document.getElementById('ha').namespaceURI === 'http://www.w3.org/1999/xhtml'",
            true,
        ),
        (
            "document.getElementById('lg').localName === 'linearGradient'",
            true,
        ),
        (
            "document.getElementById('lg').tagName === 'linearGradient'",
            true,
        ),
        // createElementNS: XML tag case preserved, namespace true.
        (
            "(() => { const c = document.createElementNS('http://www.w3.org/2000/svg', 'linearGradient'); \
                  return c.tagName === 'linearGradient' && c.namespaceURI === 'http://www.w3.org/2000/svg' \
                  && c instanceof SVGLinearGradientElement; })()",
            true,
        ),
        // Empty namespace string is the null namespace per spec.
        (
            "document.createElementNS('', 'foo').namespaceURI === null",
            true,
        ),
        // Class-body methods stay enumerable after the tail pass.
        (
            "Object.getOwnPropertyDescriptor(SVGGraphicsElement.prototype, 'getBBox').enumerable",
            true,
        ),
    ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
    let names = rt
        .evaluate(
            r#"JSON.stringify([
                    document.getElementById('sv').constructor.name,
                    document.getElementById('pa').constructor.name,
                    document.getElementById('lg').constructor.name,
                    document.getElementById('sa').constructor.name,
                    document.getElementById('ha').constructor.name,
                ])"#,
        )
        .unwrap();
    assert_eq!(
        names,
        serde_json::json!(
            r#"["SVGSVGElement","SVGPathElement","SVGLinearGradientElement","SVGAElement","HTMLAnchorElement"]"#
        )
    );
    // A patch on SVGSVGElement's prototype reaches the svg root only.
    let isolation = rt
        .evaluate(
            r#"(() => {
                    SVGSVGElement.prototype._probe = 'svg';
                    const sv = document.getElementById('sv');
                    return JSON.stringify([sv._probe, document.body._probe === undefined]);
                })()"#,
        )
        .unwrap();
    assert_eq!(isolation, serde_json::json!(r#"["svg",true]"#));
}

// #15 (obscura#984): the DOM conversion trio must match Chrome —
// contains() self-short-circuits to true, isSameNode() is identity (not
// equality), and getElementById() coerces its argument to string (a
// numeric 42 finds id="42") with the empty needle returning null.
#[test]
fn dom_conversion_trio_matches_chrome() {
    let mut rt = setup_runtime(
        r#"<html><body><div id="p"><span id="c">x</span></div><div id="q">y</div><div id="42">z</div></body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify([
                    document.getElementById('p').contains(document.getElementById('p')),
                    document.getElementById('p').contains(document.getElementById('c')),
                    document.getElementById('c').contains(document.getElementById('p')),
                    document.getElementById('p').contains(document.getElementById('q')),
                    document.getElementById('p').isSameNode(document.getElementById('p')),
                    document.getElementById('p').isSameNode(document.getElementById('q')),
                    document.getElementById('p').isSameNode(null),
                    document.getElementById(42) === document.getElementById('42'),
                    document.getElementById('') === null
                ])"#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!(r#"[true,true,false,false,true,false,false,true,true]"#)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn read_only_read_write_face_matches_chrome() {
    // #219 / lightpanda #3718: :read-only/:read-write used to fall out
    // of the selector parser entirely (unknown pseudo → rule dropped,
    // matches() false). Chrome oracle: :read-write = editable text
    // controls (no readonly/disabled) or anything on the contenteditable
    // host chain — the host-chain walk is the lightpanda knife (input
    // inside a custom element resolves through its host); :read-only is
    // the negation over every element, plain divs included.
    let mut rt = setup_runtime(
        r#"<html><body>
<ul><li id=a1>1</li><li id=a2>2</li><li id=a3>3</li><li id=a4>4</li></ul>
<div id="host"><input id="plain"></div>
<input id="ro" readonly>
<input id="dis" disabled>
<input id="chk" type="checkbox">
<div id="ce" contenteditable><span id="inner">x</span></div>
<div id="cefalse" contenteditable="false"><span id="innerfalse">x</span></div>
</body></html>"#,
    );
    let script = r#"async () => {
            const m = (id, sel) => { try { return document.getElementById(id).matches(sel); } catch (e) { return 'throw: ' + e.message; } };
            return {
                nth: document.querySelectorAll('li:nth-child( 2n )').length,
                ro_plain: m('plain', ':read-only'), rw_plain: m('plain', ':read-write'),
                ro_ro: m('ro', ':read-only'), rw_ro: m('ro', ':read-write'),
                ro_dis: m('dis', ':read-only'), rw_dis: m('dis', ':read-write'),
                ro_chk: m('chk', ':read-only'), rw_chk: m('chk', ':read-write'),
                ro_div: m('host', ':read-only'), rw_div: m('host', ':read-write'),
                ro_ce: m('ce', ':read-only'), rw_ce: m('ce', ':read-write'),
                ro_inner: m('inner', ':read-only'), rw_inner: m('inner', ':read-write'),
                ro_cefalse: m('cefalse', ':read-only'), rw_innerfalse: m('innerfalse', ':read-write'),
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    // Whitespace-tolerant :nth-child arg (lightpanda #3722) — the
    // selectors crate parses `( 2n )` natively; pinned so a parser
    // regression can't slip through unnoticed.
    assert_eq!(
        v["nth"],
        serde_json::json!(2),
        "li:nth-child( 2n ) matches a2+a4"
    );
    assert_eq!(
        v["ro_plain"],
        serde_json::json!(false),
        "plain input is editable"
    );
    assert_eq!(v["rw_plain"], serde_json::json!(true));
    assert_eq!(v["ro_ro"], serde_json::json!(true), "readonly input");
    assert_eq!(v["rw_ro"], serde_json::json!(false));
    assert_eq!(
        v["ro_dis"],
        serde_json::json!(true),
        "disabled input is read-only"
    );
    assert_eq!(v["rw_dis"], serde_json::json!(false));
    assert_eq!(
        v["ro_chk"],
        serde_json::json!(true),
        "checkbox is never editable"
    );
    assert_eq!(v["rw_chk"], serde_json::json!(false));
    assert_eq!(
        v["ro_div"],
        serde_json::json!(true),
        "plain div is read-only"
    );
    assert_eq!(v["rw_div"], serde_json::json!(false));
    assert_eq!(v["ro_ce"], serde_json::json!(false), "editing host itself");
    assert_eq!(v["rw_ce"], serde_json::json!(true));
    assert_eq!(
        v["ro_inner"],
        serde_json::json!(false),
        "descendant of editing host"
    );
    assert_eq!(v["rw_inner"], serde_json::json!(true));
    assert_eq!(
        v["ro_cefalse"],
        serde_json::json!(true),
        "contenteditable=false is not an editing host"
    );
    assert_eq!(v["rw_innerfalse"], serde_json::json!(false));
}

/// 批240 / obscura#1100: getTotalLength/getPointAtLength through the
/// bootstrap SVGGeometryElement — Chrome's own numbers (headless probe
/// 2026-09-28), the Rust-side PathMeasure (conic chord measure for
/// ovals, analytic for explicit d beziers). getPointAtLength returns an
/// SVGPoint and clamps out-of-range distances to the path's ends.
#[test]
fn svg_geometry_measures_chrome_numbers() {
    let mut rt = setup_runtime(
        r#"<body><svg><path id="p2" d="M0,0 C50,100 100,0 100,100"></path><circle id="c" r="25"></circle></svg></body>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const p2 = document.getElementById('p2');
                    const c = document.getElementById('c');
                    const mid = p2.getPointAtLength(p2.getTotalLength() / 2);
                    const over = p2.getPointAtLength(1e6);
                    const neg = p2.getPointAtLength(-1);
                    return {
                        p2Len: p2.getTotalLength(),
                        cLen: c.getTotalLength(),
                        midX: mid.x, midY: mid.y,
                        overX: over.x, overY: over.y,
                        negX: neg.x, negY: neg.y,
                        ptName: mid.constructor.name,
                        ptIs: mid instanceof SVGPoint,
                    };
                })())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let close = |k: &str, want: f64, tol: f64| {
        let got = d[k].as_f64().unwrap();
        assert!(
            (got - want).abs() < tol,
            "{k}: got {got}, want {want} (diag={d})"
        );
    };
    close("p2Len", 160.71572875976562, 0.05);
    close("cLen", 156.0674285888672, 0.01);
    close("midX", 58.0898, 0.2);
    close("midY", 49.7112, 0.1);
    close("overX", 100.0, 0.01);
    close("overY", 100.0, 0.01);
    close("negX", 0.0, 1e-3);
    close("negY", 0.0, 1e-3);
    assert_eq!(d["ptName"], serde_json::json!("SVGPoint"));
    assert_eq!(
        d["ptIs"],
        serde_json::json!(true),
        "getPointAtLength returns an SVGPoint"
    );
}

/// #191: listeners are DOM §2.9 callbacks — `{handleEvent}` object listeners
/// are legal (github's webcomponents-era code registers four of them) and
/// were blind `.call()`ed at dispatch, producing "h.call is not a function".
/// The object form runs handleEvent with the LISTENER as `this`; null is a
/// legal inert registration; primitives are WebIDL violations that throw at
/// registration, exactly where Chrome throws them (message matches Chrome's).
#[test]
fn object_form_listeners_and_webidl_registration_gate() {
    let mut rt = setup_runtime("<html><body><button id='b'>x</button></body></html>");
    let js = r##"
        const b = document.getElementById('b');
        const log = [];
        const objListener = { handleEvent(e) { log.push('obj:' + (this === objListener) + ':' + e.type); } };
        b.addEventListener('click', function(e) { log.push('fn:' + e.type); });
        b.addEventListener('click', objListener);
        b.addEventListener('click', null);
        b.click();
        document.addEventListener('ping', objListener);
        document.dispatchEvent(new Event('ping'));
        window.addEventListener('pong', objListener);
        window.dispatchEvent(new Event('pong'));
        let threw = null;
        try { b.addEventListener('click', 5); } catch (e) { threw = e && e.name; }
        let wThrew = null;
        try { window.addEventListener('message', 'str'); } catch (e) { wThrew = e && e.name; }
        let dThrew = null;
        try { document.addEventListener('click', true); } catch (e) { dThrew = e && e.name; }
        ({ log, threw, wThrew, dThrew })
    "##;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "log": [
                "fn:click",
                "obj:true:click",
                "obj:true:ping",
                "obj:true:pong"
            ],
            "threw": "TypeError",
            "wThrew": "TypeError",
            "dThrew": "TypeError"
        })
    );
}

/// #170: RTCPeerConnection is a branded WebIDL interface — a bare call
/// throws Chrome's DOM TypeError (not V8's class-constructor text), the
/// brand is [object RTCPeerConnection], and the whole RTC orbit carries
/// its own Symbol.toStringTag. The AWSC collection on the punish page
/// sniffs both faces; a plain ES class answers [object Object] + V8 raw
/// message.
#[test]
fn rtc_peer_connection_face_matches_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
            .evaluate(
                r#"(() => {
                    const out = {};
                    try { RTCPeerConnection(); out.bareCall = 'no-throw'; }
                    catch (e) { out.bareCall = e instanceof TypeError ? e.message : ('wrong:' + e); }
                    const pc = new RTCPeerConnection();
                    out.brand = Object.prototype.toString.call(pc);
                    out.instanceof = pc instanceof RTCPeerConnection;
                    out.ctorName = pc.constructor.name;
                    out.subclass = (() => {
                        class X extends RTCPeerConnection { tag() { return 'x'; } }
                        const x = new X();
                        return x instanceof X && x instanceof RTCPeerConnection && x.tag() === 'x';
                    })();
                    out.webkitBrand = Object.prototype.toString.call(new webkitRTCPeerConnection());
                    out.orbitTags = ['RTCSessionDescription', 'RTCDataChannel', 'RTCStatsReport', 'RTCRtpTransceiver']
                        .map((n) => globalThis[n].prototype[Symbol.toStringTag]);
                    try { new RTCDataChannel(); out.dataChannelCtor = 'no-throw'; }
                    catch (e) { out.dataChannelCtor = e.message; }
                    return JSON.stringify(out);
                })()"#,
            )
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["bareCall"],
        serde_json::json!(
            "Failed to construct 'RTCPeerConnection': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
        ),
        "bare call throws Chrome's DOM TypeError"
    );
    assert_eq!(
        v["brand"],
        serde_json::json!("[object RTCPeerConnection]"),
        "toString brand"
    );
    assert_eq!(
        v["instanceof"],
        serde_json::json!(true),
        "instanceof through the wrapper"
    );
    assert_eq!(
        v["ctorName"],
        serde_json::json!("RTCPeerConnection"),
        "constructor.name stays the interface name"
    );
    assert_eq!(
        v["subclass"],
        serde_json::json!(true),
        "extends still constructs through Reflect.construct"
    );
    assert_eq!(
        v["webkitBrand"],
        serde_json::json!("[object RTCPeerConnection]"),
        "webkit alias carries the brand"
    );
    assert_eq!(
        v["orbitTags"],
        serde_json::json!([
            "RTCSessionDescription",
            "RTCDataChannel",
            "RTCStatsReport",
            "RTCRtpTransceiver"
        ]),
        "RTC orbit carries per-interface tags"
    );
    assert_eq!(
        v["dataChannelCtor"],
        serde_json::json!("Failed to construct 'RTCDataChannel': Illegal constructor"),
        "satellite stubs still throw Illegal constructor (刀78: Chrome's message names the interface)"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webidl_to_string_tag_brands() {
    // #203: every engine shim class used to answer '[object Object]', and
    // doudian's deep-clone helper treated that string as "plain data, walk
    // own keys" — it recursed into an Event (target→node→ownerDocument
    // back-edges) and died with RangeError, taking the whole publish app
    // down. Chrome brands every platform object with its interface name.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const ots = Object.prototype.toString;
            return [
                ots.call(new Event('x')),
                ots.call(new CustomEvent('x')),          // subclass: own tag, not Event's
                ots.call(new XMLHttpRequest()),
                ots.call(new WebSocket('ws://x')),        // no throw: brand read only
                ots.call(document.createElement('div')),
                ots.call(document.createElement('input')),
                ots.call(document),                       // '[object HTMLDocument]'
                ots.call(location),
                ots.call(console),
                ots.call(localStorage),
                ots.call(history),
                ots.call(performance),
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            "[object Event]",
            "[object CustomEvent]",
            "[object XMLHttpRequest]",
            "[object WebSocket]",
            "[object HTMLDivElement]",
            "[object HTMLInputElement]",
            "[object HTMLDocument]",
            "[object Location]",
            "[object console]",
            "[object Storage]",
            "[object History]",
            "[object Performance]",
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_list_reflection_indeterminate_font_family_ec_jwk_order() {
    // Chrome truth for the 2026-10-22 sweep gaps (checked against a
    // local headless Chrome): ol.start/reversed/type, li.value,
    // :indeterminate / live :checked, fontFamily read-back, EC JWK keys.
    let mut rt = setup_runtime(
        "<html><body><ol id=o start=5 reversed type=a><li id=l1 value=9>a<li id=l2 value=x>b</ol>\
             <input type=checkbox id=cb><input type=checkbox id=cb2 checked><progress id=pg></progress></body></html>",
    );
    let v = rt
            .evaluate(
                "(() => { cb.indeterminate = true; const ff = (s) => { const d = document.createElement('div'); d.style.fontFamily = s; return d.style.fontFamily; };
                  const r = { ol: [o.start, o.reversed, o.type, l1.value, l2.value].join(),
                    ind: [cb.matches(':indeterminate'), cb2.matches(':indeterminate'), pg.matches(':indeterminate')].join() };
                  cb.indeterminate = false; r.indOff = cb.matches(':indeterminate');
                  cb.checked = true; cb2.checked = false;
                  r.checked = [cb.matches(':checked'), cb2.matches(':checked')].join();
                  r.ff = [ff('Foo Bar, serif'), ff(\"'Menlo',monospace\"), ff('Arial'), ff('\"Times New Roman\", \"serif\"'), ff('a b, 1x')].join('|');
                  o.start = -3; o.reversed = false; r.set = [o.getAttribute('start'), o.hasAttribute('reversed')].join();
                  return r; })()",
            )
            .unwrap();
    assert_eq!(v["ol"], "5,true,a,9,0");
    assert_eq!(v["ind"], "true,false,true");
    assert_eq!(v["indOff"], false);
    assert_eq!(
        v["checked"], "true,false",
        ":checked follows the dirty checkedness"
    );
    assert_eq!(
        v["ff"], "\"Foo Bar\", serif|Menlo, monospace|Arial|\"Times New Roman\", \"serif\"|",
        "invalid unquoted family drops the whole declaration"
    );
    assert_eq!(v["set"], "-3,false");

    let keys = rt
            .evaluate_remote(
                "(async () => { const k = await crypto.subtle.generateKey({name:'ECDSA',namedCurve:'P-256'}, true, ['sign','verify']);
                   return Object.keys(await crypto.subtle.exportKey('jwk', k.privateKey)).join(); })()",
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(keys.value.unwrap(), "crv,d,ext,key_ops,kty,x,y");
}

#[tokio::test(flavor = "current_thread")]
async fn test_small_web_api_gaps_from_the_chrome_battery() {
    // Chrome truth (headless Chrome, 2026-10-22).
    let mut rt = setup_runtime("<html><body><div id=host></div></body></html>");
    let v = rt
            .evaluate_remote(
                "(async () => { const r = [];
                  r.push(typeof scheduler.postTask + typeof scheduler.yield);
                  r.push(await scheduler.postTask(() => 7, { priority: 'background' }));
                  r.push(typeof Blob.prototype.stream);
                  const rd = new Blob(['abc']).stream().getReader(); const c = await rd.read(); r.push(c.value.length);
                  r.push(new XMLSerializer().serializeToString(document.createElement('div')));
                  const d = document.createElement('div'); document.body.append(d);
                  const a = d.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 40 });
                  r.push(a.playState + document.getAnimations().length + d.getAnimations().length);
                  await a.finished; r.push(a.playState + document.getAnimations().length);
                  r.push(typeof document.startViewTransition + typeof document.queryCommandSupported + document.queryCommandSupported('bold'));
                  const vt = document.startViewTransition(() => { r.push('updated'); }); await vt.finished;
                  r.push(CSS.supports('caret-color', 'red') + ',' + CSS.supports('selector(::selection)') + ',' + CSS.supports('grid-template-columns', 'subgrid'));
                  return r.join('|'); })()",
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        v.value.unwrap(),
        "functionfunction|7|function|3|<div xmlns=\"http://www.w3.org/1999/xhtml\"></div>|running11|finished0|functionfunctiontrue|updated|true,true,false"
    );
}

/// JS-PLATFORM-ARCH M3: DOMException, EventTarget, Event and CustomEvent are
/// NATIVE interfaces (the Moli-direction binding layer). This runs the SAME
/// probe that produced chrome/event-truth-145.json in real Chrome 145 and
/// demands the engine match, probe by probe - except the known gaps below.
/// It is a RATCHET: a listed gap that starts matching must be removed from
/// the list. The Event-class migration (step C) emptied the list; new
/// entries may only be added for genuinely-new regressions.
#[test]
fn event_stack_matches_chrome_truth_except_known_event_gaps() {
    const KNOWN_EVENT_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/event-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/event-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        let known = KNOWN_EVENT_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_EVENT_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 1: the geometry family (DOMPoint/DOMRect/DOMQuad/DOMMatrix*
/// and WebKitCSSMatrix) is generated shape (gen/geometry.gen.js) + behavior
/// module (geometry.impl.js). The probe runs against the live engine and is
/// diffed key-by-key against real Chrome 145 (captured via
/// tools/webplatform/capture/probe_run.py). KNOWN_GEOMETRY_GAPS is a
/// ratchet: an entry that starts matching must be removed.
///
/// ro.rotate45 (≤1 ulp): Blink's rotate path uses Chromium's own SIMD
/// sincos (third_party/simd), whose last-ulp rounding differs from
/// V8's Math.sin at some angles (observed 45° and 60°: sin 0.7071067811865475
/// vs ...476, 0.8660254037844386 vs ...387). The engine's Math.sin itself is
/// bit-identical to real Chrome's Math.sin — verified by capture — so this is
/// purely the C++ library's rounding, not reachable without emulating its
/// FMA-evaluated polynomial. cosSin() uses Math.sin/Math.cos.
#[test]
fn geometry_stack_matches_chrome_truth_except_known_geometry_gaps() {
    const KNOWN_GEOMETRY_GAPS: &[&str] = &["ro.rotate45"];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/geometry-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/geometry-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        let known = KNOWN_GEOMETRY_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_GEOMETRY_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 2: the HTMLElement/Element common-member layer. Chrome 145 owns
/// 102 on* + 38 members on HTMLElement.prototype, 8 on* on Element.prototype,
/// all 110 on* on Document.prototype; the engine bootstrap had 98 on* on
/// Element.prototype and a bare HTMLElement.prototype. The patch module
/// (gen/htmlelement.gen.js, generated from htmlelement-truth-145.json in
/// Chrome's own property order) relocates misplaced descriptors, installs the
/// missing members (uniform reflection = DATA via P.reflector; non-uniform
/// semantics in htmlelement.impl.js) and deletes engine on* extras Chrome
/// owns on no level. This test diffs the live engine against the same truth
/// file, key by key.
///
/// STAGED members are later family knives (the engine ledger, local-only)
/// and are
/// filtered out of the order/kinds comparisons. Knife 4 landed every staged
/// family, so the filter is empty. KNOWN_HTML_GAPS is a ratchet: an entry
/// that starts matching must be removed.
///
/// - fn.length: resolved at knife 4 (moveBefore/requestFullscreen installed
///   with Chrome's arity); the dict now matches in full.
/// - hasAttributeNS.cases: resolved (namespace-aware attribute storage) —
///   the engine now matches Chrome on the null-namespace lookups too.
#[test]
fn htmlelement_members_match_chrome_truth_except_known_gaps() {
    // Knife 4 landed all nine staged families (part/Typed OM/fullscreen
    // trio/pointer lock/moveBefore/ariaNotify — their behavior lives in
    // element_staged_members_match_chrome_truth); the filter stays as the
    // mechanism for any future staging.
    const STAGED: &[&str] = &[];
    const KNOWN_HTML_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/htmlelement-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/htmlelement-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        match k.as_str() {
            // HTMLElement own order IS the contract: Chrome's sequence minus
            // the staged names, byte for byte.
            "HT.order" => {
                let w: Vec<&str> = want
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .filter(|n| !STAGED.contains(n))
                    .collect();
                let g: Vec<&str> = got
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .filter(|n| !STAGED.contains(n))
                    .collect();
                if g != w {
                    let extra: Vec<&str> = g.iter().filter(|n| !w.contains(n)).copied().collect();
                    let miss: Vec<&str> = w.iter().filter(|n| !g.contains(n)).copied().collect();
                    let fd = g.iter().zip(w.iter()).position(|(a, b)| a != b);
                    bad.push(format!("HT.order: len {}/{} extra {extra:?} missing {miss:?} first-divergence {fd:?} (engine {:?}, chrome {:?})",
                        g.len(), w.len(),
                        fd.map(|i| g[i]).unwrap_or(""),
                        fd.map(|i| w[i]).unwrap_or("")));
                }
                continue;
            }
            "HT.kinds" => {
                for (n, kwant) in want.as_object().unwrap() {
                    if STAGED.contains(&n.as_str()) {
                        continue;
                    }
                    let kgot = &got[n];
                    if kgot != kwant {
                        bad.push(format!("HT.kinds.{n}: engine {kgot} chrome {kwant}"));
                    }
                }
                continue;
            }
            // Element still carries the staged per-element faces (later
            // family knives move them), so EL is a SUBSET contract: every
            // non-staged Chrome member must be own with the same kind.
            "EL.order" => {
                let g: Vec<&str> = got
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                for n in want.as_array().unwrap() {
                    let n = n.as_str().unwrap();
                    if !STAGED.contains(&n) && !g.contains(&n) {
                        bad.push(format!("EL.order: missing {n}"));
                    }
                }
                continue;
            }
            "EL.kinds" => {
                for (n, kwant) in want.as_object().unwrap() {
                    if STAGED.contains(&n.as_str()) {
                        continue;
                    }
                    let kgot = &got[n];
                    if kgot != kwant {
                        bad.push(format!("EL.kinds.{n}: engine {kgot} chrome {kwant}"));
                    }
                }
                continue;
            }
            _ => {}
        }
        let known = KNOWN_HTML_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_HTML_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 4: the Element staged family, landed. Chrome truth
/// (element-staged-truth-157.json, 38 keys; 157 re-truth: computed length 413->480) pins the nine members the
/// knife-2 ledger staged — part (cached DOMTokenList rw accessor),
/// attributeStyleMap/computedStyleMap (Typed OM; StylePropertyMap/
/// StylePropertyMapReadOnly owned by the stylemap module: ctor keeps
/// Chrome's Illegal-constructor throw, prototype members in Chrome's own
/// order, @@iterator = entries), the fullscreen trio (requestFullscreen
/// rejects TypeError "Permissions check failed"; the webkit pair returns
/// undefined), requestPointerLock (real Promise rejecting DOMException
/// WrongDocumentError), moveBefore (Element/Document/DocumentFragment
/// hosts, insertBefore-shaped move with Chrome's exact TypeError/
/// HierarchyRequestError/NotFoundError messages) and ariaNotify (len 1,
/// returns undefined). The probe settles its promise faces (fullscreen/
/// pointer-lock rejections race a 3s timeout so a hang can't wedge the
/// test) and is awaited through call_function_on_remote(await_promise).
///
/// KNOWN_GAPS ratchet: empty since M4 knife 54 (the computed-property list is Chrome's 413 longhands). csm.face is checked as the
/// INVARIANT computedStyleMap().size === getComputedStyle(el).length.
#[tokio::test(flavor = "current_thread")]
async fn element_staged_members_match_chrome_truth() {
    const KNOWN_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/element-staged-probe.js");
    let result = rt
        // call_function_on_remote invokes its argument as a FUNCTION
        // (__fn.call), so hand it the probe wrapped as an arrow returning
        // the probe's promise; await_promise settles it.
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/element-staged-truth-157.json"
    ))
    .unwrap();
    let eng_gcs = engine["gcs.len"].clone();
    let tru_gcs = truth["gcs.len"].clone();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        if k.as_str() == "csm.face" {
            let (g, w) = (got.as_array().unwrap(), want.as_array().unwrap());
            if !(g.len() == w.len()
                && g[0] == w[0]
                && g[1] == w[1]
                && g[2] == eng_gcs
                && w[2] == tru_gcs)
            {
                bad.push(format!(
                    "csm.face: engine {got} (own gcs {eng_gcs}) chrome {want} (own gcs {tru_gcs})"
                ));
            }
            continue;
        }
        let known = KNOWN_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 6: the HTML reflected-attribute element family
/// (htmlreflect-probe.js). For 27 element interfaces — plus HTMLElement/
/// HTMLImageElement/HTMLVideoElement/HTMLMediaElement as regression rows for
/// the reflectors knives 2 and 5 landed — the probe records a BEHAVIOR
/// FINGERPRINT per read-write member in real Chrome 145 (default read; reads
/// after 11 property sets; the content attribute the set created; reads after
/// 15 raw setAttribute values; a keyword-vocabulary sweep for normalizing
/// members) and the test diffs the engine's fingerprint against it member by
/// member, plus own-name presence, own ORDER of the names both sides have,
/// DOMTokenList supports() sets, instance routing (createElement(tag)
/// instanceof its interface) and the knife-2 string-conversion extras.
///
/// Reflector kinds are MEASURED, not remembered: gen/htmlreflect.gen.js is
/// generated from this same truth through tools/webplatform/gen/
/// reflect_classify.py (a mirror of core.js P.reflector that accepts a kind
/// only if it reproduces every facet). The fingerprint caught latent
/// deviations in knife 2/5 reflectors (null/undefined DOMString conversion,
/// translate's yes/no, preload's empty state, crossOrigin's unknown value,
/// img.longDesc being a URL, unsigned-long edge parsing, the 14 interfaces
/// whose createElement instances were bare HTMLElement).
///
/// KNOWN ratchets (not this family; each entry that starts matching must be
/// removed — the test fails on a stale entry):
/// - protocol (a/area): URL decomposition of an href-less anchor (':' vs '').
/// - HTMLElement editContext/innerText/outerText/style: EditContext and
///   CSSOM families (offsetHeight/offsetWidth differ only in the layout-backed
///   served engine, not in this unit runtime).
/// - Form.elements: HTMLFormControlsCollection default.
/// - Media buffered/played/seekable (TimeRanges), currentTime/playbackRate
///   (double conversion + NotSupportedError range): media state family.
/// - Script.innerText undefined: Trusted Types union conversion.
/// - extra *.sym: relocated native string setters do not throw on Symbol.
#[tokio::test(flavor = "current_thread")]
async fn htmlreflect_members_match_chrome_truth() {
    const KNOWN_FP: &[&str] = &[
        "HTMLAnchorElement.protocol",
        "HTMLAreaElement.protocol",
        "HTMLElement.innerText",
        "HTMLElement.outerText",
        "HTMLElement.style",
        "HTMLMediaElement.buffered",
        "HTMLMediaElement.currentTime",
        "HTMLMediaElement.played",
        "HTMLMediaElement.playbackRate",
        "HTMLMediaElement.seekable",
        "HTMLScriptElement.innerText",
        // knife 7 (form family) - state machinery that is not reflection:
        "HTMLInputElement.files",
        "HTMLInputElement.selectionEnd",
        "HTMLInputElement.selectionStart",
        "HTMLInputElement.type",
        "HTMLInputElement.value",
        "HTMLInputElement.valueAsDate",
        "HTMLTextAreaElement.selectionEnd",
        "HTMLTextAreaElement.selectionStart",
        "HTMLTextAreaElement.value",
    ];
    // Empty (2026-11-04): every previously-missing experimental name —
    // interestForElement, the iframe ad/privacy cluster, form/select/
    // template collections — now installs with Chrome's fingerprint.
    const KNOWN_MISSING: &[&str] = &[];
    // bootstrap {value} constants are non-configurable and cannot be
    // re-seated (knife 5); strip them from the order comparison
    const IMMOVABLE: &[&str] = &[
        "NETWORK_EMPTY",
        "NETWORK_IDLE",
        "NETWORK_LOADING",
        "NETWORK_NO_SOURCE",
        "HAVE_NOTHING",
        "HAVE_METADATA",
        "HAVE_CURRENT_DATA",
        "HAVE_FUTURE_DATA",
        "HAVE_ENOUGH_DATA",
        // HTMLTrackElement IDL constants: non-configurable, so reseat cannot move them
        "NONE",
        "LOADING",
        "LOADED",
        "ERROR",
    ];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/htmlreflect-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/htmlreflect-truth-145.json"
    ))
    .unwrap();

    let mut bad: Vec<String> = Vec::new();
    let names = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .map(|a| a.iter().map(|n| n.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    };
    let mut stale_known: Vec<&str> = Vec::new();
    let mut seen_fp_diff: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_missing: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (iface, t_order) in truth["order"].as_object().unwrap() {
        let t_names = names(t_order);
        let e_names = names(&engine["order"][iface]);
        if e_names.is_empty() {
            bad.push(format!("{iface}: interface missing in engine"));
            continue;
        }
        // 1. presence
        for n in &t_names {
            if !e_names.contains(n) {
                let key = format!("{iface}.{n}");
                if KNOWN_MISSING.contains(&key.as_str()) {
                    seen_missing.insert(key);
                } else {
                    bad.push(format!("{key}: Chrome owns it, engine does not"));
                }
            }
        }
        // 2. order of the names both sides own
        let strip = |v: &Vec<String>, other: &Vec<String>| -> Vec<String> {
            v.iter()
                .filter(|n| other.contains(n) && !IMMOVABLE.contains(&n.as_str()))
                .cloned()
                .collect()
        };
        let (to, eo) = (strip(&t_names, &e_names), strip(&e_names, &t_names));
        if to != eo {
            bad.push(format!(
                "{iface}: own order differs\n   engine {eo:?}\n   chrome {to:?}"
            ));
        }
        // 3. fingerprints
        for (m, t_fp) in truth["fp"][iface].as_object().unwrap() {
            let e_fp = &engine["fp"][iface][m];
            if e_fp.is_null() {
                continue; // absence already reported above
            }
            let key = format!("{iface}.{m}");
            if t_fp != e_fp {
                seen_fp_diff.insert(key.clone());
                if !KNOWN_FP.contains(&key.as_str()) {
                    // name the first differing facet
                    let mut facet = String::from("?");
                    for g in ["def", "attr"] {
                        if t_fp[g] != e_fp[g] {
                            facet = format!("{g}: chrome {} engine {}", t_fp[g], e_fp[g]);
                        }
                    }
                    for g in ["set", "fromAttr"] {
                        if let Some(o) = t_fp[g].as_object() {
                            for (k, tv) in o {
                                if &e_fp[g][k] != tv && facet == "?" {
                                    facet = format!("{g}[{k}]: chrome {tv} engine {}", e_fp[g][k]);
                                }
                            }
                        }
                    }
                    if facet == "?" && t_fp["vocab"] != e_fp["vocab"] {
                        facet = "vocab".to_string();
                    }
                    bad.push(format!("{key}: fingerprint differs ({facet})"));
                }
            }
        }
    }
    for k in KNOWN_FP {
        if !seen_fp_diff.contains(*k) {
            stale_known.push(k);
        }
    }
    for k in KNOWN_MISSING {
        if !seen_missing.contains(*k) {
            stale_known.push(k);
        }
    }
    for k in stale_known {
        bad.push(format!(
            "{k}: now matches Chrome - remove it from the KNOWN list"
        ));
    }

    // 4. supports() sets of every DOMTokenList member
    if truth["tokens"] != engine["tokens"] {
        bad.push(format!(
            "tokens (supports()) differ:\n   engine {}\n   chrome {}",
            engine["tokens"], truth["tokens"]
        ));
    }
    // 5. instance routing
    if truth["routing"] != engine["routing"] {
        for (k, tv) in truth["routing"].as_object().unwrap() {
            if &engine["routing"][k] != tv {
                bad.push(format!(
                    "routing {k}: engine {} chrome {tv}",
                    engine["routing"][k]
                ));
            }
        }
    }
    // 6. knife-2 string conversions (title/lang/accessKey): everything but
    //    the Symbol edge, which the relocated native setters do not throw on
    for (k, tv) in truth["extra"].as_object().unwrap() {
        for (case, want) in tv.as_object().unwrap() {
            if case == "sym" {
                continue;
            }
            if &engine["extra"][k][case] != want {
                bad.push(format!(
                    "extra {k}.{case}: engine {} chrome {want}",
                    engine["extra"][k][case]
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} divergence(s):\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 9: the Generic Sensor family (sensor-probe.js, real Chrome 145
/// headless): Sensor / Accelerometer / LinearAccelerationSensor /
/// GravitySensor / Gyroscope / OrientationSensor / Absolute- and
/// RelativeOrientationSensor / SensorErrorEvent. With no sensor hardware every
/// sensor constructs, start() asynchronously fires a TRUSTED SensorErrorEvent
/// (NotReadableError "Could not connect to a sensor") and nothing ever
/// activates; Magnetometer / AmbientLightSensor do not exist. The probe covers
/// shape and chain, options-dictionary conversion order and wording, brand
/// checks, populateMatrix argument checks, the start()/stop() event sequences
/// (twice -> one error, stop cancels, restart works) and SensorErrorEvent's
/// required `error`. IDL on* handlers are logged apart from listeners: the
/// engine core fires handlers first (a known core-level divergence).
#[tokio::test(flavor = "current_thread")]
async fn sensor_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/sensor-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/sensor-truth-145.json"
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

/// M4 knife 10: Web Notifications (notification-probe.js, 42 keys, real
/// Chrome 145 headless). Permission starts 'default'; the first
/// requestPermission() settles 'denied' and sticks (no prompt UI); a
/// Notification constructed without permission asynchronously fires a TRUSTED
/// plain `error` Event and never shows. Covers shape and statics order,
/// constructor errors, the NotificationOptions dictionary read in WebIDL order
/// with Chrome's wording and cross-member rules (silent+vibrate, renotify
/// needs a tag, actions only for persistent notifications), URL resolution of
/// icon/badge, unsigned-long-long timestamp wrap, frozen vibrate arrays,
/// structured-cloned data (DataCloneError text names the value) and the
/// requestPermission promise/callback/argument check. Must run in a FRESH
/// runtime: the live /eval path executes a script twice and would see the
/// permission already 'denied'.
#[tokio::test(flavor = "current_thread")]
async fn notification_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/notification-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/notification-truth-145.json"
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

/// M4 knife 11: Web Speech recognition + ClipboardChangeEvent
/// (speech-probe.js, 74 keys, real Chrome 145 headless). SpeechRecognition
/// constructs; start() asynchronously fires a trusted SpeechRecognitionErrorEvent
/// 'not-allowed' then a trusted plain 'end' (never 'start'); a second start()
/// throws InvalidStateError; stop()/abort() while started cancel the error and
/// leave only 'end'. The webkit* names are the same interface objects;
/// SpeechRecognitionResult/ResultList/Alternative are not exposed. Also pinned:
/// shapes and statics, grammar list (own index properties, data: URIs, float32
/// weights), SpeechRecognitionPhrase boost range, the event inits (error is a
/// free DOMString), SpeechRecognition.available()/install() option checks and
/// the on-device availability answers, and ClipboardChangeEvent(init) - no
/// type argument, BigInt changeId, a fresh frozen `types` array per read.
#[tokio::test(flavor = "current_thread")]
async fn speech_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/speech-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/speech-truth-145.json"
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

/// M4 knife 13: RTCIceCandidate (rtcice-probe.js, 72 keys, real Chrome 145).
/// The bootstrap class only stored `candidate`; Chrome parses the candidate-
/// attribute string into foundation/component/priority/address/protocol/port/
/// type/tcpType/relatedAddress/relatedPort, ALL-OR-NOTHING (any malformed token
/// leaves every parsed field null): single spaces only, lowercase `typ` and
/// type keywords, transport case-insensitive and normalized, port/priority
/// range checks, IPv6 bare addresses bracketed, ufrag/generation/... ignored,
/// raddr/rport defaulting each other ('' / 0), `usernameFragment` only from
/// the init. Pinned too: constructor validation (sdpMid and sdpMLineIndex both
/// null), the RTCIceCandidateInit conversions (unsigned short wrap) and toJSON
/// (null sdpMid/usernameFragment -> '', null sdpMLineIndex omitted).
#[tokio::test(flavor = "current_thread")]
async fn rtcice_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/rtcice-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/rtcice-truth-145.json"
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

/// M4 knife 14: the small-member sweep (smallmembers-probe.js, 51 keys, real
/// Chrome 145): CharacterData.nextElementSibling/previousElementSibling,
/// Text.assignedSlot, ProcessingInstruction.sheet, DocumentFragment.
/// childElementCount; HashChangeEvent / SubmitEvent / PopStateEvent /
/// ClipboardEvent (init dictionaries, exact conversion wording; SubmitEvent
/// also from the real form-submission path, carrying the submitter and
/// trusted); MessageEvent.initMessageEvent + userActivation; DataTransfer
/// setData/getData/clearData (format normalization, replace, frozen cached
/// `types`); RTCSessionDescription (read-write type/sdp, enum check, toJSON);
/// InputDeviceCapabilities. Shapes are compared as: same parent, every Chrome
/// member present, the members both sides have in the same relative order.
/// KNOWN: Text/Comment/ProcessingInstruction/DocumentFragment carry extra own
/// members (nodeName/nodeType/cloneNode/innerHTML/nodeValue - Chrome puts them
/// on Node.prototype) and DocumentFragment/ProcessingInstruction constructor
/// lengths differ: the node-level placement family.
#[tokio::test(flavor = "current_thread")]
async fn smallmembers_match_chrome_truth() {
    const KNOWN_LEN: &[&str] = &["DocumentFragment", "ProcessingInstruction"];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/smallmembers-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/smallmembers-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if k == "shape" {
            for (name, t) in want.as_object().unwrap() {
                let e = &engine["shape"][name];
                if e.is_string() || e.is_null() {
                    bad.push(format!("shape {name}: interface missing in engine"));
                    continue;
                }
                let names = |v: &serde_json::Value| -> Vec<String> {
                    v["order"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect()
                };
                let (tn, en) = (names(t), names(e));
                for n in &tn {
                    if !en.contains(n) {
                        bad.push(format!("shape {name}: Chrome owns {n}, engine does not"));
                    }
                }
                let common_t: Vec<_> = tn.iter().filter(|n| en.contains(n)).collect();
                let common_e: Vec<_> = en.iter().filter(|n| tn.contains(n)).collect();
                if common_t != common_e {
                    bad.push(format!("shape {name}: member order differs\n   engine {common_e:?}\n   chrome {common_t:?}"));
                }
                if t["parent"] != e["parent"] {
                    bad.push(format!(
                        "shape {name}: parent engine {} chrome {}",
                        e["parent"], t["parent"]
                    ));
                }
                if t["len"] != e["len"] && !KNOWN_LEN.contains(&name.as_str()) {
                    bad.push(format!(
                        "shape {name}: ctor length engine {} chrome {}",
                        e["len"], t["len"]
                    ));
                }
            }
        } else if &engine[k] != want {
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

/// M4 knife 19: IntersectionObserver + IntersectionObserverEntry (ioentry-probe.js,
/// 31 keys, real Chrome 145): the entry is a getter-only interface with an illegal
/// constructor and observer-delivered entries are real instances over
/// DOMRectReadOnly; the observer validates callback/options with exact wording,
/// normalizes rootMargin/scrollMargin to four tokens, keeps thresholds as sorted
/// float32 and returns a fresh frozen copy per read, calls back with
/// (entries, observer) and this === observer. The always-intersecting policy and
/// the re-fire schedule are unchanged.
#[tokio::test(flavor = "current_thread")]
async fn intersection_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/ioentry-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/ioentry-truth-145.json"
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

/// M4 knife 20: Document legacy/misc members (docmisc-probe.js, 36 keys, real Chrome 145):
/// the body-colour legacy attributes (reflect bgcolor/text/link/vlink/alink, null -> ''),
/// live anchors/embeds/plugins with stable identity (embeds === plugins), applets,
/// designMode normalization, idle fullscreen/pointer-lock faces whose rw setters ignore
/// writes, lastModified, no-op captureEvents/releaseEvents/clear, caretPositionFromPoint /
/// CaretPosition, the privacy promise members, FragmentDirective, relative member order.
/// Previously every one of these was absent, so `document.bgColor = 'red'` minted an own
/// data property on the document.
#[tokio::test(flavor = "current_thread")]
async fn docmisc_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/docmisc-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/docmisc-truth-145.json"
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

/// M4 knife 21: XPath 1.0 (xpath-probe.js, 583 keys, real Chrome 145): ~120 location
/// paths over every axis, ~370 scalar expressions and core functions (number formatting
/// to six significant digits, substring/round edge cases, comparison rules), result types
/// and their accessor errors, ~130 malformed expressions with exact wording, namespace
/// resolution (NamespaceError, resolver forms), unordered-vs-ordered result order,
/// iterator invalidation on mutation, XPathExpression / XPathEvaluator / createNSResolver.
/// KNOWN gap (engine, not XPath): documents made by DOMParser / createHTMLDocument /
/// createDocument are not Document instances here, so they carry no evaluate().
#[tokio::test(flavor = "current_thread")]
async fn xpath_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/xpath-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/xpath-truth-145.json"
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

/// M4 knife 51 - the HTMLCollection family against real Chrome 145 (coll-probe.js, 25 keys): HTMLCollection is a legacy platform
/// object (not an Array: indexed read-only props, non-enumerable named props, prototype `length`, only item/namedItem/@@iterator),
/// HTMLOptionsCollection (add/remove/selectedIndex/writable length) and HTMLFormControlsCollection (RadioNodeList on several matches).
#[tokio::test(flavor = "current_thread")]
async fn htmlcollection_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/coll-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/coll-truth-145.json"
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

/// M4 刀55: Element.prototype's own member set == Chrome 145's exactly
/// (shape-audit-truth-145.json, Element entry, string keys). The engine
/// historically reflected every HTML/SVG member onto Element.prototype
/// with localName guards ("the kitchen sink"): a <div> answered .value/
/// .rows/.href, for-in over any element exposed all of them, and Chrome
/// has none of it. The sink moved to the interfaces that own it (the
/// bootstrap _interfaceProps MOVE table + SVG homes in element_sink.js);
/// this pins the graveyard shut — a new Element member Chrome doesn't
/// have fails here, and belongs in element_sink.js's delete list instead.
#[tokio::test(flavor = "current_thread")]
async fn element_prototype_members_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate("JSON.stringify(Object.getOwnPropertyNames(Element.prototype).sort())")
        .unwrap();
    let engine: Vec<String> = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let chrome: Vec<&str> = [
        "after",
        "animate",
        "append",
        "ariaActiveDescendantElement",
        "ariaAtomic",
        "ariaAutoComplete",
        "ariaBrailleLabel",
        "ariaBrailleRoleDescription",
        "ariaBusy",
        "ariaChecked",
        "ariaColCount",
        "ariaColIndex",
        "ariaColIndexText",
        "ariaColSpan",
        "ariaControlsElements",
        "ariaCurrent",
        "ariaDescribedByElements",
        "ariaDescription",
        "ariaDetailsElements",
        "ariaDisabled",
        "ariaErrorMessageElements",
        "ariaExpanded",
        "ariaFlowToElements",
        "ariaHasPopup",
        "ariaHidden",
        "ariaInvalid",
        "ariaKeyShortcuts",
        "ariaLabel",
        "ariaLabelledByElements",
        "ariaLevel",
        "ariaLive",
        "ariaModal",
        "ariaMultiLine",
        "ariaMultiSelectable",
        "ariaNotify",
        "ariaOrientation",
        "ariaPlaceholder",
        "ariaPosInSet",
        "ariaPressed",
        "ariaReadOnly",
        "ariaRelevant",
        "ariaRequired",
        "ariaRoleDescription",
        "ariaRowCount",
        "ariaRowIndex",
        "ariaRowIndexText",
        "ariaRowSpan",
        "ariaSelected",
        "ariaSetSize",
        "ariaSort",
        "ariaValueMax",
        "ariaValueMin",
        "ariaValueNow",
        "ariaValueText",
        "assignedSlot",
        "attachShadow",
        "attributes",
        "before",
        "checkVisibility",
        "childElementCount",
        "children",
        "classList",
        "className",
        "clientHeight",
        "clientLeft",
        "clientTop",
        "clientWidth",
        "closest",
        "computedStyleMap",
        "constructor",
        "currentCSSZoom",
        "elementTiming",
        "firstElementChild",
        "getAnimations",
        "getAttribute",
        "getAttributeNS",
        "getAttributeNames",
        "getAttributeNode",
        "getAttributeNodeNS",
        "getBoundingClientRect",
        "getClientRects",
        "getElementsByClassName",
        "getElementsByTagName",
        "getElementsByTagNameNS",
        "getHTML",
        "hasAttribute",
        "hasAttributeNS",
        "hasAttributes",
        "hasPointerCapture",
        "id",
        "innerHTML",
        "insertAdjacentElement",
        "insertAdjacentHTML",
        "insertAdjacentText",
        "lastElementChild",
        "localName",
        "matches",
        "moveBefore",
        "namespaceURI",
        "nextElementSibling",
        "onbeforecopy",
        "onbeforecut",
        "onbeforepaste",
        "onfullscreenchange",
        "onfullscreenerror",
        "onsearch",
        "onwebkitfullscreenchange",
        "onwebkitfullscreenerror",
        "outerHTML",
        "part",
        "prefix",
        "prepend",
        "previousElementSibling",
        "querySelector",
        "querySelectorAll",
        "releasePointerCapture",
        "remove",
        "removeAttribute",
        "removeAttributeNS",
        "removeAttributeNode",
        "replaceChildren",
        "replaceWith",
        "requestFullscreen",
        "requestPointerLock",
        "role",
        "scroll",
        "scrollBy",
        "scrollHeight",
        "scrollIntoView",
        "scrollIntoViewIfNeeded",
        "scrollLeft",
        "scrollTo",
        "scrollTop",
        "scrollWidth",
        "setAttribute",
        "setAttributeNS",
        "setAttributeNode",
        "setAttributeNodeNS",
        "setHTMLUnsafe",
        "setPointerCapture",
        "shadowRoot",
        "slot",
        "tagName",
        "toggleAttribute",
        "webkitMatchesSelector",
        "webkitRequestFullScreen",
        "webkitRequestFullscreen",
    ]
    .to_vec();
    let extra: Vec<&String> = engine
        .iter()
        .filter(|k| !chrome.contains(&k.as_str()))
        .collect();
    let missing: Vec<&&str> = chrome
        .iter()
        .filter(|k| !engine.contains(&k.to_string()))
        .collect();
    assert!(
        extra.is_empty() && missing.is_empty(),
        "Element.prototype member set drifted from Chrome 145: extra {extra:?} missing {missing:?}"
    );
}
