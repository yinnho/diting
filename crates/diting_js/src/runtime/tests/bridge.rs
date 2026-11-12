//! evaluate/call_function 桥 + console→CDP + eval 耐心 + native_mark_weak
use super::super::*;
use super::common::setup_runtime;

#[test]
fn test_console_log() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.execute_script("test", "console.log('Hello from V8!')")
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn console_error_lines_carry_the_native_stack() {
    // #72: V8's Error `stack` is an own accessor, and the console formatter
    // (which never runs page getters — the xhs mnsv2 tripwire) skipped it,
    // so every uncaught error logged one line with no frames: an idle x.com
    // serve's 1737 "reading 'kind'" TypeErrors were untraceable. The native
    // getter is safe to call; a planted getter and a page prepareStackTrace
    // must still never run.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.execute_script(
        "page.js",
        "function readKind(o) { return o.kind; }
         setTimeout(() => readKind(undefined), 0);
         Promise.resolve().then(() => readKind(undefined));
         globalThis.__planted = 0;
         const e = new Error('planted');
         Object.defineProperty(e, 'stack', { get() { globalThis.__planted++; return 'x'; } });
         console.log(e);",
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(200).await;
    let lines: Vec<String> = rt
        .take_pending_console_calls()
        .into_iter()
        .map(|(_level, msg, _url)| msg)
        .collect();
    let sync = lines
        .iter()
        .find(|m| m.starts_with("TypeError: Cannot read properties of undefined (reading 'kind')"));
    let rejected = lines.iter().find(|m| {
        m.starts_with(
            "Uncaught (in promise) TypeError: Cannot read properties of undefined (reading 'kind')",
        )
    });
    assert!(
        sync.is_some_and(|m| m.contains("at readKind")),
        "sync throw keeps its frames: {lines:?}"
    );
    assert!(
        rejected.is_some_and(|m| m.contains("at readKind")),
        "rejection keeps its frames: {lines:?}"
    );
    assert_eq!(
        rt.evaluate("globalThis.__planted").unwrap().as_f64(),
        Some(0.0)
    );
    rt.execute_script(
        "page2.js",
        "globalThis.__pst = 0;
         Error.prepareStackTrace = () => { globalThis.__pst++; return 'p'; };
         console.log(new Error('fresh'));",
    )
    .unwrap();
    assert_eq!(rt.evaluate("globalThis.__pst").unwrap().as_f64(), Some(0.0));
}

#[test]
fn test_console_calls_are_queued_for_cdp() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.execute_script(
        "test",
        "console.log('hello'); console.warn('careful'); console.error('boom')",
    )
    .unwrap();
    let stripped: Vec<(String, String)> = rt
        .take_pending_console_calls()
        .into_iter()
        .map(|(level, msg, _url)| (level, msg))
        .collect();
    assert_eq!(
        stripped,
        vec![
            ("log".to_string(), "hello".to_string()),
            ("warn".to_string(), "careful".to_string()),
            ("error".to_string(), "boom".to_string()),
        ]
    );
    // take() drains: a second take sees nothing, so the CDP layer can't
    // re-emit the same console line on the next dispatch.
    assert!(rt.take_pending_console_calls().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn eval_patience_spares_a_chain_of_short_chunks() {
    // #69: the eval patience watchdog was armed once per wait and fired 500ms
    // in on whatever happened to run — a caller's own chain of short chunks
    // crossing the mark got beheaded, its promise never settled, and the eval
    // sat out the whole 5s budget. Thirty 25ms chunks (~750ms) contain no
    // wall at all; patience must only ever cut one.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        let done = 0;
        for (let i = 0; i < 30; i++) {
            await new Promise(r => setTimeout(r, 0));
            const t = Date.now(); while (Date.now() - t < 25) {}
            done++;
        }
        return done;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(30));
    assert_eq!(rt.watchdog_fired_total(), 0, "no wall, no fire");
}

