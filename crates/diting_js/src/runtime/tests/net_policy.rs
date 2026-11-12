//! 网络策略面：CORS/preflight/beacon/SSRF/referrer/proxy 凭据
use super::super::*;
use super::common::setup_runtime;

/// The document's own Referrer Policy (batch-84 leftover): a policy from
/// the navigation response's Referrer-Policy header beats every <meta
/// name=referrer>; among metas the FIRST whose content yields a valid
/// token wins; within one value the LAST valid comma token wins. fetch()
/// with no init policy and sync XHR resolve the document policy
/// Rust-side at op time; an explicit init policy still overrides.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_honors_document_referrer_policy() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // /probe answers with the request's Referer header value, "(none)"
    // when the header is absent.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..6 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let head = String::from_utf8_lossy(&buf[..n]);
            let referer = head
                .lines()
                .find_map(|l| {
                    let lower = l.to_ascii_lowercase();
                    lower
                        .starts_with("referer:")
                        .then(|| lower["referer:".len()..].trim().to_string())
                })
                .unwrap_or_else(|| "(none)".to_string());
            let body = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                referer.len(),
                referer
            );
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
        }
    });

    // Page A: two referrer metas — the first valid one ("origin") wins
    // over the later "no-referrer" (tree order, not last-wins).
    let mut rt = setup_runtime(
        r#"<html><head>
                <meta name="referrer" content="origin">
                <meta name="referrer" content="no-referrer">
            </head><body></body></html>"#,
    );
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const out = {};
                    // 刀57: Chrome 145's Document.prototype carries no
                    // referrerPolicy — the meta policy shows on the wire only.
                    out.surface = ('referrerPolicy' in Document.prototype) ? 'present' : 'gone';
                    // No init policy → document policy ("origin") → Referer is
                    // just the origin.
                    out.metaFirstValid = await (await fetch('/probe')).text();
                    // Explicit init still beats the document policy.
                    out.initOverridesDocument =
                        await (await fetch('/probe', { referrerPolicy: 'no-referrer' })).text();
                    return out;
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
        serde_json::json!({
            "surface": "gone",
            "metaFirstValid": format!("http://127.0.0.1:{}/", port),
            "initOverridesDocument": "(none)",
        })
    );

    // Page B: comma list keeps the LAST valid token ("unsafe-url"); a
    // header-delivered policy (set_referrer_policy) then beats the meta
    // outright — on the wire (the surface face is gone, 刀57).
    let mut rt = setup_runtime(
        r#"<html><head>
                <meta name="referrer" content="unsafe-url, garbage">
            </head><body></body></html>"#,
    );
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const out = {};
                    out.lastValidTokenSurface = ('referrerPolicy' in Document.prototype) ? 'present' : 'gone';
                    out.lastValidToken = await (await fetch('/probe')).text();
                    return out;
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
        serde_json::json!({
            "lastValidTokenSurface": "gone",
            "lastValidToken": format!("http://127.0.0.1:{}/test", port),
        })
    );
    rt.set_referrer_policy("no-referrer");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const out = {};
                    out.headerBeatsMetaSurface = ('referrerPolicy' in Document.prototype) ? 'present' : 'gone';
                    out.headerBeatsMeta = await (await fetch('/probe')).text();
                    return out;
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
        serde_json::json!({
            "headerBeatsMetaSurface": "gone",
            "headerBeatsMeta": "(none)",
        })
    );

    // Page C: an invalid first meta is skipped (its content yields no
    // valid token), the second applies; sync XHR resolves the same
    // document policy.
    let mut rt = setup_runtime(
        r#"<html><head>
                <meta name="referrer" content="trash">
                <meta name="referrer" content="no-referrer">
            </head><body></body></html>"#,
    );
    rt.set_url(&format!("http://127.0.0.1:{}/test", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const out = {};
                    out.invalidMetaSkipped = await (await fetch('/probe')).text();
                    const xhr = new XMLHttpRequest();
                    xhr.open('GET', '/probe', false);
                    xhr.send();
                    out.syncXhr = xhr.responseText;
                    return out;
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
            "invalidMetaSkipped": "(none)",
            "syncXhr": "(none)",
        })
    );
}

