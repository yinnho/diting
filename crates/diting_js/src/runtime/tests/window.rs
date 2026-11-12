//! window 对象面：descriptor/命名访问/navigator/matchMedia/UA
use super::super::*;
use super::common::setup_runtime;

/// obscura#737 lineage probe: matchMedia answered `(min-width:640px)`
/// with false while the persona published a 2560px innerWidth — a page's
/// JS branching disagreed with the @media rules the CSS cascade applied,
/// and the self-contradiction was itself a fingerprint tell (scripts
/// cross-check the two). The evaluator now reads the live window
/// viewport, so the two cannot drift.
#[test]
fn match_media_agrees_with_published_viewport() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let checks = rt
        .evaluate(
            r#"
            const vw = innerWidth, vh = innerHeight;
            return [
                matchMedia("").matches,
                matchMedia("(min-width: 640px)").matches === (vw >= 640),
                matchMedia("(min-width: 99999px)").matches,
                matchMedia("(max-width: 10px)").matches === (vw <= 10),
                matchMedia("(min-width: " + vw + "px)").matches,
                matchMedia("(max-width: " + (vw - 0.01) + "px)").matches,
                matchMedia("screen and (min-width: 100px)").matches,
                matchMedia("print").matches,
                matchMedia("not print").matches,
                matchMedia("(min-width: 99999px), (min-width: 100px)").matches,
                matchMedia("(orientation: landscape)").matches === (vw >= vh),
                matchMedia("(prefers-color-scheme: light)").matches,
                matchMedia("(prefers-color-scheme: dark)").matches,
                matchMedia("(unknown-feature: 3)").matches,
            ];
        "#,
        )
        .unwrap();
    let parts = checks.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(true),
        "empty query matches everything"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(true),
        "min-width tracks the real viewport"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(false),
        "impossible min-width is false"
    );
    assert_eq!(
        parts[3],
        serde_json::json!(true),
        "max-width tracks the real viewport"
    );
    assert_eq!(
        parts[4],
        serde_json::json!(true),
        "boundary width is inclusive"
    );
    assert_eq!(
        parts[5],
        serde_json::json!(false),
        "just-below viewport is false"
    );
    assert_eq!(
        parts[6],
        serde_json::json!(true),
        "screen and (...) matches"
    );
    assert_eq!(parts[7], serde_json::json!(false), "print does not match");
    assert_eq!(parts[8], serde_json::json!(true), "not print matches");
    assert_eq!(parts[9], serde_json::json!(true), "comma list is OR");
    assert_eq!(
        parts[10],
        serde_json::json!(true),
        "orientation follows the viewport"
    );
    assert_eq!(
        parts[11],
        serde_json::json!(true),
        "persona is light color scheme"
    );
    assert_eq!(
        parts[12],
        serde_json::json!(false),
        "dark scheme does not match"
    );
    assert_eq!(
        parts[13],
        serde_json::json!(false),
        "unknown features are false"
    );
}

/// Issue #25 / obscura#1007: MediaQueryList objects must be live —
/// `matches` re-evaluates against the current viewport (not frozen at
/// creation), listeners actually register, and a viewport change fires
/// `change` (carrying the new matches/media) on every subscribed
/// object that crossed. The old face returned a disposable literal
/// whose addListener was a no-op, so responsive pages never saw a
/// breakpoint flip after session_viewport.
#[test]
fn matchmedia_live_objects_fire_change_on_viewport_flip() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
            var hits = [];
            var wide = matchMedia('(min-width: 800px)');
            wide.addEventListener('change', function (e) { hits.push('ac:' + e.matches + ':' + e.media); });
            var narrow = matchMedia('(max-width: 500px)');
            narrow.onchange = function (e) { hits.push('oc:' + e.matches); };
            var inert = matchMedia('(min-width: 800px)'); // never subscribed
            var before = wide.matches;
            __diting_setViewport(400, 800, false, undefined);
            var out = [before, wide.matches, narrow.matches, inert.matches,
                       hits.join(';')].join('|');
            __diting_setViewport(1920, 1000, false, undefined);
            return out;
        "#).unwrap();
    // Default viewport is wide: min-width 800 starts true; after the
    // 400px viewport it flips (firing change with the query string),
    // max-width 500 crosses to true (onchange fires), and the
    // unsubscribed object still reports live matches without events.
    assert_eq!(
        out.as_str().unwrap(),
        "true|false|true|false|ac:false:(min-width: 800px);oc:true"
    );
}

/// Same object surface, override half: the mobile viewport emulation
/// flips pointer/hover answers, and the legacy addListener API must
/// observe the crossing too.
#[test]
fn matchmedia_override_flip_reaches_legacy_listeners() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
            var hits = [];
            var coarse = matchMedia('(pointer: coarse)');
            coarse.addListener(function (e) { hits.push('legacy:' + e.matches); });
            __diting_setViewport(400, 800, true, undefined);
            var out = coarse.matches + '|' + hits.join(';');
            __diting_setViewport(1920, 1000, false, undefined);
            return out;
        "#,
        )
        .unwrap();
    assert_eq!(out.as_str().unwrap(), "true|legacy:true");
}

