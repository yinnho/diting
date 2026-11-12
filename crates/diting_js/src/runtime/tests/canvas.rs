//! canvas 2D/path2d/toDataURL/webgl 扩展面
use super::common::setup_runtime;

/// (300015 P3) Blank-canvas readout face: the XHS shield fingerprints
/// `toDataURL('image/webp')` on a never-drawn 300x150 canvas — the
/// encoder's byte output IS the signal, and for a blank canvas it is a
/// CONSTANT per (format, w, h). Chrome truth: webp 279 / jpeg 783 /
/// png 2118 chars at 300x150 (CDP-captured, canvas_face.js table).
/// Blank = no 2D context OR an all-zero buffer; uncovered dims and
/// drawn canvases fall to the honest PNG floor (real magic bytes, no
/// seeded fake stub).
#[test]
fn canvas_blank_readout_face_p3() {
    let mut rt = setup_runtime("<html><body>x</body></html>");
    let v = rt
        .evaluate(
            r#"
            var r = {};
            // never-contexted 300x150 — the exact shield probe shape
            var c = document.createElement('canvas');
            var webp = c.toDataURL('image/webp');
            var jpeg = c.toDataURL('image/jpeg');
            var png = c.toDataURL();
            var bmp = c.toDataURL('image/bmp');
            // getContext'd but never drawn — the zero-buffer scan must agree
            var c2 = document.createElement('canvas');
            c2.getContext('2d');
            var webp2 = c2.toDataURL('image/webp');
            // webp blob from the constant: type + exact byte size
            var bSize, bType;
            c.toBlob(function(b){ bSize = b.size; bType = b.type; }, 'image/webp');
            // uncovered dims (137x91) blank webp → honest floor: REAL png magic
            var c3 = document.createElement('canvas');
            c3.width = 137; c3.height = 91;
            var odd = c3.toDataURL('image/webp');
            var oddBin = atob(odd.slice(odd.indexOf(',') + 1));
            // drawn canvas webp → falls through to the engine's real PNG
            var c4 = document.createElement('canvas');
            var x = c4.getContext('2d');
            x.fillStyle = '#f60'; x.fillRect(0, 0, 50, 50);
            var drawn = c4.toDataURL('image/webp');
            // wrapper face: name/length native parity
            var d1 = HTMLCanvasElement.prototype.toDataURL;
            var d2 = HTMLCanvasElement.prototype.toBlob;
            r.webp = [webp.length, webp.slice(0, 48), webp.slice(-24)];
            r.jpeg = [jpeg.length, jpeg.slice(0, 48), jpeg.slice(-24)];
            r.png = [png.length, png.slice(0, 48), png.slice(-24)];
            r.bmp_is_png_const = bmp === png;
            r.ctx_blank_same = webp2 === webp;
            r.blob = [bType, bSize];
            r.odd = [odd.slice(0, 22), oddBin.length,
                     oddBin.charCodeAt(0) === 137 && oddBin.charCodeAt(1) === 80];
            r.drawn = [drawn.slice(0, 22), drawn.length > 300];
            r.face = [d1.name, d1.length, String(d1).indexOf('native code') >= 0,
                      d2.name, d2.length, String(d2).indexOf('native code') >= 0];
            JSON.stringify(r)
        "#,
        )
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(v.as_str().expect("string from JSON.stringify")).unwrap();
    assert_eq!(v["webp"][0], 279, "webp const length");
    assert_eq!(
        v["webp"][1], "data:image/webp;base64,UklGRrgAAABXRUJQVlA4WAoAA",
        "webp const head"
    );
    assert_eq!(v["webp"][2], "ZOQ99qwAAP7/1gAAAAAAAAAA", "webp const tail");
    assert_eq!(v["jpeg"][0], 783, "jpeg const length");
    assert_eq!(
        v["jpeg"][1], "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABA",
        "jpeg const head"
    );
    assert_eq!(v["jpeg"][2], "AAAAAAAAAAAAAAAAAAA//9k=", "jpeg const tail");
    assert_eq!(v["png"][0], 2118, "png const length");
    assert_eq!(
        v["png"][1], "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAS",
        "png const head"
    );
    assert_eq!(v["png"][2], "AS0pote3AAAAAElFTkSuQmCC", "png const tail");
    assert_eq!(
        v["bmp_is_png_const"], true,
        "unknown type normalizes to the png const"
    );
    assert_eq!(
        v["ctx_blank_same"], true,
        "zero-buffer canvas === never-contexted canvas"
    );
    assert_eq!(v["blob"][0], "image/webp", "toBlob webp type");
    assert_eq!(v["blob"][1], 192, "toBlob webp exact byte size");
    assert_eq!(
        v["odd"][0], "data:image/png;base64,",
        "uncovered dims → honest png prefix"
    );
    assert_eq!(
        v["odd"][2], true,
        "uncovered dims → real PNG magic bytes (no fake stub)"
    );
    assert_eq!(
        v["drawn"][0], "data:image/png;base64,",
        "drawn canvas → engine PNG (documented residual)"
    );
    assert_eq!(v["drawn"][1], true, "drawn canvas payload is real bytes");
    assert_eq!(v["face"][0], "toDataURL", "wrapper name");
    assert_eq!(
        v["face"][1], 0,
        "toDataURL.length (both args optional per IDL; Chrome 145 shape audit)"
    );
    assert_eq!(v["face"][2], true, "toDataURL toString native");
    assert_eq!(v["face"][3], "toBlob", "wrapper name");
    assert_eq!(
        v["face"][4], 1,
        "toBlob.length (callback required; Chrome 145 shape audit)"
    );
    assert_eq!(v["face"][5], true, "toBlob toString native");
}

