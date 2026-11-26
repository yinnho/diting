//! The `Runtime` domain: what a remote object says of itself, what comes
//! back by value and in whose words what cannot, a throw shown and not
//! refused, `callFunctionOn`, and what is said of an id that names nothing.
//!
//! Every row was read off Chrome 158 first. Three things are left out of the
//! comparisons on purpose: where a throw was thrown — compared only for the
//! two scripts the two engines place alike; the `preview` Chrome adds to a
//! thrown object and the `stackTrace` of a call — this bridge gives neither
//! yet; and the spelling of an id — Chrome writes `5.1`, this bridge its own
//! JSON, so the rows about an id that is none spell it this bridge's way and
//! expect Chrome's word for that kind of id.

use super::*;

/// The keys of a JSON object.
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// `reply["result"]`, without its handle.
fn looks(reply: &Value) -> Value {
    let mut result = reply["result"].clone();
    if let Some(object) = result.as_object_mut() {
        object.remove("objectId");
    }
    result
}

fn without(object: &mut Value, keys: &[&str]) {
    if let Some(object) = object.as_object_mut() {
        for key in keys {
            object.remove(*key);
        }
    }
}

/// A reply with its handles and previews left out — and a throw's number,
/// place, script and stack. What is left is what is compared.
fn shape(reply: &Value) -> Result<Value, (i64, String)> {
    let mut reply = reply.clone();
    if let Some(result) = reply.get_mut("result") {
        without(result, &["objectId", "preview"]);
    }
    if let Some(details) = reply.get_mut("exceptionDetails") {
        without(
            details,
            &[
                "exceptionId",
                "lineNumber",
                "columnNumber",
                "scriptId",
                "stackTrace",
            ],
        );
        if let Some(exception) = details.get_mut("exception") {
            without(exception, &["objectId", "preview"]);
        }
    }
    Ok(reply)
}

/// An error handle as Chrome describes it.
fn error(class: &str, description: &str) -> Value {
    json!({
        "type": "object",
        "subtype": "error",
        "className": class,
        "description": description,
    })
}

/// The reply Chrome gives: a result alone — or with the exception thrown
/// beside it, under what Chrome wrote of it.
fn answer(result: Value) -> Result<Value, (i64, String)> {
    Ok(json!({ "result": result }))
}

fn throwing(result: Value, text: &str, exception: Value) -> Result<Value, (i64, String)> {
    Ok(json!({ "result": result, "exceptionDetails": { "text": text, "exception": exception } }))
}

/// A refusal, in Chrome's words: `-32000`.
fn said(message: &str) -> Result<Value, (i64, String)> {
    Err((-32000, message.to_string()))
}

/// Parameters Chrome will not read: `-32602`.
fn not_read(message: &str) -> Result<Value, (i64, String)> {
    Err((-32602, message.to_string()))
}

const LOST: &str = "Could not find object with given id";
const NO_ID: &str = "Invalid remote object id";
const NO_WORLD: &str = "Cannot find context with specified id";
const NOT_A_FUNCTION: &str = "Given expression does not evaluate to a function";
const NO_TARGET: &str =
    "Either objectId or executionContextId or uniqueContextId must be specified";
const TWO_TARGETS: &str =
    "ObjectId, executionContextId and uniqueContextId must mutually exclude each other";

/// The params of a `Runtime.callFunctionOn`: `declaration`, aimed by the
/// keys of `target` — a handle, a world, or a world's `uniqueId` — and the
/// rest.
fn call(declaration: &str, target: Value, rest: Value) -> Value {
    let mut params = rest;
    params["functionDeclaration"] = json!(declaration);
    for (key, value) in target.as_object().into_iter().flatten() {
        params[key] = value.clone();
    }
    params
}

/// `Runtime.callFunctionOn` of `declaration` on `target`: its reply — or the
/// code and message of its error.
async fn calling(
    client: &mut Client,
    session: &str,
    declaration: &str,
    target: Value,
    rest: Value,
) -> Result<Value, (i64, String)> {
    client
        .ask(
            "Runtime.callFunctionOn",
            call(declaration, target, rest),
            Some(session),
        )
        .await
}

fn on_handle(id: &str) -> Value {
    json!({ "objectId": id })
}

fn in_world(context: i64) -> Value {
    json!({ "executionContextId": context })
}

/// The last world `Runtime.enable` announced, as the probe that read these
/// rows off Chrome took it: the page's main world.
async fn main_world(client: &mut Client, session: &str) -> i64 {
    client.eval("1", session).await;
    main_context(client)
}