/// Issue #29: `Emulation.setEmulatedMedia` (Playwright's
/// page.emulateMedia) must flip all three faces together — the
/// matchMedia script face, its change events, AND the @media cascade
/// that getComputedStyle resolves through. The pre-emulation gCS read
/// matters: it pins the cascade snapshot at the old epoch, and the
/// regression was exactly that ordering serving the stale table after
/// the flip.
#[cfg(feature = "screenshot")]
#[test]
fn media_emulation_flips_matchmedia_and_cascade() {
    let mut rt = setup_runtime(
        "<html><head><style>\
             @media (prefers-color-scheme: dark) { #t { color: rgb(0, 0, 255); } }\
             @media (prefers-reduced-motion: reduce) { #m { display: none; } }\
             </style></head><body><div id='t'>x</div><div id='m'>y</div></body></html>",
    );
    assert_eq!(
        rt.evaluate("getComputedStyle(document.getElementById('t')).color")
            .unwrap(),
        serde_json::json!("rgb(0, 0, 0)"),
        "persona default is light — dark arm does not apply before emulation"
    );
    let out = rt.evaluate(r#"
            var hits = [];
            var q = matchMedia('(prefers-color-scheme: dark)');
            q.addEventListener('change', function (e) { hits.push(e.matches); });
            __diting_setMediaFeatures('[["prefers-color-scheme","dark"],["prefers-reduced-motion","reduce"]]', 'screen');
            return [
                matchMedia('(prefers-color-scheme: dark)').matches,
                matchMedia('(prefers-reduced-motion: reduce)').matches,
                hits.join(';'),
                getComputedStyle(document.getElementById('t')).color,
                getComputedStyle(document.getElementById('m')).display,
            ].join('|');
        "#).unwrap();
    assert_eq!(
        out.as_str().unwrap(),
        "true|true|true|rgb(0, 0, 255)|none",
        "script face, change event, and cascade must flip together"
    );
    // Chrome semantics on the follow-ups: `features: []` REPLACES the
    // emulated set (dark arm drops, color falls back) while `media:
    // 'print'` flips the type; then `media: ''` clears back to screen.
    let out2 = rt
        .evaluate(
            r#"
            __diting_setMediaFeatures('[]', 'print');
            var a = [matchMedia('print').matches, matchMedia('screen').matches,
                     getComputedStyle(document.getElementById('t')).color].join('|');
            __diting_setMediaFeatures('[]', '');
            return a + '#' + matchMedia('print').matches;
        "#,
        )
        .unwrap();
    assert_eq!(
        out2.as_str().unwrap(),
        "true|false|rgb(0, 0, 0)#false",
        "features replace, media replaces, empty media clears"
    );
}

/// (300015 P2) Window own-prop face parity with Chrome 155 (https page
/// baseline): the relocated trio lives on the proto layer, worker
/// plumbing and legacy globals are gone, and the generated surface
/// (global_surface.js) lands interface/method/object faces with
/// Chrome's own descriptor shapes. Truth captured via CDP —
/// tools/webplatform/chrome/fp-winkeys-diff.json, fp-chrome-shapes.json.
#[test]
fn window_own_prop_face_p2() {
    let mut rt = setup_runtime("<html><body>x</body></html>");
    let v = rt.evaluate(r#"
            var own = Object.getOwnPropertyNames(window);
            var has = function(n){ return own.indexOf(n) >= 0; };
            var r = {};
            ['addEventListener','removeEventListener','dispatchEvent','constructor'].forEach(function(n){
                r['reloc_'+n] = {off: !has(n), resolves: typeof window[n]};
            });
            ['bootWorker','runWorkerMessage','workerImportUrls','preloadWorkerImports',
             'resolveWorkerImportUrl','CSS2Properties','WebKitMediaSource','SVGDiscardElement',
             'scheduling','webkitAudioContext','webkitOfflineAudioContext'].forEach(function(n){
                r['gone_'+n] = !has(n) && !(n in window);
            });
            ['oncopy','oncut','onpaste','ondragexit','ontouchstart'].forEach(function(n){
                r['gone_'+n] = !has(n);
            });
            // Interfaces the pinned target (chrome-target.toml) exposes; the global_surface tables
            // come from an older capture and version_prune.js reconciles the window face to the
            // claimed version (2026-11-06: 157 HAS all three - the 145-era prune list flipped).
            r.throw_iface = typeof window.AbortController;
            r.throw_msg = (function(){ try { new IDBCursor(); return 'no-throw'; } catch(e){ return e.message; } })();
            r.throw_enum = Object.getOwnPropertyDescriptor(window,'IDBCursor').enumerable;
            r.ok_iface = typeof window.XMLSerializer;
            r.ok_new = window.XMLSerializer && (new XMLSerializer()) instanceof XMLSerializer;
            r.face157 = [typeof window.PermissionsPolicy, typeof window.XSLTProcessor, typeof window.Sanitizer];
            r.pruned = [typeof window.SharedStorage, typeof window.sharedStorage];
            r.method = typeof window.moveTo;
            r.method_enum = Object.getOwnPropertyDescriptor(window,'moveTo').enumerable;
            r.method_ctor = (function(){ try { new moveTo(); return 'no-throw'; } catch(e){ return e.message; } })();
            r.obj_tag = Object.prototype.toString.call(window.locationbar);
            r.obj_enum = Object.getOwnPropertyDescriptor(window,'locationbar').enumerable;
            r.client_info = window.clientInformation === navigator;
            r.credless = window.credentialless;
            r.onsearch_acc = typeof Object.getOwnPropertyDescriptor(window,'onsearch').get;
            r.onsearch_val = window.onsearch;
            JSON.stringify(r)
        "#).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(v.as_str().expect("string from JSON.stringify")).unwrap();
    let obj = v.as_object().unwrap();
    for n in [
        "addEventListener",
        "removeEventListener",
        "dispatchEvent",
        "constructor",
    ] {
        let e = &obj[&format!("reloc_{}", n)];
        assert_eq!(e["off"], true, "{} must not be a window own prop", n);
        assert_eq!(e["resolves"], "function", "{} must still resolve", n);
    }
    for k in obj.keys().filter(|k| k.starts_with("gone_")) {
        assert_eq!(obj[k], true, "{} leaked onto the window face", k);
    }
    assert_eq!(obj["throw_iface"], "function");
    assert_eq!(
        obj["throw_msg"],
        "Failed to construct 'IDBCursor': Illegal constructor"
    );
    assert_eq!(obj["throw_enum"], false);
    assert_eq!(obj["ok_iface"], "function");
    assert_eq!(obj["ok_new"], true);
    assert_eq!(
        obj["face157"],
        serde_json::json!(["function", "function", "function"]),
        "157 exposes all three (truth: winkeys-truth-157.json)"
    );
    assert_eq!(
        obj["pruned"],
        serde_json::json!(["undefined", "undefined"]),
        "the SharedStorage family left with 155; 157 prunes it (version_prune.js)"
    );
    assert_eq!(obj["method"], "function");
    assert_eq!(obj["method_enum"], true);
    assert_eq!(obj["method_ctor"], "moveTo is not a constructor");
    assert_eq!(obj["obj_tag"], "[object BarProp]");
    assert_eq!(obj["obj_enum"], true);
    assert_eq!(obj["client_info"], true);
    assert_eq!(obj["credless"], false);
    assert_eq!(obj["onsearch_acc"], "function");
    assert_eq!(obj["onsearch_val"], serde_json::Value::Null);
}

/// (300015 P5) Window descriptor parity: Chrome exposes the classic
/// window table as accessors, not writable data props — getter-only
/// enumerable configurable (crossOriginIsolated/crypto/navigator/...),
/// getter + [[Replaceable]] setter that really stores (name/dpr/self/...,
/// name/status coerce via ToString), getter-only NON-configurable
/// (document/top/window), and webkitRTCPeerConnection as a
/// non-enumerable writable value. Truth: tools/webplatform/chrome/chrome-windesc-truth-1015.json.
#[test]
fn window_descriptor_face_p5() {
    let mut rt = setup_runtime("<html><body>x</body></html>");
    let v = rt.evaluate(r#"
            var r = {};
            var d = function(n){ return Object.getOwnPropertyDescriptor(window, n); };
            // Group A: getter-only, enumerable, configurable (delete works).
            ['crossOriginIsolated','crypto','customElements','frameElement','history',
             'indexedDB','localStorage','navigator','sessionStorage','speechSynthesis']
              .forEach(function(n){
                var x = d(n);
                r['ga_'+n] = !!(x && typeof x.get === 'function' && x.set === undefined
                             && x.enumerable === true && x.configurable === true);
              });
            // Group B: getter + replaceable setter, enumerable, configurable.
            ['devicePixelRatio','frames','length','name','onerror','opener','parent',
             'performance','screen','screenLeft','screenTop','screenX','screenY',
             'self','status','visualViewport'].forEach(function(n){
                var x = d(n);
                r['gb_'+n] = !!(x && typeof x.get === 'function' && typeof x.set === 'function'
                             && x.enumerable === true && x.configurable === true);
              });
            // Group C: getter-only NON-configurable.
            ['document','top','window'].forEach(function(n){
                var x = d(n);
                r['gc_'+n] = !!(x && typeof x.get === 'function' && x.set === undefined
                             && x.enumerable === true && x.configurable === false);
              });
            // webkitRTCPeerConnection: non-enumerable writable data, still in gopn.
            var w = d('webkitRTCPeerConnection');
            r.wrtc = !!(w && !w.enumerable && w.writable === true && typeof w.value === 'function');
            r.wrtc_gopn = Object.getOwnPropertyNames(window).indexOf('webkitRTCPeerConnection') >= 0;
            r.wrtc_keys = Object.keys(window).indexOf('webkitRTCPeerConnection') < 0;
            // Identity invariants survive the conversion.
            r.self_eq = window.self === window;
            r.window_eq = window.window === window;
            r.top_eq = window.top === window;
            r.frames_eq = window.frames === window;
            r.parent_eq = window.parent === window;
            r.doc_eq = window.document === document;
            r.nav_type = typeof window.navigator;
            r.perf_type = typeof window.performance;
            r.hist_type = typeof window.history;
            // Assignment semantics: Group A silently ignored (sloppy, like Chrome).
            var navBefore = window.navigator;
            r.nav_ignored = (function(){ try { window.navigator = 1; return window.navigator === navBefore; } catch(e){ return 'THROW'; } })();
            var docBefore = window.document;
            r.doc_ignored = (function(){ try { window.document = 1; return window.document === docBefore; } catch(e){ return 'THROW'; } })();
            // Group B really stores; name/status coerce through String().
            var dprBefore = window.devicePixelRatio;
            window.devicePixelRatio = 3;
            r.dpr_stored = window.devicePixelRatio === 3;
            window.devicePixelRatio = dprBefore;
            window.name = 42;
            r.name_stored = window.name === '42';
            r.name_type = typeof window.name;
            window.status = {a:1};
            r.status_coerced = window.status === '[object Object]';
            window.opener = 2;
            r.opener_stored = window.opener === 2;
            // Group C: delete is refused (non-configurable); navigator delete works.
            r.top_del = (function(){ try { delete window.top; return 'no-throw'; } catch(e){ return e.name; } })();
            r.top_survives = window.top === window;
            r.nav_del = delete window.navigator;
            r.nav_gone = window.navigator === undefined;
            JSON.stringify(r)
        "#).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(v.as_str().expect("string from JSON.stringify")).unwrap();
    let obj = v.as_object().unwrap();
    let typed = ["nav_type", "perf_type", "hist_type", "name_type", "top_del"];
    for k in obj.keys() {
        if typed.contains(&k.as_str()) {
            continue;
        }
        assert_eq!(
            obj[k], true,
            "face key {} must be true, got {:?}",
            k, obj[k]
        );
    }
    assert_eq!(obj["nav_type"], "object");
    assert_eq!(obj["perf_type"], "object");
    assert_eq!(obj["hist_type"], "object");
    assert_eq!(
        obj["name_type"], "string",
        "name stores through String() coercion"
    );
    // Sloppy delete on a non-configurable is a silent false, never a throw.
    assert_eq!(
        obj["top_del"], "no-throw",
        "sloppy delete must not throw on Group C"
    );
}

/// (300015 P6) navigator.connection / navigator.userAgentData shape.
/// Chrome truth (CDP 1015, ark.xiaohongshu.com): both interfaces are
/// window globals whose `new` throws Illegal constructor;
/// NetworkInformation.prototype holds exactly [onchange, effectiveType,
/// rtt, downlink, saveData, constructor] — constructor LAST — with the
/// EventTarget methods inherited one level up; the dead-spec faces
/// type/downlinkMax/ontypechange are gone (undefined).
/// NavigatorUAData.prototype is [brands, mobile, platform,
/// getHighEntropyValues, toJSON, constructor] with mobile a prototype
/// getter (desktop persona: false). Both instances have ZERO own names
/// and instanceof ties back to the exposed global.
#[test]
fn conn_uad_face_p6() {
    let mut rt = setup_runtime("<html><body>x</body></html>");
    let v = rt.evaluate(r#"
            var r = {};
            var c = navigator.connection, uad = navigator.userAgentData;
            r.ni_type = typeof window.NetworkInformation;
            r.nuad_type = typeof window.NavigatorUAData;
            r.ni_enum = Object.getOwnPropertyDescriptor(window, 'NetworkInformation').enumerable === false;
            r.nuad_enum = Object.getOwnPropertyDescriptor(window, 'NavigatorUAData').enumerable === false;
            r.ni_ctor = (function(){ try { new NetworkInformation(); return 'no-throw'; } catch(e){ return e.name + ': ' + e.message; } })();
            r.nuad_ctor = (function(){ try { new NavigatorUAData(); return 'no-throw'; } catch(e){ return e.name + ': ' + e.message; } })();
            r.c_own = JSON.stringify(Object.getOwnPropertyNames(c));
            r.c_tag = Object.prototype.toString.call(c);
            r.c_str = '' + c;
            r.c_typeof_type = typeof c.type;
            r.c_typeof_dmax = typeof c.downlinkMax;
            r.c_typeof_otc = typeof c.ontypechange;
            r.c_vals = [c.effectiveType, c.rtt, c.downlink, c.saveData].join('|');
            r.c_proto = JSON.stringify(Object.getOwnPropertyNames(Object.getPrototypeOf(c)));
            r.c_proto_tag = Object.prototype.toString.call(Object.getPrototypeOf(c));
            r.c_et_level = Object.getPrototypeOf(Object.getPrototypeOf(c)) === EventTarget.prototype;
            r.c_add_rm = (function(){ var f=function(){}; c.addEventListener('change', f); c.removeEventListener('change', f); return typeof c.addEventListener; })();
            r.c_onchange = (function(){ var f=function(){}; c.onchange = f; var ok = c.onchange === f; c.onchange = null; return ok && c.onchange === null; })();
            r.c_instanceof = c instanceof NetworkInformation;
            r.c_enum = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(c), 'downlink').enumerable === true;
            r.u_own = JSON.stringify(Object.getOwnPropertyNames(uad));
            r.u_tag = Object.prototype.toString.call(uad);
            r.u_proto = JSON.stringify(Object.getOwnPropertyNames(Object.getPrototypeOf(uad)));
            r.u_mobile = uad.mobile === false;
            r.u_mobile_proto = typeof Object.getOwnPropertyDescriptor(Object.getPrototypeOf(uad), 'mobile').get;
            r.u_json_shape = (function(){ var j = uad.toJSON(); return j.mobile === false && typeof j.platform === 'string' && j.platform.length > 0; })();
            r.u_json_brands = JSON.stringify(uad.toJSON().brands);
            r.u_gev = typeof uad.getHighEntropyValues === 'function';
            r.u_instanceof = uad instanceof NavigatorUAData;
            r.u_enum = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(uad), 'brands').enumerable === true;
            JSON.stringify(r)
        "#).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(v.as_str().expect("string from JSON.stringify")).unwrap();
    let obj = v.as_object().unwrap();
    let typed = [
        "ni_type",
        "nuad_type",
        "ni_ctor",
        "nuad_ctor",
        "c_typeof_type",
        "c_typeof_dmax",
        "c_typeof_otc",
        "c_vals",
        "c_own",
        "c_tag",
        "c_str",
        "c_proto",
        "c_proto_tag",
        "c_add_rm",
        "u_own",
        "u_tag",
        "u_proto",
        "u_mobile_proto",
        "u_json_brands",
    ];
    for k in obj.keys() {
        if typed.contains(&k.as_str()) {
            continue;
        }
        assert_eq!(
            obj[k], true,
            "face key {} must be true, got {:?}",
            k, obj[k]
        );
    }
    // toJSON().brands, derived from the target instead of pinned (the '145'/'99' literals went red on the 157 flip, #28):
    // Chromium's GREASE (user_agent_utils.cc GenerateBrandVersionList), re-derived here independently of ua_helpers.js so the
    // test is an oracle, not a copy - "Not"+C[m%11]+"A"+C[(m+1)%11]+"Brand" at [8,99,24][m%3], [grease, Chromium, Google
    // Chrome] placed by ORD3[m%6] (shuffled[order[i]] = list[i]).
    let m = diting_env::chrome_target::CHROME_MAJOR as usize;
    const C: [&str; 11] = [" ", "(", ":", "-", ".", "/", ")", ";", "=", "?", "_"];
    const ORD3: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let list = [
        (
            format!("Not{}A{}Brand", C[m % 11], C[(m + 1) % 11]),
            ["8", "99", "24"][m % 3].to_string(),
        ),
        ("Chromium".to_string(), m.to_string()),
        ("Google Chrome".to_string(), m.to_string()),
    ];
    let mut want = vec![serde_json::Value::Null; 3];
    for (i, (brand, version)) in list.into_iter().enumerate() {
        want[ORD3[m % 6][i]] = serde_json::json!({ "brand": brand, "version": version });
    }
    let brands: serde_json::Value =
        serde_json::from_str(obj["u_json_brands"].as_str().unwrap()).unwrap();
    assert_eq!(
        brands,
        serde_json::Value::Array(want),
        "UA-CH brands for Chrome {m}"
    );
    assert_eq!(obj["ni_type"], "function");
    assert_eq!(obj["nuad_type"], "function");
    assert_eq!(
        obj["ni_ctor"],
        "TypeError: Failed to construct 'NetworkInformation': Illegal constructor"
    );
    assert_eq!(
        obj["nuad_ctor"],
        "TypeError: Failed to construct 'NavigatorUAData': Illegal constructor"
    );
    assert_eq!(
        obj["c_typeof_type"], "undefined",
        "dead-spec conn.type must be gone"
    );
    assert_eq!(obj["c_typeof_dmax"], "undefined");
    assert_eq!(obj["c_typeof_otc"], "undefined");
    assert_eq!(obj["c_vals"], "4g|50|10|false");
    assert_eq!(obj["c_own"], "[]");
    assert_eq!(obj["c_tag"], "[object NetworkInformation]");
    assert_eq!(obj["c_str"], "[object NetworkInformation]");
    assert_eq!(
        obj["c_proto"],
        r#"["onchange","effectiveType","rtt","downlink","saveData","constructor"]"#
    );
    assert_eq!(obj["c_proto_tag"], "[object NetworkInformation]");
    assert_eq!(obj["c_add_rm"], "function");
    assert_eq!(obj["u_own"], "[]");
    assert_eq!(obj["u_tag"], "[object NavigatorUAData]");
    assert_eq!(
        obj["u_proto"],
        r#"["brands","mobile","platform","getHighEntropyValues","toJSON","constructor"]"#
    );
    assert_eq!(obj["u_mobile_proto"], "function");
}

/// (#37) HTMLBodyElement's window-reflecting on* family: 24 names whose
/// body accessors ARE the window's — shared identity both directions,
/// `<body onX>` content attributes install as the window's handler and
/// fire on window dispatch, non-callables store null (suppressing the
/// attribute handler), click stays element-local. Chrome 152 truth,
/// probed on headless 152 before writing any of this.

#[test]
fn body_window_reflecting_on_family() {
    let mut rt = setup_runtime(
            "<html><body onresize='window.__attrResizeRan=1' onclick='window.__attrClickRan=1'>x</body></html>",
        );
    let out = rt.evaluate(r#"
            var b = document.body;
            var names = ['afterprint','beforeprint','beforeunload','blur','error','focus',
                         'gamepadconnected','gamepaddisconnected','hashchange','languagechange',
                         'load','message','messageerror','offline','online','pagehide',
                         'pageshow','popstate','rejectionhandled','resize','scroll','storage',
                         'unhandledrejection','unload'];
            var protoKeys = Object.keys(HTMLBodyElement.prototype);
            var face = names.every(function(n){ return protoKeys.indexOf('on'+n) >= 0; });
            var d = Object.getOwnPropertyDescriptor(HTMLBodyElement.prototype, 'onhashchange');
            var desc = d ? ['get' in d ? 'acc' : 'data', d.enumerable, d.configurable].join(':') : 'none';
            // shared slot, identity both directions (dirty hashchange/popstate;
            // resize keeps its attribute for the exposure checks below)
            var f1 = function(){}, f2 = function(){};
            b.onhashchange = f1; var bodyToWin = window.onhashchange === f1;
            window.onpopstate = f2; var winToBody = b.onpopstate === f2;
            // <body onresize> is the WINDOW's handler: same function from both
            // getters, and it runs when the event dispatches on window.
            var attrBody = typeof b.onresize, attrWin = typeof window.onresize;
            var attrSame = b.onresize === window.onresize;
            window.dispatchEvent(new Event('resize'));
            var attrFired = window.__attrResizeRan || 0;
            // non-callable assignment stores null AND suppresses the attribute
            // handler (an explicit null overwrites, it does not fall through)
            b.onresize = 'garbage';
            var afterGarbage = window.onresize === null;
            window.__attrResizeRan = 0;
            window.dispatchEvent(new Event('resize'));
            var nullSuppresses = window.__attrResizeRan || 0;
            // click is NOT in the family: element-local store, no window
            // forwarding, and its content attribute runs on the element path
            var f3 = function(){};
            b.onclick = f3;
            var clickForwarded = window.onclick === f3;
            b.onclick = null;
            b.dispatchEvent(new Event('click'));
            var clickFired = window.__attrClickRan || 0;
            var gamepad = ['ongamepadconnected','ongamepaddisconnected'].every(function(k){ return k in window; });
            return JSON.stringify({
                face: face, desc: desc, bodyToWin: bodyToWin, winToBody: winToBody,
                attrBody: attrBody, attrWin: attrWin, attrSame: attrSame, attrFired: attrFired,
                afterGarbage: afterGarbage, nullSuppresses: nullSuppresses,
                clickForwarded: clickForwarded, clickFired: clickFired, gamepad: gamepad,
            });
        "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["face"],
        serde_json::json!(true),
        "all 24 window-reflecting names are enumerable own members of HTMLBodyElement.prototype"
    );
    assert_eq!(
        v["desc"],
        serde_json::json!("acc:true:true"),
        "accessor + enumerable + configurable, like Chrome 152"
    );
    assert_eq!(
        v["bodyToWin"],
        serde_json::json!(true),
        "body.X = f makes window.X === f"
    );
    assert_eq!(
        v["winToBody"],
        serde_json::json!(true),
        "window.X = f makes body.X === f"
    );
    assert_eq!(
        v["attrBody"],
        serde_json::json!("function"),
        "<body onresize> reads back as a function from the body getter"
    );
    assert_eq!(
        v["attrWin"],
        serde_json::json!("function"),
        "...and from the window getter"
    );
    assert_eq!(
        v["attrSame"],
        serde_json::json!(true),
        "both getters return the SAME compiled function — one handler"
    );
    assert_eq!(
        v["attrFired"],
        serde_json::json!(1),
        "<body onresize> runs when resize dispatches on window"
    );
    assert_eq!(
        v["afterGarbage"],
        serde_json::json!(true),
        "non-callable assignment stores null"
    );
    assert_eq!(
        v["nullSuppresses"],
        serde_json::json!(0),
        "an explicit null overwrites the attribute handler — no fall-through"
    );
    assert_eq!(
        v["clickForwarded"],
        serde_json::json!(false),
        "click is element-local: no window forwarding"
    );
    assert_eq!(
        v["clickFired"],
        serde_json::json!(1),
        "<body onclick> still runs on the element dispatch path"
    );
    assert_eq!(
        v["gamepad"],
        serde_json::json!(true),
        "ongamepadconnected/ongamepaddisconnected exist on window too"
    );
}

/// (#168) The dispatch host is the event's target. The window arm never
/// pinned target/currentTarget/eventPhase, so a listener reading
/// e.target.tagName saw null — Alibaba AWSC's nc-slider init dies on
/// exactly that (the punish-page slider track then never sizes). The
/// document/element arms already pinned; this pins the window arm, the
/// on*-attribute leg, and the srcElement alias Chrome keeps on
/// Event.prototype.
#[test]
fn window_dispatch_pins_target_family() {
    let mut rt = setup_runtime("<html><body><div id=\"d\">x</div></body></html>");
    let out = rt
        .evaluate(
            r#"
            var seen = {};
            addEventListener('__wt', function (e) {
                seen.l = [e.target === window, e.currentTarget === window,
                          e.eventPhase, e.srcElement === window];
            });
            window.dispatchEvent(new CustomEvent('__wt'));
            var onSeen = {};
            // a name inside _WINDOW_EVENT_NAMES — custom names land as plain
            // data props and never reach the __windowOnHandlers store
            window.onlanguagechange = function (e) {
                onSeen.l = [e.target === window, e.currentTarget === window,
                            e.eventPhase, e.srcElement === window];
            };
            window.dispatchEvent(new Event('languagechange'));
            var docSeen = {}, elSeen = {};
            document.addEventListener('__wt3', function (e) {
                docSeen.l = [e.target === document, e.srcElement === document,
                             e.currentTarget === document];
            });
            document.dispatchEvent(new Event('__wt3'));
            var el = document.getElementById('d');
            el.addEventListener('__wt4', function (e) {
                elSeen.l = [e.target === el, e.srcElement === el,
                            e.currentTarget === el, e.eventPhase];
            });
            el.dispatchEvent(new Event('__wt4'));
            var srcDesc = Object.getOwnPropertyDescriptor(Event.prototype, 'srcElement');
            return JSON.stringify({
                l: seen.l, on: onSeen.l, doc: docSeen.l, el: elSeen.l,
                srcGetter: !!srcDesc && !!srcDesc.get && !srcDesc.set,
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["l"],
        serde_json::json!([true, true, 2, true]),
        "window dispatch: listener sees target/currentTarget/srcElement=window at AT_TARGET"
    );
    assert_eq!(
        v["on"],
        serde_json::json!([true, true, 2, true]),
        "the on*-attribute leg sees the same pinned state"
    );
    assert_eq!(
        v["doc"],
        serde_json::json!([true, true, true]),
        "document dispatch stays pinned to document (regression guard)"
    );
    assert_eq!(
        v["el"],
        serde_json::json!([true, true, true, 2]),
        "element dispatch stays pinned to the element (regression guard)"
    );
    assert_eq!(
        v["srcGetter"],
        serde_json::json!(true),
        "srcElement is a getter-only alias on Event.prototype, Chrome shape"
    );
}

/// #40: navigator.platform under a Windows UA must be "Win32" (real
/// Chrome reports Win32 on 64-bit Windows and 64-bit Chrome alike) while
/// userAgentData.platform says "Windows" — the two faces must never
/// collide. Linux personas (obscura#987 same face) must derive
/// "Linux x86_64" so a Linux deployment's JS face agrees with its
/// kernel's SYN fingerprint.
#[test]
fn navigator_platform_matches_ua_os_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // The production path: set_user_agent writes __diting_ua and
    // refreshes the persona; the getters derive from the UA after.
    rt.set_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36");
    assert_eq!(
        rt.evaluate("navigator.platform").unwrap(),
        serde_json::json!("Win32")
    );
    assert_eq!(
        rt.evaluate("navigator.userAgentData.platform").unwrap(),
        serde_json::json!("Windows")
    );
    rt.set_user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36");
    assert_eq!(
        rt.evaluate("navigator.platform").unwrap(),
        serde_json::json!("Linux x86_64")
    );
    assert_eq!(
        rt.evaluate("navigator.userAgentData.platform").unwrap(),
        serde_json::json!("Linux")
    );
    rt.set_user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36");
    assert_eq!(
        rt.evaluate("navigator.platform").unwrap(),
        serde_json::json!("MacIntel")
    );
    assert_eq!(
        rt.evaluate("navigator.userAgentData.platform").unwrap(),
        serde_json::json!("macOS")
    );
}

/// (#104) The Navigator interface object exists on window (the Douyin
/// SDK subclasses it), `new Navigator()` is an illegal constructor, the
/// navigator instance sits on Navigator.prototype (instanceof and
/// getPrototypeOf both Chrome-shaped), and subclass linkage works.
#[tokio::test(flavor = "current_thread")]
async fn navigator_interface_object_face() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const isFn = typeof Navigator === 'function';
                    const inst = navigator instanceof Navigator;
                    const proto = Object.getPrototypeOf(navigator) === Navigator.prototype;
                    let ctorThrew = false;
                    try { new Navigator(); } catch (e) { ctorThrew = e.name === 'TypeError'; }
                    let subLinked = false;
                    try {
                        class t extends Navigator {}
                        subLinked = Object.create(t.prototype) instanceof Navigator;
                    } catch (e) {}
                    const tag = navigator[Symbol.toStringTag];
                    return [isFn, inst, proto, ctorThrew, subLinked, tag];
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v[0], serde_json::json!(true));
    assert_eq!(
        v[1],
        serde_json::json!(true),
        "navigator instanceof Navigator"
    );
    assert_eq!(v[2], serde_json::json!(true), "prototype chain");
    assert_eq!(v[3], serde_json::json!(true), "new Navigator() is illegal");
    assert_eq!(v[4], serde_json::json!(true), "subclass linkage");
    assert_eq!(v[5], serde_json::json!("Navigator"), "toStringTag survives");
}

/// (#117/F4) navigator/screen carry ZERO own enumerable props (Chrome
/// ground truth: Object.keys(navigator) === []), the Chrome 145
/// prototype key set is present, page writes against the getter-only
/// prototype accessors no-op, and the sanctioned internal write path
/// (__diting_navSet) still moves values.
#[tokio::test(flavor = "current_thread")]
async fn navigator_screen_own_props_hoisted_to_prototype() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const navKeys = Object.keys(navigator).length;
                    const protoKeys = Object.keys(Object.getPrototypeOf(navigator));
                    const chromeOnlyKeys = ['vendorSub', 'appCodeName', 'appName', 'vibrate',
                                            'getInstalledRelatedApps', 'credentials', 'gpu'].every(
                        (k) => protoKeys.includes(k));
                    const scrKeys = Object.keys(screen).length;
                    const scrW = screen.width > 0;
                    const uaGetter = !!Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent').get;
                    navigator.hardwareConcurrency = 2;
                    const pageWriteIgnored = navigator.hardwareConcurrency !== 2;
                    globalThis.__diting_navSet('hardwareConcurrency', 3);
                    const navSetMoves = navigator.hardwareConcurrency === 3;
                    const tag = Object.prototype.toString.call(navigator);
                    return [navKeys, protoKeys.length, chromeOnlyKeys, scrKeys, scrW,
                            uaGetter, pageWriteIgnored, navSetMoves, tag];
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();

    let v = result.value.unwrap();
    assert_eq!(
        v[0],
        serde_json::json!(0),
        "Object.keys(navigator) is empty"
    );
    // 刀57: 82 = Chrome 145's exact enumerable prototype face (the old 83
    // counted the invented cpuPerformance; constructor is non-enumerable).
    assert_eq!(
        v[1],
        serde_json::json!(82),
        "Chrome 145's exact prototype face"
    );
    assert_eq!(
        v[2],
        serde_json::json!(true),
        "previously missing keys present"
    );
    assert_eq!(v[3], serde_json::json!(0), "Object.keys(screen) is empty");
    assert_eq!(v[4], serde_json::json!(true), "screen.width still reads");
    assert_eq!(
        v[5],
        serde_json::json!(true),
        "prototype accessor (not data prop)"
    );
    assert_eq!(
        v[6],
        serde_json::json!(true),
        "page write no-ops like Chrome"
    );
    assert_eq!(
        v[7],
        serde_json::json!(true),
        "__diting_navSet moves the value"
    );
    assert_eq!(
        v[8],
        serde_json::json!("[object Navigator]"),
        "toStringTag survives the hoist"
    );
}

