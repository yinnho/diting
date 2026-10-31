(function () { // @@SCOPE
// IndexedDB (#53 → M4 刀68 rewrite).
// Split out of bootstrap.js wholesale (god-file ratchet, pure move): top level only
// declares bindings; every bootstrap helper it names is touched at call time.
//
// Shape (刀68): every IDB interface is a real class whose instances carry NO own
// state — all of it lives in per-interface WeakMaps — and whose attributes are
// prototype-level accessors with Chrome's pinned `get x`/`set x` names. Requests,
// transactions and databases are real EventTargets (native dispatch core), the
// events they fire are real Event / IDBVersionChangeEvent objects, and request
// errors bubble request → transaction → database through the
// `__diting_etParents` hop table the event hooks consult. The data layer keeps the
// #53 design: page-lifetime in-memory records, with the *schema* (version, stores,
// indexes) mirrored into localStorage so a later navigation re-opens at the
// installed version and skips the upgrade, like a warm real profile.
// Known gaps (GUESS ledger, see SURFACE.md 刀68): no cross-connection
// versionchange/blocked protocol, an upgrade handler that throws does not abort
// the upgrade (the native dispatch swallows it), cursor entry lists are
// snapshotted at open, transactions auto-finish on a microtask boundary.

const _idbMap = () => new WeakMap();
const _Rq = _idbMap(), _Tx = _idbMap(), _Db = _idbMap(), _Os = _idbMap(), _Ix = _idbMap(),
  _Cu = _idbMap(), _Kr = _idbMap(), _Rc = _idbMap(), _Fa = _idbMap();
// Hop table consulted by the event hooks' path walk (request → tx → db).
const _idbParents = new WeakMap();
Object.defineProperty(globalThis, '__diting_etParents', { value: _idbParents, writable: true, enumerable: false, configurable: true });
// Error tap the event hooks' report() leg fills while an upgradeneeded dispatch is open.
Object.defineProperty(globalThis, '__diting_etErrTap', { value: { active: 0, err: null }, writable: true, enumerable: false, configurable: true });

function _idbErr(name, msg) {
  try { return new DOMException(msg || name, name); } catch (e) { const err = new Error(msg || name); err.name = name; return err; }
}
function _idbEx(name, meth, iface, msg) { return _idbErr(name, "Failed to execute '" + meth + "' on '" + iface + "': " + msg); }
function _idbArgc(args, n, meth, iface) {
  if (args.length < n) throw new TypeError("Failed to execute '" + meth + "' on '" + iface + "': " + n + ' argument' + (n > 1 ? 's' : '') + ' required, but only ' + args.length + ' present.');
}
function _idbBrand(map, self) {
  const s = (self !== null && (typeof self === 'object' || typeof self === 'function')) ? map.get(self) : undefined;
  if (!s) throw new TypeError('Illegal invocation');
  return s;
}
function _idbEnum(v, list, typeName, ctx) {
  const s = String(v);
  if (list.indexOf(s) < 0) throw new TypeError((ctx || '') + "The provided value '" + s + "' is not a valid enum value of type " + typeName + '.');
  return s;
}

// ---- keys -----------------------------------------------------------------
function _idbValidKey(k, seen) {
  if (typeof k === 'number') return !Number.isNaN(k);
  if (typeof k === 'string') return true;
  if (k instanceof Date) return !Number.isNaN(+k);
  if (k instanceof ArrayBuffer || ArrayBuffer.isView(k)) return true;
  if (Array.isArray(k)) {
    seen = seen || [];
    if (seen.indexOf(k) >= 0) return false;
    seen.push(k);
    for (let i = 0; i < k.length; i++) { if (!(i in k) || !_idbValidKey(k[i], seen)) { seen.pop(); return false; } }
    seen.pop();
    return true;
  }
  return false;
}
function _idbBytes(k) { return k instanceof ArrayBuffer ? new Uint8Array(k) : new Uint8Array(k.buffer, k.byteOffset, k.byteLength); }
function _idbRank(v) {
  return typeof v === 'number' ? 0 : v instanceof Date ? 1 : typeof v === 'string' ? 2 : Array.isArray(v) ? 4 : (v instanceof ArrayBuffer || ArrayBuffer.isView(v)) ? 3 : 5;
}
function _idbCmp(a, b) {
  const ra = _idbRank(a), rb = _idbRank(b);
  if (ra !== rb) return ra < rb ? -1 : 1;
  if (ra === 2) return a < b ? -1 : a > b ? 1 : 0;
  if (ra === 0 || ra === 1) return +a < +b ? -1 : +a > +b ? 1 : 0;
  if (ra === 3) {
    const x = _idbBytes(a), y = _idbBytes(b), n = Math.min(x.length, y.length);
    for (let i = 0; i < n; i++) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1;
    return x.length < y.length ? -1 : x.length > y.length ? 1 : 0;
  }
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) { const c = _idbCmp(a[i], b[i]); if (c) return c; }
  return a.length < b.length ? -1 : a.length > b.length ? 1 : 0;
}
function _idbEnc(k) {
  const r = _idbRank(k);
  if (r === 0) return 'n' + (Object.is(k, -0) ? 0 : k);
  if (r === 1) return 'd' + (+k);
  if (r === 2) return 's' + k;
  if (r === 3) return 'b' + Array.prototype.join.call(_idbBytes(k), ',');
  return 'a' + JSON.stringify(Array.prototype.map.call(k, _idbEnc));
}
function _idbCloneKey(k) {
  const r = _idbRank(k);
  if (r === 1) return new Date(+k);
  if (r === 3) return k instanceof ArrayBuffer ? k.slice(0) : k.buffer.slice(k.byteOffset, k.byteOffset + k.byteLength);
  if (r === 4) return Array.prototype.map.call(k, _idbCloneKey);
  return k;
}
const _IDB_KEYERR = 'The parameter is not a valid key.';

// ---- key paths ------------------------------------------------------------
const _IDB_IDENT = /^[\p{ID_Start}$_][\p{ID_Continue}$‌‍]*$/u;
function _idbValidPath(p) {
  if (typeof p === 'string') return p === '' || p.split('.').every((s) => _IDB_IDENT.test(s));
  return Array.isArray(p) && p.length > 0 && p.every((s) => typeof s === 'string' && s !== '' && s.split('.').every((x) => _IDB_IDENT.test(x)));
}
function _idbExtract(kp, value) {
  if (Array.isArray(kp)) {
    const out = [];
    for (const p of kp) { const k = _idbExtract(p, value); if (k === undefined) return undefined; out.push(k); }
    return out;
  }
  if (kp === '') return value;
  let k = value;
  for (const part of String(kp).split('.')) {
    if (k === null || k === undefined) return undefined;
    if (typeof k === 'string' && part === 'length') { k = k.length; continue; }
    if (typeof k !== 'object' && typeof k !== 'function') return undefined;
    if (!Object.prototype.hasOwnProperty.call(k, part)) return undefined;
    k = k[part];
  }
  return k;
}
function _idbInject(kp, value, key) {
  const parts = String(kp).split('.');
  let o = value;
  for (let i = 0; i < parts.length - 1; i++) {
    if (o[parts[i]] === undefined) o[parts[i]] = {};
    o = o[parts[i]];
    if (o === null || typeof o !== 'object') return false;
  }
  if (value === null || typeof value !== 'object') return false;
  o[parts[parts.length - 1]] = key;
  return true;
}
function _idbCanInject(kp, value) {
  if (value === null || typeof value !== 'object') return false;
  let o = value;
  const parts = String(kp).split('.');
  for (let i = 0; i < parts.length - 1; i++) {
    if (!Object.prototype.hasOwnProperty.call(o, parts[i])) return true;
    o = o[parts[i]];
    if (o === null || typeof o !== 'object') return false;
  }
  return true;
}

// ---- DOMStringList --------------------------------------------------------
function _idbStrList(view) {
  const L = globalThis.DOMStringList;
  const target = Object.create(L && L.prototype ? L.prototype : Object.prototype);
  const idx = (k) => typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k);
  const item = function item(i) { const n = view(); i = Number(i) >>> 0; return i < n.length ? n[i] : null; };
  const contains = function contains(s) { return view().indexOf(String(s)) >= 0; };
  const iter = function values() { return view()[Symbol.iterator](); };
  for (const f of [item, contains, iter]) { Object.defineProperty(f, 'length', { value: f === iter ? 0 : 1 }); _markNative(f); }
  Object.defineProperty(iter, 'name', { value: '[Symbol.iterator]' });
  return new Proxy(target, {
    get(t, k, r) {
      if (k === 'length') return view().length;
      if (k === 'item') return item;
      if (k === 'contains') return contains;
      if (k === Symbol.iterator) return iter;
      if (idx(k)) { const n = view(); const i = +k; return i < n.length ? n[i] : undefined; }
      return Reflect.get(t, k, r);
    },
    has(t, k) { return idx(k) ? +k < view().length : Reflect.has(t, k); },
    ownKeys() { return view().map((_, i) => String(i)); },
    getOwnPropertyDescriptor(t, k) {
      if (idx(k)) { const n = view(); if (+k < n.length) return { value: n[+k], writable: false, enumerable: true, configurable: true }; }
      return undefined;
    },
  });
}

