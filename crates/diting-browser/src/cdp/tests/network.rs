//! `Network.setBlockedURLs` and `setExtraHTTPHeaders`: a blocked URL is
//! never requested, and the headers reach the wire.

use super::*;

/// `/page/{n}` pulls a stylesheet, a script and an image of its number;
/// `/echo` answers with the `x-probe` header it was sent. Every path
/// requested is appended to the log.
///
/// The routes take the whole segment after the slash as their name —
/// `"1.css"`, not `1`. A path parameter is a whole segment; one followed
/// by a suffix (`/sheet/:n.css`) matches `/sheet/1.css` only with `n` as
/// `"1.css"`, and a `u32` extractor answers 400 before the handler runs.
#[derive(Clone, Default)]
struct Site(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl Site {
    fn hits(&self, path: &str) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|hit| hit.as_str() == path)
            .count()
    }

    fn log(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

async fn site_page(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(n): axum::extract::Path<u32>,
) -> axum::response::Html<String> {
    site.0.lock().unwrap().push("/page".into());
    axum::response::Html(format!(
        "<html><head><link rel=\"stylesheet\" href=\"/sheet/{n}.css\"></head>\
         <body style=\"margin:0\"><img src=\"/pixel/{n}.png\" width=20 height=20>\
         <div id=s>no</div><script src=\"/script/{n}.js\"></script></body></html>"
    ))
}

async fn site_pixel(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    site.0.lock().unwrap().push(format!("/pixel/{name}"));
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        PIXEL.to_vec(),
    )
}

async fn site_sheet(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    site.0.lock().unwrap().push(format!("/sheet/{name}"));
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        "#s{color:rgb(7,8,9)}",
    )
}

async fn site_script(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    site.0.lock().unwrap().push(format!("/script/{name}"));
    (
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        "document.getElementById('s').textContent += 'js'",
    )
}

async fn site_echo(
    axum::extract::State(site): axum::extract::State<Site>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    site.0.lock().unwrap().push("/echo".into());
    let probe = headers
        .get("x-probe")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("none");
    axum::response::Html(format!("<html><body id=b>{probe}</body></html>"))
}

/// Answers, but names another origin than the page's: a CORS refusal
/// whose header is there and wrong.
async fn site_mismatch() -> impl axum::response::IntoResponse {
    (
        [
            (axum::http::header::CONTENT_TYPE, "text/plain"),
            (
                axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                "http://example.test",
            ),
        ],
        "x",
    )
}

/// A 1×1 png.
static PIXEL: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==")
        .expect("1x1 png")
});

fn site() -> (axum::Router, Site) {
    let log = Site::default();
    let router = axum::Router::new()
        .route("/page/:n", axum::routing::get(site_page))
        .route("/pixel/:n.png", axum::routing::get(site_pixel))
        .route("/sheet/:n.css", axum::routing::get(site_sheet))
        .route("/script/:n.js", axum::routing::get(site_script))
        .route("/echo", axum::routing::get(site_echo))
        .route("/mismatch", axum::routing::get(site_mismatch))
        .with_state(log.clone());
    (router, log)
}

async fn await_hits(site: &Site, path: &str, count: usize) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while site.hits(path) < count {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    true
}