#[tokio::test]
async fn a_handle_says_what_it_is() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    client
        .run(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "UTC" }),
            on,
        )
        .await;
    let a_div = "(function(){ var d = document.createElement('div'); d.id = 'foo'; \
                 d.className = 'bar baz'; return d; })()";
    let an_instance = "(function(){ class P { constructor(){ this.a = 1; } } return new P(); })()";
    let a = |subtype: &str, class: &str, description: &str| json!({ "type": "object", "subtype": subtype, "className": class, "description": description });
    let a_function = |class: &str, description: &str| json!({ "type": "function", "className": class, "description": description });
    for (expression, said) in [
        (
            "new Error('e')",
            error("Error", "Error: e\n    at <anonymous>:1:1"),
        ),
        (
            "new TypeError('bad')",
            error("TypeError", "TypeError: bad\n    at <anonymous>:1:1"),
        ),
        (
            "(function(){ function inner(){ throw new RangeError('deep'); } \
             try { inner(); } catch (e) { return e; } })()",
            error(
                "RangeError",
                "RangeError: deep\n    at inner (<anonymous>:1:38)\n    at <anonymous>:1:70\n    \
                 at <anonymous>:1:107",
            ),
        ),
        (
            "new RangeError()",
            error("RangeError", "RangeError\n    at <anonymous>:1:1"),
        ),
        (
            "new DOMException('nope', 'NotSupportedError')",
            error("DOMException", "NotSupportedError: nope"),
        ),
        ("/ab/gi", a("regexp", "RegExp", "/ab/gi")),
        ("/a\\/b/", a("regexp", "RegExp", "/a\\/b/")),
        ("new Map([[1,2],[3,4]])", a("map", "Map", "Map(2)")),
        ("new Set([1,2,3])", a("set", "Set", "Set(3)")),
        ("new WeakMap()", a("weakmap", "WeakMap", "WeakMap")),
        (
            "new Uint8Array([1,2])",
            a("typedarray", "Uint8Array", "Uint8Array(2)"),
        ),
        (
            "new ArrayBuffer(8)",
            a("arraybuffer", "ArrayBuffer", "ArrayBuffer(8)"),
        ),
        (
            "new DataView(new ArrayBuffer(8))",
            a("dataview", "DataView", "DataView(8)"),
        ),
        ("Promise.resolve(1)", a("promise", "Promise", "Promise")),
        ("(class Foo {})", a_function("Function", "class Foo {}")),
        (
            "(async () => 1)",
            a_function("AsyncFunction", "async () => 1"),
        ),
        ("()=>{}", a_function("Function", "()=>{}")),
        (
            "Array.prototype.slice",
            a_function("Function", "function slice() { [native code] }"),
        ),
        (
            "Function('a', 'return a')",
            a_function("Function", "function anonymous(a\n) {\nreturn a\n}"),
        ),
        (a_div, a("node", "HTMLDivElement", "div#foo.bar.baz")),
        (
            "document.createComment('c')",
            a("node", "Comment", "#comment"),
        ),
        (
            "document.querySelectorAll('body')",
            a("array", "NodeList", "NodeList(1)"),
        ),
        (
            "(function(){ return arguments; })(1)",
            a("array", "Arguments", "Arguments(1)"),
        ),
        (
            an_instance,
            json!({ "type": "object", "className": "P", "description": "P" }),
        ),
        (
            "window",
            json!({ "type": "object", "className": "Window", "description": "Window" }),
        ),
        (
            "location",
            json!({ "type": "object", "className": "Location", "description": "Location" }),
        ),
        (
            "Math",
            json!({ "type": "object", "className": "Math", "description": "Math" }),
        ),
    ] {
        let reply = client
            .run("Runtime.evaluate", json!({ "expression": expression }), on)
            .await;
        assert_eq!(keys(&reply), ["result"], "{expression}");
        assert_eq!(looks(&reply), said, "{expression}");
        let held = reply["result"]["objectId"].as_str().unwrap_or("");
        assert!(!held.is_empty(), "{expression}");
        assert!(reply["result"].get("preview").is_none(), "{expression}");
    }

    // A date reads its zone: pinned, and read up to the zone's name — which
    // is written in the reader's language.
    let reply = client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "new Date(0)" }),
            on,
        )
        .await;
    assert_eq!(keys(&reply), ["result"]);
    assert_eq!(
        (
            reply["result"]["type"].as_str(),
            reply["result"]["subtype"].as_str(),
            reply["result"]["className"].as_str(),
        ),
        (Some("object"), Some("date"), Some("Date"))
    );
    let read = reply["result"]["description"].as_str().unwrap();
    assert!(
        read.starts_with("Thu Jan 01 1970 00:00:00 GMT+0000 ("),
        "{read}"
    );

    // What has no handle comes back whole, as Chrome writes it.
    for (expression, whole) in [
        (
            "NaN",
            json!({ "type": "number", "unserializableValue": "NaN", "description": "NaN" }),
        ),
        (
            "-0",
            json!({ "type": "number", "unserializableValue": "-0", "description": "-0" }),
        ),
        (
            "10n",
            json!({ "type": "bigint", "unserializableValue": "10n", "description": "10n" }),
        ),
        (
            "1e21",
            json!({ "type": "number", "value": 1e21, "description": "1e+21" }),
        ),
        (
            "Number.MAX_SAFE_INTEGER",
            json!({ "type": "number", "value": 9007199254740991i64, "description": "9007199254740991" }),
        ),
        (
            "0.1 + 0.2",
            json!({ "type": "number", "value": 0.30000000000000004, "description": "0.30000000000000004" }),
        ),
        (
            "Number.MIN_VALUE",
            json!({ "type": "number", "value": 5e-324, "description": "5e-324" }),
        ),
        ("undefined", json!({ "type": "undefined" })),
        ("'text'", json!({ "type": "string", "value": "text" })),
        ("true", json!({ "type": "boolean", "value": true })),
        (
            "40 + 2",
            json!({ "type": "number", "value": 42, "description": "42" }),
        ),
    ] {
        assert_eq!(
            shape(
                &client
                    .run("Runtime.evaluate", json!({ "expression": expression }), on)
                    .await
            ),
            answer(whole),
            "{expression}"
        );
    }
}

#[tokio::test]
async fn a_value_asked_for_comes_back_as_chrome_brings_one() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let a_value = |value: Value| answer(json!({ "type": "object", "value": value }));
    for (expression, expected) in [
        (
            "({a: 1, b: [1, 2], c: {d: null}})",
            a_value(json!({ "a": 1, "b": [1, 2], "c": { "d": null } })),
        ),
        ("({a: {b: undefined}})", a_value(json!({ "a": {} }))),
        ("({c: -0})", a_value(json!({ "c": 0 }))),
        ("({x: -Infinity})", a_value(json!({ "x": null }))),
        ("({x: [1, NaN]})", a_value(json!({ "x": [1, null] }))),
        ("[[1, , 3]]", a_value(json!([[1, null, 3]]))),
        (
            "Object.assign(Object.create({proto: 1}), {own: 2})",
            a_value(json!({ "own": 2 })),
        ),
        (
            "({toJSON(){ return 5; }})",
            a_value(json!({ "toJSON": {} })),
        ),
        (
            "Object.assign(new Date(0), {a: 1})",
            a_value(json!({ "a": 1 })),
        ),
        (
            "(function(){ var s = {x: 1}; return {a: s, b: s}; })()",
            a_value(json!({ "a": { "x": 1 }, "b": { "x": 1 } })),
        ),
        ("({get g(){ return NaN; }})", a_value(json!({ "g": null }))),
        (
            "(function(){ class P { constructor(){ this.a = 1; } } return new P(); })()",
            a_value(json!({ "a": 1 })),
        ),
        (
            "new Proxy({}, {get(){ throw new TypeError('no'); }})",
            a_value(json!({})),
        ),
        (
            "document.querySelector('nope')",
            answer(json!({ "type": "object", "subtype": "null", "value": null })),
        ),
    ] {
        assert_eq!(
            shape(
                &client
                    .run(
                        "Runtime.evaluate",
                        json!({ "expression": expression, "returnByValue": true }),
                        on,
                    )
                    .await
            ),
            expected,
            "{expression}"
        );
    }
}

#[tokio::test]
async fn what_cannot_be_brought_back_by_value_is_chromes_word() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let internal = || Err((-32603, "Internal error".to_string()));
    for (expression, expected) in [
        (
            "({a: NaN, b: Infinity, c: -0, d: 10n})",
            said("Object couldn't be returned by value"),
        ),
        ("({d: 10n})", said("Object couldn't be returned by value")),
        (
            "({g: Symbol('s')})",
            said("Object couldn't be returned by value"),
        ),
        ("Symbol('x')", said("Object couldn't be returned by value")),
        (
            "(function(){var o={};o.self=o;return o;})()",
            said("Object reference chain is too long"),
        ),
        ("globalThis", said("Object reference chain is too long")),
        ("({get x(){throw new Error('boom')}})", internal()),
        ("({a: {get x(){ throw new Error('boom'); }}})", internal()),
    ] {
        assert_eq!(
            client
                .ask(
                    "Runtime.evaluate",
                    json!({ "expression": expression, "returnByValue": true }),
                    on,
                )
                .await,
            expected,
            "{expression}"
        );
    }
}

