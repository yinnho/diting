//! WebIDL 真值电池 knife67-87
use super::common::{setup_runtime, STORAGE_ENV_LOCK};

// 刀67: VideoFrame 13 件（WebCodecs 值类型大户）——Chrome 145 真值四轮探针
// （k67-chrome{,2,3,4}）：双 ctor 路（canvas 真 BGRA 像素 / BufferSource+
// VideoFrameBufferInit 验证梯）、平面数学（allocationSize/copyTo layouts）、
// closed 矩阵（ts/duration 存活、几何归零、rects null、colorSpace 全 null）、
// visibleRect 收缩（帧塌到可见矩形、原点重锚、数据尺寸按解析态 coded 验）、
// VideoColorSpace 行为接管（ctor 存 init、成员默认 null、toJSON 序）。
#[test]
fn knife67_videoframe_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const t = (f) => { try { const v = f(); return v === undefined ? 'undef' : (v === null ? 'null' : v); } catch (e) { return 'E:' + e.name + ':' + e.message; } };
          const out = {};
          const P = VideoFrame.prototype;
          out.has13 = ['format','timestamp','duration','codedWidth','codedHeight','codedRect','visibleRect','displayWidth','displayHeight','colorSpace','rotation','flip','allocationSize','clone','copyTo','metadata','close'].filter((n) => Object.getOwnPropertyNames(P).indexOf(n) < 0);
          out.ctorLens = [VideoFrame.length, VideoColorSpace.length];
          const cv = document.createElement('canvas'); cv.width = 4; cv.height = 2;
          // ---- ctor ladder: exact Chrome messages --------------------------
          out.noarg = t(() => new VideoFrame());
          out.garbage = t(() => new VideoFrame(42));
          out.bufNoOpts = t(() => new VideoFrame(new Uint8Array(32)));
          out.bufNullOpts = t(() => new VideoFrame(new Uint8Array(32), null));
          out.noCh = t(() => new VideoFrame(new Uint8Array(32), {}));
          out.noFmt = t(() => new VideoFrame(new Uint8Array(32), { codedWidth: 4, codedHeight: 2, timestamp: 0 }));
          out.badFmt = t(() => new VideoFrame(new Uint8Array(32), { format: 'ZZZ', codedWidth: 4, codedHeight: 2, timestamp: 0 }));
          out.cw0 = t(() => new VideoFrame(new Uint8Array(0), { format: 'RGBA', codedWidth: 0, codedHeight: 2, timestamp: 0 }));
          out.badBuf = t(() => new VideoFrame(new Uint8Array(3), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0 }));
          out.visRight = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, visibleRect: { x: 0, y: 0, width: 9, height: 2 } }));
          out.visBottom = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, visibleRect: { x: 0, y: 0, width: 4, height: 9 } }));
          out.visYNeg = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, visibleRect: { x: 0, y: -1, width: 2, height: 2 } }));
          out.visW0 = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, visibleRect: { x: 0, y: 0, width: 0, height: 2 } }));
          out.dispWOnly = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, displayWidth: 8 }));
          out.dispHNeg = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, displayWidth: -1, displayHeight: 2 }));
          out.durNeg = t(() => new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, duration: -1 }));
          out.noTsCanvas = t(() => new VideoFrame(cv));
          // ---- happy paths ---------------------------------------------------
          const vf = new VideoFrame(cv, { timestamp: 7, duration: 9 });
          out.canvas = [vf.format, vf.codedWidth, vf.codedHeight, vf.displayWidth, vf.displayHeight, vf.rotation, vf.flip, vf.timestamp, vf.duration];
          out.alloc = [vf.allocationSize(), new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0 }).allocationSize(), new VideoFrame(new Uint8Array(12), { format: 'I420', codedWidth: 4, codedHeight: 2, timestamp: 0 }).allocationSize(), new VideoFrame(new Uint8Array(12), { format: 'NV12', codedWidth: 4, codedHeight: 2, timestamp: 0 }).allocationSize()];
          out.rects = [vf.codedRect.constructor.name, vf.codedRect.x, vf.codedRect.width, vf.visibleRect === vf.visibleRect, vf.codedRect === vf.codedRect, vf.colorSpace === vf.colorSpace];
          out.cs = JSON.parse(JSON.stringify(vf.colorSpace));
          out.csCtor = [vf.colorSpace.constructor.name, JSON.stringify(new VideoColorSpace({ primaries: 'bt709' })), JSON.stringify(new VideoColorSpace({ fullRange: 'x' })), JSON.stringify(new VideoColorSpace()), Object.getOwnPropertyNames(new VideoColorSpace()).length];
          const csd = Object.getOwnPropertyDescriptor(VideoColorSpace.prototype, 'primaries');
          out.csDsc = [!!csd.get, csd.get.name, csd.enumerable, csd.configurable];
          out.csTag = VideoColorSpace.prototype[Symbol.toStringTag];
          // visibleRect collapse: frame shrinks to the visible rect, re-anchored
          const v2 = new VideoFrame(new Uint8Array(48), { format: 'RGBA', codedWidth: 6, codedHeight: 2, timestamp: 0, visibleRect: { x: 3, y: 0, width: 2, height: 1 } });
          out.collapse = [v2.codedWidth, v2.codedHeight, v2.visibleRect.x, v2.visibleRect.y, v2.visibleRect.width, v2.displayWidth, v2.allocationSize()];
          out.snap = (() => { const f = new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, visibleRect: { x: 1, y: 1.9, width: 2.9, height: 1 } }); return [f.visibleRect.x, f.visibleRect.y, f.visibleRect.width]; })();
          out.rot = [new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, rotation: 45 }).rotation, new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, rotation: '90' }).rotation, new VideoFrame(new Uint8Array(32), { format: 'RGBA', codedWidth: 4, codedHeight: 2, timestamp: 0, flip: 'yes' }).flip, new VideoFrame(cv, { timestamp: '5' }).timestamp, new VideoFrame(cv, { timestamp: -5 }).timestamp];
          out.alpha = new VideoFrame(cv, { timestamp: 0, alpha: 'discard' }).format;
          out.fromFrame = (() => { const a = new VideoFrame(cv, { timestamp: 3, duration: 4 }); const b = new VideoFrame(a); return [b.format, b.codedWidth, b.timestamp, b.duration]; })();
          out.clone = (() => { const c = vf.clone(); a: { } return [c.constructor.name, c.format, c.timestamp === vf.timestamp, c === vf, (() => { vf.close(); return [c.format, c.codedWidth]; })()]; })();
          out.cloneI420 = (() => { const f = new VideoFrame(new Uint8Array(12), { format: 'I420', codedWidth: 4, codedHeight: 2, timestamp: 0 }); const c = f.clone(); return [c.format, c.allocationSize()]; })();
          const mf = new VideoFrame(cv, { timestamp: 0 });
          out.meta = [typeof mf.metadata(), mf.metadata().constructor === Object, Object.keys(mf.metadata()).length, mf.metadata() === mf.metadata()];
          // copyTo: promise identity, sync-NO-THROW on bad/small (async rejections)
          out.copyKind = [Object.prototype.toString.call(new VideoFrame(cv, { timestamp: 0 }).copyTo(new ArrayBuffer(64))), t(() => { new VideoFrame(cv, { timestamp: 0 }).copyTo({}); return 'NO-THROW'; }), t(() => { new VideoFrame(cv, { timestamp: 0 }).copyTo(new ArrayBuffer(4)); return 'NO-THROW'; })];
          // ---- closed matrix -------------------------------------------------
          const cf = new VideoFrame(cv, { timestamp: 7, duration: 9 });
          cf.close();
          out.closed = [cf.format, cf.timestamp, cf.duration, cf.codedWidth, cf.codedHeight, cf.displayWidth, cf.visibleRect, cf.codedRect, JSON.parse(JSON.stringify(cf.colorSpace)), cf.rotation, cf.flip];
          out.closedOps = [t(() => cf.allocationSize()), t(() => cf.clone()), t(() => cf.metadata()), t(() => { cf.copyTo(new ArrayBuffer(64)); return 'NO-THROW'; }), t(() => { cf.close(); return 'NO-THROW'; })];
          out.closedTwice = cf.format;
          // ---- illegal invocation ---------------------------------------------
          out.illegal = [t(() => Object.getOwnPropertyDescriptor(P, 'codedWidth').get.call({})), t(() => P.clone.call({})), t(() => P.copyTo.call({}, new ArrayBuffer(8)))];
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let g = |k: &str| v.get(k).cloned().unwrap_or(serde_json::Value::Null);
    assert_eq!(g("has13"), serde_json::json!([]));
    assert_eq!(g("ctorLens"), serde_json::json!([1, 0]));
    assert_eq!(
        g("noarg"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': 1 argument required, but only 0 present."
        )
    );
    assert_eq!(
        g("garbage"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': The provided value is not of type '(CSSImageValue or HTMLCanvasElement or HTMLImageElement or HTMLVideoElement or ImageBitmap or OffscreenCanvas or SVGImageElement or VideoFrame)'."
        )
    );
    assert_eq!(
        g("bufNoOpts"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': The provided value is not of type '(CSSImageValue or HTMLCanvasElement or HTMLImageElement or HTMLVideoElement or ImageBitmap or OffscreenCanvas or SVGImageElement or VideoFrame)'."
        )
    );
    assert_eq!(
        g("bufNullOpts"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': The provided value is not of type 'VideoFrameBufferInit'."
        )
    );
    assert_eq!(
        g("noCh"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Failed to read the 'codedHeight' property from 'VideoFrameBufferInit': Required member is undefined."
        )
    );
    assert_eq!(
        g("noFmt"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Failed to read the 'format' property from 'VideoFrameBufferInit': Required member is undefined."
        )
    );
    assert_eq!(
        g("badFmt"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Failed to read the 'format' property from 'VideoFrameBufferInit': The provided value 'ZZZ' is not a valid enum value of type VideoPixelFormat."
        )
    );
    assert_eq!(
        g("cw0"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': codedWidth must be nonzero."
        )
    );
    assert_eq!(
        g("badBuf"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': data is not large enough."
        )
    );
    assert_eq!(
        g("visRight"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Invalid visibleRect. right 9 exceeds codedWidth 4."
        )
    );
    assert_eq!(
        g("visBottom"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Invalid visibleRect. bottom 9 exceeds codedHeight 2."
        )
    );
    assert_eq!(
        g("visYNeg"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Invalid visibleRect. y cannot be negative."
        )
    );
    assert_eq!(
        g("visW0"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Invalid visibleRect. width must be nonzero."
        )
    );
    assert_eq!(
        g("dispWOnly"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': displayWidth specified without displayHeight."
        )
    );
    assert_eq!(
        g("dispHNeg"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Failed to read the 'displayWidth' property from 'VideoFrameBufferInit': Value is outside the 'unsigned long' value range."
        )
    );
    assert_eq!(
        g("durNeg"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': Failed to read the 'duration' property from 'VideoFrameBufferInit': Value is outside the 'unsigned long long' value range."
        )
    );
    assert_eq!(
        g("noTsCanvas"),
        serde_json::json!(
            "E:TypeError:Failed to construct 'VideoFrame': VideoFrameInit must provide timestamp"
        )
    );
    assert_eq!(
        g("canvas"),
        serde_json::json!(["BGRA", 4, 2, 4, 2, 0, false, 7, 9])
    );
    assert_eq!(g("alloc"), serde_json::json!([32, 32, 12, 12]));
    assert_eq!(
        g("rects"),
        serde_json::json!(["DOMRectReadOnly", 0, 4, true, true, true])
    );
    assert_eq!(
        g("cs"),
        serde_json::json!({"fullRange": true, "matrix": "rgb", "primaries": "bt709", "transfer": "iec61966-2-1"})
    );
    assert_eq!(
        g("csCtor"),
        serde_json::json!([
            "VideoColorSpace",
            "{\"fullRange\":null,\"matrix\":null,\"primaries\":\"bt709\",\"transfer\":null}",
            "{\"fullRange\":true,\"matrix\":null,\"primaries\":null,\"transfer\":null}",
            "{\"fullRange\":null,\"matrix\":null,\"primaries\":null,\"transfer\":null}",
            0
        ])
    );
    assert_eq!(
        g("csDsc"),
        serde_json::json!([true, "get primaries", true, true])
    );
    assert_eq!(g("csTag"), serde_json::json!("VideoColorSpace"));
    assert_eq!(g("collapse"), serde_json::json!([2, 1, 0, 0, 2, 2, 8]));
    assert_eq!(g("snap"), serde_json::json!([0, 0, 2]));
    assert_eq!(g("rot"), serde_json::json!([90, 90, true, 5, -5]));
    assert_eq!(g("alpha"), serde_json::json!("BGRX"));
    assert_eq!(g("fromFrame"), serde_json::json!(["BGRA", 4, 3, 4]));
    assert_eq!(
        g("clone"),
        serde_json::json!(["VideoFrame", "BGRA", true, false, ["BGRA", 4]])
    );
    assert_eq!(g("cloneI420"), serde_json::json!(["I420", 12]));
    assert_eq!(g("meta"), serde_json::json!(["object", true, 0, false]));
    assert_eq!(
        g("copyKind"),
        serde_json::json!(["[object Promise]", "NO-THROW", "NO-THROW"])
    );
    assert_eq!(
        g("closed"),
        serde_json::json!([null, 7, 9, 0, 0, 0, null, null, {"fullRange": null, "matrix": null, "primaries": null, "transfer": null}, 0, false])
    );
    assert_eq!(
        g("closedOps"),
        serde_json::json!([
            "E:InvalidStateError:Failed to execute 'allocationSize' on 'VideoFrame': VideoFrame is closed.",
            "E:InvalidStateError:Failed to execute 'clone' on 'VideoFrame': Cannot clone closed VideoFrame.",
            "E:InvalidStateError:Failed to execute 'metadata' on 'VideoFrame': VideoFrame is closed.",
            "NO-THROW",
            "NO-THROW"
        ])
    );
    assert_eq!(g("closedTwice"), serde_json::json!(null));
    assert_eq!(
        g("illegal"),
        serde_json::json!([
            "E:TypeError:Illegal invocation",
            "E:TypeError:Illegal invocation",
            "E:TypeError:Illegal invocation"
        ])
    );
}

