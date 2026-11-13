//! 脚本加载：dynamic script/module loader/import 策略
use super::common::setup_runtime;

#[test]
fn test_script_execution() {
    let mut rt = setup_runtime("<ul><li>A</li><li>B</li></ul>");
    rt.execute_script(
        "test",
        r#"
            globalThis.__result = [];
            document.querySelectorAll('li').forEach(function(el) {
                globalThis.__result.push(el.textContent);
            });
        "#,
    )
    .unwrap();
    let result = rt.evaluate("globalThis.__result").unwrap();
    assert_eq!(result, serde_json::json!(["A", "B"]));
}

/// Regression test for #147: a TypeError in one script must not poison
/// the runtime so that subsequent scripts (or DOM queries) collapse to
/// empty. The reporter saw `--dump text` return 1 byte after offside.js
/// crashed; that cascade should never happen.
#[test]
fn script_typeerror_does_not_poison_subsequent_execution() {
    let mut rt = setup_runtime("<html><body><p id=hit>BODY_TEXT</p></body></html>");

    // 1. First script throws the same flavor of error offside.js produced
    //    (`Cannot read properties of undefined (reading 'classList')`).
    let err = rt
        .execute_script("buggy", "var x; x.classList.add('y');")
        .unwrap_err();
    assert!(
        err.contains("classList") || err.contains("undefined"),
        "expected classList/undefined error, got: {}",
        err
    );

    // 2. The runtime must still be usable: a follow-up script runs.
    rt.execute_script("ok", "globalThis.__after_error = 'still alive';")
        .unwrap();
    let result = rt.evaluate("globalThis.__after_error").unwrap();
    assert_eq!(result, serde_json::json!("still alive"));

    // 3. DOM queries still work after the script error.
    let text = rt
        .evaluate("document.querySelector('#hit').textContent")
        .unwrap();
    assert_eq!(text, serde_json::json!("BODY_TEXT"));
}

// #98: script/anchor/option .text — Chrome's real IDL attributes. PDD's
// goods-list tracker scan reads el.text on every script without a src;
// external scripts have no children so Chrome answers "" where we
// answered undefined and its .indexOf threw inside the tracker
// inventory.
#[tokio::test(flavor = "current_thread")]
async fn script_anchor_option_text_idl_faces() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"(() => {
                    const s = document.createElement('script');
                    const empty = s.text;
                    s.text = 'var x = 1';
                    const inline = s.text + '/' + s.textContent;
                    const a = document.createElement('a');
                    a.textContent = ' hi ';
                    const o = document.createElement('option');
                    o.textContent = 'opt';
                    return [typeof empty + ':' + JSON.stringify(empty), inline, JSON.stringify(a.text),
                            o.text,
                            Object.getOwnPropertyDescriptor(HTMLScriptElement.prototype, 'text') !== undefined,
                            Object.getOwnPropertyDescriptor(HTMLAnchorElement.prototype, 'text') !== undefined,
                            Object.getOwnPropertyDescriptor(HTMLOptionElement.prototype, 'text') !== undefined,
                            Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'text') === undefined].join('#');
                })()"#,
                false,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("string:\"\"#var x = 1/var x = 1#\" hi \"#opt#true#true#true#true"),
        ".text lives on the three interface prototypes, mirrors textContent, defaults to \"\""
    );
}