#[tokio::test]
async fn blocked_urls_never_reach_the_wire_and_extra_headers_do() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/page/1")).await;
    let on = Some(session.as_str());
    // The script's text and the stylesheet's colour: what each left on
    // the page.
    let shown = "document.getElementById('s').textContent";
    // The colour needs the style engine, which the `screenshot` feature
    // brings; the wire does not.
    #[cfg(feature = "screenshot")]
    let color = "getComputedStyle(document.getElementById('s')).color";
    for path in ["/sheet/1.css", "/script/1.js"] {
        assert!(
            await_hits(&site, path, 1).await,
            "{path} was never requested: {:?}",
            site.log()
        );
    }
    assert_eq!(client.eval(shown, &session).await, "nojs");
    #[cfg(feature = "screenshot")]
    assert_eq!(client.eval(color, &session).await, "rgb(7, 8, 9)");

    // An image, a stylesheet and a script, blocked: none is requested, and
    // the page shows the two it was to change.
    client
        .run(
            "Network.setBlockedURLs",
            json!({ "urls": ["*/pixel/2.png", "*/script/2.js", "*/sheet/2.css"] }),
            on,
        )
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/page/2") }),
            on,
        )
        .await;
    let text = client.eval(shown, &session).await;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(
        (
            site.hits("/pixel/2.png"),
            site.hits("/sheet/2.css"),
            site.hits("/script/2.js"),
        ),
        (0, 0, 0),
        "a blocked resource was requested: {:?}",
        site.log()
    );
    assert_eq!(text, "no");
    #[cfg(feature = "screenshot")]
    assert_eq!(client.eval(color, &session).await, "rgb(0, 0, 0)");

    // Unblocked, the next page is its whole self.
    client
        .run("Network.setBlockedURLs", json!({ "urls": [] }), on)
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/page/3") }),
            on,
        )
        .await;
    assert!(
        await_hits(&site, "/script/3.js", 1).await,
        "an unblocked script was not requested: {:?}",
        site.log()
    );
    assert_eq!(client.eval(shown, &session).await, "nojs");
    #[cfg(feature = "screenshot")]
    assert_eq!(client.eval(color, &session).await, "rgb(7, 8, 9)");

    client
        .run(
            "Network.setExtraHTTPHeaders",
            json!({ "headers": { "X-Probe": "sent" } }),
            on,
        )
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo") }),
            on,
        )
        .await;
    assert_eq!(
        client
            .eval("document.getElementById('b').textContent", &session)
            .await,
        "sent"
    );
    client
        .run("Network.setExtraHTTPHeaders", json!({ "headers": {} }), on)
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo?again") }),
            on,
        )
        .await;
    assert_eq!(
        client
            .eval("document.getElementById('b').textContent", &session)
            .await,
        "none"
    );
}

/// What `events` says of `method`: the params of the first such event.
fn event_params<'a>(events: &'a [Value], method: &str) -> Option<&'a Value> {
    events
        .iter()
        .find(|event| event["method"] == method)
        .map(|event| &event["params"])
}

fn position_of(events: &[Value], method: &str) -> Option<usize> {
    events.iter().position(|event| event["method"] == method)
}

#[tokio::test]
async fn a_navigation_is_told_as_its_requests_before_frame_navigated() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    client.run("Page.enable", json!({}), on).await;
    client.run("Network.enable", json!({}), on).await;
    client.events.clear();
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo") }),
            on,
        )
        .await;
    let events = std::mem::take(&mut client.events);
    let sent = event_params(&events, "Network.requestWillBeSent")
        .unwrap_or_else(|| panic!("no requestWillBeSent in {events:?}"));
    assert_eq!(sent["type"], "Document");
    assert_eq!(sent["request"]["url"], format!("{origin}/echo"));
    assert_eq!(sent["request"]["method"], "GET");
    let received = event_params(&events, "Network.responseReceived").expect("responseReceived");
    assert_eq!(received["response"]["status"], 200);
    assert_eq!(received["response"]["mimeType"], "text/html");
    assert_eq!(received["requestId"], sent["requestId"]);
    let finished = event_params(&events, "Network.loadingFinished").expect("loadingFinished");
    assert_eq!(finished["requestId"], sent["requestId"]);
    // Playwright looks for the document's request when the frame
    // navigates: it has to be there already.
    let navigated = position_of(&events, "Page.frameNavigated").expect("frameNavigated");
    for method in [
        "Network.requestWillBeSent",
        "Network.responseReceived",
        "Network.loadingFinished",
    ] {
        assert!(
            position_of(&events, method).unwrap() < navigated,
            "{method} came after frameNavigated"
        );
    }
    assert_eq!(sent["loaderId"], received["loaderId"]);
}

