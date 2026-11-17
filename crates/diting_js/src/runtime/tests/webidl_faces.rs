//! 通用 WebIDL 面拍：members/stack/surface snapshot/svg/xpath
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
/// probe that produced chrome/event-truth-157.json in real Chrome 145 and
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
        "../../../../../tools/webplatform/chrome/event-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/geometry-truth-157.json"
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
/// (gen/htmlelement.gen.js, generated from htmlelement-truth-157.json in
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
    // A layout disagreement goes back in this list only while it still
    // differs. A match must leave: the ratchet below fails a stale entry.
    const KNOWN_HTML_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/htmlelement-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/htmlelement-truth-157.json"
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
/// KNOWN_FP is a ratchet: an entry that starts matching must be removed.
/// The reflect fingerprints above (protocol, innerText, outerText, style,
/// input value/type/files/selection/valueAsDate, media time/rate/ranges)
/// match Chrome 157, so the list is empty.
#[tokio::test(flavor = "current_thread")]
async fn htmlreflect_members_match_chrome_truth() {
    const KNOWN_FP: &[&str] = &[];
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
        "../../../../../tools/webplatform/chrome/htmlreflect-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/sensor-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/notification-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/speech-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/rtcice-truth-157.json"
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
/// on Node.prototype). Constructor length is part of the shape: a match that
/// is still listed below fails the test.
#[tokio::test(flavor = "current_thread")]
async fn smallmembers_match_chrome_truth() {
    const KNOWN_LEN: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/smallmembers-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/smallmembers-truth-157.json"
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
                if t["len"] == e["len"] && KNOWN_LEN.contains(&name.as_str()) {
                    bad.push(format!(
                        "shape {name}: ctor length now matches Chrome - remove it from KNOWN_LEN"
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
        "../../../../../tools/webplatform/chrome/ioentry-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/docmisc-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/xpath-truth-157.json"
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
        "../../../../../tools/webplatform/chrome/coll-truth-157.json"
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
/// (shape-audit-truth-157.json, Element entry, string keys). The engine
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
        "activeViewTransition",
        "after",
        "afterHTML",
        "afterHTMLUnsafe",
        "animate",
        "append",
        "appendHTML",
        "appendHTMLUnsafe",
        "ariaActionsElements",
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
        "beforeHTML",
        "beforeHTMLUnsafe",
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
        "customElementRegistry",
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
        "prependHTML",
        "prependHTMLUnsafe",
        "previousElementSibling",
        "pseudo",
        "querySelector",
        "querySelectorAll",
        "releasePointerCapture",
        "remove",
        "removeAttribute",
        "removeAttributeNS",
        "removeAttributeNode",
        "replaceChildren",
        "replaceWith",
        "replaceWithHTML",
        "replaceWithHTMLUnsafe",
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
        "setHTML",
        "setHTMLUnsafe",
        "setPointerCapture",
        "shadowRoot",
        "slot",
        "startViewTransition",
        "streamAfterHTML",
        "streamAfterHTMLUnsafe",
        "streamAppendHTML",
        "streamAppendHTMLUnsafe",
        "streamBeforeHTML",
        "streamBeforeHTMLUnsafe",
        "streamHTML",
        "streamHTMLUnsafe",
        "streamPrependHTML",
        "streamPrependHTMLUnsafe",
        "streamReplaceWithHTML",
        "streamReplaceWithHTMLUnsafe",
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
        "Element.prototype member set drifted from Chrome 157: extra {extra:?} missing {missing:?}"
    );
}

/// Chrome 157 batch: media loading, input value range, iterator helpers,
/// and the idle WebTransport face.
#[test]
fn chrome157_loading_range_iterator_transport() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const video = document.createElement('video');
        const lazy = document.createElement('video');
        lazy.loading = 'LAZY';
        const input = document.createElement('input');
        input.value = 'abcdef';
        const mid = input.createValueRange(1, 4);
        const swapped = input.createValueRange(4, 1);
        let bad = '';
        try { input.createValueRange(-1, 2); } catch (e) { bad = e.name; }
        let argc = '';
        try { input.createValueRange(0); } catch (e) { argc = e.name + ': ' + e.message; }
        const area = document.createElement('textarea');
        area.value = 'abcdef';
        const areaMid = area.createValueRange(1.9, 4);
        const areaNan = area.createValueRange(NaN, 1);
        let areaSym = '';
        try { area.createValueRange(Symbol('x'), 1); } catch (e) { areaSym = e.name + ': ' + e.message; }
        const areaNames = Object.getOwnPropertyNames(HTMLTextAreaElement.prototype);
        const inputNan = input.createValueRange(NaN, 1);
        const nums = [1, 2, 3].values();
        const wt = new WebTransport('https://example.com/wt');
        ({
          def: video.loading,
          attr: [lazy.loading, lazy.getAttribute('loading')],
          badAttr: (function () { const e = document.createElement('video'); e.loading = 'nope'; return [e.loading, e.getAttribute('loading')]; })(),
          mid: [mid.startOffset, mid.endOffset, mid.collapsed, mid.constructor.name],
          swapped: [swapped.startOffset, swapped.endOffset, swapped.collapsed],
          bad, argc,
          join: [1, 2, 3].values().join('-'),
          joinDef: [1, 2].values().join(),
          areaMid: [areaMid.startOffset, areaMid.endOffset, areaMid.collapsed],
          areaNan: [areaNan.startOffset, areaNan.endOffset],
          areaSym,
          areaAt: areaNames.slice(areaNames.indexOf('setSelectionRange'), areaNames.indexOf('setSelectionRange') + 3),
          inputNan: [inputNan.startOffset, inputNan.endOffset],
          nan: [1, NaN].values().includes(NaN),
          miss: [1].values().includes(),
          rel: wt.reliability,
          hdr: wt.responseHeaders,
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "def": "eager",
            "attr": ["lazy", "LAZY"],
            "badAttr": ["eager", "nope"],
            "mid": [1, 4, false, "OpaqueRange"],
            "swapped": [4, 4, true],
            "bad": "IndexSizeError",
            "argc": "TypeError: Failed to execute 'createValueRange' on 'HTMLInputElement': 2 arguments required, but only 1 present.",
            "join": "1-2-3",
            "joinDef": "1,2",
            "areaMid": [1, 4, false],
            "areaNan": [0, 1],
            "areaSym": "TypeError: Failed to execute 'createValueRange' on 'HTMLTextAreaElement': Cannot convert a Symbol value to a number",
            "areaAt": ["setSelectionRange", "createValueRange", "constructor"],
            "inputNan": [0, 1],
            "nan": true,
            "miss": false,
            "rel": "pending",
            "hdr": null,
        })
    );
}

/// Chrome 157 OpaqueRange geometry face. disconnect collapses the offsets.
/// The engine does not measure glyphs inside a control, so the live rect is
/// empty and the list drops to none after disconnect.
#[test]
fn chrome157_opaque_range_geometry() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const area = document.createElement('textarea');
        area.value = 'abcdef';
        const r = area.createValueRange(1, 4);
        const box = r.getBoundingClientRect();
        const list = r.getClientRects();
        const names = Object.getOwnPropertyNames(OpaqueRange.prototype);
        r.disconnect();
        const gone = r.getClientRects();
        let bad = '';
        try { OpaqueRange.prototype.disconnect.call({}); } catch (e) { bad = e.name + ': ' + e.message; }
        ({
          live: [box.constructor.name, box.width, box.height, list.constructor.name, list.length, list.item(0).width],
          off: [r.startOffset, r.endOffset, r.collapsed, gone.length, r.getBoundingClientRect().width],
          again: r.disconnect() === undefined,
          order: names,
          bad,
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "live": ["DOMRect", 0, 0, "DOMRectList", 1, 0],
            "off": [0, 0, true, 0, 0],
            "again": true,
            "order": ["disconnect", "getBoundingClientRect", "getClientRects", "constructor"],
            "bad": "TypeError: Illegal invocation",
        })
    );
}

