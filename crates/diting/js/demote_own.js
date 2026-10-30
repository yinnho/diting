// Singleton objects whose members the engine keeps as OWN properties of the instance (history.pushState, crypto.getRandomValues,
// navigator.geolocation.getCurrentPosition, ...). In Chrome these instances have no own properties at all: every member is a
// prototype property (methods and accessors). Each member is moved to the interface's prototype with the same function objects
// (so identity, `this` and closures are unchanged); own DATA properties become prototype accessors over a private slot, which keeps
// engine-side assignments (`history.length = n`) working. `window.crypto` etc. are read lazily, so a missing singleton is skipped.
(function () {
  const run = function () {
  const TARGETS = [
    ['history', () => globalThis.history, 'History'],
    ['crypto', () => globalThis.crypto, 'Crypto'],
    ['crypto.subtle', () => globalThis.crypto && globalThis.crypto.subtle, 'SubtleCrypto'],
    ['geolocation', () => globalThis.navigator && globalThis.navigator.geolocation, 'Geolocation'],
    ['serviceWorker', () => globalThis.navigator && globalThis.navigator.serviceWorker, 'ServiceWorkerContainer'],
    ['indexedDB', () => globalThis.indexedDB, 'IDBFactory'],
  ];
  const gopn = Object.getOwnPropertyNames, gopd = Object.getOwnPropertyDescriptor, def = Object.defineProperty;
  for (const [label, get, cn] of TARGETS) {
    try {
      const obj = get(), C = globalThis[cn];
      if (obj === null || typeof obj !== 'object' || typeof C !== 'function' || !C.prototype) continue;
      const P = C.prototype;
      if (Object.getPrototypeOf(obj) !== P) { try { Object.setPrototypeOf(obj, P); } catch (e) { continue; } }
      const slot = new WeakMap();
      for (const name of gopn(obj)) {
        if (name.charCodeAt(0) === 95 || Object.prototype.hasOwnProperty.call(P, name)) continue;
        // EventTarget trio: a custom per-object listener list may back these (ServiceWorkerContainer); moving them onto the
        // prototype would also trip the EventTarget-chain truth (the trio lives on EventTarget.prototype only)
        if (name === 'addEventListener' || name === 'removeEventListener' || name === 'dispatchEvent') continue;
        const d = gopd(obj, name);
        if (!d || !d.configurable) continue;
        if ('value' in d && typeof d.value !== 'function') {
          slot.set(obj, Object.assign(slot.get(obj) || Object.create(null), { [name]: d.value }));
          def(P, name, {
            get() { const s = slot.get(this); return s === undefined ? undefined : s[name]; },
            set(v) { let s = slot.get(this); if (s === undefined) { s = Object.create(null); slot.set(this, s); } s[name] = v; },
            enumerable: true, configurable: true,
          });
        } else {
          def(P, name, d);
        }
        delete obj[name];
      }
      // own well-known symbols the prototype already carries (toStringTag): the instance must not shadow them
      for (const sy of Object.getOwnPropertySymbols(obj)) {
        const d = gopd(obj, sy);
        if (d && d.configurable && Object.prototype.hasOwnProperty.call(P, sy)) delete obj[sy];
      }
    } catch (e) {}
  }
  };
  run();
  // singletons such as indexedDB / navigator.serviceWorker are created per page (after the snapshot): run again at the end of page init
  Object.defineProperty(globalThis, '__diting_demote_own', { value: run, writable: true, enumerable: false, configurable: true });
})();