// ---- records --------------------------------------------------------------
const _idbRegistryMap = new Map();
function _idbPersistMeta(rec) {
  try {
    const stores = [];
    rec.stores.forEach((st) => stores.push([st.name, st.keyPath, st.autoIncrement ? 1 : 0,
      Array.from(st.indexes.values()).map((ix) => [ix.name, ix.keyPath, ix.unique ? 1 : 0, ix.multiEntry ? 1 : 0])]));
    globalThis.localStorage.setItem('__diting_idb__' + rec.name, JSON.stringify({ v: rec.version, s: stores }));
  } catch (e) {}
}
function _idbNewStoreRec(name, keyPath, autoIncrement) {
  return { name, keyPath: keyPath === undefined ? null : keyPath, autoIncrement: !!autoIncrement, seq: 0, data: new Map(), indexes: new Map() };
}
function _idbRecord(name) {
  let rec = _idbRegistryMap.get(name);
  if (!rec) {
    rec = { name, version: 0, stores: new Map() };
    try {
      const raw = globalThis.localStorage.getItem('__diting_idb__' + name);
      if (raw) {
        const meta = JSON.parse(raw);
        rec.version = meta.v | 0;
        for (const s of meta.s || []) {
          const sr = _idbNewStoreRec(s[0], s[1] ?? null, !!s[2]);
          for (const ix of s[3] || []) sr.indexes.set(ix[0], { name: ix[0], keyPath: ix[1], unique: !!ix[2], multiEntry: !!ix[3] });
          rec.stores.set(s[0], sr);
        }
      }
    } catch (e) {}
    _idbRegistryMap.set(name, rec);
  }
  return rec;
}
function _idbSortedNames(it) { return Array.from(it).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0)); }

// ---- cross-connection open protocol (刀86) --------------------------------
// Chrome: a version-changing open (or deleteDatabase) first fires
// `versionchange` (IDBVersionChangeEvent, oldVersion/newVersion — null for a
// delete) at every OTHER live connection; if any is still open after those
// handlers ran, the waiting request fires `blocked` (also an
// IDBVersionChangeEvent; readyState still 'pending') and parks. The last
// close() releases the parked run (its upgradeneeded fires with readyState
// already 'done' — probed, k86-idb2-chrome.json).
function _idbLiveConns(rec) { return (rec.conns || []).filter((c) => !c.closed); }
function _idbPark(rec, run) {
  (rec.parked || (rec.parked = [])).push(run);
  // last close already happened while nobody was parked: run straight away
  if (!_idbLiveConns(rec).length) _idbDrain(rec);
}
function _idbDrain(rec) {
  if (_idbLiveConns(rec).length || !rec.parked || !rec.parked.length) return;
  const run = rec.parked.shift();
  Promise.resolve().then(run);
}

// ---- ranges ---------------------------------------------------------------
function _idbNewRange(lo, hi, loOpen, hiOpen, hasLo, hasHi) {
  const r = Object.create(IDBKeyRange.prototype);
  _Kr.set(r, { lo, hi, loOpen, hiOpen, hasLo, hasHi });
  return r;
}
function _idbInRange(r, k) {
  if (!r) return true;
  const s = _Kr.get(r);
  if (s.hasLo) { const c = _idbCmp(k, s.lo); if (c < 0 || (c === 0 && s.loOpen)) return false; }
  if (s.hasHi) { const c = _idbCmp(k, s.hi); if (c > 0 || (c === 0 && s.hiOpen)) return false; }
  return true;
}
function _idbToRange(q, meth, iface, strict) {
  if (q === undefined || q === null) {
    if (strict) throw _idbEx('DataError', meth, iface, _IDB_KEYERR);
    return null;
  }
  if (q instanceof IDBKeyRange && _Kr.has(q)) return q;
  if (!_idbValidKey(q)) throw _idbEx('DataError', meth, iface, _IDB_KEYERR);
  const k = _idbCloneKey(q);
  return _idbNewRange(k, k, false, false, true, true);
}

// ---- transactions & requests ----------------------------------------------
function _idbNew(C) { _idbMint = true; try { return new C(); } finally { _idbMint = false; } }
let _idbMint = false;
function _idbIllegal(name) { if (!_idbMint) throw new TypeError("Failed to construct '" + name + "': Illegal constructor"); }

function _idbFire(target, ev) {
  try { target.dispatchEvent(ev); } catch (e) {}
  return ev;
}
function _idbErrEvent() { return new Event('error', { bubbles: true, cancelable: true }); }

function _idbNewReq(Cls, source, tx) {
  const r = _idbNew(Cls);
  _Rq.set(r, { done: false, result: undefined, error: null, source: source === undefined ? null : source, tx: tx || null, h: Object.create(null) });
  if (tx) _idbParents.set(r, tx);
  return r;
}
function _idbCheckTx(tx, meth, iface) {
  const t = _Tx.get(tx);
  if (t.done || t.aborted || t.committing) throw _idbEx('TransactionInactiveError', meth, iface, 'The transaction has finished.');
  return t;
}
function _idbCheckWrite(t, meth, iface) {
  if (t.mode === 'readonly') throw _idbEx('ReadOnlyError', meth, iface, 'The transaction is read-only.');
}
// Requests settle asynchronously; after the success/error dispatch (so a
// handler chaining another op on the same tx grows the live set first) the
// request detaches from its transaction and the tx may complete. A request
// error that is not preventDefault'ed aborts the transaction (spec).
function _idbQueue(req, produce) {
  const rs = _Rq.get(req);
  const tx = rs.tx;
  const t = tx ? _Tx.get(tx) : null;
  if (t) t.live.add(req);
  Promise.resolve().then(() => {
    let err = null;
    if (t && t.aborted) err = _idbErr('AbortError', 'The transaction was aborted, so the request cannot be fulfilled.');
    else {
      try { rs.result = produce(); } catch (e) { err = e; }
    }
    rs.done = true;
    if (err) {
      rs.error = err; rs.result = undefined;
      const ev = _idbFire(req, _idbErrEvent());
      if (t && !ev.defaultPrevented && !t.aborted && !t.done) _idbAbort(tx, err);
    } else {
      _idbFire(req, new Event('success'));
    }
    if (t) { t.live.delete(req); _idbMaybeComplete(tx); }
  });
}
function _idbMaybeComplete(tx) {
  const t = _Tx.get(tx);
  if (t.done || t.aborted || t.live.size > 0 || t.completeQueued) return;
  t.completeQueued = true;
  Promise.resolve().then(() => {
    t.completeQueued = false;
    if (t.done || t.aborted || t.live.size > 0) return;
    t.done = true;
    _idbFire(tx, new Event('complete'));
    if (t.onDone) t.onDone('complete');
  });
}
function _idbAbort(tx, error) {
  const t = _Tx.get(tx);
  if (t.done || t.aborted) return;
  t.aborted = true;
  t.error = error || null;
  for (let i = t.undo.length - 1; i >= 0; i--) { try { t.undo[i](); } catch (e) {} }
  t.undo.length = 0;
  // Pending requests already have their settle microtasks queued ahead of this
  // one: they fail with AbortError first, then the tx's abort event fires.
  Promise.resolve().then(() => {
    t.done = true;
    _idbFire(tx, new Event('abort', { bubbles: true }));
    if (t.onDone) t.onDone('abort');
  });
}
function _idbSnap(t, srec) {
  if (t.snapped.has(srec)) return;
  t.snapped.add(srec);
  const d = new Map(srec.data), sq = srec.seq;
  t.undo.push(() => { srec.data = d; srec.seq = sq; });
}
function _idbNewTx(rec, db, names, mode, durability) {
  const tx = _idbNew(IDBTransaction);
  const t = { rec, db, mode, durability, scope: new Set(names), live: new Set(), done: false, aborted: false, committing: false,
    completeQueued: false, error: null, undo: [], snapped: new Set(), stores: new Map(), onDone: null, h: Object.create(null) };
  _Tx.set(tx, t);
  _idbParents.set(tx, db);
  _idbMaybeComplete(tx);
  return tx;
}
function _idbStoreInst(tx, srec) {
  const t = _Tx.get(tx);
  let s = t.stores.get(srec);
  if (!s) {
    s = _idbNew(IDBObjectStore);
    _Os.set(s, { tx, srec, ixs: new Map(), deleted: false });
    t.stores.set(srec, s);
  }
  return s;
}
function _idbIndexInst(store, irec) {
  const os = _Os.get(store);
  let x = os.ixs.get(irec);
  if (!x) {
    x = _idbNew(IDBIndex);
    _Ix.set(x, { store, irec, deleted: false });
    os.ixs.set(irec, x);
  }
  return x;
}

