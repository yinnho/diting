// Chrome 157 sync HTML insertion. setHTML runs the default sanitizer (the
// probed allow list: safe text structure, table parts, headings). The Unsafe
// twins parse like innerHTML and keep script, handlers, and javascript: URLs.
// Stream variants are not here. A parentless before/after/replaceWith is a
// no-op, matching Chrome.
(() => {
  const ALLOW = new Set(['p','div','span','b','i','a','table','ul','li','br','hr','h1','h2','h3','h4','h5','h6','section','article','svg','math','figure','figcaption','blockquote','pre','code','em','strong','small','sub','sup','mark','wbr','data','time','abbr','cite','q','dfn','kbd','samp','var','ruby','rt','bdi','bdo','del','ins','s','u','caption','thead','tbody','tfoot','tr','th','td','ol','dl','dt','dd','address','main','nav','header','footer','aside']);
  const GLOBAL = new Set(['title', 'dir', 'lang']);
  const EXTRA = {
    a: ['href'],
    blockquote: ['cite'],
    q: ['cite'],
    time: ['datetime'],
    ins: ['datetime'],
    del: ['datetime'],
    td: ['colspan', 'rowspan'],
    th: ['colspan', 'rowspan', 'abbr'],
  };
  const mark = globalThis._markNative || ((f) => f);
  function jsUrl(v) { return /^\s*javascript:/i.test(v); }
  function attrOk(tag, name, value) {
    if (GLOBAL.has(name)) return true;
    const extra = EXTRA[tag];
    if (!extra || extra.indexOf(name) < 0) return false;
    if ((name === 'href' || name === 'cite') && jsUrl(value)) return false;
    return true;
  }
  function sanitize(root) {
    for (const child of [...root.childNodes]) {
      if (child.nodeType === 3) continue;
      if (child.nodeType !== 1 || !ALLOW.has(child.localName)) { child.remove(); continue; }
      for (const attr of [...child.attributes]) {
        if (!attrOk(child.localName, attr.name, attr.value)) child.removeAttribute(attr.name);
      }
      sanitize(child);
    }
  }
  function parse(html, safe) {
    const tpl = document.createElement('template');
    tpl.innerHTML = String(html);
    if (safe) sanitize(tpl.content);
    return [...tpl.content.childNodes];
  }
  function ifaceOf(self) {
    if (typeof ShadowRoot === 'function' && self instanceof ShadowRoot) return 'ShadowRoot';
    if (self.nodeType === 1) return 'Element';
    return 'CharacterData';
  }
  function install(proto, entries) {
    if (!proto) return;
    for (const [name, how, safe] of entries) {
      if (Object.prototype.hasOwnProperty.call(proto, name)) continue;
      const fn = function (html) {
        if (this == null || typeof this.nodeType !== 'number') throw new TypeError('Illegal invocation');
        if (arguments.length < 1) {
          throw new TypeError("Failed to execute '" + name + "' on '" + ifaceOf(this) + "': 1 argument required, but only 0 present.");
        }
        const list = parse(html, safe);
        if ((how === 'before' || how === 'after' || how === 'replaceWith') && !this.parentNode) return;
        if (how === 'replace') this.replaceChildren(...list);
        else this[how](...list);
      };
      Object.defineProperty(fn, 'name', { value: name, configurable: true });
      Object.defineProperty(fn, 'length', { value: 1, configurable: true });
      mark(fn);
      Object.defineProperty(proto, name, { value: fn, writable: true, enumerable: true, configurable: true });
    }
  }
  const SAFE = [
    ['setHTML', 'replace', true],
    ['appendHTML', 'append', true],
    ['prependHTML', 'prepend', true],
    ['beforeHTML', 'before', true],
    ['afterHTML', 'after', true],
    ['replaceWithHTML', 'replaceWith', true],
    ['appendHTMLUnsafe', 'append', false],
    ['prependHTMLUnsafe', 'prepend', false],
    ['beforeHTMLUnsafe', 'before', false],
    ['afterHTMLUnsafe', 'after', false],
    ['replaceWithHTMLUnsafe', 'replaceWith', false],
  ];
  if (typeof Element === 'function') install(Element.prototype, SAFE);
  if (typeof CharacterData === 'function') {
    install(CharacterData.prototype, SAFE.filter(([name]) => name.startsWith('before') || name.startsWith('after') || name.startsWith('replaceWith')));
  }
  if (typeof ShadowRoot === 'function') {
    install(ShadowRoot.prototype, SAFE.filter(([name]) => name.startsWith('set') || name.startsWith('append') || name.startsWith('prepend')));
  }

  // Each write reparses the whole buffer so a tag opened in one chunk and
  // closed in the next stays one element. streamHTML clears its target when
  // the stream is created. Abort leaves what was already inserted.
  function place(state) {
    const parent = state.parent;
    if (!parent) return;
    if (state.mode === 'replace' || state.mode === 'append') parent.append(...state.nodes);
    else for (const n of state.nodes) parent.insertBefore(n, state.marker);
  }
  function openStream(anchor, mode, safe) {
    const state = { buf: '', nodes: [], mode, parent: null, marker: null, opened: false };
    if (mode === 'replace') {
      anchor.replaceChildren();
      state.parent = anchor;
    } else if (mode === 'append') {
      state.parent = anchor;
    } else if (mode === 'prepend') {
      state.parent = anchor;
      state.marker = anchor.firstChild;
    } else if (anchor.parentNode) {
      state.parent = anchor.parentNode;
      state.marker = mode === 'before' ? anchor : anchor.nextSibling;
      if (mode === 'replaceWith') {
        state.marker = anchor.nextSibling;
        anchor.remove();
      }
    }
    return new WritableStream({
      write(chunk) {
        state.buf += String(chunk);
        for (const n of state.nodes) n.remove();
        state.nodes = parse(state.buf, safe);
        place(state);
      },
    });
  }
  function installStream(proto, entries) {
    if (!proto || typeof WritableStream !== 'function') return;
    for (const [name, mode, safe] of entries) {
      if (Object.prototype.hasOwnProperty.call(proto, name)) continue;
      const fn = function () {
        if (this == null || typeof this.nodeType !== 'number') throw new TypeError('Illegal invocation');
        return openStream(this, mode, safe);
      };
      Object.defineProperty(fn, 'name', { value: name, configurable: true });
      Object.defineProperty(fn, 'length', { value: 0, configurable: true });
      mark(fn);
      Object.defineProperty(proto, name, { value: fn, writable: true, enumerable: true, configurable: true });
    }
  }
  const STREAMS = [
    ['streamHTML', 'replace', true],
    ['streamHTMLUnsafe', 'replace', false],
    ['streamAppendHTML', 'append', true],
    ['streamAppendHTMLUnsafe', 'append', false],
    ['streamPrependHTML', 'prepend', true],
    ['streamPrependHTMLUnsafe', 'prepend', false],
    ['streamBeforeHTML', 'before', true],
    ['streamBeforeHTMLUnsafe', 'before', false],
    ['streamAfterHTML', 'after', true],
    ['streamAfterHTMLUnsafe', 'after', false],
    ['streamReplaceWithHTML', 'replaceWith', true],
    ['streamReplaceWithHTMLUnsafe', 'replaceWith', false],
  ];
  if (typeof Element === 'function') installStream(Element.prototype, STREAMS);
  if (typeof CharacterData === 'function') {
    installStream(CharacterData.prototype, STREAMS.filter(([name]) => name.indexOf('Before') >= 0 || name.indexOf('After') >= 0 || name.indexOf('Replace') >= 0));
  }
  if (typeof ShadowRoot === 'function') {
    installStream(ShadowRoot.prototype, STREAMS.filter(([name]) => name.indexOf('HTML') >= 0 && name.indexOf('Before') < 0 && name.indexOf('After') < 0 && name.indexOf('Replace') < 0));
  }
})();
