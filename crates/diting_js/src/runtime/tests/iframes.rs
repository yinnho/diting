//! iframe 全家：srcdoc/realm/nid 域/视口独立/teardown
use super::super::*;
use super::common::setup_runtime;

// Frame-scoped eval (batch 167): the single-realm engine emulates a child
// frame's execution context by swapping the main realm's global bindings
// (document/window/self/frames/location) for the iframe's content window
// for the duration of the call, then restoring them — including when the
// eval throws and across an awaited promise's pump.
#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_frame_nid_scopes_to_iframe_and_restores() {
    let mut rt = setup_runtime(
        r#"<html><body><iframe id="f"></iframe><div id="main-marker"></div></body></html>"#,
    );
    let nid = rt
        .with_dom(|dom| dom.query_selector_all("#f").unwrap()[0].index() as u32)
        .unwrap();
    rt.evaluate("globalThis.__probeIframe = document.getElementById('f')")
        .unwrap();

    // Sync path: document/window/self/frames/location all resolve to the
    // iframe's.
    let outcome = rt
            .evaluate_remote_outcome(
                "document === __probeIframe.contentDocument && window === __probeIframe.contentWindow && self === window && frames === window && location === __probeIframe.contentWindow.location",
                true,
                false,
                DEFAULT_AWAIT_BUDGET_MS,
                Some(nid),
            )
            .await
            .unwrap();
    assert_eq!(outcome.info.value, Some(serde_json::json!(true)));

    // Awaited path: the swap must survive the promise pump.
    let outcome = rt
            .evaluate_remote_outcome(
                "Promise.resolve(document === __probeIframe.contentDocument && location === __probeIframe.contentWindow.location)",
                true,
                true,
                DEFAULT_AWAIT_BUDGET_MS,
                Some(nid),
            )
            .await
            .unwrap();
    assert_eq!(outcome.info.value, Some(serde_json::json!(true)));

    // Restore happened on the success path: main document is back and the
    // stash slots are gone.
    let restored = rt
            .evaluate(
                "document.getElementById('main-marker') !== null && !('__diting_frame_saved' in globalThis) && !('__diting_frame_location' in globalThis)",
            )
            .unwrap();
    assert_eq!(restored, serde_json::json!(true));
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_frame_nid_unknown_iframe_errors() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let err = rt
        .evaluate_remote_outcome("1 + 1", true, false, DEFAULT_AWAIT_BUDGET_MS, Some(999_999))
        .await
        .unwrap_err();
    assert!(err.contains("content not ready"), "got: {err}");
}

