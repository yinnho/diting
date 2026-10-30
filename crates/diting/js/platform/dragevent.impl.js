//! module: dragevent
//! layer: L2
//! exposes: DragEvent
//! requires-load: legacy:MouseEvent
//! requires-call: -

// DragEvent (M4 knife 53): MouseEvent + `dataTransfer`. The generated stub ignored its arguments (`new DragEvent('x')` threw
// "1 argument required"), so no page could construct or dispatch one. `dataTransfer` is the init value or null (no DataTransfer
// object is synthesised for script-built events, like Chrome).

function DragEvent(type, init) {
  if (new.target === undefined) P.fail(DragEvent, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'DragEvent'");
  P.args("Failed to construct 'DragEvent'", arguments.length, 1, DragEvent);
  const e = Reflect.construct(MouseEvent, [P.conv.DOMString(type, "Failed to construct 'DragEvent'", DragEvent), init], new.target);
  slots.set(e, init !== undefined && init !== null && init.dataTransfer !== undefined ? init.dataTransfer : null);
  return e;
}
const DP = P.iface(DragEvent, 'DragEvent', 1, MouseEvent);
const slots = new WeakMap();
P.getter(DP, 'dataTransfer', function __f() { if (!slots.has(this)) P.illegal(__f); return slots.get(this); });
P.seal(DragEvent);
P.expose('DragEvent', DragEvent);