#[tokio::test]
async fn a_request_the_page_makes_between_commands_is_pushed() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/echo")).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "fetch('/echo?later'); 1" }),
            on,
        )
        .await;
    client.events.clear();
    // Nothing is sent from here: the page's own fetch finishes on the
    // idle page, and its rows are pushed.
    let seen = client
        .events_until("Network.loadingFinished", Duration::from_secs(5))
        .await;
    let sent = seen
        .iter()
        .find(|event| {
            event["method"] == "Network.requestWillBeSent"
                && event["params"]["request"]["url"]
                    .as_str()
                    .is_some_and(|url| url.ends_with("/echo?later"))
        })
        .unwrap_or_else(|| panic!("the fetch was not told: {seen:?}; log {:?}", site.log()));
    assert_eq!(sent["params"]["type"], "Fetch");
    assert_eq!(sent["sessionId"], session);
    assert!(position_of(&seen, "Network.responseReceived").is_some());
}

#[tokio::test]
async fn a_session_without_network_is_told_nothing() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    client.events.clear();
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo") }),
            on,
        )
        .await;
    let events = std::mem::take(&mut client.events);
    assert!(
        events.iter().all(|event| !event["method"]
            .as_str()
            .unwrap_or("")
            .starts_with("Network.")),
        "Network events without Network.enable: {events:?}"
    );
}

/// The `requestId` of the response `events` carries for a URL ending in
/// `suffix`.
fn request_id_of(events: &[Value], suffix: &str) -> String {
    events
        .iter()
        .filter(|event| event["method"] == "Network.responseReceived")
        .find(|event| {
            event["params"]["response"]["url"]
                .as_str()
                .is_some_and(|url| url.ends_with(suffix))
        })
        .and_then(|event| event["params"]["requestId"].as_str())
        .unwrap_or_else(|| panic!("no response for {suffix} in {events:?}"))
        .to_string()
}

#[tokio::test]
async fn a_response_body_is_what_the_page_kept_worded_as_chrome_words_it() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    client.events.clear();
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo") }),
            on,
        )
        .await;
    let document = request_id_of(&client.events, "/echo");
    let body = client
        .run(
            "Network.getResponseBody",
            json!({ "requestId": document }),
            on,
        )
        .await;
    assert_eq!(body["base64Encoded"], false);
    assert!(
        body["body"]
            .as_str()
            .unwrap_or("")
            .contains("<body id=b>none</body>"),
        "the document's text: {body}"
    );

    // A script's fetch, text and binary: the binary one is base64.
    client.events.clear();
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "fetch('/echo?t').then(r => r.text()); \
                                   fetch('/pixel/1.png').then(r => r.arrayBuffer()); 1" }),
            on,
        )
        .await;
    // Two requests, two ends: what `events_until` takes off the socket is
    // all there is to read them from.
    let mut seen = client
        .events_until("Network.loadingFinished", Duration::from_secs(5))
        .await;
    seen.extend(
        client
            .events_until("Network.loadingFinished", Duration::from_secs(5))
            .await,
    );
    let text_id = request_id_of(&seen, "/echo?t");
    let png_id = request_id_of(&seen, "/pixel/1.png");
    let text = client
        .run(
            "Network.getResponseBody",
            json!({ "requestId": text_id }),
            on,
        )
        .await;
    assert_eq!(text["base64Encoded"], false);
    assert!(text["body"].as_str().unwrap_or("").contains("<body id=b>"));
    let png = client
        .run(
            "Network.getResponseBody",
            json!({ "requestId": png_id }),
            on,
        )
        .await;
    assert_eq!(png["base64Encoded"], true);
    assert!(png["body"]
        .as_str()
        .unwrap_or("")
        .starts_with("iVBORw0KGgo"));
}

