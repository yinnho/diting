//! stealth 指纹/persona/intl/下划线全局卫生/function toString
use super::super::*;
use super::common::setup_runtime;

/// The hardware persona (screen/dpr/GPU/canvas) is drawn from a seed the
/// host owns: the realm is rebuilt per navigation and __diting_init
/// self-deletes after drawing, so a Page re-pins the same seed via
/// set_fingerprint_seed on every fresh realm — a real machine does not
/// change its screen or GPU between pages of one visit (the identity
/// flipping per page is its own automation tell).
#[test]
fn fingerprint_seed_pins_hardware_identity_across_realms() {
    let probe = "JSON.stringify([screen.width, screen.height, devicePixelRatio, \
                     navigator.hardwareConcurrency])";
    let id_a = {
        let mut a = setup_runtime("<html><body></body></html>");
        a.set_fingerprint_seed(0x5EED_0001);
        a.evaluate(probe).unwrap()
    };
    let id_b = {
        let mut b = setup_runtime("<html><body></body></html>");
        b.set_fingerprint_seed(0x5EED_0001);
        let v = b.evaluate(probe).unwrap();
        assert_eq!(
            b.evaluate("_fpSeed").unwrap(),
            serde_json::json!(0x5EED_0001u64 as f64),
            "the pinned seed lands verbatim and drives the persona draws"
        );
        v
    };
    assert_eq!(
        id_a, id_b,
        "same seed must reproduce the same machine persona in a fresh realm"
    );
}

/// obscura#734 lineage: Intl's default locale must follow the configured
/// language source, not the process locale. Two layers keep them agreed:
/// set_language pins ICU's default (fresh isolates), and bootstrap.js
/// binds undefined locale args to `__diting_lang` per call (V8 caches the
/// resolved default per-isolate after first Intl use, so a re-pin alone
/// can't refresh an existing isolate). A three-way (Intl / navigator /
/// Accept-Language) mismatch is a hard headless tell.
#[test]
fn set_language_keeps_intl_and_navigator_locale_agreed() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_language("zh-CN,zh;q=0.9,en;q=0.8");
    let locale = rt
        .evaluate("Intl.DateTimeFormat().resolvedOptions().locale")
        .unwrap();
    assert_eq!(
        locale,
        serde_json::json!("zh-CN"),
        "Intl follows the configured language"
    );
    let nav = rt.evaluate("navigator.language").unwrap();
    assert_eq!(
        nav,
        serde_json::json!("zh-CN"),
        "navigator.language agrees with Intl"
    );
    let num = rt
        .evaluate("Intl.NumberFormat().resolvedOptions().locale")
        .unwrap();
    assert_eq!(
        num,
        serde_json::json!("zh-CN"),
        "generic Intl wrappers bind too"
    );
    // And back the other way on the SAME isolate: the bootstrap binding
    // reads `__diting_lang` per call, so this flips even though V8 has
    // already cached an ICU default for the isolate.
    rt.set_language("en-US,en;q=0.9");
    let locale = rt
        .evaluate("Intl.DateTimeFormat().resolvedOptions().locale")
        .unwrap();
    assert_eq!(locale, serde_json::json!("en-US"));
    let nav = rt.evaluate("navigator.language").unwrap();
    assert_eq!(nav, serde_json::json!("en-US"));
}

/// obscura#777 class: the CDP acceptLanguage override seeds the persona
/// on a LIVE isolate (set_navigator_language), which must move navigator
/// and Intl's undefined-locale binding while leaving the process-global
/// ICU pin alone — set_default_locale is not per-isolate, so re-pinning
/// mid-session would contaminate sibling isolates (obscura #778 hazard).
#[test]
fn set_navigator_language_moves_persona_without_repinning_icu() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_language("zh-CN,zh;q=0.9,en;q=0.8");
    assert_eq!(
        rt.evaluate("navigator.language").unwrap(),
        serde_json::json!("zh-CN")
    );
    // Mid-session persona move via the CDP acceptLanguage path.
    rt.set_navigator_language("fr-FR,fr;q=0.9");
    assert_eq!(
        rt.evaluate("navigator.language").unwrap(),
        serde_json::json!("fr-FR")
    );
    assert_eq!(
        rt.evaluate("navigator.languages.join('|')").unwrap(),
        serde_json::json!("fr-FR|fr")
    );
    // Intl follows through the bootstrap binding (reads __diting_lang per
    // call), so the persona stays coherent — same contract as set_language
    // minus the process-global ICU re-pin.
    let locale = rt
        .evaluate("Intl.DateTimeFormat().resolvedOptions().locale")
        .unwrap();
    assert_eq!(locale, serde_json::json!("fr-FR"));
}

