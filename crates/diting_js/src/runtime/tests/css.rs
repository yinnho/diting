//!
use super::super::*;
use super::common::setup_runtime;

/// moli#282: an @container condition must answer against the nearest
/// container ancestor's solved geometry — the arm styles the element when
/// the container is wide enough, not when it is narrow or absent. Before
/// this batch the arms parsed but never matched, exactly moli's repro.
#[cfg(feature = "screenshot")]
#[test]
fn container_query_gates_by_container_geometry() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             #cqbox { container-type: inline-size; width: 300px; }\
             .inner { color: rgb(0, 0, 255); }\
             @container (min-width: 200px) { .inner { color: rgb(255, 0, 0); } }\
             </style></head><body>\
             <div id='cqbox'><div class='inner' id='i1'>x</div></div>\
             <div style='width: 100px; container-type: inline-size'><div class='inner' id='i2'>y</div></div>\
             <div class='inner' id='i3'>z</div>\
             </body></html>",
    );
    let out = rt
            .evaluate(
                "['i1','i2','i3'].map(id => getComputedStyle(document.getElementById(id)).color).join('|')",
            )
            .unwrap();
    assert_eq!(
        out,
        serde_json::json!("rgb(255, 0, 0)|rgb(0, 0, 255)|rgb(0, 0, 255)"),
        "300px container passes min-width:200px; the 100px container and the container-less element do not"
    );
    // The container properties themselves read back through gCS.
    let face = rt
        .evaluate(
            "[getComputedStyle(document.getElementById('cqbox')).containerType,\
                  getComputedStyle(document.getElementById('i1')).containerType,\
                  getComputedStyle(document.getElementById('i1')).containerName].join('|')",
        )
        .unwrap();
    assert_eq!(face, serde_json::json!("inline-size|normal|none"));
}

/// Container-query units bake against the answering container's border
/// box: 10cqw of a 300px container is 30px.
#[cfg(feature = "screenshot")]
#[test]
fn container_query_units_bake_against_container_size() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             #cqbox { container-type: inline-size; width: 300px; }\
             @container (min-width: 10px) { #p { padding-left: 10cqw; } }\
             </style></head><body><div id='cqbox'><div id='p'>x</div></div></body></html>",
    );
    assert_eq!(
        rt.evaluate("getComputedStyle(document.getElementById('p')).paddingLeft")
            .unwrap(),
        serde_json::json!("30px"),
    );
}

/// A named @container answers only against containers carrying the name:
/// a NEARER anonymous container must not satisfy it — the lookup walks
/// up to the named one (here 400px wide, so the 200px condition fires).
#[cfg(feature = "screenshot")]
#[test]
fn container_query_named_walks_past_nearer_anonymous() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             .item { color: rgb(0, 0, 255); }\
             @container side (min-width: 200px) { .item { color: rgb(255, 0, 0); } }\
             </style></head><body>\
             <div style='container-type: inline-size; container-name: side; width: 400px'>\
               <div style='container-type: inline-size; width: 100px'><div class='item' id='a'>x</div></div>\
             </div>\
             <div style='container-type: inline-size; width: 400px'><div class='item' id='b'>y</div></div>\
             </body></html>",
    );
    let out = rt
        .evaluate(
            "['a','b'].map(id => getComputedStyle(document.getElementById(id)).color).join('|')",
        )
        .unwrap();
    assert_eq!(
        out,
        serde_json::json!("rgb(255, 0, 0)|rgb(0, 0, 255)"),
        "a: named lookup reaches the 400px 'side' container past the nearer 100px one; b has no 'side' ancestor"
    );
}

/// `:scope` must bind to the element a query is rooted at — the archify
/// viewer's Intent Trace does `container.querySelector(':scope > svg')`
/// and used to get null back, killing its IIFU with a TypeError.
#[test]
fn scope_pseudo_class_binds_to_query_root() {
    let mut rt = setup_runtime(
        r#"<div id="a"><svg id="direct"></svg><div><svg id="deep"></svg></div></div>
               <div id="b"><svg id="other"></svg></div>"#,
    );
    let direct = rt
        .evaluate("document.getElementById('a').querySelector(':scope > svg') ? 'yes' : 'no'")
        .unwrap();
    assert_eq!(direct, serde_json::json!("yes"));

    let deep_excluded = rt
        .evaluate("document.getElementById('a').querySelectorAll(':scope > svg').length")
        .unwrap();
    assert_eq!(deep_excluded.as_f64().unwrap() as i64, 1);

    // :scope in matches() is the element itself — the feature-detect idiom.
    let self_match = rt
        .evaluate("document.getElementById('a').matches(':scope')")
        .unwrap();
    assert_eq!(self_match, serde_json::json!(true));
    let class_match = rt
        .evaluate("document.getElementById('a').matches(':scope#a')")
        .unwrap();
    assert_eq!(class_match, serde_json::json!(true));

    // Document-rooted :scope means the document element (html).
    let doc_scope = rt
        .evaluate("document.querySelector(':scope') === document.documentElement")
        .unwrap();
    assert_eq!(doc_scope, serde_json::json!(true));
}

/// #171: removing a `<style>` element must drop its rules from the next
/// style resolution. The collected css bytes change with the removal, so
/// the match cache's css key rotates and a fresh full match runs — but
/// the contract is behavioral: computed color falls back the moment the
/// sheet's element leaves the document, whether it left by removeChild,
/// by clearing innerHTML, or by being detached and re-added (the
/// re-add must restyle, proving the removal truly dropped the hits).
#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn style_element_removal_drops_its_rules() {
    let mut rt = setup_runtime(
        "<html><head></head><body><div id=\"x\">t</div><div id=\"y\">u</div></body></html>",
    );
    let out = rt
        .evaluate(
            r#"(() => {
                    const cs = (id) => getComputedStyle(document.getElementById(id)).color;
                    const mk = (sel) => {
                        const s = document.createElement('style');
                        s.textContent = sel + '{color:rgb(255, 0, 0)}';
                        document.head.appendChild(s);
                        return s;
                    };
                    const a = mk('#x');
                    const before = cs('x');
                    a.parentNode.removeChild(a);
                    const afterRemove = cs('x');
                    const b = mk('#y');
                    const beforeClear = cs('y');
                    document.head.innerHTML = '';
                    const afterClear = cs('y');
                    const c = mk('#x');
                    const reAddedBefore = cs('x');
                    c.parentNode.removeChild(c);
                    document.head.appendChild(c);
                    const reAdded = cs('x');
                    return {before, afterRemove, beforeClear, afterClear, reAddedBefore, reAdded};
                })()"#,
        )
        .unwrap();

    assert_eq!(
        out,
        serde_json::json!({
            "before": "rgb(255, 0, 0)",
            "afterRemove": "rgb(0, 0, 0)",
            "beforeClear": "rgb(255, 0, 0)",
            "afterClear": "rgb(0, 0, 0)",
            "reAddedBefore": "rgb(255, 0, 0)",
            "reAdded": "rgb(255, 0, 0)",
        })
    );
}

