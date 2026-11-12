#![allow(clippy::await_holding_lock)] // net tests hold PRIVATE_NET_ENV_LOCK across awaits on purpose — the guard IS the serialization (1f7486c)

use super::*;
use self::common::*;

// 测试已按域分桶（docs/tests-分桶表-20261113.md，#74）；新测试进对应域桶，hub 只挂声明。
#[cfg(test)]
mod bridge;

#[cfg(test)]
mod canvas;

#[cfg(test)]
mod common;

#[cfg(test)]
mod cookies;

#[cfg(test)]
mod crypto;

#[cfg(test)]
mod css;

#[cfg(test)]
mod css_anim;

#[cfg(test)]
mod css_computed;

#[cfg(test)]
mod document;

#[cfg(test)]
mod dom_shadow;

#[cfg(test)]
mod dom_tree;

#[cfg(test)]
mod events;

#[cfg(test)]
mod files;

#[cfg(test)]
mod forms;

#[cfg(test)]
mod iframes;

#[cfg(test)]
mod knife_a;

#[cfg(test)]
mod knife_b;

#[cfg(test)]
mod knife_c;

#[cfg(test)]
mod layout_flow;

#[cfg(test)]
mod layout_geometry;

#[cfg(test)]
mod layout_scroll;

#[cfg(test)]
mod location;

#[cfg(test)]
mod media;

#[cfg(test)]
mod mutation;

#[cfg(test)]
mod net_fetch;

#[cfg(test)]
mod net_policy;

#[cfg(test)]
mod net_xhr;

#[cfg(test)]
mod performance;

#[cfg(test)]
mod scripts;

#[cfg(test)]
mod stealth_env;

#[cfg(test)]
mod storage;

#[cfg(test)]
mod text;

#[cfg(test)]
mod timers;


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

/// `svg.viewBox` must reflect as an animated rect on the fit-to-viewbox
/// SVG elements — the archify viewer's export path reads
/// `svg.viewBox.baseVal.width` directly, so an undefined `viewBox`
/// crashes one property later (found by the artifact viewer smoke probe).
#[test]
fn svg_viewbox_reflects_animated_rect() {
    let mut rt = setup_runtime(
        r#"<svg id="a" viewBox="0 0 880 588"></svg><svg id="b"></svg><div id="h"></div>"#,
    );
    let w = rt
        .evaluate("document.getElementById('a').viewBox.baseVal.width")
        .unwrap();
    assert_eq!(w.as_f64().unwrap(), 880.0);
    let h = rt
        .evaluate("document.getElementById('a').viewBox.animVal.height")
        .unwrap();
    assert_eq!(h.as_f64().unwrap(), 588.0);

    // Missing attribute: still an animated rect, all zeros (Chrome shape).
    let absent = rt
            .evaluate("var vb = document.getElementById('b').viewBox.baseVal; vb.x + ',' + vb.y + ',' + vb.width + ',' + vb.height")
            .unwrap();
    assert_eq!(absent, serde_json::json!("0,0,0,0"));

    // Malformed attribute parses as absent, not as garbage.
    let bad = rt
            .evaluate("document.getElementById('b').setAttribute('viewBox','10 20 100'); document.getElementById('b').viewBox.baseVal.width")
            .unwrap();
    assert_eq!(bad.as_f64().unwrap(), 0.0);
    let good = rt
            .evaluate("document.getElementById('b').setAttribute('viewBox','10 20 100 50'); var v = document.getElementById('b').viewBox.baseVal; v.x + ',' + v.width")
            .unwrap();
    assert_eq!(good, serde_json::json!("10,100"));

    // HTML elements keep no viewBox (Chrome exposes it only on the
    // fit-to-viewbox SVG tags).
    let html = rt
        .evaluate("document.getElementById('h').viewBox === undefined")
        .unwrap();
    assert_eq!(html, serde_json::json!(true));
}