#[tokio::test]
async fn a_response_body_nobody_kept_is_chromes_refusal() {
    let _guard = CDP_LOCK.lock().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    // Chrome answers an id it never issued the same way before and after
    // `Network.enable`.
    for enabled in [false, true] {
        if enabled {
            client.run("Network.enable", json!({}), on).await;
        }
        for id in ["no-such-request", ""] {
            assert_eq!(
                client
                    .refusal("Network.getResponseBody", json!({ "requestId": id }), on)
                    .await,
                "No resource with given identifier found"
            );
        }
    }
    for params in [json!({}), json!({ "requestId": 5 })] {
        assert_eq!(
            client.invalid("Network.getResponseBody", params, on).await,
            "Invalid parameters"
        );
    }
}

#[tokio::test]
async fn requests_carry_the_fields_chrome_sends_with_them() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    client.events.clear();
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo") }),
            on,
        )
        .await;
    let document = std::mem::take(&mut client.events);
    let sent = event_params(&document, "Network.requestWillBeSent").unwrap();
    assert_eq!(sent["request"]["mixedContentType"], "none");
    assert_eq!(sent["initiator"]["type"], "other");
    let received = event_params(&document, "Network.responseReceived").unwrap();
    assert_eq!(received["response"]["charset"], "utf-8");
    for flag in ["fromDiskCache", "fromServiceWorker", "fromPrefetchCache"] {
        assert_eq!(received["response"][flag], false, "{flag}");
    }
    let finished = event_params(&document, "Network.loadingFinished").unwrap();
    assert_eq!(finished["encodedBodyLength"], finished["encodedDataLength"]);

    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "fetch('/echo?f'); 1" }),
            on,
        )
        .await;
    let fetched = client
        .events_until("Network.loadingFinished", Duration::from_secs(5))
        .await;
    let sent = event_params(&fetched, "Network.requestWillBeSent").unwrap();
    assert_eq!(sent["initiator"]["type"], "script");
}

/// The `Network.loadingFailed` params that `events_until` collected, for
/// the request whose URL ends in `suffix`.
fn failed_params<'a>(events: &'a [Value], suffix: &str) -> &'a Value {
    let id = events
        .iter()
        .filter(|event| event["method"] == "Network.requestWillBeSent")
        .find(|event| {
            event["params"]["request"]["url"]
                .as_str()
                .is_some_and(|url| url.ends_with(suffix))
        })
        .and_then(|event| event["params"]["requestId"].as_str())
        .unwrap_or_else(|| panic!("no request for {suffix} in {events:?}"));
    events
        .iter()
        .find(|event| {
            event["method"] == "Network.loadingFailed" && event["params"]["requestId"] == id
        })
        .map(|event| &event["params"])
        .unwrap_or_else(|| panic!("{suffix} did not fail: {events:?}"))
}

#[tokio::test]
async fn a_request_that_fails_is_told_in_chromes_words() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/echo")).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    client
        .run(
            "Network.setBlockedURLs",
            json!({ "urls": ["*blockme*"] }),
            on,
        )
        .await;
    // A port that was open a moment ago and is closed now.
    let dead = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let expression = format!(
        "fetch('http://127.0.0.1:{dead}/refused').catch(() => 0); \
         fetch('/blockme').catch(() => 0); \
         fetch('http://localhost:{}/echo?cors').catch(() => 0); 1",
        addr.port()
    );
    client.events.clear();
    client
        .run("Runtime.evaluate", json!({ "expression": expression }), on)
        .await;
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.extend(
            client
                .events_until("Network.loadingFailed", Duration::from_secs(5))
                .await,
        );
    }
    let refused = failed_params(&seen, "/refused");
    assert_eq!(refused["errorText"], "net::ERR_CONNECTION_REFUSED");
    assert_eq!(refused["canceled"], false);
    assert!(refused.get("blockedReason").is_none());

    let blocked = failed_params(&seen, "/blockme");
    assert_eq!(blocked["errorText"], "");
    assert_eq!(blocked["blockedReason"], "inspector");

    let cors = failed_params(&seen, "/echo?cors");
    assert_eq!(cors["errorText"], "net::ERR_FAILED");
    assert_eq!(
        cors["corsErrorStatus"]["corsError"],
        "MissingAllowOriginHeader"
    );
    assert_eq!(cors["corsErrorStatus"]["failedParameter"], "");
}

