//! Sessions: what each one hears, how they are attached and taken back,
//! whom they answer to, whose page is whose — and a page websocket, which
//! is a page's root level.

use super::*;

/// The methods of `events`, in order.
fn methods(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["method"].as_str().unwrap_or(""))
        .collect()
}

/// The session a `Target.attachedToTarget` minted.
fn minted(event: &Value) -> &str {
    assert_eq!(event["method"], "Target.attachedToTarget", "{event}");
    event["params"]["sessionId"].as_str().unwrap()
}

#[tokio::test]
async fn each_session_hears_only_what_it_enabled() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    let (target, quiet) = owned_page(&mut client, 1).await;
    let loud = client
        .run("Target.attachToTarget", json!({ "targetId": target }), None)
        .await["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    for (method, params) in [
        ("Page.enable", json!({})),
        ("Page.setLifecycleEventsEnabled", json!({ "enabled": true })),
        ("Runtime.enable", json!({})),
    ] {
        client.run(method, params, Some(&loud)).await;
    }
    let world_of = |events: &[Value], session: &str| {
        events
            .iter()
            .filter(|event| {
                event["method"] == "Runtime.executionContextCreated"
                    && event["sessionId"] == session
            })
            .map(|event| event["params"]["context"]["id"].as_i64().unwrap())
            .collect::<Vec<i64>>()
    };

    // The quiet session navigates, and hears nothing of it.
    let (_, heard) = client
        .with_events(
            "Page.navigate",
            json!({ "url": "data:text/html,<p>x</p>" }),
            Some(&quiet),
        )
        .await;
    assert_eq!(
        methods(&heard),
        [
            "Page.frameStartedNavigating",
            "Page.frameStartedLoading",
            "Runtime.executionContextsCleared",
            "Runtime.executionContextsCleared",
            "Page.frameNavigated",
            "Runtime.executionContextCreated",
            "Page.domContentEventFired",
            "Page.lifecycleEvent",
            "Page.loadEventFired",
            "Page.lifecycleEvent",
            "Page.frameStoppedLoading",
        ],
        "{heard:?}"
    );
    assert!(
        heard.iter().all(|event| event["sessionId"] == loud),
        "{heard:?}"
    );
    let named: Vec<&str> = heard
        .iter()
        .filter(|event| event["method"] == "Page.lifecycleEvent")
        .map(|event| event["params"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(named, ["DOMContentLoaded", "load"]);
    let loud_world = world_of(&heard, &loud);
    assert_eq!(loud_world.len(), 1);

    // The other session hears the same world once it asks — once.
    let (_, heard) = client
        .with_events("Runtime.enable", json!({}), Some(&quiet))
        .await;
    assert_eq!(world_of(&heard, &quiet), loud_world);
    assert!(
        heard.iter().all(|event| event["sessionId"] == quiet),
        "{heard:?}"
    );
    let (_, again) = client
        .with_events("Runtime.enable", json!({}), Some(&quiet))
        .await;
    assert!(again.is_empty(), "{again:?}");

    // `Page.disable` on the loud session: it keeps the contexts alone — a
    // lifecycle event is the Page domain's, its switch notwithstanding. The
    // quiet session turns the domain on and hears the whole navigation.
    client.run("Page.disable", json!({}), Some(&loud)).await;
    client.run("Page.enable", json!({}), Some(&quiet)).await;
    let (_, heard) = client
        .with_events(
            "Page.navigate",
            json!({ "url": "data:text/html,<p>y</p>" }),
            Some(&quiet),
        )
        .await;
    let of = |session: &str| {
        heard
            .iter()
            .filter(|event| event["sessionId"] == session)
            .map(|event| event["method"].as_str().unwrap().to_string())
            .collect::<Vec<String>>()
    };
    assert_eq!(
        of(&loud),
        [
            "Runtime.executionContextsCleared",
            "Runtime.executionContextsCleared",
            "Runtime.executionContextCreated",
        ]
    );
    assert_eq!(
        of(&quiet),
        [
            "Page.frameStartedNavigating",
            "Page.frameStartedLoading",
            "Runtime.executionContextsCleared",
            "Runtime.executionContextsCleared",
            "Page.frameNavigated",
            "Runtime.executionContextCreated",
            "Page.domContentEventFired",
            "Page.loadEventFired",
            "Page.frameStoppedLoading",
        ]
    );
    assert_eq!(heard.len(), of(&loud).len() + of(&quiet).len(), "{heard:?}");
    // A navigation is a new world: the id is not the last one's.
    assert_ne!(world_of(&heard, &quiet), loud_world);
}

#[tokio::test]
async fn auto_attach_is_a_switch_on_the_socket() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    for (params, message) in [
        (json!({ "autoAttach": true }), "Invalid parameters"),
        (
            json!({ "waitForDebuggerOnStart": true, "flatten": true }),
            "Invalid parameters",
        ),
        (
            json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": false }),
            "Only flatten protocol is supported with browser level auto-attach",
        ),
    ] {
        assert_eq!(
            client.invalid("Target.setAutoAttach", params, None).await,
            message
        );
    }
    let on = || json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true });

    // On, with no page of this socket's: the switch, and nothing else.
    assert_eq!(
        client.with_events("Target.setAutoAttach", on(), None).await,
        (json!({}), vec![])
    );
    let mut pages = Vec::new();
    for _ in 0..2 {
        let before = client.events.len();
        let created = client
            .run("Target.createTarget", json!({ "url": "about:blank" }), None)
            .await;
        let target = created["targetId"].as_str().unwrap().to_string();
        let batch = &client.events[before..];
        assert_eq!(methods(batch), ["Target.attachedToTarget"], "{batch:?}");
        let event = &batch[0];
        assert!(event.get("sessionId").is_none(), "{event}");
        assert_eq!(event["params"]["targetInfo"]["targetId"], target);
        assert_eq!(event["params"]["waitingForDebugger"], true);
        let session = event["params"]["sessionId"].as_str().unwrap().to_string();
        assert_eq!(client.eval("1", &session).await, 1);
        pages.push((target, session));
    }
    // Asked again: each page already has the session the switch makes.
    assert_eq!(
        client.with_events("Target.setAutoAttach", on(), None).await,
        (json!({}), vec![])
    );

    // Off: both are taken back.
    let (_, batch) = client
        .with_events(
            "Target.setAutoAttach",
            json!({ "autoAttach": false, "waitForDebuggerOnStart": true, "flatten": true }),
            None,
        )
        .await;
    assert_eq!(
        methods(&batch),
        ["Target.detachedFromTarget", "Target.detachedFromTarget"]
    );
    let gone: Vec<&str> = batch
        .iter()
        .map(|event| event["params"]["sessionId"].as_str().unwrap())
        .collect();
    assert_ne!(gone[0], gone[1]);
    assert!(
        batch.iter().all(|event| event.get("sessionId").is_none()),
        "{batch:?}"
    );
    for (target, session) in &pages {
        assert!(gone.contains(&session.as_str()), "{batch:?}");
        assert_ne!(
            batch[0]["params"]["targetId"],
            batch[1]["params"]["targetId"]
        );
        client.assert_gone(session).await;
        assert!(!attached_of(&mut client, target).await);
    }

    // A session taken by hand is made again by the next switch on — a new one.
    let (target, _) = pages[0].clone();
    let (_, batch) = client.with_events("Target.setAutoAttach", on(), None).await;
    assert_eq!(
        methods(&batch),
        ["Target.attachedToTarget", "Target.attachedToTarget"]
    );
    let at = batch
        .iter()
        .position(|event| event["params"]["targetInfo"]["targetId"] == target)
        .expect("the page's session");
    let first = minted(&batch[at]).to_string();
    client
        .run(
            "Target.detachFromTarget",
            json!({ "sessionId": first }),
            None,
        )
        .await;
    assert!(!attached_of(&mut client, &target).await);
    let (_, batch) = client.with_events("Target.setAutoAttach", on(), None).await;
    assert_eq!(methods(&batch), ["Target.attachedToTarget"], "{batch:?}");
    let again = minted(&batch[0]);
    assert_ne!(again, first);
    assert_eq!(batch[0]["params"]["targetInfo"]["targetId"], target);
    assert_eq!(client.eval("2", again).await, 2);
}