/// obscura #849 class: the ES module loader is a second page-controlled
/// fetch path, so the #581 body cap must hold there too — same env knob,
/// same streaming check, and the failure surfaces as a catchable
/// import() rejection (not a silent empty module).
#[tokio::test(flavor = "current_thread")]
async fn module_import_rejects_body_over_limit() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    std::env::set_var("DITING_FETCH_BODY_LIMIT", "1024");

    // Advertised Content-Length over the cap: rejected before buffering.
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\ncontent-type: text/javascript\r\ncontent-length: 99999999\r\nconnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        });
        let mut rt = setup_runtime("<html><body></body></html>");
        rt.set_url(&format!("http://127.0.0.1:{}/", port));
        let result = rt
            .call_function_on_remote(
                r#"async () => {
                        try { await import(document.URL + "big.mjs"); return "resolved"; }
                        catch (e) { return "rejected: " + e.message; }
                    }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
        let v = result.value.unwrap();
        assert_eq!(
                v,
                serde_json::json!(
                    "rejected: Module http://127.0.0.1:PORT/big.mjs response body too large: content-length 99999999 exceeds limit 1024 bytes"
                    .replace("PORT", &port.to_string())
                ),
                "oversized advertised Content-Length must reject the import before buffering"
            );
    }

    // No Content-Length: the stream itself must cross the cap and reject.
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\ncontent-type: text/javascript\r\nconnection: close\r\n\r\nexport const ok = true;\n";
            let _ = stream.write_all(resp.as_bytes());
            for _ in 0..64 {
                if stream.write_all(&[b'x'; 128]).is_err() {
                    break; // client hung up once the cap tripped
                }
            }
            let _ = stream.flush();
        });
        let mut rt = setup_runtime("<html><body></body></html>");
        rt.set_url(&format!("http://127.0.0.1:{}/", port));
        let result = rt
            .call_function_on_remote(
                r#"async () => {
                        try { await import(document.URL + "stream.mjs"); return "resolved"; }
                        catch (e) { return "rejected: " + e.message; }
                    }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
        let v = result.value.unwrap();
        let msg = v.as_str().unwrap();
        assert!(
            msg.starts_with("rejected: ") && msg.contains("exceeded limit of 1024 bytes"),
            "unbounded stream over the cap must reject the import, got: {}",
            msg
        );
    }

    std::env::remove_var("DITING_FETCH_BODY_LIMIT");
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

/// A module above obscura's hardcoded 33554432-byte cap must load fine
/// under our default (the Angular 18 `main` bundle from the issue is
/// 37.9 MB) — the knob is the only policy, and the default sits above
/// routine production bundles.
#[tokio::test(flavor = "current_thread")]
async fn module_import_allows_body_above_upstream_cap() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    std::env::remove_var("DITING_FETCH_BODY_LIMIT");

    const BIG: usize = 34 * 1024 * 1024; // 34 MiB > upstream's 32 MiB cap
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        // A 34 MiB line comment: the full body must be downloaded (that
        // is the point) without V8 parsing megabytes of tokens.
        let body = format!(
            "//{}\nglobalThis.__BIG_MODULE__ = true;\nexport const ok = true;\n",
            "x".repeat(BIG)
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/javascript\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();
    });
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    await import(document.URL + "big.mjs");
                    return "resolved:" + (globalThis.__BIG_MODULE__ === true);
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("resolved:true"),
        "a 34 MiB module must load and execute under the default cap"
    );

    std::env::remove_var("DITING_FETCH_BODY_LIMIT");
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

