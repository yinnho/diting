//! Chrome DevTools Protocol bridge.
//!
//! `/json/version` names this engine `diting`; the page's own user agent
//! stays the Chrome pin it was built with.
//!
//! Each browser socket owns the pages it opens. A second connection does
//! not see those pages, and disconnecting one does not close the other's.
//! The hub lock is never held across a session await. The session thread
//! does not know about the hub, but an HTTP handler can hold `SESSIONS`
//! while another task wants the hub.
//!
//! A socket's root level is the browser. `Target.attachToTarget` and
//! `Target.attachToBrowserTarget` mint a session on it, a new id for each
//! call, and a page websocket (`/devtools/page/<id>`) is a socket whose
//! root level is that page. A session is its socket's: on another socket
//! its id names nothing. Reporting domains are switched on per session:
//! navigation and load events go to the sessions that ran `Page.enable`,
//! world announcements and console calls to those that ran `Runtime.enable`,
//! on whichever socket they live ([`to_sessions`]). A page closed — by
//! command, or because the socket that owned it left — ends every session
//! on it, each with `Inspector.detached`.
//!
//! A command lands in one of four places: a handler here — the `Target` and
//! `Browser` domains and navigation in [`target`], screenshots in
//! [`screenshot`] — on a build with that feature; [`PAGE_CHANNEL`], the
//! session thread running it on the
//! page; [`STATELESS`], an empty ok because the engine has no state for it
//! to flip; or `-32601`, the method named. A frame is read the way Chrome
//! reads one ([`envelope`]), and so is every parameter ([`params`]): one
//! Chrome would not read is `-32602`. A command that arrives and cannot be
//! applied as sent is `-32000` with the reason — an empty ok would be a lie
//! the client cannot detect. A `sessionId` this bridge did not mint is
//! `-32001`.

pub(crate) mod emulation;
mod envelope;
mod network_events;
mod objects;
mod page_side;
mod params;
mod push;
pub(crate) mod runtime;
#[cfg(feature = "screenshot")]
mod screenshot;
mod target;

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::session::{send_command, CallOnRequest, SessionCommand};

use params::{boolean, invalid, kinds, optional_string, Kind};

#[cfg(test)]
mod tests;

const CONTEXT_ID: &str = "default";
/// What `Page.captureScreenshot` answers on a build without the feature.
#[cfg(not(feature = "screenshot"))]
const NEEDS_SCREENSHOT: &str =
    "Page.captureScreenshot needs the `screenshot` feature: this build cannot rasterize";
/// How the session a page websocket's root level files under is spelled.
/// The id never leaves the bridge: no frame carries it ([`framed`]).
const HOME_PREFIX: &str = "home:";
const UNKNOWN_SESSION: &str = "Session with given id not found.";

static BROWSER_ID: std::sync::LazyLock<String> = std::sync::LazyLock::new(new_id);
static SCRIPT_IDS: AtomicU64 = AtomicU64::new(1);
/// The bridge's shared state — one per `routes()` mount, not per
/// process: two servers in one process (tests, embedders) get two hubs.
type SharedHub = std::sync::Arc<tokio::sync::Mutex<Hub>>;

/// Everything the bridge remembers, under one lock: the open sockets, the
/// pages, and the sessions attached to either.
#[derive(Default)]
struct Hub {
    next_client: u64,
    clients: HashMap<u64, ClientSlot>,
    pages: HashMap<String, PageSlot>,
    /// Browser contexts made with `Target.createBrowserContext`. The
    /// default context is not one: it is [`CONTEXT_ID`], never listed,
    /// as in Chrome. A context here is a label a page carries — the
    /// engine isolates every page's storage already, so the label is
    /// all the isolation there is to name.
    contexts: std::collections::HashSet<String>,
    /// CDP session id → session. Ids are minted here and mean nothing on
    /// another socket.
    sessions: HashMap<String, SessionSlot>,
}

/// An open socket. `outbox` is how another socket's command reaches it:
/// dropping the hub's copy closes the socket, after what is queued goes out.
struct ClientSlot {
    outbox: mpsc::UnboundedSender<String>,
    auto_attach: bool,
    wait_debugger: bool,
    discover: bool,
}

struct PageSlot {
    /// The browser socket that created the page. It is the only one that
    /// can name it.
    owner: u64,
    target_id: String,
    /// The engine session that holds the page.
    engine_session: String,
    /// The browser context the page was made in: [`CONTEXT_ID`] when
    /// `createTarget` named none or the default.
    context: String,
    url: String,
    title: String,
    loader: u64,
    /// The document row each loader id was told as, for
    /// `Network.getResponseBody`: a document's wire id is its loader id
    /// (as in Chrome, where `requestId == loaderId` is what makes the
    /// request the navigation's), and this names the engine row behind it.
    doc_rows: HashMap<String, String>,
    next_ctx: i64,
    /// The next number `DOM.describeNode` hands a node that has none.
    /// Counting goes on through a navigation: a number of a document gone
    /// names nothing in the next one (Chrome counts each document from 1,
    /// so an old number names whatever sits there now).
    dom_next: i64,
    worlds: Vec<World>,
}

struct World {
    id: i64,
    /// `None` is the default main world. `Some` is an isolated world name.
    name: Option<String>,
}

/// What a session is attached to. A browser session gets a target id of its
/// own, the way each `Target.attachToBrowserTarget` in Chrome does.
#[derive(Clone, PartialEq)]
enum Attached {
    Browser(String),
    Page(String),
}