// ---- store data helpers ---------------------------------------------------
function _idbRows(srec) {
  const rows = Array.from(srec.data.values());
  rows.sort((a, b) => _idbCmp(a.key, b.key));
  return rows;
}
function _idbIdxKeys(irec, value) {
  let k = _idbExtract(irec.keyPath, value);
  if (k === undefined) return [];
  if (irec.multiEntry && Array.isArray(k)) {
    const seen = new Set(), out = [];
    for (const x of k) { if (_idbValidKey(x)) { const e = _idbEnc(x); if (!seen.has(e)) { seen.add(e); out.push(x); } } }
    return out;
  }
  return _idbValidKey(k) ? [k] : [];
}
function _idbIdxRows(srec, irec) {
  const out = [];
  for (const row of srec.data.values()) for (const ik of _idbIdxKeys(irec, row.value)) out.push({ key: ik, pk: row.key, value: row.value });
  out.sort((a, b) => _idbCmp(a.key, b.key) || _idbCmp(a.pk, b.pk));
  return out;
}
function _idbCheckUnique(srec, row, pkEnc) {
  for (const irec of srec.indexes.values()) {
    if (!irec.unique) continue;
    const mine = _idbIdxKeys(irec, row);
    if (!mine.length) continue;
    const enc = new Set(mine.map(_idbEnc));
    for (const [e, other] of srec.data) {
      if (e === pkEnc) continue;
      for (const ok of _idbIdxKeys(irec, other.value)) {
        if (enc.has(_idbEnc(ok))) throw _idbErr('ConstraintError', "Unable to add key to index '" + irec.name + "': at least one key does not satisfy the uniqueness requirements.");
      }
    }
  }
}
function _idbUncloneable(v, seen) {
  // First value structuredClone cannot carry (function / symbol), found shallowly
  // so the DataCloneError can name it the way Chrome does.
  if (typeof v === 'function' || typeof v === 'symbol') return v;
  if (v === null || typeof v !== 'object' || seen.indexOf(v) >= 0 || seen.length > 64) return undefined;
  seen.push(v);
  for (const k of Object.keys(v)) { const f = _idbUncloneable(v[k], seen); if (f !== undefined) return f; }
  return undefined;
}
function _idbClone(v, meth, iface) {
  try { return structuredClone(v); } catch (e) {
    const bad = _idbUncloneable(v, []);
    const what = typeof bad === 'function' ? Function.prototype.toString.call(bad) : bad !== undefined ? String(bad) : '#<Object>';
    throw _idbEx('DataCloneError', meth, iface, what + ' could not be cloned.');
  }
}
const _IDB_DIRS = ['next', 'nextunique', 'prev', 'prevunique'];
function _idbList(rows, range, dir) {
  let list = range ? rows.filter((r) => _idbInRange(range, r.key)) : rows.slice();
  if (dir === 'nextunique' || dir === 'prevunique') {
    const keep = [];
    for (const r of list) if (!keep.length || _idbCmp(keep[keep.length - 1].key, r.key) !== 0) keep.push(r);
    list = keep;
  }
  if (dir === 'prev' || dir === 'prevunique') list.reverse();
  return list;
}
// getAll/getAllKeys/getAllRecords argument decoding: (query, count) or an options dict.
function _idbQueryArgs(a0, a1, meth, iface, allowDir) {
  let query = a0, count = a1, dir = 'next';
  if (a0 !== null && typeof a0 === 'object' && !(a0 instanceof IDBKeyRange) && !_idbValidKey(a0)) {
    query = a0.query; count = a0.count;
    if (a0.direction !== undefined) dir = _idbEnum(a0.direction, _IDB_DIRS, 'IDBCursorDirection', "Failed to execute '" + meth + "' on '" + iface + "': Failed to read the 'direction' property from 'IDBGetAllOptions': ");
  } else if (!allowDir) dir = 'next';
  const range = _idbToRange(query, meth, iface, false);
  const n = count === undefined || count === null || count === 0 ? Infinity : (Number(count) >>> 0) || Infinity;
  return { range, n, dir };
}
function _idbMkRecord(key, pk, value) {
  const r = _idbNew(IDBRecord);
  _Rc.set(r, { key, pk, value });
  return r;
}

// ---- cursors --------------------------------------------------------------
function _idbOpenCursor(source, tx, list, dir, keyOnly, srec, isIndex) {
  const req = _idbNewReq(IDBRequest, source, tx);
  const rs = _Rq.get(req);
  const t = _Tx.get(tx);
  const cs = { req, tx, source, list, dir, keyOnly, srec, isIndex, pos: -1, got: false, key: undefined, pk: undefined, value: undefined, cur: null };
  const step = (target) => {
    // target: index to land on (or list.length for past-the-end)
    cs.got = false;
    cs.pending = true;
    rs.done = false;
    t.live.add(req);
    Promise.resolve().then(() => {
      cs.pending = false;
      rs.done = true;
      cs.pos = target;
      if (t.aborted) {
        rs.error = _idbErr('AbortError', 'The transaction was aborted, so the request cannot be fulfilled.');
        rs.result = undefined;
        const ev = _idbFire(req, _idbErrEvent());
        void ev;
      } else if (target >= list.length) {
        rs.result = null;
        _idbFire(req, new Event('success'));
      } else {
        const row = list[target];
        const rpk = row.pk !== undefined ? row.pk : row.key;
        cs.key = _idbCloneKey(row.key);
        cs.pk = _idbCloneKey(rpk);
        const cur = srec.data.get(_idbEnc(rpk));
        cs.value = keyOnly ? undefined : (cur ? _idbClone(cur.value, 'openCursor', 'IDBObjectStore') : undefined);
        if (!cs.cur) { cs.cur = _idbNew(keyOnly ? IDBCursor : IDBCursorWithValue); _Cu.set(cs.cur, cs); }
        cs.got = true;
        rs.result = cs.cur;
        _idbFire(req, new Event('success'));
      }
      // a handler that chained continue()/advance() re-armed the request: keep
      // it live; otherwise it detaches and the transaction may complete.
      if (!cs.pending) t.live.delete(req);
      _idbMaybeComplete(tx);
    });
  };
  cs.step = step;
  step(0);
  return req;
}
function _idbCursorGate(self, meth) {
  const cs = _idbBrand(_Cu, self);
  const t = _Tx.get(cs.tx);
  if (t.done || t.aborted || t.committing) throw _idbEx('TransactionInactiveError', meth, 'IDBCursor', 'The transaction has finished.');
  return cs;
}
const _IDB_BUSY = 'The cursor is being iterated or has iterated past its end.';

// ---- classes --------------------------------------------------------------
class IDBRequest extends EventTarget { constructor() { _idbIllegal('IDBRequest'); super(); } }
class IDBOpenDBRequest extends IDBRequest { constructor() { _idbIllegal('IDBOpenDBRequest'); super(); } }
class IDBDatabase extends EventTarget { constructor() { _idbIllegal('IDBDatabase'); super(); } }
class IDBTransaction extends EventTarget { constructor() { _idbIllegal('IDBTransaction'); super(); } }
class IDBObjectStore { constructor() { _idbIllegal('IDBObjectStore'); } }
class IDBIndex { constructor() { _idbIllegal('IDBIndex'); } }
class IDBCursor { constructor() { _idbIllegal('IDBCursor'); } }
class IDBCursorWithValue extends IDBCursor { constructor() { _idbIllegal('IDBCursorWithValue'); super(); } }
class IDBRecord { constructor() { _idbIllegal('IDBRecord'); } }
class IDBFactory { constructor() { _idbIllegal('IDBFactory'); } }
class IDBKeyRange { constructor() { throw new TypeError("Failed to construct 'IDBKeyRange': Illegal constructor"); } }