/// A Continue resolution that rewrites the request URL must pass the same
/// SSRF gate as the original request and as redirect hops — otherwise a
/// rewrite to an internal address bypasses validate_fetch_url entirely.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn test_intercept_url_rewrite_is_revalidated_against_ssrf() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let mut rt = setup_runtime("<html><body></body></html>");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    rt.set_intercept_tx(tx);
    rt.set_intercept_enabled(true);

    // Answer every intercepted request with a rewrite to a loopback address.
    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            let _ = req
                .resolver
                .send(crate::ops::InterceptResolution::Continue {
                    url: Some("http://127.0.0.1:9/secret".to_string()),
                    method: None,
                    headers: None,
                    body: None,
                });
        }
    });

    let result = rt
        .call_function_on_remote(
            r#"async () => {
                try {
                    await fetch("http://example.com/data.json");
                    return "not-blocked";
                } catch (e) {
                    return "blocked:" + (e && e.message);
                }
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v, serde_json::json!("blocked:Failed to fetch"));
}

/// The taobao shop-SPA report (0.3.0): a blocked or CORS-refused fetch
/// left NO row in the network log, so a page whose API calls all died at
/// the SSRF gate read as "never issued a request". The early exit must
/// record a status-0 js_network_event carrying the reason.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn ssrf_blocked_fetch_records_status_zero_event_with_reason() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        await fetch("http://127.0.0.1:9/secret");
                        return "not-blocked";
                    } catch (e) {
                        return "rejected";
                    }
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("rejected"));

    let events = rt.take_js_network_events();
    let blocked = events
        .iter()
        .find(|e| e.url.contains("127.0.0.1:9"))
        .expect("SSRF-blocked fetch must leave a network event");
    assert_eq!(blocked.status, 0);
    let error = blocked.error.as_deref().unwrap_or("");
    assert!(
        error.contains("private/internal IP address"),
        "event must carry the block reason, got: {error}"
    );
    // obscura#1108 absorption: the denial names the opt-in so an
    // operator hitting the deliberate gate learns the flag on the spot.
    assert!(
        error.contains("--allow-private-network") && error.contains("DITING_ALLOW_PRIVATE_NETWORK"),
        "denial must name the opt-in, got: {error}"
    );
}

/// The punished-mtop shape: the response body arrives but carries no
/// Access-Control-Allow-Origin, so the post-response CORS gate refuses it
/// — status 0 to JS, and (before this fix) invisible in /network. The
/// refusal must record a status-0 event with the CORS reason.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn cors_refused_fetch_records_status_zero_event_with_reason() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let body = b"{}";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        await fetch("http://127.0.0.1:PORT/api");
                        return "not-blocked";
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(msg.starts_with("rejected:Failed to fetch"), "got: {msg}");

    let events = rt.take_js_network_events();
    let refused = events
        .iter()
        .find(|e| e.url.contains(&format!(":{port}/api")))
        .expect("CORS-refused fetch must leave a network event");
    assert_eq!(refused.status, 0);
    let error = refused.error.as_deref().unwrap_or("");
    assert!(
        error.contains("CORS error"),
        "event must carry the CORS reason, got: {error}"
    );
}

