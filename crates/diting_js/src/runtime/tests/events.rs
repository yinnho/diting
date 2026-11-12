//! 事件派发/构造 + 焦点/点击激活/pointer 家族
use super::super::*;
use super::common::setup_runtime;

/// #335: window.event — the legacy IE global lives for the whole
/// synchronous dispatch. Chrome 157 pinned 2026-11-08: 'click' inside the
/// target handler AND every listener along the bubble path, nested
/// dispatchEvent restores the OUTER event when the inner returns (stack
/// discipline, not clear-to-undefined), undefined the moment the outermost
/// dispatchEvent returns, and never visible to async callbacks.
#[tokio::test(flavor = "current_thread")]
async fn window_event_during_dispatch() {
    let mut rt = setup_runtime("<html><body><button>b</button></body></html>");
    let out = rt.evaluate(r#"
        var b = document.querySelector('button');
        var r = {};
        var saw;
        b.onclick = function () { saw = window.event ? window.event.type : 'undefined'; };
        b.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        r.in_handler = saw;
        var docSaw;
        document.addEventListener('click', function () {
            docSaw = window.event ? window.event.type : 'undefined';
        });
        b.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        r.on_bubble_path = docSaw;
        r.after_return = String(window.event);
        // nested dispatch: the inner overwrites, its return restores the outer
        var nestedOuter, nestedInnerBack;
        var inner = new Event('inner');
        b.addEventListener('outer', function () {
            nestedOuter = window.event.type;
            var innerSaw;
            b.addEventListener('inner', function () { innerSaw = window.event.type; }, { once: true });
            b.dispatchEvent(inner);
            nestedInnerBack = window.event.type;
        }, { once: true });
        b.dispatchEvent(new Event('outer'));
        r.nested_outer = nestedOuter;
        r.nested_inner_back = nestedInnerBack;
        setTimeout(function () { window.__asyncSaw = String(window.event); }, 0);
        b.dispatchEvent(new Event('click'));
        JSON.stringify(r);
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["in_handler"],
        serde_json::json!("click"),
        "target handler"
    );
    assert_eq!(
        v["on_bubble_path"],
        serde_json::json!("click"),
        "bubble path"
    );
    assert_eq!(
        v["after_return"],
        serde_json::json!("undefined"),
        "cleared after"
    );
    assert_eq!(v["nested_outer"], serde_json::json!("outer"));
    assert_eq!(
        v["nested_inner_back"],
        serde_json::json!("outer"),
        "inner dispatch return restores the outer event, not undefined"
    );
    let _ = rt.run_event_loop_bounded(100).await;
    let async_saw = rt.evaluate("window.__asyncSaw || 'never'").unwrap();
    assert_eq!(
        async_saw.as_str().unwrap_or("never"),
        "undefined",
        "async callbacks never see window.event"
    );
}

#[test]
fn test_button_click_dispatches_listener() {
    let mut rt = setup_runtime(r#"<button id="go">Go</button>"#);
    let result = rt
        .evaluate(
            r#"
            const button = document.getElementById('go');
            button.addEventListener('click', () => { button.dataset.clicked = 'yes'; });
            button.click();
            return button.dataset.clicked;
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!("yes"));
}

#[test]
fn test_dispatch_mouse_event_runs_listener() {
    let mut rt = setup_runtime(r#"<button id="go">Go</button>"#);
    let result = rt
        .evaluate(
            r#"
            const button = document.getElementById('go');
            let count = 0;
            button.addEventListener('click', () => { count += 1; });
            button.dispatchEvent(new MouseEvent('click', { bubbles: true }));
            return count;
        "#,
        )
        .unwrap();
    assert_eq!(result.as_f64().unwrap() as i64, 1);
}

#[test]
fn test_dispatchevent_click_runs_activation_like_chrome() {
    // Chrome runs activation behavior for untrusted clicks too:
    // `cb.dispatchEvent(new MouseEvent('click'))` toggles the checkbox,
    // a preventDefault listener cancels the flip, and a synthetic click
    // on a label forwards to its labeled control (obscura#826 lineage —
    // our .click()/label.click() already matched; this closes the
    // dispatchEvent arm).
    let mut rt = setup_runtime(
        r#"<input type=checkbox id=syn><input type=checkbox id=cxl>
            <label for=syn id=lb>go</label><input type=radio name=g id=r1><input type=radio name=g id=r2 checked>"#,
    );
    let result = rt.evaluate(r#"
            const syn = document.getElementById('syn'), cxl = document.getElementById('cxl');
            let changes = 0;
            syn.addEventListener('change', () => changes++);
            syn.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));
            const synAfter = syn.checked;
            cxl.addEventListener('click', (e) => e.preventDefault());
            cxl.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));
            const cxlAfter = cxl.checked;
            document.getElementById('lb').dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));
            document.getElementById('r2').dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));
            return [synAfter, cxlAfter, syn.checked, changes,
                    document.getElementById('r1').checked, document.getElementById('r2').checked];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true,  // synthetic dispatch toggles
            false, // preventDefault cancels the flip
            false, // label dispatch forwards and toggles back
            2,     // change fired for both syn toggles
            false, // peer radio stays unchecked
            true,  // clicked radio checked
        ])
    );
}