/// (#105) innerWidth/innerHeight/outer* are getter-only accessors like
/// Chrome's — a page's plain assignment must be ignored (a resize
/// polyfill on douyin overwrote them with document extents and skewed
/// every coordinate by a viewport), while __diting_setViewport still
/// moves them.
#[tokio::test(flavor = "current_thread")]
async fn window_size_props_ignore_page_writes() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const d = Object.getOwnPropertyDescriptor(window, 'innerWidth');
                    const before = window.innerWidth;
                    window.innerWidth = 5;
                    window.innerHeight = 5;
                    window.outerWidth = 5;
                    const after = [window.innerWidth, window.innerHeight, window.outerWidth];
                    const unchanged = after[0] === before && after[1] > 0 && after[2] === before;
                    globalThis.__diting_setViewport(400, 800, false, 0);
                    const moved = window.innerWidth === 400 && window.innerHeight === 800;
                    return [!!d.get, unchanged, moved];
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v[0], serde_json::json!(true), "innerWidth is an accessor");
    assert_eq!(v[1], serde_json::json!(true), "page writes are ignored");
    assert_eq!(
        v[2],
        serde_json::json!(true),
        "setViewport still moves them"
    );
}

#[test]
fn test_navigator() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let ua = rt.evaluate("navigator.userAgent").unwrap();
    assert!(
        ua.as_str().unwrap().contains("Chrome"),
        "UA should contain Chrome: {}",
        ua
    );
    let wd = rt.evaluate("navigator.webdriver").unwrap();
    // (#78) Chrome defines webdriver (NavigatorAutomationInformation) and
    // it reads false on a normal session — `undefined` was the outlier.
    assert_eq!(wd, serde_json::json!(false));
    let plugins = rt.evaluate("navigator.plugins.length").unwrap();
    assert!(plugins.as_f64().unwrap() > 0.0, "Should have plugins");
    let chrome = rt.evaluate("typeof window.chrome").unwrap();
    assert_eq!(chrome, serde_json::json!("object"));
}