/// #121: HTMLStyleElement.sheet. CKEditor's appendStyleText returns
/// `styleEl.sheet` and its wysiwyg editable setup reads `.ownerNode`
/// off that sheet — an undefined sheet killed the whole editor mount
/// at setMode('wysiwyg'). Chrome semantics pinned here: a connected
/// <style> owns a sheet (rules parsed, ownerNode = the element, stable
/// identity), a detached one reads null, a <link rel=stylesheet> is
/// null until its fetch lands, and the element's sheet is the same
/// object document.styleSheets lists.
#[tokio::test(flavor = "current_thread")]
async fn style_element_sheet_reflects_owner_node_and_rules() {
    let mut rt = setup_runtime("<html><head></head><body></body></html>");
    let out = rt
        .evaluate(
            r#"(() => {
                    const style = document.createElement('style');
                    style.textContent = '.a { color: rgb(1, 2, 3); } .b { margin: 0; }';
                    document.head.appendChild(style);
                    const s1 = style.sheet;
                    const link = document.createElement('link');
                    link.rel = 'stylesheet';
                    link.href = '/not-yet-loaded.css';
                    document.head.appendChild(link);
                    const detached = document.createElement('style');
                    return {
                        hasSheet: s1 != null,
                        ownerIsEl: !!s1 && s1.ownerNode === style,
                        rules: s1 ? String(s1.cssRules.length) : 'none',
                        stableIdentity: s1 === style.sheet,
                        linkPendingNull: link.sheet == null,
                        detachedNull: detached.sheet == null,
                        inDocSheets: Array.from(document.styleSheets).includes(s1),
                        probeTagNull: document.body.sheet == null,
                    };
                })()"#,
        )
        .unwrap();

    assert_eq!(
        out,
        serde_json::json!({
            "hasSheet": true,
            "ownerIsEl": true,
            "rules": "2",
            "stableIdentity": true,
            "linkPendingNull": true,
            "detachedNull": true,
            "inDocSheets": true,
            "probeTagNull": true,
        })
    );
}

/// A dynamically inserted `<link rel=stylesheet>` must fetch its sheet,
/// expose it through document.styleSheets, and fire the element's `load`
/// event. The event is the load-bearing part: webpack's mini-css chunk
/// runtime (d.f.miniCss) resolves chunk promises from it with no timeout
/// — without it, tmall pc-shop-webapp's lazy routes render a styled
/// blank page (router parked at navigation:"loading", Component never
/// attached).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn dynamic_stylesheet_link_fires_load_and_registers_sheet() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let css = "#probe { color: rgb(4, 5, 6); }";
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/css\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            css.len(),
            css
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body><p id=\"probe\">x</p></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const link = document.createElement('link');
                    link.rel = 'stylesheet';
                    link.href = '/sheet.css';
                    let heardListener = 'no';
                    link.addEventListener('load', () => { heardListener = 'yes'; }, { once: true });
                    const loaded = new Promise(r => { link.onload = () => r('fired'); });
                    document.head.appendChild(link);
                    const how = await loaded;
                    const sheet = Array.from(document.styleSheets).find(s => (s.href || '').endsWith('/sheet.css'));
                    return {
                        how,
                        heardListener,
                        sheet: String(sheet != null),
                        rules: sheet ? String(sheet.cssRules.length) : 'none',
                    };
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();

    // The landed sheet must also reach the cascade (layout cache drop on
    // ext_sheet_put). Cascade reads are screenshot-feature ops.
    #[cfg(feature = "screenshot")]
    let color = rt
        .evaluate("getComputedStyle(document.getElementById('probe')).color")
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["heardListener"], serde_json::json!("yes"));
    assert_eq!(v["sheet"], serde_json::json!("true"));
    assert_eq!(v["rules"], serde_json::json!("1"));
    #[cfg(feature = "screenshot")]
    assert_eq!(color, serde_json::json!("rgb(4, 5, 6)"));
}

/// The failure side of the same contract: a stylesheet link whose fetch
/// fails fires `error` (both the on* property and listeners), never a
/// phantom `load`.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn dynamic_stylesheet_link_fires_error_on_failed_fetch() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Bind then drop: the port answers connection-refused.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const link = document.createElement('link');
                    link.rel = 'stylesheet';
                    link.href = '/missing.css';
                    let sawLoad = false;
                    link.addEventListener('load', () => { sawLoad = true; }, { once: true });
                    const failed = new Promise(r => { link.onerror = () => r('fired'); });
                    document.head.appendChild(link);
                    const how = await failed;
                    return { how, sawLoad: String(sawLoad) };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["sawLoad"], serde_json::json!("false"));
}

/// #259: `<link rel=preload>` fires `load`/`error` like a stylesheet
/// link — it never applies a sheet, only warms the fetch and reports
/// settle. data: URLs settle as a 0ms task (no network); the warming
/// parks the body in the ext-sheet cache so the `<link rel=stylesheet>`
/// that follows the preload settles instantly off it.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn preload_link_fires_load_and_warms_the_sheet_cache() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let css = "#probe { color: rgb(1, 2, 3); }";
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/css\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            css.len(),
            css
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const dataLink = document.createElement('link');
                    dataLink.rel = 'preload'; dataLink.as = 'style';
                    dataLink.href = 'data:text/css,body{}';
                    const dataDone = new Promise(r => { dataLink.onload = () => r('fired'); });
                    document.head.appendChild(dataLink);
                    const dataHow = await dataDone;

                    const pre = document.createElement('link');
                    pre.rel = 'preload'; pre.as = 'style';
                    pre.href = '/sheet.css';
                    let preSawError = false;
                    pre.addEventListener('error', () => { preSawError = true; }, { once: true });
                    const preDone = new Promise(r => { pre.onload = () => r('fired'); });
                    document.head.appendChild(pre);
                    const preHow = await preDone;

                    // The warmed body settles the follow-up stylesheet link
                    // through the cache (no second hit on the one-shot server).
                    const sheet = document.createElement('link');
                    sheet.rel = 'stylesheet';
                    sheet.href = '/sheet.css';
                    const sheetDone = new Promise(r => { sheet.onload = () => r('fired'); });
                    document.head.appendChild(sheet);
                    const sheetHow = await sheetDone;

                    return { dataHow, preHow, preSawError: String(preSawError), sheetHow,
                             sheetCount: String(document.styleSheets.length) };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["dataHow"], serde_json::json!("fired"));
    assert_eq!(v["preHow"], serde_json::json!("fired"));
    assert_eq!(v["preSawError"], serde_json::json!("false"));
    assert_eq!(v["sheetHow"], serde_json::json!("fired"));
    // Preload alone never registers a sheet — the follow-up link does.
    assert_eq!(v["sheetCount"], serde_json::json!("1"));
}

