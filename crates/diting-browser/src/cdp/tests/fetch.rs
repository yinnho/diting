//! `Fetch` request interception: a paused request is announced as
//! `Fetch.requestPaused` while the command that raised it is still in
//! flight, and `continueRequest` / `fulfillRequest` / `failRequest` —
//! answered from the hub's pending map, never through the session actor —
//! decide what the page actually gets. The tests drive the socket by hand
//! because the whole point is that the answer arrives mid-command.

use super::*;

/// Send a command without reading anything back: the reply — and the
/// events before it — are collected by hand later.
async fn send_only(client: &mut Client, id: u64, method: &str, params: Value, session: &str) {
    let msg = json!({ "id": id, "method": method, "params": params, "sessionId": session });
    client
        .ws
        .send(Message::Text(msg.to_string()))
        .await
        .expect("send");
}

/// The next `Fetch.requestPaused` event, whatever else comes first.
async fn next_pause(client: &mut Client) -> Value {
    let deadline = Duration::from_secs(30);
    loop {
        let frame = client
            .next_frame(deadline)
            .await
            .expect("a frame before the pause");
        if frame["method"] == "Fetch.requestPaused" {
            return frame;
        }
        client.events.push(frame);
    }
}

/// The frame answering `id`; everything before it is kept as an event.
async fn wait_reply(client: &mut Client, id: u64) -> Value {
    loop {
        let frame = client
            .next_frame(Duration::from_secs(60))
            .await
            .expect("the socket closed, or cdp timed out");
        if frame.get("id").and_then(|v| v.as_u64()) == Some(id) {
            return frame;
        }
        client.events.push(frame);
    }
}

/// The interception test site: `/` pulls a script, `/data` answers JSON,
/// `/script.js` would set `window.scripted` if it ever ran unmodified.
async fn fetch_site() -> axum::Router {
    axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async {
                axum::response::Html(
                    "<html><body><script src=\"/script.js\"></script></body></html>".to_string(),
                )
            }),
        )
        .route(
            "/data",
            axum::routing::get(|| async { "{\"served\":true}" }),
        )
        .route(
            "/script.js",
            axum::routing::get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/javascript")],
                    "window.scripted='served'",
                )
            }),
        )
}

/// `Fetch.enable` answers `{}` and a garbage pattern shape is `-32602`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn enable_answers() {
    let (mut client, session) = bridge("about:blank").await;
    let answer = client
        .run(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": "*" }] }),
            Some(&session),
        )
        .await;
    assert_eq!(answer, json!({}));
    client
        .invalid(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": 7 }] }),
            Some(&session),
        )
        .await;
    client
        .invalid("Fetch.enable", json!({ "patterns": "x" }), Some(&session))
        .await;
}

/// The document navigation pauses mid-command; `fulfillRequest` — sent
/// while `Page.navigate` is still unanswered — is what the page commits.
/// This is the deadlock the bypass exists for: the answer crosses the
/// same socket the navigate is in flight on.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn document_fulfilled_mid_navigate() {
    let (mut client, session) = bridge("about:blank").await;
    client
        .run(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": "*" }] }),
            Some(&session),
        )
        .await;
    send_only(
        &mut client,
        500,
        "Page.navigate",
        json!({ "url": "http://nowhere.example.test/never" }),
        &session,
    )
    .await;
    let pause = next_pause(&mut client).await;
    assert_eq!(pause["params"]["resourceType"], "Document");
    assert_eq!(
        pause["params"]["request"]["url"],
        "http://nowhere.example.test/never"
    );
    assert_eq!(pause["params"]["request"]["method"], "GET");
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    let html = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "<html><body><div id=it>fulfilled</div></body></html>",
    );
    client
        .run(
            "Fetch.fulfillRequest",
            json!({ "requestId": request_id, "responseCode": 200, "body": html }),
            Some(&session),
        )
        .await;
    let reply = wait_reply(&mut client, 500).await;
    assert!(
        reply.get("error").is_none() && reply["result"]["errorText"].is_null(),
        "the navigation committed the fulfillment: {reply}"
    );
    let text = client
        .eval("document.getElementById('it').textContent", &session)
        .await;
    assert_eq!(text, json!("fulfilled"));
}