/// Upstream 9dfc67a: the global's constructor identity is Window, not the
/// inherited Object — framework environment gates check it directly.
#[test]
fn global_window_has_browser_constructor_identity() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
            .evaluate(
                "(() => [window === self, self.constructor === Window, window instanceof Window, self.document === document, self.navigator === navigator])()",
            )
            .unwrap();
    assert_eq!(v, serde_json::json!([true, true, true, true, true]));
}

#[test]
fn test_network_information_event_listeners() {
    // Upstream fc9f524: navigator.connection was a data-only object with
    // no event methods at all; analytics libs calling addEventListener
    // threw. dispatchEvent must run registered listeners with the
    // connection as receiver, honor the on* property, and respect
    // removeEventListener.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const connection = navigator.connection;
            let calls = 0, receiverMatches = false, viaProp = 0;
            function listener(event) {
                calls += 1;
                receiverMatches = this === connection && event.type === 'change';
            }
            connection.addEventListener('change', listener);
            connection.onchange = () => viaProp++;
            const dispatchResult = connection.dispatchEvent(new Event('change'));
            connection.removeEventListener('change', listener);
            connection.dispatchEvent(new Event('change'));
            return [
                typeof connection.addEventListener,
                typeof connection.removeEventListener,
                typeof connection.dispatchEvent,
                dispatchResult, calls, receiverMatches, viaProp,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["function", "function", "function", true, 1, true, 2])
    );
}

