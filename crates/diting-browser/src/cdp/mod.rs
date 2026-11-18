//! First-slice Chrome DevTools Protocol.
//!
//! Enough for Playwright `chromium.connectOverCDP` to connect, open a page,
//! `goto`, `setContent`, `evaluate`, click, fill, and `screenshot`.
//! `/json/version` names this engine `diting`; the page's own user agent
//! stays the Chrome 157 pin. This is not the 1,740-task driver matrix — a
//! method outside the slice returns a protocol error naming itself.
//!
//! The hub lock is never held across a session await. The session thread
//! does not know about the hub, but an HTTP handler can hold `SESSIONS`
//! while another task wants the hub.

pub(crate) mod runtime;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::session::{send_command, send_screenshot, SessionCommand, SESSIONS};

#[cfg(test)]
mod tests;

const CONTEXT_ID: &str = "default";

static BROWSER_ID: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| uuid::Uuid::new_v4().simple().to_string());
static SCRIPT_IDS: AtomicU64 = AtomicU64::new(1);
static HUB: std::sync::LazyLock<tokio::sync::Mutex<Hub>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(Hub::default()));

#[derive(Default)]
struct Hub {
    auto_attach: bool,
    wait_debugger: bool,
    discover: bool,
    pages: HashMap<String, PageSlot>,
    /// CDP session id → target id.
    sessions: HashMap<String, String>,
}

struct PageSlot {
    target_id: String,
    session_id: String,
    cdp_session: String,
    frame_id: String,
    url: String,
    title: String,
    loader: u64,
    next_ctx: i64,
    runtime_on: bool,
    attached: bool,
    worlds: Vec<World>,
    viewport: (u32, u32),
}

struct World {
    id: i64,
    /// `None` is the default main world. `Some` is an isolated world name.
    name: Option<String>,
}

struct Out {
    frames: Vec<String>,
    created: Vec<String>,
    closed: Vec<String>,
    close_socket: bool,
}

impl Out {
    fn from_frames(frames: Vec<String>) -> Self {
        Self {
            frames,
            created: Vec::new(),
            closed: Vec::new(),
            close_socket: false,
        }
    }
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route("/json/version", get(version))
        .route("/json/version/", get(version))
        .route("/json/list", get(list))
        .route("/json", get(list))
        .route("/devtools/browser/:id", get(browser_ws))
        .route("/devtools/page/:id", get(page_ws))
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
        "webSocketDebuggerUrl": format!("ws://{}/devtools/browser/{}", host_of(&headers), browser_id()),
    }))
}

async fn list(headers: HeaderMap) -> Json<Value> {
    let host = host_of(&headers);
    let hub = HUB.lock().await;
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

async fn browser_ws(ws: WebSocketUpgrade, Path(id): Path<String>) -> Response {
    if id != browser_id() {
        return (StatusCode::NOT_FOUND, "unknown browser").into_response();
    }
    ws.on_upgrade(|socket| run_socket(socket, None))
}

async fn page_ws(ws: WebSocketUpgrade, Path(id): Path<String>) -> Response {
    let session = {
        let hub = HUB.lock().await;
        hub.pages.get(&id).map(|page| page.cdp_session.clone())
    };
    match session {
        Some(session) => ws.on_upgrade(move |socket| run_socket(socket, Some(session))),
        None => (StatusCode::NOT_FOUND, "unknown page").into_response(),
    }
}

async fn run_socket(mut socket: WebSocket, force_session: Option<String>) {
    let mut mine: Vec<String> = Vec::new();
    loop {
        let msg = match socket.recv().await {
            Some(Ok(msg)) => msg,
            _ => break,
        };
        let text = match msg {
            Message::Text(text) => text,
            Message::Ping(payload) => {
                let _ = socket.send(Message::Pong(payload)).await;
                continue;
            }
            Message::Close(_) => break,
            _ => continue,
        };
        let text = force_session
            .as_deref()
            .map(|session| inject_session(&text, session))
            .unwrap_or(text);
        let out = dispatch(&text).await;
        mine.extend(out.created);
        mine.retain(|id| !out.closed.contains(id));
        let mut failed = false;
        for frame in out.frames {
            if socket.send(Message::Text(frame)).await.is_err() {
                failed = true;
                break;
            }
        }
        if failed || out.close_socket {
            break;
        }
    }
    for id in mine {
        let mut mgr = SESSIONS.lock().await;
        mgr.close_and_wait(&id).await;
    }
}

fn inject_session(text: &str, session: &str) -> String {
    let mut msg: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return text.to_string(),
    };
    if msg.get("sessionId").and_then(|v| v.as_str()).is_none() {
        msg["sessionId"] = json!(session);
    }
    msg.to_string()
}