/// Chrome 157 OffscreenCanvas geometry. clear and update accept any element.
/// Asking for a default size rejects every element.
#[test]
fn chrome157_offscreen_element_geometry() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const oc = new OffscreenCanvas(8, 8);
        const d = document.createElement('div');
        const t = document.createTextNode('t');
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const names = Object.getOwnPropertyNames(OffscreenCanvas.prototype);
        ({
          zero: err(() => oc.getElementImageDefaultSize()),
          text: err(() => oc.updateElementGeometry(t)),
          clear: err(() => oc.clearElementGeometry(d)),
          upd: err(() => oc.updateElementGeometry(d)),
          size: err(() => oc.getElementImageDefaultSize(d)),
          tail: names.slice(names.indexOf('constructor')),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "zero": "TypeError: Failed to execute 'getElementImageDefaultSize' on 'OffscreenCanvas': 1 argument required, but only 0 present.",
            "text": "TypeError: Failed to execute 'updateElementGeometry' on 'OffscreenCanvas': The provided value is not of type '(Element or ElementImage)'.",
            "clear": "undef",
            "upd": "undef",
            "size": "InvalidStateError: Failed to execute 'getElementImageDefaultSize' on 'OffscreenCanvas': Elements cannot be drawn into an OffscreenCanvas.",
            "tail": ["constructor", "clearElementGeometry", "getElementImageDefaultSize", "updateElementGeometry"],
        })
    );
}

/// Chrome 157 MediaStreamTrackProcessor counters. They stay at 0 because the
/// engine does not pull frames. The constructor still requires a track.
#[test]
fn chrome157_track_processor_frames() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const track = Object.create(MediaStreamTrack.prototype);
        const p = new MediaStreamTrackProcessor({ track });
        p.totalFrames = 5;
        const names = Object.getOwnPropertyNames(MediaStreamTrackProcessor.prototype);
        const err = (fn) => { try { fn(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } };
        ({
          vals: [p.totalFrames, p.discardedFrames],
          readable: p.readable.constructor.name,
          order: names,
          zero: err(() => new MediaStreamTrackProcessor()),
          missing: err(() => new MediaStreamTrackProcessor({})),
          bad: err(() => new MediaStreamTrackProcessor({ track: document.createElement('div') })),
          foreign: err(() => Object.getOwnPropertyDescriptor(MediaStreamTrackProcessor.prototype, 'totalFrames').get.call({})),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "vals": [0, 0],
            "readable": "ReadableStream",
            "order": ["readable", "totalFrames", "discardedFrames", "constructor"],
            "zero": "TypeError: Failed to construct 'MediaStreamTrackProcessor': 1 argument required, but only 0 present.",
            "missing": "TypeError: Failed to construct 'MediaStreamTrackProcessor': Failed to read the 'track' property from 'MediaStreamTrackProcessorInit': Required member is undefined.",
            "bad": "TypeError: Failed to construct 'MediaStreamTrackProcessor': Failed to read the 'track' property from 'MediaStreamTrackProcessorInit': Failed to convert value to 'MediaStreamTrack'.",
            "foreign": "TypeError: Illegal invocation",
        })
    );
}

/// Chrome 157 TrustedTypePolicy.createParserOptions. A policy without the
/// callback refuses. The callback sees `{runScripts}` and the result is an
/// empty TrustedHTMLParserOptions.
#[test]
fn chrome157_create_parser_options() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const seen = [];
        const p = trustedTypes.createPolicy('probe', {
          createParserOptions: function (o) { seen.push(!!o.runScripts); return null; },
        });
        const bare = trustedTypes.createPolicy('bare');
        const made = p.createParserOptions({ runScripts: 'never' });
        const err = (fn) => { try { fn(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } };
        const names = Object.getOwnPropertyNames(TrustedTypePolicy.prototype);
        ({
          seen,
          made: [made.constructor.name, Object.prototype.toString.call(made)],
          missing: err(() => bare.createParserOptions()),
          num: err(() => p.createParserOptions(1)),
          foreign: err(() => TrustedTypePolicy.prototype.createParserOptions.call({})),
          illegal: err(() => new TrustedHTMLParserOptions()),
          order: names.slice(names.indexOf('constructor')),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "seen": [true],
            "made": ["TrustedHTMLParserOptions", "[object TrustedHTMLParserOptions]"],
            "missing": "TypeError: Failed to execute 'createParserOptions' on 'TrustedTypePolicy': Policy bare's TrustedTypePolicyOptions did not specify a 'createParserOptions' member.",
            "num": "TypeError: Failed to execute 'createParserOptions' on 'TrustedTypePolicy': The provided value is not of type 'SetHTMLUnsafeOptions'.",
            "foreign": "TypeError: Illegal invocation",
            "illegal": "TypeError: Failed to construct 'TrustedHTMLParserOptions': Illegal constructor",
            "order": ["constructor", "createParserOptions"],
        })
    );
}

/// Chrome 157 animation triggers. Behaviors are none, pause, play, and reset.
/// Each Animation is kept once. ranges is one empty list.
#[test]
fn chrome157_animation_trigger() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const el = document.createElement('div');
        const a = el.animate([{ opacity: 0 }, { opacity: 1 }], 1000);
        const b = el.animate([{ opacity: 1 }, { opacity: 0 }], 800);
        const tr = new TimelineTrigger();
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        tr.addAnimation(a, 'play');
        tr.addAnimation(a, 'reset');
        tr.addAnimation(b, 'pause');
        const got = tr.getAnimations();
        tr.removeAnimation(a);
        const left = tr.getAnimations();
        const names = Object.getOwnPropertyNames(AnimationTrigger.prototype);
        const list = tr.ranges;
        ({
          got: [got.length, got[0] === a, got[1] === b, got !== tr.getAnimations()],
          left: [left.length, left[0] === b],
          rmGone: err(() => tr.removeAnimation(a)),
          div: err(() => tr.addAnimation(document.createElement('div'), 'play')),
          upper: err(() => tr.addAnimation(a, 'PLAY')),
          zero: err(() => tr.addAnimation()),
          ranges: [list === tr.ranges, list.constructor.name, list.length, list.item(0)],
          order: names,
          illegal: err(() => new AnimationTrigger()),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "got": [2, true, true, true],
            "left": [1, true],
            "rmGone": "undef",
            "div": "TypeError: Failed to execute 'addAnimation' on 'AnimationTrigger': parameter 1 is not of type 'Animation'.",
            "upper": "TypeError: Failed to execute 'addAnimation' on 'AnimationTrigger': The provided value 'PLAY' is not a valid enum value of type AnimationTriggerBehavior.",
            "zero": "TypeError: Failed to execute 'addAnimation' on 'AnimationTrigger': 2 arguments required, but only 0 present.",
            "ranges": [true, "TimelineTriggerRangeList", 0, null],
            "order": ["addAnimation", "getAnimations", "removeAnimation", "constructor"],
            "illegal": "TypeError: Failed to construct 'AnimationTrigger': Illegal constructor",
        })
    );
}