#[test]
fn test_window_scalar_faces_exist_like_chrome() {
    // window.name was absent entirely and PDD's SDK did window.name.split
    // inside a windowName getter — the TypeError killed the goods-list
    // React mount (2026-10-12). name/status are writable "" defaults;
    // origin tracks location; closed is false.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const before = window.name;
            window.name = 'renamed';
            return [
                before, window.name, 'name' in window,
                window.status, typeof window.origin, window.origin === location.origin,
                window.closed === false,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["", "renamed", true, "", "string", true, true,])
    );
}

/// #203: WebIDL operations are writable data properties on the prototype.
/// The 抖店 secsdk monkey-patches navigator.sendBeacon (createAspectByPath)
/// and the getter-only accessors #117 installed turned that strict-mode
/// assignment into a TypeError. Attributes stay getter-only.
#[test]
fn navigator_methods_are_writable_data_props() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () {
          const out = {};
          const d = Object.getOwnPropertyDescriptor(Navigator.prototype, 'sendBeacon');
          out.desc = { w: d.writable, e: d.enumerable, c: d.configurable, t: typeof d.value };
          out.ownKeysBefore = Object.keys(navigator).length;
          out.orig = navigator.sendBeacon;
          out.strictAssign = (function () {
            'use strict';
            try { navigator.sendBeacon = function () { return 99; }; return 'ok'; }
            catch (e) { return e.name; }
          })();
          out.shadowed = navigator.sendBeacon();
          out.shadowOwn = Object.keys(navigator);
          delete navigator.sendBeacon;
          out.restored = (navigator.sendBeacon === out.orig);
          out.attrWrite = (function () {
            'use strict';
            try { navigator.userAgent = 'spoof'; return 'no-throw'; }
            catch (e) { return e.name; }
          })();
          out.attrKept = (navigator.userAgent !== 'spoof');
          return JSON.stringify(out);
        })()
    "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["desc"],
        serde_json::json!({"w": true, "e": true, "c": true, "t": "function"})
    );
    assert_eq!(
        v["ownKeysBefore"],
        serde_json::json!(0),
        "#117 invariant: no own enumerable props"
    );
    assert_eq!(
        v["strictAssign"],
        serde_json::json!("ok"),
        "secsdk's strict-mode patch must succeed"
    );
    assert_eq!(v["shadowed"], serde_json::json!(99));
    assert_eq!(
        v["shadowOwn"],
        serde_json::json!(["sendBeacon"]),
        "assignment shadows via an own prop, like Chrome"
    );
    assert_eq!(
        v["restored"],
        serde_json::json!(true),
        "delete falls back to the prototype op"
    );
    assert_eq!(
        v["attrWrite"],
        serde_json::json!("TypeError"),
        "attributes stay getter-only"
    );
    assert_eq!(v["attrKept"], serde_json::json!(true));
}