impl Attached {
    fn id(&self) -> &str {
        match self {
            Attached::Browser(id) | Attached::Page(id) => id,
        }
    }
}

#[derive(Clone)]
struct SessionSlot {
    id: String,
    client: u64,
    attached: Attached,
    /// The session `Target.attachToTarget` ran on. `None` at the root
    /// level of a browser socket.
    parent: Option<String>,
    /// `Target.setAutoAttach` made it, so `autoAttach: false` takes it back.
    auto: bool,
    /// The root level of a page websocket. Its events carry no `sessionId`.
    home: bool,
    /// `Page.enable`: navigation and load events.
    page_on: bool,
    /// `Runtime.enable`: world announcements and console calls.
    runtime_on: bool,
    /// `Page.setLifecycleEventsEnabled`.
    lifecycle_on: bool,
    /// `Network.enable`: the page's requests, responses and their ends.
    network_on: bool,
}

impl SessionSlot {
    fn new(client: u64, attached: Attached, parent: Option<&str>, auto: bool, home: bool) -> Self {
        Self {
            id: new_id(),
            client,
            attached,
            parent: parent.map(str::to_string),
            auto,
            home,
            page_on: false,
            runtime_on: false,
            lifecycle_on: false,
            network_on: false,
        }
    }

    fn on(&self, target: &str) -> bool {
        matches!(&self.attached, Attached::Page(id) if id == target)
    }
}

/// For other sockets, each with the outbox that reaches it.
type Spill = Vec<(mpsc::UnboundedSender<String>, String)>;

/// What one command leaves behind.
#[derive(Default)]
struct Out {
    /// For the socket the command came in on, in this order.
    frames: Vec<String>,
    spill: Spill,
    /// The socket the command came in on is done.
    close_socket: bool,
}

impl Out {
    fn from_frames(frames: Vec<String>) -> Self {
        Self {
            frames,
            spill: Vec::new(),
            close_socket: false,
        }
    }
}

impl Hub {
    /// Register a socket as it is upgraded. With a page, the session its
    /// root level is.
    fn open(
        &mut self,
        page: Option<String>,
    ) -> (u64, Option<String>, mpsc::UnboundedReceiver<String>) {
        let (outbox, inbox) = mpsc::unbounded_channel();
        self.next_client += 1;
        let client = self.next_client;
        self.clients.insert(
            client,
            ClientSlot {
                outbox,
                auto_attach: false,
                wait_debugger: false,
                discover: false,
            },
        );
        let home = page.map(|target| {
            let mut slot =
                SessionSlot::new(client, Attached::Page(target.clone()), None, false, true);
            let id = format!("{HOME_PREFIX}{target}");
            slot.id = id.clone();
            self.sessions.insert(id.clone(), slot);
            id
        });
        (client, home, inbox)
    }

    /// A socket is gone: its sessions, and the pages it owned. Engine
    /// sessions to close, and the frames owed to other sockets — a page
    /// websocket on one of those pages — once the lock is up.
    fn close_client(&mut self, client: u64) -> (Vec<String>, Spill) {
        self.clients.remove(&client);
        let sessions: Vec<String> = self
            .sessions
            .values()
            .filter(|slot| slot.client == client)
            .map(|slot| slot.id.clone())
            .collect();
        let mut out = Out::default();
        for session in sessions {
            self.detach(&session, client, &mut out);
        }
        let owned = self.owned(client);
        (
            self.close_pages(&owned, client, &mut out),
            std::mem::take(&mut out.spill),
        )
    }

    /// Whether a socket may name a page: it created the page, or the
    /// session it speaks through is attached to that page.
    fn owns(&self, client: u64, session: Option<&str>, target: &str) -> bool {
        self.pages
            .get(target)
            .is_some_and(|page| page.owner == client)
            || session
                .and_then(|session| self.sessions.get(session))
                .is_some_and(|slot| slot.on(target))
    }

    fn owned(&self, client: u64) -> Vec<String> {
        self.pages
            .values()
            .filter(|page| page.owner == client)
            .map(|page| page.target_id.clone())
            .collect()
    }

    /// `(discover, auto_attach, wait_debugger)` of a socket.
    fn flags(&self, client: u64) -> (bool, bool, bool) {
        self.clients
            .get(&client)
            .map(|slot| (slot.discover, slot.auto_attach, slot.wait_debugger))
            .unwrap_or((false, false, false))
    }

    fn discovering(&self, client: u64) -> bool {
        self.clients.get(&client).is_some_and(|slot| slot.discover)
    }

    /// The level a session's own events are written on: its socket, and the
    /// session — except a page websocket's root level, which is the socket's.
    fn level(&self, session: Option<&str>) -> Option<String> {
        session
            .filter(|session| self.sessions.get(*session).is_some_and(|slot| !slot.home))
            .map(str::to_string)
    }

    /// The level a session's `Target.*` events go to: the one it was
    /// attached on.
    fn parent_level(&self, slot: &SessionSlot) -> (u64, Option<String>) {
        (
            slot.client,
            slot.parent
                .as_deref()
                .and_then(|parent| self.level(Some(parent))),
        )
    }