function _idbPin(fn, name, len) {
  Object.defineProperty(fn, 'name', { value: name, configurable: true });
  if (len !== undefined) Object.defineProperty(fn, 'length', { value: len, configurable: true });
  return _markNative(fn);
}
function _idbMethod(C, name, len, fn) {
  Object.defineProperty(C.prototype, name, { value: _idbPin(fn, name, len), writable: true, enumerable: true, configurable: true });
}
function _idbAttr(C, name, getter, setter) {
  const d = { get: _idbPin(getter, 'get ' + name), enumerable: true, configurable: true };
  if (setter) d.set = _idbPin(setter, 'set ' + name);
  Object.defineProperty(C.prototype, name, d);
}
function _idbHandlers(C, map, types) {
  for (const type of types) {
    _idbAttr(C, 'on' + type, function () { return _idbBrand(map, this).h[type] || null; },
      function (v) { _idbBrand(map, this).h[type] = typeof v === 'function' ? v : null; });
  }
}
for (const [C, tag] of [[IDBRequest, 'IDBRequest'], [IDBOpenDBRequest, 'IDBOpenDBRequest'], [IDBDatabase, 'IDBDatabase'],
  [IDBTransaction, 'IDBTransaction'], [IDBObjectStore, 'IDBObjectStore'], [IDBIndex, 'IDBIndex'], [IDBCursor, 'IDBCursor'],
  [IDBCursorWithValue, 'IDBCursorWithValue'], [IDBRecord, 'IDBRecord'], [IDBFactory, 'IDBFactory'], [IDBKeyRange, 'IDBKeyRange']]) {
  Object.defineProperty(C.prototype, Symbol.toStringTag, { value: tag, configurable: true });
  Object.defineProperty(C, 'name', { value: tag, configurable: true });
  Object.defineProperty(C, 'length', { value: 0, configurable: true });
  _markNative(C);
}

// IDBRequest
_idbAttr(IDBRequest, 'result', function () {
  const s = _idbBrand(_Rq, this);
  if (!s.done) throw _idbErr('InvalidStateError', "Failed to read the 'result' property from 'IDBRequest': The request has not finished.");
  return s.result;
});
_idbAttr(IDBRequest, 'error', function () {
  const s = _idbBrand(_Rq, this);
  if (!s.done) throw _idbErr('InvalidStateError', "Failed to read the 'error' property from 'IDBRequest': The request has not finished.");
  return s.error;
});
_idbAttr(IDBRequest, 'source', function () { return _idbBrand(_Rq, this).source; });
_idbAttr(IDBRequest, 'transaction', function () { return _idbBrand(_Rq, this).tx; });
_idbAttr(IDBRequest, 'readyState', function () { return _idbBrand(_Rq, this).done ? 'done' : 'pending'; });
_idbHandlers(IDBRequest, _Rq, ['success', 'error']);
_idbHandlers(IDBOpenDBRequest, _Rq, ['blocked', 'upgradeneeded']);

// IDBTransaction
_idbAttr(IDBTransaction, 'objectStoreNames', function () {
  const t = _idbBrand(_Tx, this);
  return _idbStrList(() => _idbSortedNames(t.mode === 'versionchange' ? t.rec.stores.keys() : t.scope));
});
_idbAttr(IDBTransaction, 'mode', function () { return _idbBrand(_Tx, this).mode; });
_idbAttr(IDBTransaction, 'durability', function () { return _idbBrand(_Tx, this).durability; });
_idbAttr(IDBTransaction, 'db', function () { return _idbBrand(_Tx, this).db; });
_idbAttr(IDBTransaction, 'error', function () { return _idbBrand(_Tx, this).error; });
_idbHandlers(IDBTransaction, _Tx, ['abort', 'complete', 'error']);
_idbMethod(IDBTransaction, 'abort', 0, function abort() {
  const t = _idbBrand(_Tx, this);
  if (t.done || t.aborted || t.committing) throw _idbEx('InvalidStateError', 'abort', 'IDBTransaction', 'The transaction has finished.');
  _idbAbort(this, null);
});
_idbMethod(IDBTransaction, 'commit', 0, function commit() {
  const t = _idbBrand(_Tx, this);
  if (t.done || t.aborted || t.committing) throw _idbEx('InvalidStateError', 'commit', 'IDBTransaction', 'The transaction has finished.');
  t.committing = true;
  _idbMaybeComplete(this);
});
_idbMethod(IDBTransaction, 'objectStore', 1, function objectStore(name) {
  const t = _idbBrand(_Tx, this);
  _idbArgc(arguments, 1, 'objectStore', 'IDBTransaction');
  if (t.done || t.aborted) throw _idbEx('InvalidStateError', 'objectStore', 'IDBTransaction', 'The transaction has finished.');
  const n = String(name);
  const srec = t.rec.stores.get(n);
  if (!srec || (t.mode !== 'versionchange' && !t.scope.has(n))) throw _idbEx('NotFoundError', 'objectStore', 'IDBTransaction', 'The specified object store was not found.');
  return _idbStoreInst(this, srec);
});

// IDBDatabase
_idbAttr(IDBDatabase, 'name', function () { return _idbBrand(_Db, this).rec.name; });
_idbAttr(IDBDatabase, 'version', function () { return _idbBrand(_Db, this).rec.version; });
_idbAttr(IDBDatabase, 'objectStoreNames', function () {
  const d = _idbBrand(_Db, this);
  return _idbStrList(() => _idbSortedNames(d.rec.stores.keys()));
});
_idbHandlers(IDBDatabase, _Db, ['abort', 'close', 'error', 'versionchange']);
_idbMethod(IDBDatabase, 'close', 0, function close() {
  const d = _idbBrand(_Db, this);
  if (!d.closed) { d.closed = true; _idbDrain(d.rec); }
});
function _idbNeedVtx(d, meth, iface) {
  if (!d.vtx || _Tx.get(d.vtx).done) throw _idbEx('InvalidStateError', meth, iface, 'The database is not running a version change transaction.');
  return _Tx.get(d.vtx);
}
_idbMethod(IDBDatabase, 'createObjectStore', 1, function createObjectStore(name, options) {
  const d = _idbBrand(_Db, this);
  _idbArgc(arguments, 1, 'createObjectStore', 'IDBDatabase');
  const vt = _idbNeedVtx(d, 'createObjectStore', 'IDBDatabase');
  const n = String(name);
  let kp = null, ai = false;
  if (options !== undefined && options !== null) {
    if (options.keyPath !== undefined && options.keyPath !== null) {
      kp = typeof options.keyPath === 'string' ? options.keyPath : (Array.isArray(options.keyPath) ? Array.from(options.keyPath, String) : String(options.keyPath));
      if (!_idbValidPath(kp)) throw _idbEx('SyntaxError', 'createObjectStore', 'IDBDatabase', 'The keyPath option is not a valid key path.');
    }
    ai = !!options.autoIncrement;
  }
  if (ai && (kp === '' || Array.isArray(kp))) throw _idbEx('InvalidAccessError', 'createObjectStore', 'IDBDatabase', 'The autoIncrement option was set but the keyPath option was empty or an array.');
  if (d.rec.stores.has(n)) throw _idbEx('ConstraintError', 'createObjectStore', 'IDBDatabase', 'An object store with the specified name already exists.');
  const srec = _idbNewStoreRec(n, kp, ai);
  d.rec.stores.set(n, srec);
  vt.scope.add(n);
  return _idbStoreInst(d.vtx, srec);
});
_idbMethod(IDBDatabase, 'deleteObjectStore', 1, function deleteObjectStore(name) {
  const d = _idbBrand(_Db, this);
  _idbArgc(arguments, 1, 'deleteObjectStore', 'IDBDatabase');
  const vt = _idbNeedVtx(d, 'deleteObjectStore', 'IDBDatabase');
  const n = String(name);
  const srec = d.rec.stores.get(n);
  if (!srec) throw _idbEx('NotFoundError', 'deleteObjectStore', 'IDBDatabase', 'The specified object store was not found.');
  d.rec.stores.delete(n);
  vt.scope.delete(n);
  const inst = vt.stores.get(srec);
  if (inst) _Os.get(inst).deleted = true;
});
_idbMethod(IDBDatabase, 'transaction', 1, function transaction(storeNames, mode, options) {
  const d = _idbBrand(_Db, this);
  _idbArgc(arguments, 1, 'transaction', 'IDBDatabase');
  const IF = 'IDBDatabase';
  if (d.vtx && !_Tx.get(d.vtx).done) throw _idbEx('InvalidStateError', 'transaction', IF, 'A version change transaction is running.');
  if (d.closed) throw _idbEx('InvalidStateError', 'transaction', IF, 'The database connection is closing.');
  let names;
  if (typeof storeNames === 'string') names = [storeNames];
  else if (storeNames !== null && typeof storeNames === 'object' && typeof storeNames[Symbol.iterator] === 'function') names = Array.from(storeNames, String);
  else names = [String(storeNames)];
  let m = 'readonly';
  if (mode !== undefined) {
    if (String(mode) === 'versionchange') throw new TypeError("Failed to execute 'transaction' on 'IDBDatabase': The mode provided ('versionchange') is not one of 'readonly' or 'readwrite'.");
    m = _idbEnum(mode, ['readonly', 'readwrite'], 'IDBTransactionMode', "Failed to execute 'transaction' on 'IDBDatabase': ");
  }
  let dur = 'default';
  if (options !== undefined && options !== null && options.durability !== undefined) {
    dur = _idbEnum(options.durability, ['default', 'strict', 'relaxed'], 'IDBTransactionDurability',
      "Failed to execute 'transaction' on 'IDBDatabase': Failed to read the 'durability' property from 'IDBTransactionOptions': ");
  }
  if (names.length === 0) throw _idbEx('InvalidAccessError', 'transaction', IF, 'The storeNames parameter was empty.');
  for (const n of names) if (!d.rec.stores.has(n)) throw _idbEx('NotFoundError', 'transaction', IF, 'One of the specified object stores was not found.');
  return _idbNewTx(d.rec, this, names, m, dur);
});