async fn dispatch(text: &str) -> Out {
    let msg: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Out::from_frames(Vec::new()),
    };
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let session = msg
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    if method.is_empty() {
        return Out::from_frames(Vec::new());
    }
    match dispatch_method(&id, session.as_deref(), method, &params).await {
        Ok(out) => out,
        Err(message) => Out::from_frames(vec![fail(&id, session.as_deref(), message)]),
    }
}

async fn dispatch_method(
    id: &Value,
    session: Option<&str>,
    method: &str,
    params: &Value,
) -> Result<Out, String> {
    if session.is_none() {
        return browser_method(id, method, params).await;
    }
    page_method(id, session.unwrap(), method, params).await
}

async fn browser_method(id: &Value, method: &str, params: &Value) -> Result<Out, String> {
    match method {
        "Browser.getVersion" => Ok(Out::from_frames(vec![ok(
            id,
            None,
            json!({
                "protocolVersion": "1.3",
                "product": product(),
                "revision": "0",
                "userAgent": cdp_user_agent(),
                "jsVersion": "diting",
            }),
        )])),
        "Browser.setDownloadBehavior" => Ok(Out::from_frames(vec![ok(id, None, json!({}))])),
        "Browser.close" => close_browser(id).await,
        "Target.setAutoAttach" => set_auto_attach(id, params).await,
        "Target.setDiscoverTargets" => set_discover(id, params).await,
        "Target.getTargetInfo" => Ok(Out::from_frames(vec![ok(
            id,
            None,
            target_info_result(params).await,
        )])),
        "Target.getTargets" => Ok(Out::from_frames(vec![ok(id, None, all_targets().await)])),
        "Target.createTarget" => create_target(id, params).await,
        "Target.attachToTarget" => attach_target(id, params).await,
        "Target.detachFromTarget" => Ok(Out::from_frames(vec![ok(id, None, json!({}))])),
        "Target.closeTarget" => close_target(id, None, params).await,
        _ if ack_only(method) => Ok(Out::from_frames(vec![ok(id, None, json!({}))])),
        _ => Err(format!("'{method}' wasn't found")),
    }
}

