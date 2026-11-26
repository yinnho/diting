//! The `Browser` and `Target` domains, and what a page's sessions hear
//! when it navigates or closes. Nothing here runs on the page but
//! `Page.navigate`: the rest is bookkeeping in the hub, under its lock and
//! never across an await.
//!
//! - A session id is minted once per attach. Two `Target.attachToTarget`
//!   on one page are two sessions, each with its own `Page.enable` and
//!   `Runtime.enable`; detaching one leaves the other running.
//! - `Target.setAutoAttach` is a switch on the socket: on, it attaches
//!   every page the socket owns; off, it takes back the sessions it made.
//! - A navigation is announced to the sessions that asked for the domain:
//!   `Page.enable` for the navigation, `Runtime.enable` for the contexts,
//!   `Page.setLifecycleEventsEnabled` for the lifecycle.

use serde_json::{json, Value};

use super::params::{
    boolean, integer, invalid, kinds, optional_boolean, optional_string, string, Kind,
};
use super::runtime::engine_failure;
use super::{
    browser_id, cdp_user_agent, context_of, deliver, event, frame_of, info_of, lifecycle,
    loader_of, local_of, new_id, now_secs, ok, page_of, page_of_mut, product, session_msg,
    to_sessions, Attached, Failure, Hub, Out, PageSlot, SessionSlot, SharedHub, World, CONTEXT_ID,
};
use crate::session::{send_command, SessionCommand, SESSIONS};

pub(super) async fn browser_method(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    method: &str,
    params: &Value,
) -> Result<Out, Failure> {
    match method {
        "Browser.getVersion" => Ok(Out::from_frames(vec![ok(
            id,
            session,
            json!({
                "protocolVersion": "1.3",
                "product": product(),
                "revision": "0",
                "userAgent": cdp_user_agent(),
                "jsVersion": "diting",
            }),
        )])),
        "Browser.setDownloadBehavior" => download_behavior(id, session, params),
        "Browser.close" => close_browser(hub.clone(), client, id, session).await,
        "Target.setAutoAttach" => set_auto_attach(hub.clone(), client, id, session, params).await,
        "Target.setDiscoverTargets" => set_discover(hub.clone(), client, id, session, params).await,
        "Target.getTargetInfo" => {
            let hub = hub.lock().await;
            let info = target_info(&hub, client, session, params)?;
            Ok(Out::from_frames(vec![ok(
                id,
                session,
                json!({ "targetInfo": info }),
            )]))
        }
        "Target.getTargets" => {
            let hub = hub.lock().await;
            let infos = all_targets(&hub, client, session);
            Ok(Out::from_frames(vec![ok(
                id,
                session,
                json!({ "targetInfos": infos }),
            )]))
        }
        "Target.createTarget" => create_target(hub.clone(), client, id, session, params).await,
        "Target.getBrowserContexts" => {
            let hub = hub.lock().await;
            let mut ids: Vec<&String> = hub.contexts.iter().collect();
            ids.sort();
            Ok(Out::from_frames(vec![ok(
                id,
                session,
                json!({
                    "browserContextIds": ids,
                    "defaultBrowserContextId": CONTEXT_ID,
                }),
            )]))
        }
        "Target.createBrowserContext" => {
            let mut hub = hub.lock().await;
            let context = new_id().to_uppercase();
            hub.contexts.insert(context.clone());
            Ok(Out::from_frames(vec![ok(
                id,
                session,
                json!({ "browserContextId": context }),
            )]))
        }
        "Target.disposeBrowserContext" => {
            let context = string(params, "browserContextId")?;
            let mut hub = hub.lock().await;
            if !hub.contexts.remove(context) {
                // Chrome 158 (`bctx-probe.cjs`): the wording differs from
                // createTarget's — no "browser" in it.
                return Err(Failure::from(format!(
                    "Failed to find context with id {context}"
                )));
            }
            Ok(Out::from_frames(vec![ok(id, session, json!({}))]))
        }
        "Target.attachToTarget" => attach_target(hub.clone(), client, id, session, params).await,
        "Target.attachToBrowserTarget" => attach_browser(hub.clone(), client, id, session).await,
        "Target.detachFromTarget" => detach(hub.clone(), client, id, session, params).await,
        "Target.closeTarget" => {
            let target = string(params, "targetId")?;
            close_target(
                hub.clone(),
                client,
                id,
                session,
                target,
                json!({ "success": true }),
            )
            .await
        }
        _ => Err(Failure::NotFound(method.to_string())),
    }
}

