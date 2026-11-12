//! Worker/SharedWorker/MessagePort/broadcast/SW 注册/websocket
use super::super::*;
use super::common::setup_runtime;

#[test]
fn test_issue_81_service_worker_event_target() {
    // X's bootstrap does `navigator.serviceWorker?.addEventListener(...)`.
    // `?.` only guards null/undefined — a truthy container without the
    // EventTarget methods throws TypeError and the page renders its
    // failure branch ("Something went wrong") while the network face is
    // fine. The container must be a functioning EventTarget (issue #81).
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            var sw = navigator.serviceWorker;
            var booted = true;
            try { sw?.addEventListener('message', function () {}); } catch (e) { booted = false; }
            return [
                typeof sw,
                typeof sw.addEventListener,
                typeof sw.removeEventListener,
                typeof sw.dispatchEvent,
                typeof sw.register,
                typeof sw.getRegistration,
                typeof sw.getRegistrations,
                booted,
                sw.dispatchEvent(new Event('x')),
                sw.addEventListener.toString().indexOf('[native code]') !== -1,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "object", "function", "function", "function", "function", "function", "function",
            // dispatchEvent of an unhandled event returns true in Chrome (the container is a real EventTarget since M4 knife 49)
            true, true, true,
        ]),
        "serviceWorker container must survive X's optional-chained addEventListener"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_issue_83_sw_register_resolves_registration() {
    // Pages chain off register(): `register('/sw.js').then(reg => reg.scope)`.
    // The old stub resolved undefined, so the .scope read threw — xhs logs
    // it as "Service Worker registration failed: Cannot read properties of
    // undefined (reading 'scope')" (issue #83). register/ready must resolve
    // a registration-shaped object; getRegistration keeps resolving
    // undefined — nothing ever registered, and a lie there pushes pages
    // into the has-registration branch.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://www.xiaohongshu.com/explore");
    rt.evaluate(
        // (P7) register follows the script fetch now — stub it ok+JS so
        // this test keeps pinning the RESOLVE face (scope math + shape).
        "globalThis.__fetch = globalThis.fetch;
             globalThis.fetch = function () {
               return Promise.resolve({ ok: true, status: 200,
                 headers: { get: function () { return 'application/javascript'; } } });
             };
             globalThis.__out = {};
             navigator.serviceWorker.register('/sw.js').then(function (reg) {
               __out.scope = reg.scope;
               __out.viaCache = reg.updateViaCache;
               __out.activeNull = reg.active === null;
               __out.methods = typeof reg.update === 'function'
                 && typeof reg.unregister === 'function'
                 && typeof reg.addEventListener === 'function';
               return reg.unregister();
             }).then(function (v) { __out.unreg = v; });
             navigator.serviceWorker.register('sw.js', { scope: '/im/' })
               .then(function (reg) { __out.optScope = reg.scope; });
             navigator.serviceWorker.ready
               .then(function (reg) { __out.readyScope = reg.scope; });
             navigator.serviceWorker.getRegistration()
               .then(function (r) { __out.getReg = (r === undefined); });",
    )
    .unwrap();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run_event_loop()).await;
    let out = rt.evaluate("globalThis.__out").unwrap();
    assert_eq!(
        out,
        serde_json::json!({
            "scope": "https://www.xiaohongshu.com/",
            "viaCache": "imports",
            "activeNull": true,
            "methods": true,
            "unreg": true,
            // register('sw.js', {scope:'/im/'}) on /explore: script
            // resolves against the page dir, scope against the option.
            "optScope": "https://www.xiaohongshu.com/im/",
            "readyScope": "https://www.xiaohongshu.com/",
            "getReg": true,
        }),
        "register/ready resolve registration-shaped objects; getRegistration stays undefined"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn sw_register_reject_faces_p7() {
    // (300015 P7) ark registers a service worker whose script 404s on the
    // CDN; real Chrome REJECTS the register() promise and the app swallows
    // it. Our unconditional resolve was a one-line bot check. Chrome truth
    // (CDP 155, 10-15): !ok -> TypeError "A bad HTTP response code…",
    // non-JS MIME -> SecurityError "unsupported MIME type", cross-origin
    // script -> SecurityError origin mismatch before any fetch. ok+JS
    // keeps resolving the stub registration.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://ark.xiaohongshu.com/ark");
    rt.evaluate(
            "globalThis.__out = {};
             function stubFetch(status, ct) {
               globalThis.fetch = function () {
                 return Promise.resolve({ ok: status >= 200 && status < 300, status: status,
                   headers: { get: function () { return ct; } } });
               };
             }
             // 404 + application/xml (the live ark SW URL shape)
             stubFetch(404, 'application/xml');
             navigator.serviceWorker.register('/service-worker?hash=1792046827315').then(
               function () { __out.a = 'RESOLVED'; },
               function (e) { __out.a = e.name + '|' + e.message; });
             // 200 + text/html (customer-origin SPA fallback shape)
             stubFetch(200, 'text/html');
             navigator.serviceWorker.register('/sw.js').then(
               function () { __out.b = 'RESOLVED'; },
               function (e) { __out.b = e.name + '|' + e.message; });
             // cross-origin script: rejects before fetch (stub stays armed,
             // proving no fetch happened)
             var fetched = false;
             globalThis.fetch = function () { fetched = true; return Promise.resolve({ ok: true }); };
             navigator.serviceWorker.register('https://other.example/sw.js').then(
               function () { __out.c = 'RESOLVED'; },
               function (e) { __out.c = e.name + '|' + e.message; });
             __out.noFetch = String(!fetched);
             // ok + JS MIME -> resolves with scope
             stubFetch(200, 'application/javascript');
             navigator.serviceWorker.register('/real-sw.js').then(
               function (reg) { __out.d = 'RESOLVED|' + reg.scope; },
               function (e) { __out.d = e.name + '|' + e.message; });",
        )
        .unwrap();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run_event_loop()).await;
    let out = rt.evaluate("globalThis.__out").unwrap();
    let m = out.as_object().unwrap();
    let get = |k: &str| m.get(k).unwrap().as_str().unwrap().to_string();
    assert_eq!(
        get("a"),
        "TypeError|Failed to register a ServiceWorker for scope ('https://ark.xiaohongshu.com/') \
             with script ('https://ark.xiaohongshu.com/service-worker?hash=1792046827315'): \
             A bad HTTP response code (404) was received when fetching the script."
    );
    assert_eq!(
        get("b"),
        "SecurityError|Failed to register a ServiceWorker for scope ('https://ark.xiaohongshu.com/') \
             with script ('https://ark.xiaohongshu.com/sw.js'): \
             The script has an unsupported MIME type ('text/html')."
    );
    assert_eq!(
        get("c"),
        "SecurityError|Failed to register a ServiceWorker: The origin of the provided scriptURL \
             ('https://other.example') does not match the current origin ('https://ark.xiaohongshu.com')."
    );
    assert_eq!(
        get("noFetch"),
        "true",
        "cross-origin rejects before fetching"
    );
    assert_eq!(
        get("d"),
        "RESOLVED|https://ark.xiaohongshu.com/",
        "ok+JS MIME keeps resolving the stub registration"
    );
}