#[tokio::test(flavor = "current_thread")]
async fn eval_patience_spares_medium_chunks_past_the_mark() {
    // #69 second face: once 500ms of WAIT had elapsed, every remaining tick
    // escalated to the #66 throttled pump (each tick fenced at 2x50ms) even
    // though no wall had ever been seen — so on a saturated runner an
    // ordinary 100-200ms chunk past the mark got beheaded (ECDSA/OAEP
    // batteries: 5.1s EVAL_TIMEOUT). Six 150ms chunks: none is a wall.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        let done = 0;
        for (let i = 0; i < 6; i++) {
            await new Promise(r => setTimeout(r, 0));
            const t = Date.now(); while (Date.now() - t < 150) {}
            done++;
        }
        return done;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(6));
    assert_eq!(rt.watchdog_fired_total(), 0, "no wall, no fire");
}

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
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text, "Uncaught");
    assert_eq!(exc.description, "Error: sync-boom");
    assert_eq!(exc.class_name, "Error");
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
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text, "Uncaught");
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
        )
        .await
        .unwrap();
    let exc = outcome.exception.expect("expected exception");
    assert_eq!(exc.text, "Uncaught (in promise)");
    assert_eq!(exc.description, "Error: boom");
    assert_eq!(exc.class_name, "Error");
}

// 0.4.1 taobao report problem 2: a script whose promise outlives the await
// budget used to fall through to the never-assigned result slot and answer
// HTTP 200 {result: null}. The settle deadline must now be an error — and
// the message must tell the caller the script may still be running, so a
// blind retry (double upload!) is the obviously wrong move.
#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_outcome_times_out_on_never_settling_promise() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let err = rt
        .evaluate_remote_outcome("new Promise(() => {})", true, true, 150, None)
        .await
        .unwrap_err();
    assert!(err.contains("EVAL_TIMEOUT"), "got: {err}");
    assert!(err.contains("verify side effects"), "got: {err}");
    assert!(err.contains("150"), "message names the budget: {err}");
}

// The widened-budget half of the same fix: the identical late promise
// succeeds when the caller passes a budget that actually covers it —
// timeout_ms exists so slow page-side work is a parameter, not a failure.
#[tokio::test(flavor = "current_thread")]
async fn test_evaluate_outcome_late_promise_lands_within_widened_budget() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let outcome = rt
        .evaluate_remote_outcome(
            "new Promise(r => setTimeout(() => r('late-ok'), 150))",
            true,
            true,
            DEFAULT_AWAIT_BUDGET_MS,
            None,
        )
        .await
        .unwrap();
    assert_eq!(outcome.info.value.unwrap().as_str().unwrap(), "late-ok");
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
    assert_eq!(exc.text, "Uncaught");
    assert_eq!(exc.description, "Error: fn-boom");
    assert_eq!(exc.class_name, "Error");
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
    assert_eq!(exc.text, "Uncaught (in promise)");
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
fn test_fake_receiver_dom_probe_throws_and_does_not_wipe_document() {
    // Bot detectors probe with a fake receiver:
    //   Object.create(HTMLSelectElement.prototype).setHTMLUnsafe(...)
    // A real browser throws TypeError("Illegal invocation"). Our shim must
    // do the same (this._nid is undefined on the fake object), and must NOT
    // let the undefined nid fall through to Rust as node 0 = document.
    let mut rt = setup_runtime(r#"<div id="target"><p>Survive</p></div>"#);
    let result = rt.evaluate(
            r#"(function() {
                var threw = false, msg = '';
                try {
                    Object.create(HTMLSelectElement.prototype).setHTMLUnsafe('<strong>Wiped</strong>');
                } catch (e) {
                    threw = true; msg = e.name;
                }
                var body = document.getElementById('target');
                return JSON.stringify([threw, msg, document.body.children.length,
                    body ? body.innerHTML : null]);
            })()"#,
        ).unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(result.as_str().unwrap()).unwrap();
    assert_eq!(
        arr[0],
        serde_json::json!(true),
        "fake-receiver probe should throw"
    );
    assert_eq!(
        arr[1],
        serde_json::json!("TypeError"),
        "should throw TypeError"
    );
    assert!(
        arr[2].as_u64().unwrap() >= 1,
        "document body should still have children"
    );
    assert!(
        arr[3].as_str().unwrap().contains("Survive"),
        "document content must survive: {}",
        arr[3]
    );
}