/// Chrome 157 WebGPU immediate-data entry points. Two arguments are required.
/// A full call returns undefined; the bytes are not uploaded.
#[test]
fn chrome157_gpu_immediates() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const pass = Object.create(GPUComputePassEncoder.prototype);
        const names = Object.getOwnPropertyNames(GPUComputePassEncoder.prototype);
        const qnames = Object.getOwnPropertyNames(GPUQueue.prototype);
        const tex = Object.create(GPUTexture.prototype);
        ({
          zero: err(() => pass.setImmediates()),
          one: err(() => pass.setImmediates(0)),
          ok: err(() => pass.setImmediates(0, new Uint32Array([1]))),
          bad: err(() => GPUComputePassEncoder.prototype.setImmediates.call({}, 0, new Uint32Array([1]))),
          at: names.slice(names.indexOf('pushDebugGroup'), names.indexOf('dispatchWorkgroups') + 1),
          q: qnames.slice(-2),
          dim: err(() => tex.textureBindingViewDimension),
          lim: Object.getOwnPropertyNames(GPUSupportedLimits.prototype).slice(-6),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "zero": "TypeError: Failed to execute 'setImmediates' on 'GPUComputePassEncoder': 2 arguments required, but only 0 present.",
            "one": "TypeError: Failed to execute 'setImmediates' on 'GPUComputePassEncoder': 2 arguments required, but only 1 present.",
            "ok": "undef",
            "bad": "TypeError: Illegal invocation",
            "at": ["pushDebugGroup", "setImmediates", "dispatchWorkgroups"],
            "q": ["constructor", "drawElementImageToTexture"],
            "dim": "undef",
            "lim": ["maxImmediateSize", "maxStorageBuffersInFragmentStage", "maxStorageTexturesInFragmentStage", "maxStorageBuffersInVertexStage", "maxStorageTexturesInVertexStage", "constructor"],
        })
    );
}

/// Chrome 157 interaction paint and soft navigation. Nothing records an
/// interaction, so the counters stay at rest and the largest paint is null.
#[test]
fn chrome157_interaction_paint() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : (r === null ? 'null' : String(r)); } catch (e) { return e.name + ': ' + e.message; } };
        const icp = Object.create(InteractionContentfulPaint.prototype);
        const soft = Object.create(PerformanceSoftNavigation.prototype);
        ({
          icp: [icp.largestContentfulPaint, icp.interactionId, icp.paintTime, icp.presentationTime],
          json: (() => { const j = icp.toJSON(); return { largestContentfulPaint: j.largestContentfulPaint, interactionId: j.interactionId, paintTime: j.paintTime, presentationTime: j.presentationTime, navigationId: j.navigationId }; })(),
          soft: [soft.navigationType, soft.interactionId, soft.paintTime, soft.presentationTime, soft.getLargestInteractionContentfulPaint()],
          bad: err(() => Object.getOwnPropertyDescriptor(InteractionContentfulPaint.prototype, 'paintTime').get.call({})),
          order: Object.getOwnPropertyNames(InteractionContentfulPaint.prototype),
          softOrder: Object.getOwnPropertyNames(PerformanceSoftNavigation.prototype),
          illegal: err(() => new InteractionContentfulPaint()),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "icp": [0, 0, 0, null],
            "json": {
                "largestContentfulPaint": 0,
                "interactionId": 0,
                "paintTime": 0,
                "presentationTime": null,
                "navigationId": 1,
            },
            "soft": ["", 0, 0, null, null],
            "bad": "TypeError: Illegal invocation",
            "order": ["largestContentfulPaint", "interactionId", "paintTime", "presentationTime", "toJSON", "constructor"],
            "softOrder": ["navigationType", "interactionId", "paintTime", "presentationTime", "getLargestInteractionContentfulPaint", "constructor"],
            "illegal": "TypeError: Failed to construct 'InteractionContentfulPaint': Illegal constructor",
        })
    );
}

/// Chrome 157 TimelineTriggerRange. The constructor is illegal and a forged
/// prototype object is not a live range.
#[test]
fn chrome157_timeline_trigger_range() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { fn(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } };
        const forged = Object.create(TimelineTriggerRange.prototype);
        ({
          order: Object.getOwnPropertyNames(TimelineTriggerRange.prototype),
          illegal: err(() => new TimelineTriggerRange()),
          forged: err(() => forged.timeline),
          bad: err(() => Object.getOwnPropertyDescriptor(TimelineTriggerRange.prototype, 'activationRangeStart').get.call({})),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "order": ["timeline", "activationRangeStart", "activationRangeEnd", "activeRangeStart", "activeRangeEnd", "constructor"],
            "illegal": "TypeError: Failed to construct 'TimelineTriggerRange': Illegal constructor",
            "forged": "TypeError: Illegal invocation",
            "bad": "TypeError: Illegal invocation",
        })
    );
}

/// Chrome 157 camera, microphone, and usermedia elements. setConstraints
/// checks the dictionary and does not open a device.
#[test]
fn chrome157_user_media_elements() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const cam = document.createElement('camera');
        const mic = document.createElement('microphone');
        const um = document.createElement('usermedia');
        cam.onerror = 'nope';
        const handler = cam.onerror;
        cam.onerror = function () {};
        ({
          names: [cam.constructor.name, mic.constructor.name, um.constructor.name],
          rest: [cam.error, cam.track, mic.track, um.stream, um.error],
          camOk: err(() => cam.setConstraints({ video: true })),
          camBad: err(() => cam.setConstraints(1)),
          umOk: err(() => um.setConstraints({ video: {} })),
          umBool: err(() => um.setConstraints({ video: true })),
          umNum: err(() => um.setConstraints(1)),
          handler, fn: typeof cam.onerror,
          order: Object.getOwnPropertyNames(HTMLCameraElement.prototype),
          umOrder: Object.getOwnPropertyNames(HTMLUserMediaElement.prototype),
          illegal: err(() => new HTMLCameraElement()),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "names": ["HTMLCameraElement", "HTMLMicrophoneElement", "HTMLUserMediaElement"],
            "rest": [null, null, null, null, null],
            "camOk": "undef",
            "camBad": "TypeError: Failed to execute 'setConstraints' on 'HTMLCameraElement': The provided value is not of type 'MediaTrackConstraintSet'.",
            "umOk": "undef",
            "umBool": "TypeError: Failed to execute 'setConstraints' on 'HTMLUserMediaElement': Failed to read the 'video' property from 'HTMLMediaStreamConstraints': The provided value is not of type 'MediaTrackConstraintSet'.",
            "umNum": "TypeError: Failed to execute 'setConstraints' on 'HTMLUserMediaElement': The provided value is not of type 'HTMLMediaStreamConstraints'.",
            "handler": null,
            "fn": "function",
            "order": ["error", "oncancel", "onerror", "ontrack", "track", "setConstraints", "constructor"],
            "umOrder": ["onstream", "stream", "error", "oncancel", "onerror", "setConstraints", "constructor"],
            "illegal": "TypeError: Failed to construct 'HTMLCameraElement': Illegal constructor",
        })
    );
}

