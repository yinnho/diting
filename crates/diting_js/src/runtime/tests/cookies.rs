//! document.cookie + CookieJar 合并/httponly/跨 iframe 共享
use super::super::*;
use super::common::{setup_runtime, setup_runtime_with_cookies};

/// Set-Cookie on a sync same-origin XHR must land in the cookie jar and
/// become visible to document.cookie.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn sync_xhr_set_cookie_reaches_jar() {
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
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\nset-cookie: synck=1; Path=/\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .unwrap();
        stream.flush().unwrap();
    });

    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"() => {
                    const x = new XMLHttpRequest();
                    x.open('GET', '/login', false);
                    x.send();
                    return { status: x.status, cookie: document.cookie };
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
            "cookie": "synck=1",
        })
    );
}

/// (#228) Script rows used to surface only the JS-shaped header set —
/// the Cookie the transport actually sent never reached the network
/// faces, so HAR/CDP forensics on "which cookies did this signed call
/// carry" (the #226 doudian WAF, the taobao cookie-rotation race) had
/// to infer from the nav row. The hop's cookie now rides the recorded
/// set on the plain path; served Buffered outcomes prefer the
/// transport's own #97 wire snapshot.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn scripted_fetch_event_carries_wire_cookie_header() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        // First connection plants the cookie; the second echoes the
        // Cookie header it received, so the wire truth is assertable
        // alongside the recorded event.
        let (mut s1, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = s1.read(&mut buf).unwrap();
        s1.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\nset-cookie: forensics=k1; Path=/\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").unwrap();
        s1.flush().unwrap();
        let (mut s2, _) = listener.accept().unwrap();
        let mut buf2 = [0u8; 4096];
        let n = s2.read(&mut buf2).unwrap();
        let req = String::from_utf8_lossy(&buf2[..n]).to_string();
        let cookie = req
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("cookie:"))
            .map(|l| l["cookie:".len()..].trim().to_string())
            .unwrap_or_default();
        let body = format!("cookie-was:{}", cookie);
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        s2.write_all(resp.as_bytes()).unwrap();
        s2.flush().unwrap();
    });

    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    await fetch('/login');
                    const r = await fetch('/api');
                    return await r.text();
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
        serde_json::json!("cookie-was:forensics=k1"),
        "the wire must have carried the jar cookie"
    );

    let events = rt.take_js_network_events();
    let api = events
        .iter()
        .rev()
        .find(|e| e.url.ends_with("/api"))
        .expect("/api must leave a recorded event");
    let cookie_hdr = api
        .request_headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("cookie"))
        .map(|(_, v)| v.as_str())
        .unwrap_or_default();
    assert!(
        cookie_hdr.contains("forensics=k1"),
        "recorded script row must carry the wire Cookie header, got: {:?}",
        cookie_hdr
    );
}

#[test]
fn test_document_cookie_reads_http_cookies() {
    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let url = url::Url::parse("http://example.com/test").unwrap();
    jar.set_cookie("session=abc123; Path=/", &url);
    jar.set_cookie("theme=dark; Path=/", &url);
    let result = rt.evaluate("document.cookie").unwrap();
    let cookie_str = result.as_str().unwrap();
    assert!(
        cookie_str.contains("session=abc123"),
        "expected session cookie, got: {}",
        cookie_str
    );
    assert!(
        cookie_str.contains("theme=dark"),
        "expected theme cookie, got: {}",
        cookie_str
    );
}

#[test]
fn test_document_cookie_excludes_httponly() {
    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let url = url::Url::parse("http://example.com/test").unwrap();
    jar.set_cookie("visible=yes; Path=/", &url);
    jar.set_cookie("secret=token; Path=/; HttpOnly", &url);
    let result = rt.evaluate("document.cookie").unwrap();
    let cookie_str = result.as_str().unwrap();
    assert!(
        cookie_str.contains("visible=yes"),
        "expected visible cookie, got: {}",
        cookie_str
    );
    assert!(
        !cookie_str.contains("secret"),
        "httpOnly cookie should not be visible to JS, got: {}",
        cookie_str
    );
}