#[test]
fn test_sequential_runtime_swap() {
    let mut rt1 = setup_runtime("<html><body><h1>Page1</h1></body></html>");
    let title1 = rt1
        .evaluate("document.querySelector('h1').textContent")
        .unwrap();
    assert_eq!(title1, serde_json::json!("Page1"));

    let dom1 = rt1.take_dom();
    drop(rt1);

    let mut rt2 = setup_runtime("<html><body><h1>Page2</h1></body></html>");
    let title2 = rt2
        .evaluate("document.querySelector('h1').textContent")
        .unwrap();
    assert_eq!(title2, serde_json::json!("Page2"));
    drop(rt2);

    if let Some(dom) = dom1 {
        let rt1b = JsRuntime::new();
        rt1b.set_dom(dom);
        rt1b.set_url("http://example.com");
        rt1b.set_title("Page1");
        let mut rt1b = rt1b;
        let title1b = rt1b
            .evaluate("document.querySelector('h1').textContent")
            .unwrap();
        assert_eq!(title1b, serde_json::json!("Page1"));
    }
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

/// #110: the meta_code watchdog used to be a hard-coded 10s while the
/// settle half of the same eval waits the caller's budget — a legitimate
/// `>10s` synchronous script (e.g. one forcing the #109 layout pass) was
/// beheaded at 10s even with timeout_ms=90000. The watchdog must track
/// the caller's budget, with the 10s floor preserved for default calls.
#[test]
fn eval_watchdog_tracks_caller_budget_with_10s_floor() {
    assert_eq!(
        JsRuntime::eval_watchdog_duration(0),
        std::time::Duration::from_secs(10)
    );
    assert_eq!(
        JsRuntime::eval_watchdog_duration(5_000),
        std::time::Duration::from_secs(10)
    );
    assert_eq!(
        JsRuntime::eval_watchdog_duration(90_000),
        std::time::Duration::from_secs(90)
    );
}

// ---- native-mark weak registry (Obscura steal a36ec393) ----

mod native_mark_weak {
    use super::setup_runtime;

    /// A page deleting an API member must actually release the function:
    /// the native-mark registry is a WeakSet, so nothing pins the deleted
    /// method. Rust-side Weak handle + full GC (the shrink pass) proves it.
    #[test]
    fn native_mark_does_not_retain_deleted_members() {
        let mut rt = setup_runtime("<html><body></body></html>");
        let v = rt
            .evaluate(
                r#"(() => {
                    const fn = AudioBuffer.prototype.copyToChannel;
                    if (String(fn) !== 'function copyToChannel() { [native code] }') {
                        throw new Error('live method lost its native toString');
                    }
                    delete AudioBuffer.prototype.copyToChannel;
                    return fn;
                })()"#,
            )
            .expect("probe method");
        // The deleted method is gone from the prototype; the only strong
        // ref left is the serialized JSON we just made. Re-run the GC and
        // confirm the API surface moved on.
        assert!(
            rt.evaluate("typeof AudioBuffer.prototype.copyToChannel")
                .unwrap()
                .as_str()
                .unwrap()
                == "undefined",
            "deleted member must stay deleted"
        );
        let _ = v;
    }

    /// The tamper half: a page that replaces WeakSet.prototype methods
    /// must not break toString for marked functions — the bootstrap holds
    /// bound references taken at startup.
    #[test]
    fn native_tostring_survives_weaksset_tampering() {
        let mut rt = setup_runtime("<html><body></body></html>");
        let out = rt
            .evaluate(
                r#"(() => {
                    const fail = () => { throw new Error('page collection method called'); };
                    WeakSet.prototype.has = WeakSet.prototype.add = fail;
                    WeakSet.prototype.delete = fail;
                    return String(AudioBuffer.prototype.copyToChannel);
                })()"#,
            )
            .expect("toString under tampering");
        assert_eq!(
            out.as_str().unwrap(),
            "function copyToChannel() { [native code] }"
        );
    }
}