/// Chrome 157 XR members. No session is live, so the new getters reject
/// every receiver. initiateRoomCapture returns a rejected promise.
#[test]
fn chrome157_xr_members() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const room = Object.create(XRSession.prototype).initiateRoomCapture();
        room.catch(() => {});
        const frame = Object.getOwnPropertyNames(XRFrame.prototype);
        const session = Object.getOwnPropertyNames(XRSession.prototype);
        ({
          planes: err(() => Object.create(XRFrame.prototype).detectedPlanes),
          layer: err(() => Object.create(XRLayerEvent.prototype).layer),
          layers: err(() => Object.create(XRRenderState.prototype).layers),
          view: err(() => Object.create(XRSubImage.prototype).viewport),
          max: err(() => Object.create(XRSession.prototype).maxRenderLayers),
          room: room instanceof Promise,
          frameTail: frame.slice(frame.indexOf('constructor')),
          layerOrder: Object.getOwnPropertyNames(XRLayerEvent.prototype),
          subOrder: Object.getOwnPropertyNames(XRSubImage.prototype),
          sessionAt: session.slice(session.indexOf('enabledFeatures'), session.indexOf('onvisibilitymaskchange') + 1).concat(session.slice(-1)),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "planes": "TypeError: Illegal invocation",
            "layer": "TypeError: Illegal invocation",
            "layers": "TypeError: Illegal invocation",
            "view": "TypeError: Illegal invocation",
            "max": "TypeError: Illegal invocation",
            "room": true,
            "frameTail": ["constructor", "detectedPlanes"],
            "layerOrder": ["layer", "constructor"],
            "subOrder": ["viewport", "constructor"],
            "sessionAt": ["enabledFeatures", "maxRenderLayers", "onvisibilitymaskchange", "initiateRoomCapture"],
        })
    );
}

/// Chrome 157 XR layers and planes. Reads and writes fail the brand check
/// before converting the assigned value. Plane attributes have no setter.
#[test]
fn chrome157_xr_layers() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const cube = Object.create(XRCubeLayer.prototype);
        const plane = Object.create(XRPlane.prototype);
        ({
          get: err(() => cube.space),
          set: err(() => { cube.orientation = Symbol('x'); }),
          plane: err(() => plane.semanticLabel),
          assign: err(() => { plane.orientation = 'horizontal'; return plane.orientation; }),
          cubeOrder: Object.getOwnPropertyNames(XRCubeLayer.prototype),
          quadOrder: Object.getOwnPropertyNames(XRQuadLayer.prototype),
          cylOrder: Object.getOwnPropertyNames(XRCylinderLayer.prototype),
          planeOrder: Object.getOwnPropertyNames(XRPlane.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "get": "TypeError: Illegal invocation",
            "set": "TypeError: Illegal invocation",
            "plane": "TypeError: Illegal invocation",
            "assign": "TypeError: Illegal invocation",
            "cubeOrder": ["space", "orientation", "onredraw", "constructor"],
            "quadOrder": ["space", "transform", "width", "height", "onredraw", "constructor"],
            "cylOrder": ["space", "transform", "radius", "centralAngle", "aspectRatio", "onredraw", "constructor"],
            "planeOrder": ["planeSpace", "polygon", "orientation", "lastChangedTime", "semanticLabel", "constructor"],
        })
    );
}

/// Chrome 157 plane set and projection layer. Brand check wins over arity,
/// and the read-only texture sizes have no setter.
#[test]
fn chrome157_xr_plane_set() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const set = Object.create(XRPlaneSet.prototype);
        const layer = Object.create(XRProjectionLayer.prototype);
        ({
          has: err(() => set.has()),
          size: err(() => set.size),
          width: err(() => layer.textureWidth),
          fov: err(() => { layer.fixedFoveation = Symbol('x'); }),
          assign: err(() => { layer.textureWidth = 4; return layer.textureWidth; }),
          setOrder: Object.getOwnPropertyNames(XRPlaneSet.prototype),
          layerOrder: Object.getOwnPropertyNames(XRProjectionLayer.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "has": "TypeError: Illegal invocation",
            "size": "TypeError: Illegal invocation",
            "width": "TypeError: Illegal invocation",
            "fov": "TypeError: Illegal invocation",
            "assign": "TypeError: Illegal invocation",
            "setOrder": ["size", "entries", "forEach", "has", "keys", "values", "constructor"],
            "layerOrder": ["textureWidth", "textureHeight", "textureArrayLength", "ignoreDepthValues", "fixedFoveation", "deltaPose", "constructor"],
        })
    );
}

/// Chrome 157 composition and equirect layers. Brand check runs before
/// value conversion. layout, mipLevels, and needsRedraw are read-only.
#[test]
fn chrome157_xr_composition_layer() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const comp = Object.create(XRCompositionLayer.prototype);
        const eq = Object.create(XREquirectLayer.prototype);
        ({
          layout: err(() => comp.layout),
          opacity: err(() => { comp.opacity = Symbol('x'); }),
          destroy: err(() => comp.destroy()),
          space: err(() => eq.space),
          angle: err(() => { eq.radius = Symbol('x'); }),
          compOrder: Object.getOwnPropertyNames(XRCompositionLayer.prototype),
          eqOrder: Object.getOwnPropertyNames(XREquirectLayer.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "layout": "TypeError: Illegal invocation",
            "opacity": "TypeError: Illegal invocation",
            "destroy": "TypeError: Illegal invocation",
            "space": "TypeError: Illegal invocation",
            "angle": "TypeError: Illegal invocation",
            "compOrder": ["layout", "blendTextureSourceAlpha", "forceMonoPresentation", "opacity", "mipLevels", "needsRedraw", "destroy", "constructor"],
            "eqOrder": ["space", "transform", "radius", "centralHorizontalAngle", "upperVerticalAngle", "lowerVerticalAngle", "onredraw", "constructor"],
        })
    );
}

/// Chrome 157 NavigationPrecommitController.addHandler. Brand check wins
/// over the argument count.
#[test]
fn chrome157_precommit_handler() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { fn(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } };
        const o = Object.create(NavigationPrecommitController.prototype);
        ({
          zero: err(() => o.addHandler()),
          fn: err(() => o.addHandler(function () {})),
          bad: err(() => NavigationPrecommitController.prototype.addHandler.call({}, function () {})),
          len: NavigationPrecommitController.prototype.addHandler.length,
          order: Object.getOwnPropertyNames(NavigationPrecommitController.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "zero": "TypeError: Illegal invocation",
            "fn": "TypeError: Illegal invocation",
            "bad": "TypeError: Illegal invocation",
            "len": 1,
            "order": ["addHandler", "redirect", "constructor"],
        })
    );
}

