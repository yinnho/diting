//! XHR：sync/async/timeout/abort/signer gate/binary responseType
use super::super::*;
use super::common::{find_subsequence, setup_runtime};

/// #235: native XHR must not ride the page-overridable fetch. A page
/// wrapping `window.fetch` (doudian's SDK wraps XHR and fetch both) used
/// to intercept our XHR.send through its wrapper — already-signed
/// requests re-processed, getSchema observed returning 200 with a 0-byte
/// body. The XHR now calls the engine's own shim closure; a pass-through
/// wrapper must see ZERO entries. data: URL keeps the probe offline.
#[tokio::test(flavor = "current_thread")]
async fn xhr_send_bypasses_page_fetch_wrapper() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    let wrapped = 0;
                    const orig = window.fetch;
                    window.fetch = function() { wrapped++; return orig.apply(this, arguments); };
                    const x = new XMLHttpRequest();
                    x.open('GET', 'data:text/plain,hi');
                    const settled = new Promise(r => { x.onload = r; x.onerror = r; });
                    x.send();
                    await settled;
                    await new Promise(r => setTimeout(r, 30));
                    window.fetch = orig;
                    return {wrapped: String(wrapped), status: String(x.status)};
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
        v["wrapped"],
        serde_json::json!("0"),
        "XHR must bypass the page fetch wrapper"
    );
    assert_eq!(
        v["status"],
        serde_json::json!("200"),
        "data: URL still loads through the shim"
    );
}

/// Sync XHR (obscura#908): open(..., false) + send() must issue the
/// request and return with status/headers/body populated — before any
/// event-loop turn. The upstream reporter's legacy pages (dojo/jQuery
/// era) read xhr.responseText on the line right after send() and see ""
/// + status 0 forever when the flag is ignored.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn sync_xhr_get_populates_status_and_body_inline() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\nx-probe: yes\r\ncontent-length: 6\r\nconnection: close\r\n\r\nsyncok")
                .unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"() => {
                    const x = new XMLHttpRequest();
                    x.open('GET', '/api', false);
                    let events = 0;
                    const bump = () => { events += 1; };
                    x.addEventListener('loadstart', bump);
                    x.addEventListener('load', bump);
                    x.addEventListener('loadend', bump);
                    x.addEventListener('readystatechange', bump);
                    x.send();
                    // All response fields must already be populated here —
                    // no promise, no event loop turn.
                    return {
                        status: x.status,
                        text: x.responseText,
                        probe: x.getResponseHeader('x-probe'),
                        allHeaders: x.getAllResponseHeaders().includes('content-type'),
                        state: x.readyState,
                        events: events,
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "status": 200,
            "text": "syncok",
            "probe": "yes",
            "allHeaders": true,
            "state": 4,
            "events": 0,
        })
    );
}

/// (#116) xhr.timeout must race the async send→done span. A transport
/// that accepts and never answers used to fire no event at all — the
/// on-surface symptom was a silent EVAL_TIMEOUT with zero callbacks
/// (tmall publish report: xhr.timeout=5000, nothing fired in 8000ms).
/// Chrome: deadline hit → `timeout` event → DONE → status 0 → loadend;
/// a late settlement of the underlying fetch is ignored.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_timeout_fires_events_on_hung_transport() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Accept the TCP connection but never write a byte — the request
    // hangs until the engine's 30s read timeout, which the 400ms
    // xhr.timeout must beat by orders of magnitude.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut parked = Vec::new();
        for stream in listener.incoming() {
            match stream {
                Ok(s) => parked.push(s), // hold open, never respond
                Err(_) => break,
            }
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const events = [];
                    const xhr = new XMLHttpRequest();
                    xhr.open('GET', '/hang');
                    xhr.timeout = 400;
                    for (const t of ['timeout', 'loadend', 'load', 'error', 'abort']) {
                        xhr.addEventListener(t, () => events.push(t));
                    }
                    const done = new Promise(r => { xhr.onloadend = () => r(); });
                    const t0 = Date.now();
                    xhr.send();
                    await done;
                    return {
                        events,
                        readyState: xhr.readyState,
                        status: xhr.status,
                        text: xhr.responseText,
                        ms: Date.now() - t0,
                    };
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
        v["events"],
        serde_json::json!(["timeout", "loadend"]),
        "a hung transport must fire timeout then loadend — nothing else"
    );
    assert_eq!(v["readyState"], serde_json::json!(4));
    assert_eq!(v["status"], serde_json::json!(0));
    assert_eq!(v["text"], serde_json::json!(""));
    let ms = v["ms"].as_f64().unwrap();
    assert!(
        ms < 5000.0,
        "deadline must fire near the 400ms budget, got {ms}ms"
    );
}