/// #259 failure side: a preload whose fetch fails fires `error`, never a
/// phantom `load` — the waiter's retry logic runs on the same signal a
/// browser gives it.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn preload_link_fires_error_on_failed_fetch() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Bind then drop: the port answers connection-refused.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const pre = document.createElement('link');
                    pre.rel = 'preload'; pre.as = 'style';
                    pre.href = '/missing.css';
                    let sawLoad = false;
                    pre.addEventListener('load', () => { sawLoad = true; }, { once: true });
                    const failed = new Promise(r => { pre.onerror = () => r('fired'); });
                    document.head.appendChild(pre);
                    const how = await failed;
                    return { how, sawLoad: String(sawLoad) };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["sawLoad"], serde_json::json!("false"));
}

/// DOMTokenList.supports() must answer for the lists the spec gives
/// supported tokens (<link> relList, iframe sandbox, and a/area/form
/// relList = noopener/noreferrer/opener - measured in Chrome 145,
/// htmlreflect-truth tokens: supports('preload') is false there, NOT a
/// throw) and throw for the rest (classList, link.sizes). Resource loaders feature-probe via
/// relList.supports("preload") — an unguarded throw there aborts the
/// whole load chain (2captcha's turnstile loader died exactly this way).
#[tokio::test(flavor = "current_thread")]
async fn tokenlist_supported_sets_match_spec() {
    let mut rt = setup_runtime(
        r##"<html><body><a id="x" href="#">a</a><iframe id="f" sandbox="allow-scripts"></iframe><link id="l" rel="stylesheet" href="s.css"></body></html>"##,
    );
    let result = rt
            .evaluate_remote(
                r#"(() => {
                    const out = {};
                    const link = document.getElementById("l");
                    const anchor = document.getElementById("x");
                    const frame = document.getElementById("f");
                    out.linkPreload = link.relList.supports("preload");
                    out.linkModulepreload = link.relList.supports("modulepreload");
                    out.linkNonsense = link.relList.supports("nonsense");
                    try { out.anchorPreload = anchor.relList.supports("preload"); out.anchorNoopener = anchor.relList.supports("noopener"); out.anchorThrows = false; }
                    catch (e) { out.anchorThrows = e instanceof TypeError; }
                    out.sandboxScripts = frame.sandbox.supports("allow-scripts");
                    out.sandboxBogus = frame.sandbox.supports("allow-bogus");
                    try { document.body.classList.supports("x"); out.classThrows = false; }
                    catch (e) { out.classThrows = e instanceof TypeError; }
                    return JSON.stringify(out);
                })()"#,
                false,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(
            r#"{"linkPreload":true,"linkModulepreload":true,"linkNonsense":false,"anchorPreload":false,"anchorNoopener":true,"anchorThrows":false,"sandboxScripts":true,"sandboxBogus":false,"classThrows":true}"#
        ),
        "supported-token sets must match the spec/Chrome shape per element kind"
    );
}

// CSS custom properties end-to-end (#229 residue): --* declared in a
// stylesheet must (a) feed var() substitution in the cascade, (b) be
// readable back through getComputedStyle().getPropertyValue('--*')
// case-sensitively, and (c) survive the inline-style round-trip without
// the kebab-lowercase pass corrupting --mainColor into --main-color.
// background-image rides along raw (longhand only).
#[test]
#[cfg(feature = "screenshot")]
fn test_custom_properties_and_var_substitution() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          :root { --brand: rgb(200, 10, 10); --card-w: 120px; }
          .card { width: var(--card-w); color: var(--brand); background-image: linear-gradient(to right, var(--brand), blue); }
          .fallback { color: var(--missing, rgb(9, 9, 9)); }
        </style></head><body>
        <div class="card" id="c">x</div>
        <div class="fallback" id="f">y</div>
        </body></html>"#,
    );
    let out = rt
            .evaluate(
                r#"JSON.stringify({
                    color: getComputedStyle(document.getElementById('c')).color,
                    bg: getComputedStyle(document.getElementById('c')).backgroundImage,
                    rootVar: getComputedStyle(document.getElementById('c')).getPropertyValue('--brand'),
                    inheritedVar: getComputedStyle(document.getElementById('c')).getPropertyValue('--card-w'),
                    fallback: getComputedStyle(document.getElementById('f')).color,
                    camelInline: (function() {
                        document.body.style.setProperty('--mainColor', '#123456');
                        return getComputedStyle(document.body).getPropertyValue('--mainColor');
                    })(),
                })"#,
            )
            .unwrap();
    println!("custom props diagnostics: {out}");
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["color"],
        serde_json::json!("rgb(200, 10, 10)"),
        "var() substitutes in the cascade"
    );
    assert_eq!(
        v["bg"],
        serde_json::json!("linear-gradient(to right, rgb(200, 10, 10), rgb(0, 0, 255))"),
        "vars substitute AND stops re-serialize Chrome-style (#17: blue → rgb(0, 0, 255))"
    );
    assert_eq!(
        v["rootVar"],
        serde_json::json!("rgb(200, 10, 10)"),
        "--brand readable on descendants"
    );
    assert_eq!(
        v["inheritedVar"],
        serde_json::json!("120px"),
        "--card-w inherits from :root"
    );
    assert_eq!(
        v["fallback"],
        serde_json::json!("rgb(9, 9, 9)"),
        "var() fallback applies"
    );
    assert_eq!(
        v["camelInline"],
        serde_json::json!("#123456"),
        "--mainColor survives case-sensitively"
    );
}