/// Chrome 157 XSLTProcessor. Parameters round-trip. A single root template
/// of elements and value-of is applied; other stylesheets yield null.
#[test]
fn chrome157_xslt_processor() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : (r === null ? 'null' : String(r)); } catch (e) { return e.name + ': ' + e.message; } };
        const p = new XSLTProcessor();
        const doc = document.createElement('a');
        doc.textContent = 'hi';
        const sheet = document.createElement('stylesheet');
        const tmpl = document.createElement('template');
        const outEl = document.createElement('out');
        const valueOf = document.createElement('value-of');
        valueOf.setAttribute('select', 'a');
        outEl.appendChild(valueOf);
        (tmpl.content || tmpl).appendChild(outEl);
        sheet.appendChild(tmpl);
        const before = p.transformToDocument(doc);
        p.importStylesheet(sheet);
        const made = p.transformToDocument(doc);
        const frag = p.transformToFragment(doc, document);
        p.setParameter(null, 'x', 'v');
        p.setParameter('', 'y', 2);
        const got = [p.getParameter(null, 'x'), p.getParameter('', 'y'), p.getParameter(null, 'missing')];
        p.removeParameter(null, 'x');
        p.clearParameters();
        p.reset();
        ({
          before: before === null,
          tag: made && made.documentElement && made.documentElement.localName,
          text: made && made.documentElement && made.documentElement.textContent,
          frag: frag && frag.constructor.name,
          fragText: frag && frag.textContent,
          got,
          removed: p.getParameter(null, 'x'),
          cleared: p.getParameter('', 'y'),
          afterReset: p.transformToDocument(doc) === null,
          imp0: err(() => p.importStylesheet()),
          impNull: err(() => p.importStylesheet(null)),
          bad: err(() => XSLTProcessor.prototype.getParameter.call({}, null, 'x')),
          order: Object.getOwnPropertyNames(XSLTProcessor.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "before": true,
            "tag": "out",
            "text": "hi",
            "frag": "DocumentFragment",
            "fragText": "hi",
            "got": ["v", 2, null],
            "removed": null,
            "cleared": null,
            "afterReset": true,
            "imp0": "TypeError: Failed to execute 'importStylesheet' on 'XSLTProcessor': 1 argument required, but only 0 present.",
            "impNull": "TypeError: Failed to execute 'importStylesheet' on 'XSLTProcessor': parameter 1 is not of type 'Node'.",
            "bad": "TypeError: Illegal invocation",
            "order": ["clearParameters", "getParameter", "importStylesheet", "removeParameter", "reset", "setParameter", "transformToDocument", "transformToFragment", "constructor"],
        })
    );
}

/// Chrome 157 XRWebGLBinding. Brand check wins over arity. Subimage
/// attributes are read-only and likewise branded.
#[test]
fn chrome157_xr_webgl_binding() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const b = Object.create(XRWebGLBinding.prototype);
        const s = Object.create(XRWebGLSubImage.prototype);
        ({
          scale: err(() => b.nativeProjectionScaleFactor),
          cube: err(() => b.createCubeLayer()),
          proj: err(() => b.createProjectionLayer()),
          sub: err(() => b.getSubImage()),
          len: [b.createQuadLayer.length, b.getViewSubImage.length],
          tex: err(() => s.colorTexture),
          order: Object.getOwnPropertyNames(XRWebGLBinding.prototype),
          subOrder: Object.getOwnPropertyNames(XRWebGLSubImage.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "scale": "TypeError: Illegal invocation",
            "cube": "TypeError: Illegal invocation",
            "proj": "TypeError: Illegal invocation",
            "sub": "TypeError: Illegal invocation",
            "len": [1, 2],
            "tex": "TypeError: Illegal invocation",
            "order": ["nativeProjectionScaleFactor", "usesDepthValues", "createCubeLayer", "createCylinderLayer", "createEquirectLayer", "createProjectionLayer", "createQuadLayer", "getSubImage", "getViewSubImage", "getCameraImage", "getDepthInformation", "getReflectionCubeMap", "constructor"],
            "subOrder": ["colorTexture", "depthStencilTexture", "motionVectorTexture", "imageIndex", "colorTextureWidth", "colorTextureHeight", "depthStencilTextureWidth", "depthStencilTextureHeight", "motionVectorTextureWidth", "motionVectorTextureHeight", "constructor"],
        })
    );
}

/// Chrome 157 LanguageModel. No session is live. promptStreaming throws
/// immediately; prompt returns a rejected promise.
#[test]
fn chrome157_language_model() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const o = Object.create(LanguageModel.prototype);
        const prompt = o.prompt('hi');
        prompt.catch(() => {});
        ({
          usage: err(() => o.contextUsage),
          handler: err(() => { o.oncontextoverflow = Symbol('x'); }),
          destroy: err(() => o.destroy()),
          stream: err(() => o.promptStreaming('hi')),
          prompt: prompt instanceof Promise,
          lens: [o.append.length, o.clone.length, o.measureContextUsage.length, o.prompt.length, o.promptStreaming.length],
          order: Object.getOwnPropertyNames(LanguageModel.prototype),
          illegal: err(() => new LanguageModel()),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "usage": "TypeError: Illegal invocation",
            "handler": "TypeError: Illegal invocation",
            "destroy": "TypeError: Illegal invocation",
            "stream": "TypeError: Illegal invocation",
            "prompt": true,
            "lens": [1, 0, 1, 1, 1],
            "order": ["contextUsage", "contextWindow", "oncontextoverflow", "append", "clone", "destroy", "measureContextUsage", "prompt", "promptStreaming", "constructor"],
            "illegal": "TypeError: Failed to construct 'LanguageModel': Illegal constructor",
        })
    );
}

/// Chrome 157 paint and geometry events. The element lists are empty.
/// ElementImage stays illegal to construct.
#[test]
fn chrome157_paint_events() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const paint = new CanvasPaintEvent('paint');
        const geo = new ElementGeometryUpdateEvent(1);
        ({
          paint: [paint.type, paint.changedElements.constructor.name, paint.changedElements.length, Object.prototype.toString.call(paint)],
          geo: [geo.type, geo.elements.length, Object.prototype.toString.call(geo)],
          zero: err(() => new CanvasPaintEvent()),
          forged: err(() => Object.create(CanvasPaintEvent.prototype).changedElements),
          img: err(() => new ElementImage()),
          close: err(() => Object.create(ElementImage.prototype).close()),
          order: Object.getOwnPropertyNames(CanvasPaintEvent.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "paint": ["paint", "Array", 0, "[object CanvasPaintEvent]"],
            "geo": ["1", 0, "[object ElementGeometryUpdateEvent]"],
            "zero": "TypeError: Failed to construct 'CanvasPaintEvent': 1 argument required, but only 0 present.",
            "forged": "TypeError: Illegal invocation",
            "img": "TypeError: Failed to construct 'ElementImage': Illegal constructor",
            "close": "TypeError: Illegal invocation",
            "order": ["changedElements", "constructor"],
        })
    );
}

/// Chrome 157 Sanitizer. The default allow-list is Chrome's 121 elements
/// and 58 attributes. parseHTML and setHTML read a Sanitizer passed in options.
#[test]
fn chrome157_sanitizer() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const s = new Sanitizer();
        const g = s.get();
        const anchor = g.elements.filter((e) => e.name === 'a' && e.namespace === 'http://www.w3.org/1999/xhtml')[0];
        s.allowElement('custom-x');
        s.removeElement('p');
        s.setComments(true);
        s.setDataAttributes(true);
        s.allowProcessingInstruction('xml');
        const after = s.get();
        s.removeProcessingInstruction('xml');
        ({
          counts: [g.elements.length, g.attributes.length, g.comments, g.dataAttributes, g.processingInstructions.length],
          hasP: g.elements.some((e) => e.name === 'p'),
          hasScript: g.elements.some((e) => e.name === 'script'),
          href: anchor && anchor.attributes.map((a) => a.name),
          custom: after.elements.some((e) => e.name === 'custom-x'),
          dropped: after.elements.some((e) => e.name === 'p'),
          flags: [after.comments, after.dataAttributes, after.processingInstructions.length, s.get().processingInstructions.length],
          zero: err(() => s.allowElement()),
          bad: err(() => Sanitizer.prototype.get.call({})),
          order: Object.getOwnPropertyNames(Sanitizer.prototype),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "counts": [121, 58, false, false, 0],
            "hasP": true,
            "hasScript": false,
            "href": ["href", "hreflang", "type"],
            "custom": true,
            "dropped": false,
            "flags": [true, true, 1, 0],
            "zero": "TypeError: Failed to execute 'allowElement' on 'Sanitizer': 1 argument required, but only 0 present.",
            "bad": "TypeError: Illegal invocation",
            "order": ["allowAttribute", "allowElement", "get", "removeAttribute", "removeElement", "removeUnsafe", "replaceElementWithChildren", "setComments", "setDataAttributes", "allowProcessingInstruction", "removeProcessingInstruction", "constructor"],
        })
    );
}

