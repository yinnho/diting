//! performance.now/observer/timeline + resource timing
use super::common::setup_runtime;

/// Resource Timing §4.4: a full buffer parks new entries and fires
/// `resourcetimingbufferfull`; a handler that clears (or grows) the buffer
/// gets the parked entries back, no handler drops them. The old shape
/// dropped silently — on asset-heavy pages (fxg create page, ~250 boot
/// loads) every later fetch/XHR vanished from the buffer.
#[tokio::test(flavor = "current_thread")]
async fn resource_timing_buffer_full_event_refills_after_clear() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let res = rt
        .call_function_on_remote(
            r#"async () => {
                const out = {};
                const tick = () => new Promise((r) => setTimeout(r, 0));
                const rec = (n) => performance._recordResource({ name: n, startTime: 0, duration: 1, initiatorType: 'fetch' });
                performance.setResourceTimingBufferSize(2);
                let fired = 0;
                rec('a'); rec('b'); rec('c');
                await tick(); await tick();
                out.noHandler = performance.getEntriesByType('resource').map((e) => e.name).join(',');
                performance.onresourcetimingbufferfull = () => { fired++; performance.clearResourceTimings(); };
                rec('d'); rec('e');
                await tick(); await tick();
                out.fired = fired;
                out.afterClear = performance.getEntriesByType('resource').map((e) => e.name).join(',');
                return JSON.stringify(out);
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let v =
        serde_json::from_str::<serde_json::Value>(res.value.unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v["noHandler"], "a,b",
        "no handler: the parked entry is dropped"
    );
    assert_eq!(v["fired"], 1, "one event per full episode");
    assert_eq!(
        v["afterClear"], "d,e",
        "the handler's clear lets the parked entries in"
    );
}