/// #255 / obscura#1197 negative evidence: tearing down a realm that
/// still holds RETAINED DETACHED iframes must not crash. Their engine
/// keeps borrowed native context annexes per frame and crashed in
/// finalizers during isolate teardown under GC stress; ours is
/// single-realm (frame windows are plain JS objects inside the main
/// isolate), so this pins the structural immunity: eval into live
/// frames, detach them while JS still references them, allocate to
/// pressure the GC, then drop the runtime — repeatedly.
/// #256 / moli#1328: script must not run in a retired frame. Chrome
/// discards a detached iframe's browsing context (its execution context
/// is gone; the XHR spec additionally gates open() on a fully-active
/// document), so frame eval into a detached iframe errors instead of
/// silently executing — in particular, no network can be initiated
/// from the retired frame.
#[tokio::test(flavor = "current_thread")]
async fn frame_eval_into_detached_iframe_is_refused_and_fires_nothing() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen = captured.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..1 {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(_) => break,
            };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf[..n]).to_string());
            let body = b"{}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(resp.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
        }
    });
    let mut rt =
        setup_runtime(r#"<html><body><div id="host"><iframe id="f"></iframe></div></body></html>"#);
    let nid = rt
        .with_dom(|dom| dom.query_selector_all("#f").unwrap()[0].index() as u32)
        .unwrap();

    // Attached: the frame answers.
    let ok = rt
        .evaluate_remote_outcome("1 + 1", true, false, DEFAULT_AWAIT_BUDGET_MS, Some(nid))
        .await
        .unwrap();
    assert_eq!(ok.info.value, Some(serde_json::json!(2)));

    // Detached (element detached but still JS-reachable — the retained
    // retired-frame shape): refused, before any script runs.
    rt.evaluate("document.getElementById('host').innerHTML = '';")
        .unwrap();
    let err = rt
        .evaluate_remote_outcome(
            &format!("fetch('http://127.0.0.1:{port}/retired').then(() => 'fired')",),
            true,
            true,
            DEFAULT_AWAIT_BUDGET_MS,
            Some(nid),
        )
        .await
        .unwrap_err();
    assert!(err.contains("content not ready"), "got: {err}");
    assert!(
        captured.lock().unwrap().is_empty(),
        "no request may leave a retired frame"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn teardown_with_retained_detached_iframes_does_not_crash() {
    for _round in 0..8 {
        let mut rt = setup_runtime(
            r#"<html><body><div id="host">
                    <iframe></iframe><iframe></iframe><iframe></iframe>
                </div></body></html>"#,
        );
        let nids = rt
            .with_dom(|dom| {
                dom.query_selector_all("iframe")
                    .unwrap()
                    .iter()
                    .map(|e| e.index() as u32)
                    .collect::<Vec<_>>()
            })
            .unwrap();
        rt.evaluate(
            "globalThis.__kept = Array.from(document.querySelectorAll('iframe')); \
                 globalThis.__mainDoc = document; \
                 __kept.push(...Array.from({length: 5000}, (_, i) => ({ i })));",
        )
        .unwrap();
        for &nid in &nids {
            let outcome = rt
                .evaluate_remote_outcome(
                    "document !== globalThis.__mainDoc ? 'swapped' : 'same'",
                    true,
                    false,
                    DEFAULT_AWAIT_BUDGET_MS,
                    Some(nid),
                )
                .await
                .unwrap();
            assert_eq!(outcome.info.value, Some(serde_json::json!("swapped")));
        }
        rt.evaluate("document.getElementById('host').innerHTML = '';")
            .unwrap();
        // The detached frames stay reachable through __kept past this
        // point — the exact "retained detached frames" shape — and the
        // next loop iteration's setup drops the runtime.
    }
}

// obscura #976: elements of a sync-created iframe document are an
// orphan Rust DOM subtree the main-document layout run never covers, so
// gBCR answered all zeros. The sub-run measures in the IFRAME'S OWN
// viewport (300x150 — the box the fabricated window publishes), so a
// body with its UA 8px margin puts the first div at (8,8), like Chrome.
#[test]
#[cfg(feature = "screenshot")]
fn test_sync_iframe_document_elements_have_layout_rects() {
    let mut rt = setup_runtime(r#"<html><body></body></html>"#);
    let out = rt
            .evaluate(
                r#"JSON.stringify((function() {
                    const f = document.createElement('iframe');
                    document.body.appendChild(f);
                    const doc = f.contentDocument;
                    doc.body.innerHTML = '<style>body{margin:8px}div{width:100px;height:20px}</style><div id=a></div><div id=b></div>';
                    const a = doc.getElementById('a');
                    const b = doc.getElementById('b');
                    const ra = a.getBoundingClientRect();
                    const rb = b.getBoundingClientRect();
                    const before = {
                        ax: ra.x, ay: ra.y, aw: ra.width, ah: ra.height, ab: ra.bottom,
                        bx: rb.x, by: rb.y,
                        rects: a.getClientRects().length,
                        ow: a.offsetWidth, oh: a.offsetHeight,
                    };
                    // A style write inside the iframe doc must re-measure.
                    a.style.width = '60px';
                    return { before: before, aw2: a.getBoundingClientRect().width };
                })())"#,
            )
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let b = &v["before"];
    assert_eq!(
        b["ax"],
        serde_json::json!(8),
        "body UA margin 8px, in the iframe's own viewport"
    );
    assert_eq!(b["ay"], serde_json::json!(8));
    assert_eq!(b["aw"], serde_json::json!(100));
    assert_eq!(b["ah"], serde_json::json!(20));
    assert_eq!(b["ab"], serde_json::json!(28), "bottom = y + height");
    assert_eq!(b["bx"], serde_json::json!(8));
    assert_eq!(
        b["by"],
        serde_json::json!(28),
        "second block stacks below the first"
    );
    assert_eq!(b["rects"], serde_json::json!(1));
    assert_eq!(
        b["ow"],
        serde_json::json!(100),
        "offsetWidth rides the same sub-run"
    );
    assert_eq!(b["oh"], serde_json::json!(20));
    assert_eq!(
        v["aw2"],
        serde_json::json!(60),
        "a style write inside the iframe doc must invalidate the sub-run cache"
    );
}