/// (300015 P4) Two in-page-detectable faces the XHS create page reads:
/// ① a DRAWN canvas through toDataURL('image/jpeg') must return real
/// baseline-JPEG bytes (FF D8 … FF D9) under a jpeg prefix — the engine
/// used to return PNG bytes there, a one-line detect; ② the audio class
/// surface must match Chrome 155 (tools/webplatform/chrome/chrome-audio-truth.json):
/// prototype member lists in Chrome's order with constructor LAST,
/// accessor kind per member, ZERO own props on instances, born
/// 'suspended', resume/suspend on AudioContext + OfflineAudioContext but
/// NOT Base, EventTarget carrying addEventListener/dispatchEvent/
/// removeEventListener/when, OAC.suspend() throwing without an argument.
#[tokio::test(flavor = "current_thread")]
async fn canvas_jpeg_readout_and_audio_face_p4() {
    let mut rt = setup_runtime("<html><body>x</body></html>");
    let v = rt.evaluate(r#"
            var r = {};
            // --- drawn canvas jpeg: real FF D8...FF D9 bytes, jpeg prefix ---
            var c = document.createElement('canvas');
            c.width = 2000; c.height = 200;
            var ctx = c.getContext('2d');
            ctx.fillStyle = '#f60'; ctx.fillRect(0, 0, 1000, 100);
            ctx.fillStyle = 'rgba(0,30,200,0.7)'; ctx.fillRect(500, 50, 800, 120);
            var url = c.toDataURL('image/jpeg');
            var bin = atob(url.slice(url.indexOf(',') + 1));
            r.jpeg = [url.slice(0, 23),
                      bin.charCodeAt(0) + ',' + bin.charCodeAt(1) + ',' + bin.charCodeAt(2),
                      bin.charCodeAt(bin.length - 2) + ',' + bin.charCodeAt(bin.length - 1),
                      bin.length];
            r.png_still_png = c.toDataURL().slice(0, 14);
            r.alias = c.toDataURL('image/jpg').slice(0, 23);
            var lo = c.toDataURL('image/jpeg', 0.1).length;
            var hi = c.toDataURL('image/jpeg', 0.99).length;
            r.quality_gap = hi - lo;
            var bt = null, bs = 0;
            c.toBlob(function(b){ bt = b.type; bs = b.size; }, 'image/jpeg');
            r.blob = [bt, bs, bs === bin.length];
            // --- audio surface vs the Chrome truth capture ---
            var ac = new AudioContext();
            r.ac_own = Object.getOwnPropertyNames(ac);
            r.ac_state = ac.state;
            r.ac_proto = Object.getOwnPropertyNames(AudioContext.prototype);
            var oac = new OfflineAudioContext(1, 44100, 44100);
            r.oac_own = Object.getOwnPropertyNames(oac);
            r.oac_proto = Object.getOwnPropertyNames(OfflineAudioContext.prototype);
            r.base_proto = Object.getOwnPropertyNames(BaseAudioContext.prototype);
            r.et_proto = Object.getOwnPropertyNames(EventTarget.prototype);
            var oc = Object.getOwnPropertyDescriptor(OfflineAudioContext.prototype, 'oncomplete');
            var lg = Object.getOwnPropertyDescriptor(OfflineAudioContext.prototype, 'length');
            r.oac_desc = [typeof oc.get, lg.enumerable, lg.configurable];
            r.oac_oncomplete = typeof oac.oncomplete;
            r.oac_length = oac.length;
            var rc = new AudioContext(); rc.resume();
            r.state_after_resume = rc.state;
            r.suspend_noarg = (function(){ try { oac.suspend(); return 'no-throw'; } catch(e){ return e.name; } })();
            r.ac_extends_et = ac instanceof EventTarget;
            r.complete_evt = (function(){
                var o2 = new OfflineAudioContext(1, 500, 44100);
                o2.addEventListener('complete', function(ev){ window.__completeFired = ev.type; });
                o2.startRendering();
                return 'armed'; // 'complete' fires from a microtask — read on the next eval
            })();
            r.ab_own = Object.getOwnPropertyNames(new AudioBuffer({numberOfChannels: 1, length: 100}));
            r.ab_proto = Object.getOwnPropertyNames(AudioBuffer.prototype);
            r.ab_same_ref = (function(){ var b = new AudioBuffer({numberOfChannels: 1, length: 10}); return b.getChannelData(0) === b.getChannelData(0); })();
            JSON.stringify(r)
        "#).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(v.as_str().expect("string from JSON.stringify")).unwrap();
    // ① drawn jpeg — prefix, SOI+APP0 JFIF marker, EOI, nonzero payload.
    assert_eq!(v["jpeg"][0], "data:image/jpeg;base64,", "drawn jpeg prefix");
    assert_eq!(
        v["jpeg"][1], "255,216,255",
        "SOI + 0xFF marker (real JPEG magic)"
    );
    assert_eq!(v["jpeg"][2], "255,217", "EOI trailer");
    assert!(
        v["jpeg"][3].as_i64().unwrap() > 1000,
        "drawn 2000x200 jpeg payload is real bytes"
    );
    assert_eq!(
        v["png_still_png"], "data:image/png",
        "default type stays PNG"
    );
    assert_eq!(
        v["alias"], "data:image/jpeg;base64,",
        "'image/jpg' alias dispatches jpeg"
    );
    assert!(
        v["quality_gap"].as_i64().unwrap() > 0,
        "higher quality → larger output"
    );
    assert_eq!(v["blob"][0], "image/jpeg", "toBlob jpeg type");
    assert_eq!(v["blob"][2], true, "toBlob byte size === toDataURL payload");
    // ② audio — exact prototype lists, Chrome order, constructor last.
    assert_eq!(
        v["ac_proto"],
        serde_json::json!([
            "baseLatency",
            "outputLatency",
            "onerror",
            "close",
            "createMediaElementSource",
            "createMediaStreamDestination",
            "createMediaStreamSource",
            "getOutputTimestamp",
            "resume",
            "suspend",
            "constructor",
            "sinkId",
            "onsinkchange",
            "setSinkId"
        ]),
        "AudioContext.prototype names (刀57: playbackStats deleted; 刀75: sinkId/onsinkchange/setSinkId per Chrome 145 truth)"
    );
    assert_eq!(
        v["oac_proto"],
        serde_json::json!([
            "oncomplete",
            "length",
            "resume",
            "startRendering",
            "suspend",
            "constructor"
        ]),
        "OfflineAudioContext.prototype names"
    );
    assert_eq!(
        v["base_proto"],
        serde_json::json!([
            "destination",
            "sampleRate",
            "currentTime",
            "listener",
            "state",
            "onstatechange",
            "createAnalyser",
            "createBiquadFilter",
            "createBuffer",
            "createBufferSource",
            "createChannelMerger",
            "createChannelSplitter",
            "createConstantSource",
            "createConvolver",
            "createDelay",
            "createDynamicsCompressor",
            "createGain",
            "createIIRFilter",
            "createOscillator",
            "createPanner",
            "createPeriodicWave",
            "createScriptProcessor",
            "createStereoPanner",
            "createWaveShaper",
            "decodeAudioData",
            "constructor",
            "audioWorklet"
        ]),
        "BaseAudioContext.prototype names (no resume/suspend — they moved up a level; 刀57: renderQuantumSize deleted)"
    );
    assert_eq!(
        v["et_proto"],
        serde_json::json!([
            "addEventListener",
            "dispatchEvent",
            "removeEventListener",
            "when",
            "constructor"
        ]),
        "EventTarget.prototype names"
    );
    assert_eq!(
        v["ac_own"],
        serde_json::json!([]),
        "AudioContext instance carries zero own props"
    );
    assert_eq!(
        v["oac_own"],
        serde_json::json!([]),
        "OfflineAudioContext instance carries zero own props"
    );
    assert_eq!(
        v["ac_state"], "suspended",
        "born suspended (autoplay policy)"
    );
    assert_eq!(
        v["oac_desc"],
        serde_json::json!(["function", true, true]),
        "oncomplete is an accessor; length getter is enumerable+configurable like Chrome"
    );
    assert_eq!(
        v["oac_oncomplete"], "object",
        "unset oncomplete reads as null ('object')"
    );
    assert_eq!(
        v["oac_length"], 44100,
        "OAC length reflects the constructor arg"
    );
    assert_eq!(v["state_after_resume"], "running", "resume() flips state");
    assert_eq!(
        v["suspend_noarg"], "TypeError",
        "OAC.suspend() without an argument throws"
    );
    assert_eq!(
        v["ac_extends_et"], true,
        "contexts sit on the EventTarget chain"
    );
    // Microtask-flushed: pump the loop, then read the flag (Chrome's
    // startRendering also settles async).
    let _ = rt.run_event_loop_bounded(100).await;
    let fired = rt.evaluate("window.__completeFired || 'never'").unwrap();
    assert_eq!(
        fired.as_str().unwrap_or("never"),
        "complete",
        "'complete' listener fires from startRendering"
    );
    assert_eq!(
        v["ab_own"],
        serde_json::json!([]),
        "AudioBuffer instance carries zero own props"
    );
    assert_eq!(
        v["ab_proto"],
        serde_json::json!([
            "length",
            "duration",
            "sampleRate",
            "numberOfChannels",
            "copyFromChannel",
            "copyToChannel",
            "getChannelData",
            "constructor"
        ]),
        "AudioBuffer.prototype names"
    );
    assert_eq!(
        v["ab_same_ref"], true,
        "getChannelData returns the same array every call"
    );
}

/// createImageBitmap must resolve a bitmap with header-parsed dims. The
/// old stub resolved 0×0 for everything and the xhs publish page's
/// upload pipeline gated on it silently — files landed in input.files,
/// no preview mounted, no upload request ever fired. Un-decodable input
/// rejects with InvalidStateError so sites' catch branches still run.
#[tokio::test(flavor = "current_thread")]
async fn create_image_bitmap_resolves_header_dims() {
    // Sig + chunk len + "IHDR" + w=62 + h=33 — offsets 16..24, same
    // pin as the Image-shim test above, but carried by a Blob this time.
    let mut png: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&[0, 0, 0, 0x0D]);
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&62u32.to_be_bytes());
    png.extend_from_slice(&33u32.to_be_bytes());

    let b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&png)
    };
    let garbage_b64 = "aGVsbG8gd29ybGQ="; // "hello world" — no image header

    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                &format!(
                    r#"async () => {{
                        const mk = async (b64, type) => {{
                            const blob = await (await fetch('data:' + type + ';base64,' + b64)).blob();
                            try {{
                                const bmp = await createImageBitmap(blob);
                                return {{ok: 'yes', w: String(bmp.width), h: String(bmp.height)}};
                            }} catch (e) {{
                                return {{ok: 'no', name: e && e.name, msg: String(e && e.message).slice(0, 60)}};
                            }}
                        }};
                        return {{
                            png: await mk("{}", 'image/png'),
                            garbage: await mk("{}", 'text/plain'),
                            notBlob: await mk2(),
                            canvas: await mk3(),
                        }};
                        async function mk2() {{
                            try {{ await createImageBitmap(null); return {{ok: 'no2'}}; }}
                            catch (e) {{ return {{ok: 'no', name: e && e.name}}; }}
                        }}
                        // Live canvas source: capability probes pass a canvas
                        // (no arrayBuffer method) — rejecting it as TypeError
                        // made the xhs publish page refuse to mount its
                        // upload UI; Chrome resolves the canvas's own dims.
                        async function mk3() {{
                            const c = document.createElement('canvas');
                            c.width = 320; c.height = 200;
                            try {{
                                const b = await createImageBitmap(c);
                                return {{ok: 'yes', w: String(b.width), h: String(b.height)}};
                            }} catch (e) {{ return {{ok: 'no', name: e && e.name}}; }}
                        }}
                    }}"#,
                    b64, garbage_b64
                ),
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    let v = result.value.unwrap();
    // Real header → real dims (the load-bearing change).
    assert_eq!(v["png"]["ok"], serde_json::json!("yes"));
    assert_eq!(v["png"]["w"], serde_json::json!("62"));
    assert_eq!(v["png"]["h"], serde_json::json!("33"));
    // Garbage bytes → Chrome-shaped rejection, not a 0×0 resolve.
    assert_eq!(v["garbage"]["ok"], serde_json::json!("no"));
    assert_eq!(v["garbage"]["name"], serde_json::json!("InvalidStateError"));
    // Non-blob source → TypeError before any read.
    assert_eq!(v["notBlob"]["name"], serde_json::json!("TypeError"));
    // Canvas source → its own dimensions (capability-probe shape).
    assert_eq!(v["canvas"]["ok"], serde_json::json!("yes"));
    assert_eq!(v["canvas"]["w"], serde_json::json!("320"));
    assert_eq!(v["canvas"]["h"], serde_json::json!("200"));
}

