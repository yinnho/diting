// URLPattern probe (M4 knife 12): corpus-driven. Every construction case
// records the eight component pattern strings + hasRegExpGroups (or the exact
// TypeError text); every match case records test() and the full exec() result
// (inputs + per-component {groups, input}). Runs in Chrome AND the engine.
(function () {
  var R = {};
  function norm(s) {
    if (!/^https?:/.test(location.origin)) return s;
    return s.split(location.href).join('#B').split(location.origin).join('#O');
  }
  function ser(v) { return JSON.parse(norm(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; }))); }
  function t(n, f) { try { var v = f(); R[n] = v === undefined ? '#undefined' : ser(v); } catch (e) { R[n] = norm('THROW ' + e.name + ': ' + String(e.message).slice(0, 260)); } }
  var COMP = ['protocol', 'username', 'password', 'hostname', 'port', 'pathname', 'search', 'hash'];
  function shape(p) { var o = {}; COMP.forEach(function (c) { o[c] = p[c]; }); o.hasRegExpGroups = p.hasRegExpGroups; return o; }

  R.exists = typeof URLPattern;
  var C = URLPattern, P = C.prototype;
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; }
  R.shape = { order: Object.getOwnPropertyNames(P), statics: Object.getOwnPropertyNames(C), len: C.length, tag: Object.prototype.toString.call(Object.create(P)), kinds: {} };
  R.shape.order.forEach(function (m) { R.shape.kinds[m] = kind(P, m); });
  t('call', function () { return URLPattern('x'); });
  t('brand', function () { return Object.getOwnPropertyDescriptor(URLPattern.prototype, 'pathname').get.call({}); });
  t('brand.test', function () { return URLPattern.prototype.test.call({}, 'x'); });
  t('brand.exec', function () { return URLPattern.prototype.exec.call({}, 'x'); });

  // ---- construction corpus: [label, source of the argument list]
  var CTOR = [
    ['empty', ''], ['undef', 'undefined'], ['emptyObj', '{}'], ['emptyStr', "''"],
    ['s.basic', "'https://example.com/books/:id'"],
    ['s.port.q.h', "'https://example.com:8080/a?x=1#h'"],
    ['s.http_s', "'http{s}?://*.example.com/*'"],
    ['s.wild.all', "'*://*/*'"],
    ['s.data', "'data:text/plain,hello'"],
    ['s.mailto', "'mailto:*@example.com'"],
    ['s.userinfo', "'https://:user::pass@host/'"],
    ['s.userinfo2', "'https://bob:pw@host.com/x'"],
    ['s.ipv6', "'https://[::1]:8000/'"],
    ['s.space', "'http://example.com/a b'"],
    ['s.dots', "'https://example.com/a/../b'"],
    ['s.file', "'file:///tmp/*'"],
    ['s.blob', "'blob:*'"],
    ['s.regexp', "'https://example.com/(\\\\d+)'"],
    ['s.opt', "'https://example.com/:a/:b?'"],
    ['s.name.regexp', "'https://example.com/:id(\\\\d+)'"],
    ['s.group', "'https://example.com/a{/b}?/c'"],
    ['s.modplus', "'https://example.com/:a+'"],
    ['s.modstar', "'https://example.com/:a*'"],
    ['s.rel.base', "'/foo?bar#baz', 'https://ex.com/dir/x'"],
    ['s.rel.norel', "'/foo'"],
    ['s.rel.q', "'?q=1#h', 'https://ex.com/p'"],
    ['s.rel.h', "'#h', 'https://ex.com/p?z'"],
    ['s.rel.dir', "'sub/page', 'https://ex.com/dir/x'"],
    ['s.rel.dotdot', "'../up', 'https://ex.com/a/b/c'"],
    ['s.base.opts', "'/x', 'https://ex.com', { ignoreCase: true }"],
    ['s.scheme.only', "'https:'"],
    ['s.noscheme.host', "'example.com/a'"],
    ['s.colon.port', "'https://example.com:*/'"],
    ['s.port.default', "'https://example.com:443/'"],
    ['s.port.http80', "'http://example.com:80/'"],
    ['s.uppercase', "'HTTPS://EXAMPLE.COM/PATH'"],
    ['s.query.pat', "'https://x.com/?a=:b&c=*'"],
    ['s.hash.pat', "'https://x.com/#:frag'"],
    ['s.pctenc', "'https://x.com/caf\u00e9'"],
    ['s.host.idn', "'https://b\u00fccher.example/'"],
    ['s.escape', "'https://x.com/a\\\\.b'"],
    ['s.star.path', "'https://x.com/*/a/*'"],
    ['s.name.after', "'https://x.com/:a-:b'"],
    ['s.name.dot', "'https://x.com/:a.:b'"],
    ['s.name.under', "'https://x.com/:a_b'"],
    ['s.name.unicode', "'https://x.com/:caf\u00e9'"],
    ['s.nonspecial', "'foo://bar/baz'"],
    ['s.nonspecial.host', "'foo://:h/p'"],
    ['s.opaque', "'about:blank'"],
    ['s.js', "'javascript:*'"],
    ['i.protocol', "{ protocol: 'https' }"], ['i.protocol.colon', "{ protocol: 'https:' }"],
    ['i.protocol.upper', "{ protocol: 'HTTPS' }"], ['i.protocol.pat', "{ protocol: 'http{s}?' }"],
    ['i.hostname.upper', "{ hostname: 'EXAMPLE.com' }"], ['i.hostname.pat', "{ hostname: '*.example.com' }"],
    ['i.hostname.ipv6', "{ hostname: '[::1]' }"], ['i.hostname.ipv6.up', "{ hostname: '[ABCD::1]' }"],
    ['i.port', "{ port: '8080' }"], ['i.port.def', "{ protocol: 'https', port: '443' }"], ['i.port.pat', "{ port: ':p' }"],
    ['i.port.http', "{ protocol: 'http', port: '80' }"],
    ['i.pathname', "{ pathname: '/a/:b*' }"], ['i.pathname.rel', "{ pathname: 'a/b' }"], ['i.pathname.dots', "{ pathname: '/a/./b/../c' }"],
    ['i.pathname.space', "{ pathname: '/a b' }"],
    ['i.search', "{ search: '?x=1' }"], ['i.search.nq', "{ search: 'x=1' }"], ['i.search.space', "{ search: 'a b' }"],
    ['i.hash', "{ hash: '#h' }"], ['i.hash.nh', "{ hash: 'h' }"], ['i.hash.space', "{ hash: 'a b' }"],
    ['i.username', "{ username: 'bob' }"], ['i.password', "{ password: 'p@ss' }"], ['i.username.pat', "{ username: ':u' }"],
    ['i.baseURL', "{ baseURL: 'https://ex.com/a/b?q#h' }"],
    ['i.baseURL.override', "{ baseURL: 'https://ex.com/a/b?q#h', pathname: 'c' }"],
    ['i.baseURL.host', "{ baseURL: 'https://ex.com:81/a', hostname: 'other.com' }"],
    ['i.baseURL.search', "{ baseURL: 'https://ex.com/a?q#h', search: 'z=1' }"],
    ['i.baseURL.proto', "{ baseURL: 'https://u:p@ex.com:81/a?q#h', protocol: 'http' }"],
    ['i.baseURL.opaque', "{ baseURL: 'data:text/plain,hi', pathname: 'x' }"],
    ['i.baseURL.bad', "{ baseURL: 'not a url' }"],
    ['i.nonspecial.path', "{ protocol: 'foo', pathname: 'a/b' }"],
    ['i.nonspecial.path2', "{ protocol: 'foo', pathname: '/a/../b' }"],
    ['i.opts.ic', "{ pathname: '/ABC' }, { ignoreCase: true }"],
    ['i.opts.undef', "{ pathname: '/x' }, undefined"],
    ['i.opts.null', "{ pathname: '/x' }, null"],
    ['i.base.str', "{ pathname: '/x' }, 'https://b.com'"],
    ['i.nums', "{ pathname: 5, port: 80 }"],
    ['i.undefmember', "{ pathname: undefined, search: undefined }"],
    ['i.nullmember', "{ pathname: null }"],
    ['i.unknown', "{ foo: 'bar', pathname: '/x' }"],
    ['n.num', '5'], ['n.null', 'null'], ['n.fn', 'function () {}'], ['n.arr', '[]'], ['n.true', 'true'],
    ['o.ic.str', "'https://x.com/ABC', undefined, { ignoreCase: true }"],
    ['o.opts.bad', "'https://x.com/', 'https://x.com', 5"],
    ['o.opts.ic.conv', "{ pathname: '/ABC' }, { ignoreCase: 1 }"],
    ['o.opts.ic.str', "{ pathname: '/ABC' }, { ignoreCase: 'false' }"],
    // errors
    ['e.paren', "{ pathname: '/(' }"], ['e.paren2', "{ pathname: '/)' }"], ['e.empty.regexp', "{ pathname: '/()' }"],
    ['e.nested', "{ pathname: '/((a))' }"], ['e.regexp.q', "{ pathname: '/(?:a)' }"], ['e.regexp.nonascii', "{ pathname: '/(\u00e9)' }"],
    ['e.regexp.bad', "{ pathname: '/([)' }"], ['e.noname', "{ pathname: '/:' }"], ['e.noname2', "{ pathname: '/:-a' }"],
    ['e.dupname', "{ pathname: '/:a/:a' }"], ['e.trail.backslash', "{ pathname: '/a\\\\' }"],
    ['e.close', "{ pathname: '/a}' }"], ['e.open', "{ pathname: '/a{' }"], ['e.open.nested', "{ pathname: '/{a{b}}' }"],
    ['e.protocol.bad', "{ protocol: 'a b' }"], ['e.protocol.digit', "{ protocol: '1http' }"], ['e.protocol.pat.bad', "{ protocol: '(a b)' }"],
    ['e.hostname.bad', "{ hostname: 'a b' }"], ['e.hostname.slash', "{ hostname: 'a/b' }"], ['e.hostname.at', "{ hostname: 'a@b' }"],
    ['e.hostname.colon', "{ hostname: 'a:b' }"], ['e.hostname.pct', "{ hostname: 'a%zzb' }"],
    ['e.port.bad', "{ port: 'abc' }"], ['e.port.big', "{ port: '99999' }"], ['e.port.neg', "{ port: '-1' }"], ['e.port.float', "{ port: '1.5' }"],
    ['e.username.bad', "{ username: 'a\\\\b' }"],
    ['e.search.bad', "{ search: '(' }"], ['e.hash.bad', "{ hash: '(' }"],
    ['e.str.paren', "'https://x.com/('"], ['e.str.port', "'https://x.com:abc/'"], ['e.str.colon', "'https://x.com:/'"],
    ['e.str.proto', "'a b://x/'"], ['e.str.host', "'https://a b/'"], ['e.str.name', "'https://x.com/:'"],
    ['e.str.base.bad', "'/x', 'not a url'"], ['e.str.base.rel', "'x', '/rel'"],
    ['e.sym', "Symbol()"], ['e.sym.member', "{ pathname: Symbol() }"],
  ];
  CTOR.forEach(function (c) {
    t('c.' + c[0], function () { var p = eval('new URLPattern(' + c[1] + ')'); return shape(p); });
  });

  // ---- match corpus
  function mk(src) { return eval('(function(){ return new URLPattern(' + src + '); })')(); }
  var PATS = {
    path: "{ pathname: '/books/:id' }",
    host: "{ hostname: ':sub.example.com', pathname: '/books/:id' }",
    full: "'https://example.com/books/:id?x=:x#:h'",
    wild: "{ pathname: '/files/*' }",
    opt: "{ pathname: '/a{/:b}?' }",
    num: "{ pathname: '/:id(\\\\d+)' }",
    star: "{ pathname: '/:a*' }",
    plus: "{ pathname: '/:a+' }",
    all: "{}",
    scheme: "{ protocol: 'http{s}?' }",
    ic: "{ pathname: '/ABC' }, { ignoreCase: true }",
    ics: "{ pathname: '/ABC' }",
    port: "{ protocol: 'https', port: '443' }",
    userinfo: "{ username: ':u', password: ':p' }",
    search: "{ search: 'q=:v&*' }",
    hash: "{ hash: ':h' }",
    anon: "{ pathname: '/(.*)/x/(.*)' }",
    base: "'/books/:id', 'https://x.com'",
    nonspecial: "'foo://bar/:p'",
  };
  var INPUTS = [
    "'https://example.com/books/42'", "'https://example.com/books/42?x=1&y=2#frag'", "'https://shop.example.com/books/7'",
    "'http://example.com/books/1'", "'https://example.com/books/'", "'https://example.com/books'", "'https://example.com:8443/books/9'",
    "'https://example.com/files/a/b.txt'", "'https://example.com/files/'", "'https://example.com/a'", "'https://example.com/a/x'",
    "'https://example.com/12'", "'https://example.com/ab'", "'https://example.com/ABC'", "'https://example.com/abc'",
    "'https://bob:secret@example.com/'", "'https://example.com/?q=1&z=2'", "'https://example.com/#top'",
    "'not a url'", "'/books/5'", "'/books/5', 'https://x.com'", "'https://x.com/books/%20x'", "'https://x.com/a/./b'",
    "'foo://bar/baz'", "'https://EXAMPLE.com/Books/42'", "'https://example.com/a/b/c/x/d'",
    "{ pathname: '/books/5' }", "{ pathname: '/books/5', hostname: 'a.example.com' }", "{ protocol: 'https', hostname: 'example.com', pathname: '/books/1' }",
    "{ pathname: '/files/q' }", "{}", "{ pathname: '/a b' }", "{ search: '?q=1' }", "{ baseURL: 'https://x.com/books/3' }",
    "{ pathname: 'rel', baseURL: 'https://x.com/books/' }", "{ port: '443', protocol: 'https' }", "{ hostname: 'EXAMPLE.COM' }",
    "{ protocol: 'a b' }", "{ port: 'zz' }",
  ];
  var n = 0;
  Object.keys(PATS).forEach(function (pk) {
    INPUTS.forEach(function (inp, i) {
      t('m.' + pk + '.' + i, function () {
        var p = mk(PATS[pk]);
        var r = eval('(function(p){ return p.exec(' + inp + '); })')(p);
        var tt = eval('(function(p){ return p.test(' + inp + '); })')(p);
        return { test: tt, exec: r };
      });
      n++;
    });
  });
  // arg edge cases for test/exec
  t('x.noargs', function () { var p = new URLPattern(); return [p.test(), p.exec() && p.exec().inputs]; });
  t('x.undef', function () { var p = new URLPattern({ pathname: '/a' }); return [p.test(undefined), p.exec(undefined, undefined) === null]; });
  t('x.init.base', function () { return new URLPattern().test({ pathname: '/a' }, 'https://x.com'); });
  t('x.str.base.undef', function () { return new URLPattern().exec('https://x.com/a', undefined).inputs; });
  t('x.num', function () { return new URLPattern().test(5); });
  t('x.null', function () { return new URLPattern().test(null); });
  t('x.sym', function () { return new URLPattern().test(Symbol()); });
  t('x.len', function () { return [URLPattern.prototype.test.length, URLPattern.prototype.exec.length]; });
  t('x.inputs.identity', function () { var o = { pathname: '/a' }; var r = new URLPattern({ pathname: '/a' }).exec(o); return [r.inputs[0] === o, r.inputs.length]; });
  t('x.result.shape', function () { var r = new URLPattern({ pathname: '/:a' }).exec('https://x.com/z'); return [Object.keys(r), Object.keys(r.pathname), Object.getPrototypeOf(r) === Object.prototype, Object.getPrototypeOf(r.pathname.groups) === Object.prototype, Object.keys(r.protocol.groups)]; });
  t('x.groups.undef', function () { var r = new URLPattern({ pathname: '/a/:b?' }).exec({ pathname: '/a' }); return [Object.keys(r.pathname.groups), r.pathname.groups.b, 'b' in r.pathname.groups]; });
  t('x.groups.order', function () { var r = new URLPattern({ pathname: '/:z/:a/(.*)/:m' }).exec({ pathname: '/1/2/3/4' }); return Object.keys(r.pathname.groups); });
  R.count = n;
  return JSON.stringify(R);
})()