/// fetch(): file:// and private/internal hosts rejected, private hosts
/// reachable again once the operator opts in.
#[tokio::test(flavor = "current_thread")]
async fn module_import_honors_fetch_url_policy() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    std::env::remove_var("DITING_FETCH_BODY_LIMIT");

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("http://example.invalid/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const out = {};
                    try { await import("file:///etc/hostname"); out.file = "resolved"; }
                    catch (e) { out.file = e.message; }
                    try { await import("http://127.0.0.1:9/x.mjs"); out.loopback = "resolved"; }
                    catch (e) { out.loopback = e.message; }
                    return out;
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let out = result.value.unwrap();
    assert!(
        out["file"].as_str().unwrap().contains("net::ERR_FAILED")
            || out["file"].as_str().unwrap().contains("Failed to fetch"),
        "file:// import must be rejected by scheme (Chrome-shaped), got: {}",
        out["file"]
    );
    assert!(
        out["loopback"]
            .as_str()
            .unwrap()
            .contains("Access to private/internal"),
        "loopback import must be rejected by the private-network policy, got: {}",
        out["loopback"]
    );

    // Operator opt-in reopens private hosts for the module path too.
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let body = "globalThis.__PRIVATE_OK__ = true;\nexport const ok = true;\n";
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/javascript\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();
    });
    rt.set_url(&format!("http://127.0.0.1:{}/", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    await import(document.URL + "local.mjs");
                    return "resolved:" + (globalThis.__PRIVATE_OK__ === true);
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("resolved:true"),
        "with the opt-in env set, a loopback module must load"
    );

    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

#[test]
fn module_loader_stores_proxy_for_dynamic_imports() {
    use crate::module_loader::DitingModuleLoader;
    let loader = DitingModuleLoader::with_proxy(
        "https://example.com/",
        Some("http://proxy.test:8080".to_string()),
    );
    assert_eq!(loader.proxy_url.as_deref(), Some("http://proxy.test:8080"));
    assert_eq!(loader.base_url, "https://example.com/");

    // Default constructor must keep the historical "no proxy" behaviour.
    let direct = DitingModuleLoader::new("https://example.com/");
    assert_eq!(direct.proxy_url, None);
}

#[test]
fn test_script_runs_once_across_dom_move() {
    // Moving a <script> in the DOM must not execute its inline body a
    // second time (upstream 41a8e1c — "already started" flag).
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let result = rt.evaluate(r#"
            const host = document.getElementById('host');
            window.__count = 0;
            const s = document.createElement('script');
            s.textContent = 'window.__count = (window.__count || 0) + 1;';
            host.appendChild(s);
            const afterFirst = window.__count;
            host.removeChild(s);
            host.appendChild(s);
            const afterMove = window.__count;
            const afterReinsert = (() => { host.removeChild(s); host.appendChild(s); return window.__count; })();
            return [afterFirst, afterMove, afterReinsert];
        "#).unwrap();
    assert_eq!(result, serde_json::json!([1, 1, 1]));
}

#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_data_url_script_executes() {
    // Upstream 0c4740a + f841205: op_fetch_url's HTTP client cannot fetch
    // the data: scheme, so dynamic <script src="data:..."> never ran. The
    // decoder accepts any MIME, %-escapes, fragments, unpadded base64, and
    // non-ASCII via a UTF-8 round-trip — and load fires on every path.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            let loads = 0;
            const mk = (url) => {
                const s = document.createElement('script');
                s.setAttribute('src', url);
                s.addEventListener('load', () => loads++);
                s.addEventListener('error', () => loads -= 100);
                document.body.appendChild(s);
            };
            mk('data:,window.__a=1');
            mk("data:text/plain,window.__g='%C3%A9'");
            mk("data:text/javascript,window.__h='é'");
            mk('data:text/javascript,window.__i=9#frag');
            mk('data:text/javascript;base64,d2luZG93Ll9fYz0z');
            mk('data:text/javascript;base64,d2luZG93Ll9fZD00NA');
            await new Promise(r => setTimeout(r, 20));
            return [window.__a, window.__g, window.__h, window.__i, window.__c, window.__d, loads];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([1, "é", "é", 9, 3, 44, 6])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_data_url_script_invalid_base64_errors() {
    // Upstream f841205: a payload whose length % 4 === 1 can never be
    // valid base64; the decoder must throw instead of executing garbage,
    // and the script element fires error without evaluating anything.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            let errors = 0;
            const mk = (url) => {
                const s = document.createElement('script');
                s.setAttribute('src', url);
                s.addEventListener('error', () => errors++);
                document.body.appendChild(s);
            };
            mk('data:text/javascript;base64,AAAAA');
            mk('data:text/javascript;base64,ab!c');
            mk('data:,window.__ok=1');
            await new Promise(r => setTimeout(r, 20));
            return [errors, window.__ok];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([2, 1]));
}

// b12405d closed fetch()/XHR and classic <script src=data:>; the module
// path was the remaining gap — import() and <script type=module> died in
// the Rust module loader (validate_fetch_url/the client). The loader now
// resolves data: locally, with Chromium's module MIME gate: empty essence
// reads as text/plain and is refused, classic-style MIME blindness does
// not apply here.
#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_import_data_url_module() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const out = {};
            const imp = (k, u) => import(u)
                .then(m => { out[k] = String(m.default) + ':' + m.n; })
                .catch(e => { out[k] = 'ERR'; });
            await imp('a', 'data:text/javascript,export default 41; export const n = 1;');
            await imp('b', 'data:application/javascript;base64,ZXhwb3J0IGRlZmF1bHQgNDI7IGV4cG9ydCBjb25zdCBuID0gMjs=');
            // A charset param between the essence and the payload is fine.
            await imp('c', 'data:text/javascript;charset=utf-8,export default 43; export const n = 4;');
            await imp('d', 'data:,export default 1;');
            await imp('e', 'data:text/plain,export default 1;');
            return out;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let out = result.value.unwrap();
    assert_eq!(out["a"], "41:1");
    assert_eq!(out["b"], "42:2");
    assert_eq!(out["c"], "43:4");
    // Empty essence reads as text/plain and is refused, like Chrome.
    assert_eq!(out["d"], "ERR");
    assert_eq!(out["e"], "ERR");
}