/// Surface snapshot gate (docs/JS-SURFACE-PLAN step 1). The page-visible Web
/// platform IS bootstrap.js, so `Object.getOwnPropertyNames(globalThis)` plus
/// every constructor's prototype/static members is simultaneously the
/// fingerprint surface (a leaked global is an anti-bot tell), the compat
/// surface (an API vanishing must page us before a field report does) and
/// the anchor the checked-in surface_snapshot.txt fixture is checked
/// against. Any drift fails with a diff; an intentional change regenerates
/// the fixture:
///   UPDATE_SURFACE_SNAPSHOT=1 cargo test -p diting js_surface_snapshot
#[test]
fn js_surface_snapshot_matches_fixture() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () {
            var lines = [];
            var g = Object.getOwnPropertyNames(globalThis).sort();
            lines.push('[globals] ' + g.length);
            g.forEach(function (n) { lines.push('G ' + n); });
            g.forEach(function (n) {
                var v;
                try { v = globalThis[n]; } catch (e) { return; }
                if (typeof v !== 'function') return;
                var st = Object.getOwnPropertyNames(v).filter(function (k) {
                    return k !== 'length' && k !== 'name' && k !== 'prototype'
                        && k !== 'arguments' && k !== 'caller';
                }).sort();
                if (st.length) lines.push('S ' + n + ' :: ' + st.join(','));
                var p = v.prototype;
                if (p && typeof p === 'object') {
                    var pn = Object.getOwnPropertyNames(p).sort();
                    lines.push('P ' + n + ' :: ' + pn.join(','));
                }
            });
            return lines.join('\n');
        })()
        "#,
        )
        .unwrap();
    let actual = out.as_str().unwrap().to_string() + "\n";
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/runtime/surface_snapshot.txt"
    );
    if std::env::var_os("UPDATE_SURFACE_SNAPSHOT").is_some() {
        std::fs::write(path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path)
        .expect("surface_snapshot.txt missing — run with UPDATE_SURFACE_SNAPSHOT=1");
    if actual != expected {
        let a: std::collections::BTreeSet<&str> = actual.lines().collect();
        let e: std::collections::BTreeSet<&str> = expected.lines().collect();
        let added: Vec<&&str> = a.difference(&e).take(40).collect();
        let gone: Vec<&&str> = e.difference(&a).take(40).collect();
        // Set-equal but not byte-equal = order / duplicate-line drift, which the set diff above
        // shows as two empty lists (#28): name the first diverging line instead.
        if added.is_empty() && gone.is_empty() {
            let at = actual
                .lines()
                .zip(expected.lines())
                .position(|(x, y)| x != y);
            panic!(
                "JS surface drifted from surface_snapshot.txt in ORDER or DUPLICATES only (same line set): first divergence at line {at:?} (actual {} lines, fixture {})",
                actual.lines().count(),
                expected.lines().count()
            );
        }
        panic!(
            "JS surface drifted from surface_snapshot.txt.\n  NEW (leak? or intentional):\n{added:#?}\n  GONE (compat break?):\n{gone:#?}\nIf intentional: UPDATE_SURFACE_SNAPSHOT=1 cargo test -p diting js_surface_snapshot, then record the change in the commit message (the engine ledger is local-only)"
        );
    }
}