async fn page_method(
    id: &Value,
    session: &str,
    method: &str,
    params: &Value,
) -> Result<Out, String> {
    match method {
        "Page.getFrameTree" => Ok(Out::from_frames(vec![ok(
            id,
            Some(session),
            frame_tree(session).await?,
        )])),
        "Page.navigate" => navigate(id, session, params).await,
        "Page.reload" => reload(id, session).await,
        "Page.close" => {
            let target = {
                let hub = HUB.lock().await;
                hub.sessions.get(session).cloned()
            };
            let Some(target) = target else {
                return Err("no such session".to_string());
            };
            close_target(id, Some(session), &json!({ "targetId": target })).await
        }
        "Page.getLayoutMetrics" => Ok(Out::from_frames(vec![ok(
            id,
            Some(session),
            layout_metrics(session).await?,
        )])),
        "Page.captureScreenshot" => capture_screenshot(id, session, params).await,
        "Page.createIsolatedWorld" => create_world(id, session, params).await,
        "Page.addScriptToEvaluateOnNewDocument" => Ok(Out::from_frames(vec![ok(
            id,
            Some(session),
            json!({ "identifier": SCRIPT_IDS.fetch_add(1, Ordering::Relaxed).to_string() }),
        )])),
        "Runtime.enable" => runtime_enable(id, session).await,
        "Runtime.evaluate"
        | "Runtime.callFunctionOn"
        | "Runtime.releaseObject"
        | "Input.dispatchMouseEvent"
        | "Input.dispatchKeyEvent"
        | "Input.insertText"
        | "DOM.getContentQuads"
        | "DOM.scrollIntoViewIfNeeded" => runtime_forward(id, session, method, params).await,
        "Emulation.setDeviceMetricsOverride" => set_metrics(id, session, params).await,
        "Target.setAutoAttach" | "Target.detachFromTarget" => {
            Ok(Out::from_frames(vec![ok(id, Some(session), json!({}))]))
        }
        _ if ack_only(method) || page_ack(method) => {
            Ok(Out::from_frames(vec![ok(id, Some(session), json!({}))]))
        }
        _ => Err(format!("'{method}' wasn't found")),
    }
}

fn ack_only(method: &str) -> bool {
    let domain = method.split_once('.').map(|(d, _)| d).unwrap_or("");
    matches!(
        domain,
        "Emulation" | "Network" | "Fetch" | "Log" | "Inspector" | "Security"
    ) && method != "Emulation.setDeviceMetricsOverride"
}

fn page_ack(method: &str) -> bool {
    matches!(
        method,
        "Page.enable"
            | "Page.disable"
            | "Page.setLifecycleEventsEnabled"
            | "Page.setBypassCSP"
            | "Page.setFontFamilies"
            | "Page.bringToFront"
            | "Page.handleJavaScriptDialog"
            | "Page.removeScriptToEvaluateOnNewDocument"
            | "Page.setInterceptFileChooserDialog"
            | "Page.stopScreencast"
            | "Page.screencastFrameAck"
            | "Runtime.runIfWaitingForDebugger"
            | "Runtime.disable"
            | "DOM.enable"
            | "CSS.enable"
    )
}

async fn set_auto_attach(id: &Value, params: &Value) -> Result<Out, String> {
    let mut hub = HUB.lock().await;
    hub.auto_attach = params
        .get("autoAttach")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    hub.wait_debugger = params
        .get("waitForDebuggerOnStart")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut frames = Vec::new();
    if hub.auto_attach {
        let waiting = hub.wait_debugger;
        for page in hub.pages.values_mut() {
            if page.attached {
                continue;
            }
            page.attached = true;
            frames.push(attached_event(page, waiting));
        }
    }
    frames.push(ok(id, None, json!({})));
    Ok(Out::from_frames(frames))
}

async fn set_discover(id: &Value, params: &Value) -> Result<Out, String> {
    let mut hub = HUB.lock().await;
    hub.discover = params
        .get("discover")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut frames = Vec::new();
    if hub.discover {
        for page in hub.pages.values() {
            frames.push(event(
                None,
                "Target.targetCreated",
                json!({ "targetInfo": info_of(page, page.attached) }),
            ));
        }
    }
    frames.push(ok(id, None, json!({})));
    Ok(Out::from_frames(frames))
}

async fn target_info_result(params: &Value) -> Value {
    let hub = HUB.lock().await;
    if let Some(target) = params.get("targetId").and_then(|v| v.as_str()) {
        if let Some(page) = hub.pages.get(target) {
            return json!({ "targetInfo": info_of(page, page.attached) });
        }
    }
    json!({
        "targetInfo": {
            "targetId": browser_id(),
            "type": "browser",
            "title": "diting",
            "url": "",
            "attached": true,
            "canAccessOpener": false,
        }
    })
}