/// Chrome 157 Document.parseHTML builds an HTML document and runs the
/// Sanitizer. setHTML reads the same options. script stays blocked even
/// after allowElement. A disallowed element drops its subtree; replace
/// unwraps it.
#[test]
fn chrome157_parse_html() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        function body(doc) {
          return Array.from(doc.body.childNodes).map(c => c.nodeType === 8 ? '<!--' + c.data + '-->' : c.nodeType === 3 ? '#' + c.data : c.localName + '[' + [...c.attributes].map(a => a.name + '=' + a.value).join(',') + ']').join(',');
        }
        const err = (fn) => { try { fn(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } };
        const html = '<b onclick="x">t</b><script>1<\/script><a href="javascript:alert(1)">a</a><a href="https://e.example/a" title="T">z</a>';
        const doc = Document.parseHTML(html);
        const s = new Sanitizer();
        s.removeElement('b');
        const dropped = Document.parseHTML('<div><b>t</b>x</div>', { sanitizer: s });
        const r = new Sanitizer();
        r.replaceElementWithChildren('font');
        const unwrapped = Document.parseHTML('<div><font><b>t</b></font>x</div>', { sanitizer: r });
        const c = new Sanitizer();
        c.setComments(true);
        const commented = Document.parseHTML('<b>t</b><!--in-->', { sanitizer: c });
        const plain = Document.parseHTML('<b>t</b><!--in-->');
        const sc = new Sanitizer();
        sc.allowElement('script');
        const still = Document.parseHTML('<div><script>1<\/script><b>t</b></div>', { sanitizer: sc });
        const el = document.createElement('div');
        const ret = el.setHTML('<b onclick="x">t</b><p>p</p>', { sanitizer: s });
        ({
          zero: err(() => Document.parseHTML()),
          num: err(() => Document.parseHTML('<b>t</b>', 1)),
          bad: err(() => document.createElement('div').setHTML('<b>t</b>', { sanitizer: 1 })),
          ret: ret === undefined,
          safe: body(doc),
          doctype: doc.doctype,
          same: doc === document,
          title: doc.querySelector('a[href]').getAttribute('title'),
          dropped: dropped.body.firstChild.innerHTML,
          unwrapped: unwrapped.body.firstChild.innerHTML,
          commented: body(commented),
          plain: body(plain),
          still: still.body.firstChild.innerHTML,
          set: el.innerHTML.replace(/></g, '> <'),
          len: Document.parseHTML.length,
          preset: body(Document.parseHTML('<b onclick="x">t</b><script>1<\/script>', { sanitizer: 'default' })),
          rep: r.get().replaceWithChildrenElements.map(e => e.name),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "zero": "TypeError: Failed to execute 'parseHTML' on 'Document': 1 argument required, but only 0 present.",
            "num": "TypeError: Failed to execute 'parseHTML' on 'Document': The provided value is not of type 'SetHTMLOptions'.",
            "bad": "TypeError: Failed to execute 'setHTML' on 'Element': Failed to read the 'sanitizer' property from 'SetHTMLOptions': The provided value '1' is not a valid enum value of type SanitizerPresets.",
            "ret": true,
            "safe": "b[],a[],a[href=https://e.example/a,title=T]",
            "doctype": null,
            "same": false,
            "title": "T",
            "dropped": "x",
            "unwrapped": "<b>t</b>x",
            "commented": "b[],<!--in-->",
            "plain": "b[]",
            "still": "<b>t</b>",
            "set": "<p>p</p>",
            "len": 1,
            "preset": "b[]",
            "rep": ["font"],
        })
    );
}

/// Chrome 157 RTCPeerConnection diagnostic logging is one process-wide
/// session. start returns a UUID. A second start, or stop/cancel with no
/// session, is InvalidStateError. A non-object options value is a TypeError.
#[test]
fn chrome157_rtc_diagnostic_logging() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        const none = err(() => RTCPeerConnection.stopDiagnosticLogging());
        const id = RTCPeerConnection.startDiagnosticLogging();
        const again = err(() => RTCPeerConnection.startDiagnosticLogging({}));
        const stopped = err(() => RTCPeerConnection.stopDiagnosticLogging());
        const after = err(() => RTCPeerConnection.cancelDiagnosticLogging());
        const id2 = RTCPeerConnection.startDiagnosticLogging(null);
        RTCPeerConnection.cancelDiagnosticLogging();
        const bad = err(() => RTCPeerConnection.startDiagnosticLogging(1));
        const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
        ({
          none, again, stopped, after, bad,
          id: uuid.test(id),
          id2: uuid.test(id2) && id2 !== id,
          lens: [
            RTCPeerConnection.startDiagnosticLogging.length,
            RTCPeerConnection.stopDiagnosticLogging.length,
            RTCPeerConnection.cancelDiagnosticLogging.length,
          ],
          order: Object.getOwnPropertyNames(RTCPeerConnection).filter(n => n !== 'length' && n !== 'name' && n !== 'prototype'),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "none": "InvalidStateError: Failed to execute 'stopDiagnosticLogging' on 'RTCPeerConnection': No active diagnostic logging session.",
            "again": "InvalidStateError: Failed to execute 'startDiagnosticLogging' on 'RTCPeerConnection': A diagnostic logging session is already active.",
            "stopped": "undef",
            "after": "InvalidStateError: Failed to execute 'cancelDiagnosticLogging' on 'RTCPeerConnection': No active diagnostic logging session.",
            "bad": "TypeError: Failed to execute 'startDiagnosticLogging' on 'RTCPeerConnection': The provided value is not of type 'RTCDiagnosticLoggingOptions'.",
            "id": true,
            "id2": true,
            "lens": [0, 0, 0],
            "order": ["generateCertificate", "cancelDiagnosticLogging", "startDiagnosticLogging", "stopDiagnosticLogging"],
        })
    );
}