/// Issue #30: engine-internal `_`-prefixed state must be invisible to
/// enumerability. Chrome keeps DOM state in native slots —
/// `Object.keys(div)` is `[]` there — and `for..in` walkers like zone.js's
/// patchClass see only the interface operations. Our JS-implemented DOM
/// stores internals as own props; they used to enumerate on every
/// element, observer, and prototype (a one-line engine tell).
#[cfg(feature = "screenshot")]
#[test]
fn internal_underscore_state_is_not_enumerable() {
    let mut rt = setup_runtime(
        "<html><body><div id='d' class='x'>t</div><a id='l' rel='nofollow'>l</a></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var el = document.getElementById('d');
            el.classList.add('y');                 // lazy _classList
            var link = document.getElementById('l');
            void link.relList;                     // lazy _relList
            var mo = new MutationObserver(function(){});
            var ro = new ResizeObserver(function(){});
            var io = new IntersectionObserver(function(){});
            var forIn = [];
            for (var k in mo) forIn.push(k);       // zone.js patchClass walk
            return JSON.stringify([
                Object.keys(el),
                Object.keys(document.body),
                Object.keys(document.documentElement),
                Object.keys(mo), Object.keys(ro), Object.keys(io),
                Object.keys(new FileReader())
                    .filter(function(k){ return k[0] === '_'; }),
                Object.keys(new URL('http://x.test/a?b=c')),
                Object.keys(new Headers()),
                Object.keys(new URLSearchParams('a=b')),
                Object.keys(MutationObserver.prototype),
                Object.keys(ResizeObserver.prototype),
                Object.keys(IntersectionObserver.prototype),
                Object.keys(FileReader.prototype).filter(function(k){ return k[0] === '_'; }),
                forIn,
            ]);
        "#,
        )
        .unwrap();
    let faces: Vec<Vec<String>> = serde_json::from_str(out.as_str().unwrap()).unwrap();
    // All instance faces (0-9) hold no own ENUMERABLE props: that is the
    // Chrome face — for-in walkers (zone.js patchClass, Object.keys
    // fingerprints) see nothing. Chrome's `[]` comes from native slots;
    // our `_` internals still exist as non-enumerable own props (v1
    // boundary, issue #30), which only getOwnPropertyNames reveals.
    for (i, face) in faces.iter().take(10).enumerate() {
        assert!(face.is_empty(), "face {i} leaked internals: {face:?}");
    }
    // Prototypes expose the public operations only — no `_notify`,
    // `_fireFor`, `_read`, `_fire`, `_pull`, `_push`.
    for (i, face) in faces.iter().enumerate().skip(10).take(4) {
        let bad: Vec<&String> = face.iter().filter(|k| k.starts_with('_')).collect();
        assert!(
            bad.is_empty(),
            "prototype face {i} leaked internals: {bad:?}"
        );
    }
    // The zone.js walk still finds the interface operations (that's the
    // #27 face — ops must stay enumerable) and nothing else. Order is
    // Chrome's own-key truth (shape-audit-truth-145; re-probed on 156):
    // Blink installs the legacy observer ops disconnect-first, not the
    // IDL source order.
    assert_eq!(
        faces[14],
        vec!["disconnect", "observe", "takeRecords"],
        "for-in over a MutationObserver must see exactly the operations"
    );
}

/// Issue #31: public interface members must live on the PROTOTYPE as
/// enumerable accessor pairs — instances carry nothing. In Chrome
/// `Object.keys(new WebSocket(...))` is `[]` and
/// `Object.keys(WebSocket.prototype)` holds the 16 interface members
/// (attributes + four state constants + the two operations);
/// our shims used to put publics (url, readyState, onopen, ...) on the
/// instance as plain data props, so both faces fingerprinted wrong.
/// The constructors keep their `this.x = ...` writes — those now route
/// through the prototype setters into hidden `_x` fields.
#[cfg(feature = "screenshot")]
#[test]
fn public_members_live_on_the_prototype() {
    let mut rt = setup_runtime("<html><body><div id='t'>x</div></body></html>");
    let out = rt
        .evaluate(
            r#"
            var ws = new WebSocket('ws://127.0.0.1:1/unreachable');
            var rs = new ReadableStream({ start: function(c){ c.enqueue('a'); } });
            var wst = new WritableStream();
            var rLock = [];
            rLock.push(rs.locked);
            var rd = rs.getReader(); rLock.push(rs.locked);
            rd.releaseLock(); rLock.push(rs.locked);
            var wLock = [];
            wLock.push(wst.locked);
            var wr = wst.getWriter(); wLock.push(wst.locked);
            wr.releaseLock(); wLock.push(wst.locked);
            var fired = 0;
            ws.addEventListener('error', function(){ fired++; });
            // Chrome's dispatchEvent validates parameter 1 (plain objects
            // throw "parameter 1 is not of type 'Event'"); a synthetic
            // Event is the Chrome-legal way to fire the listener.
            ws.dispatchEvent(new Event('error'));
            return JSON.stringify({
                inst: [
                    Object.keys(new FileReader()),
                    Object.keys(ws),
                    Object.keys(new EventSource('/x')),
                    Object.keys(new BroadcastChannel('t')),
                    Object.keys(new ReadableStream()),
                    Object.keys(wst),
                ],
                wsProto: Object.keys(WebSocket.prototype),
                rLock: rLock, wLock: wLock,
                wsWrite: (function(){ ws.readyState = 3; return ws.readyState; })(),
                fired: fired,
                desc: (function(){
                    var d = Object.getOwnPropertyDescriptor(
                        WebSocket.prototype, 'readyState');
                    // [hasGet, hasSet, enumerable] — Chrome pins a readonly
                    // IDL attribute as a getter-only enumerable pair, and
                    // assignment is silently ignored (wsWrite stays 0).
                    return [!!(d && d.get), !!(d && d.set), !!d && d.enumerable];
                })(),
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    // All six instance faces are empty — the Chrome face.
    for (i, face) in v["inst"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            face,
            &serde_json::json!([]),
            "instance face {i} must be [] (publics belong on the prototype)"
        );
    }
    // WebSocket.prototype carries exactly Chrome's 16 members (probed:
    // attributes in IDL order, the four state constants, then the two
    // operations; the EventTarget trio lives on EventTarget.prototype).
    assert_eq!(
        v["wsProto"],
        serde_json::json!([
            "url",
            "readyState",
            "bufferedAmount",
            "onopen",
            "onerror",
            "onclose",
            "extensions",
            "protocol",
            "onmessage",
            "binaryType",
            "CONNECTING",
            "OPEN",
            "CLOSING",
            "CLOSED",
            "close",
            "send",
        ]),
        "WebSocket.prototype member set must match Chrome"
    );
    // Lock semantics survive the prototype-getter conversion.
    assert_eq!(v["rLock"], serde_json::json!([false, true, false]));
    assert_eq!(v["wLock"], serde_json::json!([false, true, false]));
    // readyState is readonly (Chrome): the public write is ignored.
    assert_eq!(v["wsWrite"], serde_json::json!(0));
    // Prototype listener methods actually fire.
    assert_eq!(v["fired"], serde_json::json!(1));
    // Getter-only enumerable accessor pair on the prototype (Chrome).
    assert_eq!(v["desc"], serde_json::json!([true, false, true]));
}

/// obscura#734 follow-up: replacing a native Intl constructor with a
/// plain JS function is itself a fingerprint — Function.prototype.toString
/// stops returning [native code], name/length drift, and
/// prototype.constructor stops closing on Intl.X. The wrappers are
/// proxies with only a construct trap, plus a toString disguise for the
/// one gap V8's proxy source-text resolution leaves (the anonymous
/// "function () { [native code] }" form, without the name).
#[test]
fn intl_wrappers_render_native_identity() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let native_str = rt
        .evaluate(
            "(function(){\
                 \nreturn [Function.prototype.toString.call(Intl.NumberFormat),\
                 \nString(Intl.DateTimeFormat),\
                 \nFunction.prototype.toString.call(Intl.DateTimeFormat.prototype.resolvedOptions)]\
                 \n.join('|')})()",
        )
        .unwrap();
    let s = native_str.as_str().unwrap();
    for part in s.split('|') {
        assert!(
            part.contains("[native code]"),
            "expected native code form, got: {}",
            part
        );
    }
    assert!(
        s.contains("NumberFormat"),
        "toString should carry the constructor name"
    );
    assert!(
        s.contains("resolvedOptions"),
        "resolvedOptions toString should carry its name"
    );
    assert!(!s.contains("Wrapped"), "wrapper function name leaked");

    let ident = rt
        .evaluate(
            "[Intl.NumberFormat.name, Intl.DateTimeFormat.name,\
                 \nIntl.NumberFormat.length, Intl.DateTimeFormat.length,\
                 \nIntl.NumberFormat.prototype.constructor === Intl.NumberFormat,\
                 \nIntl.DateTimeFormat.prototype.constructor === Intl.DateTimeFormat,\
                 \n(new Intl.NumberFormat()) instanceof Intl.NumberFormat].join('|')",
        )
        .unwrap();
    assert_eq!(
        ident,
        serde_json::json!("NumberFormat|DateTimeFormat|0|0|true|true|true"),
        "name/length/constructor identity must match stock"
    );

    // The toString disguise must not eat real source: plain functions
    // still render their own text, and the disguise itself renders
    // native (spec resolves a callable proxy's source from its target).
    let passthrough = rt
            .evaluate(
                "(function(){var f=function foo(a){return a+1};\
                 \nreturn [f.toString().indexOf('return a+1')>=0,\
                 \nFunction.prototype.toString.toString().indexOf('[native code]')>=0].join('|')})()",
            )
            .unwrap();
    assert_eq!(passthrough, serde_json::json!("true|true"));
}

/// obscura#734 (WorkOS differ follow-up): a proxy trap inserts one extra
/// frame into error stacks that stock does not have, and V8 labels it
/// after the handler's constructor and the trap key. A plain-object
/// handler reads "Object.construct"; the handler carrier is named after
/// the wrapped constructor so the frame reads "NumberFormat.construct" —
/// a relabel, since the frame itself is inherent to trapping in JS.
#[test]
fn intl_wrapper_trap_frames_carry_the_wrapped_name() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let stack = rt
        .evaluate(
            "(function(){\
                 \ntry { new Intl.NumberFormat('en-US', {localeMatcher: 'bogus'}); }\
                 \ncatch (e) { return e.stack; }\
                 \nreturn 'no-throw';\
                 \n})()",
        )
        .unwrap();
    let s = stack.as_str().expect("stack string");
    assert!(
        s.contains("NumberFormat.construct"),
        "trap frame should carry the wrapped constructor name, got:\n{}",
        s
    );
    assert!(
        !s.contains("Object.construct"),
        "plain-object handler label leaked, stack:\n{}",
        s
    );
}

