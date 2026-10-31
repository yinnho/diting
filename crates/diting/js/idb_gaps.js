// 刀68: IDBVersionChangeEvent behavior takeover (the IDB family's one event
// class). indexeddb.js loads before the platform bundle defines Event, so the
// class cannot be built there; this file runs after event_hooks, replaces only
// the minted constructor and keeps the minted prototype object (its
// Symbol.toStringTag brand survives). Read the minted face through globalThis:
// the function declaration below hoists and would shadow the global name from
// the IIFE's first line (see videoframe_gaps.js for the same lesson).
(() => {
  const minted = globalThis.IDBVersionChangeEvent;
  if (typeof minted !== 'function' || !minted.prototype || typeof globalThis.Event !== 'function') return;
  const proto = minted.prototype;
  const store = new WeakMap();
  const u64 = (v, m) => {
    const n = Math.trunc(Number(v));
    if (!(n >= 0) || n > 18446744073709551615) throw new TypeError("Failed to construct 'IDBVersionChangeEvent': Failed to read the '" + m + "' property from 'IDBVersionChangeEventInit': Value is outside the 'unsigned long long' value range.");
    return n;
  };
  function IDBVersionChangeEvent(type, init) {
    if (!new.target) throw new TypeError("Failed to construct 'IDBVersionChangeEvent': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    if (arguments.length < 1) throw new TypeError("Failed to construct 'IDBVersionChangeEvent': 1 argument required, but only 0 present.");
    const ev = Reflect.construct(globalThis.Event, [type, init], new.target);
    let oldV = 0, newV = null;
    if (init !== null && init !== undefined && typeof init === 'object') {
      if (init.oldVersion !== undefined) oldV = u64(init.oldVersion, 'oldVersion');
      if (init.newVersion !== undefined && init.newVersion !== null) newV = u64(init.newVersion, 'newVersion');
    }
    store.set(ev, { oldV, newV });
    return ev;
  }
  IDBVersionChangeEvent.prototype = proto;
  Object.defineProperty(IDBVersionChangeEvent, 'length', { value: 1 });
  Object.defineProperty(IDBVersionChangeEvent, 'name', { value: 'IDBVersionChangeEvent' });
  Object.defineProperty(proto, 'constructor', { value: IDBVersionChangeEvent, writable: true, enumerable: false, configurable: true });
  Object.setPrototypeOf(IDBVersionChangeEvent, globalThis.Event);
  function guard() { const s = (this !== null && typeof this === 'object') ? store.get(this) : undefined; if (!s) throw new TypeError('Illegal invocation'); return s; }
  const attr = (k, read) => {
    const f = function () { return read(guard.call(this)); };
    Object.defineProperty(f, 'name', { value: 'get ' + k });
    Object.defineProperty(proto, k, { get: f, enumerable: true, configurable: true });
  };
  attr('oldVersion', (s) => s.oldV);
  attr('newVersion', (s) => s.newV);
  attr('dataLoss', () => 'none');
  attr('dataLossMessage', () => '');
  Object.defineProperty(globalThis, 'IDBVersionChangeEvent', { value: IDBVersionChangeEvent, writable: true, configurable: true });
})();
