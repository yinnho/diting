// Window members Chrome 145 has that no engine script installed. `sharedStorage` (the Shared Storage API entry point) is an
// accessor on the window returning one SharedStorage object (the interface itself is minted by stub_members.js).
(function () {
  const SS = globalThis.SharedStorage;
  if (typeof SS !== 'function' || Object.prototype.hasOwnProperty.call(globalThis, 'sharedStorage')) return;
  let inst = null;
  const get = function () { return inst || (inst = Object.create(SS.prototype)); };
  const mark = globalThis.__diting_markNative || ((f) => f);
  Object.defineProperty(get, 'name', { value: 'get sharedStorage', configurable: true });
  Object.defineProperty(globalThis, 'sharedStorage', { get: mark(get), set: undefined, enumerable: true, configurable: true });
})();

// NOTE (tried, reverted): Chrome's window has no OWN Symbol.toStringTag - it is inherited from Window.prototype, which is
// window's IMMEDIATE prototype. In the engine the global's immediate prototype is a different object than Window.prototype
// (Object.getPrototypeOf(window) !== Window.prototype, EventTarget is not in the chain: `window instanceof EventTarget` is false
// where Chrome's is true), so deleting the own tag turns Object.prototype.toString.call(window) into "[object Object]".
// Fixing that is a V8 global-template change (Rust side), not a script-layer one.

// Window prototype chain (shape-audit finding, M4 knife 37). The main window's chain was window -> gate(WindowProperties, named
// access) -> a bare `{constructor}` layer -> Object.prototype: no Window.prototype, no EventTarget.prototype, so
// `window instanceof EventTarget` was false (Chrome: true). Re-point the gate's raw target (bootstrap's `_namedHolder`, exposed as the hidden `__diting_namedHolder`) at the real
// Window.prototype, which chains to EventTarget.prototype (iframe_realm.js). The gate still sits ahead of everything, so named
// access resolution and the trio the holder carries are unchanged; the immediate prototype is still the gate, not Window.prototype
// (Chrome's order is window -> Window.prototype -> WindowProperties -> EventTarget), which needs a per-realm Window.prototype.
(function () {
  try {
    const holder = globalThis.__diting_namedHolder;
    if (typeof holder !== 'object' || holder === null || holder === globalThis) return;
    const W = globalThis.Window;
    if (typeof W !== 'function' || !W.prototype || W.prototype === holder) return;
    Object.setPrototypeOf(holder, W.prototype);
  } catch (e) {}
})();