#[tokio::test]
async fn closing_a_target_ends_its_sessions() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut client, addr) = connect(axum::Router::new()).await;
    assert_eq!(
        client
            .with_events(
                "Target.setDiscoverTargets",
                json!({ "discover": true }),
                None
            )
            .await,
        (json!({}), vec![])
    );
    let before = client.events.len();
    let created = client
        .run("Target.createTarget", json!({ "url": "about:blank" }), None)
        .await;
    let target = created["targetId"].as_str().unwrap().to_string();
    let batch = &client.events[before..];
    assert_eq!(methods(batch), ["Target.targetCreated"], "{batch:?}");
    assert_eq!(batch[0]["params"]["targetInfo"]["targetId"], target);
    assert_eq!(batch[0]["params"]["targetInfo"]["attached"], false);

    let mut sessions = Vec::new();
    for _ in 0..2 {
        let before = client.events.len();
        let attached = client
            .run("Target.attachToTarget", json!({ "targetId": target }), None)
            .await;
        sessions.push(attached["sessionId"].as_str().unwrap().to_string());
        assert_eq!(minted(&client.events[before]), sessions.last().unwrap());
    }
    let info = client
        .run("Target.getTargetInfo", json!({ "targetId": target }), None)
        .await;
    assert_eq!(info["targetInfo"]["attached"], true);

    // In Chrome's order: each session hears it is detached, then the level
    // each was attached on hears it went, then the owner hears the page go.
    let before = client.events.len();
    let closed = client
        .run("Target.closeTarget", json!({ "targetId": target }), None)
        .await;
    assert_eq!(closed, json!({ "success": true }));
    let batch = &client.events[before..];
    assert_eq!(
        methods(batch),
        [
            "Inspector.detached",
            "Inspector.detached",
            "Target.detachedFromTarget",
            "Target.detachedFromTarget",
            "Target.targetInfoChanged",
            "Target.targetDestroyed",
        ],
        "{batch:?}"
    );
    assert!(batch[..2]
        .iter()
        .all(|event| event["params"] == json!({ "reason": "target_closed" })));
    let on: Vec<&str> = batch[..2]
        .iter()
        .map(|event| event["sessionId"].as_str().unwrap())
        .collect();
    let gone: Vec<&str> = batch[2..4]
        .iter()
        .map(|event| event["params"]["sessionId"].as_str().unwrap())
        .collect();
    assert_ne!(on[0], on[1]);
    assert_ne!(gone[0], gone[1]);
    assert!(
        on.iter()
            .all(|id| sessions.iter().any(|session| session == id)),
        "{batch:?}"
    );
    assert!(
        gone.iter()
            .all(|id| sessions.iter().any(|session| session == id)),
        "{batch:?}"
    );
    assert!(
        batch[2..]
            .iter()
            .all(|event| event.get("sessionId").is_none()),
        "{batch:?}"
    );
    assert_eq!(batch[4]["params"]["targetInfo"]["attached"], false);
    assert_eq!(batch[5]["params"]["targetId"], target);
    for session in &sessions {
        client.assert_gone(session).await;
    }
    assert_eq!(
        client
            .invalid("Target.getTargetInfo", json!({ "targetId": target }), None)
            .await,
        "No target with given id found"
    );
    assert_eq!(
        client
            .invalid("Target.closeTarget", json!({ "targetId": target }), None)
            .await,
        "No target with given id found"
    );
    assert_eq!(
        client
            .invalid(
                "Target.detachFromTarget",
                json!({ "sessionId": sessions[0] }),
                None
            )
            .await,
        "No session with given id"
    );
    assert!(http_get(addr, "/json/list")
        .await
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_browser_session_speaks_for_the_browser() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    let (target, page) = owned_page(&mut client, 1).await;

    let (reply, batch) = client
        .with_events("Target.attachToBrowserTarget", json!({}), None)
        .await;
    let browser = reply["sessionId"].as_str().unwrap().to_string();
    assert_eq!(methods(&batch), ["Target.attachedToTarget"]);
    assert_eq!(minted(&batch[0]), browser);
    let event = &batch[0];
    assert!(event.get("sessionId").is_none(), "{event}");
    assert_eq!(event["params"]["waitingForDebugger"], false);
    let info = &event["params"]["targetInfo"];
    assert_eq!(info["type"], "browser");
    assert_eq!(
        (info["title"].as_str(), info["url"].as_str()),
        (Some(""), Some(""))
    );
    assert_eq!(info["attached"], true);
    // A target id of its own, as each browser target in Chrome has.
    let browser_target = info["targetId"].as_str().unwrap().to_string();
    assert_ne!(browser_target, browser);
    let again = client
        .run("Target.attachToBrowserTarget", json!({}), None)
        .await;
    assert_ne!(again["sessionId"], browser);

    // Browser domains answer on it; the page's do not.
    let frame = client
        .exchange(1, "Browser.getVersion", json!({}), Some(&browser))
        .await;
    assert_eq!(frame["sessionId"], browser, "{frame}");
    assert!(frame["result"]["product"]
        .as_str()
        .unwrap()
        .starts_with("diting/"));
    assert_eq!(
        client
            .ask(
                "Runtime.evaluate",
                json!({ "expression": "1" }),
                Some(&browser)
            )
            .await,
        Err((-32601, "'Runtime.evaluate' wasn't found".to_string()))
    );
    let own = client
        .run("Target.getTargetInfo", json!({}), Some(&browser))
        .await;
    assert_eq!(own["targetInfo"]["targetId"], browser_target);

    // A page session attached under it is announced on it, and runs.
    let mut under = String::new();
    for _ in 0..2 {
        let (_, batch) = client
            .with_events(
                "Target.attachToTarget",
                json!({ "targetId": target }),
                Some(&browser),
            )
            .await;
        assert_eq!(methods(&batch), ["Target.attachedToTarget"], "{batch:?}");
        assert_eq!(batch[0]["sessionId"], browser);
        let fresh = minted(&batch[0]).to_string();
        if under.is_empty() {
            assert_eq!(client.eval("6 * 7", &fresh).await, 42);
        } else {
            assert_ne!(fresh, under);
        }
        under = fresh;
    }
    // It takes its own child by id…
    let (_, batch) = client
        .with_events(
            "Target.detachFromTarget",
            json!({ "sessionId": under }),
            Some(&browser),
        )
        .await;
    assert_eq!(methods(&batch), ["Target.detachedFromTarget"]);
    assert_eq!(batch[0]["sessionId"], browser);
    assert_eq!(batch[0]["params"]["sessionId"], under);
    client.assert_gone(&under).await;
    let (_, batch) = client
        .with_events(
            "Target.attachToTarget",
            json!({ "targetId": target }),
            Some(&browser),
        )
        .await;
    under = minted(&batch[0]).to_string();

    // …the root cannot reach that child…
    assert_eq!(
        client
            .invalid(
                "Target.detachFromTarget",
                json!({ "sessionId": under }),
                None
            )
            .await,
        "No session with given id"
    );
    // …and takes the browser session: what hangs under it goes unannounced.
    let (_, batch) = client
        .with_events(
            "Target.detachFromTarget",
            json!({ "sessionId": browser }),
            None,
        )
        .await;
    assert_eq!(methods(&batch), ["Target.detachedFromTarget"], "{batch:?}");
    assert!(batch[0].get("sessionId").is_none());
    assert_eq!(
        batch[0]["params"],
        json!({ "sessionId": browser, "targetId": browser_target })
    );
    client.assert_gone(&browser).await;
    client.assert_gone(&under).await;
    assert_eq!(client.eval("2 + 2", &page).await, 4);
}