// The two coordinate worlds must not bleed into each other: the iframe
// doc's own html/body span the IFRAME's 300x150 default box, while
// host-page elements keep measuring against the host viewport.
#[test]
#[cfg(feature = "screenshot")]
fn test_iframe_document_viewport_is_independent() {
    let mut rt = setup_runtime(
        r#"<html><head><style>body{margin:0}#main{width:400px;height:50px}</style></head><body><div id="main"></div></body></html>"#,
    );
    let out = rt
        .evaluate(
            r#"JSON.stringify((function() {
                    const f = document.createElement('iframe');
                    document.body.appendChild(f);
                    const doc = f.contentDocument;
                    doc.body.innerHTML = '<p>hi</p>';
                    const rRoot = doc.documentElement.getBoundingClientRect();
                    const rBody = doc.body.getBoundingClientRect();
                    const main = document.getElementById('main').getBoundingClientRect();
                    return {
                        rootW: rRoot.width, rootH: rRoot.height,
                        bodyW: rBody.width, bodyH: rBody.height,
                        mainW: main.width, mainH: main.height,
                    };
                })())"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["rootW"],
        serde_json::json!(300),
        "iframe html spans the 300x150 default box"
    );
    assert_eq!(v["rootH"], serde_json::json!(150));
    assert_eq!(v["bodyW"], serde_json::json!(300));
    assert_eq!(v["bodyH"], serde_json::json!(150));
    assert_eq!(
        v["mainW"],
        serde_json::json!(400),
        "host-page geometry unchanged"
    );
    assert_eq!(v["mainH"], serde_json::json!(50));
}

#[test]
fn test_iframe_document_event_listeners() {
    // Upstream 2e3f5d8: addEventListener/removeEventListener/dispatchEvent
    // on an iframe document used to be no-ops.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const iframe = document.createElement('iframe');
            document.body.appendChild(iframe);
            const doc = iframe.contentDocument;
            let calls = 0;
            const listener = () => calls++;
            doc.addEventListener('probe', listener);
            doc.dispatchEvent(new Event('probe'));
            const afterRegister = calls;
            doc.addEventListener('probe', listener);
            doc.addEventListener('probe', listener);
            doc.dispatchEvent(new Event('probe'));
            const afterDuplicate = calls;
            doc.removeEventListener('probe', listener);
            doc.dispatchEvent(new Event('probe'));
            const afterRemove = calls;
            doc.addEventListener('cancelme', e => e.preventDefault());
            const cancelReturn = doc.dispatchEvent(new Event('cancelme', { cancelable: true }));
            const plainReturn = doc.dispatchEvent(new Event('nolisteners'));
            return [!!doc, afterRegister, afterDuplicate, afterRemove, cancelReturn, plainReturn];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([true, 1, 2, 2, false, true]));
}