/// Close engine sessions — outside the hub lock, and before the reply, so
/// a client that has the reply finds them closed.
pub(super) async fn close_engine(_hub: SharedHub, sessions: Vec<String>) {
    if sessions.is_empty() {
        return;
    }
    let mut mgr = SESSIONS.lock().await;
    for session in sessions {
        mgr.close_and_wait(&session).await;
    }
}

/// The engine has no page downloads, and Playwright cannot be talked out of
/// asking: every `connectOverCDP` sends `allowAndName` with a path. So the
/// validation is Chrome's and nothing is kept — no file is ever saved and
/// no `Browser.download*` event ever follows. A navigation that would
/// download is not told apart either: `Page.navigate` answers
/// `isDownload: false` whatever the response says.
fn download_behavior(id: &Value, session: Option<&str>, params: &Value) -> Result<Out, Failure> {
    // Every field is read before any value is looked at, as Chrome reads them.
    let behavior = string(params, "behavior")?;
    let path = optional_string(params, "downloadPath")?;
    kinds(params, &[("eventsEnabled", Kind::Bool)])?;
    if let Some(context) = optional_string(params, "browserContextId")? {
        // No browser context here for an id to name — and Chrome says so
        // before it speaks of a path that was not sent.
        return Err(invalid(&format!(
            "Failed to find browser context for id {context}"
        )));
    }
    match (behavior, path) {
        ("allow" | "allowAndName", None) => Err(invalid("downloadPath not provided")),
        // A `behavior` Chrome does not know is taken and changes nothing there.
        _ => Ok(Out::from_frames(vec![ok(id, session, json!({}))])),
    }
}

/// `Target.setAutoAttach` at the browser level — a switch on the socket.
/// On: every page the socket owns gets a session, unless one made the same
/// way is still live. Off: the sessions it made are taken back.
async fn set_auto_attach(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    params: &Value,
) -> Result<Out, Failure> {
    let auto_attach = boolean(params, "autoAttach")?;
    let waiting = boolean(params, "waitForDebuggerOnStart")?;
    if optional_boolean(params, "flatten")? == Some(false) {
        return Err(invalid(
            "Only flatten protocol is supported with browser level auto-attach",
        ));
    }
    let mut hub = hub.lock().await;
    if let Some(slot) = hub.clients.get_mut(&client) {
        slot.auto_attach = auto_attach;
        slot.wait_debugger = waiting;
    }
    let mut out = Out::default();
    if auto_attach {
        let fresh: Vec<String> = hub
            .pages
            .values()
            .filter(|page| {
                page.owner == client
                    && !hub
                        .sessions
                        .values()
                        .any(|slot| slot.auto && slot.on(&page.target_id))
            })
            .map(|page| page.target_id.clone())
            .collect();
        for target in fresh {
            hub.attach_page(&target, client, session, true, waiting, &mut out)?;
        }
    } else {
        let made: Vec<String> = hub
            .sessions
            .values()
            .filter(|slot| slot.client == client && slot.auto)
            .map(|slot| slot.id.clone())
            .collect();
        for session in made {
            hub.detach(&session, client, &mut out);
        }
    }
    out.frames.push(ok(id, session, json!({})));
    Ok(out)
}

/// `Target.setAutoAttach` at a page level. Chrome attaches child targets —
/// iframes, workers — this engine has none of: a frame is part of the
/// page, not a target. The parameters are read the same way.
pub(super) fn page_auto_attach(id: &Value, session: &str, params: &Value) -> Result<Out, Failure> {
    boolean(params, "autoAttach")?;
    boolean(params, "waitForDebuggerOnStart")?;
    optional_boolean(params, "flatten")?;
    Ok(super::ack(id, session))
}

