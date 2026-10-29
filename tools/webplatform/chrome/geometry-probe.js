// DOMPoint/DOMRect/DOMQuad/DOMMatrix family probe (JS-PLATFORM-ARCH M4,
// geometry knife). Runs in real Chrome AND in the diting engine; results are
// diffed. Deterministic math only. Returns a JSON string.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + (e && e.name && e.name !== (e.constructor && e.constructor.name) ? '(' + e.name + ')' : '') + ': ' + (e && e.message); } }
  function shape(C, inst) {
    var P = C.prototype, o = {};
    // descriptor kinds, NOT value reads: native accessor getters throw
    // Illegal invocation when the receiver is the prototype object itself.
    Object.getOwnPropertyNames(P).sort().forEach(function (k) {
      var d = Object.getOwnPropertyDescriptor(P, k);
      o[k] = d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value;
    });
    var st = {};
    Object.getOwnPropertyNames(C).sort().forEach(function (k) {
      if (k === 'prototype' || k === 'length' || k === 'name') return;
      var d = Object.getOwnPropertyDescriptor(C, k);
      st[k] = (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value) + (d.enumerable ? '+e' : '');
    });
    return { len: C.length, name: C.name, members: o, statics: st,
      order: Object.getOwnPropertyNames(P),  // unsorted: Chrome's own-key install order
      parent: Object.getPrototypeOf(P) === Object.prototype ? 'Object' : (Object.getPrototypeOf(P).constructor || {}).name,
      tag: Object.prototype.toString.call(inst) };
  }
  var I2 = [1, 2, 3, 4, 5, 6];
  var I16 = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
  // ---- shapes
  ['DOMPointReadOnly', 'DOMPoint', 'DOMRectReadOnly', 'DOMRect', 'DOMQuad', 'DOMMatrixReadOnly', 'DOMMatrix'].forEach(function (n) {
    t('shape.' + n, function () { var C = self[n]; return shape(C, n === 'DOMPoint' ? new DOMPoint() : n === 'DOMPointReadOnly' ? new DOMPointReadOnly() : n === 'DOMRect' ? new DOMRect() : n === 'DOMRectReadOnly' ? new DOMRectReadOnly() : n === 'DOMQuad' ? new DOMQuad() : n === 'DOMMatrixReadOnly' ? new DOMMatrixReadOnly() : new DOMMatrix()); });
  });
  // ---- constructors
  t('dm.identity', function () { var m = new DOMMatrix(); return [m.a, m.b, m.c, m.d, m.e, m.f, m.is2D, m.isIdentity]; });
  t('dm.ctor2d', function () { var m = new DOMMatrix(I2); return [m.a, m.b, m.c, m.d, m.e, m.f, m.is2D, m.isIdentity]; });
  t('dm.ctor3d', function () { var m = new DOMMatrix(I16); return [m.m11, m.m12, m.m21, m.m22, m.m41, m.m44, m.is2D]; });
  t('dm.ctor2dMap', function () { var m = new DOMMatrix(I2); return [m.a === m.m11, m.b === m.m12, m.c === m.m21, m.d === m.m22, m.e === m.m41, m.f === m.m42]; });
  t('dm.ctor.badlen', function () { return new DOMMatrix([1, 2, 3]); });
  t('dm.ctor.nan', function () { var m = new DOMMatrix([1, NaN, 3, 4, 5, 6]); return [m.a, m.b, m.isIdentity]; });
  t('dm.noargs.call', function () { return DOMMatrix(); });
  t('dmro.ctor', function () { var m = new DOMMatrixReadOnly(I2); return [m.a, m.f, m.is2D, m.isIdentity]; });
  t('dm.instanceofRO', function () { return [new DOMMatrix() instanceof DOMMatrixReadOnly, new DOMMatrixReadOnly() instanceof DOMMatrix]; });
  // ---- statics
  t('dm.fromMatrix', function () { var m = DOMMatrix.fromMatrix(new DOMMatrix(I2)); return [m.a, m.f, m.is2D]; });
  t('dm.fromMatrix3d', function () { var s = new DOMMatrix(I16); var m = DOMMatrix.fromMatrix(s); return [m.m11, m.is2D]; });
  t('dm.fromFloat32Array', function () { var m = DOMMatrix.fromFloat32Array(new Float32Array(I2)); return [m.a, m.f]; });
  t('dm.fromFloat64Array', function () { var m = DOMMatrix.fromFloat64Array(new Float64Array(I16)); return [m.m11, m.is2D]; });
  t('dm.fromFloat32Array.badlen', function () { return DOMMatrix.fromFloat32Array(new Float32Array(5)); });
  t('dm.fromFloat64Array.badlen', function () { return DOMMatrix.fromFloat64Array(new Float64Array(7)); });
  // ---- immutable ops (ReadOnly)
  t('ro.multiply', function () { var m = new DOMMatrixReadOnly([1, 0, 0, 1, 10, 20]).multiply(new DOMMatrix([2, 0, 0, 2, 0, 0])); return [m.a, m.d, m.e, m.f]; });
  t('ro.translate', function () { var m = new DOMMatrixReadOnly().translate(10, 20, 30); return [m.e, m.f, m.m43, m.is2D]; });
  t('ro.scale', function () { var m = new DOMMatrixReadOnly().scale(2, 3); return [m.a, m.d, m.e, m.f]; });
  t('ro.scale3d', function () { var m = new DOMMatrixReadOnly().scale3d(2); return [m.m11, m.m33, m.is2D]; });
  t('ro.rotate', function () { var m = new DOMMatrixReadOnly().rotate(90); return [m.a, m.b, m.c, m.d]; });
  t('ro.rotate180', function () { var m = new DOMMatrixReadOnly().rotate(180); return [m.a, m.b, m.c, m.d]; });
  t('ro.rotateFromVector', function () { var m = new DOMMatrixReadOnly().rotateFromVector(3, 4); return [m.a, m.b, m.c, m.d]; });
  t('ro.rotateAxisAngle', function () { var m = new DOMMatrixReadOnly().rotateAxisAngle(0, 0, 1, 90); return [m.a, m.b, m.c, m.d, m.is2D]; });
  t('ro.skewX', function () { var m = new DOMMatrixReadOnly().skewX(45); return [m.c]; });
  t('ro.skewY', function () { var m = new DOMMatrixReadOnly().skewY(45); return [m.b]; });
  t('ro.flipX', function () { var m = new DOMMatrixReadOnly([1, 2, 3, 4, 5, 6]).flipX(); return [m.a, m.b, m.c, m.d, m.e, m.f]; });
  t('ro.flipY', function () { var m = new DOMMatrixReadOnly([1, 2, 3, 4, 5, 6]).flipY(); return [m.a, m.b, m.c, m.d, m.e, m.f]; });
  t('ro.inverse', function () { var m = new DOMMatrixReadOnly([2, 0, 0, 2, 10, 20]).inverse(); return [m.a, m.d, m.e, m.f]; });
  t('ro.inverse.singular', function () { var m = new DOMMatrixReadOnly([0, 0, 0, 0, 0, 0]).inverse(); return [m.a, m.b, m.is2D]; });
  t('ro.transformPoint', function () { var p = new DOMMatrixReadOnly([2, 0, 0, 2, 10, 20]).transformPoint(new DOMPoint(5, 5, 5, 1)); return [p.x, p.y, p.z, p.w]; });
  t('ro.transformPoint.w0', function () { var p = new DOMMatrixReadOnly().translate(10, 0).transformPoint(new DOMPoint(1, 0, 0, 0)); return [p.x, p.y, p.w]; });
  t('ro.toFloat64Array', function () { var m = new DOMMatrixReadOnly(I2); return Array.from(m.toFloat64Array()); });
  t('ro.toFloat32Array', function () { var m = new DOMMatrixReadOnly(I16); return [m.toFloat32Array() instanceof Float32Array, m.toFloat32Array().length]; });
  t('ro.toJSON', function () { return new DOMMatrixReadOnly(I2).toJSON(); });
  // ---- mutable ops (Self family)
  t('dm.multiplySelf', function () { var m = new DOMMatrix([1, 0, 0, 1, 10, 20]); m.multiplySelf(new DOMMatrix([2, 0, 0, 2, 0, 0])); return [m.a, m.d, m.e, m.f]; });
  t('dm.translateSelf', function () { var m = new DOMMatrix(); m.translateSelf(10, 20, 30); return [m.e, m.f, m.m43, m.is2D]; });
  t('dm.scaleSelf', function () { var m = new DOMMatrix(); m.scaleSelf(2, 3); return [m.a, m.d]; });
  t('dm.rotateSelf', function () { var m = new DOMMatrix(); m.rotateSelf(90); return [m.a, m.b, m.c, m.d]; });
  t('dm.rotateAxisAngleSelf', function () { var m = new DOMMatrix(); m.rotateAxisAngleSelf(0, 1, 0, 90); return [m.is2D, m.m11.toFixed(6), m.m13.toFixed(6)]; });
  t('dm.skewXSelf', function () { var m = new DOMMatrix(); m.skewXSelf(45); return [m.c.toFixed(9)]; });
  t('dm.invertSelf', function () { var m = new DOMMatrix([2, 0, 0, 2, 10, 20]); m.invertSelf(); return [m.a, m.d, m.e, m.f]; });
  t('dm.invertSelf.singular', function () { var m = new DOMMatrix([0, 0, 0, 0, 0, 0]); m.invertSelf(); return [m.a, m.b, m.c, m.d, m.e, m.f, m.is2D]; });
  t('dm.preMultiplySelf', function () { var m = new DOMMatrix([1, 0, 0, 1, 10, 20]); m.preMultiplySelf(new DOMMatrix([2, 0, 0, 2, 0, 0])); return [m.a, m.d, m.e, m.f]; });
  t('dm.setMatrixValue', function () { var m = new DOMMatrix(); m.setMatrixValue('translate(10px, 20px) scale(2)'); return [m.a, m.d, m.e, m.f, m.is2D]; });
  t('dm.setMatrixValue.3d', function () { var m = new DOMMatrix(); m.setMatrixValue('matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1)'); return [m.e, m.f, m.m43, m.is2D]; });
  t('dm.setMatrixValue.rotate', function () { var m = new DOMMatrix(); m.setMatrixValue('rotate(90deg)'); return [m.a, m.b, m.c, m.d]; });
  t('dm.setMatrixValue.bad', function () { var m = new DOMMatrix(); return m.setMatrixValue('garbage('); });
  t('dm.setMatrixValue.ret', function () { var m = new DOMMatrix(); return m.setMatrixValue('scale(3)') === m; });
  // ---- DOMPoint
  t('dp.ctor', function () { var p = new DOMPoint(1, 2, 3, 4); return [p.x, p.y, p.z, p.w]; });
  t('dp.defaults', function () { var p = new DOMPoint(); return [p.x, p.y, p.z, p.w]; });
  t('dp.fromPoint', function () { var p = DOMPoint.fromPoint({ x: 1, y: 2, z: 3, w: 4 }); return [p.x, p.w]; });
  t('dp.fromPoint.empty', function () { var p = DOMPoint.fromPoint({}); return [p.x, p.y, p.z, p.w]; });
  t('dpro.fromPoint', function () { var p = DOMPointReadOnly.fromPoint(new DOMPoint(1, 2, 3, 4)); return [p.x, p.w, p.constructor.name]; });
  t('dp.instanceofRO', function () { return new DOMPoint() instanceof DOMPointReadOnly; });
  t('dp.toJSON', function () { return new DOMPointReadOnly(1, 2, 3, 4).toJSON(); });
  // ---- DOMRect
  t('dr.ctor', function () { var r = new DOMRect(1, 2, 3, 4); return [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]; });
  t('dr.neg', function () { var r = new DOMRect(0, 0, -3, -4); return [r.top, r.right, r.bottom, r.left]; });
  t('dr.fromRect', function () { var r = DOMRect.fromRect({ x: 1, y: 2, width: 3, height: 4 }); return [r.x, r.width]; });
  t('dr.fromRect.empty', function () { var r = DOMRect.fromRect({}); return [r.x, r.y, r.width, r.height]; });
  t('dr.toJSON', function () { return new DOMRect(1, 2, 3, 4).toJSON(); });
  t('dr.topIsAccessor', function () { var d = Object.getOwnPropertyDescriptor(DOMRect.prototype, 'top'); return !!d && !('value' in d); });
  t('dr.instanceofRO', function () { return new DOMRect() instanceof DOMRectReadOnly; });
  // ---- DOMQuad
  t('dq.ctor', function () { var q = new DOMQuad(new DOMPoint(0, 0), new DOMPoint(4, 0), new DOMPoint(4, 4), new DOMPoint(0, 4)); return [q.p1.x, q.p3.y, q.p1 instanceof DOMPoint]; });
  t('dq.fromRect', function () { var q = DOMQuad.fromRect(new DOMRect(1, 2, 3, 4)); return [q.p1.x, q.p1.y, q.p2.x, q.p4.y]; });
  t('dq.fromRect.empty', function () { var q = DOMQuad.fromRect({}); return [q.p1.x, q.p1.y, q.p2.x, q.p3.y]; });
  t('dq.bounds', function () { var q = new DOMQuad(new DOMPoint(0, 4), new DOMPoint(4, 0), new DOMPoint(2, 6), new DOMPoint(6, 2)); var b = q.bounds; return [b.x, b.y, b.width, b.height, b.constructor.name]; });
  t('dq.fromQuad', function () { var q1 = DOMQuad.fromRect({ x: 0, y: 0, width: 2, height: 2 }); var q2 = DOMQuad.fromQuad(q1); return [q2.p2.x, q2.p2.y]; });
  t('dq.toJSON', function () { var q = DOMQuad.fromRect({ x: 0, y: 0, width: 1, height: 1 }); var j = q.toJSON(); return [j.p1.x, j.p3.y, Object.keys(j).join(',')]; });
  // ---- WebKitCSSMatrix
  t('wkcm.instanceof', function () { return [new WebKitCSSMatrix() instanceof DOMMatrix, typeof WebKitCSSMatrix.prototype.setMatrixValue]; });
  t('wkcm.toString', function () { return String(new WebKitCSSMatrix('translate(10px, 20px)')); });
  t('dm.toString', function () { return String(new DOMMatrix([1, 2, 3, 4, 5, 6])); });
  t('dm.toString3d', function () { return String(new DOMMatrix(I16)).slice(0, 12); });
  // ---- WebKitCSSMatrix ctor string
  t('wkcm.ctorString', function () { var m = new WebKitCSSMatrix('scale(2)'); return [m.a, m.d]; });
  // ---- arity (Function.length on methods/statics; IDL required-arg count)
  t('ar.methods', function () {
    var g = function (o, n) { return o[n] && o[n].length; };
    return [g(DOMMatrixReadOnly.prototype, 'multiply'), g(DOMMatrixReadOnly.prototype, 'scale'), g(DOMMatrixReadOnly.prototype, 'transformPoint'), g(DOMMatrixReadOnly.prototype, 'toFloat32Array'), g(DOMMatrix.prototype, 'setMatrixValue'), g(DOMMatrix.prototype, 'multiplySelf'), g(DOMPointReadOnly.prototype, 'matrixTransform'), g(DOMQuad.prototype, 'getBounds'), g(DOMRectReadOnly.prototype, 'toJSON')];
  });
  t('ar.statics', function () { return [DOMMatrix.fromMatrix.length, DOMMatrix.fromFloat32Array.length, DOMPoint.fromPoint.length, DOMRect.fromRect.length, DOMQuad.fromQuad.length]; });
  // ---- number formatting in toString
  t('dm.toString.frac', function () { return String(new DOMMatrix([0.1 + 0.2, -0, 1e21, 1.5e-7, 6.123233995736766e-17, 1])); });
  t('dm.toString3d.full', function () { return String(new DOMMatrix(I16)); });
  // ---- getBounds (bounds alias was removed from Chrome)
  t('dq.getBounds', function () { var q = new DOMQuad(new DOMPoint(0, 4), new DOMPoint(4, 0), new DOMPoint(2, 6), new DOMPoint(6, 2)); var b = q.getBounds(); return [b.x, b.y, b.width, b.height, b.constructor.name]; });
  t('dq.boundsGone', function () { return typeof DOMQuad.prototype.getBounds === 'function' && DOMQuad.prototype.bounds === undefined; });
  // ---- rotation precision (right-angle fast path?)
  t('ro.rotate45', function () { var m = new DOMMatrixReadOnly().rotate(45); return [m.a, m.b]; });
  t('ro.rotate30', function () { var m = new DOMMatrixReadOnly().rotate(30); return [m.a, m.b]; });
  t('ro.rotate270', function () { var m = new DOMMatrixReadOnly().rotate(270); return [m.a, m.b]; });
  t('ro.rotateNeg90', function () { var m = new DOMMatrixReadOnly().rotate(-90); return [m.a, m.b]; });
  // ---- bad dictionary args (string/number instead of dict)
  t('ro.multiply.badtype', function () { return new DOMMatrixReadOnly().multiply('nope'); });
  t('dp.fromPoint.badtype', function () { return DOMPoint.fromPoint('nope'); });
  t('dq.fromRect.badtype', function () { return DOMQuad.fromRect(5); });
  t('dm.ctor.badtype', function () { return new DOMMatrix(5); });
  // ---- string ctor paths
  t('dm.ctorString', function () { var m = new DOMMatrix('scale(2) translate(10px, 0)'); return [m.a, m.e, m.is2D]; });
  t('dm.ctorString.none', function () { var m = new DOMMatrix('none'); return [m.a, m.d, m.is2D]; });
  // ---- f32 storage fidelity
  t('dm.f32.round', function () { return DOMMatrix.fromFloat32Array(new Float32Array([0.1, 0, 0, 0.1, 0, 0])).m11; });
  // ---- 2D loss via m13+ setters
  t('dm.m13.set', function () { var m = new DOMMatrix(); m.m13 = 5; return [m.m13, m.is2D]; });
  t('dm.a.set', function () { var m = new DOMMatrix(); m.a = 7; return [m.a, m.m11, m.is2D]; });
  // ---- WebKitCSSMatrix subclass shape
  t('wkcm.subclass', function () { var W = WebKitCSSMatrix; return [Object.getPrototypeOf(W) === DOMMatrix, Object.getPrototypeOf(W.prototype) === DOMMatrix.prototype, W.name, W.length]; });
  t('wkcm.setMatrixValue.ret', function () { var m = new WebKitCSSMatrix(); return m.setMatrixValue('scale(2)') === m; });
  // ---- WebKitCSSMatrix is DOMMatrix's LegacyFactoryFunction, not a subclass
  t('wkcm.sameProto', function () { return [WebKitCSSMatrix.prototype === DOMMatrix.prototype, DOMMatrix.prototype.constructor === DOMMatrix]; });
  t('wkcm.ownNames', function () { return Object.getOwnPropertyNames(WebKitCSSMatrix).sort().join(','); });
  t('wkcm.instanceofSelf', function () { return [new WebKitCSSMatrix() instanceof WebKitCSSMatrix, WebKitCSSMatrix.fromMatrix === undefined, Object.prototype.toString.call(new WebKitCSSMatrix())]; });
  t('wkcm.parent', function () { return [Object.getPrototypeOf(WebKitCSSMatrix) === Function.prototype, typeof WebKitCSSMatrix]; });
  // ---- CSS transform list parsing (setMatrixValue grammar)
  t('dm.smv.chained', function () { var m = new DOMMatrix(); m.setMatrixValue('translate(10px) rotate(90deg) scale(2, 3) skewX(45deg)'); return [m.e, m.a, m.b, m.c, m.d]; });
  t('dm.smv.matrix', function () { var m = new DOMMatrix(); m.setMatrixValue('matrix(2, 0, 0, 3, 10, 20)'); return [m.a, m.d, m.e]; });
  t('dm.smv.percent', function () { var m = new DOMMatrix(); return m.setMatrixValue('translate(50%, 10%)'); });
  t('dm.smv.rotate3d', function () { var m = new DOMMatrix(); m.setMatrixValue('rotate3d(0, 0, 1, 90deg)'); return [m.a, m.b, m.is2D]; });
  t('dm.smv.perspective', function () { var m = new DOMMatrix(); m.setMatrixValue('perspective(400px)'); return [m.is2D, m.m34.toFixed(6)]; });
  t('dm.smv.empty', function () { var m = new DOMMatrix([1,2,3,4,5,6]); m.setMatrixValue(''); return [m.a, m.e]; });
  t('dm.smv.degfree', function () { var m = new DOMMatrix(); m.setMatrixValue('rotate(1.5708rad)'); return [m.a.toFixed(6), m.b.toFixed(6)]; });
  t('dm.smv.numberOnly', function () { var m = new DOMMatrix(); return m.setMatrixValue('scale'); });
  t('dm.smv.trailing', function () { var m = new DOMMatrix(); return m.setMatrixValue('scale(2) garbage'); });
  // ---- DOMMatrixInit validation (fromMatrix/multiply dict fixing)
  t('dm.fromMatrix.empty', function () { return DOMMatrix.fromMatrix({}); });
  t('ro.multiply.init2d', function () { var m = new DOMMatrixReadOnly().multiply({ a: 2, d: 2, e: 10 }); return [m.a, m.d, m.e, m.is2D]; });
  t('ro.multiply.init3d', function () { var m = new DOMMatrixReadOnly().multiply({ m11: 2, m33: 2 }); return [m.m11, m.is2D]; });
  t('ro.multiply.initConflict', function () { return new DOMMatrixReadOnly().multiply({ is2D: true, m13: 5 }); });
  t('ro.multiply.initContradict', function () { return new DOMMatrixReadOnly().multiply({ is2D: false, a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 }); });
  // ---- matrixTransform / dict ctors
  t('dp.matrixTransform', function () { var p = new DOMPoint(1, 2, 0, 1).matrixTransform({ a: 2, d: 2, e: 10 }); return [p.x, p.y, p.constructor.name]; });
  t('dp.matrixTransform.empty', function () { var p = new DOMPoint(1, 2, 3, 4).matrixTransform({}); return [p.x, p.y, p.z, p.w]; });
  t('dp.transformPoint.empty', function () { var p = new DOMMatrix().transformPoint({}); return [p.x, p.y, p.z, p.w, p.constructor.name]; });
  t('dp.transformPoint.badtype', function () { return new DOMMatrix().transformPoint('x'); });
  t('dq.ctorDicts', function () { var q = new DOMQuad({ x: 0, y: 0 }, { x: 4, y: 4 }); return [q.p1.x, q.p2.x, q.p2.y, q.p1 instanceof DOMPoint]; });
  // ---- op defaults / arity errors
  t('ro.scale.oneArg', function () { var m = new DOMMatrixReadOnly().scale(2); return [m.a, m.d, m.e]; });
  t('ro.scale.z', function () { var m = new DOMMatrixReadOnly().scale(2, undefined, 3); return [m.a, m.d, m.m33, m.is2D]; });
  t('ro.rotate.noargs', function () { var m = new DOMMatrixReadOnly().rotate(); return [m.a, m.d, m.is2D]; });
  t('ro.rotate.xy', function () { var m = new DOMMatrixReadOnly().rotate(0, 90, 0); return [m.is2D, m.m11.toFixed(6), m.m13.toFixed(6)]; });
  t('ro.translate.oneArg', function () { var m = new DOMMatrixReadOnly().translate(10); return [m.e, m.f, m.m43, m.is2D]; });
  t('dp.ctor.coerce', function () { return [new DOMPoint('5').x, new DOMPoint({ valueOf: function () { return 7; } }).x]; });
  t('dp.ctor.symbol', function () { return new DOMPoint(Symbol.iterator).x; });
  t('dm.ff32.noargs', function () { return DOMMatrix.fromFloat32Array(); });
  t('dm.smv.noargs', function () { return new DOMMatrix().setMatrixValue(); });
  t('dm.ff32.nan', function () { return DOMMatrix.fromFloat32Array(new Float32Array([NaN, 0, 0, 1, 0, 0])); });
  // ---- CSS parser corner units
  t('dm.smv.unitless', function () { var m = new DOMMatrix(); return m.setMatrixValue('rotate(90)'); });
  t('dm.smv.turn', function () { var m = new DOMMatrix(); m.setMatrixValue('rotate(0.25turn)'); return [m.a.toFixed(9), m.b.toFixed(9)]; });
  t('dm.smv.grad', function () { var m = new DOMMatrix(); m.setMatrixValue('rotate(100grad)'); return [m.a.toFixed(9), m.b.toFixed(9)]; });
  t('dm.smv.calc', function () { var m = new DOMMatrix(); return m.setMatrixValue('translate(calc(10px + 5px))'); });
  t('dm.smv.zero', function () { var m = new DOMMatrix(); m.setMatrixValue('translate(0)'); return [m.e, m.is2D]; });
  t('dm.smv.expnot', function () { var m = new DOMMatrix(); return m.setMatrixValue('scale(1e2)'); });
  t('dm.smv.tx', function () { var m = new DOMMatrix(); m.setMatrixValue('translateX(5px) translateZ(3px)'); return [m.e, m.m43, m.is2D]; });
  t('dm.smv.scale1', function () { var m = new DOMMatrix(); m.setMatrixValue('scaleX(2) scaleY(3) scaleZ(4)'); return [m.a, m.d, m.m33, m.is2D]; });
  t('dm.smv.skew', function () { var m = new DOMMatrix(); m.setMatrixValue('skew(30deg, 60deg)'); return [m.c.toFixed(9), m.b.toFixed(9)]; });
  t('dm.smv.rotX', function () { var m = new DOMMatrix(); m.setMatrixValue('rotateX(90deg)'); return [m.is2D, m.m22.toFixed(6), m.m23.toFixed(6)]; });
  t('dm.smv.neg', function () { var m = new DOMMatrix(); m.setMatrixValue('translate(-10px, -2.5e1px)'); return [m.e, m.f]; });
  t('dm.smv.ws', function () { var m = new DOMMatrix(); m.setMatrixValue('  scale( 2 )  '); return [m.a]; });
  // ---- WebKitCSSMatrix proto mystery + toString 3D
  t('wkcm.protoOf', function () { var p = Object.getPrototypeOf(WebKitCSSMatrix); return [p === Object.prototype, p === null, String(p).slice(0, 40)]; });
  t('dm.toString.loss2d', function () { var m = new DOMMatrix(); m.m31 = 1; return String(m).slice(0, 10); });
  // ---- missing-vs-undefined defaults & 2D-preservation laws
  t('ro.translate2d', function () { var m = new DOMMatrixReadOnly().translate(10, 20); return [m.e, m.f, m.m43, m.is2D]; });
  t('ro.translate.z0', function () { var m = new DOMMatrixReadOnly().translate(10, 20, 0); return [m.e, m.f, m.m43, m.is2D]; });
  t('ro.translate.undef', function () { var m = new DOMMatrixReadOnly().translate(10, 20, undefined); return [m.e, m.f, m.m43, m.is2D]; });
  t('ro.rotateFromVector.oneArg', function () { var m = new DOMMatrixReadOnly().rotateFromVector(3); return [m.a, m.b, m.c, m.d]; });
  t('ro.rotateFromVector.undef', function () { var m = new DOMMatrixReadOnly().rotateFromVector(3, undefined); return [m.a, m.b, m.c, m.d]; });
  t('ro.rotate.undef', function () { var m = new DOMMatrixReadOnly().rotate(0, undefined, 90); return [m.a, m.b, m.is2D]; });
  t('ro.scale.undefX', function () { var m = new DOMMatrixReadOnly().scale(undefined); return [m.a, m.d, m.m33, m.is2D]; });
  t('dm.smv.rotY', function () { var m = new DOMMatrix(); m.setMatrixValue('rotateY(90deg)'); return [m.is2D, m.m31.toFixed(6), m.m33.toFixed(6)]; });
  return JSON.stringify(R);
})()