/// Signer-wait gate (engine-native): an async XHR matching the gate
/// pattern, sent while the page's signing function does not exist yet,
/// must hold inside the engine's own send delegation — no page-visible
/// wrapper anywhere — then re-sign through the page's signer at release:
/// X-s/X-t stamped from the signer's return value, the page's stale
/// X-S-Common stripped rather than paired with the fresh signature, and
/// the signer called with the URL's path plus the parsed JSON body. The
/// gate config global must stay invisible to the page on both Object.keys
/// and getOwnPropertyNames (the internal-name enumeration boundary).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_signer_gate_holds_then_resigns_via_page_signer() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap();
        let head = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
        // "x-s:" must not also match "x-s-common:" — startswith on the
        // colon does the separating.
        let saw_xs = head
            .lines()
            .any(|l| l.starts_with("x-s:") && l.contains("engsig:"));
        let saw_path_in_sig = head.contains("engsig:/api/edith/product/publish_render:");
        let saw_xt = head.lines().any(|l| l.starts_with("x-t:"));
        let stale_common = head.contains("stale-page-common");
        let payload = format!(
            "{{\"code\":0,\"sawXs\":{saw_xs},\"sigCarriedPath\":{saw_path},\"sawXt\":{saw_xt},\"staleCommon\":{stale_common}}}",
            saw_xs = saw_xs,
            saw_path = saw_path_in_sig,
            saw_xt = saw_xt,
            stale_common = stale_common,
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nconnection: close\r\ncontent-length: {}\r\n\r\n{}",
            payload.len(),
            payload
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    Object.defineProperty(globalThis, '__ditingSignerGate', {value: {
                        re: /\/api\/edith\/product\/publish_render/,
                        signer: () => (typeof window.__mySigner === 'function' ? window.__mySigner : null),
                        waitMs: 5000, settleMs: 60, pollMs: 10, seenAt: 0, signed: 1,
                        resigned: 0, passthrough: 0, softfail: 0, dropped: 0, unsigned: 0,
                    }, writable: true, configurable: true, enumerable: false});
                    const x = new XMLHttpRequest();
                    x.open('POST', '/api/edith/product/publish_render');
                    x.setRequestHeader('Content-Type', 'application/json');
                    x.setRequestHeader('X-S-Common', 'stale-page-common');
                    // The signer mounts after send(): the gate must hold the
                    // delegation until it exists (plus the settle window).
                    setTimeout(() => {
                        window.__mySigner = (p, d) => ({'X-s': 'engsig:' + p + ':' + JSON.stringify(d), 'X-t': '1700000000000'});
                    }, 120);
                    const t0 = Date.now();
                    x.send(JSON.stringify({name: 'test'}));
                    await new Promise((res) => { x.onloadend = () => res(); });
                    return {
                        ms: Date.now() - t0,
                        body: x.responseText ? JSON.parse(x.responseText) : null,
                        keysLeak: Object.keys(window).includes('__ditingSignerGate'),
                        gopnLeak: Object.getOwnPropertyNames(window).includes('__ditingSignerGate'),
                        gate: {
                            resigned: globalThis.__ditingSignerGate.resigned,
                            softfail: globalThis.__ditingSignerGate.softfail,
                        },
                    };
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
    let body = v["body"].as_object().unwrap();
    assert_eq!(
        body["code"],
        serde_json::json!(0),
        "wire echo must come back"
    );
    assert_eq!(
        body["sawXs"],
        serde_json::json!(true),
        "server must see the re-signed X-s"
    );
    assert_eq!(
        body["sigCarriedPath"],
        serde_json::json!(true),
        "signer must be called with the stripped path and parsed body"
    );
    assert_eq!(body["sawXt"], serde_json::json!(true), "fresh X-t stamped");
    assert_eq!(
        body["staleCommon"],
        serde_json::json!(false),
        "page's stale X-S-Common must be stripped, not paired with the fresh signature"
    );
    let ms = v["ms"].as_f64().unwrap();
    assert!(
        ms >= 150.0,
        "gate must hold past the signer mount (120ms), released at {ms}ms"
    );
    assert_eq!(
        v["keysLeak"],
        serde_json::json!(false),
        "gate global invisible to Object.keys"
    );
    assert_eq!(
        v["gopnLeak"],
        serde_json::json!(false),
        "gate global invisible to getOwnPropertyNames (enumeration boundary)"
    );
    assert_eq!(v["gate"]["resigned"], serde_json::json!(1));
    assert_eq!(v["gate"]["softfail"], serde_json::json!(0));
}

