// Chrome WebGL extension-object surface (per extension name, both context types): interface tag, getExtension
// caching, and the prototype members with descriptor flags. Needs a Chrome with WebGL (DITING_CHROME_WEBGL=1 with
// probe_run.py chrome). Feeds tools/webplatform/gen/gen_webgl_ext.py.
(function(){
const out = { gl1: {}, gl2: {} };
for (const [key, type] of [['gl1', 'webgl'], ['gl2', 'webgl2']]) {
  const gl = document.createElement('canvas').getContext(type);
  const names = gl.getSupportedExtensions();
  out[key + '_supported'] = names;
  for (const n of names) {
    const e = gl.getExtension(n);
    const proto = Object.getPrototypeOf(e);
    const members = [];
    for (const k of Reflect.ownKeys(proto)) {
      if (typeof k === 'symbol') continue;
      const d = Object.getOwnPropertyDescriptor(proto, k);
      const fl = (d.enumerable ? 'e' : '-') + (d.configurable ? 'c' : '-') + ('writable' in d && d.writable ? 'w' : '-');
      if (typeof d.value === 'function') members.push([k, 'f', d.value.length, fl]);
      else if ('value' in d) members.push([k, 'n', d.value, fl]);
      else members.push([k, 'a']);
    }
    out[key][n] = { tag: Object.prototype.toString.call(e), same: gl.getExtension(n) === e, ownKeys: Reflect.ownKeys(e).length,
      protoProto: Object.getPrototypeOf(proto) === Object.prototype, protoTag: Object.prototype.toString.call(proto), hasCtor: Object.prototype.hasOwnProperty.call(proto, 'constructor'), members };
  }
  out[key + '_unknown'] = gl.getExtension('NOPE_not_real');
}
return JSON.stringify(out);
})()