async fn all_targets() -> Value {
    let hub = HUB.lock().await;
    let mut infos = vec![json!({
        "targetId": browser_id(),
        "type": "browser",
        "title": "diting",
        "url": "",
        "attached": true,
        "canAccessOpener": false,
    })];
    for page in hub.pages.values() {
        infos.push(info_of(page, page.attached));
    }
    json!({ "targetInfos": infos })
}

async fn create_target(id: &Value, params: &Value) -> Result<Out, String> {
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("about:blank");
    let session_id = {
        let mut mgr = SESSIONS.lock().await;
        mgr.evict_expired();
        mgr.create(
            Some(url),
            false,
            Vec::new(),
            None,
            None,
            None,
            true,
            false,
            None,
        )
    };
    let nav = send_command(&session_id, |reply| SessionCommand::Navigate {
        url: url.to_string(),
        reply,
    })
    .await;
    let (url, title) = match nav {
        Ok(resp) => (resp.url, resp.title.unwrap_or_default()),
        Err(err) => {
            let mut mgr = SESSIONS.lock().await;
            mgr.close_and_wait(&session_id).await;
            return Err(session_msg(err));
        }
    };
    let target_id = uuid::Uuid::new_v4().simple().to_string();
    let cdp_session = uuid::Uuid::new_v4().simple().to_string();
    let mut page = PageSlot {
        target_id: target_id.clone(),
        session_id: session_id.clone(),
        cdp_session: cdp_session.clone(),
        frame_id: target_id.clone(),
        url,
        title,
        loader: 1,
        next_ctx: 1,
        runtime_on: false,
        attached: false,
        worlds: Vec::new(),
        viewport: (1280, 720),
    };
    let mut frames = Vec::new();
    {
        let mut hub = HUB.lock().await;
        if hub.discover {
            frames.push(event(
                None,
                "Target.targetCreated",
                json!({ "targetInfo": info_of(&page, false) }),
            ));
        }
        if hub.auto_attach {
            page.attached = true;
            frames.push(attached_event(&page, hub.wait_debugger));
        }
        hub.sessions.insert(cdp_session, target_id.clone());
        hub.pages.insert(target_id.clone(), page);
    }
    frames.push(ok(id, None, json!({ "targetId": target_id })));
    Ok(Out {
        frames,
        created: vec![session_id],
        closed: Vec::new(),
        close_socket: false,
    })
}

async fn attach_target(id: &Value, params: &Value) -> Result<Out, String> {
    let target = params
        .get("targetId")
        .and_then(|v| v.as_str())
        .ok_or("Target.attachToTarget: missing targetId")?;
    let mut hub = HUB.lock().await;
    let waiting = hub.wait_debugger;
    let page = hub
        .pages
        .get_mut(target)
        .ok_or_else(|| format!("no such target {target}"))?;
    let session = page.cdp_session.clone();
    let mut frames = Vec::new();
    if !page.attached {
        page.attached = true;
        frames.push(attached_event(page, waiting));
    }
    frames.push(ok(id, None, json!({ "sessionId": session })));
    Ok(Out::from_frames(frames))
}

async fn close_target(id: &Value, session: Option<&str>, params: &Value) -> Result<Out, String> {
    let target = params
        .get("targetId")
        .and_then(|v| v.as_str())
        .ok_or("Target.closeTarget: missing targetId")?
        .to_string();
    let removed = {
        let mut hub = HUB.lock().await;
        let page = hub.pages.remove(&target);
        if let Some(page) = page.as_ref() {
            hub.sessions.remove(&page.cdp_session);
        }
        page
    };
    let Some(page) = removed else {
        return Ok(Out::from_frames(vec![ok(
            id,
            session,
            json!({ "success": false }),
        )]));
    };
    let frames = vec![
        event(
            None,
            "Target.detachedFromTarget",
            json!({ "sessionId": page.cdp_session, "targetId": page.target_id }),
        ),
        event(
            None,
            "Target.targetDestroyed",
            json!({ "targetId": page.target_id }),
        ),
        ok(id, session, json!({ "success": true })),
    ];
    {
        let mut mgr = SESSIONS.lock().await;
        mgr.close_and_wait(&page.session_id).await;
    }
    Ok(Out {
        frames,
        created: Vec::new(),
        closed: vec![page.session_id],
        close_socket: false,
    })
}

