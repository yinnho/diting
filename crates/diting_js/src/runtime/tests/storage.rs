//! IndexedDB + localStorage/sessionStorage + storage 事件
use super::common::{setup_runtime, STORAGE_ENV_LOCK};

#[test]
fn test_storage_event_constructor_and_legacy_factory() {
    // Upstream 776c915: StorageEvent global + legacy createEvent/initStorageEvent path.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            const event = new StorageEvent('storage', {
                key: 'theme', oldValue: 'light', newValue: 'dark', url: 'https://example.test/'
            });
            const legacy = document.createEvent('StorageEvent');
            legacy.initStorageEvent('storage', false, false, 'count', '1', '2', 'https://example.test/', null);
            return [
                event instanceof Event,
                event.key, event.oldValue, event.newValue, event.url,
                legacy instanceof StorageEvent, legacy.key, legacy.newValue
            ];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true,
            "theme",
            "light",
            "dark",
            "https://example.test/",
            true,
            "count",
            "2"
        ])
    );
}

#[allow(clippy::await_holding_lock)] // the env guard must span the awaits
#[tokio::test(flavor = "current_thread")]
async fn local_storage_persists_across_realms_and_session_storage_does_not() {
    // Ephemeral flips storage_file to None, killing every flush below —
    // hold its lock so the ephemeral tests can't race this one.
    let _ephemeral = diting_env::env_knobs::EPHEMERAL_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _env = STORAGE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("diting-ls-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("DITING_STORAGE_DIR", &dir);
    crate::ops::reset_local_storage_for_tests();

    // Realm 1: the method path and the property path both flush, and
    // sessionStorage stays out of the picture.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate_remote(
                "(async () => { localStorage.setItem('token', 'abc'); localStorage.theme = 'dark'; \
                 sessionStorage.setItem('s', '1'); await new Promise(r => setTimeout(r, 250)); return 'ok'; })()",
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("ok"));

    let ls_dir = dir.join("localStorage");
    let files: Vec<_> = std::fs::read_dir(&ls_dir)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1, "one file per origin");
    let on_disk = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(
        on_disk.contains("\"token\":\"abc\""),
        "disk copy: {on_disk}"
    );
    assert!(
        !on_disk.contains("\"s\""),
        "sessionStorage never reaches disk: {on_disk}"
    );

    // Realm 2 = restart equivalent: drop the in-memory mirror and
    // rebuild; the fresh store reads the same values back from disk.
    crate::ops::reset_local_storage_for_tests();
    drop(rt);
    let mut rt2 = setup_runtime("<html><body></body></html>");
    assert_eq!(
        rt2.evaluate("localStorage.getItem('token')").unwrap(),
        serde_json::json!("abc")
    );
    assert_eq!(
        rt2.evaluate("localStorage.theme").unwrap(),
        serde_json::json!("dark")
    );
    assert_eq!(
        rt2.evaluate("sessionStorage.getItem('s')").unwrap(),
        serde_json::json!(null)
    );

    // The deletion flushes too, not just additions.
    let result = rt2
            .evaluate_remote(
                "(async () => { localStorage.removeItem('token'); await new Promise(r => setTimeout(r, 250)); return 'ok'; })()",
                true,
                true,
            )
            .await
            .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("ok"));
    let on_disk = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(
        !on_disk.contains("token"),
        "removal reached disk: {on_disk}"
    );

    std::env::remove_var("DITING_STORAGE_DIR");
    crate::ops::reset_local_storage_for_tests();
    let _ = std::fs::remove_dir_all(&dir);
}