/// #48: window named access. Elements with an id (and form/iframe/embed/
/// object/img with a name) are reachable as bare identifiers and window
/// properties, live at read time, shadowed by real globals, and ceded on
/// explicit assignment — the resolution order Chrome implements.
#[test]
fn window_named_access_live_and_shadowing() {
    let mut rt = setup_runtime(
        "<html><body><div id=\"hero\">H</div><form name=\"login\"></form>\
         <div id=\"location\">L</div><div id=\"123\">N</div></body></html>",
    );
    let out = rt.evaluate(r#"
        (function () { try {
        globalThis.__out = JSON.stringify([
            typeof hero, hero && hero.tagName,                       // parser tree: bare id resolves
            window.hero === document.getElementById('hero'),          // identity through the same wrap cache
            typeof login, login.tagName,                              // form name access
            window['hero'] === document.getElementById('hero'),       // bracket form
            typeof location === 'object' && location.href !== undefined, // real global wins over id="location"
            typeof window['123'],                                     // array-index names never become named props
            (function () { try { return window.nope.tagName; } catch (e) { return 'unresolved'; } })(),
        ]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#).unwrap();
    let s = out.as_str().unwrap();
    assert!(!s.starts_with("ERR"), "named access probe threw: {}", s);
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!(["object", "DIV", true, "object", "FORM", true, true, "undefined", "unresolved"]),
        "named access must resolve parser-built ids/names, keep real globals, skip index-like names"
    );
}

/// #48 second face: the registry tracks mutations (appendChild/removeChild/
/// innerHTML) so named access reflects the live tree, and an explicit
/// assignment converts the slot to a data property that wins from then on.
#[test]
fn window_named_access_tracks_mutations_and_assignment() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let _out = rt
        .evaluate(
            r#"
        const mk = function (id) { const d = document.createElement('div'); d.id = id; return d; };
        const a = mk('zed'); document.body.appendChild(a);
        const afterAppend = typeof zed !== 'undefined' && zed === a;
        document.body.removeChild(a);
        const afterRemove = typeof zed;
        document.body.appendChild(mk('zed2'));
        window.zed2 = 5;
        const afterAssign = zed2;
        document.body.innerHTML = '<div id="rebuilt">R</div>';
        const afterInner = typeof rebuilt !== 'undefined' && rebuilt.textContent;
        globalThis.__out = JSON.stringify([afterAppend, afterRemove, afterAssign, afterInner]);
    "#,
        )
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(rt.evaluate("globalThis.__out").unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v,
        serde_json::json!([true, "undefined", 5, "R"]),
        "named access must follow appends/removals/innerHTML and cede to explicit assignment"
    );
}

/// 2026-10-18 sweep (moli ca22988): navigator.languages is a frozen array and
/// the same object on every read while the locale is unchanged (HTML spec
/// returns a frozen array; Chrome hands back the cached instance).
#[test]
fn navigator_languages_frozen_and_stable() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () { try {
        globalThis.__out = JSON.stringify([
            Object.isFrozen(navigator.languages),
            navigator.languages === navigator.languages,
            Array.isArray(navigator.languages),
        ]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#,
        )
        .unwrap();
    let s = out.as_str().unwrap();
    assert!(!s.starts_with("ERR"), "languages probe threw: {}", s);
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!([true, true, true]),
        "languages must be a frozen array, identical across reads"
    );
}

/// #48 third face (300015 winKeys, 10-17): Chrome resolves named access
/// through a WindowProperties layer (under Window.prototype) — window.q
/// works, but the name never appears in getOwnPropertyNames(window) and
/// never owns a window slot. Own enumerable getters on window leak every
/// element id/name into the winKeys face (and never uninstall on untrack,
/// so the face only grows). The named layer sits under Window.prototype:
/// resolution unchanged, enumeration face clean, untrack shrinks with tree.
#[test]
fn window_named_access_invisible_to_enumeration() {
    let mut rt = setup_runtime(
        "<html><body><div id=\"hero\">H</div><svg><path id=\"glyph\"/></svg></body></html>",
    );
    let _out = rt.evaluate(r#"
        const base = Object.getOwnPropertyNames(window).length;
        document.body.appendChild(document.createElement('img')).id = 'shot';
        const layer = Object.getPrototypeOf(window);
        const resolves = typeof hero === 'object' && hero.tagName;      // resolution walks the proto chain
        const removed = document.body.removeChild(document.getElementById('hero'));
        globalThis.__out = JSON.stringify([
            resolves,
            Object.getOwnPropertyNames(window).includes('hero'),        // NEVER in the window face
            Object.getOwnPropertyNames(window).length === base,          // appending a named element doesn't grow it
            Object.getOwnPropertyDescriptor(window, 'hero') === undefined, // never owns a window slot
            'glyph' in window,                                           // has-check walks the chain
            Object.getOwnPropertyNames(layer).includes('glyph'),         // the named layer carries it…
            removed.tagName,                                             // removal really happened
            Object.getOwnPropertyNames(layer).includes('hero'),         // …and shrinks when the tree does
            typeof window.hero,                                          // …and the access dies with it
            window.shot && window.shot.tagName,                          // post-change install path resolves too
            (window.hero2 = 7) === 7 && window.hero2,                    // assignment lands as the own window prop
        ]);
    "#).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(rt.evaluate("globalThis.__out").unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v,
        serde_json::json!(["DIV", false, true, true, true, false, "DIV", false, "undefined", "IMG", 7]),
        "named access must stay resolvable while invisible to window AND layer enumeration (Chrome's WindowProperties is empty to gopn)"
    );
}

/// Chrome-version coherence (docs: CHROME-COHERENCE-AUDIT): the full version the engine
/// claims through userAgentData must be a build Google actually published. The old table
/// extrapolated builds ("145.0.6509.41" - build 6509 is Chrome 127's), a tell any detector
/// can check against the public version database.
#[tokio::test(flavor = "current_thread")]
async fn ua_data_full_version_is_a_real_chrome_build() {
    let real: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/cft-builds.json"
    ))
    .unwrap();
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                var h = await navigator.userAgentData.getHighEntropyValues(['uaFullVersion', 'fullVersionList']);
                return JSON.stringify({ ua: h.uaFullVersion, list: h.fullVersionList });
            }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let mut claims = vec![v["ua"].as_str().unwrap().to_string()];
    for b in v["list"].as_array().unwrap() {
        let ver = b["version"].as_str().unwrap();
        if !ver.ends_with(".0.0.0") {
            claims.push(ver.to_string());
        }
    }
    for full in claims {
        let mut parts = full.splitn(3, '.');
        let major = parts.next().unwrap().to_string();
        parts.next();
        let build = parts.next().unwrap().to_string();
        let ok = real["builds"][&major]
            .as_array()
            .map(|a| a.iter().any(|b| b.as_str() == Some(build.as_str())))
            .unwrap_or(false);
        assert!(ok, "claimed {full} was never published by Google");
    }
}

