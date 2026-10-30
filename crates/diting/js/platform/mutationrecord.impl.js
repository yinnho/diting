//! module: mutationrecord
//! layer: L2
//! exposes: MutationRecord, __diting_mkMutationRecord
//! requires-load: -
//! requires-call: -

// MutationRecord (M4 knife 53). The observer delivered plain object literals ({type, target, addedNodes, ...}: [object Object], own
// enumerable data, no MutationRecord identity); Chrome's records are MutationRecord instances with read-only accessors on the
// prototype. Instances keep their data in a WeakMap slot; the hidden factory `__diting_mkMutationRecord(fields)` builds one.

function MutationRecord() { P.fail(MutationRecord, 'Illegal constructor', "Failed to construct 'MutationRecord'"); }
const MP = P.iface(MutationRecord, 'MutationRecord', 0);
const slots = new WeakMap();
const slotOf = (o, fn) => { const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined; if (s === undefined) P.illegal(fn); return s; };
for (const k of ['type', 'target', 'addedNodes', 'removedNodes', 'previousSibling', 'nextSibling', 'attributeName', 'attributeNamespace', 'oldValue']) {
  P.getter(MP, k, function __f() { return slotOf(this, __f)[k]; });
}
P.seal(MutationRecord);

function make(f) {
  const o = Object.create(MP);
  slots.set(o, { type: f.type, target: f.target, addedNodes: f.addedNodes, removedNodes: f.removedNodes, previousSibling: f.previousSibling,
    nextSibling: f.nextSibling, attributeName: f.attributeName, attributeNamespace: null, oldValue: f.oldValue });
  return o;
}
P.expose('MutationRecord', MutationRecord);
P.exposeValue('__diting_mkMutationRecord', make);