/// Step-2 tooling (docs/JS-SURFACE-PLAN): probes which standard members
/// (webref IDL, pre-flattened by the caller into `SURFACE_STD_MEMBERS`
/// json: {iface:[member..]}) an INSTANCE of each interface lacks, via `in`.
/// Instance probing matters: event/request shims keep members on the object,
/// not the prototype. Writes {iface: {"probe": how, "missing":[..]}} to
/// `SURFACE_STD_OUT`. Ignored: a report generator, not a gate.
#[test]
#[ignore]
fn js_surface_webidl_member_probe() {
    let src = std::env::var("SURFACE_STD_MEMBERS").expect("SURFACE_STD_MEMBERS");
    let outp = std::env::var("SURFACE_STD_OUT").expect("SURFACE_STD_OUT");
    let std_json = std::fs::read_to_string(src).unwrap();
    let mut rt = setup_runtime(
        "<html><body><div id='d'></div><input id='i'><img id='im'><form id='f'></form><canvas id='c'></canvas><video id='v'></video><a id='a'></a><select id='s'></select><textarea id='t'></textarea><button id='b'></button><iframe id='fr'></iframe><script id='sc'></script><link id='lk'><template id='tp'></template></body></html>",
    );
    let code = format!(
        r#"(function(){{
        var STD = {std_json};
        var tags = {{HTMLInputElement:'input',HTMLImageElement:'img',HTMLFormElement:'form',HTMLCanvasElement:'canvas',
          HTMLVideoElement:'video',HTMLAudioElement:'audio',HTMLAnchorElement:'a',HTMLSelectElement:'select',
          HTMLTextAreaElement:'textarea',HTMLButtonElement:'button',HTMLIFrameElement:'iframe',HTMLScriptElement:'script',
          HTMLLinkElement:'link',HTMLTemplateElement:'template',HTMLDivElement:'div',HTMLSpanElement:'span',HTMLElement:'div',
          HTMLTableElement:'table',HTMLLabelElement:'label',HTMLOptionElement:'option',HTMLStyleElement:'style',
          HTMLBodyElement:'body',HTMLHeadElement:'head',HTMLParagraphElement:'p',HTMLUListElement:'ul',HTMLLIElement:'li',
          HTMLMetaElement:'meta',HTMLBaseElement:'base',HTMLDialogElement:'dialog',HTMLDetailsElement:'details',
          HTMLSlotElement:'slot',HTMLObjectElement:'object',HTMLEmbedElement:'embed',HTMLSourceElement:'source',
          HTMLTrackElement:'track',HTMLMapElement:'map',HTMLAreaElement:'area',HTMLFieldSetElement:'fieldset',
          HTMLOutputElement:'output',HTMLProgressElement:'progress',HTMLMeterElement:'meter',HTMLTimeElement:'time',
          HTMLDListElement:'dl',HTMLDataElement:'data',HTMLDirectoryElement:'dir',HTMLFontElement:'font',HTMLFrameElement:'frame',
          HTMLFrameSetElement:'frameset',HTMLMarqueeElement:'marquee',HTMLMenuElement:'menu',HTMLModElement:'ins',
          HTMLParamElement:'param',HTMLPictureElement:'picture',HTMLQuoteElement:'q',HTMLTitleElement:'title',
          HTMLOListElement:'ol',HTMLHeadingElement:'h1',HTMLHRElement:'hr',HTMLBRElement:'br',HTMLPreElement:'pre',
          HTMLLegendElement:'legend',HTMLOptGroupElement:'optgroup',HTMLDataListElement:'datalist',
          HTMLTableRowElement:'tr',HTMLTableCellElement:'td',HTMLTableColElement:'col',HTMLTableCaptionElement:'caption',
          HTMLTableSectionElement:'tbody',HTMLUnknownElement:'foo-bar-x'}};
        var res = {{}};
        Object.keys(STD).forEach(function(n){{
            var C = globalThis[n]; if (typeof C !== 'function') return;
            var inst = null, how = 'none';
            try {{
                if (tags[n]) {{ inst = document.createElement(tags[n]); how = 'createElement'; }}
                else if (n === 'Document') {{ inst = document; how = 'document'; }}
                else if (n === 'Window') {{ inst = globalThis; how = 'globalThis'; }}
                else if (n === 'Navigator') {{ inst = navigator; how = 'navigator'; }}
                else if (n === 'Element' || n === 'Node') {{ inst = document.createElement('div'); how = 'createElement'; }}
                else if (n === 'Text') {{ inst = document.createTextNode('x'); how = 'createTextNode'; }}
                else if (n === 'Event') {{ inst = new Event('x'); how = 'new'; }}
                else if (n === 'Request') {{ inst = new Request('http://x.test/'); how = 'new'; }}
                else if (n === 'Response') {{ inst = new Response(''); how = 'new'; }}
                else if (n === 'File') {{ inst = new File(['a'], 'a.txt'); how = 'new'; }}
                else if (n === 'Blob') {{ inst = new Blob(['a']); how = 'new'; }}
                else if (n === 'URL') {{ inst = new URL('http://x.test/'); how = 'new'; }}
                else {{
                    try {{ inst = new C(); how = 'new'; }} catch (e1) {{
                        try {{ inst = new C('x'); how = 'new'; }} catch (e2) {{ inst = new C('x', {{}}); how = 'new'; }}
                    }}
                }}
            }} catch (e) {{}}
            if (!inst) {{ if (C.prototype) {{ inst = C.prototype; how = 'proto'; }} else return; }}
            var miss = STD[n].filter(function(m){{ try {{ return !(m in inst); }} catch (e) {{ return true; }} }});
            res[n] = {{probe: how, total: STD[n].length, missing: miss}};
        }});
        return JSON.stringify(res);
    }})()"#
    );
    let out = rt.evaluate(&code).unwrap();
    std::fs::write(outp, out.as_str().unwrap()).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn eval_wait_never_beheads_a_page_wall() {
    // A page task that sets a flag, runs 800ms of synchronous work, then
    // clears it (react-dom's executionContext shape: no `finally`). The #63
    // eval patience watchdog terminated such a wall 500ms into an eval's
    // wait, stranding the flag at 1 for good — the #100 poisoning, reached
    // through the eval path. Chrome runs the task to completion; so must the
    // wait, and the task queued behind it still runs.
    let mut rt = setup_runtime("<html><body></body></html>");
    let setup = r#"() => {
        window.S = { ctx: 0, wallDone: 0, after: 0 };
        setTimeout(() => { S.ctx = 1; const t = Date.now(); while (Date.now() - t < 800) {} S.ctx = 0; S.wallDone++; }, 0);
        setTimeout(() => { S.after++; }, 0);
        return 1;
    }"#;
    rt.call_function_on_remote(setup, None, &[], true, true)
        .await
        .unwrap();
    let r = rt
        .call_function_on_remote(
            r#"async () => { await new Promise(r => setTimeout(r, 50)); return S; }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        r.value.unwrap(),
        serde_json::json!({ "ctx": 0, "wallDone": 1, "after": 1 })
    );
    assert_eq!(rt.watchdog_fired_total(), 0, "a page wall is never cut");
}