// IDBObjectStore
function _idbOs(self, meth) {
  const s = _idbBrand(_Os, self);
  const t = _Tx.get(s.tx);
  if (s.deleted) throw _idbEx('InvalidStateError', meth, 'IDBObjectStore', 'The object store has been deleted.');
  if (t.done || t.aborted || t.committing) throw _idbEx('TransactionInactiveError', meth, 'IDBObjectStore', 'The transaction has finished.');
  return { s, t, srec: s.srec };
}
_idbAttr(IDBObjectStore, 'name', function () { return _idbBrand(_Os, this).srec.name; }, function (v) {
  const s = _idbBrand(_Os, this);
  const t = _Tx.get(s.tx);
  if (t.mode !== 'versionchange' || t.done) throw _idbErr('InvalidStateError', "Failed to set the 'name' property on 'IDBObjectStore': The database is not running a version change transaction.");
  const n = String(v);
  if (n === s.srec.name) return;
  if (t.rec.stores.has(n)) throw _idbErr('ConstraintError', "Failed to set the 'name' property on 'IDBObjectStore': An object store with the specified name already exists.");
  t.rec.stores.delete(s.srec.name); s.srec.name = n; t.rec.stores.set(n, s.srec); t.scope.add(n);
});
_idbAttr(IDBObjectStore, 'keyPath', function () { return _idbBrand(_Os, this).srec.keyPath; });
_idbAttr(IDBObjectStore, 'indexNames', function () {
  const s = _idbBrand(_Os, this);
  return _idbStrList(() => _idbSortedNames(s.srec.indexes.keys()));
});
_idbAttr(IDBObjectStore, 'transaction', function () { return _idbBrand(_Os, this).tx; });
_idbAttr(IDBObjectStore, 'autoIncrement', function () { return _idbBrand(_Os, this).srec.autoIncrement; });

function _idbPut(self, value, key, meth, noOverwrite, argc) {
  _idbArgc(argc, 1, meth, 'IDBObjectStore');
  const { s, t, srec } = _idbOs(self, meth);
  _idbCheckWrite(t, meth, 'IDBObjectStore');
  const clone = _idbClone(value, meth, 'IDBObjectStore');
  let explicit;
  if (srec.keyPath !== null) {
    if (key !== undefined) throw _idbEx('DataError', meth, 'IDBObjectStore', 'The object store uses in-line keys and the key parameter was provided.');
    const k = _idbExtract(srec.keyPath, clone);
    if (k === undefined) {
      if (!srec.autoIncrement) throw _idbEx('DataError', meth, 'IDBObjectStore', "Evaluating the object store's key path did not yield a value.");
      if (!_idbCanInject(srec.keyPath, clone)) throw _idbEx('DataError', meth, 'IDBObjectStore', 'A generated key could not be inserted into the value using the key path.');
    } else if (!_idbValidKey(k)) throw _idbEx('DataError', meth, 'IDBObjectStore', "Evaluating the object store's key path yielded a value that is not a valid key.");
    else explicit = k;
  } else if (key !== undefined) {
    if (!_idbValidKey(key)) throw _idbEx('DataError', meth, 'IDBObjectStore', _IDB_KEYERR);
    explicit = key;
  } else if (!srec.autoIncrement) throw _idbEx('DataError', meth, 'IDBObjectStore', 'The object store uses out-of-line keys and has no key generator and the key parameter was not provided.');
  const req = _idbNewReq(IDBRequest, self, s.tx);
  _idbQueue(req, () => {
    let k;
    if (explicit !== undefined) {
      k = _idbCloneKey(explicit);
      if (typeof k === 'number' && srec.autoIncrement && Math.floor(k) > srec.seq) { _idbSnap(t, srec); srec.seq = Math.min(Math.floor(k), 9007199254740992); }
    } else {
      if (srec.seq >= 9007199254740992) throw _idbErr('ConstraintError', 'The key generator has reached its maximum value.');
      _idbSnap(t, srec);
      k = ++srec.seq;
      if (srec.keyPath !== null) _idbInject(srec.keyPath, clone, k);
    }
    const enc = _idbEnc(k);
    if (noOverwrite && srec.data.has(enc)) throw _idbErr('ConstraintError', 'Key already exists in the object store.');
    _idbCheckUnique(srec, clone, enc);
    _idbSnap(t, srec);
    srec.data.set(enc, { key: k, value: clone });
    return _idbCloneKey(k);
  });
  return req;
}
_idbMethod(IDBObjectStore, 'put', 1, function put(value, key) { return _idbPut(this, value, key, 'put', false, arguments); });
_idbMethod(IDBObjectStore, 'add', 1, function add(value, key) { return _idbPut(this, value, key, 'add', true, arguments); });
_idbMethod(IDBObjectStore, 'get', 1, function get(query) {
  _idbArgc(arguments, 1, 'get', 'IDBObjectStore');
  const { s, srec } = _idbOs(this, 'get');
  const range = _idbToRange(query, 'get', 'IDBObjectStore', true);
  const req = _idbNewReq(IDBRequest, this, s.tx);
  _idbQueue(req, () => { for (const r of _idbRows(srec)) if (_idbInRange(range, r.key)) return _idbClone(r.value, 'get', 'IDBObjectStore'); return undefined; });
  return req;
});
_idbMethod(IDBObjectStore, 'getKey', 1, function getKey(query) {
  _idbArgc(arguments, 1, 'getKey', 'IDBObjectStore');
  const { s, srec } = _idbOs(this, 'getKey');
  const range = _idbToRange(query, 'getKey', 'IDBObjectStore', true);
  const req = _idbNewReq(IDBRequest, this, s.tx);
  _idbQueue(req, () => { for (const r of _idbRows(srec)) if (_idbInRange(range, r.key)) return _idbCloneKey(r.key); return undefined; });
  return req;
});
function _idbStoreAll(self, meth, a0, a1, shape) {
  const { s, srec } = _idbOs(self, meth);
  const { range, n, dir } = _idbQueryArgs(a0, a1, meth, 'IDBObjectStore', true);
  const req = _idbNewReq(IDBRequest, self, s.tx);
  _idbQueue(req, () => _idbList(_idbRows(srec), range, dir).slice(0, n === Infinity ? undefined : n).map((r) => shape(r)));
  return req;
}
_idbMethod(IDBObjectStore, 'getAll', 0, function getAll(query, count) {
  return _idbStoreAll(this, 'getAll', query, count, (r) => _idbClone(r.value, 'getAll', 'IDBObjectStore'));
});
_idbMethod(IDBObjectStore, 'getAllKeys', 0, function getAllKeys(query, count) {
  return _idbStoreAll(this, 'getAllKeys', query, count, (r) => _idbCloneKey(r.key));
});
_idbMethod(IDBObjectStore, 'getAllRecords', 0, function getAllRecords(options) {
  return _idbStoreAll(this, 'getAllRecords', options === undefined ? {} : options, undefined,
    (r) => _idbMkRecord(_idbCloneKey(r.key), _idbCloneKey(r.key), _idbClone(r.value, 'getAllRecords', 'IDBObjectStore')));
});
_idbMethod(IDBObjectStore, 'count', 0, function count(query) {
  const { s, srec } = _idbOs(this, 'count');
  const range = _idbToRange(query, 'count', 'IDBObjectStore', false);
  const req = _idbNewReq(IDBRequest, this, s.tx);
  _idbQueue(req, () => (range ? _idbRows(srec).filter((r) => _idbInRange(range, r.key)).length : srec.data.size));
  return req;
});
_idbMethod(IDBObjectStore, 'delete', 1, function (query) {
  _idbArgc(arguments, 1, 'delete', 'IDBObjectStore');
  const { s, t, srec } = _idbOs(this, 'delete');
  _idbCheckWrite(t, 'delete', 'IDBObjectStore');
  const range = _idbToRange(query, 'delete', 'IDBObjectStore', true);
  const req = _idbNewReq(IDBRequest, this, s.tx);
  _idbQueue(req, () => {
    _idbSnap(t, srec);
    for (const r of _idbRows(srec)) if (_idbInRange(range, r.key)) srec.data.delete(_idbEnc(r.key));
    return undefined;
  });
  return req;
});
_idbMethod(IDBObjectStore, 'clear', 0, function clear() {
  const { s, t, srec } = _idbOs(this, 'clear');
  _idbCheckWrite(t, 'clear', 'IDBObjectStore');
  const req = _idbNewReq(IDBRequest, this, s.tx);
  _idbQueue(req, () => { _idbSnap(t, srec); srec.data.clear(); return undefined; });
  return req;
});
function _idbStoreCursor(self, meth, query, direction, keyOnly) {
  const { s, srec } = _idbOs(self, meth);
  const range = _idbToRange(query, meth, 'IDBObjectStore', false);
  const dir = direction === undefined ? 'next' : _idbEnum(direction, _IDB_DIRS, 'IDBCursorDirection', "Failed to execute '" + meth + "' on 'IDBObjectStore': ");
  return _idbOpenCursor(self, s.tx, _idbList(_idbRows(srec), range, dir), dir, keyOnly, srec, false);
}
_idbMethod(IDBObjectStore, 'openCursor', 0, function openCursor(query, direction) { return _idbStoreCursor(this, 'openCursor', query, direction, false); });
_idbMethod(IDBObjectStore, 'openKeyCursor', 0, function openKeyCursor(query, direction) { return _idbStoreCursor(this, 'openKeyCursor', query, direction, true); });
_idbMethod(IDBObjectStore, 'index', 1, function index(name) {
  _idbArgc(arguments, 1, 'index', 'IDBObjectStore');
  const s = _idbBrand(_Os, this);
  if (s.deleted) throw _idbEx('InvalidStateError', 'index', 'IDBObjectStore', 'The object store has been deleted.');
  if (_Tx.get(s.tx).done) throw _idbEx('InvalidStateError', 'index', 'IDBObjectStore', 'The transaction has finished.');
  const irec = s.srec.indexes.get(String(name));
  if (!irec) throw _idbEx('NotFoundError', 'index', 'IDBObjectStore', 'The specified index was not found.');
  return _idbIndexInst(this, irec);
});
_idbMethod(IDBObjectStore, 'createIndex', 2, function createIndex(name, keyPath, options) {
  _idbArgc(arguments, 2, 'createIndex', 'IDBObjectStore');
  const s = _idbBrand(_Os, this);
  const t = _Tx.get(s.tx);
  if (t.mode !== 'versionchange' || t.done) throw _idbEx('InvalidStateError', 'createIndex', 'IDBObjectStore', 'The database is not running a version change transaction.');
  if (s.deleted) throw _idbEx('InvalidStateError', 'createIndex', 'IDBObjectStore', 'The object store has been deleted.');
  const n = String(name);
  if (s.srec.indexes.has(n)) throw _idbEx('ConstraintError', 'createIndex', 'IDBObjectStore', 'An index with the specified name already exists.');
  const kp = Array.isArray(keyPath) ? Array.from(keyPath, String) : String(keyPath);
  if (!_idbValidPath(kp)) throw _idbEx('SyntaxError', 'createIndex', 'IDBObjectStore', 'The keyPath argument contains an invalid key path.');
  const multi = !!(options && options.multiEntry);
  if (Array.isArray(kp) && multi) throw _idbEx('InvalidAccessError', 'createIndex', 'IDBObjectStore', 'The keyPath argument was an array and the multiEntry option is true.');
  const irec = { name: n, keyPath: kp, unique: !!(options && options.unique), multiEntry: multi };
  s.srec.indexes.set(n, irec);
  t.undo.push(() => { s.srec.indexes.delete(n); });
  return _idbIndexInst(this, irec);
});
_idbMethod(IDBObjectStore, 'deleteIndex', 1, function deleteIndex(name) {
  _idbArgc(arguments, 1, 'deleteIndex', 'IDBObjectStore');
  const s = _idbBrand(_Os, this);
  const t = _Tx.get(s.tx);
  if (t.mode !== 'versionchange' || t.done) throw _idbEx('InvalidStateError', 'deleteIndex', 'IDBObjectStore', 'The database is not running a version change transaction.');
  const n = String(name);
  const irec = s.srec.indexes.get(n);
  if (!irec) throw _idbEx('NotFoundError', 'deleteIndex', 'IDBObjectStore', 'The specified index was not found.');
  s.srec.indexes.delete(n);
  t.undo.push(() => { s.srec.indexes.set(n, irec); });
  const inst = s.ixs.get(irec);
  if (inst) _Ix.get(inst).deleted = true;
});

