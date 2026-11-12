//! img/video/audio 编解码面/vtt/editcontext
use super::super::*;
use super::common::setup_runtime;

/// (300015 P6) Chrome truth (headed+headless probe 10-15): an
/// empty-graph OfflineAudioContext render is SILENCE — nonzero:0,
/// sum:0 over the whole buffer. The engine used to fill a seeded
/// pseudo-signal as an anti-detector guess with the polarity backwards;
/// the shield's probe is exactly this shape.
#[tokio::test(flavor = "current_thread")]
async fn oac_empty_render_is_silence() {
    let mut rt = setup_runtime("<html><body>x</body></html>");
    rt.evaluate(
        r#"
            var oac = new OfflineAudioContext(1, 44100, 44100);
            oac.startRendering().then(function (buf) {
                var d = buf.getChannelData(0);
                var nonzero = 0, sum = 0;
                for (var i = 0; i < d.length; i++) { if (d[i] !== 0) nonzero++; sum += d[i]; }
                window.__oacResult = JSON.stringify(
                    {len: d.length, nonzero: nonzero, sum: sum, state: oac.state});
            });
        "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(500).await;
    let v = rt.evaluate("window.__oacResult || 'unset'").unwrap();
    let v: serde_json::Value =
        serde_json::from_str(v.as_str().expect("string from JSON.stringify"))
            .expect("oac result json");
    assert_eq!(v["len"], 44100);
    assert_eq!(v["nonzero"], 0, "empty graph renders all zeros like Chrome");
    assert_eq!(v["sum"], 0);
    assert_eq!(
        v["state"], "closed",
        "render settles the context into closed"
    );
}

/// (#41) `new Image()` must gate `load` on a real fetch: a 2xx response
/// fires `load` with complete=true; the old stub fired `load`
/// unconditionally without any network attempt.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn image_factory_fires_load_on_2xx_fetch() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Minimal PNG-magic body — the load face gates on transport status,
    // not on decode (no JS-face decoder; documented boundary in #41).
    let png: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            png.len()
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.write_all(png).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const img = new Image();
                    let sawError = false;
                    img.addEventListener('error', () => { sawError = true; }, { once: true });
                    const loaded = new Promise(r => { img.onload = () => r('fired'); });
                    img.src = '/x.png';
                    const how = await loaded;
                    return { how, sawError: String(sawError), complete: String(img.complete) };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["sawError"], serde_json::json!("false"));
    assert_eq!(v["complete"], serde_json::json!("true"));
}

/// (#41 follow-up) A 2xx body carrying a real image header must
/// backfill naturalWidth/naturalHeight via op_image_info. Sites gate on
/// `naturalWidth != 0` to decide whether an artifact decoded (the xhs
/// zeus AB SDK retries its PNG artifact 25× and dead-ends on 0), so
/// header-parsed numbers are load-bearing — the load/error face itself
/// still gates on transport status only.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn image_factory_backfills_natural_dimensions_from_2xx_body() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Sig + chunk len + "IHDR" + w=62 + h=33 — the parser reads offsets
    // 16..24, so a header-only body pins the parse without a decoder.
    let mut png: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&[0, 0, 0, 0x0D]);
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&62u32.to_be_bytes());
    png.extend_from_slice(&33u32.to_be_bytes());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            png.len()
        );
        stream.write_all(resp.as_bytes()).unwrap();
        stream.write_all(&png).unwrap();
        stream.flush().unwrap();
    });

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const img = new Image(); // no width/height args: fallback would be 0
                    const loaded = new Promise(r => { img.onload = () => r('fired'); });
                    img.src = '/x.png';
                    const how = await loaded;
                    return {
                        how,
                        nw: String(img.naturalWidth),
                        nh: String(img.naturalHeight),
                        // #236: width/height fall back to the natural size when
                        // nothing pinned them — the old own-prop 0 gated SDKs
                        // on a phantom zero.
                        w: String(img.width),
                        h: String(img.height),
                        complete: String(img.complete),
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["nw"], serde_json::json!("62"));
    assert_eq!(v["nh"], serde_json::json!("33"));
    assert_eq!(
        v["w"],
        serde_json::json!("62"),
        "width must fall back to natural"
    );
    assert_eq!(
        v["h"],
        serde_json::json!("33"),
        "height must fall back to natural"
    );
    assert_eq!(v["complete"], serde_json::json!("true"));
}