#[test]
fn test_click_activation_flips_checked_without_page_traps() {
    // React 16's `_valueTracker` installs an own-property `checked` trap
    // on controlled radios/checkboxes whose setter mirrors every JS
    // assignment into a shadow "last value"; the change extraction later
    // diffs the DOM state against that shadow. Chrome's pre-click
    // activation flips `checked` in C++ — the page trap never runs, so
    // the diff sees false→true and onChange fires. An engine that flips
    // through the plain JS property routes the activation into the trap,
    // the shadow syncs ahead of the click event, the diff comes up empty
    // and every controlled radio onChange goes deaf (the #119 freight
    // radio face: model stuck on seller-pays). Activation — and its
    // cancel-restore — must call the prototype's native setter.
    let mut rt = setup_runtime(
        r#"<input type=radio name=g id=r1><input type=radio name=g id=r2 checked>
            <input type=checkbox id=cxl>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const proto = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'checked');
            const hijack = (el) => {
                el.__trapCount = 0;
                Object.defineProperty(el, 'checked', {
                    get() { return proto.get.call(el); },
                    set(v) { el.__trapCount++; el.__shadow = '' + v; proto.set.call(el, v); },
                });
            };
            const r1 = document.getElementById('r1'), r2 = document.getElementById('r2'),
                  cxl = document.getElementById('cxl');
            [r1, r2, cxl].forEach(hijack);
            let fired = [];
            r1.addEventListener('change', () => fired.push('r1'));
            r1.click();                      // flips r1, unchecks peer r2
            const afterClick = [r1.checked, r2.checked, fired,
                                r1.__trapCount, r2.__trapCount, el0(r1)];
            cxl.addEventListener('click', (e) => e.preventDefault());
            cxl.click();                     // cancelled: restores without the trap
            const afterCancel = [cxl.checked, cxl.__trapCount, el0(cxl)];
            function el0(el) { return ('__shadow' in el) ? el.__shadow : null; }
            return [afterClick, afterCancel];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            [true, false, ["r1"], 0, 0, null], // DOM flipped, change fired, traps never ran
            [false, 0, null],                  // cancel restored through the native path too
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_unhandled_rejection_dispatches_window_events() {
    // Chrome surfaces unhandled promise rejections on the window as
    // PromiseRejectionEvents (obscura#797 lineage): `unhandledrejection`
    // carries the real promise + reason, and `rejectionhandled` fires
    // when a handler is attached late. deno_core routes both through the
    // core callbacks bootstrap registers.
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(r#"
            globalThis.__seen = [];
            addEventListener('unhandledrejection', function (e) {
                __seen.push(['unhandledrejection', String(e.reason && e.reason.message), e.promise instanceof Promise, e.cancelable]);
            });
            addEventListener('rejectionhandled', function (e) {
                __seen.push(['rejectionhandled', e.promise instanceof Promise]);
            });
            Promise.reject(new Error('boom-xyz'));
            globalThis.__late = Promise.reject(new Error('late-1'));
        "#).unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    rt.evaluate("globalThis.__late.catch(function () {})")
        .unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let result = rt.evaluate("globalThis.__seen").unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            ["unhandledrejection", "boom-xyz", true, true],
            ["unhandledrejection", "late-1", true, true],
            ["rejectionhandled", true],
        ])
    );
}

// #60: engine input paths carry isTrusted=true — Chrome's editing pipeline
// (execCommand insertText) and native file selection fire trusted events,
// and a site reading isTrusted on composer input is the cheapest automation
// tell there is. Page-JS events must STAY false with isTrusted as the only
// own key (the #303 face this fix must not regress).
#[tokio::test(flavor = "current_thread")]
async fn engine_input_events_are_trusted_and_page_events_stay_false() {
    let mut rt = setup_runtime("<html><body><input id=i></body></html>");
    let result = rt
        .evaluate(
            r#"return (() => {
                    const out = {};
                    const i = document.getElementById('i');
                    const e = new Event('x');
                    out.pageEvent = e.isTrusted;
                    out.ownKeys = Object.getOwnPropertyNames(e);
                    const seen = {};
                    i.addEventListener('beforeinput', (ev) => { seen.bi = ev.isTrusted; });
                    i.addEventListener('input', (ev) => { seen.in = ev.isTrusted; });
                    i.focus();
                    document.execCommand('insertText', false, 'hi');
                    out.execCmd = [seen.bi, seen.in, i.value];
                    return out;
                })()"#,
        )
        .unwrap();
    let out = result.as_object().expect("object result");
    assert_eq!(
        out["pageEvent"],
        serde_json::json!(false),
        "page-JS events stay untrusted"
    );
    assert_eq!(out["ownKeys"], serde_json::json!(["isTrusted"]));
    assert_eq!(
        out["execCmd"],
        serde_json::json!([true, true, "hi"]),
        "execCommand rides the editing pipeline — its beforeinput/input must be trusted like Chrome's"
    );
}