#[test]
fn test_document_cookie_setter_stores_in_jar() {
    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.evaluate("document.cookie = 'foo=bar; Path=/'").unwrap();
    let url = url::Url::parse("http://example.com/test").unwrap();
    let result = rt.evaluate("document.cookie").unwrap();
    assert!(result.as_str().unwrap().contains("foo=bar"));
    let header = jar.get_cookie_header(&url);
    assert!(
        header.contains("foo=bar"),
        "cookie should be in jar, got: {}",
        header
    );
}

#[test]
fn test_document_cookie_delete_via_max_age() {
    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let url = url::Url::parse("http://example.com/test").unwrap();
    rt.evaluate("document.cookie = 'temp=val; Path=/'").unwrap();
    assert!(rt
        .evaluate("document.cookie")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("temp=val"));
    rt.evaluate("document.cookie = 'temp=; Max-Age=0'").unwrap();
    let result = rt.evaluate("document.cookie").unwrap();
    assert!(
        !result.as_str().unwrap().contains("temp="),
        "cookie should be deleted, got: {}",
        result
    );
    assert!(!jar.get_cookie_header(&url).contains("temp="));
}

#[test]
fn test_document_cookie_js_and_http_merge() {
    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let url = url::Url::parse("http://example.com/test").unwrap();
    jar.set_cookie("server_sid=xyz; Path=/", &url);
    rt.evaluate("document.cookie = 'client_pref=light'")
        .unwrap();
    let result = rt.evaluate("document.cookie").unwrap();
    let cookie_str = result.as_str().unwrap();
    assert!(
        cookie_str.contains("server_sid=xyz"),
        "expected server cookie, got: {}",
        cookie_str
    );
    assert!(
        cookie_str.contains("client_pref=light"),
        "expected client cookie, got: {}",
        cookie_str
    );
}

#[test]
fn test_document_cookie_empty_when_no_cookies() {
    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let result = rt.evaluate("document.cookie").unwrap();
    assert_eq!(result.as_str().unwrap(), "");
}