#[tokio::test(flavor = "current_thread")]
async fn test_iframe_load_reaches_onload_and_addeventlistener() {
    // Upstream 2e3f5d8: iframe load used to call el.onload() directly,
    // bypassing addEventListener('load') listeners.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            return await new Promise(resolve => {
                const iframe = document.createElement('iframe');
                const events = [];
                // Append first: the initial about:blank load fires inside the
                // append (Chrome), before these handlers exist.
                document.body.appendChild(iframe);
                iframe.onload = () => {
                    events.push('property');
                    Promise.resolve().then(() => resolve(events));
                };
                iframe.addEventListener('load', () => events.push('listener'));
                // Unroutable port: fetch rejects, the catch path still fires load.
                iframe.src = 'http://127.0.0.1:1/';
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["property", "listener"])
    );
}

/// #121: an iframe that enters the document with its src already set as
/// an ATTRIBUTE (how React and most framework DOM writers build one)
/// must start its navigation at insertion — the IDL `iframe.src = …`
/// setter was the only load path, so the tmall sucai-selector iframe
/// sat registered (window[N]) with its document never fetched: a
/// 900x600 frame with an empty body.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn test_iframe_with_src_attribute_loads_on_insertion() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let frame_body = "<html><body>frame-MARKER-42</body></html>";
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            frame_body.len(),
            frame_body
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let script = format!(
        r#"async () => {{
            const f = document.createElement('iframe');
            // Attribute only — never touches the src property, exactly like
            // a framework DOM writer.
            f.setAttribute('src', 'http://127.0.0.1:{port}/frame.html');
            let loaded = 'no';
            f.addEventListener('load', () => {{ loaded = 'yes'; }}, {{ once: true }});
            document.body.appendChild(f);
            await new Promise(r => setTimeout(r, 100));
            const doc = f.contentDocument;
            return {{
                loaded,
                bodyText: doc && doc.body ? (doc.body.textContent || '').trim() : 'no-doc',
            }};
        }}"#
    );
    let result = rt
        .call_function_on_remote(&script, None, &[], true, true)
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(
        v["loaded"],
        serde_json::json!("yes"),
        "frame load event must fire: {v}"
    );
    assert_eq!(
        v["bodyText"],
        serde_json::json!("frame-MARKER-42"),
        "frame document must be fetched and parsed: {v}"
    );
}

/// #268: iframe srcdoc — the child document comes from the attribute
/// (about:srcdoc, inherits the parent's origin, wins over src), its
/// inline scripts RUN (a document load, not innerHTML), and the load
/// event fires only after the scripts finished (obscura #1265).
#[tokio::test(flavor = "current_thread")]
async fn test_iframe_srcdoc_document_scripts_and_load() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let script = r##"async () => {
            const f = document.createElement('iframe');
            f.setAttribute('srcdoc', '<head><title>t-42</title><script>window.__srcdocRan = document.querySelector("#srcdoc-el") ? "dom-ready" : "no-dom";<\/script></head><body><div id="srcdoc-el">SRCDOC-BODY-42</div><script>self.__srcdocWinIsSelf = (self === window);<\/script></body>');
            // srcdoc present must win over src (never fetched).
            f.setAttribute('src', 'http://127.0.0.1:1/never-fetched');
            const events = [];
            f.addEventListener('load', () => events.push('load:' + (f.contentWindow.__srcdocRan || 'missed')));
            document.body.appendChild(f);
            await new Promise(r => setTimeout(r, 50));
            const doc = f.contentDocument;
            const win = f.contentWindow;
            return {
                events,
                url: doc ? doc.URL : 'no-doc',
                title: doc ? doc.title : 'no-doc',
                bodyText: doc && doc.body ? (doc.body.textContent || '').trim() : 'no-body',
                scriptRan: win ? win.__srcdocRan : 'no-win',
                winIsSelf: win ? String(win.__srcdocWinIsSelf) : 'no-win',
                docReachable: doc !== null && doc !== undefined,
                locHref: win ? win.location.href : 'no-win',
                locProtocol: win ? win.location.protocol : 'no-win',
                originInherited: win ? String(win.location.origin === location.origin) : 'no-win',
            };
        }"##;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["events"],
        serde_json::json!(["load:dom-ready"]),
        "load must fire after the scripts ran: {v}"
    );
    assert_eq!(
        v["url"],
        serde_json::json!("about:srcdoc"),
        "srcdoc browsing context URL: {v}"
    );
    assert_eq!(
        v["title"],
        serde_json::json!("t-42"),
        "head content (title) must survive parsing: {v}"
    );
    // Chrome truth: textContent includes <script> bodies, so assert the
    // marker is present rather than exact equality.
    assert!(
        v["bodyText"]
            .as_str()
            .unwrap_or("")
            .contains("SRCDOC-BODY-42"),
        "srcdoc body must parse: {v}"
    );
    assert_eq!(
        v["scriptRan"],
        serde_json::json!("dom-ready"),
        "inline scripts in srcdoc must run against the frame document: {v}"
    );
    assert_eq!(
        v["winIsSelf"],
        serde_json::json!("true"),
        "self === window inside the frame: {v}"
    );
    assert_eq!(
        v["docReachable"],
        serde_json::json!(true),
        "srcdoc contentDocument is same-origin, never null: {v}"
    );
    assert_eq!(
        v["locHref"],
        serde_json::json!("about:srcdoc"),
        "frame location.href: {v}"
    );
    assert_eq!(
        v["locProtocol"],
        serde_json::json!("about:"),
        "frame location.protocol: {v}"
    );
    assert_eq!(
        v["originInherited"],
        serde_json::json!("true"),
        "about:srcdoc inherits the parent origin: {v}"
    );
}