#[test]
fn test_create_event_custom_event_has_init_method() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let kind = rt
        .evaluate("typeof document.createEvent('CustomEvent').initCustomEvent")
        .unwrap();
    assert_eq!(kind, serde_json::json!("function"));
}

#[test]
fn test_init_custom_event_sets_fields() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.execute_script(
        "test",
        r#"
            globalThis.__e = document.createEvent('CustomEvent');
            globalThis.__e.initCustomEvent('myevent', true, false, {hello: 'world'});
        "#,
    )
    .unwrap();
    let t = rt.evaluate("globalThis.__e.type").unwrap();
    assert_eq!(t, serde_json::json!("myevent"));
    let b = rt.evaluate("globalThis.__e.bubbles").unwrap();
    assert_eq!(b, serde_json::json!(true));
    let c = rt.evaluate("globalThis.__e.cancelable").unwrap();
    assert_eq!(c, serde_json::json!(false));
    let d = rt.evaluate("globalThis.__e.detail.hello").unwrap();
    assert_eq!(d, serde_json::json!("world"));
}

#[test]
fn test_create_event_returns_correct_class() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let cust = rt
        .evaluate("document.createEvent('CustomEvent') instanceof CustomEvent")
        .unwrap();
    assert_eq!(cust, serde_json::json!(true));
    let mouse = rt
        .evaluate("document.createEvent('MouseEvent') instanceof MouseEvent")
        .unwrap();
    assert_eq!(mouse, serde_json::json!(true));
    let mouses = rt
        .evaluate("document.createEvent('MouseEvents') instanceof MouseEvent")
        .unwrap();
    assert_eq!(mouses, serde_json::json!(true));
    let kb = rt
        .evaluate("document.createEvent('KeyboardEvent') instanceof KeyboardEvent")
        .unwrap();
    assert_eq!(kb, serde_json::json!(true));
}

#[test]
fn test_create_event_unknown_type_returns_event() {
    // 7e6f403 flipped the contract: unknown interface names now throw
    // NotSupportedError (Chrome behavior) instead of returning a generic
    // Event whose init* methods would all be missing.
    let mut rt = setup_runtime("<html><body></body></html>");
    let kind = rt
        .evaluate(
            r#"(() => {
                    try { document.createEvent('NotARealType'); return 'no-throw'; }
                    catch (e) { return e.name; }
                })()"#,
        )
        .unwrap();
    assert_eq!(kind, serde_json::json!("NotSupportedError"));
}

#[test]
fn test_event_constructor_webidl_semantics() {
    // Upstream af1e15f: no-arg constructors throw, type coerces to string,
    // CustomEvent.detail defaults to null, createEvent still builds "" type.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const out = [];
            try { new Event(); out.push('no-throw'); } catch (e) { out.push(e.name); }
            try { new CustomEvent(); out.push('no-throw'); } catch (e) { out.push(e.name); }
            out.push(new Event(123).type + ':' + typeof new Event(123).type);
            out.push(String(new CustomEvent('x').detail));
            out.push(String(new CustomEvent('x', { detail: 7 }).detail));
            out.push(new Event('click').type);
            out.push(document.createEvent('Event').type);
            return out.join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!("TypeError|TypeError|123:string|null|7|click|")
    );
}

#[test]
fn test_promise_rejection_event_requires_promise() {
    // Upstream 0ff1ba0 + 776c915: the promise member is required; the
    // class must exist globally (core-js feature-detects it).
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            const promise = Promise.resolve(1);
            const event = new PromiseRejectionEvent('unhandledrejection', { promise, reason: 'failed' });
            let missingThrows = false;
            try { new PromiseRejectionEvent('unhandledrejection'); } catch (e) { missingThrows = e instanceof TypeError; }
            let nullInitThrows = false;
            try { new PromiseRejectionEvent('unhandledrejection', {}); } catch (e) { nullInitThrows = e instanceof TypeError; }
            return [event instanceof Event, event.promise === promise, event.reason, missingThrows, nullInitThrows];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, true, "failed", true, true])
    );
}