    /// Mint a session on `target` for `client`, attached from `parent`, and
    /// queue its `Target.attachedToTarget` on that level — before the
    /// reply, as Chrome does.
    fn attach_page(
        &mut self,
        target: &str,
        client: u64,
        parent: Option<&str>,
        auto: bool,
        waiting: bool,
        out: &mut Out,
    ) -> Result<String, Failure> {
        self.pages
            .get(target)
            .filter(|_| self.owns(client, parent, target))
            .ok_or_else(|| Failure::Invalid("No target with given id found".to_string()))?;
        let level = self.level(parent);
        let slot = SessionSlot::new(
            client,
            Attached::Page(target.to_string()),
            parent,
            auto,
            false,
        );
        let id = slot.id.clone();
        self.sessions.insert(id.clone(), slot);
        // The session is in when the page is described: it says `attached`.
        let info = info_of(self, &self.pages[target]);
        out.frames.push(event(
            level.as_deref(),
            "Target.attachedToTarget",
            json!({
                "sessionId": id,
                "waitingForDebugger": waiting,
                "targetInfo": info,
            }),
        ));
        Ok(id)
    }

    /// End a session, and every session attached through it. The level it
    /// was attached on hears `Target.detachedFromTarget` for that one; the
    /// rest go with it, unannounced — as in Chrome.
    fn detach(&mut self, session: &str, local: u64, out: &mut Out) {
        let Some(slot) = self.sessions.remove(session) else {
            return;
        };
        // A page websocket's root level was never attached: nothing to hear
        // that it is gone, and its id leaves the bridge in no frame.
        if slot.home {
            return;
        }
        let to = self.parent_level(&slot);
        deliver(
            self,
            out,
            local,
            to,
            "Target.detachedFromTarget",
            json!({ "sessionId": slot.id, "targetId": slot.attached.id() }),
        );
        let mut gone = vec![slot];
        while let Some(parent) = gone.pop() {
            let children: Vec<SessionSlot> = self
                .sessions
                .values()
                .filter(|other| other.parent.as_deref() == Some(parent.id.as_str()))
                .cloned()
                .collect();
            for child in children {
                self.sessions.remove(&child.id);
                gone.push(child);
            }
        }
    }

    /// Close pages. In Chrome's order: every session on one hears
    /// `Inspector.detached`, then the level each was attached on hears
    /// `Target.detachedFromTarget`, then an owner that asked to discover
    /// hears the page is no longer attached, and that it is gone. A page
    /// websocket on it is closed. `local` is the socket the command came in
    /// on. Engine sessions to close, once the lock is up.
    fn close_pages(&mut self, targets: &[String], local: u64, out: &mut Out) -> Vec<String> {
        let mut engine = Vec::new();
        for target in targets {
            let Some(page) = self.pages.remove(target) else {
                continue;
            };
            engine.push(page.engine_session.clone());
            let sessions: Vec<SessionSlot> = self
                .sessions
                .values()
                .filter(|slot| slot.on(&page.target_id))
                .cloned()
                .collect();
            for slot in &sessions {
                let own = (slot.client, self.level(Some(slot.id.as_str())));
                deliver(
                    self,
                    out,
                    local,
                    own,
                    "Inspector.detached",
                    json!({ "reason": "target_closed" }),
                );
            }
            for slot in &sessions {
                self.sessions.remove(&slot.id);
                // A page websocket on a closed page is closed too, and hears
                // nothing more: it was never attached.
                if slot.home {
                    self.clients.remove(&slot.client);
                    continue;
                }
                let to = self.parent_level(slot);
                deliver(
                    self,
                    out,
                    local,
                    to,
                    "Target.detachedFromTarget",
                    json!({ "sessionId": slot.id, "targetId": page.target_id }),
                );
            }
            if self.discovering(page.owner) {
                deliver(
                    self,
                    out,
                    local,
                    (page.owner, None),
                    "Target.targetInfoChanged",
                    json!({ "targetInfo": info_of(self, &page) }),
                );
                deliver(
                    self,
                    out,
                    local,
                    (page.owner, None),
                    "Target.targetDestroyed",
                    json!({ "targetId": page.target_id }),
                );
            }
        }
        engine
    }
}

pub(crate) fn routes() -> Router {
    let hub: SharedHub = std::sync::Arc::new(tokio::sync::Mutex::new(Hub::default()));
    Router::new()
        .route("/json/version", get(version))
        .route("/json/version/", get(version))
        .route("/json/list", get(list))
        .route("/json", get(list))
        .route("/devtools/browser/:id", get(browser_ws))
        .route("/devtools/page/:id", get(page_ws))
        .with_state(hub)
}

fn product() -> String {
    format!("diting/{}", env!("CARGO_PKG_VERSION"))
}

fn cdp_user_agent() -> String {
    let os = if cfg!(target_os = "macos") {
        "Macintosh; Intel Mac OS X 10_15_7"
    } else if cfg!(target_os = "windows") {
        "Windows NT 10.0; Win64; x64"
    } else {
        "X11; Linux x86_64"
    };
    format!(
        "Mozilla/5.0 ({os}) AppleWebKit/537.36 (KHTML, like Gecko) {}",
        product()
    )
}

fn browser_id() -> &'static str {
    BROWSER_ID.as_str()
}

/// An id in Chrome's spelling: 32 hex digits.
fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Event timestamps: seconds of the wall clock, as a float. Chrome counts
/// monotonic time from an origin of its own; nothing reads the value but
/// as "later is bigger".
fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn host_of(headers: &HeaderMap) -> String {
    headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or("127.0.0.1:8089")
        .to_string()
}