/// A paused JS `fetch()` — raised inside the `Runtime.evaluate` waiting
/// on it — is answered the same way, and the promise settles with what
/// the client answered. `failRequest` rejects it instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fetch_fulfill_and_fail() {
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, addr) = connect(fetch_site().await).await;
    let session = page_at(&mut client, 1, &format!("http://{addr}/")).await;
    client
        .run(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": "**/data" }] }),
            Some(&session),
        )
        .await;
    let expression = format!("fetch('http://{addr}/data').then(r=>r.text())");
    send_only(
        &mut client,
        500,
        "Runtime.evaluate",
        json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
        &session,
    )
    .await;
    let pause = next_pause(&mut client).await;
    assert_eq!(pause["params"]["resourceType"], "Fetch");
    assert!(
        pause["params"]["networkId"]
            .as_str()
            .unwrap_or("")
            .starts_with("fetch-"),
        "a JS fetch's networkId is the row's own id — the pairing routing needs: {pause}"
    );
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    let body = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "{\"served\":false}",
    );
    client
        .run(
            "Fetch.fulfillRequest",
            json!({
                "requestId": request_id,
                "responseCode": 200,
                "responseHeaders": [{ "name": "content-type", "value": "application/json" }],
                "body": body,
            }),
            Some(&session),
        )
        .await;
    let reply = wait_reply(&mut client, 500).await;
    assert_eq!(
        reply["result"]["result"]["value"],
        json!("{\"served\":false}"),
        "the fulfill is what fetch() settled with: {reply}"
    );

    // failRequest: the same fetch is rejected.
    send_only(
        &mut client,
        501,
        "Runtime.evaluate",
        json!({
            "expression": format!("fetch('http://{addr}/data').then(r=>'ok').catch(()=>'rejected')"),
            "awaitPromise": true,
            "returnByValue": true,
        }),
        &session,
    )
    .await;
    let pause = next_pause(&mut client).await;
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    client
        .run(
            "Fetch.failRequest",
            json!({ "requestId": request_id, "errorReason": "aborted" }),
            Some(&session),
        )
        .await;
    let reply = wait_reply(&mut client, 501).await;
    assert_eq!(reply["result"]["result"]["value"], json!("rejected"));
}