/// (#117/F1) The masked GL strings carry the Chrome 145 constants and
/// VENDOR/RENDERER/VERSION/SHADING_LANGUAGE_VERSION resolve through the
/// context (they were missing from _GL_CONSTS, so fp.js read
/// `gl.VERSION` as undefined and got the default:0 branch — the server
/// saw a number where every browser sends a string). VERSION/SHADING
/// differ per context class.
#[tokio::test(flavor = "current_thread")]
async fn webgl_masked_strings_match_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    // Two canvases: our getContext caches the first context
                    // per canvas, and fp probes always burn a fresh canvas.
                    const g1 = document.createElement('canvas').getContext('webgl');
                    const g2 = document.createElement('canvas').getContext('webgl2');
                    const consts = [g1.VENDOR, g1.RENDERER, g1.VERSION, g1.SHADING_LANGUAGE_VERSION];
                    const v1 = [g1.getParameter(g1.VENDOR), g1.getParameter(g1.RENDERER),
                                g1.getParameter(g1.VERSION), g1.getParameter(g1.SHADING_LANGUAGE_VERSION)];
                    const v2 = [g2.getParameter(g2.VERSION), g2.getParameter(g2.SHADING_LANGUAGE_VERSION)];
                    return [consts, v1, v2];
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
        serde_json::json!([0x1F00, 0x1F01, 0x1F02, 0x8B8C]),
        "the four pname constants resolve on the context"
    );
    assert_eq!(
        v[1],
        serde_json::json!([
            "WebKit",
            "WebKit WebGL",
            "WebGL 1.0 (OpenGL ES 2.0 Chromium)",
            "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)"
        ]),
        "webgl1 masked strings (Chrome 145 capture)"
    );
    assert_eq!(
        v[2],
        serde_json::json!([
            "WebGL 2.0 (OpenGL ES 3.0 Chromium)",
            "WebGL GLSL ES 3.00 (OpenGL ES 3.0 Chromium)"
        ]),
        "webgl2 masked strings"
    );
}