#[test]
fn test_create_event_rejects_unknown_and_supports_legacy_aliases() {
    // Upstream 7e6f403: unknown interface names throw NotSupportedError;
    // the DOM Level 2 aliases and hashchange/message map entries resolve.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            let rejected = null;
            try { document.createEvent('NotAnEventInterface'); } catch (e) { rejected = [e.name, e instanceof DOMException]; }
            const aliases = ['Event', 'Events', 'HTMLEvents', 'SVGEvents'].map(name => {
                const event = document.createEvent(name);
                return [event instanceof Event, event.constructor === Event, event.type];
            });
            const hash = document.createEvent('HashChangeEvent') instanceof HashChangeEvent;
            const message = document.createEvent('MessageEvent') instanceof MessageEvent;
            let preRejects = null;
            try { document.createEvent('PromiseRejectionEvent'); } catch (e) { preRejects = e.name; }
            return [rejected, aliases, hash, message, preRejects];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            ["NotSupportedError", true],
            [
                [true, true, ""],
                [true, true, ""],
                [true, true, ""],
                [true, true, ""]
            ],
            true,
            true,
            "NotSupportedError"
        ])
    );
}

#[test]
fn test_pointer_event_class_matches_chrome_defaults() {
    // A weak `PointerEvent extends Event` used to squat on the name,
    // keeping the real MouseEvent subclass dead behind its typeof-guard.
    // Chrome shape: pointer events are MouseEvents; constructor defaults
    // are pointerId 0, pointerType '', isPrimary false, pressure 0,
    // width/height 1 — only real input carries 'mouse'/'pen'/'touch'.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const bare = new PointerEvent('pointerdown');
            const full = new PointerEvent('pointermove', {
                pointerId: 7, pointerType: 'touch', isPrimary: true,
                pressure: 0.25, clientX: 5, clientY: 6, button: 0,
            });
            return [
                bare instanceof MouseEvent, bare instanceof UIEvent, bare instanceof Event,
                bare.pointerId, bare.pointerType, bare.isPrimary, bare.pressure,
                bare.width, bare.height,
                full.pointerId, full.pointerType, full.isPrimary, full.pressure,
                full.clientX, full.bubbles,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true, true, true, 0, "", false, 0, 1, 1, 7, "touch", true, 0.25, 5, false,
        ])
    );
}

/// blitz#839's complaint through the whole stack: focus() must reach the
/// Rust tree (selector matching), so :focus/:focus-within/:focus-visible
/// rules actually re-style, and blur() clears them again. The old focus()
/// flipped a JS global only — every focus-dependent rule stayed inert.
#[cfg(feature = "screenshot")]
#[test]
fn test_focus_pseudo_styles_react_to_focus_and_blur() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             #q:focus { background-color: rgb(10, 20, 30); }\
             form:focus-within { margin-top: 7px; }\
             #q:focus-visible { color: rgb(1, 2, 3); }\
             </style></head><body><form><input id=\"q\"></form></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const q = document.getElementById("q");
            const form = document.querySelector("form");
            const before = getComputedStyle(q).backgroundColor;
            const formBefore = getComputedStyle(form).marginTop;
            q.focus();
            const focusedBg = getComputedStyle(q).backgroundColor;
            const visibleColor = getComputedStyle(q).color;
            const formWithin = getComputedStyle(form).marginTop;
            q.blur();
            return [before, focusedBg, visibleColor, formBefore, formWithin,
                    getComputedStyle(q).backgroundColor];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    let before = parts[0].as_str().expect("before is a string");
    let focused = parts[1].as_str().expect("focused is a string");
    assert_ne!(
        before, focused,
        ":focus rule must re-style the focused input"
    );
    assert_eq!(
        focused, "rgb(10, 20, 30)",
        "the :focus background lands in computed style"
    );
    assert_eq!(
        parts[2].as_str().expect("color string"),
        "rgb(1, 2, 3)",
        ":focus-visible matches — a text input shows the ring on programmatic focus"
    );
    assert_ne!(
        parts[3].as_str().expect("form before"),
        parts[4].as_str().expect("form within"),
        ":focus-within re-styles the containing form while the input holds focus"
    );
    assert_eq!(
        parts[5].as_str().expect("after blur"),
        before,
        "blur() returns the input to its unfocused style"
    );
}