// import(blob:) resolves through the Rust-side mirror of
// URL.createObjectURL: works after create, rejects after revoke, and a
// non-JavaScript blob type is refused like Chrome refuses it.
#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_import_blob_url_module() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const out = {};
            const url = URL.createObjectURL(
                new Blob(['export default 7; export const n = 3;'], { type: 'text/javascript' }));
            try {
                const m = await import(url);
                out.ok = String(m.default) + ':' + m.n;
            } catch (e) { out.ok = 'ERR'; }
            // Revoked before it was ever imported: the fetch reaches the
            // mirror and misses, like Chrome. (A URL imported once and then
            // revoked keeps resolving from the module map — also Chrome.)
            const dead = URL.createObjectURL(
                new Blob(['export default 9;'], { type: 'text/javascript' }));
            URL.revokeObjectURL(dead);
            try { await import(dead); out.after = 'resolved'; }
            catch (e) { out.after = 'rejected'; }
            const u2 = URL.createObjectURL(new Blob(['export default 1;'], { type: 'text/plain' }));
            try { await import(u2); out.mime = 'resolved'; }
            catch (e) { out.mime = 'rejected'; }
            return out;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let out = result.value.unwrap();
    assert_eq!(out["ok"], "7:3");
    assert_eq!(out["after"], "rejected");
    assert_eq!(out["mime"], "rejected");
}

// Classic dynamic <script src="blob:"> executes the blob body (no MIME
// gate, like every other classic path); a revoked URL fires error and
// evaluates nothing.
#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_blob_url_script_executes() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            let loads = 0, errors = 0;
            const url = URL.createObjectURL(
                new Blob(['window.__bs = 5;'], { type: 'text/javascript' }));
            const s = document.createElement('script');
            s.setAttribute('src', url);
            s.addEventListener('load', () => loads++);
            s.addEventListener('error', () => errors++);
            document.body.appendChild(s);
            const dead = URL.createObjectURL(new Blob(['window.__dead = 1;']));
            URL.revokeObjectURL(dead);
            const s2 = document.createElement('script');
            s2.setAttribute('src', dead);
            s2.addEventListener('error', () => errors++);
            document.body.appendChild(s2);
            await new Promise(r => setTimeout(r, 20));
            return [window.__bs, window.__dead === undefined, loads, errors];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([5, true, 1, 1]));
}

/// Upstream f61493f: the HTML script-fetch algorithm treats an
/// unsuccessful HTTP response as a network error. A 404 body (here, one
/// that would clobber a global if it ran) must never become script source.
#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_script_non_2xx_body_not_evaluated() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let body = b"window.__leak = 1;";
        let response = format!(
            "HTTP/1.1 404 Not Found\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let script = format!(
        r#"async () => {{
            let errors = 0, loads = 0;
            const s = document.createElement('script');
            s.setAttribute('src', 'http://127.0.0.1:{port}/missing.js');
            s.addEventListener('error', () => errors++);
            s.addEventListener('load', () => loads++);
            document.body.appendChild(s);
            await new Promise(r => setTimeout(r, 50));
            return [errors, loads, window.__leak === undefined];
        }}"#
    );
    let result = rt
        .call_function_on_remote(&script, None, &[], true, true)
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(result.value.unwrap(), serde_json::json!([1, 0, true]));
}

/// #121: the dynamic-script load path used to call `script.onload(...)`
/// manually *and* then dispatchEvent — but element dispatch generically
/// routes the `on<type>` property handler, so the property handler ran
/// twice (addEventListener listeners once). CKEditor's scriptLoader
/// deletes its pending-callback map on the first completion, so the
/// second run threw `Cannot read properties of undefined (reading
/// 'length')` and killed the tmall source-import editor mount. A real
/// browser fires onload/onerror exactly once; the property handler and
/// listener must both see exactly one event.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_script_load_and_error_fire_onload_exactly_once() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for (status, body) in [
            ("200 OK", b"window.__ok = 1;".as_slice()),
            ("404 Not Found", b"window.__leak = 1;".as_slice()),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: text/javascript\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let script = format!(
        r#"async () => {{
            const tally = {{ load_prop: 0, load_listen: 0, err_prop: 0, err_listen: 0 }};
            const s1 = document.createElement('script');
            s1.setAttribute('src', 'http://127.0.0.1:{port}/ok.js');
            s1.onload = () => tally.load_prop++;
            s1.addEventListener('load', () => tally.load_listen++);
            document.body.appendChild(s1);
            const s2 = document.createElement('script');
            s2.setAttribute('src', 'http://127.0.0.1:{port}/missing.js');
            s2.onerror = () => tally.err_prop++;
            s2.addEventListener('error', () => tally.err_listen++);
            document.body.appendChild(s2);
            await new Promise(r => setTimeout(r, 50));
            return tally;
        }}"#
    );
    let result = rt
        .call_function_on_remote(&script, None, &[], true, true)
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    // Exactly one fire per handler, per side — [2, 1] on either row is
    // the double-fire signature the old code produced.
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({"load_prop": 1, "load_listen": 1, "err_prop": 1, "err_listen": 1})
    );
}

