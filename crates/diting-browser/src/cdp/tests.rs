//! The CDP bridge, driven the way Playwright's connectOverCDP starts. Only
//! what the tests share lives here — the process-wide hub lock, a socket
//! that keeps what it hears, a page opened and attached — and each
//! domain's tests are in a file of their own: the frame and the error
//! codes (`protocol`), the session model (`session`), `Runtime`
//! (`runtime`), the `DOM` domain (`dom`), emulation and document-start
//! scripts (`emulation`), navigation, load events and `Input` (`page`),
//! `Network` (`network`), and the screenshot.
#[cfg(test)]
mod dom;
#[cfg(test)]
mod emulation;
#[cfg(test)]
mod network;
#[cfg(test)]
mod page;
#[cfg(test)]
mod protocol;
#[cfg(test)]
mod runtime;
#[cfg(test)]
#[cfg(feature = "screenshot")]
mod screenshot;
#[cfg(test)]
mod session;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::routes;

/// A session id this bridge never minted.
const NO_SUCH_ID: &str = "00000000000000000000000000000000";

/// A real handle's realm, with a number that realm never minted.
fn never_minted(id: &str) -> String {
    id.replace("\"id\":", "\"id\":100000")
}

/// The page's main world, as `Runtime.enable` last announced it.
fn main_context(client: &Client) -> i64 {
    client
        .events
        .iter()
        .rev()
        .find(|event| event["method"] == "Runtime.executionContextCreated")
        .and_then(|event| event["params"]["context"]["id"].as_i64())
        .expect("an execution context was announced")
}

/// The handle `expression` gives in the page.
async fn handle_of(client: &mut Client, session: &str, expression: &str) -> String {
    let reply = client
        .run(
            "Runtime.evaluate",
            json!({ "expression": expression }),
            Some(session),
        )
        .await;
    reply["result"]["objectId"]
        .as_str()
        .unwrap_or_else(|| panic!("{expression} gave no handle: {reply}"))
        .to_string()
}

/// `GET path`, answered with the status and the body.
async fn http_raw(addr: std::net::SocketAddr, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|status| status.parse().ok())
        .expect("http status line");
    let body = text
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();
    (status, body)
}

async fn http_get(addr: std::net::SocketAddr, path: &str) -> Value {
    let (_, body) = http_raw(addr, path).await;
    serde_json::from_str(&body).expect(&body)
}

struct Client {
    ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    events: Vec<Value>,
    /// The last id [`Client::ask`] handed out. Starts clear of the ids the
    /// tests write out by hand.
    next: u64,
}