async fn close_browser(id: &Value) -> Result<Out, String> {
    let pages: Vec<PageSlot> = {
        let mut hub = HUB.lock().await;
        hub.sessions.clear();
        hub.pages.drain().map(|(_, page)| page).collect()
    };
    let mut frames = Vec::new();
    let mut closed = Vec::new();
    for page in &pages {
        frames.push(event(
            None,
            "Target.detachedFromTarget",
            json!({ "sessionId": page.cdp_session, "targetId": page.target_id }),
        ));
        frames.push(event(
            None,
            "Target.targetDestroyed",
            json!({ "targetId": page.target_id }),
        ));
        closed.push(page.session_id.clone());
    }
    frames.push(ok(id, None, json!({})));
    {
        let mut mgr = SESSIONS.lock().await;
        for page in &pages {
            mgr.close_and_wait(&page.session_id).await;
        }
    }
    Ok(Out {
        frames,
        created: Vec::new(),
        closed,
        close_socket: true,
    })
}

async fn frame_tree(session: &str) -> Result<Value, String> {
    let hub = HUB.lock().await;
    let page = page_of(&hub, session)?;
    Ok(json!({ "frameTree": { "frame": frame_of(page) } }))
}

async fn navigate(id: &Value, session: &str, params: &Value) -> Result<Out, String> {
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("Page.navigate: missing url")?;
    let session_id = {
        let hub = HUB.lock().await;
        page_of(&hub, session)?.session_id.clone()
    };
    let nav = send_command(&session_id, |reply| SessionCommand::Navigate {
        url: url.to_string(),
        reply,
    })
    .await;
    let mut hub = HUB.lock().await;
    let page = page_of_mut(&mut hub, session)?;
    match nav {
        Ok(resp) => {
            page.url = resp.url;
            page.title = resp.title.unwrap_or_default();
            page.loader += 1;
            let loader = loader_of(page);
            let mut frames = nav_events(page, &loader);
            frames.push(ok(
                id,
                Some(session),
                json!({ "frameId": page.frame_id, "loaderId": loader }),
            ));
            Ok(Out::from_frames(frames))
        }
        Err(err) => {
            page.loader += 1;
            let loader = loader_of(page);
            let frame_id = page.frame_id.clone();
            Ok(Out::from_frames(vec![ok(
                id,
                Some(session),
                json!({
                    "frameId": frame_id,
                    "loaderId": loader,
                    "errorText": session_msg(err),
                }),
            )]))
        }
    }
}

async fn reload(id: &Value, session: &str) -> Result<Out, String> {
    let url = {
        let hub = HUB.lock().await;
        page_of(&hub, session)?.url.clone()
    };
    navigate(id, session, &json!({ "url": url })).await
}

fn nav_events(page: &mut PageSlot, loader: &str) -> Vec<String> {
    let session = page.cdp_session.clone();
    let frame_id = page.frame_id.clone();
    let mut frames = vec![event(
        Some(&session),
        "Page.frameNavigated",
        json!({ "frame": frame_of(page) }),
    )];
    frames.extend(refresh_worlds(page));
    frames.push(event(
        Some(&session),
        "Page.lifecycleEvent",
        json!({ "frameId": frame_id, "loaderId": loader, "name": "DOMContentLoaded" }),
    ));
    frames.push(event(
        Some(&session),
        "Page.domContentEventFired",
        json!({ "timestamp": 0 }),
    ));
    frames.push(event(
        Some(&session),
        "Page.lifecycleEvent",
        json!({ "frameId": frame_id, "loaderId": loader, "name": "load" }),
    ));
    frames.push(event(
        Some(&session),
        "Page.loadEventFired",
        json!({ "timestamp": 0 }),
    ));
    frames
}