// IDBIndex
function _idbIx(self, meth) {
  const x = _idbBrand(_Ix, self);
  const os = _Os.get(x.store);
  const t = _Tx.get(os.tx);
  if (x.deleted || os.deleted) throw _idbEx('InvalidStateError', meth, 'IDBIndex', 'The index or its object store has been deleted.');
  if (t.done || t.aborted || t.committing) throw _idbEx('TransactionInactiveError', meth, 'IDBIndex', 'The transaction has finished.');
  return { x, os, t, srec: os.srec, irec: x.irec };
}
_idbAttr(IDBIndex, 'name', function () { return _idbBrand(_Ix, this).irec.name; }, function (v) {
  const x = _idbBrand(_Ix, this);
  const os = _Os.get(x.store);
  const t = _Tx.get(os.tx);
  if (t.mode !== 'versionchange' || t.done) throw _idbErr('InvalidStateError', "Failed to set the 'name' property on 'IDBIndex': The database is not running a version change transaction.");
  const n = String(v);
  if (n === x.irec.name) return;
  if (os.srec.indexes.has(n)) throw _idbErr('ConstraintError', "Failed to set the 'name' property on 'IDBIndex': An index with the specified name already exists.");
  os.srec.indexes.delete(x.irec.name); x.irec.name = n; os.srec.indexes.set(n, x.irec);
});
_idbAttr(IDBIndex, 'objectStore', function () { return _idbBrand(_Ix, this).store; });
_idbAttr(IDBIndex, 'keyPath', function () { return _idbBrand(_Ix, this).irec.keyPath; });
_idbAttr(IDBIndex, 'multiEntry', function () { return _idbBrand(_Ix, this).irec.multiEntry; });
_idbAttr(IDBIndex, 'unique', function () { return _idbBrand(_Ix, this).irec.unique; });
_idbMethod(IDBIndex, 'get', 1, function get(query) {
  _idbArgc(arguments, 1, 'get', 'IDBIndex');
  const { os, srec, irec } = _idbIx(this, 'get');
  const range = _idbToRange(query, 'get', 'IDBIndex', true);
  const req = _idbNewReq(IDBRequest, this, os.tx);
  _idbQueue(req, () => { for (const r of _idbIdxRows(srec, irec)) if (_idbInRange(range, r.key)) return _idbClone(r.value, 'get', 'IDBIndex'); return undefined; });
  return req;
});
_idbMethod(IDBIndex, 'getKey', 1, function getKey(query) {
  _idbArgc(arguments, 1, 'getKey', 'IDBIndex');
  const { os, srec, irec } = _idbIx(this, 'getKey');
  const range = _idbToRange(query, 'getKey', 'IDBIndex', true);
  const req = _idbNewReq(IDBRequest, this, os.tx);
  _idbQueue(req, () => { for (const r of _idbIdxRows(srec, irec)) if (_idbInRange(range, r.key)) return _idbCloneKey(r.pk); return undefined; });
  return req;
});
function _idbIdxAll(self, meth, a0, a1, shape) {
  const { os, srec, irec } = _idbIx(self, meth);
  const { range, n, dir } = _idbQueryArgs(a0, a1, meth, 'IDBIndex', true);
  const req = _idbNewReq(IDBRequest, self, os.tx);
  _idbQueue(req, () => _idbList(_idbIdxRows(srec, irec), range, dir).slice(0, n === Infinity ? undefined : n).map((r) => shape(r)));
  return req;
}
_idbMethod(IDBIndex, 'getAll', 0, function getAll(query, count) {
  return _idbIdxAll(this, 'getAll', query, count, (r) => _idbClone(r.value, 'getAll', 'IDBIndex'));
});
_idbMethod(IDBIndex, 'getAllKeys', 0, function getAllKeys(query, count) {
  return _idbIdxAll(this, 'getAllKeys', query, count, (r) => _idbCloneKey(r.pk));
});
_idbMethod(IDBIndex, 'getAllRecords', 0, function getAllRecords(options) {
  return _idbIdxAll(this, 'getAllRecords', options === undefined ? {} : options, undefined,
    (r) => _idbMkRecord(_idbCloneKey(r.key), _idbCloneKey(r.pk), _idbClone(r.value, 'getAllRecords', 'IDBIndex')));
});
_idbMethod(IDBIndex, 'count', 0, function count(query) {
  const { os, srec, irec } = _idbIx(this, 'count');
  const range = _idbToRange(query, 'count', 'IDBIndex', false);
  const req = _idbNewReq(IDBRequest, this, os.tx);
  _idbQueue(req, () => _idbIdxRows(srec, irec).filter((r) => _idbInRange(range, r.key)).length);
  return req;
});
function _idbIdxCursor(self, meth, query, direction, keyOnly) {
  const { os, srec, irec } = _idbIx(self, meth);
  const range = _idbToRange(query, meth, 'IDBIndex', false);
  const dir = direction === undefined ? 'next' : _idbEnum(direction, _IDB_DIRS, 'IDBCursorDirection', "Failed to execute '" + meth + "' on 'IDBIndex': ");
  return _idbOpenCursor(self, os.tx, _idbList(_idbIdxRows(srec, irec), range, dir), dir, keyOnly, srec, true);
}
_idbMethod(IDBIndex, 'openCursor', 0, function openCursor(query, direction) { return _idbIdxCursor(this, 'openCursor', query, direction, false); });
_idbMethod(IDBIndex, 'openKeyCursor', 0, function openKeyCursor(query, direction) { return _idbIdxCursor(this, 'openKeyCursor', query, direction, true); });