/// #27: the interface prototype chain must read like Chrome's —
/// HTMLDivElement → HTMLElement → Element → Node → EventTarget — with
/// per-interface prototypes that actually isolate patches, and
/// constructor.name per tag (unknown tags report HTMLElement).
#[test]
fn interface_prototype_chain_and_patch_isolation() {
    let mut rt = setup_runtime(
        r#"<body><div id="d"></div><span id="s"></span><x-foo id="x"></x-foo></body>"#,
    );
    let checks: &[(&str, bool)] = &[
        ("document.getElementById('d') instanceof HTMLElement", true),
        ("document.getElementById('d') instanceof Element", true),
        ("document.getElementById('d') instanceof Node", true),
        ("document.getElementById('d') instanceof EventTarget", true),
        (
            "Object.getPrototypeOf(HTMLElement.prototype) === Element.prototype",
            true,
        ),
        (
            "Object.getPrototypeOf(Element.prototype) === Node.prototype",
            true,
        ),
        (
            "Object.getPrototypeOf(Node.prototype) === EventTarget.prototype",
            true,
        ),
        ("document instanceof EventTarget", true),
        ("EventTarget.prototype !== Node.prototype", true),
    ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
    // constructor.name per interface; unknown tag falls back to HTMLElement.
    let names = rt
        .evaluate(
            r#"JSON.stringify([
                    document.getElementById('d').constructor.name,
                    document.getElementById('s').constructor.name,
                    document.getElementById('x').constructor.name,
                ])"#,
        )
        .unwrap();
    assert_eq!(
        names,
        serde_json::json!(r#"["HTMLDivElement","HTMLSpanElement","HTMLElement"]"#)
    );
    // #167: document's concrete interface is HTMLDocument — a real
    // global class (site SDKs reference it by name; Alibaba's AWSC
    // nc-slider init died on `HTMLDocument is not defined` and its
    // punish-page track stayed 0-wide), with document still instanceof
    // Document, and construction illegal like Chrome's.
    let html_doc = rt
            .evaluate(
                r#"(() => {
                    let threw = false;
                    try { new HTMLDocument(); } catch (e) { threw = e instanceof TypeError; }
                    const d = Object.getOwnPropertyDescriptor(window, 'HTMLDocument');
                    return JSON.stringify({
                        t: typeof HTMLDocument,
                        instDoc: document instanceof HTMLDocument,
                        instSuper: document instanceof Document,
                        name: document.constructor.name,
                        chain: Object.getPrototypeOf(Object.getPrototypeOf(document)) === Document.prototype,
                        threw,
                        ne: d ? d.enumerable === false : false,
                        xml: new DOMParser().parseFromString('<a/>', 'text/xml') instanceof HTMLDocument,
                    });
                })()"#,
            )
            .unwrap();
    assert_eq!(
        html_doc,
        serde_json::json!(
            r#"{"t":"function","instDoc":true,"instSuper":true,"name":"HTMLDocument","chain":true,"threw":true,"ne":true,"xml":false}"#
        )
    );
    // A patch on one interface's prototype reaches its own tags only.
    let isolation = rt
        .evaluate(
            r#"(() => {
                    HTMLDivElement.prototype._probe = 'div';
                    Element.prototype._probe2 = 'all';
                    const d = document.getElementById('d');
                    const s = document.getElementById('s');
                    return JSON.stringify([d._probe, s._probe === undefined, d._probe2, s._probe2]);
                })()"#,
        )
        .unwrap();
    assert_eq!(isolation, serde_json::json!(r#"["div",true,"all","all"]"#));
}

/// #27 (obscura#999 face): Web IDL operations are enumerable — zone.js
/// patchClass() discovers methods with `for (prop in instance)` and only
/// walks enumerable properties, so non-enumerable class methods made
/// Angular die with "n.observe is not a function".
#[test]
fn interface_operations_are_enumerable() {
    let mut rt = setup_runtime(r#"<body><div id="d"></div></body>"#);
    let checks: &[(&str, bool)] = &[
            (
                "Object.getOwnPropertyDescriptor(MutationObserver.prototype, 'observe').enumerable",
                true,
            ),
            (
                "Object.getOwnPropertyDescriptor(Element.prototype, 'getAttribute').enumerable",
                true,
            ),
            (
                "Object.getOwnPropertyDescriptor(Node.prototype, 'appendChild').enumerable",
                true,
            ),
            // The zone.js patchClass discovery pattern: for-in over an
            // instance must surface the interface operations as functions.
            (
                "(() => { const mo = new MutationObserver(function(){}); let saw = 0; \
                  for (const k in mo) { if (k === 'observe' && typeof mo[k] === 'function') saw++; } \
                  return saw === 1; })()",
                true,
            ),
            // window.constructor.prototype members (WindowProxy face) stay out
            // of scope: Object.prototype built-ins must NOT become enumerable.
            (
                "Object.getOwnPropertyDescriptor(Object.prototype, 'hasOwnProperty').enumerable",
                false,
            ),
        ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
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

/// #28: SVG elements wrap in their real interfaces (constructor.name,
/// instanceof, patch isolation) and read the true per-node namespace from
/// the tree — parsed SVG children inherit theirs, createElementNS keeps
/// the XML tag case, and svg/HTML <a> discriminate by namespace.
#[test]
fn svg_interface_family_and_namespace() {
    let mut rt = setup_runtime(
        r#"<body><svg id="sv"><path id="pa"></path><linearGradient id="lg"></linearGradient><a id="sa"></a></svg><a id="ha" href="/h">h</a></body>"#,
    );
    let checks: &[(&str, bool)] = &[
            // Chrome chain: SVGSVGElement → SVGGraphicsElement → SVGElement
            // → HTMLElement → Element (Blink routes SVG through HTMLElement).
            ("document.getElementById('sv') instanceof SVGSVGElement", true),
            ("document.getElementById('sv') instanceof SVGGraphicsElement", true),
            ("document.getElementById('sv') instanceof SVGElement", true),
            // (刀72) Chrome 145: the SVG chain has no HTMLElement level (the #28 claim was wrong).
            ("document.getElementById('sv') instanceof HTMLElement", false),
            ("document.getElementById('sv') instanceof Element", true),
            ("document.getElementById('pa') instanceof SVGGeometryElement", true),
            // svg is graphics but NOT geometry; shapes are both.
            ("document.getElementById('sv') instanceof SVGGeometryElement", false),
            // Namespace discrimination: svg <a> vs HTML <a> never collide.
            ("document.getElementById('sa') instanceof SVGAElement", true),
            ("document.getElementById('sa') instanceof HTMLAnchorElement", false),
            ("document.getElementById('ha') instanceof HTMLAnchorElement", true),
            ("document.getElementById('ha') instanceof SVGAElement", false),
            // The old bare alias made EVERY node instanceof SVGSVGElement.
            ("document.body instanceof SVGSVGElement", false),
            ("SVGSVGElement !== Element", true),
            (
                "Object.getPrototypeOf(SVGSVGElement.prototype) === SVGGraphicsElement.prototype",
                true,
            ),
            (
                "Object.getPrototypeOf(SVGGraphicsElement.prototype) === SVGElement.prototype",
                true,
            ),
            // (刀72) Chrome 145: SVGElement sits directly on Element.
            (
                "Object.getPrototypeOf(SVGElement.prototype) === Element.prototype",
                true,
            ),
            // Parsed SVG children inherit the namespace from the tree; HTML
            // elements keep XHTML. Case: linearGradient keeps its camelCase.
            (
                "document.getElementById('pa').namespaceURI === 'http://www.w3.org/2000/svg'",
                true,
            ),
            (
                "document.getElementById('ha').namespaceURI === 'http://www.w3.org/1999/xhtml'",
                true,
            ),
            ("document.getElementById('lg').localName === 'linearGradient'", true),
            ("document.getElementById('lg').tagName === 'linearGradient'", true),
            // createElementNS: XML tag case preserved, namespace true.
            (
                "(() => { const c = document.createElementNS('http://www.w3.org/2000/svg', 'linearGradient'); \
                  return c.tagName === 'linearGradient' && c.namespaceURI === 'http://www.w3.org/2000/svg' \
                  && c instanceof SVGLinearGradientElement; })()",
                true,
            ),
            // Empty namespace string is the null namespace per spec.
            ("document.createElementNS('', 'foo').namespaceURI === null", true),
            // Class-body methods stay enumerable after the tail pass.
            (
                "Object.getOwnPropertyDescriptor(SVGGraphicsElement.prototype, 'getBBox').enumerable",
                true,
            ),
        ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
    let names = rt
        .evaluate(
            r#"JSON.stringify([
                    document.getElementById('sv').constructor.name,
                    document.getElementById('pa').constructor.name,
                    document.getElementById('lg').constructor.name,
                    document.getElementById('sa').constructor.name,
                    document.getElementById('ha').constructor.name,
                ])"#,
        )
        .unwrap();
    assert_eq!(
        names,
        serde_json::json!(
            r#"["SVGSVGElement","SVGPathElement","SVGLinearGradientElement","SVGAElement","HTMLAnchorElement"]"#
        )
    );
    // A patch on SVGSVGElement's prototype reaches the svg root only.
    let isolation = rt
        .evaluate(
            r#"(() => {
                    SVGSVGElement.prototype._probe = 'svg';
                    const sv = document.getElementById('sv');
                    return JSON.stringify([sv._probe, document.body._probe === undefined]);
                })()"#,
        )
        .unwrap();
    assert_eq!(isolation, serde_json::json!(r#"["svg",true]"#));
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
            serde_json::json!("undefined|undefined|undefined|undefined|undefined|undefined|undefined|undefined|function|object"),
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

// #15 (obscura#984): the DOM conversion trio must match Chrome —
// contains() self-short-circuits to true, isSameNode() is identity (not
// equality), and getElementById() coerces its argument to string (a
// numeric 42 finds id="42") with the empty needle returning null.
#[test]
fn dom_conversion_trio_matches_chrome() {
    let mut rt = setup_runtime(
        r#"<html><body><div id="p"><span id="c">x</span></div><div id="q">y</div><div id="42">z</div></body></html>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify([
                    document.getElementById('p').contains(document.getElementById('p')),
                    document.getElementById('p').contains(document.getElementById('c')),
                    document.getElementById('c').contains(document.getElementById('p')),
                    document.getElementById('p').contains(document.getElementById('q')),
                    document.getElementById('p').isSameNode(document.getElementById('p')),
                    document.getElementById('p').isSameNode(document.getElementById('q')),
                    document.getElementById('p').isSameNode(null),
                    document.getElementById(42) === document.getElementById('42'),
                    document.getElementById('') === null
                ])"#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!(r#"[true,true,false,false,true,false,false,true,true]"#)
    );
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

#[tokio::test(flavor = "current_thread")]
async fn read_only_read_write_face_matches_chrome() {
    // #219 / lightpanda #3718: :read-only/:read-write used to fall out
    // of the selector parser entirely (unknown pseudo → rule dropped,
    // matches() false). Chrome oracle: :read-write = editable text
    // controls (no readonly/disabled) or anything on the contenteditable
    // host chain — the host-chain walk is the lightpanda knife (input
    // inside a custom element resolves through its host); :read-only is
    // the negation over every element, plain divs included.
    let mut rt = setup_runtime(
        r#"<html><body>
<ul><li id=a1>1</li><li id=a2>2</li><li id=a3>3</li><li id=a4>4</li></ul>
<div id="host"><input id="plain"></div>
<input id="ro" readonly>
<input id="dis" disabled>
<input id="chk" type="checkbox">
<div id="ce" contenteditable><span id="inner">x</span></div>
<div id="cefalse" contenteditable="false"><span id="innerfalse">x</span></div>
</body></html>"#,
    );
    let script = r#"async () => {
            const m = (id, sel) => { try { return document.getElementById(id).matches(sel); } catch (e) { return 'throw: ' + e.message; } };
            return {
                nth: document.querySelectorAll('li:nth-child( 2n )').length,
                ro_plain: m('plain', ':read-only'), rw_plain: m('plain', ':read-write'),
                ro_ro: m('ro', ':read-only'), rw_ro: m('ro', ':read-write'),
                ro_dis: m('dis', ':read-only'), rw_dis: m('dis', ':read-write'),
                ro_chk: m('chk', ':read-only'), rw_chk: m('chk', ':read-write'),
                ro_div: m('host', ':read-only'), rw_div: m('host', ':read-write'),
                ro_ce: m('ce', ':read-only'), rw_ce: m('ce', ':read-write'),
                ro_inner: m('inner', ':read-only'), rw_inner: m('inner', ':read-write'),
                ro_cefalse: m('cefalse', ':read-only'), rw_innerfalse: m('innerfalse', ':read-write'),
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    // Whitespace-tolerant :nth-child arg (lightpanda #3722) — the
    // selectors crate parses `( 2n )` natively; pinned so a parser
    // regression can't slip through unnoticed.
    assert_eq!(
        v["nth"],
        serde_json::json!(2),
        "li:nth-child( 2n ) matches a2+a4"
    );
    assert_eq!(
        v["ro_plain"],
        serde_json::json!(false),
        "plain input is editable"
    );
    assert_eq!(v["rw_plain"], serde_json::json!(true));
    assert_eq!(v["ro_ro"], serde_json::json!(true), "readonly input");
    assert_eq!(v["rw_ro"], serde_json::json!(false));
    assert_eq!(
        v["ro_dis"],
        serde_json::json!(true),
        "disabled input is read-only"
    );
    assert_eq!(v["rw_dis"], serde_json::json!(false));
    assert_eq!(
        v["ro_chk"],
        serde_json::json!(true),
        "checkbox is never editable"
    );
    assert_eq!(v["rw_chk"], serde_json::json!(false));
    assert_eq!(
        v["ro_div"],
        serde_json::json!(true),
        "plain div is read-only"
    );
    assert_eq!(v["rw_div"], serde_json::json!(false));
    assert_eq!(v["ro_ce"], serde_json::json!(false), "editing host itself");
    assert_eq!(v["rw_ce"], serde_json::json!(true));
    assert_eq!(
        v["ro_inner"],
        serde_json::json!(false),
        "descendant of editing host"
    );
    assert_eq!(v["rw_inner"], serde_json::json!(true));
    assert_eq!(
        v["ro_cefalse"],
        serde_json::json!(true),
        "contenteditable=false is not an editing host"
    );
    assert_eq!(v["rw_innerfalse"], serde_json::json!(false));
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

/// 批240 / obscura#1100: getTotalLength/getPointAtLength through the
/// bootstrap SVGGeometryElement — Chrome's own numbers (headless probe
/// 2026-09-28), the Rust-side PathMeasure (conic chord measure for
/// ovals, analytic for explicit d beziers). getPointAtLength returns an
/// SVGPoint and clamps out-of-range distances to the path's ends.
#[test]
fn svg_geometry_measures_chrome_numbers() {
    let mut rt = setup_runtime(
        r#"<body><svg><path id="p2" d="M0,0 C50,100 100,0 100,100"></path><circle id="c" r="25"></circle></svg></body>"#,
    );
    let v = rt
        .evaluate(
            r#"JSON.stringify((() => {
                    const p2 = document.getElementById('p2');
                    const c = document.getElementById('c');
                    const mid = p2.getPointAtLength(p2.getTotalLength() / 2);
                    const over = p2.getPointAtLength(1e6);
                    const neg = p2.getPointAtLength(-1);
                    return {
                        p2Len: p2.getTotalLength(),
                        cLen: c.getTotalLength(),
                        midX: mid.x, midY: mid.y,
                        overX: over.x, overY: over.y,
                        negX: neg.x, negY: neg.y,
                        ptName: mid.constructor.name,
                        ptIs: mid instanceof SVGPoint,
                    };
                })())"#,
        )
        .unwrap();
    let d: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let close = |k: &str, want: f64, tol: f64| {
        let got = d[k].as_f64().unwrap();
        assert!(
            (got - want).abs() < tol,
            "{k}: got {got}, want {want} (diag={d})"
        );
    };
    close("p2Len", 160.71572875976562, 0.05);
    close("cLen", 156.0674285888672, 0.01);
    close("midX", 58.0898, 0.2);
    close("midY", 49.7112, 0.1);
    close("overX", 100.0, 0.01);
    close("overY", 100.0, 0.01);
    close("negX", 0.0, 1e-3);
    close("negY", 0.0, 1e-3);
    assert_eq!(d["ptName"], serde_json::json!("SVGPoint"));
    assert_eq!(
        d["ptIs"],
        serde_json::json!(true),
        "getPointAtLength returns an SVGPoint"
    );
}

/// #191: listeners are DOM §2.9 callbacks — `{handleEvent}` object listeners
/// are legal (github's webcomponents-era code registers four of them) and
/// were blind `.call()`ed at dispatch, producing "h.call is not a function".
/// The object form runs handleEvent with the LISTENER as `this`; null is a
/// legal inert registration; primitives are WebIDL violations that throw at
/// registration, exactly where Chrome throws them (message matches Chrome's).
#[test]
fn object_form_listeners_and_webidl_registration_gate() {
    let mut rt = setup_runtime("<html><body><button id='b'>x</button></body></html>");
    let js = r##"
        const b = document.getElementById('b');
        const log = [];
        const objListener = { handleEvent(e) { log.push('obj:' + (this === objListener) + ':' + e.type); } };
        b.addEventListener('click', function(e) { log.push('fn:' + e.type); });
        b.addEventListener('click', objListener);
        b.addEventListener('click', null);
        b.click();
        document.addEventListener('ping', objListener);
        document.dispatchEvent(new Event('ping'));
        window.addEventListener('pong', objListener);
        window.dispatchEvent(new Event('pong'));
        let threw = null;
        try { b.addEventListener('click', 5); } catch (e) { threw = e && e.name; }
        let wThrew = null;
        try { window.addEventListener('message', 'str'); } catch (e) { wThrew = e && e.name; }
        let dThrew = null;
        try { document.addEventListener('click', true); } catch (e) { dThrew = e && e.name; }
        ({ log, threw, wThrew, dThrew })
    "##;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "log": [
                "fn:click",
                "obj:true:click",
                "obj:true:ping",
                "obj:true:pong"
            ],
            "threw": "TypeError",
            "wThrew": "TypeError",
            "dThrew": "TypeError"
        })
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