#[test]
fn test_cssom_setter_rejects_invalid_length_values() {
    // #148: invalid values are silent no-ops that keep the previous
    // declaration (Chrome headless ground truth: style.left='NaNpx'
    // reads back ''). The door covers the pure length/percentage family;
    // custom properties and judgement-proof values stay permissive.
    let mut rt = setup_runtime(r#"<div id="el"></div>"#);
    let result = rt.evaluate(r#"
            const s = document.getElementById('el').style;
            s.left = 'NaNpx';
            const nanRejected = s.left;
            s.left = '10px';
            s.left = 'banana';
            const keepOld = s.left;
            const moreRejects = (() => {
                s.top = 'undefinedpx'; const a = s.top;
                s.top = 'Infinitypx'; const b = s.top;
                s.top = '[object Object]'; const c = s.top;
                s.top = '10'; const d = s.top;
                s.top = '10pxpx'; const e = s.top;
                return [a, b, c, d, e];
            })();
            s.left = '0';
            const zero = s.left;
            s.width = '-5.5%';
            const pct = s.width;
            s.left = 'auto';
            const auto = s.left;
            s.margin = '5px 10px';
            const shorthand = s.margin;
            s.paddingTop = 'calc(10px + 2em)';
            const calc = s.paddingTop;
            s.borderTopWidth = 'thin';
            const thin = s.borderTopWidth;
            s.setProperty('--x', 'NaNpx anything');
            const custom = s.getPropertyValue('--x');
            s.cssText = 'left: NaNpx; color: red';
            const partial = s.cssText;
            return [nanRejected, keepOld, moreRejects, zero, pct, auto, shorthand, calc, thin, custom, partial];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "",
            "10px",
            ["", "", "", "", ""],
            "0",
            "-5.5%",
            "auto",
            "5px 10px",
            "calc(10px + 2em)",
            "thin",
            "NaNpx anything",
            "color: red;"
        ])
    );
}

#[test]
fn test_style_attribute_syncs_both_directions() {
    // CSSStyleDeclaration was in-memory only: parsed inline styles were
    // invisible to el.style.*, and el.style.x = … never reached the
    // attribute or serialization.
    let mut rt = setup_runtime(r#"<div id="el" style="color: red"></div>"#);
    let result = rt
        .evaluate(
            r#"
            const el = document.getElementById('el');
            const before = el.style.color;
            el.style.color = 'blue';
            const attrAfterSet = el.getAttribute('style');
            el.setAttribute('style', 'margin: 5px');
            const margin = el.style.margin;
            const colorGone = el.style.color;
            el.style.removeProperty('margin');
            return [before, attrAfterSet, margin, colorGone, el.getAttribute('style')];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["red", "color: blue;", "5px", "", null])
    );
}

/// #196: the CSSOM face of the display gate — a replaced element
/// (textarea) VISIBLE at parse time must vanish when the page hides it
/// with a runtime `style.display = 'none'` write. The write rides the
/// same restyle → recascade → layout-rebuild path as an attribute, and
/// build_replaced_leaf's gate closes every attach site, so the control
/// loses its box (gBCR/offsets zero) instead of painting its value —
/// the raw `<style …>` template text baidu/sina ship in hidden
/// textareas.
#[cfg(feature = "screenshot")]
#[test]
fn test_cssom_display_none_write_hides_replaced_element() {
    let mut rt = setup_runtime(
        r#"<html><body><textarea id="ta">&lt;style id="css_result"&gt;#ftCon{display:none}&lt;/style&gt;</textarea><p id="p">hello</p></body></html>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const ta = document.getElementById("ta");
            const before = [ta.getBoundingClientRect().width, ta.getBoundingClientRect().height];
            ta.style.display = "none";
            const after = [ta.getBoundingClientRect().width, ta.getBoundingClientRect().height,
                           ta.offsetWidth, ta.offsetHeight];
            const p = document.getElementById("p").getBoundingClientRect();
            return [before[0] > 0, before[1] > 0, after, p.height > 0];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "visible at parse: real box before the write"
    );
    assert_eq!(parts[1], serde_json::json!(true));
    assert_eq!(
        parts[2],
        serde_json::json!([0, 0, 0, 0]),
        "CSSOM display:none write must rebuild layout to a hidden box"
    );
    assert_eq!(
        parts[3],
        serde_json::json!(true),
        "rest of the page keeps rendering"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_style_element_load_event_on_insert() {
    // blitz#863 family 3: a dynamically connected <style> fires `load` as a
    // task (diting's CSS parser drops @import, so there is nothing to wait
    // for beyond the element's own text). Initial-markup <style> elements
    // come through __prepareInitialStylesheets instead — page.rs invokes it
    // before the script loop (see initial_parse_stylesheets_fire_load test).
    let mut rt = setup_runtime("<html><head></head><body></body></html>");
    let script = r#"async () => {
        const s = document.createElement('style');
        s.textContent = '.x { color: red; }';
        const fired = new Promise(r => s.addEventListener('load', (e) =>
            r([e.type, e instanceof Event, e.target === s, e.bubbles])));
        document.head.appendChild(s);
        await Promise.race([fired, new Promise(r => setTimeout(r, 2000))]);
        return fired;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["load", true, true, false])
    );
}

/// Batch-78 leftover: initial-parse `<style>`/`<link rel=stylesheet>` also
/// fire `load`. In Chrome those events are queued as tasks once each sheet
/// applies, and an inline script registering a listener during the parse
/// still catches them — page.rs reproduces that ordering by invoking
/// __prepareInitialStylesheets BEFORE the script loop, so the 0ms tasks land
/// between script executions. This test mirrors the exact page.rs sequence:
/// initial-sheets call, then an "inline script" registering listeners, then
/// the pump. The <link> leg rides the async fetch here (no navigation
/// prefetch in this harness — production hits the ext-sheet fast path).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn initial_parse_stylesheets_fire_load_for_early_inline_scripts() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let css = ".x { color: rgb(9, 9, 9); }";
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/css\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            css.len(),
            css
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime(
        "<html><head>\
         <style id=\"s\">.x { color: red; }</style>\
         <link id=\"l\" rel=\"stylesheet\" href=\"/sheet.css\">\
         </head><body></body></html>",
    );
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));

    // Exactly the line page.rs runs before the script loop.
    rt.execute_script(
        "<initial-sheets>",
        "if (typeof __prepareInitialStylesheets === 'function') __prepareInitialStylesheets();",
    )
    .unwrap();
    // The "inline script": execute_script is synchronous (no event-loop
    // pump), so these listeners land before the queued 0ms tasks fire.
    rt.execute_script(
        "<inline-1>",
        "globalThis.__heard = []; \
         for (const id of ['s', 'l']) { \
           const el = document.getElementById(id); \
           el.addEventListener('load', (e) => { \
             globalThis.__heard.push([id, e.type, e.target === el, e.bubbles]); \
           }); \
         }",
    )
    .unwrap();

    let result = rt
        .call_function_on_remote(
            r#"async () => {
                await new Promise(r => setTimeout(r, 1500));
                const h = globalThis.__heard || [];
                const pick = (id) => h.find(x => x[0] === id) || null;
                return [pick('s'), pick('l')];
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(
        v,
        serde_json::json!([["s", "load", true, false], ["l", "load", true, false]]),
        "both initial-parse sheets fire load, catchable by an earlier inline script"
    );
}