/// Upstream a6bb741: a dynamic external script slower than the settle
/// loop's 500ms fast-path deadline must still be visible as pending while
/// in flight (so the loop keeps pumping) and must land once its fetch
/// resolves — including after a failed fetch, where the finally-bracket
/// must return the counter to zero.
#[tokio::test(flavor = "current_thread")]
async fn test_slow_dynamic_script_visible_as_pending_until_lands() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let body = b"window.__slow = 1;";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/javascript\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let insert = format!(
        r#"
            const s = document.createElement('script');
            s.setAttribute('src', 'http://127.0.0.1:{port}/slow.js');
            document.body.appendChild(s);
        "#
    );
    rt.evaluate(&insert).unwrap();

    // Pump the event loop past 500ms while the 300ms-slow fetch is in
    // flight; the counter must be observed live at least once, the script
    // must land, and the counter must drain back to zero afterwards.
    let start = tokio::time::Instant::now();
    let mut saw_pending = false;
    while start.elapsed() < std::time::Duration::from_millis(2_000) {
        let _ =
            tokio::time::timeout(std::time::Duration::from_millis(20), rt.run_event_loop()).await;
        if rt.has_pending_dynamic_scripts() {
            saw_pending = true;
        }
        if saw_pending
            && !rt.has_pending_dynamic_scripts()
            && rt.evaluate("window.__slow").unwrap().as_f64() == Some(1.0)
        {
            break;
        }
    }
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert!(
        saw_pending,
        "slow dynamic script fetch should be observable as pending"
    );
    assert!(
        !rt.has_pending_dynamic_scripts(),
        "counter must drain after the fetch lands"
    );
    assert_eq!(rt.evaluate("window.__slow").unwrap().as_f64(), Some(1.0));
}

#[test]
fn test_thrown_error_in_one_script_does_not_stop_later_scripts() {
    // Upstream 5c3d560 (regression for #355/#358): an uncaught throw in
    // one inline script must not prevent later independent scripts from
    // running — the babel-polyfill double-load pattern.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.execute_script("s1", "globalThis.__ran1 = true;")
        .unwrap();
    let err = rt
        .execute_script(
            "s2",
            "throw new Error('only one instance of babel-polyfill is allowed');",
        )
        .unwrap_err();
    assert!(
        err.contains("babel-polyfill"),
        "expected the thrown message, got: {}",
        err
    );
    rt.execute_script("s3", "globalThis.__ran3 = true;")
        .unwrap();
    let ran = rt
        .evaluate("[globalThis.__ran1 === true, globalThis.__ran3 === true]")
        .unwrap();
    assert_eq!(ran, serde_json::json!([true, true]));
}

/// The empty-insert-then-src idiom (createElement → appendChild → el.src =
/// url): the report from the 10-27 upload run — the most common dynamic
/// loader shape silently no-op'd because insertion consumed the start slot
/// on an empty script.
#[tokio::test(flavor = "current_thread")]
async fn dynamic_script_src_after_empty_insert_executes() {
    let _guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port_now = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: 30\r\nConnection: close\r\n\r\nwindow.__lateSrc = 'did-load';",
                );
            }
        });
        port_now
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("globalThis.__lastErrs=[]; const _ce=console.error; console.error=function(){ (globalThis.__lastErrs=globalThis.__lastErrs||[]).push(Array.from(arguments).map(String).join(' ')); _ce.apply(console,arguments); };").unwrap();
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const s = document.createElement('script');
                document.body.appendChild(s);      // empty insert — no slot consumed
                const done = new Promise(r => { s.onload = r; s.onerror = () => r('err'); });
                s.src = '/late.js';               // src AFTER insert arms the loader
                const ev = await done;            // load must fire once fetched
                await new Promise(r => setTimeout(r, 50));
                return { loaded: window.__lateSrc === 'did-load', ev: String(ev), src: s.src.slice(0, 40)};
                return { loaded: window.__lateSrc === 'did-load', src: s.src.slice(0, 40) };
            }"#,
            None, &[],
            true, true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    let v = serde_json::to_value(&result.value).unwrap_or_default();
    assert_eq!(
        v.get("loaded").and_then(|b| b.as_bool()),
        Some(true),
        "late-src script must execute: {v:?}"
    );
}