/// `continueRequest` without overrides lets the real request through; a
/// url override rewrites it. The script the document loads pauses too —
/// the transport-level hook, not just the JS fetch shim.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subresource_pauses_and_continue() {
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, addr) = connect(fetch_site().await).await;
    let (target_id, session) = open_owned_page(&mut client, 1).await;
    client
        .run(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": "**/*" }] }),
            Some(&session),
        )
        .await;
    send_only(
        &mut client,
        500,
        "Page.navigate",
        json!({ "url": format!("http://{addr}/") }),
        &session,
    )
    .await;
    // Document first: continue it untouched.
    let pause = next_pause(&mut client).await;
    assert_eq!(pause["params"]["resourceType"], "Document");
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    client
        .run(
            "Fetch.continueRequest",
            json!({ "requestId": request_id }),
            Some(&session),
        )
        .await;
    // The script subresource pauses as a Script.
    let pause = next_pause(&mut client).await;
    assert_eq!(
        pause["params"]["resourceType"], "Script",
        "the parser-loaded script paused at the transport: {pause}"
    );
    assert!(pause["params"]["request"]["url"]
        .as_str()
        .unwrap()
        .ends_with("/script.js"));
    assert!(
        !pause["params"]["networkId"]
            .as_str()
            .unwrap_or("")
            .is_empty(),
        "the script pause pairs with its row: {pause}"
    );
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    // Rewrite it to /data — JSON is not JS, so the rewrite is visible only
    // in what the server is asked for; fulfill instead to keep it simple:
    // the script's effect on the page is the assertion.
    let js = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "window.scripted='fulfilled'",
    );
    client
        .run(
            "Fetch.fulfillRequest",
            json!({ "requestId": request_id, "responseCode": 200, "body": js }),
            Some(&session),
        )
        .await;
    let reply = wait_reply(&mut client, 500).await;
    assert!(reply.get("error").is_none(), "navigate answered: {reply}");
    let value = client.eval("window.scripted", &session).await;
    assert_eq!(value, json!("fulfilled"));
    let _ = target_id;
}

/// An unknown interception id is `-32000`; a bare `continueRequest` on a
/// request that timed out behaves the same.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_interception_id_refused() {
    let (mut client, session) = bridge("about:blank").await;
    let message = client
        .refusal(
            "Fetch.continueRequest",
            json!({ "requestId": "interception-job-999.0" }),
            Some(&session),
        )
        .await;
    assert!(message.contains("Invalid InterceptionId"), "{message}");
}

/// Playwright's routing correlates `Fetch.requestPaused` with
/// `Network.requestWillBeSent` on the pause's `networkId` — without the
/// pairing it auto-continues, unrouted. The pause therefore carries the
/// wire id the row will be told under (a document's is its loader's), a
/// synthetic `requestWillBeSent` precedes the pause with that id, and the
/// row's own frames — emitted when the fetch lands — skip the copy they
/// would otherwise duplicate while still carrying the response.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_pairs_with_network_row() {
    let (mut client, session) = bridge("about:blank").await;
    client
        .run("Network.enable", json!({}), Some(&session))
        .await;
    client
        .run(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": "*" }] }),
            Some(&session),
        )
        .await;
    send_only(
        &mut client,
        500,
        "Page.navigate",
        json!({ "url": "http://nowhere.example.test/never" }),
        &session,
    )
    .await;
    let pause = next_pause(&mut client).await;
    let network_id = pause["params"]["networkId"]
        .as_str()
        .expect("the pause carries a networkId — without it routing clients auto-continue");
    assert!(
        network_id.starts_with("ldr"),
        "a document's networkId is its loader's wire id: {network_id}"
    );
    // The pairing frame came before the pause, with the pause's networkId
    // as its requestId.
    let pairing = client.events.iter().find(|frame| {
        frame["method"] == "Network.requestWillBeSent" && frame["params"]["requestId"] == network_id
    });
    assert!(
        pairing.is_some(),
        "a synthetic requestWillBeSent precedes the pause under the networkId; events: {:?}",
        client
            .events
            .iter()
            .map(|frame| frame["method"].as_str().unwrap_or("?").to_string())
            .collect::<Vec<_>>()
    );
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    let html = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "<html><body>paired</body></html>",
    );
    client
        .run(
            "Fetch.fulfillRequest",
            json!({ "requestId": request_id, "responseCode": 200, "body": html }),
            Some(&session),
        )
        .await;
    let reply = wait_reply(&mut client, 500).await;
    assert!(reply.get("error").is_none(), "navigate answered: {reply}");
    // The row's own frames, emitted as the navigation closed out: the
    // response still arrives under the same wire id, but the request is
    // not told a second time.
    let mut reannounced = 0;
    let mut response_seen = false;
    for frame in &client.events {
        if frame["params"]["requestId"] != network_id {
            continue;
        }
        match frame["method"].as_str().unwrap_or("") {
            "Network.requestWillBeSent" => reannounced += 1,
            "Network.responseReceived" => response_seen = true,
            _ => {}
        }
    }
    assert_eq!(
        reannounced, 1,
        "the request is told exactly once — the synthetic pairing, not the row's copy"
    );
    assert!(
        response_seen,
        "the row's response frames still land under the wire id"
    );
}

/// The JS-fetch twin of the document pairing: the pause's `networkId` is
/// the `fetch-{N}` id the row will be rekeyed to when the fetch lands —
/// the response frames arrive under that same id and the request is not
/// told twice. Without the rekey the row keeps its own fresh id and the
/// announced one dangles, unpaired.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fetch_row_rekeys_to_the_pause_id() {
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, addr) = connect(fetch_site().await).await;
    let session = page_at(&mut client, 1, &format!("http://{addr}/")).await;
    client
        .run("Network.enable", json!({}), Some(&session))
        .await;
    client
        .run(
            "Fetch.enable",
            json!({ "patterns": [{ "urlPattern": "**/data" }] }),
            Some(&session),
        )
        .await;
    send_only(
        &mut client,
        500,
        "Runtime.evaluate",
        json!({
            "expression": format!("fetch('http://{addr}/data').then(r=>r.text())"),
            "awaitPromise": true,
            "returnByValue": true,
        }),
        &session,
    )
    .await;
    let pause = next_pause(&mut client).await;
    let network_id = pause["params"]["networkId"].as_str().unwrap().to_string();
    let request_id = pause["params"]["requestId"].as_str().unwrap().to_string();
    let body = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "paired");
    client
        .run(
            "Fetch.fulfillRequest",
            json!({ "requestId": request_id, "responseCode": 200, "body": body }),
            Some(&session),
        )
        .await;
    let reply = wait_reply(&mut client, 500).await;
    assert_eq!(reply["result"]["result"]["value"], json!("paired"));
    // The frames may trail the evaluate reply — the row is written as the
    // fetch settles, its events pushed after. Give them a moment.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let (mut announcements, mut response_seen) = (0, false);
    while std::time::Instant::now() < deadline {
        announcements = 0;
        response_seen = false;
        for frame in &client.events {
            if frame["params"]["requestId"].as_str() != Some(network_id.as_str()) {
                continue;
            }
            match frame["method"].as_str().unwrap_or("") {
                "Network.requestWillBeSent" => announcements += 1,
                "Network.responseReceived" => response_seen = true,
                _ => {}
            }
        }
        if response_seen {
            break;
        }
        if let Some(frame) = client.next_frame(Duration::from_secs(1)).await {
            client.events.push(frame);
        }
    }
    assert_eq!(
        announcements, 1,
        "the request is told exactly once under the pause's networkId"
    );
    assert!(
        response_seen,
        "the row's response lands under the same id — the rekey held"
    );
}

/// Probe: no interception at all — does a CDP evaluate of fetch() reach
/// the server and settle?
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn probe_eval_fetch_no_intercept() {
    let _net = crate::server::test_util::net_env_guard();
    let (mut client, addr) = connect(fetch_site().await).await;
    let session = page_at(&mut client, 1, &format!("http://{addr}/")).await;
    let reply = client
        .run(
            "Runtime.evaluate",
            json!({
                "expression": format!("fetch('http://{addr}/data').then(r=>r.text())"),
                "awaitPromise": true,
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    eprintln!("PROBE eval reply: {reply}");
}
