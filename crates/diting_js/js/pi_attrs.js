// Chrome 157: ProcessingInstruction pseudo-attributes. Only name="value"
// pairs count (unquoted tokens are ignored, names are case-sensitive).
// Writing re-serializes with double quotes and escapes & < > ".
(() => {
  const P = typeof ProcessingInstruction === 'function' ? ProcessingInstruction.prototype : null;
  if (!P) return;
  const mark = globalThis._markNative || ((f) => f);
  const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const unesc = (s) => s.replace(/&(?:quot|amp|lt|gt|apos);/g, (m) => ({ '&quot;': '"', '&amp;': '&', '&lt;': '<', '&gt;': '>', '&apos;': "'" })[m]);
  function pairs(data) {
    const out = [];
    const re = /([^\s=]+)\s*=\s*"([^"]*)"/g;
    let m;
    while ((m = re.exec(String(data || '')))) out.push([m[1], unesc(m[2])]);
    return out;
  }
  function write(self, list) {
    self.data = list.map(([n, v]) => n + '="' + esc(v) + '"').join(' ');
  }
  function me(self) {
    if (!(self instanceof ProcessingInstruction)) throw new TypeError('Illegal invocation');
    return self;
  }
  function need(args, n, name) {
    if (args.length < n) throw new TypeError("Failed to execute '" + name + "' on 'ProcessingInstruction': " + n + " argument" + (n > 1 ? "s" : "") + " required, but only " + args.length + " present.");
  }
  function put(name, len, fn) {
    if (Object.prototype.hasOwnProperty.call(P, name)) return;
    Object.defineProperty(fn, 'name', { value: name, configurable: true });
    Object.defineProperty(fn, 'length', { value: len, configurable: true });
    mark(fn);
    Object.defineProperty(P, name, { value: fn, writable: true, enumerable: true, configurable: true });
  }
  put('getAttribute', 1, function (name) {
    const self = me(this); need(arguments, 1, 'getAttribute');
    const hit = pairs(self.data).find(([n]) => n === String(name));
    return hit ? hit[1] : null;
  });
  put('getAttributeNames', 0, function () {
    return pairs(me(this).data).map(([n]) => n);
  });
  put('hasAttribute', 1, function (name) {
    const self = me(this); need(arguments, 1, 'hasAttribute');
    return pairs(self.data).some(([n]) => n === String(name));
  });
  put('hasAttributes', 0, function () {
    return pairs(me(this).data).length > 0;
  });
  put('removeAttribute', 1, function (name) {
    const self = me(this); need(arguments, 1, 'removeAttribute');
    const key = String(name);
    write(self, pairs(self.data).filter(([n]) => n !== key));
  });
  put('setAttribute', 2, function (name, value) {
    const self = me(this); need(arguments, 2, 'setAttribute');
    const key = String(name);
    const list = pairs(self.data);
    const i = list.findIndex(([n]) => n === key);
    if (i >= 0) list[i][1] = String(value);
    else list.push([key, String(value)]);
    write(self, list);
  });
  put('toggleAttribute', 1, function (name, force) {
    const self = me(this); need(arguments, 1, 'toggleAttribute');
    const key = String(name);
    const list = pairs(self.data);
    const i = list.findIndex(([n]) => n === key);
    const on = arguments.length > 1 ? Boolean(force) : i < 0;
    if (on) { if (i < 0) list.push([key, '']); }
    else if (i >= 0) list.splice(i, 1);
    write(self, list);
  });
  // Chrome order: target, sheet, the seven methods, constructor.
  for (const n of ['target', 'sheet', 'getAttribute', 'getAttributeNames', 'hasAttribute', 'hasAttributes', 'removeAttribute', 'setAttribute', 'toggleAttribute', 'constructor']) {
    const d = Object.getOwnPropertyDescriptor(P, n);
    if (d && d.configurable) { delete P[n]; Object.defineProperty(P, n, d); }
  }
})();