/// #274, re-diagnosed: about:blank initial documents have no cookie
/// origin, and Chrome THROWS SecurityError on both document.cookie read
/// and write (different prepositions) — it never silently no-ops. The
/// 10-18 probe saw a silent drop here and filed it as a UTF-8 value
/// face; the value face was always fine — the stub document was the drop.
#[test]
fn test_document_cookie_about_blank_throws_security_error() {
    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.set_url("about:blank");
    let get = rt
        .evaluate(r#"try { document.cookie; "no-throw" } catch (e) { e.name + "|" + e.message }"#)
        .unwrap();
    assert_eq!(
        get.as_str().unwrap(),
        "SecurityError|Failed to read the 'cookie' property from 'Document': Access is denied for this document."
    );
    let set = rt
            .evaluate(r#"try { document.cookie = "mb=你好世界; path=/"; "no-throw" } catch (e) { e.name + "|" + e.message }"#)
            .unwrap();
    assert_eq!(
        set.as_str().unwrap(),
        "SecurityError|Failed to set the 'cookie' property on 'Document': Access is denied for this document."
    );
}

/// #274: multi-byte UTF-8 values round-trip on a real origin — the jar
/// stores raw bytes and Chrome accepts any UTF-8 in values. Pinned so a
/// future "strict cookie-octet" pass cannot regress the value face.
#[test]
fn test_document_cookie_multibyte_utf8_roundtrip() {
    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.evaluate(
        r#"document.cookie = "mb=你好世界; path=/"; document.cookie = "emoji=😀🎉; path=/";"#,
    )
    .unwrap();
    let read = rt.evaluate("document.cookie").unwrap();
    let s = read.as_str().unwrap();
    assert!(s.contains("mb=你好世界"), "CJK value must round-trip: {s}");
    assert!(s.contains("emoji=😀🎉"), "emoji value must round-trip: {s}");
}

/// #274, the real second face: same-origin/srcdoc iframes share the
/// parent's cookie jar — the frame inherits the parent origin, and
/// Chrome pins `d.cookie = x` set inside a srcdoc frame landing in the
/// parent's read-back. The old iframe-document stub silently dropped
/// every cookie.
#[tokio::test(flavor = "current_thread")]
async fn test_iframe_document_cookie_shares_parent_jar() {
    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let script = r##"async () => {
            const f = document.createElement('iframe');
            f.setAttribute('srcdoc', '<p>x</p>');
            document.body.appendChild(f);
            await new Promise(r => setTimeout(r, 50));
            const d = f.contentDocument;
            d.cookie = 'ifmb=iframe值; path=/';
            d.cookie = 'ifascii=ok; path=/';
            return { inFrame: d.cookie, parentSees: document.cookie };
        }"##;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let in_frame = v["inFrame"].as_str().unwrap_or("");
    assert!(
        in_frame.contains("ifmb=iframe值"),
        "iframe read-back must see its CJK cookie: {v}"
    );
    assert!(
        in_frame.contains("ifascii=ok"),
        "iframe read-back must see its ascii cookie: {v}"
    );
    assert!(
        v["parentSees"]
            .as_str()
            .unwrap_or("")
            .contains("ifmb=iframe值"),
        "parent must see the iframe's cookie (shared jar): {v}"
    );
}

#[test]
fn queued_navigation_does_not_move_the_cookie_context() {
    // obscura #940 shape: op_navigate must only queue the navigation —
    // the realm URL moves on commit, not at assignment time. The early
    // move let synchronous JS between `location.href = target` and the
    // actual navigation read and write the TARGET origin's cookies
    // through document.cookie, whose ops derive the domain from that
    // URL (SOP bypass). The navigation tuple itself must still be queued.
    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    let victim = url::Url::parse("https://victim.example/").unwrap();
    jar.set_cookie("secret=victimtoken; Path=/", &victim);

    rt.evaluate("location.href = 'https://victim.example/'")
        .unwrap();
    let cookie_str = rt
        .evaluate("document.cookie")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        !cookie_str.contains("victimtoken"),
        "document.cookie must not expose another origin's cookies while the navigation is only queued, got: {}",
        cookie_str
    );

    // The write side of the same hole: a JS cookie set while the
    // navigation is queued must land on the CURRENT origin, not the
    // navigation target.
    rt.evaluate("document.cookie = 'poison=1; Path=/'").unwrap();
    assert!(
        !jar.get_cookie_header(&victim).contains("poison=1"),
        "a JS cookie written before navigation commit must not land on the target origin"
    );

    // The navigation itself is unaffected: still queued, GET, empty body.
    assert_eq!(
        rt.take_pending_navigation(),
        Some((
            "https://victim.example/".to_string(),
            "GET".to_string(),
            "".to_string()
        ))
    );
}

#[test]
fn test_document_cookie_no_jar_returns_empty() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate("document.cookie").unwrap();
    assert_eq!(result.as_str().unwrap(), "");
}

/// #203: an iframe load is a child-frame NAVIGATION, so its fetch must
/// carry credentials — `same-origin` (the fetch default the loader used
/// to inherit) sends nothing cross-origin, the server answers as a
/// guest, and its Set-Cookie mint lands on the shared domain over the
/// login state. The taobao editor's detailDescPreview iframe was
/// silently logging the session out this way.
#[tokio::test(flavor = "current_thread")]
async fn iframe_load_sends_cookies_cross_origin() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    use std::io::{Read, Write};
    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }

    // Page origin A: seeds a cookie via a same-origin fetch.
    let listener_a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_a = listener_a.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener_a.accept() else {
            return;
        };
        read_request(&mut stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\nset-cookie: s=login; Path=/\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").unwrap();
    });

    // Cross-origin B: the iframe target — reports the Cookie header it saw.
    let listener_b = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port_b = listener_b.local_addr().unwrap().port();
    let (cookie_tx, cookie_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener_b.accept() else {
            return;
        };
        let req = read_request(&mut stream);
        let seen = req
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("cookie:"))
            .map(|l| l[7..].trim().to_string())
            .unwrap_or_default();
        cookie_tx.send(seen).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 48\r\nconnection: close\r\n\r\n<html><body>preview</body></html>").unwrap();
    });

    let (mut rt, _jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port_a));
    let result = rt.call_function_on_remote(
            r#"async (pa, pb) => {
                await fetch("http://127.0.0.1:" + pa + "/seed");
                return await new Promise(resolve => {
                    const iframe = document.createElement('iframe');
                    // Chrome fires the initial about:blank load inside the
                    // append; the navigation to src is the second load.
                    let loads = 0;
                    iframe.addEventListener('load', () => { if (++loads === 2) resolve('loaded'); });
                    document.body.appendChild(iframe);
                    iframe.src = "http://127.0.0.1:" + pb + "/preview.htm";
                });
            }"#,
            None,
            &[
                serde_json::json!({ "value": port_a }),
                serde_json::json!({ "value": port_b }),
            ],
            true,
            true,
        ).await.unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    assert_eq!(result.value.unwrap(), serde_json::json!("loaded"));
    let seen = cookie_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        seen.split("; ").any(|c| c == "s=login"),
        "iframe load must carry the jar's cookies cross-origin (navigation semantics), saw: '{seen}'"
    );
}