/// (#117/F2) getSupportedExtensions serves the captured Chrome 145 lists
/// (39 webgl1 / 36 webgl2) — the old hardcoded 4-item list was a bot
/// tell on its own — and getExtension answers every advertised name.
#[tokio::test(flavor = "current_thread")]
async fn webgl_extension_lists_match_chrome_capture() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const g1 = document.createElement('canvas').getContext('webgl');
                    const g2 = document.createElement('canvas').getContext('webgl2');
                    const e1 = g1.getSupportedExtensions(), e2 = g2.getSupportedExtensions();
                    return [
                        e1.length, e2.length,
                        e1.includes('WEBGL_debug_renderer_info'),
                        e1.includes('EXT_sRGB'),
                        e2.includes('EXT_conservative_depth'),
                        !e2.includes('EXT_sRGB'),
                        typeof g1.getExtension('WEBGL_lose_context').loseContext,
                        g1.getExtension('no_such_extension_xyz') === null,
                        typeof g1.getExtension('EXT_sRGB'),
                    ];
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v[0], serde_json::json!(39), "webgl1 extension count");
    assert_eq!(v[1], serde_json::json!(36), "webgl2 extension count");
    assert_eq!(v[2], serde_json::json!(true));
    assert_eq!(v[3], serde_json::json!(true), "EXT_sRGB is webgl1-only");
    assert_eq!(
        v[4],
        serde_json::json!(true),
        "EXT_conservative_depth is webgl2-only"
    );
    assert_eq!(v[5], serde_json::json!(true));
    assert_eq!(
        v[6],
        serde_json::json!("function"),
        "loseContext live object"
    );
    assert_eq!(v[7], serde_json::json!(true), "unlisted name -> null");
    assert_eq!(
        v[8],
        serde_json::json!("object"),
        "advertised name -> object"
    );
}