/// (#52) Identical image bytes must yield the same intrinsic size whether
/// they arrive via a data: URL (header-parsed) or a blob: URL minted by
/// createObjectURL. The blob: branch used to dispatch `load` with 0×0 —
/// the xhs publish page probes every uploaded file through
/// `URL.createObjectURL(file)` + `new Image()` to compute width/height/
/// ratio, got width=0/ratio=NaN, and the upload chain died silently (no
/// request, no rejection, page stuck in the upload state). Unparsable
/// bytes keep the old load-with-0×0 face; an unregistered blob: URL
/// still errors.
#[tokio::test(flavor = "current_thread")]
async fn image_blob_url_backfills_natural_dimensions() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    // Sig + chunk len + "IHDR" + w=62 + h=33 — offsets 16..24
                    // pin the header parse without a real decoder.
                    const png = new Uint8Array([137,80,78,71,13,10,26,10, 0,0,0,13, 73,72,68,82, 0,0,0,62, 0,0,0,33]);
                    const probe = (img, src) => new Promise(r => {
                        img.onload = () => r('load:' + img.naturalWidth + 'x' + img.naturalHeight);
                        img.onerror = () => r('error');
                        img.src = src;
                    });
                    const img = new Image();
                    const pngRes = await probe(img, URL.createObjectURL(new Blob([png], {type: 'image/png'})));
                    const garbage = new Image();
                    const garbageRes = await probe(garbage, URL.createObjectURL(new Blob([new Uint8Array([1,2,3])], {type: 'image/png'})));
                    const missing = new Image();
                    const missingRes = await probe(missing, 'blob:https://example.com/never-registered');
                    return { pngRes, garbageRes, missingRes };
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["pngRes"], serde_json::json!("load:62x33"));
    assert_eq!(v["garbageRes"], serde_json::json!("load:0x0"));
    assert_eq!(v["missingRes"], serde_json::json!("error"));
}