fn refresh_worlds(page: &mut PageSlot) -> Vec<String> {
    if !page.runtime_on || page.worlds.is_empty() {
        return Vec::new();
    }
    let session = page.cdp_session.clone();
    let old: Vec<i64> = page.worlds.iter().map(|world| world.id).collect();
    let names: Vec<Option<String>> = page.worlds.iter().map(|world| world.name.clone()).collect();
    page.worlds.clear();
    let mut frames: Vec<String> = old
        .into_iter()
        .map(|ctx| {
            event(
                Some(&session),
                "Runtime.executionContextDestroyed",
                json!({ "executionContextId": ctx }),
            )
        })
        .collect();
    for name in names {
        let id = page.next_ctx;
        page.next_ctx += 1;
        page.worlds.push(World { id, name });
    }
    let created: Vec<World> = page
        .worlds
        .iter()
        .map(|world| World {
            id: world.id,
            name: world.name.clone(),
        })
        .collect();
    for world in &created {
        frames.push(context_event(page, world));
    }
    frames
}

async fn runtime_enable(id: &Value, session: &str) -> Result<Out, String> {
    let mut hub = HUB.lock().await;
    let page = page_of_mut(&mut hub, session)?;
    if !page.worlds.iter().any(|world| world.name.is_none()) {
        let ctx = page.next_ctx;
        page.next_ctx += 1;
        page.worlds.push(World {
            id: ctx,
            name: None,
        });
    }
    let first = !page.runtime_on;
    page.runtime_on = true;
    let mut frames = Vec::new();
    if first {
        let worlds: Vec<World> = page
            .worlds
            .iter()
            .map(|world| World {
                id: world.id,
                name: world.name.clone(),
            })
            .collect();
        for world in &worlds {
            frames.push(context_event(page, world));
        }
    }
    frames.push(ok(id, Some(session), json!({})));
    Ok(Out::from_frames(frames))
}