/// `Target.setDiscoverTargets`: on, the socket hears `Target.targetCreated`
/// for the pages it owns — the ones there are, then each new one.
async fn set_discover(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    params: &Value,
) -> Result<Out, Failure> {
    let discover = boolean(params, "discover")?;
    let mut hub = hub.lock().await;
    if let Some(slot) = hub.clients.get_mut(&client) {
        slot.discover = discover;
    }
    let mut out = Out::default();
    if discover {
        let created: Vec<String> = hub
            .pages
            .values()
            .filter(|page| page.owner == client)
            .map(|page| {
                event(
                    session,
                    "Target.targetCreated",
                    json!({ "targetInfo": info_of(&hub, page) }),
                )
            })
            .collect();
        out.frames.extend(created);
    }
    out.frames.push(ok(id, session, json!({})));
    Ok(out)
}

/// The target `Target.getTargetInfo` names, or the one the asking level is on.
fn target_info(
    hub: &Hub,
    client: u64,
    session: Option<&str>,
    params: &Value,
) -> Result<Value, Failure> {
    if let Some(target) = optional_string(params, "targetId")? {
        return match hub.pages.get(target) {
            Some(page) if hub.owns(client, session, target) => Ok(info_of(hub, page)),
            _ => Err(invalid("No target with given id found")),
        };
    }
    Ok(
        match session.and_then(|session| hub.sessions.get(session)) {
            Some(slot) => match &slot.attached {
                Attached::Page(target) => {
                    let page = hub
                        .pages
                        .get(target)
                        .ok_or_else(|| format!("no such target {target}"))?;
                    info_of(hub, page)
                }
                Attached::Browser(id) => browser_info(id),
            },
            None => browser_info(browser_id()),
        },
    )
}

/// `Target.getTargets`: the browser and the pages this socket may name.
/// Asked from a page, the browser is not on the list, as in Chrome.
fn all_targets(hub: &Hub, client: u64, session: Option<&str>) -> Vec<Value> {
    let own = session.and_then(|session| hub.sessions.get(session));
    let mut infos = Vec::new();
    if !own.is_some_and(|slot| matches!(slot.attached, Attached::Page(_))) {
        let id = own.map_or_else(
            || browser_id().to_string(),
            |slot| slot.attached.id().to_string(),
        );
        infos.push(browser_info(&id));
    }
    for page in hub
        .pages
        .values()
        .filter(|page| hub.owns(client, session, &page.target_id))
    {
        infos.push(info_of(hub, page));
    }
    infos
}