#[test]
fn test_stealth_fingerprint_apis_pluginarray_and_webgl() {
    // authk.smithery.ai (WorkOS+Cloudflare) crashed after hydration:
    // `ReferenceError: PluginArray is not defined` (bot-detector references
    // the constructor) and `e.uniform2f is not a function` (missing WebGL
    // methods). These must exist and behave like real browsers.
    let mut rt = setup_runtime("<html><body><canvas id='c'></canvas></body></html>");
    let res = rt.evaluate(r#"
            const out = {};
            out.pluginArrayDefined = typeof PluginArray !== 'undefined';
            out.pluginsIsInstance = navigator.plugins instanceof PluginArray;
            out.pluginsLength = navigator.plugins.length;
            out.pluginsIdentity = navigator.plugins === navigator.plugins;
            out.mimeIdentity = navigator.mimeTypes === navigator.mimeTypes;
            out.pluginLength = navigator.plugins[0] && navigator.plugins[0].length;
            out.mimeIsInstance = navigator.mimeTypes instanceof MimeTypeArray;
            // (#125) Chrome's PDF persona is a linked graph, verified against
            // a real headless Chrome 145 dump: each plugin carries both mime
            // types as members (length 2), item()/namedItem() resolve them,
            // every mime's enabledPlugin IS navigator.plugins[0] by identity,
            // and the member MimeType objects are the SAME instances
            // navigator.mimeTypes exposes.
            out.p00 = navigator.plugins[0][0] && navigator.plugins[0][0].type;
            out.p01 = navigator.plugins[0][1] && navigator.plugins[0][1].type;
            out.p4len = navigator.plugins[4] && navigator.plugins[4].length;
            out.pItem = navigator.plugins[0].item(0) && navigator.plugins[0].item(0).type;
            out.pNamed = navigator.plugins[0].namedItem("text/pdf") && navigator.plugins[0].namedItem("text/pdf").type;
            out.mt0Owner = navigator.mimeTypes[0].enabledPlugin && navigator.mimeTypes[0].enabledPlugin.name;
            out.mt1Owner = navigator.mimeTypes[1] && navigator.mimeTypes[1].enabledPlugin && navigator.mimeTypes[1].enabledPlugin.name;
            out.mt0OwnerIsP0 = navigator.mimeTypes[0].enabledPlugin === navigator.plugins[0];
            out.p00IsMt0 = navigator.plugins[0][0] === navigator.mimeTypes[0];
            out.p0keys = Object.keys(navigator.plugins[0]);
            const c = document.getElementById('c');
            const gl = c.getContext('webgl');
            out.gl = !!gl;
            out.glIdentity = c.getContext('webgl') === gl;
            out.glInstanceof = gl instanceof WebGLRenderingContext;
            out.gl2Instanceof = document.createElement('canvas').getContext('webgl2') instanceof WebGL2RenderingContext;
            out.glNotThenable = gl.then === undefined;
            out.glSymbolUndefined = gl[Symbol.iterator] === undefined;
            out.uniform2f = typeof gl.uniform2f === 'function';
            out.getContextAttributes = typeof gl.getContextAttributes === 'function' && !!gl.getContextAttributes();
            out.getError = gl.getError() === 0;
            out.unknownMethod = (function(){ try { return typeof gl.someUnknownMethod === 'undefined' && (function(){ try { gl.someUnknownMethod(1,2); return false; } catch(e) { return e instanceof TypeError; } })(); } catch(e) { return 'threw:' + e.message; } })();
            return JSON.stringify(out);
        "#).unwrap();
    let v = serde_json::from_str::<serde_json::Value>(res.as_str().unwrap()).unwrap();
    assert_eq!(
        v["pluginArrayDefined"], true,
        "PluginArray global must exist"
    );
    assert_eq!(
        v["pluginsIsInstance"], true,
        "navigator.plugins must be a PluginArray"
    );
    assert_eq!(v["pluginsLength"].as_i64().unwrap(), 5);
    assert_eq!(
        v["pluginsIdentity"], true,
        "plugins must be a cached singleton (identity is fingerprintable)"
    );
    assert_eq!(
        v["mimeIdentity"], true,
        "mimeTypes must be a cached singleton"
    );
    assert_eq!(
        v["pluginLength"].as_i64().unwrap(),
        2,
        "Chrome PDF plugins carry both mime types"
    );
    assert_eq!(v["mimeIsInstance"], true);
    assert_eq!(
        v["p00"], "application/pdf",
        "plugins[0][0] must expose the member MimeType"
    );
    assert_eq!(
        v["p01"], "text/pdf",
        "plugins[0][1] must expose the second member MimeType"
    );
    assert_eq!(
        v["p4len"].as_i64().unwrap(),
        2,
        "every PDF plugin carries both members"
    );
    assert_eq!(
        v["pItem"], "application/pdf",
        "Plugin.item(0) must resolve the member"
    );
    assert_eq!(
        v["pNamed"], "text/pdf",
        "Plugin.namedItem must resolve by mime type"
    );
    assert_eq!(
        v["mt0Owner"], "PDF Viewer",
        "mimeTypes[0].enabledPlugin must name the owning plugin"
    );
    assert_eq!(
        v["mt1Owner"], "PDF Viewer",
        "text/pdf is also owned by the first plugin"
    );
    assert_eq!(
        v["mt0OwnerIsP0"], true,
        "enabledPlugin must be navigator.plugins[0] by identity"
    );
    assert_eq!(
        v["p00IsMt0"], true,
        "plugin members and navigator.mimeTypes share MimeType instances"
    );
    assert_eq!(
        v["p0keys"],
        serde_json::json!(["0", "1"]),
        "Object.keys(plugin) shows member indices only (Chrome puts name/length on the prototype)"
    );
    assert_eq!(v["gl"], true);
    assert_eq!(
        v["glIdentity"], true,
        "getContext must return the same context on repeat calls"
    );
    assert_eq!(
        v["glInstanceof"], true,
        "gl must be instanceof WebGLRenderingContext"
    );
    assert_eq!(
        v["gl2Instanceof"], true,
        "webgl2 context must be instanceof WebGL2RenderingContext"
    );
    assert_eq!(
        v["glNotThenable"], true,
        "gl.then must stay undefined or the context becomes thenable"
    );
    assert_eq!(
        v["glSymbolUndefined"], true,
        "symbol props must not hit the numNoop fallback"
    );
    assert_eq!(v["uniform2f"], true, "uniform2f must exist");
    assert_eq!(v["getContextAttributes"], true);
    assert_eq!(v["getError"], true);
    assert_eq!(
        v["unknownMethod"], true,
        "(刀69) a name Chrome lacks is undefined and calling it is a TypeError — the number-ish Proxy fallback is gone"
    );
}

#[test]
fn test_stealth_webgl_chrome_truth_interface_shape() {
    // (P8 #300015) The WebGL interface must match Chrome 154 byte-for-byte
    // at the reflection level: the INSTANCE carries zero own properties,
    // the PROTOTYPE carries exactly 442/791 own names (constants +
    // methods + 6 accessors), constants are enumerable/non-writable/
    // non-configurable, and extension objects keep their constants on the
    // carrier prototype. The old shape leaked ~80 own props on the
    // instance and a 29-entry constant table — a counting detector's
    // one-liner. Truth captured from real Chrome 154.0.8037.98 (macOS);
    // 154 lacks makeXRCompatible (155 added it), hence -1 vs the 155 pin.
    let mut rt = setup_runtime(
        "<html><body><canvas id='c' width='320' height='200'></canvas></body></html>",
    );
    let res = rt.evaluate(r#"
            const out = {};
            const c = document.getElementById('c');
            const gl = c.getContext('webgl');
            const gl2 = document.createElement('canvas').getContext('webgl2');
            const P1 = WebGLRenderingContext.prototype;
            const P2 = WebGL2RenderingContext.prototype;
            out.instKeys1 = Object.keys(gl).length;
            out.instKeys2 = Object.keys(gl2).length;
            out.instNames1 = Object.getOwnPropertyNames(gl).length;
            out.protoNames1 = Object.getOwnPropertyNames(P1).length;
            out.protoNames2 = Object.getOwnPropertyNames(P2).length;
            out.protoKeys1 = Object.keys(P1).length;
            out.protoKeys2 = Object.keys(P2).length;
            const d = Object.getOwnPropertyDescriptor(P1, 'VERTEX_SHADER');
            out.constDesc = [d.value, d.writable, d.enumerable, d.configurable];
            const md = Object.getOwnPropertyDescriptor(P1, 'getParameter');
            out.methodDesc = [md.writable, md.enumerable, md.configurable, typeof md.get];
            out.ctorEnum = Object.getOwnPropertyDescriptor(P1, 'constructor').enumerable;
            out.acc = Object.getOwnPropertyDescriptor(P1, 'canvas');
            out.accIsGetter = typeof out.acc.get === 'function' && out.acc.set === undefined && !('value' in out.acc);
            out.canvasIsC = gl.canvas === c;
            out.dbw = gl.drawingBufferWidth;
            out.dbh = gl.drawingBufferHeight;
            out.dbcs = gl.drawingBufferColorSpace;
            out.ucs = gl.unpackColorSpace;
            out.dbf = gl.drawingBufferFormat;
            out.dbf2 = gl2.drawingBufferFormat;
            const ext = gl.getExtension('WEBGL_debug_renderer_info');
            out.extKeys = Object.keys(ext).length;
            out.extNames = Object.getOwnPropertyNames(ext).length;
            out.extConstIn = 'UNMASKED_VENDOR_WEBGL' in ext;
            out.extConstVal = ext.UNMASKED_VENDOR_WEBGL;
            out.extCtorName = ext.constructor && ext.constructor.name;
            out.extProtoProtoObject = Object.getPrototypeOf(Object.getPrototypeOf(ext)) === Object.prototype;
            const lose = gl.getExtension('WEBGL_lose_context');
            out.loseMethods = typeof lose.loseContext === 'function' && typeof lose.restoreContext === 'function';
            out.loseOwn = Object.keys(lose).length;
            out.gl2Methods = typeof gl2.createVertexArray === 'function' && typeof gl2.transformFeedbackVaryings === 'function';
            out.gl2Const = gl2.TEXTURE_BINDING_3D === 32874 && gl2.MAX_ELEMENT_INDEX === 36203;
            out.memPow2 = [8, 16, 32].includes(navigator.deviceMemory);
            out.memGt8Possible = navigator.deviceMemory >= 8;
            return JSON.stringify(out);
        "#).unwrap();
    let v = serde_json::from_str::<serde_json::Value>(res.as_str().unwrap()).unwrap();
    assert_eq!(
        v["instKeys1"], 0,
        "GL1 instance must carry ZERO own enumerable props (Chrome: [])"
    );
    assert_eq!(
        v["instKeys2"], 0,
        "GL2 instance must carry ZERO own enumerable props"
    );
    assert_eq!(
        v["instNames1"], 0,
        "GL1 instance must carry ZERO own props of any kind (only hidden _canvas)"
    );
    assert_eq!(
        v["protoNames1"].as_i64().unwrap(),
        443,
        "GL1 prototype own-name count must be exactly Chrome 145's 443 (298 consts + 138 methods + 6 accessors + makeXRCompatible)"
    );
    assert_eq!(
        v["protoNames2"].as_i64().unwrap(),
        792,
        "GL2 prototype own-name count must be exactly Chrome 145's 792 (559 + 226 + 6 + makeXRCompatible)"
    );
    assert_eq!(
        v["protoKeys1"].as_i64().unwrap(),
        442,
        "GL1 enumerable proto keys = 442 (constructor is non-enumerable)"
    );
    assert_eq!(
        v["protoKeys2"].as_i64().unwrap(),
        791,
        "GL2 enumerable proto keys = 791"
    );
    assert_eq!(
        v["constDesc"],
        serde_json::json!([35633, false, true, false]),
        "constants: value-carrying, enumerable, NON-writable, NON-configurable"
    );
    assert_eq!(
        v["methodDesc"],
        serde_json::json!([true, true, true, "undefined"]),
        "methods: writable+enumerable+configurable, not accessors"
    );
    assert_eq!(
        v["ctorEnum"], false,
        "constructor stays non-enumerable (the class default)"
    );
    assert_eq!(
        v["accIsGetter"], true,
        "canvas must be a getter-only accessor (no value, no set)"
    );
    assert_eq!(v["acc"]["enumerable"], true);
    assert_eq!(v["acc"]["configurable"], true);
    assert_eq!(
        v["canvasIsC"], true,
        "gl.canvas must be the creating canvas by identity"
    );
    assert_eq!(
        v["dbw"].as_i64().unwrap(),
        320,
        "drawingBufferWidth tracks the canvas width"
    );
    assert_eq!(
        v["dbh"].as_i64().unwrap(),
        200,
        "drawingBufferHeight tracks the canvas height"
    );
    assert_eq!(v["dbcs"], "srgb");
    assert_eq!(v["ucs"], "srgb");
    assert_eq!(
        v["dbf"].as_i64().unwrap(),
        32856,
        "drawingBufferFormat is the GLenum GL_RGBA8 (0x8058)"
    );
    assert_eq!(v["dbf2"].as_i64().unwrap(), 32856);
    assert_eq!(
        v["extKeys"], 0,
        "extension objects carry ZERO own enumerable props"
    );
    assert_eq!(
        v["extNames"], 0,
        "extension objects carry ZERO own props, period"
    );
    assert_eq!(
        v["extConstIn"], true,
        "the ext constant resolves through the carrier prototype"
    );
    assert_eq!(v["extConstVal"].as_i64().unwrap(), 0x9245);
    assert_eq!(
        v["extCtorName"], "Object",
        "Chrome ext objects report constructor name \"Object\""
    );
    assert_eq!(
        v["extProtoProtoObject"], true,
        "carrier chain ends at Object.prototype"
    );
    assert_eq!(
        v["loseMethods"], true,
        "WEBGL_lose_context carries loseContext/restoreContext"
    );
    assert_eq!(v["loseOwn"], 0);
    assert_eq!(
        v["gl2Methods"], true,
        "GL2-only methods must exist (createVertexArray, transformFeedbackVaryings)"
    );
    assert_eq!(
        v["gl2Const"], true,
        "GL2-only constants must resolve (TEXTURE_BINDING_3D)"
    );
    assert_eq!(
        v["memPow2"], true,
        "deviceMemory must be a power-of-two GiB value from the persona table"
    );
    assert_eq!(
        v["memGt8Possible"], true,
        "Chrome 155 dropped the 8 GiB cap — persona mem can exceed 8"
    );
}

#[test]
fn test_issue_78_jsvmp_env_scan_face() {
    // The XHS jsvmp environment scan reads these four faces directly; all
    // must match Chrome (issue #78): listener registries invisible
    // (WeakMap storage — a Symbol key would still be enumerable),
    // webdriver defined-and-false, sub-ms floats from a 100µs-quantized
    // clock, and interface toStringTags on navigator/window.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            document.addEventListener('x', function () {});
            navigator.connection.addEventListener('change', function () {});
            const xhr = new XMLHttpRequest();
            xhr.addEventListener('load', function () {});
            const fr = new FileReader();
            fr.addEventListener('load', function () {});
            // A ~3ms busy-wait window spans >=30 100µs ticks, so a sub-ms
            // fraction must show up. Sampling a fixed count instead can land
            // every read on one integral tick and flake ~10% of runs.
            const start = performance.now();
            let sawFraction = false, guard = 0;
            while (performance.now() - start < 3 && guard < 2000000) {
                if (performance.now() % 1 !== 0) { sawFraction = true; break; }
                guard++;
            }
            const to = performance.timeOrigin;
            const ns = performance.timing.navigationStart;
            const originSane = to >= ns && to < ns + 1;
            return [
                typeof document._listeners,
                typeof navigator.connection._listeners,
                typeof xhr._listeners,
                typeof fr._listeners,
                typeof navigator.webdriver,
                Object.prototype.toString.call(navigator),
                Object.prototype.toString.call(window),
                sawFraction,
                originSane,
                Number.isInteger(ns),
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "undefined",
            "undefined",
            "undefined",
            "undefined",
            "boolean",
            "[object Navigator]",
            "[object Window]",
            true,
            true,
            true,
        ])
    );
}