#[tokio::test]
async fn a_navigation_that_fails_answers_with_a_net_error() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    let dead = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let reply = client
        .run(
            "Page.navigate",
            json!({ "url": format!("http://127.0.0.1:{dead}/") }),
            on,
        )
        .await;
    assert_eq!(reply["errorText"], "net::ERR_CONNECTION_REFUSED");
}

#[tokio::test]
async fn a_cors_refusal_by_a_wrong_header_names_the_origin_it_allowed() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/echo")).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    client.events.clear();
    // `localhost` against the page's `127.0.0.1`: another origin.
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": format!("fetch('http://localhost:{}/mismatch').catch(() => 0); 1", addr.port()) }),
            on,
        )
        .await;
    let seen = client
        .events_until("Network.loadingFailed", Duration::from_secs(5))
        .await;
    let cors = failed_params(&seen, "/mismatch");
    assert_eq!(cors["errorText"], "net::ERR_FAILED");
    assert_eq!(cors["corsErrorStatus"]["corsError"], "AllowOriginMismatch");
    assert_eq!(
        cors["corsErrorStatus"]["failedParameter"],
        "http://example.test"
    );
}

#[tokio::test]
async fn a_request_to_a_port_a_browser_refuses_never_leaves() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/echo")).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    client.events.clear();
    // Port 6000 is on Chrome's list. Nothing listens there, so a request
    // that was sent would be refused (`ERR_CONNECTION_REFUSED`); this one
    // is turned back before a socket is opened.
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "fetch('http://127.0.0.1:6000/unsafe').catch(() => 0); 1" }),
            on,
        )
        .await;
    let seen = client
        .events_until("Network.loadingFailed", Duration::from_secs(5))
        .await;
    let failed = failed_params(&seen, "/unsafe");
    assert_eq!(failed["errorText"], "net::ERR_UNSAFE_PORT");
    assert_eq!(failed["canceled"], false);
}

#[tokio::test]
async fn a_navigation_to_a_port_a_browser_refuses_answers_with_unsafe_port() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    let reply = client
        .run(
            "Page.navigate",
            json!({ "url": "http://127.0.0.1:6000/" }),
            on,
        )
        .await;
    assert_eq!(reply["errorText"], "net::ERR_UNSAFE_PORT");
}

/// A server that does what its path says once it has the request — the
/// same one `reset-probe.cjs` put to Chrome: `/empty` closes, `/reset`
/// resets, `/shortfin` and `/shortrst` send 3 of a declared 100 body
/// bytes and then close or reset, `/chunkcut` ends inside a chunked body,
/// and `/doc` is an ordinary page.
async fn misbehaving_site() -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = request.lines().next().unwrap_or("").to_string();
                let reset = |sock: &tokio::net::TcpStream| {
                    // SO_LINGER 0 turns the close into a reset. The deprecation is
                    // about blocking the thread on drop; these sockets are test-local.
                    #[allow(deprecated)]
                    let _ = sock.set_linger(Some(Duration::ZERO));
                };
                if line.contains("/doc") {
                    let body = "<!doctype html><body>x</body>";
                    let _ = sock
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\n\
                                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                                body.len()
                            )
                            .as_bytes(),
                        )
                        .await;
                } else if line.contains("/reset") {
                    reset(&sock);
                } else if line.contains("/shortfin") || line.contains("/shortrst") {
                    let _ = sock
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\
                              content-length: 100\r\n\r\nabc",
                        )
                        .await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if line.contains("/shortrst") {
                        reset(&sock);
                    }
                } else if line.contains("/chunkcut") {
                    let _ = sock
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\
                              transfer-encoding: chunked\r\n\r\n3\r\nabc\r\n",
                        )
                        .await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                // `/empty`: the socket is dropped unanswered.
            });
        }
    });
    port
}