/// `Target.createTarget`: an engine session with the page loaded, before
/// the reply — the client's first command on the new target finds a page.
async fn create_target(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    params: &Value,
) -> Result<Out, Failure> {
    // Every field is read before any value is looked at, as Chrome reads them.
    let url = string(params, "url")?;
    kinds(
        params,
        &[
            ("width", Kind::Integer),
            ("height", Kind::Integer),
            ("left", Kind::Integer),
            ("browserContextId", Kind::Text),
            ("newWindow", Kind::Bool),
            ("background", Kind::Bool),
            ("forTab", Kind::Bool),
        ],
    )?;
    let context = optional_string(params, "browserContextId")?.unwrap_or(CONTEXT_ID);
    {
        let hub = hub.lock().await;
        if context != CONTEXT_ID && !hub.contexts.contains(context) {
            // Chrome 158 (`bctx2-probe.cjs`): the refusal is a plain
            // protocol error, spelt exactly so.
            return Err(Failure::from(format!(
                "Failed to find browser context with id {context}"
            )));
        }
    }
    let url = if url.is_empty() { "about:blank" } else { url };
    let engine = {
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
    let nav = send_command(&engine, |reply| SessionCommand::Navigate {
        url: url.to_string(),
        reply,
    })
    .await;
    let (url, title) = match nav {
        Ok(resp) => (resp.url, resp.title.unwrap_or_default()),
        Err(err) => {
            let mut mgr = SESSIONS.lock().await;
            mgr.close_and_wait(&engine).await;
            return Err(session_msg(err).into());
        }
    };
    let page = PageSlot {
        owner: client,
        target_id: new_id(),
        engine_session: engine,
        context: context.to_string(),
        url,
        title,
        loader: 1,
        doc_rows: std::collections::HashMap::new(),
        // The main world is there from the start, as in Chrome: a context
        // id names it before any session turns `Runtime` on.
        next_ctx: 2,
        dom_next: 1,
        worlds: vec![World { id: 1, name: None }],
    };
    super::push::follow(hub.clone(), page.target_id.clone(), &page.engine_session).await;
    let mut out = Out::default();
    let mut hub = hub.lock().await;
    let (discover, auto_attach, waiting) = hub.flags(client);
    if discover {
        out.frames.push(event(
            session,
            "Target.targetCreated",
            json!({ "targetInfo": info_of(&hub, &page) }),
        ));
    }
    let target_id = page.target_id.clone();
    hub.pages.insert(target_id.clone(), page);
    if auto_attach {
        hub.attach_page(&target_id, client, session, true, waiting, &mut out)?;
    }
    out.frames
        .push(ok(id, session, json!({ "targetId": target_id })));
    Ok(out)
}

/// `Target.attachToTarget`: a new session every time, announced on the
/// level it was asked from, before the reply.
async fn attach_target(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    params: &Value,
) -> Result<Out, Failure> {
    const METHOD: &str = "Target.attachToTarget";
    let target = string(params, "targetId")?;
    if optional_boolean(params, "flatten")? == Some(false) {
        // Chrome would then answer through `Target.receivedMessageFromTarget`
        // on the calling session. Nothing here speaks that.
        return Err(format!("{METHOD}: only the flatten protocol is supported").into());
    }
    let mut hub = hub.lock().await;
    let mut out = Out::default();
    let new = hub.attach_page(target, client, session, false, false, &mut out)?;
    out.frames
        .push(ok(id, session, json!({ "sessionId": new })));
    Ok(out)
}

/// `Target.attachToBrowserTarget`: a session on the browser, for browser
/// domains from under a session. Chrome gives each one a target id of its
/// own.
async fn attach_browser(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
) -> Result<Out, Failure> {
    let mut hub = hub.lock().await;
    let level = hub.level(session);
    let slot = SessionSlot::new(client, Attached::Browser(new_id()), session, false, false);
    let mut out = Out::default();
    out.frames.push(event(
        level.as_deref(),
        "Target.attachedToTarget",
        json!({
            "sessionId": slot.id,
            "waitingForDebugger": false,
            "targetInfo": browser_info(slot.attached.id()),
        }),
    ));
    out.frames
        .push(ok(id, session, json!({ "sessionId": slot.id })));
    hub.sessions.insert(slot.id.clone(), slot);
    Ok(out)
}

/// `Target.detachFromTarget` at any level. A session is named, or a target —
/// which means the one session this level has on it. Only the sessions
/// attached on the asking level are on the table: the root's at the root, a
/// session's own under it. A session cannot take itself, and a page
/// websocket's root level is never taken.
async fn detach(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    params: &Value,
) -> Result<Out, Failure> {
    const METHOD: &str = "Target.detachFromTarget";
    let mut hub = hub.lock().await;
    let level = hub.level(session);
    // This socket's sessions, attached on the asking level, on `target`.
    let on_table = |target: Option<&str>| {
        hub.sessions
            .values()
            .filter(|slot| {
                slot.client == client
                    && !slot.home
                    && hub.level(slot.parent.as_deref()) == level
                    && target.is_none_or(|target| slot.on(target))
            })
            .map(|slot| slot.id.clone())
            .collect::<Vec<String>>()
    };
    let named = match (
        optional_string(params, "sessionId")?,
        optional_string(params, "targetId")?,
    ) {
        (Some(named), _) => {
            if !on_table(None).iter().any(|id| id == named) {
                return Err(invalid("No session with given id"));
            }
            named.to_string()
        }
        (None, Some(target)) => match on_table(Some(target)).as_slice() {
            [one] => one.clone(),
            [] => return Err(invalid("No session for given target id")),
            // Chrome's words: which one is not this command's to guess.
            _ => return Err(format!("{METHOD}: Multiple sessions attached, specify id.").into()),
        },
        (None, None) => return Err(invalid("Session id must be specified")),
    };
    let mut out = Out::default();
    hub.detach(&named, client, &mut out);
    out.frames.push(ok(id, session, json!({})));
    Ok(out)
}

/// `Target.closeTarget`, and `Page.close` for the session's own page. The
/// page goes, every session on it with it.
async fn close_target(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
    target: &str,
    reply: Value,
) -> Result<Out, Failure> {
    let mut out = Out::default();
    let engine = {
        let mut hub = hub.lock().await;
        if !hub.owns(client, session, target) {
            return Err(invalid("No target with given id found"));
        }
        // A page websocket on the page it closed is closed too.
        out.close_socket = session.is_some_and(|session| {
            hub.sessions
                .get(session)
                .is_some_and(|slot| slot.home && slot.on(target))
        });
        hub.close_pages(&[target.to_string()], client, &mut out)
    };
    close_engine(hub, engine).await;
    out.frames.push(ok(id, session, reply));
    Ok(out)
}

/// `Page.close`: the page the session is on.
pub(super) async fn close_page(hub: SharedHub, id: &Value, session: &str) -> Result<Out, Failure> {
    let (client, target) = {
        let hub = hub.lock().await;
        let slot = hub
            .sessions
            .get(session)
            .ok_or_else(|| no_session(session))?;
        (slot.client, slot.attached.id().to_string())
    };
    close_target(hub.clone(), client, id, Some(session), &target, json!({})).await
}

/// `Browser.close`: this socket's pages, then the socket. Chrome would
/// close the whole browser; the pages of other sockets are theirs to close.
async fn close_browser(
    hub: SharedHub,
    client: u64,
    id: &Value,
    session: Option<&str>,
) -> Result<Out, Failure> {
    let mut out = Out::default();
    let engine = {
        let mut hub = hub.lock().await;
        let owned = hub.owned(client);
        hub.close_pages(&owned, client, &mut out)
    };
    close_engine(hub, engine).await;
    out.frames.push(ok(id, session, json!({})));
    out.close_socket = true;
    Ok(out)
}

pub(super) fn frame_tree(hub: &Hub, session: &str) -> Result<Value, String> {
    let page = page_of(hub, session)?;
    Ok(json!({ "frameTree": { "frame": frame_of(page) } }))
}

/// `Page.navigate`. The page is loaded before the reply — as a navigation
/// that has committed is in Chrome (`about:blank`, `data:`), and the
/// sessions that asked for `Page` events hear the whole round first. A URL
/// that does not stand on its own is a protocol error before the page is
/// touched: Chrome resolves nothing against the page's URL either.
pub(super) async fn navigate(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    // Every field is read before the page is asked to go anywhere.
    let url = string(params, "url")?;
    kinds(
        params,
        &[
            ("referrer", Kind::Text),
            ("transitionType", Kind::Text),
            ("frameId", Kind::Text),
        ],
    )?;
    if url::Url::parse(url.trim()).is_err() {
        return Err(Failure::from("Cannot navigate to invalid URL"));
    }
    let (mut out, reply) = load(hub, session, url).await?;
    out.frames.push(ok(id, Some(session), reply));
    Ok(out)
}

/// `Page.reload`. Chrome's reply is empty, whatever came of the reload; the
/// events and the page are the reload's own.
pub(super) async fn reload(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    kinds(params, &[("ignoreCache", Kind::Bool)])?;
    let url = {
        let hub = hub.lock().await;
        page_of(&hub, session)?.url.clone()
    };
    let (mut out, _) = load(hub, session, &url).await?;
    out.frames.push(ok(id, Some(session), json!({})));
    Ok(out)
}

/// Load `url` in the page and say how it went: Chrome's reply — the frame,
/// `isDownload`, and the loader once the page is the new document — with
/// `errorText` when it is not. Chrome's `errorText` is a `net::` error code;
/// the engine's navigation failures are spelled out and not sorted into
/// kinds, so the reply says what the engine said.
async fn load(hub: SharedHub, session: &str, url: &str) -> Result<(Out, Value), Failure> {
    let engine = {
        let hub = hub.lock().await;
        page_of(&hub, session)?.engine_session.clone()
    };
    let (nav, rows, failure, calls) =
        match send_command(&engine, |reply| SessionCommand::NavigateCdp {
            url: url.to_string(),
            reply,
        })
        .await
        {
            Ok(answer) => answer,
            Err(err) => (
                Err(session_msg(err)),
                Vec::new(),
                diting::diting_net::NetFailure::Other,
                Vec::new(),
            ),
        };
    let mut hub = hub.lock().await;
    let local = local_of(&hub, session);
    {
        let page = page_of_mut(&mut hub, session)?;
        page.loader += 1;
        if let Ok(resp) = &nav {
            page.url = resp.url.clone();
            page.title = resp.title.clone().unwrap_or_default();
            // A navigation that did not commit keeps the old document —
            // its state, its realm and so its execution contexts, which a
            // renewal here orphaned while the realm lived on (measured:
            // `evaluate` with a context id died "Cannot find context"
            // after any failed navigation).
            renew_worlds(page);
        }
        super::remember_doc_rows(page, &rows);
    }
    let page = page_of(&hub, session)?;
    let mut out = Out::default();
    // The requests come before the navigation: Playwright looks for the
    // document's request when `frameNavigated` arrives.
    super::network_events::network_frames(&hub, local, page, &rows, &mut out);
    let reply = match &nav {
        Ok(_) => {
            nav_events(&hub, page, local, &mut out);
            super::page_side::navigation_console_frames(&hub, local, page, &calls, &mut out);
            json!({
                "frameId": page.target_id,
                "loaderId": loader_of(page),
                "isDownload": false,
            })
        }
        Err(_) => json!({
            "frameId": page.target_id,
            "errorText": super::network_events::navigation_error(&failure),
            "isDownload": false,
            "loaderId": loader_of(page),
        }),
    };
    Ok((out, reply))
}

/// `Page.getNavigationHistory`: the entries in visit order and where the
/// cursor is. Chrome numbers the entries with opaque ids; ours are the
/// positions, which is all a client needs to come back.
pub(super) async fn navigation_history(hub: SharedHub, session: &str) -> Result<Value, Failure> {
    let engine = {
        let hub = hub.lock().await;
        page_of(&hub, session)?.engine_session.clone()
    };
    let (entries, index) =
        send_command(&engine, |reply| SessionCommand::NavigationHistory { reply })
            .await
            .map_err(|err| engine_failure(session_msg(err)))?;
    let entries: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(position, url)| {
            json!({
                "id": position,
                "url": url,
                "userTypedURL": url,
                "title": "",
                "transitionType": "link",
            })
        })
        .collect();
    Ok(json!({ "currentIndex": index, "entries": entries }))
}