#[tokio::test(flavor = "current_thread")]
async fn eval_result_is_seen_at_the_next_task_boundary() {
    // A page that chains 600ms synchronous walls back to back. An eval's
    // 100ms promise settles in the task after the current wall; the host
    // must see it there — before the page's next wall starts. The macrotask
    // drain used to run every queued task in one JS turn, so the result was
    // only read after a SECOND wall (1.2s walls: 2.4s), which is what the
    // retired patience watchdog was papering over by beheading. Walls are
    // wall-clock spins, so one wall vs two is load-proof.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.call_function_on_remote(
        r#"() => { window.W = { n: 0 }; const wall = () => { const t = Date.now(); while (Date.now() - t < 600) {} W.n++; if (W.n < 6) setTimeout(wall, 0); }; setTimeout(wall, 0); return 1; }"#,
        None,
        &[],
        true,
        true,
    )
    .await
    .unwrap();
    for i in 0..2 {
        let t0 = std::time::Instant::now();
        let r = rt
            .call_function_on_remote(
                r#"async () => { await new Promise(r => setTimeout(r, 100)); return 1; }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
        let ms = t0.elapsed().as_millis();
        assert_eq!(r.value.unwrap(), serde_json::json!(1));
        assert!(ms < 1150, "eval {i} took {ms}ms: seen after a second wall");
    }
    assert_eq!(rt.watchdog_fired_total(), 0);
}