/// The params of a `Runtime.evaluate`: `expression`, and the rest.
fn asking(expression: &str, rest: Value) -> Value {
    let mut params = rest;
    params["expression"] = json!(expression);
    params
}

/// A whole number, as Chrome writes one back.
fn number(n: i64) -> Value {
    json!({ "type": "number", "value": n, "description": n.to_string() })
}

#[tokio::test]
async fn a_script_that_throws_is_shown_not_refused() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let the_error = error("TypeError", "TypeError: outer\n    at <anonymous>:1:7");
    // Asked back by value or not: a throw is the result, and the exception.
    for by_value in [false, true] {
        let reply = client
            .run(
                "Runtime.evaluate",
                asking(
                    "throw new TypeError('outer')",
                    json!({ "returnByValue": by_value }),
                ),
                on,
            )
            .await;
        assert_eq!(
            shape(&reply),
            throwing(the_error.clone(), "Uncaught", the_error.clone()),
            "by value: {by_value}"
        );
    }

    // The two scripts the two engines place alike: where, and in what script.
    for (expression, described, at) in [
        (
            "null.x",
            "TypeError: Cannot read properties of null (reading 'x')\n    at <anonymous>:1:6",
            (0, 5),
        ),
        (
            "notDefined",
            "ReferenceError: notDefined is not defined\n    at <anonymous>:1:1",
            (0, 0),
        ),
    ] {
        let reply = client
            .run("Runtime.evaluate", asking(expression, json!({})), on)
            .await;
        assert_eq!(looks(&reply)["description"], described, "{expression}");
        let details = &reply["exceptionDetails"];
        assert_eq!(
            (
                details["lineNumber"].as_i64(),
                details["columnNumber"].as_i64(),
            ),
            (Some(at.0), Some(at.1)),
            "{expression}"
        );
        assert!(
            details["scriptId"]
                .as_str()
                .is_some_and(|id| !id.is_empty()),
            "{expression}"
        );
    }

    // What was thrown is the result — a number with its description on one
    // side and without it on the other, as Chrome writes it.
    let a_string = json!({ "type": "string", "value": "plain" });
    let a_null = json!({ "type": "object", "subtype": "null", "value": null });
    let an_undefined = json!({ "type": "undefined" });
    let a_plain_object =
        json!({ "type": "object", "className": "Object", "description": "Object" });
    for (expression, shown, exception) in [
        ("throw 'plain'", a_string.clone(), a_string.clone()),
        ("throw null", a_null.clone(), a_null),
        (
            "throw undefined",
            an_undefined.clone(),
            an_undefined.clone(),
        ),
        ("throw 42", number(42), number(42)),
    ] {
        let reply = client
            .run("Runtime.evaluate", asking(expression, json!({})), on)
            .await;
        assert_eq!(
            shape(&reply),
            throwing(shown, "Uncaught", exception),
            "{expression}"
        );
    }
    let unexpected_end = error("SyntaxError", "SyntaxError: Unexpected end of input");
    for (expression, shown) in [
        ("throw {a: 1}", a_plain_object.clone()),
        ("(", unexpected_end.clone()),
    ] {
        let reply = client
            .run("Runtime.evaluate", asking(expression, json!({})), on)
            .await;
        assert_eq!(
            shape(&reply),
            throwing(shown.clone(), "Uncaught", shown),
            "{expression}"
        );
    }

    // A promise not waited for is a promise.
    assert_eq!(
        shape(
            &client
                .run(
                    "Runtime.evaluate",
                    asking("Promise.reject(new Error('x'))", json!({})),
                    on,
                )
                .await
        ),
        answer(json!({
            "type": "object",
            "subtype": "promise",
            "className": "Promise",
            "description": "Promise",
        }))
    );
    let refused = error("Error", "Error: x\n    at <anonymous>:1:16");
    let reply = client
        .run(
            "Runtime.evaluate",
            asking(
                "Promise.reject(new Error('x'))",
                json!({ "awaitPromise": true }),
            ),
            on,
        )
        .await;
    assert_eq!(
        shape(&reply),
        throwing(
            refused.clone(),
            "Uncaught (in promise) Error: x",
            refused.clone()
        )
    );
    // Asked back by value the rejection comes back as the value it is —
    // an error's own, which is none — while `exceptionDetails` still shows it.
    let reply = client
        .run(
            "Runtime.evaluate",
            asking(
                "Promise.reject(new Error('x'))",
                json!({ "awaitPromise": true, "returnByValue": true }),
            ),
            on,
        )
        .await;
    assert_eq!(
        shape(&reply),
        throwing(
            json!({ "type": "object", "value": {} }),
            "Uncaught (in promise) Error: x",
            refused.clone()
        )
    );
    let a_bigint = json!({ "type": "bigint", "unserializableValue": "10n", "description": "10n" });
    for (expression, shown) in [
        (
            "Promise.reject(new DOMException('nope', 'AbortError'))",
            error("DOMException", "AbortError: nope"),
        ),
        ("Promise.reject('plain')", a_string.clone()),
        ("Promise.reject(10n)", a_bigint.clone()),
    ] {
        let reply = client
            .run(
                "Runtime.evaluate",
                asking(expression, json!({ "awaitPromise": true })),
                on,
            )
            .await;
        assert_eq!(
            shape(&reply),
            throwing(shown.clone(), "Uncaught (in promise)", shown),
            "{expression}"
        );
    }

    // Waited for and asked back by value: what comes back is Chrome's — of
    // what resolved, and of what did not.
    let a_cycle = "(function(){ var o = {a: 1}; o.self = o; return o; })()";
    let a_getter = "({ get g(){ throw new Error('getter'); }, a: 1 })";
    let no_value = said("Object couldn't be returned by value");
    let too_long = said("Object reference chain is too long");
    let internal = Err((-32603, "Internal error".to_string()));
    let an_empty_object = json!({ "type": "object", "value": {} });
    let an_object = json!({ "type": "object", "value": { "a": 1 } });
    let not_a_number =
        json!({ "type": "number", "unserializableValue": "NaN", "description": "NaN" });
    for (thing, resolved, rejected) in [
        ("Symbol('x')", no_value.clone(), no_value.clone()),
        (
            "10n",
            answer(a_bigint.clone()),
            throwing(a_bigint.clone(), "Uncaught (in promise)", a_bigint),
        ),
        (a_cycle, too_long.clone(), too_long),
        (a_getter, internal.clone(), internal),
        ("({n: 10n})", no_value.clone(), no_value.clone()),
        ("({s: Symbol('x')})", no_value.clone(), no_value),
        (
            "new Error('x')",
            answer(an_empty_object.clone()),
            throwing(
                an_empty_object.clone(),
                "Uncaught (in promise) Error: x",
                refused.clone(),
            ),
        ),
        (
            "Object.assign(new Error('x'), {a: 1})",
            answer(an_object.clone()),
            throwing(
                an_object.clone(),
                "Uncaught (in promise) Error: x",
                error("Error", "Error: x\n    at <anonymous>:1:30"),
            ),
        ),
        (
            "(function f(){})",
            answer(json!({ "type": "function", "value": {} })),
            throwing(
                json!({ "type": "function", "value": {} }),
                "Uncaught (in promise)",
                json!({ "type": "function", "className": "Function", "description": "function f(){}" }),
            ),
        ),
        (
            "new DOMException('nope', 'NotSupportedError')",
            answer(an_empty_object.clone()),
            throwing(
                an_empty_object,
                "Uncaught (in promise)",
                error("DOMException", "NotSupportedError: nope"),
            ),
        ),
        (
            "({a: 1})",
            answer(an_object.clone()),
            throwing(an_object, "Uncaught (in promise)", a_plain_object),
        ),
        (
            "'str'",
            answer(json!({ "type": "string", "value": "str" })),
            throwing(
                json!({ "type": "string", "value": "str" }),
                "Uncaught (in promise)",
                json!({ "type": "string", "value": "str" }),
            ),
        ),
        (
            "42",
            answer(number(42)),
            throwing(number(42), "Uncaught (in promise)", number(42)),
        ),
        (
            "undefined",
            answer(an_undefined.clone()),
            throwing(an_undefined.clone(), "Uncaught (in promise)", an_undefined),
        ),
        (
            "NaN",
            answer(not_a_number.clone()),
            throwing(not_a_number.clone(), "Uncaught (in promise)", not_a_number),
        ),
    ] {
        for (side, expression, expected) in [
            ("resolved", format!("Promise.resolve({thing})"), resolved),
            ("rejected", format!("Promise.reject({thing})"), rejected),
        ] {
            let reply = client
                .ask(
                    "Runtime.evaluate",
                    asking(
                        &expression,
                        json!({ "awaitPromise": true, "returnByValue": true }),
                    ),
                    on,
                )
                .await
                .and_then(|reply| shape(&reply));
            assert_eq!(reply, expected, "{side} {thing}");
        }
    }

    // Thrown, asked back by value: what cannot come back stays a handle —
    // refused only inside something.
    for (expression, shown) in [
        (
            "throw Symbol('x')",
            json!({ "type": "symbol", "description": "Symbol(x)" }),
        ),
        (
            "throw (function(){ var o = {}; o.self = o; return o; })()",
            json!({ "type": "object", "className": "Object", "description": "Object" }),
        ),
    ] {
        let reply = client
            .run(
                "Runtime.evaluate",
                asking(expression, json!({ "returnByValue": true })),
                on,
            )
            .await;
        assert_eq!(
            shape(&reply),
            throwing(shown.clone(), "Uncaught", shown),
            "{expression}"
        );
    }

    // `serializationOptions` says it too: `json` asks for the value, `idOnly`
    // for the handle — a throw shown either way.
    let the_json = json!({ "serializationOptions": { "serialization": "json" } });
    let thrown_error = error("Error", "Error: x\n    at <anonymous>:1:7");
    let reply = client
        .run(
            "Runtime.evaluate",
            asking("throw new Error('x')", the_json.clone()),
            on,
        )
        .await;
    assert_eq!(
        shape(&reply),
        throwing(thrown_error.clone(), "Uncaught", thrown_error)
    );
    let mut awaited_json = the_json;
    awaited_json["awaitPromise"] = json!(true);
    let reply = client
        .run(
            "Runtime.evaluate",
            asking("Promise.reject(new Error('x'))", awaited_json),
            on,
        )
        .await;
    assert_eq!(
        shape(&reply),
        throwing(
            json!({ "type": "object", "value": {} }),
            "Uncaught (in promise) Error: x",
            refused.clone(),
        )
    );
    let reply = client
        .run(
            "Runtime.evaluate",
            asking(
                "Promise.reject(new Error('x'))",
                json!({
                    "awaitPromise": true,
                    "serializationOptions": { "serialization": "idOnly" },
                }),
            ),
            on,
        )
        .await;
    assert_eq!(
        shape(&reply),
        throwing(refused.clone(), "Uncaught (in promise) Error: x", refused)
    );
}

