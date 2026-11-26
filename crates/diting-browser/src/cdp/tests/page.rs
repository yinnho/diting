//! Navigation, load events and Input.
//!
//! Every row was read off Chrome 158 first. What is not pinned on purpose:
//! the exact `net::ERR_*` a failed navigation gets beyond the refused
//! connection (`tests/network.rs` pins that one) — and anything `Input` does to the
//! page: the key and pointer events are the next batch's, so only what the
//! commands answer is held here.

use super::*;

/// The keys of a JSON object, sorted — this workspace's `serde_json` does
/// not keep the order they were written in.
fn sorted_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

/// `Page.navigate`'s receipt has exactly these keys — nothing more.
#[tokio::test]
async fn navigate_answers_with_its_receipt() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    for url in ["about:blank", "  about:blank  ", "data:text/html,hi"] {
        let reply = client.run("Page.navigate", json!({ "url": url }), on).await;
        assert_eq!(
            sorted_keys(&reply),
            ["frameId", "isDownload", "loaderId"],
            "{url}"
        );
        assert_eq!(reply["isDownload"], json!(false), "{url}");
    }

    // A navigation that fails still answers — with a `net::ERR_*` as
    // `errorText`. `foo://bar` fails offline; which name Chrome gives it
    // (`ERR_UNKNOWN_URL_SCHEME`) is not read here, so it is the catch-all.
    let reply = client
        .run("Page.navigate", json!({ "url": "foo://bar" }), on)
        .await;
    assert_eq!(
        sorted_keys(&reply),
        ["errorText", "frameId", "isDownload", "loaderId"]
    );
    assert!(
        reply["errorText"]
            .as_str()
            .is_some_and(|text| text.starts_with("net::ERR_")),
        "{reply}"
    );
}

/// What is no URL at all is refused in Chrome's words.
#[tokio::test]
async fn navigate_refuses_what_is_no_url() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    // `""` and `"not a url"` are pinned in `tests/protocol.rs` already.
    for url in ["http://", "//", "http://[::1", "#y"] {
        assert_eq!(
            client
                .refusal("Page.navigate", json!({ "url": url }), on)
                .await,
            "Cannot navigate to invalid URL",
            "{url}"
        );
    }
}

/// `Page.reload` answers `{}`, and the document really loads again: what
/// the page set is gone, and its own script ran once more.
#[tokio::test]
async fn reload_loads_the_document_again() {
    let page = "data:text/html,<script>window.loads=(window.loads||0)+1</script>";
    let (mut client, session) = bridge(page).await;
    let on = Some(session.as_str());
    client.run("Page.enable", json!({}), on).await;
    assert_eq!(client.eval("window.loads", &session).await, json!(1));
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "window.marker = 41" }),
            on,
        )
        .await;

    for params in [json!({}), json!({ "ignoreCache": true })] {
        client.run("Page.reload", params, on).await;
        let heard = client
            .events_until("Page.loadEventFired", Duration::from_secs(10))
            .await;
        assert!(!heard.is_empty(), "the reload loaded nothing");
        assert_eq!(client.eval("window.marker", &session).await, Value::Null);
        // A reload is a fresh window: the page's own script ran once more.
        assert_eq!(client.eval("window.loads", &session).await, json!(1));
        client
            .run(
                "Runtime.evaluate",
                json!({ "expression": "window.marker = 41" }),
                on,
            )
            .await;
    }
}

/// With no dialog up, `Page.handleJavaScriptDialog` is `-32602` — the
/// `{accept: true}` half is pinned in `tests/protocol.rs`.
#[tokio::test]
async fn a_dialog_that_is_not_there_is_said_so() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    assert_eq!(
        client
            .invalid(
                "Page.handleJavaScriptDialog",
                json!({ "accept": false, "promptText": "x" }),
                on,
            )
            .await,
        "No dialog is showing"
    );
}