/// Real browsers send and store cookies on cross-origin classic
/// `<script src>` requests (the JSONP-era contract: taobao mtop's token
/// refresh rotates `_m_h5_tk` via Set-Cookie on a cross-origin JSONP GET,
/// and with credentials "same-origin" the refresh looped on
/// FAIL_SYS_TOKEN_EMPTY forever, leaving decorated shop pages empty).
/// Page on port A, script server on port B — different ports are
/// different origins, same 127.0.0.1 host so jar cookies domain-match.
#[tokio::test(flavor = "current_thread")]
async fn test_dynamic_cross_origin_script_sends_and_stores_cookies() {
    // The tuple field is the RAII payload (holding the env lock); Drop
    // is its reader — it removes the env var when the guard releases.
    #[allow(dead_code)]
    struct PrivateNetGuard(std::sync::MutexGuard<'static, ()>);
    impl Drop for PrivateNetGuard {
        fn drop(&mut self) {
            std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
        }
    }
    let guard = PrivateNetGuard(crate::PRIVATE_NET_ENV_LOCK.lock().unwrap());
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Owns the page origin's port (nothing serves it — the runtime's
    // document is injected locally; binding keeps the OS from reusing it).
    let page_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let page_port = page_listener.local_addr().unwrap().port();

    let script_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let script_port = script_listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..3 {
            let Ok((mut stream, _)) = script_listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let sent_cookie = req.lines().any(|l| {
                let lower = l.to_ascii_lowercase();
                lower.starts_with("cookie:") && lower.contains("probe=")
            });
            let body: &[u8] = if sent_cookie {
                // Runs in the page realm, so document.cookie must already
                // reflect the Set-Cookie stored from these very headers.
                b"window.__dynEcho = 'COOKIE_SENT';"
            } else {
                b"window.__dynEcho = 'NO_COOKIE';"
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/javascript\r\nset-cookie: dyn_sc=stored; Path=/\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
        }
    });

    let (mut rt, jar) = setup_runtime_with_cookies("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", page_port));
    jar.set_cookie(
        "probe=1; Path=/",
        &url::Url::parse(&format!("http://127.0.0.1:{}/page", page_port)).unwrap(),
    );

    let script = format!(
        r#"async () => {{
            const s = document.createElement('script');
            s.setAttribute('src', 'http://127.0.0.1:{script_port}/jsonp.js');
            document.body.appendChild(s);
            await new Promise(r => setTimeout(r, 300));
            return window.__dynEcho;
        }}"#
    );
    let result = rt
        .call_function_on_remote(&script, None, &[], true, true)
        .await
        .unwrap();
    drop(guard);

    assert_eq!(result.value.unwrap(), serde_json::json!("COOKIE_SENT"));
    let all = jar.get_all_cookies();
    let stored = all
        .iter()
        .find(|c| c.name == "dyn_sc")
        .expect("Set-Cookie from cross-origin dynamic script must land in the jar");
    assert_eq!(stored.value, "stored");
}
