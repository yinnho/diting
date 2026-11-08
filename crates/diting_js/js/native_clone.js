// Structured-clone hooks for NATIVE interfaces (crates/diting_runtime/src/
// native). A native instance keeps its state in internal fields, which the JS
// structuredClone cannot see, so each interface registers a rebuild hook in
// the shared __diting_clone_hooks registry (keyed by @@toStringTag, consulted
// by _structuredClone before its generic Error/object paths). Constructors
// are captured now so a page overwriting a global cannot redirect the clone.
(() => {
  const hooks = (globalThis.__diting_clone_hooks = globalThis.__diting_clone_hooks || {});
  const reg = (name, rebuild) => {
    const C = globalThis[name];
    if (typeof C !== 'function') return;
    hooks[name] = function (src, seen) {
      if (seen && seen.has(src)) return seen.get(src);
      const copy = rebuild(C, src);
      if (seen) seen.set(src, copy);
      return copy;
    };
  };
  for (const n of ['DOMRect', 'DOMRectReadOnly']) reg(n, (C, s) => new C(s.x, s.y, s.width, s.height));
  for (const n of ['DOMPoint', 'DOMPointReadOnly']) reg(n, (C, s) => new C(s.x, s.y, s.z, s.w));
  // HTML structured-clone: DOMException serializes name + message (code
  // follows from the name); the clone carries no stack, like Chrome's.
  reg('DOMException', (C, s) => new C(s.message, s.name));
})();