// importScripts() was a silent no-op stub: Yahoo Finance's worker imports
// protobuf at startup, got `undefined` instead, and the SDK died to a
// TypeError pages could not see (obscura#827 family). The worker source
// executes synchronously inside new Function, so imports cannot be
// fetched at call time — they are preloaded from string-literal targets
// in the source, then replayed from cache via indirect eval, which puts
// top-level `var` in the global lexical scope the Function-wrapped body
// can see (a plain `var` assignment would also work, but declarations
// would not — pin the declaration semantics).
#[tokio::test(flavor = "current_thread")]
async fn worker_import_scripts_expose_declarations_to_worker_body() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const body = "importScripts('data:text/javascript,var%20pbVersion=42');" +
                        "self.onmessage = function(e) { self.postMessage(pbVersion + '|' + typeof window); };";
                    const w = new Worker(URL.createObjectURL(new Blob([body])));
                    w.onerror = e => reject(new Error('worker error: ' + e.message));
                    w.onmessage = e => resolve(e.data);
                    w.postMessage('go');
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("42|undefined"));
}

// #98: importScripts replays through the page realm (so declarations
// persist), which exposed the page's MutationObserver to PDD's wheel —
// its bundled scheduler picked the DOM branch and died on
// document.createTextNode. The replay window must mask the
// main-thread-only family like a Chrome worker global, and restore the
// page faces afterwards.
#[tokio::test(flavor = "current_thread")]
async fn worker_import_scripts_see_no_dom_globals() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const imp = "var __domProbe = [typeof MutationObserver, typeof document, typeof window,"
                        + " typeof localStorage, typeof Promise, typeof Math].join('|');";
                    const url = 'data:text/javascript;base64,' + btoa(imp);
                    const body = "importScripts(" + JSON.stringify(url) + ");"
                        + "self.onmessage = function(e) { self.postMessage(__domProbe); };";
                    const w = new Worker(URL.createObjectURL(new Blob([body])));
                    w.onerror = e => reject(new Error('worker error: ' + e.message));
                    w.onmessage = e => resolve([e.data,
                        typeof MutationObserver, typeof document, typeof window].join('#'));
                    w.postMessage('go');
                    setTimeout(() => reject(new Error('no reply')), 2000);
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(
            "undefined|undefined|undefined|undefined|function|object#function#object#object"
        ),
        "imports see Chrome-worker reality; the page global is fully restored after the replay"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn worker_import_scripts_failed_target_fires_error_event() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"new Promise((resolve) => {
                    const body = "importScripts('data:text/javascript;base64,!!!!');" +
                        "self.onmessage = function(e) { self.postMessage('unreached'); };";
                    const w = new Worker(URL.createObjectURL(new Blob([body])));
                    w.onerror = e => resolve('err|' + e.message);
                    w.onmessage = e => resolve('msg|' + e.data);
                    w.postMessage('go');
                })"#,
            true,
            true,
        )
        .await
        .unwrap();
    let msg = result.value.unwrap();
    let msg = msg.as_str().unwrap();
    assert!(
        msg.starts_with("err|"),
        "worker delivered instead of erroring: {msg}"
    );
    assert!(
        msg.contains("failed to load"),
        "error lost the importScripts origin: {msg}"
    );
}