#[tokio::test]
async fn a_session_belongs_to_its_socket() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut owner, addr) = connect(axum::Router::new()).await;
    let (target, session) = open_owned_page(&mut owner, 1).await;
    let mut other = another(addr).await;

    // Another socket knows no session of this one — and its reply, like
    // Chrome's, names none.
    let frame = other
        .exchange(
            1,
            "Runtime.evaluate",
            json!({ "expression": "1" }),
            Some(&session),
        )
        .await;
    assert_eq!(
        frame["error"],
        json!({ "code": -32001, "message": "Session with given id not found." })
    );
    assert!(frame.get("sessionId").is_none(), "{frame}");
    assert_eq!(
        other
            .invalid(
                "Target.detachFromTarget",
                json!({ "sessionId": session }),
                None
            )
            .await,
        "No session with given id"
    );
    for method in [
        "Target.attachToTarget",
        "Target.closeTarget",
        "Target.getTargetInfo",
    ] {
        assert_eq!(
            other
                .invalid(method, json!({ "targetId": target }), None)
                .await,
            "No target with given id found",
            "{method}"
        );
    }
    // Nor its page.
    let targets = other.run("Target.getTargets", json!({}), None).await;
    assert!(page_target_ids(&targets).is_empty(), "{targets}");
    assert_eq!(
        http_get(addr, "/json/list").await.as_array().unwrap().len(),
        1
    );
    // The owner lost nothing.
    assert_eq!(owner.eval("1 + 1", &session).await, 2);
    assert!(attached_of(&mut owner, &target).await);
}

