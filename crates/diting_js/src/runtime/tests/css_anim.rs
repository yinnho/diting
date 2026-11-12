//! CSS 动画/过渡 + Web Animations + 动画时钟
use super::super::*;
use super::common::setup_runtime;

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "screenshot")]
async fn test_css_time_clock_and_animation_extent_reach_native_state() {
    // #417: a declarative CSS animation (SVG keyframes) has no
    // __timelines entry — the video pump drives a virtual clock through
    // set_css_time and reads the timeline length from css_extent, both
    // folded by the layout run.
    let mut rt = setup_runtime(
        r#"<html><head><style>
                @keyframes k { from { opacity: 0 } to { opacity: 1 } }
                @keyframes slow { from { opacity: 0 } to { opacity: 1 } }
                .a { animation: k 1s .5s forwards }
                .b { animation: slow 2s forwards }
            </style></head><body><div class="a">x</div><div class="b">y</div></body></html>"#,
    );
    // Any layout-driven read runs the layout pass that folds the extent.
    let script = r#"() => document.querySelector('.a').getBoundingClientRect().height"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert!(result.value.is_some(), "layout read succeeds");
    let extent = rt.with_state(|st| st.css_extent.get());
    assert!(
        (extent - 2.0).abs() < 1e-6,
        "max(delay+duration) = 2.0, got {extent}"
    );
    assert_eq!(
        rt.with_state(|st| st.css_time),
        None,
        "clock starts unset (static)"
    );

    // The pump drives the clock through the same op the Page method uses.
    async fn set_time(rt: &mut JsRuntime, t: &str) {
        let script = format!(r#"() => __diting_domRaw('set_css_time', '{t}')"#);
        rt.call_function_on_remote(&script, None, &[], true, true)
            .await
            .unwrap();
    }
    set_time(&mut rt, "0.75").await;
    assert_eq!(rt.with_state(|st| st.css_time), Some(0.75));
    // Non-finite and negative inputs are rejected; the last valid time
    // holds.
    set_time(&mut rt, "-1").await;
    set_time(&mut rt, "NaN").await;
    set_time(&mut rt, "abc").await;
    assert_eq!(rt.with_state(|st| st.css_time), Some(0.75));

    // The sampler answers to the clock end-to-end: getComputedStyle
    // opacity mid-animation reflects the eased stop.
    let script = r#"() => {
            __diting_domRaw('set_css_time', '1.5');
            return getComputedStyle(document.querySelector('.b')).opacity;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    // .b: 2s linear... no easing token → default ease; at t=1.5/2 = .75
    // progress, eased well past 0.75 and below 1.
    let opacity = result
        .value
        .unwrap()
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!(
        opacity > 0.5 && opacity < 1.0,
        "mid-flight eased opacity: {opacity}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_start_end_events() {
    // blitz#863 family 1: diting samples CSS animations at a fixed clock (no
    // compositor loop), so `await animationend` scripts used to hang forever.
    // The collapsed lifecycle fires animationstart synchronously when the
    // animation becomes observable and animationend after the declared
    // active duration, with elapsedTime = that duration.
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes fade{from{opacity:1}to{opacity:0}} .box{animation:fade 100ms linear;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        const seq = [];
        // In an isolated test process the first layout run (font book init)
        // blocks the loop for ~0.9s; it lands inside the check drain here, so
        // the end timer is armed only after that block and a wall-clock
        // await armed earlier expires first — await the event, not a window.
        const ended = new Promise(r =>
            document.body.addEventListener('animationend', r));
        document.body.addEventListener('animationstart', (e) =>
            seq.push(['start', e.animationName, e.elapsedTime, e instanceof AnimationEvent, e.target.id]));
        document.body.addEventListener('animationend', (e) =>
            seq.push(['end', e.animationName, e.elapsedTime]));
        const el = document.createElement('div');
        el.id = 'box';
        el.setAttribute('class', 'box');
        document.body.appendChild(el);
        await Promise.race([ended, new Promise(r => setTimeout(r, 3000))]);
        return seq;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([["start", "fade", 0, true, "box"], ["end", "fade", 0.1]])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_flood_epoch_cap_defers_not_starves() {
    // #224: a MutationObserver-flooded page rebuilds subtrees every batch and
    // the animation sweep pays a full-cascade computed-style read per
    // candidate — capped at 512 per mutation epoch, so a flood defers NEW
    // animations to the next epoch. Deferral must never become starvation:
    // once the flood stops (or bumps the epoch), deferred starts land.
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes fade{from{opacity:1}to{opacity:0}} .box{animation:fade 60ms linear;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        let starts = 0, ends = 0;
        document.body.addEventListener('animationstart', () => starts++);
        document.body.addEventListener('animationend', () => ends++);
        // 600 synchronous inserts > the 512/epoch cap: the tail defers.
        for (let i = 0; i < 600; i++) {
            const el = document.createElement('div');
            el.setAttribute('class', 'box');
            document.body.appendChild(el);
        }
        // 600 synchronous inserts > the 512/epoch cap: the tail defers. The
        // sweep runs in bounded slices, so landing everything is a liveness
        // property, not a speed one — poll for it (bumping the epoch each
        // round the way a flooding page does; the overflow macrotask also
        // picks up the capped tail without the bump) instead of betting on a
        // fixed wall-clock window that a slow CI runner blows through.
        for (let round = 0; starts < 600 && round < 60; round++) {
            await new Promise(r => setTimeout(r, 50));
            document.body.setAttribute('data-bump', String(round));
        }
        await new Promise(r => setTimeout(r, 50));
        return { starts, ends };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["starts"],
        serde_json::json!(600),
        "all starts landed: {v}"
    );
    assert!(
        v["ends"].as_u64().unwrap() > 0,
        "ends flowing after cap: {v}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_sweep_survives_behead_inside_check() {
    // #224 second hole: a wall inside _animationCheckIn's computed-style read
    // (on a loaded runner, the first layout's font-book init) gets beheaded by
    // the eval patience watchdog mid-check. The terminate skipped the reset of
    // the reentrancy depth, and a depth stuck at 1 turned every later check
    // into a silent defer — the flood test froze at starts=127/272/405 of
    // 600. The wall here is a one-shot page wrapper around getComputedStyle
    // (the sweep resolves the global per call); the insert waits a tick so it
    // runs inside the pump, under patience.
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes fade{from{opacity:1}to{opacity:0}} .box{animation:fade 60ms linear;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        let starts = 0, walled = false;
        document.body.addEventListener('animationstart', () => starts++);
        const realGcs = globalThis.getComputedStyle;
        globalThis.getComputedStyle = function (el, p) {
            if (!walled && el.className === 'box') { walled = true; const t = Date.now(); while (Date.now() - t < 800) {} }
            return realGcs.call(this, el, p);
        };
        await new Promise(r => setTimeout(r, 10));
        for (let i = 0; i < 30; i++) {
            const el = document.createElement('div');
            el.setAttribute('class', 'box');
            document.body.appendChild(el);
        }
        for (let round = 0; starts < 30 && round < 60; round++) {
            await new Promise(r => setTimeout(r, 50));
        }
        return { starts, walled };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["starts"],
        serde_json::json!(30),
        "starts after behead: {v}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_second_look_does_not_restart_ended() {
    // #224: one insert is looked at twice — the body scan (an animation*
    // listener landed) walks the new boxes, then each box comes up again as
    // its own inserted root. Slow reads stretch that gap past a short
    // animation, its end timer untracks it, and the second look used to start
    // it again (flood test: 711 starts of 600 on a slowed runner).
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes fade{from{opacity:1}to{opacity:0}} .box{animation:fade 10ms linear;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        let starts = 0, ends = 0;
        const realGcs = globalThis.getComputedStyle;
        globalThis.getComputedStyle = function (el, p) {
            if (el.className === 'box') { const t = Date.now(); while (Date.now() - t < 25) {} }
            return realGcs.call(this, el, p);
        };
        document.body.addEventListener('animationstart', () => starts++);
        document.body.addEventListener('animationend', () => ends++);
        for (let i = 0; i < 5; i++) {
            const el = document.createElement('div');
            el.setAttribute('class', 'box');
            document.body.appendChild(el);
        }
        for (let round = 0; ends < 5 && round < 40; round++) {
            await new Promise(r => setTimeout(r, 25));
        }
        await new Promise(r => setTimeout(r, 200));
        return { starts, ends };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({ "starts": 5, "ends": 5 })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_reinsert_and_rename_still_restart() {
    // The second-look guard must not swallow Chrome's real restarts: a
    // re-insertion and a new animation-name both start the animation again.
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes fade{from{opacity:1}to{opacity:0}} @keyframes spin{from{opacity:0}to{opacity:1}} .box{animation:fade 10ms linear;} .box2{animation:spin 10ms linear;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        const seq = [];
        document.body.addEventListener('animationstart', (e) => seq.push(e.animationName));
        const wait = (ms) => new Promise(r => setTimeout(r, ms));
        const el = document.createElement('div');
        el.setAttribute('class', 'box');
        document.body.appendChild(el);
        await wait(60);
        el.setAttribute('data-x', '1');
        el.style.color = 'red';
        await wait(60);
        el.remove();
        document.body.appendChild(el);
        await wait(60);
        el.setAttribute('class', 'box2');
        await wait(60);
        el.setAttribute('class', 'box');
        await wait(60);
        return seq;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["fade", "fade", "spin", "fade"])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_cancel_on_class_removal() {
    // Removing the animation between start and end must fire animationcancel
    // and suppress animationend (the pending end timer is cleared).
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes fade{from{opacity:1}to{opacity:0}} .box{animation:fade 100ms;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        const seq = [];
        const el = document.createElement('div');
        el.addEventListener('animationstart', () => seq.push('start'));
        el.addEventListener('animationcancel', (e) => seq.push('cancel:' + e.animationName));
        el.addEventListener('animationend', () => seq.push('end'));
        document.body.appendChild(el);
        el.setAttribute('class', 'box');
        await new Promise(r => setTimeout(r, 10));
        el.removeAttribute('class');
        await new Promise(r => setTimeout(r, 30));
        return seq;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["start", "cancel:fade"])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_transition_computed_values() {
    // The computed table spells transition-* in seconds with `all` as the
    // property default and `ease` as the timing default (Chrome spellings).
    let mut rt = setup_runtime(
        "<html><head><style>#b{transition:opacity 1s ease 250ms}\n#c{transition-property:color;transition-duration:2s}</style></head><body><div id=\"b\"></div><div id=\"c\"></div><div id=\"p\"></div></body></html>",
    );
    let script = r#"() => {
        const cs = (id) => {
            const s = getComputedStyle(document.getElementById(id));
            return [s.transitionProperty, s.transitionDuration, s.transitionDelay, s.transitionTimingFunction];
        };
        return [cs('b'), cs('c'), cs('p')];
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            ["opacity", "1s", "0.25s", "ease"],
            ["color", "2s", "0s", "ease"],
            ["all", "0s", "0s", "ease"],
        ])
    );
}

#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_css_transition_events_and_registry() {
    // The JS face diffs watched properties across style writes: the first
    // resolution only observes (no transition on a first style resolution),
    // the second registers an entry (start=0 on the video timeline) and
    // fires run → start → end with the declared curve length.
    let mut rt = setup_runtime("<html><body><div id=\"d\">x</div></body></html>");
    let script = r#"async () => {
        const el = document.getElementById('d');
        const seq = [];
        el.addEventListener('transitionrun', (e) => seq.push(['run', e.propertyName, e.elapsedTime, e instanceof TransitionEvent]));
        el.addEventListener('transitionstart', (e) => seq.push(['start', e.propertyName]));
        el.addEventListener('transitionend', (e) => seq.push(['end', e.propertyName, e.elapsedTime]));
        el.style.transition = 'opacity 80ms linear';
        el.style.opacity = '1';
        await new Promise(r => setTimeout(r, 5));
        el.style.opacity = '0';
        await new Promise(r => setTimeout(r, 200));
        return seq;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            ["run", "opacity", 0, true],
            ["start", "opacity"],
            ["end", "opacity", 0.08],
        ])
    );
    let list = rt.with_state(|st| st.css_transitions.borrow().clone());
    assert_eq!(list.len(), 1, "one registered transition");
    let tr = &list[0];
    assert_eq!(tr.property, "opacity");
    assert_eq!(tr.from, diting_css::TransitionValue::Opacity(1.0));
    assert_eq!(tr.to, diting_css::TransitionValue::Opacity(0.0));
    assert_eq!(tr.start, 0.0, "video-timeline outset");
    assert!((tr.duration - 0.08).abs() < 1e-5);
    assert!(matches!(tr.easing, diting_css::Easing::Linear));
}

#[cfg(feature = "screenshot")]
#[tokio::test(flavor = "current_thread")]
async fn test_css_transition_cancel_on_retrigger_and_detach() {
    // Chrome fires transitioncancel two ways: a re-trigger on the same
    // property preempts the in-flight entry (elapsedTime = active time so
    // far), and removing the element from the document cancels every
    // pending timer. Both legs here, transform included in the watched set.
    let mut rt =
        setup_runtime("<html><body><div id=\"d\">x</div><div id=\"e\">y</div></body></html>");
    let script = r#"async () => {
        const d = document.getElementById('d');
        const e = document.getElementById('e');
        const seq = [];
        d.addEventListener('transitioncancel', (ev) => seq.push(['cancel', ev.propertyName, Math.round(ev.elapsedTime * 1000)]));
        d.addEventListener('transitionend', () => seq.push(['end']));
        // Check 1 observes (no transition on a first style resolution),
        // check 2 arms the run, check 3 re-triggers mid-flight — that is
        // the write that must preempt check 2's entry with a cancel.
        d.style.transition = 'opacity 500ms linear';
        d.style.opacity = '0.5';
        await new Promise(r => setTimeout(r, 60));
        d.style.opacity = '0';
        await new Promise(r => setTimeout(r, 60));
        d.style.opacity = '1';
        await new Promise(r => setTimeout(r, 40));
        const eSeq = [];
        e.addEventListener('transitionrun', (ev) => eSeq.push(['run', ev.propertyName]));
        e.addEventListener('transitioncancel', (ev) => eSeq.push(['cancel', ev.propertyName]));
        e.style.transition = 'transform 500ms linear';
        e.style.transform = 'translate(40px, 0px)';
        await new Promise(r => setTimeout(r, 40));
        e.style.transform = 'translate(0px, 0px)';
        await new Promise(r => setTimeout(r, 30));
        e.remove();
        return [seq, eSeq];
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let (seq, e_seq) = match result.value.unwrap() {
        serde_json::Value::Array(a) => (a[0].clone(), a[1].clone()),
        _ => panic!("expected [seq, eSeq]"),
    };
    assert_eq!(
        seq.as_array().unwrap().len(),
        1,
        "one cancel, old end timer cleared"
    );
    let entry = seq.as_array().unwrap().last().unwrap();
    assert_eq!(entry[0], "cancel");
    assert_eq!(entry[1], "opacity");
    let elapsed_ms = entry[2].as_i64().unwrap();
    assert!(
        elapsed_ms > 20 && elapsed_ms < 300,
        "cancel elapsed tracks active time, got {elapsed_ms}ms"
    );
    assert_eq!(
        e_seq,
        serde_json::json!([["run", "transform"], ["cancel", "transform"]])
    );
    let list = rt.with_state(|st| st.css_transitions.borrow().clone());
    assert_eq!(
        list.len(),
        2,
        "d's re-trigger replaced its entry; e's stale entry stays but the sampler skips detached nids"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_element_animate_lifecycle_events() {
    // WAAPI Element.animate fires the same lifecycle events with an empty
    // animationName (Chrome parity); detached elements never start.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const seq = [];
        const el = document.createElement('div');
        document.body.appendChild(el);
        el.addEventListener('animationstart', (e) => seq.push(['start', e.animationName]));
        el.addEventListener('animationend', (e) => seq.push(['end', e.animationName, e.elapsedTime]));
        const anim = el.animate([{opacity: 1}, {opacity: 0}], {duration: 80});
        const detached = document.createElement('div');
        detached.animate([{opacity: 1}, {opacity: 0}], {duration: 10});
        await new Promise(r => setTimeout(r, 150));
        return [seq, anim.playState];
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([[["start", ""], ["end", "", 0.08]], "finished"])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_static_markup_animation_listener_scan() {
    // Animations already present in initial markup have no mutation hook to
    // ride on; registering an animation* listener is the signal that the page
    // cares and triggers a capped subtree scan, so the lifecycle still fires.
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes pulse{from{opacity:0}to{opacity:1}} .p{animation:pulse 50ms;}</style></head><body><div id=\"t\" class=\"p\">x</div></body></html>",
    );
    let script = r#"async () => {
        const seq = [];
        // Await the end event itself: in an isolated test process the first
        // layout run (font book init) blocks the loop for ~0.9s, so any
        // wall-clock await armed before the listener-scan microtask would
        // expire before the end timer — a test artifact, not engine behavior.
        const ended = new Promise(r =>
            document.getElementById('t').addEventListener('animationend', r));
        document.getElementById('t').addEventListener('animationstart', (e) =>
            seq.push(['start', e.animationName]));
        document.getElementById('t').addEventListener('animationend', (e) =>
            seq.push(['end', e.animationName]));
        await ended;
        return seq;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([["start", "pulse"], ["end", "pulse"]])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_css_animation_iteration_event() {
    // blitz#863 family 1: the interior iteration boundary of a 2-iteration
    // animation fires animationiteration (elapsedTime = iteration start =
    // delay + duration×k) between start and end. Values chosen so every
    // elapsedTime is an exact binary float (0.05×2 is 0.1 bit-for-bit).
    let mut rt = setup_runtime(
        "<html><head><style>@keyframes pulse{from{opacity:0}to{opacity:1}} .p{animation:pulse 50ms linear 2;}</style></head><body></body></html>",
    );
    let script = r#"async () => {
        const seq = [];
        const ended = new Promise(r =>
            document.body.addEventListener('animationend', r));
        document.body.addEventListener('animationstart', (e) =>
            seq.push(['start', e.animationName, e.elapsedTime]));
        document.body.addEventListener('animationiteration', (e) =>
            seq.push(['iteration', e.animationName, e.elapsedTime, e instanceof AnimationEvent]));
        document.body.addEventListener('animationend', (e) =>
            seq.push(['end', e.animationName, e.elapsedTime]));
        const el = document.createElement('div');
        el.setAttribute('class', 'p');
        document.body.appendChild(el);
        await Promise.race([ended, new Promise(r => setTimeout(r, 3000))]);
        return seq;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            ["start", "pulse", 0],
            ["iteration", "pulse", 0.05, true],
            ["end", "pulse", 0.1]
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_transition_event_interface() {
    // blitz#863 family 2: TransitionEvent carries propertyName / elapsedTime /
    // pseudoElement with the WebIDL defaults and the 1-argument requirement —
    // it used to be an empty Event subclass, so every read was undefined.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
        const e1 = new TransitionEvent('transitionend', {
            propertyName: 'opacity', elapsedTime: 1.5, pseudoElement: '::before' });
        const e2 = new TransitionEvent('transitionrun');
        let threw = '';
        try { new TransitionEvent(); } catch (err) { threw = err.constructor.name; }
        return [
            e1 instanceof TransitionEvent, e1 instanceof Event,
            e1.propertyName, e1.elapsedTime, e1.pseudoElement,
            e2.propertyName, e2.elapsedTime, e2.pseudoElement, threw,
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
            "opacity",
            1.5,
            "::before",
            "",
            0,
            "",
            "TypeError"
        ])
    );
}

/// M4 knife 17: Web Animations (anim-probe.js, 89 keys, real Chrome 145): shape of
/// Animation / AnimationEffect / KeyframeEffect / AnimationTimeline /
/// DocumentTimeline / AnimationPlaybackEvent, EffectTiming validation wording,
/// keyframe normalization (array / property-indexed, computed offsets, offset and
/// easing and composite checks), the play / pause / finish / cancel / reverse /
/// updatePlaybackRate state machine with the pending flag, ready/finished promise
/// identity, finish/cancel/remove events, replace state, getAnimations,
/// commitStyles. Element.animate returns a real Animation.
#[tokio::test(flavor = "current_thread")]
async fn animation_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/anim-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/anim-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if &engine[k] != want {
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