#[tokio::test]
async fn a_script_is_read_as_chrome_reads_one() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let a_colon = error("SyntaxError", "SyntaxError: Unexpected token ':'");
    let a_return = error("SyntaxError", "SyntaxError: Illegal return statement");
    let no_description = json!({ "type": "object", "value": { "a": 1 } });
    for (expression, expected) in [
        ("{a: 1}", answer(number(1))),
        ("  40 + 2  ", answer(number(42))),
        ("var x = 2; x * 3", answer(number(6))),
        ("// c\n({a:1})", answer(no_description)),
        ("\n\n 1 + 1", answer(number(2))),
        (";42", answer(number(42))),
        ("1 + 1 // done", answer(number(2))),
        (
            "  {\"k\": \"v\"}  ",
            throwing(a_colon.clone(), "Uncaught", a_colon.clone()),
        ),
        (
            "{\"k\": \"v\"}",
            throwing(a_colon.clone(), "Uncaught", a_colon.clone()),
        ),
        ("{a:1, b:2}", throwing(a_colon.clone(), "Uncaught", a_colon)),
        ("return 7", throwing(a_return.clone(), "Uncaught", a_return)),
    ] {
        let reply = client
            .run(
                "Runtime.evaluate",
                asking(expression, json!({ "returnByValue": true })),
                on,
            )
            .await;
        assert_eq!(shape(&reply), expected, "{expression}");
    }
}