/// Preflight permission enforcement (obscura "enforce CORS preflight
/// permissions", 04f0475 same-hole): a preflight that allows the origin
/// but never lists Access-Control-Allow-Methods does not consent to a
/// cross-origin PUT — the actual request must not go out. Before the fix
/// the origin-only check passed and the PUT reached the server anyway.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn preflight_without_allow_methods_blocks_the_actual_request() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // Only the OPTIONS preflight should ever land here; a second
        // connection means the actual PUT leaked through and the test's
        // assertion below fires (recording it either way aids the
        // failure message).
        for _ in 0..2 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let req = String::from_utf8_lossy(&buf).to_string();
            seen.lock()
                .unwrap()
                .push(req.lines().next().unwrap_or("").to_string());
            let response = "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        const r = await fetch("http://127.0.0.1:PORT/api", {
                            method: "PUT",
                            headers: { "content-type": "application/json" },
                            body: "{}",
                        });
                        return "not-blocked:" + r.status;
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    // #62: Chrome's page face is the bare TypeError; the method refusal
    // reason rides the network event.
    assert_eq!(msg, "rejected:Failed to fetch", "got: {msg}");

    // The actual request never reached the wire: only the OPTIONS hop.
    let served = requests.lock().unwrap().clone();
    assert!(
        served.iter().all(|line| line.starts_with("OPTIONS")),
        "only the preflight may go out, served: {served:?}"
    );

    let events = rt.take_js_network_events();
    let refused = events
        .iter()
        .find(|e| e.url.contains(&format!(":{port}/api")))
        .expect("preflight-refused fetch must leave a network event");
    assert_eq!(refused.status, 0);
    let error = refused.error.as_deref().unwrap_or("");
    assert!(
        error.contains("did not allow method 'PUT'"),
        "event must carry the preflight reason, got: {error}"
    );
}

/// The positive companion: when the preflight DOES list the method and
/// headers, the actual request proceeds and resolves. Guards against the
/// enforcement over-blocking (the safelist value check must not reject
/// what the preflight explicitly allows).
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn preflight_that_allows_the_method_lets_the_request_through() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // Hop 1: the OPTIONS preflight with a full consent set.
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        let preflight = concat!(
            "HTTP/1.1 204 No Content\r\n",
            "access-control-allow-origin: *\r\n",
            "access-control-allow-methods: PUT, POST\r\n",
            "access-control-allow-headers: content-type\r\n",
            "access-control-max-age: 600\r\n",
            "content-length: 0\r\nconnection: close\r\n\r\n",
        );
        stream.write_all(preflight.as_bytes()).unwrap();
        stream.flush().unwrap();
        // Hop 2: the actual PUT.
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        let body = b"ok";
        let response = format!(
            "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        const r = await fetch("http://127.0.0.1:PORT/api", {
                            method: "PUT",
                            headers: { "content-type": "application/json" },
                            body: "{}",
                        });
                        return "ok:" + r.status + ":" + (await r.text());
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        msg, "ok:200:ok",
        "consented preflight must let the PUT through, got: {msg}"
    );
}

/// Redirect credential retention (obscura#967 same hole): a redirect
/// chain that never leaves the origin keeps the scripted Authorization
/// header on every hop. Guards against over-stripping implementations
/// that drop credentials after ANY redirect.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn same_origin_redirect_keeps_scripted_credentials() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..3 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf).to_string());
            let response = match seen.lock().unwrap().len() {
                    1 => concat!(
                        "HTTP/1.1 200 OK\r\n",
                        "access-control-allow-origin: *\r\n",
                        "access-control-allow-methods: GET\r\n",
                        "access-control-allow-headers: authorization\r\n",
                        "content-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .to_string(),
                    // The 302 itself carries ACAO because a cors-tainted
                    // request CORS-checks every response in the chain.
                    2 => "HTTP/1.1 302 Found\r\naccess-control-allow-origin: *\r\nlocation: /data\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string(),
                    _ => {
                        let body = b"done";
                        format!(
                            "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                            body.len()
                        ) + std::str::from_utf8(body).unwrap()
                    }
                };
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        const r = await fetch("http://127.0.0.1:PORT/hop", {
                            headers: { "Authorization": "Bearer token123" },
                        });
                        return "ok:" + r.status + ":" + (await r.text());
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        msg, "ok:200:done",
        "same-origin redirect chain must complete, got: {msg}"
    );

    let hops = requests.lock().unwrap();
    assert_eq!(hops.len(), 3, "expected preflight + /hop + /data");
    let data_hop = hops[2].to_ascii_lowercase();
    assert!(
        data_hop.starts_with("get /data"),
        "third hop must be the redirected GET, got: {}",
        hops[2].lines().next().unwrap_or("")
    );
    assert!(
        data_hop.contains("authorization: bearer token123"),
        "same-origin redirect must keep the Authorization header, got: {data_hop}"
    );
}