/// The `Input` commands' parameters are read as Chrome reads them — the
/// kinds of every declared field, before what the engine would do with
/// them. What the events do to the page is not pinned here.
#[tokio::test]
async fn input_parameters_are_read_as_chrome_reads_them() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let invalid = |message: &str| Err((-32602, message.to_string()));
    let bad = || invalid("Invalid parameters");
    let ok = || Ok(json!({}));
    let mouse = "Input.dispatchMouseEvent";
    let key = "Input.dispatchKeyEvent";
    // (method, params, what Chrome answered) — the params are the probe's
    // own: every positioned mouse row carries x:1, y:2.
    let moved = |extra: Value| {
        let mut p = json!({ "type": "mouseMoved", "x": 1, "y": 2 });
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        p
    };
    let pressed = |extra: Value| {
        let mut p = json!({ "type": "mousePressed", "x": 1, "y": 2 });
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        p
    };
    let wheeled = |extra: Value| {
        let mut p = json!({ "type": "mouseWheel", "x": 1, "y": 2 });
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        p
    };
    let down = |extra: Value| {
        let mut p = json!({ "type": "keyDown" });
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        p
    };
    for (method, params, expected) in [
        (mouse, json!({}), bad()),
        (mouse, json!({ "type": "bogus" }), bad()),
        (mouse, json!({ "type": 5 }), bad()),
        (mouse, json!({ "type": null }), bad()),
        (mouse, json!({ "type": "mouseMoved" }), bad()),
        (mouse, json!({ "type": "mouseMoved", "x": 1 }), bad()),
        (
            mouse,
            json!({ "type": "mouseMoved", "x": "1", "y": 2 }),
            bad(),
        ),
        (
            mouse,
            json!({ "type": "mouseMoved", "x": 1, "y": null }),
            bad(),
        ),
        (mouse, moved(json!({ "x": 1.5, "y": 2.5 })), ok()),
        (mouse, moved(json!({ "x": 1e30, "y": 2 })), ok()),
        (mouse, pressed(json!({})), ok()),
        (
            mouse,
            pressed(json!({ "button": "bogus" })),
            invalid("Invalid mouse button"),
        ),
        (mouse, pressed(json!({ "button": 5 })), bad()),
        (mouse, pressed(json!({ "button": null })), bad()),
        (mouse, pressed(json!({ "clickCount": "1" })), bad()),
        (mouse, pressed(json!({ "clickCount": null })), bad()),
        (mouse, pressed(json!({ "clickCount": -1 })), ok()),
        (mouse, pressed(json!({ "clickCount": 1.5 })), bad()),
        (mouse, pressed(json!({ "buttons": "x" })), bad()),
        (mouse, pressed(json!({ "buttons": null })), bad()),
        (mouse, pressed(json!({ "modifiers": "x" })), bad()),
        (mouse, pressed(json!({ "modifiers": null })), bad()),
        (mouse, pressed(json!({ "modifiers": 1.5 })), bad()),
        (
            mouse,
            wheeled(json!({})),
            invalid("'deltaX' and 'deltaY' are expected for mouseWheel event"),
        ),
        (mouse, wheeled(json!({ "deltaX": "x" })), bad()),
        (mouse, wheeled(json!({ "deltaY": null })), bad()),
        (mouse, moved(json!({ "timestamp": "x" })), bad()),
        (mouse, moved(json!({ "pointerType": 5 })), bad()),
        (mouse, moved(json!({ "pointerType": "bogus" })), ok()),
        (mouse, moved(json!({ "force": "x" })), bad()),
        (mouse, moved(json!({ "tangentialPressure": null })), bad()),
        (mouse, moved(json!({ "twist": 1.5 })), bad()),
        (mouse, moved(json!({ "tilt": 1.5 })), ok()),
        (mouse, moved(json!({ "id": null })), ok()),
        (mouse, moved(json!({ "id": "5" })), ok()),
        ("Input.insertText", json!({}), bad()),
        ("Input.insertText", json!({ "text": 5 }), bad()),
        ("Input.insertText", json!({ "text": null }), bad()),
        ("Input.insertText", json!({ "text": "" }), ok()),
        (key, json!({}), bad()),
        (
            key,
            json!({ "type": "bogus" }),
            invalid("Unexpected event type 'bogus'"),
        ),
        (key, json!({ "type": 5 }), bad()),
        (key, json!({ "type": null }), bad()),
        (key, json!({ "type": "keyDown" }), ok()),
        (key, json!({ "type": "rawKeyDown" }), ok()),
        (key, json!({ "type": "char" }), ok()),
        (key, json!({ "type": "keyUp" }), ok()),
        (key, down(json!({ "key": 5 })), bad()),
        (key, down(json!({ "key": null })), bad()),
        (key, down(json!({ "text": 5 })), bad()),
        (key, down(json!({ "text": null })), bad()),
        (key, down(json!({ "code": 5 })), bad()),
        (key, down(json!({ "modifiers": "x" })), bad()),
        (key, down(json!({ "modifiers": null })), bad()),
        (key, down(json!({ "windowsVirtualKeyCode": "1" })), bad()),
        (key, down(json!({ "autoRepeat": "x" })), bad()),
        (key, down(json!({ "location": 9 })), ok()),
        (key, down(json!({ "location": 1.5 })), bad()),
        (key, down(json!({ "location": "0" })), bad()),
        (key, down(json!({ "key": "a", "commands": "x" })), bad()),
        (key, down(json!({ "key": "a", "editCommands": {} })), ok()),
        (key, down(json!({ "key": "a", "editCommands": [5] })), ok()),
        (key, down(json!({ "key": "a", "unmodifiedText": 5 })), bad()),
        (
            key,
            down(json!({ "key": "a", "nativeVirtualKeyCode": 1.5 })),
            bad(),
        ),
        (key, down(json!({ "key": "a", "keepRawText": "x" })), ok()),
        (key, down(json!({ "key": "a", "isKeypad": "x" })), bad()),
        (key, down(json!({ "key": "a", "isSystemKey": null })), bad()),
        (key, down(json!({ "key": "a", "markAsFallback": 5 })), ok()),
        (key, down(json!({ "key": "a", "text": null })), bad()),
    ] {
        let label = format!("{method} {params}");
        assert_eq!(client.ask(method, params, on).await, expected, "{label}");
    }
}