/// #59 REQ-1: a blob-backed `<video>` must reach Chrome's local-source
/// metadata face — the video sibling of the two image tests above.
/// Probe-pinned (real Chrome 157 + a 2.6MB mp4, 2026-11-09): emptied →
/// loadstart → durationchange → resize → loadedmetadata → progress →
/// suspend → loadeddata → canplay → canplaythrough, with dims/duration/
/// readyState=4 visible from durationchange on. Composers gate their
/// upload INIT on this face — the X one dead-ends on a preview parked at
/// 0×0/NaN and never hits upload.twitter.com. Unparsable bytes keep the
/// stub face (no events, 0×0, NaN duration).
#[tokio::test(flavor = "current_thread")]
async fn video_blob_source_reaches_chrome_metadata_face() {
    // Hand-built ftyp + moov{mvhd timescale=1000 duration=36000,
    // trak{tkhd identity 1280×720}} — offset tables shared with the
    // ops::tests parser pins.
    let box_of = |ty: &[u8; 4], payload: &[u8]| -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + payload.len());
        v.extend_from_slice(&((8 + payload.len()) as u32).to_be_bytes());
        v.extend_from_slice(ty);
        v.extend_from_slice(payload);
        v
    };
    let mut mvhd = vec![0u8; 20];
    mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
    mvhd[16..20].copy_from_slice(&36000u32.to_be_bytes());
    let mut tkhd = vec![0u8; 84];
    tkhd[40..44].copy_from_slice(&0x0001_0000u32.to_be_bytes()); // matrix[0]
    tkhd[56..60].copy_from_slice(&0x0001_0000u32.to_be_bytes()); // matrix[4]
    tkhd[76..80].copy_from_slice(&(1280u32 << 16).to_be_bytes());
    tkhd[80..84].copy_from_slice(&(720u32 << 16).to_be_bytes());
    let moov = box_of(
        b"moov",
        &[
            box_of(b"mvhd", &mvhd),
            box_of(b"trak", &box_of(b"tkhd", &tkhd)),
        ]
        .concat(),
    );
    let mp4 = [box_of(b"ftyp", b"isom"), moov].concat();
    let b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&mp4)
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url("https://example.com/page");
    let result = rt
        .call_function_on_remote(
            &format!(
                r#"async () => {{
                    const bin = Uint8Array.from(atob("{b64}"), c => c.charCodeAt(0));
                    const f = new File([bin], 'a.mp4', {{ type: 'video/mp4' }});
                    const url = URL.createObjectURL(f);
                    const v = document.createElement('video');
                    const seen = [];
                    const EVS = ['emptied','loadstart','durationchange','resize','loadedmetadata','progress','suspend','loadeddata','canplay','canplaythrough'];
                    for (const e of EVS) v.addEventListener(e, () => seen.push(e + '@rs' + v.readyState + '@' + v.videoWidth + 'x' + v.videoHeight + '@' + v.duration), false);
                    v.src = url;
                    await new Promise(r => v.addEventListener('canplaythrough', r, {{ once: true }}));
                    // Garbage bytes: the stub face must survive untouched.
                    const gv = document.createElement('video');
                    let gFired = 0;
                    for (const e of EVS) gv.addEventListener(e, () => {{ gFired++; }}, false);
                    gv.src = URL.createObjectURL(new File([new Uint8Array([104, 105])], 'g.mp4', {{ type: 'video/mp4' }}));
                    gv.load();
                    await new Promise(r => setTimeout(r, 60));
                    return {{
                        seen: seen.join(','),
                        w: v.videoWidth, h: v.videoHeight, d: v.duration,
                        rs: v.readyState, ns: v.networkState, csrc: v.currentSrc.slice(0, 5),
                        gFired, gRs: gv.readyState, gW: gv.videoWidth, gD: String(gv.duration),
                    }};
                }}"#
            ),
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    // Chrome's probe order verbatim, values visible from durationchange on.
    assert_eq!(
        v["seen"],
        serde_json::json!(
            "emptied@rs0@0x0@NaN,loadstart@rs0@0x0@NaN,durationchange@rs4@1280x720@36,resize@rs4@1280x720@36,loadedmetadata@rs4@1280x720@36,progress@rs4@1280x720@36,suspend@rs4@1280x720@36,loadeddata@rs4@1280x720@36,canplay@rs4@1280x720@36,canplaythrough@rs4@1280x720@36"
        )
    );
    assert_eq!(v["w"], serde_json::json!(1280));
    assert_eq!(v["h"], serde_json::json!(720));
    assert_eq!(v["d"].as_f64(), Some(36.0)); // JS numbers lose the .0 face in JSON
    assert_eq!(v["rs"], serde_json::json!(4));
    assert_eq!(v["ns"], serde_json::json!(1));
    assert_eq!(v["csrc"], serde_json::json!("blob:"));
    // Stub face intact for garbage.
    assert_eq!(v["gFired"], serde_json::json!(0));
    assert_eq!(v["gRs"], serde_json::json!(0));
    assert_eq!(v["gW"], serde_json::json!(0));
    assert_eq!(v["gD"], serde_json::json!("NaN"));
}

/// (#41) failure side: an unreachable src (connection-refused port) must
/// fire `error`, never the phantom `load` the old stub produced.
#[allow(clippy::await_holding_lock)] // the env guard must span the await — that's the serialization
#[tokio::test(flavor = "current_thread")]
async fn image_factory_fires_error_on_refused_port() {
    let _env_guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
    std::env::set_var("DITING_ALLOW_PRIVATE_NETWORK", "1");

    // Bind then drop: the port answers connection-refused.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };

    let mut rt = setup_runtime("<html><body></body></html>");
    rt.set_url(&format!("http://127.0.0.1:{}/page", port));
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const img = new Image();
                    let sawLoad = false;
                    img.addEventListener('load', () => { sawLoad = true; }, { once: true });
                    const failed = new Promise(r => { img.onerror = () => r('fired'); });
                    img.src = '/x.png';
                    const how = await failed;
                    return { how, sawLoad: String(sawLoad), complete: String(img.complete) };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["sawLoad"], serde_json::json!("false"));
    assert_eq!(v["complete"], serde_json::json!("false"));
}