/// Redirect credential stripping (obscura#967 same hole): once the chain
/// crosses origins the scripted Authorization header must stop riding —
/// it went out on the first hop (that origin was told at fetch time) but
/// must never reach the second, different origin.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn cross_origin_redirect_strips_scripted_credentials() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener_a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_a = listener_a.local_addr().unwrap().port();
    let requests_a = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_a = requests_a.clone();
    let listener_b = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_b = listener_b.local_addr().unwrap().port();
    let requests_b = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_b = requests_b.clone();

    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..2 {
            let Ok((mut stream, _)) = listener_a.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            seen_a
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf).to_string());
            let response = match seen_a.lock().unwrap().len() {
                1 => concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "access-control-allow-origin: *\r\n",
                    "access-control-allow-methods: GET\r\n",
                    "access-control-allow-headers: authorization\r\n",
                    "content-length: 0\r\nconnection: close\r\n\r\n",
                )
                .to_string(),
                _ => format!(
                    "HTTP/1.1 302 Found\r\naccess-control-allow-origin: *\r\nlocation: http://127.0.0.1:{}/data\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    port_b
                ),
            };
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let Ok((mut stream, _)) = listener_b.accept() else {
            return;
        };
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        seen_b
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(&buf).to_string());
        let body = b"secret";
        let response = format!(
            "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        const r = await fetch("http://127.0.0.1:PORTA/hop", {
                            headers: { "Authorization": "Bearer secret-token" },
                        });
                        return "ok:" + r.status + ":" + (await r.text());
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORTA", &port_a.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        msg, "ok:200:secret",
        "redirect chain must complete, got: {msg}"
    );

    let hops_a = requests_a.lock().unwrap();
    assert_eq!(
        hops_a.len(),
        2,
        "origin A sees the preflight and the first GET"
    );
    let first_hop = hops_a[1].to_ascii_lowercase();
    assert!(
        first_hop.contains("authorization: bearer secret-token"),
        "the initial cross-origin request carries the header it was given, got: {first_hop}"
    );

    let hops_b = requests_b.lock().unwrap();
    assert_eq!(hops_b.len(), 1, "origin B sees exactly the redirected GET");
    let second_hop = hops_b[0].to_ascii_lowercase();
    assert!(
        second_hop.starts_with("get /data"),
        "hop 2 must be the redirected GET, got: {}",
        hops_b[0].lines().next().unwrap_or("")
    );
    assert!(
        !second_hop.contains("authorization"),
        "cross-origin redirect must strip the Authorization header, got: {second_hop}"
    );
}

/// Redirect CORS enforcement (obscura#973 same hole): with a cors-tainted
/// request, the 302 response itself must pass the CORS check before the
/// follow happens — a redirect response without
/// Access-Control-Allow-Origin blocks the fetch (status 0 to JS) and the
/// second hop never goes out.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn cross_origin_redirect_without_acao_is_blocked() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // Exactly one connection: a second one would mean the redirected
        // GET leaked past the redirect-response CORS check.
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        let response = format!(
            "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:{}/data\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            port
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        await fetch("http://127.0.0.1:PORT/hop");
                        return "not-blocked";
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(msg.starts_with("rejected:Failed to fetch"), "got: {msg}");

    let events = rt.take_js_network_events();
    let blocked = events
        .iter()
        .find(|e| e.error.as_deref().unwrap_or("").contains("redirect"))
        .expect("blocked redirect must leave a network event");
    assert_eq!(blocked.status, 0);
    let error = blocked.error.as_deref().unwrap_or("");
    assert!(
        error.contains("CORS error"),
        "event must carry the redirect-CORS reason, got: {error}"
    );
}