/// #126: the resource-timing buffer carries REAL entries from both
/// faces — the navigation pipeline's fetch timings (Rust-recorded,
/// epoch-stamped, converted into the resource clock) and the fetch/XHR
/// shims' own samples (recorded in-page). Nothing fabricated: a request
/// without a measurement stays absent, and clearResourceTimings
/// empties BOTH faces.
#[tokio::test(flavor = "current_thread")]
async fn resource_timing_buffer_carries_real_entries() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.push_resource_timings(vec![crate::ops::ResourceTimingRecord {
        name: "http://example.com/sheet.css".to_string(),
        initiator: "link",
        // Deliberately BEFORE timeOrigin: pre-init fetches clamp to 0.
        start_epoch_ms: 1.0,
        duration_ms: 42.5,
        status: 200,
        body_size: 1024,
        render_blocking: true,
    }]);
    // The assertions await real fetches, so they ride the
    // callFunctionOn await-promise channel (evaluate is sync-script
    // semantics and would hand back the function object itself).
    let res = rt
            .call_function_on_remote(
                r#"async () => {
                const out = {};
                // Rust face: entry shape, clock clamp, blocking posture.
                let rs = performance.getEntriesByType('resource');
                out.rustCount = rs.length;
                const e = rs[0];
                out.name = e && e.name;
                out.initiator = e && e.initiatorType;
                out.entryType = e && e.entryType;
                out.startTime = e && e.startTime;
                out.duration = e && e.duration;
                out.encoded = e && e.encodedBodySize;
                out.decoded = e && e.decodedBodySize;
                out.status = e && e.responseStatus;
                out.blocking = e && e.renderBlockingStatus;
                out.instance = e instanceof PerformanceResourceTiming;
                out.illegal = (function(){ try { new PerformanceResourceTiming(); return false; } catch (err) { return /Illegal constructor/.test(err.message); } })();
                out.getEntriesHasIt = performance.getEntries().some((x) => x.entryType === 'resource' && x.name === 'http://example.com/sheet.css');
                out.toJSONKeys = e ? Object.keys(e.toJSON()).sort().join(',') : '';
                // JS face: a real fetch() records its own sample.
                await fetch('data:text/plain,hello');
                rs = performance.getEntriesByType('resource');
                out.afterFetch = rs.length;
                const f = rs.find((x) => x.initiatorType === 'fetch');
                out.fetchName = f && f.name.slice(0, 15);
                out.fetchDur = f && (f.duration >= 0 && f.duration < 1000);
                out.fetchBody = f && f.encodedBodySize;
                out.fetchStatus = f && f.responseStatus;
                // Async XHR delegates to fetch but must label itself.
                await new Promise((resolve) => {
                    const x = new XMLHttpRequest();
                    x.open('GET', 'data:text/plain,wxhr');
                    x.onload = () => resolve();
                    x.onerror = () => resolve();
                    x.send();
                });
                const x = performance.getEntriesByType('resource').find((e2) => e2.initiatorType === 'xmlhttprequest');
                out.xhrEntry = !!x;
                // Cap: the in-page buffer drops beyond the limit.
                performance.setResourceTimingBufferSize(3);
                for (let i = 0; i < 5; i++) await fetch('data:text/plain,cap' + i);
                const all = performance.getEntriesByType('resource');
                out.capped = all.filter((e2) => e2.initiatorType === 'fetch').length;
                // Clear empties BOTH faces (the Rust record included).
                performance.clearResourceTimings();
                out.cleared = performance.getEntriesByType('resource').length;
                return JSON.stringify(out);
            }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    let v =
        serde_json::from_str::<serde_json::Value>(res.value.unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v["rustCount"].as_i64().unwrap(),
        1,
        "Rust-recorded sheet must appear"
    );
    assert_eq!(v["name"], "http://example.com/sheet.css");
    assert_eq!(v["initiator"], "link");
    assert_eq!(v["entryType"], "resource");
    assert_eq!(
        v["startTime"].as_f64().unwrap(),
        0.0,
        "pre-timeOrigin starts clamp to 0"
    );
    assert_eq!(
        v["duration"].as_f64().unwrap(),
        42.5,
        "duration is the real measurement"
    );
    assert_eq!(v["encoded"].as_i64().unwrap(), 1024);
    assert_eq!(v["decoded"].as_i64().unwrap(), 1024);
    assert_eq!(v["status"].as_i64().unwrap(), 200);
    assert_eq!(v["blocking"], "blocking", "stylesheets block first render");
    assert_eq!(
        v["instance"], true,
        "entries must be PerformanceResourceTiming instances"
    );
    assert_eq!(v["illegal"], true, "page code still cannot construct one");
    assert_eq!(
        v["getEntriesHasIt"], true,
        "getEntries() must include resource entries"
    );
    assert!(
        v["toJSONKeys"].as_str().unwrap().contains("initiatorType"),
        "toJSON carries resource fields"
    );
    assert_eq!(
        v["afterFetch"].as_i64().unwrap(),
        2,
        "fetch() adds one real entry"
    );
    assert_eq!(v["fetchName"], "data:text/plain");
    assert_eq!(
        v["fetchDur"], true,
        "duration is a real local measurement (data: resolves synchronously, 0 is honest)"
    );
    assert_eq!(
        v["fetchBody"].as_i64().unwrap(),
        5,
        "encodedBodySize is the body length"
    );
    assert_eq!(v["fetchStatus"].as_i64().unwrap(), 200);
    assert_eq!(
        v["xhrEntry"], true,
        "async XHR labels its delegated entry xmlhttprequest"
    );
    assert_eq!(
        v["capped"].as_i64().unwrap(),
        2,
        "cap holds: hello fetch + XHR predate it, one slot left for the cap run"
    );
    assert_eq!(
        v["cleared"].as_i64().unwrap(),
        0,
        "clearResourceTimings empties both faces"
    );
}

#[test]
fn test_performance_now_is_offset_monotonic_and_bounded() {
    // Upstream cdab919 + d93ff51: now() reports ms since timeOrigin (not
    // the raw epoch), never goes backwards under bursty calls, and does
    // not run ahead of real elapsed time. timeOrigin carries ±50ms of
    // persona jitter, so allow a slightly negative floor.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const n1 = performance.now();
            const offsetSane = n1 > -100 && n1 < 60000;
            let bad = 0, prev = -Infinity;
            for (let i = 0; i < 10000; i++) {
                const t = performance.now();
                if (t < prev) bad++;
                prev = t;
            }
            const lead = performance.now() - (Date.now() - performance.timeOrigin);
            return [offsetSane, bad, lead <= 1];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([true, 0, true]));
}

