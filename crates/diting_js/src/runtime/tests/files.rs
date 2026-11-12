//! Blob/File/FileList/DataTransfer/structuredClone/TextDecoder/btoa
use super::super::*;
use super::common::setup_runtime;

/// #258: `FileList` is a real global — boot-time feature detects and
/// `instanceof` guards ReferenceError without it (obscura #1232).
/// Constructing throws like Chrome's; `input.files` keeps its shape and
/// now carries the prototype, so instanceof holds.
#[tokio::test(flavor = "current_thread")]
async fn filelist_global_present_and_files_instanceof() {
    let mut rt = setup_runtime("<html><body><input id=\"f\" type=\"file\"></body></html>");
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const files = document.getElementById('f').files;
                    let ctorThrew = 'no';
                    try { new FileList(); } catch (e) { ctorThrew = e instanceof TypeError ? 'TypeError' : String(e).slice(0, 30); }
                    return {
                        typeofGlobal: typeof FileList,
                        instanceof: String(files instanceof FileList),
                        tag: Object.prototype.toString.call(files),
                        length: String(files.length),
                        item: String(files.item(0)),
                        ctorThrew,
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
    assert_eq!(v["typeofGlobal"], serde_json::json!("function"));
    assert_eq!(v["instanceof"], serde_json::json!("true"));
    assert_eq!(v["tag"], serde_json::json!("[object FileList]"));
    assert_eq!(v["length"], serde_json::json!("0"));
    assert_eq!(v["item"], serde_json::json!("null"));
    assert_eq!(v["ctorThrew"], serde_json::json!("TypeError"));
}

#[test]
fn test_structured_clone_preserves_buffers_and_collections() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // The old JSON.parse(JSON.stringify) fallback dropped ArrayBuffer and
    // TypedArray to {}. Real structuredClone keeps them intact.
    let result = rt
        .evaluate(
            r#"
            const ab = new ArrayBuffer(4);
            new Uint8Array(ab).set([1, 2, 3, 4]);
            const c = structuredClone({
                buf: ab,
                view: new Uint16Array([5, 6]),
                map: new Map([["k", new Uint8Array([7])]]),
                set: new Set([8]),
                date: new Date(0),
                re: /ab+c/gi,
            });
            return [
                c.buf instanceof ArrayBuffer,
                Array.from(new Uint8Array(c.buf)),
                c.view instanceof Uint16Array,
                Array.from(c.view),
                Array.from(c.map.get("k")),
                c.set.has(8),
                c.date.getTime(),
                c.re.source,
                c.re.flags,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, [1, 2, 3, 4], true, [5, 6], [7], true, 0, "ab+c", "gi"])
    );
}

#[test]
fn test_structured_clone_handles_cycles_and_error_cause() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // Cycles preserve identity; an Error whose `cause` points back at
    // itself must not recurse until stack overflow.
    let result = rt
        .evaluate(
            r#"
            const obj = { name: 'a' };
            obj.self = obj;
            const c = structuredClone(obj);
            const cycleOk = c.self === c && c.name === 'a' && c !== obj;

            const err = new Error('boom');
            err.cause = err;
            const ec = structuredClone(err);
            const causeOk = ec.cause === ec && ec.message === 'boom';
            return [cycleOk, causeOk];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([true, true]));
}

#[test]
fn test_structured_clone_own_proto_and_function_rejection() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // An own enumerable `__proto__` data property (what JSON.parse yields)
    // must clone as an own property, not reparent the clone. Functions are
    // not structured-cloneable and must throw DataCloneError.
    let result = rt
        .evaluate(
            r#"
            const obj = JSON.parse('{"__proto__": {"x": 1}, "y": 2}');
            const c = structuredClone(obj);
            const protoOk = Object.getPrototypeOf(c) === Object.prototype
                && c.y === 2
                && c.__proto__.x === 1;

            let threw = false;
            try { structuredClone({ f: function() {} }); } catch (e) {
                threw = e instanceof DOMException && e.name === "DataCloneError";
            }
            return [protoOk, threw];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([true, true]));
}

