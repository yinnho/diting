//! The `DOM` domain: a node named by handle or by number, what is asked of
//! it — and what is said when it cannot be found, or laid out.

use super::*;

/// A page with a red box of known sides, a div to take out of it, and one
/// nothing displays.
const NODES: &str = "data:text/html,<!doctype html><body style=\"margin:0\">\
     <div id=red class=a style=\"width:100px;height:50px;padding:4px;border:2px solid;margin:6px;background:rgb(255,0,0)\"></div>\
     <div id=gone></div><div id=hidden style=\"display:none\">h</div></body>";

/// The commands that name a node by `objectId`, `backendNodeId` or `nodeId`.
const NAMED: &[&str] = &[
    "DOM.describeNode",
    "DOM.getBoxModel",
    "DOM.getContentQuads",
    "DOM.scrollIntoViewIfNeeded",
];

/// The numbers of a JSON array, however they were spelled.
#[cfg(feature = "screenshot")]
fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(|number| number.as_f64().expect("a number"))
        .collect()
}

#[tokio::test]
async fn a_node_is_asked_for_by_handle_and_by_number() {
    let (mut client, _) = connect(axum::Router::new()).await;
    let (target, session) = open_owned_page(&mut client, 1).await;
    client
        .run(
            "Page.navigate",
            json!({ "url": NODES }),
            Some(session.as_str()),
        )
        .await;
    let on = Some(session.as_str());
    let body = handle_of(&mut client, &session, "document.body").await;
    let red = handle_of(&mut client, &session, "document.getElementById('red')").await;

    let described = client
        .run("DOM.describeNode", json!({ "objectId": body }), on)
        .await;
    let node = &described["node"];
    assert_eq!(node["nodeId"], 0);
    let body_number = node["backendNodeId"].as_i64().expect("a number");
    assert!(body_number > 0, "{node}");
    assert_eq!(
        (
            node["nodeType"].as_i64(),
            node["nodeName"].as_str(),
            node["localName"].as_str(),
            node["nodeValue"].as_str(),
            node["childNodeCount"].as_i64(),
        ),
        (Some(1), Some("BODY"), Some("body"), Some(""), Some(3))
    );
    assert_eq!(node["attributes"], json!(["style", "margin:0"]));
    // The one node, the one number — by its number, or asked for again.
    assert_eq!(
        client
            .run("DOM.describeNode", json!({ "objectId": body }), on)
            .await,
        described
    );
    assert_eq!(
        client
            .run(
                "DOM.describeNode",
                json!({ "backendNodeId": body_number }),
                on
            )
            .await,
        described
    );

    let document = handle_of(&mut client, &session, "document").await;
    let node = &client
        .run("DOM.describeNode", json!({ "objectId": document }), on)
        .await["node"];
    assert_eq!(
        (
            node["nodeType"].as_i64(),
            node["nodeName"].as_str(),
            node["childNodeCount"].as_i64(),
        ),
        (Some(9), Some("#document"), Some(2))
    );
    for key in ["documentURL", "baseURL"] {
        assert!(
            node[key].as_str().unwrap().starts_with("data:text/html,"),
            "{key}: {node}"
        );
    }
    assert_eq!(
        (
            node["xmlVersion"].as_str(),
            node["compatibilityMode"].as_str()
        ),
        (Some(""), Some("NoQuirksMode"))
    );
    let html = handle_of(&mut client, &session, "document.documentElement").await;
    let node = &client
        .run("DOM.describeNode", json!({ "objectId": html }), on)
        .await["node"];
    assert_eq!(node["frameId"], target);
    assert_eq!(node["attributes"], json!([]));

    // A number is turned back into a handle, spelled as Chrome spells it —
    // and it is the node.
    let red_number = client
        .run("DOM.describeNode", json!({ "objectId": red }), on)
        .await["node"]["backendNodeId"]
        .as_i64()
        .unwrap();
    // One number each: the body's still names the body.
    assert_ne!(red_number, body_number);
    assert_eq!(
        client
            .run(
                "DOM.describeNode",
                json!({ "backendNodeId": body_number }),
                on
            )
            .await,
        described
    );
    let object = &client
        .run(
            "DOM.resolveNode",
            json!({ "backendNodeId": red_number }),
            on,
        )
        .await["object"];
    assert_eq!(
        (
            object["type"].as_str(),
            object["subtype"].as_str(),
            object["className"].as_str(),
            object["description"].as_str(),
        ),
        (
            Some("object"),
            Some("node"),
            Some("HTMLDivElement"),
            Some("div#red.a")
        )
    );
    let resolved = object["objectId"].as_str().expect("the node's handle");
    // The handle is the node: `this` inside a call.
    let reply = client
        .run(
            "Runtime.callFunctionOn",
            json!({
                "objectId": resolved,
                "functionDeclaration": "function(){ return this.id + '.' + this.localName }",
                "returnByValue": true,
            }),
            on,
        )
        .await;
    assert_eq!(reply["result"]["value"], "red.div");
}