/// The shapes a key event takes all answer `{}` — what the events do is the
/// next batch's. Two will not read: a `windowsVirtualKeyCode` past int32,
/// and `commands` given a string.
#[tokio::test]
async fn key_events_take_their_shapes_as_chrome_takes_them() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let key = |params: Value| ("Input.dispatchKeyEvent", params);
    for (_, params) in [
        key(json!({ "type": "keyDown" })),
        key(json!({ "type": "keyDown", "key": "a" })),
        key(json!({ "type": "keyDown", "code": "KeyA" })),
        key(json!({ "type": "keyDown", "windowsVirtualKeyCode": 65 })),
        key(json!({ "type": "keyDown", "code": "KeyA", "windowsVirtualKeyCode": 65 })),
        key(json!({ "type": "keyDown", "key": "a", "code": "KeyA" })),
        key(json!({ "type": "keyDown", "key": "a", "windowsVirtualKeyCode": 65 })),
        key(json!({ "type": "keyDown", "key": "a", "code": "KeyA", "windowsVirtualKeyCode": 65 })),
        key(json!({ "type": "keyUp", "code": "KeyA", "windowsVirtualKeyCode": 65 })),
        key(json!({ "type": "keyUp", "key": "a" })),
        key(json!({ "type": "rawKeyDown", "code": "KeyA", "windowsVirtualKeyCode": 65 })),
        key(json!({ "type": "char", "text": "a" })),
        key(json!({ "type": "char", "key": "a", "text": "a" })),
        key(json!({ "type": "keyDown", "text": "a" })),
        key(json!({ "type": "keyDown", "text": "" })),
        key(json!({ "type": "keyDown", "unmodifiedText": "a" })),
        key(json!({ "type": "keyDown", "text": "ab" })),
        key(json!({ "type": "keyDown", "text": "\n" })),
        key(json!({ "type": "keyDown", "location": 9, "key": "a" })),
        key(json!({ "type": "keyDown", "code": "Space", "windowsVirtualKeyCode": 32 })),
        key(json!({ "type": "keyDown", "code": "Space" })),
        key(json!({ "type": "keyDown", "windowsVirtualKeyCode": 32 })),
        key(json!({ "type": "keyDown", "text": " " })),
        key(json!({ "type": "keyUp", "code": "Space", "windowsVirtualKeyCode": 32 })),
        key(json!({ "type": "rawKeyDown", "windowsVirtualKeyCode": 32 })),
        key(json!({ "type": "keyDown", "code": "Enter", "windowsVirtualKeyCode": 13 })),
        key(json!({ "type": "keyDown", "code": "bogus", "key": "a" })),
        key(json!({ "type": "keyDown", "windowsVirtualKeyCode": -1 })),
    ] {
        let label = params.to_string();
        assert_eq!(
            client.ask("Input.dispatchKeyEvent", params, on).await,
            Ok(json!({})),
            "{label}"
        );
    }
    for params in [
        json!({ "type": "keyDown", "windowsVirtualKeyCode": 1e30 }),
        json!({ "type": "keyDown", "key": "a", "commands": "selectAll" }),
    ] {
        assert_eq!(
            client
                .ask("Input.dispatchKeyEvent", params.clone(), on)
                .await,
            Err((-32602, "Invalid parameters".to_string())),
            "{params}"
        );
    }
}

