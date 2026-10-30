// XPath 1.0 probe (M4 knife 21): a corpus of expressions evaluated against a fixed document, results
// serialized structurally (node-sets as nodeName#id/text descriptors in order). Covers all axes, node
// tests, predicates, operators, the core function library, result-type behaviour, error wording and
// the XPathResult/XPathExpression/XPathEvaluator faces. Runs in Chrome AND the engine.
(function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : (typeof x === 'number' ? (isNaN(x) ? '#NaN' : !isFinite(x) ? '#' + x : (x === 0 && 1 / x < 0 ? '#-0' : x)) : x); })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 260); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) {
    var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), len: C.length, statics: Object.getOwnPropertyNames(C).filter(function (k) { return k !== 'length' && k !== 'name' && k !== 'prototype'; }), kinds: {}, tag: Object.prototype.toString.call(Object.create(P)) };
    s.order.forEach(function (m) { s.kinds[m] = kind(P, m); });
    return s;
  }
  ['XPathResult', 'XPathExpression', 'XPathEvaluator'].forEach(function (n) { R['shape.' + n] = typeof globalThis[n] === 'function' ? shape(globalThis[n]) : 'absent'; });
  // Normalize the host page so counts/ids do not depend on the harness (Chrome's has a <pre id=o> and a
  // <meta charset>, the unit-test page has neither): detach everything, restore at the end.
  Array.prototype.slice.call(document.querySelectorAll('#root')).forEach(function (e) { e.remove(); }); // the engine's probe page persists across runs
  var savedHead = Array.prototype.slice.call(document.head.childNodes), savedBody = Array.prototype.slice.call(document.body.childNodes);
  savedHead.forEach(function (n) { n.remove(); }); savedBody.forEach(function (n) { n.remove(); });
  var root = document.createElement('div'); root.id = 'root';
  root.innerHTML = '<ul id="list" class="l a"><li id="i1" class="x" data-n="1">one</li><li id="i2" class="y" data-n="2">two <b id="b1">bold</b> tail</li><li id="i3" class="x y" data-n="3">three</li><li id="i4" data-n="10"></li></ul>' +
    '<p id="p1" lang="en-US">Hello <i>there</i>, <!--c1--> world</p><p id="p2" lang="fr">Bonjour</p>' +
    '<table id="t"><tbody><tr id="r1"><td>a</td><td>b</td></tr><tr id="r2"><td>c</td><td>d</td></tr></tbody></table>' +
    '<span id="s1">  spaced   out  text </span><a id="a1" href="#z" name="nm">link</a><input id="in1" value="v1">';
  document.body.appendChild(root);
  function desc(n) {
    if (n === null) return null;
    if (n.nodeType === 1) return n.nodeName + (n.id ? '#' + n.id : '');
    if (n.nodeType === 2) return '@' + n.name + '=' + n.value;
    if (n.nodeType === 3) return '"' + n.data.slice(0, 12) + '"';
    if (n.nodeType === 8) return '<!--' + n.data + '-->';
    if (n.nodeType === 9) return '#document';
    return 'n' + n.nodeType;
  }
  function ev(expr, ctx, type) {
    var r = document.evaluate(expr, ctx || root, null, type === undefined ? 0 : type, null);
    switch (r.resultType) {
      case 1: return ['num', r.numberValue];
      case 2: return ['str', r.stringValue];
      case 3: return ['bool', r.booleanValue];
      case 4: case 5: var o = [], n; while ((n = r.iterateNext())) o.push(desc(n)); return ['it' + r.resultType, o];
      case 6: case 7: var s = []; for (var i = 0; i < r.snapshotLength; i++) s.push(desc(r.snapshotItem(i))); return ['snap' + r.resultType, s];
      case 8: case 9: return ['one' + r.resultType, desc(r.singleNodeValue)];
    }
    return 'type' + r.resultType;
  }
  function E(name, expr, ctx, type) { t(name, function () { return ev(expr, ctx, type); }); }
  var $ = function (id) { return document.getElementById(id); };
  // ---- location paths and axes
  var PATHS = ['ul', 'ul/li', './ul/li', '//li', '/html/body', '/html/body/div', 'ul/li[1]', 'ul/li[last()]', 'ul/li[position()>2]', 'ul/li[position()=2 or position()=3]', 'ul/li[@class="x"]', 'ul/li[@id]', 'ul/li[@data-n>1]',
    'ul/li[@data-n=10]', 'ul/li[b]', 'ul/li[not(b)]', 'ul/li[.="one"]', 'ul/li[text()="three"]', 'ul/li[contains(.,"o")]', '//li[@class="x y"]', '//*[@id="b1"]', '//b', '//b/..', '//b/parent::li', '//b/ancestor::ul', '//b/ancestor-or-self::*',
    '//li[2]/following-sibling::li', '//li[3]/preceding-sibling::li', '//li[3]/preceding-sibling::li[1]', '//li[2]/following::*', '//li[3]/preceding::li', '//ul/descendant::b', '//ul/descendant-or-self::*', '//ul/self::ul', '//ul/self::div',
    '//li/@id', '//li/attribute::class', '//@id', '//@*', '//li[1]/@*', 'ul/li/text()', '//p/text()', '//p/node()', '//p/comment()', '//comment()', '//text()[.="Bonjour"]', 'p/i', 'p[1]/i/text()', '//p[@lang]', '//p[starts-with(@lang,"en")]', '//p[lang("en")]', '//p[lang("fr")]',
    '//tr/td', '//tr[2]/td[1]', '//td[1]', '(//td)[1]', '(//td)[last()]', '(//td)[position()<3]', '//tr/td[position()=last()]', '//table//td', '//tr[td="c"]', '//tr[td[2]="b"]/@id', '*', 'node()', 'ul | p', 'ul | p[1] | table', '//li | //b', '//b | //li', '(//li | //b)[2]',
    '/', '/*', '/descendant::body', '//body/*[1]', '.', '..', './/b', '//ul//b', 'ul/li[2]/b/text()', 'ul/li[position()>1][1]', 'ul/li[2][b]', 'ul/li[last()-1]', 'ul/li[position() mod 2 = 1]', '//li[@data-n>=2][@data-n<=3]', '//*[self::b or self::i]', '//*[@class]', '//*[contains(@class,"x")]',
    '//li[normalize-space(.)="one"]', '//*[local-name()="b"]', '//*[name()="LI"]', '//*[name()="li"]', '//li[string-length(.)>3]', '//li[count(b)=1]', '//ul[count(li)=4]', '//a[@name]', '//a[@href="#z"]/@name', 'id("i1")', 'id("i1 i3")', 'id("i1")/following-sibling::li[1]', 'id(//li[1]/@id)',
    '//input/@value', '//input[@value="v1"]', '//span[1]/text()', '//li[. = "two  tail"]', '//li[2]/text()[2]', 'ul/li[@id="i2"]/b', 'ul/li[@id!="i2"]', '//li[@id and @class]', '//li[@id and not(@class)]'];
  PATHS.forEach(function (p, i) { E('path.' + p, p, root, 7); });
  E('path.fromdoc', '//li', document, 7);
  E('path.fromattr', '..', $('i1').getAttributeNode('id'), 7);
  E('path.attr.ancestor', 'ancestor::*', $('i1').getAttributeNode('id'), 7);
  E('path.attr.self', 'self::node()', $('i1').getAttributeNode('id'), 7);
  E('path.text.ctx', 'parent::*', $('i2').firstChild, 7);
  E('path.text.sib', 'following-sibling::node()', $('i2').firstChild, 7);
  E('path.abs.fromdeep', '/html/body/div/ul/li[1]', $('b1'), 7);
  E('path.rev.pos', 'preceding-sibling::li[1]/@id', $('i3'), 7);
  E('path.rev.pos2', 'preceding::li[2]/@id', $('i4'), 7);
  // ---- scalar expressions and functions
  var SCALARS = ['1', '1+2', '5-3', '2*3', '7 div 2', '7 mod 3', '-7 mod 3', '7 mod -3', '-(-3)', '1 - -1', '1 div 0', '-1 div 0', '0 div 0', '0 = -0', '1 < 2', '2 <= 2', '3 > 4', '1 = 1', '1 != 1', '"a" = "a"', '"a" != "b"', '"a" < "b"', '1 = "1"', 'true() = 1', 'true() and false()', 'true() or false()', 'not(true())', 'not(1)', 'not("")', 'not(//zz)', 'boolean(//li)', 'boolean(//zz)', 'boolean("")', 'boolean("a")', 'boolean(0)', 'boolean(0 div 0)',
    'count(//li)', 'count(//zz)', 'count(//li/@id)', 'sum(//li/@data-n)', 'sum(//zz)', 'sum(//b)', 'string(//li[1])', 'string(//li[2])', 'string(//zz)', 'string(1.5)', 'string(10)', 'string(-0)', 'string(1 div 0)', 'string(0 div 0)', 'string(true())', 'string(1e3)', 'string(0.000001)', 'string(123456789012345678)', 'string(1.0)', 'string(.1)', 'string(1234567)', 'string(123456)', 'string(100000000)', 'string(0.1234567)', 'string(1 div 3)', 'string(1234567.891)', 'string(-1234567)', 'string(0.00001234567)', 'string(1000000*1000000*1000)', 'string(2147483648)', 'string(4294967296)', 'string(2147483647)', 'string(-2147483648)', 'string(0.1+0.2)', 'string(100000*10)', 'string(999999)', 'string(1000000)', 'string(1000001)', 'string(12345.6789)', 'string(123456.7)', 'string(1234567.5)', 'string(0.0000001)', 'string(0.00000123456789)', 'string(1000000000000)', 'string(123456789 * 1000)', 'string(-0.5)', 'string(3.14159265358979)', 'string(1 div 7)', 'string(100 div 3)', 'string(1000000 div 3)', 'string(10000000 div 3)', 'concat(1 div 3, "")', 'string(5 div 3)', 'string(16777216)', 'string(16777217)',
    'number("12")', 'number(" 12 ")', 'number("1e3")', 'number("x")', 'number("")', 'number(true())', 'number(//li[1]/@data-n)', 'number(//li[4]/@data-n)', 'number(//zz)', 'number("-3.5")', 'number("+3")', 'number(".5")', 'number("5.")', 'number("0x10")', 'number("Infinity")',
    'concat("a","b")', 'concat("a","b","c","d")', 'concat(1,2)', 'starts-with("hello","he")', 'starts-with("hello","")', 'contains("hello","ll")', 'contains("hello","")', 'contains("","a")', 'substring-before("a-b-c","-")', 'substring-after("a-b-c","-")', 'substring-before("abc","z")', 'substring-after("abc","z")', 'substring-after("abc","")',
    'substring("12345",2)', 'substring("12345",2,3)', 'substring("12345",0)', 'substring("12345",0,3)', 'substring("12345",-1)', 'substring("12345",1.5,2.6)', 'substring("12345",0 div 0)', 'substring("12345",1,0 div 0)', 'substring("12345",1,1 div 0)', 'substring("12345",-1 div 0,1 div 0)', 'substring("12345",5,2)', 'substring("12345",6)',
    'string-length("abc")', 'string-length("")', 'string-length(//li[1])', 'string-length()', 'normalize-space("  a  b  ")', 'normalize-space("")', 'normalize-space(//span)', 'normalize-space()', 'translate("abc","abc","ABC")', 'translate("abc","ab","A")', 'translate("abc","aa","AB")', 'translate("--aaa--","a-","A")', 'translate("abc","","x")',
    'floor(1.5)', 'floor(-1.5)', 'ceiling(1.2)', 'ceiling(-1.2)', 'round(1.5)', 'round(2.5)', 'round(-1.5)', 'round(-2.5)', 'round(0.4)', 'round(-0.4)', 'round(1 div 0)', 'round(0 div 0)', 'floor(0 div 0)', 'floor("x")',
    'local-name(//li[1])', 'local-name(//zz)', 'local-name()', 'name(//li[1])', 'name(//@id)', 'name(/)', 'namespace-uri(//li[1])', 'namespace-uri(//zz)', 'lang("en")', 'lang("EN")', 'lang("e")', 'last()', 'position()', 'true()', 'false()',
    '"a" = //zz', '"x" = //li', '"one" = //li', '"zz" != //li', '//li = "one"', '//li/@data-n = 2', '//li/@data-n > 9', '//li/@data-n < 2', '//li/@data-n >= 10', '//li/@id = //li/@class', '//zz = //zz', '//zz != //zz', '//li = //zz', 'true() = "a"', 'false() = ""', '1 = true()', '0 = false()', '"1" = true()', '//li = true()', '//zz = false()', '//zz = true()',
    '1 < "2"', '"a" < 1', '"10" > "9"', '"abc" < "abd"', 'true() < 2', '//li/@data-n < //li/@data-n', '1 + "2"', '"3" * "4"', '1 - //li[1]/@data-n', '//li[1] + 1', '- //li[1]/@data-n', '5 mod 0', '5 div 3', '1 + 2 * 3', '(1 + 2) * 3', '2 * 3 mod 4', '10 div 2 div 5', '1 < 2 < 3', '3 > 2 > 1', '1 = 1 = 1', '1 or 0 and 0', '(1 or 0) and 0', '1 and 2 and 0',
    'count(//li) + count(//p)', 'count(//li | //p)', 'count(//li | //li)', 'count(/descendant::*)', 'count(//*)', 'count(//@*)', 'count(id("root")//text())', 'count(id("root")//node())', 'count(//comment())', 'count(//processing-instruction())', 'count(id("i1 i2 zz"))', 'count(//p[1]/node())', 'count(/html/*)', 'sum(//li/@data-n) div count(//li)', 'string-length(string(//li[2]))'];
  SCALARS.forEach(function (s) { E('sc.' + s, s, root, 0); });
  // ---- requested result types
  var TYPES = [[1, 'count(//li)'], [2, 'count(//li)'], [3, 'count(//li)'], [1, '"12"'], [2, '1+1'], [3, '"x"'], [1, '//li[1]/@data-n'], [2, '//li[2]'], [3, '//li'], [3, '//zz'], [1, '//zz'], [2, '//zz'],
    [4, '//li'], [5, '//li'], [6, '//li'], [7, '//li'], [8, '//li'], [9, '//li'], [8, '//zz'], [9, '//zz'], [4, '//zz'], [7, '//zz'], [4, '1'], [5, 'true()'], [6, '"s"'], [7, '1'], [8, '1'], [9, '"x"'], [4, '//li | //b'], [7, '//b | //li'], [9, '//li | //b'], [8, '//b | //li'], [0, '//li'], [0, 'count(//li)'], [0, '"s"'], [0, '1=1'], [10, '//li'], [-1, '//li'], [4.9, '//li'], ['7', '//li']];
  TYPES.forEach(function (p, i) { E('type.' + i + '.' + p[0] + '.' + p[1], p[1], root, p[0]); });
  // ---- result accessors across types
  t('res.access', function () { var r = document.evaluate('//li', root, null, 7, null); var o = []; ['numberValue', 'stringValue', 'booleanValue', 'singleNodeValue'].forEach(function (k) { try { o.push(String(r[k])); } catch (e) { o.push(e.name + ': ' + e.message); } }); try { r.iterateNext(); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('res.access.num', function () { var r = document.evaluate('1+1', root, null, 1, null); var o = []; ['snapshotLength', 'stringValue', 'booleanValue', 'singleNodeValue', 'invalidIteratorState'].forEach(function (k) { try { o.push(String(r[k])); } catch (e) { o.push(e.name + ': ' + e.message); } }); try { r.snapshotItem(0); } catch (e) { o.push(e.name + ': ' + e.message); } try { r.iterateNext(); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('res.access.it', function () { var r = document.evaluate('//li', root, null, 4, null); var o = []; ['snapshotLength', 'numberValue', 'stringValue', 'booleanValue', 'singleNodeValue'].forEach(function (k) { try { o.push(String(r[k])); } catch (e) { o.push(e.name + ': ' + e.message); } }); try { r.snapshotItem(0); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('res.access.one', function () { var r = document.evaluate('//li', root, null, 9, null); var o = []; ['snapshotLength', 'numberValue', 'stringValue', 'booleanValue'].forEach(function (k) { try { o.push(String(r[k])); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('res.snap.edges', function () { var r = document.evaluate('//li', root, null, 7, null); return [r.snapshotItem(-1), r.snapshotItem(99) === null, desc(r.snapshotItem(0)), desc(r.snapshotItem('1')), desc(r.snapshotItem(1.9)), r.snapshotLength, r.resultType, r.invalidIteratorState]; });
  t('res.iter.exhaust', function () { var r = document.evaluate('//li', root, null, 4, null); var n = 0; while (r.iterateNext()) n++; return [n, r.iterateNext(), r.invalidIteratorState]; });
  t('res.iter.mutate', function () { var d = document.createElement('div'); d.innerHTML = '<i></i><i></i>'; document.body.appendChild(d); var r = document.evaluate('i', d, null, 5, null); var a = r.iterateNext() !== null; d.appendChild(document.createElement('i')); try { r.iterateNext(); return [a, 'noerr', r.invalidIteratorState]; } catch (e) { return [a, e.name + ': ' + e.message, r.invalidIteratorState]; } finally { d.remove(); } });
  t('res.iter.mutate.unrelated', function () { var d = document.createElement('div'); d.innerHTML = '<i></i>'; document.body.appendChild(d); var r = document.evaluate('i', d, null, 5, null); document.body.appendChild(document.createElement('hr')).remove(); try { r.iterateNext(); return 'noerr'; } catch (e) { return e.name; } finally { d.remove(); } });
  t('res.snap.mutate', function () { var d = document.createElement('div'); d.innerHTML = '<i></i>'; document.body.appendChild(d); var r = document.evaluate('i', d, null, 7, null); d.appendChild(document.createElement('i')); var l = r.snapshotLength; d.remove(); return [l, r.invalidIteratorState]; });
  t('res.reuse', function () { var r = document.evaluate('//li', root, null, 7, null); var r2 = document.evaluate('//b', root, null, 7, r); return [r === r2, r.snapshotLength, r.resultType]; });
  t('res.reuse.bad', function () { try { return document.evaluate('//li', root, null, 7, {}) ? 'ok' : 'null'; } catch (e) { return e.name + ': ' + e.message; } });
  t('res.consts', function () { var o = {}; ['ANY_TYPE', 'NUMBER_TYPE', 'STRING_TYPE', 'BOOLEAN_TYPE', 'UNORDERED_NODE_ITERATOR_TYPE', 'ORDERED_NODE_ITERATOR_TYPE', 'UNORDERED_NODE_SNAPSHOT_TYPE', 'ORDERED_NODE_SNAPSHOT_TYPE', 'ANY_UNORDERED_NODE_TYPE', 'FIRST_ORDERED_NODE_TYPE'].forEach(function (k) { o[k] = [XPathResult[k], Object.getOwnPropertyDescriptor(XPathResult, k).enumerable, Object.getOwnPropertyDescriptor(XPathResult, k).writable, XPathResult.prototype[k]]; }); return o; });
  t('res.ctor', function () { try { new XPathResult(); } catch (e) { return e.name + ': ' + e.message; } });
  t('res.tag', function () { var r = document.evaluate('1', root, null, 1, null); return [Object.prototype.toString.call(r), r instanceof XPathResult, Object.getOwnPropertyNames(r)]; });
  t('res.brand', function () { var o = []; ['resultType', 'snapshotLength', 'numberValue', 'singleNodeValue'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(XPathResult.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['iterateNext', 'snapshotItem'].forEach(function (m) { try { XPathResult.prototype[m].call({}, 0); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  // ---- errors
  var BAD = ['', ' ', '/*/', '//', 'li[', 'li[1', 'li]', '1 +', '(1', '1)', '@', '@@id', 'child::', 'foo::bar', 'li/', '/li/', '..//', '1 2', 'a b', '"abc', "'abc", '$x', '$x + 1', 'f(', 'unknown()', 'unknown(1)', 'count()', 'count(1)', 'count("a")', 'count(1,2)', 'not()', 'not(1,2)', 'contains("a")', 'contains("a","b","c")', 'substring("a")', 'concat("a")', 'concat()', 'translate("a","b")', 'true(1)', 'last(1)', 'position(1)', 'id()', 'sum(1)', 'sum()', 'lang()', 'local-name(1)', 'name("a")', 'string(1,2)', 'normalize-space(1,2)', 'string-length(1,2)', 'number(1,2)', 'floor()', 'round(1,2)', 'boolean()', 'li[]', '//li[@]', '/ /', '1 = = 1', '*:x', 'a:b', 'a:*', '*:*', '|', 'a |', '| a', '1 | 2', '"a" | "b"', 'count(//li) | 1', 'ancestor::', '::a', '/..', '/.', './/', 'text(1)', 'node(1)', 'comment(1)', 'processing-instruction(1)', 'processing-instruction("a","b")', 'node', 'text', 'comment()()', '///a', 'a///b', './/.', '...', '.5', '5.', '1.2.3', '1e3', '0x1', '١', 'é', 'aé', 'éa', 'a-b', 'a.b', 'a_b', '_a', '-a', '1a', 'a1', 'div', 'mod', 'and', 'or', 'div div div', 'and and and', 'or or or', 'a and', 'and a', '* * *', '* div *', 'child::*', 'child::node()', 'descendant::*', '@*', 'a/@*', 'a/@b/c', '(a)(b)', '(a)[1]', '(a)[1][2]', '(a)/b', '(a)//b', 'a[1](b)', '(1)[1]', '"a"[1]', '1[1]', '(1)/a', '"a"/a'];
  BAD.forEach(function (b) { t('bad.' + JSON.stringify(b), function () { return ev(b, root, 0); }); });
  t('bad.nullexpr', function () { return ev(null, root, 0); });
  t('bad.undefexpr', function () { return ev(undefined, root, 0); });
  t('bad.noargs', function () { try { document.evaluate(); } catch (e) { return e.name + ': ' + e.message; } });
  t('bad.onearg', function () { try { document.evaluate('1'); } catch (e) { return e.name + ': ' + e.message; } });
  t('bad.ctx', function () { var o = []; [null, undefined, {}, 5, 'x'].forEach(function (c) { try { var r = document.evaluate('1', c, null, 1, null); o.push(r.numberValue); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('bad.ctx.nodes', function () { var o = []; [document.createDocumentFragment(), document.createTextNode('x'), document.createComment('c'), document.implementation.createDocumentType('html', '', ''), document.createElement('div')].forEach(function (c) { try { var r = document.evaluate('self::node()', c, null, 7, null); o.push(r.snapshotLength); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('bad.resolver', function () { var o = []; [{}, 5, function () {}, { lookupNamespaceURI: 5 }].forEach(function (rs) { try { document.evaluate('1', root, rs, 1, null); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  // ---- namespaces
  t('ns.prefix.unresolved', function () { try { return ev('x:li', root, 0); } catch (e) { return e.name + ': ' + e.message; } });
  t('ns.prefix.resolver', function () { var r = document.evaluate('h:li', root, function (p) { return p === 'h' ? 'http://www.w3.org/1999/xhtml' : null; }, 7, null); return r.snapshotLength; });
  t('ns.prefix.resolver.obj', function () { var r = document.evaluate('h:li', root, { lookupNamespaceURI: function (p) { return p === 'h' ? 'http://www.w3.org/1999/xhtml' : null; } }, 7, null); return r.snapshotLength; });
  t('ns.prefix.resolver.star', function () { var r = document.evaluate('//h:*', root, function (p) { return p === 'h' ? 'http://www.w3.org/1999/xhtml' : null; }, 7, null); return r.snapshotLength > 5; });
  t('ns.prefix.wrongns', function () { var r = document.evaluate('h:li', root, function (p) { return 'urn:other'; }, 7, null); return r.snapshotLength; });
  t('ns.prefix.nullreturn', function () { try { return document.evaluate('h:li', root, function (p) { return null; }, 7, null).snapshotLength; } catch (e) { return e.name + ': ' + e.message; } });
  t('ns.createNSResolver', function () { var r = document.createNSResolver(root); return [r === root, typeof r.lookupNamespaceURI]; });
  t('ns.createNSResolver.bad', function () { try { document.createNSResolver(); } catch (e) { return e.name + ': ' + e.message; } });
  t('ns.createNSResolver.node', function () { var r = document.createNSResolver(root); return document.evaluate('h:li', root, r, 7, null).snapshotLength; });
  t('ns.svg', function () { var d = document.createElement('div'); d.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg"><circle id="c1"/></svg>'; document.body.appendChild(d); var a = document.evaluate('//circle', d, null, 7, null).snapshotLength; var b = document.evaluate('//s:circle', d, function (p) { return p === 's' ? 'http://www.w3.org/2000/svg' : null; }, 7, null).snapshotLength; var c = document.evaluate('//*[local-name()="circle"]', d, null, 7, null).snapshotLength; var n = document.evaluate('namespace-uri(//*[local-name()="circle"])', d, null, 2, null).stringValue; d.remove(); return [a, b, c, n]; });
  t('ns.html.case', function () { return [ev('//LI', root, 7)[1].length, ev('//Li', root, 7)[1].length, ev('//li', root, 7)[1].length, ev('//*[@ID]', root, 7)[1].length, ev('//*[@id]', root, 7)[1].length]; });
  // ---- XPathExpression / XPathEvaluator
  t('expr.create', function () { var e = document.createExpression('//li'); var r = e.evaluate(root, 7, null); return [Object.prototype.toString.call(e), e instanceof XPathExpression, r.snapshotLength, Object.getOwnPropertyNames(e)]; });
  t('expr.evaluate.defaults', function () { var e = document.createExpression('count(//li)'); var r = e.evaluate(root); return [r.resultType, r.numberValue]; });
  t('expr.reuse', function () { var e = document.createExpression('li'); return [e.evaluate($('list'), 7).snapshotLength, e.evaluate($('t'), 7).snapshotLength, e.evaluate($('list'), 7).snapshotLength]; });
  t('expr.bad', function () { try { document.createExpression('li['); } catch (e) { return e.name + ': ' + e.message; } });
  t('expr.noargs', function () { try { document.createExpression(); } catch (e) { return e.name + ': ' + e.message; } });
  t('expr.evaluate.noargs', function () { try { document.createExpression('1').evaluate(); } catch (e) { return e.name + ': ' + e.message; } });
  t('expr.evaluate.badctx', function () { try { document.createExpression('1').evaluate({}); } catch (e) { return e.name + ': ' + e.message; } });
  t('expr.resolver', function () { var e = document.createExpression('h:li', function (p) { return p === 'h' ? 'http://www.w3.org/1999/xhtml' : null; }); return e.evaluate(root, 7).snapshotLength; });
  t('expr.brand', function () { try { XPathExpression.prototype.evaluate.call({}, root); } catch (e) { return e.name + ': ' + e.message; } });
  t('expr.ctor', function () { try { new XPathExpression(); } catch (e) { return e.name + ': ' + e.message; } });
  t('evalr.ctor', function () { var e = new XPathEvaluator(); return [Object.prototype.toString.call(e), Object.getOwnPropertyNames(e)]; });
  t('evalr.call', function () { try { XPathEvaluator(); } catch (e) { return e.name + ': ' + e.message; } });
  t('evalr.evaluate', function () { var e = new XPathEvaluator(); var r = e.evaluate('//li', root, null, 7, null); return r.snapshotLength; });
  t('evalr.createExpression', function () { var e = new XPathEvaluator(); return e.createExpression('count(//li)').evaluate(root, 1, null).numberValue; });
  t('evalr.createNSResolver', function () { var e = new XPathEvaluator(); return e.createNSResolver(root) === root; });
  t('evalr.brand', function () { var o = []; ['evaluate', 'createExpression', 'createNSResolver'].forEach(function (m) { try { XPathEvaluator.prototype[m].call({}, '1', root); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('evalr.lens', function () { return [XPathEvaluator.prototype.evaluate.length, XPathEvaluator.prototype.createExpression.length, XPathEvaluator.prototype.createNSResolver.length, document.evaluate.length, document.createExpression.length, document.createNSResolver.length, XPathExpression.prototype.evaluate.length]; });
  t('evalr.docfaces', function () { var d = Object.getOwnPropertyNames(Document.prototype); return [d.indexOf('createExpression') >= 0, d.indexOf('createNSResolver') >= 0, d.indexOf('evaluate') >= 0]; });
  t('ctx.detached', function () { var d = document.createElement('div'); d.innerHTML = '<p>1</p><p>2</p>'; return [document.evaluate('p', d, null, 7, null).snapshotLength, document.evaluate('//p', d, null, 7, null).snapshotLength, document.evaluate('/p', d, null, 7, null).snapshotLength, document.evaluate('..', d, null, 7, null).snapshotLength, document.evaluate('ancestor::*', d, null, 7, null).snapshotLength]; });
  t('ctx.detached.abs', function () { var d = document.createElement('div'); d.innerHTML = '<p>1</p>'; try { return document.evaluate('/*', d, null, 7, null).snapshotLength; } catch (e) { return e.name + ': ' + e.message; } });
  t('order.union', function () { return ev('//li[3] | //li[1] | //li[2]', root, 7); });
  t('order.rev', function () { return ev('//li[4]/preceding-sibling::*', root, 7); });
  t('order.dedupe', function () { return ev('//li | //li[2] | //ul/li', root, 7); });
  t('order.ancestors', function () { return ev('//b/ancestor::*', root, 7); });
  t('order.following', function () { return ev('//li[1]/following::*', root, 7).slice(0, 2); });
  t('order.attrs', function () { return ev('//li[1]/@*', root, 7); });
  t('order.attrs.multi', function () { return ev('//ul/@*', root, 7); });
  t('order.attr.vs.child', function () { return ev('//ul/@* | //ul/li[1]', root, 7); });
  t('order.iter', function () { return ev('//li | //b', root, 4); });
  root.remove();
  savedHead.forEach(function (n) { document.head.appendChild(n); }); savedBody.forEach(function (n) { document.body.appendChild(n); });
  return JSON.stringify(R);
})()