/// M4 刀68: the IndexedDB family — state in private slots, attributes as
/// prototype accessors with pinned names, real EventTarget requests /
/// transactions / databases firing real Event / IDBVersionChangeEvent objects
/// with request → tx → db error bubbling, Chrome's error ladders and
/// transaction lifecycle (abort rolls back, a request error aborts unless
/// prevented), index cursors and the getAll/getAllRecords family.
#[tokio::test(flavor = "current_thread")]
async fn knife68_idb_family() {
    // The schema meta mirrors into localStorage (disk-backed, process-global):
    // hold the storage locks so this long-running test cannot flush into the
    // temp dir the localStorage persistence test points the store at.
    let _ephemeral = diting_env::env_knobs::EPHEMERAL_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _env = STORAGE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__k68-db')")
        .unwrap();
    rt.evaluate(r#"
        globalThis.__out = {};
        const o = __out;
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        t('illegal', () => { try { new IDBRequest(); return 'NO'; } catch (e) { return e.message; } });
        t('names', () => ['result', 'onsuccess'].map(k => { const d = Object.getOwnPropertyDescriptor(IDBRequest.prototype, k); return [d.get.name, d.set ? d.set.name : null]; }));
        t('foreign', () => { try { Object.getOwnPropertyDescriptor(IDBObjectStore.prototype, 'name').get.call({}); return 'NO'; } catch (e) { return e.message; } });
        t('range', () => { const k = IDBKeyRange.bound(1, 5, false, true); return [k.lower, k.upper, k.lowerOpen, k.upperOpen, k.includes(1), k.includes(5), Reflect.ownKeys(k).length]; });
        t('rangeErr', () => { try { IDBKeyRange.bound(5, 1); return 'NO'; } catch (e) { return e.message; } });
        t('cmp', () => [indexedDB.cmp(1, 2), indexedDB.cmp([1], [1, 0]), indexedDB.cmp('a', 1)]);
        t('vce', () => { const e = new IDBVersionChangeEvent('x', { oldVersion: 1, newVersion: 2 }); return [e.type, e.oldVersion, e.newVersion, e.dataLoss, e instanceof Event, IDBVersionChangeEvent.length]; });
        const r0 = indexedDB.open('k68-db', 1);
        t('req0', () => [r0.constructor.name, r0.readyState, Reflect.ownKeys(r0).length, r0 instanceof EventTarget]);
        t('pendingResult', () => { try { return r0.result; } catch (e) { return e.message; } });
        const log = [];
        function wait(r) { return new Promise((res) => { r.onsuccess = () => res(r.result); r.onerror = (e) => { e.preventDefault(); res('ERR:' + r.error.name + ':' + r.error.message); }; }); }
        r0.onupgradeneeded = (e) => {
          const d = r0.result;
          log.push(['up', e.constructor.name, e.oldVersion, e.newVersion, r0.transaction.mode, r0.readyState]);
          const s = d.createObjectStore('s', { keyPath: 'id', autoIncrement: true });
          s.createIndex('byU', 'u', { unique: true });
          s.createIndex('byG', 'g');
          d.createObjectStore('plain');
          log.push(['attrs', s.name, s.keyPath, s.autoIncrement, [...s.indexNames], s.transaction === r0.transaction, [...d.objectStoreNames], d.objectStoreNames.constructor.name]);
          try { d.transaction('s'); } catch (x) { log.push(x.message); }
          try { d.createObjectStore('s'); } catch (x) { log.push(x.name); }
        };
        r0.oncomplete = null;
        globalThis.__p = new Promise((res) => { r0.onsuccess = () => res(r0.result); }).then(async (db) => {
          log.push(['open', db.version, [...db.objectStoreNames], Reflect.ownKeys(db).length]);
          try { db.createObjectStore('z'); } catch (x) { log.push(x.name); }
          const x = db.transaction('s', 'readwrite'); const s = x.objectStore('s');
          log.push(['tx', x.mode, x.durability, x.objectStore('s') === s, s.transaction === x]);
          const pre = s.put({ u: 'a', g: 1 });
          log.push(['pre', pre.readyState]);
          x.oncomplete = (e) => log.push(['complete', e.constructor.name, e.bubbles]);
          log.push(['put1', await wait(pre)]);
          log.push(['put2', await wait(s.put({ u: 'b', g: 0 })), await wait(s.put({ u: 'c', g: 1 }))]);
          log.push(['get', JSON.stringify(await wait(s.get(1)))]);
          const dup = await wait(s.add({ id: 1, u: 'zz' }));
          log.push(['dup', dup]);
          log.push(['uniq', await wait(s.put({ u: 'a' }))]);
          log.push(['count', await wait(s.count()), await wait(s.getAllKeys())]);
          log.push(['cur', await new Promise((res) => { const out = []; const r = s.index('byG').openCursor(); r.onsuccess = () => { const c = r.result; if (!c) return res(out); out.push([c.key, c.primaryKey, c.constructor.name]); c.continue(); }; })]);
          log.push(['uniqueDir', await new Promise((res) => { const out = []; const r = s.index('byG').openKeyCursor(null, 'nextunique'); r.onsuccess = () => { const c = r.result; if (!c) return res(out); out.push([c.key, c.primaryKey, c.constructor.name]); c.continue(); }; })]);
          log.push(['recs', (await wait(s.getAllRecords({ count: 2 }))).map(r => [r.constructor.name, r.key, r.primaryKey])]);
          log.push(['opts', JSON.stringify(await wait(s.getAll({ query: IDBKeyRange.lowerBound(2), count: 1 })))]);
          await new Promise(r => setTimeout(r, 30));
          log.push(['afterDone', (() => { try { s.get(1); return 'NO'; } catch (e) { return e.name; } })()]);
          // abort rolls back + request error bubbles to db.onerror
          const x2 = db.transaction('s', 'readwrite'); const s2 = x2.objectStore('s');
          const bub = [];
          db.onerror = (e) => bub.push(['db', e.target === dupReq, e.eventPhase]);
          x2.onerror = (e) => bub.push(['tx', e.target === dupReq, e.eventPhase]);
          x2.onabort = (e) => bub.push(['abort', e.bubbles, x2.error && x2.error.name]);
          const dupReq = s2.add({ id: 1, u: 'q' });
          await new Promise(r => setTimeout(r, 50));
          log.push(['bubble', bub.slice()]); db.onerror = null;
          const x3 = db.transaction('plain', 'readwrite'); const p3 = x3.objectStore('plain');
          p3.put(1, 'k');
          x3.abort();
          try { x3.abort(); } catch (e) { log.push(['abort2', e.name]); }
          await new Promise(r => setTimeout(r, 30));
          log.push(['rolledBack', await wait(db.transaction('plain').objectStore('plain').get('k'))]);
          const bad = [];
          for (const f of [
            () => db.transaction('nope'), () => db.transaction([]), () => db.transaction('s', 'bogus'),
            () => db.transaction('s').objectStore('s').put({}),
            () => db.transaction('s').objectStore('s').get(),
            () => db.transaction('s').objectStore('s').get({}),
            () => db.transaction('s').objectStore('s').index('zz'),
            () => db.transaction('plain', 'readwrite').objectStore('plain').put(1),
            () => db.transaction('s').objectStore('s').openCursor(null, 'sideways'),
          ]) { try { f(); bad.push('NO'); } catch (e) { bad.push(e.name + ':' + e.message); } }
          log.push(['errs', bad]);
          db.close();
          try { db.transaction('s'); } catch (e) { log.push(['closed', e.message]); }
          const del = indexedDB.deleteDatabase('k68-db');
          await new Promise((res) => { del.onsuccess = (e) => { log.push(['del', e.constructor.name, e.oldVersion, e.newVersion, del.result === undefined]); res(); }; });
        });
        globalThis.__log = log;
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(1200).await;
    let out = rt.evaluate("JSON.stringify(globalThis.__out)").unwrap();
    let o: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        o["illegal"],
        "Failed to construct 'IDBRequest': Illegal constructor"
    );
    assert_eq!(
        o["names"],
        serde_json::json!([["get result", null], ["get onsuccess", "set onsuccess"]])
    );
    assert_eq!(o["foreign"], "Illegal invocation");
    assert_eq!(
        o["range"],
        serde_json::json!([1, 5, false, true, true, false, 0])
    );
    assert_eq!(
        o["rangeErr"],
        "Failed to execute 'bound' on 'IDBKeyRange': The lower key is greater than the upper key."
    );
    assert_eq!(o["cmp"], serde_json::json!([-1, -1, 1]));
    assert_eq!(o["vce"], serde_json::json!(["x", 1, 2, "none", true, 1]));
    assert_eq!(
        o["req0"],
        serde_json::json!(["IDBOpenDBRequest", "pending", 0, true])
    );
    assert_eq!(
        o["pendingResult"],
        "Failed to read the 'result' property from 'IDBRequest': The request has not finished."
    );
    let log = rt.evaluate("JSON.stringify(globalThis.__log)").unwrap();
    let l: serde_json::Value = serde_json::from_str(log.as_str().unwrap()).unwrap();
    let row = |i: usize| l[i].clone();
    assert_eq!(
        row(0),
        serde_json::json!(["up", "IDBVersionChangeEvent", 0, 1, "versionchange", "done"])
    );
    assert_eq!(
        row(1),
        serde_json::json!([
            "attrs",
            "s",
            "id",
            true,
            ["byG", "byU"],
            true,
            ["plain", "s"],
            "DOMStringList"
        ])
    );
    assert_eq!(
        row(2),
        "Failed to execute 'transaction' on 'IDBDatabase': A version change transaction is running."
    );
    assert_eq!(row(3), "ConstraintError");
    assert_eq!(row(4), serde_json::json!(["open", 1, ["plain", "s"], 0]));
    assert_eq!(row(5), "InvalidStateError");
    assert_eq!(
        row(6),
        serde_json::json!(["tx", "readwrite", "default", true, true])
    );
    assert_eq!(row(7), serde_json::json!(["pre", "pending"]));
    assert_eq!(row(8), serde_json::json!(["put1", 1]));
    assert_eq!(row(9), serde_json::json!(["put2", 2, 3]));
    assert_eq!(
        row(10),
        serde_json::json!(["get", "{\"u\":\"a\",\"g\":1,\"id\":1}"])
    );
    assert_eq!(
        row(11),
        serde_json::json!([
            "dup",
            "ERR:ConstraintError:Key already exists in the object store."
        ])
    );
    assert_eq!(
        row(12),
        serde_json::json!([
            "uniq",
            "ERR:ConstraintError:Unable to add key to index 'byU': at least one key does not satisfy the uniqueness requirements."
        ])
    );
    assert_eq!(row(13), serde_json::json!(["count", 3, [1, 2, 3]]));
    assert_eq!(
        row(14),
        serde_json::json!([
            "cur",
            [
                [0, 2, "IDBCursorWithValue"],
                [1, 1, "IDBCursorWithValue"],
                [1, 3, "IDBCursorWithValue"]
            ]
        ])
    );
    assert_eq!(
        row(15),
        serde_json::json!(["uniqueDir", [[0, 2, "IDBCursor"], [1, 1, "IDBCursor"]]])
    );
    assert_eq!(
        row(16),
        serde_json::json!(["recs", [["IDBRecord", 1, 1], ["IDBRecord", 2, 2]]])
    );
    assert_eq!(
        row(17),
        serde_json::json!(["opts", "[{\"u\":\"b\",\"g\":0,\"id\":2}]"])
    );
    assert_eq!(row(18), serde_json::json!(["complete", "Event", false]));
    assert_eq!(
        row(19),
        serde_json::json!(["afterDone", "TransactionInactiveError"])
    );
    assert_eq!(
        row(20),
        serde_json::json!([
            "bubble",
            [
                ["tx", true, 3],
                ["db", true, 3],
                ["abort", true, "ConstraintError"]
            ]
        ])
    );
    assert_eq!(row(21), serde_json::json!(["abort2", "InvalidStateError"]));
    assert_eq!(row(22), serde_json::json!(["rolledBack", null]));
    assert_eq!(
        row(23),
        serde_json::json!([
            "errs",
            [
                "NotFoundError:Failed to execute 'transaction' on 'IDBDatabase': One of the specified object stores was not found.",
                "InvalidAccessError:Failed to execute 'transaction' on 'IDBDatabase': The storeNames parameter was empty.",
                "TypeError:Failed to execute 'transaction' on 'IDBDatabase': The provided value 'bogus' is not a valid enum value of type IDBTransactionMode.",
                "ReadOnlyError:Failed to execute 'put' on 'IDBObjectStore': The transaction is read-only.",
                "TypeError:Failed to execute 'get' on 'IDBObjectStore': 1 argument required, but only 0 present.",
                "DataError:Failed to execute 'get' on 'IDBObjectStore': The parameter is not a valid key.",
                "NotFoundError:Failed to execute 'index' on 'IDBObjectStore': The specified index was not found.",
                "DataError:Failed to execute 'put' on 'IDBObjectStore': The object store uses out-of-line keys and has no key generator and the key parameter was not provided.",
                "TypeError:Failed to execute 'openCursor' on 'IDBObjectStore': The provided value 'sideways' is not a valid enum value of type IDBCursorDirection.",
            ]
        ])
    );
    assert_eq!(
        row(24),
        serde_json::json!([
            "closed",
            "Failed to execute 'transaction' on 'IDBDatabase': The database connection is closing."
        ])
    );
    assert_eq!(
        row(25),
        serde_json::json!(["del", "IDBVersionChangeEvent", 1, null, true])
    );
}

/// M4 刀69: the WebGL context surface against Chrome 145 (real software-WebGL
/// captures, k69 probes): every GL constant exists TWICE (prototype + static,
/// both e--), the member order is Chrome's (constants after the colorSpace
/// faces, makeXRCompatible after `constructor`), `new` is an illegal
/// constructor, the colorSpace faces are per-context rw attributes that
/// silently ignore bad values, makeXRCompatible is one cached never-settling
/// promise, creators are distinct functions returning real resource classes,
/// getExtension is case-insensitive and cached with Chrome's interface tags,
/// getContextAttributes echoes the requested dictionary in alphabetical order,
/// and a name the tables lack is undefined (no number-ish Proxy fallback).
#[tokio::test(flavor = "current_thread")]
async fn knife69_webgl_surface() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const cv = document.createElement('canvas'); cv.width = 64; cv.height = 32;
        const gl = cv.getContext('webgl'); const gl2 = document.createElement('canvas').getContext('webgl2');
        const G1 = WebGLRenderingContext, G2 = WebGL2RenderingContext;
        t('statics', () => [Object.keys(G1).length, Object.getOwnPropertyNames(G1).length, Object.keys(G2).length, Object.getOwnPropertyNames(G2).length]);
        t('staticDesc', () => { const d = Object.getOwnPropertyDescriptor(G1, 'DEPTH_TEST'); return [d.value, d.writable, d.enumerable, d.configurable]; });
        t('staticTail', () => [Object.getOwnPropertyNames(G1).slice(-2), Object.getOwnPropertyNames(G2).slice(-1)]);
        t('rgb8', () => [G1.RGB8, G1.prototype.RGB8, G2.RGB8, G2.prototype.RGBA8]);
        t('protoDesc', () => { const d = Object.getOwnPropertyDescriptor(G2.prototype, 'DEPTH_TEST'); return [d.writable, d.enumerable, d.configurable]; });
        t('orderHead', () => Object.getOwnPropertyNames(G1.prototype).slice(0, 7));
        t('orderTail', () => Object.getOwnPropertyNames(G1.prototype).slice(-4));
        t('orderTail2', () => Object.getOwnPropertyNames(G2.prototype).slice(-3));
        t('illegal', () => { try { new G1(); return 'NO'; } catch (e) { return e.message; } });
        t('mxrShape', () => { const d = Object.getOwnPropertyDescriptor(G1.prototype, 'makeXRCompatible'); return [d.value.length, d.value.name, d.writable, d.enumerable, d.configurable]; });
        t('mxrSame', () => [gl.makeXRCompatible() === gl.makeXRCompatible(), gl.makeXRCompatible() !== gl2.makeXRCompatible(), gl.makeXRCompatible() instanceof Promise]);
        t('cs', () => { const r = [gl.drawingBufferColorSpace, gl.unpackColorSpace]; gl.drawingBufferColorSpace = 'display-p3'; gl.unpackColorSpace = 'display-p3'; r.push(gl.drawingBufferColorSpace, gl.unpackColorSpace); gl.drawingBufferColorSpace = 'bogus'; gl.unpackColorSpace = null; r.push(gl.drawingBufferColorSpace, gl.unpackColorSpace, gl2.drawingBufferColorSpace); return r; });
        t('csNames', () => ['drawingBufferColorSpace', 'unpackColorSpace'].map(k => { const d = Object.getOwnPropertyDescriptor(G1.prototype, k); return [d.get.name, d.set.name, d.set.length]; }));
        t('csForeign', () => { try { Object.getOwnPropertyDescriptor(G1.prototype, 'drawingBufferColorSpace').set.call({}, 'srgb'); return 'NO'; } catch (e) { return e.message; } });
        t('lens', () => ['createBuffer', 'createFramebuffer', 'createProgram', 'createRenderbuffer', 'createTexture', 'createShader'].map(k => gl[k].length).concat(['createQuery', 'fenceSync'].map(k => gl2[k].length)));
        t('handles', () => [gl.createBuffer(), gl.createTexture(), gl.createShader(gl.VERTEX_SHADER), gl2.createVertexArray(), gl2.fenceSync(gl2.SYNC_GPU_COMMANDS_COMPLETE, 0)].map(h => Object.prototype.toString.call(h)));
        t('fenceGl1', () => [typeof gl.fenceSync, 'fenceSync' in gl, typeof gl.nonsense, typeof gl.TEXTURE_3D, typeof gl.then, typeof gl2.fenceSync]);
        t('extTags', () => ['WEBGL_debug_renderer_info', 'EXT_texture_filter_anisotropic', 'WEBGL_lose_context', 'OES_vertex_array_object', 'ANGLE_instanced_arrays'].map(n => Object.prototype.toString.call(gl.getExtension(n))));
        t('extCache', () => [gl.getExtension('WEBGL_lose_context') === gl.getExtension('WEBGL_lose_context'), gl.getExtension('webgl_lose_context') === gl.getExtension('WEBGL_lose_context'), gl.getExtension('WEBGL_lose_context') !== gl2.getExtension('WEBGL_lose_context'), gl.getExtension('NOPE')]);
        t('extShape', () => { const e = gl.getExtension('OES_vertex_array_object'); const p = Object.getPrototypeOf(e); return [Reflect.ownKeys(e).length, Object.getOwnPropertyNames(p), p.VERTEX_ARRAY_BINDING_OES, p.createVertexArrayOES.length, Object.prototype.hasOwnProperty.call(p, 'constructor'), Object.getPrototypeOf(p) === Object.prototype]; });
        t('extConst', () => { const e = gl.getExtension('WEBGL_debug_renderer_info'); const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e), 'UNMASKED_VENDOR_WEBGL'); return [e.UNMASKED_VENDOR_WEBGL, e.UNMASKED_RENDERER_WEBGL, d.writable, d.configurable]; });
        t('extNoArg', () => { try { gl.getExtension(); return 'NO'; } catch (e) { return e.message; } });
        t('attrs', () => Object.keys(gl.getContextAttributes()));
        t('attrsReq', () => JSON.stringify(document.createElement('canvas').getContext('webgl', { alpha: false, antialias: false, depth: false, stencil: true, powerPreference: 'high-performance', preserveDrawingBuffer: true }).getContextAttributes()));
        t('attrsBad', () => { try { document.createElement('canvas').getContext('webgl', { powerPreference: 'x' }); return 'NO'; } catch (e) { return e.name; } });
        t('strict', () => { 'use strict'; const r = []; for (const f of [() => { G1.DEPTH_TEST = 1; }, () => { G1.prototype.DEPTH_TEST = 1; }, () => { gl.DEPTH_TEST = 1; }]) { try { f(); r.push('silent'); } catch (e) { r.push(e.name); } } return r; });
        t('own', () => { gl.__x = 5; return [Reflect.ownKeys(gl2).length, Reflect.ownKeys(gl), gl.__x]; });
        t('tag', () => [Object.prototype.toString.call(gl), gl instanceof G1, gl2 instanceof G2, gl.canvas === cv, gl.drawingBufferWidth, gl.drawingBufferHeight, gl.drawingBufferFormat]);
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(o["statics"], serde_json::json!([298, 301, 559, 562]));
    assert_eq!(
        o["staticDesc"],
        serde_json::json!([2929, false, true, false])
    );
    assert_eq!(
        o["staticTail"],
        serde_json::json!([["RGB8", "RGBA8"], ["MAX_CLIENT_WAIT_TIMEOUT_WEBGL"]])
    );
    assert_eq!(o["rgb8"], serde_json::json!([32849, 32849, 32849, 32856]));
    assert_eq!(o["protoDesc"], serde_json::json!([false, true, false]));
    assert_eq!(
        o["orderHead"],
        serde_json::json!([
            "canvas",
            "drawingBufferWidth",
            "drawingBufferHeight",
            "drawingBufferColorSpace",
            "unpackColorSpace",
            "DEPTH_BUFFER_BIT",
            "STENCIL_BUFFER_BIT"
        ])
    );
    assert_eq!(
        o["orderTail"],
        serde_json::json!([
            "RGBA8",
            "drawingBufferStorage",
            "constructor",
            "makeXRCompatible"
        ])
    );
    assert_eq!(
        o["orderTail2"],
        serde_json::json!(["drawingBufferStorage", "constructor", "makeXRCompatible"])
    );
    assert_eq!(
        o["illegal"],
        "Failed to construct 'WebGLRenderingContext': Illegal constructor"
    );
    assert_eq!(
        o["mxrShape"],
        serde_json::json!([0, "makeXRCompatible", true, true, true])
    );
    assert_eq!(o["mxrSame"], serde_json::json!([true, true, true]));
    assert_eq!(
        o["cs"],
        serde_json::json!([
            "srgb",
            "srgb",
            "display-p3",
            "display-p3",
            "display-p3",
            "display-p3",
            "srgb"
        ])
    );
    assert_eq!(
        o["csNames"],
        serde_json::json!([
            [
                "get drawingBufferColorSpace",
                "set drawingBufferColorSpace",
                1
            ],
            ["get unpackColorSpace", "set unpackColorSpace", 1]
        ])
    );
    assert_eq!(o["csForeign"], "Illegal invocation");
    assert_eq!(o["lens"], serde_json::json!([0, 0, 0, 0, 0, 1, 0, 2]));
    assert_eq!(
        o["handles"],
        serde_json::json!([
            "[object WebGLBuffer]",
            "[object WebGLTexture]",
            "[object WebGLShader]",
            "[object WebGLVertexArrayObject]",
            "[object WebGLSync]"
        ])
    );
    assert_eq!(
        o["fenceGl1"],
        serde_json::json!([
            "undefined",
            false,
            "undefined",
            "undefined",
            "undefined",
            "function"
        ])
    );
    assert_eq!(
        o["extTags"],
        serde_json::json!([
            "[object WebGLDebugRendererInfo]",
            "[object EXTTextureFilterAnisotropic]",
            "[object WebGLLoseContext]",
            "[object OESVertexArrayObject]",
            "[object ANGLEInstancedArrays]"
        ])
    );
    assert_eq!(o["extCache"], serde_json::json!([true, true, true, null]));
    assert_eq!(
        o["extShape"],
        serde_json::json!([
            0,
            [
                "VERTEX_ARRAY_BINDING_OES",
                "bindVertexArrayOES",
                "createVertexArrayOES",
                "deleteVertexArrayOES",
                "isVertexArrayOES"
            ],
            34229,
            0,
            false,
            true
        ])
    );
    assert_eq!(
        o["extConst"],
        serde_json::json!([37445, 37446, false, false])
    );
    assert_eq!(
        o["extNoArg"],
        "Failed to execute 'getExtension' on 'WebGLRenderingContext': 1 argument required, but only 0 present."
    );
    assert_eq!(
        o["attrs"],
        serde_json::json!([
            "alpha",
            "antialias",
            "depth",
            "desynchronized",
            "failIfMajorPerformanceCaveat",
            "powerPreference",
            "premultipliedAlpha",
            "preserveDrawingBuffer",
            "stencil",
            "xrCompatible"
        ])
    );
    assert_eq!(
        o["attrsReq"],
        serde_json::json!(
            "{\"alpha\":false,\"antialias\":false,\"depth\":false,\"desynchronized\":false,\"failIfMajorPerformanceCaveat\":false,\"powerPreference\":\"high-performance\",\"premultipliedAlpha\":true,\"preserveDrawingBuffer\":true,\"stencil\":true,\"xrCompatible\":false}"
        )
    );
    assert_eq!(o["attrsBad"], "TypeError");
    assert_eq!(
        o["strict"],
        serde_json::json!(["TypeError", "TypeError", "TypeError"])
    );
    assert_eq!(o["own"], serde_json::json!([0, ["__x"], 5]));
    assert_eq!(
        o["tag"],
        serde_json::json!([
            "[object WebGLRenderingContext]",
            true,
            true,
            true,
            64,
            32,
            32856
        ])
    );
}