/// A dynamic script whose first dial dies before any HTTP status (the
/// proxy-connect drop that left x.com's HomeTimeline chunk unloaded)
/// retries once and runs. The op rejects instead of returning status 0,
/// which the body-shaped retry does not see.
#[tokio::test(flavor = "current_thread")]
async fn dynamic_script_retries_once_when_the_dial_drops() {
    let _guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let h2 = hits.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for stream in l.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 2048];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            if !req.starts_with("GET /drop.js") {
                continue;
            }
            let n = h2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                drop(s);
                continue;
            }
            let body = "window.__dropLoaded = true;";
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                globalThis.__dynScriptRetries = 0;
                const a = document.createElement('script');
                a.src = '/drop.js';
                document.body.appendChild(a);
                const done = new Promise(r => { a.onload = () => r('load'); a.onerror = () => r('err'); });
                const ev = await done;
                return { ev, loaded: window.__dropLoaded === true, retries: globalThis.__dynScriptRetries };
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    let v = serde_json::to_value(&result.value).unwrap_or_default();
    assert_eq!(
        v.get("loaded").and_then(|b| b.as_bool()),
        Some(true),
        "the second dial must run the chunk: {v:?}"
    );
    assert_eq!(
        v.get("retries").and_then(|n| n.as_i64()),
        Some(1),
        "exactly one retry: {v:?}"
    );
    assert!(
        hits.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "the dropped dial must be attempted twice"
    );
}

/// Lazy-chunk resilience (10-27 复盘): a transient network-shaped failure on
/// a dynamic script (status 0 / 5xx) retries once and succeeds; a 404 does
/// NOT retry (the resource genuinely isn't there — the retry counter stays
/// flat and the error still fires).
#[tokio::test(flavor = "current_thread")]
async fn dynamic_script_retries_once_on_network_failure_not_on_404() {
    let _guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let (port, _hits) = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port_now = l.local_addr().unwrap().port();
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let h2 = hits.clone();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let count = h2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let (head, body) = if req.starts_with("GET /flaky") {
                    if count == 0 {
                        // First hit: connection-shaped failure — close mid-head.
                        let _ = s.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        continue;
                    }
                    (
                        "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: 28\r\nConnection: close\r\n\r\n",
                        "window.__flakyLoaded = true;",
                    )
                } else {
                    // /gone → 404 every time.
                    (
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        "",
                    )
                };
                let _ = s.write_all(format!("{}{}", head, body).as_bytes());
            }
        });
        (port_now, hits)
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    rt.evaluate("globalThis.__lastErrs2=[]; const _e2=console.error; console.error=function(){ (globalThis.__lastErrs2=globalThis.__lastErrs2||[]).push(Array.from(arguments).map(String).join(' ').slice(0,90)); _e2.apply(console,arguments); };").unwrap();
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                globalThis.__dynScriptRetries = 0;
                // flaky: 500 then 200 → retry saves it.
                const a = document.createElement('script');
                a.src = '/flaky.js';
                document.body.appendChild(a);
                const aDone = new Promise(r => { a.onload = r; a.onerror = () => r('err'); });
                await aDone;
                // gone: 404 → no retry, error fires.
                const b = document.createElement('script');
                b.src = '/gone.js';
                document.body.appendChild(b);
                const bDone = new Promise(r => { b.onload = () => r('load'); b.onerror = () => r('err'); });
                const bEv = await bDone;
                await new Promise(r => setTimeout(r, 50));
                return { flaky: window.__flakyLoaded === true, retries: globalThis.__dynScriptRetries, goneEv: bEv};
            }"#,
            None, &[],
            true, true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
    let v = serde_json::to_value(&result.value).unwrap_or_default();
    assert_eq!(
        v.get("flaky").and_then(|b| b.as_bool()),
        Some(true),
        "retry must save the flaky chunk: {v:?}"
    );
    assert_eq!(
        v.get("retries").and_then(|n| n.as_i64()),
        Some(1),
        "exactly one retry: {v:?}"
    );
    assert_eq!(
        v.get("goneEv").and_then(|s| s.as_str()),
        Some("err"),
        "404 errors without retry: {v:?}"
    );
}