#[tokio::test]
async fn a_page_websocket_is_that_pages_root_level() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut owner, addr) = connect(axum::Router::new()).await;
    let (target, session) = open_owned_page(&mut owner, 1).await;
    let listed = http_get(addr, "/json/list").await;
    assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(listed[0]["id"], target);
    let ws_url = listed[0]["webSocketDebuggerUrl"].as_str().unwrap();
    assert!(
        ws_url.ends_with(&format!("/devtools/page/{target}")),
        "{ws_url}"
    );
    let mut page = page_socket(addr, &target).await;

    // Its root level is the page: a command there reaches it, from the
    // browser socket's session too, and its replies name no session.
    let frame = page
        .exchange(
            1,
            "Runtime.evaluate",
            json!({ "expression": "globalThis.__page = 'socket'", "returnByValue": true }),
            None,
        )
        .await;
    assert!(frame.get("sessionId").is_none(), "{frame}");
    assert_eq!(frame["result"]["result"]["value"], "socket");
    assert_eq!(owner.eval("globalThis.__page", &session).await, "socket");
    let info = page.run("Target.getTargetInfo", json!({}), None).await;
    assert_eq!(info["targetInfo"]["targetId"], target);

    // It names no session — not another socket's, not its own: its level
    // has no id.
    for named in [NO_SUCH_ID.to_string(), session.clone()] {
        let frame = page
            .exchange(
                2,
                "Runtime.evaluate",
                json!({ "expression": "1" }),
                Some(&named),
            )
            .await;
        assert_eq!(
            frame["error"],
            json!({ "code": -32001, "message": "Session with given id not found." })
        );
        assert!(frame.get("sessionId").is_none(), "{frame}");
    }

    // The page's events reach it at its root level, whoever causes them.
    let (reply, batch) = page.with_events("Runtime.enable", json!({}), None).await;
    assert_eq!(reply, json!({}));
    assert_eq!(
        methods(&batch),
        ["Runtime.executionContextCreated"],
        "{batch:?}"
    );
    assert!(batch[0].get("sessionId").is_none(), "{batch:?}");
    let (_, again) = page.with_events("Runtime.enable", json!({}), None).await;
    assert!(again.is_empty(), "{again:?}");
    page.run("Page.enable", json!({}), None).await;
    owner
        .run(
            "Page.navigate",
            json!({ "url": "data:text/html,<p>far</p>" }),
            Some(&session),
        )
        .await;
    let heard = page
        .events_until("Page.frameStoppedLoading", Duration::from_secs(10))
        .await;
    assert_eq!(
        methods(&heard),
        [
            "Page.frameStartedNavigating",
            "Page.frameStartedLoading",
            "Runtime.executionContextsCleared",
            "Runtime.executionContextsCleared",
            "Page.frameNavigated",
            "Runtime.executionContextCreated",
            "Page.domContentEventFired",
            "Page.loadEventFired",
            "Page.frameStoppedLoading",
        ],
        "{heard:?}"
    );
    assert!(
        heard.iter().all(|event| event.get("sessionId").is_none()),
        "{heard:?}"
    );

    // Its page closed under it: one word, then the socket goes.
    owner
        .run("Target.closeTarget", json!({ "targetId": target }), None)
        .await;
    let event = page
        .next_event(Duration::from_secs(10))
        .await
        .expect("the page websocket was closed without a word");
    assert_eq!(event["method"], "Inspector.detached");
    assert_eq!(event["params"], json!({ "reason": "target_closed" }));
    assert!(event.get("sessionId").is_none(), "{event}");
    assert!(page.next_frame(Duration::from_secs(10)).await.is_none());
    let closed = format!("ws://{addr}/devtools/page/{target}");
    assert!(
        connect_async(closed).await.is_err(),
        "a closed page still has a websocket"
    );
}