/// The boxes need the style and layout engine, which the `screenshot`
/// feature brings: they are laid out, not parsed.
#[cfg(feature = "screenshot")]
#[tokio::test]
async fn its_boxes_are_content_padding_border_margin() {
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    client
        .run(
            "Page.navigate",
            json!({ "url": NODES }),
            Some(session.as_str()),
        )
        .await;
    let on = Some(session.as_str());
    let red = handle_of(&mut client, &session, "document.getElementById('red')").await;
    let red_number = client
        .run("DOM.describeNode", json!({ "objectId": red }), on)
        .await["node"]["backendNodeId"]
        .as_i64()
        .unwrap();
    let resolved = client
        .run(
            "DOM.resolveNode",
            json!({ "backendNodeId": red_number }),
            on,
        )
        .await["object"]["objectId"]
        .as_str()
        .unwrap()
        .to_string();
    let laid = client
        .run(
            "Runtime.callFunctionOn",
            json!({
                "objectId": resolved,
                "functionDeclaration": "function(){ return this.offsetWidth + 'x' + this.offsetHeight }",
                "returnByValue": true,
            }),
            on,
        )
        .await;
    assert_eq!(laid["result"]["value"], "112x62");

    // 100×50 of content, 4 of padding, 2 of border, 6 of margin, on an
    // empty page: content, padding, border and margin quads — the same
    // by handle and by number.
    let expected: [Vec<f64>; 4] = [
        vec![12.0, 12.0, 112.0, 12.0, 112.0, 62.0, 12.0, 62.0],
        vec![8.0, 8.0, 116.0, 8.0, 116.0, 66.0, 8.0, 66.0],
        vec![6.0, 6.0, 118.0, 6.0, 118.0, 68.0, 6.0, 68.0],
        vec![0.0, 0.0, 124.0, 0.0, 124.0, 74.0, 0.0, 74.0],
    ];
    for named in [
        json!({ "objectId": red }),
        json!({ "backendNodeId": red_number }),
    ] {
        let model = client.run("DOM.getBoxModel", named.clone(), on).await["model"].clone();
        assert_eq!(
            ["content", "padding", "border", "margin"].map(|key| numbers(&model[key])),
            expected
        );
        assert_eq!(
            (model["width"].as_f64(), model["height"].as_f64()),
            (Some(112.0), Some(62.0))
        );
        let quads = client.run("DOM.getContentQuads", named, on).await["quads"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(quads.len(), 1, "{quads:?}");
        assert_eq!(numbers(&quads[0]), expected[2]);
    }
}

#[tokio::test]
async fn what_cannot_be_found_is_named_in_chromes_words() {
    let (mut client, _) = connect(axum::Router::new()).await;
    let session = page_at(&mut client, 1, NODES).await;
    let on = Some(session.as_str());
    let at = |id: &str| format!("document.getElementById('{id}')");
    let red = handle_of(&mut client, &session, &at("red")).await;
    let gone = handle_of(&mut client, &session, &at("gone")).await;
    let hidden = handle_of(&mut client, &session, &at("hidden")).await;
    let window = handle_of(&mut client, &session, "window").await;

    // Nothing displays it: there is no box to give.
    assert_eq!(
        client
            .refusal("DOM.getBoxModel", json!({ "objectId": hidden }), on)
            .await,
        "Could not compute box model."
    );
    assert_eq!(
        client
            .run("DOM.getContentQuads", json!({ "objectId": hidden }), on)
            .await,
        json!({ "quads": [] })
    );
    assert_eq!(
        client
            .refusal(
                "DOM.scrollIntoViewIfNeeded",
                json!({ "objectId": hidden }),
                on
            )
            .await,
        "Node does not have a layout object"
    );

    // Taken out of the document: still a node, no longer placed.
    let gone_number = client
        .run("DOM.describeNode", json!({ "objectId": gone }), on)
        .await["node"]["backendNodeId"]
        .as_i64()
        .unwrap();
    client
        .eval("document.getElementById('gone').remove()", &session)
        .await;
    assert_eq!(
        client
            .run("DOM.describeNode", json!({ "objectId": gone }), on)
            .await["node"]["backendNodeId"],
        gone_number
    );
    for named in [
        json!({ "objectId": gone }),
        json!({ "backendNodeId": gone_number }),
    ] {
        assert_eq!(
            client.refusal("DOM.getBoxModel", named.clone(), on).await,
            "Could not compute box model."
        );
        assert_eq!(
            client.run("DOM.getContentQuads", named.clone(), on).await,
            json!({ "quads": [] })
        );
        assert_eq!(
            client
                .refusal("DOM.scrollIntoViewIfNeeded", named, on)
                .await,
            "Node is detached from document"
        );
    }

    // A handle — to something that is not a node.
    for method in NAMED {
        assert_eq!(
            client
                .refusal(method, json!({ "objectId": window }), on)
                .await,
            "Object id doesn't reference a Node",
            "{method}"
        );
    }
    // No node named: none at all, the frontend's own numbering, or a number
    // this page never gave.
    for method in NAMED {
        for (named, message) in [
            (
                json!({}),
                "Either nodeId, backendNodeId or objectId must be specified",
            ),
            (json!({ "nodeId": 1 }), "Could not find node with given id"),
            (
                json!({ "backendNodeId": 999999 }),
                "No node found for given backend id",
            ),
            (
                json!({ "backendNodeId": 0 }),
                "No node found for given backend id",
            ),
        ] {
            assert_eq!(
                client.refusal(method, named.clone(), on).await,
                message,
                "{method} {named}"
            );
        }
    }
    // A `nodeId` is looked up first, found or not; a number next — and
    // beside a number a handle is not asked about, whatever it names.
    assert_eq!(
        client
            .refusal(
                "DOM.describeNode",
                json!({ "nodeId": 1, "backendNodeId": gone_number }),
                on,
            )
            .await,
        "Could not find node with given id"
    );
    for spare in [window.as_str(), NO_SUCH_ID] {
        let described = client
            .run(
                "DOM.describeNode",
                json!({ "backendNodeId": gone_number, "objectId": spare }),
                on,
            )
            .await;
        assert_eq!(described["node"]["backendNodeId"], gone_number, "{spare}");
    }
    // The handle is of its kind all the same, and a number is a whole one.
    for wrong in [
        json!({ "backendNodeId": gone_number, "objectId": 5 }),
        json!({ "backendNodeId": gone_number, "objectId": null }),
        json!({ "backendNodeId": "4" }),
        json!({ "backendNodeId": 1.5 }),
    ] {
        assert_eq!(
            client.invalid("DOM.describeNode", wrong.clone(), on).await,
            "Invalid parameters",
            "{wrong}"
        );
    }

    // `DOM.resolveNode` has its own words, and reads no `objectId`.
    for named in [
        json!({}),
        json!({ "objectId": red }),
        json!({ "objectId": 5 }),
        json!({ "objectId": null }),
        json!({ "nodeId": 1, "backendNodeId": gone_number }),
    ] {
        assert_eq!(
            client.refusal("DOM.resolveNode", named.clone(), on).await,
            "Either nodeId or backendNodeId must be specified.",
            "{named}"
        );
    }
    for named in [
        json!({ "backendNodeId": 999999 }),
        json!({ "nodeId": 1 }),
        json!({ "backendNodeId": 999999, "executionContextId": 999999 }),
    ] {
        assert_eq!(
            client.refusal("DOM.resolveNode", named.clone(), on).await,
            "No node with given id found",
            "{named}"
        );
    }
    // The node first, then the context it is asked for.
    for context in [json!(999999), json!(0)] {
        assert_eq!(
            client
                .refusal(
                    "DOM.resolveNode",
                    json!({ "backendNodeId": gone_number, "executionContextId": context }),
                    on,
                )
                .await,
            "Node with given id does not belong to the document",
            "{context}"
        );
    }
    assert_eq!(
        client
            .invalid(
                "DOM.resolveNode",
                json!({ "backendNodeId": gone_number, "executionContextId": "1" }),
                on,
            )
            .await,
        "Invalid parameters"
    );
    // In the page's own context, the node comes back as a handle.
    let object = &client
        .run(
            "DOM.resolveNode",
            json!({ "backendNodeId": gone_number, "executionContextId": main_context(&client) }),
            on,
        )
        .await["object"];
    assert_eq!(
        (
            object["subtype"].as_str(),
            object["className"].as_str(),
            object["description"].as_str(),
        ),
        (Some("node"), Some("HTMLDivElement"), Some("div#gone"))
    );

    // A handle let go, one that was never held, and one that is no id at
    // all — each in Chrome's word for it.
    client
        .run("Runtime.releaseObject", json!({ "objectId": red }), on)
        .await;
    let never = never_minted(&red);
    for (handle, message) in [
        (red.as_str(), "Could not find object with given id"),
        (never.as_str(), "Could not find object with given id"),
        (NO_SUCH_ID, "Invalid remote object id"),
    ] {
        for method in NAMED {
            assert_eq!(
                client
                    .refusal(method, json!({ "objectId": handle }), on)
                    .await,
                message,
                "{method} {handle}"
            );
        }
        assert_eq!(
            client
                .refusal("Runtime.releaseObject", json!({ "objectId": handle }), on)
                .await,
            message
        );
    }
}

#[tokio::test]
async fn a_number_of_a_document_gone_names_nothing() {
    let (mut client, _) = connect(axum::Router::new()).await;
    let session = page_at(&mut client, 1, NODES).await;
    let on = Some(session.as_str());
    let body = handle_of(&mut client, &session, "document.body").await;
    let old = client
        .run("DOM.describeNode", json!({ "objectId": body }), on)
        .await["node"]["backendNodeId"]
        .as_i64()
        .unwrap();

    // A document without a doctype.
    client
        .run(
            "Page.navigate",
            json!({ "url": "data:text/html,<p>next</p>" }),
            on,
        )
        .await;
    assert_eq!(
        client
            .refusal("DOM.describeNode", json!({ "backendNodeId": old }), on)
            .await,
        "No node found for given backend id"
    );
    assert_eq!(
        client
            .refusal("DOM.resolveNode", json!({ "backendNodeId": old }), on)
            .await,
        "No node with given id found"
    );
    // The handle of a node of the document before is of a context that is
    // gone — whatever wears its number now.
    for method in NAMED {
        assert_eq!(
            client
                .refusal(method, json!({ "objectId": body }), on)
                .await,
            "Cannot find context with specified id",
            "{method}"
        );
    }
    let body = handle_of(&mut client, &session, "document.body").await;
    let fresh = client
        .run("DOM.describeNode", json!({ "objectId": body }), on)
        .await["node"]["backendNodeId"]
        .as_i64()
        .unwrap();
    assert!(
        fresh > old,
        "the new document counted over again: {old} → {fresh}"
    );
    let document = handle_of(&mut client, &session, "document").await;
    let node = &client
        .run("DOM.describeNode", json!({ "objectId": document }), on)
        .await["node"];
    assert_eq!(node["childNodeCount"], 1);
    // The engine's page says `CSS1Compat` for a document without a
    // doctype — Chrome says `BackCompat`. The bridge answers what the page
    // says; this flips when the engine learns quirks mode.
    assert_eq!(node["compatibilityMode"], "NoQuirksMode");
}