#[test]
fn btoa_atob_latin1_contract() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // Latin1 per the HTML spec: btoa("é") is the single byte 0xE9 →
    // "6Q==", not the two-byte UTF-8 sequence ("w6k="), and anything
    // above 0xFF throws InvalidCharacterError. atob must reject
    // non-alphabet input (it used to decode garbage through indexOf's
    // -1) while ignoring ASCII whitespace like Chrome.
    assert_eq!(
        rt.evaluate("btoa('hello')").unwrap(),
        serde_json::json!("aGVsbG8=")
    );
    assert_eq!(
        rt.evaluate("btoa('\\u00e9')").unwrap(),
        serde_json::json!("6Q==")
    );
    assert_eq!(
        rt.evaluate("try { btoa('\\u4e2d') } catch (e) { e.name }")
            .unwrap(),
        serde_json::json!("InvalidCharacterError")
    );
    assert_eq!(
        rt.evaluate("atob('6Q==')").unwrap(),
        serde_json::json!("\u{e9}")
    );
    assert_eq!(
        rt.evaluate("try { atob('aGVs!!o') } catch (e) { e.name }")
            .unwrap(),
        serde_json::json!("InvalidCharacterError")
    );
    // A lone trailing char is not a decodable quantum.
    assert_eq!(
        rt.evaluate("try { atob('Q') } catch (e) { e.name }")
            .unwrap(),
        serde_json::json!("InvalidCharacterError")
    );
    // ASCII whitespace is ignored; the four chars here decode "hel".
    assert_eq!(
        rt.evaluate("atob('aG Vs\\n')").unwrap(),
        serde_json::json!("hel")
    );
    assert_eq!(
        rt.evaluate("atob(btoa('AB'))").unwrap(),
        serde_json::json!("AB")
    );
    // WebIDL DOMString: the argument is ToString-coerced before the
    // Latin-1 scan, so numbers/arrays encode their string form and the
    // >0xFF throw still fires on a coerced value (issue #22).
    assert_eq!(rt.evaluate("btoa(123)").unwrap(), serde_json::json!("MTIz"));
    assert_eq!(
        rt.evaluate("btoa([1,2])").unwrap(),
        serde_json::json!("MSwy")
    );
    assert_eq!(
        rt.evaluate("try { btoa({toString: () => '\\u4e2d'}) } catch (e) { e.name }")
            .unwrap(),
        serde_json::json!("InvalidCharacterError")
    );
    // Large payloads must survive: the decoder used to finish with
    // String.fromCharCode(...bytes), whose spread blew the call stack on
    // real upload sizes (file content arrives through this path).
    assert_eq!(
            rt.evaluate(
                "(function () { const s = 'A'.repeat(262144); \
                 return atob(btoa(s)).length === s.length ? 'ok' : 'len:' + atob(btoa(s)).length; })()"
            )
            .unwrap(),
            serde_json::json!("ok")
        );
}

