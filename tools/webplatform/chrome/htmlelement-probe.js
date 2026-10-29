// HTMLElement/Element common-member probe (M4 knife 2). Runs in real Chrome
// AND in the diting engine; diffed. Returns a JSON string.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + (e && e.name && e.name !== (e.constructor && e.constructor.name) ? '(' + e.name + ')' : '') + ': ' + (e && e.message); } }
  function lvls(P) {
    var o = {};
    Object.getOwnPropertyNames(P).forEach(function (k) {
      var d = Object.getOwnPropertyDescriptor(P, k);
      o[k] = d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value;
    });
    return o;
  }
  // descriptor kinds + OWN ORDER (unsorted) — the generator's install order
  t('HT.order', function () { return Object.getOwnPropertyNames(HTMLElement.prototype); });
  t('HT.kinds', function () { return lvls(HTMLElement.prototype); });
  t('EL.order', function () { return Object.getOwnPropertyNames(Element.prototype); });
  t('EL.kinds', function () { return lvls(Element.prototype); });
  var div = document.createElement('div');
  document.body.appendChild(div);
  // reflected-attribute defaults and roundtrips
  t('def', function () { return { draggable: div.draggable, inert: div.inert, spellcheck: div.spellcheck, translate: div.translate, hidden: div.hidden, lang: div.lang, dir: div.dir, accessKey: div.accessKey, accessKeyLabel: div.accessKeyLabel, autocapitalize: div.autocapitalize, enterKeyHint: div.enterKeyHint, inputMode: div.inputMode, nonce: div.nonce, contentEditable: div.contentEditable, isContentEditable: div.isContentEditable, writingSuggestions: div.writingSuggestions, autofocus: div.autofocus, tabIndex: div.tabIndex, popover: div.popover, title: div.title, outerText: div.outerText, innerText: div.innerText, elementTiming: div.elementTiming, currentCSSZoom: div.currentCSSZoom, clientLeft: div.clientLeft, clientTop: div.clientTop, prefix: div.prefix }; });
  t('set.draggable', function () { div.draggable = true; return [div.draggable, div.getAttribute('draggable')]; });
  t('set.draggable.false', function () { div.draggable = false; return [div.draggable, div.getAttribute('draggable')]; });
  t('set.inert', function () { div.inert = 1; return [div.inert, div.getAttribute('inert')]; });
  t('set.inert.zero', function () { div.inert = 0; return [div.inert, div.getAttribute('inert')]; });
  t('set.inherit', function () { div.inherit = 'x'; return div.inherit; });
  t('set.translate', function () { div.translate = 'yes'; return [div.translate, div.getAttribute('translate')]; });
  t('set.translate.bad', function () { div.translate = 'maybe'; return [div.translate, div.getAttribute('translate')]; });
  t('set.hidden', function () { div.hidden = ''; return [div.hidden, div.getAttribute('hidden')]; });
  t('set.dir', function () { div.dir = 'RTL'; return [div.dir, div.getAttribute('dir')]; });
  t('set.dir.empty', function () { div.dir = ''; return [div.dir, div.getAttribute('dir')]; });
  t('set.lang', function () { div.lang = 42; return [div.lang, div.getAttribute('lang')]; });
  t('set.title', function () { div.title = 42; return [div.title, div.getAttribute('title')]; });
  t('set.accessKey', function () { div.accessKey = 'jk'; return [div.accessKey, div.getAttribute('accesskey')]; });
  t('set.autocapitalize', function () { div.autocapitalize = 'WORDS'; return [div.autocapitalize, div.getAttribute('autocapitalize')]; });
  t('set.autocapitalize.bad', function () { div.autocapitalize = 'nonsense'; return [div.autocapitalize, div.getAttribute('autocapitalize')]; });
  t('set.enterKeyHint', function () { div.enterKeyHint = 'GO'; return [div.enterKeyHint, div.getAttribute('enterkeyhint')]; });
  t('set.inputMode', function () { div.inputMode = 'NUMERIC'; return [div.inputMode, div.getAttribute('inputmode')]; });
  t('set.nonce', function () { div.nonce = 'abc'; return [div.nonce, div.getAttribute('nonce')]; });
  t('set.contentEditable', function () { div.contentEditable = 'TRUE'; return [div.contentEditable, div.getAttribute('contenteditable'), div.isContentEditable]; });
  t('set.contentEditable.bad', function () { div.contentEditable = 'weird'; return [div.contentEditable, div.getAttribute('contenteditable')]; });
  t('set.writingSuggestions', function () { div.writingSuggestions = 'no'; return [div.writingSuggestions, div.getAttribute('writingsuggestions')]; });
  t('set.elementTiming', function () { div.elementTiming = 'x'; return [div.elementTiming, div.getAttribute('elementtiming')]; });
  t('set.autofocus', function () { div.autofocus = 'yes'; return [div.autofocus, div.getAttribute('autofocus')]; });
  t('set.spellcheck', function () { div.spellcheck = 'true'; return [div.spellcheck, div.getAttribute('spellcheck')]; });
  t('set.spellcheck.bad', function () { div.spellcheck = 'maybe'; return [div.spellcheck, div.getAttribute('spellcheck')]; });
  t('set.popover', function () { div.popover = 'AUTO'; return [div.popover, div.getAttribute('popover')]; });
  t('set.popover.manual', function () { div.popover = 'manual'; return [div.popover, div.getAttribute('popover')]; });
  t('set.popover.bad', function () { div.popover = 'weird'; return [div.popover, div.getAttribute('popover')]; });
  t('set.popover.empty', function () { div.popover = ''; return [div.popover, div.getAttribute('popover')]; });
  t('set.tabIndex', function () { div.tabIndex = '3'; return div.tabIndex; });
  t('set.outerText', function () { var d = document.createElement('div'); d.textContent = 'old'; document.body.appendChild(d); d.outerText = 'NEW'; return [d.outerText, d.parentNode.tagName]; });
  t('showPopover.plain', function () { return div.showPopover(); });
  t('hidePopover.plain', function () { return div.hidePopover(); });
  t('togglePopover.plain', function () { return div.togglePopover(); });
  t('attachInternals.plain', function () { return div.attachInternals(); });
  t('hasAttributeNS', function () { var d = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); d.setAttributeNS('http://www.w3.org/1999/xlink', 'href', 'x'); return d.hasAttributeNS('http://www.w3.org/1999/xlink', 'href'); });
  t('classList.set', function () { var d = document.createElement('div'); d.classList = 'a b'; return d.className; });
  t('outerHTML.set', function () { var d = document.createElement('span'); d.outerHTML = '<b>x</b>'; return [d.tagName, typeof d.outerHTML]; });
  t('fn.length', function () { return { click: HTMLElement.prototype.click.length, focus: HTMLElement.prototype.focus.length, blur: HTMLElement.prototype.blur.length, showPopover: HTMLElement.prototype.showPopover.length, togglePopover: HTMLElement.prototype.togglePopover.length, attachInternals: HTMLElement.prototype.attachInternals.length, hasAttributeNS: Element.prototype.hasAttributeNS.length, requestFullscreen: Element.prototype.requestFullscreen ? Element.prototype.requestFullscreen.length : 'none', scrollIntoViewIfNeeded: Element.prototype.scrollIntoViewIfNeeded.length, moveBefore: Element.prototype.moveBefore ? Element.prototype.moveBefore.length : 'none' }; });
  t('offset.simple', function () { var outer = document.createElement('div'); var inner = document.createElement('p'); document.body.appendChild(outer); outer.appendChild(inner); return [typeof inner.offsetParent, inner.offsetTop === 0, inner.offsetLeft === 0, inner.offsetWidth === 0, inner.offsetHeight === 0, inner.offsetParent === null]; });
  // on* OWNERSHIP per level (first owner wins) — decides the knife's
  // relocation targets: engine installs all 98 on Element.prototype, Chrome
  // owns ~110 on HTMLElement.prototype, a few on Element, all also on
  // SVGElement.prototype.
  t('on.levels', function () {
    var names = Object.getOwnPropertyNames(HTMLElement.prototype).concat(Object.getOwnPropertyNames(Element.prototype)).filter(function (n) { return /^on./.test(n); });
    var levels = [HTMLElement.prototype, SVGElement.prototype, Element.prototype, Document.prototype];
    var lnames = ['HT', 'SVG', 'EL', 'DOC'];
    var o = {};
    names.forEach(function (n) {
      for (var i = 0; i < levels.length; i++) { if (Object.prototype.hasOwnProperty.call(levels[i], n)) { o[n] = (o[n] ? o[n] + '+' : '') + lnames[i]; } }
    });
    return o;
  });
  t('ce.plaintextonly', function () { div.contentEditable = 'plaintext-only'; return [div.contentEditable, div.isContentEditable, div.getAttribute('contenteditable')]; });
  t('ce.inherit', function () { div.contentEditable = 'true'; div.contentEditable = 'inherit'; return [div.contentEditable, div.isContentEditable, div.getAttribute('contenteditable')]; });
  t('dir.attr.invalid', function () { div.setAttribute('dir', 'weird'); return div.dir; });
  t('spellcheck.attr.invalid', function () { div.setAttribute('spellcheck', 'x'); return div.spellcheck; });
  t('autocapitalize.attr.upper', function () { div.setAttribute('autocapitalize', 'WORDS'); return div.autocapitalize; });
  t('writingSuggestions.attr.invalid', function () { div.setAttribute('writingsuggestions', 'zz'); return div.writingSuggestions; });
  t('hasAttributeNS.cases', function () { var d = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); d.setAttributeNS('http://www.w3.org/1999/xlink', 'href', 'x'); return [d.hasAttributeNS(null, 'href'), d.hasAttributeNS('', 'href'), d.hasAttributeNS('http://www.w3.org/1999/xlink', 'HREF')]; });
  t('webkitMatchesSelector', function () { return [typeof Element.prototype.webkitMatchesSelector, div.webkitMatchesSelector('div'), Element.prototype.webkitMatchesSelector.length]; });
  document.body.removeChild(div);
  return JSON.stringify(R);
})()