async fn version(headers: HeaderMap) -> Json<Value> {
    Json(json!({
        "Browser": product(),
        "Protocol-Version": "1.3",
        "User-Agent": cdp_user_agent(),
        "V8-Version": "diting",
        "WebKit-Version": "537.36",
        // Chrome's own key: `posix` or `windows` by the host. Measured on
        // a posix host; the Windows word is Chrome's, not seen here.
        "File-Path-Style": if cfg!(windows) { "windows" } else { "posix" },
        "webSocketDebuggerUrl": format!("ws://{}/devtools/browser/{}", host_of(&headers), browser_id()),
    }))
}

async fn list(State(hub): State<SharedHub>, headers: HeaderMap) -> Json<Value> {
    let host = host_of(&headers);
    let hub = hub.lock().await;
    let pages: Vec<Value> = hub
        .pages
        .values()
        .map(|page| {
            json!({
                "id": page.target_id,
                "type": "page",
                "title": page.title,
                "url": page.url,
                "webSocketDebuggerUrl": format!("ws://{host}/devtools/page/{}", page.target_id),
                "devtoolsFrontendUrl": "",
            })
        })
        .collect();
    Json(Value::Array(pages))
}

async fn browser_ws(
    State(hub): State<SharedHub>,
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
) -> Response {
    if id != browser_id() {
        return (StatusCode::NOT_FOUND, "unknown browser").into_response();
    }
    let (client, home, inbox) = hub.lock().await.open(None);
    ws.on_upgrade(move |socket| run_socket(hub, socket, client, home, inbox))
}

async fn page_ws(
    State(hub): State<SharedHub>,
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
) -> Response {
    let opened = {
        let mut hub = hub.lock().await;
        hub.pages.contains_key(&id).then(|| hub.open(Some(id)))
    };
    match opened {
        Some((client, home, inbox)) => {
            ws.on_upgrade(move |socket| run_socket(hub, socket, client, home, inbox))
        }
        None => (StatusCode::NOT_FOUND, "unknown page").into_response(),
    }
}

/// One socket, browser or page. Commands come in from the socket, events
/// for it from other sockets come in through the inbox. A dropped outbox —
/// its page was closed, it ran `Browser.close` — ends the socket once what
/// is queued has gone out.
async fn run_socket(
    hub: SharedHub,
    mut socket: WebSocket,
    client: u64,
    home: Option<String>,
    mut inbox: mpsc::UnboundedReceiver<String>,
) {
    loop {
        tokio::select! {
            frame = inbox.recv() => {
                let Some(frame) = frame else { break };
                if socket.send(Message::Text(frame)).await.is_err() {
                    break;
                }
            }
            message = socket.recv() => {
                let text = match message {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                        continue;
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => continue,
                };
                let out = dispatch(hub.clone(), client, home.as_deref(), &text).await;
                let mut open = true;
                for frame in out.frames {
                    if socket.send(Message::Text(frame)).await.is_err() {
                        open = false;
                        break;
                    }
                }
                for (outbox, frame) in out.spill {
                    let _ = outbox.send(frame);
                }
                if !open || out.close_socket {
                    break;
                }
            }
        }
    }
    let (engine, spill) = {
        let mut guard = hub.lock().await;
        guard.close_client(client)
    };
    for (outbox, frame) in spill {
        let _ = outbox.send(frame);
    }
    target::close_engine(hub.clone(), engine).await;
}

async fn dispatch(hub: SharedHub, client: u64, home: Option<&str>, text: &str) -> Out {
    let envelope::Frame {
        id,
        session: named,
        method,
        params,
    } = match envelope::read(text) {
        Ok(frame) => frame,
        Err(out) => return out,
    };
    // A page websocket has the one session, its root level, and names no other.
    let session = match (&named, home) {
        (Some(_), Some(_)) => return refused(&id, None, -32001, UNKNOWN_SESSION),
        (Some(session), _) => Some(session.as_str()),
        (None, home) => home,
    };
    match dispatch_method(hub, client, &id, session, &method, &params).await {
        Ok(out) => out,
        Err(Failure::NotFound(method)) => {
            refused(&id, session, -32601, &format!("'{method}' wasn't found"))
        }
        Err(Failure::Invalid(message)) => refused(&id, session, -32602, &message),
        Err(Failure::Refused(message)) => refused(&id, session, -32000, &message),
        Err(Failure::Internal(message)) => refused(&id, session, -32603, &message),
        // Chrome's reply names no session for an id it does not know.
        Err(Failure::UnknownSession) => refused(&id, None, -32001, UNKNOWN_SESSION),
    }
}