#[test]
fn worker_import_url_extraction_skips_dynamic_arguments() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate(
                r#"return globalThis.__ditingWorkerImportUrls(
                    "importScripts('a.js', \"b.js\"); importScripts(runtimePath); importScripts()")"#,
            )
            .unwrap();
    assert_eq!(result, serde_json::json!(["a.js", "b.js"]));
}

// A bare `onmessage = fn` (no self. prefix) inside the worker body must
// land on the worker scope, not create window.onmessage — the plain
// Function wrapper let the assignment fall through to the page global
// and the worker never received anything (obscura#867 family).
#[tokio::test(flavor = "current_thread")]
async fn worker_bare_onmessage_assignment_receives_messages() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"new Promise((resolve, reject) => {
                    const before = globalThis.onmessage;
                    const body = "onmessage = function(e) { postMessage('bare|' + e.data); };";
                    const w = new Worker(URL.createObjectURL(new Blob([body])));
                    w.onerror = e => reject(new Error('worker error: ' + e.message));
                    w.onmessage = e => resolve([e.data, globalThis.onmessage === before]);
                    w.postMessage('go');
                })"#,
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["bare|go", true]),
        "bare onmessage works and leaves the page global untouched"
    );
}

// #213: the SharedWorker stub swallowed every port.postMessage and never
// replied. 抖店's sif-dxs security layer builds its request-signing channel
// on exactly this shape (self.onconnect → event.ports[0] → port.start(),
// then every page request waits on it), so the page boot died on the SDK's
// own timeout with a blank screen. Messages posted before the script
// attached its handler must be buffered, not dropped.
#[tokio::test(flavor = "current_thread")]
async fn shared_worker_port_roundtrips_messages() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const body = "self.onconnect = function(e) {"
                        + "  const p = e.ports[0];"
                        + "  p.onmessage = function(ev) { p.postMessage('sw|' + ev.data); };"
                        + "  p.start();"
                        + "};";
                    const w = new SharedWorker(URL.createObjectURL(new Blob([body])), 'probe');
                    const got = [];
                    w.port.onmessage = e => { got.push(e.data); if (got.length === 2) resolve(got); };
                    w.port.postMessage('a');
                    w.port.postMessage('b');
                    setTimeout(() => reject(new Error('no reply: ' + JSON.stringify(got))), 2000);
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(["sw|a", "sw|b"]));
}

// Window timer functions are native-shaped ("Illegal invocation" for a
// foreign receiver). A worker script runs under with(self), so its bare
// setTimeout/setInterval calls arrive with the worker scope as receiver —
// that must stay legal. 抖店's sif-dxs worker calls setInterval at boot;
// the receiver check broke it and the page never got past its skeleton.
#[tokio::test(flavor = "current_thread")]
async fn shared_worker_scope_can_call_window_timers() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const body = "var ticks = 0;"
                        + "var iv = setInterval(function() { ticks++; if (ticks === 2) clearInterval(iv); }, 5);"
                        + "setTimeout(function() {}, 1);"
                        + "self.onconnect = function(e) {"
                        + "  const p = e.ports[0];"
                        + "  p.onmessage = function() { setTimeout(function() { p.postMessage('ticks=' + ticks); }, 60); };"
                        + "  p.start();"
                        + "};";
                    const w = new SharedWorker(URL.createObjectURL(new Blob([body])), 'timers');
                    w.port.onmessage = e => resolve(e.data);
                    w.port.postMessage('go');
                    setTimeout(() => reject(new Error('worker timers broke')), 2000);
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("ticks=2"));
}