/// Chrome 157 Iterator.concat / zip / zipKeyed. concat rejects primitives,
/// including strings. zip defaults to the shortest input. longest fills a
/// finished side with undefined, and the key stays on the result. strict
/// refuses unequal lengths. zipKeyed results have a null prototype.
#[test]
fn chrome157_iterator_zip() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const err = (fn) => { try { return JSON.stringify([...fn()]); } catch (e) { return e.name + ': ' + e.message; } };
        const cat = Iterator.concat([1], [2, 3].values());
        const hole = [...Iterator.zip([[1], ['a', 'b']], { mode: 'longest' })][1];
        const keyedHole = [...Iterator.zipKeyed({ a: [1], b: ['x', 'y'] }, { mode: 'longest' })][1];
        const face = Object.getOwnPropertyDescriptor(Iterator, 'zip');
        ({
          cat: [...cat.map((x) => x + 1)],
          empty: [...Iterator.concat()],
          prim: err(() => Iterator.concat(1)),
          str: err(() => Iterator.concat('ab')),
          plain: err(() => Iterator.concat({ a: 1 })),
          zip: [...Iterator.zip([[1, 2, 3], ['a', 'b']])],
          hole: [hole[0] === undefined ? 'undef' : String(hole[0]), hole[1]],
          pad: [...Iterator.zip([[1], ['a', 'b']], { mode: 'longest', padding: ['x', 'y'] })],
          strict: err(() => Iterator.zip([[1, 2], ['a']], { mode: 'strict' })),
          mode: err(() => Iterator.zip([[1]], { mode: 'Shortest' })),
          opt: err(() => Iterator.zip([[1]], null)),
          keyed: [...Iterator.zipKeyed({ z: [1], a: [2, 3] })],
          keyedLong: [...Iterator.zipKeyed({ a: [1], b: ['x', 'y'] }, { mode: 'longest', padding: { a: 'A' } })],
          keyedHole: ['a' in keyedHole, keyedHole.a === undefined, keyedHole.b, Object.getPrototypeOf(keyedHole) === null],
          lens: [Iterator.concat.length, Iterator.zip.length, Iterator.zipKeyed.length],
          tag: Object.prototype.toString.call(Iterator.concat([1])),
          face: [face.enumerable, face.writable, face.configurable],
          order: Object.getOwnPropertyNames(Iterator).filter((n) => n !== 'length' && n !== 'name' && n !== 'prototype'),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "cat": [2, 3, 4],
            "empty": [],
            "prim": "TypeError: Iterator.concat called on non-object",
            "str": "TypeError: Iterator.concat called on non-object",
            "plain": "TypeError: #<Object> is not iterable",
            "zip": [[1, "a"], [2, "b"]],
            "hole": ["undef", "b"],
            "pad": [[1, "a"], ["x", "b"]],
            "strict": "TypeError: Iterators passed to Iterator.zip in 'strict' mode must have the same length",
            "mode": "TypeError: Iterator.zip mode must be 'shortest', 'longest', or 'strict'",
            "opt": "TypeError: options called on non-object",
            "keyed": [{"z": 1, "a": 2}],
            "keyedLong": [{"a": 1, "b": "x"}, {"a": "A", "b": "y"}],
            "keyedHole": [true, true, "y", true],
            "lens": [0, 1, 1],
            "tag": "[object Iterator Helper]",
            "face": [false, true, true],
            "order": ["from", "concat", "zip", "zipKeyed"],
        })
    );
}

/// Chrome 157 datagram caps. 0 and non-finite numbers are stored as 1.
/// A negative value wraps as an unsigned long.
#[test]
fn chrome157_datagram_caps() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const d = new WebTransport('https://example.com/wt').datagrams;
        const def = [d.incomingMaxBufferedDatagrams, d.outgoingMaxBufferedDatagrams];
        d.incomingMaxBufferedDatagrams = -1;
        const neg = d.incomingMaxBufferedDatagrams;
        d.outgoingMaxBufferedDatagrams = 0;
        const zero = d.outgoingMaxBufferedDatagrams;
        d.incomingMaxBufferedDatagrams = 2.9;
        const frac = d.incomingMaxBufferedDatagrams;
        d.incomingMaxBufferedDatagrams = NaN;
        const nan = d.incomingMaxBufferedDatagrams;
        let sym = '';
        try { d.incomingMaxBufferedDatagrams = Symbol('x'); } catch (e) { sym = e.name + ': ' + e.message; }
        const names = Object.getOwnPropertyNames(WebTransportDatagramDuplexStream.prototype);
        let bad = '';
        try { Object.getOwnPropertyDescriptor(WebTransportDatagramDuplexStream.prototype, 'incomingMaxBufferedDatagrams').get.call({}); } catch (e) { bad = e.name + ': ' + e.message; }
        ({
          def, neg, zero, frac, nan, sym, bad,
          isa: d instanceof WebTransportDatagramDuplexStream,
          streams: [d.readable.constructor.name, d.writable.constructor.name],
          at: names.slice(names.indexOf('outgoingMaxAge'), names.indexOf('incomingHighWaterMark') + 1),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "def": [1, 1],
            "neg": 4294967295u64,
            "zero": 1,
            "frac": 2,
            "nan": 1,
            "sym": "TypeError: Failed to set the 'incomingMaxBufferedDatagrams' property on 'WebTransportDatagramDuplexStream': Cannot convert a Symbol value to a number",
            "bad": "TypeError: Illegal invocation",
            "isa": true,
            "streams": ["ReadableStream", "WritableStream"],
            "at": ["outgoingMaxAge", "incomingMaxBufferedDatagrams", "outgoingMaxBufferedDatagrams", "incomingHighWaterMark"],
        })
    );
}

