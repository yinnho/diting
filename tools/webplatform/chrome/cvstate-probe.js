// Canvas 2D state attributes probe (M4 knife 41): defaults, enum / numeric / length validation, line dash, reset, roundRect errors.
(function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : (typeof x === 'number' && !isFinite(x) ? '#' + x : x); })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 200); } }
  var cv = document.createElement('canvas'); cv.width = 20; cv.height = 20;
  function ctx() { return cv.getContext('2d'); }
  var x = ctx();
  var NAMES = ['lang', 'direction', 'fontKerning', 'fontStretch', 'fontVariantCaps', 'letterSpacing', 'textRendering', 'wordSpacing', 'filter', 'imageSmoothingQuality', 'imageSmoothingEnabled', 'shadowColor', 'shadowOffsetX', 'shadowOffsetY', 'shadowBlur', 'lineCap', 'lineJoin', 'miterLimit', 'lineDashOffset'];
  t('defaults', function () { var o = {}; NAMES.forEach(function (n) { o[n] = x[n]; }); return o; });
  t('kinds', function () { var o = {}; NAMES.concat(['getLineDash', 'setLineDash', 'reset', 'roundRect', 'isContextLost', 'getContextAttributes', 'createConicGradient', 'drawFocusIfNeeded']).forEach(function (n) { var d = Object.getOwnPropertyDescriptor(CanvasRenderingContext2D.prototype, n); o[n] = d ? (d.get ? (d.set ? 'rw' : 'ro') : 'fn' + d.value.length) : 'absent'; }); return o; });
  var EN = { direction: ['ltr', 'rtl', 'inherit', 'zz', 'LTR', ''], fontKerning: ['auto', 'normal', 'none', 'zz'], fontStretch: ['condensed', 'ultra-expanded', 'zz'], fontVariantCaps: ['small-caps', 'titling-caps', 'zz'], imageSmoothingQuality: ['low', 'medium', 'high', 'zz'], lineCap: ['round', 'square', 'zz', 'BUTT'], lineJoin: ['bevel', 'round', 'zz'], textRendering: ['optimizeSpeed', 'geometricPrecision', 'zz'] };
  Object.keys(EN).forEach(function (k) { t('enum.' + k, function () { var c = ctx(); return EN[k].map(function (v) { c[k] = v; return c[k]; }); }); });
  t('enum.coerce', function () { var c = ctx(); c.lineCap = { toString: function () { return 'round'; } }; var a = c.lineCap; c.lineCap = null; return [a, c.lineCap]; });
  t('num.shadowBlur', function () { var c = ctx(); return [5, -1, NaN, Infinity, '7', null, undefined, 0].map(function (v) { c.shadowBlur = v; return c.shadowBlur; }); });
  t('num.shadowOffset', function () { var c = ctx(); return [3, -4, NaN, Infinity, '2', null].map(function (v) { c.shadowOffsetX = v; return c.shadowOffsetX; }); });
  t('num.miterLimit', function () { var c = ctx(); return [3, 0, -1, NaN, 1e9, '4'].map(function (v) { c.miterLimit = v; return c.miterLimit; }); });
  t('num.lineDashOffset', function () { var c = ctx(); return [3, -2, NaN, Infinity].map(function (v) { c.lineDashOffset = v; return c.lineDashOffset; }); });
  t('bool.smoothing', function () { var c = ctx(); return [false, 0, 'x', '', null, undefined, 1].map(function (v) { c.imageSmoothingEnabled = v; return c.imageSmoothingEnabled; }); });
  t('len.spacing', function () { var c = ctx(); return ['2px', '1em', '-3px', 'zz', '5', '0', '1.5pt', '', ' 4px '].map(function (v) { c.letterSpacing = v; return c.letterSpacing; }); });
  t('len.word', function () { var c = ctx(); return ['2px', 'zz', '0'].map(function (v) { c.wordSpacing = v; return c.wordSpacing; }); });
  t('str.lang', function () { var c = ctx(); return ['fr', 5, null, undefined, ''].map(function (v) { c.lang = v; return c.lang; }); });
  t('str.filter', function () { var c = ctx(); return ['blur(2px)', 'zz', 'none', 'grayscale(1)'].map(function (v) { c.filter = v; return c.filter; }); });
  t('dash.basic', function () { var c = ctx(); c.setLineDash([1, 2, 3]); var a = c.getLineDash(); c.setLineDash([4, 5]); var b = c.getLineDash(); c.setLineDash([-1, 2]); var d = c.getLineDash(); c.setLineDash([NaN]); var e = c.getLineDash(); c.setLineDash([]); return [a, b, d, e, c.getLineDash(), c.getLineDash() === c.getLineDash()]; });
  t('dash.bad', function () { var c = ctx(); return [function () { c.setLineDash(); }, function () { c.setLineDash(5); }, function () { c.setLineDash(null); }, function () { c.setLineDash('ab'); }].map(function (f) { try { f(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('dash.iter', function () { var c = ctx(); c.setLineDash(new Set([2, 4])); return c.getLineDash(); });
  t('reset', function () { var c = ctx(); c.lineCap = 'round'; c.shadowBlur = 4; c.setLineDash([2]); c.fillStyle = '#123456'; c.lineWidth = 7; c.direction = 'rtl'; var r = c.reset(); return [r, c.lineCap, c.shadowBlur, c.getLineDash(), c.fillStyle, c.lineWidth, c.direction]; });
  t('lost', function () { return [ctx().isContextLost(), ctx().getContextAttributes()]; });
  t('focus', function () { var c = ctx(); try { c.drawFocusIfNeeded(); } catch (e) { return e.name + ': ' + e.message; } });
  t('conic', function () { var c = ctx(); var o = []; [function () { return c.createConicGradient(0, 1, 2); }, function () { return c.createConicGradient(0, 1); }, function () { return c.createConicGradient(NaN, 1, 2); }].forEach(function (f) { try { var g = f(); o.push([typeof g, typeof g.addColorStop]); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('round.errors', function () { var c = ctx(); return [function () { c.roundRect(); }, function () { c.roundRect(1, 2, 3); }, function () { c.roundRect(1, 2, 3, 4, -1); }, function () { c.roundRect(1, 2, 3, 4, [1, 2, 3, 4, 5]); }, function () { c.roundRect(1, 2, 3, 4, []); }, function () { c.roundRect(NaN, 2, 3, 4); }, function () { c.roundRect(1, 2, 3, 4, [2]); }, function () { c.roundRect(1, 2, 3, 4, { x: 2, y: 2 }); }].map(function (f) { try { f(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('round.fill', function () { var c = ctx(); c.fillStyle = '#ff0000'; c.beginPath(); c.roundRect(2, 2, 16, 16, 4); c.fill(); var d = c.getImageData(10, 10, 1, 1).data; return [d[0], d[3]]; });
  t('brand', function () { var o = []; ['lang', 'shadowBlur', 'lineCap'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(CanvasRenderingContext2D.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['getLineDash', 'reset', 'roundRect'].forEach(function (m) { try { CanvasRenderingContext2D.prototype[m].call({}, 1, 2, 3, 4); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('instance.own', function () { return [Object.getOwnPropertyNames(ctx()), ctx() instanceof CanvasRenderingContext2D, ctx().constructor.name, Object.prototype.toString.call(ctx())]; });
  t('ctor', function () { try { new CanvasRenderingContext2D(); } catch (e) { return e.name + ': ' + e.message; } });
  return JSON.stringify(R);
})()