/// #30 - the window's own-property face against real Chrome 157 (window-face-probe.js): every own accessor is a native
/// `get X` / `set X` pair (188, names + flags + toString), the 20 [Replaceable] setters replace in place, the
/// non-configurable set is Chrome's seven, and getOwnPropertyNames(window) is Chrome's order position for position.
/// KNOWN GAP, must shrink: four 157 interfaces the engine does not carry yet (#25) are dropped from the truth order.
/// Deliberate deviation: `replace.after` (innerWidth assignment) - the size attributes keep ignoring page writes (#105,
/// window_size_props_ignore_page_writes); `replace.strict` (scrollX) still pins Chrome's in-place replacement.
#[tokio::test(flavor = "current_thread")]
async fn window_own_property_face_matches_chrome_truth() {
    const MISSING: &[&str] = &[
        "CanvasPaintEvent",
        "ElementGeometryUpdateEvent",
        "ElementImage",
        "TrustedHTMLParserOptions",
    ];
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(include_str!(
            "../../../../../tools/webplatform/chrome/window-face-probe.js"
        ))
        .unwrap();
    let engine: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let mut truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/window-face-truth-157.json"
    ))
    .unwrap();
    truth["order"]
        .as_array_mut()
        .unwrap()
        .retain(|n| !MISSING.contains(&n.as_str().unwrap()));
    let order_e = engine["order"].as_array().unwrap();
    let order_c = truth["order"].as_array().unwrap();
    if let Some(i) = order_c.iter().zip(order_e).position(|(c, e)| c != e) {
        panic!(
            "own-name order departs at {i}: engine {:?} chrome {:?}",
            &order_e[i..(i + 3).min(order_e.len())],
            &order_c[i..(i + 3).min(order_c.len())]
        );
    }
    assert_eq!(order_e.len(), order_c.len(), "own-name count");
    let mut bad = Vec::new();
    truth["replace"]["after"] = engine["replace"]["after"].clone();
    assert_eq!(
        engine["replace"]["after"],
        serde_json::json!([true, null, null, true, true, true]),
        "innerWidth stays an accessor that ignored the write (#105)"
    );
    for k in ["nonConfigurable", "replace"] {
        if engine[k] != truth[k] {
            bad.push(format!(
                "{k}:\n   engine {}\n   chrome {}",
                engine[k], truth[k]
            ));
        }
    }
    for (n, want) in truth["accessors"].as_object().unwrap() {
        if &engine["accessors"][n] != want {
            bad.push(format!(
                "accessor {n}:\n   engine {}\n   chrome {want}",
                engine["accessors"][n]
            ));
        }
    }
    assert!(bad.is_empty(), "{} off:\n{}", bad.len(), bad.join("\n"));
}