#[tokio::test]
async fn a_connection_that_goes_wrong_is_told_as_chrome_tells_it() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let origin = format!("http://127.0.0.1:{}", misbehaving_site().await);
    let (mut client, _) = connect(axum::Router::new()).await;
    let session = page_at(&mut client, 1, &format!("{origin}/doc")).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    // Chrome 158's answer to each (`reset-probe.cjs`, `run-reset.log`).
    for (path, chrome) in [
        ("empty", "net::ERR_EMPTY_RESPONSE"),
        ("reset", "net::ERR_CONNECTION_RESET"),
        ("shortfin", "net::ERR_CONTENT_LENGTH_MISMATCH"),
        ("shortrst", "net::ERR_CONNECTION_RESET"),
        ("chunkcut", "net::ERR_INCOMPLETE_CHUNKED_ENCODING"),
    ] {
        client.events.clear();
        client
            .run(
                "Runtime.evaluate",
                json!({ "expression": format!("fetch('{origin}/{path}').then(r => r.text()).catch(() => 0); 1") }),
                on,
            )
            .await;
        let seen = client
            .events_until("Network.loadingFailed", Duration::from_secs(5))
            .await;
        let failed = failed_params(&seen, &format!("/{path}"));
        assert_eq!(failed["errorText"], chrome, "{path}");
    }
}

#[tokio::test]
async fn a_navigation_to_a_server_that_hangs_up_answers_as_chrome_does() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let origin = format!("http://127.0.0.1:{}", misbehaving_site().await);
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    for (path, chrome) in [
        ("empty", "net::ERR_EMPTY_RESPONSE"),
        ("reset", "net::ERR_CONNECTION_RESET"),
    ] {
        let reply = client
            .run(
                "Page.navigate",
                json!({ "url": format!("{origin}/{path}") }),
                on,
            )
            .await;
        assert_eq!(reply["errorText"], chrome, "{path}");
    }
}

#[tokio::test]
async fn a_navigation_that_fails_is_told_as_a_document_request_that_failed() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    client.run("Network.enable", json!({}), on).await;
    let dead = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let url = format!("http://127.0.0.1:{dead}/x");
    client.events.clear();
    let reply = client.run("Page.navigate", json!({ "url": url }), on).await;
    assert_eq!(reply["errorText"], "net::ERR_CONNECTION_REFUSED");
    // Chrome 158 (`failnav-probe.cjs`): the document's request, with the
    // loader the navigation answered, then its failure.
    let events = std::mem::take(&mut client.events);
    let sent = event_params(&events, "Network.requestWillBeSent")
        .unwrap_or_else(|| panic!("no requestWillBeSent in {events:?}"));
    assert_eq!(sent["type"], "Document");
    assert_eq!(sent["request"]["url"], url);
    assert_eq!(sent["loaderId"], reply["loaderId"]);
    assert_eq!(sent["frameId"], reply["frameId"]);
    let failed = event_params(&events, "Network.loadingFailed").expect("loadingFailed");
    assert_eq!(failed["type"], "Document");
    assert_eq!(failed["errorText"], "net::ERR_CONNECTION_REFUSED");
    assert_eq!(failed["canceled"], false);
    assert_eq!(failed["requestId"], sent["requestId"]);
    assert!(
        position_of(&events, "Network.requestWillBeSent")
            < position_of(&events, "Network.loadingFailed")
    );
    // No response came, so none is told.
    assert!(position_of(&events, "Network.responseReceived").is_none());
}