/// innerHTML-connected markup gets the same style-load treatment as
/// appendChild-inserted markup: the setter queues the 0ms task, and a
/// listener attached synchronously after the innerHTML write still catches
/// it (no pump happens in between).
#[tokio::test(flavor = "current_thread")]
async fn inner_html_style_element_fires_load() {
    let mut rt = setup_runtime("<html><head></head><body><div id=\"host\"></div></body></html>");
    let script = r#"async () => {
        const host = document.getElementById('host');
        host.innerHTML = '<style id="is">.y { color: blue; }</style>';
        const s = document.getElementById('is');
        const fired = new Promise(r => s.addEventListener('load', (e) =>
            r([e.type, e.target === s])));
        await Promise.race([fired, new Promise(r => setTimeout(r, 2000))]);
        return fired;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(["load", true]));
}

/// Generated content v1 (batch 99): ::before/::after boxes synthesize into
/// layout. Block pseudos grow the host (the Bootstrap 3 clearfix family —
/// `display:table` coerces to block for flow presence), inline pseudos
/// merge onto the adjacent text run without adding a line, attr() resolves
/// against the host, and content:none cascades but produces no box.
#[cfg(feature = "screenshot")]
#[test]
fn generated_content_before_after_layout() {
    let mut rt = setup_runtime(
        r#"<style>
          body{margin:0}
          .grow::after{content:"";display:block;height:20px}
          .tbl::before{content:"";display:table;height:15px}
          .merge li::before{content:"• "}
          .attr::after{content:attr(data-tip)}
          .none::after{content:none}
        </style>
        <ul class="merge"><li id="li">one</li></ul>
        <ul><li id="liCtrl">one</li></ul>
        <div id="grow" class="grow"></div>
        <div id="tbl" class="tbl"></div>
        <div id="attr" class="attr" data-tip="abc"></div>
        <div id="none" class="none"></div>"#,
    );
    let v = rt
        .evaluate(
            r#"
            const h = (id) => Math.round(document.getElementById(id).getBoundingClientRect().height);
            return { li: h('li'), liCtrl: h('liCtrl'), grow: h('grow'), tbl: h('tbl'), attr: h('attr'), none: h('none') };
        "#,
        )
        .unwrap();
    assert_eq!(
        v["li"], v["liCtrl"],
        "inline ::before merges onto the adjacent run: no extra line"
    );
    assert_eq!(
        v["grow"],
        serde_json::json!(20),
        "block ::after with a height grows the host (clearfix box)"
    );
    assert_eq!(
        v["tbl"],
        serde_json::json!(15),
        "display:table pseudos coerce to block: flow presence, not the table walk"
    );
    assert!(
        v["attr"].as_f64().unwrap_or(0.0) >= 10.0,
        "attr() content on a childless host still produces a line"
    );
    assert_eq!(
        v["none"],
        serde_json::json!(0),
        "content:none cascades but produces no box"
    );
}

/// Batch 105 + #19: counter()/counters() numbering — the css-lists-3 §4.3
/// worked example's canonical shapes: flat 1..n, nested ol joins "3.1", the
/// li after the nested list continues the OUTER counter (inner counter
/// never reaches following siblings of its creator's parent), and a sibling
/// ol restarts because reset shadows previous-sibling-origin counters
/// (§4.4.2). The ::before face keeps the functional shape (#19), so the
/// numbering itself is asserted by width-parity against literal-content
/// controls — identical construction (same rule shape, same merge into the
/// li's text run), so widths are equal iff resolution produced exactly the
/// control's text.
#[cfg(feature = "screenshot")]
#[test]
fn counters_numbering_scoping_and_restart() {
    let mut rt = setup_runtime(
        r#"<style>
          ol{counter-reset:item;list-style:none;margin:0;padding:0}
          li{margin:0;padding:0}
          li::before{counter-increment:item;content:counters(item,".") " "}
          li.m1::before{content:"1 "}
          li.m31::before{content:"3.1 "}
          li.m4::before{content:"4 "}
        </style>
        <ol>
          <li id="a">alpha</li>
          <li id="b">beta</li>
          <li id="c">gamma<ol><li id="d">delta</li></ol></li>
          <li id="e">epsilon</li>
        </ol>
        <ol><li id="f">zeta</li></ol>
        <ol><li class="m1" id="ca">alpha</li></ol>
        <ol><li class="m31" id="cd">delta</li></ol>
        <ol><li class="m4" id="ce">epsilon</li></ol>
        <ol><li class="m1" id="cf">zeta</li></ol>"#,
    );
    let read = |rt: &mut JsRuntime, expr: &str| -> String {
        match rt.evaluate(expr) {
            Ok(serde_json::Value::String(s)) => s,
            other => panic!("{expr} -> {other:?}"),
        }
    };
    // #19: every ::before face serves the authored functional shape.
    for id in ["a", "b", "c", "d", "e", "f"] {
        assert_eq!(
            read(
                &mut rt,
                &format!(
                    "getComputedStyle(document.getElementById('{id}'),'::before').getPropertyValue('content')"
                )
            ),
            r#"counters(item, ".") " ""#,
            "{id} face"
        );
    }
    let width = |rt: &mut JsRuntime, id: &str| -> f64 {
        rt.evaluate(&format!(
            "document.getElementById('{id}').getBoundingClientRect().width"
        ))
        .unwrap()
        .as_f64()
        .unwrap_or(-1.0)
    };
    let parity = |rt: &mut JsRuntime, probe: &str, ctl: &str| {
        let (p, c) = (width(rt, probe), width(rt, ctl));
        assert!(
            (p - c).abs() < 0.5,
            "{probe} resolved text width {p} != {ctl} {c}"
        );
    };
    parity(&mut rt, "a", "ca");
    parity(&mut rt, "d", "cd");
    parity(&mut rt, "e", "ce");
    parity(&mut rt, "f", "cf");
}