/// Upstream 846ed7d: the Function.prototype.toString override must have a
/// native function's shape — name, length, non-constructible, no own
/// `prototype` property.
#[test]
fn function_to_string_has_native_function_shape() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
            .evaluate(
                r#"(() => {
                    const fn = Function.prototype.toString;
                    let constructible = true;
                    try { Reflect.construct(function () {}, [], fn); } catch (e) { constructible = false; }
                    return [fn.toString(), fn.name, fn.length,
                            Object.prototype.hasOwnProperty.call(fn, "prototype"),
                            constructible].join("|");
                })()"#,
            )
            .unwrap();
    assert_eq!(
        v,
        serde_json::json!("function toString() { [native code] }|toString|0|false|false")
    );
}

/// Upstream 4c33f6d (tamperedFunctions): JS-backed builtins — constructors,
/// prototype methods, and accessors — must all report [native code].
#[test]
fn builtin_members_report_native_code() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
            .evaluate(
                r#"(() => {
                    const nodeTypeGet = Object.getOwnPropertyDescriptor(Node.prototype, "nodeType").get;
                    return [String(Element), String(Node),
                            String(Element.prototype.getAttribute),
                            String(nodeTypeGet)].join("|");
                })()"#,
            )
            .unwrap();
    assert_eq!(
        v,
        serde_json::json!(
            "function Element() { [native code] }|function Node() { [native code] }|function getAttribute() { [native code] }|function get nodeType() { [native code] }"
        )
    );
}