/// (#117/F3) toDataURL returns a REAL PNG — magic bytes, IHDR width/
/// height matching the canvas, stable across calls, and content-
/// dependent (fillRect+fillText change it). The old stub returned a
/// fixed-shape fake base64 that fails any server-side decode.
#[tokio::test(flavor = "current_thread")]
async fn canvas_to_data_url_is_a_real_png() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const c = document.createElement('canvas');
                    c.width = 220; c.height = 30;
                    const pristine = c.toDataURL();
                    const pristine2 = c.toDataURL();
                    const x = c.getContext('2d');
                    x.textBaseline = 'top';
                    x.font = "14px 'Arial'";
                    x.fillStyle = '#f60';
                    x.fillRect(0, 0, 110, 30);
                    x.fillStyle = '#069';
                    x.fillText('mmmmmmmmlli ~@#', 2, 2);
                    const drawn = c.toDataURL();
                    const b64 = drawn.slice(drawn.indexOf(',') + 1);
                    const bin = atob(b64);
                    const sig = [bin.charCodeAt(0), bin.charCodeAt(1), bin.charCodeAt(2), bin.charCodeAt(3)];
                    const ihdrW = (bin.charCodeAt(16) << 24 | bin.charCodeAt(17) << 16 |
                                   bin.charCodeAt(18) << 8 | bin.charCodeAt(19)) >>> 0;
                    const ihdrH = (bin.charCodeAt(20) << 24 | bin.charCodeAt(21) << 16 |
                                   bin.charCodeAt(22) << 8 | bin.charCodeAt(23)) >>> 0;
                    const blob = await new Promise((res) => {
                        const cc = document.createElement('canvas');
                        cc.width = 10; cc.height = 10;
                        cc.getContext('2d');
                        cc.toBlob(res);
                    });
                    return [sig, ihdrW, ihdrH, pristine === pristine2, pristine !== drawn,
                            drawn.length > 300, blob.size > 50, blob.type];
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
        serde_json::json!([137, 80, 78, 71]),
        "PNG magic bytes"
    );
    assert_eq!(v[1], serde_json::json!(220), "IHDR width");
    assert_eq!(v[2], serde_json::json!(30), "IHDR height");
    assert_eq!(v[3], serde_json::json!(true), "stable across reads");
    assert_eq!(v[4], serde_json::json!(true), "drawing changes the PNG");
    assert_eq!(v[5], serde_json::json!(true), "fingerprint-size payload");
    assert_eq!(v[6], serde_json::json!(true), "toBlob carries real bytes");
    assert_eq!(v[7], serde_json::json!("image/png"), "toBlob MIME");
}

/// fillText reaches the swash raster through op_canvas_text's `#[object]`
/// return (hand-built since serde_v8 was absorbed on the v8 152 bump).
/// canvas2d.js swallows an op failure into the noise-glyph fallback, which
/// only ever writes full-alpha pixels — so anti-aliased edge pixels
/// (0 < alpha < 255) prove the real tile arrived, with its fields intact.
#[cfg(feature = "screenshot")]
#[test]
fn canvas_fill_text_uses_the_raster_tile_not_the_fallback() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt
        .evaluate(
            r#"(() => {
            const c = document.createElement('canvas');
            c.width = 100; c.height = 30;
            const x = c.getContext('2d');
            x.textBaseline = 'top';
            x.font = '14px Arial';
            x.fillStyle = 'rgb(10,20,30)';
            x.fillText('mm', 2, 2);
            const d = x.getImageData(0, 0, 100, 30).data;
            let ink = 0, edge = 0;
            for (let i = 3; i < d.length; i += 4) {
                if (d[i] > 0) ink++;
                if (d[i] > 0 && d[i] < 255) edge++;
            }
            return JSON.stringify({ ink, edge });
        })()"#,
        )
        .unwrap();
    let got: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert!(got["ink"].as_u64().unwrap() > 0, "text drew ink: {got}");
    assert!(
        got["edge"].as_u64().unwrap() > 0,
        "anti-aliased edges = the swash tile, not the full-alpha fallback: {got}"
    );
}