#[tokio::test]
async fn a_link_the_page_follows_by_itself_is_a_navigation_the_socket_sees() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let router = router
        .route(
            "/linkpage",
            axum::routing::get(|| async {
                axum::response::Html("<html><body id=one><a id=l href='/two'>go</a></body></html>")
            }),
        )
        .route(
            "/two",
            axum::routing::get(|| async {
                axum::response::Html("<html><body id=two>two</body></html>")
            }),
        );
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/linkpage")).await;
    let on = Some(session.as_str());
    client.run("Page.enable", json!({}), on).await;
    client.run("Network.enable", json!({}), on).await;
    client.events.clear();
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "document.getElementById('l').click(); 1" }),
            on,
        )
        .await;
    // Nothing is sent from here: the click queued a navigation, and the
    // session follows it. Chrome 158 (`clicknav-probe.cjs`): the document's
    // request, then the frame's navigation, both of the new loader.
    let seen = client
        .events_until("Page.frameNavigated", Duration::from_secs(8))
        .await;
    let sent = seen
        .iter()
        .find(|event| {
            event["method"] == "Network.requestWillBeSent"
                && event["params"]["type"] == "Document"
                && event["params"]["request"]["url"]
                    .as_str()
                    .is_some_and(|url| url.ends_with("/two"))
        })
        .unwrap_or_else(|| panic!("the followed link was not told: {seen:?}"));
    let navigated = seen
        .iter()
        .find(|event| event["method"] == "Page.frameNavigated")
        .expect("the navigation was told");
    assert_eq!(navigated["params"]["frame"]["url"], format!("{origin}/two"));
    assert_eq!(
        sent["params"]["loaderId"],
        navigated["params"]["frame"]["loaderId"]
    );
    assert!(
        position_of(&seen, "Network.requestWillBeSent") < position_of(&seen, "Page.frameNavigated")
    );
    // And the document really is the new one — not just `location`.
    assert_eq!(
        client
            .eval("document.body && document.body.id", &session)
            .await,
        "two"
    );
}

#[tokio::test]
async fn a_move_inside_the_document_is_told_as_one() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let router = router.route(
        "/spa",
        axum::routing::get(|| async { axum::response::Html("<html><body id=one>x</body></html>") }),
    );
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/spa")).await;
    let on = Some(session.as_str());
    client.run("Page.enable", json!({}), on).await;
    // Chrome 158 (`spa-probe.cjs`): loading starts, the frame says where it
    // is and how it got there, loading stops; no new document.
    for (expression, path, kind) in [
        (
            "history.pushState({a: 1}, '', '/pushed'); 1",
            "/pushed",
            "historyApi",
        ),
        (
            "history.replaceState({a: 2}, '', '/replaced'); 1",
            "/replaced",
            "historyApi",
        ),
        ("location.hash = '#frag'; 1", "/replaced#frag", "fragment"),
    ] {
        client.events.clear();
        client
            .run("Runtime.evaluate", json!({ "expression": expression }), on)
            .await;
        let seen = client
            .events_until("Page.navigatedWithinDocument", Duration::from_secs(5))
            .await;
        let moved = event_params(&seen, "Page.navigatedWithinDocument")
            .unwrap_or_else(|| panic!("{expression}: not told in {seen:?}"));
        assert_eq!(moved["url"], format!("{origin}{path}"), "{expression}");
        assert_eq!(moved["navigationType"], kind, "{expression}");
        assert_eq!(
            moved["frameId"], seen[0]["params"]["frameId"],
            "{expression}"
        );
        assert_eq!(
            seen[0]["method"], "Page.frameStartedLoading",
            "{expression}"
        );
        // The document is the same one: its body is still the first's.
        assert_eq!(
            client.eval("document.body.id", &session).await,
            "one",
            "{expression}"
        );
    }
}

#[tokio::test]
async fn a_session_without_page_is_told_no_moves() {
    let _guard = CDP_LOCK.lock().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, _site) = site();
    let router = router.route(
        "/spa",
        axum::routing::get(|| async { axum::response::Html("<html><body id=one>x</body></html>") }),
    );
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/spa")).await;
    let on = Some(session.as_str());
    client.events.clear();
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "history.pushState({}, '', '/quiet'); 1" }),
            on,
        )
        .await;
    // Let the idle pump see it, then look: nothing was said.
    let _ = client
        .events_until("Page.navigatedWithinDocument", Duration::from_secs(2))
        .await;
    let heard: Vec<String> = client
        .events
        .iter()
        .map(|event| event["method"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        heard.iter().all(|method| !method.starts_with("Page.")),
        "Page events without Page.enable: {heard:?}"
    );
}