/// `Page.navigateToHistoryEntry`. Chrome answers empty, whatever came of
/// the jump; the events and the page are the jump's own.
pub(super) async fn navigate_to_entry(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    let entry = integer(params, "entryId")?;
    if entry < 0 {
        return Err(invalid("No entry with passed id"));
    }
    let engine = {
        let hub = hub.lock().await;
        page_of(&hub, session)?.engine_session.clone()
    };
    let index = entry as usize;
    let (nav, rows, failure, calls) = match send_command(&engine, |reply| {
        SessionCommand::NavigateHistoryCdp { index, reply }
    })
    .await
    {
        Ok(answer) => answer,
        Err(err) => (
            Err(session_msg(err)),
            Vec::new(),
            diting::diting_net::NetFailure::Other,
            Vec::new(),
        ),
    };
    let mut hub = hub.lock().await;
    let local = local_of(&hub, session);
    {
        let page = page_of_mut(&mut hub, session)?;
        page.loader += 1;
        if let Ok(resp) = &nav {
            page.url = resp.url.clone();
            page.title = resp.title.clone().unwrap_or_default();
            // A navigation that did not commit keeps the old document —
            // its state, its realm and so its execution contexts, which a
            // renewal here orphaned while the realm lived on (measured:
            // `evaluate` with a context id died "Cannot find context"
            // after any failed navigation).
            renew_worlds(page);
        }
        super::remember_doc_rows(page, &rows);
    }
    let page = page_of(&hub, session)?;
    let mut out = Out::default();
    super::network_events::network_frames(&hub, local, page, &rows, &mut out);
    if nav.is_ok() {
        nav_events(&hub, page, local, &mut out);
        super::page_side::navigation_console_frames(&hub, local, page, &calls, &mut out);
    }
    let _ = failure;
    out.frames.push(ok(id, Some(session), json!({})));
    Ok(out)
}

