#!/usr/bin/env python3
# HISTORICAL ONE-SHOT (kept for provenance, NOT run by check_fresh.sh): this
# splices tables INTO bootstrap.js by regex and was applied once; bootstrap has
# moved on since. The GL tables now live in bootstrap.js as the artifact; the
# truth capture is tools/webplatform/chrome/chrome-gl-const-tables.json.
# P8 generator: splice Chrome-truth GL constant/method tables (captured from
# Chrome 155.0.8059.41) into bootstrap.js, replacing the partial _GL_CONSTS
# block, and rewrite the per-canvas GL instance to a zero-own-property shape.
import json, re, sys

BS = "crates/diting/js/bootstrap.js"
T = json.load(open("tools/webplatform/chrome/chrome-gl-const-tables.json"))

def js_consts(obj):
    items = [f"{k}: {v}" for k, v in obj.items()]
    return ", ".join(items)

def js_methods(names):
    keep = [f"'{n}'" for n in names if n != "constructor"]
    return ", ".join(keep)

NEW_BLOCK = """\
// (P8 #300015) Chrome-truth GL interface tables (captured verbatim from
// Chrome 155.0.8059.41/macOS): WebGL1 prototype carries 443 own names
// (298 constants + 139 methods incl. constructor + 6 accessors), WebGL2 792.
// The context INSTANCE carries ZERO own properties — `Object.keys(gl) === []`,
// every constant resolves through the chain. Constants install as own props
// of the prototype: enumerable, NON-writable, NON-configurable; methods
// enumerable+writable+configurable. Generated from
// tools/webplatform/chrome/chrome-gl-const-tables.json by tools/webplatform/gen/gen_gl_tables.py.
const _GL_CONSTS1 = { %(c1)s };
const _GL_CONSTS2 = { %(c2)s };
const _GL_METHODS1 = [%(m1)s];
const _GL_METHODS2 = [%(m2)s];
// Return values for the handful of methods detectors actually chain-read;
// everything else in the tables is a true void no-op (undefined — Chrome
// methods that "return nothing" give undefined, never 0).
const _GL_CREATE = function() { return {}; };
const _GL_RET = {
  createBuffer: _GL_CREATE, createShader: _GL_CREATE, createProgram: _GL_CREATE,
  createTexture: _GL_CREATE, createFramebuffer: _GL_CREATE, createRenderbuffer: _GL_CREATE,
  createQuery: _GL_CREATE, createSampler: _GL_CREATE, createTransformFeedback: _GL_CREATE,
  createVertexArray: _GL_CREATE, fenceSync: _GL_CREATE,
  getProgramParameter: function() { return true; }, getShaderParameter: function() { return true; },
  getUniformLocation: function() { return {}; },
  getAttribLocation: function() { return 0; },
  getActiveUniform: function() { return null; }, getActiveAttrib: function() { return null; },
  getActiveUniforms: function() { return []; }, getUniform: function() { return null; },
  getProgramInfoLog: function() { return ''; }, getShaderInfoLog: function() { return ''; },
  getFramebufferAttachmentParameter: function() { return 0; },
  getRenderbufferParameter: function() { return 0; },
  getInternalformatParameter: function() { return []; },
  getIndexedParameter: function() { return 0; },
  getSyncParameter: function() { return 0; },
  getTransformFeedbackVarying: function() { return null; },
  getVertexAttrib: function() { return 0; }, getVertexAttribOffset: function() { return 0; },
  getFramebufferStatus: function() { return 0x8CD5; }, checkFramebufferStatus: function() { return 0x8CD5; },
  getError: function() { return 0; },
  isContextLost: function() { return false; },
  isBuffer: function() { return false; }, isFramebuffer: function() { return false; },
  isProgram: function() { return false; }, isQuery: function() { return false; },
  isRenderbuffer: function() { return false; }, isSampler: function() { return false; },
  isShader: function() { return false; }, isSync: function() { return false; },
  isTexture: function() { return false; }, isTransformFeedback: function() { return false; },
  isVertexArray: function() { return false; }, isEnabled: function() { return false; },
  getContextAttributes: function() { return { alpha: true, antialias: true, depth: true, stencil: false, failIfMajorPerformanceCaveat: false, powerPreference: 'default', preserveDrawingBuffer: false, desynchronized: false, premultipliedAlpha: true }; },
};
// Chrome's extension objects expose their constants through the PROTOTYPE,
// never as own props (Object.keys(ext) === []; for..in sees them). ctor name
// stays "Object" because the carrier proto has no own constructor.
function _glExtCarrier(consts) {
  const p = {};
  for (const k in consts) Object.defineProperty(p, k, { value: consts[k], enumerable: true });
  return Object.create(p);
}
const _GL_EXT_DEBUG = _glExtCarrier({ UNMASKED_VENDOR_WEBGL: 0x9245, UNMASKED_RENDERER_WEBGL: 0x9246 });
const _GL_EXT_ANISO = _glExtCarrier({ MAX_TEXTURE_MAX_ANISOTROPY_EXT: 0x84FF });
const _GL_EXT_LOSE = _glExtCarrier({});
Object.defineProperty(Object.getPrototypeOf(_GL_EXT_LOSE), 'loseContext', { value: function() {}, enumerable: true });
Object.defineProperty(Object.getPrototypeOf(_GL_EXT_LOSE), 'restoreContext', { value: function() {}, enumerable: true });
for (const [_GLC, _isGL2] of [[globalThis.WebGLRenderingContext, false], [globalThis.WebGL2RenderingContext, true]]) {
  const P = _GLC.prototype;
  const isGL2 = _isGL2;
  const consts = isGL2 ? _GL_CONSTS2 : _GL_CONSTS1;
  const exts = isGL2 ? _GL_EXTS2 : _GL_EXTS1;
  for (const k in consts) Object.defineProperty(P, k, { value: consts[k], enumerable: true });
  // Introspection faces keep real implementations (#117 lineage) — installed
  // BEFORE the table fill so the generic pass sees them as already-present.
  P.getParameter = function(pname) { return _glParam(pname, isGL2); };
  P.getExtension = function(name) {
    if (name === 'WEBGL_debug_renderer_info') return _GL_EXT_DEBUG;
    if (name === 'EXT_texture_filter_anisotropic') return _GL_EXT_ANISO;
    if (name === 'WEBGL_lose_context') return _GL_EXT_LOSE;
    return exts.includes(name) ? _glExtCarrier({}) : null;
  };
  P.getSupportedExtensions = function() { return exts.slice(); };
  P.getShaderPrecisionFormat = function() { return { rangeMin: 127, rangeMax: 127, precision: 23 }; };
  P.bufferData = function() {};
  P.readPixels = function(x,y,w,h,f,t,d) { if(d) for(let i=0;i<d.length;i++) d[i]=0; };
  for (const m of (isGL2 ? _GL_METHODS2 : _GL_METHODS1)) {
    if (Object.prototype.hasOwnProperty.call(P, m)) continue;
    P[m] = _GL_RET[m] || function() {};
  }
  // The six accessor faces (Chrome 155): canvas tracks the creating canvas;
  // drawingBuffer* the buffer at current canvas size; colorSpaces default
  // srgb; drawingBufferFormat is the GLenum GL_RGBA (32856) — a number, not
  // a string.
  const _acc = function(get) { return { get, enumerable: true, configurable: true }; };
  Object.defineProperty(P, 'canvas', _acc(function() { return this._canvas || null; }));
  Object.defineProperty(P, 'drawingBufferWidth', _acc(function() { return this._canvas ? this._canvas.width : 300; }));
  Object.defineProperty(P, 'drawingBufferHeight', _acc(function() { return this._canvas ? this._canvas.height : 150; }));
  Object.defineProperty(P, 'drawingBufferColorSpace', _acc(function() { return 'srgb'; }));
  Object.defineProperty(P, 'unpackColorSpace', _acc(function() { return 'srgb'; }));
  Object.defineProperty(P, 'drawingBufferFormat', _acc(function() { return 0x8018; }));
}
""" % {"c1": js_consts(T["consts1"]), "c2": js_consts(T["consts2"]),
       "m1": js_methods(T["methods1"]), "m2": js_methods(T["methods2"])}

