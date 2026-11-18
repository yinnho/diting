//! The first CDP slice, driven the way Playwright's connectOverCDP starts:
//! version endpoint, auto-attach, new page, evaluate, navigate, screenshot.
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::routes;

async fn http_get(addr: std::net::SocketAddr, path: &str) -> Value {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let body = text.split("\r\n\r\n").nth(1).expect("http body");
    serde_json::from_str(body.trim()).expect(body)
}

struct Client {
    ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    events: Vec<Value>,
}

impl Client {
    async fn call(&mut self, id: u64, method: &str, params: Value, session: Option<&str>) -> Value {
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            msg["sessionId"] = json!(session);
        }
        self.ws
            .send(Message::Text(msg.to_string()))
            .await
            .expect("send");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        loop {
            let frame = tokio::time::timeout_at(deadline, self.ws.next())
                .await
                .expect("cdp timed out")
                .expect("socket closed")
                .expect("ws frame");
            let text = match frame {
                Message::Text(text) => text,
                Message::Ping(payload) => {
                    self.ws.send(Message::Pong(payload)).await.unwrap();
                    continue;
                }
                other => panic!("unexpected frame {other:?}"),
            };
            let value: Value = serde_json::from_str(&text).expect(&text);
            if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(err) = value.get("error") {
                    panic!("{method} failed: {err}");
                }
                return value.get("result").cloned().unwrap_or(Value::Null);
            }
            self.events.push(value);
        }
    }
}

#[tokio::test]
async fn version_names_diting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, routes()).await.unwrap();
    });
    let body = http_get(addr, "/json/version/").await;
    let browser = body["Browser"].as_str().unwrap();
    assert!(
        browser.starts_with("diting/"),
        "Browser product is {browser}"
    );
    assert!(
        !browser.contains("Chrome/"),
        "version endpoint must not claim to be Chrome"
    );
    let ws = body["webSocketDebuggerUrl"].as_str().unwrap();
    assert!(ws.contains("/devtools/browser/"), "{ws}");
    let ua = body["User-Agent"].as_str().unwrap();
    assert!(ua.contains("diting/"), "{ua}");
    assert!(!ua.contains("Headless"), "{ua}");
}