/// Batch 105 + #19: counter style formatting, implicit-zero counters,
/// same-element reset+increment composition (reset then increment → 6), and
/// the gCS faces keeping the source shape functional (Chrome parity). The
/// resolved values ride `content_resolved` (invisible to JS by design), so
/// the formatting is asserted by width-parity against literal-content
/// control spans — equal widths iff resolution produced the control's text.
#[cfg(feature = "screenshot")]
#[test]
fn counter_styles_composition_and_host_face() {
    let mut rt = setup_runtime(
        r#"<style>
          .r::before{content:counter(x, upper-roman) " "}
          .rc::before{content:"III "}
          .al::before{content:counter(y, lower-alpha) " "}
          .ci::before{content:counter(n) " "}
          .cic::before{content:"6 "}
          .z::before{content:counter(ghost)}
          .zc::before{content:"0"}
          #h{content:counter(w, upper-roman) " " attr(data-k)}
        </style>
        <div style="counter-reset:x 3"><span class="r"></span></div>
        <div><span class="rc"></span></div>
        <div style="counter-reset:y"><span class="al"></span></div>
        <span class="ci" style="counter-reset:n 5;counter-increment:n"></span><span class="cic"></span>
        <span class="z"></span><span class="zc"></span>
        <span id="h" data-k="K"></span>"#,
    );
    let read = |rt: &mut JsRuntime, expr: &str| -> String {
        match rt.evaluate(expr) {
            Ok(serde_json::Value::String(s)) => s,
            other => panic!("{expr} -> {other:?}"),
        }
    };
    let face = |rt: &mut JsRuntime, sel: &str| -> String {
        read(
            rt,
            &format!(
                "getComputedStyle(document.querySelector('{sel}'),'::before').getPropertyValue('content')"
            ),
        )
    };
    // #19: functional shape, both counter() spellings.
    assert_eq!(face(&mut rt, ".r"), r#"counter(x, upper-roman) " ""#);
    assert_eq!(face(&mut rt, ".al"), r#"counter(y, lower-alpha) " ""#);
    assert_eq!(face(&mut rt, ".ci"), r#"counter(n) " ""#);
    assert_eq!(face(&mut rt, ".z"), "counter(ghost)");
    // Host face: source shape round-trips functional notation.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('h')).content"
        ),
        "counter(w, upper-roman) \" \" attr(data-k)"
    );
    let width = |rt: &mut JsRuntime, sel: &str| -> f64 {
        rt.evaluate(&format!(
            "document.querySelector('{sel}').getBoundingClientRect().width"
        ))
        .unwrap()
        .as_f64()
        .unwrap_or(-1.0)
    };
    let parity = |rt: &mut JsRuntime, probe: &str, ctl: &str| {
        let (p, c) = (width(rt, probe), width(rt, ctl));
        assert!(
            (p - c).abs() < 0.5,
            "{probe} resolved text width {p} != {ctl} {c}"
        );
    };
    parity(&mut rt, ".r", ".rc"); // reset to 3 renders upper-roman III
    parity(&mut rt, ".ci", ".cic"); // reset 5 then increment on the same element
    parity(&mut rt, ".z", ".zc"); // a name never reset resolves to 0
}

/// #19 (Chrome-diff battery): the pseudo's computed content keeps its
/// functional shape — Chrome's gCS serves `counter(c) ". "` while the box
/// renders `1. `. The flattened text lives on `content_resolved` for the
/// box builder and never leaks into the CSSOM face.
#[cfg(feature = "screenshot")]
#[test]
fn pseudo_content_functional_shape_and_render() {
    let mut rt = setup_runtime(
        r#"<style>
          li{counter-increment:c}
          li::before{content:counter(c) ". "}
          li.one::before{content:"1. "}
        </style>
        <ul style="list-style:none;margin:0;padding:0">
          <li id="a">x</li>
          <li class="one" id="ctl">x</li>
        </ul>"#,
    );
    let read = |rt: &mut JsRuntime, expr: &str| -> String {
        match rt.evaluate(expr) {
            Ok(serde_json::Value::String(s)) => s,
            other => panic!("{expr} -> {other:?}"),
        }
    };
    // The battery face: functional shape, counter unresolved.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('a'),'::before').getPropertyValue('content')"
        ),
        r#"counter(c) ". ""#
    );
    // Literal strings keep their plain quoted face.
    assert_eq!(
        read(
            &mut rt,
            "getComputedStyle(document.getElementById('ctl'),'::before').getPropertyValue('content')"
        ),
        r#""1. ""#
    );
    // And the box still renders the resolved `1. ` (width parity against
    // the literal control: same merge into the same li text run).
    let widths: Vec<f64> = ["a", "ctl"]
        .into_iter()
        .map(|id| {
            rt.evaluate(&format!(
                "document.getElementById('{id}').getBoundingClientRect().width"
            ))
            .unwrap()
            .as_f64()
            .unwrap_or(-1.0)
        })
        .collect();
    assert!(
        (widths[0] - widths[1]).abs() < 0.5,
        "resolved counter text width {:?} != literal control {:?}",
        widths[0],
        widths[1]
    );
}