/// A move inside the document, as Chrome 158 tells it (`spa-probe.cjs`):
/// loading starts, the frame says where it now is and how, loading stops.
/// There is no new document, so no new loader and no new contexts.
pub(super) fn same_document_events(
    hub: &Hub,
    page: &PageSlot,
    url: &str,
    kind: crate::session::SameDocumentKind,
    out: &mut Out,
) {
    let frame_id = page.target_id.as_str();
    let page_on = |slot: &SessionSlot| slot.page_on;
    let navigation_type = match kind {
        crate::session::SameDocumentKind::Fragment => "fragment",
        crate::session::SameDocumentKind::History => "historyApi",
    };
    to_sessions(
        hub,
        out,
        0,
        frame_id,
        page_on,
        "Page.frameStartedLoading",
        || json!({ "frameId": frame_id }),
    );
    to_sessions(
        hub,
        out,
        0,
        frame_id,
        page_on,
        "Page.navigatedWithinDocument",
        || json!({ "frameId": frame_id, "url": url, "navigationType": navigation_type }),
    );
    to_sessions(
        hub,
        out,
        0,
        frame_id,
        page_on,
        "Page.frameStoppedLoading",
        || json!({ "frameId": frame_id }),
    );
}

/// One navigation as Chrome reports it, each event to the sessions of the
/// page that run its domain. `Page.frameNavigated` comes before the
/// lifecycle events of the new document: Playwright clears them there.
/// Chrome 158 clears the contexts twice around a commit — once when the
/// navigation starts, once before the new document is announced — and
/// clients take the event as "none are left", so twice says no more than
/// once.
pub(super) fn nav_events(hub: &Hub, page: &PageSlot, local: u64, out: &mut Out) {
    let frame_id = page.target_id.as_str();
    let loader = loader_of(page);
    let now = now_secs();
    let page_on = |slot: &SessionSlot| slot.page_on;
    let runtime_on = |slot: &SessionSlot| slot.runtime_on;
    // A lifecycle event is the Page domain's: the switch only picks among
    // its events for a session that runs the domain.
    let lifecycle_on = |slot: &SessionSlot| slot.page_on && slot.lifecycle_on;
    let lifecycle_of = |name: &str| lifecycle(frame_id, &loader, name, now);
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        page_on,
        "Page.frameStartedNavigating",
        || {
            json!({
                "frameId": frame_id,
                "url": page.url,
                "loaderId": loader,
                "navigationType": "differentDocument",
            })
        },
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        page_on,
        "Page.frameStartedLoading",
        || json!({ "frameId": frame_id }),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        runtime_on,
        "Runtime.executionContextsCleared",
        || json!({}),
    );
    // Chrome 158 sends `init` between the two clears, and it is the one
    // lifecycle event a client cannot do without: Puppeteer takes the
    // frame's loader id from it, and without the new loader no navigation
    // of its ever finishes.
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        lifecycle_on,
        "Page.lifecycleEvent",
        || lifecycle_of("init"),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        runtime_on,
        "Runtime.executionContextsCleared",
        || json!({}),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        page_on,
        "Page.frameNavigated",
        || json!({ "frame": frame_of(page), "type": "Navigation" }),
    );
    for world in &page.worlds {
        to_sessions(
            hub,
            out,
            local,
            frame_id,
            runtime_on,
            "Runtime.executionContextCreated",
            || context_of(page, world),
        );
    }
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        page_on,
        "Page.domContentEventFired",
        || json!({ "timestamp": now }),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        lifecycle_on,
        "Page.lifecycleEvent",
        || lifecycle_of("DOMContentLoaded"),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        page_on,
        "Page.loadEventFired",
        || json!({ "timestamp": now }),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        lifecycle_on,
        "Page.lifecycleEvent",
        || lifecycle_of("load"),
    );
    to_sessions(
        hub,
        out,
        local,
        frame_id,
        page_on,
        "Page.frameStoppedLoading",
        || json!({ "frameId": frame_id }),
    );
    if hub.discovering(page.owner) {
        deliver(
            hub,
            out,
            local,
            (page.owner, None),
            "Target.targetInfoChanged",
            json!({ "targetInfo": info_of(hub, page) }),
        );
    }
}