async fn create_world(id: &Value, session: &str, params: &Value) -> Result<Out, String> {
    let name = params
        .get("worldName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut hub = HUB.lock().await;
    let page = page_of_mut(&mut hub, session)?;
    let ctx = page.next_ctx;
    page.next_ctx += 1;
    page.worlds.push(World {
        id: ctx,
        name: Some(name),
    });
    let mut frames = Vec::new();
    if page.runtime_on {
        let world = World {
            id: ctx,
            name: page.worlds.last().and_then(|world| world.name.clone()),
        };
        frames.push(context_event(page, &world));
    }
    frames.push(ok(id, Some(session), json!({ "executionContextId": ctx })));
    Ok(Out::from_frames(frames))
}

async fn runtime_forward(
    id: &Value,
    session: &str,
    method: &str,
    params: &Value,
) -> Result<Out, String> {
    let session_id = {
        let hub = HUB.lock().await;
        page_of(&hub, session)?.session_id.clone()
    };
    let mut result = send_command(&session_id, |reply| SessionCommand::CdpRuntime {
        method: method.to_string(),
        params: params.clone(),
        reply,
    })
    .await
    .map_err(session_msg)?;
    let side = result
        .as_object_mut()
        .and_then(|obj| obj.remove(runtime::SIDE_KEY));
    let mut frames = Vec::new();
    if let Some(side) = side {
        let hub = HUB.lock().await;
        frames.extend(side_frames(session, &side, &hub));
    }
    frames.push(ok(id, Some(session), result));
    Ok(Out::from_frames(frames))
}

/// Console events first, then the document.write load. Playwright's
/// setContent clears lifecycle when the debug tag arrives and then waits
/// for `load`. A load that lands first is wiped by that clear.
fn side_frames(session: &str, side: &Value, hub: &Hub) -> Vec<String> {
    let console = side
        .get("console")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let write_nav = side
        .get("writeNav")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if console.is_empty() && !write_nav {
        return Vec::new();
    }
    let Ok(page) = page_of(hub, session) else {
        return Vec::new();
    };
    let asked = side
        .get("contextId")
        .and_then(|v| v.as_i64())
        .filter(|id| *id > 0);
    // The utility world is created after Playwright installs its console
    // listener. The main world may have been announced before that, so a
    // call with no context id is attributed to the latest isolated world.
    let context_id = asked.unwrap_or_else(|| {
        page.worlds
            .iter()
            .rev()
            .find(|world| world.name.is_some())
            .or_else(|| page.worlds.iter().find(|world| world.name.is_none()))
            .map(|world| world.id)
            .unwrap_or(1)
    });
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let mut frames = Vec::new();
    for call in console {
        let kind = call.get("type").and_then(|v| v.as_str()).unwrap_or("log");
        let text = call.get("text").and_then(|v| v.as_str()).unwrap_or("");
        frames.push(event(
            Some(session),
            "Runtime.consoleAPICalled",
            json!({
                "type": kind,
                "args": [{ "type": "string", "value": text }],
                "executionContextId": context_id,
                "timestamp": timestamp,
                "stackTrace": { "callFrames": [] },
            }),
        ));
    }
    if write_nav {
        let frame_id = page.frame_id.clone();
        let loader = loader_of(page);
        frames.push(event(
            Some(session),
            "Page.lifecycleEvent",
            json!({ "frameId": frame_id, "loaderId": loader, "name": "DOMContentLoaded" }),
        ));
        frames.push(event(
            Some(session),
            "Page.domContentEventFired",
            json!({ "timestamp": timestamp }),
        ));
        frames.push(event(
            Some(session),
            "Page.lifecycleEvent",
            json!({ "frameId": frame_id, "loaderId": loader, "name": "load" }),
        ));
        frames.push(event(
            Some(session),
            "Page.loadEventFired",
            json!({ "timestamp": timestamp }),
        ));
    }
    frames
}

async fn set_metrics(id: &Value, session: &str, params: &Value) -> Result<Out, String> {
    let width = params
        .get("width")
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0) as u32;
    let height = params
        .get("height")
        .and_then(|v| v.as_f64())
        .unwrap_or(720.0) as u32;
    let mobile = params
        .get("mobile")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let width = width.max(1);
    let height = height.max(1);
    let session_id = {
        let mut hub = HUB.lock().await;
        let page = page_of_mut(&mut hub, session)?;
        page.viewport = (width, height);
        page.session_id.clone()
    };
    let _ = send_command(&session_id, |reply| SessionCommand::Viewport {
        width: Some(width),
        height: Some(height),
        mobile,
        reply,
    })
    .await;
    Ok(Out::from_frames(vec![ok(id, Some(session), json!({}))]))
}

async fn layout_metrics(session: &str) -> Result<Value, String> {
    let hub = HUB.lock().await;
    let (w, h) = page_of(&hub, session)?.viewport;
    let w = w as f64;
    let h = h as f64;
    Ok(json!({
        "layoutViewport": { "pageX": 0, "pageY": 0, "clientWidth": w, "clientHeight": h },
        "visualViewport": {
            "offsetX": 0, "offsetY": 0, "pageX": 0, "pageY": 0,
            "clientWidth": w, "clientHeight": h, "scale": 1, "zoom": 1
        },
        "contentSize": { "x": 0, "y": 0, "width": w, "height": h },
        "cssLayoutViewport": { "pageX": 0, "pageY": 0, "clientWidth": w, "clientHeight": h },
        "cssContentSize": { "x": 0, "y": 0, "width": w, "height": h },
    }))
}