#[tokio::test]
async fn call_function_on_runs_on_the_handle() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let context = main_world(&mut client, &session).await;
    let object = handle_of(&mut client, &session, "({n: 41})").await;
    let argument = handle_of(&mut client, &session, "({n: 1})").await;
    let one = "function(){ return 1 }";
    let by_value = json!({ "returnByValue": true });
    for (declaration, rest, expected) in [
        (
            "(o) => o.n + this.n",
            json!({ "arguments": [on_handle(&argument)], "returnByValue": true }),
            answer(json!({ "type": "number", "unserializableValue": "NaN", "description": "NaN" })),
        ),
        (
            "function(){ return 1 }",
            by_value.clone(),
            answer(number(1)),
        ),
        (
            "function(){ return this }",
            by_value.clone(),
            answer(json!({ "type": "object", "value": { "n": 41 } })),
        ),
        (
            "function(a, b){ return a + b }",
            json!({ "arguments": [{ "value": 40 }, { "value": 2 }], "returnByValue": true }),
            answer(number(42)),
        ),
        (
            "async function(){ return 7 }",
            json!({ "returnByValue": true, "awaitPromise": true }),
            answer(number(7)),
        ),
    ] {
        let reply = calling(
            &mut client,
            &session,
            declaration,
            on_handle(&object),
            rest.clone(),
        )
        .await;
        assert_eq!(reply, expected, "{declaration} {rest}");
    }
    let reply = calling(
        &mut client,
        &session,
        "function(){ return this }",
        on_handle(&object),
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        looks(&reply),
        json!({ "type": "object", "className": "Object", "description": "Object" })
    );
    assert!(!reply["result"]["objectId"]
        .as_str()
        .unwrap_or("")
        .is_empty());

    // A declaration that will not read, or a body that throws: shown.
    let no_parenthesis = error("SyntaxError", "SyntaxError: Unexpected token ')'");
    let reply = calling(&mut client, &session, "(", on_handle(&object), json!({}))
        .await
        .unwrap();
    assert_eq!(
        shape(&reply),
        throwing(no_parenthesis.clone(), "Uncaught", no_parenthesis)
    );
    let in_call = error(
        "TypeError",
        "TypeError: in call\n    at Object.<anonymous> (<anonymous>:1:20)",
    );
    let reply = calling(
        &mut client,
        &session,
        "function(){ throw new TypeError('in call') }",
        on_handle(&object),
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        shape(&reply),
        throwing(in_call.clone(), "Uncaught", in_call)
    );

    // What it runs on: one of a handle, a world, a world's `uniqueId` — and
    // no two of them.
    assert_eq!(
        calling(&mut client, &session, one, json!({}), json!({})).await,
        not_read(NO_TARGET)
    );
    let unique_id = client
        .events
        .iter()
        .rev()
        .find(|event| event["method"] == "Runtime.executionContextCreated")
        .and_then(|event| event["params"]["context"]["uniqueId"].as_str())
        .expect("a world was announced")
        .to_string();
    for target in [
        json!({ "objectId": object, "executionContextId": context }),
        json!({ "objectId": object, "uniqueContextId": unique_id }),
        json!({ "executionContextId": context, "uniqueContextId": unique_id }),
        json!({ "objectId": object, "executionContextId": context, "uniqueContextId": unique_id }),
    ] {
        assert_eq!(
            calling(&mut client, &session, one, target.clone(), json!({})).await,
            not_read(TWO_TARGETS),
            "{target}"
        );
    }
    for target in [
        json!({ "objectId": null }),
        json!({ "objectId": null, "executionContextId": context }),
    ] {
        assert_eq!(
            calling(&mut client, &session, one, target.clone(), json!({})).await,
            not_read("Invalid parameters"),
            "{target}"
        );
    }
    for wrong in [json!(0), json!(99)] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                one,
                json!({ "executionContextId": wrong }),
                json!({}),
            )
            .await,
            said(NO_WORLD),
            "{wrong}"
        );
    }
    for wrong in [json!(null), json!(1.5)] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                one,
                json!({ "executionContextId": wrong }),
                json!({}),
            )
            .await,
            not_read("Invalid parameters"),
            "{wrong}"
        );
    }
    assert_eq!(
        calling(
            &mut client,
            &session,
            one,
            json!({ "uniqueContextId": unique_id }),
            json!({}),
        )
        .await,
        answer(number(1))
    );
    assert_eq!(
        shape(
            &client
                .run(
                    "Runtime.evaluate",
                    json!({ "expression": "1", "uniqueContextId": unique_id }),
                    on,
                )
                .await
        ),
        answer(number(1))
    );
    for wrong in [context.to_string(), "nope".to_string(), String::new()] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                one,
                json!({ "uniqueContextId": wrong }),
                json!({}),
            )
            .await,
            not_read("invalid uniqueContextId"),
            "{wrong:?}"
        );
        assert_eq!(
            client
                .ask(
                    "Runtime.evaluate",
                    json!({ "expression": "1", "uniqueContextId": wrong }),
                    on,
                )
                .await,
            not_read("invalid uniqueContextId"),
            "{wrong:?}"
        );
    }
    assert_eq!(
        client
            .ask(
                "Runtime.evaluate",
                json!({ "expression": "1", "contextId": context, "uniqueContextId": unique_id }),
                on,
            )
            .await,
        not_read("contextId and uniqueContextId are mutually exclusive")
    );

    // Let go, it names nothing.
    assert_eq!(
        client
            .run("Runtime.releaseObject", on_handle(&object), on)
            .await,
        json!({})
    );
    assert_eq!(
        calling(&mut client, &session, one, on_handle(&object), json!({})).await,
        said(LOST)
    );
}

#[tokio::test]
async fn an_argument_is_read_by_what_keys_it_has() {
    let (mut client, session) = bridge("about:blank").await;
    let world = in_world(main_world(&mut client, &session).await);
    let live = handle_of(&mut client, &session, "({tag: 'live'})").await;
    let let_go = handle_of(&mut client, &session, "({tag: 'dead'})").await;
    client
        .run(
            "Runtime.releaseObject",
            on_handle(&let_go),
            Some(session.as_str()),
        )
        .await;
    let a_string = |text: &str| answer(json!({ "type": "string", "value": text }));
    let str_of = "function(a){ return String(a) }";
    let tag_of = "function(a){ return a && a.tag }";
    // A handle before a value: what it names is the argument, whatever else
    // is written beside it.
    for (declaration, argument, expected) in [
        (
            str_of,
            json!({ "value": 5, "unserializableValue": "NaN" }),
            a_string("5"),
        ),
        (
            str_of,
            json!({ "value": null, "unserializableValue": "NaN" }),
            a_string("null"),
        ),
        (
            str_of,
            json!({ "value": false, "unserializableValue": "NaN" }),
            a_string("false"),
        ),
        (
            str_of,
            json!({ "value": 0, "unserializableValue": "1+1" }),
            a_string("0"),
        ),
        (
            str_of,
            json!({ "value": { "a": 1 }, "unserializableValue": "NaN" }),
            a_string("[object Object]"),
        ),
        (
            tag_of,
            json!({ "objectId": live, "unserializableValue": "NaN" }),
            a_string("live"),
        ),
        (
            tag_of,
            json!({ "objectId": live, "value": 5 }),
            a_string("live"),
        ),
        (
            tag_of,
            json!({ "objectId": live, "value": 5, "unserializableValue": "NaN" }),
            a_string("live"),
        ),
        (
            str_of,
            json!({ "objectId": let_go, "value": 5 }),
            said(LOST),
        ),
        (
            str_of,
            json!({ "objectId": let_go, "unserializableValue": "NaN" }),
            said(LOST),
        ),
        (
            str_of,
            json!({ "objectId": let_go, "value": null }),
            said(LOST),
        ),
        (
            str_of,
            json!({ "objectId": "junk", "value": 5 }),
            said(NO_ID),
        ),
        (
            str_of,
            json!({ "objectId": "junk", "unserializableValue": "NaN" }),
            said(NO_ID),
        ),
        (str_of, json!({ "objectId": "" }), said(NO_ID)),
        (
            str_of,
            json!({ "unserializableValue": "bogus" }),
            said("Couldn't parse value object in call argument"),
        ),
        (
            "function(a){ return typeof a }",
            json!({}),
            a_string("undefined"),
        ),
    ] {
        let reply = calling(
            &mut client,
            &session,
            declaration,
            world.clone(),
            json!({ "arguments": [argument.clone()], "returnByValue": true }),
        )
        .await;
        assert_eq!(reply, expected, "{declaration} {argument}");
    }
    // A value nested comes whole; and what is not an argument at all is not read.
    assert_eq!(
        calling(
            &mut client,
            &session,
            "function(a){ return a }",
            world.clone(),
            json!({ "arguments": [{ "value": { "a": 1 } }], "returnByValue": true }),
        )
        .await,
        answer(json!({ "type": "object", "value": { "a": 1 } }))
    );
    assert_eq!(
        calling(
            &mut client,
            &session,
            "function(){ return 1 }",
            world.clone(),
            json!({ "arguments": [] }),
        )
        .await,
        answer(number(1))
    );
    for (declaration, arguments) in [
        ("function(){ return 1 }", json!(null)),
        ("function(){ return 1 }", json!({})),
        ("function(){ return 1 }", json!([null])),
        ("function(){ return 1 }", json!([5])),
        ("function(a){ return typeof a }", json!([{ "a": 1 }])),
        ("function(a){ return typeof a }", json!([{ "objectId": 5 }])),
    ] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                declaration,
                world.clone(),
                json!({ "arguments": arguments.clone() }),
            )
            .await,
            not_read("Invalid parameters"),
            "{declaration} {arguments}"
        );
    }
}

