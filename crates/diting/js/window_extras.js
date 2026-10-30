// Window members Chrome 145 has that no engine script installed. `sharedStorage` (the Shared Storage API entry point) is an
// accessor on the window returning one SharedStorage object (the interface itself is minted by stub_members.js).
(function () {
  const SS = globalThis.SharedStorage;
  if (typeof SS !== 'function' || Object.prototype.hasOwnProperty.call(globalThis, 'sharedStorage')) return;
  let inst = null;
  const get = function () { return inst || (inst = Object.create(SS.prototype)); };
  const mark = typeof _markNative === 'function' ? _markNative : (f) => f;
  Object.defineProperty(get, 'name', { value: 'get sharedStorage', configurable: true });
  Object.defineProperty(globalThis, 'sharedStorage', { get: mark(get), set: undefined, enumerable: true, configurable: true });
})();

// NOTE (tried, reverted): Chrome's window has no OWN Symbol.toStringTag - it is inherited from Window.prototype, which is
// window's IMMEDIATE prototype. In the engine the global's immediate prototype is a different object than Window.prototype
// (Object.getPrototypeOf(window) !== Window.prototype, EventTarget is not in the chain: `window instanceof EventTarget` is false
// where Chrome's is true), so deleting the own tag turns Object.prototype.toString.call(window) into "[object Object]".
// Fixing that is a V8 global-template change (Rust side), not a script-layer one.