/// Signer gate fail-closed: a signer that never mounts within waitMs must
/// NEVER wire the request unsigned — that 406 is exactly what crash-loops
/// the app and poisons the session. The app instead sees a synthetic
/// soft-fail response through the normal load pipeline, and no TCP
/// connection is made at all.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_signer_gate_softfails_without_touching_the_wire() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let connections = connections.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(_) => {
                        connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    Object.defineProperty(globalThis, '__ditingSignerGate', {value: {
                        re: /\/api\/edith\/product\/publish_render/,
                        signer: () => null, // never mounts
                        waitMs: 300, settleMs: 50, pollMs: 10, seenAt: 0, signed: 1,
                        resigned: 0, passthrough: 0, softfail: 0, dropped: 0, unsigned: 0,
                    }, writable: true, configurable: true, enumerable: false});
                    const events = [];
                    const x = new XMLHttpRequest();
                    x.open('POST', '/api/edith/product/publish_render');
                    x.setRequestHeader('Content-Type', 'application/json');
                    for (const t of ['load', 'loadend', 'error', 'timeout']) {
                        x.addEventListener(t, () => events.push(t));
                    }
                    x.send('{}');
                    await new Promise((res) => { x.onloadend = () => res(); });
                    return {
                        events,
                        status: x.status,
                        state: x.readyState,
                        body: JSON.parse(x.responseText),
                        softfail: globalThis.__ditingSignerGate.softfail,
                    };
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
        v["events"],
        serde_json::json!(["load", "loadend"]),
        "soft-fail must surface through the normal load pipeline, not an error"
    );
    assert_eq!(v["status"], serde_json::json!(200));
    assert_eq!(v["state"], serde_json::json!(4));
    assert_eq!(v["body"]["code"], serde_json::json!(-1));
    assert_eq!(v["body"]["msg"], serde_json::json!("signer-not-ready"));
    assert_eq!(v["softfail"], serde_json::json!(1));
    assert_eq!(
        connections.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "held request must never reach the wire unsigned"
    );
}

/// (#248) The #116 timeout fix only covered addEventListener handlers:
/// the error and timeout paths set readyState with a raw assignment and
/// _fireEvent('readystatechange'), which deliberately skips the
/// onreadystatechange *property* — real-world code (platform SDKs, our
/// probes) awaits rs4 on the property and hung forever. The same paths
/// also called onerror/onabort directly after _fireEvent had already
/// invoked them, double-firing. Both property-attached here.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_network_error_delivers_rs4_to_property_handlers_once() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Bind then drop: nothing listens, so the fetch rejects fast with a
    // connection error and the XHR walks the catch path.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const events = [];
                    const xhr = new XMLHttpRequest();
                    xhr.onreadystatechange = () => events.push('rs' + xhr.readyState);
                    xhr.onerror = () => events.push('error');
                    xhr.onload = () => events.push('load');
                    xhr.open('GET', '/refused');
                    const done = new Promise(r => { xhr.onloadend = () => { events.push('loadend'); r(); }; });
                    xhr.send();
                    await done;
                    return { events, readyState: xhr.readyState, status: xhr.status };
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
        v["events"],
        serde_json::json!(["rs1", "rs4", "error", "loadend"]),
        "network error must reach property handlers exactly once each: rs4, error, loadend"
    );
    assert_eq!(v["readyState"], serde_json::json!(4));
    assert_eq!(v["status"], serde_json::json!(0));
}

