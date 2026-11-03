//! module: ce_attr
//! layer: L2
//! exposes: __diting_ceCapture, __diting_ceAttrChanged, __diting_ceUpgradeReactions
//! requires-load: -
//! requires-call: legacy:customElements

// Custom-element attribute reactions (2026-11-03). The registry in bootstrap.js
// never called attributeChangedCallback: not on upgrade, not for parser-created
// elements, not on setAttribute. micro-app reads its app name and url through
// it, so <micro-app name url> never started and the WeChat store goods pages
// stayed blank. The machinery lives here; bootstrap keeps thin call-ins in the
// same shape as __diting_cloneAttrNs (attributes): define() captures callback +
// observedAttributes ONCE via __diting_ceCapture (HTML "define" steps - a
// callback patched onto the prototype later is never called, a getter or plain
// static field both count, errors reading them abort the define), _upgradeElement
// fires one reaction per observed attribute already present via
// __diting_ceUpgradeReactions (before connectedCallback), and the four Element
// attribute writers report every write via __diting_ceAttrChanged (Chrome fires
// on every set, same value included, and not for removing an absent attribute -
// the callers enforce the latter). Classes land in the map only when they
// observe at least one name, so attribute writes on everything else pay one
// map miss.

const defs = new Map();  // class -> { acc, observed:Set }

P.expose('__diting_ceCapture', function (cls) {
  const acc = cls.prototype.attributeChangedCallback;
  if (acc !== undefined && typeof acc !== 'function') throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': The \"attributeChangedCallback\" property of the provided constructor's prototype is not a function.");
  let observed = null;
  if (acc !== undefined) {
    const oa = cls.observedAttributes;
    if (oa !== undefined) observed = new Set(Array.from(oa, String));
  }
  if (observed && observed.size) defs.set(cls, { acc, observed });
});

P.expose('__diting_ceAttrChanged', function (el, name, oldValue, newValue, ns) {
  if (!el.__customUpgraded || !defs.size) return;
  const ce = globalThis.customElements;
  const cls = ce && ce._registry ? ce._registry.get(el.localName) : null;
  const def = cls && Object.prototype.isPrototypeOf.call(cls.prototype, el) ? defs.get(cls) : null;
  if (!def || !def.observed.has(name)) return;
  try { def.acc.call(el, name, oldValue, newValue, ns == null || ns === '' ? null : String(ns)); } catch (e) {}
});

P.expose('__diting_ceUpgradeReactions', function (el, cls) {
  const def = defs.get(cls);
  if (!def) return;
  for (const at of Array.from(el.attributes)) {
    const n = at.localName || at.name;
    if (def.observed.has(n)) { try { def.acc.call(el, n, null, at.value, at.namespaceURI || null); } catch (e) {} }
  }
});

if (globalThis.__diting_hide_list && typeof globalThis.__diting_hide_list.push === 'function') {
  globalThis.__diting_hide_list.push('__diting_ceCapture', '__diting_ceAttrChanged', '__diting_ceUpgradeReactions');
}
