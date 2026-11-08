// 刀71: `interestForElement` (HTMLAnchorElement / HTMLAreaElement / HTMLButtonElement /
// SVGAElement) — an Element-reference reflection of the `interestfor` attribute.
// Chrome 145 truth (k71): getter null without the attribute; with it, the explicitly
// assigned element while the attribute is still the '' the setter wrote, otherwise
// the element whose id equals the attribute value in the receiver's tree (null when
// none); setter Element -> remember it and set the attribute to '', null/undefined ->
// remove the attribute, anything else TypeError "Failed to convert value to 'Element'.".
// Foreign receivers throw Illegal invocation. Runs after the platform bundle so the
// four interface prototypes exist.
(() => {
  const assoc = new WeakMap();
  const nm = (f, n) => { Object.defineProperty(f, 'name', { value: n }); return f; };
  const mark = globalThis.__diting_markNative;
  for (const iname of ['HTMLAnchorElement', 'HTMLAreaElement', 'HTMLButtonElement', 'SVGAElement']) {
    const C = globalThis[iname];
    if (typeof C !== 'function' || !C.prototype || Object.prototype.hasOwnProperty.call(C.prototype, 'interestForElement')) continue;
    const ok = (self) => { if (!(self instanceof C)) throw new TypeError('Illegal invocation'); };
    const get = nm(function () {
      ok(this);
      const v = this.getAttribute('interestfor');
      if (v === null) return null;
      const a = assoc.get(this);
      if (v === '' && a) return a;
      const root = typeof this.getRootNode === 'function' ? this.getRootNode() : null;
      if (v === '' || !root || typeof root.getElementById !== 'function') return null;
      return root.getElementById(v) || null;
    }, 'get interestForElement');
    const set = nm(function (v) {
      ok(this);
      if (v === null || v === undefined) { assoc.delete(this); this.removeAttribute('interestfor'); return; }
      if (!(v instanceof Element)) throw new TypeError("Failed to set the 'interestForElement' property on '" + iname + "': Failed to convert value to 'Element'.");
      assoc.set(this, v);
      this.setAttribute('interestfor', '');
    }, 'set interestForElement');
    if (typeof mark === 'function') { mark(get); mark(set); }
    Object.defineProperty(C.prototype, 'interestForElement', { get, set, enumerable: true, configurable: true });
  }
})();