/// Redirect downgrade (obscura#973 family, body half): a POST that lands
/// on a 302 re-issues as GET with the body headers gone — content-type
/// describes a body that no longer exists.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn redirect_downgrade_drops_body_headers() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..3 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf).to_string());
            let response = match seen.lock().unwrap().len() {
                    1 => concat!(
                        "HTTP/1.1 200 OK\r\n",
                        "access-control-allow-origin: *\r\n",
                        "access-control-allow-methods: POST\r\n",
                        "access-control-allow-headers: content-type\r\n",
                        "content-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .to_string(),
                    2 => "HTTP/1.1 302 Found\r\naccess-control-allow-origin: *\r\nlocation: /data\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string(),
                    _ => {
                        let body = b"moved";
                        format!(
                            "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                            body.len()
                        ) + std::str::from_utf8(body).unwrap()
                    }
                };
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        const r = await fetch("http://127.0.0.1:PORT/hop", {
                            method: "POST",
                            headers: { "content-type": "application/json" },
                            body: "{\"a\":1}",
                        });
                        return "ok:" + r.status + ":" + (await r.text());
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        msg, "ok:200:moved",
        "redirected POST must complete as GET, got: {msg}"
    );

    let hops = requests.lock().unwrap();
    assert_eq!(hops.len(), 3, "expected preflight + POST + redirected GET");
    assert!(
        hops[1].starts_with("POST /hop"),
        "hop 1 must be the POST, got: {}",
        hops[1].lines().next().unwrap_or("")
    );
    let downgraded = hops[2].to_ascii_lowercase();
    assert!(
        downgraded.starts_with("get /data"),
        "302 must downgrade POST to GET, got: {}",
        hops[2].lines().next().unwrap_or("")
    );
    assert!(
        !downgraded.contains("content-type"),
        "the downgraded GET must drop the body's content-type, got: {downgraded}"
    );
}

/// navigator.sendBeacon used to be a stub that returned true without
/// sending anything — pd-lib's three detection beacons silently vanished
/// (proxydetect.live never settled). A beacon must leave the machine as a
/// fire-and-forget POST; the relative URL resolves against the document.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn send_beacon_relative_url_posts_with_text_plain() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        seen.lock()
            .unwrap()
            .push(String::from_utf8_lossy(&buf).to_string());
        let response = "HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port}/page"));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const ok = navigator.sendBeacon("/beacon", "hello");
                    await new Promise(r => setTimeout(r, 800));
                    return "queued:" + ok;
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        msg, "queued:true",
        "sendBeacon must report a queued transfer"
    );

    let hops = requests.lock().unwrap();
    assert_eq!(hops.len(), 1, "exactly one beacon must reach the wire");
    let raw = hops[0].to_ascii_lowercase();
    assert!(
        raw.starts_with("post /beacon"),
        "beacon must POST to the resolved relative URL, got: {}",
        hops[0].lines().next().unwrap_or("")
    );
    assert!(
        raw.contains(
            "content-type: text/plain;charset=UTF-8"
                .to_ascii_lowercase()
                .as_str()
        ),
        "string data must ride text/plain;charset=UTF-8, got: {raw}"
    );
    assert!(
        raw.contains("\r\n\r\nhello"),
        "string data must be the body, got: {raw}"
    );
}

/// Spec content-type mapping by data type: URLSearchParams → urlencoded
/// (fetch's own default), BufferSource → application/octet-stream (the
/// caller must supply it — fetch does not), Blob → the blob's own type.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn send_beacon_body_type_to_content_type_map() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..3 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf).to_string());
            let response =
                "HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const base = "http://127.0.0.1:PORT/beacon";
                    navigator.sendBeacon(base + "?k=urlsp", new URLSearchParams("a=1&b=2"));
                    navigator.sendBeacon(base + "?k=binary", new Uint8Array([104, 105]));
                    navigator.sendBeacon(base + "?k=blob", new Blob(["csv,data"], { type: "text/csv" }));
                    await new Promise(r => setTimeout(r, 1200));
                    return "queued";
                }"#
                .replace("PORT", &port.to_string())
                .as_str(),
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(msg, "queued");

    let hops = requests.lock().unwrap();
    assert_eq!(hops.len(), 3, "all three beacons must reach the wire");
    let find = |needle: &str| hops.iter().any(|h| h.to_ascii_lowercase().contains(needle));
    assert!(
        find("content-type: application/x-www-form-urlencoded;charset=utf-8")
            && find("\r\n\r\na=1&b=2"),
        "URLSearchParams must ride the urlencoded content-type with its serialized body, got: {hops:?}"
    );
    assert!(
        find("content-type: application/octet-stream") && find("\r\n\r\nhi"),
        "BufferSource must ride octet-stream with its raw bytes, got: {hops:?}"
    );
    assert!(
        find("content-type: text/csv") && find("\r\n\r\ncsv,data"),
        "Blob must ride its own type, got: {hops:?}"
    );
}