// #98: the with(self) wrapper only shadows names the scope object
// carries — everything else fell through to the page realm, so PDD's
// sharedWorker polyfill saw the page's MutationObserver, picked its DOM
// scheduling branch (document.createTextNode on a text node) and died.
// Chrome workers have no DOM at all: those names must read undefined,
// while genuine worker globals keep resolving.
#[tokio::test(flavor = "current_thread")]
async fn shared_worker_scope_masks_main_thread_globals() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const body = "self.onconnect = function(e) {"
                        + "  const p = e.ports[0];"
                        + "  const probe = [typeof globalThis.MutationObserver, typeof globalThis.document,"
                        + "    typeof globalThis.window].join('|');"
                        + "  p.onmessage = function() {"
                        + "    p.postMessage(probe + '|' + [typeof MutationObserver, typeof document, typeof window,"
                        + "      typeof HTMLElement, typeof localStorage, typeof Promise, typeof Math].join('|'));"
                        + "  };"
                        + "  p.start();"
                        + "};";
                    const w = new SharedWorker(URL.createObjectURL(new Blob([body])), 'mask');
                    w.port.onmessage = e => resolve(e.data);
                    w.port.postMessage('x');
                    setTimeout(() => reject(new Error('no reply')), 2000);
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(
            "undefined|undefined|undefined|undefined|undefined|undefined|undefined|undefined|function|object"
        ),
        "DOM names read undefined in the worker scope — bare names and globalThis reads during the synchronous dispatch — while real globals still fall through"
    );
}

// #98: BroadcastChannel delivers as a task to every OTHER open channel
// of the same name, payload structured-cloned per receiver — the old
// postMessage(){} stub hung PDD's titan handshake (replyStatus over
// BroadcastChannel('WebSocketChannel')) and with it mms goods_list.
#[tokio::test(flavor = "current_thread")]
async fn broadcast_channel_delivers_cloned_tasks() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const a1 = new BroadcastChannel('bus');
                    const a2 = new BroadcastChannel('bus');
                    const a3 = new BroadcastChannel('bus');
                    const b1 = new BroadcastChannel('other');
                    const events = [];
                    a2.onmessage = e => { events.push('a2:' + e.data.v + ':' + (e.target === a2)); e.data.v = 'mutated'; };
                    a3.onmessage = e => events.push('a3:' + e.data.v);
                    b1.onmessage = e => events.push('b1:' + e.data.v);
                    a1.onmessage = e => events.push('self:' + e.data.v);
                    a1.postMessage({ v: 'hello' });
                    a1.postMessage({ v: 'second' });
                    setTimeout(() => {
                        a1.close();
                        let threw = '';
                        try { a1.postMessage({ v: 'x' }); } catch (err) { threw = err.name; }
                        a2.close(); a3.close(); b1.close();
                        resolve(events.join('|') + '#' + threw);
                    }, 300);
                    setTimeout(() => reject(new Error('no delivery: ' + JSON.stringify(events))), 2000);
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("a2:hello:true|a3:hello|a2:second:true|a3:second#InvalidStateError"),
        "delivery reaches the same-named channel only, as a task, with per-receiver clones (a2's mutation never reaches a3); postMessage after close throws InvalidStateError"
    );
}

// #98: the PDD topology — a SharedWorker replies through
// BroadcastChannel, not through the port. The page listens on the bus
// and only then posts createSocket on the port.
#[tokio::test(flavor = "current_thread")]
async fn shared_worker_replies_via_broadcast_channel() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const bus = new BroadcastChannel('WebSocketChannel');
                    const body = "self.onconnect = function(e) {"
                        + "  const p = e.ports[0];"
                        + "  const bc = new BroadcastChannel('WebSocketChannel');"
                        + "  p.onmessage = function(ev) {"
                        + "    if (ev.data.type !== 'queryStatus') return;"
                        + "    bc.postMessage({ type: 'replyStatus', data: { uniqueId: ev.data.data.uniqueId } });"
                        + "  };"
                        + "  p.start();"
                        + "};";
                    const w = new SharedWorker(URL.createObjectURL(new Blob([body])), 'bc');
                    w.port.start();
                    bus.onmessage = e => {
                        bus.close();
                        resolve(e.data.type + ':' + e.data.data.uniqueId);
                    };
                    w.port.postMessage({ type: 'queryStatus', data: { uniqueId: 'u1' } });
                    setTimeout(() => reject(new Error('no bus reply')), 2000);
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("replyStatus:u1"),
        "the worker's BroadcastChannel post reaches the page-side listener — same name, cross-context"
    );
}