/// input.files is the file-upload leg: assignment stores File objects per
/// node (stable-nid store, same pattern as _formValues), value synthesizes
/// Chrome's fakepath string, and FormData(form) turns the selection into
/// real multipart File entries — an unset file input still contributes an
/// empty octet-stream part like the spec's "constructing the form data set".
#[test]
fn file_input_files_value_and_formdata() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate(
                r#"(function () {
            document.body.innerHTML =
                '<form id="f"><input type="file" name="up" id="file1">' +
                '<input type="text" name="t" id="txt" value="x">' +
                '<input type="file" name="empty" id="file2"></form>';
            const out = [];
            const f1 = document.getElementById('file1');
            out.push('len0:' + f1.files.length);
            out.push('val0:' + f1.value);
            out.push('textfiles:' + (document.getElementById('txt').files === null));
            const fileA = new File([new Uint8Array([104, 105])], 'a.png', { type: 'image/png' });
            const fileB = new File([new Uint8Array([66])], 'b.jpg');
            f1.files = [fileA, fileB];
            const fl = f1.files;
            out.push('len:' + fl.length);
            out.push('item:' + fl.item(0).name + ',' + fl.item(1).name + ',oob:' + fl.item(5));
            out.push('idx:' + fl[0].name + '/' + fl[1].name);
            const names = []; for (const f of fl) names.push(f.name);
            out.push('iter:' + names.join('+'));
            out.push('tag:' + Object.prototype.toString.call(fl));
            out.push('value:' + f1.value);
            out.push('size:' + fileA.size + ',type:' + fileA.type);
            // Junk tolerance: non-array-like and non-File entries are dropped
            // silently, the way Chrome tolerates polyfill reassignments. An
            // ignored assignment keeps the previous selection intact.
            f1.files = 'nope';
            out.push('junkstr:' + f1.files.length);
            f1.files = [fileA, 'string', null];
            out.push('junkmix:' + f1.files.length + ':' + f1.files.item(0).name);
            // Value assignment: non-empty throws InvalidStateError (Chrome
            // message contract), empty clears the selection.
            let threw = ''; try { f1.value = 'C:\\x'; } catch (e) { threw = e.name; }
            out.push('setthrow:' + threw);
            f1.value = '';
            out.push('cleared:' + f1.files.length + ':' + f1.value);
            // FormData(form): the selection rides as File entries, an unset
            // file input still yields an empty octet-stream part.
            f1.files = [fileA];
            const entries = [];
            for (const [k, v] of new FormData(document.getElementById('f')).entries())
                entries.push(k + '=' + (typeof File === 'function' && v instanceof File ? 'File(' + v.name + ',' + v.type + ',' + v.size + ')' : v));
            out.push('fd:' + entries.join('|'));
            return out.join('\n');
        })()"#,
            )
            .unwrap();
    let text = result.as_str().unwrap();
    let expected = [
        "len0:0",
        "val0:",
        "textfiles:true",
        "len:2",
        "item:a.png,b.jpg,oob:null",
        "idx:a.png/b.jpg",
        "iter:a.png+b.jpg",
        "tag:[object FileList]",
        "value:C:\\fakepath\\a.png",
        "size:2,type:image/png",
        "junkstr:2",
        "junkmix:1:a.png",
        "setthrow:InvalidStateError",
        "cleared:0:",
        "fd:up=File(a.png,image/png,2)|t=x|empty=File(,application/octet-stream,0)",
    ]
    .join("\n");
    assert_eq!(text, expected);
}

#[tokio::test(flavor = "current_thread")]
async fn test_structured_clone_preserves_cryptokey_identity() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // A CryptoKey reached twice in a graph must clone to one shared object
    // that crypto.subtle still accepts (upstream 8698afc + a921668).
    let script = r#"async () => {
            const enc = new TextEncoder();
            const key = await crypto.subtle.importKey('raw', enc.encode('k'), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
            const c = structuredClone({ a: key, b: key });
            const sameObject = c.a === c.b;
            // The clone stays usable by crypto.subtle (key material re-registered).
            const sig = await crypto.subtle.sign('HMAC', c.a, enc.encode('msg'));
            return [sameObject, sig instanceof ArrayBuffer, c.a instanceof CryptoKey];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([true, true, true]));
}

#[tokio::test(flavor = "current_thread")]
async fn release_object_with_adversarial_id_is_inert() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.release_object("x']; globalThis.__injected = 42; ('");
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

// Blob URLs must look like Chrome's: `blob:<document origin>/<uuid>`.
// The pre-rename engine handed out `blob:obscura/<base36>`, so a page
// reading back its own blob URL (a Worker's script URL, an anchor href,
// performance entries) could name the engine with one startsWith.
// Non-Blob input throws Chrome's TypeErrors instead of minting a
// fallback URL, and revoke drops the blob from both stores so a
// revoked URL can no longer construct a Worker synchronously.
#[test]
fn blob_urls_are_chrome_shaped_and_non_blob_input_throws() {
    let mut rt = JsRuntime::with_base_url("https://example.com/page");
    let result = rt
            .evaluate(
                r#"return (() => {
                    // with_base_url only feeds the module loader; the realm's
                    // location reads __virtualUrl/document_url. Pin it so the
                    // minted blob URL carries a real origin, like a page.
                    globalThis.__virtualUrl = 'https://example.com/page';
                    const out = {};
                    const url = URL.createObjectURL(new Blob(['hello'], {type: 'text/plain'}));
                    out.url = url;
                    out.shaped = /^blob:https:\/\/example\.com\/[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(url);
                    out.inStore = !!globalThis.__blobObjs[url];
                    URL.revokeObjectURL(url);
                    out.afterRevoke = !!globalThis.__blobObjs[url];
                    out.missingArg = (() => { try { URL.createObjectURL(); return 'no-throw'; } catch (e) { return e.name + '|' + e.message; } })();
                    out.wrongType = (() => { try { URL.createObjectURL('nope'); return 'no-throw'; } catch (e) { return e.name + '|' + e.message; } })();
                    return out;
                })()"#,
            )
            .unwrap();
    let out = result.as_object().expect("object result");
    let url = out["url"].as_str().unwrap();
    assert_eq!(
        out["shaped"],
        serde_json::json!(true),
        "blob url not chrome-shaped: {url}"
    );
    assert_eq!(out["inStore"], serde_json::json!(true));
    assert_eq!(out["afterRevoke"], serde_json::json!(false));
    assert_eq!(
        out["missingArg"],
        serde_json::json!(
            "TypeError|Failed to execute 'createObjectURL' on 'URL': 1 argument required, but only 0 present."
        )
    );
    assert_eq!(
        out["wrongType"],
        serde_json::json!(
            "TypeError|Failed to execute 'createObjectURL' on 'URL': parameter 1 is not of type 'Blob'."
        )
    );
}