const OTHER_WORLD: &str = "Argument should belong to the same JavaScript world as target object";

#[tokio::test]
async fn an_id_that_names_nothing_is_chromes_word() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let world = in_world(main_world(&mut client, &session).await);
    let one = "function(){ return 1 }";
    let object = handle_of(&mut client, &session, "({n: 41})").await;
    let let_go = handle_of(&mut client, &session, "({n: 1})").await;
    client
        .run("Runtime.releaseObject", on_handle(&let_go), on)
        .await;
    let numbered = |instead: &str| object.replace("\"id\":", &format!("\"id\":{instead}"));
    let realm = |instead: &str| {
        object.replace(
            "\"injectedScriptId\":",
            &format!("\"injectedScriptId\":{instead}"),
        )
    };

    // A blank in front of an id is no part of it: it is the object.
    let padded = format!("  {object}");
    assert_eq!(
        calling(&mut client, &session, one, on_handle(&padded), json!({})).await,
        answer(number(1))
    );
    assert_eq!(
        calling(
            &mut client,
            &session,
            "function(a){ return a.n }",
            world.clone(),
            json!({ "arguments": [on_handle(&padded)], "returnByValue": true }),
        )
        .await,
        answer(number(41))
    );
    assert_eq!(
        client
            .ask("Runtime.releaseObject", on_handle(&padded), on)
            .await,
        Ok(json!({}))
    );

    // The spelling of an id this bridge gave, and nothing behind it; and the
    // spelling of one of a world that is gone.
    let (_, other_session) = open_owned_page(&mut client, 50).await;
    let foreign = handle_of(&mut client, &other_session, "({})").await;
    for (id, as_target, as_argument) in [
        (let_go.clone(), LOST, LOST),
        (numbered("\"\""), LOST, LOST),
        (numbered("0"), LOST, LOST),
        (numbered("-1"), LOST, LOST),
        (numbered("1000000"), LOST, LOST),
        // Chrome's number reader takes `01` as 1 and `+1` as 1 — leading
        // blanks, a sign and zeroes are no reason to refuse the id.
        (numbered("01"), LOST, LOST),
        (numbered("+1"), LOST, LOST),
        (realm("\"\""), NO_WORLD, OTHER_WORLD),
        (realm("999999"), NO_WORLD, OTHER_WORLD),
        (foreign, NO_WORLD, OTHER_WORLD),
    ] {
        assert_eq!(
            calling(&mut client, &session, one, on_handle(&id), json!({})).await,
            said(as_target),
            "as the target: {id}"
        );
        assert_eq!(
            calling(
                &mut client,
                &session,
                one,
                world.clone(),
                json!({ "arguments": [on_handle(&id)] }),
            )
            .await,
            said(as_argument),
            "as an argument: {id}"
        );
        assert_eq!(
            client
                .ask("Runtime.releaseObject", on_handle(&id), on)
                .await,
            said(as_target),
            "let go: {id}"
        );
    }
    // No id at all.
    for id in [
        format!("{object} "),
        format!("{object}x"),
        "x".to_string(),
        String::new(),
        NO_SUCH_ID.to_string(),
        "junk".to_string(),
        numbered("null"),
        numbered("1.5"),
        numbered("1.0"),
        numbered("1e400"),
        numbered("0x10"),
        numbered("\"13\""),
    ] {
        assert_eq!(
            calling(&mut client, &session, one, on_handle(&id), json!({})).await,
            said(NO_ID),
            "as the target: {id}"
        );
        assert_eq!(
            calling(
                &mut client,
                &session,
                one,
                world.clone(),
                json!({ "arguments": [on_handle(&id)] }),
            )
            .await,
            said(NO_ID),
            "as an argument: {id}"
        );
        assert_eq!(
            client
                .ask("Runtime.releaseObject", on_handle(&id), on)
                .await,
            said(NO_ID),
            "let go: {id}"
        );
    }

    // The page went elsewhere: the world before is gone.
    client
        .run("Page.navigate", json!({ "url": "about:blank" }), on)
        .await;
    let fresh = handle_of(&mut client, &session, "({})").await;
    assert_eq!(
        calling(&mut client, &session, one, on_handle(&object), json!({})).await,
        said(NO_WORLD)
    );
    assert_eq!(
        calling(
            &mut client,
            &session,
            one,
            on_handle(&fresh),
            json!({ "arguments": [on_handle(&object)] }),
        )
        .await,
        said(OTHER_WORLD)
    );
    assert_eq!(
        client
            .ask("Runtime.releaseObject", on_handle(&object), on)
            .await,
        said(NO_WORLD)
    );
}