/// How a command failed. Chrome answers a method it does not have with
/// `-32601`, one whose parameters it will not read with `-32602`, one it
/// has but cannot run with `-32000` and the reason, and a session it never
/// minted with `-32001`; a client reads the four differently. `From<String>`
/// and `From<&str>` are the last but one: that is what a handler means by
/// `Err(message)`. The fifth, `Internal`, is Chrome's `-32603` — a failure
/// of the browser's own that no handler could have checked for.
#[derive(Debug, PartialEq)]
pub(crate) enum Failure {
    NotFound(String),
    Invalid(String),
    Refused(String),
    UnknownSession,
    Internal(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Refused(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Failure::Refused(message.to_string())
    }
}

fn refused(id: &Value, session: Option<&str>, code: i64, message: &str) -> Out {
    Out::from_frames(vec![fail(id, session, code, message)])
}

async fn dispatch_method(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    method: &str,
    params: &Value,
) -> Result<Out, Failure> {
    // A session is its socket's: an id minted on another socket names
    // nothing here.
    let on_page = {
        let hub = hub.lock().await;
        match session.filter(|session| {
            hub.sessions
                .get(*session)
                .is_some_and(|slot| slot.client == client)
        }) {
            None if session.is_some() => return Err(Failure::UnknownSession),
            None => false,
            // A page websocket's root level takes `Target.setAutoAttach` as a page does.
            Some(session) => {
                let slot = &hub.sessions[session];
                matches!(slot.attached, Attached::Page(_))
                    && (!browser_domain(method) || (slot.home && method == "Target.setAutoAttach"))
            }
        }
    };
    match session.filter(|_| on_page) {
        Some(session) => page_method(hub, id, session, method, params).await,
        _ => target::browser_method(hub, client, id, session, method, params).await,
    }
}

/// `Browser.*` and `Target.*` are answered at the level they arrive on.
fn browser_domain(method: &str) -> bool {
    method.starts_with("Browser.") || method.starts_with("Target.")
}

async fn page_method(
    hub: SharedHub,
    id: &Value,
    session: &str,
    method: &str,
    params: &Value,
) -> Result<Out, Failure> {
    let on = Some(session);
    match method {
        "Page.getFrameTree" => {
            let hub = hub.lock().await;
            let tree = target::frame_tree(&hub, session)?;
            Ok(Out::from_frames(vec![ok(id, on, tree)]))
        }
        "Page.navigate" => target::navigate(hub.clone(), id, session, params).await,
        "Page.getNavigationHistory" => {
            let history = target::navigation_history(hub.clone(), session).await?;
            Ok(Out::from_frames(vec![ok(id, on, history)]))
        }
        "Page.navigateToHistoryEntry" => {
            target::navigate_to_entry(hub.clone(), id, session, params).await
        }
        "Page.reload" => target::reload(hub.clone(), id, session, params).await,
        "Page.close" => target::close_page(hub.clone(), id, session).await,
        "Page.captureScreenshot" => capture_screenshot(hub, id, session, params).await,
        "Page.enable" | "Page.disable" | "Page.setLifecycleEventsEnabled" => {
            target::page_switch(hub.clone(), id, session, method, params).await
        }
        "Page.createIsolatedWorld" => target::create_world(hub.clone(), id, session, params).await,
        "Runtime.enable" => target::runtime_enable(hub.clone(), id, session).await,
        "Runtime.disable" => target::runtime_disable(hub.clone(), id, session).await,
        "Target.setAutoAttach" => target::page_auto_attach(id, session, params),
        "Emulation.setFocusEmulationEnabled" => {
            standing_switch(params, "enabled", true, method, FOCUS)?;
            Ok(ack(id, session))
        }
        "Page.setBypassCSP" => {
            standing_switch(params, "enabled", true, method, CSP)?;
            Ok(ack(id, session))
        }
        "Security.setIgnoreCertificateErrors" => {
            standing_switch(params, "ignore", false, method, CERTIFICATES)?;
            Ok(ack(id, session))
        }
        "Page.handleJavaScriptDialog" => {
            // Read as Chrome reads them, then: the page has no dialogs.
            boolean(params, "accept")?;
            optional_string(params, "promptText")?;
            Err(invalid("No dialog is showing"))
        }
        // Its fields are read, though nothing is done with them: the
        // engine buffers no bodies by size.
        "Network.enable" => {
            kinds(
                params,
                &[
                    ("maxTotalBufferSize", Kind::Integer),
                    ("maxResourceBufferSize", Kind::Integer),
                    ("maxPostDataSize", Kind::Integer),
                ],
            )?;
            set_network(&hub, session, true).await;
            Ok(ack(id, session))
        }
        "Network.getResponseBody" => {
            let body = network_events::response_body(hub, session, params).await?;
            Ok(Out::from_frames(vec![ok(id, on, body)]))
        }
        "Network.disable" => {
            set_network(&hub, session, false).await;
            Ok(ack(id, session))
        }
        _ if PAGE_CHANNEL.contains(&method) => {
            runtime_forward(hub, id, session, method, params).await
        }
        _ if STATELESS.contains(&method) => Ok(ack(id, session)),
        _ => Err(Failure::NotFound(method.to_string())),
    }
}

/// `Network.enable` / `Network.disable` for the session they came on.
async fn set_network(hub: &SharedHub, session: &str, on: bool) {
    if let Some(slot) = hub.lock().await.sessions.get_mut(session) {
        slot.network_on = on;
    }
}

/// `Page.captureScreenshot`. A build without the `screenshot` feature has
/// no raster: the command is refused by name — an empty picture would be
/// a lie.
async fn capture_screenshot(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    #[cfg(feature = "screenshot")]
    {
        screenshot::capture(hub, id, session, params).await
    }
    #[cfg(not(feature = "screenshot"))]
    {
        let _ = (id, session, params);
        Err(NEEDS_SCREENSHOT.into())
    }
}

fn ack(id: &Value, session: &str) -> Out {
    Out::from_frames(vec![ok(id, Some(session), json!({}))])
}

const FOCUS: &str = "a diting page always reports focus";
const CSP: &str = "diting never enforces CSP";
const CERTIFICATES: &str = "diting verifies every certificate";

/// Page-level commands the session thread runs, where the page and the
/// isolate live (`runtime::dispatch` sorts them).
const PAGE_CHANNEL: &[&str] = &[
    "Runtime.evaluate",
    "Runtime.callFunctionOn",
    "Runtime.releaseObject",
    "Input.dispatchMouseEvent",
    "Input.dispatchKeyEvent",
    "Input.insertText",
    "DOM.getContentQuads",
    "DOM.scrollIntoViewIfNeeded",
    "DOM.describeNode",
    "DOM.resolveNode",
    "DOM.getBoxModel",
    "Emulation.setEmulatedMedia",
    "Emulation.setTouchEmulationEnabled",
    "Emulation.setTimezoneOverride",
    "Emulation.setLocaleOverride",
    "Emulation.setUserAgentOverride",
    "Emulation.setDeviceMetricsOverride",
    "Emulation.clearDeviceMetricsOverride",
    "Page.addScriptToEvaluateOnNewDocument",
    "Page.removeScriptToEvaluateOnNewDocument",
    "Page.getLayoutMetrics",
    "Network.setBlockedURLs",
    "Network.setExtraHTTPHeaders",
];

/// Answered with an empty ok: the engine has no state for them to flip —
/// no reporting domain to turn on, no lifecycle gating, no window order.
const STATELESS: &[&str] = &[
    "Audits.enable",
    "Audits.disable",
    "Performance.enable",
    "Performance.disable",
    "Page.bringToFront",
    "Log.enable",
    "Log.disable",
    "DOM.enable",
    "DOM.disable",
    "CSS.enable",
    "CSS.disable",
    "Runtime.runIfWaitingForDebugger",
];

async fn runtime_forward(
    hub: SharedHub,
    id: &Value,
    session: &str,
    method: &str,
    params: &Value,
) -> Result<Out, Failure> {
    let mut out = Out::default();
    let result = forward(hub, session, method, params, &mut out).await?;
    out.frames.push(ok(id, Some(session), result));
    Ok(out)
}

/// Run one [`PAGE_CHANNEL`] command on the session thread. What it drags
/// along — console events — goes into `out` before its reply.
async fn forward(
    hub: SharedHub,
    session: &str,
    method: &str,
    params: &Value,
    out: &mut Out,
) -> Result<Value, Failure> {
    let (engine, local, frame_id, number, contexts) = {
        let mut hub = hub.lock().await;
        let local = local_of(&hub, session);
        let page = page_of_mut(&mut hub, session)?;
        // A node named by handle takes its number from the hub, which goes
        // on counting where the last document stopped. A number sent with
        // the handle is what names the node, and no number is asked for.
        let number = (method == "DOM.describeNode"
            && params.get("objectId").is_some()
            && params.get("backendNodeId").is_none()
            && params.get("nodeId").is_none())
        .then(|| {
            let number = page.dom_next;
            page.dom_next += 1;
            number
        });
        let contexts = page.worlds.iter().map(|world| world.id).collect::<Vec<_>>();
        (
            page.engine_session.clone(),
            local,
            page.target_id.clone(),
            number,
            contexts,
        )
    };
    let mut params = params.clone();
    // A `params` that is no object goes on as it is, for the session thread
    // to refuse; it has no fields to add to.
    if let Some(fields) = params.as_object_mut() {
        if method == "DOM.describeNode" {
            // The frame a document element belongs to, and the number the
            // node takes if it has none. The session thread has no hub to ask.
            fields.insert("__ditingFrameId".into(), json!(frame_id));
            if let Some(number) = number {
                fields.insert("__ditingNodeId".into(), json!(number));
            }
        }
        if matches!(
            method,
            "Runtime.evaluate" | "Runtime.callFunctionOn" | "DOM.resolveNode"
        ) {
            // The contexts the page announced, for a command that names one.
            fields.insert("__ditingContexts".into(), json!(contexts));
        }
    }
    // `Runtime.evaluate` is an IR command now, not a Cdp passthrough: the
    // params are read here, the engine work happens on the session thread
    // as `EvalRemote`, and the wire shape is wrapped back here.
    if method == "Runtime.evaluate" {
        let args = runtime::eval_params(&params)?;
        let (outcome, pre_info, calls, write_nav) =
            send_command(&engine, |reply| SessionCommand::EvalRemote {
                script: args.script,
                await_promise: args.await_promise,
                return_by_value: args.return_by_value,
                timeout_ms: None,
                reply,
            })
            .await
            .map_err(|err| runtime::engine_failure(session_msg(err)))?;
        let value = runtime::eval_result(outcome, pre_info)?;
        let console: Vec<Value> = calls
            .into_iter()
            .filter_map(|(level, text, url)| {
                let kind = runtime::console_type(&level)?;
                Some(json!({ "type": kind, "text": text, "url": url }))
            })
            .collect();
        if !console.is_empty() || write_nav {
            let context_id = params
                .get("contextId")
                .or_else(|| params.get("executionContextId"))
                .and_then(|v| v.as_i64())
                .filter(|id| *id > 0);
            let side = json!({
                "console": console,
                "writeNav": write_nav,
                "contextId": context_id,
            });
            let hub = hub.lock().await;
            side_frames(&hub, local, session, &side, out);
        }
        return Ok(value);
    }
    // `Runtime.callFunctionOn` is an IR command too — same shape as
    // `Runtime.evaluate` above: params read here, engine work as
    // `CallOn` on the session thread, wire shape wrapped back here.
    if method == "Runtime.callFunctionOn" {
        let args = runtime::call_params(&params)?;
        let (outcome, pre_info, calls, write_nav) =
            send_command(&engine, |reply| SessionCommand::CallOn {
                args: CallOnRequest {
                    declaration: args.declaration,
                    object_id: args.object_id,
                    arguments: args.arguments,
                    by_value: false,
                    await_promise: args.await_promise,
                    return_by_value: args.return_by_value,
                    timeout_ms: None,
                    receiver: args.receiver,
                    arg_ids: args.arg_ids,
                },
                reply,
            })
            .await
            .map_err(|err| runtime::engine_failure(session_msg(err)))?;
        let value = runtime::eval_result(outcome, pre_info)?;
        let console: Vec<Value> = calls
            .into_iter()
            .filter_map(|(level, text, url)| {
                let kind = runtime::console_type(&level)?;
                Some(json!({ "type": kind, "text": text, "url": url }))
            })
            .collect();
        if !console.is_empty() || write_nav {
            let context_id = params
                .get("executionContextId")
                .or_else(|| params.get("contextId"))
                .and_then(|v| v.as_i64())
                .filter(|id| *id > 0);
            let side = json!({
                "console": console,
                "writeNav": write_nav,
                "contextId": context_id,
            });
            let hub = hub.lock().await;
            side_frames(&hub, local, session, &side, out);
        }
        return Ok(value);
    }
    // `Runtime.releaseObject` — the last Runtime-domain command on the
    // IR: the id and its parts go down, the engine lets the handle go.
    if method == "Runtime.releaseObject" {
        let (object_id, parts) = runtime::release_params(&params)?;
        send_command(&engine, |reply| SessionCommand::ReleaseObject {
            object_id,
            parts,
            reply,
        })
        .await
        .map_err(|err| runtime::engine_failure(session_msg(err)))?;
        return Ok(json!({}));
    }
    // The five `DOM.*` node commands — IR `CallOn`s orchestrated in
    // `dom_run`: params read here, engine work on the session thread.
    if matches!(
        method,
        "DOM.describeNode"
            | "DOM.resolveNode"
            | "DOM.getBoxModel"
            | "DOM.getContentQuads"
            | "DOM.scrollIntoViewIfNeeded"
    ) {
        let (value, side) = runtime::dom_run(&engine, method, &params).await?;
        if let Some(side) = side {
            let hub = hub.lock().await;
            side_frames(&hub, local, session, &side, out);
        }
        return Ok(value);
    }
    // The three `Input.*` commands — params become a script here, the
    // script runs as an `EvalRemote` on the session thread.
    if method.starts_with("Input.") {
        let script = runtime::input_script(method, &params)?;
        if let Some(script) = script {
            let (value, side) = runtime::input_run(&engine, &script).await?;
            if let Some(side) = side {
                let hub = hub.lock().await;
                side_frames(&hub, local, session, &side, out);
            }
            return Ok(value);
        }
        return Ok(json!({}));
    }
    // The seven `Emulation.*` setters — params become an `EmulateOp`
    // here, the session thread pins the page state.
    if method.starts_with("Emulation.") {
        return emulation::emulate_run(&engine, method, &params).await;
    }
    // The last page-level commands on the IR: init-script registry,
    // blocked URLs, extra headers — and the layout-metrics read.
    if matches!(
        method,
        "Page.addScriptToEvaluateOnNewDocument"
            | "Page.removeScriptToEvaluateOnNewDocument"
            | "Network.setBlockedURLs"
            | "Network.setExtraHTTPHeaders"
    ) {
        return emulation::page_run(&engine, method, &params).await;
    }
    if method == "Page.getLayoutMetrics" {
        return emulation::metrics_run(&engine).await;
    }
    // Everything the bridge routes is an IR command now — what is left
    // has no handler, in the words the old Cdp channel gave it.
    Err(Failure::from(format!(
        "'{method}' has no handler on the session thread"
    )))
}

/// Console events first, then the document.write load — for every session
/// of the page that runs the domain. Playwright's setContent clears
/// lifecycle when the debug tag arrives and then waits for `load`. A load
/// that lands first is wiped by that clear.
fn side_frames(hub: &Hub, local: u64, session: &str, side: &Value, out: &mut Out) {
    if let Ok(page) = page_of(hub, session) {
        page_side::page_side_frames(hub, local, page, side, out);
    }
}

/// An event for one place: the socket a command came in on, or another
/// socket's outbox. The session it is written for is `None` on a root
/// level.
fn deliver(
    hub: &Hub,
    out: &mut Out,
    local: u64,
    to: (u64, Option<String>),
    method: &str,
    params: Value,
) {
    let frame = event(to.1.as_deref(), method, params);
    if to.0 == local {
        out.frames.push(frame);
    } else if let Some(slot) = hub.clients.get(&to.0) {
        out.spill.push((slot.outbox.clone(), frame));
    }
}

/// An event for every session attached to `target` that `wants` it —
/// whatever socket it is on.
fn to_sessions(
    hub: &Hub,
    out: &mut Out,
    local: u64,
    target: &str,
    wants: impl Fn(&SessionSlot) -> bool,
    method: &str,
    params: impl Fn() -> Value,
) {
    for slot in hub.sessions.values() {
        if !slot.on(target) || !wants(slot) {
            continue;
        }
        deliver(
            hub,
            out,
            local,
            (slot.client, hub.level(Some(&slot.id))),
            method,
            params(),
        );
    }
}

/// A switch the engine keeps in one state for good. That state is an empty
/// ok; the other is refused with `why`, because answering it would promise
/// a change that never happens.
fn standing_switch(
    params: &Value,
    field: &str,
    kept: bool,
    method: &str,
    why: &str,
) -> Result<(), Failure> {
    match boolean(params, field)? {
        want if want == kept => Ok(()),
        _ => Err(Failure::from(format!("{method}: {why}"))),
    }
}

/// The socket a session speaks on: where its events go, and the one a
/// command from it keeps its frames on.
fn local_of(hub: &Hub, session: &str) -> u64 {
    hub.sessions.get(session).map_or(0, |slot| slot.client)
}

/// The page a session is attached to. `Err` once the session is gone.
fn page_of<'a>(hub: &'a Hub, session: &str) -> Result<&'a PageSlot, String> {
    let slot = hub
        .sessions
        .get(session)
        .ok_or_else(|| format!("no such session {session}"))?;
    match &slot.attached {
        Attached::Page(target) => hub
            .pages
            .get(target)
            .ok_or_else(|| format!("no such target {target}")),
        Attached::Browser(_) => Err("not a page session".to_string()),
    }
}