// Same (url, name) is ONE shared script instance, not one copy per
// construction: its connect counter advances across both clients. The id
// is captured at connect time — reading the shared self.__n at reply time
// would give 2 to both clients whether or not they share a scope, so it
// could not tell sharing from not-sharing.
#[tokio::test(flavor = "current_thread")]
async fn shared_worker_same_key_shares_one_instance() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"new Promise((resolve, reject) => {
                    const body = "self.__n = 0;"
                        + "self.onconnect = function(e) {"
                        + "  const id = ++self.__n;"
                        + "  const p = e.ports[0];"
                        + "  p.onmessage = function() { p.postMessage(id); };"
                        + "};";
                    const u = URL.createObjectURL(new Blob([body]));
                    const a = new SharedWorker(u, 'shared');
                    const b = new SharedWorker(u, 'shared');
                    const seen = [];
                    const done = v => { seen.push(v); if (seen.length === 2) resolve(seen); };
                    a.port.onmessage = e => done(e.data);
                    b.port.onmessage = e => done(e.data);
                    a.port.postMessage('x');
                    b.port.postMessage('x');
                    setTimeout(() => reject(new Error('no reply: ' + JSON.stringify(seen))), 2000);
                })"#,
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([1, 2]),
        "both clients must reach the same scope"
    );
}

// A different name is a different instance, so neither client sees the
// other's connects.
#[tokio::test(flavor = "current_thread")]
async fn shared_worker_distinct_names_are_distinct_instances() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"new Promise((resolve, reject) => {
                    const body = "self.__n = 0;"
                        + "self.onconnect = function(e) {"
                        + "  const id = ++self.__n;"
                        + "  const p = e.ports[0];"
                        + "  p.onmessage = function() { p.postMessage(id); };"
                        + "};";
                    const u = URL.createObjectURL(new Blob([body]));
                    const a = new SharedWorker(u, 'one');
                    const b = new SharedWorker(u, 'two');
                    const seen = [];
                    const done = v => { seen.push(v); if (seen.length === 2) resolve(seen); };
                    a.port.onmessage = e => done(e.data);
                    b.port.onmessage = e => done(e.data);
                    a.port.postMessage('x');
                    b.port.postMessage('x');
                    setTimeout(() => reject(new Error('no reply: ' + JSON.stringify(seen))), 2000);
                })"#,
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([1, 1]));
}

// MessagePort was a no-op stub too: a listener registered through
// addEventListener never heard anything. Delivery starts on start() (or on
// the first listener), and messages posted before that are buffered.
#[tokio::test(flavor = "current_thread")]
async fn message_port_buffers_until_start_and_supports_listeners() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"new Promise((resolve) => {
                    const ch = new MessageChannel();
                    const got = [];
                    ch.port2.addEventListener('message', e => got.push(e.data));
                    ch.port1.postMessage('one');
                    ch.port1.postMessage('two');
                    setTimeout(() => resolve([
                        got,
                        ch.port1 instanceof MessagePort,
                        ch.port2 instanceof MessagePort,
                    ]), 50);
                })"#,
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([["one", "two"], true, true])
    );
}

// Anti-fraud SDKs probe the worker's HTTP surface before using it and
// bail with "no supported http request object" when it is bare. fetch is
// on the synthetic scope; XMLHttpRequest resolves through to the shared
// realm's constructor — pin all three readable by bare identifier
// (obscura#851 family).
#[tokio::test(flavor = "current_thread")]
async fn worker_scope_exposes_http_request_surface() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                r#"new Promise((resolve, reject) => {
                    const body = "onmessage = function(e) { postMessage(typeof fetch + '|' + typeof XMLHttpRequest + '|' + typeof Request); };";
                    const w = new Worker(URL.createObjectURL(new Blob([body])));
                    w.onerror = e => reject(new Error('worker error: ' + e.message));
                    w.onmessage = e => resolve(e.data);
                    w.postMessage('go');
                })"#,
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("function|function|function")
    );
}