/// #170: RTCPeerConnection is a branded WebIDL interface — a bare call
/// throws Chrome's DOM TypeError (not V8's class-constructor text), the
/// brand is [object RTCPeerConnection], and the whole RTC orbit carries
/// its own Symbol.toStringTag. The AWSC collection on the punish page
/// sniffs both faces; a plain ES class answers [object Object] + V8 raw
/// message.
#[test]
fn rtc_peer_connection_face_matches_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
            .evaluate(
                r#"(() => {
                    const out = {};
                    try { RTCPeerConnection(); out.bareCall = 'no-throw'; }
                    catch (e) { out.bareCall = e instanceof TypeError ? e.message : ('wrong:' + e); }
                    const pc = new RTCPeerConnection();
                    out.brand = Object.prototype.toString.call(pc);
                    out.instanceof = pc instanceof RTCPeerConnection;
                    out.ctorName = pc.constructor.name;
                    out.subclass = (() => {
                        class X extends RTCPeerConnection { tag() { return 'x'; } }
                        const x = new X();
                        return x instanceof X && x instanceof RTCPeerConnection && x.tag() === 'x';
                    })();
                    out.webkitBrand = Object.prototype.toString.call(new webkitRTCPeerConnection());
                    out.orbitTags = ['RTCSessionDescription', 'RTCDataChannel', 'RTCStatsReport', 'RTCRtpTransceiver']
                        .map((n) => globalThis[n].prototype[Symbol.toStringTag]);
                    try { new RTCDataChannel(); out.dataChannelCtor = 'no-throw'; }
                    catch (e) { out.dataChannelCtor = e.message; }
                    return JSON.stringify(out);
                })()"#,
            )
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
            v["bareCall"],
            serde_json::json!("Failed to construct 'RTCPeerConnection': Please use the 'new' operator, this DOM object constructor cannot be called as a function."),
            "bare call throws Chrome's DOM TypeError"
        );
    assert_eq!(
        v["brand"],
        serde_json::json!("[object RTCPeerConnection]"),
        "toString brand"
    );
    assert_eq!(
        v["instanceof"],
        serde_json::json!(true),
        "instanceof through the wrapper"
    );
    assert_eq!(
        v["ctorName"],
        serde_json::json!("RTCPeerConnection"),
        "constructor.name stays the interface name"
    );
    assert_eq!(
        v["subclass"],
        serde_json::json!(true),
        "extends still constructs through Reflect.construct"
    );
    assert_eq!(
        v["webkitBrand"],
        serde_json::json!("[object RTCPeerConnection]"),
        "webkit alias carries the brand"
    );
    assert_eq!(
        v["orbitTags"],
        serde_json::json!([
            "RTCSessionDescription",
            "RTCDataChannel",
            "RTCStatsReport",
            "RTCRtpTransceiver"
        ]),
        "RTC orbit carries per-interface tags"
    );
    assert_eq!(v["dataChannelCtor"], serde_json::json!("Failed to construct 'RTCDataChannel': Illegal constructor"), "satellite stubs still throw Illegal constructor (刀78: Chrome's message names the interface)");
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
async fn test_webidl_to_string_tag_brands() {
    // #203: every engine shim class used to answer '[object Object]', and
    // doudian's deep-clone helper treated that string as "plain data, walk
    // own keys" — it recursed into an Event (target→node→ownerDocument
    // back-edges) and died with RangeError, taking the whole publish app
    // down. Chrome brands every platform object with its interface name.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const ots = Object.prototype.toString;
            return [
                ots.call(new Event('x')),
                ots.call(new CustomEvent('x')),          // subclass: own tag, not Event's
                ots.call(new XMLHttpRequest()),
                ots.call(new WebSocket('ws://x')),        // no throw: brand read only
                ots.call(document.createElement('div')),
                ots.call(document.createElement('input')),
                ots.call(document),                       // '[object HTMLDocument]'
                ots.call(location),
                ots.call(console),
                ots.call(localStorage),
                ots.call(history),
                ots.call(performance),
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            "[object Event]",
            "[object CustomEvent]",
            "[object XMLHttpRequest]",
            "[object WebSocket]",
            "[object HTMLDivElement]",
            "[object HTMLInputElement]",
            "[object HTMLDocument]",
            "[object Location]",
            "[object console]",
            "[object Storage]",
            "[object History]",
            "[object Performance]",
        ])
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