impl Client {
    /// Send one command and wait for its reply: its result, or the code
    /// and message of its error. Every other frame on the way — an event —
    /// is kept in `events`.
    async fn send(
        &mut self,
        id: u64,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, (i64, String)> {
        let frame = self.exchange(id, method, params, session).await;
        match frame.get("error") {
            Some(err) => Err((
                err["code"].as_i64().unwrap_or(0),
                err["message"].as_str().unwrap_or("").to_string(),
            )),
            None => Ok(frame.get("result").cloned().unwrap_or(Value::Null)),
        }
    }

    /// [`Client::send`], with the reply frame whole — error or not. What
    /// it says of the session it answers is half of what is being asked.
    async fn exchange(
        &mut self,
        id: u64,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Value {
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            msg["sessionId"] = json!(session);
        }
        self.ws
            .send(Message::Text(msg.to_string()))
            .await
            .expect("send");
        let frame = self
            .next_frame(Duration::from_secs(60))
            .await
            .expect("the socket closed, or cdp timed out");
        self.answered(id, frame).await
    }

    /// The frame that answers `id`; anything else on the way is kept.
    async fn answered(&mut self, id: u64, mut frame: Value) -> Value {
        while frame.get("id").and_then(|v| v.as_u64()) != Some(id) {
            self.events.push(frame);
            frame = self
                .next_frame(Duration::from_secs(60))
                .await
                .expect("the socket closed, or cdp timed out");
        }
        frame
    }

    /// The next frame on the socket, pings answered; `None` when the
    /// socket closed or nothing came in time.
    async fn next_frame(&mut self, within: Duration) -> Option<Value> {
        let deadline = tokio::time::Instant::now() + within;
        loop {
            let frame = match tokio::time::timeout_at(deadline, self.ws.next()).await {
                Ok(Some(Ok(frame))) => frame,
                _ => return None,
            };
            match frame {
                Message::Text(text) => return Some(serde_json::from_str(&text).expect(&text)),
                Message::Ping(payload) => {
                    self.ws.send(Message::Pong(payload)).await.unwrap();
                }
                Message::Close(_) => return None,
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    /// The next event: the next frame that is not a reply.
    async fn next_event(&mut self, within: Duration) -> Option<Value> {
        loop {
            let frame = self.next_frame(within).await?;
            if frame.get("id").is_none() {
                return Some(frame);
            }
        }
    }

    /// Every event on the socket up to and including the next one called
    /// `method`; all of them if none came in time.
    async fn events_until(&mut self, method: &str, within: Duration) -> Vec<Value> {
        let end = self
            .events
            .iter()
            .position(|event| event["method"] == method)
            .map(|at| at + 1);
        match end {
            Some(end) => self.events.drain(..end).collect(),
            None => loop {
                let Some(event) = self.next_event(within).await else {
                    break std::mem::take(&mut self.events);
                };
                let last = event["method"] == method;
                self.events.push(event);
                if last {
                    break std::mem::take(&mut self.events);
                }
            },
        }
    }

    /// [`Client::send`], panicking on a protocol error.
    async fn call(&mut self, id: u64, method: &str, params: Value, session: Option<&str>) -> Value {
        self.send(id, method, params, session)
            .await
            .unwrap_or_else(|(code, message)| panic!("{method} failed ({code}): {message}"))
    }

    /// [`Client::send`] with the next id.
    async fn ask(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, (i64, String)> {
        self.next += 1;
        let id = self.next;
        self.send(id, method, params, session).await
    }

    async fn run(&mut self, method: &str, params: Value, session: Option<&str>) -> Value {
        self.ask(method, params, session)
            .await
            .unwrap_or_else(|(code, message)| panic!("{method} failed ({code}): {message}"))
    }

    /// The message of a command refused with `-32000`.
    async fn refusal(&mut self, method: &str, params: Value, session: Option<&str>) -> String {
        self.error(-32000, method, params, session).await
    }

    /// The message of a command whose parameters will not be read: `-32602`.
    async fn invalid(&mut self, method: &str, params: Value, session: Option<&str>) -> String {
        self.error(-32602, method, params, session).await
    }

    /// The message of a command refused with `code`. An empty ok would be
    /// the one wrong answer.
    async fn error(
        &mut self,
        code: i64,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> String {
        match self.ask(method, params, session).await {
            Err((got, message)) if got == code => message,
            Ok(result) => panic!("{method} was answered: {result}"),
            Err((got, message)) => panic!("{method}: code {got}, not {code}: {message}"),
        }
    }

    /// Run a command; it, and the events that came before its reply.
    async fn with_events(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> (Value, Vec<Value>) {
        let before = self.events.len();
        let reply = self.run(method, params, session).await;
        (reply, self.events.split_off(before))
    }

    /// A session that is no more: every command on it is `-32001`.
    async fn assert_gone(&mut self, session: &str) {
        let asked = self
            .ask(
                "Runtime.evaluate",
                json!({ "expression": "1" }),
                Some(session),
            )
            .await;
        assert_eq!(
            asked,
            Err((-32001, "Session with given id not found.".to_string())),
            "{session}"
        );
    }

    /// Evaluate `expression` in the page; the value it produced.
    async fn eval(&mut self, expression: &str, session: &str) -> Value {
        self.run(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
            Some(session),
        )
        .await["result"]["value"]
            .clone()
    }

    /// Send `text` as it stands and take the frame that answers it: the
    /// next one that is no event. Those are kept.
    async fn raw(&mut self, text: &str) -> Option<Value> {
        self.ws
            .send(Message::Text(text.to_string()))
            .await
            .expect("send");
        loop {
            let frame = self.next_frame(Duration::from_secs(60)).await?;
            if frame.get("method").is_none() {
                return Some(frame);
            }
            self.events.push(frame);
        }
    }
}

/// A browser socket that also serves `site`.
async fn connect(site: axum::Router) -> (Client, std::net::SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, routes().merge(site)).await.unwrap();
    });
    (
        open_socket(&format!(
            "ws://{addr}/devtools/browser/{}",
            super::browser_id()
        ))
        .await,
        addr,
    )
}

/// A second browser socket on the same bridge.
async fn another(addr: std::net::SocketAddr) -> Client {
    let ws_url = http_get(addr, "/json/version/").await["webSocketDebuggerUrl"]
        .as_str()
        .unwrap()
        .to_string();
    open_socket(&ws_url).await
}

/// The page's own websocket: its root level is that page.
async fn page_socket(addr: std::net::SocketAddr, target: &str) -> Client {
    open_socket(&format!("ws://{addr}/devtools/page/{target}")).await
}

async fn open_socket(ws_url: &str) -> Client {
    let (ws, _) = connect_async(ws_url).await.expect("websocket");
    Client {
        ws,
        events: Vec::new(),
        next: 1000,
    }
}

/// This client's own page, attached by auto-attach and switched on for
/// nothing: its target and its session.
async fn owned_page(client: &mut Client, n: u64) -> (String, String) {
    client
        .call(
            n,
            "Target.setAutoAttach",
            json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }),
            None,
        )
        .await;
    let created = client
        .call(
            n + 1,
            "Target.createTarget",
            json!({ "url": "about:blank" }),
            None,
        )
        .await;
    let target_id = created["targetId"].as_str().unwrap().to_string();
    let session = client
        .events
        .iter()
        .rev()
        .find(|event| {
            event["method"] == "Target.attachedToTarget"
                && event["params"]["targetInfo"]["targetId"] == target_id
        })
        .and_then(|event| event["params"]["sessionId"].as_str())
        .expect("attachedToTarget")
        .to_string();
    (target_id, session)
}

/// [`owned_page`], with `Runtime.enable` run on the session.
async fn open_owned_page(client: &mut Client, n: u64) -> (String, String) {
    let (target_id, session) = owned_page(client, n).await;
    client
        .call(n + 2, "Runtime.enable", json!({}), Some(&session))
        .await;
    (target_id, session)
}

/// This client's own page, navigated to `url`: its CDP session.
async fn page_at(client: &mut Client, n: u64, url: &str) -> String {
    let (_, session) = open_owned_page(client, n).await;
    client
        .call(
            n + 3,
            "Page.navigate",
            json!({ "url": url }),
            Some(&session),
        )
        .await;
    session
}

/// The bridge with one page of its own at `url`: the socket and the
/// page's session. Every test's bridge is its own hub — `routes()` builds
/// one per instance — so tests run in parallel and nothing here serializes
/// them. What truly shares a process is the private-network env knob, and
/// it has its own lock (`net_env_guard`).
async fn bridge(url: &str) -> (Client, String) {
    let (mut client, _) = connect(axum::Router::new()).await;
    let session = page_at(&mut client, 1, url).await;
    (client, session)
}