fn page_of_mut<'a>(hub: &'a mut Hub, session: &str) -> Result<&'a mut PageSlot, String> {
    let target = hub
        .sessions
        .get(session)
        .ok_or_else(|| format!("no such session {session}"))?
        .attached
        .id()
        .to_string();
    hub.pages
        .get_mut(&target)
        .ok_or_else(|| format!("no such target {target}"))
}

fn loader_of(page: &PageSlot) -> String {
    format!("ldr{}", page.loader)
}

/// Remember which engine row the current loader's document is told as, so
/// `Network.getResponseBody` can translate the wire id back. Rows of one
/// navigation share the loader, and only document rows are told under it.
/// The table keeps the recent few: a client that asks for a body does it
/// soon after the response, and the map must not grow with every visit.
fn remember_doc_rows(page: &mut PageSlot, rows: &[diting::diting_browser::page::NetworkEvent]) {
    let loader = loader_of(page);
    for row in rows {
        if row.resource_type == "Document" {
            page.doc_rows.insert(loader.clone(), row.request_id.clone());
        }
    }
    if page.doc_rows.len() > 16 {
        let keep_from = page.loader.saturating_sub(16);
        page.doc_rows.retain(|wire, _| {
            wire.strip_prefix("ldr")
                .and_then(|n| n.parse::<u64>().ok())
                .is_some_and(|n| n >= keep_from)
        });
    }
}

