// Generic Sensor family probe (M4 knife 9): Sensor / Accelerometer /
// LinearAccelerationSensor / GravitySensor / Gyroscope / OrientationSensor /
// AbsoluteOrientationSensor / RelativeOrientationSensor / SensorErrorEvent.
// Chrome 145 (headless, no sensor hardware): every sensor constructs, start()
// asynchronously fires a trusted SensorErrorEvent (NotReadableError, 'Could not
// connect to a sensor'), nothing ever activates. Runs in Chrome AND the engine.
(async function () {
  var R = {}, names = ['Sensor', 'Accelerometer', 'LinearAccelerationSensor', 'GravitySensor', 'Gyroscope', 'OrientationSensor', 'AbsoluteOrientationSensor', 'RelativeOrientationSensor', 'Magnetometer', 'AmbientLightSensor', 'SensorErrorEvent'];
  function t(n, f) { try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 260); } }
  R.exist = {}; names.forEach(function (n) { R.exist[n] = typeof window[n]; });
  R.shape = {};
  names.forEach(function (n) {
    var C = window[n]; if (typeof C !== 'function') return;
    var P = C.prototype, k = {};
    Object.getOwnPropertyNames(P).forEach(function (m) { var d = Object.getOwnPropertyDescriptor(P, m); k[m] = (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; });
    R.shape[n] = { order: Object.getOwnPropertyNames(P), kinds: k, parent: Object.getPrototypeOf(C).name, len: C.length, statics: Object.getOwnPropertyNames(C), tag: Object.prototype.toString.call(Object.create(P)) };
  });
  t('ctor.sensor', function () { return new Sensor(); });
  t('ctor.orient', function () { return new OrientationSensor(); });
  t('ctor.call', function () { return Accelerometer(); });
  t('ctor.errevent.call', function () { return SensorErrorEvent('error', { error: new DOMException('x') }); });
  t('instance', function () { return ['Accelerometer', 'LinearAccelerationSensor', 'GravitySensor', 'Gyroscope', 'AbsoluteOrientationSensor', 'RelativeOrientationSensor'].map(function (n) { var a = new window[n](); return [n, a.activated, a.hasReading, a.timestamp, a.x, a.y, a.z, a.quaternion, Object.prototype.toString.call(a), Object.getOwnPropertyNames(a).length, a instanceof Sensor, a instanceof EventTarget, a.onreading, a.onerror, a.onactivate]; }); });
  t('chain', function () { return [new GravitySensor() instanceof Accelerometer, new LinearAccelerationSensor() instanceof Accelerometer, new AbsoluteOrientationSensor() instanceof OrientationSensor, new Gyroscope() instanceof Accelerometer]; });
  // options
  ['Accelerometer', 'GravitySensor', 'Gyroscope', 'RelativeOrientationSensor'].forEach(function (n) {
    t('badref.' + n, function () { new window[n]({ referenceFrame: 'zzz' }); });
    t('badopt.' + n, function () { new window[n](5); });
  });
  t('freq.conv', function () { var o = {}; [['str', '30'], ['nan', NaN], ['inf', Infinity], ['neg', -1], ['zero', 0], ['null', null], ['obj', {}], ['big', 1e9], ['sym', Symbol()]].forEach(function (c) { try { new Accelerometer({ frequency: c[1] }); o[c[0]] = 'ok'; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('ref.vals', function () { var o = {}; ['device', 'screen', 'DEVICE', ''].forEach(function (v) { try { new Gyroscope({ referenceFrame: v }); o[v] = 'ok'; } catch (e) { o[v] = 'T'; } }); return o; });
  t('opts.nullish', function () { new Accelerometer(null); new Accelerometer(undefined); new Accelerometer(function () {}); return 'ok'; });
  t('order.freq-first', function () { var log = []; try { new Accelerometer({ get referenceFrame() { log.push('ref'); return 'zzz'; }, get frequency() { log.push('freq'); return 1; } }); } catch (e) { log.push('throw'); } return log; });
  // handlers
  t('handlers', function () { var a = new Accelerometer(), f = function () {}; a.onreading = f; a.onerror = 5; a.onactivate = f; return [a.onreading === f, a.onerror, a.onactivate === f, Object.prototype.hasOwnProperty.call(a, 'onreading')]; });
  // brand checks
  t('brand.get', function () { return Object.getOwnPropertyDescriptor(Sensor.prototype, 'activated').get.call({}); });
  t('brand.start', function () { return Sensor.prototype.start.call({}); });
  t('brand.stop', function () { return Sensor.prototype.stop.call({}); });
  t('brand.xyz', function () { return Object.getOwnPropertyDescriptor(Accelerometer.prototype, 'x').get.call(new Gyroscope()); });
  t('brand.quat', function () { return Object.getOwnPropertyDescriptor(OrientationSensor.prototype, 'quaternion').get.call(new Accelerometer()); });
  // populateMatrix
  t('populate.noargs', function () { return new AbsoluteOrientationSensor().populateMatrix(); });
  t('populate.types', function () { var a = new RelativeOrientationSensor(), o = {}; [['f32', new Float32Array(16)], ['f64', new Float64Array(16)], ['dm', new DOMMatrix()], ['f32s', new Float32Array(4)], ['f32big', new Float32Array(32)], ['arr', []], ['i32', new Int32Array(16)], ['obj', {}], ['null', null]].forEach(function (c) { try { a.populateMatrix(c[1]); o[c[0]] = 'ok'; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('stop.noop', function () { return new Accelerometer().stop(); });
  // SensorErrorEvent
  t('errevent.init', function () { var d = new DOMException('boo', 'NotReadableError'); var e = new SensorErrorEvent('error', { error: d, bubbles: true, cancelable: true }); return [e.error === d, e.bubbles, e.cancelable, e.constructor.name, Object.getPrototypeOf(SensorErrorEvent) === Event, e.isTrusted]; });
  t('errevent.noerr', function () { return new SensorErrorEvent('error'); });
  t('errevent.bad', function () { return new SensorErrorEvent('error', { error: 5 }); });
  t('errevent.badinit', function () { return new SensorErrorEvent('error', 5); });
  t('errevent.empty', function () { return new SensorErrorEvent('error', {}); });
  t('errevent.brand', function () { return Object.getOwnPropertyDescriptor(SensorErrorEvent.prototype, 'error').get.call({}); });
  t('presence', function () { return ['Accelerometer', 'Magnetometer', 'AmbientLightSensor', 'LinearAccelerationSensor', 'GravitySensor', 'OrientationSensor', 'Sensor', 'SensorErrorEvent', 'UncalibratedMagnetometer', 'ProximitySensor'].map(function (n) { return n in window; }); });
  // start()/stop() event sequences. Handler (on*) fires are logged separately:
  // the engine core fires IDL handlers before listeners (a known core-level
  // divergence), so the two orders are not compared against each other.
  async function ev(label, ctorName, fn) {
    var s = new window[ctorName](), log = [], hlog = [];
    ['reading', 'activate', 'error'].forEach(function (n) { s.addEventListener(n, function (e) { log.push(n + ':' + e.bubbles + ':' + e.cancelable + ':' + (e.error && e.error.constructor.name) + ':' + (e.error && e.error.name) + ':' + (e.error && e.error.message) + ':' + (e.error && e.error.code) + ':' + e.isTrusted + ':' + (e.target === s) + ':' + e.constructor.name); }); });
    s.onerror = function () { hlog.push('onerror'); };
    fn(s, log);
    await new Promise(function (res) { setTimeout(res, 300); });
    log.push('end: activated=' + s.activated + ' hasReading=' + s.hasReading + ' x=' + s.x);
    R[label] = { log: log, hlog: hlog };
  }
  await ev('ev.start', 'Accelerometer', function (s, log) { log.push('start->' + s.start()); log.push('sync activated=' + s.activated); });
  await ev('ev.twice', 'Gyroscope', function (s) { s.start(); s.start(); });
  await ev('ev.stopNow', 'LinearAccelerationSensor', function (s) { s.start(); s.stop(); });
  await ev('ev.restart', 'GravitySensor', function (s) { s.start(); setTimeout(function () { s.start(); }, 120); });
  await ev('ev.none', 'AbsoluteOrientationSensor', function () {});
  await ev('ev.abs', 'AbsoluteOrientationSensor', function (s) { s.start(); });
  await ev('ev.rel', 'RelativeOrientationSensor', function (s) { s.start(); });
  return JSON.stringify(R);
})()