// x.com's video APPEND chunker (2026-11-09) was the first page-side
// Blob.prototype.slice caller. lift_own's RO prototype size getter has no
// setter, and slice()'s old direct `out.size =` assignment threw in strict
// realms ("Cannot set property size of #<Blob> which has only a getter"),
// killing the upload pipeline before INIT ever fired. slice must write the
// lift slot like the constructor; size itself stays read-only.
#[test]
fn blob_slice_survives_strict_realms_and_sizes_chunks() {
    let mut rt = JsRuntime::with_base_url("https://example.com/page");
    let result = rt
            .evaluate(
                r#"return (() => {
                    'use strict';
                    const out = {};
                    const b = new Blob(['0123456789abcdef'], {type: 'text/plain'});
                    const mid = b.slice(4, 8);
                    out.mid = [mid.size, mid.type];
                    out.typed = b.slice(0, 4, 'TEXT/PLAIN').type;
                    out.tail = b.slice(-3).size;
                    out.clamped = b.slice(8, 4).size;
                    out.isBlob = mid instanceof Blob;
                    out.readsBack = mid.slice(0, 2).size;
                    const f = new File(['xyz'], 'a.bin', {type: 'application/octet-stream'});
                    out.fileSlice = [f.slice(1).size, f.name, f.size];
                    out.assignStillThrows = (() => { try { b.size = 5; return 'no-throw'; } catch (e) { return e.name; } })();
                    return out;
                })()"#,
            )
            .unwrap();
    let out = result.as_object().expect("object result");
    assert_eq!(out["mid"], serde_json::json!([4, ""]));
    assert_eq!(out["typed"], serde_json::json!("text/plain"));
    assert_eq!(out["tail"], serde_json::json!(3));
    assert_eq!(out["clamped"], serde_json::json!(0));
    assert_eq!(out["isBlob"], serde_json::json!(true));
    assert_eq!(out["readsBack"], serde_json::json!(2));
    assert_eq!(out["fileSlice"], serde_json::json!([2, "a.bin", 3]));
    assert_eq!(
        out["assignStillThrows"],
        serde_json::json!("TypeError"),
        "size must stay read-only — the fix rides the lift slot, not an own property"
    );
}

#[test]
fn test_text_decoder_respects_typed_array_view() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate("new TextDecoder().decode(new Uint8Array([65, 66, 67]).subarray(1, 2))")
        .unwrap();
    assert_eq!(result.as_str().unwrap(), "B");
}

// PDD "invalid wire type": binary must stay bytes through every body path.
#[test]
fn test_text_decoder_malformed_utf8_gets_replacement_chars() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // lone continuation, overlong C0 80, surrogate ED A0 80, truncated E2 82, F5 (invalid lead)
    let r = rt.evaluate(
            "Array.from(new TextDecoder().decode(new Uint8Array([0x61,0x80,0x62,0xC0,0x80,0xED,0xA0,0x80,0xE2,0x82,0x63,0xF5,0x64]))).map(c=>c.charCodeAt(0).toString(16)).join(',')"
        ).unwrap();
    assert_eq!(
        r.as_str().unwrap(),
        "61,fffd,62,fffd,fffd,fffd,fffd,fffd,fffd,63,fffd,64"
    );
}