/// The frame a page is: the main frame, with the target's id.
fn frame_of(page: &PageSlot) -> Value {
    json!({
        "id": page.target_id,
        "loaderId": loader_of(page),
        "url": page.url,
        "mimeType": "text/html",
        "securityOrigin": security_origin(&page.url),
        "domainAndRegistry": "",
        "name": "",
    })
}

fn security_origin(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {
            parsed.origin().ascii_serialization()
        }
        _ => "://".to_string(),
    }
}

fn info_of(hub: &Hub, page: &PageSlot) -> Value {
    let attached = hub.sessions.values().any(|slot| slot.on(&page.target_id));
    json!({
        "targetId": page.target_id,
        "type": "page",
        "title": page.title,
        "url": page.url,
        "attached": attached,
        "canAccessOpener": false,
        "browserContextId": page.context,
    })
}

/// A world's `uniqueId` as it is announced and as `uniqueContextId` names
/// it back — its own spelling, never the context id's, so the two cannot
/// be confused for one another.
fn unique_id(id: i64) -> String {
    format!("{id}.{id}")
}

fn context_of(page: &PageSlot, world: &World) -> Value {
    let is_default = world.name.is_none();
    json!({
        "context": {
            "id": world.id,
            "origin": security_origin(&page.url),
            "name": world.name.clone().unwrap_or_default(),
            "uniqueId": unique_id(world.id),
            "auxData": {
                "isDefault": is_default,
                "type": if is_default { "default" } else { "isolated" },
                "frameId": page.target_id,
            }
        }
    })
}