/// (#248) The client-blocked face — the channel the refused-connection
/// test above cannot reach: with the private-network gate armed (no
/// env allow), the transport never leaves the client layer and the op
/// resolves status-0 + blocked, which the fetch shim rejects with an
/// TypeError("Failed to fetch"). XHR must still fire 'error' (Chrome's
/// face for a blocked request); the #234 absorption keyed the catch on
/// err.__aborted and fired 'abort' instead, starving onerror on every
/// client-blocked request. Live-verified on 8129 before the fix: rs4 +
/// loadend with error missing on both the property and listener
/// channels.
/// moli 2026-10-24 XHR series (tool-line follow-up): abort() on an
/// in-flight request must fire the full event contract — readystatechange
/// (rs4), abort, loadend — and land readyState 0 with status 0. Verified
/// against Chrome's XHR spec order.
#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_abort_fires_contract_and_resets_state() {
    // A SLOW server: accepts, reads the request, then sits on it — the
    // request is genuinely in-flight (connect + sent, no response) when
    // abort() lands. A bare never-accepting listener errors too fast in
    // some stacks to be a reliable in-flight fixture.
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port_now = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                std::thread::sleep(std::time::Duration::from_secs(10));
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
        });
        port_now
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const events = [];
                    const xhr = new XMLHttpRequest();
                    xhr.onreadystatechange = () => events.push('rs' + xhr.readyState);
                    xhr.onabort = () => events.push('abort');
                    xhr.onerror = () => events.push('error');
                    xhr.ontimeout = () => events.push('timeout');
                    xhr.onload = () => events.push('load');
                    xhr.onloadend = () => events.push('loadend');
                    xhr.open('GET', '/never-answers');
                    const done = new Promise(r => { xhr.onloadend = () => { events.push('ld'); r(); }; });
                    xhr.send();
                    // Give the request a beat to leave, then abort mid-flight.
                    await new Promise(r => setTimeout(r, 30));
                    xhr.abort();
                    await done;
                    return { events: events.filter(e => e !== 'loadend'), readyState: xhr.readyState, status: xhr.status };
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    let v = result.value.unwrap();
    let events: Vec<String> = v
        .get("events")
        .and_then(|e| e.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        events.contains(&"abort".to_string()),
        "abort event missing: {events:?}"
    );
    assert!(
        events.iter().any(|e| e == "rs4" || e == "rs0" || e == "ld"),
        "no terminal state transition after abort: {events:?}"
    );
    let rs = v.get("readyState").and_then(|r| r.as_i64()).unwrap_or(-1);
    let status = v.get("status").and_then(|x| x.as_i64()).unwrap_or(-1);
    assert_eq!(rs, 0, "abort must reset readyState to 0 (UNSENT), got {rs}");
    assert_eq!(status, 0, "abort must zero status, got {status}");
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

/// Reentrancy (the other half of moli's series): a NEW request issued
/// from inside the abort handler must run to completion on the same XHR
/// instance (open() while aborting is the classic business-page pattern:
/// cancel + retry widgets). Contract holds today; NOTE the fixture wait
/// is 1.5s because same-origin requests serialize behind a busy
/// connection in the net client (Chrome opens a parallel connection and
/// answers in ~ms) — pooled-connection parallelism is a separate
/// candidate, not an XHR-layer defect.
#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_reentrant_send_from_abort_handler_completes() {
    // Echo-ish server: any request gets an immediate 200.
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port_now = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut buf = [0u8; 4096];
                use std::io::{Read, Write};
                let mut s = stream;
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                // FIRST hangs a second (in-flight when abort lands);
                // the reentrant retry answers instantly.
                if req.contains("/first") {
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
        });
        port_now
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const xhr = new XMLHttpRequest();
                    const log = [];
                    let retried = false;
                    xhr.onloadend = () => { log.push('loadend:' + xhr.status + ':' + xhr.responseText); };
                    xhr.onload = () => log.push('load');
                    xhr.onerror = () => log.push('error');
                    xhr.onreadystatechange = () => log.push('rs' + xhr.readyState);
                    xhr.onabort = () => {
                        log.push('abort-enter');
                        if (!retried) {
                            retried = true;
                            try {
                                xhr.open('GET', '/retry');
                                xhr.send();  // reentrant: from inside abort
                                log.push('resend-ok');
                            } catch (e) {
                                log.push('resend-threw:' + String(e).slice(0, 60));
                            }
                        }
                    };
                    xhr.open('GET', '/first');
                    xhr.send();
                    await new Promise(r => setTimeout(r, 20));
                    xhr.abort();
                    await new Promise(r => setTimeout(r, 1500));
                    return { retried, log, status: xhr.status, body: xhr.responseText };
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    let v = result.value.unwrap();
    let log = serde_json::to_string(&v).unwrap_or_default();
    assert_eq!(
        v.get("retried").and_then(|b| b.as_bool()),
        Some(true),
        "{log}"
    );
    assert_eq!(v.get("body").and_then(|b| b.as_str()), Some("ok"), "{log}");
    assert_eq!(v.get("status").and_then(|x| x.as_i64()), Some(200), "{log}");
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_client_blocked_fires_error_not_abort() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const events = [];
                    const xhr = new XMLHttpRequest();
                    xhr.onreadystatechange = () => events.push('rs' + xhr.readyState);
                    xhr.onerror = () => events.push('error');
                    xhr.onabort = () => events.push('abort');
                    xhr.open('GET', '/gate-blocked');
                    const done = new Promise(r => { xhr.onloadend = () => { events.push('loadend'); r(); }; });
                    xhr.send();
                    await done;
                    return { events, readyState: xhr.readyState, status: xhr.status };
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
        v["events"],
        serde_json::json!(["rs1", "rs4", "error", "loadend"]),
        "client-blocked XHR is an 'error' in Chrome, never 'abort'"
    );
    assert_eq!(v["status"], serde_json::json!(0));
}