/// (#117/F5) Canvas-only methods live on HTMLCanvasElement.prototype —
/// `'toDataURL' in document.body` and `'getContext' in <div>` are both
/// false (they were true when the methods sat on Element.prototype),
/// while canvas elements keep working through the prototype chain.
#[tokio::test(flavor = "current_thread")]
async fn canvas_methods_left_element_prototype() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const bodyHas = 'toDataURL' in document.body;
                    const divHasCtx = 'getContext' in document.createElement('div');
                    const canvasHasAll = ['getContext', 'toDataURL', 'toBlob'].every(
                        (m) => typeof document.createElement('canvas')[m] === 'function');
                    let draws = false;
                    try {
                        const c = document.createElement('canvas');
                        const x = c.getContext('2d');
                        x.font = '14px Arial';
                        x.fillText('hi', 2, 2);
                        x.fillRect(0, 0, 5, 5);
                        draws = c.toDataURL().indexOf('data:image/png;base64,') === 0;
                    } catch (e) {}
                    const onCanvasProto = Object.getPrototypeOf(document.createElement('canvas')) === HTMLCanvasElement.prototype;
                    return [!bodyHas, !divHasCtx, canvasHasAll, draws, onCanvasProto];
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
        serde_json::json!(true),
        "body no longer answers toDataURL"
    );
    assert_eq!(
        v[1],
        serde_json::json!(true),
        "div no longer answers getContext"
    );
    assert_eq!(v[2], serde_json::json!(true), "canvas keeps all three");
    assert_eq!(
        v[3],
        serde_json::json!(true),
        "2d draw + encode still works"
    );
    assert_eq!(
        v[4],
        serde_json::json!(true),
        "canvas prototype chain intact"
    );
}

/// #15: canvas clip() constrains every paint path. Chrome truth (issue
/// probe): after clip to a 50×50 rect, a 100×100 fill leaves (70,70) at its
/// pre-clip color while (20,20) takes the new one; restore() drops the
/// region; an empty-path clip suppresses everything; clearRect is clipped
/// too; two save/clip levels intersect.
#[test]
fn canvas_clip_constrains_paint_paths() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () { try {
        const mk = () => { const c = document.createElement('canvas'); c.width = 100; c.height = 100; return c.getContext('2d'); };
        const px = (g, x, y) => { const d = g.getImageData(x, y, 1, 1).data; return [d[0], d[1], d[2]]; };
        const g = mk();
        g.fillStyle = '#f00'; g.fillRect(0, 0, 100, 100);
        g.save(); g.beginPath(); g.rect(0, 0, 50, 50); g.clip();
        g.fillStyle = '#00f'; g.fillRect(0, 0, 100, 100);
        const outsideStaysRed = px(g, 70, 70);
        const insideTakesBlue = px(g, 20, 20);
        g.restore();
        g.fillStyle = '#0f0'; g.fillRect(0, 0, 100, 100);
        const afterRestore = px(g, 70, 70);
        // clearRect honors the clip: clearing under it leaves the outside intact.
        const g2 = mk();
        g2.fillStyle = '#f00'; g2.fillRect(0, 0, 100, 100);
        g2.save(); g2.beginPath(); g2.rect(10, 10, 20, 20); g2.clip();
        g2.clearRect(0, 0, 100, 100); g2.restore();
        const clearOutsideIntact = px(g2, 70, 70);
        const clearInsideCleared = px(g2, 15, 15);
        // Two save/clip levels intersect: 30×30 ∩ rows 20..40 = a 10-row band.
        const g3 = mk();
        g3.fillStyle = '#00f'; g3.fillRect(0, 0, 100, 100);
        g3.save(); g3.beginPath(); g3.rect(0, 0, 30, 30); g3.clip();
        g3.save(); g3.beginPath(); g3.rect(0, 20, 100, 20); g3.clip();
        g3.fillStyle = '#f00'; g3.fillRect(0, 0, 100, 100);
        g3.restore(); g3.restore();
        const intersectBand = px(g3, 15, 25);
        const intersectBlocked = px(g3, 15, 45);
        // Empty path: everything after is clipped away, restore brings it back.
        const g4 = mk();
        g4.save(); g4.beginPath(); g4.clip();
        g4.fillStyle = '#f00'; g4.fillRect(0, 0, 100, 100); g4.restore();
        g4.fillStyle = '#00f'; g4.fillRect(0, 0, 100, 100);
        const emptyClipThenFree = px(g4, 50, 50);
        // Path is NOT consumed by clip(): the same path still fills.
        const g5 = mk();
        g5.beginPath(); g5.rect(10, 10, 30, 30); g5.clip();
        g5.fillStyle = '#f00'; g5.fill();
        const clipThenFill = px(g5, 20, 20);
        globalThis.__out = JSON.stringify([
            outsideStaysRed, insideTakesBlue, afterRestore,
            clearOutsideIntact, clearInsideCleared,
            intersectBand, intersectBlocked, emptyClipThenFree, clipThenFill,
        ]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#,
        )
        .unwrap();
    let s = out.as_str().unwrap();
    assert!(!s.starts_with("ERR"), "clip probe threw: {}", s);
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!([
            [255, 0, 0],
            [0, 0, 255],
            [0, 255, 0],
            [255, 0, 0],
            [0, 0, 0],
            [255, 0, 0],
            [0, 0, 255],
            [0, 0, 255],
            [255, 0, 0],
        ]),
        "clip must gate fillRect/fill/clearRect, stack via save/restore, and leave the path usable"
    );
}