/// #152's complaint through the whole stack, same shape as the blitz#839
/// focus test above: the hover mirror must reach the Rust tree so
/// :hover rules re-style — on the hit target, on ancestors, and through
/// descendant combinators off an hovered ancestor — and leaving must
/// clear them again.
#[cfg(feature = "screenshot")]
#[test]
fn test_hover_pseudo_styles_react_to_hover_state() {
    let mut rt = setup_runtime(
        "<!DOCTYPE html><html><head><style>\
             #a:hover { background-color: rgb(10, 20, 30); }\
             #outer:hover .menu { display: block; }\
             .menu { display: none; }\
             </style></head><body>\
             <div id=\"outer\"><div id=\"a\"></div><div class=\"menu\">m</div></div>\
             </body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const a = document.getElementById("a");
            const menu = document.querySelector(".menu");
            const before = getComputedStyle(a).backgroundColor;
            const menuHidden = getComputedStyle(menu).display;
            globalThis.__diting_setHoverTree(null, a);
            const hoveredBg = getComputedStyle(a).backgroundColor;
            const menuShown = getComputedStyle(menu).display;
            const matchesA = a.matches(":hover");
            globalThis.__diting_setHoverTree(a, null);
            return [before, hoveredBg, menuHidden, menuShown, matchesA,
                    getComputedStyle(a).backgroundColor, getComputedStyle(menu).display];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    let before = parts[0].as_str().expect("before is a string");
    let hovered = parts[1].as_str().expect("hovered is a string");
    assert_ne!(before, hovered, ":hover rule must re-style the hit target");
    assert_eq!(
        hovered, "rgb(10, 20, 30)",
        "the :hover background lands in computed style"
    );
    assert_eq!(
        parts[2].as_str().expect("menu hidden"),
        "none",
        "menu starts hidden"
    );
    assert_eq!(
        parts[3].as_str().expect("menu shown"),
        "block",
        "an ancestor's :hover flips a descendant-combinator rule"
    );
    assert_eq!(
        parts[4], true,
        "matches(':hover') agrees with computed style (single match path)"
    );
    assert_eq!(
        parts[5].as_str().expect("after leave"),
        before,
        "leaving restores the un-hovered style"
    );
    assert_eq!(
        parts[6].as_str().expect("menu re-hidden"),
        "none",
        "leaving re-hides the menu"
    );
}

/// :active window (#152): the press/release mirrors flip the state
/// around the dispatch window exactly like Chrome's gCS showed —
/// set before pointerdown, cleared before pointerup.
#[cfg(feature = "screenshot")]
#[test]
fn test_active_pseudo_styles_react_to_press_state() {
    let mut rt = setup_runtime(
        "<!DOCTYPE html><html><head><style>\
             #a:active { color: rgb(4, 5, 6); }\
             </style></head><body><div id=\"a\">x</div></body></html>",
    );
    let result = rt
        .evaluate(
            r#"
            const a = document.getElementById("a");
            const before = getComputedStyle(a).color;
            globalThis.__diting_setActiveTree(null, a);
            const activeColor = getComputedStyle(a).color;
            globalThis.__diting_setActiveTree(a, null);
            return [before, activeColor, getComputedStyle(a).color];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_ne!(
        parts[0].as_str().expect("before"),
        parts[1].as_str().expect("active"),
        ":active rule must re-style mid-press"
    );
    assert_eq!(
        parts[1].as_str().expect("active"),
        "rgb(4, 5, 6)",
        "the :active color lands in computed style"
    );
    assert_eq!(
        parts[2].as_str().expect("after release"),
        parts[0].as_str().expect("before"),
        "release restores the un-pressed style"
    );
}

/// #100 (tmall SKU): blur/focus never bubble, so the only way a
/// delegated listener sees them is the capture leg — and React 16 traps
/// both at the root container as CAPTURE listeners. A dispatch model
/// with target+bubble legs only leaves every page's onBlur/onFocus deaf;
/// the SKU drawer's suggest input commits its typed value in onBlur, so
/// the whole publish flow died at "type a color, it never sticks".
#[test]
fn test_dispatch_runs_capture_phase_for_non_bubbling_events() {
    let mut rt = setup_runtime(r#"<html><body><div id="box"><input id="q"></div></body></html>"#);
    let result = rt.evaluate(
            r#"(function(){
                const log = [];
                const box = document.getElementById('box');
                const q = document.getElementById('q');
                document.addEventListener('blur', function(e){ log.push('doc-cap:' + e.target.id); }, true);
                box.addEventListener('blur', function(e){ log.push('box-cap:' + e.currentTarget.id); }, true);
                box.addEventListener('blur', function(){ log.push('box-bubble'); });
                q.addEventListener('blur', function(){ log.push('q-own'); }, true);
                q.dispatchEvent(new FocusEvent('blur'));
                return log.join('|');
            })()"#,
        ).unwrap();
    assert_eq!(
            result.as_str().unwrap_or(""),
            "doc-cap:q|box-cap:box|q-own",
            "capture runs document→container→target for blur; the bubble bucket on the container stays silent (blur does not bubble)"
        );

    // stopPropagation on the way down stops the whole rest of the
    // propagation path (container capture, target) — the target is
    // later on the path than the stopping listener. Fresh runtime: the
    // first leg's listeners would otherwise pollute this one.
    let mut rt = setup_runtime(r#"<html><body><div id="box"><input id="q"></div></body></html>"#);
    let result = rt.evaluate(
            r#"(function(){
                const log = [];
                const box = document.getElementById('box');
                const q = document.getElementById('q');
                document.addEventListener('blur', function(e){ e.stopPropagation(); log.push('stopped'); }, true);
                box.addEventListener('blur', function(){ log.push('box-cap'); }, true);
                q.addEventListener('blur', function(){ log.push('q-own'); });
                q.dispatchEvent(new FocusEvent('blur'));
                return log.join('|');
            })()"#,
        ).unwrap();
    assert_eq!(
        result.as_str().unwrap_or(""),
        "stopped",
        "stopPropagation in the capture leg prevents the target and container legs"
    );

    // The capture flag is part of listener identity for removal: a
    // capture registration is not removed by the bubble-signature call.
    let mut rt = setup_runtime(r#"<html><body><div id="box"><input id="q"></div></body></html>"#);
    let result = rt
        .evaluate(
            r#"(function(){
                const log = [];
                const box = document.getElementById('box');
                const q = document.getElementById('q');
                const cap = function(){ log.push('cap'); };
                box.addEventListener('blur', cap, true);
                box.removeEventListener('blur', cap);
                q.dispatchEvent(new FocusEvent('blur'));
                log.push('/');
                box.removeEventListener('blur', cap, true);
                q.dispatchEvent(new FocusEvent('blur'));
                return log.join('');
            })()"#,
        )
        .unwrap();
    assert_eq!(
            result.as_str().unwrap_or(""),
            "cap/",
            "removeEventListener with the matching capture flag unregisters; the bubble-signature call leaves the capture listener in place"
        );
}