/// Chrome 157 element-image canvas face. The engine does not paint elements,
/// so a call that needs a cached paint record throws. Transforms that pass
/// the drawable checks return an identity matrix.
#[test]
fn chrome157_element_image_canvas() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const c = document.createElement('canvas');
        const d = document.createElement('div');
        const t = document.createTextNode('t');
        const ctx = c.getContext('2d');
        const err = (fn) => { try { const r = fn(); return r === undefined ? 'undef' : String(r); } catch (e) { return e.name + ': ' + e.message; } };
        c.content = 'paint';
        const painted = [c.content, c.getAttribute('content')];
        c.content = null;
        const nulled = [c.content, c.getAttribute('content')];
        c.removeAttribute('content');
        c.onpaint = 'nope';
        const handler = c.onpaint;
        c.onpaint = function () {};
        const bare = {
          size: err(() => c.getElementImageDefaultSize(d)),
          xfText: err(() => c.getElementTransform(t)),
          upd: err(() => c.updateElementGeometry(d)),
          clear: err(() => c.clearElementGeometry(d)),
          clearText: err(() => c.clearElementGeometry(t)),
          paint: err(() => c.requestPaint()),
          cap0: err(() => c.captureElementImage()),
          draw0: err(() => ctx.drawElementImage()),
          drawText: err(() => ctx.drawElementImage(t, 0, 0)),
          drawSym: err(() => ctx.drawElementImage(d, Symbol('x'), 0)),
        };
        c.setAttribute('content', 'drawable');
        const needAttr = err(() => c.getElementTransform(d));
        d.setAttribute('drawable', '');
        document.body.appendChild(d);
        const outside = err(() => c.getElementImageDefaultSize(d));
        c.appendChild(d);
        const m = c.getElementTransform(d);
        const inner = document.createElement('canvas');
        const nested = document.createElement('div');
        nested.setAttribute('drawable', '');
        inner.appendChild(nested);
        c.appendChild(inner);
        const gl = document.createElement('canvas').getContext('webgl');
        const gl2 = document.createElement('canvas').getContext('webgl2');
        const names = Object.getOwnPropertyNames(HTMLCanvasElement.prototype);
        const ctxNames = Object.getOwnPropertyNames(CanvasRenderingContext2D.prototype);
        const offNames = Object.getOwnPropertyNames(OffscreenCanvasRenderingContext2D.prototype);
        const glNames = Object.getOwnPropertyNames(WebGLRenderingContext.prototype);
        ({
          def: document.createElement('canvas').content,
          painted, nulled, handler, handlerFn: typeof c.onpaint,
          bare, needAttr, outside,
          xf: [m.constructor.name, String(m), m.isIdentity],
          upd: err(() => c.updateElementGeometry(d)),
          size: err(() => c.getElementImageDefaultSize(d)),
          cap: err(() => c.captureElementImage(d)),
          draw: err(() => ctx.drawElementImage(d, 1, 2)),
          nested: err(() => c.getElementImageDefaultSize(nested)),
          tail: names.slice(names.indexOf('constructor')),
          drawTail: ctxNames.slice(-2),
          offTail: offNames.slice(-3),
          off: err(() => OffscreenCanvasRenderingContext2D.prototype.drawElementImage.call({}, d, 0, 0)),
          gl0: err(() => gl.texElementSubImage2D()),
          gl2_0: err(() => gl2.texElementSubImage2D()),
          glText: err(() => gl.texElementSubImage2D(gl.TEXTURE_2D, 0, 0, 0, t)),
          glOk: err(() => gl.texElementSubImage2D(gl.TEXTURE_2D, 0, 0, 0, d)),
          glForeign: err(() => WebGLRenderingContext.prototype.texElementSubImage2D.call({})),
          glTail: glNames.slice(-3),
          glLen: gl.texElementSubImage2D.length,
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "def": "fallback",
            "painted": ["fallback", "paint"],
            "nulled": ["fallback", "null"],
            "handler": null,
            "handlerFn": "function",
            "bare": {
                "size": "InvalidStateError: Failed to execute 'getElementImageDefaultSize' on 'HTMLCanvasElement': getElementImageDefaultSize requires the canvas to have the content=drawable attribute.",
                "xfText": "TypeError: Failed to execute 'getElementTransform' on 'HTMLCanvasElement': parameter 1 is not of type 'Element'.",
                "upd": "InvalidStateError: Failed to execute 'updateElementGeometry' on 'HTMLCanvasElement': updateElementGeometry requires the canvas to have the content=drawable attribute.",
                "clear": "undef",
                "clearText": "TypeError: Failed to execute 'clearElementGeometry' on 'HTMLCanvasElement': The provided value is not of type '(Element or ElementImage)'.",
                "paint": "undef",
                "cap0": "TypeError: Failed to execute 'captureElementImage' on 'HTMLCanvasElement': 1 argument required, but only 0 present.",
                "draw0": "TypeError: Failed to execute 'drawElementImage' on 'CanvasRenderingContext2D': 3 arguments required, but only 0 present.",
                "drawText": "TypeError: Failed to execute 'drawElementImage' on 'CanvasRenderingContext2D': The provided value is not of type '(Element or ElementImage)'.",
                "drawSym": "TypeError: Failed to execute 'drawElementImage' on 'CanvasRenderingContext2D': Cannot convert a Symbol value to a number",
            },
            "needAttr": "InvalidStateError: Failed to execute 'getElementTransform' on 'HTMLCanvasElement': Descendants passed to getElementTransform must have the 'drawable' attribute.",
            "outside": "InvalidStateError: Failed to execute 'getElementImageDefaultSize' on 'HTMLCanvasElement': The element can only be drawn into its nearest ancestor <canvas>.",
            "xf": ["DOMMatrix", "matrix(1, 0, 0, 1, 0, 0)", true],
            "upd": "undef",
            "size": "InvalidStateError: Failed to execute 'getElementImageDefaultSize' on 'HTMLCanvasElement': No cached paint record for element.",
            "cap": "InvalidStateError: Failed to execute 'captureElementImage' on 'HTMLCanvasElement': No cached paint record for element.",
            "draw": "InvalidStateError: Failed to execute 'drawElementImage' on 'CanvasRenderingContext2D': No cached paint record for element.",
            "nested": "InvalidStateError: Failed to execute 'getElementImageDefaultSize' on 'HTMLCanvasElement': The element can only be drawn into its nearest ancestor <canvas>.",
            "tail": ["constructor", "content", "onpaint", "onelementgeometryupdate", "captureElementImage", "clearElementGeometry", "getElementImageDefaultSize", "getElementTransform", "requestPaint", "updateElementGeometry"],
            "drawTail": ["constructor", "drawElementImage"],
            "offTail": ["getContextAttributes", "constructor", "drawElementImage"],
            "off": "InvalidStateError: Failed to execute 'drawElementImage' on 'OffscreenCanvasRenderingContext2D': Elements cannot be drawn into an OffscreenCanvas.",
            "gl0": "TypeError: Failed to execute 'texElementSubImage2D' on 'WebGLRenderingContext': 5 arguments required, but only 0 present.",
            "gl2_0": "TypeError: Failed to execute 'texElementSubImage2D' on 'WebGL2RenderingContext': 5 arguments required, but only 0 present.",
            "glText": "TypeError: Failed to execute 'texElementSubImage2D' on 'WebGLRenderingContext': The provided value is not of type '(Element or ElementImage)'.",
            "glOk": "undef",
            "glForeign": "TypeError: Illegal invocation",
            "glTail": ["constructor", "makeXRCompatible", "texElementSubImage2D"],
            "glLen": 5,
        })
    );
}

/// Chrome 157 SVGTextPathElement side constants. They sit after the spacing
/// constants on both the prototype and the constructor, and they are sealed.
#[test]
fn chrome157_textpath_side_constants() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"
        const P = SVGTextPathElement.prototype;
        const C = SVGTextPathElement;
        const desc = (obj, n) => {
          const d = Object.getOwnPropertyDescriptor(obj, n);
          return [d.value, d.enumerable, d.writable, d.configurable];
        };
        const names = ['TEXTPATH_SIDETYPE_UNKNOWN', 'TEXTPATH_SIDETYPE_LEFT', 'TEXTPATH_SIDETYPE_RIGHT'];
        const proto = Object.getOwnPropertyNames(P);
        const stat = Object.getOwnPropertyNames(C).filter((k) => k !== 'length' && k !== 'name' && k !== 'prototype');
        ({
          vals: names.map((n) => [P[n], C[n], P[n] === C[n]]),
          desc: names.map((n) => desc(P, n)),
          statDesc: desc(C, 'TEXTPATH_SIDETYPE_LEFT'),
          protoTail: proto.slice(proto.indexOf('TEXTPATH_SPACINGTYPE_EXACT')),
          statTail: stat.slice(stat.indexOf('TEXTPATH_SPACINGTYPE_EXACT')),
        })
        "#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "vals": [[0, 0, true], [1, 1, true], [2, 2, true]],
            "desc": [[0, true, false, false], [1, true, false, false], [2, true, false, false]],
            "statDesc": [1, true, false, false],
            "protoTail": ["TEXTPATH_SPACINGTYPE_EXACT", "TEXTPATH_SIDETYPE_UNKNOWN", "TEXTPATH_SIDETYPE_LEFT", "TEXTPATH_SIDETYPE_RIGHT", "constructor"],
            "statTail": ["TEXTPATH_SPACINGTYPE_EXACT", "TEXTPATH_SIDETYPE_UNKNOWN", "TEXTPATH_SIDETYPE_LEFT", "TEXTPATH_SIDETYPE_RIGHT"],
        })
    );
}