/// A `Page.lifecycleEvent`: Chrome counts the timestamp in monotonic
/// seconds; nothing reads the value but as "later is bigger".
fn lifecycle(frame_id: &str, loader: &str, name: &str, timestamp: f64) -> Value {
    json!({
        "frameId": frame_id,
        "loaderId": loader,
        "name": name,
        "timestamp": timestamp,
    })
}

/// The session a frame names: the one it was written for, unless that is a
/// page websocket's root level, whose frames carry no `sessionId`.
fn framed(session: Option<&str>) -> Option<&str> {
    session.filter(|session| !session.starts_with(HOME_PREFIX))
}

fn ok(id: &Value, session: Option<&str>, result: Value) -> String {
    let mut body = json!({ "id": id, "result": result });
    if let Some(session) = framed(session) {
        body["sessionId"] = json!(session);
    }
    body.to_string()
}

fn fail(id: &Value, session: Option<&str>, code: i64, message: &str) -> String {
    let mut body = json!({
        "id": id,
        "error": { "code": code, "message": message }
    });
    if let Some(session) = framed(session) {
        body["sessionId"] = json!(session);
    }
    body.to_string()
}

fn event(session: Option<&str>, method: &str, params: Value) -> String {
    let mut body = json!({ "method": method, "params": params });
    if let Some(session) = framed(session) {
        body["sessionId"] = json!(session);
    }
    body.to_string()
}

pub(super) fn session_msg(err: crate::session::SessionError) -> String {
    match err {
        crate::session::SessionError::NotFound(msg)
        | crate::session::SessionError::Expired(msg)
        | crate::session::SessionError::ThreadDied(msg)
        | crate::session::SessionError::Command(msg)
        | crate::session::SessionError::Eval(msg)
        | crate::session::SessionError::Dormant(msg) => msg,
    }
}