/// M4 刀70: SpeechSynthesisUtterance + voice/event faces against Chrome 145
/// (k70 probes): EventTarget instances with zero own keys, Chrome defaults
/// (lang ''), ToString text/lang (Symbol throws), finite-checked clamped
/// float32 volume/rate/pitch, voice only a real SpeechSynthesisVoice or null,
/// on* attributes keep functions/objects and null the rest, speak() argument
/// ladder, and the SpeechSynthesisEvent required-member ctor.
#[tokio::test(flavor = "current_thread")]
async fn knife70_speech_utterance_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const U = SpeechSynthesisUtterance;
        t('shape', () => [U.length, U.name, Reflect.ownKeys(new U()).length, Object.prototype.toString.call(new U()), new U() instanceof EventTarget]);
        t('chain', () => { const c = []; let p = Object.getPrototypeOf(new U()); while (p) { c.push(p.constructor.name); p = Object.getPrototypeOf(p); } return c; });
        t('defaults', () => { const u = new U(); return [u.text, u.lang, u.voice, u.volume, u.rate, u.pitch, u.onstart, u.onboundary]; });
        t('ctorText', () => [new U('hi').text, new U(undefined).text, new U(null).text, new U(5).text]);
        t('noNew', () => { try { U('x'); return 'NO'; } catch (e) { return e.message; } });
        t('strings', () => { const u = new U(); u.text = 7; u.lang = null; return [u.text, u.lang]; });
        t('symbol', () => { const u = new U(); const r = []; for (const f of [() => { u.text = Symbol(); }, () => { u.volume = Symbol(); }]) { try { f(); r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('floats', () => { const u = new U(); const r = []; for (const [k, vs] of [['volume', [0.5, 2, -1, '0.3', null]], ['rate', [0.05, 11, 2]], ['pitch', [2.5, -1, 1.5]]]) for (const x of vs) { u[k] = x; r.push(u[k]); } return r; });
        t('nonFinite', () => { const u = new U(); const r = []; for (const [k, x] of [['volume', NaN], ['rate', Infinity], ['pitch', 'x']]) { try { u[k] = x; r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('voice', () => { const u = new U(); const r = []; u.voice = null; r.push(u.voice); for (const x of [{}, 'x', 5]) { try { u.voice = x; r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('handlers', () => { const u = new U(); const f = () => 1; u.onend = f; const a = u.onend === f; u.onstart = 5; const b = u.onstart; const obj = {}; u.onerror = obj; return [a, b, u.onerror === obj]; });
        t('dispatch', () => { const u = new U(); let n = 0; u.onstart = () => n++; u.addEventListener('start', () => n++); u.dispatchEvent(new Event('start')); return n; });
        t('names', () => ['text', 'voice', 'onstart'].map(k => { const d = Object.getOwnPropertyDescriptor(U.prototype, k); return [d.get.name, d.set.name, d.set.length, d.enumerable, d.configurable]; }));
        t('foreign', () => { const d = Object.getOwnPropertyDescriptor(U.prototype, 'text'); const r = []; for (const f of [() => d.get.call({}), () => d.set.call({}, 'x')]) { try { f(); r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('speak', () => { const r = []; for (const f of [() => speechSynthesis.speak(), () => speechSynthesis.speak({}), () => speechSynthesis.speak(new U('x'))]) { try { f(); r.push('ok'); } catch (e) { r.push(e.message); } } return r; });
        t('evCtor', () => { const u = new U(); const e = new SpeechSynthesisEvent('x', { utterance: u, charIndex: 3.7, name: 'n' }); return [e.type, e.utterance === u, e.charIndex, e.charLength, e.elapsedTime, e.name, e instanceof Event, SpeechSynthesisEvent.length]; });
        t('evErrs', () => { const r = []; for (const f of [() => new SpeechSynthesisEvent('x'), () => new SpeechSynthesisEvent('x', {}), () => new SpeechSynthesisErrorEvent('x', { utterance: new U() })]) { try { f(); r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('errEv', () => { const e = new SpeechSynthesisErrorEvent('error', { utterance: new U(), error: 'not-allowed' }); return [e.error, e instanceof SpeechSynthesisEvent, e instanceof Event, SpeechSynthesisErrorEvent.length]; });
        t('voices', () => { const vs = speechSynthesis.getVoices(); return [Array.isArray(vs), Object.prototype.toString.call(vs[0]), speechSynthesis.getVoices() !== vs]; });
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["shape"],
        serde_json::json!([
            0,
            "SpeechSynthesisUtterance",
            0,
            "[object SpeechSynthesisUtterance]",
            true
        ])
    );
    assert_eq!(
        o["chain"],
        serde_json::json!(["SpeechSynthesisUtterance", "EventTarget", "Object"])
    );
    assert_eq!(
        o["defaults"],
        serde_json::json!(["", "", null, 1, 1, 1, null, null])
    );
    assert_eq!(o["ctorText"], serde_json::json!(["hi", "", "null", "5"]));
    assert_eq!(
        o["noNew"],
        "Failed to construct 'SpeechSynthesisUtterance': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
    );
    assert_eq!(o["strings"], serde_json::json!(["7", "null"]));
    assert_eq!(
        o["symbol"],
        serde_json::json!([
            "Failed to set the 'text' property on 'SpeechSynthesisUtterance': Cannot convert a Symbol value to a string",
            "Failed to set the 'volume' property on 'SpeechSynthesisUtterance': Cannot convert a Symbol value to a number"
        ])
    );
    assert_eq!(
        o["floats"],
        serde_json::json!([
            0.5,
            1,
            0,
            0.30000001192092896,
            0,
            0.10000000149011612,
            10,
            2,
            2,
            0,
            1.5
        ])
    );
    assert_eq!(
        o["nonFinite"],
        serde_json::json!([
            "Failed to set the 'volume' property on 'SpeechSynthesisUtterance': The provided float value is non-finite.",
            "Failed to set the 'rate' property on 'SpeechSynthesisUtterance': The provided float value is non-finite.",
            "Failed to set the 'pitch' property on 'SpeechSynthesisUtterance': The provided float value is non-finite."
        ])
    );
    assert_eq!(
        o["voice"],
        serde_json::json!([
            null,
            "Failed to set the 'voice' property on 'SpeechSynthesisUtterance': Failed to convert value to 'SpeechSynthesisVoice'.",
            "Failed to set the 'voice' property on 'SpeechSynthesisUtterance': Failed to convert value to 'SpeechSynthesisVoice'.",
            "Failed to set the 'voice' property on 'SpeechSynthesisUtterance': Failed to convert value to 'SpeechSynthesisVoice'."
        ])
    );
    assert_eq!(o["handlers"], serde_json::json!([true, null, true]));
    assert_eq!(o["dispatch"], 2);
    assert_eq!(
        o["names"],
        serde_json::json!([
            ["get text", "set text", 1, true, true],
            ["get voice", "set voice", 1, true, true],
            ["get onstart", "set onstart", 1, true, true]
        ])
    );
    assert_eq!(
        o["foreign"],
        serde_json::json!(["Illegal invocation", "Illegal invocation"])
    );
    assert_eq!(
        o["speak"],
        serde_json::json!([
            "Failed to execute 'speak' on 'SpeechSynthesis': 1 argument required, but only 0 present.",
            "Failed to execute 'speak' on 'SpeechSynthesis': parameter 1 is not of type 'SpeechSynthesisUtterance'.",
            "ok"
        ])
    );
    assert_eq!(
        o["evCtor"],
        serde_json::json!(["x", true, 3, 0, 0, "n", true, 2])
    );
    assert_eq!(
        o["evErrs"],
        serde_json::json!([
            "Failed to construct 'SpeechSynthesisEvent': 2 arguments required, but only 1 present.",
            "Failed to construct 'SpeechSynthesisEvent': Failed to read the 'utterance' property from 'SpeechSynthesisEventInit': Required member is undefined.",
            "Failed to construct 'SpeechSynthesisErrorEvent': Failed to read the 'error' property from 'SpeechSynthesisErrorEventInit': Required member is undefined."
        ])
    );
    assert_eq!(
        o["errEv"],
        serde_json::json!(["not-allowed", true, true, 2])
    );
}

/// M4 刀71: SVGGradientElement 7 + SVGAElement 6 + interestForElement on the
/// four interfaces Chrome carries it on (k71 probes, Chrome 145): enum views
/// with default/junk/range ladder and keyword-serializing baseVal writes,
/// gradientTransform list view, href SVGAnimatedString with xlink fallback,
/// four SVG_SPREADMETHOD_* constants on prototype AND constructor, plain
/// reflected download/ping/hreflang/referrerPolicy, target view, and the
/// Element-reference reflection with property assignment, id lookup, and
/// the Element conversion TypeError.
#[tokio::test(flavor = "current_thread")]
async fn knife71_svg_gradient_anchor_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const NS = 'http://www.w3.org/2000/svg';
        const mk = (n) => document.createElementNS(NS, n);
        const d = document.createElement('div'); document.body.appendChild(d);
        const svg = mk('svg'); d.appendChild(svg);
        const g = mk('linearGradient'); svg.appendChild(g);
        const G = SVGGradientElement;
        t('defaults', () => [g.gradientUnits.baseVal, g.spreadMethod.baseVal, g.gradientTransform.baseVal.numberOfItems, g.gradientUnits.constructor.name, g.gradientTransform.constructor.name, g.href.constructor.name, g.href.baseVal]);
        t('units', () => { const r = []; for (const v of ['userSpaceOnUse', 'objectBoundingBox', 'junk', '', 'USERSPACEONUSE']) { g.setAttribute('gradientUnits', v); r.push(g.gradientUnits.baseVal); } return r; });
        t('spread', () => { const r = []; for (const v of ['pad', 'reflect', 'repeat', 'junk', '', 'REFLECT']) { g.setAttribute('spreadMethod', v); r.push(g.spreadMethod.baseVal); } return r; });
        t('setUnits', () => { g.setAttribute('gradientUnits', 'objectBoundingBox'); g.gradientUnits.baseVal = 1; const a = [g.getAttribute('gradientUnits'), g.gradientUnits.baseVal]; for (const x of [5, 0]) { try { g.gradientUnits.baseVal = x; a.push('nothrow'); } catch (e) { a.push(e.message); } } return a; });
        t('setSpread', () => { g.spreadMethod.baseVal = 3; const a = [g.getAttribute('spreadMethod'), g.spreadMethod.baseVal]; try { g.spreadMethod.baseVal = 9; a.push('nothrow'); } catch (e) { a.push(e.message); } return a; });
        t('transform', () => { g.setAttribute('gradientTransform', 'translate(10 20) scale(2)'); const l = g.gradientTransform.baseVal; return [l.numberOfItems, l.getItem(0).type, l.getItem(0).matrix.e, l.getItem(1).type, l.getItem(1).matrix.a, l.consolidate().matrix.e]; });
        t('transformJunk', () => { g.setAttribute('gradientTransform', 'junk'); return g.gradientTransform.baseVal.numberOfItems; });
        t('href', () => { g.setAttributeNS('http://www.w3.org/1999/xlink', 'xlink:href', '#a'); const a = [g.href.baseVal, g.href.animVal]; g.setAttribute('href', '#b'); a.push(g.href.baseVal); return a; });
        t('consts', () => [G.SVG_SPREADMETHOD_UNKNOWN, G.SVG_SPREADMETHOD_PAD, G.SVG_SPREADMETHOD_REFLECT, G.SVG_SPREADMETHOD_REPEAT, G.prototype.SVG_SPREADMETHOD_REPEAT]);
        t('constDesc', () => { const x = Object.getOwnPropertyDescriptor(G, 'SVG_SPREADMETHOD_PAD'); const y = Object.getOwnPropertyDescriptor(G.prototype, 'SVG_SPREADMETHOD_PAD'); return [x.writable, x.enumerable, x.configurable, y.writable, y.enumerable, y.configurable]; });
        t('getters', () => ['gradientUnits', 'gradientTransform', 'spreadMethod', 'href'].map(k => { const x = Object.getOwnPropertyDescriptor(G.prototype, k); return [x.get.name, !!x.set, x.enumerable, x.configurable]; }));
        t('foreign', () => { const r = []; for (const recv of [{}, mk('rect')]) { try { Object.getOwnPropertyDescriptor(G.prototype, 'spreadMethod').get.call(recv); r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        const a = mk('a'); svg.appendChild(a);
        t('aDefaults', () => [a.target.constructor.name, a.target.baseVal, a.download, a.ping, a.hreflang, a.type, a.referrerPolicy, a.rel, a.interestForElement, a.href.baseVal, a.relList.length]);
        t('aSet', () => { a.download = 'f.txt'; a.ping = 'http://x/p'; a.hreflang = 'en'; a.type = 'text/html'; a.rel = 'noopener'; return [a.getAttribute('download'), a.getAttribute('ping'), a.getAttribute('hreflang'), a.getAttribute('type'), a.getAttribute('rel'), a.relList.contains('noopener')]; });
        t('aNullish', () => { a.download = null; a.hreflang = undefined; return [a.getAttribute('download'), a.getAttribute('hreflang')]; });
        t('aReferrer', () => { const r = []; for (const v of ['no-referrer', 'ORIGIN', 'junk', '']) { a.referrerPolicy = v; r.push([a.getAttribute('referrerpolicy'), a.referrerPolicy]); } a.setAttribute('referrerpolicy', 'Same-Origin'); r.push(a.referrerPolicy); return r; });
        t('aTarget', () => { a.setAttribute('target', '_blank'); const x = [a.target.baseVal, a.target.animVal, a.target === a.target]; a.target.baseVal = '_self'; x.push(a.getAttribute('target')); return x; });
        t('aGetters', () => ['target', 'download', 'ping', 'hreflang', 'referrerPolicy', 'interestForElement'].map(k => { const x = Object.getOwnPropertyDescriptor(SVGAElement.prototype, k); return [x.get.name, x.set ? x.set.name : null, x.set ? x.set.length : null, x.enumerable, x.configurable]; }));
        t('aForeign', () => { const r = []; for (const f of [() => Object.getOwnPropertyDescriptor(SVGAElement.prototype, 'download').get.call({}), () => Object.getOwnPropertyDescriptor(SVGAElement.prototype, 'ping').set.call(mk('rect'), 'x')]) { try { f(); r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('aOwn', () => Reflect.ownKeys(a).length);
        t('interest', () => { const r = [a.interestForElement]; const tgt = document.createElement('div'); tgt.id = 'tg'; d.appendChild(tgt); a.setAttribute('interestfor', 'tg'); r.push(a.interestForElement === tgt); a.setAttribute('interestfor', 'nope'); r.push(a.interestForElement); a.interestForElement = tgt; r.push(a.getAttribute('interestfor')); r.push(a.interestForElement === tgt); try { a.interestForElement = {}; r.push('nothrow'); } catch (e) { r.push(e.message); } a.interestForElement = null; r.push(a.hasAttribute('interestfor')); return r; });
        t('interestHtml', () => ['HTMLAnchorElement', 'HTMLAreaElement', 'HTMLButtonElement'].map(n => 'interestForElement' in globalThis[n].prototype));
        t('interestHtmlRoundTrip', () => { const h = document.createElement('a'); const tgt = document.createElement('div'); h.interestForElement = tgt; return [h.getAttribute('interestfor'), h.interestForElement === tgt]; });
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["defaults"],
        serde_json::json!([
            2,
            1,
            0,
            "SVGAnimatedEnumeration",
            "SVGAnimatedTransformList",
            "SVGAnimatedString",
            ""
        ])
    );
    assert_eq!(o["units"], serde_json::json!([1, 2, 2, 2, 2]));
    assert_eq!(o["spread"], serde_json::json!([1, 2, 3, 1, 1, 1]));
    assert_eq!(
        o["setUnits"],
        serde_json::json!([
            "userSpaceOnUse",
            1,
            "Failed to set the 'baseVal' property on 'SVGAnimatedEnumeration': The enumeration value provided (5) is larger than the largest allowed value (2).",
            "Failed to set the 'baseVal' property on 'SVGAnimatedEnumeration': The enumeration value provided is 0, which is not settable."
        ])
    );
    assert_eq!(
        o["setSpread"],
        serde_json::json!([
            "repeat",
            3,
            "Failed to set the 'baseVal' property on 'SVGAnimatedEnumeration': The enumeration value provided (9) is larger than the largest allowed value (3)."
        ])
    );
    assert_eq!(o["transform"], serde_json::json!([2, 2, 10, 3, 2, 10]));
    assert_eq!(o["transformJunk"], 0);
    assert_eq!(o["href"], serde_json::json!(["#a", "#a", "#b"]));
    assert_eq!(o["consts"], serde_json::json!([0, 1, 2, 3, 3]));
    assert_eq!(
        o["constDesc"],
        serde_json::json!([false, true, false, false, true, false])
    );
    assert_eq!(
        o["getters"],
        serde_json::json!([
            ["get gradientUnits", false, true, true],
            ["get gradientTransform", false, true, true],
            ["get spreadMethod", false, true, true],
            ["get href", false, true, true]
        ])
    );
    assert_eq!(
        o["foreign"],
        serde_json::json!(["Illegal invocation", "Illegal invocation"])
    );
    assert_eq!(
        o["aDefaults"],
        serde_json::json!(["SVGAnimatedString", "", "", "", "", "", "", "", null, "", 0])
    );
    assert_eq!(
        o["aSet"],
        serde_json::json!(["f.txt", "http://x/p", "en", "text/html", "noopener", true])
    );
    assert_eq!(o["aNullish"], serde_json::json!(["null", "undefined"]));
    assert_eq!(
        o["aReferrer"],
        serde_json::json!([
            ["no-referrer", "no-referrer"],
            ["ORIGIN", "ORIGIN"],
            ["junk", "junk"],
            ["", ""],
            "Same-Origin"
        ])
    );
    assert_eq!(
        o["aTarget"],
        serde_json::json!(["_blank", "_blank", true, "_self"])
    );
    assert_eq!(
        o["aGetters"],
        serde_json::json!([
            ["get target", null, null, true, true],
            ["get download", "set download", 1, true, true],
            ["get ping", "set ping", 1, true, true],
            ["get hreflang", "set hreflang", 1, true, true],
            ["get referrerPolicy", "set referrerPolicy", 1, true, true],
            [
                "get interestForElement",
                "set interestForElement",
                1,
                true,
                true
            ]
        ])
    );
    assert_eq!(
        o["aForeign"],
        serde_json::json!(["Illegal invocation", "Illegal invocation"])
    );
    assert_eq!(o["aOwn"], 0);
    assert_eq!(
        o["interest"],
        serde_json::json!([
            null,
            true,
            null,
            "",
            true,
            "Failed to set the 'interestForElement' property on 'SVGAElement': Failed to convert value to 'Element'.",
            false
        ])
    );
    assert_eq!(o["interestHtml"], serde_json::json!([true, true, true]));
    assert_eq!(o["interestHtmlRoundTrip"], serde_json::json!(["", true]));
}

/// M4 刀72: the SVG parent chain is Element, not HTMLElement (Chrome 145 truth,
/// k72 probes): SVGElement owns className (read-only SVGAnimatedString) /
/// ownerSVGElement / viewportElement / autofocus / attributeStyleMap, none of
/// HTMLElement's 30 members (hidden/click/innerText/title/lang/...) reach an SVG
/// element, SVGClipPathElement is a plain SVGElement while SVGSymbolElement is
/// a graphics element, SVGGraphicsElement carries transform /
/// nearest+farthestViewportElement / requiredExtensions / systemLanguage, the
/// illegal-constructor messages name the interface constructed; CSS side: no
/// CSS2Properties level under CSSStyleDeclaration (whose constructor is
/// illegal), CSSStyleRule sits directly on CSSRule, and the style attribute
/// serializes `name: value;` space-joined.
#[tokio::test(flavor = "current_thread")]
async fn knife72_svg_parent_chain() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const NS = 'http://www.w3.org/2000/svg'; const mk = (n) => document.createElementNS(NS, n);
        const host = document.createElement('div'); document.body.appendChild(host);
        const svg = mk('svg'); host.appendChild(svg); const g = mk('g'); svg.appendChild(g); const inner = mk('svg'); g.appendChild(inner);
        const rect = mk('rect'); inner.appendChild(rect); const sym = mk('symbol'); svg.appendChild(sym); const sr = mk('rect'); sym.appendChild(sr);
        const det = mk('rect'); const img = mk('image'); svg.appendChild(img); const ir = mk('rect'); img.appendChild(ir);
        const mrk = mk('marker'); svg.appendChild(mrk); const mr = mk('rect'); mrk.appendChild(mr);
        const chain = (e) => { const c = []; let p = Object.getPrototypeOf(e); while (p && c.length < 7) { c.push(p.constructor.name); p = Object.getPrototypeOf(p); } return c.join('>'); };
        t('chains', () => ['svg', 'rect', 'clipPath', 'symbol', 'linearGradient', 'a', 'g'].map(n => chain(mk(n))));
        t('ctor', () => ['SVGElement', 'SVGGraphicsElement', 'SVGGradientElement', 'SVGClipPathElement', 'SVGSymbolElement'].map(n => { try { new window[n](); return 'NO'; } catch (e) { return e.name + ':' + e.message; } }));
        t('className', () => [rect.className.constructor.name, rect.className.baseVal, rect.className === rect.className]);
        t('classNameSet', () => { rect.setAttribute('class', 'a b'); const r = [rect.className.baseVal, rect.className.animVal]; rect.className.baseVal = 'z'; r.push(rect.getAttribute('class')); rect.className = 'q'; r.push(typeof rect.className, rect.getAttribute('class')); return r; });
        t('owner', () => [svg.ownerSVGElement, g.ownerSVGElement === svg, inner.ownerSVGElement === svg, rect.ownerSVGElement === inner, det.ownerSVGElement, sr.ownerSVGElement === svg]);
        t('viewport', () => [svg.viewportElement, g.viewportElement === svg, rect.viewportElement === inner, sr.viewportElement === sym, sym.viewportElement === svg, ir.viewportElement === img, mr.viewportElement === svg, det.viewportElement]);
        t('nearFar', () => [rect.nearestViewportElement === inner, rect.farthestViewportElement === svg, sr.nearestViewportElement === sym, sr.farthestViewportElement === svg, svg.nearestViewportElement, det.farthestViewportElement]);
        t('autofocus', () => { const r = [rect.autofocus]; rect.autofocus = true; r.push(rect.getAttribute('autofocus')); rect.autofocus = false; r.push(rect.hasAttribute('autofocus')); return r; });
        t('asm', () => { rect.removeAttribute('style'); rect.attributeStyleMap.set('opacity', '0.5'); return [Object.prototype.toString.call(rect.attributeStyleMap), rect.attributeStyleMap === rect.attributeStyleMap, rect.getAttribute('style')]; });
        t('absentHtml', () => ['title', 'lang', 'translate', 'dir', 'hidden', 'inert', 'accessKey', 'draggable', 'spellcheck', 'innerText', 'outerText', 'offsetWidth', 'offsetParent', 'click', 'contentEditable', 'popover', 'attachInternals', 'showPopover', 'enterKeyHint', 'inputMode'].map(k => typeof rect[k]));
        t('present', () => ['dataset', 'nonce', 'tabIndex', 'style', 'blur', 'focus', 'onclick', 'getBoundingClientRect', 'innerHTML', 'children', 'classList', 'ariaLabel', 'matches'].map(k => typeof rect[k]));
        t('instanceof', () => [rect instanceof SVGElement, rect instanceof Element, rect instanceof HTMLElement, mk('clipPath') instanceof SVGGraphicsElement, mk('symbol') instanceof SVGGraphicsElement, mk('linearGradient') instanceof SVGGraphicsElement]);
        t('graphics', () => ['rect', 'g', 'svg', 'symbol', 'path', 'a', 'use', 'text', 'image', 'foreignObject'].map(n => { const e = mk(n); return typeof e.transform + typeof e.nearestViewportElement + typeof e.requiredExtensions + typeof e.systemLanguage + typeof e.getBBox; }).join('|'));
        t('transform', () => { rect.setAttribute('transform', 'rotate(45) translate(1 2)'); const l = rect.transform.baseVal; return [rect.transform.constructor.name, l.numberOfItems, l.getItem(0).type, l.getItem(1).type, rect.transform === rect.transform]; });
        t('strLists', () => { rect.setAttribute('systemLanguage', 'en, fr'); return [rect.requiredExtensions.constructor.name, rect.systemLanguage.constructor.name, rect.systemLanguage.length, rect.systemLanguage === rect.systemLanguage]; });
        t('clipSymbol', () => { const c = mk('clipPath'), s = mk('symbol'); return [typeof c.transform, typeof c.getBBox, typeof c.getCTM, typeof s.transform, typeof s.getBBox, typeof s.getCTM, typeof s.nearestViewportElement]; });
        t('getters', () => ['transform', 'nearestViewportElement', 'farthestViewportElement', 'requiredExtensions', 'systemLanguage'].map(k => { const d = Object.getOwnPropertyDescriptor(SVGGraphicsElement.prototype, k); return [d.get.name, !!d.set, d.enumerable, d.configurable]; }));
        t('foreign', () => { const r = []; for (const recv of [{}, mk('clipPath')]) { try { Object.getOwnPropertyDescriptor(SVGGraphicsElement.prototype, 'transform').get.call(recv); r.push('NO'); } catch (e) { r.push(e.message); } } return r; });
        t('own', () => Reflect.ownKeys(rect).length + ':' + Object.keys(rect).length);
        t('cssChain', () => chain(document.body.style));
        t('cssCtor', () => { try { new CSSStyleDeclaration(); return 'NO'; } catch (e) { return e.name + ':' + e.message; } });
        t('cssGlobal', () => typeof CSS2Properties);
        t('ruleChain', () => { const s = document.createElement('style'); s.textContent = 'a{color:red}'; document.head.appendChild(s); return chain(s.sheet.cssRules[0]); });
        t('styleSer', () => { const d = document.createElement('div'); d.style.opacity = '0.5'; d.style.color = 'red'; const a = [d.style.cssText, d.getAttribute('style')]; d.removeAttribute('style'); d.attributeStyleMap.set('opacity', '0.5'); a.push(d.getAttribute('style')); return a; });
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["chains"],
        serde_json::json!([
            "SVGSVGElement>SVGGraphicsElement>SVGElement>Element>Node>EventTarget>Object",
            "SVGRectElement>SVGGeometryElement>SVGGraphicsElement>SVGElement>Element>Node>EventTarget",
            "SVGClipPathElement>SVGElement>Element>Node>EventTarget>Object",
            "SVGSymbolElement>SVGGraphicsElement>SVGElement>Element>Node>EventTarget>Object",
            "SVGLinearGradientElement>SVGGradientElement>SVGElement>Element>Node>EventTarget>Object",
            "SVGAElement>SVGGraphicsElement>SVGElement>Element>Node>EventTarget>Object",
            "SVGGElement>SVGGraphicsElement>SVGElement>Element>Node>EventTarget>Object"
        ])
    );
    assert_eq!(
        o["ctor"],
        serde_json::json!([
            "TypeError:Failed to construct 'SVGElement': Illegal constructor",
            "TypeError:Failed to construct 'SVGGraphicsElement': Illegal constructor",
            "TypeError:Failed to construct 'SVGGradientElement': Illegal constructor",
            "TypeError:Failed to construct 'SVGClipPathElement': Illegal constructor",
            "TypeError:Failed to construct 'SVGSymbolElement': Illegal constructor"
        ])
    );
    assert_eq!(
        o["className"],
        serde_json::json!(["SVGAnimatedString", "", true])
    );
    assert_eq!(
        o["classNameSet"],
        serde_json::json!(["a b", "a b", "z", "object", "z"])
    );
    assert_eq!(
        o["owner"],
        serde_json::json!([null, true, true, true, null, true])
    );
    assert_eq!(
        o["viewport"],
        serde_json::json!([null, true, true, true, true, true, true, null])
    );
    assert_eq!(
        o["nearFar"],
        serde_json::json!([true, true, true, true, null, null])
    );
    assert_eq!(o["autofocus"], serde_json::json!([false, "", false]));
    assert_eq!(
        o["asm"],
        serde_json::json!(["[object StylePropertyMap]", true, "opacity: 0.5;"])
    );
    assert_eq!(o["absentHtml"], serde_json::json!(vec!["undefined"; 20]));
    assert_eq!(
        o["present"],
        serde_json::json!([
            "object", "string", "number", "object", "function", "function", "object", "function",
            "string", "object", "object", "object", "function"
        ])
    );
    assert_eq!(
        o["instanceof"],
        serde_json::json!([true, true, false, false, true, false])
    );
    assert_eq!(
        o["graphics"],
        serde_json::json!(["objectobjectobjectobjectfunction"; 10].join("|"))
    );
    assert_eq!(
        o["transform"],
        serde_json::json!(["SVGAnimatedTransformList", 2, 4, 2, true])
    );
    assert_eq!(
        o["strLists"],
        serde_json::json!(["SVGStringList", "SVGStringList", 2, true])
    );
    assert_eq!(
        o["clipSymbol"],
        serde_json::json!([
            "object",
            "undefined",
            "undefined",
            "object",
            "function",
            "function",
            "object"
        ])
    );
    assert_eq!(
        o["getters"],
        serde_json::json!([
            ["get transform", false, true, true],
            ["get nearestViewportElement", false, true, true],
            ["get farthestViewportElement", false, true, true],
            ["get requiredExtensions", false, true, true],
            ["get systemLanguage", false, true, true]
        ])
    );
    assert_eq!(
        o["foreign"],
        serde_json::json!(["Illegal invocation", "Illegal invocation"])
    );
    assert_eq!(o["own"], "0:0");
    assert_eq!(o["cssChain"], "CSSStyleDeclaration>Object");
    assert_eq!(
        o["cssCtor"],
        "TypeError:Failed to construct 'CSSStyleDeclaration': Illegal constructor"
    );
    assert_eq!(o["cssGlobal"], "undefined");
    assert_eq!(o["ruleChain"], "CSSStyleRule>CSSRule>Object");
    assert_eq!(
        o["styleSer"],
        serde_json::json!([
            "opacity: 0.5; color: red;",
            "opacity: 0.5; color: red;",
            "opacity: 0.5;"
        ])
    );
}

/// M4 刀73: Selection direction / base* / extent* / getComposedRanges against
/// Chrome 145 (k73 probes): direction is 'none' for empty/collapsed/addRange'd
/// selections and forward/backward otherwise, base*/extent* alias anchor/focus,
/// getComposedRanges takes an options dict (arrays/ShadowRoots/null are empty
/// options, primitives are a TypeError, bad shadowRoots have their own
/// messages), returns fresh normalized StaticRanges and lifts endpoints out of
/// unlisted shadow trees to just around the host.
#[tokio::test(flavor = "current_thread")]
async fn knife73_selection_direction_and_composed() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const box = document.createElement('div'); box.innerHTML = '<p id="p">hello <b id="b">big</b> world</p>'; document.body.appendChild(box);
        const tn = box.querySelector('#p').firstChild;
        const host = document.createElement('div'); box.appendChild(host); const sr = host.attachShadow({ mode: 'open' }); sr.innerHTML = '<span id="sp">in shadow</span>'; const st = sr.getElementById('sp').firstChild;
        const s = getSelection(); s.removeAllRanges();
        t('none', () => [s.direction, s.type, s.baseNode, s.baseOffset, s.extentNode, s.extentOffset]);
        t('collapse', () => { s.collapse(tn, 2); return [s.direction, s.type, s.baseNode === tn, s.baseOffset, s.extentNode === tn, s.extentOffset]; });
        t('forward', () => { s.setBaseAndExtent(tn, 1, tn, 4); return [s.direction, s.baseOffset, s.extentOffset, s.anchorOffset, s.focusOffset]; });
        t('backward', () => { s.setBaseAndExtent(tn, 4, tn, 1); return [s.direction, s.baseOffset, s.extentOffset, s.anchorOffset, s.focusOffset]; });
        t('range', () => { const r = document.createRange(); r.setStart(tn, 0); r.setEnd(tn, 3); s.removeAllRanges(); s.addRange(r); return [s.direction, s.baseOffset, s.extentOffset]; });
        t('extend', () => { s.collapse(tn, 3); s.extend(tn, 1); return [s.direction, s.baseOffset, s.extentOffset]; });
        t('selectAll', () => { s.selectAllChildren(box.querySelector('#p')); return [s.direction, s.type]; });
        t('removed', () => { s.removeAllRanges(); return [s.direction, s.baseNode]; });
        t('getters', () => ['direction', 'baseNode', 'baseOffset', 'extentNode', 'extentOffset'].map(k => { const d = Object.getOwnPropertyDescriptor(Selection.prototype, k); return [d.get.name, !!d.set, d.enumerable, d.configurable]; }));
        t('foreign', () => { try { Object.getOwnPropertyDescriptor(Selection.prototype, 'baseNode').get.call({}); return 'NO'; } catch (e) { return e.name + ':' + e.message; } });
        t('gcrLen', () => [Selection.prototype.getComposedRanges.length, Selection.prototype.getComposedRanges.name]);
        t('empty', () => { s.removeAllRanges(); const r = s.getComposedRanges(); return [Array.isArray(r), r.length]; });
        t('range1', () => { s.setBaseAndExtent(tn, 1, tn, 4); const r = s.getComposedRanges(); return [r.length, r[0].constructor.name, r[0].startContainer === tn, r[0].startOffset, r[0].endOffset, r[0].collapsed]; });
        t('backNorm', () => { s.setBaseAndExtent(tn, 4, tn, 1); const r = s.getComposedRanges()[0]; return [r.startOffset, r.endOffset, s.direction]; });
        t('fresh', () => { const a = s.getComposedRanges(), b = s.getComposedRanges(); return [a !== b, a[0] !== b[0]]; });
        t('collapsedRange', () => { s.collapse(tn, 2); const a = s.getComposedRanges(); return [a.length, a[0].collapsed, a[0].startOffset]; });
        t('badArgs', () => { s.setBaseAndExtent(tn, 1, tn, 2); const r = []; for (const a of [5, 'x', null, undefined, [1], [], sr, { shadowRoots: [1] }, { shadowRoots: 5 }, { shadowRoots: [sr] }, { shadowRoots: null }]) { try { s.getComposedRanges(a); r.push('ok'); } catch (e) { r.push(e.message.replace("Failed to execute 'getComposedRanges' on 'Selection': ", '')); } } return r; });
        t('retarget', () => { s.setBaseAndExtent(st, 1, st, 4); const a = s.getComposedRanges(); const r = a[0]; const idx = Array.prototype.indexOf.call(box.childNodes, host); return [a.length, r.startContainer === box, r.startOffset === idx, r.endContainer === box, r.endOffset === idx + 1, r.collapsed]; });
        t('withRoot', () => { s.setBaseAndExtent(st, 1, st, 4); const r = s.getComposedRanges({ shadowRoots: [sr] })[0]; return [r.startContainer === st, r.startOffset, r.endContainer === st, r.endOffset]; });
        t('positional', () => { s.setBaseAndExtent(st, 1, st, 4); const r = s.getComposedRanges(sr)[0]; return [r.startContainer === st, r.startContainer === box]; });
        t('own', () => Reflect.ownKeys(s).length);
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["none"],
        serde_json::json!(["none", "None", null, 0, null, 0])
    );
    assert_eq!(
        o["collapse"],
        serde_json::json!(["none", "Caret", true, 2, true, 2])
    );
    assert_eq!(o["forward"], serde_json::json!(["forward", 1, 4, 1, 4]));
    assert_eq!(o["backward"], serde_json::json!(["backward", 4, 1, 4, 1]));
    assert_eq!(o["range"], serde_json::json!(["none", 0, 3]));
    assert_eq!(o["extend"], serde_json::json!(["backward", 3, 1]));
    assert_eq!(o["selectAll"], serde_json::json!(["forward", "Range"]));
    assert_eq!(o["removed"], serde_json::json!(["none", null]));
    assert_eq!(
        o["getters"],
        serde_json::json!([
            ["get direction", false, true, true],
            ["get baseNode", false, true, true],
            ["get baseOffset", false, true, true],
            ["get extentNode", false, true, true],
            ["get extentOffset", false, true, true]
        ])
    );
    assert_eq!(o["foreign"], "TypeError:Illegal invocation");
    assert_eq!(o["gcrLen"], serde_json::json!([0, "getComposedRanges"]));
    assert_eq!(o["empty"], serde_json::json!([true, 0]));
    assert_eq!(
        o["range1"],
        serde_json::json!([1, "StaticRange", true, 1, 4, false])
    );
    assert_eq!(o["backNorm"], serde_json::json!([1, 4, "backward"]));
    assert_eq!(o["fresh"], serde_json::json!([true, true]));
    assert_eq!(o["collapsedRange"], serde_json::json!([1, true, 2]));
    assert_eq!(
        o["badArgs"],
        serde_json::json!([
            "The provided value is not of type 'GetComposedRangesOptions'.",
            "The provided value is not of type 'GetComposedRangesOptions'.",
            "ok",
            "ok",
            "ok",
            "ok",
            "ok",
            "Failed to read the 'shadowRoots' property from 'GetComposedRangesOptions': Failed to convert value to 'ShadowRoot'.",
            "Failed to read the 'shadowRoots' property from 'GetComposedRangesOptions': The provided value cannot be converted to a sequence.",
            "ok",
            "Failed to read the 'shadowRoots' property from 'GetComposedRangesOptions': The provided value cannot be converted to a sequence."
        ])
    );
    assert_eq!(
        o["retarget"],
        serde_json::json!([1, true, true, true, true, false])
    );
    assert_eq!(o["withRoot"], serde_json::json!([true, 1, true, 4]));
    assert_eq!(o["positional"], serde_json::json!([false, true]));
    assert_eq!(o["own"], 0);
}

/// M4 刀74: TextMetrics against Chrome 145 (k74 probes): ten read-only
/// prototype accessors with pinned names over WeakMap state (instances carry no
/// own keys and cannot be written), `new TextMetrics()` illegal, foreign
/// receivers illegal, measureText's argument ladder and zero-size font, and the
/// metrics model's relations measured on Chrome (default alphabetic baseline,
/// hanging = 0.8*ascent, ideographic = -descent, top/middle/bottom shifts, ink
/// bounds aligned by textAlign, whitespace/empty text without ink). `width`
/// keeps the engine's persona-font model, so it is not compared to Chrome here.
#[tokio::test(flavor = "current_thread")]
async fn knife74_text_metrics() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const c = document.createElement('canvas'); const x = c.getContext('2d');
        const props = ['width', 'actualBoundingBoxLeft', 'actualBoundingBoxRight', 'fontBoundingBoxAscent', 'fontBoundingBoxDescent', 'actualBoundingBoxAscent', 'actualBoundingBoxDescent', 'hangingBaseline', 'alphabeticBaseline', 'ideographicBaseline'];
        const M = (font, text, base, align) => { x.font = font; x.textBaseline = base || 'alphabetic'; x.textAlign = align || 'start'; return x.measureText(text); };
        const rest = (m) => props.slice(3).map(k => +m[k].toFixed(3));
        t('shape', () => { const m = M('10px sans-serif', 'abc'); return [m.constructor.name, Reflect.ownKeys(m).length, Object.prototype.toString.call(m), Object.getPrototypeOf(m) === TextMetrics.prototype, JSON.stringify(m)]; });
        t('ctor', () => { try { new TextMetrics(); return 'NO'; } catch (e) { return e.name + ':' + e.message; } });
        t('getters', () => props.map(k => { const d = Object.getOwnPropertyDescriptor(TextMetrics.prototype, k); return [d.get.name, !!d.set, d.enumerable, d.configurable]; }));
        t('foreign', () => { try { Object.getOwnPropertyDescriptor(TextMetrics.prototype, 'width').get.call({}); return 'NO'; } catch (e) { return e.name + ':' + e.message; } });
        t('frozen', () => { const m = x.measureText('a'); const w = m.width; m.width = 99; return [m.width === w, Object.keys(m).length, x.measureText('a') !== x.measureText('a')]; });
        t('arial16', () => rest(M('16px Arial', 'Hello')));
        t('arial32', () => rest(M('32px Arial', 'Hello')));
        t('tall', () => rest(M('16px Arial', 'Hgjpq')));
        t('top', () => rest(M('16px Arial', 'Hello', 'top')));
        t('hanging', () => rest(M('16px Arial', 'Hello', 'hanging')));
        t('middle', () => rest(M('16px Arial', 'Hello', 'middle')));
        t('ideo', () => rest(M('16px Arial', 'Hello', 'ideographic')));
        t('bottom', () => rest(M('16px Arial', 'Hello', 'bottom')));
        t('empty', () => { const m = M('16px Arial', ''); return [m.width, m.actualBoundingBoxLeft, m.actualBoundingBoxRight, m.actualBoundingBoxAscent, m.actualBoundingBoxDescent, m.fontBoundingBoxAscent]; });
        t('space', () => { const m = M('16px Arial', ' '); return [m.actualBoundingBoxLeft, m.actualBoundingBoxRight, m.actualBoundingBoxAscent]; });
        t('align', () => { const w = M('16px Arial', 'Hello').width; return ['start', 'center', 'right', 'end'].map(a => { const m = M('16px Arial', 'Hello', 'alphabetic', a); return [+(m.actualBoundingBoxLeft - (a === 'center' ? w / 2 : a === 'right' || a === 'end' ? w : 0)).toFixed(3), +(m.actualBoundingBoxRight - (w - (a === 'center' ? w / 2 : a === 'right' || a === 'end' ? w : 0))).toFixed(3)]; }); });
        t('zero', () => props.map(k => M('0px Arial', 'Hello')[k]));
        t('args', () => { try { x.measureText(); return 'NO'; } catch (e) { return e.message; } });
        t('lens', () => [CanvasRenderingContext2D.prototype.measureText.length]);
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["shape"],
        serde_json::json!(["TextMetrics", 0, "[object TextMetrics]", true, "{}"])
    );
    assert_eq!(
        o["ctor"],
        "TypeError:Failed to construct 'TextMetrics': Illegal constructor"
    );
    let names = [
        "get width",
        "get actualBoundingBoxLeft",
        "get actualBoundingBoxRight",
        "get fontBoundingBoxAscent",
        "get fontBoundingBoxDescent",
        "get actualBoundingBoxAscent",
        "get actualBoundingBoxDescent",
        "get hangingBaseline",
        "get alphabeticBaseline",
        "get ideographicBaseline",
    ];
    assert_eq!(
        o["getters"],
        serde_json::json!(names
            .iter()
            .map(|n| serde_json::json!([n, false, true, true]))
            .collect::<Vec<_>>())
    );
    assert_eq!(o["foreign"], "TypeError:Illegal invocation");
    assert_eq!(o["frozen"], serde_json::json!([true, 0, true]));
    assert_eq!(
        o["arial16"],
        serde_json::json!([14, 3, 11.453, 0.188, 11.2, 0, -3])
    );
    assert_eq!(
        o["arial32"],
        serde_json::json!([29, 7, 22.906, 0.375, 23.2, 0, -7])
    );
    assert_eq!(
        o["tall"],
        serde_json::json!([14, 3, 11.453, 3.367, 11.2, 0, -3])
    );
    assert_eq!(
        o["top"],
        serde_json::json!([1.594, 15.406, -0.953, 12.594, -1.206, -12.406, -15.406])
    );
    assert_eq!(
        o["hanging"],
        serde_json::json!([2.8, 14.2, 0.253, 11.387, 0, -11.2, -14.2])
    );
    assert_eq!(
        o["middle"],
        serde_json::json!([9.594, 7.406, 7.047, 4.594, 6.794, -4.406, -7.406])
    );
    assert_eq!(
        o["ideo"],
        serde_json::json!([17, 0, 14.453, -2.813, 14.2, 3, 0])
    );
    assert_eq!(
        o["bottom"],
        serde_json::json!([17.594, -0.594, 15.047, -3.406, 14.794, 3.594, 0.594])
    );
    assert_eq!(o["empty"], serde_json::json!([0, 0, 0, 0, 0, 14]));
    assert_eq!(o["space"], serde_json::json!([0, 0, 0]));
    assert_eq!(
        o["align"],
        serde_json::json!([
            [-1.281, -0.594],
            [-1.281, -0.594],
            [-1.281, -0.594],
            [-1.281, -0.594]
        ])
    );
    assert_eq!(o["zero"], serde_json::json!(vec![0; 10]));
    assert_eq!(
        o["args"],
        "Failed to execute 'measureText' on 'CanvasRenderingContext2D': 1 argument required, but only 0 present."
    );
    assert_eq!(o["lens"], serde_json::json!([1]));
}

/// M4 刀75: the missing-member tail (audit missing 58 -> 0). One test over the
/// families: handler attributes round-trip, legacy constants on prototype+ctor,
/// AudioNode state as prototype accessors over real EventTarget nodes, FileList
/// with prototype length/item/iterator, ImageData/MediaSource/TextDecoderStream/
/// CryptoKey/RTCError lifted to accessors without own keys, cssFloat through the
/// style proxy, and the small reflections/methods.
#[tokio::test(flavor = "current_thread")]
async fn knife75_tail_members() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const own = (x) => Reflect.ownKeys(x).map(String).filter(k => k !== 'Symbol(Symbol.toStringTag)').join(',');
        t('tailErrors', () => (globalThis.__diting_tailErrors || []).length);
        t('handlers', () => { const r = []; for (const [C, ty] of [[MediaStream, 'onaddtrack'], [VisualViewport, 'onresize'], [ServiceWorkerContainer, 'oncontrollerchange'], [TextTrackCue, 'onenter'], [OffscreenCanvas, 'oncontextlost']]) { const d = Object.getOwnPropertyDescriptor(C.prototype, ty); const f = () => 1; r.push([d.get.name, d.set.name, d.set.length, typeof d.get.call.bind]); } return r.length; });
        t('handlerRoundTrip', () => { const m = new MediaStream(); const f = () => 1; m.onaddtrack = f; const a = m.onaddtrack === f; m.onaddtrack = 5; return [a, m.onaddtrack, Object.keys(m).length]; });
        t('constants', () => [WebSocket.CONNECTING, WebSocket.prototype.CLOSING, EventSource.prototype.CLOSED, EventSource.OPEN, Window.TEMPORARY, Window.prototype.PERSISTENT]);
        t('constDesc', () => { const d = Object.getOwnPropertyDescriptor(WebSocket.prototype, 'OPEN'); const s = Object.getOwnPropertyDescriptor(WebSocket, 'OPEN'); return [d.writable, d.enumerable, d.configurable, s.writable, s.enumerable, s.configurable]; });
        t('audioNode', () => { const c = new AudioContext(); const g = c.createGain(); return [own(g), g.context === c, g.numberOfInputs, g.numberOfOutputs, g.channelCount, g.channelCountMode, g.channelInterpretation, g instanceof AudioNode, g instanceof EventTarget]; });
        t('audioSet', () => { const c = new AudioContext(); const g = c.createGain(); g.channelCount = 4; g.channelCountMode = 'explicit'; g.channelCountMode = 'bogus'; g.channelInterpretation = 'discrete'; let e; try { g.channelCount = 0; } catch (x) { e = x.name; } return [g.channelCount, g.channelCountMode, g.channelInterpretation, e]; });
        t('audioConnect', () => { const c = new AudioContext(); const g = c.createGain(); const r = g.connect(c.destination); return [r === c.destination, typeof g.disconnect, AudioNode.prototype.connect.length, Object.prototype.hasOwnProperty.call(g, 'connect')]; });
        t('audioEvents', () => { const c = new AudioContext(); const g = c.createGain(); let n = 0; g.addEventListener('x', () => n++); g.dispatchEvent(new Event('x')); return n; });
        t('sink', () => { const c = new AudioContext(); return [c.sinkId, typeof c.setSinkId, c.setSinkId.length, c.onsinkchange, typeof c.audioWorklet]; });
        t('fileList', () => { const i = document.createElement('input'); i.type = 'file'; const fl = i.files; return [fl.constructor.name, fl.length, own(fl), Object.getOwnPropertyDescriptor(FileList.prototype, 'length').get.name, FileList.prototype.item.length, Object.prototype.toString.call(fl), [...fl].length]; });
        t('fileListFoo', () => { const dt = new DataTransfer(); dt.items.add(new File(['a'], 'a.txt')); const fl = dt.files; return [fl.length, fl.item(0).name, fl[0].name, own(fl), fl.item(5), [...fl].length]; });
        t('file', () => { const f = new File(['a'], 'a.txt', { lastModified: 5000 }); return [f.lastModifiedDate.getTime(), f.webkitRelativePath, own(f)]; });
        t('imageData', () => { const d = new ImageData(2, 2); return [d.colorSpace, d.pixelFormat, Object.keys(d), d.width, d.height, d.data.length]; });
        t('lifted', () => [own(new TextDecoderStream()), own(new TextEncoderStream()), own(new MediaSource()), own(new RTCError({ errorDetail: 'data-channel-failure' }))]);
        t('liftedValues', () => { const s = new TextDecoderStream('utf-8', { fatal: true }); const e = new RTCError({ errorDetail: 'dtls-failure', sdpLineNumber: 3 }); return [s.encoding, s.fatal, s.ignoreBOM, typeof s.readable, e.errorDetail, e.sdpLineNumber, e.httpRequestStatusCode]; });
        t('cssFloat', () => { const d = document.createElement('div'); d.style.cssFloat = 'left'; const r = [d.style.cssFloat, d.style.float, d.getAttribute('style').replace(/\s/g, '')]; d.style.float = 'right'; r.push(d.style.cssFloat, 'cssFloat' in d.style, Object.prototype.hasOwnProperty.call(d.style, 'cssFloat'), d.style.parentRule); return r; });
        t('html', () => { const h = document.documentElement; h.version = '1'; const f = document.createElement('iframe'); f.privateToken = 'x'; f.browsingTopics = true; return [h.version, h.getAttribute('version'), f.privateToken, f.browsingTopics, f.hasAttribute('browsingtopics'), typeof f.featurePolicy]; });
        t('doctype', () => { const dt = document.implementation.createDocumentType('html', '', ''); const d2 = document.implementation.createDocument(null, null, dt); const r = [typeof dt.after, typeof dt.before, typeof dt.remove, typeof dt.replaceWith, DocumentType.prototype.remove.length]; return r; });
        t('svg', () => { const NS = 'http://www.w3.org/2000/svg'; const s = document.createElementNS(NS, 'style'); s.media = 'print'; s.title = 'T'; const p = document.createElementNS(NS, 'polygon'); return [s.getAttribute('media'), s.title, p.pathLength.constructor.name, typeof p.points, typeof p.animatedPoints, p.isPointInFill.length, p.isPointInFill(new DOMPoint(0, 0))]; });
        t('xhr', () => { const x = new XMLHttpRequest(); let e1, e2; try { x.setPrivateToken({}); } catch (e) { e1 = e.name; } try { x.setAttributionReporting(); } catch (e) { e2 = e.message; } return [e1, e2]; });
        t('geo', () => [typeof GeolocationPosition.prototype.toJSON, typeof GeolocationCoordinates.prototype.toJSON]);
        t('perf', () => { const j = performance.toJSON(); return [typeof j.timeOrigin, typeof j.timing, typeof performance.interactionCount, typeof performance.onresourcetimingbufferfull]; });
        t('misc', () => [typeof Path2D.prototype.roundRect, Path2D.prototype.roundRect.length, typeof Range.prototype.expand, typeof HTMLCanvasElement.prototype.captureStream, typeof HTMLCanvasElement.prototype.transferControlToOffscreen, typeof CSSStyleRule.prototype.styleMap]);
        t('sourceBuffer', () => { const d = k => Object.getOwnPropertyDescriptor(SourceBuffer.prototype, k); return ['mode', 'updating', 'timestampOffset', 'appendWindowStart', 'appendWindowEnd'].map(k => [!!d(k).get, !!d(k).set]); });
        t('cryptoKey', () => { const d = k => Object.getOwnPropertyDescriptor(CryptoKey.prototype, k); return ['type', 'extractable', 'algorithm', 'usages'].map(k => typeof d(k).get); });
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(o["tailErrors"], 0);
    assert_eq!(o["handlerRoundTrip"], serde_json::json!([true, null, 0]));
    assert_eq!(o["constants"], serde_json::json!([0, 2, 2, 1, 0, 1]));
    assert_eq!(
        o["constDesc"],
        serde_json::json!([false, true, false, false, true, false])
    );
    assert_eq!(
        o["audioNode"],
        serde_json::json!(["gain", true, 1, 1, 2, "max", "speakers", true, true])
    );
    assert_eq!(
        o["audioSet"],
        serde_json::json!([4, "explicit", "discrete", "NotSupportedError"])
    );
    assert_eq!(
        o["audioConnect"],
        serde_json::json!([true, "function", 1, false])
    );
    assert_eq!(o["audioEvents"], 1);
    assert_eq!(
        o["sink"],
        serde_json::json!(["", "function", 1, null, "object"])
    );
    assert_eq!(
        o["fileList"],
        serde_json::json!(["FileList", 0, "", "get length", 1, "[object FileList]", 0])
    );
    assert_eq!(
        o["fileListFoo"],
        serde_json::json!([1, "a.txt", "a.txt", "0", null, 1])
    );
    assert_eq!(o["file"], serde_json::json!([5000, "", ""]));
    assert_eq!(
        o["imageData"],
        serde_json::json!(["srgb", "rgba-unorm8", ["data"], 2, 2, 16])
    );
    assert_eq!(o["lifted"], serde_json::json!(["", "", "", ""]));
    assert_eq!(
        o["liftedValues"],
        serde_json::json!(["utf-8", true, false, "object", "dtls-failure", 3, null])
    );
    assert_eq!(
        o["cssFloat"],
        serde_json::json!(["left", "left", "float:left;", "right", true, false, null])
    );
    assert_eq!(
        o["html"],
        serde_json::json!(["1", "1", "x", true, true, "object"])
    );
    assert_eq!(
        o["doctype"],
        serde_json::json!(["function", "function", "function", "function", 0])
    );
    assert_eq!(
        o["svg"],
        serde_json::json!([
            "print",
            "T",
            "SVGAnimatedNumber",
            "object",
            "object",
            0,
            false
        ])
    );
    assert_eq!(
        o["xhr"],
        serde_json::json!([
            "InvalidStateError",
            "Failed to execute 'setAttributionReporting' on 'XMLHttpRequest': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(o["geo"], serde_json::json!(["function", "function"]));
    assert_eq!(
        o["perf"],
        serde_json::json!(["number", "object", "number", "object"])
    );
    assert_eq!(
        o["misc"],
        serde_json::json!(["function", 4, "function", "function", "function", "object"])
    );
    // 刀84: SourceBuffer.updating is read-only in Chrome (sealed by the lift RO table); the other four stay rw.
    assert_eq!(
        o["sourceBuffer"],
        serde_json::json!([
            [true, true],
            [true, false],
            [true, true],
            [true, true],
            [true, true]
        ])
    );
    assert_eq!(
        o["cryptoKey"],
        serde_json::json!(["function", "function", "function", "function"])
    );
}

/// M4 刀77: speechSynthesis is a real utterance queue (the persona advertises
/// voices, so speech "plays"): speak() queues and the head utterance fires
/// start then end (SpeechSynthesisEvent), speaking/pending track the queue,
/// cancel() fires error:'canceled' on the running and every queued utterance,
/// pause()/resume() freeze/continue the running one.
#[tokio::test(flavor = "current_thread")]
async fn knife77_speech_queue_lifecycle() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(r#"
        globalThis.__log = [];
        const mk = (name, text) => { const u = new SpeechSynthesisUtterance(text); for (const ty of ['start', 'end', 'error', 'pause', 'resume']) u.addEventListener(ty, (e) => __log.push([name, e.type, e.constructor.name, e.utterance === u, e.error || null])); return u; };
        const a = mk('a', 'hi'), b = mk('b', 'yo'), c = mk('c', 'later'), d = mk('d', 'dropped');
        speechSynthesis.speak(a); speechSynthesis.speak(b);
        __log.push(['state0', speechSynthesis.speaking, speechSynthesis.pending, speechSynthesis.paused]);
        globalThis.__done = new Promise((res) => {
          b.addEventListener('end', () => {
            __log.push(['state1', speechSynthesis.speaking, speechSynthesis.pending]);
            speechSynthesis.speak(c); speechSynthesis.speak(d);
            c.addEventListener('start', () => {
              speechSynthesis.pause(); __log.push(['paused', speechSynthesis.paused]);
              speechSynthesis.resume(); __log.push(['resumed', speechSynthesis.paused]);
              speechSynthesis.cancel();
              __log.push(['state2', speechSynthesis.speaking, speechSynthesis.pending]);
              setTimeout(res, 50);
            });
          });
        });
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(2500).await;
    let log = rt.evaluate("JSON.stringify(globalThis.__log)").unwrap();
    let l: serde_json::Value = serde_json::from_str(log.as_str().unwrap()).unwrap();
    assert_eq!(l[0], serde_json::json!(["state0", true, true, false]));
    assert_eq!(
        l[1],
        serde_json::json!(["a", "start", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(
        l[2],
        serde_json::json!(["a", "end", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(
        l[3],
        serde_json::json!(["b", "start", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(
        l[4],
        serde_json::json!(["b", "end", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(l[5], serde_json::json!(["state1", false, false]));
    assert_eq!(
        l[6],
        serde_json::json!(["c", "start", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(
        l[7],
        serde_json::json!(["c", "pause", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(l[8], serde_json::json!(["paused", true]));
    assert_eq!(
        l[9],
        serde_json::json!(["c", "resume", "SpeechSynthesisEvent", true, null])
    );
    assert_eq!(l[10], serde_json::json!(["resumed", false]));
    assert_eq!(l[11], serde_json::json!(["state2", false, false]));
    assert_eq!(
        l[12],
        serde_json::json!(["c", "error", "SpeechSynthesisErrorEvent", true, "canceled"])
    );
    assert_eq!(
        l[13],
        serde_json::json!(["d", "error", "SpeechSynthesisErrorEvent", true, "canceled"])
    );
}

/// M4 刀77: every SVG tag wraps as its Chrome interface (63 tags, captured from Chrome 145), the formerly
/// pure-stub SVG interfaces (mask/filter/fe*/light sources/transfer functions/textPath/view/script/mpath)
/// answer real defaults through animated views, SVGStringList is indexed and systemLanguage is
/// comma-separated.
#[tokio::test(flavor = "current_thread")]
async fn knife77_svg_tags_and_filter_reflectors() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        const NS = 'http://www.w3.org/2000/svg'; const mk = (n) => document.createElementNS(NS, n);
        const want = TAGS_JSON;
        t('tags', () => { const bad = []; for (const [tag, w] of Object.entries(want)) { const e = mk(tag); const got = e.constructor.name + '>' + Object.getPrototypeOf(Object.getPrototypeOf(e)).constructor.name; if (got !== w) bad.push([tag, got, w]); } return bad; });
        t('mask', () => { const m = mk('mask'); return [m.maskUnits.baseVal, m.maskContentUnits.baseVal, m.maskUnits.constructor.name, m.requiredExtensions.constructor.name, m.x.constructor.name]; });
        t('maskLen', () => { const m = mk('mask'); try { return m.x.baseVal.value; } catch (e) { return e.name; } });
        t('filter', () => { const f = mk('filter'); f.setAttribute('filterUnits', 'userSpaceOnUse'); return [f.filterUnits.baseVal, f.primitiveUnits.baseVal, f.href.constructor.name]; });
        t('feBlend', () => { const e = mk('feBlend'); const r = [e.mode.baseVal, e.in1.baseVal]; e.setAttribute('mode', 'multiply'); e.setAttribute('in', 'SourceGraphic'); r.push(e.mode.baseVal, e.in1.baseVal); e.setAttribute('mode', 'junk'); r.push(e.mode.baseVal); return r; });
        t('feColorMatrix', () => { const e = mk('feColorMatrix'); const r = [e.type.baseVal, e.values.baseVal.numberOfItems]; e.setAttribute('type', 'hueRotate'); r.push(e.type.baseVal); return r; });
        t('feComposite', () => { const e = mk('feComposite'); const r = [e.operator.baseVal, e.k1.baseVal, e.k2.baseVal]; e.setAttribute('operator', 'arithmetic'); e.setAttribute('k1', '2.5'); r.push(e.operator.baseVal, e.k1.baseVal); return r; });
        t('feConvolve', () => { const e = mk('feConvolveMatrix'); const r = [e.orderX.baseVal, e.orderY.baseVal, e.divisor.baseVal, e.edgeMode.baseVal, e.preserveAlpha.baseVal, e.orderX.constructor.name, e.preserveAlpha.constructor.name]; e.setAttribute('order', '5 7'); r.push(e.orderX.baseVal, e.orderY.baseVal); e.setAttribute('preserveAlpha', 'true'); r.push(e.preserveAlpha.baseVal); return r; });
        t('feTurbulence', () => { const e = mk('feTurbulence'); const r = [e.type.baseVal, e.stitchTiles.baseVal, e.numOctaves.baseVal, e.seed.baseVal, e.baseFrequencyX.baseVal]; e.setAttribute('baseFrequency', '0.5 0.25'); r.push(e.baseFrequencyX.baseVal, e.baseFrequencyY.baseVal); return r; });
        t('feBlur', () => { const e = mk('feGaussianBlur'); e.setAttribute('stdDeviation', '3 4'); return [e.stdDeviationX.baseVal, e.stdDeviationY.baseVal, typeof e.setStdDeviation]; });
        t('feFunc', () => { const e = mk('feFuncR'); return [e.type.baseVal, e.slope.baseVal, e.exponent.baseVal, e.tableValues.baseVal.numberOfItems, e instanceof SVGComponentTransferFunctionElement]; });
        t('light', () => [mk('feDistantLight').azimuth.baseVal, mk('feSpotLight').limitingConeAngle.baseVal, mk('fePointLight').z.baseVal]);
        t('textPath', () => { const e = mk('textPath'); return [e.method.baseVal, e.spacing.baseVal, e.href.baseVal, e.startOffset.baseVal.unitType]; });
        t('script', () => { const s = mk('script'); const r = [s.type, s.async]; s.type = 'text/x'; s.async = true; r.push(s.getAttribute('type'), s.hasAttribute('async'), s.async); return r; });
        t('view', () => { const v = mk('view'); const r = [v.zoomAndPan]; v.zoomAndPan = 1; r.push(v.zoomAndPan); return r; });
        t('foreign', () => { try { Object.getOwnPropertyDescriptor(SVGFEBlendElement.prototype, 'mode').get.call(mk('rect')); return 'NO'; } catch (e) { return e.message; } });
        t('strlist', () => { const r = mk('rect'); r.setAttribute('systemLanguage', 'en, fr'); const l = r.systemLanguage; return [l.length, l[0], l[1], l[2], Object.keys(l).join(','), 1 in l]; });
        return JSON.stringify(o);
    })()"#.replace("TAGS_JSON", TAGS);
    let v = rt.evaluate(&script).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(o["tags"], serde_json::json!([]));
    assert_eq!(
        o["mask"],
        serde_json::json!([
            2,
            1,
            "SVGAnimatedEnumeration",
            "SVGStringList",
            "SVGAnimatedLength"
        ])
    );
    assert_eq!(o["maskLen"], "NotSupportedError");
    assert_eq!(o["filter"], serde_json::json!([1, 1, "SVGAnimatedString"]));
    assert_eq!(
        o["feBlend"],
        serde_json::json!([1, "", 2, "SourceGraphic", 1])
    );
    assert_eq!(o["feColorMatrix"], serde_json::json!([1, 0, 3]));
    assert_eq!(o["feComposite"], serde_json::json!([1, 0, 0, 6, 2.5]));
    assert_eq!(
        o["feConvolve"],
        serde_json::json!([
            3,
            3,
            1,
            1,
            false,
            "SVGAnimatedInteger",
            "SVGAnimatedBoolean",
            5,
            7,
            true
        ])
    );
    assert_eq!(
        o["feTurbulence"],
        serde_json::json!([2, 2, 1, 0, 0, 0.5, 0.25])
    );
    assert_eq!(o["feBlur"], serde_json::json!([3, 4, "function"]));
    assert_eq!(o["feFunc"], serde_json::json!([1, 1, 1, 0, true]));
    assert_eq!(o["textPath"], serde_json::json!([1, 2, "", 1]));
    assert_eq!(
        o["script"],
        serde_json::json!(["", false, "text/x", true, true])
    );
    assert_eq!(o["view"], serde_json::json!([2, 1]));
    assert_eq!(o["foreign"], "Illegal invocation");
    assert_eq!(
        o["strlist"],
        serde_json::json!([2, "en", "fr", null, "0,1", true])
    );
}

/// M4 刀78: constructor fidelity (Chrome 145 `new X()` probe over all 936 interfaces): Illegal-constructor
/// messages carry the constructed interface's name and no trailing period (and are real TypeErrors, not
/// DOMExceptions named TypeError), twenty interfaces enforce Chrome's "N argument(s) required" ladder, the
/// nid-based node classes reject the zero-argument page form, and 15 legacy HTML tags wrap as their interface.
#[tokio::test(flavor = "current_thread")]
async fn knife78_constructor_ladders_and_html_tags() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = e.name + ':' + e.message; } };
        const ctor = (n, ...a) => { try { new globalThis[n](...a); return 'ok'; } catch (e) { return (e instanceof TypeError ? 'TypeError' : e.name) + ':' + e.message; } };
        t('illegal', () => ['Blob', 'AudioNode', 'CryptoKey', 'FileList', 'StyleSheet', 'HTMLFormElement', 'FeaturePolicy', 'GeolocationCoordinates', 'HTMLMediaElement', 'HTMLUnknownElement', 'CSSStyleDeclaration', 'TextMetrics'].map(n => ctor(n)).filter(r => r !== 'ok'));
        t('ladders', () => ['AudioBuffer', 'BroadcastChannel', 'CloseEvent', 'ErrorEvent', 'EventSource', 'ImageData', 'MessageEvent', 'MutationObserver', 'OffscreenCanvas', 'PerformanceMark', 'ProgressEvent', 'RTCError', 'ResizeObserver', 'SharedWorker', 'ToggleEvent', 'Touch', 'TouchEvent', 'Worker'].map(n => ctor(n)));
        t('ladderOk', () => [ctor('ProgressEvent', 'x'), ctor('MutationObserver', () => {}), ctor('ErrorEvent', 'error'), ctor('ImageData', 1, 1), ctor('MessageEvent', 'message')]);
        t('nodes', () => ['Node', 'Element', 'CharacterData', 'CDATASection', 'DocumentType', 'ProcessingInstruction'].map(n => ctor(n)));
        t('stillWorks', () => { const d = document.createElement('div'); d.appendChild(document.createTextNode('x')); const c = document.createComment('c'); return [d.firstChild.data, c.nodeType, d instanceof Element, d instanceof Node, Node.prototype.constructor === Node, ProgressEvent.prototype.constructor === ProgressEvent]; });
        t('subclass', () => { class MyEv extends ProgressEvent {} const e = new MyEv('x'); return [e.type, e instanceof ProgressEvent, e instanceof MyEv, ProgressEvent.length, ProgressEvent.name]; });
        t('htmlTags', () => ['applet', 'bgsound', 'blink', 'data', 'frameset', 'image', 'isindex', 'keygen', 'listing', 'menuitem', 'multicol', 'nextid', 'picture', 'spacer', 'xmp'].map(tag => document.createElement(tag).constructor.name));
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(
        o["illegal"],
        serde_json::json!([
            "TypeError:Failed to construct 'AudioNode': Illegal constructor",
            "TypeError:Failed to construct 'CryptoKey': Illegal constructor",
            "TypeError:Failed to construct 'FileList': Illegal constructor",
            "TypeError:Failed to construct 'StyleSheet': Illegal constructor",
            "TypeError:Failed to construct 'HTMLFormElement': Illegal constructor",
            "TypeError:Failed to construct 'FeaturePolicy': Illegal constructor",
            "TypeError:Failed to construct 'GeolocationCoordinates': Illegal constructor",
            "TypeError:Failed to construct 'HTMLMediaElement': Illegal constructor",
            "TypeError:Failed to construct 'HTMLUnknownElement': Illegal constructor",
            "TypeError:Failed to construct 'CSSStyleDeclaration': Illegal constructor",
            "TypeError:Failed to construct 'TextMetrics': Illegal constructor"
        ])
    );
    let lad: Vec<String> = serde_json::from_value(o["ladders"].clone()).unwrap();
    let want = [
        ("AudioBuffer", 1),
        ("BroadcastChannel", 1),
        ("CloseEvent", 1),
        ("ErrorEvent", 1),
        ("EventSource", 1),
        ("ImageData", 2),
        ("MessageEvent", 1),
        ("MutationObserver", 1),
        ("OffscreenCanvas", 2),
        ("PerformanceMark", 1),
        ("ProgressEvent", 1),
        ("RTCError", 1),
        ("ResizeObserver", 1),
        ("SharedWorker", 1),
        ("ToggleEvent", 1),
        ("Touch", 1),
        ("TouchEvent", 1),
        ("Worker", 1),
    ];
    for (got, (name, n)) in lad.iter().zip(want.iter()) {
        let w = format!(
            "TypeError:Failed to construct '{}': {} argument{} required, but only 0 present.",
            name,
            n,
            if *n > 1 { "s" } else { "" }
        );
        assert_eq!(got, &w);
    }
    assert_eq!(
        o["ladderOk"],
        serde_json::json!(["ok", "ok", "ok", "ok", "ok"])
    );
    assert_eq!(
        o["nodes"],
        serde_json::json!([
            "Node",
            "Element",
            "CharacterData",
            "CDATASection",
            "DocumentType",
            "ProcessingInstruction"
        ]
        .iter()
        .map(|n| format!("TypeError:Failed to construct '{}': Illegal constructor", n))
        .collect::<Vec<_>>())
    );
    assert_eq!(
        o["stillWorks"],
        serde_json::json!(["x", 8, true, true, true, true])
    );
    assert_eq!(
        o["subclass"],
        serde_json::json!(["x", true, true, 1, "ProgressEvent"])
    );
    assert_eq!(
        o["htmlTags"],
        serde_json::json!([
            "HTMLUnknownElement",
            "HTMLUnknownElement",
            "HTMLUnknownElement",
            "HTMLDataElement",
            "HTMLFrameSetElement",
            "HTMLUnknownElement",
            "HTMLUnknownElement",
            "HTMLUnknownElement",
            "HTMLPreElement",
            "HTMLUnknownElement",
            "HTMLUnknownElement",
            "HTMLUnknownElement",
            "HTMLPictureElement",
            "HTMLUnknownElement",
            "HTMLPreElement"
        ])
    );
}

/// M4 刀79: default-value reconciliation (every no-arg constructible interface, Chrome 145 vs engine): pure-stub
/// getters answer Chrome's primitive/null defaults instead of undefined, Animation.rangeStart/End are 'normal',
/// MediaStream has an uuid id and starts inactive, `new Image().complete` is true, XHR.response is '' by default,
/// MediaSource has the three source* handlers, SourceBufferList has Chrome's constructor name.
#[tokio::test(flavor = "current_thread")]
async fn knife79_default_values() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r#"(function () {
        const o = {};
        const t = (k, f) => { try { o[k] = f(); } catch (e) { o[k] = 'THROW:' + e.name + ':' + e.message; } };
        t('highlight', () => { const h = new Highlight(); return [h.priority, h.size, h.type]; });
        t('idle', () => { const d = new IdleDetector(); return [d.onchange, d.screenState, d.userState]; });
        t('closeWatcher', () => { const c = new CloseWatcher(); return [c.oncancel, c.onclose]; });
        t('captureController', () => { const c = new CaptureController(); return [c.zoomLevel, c.onzoomlevelchange]; });
        t('origin', () => new Origin().opaque);
        t('scroll', () => new ScrollTimeline().axis);
        t('view', () => { const v = new ViewTimeline(); return [v.startOffset, v.endOffset, v.subject]; });
        t('wsError', () => { const e = new WebSocketError(); return [e.closeCode, e.reason]; });
        t('wtError', () => { const e = new WebTransportError(); return [e.source, e.streamErrorCode]; });
        t('quota', () => { const e = new QuotaExceededError(); return [e.quota, e.requested]; });
        t('idcred', () => { const e = new IdentityCredentialError(); return [e.code, e.error, e.url]; });
        t('anim', () => { const a = new Animation(); return [a.rangeStart, a.rangeEnd]; });
        t('stream', () => { const s = new MediaStream(); return [s.active, /^[0-9a-f-]{36}$/.test(s.id), new MediaStream().id !== s.id]; });
        t('image', () => { const i = new Image(); const r = [i.complete]; return r; });
        t('xhr', () => new XMLHttpRequest().response);
        t('mediaSource', () => { const m = new MediaSource(); return [m.onsourceopen, m.onsourceended, m.onsourceclose, m.sourceBuffers.constructor.name, m.activeSourceBuffers.constructor.name]; });
        return JSON.stringify(o);
    })()"#).unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    assert_eq!(o["highlight"], serde_json::json!([0, 0, "highlight"]));
    assert_eq!(o["idle"], serde_json::json!([null, null, null]));
    assert_eq!(o["closeWatcher"], serde_json::json!([null, null]));
    assert_eq!(o["captureController"], serde_json::json!([null, null]));
    assert_eq!(o["origin"], true);
    assert_eq!(o["scroll"], "block");
    assert_eq!(o["view"], serde_json::json!([null, null, null]));
    assert_eq!(o["wsError"], serde_json::json!([null, ""]));
    assert_eq!(o["wtError"], serde_json::json!(["stream", null]));
    assert_eq!(o["quota"], serde_json::json!([null, null]));
    assert_eq!(o["idcred"], serde_json::json!(["", "", ""]));
    assert_eq!(o["anim"], serde_json::json!(["normal", "normal"]));
    assert_eq!(o["stream"], serde_json::json!([false, true, true]));
    assert_eq!(o["image"], serde_json::json!([true]));
    assert_eq!(o["xhr"], "");
    assert_eq!(
        o["mediaSource"],
        serde_json::json!([null, null, null, "SourceBufferList", "SourceBufferList"])
    );
}

/// M4 刀80: WebIDL required-argument ladders on the methods whose no-argument call differed from Chrome
/// (76 sync TypeErrors + 4 promise rejections, derived from a Chrome-vs-engine probe over every no-arg
/// constructible interface): message text, plural rule, rejection for promise-returning operations, wrapper
/// shape (no prototype, native, Chrome length), and that normal calls still pass straight through.
#[tokio::test(flavor = "current_thread")]
async fn knife80_method_argument_ladders() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(r#"
        globalThis.__r = {};
        const t = (k, f) => { try { __r[k] = f(); } catch (e) { __r[k] = e.name + ':' + e.message; } };
        t('formdata', () => { const f = new FormData(); const out = []; for (const m of ['append', 'delete', 'get', 'getAll', 'forEach']) { try { f[m](); out.push('NO'); } catch (e) { out.push(e.message); } } return out; });
        t('usp', () => { const u = new URLSearchParams(); const out = []; for (const m of ['append', 'delete', 'get', 'getAll']) { try { u[m](); out.push('NO'); } catch (e) { out.push(e.message); } } return out; });
        t('path2d', () => { const p = new Path2D(); const out = []; for (const m of ['arc', 'lineTo', 'ellipse', 'addPath']) { try { p[m](); out.push('NO'); } catch (e) { out.push(e.message); } } return out; });
        t('xhr', () => { const x = new XMLHttpRequest(); const out = []; for (const m of ['open', 'setRequestHeader', 'overrideMimeType', 'getResponseHeader']) { try { x[m](); out.push('NO'); } catch (e) { out.push(e.message); } } return out; });
        t('frag', () => { const f = document.createDocumentFragment(); const out = []; for (const m of ['getElementById', 'querySelector', 'querySelectorAll']) { try { f[m](); out.push('NO'); } catch (e) { out.push(e.message); } } return out; });
        t('range', () => { const r = document.createRange(); const out = []; for (const m of ['selectNode', 'comparePoint', 'insertNode']) { try { r[m](); out.push('NO'); } catch (e) { out.push(e.message); } } return out; });
        t('shape', () => { const d = Object.getOwnPropertyDescriptor(FormData.prototype, 'append'); const f = d.value; return [f.length, f.name, 'prototype' in f, d.writable, d.enumerable, d.configurable, Function.prototype.toString.call(f).indexOf('[native code]') > 0]; });
        t('passthrough', () => { const f = new FormData(); f.append('a', '1'); const u = new URLSearchParams(); u.append('x', 'y'); const x = new XMLHttpRequest(); x.open('GET', 'http://example.invalid/'); return [f.get('a'), u.get('x'), x.readyState, f.getAll('a').length]; });
        globalThis.__p = (async () => {
            const out = [];
            for (const f of [() => new CSSStyleSheet().replace(), () => new ReadableStream().pipeTo()]) { try { const p = f(); out.push(p instanceof Promise); await p; out.push('resolved'); } catch (e) { out.push(e.message); } }
            return out;
        })();
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(200).await;
    let v = rt.evaluate("JSON.stringify([globalThis.__r])").unwrap();
    let o: serde_json::Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let r = &o[0];
    assert_eq!(
        r["formdata"],
        serde_json::json!([
            "Failed to execute 'append' on 'FormData': 2 arguments required, but only 0 present.",
            "Failed to execute 'delete' on 'FormData': 1 argument required, but only 0 present.",
            "Failed to execute 'get' on 'FormData': 1 argument required, but only 0 present.",
            "Failed to execute 'getAll' on 'FormData': 1 argument required, but only 0 present.",
            "Failed to execute 'forEach' on 'FormData': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(
        r["usp"],
        serde_json::json!([
            "Failed to execute 'append' on 'URLSearchParams': 2 arguments required, but only 0 present.",
            "Failed to execute 'delete' on 'URLSearchParams': 1 argument required, but only 0 present.",
            "Failed to execute 'get' on 'URLSearchParams': 1 argument required, but only 0 present.",
            "Failed to execute 'getAll' on 'URLSearchParams': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(
        r["path2d"],
        serde_json::json!([
            "Failed to execute 'arc' on 'Path2D': 5 arguments required, but only 0 present.",
            "Failed to execute 'lineTo' on 'Path2D': 2 arguments required, but only 0 present.",
            "Failed to execute 'ellipse' on 'Path2D': 7 arguments required, but only 0 present.",
            "Failed to execute 'addPath' on 'Path2D': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(
        r["xhr"],
        serde_json::json!([
            "Failed to execute 'open' on 'XMLHttpRequest': 2 arguments required, but only 0 present.",
            "Failed to execute 'setRequestHeader' on 'XMLHttpRequest': 2 arguments required, but only 0 present.",
            "Failed to execute 'overrideMimeType' on 'XMLHttpRequest': 1 argument required, but only 0 present.",
            "Failed to execute 'getResponseHeader' on 'XMLHttpRequest': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(
        r["frag"],
        serde_json::json!([
            "Failed to execute 'getElementById' on 'DocumentFragment': 1 argument required, but only 0 present.",
            "Failed to execute 'querySelector' on 'DocumentFragment': 1 argument required, but only 0 present.",
            "Failed to execute 'querySelectorAll' on 'DocumentFragment': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(
        r["range"],
        serde_json::json!([
            "Failed to execute 'selectNode' on 'Range': 1 argument required, but only 0 present.",
            "Failed to execute 'comparePoint' on 'Range': 2 arguments required, but only 0 present.",
            "Failed to execute 'insertNode' on 'Range': 1 argument required, but only 0 present."
        ])
    );
    assert_eq!(
        r["shape"],
        serde_json::json!([2, "append", false, true, true, true, true])
    );
    assert_eq!(r["passthrough"], serde_json::json!(["1", "y", 1, 1]));
    let p = rt
        .evaluate("globalThis.__p.then ? 'promise' : 'other'")
        .unwrap();
    assert_eq!(p, "promise");
}

#[tokio::test(flavor = "current_thread")]
async fn knife82_illegal_zero_arg_constructors() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"(() => { const out = [];
      for (const n of ['BaseAudioContext','CustomElementRegistry','DOMTokenList','ElementInternals','GPU','GPUAdapter','MediaQueryList','MessagePort','MimeType','MimeTypeArray','PerformanceEntry','PerformanceObserverEntryList','Plugin','PluginArray','SVGPoint','Selection','ServiceWorkerContainer','SourceBuffer','XMLHttpRequestEventTarget']) {
        try { new globalThis[n](); out.push(n + ':ok'); } catch (e) { out.push(n + ':' + e.message); }
      }
      return out.join('|'); })()"#;
    let r = rt
        .evaluate(script)
        .unwrap()
        .as_str()
        .unwrap_or_default()
        .to_string();
    for n in ["BaseAudioContext", "Selection", "SVGPoint", "MessagePort"] {
        assert!(
            r.contains(&format!(
                "{n}:Failed to construct '{n}': Illegal constructor"
            )),
            "{r}"
        );
    }
    assert!(!r.contains(":ok"), "{r}");
}

#[tokio::test(flavor = "current_thread")]
async fn knife83_mint_sealed_constructors() {
    let mut rt = setup_runtime("<html><body><div>a</div><div>b</div></body></html>");
    rt.evaluate(r#"
        globalThis.__r = {};
        const t = (k, f) => { try { __r[k] = f(); } catch (e) { __r[k] = e.name + ':' + e.message; } };
        t('nl0', () => new NodeList());
        t('nl1', () => new NodeList(1));
        t('ib0', () => new ImageBitmap());
        t('ib1', () => new ImageBitmap(undefined));
        t('subclass', () => { class A extends NodeList {}; return new A(); });
        t('qsa', () => { const l = document.querySelectorAll('div'); return [l.length, l instanceof NodeList, Object.prototype.toString.call(l), l.item(0).textContent]; });
        t('cn', () => { const l = document.body.childNodes; return [l instanceof NodeList, Object.prototype.toString.call(l)]; });
        t('cib', () => { const c = document.createElement('canvas'); c.width = 3; c.height = 4; return createImageBitmap(c); });
    "#).unwrap();
    let r = rt.evaluate("globalThis.__r").unwrap();
    for k in ["nl0", "nl1", "ib0", "ib1", "subclass"] {
        let v = r[k].as_str().unwrap_or_default();
        let name = if k.starts_with("ib") {
            "ImageBitmap"
        } else {
            "NodeList"
        };
        assert_eq!(
            v,
            format!("TypeError:Failed to construct '{name}': Illegal constructor"),
            "{k}={v}"
        );
    }
    assert_eq!(
        r["qsa"],
        serde_json::json!([2, true, "[object NodeList]", "a"])
    );
    assert_eq!(r["cn"], serde_json::json!([true, "[object NodeList]"]));
    let result = rt
        .call_function_on_remote(
            r#"async () => { const b = await globalThis.__r.cib; return b.width + 'x' + b.height + ':' + (b instanceof ImageBitmap); }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();
    let dims = result.value.unwrap();
    assert_eq!(dims, serde_json::json!("3x4:true"));
}

/// M4 刀84: the lift tables seal Chrome's ro/rw split. Every member Chrome exposes as a
/// read-only accessor is a getter-only accessor on the engine prototype too: page writes are
/// silent no-ops in sloppy mode (never "Cannot set property"), strict writes throw TypeError,
/// and the engine's own state changes still land — internal write sites go through the hidden
/// lift slot, so sealed members keep living values.
#[tokio::test(flavor = "current_thread")]
async fn knife84_sealed_ro_accessors() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"(function () {
        const r = {};
        // 1) descriptor shape: RO members carry a getter, no setter (rw members keep both)
        const d = (C, k) => { const g = Object.getOwnPropertyDescriptor(C.prototype, k); return [!!g && !!g.get, !!g && !!g.set]; };
        r.sbUpdating = d(SourceBuffer, 'updating');
        r.sbMode = d(SourceBuffer, 'mode');
        r.decEncoding = d(TextDecoder, 'encoding');
        r.attrName = d(Attr, 'name');
        r.blobSize = d(Blob, 'size');
        r.fileLast = d(File, 'lastModified');
        // 2) sloppy-mode page writes are silent no-ops, value unchanged
        const td = new TextDecoder();
        td.encoding = 'windows-1252';
        r.sloppyEncoding = td.encoding;
        const b = new Blob(['abc'], { type: 'text/x-k' });
        b.size = 99;
        b.type = 'text/other';
        r.sloppyBlob = [b.size, b.type];
        // strict mode still throws
        let threw = '';
        try { (function () { 'use strict'; b.size = 1; })(); } catch (e) { threw = e.name; }
        r.strictThrows = threw;
        // 3) engine state changes land through the slot (sealed members stay live)
        const x = new XMLHttpRequest();
        x.open('GET', 'data:text/plain,hi', false);
        x.send();
        const a = document.createAttributeNS('urn:x', 'p:k');
        r.live = [x.readyState, x.status, x.responseText, a.name, a.localName, a.prefix];
        return JSON.stringify(r);
    })()"#,
        )
        .unwrap();
    let o: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(o["sbUpdating"], serde_json::json!([true, false]));
    assert_eq!(o["sbMode"], serde_json::json!([true, true]));
    assert_eq!(o["decEncoding"], serde_json::json!([true, false]));
    assert_eq!(o["attrName"], serde_json::json!([true, false]));
    assert_eq!(o["blobSize"], serde_json::json!([true, false]));
    assert_eq!(o["fileLast"], serde_json::json!([true, false]));
    assert_eq!(o["sloppyEncoding"], serde_json::json!("utf-8"));
    assert_eq!(o["sloppyBlob"], serde_json::json!([3, "text/x-k"]));
    assert_eq!(o["strictThrows"], serde_json::json!("TypeError"));
    assert_eq!(
        o["live"],
        serde_json::json!([4, 200, "hi", "p:k", "k", "p"])
    );
}

#[tokio::test]
async fn knife85_readonly_state_faces() {
    // 刀85: the last 24 Chrome-ro faces the engine still served rw are sealed
    // (_evxRo event dicts, __accRo state classes, demoted singletons, the
    // reflected five, NodeList.length, StyleSheet base). Values stay live —
    // engine writes go straight to the hidden slots — while page writes no-op.
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"(function () {
        const r = {};
        const d = (C, k) => { const g = Object.getOwnPropertyDescriptor(C.prototype, k); return [!!g && !!g.get, !!g && !!g.set]; };
        // 1) event dicts: ctor values readable, sloppy writes no-op
        r.evxDesc = [d(ErrorEvent, 'message'), d(MessageEvent, 'data'), d(ToggleEvent, 'oldState')];
        const ee = new ErrorEvent('e', { message: 'm', filename: 'f', lineno: 3, colno: 7 });
        ee.message = 'x'; ee.lineno = 9;
        r.errEvt = [ee.message, ee.filename, ee.lineno, ee.colno];
        const me = new MessageEvent('m', { data: 42, origin: 'urn:o' });
        me.data = 'z'; r.msgEvt = [me.data, me.origin];
        const te = new ToggleEvent('toggle', { oldState: 'closed', newState: 'open' });
        te.newState = 'zzz'; r.tglEvt = [te.oldState, te.newState];
        // 2) state classes: ctor defaults + engine transitions stay live
        r.stateDesc = [d(WebSocket, 'url'), d(EventSource, 'readyState'), d(FileReader, 'result'), d(BroadcastChannel, 'name')];
        const bc = new BroadcastChannel('k85');
        bc.name = 'other'; r.bcName = bc.name;
        const ws = new WebSocket('ws://127.0.0.1:1/x', ['chat']);
        r.wsInit = [ws.url, ws.readyState, ws.bufferedAmount, ws.extensions, ws.protocol];
        ws.url = 'zzz'; ws.readyState = 9;
        ws.close(); r.wsClosed = [ws.url, ws.readyState];
        const es = new EventSource('/feed');
        es.close(); es.readyState = 9; r.esStates = [es.readyState, es.withCredentials];
        const fr = new FileReader();
        r.frInit = [fr.result, fr.error, fr.readyState];
        // 3) demoted singletons: getter-only, values readable
        r.demoDesc = [d(PerformanceTiming, 'navigationStart'), d(Crypto, 'subtle'), d(SpeechSynthesis, 'pending'), d(ServiceWorkerContainer, 'controller')];
        const ns = performance.timing.navigationStart;
        performance.timing.navigationStart = 1; r.pt = [performance.timing.navigationStart === ns, typeof performance.timing.toJSON === 'function'];
        r.swc = [navigator.serviceWorker.controller, typeof crypto.subtle.encrypt];
        speechSynthesis.pending = true; r.ssPending = speechSynthesis.pending;
        // 4) reflected five + NodeList.length + StyleSheet base
        r.reflDesc = [d(HTMLTemplateElement, 'content'), d(HTMLSelectElement, 'options'), d(HTMLDataListElement, 'options'), d(HTMLTableElement, 'rows'), d(HTMLTableSectionElement, 'rows'), d(NodeList, 'length'), d(StyleSheet, 'href')];
        const tpl = document.createElement('template');
        r.tplContent = tpl.content instanceof DocumentFragment;
        const sel = document.createElement('select');
        document.body.appendChild(sel);
        sel.options = 'zzz'; r.selOpts = sel.options instanceof HTMLOptionsCollection;
        const p = document.createElement('p'); document.body.appendChild(p);
        const nl = document.querySelectorAll('p');
        nl.length = 9; r.nlLen = [nl.length, nl.item(0) === p];
        const st = document.createElement('style'); st.textContent = 'p{color:red}'; document.head.appendChild(st);
        const sheet = st.sheet;
        r.sheet = [sheet.type, sheet.ownerNode === st, sheet.href];
        sheet.type = 'zzz'; r.sheetSloppy = sheet.type;
        // strict mode still throws on every family
        let threw = [];
        try { (function () { 'use strict'; ee.message = 'x'; })(); } catch (e) { threw.push(e.name); }
        try { (function () { 'use strict'; ws.url = 'x'; })(); } catch (e) { threw.push(e.name); }
        try { (function () { 'use strict'; nl.length = 1; })(); } catch (e) { threw.push(e.name); }
        try { (function () { 'use strict'; sheet.type = 'x'; })(); } catch (e) { threw.push(e.name); }
        try { (function () { 'use strict'; performance.timing.navigationStart = 1; })(); } catch (e) { threw.push(e.name); }
        r.strictThrows = threw;
        return JSON.stringify(r);
    })()"#,
        )
        .unwrap();
    let o: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let gf = serde_json::json!([true, false]);
    assert_eq!(
        o["evxDesc"],
        serde_json::json!([[true, false], [true, false], [true, false]])
    );
    assert_eq!(o["errEvt"], serde_json::json!(["m", "f", 3, 7]));
    assert_eq!(o["msgEvt"], serde_json::json!([42, "urn:o"]));
    assert_eq!(o["tglEvt"], serde_json::json!(["closed", "open"]));
    assert_eq!(o["stateDesc"], serde_json::json!([gf, gf, gf, gf]));
    assert_eq!(o["bcName"], serde_json::json!("k85"));
    assert_eq!(
        o["wsInit"],
        serde_json::json!(["ws://127.0.0.1:1/x", 0, 0, "", "chat"])
    );
    assert_eq!(o["wsClosed"], serde_json::json!(["ws://127.0.0.1:1/x", 3]));
    assert_eq!(o["esStates"], serde_json::json!([2, false]));
    assert_eq!(o["frInit"], serde_json::json!([null, null, 0]));
    assert_eq!(o["demoDesc"], serde_json::json!([gf, gf, gf, gf]));
    assert_eq!(o["pt"], serde_json::json!([true, true]));
    assert_eq!(o["swc"], serde_json::json!([null, "function"]));
    assert_eq!(o["ssPending"], serde_json::json!(false));
    assert_eq!(
        o["reflDesc"],
        serde_json::json!([gf, gf, gf, gf, gf, gf, gf])
    );
    assert_eq!(o["tplContent"], serde_json::json!(true));
    assert_eq!(o["selOpts"], serde_json::json!(true));
    assert_eq!(o["nlLen"], serde_json::json!([1, true]));
    assert_eq!(o["sheet"], serde_json::json!(["text/css", true, null]));
    assert_eq!(o["sheetSloppy"], serde_json::json!("text/css"));
    assert_eq!(
        o["strictThrows"],
        serde_json::json!([
            "TypeError",
            "TypeError",
            "TypeError",
            "TypeError",
            "TypeError"
        ])
    );
}

/// 刀86: the cross-connection open protocol. A version-changing `open` (or
/// `deleteDatabase`) against a database with live connections must first fire
/// `versionchange` (an IDBVersionChangeEvent) on each of them; if any is still
/// open afterwards the request fires `blocked` — itself an IDBVersionChangeEvent
/// carrying oldVersion/newVersion, with readyState still "pending" — and waits;
/// the last `close()` releases it into `upgradeneeded`+`success`. A handler
/// that closes inside `versionchange` skips `blocked` entirely; a same-version
/// open runs no protocol; `deleteDatabase` success is an IDBVersionChangeEvent
/// with newVersion null. All pinned against Chrome 145 (k86-idb/k86-idb2).
#[tokio::test(flavor = "current_thread")]
async fn knife86_idb_cross_connection_open_protocol() {
    let mut rt = setup_runtime("<html><body></body></html>");
    for n in ["k86-a", "k86-b", "k86-c", "k86-d"] {
        rt.evaluate(&format!("localStorage.removeItem('__diting_idb__{}')", n))
            .unwrap();
    }
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const wait = () => new Promise(r => setTimeout(r, 0));
        function openDb(n, v, onvc) {
          return new Promise(function (resolve, reject) {
            const q = indexedDB.open(n, v);
            q.onupgradeneeded = function (e) {
              if (!e.target.result.objectStoreNames.contains('kv'))
                e.target.result.createObjectStore('kv', { keyPath: 'k' });
            };
            q.onsuccess = function () {
              if (onvc) q.result.onversionchange = onvc;
              resolve(q.result);
            };
            q.onerror = function () { reject(q.error); };
          });
        }
        globalThis.__p = (async function () {
          // 1) blocker stays open: versionchange, blocked (pending), release on close
          const db1 = await openDb('k86-a', 1, function (e) {
            __log.push(['vc-a', e.oldVersion, e.newVersion, e instanceof IDBVersionChangeEvent]);
          });
          const q2 = indexedDB.open('k86-a', 2);
          q2.onblocked = function (e) {
            __log.push(['blocked-a', e instanceof IDBVersionChangeEvent, e.oldVersion, e.newVersion, q2.readyState]);
          };
          q2.onupgradeneeded = function (e) { __log.push(['upg-a', e.oldVersion, e.newVersion]); };
          q2.onsuccess = function () { __log.push(['ok-a', q2.result.version]); };
          await wait();
          __log.push(['a-still-blocked']);
          db1.close();
          await wait();
          // 2) versionchange handler closes: no blocked at all
          let db3;
          db3 = await openDb('k86-b', 1, function (e) { __log.push(['vc-b', e.oldVersion, e.newVersion]); db3.close(); });
          const q4 = indexedDB.open('k86-b', 3);
          q4.onblocked = function () { __log.push(['blocked-b']); };
          q4.onupgradeneeded = function () { __log.push(['upg-b']); };
          q4.onsuccess = function () { __log.push(['ok-b', q4.result.version]); };
          await wait();
          // 3) deleteDatabase against a live connection
          const db5 = await openDb('k86-c', 1, function (e) { __log.push(['vc-c', e.oldVersion, e.newVersion]); });
          const q6 = indexedDB.deleteDatabase('k86-c');
          q6.onblocked = function () { __log.push(['blocked-c']); };
          q6.onsuccess = function (e) {
            __log.push(['del-c', e.oldVersion, e.newVersion, e instanceof IDBVersionChangeEvent]);
          };
          await wait();
          db5.close();
          await wait();
          // 4) same-version open with a live connection: silent success
          const db7 = await openDb('k86-d', 2);
          const q8 = indexedDB.open('k86-d', 2);
          q8.onblocked = function () { __log.push(['blocked-d']); };
          q8.onupgradeneeded = function () { __log.push(['upg-d']); };
          q8.onsuccess = function () { __log.push(['ok-d', q8.result.version]); };
          await wait();
          db7.close();
        })().then(function () { __log.push(['done']); },
                  function (e) { __log.push(['ERR', String(e)]); });
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(400).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            ["vc-a", 1, 2, true],
            ["blocked-a", true, 1, 2, "pending"],
            ["a-still-blocked"],
            ["upg-a", 1, 2],
            ["ok-a", 2],
            ["vc-b", 1, 3],
            ["upg-b"],
            ["ok-b", 3],
            ["vc-c", 1, null],
            ["blocked-c"],
            ["del-c", 1, null, true],
            ["ok-d", 2],
            ["done"],
        ]),
        "versionchange→blocked→release on close; close-in-handler skips blocked; \
         deleteDatabase carries null newVersion; same-version open is silent"
    );
}

/// 刀87 — constructor parameter-validation ladders (Chrome 145 truth:
/// k87-ctor-e / k87-ctor-f / k87-fp probes). Chrome reports argc, interface
/// type, dictionary member read ORDER, sequence-vs-double conversion and
/// range violations as distinct messages — CSSMath* (variadic empty =
/// SyntaxError, N-present TypeErrors, CSSNumberish double coercion),
/// IIRFilterNode (feedback read before feedforward, fb[0]=0 / all-zero-ff
/// InvalidStateErrors), BiquadFilterNode, OfflineAudioContext (0-arg /
/// 2-arg overload / required members / [1,32]+[3000,768000] ranges),
/// ReadableStreamDefaultReader, RTCEncodedAudioFrame, URL (base validated
/// first with its own message), WebSocket (http→ws conversion, 4-scheme
/// message, subprotocol token check) and StaticRange (endContainer read
/// BEFORE startContainer, DocumentType/Attr endpoints).
#[tokio::test(flavor = "current_thread")]
async fn knife87_ctor_validation_ladders() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
        globalThis.__log = [];
        const T = (n, f) => { try { const v = f(); __log.push([n, 'ok', (v && typeof v === 'object') ? (v.constructor && v.constructor.name) : String(v)]); }
                              catch (e) { __log.push([n, e.name, e.message]); } };
        // CSSMath*: variadic empty = SyntaxError; fixed arity = N-present;
        // non-finite coercion; success mints the instance.
        T('max0', () => new CSSMathMax());
        T('inv0', () => new CSSMathInvert());
        T('invbad', () => new CSSMathInvert('x'));
        T('clamp2', () => new CSSMathClamp(1, 2));
        T('sumok', () => new CSSMathSum(1, 2));
        T('lens', () => [CSSMathClamp.length, CSSMathInvert.length, CSSMathMax.length].join(','));
        // IIRFilterNode: argc → ctx → dict (feedback BEFORE feedforward) →
        // sequence/non-finite → fb[0]=0 / all-zero ff.
        const ac = new OfflineAudioContext(1, 128, 8000);
        T('iir0', () => new IIRFilterNode(ac));
        T('iirseq1', () => new IIRFilterNode(ac, [1]));
        T('iirfbstr', () => new IIRFilterNode(ac, { feedforward: [1], feedback: 'x' }));
        T('iirfb0', () => new IIRFilterNode(ac, { feedforward: [1], feedback: [0] }));
        T('iirff0', () => new IIRFilterNode(ac, { feedforward: [0], feedback: [1] }));
        T('iirok', () => new IIRFilterNode(ac, { feedforward: [1], feedback: [1, 0.5] }));
        T('bq0', () => new BiquadFilterNode());
        T('bqctx', () => new BiquadFilterNode({}));
        T('bqok', () => new BiquadFilterNode(ac));
        // OfflineAudioContext: argc / overload / required members / ranges.
        T('oac0', () => new OfflineAudioContext());
        T('oac2', () => new OfflineAudioContext(1, 128));
        T('oacch33', () => new OfflineAudioContext(33, 128, 8000));
        T('oacpart', () => new OfflineAudioContext({ length: 128 }));
        // Reader / frame / URL / WebSocket / StaticRange ladders.
        T('rsr0', () => new ReadableStreamDefaultReader());
        T('rsrbad', () => new ReadableStreamDefaultReader({}));
        T('reaf0', () => new RTCEncodedAudioFrame());
        T('url0', () => new URL());
        T('urlbad', () => new URL('notaurl'));
        T('urlbase', () => new URL('a', 'b'));
        T('urlok', () => new URL('a', 'http://x.example/'));
        T('ws0', () => new WebSocket());
        T('wsftp', () => new WebSocket('ftp://h/x'));
        T('wsdup', () => new WebSocket('ws://127.0.0.1:1/x', ['a', 'a']));
        T('wshttp', () => { const w = new WebSocket('http://127.0.0.1:1/x'); const u = w.url; w.close(); return u; });
        T('sr0', () => new StaticRange());
        T('srempty', () => new StaticRange({}));
        T('srsc', () => new StaticRange({ startContainer: 'x', endContainer: document, startOffset: 0, endOffset: 0 }));
        T('srdoct', () => { const dt = document.implementation.createDocumentType('html', '', ''); return new StaticRange({ startContainer: dt, endContainer: dt, startOffset: 0, endOffset: 0 }); });
        T('srok', () => new StaticRange({ startContainer: document.body, endContainer: document.body, startOffset: 0, endOffset: 0 }));
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(100).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            [
                "max0",
                "SyntaxError",
                "Failed to construct 'CSSMathMax': Arguments can't be empty"
            ],
            [
                "inv0",
                "TypeError",
                "Failed to construct 'CSSMathInvert': 1 argument required, but only 0 present."
            ],
            [
                "invbad",
                "TypeError",
                "Failed to construct 'CSSMathInvert': The provided double value is non-finite."
            ],
            [
                "clamp2",
                "TypeError",
                "Failed to construct 'CSSMathClamp': 3 arguments required, but only 2 present."
            ],
            ["sumok", "ok", "CSSMathSum"],
            ["lens", "ok", "3,1,0"],
            [
                "iir0",
                "TypeError",
                "Failed to construct 'IIRFilterNode': 2 arguments required, but only 1 present."
            ],
            [
                "iirseq1",
                "TypeError",
                "Failed to construct 'IIRFilterNode': Failed to read the 'feedback' property from 'IIRFilterOptions': Required member is undefined."
            ],
            [
                "iirfbstr",
                "TypeError",
                "Failed to construct 'IIRFilterNode': Failed to read the 'feedback' property from 'IIRFilterOptions': The provided value cannot be converted to a sequence."
            ],
            [
                "iirfb0",
                "InvalidStateError",
                "Failed to construct 'IIRFilterNode': First feedback coefficient cannot be zero."
            ],
            [
                "iirff0",
                "InvalidStateError",
                "Failed to construct 'IIRFilterNode': At least one feedforward coefficient must be non-zero."
            ],
            ["iirok", "ok", "IIRFilterNode"],
            [
                "bq0",
                "TypeError",
                "Failed to construct 'BiquadFilterNode': 1 argument required, but only 0 present."
            ],
            [
                "bqctx",
                "TypeError",
                "Failed to construct 'BiquadFilterNode': parameter 1 is not of type 'BaseAudioContext'."
            ],
            ["bqok", "ok", "BiquadFilterNode"],
            [
                "oac0",
                "TypeError",
                "Failed to construct 'OfflineAudioContext': 1 argument required, but only 0 present."
            ],
            [
                "oac2",
                "TypeError",
                "Failed to construct 'OfflineAudioContext': Overload resolution failed."
            ],
            [
                "oacch33",
                "NotSupportedError",
                "Failed to construct 'OfflineAudioContext': The number of channels provided (33) is outside the range [1, 32]."
            ],
            [
                "oacpart",
                "TypeError",
                "Failed to construct 'OfflineAudioContext': Failed to read the 'sampleRate' property from 'OfflineAudioContextOptions': Required member is undefined."
            ],
            [
                "rsr0",
                "TypeError",
                "Failed to construct 'ReadableStreamDefaultReader': 1 argument required, but only 0 present."
            ],
            [
                "rsrbad",
                "TypeError",
                "Failed to construct 'ReadableStreamDefaultReader': parameter 1 is not of type 'ReadableStream'."
            ],
            [
                "reaf0",
                "TypeError",
                "Failed to construct 'RTCEncodedAudioFrame': 1 argument required, but only 0 present."
            ],
            [
                "url0",
                "TypeError",
                "Failed to construct 'URL': 1 argument required, but only 0 present."
            ],
            [
                "urlbad",
                "TypeError",
                "Failed to construct 'URL': Invalid URL"
            ],
            [
                "urlbase",
                "TypeError",
                "Failed to construct 'URL': Invalid base URL"
            ],
            ["urlok", "ok", "URL"],
            [
                "ws0",
                "TypeError",
                "Failed to construct 'WebSocket': 1 argument required, but only 0 present."
            ],
            [
                "wsftp",
                "SyntaxError",
                "Failed to construct 'WebSocket': The URL's scheme must be either 'http', 'https', 'ws', or 'wss'. 'ftp' is not allowed."
            ],
            [
                "wsdup",
                "SyntaxError",
                "Failed to construct 'WebSocket': The subprotocol 'a' is duplicated."
            ],
            ["wshttp", "ok", "ws://127.0.0.1:1/x"],
            [
                "sr0",
                "TypeError",
                "Failed to construct 'StaticRange': 1 argument required, but only 0 present."
            ],
            [
                "srempty",
                "TypeError",
                "Failed to construct 'StaticRange': Failed to read the 'endContainer' property from 'StaticRangeInit': Required member is undefined."
            ],
            [
                "srsc",
                "TypeError",
                "Failed to construct 'StaticRange': Failed to read the 'startContainer' property from 'StaticRangeInit': Failed to convert value to 'Node'."
            ],
            [
                "srdoct",
                "InvalidNodeTypeError",
                "Failed to construct 'StaticRange': Neither startContainer nor endContainer can be a DocumentType or Attribute node."
            ],
            ["srok", "ok", "StaticRange"]
        ]),
        "ctor ladders: argc/ctx/dict-read-order/sequence/range messages, \
         http→ws conversion, endContainer-before-startContainer"
    );
}

// SVG tags 真值（knife77 用）
const TAGS: &str = r#"{"a":"SVGAElement>SVGGraphicsElement","animate":"SVGAnimateElement>SVGAnimationElement","animateMotion":"SVGAnimateMotionElement>SVGAnimationElement","animateTransform":"SVGAnimateTransformElement>SVGAnimationElement","circle":"SVGCircleElement>SVGGeometryElement","clipPath":"SVGClipPathElement>SVGElement","defs":"SVGDefsElement>SVGGraphicsElement","desc":"SVGDescElement>SVGElement","ellipse":"SVGEllipseElement>SVGGeometryElement","feBlend":"SVGFEBlendElement>SVGElement","feColorMatrix":"SVGFEColorMatrixElement>SVGElement","feComponentTransfer":"SVGFEComponentTransferElement>SVGElement","feComposite":"SVGFECompositeElement>SVGElement","feConvolveMatrix":"SVGFEConvolveMatrixElement>SVGElement","feDiffuseLighting":"SVGFEDiffuseLightingElement>SVGElement","feDisplacementMap":"SVGFEDisplacementMapElement>SVGElement","feDistantLight":"SVGFEDistantLightElement>SVGElement","feDropShadow":"SVGFEDropShadowElement>SVGElement","feFlood":"SVGFEFloodElement>SVGElement","feFuncA":"SVGFEFuncAElement>SVGComponentTransferFunctionElement","feFuncB":"SVGFEFuncBElement>SVGComponentTransferFunctionElement","feFuncG":"SVGFEFuncGElement>SVGComponentTransferFunctionElement","feFuncR":"SVGFEFuncRElement>SVGComponentTransferFunctionElement","feGaussianBlur":"SVGFEGaussianBlurElement>SVGElement","feImage":"SVGFEImageElement>SVGElement","feMerge":"SVGFEMergeElement>SVGElement","feMergeNode":"SVGFEMergeNodeElement>SVGElement","feMorphology":"SVGFEMorphologyElement>SVGElement","feOffset":"SVGFEOffsetElement>SVGElement","fePointLight":"SVGFEPointLightElement>SVGElement","feSpecularLighting":"SVGFESpecularLightingElement>SVGElement","feSpotLight":"SVGFESpotLightElement>SVGElement","feTile":"SVGFETileElement>SVGElement","feTurbulence":"SVGFETurbulenceElement>SVGElement","filter":"SVGFilterElement>SVGElement","foreignObject":"SVGForeignObjectElement>SVGGraphicsElement","g":"SVGGElement>SVGGraphicsElement","image":"SVGImageElement>SVGGraphicsElement","line":"SVGLineElement>SVGGeometryElement","linearGradient":"SVGLinearGradientElement>SVGGradientElement","marker":"SVGMarkerElement>SVGElement","mask":"SVGMaskElement>SVGElement","metadata":"SVGMetadataElement>SVGElement","mpath":"SVGMPathElement>SVGElement","path":"SVGPathElement>SVGGeometryElement","pattern":"SVGPatternElement>SVGElement","polygon":"SVGPolygonElement>SVGGeometryElement","polyline":"SVGPolylineElement>SVGGeometryElement","radialGradient":"SVGRadialGradientElement>SVGGradientElement","rect":"SVGRectElement>SVGGeometryElement","script":"SVGScriptElement>SVGElement","set":"SVGSetElement>SVGAnimationElement","stop":"SVGStopElement>SVGElement","style":"SVGStyleElement>SVGElement","svg":"SVGSVGElement>SVGGraphicsElement","switch":"SVGSwitchElement>SVGGraphicsElement","symbol":"SVGSymbolElement>SVGGraphicsElement","text":"SVGTextElement>SVGTextPositioningElement","textPath":"SVGTextPathElement>SVGTextContentElement","title":"SVGTitleElement>SVGElement","tspan":"SVGTSpanElement>SVGTextPositioningElement","use":"SVGUseElement>SVGGraphicsElement","view":"SVGViewElement>SVGElement"}"#;