/// #53: `indexedDB.open` used to resolve success immediately and never
/// dispatch `onupgradeneeded`, so schema-installing openers (Dexie,
/// localForage, Firebase) never settled their open promise and the app
/// dead-ended at its skeleton. The open contract: fresh DB at a higher
/// version dispatches upgradeneeded (with a versionchange tx whose
/// createObjectStore lands on the database), then success carrying the new
/// version and visible store names; a re-open at the same version skips the
/// upgrade entirely.
#[tokio::test(flavor = "current_thread")]
async fn idb_open_dispatches_upgrade_then_success_and_skips_on_reopen() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // Disk-backed localStorage outlives the process; a meta row a previous
    // run persisted would make open() skip the upgrade path under test.
    rt.evaluate("localStorage.removeItem('__diting_idb__probe-db')")
        .unwrap();
    rt.evaluate(r#"
        globalThis.__log = [];
        function openDb(v) {
          return new Promise(function (resolve, reject) {
            const req = indexedDB.open('probe-db', v);
            req.onupgradeneeded = function (e) {
              __log.push(['upgrade', e.oldVersion, e.newVersion, e.target.transaction && e.target.transaction.mode]);
              if (!e.target.result.objectStoreNames.contains('drafts')) {
                e.target.result.createObjectStore('drafts', { keyPath: 'id' });
                e.target.result.createObjectStore('markers');
              }
            };
            req.onsuccess = function (e) {
              const db = e.target.result;
              __log.push(['success', db.version, db.objectStoreNames.contains('drafts'),
                          db.objectStoreNames.length]);
              resolve(db);
            };
            req.onerror = function () { reject(req.error); };
          });
        }
        globalThis.__p = openDb(2).then(function (db) {
          db.close();
          return openDb(2); // same version: must NOT upgrade again
        });
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            ["upgrade", 0, 2, "versionchange"],
            ["success", 2, true, 2],
            ["success", 2, true, 2], // reopen: no upgrade row between
        ]),
        "fresh open upgrades then succeeds; re-open at the same version goes straight to success"
    );
}

/// #203: `close()` used to set a closed flag on the shared per-name record,
/// so one connection's close poisoned every other connection — 抖店's
/// @ecom/browser-tcc opens its DB from several modules and died with
/// `Uncaught (in promise) InvalidStateError: database is closed` ×2 per
/// navigation. Chrome semantics: closed is per-connection; siblings keep
/// transacting, the closed one throws.
#[tokio::test(flavor = "current_thread")]
async fn idb_close_is_per_connection_not_per_database() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__close-db')")
        .unwrap();
    rt.evaluate(
        r#"
        function openDb() {
          return new Promise(function (resolve, reject) {
            const req = indexedDB.open('close-db', 1);
            req.onupgradeneeded = function (e) {
              e.target.result.createObjectStore('kv', { keyPath: 'k' });
            };
            req.onsuccess = function (e) { resolve(e.target.result); };
            req.onerror = function () { reject(req.error); };
          });
        }
        globalThis.__log = null;
        globalThis.__p = openDb().then(function (a) {
          return openDb().then(function (b) {
            a.close();
            const out = [];
            try {
              b.transaction('kv', 'readwrite').objectStore('kv').put({ k: 'x', v: 42 });
              out.push('b-tx-ok');
            } catch (e) { out.push('b-tx-throw:' + e.name); }
            try {
              a.transaction('kv');
              out.push('a-tx-ok');
            } catch (e) { out.push('a-tx-throw:' + e.name + ':' + e.message); }
            globalThis.__log = out;
          });
        });
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            "b-tx-ok",
            "a-tx-throw:InvalidStateError:Failed to execute 'transaction' on 'IDBDatabase': The database connection is closing."
        ]),
        "closing one connection must leave sibling connections transacting; only the closed one throws"
    );
}