/// #268 second path: `iframe.srcdoc = …` (IDL property, connected
/// element) navigates the frame to the inline document. `srcdoc` is a
/// NON-nullable DOMString: `= null` writes the string 'null' (measured in
/// real Chrome 145, htmlreflect-truth IFrame.srcdoc set.null; the earlier
/// "null removes the attribute" reading of it as nullable was wrong) and
/// removing the attribute is what reflects as ''.
#[tokio::test(flavor = "current_thread")]
async fn test_iframe_srcdoc_property_setter_navigates() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r##"async () => {
            const f = document.createElement('iframe');
            document.body.appendChild(f);
            f.srcdoc = '<body><div id="p">P-42</div><script>window.__p = 7;<\/script></body>';
            await new Promise(r => setTimeout(r, 30));
            const first = {
                body: f.contentDocument ? (f.contentDocument.body.textContent || '').trim() : 'no-doc',
                ran: f.contentWindow ? f.contentWindow.__p : 'no-win',
                reflected: f.srcdoc.indexOf('P-42') !== -1,
            };
            f.srcdoc = null;
            const nullAttr = f.getAttribute('srcdoc');
            const nullRead = f.srcdoc;
            f.removeAttribute('srcdoc');
            return { first, nullAttr, nullRead, reflectedEmpty: f.srcdoc === '' };
        }"##;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert!(
        v["first"]["body"].as_str().unwrap_or("").contains("P-42"),
        "srcdoc property setter must navigate: {v}"
    );
    assert_eq!(
        v["first"]["ran"],
        serde_json::json!(7),
        "srcdoc scripts must run via the property path too: {v}"
    );
    assert_eq!(
        v["first"]["reflected"],
        serde_json::json!(true),
        "srcdoc getter reflects the attribute: {v}"
    );
    assert_eq!(
        v["nullAttr"],
        serde_json::json!("null"),
        "srcdoc = null writes the string 'null' (DOMString): {v}"
    );
    assert_eq!(
        v["nullRead"],
        serde_json::json!("null"),
        "srcdoc reads back 'null': {v}"
    );
    assert_eq!(
        v["reflectedEmpty"],
        serde_json::json!(true),
        "absent srcdoc reflects as '': {v}"
    );
}