/// Batch 105 + #19: quotes — the gCS face keeps the open-quote/close-quote
/// keywords (the glyph is a walk-time used value: it depends on the
/// document-position quote depth, which a computed value cannot see). The
/// depth-pair selection, the `quotes: none` emptying and the property-less
/// curly default are asserted by width-parity against literal-glyph
/// controls with identical box shapes.
#[cfg(feature = "screenshot")]
#[test]
fn quotes_depth_pairs_none_and_default() {
    let mut rt = setup_runtime(
        r#"<style>
          .o::before{content:open-quote}
          .o::after{content:close-quote}
          #nn{quotes:none}
          #nn::before{content:open-quote}
          #dd::before{content:open-quote}
          #dd::after{content:close-quote}
          .kc::before{content:"«"}
          .kc::after{content:"»"}
          .ki::before{content:"‹"}
          .ki::after{content:"›"}
          .kd::before{content:"“"}
          .kd::after{content:"”"}
        </style>
        <div style="quotes: '«' '»' '‹' '›'">
          <span class="o" id="oo"><span class="o" id="oi"></span></span>
          <span class="kc" id="ck"><span class="ki"></span></span>
          <span class="ki" id="ci"></span>
        </div>
        <span id="dd"></span><span class="kd" id="cd"></span>
        <span id="nn"></span><span id="bare"></span>"#,
    );
    let read = |rt: &mut JsRuntime, expr: &str| -> String {
        match rt.evaluate(expr) {
            Ok(serde_json::Value::String(s)) => s,
            other => panic!("{expr} -> {other:?}"),
        }
    };
    let face = |rt: &mut JsRuntime, id: &str, pseudo: &str| -> String {
        read(
            rt,
            &format!(
                "getComputedStyle(document.getElementById('{id}'),'{pseudo}').getPropertyValue('content')"
            ),
        )
    };
    // #19: the keywords persist on every face — outer, inner, none, default.
    for id in ["oo", "oi", "dd", "nn"] {
        assert_eq!(face(&mut rt, id, "::before"), "open-quote", "{id} face");
    }
    for id in ["oo", "oi", "dd"] {
        assert_eq!(face(&mut rt, id, "::after"), "close-quote", "{id} face");
    }
    let width = |rt: &mut JsRuntime, id: &str| -> f64 {
        rt.evaluate(&format!(
            "document.getElementById('{id}').getBoundingClientRect().width"
        ))
        .unwrap()
        .as_f64()
        .unwrap_or(-1.0)
    };
    let parity = |rt: &mut JsRuntime, probe: &str, ctl: &str| {
        let (p, c) = (width(rt, probe), width(rt, ctl));
        assert!(
            (p - c).abs() < 0.5,
            "{probe} resolved glyph width {p} != {ctl} {c}"
        );
    };
    // Inner: depth-1 pair ‹›; outer: depth-0 pair «» around the inner box.
    parity(&mut rt, "oi", "ci");
    parity(&mut rt, "oo", "ck");
    // Property-less default is the curly double pair.
    parity(&mut rt, "dd", "cd");
    // quotes:none renders nothing — the span stays empty-width like a
    // pseudo-less bare span.
    parity(&mut rt, "nn", "bare");
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_supports_probes() {
    // CSS.supports routes to the Rust @supports evaluator. doudian probes
    // it ~15k times during boot; a throw or a wrong answer there detours
    // the app into polyfill paths.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => [
            CSS.supports('display', 'flex'),
            CSS.supports('display', 'not-a-real-value'),
            CSS.supports('(display: flex)'),
            CSS.supports('(display: bogus)'),
            CSS.supports('not (display: flex)'),
            CSS.supports('not (display: bogus)'),
            CSS.supports('(display: flex) and (display: bogus)'),
            CSS.supports('(display: flex) or (display: bogus)'),
            CSS.supports(':hover'),
            CSS.supports(':bogus-pseudo'),
            // #20: these three stay FALSE on purpose — Chrome says true,
            // but the engine has none of the layout behind them (anchor
            // positioning resolution, subgrid track inheritance — taffy
            // pins no subgrid, scroll-timeline sampling). Saying true
            // would flip feature-detected pages onto paths this layout
            // cannot honor; the honest answer routes them to working
            // fallbacks. Flip ONLY when the layout lands.
            CSS.supports('anchor-name', '--a'),
            CSS.supports('grid-template-columns', 'subgrid'),
            CSS.supports('animation-timeline', 'scroll()'),
        ]"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            true, false, true, false, false, true, false, true, true, false, false, false, false
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_nth_child_of_selector_and_register_property() {
    // Chrome truth (headless Chrome, 2026-10-22).
    let mut rt = setup_runtime(
        "<html><body><ul><li class=a>1<li>2<li class=a>3<li class=a>4</ul></body></html>",
    );
    let v = rt
            .evaluate(
                "(() => { const r = [];
                  for (const sel of ['li:nth-child(2 of .a)', 'li:nth-child(2n of .a)', 'li:nth-child(odd of .a)', 'li:nth-last-child(1 of .a)'])
                    r.push([...document.querySelectorAll(sel)].map(e => e.textContent).join());
                  const T = (f) => { try { f(); return 'ok'; } catch (e) { return e.name; } };
                  r.push(CSS.registerProperty.length);
                  r.push(T(() => CSS.registerProperty({ name: '--a', syntax: '*' })));
                  r.push(T(() => CSS.registerProperty({ name: 'a', syntax: '*', inherits: false })));
                  r.push(T(() => CSS.registerProperty({ name: '--b', syntax: '*', inherits: false })));
                  r.push(T(() => CSS.registerProperty({ name: '--b', syntax: '*', inherits: false })));
                  r.push(T(() => CSS.registerProperty({ name: '--c', syntax: '<length>', inherits: false })));
                  r.push(T(() => CSS.registerProperty({ name: '--d', syntax: '<length>', inherits: false, initialValue: 'red' })));
                  r.push(T(() => CSS.registerProperty({ name: '--e', syntax: '<length>', inherits: false, initialValue: '2em' })));
                  r.push(T(() => CSS.registerProperty({ name: '--f', syntax: '<bogus>', inherits: false, initialValue: '1' })));
                  r.push(T(() => CSS.registerProperty({ name: '--g', syntax: '<length>|auto', inherits: true, initialValue: '10px' })));
                  const cs = getComputedStyle(document.documentElement);
                  r.push(cs.getPropertyValue('--g') + '|' + JSON.stringify(cs.getPropertyValue('--b')));
                  return r.join('\\n'); })()",
            )
            .unwrap();
    assert_eq!(
        v,
        "3\n3\n1,4\n4\n1\nTypeError\nSyntaxError\nok\nInvalidModificationError\nSyntaxError\nSyntaxError\nSyntaxError\nSyntaxError\nok\n10px|\"\""
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_adopted_stylesheets_join_the_cascade() {
    // Chrome truth (headless Chrome, 2026-10-22): constructable sheets
    // adopted by the document / a shadow root style the page; edits to an
    // adopted sheet and removal from the list are live; insertRule
    // defaults to index 0.
    let mut rt = setup_runtime("<html><body><p id=p>x</p><div id=host></div></body></html>");
    let v = rt
            .evaluate(
                "(() => { const r = []; const c = (i) => getComputedStyle(document.getElementById(i)).color;
                  r.push(c('p'));
                  const s = new CSSStyleSheet(); s.replaceSync('#p{color:red}');
                  document.adoptedStyleSheets = [s]; r.push(c('p'));
                  s.replaceSync('#p{color:blue}'); r.push(c('p'));
                  s.insertRule('#p{color:green}'); r.push(c('p'));
                  document.adoptedStyleSheets = []; r.push(c('p'));
                  const sr = host.attachShadow({ mode: 'open' }); sr.innerHTML = '<b id=q>q</b>';
                  const t = new CSSStyleSheet(); t.replaceSync('b{color:red}'); sr.adoptedStyleSheets = [t];
                  r.push(getComputedStyle(sr.getElementById('q')).color);
                  let err = 'none'; try { document.adoptedStyleSheets = [{}]; } catch (e) { err = e.name; }
                  r.push(err);
                  return r.join('|'); })()",
            )
            .unwrap();
    assert_eq!(
        v,
        "rgb(0, 0, 0)|rgb(255, 0, 0)|rgb(0, 0, 255)|rgb(0, 0, 255)|rgb(0, 0, 0)|rgb(255, 0, 0)|TypeError"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_cssom_rule_tree_and_style_sheet_writes_reach_the_cascade() {
    // Chrome truth (headless Chrome, 2026-10-22): rule classes/types,
    // cssText spelling, nested rules, insertRule errors, and insertRule on a
    // <style> sheet restyling the page (the CSS-in-JS path).
    let mut rt = setup_runtime(
        "<html><head><style id=st>
              @import url(\"other.css\") screen;
              a,b  >  i{color:red}
              @media (min-width:100px) and (max-width:9000px){p{margin-top:3px}}
              @layer base,theme;
              @layer base{d{color:green}}
              @keyframes spin{from{opacity:0}to{opacity:1}}
              @property --x{syntax:\"<length>\";inherits:false;initial-value:0px}
             </style></head><body><p id=p>x</p></body></html>",
    );
    let v = rt
            .evaluate(
                "(() => { const sh = document.getElementById('st').sheet; const r = [];
                  r.push([...sh.cssRules].map(x => x.constructor.name + ':' + x.type).join());
                  const F = n => [...sh.cssRules].find(x => x.constructor.name === n);
                  r.push(sh.cssRules[1].cssText);
                  const m = F('CSSMediaRule');
                  r.push(m.cssText.replace(/\\n/g, '~'));
                  r.push(m.conditionText + '|' + m.cssRules.length + '|' + m.cssRules[0].selectorText + '|' + (m.parentStyleSheet === sh));
                  r.push(F('CSSLayerStatementRule').cssText + '|' + F('CSSLayerBlockRule').name);
                  r.push(F('CSSKeyframesRule').name + '|' + F('CSSKeyframesRule').cssRules[0].keyText + '|' + F('CSSKeyframesRule').cssRules[1].keyText);
                  r.push(F('CSSPropertyRule').name + '|' + F('CSSPropertyRule').syntax + '|' + F('CSSPropertyRule').inherits + '|' + F('CSSPropertyRule').initialValue);
                  r.push(F('CSSImportRule').href + '|' + F('CSSImportRule').media.mediaText);
                  const T = f => { try { return String(f()); } catch (e) { return e.name; } };
                  r.push(T(() => sh.insertRule('not a rule')) + ',' + T(() => sh.insertRule('p{color:red}', 999)) + ',' + T(() => sh.deleteRule(999)) + ',' + T(() => sh.insertRule('p{color:red}', 0)));
                  const p = document.getElementById('p'); const before = getComputedStyle(p).color;
                  sh.insertRule('#p{color:rgb(1,2,3)}', sh.cssRules.length);
                  r.push(before + '->' + getComputedStyle(p).color);
                  sh.deleteRule(sh.cssRules.length - 1); r.push(getComputedStyle(p).color);
                  m.insertRule('#p{margin-top:7px}'); r.push(getComputedStyle(p).marginTop);
                  const cs = new CSSStyleSheet(); cs.replaceSync('x{color:red}y{color:blue}'); r.push(cs.cssRules.length + ' ' + cs.cssRules[1].cssText);
                  return r.join('|'); })()",
            )
            .unwrap();
    assert_eq!(
        v,
        "CSSImportRule:3,CSSStyleRule:1,CSSMediaRule:4,CSSLayerStatementRule:0,CSSLayerBlockRule:0,CSSKeyframesRule:7,CSSPropertyRule:0\
|a, b > i { color: red; }\
|@media (min-width: 100px) and (max-width: 9000px) {~  p { margin-top: 3px; }~}\
|(min-width: 100px) and (max-width: 9000px)|1|p|true\
|@layer base, theme;|base\
|spin|0%|100%\
|--x|<length>|false|0px\
|other.css|screen\
|SyntaxError,IndexSizeError,IndexSizeError,HierarchyRequestError\
|rgb(0, 0, 0)->rgb(1, 2, 3)\
|rgb(0, 0, 0)\
|7px\
|2 y { color: blue; }"
    );
}

#[tokio::test]
async fn test_at_scope_roots_rules_and_honours_the_lower_bound() {
    // Chrome truth: rules inside @scope (root) only hit descendants of the
    // root, `:scope` is the root itself, and `to (limit)` stops at the limit.
    let mut rt = setup_runtime(
        "<html><head><style>
              @scope (.s) { p { color: red } :scope { margin-top: 7px } }
              @scope (.t) to (.lim) { em { color: blue } }
             </style></head><body>
              <div class=s id=r><p id=in>x</p></div><p id=out>y</p>
              <div class=t><em id=e1>a</em><div class=lim><em id=e2>b</em></div></div>
             </body></html>",
    );
    let v = rt
            .evaluate(
                "(() => { const c = i => getComputedStyle(document.getElementById(i));
                  return [c('in').color, c('out').color, c('r').marginTop, c('e1').color, c('e2').color].join('|'); })()",
            )
            .unwrap();
    assert_eq!(
        v,
        "rgb(255, 0, 0)|rgb(0, 0, 0)|7px|rgb(0, 0, 255)|rgb(0, 0, 0)"
    );
}