/// An unresolvable URL fails synchronously with false — the caller gets to
/// fall back to XHR, exactly like a real browser (no queued, no network).
#[tokio::test(flavor = "current_thread")]
async fn send_beacon_invalid_url_returns_false() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"() => {
                    return "bad:" + navigator.sendBeacon("http://[bad", "x");
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        msg, "bad:false",
        "an unresolvable URL must fail synchronously with false"
    );
}

/// Preflight validation order (obscura#973 same hole): the preflight's
/// HTTP status is checked before its CORS headers, so a 403 preflight
/// reports the status — not a misleading "origin not allowed" error.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn preflight_http_status_reported_before_origin() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        let response = "HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    try {
                        await fetch("http://127.0.0.1:PORT/api", { method: "PUT" });
                        return "not-blocked";
                    } catch (e) {
                        return "rejected:" + (e && e.message);
                    }
                }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    // #62: the page face is Chrome's bare TypeError — the diagnostic lives
    // in the network event (Chrome's console-equivalent split).
    assert_eq!(msg, "rejected:Failed to fetch", "got: {msg}");
    let events = rt.take_js_network_events();
    let pf = events
        .iter()
        .find(|e| e.url.contains(&format!(":{port}/api")))
        .expect("403 preflight must leave a network event");
    let reason = pf.error.as_deref().unwrap_or("");
    assert!(
        reason.contains("CORS preflight returned HTTP 403"),
        "403 preflight must report its status in the event, got: {reason}"
    );
    assert!(
        !reason.contains("not in Access-Control-Allow-Origin"),
        "status check must fire before the origin check, got: {reason}"
    );
}

/// #163: a same-origin credentialed fetch whose endpoint 302s to a
/// cross-origin host without CORS headers must fail (Chrome parity — the
/// response is cross-origin-tainted and unauthorized), and the error must
/// NAME the redirect target so the failure can't read as "engine
/// misjudged a same-origin request". The bounce host authorizing the
/// origin (ACAO + ACAC) makes the same flow succeed.
#[tokio::test(flavor = "current_thread")]
async fn same_origin_credentialed_fetch_redirected_cross_origin_names_bounce_host() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");
    use std::io::{Read, Write};

    // Deny host: 200, no CORS headers.
    let listener_deny = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_deny = listener_deny.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener_deny.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nno")
            .unwrap();
    });

    // Allow host: echoes the request's Origin back with credentials
    // consent — the standard "authorized bounce" shape.
    let listener_allow = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_allow = listener_allow.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener_allow.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]).into_owned();
        let origin = req
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("origin:"))
            .map(|l| l[7..].trim().to_string())
            .unwrap_or_default();
        let resp = format!(
            "HTTP/1.1 200 OK\r\naccess-control-allow-origin: {origin}\r\naccess-control-allow-credentials: true\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
        );
        stream.write_all(resp.as_bytes()).unwrap();
    });

    // Page origin A: both endpoints redirect cross-origin, by path.
    let listener_a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_a = listener_a.local_addr().unwrap().port();
    let (redirect_tx, redirect_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for _ in 0..2 {
            let Ok((mut stream, _)) = listener_a.accept() else {
                return;
            };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
            redirect_tx.send(path.clone()).unwrap();
            let location = if path.contains("bounce-deny") {
                format!("http://127.0.0.1:{port_deny}/bounced")
            } else {
                format!("http://127.0.0.1:{port_allow}/ok")
            };
            let resp = format!(
                "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            stream.write_all(resp.as_bytes()).unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{port_a}/page"));
    let script = r#"async (pa, pd) => {
            const A = "http://127.0.0.1:" + pa;
            const out = [];
            try {
                const r = await fetch(A + "/bounce-deny", { credentials: "include" });
                out.push("deny:" + r.status);
            } catch (e) {
                out.push("deny:" + (e && e.message));
            }
            try {
                const r = await fetch(A + "/bounce-allow", { credentials: "include" });
                out.push("allow:" + r.status);
            } catch (e) {
                out.push("allow:" + (e && e.message));
            }
            return out.join("|");
        }"#;
    let result = rt
        .call_function_on_remote(
            script,
            None,
            &[
                serde_json::json!({ "value": port_a }),
                serde_json::json!({ "value": port_deny }),
            ],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let got = result
        .value
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    let expected_deny = "deny:Failed to fetch".to_string();
    assert_eq!(got, format!("{expected_deny}|allow:200"), "got: {got}");
    assert_eq!(
        redirect_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        "/bounce-deny"
    );
}