async fn capture_screenshot(id: &Value, session: &str, params: &Value) -> Result<Out, String> {
    let format = params
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("png");
    if format != "png" {
        return Err("only png screenshots are supported in this CDP slice".to_string());
    }
    let full_page = params
        .get("captureBeyondViewport")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let session_id = {
        let hub = HUB.lock().await;
        page_of(&hub, session)?.session_id.clone()
    };
    let shot = send_screenshot(&session_id, None, None, full_page, None, false, None)
        .await
        .map_err(session_msg)?;
    let body: Value = serde_json::from_str(&shot).map_err(|err| err.to_string())?;
    let data = body
        .get("image_base64")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(Out::from_frames(vec![ok(
        id,
        Some(session),
        json!({ "data": data }),
    )]))
}

fn page_of<'a>(hub: &'a Hub, session: &str) -> Result<&'a PageSlot, String> {
    let target = hub
        .sessions
        .get(session)
        .ok_or_else(|| format!("no such session {session}"))?;
    hub.pages
        .get(target)
        .ok_or_else(|| format!("no such target {target}"))
}

fn page_of_mut<'a>(hub: &'a mut Hub, session: &str) -> Result<&'a mut PageSlot, String> {
    let target = hub
        .sessions
        .get(session)
        .cloned()
        .ok_or_else(|| format!("no such session {session}"))?;
    hub.pages
        .get_mut(&target)
        .ok_or_else(|| format!("no such target {target}"))
}

fn loader_of(page: &PageSlot) -> String {
    format!("ldr{}", page.loader)
}

fn frame_of(page: &PageSlot) -> Value {
    json!({
        "id": page.frame_id,
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

fn info_of(page: &PageSlot, attached: bool) -> Value {
    json!({
        "targetId": page.target_id,
        "type": "page",
        "title": page.title,
        "url": page.url,
        "attached": attached,
        "canAccessOpener": false,
        "browserContextId": CONTEXT_ID,
    })
}

fn attached_event(page: &PageSlot, waiting: bool) -> String {
    event(
        None,
        "Target.attachedToTarget",
        json!({
            "sessionId": page.cdp_session,
            "waitingForDebugger": waiting,
            "targetInfo": info_of(page, true),
        }),
    )
}

fn context_event(page: &PageSlot, world: &World) -> String {
    let is_default = world.name.is_none();
    event(
        Some(&page.cdp_session),
        "Runtime.executionContextCreated",
        json!({
            "context": {
                "id": world.id,
                "origin": security_origin(&page.url),
                "name": world.name.clone().unwrap_or_default(),
                "uniqueId": world.id.to_string(),
                "auxData": {
                    "isDefault": is_default,
                    "type": if is_default { "default" } else { "isolated" },
                    "frameId": page.frame_id,
                }
            }
        }),
    )
}

fn ok(id: &Value, session: Option<&str>, result: Value) -> String {
    let mut body = json!({ "id": id, "result": result });
    if let Some(session) = session {
        body["sessionId"] = json!(session);
    }
    body.to_string()
}

fn fail(id: &Value, session: Option<&str>, message: String) -> String {
    let mut body = json!({
        "id": id,
        "error": { "code": -32000, "message": message }
    });
    if let Some(session) = session {
        body["sessionId"] = json!(session);
    }
    body.to_string()
}

fn event(session: Option<&str>, method: &str, params: Value) -> String {
    let mut body = json!({ "method": method, "params": params });
    if let Some(session) = session {
        body["sessionId"] = json!(session);
    }
    body.to_string()
}

fn session_msg(err: crate::session::SessionError) -> String {
    match err {
        crate::session::SessionError::NotFound(msg)
        | crate::session::SessionError::Expired(msg)
        | crate::session::SessionError::ThreadDied(msg)
        | crate::session::SessionError::Command(msg)
        | crate::session::SessionError::Eval(msg)
        | crate::session::SessionError::Dormant(msg) => msg,
    }
}