/// `Page.enable`, `Page.disable` and `Page.setLifecycleEventsEnabled`: a
/// switch on this session alone.
pub(super) async fn page_switch(
    hub: SharedHub,
    id: &Value,
    session: &str,
    method: &str,
    params: &Value,
) -> Result<Out, Failure> {
    // `Page.setLifecycleEventsEnabled` reads its switch before anything moves.
    let lifecycle = (method == "Page.setLifecycleEventsEnabled")
        .then(|| boolean(params, "enabled"))
        .transpose()?;
    let mut hub = hub.lock().await;
    {
        let slot = hub
            .sessions
            .get_mut(session)
            .ok_or_else(|| no_session(session))?;
        match method {
            "Page.enable" => slot.page_on = true,
            "Page.disable" => slot.page_on = false,
            _ => slot.lifecycle_on = lifecycle.unwrap_or(false),
        }
    }
    Ok(super::ack(id, session))
}

/// `Runtime.enable`: the page's worlds are announced to this session —
/// each time it is enabled after a disable, never on a repeat.
pub(super) async fn runtime_enable(
    hub: SharedHub,
    id: &Value,
    session: &str,
) -> Result<Out, Failure> {
    let mut hub = hub.lock().await;
    let slot = hub
        .sessions
        .get_mut(session)
        .ok_or_else(|| no_session(session))?;
    // Chrome hears nothing from a repeat: the worlds are announced when
    // the domain turns on.
    let turned_on = !slot.runtime_on;
    slot.runtime_on = true;
    let page = page_of(&hub, session)?;
    let worlds = if turned_on {
        page.worlds
            .iter()
            .map(|world| {
                event(
                    Some(session),
                    "Runtime.executionContextCreated",
                    context_of(page, world),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut out = Out::from_frames(worlds);
    out.frames.push(ok(id, Some(session), json!({})));
    Ok(out)
}

pub(super) async fn runtime_disable(
    hub: SharedHub,
    id: &Value,
    session: &str,
) -> Result<Out, Failure> {
    let mut hub = hub.lock().await;
    hub.sessions
        .get_mut(session)
        .ok_or_else(|| no_session(session))?
        .runtime_on = false;
    Ok(super::ack(id, session))
}

/// `Page.createIsolatedWorld`. The engine runs one realm: the world is a
/// name and a context id, which is what Playwright needs of it.
pub(super) async fn create_world(
    hub: SharedHub,
    id: &Value,
    session: &str,
    params: &Value,
) -> Result<Out, Failure> {
    // Every field is read before any value is looked at, as Chrome reads them.
    let frame = string(params, "frameId")?;
    let name = optional_string(params, "worldName")?
        .unwrap_or("")
        .to_string();
    kinds(params, &[("grantUniveralAccess", Kind::Bool)])?;
    let mut hub = hub.lock().await;
    let local = local_of(&hub, session);
    let ctx = {
        let page = page_of_mut(&mut hub, session)?;
        // The page is one frame, and it is the one `Page.getFrameTree` names.
        if page.target_id != frame {
            return Err(invalid("No frame for given id found"));
        }
        let ctx = page.next_ctx;
        page.next_ctx += 1;
        page.worlds.push(World {
            id: ctx,
            name: Some(name),
        });
        ctx
    };
    let mut out = Out::default();
    {
        let page = page_of(&hub, session)?;
        let created = page.worlds.last().map(|world| context_of(page, world));
        if let Some(params) = created {
            to_sessions(
                &hub,
                &mut out,
                local,
                &page.target_id,
                |slot| slot.runtime_on,
                "Runtime.executionContextCreated",
                || params.clone(),
            );
        }
    }
    out.frames
        .push(ok(id, Some(session), json!({ "executionContextId": ctx })));
    Ok(out)
}

/// New context ids for the worlds after a navigation, as Chrome mints them.
/// The names stay.
pub(super) fn renew_worlds(page: &mut PageSlot) {
    for world in &mut page.worlds {
        world.id = page.next_ctx;
        page.next_ctx += 1;
    }
}

fn browser_info(target: &str) -> Value {
    json!({
        "targetId": target,
        "type": "browser",
        "title": "",
        "url": "",
        "attached": true,
        "canAccessOpener": false,
    })
}

/// The session was there when the command arrived and is gone now: its
/// page or socket closed while the command ran.
fn no_session(session: &str) -> String {
    format!("no such session {session}")
}