/// #157 / obscura#1085: Canvas2D color parsing covers the CSS Color
/// grammar Chrome accepts — hsl()/hsla() with hue wrap and percent
/// channels, percent alpha, the modern space/slash syntax, hex8,
/// keywords, and 'none'. Opaque families pin through the
/// fillRect→getImageData roundtrip (parse feeds the real raster);
/// alpha families pin on _parseColor directly because the raster
/// src-over composites alpha over whatever is already in the buffer.
#[tokio::test(flavor = "current_thread")]
async fn canvas_fill_style_parses_css_color_grammar() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const px = (fill) => {
                        // Fresh canvas per sample: the raster composites
                        // src-over, so leftover ink from the previous fill
                        // would leak into this one's channels.
                        const c = document.createElement('canvas');
                        c.width = 4; c.height = 1;
                        const g = c.getContext('2d');
                        g.fillStyle = fill;
                        g.fillRect(0, 0, 4, 1);
                        const d = g.getImageData(0, 0, 1, 1).data;
                        return [d[0], d[1], d[2], d[3]];
                    };
                    const g0 = document.createElement('canvas').getContext('2d');
                    return {
                        hsl: px('hsl(120, 50%, 50%)'),
                        hueWraps: px('hsl(480, 50%, 50%)'),
                        rgbPercent: px('rgb(100%, 0%, 0%)'),
                        rgbaComma: px('rgba(12, 34, 56, 1)'),
                        keyword: px('rebeccapurple'),
                        garbage: px('not-a-color'),
                        parsed: {
                            hslaPercentAlpha: g0._parseColor('hsla(120, 50%, 50%, 50%)'),
                            rgbaModernSlash: g0._parseColor('rgba(255 0 0 / 0.5)'),
                            hex8: g0._parseColor('#ff000080'),
                            none: g0._parseColor('none'),
                            transparent: g0._parseColor('transparent'),
                        },
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let v = result.value.unwrap();
    // Chrome truth: hsl(120,50%,50%) = #40BF40 = (64,191,64); 480 wraps
    // to 120; rgb percents scale ×255/100; rebeccapurple is (102,51,153);
    // garbage falls back to opaque black; 50% alpha rounds to 128.
    assert_eq!(
        v["hsl"],
        serde_json::json!([64, 191, 64, 255]),
        "hsl sextant math"
    );
    assert_eq!(
        v["hueWraps"],
        serde_json::json!([64, 191, 64, 255]),
        "hue wraps mod 360"
    );
    assert_eq!(v["rgbPercent"], serde_json::json!([255, 0, 0, 255]));
    assert_eq!(v["rgbaComma"], serde_json::json!([12, 34, 56, 255]));
    assert_eq!(v["keyword"], serde_json::json!([102, 51, 153, 255]));
    assert_eq!(v["garbage"], serde_json::json!([0, 0, 0, 255]));
    assert_eq!(
        v["parsed"]["hslaPercentAlpha"],
        serde_json::json!([64, 191, 64, 128])
    );
    assert_eq!(
        v["parsed"]["rgbaModernSlash"],
        serde_json::json!([255, 0, 0, 128])
    );
    assert_eq!(v["parsed"]["hex8"], serde_json::json!([255, 0, 0, 128]));
    assert_eq!(v["parsed"]["none"], serde_json::json!([0, 0, 0, 0]));
    assert_eq!(v["parsed"]["transparent"], serde_json::json!([0, 0, 0, 0]));
}