/// #100 end-to-end pin: focus()/blur() fire the real focus family, and
/// React 16's exact registration shape — container capture listeners —
/// observes the non-bubbling halves. Before the capture phase this saw
/// nothing, which is the whole "typed value never commits" failure.
#[test]
fn test_focus_blur_family_reaches_container_capture_listeners() {
    let mut rt = setup_runtime(r#"<html><body><div id="root"><input id="q"></div></body></html>"#);
    let result = rt.evaluate(
            r#"(function(){
                const log = [];
                const root = document.getElementById('root');
                const q = document.getElementById('q');
                ['focus', 'blur'].forEach(function(t){
                    root.addEventListener(t, function(e){ log.push(t + '@' + e.target.id + ':capture'); }, true);
                    root.addEventListener(t, function(){ log.push(t + ':bubble'); });
                });
                ['focusin', 'focusout'].forEach(function(t){
                    root.addEventListener(t, function(){ log.push(t + ':bubble'); });
                });
                q.focus();
                q.blur();
                return log.join('|');
            })()"#,
        ).unwrap();
    assert_eq!(
            result.as_str().unwrap_or(""),
            "focus@q:capture|focusin:bubble|blur@q:capture|focusout:bubble",
            "focus/blur never bubble — only the container's capture leg sees them (React 16 traps them exactly there); the -in/-out halves bubble"
        );
}

#[tokio::test(flavor = "current_thread")]
async fn test_close_event_interface() {
    // doudian's WS wrapper constructs CloseEvent on its reconnect path —
    // the class didn't exist and the ReferenceError killed the socket
    // layer. Chrome shape: code/reason/wasClean with WebIDL defaults, and
    // createEvent('CloseEvent') maps to the real class.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const e1 = new CloseEvent('close', { code: 1005, reason: 'gone', wasClean: true });
            const e2 = new CloseEvent('close');
            return [
                e1 instanceof CloseEvent, e1 instanceof Event,
                e1.code, e1.reason, e1.wasClean,
                e2.code, e2.reason, e2.wasClean,
                document.createEvent('CloseEvent') instanceof CloseEvent,
                Object.prototype.toString.call(e1),
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            true,
            true,
            1005,
            "gone",
            true,
            0,
            "",
            false,
            true,
            "[object CloseEvent]"
        ])
    );
}