// Native DOMException keeps name/message in slots (accessors, no setters):
// structuredClone must rebuild it via the clone hook, not assign onto a copy.
#[test]
fn test_structured_clone_of_native_dom_exception_and_geometry() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let r = rt.evaluate(
            "(function(){ var e = new DOMException('m','AbortError'); var c = structuredClone({e:e, r:new DOMRect(1,2,3,4), p:new DOMPoint(5,6,7,8)}); \
              return [c.e.name, c.e.message, c.e.code, c.e !== e, c.e instanceof DOMException, c.r.right, c.p.w, Object.getOwnPropertyNames(c.e).length].join(','); })()"
        ).unwrap();
    assert_eq!(r.as_str().unwrap(), "AbortError,m,20,true,true,4,8,0");
}

/// 2026-10-18 sweep (moli 785d6da): WebIDL converts Blob arguments left to
/// right — every part's union conversion (ToString side effects included)
/// fires before the options dictionary is read, and each bag member getter
/// fires exactly once. Chrome pins part-toString > endings > type; the old
/// `x != null ? String(x)` shape read each getter twice.
#[test]
fn blob_constructor_conversion_order_and_single_getters() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () { try {
        const log = [];
        const part = { toString() { log.push('part'); return 'x'; } };
        const bag = {
            get endings() { log.push('endings'); return 'native'; },
            get type() { log.push('type'); return 'TEXT/PLAIN'; },
        };
        const b = new Blob(['a\r\nb', part], bag);
        globalThis.__out = JSON.stringify([log.join('>'), b.size, b.type]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#,
        )
        .unwrap();
    let s = out.as_str().unwrap();
    assert!(!s.starts_with("ERR"), "blob order probe threw: {}", s);
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!(["part>endings>type", 4, "text/plain"]),
        "parts convert before the bag (native \\r\\n folded), each bag getter fires once, type lowercases"
    );
}

/// #57: `input.files = singleFile` — Chrome accepts a bare File as a
/// one-entry selection; the silent no-op cost the xhs send batch a debug
/// cycle. The wrapped FileList keeps length/item/index/iteration shape.
#[test]
fn input_files_accepts_bare_file() {
    let mut rt = setup_runtime("<html><body><input id=\"f\" type=\"file\"></body></html>");
    let _out = rt
        .evaluate(
            r#"
        const inp = document.getElementById('f');
        const before = inp.files.length;
        const file = new File(['abc'], 'a.txt', { type: 'text/plain' });
        inp.files = file;
        let iterated = [];
        for (const f of inp.files) iterated.push(f.name);
        globalThis.__out = JSON.stringify([
            before, inp.files.length, inp.files[0] === file,
            inp.files.item(0).name, inp.files.item(9), iterated,
        ]);
    "#,
        )
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(rt.evaluate("globalThis.__out").unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v,
        serde_json::json!([0, 1, true, "a.txt", null, ["a.txt"]]),
        "a bare File must land as a one-entry FileList with full list shape"
    );
}

/// #58: DataTransfer is zero-arg constructible (since Chrome 60) and the
/// standard injection pattern `dt.items.add(file); input.files = dt.files`
/// must carry the file through. Indexed item access reads the live list.
#[test]
fn data_transfer_items_and_files_feed_input() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let _out = rt
        .evaluate(
            r#"
        const dt = new DataTransfer();
        const empty = dt.files.length;
        const file = new File(['x'], 'x.txt');
        const added = dt.items.add(file);
        const oneFile = dt.files.length;
        dt.items.add('hello');
        const types = Array.from(dt.types);
        const itemsIdentity = dt.items === dt.items;
        const idx = dt.items[0];
        const inp = document.createElement('input');
        inp.type = 'file';
        document.body.appendChild(inp);
        inp.files = dt.files;
        globalThis.__out = JSON.stringify([
            empty, oneFile, types, itemsIdentity,
            idx && idx.kind, idx && idx.getAsFile() === file,
            inp.files.length, inp.files[0] === file,
            added && added.kind,
        ]);
    "#,
        )
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(rt.evaluate("globalThis.__out").unwrap().as_str().unwrap()).unwrap();
    assert_eq!(
        v,
        serde_json::json!([
            0,
            1,
            ["text/plain", "Files"],
            true,
            "file",
            true,
            1,
            true,
            "file"
        ]),
        "DataTransfer must support add(file)/add(str), live indexed items, and feed input.files"
    );
}
