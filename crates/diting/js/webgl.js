(function () { // @@SCOPE
// WebGL contexts: parameter/extension/handle tables, per-context state and the
// prototype method install (P8 #300015, 刀69, 刀92), split out of bootstrap.js
// (god-file ratchet). The two interface classes stay in bootstrap; this runs
// right after it, before webgl_ext.js and the sweeps.
// Bot detectors verify these methods exist on the PROTOTYPE (they grab
// `WebGLRenderingContext.prototype.getParameter` etc. to test for API
// tampering); the per-canvas instance methods alone are not enough.
// These prototype versions don't touch `this`, so they also survive being
// called with a fake receiver.
// Shared getParameter implementation with realistic ANGLE values. Array-valued
// pnames must return iterables — detectors spread them (`[...gl.getParameter(gl.MAX_VIEWPORT_DIMS)]`).
// (#117) Masked GL strings captured from a real Chrome 145 (macOS, ANGLE
// Metal). VENDOR/RENDERER are constant across machines; VERSION/SHADING
// differ per context class — the old returns ("OpenGL ES 3.0 (ANGLE)")
// match no Chrome. The unmasked 0x9245/0x9246 stay persona-driven.
function _glParam(pname, isGL2) {
  switch (pname) {
    case 0x9245: return _fp('gpuVendor');
    case 0x9246: return _fp('gpu');
    case 0x1F00: return 'WebKit';
    case 0x1F01: return 'WebKit WebGL';
    case 0x1F02: return isGL2 ? 'WebGL 2.0 (OpenGL ES 3.0 Chromium)' : 'WebGL 1.0 (OpenGL ES 2.0 Chromium)';
    case 0x8B8C: return isGL2 ? 'WebGL GLSL ES 3.00 (OpenGL ES 3.0 Chromium)' : 'WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)';
    case 0x0D3A: return new Int32Array([16384, 16384]);   // MAX_VIEWPORT_DIMS (Apple GPU; 32767 is the llvmpipe softpipe tell, fp-diff-1014 #5)
    case 0x846E: return new Float32Array([1, 1]);        // ALIASED_LINE_WIDTH_RANGE (Metal ANGLE: no wide aliased lines)
    case 0x846D: return new Float32Array([1, 1024]);      // ALIASED_POINT_SIZE_RANGE
    case 0x0B70: return new Float32Array([0, 1]);         // DEPTH_RANGE
    case 0x0C22: return new Float32Array([0, 0, 0, 0]);   // COLOR_CLEAR_VALUE
    case 0x0C2D: return new Float32Array([0, 0, 0, 0]);   // BLEND_COLOR
    case 0x86A3: return new Uint32Array(0);               // COMPRESSED_TEXTURE_FORMATS
    case 0x0D33: return 16384;   // MAX_TEXTURE_SIZE
    case 0x0D38: return 16384;   // MAX_CUBE_MAP_TEXTURE_SIZE
    case 0x84E8: return 16384;   // MAX_RENDERBUFFER_SIZE
    case 0x8869: return 16;      // MAX_VERTEX_ATTRIBS
    case 0x8872: return 16;      // MAX_TEXTURE_IMAGE_UNITS
    case 0x8B4C: return 16;      // MAX_VERTEX_TEXTURE_IMAGE_UNITS
    case 0x8B4D: return 32;      // MAX_COMBINED_TEXTURE_IMAGE_UNITS
    case 0x8DFB: return 4096;    // MAX_VERTEX_UNIFORM_VECTORS
    case 0x8DFD: return 1024;    // MAX_FRAGMENT_UNIFORM_VECTORS
    case 0x8DFC: return 30;      // MAX_VARYING_VECTORS
    case 0x84FF: return 16;      // MAX_TEXTURE_MAX_ANISOTROPY_EXT
    case 0x8513: return 4;       // SAMPLES
    case 0x80A9: return 1;       // SAMPLE_BUFFERS
    case 0x0D50: return 4;       // SUBPIXEL_BITS
    case 0x0D52: case 0x0D53: case 0x0D54: case 0x0D55: return 8;  // R/G/B/A BITS
    case 0x0D56: return 24;      // DEPTH_BITS
    case 0x0D57: return 0;       // STENCIL_BITS
    default: return 0;
  }
}
const _GL_EXTS1 = [
  'ANGLE_instanced_arrays', 'EXT_blend_minmax', 'EXT_clip_control', 'EXT_color_buffer_half_float',
  'EXT_depth_clamp', 'EXT_disjoint_timer_query', 'EXT_float_blend', 'EXT_frag_depth',
  'EXT_polygon_offset_clamp', 'EXT_shader_texture_lod', 'EXT_texture_compression_bptc',
  'EXT_texture_compression_rgtc', 'EXT_texture_filter_anisotropic', 'EXT_texture_mirror_clamp_to_edge',
  'EXT_sRGB', 'KHR_parallel_shader_compile', 'OES_element_index_uint', 'OES_fbo_render_mipmap',
  'OES_standard_derivatives', 'OES_texture_float', 'OES_texture_float_linear', 'OES_texture_half_float',
  'OES_texture_half_float_linear', 'OES_vertex_array_object', 'WEBGL_blend_func_extended',
  'WEBGL_color_buffer_float', 'WEBGL_compressed_texture_astc', 'WEBGL_compressed_texture_etc',
  'WEBGL_compressed_texture_etc1', 'WEBGL_compressed_texture_pvrtc', 'WEBGL_compressed_texture_s3tc',
  'WEBGL_compressed_texture_s3tc_srgb', 'WEBGL_debug_renderer_info', 'WEBGL_debug_shaders',
  'WEBGL_depth_texture', 'WEBGL_draw_buffers', 'WEBGL_lose_context', 'WEBGL_multi_draw',
  'WEBGL_polygon_mode',
];
const _GL_EXTS2 = [
  'EXT_clip_control', 'EXT_color_buffer_float', 'EXT_color_buffer_half_float',
  'EXT_conservative_depth', 'EXT_depth_clamp', 'EXT_disjoint_timer_query_webgl2', 'EXT_float_blend',
  'EXT_polygon_offset_clamp', 'EXT_render_snorm', 'EXT_texture_compression_bptc',
  'EXT_texture_compression_rgtc', 'EXT_texture_filter_anisotropic', 'EXT_texture_mirror_clamp_to_edge',
  'EXT_texture_norm16', 'KHR_parallel_shader_compile', 'NV_shader_noperspective_interpolation',
  'OES_draw_buffers_indexed', 'OES_sample_variables', 'OES_shader_multisample_interpolation',
  'OES_texture_float_linear', 'WEBGL_blend_func_extended', 'WEBGL_clip_cull_distance',
  'WEBGL_compressed_texture_astc', 'WEBGL_compressed_texture_etc', 'WEBGL_compressed_texture_etc1',
  'WEBGL_compressed_texture_pvrtc', 'WEBGL_compressed_texture_s3tc', 'WEBGL_compressed_texture_s3tc_srgb',
  'WEBGL_debug_renderer_info', 'WEBGL_debug_shaders', 'WEBGL_lose_context', 'WEBGL_multi_draw',
  'WEBGL_polygon_mode', 'WEBGL_provoking_vertex', 'WEBGL_render_shared_exponent',
  'WEBGL_stencil_texturing',
];
// (P8 #300015) Chrome-truth GL interface tables (captured verbatim from
// real Chrome 154.0.8037.98/macOS): WebGL1 prototype carries 442 own names
// (298 constants + 138 methods + 6 accessors), WebGL2 791.
// The context INSTANCE carries ZERO own properties — `Object.keys(gl) === []`,
// every constant resolves through the chain. Constants install as own props
// of the prototype: enumerable, NON-writable, NON-configurable; methods
// enumerable+writable+configurable. Generated from
// tools/webplatform/chrome/chrome-gl-const-tables.json by tools/webplatform/gen/gen_gl_tables.py.
const _GL_CONSTS1 = { DEPTH_BUFFER_BIT: 256, STENCIL_BUFFER_BIT: 1024, COLOR_BUFFER_BIT: 16384, POINTS: 0, LINES: 1, LINE_LOOP: 2, LINE_STRIP: 3, TRIANGLES: 4, TRIANGLE_STRIP: 5, TRIANGLE_FAN: 6, ZERO: 0, ONE: 1, SRC_COLOR: 768, ONE_MINUS_SRC_COLOR: 769, SRC_ALPHA: 770, ONE_MINUS_SRC_ALPHA: 771, DST_ALPHA: 772, ONE_MINUS_DST_ALPHA: 773, DST_COLOR: 774, ONE_MINUS_DST_COLOR: 775, SRC_ALPHA_SATURATE: 776, FUNC_ADD: 32774, BLEND_EQUATION: 32777, BLEND_EQUATION_RGB: 32777, BLEND_EQUATION_ALPHA: 34877, FUNC_SUBTRACT: 32778, FUNC_REVERSE_SUBTRACT: 32779, BLEND_DST_RGB: 32968, BLEND_SRC_RGB: 32969, BLEND_DST_ALPHA: 32970, BLEND_SRC_ALPHA: 32971, CONSTANT_COLOR: 32769, ONE_MINUS_CONSTANT_COLOR: 32770, CONSTANT_ALPHA: 32771, ONE_MINUS_CONSTANT_ALPHA: 32772, BLEND_COLOR: 32773, ARRAY_BUFFER: 34962, ELEMENT_ARRAY_BUFFER: 34963, ARRAY_BUFFER_BINDING: 34964, ELEMENT_ARRAY_BUFFER_BINDING: 34965, STREAM_DRAW: 35040, STATIC_DRAW: 35044, DYNAMIC_DRAW: 35048, BUFFER_SIZE: 34660, BUFFER_USAGE: 34661, CURRENT_VERTEX_ATTRIB: 34342, FRONT: 1028, BACK: 1029, FRONT_AND_BACK: 1032, TEXTURE_2D: 3553, CULL_FACE: 2884, BLEND: 3042, DITHER: 3024, STENCIL_TEST: 2960, DEPTH_TEST: 2929, SCISSOR_TEST: 3089, POLYGON_OFFSET_FILL: 32823, SAMPLE_ALPHA_TO_COVERAGE: 32926, SAMPLE_COVERAGE: 32928, NO_ERROR: 0, INVALID_ENUM: 1280, INVALID_VALUE: 1281, INVALID_OPERATION: 1282, OUT_OF_MEMORY: 1285, CW: 2304, CCW: 2305, LINE_WIDTH: 2849, ALIASED_POINT_SIZE_RANGE: 33901, ALIASED_LINE_WIDTH_RANGE: 33902, CULL_FACE_MODE: 2885, FRONT_FACE: 2886, DEPTH_RANGE: 2928, DEPTH_WRITEMASK: 2930, DEPTH_CLEAR_VALUE: 2931, DEPTH_FUNC: 2932, STENCIL_CLEAR_VALUE: 2961, STENCIL_FUNC: 2962, STENCIL_FAIL: 2964, STENCIL_PASS_DEPTH_FAIL: 2965, STENCIL_PASS_DEPTH_PASS: 2966, STENCIL_REF: 2967, STENCIL_VALUE_MASK: 2963, STENCIL_WRITEMASK: 2968, STENCIL_BACK_FUNC: 34816, STENCIL_BACK_FAIL: 34817, STENCIL_BACK_PASS_DEPTH_FAIL: 34818, STENCIL_BACK_PASS_DEPTH_PASS: 34819, STENCIL_BACK_REF: 36003, STENCIL_BACK_VALUE_MASK: 36004, STENCIL_BACK_WRITEMASK: 36005, VIEWPORT: 2978, SCISSOR_BOX: 3088, COLOR_CLEAR_VALUE: 3106, COLOR_WRITEMASK: 3107, UNPACK_ALIGNMENT: 3317, PACK_ALIGNMENT: 3333, MAX_TEXTURE_SIZE: 3379, MAX_VIEWPORT_DIMS: 3386, SUBPIXEL_BITS: 3408, RED_BITS: 3410, GREEN_BITS: 3411, BLUE_BITS: 3412, ALPHA_BITS: 3413, DEPTH_BITS: 3414, STENCIL_BITS: 3415, POLYGON_OFFSET_UNITS: 10752, POLYGON_OFFSET_FACTOR: 32824, TEXTURE_BINDING_2D: 32873, SAMPLE_BUFFERS: 32936, SAMPLES: 32937, SAMPLE_COVERAGE_VALUE: 32938, SAMPLE_COVERAGE_INVERT: 32939, COMPRESSED_TEXTURE_FORMATS: 34467, DONT_CARE: 4352, FASTEST: 4353, NICEST: 4354, GENERATE_MIPMAP_HINT: 33170, BYTE: 5120, UNSIGNED_BYTE: 5121, SHORT: 5122, UNSIGNED_SHORT: 5123, INT: 5124, UNSIGNED_INT: 5125, FLOAT: 5126, DEPTH_COMPONENT: 6402, ALPHA: 6406, RGB: 6407, RGBA: 6408, LUMINANCE: 6409, LUMINANCE_ALPHA: 6410, UNSIGNED_SHORT_4_4_4_4: 32819, UNSIGNED_SHORT_5_5_5_1: 32820, UNSIGNED_SHORT_5_6_5: 33635, FRAGMENT_SHADER: 35632, VERTEX_SHADER: 35633, MAX_VERTEX_ATTRIBS: 34921, MAX_VERTEX_UNIFORM_VECTORS: 36347, MAX_VARYING_VECTORS: 36348, MAX_COMBINED_TEXTURE_IMAGE_UNITS: 35661, MAX_VERTEX_TEXTURE_IMAGE_UNITS: 35660, MAX_TEXTURE_IMAGE_UNITS: 34930, MAX_FRAGMENT_UNIFORM_VECTORS: 36349, SHADER_TYPE: 35663, DELETE_STATUS: 35712, LINK_STATUS: 35714, VALIDATE_STATUS: 35715, ATTACHED_SHADERS: 35717, ACTIVE_UNIFORMS: 35718, ACTIVE_ATTRIBUTES: 35721, SHADING_LANGUAGE_VERSION: 35724, CURRENT_PROGRAM: 35725, NEVER: 512, LESS: 513, EQUAL: 514, LEQUAL: 515, GREATER: 516, NOTEQUAL: 517, GEQUAL: 518, ALWAYS: 519, KEEP: 7680, REPLACE: 7681, INCR: 7682, DECR: 7683, INVERT: 5386, INCR_WRAP: 34055, DECR_WRAP: 34056, VENDOR: 7936, RENDERER: 7937, VERSION: 7938, NEAREST: 9728, LINEAR: 9729, NEAREST_MIPMAP_NEAREST: 9984, LINEAR_MIPMAP_NEAREST: 9985, NEAREST_MIPMAP_LINEAR: 9986, LINEAR_MIPMAP_LINEAR: 9987, TEXTURE_MAG_FILTER: 10240, TEXTURE_MIN_FILTER: 10241, TEXTURE_WRAP_S: 10242, TEXTURE_WRAP_T: 10243, TEXTURE: 5890, TEXTURE_CUBE_MAP: 34067, TEXTURE_BINDING_CUBE_MAP: 34068, TEXTURE_CUBE_MAP_POSITIVE_X: 34069, TEXTURE_CUBE_MAP_NEGATIVE_X: 34070, TEXTURE_CUBE_MAP_POSITIVE_Y: 34071, TEXTURE_CUBE_MAP_NEGATIVE_Y: 34072, TEXTURE_CUBE_MAP_POSITIVE_Z: 34073, TEXTURE_CUBE_MAP_NEGATIVE_Z: 34074, MAX_CUBE_MAP_TEXTURE_SIZE: 34076, TEXTURE0: 33984, TEXTURE1: 33985, TEXTURE2: 33986, TEXTURE3: 33987, TEXTURE4: 33988, TEXTURE5: 33989, TEXTURE6: 33990, TEXTURE7: 33991, TEXTURE8: 33992, TEXTURE9: 33993, TEXTURE10: 33994, TEXTURE11: 33995, TEXTURE12: 33996, TEXTURE13: 33997, TEXTURE14: 33998, TEXTURE15: 33999, TEXTURE16: 34000, TEXTURE17: 34001, TEXTURE18: 34002, TEXTURE19: 34003, TEXTURE20: 34004, TEXTURE21: 34005, TEXTURE22: 34006, TEXTURE23: 34007, TEXTURE24: 34008, TEXTURE25: 34009, TEXTURE26: 34010, TEXTURE27: 34011, TEXTURE28: 34012, TEXTURE29: 34013, TEXTURE30: 34014, TEXTURE31: 34015, ACTIVE_TEXTURE: 34016, REPEAT: 10497, CLAMP_TO_EDGE: 33071, MIRRORED_REPEAT: 33648, FLOAT_VEC2: 35664, FLOAT_VEC3: 35665, FLOAT_VEC4: 35666, INT_VEC2: 35667, INT_VEC3: 35668, INT_VEC4: 35669, BOOL: 35670, BOOL_VEC2: 35671, BOOL_VEC3: 35672, BOOL_VEC4: 35673, FLOAT_MAT2: 35674, FLOAT_MAT3: 35675, FLOAT_MAT4: 35676, SAMPLER_2D: 35678, SAMPLER_CUBE: 35680, VERTEX_ATTRIB_ARRAY_ENABLED: 34338, VERTEX_ATTRIB_ARRAY_SIZE: 34339, VERTEX_ATTRIB_ARRAY_STRIDE: 34340, VERTEX_ATTRIB_ARRAY_TYPE: 34341, VERTEX_ATTRIB_ARRAY_NORMALIZED: 34922, VERTEX_ATTRIB_ARRAY_POINTER: 34373, VERTEX_ATTRIB_ARRAY_BUFFER_BINDING: 34975, IMPLEMENTATION_COLOR_READ_TYPE: 35738, IMPLEMENTATION_COLOR_READ_FORMAT: 35739, COMPILE_STATUS: 35713, LOW_FLOAT: 36336, MEDIUM_FLOAT: 36337, HIGH_FLOAT: 36338, LOW_INT: 36339, MEDIUM_INT: 36340, HIGH_INT: 36341, FRAMEBUFFER: 36160, RENDERBUFFER: 36161, RGBA4: 32854, RGB5_A1: 32855, RGB565: 36194, DEPTH_COMPONENT16: 33189, STENCIL_INDEX8: 36168, DEPTH_STENCIL: 34041, RENDERBUFFER_WIDTH: 36162, RENDERBUFFER_HEIGHT: 36163, RENDERBUFFER_INTERNAL_FORMAT: 36164, RENDERBUFFER_RED_SIZE: 36176, RENDERBUFFER_GREEN_SIZE: 36177, RENDERBUFFER_BLUE_SIZE: 36178, RENDERBUFFER_ALPHA_SIZE: 36179, RENDERBUFFER_DEPTH_SIZE: 36180, RENDERBUFFER_STENCIL_SIZE: 36181, FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE: 36048, FRAMEBUFFER_ATTACHMENT_OBJECT_NAME: 36049, FRAMEBUFFER_ATTACHMENT_TEXTURE_LEVEL: 36050, FRAMEBUFFER_ATTACHMENT_TEXTURE_CUBE_MAP_FACE: 36051, COLOR_ATTACHMENT0: 36064, DEPTH_ATTACHMENT: 36096, STENCIL_ATTACHMENT: 36128, DEPTH_STENCIL_ATTACHMENT: 33306, NONE: 0, FRAMEBUFFER_COMPLETE: 36053, FRAMEBUFFER_INCOMPLETE_ATTACHMENT: 36054, FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT: 36055, FRAMEBUFFER_INCOMPLETE_DIMENSIONS: 36057, FRAMEBUFFER_UNSUPPORTED: 36061, FRAMEBUFFER_BINDING: 36006, RENDERBUFFER_BINDING: 36007, MAX_RENDERBUFFER_SIZE: 34024, INVALID_FRAMEBUFFER_OPERATION: 1286, UNPACK_FLIP_Y_WEBGL: 37440, UNPACK_PREMULTIPLY_ALPHA_WEBGL: 37441, CONTEXT_LOST_WEBGL: 37442, UNPACK_COLORSPACE_CONVERSION_WEBGL: 37443, BROWSER_DEFAULT_WEBGL: 37444, RGB8: 32849, RGBA8: 32856 };
const _GL_CONSTS2 = { DEPTH_BUFFER_BIT: 256, STENCIL_BUFFER_BIT: 1024, COLOR_BUFFER_BIT: 16384, POINTS: 0, LINES: 1, LINE_LOOP: 2, LINE_STRIP: 3, TRIANGLES: 4, TRIANGLE_STRIP: 5, TRIANGLE_FAN: 6, ZERO: 0, ONE: 1, SRC_COLOR: 768, ONE_MINUS_SRC_COLOR: 769, SRC_ALPHA: 770, ONE_MINUS_SRC_ALPHA: 771, DST_ALPHA: 772, ONE_MINUS_DST_ALPHA: 773, DST_COLOR: 774, ONE_MINUS_DST_COLOR: 775, SRC_ALPHA_SATURATE: 776, FUNC_ADD: 32774, BLEND_EQUATION: 32777, BLEND_EQUATION_RGB: 32777, BLEND_EQUATION_ALPHA: 34877, FUNC_SUBTRACT: 32778, FUNC_REVERSE_SUBTRACT: 32779, BLEND_DST_RGB: 32968, BLEND_SRC_RGB: 32969, BLEND_DST_ALPHA: 32970, BLEND_SRC_ALPHA: 32971, CONSTANT_COLOR: 32769, ONE_MINUS_CONSTANT_COLOR: 32770, CONSTANT_ALPHA: 32771, ONE_MINUS_CONSTANT_ALPHA: 32772, BLEND_COLOR: 32773, ARRAY_BUFFER: 34962, ELEMENT_ARRAY_BUFFER: 34963, ARRAY_BUFFER_BINDING: 34964, ELEMENT_ARRAY_BUFFER_BINDING: 34965, STREAM_DRAW: 35040, STATIC_DRAW: 35044, DYNAMIC_DRAW: 35048, BUFFER_SIZE: 34660, BUFFER_USAGE: 34661, CURRENT_VERTEX_ATTRIB: 34342, FRONT: 1028, BACK: 1029, FRONT_AND_BACK: 1032, TEXTURE_2D: 3553, CULL_FACE: 2884, BLEND: 3042, DITHER: 3024, STENCIL_TEST: 2960, DEPTH_TEST: 2929, SCISSOR_TEST: 3089, POLYGON_OFFSET_FILL: 32823, SAMPLE_ALPHA_TO_COVERAGE: 32926, SAMPLE_COVERAGE: 32928, NO_ERROR: 0, INVALID_ENUM: 1280, INVALID_VALUE: 1281, INVALID_OPERATION: 1282, OUT_OF_MEMORY: 1285, CW: 2304, CCW: 2305, LINE_WIDTH: 2849, ALIASED_POINT_SIZE_RANGE: 33901, ALIASED_LINE_WIDTH_RANGE: 33902, CULL_FACE_MODE: 2885, FRONT_FACE: 2886, DEPTH_RANGE: 2928, DEPTH_WRITEMASK: 2930, DEPTH_CLEAR_VALUE: 2931, DEPTH_FUNC: 2932, STENCIL_CLEAR_VALUE: 2961, STENCIL_FUNC: 2962, STENCIL_FAIL: 2964, STENCIL_PASS_DEPTH_FAIL: 2965, STENCIL_PASS_DEPTH_PASS: 2966, STENCIL_REF: 2967, STENCIL_VALUE_MASK: 2963, STENCIL_WRITEMASK: 2968, STENCIL_BACK_FUNC: 34816, STENCIL_BACK_FAIL: 34817, STENCIL_BACK_PASS_DEPTH_FAIL: 34818, STENCIL_BACK_PASS_DEPTH_PASS: 34819, STENCIL_BACK_REF: 36003, STENCIL_BACK_VALUE_MASK: 36004, STENCIL_BACK_WRITEMASK: 36005, VIEWPORT: 2978, SCISSOR_BOX: 3088, COLOR_CLEAR_VALUE: 3106, COLOR_WRITEMASK: 3107, UNPACK_ALIGNMENT: 3317, PACK_ALIGNMENT: 3333, MAX_TEXTURE_SIZE: 3379, MAX_VIEWPORT_DIMS: 3386, SUBPIXEL_BITS: 3408, RED_BITS: 3410, GREEN_BITS: 3411, BLUE_BITS: 3412, ALPHA_BITS: 3413, DEPTH_BITS: 3414, STENCIL_BITS: 3415, POLYGON_OFFSET_UNITS: 10752, POLYGON_OFFSET_FACTOR: 32824, TEXTURE_BINDING_2D: 32873, SAMPLE_BUFFERS: 32936, SAMPLES: 32937, SAMPLE_COVERAGE_VALUE: 32938, SAMPLE_COVERAGE_INVERT: 32939, COMPRESSED_TEXTURE_FORMATS: 34467, DONT_CARE: 4352, FASTEST: 4353, NICEST: 4354, GENERATE_MIPMAP_HINT: 33170, BYTE: 5120, UNSIGNED_BYTE: 5121, SHORT: 5122, UNSIGNED_SHORT: 5123, INT: 5124, UNSIGNED_INT: 5125, FLOAT: 5126, DEPTH_COMPONENT: 6402, ALPHA: 6406, RGB: 6407, RGBA: 6408, LUMINANCE: 6409, LUMINANCE_ALPHA: 6410, UNSIGNED_SHORT_4_4_4_4: 32819, UNSIGNED_SHORT_5_5_5_1: 32820, UNSIGNED_SHORT_5_6_5: 33635, FRAGMENT_SHADER: 35632, VERTEX_SHADER: 35633, MAX_VERTEX_ATTRIBS: 34921, MAX_VERTEX_UNIFORM_VECTORS: 36347, MAX_VARYING_VECTORS: 36348, MAX_COMBINED_TEXTURE_IMAGE_UNITS: 35661, MAX_VERTEX_TEXTURE_IMAGE_UNITS: 35660, MAX_TEXTURE_IMAGE_UNITS: 34930, MAX_FRAGMENT_UNIFORM_VECTORS: 36349, SHADER_TYPE: 35663, DELETE_STATUS: 35712, LINK_STATUS: 35714, VALIDATE_STATUS: 35715, ATTACHED_SHADERS: 35717, ACTIVE_UNIFORMS: 35718, ACTIVE_ATTRIBUTES: 35721, SHADING_LANGUAGE_VERSION: 35724, CURRENT_PROGRAM: 35725, NEVER: 512, LESS: 513, EQUAL: 514, LEQUAL: 515, GREATER: 516, NOTEQUAL: 517, GEQUAL: 518, ALWAYS: 519, KEEP: 7680, REPLACE: 7681, INCR: 7682, DECR: 7683, INVERT: 5386, INCR_WRAP: 34055, DECR_WRAP: 34056, VENDOR: 7936, RENDERER: 7937, VERSION: 7938, NEAREST: 9728, LINEAR: 9729, NEAREST_MIPMAP_NEAREST: 9984, LINEAR_MIPMAP_NEAREST: 9985, NEAREST_MIPMAP_LINEAR: 9986, LINEAR_MIPMAP_LINEAR: 9987, TEXTURE_MAG_FILTER: 10240, TEXTURE_MIN_FILTER: 10241, TEXTURE_WRAP_S: 10242, TEXTURE_WRAP_T: 10243, TEXTURE: 5890, TEXTURE_CUBE_MAP: 34067, TEXTURE_BINDING_CUBE_MAP: 34068, TEXTURE_CUBE_MAP_POSITIVE_X: 34069, TEXTURE_CUBE_MAP_NEGATIVE_X: 34070, TEXTURE_CUBE_MAP_POSITIVE_Y: 34071, TEXTURE_CUBE_MAP_NEGATIVE_Y: 34072, TEXTURE_CUBE_MAP_POSITIVE_Z: 34073, TEXTURE_CUBE_MAP_NEGATIVE_Z: 34074, MAX_CUBE_MAP_TEXTURE_SIZE: 34076, TEXTURE0: 33984, TEXTURE1: 33985, TEXTURE2: 33986, TEXTURE3: 33987, TEXTURE4: 33988, TEXTURE5: 33989, TEXTURE6: 33990, TEXTURE7: 33991, TEXTURE8: 33992, TEXTURE9: 33993, TEXTURE10: 33994, TEXTURE11: 33995, TEXTURE12: 33996, TEXTURE13: 33997, TEXTURE14: 33998, TEXTURE15: 33999, TEXTURE16: 34000, TEXTURE17: 34001, TEXTURE18: 34002, TEXTURE19: 34003, TEXTURE20: 34004, TEXTURE21: 34005, TEXTURE22: 34006, TEXTURE23: 34007, TEXTURE24: 34008, TEXTURE25: 34009, TEXTURE26: 34010, TEXTURE27: 34011, TEXTURE28: 34012, TEXTURE29: 34013, TEXTURE30: 34014, TEXTURE31: 34015, ACTIVE_TEXTURE: 34016, REPEAT: 10497, CLAMP_TO_EDGE: 33071, MIRRORED_REPEAT: 33648, FLOAT_VEC2: 35664, FLOAT_VEC3: 35665, FLOAT_VEC4: 35666, INT_VEC2: 35667, INT_VEC3: 35668, INT_VEC4: 35669, BOOL: 35670, BOOL_VEC2: 35671, BOOL_VEC3: 35672, BOOL_VEC4: 35673, FLOAT_MAT2: 35674, FLOAT_MAT3: 35675, FLOAT_MAT4: 35676, SAMPLER_2D: 35678, SAMPLER_CUBE: 35680, VERTEX_ATTRIB_ARRAY_ENABLED: 34338, VERTEX_ATTRIB_ARRAY_SIZE: 34339, VERTEX_ATTRIB_ARRAY_STRIDE: 34340, VERTEX_ATTRIB_ARRAY_TYPE: 34341, VERTEX_ATTRIB_ARRAY_NORMALIZED: 34922, VERTEX_ATTRIB_ARRAY_POINTER: 34373, VERTEX_ATTRIB_ARRAY_BUFFER_BINDING: 34975, IMPLEMENTATION_COLOR_READ_TYPE: 35738, IMPLEMENTATION_COLOR_READ_FORMAT: 35739, COMPILE_STATUS: 35713, LOW_FLOAT: 36336, MEDIUM_FLOAT: 36337, HIGH_FLOAT: 36338, LOW_INT: 36339, MEDIUM_INT: 36340, HIGH_INT: 36341, FRAMEBUFFER: 36160, RENDERBUFFER: 36161, RGBA4: 32854, RGB5_A1: 32855, RGB565: 36194, DEPTH_COMPONENT16: 33189, STENCIL_INDEX8: 36168, DEPTH_STENCIL: 34041, RENDERBUFFER_WIDTH: 36162, RENDERBUFFER_HEIGHT: 36163, RENDERBUFFER_INTERNAL_FORMAT: 36164, RENDERBUFFER_RED_SIZE: 36176, RENDERBUFFER_GREEN_SIZE: 36177, RENDERBUFFER_BLUE_SIZE: 36178, RENDERBUFFER_ALPHA_SIZE: 36179, RENDERBUFFER_DEPTH_SIZE: 36180, RENDERBUFFER_STENCIL_SIZE: 36181, FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE: 36048, FRAMEBUFFER_ATTACHMENT_OBJECT_NAME: 36049, FRAMEBUFFER_ATTACHMENT_TEXTURE_LEVEL: 36050, FRAMEBUFFER_ATTACHMENT_TEXTURE_CUBE_MAP_FACE: 36051, COLOR_ATTACHMENT0: 36064, DEPTH_ATTACHMENT: 36096, STENCIL_ATTACHMENT: 36128, DEPTH_STENCIL_ATTACHMENT: 33306, NONE: 0, FRAMEBUFFER_COMPLETE: 36053, FRAMEBUFFER_INCOMPLETE_ATTACHMENT: 36054, FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT: 36055, FRAMEBUFFER_INCOMPLETE_DIMENSIONS: 36057, FRAMEBUFFER_UNSUPPORTED: 36061, FRAMEBUFFER_BINDING: 36006, RENDERBUFFER_BINDING: 36007, MAX_RENDERBUFFER_SIZE: 34024, INVALID_FRAMEBUFFER_OPERATION: 1286, UNPACK_FLIP_Y_WEBGL: 37440, UNPACK_PREMULTIPLY_ALPHA_WEBGL: 37441, CONTEXT_LOST_WEBGL: 37442, UNPACK_COLORSPACE_CONVERSION_WEBGL: 37443, BROWSER_DEFAULT_WEBGL: 37444, READ_BUFFER: 3074, UNPACK_ROW_LENGTH: 3314, UNPACK_SKIP_ROWS: 3315, UNPACK_SKIP_PIXELS: 3316, PACK_ROW_LENGTH: 3330, PACK_SKIP_ROWS: 3331, PACK_SKIP_PIXELS: 3332, COLOR: 6144, DEPTH: 6145, STENCIL: 6146, RED: 6403, RGB8: 32849, RGBA8: 32856, RGB10_A2: 32857, TEXTURE_BINDING_3D: 32874, UNPACK_SKIP_IMAGES: 32877, UNPACK_IMAGE_HEIGHT: 32878, TEXTURE_3D: 32879, TEXTURE_WRAP_R: 32882, MAX_3D_TEXTURE_SIZE: 32883, UNSIGNED_INT_2_10_10_10_REV: 33640, MAX_ELEMENTS_VERTICES: 33000, MAX_ELEMENTS_INDICES: 33001, TEXTURE_MIN_LOD: 33082, TEXTURE_MAX_LOD: 33083, TEXTURE_BASE_LEVEL: 33084, TEXTURE_MAX_LEVEL: 33085, MIN: 32775, MAX: 32776, DEPTH_COMPONENT24: 33190, MAX_TEXTURE_LOD_BIAS: 34045, TEXTURE_COMPARE_MODE: 34892, TEXTURE_COMPARE_FUNC: 34893, CURRENT_QUERY: 34917, QUERY_RESULT: 34918, QUERY_RESULT_AVAILABLE: 34919, STREAM_READ: 35041, STREAM_COPY: 35042, STATIC_READ: 35045, STATIC_COPY: 35046, DYNAMIC_READ: 35049, DYNAMIC_COPY: 35050, MAX_DRAW_BUFFERS: 34852, DRAW_BUFFER0: 34853, DRAW_BUFFER1: 34854, DRAW_BUFFER2: 34855, DRAW_BUFFER3: 34856, DRAW_BUFFER4: 34857, DRAW_BUFFER5: 34858, DRAW_BUFFER6: 34859, DRAW_BUFFER7: 34860, DRAW_BUFFER8: 34861, DRAW_BUFFER9: 34862, DRAW_BUFFER10: 34863, DRAW_BUFFER11: 34864, DRAW_BUFFER12: 34865, DRAW_BUFFER13: 34866, DRAW_BUFFER14: 34867, DRAW_BUFFER15: 34868, MAX_FRAGMENT_UNIFORM_COMPONENTS: 35657, MAX_VERTEX_UNIFORM_COMPONENTS: 35658, SAMPLER_3D: 35679, SAMPLER_2D_SHADOW: 35682, FRAGMENT_SHADER_DERIVATIVE_HINT: 35723, PIXEL_PACK_BUFFER: 35051, PIXEL_UNPACK_BUFFER: 35052, PIXEL_PACK_BUFFER_BINDING: 35053, PIXEL_UNPACK_BUFFER_BINDING: 35055, FLOAT_MAT2x3: 35685, FLOAT_MAT2x4: 35686, FLOAT_MAT3x2: 35687, FLOAT_MAT3x4: 35688, FLOAT_MAT4x2: 35689, FLOAT_MAT4x3: 35690, SRGB: 35904, SRGB8: 35905, SRGB8_ALPHA8: 35907, COMPARE_REF_TO_TEXTURE: 34894, RGBA32F: 34836, RGB32F: 34837, RGBA16F: 34842, RGB16F: 34843, VERTEX_ATTRIB_ARRAY_INTEGER: 35069, MAX_ARRAY_TEXTURE_LAYERS: 35071, MIN_PROGRAM_TEXEL_OFFSET: 35076, MAX_PROGRAM_TEXEL_OFFSET: 35077, MAX_VARYING_COMPONENTS: 35659, TEXTURE_2D_ARRAY: 35866, TEXTURE_BINDING_2D_ARRAY: 35869, R11F_G11F_B10F: 35898, UNSIGNED_INT_10F_11F_11F_REV: 35899, RGB9_E5: 35901, UNSIGNED_INT_5_9_9_9_REV: 35902, TRANSFORM_FEEDBACK_BUFFER_MODE: 35967, MAX_TRANSFORM_FEEDBACK_SEPARATE_COMPONENTS: 35968, TRANSFORM_FEEDBACK_VARYINGS: 35971, TRANSFORM_FEEDBACK_BUFFER_START: 35972, TRANSFORM_FEEDBACK_BUFFER_SIZE: 35973, TRANSFORM_FEEDBACK_PRIMITIVES_WRITTEN: 35976, RASTERIZER_DISCARD: 35977, MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS: 35978, MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS: 35979, INTERLEAVED_ATTRIBS: 35980, SEPARATE_ATTRIBS: 35981, TRANSFORM_FEEDBACK_BUFFER: 35982, TRANSFORM_FEEDBACK_BUFFER_BINDING: 35983, RGBA32UI: 36208, RGB32UI: 36209, RGBA16UI: 36214, RGB16UI: 36215, RGBA8UI: 36220, RGB8UI: 36221, RGBA32I: 36226, RGB32I: 36227, RGBA16I: 36232, RGB16I: 36233, RGBA8I: 36238, RGB8I: 36239, RED_INTEGER: 36244, RGB_INTEGER: 36248, RGBA_INTEGER: 36249, SAMPLER_2D_ARRAY: 36289, SAMPLER_2D_ARRAY_SHADOW: 36292, SAMPLER_CUBE_SHADOW: 36293, UNSIGNED_INT_VEC2: 36294, UNSIGNED_INT_VEC3: 36295, UNSIGNED_INT_VEC4: 36296, INT_SAMPLER_2D: 36298, INT_SAMPLER_3D: 36299, INT_SAMPLER_CUBE: 36300, INT_SAMPLER_2D_ARRAY: 36303, UNSIGNED_INT_SAMPLER_2D: 36306, UNSIGNED_INT_SAMPLER_3D: 36307, UNSIGNED_INT_SAMPLER_CUBE: 36308, UNSIGNED_INT_SAMPLER_2D_ARRAY: 36311, DEPTH_COMPONENT32F: 36012, DEPTH32F_STENCIL8: 36013, FLOAT_32_UNSIGNED_INT_24_8_REV: 36269, FRAMEBUFFER_ATTACHMENT_COLOR_ENCODING: 33296, FRAMEBUFFER_ATTACHMENT_COMPONENT_TYPE: 33297, FRAMEBUFFER_ATTACHMENT_RED_SIZE: 33298, FRAMEBUFFER_ATTACHMENT_GREEN_SIZE: 33299, FRAMEBUFFER_ATTACHMENT_BLUE_SIZE: 33300, FRAMEBUFFER_ATTACHMENT_ALPHA_SIZE: 33301, FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE: 33302, FRAMEBUFFER_ATTACHMENT_STENCIL_SIZE: 33303, FRAMEBUFFER_DEFAULT: 33304, UNSIGNED_INT_24_8: 34042, DEPTH24_STENCIL8: 35056, UNSIGNED_NORMALIZED: 35863, DRAW_FRAMEBUFFER_BINDING: 36006, READ_FRAMEBUFFER: 36008, DRAW_FRAMEBUFFER: 36009, READ_FRAMEBUFFER_BINDING: 36010, RENDERBUFFER_SAMPLES: 36011, FRAMEBUFFER_ATTACHMENT_TEXTURE_LAYER: 36052, MAX_COLOR_ATTACHMENTS: 36063, COLOR_ATTACHMENT1: 36065, COLOR_ATTACHMENT2: 36066, COLOR_ATTACHMENT3: 36067, COLOR_ATTACHMENT4: 36068, COLOR_ATTACHMENT5: 36069, COLOR_ATTACHMENT6: 36070, COLOR_ATTACHMENT7: 36071, COLOR_ATTACHMENT8: 36072, COLOR_ATTACHMENT9: 36073, COLOR_ATTACHMENT10: 36074, COLOR_ATTACHMENT11: 36075, COLOR_ATTACHMENT12: 36076, COLOR_ATTACHMENT13: 36077, COLOR_ATTACHMENT14: 36078, COLOR_ATTACHMENT15: 36079, FRAMEBUFFER_INCOMPLETE_MULTISAMPLE: 36182, MAX_SAMPLES: 36183, HALF_FLOAT: 5131, RG: 33319, RG_INTEGER: 33320, R8: 33321, RG8: 33323, R16F: 33325, R32F: 33326, RG16F: 33327, RG32F: 33328, R8I: 33329, R8UI: 33330, R16I: 33331, R16UI: 33332, R32I: 33333, R32UI: 33334, RG8I: 33335, RG8UI: 33336, RG16I: 33337, RG16UI: 33338, RG32I: 33339, RG32UI: 33340, VERTEX_ARRAY_BINDING: 34229, R8_SNORM: 36756, RG8_SNORM: 36757, RGB8_SNORM: 36758, RGBA8_SNORM: 36759, SIGNED_NORMALIZED: 36764, COPY_READ_BUFFER: 36662, COPY_WRITE_BUFFER: 36663, COPY_READ_BUFFER_BINDING: 36662, COPY_WRITE_BUFFER_BINDING: 36663, UNIFORM_BUFFER: 35345, UNIFORM_BUFFER_BINDING: 35368, UNIFORM_BUFFER_START: 35369, UNIFORM_BUFFER_SIZE: 35370, MAX_VERTEX_UNIFORM_BLOCKS: 35371, MAX_FRAGMENT_UNIFORM_BLOCKS: 35373, MAX_COMBINED_UNIFORM_BLOCKS: 35374, MAX_UNIFORM_BUFFER_BINDINGS: 35375, MAX_UNIFORM_BLOCK_SIZE: 35376, MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS: 35377, MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS: 35379, UNIFORM_BUFFER_OFFSET_ALIGNMENT: 35380, ACTIVE_UNIFORM_BLOCKS: 35382, UNIFORM_TYPE: 35383, UNIFORM_SIZE: 35384, UNIFORM_BLOCK_INDEX: 35386, UNIFORM_OFFSET: 35387, UNIFORM_ARRAY_STRIDE: 35388, UNIFORM_MATRIX_STRIDE: 35389, UNIFORM_IS_ROW_MAJOR: 35390, UNIFORM_BLOCK_BINDING: 35391, UNIFORM_BLOCK_DATA_SIZE: 35392, UNIFORM_BLOCK_ACTIVE_UNIFORMS: 35394, UNIFORM_BLOCK_ACTIVE_UNIFORM_INDICES: 35395, UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER: 35396, UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER: 35398, INVALID_INDEX: 4294967295, MAX_VERTEX_OUTPUT_COMPONENTS: 37154, MAX_FRAGMENT_INPUT_COMPONENTS: 37157, MAX_SERVER_WAIT_TIMEOUT: 37137, OBJECT_TYPE: 37138, SYNC_CONDITION: 37139, SYNC_STATUS: 37140, SYNC_FLAGS: 37141, SYNC_FENCE: 37142, SYNC_GPU_COMMANDS_COMPLETE: 37143, UNSIGNALED: 37144, SIGNALED: 37145, ALREADY_SIGNALED: 37146, TIMEOUT_EXPIRED: 37147, CONDITION_SATISFIED: 37148, WAIT_FAILED: 37149, SYNC_FLUSH_COMMANDS_BIT: 1, VERTEX_ATTRIB_ARRAY_DIVISOR: 35070, ANY_SAMPLES_PASSED: 35887, ANY_SAMPLES_PASSED_CONSERVATIVE: 36202, SAMPLER_BINDING: 35097, RGB10_A2UI: 36975, INT_2_10_10_10_REV: 36255, TRANSFORM_FEEDBACK: 36386, TRANSFORM_FEEDBACK_PAUSED: 36387, TRANSFORM_FEEDBACK_ACTIVE: 36388, TRANSFORM_FEEDBACK_BINDING: 36389, TEXTURE_IMMUTABLE_FORMAT: 37167, MAX_ELEMENT_INDEX: 36203, TEXTURE_IMMUTABLE_LEVELS: 33503, TIMEOUT_IGNORED: -1, MAX_CLIENT_WAIT_TIMEOUT_WEBGL: 37447 };
// Fleet claims Chrome 154: its WebGL method face lacks makeXRCompatible (155 added it).
const _GL_METHODS1 = ['activeTexture', 'attachShader', 'bindAttribLocation', 'bindRenderbuffer', 'blendColor', 'blendEquation', 'blendEquationSeparate', 'blendFunc', 'blendFuncSeparate', 'bufferData', 'bufferSubData', 'checkFramebufferStatus', 'compileShader', 'compressedTexImage2D', 'compressedTexSubImage2D', 'copyTexImage2D', 'copyTexSubImage2D', 'createBuffer', 'createFramebuffer', 'createProgram', 'createRenderbuffer', 'createShader', 'createTexture', 'cullFace', 'deleteBuffer', 'deleteFramebuffer', 'deleteProgram', 'deleteRenderbuffer', 'deleteShader', 'deleteTexture', 'depthFunc', 'depthMask', 'depthRange', 'detachShader', 'disable', 'enable', 'finish', 'flush', 'framebufferRenderbuffer', 'framebufferTexture2D', 'frontFace', 'generateMipmap', 'getActiveAttrib', 'getActiveUniform', 'getAttachedShaders', 'getAttribLocation', 'getBufferParameter', 'getContextAttributes', 'getError', 'getExtension', 'getFramebufferAttachmentParameter', 'getParameter', 'getProgramInfoLog', 'getProgramParameter', 'getRenderbufferParameter', 'getShaderInfoLog', 'getShaderParameter', 'getShaderPrecisionFormat', 'getShaderSource', 'getSupportedExtensions', 'getTexParameter', 'getUniform', 'getUniformLocation', 'getVertexAttrib', 'getVertexAttribOffset', 'hint', 'isBuffer', 'isContextLost', 'isEnabled', 'isFramebuffer', 'isProgram', 'isRenderbuffer', 'isShader', 'isTexture', 'lineWidth', 'linkProgram', 'pixelStorei', 'polygonOffset', 'readPixels', 'renderbufferStorage', 'sampleCoverage', 'shaderSource', 'stencilFunc', 'stencilFuncSeparate', 'stencilMask', 'stencilMaskSeparate', 'stencilOp', 'stencilOpSeparate', 'texImage2D', 'texParameterf', 'texParameteri', 'texSubImage2D', 'useProgram', 'validateProgram', 'bindBuffer', 'bindFramebuffer', 'bindTexture', 'clear', 'clearColor', 'clearDepth', 'clearStencil', 'colorMask', 'disableVertexAttribArray', 'drawArrays', 'drawElements', 'enableVertexAttribArray', 'scissor', 'uniform1f', 'uniform1fv', 'uniform1i', 'uniform1iv', 'uniform2f', 'uniform2fv', 'uniform2i', 'uniform2iv', 'uniform3f', 'uniform3fv', 'uniform3i', 'uniform3iv', 'uniform4f', 'uniform4fv', 'uniform4i', 'uniform4iv', 'uniformMatrix2fv', 'uniformMatrix3fv', 'uniformMatrix4fv', 'vertexAttrib1f', 'vertexAttrib1fv', 'vertexAttrib2f', 'vertexAttrib2fv', 'vertexAttrib3f', 'vertexAttrib3fv', 'vertexAttrib4f', 'vertexAttrib4fv', 'vertexAttribPointer', 'viewport', 'drawingBufferStorage'];
const _GL_METHODS2 = ['activeTexture', 'attachShader', 'beginQuery', 'beginTransformFeedback', 'bindAttribLocation', 'bindBufferBase', 'bindBufferRange', 'bindRenderbuffer', 'bindSampler', 'bindTransformFeedback', 'bindVertexArray', 'blendColor', 'blendEquation', 'blendEquationSeparate', 'blendFunc', 'blendFuncSeparate', 'blitFramebuffer', 'bufferData', 'bufferSubData', 'checkFramebufferStatus', 'clientWaitSync', 'compileShader', 'compressedTexImage2D', 'compressedTexImage3D', 'compressedTexSubImage2D', 'compressedTexSubImage3D', 'copyBufferSubData', 'copyTexImage2D', 'copyTexSubImage2D', 'copyTexSubImage3D', 'createBuffer', 'createFramebuffer', 'createProgram', 'createQuery', 'createRenderbuffer', 'createSampler', 'createShader', 'createTexture', 'createTransformFeedback', 'createVertexArray', 'cullFace', 'deleteBuffer', 'deleteFramebuffer', 'deleteProgram', 'deleteQuery', 'deleteRenderbuffer', 'deleteSampler', 'deleteShader', 'deleteSync', 'deleteTexture', 'deleteTransformFeedback', 'deleteVertexArray', 'depthFunc', 'depthMask', 'depthRange', 'detachShader', 'disable', 'drawArraysInstanced', 'drawElementsInstanced', 'drawRangeElements', 'enable', 'endQuery', 'endTransformFeedback', 'fenceSync', 'finish', 'flush', 'framebufferRenderbuffer', 'framebufferTexture2D', 'framebufferTextureLayer', 'frontFace', 'generateMipmap', 'getActiveAttrib', 'getActiveUniform', 'getActiveUniformBlockName', 'getActiveUniformBlockParameter', 'getActiveUniforms', 'getAttachedShaders', 'getAttribLocation', 'getBufferParameter', 'getBufferSubData', 'getContextAttributes', 'getError', 'getExtension', 'getFragDataLocation', 'getFramebufferAttachmentParameter', 'getIndexedParameter', 'getInternalformatParameter', 'getParameter', 'getProgramInfoLog', 'getProgramParameter', 'getQuery', 'getQueryParameter', 'getRenderbufferParameter', 'getSamplerParameter', 'getShaderInfoLog', 'getShaderParameter', 'getShaderPrecisionFormat', 'getShaderSource', 'getSupportedExtensions', 'getSyncParameter', 'getTexParameter', 'getTransformFeedbackVarying', 'getUniform', 'getUniformBlockIndex', 'getUniformIndices', 'getUniformLocation', 'getVertexAttrib', 'getVertexAttribOffset', 'hint', 'invalidateFramebuffer', 'invalidateSubFramebuffer', 'isBuffer', 'isContextLost', 'isEnabled', 'isFramebuffer', 'isProgram', 'isQuery', 'isRenderbuffer', 'isSampler', 'isShader', 'isSync', 'isTexture', 'isTransformFeedback', 'isVertexArray', 'lineWidth', 'linkProgram', 'pauseTransformFeedback', 'pixelStorei', 'polygonOffset', 'readBuffer', 'readPixels', 'renderbufferStorage', 'renderbufferStorageMultisample', 'resumeTransformFeedback', 'sampleCoverage', 'samplerParameterf', 'samplerParameteri', 'shaderSource', 'stencilFunc', 'stencilFuncSeparate', 'stencilMask', 'stencilMaskSeparate', 'stencilOp', 'stencilOpSeparate', 'texImage2D', 'texImage3D', 'texParameterf', 'texParameteri', 'texStorage2D', 'texStorage3D', 'texSubImage2D', 'texSubImage3D', 'transformFeedbackVaryings', 'uniform1ui', 'uniform2ui', 'uniform3ui', 'uniform4ui', 'uniformBlockBinding', 'useProgram', 'validateProgram', 'vertexAttribDivisor', 'vertexAttribI4i', 'vertexAttribI4ui', 'vertexAttribIPointer', 'waitSync', 'bindBuffer', 'bindFramebuffer', 'bindTexture', 'clear', 'clearBufferfi', 'clearBufferfv', 'clearBufferiv', 'clearBufferuiv', 'clearColor', 'clearDepth', 'clearStencil', 'colorMask', 'disableVertexAttribArray', 'drawArrays', 'drawBuffers', 'drawElements', 'enableVertexAttribArray', 'scissor', 'uniform1f', 'uniform1fv', 'uniform1i', 'uniform1iv', 'uniform1uiv', 'uniform2f', 'uniform2fv', 'uniform2i', 'uniform2iv', 'uniform2uiv', 'uniform3f', 'uniform3fv', 'uniform3i', 'uniform3iv', 'uniform3uiv', 'uniform4f', 'uniform4fv', 'uniform4i', 'uniform4iv', 'uniform4uiv', 'uniformMatrix2fv', 'uniformMatrix2x3fv', 'uniformMatrix2x4fv', 'uniformMatrix3fv', 'uniformMatrix3x2fv', 'uniformMatrix3x4fv', 'uniformMatrix4fv', 'uniformMatrix4x2fv', 'uniformMatrix4x3fv', 'vertexAttrib1f', 'vertexAttrib1fv', 'vertexAttrib2f', 'vertexAttrib2fv', 'vertexAttrib3f', 'vertexAttrib3fv', 'vertexAttrib4f', 'vertexAttrib4fv', 'vertexAttribI4iv', 'vertexAttribI4uiv', 'vertexAttribPointer', 'viewport', 'drawingBufferStorage'];
// Return values for the handful of methods detectors actually chain-read;
// everything else in the tables is a true void no-op (undefined — Chrome
// methods that "return nothing" give undefined, never 0).
// (刀69) Resource handles are instances of Chrome's resource classes (minted
// illegal-ctor interfaces, read at call time), and every creator is its OWN
// function object: they used to share one function, so the shape pass's
// `.length` fix for createShader (1) leaked onto createBuffer/createProgram/
// fenceSync (Chrome: 0/0/2).
const _glHandle = function(cls) { const C = globalThis[cls]; return (C && C.prototype) ? Object.create(C.prototype) : {}; };
const _glCreator = function(cls) { return function() { return _glHandle(cls); }; };
const _GL_RET = {
  createBuffer: _glCreator('WebGLBuffer'), createShader: _glCreator('WebGLShader'), createProgram: _glCreator('WebGLProgram'),
  createTexture: _glCreator('WebGLTexture'), createFramebuffer: _glCreator('WebGLFramebuffer'), createRenderbuffer: _glCreator('WebGLRenderbuffer'),
  createQuery: _glCreator('WebGLQuery'), createSampler: _glCreator('WebGLSampler'), createTransformFeedback: _glCreator('WebGLTransformFeedback'),
  createVertexArray: _glCreator('WebGLVertexArrayObject'), fenceSync: _glCreator('WebGLSync'),
  getProgramParameter: function() { return true; }, getShaderParameter: function() { return true; },
  getUniformLocation: function() { return _glHandle('WebGLUniformLocation'); },
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
  // (刀92) getError reads-and-clears the per-context sticky error; while lost it is
  // the CONTEXT_LOST_WEBGL constant, forever. isContextLost mirrors the sync flag.
  getError: function() { const b = _glBaseOf.get(this) || this; if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation'); const st = _glX(b); if (st.lost) return 37442; const e = st.err; st.err = 0; return e; },
  isContextLost: function() { const b = _glBaseOf.get(this) || this; if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation'); return _glX(b).lost; },
  isBuffer: function() { return false; }, isFramebuffer: function() { return false; },
  isProgram: function() { return false; }, isQuery: function() { return false; },
  isRenderbuffer: function() { return false; }, isSampler: function() { return false; },
  isShader: function() { return false; }, isSync: function() { return false; },
  isTexture: function() { return false; }, isTransformFeedback: function() { return false; },
  isVertexArray: function() { return false; }, isEnabled: function() { return false; },
  // (刀69) Per-context attributes: the dictionary getContext was handed, defaulted
  // and ToBoolean-coerced, in Chrome's alphabetical key order, plus xrCompatible.
  getContextAttributes: function() {
    const a = _glAttrs.get(_glBaseOf.get(this) || this) || {};
    return { alpha: a.alpha !== false, antialias: a.antialias !== false, depth: a.depth !== false, desynchronized: !!a.desynchronized,
      failIfMajorPerformanceCaveat: !!a.failIfMajorPerformanceCaveat, powerPreference: a.powerPreference || 'default',
      premultipliedAlpha: a.premultipliedAlpha !== false, preserveDrawingBuffer: !!a.preserveDrawingBuffer, stencil: !!a.stencil, xrCompatible: !!a.xrCompatible };
  },
};
// (刀69) Extension objects come from the generated webgl_ext.js table (Chrome interface
// tags + prototype members, one cached bare carrier per context and extension).
// The canvas→context link lives in a WeakMap, not an own property — even a
// non-enumerable own prop would show in getOwnPropertyNames(gl), and Chrome's
// instance has exactly zero own props of any kind. Keyed on the Proxy (the
// public identity) so the prototype accessors' `this` resolves it.
const _glCanvasMap = new WeakMap();
const _glBaseOf = new WeakMap();      // context Proxy -> its base (the WeakMap key space)
const _glColorSpace = new WeakMap();  // base -> { drawingBufferColorSpace, unpackColorSpace }
const _glAttrs = new WeakMap();       // base -> the normalized WebGLContextAttributes getContext received
const _glXrPromise = new WeakMap();   // base -> the cached makeXRCompatible() promise
// (刀92) Per-context extension-method state (Chrome 145 truth, k92-p* probes): sticky GL
// error, context-loss machine, the ext-parameter readback state (clip origin/depth,
// polygon mode, provoking vertex), the current timer query, and the activation sets.
// Cross-context object TYPING lives in the two global WeakSets below — a VAO minted on
// context A passes the parameter check on context B (is → false there), it never
// becomes a TypeError.
const _glXSt = new WeakMap();
const _glX = (b) => {
  let s = _glXSt.get(b);
  if (!s) { s = { err: 0, lost: false, lostPrevented: false, restoring: false, clipO: 36001, clipD: 37726,
                  polyMode: 6913, provoking: 36429, curQ: null, en: new Set(),
                  vAct: new WeakSet(), qAct: new WeakSet() }; _glXSt.set(b, s); }
  return s;
};
const _GLX_VALL = new WeakSet();      // every minted WebGLVertexArrayObjectOES, any context
const _GLX_QALL = new WeakSet();      // every minted WebGLTimerQueryEXT, any context
const _glShSrc = new WeakMap();       // core shader -> its shaderSource() string (debug-shaders face)
const _glShCompiled = new WeakSet();  // core shaders that went through compileShader()
// Ext-owned pnames answer on getParameter ONLY after that extension was fetched on
// this context (enable-on-getExtension, k92-p5/p6) — before that they read null.
const _GL_XPN = new Map([
  [34047, { ext: 'EXT_texture_filter_anisotropic', get: function() { return 16; } }],
  [37724, { ext: 'EXT_clip_control', get: function(st) { return st.clipO; } }],
  [37725, { ext: 'EXT_clip_control', get: function(st) { return st.clipD; } }],
  [2880, { ext: 'WEBGL_polygon_mode', get: function(st) { return st.polyMode; } }],
  [36431, { ext: 'WEBGL_provoking_vertex', get: function(st) { return st.provoking; } }],
  [36795, { ext: null, get: function() { return false; } }],  // GPU_DISJOINT: either timer ext enables
]);
for (const [_GLC, _isGL2] of [[globalThis.WebGLRenderingContext, false], [globalThis.WebGL2RenderingContext, true]]) {
  const P = _GLC.prototype;
  const isGL2 = _isGL2;
  const consts = isGL2 ? _GL_CONSTS2 : _GL_CONSTS1;
  const exts = isGL2 ? _GL_EXTS2 : _GL_EXTS1;
  // (刀69) Chrome carries every GL constant TWICE: on the prototype and as a
  // static on the constructor (both e--). The prototype copy installs
  // configurable here so the shape pass can put it in Chrome's member order,
  // and it seals the flags afterwards; the static copy has no ordering
  // concern and is final on creation (table order = Chrome's static order).
  for (const k in consts) {
    Object.defineProperty(P, k, { value: consts[k], enumerable: true, configurable: true });
    Object.defineProperty(_GLC, k, { value: consts[k], enumerable: true });
  }
  // Introspection faces keep real implementations (#117 lineage) — installed
  // BEFORE the table fill so the generic pass sees them as already-present.
  // (刀92) All four introspection faces are receiver-strict (foreign `this` →
  // bare "Illegal invocation"), ext-pname reads gate on enable-on-getExtension,
  // and loss gates getExtension→null / getParameter→null / list→[].
  P.getParameter = function(pname) {
    const b = _glBaseOf.get(this) || this;
    if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation');
    const st = _glX(b);
    if (st.lost) return null;
    const xp = _GL_XPN.get(pname);
    if (xp !== undefined) {
      const on = xp.ext === null
        ? (st.en.has('EXT_disjoint_timer_query') || st.en.has('EXT_disjoint_timer_query_webgl2'))
        : st.en.has(xp.ext);
      return on ? xp.get(st) : null;
    }
    return _glParam(pname, isGL2);
  };
  P.getExtension = function(name) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'getExtension' on '" + _GLC.name + "': 1 argument required, but only 0 present.");
    const b = _glBaseOf.get(this) || this;
    if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation');
    // Chrome matches extension names case-insensitively.
    const want = String(name).toLowerCase();
    let hit = null;
    for (const e of exts) { if (e.toLowerCase() === want) { hit = e; break; } }
    if (hit === null) return null;
    const st = _glX(b);
    if (st.lost) return null;
    st.en.add(hit);
    return globalThis.__diting_glExt(hit, b);
  };
  P.getSupportedExtensions = function() {
    const b = _glBaseOf.get(this) || this;
    if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation');
    return _glX(b).lost ? [] : exts.slice();
  };
  P.getShaderPrecisionFormat = function() { return { rangeMin: 127, rangeMax: 127, precision: 23 }; };
  P.bufferData = function() {};
  P.readPixels = function(x,y,w,h,f,t,d) { if(d) for(let i=0;i<d.length;i++) d[i]=0; };
  for (const m of (isGL2 ? _GL_METHODS2 : _GL_METHODS1)) {
    if (Object.prototype.hasOwnProperty.call(P, m)) continue;
    P[m] = _GL_RET[m] || function() {};
  }
  // The six accessor faces (Chrome 155): canvas tracks the creating canvas;
  // drawingBuffer* the buffer at current canvas size; colorSpaces default
  // srgb; drawingBufferFormat is the GLenum GL_RGBA8 (32856 = 0x8058) — a
  // number, not a string.
  const _acc = function(get) { return { get, enumerable: true, configurable: true }; };
  Object.defineProperty(P, 'canvas', _acc(function() { return _glCanvasMap.get(this) || null; }));
  Object.defineProperty(P, 'drawingBufferWidth', _acc(function() { const c = _glCanvasMap.get(this); return c ? c.width : 300; }));
  Object.defineProperty(P, 'drawingBufferHeight', _acc(function() { const c = _glCanvasMap.get(this); return c ? c.height : 150; }));
  // (刀69) The two colorSpace faces are read/write PredefinedColorSpace
  // attributes: per-context state, an invalid value is silently IGNORED (not a
  // TypeError), a foreign receiver throws Illegal invocation (Chrome 145 truth).
  const _csFace = (name) => {
    const get = function() { const b = _glBaseOf.get(this) || this; if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation'); const st = _glColorSpace.get(b); return st && st[name] || 'srgb'; };
    const set = function(v) {
      const b = _glBaseOf.get(this) || this;
      if (!_glCanvasMap.has(b)) throw new TypeError('Illegal invocation');
      const sv = String(v);
      if (sv !== 'srgb' && sv !== 'display-p3') return;
      let st = _glColorSpace.get(b);
      if (!st) { st = {}; _glColorSpace.set(b, st); }
      st[name] = sv;
    };
    Object.defineProperty(get, 'name', { value: 'get ' + name });
    Object.defineProperty(set, 'name', { value: 'set ' + name });
    Object.defineProperty(P, name, { get, set, enumerable: true, configurable: true });
  };
  _csFace('drawingBufferColorSpace');
  _csFace('unpackColorSpace');
  // (刀69) makeXRCompatible: one cached Promise per context that never
  // settles on a machine without an XR runtime (Chrome 145 headless truth:
  // pending after 9s, the SAME promise on every call, xrCompatible stays
  // false); a foreign receiver REJECTS (not throws) with the Illegal
  // invocation TypeError.
  Object.defineProperty(P, 'makeXRCompatible', { value: function makeXRCompatible() {
    const b = _glBaseOf.get(this) || this;
    if (!_glCanvasMap.has(b)) return Promise.reject(new TypeError("Failed to execute 'makeXRCompatible' on '" + _GLC.name + "': Illegal invocation"));
    let pr = _glXrPromise.get(b);
    if (!pr) { pr = new Promise(function() {}); _glXrPromise.set(b, pr); }
    return pr;
  }, writable: true, enumerable: true, configurable: true });
  Object.defineProperty(P, 'drawingBufferFormat', _acc(function() { return 0x8058; }));
}
// ---------- (刀92) WebGL extension method behavior ----------
// webgl_ext.js forwards every extension method here carrying (iface, method, nreq);
// this block owns the WebIDL argc ladder, receiver strictness (bare "Illegal
// invocation", k92-p4) and the per-method Chrome-145 behavior (k92-glxt/p1-p6).
// The block loads before webgl_ext.js but is only ever CALLED after both, so the
// late __diting_glExtBaseOf reference is safe at runtime.
const _glxType = function(v, ws, meth, iface, param, tname) {
  if (v === null || v === undefined || typeof v !== 'object' || !ws.has(v))
    throw new TypeError("Failed to execute '" + meth + "' on '" + iface + "': parameter " + param + " is not of type '" + tname + "'.");
};
const _glObjProtos = new Map();
const _glObjProto = function(tag) {
  let p = _glObjProtos.get(tag);
  if (p) return p;
  p = {};
  Object.defineProperty(p, Symbol.toStringTag, { value: tag, configurable: true });
  Object.setPrototypeOf(p, globalThis.WebGLObject.prototype);   // k92-p1: the mint chain ends at [object WebGLObject], not Object
  _glObjProtos.set(tag, p);
  return p;
};
const _glMint = function(tag, ws) { const o = Object.create(_glObjProto(tag)); ws.add(o); return o; };
const _GLX_H = {
  // ---- OESVertexArrayObject: GL-name activation — fresh/deleted objects fail is*,
  //      bind activates, the TYPE outlives deletion (global typing set). ----
  'OESVertexArrayObject.createVertexArrayOES': function(st) { return st.lost ? null : _glMint('WebGLVertexArrayObjectOES', _GLX_VALL); },
  'OESVertexArrayObject.bindVertexArrayOES': function(st, b, a) { const v = a[0]; if (v === null || v === undefined) return; _glxType(v, _GLX_VALL, 'bindVertexArrayOES', 'OESVertexArrayObject', 1, 'WebGLVertexArrayObjectOES'); st.vAct.add(v); },
  'OESVertexArrayObject.deleteVertexArrayOES': function(st, b, a) { const v = a[0]; if (v === null || v === undefined) return; _glxType(v, _GLX_VALL, 'deleteVertexArrayOES', 'OESVertexArrayObject', 1, 'WebGLVertexArrayObjectOES'); st.vAct.delete(v); },
  'OESVertexArrayObject.isVertexArrayOES': function(st, b, a) { const v = a[0]; if (v === null || v === undefined) return false; _glxType(v, _GLX_VALL, 'isVertexArrayOES', 'OESVertexArrayObject', 1, 'WebGLVertexArrayObjectOES'); return st.vAct.has(v); },
  // ---- EXTDisjointTimerQuery: TIME_ELAPSED is the only begin target, activation
  //      survives endQuery and only delete clears it; results never complete (persona
  //      machine truth: AVAILABLE false / RESULT 0, zero error). ----
  'EXTDisjointTimerQuery.createQueryEXT': function(st) { return st.lost ? null : _glMint('WebGLTimerQueryEXT', _GLX_QALL); },
  'EXTDisjointTimerQuery.beginQueryEXT': function(st, b, a) {
    _glxType(a[1], _GLX_QALL, 'beginQueryEXT', 'EXTDisjointTimerQuery', 2, 'WebGLTimerQueryEXT');
    if ((a[0] >>> 0) !== 35007) { st.err = 1280; return; }
    if (st.curQ) { st.err = 1282; return; }
    st.curQ = a[1]; st.qAct.add(a[1]);
  },
  'EXTDisjointTimerQuery.endQueryEXT': function(st, b, a) {
    if ((a[0] >>> 0) !== 35007) { st.err = 1280; return; }
    if (!st.curQ) { st.err = 1282; return; }
    st.curQ = null;
  },
  'EXTDisjointTimerQuery.getQueryEXT': function(st, b, a) {
    const t = a[0] >>> 0, p = a[1] >>> 0;
    if (t === 35007) return p === 34917 ? (st.curQ || null) : (p === 34916 ? 64 : null);
    if (t === 36392) return p === 34917 ? null : (p === 34916 ? 0 : null);
    return null;
  },
  'EXTDisjointTimerQuery.getQueryObjectEXT': function(st, b, a) {
    _glxType(a[0], _GLX_QALL, 'getQueryObjectEXT', 'EXTDisjointTimerQuery', 1, 'WebGLTimerQueryEXT');
    const p = a[1] >>> 0;
    if (p === 34919) return false;
    if (p === 34918) return 0;
    st.err = 1280; return null;
  },
  'EXTDisjointTimerQuery.queryCounterEXT': function(st, b, a) {
    _glxType(a[0], _GLX_QALL, 'queryCounterEXT', 'EXTDisjointTimerQuery', 1, 'WebGLTimerQueryEXT');
    if ((a[1] >>> 0) !== 36392) { st.err = 1280; return; }
    st.qAct.add(a[0]);
  },
  'EXTDisjointTimerQuery.isQueryEXT': function(st, b, a) { const q = a[0]; if (q === null || q === undefined) return false; _glxType(q, _GLX_QALL, 'isQueryEXT', 'EXTDisjointTimerQuery', 1, 'WebGLTimerQueryEXT'); return st.qAct.has(q); },
  'EXTDisjointTimerQuery.deleteQueryEXT': function(st, b, a) { const q = a[0]; if (q === null || q === undefined) return; _glxType(q, _GLX_QALL, 'deleteQueryEXT', 'EXTDisjointTimerQuery', 1, 'WebGLTimerQueryEXT'); st.qAct.delete(q); if (st.curQ === q) st.curQ = null; },
  // ---- EXTDisjointTimerQueryWebGL2: the lone method, typed against CORE
  //      WebGLQuery. ----
  'EXTDisjointTimerQueryWebGL2.queryCounterEXT': function(st, b, a) {
    const q = a[0];
    if (!(q instanceof globalThis.WebGLQuery))
      throw new TypeError("Failed to execute 'queryCounterEXT' on 'EXTDisjointTimerQueryWebGL2': parameter 1 is not of type 'WebGLQuery'.");
    if ((a[1] >>> 0) !== 36392) { st.err = 1280; return; }
  },
  // ---- WEBGLDebugShaders: the translated source is the shader's own text, and only
  //      once compileShader() has run — sourced-but-uncompiled reads "". ----
  'WebGLDebugShaders.getTranslatedShaderSource': function(st, b, a) {
    const s = a[0];
    if (!(s instanceof globalThis.WebGLShader))
      throw new TypeError("Failed to execute 'getTranslatedShaderSource' on 'WebGLDebugShaders': parameter 1 is not of type 'WebGLShader'.");
    return _glShCompiled.has(s) ? (_glShSrc.get(s) || '') : '';
  },
  'WebGLCompressedTextureASTC.getSupportedProfiles': function() { return ['ldr', 'hdr']; },
  // ---- ANGLEInstancedArrays: on the programless persona every draw fails first
  //      (1282 beats mode/count validation); divisor bounds are the attrib count. ----
  'ANGLEInstancedArrays.drawArraysInstancedANGLE': function(st) { st.err = 1282; },
  'ANGLEInstancedArrays.drawElementsInstancedANGLE': function(st) { st.err = 1282; },
  'ANGLEInstancedArrays.vertexAttribDivisorANGLE': function(st, b, a) { if ((a[0] >>> 0) >= 16) st.err = 1281; },
  // ---- WebGLDrawBuffers: length before element ([]→1282, len>1→1280, bad elem→1282);
  //      non-object / non-iterable is the WebIDL sequence TypeError. ----
  'WebGLDrawBuffers.drawBuffersWEBGL': function(st, b, a) {
    const bufs = a[0];
    if (bufs === null || typeof bufs !== 'object' || typeof bufs[Symbol.iterator] !== 'function')
      throw new TypeError("Failed to execute 'drawBuffersWEBGL' on 'WebGLDrawBuffers': The provided value cannot be converted to a sequence.");
    const arr = Array.from(bufs);
    if (arr.length === 0) { st.err = 1282; return; }
    if (arr.length > 1) { st.err = 1280; return; }
    const e = arr[0] >>> 0;
    if (e !== 1029 && e !== 0) st.err = 1282;
  },
  // ---- OESDrawBuffersIndexed: draw-buffer index OOB → 1281; indexed enable takes
  //      BLEND only (anything else 1280). Valid calls are silent. ----
  'OESDrawBuffersIndexed.blendFunciOES': function(st, b, a) { if ((a[0] >>> 0) >= 8) st.err = 1281; },
  'OESDrawBuffersIndexed.blendFuncSeparateiOES': function(st, b, a) { if ((a[0] >>> 0) >= 8) st.err = 1281; },
  'OESDrawBuffersIndexed.blendEquationiOES': function(st, b, a) { if ((a[0] >>> 0) >= 8) st.err = 1281; },
  'OESDrawBuffersIndexed.blendEquationSeparateiOES': function(st, b, a) { if ((a[0] >>> 0) >= 8) st.err = 1281; },
  'OESDrawBuffersIndexed.colorMaskiOES': function(st, b, a) { if ((a[0] >>> 0) >= 8) st.err = 1281; },
  'OESDrawBuffersIndexed.enableiOES': function(st, b, a) { if ((a[0] >>> 0) !== 3042) { st.err = 1280; return; } if ((a[1] >>> 0) >= 8) st.err = 1281; },
  'OESDrawBuffersIndexed.disableiOES': function(st, b, a) { if ((a[0] >>> 0) !== 3042) { st.err = 1280; return; } if ((a[1] >>> 0) >= 8) st.err = 1281; },
  // ---- EXTClipControl / WEBGL_polygon_mode / WEBGL_provoking_vertex: enum
  //      validation + the state getParameter reads back. polygonMode takes
  //      FRONT_AND_BACK (1032) ONLY — a FRONT face is an error (pm.ok → 1280). ----
  'EXTClipControl.clipControlEXT': function(st, b, a) {
    const o = a[0] >>> 0, d = a[1] >>> 0;
    if (o !== 36001 && o !== 36002) { st.err = 1280; return; }
    if (d !== 37726 && d !== 37727) { st.err = 1280; return; }
    st.clipO = o; st.clipD = d;
  },
  'WebGLPolygonMode.polygonModeWEBGL': function(st, b, a) {
    const f = a[0] >>> 0, m = a[1] >>> 0;
    if (f !== 1032) { st.err = 1280; return; }
    if (m !== 6913 && m !== 6914) { st.err = 1280; return; }
    st.polyMode = m;
  },
  'WebGLProvokingVertex.provokingVertexWEBGL': function(st, b, a) {
    const c = a[0] >>> 0;
    if (c !== 36429 && c !== 36430) { st.err = 1280; return; }
    st.provoking = c;
  },
  'EXTPolygonOffsetClamp.polygonOffsetClampEXT': function() {},
  // ---- WebGLMultiDraw: draws on the programless persona — 1282, plain arrays accepted. ----
  'WebGLMultiDraw.multiDrawArraysWEBGL': function(st) { st.err = 1282; },
  'WebGLMultiDraw.multiDrawArraysInstancedWEBGL': function(st) { st.err = 1282; },
  'WebGLMultiDraw.multiDrawElementsWEBGL': function(st) { st.err = 1282; },
  'WebGLMultiDraw.multiDrawElementsInstancedWEBGL': function(st) { st.err = 1282; },
  // ---- WebGLLoseContext: sync flag + async WebGLContextEvent on the CANVAS;
  //      restore only arms when the lost event's preventDefault ran, then the
  //      restored event lands async with fully reset per-context state. ----
  'WebGLLoseContext.loseContext': function(st, b) {
    if (st.lost) return;
    st.lost = true; st.lostPrevented = false;
    const c = _glCanvasMap.get(b);
    if (c) setTimeout(function() {
      const e = new globalThis.WebGLContextEvent('webglcontextlost', { cancelable: true });
      c.dispatchEvent(e);
      if (e.defaultPrevented) st.lostPrevented = true;
    }, 0);
  },
  'WebGLLoseContext.restoreContext': function(st, b) {
    if (!st.lost || st.restoring || !st.lostPrevented) return;
    st.restoring = true;
    const c = _glCanvasMap.get(b);
    setTimeout(function() {
      st.lost = false; st.restoring = false; st.lostPrevented = false;
      st.err = 0; st.curQ = null; st.clipO = 36001; st.clipD = 37726; st.polyMode = 6913; st.provoking = 36429;
      st.en = new Set(); st.vAct = new WeakSet(); st.qAct = new WeakSet();
      if (c) c.dispatchEvent(new globalThis.WebGLContextEvent('webglcontextrestored'));
    }, 0);
  },
};
Object.defineProperty(globalThis, '__diting_glExtCall', {
  value: function(iface, meth, nreq, self, args) {
    if (args.length < nreq)
      throw new TypeError("Failed to execute '" + meth + "' on '" + iface + "': " + nreq + " argument" + (nreq > 1 ? "s" : "") + " required, but only " + args.length + " present.");
    const b = globalThis.__diting_glExtBaseOf(self);
    if (b === null || !_glCanvasMap.has(b)) throw new TypeError('Illegal invocation');
    const h = _GLX_H[iface + '.' + meth];
    return h ? h(_glX(b), b, args) : undefined;
  },
  writable: true, enumerable: false, configurable: true,
});
// Core shader tracking for the debug-shaders face (the old no-ops): shaderSource
// remembers the text, compileShader stamps the shader — getTranslatedShaderSource
// reads exactly that pair back.
Object.defineProperty(globalThis.WebGLRenderingContext.prototype, 'shaderSource', { value: function shaderSource(shader, source) { _glShSrc.set(shader, String(source)); }, writable: true, enumerable: true, configurable: true });
Object.defineProperty(globalThis.WebGL2RenderingContext.prototype, 'shaderSource', { value: function shaderSource(shader, source) { _glShSrc.set(shader, String(source)); }, writable: true, enumerable: true, configurable: true });
Object.defineProperty(globalThis.WebGLRenderingContext.prototype, 'compileShader', { value: function compileShader(shader) { _glShCompiled.add(shader); }, writable: true, enumerable: true, configurable: true });
Object.defineProperty(globalThis.WebGL2RenderingContext.prototype, 'compileShader', { value: function compileShader(shader) { _glShCompiled.add(shader); }, writable: true, enumerable: true, configurable: true });
// (k92-p6) WebGLContextEvent is a CONSTRUCTIBLE Event subclass, not one of the
// inert stubs (global_surface's THROW table SKIPs it — we are here first): mint
// through the real Event via Reflect.construct, statusMessage from the init dict,
// prototype own-name order [statusMessage, constructor] exactly as Chrome.
const _glCE_SM = new WeakMap();
globalThis.WebGLContextEvent = function WebGLContextEvent(type, init) {
  if (!new.target) throw new TypeError("Failed to construct 'WebGLContextEvent': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
  const e = Reflect.construct(globalThis.Event, [type, init || {}], new.target);
  _glCE_SM.set(e, String((init && init.statusMessage) || ''));
  return e;
};
Object.defineProperty(globalThis.WebGLContextEvent, 'length', { value: 1, configurable: true });
(function () {
  const p = globalThis.WebGLContextEvent.prototype;
  const ctor = Object.getOwnPropertyDescriptor(p, 'constructor');
  delete p.constructor;
  Object.defineProperty(p, 'statusMessage', { get: function statusMessage() { return _glCE_SM.has(this) ? _glCE_SM.get(this) : ''; }, enumerable: true, configurable: true });
  Object.defineProperty(p, 'constructor', ctor);
})();
// (k92-p6) WebGLObject.prototype carries the interface's own toStringTag — the
// minted-object chain ([object WebGLVertexArrayObjectOES] → [object WebGLObject])
// ends in a tagged prototype, not a bare Object.prototype.
Object.defineProperty(globalThis.WebGLObject.prototype, Symbol.toStringTag, { value: 'WebGLObject', writable: true, configurable: true });  // == bootstrap __def

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_glAttrs', _glAttrs);
  d('_glBaseOf', _glBaseOf);
  d('_glCanvasMap', _glCanvasMap);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
