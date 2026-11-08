//! module: sensor
//! layer: L2
//! exposes: Sensor, Accelerometer, LinearAccelerationSensor, GravitySensor, Gyroscope, OrientationSensor, AbsoluteOrientationSensor, RelativeOrientationSensor, SensorErrorEvent
//! requires-load: legacy:Event, legacy:EventTarget, legacy:_eventState
//! requires-call: legacy:DOMException, legacy:DOMMatrix, legacy:setTimeout, legacy:clearTimeout

// Generic Sensor API family (M4 knife 9). Chrome 145 without sensor hardware
// (headless, desktop): every sensor constructs, start() asynchronously fires a
// trusted SensorErrorEvent (DOMException NotReadableError, 'Could not connect
// to a sensor'), nothing ever activates and no reading ever arrives - exactly
// the surface fingerprinting scripts and feature probes observe. Magnetometer
// and AmbientLightSensor do NOT exist in Chrome 145 and are deliberately not
// defined here. Every fact is pinned by chrome/sensor-truth-145.json
// (sensor-probe.js) and diffed by sensor_family_matches_chrome_truth.

const slots = new WeakMap();   // sensor -> { handlers, timer }
const evSlots = new WeakMap(); // SensorErrorEvent -> DOMException

// ---- options dictionaries: SensorOptions { frequency } and
// SpatialSensorOptions : SensorOptions { referenceFrame } (ancestor first)
function readOptions(name, fn, opt) {
  const pre = "Failed to construct '" + name + "'";
  if (opt === undefined || opt === null) return;
  if (typeof opt !== 'object' && typeof opt !== 'function') {
    P.fail(fn, "The provided value is not of type 'SpatialSensorOptions'.", pre);
  }
  const f = opt.frequency;
  if (f !== undefined) {
    const fpre = pre + ": Failed to read the 'frequency' property from 'SensorOptions'";
    if (typeof f === 'symbol') P.fail(fn, 'Cannot convert a Symbol value to a number', fpre);
    if (!isFinite(Number(f))) P.fail(fn, 'The provided double value is non-finite.', fpre);
  }
  const r = opt.referenceFrame;
  if (r !== undefined) {
    const s = P.conv.DOMString(r, pre + ": Failed to read the 'referenceFrame' property from 'SpatialSensorOptions'", fn);
    if (s !== 'device' && s !== 'screen') {
      P.fail(fn, "The provided value '" + s + "' is not a valid enum value of type LocalCoordinateSystem.",
        pre + ": Failed to read the 'referenceFrame' property from 'SpatialSensorOptions'");
    }
  }
}
function build(name, fn, args, nt) {
  if (nt === undefined) P.fail(fn, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct '" + name + "'");
  readOptions(name, fn, args[0]);
  const s = Reflect.construct(EventTarget, [], nt);
  slots.set(s, { handlers: { error: null, reading: null, activate: null }, timer: null });
  return s;
}
function slot(o, fn) {
  const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined;
  if (s === undefined) P.illegal(fn);
  return s;
}
// brand for the per-kind members: x/y/z only on the accelerometer / gyroscope
// kinds, quaternion/populateMatrix only on orientation sensors
const KIND = new WeakMap();
function kindOf(o, want, fn) {
  const s = slot(o, fn);
  const k = KIND.get(o);
  if (!want.includes(k)) P.illegal(fn);
  return s;
}

// ================= Sensor (abstract)
function Sensor() { P.fail(Sensor, 'Illegal constructor', "Failed to construct 'Sensor'"); }
const SP = P.iface(Sensor, 'Sensor', 0, EventTarget);
P.getter(SP, 'activated', function __f() { slot(this, __f); return false; });
P.getter(SP, 'hasReading', function __f() { slot(this, __f); return false; });
P.getter(SP, 'timestamp', function __f() { slot(this, __f); return null; });
for (const ev of ['error', 'reading', 'activate']) {
  P.accessor(SP, 'on' + ev,
    function __f() { return slot(this, __f).handlers[ev]; },
    function __f(v) { slot(this, __f).handlers[ev] = typeof v === 'function' ? v : null; });
}
// start(): with no sensor to connect, the failure is reported on a later task;
// a second start() while one is pending is a no-op, stop() cancels the pending
// failure (probe ev.twice / ev.stopNow), and a sensor can be started again
// after the error (ev.restart).
P.method(SP, 'start', 0, function __f() {
  const s = slot(this, __f);
  if (s.timer !== null) return;
  const self = this;
  s.timer = setTimeout(function () {
    s.timer = null;
    const e = new SensorErrorEvent('error', { error: new DOMException('Could not connect to a sensor', 'NotReadableError') });
    _eventState(e, 'isTrusted', true);
    self.dispatchEvent(e);
  }, 0);
});
P.method(SP, 'stop', 0, function __f() {
  const s = slot(this, __f);
  if (s.timer !== null) { clearTimeout(s.timer); s.timer = null; }
});
P.seal(Sensor);

// ================= concrete kinds
function xyz(C, name, kind) {
  const proto = C.prototype;
  for (const a of ['x', 'y', 'z']) {
    P.getter(proto, a, function __f() { kindOf(this, kind, __f); return null; });
  }
}
function mk(name, parent, kind) {
  const C = { [name]: function () { const s = build(name, C, arguments, new.target); KIND.set(s, kind); return s; } }[name];
  const proto = P.iface(C, name, 0, parent);
  void proto;
  return C;
}
const Accelerometer = mk('Accelerometer', Sensor, 'accel');
xyz(Accelerometer, 'Accelerometer', ['accel']);
P.seal(Accelerometer);
const LinearAccelerationSensor = mk('LinearAccelerationSensor', Accelerometer, 'accel');
P.seal(LinearAccelerationSensor);
const GravitySensor = mk('GravitySensor', Accelerometer, 'accel');
P.seal(GravitySensor);
const Gyroscope = mk('Gyroscope', Sensor, 'gyro');
xyz(Gyroscope, 'Gyroscope', ['gyro']);
P.seal(Gyroscope);

// ================= OrientationSensor (abstract) + Absolute/Relative
function OrientationSensor() { P.fail(OrientationSensor, 'Illegal constructor', "Failed to construct 'OrientationSensor'"); }
const OP = P.iface(OrientationSensor, 'OrientationSensor', 0, Sensor);
P.getter(OP, 'quaternion', function __f() { kindOf(this, ['orient'], __f); return null; });
P.method(OP, 'populateMatrix', 1, function __f() {
  const pre = "Failed to execute 'populateMatrix' on 'OrientationSensor'";
  kindOf(this, ['orient'], __f);
  if (arguments.length < 1) P.fail(__f, '1 argument required, but only 0 present.', pre);
  const m = arguments[0];
  const isArr = m instanceof Float32Array || m instanceof Float64Array;
  const isMat = typeof DOMMatrix === 'function' && m instanceof DOMMatrix;
  if (!isArr && !isMat) P.fail(__f, "The provided value is not of type '(DOMMatrix or Float32Array or Float64Array)'.", pre);
  if (isArr && m.length < 16) P.fail(__f, 'Target buffer must have at least 16 elements.', pre);
  throw new DOMException(pre + ': Sensor data is not available.', 'NotReadableError');
});
P.seal(OrientationSensor);
const AbsoluteOrientationSensor = mk('AbsoluteOrientationSensor', OrientationSensor, 'orient');
P.seal(AbsoluteOrientationSensor);
const RelativeOrientationSensor = mk('RelativeOrientationSensor', OrientationSensor, 'orient');
P.seal(RelativeOrientationSensor);

// ================= SensorErrorEvent(type, {error: DOMException} required)
function SensorErrorEvent() {
  const pre = "Failed to construct 'SensorErrorEvent'";
  if (new.target === undefined) P.fail(SensorErrorEvent, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
  if (arguments.length < 2) P.fail(SensorErrorEvent, '2 arguments required, but only ' + arguments.length + ' present.', pre);
  const type = P.conv.DOMString(arguments[0], pre, SensorErrorEvent);
  const init = arguments[1];
  if (init === null || init === undefined || (typeof init !== 'object' && typeof init !== 'function')) {
    P.fail(SensorErrorEvent, "The provided value is not of type 'SensorErrorEventInit'.", pre);
  }
  const ev = Reflect.construct(Event, [type, init], new.target);
  const epre = pre + ": Failed to read the 'error' property from 'SensorErrorEventInit'";
  const err = init.error;
  if (err === undefined) P.fail(SensorErrorEvent, 'Required member is undefined.', epre);
  if (!(err instanceof DOMException)) P.fail(SensorErrorEvent, "Failed to convert value to 'DOMException'.", epre);
  evSlots.set(ev, err);
  return ev;
}
const EP = P.iface(SensorErrorEvent, 'SensorErrorEvent', 2, Event);
P.getter(EP, 'error', function __f() {
  const e = evSlots.get(this);
  if (e === undefined) P.illegal(__f);
  return e;
});
P.seal(SensorErrorEvent);

for (const [n, C] of Object.entries({ Sensor, Accelerometer, LinearAccelerationSensor, GravitySensor, Gyroscope,
  OrientationSensor, AbsoluteOrientationSensor, RelativeOrientationSensor, SensorErrorEvent })) {
  P.expose(n, C);
}