src = open(BS).read()

# --- 1. replace the old constants + install-loop block ---
start_anchor = "// GL constants. Real browsers expose these on the interface PROTOTYPE (WebIDL"
end_anchor = "// (#117) Chrome's screen carries ZERO own enumerable props"
si = src.index(start_anchor)
ei = src.index(end_anchor)
old_block = src[si:ei]
# sanity: the old block must contain the pieces we expect to retire
for probe in ("_GL_CONSTS = {", "Object.assign(_GLC.prototype, _GL_CONSTS)", "_GL_EXT_DEBUG = {"):
    assert probe in old_block, "anchor drift: " + probe
src = src[:si] + NEW_BLOCK + "\n" + src[ei:]

# --- 2. rewrite the per-canvas GL instance to a zero-own-prop shape ---
inst_start = "  if (type === 'webgl' || type === 'experimental-webgl' || type === 'webgl2') {\n    if (this._glCtx) return this._glCtx;\n    const base = {"
inst_end = "    __def(this, '_ctxType', _ctxNorm);\n    __def(this, '_glCtx', gl);\n    return gl;\n  }\n  return null;\n};"
is_i = src.index(inst_start)
ie_i = src.index(inst_end, is_i) + len(inst_end)
NEW_INST = """  if (type === 'webgl' || type === 'experimental-webgl' || type === 'webgl2') {
    if (this._glCtx) return this._glCtx;
    // (P8 #300015) Chrome shape: the instance carries ZERO own enumerable
    // properties — every method/constant/accessor resolves through the
    // WebGL{,2}RenderingContext.prototype tables; only the hidden _canvas
    // link is instance state. The old base object shadowed ~80 methods and
    // constants as own props, so Object.keys(gl) leaked the whole engine.
    const base = {};
    __def(base, '_canvas', this);
    Object.setPrototypeOf(base, (type === 'webgl2')
      ? globalThis.WebGL2RenderingContext.prototype
      : globalThis.WebGLRenderingContext.prototype);
    // Proxy fallback: any WebGL method a library calls that we didn't
    // enumerate becomes a number-ish no-op instead of throwing TypeError.
    const numNoop = function() { return 0; };
    numNoop.valueOf = () => 0;
    numNoop.toString = () => "0";
    const gl = new Proxy(base, {
      get(target, prop) {
        // Symbols (Symbol.iterator, Symbol.toPrimitive, ...) must fall through
        // to the real value, and 'then' must stay undefined — returning
        // numNoop for it would make the context thenable and break
        // await/Promise.resolve on it.
        if (typeof prop === 'symbol' || prop === 'then' || prop === 'toJSON') return target[prop];
        if (prop in target) return target[prop];
        return numNoop;
      },
    });
    __def(this, '_ctxType', _ctxNorm);
    __def(this, '_glCtx', gl);
    return gl;
  }
  return null;
};"""
src = src[:is_i] + NEW_INST + src[ie_i:]

open(BS, "w").write(src)
print("spliced OK")
print("consts1 entries:", len(T["consts1"]), "| consts2:", len(T["consts2"]))
print("methods1:", len([m for m in T["methods1"] if m != "constructor"]),
      "| methods2:", len([m for m in T["methods2"] if m != "constructor"]))