/// Upstream 4c33f6d (unusualWindowProperties): internal globals must not
/// surface through any reflection API on the global object.
/// Engine helpers (`_wrap`, `__diting*`, …) stay hidden; page-installed
/// single-underscore names are covered by
/// `page_underscore_globals_remain_visible_to_gopn` (Chrome face).
#[test]
fn internal_globals_are_hidden_from_all_reflection_apis() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
            .evaluate(
                r#"(() => {
                    const bad = (a) => a.filter(n => typeof n === "string" &&
                        (n[0] === "_" || n.includes("obscura") || n.includes("Obscura") || n.includes("diting") || n.includes("Diting"))).length;
                    const descs = Object.getOwnPropertyDescriptors(window);
                    return [bad(Object.getOwnPropertyNames(window)),
                            bad(Reflect.ownKeys(window)),
                            bad(Object.keys(window)),
                            bad(Object.keys(descs))].join("|");
                })()"#,
            )
            .unwrap();
    assert_eq!(v, serde_json::json!("0|0|0|0"));
}

/// Chrome exposes page-installed single-underscore window props (xhs
/// `_webmsxyw`) via getOwnPropertyNames; double-underscore bookkeeping
/// and engine hide-list names stay filtered. Blanket `startsWith('_')`
/// used to hide `_webmsxyw` and skew the winKeys face ark's 300015
/// probes hash.
#[test]
fn page_underscore_globals_remain_visible_to_gopn() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
            .evaluate(
                r#"(() => {
                    window._webmsxyw = function xy() { return 1; };
                    window.__shouldStayHidden = 1;
                    const names = Object.getOwnPropertyNames(window);
                    const ownKeys = Reflect.ownKeys(window);
                    const descs = Object.getOwnPropertyDescriptors(window);
                    return [
                      names.includes('_webmsxyw'),
                      ownKeys.includes('_webmsxyw'),
                      Object.prototype.hasOwnProperty.call(descs, '_webmsxyw'),
                      names.includes('__shouldStayHidden'),
                      names.some(n => typeof n === 'string' && (n.includes('diting') || n.includes('Diting'))),
                      typeof window._webmsxyw
                    ].join('|');
                })()"#,
            )
            .unwrap();
    assert_eq!(v, serde_json::json!("true|true|true|false|false|function"));
}