#[tokio::test]
async fn the_error_that_comes_first_is_chromes() {
    let (mut client, session) = bridge("about:blank").await;
    let world = in_world(main_world(&mut client, &session).await);
    let live = handle_of(&mut client, &session, "({tag: 'live'})").await;
    let other = handle_of(&mut client, &session, "({tag: 'other'})").await;
    let let_go = handle_of(&mut client, &session, "({tag: 'dead'})").await;
    client
        .run(
            "Runtime.releaseObject",
            on_handle(&let_go),
            Some(session.as_str()),
        )
        .await;
    let one = "function(){ return 1 }";
    let not_one = "1";
    // The handle it runs on, then each argument in its turn — a handle before
    // what is written beside it — and only then whether it is a function.
    for (declaration, target, rest, expected) in [
        (not_one, on_handle(&let_go), json!({}), said(LOST)),
        (
            not_one,
            world.clone(),
            json!({ "arguments": [on_handle(&let_go)] }),
            said(LOST),
        ),
        (
            not_one,
            world.clone(),
            json!({ "arguments": [on_handle("junk")] }),
            said(NO_ID),
        ),
        (
            not_one,
            world.clone(),
            json!({ "arguments": [{ "unserializableValue": "bogus" }] }),
            said("Couldn't parse value object in call argument"),
        ),
        (
            not_one,
            world.clone(),
            json!({ "awaitPromise": true }),
            said(NOT_A_FUNCTION),
        ),
        (
            not_one,
            world.clone(),
            json!({ "returnByValue": true }),
            said(NOT_A_FUNCTION),
        ),
        (not_one, on_handle(&live), json!({}), said(NOT_A_FUNCTION)),
        (
            not_one,
            world.clone(),
            json!({ "arguments": [on_handle(&other)] }),
            said(NOT_A_FUNCTION),
        ),
        (
            "(function(){ throw new RangeError('d') })()",
            world.clone(),
            json!({ "arguments": [on_handle(&let_go)] }),
            said(LOST),
        ),
        (
            "(",
            world.clone(),
            json!({ "arguments": [on_handle(&let_go)] }),
            said(LOST),
        ),
        (
            one,
            on_handle("junk"),
            json!({ "arguments": [on_handle(&let_go)] }),
            said(NO_ID),
        ),
        (
            one,
            on_handle(&let_go),
            json!({ "arguments": [on_handle("junk")] }),
            said(LOST),
        ),
        (
            one,
            world.clone(),
            json!({ "arguments": [on_handle("junk"), on_handle(&let_go)] }),
            said(NO_ID),
        ),
        (
            one,
            world.clone(),
            json!({ "arguments": [on_handle(&let_go), on_handle("junk")] }),
            said(LOST),
        ),
        (
            one,
            world.clone(),
            json!({ "arguments": [on_handle(&let_go), { "unserializableValue": "bogus" }] }),
            said(LOST),
        ),
        (
            one,
            world.clone(),
            json!({ "arguments": [{ "unserializableValue": "bogus" }, on_handle(&let_go)] }),
            said("Couldn't parse value object in call argument"),
        ),
    ] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                declaration,
                target.clone(),
                rest.clone()
            )
            .await,
            expected,
            "{declaration} {target} {rest}"
        );
    }
}

#[tokio::test]
async fn a_declaration_that_is_no_function_is_said_so() {
    let (mut client, session) = bridge("about:blank").await;
    let world = in_world(main_world(&mut client, &session).await);
    let live = handle_of(&mut client, &session, "({tag: 'live'})").await;
    for declaration in [
        "undefined",
        "null",
        "({})",
        "[]",
        "document",
        "Symbol.iterator",
        "typeof 1",
    ] {
        assert_eq!(
            calling(&mut client, &session, declaration, world.clone(), json!({})).await,
            said(NOT_A_FUNCTION),
            "{declaration}"
        );
    }
    // What is called is what the declaration says — a class no way, a
    // generator not yet run, a proxy of a function nothing.
    let no_new = error(
        "TypeError",
        "TypeError: Class constructors cannot be invoked without 'new'",
    );
    let no_semicolon = error("SyntaxError", "SyntaxError: Unexpected token ';'");
    let deep = error(
        "RangeError",
        "RangeError: d\n    at <anonymous>:1:21\n    at <anonymous>:1:43",
    );
    let in_call = error("RangeError", "RangeError: d\n    at <anonymous>:1:20");
    let no_boom = error(
        "ReferenceError",
        "ReferenceError: boom is not defined\n    at <anonymous>:1:1",
    );
    let no_parenthesis = error("SyntaxError", "SyntaxError: Unexpected token ')'");
    let an_undefined = json!({ "type": "undefined" });
    let a_generator = json!({
        "type": "object",
        "subtype": "generator",
        "className": "Generator",
        "description": "Generator",
    });
    let a_promise = json!({
        "type": "object",
        "subtype": "promise",
        "className": "Promise",
        "description": "Promise",
    });
    for (declaration, target, rest, expected) in [
        ("() => 2", world.clone(), json!({}), answer(number(2))),
        (
            "(function(){ return 3 })",
            world.clone(),
            json!({}),
            answer(number(3)),
        ),
        (
            "1, function(){ return 7 }",
            world.clone(),
            json!({}),
            answer(number(7)),
        ),
        (
            "class {}",
            world.clone(),
            json!({}),
            throwing(no_new.clone(), "Uncaught", no_new),
        ),
        (
            "function*(){}",
            world.clone(),
            json!({}),
            answer(a_generator.clone()),
        ),
        (
            "async function(){}",
            world.clone(),
            json!({}),
            answer(a_promise),
        ),
        (
            "new Proxy(function(){}, {})",
            world.clone(),
            json!({}),
            answer(an_undefined.clone()),
        ),
        (
            "Function.prototype",
            world.clone(),
            json!({}),
            answer(an_undefined.clone()),
        ),
        (
            "Object.getPrototypeOf(function(){})",
            world.clone(),
            json!({}),
            answer(an_undefined),
        ),
        (
            "function(){}; 1",
            world.clone(),
            json!({}),
            throwing(no_semicolon.clone(), "Uncaught", no_semicolon),
        ),
        (
            "(function(){ throw new RangeError('d') })()",
            world.clone(),
            json!({}),
            throwing(deep.clone(), "Uncaught", deep),
        ),
        (
            "(boom())",
            world.clone(),
            json!({}),
            throwing(no_boom.clone(), "Uncaught", no_boom),
        ),
        (
            "function(){ throw new RangeError('d') }",
            world.clone(),
            json!({ "returnByValue": true }),
            throwing(in_call.clone(), "Uncaught", in_call.clone()),
        ),
        (
            "function(){ throw new RangeError('d') }",
            world.clone(),
            json!({ "awaitPromise": true }),
            throwing(in_call.clone(), "Uncaught", in_call),
        ),
        (
            "(",
            world.clone(),
            json!({ "awaitPromise": true }),
            throwing(no_parenthesis.clone(), "Uncaught", no_parenthesis.clone()),
        ),
        (
            "(",
            world.clone(),
            json!({ "returnByValue": true }),
            throwing(no_parenthesis.clone(), "Uncaught", no_parenthesis.clone()),
        ),
        (
            "",
            world.clone(),
            json!({}),
            throwing(no_parenthesis.clone(), "Uncaught", no_parenthesis),
        ),
        (
            "function*(){ yield 1 }",
            world.clone(),
            json!({ "awaitPromise": true }),
            answer(a_generator),
        ),
        (
            "async function(){ return 9 }",
            world.clone(),
            json!({ "awaitPromise": true }),
            answer(number(9)),
        ),
        (
            "function(){ return 4 }",
            world.clone(),
            json!({ "awaitPromise": true }),
            answer(number(4)),
        ),
        (
            "function(){ return this === globalThis }",
            world.clone(),
            json!({ "returnByValue": true }),
            answer(json!({ "type": "boolean", "value": true })),
        ),
        (
            "() => this",
            on_handle(&live),
            json!({ "returnByValue": true }),
            said("Object reference chain is too long"),
        ),
        (
            "function(){ return this }",
            on_handle(&live),
            json!({ "returnByValue": true }),
            answer(json!({ "type": "object", "value": { "tag": "live" } })),
        ),
    ] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                declaration,
                target.clone(),
                rest.clone()
            )
            .await
            .and_then(|reply| shape(&reply)),
            expected,
            "{declaration} {rest}"
        );
    }
}