/// pointer_touch.js ratchet split (JS-SURFACE-PLAN step 3): PointerEvent,
/// pointer capture, Touch, TouchEvent left bootstrap.js. The capture
/// registry's __diting_* globals must register themselves in the hide list
/// (the list was built before this file loads), or they would show on window.
#[test]
fn pointer_touch_family_capture_and_hidden_registry() {
    let mut rt = setup_runtime("<html><body><div id='h'></div></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () {
            var h = document.getElementById('h');
            var pe = new PointerEvent('pointerdown', {pointerId: 7});
            var te = new TouchEvent('touchstart', {touches: [new Touch({identifier: 3, clientX: 5})]});
            var seen = [];
            h.addEventListener('gotpointercapture', function (e) { seen.push('got' + e.pointerId); });
            h.addEventListener('lostpointercapture', function (e) { seen.push('lost' + e.pointerId); });
            var before = h.hasPointerCapture(7);
            h.setPointerCapture(7);
            var during = h.hasPointerCapture(7);
            var forId = globalThis.__diting_pointerCaptureFor(7) === h;
            globalThis.__diting_pointerCaptureRelease(7);
            var hide = globalThis.__diting_hide_list;
            return JSON.stringify({
                pe: [pe.pointerId, pe.pointerType, pe.isPrimary, pe instanceof MouseEvent],
                te: [te.touches.length, te.touches[0].identifier, te.touches[0].clientX, te instanceof UIEvent],
                before: before, during: during, forId: forId,
                after: h.hasPointerCapture(7), seen: seen,
                nat: Element.prototype.setPointerCapture.toString().indexOf('[native code]') >= 0,
                hidden: ['__diting_ptrCaps','__diting_pointerCaptureFor','__diting_pointerCaptureRelease']
                          .every(function (n) { return hide.indexOf(n) >= 0; }),
            });
        })()
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["pe"], serde_json::json!([7, "", false, true]), "{v}");
    assert_eq!(v["te"], serde_json::json!([1, 3, 5, true]));
    assert_eq!(v["before"], false);
    assert_eq!(v["during"], true);
    assert_eq!(v["forId"], true);
    assert_eq!(v["after"], false);
    assert_eq!(v["seen"], serde_json::json!(["got7", "lost7"]));
    assert_eq!(v["nat"], true);
    assert_eq!(v["hidden"], true);
}