/// #54: the open request's `.transaction` stayed null through the upgrade
/// window — the event object carried the versionchange tx, but IDB libraries
/// (Dexie) read `e.target.transaction`, then dereferenced it inside their
/// upgrade callback ("Cannot set properties of null"). Chromium semantics:
/// the open request exposes the versionchange tx during upgradeneeded and
/// detaches it once the request settles.
#[tokio::test(flavor = "current_thread")]
async fn idb_open_request_exposes_versionchange_tx_during_upgrade() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__reqtx-db')")
        .unwrap();
    rt.evaluate(
        r#"
        globalThis.__log = [];
        globalThis.__p = new Promise(function (resolve) {
          const req = indexedDB.open('reqtx-db', 1);
          req.onupgradeneeded = function (e) {
            const tx = e.target.transaction;
            __log.push(['upgrade-tx', !!tx, tx && tx.mode]);
            e.target.result.createObjectStore('items', { keyPath: 'id' });
            // Dexie's exact shape: reach the fresh store THROUGH the tx.
            const st = e.target.transaction.objectStore('items');
            __log.push(['tx-store', st && st.name]);
          };
          req.onsuccess = function (e) {
            __log.push(['success-tx', e.target.transaction === null]);
            resolve(1);
          };
        });
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let log = rt.evaluate("globalThis.__log").unwrap();
    assert_eq!(
        log,
        serde_json::json!([
            ["upgrade-tx", true, "versionchange"],
            ["tx-store", "items"],
            ["success-tx", true],
        ]),
        "request.transaction is the versionchange tx during upgrade, null after settle"
    );
}

