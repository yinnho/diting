//! module: storage
//! layer: L2
//! exposes: localStorage, sessionStorage
//! requires-load: gen/storage
//! requires-call: legacy:_OPS, legacy:location, legacy:setTimeout

// Storage behavior only; the interface shape (classes, descriptors, the
// legacy-platform-object Proxy, conversions, error wording) is generated from
// WebIDL in gen/storage.gen.js. This file supplies the BACKING objects the
// generated createStorage() delegates to.
//
// localStorage loads its persisted entries on first access (one JSON file per
// origin, mirrored through the Rust-side global map - see op_storage_read) and
// flushes writes debounced; sessionStorage stays memory-only (per-tab
// lifetime is the spec'd behavior). Opaque origins (data: pages report "null")
// stay memory-only too. Loading must be lazy: the bootstrap runs before the
// document URL lands, so an eager read at construction would pin the origin
// to about:blank.
const gen = require('gen/storage');

function makeBacking(persistent) {
  const data = Object.create(null);
  let flushPending = false;
  let loadPending = persistent;

  const originOrNull = () => {
    const origin = location.origin;
    return (!origin || origin === 'null') ? null : origin;
  };
  const scheduleFlush = () => {
    if (!persistent || flushPending) return;
    flushPending = true;
    setTimeout(() => {
      flushPending = false;
      try {
        const origin = originOrNull();
        if (origin === null) return;
        _OPS.op_storage_write(origin, JSON.stringify(data));
      } catch (e) { }
    }, 100);
  };
  const ensureLoaded = () => {
    if (!loadPending) return;
    loadPending = false;
    try {
      const origin = originOrNull();
      if (origin === null) return;
      const entries = JSON.parse(_OPS.op_storage_read(origin));
      for (const k in entries) data[k] = entries[k];
    } catch (e) { }
  };

  return {
    get length() { ensureLoaded(); return Object.keys(data).length; },
    key(i) { ensureLoaded(); const ks = Object.keys(data); return i < ks.length ? ks[i] : null; },
    getItem(k) { ensureLoaded(); return (k in data) ? data[k] : null; },
    setItem(k, v) { ensureLoaded(); data[k] = v; scheduleFlush(); },
    removeItem(k) { ensureLoaded(); delete data[k]; scheduleFlush(); },
    clear() { ensureLoaded(); for (const k of Object.keys(data)) delete data[k]; scheduleFlush(); },
    supportedPropertyNames() { ensureLoaded(); return Object.keys(data); },
  };
}

P.exposeValue('localStorage', gen.createStorage(makeBacking(true)));
P.exposeValue('sessionStorage', gen.createStorage(makeBacking(false)));