/// Key events reach the page as KeyboardEvents on the focused element, in
/// Chrome's order (`key-probe.cjs`, Chrome 158): keyDown with a text is
/// keydown, keypress, the insertion and its input; rawKeyDown only the
/// keydown; a char is keypress plus insertion with no keydown at all.
#[tokio::test]
async fn key_events_reach_the_page_as_keyboard_events() {
    let (mut client, session) = bridge(
        "data:text/html,<input id=inp><script>        window.log = [];        const inp = document.getElementById('inp');        inp.focus();        for (const k of ['keydown', 'keypress', 'input', 'keyup']) {            inp.addEventListener(k, (e) => {                const key = e instanceof KeyboardEvent ? e.key : '-';                const code = e instanceof KeyboardEvent ? e.code : '-';                window.log.push(k + ':' + key + ':' + code);            }, true);        }        </script>",
    )
    .await;
    let on = Some(session.as_str());
    // KeyDown carrying the text — the full printable sequence.
    for params in [
        json!({ "type": "keyDown", "key": "a", "code": "KeyA", "text": "a" }),
        json!({ "type": "keyUp", "key": "a", "code": "KeyA" }),
        json!({ "type": "rawKeyDown", "key": "Enter", "code": "Enter" }),
        json!({ "type": "keyUp", "key": "Enter", "code": "Enter" }),
        json!({ "type": "char", "text": "b", "key": "b" }),
    ] {
        assert_eq!(
            client.ask("Input.dispatchKeyEvent", params, on).await,
            Ok(json!({}))
        );
    }
    let log = client.eval("window.log.join('|')", &session).await;
    assert_eq!(
        log,
        "keydown:a:KeyA|keypress:a:KeyA|input:-:-|keyup:a:KeyA|keydown:Enter:Enter|keyup:Enter:Enter|keypress:b:|input:-:-",
        "the event order Chrome 158 showed, verbatim — the char press carries no code"
    );
    assert_eq!(
        client
            .eval("document.getElementById('inp').value", &session)
            .await,
        "ab"
    );
}

/// Ctrl/Meta+A still selects the focused field's contents — and the keydown
/// reaches the page now.
#[tokio::test]
async fn control_a_selects_and_reaches_the_page() {
    let (mut client, session) = bridge(
        "data:text/html,<input id=inp value=hello><script>        window.downs = 0;        const inp = document.getElementById('inp');        inp.focus();        inp.addEventListener('keydown', (e) => {            if (e.key.toLowerCase() === 'a' && e.ctrlKey) window.downs += 1;        });        </script>",
    )
    .await;
    let on = Some(session.as_str());
    assert_eq!(
        client
            .ask(
                "Input.dispatchKeyEvent",
                json!({ "type": "keyDown", "key": "a", "code": "KeyA", "modifiers": 2 }),
                on
            )
            .await,
        Ok(json!({}))
    );
    assert_eq!(client.eval("window.downs", &session).await, 1);
    assert_eq!(
        client
            .eval(
                "const inp = document.getElementById('inp'); inp.selectionStart + '/' + inp.selectionEnd",
                &session
            )
            .await,
        "0/5"
    );
}