/// (#41) regression: data: URLs keep resolving locally and still fire
/// `load` without any network attempt.
#[tokio::test(flavor = "current_thread")]
async fn image_factory_data_url_still_fires_load() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const img = new Image();
                    let sawError = false;
                    img.addEventListener('error', () => { sawError = true; }, { once: true });
                    const loaded = new Promise(r => { img.onload = () => r('fired'); });
                    img.src = 'data:image/gif;base64,R0lGODlhAQABAAAAACw=';
                    const how = await loaded;
                    return { how, sawError: String(sawError), complete: String(img.complete), nw: String(img.naturalWidth), nh: String(img.naturalHeight) };
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["how"], serde_json::json!("fired"));
    assert_eq!(v["sawError"], serde_json::json!("false"));
    assert_eq!(v["complete"], serde_json::json!("true"));
    // The embedded GIF87a header carries a 1×1 logical screen — the
    // data: branch goes through the same header parse as the fetch one.
    assert_eq!(v["nw"], serde_json::json!("1"));
    assert_eq!(v["nh"], serde_json::json!("1"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_image_is_real_element_and_emulates_load() {
    // Upstream a5a8de7 + 891d850: new Image() must be a real element
    // (style/attribute reflection/event dispatch), assigning a src must
    // fire `load` on both the onload property and listeners (#41: for a
    // data: URL the load face is local; network srcs now gate on a real
    // fetch — see image_factory_fires_* tests), and a pre-defined
    // non-configurable own src (Booking.com instrumentation) must not
    // crash the constructor.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const img = new Image(10, 20);
            const isEl = img instanceof globalThis.HTMLImageElement;
            const styleOk = img.style instanceof globalThis.CSSStyleDeclaration;
            img.style.width = '30px';
            const styleSet = img.style.width === '30px';
            img.width = 10; img.height = 20;
            let viaProp = 0, viaListener = 0;
            img.onload = () => viaProp++;
            img.addEventListener('load', () => viaListener++);
            img.src = 'data:image/gif;base64,R0lGODlhAQABAAAAACw=';
            const earlyComplete = img.complete;
            await new Promise(r => setTimeout(r, 20));
            // Anti-bot pattern: hijack createElement and pre-define a
            // non-configurable own src on every <img>.
            const origCreate = document.createElement.bind(document);
            document.createElement = function (tag) {
                const el = origCreate(tag);
                if (String(tag).toLowerCase() === 'img') {
                    Object.defineProperty(el, 'src', { value: '', writable: true, configurable: false });
                }
                return el;
            };
            let hijackSurvived = false, hijackW = 0;
            try {
                const img2 = new Image(7, 8);
                hijackSurvived = true;
                hijackW = img2.width;
            } catch (e) { hijackSurvived = e.message; }
            document.createElement = origCreate;
            return [isEl, styleOk, styleSet, earlyComplete, img.complete,
                    img.naturalWidth, viaProp, viaListener, hijackSurvived, hijackW];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([true, true, true, false, true, 1, 1, 1, true, 7])
    );
    // naturalWidth=1: the parsed GIF intrinsic size, not the width-attr
    // fallback (Chrome reads intrinsic dims here; img.width stays 10).
}

/// #190: the media-state face (paused/readyState/duration/play/…) used to
/// sit on Element.prototype, so EVERY element answered `readyState === 0`.
/// Baidu's sbase assetOnload gates its load callback on
/// `/loaded|complete|undefined/.test(node.readyState)` — written for
/// browsers where script elements have no readyState — so every
/// dynamically-loaded superman component timed out 10s in and the feed fell
/// back to default ordering. Chrome keeps the whole face on
/// HTMLMediaElement.prototype; script/div answer undefined again.
#[test]
fn media_face_lives_on_htmlmediaelement_not_element() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r##"
        const v = document.createElement('video');
        const a = document.createElement('audio');
        const s = document.createElement('script');
        const d = document.createElement('div');
        let ctorThrew = null;
        try { new HTMLMediaElement(); } catch (e) { ctorThrew = e.name; }
        ({
            hasMedia: typeof HTMLMediaElement,
            vInstanceOfMedia: v instanceof HTMLMediaElement,
            aInstanceOfMedia: a instanceof HTMLMediaElement,
            protoParent: Object.getPrototypeOf(HTMLVideoElement.prototype) === HTMLMediaElement.prototype,
            scriptReadyState: String(s.readyState),
            // The exact gate baidu's loader tests — undefined must open it.
            scriptGateOpen: /loaded|complete|undefined/.test(String(s.readyState)),
            divPaused: String(d.paused),
            readyStateInElementProto: 'readyState' in Element.prototype,
            videoPaused: v.paused,
            videoReadyState: v.readyState,
            videoHaveEnough: v.HAVE_ENOUGH_DATA,
            videoPlayType: typeof v.play,
            videoCtorName: v.constructor.name,
            ctorThrew
        })
    "##;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "hasMedia": "function",
            "vInstanceOfMedia": true,
            "aInstanceOfMedia": true,
            "protoParent": true,
            "scriptReadyState": "undefined",
            "scriptGateOpen": true,
            "divPaused": "undefined",
            "readyStateInElementProto": false,
            "videoPaused": true,
            "videoReadyState": 0,
            "videoHaveEnough": 4,
            "videoPlayType": "function",
            "videoCtorName": "HTMLVideoElement",
            "ctorThrew": "TypeError"
        })
    );
}