// IDBCursor
_idbAttr(IDBCursor, 'source', function () { return _idbBrand(_Cu, this).source; });
_idbAttr(IDBCursor, 'direction', function () { return _idbBrand(_Cu, this).dir; });
_idbAttr(IDBCursor, 'key', function () { return _idbBrand(_Cu, this).key; });
_idbAttr(IDBCursor, 'primaryKey', function () { return _idbBrand(_Cu, this).pk; });
_idbAttr(IDBCursor, 'request', function () { return _idbBrand(_Cu, this).req; });
_idbAttr(IDBCursorWithValue, 'value', function () { return _idbBrand(_Cu, this).value; });
function _idbForward(cs, key, pkey, meth) {
  const rev = cs.dir === 'prev' || cs.dir === 'prevunique';
  if (key !== undefined) {
    if (!_idbValidKey(key)) throw _idbEx('DataError', meth, 'IDBCursor', _IDB_KEYERR);
    const c = _idbCmp(key, cs.key);
    if (cs.isIndex && pkey !== undefined) {
      const cp = c || _idbCmp(pkey, cs.pk);
      if (rev ? cp >= 0 : cp <= 0) throw _idbEx('DataError', meth, 'IDBCursor', "The parameter is less than or equal to this cursor's position.");
    } else if (rev ? c >= 0 : c <= 0) throw _idbEx('DataError', meth, 'IDBCursor', "The parameter is less than or equal to this cursor's position.");
  }
  let i = cs.pos + 1;
  if (key !== undefined) {
    for (; i < cs.list.length; i++) {
      const c = _idbCmp(cs.list[i].key, key);
      if (cs.isIndex && pkey !== undefined) {
        const cp = c || _idbCmp(cs.list[i].pk, pkey);
        if (rev ? cp <= 0 : cp >= 0) break;
      } else if (rev ? c <= 0 : c >= 0) break;
    }
  }
  return i;
}
_idbMethod(IDBCursor, 'advance', 1, function advance(count) {
  _idbArgc(arguments, 1, 'advance', 'IDBCursor');
  const cs = _idbCursorGate(this, 'advance');
  const n = Number(count) >>> 0;
  if (n === 0) throw new TypeError("Failed to execute 'advance' on 'IDBCursor': A count argument with value 0 (zero) was supplied, must be greater than 0.");
  if (!cs.got) throw _idbEx('InvalidStateError', 'advance', 'IDBCursor', _IDB_BUSY);
  cs.step(Math.min(cs.pos + n, cs.list.length));
});
_idbMethod(IDBCursor, 'continue', 0, function (key) {
  const cs = _idbCursorGate(this, 'continue');
  if (!cs.got) throw _idbEx('InvalidStateError', 'continue', 'IDBCursor', _IDB_BUSY);
  cs.step(_idbForward(cs, key, undefined, 'continue'));
});
_idbMethod(IDBCursor, 'continuePrimaryKey', 2, function continuePrimaryKey(key, primaryKey) {
  _idbArgc(arguments, 2, 'continuePrimaryKey', 'IDBCursor');
  const cs = _idbCursorGate(this, 'continuePrimaryKey');
  if (!cs.isIndex) throw _idbEx('InvalidAccessError', 'continuePrimaryKey', 'IDBCursor', 'The cursor\'s source is not an index.');
  if (cs.dir !== 'next' && cs.dir !== 'prev') throw _idbEx('InvalidAccessError', 'continuePrimaryKey', 'IDBCursor', 'The cursor\'s direction is not \'next\' or \'prev\'.');
  if (!cs.got) throw _idbEx('InvalidStateError', 'continuePrimaryKey', 'IDBCursor', _IDB_BUSY);
  if (!_idbValidKey(primaryKey)) throw _idbEx('DataError', 'continuePrimaryKey', 'IDBCursor', _IDB_KEYERR);
  cs.step(_idbForward(cs, key, primaryKey, 'continuePrimaryKey'));
});
_idbMethod(IDBCursor, 'update', 1, function update(value) {
  _idbArgc(arguments, 1, 'update', 'IDBCursor');
  const cs = _idbCursorGate(this, 'update');
  const t = _Tx.get(cs.tx);
  if (t.mode === 'readonly') throw _idbEx('ReadOnlyError', 'update', 'IDBCursor', 'The record may not be updated inside a read-only transaction.');
  if (cs.keyOnly) throw _idbEx('InvalidStateError', 'update', 'IDBCursor', 'The cursor is a key cursor.');
  if (!cs.got) throw _idbEx('InvalidStateError', 'update', 'IDBCursor', _IDB_BUSY);
  const clone = _idbClone(value, 'update', 'IDBCursor');
  if (cs.srec.keyPath !== null) {
    const k = _idbExtract(cs.srec.keyPath, clone);
    if (k === undefined || !_idbValidKey(k) || _idbCmp(k, cs.pk) !== 0) {
      throw _idbEx('DataError', 'update', 'IDBCursor', "The effective object store of this cursor uses in-line keys and evaluating the key path of the value parameter results in a different value than the cursor's effective key.");
    }
  }
  const req = _idbNewReq(IDBRequest, this, cs.tx);
  const pk = cs.pk;
  _idbQueue(req, () => {
    const enc = _idbEnc(pk);
    _idbCheckUnique(cs.srec, clone, enc);
    _idbSnap(t, cs.srec);
    cs.srec.data.set(enc, { key: _idbCloneKey(pk), value: clone });
    return _idbCloneKey(pk);
  });
  return req;
});
_idbMethod(IDBCursor, 'delete', 0, function () {
  const cs = _idbCursorGate(this, 'delete');
  const t = _Tx.get(cs.tx);
  if (t.mode === 'readonly') throw _idbEx('ReadOnlyError', 'delete', 'IDBCursor', 'The record may not be deleted inside a read-only transaction.');
  if (!cs.got) throw _idbEx('InvalidStateError', 'delete', 'IDBCursor', _IDB_BUSY);
  const req = _idbNewReq(IDBRequest, this, cs.tx);
  const pk = cs.pk;
  _idbQueue(req, () => { _idbSnap(t, cs.srec); cs.srec.data.delete(_idbEnc(pk)); return undefined; });
  return req;
});

// IDBRecord
_idbAttr(IDBRecord, 'key', function () { return _idbBrand(_Rc, this).key; });
_idbAttr(IDBRecord, 'primaryKey', function () { return _idbBrand(_Rc, this).pk; });
_idbAttr(IDBRecord, 'value', function () { return _idbBrand(_Rc, this).value; });

