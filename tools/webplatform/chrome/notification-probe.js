// Notification probe (M4 knife 10). Chrome 145 headless: permission starts
// 'default', the first requestPermission() settles 'denied' (and sticks),
// and constructing a Notification without permission asynchronously fires a
// trusted plain `error` Event. Order matters: permission is read BEFORE the
// request. Runs in Chrome AND the engine; diffed key by key.
(async function () {
  var R = {};
  function norm(str) {
    if (!/^https?:/.test(location.origin)) return str;
    return str.split(location.href).join('#B').split(location.origin).join('#O');
  }
  function t(n, f) {
    try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(norm(JSON.stringify(v))); }
    catch (e) { R[n] = norm('THROW ' + e.name + ': ' + String(e.message).slice(0, 240)); }
  }
  var C = Notification, P = C.prototype;
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; }
  R.shape = { order: Object.getOwnPropertyNames(P), statics: Object.getOwnPropertyNames(C), len: C.length, parent: Object.getPrototypeOf(C).name, tag: Object.prototype.toString.call(Object.create(P)), kinds: {}, staticKinds: {} };
  R.shape.order.forEach(function (m) { R.shape.kinds[m] = kind(P, m); });
  R.shape.statics.forEach(function (m) { R.shape.staticKinds[m] = kind(C, m); });
  t('permission.initial', function () { return Notification.permission; });
  t('maxActions', function () { return Notification.maxActions; });
  t('ctor.noargs', function () { return new Notification(); });
  t('ctor.call', function () { return Notification('x'); });
  t('ctor.badopt', function () { return new Notification('x', 5); });
  t('ctor.sym', function () { return new Notification(Symbol()); });
  t('fresh', function () {
    var n = new Notification('Title'), o = {};
    ['title', 'dir', 'lang', 'body', 'tag', 'icon', 'badge', 'vibrate', 'renotify', 'silent', 'requireInteraction', 'data', 'actions', 'image', 'maxActions', 'onclick', 'onshow', 'onerror', 'onclose'].forEach(function (k) {
      var v = n[k]; o[k] = v === undefined ? '#undefined' : (typeof v === 'object' && v !== null) ? 'OBJ:' + (Array.isArray(v) ? 'Array' + v.length : v.constructor.name) : v;
    });
    o.tsNear = Math.abs(n.timestamp - Date.now()) < 5000; o.own = Object.getOwnPropertyNames(n); o.tag2 = Object.prototype.toString.call(n);
    o.chain = [n instanceof EventTarget, Object.getPrototypeOf(Notification) === EventTarget];
    n.close(); return o;
  });
  t('opts1', function () { var n = new Notification('T', { body: 'b', dir: 'rtl', lang: 'fr', tag: 'tg', icon: '/i.png', badge: '/b.png', data: { a: 1 }, silent: true, renotify: true, requireInteraction: true, timestamp: 12345 }); var r = { title: n.title, body: n.body, dir: n.dir, lang: n.lang, tag: n.tag, icon: n.icon, badge: n.badge, data: n.data, silent: n.silent, renotify: n.renotify, requireInteraction: n.requireInteraction, timestamp: n.timestamp, vibrate: n.vibrate }; n.close(); return r; });
  t('opts.renotify.notag', function () { return new Notification('T', { renotify: true }); });
  t('opts.silent.vibrate', function () { return new Notification('T', { silent: true, vibrate: [1] }); });
  t('opts.badDir', function () { return new Notification('T', { dir: 'zzz' }); });
  t('opts.dir.vals', function () { var o = {}; ['auto', 'ltr', 'rtl', 'RTL'].forEach(function (v) { try { var n = new Notification('T', { dir: v }); o[v] = n.dir; n.close(); } catch (e) { o[v] = 'T'; } }); return o; });
  t('opts.conv', function () { var n = new Notification(5, { body: 7, tag: null, lang: undefined, silent: 0, timestamp: '9' }); var r = [n.title, n.body, n.tag, n.lang, n.silent, n.timestamp]; n.close(); return r; });
  t('opts.actions', function () { return new Notification('T', { actions: [{ action: 'a', title: 'A' }] }); });
  t('opts.actions.empty', function () { var n = new Notification('T', { actions: [] }); n.close(); return 'ok'; });
  t('opts.vibrate', function () { var n = new Notification('T', { vibrate: [100, 50, 200] }); var v = n.vibrate; var r = [v, Object.isFrozen(v), Array.isArray(v), n.vibrate === v]; n.close(); return r; });
  t('opts.vibrate.num', function () { var n = new Notification('T', { vibrate: 200 }); var r = n.vibrate; n.close(); return r; });
  t('opts.vibrate.bad', function () { var n = new Notification('T', { vibrate: 'x' }); var r = n.vibrate; n.close(); return r; });
  t('opts.image', function () { var n = new Notification('T', { image: '/im.png' }); var r = ['image' in n, n.image]; n.close(); return r; });
  t('opts.icon.rel', function () { var n = new Notification('T', { icon: 'rel.png', badge: 'http://x.test/b a.png' }); var r = [n.icon, n.badge]; n.close(); return r; });
  t('opts.icon.empty', function () { var n = new Notification('T', { icon: '' }); var r = n.icon; n.close(); return r; });
  t('opts.ts', function () { var o = {}; [['str', '9'], ['neg', -5], ['float', 2.7], ['nan', NaN], ['null', null], ['big', 1e20]].forEach(function (c) { try { var n = new Notification('T', { timestamp: c[1] }); o[c[0]] = n.timestamp; n.close(); } catch (e) { o[c[0]] = e.name + ': ' + e.message.slice(0, 130); } }); return o; });
  t('opts.bools', function () { var n = new Notification('T', { silent: 'x', requireInteraction: 1, renotify: 0 }); var r = [n.silent, n.requireInteraction, n.renotify]; n.close(); return r; });
  t('opts.silent.null', function () { var n = new Notification('T', { silent: null }); var r = n.silent; n.close(); return r; });
  t('opts.nullish', function () { var a = new Notification('T', null), b = new Notification('T', undefined); a.close(); b.close(); return 'ok'; });
  t('title.conv', function () { var o = {}; [['null', null], ['undef', undefined], ['obj', {}], ['num', 0]].forEach(function (c) { try { var n = new Notification(c[1]); o[c[0]] = n.title; n.close(); } catch (e) { o[c[0]] = 'T'; } }); return o; });
  t('data.clone', function () { var d = { x: [1, 2] }; var n = new Notification('T', { data: d }); var r = [n.data === d, JSON.stringify(n.data), n.data === n.data]; n.close(); return r; });
  t('data.fn', function () { return new Notification('T', { data: function () {} }); });
  t('data.vals', function () { var o = {}; [['undef', undefined], ['null', null], ['num', 5], ['str', 's'], ['date', new Date(0)], ['map', new Map([[1, 2]])], ['sym', Symbol()]].forEach(function (c) { try { var n = new Notification('T', { data: c[1] }); var d = n.data; o[c[0]] = d === null ? null : typeof d === 'object' ? Object.prototype.toString.call(d) : d; n.close(); } catch (e) { o[c[0]] = e.name + ': ' + e.message.slice(0, 90); } }); return o; });
  t('brand', function () { return Object.getOwnPropertyDescriptor(Notification.prototype, 'title').get.call({}); });
  t('close.brand', function () { return Notification.prototype.close.call({}); });
  t('close.ret', function () { var n = new Notification('T'); return [n.close(), n.close()]; });
  t('handlers', function () { var n = new Notification('T'), f = function () {}; n.onclick = f; n.onerror = 5; var r = [n.onclick === f, n.onerror, n.onshow]; n.close(); return r; });
  t('requestPermission.len', function () { return Notification.requestPermission.length; });
  async function p(label, fn) { try { var v = fn(); R[label] = v && v.then ? await v : v; } catch (e) { R[label] = 'THROW ' + e.name + ': ' + e.message; } }
  async function ev(label, mk, wait) {
    var log = [], hlog = [], n;
    try { n = mk(); } catch (e) { R[label] = 'THROW ' + e.name + ': ' + e.message; return; }
    ['show', 'click', 'close', 'error'].forEach(function (k) { n.addEventListener(k, function (e) { log.push(k + ':' + e.constructor.name + ':' + e.isTrusted + ':' + e.bubbles + ':' + e.cancelable + ':' + (e.target === n)); }); });
    n.onerror = function () { hlog.push('onerror'); };
    await new Promise(function (r) { setTimeout(r, wait || 250); });
    log.push('end');
    try { n.close(); } catch (e) { log.push('closeTHROW'); }
    await new Promise(function (r) { setTimeout(r, 120); });
    log.push('afterclose');
    R[label] = { log: log, hlog: hlog };
  }
  await ev('ev.basic', function () { return new Notification('T'); });
  await ev('ev.close', function () { var n = new Notification('T'); n.close(); return n; });
  await p('reqperm.badcb', function () { return Notification.requestPermission(5); });
  await p('reqperm.cb', function () { return new Promise(function (res) { var r = Notification.requestPermission(function (x) { res(['cb', x, r && typeof r.then]); }); }); });
  await p('reqperm', function () { return Notification.requestPermission(); });
  t('permission.after', function () { return Notification.permission; });
  t('window.names', function () { return ['Notification', 'NotificationEvent'].map(function (k) { return typeof window[k]; }); });
  return JSON.stringify(R);
})()