#[tokio::test(flavor = "current_thread")]
async fn test_list_reflection_indeterminate_font_family_ec_jwk_order() {
    // Chrome truth for the 2026-10-22 sweep gaps (checked against a
    // local headless Chrome): ol.start/reversed/type, li.value,
    // :indeterminate / live :checked, fontFamily read-back, EC JWK keys.
    let mut rt = setup_runtime(
            "<html><body><ol id=o start=5 reversed type=a><li id=l1 value=9>a<li id=l2 value=x>b</ol>\
             <input type=checkbox id=cb><input type=checkbox id=cb2 checked><progress id=pg></progress></body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { cb.indeterminate = true; const ff = (s) => { const d = document.createElement('div'); d.style.fontFamily = s; return d.style.fontFamily; };
                  const r = { ol: [o.start, o.reversed, o.type, l1.value, l2.value].join(),
                    ind: [cb.matches(':indeterminate'), cb2.matches(':indeterminate'), pg.matches(':indeterminate')].join() };
                  cb.indeterminate = false; r.indOff = cb.matches(':indeterminate');
                  cb.checked = true; cb2.checked = false;
                  r.checked = [cb.matches(':checked'), cb2.matches(':checked')].join();
                  r.ff = [ff('Foo Bar, serif'), ff(\"'Menlo',monospace\"), ff('Arial'), ff('\"Times New Roman\", \"serif\"'), ff('a b, 1x')].join('|');
                  o.start = -3; o.reversed = false; r.set = [o.getAttribute('start'), o.hasAttribute('reversed')].join();
                  return r; })()",
            )
            .unwrap();
    assert_eq!(v["ol"], "5,true,a,9,0");
    assert_eq!(v["ind"], "true,false,true");
    assert_eq!(v["indOff"], false);
    assert_eq!(
        v["checked"], "true,false",
        ":checked follows the dirty checkedness"
    );
    assert_eq!(
        v["ff"], "\"Foo Bar\", serif|Menlo, monospace|Arial|\"Times New Roman\", \"serif\"|",
        "invalid unquoted family drops the whole declaration"
    );
    assert_eq!(v["set"], "-3,false");

    let keys = rt
            .evaluate_remote(
                "(async () => { const k = await crypto.subtle.generateKey({name:'ECDSA',namedCurve:'P-256'}, true, ['sign','verify']);
                   return Object.keys(await crypto.subtle.exportKey('jwk', k.privateKey)).join(); })()",
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(keys.value.unwrap(), "crv,d,ext,key_ops,kty,x,y");
}

#[tokio::test(flavor = "current_thread")]
async fn test_small_web_api_gaps_from_the_chrome_battery() {
    // Chrome truth (headless Chrome, 2026-10-22).
    let mut rt = setup_runtime("<html><body><div id=host></div></body></html>");
    let v = rt
            .evaluate_remote(
                "(async () => { const r = [];
                  r.push(typeof scheduler.postTask + typeof scheduler.yield);
                  r.push(await scheduler.postTask(() => 7, { priority: 'background' }));
                  r.push(typeof Blob.prototype.stream);
                  const rd = new Blob(['abc']).stream().getReader(); const c = await rd.read(); r.push(c.value.length);
                  r.push(new XMLSerializer().serializeToString(document.createElement('div')));
                  const d = document.createElement('div'); document.body.append(d);
                  const a = d.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 40 });
                  r.push(a.playState + document.getAnimations().length + d.getAnimations().length);
                  await a.finished; r.push(a.playState + document.getAnimations().length);
                  r.push(typeof document.startViewTransition + typeof document.queryCommandSupported + document.queryCommandSupported('bold'));
                  const vt = document.startViewTransition(() => { r.push('updated'); }); await vt.finished;
                  r.push(CSS.supports('caret-color', 'red') + ',' + CSS.supports('selector(::selection)') + ',' + CSS.supports('grid-template-columns', 'subgrid'));
                  return r.join('|'); })()",
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(
            v.value.unwrap(),
            "functionfunction|7|function|3|<div xmlns=\"http://www.w3.org/1999/xhtml\"></div>|running11|finished0|functionfunctiontrue|updated|true,true,false"
        );
}