// A real worker executes its top-level script the moment the source
// lands — collectors that only set up timers/probes and never wait for
// a parent message must still run. The old flow deferred the whole body
// to the first postMessage, so message-less workers never booted
// (obscura#851 family).
#[tokio::test(flavor = "current_thread")]
async fn worker_top_level_runs_without_any_postmessage() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate_remote(
            r#"new Promise((resolve, reject) => {
                    const body = "postMessage('booted');";
                    const w = new Worker(URL.createObjectURL(new Blob([body])));
                    w.onerror = e => reject(new Error('worker error: ' + e.message));
                    w.onmessage = e => resolve(e.data);
                })"#,
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("booted"));
}

/// Upstream obscura #704: postMessage's targetOrigin argument must gate
/// delivery — '*' or a matching origin delivers, a mismatched origin
/// drops silently (browsers never throw), '/' requires same-origin with
/// the calling document. The pre-fix wrappers delivered unconditionally,
/// leaking caller-restricted payloads to whatever frame was targeted.
#[tokio::test(flavor = "current_thread")]
async fn test_post_message_target_origin_gates_delivery() {
    let mut rt = setup_runtime(
        "<html><body><iframe src=\"https://frame.example/widget\"></iframe></body></html>",
    );

    // One async eval drives all three gates: mismatched targetOrigin
    // drops silently (iframe origin is frame.example; the caller
    // restricted delivery to trusted.example), matching origin delivers,
    // '*' wildcard delivers.
    let result = rt
        .evaluate_remote(
            "(async function(){ \
                window.__leak = []; \
                window.addEventListener('message', function(e){ window.__leak.push(e.data) }); \
                const w = document.querySelector('iframe').contentWindow; \
                w.postMessage('secret', 'https://trusted.example'); \
                await new Promise(r => setTimeout(r, 20)); \
                const afterMismatch = window.__leak.slice(); \
                w.postMessage('hello', 'https://frame.example'); \
                await new Promise(r => setTimeout(r, 20)); \
                const afterMatch = window.__leak.slice(); \
                w.postMessage('wild', '*'); \
                await new Promise(r => setTimeout(r, 20)); \
                return [afterMismatch, afterMatch, window.__leak.slice()]; \
            })()",
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([[], ["hello"], ["hello", "wild"],])
    );
}

/// Same-origin '/' targetOrigin delivers on a same-origin frame and the
/// self-targeted window.postMessage(path) honors the same gate.
#[tokio::test(flavor = "current_thread")]
async fn test_post_message_same_origin_slash_and_self_gate() {
    let mut rt = setup_runtime(
        "<html><body><iframe src=\"http://example.com/frame\"></iframe></body></html>",
    );
    rt.set_url("http://example.com/test");

    // '/': iframe origin equals page origin → deliver. Self-targeted
    // with mismatched explicit origin → drop. Self-targeted matching
    // origin → deliver.
    rt.evaluate(
            "(function(){ window.__got = []; window.addEventListener('message', function(e){ window.__got.push(e.data) }); document.querySelector('iframe').contentWindow.postMessage('same-origin', '/'); postMessage('self-mismatch', 'https://other.example'); postMessage('self-ok', 'http://example.com'); })()",
        )
        .unwrap();

    let got = rt
        .evaluate_remote(
            "new Promise(r => setTimeout(() => r(window.__got), 50))",
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        got.value.unwrap(),
        serde_json::json!(["same-origin", "self-ok"])
    );
}