/// canPlayType / isTypeSupported answer Chrome 145's measured table (a
/// 1068-string matrix compared against a real Chrome): a container knows a
/// codec set, unknown codecs for a known container are "", bare known
/// containers are "maybe", a miscased `Codecs=` parameter is "", and the
/// method lives on HTMLMediaElement.prototype only.
#[test]
fn can_play_type_follows_chromes_codec_table() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r##"
        const v = document.createElement('video');
        const t = (x) => v.canPlayType(x);
        ({
            mp4Avc: t('video/mp4; codecs="avc1.42E01E, mp4a.40.2"'),
            mp4Bare: t('video/mp4'),
            mp4Vp9Alias: t('video/mp4; codecs="vp9"'),
            webmVp9: t('video/webm; codecs="vp9"'),
            webmAvc: t('video/webm; codecs="avc1.42E01E"'),
            theora: t('video/ogg; codecs="theora"'),
            wav: t('audio/wav'),
            mp3: t('audio/mpeg'),
            audioMp4Aac: t('audio/mp4; codecs="mp4a.40.2"'),
            unquoted: t('video/webm; codecs=vp9, opus'),
            miscased: t('video/webm; Codecs="VP9"'),
            unknown: t('foo/bar'),
            mseBare: MediaSource.isTypeSupported('video/mp4'),
            mseAvc: MediaSource.isTypeSupported('video/mp4; codecs="avc1.42E01E"'),
            mseMpeg: MediaSource.isTypeSupported('audio/mpeg'),
            onElementProto: 'canPlayType' in Element.prototype,
            onMediaProto: Object.prototype.hasOwnProperty.call(HTMLMediaElement.prototype, 'canPlayType'),
            mrWebm: MediaRecorder.isTypeSupported('video/webm;codecs=vp9'),
            mrOgg: MediaRecorder.isTypeSupported('audio/ogg'),
            pip: document.pictureInPictureEnabled
        })
    "##;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "mp4Avc": "probably", "mp4Bare": "maybe", "mp4Vp9Alias": "",
            "webmVp9": "probably", "webmAvc": "", "theora": "", "wav": "maybe",
            "mp3": "probably", "audioMp4Aac": "probably", "unquoted": "probably",
            "miscased": "", "unknown": "",
            "mseBare": false, "mseAvc": true, "mseMpeg": true,
            "onElementProto": false, "onMediaProto": true,
            "mrWebm": true, "mrOgg": false, "pip": true
        })
    );
}