/// Upstream c7e7c70: WebIDL interface globals are non-enumerable in a real
/// browser (and stay callable).
#[test]
fn webidl_interface_globals_are_non_enumerable() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"(() => {
                    const names = ["Node", "Element", "Document", "Window",
                                   "CSSStyleDeclaration", "DOMStringMap"];
                    const enumerable = names.filter(n => {
                        const d = Object.getOwnPropertyDescriptor(window, n);
                        return !d || d.enumerable !== false;
                    });
                    return [enumerable.length, Object.keys(window).includes("Node"),
                            typeof Node, document.body instanceof Element].join("|");
                })()"#,
        )
        .unwrap();
    assert_eq!(v, serde_json::json!("0|false|function|true"));
}

/// #184: pages may legally replace the interface globals — the
/// webcomponents-era HTMLElement "native shim"
/// (`function(){return Reflect.construct(orig,[],this.constructor)}`, which
/// forwards NO constructor arguments) is the canonical shape, and github.com
/// installs one. Once the global was swapped, internal wrapping that routed
/// through the live global (`_elementClassFor`'s fallback → `new C(nid)`)
/// constructed the authorization gate with `undefined`, and every wrapper
/// mint for a custom/unknown tag threw "Illegal constructor" — github's
/// hydration retried it hundreds of times per page load. Real Chrome never
/// consults the global for its internal C++ wrappers; internal wrapping now
/// binds the bootstrap-time classes, and the page-mutable globals serve
/// page code only.
#[test]
fn native_shim_on_interface_globals_does_not_break_internal_wrapping() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r##"
        const NativeHTMLElement = HTMLElement;
        const NativeForm = HTMLFormElement;
        // The canonical native-shim shape github installs (verified live on
        // the 86quan engine: window.HTMLElement.src was exactly this).
        window.HTMLElement = function HTMLElement() {
          return Reflect.construct(NativeHTMLElement, [], this.constructor);
        };
        window.HTMLElement.prototype = NativeHTMLElement.prototype;
        window.HTMLFormElement = function HTMLFormElement() {
          return Reflect.construct(NativeForm, [], this.constructor);
        };
        window.HTMLFormElement.prototype = NativeForm.prototype;
        // innerHTML builds raw nids; the first querySelectorAll/getElementById
        // mints the wrappers, exercising _elementClassFor's fallback path.
        document.body.innerHTML = '<x-issue184 id="x"></x-issue184>' +
            '<form id="f" action="#"><input name="q" value="v"></form>';
        const all = document.querySelectorAll('x-issue184');
        const byId = document.getElementById('x');
        const form = document.querySelector('#f');
        ({
          found: all.length === 1,
          tagName: all[0].tagName,
          identity: all[0] === byId,
          formTag: form.tagName,
          namedAccess: form.q !== undefined && form.q.value === 'v'
        })
    "##;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "found": true,
            "tagName": "X-ISSUE184",
            "identity": true,
            "formTag": "FORM",
            "namedAccess": true
        })
    );
}