/// JS-PLATFORM-ARCH M3: DOMException, EventTarget, Event and CustomEvent are
/// NATIVE interfaces (the Moli-direction binding layer). This runs the SAME
/// probe that produced chrome/event-truth-145.json in real Chrome 145 and
/// demands the engine match, probe by probe - except the known gaps below.
/// It is a RATCHET: a listed gap that starts matching must be removed from
/// the list. The Event-class migration (step C) emptied the list; new
/// entries may only be added for genuinely-new regressions.
#[test]
fn event_stack_matches_chrome_truth_except_known_event_gaps() {
    const KNOWN_EVENT_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/event-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/event-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        let known = KNOWN_EVENT_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_EVENT_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Chrome-version coherence (docs: CHROME-COHERENCE-AUDIT): the full version the engine
/// claims through userAgentData must be a build Google actually published. The old table
/// extrapolated builds ("145.0.6509.41" - build 6509 is Chrome 127's), a tell any detector
/// can check against the public version database.
#[tokio::test(flavor = "current_thread")]
async fn ua_data_full_version_is_a_real_chrome_build() {
    let real: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/cft-builds.json"
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

/// M4 knife 1: the geometry family (DOMPoint/DOMRect/DOMQuad/DOMMatrix*
/// and WebKitCSSMatrix) is generated shape (gen/geometry.gen.js) + behavior
/// module (geometry.impl.js). The probe runs against the live engine and is
/// diffed key-by-key against real Chrome 145 (captured via
/// tools/webplatform/capture/probe_run.py). KNOWN_GEOMETRY_GAPS is a
/// ratchet: an entry that starts matching must be removed.
///
/// ro.rotate45 (≤1 ulp): Blink's rotate path uses Chromium's own SIMD
/// sincos (third_party/simd), whose last-ulp rounding differs from
/// V8's Math.sin at some angles (observed 45° and 60°: sin 0.7071067811865475
/// vs ...476, 0.8660254037844386 vs ...387). The engine's Math.sin itself is
/// bit-identical to real Chrome's Math.sin — verified by capture — so this is
/// purely the C++ library's rounding, not reachable without emulating its
/// FMA-evaluated polynomial. cosSin() uses Math.sin/Math.cos.
#[test]
fn geometry_stack_matches_chrome_truth_except_known_geometry_gaps() {
    const KNOWN_GEOMETRY_GAPS: &[&str] = &["ro.rotate45"];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/geometry-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/geometry-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        let known = KNOWN_GEOMETRY_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_GEOMETRY_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 2: the HTMLElement/Element common-member layer. Chrome 145 owns
/// 102 on* + 38 members on HTMLElement.prototype, 8 on* on Element.prototype,
/// all 110 on* on Document.prototype; the engine bootstrap had 98 on* on
/// Element.prototype and a bare HTMLElement.prototype. The patch module
/// (gen/htmlelement.gen.js, generated from htmlelement-truth-145.json in
/// Chrome's own property order) relocates misplaced descriptors, installs the
/// missing members (uniform reflection = DATA via P.reflector; non-uniform
/// semantics in htmlelement.impl.js) and deletes engine on* extras Chrome
/// owns on no level. This test diffs the live engine against the same truth
/// file, key by key.
///
/// STAGED members are later family knives (the engine ledger, local-only)
/// and are
/// filtered out of the order/kinds comparisons. Knife 4 landed every staged
/// family, so the filter is empty. KNOWN_HTML_GAPS is a ratchet: an entry
/// that starts matching must be removed.
///
/// - fn.length: resolved at knife 4 (moveBefore/requestFullscreen installed
///   with Chrome's arity); the dict now matches in full.
/// - hasAttributeNS.cases: resolved (namespace-aware attribute storage) —
///   the engine now matches Chrome on the null-namespace lookups too.
#[test]
fn htmlelement_members_match_chrome_truth_except_known_gaps() {
    // Knife 4 landed all nine staged families (part/Typed OM/fullscreen
    // trio/pointer lock/moveBefore/ariaNotify — their behavior lives in
    // element_staged_members_match_chrome_truth); the filter stays as the
    // mechanism for any future staging.
    const STAGED: &[&str] = &[];
    const KNOWN_HTML_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/htmlelement-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/htmlelement-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        match k.as_str() {
            // HTMLElement own order IS the contract: Chrome's sequence minus
            // the staged names, byte for byte.
            "HT.order" => {
                let w: Vec<&str> = want
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .filter(|n| !STAGED.contains(n))
                    .collect();
                let g: Vec<&str> = got
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .filter(|n| !STAGED.contains(n))
                    .collect();
                if g != w {
                    let extra: Vec<&str> = g.iter().filter(|n| !w.contains(n)).copied().collect();
                    let miss: Vec<&str> = w.iter().filter(|n| !g.contains(n)).copied().collect();
                    let fd = g.iter().zip(w.iter()).position(|(a, b)| a != b);
                    bad.push(format!("HT.order: len {}/{} extra {extra:?} missing {miss:?} first-divergence {fd:?} (engine {:?}, chrome {:?})",
                        g.len(), w.len(),
                        fd.map(|i| g[i]).unwrap_or(""),
                        fd.map(|i| w[i]).unwrap_or("")));
                }
                continue;
            }
            "HT.kinds" => {
                for (n, kwant) in want.as_object().unwrap() {
                    if STAGED.contains(&n.as_str()) {
                        continue;
                    }
                    let kgot = &got[n];
                    if kgot != kwant {
                        bad.push(format!("HT.kinds.{n}: engine {kgot} chrome {kwant}"));
                    }
                }
                continue;
            }
            // Element still carries the staged per-element faces (later
            // family knives move them), so EL is a SUBSET contract: every
            // non-staged Chrome member must be own with the same kind.
            "EL.order" => {
                let g: Vec<&str> = got
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                for n in want.as_array().unwrap() {
                    let n = n.as_str().unwrap();
                    if !STAGED.contains(&n) && !g.contains(&n) {
                        bad.push(format!("EL.order: missing {n}"));
                    }
                }
                continue;
            }
            "EL.kinds" => {
                for (n, kwant) in want.as_object().unwrap() {
                    if STAGED.contains(&n.as_str()) {
                        continue;
                    }
                    let kgot = &got[n];
                    if kgot != kwant {
                        bad.push(format!("EL.kinds.{n}: engine {kgot} chrome {kwant}"));
                    }
                }
                continue;
            }
            _ => {}
        }
        let known = KNOWN_HTML_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_HTML_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 4: the Element staged family, landed. Chrome truth
/// (element-staged-truth-157.json, 38 keys; 157 re-truth: computed length 413->480) pins the nine members the
/// knife-2 ledger staged — part (cached DOMTokenList rw accessor),
/// attributeStyleMap/computedStyleMap (Typed OM; StylePropertyMap/
/// StylePropertyMapReadOnly owned by the stylemap module: ctor keeps
/// Chrome's Illegal-constructor throw, prototype members in Chrome's own
/// order, @@iterator = entries), the fullscreen trio (requestFullscreen
/// rejects TypeError "Permissions check failed"; the webkit pair returns
/// undefined), requestPointerLock (real Promise rejecting DOMException
/// WrongDocumentError), moveBefore (Element/Document/DocumentFragment
/// hosts, insertBefore-shaped move with Chrome's exact TypeError/
/// HierarchyRequestError/NotFoundError messages) and ariaNotify (len 1,
/// returns undefined). The probe settles its promise faces (fullscreen/
/// pointer-lock rejections race a 3s timeout so a hang can't wedge the
/// test) and is awaited through call_function_on_remote(await_promise).
///
/// KNOWN_GAPS ratchet: empty since M4 knife 54 (the computed-property list is Chrome's 413 longhands). csm.face is checked as the
/// INVARIANT computedStyleMap().size === getComputedStyle(el).length.
#[tokio::test(flavor = "current_thread")]
async fn element_staged_members_match_chrome_truth() {
    const KNOWN_GAPS: &[&str] = &[];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/element-staged-probe.js");
    let result = rt
        // call_function_on_remote invokes its argument as a FUNCTION
        // (__fn.call), so hand it the probe wrapped as an arrow returning
        // the probe's promise; await_promise settles it.
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/element-staged-truth-157.json"
    ))
    .unwrap();
    let eng_gcs = engine["gcs.len"].clone();
    let tru_gcs = truth["gcs.len"].clone();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        if k.as_str() == "csm.face" {
            let (g, w) = (got.as_array().unwrap(), want.as_array().unwrap());
            if !(g.len() == w.len()
                && g[0] == w[0]
                && g[1] == w[1]
                && g[2] == eng_gcs
                && w[2] == tru_gcs)
            {
                bad.push(format!(
                    "csm.face: engine {got} (own gcs {eng_gcs}) chrome {want} (own gcs {tru_gcs})"
                ));
            }
            continue;
        }
        let known = KNOWN_GAPS.contains(&k.as_str());
        if got != want && !known {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
        if got == want && known {
            bad.push(format!(
                "{k}: now matches Chrome - remove it from KNOWN_GAPS"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 6: the HTML reflected-attribute element family
/// (htmlreflect-probe.js). For 27 element interfaces — plus HTMLElement/
/// HTMLImageElement/HTMLVideoElement/HTMLMediaElement as regression rows for
/// the reflectors knives 2 and 5 landed — the probe records a BEHAVIOR
/// FINGERPRINT per read-write member in real Chrome 145 (default read; reads
/// after 11 property sets; the content attribute the set created; reads after
/// 15 raw setAttribute values; a keyword-vocabulary sweep for normalizing
/// members) and the test diffs the engine's fingerprint against it member by
/// member, plus own-name presence, own ORDER of the names both sides have,
/// DOMTokenList supports() sets, instance routing (createElement(tag)
/// instanceof its interface) and the knife-2 string-conversion extras.
///
/// Reflector kinds are MEASURED, not remembered: gen/htmlreflect.gen.js is
/// generated from this same truth through tools/webplatform/gen/
/// reflect_classify.py (a mirror of core.js P.reflector that accepts a kind
/// only if it reproduces every facet). The fingerprint caught latent
/// deviations in knife 2/5 reflectors (null/undefined DOMString conversion,
/// translate's yes/no, preload's empty state, crossOrigin's unknown value,
/// img.longDesc being a URL, unsigned-long edge parsing, the 14 interfaces
/// whose createElement instances were bare HTMLElement).
///
/// KNOWN ratchets (not this family; each entry that starts matching must be
/// removed — the test fails on a stale entry):
/// - protocol (a/area): URL decomposition of an href-less anchor (':' vs '').
/// - HTMLElement editContext/innerText/outerText/style: EditContext and
///   CSSOM families (offsetHeight/offsetWidth differ only in the layout-backed
///   served engine, not in this unit runtime).
/// - Form.elements: HTMLFormControlsCollection default.
/// - Media buffered/played/seekable (TimeRanges), currentTime/playbackRate
///   (double conversion + NotSupportedError range): media state family.
/// - Script.innerText undefined: Trusted Types union conversion.
/// - extra *.sym: relocated native string setters do not throw on Symbol.
#[tokio::test(flavor = "current_thread")]
async fn htmlreflect_members_match_chrome_truth() {
    const KNOWN_FP: &[&str] = &[
        "HTMLAnchorElement.protocol",
        "HTMLAreaElement.protocol",
        "HTMLElement.innerText",
        "HTMLElement.outerText",
        "HTMLElement.style",
        "HTMLMediaElement.buffered",
        "HTMLMediaElement.currentTime",
        "HTMLMediaElement.played",
        "HTMLMediaElement.playbackRate",
        "HTMLMediaElement.seekable",
        "HTMLScriptElement.innerText",
        // knife 7 (form family) - state machinery that is not reflection:
        "HTMLInputElement.files",
        "HTMLInputElement.selectionEnd",
        "HTMLInputElement.selectionStart",
        "HTMLInputElement.type",
        "HTMLInputElement.value",
        "HTMLInputElement.valueAsDate",
        "HTMLTextAreaElement.selectionEnd",
        "HTMLTextAreaElement.selectionStart",
        "HTMLTextAreaElement.value",
    ];
    // Empty (2026-11-04): every previously-missing experimental name —
    // interestForElement, the iframe ad/privacy cluster, form/select/
    // template collections — now installs with Chrome's fingerprint.
    const KNOWN_MISSING: &[&str] = &[];
    // bootstrap {value} constants are non-configurable and cannot be
    // re-seated (knife 5); strip them from the order comparison
    const IMMOVABLE: &[&str] = &[
        "NETWORK_EMPTY",
        "NETWORK_IDLE",
        "NETWORK_LOADING",
        "NETWORK_NO_SOURCE",
        "HAVE_NOTHING",
        "HAVE_METADATA",
        "HAVE_CURRENT_DATA",
        "HAVE_FUTURE_DATA",
        "HAVE_ENOUGH_DATA",
        // HTMLTrackElement IDL constants: non-configurable, so reseat cannot move them
        "NONE",
        "LOADING",
        "LOADED",
        "ERROR",
    ];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/htmlreflect-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/htmlreflect-truth-145.json"
    ))
    .unwrap();

    let mut bad: Vec<String> = Vec::new();
    let names = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .map(|a| a.iter().map(|n| n.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    };
    let mut stale_known: Vec<&str> = Vec::new();
    let mut seen_fp_diff: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_missing: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (iface, t_order) in truth["order"].as_object().unwrap() {
        let t_names = names(t_order);
        let e_names = names(&engine["order"][iface]);
        if e_names.is_empty() {
            bad.push(format!("{iface}: interface missing in engine"));
            continue;
        }
        // 1. presence
        for n in &t_names {
            if !e_names.contains(n) {
                let key = format!("{iface}.{n}");
                if KNOWN_MISSING.contains(&key.as_str()) {
                    seen_missing.insert(key);
                } else {
                    bad.push(format!("{key}: Chrome owns it, engine does not"));
                }
            }
        }
        // 2. order of the names both sides own
        let strip = |v: &Vec<String>, other: &Vec<String>| -> Vec<String> {
            v.iter()
                .filter(|n| other.contains(n) && !IMMOVABLE.contains(&n.as_str()))
                .cloned()
                .collect()
        };
        let (to, eo) = (strip(&t_names, &e_names), strip(&e_names, &t_names));
        if to != eo {
            bad.push(format!(
                "{iface}: own order differs\n   engine {eo:?}\n   chrome {to:?}"
            ));
        }
        // 3. fingerprints
        for (m, t_fp) in truth["fp"][iface].as_object().unwrap() {
            let e_fp = &engine["fp"][iface][m];
            if e_fp.is_null() {
                continue; // absence already reported above
            }
            let key = format!("{iface}.{m}");
            if t_fp != e_fp {
                seen_fp_diff.insert(key.clone());
                if !KNOWN_FP.contains(&key.as_str()) {
                    // name the first differing facet
                    let mut facet = String::from("?");
                    for g in ["def", "attr"] {
                        if t_fp[g] != e_fp[g] {
                            facet = format!("{g}: chrome {} engine {}", t_fp[g], e_fp[g]);
                        }
                    }
                    for g in ["set", "fromAttr"] {
                        if let Some(o) = t_fp[g].as_object() {
                            for (k, tv) in o {
                                if &e_fp[g][k] != tv && facet == "?" {
                                    facet = format!("{g}[{k}]: chrome {tv} engine {}", e_fp[g][k]);
                                }
                            }
                        }
                    }
                    if facet == "?" && t_fp["vocab"] != e_fp["vocab"] {
                        facet = "vocab".to_string();
                    }
                    bad.push(format!("{key}: fingerprint differs ({facet})"));
                }
            }
        }
    }
    for k in KNOWN_FP {
        if !seen_fp_diff.contains(*k) {
            stale_known.push(k);
        }
    }
    for k in KNOWN_MISSING {
        if !seen_missing.contains(*k) {
            stale_known.push(k);
        }
    }
    for k in stale_known {
        bad.push(format!(
            "{k}: now matches Chrome - remove it from the KNOWN list"
        ));
    }

    // 4. supports() sets of every DOMTokenList member
    if truth["tokens"] != engine["tokens"] {
        bad.push(format!(
            "tokens (supports()) differ:\n   engine {}\n   chrome {}",
            engine["tokens"], truth["tokens"]
        ));
    }
    // 5. instance routing
    if truth["routing"] != engine["routing"] {
        for (k, tv) in truth["routing"].as_object().unwrap() {
            if &engine["routing"][k] != tv {
                bad.push(format!(
                    "routing {k}: engine {} chrome {tv}",
                    engine["routing"][k]
                ));
            }
        }
    }
    // 6. knife-2 string conversions (title/lang/accessKey): everything but
    //    the Symbol edge, which the relocated native setters do not throw on
    for (k, tv) in truth["extra"].as_object().unwrap() {
        for (case, want) in tv.as_object().unwrap() {
            if case == "sym" {
                continue;
            }
            if &engine["extra"][k][case] != want {
                bad.push(format!(
                    "extra {k}.{case}: engine {} chrome {want}",
                    engine["extra"][k][case]
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} divergence(s):\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// M4 knife 9: the Generic Sensor family (sensor-probe.js, real Chrome 145
/// headless): Sensor / Accelerometer / LinearAccelerationSensor /
/// GravitySensor / Gyroscope / OrientationSensor / Absolute- and
/// RelativeOrientationSensor / SensorErrorEvent. With no sensor hardware every
/// sensor constructs, start() asynchronously fires a TRUSTED SensorErrorEvent
/// (NotReadableError "Could not connect to a sensor") and nothing ever
/// activates; Magnetometer / AmbientLightSensor do not exist. The probe covers
/// shape and chain, options-dictionary conversion order and wording, brand
/// checks, populateMatrix argument checks, the start()/stop() event sequences
/// (twice -> one error, stop cancels, restart works) and SensorErrorEvent's
/// required `error`. IDL on* handlers are logged apart from listeners: the
/// engine core fires handlers first (a known core-level divergence).
#[tokio::test(flavor = "current_thread")]
async fn sensor_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/sensor-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/sensor-truth-145.json"
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

/// M4 knife 10: Web Notifications (notification-probe.js, 42 keys, real
/// Chrome 145 headless). Permission starts 'default'; the first
/// requestPermission() settles 'denied' and sticks (no prompt UI); a
/// Notification constructed without permission asynchronously fires a TRUSTED
/// plain `error` Event and never shows. Covers shape and statics order,
/// constructor errors, the NotificationOptions dictionary read in WebIDL order
/// with Chrome's wording and cross-member rules (silent+vibrate, renotify
/// needs a tag, actions only for persistent notifications), URL resolution of
/// icon/badge, unsigned-long-long timestamp wrap, frozen vibrate arrays,
/// structured-cloned data (DataCloneError text names the value) and the
/// requestPermission promise/callback/argument check. Must run in a FRESH
/// runtime: the live /eval path executes a script twice and would see the
/// permission already 'denied'.
#[tokio::test(flavor = "current_thread")]
async fn notification_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/notification-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/notification-truth-145.json"
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

/// M4 knife 11: Web Speech recognition + ClipboardChangeEvent
/// (speech-probe.js, 74 keys, real Chrome 145 headless). SpeechRecognition
/// constructs; start() asynchronously fires a trusted SpeechRecognitionErrorEvent
/// 'not-allowed' then a trusted plain 'end' (never 'start'); a second start()
/// throws InvalidStateError; stop()/abort() while started cancel the error and
/// leave only 'end'. The webkit* names are the same interface objects;
/// SpeechRecognitionResult/ResultList/Alternative are not exposed. Also pinned:
/// shapes and statics, grammar list (own index properties, data: URIs, float32
/// weights), SpeechRecognitionPhrase boost range, the event inits (error is a
/// free DOMString), SpeechRecognition.available()/install() option checks and
/// the on-device availability answers, and ClipboardChangeEvent(init) - no
/// type argument, BigInt changeId, a fresh frozen `types` array per read.
#[tokio::test(flavor = "current_thread")]
async fn speech_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/speech-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/speech-truth-145.json"
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

/// M4 knife 13: RTCIceCandidate (rtcice-probe.js, 72 keys, real Chrome 145).
/// The bootstrap class only stored `candidate`; Chrome parses the candidate-
/// attribute string into foundation/component/priority/address/protocol/port/
/// type/tcpType/relatedAddress/relatedPort, ALL-OR-NOTHING (any malformed token
/// leaves every parsed field null): single spaces only, lowercase `typ` and
/// type keywords, transport case-insensitive and normalized, port/priority
/// range checks, IPv6 bare addresses bracketed, ufrag/generation/... ignored,
/// raddr/rport defaulting each other ('' / 0), `usernameFragment` only from
/// the init. Pinned too: constructor validation (sdpMid and sdpMLineIndex both
/// null), the RTCIceCandidateInit conversions (unsigned short wrap) and toJSON
/// (null sdpMid/usernameFragment -> '', null sdpMLineIndex omitted).
#[tokio::test(flavor = "current_thread")]
async fn rtcice_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/rtcice-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/rtcice-truth-145.json"
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

/// M4 knife 14: the small-member sweep (smallmembers-probe.js, 51 keys, real
/// Chrome 145): CharacterData.nextElementSibling/previousElementSibling,
/// Text.assignedSlot, ProcessingInstruction.sheet, DocumentFragment.
/// childElementCount; HashChangeEvent / SubmitEvent / PopStateEvent /
/// ClipboardEvent (init dictionaries, exact conversion wording; SubmitEvent
/// also from the real form-submission path, carrying the submitter and
/// trusted); MessageEvent.initMessageEvent + userActivation; DataTransfer
/// setData/getData/clearData (format normalization, replace, frozen cached
/// `types`); RTCSessionDescription (read-write type/sdp, enum check, toJSON);
/// InputDeviceCapabilities. Shapes are compared as: same parent, every Chrome
/// member present, the members both sides have in the same relative order.
/// KNOWN: Text/Comment/ProcessingInstruction/DocumentFragment carry extra own
/// members (nodeName/nodeType/cloneNode/innerHTML/nodeValue - Chrome puts them
/// on Node.prototype) and DocumentFragment/ProcessingInstruction constructor
/// lengths differ: the node-level placement family.
#[tokio::test(flavor = "current_thread")]
async fn smallmembers_match_chrome_truth() {
    const KNOWN_LEN: &[&str] = &["DocumentFragment", "ProcessingInstruction"];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/smallmembers-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/smallmembers-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if k == "shape" {
            for (name, t) in want.as_object().unwrap() {
                let e = &engine["shape"][name];
                if e.is_string() || e.is_null() {
                    bad.push(format!("shape {name}: interface missing in engine"));
                    continue;
                }
                let names = |v: &serde_json::Value| -> Vec<String> {
                    v["order"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect()
                };
                let (tn, en) = (names(t), names(e));
                for n in &tn {
                    if !en.contains(n) {
                        bad.push(format!("shape {name}: Chrome owns {n}, engine does not"));
                    }
                }
                let common_t: Vec<_> = tn.iter().filter(|n| en.contains(n)).collect();
                let common_e: Vec<_> = en.iter().filter(|n| tn.contains(n)).collect();
                if common_t != common_e {
                    bad.push(format!("shape {name}: member order differs\n   engine {common_e:?}\n   chrome {common_t:?}"));
                }
                if t["parent"] != e["parent"] {
                    bad.push(format!(
                        "shape {name}: parent engine {} chrome {}",
                        e["parent"], t["parent"]
                    ));
                }
                if t["len"] != e["len"] && !KNOWN_LEN.contains(&name.as_str()) {
                    bad.push(format!(
                        "shape {name}: ctor length engine {} chrome {}",
                        e["len"], t["len"]
                    ));
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

/// M4 knife 19: IntersectionObserver + IntersectionObserverEntry (ioentry-probe.js,
/// 31 keys, real Chrome 145): the entry is a getter-only interface with an illegal
/// constructor and observer-delivered entries are real instances over
/// DOMRectReadOnly; the observer validates callback/options with exact wording,
/// normalizes rootMargin/scrollMargin to four tokens, keeps thresholds as sorted
/// float32 and returns a fresh frozen copy per read, calls back with
/// (entries, observer) and this === observer. The always-intersecting policy and
/// the re-fire schedule are unchanged.
#[tokio::test(flavor = "current_thread")]
async fn intersection_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/ioentry-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/ioentry-truth-145.json"
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

/// M4 knife 20: Document legacy/misc members (docmisc-probe.js, 36 keys, real Chrome 145):
/// the body-colour legacy attributes (reflect bgcolor/text/link/vlink/alink, null -> ''),
/// live anchors/embeds/plugins with stable identity (embeds === plugins), applets,
/// designMode normalization, idle fullscreen/pointer-lock faces whose rw setters ignore
/// writes, lastModified, no-op captureEvents/releaseEvents/clear, caretPositionFromPoint /
/// CaretPosition, the privacy promise members, FragmentDirective, relative member order.
/// Previously every one of these was absent, so `document.bgColor = 'red'` minted an own
/// data property on the document.
#[tokio::test(flavor = "current_thread")]
async fn docmisc_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/docmisc-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/docmisc-truth-145.json"
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

/// M4 knife 21: XPath 1.0 (xpath-probe.js, 583 keys, real Chrome 145): ~120 location
/// paths over every axis, ~370 scalar expressions and core functions (number formatting
/// to six significant digits, substring/round edge cases, comparison rules), result types
/// and their accessor errors, ~130 malformed expressions with exact wording, namespace
/// resolution (NamespaceError, resolver forms), unordered-vs-ordered result order,
/// iterator invalidation on mutation, XPathExpression / XPathEvaluator / createNSResolver.
/// KNOWN gap (engine, not XPath): documents made by DOMParser / createHTMLDocument /
/// createDocument are not Document instances here, so they carry no evaluate().
#[tokio::test(flavor = "current_thread")]
async fn xpath_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/xpath-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/xpath-truth-145.json"
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

/// M4 knife 51 - the HTMLCollection family against real Chrome 145 (coll-probe.js, 25 keys): HTMLCollection is a legacy platform
/// object (not an Array: indexed read-only props, non-enumerable named props, prototype `length`, only item/namedItem/@@iterator),
/// HTMLOptionsCollection (add/remove/selectedIndex/writable length) and HTMLFormControlsCollection (RadioNodeList on several matches).
#[tokio::test(flavor = "current_thread")]
async fn htmlcollection_family_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../tools/webplatform/chrome/coll-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/coll-truth-145.json"
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
            "../../../../tools/webplatform/chrome/window-face-probe.js"
        ))
        .unwrap();
    let engine: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let mut truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/webplatform/chrome/window-face-truth-157.json"
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
        "../../../../tools/webplatform/chrome/enum-truth-157.json"
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

/// M4 刀55: Element.prototype's own member set == Chrome 145's exactly
/// (shape-audit-truth-145.json, Element entry, string keys). The engine
/// historically reflected every HTML/SVG member onto Element.prototype
/// with localName guards ("the kitchen sink"): a <div> answered .value/
/// .rows/.href, for-in over any element exposed all of them, and Chrome
/// has none of it. The sink moved to the interfaces that own it (the
/// bootstrap _interfaceProps MOVE table + SVG homes in element_sink.js);
/// this pins the graveyard shut — a new Element member Chrome doesn't
/// have fails here, and belongs in element_sink.js's delete list instead.
#[tokio::test(flavor = "current_thread")]
async fn element_prototype_members_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate("JSON.stringify(Object.getOwnPropertyNames(Element.prototype).sort())")
        .unwrap();
    let engine: Vec<String> = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let chrome: Vec<&str> = [
        "after",
        "animate",
        "append",
        "ariaActiveDescendantElement",
        "ariaAtomic",
        "ariaAutoComplete",
        "ariaBrailleLabel",
        "ariaBrailleRoleDescription",
        "ariaBusy",
        "ariaChecked",
        "ariaColCount",
        "ariaColIndex",
        "ariaColIndexText",
        "ariaColSpan",
        "ariaControlsElements",
        "ariaCurrent",
        "ariaDescribedByElements",
        "ariaDescription",
        "ariaDetailsElements",
        "ariaDisabled",
        "ariaErrorMessageElements",
        "ariaExpanded",
        "ariaFlowToElements",
        "ariaHasPopup",
        "ariaHidden",
        "ariaInvalid",
        "ariaKeyShortcuts",
        "ariaLabel",
        "ariaLabelledByElements",
        "ariaLevel",
        "ariaLive",
        "ariaModal",
        "ariaMultiLine",
        "ariaMultiSelectable",
        "ariaNotify",
        "ariaOrientation",
        "ariaPlaceholder",
        "ariaPosInSet",
        "ariaPressed",
        "ariaReadOnly",
        "ariaRelevant",
        "ariaRequired",
        "ariaRoleDescription",
        "ariaRowCount",
        "ariaRowIndex",
        "ariaRowIndexText",
        "ariaRowSpan",
        "ariaSelected",
        "ariaSetSize",
        "ariaSort",
        "ariaValueMax",
        "ariaValueMin",
        "ariaValueNow",
        "ariaValueText",
        "assignedSlot",
        "attachShadow",
        "attributes",
        "before",
        "checkVisibility",
        "childElementCount",
        "children",
        "classList",
        "className",
        "clientHeight",
        "clientLeft",
        "clientTop",
        "clientWidth",
        "closest",
        "computedStyleMap",
        "constructor",
        "currentCSSZoom",
        "elementTiming",
        "firstElementChild",
        "getAnimations",
        "getAttribute",
        "getAttributeNS",
        "getAttributeNames",
        "getAttributeNode",
        "getAttributeNodeNS",
        "getBoundingClientRect",
        "getClientRects",
        "getElementsByClassName",
        "getElementsByTagName",
        "getElementsByTagNameNS",
        "getHTML",
        "hasAttribute",
        "hasAttributeNS",
        "hasAttributes",
        "hasPointerCapture",
        "id",
        "innerHTML",
        "insertAdjacentElement",
        "insertAdjacentHTML",
        "insertAdjacentText",
        "lastElementChild",
        "localName",
        "matches",
        "moveBefore",
        "namespaceURI",
        "nextElementSibling",
        "onbeforecopy",
        "onbeforecut",
        "onbeforepaste",
        "onfullscreenchange",
        "onfullscreenerror",
        "onsearch",
        "onwebkitfullscreenchange",
        "onwebkitfullscreenerror",
        "outerHTML",
        "part",
        "prefix",
        "prepend",
        "previousElementSibling",
        "querySelector",
        "querySelectorAll",
        "releasePointerCapture",
        "remove",
        "removeAttribute",
        "removeAttributeNS",
        "removeAttributeNode",
        "replaceChildren",
        "replaceWith",
        "requestFullscreen",
        "requestPointerLock",
        "role",
        "scroll",
        "scrollBy",
        "scrollHeight",
        "scrollIntoView",
        "scrollIntoViewIfNeeded",
        "scrollLeft",
        "scrollTo",
        "scrollTop",
        "scrollWidth",
        "setAttribute",
        "setAttributeNS",
        "setAttributeNode",
        "setAttributeNodeNS",
        "setHTMLUnsafe",
        "setPointerCapture",
        "shadowRoot",
        "slot",
        "tagName",
        "toggleAttribute",
        "webkitMatchesSelector",
        "webkitRequestFullScreen",
        "webkitRequestFullscreen",
    ]
    .to_vec();
    let extra: Vec<&String> = engine
        .iter()
        .filter(|k| !chrome.contains(&k.as_str()))
        .collect();
    let missing: Vec<&&str> = chrome
        .iter()
        .filter(|k| !engine.contains(&k.to_string()))
        .collect();
    assert!(
        extra.is_empty() && missing.is_empty(),
        "Element.prototype member set drifted from Chrome 145: extra {extra:?} missing {missing:?}"
    );
}
