// Chrome 157 sync HTML insertion. The safe methods and Document.parseHTML
// run the Sanitizer (default, or the one in SetHTMLOptions). The Unsafe
// twins parse like innerHTML and keep script, handlers, and javascript: URLs.
// A parentless before/after/replaceWith is a no-op, matching Chrome.
(() => {
  // These stay out even after allowElement (probed on Chrome 157).
  const BLOCKED = new Set(['script', 'iframe', 'object', 'embed', 'base', 'frame']);
  const URL_ATTR = new Set(['href', 'cite', 'src', 'action', 'formaction']);
  const mark = globalThis._markNative || ((f) => f);
  function defaultConfig() {
    const g = new Sanitizer().get();
    return {
      elements: g.elements,
      attributes: g.attributes,
      comments: g.comments,
      dataAttributes: g.dataAttributes,
      processingInstructions: g.processingInstructions || [],
      replaced: g.replaceWithChildrenElements || [],
    };
  }
  function readConfig(options, method, iface) {
    if (options === undefined || options === null) return defaultConfig();
    const pre = "Failed to execute '" + method + "' on '" + iface + "': ";
    if (typeof options !== 'object') {
      throw new TypeError(pre + "The provided value is not of type 'SetHTMLOptions'.");
    }
    if (!('sanitizer' in options) || options.sanitizer === undefined) return defaultConfig();
    const san = options.sanitizer;
    if (san === 'default') return defaultConfig();
    if (typeof Sanitizer === 'function' && san instanceof Sanitizer) {
      const g = san.get();
      return {
        elements: g.elements,
        attributes: g.attributes,
        comments: g.comments,
        dataAttributes: g.dataAttributes,
        processingInstructions: g.processingInstructions || [],
        replaced: g.replaceWithChildrenElements || [],
      };
    }
    if (san !== null && typeof san === 'object') return defaultConfigFrom(san);
    const shown = typeof san === 'string' ? san : String(san);
    throw new TypeError(pre + "Failed to read the 'sanitizer' property from 'SetHTMLOptions': The provided value '" + shown + "' is not a valid enum value of type SanitizerPresets.");
  }
  function defaultConfigFrom(dict) {
    const g = new Sanitizer(dict).get();
    return {
      elements: g.elements,
      attributes: g.attributes,
      comments: g.comments,
      dataAttributes: g.dataAttributes,
      processingInstructions: g.processingInstructions || [],
      replaced: g.replaceWithChildrenElements || [],
    };
  }
  function named(list, name) {
    if (!list) return false;
    for (const item of list) if (item && item.name === name) return true;
    return false;
  }
  function jsUrl(name, value) {
    return URL_ATTR.has(name) && /^\s*javascript:/i.test(String(value));
  }
  function attrOk(cfg, el, attr) {
    const name = attr.name;
    if (/^on/i.test(name)) return false;
    if (cfg.dataAttributes && name.length > 5 && name.startsWith('data-')) return true;
    const rec = cfg.elements.find((e) => e.name === el.localName);
    if (rec && rec.attributes && named(rec.attributes, name) && !jsUrl(name, attr.value)) return true;
    return named(cfg.attributes, name) && !jsUrl(name, attr.value);
  }
  function elementOk(cfg, el) {
    if (BLOCKED.has(el.localName)) return false;
    return named(cfg.elements, el.localName);
  }
  // keepComments is only for the Document node itself: a comment before
  // <html> stays even when the sanitizer's comments flag is false.
  function sanitizeNode(node, cfg, keepComments) {
    if (node.nodeType === 8) {
      if (!keepComments && !cfg.comments) node.remove();
      return;
    }
    if (node.nodeType !== 1) return;
    if (named(cfg.replaced, node.localName) && !BLOCKED.has(node.localName)) {
      const kids = [...node.childNodes];
      node.replaceWith(...kids);
      for (const kid of kids) sanitizeNode(kid, cfg, false);
      return;
    }
    if (!elementOk(cfg, node)) { node.remove(); return; }
    if (node.attributes) {
      for (const attr of [...node.attributes]) {
        if (!attrOk(cfg, node, attr)) node.removeAttribute(attr.name);
      }
    }
    sanitizeChildren(node, cfg, false);
    if (node.localName === 'template' && node.content) sanitizeChildren(node.content, cfg, false);
  }
  function sanitizeChildren(parent, cfg, keepComments) {
    for (const child of [...parent.childNodes]) sanitizeNode(child, cfg, keepComments);
  }
  function parse(html, safe, cfg) {
    const tpl = document.createElement('template');
    tpl.innerHTML = String(html);
    if (safe) sanitizeChildren(tpl.content, cfg || defaultConfig(), false);
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
      const fn = function (html, options) {
        if (this == null || typeof this.nodeType !== 'number') throw new TypeError('Illegal invocation');
        if (arguments.length < 1) {
          throw new TypeError("Failed to execute '" + name + "' on '" + ifaceOf(this) + "': 1 argument required, but only 0 present.");
        }
        const cfg = safe ? readConfig(arguments.length > 1 ? options : undefined, name, ifaceOf(this)) : null;
        const list = parse(html, safe, cfg);
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

  if (typeof Document === 'function' && typeof Document.parseHTML !== 'function') {
    const parseHTML = function parseHTML(html, options) {
      if (arguments.length < 1) {
        throw new TypeError("Failed to execute 'parseHTML' on 'Document': 1 argument required, but only " + arguments.length + " present.");
      }
      const cfg = readConfig(arguments.length > 1 ? options : undefined, 'parseHTML', 'Document');
      const doc = new DOMParser().parseFromString(String(html), 'text/html');
      if (doc.doctype) doc.doctype.remove();
      sanitizeChildren(doc, cfg, true);
      return doc;
    };
    Object.defineProperty(parseHTML, 'length', { value: 1, configurable: true });
    mark(parseHTML);
    Object.defineProperty(Document, 'parseHTML', { value: parseHTML, writable: true, enumerable: true, configurable: true });
  }
})();