#[test]
fn test_performance_timeline_mark_measure_navigation_paint() {
    // User-timing marks/measures are recorded and queryable; navigation
    // and paint entries are derived from performance.timing (upstream
    // v0.2.1 landed the same surface). mark()/measure() argument
    // validation matches Chrome's TypeError/SyntaxError.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            performance.mark('a');
            performance.mark('b');
            const m = performance.measure('ab', 'a', 'b');
            const marks = performance.getEntriesByType('mark').map(e => e.name);
            const measures = performance.getEntriesByType('measure').map(e => e.name);
            const nav = performance.getEntriesByType('navigation')[0];
            const paint = performance.getEntriesByType('paint').map(e => e.name);
            const dcl = nav.domContentLoadedEventEnd, loadEnd = nav.loadEventEnd;
            performance.clearMarks('a');
            const afterClear = performance.getEntriesByType('mark').map(e => e.name);
            const byName = performance.getEntriesByName('b', 'mark').length;
            const sum = performance.getEntries().length;
            let errName = 'no-throw';
            try { performance.measure('x', 'nope'); } catch (e) { errName = e.name; }
            return [marks, measures, paint,
                    nav.entryType, nav.startTime === 0, nav.type,
                    dcl > 0, loadEnd >= dcl, nav.duration >= loadEnd,
                    m.duration >= 0, m.startTime > -100,
                    afterClear, byName, sum >= 5, errName,
                    performance.getEntriesByType('resource').length];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            ["a", "b"],
            ["ab"],
            ["first-paint", "first-contentful-paint"],
            "navigation",
            true,
            "navigate",
            true,
            true,
            true,
            true,
            true,
            ["b"],
            1,
            true,
            "SyntaxError",
            0
        ])
    );
}

#[test]
fn test_performance_measure_resolves_timing_attribute_names() {
    // User Timing L2: a start/end string that names a PerformanceTiming
    // attribute resolves against the timing object, relative to
    // navigationStart — PDD's perf SDK measures from 'navigationStart'
    // on every boot and Chrome hands it 0 where we used to throw
    // SyntaxError (poisoning the goods-list React mount, 2026-10-12).
    // Unknown names still throw; a page's own mark with the same name
    // wins over the timing fallback.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            const nav = performance.measure('from-nav', 'navigationStart');
            performance.mark('custom');
            const toCustom = performance.measure('to-custom', 'navigationStart', 'custom');
            let threw = 'no-throw';
            try { performance.measure('x', 'definitely-not-a-timing-field'); } catch (e) { threw = e.name; }
            return [
                nav.startTime === 0,
                Number.isFinite(nav.duration),
                toCustom.startTime === 0,
                Number.isFinite(toCustom.duration),
                threw,
            ];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, true, true, true, "SyntaxError"])
    );
}

#[test]
fn test_performance_observer_supported_entry_types_is_honest() {
    // PerformanceObserver.supportedEntryTypes must mirror real Chrome
    // stable exactly — order included, the array order is a fingerprint
    // (dumped from this host's Chrome 2026-11-02). Chrome accepts
    // longtask/LCP observe() calls without throwing; the old 4-type
    // "honest" list threw TypeError inside page monitors mid-init
    // (tmall publish, 2026-11-02). Advertised types we don't record
    // simply never deliver; getEntries types stay a subset of the list.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            const listed = PerformanceObserver.supportedEntryTypes;
            performance.mark('probe');
            const recorded = performance.getEntries().map(e => e.entryType);
            const unrecorded = recorded.filter(t => !listed.includes(t));
            const accepted = (() => {
              try {
                const o = new PerformanceObserver(() => {});
                o.observe({ entryTypes: ['longtask', 'largest-contentful-paint'] });
                o.disconnect();
                return true;
              } catch (e) { return false; }
            })();
            const throwsUnknown = (() => {
              try {
                const o = new PerformanceObserver(() => {});
                o.observe({ entryTypes: ['bogus-type'] });
                return false;
              } catch (e) { return true; }
            })();
            return [listed, unrecorded, listed.includes('largest-contentful-paint'), accepted, throwsUnknown];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            [
                "element",
                "event",
                "first-input",
                "interaction-contentful-paint",
                "largest-contentful-paint",
                "layout-shift",
                "long-animation-frame",
                "longtask",
                "mark",
                "measure",
                "navigation",
                "paint",
                "resource",
                "soft-navigation",
                "visibility-state",
            ],
            [],
            true,
            true,
            true
        ])
    );
}