/// M4 knife 5: HTMLMediaElement/HTMLVideoElement/HTMLImageElement
/// media/image family (htmlmedia-probe.js, 49 keys). Chrome owns the three
/// prototypes complete (53/16/30 own names; the engine had 25/5/3 with three
/// kind drifts: duration rw, sizes ro, the VID on* pair as data props).
/// gen/htmlmedia.gen.js installs the missing members, applies the kind fixes
/// and reseat()s each prototype into Chrome's exact own order; uniform
/// reflection is DATA (P.reflector — this family grew the unsigned-long /
/// crossorigin / url kinds), the rest delegates to the htmlmedia impl
/// module. media_late.js's HTMLVideoElement PiP block was deleted (it ran
/// after the platform bundle and clobbered the module's on* accessors);
/// those four members are platform-owned now.
///
/// KNOWN_GAPS ratchet: none. MED.order is compared CONSTANT-NORMALIZED: the
/// nine readiness/network constants are bootstrap-installed {value: N} =
/// non-configurable (and non-enumerable, where Chrome has them enumerable),
/// so no module-level pass can relocate them — they cluster at the front
/// while Chrome interleaves them mid-sequence. Every movable member's
/// position matches Chrome exactly; the constants' placement is a
/// bootstrap-layer fact (knife 5, engine ledger).
#[tokio::test(flavor = "current_thread")]
async fn htmlmedia_members_match_chrome_truth() {
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
    ];
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/htmlmedia-probe.js");
    let result = rt
        // call_function_on_remote invokes its argument as a FUNCTION, so the
        // promise-returning probe rides an arrow; await_promise settles the
        // decode/setSinkId/setMediaKeys faces (each races a 3s timeout).
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/htmlmedia-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        if k == "MED.order" {
            let strip = |v: &serde_json::Value| -> Vec<String> {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str().unwrap().to_string())
                    .filter(|n| !IMMOVABLE.contains(&n.as_str()))
                    .collect()
            };
            let (e, t) = (strip(got), strip(want));
            if e != t {
                bad.push(format!(
                    "MED.order (constant-normalized):\n   engine {e:?}\n   chrome {t:?}"
                ));
            }
            continue;
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

/// M4 knife 18: EditContext family (editctx-probe.js, 49 keys, real Chrome 145):
/// shape of EditContext / TextFormat / TextUpdateEvent / TextFormatUpdateEvent /
/// CharacterBoundsUpdateEvent, EditContextInit conversions, updateText (reversed
/// range swapped, ends clamped), updateSelection (clamped, order kept), read-only
/// text/selection, fresh characterBounds copies, DOMRect argument checks, the five
/// handler accessors, HTMLElement.editContext attach rules (one element per
/// context, NotSupportedError for unsupported elements, conversion TypeError).
#[tokio::test(flavor = "current_thread")]
async fn editcontext_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/editctx-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/editctx-truth-145.json"
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