/// (#248) Same property-side contract for the timeout path: the deadline
/// must hand DONE to onreadystatechange, not just to listeners (#116).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn async_xhr_timeout_delivers_rs4_to_property_handlers() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut parked = Vec::new();
        for stream in listener.incoming() {
            match stream {
                Ok(s) => parked.push(s), // hold open, never respond
                Err(_) => break,
            }
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const events = [];
                    const xhr = new XMLHttpRequest();
                    xhr.onreadystatechange = () => events.push('rs' + xhr.readyState);
                    xhr.ontimeout = () => events.push('timeout');
                    xhr.onerror = () => events.push('error');
                    xhr.open('GET', '/hang');
                    xhr.timeout = 300;
                    const done = new Promise(r => { xhr.onloadend = () => { events.push('loadend'); r(); }; });
                    xhr.send();
                    await done;
                    return { events, readyState: xhr.readyState, status: xhr.status };
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
        v["events"],
        serde_json::json!(["rs1", "rs4", "timeout", "loadend"]),
        "deadline must hand DONE to the property handler too, then timeout+loadend exactly once"
    );
    assert_eq!(v["readyState"], serde_json::json!(4));
    assert_eq!(v["status"], serde_json::json!(0));
}

/// (#116) Chrome parity: sync XHR with a non-zero timeout throws
/// InvalidAccessError on send() — there is no event-loop turn to fire
/// the deadline on.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn sync_xhr_with_timeout_throws_invalid_access() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"() => {
                    const xhr = new XMLHttpRequest();
                    xhr.open('GET', '/x', false);
                    xhr.timeout = 1;
                    try { xhr.send(); return { threw: false }; }
                    catch (e) { return { threw: true, name: e.name }; }
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
        serde_json::json!({ "threw": true, "name": "InvalidAccessError" })
    );
}

/// Sync POST round-trips the body; the server must have received the
/// exact bytes send() was handed (form-POST legacy flows).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn sync_xhr_post_body_roundtrip() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // Read until the full body (per content-length) has landed.
        let body;
        loop {
            let n = stream.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                let clen: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .map(|v| v.trim().parse().unwrap_or(0))
                    .unwrap_or(0);
                let have = buf.len() - (pos + 4);
                if have >= clen {
                    body = buf[pos + 4..pos + 4 + clen].to_vec();
                    break;
                }
            }
            if n == 0 {
                body = Vec::new();
                break;
            }
        }
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"() => {
                    const x = new XMLHttpRequest();
                    x.open('POST', '/submit', false);
                    x.setRequestHeader('content-type', 'application/x-www-form-urlencoded');
                    x.send('a=1&b=two');
                    return { status: x.status, echoed: x.responseText, state: x.readyState };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "status": 200,
            "echoed": "a=1&b=two",
            "state": 4,
        })
    );
}