#[test]
fn test_performance_observer_delivers_mark_measure() {
    // #217 / obscura #1166: entryTypes mark/measure must reach
    // takeRecords (and the observe callback). Bootstrap used to stub
    // observe() as a no-op while getEntriesByType already recorded.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const seen = [];
            const obs = new PerformanceObserver((list) => {
              for (const e of list.getEntries()) seen.push(e.entryType + ':' + e.name);
            });
            obs.observe({ entryTypes: ['mark', 'measure'] });
            performance.mark('m1');
            performance.measure('mm', 'm1');
            // Sync drain before the microtask callback runs.
            const drained = obs.takeRecords().map(e => e.entryType + ':' + e.name);
            return {
              drained,
              byType: performance.getEntriesByType('mark').map(e => e.name),
              supported: PerformanceObserver.supportedEntryTypes,
            };
        "#,
        )
        .unwrap();
    assert_eq!(
        result["drained"],
        serde_json::json!(["mark:m1", "measure:mm"]),
        "takeRecords must see mark+measure: {result}"
    );
    assert_eq!(result["byType"], serde_json::json!(["m1"]));
    assert_eq!(
        result["supported"],
        serde_json::json!([
            "element",
            "event",
            "first-input",
            "interaction-contentful-paint",
            "largest-contentful-paint",
            "layout-shift",
            "long-animation-frame",
            "longtask",
            "mark",
            "measure",
            "navigation",
            "paint",
            "resource",
            "soft-navigation",
            "visibility-state",
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_performance_observer_callback_fires_on_microtask() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"(async () => {
            const seen = [];
            const obs = new PerformanceObserver((list) => {
              for (const e of list.getEntries()) seen.push(e.name);
            });
            obs.observe({ type: 'mark' });
            performance.mark('via-cb');
            // Leave the queue for the microtask callback (do not takeRecords).
            await Promise.resolve();
            await Promise.resolve();
            return seen;
        })()"#,
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value,
        Some(serde_json::json!(["via-cb"])),
        "callback delivery: {result:?}"
    );
}

#[test]
fn test_performance_observer_buffered_replays_existing_marks() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            performance.mark('early');
            const obs = new PerformanceObserver(() => {});
            obs.observe({ type: 'mark', buffered: true });
            return obs.takeRecords().map(e => e.name);
        "#,
        )
        .unwrap();
    assert!(
        result
            .as_array()
            .map(|a| a.iter().any(|v| v.as_str() == Some("early")))
            .unwrap_or(false),
        "buffered observe must surface prior mark: {result}"
    );
}

/// #192: the four Performance* entry constructors Chrome exposes — github's
/// analytics references PerformanceNavigationTiming as a bare global, which
/// used to be a ReferenceError. Mark is page-constructible (startTime
/// defaults 0, never reads the clock); Nav/Paint/Measure mint only through
/// the internal token (page construction throws Illegal constructor), and
/// every entry handed out by the buffer is an instance of the right class.
#[test]
fn performance_entry_constructors_exposed() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r##"
        performance.mark('m1');
        performance.measure('d1', 'm1');
        const ctorMark = new PerformanceMark('ctor-mark', { startTime: 5, detail: { a: 1 } });
        let navThrew = null, paintThrew = null, measureThrew = null;
        try { new PerformanceNavigationTiming(); } catch (e) { navThrew = e.name; }
        try { new PerformancePaintTiming(); } catch (e) { paintThrew = e.name; }
        try { new PerformanceMeasure(); } catch (e) { measureThrew = e.name; }
        ({
            navType: typeof PerformanceNavigationTiming,
            paintType: typeof PerformancePaintTiming,
            markType: typeof PerformanceMark,
            measureType: typeof PerformanceMeasure,
            navInstance: performance.getEntriesByType('navigation')[0] instanceof PerformanceNavigationTiming,
            paintInstance: performance.getEntriesByType('paint').every(p => p instanceof PerformancePaintTiming),
            markInstance: performance.getEntriesByName('m1', 'mark')[0] instanceof PerformanceMark,
            measureInstance: performance.getEntriesByName('d1', 'measure')[0] instanceof PerformanceMeasure,
            ctorMarkStart: ctorMark.startTime,
            ctorMarkDetail: ctorMark.detail.a,
            entryDetailNull: performance.getEntriesByName('m1', 'mark')[0].detail,
            navThrew, paintThrew, measureThrew
        })
    "##;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "navType": "function",
            "paintType": "function",
            "markType": "function",
            "measureType": "function",
            "navInstance": true,
            "paintInstance": true,
            "markInstance": true,
            "measureInstance": true,
            "ctorMarkStart": 5,
            "ctorMarkDetail": 1,
            "entryDetailNull": null,
            "navThrew": "TypeError",
            "paintThrew": "TypeError",
            "measureThrew": "TypeError"
        })
    );
}