/// VTTCue was an inert stub that threw "3 arguments required, but only 0
/// present." for every call, so hls.js (`new VTTCue(0, 0, "")` at module top
/// level) killed whole pages (WeChat store, 2026-11-03). Expected values are
/// real-Chrome probe answers (Chrome 156, headless, same script).
#[tokio::test(flavor = "current_thread")]
async fn vttcue_constructs_like_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { const v = f(); o[k] = v === undefined ? 'undefined' : v; } catch (e) { o[k] = 'THROW ' + e.name + ': ' + e.message; } };
        t('noargs', () => new VTTCue());
        t('two', () => new VTTCue(1, 2));
        t('call', () => VTTCue(1, 2, 'x'));
        t('nonfinite_start', () => new VTTCue(NaN, 2, 'x'));
        t('inf_end', () => String(new VTTCue(0, Infinity, 'x').endTime));
        t('nan_end', () => new VTTCue(0, NaN, 'x'));
        t('neg', () => { const c = new VTTCue(-1, -2, 5); return [c.startTime, c.endTime, c.text]; });
        const c = new VTTCue(1.5, 3, 'hi <b>x</b>');
        t('defaults', () => [c.id, c.startTime, c.endTime, c.text, c.track, c.pauseOnExit, c.vertical, c.snapToLines, c.line, c.position, c.size, c.align, c.onenter, c.onexit]);
        t('own', () => Reflect.ownKeys(c).length);
        t('keys', () => { const k = []; for (const x in c) k.push(x); return k.slice(0, 15); });
        t('proto', () => [Object.getPrototypeOf(c) === VTTCue.prototype, Object.getPrototypeOf(VTTCue.prototype) === TextTrackCue.prototype, String(c), c instanceof EventTarget]);
        t('set_id', () => { c.id = 5; return c.id; });
        t('set_start_nan', () => { c.startTime = NaN; });
        t('set_end_inf', () => { c.endTime = Infinity; return String(c.endTime); });
        t('set_pause', () => { c.pauseOnExit = 'a'; return c.pauseOnExit; });
        t('vert', () => { c.vertical = 'zz'; const a = c.vertical; c.vertical = 'rl'; return [a, c.vertical]; });
        t('snap', () => { c.snapToLines = 0; return c.snapToLines; });
        t('line', () => { c.line = 3.5; const a = c.line; c.line = 'auto'; return [a, c.line]; });
        t('line_str', () => { c.line = 'x'; });
        t('line_nan', () => { c.line = NaN; });
        t('line_obj', () => { c.line = {}; });
        t('pos', () => { c.position = 50; const a = c.position; c.position = 'auto'; return [a, c.position]; });
        t('pos_200', () => { c.position = 200; });
        t('size', () => { c.size = 50; return c.size; });
        t('size_neg', () => { c.size = -1; });
        t('size_str', () => { c.size = 'auto'; });
        t('align', () => { c.align = 'zz'; const a = c.align; c.align = 'left'; return [a, c.align]; });
        t('text_set', () => { c.text = 7; return c.text; });
        t('html', () => { const f = new VTTCue(0, 1, 'a <b>bold</b> &amp; c').getCueAsHTML(); return [String(f), f.childNodes.length, [...f.childNodes].map(n => n.nodeName + ':' + n.textContent)]; });
        t('html_bad_this', () => VTTCue.prototype.getCueAsHTML.call({}));
        t('get_bad_this', () => Object.getOwnPropertyDescriptor(VTTCue.prototype, 'text').get.call({}));
        t('tt_bad_this', () => Object.getOwnPropertyDescriptor(TextTrackCue.prototype, 'startTime').get.call({}));
        t('ttc_new', () => new TextTrackCue());
        t('len', () => [VTTCue.length, VTTCue.name, String(VTTCue)]);
        t('proto_keys', () => Object.getOwnPropertyNames(VTTCue.prototype));
        t('tt_keys', () => Object.getOwnPropertyNames(TextTrackCue.prototype));
        t('coerce', () => { const x = new VTTCue('2', '3', null); return [x.startTime, x.endTime, x.text, new VTTCue(0, 1, undefined).text]; });
        t('sub', () => { class S extends VTTCue {} const s = new S(0, 1, 'q'); return [s instanceof S, s.text]; });
        t('events', () => { const x = new VTTCue(0, 1, ''); let n = 0; x.onenter = () => n++; x.addEventListener('enter', () => n++); x.dispatchEvent(new Event('enter')); return n; });
        t('hls_probe', () => { const e = VTTCue; try { new e(0, Number.POSITIVE_INFINITY, ''); return 'inf ok'; } catch (_) { return 'fallback'; } });
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["noargs"],
        "THROW TypeError: Failed to construct 'VTTCue': 3 arguments required, but only 0 present."
    );
    assert_eq!(
        o["two"],
        "THROW TypeError: Failed to construct 'VTTCue': 3 arguments required, but only 2 present."
    );
    assert_eq!(
        o["call"],
        "THROW TypeError: Failed to construct 'VTTCue': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
    );
    assert_eq!(
        o["nonfinite_start"],
        "THROW TypeError: Failed to construct 'VTTCue': The provided double value is non-finite."
    );
    assert_eq!(o["inf_end"], "Infinity");
    assert_eq!(
        o["nan_end"],
        "THROW TypeError: Failed to construct 'VTTCue': The endTime is not a number."
    );
    assert_eq!(o["neg"], serde_json::json!([-1, -2, "5"]));
    assert_eq!(
        o["defaults"],
        serde_json::json!([
            "",
            1.5,
            3,
            "hi <b>x</b>",
            null,
            false,
            "",
            true,
            "auto",
            "auto",
            100,
            "center",
            null,
            null
        ])
    );
    assert_eq!(o["own"], 0);
    assert_eq!(
        o["keys"],
        serde_json::json!([
            "vertical",
            "snapToLines",
            "line",
            "position",
            "size",
            "align",
            "text",
            "getCueAsHTML",
            "track",
            "id",
            "startTime",
            "endTime",
            "pauseOnExit",
            "onenter",
            "onexit"
        ])
    );
    assert_eq!(
        o["proto"],
        serde_json::json!([true, true, "[object VTTCue]", true])
    );
    assert_eq!(o["set_id"], "5");
    assert_eq!(
        o["set_start_nan"],
        "THROW TypeError: Failed to set the 'startTime' property on 'TextTrackCue': The provided double value is non-finite."
    );
    assert_eq!(o["set_end_inf"], "Infinity");
    assert_eq!(o["set_pause"], true);
    assert_eq!(o["vert"], serde_json::json!(["", "rl"]));
    assert_eq!(o["snap"], false);
    assert_eq!(o["line"], serde_json::json!([3.5, "auto"]));
    assert_eq!(
        o["line_str"],
        "THROW TypeError: Failed to set the 'line' property on 'VTTCue': The provided value 'x' is not a valid enum value of type AutoKeyword."
    );
    assert_eq!(
        o["line_nan"],
        "THROW TypeError: Failed to set the 'line' property on 'VTTCue': The provided double value is non-finite."
    );
    assert_eq!(
        o["line_obj"],
        "THROW TypeError: Failed to set the 'line' property on 'VTTCue': The provided value '[object Object]' is not a valid enum value of type AutoKeyword."
    );
    assert_eq!(o["pos"], serde_json::json!([50, "auto"]));
    assert_eq!(
        o["pos_200"],
        "THROW IndexSizeError: Failed to set the 'position' property on 'VTTCue': The value provided (200) is outside the range [0, 100]."
    );
    assert_eq!(o["size"], 50);
    assert_eq!(
        o["size_neg"],
        "THROW IndexSizeError: Failed to set the 'size' property on 'VTTCue': The value provided (-1) is outside the range [0, 100]."
    );
    assert_eq!(
        o["size_str"],
        "THROW TypeError: Failed to set the 'size' property on 'VTTCue': The provided double value is non-finite."
    );
    assert_eq!(o["align"], serde_json::json!(["center", "left"]));
    assert_eq!(o["text_set"], "7");
    assert_eq!(
        o["html"],
        serde_json::json!([
            "[object DocumentFragment]",
            3,
            ["#text:a ", "B:bold", "#text: & c"]
        ])
    );
    assert_eq!(o["html_bad_this"], "THROW TypeError: Illegal invocation");
    assert_eq!(o["get_bad_this"], "THROW TypeError: Illegal invocation");
    assert_eq!(o["tt_bad_this"], "THROW TypeError: Illegal invocation");
    assert_eq!(
        o["ttc_new"],
        "THROW TypeError: Failed to construct 'TextTrackCue': Illegal constructor"
    );
    assert_eq!(
        o["len"],
        serde_json::json!([3, "VTTCue", "function VTTCue() { [native code] }"])
    );
    assert_eq!(
        o["proto_keys"],
        serde_json::json!([
            "vertical",
            "snapToLines",
            "line",
            "position",
            "size",
            "align",
            "text",
            "getCueAsHTML",
            "constructor"
        ])
    );
    assert_eq!(
        o["tt_keys"],
        serde_json::json!([
            "track",
            "id",
            "startTime",
            "endTime",
            "pauseOnExit",
            "onenter",
            "onexit",
            "constructor"
        ])
    );
    assert_eq!(o["coerce"], serde_json::json!([2, 3, "null", "undefined"]));
    assert_eq!(o["sub"], serde_json::json!([true, "q"]));
    assert_eq!(o["events"], 2);
    assert_eq!(o["hls_probe"], "inf ok");
}

/// Issue #67 marginal-cost family: VTTCue is generated (gen/vttcue.gen.js from
/// the webvtt IDL slice, with NO generator change) plus a behavior module
/// (platform/vttcue.impl.js). tools/webplatform/chrome/vttcue-probe.js is
/// diffed key by key against real Chrome 157 (vttcue-truth-157.json), no
/// known gaps. The hand-written face this replaced was off on four keys:
/// argument conversion order (endTime's NaN check fired before text was
/// converted), the Symbol messages' ctx prefix, a Symbol text taken silently,
/// and the message own-property enumerability.
#[test]
fn vttcue_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/vttcue-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/vttcue-truth-157.json"
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
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