/// Sync XHR to a dead endpoint is a network error surfaced through the
/// fields (status 0, readyState 4), not an exception.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn sync_xhr_dead_endpoint_throws_network_error() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
            .call_function_on_remote(
                r#"() => {
                    const x = new XMLHttpRequest();
                    x.open('GET', '/gone', false);
                    let threw = 'no';
                    let name = '';
                    try { x.send(); } catch (e) { threw = 'yes'; name = e.name; }
                    return { threw, name, status: x.status, text: x.responseText, state: x.readyState };
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    // Chrome ground truth (headless, 2026-10-11): sync XHR network error
    // throws NetworkError, readyState already 4, status 0, empty body.
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "threw": "yes",
            "name": "NetworkError",
            "status": 0,
            "text": "",
            "state": 4,
        })
    );
}

/// (#42) Sync XHR resolves data: URLs locally — status 200, decoded
/// payload, content-type header, responseURL keeps the data: URL. The
/// old behavior fell into the network-op catch and zeroed status.
#[tokio::test(flavor = "current_thread")]
async fn xhr_sync_data_url_reports_200() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const x = new XMLHttpRequest();
                    x.open('GET', 'data:text/plain,XYZZY', false);
                    x.send();
                    return {
                        status: String(x.status),
                        body: x.responseText,
                        ready: String(x.readyState),
                        url: x.responseURL === 'data:text/plain,XYZZY',
                        ct: x.getResponseHeader('content-type'),
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["status"], serde_json::json!("200"));
    assert_eq!(v["body"], serde_json::json!("XYZZY"));
    assert_eq!(v["ready"], serde_json::json!("4"));
    assert_eq!(v["url"], serde_json::json!(true));
    assert_eq!(v["ct"], serde_json::json!("text/plain"));
}

/// (#106) send() on an XHR whose URL slot is not a string must throw
/// Chrome's InvalidStateError, not die on the internal .startsWith crash
/// (the Douyin SDK hit exactly that). open() also coerces undefined via
/// WebIDL USVString to "undefined", so the stored slot is always a string.
#[tokio::test(flavor = "current_thread")]
async fn xhr_send_guard_non_string_url() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const x = new XMLHttpRequest();
                    x.open('GET', 'data:text/plain,hi', false);
                    x._url = undefined;
                    let guard = 'no-throw';
                    try { x.send(); } catch (e) { guard = e.name; }
                    const x2 = new XMLHttpRequest();
                    x2.open('GET', undefined, false);
                    const coerced = typeof x2._url === 'string' ? x2._url : 'NOT-A-STRING';
                    return [guard, coerced];
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
        v[0],
        serde_json::json!("InvalidStateError"),
        "cleared URL slot must throw like Chrome, not TypeError"
    );
    assert_eq!(
        v[1],
        serde_json::json!("undefined"),
        "open() USVString coercion"
    );
}

/// (#42) base64 payloads decode through the same RFC 2397 walk, and an
/// unparseable data: URL lands on status 0 (Chrome: network error face).
#[tokio::test(flavor = "current_thread")]
async fn xhr_sync_data_url_base64_and_broken() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const b = new XMLHttpRequest();
                    b.open('GET', 'data:text/plain;base64,WFlaWlk=', false);
                    b.send();
                    const broken = new XMLHttpRequest();
                    broken.open('GET', 'data:text/plain-nocomma', false);
                    broken.send();
                    return {
                        status: String(b.status),
                        body: b.responseText,
                        brokenStatus: String(broken.status),
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["status"], serde_json::json!("200"));
    assert_eq!(v["body"], serde_json::json!("XYZZY"));
    assert_eq!(v["brokenStatus"], serde_json::json!("0"));
}

/// (#42) blob: URLs read through the createObjectURL registry with the
/// blob's own type surfacing as content-type.
#[tokio::test(flavor = "current_thread")]
async fn xhr_sync_blob_url_reads_registry() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const u = URL.createObjectURL(new Blob(['BLOBOK'], { type: 'text/x-demo' }));
                    const x = new XMLHttpRequest();
                    x.open('GET', u, false);
                    x.send();
                    return {
                        status: String(x.status),
                        body: x.responseText,
                        ct: x.getResponseHeader('content-type'),
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["status"], serde_json::json!("200"));
    assert_eq!(v["body"], serde_json::json!("BLOBOK"));
    assert_eq!(v["ct"], serde_json::json!("text/x-demo"));
}