/// #161 / obscura#1038 family: send() while CONNECTING buffers (Chrome
/// semantics) instead of throwing InvalidStateError — the queued frames
/// flush in order once the handshake completes, and bufferedAmount
/// tracks the queued bytes until then.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn websocket_send_buffers_while_connecting() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        use futures_util::StreamExt;
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("ws handshake");
        let mut got = Vec::new();
        // Read until both queued frames arrive or the peer stalls.
        while got.len() < 2 {
            match tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await {
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) => got.push(t),
                Ok(Some(Ok(_))) => continue,
                _ => break,
            }
        }
        got
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const ws = new WebSocket('ws://127.0.0.1:__PORT__/');
                    let sent = 'no-throw';
                    let queued = -1;
                    try { ws.send('early-1'); ws.send('early-2'); } catch (e) { sent = 'throw:' + e.name; }
                    queued = ws.bufferedAmount;
                    await new Promise((res) => { ws.onopen = res; ws.onerror = res; });
                    return { sent, queued, state: ws.readyState, after: ws.bufferedAmount };
                }"#
                    .replace("__PORT__", &port.to_string())
                    .as_str(),
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();

    let v = result.value.unwrap();
    assert_eq!(
        v["sent"],
        serde_json::json!("no-throw"),
        "send at CONNECTING must buffer, not throw"
    );
    assert_eq!(
        v["queued"],
        serde_json::json!(14),
        "bufferedAmount counts the queued bytes (7+7)"
    );
    assert_eq!(v["state"], serde_json::json!(1), "open resolved");
    assert_eq!(
        v["after"],
        serde_json::json!(0),
        "bufferedAmount resets after the flush"
    );

    let got = server.await.unwrap();
    assert_eq!(
        got,
        vec!["early-1".to_string(), "early-2".to_string()],
        "queued frames flush in order"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_message_delivery_waits_for_dispatch_window() {
    // MessageChannel delivery must never fire inside a synchronous event
    // dispatch. The op_sleep continuation a post resolves to is a
    // microtask that lands at the next op checkpoint — routinely inside
    // a listener on heavy pages. React 16's scheduler re-entered its own
    // commit that way and threw #327 ("This root is already working"),
    // after which the root stopped committing anything (taobao publish
    // form frozen: every setState queued forever).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const log = [];
            const ch = new MessageChannel();
            ch.port1.onmessage = () => log.push('msg@' + globalThis.__phase);
            let phase = 'start';
            globalThis.__phase = 'start';
            globalThis.__log = log;
            document.addEventListener('ping', () => {
                ch.port2.postMessage('x');      // due immediately, but the dispatch window is open
                setTimeout(() => {}, 0);        // an op checkpoint inside the listener
                phase = 'in-dispatch';
                globalThis.__phase = phase;
            });
            document.dispatchEvent(new Event('ping'));
            globalThis.__phase = 'after-dispatch';
            return new Promise((resolve) => {
                setTimeout(() => resolve({ log: globalThis.__log.slice() }), 30);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let log = v["log"].as_array().unwrap();
    assert_eq!(log.len(), 1, "message delivered exactly once: {:?}", log);
    assert_eq!(
        log[0], "msg@after-dispatch",
        "delivery must wait for the dispatch window to close: {:?}",
        log
    );
}

/// #237: a MessageChannel arm is a macrotask; a severed op chain means
/// its closure never runs. Splicing the queued closure out of `_mtQueue`
/// is the deterministic stand-in for that loss (observed live on fxg:
/// React 18's scheduler latches `isMessageLoopScheduled` on its first
/// post, so one lost delivery wedges every later async update — the
/// doudian upload stuck at 上传中1%). After the stall window the sweep
/// must re-arm and deliver exactly once, with clean bookkeeping.
#[tokio::test(flavor = "current_thread")]
async fn test_port_stalled_message_recovered_by_sweep() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const ch = new MessageChannel();
            const got = [];
            globalThis.__got = got;
            ch.port1.onmessage = (e) => got.push(e.data);
            ch.port2.postMessage('survivor');
            // Kill the armed delivery: the closure is the last item pushed
            // (same synchronous block), so splice it off the tail.
            const dropped = _mtQueue.splice(_mtQueue.length - 1, 1);
            globalThis.__dropped = dropped.length;
            // Age the stall window past the 5s threshold, then run one sweep.
            ch.port1._pending[0].since -= 6000;
            __ditingPortSweep();
            return new Promise((resolve) => {
                setTimeout(() => resolve({
                    got: globalThis.__got.slice(),
                    dropped: globalThis.__dropped,
                    left: ch.port1._pending.length,
                }), 50);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(v["dropped"].as_i64(), Some(1), "the arm was really removed");
    let got = v["got"].as_array().unwrap();
    assert_eq!(
        got,
        &[serde_json::json!("survivor")],
        "sweep re-arm must deliver the stalled message: {:?}",
        got
    );
    assert_eq!(
        v["left"].as_i64(),
        Some(0),
        "pending list empties (and a lost arm leaves no counter residue)"
    );
}

/// #237: termination recovery is the engine's "nothing is executing"
/// moment — still-pending port arms there are necessarily orphaned, so
/// the recovery hook re-arms them immediately instead of waiting out the
/// 5s sweep. No stall aging involved.
#[tokio::test(flavor = "current_thread")]
async fn test_port_recovery_on_termination_recover() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const ch = new MessageChannel();
            const got = [];
            globalThis.__got = got;
            ch.port1.onmessage = (e) => got.push(e.data);
            ch.port2.postMessage('after-terminate');
            _mtQueue.splice(_mtQueue.length - 1, 1);
            __diting_mt_recover_termination();
            return new Promise((resolve) => {
                setTimeout(() => resolve({ got: globalThis.__got.slice() }), 50);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let got = v["got"].as_array().unwrap();
    assert_eq!(
        got,
        &[serde_json::json!("after-terminate")],
        "recovery hook must re-arm orphaned messages: {:?}",
        got
    );
}

/// #237: re-arming must be at-least-once, never at-least-twice. If the
/// sweep fires while the original arm is still queued (a #230-style
/// backlog, not a loss), both closures run — the per-entry generation
/// must turn the superseded one into a no-op so the page sees exactly
/// one message event.
#[tokio::test(flavor = "current_thread")]
async fn test_port_sweep_rarm_does_not_duplicate() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const ch = new MessageChannel();
            const got = [];
            globalThis.__got = got;
            ch.port1.onmessage = (e) => got.push(e.data);
            ch.port2.postMessage('only-once');
            // Original arm still queued — only age the window and sweep.
            // Sibling recovery test ages `_pending[0].since`; `_pendSince`
            // was a typo and never crossed the stall gate, so the sweep
            // never re-armed and this pin never actually saw two arms.
            ch.port1._pending[0].since -= 6000;
            __ditingPortSweep();
            return new Promise((resolve) => {
                setTimeout(() => resolve({
                    got: globalThis.__got.slice(),
                    // #330: the queue consumes under a head cursor — the
                    // executing item itself still occupies its slot, so the
                    // live backlog is length - head (what shift() used to
                    // hide by splicing the running item out). Prefer the
                    // public probe so the pin tracks the same face pages use.
                    qlen: globalThis.__MT_STATE().qlen,
                }), 80);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let got = v["got"].as_array().unwrap();
    assert_eq!(
        got.len(),
        1,
        "superseded arm must no-op; exactly one delivery: {:?}",
        got
    );
    assert_eq!(
        got[0],
        serde_json::json!("only-once"),
        "payload intact through the re-arm"
    );
    assert_eq!(v["qlen"].as_i64(), Some(0), "queue drains fully");
}

/// #249: soft PORT-RECOVER under a live `_mtQueue` backlog must NOT bump
/// `gen`. Re-arming demotes the still-queued original arm to the tail
/// (and no-ops it), which is exactly the PDD goods_edit self-starvation
/// loop (PORT-RECOVER wait climbing every 5s). Soft recover returns 0
/// while qlen>0 && !mt_stalled; the original arm delivers once.
#[tokio::test(flavor = "current_thread")]
async fn test_port_soft_recover_skips_live_backlog() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const ch = new MessageChannel();
            const got = [];
            globalThis.__got = got;
            ch.port1.onmessage = (e) => got.push(e.data);
            // Artificial backlog ahead of the port arm.
            for (let i = 0; i < 200; i++) setTimeout(() => {}, 0);
            ch.port2.postMessage('keep-front');
            const genBefore = ch.port1._pending[0].gen;
            ch.port1._pending[0].since -= 6000;
            const recovered = __ditingPortSweep();
            const genAfter = ch.port1._pending[0] ? ch.port1._pending[0].gen : -1;
            return new Promise((resolve) => {
                setTimeout(() => resolve({
                    got: globalThis.__got.slice(),
                    genBefore,
                    genAfter,
                    recovered,
                    qlen: globalThis.__MT_STATE().qlen,
                }), 200);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["genBefore"].as_i64(),
        v["genAfter"].as_i64(),
        "soft sweep must not bump gen under live backlog: {:?}",
        v
    );
    let got = v["got"].as_array().unwrap();
    assert_eq!(
        got,
        &[serde_json::json!("keep-front")],
        "original arm still delivers exactly once: {:?}",
        got
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_terminated_payload_severed_chain_self_heals() {
    // #39: the #37 escape hatch was push-counted (retire after 1000
    // stale pushes) — it needs the page to keep pushing. A page whose
    // timers are self-chained setTimeouts stops pushing the moment the
    // chain dies (its callbacks never run, so nothing schedules the
    // next one); observed live on a wedged fxg create page as
    // {qlen:160819, stale:0, armed:true}. This reproduces the kill —
    // a terminate_execution landing while a payload spins inside the
    // drain frame unwinds the frame itself (the boot-time heap guard
    // does exactly this) — then requires a quiet page to recover with
    // zero further pushes: the only push after the kill is the probe
    // timer's own delivery.
    let mut rt = setup_runtime("<html><body></body></html>");
    let wd = rt.arm_watchdog(std::time::Duration::from_millis(400));
    let script = r#"() => {
            setTimeout(() => { while (true) {} }, 0);
            return new Promise((resolve) => {
                setTimeout(() => resolve('healed'), 100);
            });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let fired = rt.disarm_watchdog(wd);
    assert!(fired, "watchdog must have fired: the spin was terminated");
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("healed"),
        "a quiet page must self-heal a severed chain without pushes"
    );
}