#[tokio::test]
async fn serialization_options_say_whether_the_value_is_asked_for() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let world = in_world(main_world(&mut client, &session).await);
    let returns = "function(){ return {a:1} }";
    let returns_deep = "function(){ return {a:{b:1}} }";
    let a_handle = json!({ "type": "object", "className": "Object", "description": "Object" });
    let an_object = json!({ "type": "object", "value": { "a": 1 } });
    let a_deep_object = json!({ "type": "object", "value": { "a": { "b": 1 } } });
    // On a call: the option says it, `returnByValue` notwithstanding — and
    // without its `serialization` there is nothing to read.
    for (declaration, rest, expected) in [
        (
            returns,
            json!({ "returnByValue": true, "serializationOptions": { "maxDepth": 0 } }),
            not_read("Invalid parameters"),
        ),
        (
            returns,
            json!({ "returnByValue": true, "serializationOptions": { "serialization": "idOnly" } }),
            answer(a_handle.clone()),
        ),
        (
            returns,
            json!({ "returnByValue": false, "serializationOptions": { "serialization": "json" } }),
            answer(an_object),
        ),
        (
            returns_deep,
            json!({ "returnByValue": true, "serializationOptions": { "serialization": "json", "maxDepth": 0 } }),
            answer(a_deep_object.clone()),
        ),
        (
            returns_deep,
            json!({ "returnByValue": true, "serializationOptions": { "serialization": "json", "maxDepth": 1 } }),
            answer(a_deep_object.clone()),
        ),
        (
            returns,
            json!({ "returnByValue": "yes", "serializationOptions": {} }),
            not_read("Invalid parameters"),
        ),
        (
            returns,
            json!({ "returnByValue": true, "serializationOptions": 5 }),
            not_read("Invalid parameters"),
        ),
    ] {
        assert_eq!(
            calling(
                &mut client,
                &session,
                declaration,
                world.clone(),
                rest.clone()
            )
            .await
            .and_then(|reply| shape(&reply)),
            expected,
            "{declaration} {rest}"
        );
    }
    // The same on an evaluate.
    for (expression, rest, expected) in [
        (
            "({a:1})",
            json!({ "returnByValue": true, "serializationOptions": { "maxDepth": 0 } }),
            not_read("Invalid parameters"),
        ),
        (
            "({a:1})",
            json!({ "returnByValue": true, "serializationOptions": { "serialization": "idOnly" } }),
            answer(a_handle),
        ),
        (
            "({a:{b:1}})",
            json!({ "returnByValue": true, "serializationOptions": { "serialization": "json", "maxDepth": 0 } }),
            answer(a_deep_object.clone()),
        ),
        (
            "({a:{b:1}})",
            json!({ "serializationOptions": { "serialization": "json", "maxDepth": 1 } }),
            answer(a_deep_object),
        ),
    ] {
        assert_eq!(
            client
                .ask("Runtime.evaluate", asking(expression, rest.clone()), on)
                .await
                .and_then(|reply| shape(&reply)),
            expected,
            "{expression} {rest}"
        );
    }
}

/// A console call the page makes while its document loads rides the
/// navigation's own batch, told after the new document's contexts are
/// announced: the id it names is one the client has already heard of.
/// Playwright drops the call otherwise (`crPage._onConsoleAPI` returns on
/// an unknown context) — which is how a load-time `console.log` used to
/// reach a real client only when a scheduling race happened to save it.
#[tokio::test]
async fn a_console_call_during_navigation_is_told_after_the_context() {
    let _net = crate::server::test_util::net_env_guard();
    let router = axum::Router::new().route(
        "/logs",
        axum::routing::get(|| async {
            axum::response::Html(
                "<html><head><script>console.log('at-load');</script></head><body>x</body></html>",
            )
        }),
    );
    let (mut client, addr) = connect(router).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let on = Some(session.as_str());
    client.run("Page.enable", json!({}), on).await;
    client.events.clear();
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("http://{addr}/logs") }),
            on,
        )
        .await;
    let seen = std::mem::take(&mut client.events);
    let position = |method: &str| {
        seen.iter()
            .position(|event| event["method"] == method)
            .unwrap_or_else(|| panic!("no {method} in {seen:?}"))
    };
    let created = position("Runtime.executionContextCreated");
    let call_pos = position("Runtime.consoleAPICalled");
    assert!(
        created < call_pos,
        "the context is announced before the call: {seen:?}"
    );
    let call = &seen[call_pos];
    assert_eq!(call["params"]["args"][0]["value"], "at-load");
    // The id the call names is one of the contexts the batch announced.
    let announced: Vec<i64> = seen
        .iter()
        .filter(|event| event["method"] == "Runtime.executionContextCreated")
        .filter_map(|event| event["params"]["context"]["id"].as_i64())
        .collect();
    assert!(
        announced.contains(&call["params"]["executionContextId"].as_i64().unwrap()),
        "the call names an announced context {announced:?}: {call:?}"
    );
}

/// A console call the page makes on its own, between commands, reaches a
/// socket that has `Runtime` on: pushed as the session drains it, with no
/// command in flight to carry it.
#[tokio::test]
async fn a_console_call_between_commands_is_pushed() {
    let (mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    client.run("Runtime.enable", json!({}), on).await;
    client
        .run(
            "Runtime.evaluate",
            json!({ "expression": "setTimeout(function () { console.warn('later'); }, 300); 1" }),
            on,
        )
        .await;
    client.events.clear();
    // Nothing is sent from here: the timer fires on the idle page.
    let seen = client
        .events_until("Runtime.consoleAPICalled", Duration::from_secs(5))
        .await;
    let call = seen
        .iter()
        .find(|event| event["method"] == "Runtime.consoleAPICalled")
        .expect("the page's own console call was pushed");
    assert_eq!(call["params"]["type"], "warning");
    assert_eq!(call["params"]["args"][0]["value"], "later");
    assert_eq!(call["sessionId"], session);
}