/// M4 knife 53 - the window's own ENUMERABLE keys (for-in + hasOwnProperty: what for-in, JSON.stringify, Object.assign and spread see;
/// Object.keys / getOwnPropertyNames are patched by the engine and cannot be trusted for this) must be exactly real Chrome 157's
/// (enum-truth-157.json selfWin; 157 vs 145: +requestResize, -sharedStorage).
/// Top-level function/var declarations are non-configurable enumerable window properties that no defineProperty can hide, so every
/// script keeps its top level in a function scope and re-exports only the names others need (hidden); see analyze.mjs --gen-exports.
#[tokio::test(flavor = "current_thread")]
async fn window_enumerable_own_keys_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"(function () { var ks = []; for (var k in globalThis) { if (Object.prototype.hasOwnProperty.call(globalThis, k)) ks.push(k); } return JSON.stringify(ks.sort()); })()"#,
        )
        .unwrap();
    let engine: Vec<String> = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/enum-truth-157.json"
    ))
    .unwrap();
    let mut chrome: Vec<String> = truth["selfWin"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    chrome.sort();
    let extra: Vec<&String> = engine.iter().filter(|k| !chrome.contains(k)).collect();
    let missing: Vec<&String> = chrome.iter().filter(|k| !engine.contains(k)).collect();
    assert!(
        extra.is_empty() && missing.is_empty(),
        "extra {extra:?} missing {missing:?}"
    );
}