/// #164: Path2D is a real path (SVG path-data constructor + command
/// API) and ctx.fill rasterizes it under the CTM; canvas.width reflects
/// the content attribute and resizes the live 2D buffer. qrcode.react
/// feature-detects addPath and draws whole QR modules as path data
/// through fill(new Path2D(d)) under ctx.scale() — the all-no-op stub
/// passed the detect and painted a blank canvas (taobao havana login).
#[tokio::test(flavor = "current_thread")]
async fn canvas_path2d_fill_rasterizes_under_transform() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"() => {
                    const px = (g, x, y) => { const d = g.getImageData(x, y, 1, 1).data; return [d[0], d[1], d[2], d[3]]; };
                    const out = {};
                    // The #164 draw sequence: whole modules as SVG path data
                    // through fill(new Path2D(d)) under ctx.scale().
                    const c = document.createElement('canvas');
                    c.width = 132; c.height = 132;
                    const g = c.getContext('2d');
                    g.scale(4, 4);
                    g.fillStyle = '#ffffff';
                    g.fillRect(0, 0, 33, 33);
                    g.fillStyle = '#000000';
                    g.fill(new Path2D('M4 4h1v1h-1zM6 4h1v1h-1z'));
                    out.qrModule = px(g, 18, 18); // module (4,4) → device 16..32
                    out.qrGap = px(g, 22, 18);    // module (5,4) absent → white bg
                    out.detect = (() => { const p = new Path2D(); p.moveTo(0, 0); p.addPath(new Path2D()); return p instanceof Path2D; })();
                    // Command-API path via the current-path form fill()
                    const c4 = document.createElement('canvas');
                    c4.width = 10; c4.height = 10;
                    const g4 = c4.getContext('2d');
                    g4.fillStyle = '#000000';
                    g4.beginPath(); g4.moveTo(2, 2); g4.lineTo(8, 2); g4.lineTo(5, 8); g4.closePath(); g4.fill();
                    out.cmdPath = px(g4, 5, 4);
                    // nonzero fills nested same-winding squares; evenodd
                    // carves the inner one into a hole.
                    const nested = () => {
                        const h = document.createElement('canvas');
                        h.width = 20; h.height = 20;
                        const gh = h.getContext('2d');
                        gh.fillStyle = '#000000';
                        return gh;
                    };
                    const gh1 = nested();
                    gh1.fill(new Path2D('M2 2h16v16h-16z M6 6h8v8h-8z'));
                    out.nonzeroInner = px(gh1, 10, 10);
                    const gh2 = nested();
                    gh2.fill(new Path2D('M2 2h16v16h-16z M6 6h8v8h-8z'), 'evenodd');
                    out.evenoddHole = px(gh2, 10, 10);
                    out.evenoddRing = px(gh2, 4, 10);
                    // width/height are reflected IDL attributes (defaults
                    // 300/150); a post-getContext write resizes the live
                    // buffer and resets drawing state, like Chrome.
                    const c2 = document.createElement('canvas');
                    out.freshDefault = [c2.width, c2.height];
                    const g2 = c2.getContext('2d');
                    c2.width = 41; c2.height = 10;
                    out.attrReflected = c2.getAttribute('width');
                    g2.fillStyle = '#123456';
                    g2.fillRect(0, 0, 41, 10);
                    out.resizedCorner = px(g2, 40, 9);
                    const url = c2.toDataURL();
                    const bin = atob(url.slice(url.indexOf(',') + 1));
                    out.ihdr = [(bin.charCodeAt(16) << 24 | bin.charCodeAt(17) << 16 | bin.charCodeAt(18) << 8 | bin.charCodeAt(19)) >>> 0,
                                (bin.charCodeAt(20) << 24 | bin.charCodeAt(21) << 16 | bin.charCodeAt(22) << 8 | bin.charCodeAt(23)) >>> 0];
                    c2.width = 8; // resize resets state
                    g2.fillRect(0, 0, 8, 10);
                    out.stateReset = px(g2, 4, 4);
                    // Real CTM: translate(2,3)·rotate(90°)·scale(2,2) =
                    // [0 2 -2 0 2 3]; save/restore snapshots the matrix.
                    const g3 = document.createElement('canvas').getContext('2d');
                    g3.translate(2, 3); g3.rotate(Math.PI / 2); g3.scale(2, 2);
                    const t = g3.getTransform();
                    g3.save(); g3.scale(5, 5); g3.restore();
                    const t2 = g3.getTransform();
                    const r = (x) => Math.round(x * 1000) / 1000;
                    out.ctm = [r(t.a), r(t.b), r(t.c), r(t.d), r(t.e), r(t.f)];
                    out.restored = [r(t2.a), r(t2.b), r(t2.c), r(t2.d), r(t2.e), r(t2.f)];
                    return out;
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
        v["qrModule"],
        serde_json::json!([0, 0, 0, 255]),
        "Path2D SVG module pixel is black"
    );
    assert_eq!(
        v["qrGap"],
        serde_json::json!([255, 255, 255, 255]),
        "absent module stays background"
    );
    assert_eq!(
        v["detect"],
        serde_json::json!(true),
        "addPath feature detect still passes"
    );
    assert_eq!(
        v["cmdPath"],
        serde_json::json!([0, 0, 0, 255]),
        "command-API path fills via fill()"
    );
    assert_eq!(
        v["nonzeroInner"],
        serde_json::json!([0, 0, 0, 255]),
        "nonzero fills nested squares"
    );
    assert_eq!(
        v["evenoddHole"],
        serde_json::json!([0, 0, 0, 0]),
        "evenodd carves the inner hole"
    );
    assert_eq!(
        v["evenoddRing"],
        serde_json::json!([0, 0, 0, 255]),
        "evenodd ring stays filled"
    );
    assert_eq!(
        v["freshDefault"],
        serde_json::json!([300, 150]),
        "IDL defaults 300×150"
    );
    assert_eq!(
        v["attrReflected"],
        serde_json::json!("41"),
        "width write reflects to the attribute"
    );
    assert_eq!(
        v["resizedCorner"],
        serde_json::json!([18, 52, 86, 255]),
        "post-getContext write resizes the buffer"
    );
    assert_eq!(
        v["ihdr"],
        serde_json::json!([41, 10]),
        "PNG IHDR follows the resized buffer"
    );
    assert_eq!(
        v["stateReset"],
        serde_json::json!([0, 0, 0, 255]),
        "resize resets fillStyle to default"
    );
    assert_eq!(
        v["ctm"],
        serde_json::json!([0, 2, -2, 0, 2, 3]),
        "CTM compose translate·rotate·scale"
    );
    assert_eq!(
        v["restored"],
        serde_json::json!([0, 2, -2, 0, 2, 3]),
        "restore rewinds the matrix"
    );
}

/// M4 knife 41: CanvasRenderingContext2D is the real interface (instanceof, constructor name, toStringTag, no own keys)
/// and carries the state attributes the 2D context never had (cvstate-probe.js, 32 keys, real Chrome 145): defaults,
/// silent rejection of invalid enum / non-finite / malformed-length values, 'inherit' direction, line-dash handling,
/// reset(), roundRect() validation and rasterization, brand checks.
#[tokio::test(flavor = "current_thread")]
async fn canvasstate_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/cvstate-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/cvstate-truth-145.json"
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

/// M4 knife 42: CanvasGradient / CanvasPattern (cvobj-probe.js, 16 keys, real Chrome 145): illegal-constructor interfaces,
/// factories that return real instances (they were object literals with `_x0` keys) and validate their arguments
/// (arity, non-finite doubles, negative radii as IndexSizeError, pattern image type / repetition / zero-size canvas),
/// addColorStop's range and color checks, setTransform, fillStyle round-trip identity, brand checks.
#[tokio::test(flavor = "current_thread")]
async fn canvasobjects_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/cvobj-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/cvobj-truth-145.json"
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