/// #53 second face: stores used to live on the transaction, so a put in one
/// transaction and a getAll in a second never agreed (count=1 vs getAll=0).
/// Store data belongs to the database record; transactions are just windows
/// onto it. This is exactly the Dexie shape the xhs creator page runs.
#[tokio::test(flavor = "current_thread")]
async fn idb_store_data_survives_across_transactions() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // localStorage outlives the process (disk-backed), so a meta row a
    // previous run left behind would make open() skip the upgrade path.
    rt.evaluate("localStorage.removeItem('__diting_idb__tx-db')")
        .unwrap();
    // Phase 1: open + upgrade + put in a readwrite tx, resolve when written.
    rt.evaluate(
        r#"
        globalThis.__phase1 = new Promise(function (resolve) {
          const req = indexedDB.open('tx-db', 1);
          req.onupgradeneeded = function (e) {
            e.target.result.createObjectStore('chapters', { keyPath: 'cid' });
          };
          req.onsuccess = function (e) {
            const db = e.target.result;
            const wtx = db.transaction('chapters', 'readwrite');
            wtx.objectStore('chapters').put({ cid: 'c1', text: 'hello' });
            wtx.oncomplete = function () { resolve('written'); };
          };
        });
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    // Phase 2: a *fresh* transaction on the same db must see the row.
    rt.evaluate(r#"
        globalThis.__out = null;
        const req = indexedDB.open('tx-db', 1);
        req.onsuccess = function (e) {
          const db = e.target.result;
          const rtx = db.transaction('chapters', 'readonly');
          const st = rtx.objectStore('chapters');
          const c = st.count();
          const g = st.getAll();
          const k = st.get('c1');
          rtx.oncomplete = function () {
            globalThis.__out = JSON.stringify([c.result, g.result.length, g.result[0] && g.result[0].text, k.result && k.result.cid]);
          };
        };
    "#).unwrap();
    rt.run_event_loop_bounded(300).await.unwrap();
    let out = rt.evaluate("globalThis.__out").unwrap();
    assert_eq!(
        out,
        serde_json::json!("[1,1,\"hello\",\"c1\"]"),
        "put in tx1 must be visible to count/getAll/get in a second tx"
    );
}

/// #53 third face: version errors and duplicate-add semantics. A lower
/// version must reject with VersionError; `add` of an existing key must
/// reject with ConstraintError while `put` overwrites; a value missing its
/// keyPath must reject with DataError — apps branch on these names.
#[tokio::test(flavor = "current_thread")]
async fn idb_error_contracts_version_constraint_and_keypath() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__err-db')")
        .unwrap();
    rt.evaluate(
        r#"
        globalThis.__out = null;
        globalThis.__phase1 = new Promise(function (resolve) {
          const req = indexedDB.open('err-db', 3);
          req.onupgradeneeded = function (e) {
            e.target.result.createObjectStore('kv', { keyPath: 'k' });
          };
          req.onsuccess = function (e) { resolve(e.target.result); };
        });
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    rt.evaluate(r#"
        globalThis.__phase1.then(function (db) {
          const out = { version: null, downgrade: null, dupAdd: null, missingKeyPath: null, putOverwrite: null };
          const lower = indexedDB.open('err-db', 2);
          lower.onerror = function () { out.downgrade = lower.error && lower.error.name; };
          lower.onupgradeneeded = function () { out.downgrade = 'UPGRADED (wrong)'; };
          lower.onsuccess = function () { out.downgrade = 'SUCCESS (wrong)'; };
          const wtx = db.transaction('kv', 'readwrite');
          const st = wtx.objectStore('kv');
          const p1 = st.put({ k: 'a', v: 1 });
          const dup = st.add({ k: 'a', v: 2 });
          dup.onerror = function (e) { out.dupAdd = dup.error && dup.error.name; e.preventDefault && e.preventDefault(); };
          // Chrome rejects a value missing its in-line key synchronously
          try { st.put({ nope: 'x' }); out.missingKeyPath = 'NO-THROW (wrong)'; } catch (e) { out.missingKeyPath = e.name; }
          wtx.oncomplete = function () {
            const rtx = db.transaction('kv', 'readonly');
            const g = rtx.objectStore('kv').getAll();
            g.onsuccess = function () {
              out.putOverwrite = g.result.length === 1 && g.result[0].v === 1;
              out.version = db.version;
              globalThis.__out = JSON.stringify(out);
            };
          };
        });
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let out = rt.evaluate("globalThis.__out").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(parsed["version"], serde_json::json!(3));
    assert_eq!(parsed["downgrade"], serde_json::json!("VersionError"));
    assert_eq!(parsed["dupAdd"], serde_json::json!("ConstraintError"));
    assert_eq!(parsed["missingKeyPath"], serde_json::json!("DataError"));
    assert_eq!(parsed["putOverwrite"], serde_json::json!(true));
}

/// #209: driver pages reference the standard IDB interface names directly
/// (抖店's `IDBRequest is not defined` loop) and probe them with instanceof.
/// Every interface is a global constructor; factory objects inherit its
/// prototype; direct construction throws the same TypeError Chrome throws.
#[tokio::test(flavor = "current_thread")]
async fn idb_interfaces_are_exposed_with_prototypes() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__iface-db')")
        .unwrap();
    rt.evaluate(r#"
        (function () {
          const names = ['IDBRequest','IDBOpenDBRequest','IDBDatabase','IDBTransaction',
                         'IDBObjectStore','IDBIndex','IDBCursor','IDBCursorWithValue',
                         'IDBKeyRange','IDBFactory'];
          const out = { types: {}, illegal: {}, inst: {} };
          for (const n of names) out.types[n] = typeof globalThis[n];
          try { new IDBRequest(); out.illegal.request = 'no-throw'; }
          catch (e) { out.illegal.request = e.name; }
          out.inst.factory = (indexedDB instanceof IDBFactory);
          out.inst.openReqProto = false;
          const r = indexedDB.open('iface-db', 1);
          out.inst.openReqProto = r instanceof IDBOpenDBRequest;
          r.onupgradeneeded = function (e) {
            const db = e.target.result;
            out.inst.db = (db instanceof IDBDatabase);
            out.inst.tx = (e.target.transaction instanceof IDBTransaction);
            const st = db.createObjectStore('s', { keyPath: 'id' });
            out.inst.storeFromCreate = (st instanceof IDBObjectStore);
            out.inst.storeFromTx = (e.target.transaction.objectStore('s') instanceof IDBObjectStore);
            out.inst.index = (st.createIndex('byN', 'n') instanceof IDBIndex);
            out.inst.keyRange = (IDBKeyRange.only(3) instanceof IDBKeyRange);
            out.inst.tag = Object.prototype.toString.call(r);
            out.inst.reqBase = (r instanceof IDBRequest);
          };
          r.onsuccess = function () { globalThis.__out = JSON.stringify(out); };
        })();
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let out = rt.evaluate("globalThis.__out").unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    for n in [
        "IDBRequest",
        "IDBOpenDBRequest",
        "IDBDatabase",
        "IDBTransaction",
        "IDBObjectStore",
        "IDBIndex",
        "IDBCursor",
        "IDBCursorWithValue",
        "IDBKeyRange",
        "IDBFactory",
    ] {
        assert_eq!(
            v["types"][n],
            serde_json::json!("function"),
            "{n} must be a constructor"
        );
    }
    assert_eq!(v["illegal"]["request"], serde_json::json!("TypeError"));
    assert_eq!(v["inst"]["factory"], serde_json::json!(true));
    assert_eq!(v["inst"]["openReqProto"], serde_json::json!(true));
    assert_eq!(
        v["inst"]["reqBase"],
        serde_json::json!(true),
        "open request is also an IDBRequest"
    );
    assert_eq!(v["inst"]["db"], serde_json::json!(true));
    assert_eq!(v["inst"]["tx"], serde_json::json!(true));
    assert_eq!(v["inst"]["storeFromCreate"], serde_json::json!(true));
    assert_eq!(v["inst"]["storeFromTx"], serde_json::json!(true));
    assert_eq!(v["inst"]["index"], serde_json::json!(true));
    assert_eq!(v["inst"]["keyRange"], serde_json::json!(true));
    assert_eq!(
        v["inst"]["tag"],
        serde_json::json!("[object IDBOpenDBRequest]")
    );
}

/// #203: idb v7 (bundled by 抖店's @ecom/browser-tcc) synthesizes its
/// `(await openDB()).put(store, value, key)` conveniences only when the
/// operation names are members of the interface *prototypes*. #209 stopped
/// at constructors + instance prototypes; the ops must now sit on the
/// interface prototypes as writable data props that forward to the
/// receiver's own implementation.
#[tokio::test(flavor = "current_thread")]
async fn idb_proto_ops_forward_and_gate_idb() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__proto-ops')")
        .unwrap();
    rt.evaluate(
        r#"
        (function () {
          const out = { gates: {}, desc: {}, fwd: {}, illegal: {} };
          out.gates.put = 'put' in IDBObjectStore.prototype;
          out.gates.getFromIndex = 'getKey' in IDBIndex.prototype;
          out.gates.tx = 'transaction' in IDBDatabase.prototype;
          out.gates.objectStore = 'objectStore' in IDBTransaction.prototype;
          out.gates.factoryOpen = 'open' in IDBFactory.prototype;
          const d = Object.getOwnPropertyDescriptor(IDBObjectStore.prototype, 'put');
          out.desc = { w: d.writable, e: d.enumerable, c: d.configurable, f: typeof d.value };
          out.gates.cursor = 'continue' in IDBCursor.prototype;
          try { IDBObjectStore.prototype.put.call({}, 1); out.illegal.bare = 'no-throw'; }
          catch (e) { out.illegal.bare = e.name; }
          const r = indexedDB.open('proto-ops', 1);
          r.onupgradeneeded = function (e) {
            e.target.result.createObjectStore('s', { keyPath: 'id' });
          };
          r.onsuccess = function (e) {
            const db = e.target.result;
            const tx = db.transaction('s', 'readwrite');
            const st = tx.objectStore('s');
            // Detached calls through the prototype forward to the instance's
            // own op — the exact shape idb's wrapped stores use.
            IDBObjectStore.prototype.put.call(st, { id: 7, n: 'x' });
            const req = IDBObjectStore.prototype.get.call(st, 7);
            req.onsuccess = function () {
              out.fwd.shape = (req.result instanceof Object && req.result.n === 'x');
              db.close();
              globalThis.__out = JSON.stringify(out);
            };
          };
        })();
    "#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let out = rt.evaluate("globalThis.__out").unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    for k in [
        "put",
        "getFromIndex",
        "tx",
        "objectStore",
        "factoryOpen",
        "cursor",
    ] {
        assert_eq!(
            v["gates"][k],
            serde_json::json!(true),
            "{k} gate must hold for idb's synthesis"
        );
    }
    assert_eq!(
        v["desc"],
        serde_json::json!({"w": true, "e": true, "c": true, "f": "function"})
    );
    assert_eq!(v["illegal"]["bare"], serde_json::json!("TypeError"));
    assert_eq!(
        v["fwd"]["shape"],
        serde_json::json!(true),
        "proto put/get must forward to the store's own ops"
    );
}

/// Cursor iteration guard + Chrome arity (competitor gap sweep 2026-10-28:
/// Moli shipped the same fixes the same day). Between continue()/advance()
/// and the next onsuccess delivery the cursor is "currently being iterated"
/// — a second call must throw InvalidStateError instead of silently
/// double-stepping (which used to eat the rest of the iteration); advance(0)
/// is a TypeError; prototype methods report WebIDL required-arg counts, not
/// the forwarder's 0; open() without a name is a TypeError.
#[tokio::test(flavor = "current_thread")]
async fn idb_cursor_guard_and_arity_follow_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("localStorage.removeItem('__diting_idb__cursor-guard')")
        .unwrap();
    rt.evaluate(r#"
        (function () {
          const out = {};
          out.arity = {
            open: IDBFactory.prototype.open.length,
            put: IDBObjectStore.prototype.put.length,
            createIndex: IDBObjectStore.prototype.createIndex.length,
            advance: IDBCursor.prototype.advance.length,
            cont: IDBCursor.prototype.continue.length,
            cmp: IDBFactory.prototype.cmp.length,
          };
          try { indexedDB.open(); out.openNoArgs = 'accepted'; } catch (e) { out.openNoArgs = e.name; }
          const r = indexedDB.open('cursor-guard', 1);
          r.onupgradeneeded = function (e) { e.target.result.createObjectStore('s'); };
          r.onsuccess = function (e) {
            const db = e.target.result;
            const tx = db.transaction('s', 'readwrite');
            const st = tx.objectStore('s');
            for (let i = 0; i < 3; i++) st.put(i, 'k' + i);
            tx.oncomplete = function () {
              const cur = db.transaction('s', 'readonly').objectStore('s').openCursor();
              const steps = [];
              cur.onsuccess = function (ev) {
                const c = ev.target.result;
                if (!c) { out.steps = steps; globalThis.__out = JSON.stringify(out); return; }
                steps.push(c.key);
                c.continue();
                if (steps.length === 1) {
                  // second continue before the delivery: InvalidStateError
                  try { c.continue(); out.secondContinue = 'no-throw'; } catch (err) { out.secondContinue = err.name; }
                  try { c.advance(1); out.advanceWhileIterating = 'no-throw'; } catch (err) { out.advanceWhileIterating = err.name; }
                }
              };
            };
          };
        })();
    "#).unwrap();
    let _ = rt.run_event_loop_bounded(300).await;
    let out = rt.evaluate("globalThis.__out").unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["arity"],
        serde_json::json!({
            "open": 1, "put": 1, "createIndex": 2, "advance": 1, "cont": 0, "cmp": 2
        }),
        "WebIDL required-arg counts on the prototype forwarders"
    );
    assert_eq!(
        v["openNoArgs"],
        serde_json::json!("TypeError"),
        "open() needs a name"
    );
    assert_eq!(
        v["secondContinue"],
        serde_json::json!("InvalidStateError"),
        "double-continue throws"
    );
    assert_eq!(
        v["advanceWhileIterating"],
        serde_json::json!("InvalidStateError"),
        "advance while iterating throws"
    );
    assert_eq!(
        v["steps"],
        serde_json::json!(["k0", "k1", "k2"]),
        "the rejected double-continue must not eat the remaining iteration"
    );
}