// Issue #139 — proxy_url must thread through to both the ES-module
// loader (module_loader.rs) and op_fetch_url's client
// (ops.rs::build_request_client). Pre-fix both built clients with
// `Client::builder().build()` — no proxy — so JS fetch/XHR and
// dynamic imports silently bypassed BrowserContext.proxy_url.
//
// Phase 5.5 RED check: each test references a symbol that does NOT
// exist on main (proxy_url() accessor, with_proxy ctor,
// with_base_url_and_proxy ctor), so the tests fail to compile without
// the prod fix.
#[test]
fn http_client_round_trips_proxy_url() {
    use diting_net::{CookieJar, HttpClient};
    let jar = std::sync::Arc::new(CookieJar::new());
    let configured = HttpClient::with_options(jar.clone(), Some("http://proxy.test:8080"));
    assert_eq!(
        configured.proxy_url(),
        Some("http://proxy.test:8080"),
        "proxy_url() must expose the value passed to with_options"
    );

    let direct = HttpClient::with_options(jar, None);
    assert_eq!(
        direct.proxy_url(),
        None,
        "proxy_url() must return None when no proxy was configured"
    );
}

#[test]
fn runtime_with_base_url_and_proxy_constructs_successfully() {
    // Sanity-check the public ctor that page.rs uses to thread proxy
    // through to the module loader. Direct (None) and proxied paths
    // must both initialise the JS environment.
    let _direct = JsRuntime::with_base_url_and_proxy("https://example.com/", None);
    let _proxied = JsRuntime::with_base_url_and_proxy(
        "https://example.com/",
        Some("http://proxy.test:8080".to_string()),
    );
}

#[test]
fn fetch_referer_policy_table() {
    use crate::ops::fetch_referer;
    let doc = url::Url::parse("http://example.com/page?a=1").unwrap();
    let same = url::Url::parse("http://example.com/other").unwrap();
    let cross = url::Url::parse("http://other.example/x").unwrap();
    let http_target = url::Url::parse("http://127.0.0.1:9/x").unwrap();
    let https_doc = url::Url::parse("https://example.com/page").unwrap();

    // Default policy (empty token): same-origin full URL, cross-origin origin.
    assert_eq!(
        fetch_referer("", "about:client", &doc, &same),
        "http://example.com/page?a=1"
    );
    assert_eq!(
        fetch_referer("", "about:client", &doc, &cross),
        "http://example.com/"
    );

    // Explicit no-referrer and empty-string referrer suppress entirely.
    assert_eq!(
        fetch_referer("no-referrer", "about:client", &doc, &same),
        ""
    );
    assert_eq!(fetch_referer("", "", &doc, &same), "");

    // unsafe-url sends the full URL cross-origin.
    assert_eq!(
        fetch_referer("unsafe-url", "about:client", &doc, &cross),
        "http://example.com/page?a=1"
    );

    // origin strips even same-origin down to the origin.
    assert_eq!(
        fetch_referer("origin", "about:client", &doc, &same),
        "http://example.com/"
    );

    // strict-origin and the default suppress on https->http downgrade;
    // the legacy origin policies do not.
    assert_eq!(
        fetch_referer("strict-origin", "about:client", &https_doc, &http_target),
        ""
    );
    assert_eq!(
        fetch_referer("", "about:client", &https_doc, &http_target),
        ""
    );
    assert_eq!(
        fetch_referer("origin", "about:client", &https_doc, &http_target),
        "https://example.com/"
    );

    // no-referrer-when-downgrade keeps the full URL (same scheme).
    assert_eq!(
        fetch_referer("no-referrer-when-downgrade", "about:client", &doc, &same),
        "http://example.com/page?a=1"
    );

    // Explicit referrer override replaces the document; this override is
    // same-origin with the target, so the default policy keeps the full URL
    // (fragment stripped). The policy still strips cross-origin overrides,
    // and credentials/fragments never reach the wire.
    assert_eq!(
        fetch_referer("", "http://other.example/deep#frag", &doc, &cross),
        "http://other.example/deep"
    );
    assert_eq!(
        fetch_referer("unsafe-url", "http://u:p@example.com/a#f", &doc, &cross),
        "http://example.com/a"
    );

    // Non-HTTP(S) referrer values are no-referrer, per spec.
    assert_eq!(
        fetch_referer("unsafe-url", "file:///etc/passwd", &doc, &cross),
        ""
    );
}