/// Key enumeration must not hide underscore names on document/performance.
/// 1f6ede4 stripped every `_name` there (Chrome-parity for the engine's own
/// slots) and also hid PAGE-defined keys; 抖店's React/Garfish mount path then
/// failed hydration (publish page stayed a skeleton, no product requests).
/// The XHS gate does not need the strip (xhs-merchant-publish is green without
/// it), so page-defined keys stay visible.
#[test]
fn key_enumeration_keeps_page_underscore_keys_on_document_and_performance() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r##"
        document._reactRootContainer = { page: true };
        document.__pageMarker = 1;
        performance.__pageNote = 2;
        const has = (o, n) => [Object.keys(o), Object.getOwnPropertyNames(o), Reflect.ownKeys(o),
            Object.keys(Object.getOwnPropertyDescriptors(o))].map((a) => a.includes(n));
        ({
            docMarker: has(document, '_reactRootContainer'),
            docMarker2: has(document, '__pageMarker'),
            perfNote: has(performance, '__pageNote')
        })
    "##;
    let v = rt.evaluate(js).unwrap();
    let t = serde_json::json!([true, true, true, true]);
    assert_eq!(
        v,
        serde_json::json!({ "docMarker": t, "docMarker2": t, "perfNote": t })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_date_to_string_pads_day_of_month() {
    // Date.prototype.toString zero-pads the day (spec TimeString step 9).
    // The unpadded 'Oct 4' broke doudian's SSR text-node comparison — the
    // server rendered 'Oct 04', React threw #418 and discarded the server
    // DOM. Pin the padding shape in a host-TZ-independent way: build the
    // instant from Date.UTC shifted by the engine's own (persona) offset,
    // so a host in UTC and a host in Asia/Shanghai must render the same
    // civil date. Local-field coherence lives next door in
    // test_date_local_fields_follow_persona_tz (#221).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const off = -new Date(Date.UTC(2026, 9, 4)).getTimezoneOffset();
            const d = new Date(Date.UTC(2026, 9, 4, 5, 6, 7) - off * 60000);
            const ds = d.toDateString();
            const ts = d.toString();
            return [ds,
                    /^[A-Za-z]{3} [A-Za-z]{3} \d{2} \d{4}$/.test(ds) && ds.endsWith('04 2026'),
                    ts.startsWith(ds + ' 05:06:07 ')];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v[1],
        serde_json::json!(true),
        "toDateString pads the day: {:?}",
        v[0]
    );
    assert_eq!(
        v[2],
        serde_json::json!(true),
        "toString pads the day: {:?}",
        v[0]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_date_local_fields_follow_persona_tz() {
    // #221: constructor local-args, the get/set field families, and the
    // formatter family must all resolve in the persona zone. Before the
    // fix, a TZ=UTC host printed toString() "13:06:07 GMT+0800" while
    // getHours() returned 5 on the same instant. Both pins below are
    // host-independent: 'America/New_York' forces the wrap on Shanghai
    // and UTC hosts alike, 'Asia/Shanghai' exercises the common persona
    // (fast path on a Shanghai host, wrapped on CI's UTC).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const out = [];
            globalThis.__diting_tz = 'Asia/Shanghai';
            // getters + formatters agree on a fixed instant (the #221 repro)
            const d = new Date(Date.UTC(2026, 9, 4, 5, 6, 7)); // wall 13:06:07 +08
            out.push(d.getHours() === 13, d.getMinutes() === 6, d.getSeconds() === 7);
            out.push(d.getFullYear() === 2026, d.getMonth() === 9, d.getDate() === 4);
            out.push(d.getDay() === 0); // 2026-10-04 is a Sunday
            out.push(d.toString().includes('13:06:07'));
            // setter: wall 20:30 +08 = 12:30Z
            d.setHours(20, 30, 0, 0);
            out.push(d.getTime() === Date.UTC(2026, 9, 4, 12, 30, 0));
            // setter keeps unspecified fields at their persona values
            const p = new Date(Date.UTC(2026, 9, 4, 5, 6, 7));
            p.setMonth(11);
            out.push(p.getTime() === Date.UTC(2026, 11, 4, 5, 6, 7));
            // local-args constructor reads fields in the persona zone
            const c = new Date(2026, 9, 4, 5, 6, 7);
            out.push(c.getTime() === Date.UTC(2026, 9, 4, 5, 6, 7) - 8 * 3600000);
            // two-digit years still land in 19xx
            out.push(new Date(98, 9, 4).getTime() === Date.UTC(1998, 9, 4) - 8 * 3600000);
            delete globalThis.__diting_tz;
            // DST zone: offset must come from the instant, not a fixed value
            globalThis.__diting_tz = 'America/New_York';
            const jul = new Date(Date.UTC(2026, 6, 4, 12, 0, 0)); // EDT -4
            const jan = new Date(Date.UTC(2026, 0, 4, 12, 0, 0)); // EST -5
            out.push(jul.getHours() === 8, jan.getHours() === 7);
            out.push(jul.getTimezoneOffset() === 240, jan.getTimezoneOffset() === 300);
            out.push(new Date(2026, 6, 4, 9, 0, 0).getTime() === Date.UTC(2026, 6, 4, 13, 0, 0));
            delete globalThis.__diting_tz;
            return out;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    for (i, item) in v.as_array().unwrap().iter().enumerate() {
        assert_eq!(
            item,
            &serde_json::json!(true),
            "#221 clause {i} failed: {v:?}"
        );
    }
}

/// The generated modules register their expected shape (constructor arity,
/// @@toStringTag, per-member descriptor flags, accessor/method form, native
/// look); __platform.verify() checks the live objects against it.
#[test]
fn platform_generated_descriptors_conform() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate("JSON.stringify(__platform.verify())").unwrap();
    assert_eq!(out.as_str().unwrap(), "[]", "descriptor violations: {out}");
}

/// JS-PLATFORM-ARCH sec 13: errors thrown by platform-generated APIs must be
/// Chrome-shaped under reflection - stack header = the UNDECORATED message,
/// e.message = "<prefix>: <message>", ZERO shim frames above the caller, and
/// the same own-property shape ({message, stack}; stack an accessor). The same
/// probe produced chrome/chrome-error-truth.json in real Chrome 156.
#[test]
fn platform_errors_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/error-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/chrome-error-truth.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth["result"].as_object().unwrap() {
        let got = &engine[k];
        if got != want {
            bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} error shape(s) differ from Chrome:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Chrome-version coherence at RUNTIME (the offline gate is tools/check_coherence.py):
/// the JS-visible identity follows crates/diting_env/chrome-target.toml - the reduced UA,
/// the Client Hints brand versions and the full version all carry the target.
#[tokio::test(flavor = "current_thread")]
async fn js_identity_follows_the_chrome_target() {
    use diting_env::chrome_target::{CHROME_FULL_VERSION, CHROME_MAJOR};
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                var h = await navigator.userAgentData.getHighEntropyValues(['uaFullVersion', 'fullVersionList']);
                return JSON.stringify({ ua: navigator.userAgent, brands: navigator.userAgentData.brands,
                                        full: h.uaFullVersion, list: h.fullVersionList });
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
    assert!(
        v["ua"]
            .as_str()
            .unwrap()
            .contains(&format!("Chrome/{CHROME_MAJOR}.0.0.0 ")),
        "navigator.userAgent must carry the target major: {}",
        v["ua"]
    );
    assert_eq!(
        v["full"], CHROME_FULL_VERSION,
        "uaFullVersion must be the target's full version"
    );
    for b in v["brands"].as_array().unwrap() {
        let name = b["brand"].as_str().unwrap();
        if name == "Chromium" || name == "Google Chrome" {
            assert_eq!(
                b["version"],
                CHROME_MAJOR.to_string(),
                "brand {name} must carry the target major"
            );
        }
    }
    for b in v["list"].as_array().unwrap() {
        let name = b["brand"].as_str().unwrap();
        if name == "Chromium" || name == "Google Chrome" {
            assert_eq!(
                b["version"], CHROME_FULL_VERSION,
                "fullVersionList {name} must be the full version"
            );
        }
    }
}

/// M4 knife 53 - the own-ENUMERABLE keys of ~200 objects (every element tag, nodes, ranges, events, XHR/WebSocket/streams, singletons,
/// canvas, observers, URL family, ...) against real Chrome 157 (chrome/enum-probe.js, enum-truth-157.json). for-in + hasOwnProperty is what
/// for-in / JSON.stringify / Object.assign / spread see. KNOWN GAPS are listed explicitly and must shrink, never grow:
///   newdoc    - createHTMLDocument()/DOMParser documents are plain objects, not Documents
///   gain/ctxDest - Web Audio nodes are plain objects (GainNode/AudioDestinationNode identity missing)
///   gl        - WebGL: headless Chrome answers null, the engine a fake context
/// and keys that depend on the host page / shared state, not on the engine (stylesheet and div counts, localStorage left by other tests).
#[tokio::test(flavor = "current_thread")]
async fn own_enumerable_keys_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/enum-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/enum-truth-157.json"
    ))
    .unwrap();
    const SKIP: &[&str] = &[
        "newdoc",
        "gain",
        "ctxDest",
        "gl",
        "sheets",
        "nodeList",
        "htmlColl",
        "rects",
        "selfWin",
        "globalThis",
        "ls",
    ];
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if k.starts_with('#') || SKIP.contains(&k.as_str()) {
            continue;
        }
        if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} object(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