/// storage.js ratchet split (JS-SURFACE-PLAN step 3): the Storage family left
/// bootstrap.js wholesale. Pins the pieces a pure move could silently break —
/// Proxy named-property semantics, load-time native marks (they moved with
/// the family because bootstrap's _markNative list runs before Storage
/// exists), StorageEvent, and the cookie gate's lexical-const form (a
/// function declaration would mint a `__cookieGate` window property).
#[test]
fn storage_family_proxy_semantics_and_gate() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"
        (function () {
            var s = sessionStorage;
            s.clear();
            s.a = '1'; s.setItem('b', 2);
            var ev = new StorageEvent('storage', {key: 'k', newValue: 'v', storageArea: s});
            var init = document.createEvent('storageevent');
            return JSON.stringify({
                len: s.length, a: s.getItem('a'), b: s.b, has: 'a' in s,
                keys: Object.keys(s), key0: s.key(0),
                del: (delete s.a, s.length),
                nat: Storage.prototype.getItem.toString(),
                ctor: Storage.toString(),
                ev: [ev.key, ev.newValue, ev.oldValue, ev.storageArea === s, ev instanceof Event],
                created: init instanceof StorageEvent,
                brand: Object.prototype.toString.call(s),
                gateLeak: Object.getOwnPropertyNames(globalThis).indexOf('__cookieGate'),
                pers: typeof localStorage.getItem,
            });
        })()
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["len"], 2);
    assert_eq!(v["a"], "1");
    assert_eq!(v["b"], "2");
    assert_eq!(v["has"], true);
    assert_eq!(v["keys"], serde_json::json!(["a", "b"]));
    assert_eq!(v["key0"], "a");
    assert_eq!(v["del"], 1);
    assert!(
        v["nat"].as_str().unwrap().contains("[native code]"),
        "marks must ride with the move: {v}"
    );
    assert!(v["ctor"].as_str().unwrap().contains("[native code]"), "{v}");
    assert_eq!(v["ev"], serde_json::json!(["k", "v", null, true, true]));
    assert_eq!(v["created"], true);
    assert_eq!(v["brand"], "[object Storage]");
    assert_eq!(v["gateLeak"], -1, "__cookieGate must stay a lexical const");
    assert_eq!(v["pers"], "function");
}