/// Comma-separated policy values (Referrer-Policy header or <meta> content):
/// tokens are case-insensitive, invalid ones are skipped, the LAST valid
/// token wins; all-invalid yields no policy.
#[test]
fn last_valid_referrer_token_parsing() {
    use crate::ops::last_valid_referrer_token;
    assert_eq!(
        last_valid_referrer_token("no-referrer").as_deref(),
        Some("no-referrer")
    );
    // Case-insensitive, surrounding whitespace trimmed.
    assert_eq!(
        last_valid_referrer_token("  ORIGIN ").as_deref(),
        Some("origin")
    );
    // The last valid token wins over an earlier one.
    assert_eq!(
        last_valid_referrer_token("origin, no-referrer").as_deref(),
        Some("no-referrer")
    );
    assert_eq!(
        last_valid_referrer_token("no-referrer, origin").as_deref(),
        Some("origin")
    );
    // Invalid tokens are skipped around a valid one.
    assert_eq!(
        last_valid_referrer_token("garbage, strict-origin").as_deref(),
        Some("strict-origin")
    );
    // All-invalid (including near-miss prefixes) yields no policy.
    assert_eq!(last_valid_referrer_token("only-garbage"), None);
    assert_eq!(last_valid_referrer_token(""), None);
    assert_eq!(last_valid_referrer_token("no-referer"), None);
}

/// RequestInit's referrerPolicy/referrer reach the wire (obscura#875
/// family): the default policy strips cross-origin to the origin,
/// no-referrer sends nothing, unsafe-url sends the full document URL, and an
/// explicit referrer override rides through the same policy table. Before
/// the fix all three scripted options were ignored on the wire.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn fetch_referrer_policy_reaches_the_wire() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen = captured.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..4 {
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
            let response = format!(
                "HTTP/1.1 200 OK\r\naccess-control-allow-origin: *\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
        }
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("http://example.com/page?a=1");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                const base = "http://127.0.0.1:PORT";
                await fetch(base + "/default");
                await fetch(base + "/noref", { referrerPolicy: "no-referrer" });
                await fetch(base + "/unsafe", { referrerPolicy: "unsafe-url" });
                await fetch(base + "/explicit", { referrer: "http://other.example/deep" });
                return "done";
            }"#
            .replace("PORT", &port.to_string())
            .as_str(),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_str().unwrap_or_default(), "done");

    let requests = captured.lock().unwrap();
    let referer_of = |path: &str| -> Option<String> {
        requests
            .iter()
            .find(|r| r.contains(&format!("GET {path} ")))
            .and_then(|r| {
                r.lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("referer:"))
                    .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
            })
    };
    assert_eq!(
        referer_of("/default").as_deref(),
        Some("http://example.com/")
    );
    assert_eq!(referer_of("/noref"), None);
    assert_eq!(
        referer_of("/unsafe").as_deref(),
        Some("http://example.com/page?a=1")
    );
    assert_eq!(
        referer_of("/explicit").as_deref(),
        Some("http://other.example/")
    );
}