/// cssom_sheets.js ratchet split (JS-SURFACE-PLAN step 3): StyleSheet /
/// CSSStyleSheet, the document.styleSheets backing and adoptedStyleSheets
/// left bootstrap.js. Pins the inheritance surgery, per-element sheet
/// identity, StyleSheetList shape, and that the lexical-const helpers did
/// not mint window properties.
#[test]
fn cssom_sheets_identity_list_and_adoption() {
    let mut rt = setup_runtime(
        "<html><head><style id='s'>a{color:red}b{margin:0}</style></head><body></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
        (function () {
            var el = document.getElementById('s');
            var sh = el.sheet;
            var list = document.styleSheets;
            var ctorThrows = false;
            try { new StyleSheet(); } catch (e) { ctorThrows = e instanceof TypeError; }
            var c = new CSSStyleSheet();
            var detached = document.createElement('style').sheet;
            document.adoptedStyleSheets = [c];
            var bad = null;
            try { document.adoptedStyleSheets = [{}]; } catch (e) { bad = e.constructor.name; }
            var names = Object.getOwnPropertyNames(globalThis);
            return JSON.stringify({
                same: el.sheet === sh, owner: sh.ownerNode === el, rules: sh.cssRules.length,
                listLen: list.length, listItem: list.item(0) === sh, listIdx: list[0] === sh,
                notArray: typeof list.push, ctorThrows: ctorThrows,
                inherit: c instanceof StyleSheet && c instanceof CSSStyleSheet,
                type: sh.type, detached: detached, adopted: document.adoptedStyleSheets.length,
                bad: bad,
                leak: ['_sheetList','_elementSheet','_sheetFromCssText','_extSheetBodies','_parseSheetRules']
                        .filter(function (n) { return names.indexOf(n) >= 0; }),
            });
        })()
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["same"], true, "{v}");
    assert_eq!(v["owner"], true);
    assert_eq!(v["rules"], 2);
    assert_eq!(v["listLen"], 1);
    assert_eq!(v["listItem"], true);
    assert_eq!(v["listIdx"], true);
    assert_eq!(v["notArray"], "undefined");
    assert_eq!(v["ctorThrows"], true);
    assert_eq!(v["inherit"], true);
    assert_eq!(v["type"], "text/css");
    assert!(v["detached"].is_null());
    assert_eq!(v["adopted"], 1);
    assert_eq!(v["bad"], "TypeError");
    assert_eq!(
        v["leak"],
        serde_json::json!([]),
        "helpers must stay lexical consts"
    );
}