/// JS-PLATFORM-ARCH M2: Storage/StorageEvent are IDL-generated modules
/// (platform/gen/storage.gen.js + platform/storage.impl.js). This runs the
/// SAME probe that produced chrome/chrome-storage-truth.json in real Chrome
/// and demands the engine's answer match, probe by probe.
///
/// Allowed differences, each with its reason:
///  - keys.order / gopn / forin: Chrome returns its internal hash order for
///    stored items (the spec leaves it implementation-defined); compared as
///    sets, and forin additionally pins the PROTOTYPE member order.
///  - ev.ownkeys / ev.areaBadType: the legacy Event base class keeps its state
///    as own instance props, Chrome as an internal slot - an Event-family
///    gap that is out of this slice (engine ledger, local-only).
#[test]
fn storage_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/storage-probe.js");
    let engine: serde_json::Value =
        serde_json::from_str(rt.evaluate(probe).unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/chrome-storage-truth.json"
    ))
    .unwrap();
    let truth = &truth["result"];
    let as_set = |v: &serde_json::Value| -> std::collections::BTreeSet<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    };
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        let got = &engine[k];
        match k.as_str() {
            "ev.ownkeys" | "ev.areaBadType" => continue,
            "keys.order" | "gopn" => {
                if as_set(got) != as_set(want) {
                    bad.push(format!("{k}: {got} != {want}"));
                }
            }
            "forin" => {
                // items: set-equal; prototype members: exact Chrome order
                let split = |v: &serde_json::Value| {
                    let all: Vec<String> = v
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect();
                    let members = ["length", "clear", "getItem", "key", "removeItem", "setItem"];
                    let items: std::collections::BTreeSet<String> = all
                        .iter()
                        .filter(|x| !members.contains(&x.as_str()))
                        .cloned()
                        .collect();
                    let proto: Vec<String> = all
                        .iter()
                        .filter(|x| members.contains(&x.as_str()))
                        .cloned()
                        .collect();
                    (items, proto)
                };
                if split(got) != split(want) {
                    bad.push(format!("{k}: {got} != {want}"));
                }
            }
            _ => {
                if got != want {
                    bad.push(format!("{k}:\n   engine {got}\n   chrome {want}"));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) differ from Chrome:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