// IDBKeyRange
_idbAttr(IDBKeyRange, 'lower', function () { const s = _idbBrand(_Kr, this); return s.hasLo ? _idbCloneKey(s.lo) : undefined; });
_idbAttr(IDBKeyRange, 'upper', function () { const s = _idbBrand(_Kr, this); return s.hasHi ? _idbCloneKey(s.hi) : undefined; });
_idbAttr(IDBKeyRange, 'lowerOpen', function () { return _idbBrand(_Kr, this).loOpen; });
_idbAttr(IDBKeyRange, 'upperOpen', function () { return _idbBrand(_Kr, this).hiOpen; });
_idbMethod(IDBKeyRange, 'includes', 1, function includes(key) {
  _idbArgc(arguments, 1, 'includes', 'IDBKeyRange');
  _idbBrand(_Kr, this);
  if (!_idbValidKey(key)) throw _idbEx('DataError', 'includes', 'IDBKeyRange', _IDB_KEYERR);
  return _idbInRange(this, key);
});
function _idbStatic(name, len, fn) {
  Object.defineProperty(IDBKeyRange, name, { value: _idbPin(fn, name, len), writable: true, enumerable: true, configurable: true });
}
function _idbRk(k, meth) {
  if (!_idbValidKey(k)) throw _idbEx('DataError', meth, 'IDBKeyRange', _IDB_KEYERR);
  return _idbCloneKey(k);
}
_idbStatic('only', 1, function only(value) { _idbArgc(arguments, 1, 'only', 'IDBKeyRange'); const k = _idbRk(value, 'only'); return _idbNewRange(k, k, false, false, true, true); });
_idbStatic('lowerBound', 1, function lowerBound(lower, open) { _idbArgc(arguments, 1, 'lowerBound', 'IDBKeyRange'); return _idbNewRange(_idbRk(lower, 'lowerBound'), undefined, !!open, true, true, false); });
_idbStatic('upperBound', 1, function upperBound(upper, open) { _idbArgc(arguments, 1, 'upperBound', 'IDBKeyRange'); return _idbNewRange(undefined, _idbRk(upper, 'upperBound'), true, !!open, false, true); });
_idbStatic('bound', 2, function bound(lower, upper, lowerOpen, upperOpen) {
  _idbArgc(arguments, 2, 'bound', 'IDBKeyRange');
  const lo = _idbRk(lower, 'bound'), hi = _idbRk(upper, 'bound');
  const c = _idbCmp(lo, hi);
  if (c > 0) throw _idbEx('DataError', 'bound', 'IDBKeyRange', 'The lower key is greater than the upper key.');
  if (c === 0 && (lowerOpen || upperOpen)) throw _idbEx('DataError', 'bound', 'IDBKeyRange', 'The lower key and upper key are equal and one of the bounds is open.');
  return _idbNewRange(lo, hi, !!lowerOpen, !!upperOpen, true, true);
});

// IDBFactory
function _idbFactoryGate(self) { if (!_Fa.has(self)) throw new TypeError('Illegal invocation'); }
_idbMethod(IDBFactory, 'open', 1, function open(name, version) {
  _idbFactoryGate(this);
  _idbArgc(arguments, 1, 'open', 'IDBFactory');
  const nm = String(name);
  let target;
  if (version !== undefined) {
    const n = Math.trunc(Number(version));
    if (!(n >= 0) || n > 9007199254740991) throw new TypeError("Failed to execute 'open' on 'IDBFactory': Value is outside the 'unsigned long long' value range.");
    if (n === 0) throw new TypeError("Failed to execute 'open' on 'IDBFactory': The version provided must not be 0.");
    target = n;
  }
  const req = _idbNewReq(IDBOpenDBRequest, null, null);
  const rs = _Rq.get(req);
  Promise.resolve().then(() => {
    const rec = _idbRecord(nm);
    const current = rec.version;
    const tv = target === undefined ? (current || 1) : target;
    if (tv < current) {
      rs.done = true;
      rs.error = _idbErr('VersionError', 'The requested version (' + tv + ') is less than the existing version (' + current + ').');
      _idbFire(req, _idbErrEvent());
      return;
    }
    const db = _idbNew(IDBDatabase);
    const d = { rec, db, closed: false, vtx: null, h: Object.create(null) };
    _Db.set(db, d);
    const succeed = () => {
      // the new connection is live only once success fires
      if (!d.closed) (rec.conns || (rec.conns = [])).push(d);
      rs.done = true;
      rs.result = db; rs.tx = null; rs.error = null;
      _idbFire(req, new Event('success'));
    };
    const startUpgrade = () => {
      rs.done = true;
      const snapStores = new Map(rec.stores);
      const snapIdx = new Map();
      rec.stores.forEach((sr, n) => snapIdx.set(n, new Map(sr.indexes)));
      const vtx = _idbNewTx(rec, db, [], 'versionchange', 'default');
      const vt = _Tx.get(vtx);
      d.vtx = vtx;
      vt.undo.push(() => {
        rec.version = current;
        rec.stores = snapStores;
        snapIdx.forEach((m, n) => { const sr = snapStores.get(n); if (sr) sr.indexes = m; });
      });
      vt.onDone = (how) => {
        d.vtx = null;
        if (how === 'complete') { _idbPersistMeta(rec); succeed(); } else {
          d.closed = true;
          rs.result = undefined; rs.tx = null;
          rs.error = _idbErr('AbortError', 'The transaction was aborted.');
          _idbFire(req, _idbErrEvent());
          _idbDrain(rec); // a queued opener behind an aborted upgrade can run
        }
      };
      rs.result = db; rs.tx = vtx;
      rec.version = tv;
      // The native dispatch swallows a throwing handler (it only reports it);
      // the error tap records it so the upgrade aborts like Chrome's does.
      const tap = globalThis.__diting_etErrTap;
      if (tap) { tap.active++; tap.err = null; }
      _idbFire(req, new globalThis.IDBVersionChangeEvent('upgradeneeded', { oldVersion: current, newVersion: tv }));
      if (tap) {
        tap.active--;
        if (tap.err !== null) { tap.err = null; _idbAbort(vtx, _idbErr('AbortError', 'Version change transaction was aborted in upgradeneeded event handler.')); }
      }
    };
    if (tv > current) {
      // 刀86: a version-changing open first fires versionchange on every
      // other live connection; if any is still open after those handlers
      // ran, the request fires `blocked` and parks until the last close().
      const blockers = _idbLiveConns(rec);
      if (blockers.length) {
        for (const c of blockers) _idbFire(c.db, new globalThis.IDBVersionChangeEvent('versionchange', { oldVersion: current, newVersion: tv }));
        if (_idbLiveConns(rec).length) {
          _idbPark(rec, startUpgrade);
          _idbFire(req, new globalThis.IDBVersionChangeEvent('blocked', { oldVersion: current, newVersion: tv }));
          return;
        }
      }
      startUpgrade();
    } else {
      succeed();
    }
  });
  return req;
});
_idbMethod(IDBFactory, 'deleteDatabase', 1, function deleteDatabase(name) {
  _idbFactoryGate(this);
  _idbArgc(arguments, 1, 'deleteDatabase', 'IDBFactory');
  const nm = String(name);
  const req = _idbNewReq(IDBOpenDBRequest, null, null);
  const rs = _Rq.get(req);
  Promise.resolve().then(() => {
    const rec = _idbRecord(nm);
    const old = rec.version;
    const doDelete = () => {
      try { globalThis.localStorage.removeItem('__diting_idb__' + nm); } catch (e) {}
      _idbRegistryMap.delete(nm);
      rs.done = true; rs.result = undefined;
      _idbFire(req, new globalThis.IDBVersionChangeEvent('success', { oldVersion: old, newVersion: null }));
    };
    // 刀86: same versionchange→blocked park as open(); the delete fires it
    // with newVersion null.
    const blockers = _idbLiveConns(rec);
    if (blockers.length) {
      for (const c of blockers) _idbFire(c.db, new globalThis.IDBVersionChangeEvent('versionchange', { oldVersion: old, newVersion: null }));
      if (_idbLiveConns(rec).length) {
        _idbPark(rec, doDelete);
        _idbFire(req, new globalThis.IDBVersionChangeEvent('blocked', { oldVersion: old, newVersion: null }));
        return;
      }
    }
    doDelete();
  });
  return req;
});
_idbMethod(IDBFactory, 'databases', 0, function databases() {
  _idbFactoryGate(this);
  const out = [];
  try {
    const ls = globalThis.localStorage;
    for (let i = 0; i < ls.length; i++) {
      const k = ls.key(i);
      if (k && k.indexOf('__diting_idb__') === 0) {
        const nm = k.slice('__diting_idb__'.length);
        let v = 0;
        try { v = (JSON.parse(ls.getItem(k)).v) | 0; } catch (e) {}
        out.push({ name: nm, version: v });
      }
    }
  } catch (e) {}
  return Promise.resolve(out);
});
_idbMethod(IDBFactory, 'cmp', 2, function cmp(first, second) {
  _idbFactoryGate(this);
  _idbArgc(arguments, 2, 'cmp', 'IDBFactory');
  if (!_idbValidKey(first) || !_idbValidKey(second)) throw _idbEx('DataError', 'cmp', 'IDBFactory', _IDB_KEYERR);
  return _idbCmp(first, second);
});

globalThis.IDBRequest = IDBRequest;
globalThis.IDBOpenDBRequest = IDBOpenDBRequest;
globalThis.IDBDatabase = IDBDatabase;
globalThis.IDBTransaction = IDBTransaction;
globalThis.IDBObjectStore = IDBObjectStore;
globalThis.IDBIndex = IDBIndex;
globalThis.IDBCursor = IDBCursor;
globalThis.IDBCursorWithValue = IDBCursorWithValue;
globalThis.IDBRecord = IDBRecord;
globalThis.IDBFactory = IDBFactory;
globalThis.IDBKeyRange = IDBKeyRange;
{
  const factory = _idbNew(IDBFactory);
  _Fa.set(factory, true);
  globalThis.indexedDB = factory;
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
