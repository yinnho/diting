//! What an evaluate or a `callFunctionOn` answers: the shape of a result as
//! Chrome's RemoteObject has it, what a thrown value says of itself, how the
//! text of a script is taken, and the life of a handle. The waiting side of
//! an eval — budgets, the watchdog, patience — is in `bridge`.
use super::super::remote_object::page_frames;
use super::super::*;
use super::common::setup_runtime;

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_no_args() {
    let mut rt = setup_runtime("<html><head><title>Test</title></head><body></body></html>");
    let result = rt
        .call_function_on("() => document.title", None, &[], true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("Test Page"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_with_args() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let args = vec![
        serde_json::json!({"value": 10}),
        serde_json::json!({"value": 20}),
    ];
    let result = rt
        .call_function_on("(a, b) => a + b", None, &args, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_f64().unwrap() as i64, 30);
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_with_string_args() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let args = vec![
        serde_json::json!({"value": "hello"}),
        serde_json::json!({"value": " world"}),
    ];
    let result = rt
        .call_function_on("(a, b) => a + b", None, &args, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("hello world"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_with_object_args() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let args = vec![serde_json::json!({"value": {"name": "test", "count": 5}})];
    let result = rt
        .call_function_on("(obj) => obj.name + ':' + obj.count", None, &args, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("test:5"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_return_object() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on("() => ({a: 1, b: 2})", None, &[], true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!({"a": 1, "b": 2}));
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_object_ref_preserves_methods() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on(
            "() => ({ items: [1,2,3], getLen: function() { return this.items.length; } })",
            None,
            &[],
            false,
        )
        .await
        .unwrap();
    let oid = result.object_id.unwrap();

    let result2 = rt
        .call_function_on(
            "function() { return this.getLen(); }",
            Some(&oid),
            &[],
            true,
        )
        .await
        .unwrap();
    assert_eq!(result2.value.unwrap().as_f64().unwrap() as i64, 3);
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_remote_detects_node() {
    let mut rt = setup_runtime("<html><body><h1>Hello</h1></body></html>");
    let result = rt
        .evaluate_remote("document.querySelector('h1')", false, false)
        .await
        .unwrap();
    assert_eq!(result.subtype.as_deref(), Some("node"));
    assert_eq!(result.js_type, "object");
    assert!(result.object_id.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_remote_detects_document() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate_remote("document", false, false).await.unwrap();
    assert_eq!(result.subtype.as_deref(), Some("node"));
    assert_eq!(result.class_name, "HTMLDocument");
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_remote_awaits_resolved_promise() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote("Promise.resolve(42)", true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_f64().unwrap() as i64, 42);
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_remote_awaits_timer_promise() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            "new Promise(resolve => setTimeout(() => resolve('done'), 1))",
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_str().unwrap(), "done");
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_remote_awaits_async_function() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote("(async () => 'async-ok')()", true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_str().unwrap(), "async-ok");
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_remote_reports_promise_rejection() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let err = rt
        .evaluate_remote("Promise.reject(new Error('boom'))", true, true)
        .await
        .unwrap_err();
    assert!(err.contains("boom"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_outcome_reports_sync_throw() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let outcome = rt
        .evaluate_remote_outcome(
            "(() => { throw new Error('sync-boom') })()",
            false,
            false,
            DEFAULT_AWAIT_BUDGET_MS,
            None,
            EvalInput::Forgiving,
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text(), "Uncaught");
    assert_eq!(exc.description, "Error: sync-boom");
    assert_eq!(outcome.info.class_name, "Error");
    assert_eq!(outcome.info.subtype.as_deref(), Some("error"));
    assert!(outcome.info.object_id.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_outcome_reports_sync_throw_by_value() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let outcome = rt
        .evaluate_remote_outcome(
            "(() => { throw new Error('bv-boom') })()",
            true,
            false,
            DEFAULT_AWAIT_BUDGET_MS,
            None,
            EvalInput::Forgiving,
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text(), "Uncaught");
    assert_eq!(exc.description, "Error: bv-boom");
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_outcome_reports_await_rejection() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let outcome = rt
        .evaluate_remote_outcome(
            "Promise.reject(new Error('boom'))",
            true,
            true,
            DEFAULT_AWAIT_BUDGET_MS,
            None,
            EvalInput::Forgiving,
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text(), "Uncaught (in promise)");
    assert_eq!(exc.description, "Error: boom");
    assert_eq!(outcome.info.class_name, "Error");
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_outcome_reports_throw() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let outcome = rt
        .call_function_on_remote_outcome(
            "() => { throw new Error('fn-boom') }",
            None,
            &[],
            false,
            false,
            None,
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text(), "Uncaught");
    assert_eq!(exc.description, "Error: fn-boom");
    assert_eq!(outcome.info.class_name, "Error");
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_outcome_reports_await_rejection() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let outcome = rt
        .call_function_on_remote_outcome(
            "() => Promise.reject(new Error('async-fn-boom'))",
            None,
            &[],
            true,
            true,
            None,
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text(), "Uncaught (in promise)");
    assert_eq!(exc.description, "Error: async-fn-boom");
}

#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_frame_nid_restores_on_throw() {
    let mut rt = setup_runtime(r#"<html><body><iframe id="f"></iframe></body></html>"#);
    let nid = rt
        .with_dom(|dom| dom.query_selector_all("#f").unwrap()[0].index() as u32)
        .unwrap();
    let outcome = rt
        .evaluate_remote_outcome(
            "(() => { throw new Error('frame-boom') })()",
            false,
            false,
            DEFAULT_AWAIT_BUDGET_MS,
            Some(nid),
            EvalInput::Forgiving,
        )
        .await
        .unwrap();
    assert_eq!(
        outcome.exception.expect("expected exception").description,
        "Error: frame-boom"
    );
    let restored = rt
        .evaluate(
            "document.getElementById('f') !== null && !('__diting_frame_saved' in globalThis)",
        )
        .unwrap();
    assert_eq!(
        restored,
        serde_json::json!(true),
        "restore ran on the exception path"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_frame_nid_scopes_this_arg() {
    let mut rt = setup_runtime(r#"<html><body><iframe id="f"></iframe></body></html>"#);
    let nid = rt
        .with_dom(|dom| dom.query_selector_all("#f").unwrap()[0].index() as u32)
        .unwrap();
    rt.evaluate("globalThis.__probeIframe = document.getElementById('f')")
        .unwrap();
    let outcome = rt
        .call_function_on_remote_outcome(
            "() => document === __probeIframe.contentDocument",
            None,
            &[],
            true,
            false,
            Some(nid),
        )
        .await
        .unwrap();
    assert_eq!(outcome.info.value, Some(serde_json::json!(true)));
    let restored = rt
        .evaluate("!('__diting_frame_saved' in globalThis)")
        .unwrap();
    assert_eq!(restored, serde_json::json!(true));
}

#[tokio::test(flavor = "current_thread")]
async fn test_call_function_on_dom_interaction() {
    let mut rt = setup_runtime(r#"<div id="items"><span>A</span><span>B</span></div>"#);
    let args = vec![serde_json::json!({"value": "span"})];
    let result = rt
        .call_function_on(
            "(sel) => document.querySelectorAll(sel).length",
            None,
            &args,
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_f64().unwrap() as i64, 2);
}

#[test]
fn test_evaluate_multistatement() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate("var x = 5; var y = 10; return x + y;").unwrap();
    assert_eq!(result.as_f64().unwrap() as i64, 15);
}

#[tokio::test(flavor = "current_thread")]
async fn test_object_ref_as_argument() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let obj = rt
        .call_function_on("() => ({ x: 42 })", None, &[], false)
        .await
        .unwrap();
    let oid = obj.object_id.unwrap();

    let args = vec![serde_json::json!({"objectId": oid})];
    let result = rt
        .call_function_on("(obj) => obj.x * 2", None, &args, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap().as_f64().unwrap() as i64, 84);
}

#[test]
fn resolve_this_parses_numeric_node_ids_only() {
    let rt = setup_runtime("<html><body></body></html>");
    let good = rt.resolve_this(Some("node-7"));
    assert!(good.contains("var nid = 7"), "{good}");
    // Anything after the digits must not ride along as script source.
    for bad in [
        "node-1; globalThis.__injected = true",
        "node-",
        "node-x",
        "node-1.5",
    ] {
        assert_eq!(rt.resolve_this(Some(bad)), "globalThis", "{bad}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn adversarial_object_id_does_not_inject_script() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let payload = "node-1; globalThis.__injected = 42";
    let result = rt
        .call_function_on(
            "function() { return typeof globalThis.__injected; }",
            Some(payload),
            &[],
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("undefined"));
    let probe = rt
        .call_function_on(
            "function() { return globalThis.__injected === undefined; }",
            None,
            &[],
            true,
        )
        .await
        .unwrap();
    assert_eq!(probe.value.unwrap(), serde_json::json!(true));
}

#[test]
fn evaluate_accepts_statement_scripts_with_completion_value() {
    // CDP Runtime.evaluate semantics: the input is a script — statements
    // are legal and the completion value of the last statement comes
    // back. The old expression-only wrap turned any statement syntax
    // into an uncatchable `Unexpected token ';'`.
    let mut rt = setup_runtime("<html><body></body></html>");
    assert_eq!(
        rt.evaluate("var x = 2; x * 3").unwrap(),
        serde_json::json!(6.0)
    );
    assert_eq!(
        rt.evaluate("try { JSON.parse('x') } catch(e) { 'caught:' + e.name }")
            .unwrap(),
        serde_json::json!("caught:SyntaxError")
    );
    // try/catch is itself statement syntax — it must parse.
    assert_eq!(
        rt.evaluate("typeof eval").unwrap(),
        serde_json::json!("function")
    );
}

#[test]
fn evaluate_supports_function_body_style_top_level_return() {
    // Legacy engine contract: scripts written as function bodies with a
    // top-level `return` (illegal in script position) still evaluate via
    // the Function-body fallback.
    let mut rt = setup_runtime("<html><body><span id='h'>x</span></body></html>");
    assert_eq!(
        rt.evaluate(
            r#"
                const el = document.getElementById('h');
                return el ? el.tagName : null;
                "#
        )
        .unwrap(),
        serde_json::json!("SPAN")
    );
}

#[test]
fn evaluate_bare_object_literal_returns_object() {
    // Pasted JSON evaluates as an object literal (DevTools console
    // behavior), not as a block whose completion value is undefined.
    let mut rt = setup_runtime("<html><body></body></html>");
    assert_eq!(
        rt.evaluate(r#"{"k": "v", "n": 1}"#).unwrap(),
        serde_json::json!({"k": "v", "n": 1})
    );
}

#[test]
fn test_page_content_puppeteer_pattern() {
    let mut rt = setup_runtime("<!DOCTYPE html><html><head></head><body><p>Test</p></body></html>");
    let result = rt.evaluate(
            "(function() { let retVal = ''; if (document.doctype) retVal = new XMLSerializer().serializeToString(document.doctype); if (document.documentElement) retVal += document.documentElement.outerHTML; return retVal; })()"
        ).unwrap();
    let html = result.as_str().unwrap();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<html>"));
    assert!(html.contains("<p>Test</p>"));
}

/// The outcome of a script taken as Chrome takes one, shown as a result is
/// (no value asked for).
async fn shaped(rt: &mut JsRuntime, script: &str) -> EvalOutcome {
    rt.evaluate_remote_outcome(
        script,
        false,
        false,
        DEFAULT_AWAIT_BUDGET_MS,
        None,
        EvalInput::Script,
    )
    .await
    .unwrap_or_else(|err| panic!("{script}: {err}"))
}

/// The same, with the value asked for.
async fn answered(rt: &mut JsRuntime, script: &str, input: EvalInput) -> EvalOutcome {
    rt.evaluate_remote_outcome(script, true, false, DEFAULT_AWAIT_BUDGET_MS, None, input)
        .await
        .unwrap_or_else(|err| panic!("{script}: {err}"))
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// The value of a result asked by value. A whole number comes back as an
/// integer and the rest as a float, so they are read as numbers.
fn number(outcome: &EvalOutcome) -> Option<f64> {
    outcome
        .info
        .value
        .as_ref()
        .and_then(serde_json::Value::as_f64)
}

/// The shape of a result, as Chrome's inspector shapes it (Chrome 158,
/// measured): a class is its `constructor`'s name — the one it extends
/// where it has none, and the string `Symbol.toStringTag` where the class is
/// `Object` — and a tag alone makes no `Map`. An Error's message line is
/// headed by its `name` unless that is not a string, empty, or the bare
/// `"Error"`: then by its class.
#[tokio::test(flavor = "current_thread")]
async fn a_result_is_shaped_as_chrome_shapes_it() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // (script, subtype, class, the first line of the description)
    let cases = [
        ("Math", "", "Math", "Math"),
        ("JSON", "", "JSON", "JSON"),
        ("console", "", "console", "console"),
        ("({[Symbol.toStringTag]: 'custom'})", "", "custom", "custom"),
        ("Object.create(Math)", "", "Math", "Math"),
        ("({[Symbol.toStringTag]: 'Map'})", "", "Map", "Map"),
        ("new Map([[1, 2]])", "map", "Map", "Map(1)"),
        ("new (class M9 extends Map {})([[1, 2]])", "map", "M9", "M9(1)"),
        ("new (class extends Map {})([[1, 2]])", "map", "Map", "Map(1)"),
        ("new (class extends Uint8Array {})(2)", "typedarray", "Uint8Array", "Uint8Array(2)"),
        ("new Set([1]).values()", "iterator", "SetIterator", "SetIterator"),
        ("new Map().keys()", "iterator", "MapIterator", "MapIterator"),
        ("[][Symbol.iterator]()", "", "Array Iterator", "Array Iterator"),
        ("'ab'[Symbol.iterator]()", "", "StringIterator", "StringIterator"),
        ("'ab'.matchAll(/b/g)", "", "RegExpStringIterator", "RegExpStringIterator"),
        ("(function*(){})()", "generator", "Generator", "Generator"),
        ("(async function*(){})()", "generator", "AsyncGenerator", "AsyncGenerator"),
        ("document.createAttribute('data-x')", "node", "Attr", "data-x"),
        (
            "Object.defineProperty({}, 'constructor', {get(){ throw new Error('x') }})",
            "",
            "Object",
            "Object",
        ),
        ("new Error('x')", "error", "Error", "Error: x"),
        ("new TypeError('x')", "error", "TypeError", "TypeError: x"),
        ("new (class E9 extends Error {})('x')", "error", "E9", "E9: x"),
        ("new (class extends Error {})('x')", "error", "Error", "Error: x"),
        ("new DOMException('nope')", "error", "DOMException", "DOMException: nope"),
        (
            "new DOMException('nope', 'AbortError')",
            "error",
            "DOMException",
            "AbortError: nope",
        ),
        ("var e = new Error('m'); e.name = 'Custom'; e", "error", "Error", "Custom: m"),
        ("var e = new Error('m'); delete e.name; e", "error", "Error", "Error: m"),
        ("var e = new Error('m'); e.name = ''; e", "error", "Error", "Error: m"),
        ("var e = new Error('m'); e.name = 5; e", "error", "Error", "Error: m"),
        ("var e = new TypeError('m'); e.name = 'Error'; e", "error", "TypeError", "TypeError: m"),
        ("new Error()", "error", "Error", "Error"),
        ("var e = new Error(); e.message = 5; e", "error", "Error", "Error"),
        (
            "var e = new Error(); Object.defineProperty(e, 'message', {get(){ throw new Error('boom') }}); e",
            "error",
            "Error",
            "Error",
        ),
        (
            "var e = new Error('m'); Object.defineProperty(e, 'name', {get(){ throw new Error('boom') }}); e",
            "error",
            "Error",
            "Error: m",
        ),
    ];
    for (script, subtype, class, line) in cases {
        let outcome = shaped(&mut rt, script).await;
        assert!(
            outcome.exception.is_none(),
            "{script}: thrown {:?}",
            outcome.exception.map(|exception| exception.description)
        );
        assert_eq!(outcome.info.js_type, "object", "{script}: type");
        assert_eq!(
            outcome.info.subtype.as_deref().unwrap_or(""),
            subtype,
            "{script}: subtype"
        );
        assert_eq!(outcome.info.class_name, class, "{script}: class");
        assert_eq!(
            first_line(&outcome.info.description),
            line,
            "{script}: description"
        );
    }
}

/// A thrown Error says two things, as in Chrome: the value shown is headed
/// by its class, and the message line — what a rejected promise's text
/// repeats — by its `name`, where that is a non-empty string and `Error`
/// where it is not. A `DOMException` is headed by its class where it still
/// says `Error`.
#[tokio::test(flavor = "current_thread")]
async fn a_thrown_error_says_what_chrome_says() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // (script, what the exception says, the class of the value, its first line)
    let cases = [
        (
            "class T9 extends Error {}; throw new T9('x')",
            "Error: x",
            "T9",
            "T9: x",
        ),
        (
            "throw new (class extends Error {})('x')",
            "Error: x",
            "Error",
            "Error: x",
        ),
        (
            "var e = new RangeError('x'); e.name = 'Error'; throw e",
            "Error: x",
            "RangeError",
            "RangeError: x",
        ),
        (
            "var e = new Error(); e.message = 5; throw e",
            "Error",
            "Error",
            "Error",
        ),
        (
            "var e = new Error('x'); e.name = 'Custom'; throw e",
            "Custom: x",
            "Error",
            "Custom: x",
        ),
        (
            "throw new DOMException('nope')",
            "DOMException: nope",
            "DOMException",
            "DOMException: nope",
        ),
        (
            "throw {name: 'a', message: 'b'}",
            "a: b",
            "Object",
            "Object",
        ),
        ("throw 'plain'", "plain", "", ""),
    ];
    for (script, said, class, line) in cases {
        let outcome = shaped(&mut rt, script).await;
        let exception = outcome
            .exception
            .as_ref()
            .unwrap_or_else(|| panic!("{script}: nothing was thrown"));
        assert_eq!(
            (exception.text(), exception.in_promise),
            ("Uncaught", false),
            "{script}"
        );
        assert_eq!(exception.description, said, "{script}: what it says");
        assert_eq!(outcome.info.class_name, class, "{script}: class");
        assert_eq!(
            first_line(&outcome.info.description),
            line,
            "{script}: shown"
        );
    }

    // (script, what the exception says, whether an Error, class, first line)
    let rejected = [
        (
            "class R9 extends Error {}; Promise.reject(new R9('x'))",
            "Error: x",
            true,
            "R9",
            "R9: x",
        ),
        (
            "var e = new RangeError('x'); e.name = 'Error'; Promise.reject(e)",
            "Error: x",
            true,
            "RangeError",
            "RangeError: x",
        ),
        (
            "Promise.reject({name: 'a', message: 'b'})",
            "a: b",
            false,
            "Object",
            "Object",
        ),
    ];
    for (script, said, is_error, class, line) in rejected {
        let outcome = rt
            .evaluate_remote_outcome(
                script,
                false,
                true,
                DEFAULT_AWAIT_BUDGET_MS,
                None,
                EvalInput::Script,
            )
            .await
            .unwrap_or_else(|err| panic!("{script}: {err}"));
        let exception = outcome
            .exception
            .as_ref()
            .unwrap_or_else(|| panic!("{script}: nothing was rejected"));
        assert_eq!(exception.text(), "Uncaught (in promise)", "{script}");
        assert_eq!(exception.is_error, is_error, "{script}: is an Error");
        assert_eq!(exception.description, said, "{script}: what it says");
        assert_eq!(outcome.info.class_name, class, "{script}: class");
        assert_eq!(
            first_line(&outcome.info.description),
            line,
            "{script}: shown"
        );
    }
}

/// The CDP face takes the text as Chrome does — a script, nothing forgiven
/// of it — where the HTTP and flow faces keep their two conveniences.
#[tokio::test(flavor = "current_thread")]
async fn a_script_is_taken_as_a_script_and_the_forgiving_text_as_text() {
    let mut rt = setup_runtime("<html><body></body></html>");
    for input in [EvalInput::Script, EvalInput::Forgiving] {
        // Statements, and the completion value of the last one.
        assert_eq!(
            number(&answered(&mut rt, "var x = 2; x * 3", input).await),
            Some(6.0),
            "{input:?}"
        );
    }
    // A bare `{…}`: a labeled statement on the CDP face, an object literal on the other.
    assert_eq!(
        number(&answered(&mut rt, "{a: 1}", EvalInput::Script).await),
        Some(1.0)
    );
    assert_eq!(
        answered(&mut rt, "{a: 1}", EvalInput::Forgiving)
            .await
            .info
            .value,
        Some(serde_json::json!({"a": 1}))
    );
    // With text around it, as pasted: a string key makes it no script —
    // the forgiving face still reads an object. Two keys make no labeled
    // statement either.
    for pasted in ["  {\"k\": \"v\"}  ", "{a: 1, b: 2}"] {
        let outcome = answered(&mut rt, pasted, EvalInput::Script).await;
        assert_eq!(outcome.info.class_name, "SyntaxError", "{pasted}");
        assert_eq!(
            first_line(&outcome.info.description),
            "SyntaxError: Unexpected token ':'",
            "{pasted}"
        );
        assert_eq!(
            outcome.exception.map(|exception| exception.description),
            Some("SyntaxError: Unexpected token ':'".to_string()),
            "{pasted}"
        );
    }
    assert_eq!(
        answered(&mut rt, "  {\"k\": \"v\"}  ", EvalInput::Forgiving)
            .await
            .info
            .value,
        Some(serde_json::json!({"k": "v"}))
    );
    assert_eq!(
        answered(&mut rt, "{a: 1, b: 2}", EvalInput::Forgiving)
            .await
            .info
            .value,
        Some(serde_json::json!({"a": 1, "b": 2}))
    );
    // A top-level `return`: a syntax error as a script, the body's value forgiven.
    let outcome = answered(&mut rt, "return 7", EvalInput::Script).await;
    assert_eq!(outcome.info.subtype.as_deref(), Some("error"));
    assert_eq!(outcome.info.class_name, "SyntaxError");
    assert_eq!(
        number(&answered(&mut rt, "return 7", EvalInput::Forgiving).await),
        Some(7.0)
    );
}

/// A result nothing can be called into — a number, `undefined`, `null` —
/// leaves nothing behind in the object store; only what has a handle does.
#[tokio::test(flavor = "current_thread")]
async fn a_result_without_a_handle_leaves_no_slot() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let slots = |rt: &mut JsRuntime| {
        rt.evaluate("Object.keys(globalThis.__diting_objects).length")
            .unwrap()
            .as_f64()
            .unwrap() as u64
    };
    let before = slots(&mut rt);
    for script in [
        "40 + 2",
        "'text'",
        "true",
        "undefined",
        "null",
        "NaN",
        "-0",
        "10n",
    ] {
        let info = &shaped(&mut rt, script).await.info;
        let primitive = info.js_type != "object" || info.subtype.as_deref() == Some("null");
        assert!(primitive, "{script}: not a primitive");
        assert!(info.object_id.is_none(), "{script}: no handle");
    }
    assert_eq!(slots(&mut rt), before);
    let symbol = shaped(&mut rt, "Symbol('s')").await;
    assert!(
        symbol.info.object_id.is_some(),
        "a symbol is named by handle"
    );
    assert_eq!(slots(&mut rt), before + 1);
    let object = shaped(&mut rt, "({})").await;
    assert!(object.info.object_id.is_some());
    assert_eq!(slots(&mut rt), before + 2);
}

#[tokio::test(flavor = "current_thread")]
async fn release_object_with_adversarial_id_is_inert() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.release_object("x']; globalThis.__injected = 42; ('");
    let probe = rt
        .call_function_on(
            "function() { return globalThis.__injected === undefined; }",
            None,
            &[],
            true,
        )
        .await
        .unwrap();
    assert_eq!(probe.value.unwrap(), serde_json::json!(true));
}

/// An id's realm and its number.
fn parts(id: &str) -> (u64, u64) {
    let fields = serde_json::from_str::<serde_json::Value>(id).expect("an id is JSON");
    (
        fields["injectedScriptId"].as_u64().unwrap(),
        fields["id"].as_u64().unwrap(),
    )
}

// What an id names, for a caller that takes ids from a wire protocol. The
// number on its own is no answer: a runtime counts its objects from 1, and
// a page builds a new runtime for each of its documents, so the document
// after mints the same numbers again. The serial of the realm is in the id;
// an id of a realm that is gone is `Gone`, and never the object that wears
// its number now.
#[tokio::test(flavor = "current_thread")]
async fn an_id_says_its_realm_so_a_stale_one_names_nothing() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let mut next = setup_runtime("<html><body></body></html>");
    let id = shaped(&mut rt, "({})")
        .await
        .info
        .object_id
        .expect("a handle for an object");
    let again = shaped(&mut next, "({})")
        .await
        .info
        .object_id
        .expect("a handle for an object");
    assert_eq!(
        parts(&again).1,
        parts(&id).1,
        "the next document counts from 1 too"
    );
    assert_ne!(parts(&again).0, parts(&id).0, "and is another realm");
    assert_eq!(rt.object_id_state(&id), ObjectIdState::Held);
    assert_eq!(rt.object_id_state(&again), ObjectIdState::Gone);
    assert_eq!(next.object_id_state(&id), ObjectIdState::Gone);
    assert!(!rt.has_object(&again));
}

// A handle is held until it is released, and what is not held is one of
// three things, so that a protocol answers as Chrome does: released or
// never minted here, of a realm that is gone, or no id of this engine's at
// all. The CDP bridge asks before `callFunctionOn` and the `DOM.*`
// commands, which would otherwise run against the global object.
#[tokio::test(flavor = "current_thread")]
async fn an_id_is_held_released_gone_or_no_id_at_all() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let id = shaped(&mut rt, "({})")
        .await
        .info
        .object_id
        .expect("a handle for an object");
    let (serial, number) = parts(&id);
    assert_eq!(rt.object_id_state(&id), ObjectIdState::Held);
    assert!(rt.has_object(&id));
    // A blank in front of an id is not part of it; one behind it is no id,
    // as Chrome's number reader takes one (Chrome 158, measured).
    assert_eq!(rt.object_id_state(&format!(" {id}")), ObjectIdState::Held);
    assert!(rt.has_object(&format!(" {id}")));
    assert_eq!(
        rt.object_id_state(&format!("{id} ")),
        ObjectIdState::Unparseable
    );
    assert!(!rt.has_object(&format!("{id} ")));
    // No id of this engine's: the fields the other way round, one field
    // more, one field short, a part that is no integer.
    for not_an_id in [
        "bogus".to_string(),
        String::new(),
        "node-7".to_string(),
        "x']; globalThis.__injected = 42; ('".to_string(),
        format!("{{\"id\":{number},\"injectedScriptId\":{serial}}}"),
        format!("{{\"injectedScriptId\":{serial},\"id\":{number},\"x\":1}}"),
        format!("{{\"injectedScriptId\":{serial}}}"),
        format!("{{\"injectedScriptId\":\"x\",\"id\":{number}}}"),
        format!("{{\"injectedScriptId\":1.5,\"id\":{number}}}"),
        format!("{{\"injectedScriptId\":{serial},\"id\":\"x\"}}"),
        format!("{{\"injectedScriptId\":{serial},\"id\":null}}"),
        format!("{{\"injectedScriptId\":{serial},\"id\":1.0}}"),
    ] {
        assert_eq!(
            rt.object_id_state(&not_an_id),
            ObjectIdState::Unparseable,
            "{not_an_id}"
        );
    }
    // This realm's spelling, and nothing behind it: a number it never
    // minted — or none at all, 0 or below — and one it let go of.
    for number in [number as i64 + 1_000_000, 0, -1] {
        let never = format!("{{\"injectedScriptId\":{serial},\"id\":{number}}}");
        assert_eq!(rt.object_id_state(&never), ObjectIdState::Released);
    }
    // A part left empty reads as 0, as Chrome's number reader takes one: the
    // number is the 0 no realm minted, the realm the one there is none of.
    assert_eq!(
        rt.object_id_state(&format!("{{\"injectedScriptId\":{serial},\"id\":\"\"}}")),
        ObjectIdState::Released
    );
    assert_eq!(
        rt.object_id_state(&format!("{{\"injectedScriptId\":\"\",\"id\":{number}}}")),
        ObjectIdState::Gone
    );
    rt.release_object(&format!(" {id}"));
    assert_eq!(rt.object_id_state(&id), ObjectIdState::Released);
    assert_eq!(
        rt.object_id_state(&format!(" {id}")),
        ObjectIdState::Released
    );
    assert!(!rt.has_object(&id));
    // An id shaped to splice is an id that is not held, nothing else.
    let probe = rt
        .call_function_on(
            "function() { return globalThis.__injected === undefined; }",
            None,
            &[],
            true,
        )
        .await
        .unwrap();
    assert_eq!(probe.value.unwrap(), serde_json::json!(true));
}

/// A stack as Chrome shows one: the engine's own frames are out, the page's
/// stay as V8 wrote them, and a script the engine compiled — the one an eval
/// ran — is shown as a script of its own. The samples are stacks V8 wrote
/// in this engine.
#[test]
fn a_stack_keeps_the_pages_frames_and_leaves_the_engines_out() {
    let shown = |stack: &str| -> Vec<String> {
        page_frames(stack)
            .into_iter()
            .map(|frame| frame.text)
            .collect()
    };
    // A function of a script an eval ran, over the trampolines that ran it.
    let frames = page_frames(
        "Error: boom\n\
         at pageThrows (eval at <anonymous> (<eval-remote>:130:37), <anonymous>:1:31)\n\
         at eval (eval at <anonymous> (<eval-remote>:130:37), <anonymous>:1:57)\n\
         at eval (<anonymous>)\n\
         at <eval-remote>:130:37\n\
         at <eval-remote>:131:19",
    );
    assert_eq!(
        shown(
            "Error: boom\n\
               at pageThrows (eval at <anonymous> (<eval-remote>:130:37), <anonymous>:1:31)\n\
               at eval (eval at <anonymous> (<eval-remote>:130:37), <anonymous>:1:57)\n\
               at eval (<anonymous>)\n\
               at <eval-remote>:130:37\n\
               at <eval-remote>:131:19"
        ),
        ["pageThrows (<anonymous>:1:31)", "<anonymous>:1:57"]
    );
    assert_eq!(
        (frames[0].script.as_str(), frames[0].line, frames[0].column),
        ("<anonymous>", 1, 31)
    );
    // The page's own scripts, a builtin without a position, and the event loop's frames.
    assert_eq!(
        shown(
            "Error: boom\n\
               at pageThrows (<script>:1:31)\n\
               at <script>:1:120\n\
               at new Promise (<anonymous>)\n\
               at <anonymous>:504:13\n\
               at _mtDrain (<anonymous>:288:11)"
        ),
        [
            "pageThrows (<script>:1:31)",
            "<script>:1:120",
            "new Promise (<anonymous>)"
        ]
    );
    // The page's own `eval`, inside what the engine compiled, stays the page's.
    assert_eq!(
        shown("Error: q\n\
               at q2 (eval at <anonymous> (eval at <anonymous> (<eval-remote>:130:37)), <anonymous>:1:24)\n\
               at eval (eval at <anonymous> (<eval-remote>:130:37), <anonymous>:1:66)\n\
               at eval (<anonymous>)"),
        ["q2 (<anonymous>:1:24)", "<anonymous>:1:66"]
    );
}

/// An Error thrown inside a script an eval ran: its description is its
/// message line and then the frames the page knows, the engine's left out.
#[tokio::test(flavor = "current_thread")]
async fn an_errors_description_carries_the_pages_frames() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("function pageThrows() { throw new Error('boom') }")
        .unwrap();
    let outcome = shaped(&mut rt, "pageThrows()").await;
    let exception = outcome.exception.expect("thrown");
    assert_eq!(exception.description, "Error: boom");
    let lines: Vec<&str> = outcome.info.description.lines().collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0], "Error: boom");
    assert!(lines[1].starts_with("    at pageThrows ("), "{lines:?}");
    assert!(lines[2].starts_with("    at <anonymous>:1:"), "{lines:?}");
    assert_eq!(exception.frames.len(), 2, "{lines:?}");
}

// A `callFunctionOn` declaration that is no function is not called, and the
// call is an error in Chrome's words — not the `TypeError` of the engine's
// own that calling it made. What the declaration does while it is read is
// the page's, as before: a throw there is an exception, and so is a throw
// of the function itself. (Chrome 158, measured.)
#[tokio::test(flavor = "current_thread")]
async fn a_declaration_that_is_no_function_is_not_called() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // Nothing of the calls below is left in the page: no flag, and no slot.
    let left = |rt: &mut JsRuntime| {
        rt.evaluate(
            "Object.keys(globalThis).filter(function(k){ return /^__diting_not_function/.test(k) })\
             .concat(Object.keys(globalThis.__diting_objects || {})).length",
        )
        .unwrap()
    };
    let before = left(&mut rt);
    for await_promise in [false, true] {
        for declaration in [
            "1",
            "undefined",
            "({})",
            "[]",
            "document",
            "Symbol.iterator",
        ] {
            let err = rt
                .call_function_on_remote_outcome(declaration, None, &[], false, await_promise, None)
                .await
                .expect_err(declaration);
            assert_eq!(err, NOT_A_FUNCTION, "{declaration}");
        }
    }
    assert_eq!(left(&mut rt), before);
    for (declaration, description) in [
        ("(boom())", "ReferenceError: boom is not defined"),
        ("function(){ throw new RangeError('d') }", "RangeError: d"),
    ] {
        let outcome = rt
            .call_function_on_remote_outcome(declaration, None, &[], false, false, None)
            .await
            .unwrap();
        let exception = outcome
            .exception
            .unwrap_or_else(|| panic!("{declaration}: not thrown"));
        assert_eq!(exception.description, description, "{declaration}");
    }
}

// One key of a call argument says what the argument is, as V8 reads one
// (`InjectedScript::resolveCallArgument`): its `objectId` if it has one, else
// its `value`; an `unserializableValue` beside either is not even run. An id
// is taken with a blank in front of it, as a receiver and as an argument
// both; one behind it is not in the store. (Chrome 158, measured.)
#[tokio::test(flavor = "current_thread")]
async fn an_argument_is_its_object_id_else_its_value() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let object = shaped(&mut rt, "({ tag: 'object' })")
        .await
        .info
        .object_id
        .expect("a handle for an object");
    for (arguments, expected) in [
        (
            vec![serde_json::json!({ "value": 5, "unserializableValue": "bogus" })],
            "5",
        ),
        (
            vec![serde_json::json!({ "value": 5, "unserializableValue": "NaN" })],
            "5",
        ),
        (
            vec![serde_json::json!({ "value": null, "unserializableValue": "NaN" })],
            "null",
        ),
        (
            vec![serde_json::json!({ "objectId": object, "value": 5 })],
            "[object Object]",
        ),
        (
            vec![serde_json::json!({ "objectId": object, "unserializableValue": "bogus" })],
            "[object Object]",
        ),
        (
            vec![serde_json::json!({ "objectId": format!(" {object}"), "value": 5 })],
            "[object Object]",
        ),
        (
            vec![serde_json::json!({ "objectId": format!("{object} "), "value": 5 })],
            "undefined",
        ),
        (
            vec![serde_json::json!({ "unserializableValue": "NaN" })],
            "NaN",
        ),
        (vec![serde_json::json!({})], "undefined"),
    ] {
        let info = string_of_argument(&mut rt, &arguments)
            .await
            .unwrap_or_else(|err| panic!("{arguments:?}: {err}"));
        assert_eq!(info.value.unwrap(), expected, "{arguments:?}");
    }
    let err = string_of_argument(
        &mut rt,
        &[serde_json::json!({ "unserializableValue": "bogus" })],
    )
    .await
    .expect_err("a text that will not run");
    assert_eq!(err, BAD_CALL_ARGUMENT);
    let padded = format!(" {object}");
    let receiver = rt
        .call_function_on("function(){ return this.tag }", Some(&padded), &[], true)
        .await
        .unwrap();
    assert_eq!(receiver.value.unwrap(), "object");
    assert_eq!(rt.object_id_state(&padded), ObjectIdState::Held);
}

/// `String(a)` for one argument of a call, as the call brings it back by
/// value.
async fn string_of_argument(
    rt: &mut JsRuntime,
    arguments: &[serde_json::Value],
) -> Result<RemoteObjectInfo, String> {
    rt.call_function_on("function(a){ return String(a) }", None, arguments, true)
        .await
}