#[tokio::test]
async fn connect_goto_eval_screenshot() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, routes()).await.unwrap();
    });
    let version = http_get(addr, "/json/version/").await;
    let ws_url = version["webSocketDebuggerUrl"]
        .as_str()
        .unwrap()
        .to_string();
    let (ws, _) = connect_async(&ws_url).await.expect("websocket");
    let mut client = Client {
        ws,
        events: Vec::new(),
    };

    let version = client.call(1, "Browser.getVersion", json!({}), None).await;
    assert!(version["product"].as_str().unwrap().starts_with("diting/"));

    client
        .call(
            2,
            "Browser.setDownloadBehavior",
            json!({ "behavior": "deny", "eventsEnabled": true }),
            None,
        )
        .await;
    client
        .call(
            3,
            "Target.setAutoAttach",
            json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }),
            None,
        )
        .await;
    let info = client
        .call(4, "Target.getTargetInfo", json!({}), None)
        .await;
    assert_eq!(info["targetInfo"]["type"], "browser");

    let created = client
        .call(
            5,
            "Target.createTarget",
            json!({ "url": "about:blank" }),
            None,
        )
        .await;
    let target_id = created["targetId"].as_str().unwrap().to_string();
    let session = client
        .events
        .iter()
        .find(|event| event["method"] == "Target.attachedToTarget")
        .and_then(|event| event["params"]["sessionId"].as_str())
        .expect("attachedToTarget")
        .to_string();
    let attached_target = client
        .events
        .iter()
        .find(|event| event["method"] == "Target.attachedToTarget")
        .unwrap();
    assert_eq!(
        attached_target["params"]["targetInfo"]["targetId"],
        target_id
    );
    assert_eq!(
        attached_target["params"]["targetInfo"]["browserContextId"],
        "default"
    );
    assert_eq!(attached_target["params"]["targetInfo"]["type"], "page");

    let tree = client
        .call(6, "Page.getFrameTree", json!({}), Some(&session))
        .await;
    assert_eq!(tree["frameTree"]["frame"]["url"], "about:blank");

    client
        .call(7, "Runtime.enable", json!({}), Some(&session))
        .await;
    client
        .call(8, "Log.enable", json!({}), Some(&session))
        .await;
    client
        .call(9, "Network.enable", json!({}), Some(&session))
        .await;
    client
        .call(10, "Page.enable", json!({}), Some(&session))
        .await;
    client
        .call(
            11,
            "Runtime.runIfWaitingForDebugger",
            json!({}),
            Some(&session),
        )
        .await;

    let sum = client
        .call(
            12,
            "Runtime.evaluate",
            json!({ "expression": "1+1", "returnByValue": true }),
            Some(&session),
        )
        .await;
    assert_eq!(sum["result"]["value"], 2);

    let handle = client
        .call(
            13,
            "Runtime.evaluate",
            json!({ "expression": "({n:41})", "returnByValue": false }),
            Some(&session),
        )
        .await;
    let object_id = handle["result"]["objectId"]
        .as_str()
        .expect("object id")
        .to_string();
    let called = client
        .call(
            14,
            "Runtime.callFunctionOn",
            json!({
                "functionDeclaration": "(o) => o.n + 1",
                "objectId": object_id,
                "arguments": [{ "objectId": object_id }],
                "returnByValue": true,
                "awaitPromise": true,
            }),
            Some(&session),
        )
        .await;
    assert_eq!(called["result"]["value"], 42, "{called}");

    let gone = client.events.len();
    client
        .call(
            15,
            "Page.navigate",
            json!({ "url": "data:text/html,<title>Hi</title><p>Hello CDP</p>" }),
            Some(&session),
        )
        .await;
    assert!(
        client.events[gone..]
            .iter()
            .any(|event| event["method"] == "Page.lifecycleEvent"
                && event["params"]["name"] == "load"),
        "navigate did not emit load: {:?}",
        &client.events[gone..]
    );
    let text = client
        .call(
            16,
            "Runtime.evaluate",
            json!({
                "expression": "document.body.innerText",
                "returnByValue": true,
                "awaitPromise": true,
            }),
            Some(&session),
        )
        .await;
    let body = text["result"]["value"].as_str().unwrap_or("");
    assert!(body.contains("Hello CDP"), "{text}");

    client
        .call(
            30,
            "Page.navigate",
            json!({
                "url": "data:text/html,<button id=b style=\"position:fixed;left:8px;top:8px;width:100px;height:40px\" onclick=\"this.dataset.hit='1'\">Go</button><input id=t>"
            }),
            Some(&session),
        )
        .await;
    for (id, kind) in [
        (31u64, "mouseMoved"),
        (32, "mousePressed"),
        (33, "mouseReleased"),
    ] {
        let mut params =
            json!({ "type": kind, "x": 20, "y": 20, "button": "left", "clickCount": 1 });
        if kind == "mousePressed" {
            params["buttons"] = json!(1);
        }
        client
            .call(id, "Input.dispatchMouseEvent", params, Some(&session))
            .await;
    }
    let hit = client
        .call(
            34,
            "Runtime.evaluate",
            json!({
                "expression": "document.getElementById('b').dataset.hit",
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    assert_eq!(hit["result"]["value"], "1", "{hit}");

    client
        .call(
            35,
            "Runtime.evaluate",
            json!({
                "expression": "var el = document.getElementById('t'); el.focus(); el.value = 'old'; el.setSelectionRange(0, 3); 'ok'",
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    client
        .call(
            36,
            "Input.insertText",
            json!({ "text": "new" }),
            Some(&session),
        )
        .await;
    let filled = client
        .call(
            37,
            "Runtime.evaluate",
            json!({
                "expression": "document.getElementById('t').value",
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    assert_eq!(filled["result"]["value"], "new", "{filled}");

    let frame_id = tree["frameTree"]["frame"]["id"]
        .as_str()
        .expect("frame id")
        .to_string();
    let wrote_at = client.events.len();
    let wrote = client
        .call(
            38,
            "Runtime.evaluate",
            json!({
                "expression": "document.open(); console.debug('--playwright--set--content--t--'); document.write('<p id=\"m\">SETCONTENT_OK</p>'); document.close(); 'wrote'",
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    assert_eq!(wrote["result"]["value"], "wrote", "{wrote}");
    assert!(wrote.get("__ditingSide").is_none(), "{wrote}");
    let batch = &client.events[wrote_at..];
    let signal = batch
        .iter()
        .position(|event| {
            event["method"] == "Runtime.consoleAPICalled"
                && event["params"]["type"] == "debug"
                && event["params"]["args"][0]["value"] == "--playwright--set--content--t--"
                && event["params"]["executionContextId"].as_i64().unwrap_or(0) > 0
        })
        .expect("setContent console tag");
    let load_at = batch
        .iter()
        .position(|event| {
            event["method"] == "Page.lifecycleEvent"
                && event["params"]["name"] == "load"
                && event["params"]["frameId"] == frame_id
        })
        .expect("setContent load");
    assert!(signal < load_at, "console tag must precede load: {batch:?}");
    assert!(
        batch
            .iter()
            .any(|event| event["method"] == "Page.lifecycleEvent"
                && event["params"]["name"] == "DOMContentLoaded"),
        "setContent did not emit DOMContentLoaded: {batch:?}"
    );
    assert!(
        batch
            .iter()
            .all(|event| event["method"] != "Page.frameNavigated"),
        "setContent must not replace the execution context"
    );
    let marker = client
        .call(
            39,
            "Runtime.evaluate",
            json!({
                "expression": "document.getElementById('m').textContent",
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    assert_eq!(marker["result"]["value"], "SETCONTENT_OK", "{marker}");

    let empty_at = client.events.len();
    client
        .call(
            40,
            "Runtime.evaluate",
            json!({
                "expression": "document.open(); console.debug('--playwright--set--content--empty--'); document.write(''); document.close();",
                "returnByValue": true,
            }),
            Some(&session),
        )
        .await;
    let empty = &client.events[empty_at..];
    let empty_tag = empty.iter().position(|event| {
        event["method"] == "Runtime.consoleAPICalled"
            && event["params"]["args"][0]["value"] == "--playwright--set--content--empty--"
    });
    let empty_load = empty.iter().position(|event| {
        event["method"] == "Page.lifecycleEvent" && event["params"]["name"] == "load"
    });
    assert!(
        empty_tag
            .zip(empty_load)
            .is_some_and(|(tag, load)| tag < load),
        "empty setContent still needs the tag before load: {empty:?}"
    );

    #[cfg(feature = "screenshot")]
    {
        let metrics = client
            .call(17, "Page.getLayoutMetrics", json!({}), Some(&session))
            .await;
        assert!(metrics["visualViewport"]["clientWidth"].as_f64().unwrap() > 0.0);
        let shot = client
            .call(
                18,
                "Page.captureScreenshot",
                json!({ "format": "png" }),
                Some(&session),
            )
            .await;
        assert!(shot["data"].as_str().unwrap().len() > 100, "png payload");
    }

    client
        .call(
            19,
            "Target.closeTarget",
            json!({ "targetId": target_id }),
            None,
        )
        .await;
    let listed = http_get(addr, "/json/list").await;
    assert!(listed.as_array().unwrap().is_empty(), "{listed}");
}