/// M4 knife 3: the EventTarget family's INHERITANCE face. Chrome truth
/// (eventtarget-chain-truth-157.json, 43 keys; 157 re-truth 2026-11-06) pins, for all 155 non-DOM
/// interfaces of the family: the full prototype chain (constructor names per
/// level), that the trio is never an own member of a leaf prototype
/// (ownTrio), live listener/dispatch round-trips on constructible leaves,
/// and singleton instance links (performance/screen/navigator.*/… instanceof
/// their class AND EventTarget). The repair is GENERATED — global_surface.js
/// ET_CHAIN/ET_ALIAS/ET_SINGLETON tables, emitted by gen_globals_surface.py
/// from this same truth — plus three legacy shrinks: the _MseEventTarget
/// intermediate deleted (media_source.js extends EventTarget directly),
/// _MediaQueryList renamed to MediaQueryList (constructor-name face), and
/// VisualViewport's inert trio stubs deleted (listeners now really work).
///
/// SHADOWED_TRIO ratchets the residue: seven pre-M3 shims still own a
/// hand-rolled trio on their prototype — dispatch registries with live
/// delivery paths reading them (WebSocket/EventSource/BroadcastChannel read
/// _lbox; MessagePort owns a message-only queue; IDB maps listeners to
/// on-handlers; MediaQueryList feeds the mq-flush registry). Rewiring each
/// onto the native core is its own family knife; when one lands, remove its
/// name here.
#[test]
fn eventtarget_chains_match_chrome_truth_except_known_gaps() {
    // 刀56 emptied the ratchet: the seven socket-like shims dropped their
    // hand-rolled trio shadows and inherit the native EventTarget core, so
    // NO interface owns addEventListener/removeEventListener/dispatchEvent
    // outside Chrome's own list. The mechanism stays for any future shim.
    const SHADOWED_TRIO: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/eventtarget-chain-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/eventtarget-chain-truth-157.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        match k.as_str() {
            // every interface's chain: level names exactly, and no
            // engine-only chains Chrome's family doesn't have.
            "chains" => {
                for (n, wchain) in want.as_object().unwrap() {
                    let gchain = &got[n];
                    if gchain != wchain {
                        bad.push(format!("chains.{n}: engine {gchain} chrome {wchain}"));
                    }
                }
                for n in got.as_object().unwrap().keys() {
                    if want.get(n).is_none() {
                        bad.push(format!(
                            "chains.{n}: engine-only chain (not in Chrome truth)"
                        ));
                    }
                }
                continue;
            }
            // the trio must be inherited at every leaf; the hand-rolled
            // dispatch shims are the ratcheted exception.
            "ownTrio" => {
                let g: Vec<&str> = got
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                let w: Vec<&str> = want
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                let mut exp: Vec<&str> = w.clone();
                exp.extend_from_slice(SHADOWED_TRIO);
                exp.sort();
                let mut gs = g.clone();
                gs.sort();
                if gs != exp {
                    for n in &gs {
                        if !exp.contains(n) {
                            bad.push(format!("ownTrio: {n} owns the trio unexpectedly"));
                        }
                    }
                    for n in &exp {
                        if !gs.contains(n) {
                            bad.push(format!(
                                "ownTrio: {n} no longer shadows - remove it from SHADOWED_TRIO"
                            ));
                        }
                    }
                }
                continue;
            }
            _ => {}
        }
        if got != want {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 8: the UI event family (uievent-probe.js, 112 keys, real Chrome
/// 145): UIEvent / MouseEvent / WheelEvent / KeyboardEvent / InputEvent /
/// FocusEvent / CompositionEvent / PointerEvent. The bootstrap classes kept a
/// hand-picked slice of the init dictionary (92 of 111 behavior keys off); the
/// platform module `uievent` now owns the eight interfaces: own-member order,
/// arity, constants and prototype chain (`shape`), constructor errors
/// (argument count, `new`, non-object init), WebIDL dictionary conversions per
/// member type (long/ulong/short/ushort, truncating double, float32, DOMString,
/// Window/EventTarget/InputDeviceCapabilities unions) with Chrome's exact
/// "Failed to read the 'x' property from 'YInit'" wording, derived members
/// (`which`, layerX/offsetX/pageX, wheelDelta*, altitudeAngle/azimuthAngle <->
/// tiltX/tiltY bit-identical to Chrome's libm), getModifierState, init*
/// legacy initializers, and event inheritance. The offsetX/Y write of the
/// dispatch chokepoint goes through the hidden __diting_uiOffset hook.
#[tokio::test(flavor = "current_thread")]
async fn uievent_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/uievent-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/uievent-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if k == "shape" {
            for (cls, tshape) in want.as_object().unwrap() {
                for (facet, tv) in tshape.as_object().unwrap() {
                    if &engine["shape"][cls][facet] != tv {
                        bad.push(format!(
                            "shape {cls}.{facet}:\n   engine {}\n   chrome {tv}",
                            engine["shape"][cls][facet]
                        ));
                    }
                }
            }
        } else if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 37: the main window's prototype chain now contains Window.prototype and EventTarget.prototype
/// (`window instanceof EventTarget` was false; Chrome: true), while window event listeners still work through the
/// named-access holder that sits ahead of them.
#[tokio::test(flavor = "current_thread")]
async fn window_chain_reaches_event_target() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate_remote(
            "(async () => { let n = 0; const f = () => { n++; }; window.addEventListener('zz', f); window.dispatchEvent(new Event('zz')); \
             window.removeEventListener('zz', f); window.dispatchEvent(new Event('zz')); \
             return [window instanceof EventTarget, window instanceof Window, Object.prototype.toString.call(window), n, typeof window.addEventListener]; })()",
            true,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        v.value.unwrap(),
        serde_json::json!([true, true, "[object Window]", 1, "function"])
    );
}

#[test]
fn test_load_event_path_stops_at_document() {
    // DOM §4.5: a Document's get-the-parent is null for `load` — an element
    // or document load never reaches window listeners. github.com/settings
    // (2026-11-06) ran web-vitals' whenReady, a window capture 'load'
    // listener that re-registers a fresh one while readyState isn't
    // complete; with the document → window hop every <link> load doubled
    // the listeners — exponential sync work that pinned the pump ~100s.
    let mut rt = setup_runtime("<html><head></head><body></body></html>");
    let v = rt
        .evaluate(
            r#"(() => {
            let calls = 0;
            const R = () => { calls++; addEventListener('load', () => R(), true); };
            addEventListener('load', () => R(), true);
            let winHandler = 0;
            window.onload = () => { winHandler++; };
            const l = document.createElement('link');
            document.head.appendChild(l);
            let path = null;
            l.addEventListener('load', e => { path = e.composedPath().length; });
            for (let i = 0; i < 16; i++) l.dispatchEvent(new Event('load'));
            document.dispatchEvent(new Event('load'));
            let ping = 0;
            addEventListener('ping', () => { ping++; });
            l.dispatchEvent(new Event('ping', { bubbles: true }));
            return JSON.stringify({ calls, winHandler, path, ping });
        })()"#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        v["calls"], 0,
        "element/document load must not reach window capture listeners"
    );
    assert_eq!(v["winHandler"], 0, "nor window.onload");
    assert_eq!(
        v["path"], 4,
        "composedPath = [link, head, html, document], no window"
    );
    assert_eq!(v["ping"], 1, "non-load events still propagate to window");
}