/// (#42) regression: async data: XHR keeps firing load through the same
/// local branch instead of the fetch() path.
#[tokio::test(flavor = "current_thread")]
async fn xhr_async_data_url_fires_load() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const x = new XMLHttpRequest();
                    const how = await new Promise((resolve) => {
                        x.onload = () => resolve('load');
                        x.onerror = () => resolve('error');
                        x.open('GET', 'data:text/plain,XYZZY', true);
                        x.send();
                    });
                    return {
                        how,
                        status: String(x.status),
                        body: x.responseText,
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("load"));
    assert_eq!(v["status"], serde_json::json!("200"));
    assert_eq!(v["body"], serde_json::json!("XYZZY"));
}

/// obscura #754/#716 class: XHR `responseType: "arraybuffer"`/`"blob"`
/// must round-trip the raw response bytes. The old path took `resp.text()`
/// (lossy UTF-8) and re-encoded it, mangling every non-UTF-8 byte — PNG
/// magic came back with 0x89/0x1A replaced by U+FFFD (EF BF BD).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn xhr_binary_response_types_roundtrip_bytes() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).unwrap();
            let body: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt.call_function_on_remote(
            r#"async () => {
                const asType = (type) => new Promise((resolve, reject) => {
                    const xhr = new XMLHttpRequest();
                    xhr.open("GET", "/img.png");
                    xhr.responseType = type;
                    xhr.onload = () => resolve(xhr.response);
                    xhr.onerror = () => reject(new Error("xhr error"));
                    xhr.send();
                });
                const bytes = Array.from(new Uint8Array(await asType("arraybuffer")));
                const blobBytes = Array.from(new Uint8Array(await (await asType("blob")).arrayBuffer()));
                return { bytes, blobBytes };
            }"#,
            None,
            &[],
            true,
            true,
        ).await.unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let png_magic = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "bytes": png_magic,
            "blobBytes": png_magic,
        })
    );
}

/// 0.4.1 taobao report problem 5: the Ali SDK opens XHRs with lowercase
/// methods (`xhr.open('get', …)`), and the network layer compares methods
/// case-sensitively — so the request went out literally `get`, breaking
/// CORS safelist matching. open() now uppercases a token that
/// byte-uppercases to a standard method (Fetch-spec "normalize a
/// method"); custom tokens ride as authored, as in Chrome.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn xhr_open_normalizes_method_case() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]);
            let line = request.lines().next().unwrap_or("").to_string();
            let body = b"ok";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
            line_tx.send(line).unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const send = (m) => new Promise((resolve, reject) => {
                        const xhr = new XMLHttpRequest();
                        xhr.open(m, "/probe");
                        xhr.onload = () => resolve(xhr.status);
                        xhr.onerror = () => reject(new Error("xhr error"));
                        xhr.send();
                    });
                    return [await send("get"), await send("put"), await send("x-custom")];
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(result.value.unwrap(), serde_json::json!([200, 200, 200]));
    // What the wire actually saw: standard tokens uppercased, custom as-is.
    let lines: Vec<String> = (0..3)
        .map(|_| {
            line_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
        })
        .collect();
    assert!(lines[0].starts_with("GET /probe "), "got: {}", lines[0]);
    assert!(lines[1].starts_with("PUT /probe "), "got: {}", lines[1]);
    assert!(
        lines[2].starts_with("x-custom /probe "),
        "got: {}",
        lines[2]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_xhr_open_encodes_query_in_document_charset() {
    // moli 8c5ffea shape: XHR URLs use the document's encoding for raw
    // non-ASCII query characters; already-percent-encoded bytes stay as
    // authored, and a UTF-8 document is untouched.
    let rt = setup_runtime("<html><body></body></html>");
    rt.set_encoding("GBK");
    let mut rt = rt;
    let v = rt
            .evaluate(
                "(() => { const o = (u) => { const x = new XMLHttpRequest(); x.open('GET', u); return x._url; };
                  return [o('/q?名=值'), o('/q?%E5%90%8D=1#名'), o('/q?a=b'), o('/path名?x=1')].join(' '); })()",
            )
            .unwrap();
    assert_eq!(v, "/q?%C3%FB=%D6%B5 /q?%E5%90%8D=1#名 /q?a=b /path名?x=1");
}