/// #48 fourth face (2026-10-18 sweep): named access onto an iframe yields
/// the nested browsing context's WindowProxy, never the element — Chrome's
/// `window.ff === frames[0]` for BOTH id and name paths (HTML spec named
/// property access; moli's 2026-10 same-origin named-child cluster is the
/// neighboring face). window.frames === window, so frames['n'] must land on
/// the same object, and the winKeys invisibility contract holds for it too.
#[test]
fn window_named_access_iframe_yields_content_window() {
    let mut rt = setup_runtime(
        "<html><body><iframe id=\"ifr\" name=\"fr\" src=\"about:blank\"></iframe></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
        (function () { try {
        const el = document.querySelector('iframe[name=fr]');
        const w = el.contentWindow;
        globalThis.__out = JSON.stringify([
            window.fr === w,
            window.ifr === w,                    // id path coerces the same way
            window.fr === el,                    // the element itself must NOT leak through
            String(window.fr).slice(0, 7) === '[object',
            window.frames !== undefined && window.frames['fr'] === window.fr,
            Object.getOwnPropertyNames(window).includes('fr'),
            Object.getOwnPropertyNames(Object.getPrototypeOf(window)).includes('fr'),
        ]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#,
        )
        .unwrap();
    let s = out.as_str().unwrap();
    assert!(
        !s.starts_with("ERR"),
        "iframe named access probe threw: {}",
        s
    );
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!([true, true, false, true, true, false, false]),
        "iframe named access must expose the content window (id and name alike), never the element"
    );
}

/// 2026-10-18 sweep follow-up (moli ba5cd6f): a browsing context only exists
/// while the iframe is connected — never-connected AND removed elements read
/// null from both contentWindow and contentDocument. The lazy synthetic
/// `_IframeWindow`/`_IframeDoc` pair used to leak for disconnected elements.
#[test]
fn disconnected_iframes_have_no_browsing_context() {
    let mut rt = setup_runtime("<html><body><p>x</p></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () { try {
        const f = document.createElement('iframe');
        const before = [f.contentWindow === null, f.contentDocument === null];
        document.body.appendChild(f);
        const connected = [!!f.contentWindow, !!f.contentDocument];
        f.remove();
        const after = [f.contentWindow === null, f.contentDocument === null];
        globalThis.__out = JSON.stringify([before, connected, after]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#,
        )
        .unwrap();
    let s = out.as_str().unwrap();
    assert!(
        !s.starts_with("ERR"),
        "disconnected iframe probe threw: {}",
        s
    );
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!([[true, true], [true, true], [true, true]]),
        "iframe browsing context exists only while connected — null before insertion and after removal"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_blank_and_shadow_iframes_fire_load() {
    // #282: an iframe with nothing to navigate to loads its initial
    // about:blank synchronously on insertion; iframes in a connected
    // host's shadow tree load too.
    let mut rt = setup_runtime("<html><body><div id=\"host\"></div></body></html>");
    let v = rt
            .evaluate(
                "(() => { const r = {};
                  const a = document.createElement('iframe'); a.onload = () => { r.before = (r.before || 0) + 1; };
                  document.body.append(a);
                  const b = document.createElement('iframe'); document.body.append(b);
                  b.addEventListener('load', () => { r.after = true; });
                  const c = document.createElement('div'); document.body.append(c);
                  c.innerHTML = '<iframe id=q></iframe>'; r.inner = !!c.firstChild.contentWindow;
                  const d = document.createElement('iframe'); d.src = 'about:blank'; d.onload = () => { r.explicit = true; };
                  document.body.append(d);
                  const box = document.createElement('div'); document.body.append(box); box.append(a);
                  const sr = host.attachShadow({ mode: 'open' });
                  const s = document.createElement('iframe'); s.srcdoc = '<p>x</p>'; s.onload = () => { globalThis.__shadowSrcdocLoaded = true; };
                  sr.append(s);
                  const t = document.createElement('iframe'); t.onload = () => { r.shadowBlank = true; }; sr.append(t);
                  return r; })()",
            )
            .unwrap();
    assert_eq!(
        v["before"], 2,
        "heard on insertion and again when moved to a new parent"
    );
    assert!(
        v["after"].is_null(),
        "listener added after the synchronous load hears nothing"
    );
    assert_eq!(
        v["explicit"], true,
        "src=about:blank is the same initial load"
    );
    assert_eq!(v["inner"], true);
    assert_eq!(
        v["shadowBlank"], true,
        "blank iframe in a shadow tree loads on insertion"
    );
    let _ = rt.run_event_loop_bounded(200).await;
    // The srcdoc frame in the shadow tree loads on a later turn.
    assert_eq!(
        rt.evaluate("globalThis.__shadowSrcdocLoaded === true")
            .unwrap(),
        true
    );
}

/// JS-PLATFORM-ARCH M3 B2c: the iframe realm legs ride the NATIVE EventTarget
/// core. The frame document IS a Document (chain → Document.prototype →
/// Node.prototype → EventTarget.prototype) and the frame window IS a Window
/// (chain → Window.prototype → EventTarget.prototype — the link the old
/// bootstrap-side setPrototypeOf never made, it died in a TDZ catch at load).
/// Frame-content propagation: an event dispatched inside the frame reaches the
/// frame's document and window listeners (the frame analog of the document →
/// window edge) and stops there — it never leaks into the host document.
#[tokio::test(flavor = "current_thread")]
async fn test_iframe_realm_dispatch_rides_native_core() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let script = r##"async () => {
        const f = document.createElement('iframe');
        f.setAttribute('srcdoc', '<body><div id="in-frame">x</div></body>');
        document.body.appendChild(f);
        await new Promise(r => setTimeout(r, 30));
        const doc = f.contentDocument, win = f.contentWindow;
        const shape = {
            docIsDoc: doc instanceof Document,
            docIsNode: doc instanceof Node,
            winIsWin: win instanceof Window,
            winIsET: win instanceof EventTarget,
            addIsNative: win.addEventListener === EventTarget.prototype.addEventListener
                && doc.addEventListener === EventTarget.prototype.addEventListener,
            docOldStoreGone: !Object.prototype.hasOwnProperty.call(doc, '_listeners'),
        };
        // Frame-content propagation: bubbling from an element inside the
        // frame reaches the frame doc, the frame window, frame on* handlers
        // — and does NOT reach the host document or main window.
        const order = [];
        const el = doc.getElementById('in-frame');
        el.addEventListener('tick', () => order.push('el'));
        doc.addEventListener('tick', () => order.push('doc'));
        win.addEventListener('tick', () => order.push('win'));
        doc.onclick = () => order.push('doc-onclick');
        el.dispatchEvent(new Event('tick', { bubbles: true }));
        doc.dispatchEvent(new Event('click'));
        const hostHeard = [];
        document.addEventListener('tick', () => hostHeard.push('host-doc'));
        window.addEventListener('tick', () => hostHeard.push('host-win'));
        el.dispatchEvent(new Event('tick', { bubbles: true }));
        // Dedup + remove on the frame faces (native store semantics).
        let n = 0; const bump = () => n++;
        doc.addEventListener('zz', bump); doc.addEventListener('zz', bump);
        doc.dispatchEvent(new Event('zz'));
        doc.removeEventListener('zz', bump); doc.dispatchEvent(new Event('zz'));
        return { shape, order, hostHeard, n };
    }"##;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["shape"],
        serde_json::json!({
            "docIsDoc": true, "docIsNode": true, "winIsWin": true, "winIsET": true,
            "addIsNative": true, "docOldStoreGone": true,
        }),
        "frame faces must chain to the native core: {v}"
    );
    assert_eq!(
        v["order"],
        serde_json::json!(["el", "doc", "win", "doc-onclick", "el", "doc", "win"]),
        "frame propagation order (tick dispatched twice; the second run must not reach the host): {v}"
    );
    assert_eq!(
        v["hostHeard"],
        serde_json::json!([]),
        "frame events must not leak into the host document/window: {v}"
    );
    assert_eq!(
        v["n"], 1,
        "native dedup/remove semantics on the frame doc: {v}"
    );
}
