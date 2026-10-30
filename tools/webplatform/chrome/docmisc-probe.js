// Document legacy/misc member probe (M4 knife 20): the body-colour legacy attributes, collections
// (anchors/applets/embeds/plugins), designMode, fullscreen / pointer-lock state faces, lastModified,
// no-op legacy methods, privacy-promise members. Behaviour is compared in a page with no
// stylesheet and no fullscreen/pointer-lock activity, so every value is deterministic.
(async function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : (typeof x === 'number' && !isFinite(x) ? '#' + x : x); })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 240); } }
  async function ta(n, f) { try { R[n] = conv(await f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 240); } }
  var NAMES = ['alinkColor', 'anchors', 'applets', 'bgColor', 'captureEvents', 'caretPositionFromPoint', 'clear', 'designMode', 'embeds', 'exitFullscreen', 'exitPointerLock', 'fgColor', 'fragmentDirective', 'fullscreen', 'fullscreenElement', 'fullscreenEnabled', 'hasPrivateToken', 'hasRedemptionRecord', 'hasUnpartitionedCookieAccess', 'lastModified', 'linkColor', 'plugins', 'pointerLockElement', 'prerendering', 'releaseEvents', 'rootElement', 'vlinkColor', 'wasDiscarded'];
  R.kinds = {}; NAMES.forEach(function (m) { var d = Object.getOwnPropertyDescriptor(Document.prototype, m); R.kinds[m] = d ? [d.get ? (d.set ? 'rw' : 'ro') : typeof d.value, d.enumerable, d.configurable, d.value && d.value.length] : 'absent'; });
  var all = Object.getOwnPropertyNames(Document.prototype);
  R.relorder = NAMES.filter(function (m) { return all.indexOf(m) >= 0; }).sort(function (a, b) { return all.indexOf(a) - all.indexOf(b); });
  function shape(C) { var P = C.prototype, k = {}; Object.getOwnPropertyNames(P).forEach(function (m) { var d = Object.getOwnPropertyDescriptor(P, m); k[m] = d.get ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : ''); }); return { order: Object.getOwnPropertyNames(P), kinds: k, len: C.length, tag: Object.prototype.toString.call(Object.create(P)) }; }
  R['shape.CaretPosition'] = typeof CaretPosition === 'function' ? shape(CaretPosition) : 'absent';
  R['shape.FragmentDirective'] = typeof FragmentDirective === 'function' ? shape(FragmentDirective) : 'absent';
  t('caret.ctor', function () { try { new CaretPosition(); } catch (e) { return e.name + ': ' + e.message; } });
  t('fd.ctor', function () { try { new FragmentDirective(); } catch (e) { return e.name + ': ' + e.message; } });
  t('caret.rect', function () { var r = document.caretPositionFromPoint(1, 1); var q = r.getClientRect(); return q === null ? 'null' : q.constructor.name; });
  // ---- colours
  t('color.defaults', function () { return [document.bgColor, document.fgColor, document.linkColor, document.alinkColor, document.vlinkColor]; });
  t('color.set', function () { document.bgColor = 'red'; document.fgColor = '#00f'; document.linkColor = 'x'; var r = [document.bgColor, document.fgColor, document.linkColor, document.body.getAttribute('bgcolor'), document.body.getAttribute('text'), document.body.getAttribute('link')]; document.body.removeAttribute('bgcolor'); document.body.removeAttribute('text'); document.body.removeAttribute('link'); return r; });
  t('color.attr', function () { document.body.setAttribute('vlink', 'green'); document.body.setAttribute('alink', 'blue'); var r = [document.vlinkColor, document.alinkColor]; document.body.removeAttribute('vlink'); document.body.removeAttribute('alink'); return r; });
  t('color.conv', function () { document.bgColor = null; var a = document.bgColor; document.bgColor = 5; var b = document.bgColor; document.body.removeAttribute('bgcolor'); return [a, b]; });
  t('color.nobody', function () { var b = document.body; var h = document.documentElement; h.removeChild(b); var r = [document.bgColor, document.vlinkColor]; try { document.bgColor = 'red'; r.push('noerr'); } catch (e) { r.push(e.name); } h.appendChild(b); return r; });
  // ---- collections
  t('coll.types', function () { return [Object.prototype.toString.call(document.anchors), Object.prototype.toString.call(document.applets), Object.prototype.toString.call(document.embeds), Object.prototype.toString.call(document.plugins), document.anchors === document.anchors, document.embeds === document.plugins]; });
  t('coll.empty', function () { return [document.anchors.length, document.applets.length, document.embeds.length, document.plugins.length]; });
  t('coll.live', function () { var a = document.createElement('a'); a.name = 'n1'; var b = document.createElement('a'); b.href = '#x'; var e = document.createElement('embed'); document.body.append(a, b, e); var r = [document.anchors.length, document.anchors[0] === a, document.embeds.length, document.embeds[0] === e, document.plugins.length]; a.remove(); b.remove(); e.remove(); r.push(document.anchors.length, document.embeds.length); return r; });
  t('coll.anchors.attr', function () { var a = document.createElement('a'); document.body.append(a); var r = [document.anchors.length]; a.setAttribute('name', ''); r.push(document.anchors.length); a.setAttribute('name', 'q'); r.push(document.anchors.length); a.remove(); return r; });
  // ---- designMode
  t('designMode', function () { var r = [document.designMode]; document.designMode = 'on'; r.push(document.designMode); document.designMode = 'ON'; r.push(document.designMode); document.designMode = 'zz'; r.push(document.designMode); document.designMode = 'off'; r.push(document.designMode); return r; });
  t('designMode.conv', function () { document.designMode = null; var a = document.designMode; document.designMode = 'on'; document.designMode = undefined; var b = document.designMode; document.designMode = 'off'; return [a, b]; });
  // ---- fullscreen / pointer lock faces
  t('fs.state', function () { return [document.fullscreen, document.fullscreenElement, typeof document.fullscreenEnabled, document.pointerLockElement, document.prerendering, document.wasDiscarded, document.rootElement]; });
  t('fs.exit', function () { var p = document.exitFullscreen(); var r = [p instanceof Promise]; p.catch(function () {}); return r; });
  await ta('fs.exit.result', async function () { try { await document.exitFullscreen(); return 'resolved'; } catch (e) { return e.name + ': ' + e.message; } });
  t('pl.exit', function () { return document.exitPointerLock(); });
  t('fragmentDirective', function () { var f = document.fragmentDirective; return [Object.prototype.toString.call(f), f === document.fragmentDirective, f.constructor.name, Object.getOwnPropertyNames(f)]; });
  t('fs.readonly', function () { document.fullscreen = true; document.prerendering = true; document.fullscreenElement = 5; return [document.fullscreen, document.prerendering, document.fullscreenElement]; });
  // ---- lastModified
  t('lastModified', function () { var s = document.lastModified; return [typeof s, /^\d\d\/\d\d\/\d{4} \d\d:\d\d:\d\d$/.test(s), s === document.lastModified]; });
  // ---- no-op legacy methods
  t('noop.methods', function () { return [document.captureEvents(1), document.releaseEvents(1), document.clear(), document.captureEvents.length, document.releaseEvents.length, document.clear.length]; });
  t('caret', function () { var r = document.caretPositionFromPoint(1, 1); return r === null ? 'null' : [r.constructor.name, typeof r.offset, r.offsetNode === null ? 'null' : typeof r.offsetNode]; });
  t('caret.args', function () { try { document.caretPositionFromPoint(1); } catch (e) { return e.name + ': ' + e.message; } });
  t('caret.len', function () { return document.caretPositionFromPoint.length; });
  // ---- privacy promises
  await ta('priv.token', async function () { try { return await document.hasPrivateToken('https://x.test'); } catch (e) { return e.name + ': ' + e.message; } });
  await ta('priv.record', async function () { try { return await document.hasRedemptionRecord('https://x.test'); } catch (e) { return e.name + ': ' + e.message; } });
  await ta('priv.cookie', async function () { try { return await document.hasUnpartitionedCookieAccess(); } catch (e) { return e.name + ': ' + e.message; } });
  t('priv.returns', function () { var a = document.hasPrivateToken('https://x.test'), b = document.hasRedemptionRecord('https://x.test'), c = document.hasUnpartitionedCookieAccess(); [a, b, c].forEach(function (p) { p.catch(function () {}); }); return [a instanceof Promise, b instanceof Promise, c instanceof Promise]; });
  t('priv.args', function () { var r = []; ['hasPrivateToken', 'hasRedemptionRecord'].forEach(function (m) { try { var p = document[m](); p.catch(function () {}); r.push('promise'); } catch (e) { r.push(e.name + ': ' + e.message); } }); return r; });
  await ta('priv.args.async', async function () { var r = []; for (var m of ['hasPrivateToken', 'hasRedemptionRecord']) { try { await document[m](); r.push('ok'); } catch (e) { r.push(e.name + ': ' + e.message); } } return r; });
  t('brand', function () { var o = []; ['alinkColor', 'anchors', 'designMode', 'fullscreen', 'lastModified', 'fragmentDirective'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(Document.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['clear', 'captureEvents', 'exitPointerLock'].forEach(function (m) { try { Document.prototype[m].call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  return JSON.stringify(R);
})()