#[tokio::test]
async fn a_socket_takes_its_pages_when_it_leaves() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut owner, addr) = connect(axum::Router::new()).await;
    let (target, _) = open_owned_page(&mut owner, 1).await;
    let mut page = page_socket(addr, &target).await;
    let mut other = another(addr).await;
    // No goodbye: the socket just goes.
    drop(owner);
    let event = page
        .next_event(Duration::from_secs(10))
        .await
        .expect("the page websocket was closed without a word");
    assert_eq!(event["method"], "Inspector.detached");
    assert_eq!(event["params"], json!({ "reason": "target_closed" }));
    assert!(page.next_frame(Duration::from_secs(10)).await.is_none());
    assert!(http_get(addr, "/json/list")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    // Another socket of the same bridge is none the worse.
    let (targets, _) = other
        .with_events("Target.getTargets", json!({}), None)
        .await;
    assert!(page_target_ids(&targets).is_empty(), "{targets}");
}

fn page_target_ids(targets: &Value) -> Vec<String> {
    targets["targetInfos"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|info| info["type"] == "page")
        .map(|info| info["targetId"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn two_clients_keep_their_own_pages() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut a, addr) = connect(axum::Router::new()).await;
    let mut b = another(addr).await;
    let (target_a, session_a) = open_owned_page(&mut a, 1).await;
    let (target_b, session_b) = open_owned_page(&mut b, 1).await;
    assert_ne!(target_a, target_b);

    a.call(
        10,
        "Runtime.evaluate",
        json!({ "expression": "globalThis.__who = 'a'", "returnByValue": true }),
        Some(&session_a),
    )
    .await;
    b.call(
        10,
        "Runtime.evaluate",
        json!({ "expression": "globalThis.__who = 'b'", "returnByValue": true }),
        Some(&session_b),
    )
    .await;
    let who_a = a
        .call(
            11,
            "Runtime.evaluate",
            json!({ "expression": "globalThis.__who", "returnByValue": true }),
            Some(&session_a),
        )
        .await;
    let who_b = b
        .call(
            11,
            "Runtime.evaluate",
            json!({ "expression": "globalThis.__who", "returnByValue": true }),
            Some(&session_b),
        )
        .await;
    assert_eq!(who_a["result"]["value"], "a", "{who_a}");
    assert_eq!(who_b["result"]["value"], "b", "{who_b}");

    let seen_a = page_target_ids(&a.call(12, "Target.getTargets", json!({}), None).await);
    let seen_b = page_target_ids(&b.call(12, "Target.getTargets", json!({}), None).await);
    assert_eq!(seen_a, vec![target_a.clone()], "{seen_a:?}");
    assert_eq!(seen_b, vec![target_b.clone()], "{seen_b:?}");
    assert_eq!(
        http_get(addr, "/json/list").await.as_array().unwrap().len(),
        2
    );

    a.call(13, "Browser.close", json!({}), None).await;
    let who_b = b
        .call(
            14,
            "Runtime.evaluate",
            json!({ "expression": "globalThis.__who", "returnByValue": true }),
            Some(&session_b),
        )
        .await;
    assert_eq!(
        who_b["result"]["value"], "b",
        "closing a dropped b: {who_b}"
    );
    let seen_b = page_target_ids(&b.call(15, "Target.getTargets", json!({}), None).await);
    assert_eq!(seen_b, vec![target_b.clone()], "{seen_b:?}");
    let listed = http_get(addr, "/json/list").await;
    assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(listed[0]["id"], target_b);
    b.call(16, "Browser.close", json!({}), None).await;
}

fn context_announcements(client: &Client, session: &str) -> usize {
    client
        .events
        .iter()
        .filter(|event| {
            event["method"] == "Runtime.executionContextCreated" && event["sessionId"] == session
        })
        .count()
}

#[tokio::test]
async fn a_detached_session_ends_until_it_is_attached_again() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    let (target, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    // Runtime.enable announces the main world again after a disable.
    assert_eq!(context_announcements(&client, &session), 1);
    client.run("Runtime.disable", json!({}), on).await;
    client.run("Runtime.enable", json!({}), on).await;
    assert_eq!(context_announcements(&client, &session), 2);

    // A session cannot take itself: only the root's sessions answer to the
    // root, and it is not gone for the try.
    assert_eq!(
        client
            .ask(
                "Target.detachFromTarget",
                json!({ "sessionId": session }),
                on
            )
            .await,
        Err((-32602, "No session with given id".to_string()))
    );
    assert_eq!(client.eval("1 + 1", &session).await, 2);

    let before = client.events.len();
    client
        .run(
            "Target.detachFromTarget",
            json!({ "sessionId": session }),
            None,
        )
        .await;
    let batch = &client.events[before..];
    assert_eq!(batch.len(), 1, "{batch:?}");
    assert_eq!(batch[0]["method"], "Target.detachedFromTarget");
    assert_eq!(
        batch[0]["params"],
        json!({ "sessionId": session, "targetId": target })
    );
    assert!(batch[0].get("sessionId").is_none(), "{batch:?}");
    assert!(!attached_of(&mut client, &target).await);
    client.assert_gone(&session).await;
    // Nothing is attached to the page any more, for the root or by target.
    assert_eq!(
        client
            .invalid(
                "Target.detachFromTarget",
                json!({ "sessionId": session }),
                None
            )
            .await,
        "No session with given id"
    );
    assert_eq!(
        client
            .invalid(
                "Target.detachFromTarget",
                json!({ "targetId": target }),
                None
            )
            .await,
        "No session for given target id"
    );

    // Attached again: a new session, with nothing switched on.
    let before = client.events.len();
    let fresh = client
        .run("Target.attachToTarget", json!({ "targetId": target }), None)
        .await["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(fresh, session);
    let batch = &client.events[before..];
    assert_eq!(batch.len(), 1, "{batch:?}");
    assert_eq!(batch[0]["method"], "Target.attachedToTarget");
    assert!(batch[0].get("sessionId").is_none(), "{batch:?}");
    assert_eq!(batch[0]["params"]["sessionId"], fresh);
    assert_eq!(batch[0]["params"]["waitingForDebugger"], false);
    assert_eq!(batch[0]["params"]["targetInfo"]["attached"], true);
    assert_eq!(client.eval("2 + 2", &fresh).await, 4);
    let (_, heard) = client
        .with_events(
            "Page.navigate",
            json!({ "url": "data:text/html,<p>again</p>" }),
            Some(&fresh),
        )
        .await;
    assert!(
        heard.is_empty(),
        "the new session inherited switches: {heard:?}"
    );

    // Two sessions on the page: a target alone names neither.
    let other = client
        .run("Target.attachToTarget", json!({ "targetId": target }), None)
        .await["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client
            .refusal(
                "Target.detachFromTarget",
                json!({ "targetId": target }),
                None
            )
            .await,
        "Target.detachFromTarget: Multiple sessions attached, specify id."
    );
    // One page session is not on another's table.
    assert_eq!(
        client
            .invalid(
                "Target.detachFromTarget",
                json!({ "sessionId": other }),
                Some(&fresh)
            )
            .await,
        "No session with given id"
    );
    client
        .run(
            "Target.detachFromTarget",
            json!({ "sessionId": other }),
            None,
        )
        .await;
    client.assert_gone(&other).await;
    assert_eq!(client.eval("3 + 3", &fresh).await, 6);
    // With one left, the target names it.
    client
        .run(
            "Target.detachFromTarget",
            json!({ "targetId": target }),
            None,
        )
        .await;
    client.assert_gone(&fresh).await;
    assert!(!attached_of(&mut client, &target).await);
}

/// Whether any session is on `target`, as `Target.getTargets` says.
async fn attached_of(client: &mut Client, target: &str) -> bool {
    let targets = client.run("Target.getTargets", json!({}), None).await;
    targets["targetInfos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|info| info["targetId"] == target)
        .expect("the page is still there")["attached"]
        .as_bool()
        .unwrap()
}
