// HTML reflected-attribute element family fingerprint probe (M4 knife 6).
// Runs in real Chrome AND the diting engine; diffed. For every non-function
// own member of each listed element prototype it records a BEHAVIOR
// FINGERPRINT - default read, reads after property sets of probe values, the
// content attribute the set created (discovered by getAttributeNames diff, so
// the attribute name is truth, never guessed), and reads after raw
// setAttribute of probe values. The generator classifies each member from the
// fingerprint (presence boolean / string / url / unsigned-long / enum ...),
// so reflection semantics are measured, not remembered.
(function () {
  var TAGS = {
    HTMLAnchorElement: 'a', HTMLAreaElement: 'area', HTMLBaseElement: 'base',
    HTMLBRElement: 'br', HTMLDListElement: 'dl', HTMLDirectoryElement: 'dir',
    HTMLEmbedElement: 'embed', HTMLFontElement: 'font', HTMLFormElement: 'form',
    HTMLFrameElement: 'frame', HTMLHRElement: 'hr', HTMLIFrameElement: 'iframe',
    HTMLLegendElement: 'legend', HTMLLinkElement: 'link', HTMLMenuElement: 'menu',
    HTMLMetaElement: 'meta', HTMLModElement: 'ins', HTMLOptGroupElement: 'optgroup',
    HTMLParamElement: 'param', HTMLPreElement: 'pre', HTMLQuoteElement: 'blockquote',
    HTMLScriptElement: 'script', HTMLSourceElement: 'source', HTMLStyleElement: 'style',
    HTMLTableCaptionElement: 'caption', HTMLTimeElement: 'time', HTMLTitleElement: 'title',
    // regression rows for reflectors landed in earlier knives (2: HTMLElement
    // common layer; 5: media/image family)
    HTMLElement: 'div', HTMLImageElement: 'img', HTMLVideoElement: 'video', HTMLMediaElement: 'audio'
  };
  var SET_VALUES = [['str', 'ABC'], ['empty', ''], ['num5', 5], ['neg1', -1], ['true', true], ['false', false],
    ['rel', '/rel/p?q=1'], ['big', 2147483648], ['num1.9', 1.9], ['null', null], ['undef', undefined]];
  var ATTR_VALUES = ['', 'ABC', 'abc', '5', '-3', '9px', '1e2', '2147483648', '99999999999', 'true', 'false',
    '/rel/p?q=1', 'http://x.test/a b', 'anonymous', 'use-credentials'];

  // Enumerated-attribute vocabulary: a SUPERSET of every HTML keyword the
  // family's enumerated reflections could accept. For members that normalize
  // their value, each keyword is tried through setAttribute and the read-back
  // recorded, so the known list / default / invalid-default are measured.
  var VOCAB = ['', 'on', 'off', 'auto', 'manual', 'eager', 'lazy', 'high', 'low', 'no-referrer',
    'no-referrer-when-downgrade', 'origin', 'origin-when-cross-origin', 'same-origin', 'strict-origin',
    'strict-origin-when-cross-origin', 'unsafe-url', 'get', 'post', 'dialog', 'application/x-www-form-urlencoded',
    'multipart/form-data', 'text/plain', 'audio', 'audioworklet', 'document', 'embed', 'fetch', 'font', 'frame',
    'iframe', 'image', 'json', 'manifest', 'object', 'paintworklet', 'report', 'script', 'serviceworker',
    'sharedworker', 'style', 'track', 'video', 'worker', 'xslt', 'anonymous', 'use-credentials', 'sync', 'async',
    'true', 'false', 'yes', 'no', 'rect', 'circle', 'poly', 'default', 'rectangle', 'circ', 'polygon', 'yes', 'no',
    'render-blocking', 'left', 'right', 'center', 'justify', 'top', 'bottom', 'middle',
    'none', 'sentences', 'words', 'characters', 'enter', 'done', 'go', 'next', 'previous', 'search', 'send', 'text',
    'tel', 'url', 'email', 'numeric', 'decimal', 'ltr', 'rtl', 'metadata', 'plaintext-only', 'inherit'];
  // DOMTokenList-backed members: which tokens does supports() accept?
  var TOKENS = ['render-blocking', 'noopener', 'noreferrer', 'opener', 'stylesheet', 'preload', 'modulepreload',
    'icon', 'allow-forms', 'allow-scripts', 'allow-same-origin', 'allow-popups', 'allow-modals', 'allow-downloads',
    'nofollow', 'prefetch', 'dns-prefetch', 'preconnect', 'canonical', 'alternate', 'author', 'bookmark', 'external',
    'help', 'license', 'next', 'prev', 'search', 'tag', 'privacy-policy', 'terms-of-service', 'manifest',
    'compression-dictionary', 'expect', 'pingback', 'nonexistent', 'nodownload', 'nofullscreen', 'noremoteplayback'];
  var R = { order: {}, kinds: {}, fnlen: {}, fp: {}, extra: {}, tokens: {}, routing: {} };

  function san(v) {
    if (v === undefined) return '#undefined';
    if (v === null) return null;
    if (typeof v === 'number') return isNaN(v) ? '#NaN' : v;
    if (typeof v === 'object' || typeof v === 'function') return 'OBJ:' + ((v.constructor && v.constructor.name) || '?');
    // the probe page's origin differs per run/server: normalize it so URL
    // reflections diff on structure, not on an ephemeral port
    // the probe page's own URL -> '#B' (base), its origin -> '#O': the same
    // fingerprint then diffs equal across Chrome's ephemeral server and the
    // test harness page (http://example.com/test)
    if (typeof v === 'string' && /^https?:/.test(location.origin)) {
      if (v.indexOf(location.href) >= 0) v = v.split(location.href).join('#B');
      if (v.indexOf(location.origin) >= 0) v = v.split(location.origin).join('#O');
      return v;
    }
    return v;
  }
  function read(el, m) { try { return san(el[m]); } catch (e) { return 'THROW ' + e.name + ': ' + String(e.message).slice(0, 80); } }
  function mk(tag) { return document.createElement(tag); }
  function attrs(el) { var o = {}; el.getAttributeNames().forEach(function (n) { o[n] = el.getAttribute(n); }); return o; }

  // instance routing: createElement(tag) must BE an instance of its interface
  // (the 14 element interfaces that were bare HTMLElement before knife 6)
  Object.keys(TAGS).forEach(function (iface) {
    var C = window[iface], el = mk(TAGS[iface]), pr = Object.getPrototypeOf(el);
    R.routing[iface] = { ctor: typeof C, inst: typeof C === 'function' ? el instanceof C : null,
      protoIsIface: typeof C === 'function' ? pr === C.prototype : null,
      protoCtor: pr && pr.constructor && pr.constructor.name };
  });

  Object.keys(TAGS).forEach(function (iface) {
    var C = window[iface];
    if (typeof C !== 'function') { R.order[iface] = 'MISSING'; return; }
    var P = C.prototype;
    var names = Object.getOwnPropertyNames(P);
    R.order[iface] = names;
    var kinds = {}, fnlen = {}, fps = {};
    names.forEach(function (m) {
      var d = Object.getOwnPropertyDescriptor(P, m);
      if (d.get || d.set) kinds[m] = d.set ? 'rw' : 'ro';
      else if (typeof d.value === 'function') { kinds[m] = 'function'; fnlen[m] = d.value.length; }
      else kinds[m] = typeof d.value;
      if (m === 'constructor' || !(d.get || d.set)) return;

      var fp = { def: read(mk(TAGS[iface]), m), set: {}, fromAttr: {} };
      if (d.set) {
        var created = null;
        SET_VALUES.forEach(function (sv) {
          var el = mk(TAGS[iface]);
          var before = attrs(el);
          var thrown = null;
          try { el[m] = sv[1]; } catch (e) { thrown = e.name + ': ' + String(e.message).slice(0, 120); }
          var after = attrs(el);
          var diff = null;
          Object.keys(after).forEach(function (k) { if (!(k in before) || before[k] !== after[k]) diff = [k, after[k]]; });
          Object.keys(before).forEach(function (k) { if (!(k in after)) diff = [k, null]; });
          if (diff && !created && diff[1] !== null) created = diff[0];
          fp.set[sv[0]] = thrown ? { throw: thrown } : { read: read(el, m), attr: diff };
        });
        if (created) {
          fp.attr = created;
          ATTR_VALUES.forEach(function (av) {
            var el = mk(TAGS[iface]);
            el.setAttribute(created, av);
            fp.fromAttr[av] = read(el, m);
          });
          var rel = fp.fromAttr['/rel/p?q=1'];
          var isUrl = typeof rel === 'string' && rel.indexOf('http') === 0;
          if (typeof fp.def === 'string' && fp.fromAttr['ABC'] !== 'ABC' && !isUrl) {
            fp.vocab = {};
            VOCAB.forEach(function (v) {
              var el = mk(TAGS[iface]);
              el.setAttribute(created, v);
              fp.vocab[v] = read(el, m);
              var up = v.toUpperCase();
              if (up !== v && !(up in fp.vocab)) {
                var e2 = mk(TAGS[iface]);
                e2.setAttribute(created, up);
                fp.vocab[up] = read(e2, m);
              }
            });
          }
        }
      }
      if (fp.def === 'OBJ:DOMTokenList') {
        var tl = mk(TAGS[iface])[m], so = {};
        TOKENS.forEach(function (t) { try { so[t] = tl.supports(t); } catch (e) { so[t] = 'THROW ' + e.name; } });
        R.tokens[iface + '.' + m] = so;
      }
      fps[m] = fp;
    });
    R.kinds[iface] = kinds;
    R.fnlen[iface] = fnlen;
    R.fp[iface] = fps;
  });
  // knife-2 string reflectors (HTMLElement level) under non-string inputs:
  // DOMString conversion is String(v) - null reads 'null', undefined
  // 'undefined' (Symbol throws TypeError).
  ['title', 'lang', 'accessKey'].forEach(function (m) {
    var o = {};
    [['null', null], ['undef', undefined], ['num', 7], ['true', true]].forEach(function (sv) {
      var el = document.createElement('div');
      el[m] = sv[1];
      o[sv[0]] = el[m];
    });
    try { document.createElement('div')[m] = Symbol('x'); o.sym = 'no throw'; }
    catch (e) { o.sym = e.name + ': ' + e.message; }
    R.extra['HTMLElement.' + m] = o;
  });
  return JSON.stringify(R);
})()
