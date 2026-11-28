//! DOMMatrix / DOMMatrixReadOnly, native. Handwritten (not declarative):
//! the constructor parses strings/sequences/dictionaries, methods take
//! variable numeric args with the missing-vs-undefined distinction, and
//! outputs include typed arrays — none of which the declarative layer
//! models. The laws still hold: brand in internal field 0, state as plain
//! numbers in fields 1..=17 (snapshot carries them), raw callbacks listed
//! in external_refs (append-only). Semantics transliterated from
//! crates/diting_js/js/platform/geometry.impl.js (Chrome-truth pinned).

use super::matmath as mm;
use super::matrix_conv::*;
use v8::{FunctionCallbackArguments, FunctionCallbackInfo, Local, ReturnValue};

const IS_2D_SLOT: [usize; 6] = [0, 1, 4, 5, 12, 13];

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [
        ctor as Cb,
        get as Cb,
        set as Cb,
        method as Cb,
        stat as Cb,
        point_matrix_transform as Cb,
    ]
    .into_iter()
    .map(|f| v8::ExternalReference { function: f })
    .collect()
}

// ---- attribute member tables --------------------------------------------
// get/set data encoding: member index into MEMBERS.
const MEMBERS: [(&str, i32); 24] = [
    ("a", 0),
    ("b", 1),
    ("c", 4),
    ("d", 5),
    ("e", 12),
    ("f", 13),
    ("m11", 0),
    ("m12", 1),
    ("m13", 2),
    ("m14", 3),
    ("m21", 4),
    ("m22", 5),
    ("m23", 6),
    ("m24", 7),
    ("m31", 8),
    ("m32", 9),
    ("m33", 10),
    ("m34", 11),
    ("m41", 12),
    ("m42", 13),
    ("m43", 14),
    ("m44", 15),
    ("is2D", -1),
    ("isIdentity", -2),
];

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let mutable = args.data().int32_value(scope).unwrap_or(0) != 0;
    let name = if mutable {
        "DOMMatrix"
    } else {
        "DOMMatrixReadOnly"
    };
    if !args.is_construct_call() {
        throw_type(scope, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", &format!("Failed to construct '{name}'"));
        return;
    }
    let ctx = format!("Failed to construct '{name}'");
    let Some((m, is2d)) = parse_init(scope, args.get(0), &ctx) else {
        return;
    };
    write_state(scope, args.this(), &m, is2d);
}

unsafe extern "C" fn get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let mi = args.data().int32_value(scope).unwrap_or(0) as usize;
    let Some((m, is2d)) = read_state(scope, args.this()) else {
        illegal(scope);
        return;
    };
    let mut rv = ReturnValue::from_function_callback_info(info);
    let (_, slot) = MEMBERS[mi];
    match slot {
        -1 => rv.set(v8::Boolean::new(scope, is2d).into()),
        -2 => rv.set(v8::Boolean::new(scope, mm::is_identity16(&m)).into()),
        i => rv.set(v8::Number::new(scope, m[i as usize]).into()),
    }
}

unsafe extern "C" fn set(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let mi = args.data().int32_value(scope).unwrap_or(0) as usize;
    let Some((mut m, mut is2d)) = read_state(scope, args.this()) else {
        illegal(scope);
        return;
    };
    let (mname, slot) = MEMBERS[mi];
    let prefix = format!("Failed to set the '{mname}' property on 'DOMMatrix'");
    let Some(n) = num(scope, args.get(0), &prefix) else {
        return;
    };
    m[slot as usize] = n;
    if !IS_2D_SLOT.contains(&(slot as usize)) {
        is2d = false;
    }
    write_state(scope, args.this(), &m, is2d);
}

// ---- method machinery -----------------------------------------------------
// Method ids on the RO prototype (install order = alphabetical per gen):
const RO_METHODS: [&str; 18] = [
    "flipX",
    "flipY",
    "inverse",
    "multiply",
    "rotate",
    "rotateAxisAngle",
    "rotateFromVector",
    "scale",
    "scale3d",
    "scaleNonUniform",
    "skewX",
    "skewY",
    "toFloat32Array",
    "toFloat64Array",
    "toJSON",
    "transformPoint",
    "translate",
    "toString",
];
// Mutating methods on DOMMatrix itself (install order per gen):
const MUT_METHODS: [&str; 12] = [
    "invertSelf",
    "multiplySelf",
    "preMultiplySelf",
    "rotateAxisAngleSelf",
    "rotateFromVectorSelf",
    "rotateSelf",
    "scale3dSelf",
    "scaleSelf",
    "skewXSelf",
    "skewYSelf",
    "translateSelf",
    "setMatrixValue",
];

/// scale origin handling shared by scale/scale3d legs (in place, columns
/// scaled directly so a NaN factor stays inside its own column).
// allow: the parameter list IS the spec operation's argument list
// (scale, origin-x/y/z) — bundling it into a struct would invent a type
// with no spec meaning.
#[allow(clippy::too_many_arguments)]
fn do_scale(
    m: &mut mm::M16,
    is2d: &mut bool,
    sx: f64,
    sy: f64,
    sz: f64,
    ox: f64,
    oy: f64,
    oz: f64,
) {
    let moved = ox != 0.0 || oy != 0.0 || oz != 0.0;
    if moved {
        mm::post_translate(m, ox, oy, oz);
    }
    for c in 0..3 {
        let s = if c == 0 {
            sx
        } else if c == 1 {
            sy
        } else {
            sz
        };
        for r in 0..4 {
            m[c * 4 + r] *= s;
        }
    }
    if moved {
        mm::post_translate(m, -ox, -oy, -oz);
    }
    if sz != 1.0 || oz != 0.0 {
        *is2d = false;
    }
}

fn f64array<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    m: &mm::M16,
    wide: bool,
) -> Option<Local<'s, v8::Value>> {
    let bytes = if wide { 16 * 8 } else { 16 * 4 };
    let ab = v8::ArrayBuffer::new(scope, bytes);
    {
        let data = ab.data()?;
        unsafe {
            for (i, v) in m.iter().enumerate() {
                if wide {
                    (data.as_ptr() as *mut f64).add(i).write(*v);
                } else {
                    (data.as_ptr() as *mut f32).add(i).write(*v as f32);
                }
            }
        }
    }
    if wide {
        let arr = v8::Float64Array::new(scope, ab, 0, 16)?;
        Some(arr.into())
    } else {
        let arr = v8::Float32Array::new(scope, ab, 0, 16)?;
        Some(arr.into())
    }
}

/// JS-style number formatting for toString: integers print without a
/// fraction, exponent notation outside [1e-6, 1e21). Covers the common
/// range exactly; extreme-exponent shapes are approximate.
fn js_num(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if n == 0.0 {
        return "0".into();
    }
    let a = n.abs();
    if (1e-6..1e21).contains(&a) {
        let s = format!("{n}");
        return s;
    }
    // exponent notation: JS uses e+21 style with no leading zeros
    let s = format!("{n:e}");
    // Rust gives "1e21"/"1.5e-7"; JS wants "1e+21"/"1.5e-7"
    if let Some(epos) = s.find('e') {
        let (mant, exp) = s.split_at(epos);
        let exp = &exp[1..];
        if exp.starts_with('-') {
            format!("{mant}e{exp}")
        } else {
            format!("{mant}e+{exp}")
        }
    } else {
        s
    }
}

fn matrix_to_json<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    m: &mm::M16,
    is2d: bool,
) -> Local<'s, v8::Value> {
    let obj = v8::Object::new(scope);
    let set = |scope: &mut v8::PinScope<'_, '_>, k: &str, v: Local<v8::Value>| {
        let key = v8::String::new(scope, k).unwrap();
        obj.set(scope, key.into(), v);
    };
    // lexicographic member order (truth dm.fromMatrix.empty): a..f, is2D,
    // isIdentity, m11..m44.
    for (k, idx) in [
        ("a", 0usize),
        ("b", 1),
        ("c", 4),
        ("d", 5),
        ("e", 12),
        ("f", 13),
    ] {
        let n = v8::Number::new(scope, m[idx]);
        set(scope, k, n.into());
    }
    let b = v8::Boolean::new(scope, is2d);
    set(scope, "is2D", b.into());
    let b = v8::Boolean::new(scope, mm::is_identity16(m));
    set(scope, "isIdentity", b.into());
    const NAMES: [&str; 16] = [
        "m11", "m12", "m13", "m14", "m21", "m22", "m23", "m24", "m31", "m32", "m33", "m34", "m41",
        "m42", "m43", "m44",
    ];
    for (k, name) in NAMES.iter().enumerate() {
        let n = v8::Number::new(scope, m[k]);
        set(scope, name, n.into());
    }
    obj.into()
}

unsafe extern "C" fn method(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let data = args.data().int32_value(scope).unwrap_or(0);
    let mutable_leg = data >= 100;
    let mid = if mutable_leg {
        (data - 100) as usize
    } else {
        data as usize
    };
    let name = if mutable_leg {
        MUT_METHODS[mid]
    } else {
        RO_METHODS[mid]
    };
    let iface = if mutable_leg {
        "DOMMatrix"
    } else {
        "DOMMatrixReadOnly"
    };
    let ctx = format!("Failed to execute '{name}' on '{iface}'");
    let Some((m, is2d)) = read_state(scope, args.this()) else {
        illegal(scope);
        return;
    };
    match name {
        // ---- non-mutating ops: mint a mutable DOMMatrix ----
        "translate" => {
            let Some(tx) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let Some(ty) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let Some(tz) = arg_num(&args, 2, 0.0, scope, &ctx) else {
                return;
            };
            let mut r = m;
            if tx != 0.0 || ty != 0.0 || tz != 0.0 {
                mm::post_translate(&mut r, tx, ty, tz);
            }
            let Some(o) = mint_matrix(scope, "DOMMatrix", &r, is2d && tz == 0.0) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "scale" | "scaleNonUniform" => {
            let Some(sx) = arg_num(&args, 0, 1.0, scope, &ctx) else {
                return;
            };
            let (syv, szv, oxv, oyv, ozv);
            if name == "scaleNonUniform" {
                // legacy: sy has a declared default of 1, no z/origin args
                let Some(v) = arg_num(&args, 1, 1.0, scope, &ctx) else {
                    return;
                };
                syv = v;
                szv = 1.0;
                oxv = 0.0;
                oyv = 0.0;
                ozv = 0.0;
            } else {
                // sy has NO declared default: missing -> sx, explicit
                // undefined -> NaN (truth ro.scale.z)
                if args.length() > 1 {
                    let Some(v) = num(scope, args.get(1), &ctx) else {
                        return;
                    };
                    syv = v;
                } else {
                    syv = sx;
                }
                let Some(v) = arg_num(&args, 2, 1.0, scope, &ctx) else {
                    return;
                };
                szv = v;
                let Some(v) = arg_num(&args, 3, 0.0, scope, &ctx) else {
                    return;
                };
                oxv = v;
                let Some(v) = arg_num(&args, 4, 0.0, scope, &ctx) else {
                    return;
                };
                oyv = v;
                let Some(v) = arg_num(&args, 5, 0.0, scope, &ctx) else {
                    return;
                };
                ozv = v;
            }
            let mut r = m;
            let mut r2d = is2d;
            do_scale(&mut r, &mut r2d, sx, syv, szv, oxv, oyv, ozv);
            let Some(o) = mint_matrix(scope, "DOMMatrix", &r, r2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "scale3d" => {
            let Some(s) = arg_num(&args, 0, 1.0, scope, &ctx) else {
                return;
            };
            let Some(ox) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let Some(oy) = arg_num(&args, 2, 0.0, scope, &ctx) else {
                return;
            };
            let Some(oz) = arg_num(&args, 3, 0.0, scope, &ctx) else {
                return;
            };
            let mut r = m;
            let mut r2d = is2d;
            do_scale(&mut r, &mut r2d, s, s, s, ox, oy, oz);
            let Some(o) = mint_matrix(scope, "DOMMatrix", &r, r2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "rotate" => {
            if let Some((rm, r2d)) = rotate_impl(scope, &args, &ctx, &m, is2d) {
                let Some(o) = mint_matrix(scope, "DOMMatrix", &rm, r2d) else {
                    return;
                };
                rv_set(info, scope, o.into());
            }
        }
        "rotateFromVector" => {
            let Some(x) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let Some(y) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let h = (x * x + y * y).sqrt();
            let out = if h == 0.0 {
                (m, is2d)
            } else {
                let mut r = mm::id16();
                r[0] = x / h;
                r[1] = y / h;
                r[4] = -y / h;
                r[5] = x / h;
                (mm::mul(&m, &r), is2d)
            };
            let Some(o) = mint_matrix(scope, "DOMMatrix", &out.0, out.1) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "rotateAxisAngle" => {
            let Some(x) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let Some(y) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let Some(z) = arg_num(&args, 2, 0.0, scope, &ctx) else {
                return;
            };
            let Some(ang) = arg_num(&args, 3, 0.0, scope, &ctx) else {
                return;
            };
            let rm = mm::axis_angle(x, y, z, ang);
            let out = mm::mul(&m, &rm);
            let r2d = is2d && x == 0.0 && y == 0.0 && z == 1.0;
            let Some(o) = mint_matrix(scope, "DOMMatrix", &out, r2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "skewX" | "skewY" => {
            let Some(a) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let mut r = mm::id16();
            if name == "skewX" {
                r[4] = (a * mm::D2R_).tan();
            } else {
                r[1] = (a * mm::D2R_).tan();
            }
            let out = mm::mul(&m, &r);
            let Some(o) = mint_matrix(scope, "DOMMatrix", &out, is2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "multiply" => {
            let Some(dict) = as_dict(scope, args.get(0), "DOMMatrixInit", &ctx) else {
                return;
            };
            let Some((om, o2d)) = read_matrix_init_opt(scope, dict, &ctx) else {
                return;
            };
            let out = mm::mul(&m, &om);
            let Some(o) = mint_matrix(scope, "DOMMatrix", &out, is2d && o2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "flipX" => {
            let mut f = mm::id16();
            f[0] = -1.0;
            let out = mm::mul(&m, &f);
            let Some(o) = mint_matrix(scope, "DOMMatrix", &out, is2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "flipY" => {
            let mut f = mm::id16();
            f[5] = -1.0;
            let out = mm::mul(&m, &f);
            let Some(o) = mint_matrix(scope, "DOMMatrix", &out, is2d) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "inverse" => match mm::inverse16(&m) {
            Some(inv) => {
                let Some(o) = mint_matrix(scope, "DOMMatrix", &inv, is2d) else {
                    return;
                };
                rv_set(info, scope, o.into());
            }
            None => {
                let nan = [f64::NAN; 16];
                let Some(o) = mint_matrix(scope, "DOMMatrix", &nan, false) else {
                    return;
                };
                rv_set(info, scope, o.into());
            }
        },
        "transformPoint" => {
            let Some(dict) = as_dict(scope, args.get(0), "DOMPointInit", &ctx) else {
                return;
            };
            let p = match dict {
                Some(d) => {
                    let Some(p) = read_point_init(scope, d, &ctx) else {
                        return;
                    };
                    p
                }
                None => [0.0, 0.0, 0.0, 1.0],
            };
            let out = mm::transform4(&m, p);
            let Some(o) = mint_point(scope, out) else {
                return;
            };
            rv_set(info, scope, o.into());
        }
        "toFloat32Array" => {
            let Some(v) = f64array(scope, &m, false) else {
                return;
            };
            rv_set(info, scope, v);
        }
        "toFloat64Array" => {
            let Some(v) = f64array(scope, &m, true) else {
                return;
            };
            rv_set(info, scope, v);
        }
        "toJSON" => {
            let v = matrix_to_json(scope, &m, is2d);
            rv_set(info, scope, v);
        }
        "toString" => {
            let s = if is2d {
                let parts: Vec<String> = [m[0], m[1], m[4], m[5], m[12], m[13]]
                    .iter()
                    .map(|n| js_num(*n))
                    .collect();
                format!("matrix({})", parts.join(", "))
            } else {
                let parts: Vec<String> = m.iter().map(|n| js_num(*n)).collect();
                format!("matrix3d({})", parts.join(", "))
            };
            let v = v8::String::new(scope, &s).unwrap();
            rv_set(info, scope, v.into());
        }
        // ---- mutating ops: mutate in place, return this ----
        "multiplySelf" | "preMultiplySelf" => {
            let Some(dict) = as_dict(scope, args.get(0), "DOMMatrixInit", &ctx) else {
                return;
            };
            let Some((om, o2d)) = read_matrix_init_opt(scope, dict, &ctx) else {
                return;
            };
            let out = if name == "multiplySelf" {
                mm::mul(&m, &om)
            } else {
                mm::mul(&om, &m)
            };
            write_state(scope, args.this(), &out, is2d && o2d);
            rv_set(info, scope, args.this().into());
        }
        "translateSelf" => {
            let Some(tx) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let Some(ty) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let Some(tz) = arg_num(&args, 2, 0.0, scope, &ctx) else {
                return;
            };
            let mut r = m;
            if tx != 0.0 || ty != 0.0 || tz != 0.0 {
                mm::post_translate(&mut r, tx, ty, tz);
            }
            write_state(scope, args.this(), &r, is2d && tz == 0.0);
            rv_set(info, scope, args.this().into());
        }
        "scaleSelf" => {
            let Some(sx) = arg_num(&args, 0, 1.0, scope, &ctx) else {
                return;
            };
            let syv = if args.length() > 1 {
                match num(scope, args.get(1), &ctx) {
                    Some(v) => v,
                    None => return,
                }
            } else {
                sx
            };
            let Some(szv) = arg_num(&args, 2, 1.0, scope, &ctx) else {
                return;
            };
            let Some(oxv) = arg_num(&args, 3, 0.0, scope, &ctx) else {
                return;
            };
            let Some(oyv) = arg_num(&args, 4, 0.0, scope, &ctx) else {
                return;
            };
            let Some(ozv) = arg_num(&args, 5, 0.0, scope, &ctx) else {
                return;
            };
            let mut r = m;
            let mut r2d = is2d;
            do_scale(&mut r, &mut r2d, sx, syv, szv, oxv, oyv, ozv);
            write_state(scope, args.this(), &r, r2d);
            rv_set(info, scope, args.this().into());
        }
        "scale3dSelf" => {
            let Some(s) = arg_num(&args, 0, 1.0, scope, &ctx) else {
                return;
            };
            let Some(ox) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let Some(oy) = arg_num(&args, 2, 0.0, scope, &ctx) else {
                return;
            };
            let Some(oz) = arg_num(&args, 3, 0.0, scope, &ctx) else {
                return;
            };
            let mut r = m;
            let mut r2d = is2d;
            do_scale(&mut r, &mut r2d, s, s, s, ox, oy, oz);
            write_state(scope, args.this(), &r, r2d);
            rv_set(info, scope, args.this().into());
        }
        "rotateSelf" => {
            if let Some((rm, r2d)) = rotate_impl(scope, &args, &ctx, &m, is2d) {
                write_state(scope, args.this(), &rm, r2d);
                rv_set(info, scope, args.this().into());
            }
        }
        "rotateFromVectorSelf" => {
            let Some(x) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let Some(y) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let h = (x * x + y * y).sqrt();
            let mut r = m;
            if h != 0.0 {
                let mut t = mm::id16();
                t[0] = x / h;
                t[1] = y / h;
                t[4] = -y / h;
                t[5] = x / h;
                r = mm::mul(&r, &t);
            }
            write_state(scope, args.this(), &r, is2d);
            rv_set(info, scope, args.this().into());
        }
        "rotateAxisAngleSelf" => {
            let Some(x) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let Some(y) = arg_num(&args, 1, 0.0, scope, &ctx) else {
                return;
            };
            let Some(z) = arg_num(&args, 2, 0.0, scope, &ctx) else {
                return;
            };
            let Some(ang) = arg_num(&args, 3, 0.0, scope, &ctx) else {
                return;
            };
            let rm = mm::axis_angle(x, y, z, ang);
            let out = mm::mul(&m, &rm);
            let r2d = if !(x == 0.0 && y == 0.0 && z == 1.0) {
                false
            } else {
                is2d
            };
            write_state(scope, args.this(), &out, r2d);
            rv_set(info, scope, args.this().into());
        }
        "skewXSelf" | "skewYSelf" => {
            let Some(a) = arg_num(&args, 0, 0.0, scope, &ctx) else {
                return;
            };
            let mut t = mm::id16();
            if name == "skewXSelf" {
                t[4] = (a * mm::D2R_).tan();
            } else {
                t[1] = (a * mm::D2R_).tan();
            }
            let out = mm::mul(&m, &t);
            write_state(scope, args.this(), &out, is2d);
            rv_set(info, scope, args.this().into());
        }
        "invertSelf" => {
            match mm::inverse16(&m) {
                Some(inv) => write_state(scope, args.this(), &inv, is2d),
                None => write_state(scope, args.this(), &[f64::NAN; 16], false),
            }
            rv_set(info, scope, args.this().into());
        }
        "setMatrixValue" => {
            if args.length() < 1 {
                throw_type(scope, "1 argument required, but only 0 present.", &ctx);
                return;
            }
            let Some(s) = num2str(scope, args.get(0), &ctx) else {
                return;
            };
            match mm::parse_transform_list(&s) {
                Ok(p) => {
                    write_state(scope, args.this(), &p.m, p.is2d);
                    rv_set(info, scope, args.this().into());
                }
                Err(mm::ParseErr::Resolvable) => {
                    dom_fail(scope, "Values must be resolvable at parse time", &ctx)
                }
                Err(mm::ParseErr::Failed) => {
                    dom_fail(scope, &format!("Failed to parse '{s}'."), &ctx)
                }
            }
        }
        _ => illegal(scope),
    }
}

unsafe extern "C" fn point_matrix_transform(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let ctx = "Failed to execute 'matrixTransform' on 'DOMPointReadOnly'";
    let this = args.this();
    let branded = (|| -> Option<bool> {
        if this.internal_field_count() != 5 {
            return Some(false);
        }
        let tag: Local<v8::Value> = this.get_internal_field(scope, 0)?.try_into().ok()?;
        Some(tag.int32_value(scope)? == 0x0D02)
    })();
    if branded != Some(true) {
        illegal(scope);
        return;
    }
    let mut p = [0.0, 0.0, 0.0, 1.0];
    for (i, slot) in p.iter_mut().enumerate() {
        let Some(f) = this.get_internal_field(scope, i + 1) else {
            return;
        };
        let Ok(f) = Local::<v8::Value>::try_from(f) else {
            return;
        };
        let Some(n) = f.number_value(scope) else {
            return;
        };
        *slot = n;
    }
    let Some(dict) = as_dict(scope, args.get(0), "DOMMatrixInit", ctx) else {
        return;
    };
    let Some((m, _)) = read_matrix_init_opt(scope, dict, ctx) else {
        return;
    };
    let out = mm::transform4(&m, p);
    let Some(o) = mint_point(scope, out) else {
        return;
    };
    rv_set(info, scope, o.into());
}

fn rv_set(info: &FunctionCallbackInfo, _scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>) {
    let mut rv = ReturnValue::from_function_callback_info(info);
    rv.set(v);
}

/// rotate(): single-arg is a Z rotation that keeps is2D; with rotY/rotZ
/// engaged (even explicit undefined -> NaN) it is the three-axis path.
fn rotate_impl(
    scope: &mut v8::PinScope<'_, '_>,
    args: &FunctionCallbackArguments,
    ctx: &str,
    m: &mm::M16,
    is2d: bool,
) -> Option<(mm::M16, bool)> {
    let rx = arg_num(args, 0, 0.0, scope, ctx)?;
    let single = args.length() < 2;
    let (out, r2d) = if single {
        let t = mm::rot_z(rx);
        (mm::mul(m, &t), is2d)
    } else {
        let ry = num(scope, args.get(1), ctx)?;
        let rz = if args.length() < 3 {
            0.0
        } else {
            num(scope, args.get(2), ctx)?
        };
        let mx = mm::rot_x(rx);
        let my = mm::rot_y(ry);
        let mz = mm::rot_z(rz);
        let mut acc = mm::mul(m, &mx);
        acc = mm::mul(&acc, &my);
        acc = mm::mul(&acc, &mz);
        (acc, is2d && rx == 0.0 && ry == 0.0)
    };
    Some((out, r2d))
}

/// DOMMatrixInit read where null/undefined input means the empty dict.
fn read_matrix_init_opt(
    scope: &mut v8::PinScope<'_, '_>,
    dict: Option<Local<v8::Object>>,
    ctx: &str,
) -> Option<(mm::M16, bool)> {
    match dict {
        Some(d) => read_matrix_init(scope, d, ctx),
        None => Some((mm::id16(), true)),
    }
}

unsafe extern "C" fn stat(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let data = args.data().int32_value(scope).unwrap_or(0);
    // encoding: mutable*10 + kind; kind 0=fromMatrix 1=fromFloat32 2=fromFloat64
    let mutable = data >= 10;
    let kind = if mutable { data - 10 } else { data };
    let cls = if mutable {
        "DOMMatrix"
    } else {
        "DOMMatrixReadOnly"
    };
    let fname = match kind {
        0 => "fromMatrix",
        1 => "fromFloat32Array",
        _ => "fromFloat64Array",
    };
    let ctx = format!("Failed to execute '{fname}' on '{cls}'");
    if kind == 0 {
        let Some(dict) = as_dict(scope, args.get(0), "DOMMatrixInit", &ctx) else {
            return;
        };
        let Some((m, is2d)) = read_matrix_init_opt(scope, dict, &ctx) else {
            return;
        };
        let Some(o) = mint_matrix(scope, cls, &m, is2d) else {
            return;
        };
        rv_set(info, scope, o.into());
        return;
    }
    // fromFloat32Array / fromFloat64Array: exact typed-array brand check.
    if args.length() < 1 {
        throw_type(scope, "1 argument required, but only 0 present.", &ctx);
        return;
    }
    let a = args.get(0);
    let ok = if kind == 1 {
        a.is_float32_array()
    } else {
        a.is_float64_array()
    };
    if !ok {
        let tn = if kind == 1 {
            "Float32Array"
        } else {
            "Float64Array"
        };
        throw_type(
            scope,
            &format!("The provided value is not of type '{tn}'."),
            &ctx,
        );
        return;
    }
    let arr = Local::<v8::TypedArray>::try_from(a);
    let Ok(arr) = arr else { return };
    let len = arr.length();
    if len != 6 && len != 16 {
        throw_type(
            scope,
            "The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix.",
            &ctx,
        );
        return;
    }
    let mut m = mm::id16();
    if len == 6 {
        const SLOTS6: [usize; 6] = [0, 1, 4, 5, 12, 13];
        for (k, slot) in SLOTS6.iter().enumerate() {
            let idx = v8::Number::new(scope, k as f64);
            let Some(v) = arr.get(scope, idx.into()) else {
                return;
            };
            m[*slot] = v.number_value(scope).unwrap_or(f64::NAN);
        }
        let Some(o) = mint_matrix(scope, cls, &m, true) else {
            return;
        };
        rv_set(info, scope, o.into());
        return;
    }
    for (k, slot) in m.iter_mut().enumerate() {
        let idx = v8::Number::new(scope, k as f64);
        let Some(v) = arr.get(scope, idx.into()) else {
            return;
        };
        *slot = v.number_value(scope).unwrap_or(f64::NAN);
    }
    let Some(o) = mint_matrix(scope, cls, &m, false) else {
        return;
    };
    rv_set(info, scope, o.into());
}

/// Move prototype keys into Chrome's exact install order. V8 templates
/// always place `constructor` after every template member; Chrome's IDL
/// order interleaves it (constructor before toString/setMatrixValue, or
/// matrixTransform before toJSON). Every tail key here is a plain data
/// property (constructor or method), so capture the value, delete, and
/// redefine in sequence: `non_enum` marks the constructor.
fn reorder_tail(scope: &mut v8::PinScope<'_, '_>, proto: Local<v8::Object>, tail: &[(&str, bool)]) {
    let mut saved: Vec<(bool, Local<v8::Value>)> = Vec::new();
    for (name, _) in tail {
        let key = v8::String::new(scope, name).unwrap();
        let v = proto.get(scope, key.into());
        proto.delete(scope, key.into());
        saved.push((false, v.unwrap_or_else(|| v8::undefined(scope).into())));
    }
    for ((name, non_enum), (_, v)) in tail.iter().zip(saved.iter()) {
        let key = v8::String::new(scope, name).unwrap();
        let attrs = if *non_enum {
            v8::PropertyAttribute::DONT_ENUM
        } else {
            v8::PropertyAttribute::NONE
        };
        proto.define_own_property(scope, key.into(), *v, attrs);
    }
}

fn builder<'s>(
    scope: &mut v8::PinScope<'s, '_, ()>,
    cb: Cb,
    data: i32,
    len: i32,
) -> Local<'s, v8::FunctionTemplate> {
    let d = v8::Integer::new(scope, data);
    v8::FunctionTemplate::builder_raw(cb)
        .data(d.into())
        .length(len)
        .build(scope)
}

fn name_of<'s>(scope: &mut v8::PinScope<'s, '_, ()>, s: &str) -> Local<'s, v8::String> {
    v8::String::new(scope, s).unwrap()
}

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    let tag_key = v8::Symbol::get_to_string_tag(scope);

    // --- DOMMatrixReadOnly ---
    let ro_t = builder(scope, ctor, 0, 0);
    let ro_nm = name_of(scope, "DOMMatrixReadOnly");
    ro_t.set_class_name(ro_nm);
    ro_t.instance_template(scope)
        .set_internal_field_count(NFIELDS);
    ro_t.read_only_prototype();
    let proto = ro_t.prototype_template(scope);
    proto.set_with_attr(
        tag_key.into(),
        ro_nm.into(),
        v8::PropertyAttribute::READ_ONLY | v8::PropertyAttribute::DONT_ENUM,
    );
    for (mi, (mname, _)) in MEMBERS.iter().enumerate() {
        let g = builder(scope, get, mi as i32, 0);
        let gn = name_of(scope, &format!("get {mname}"));
        g.set_class_name(gn);
        let key = name_of(scope, mname);
        proto.set_accessor_property(key.into(), Some(g), None, v8::PropertyAttribute::NONE);
    }
    for (mi, mname) in RO_METHODS.iter().enumerate() {
        let len = 0;
        let f = builder(scope, method, mi as i32, len);
        let key = name_of(scope, mname);
        proto.set_with_attr(key.into(), f.into(), v8::PropertyAttribute::NONE);
    }
    for (kind, (fname, flen)) in [
        ("fromMatrix", 0),
        ("fromFloat32Array", 1),
        ("fromFloat64Array", 1),
    ]
    .iter()
    .enumerate()
    {
        let f = builder(scope, stat, kind as i32, *flen);
        let key = name_of(scope, fname);
        ro_t.set_with_attr(key.into(), f.into(), v8::PropertyAttribute::NONE);
    }

    // --- DOMMatrix ---
    let mu_t = builder(scope, ctor, 1, 0);
    let mu_nm = name_of(scope, "DOMMatrix");
    mu_t.set_class_name(mu_nm);
    mu_t.instance_template(scope)
        .set_internal_field_count(NFIELDS);
    mu_t.inherit(ro_t);
    mu_t.read_only_prototype();
    let mproto = mu_t.prototype_template(scope);
    mproto.set_with_attr(
        tag_key.into(),
        mu_nm.into(),
        v8::PropertyAttribute::READ_ONLY | v8::PropertyAttribute::DONT_ENUM,
    );
    // rw numeric attrs only (is2D/isIdentity are inherited getters)
    for (mi, (mname, slot)) in MEMBERS.iter().enumerate() {
        if *slot < 0 {
            continue;
        }
        let g = builder(scope, get, mi as i32, 0);
        let gn = name_of(scope, &format!("get {mname}"));
        g.set_class_name(gn);
        let s = builder(scope, set, mi as i32, 1);
        let sn = name_of(scope, &format!("set {mname}"));
        s.set_class_name(sn);
        let key = name_of(scope, mname);
        mproto.set_accessor_property(key.into(), Some(g), Some(s), v8::PropertyAttribute::NONE);
    }
    for (mi, mname) in MUT_METHODS.iter().enumerate() {
        let len = if *mname == "setMatrixValue" { 1 } else { 0 };
        let f = builder(scope, method, 100 + mi as i32, len);
        let key = name_of(scope, mname);
        mproto.set_with_attr(key.into(), f.into(), v8::PropertyAttribute::NONE);
    }
    for (kind, (fname, flen)) in [
        ("fromMatrix", 0),
        ("fromFloat32Array", 1),
        ("fromFloat64Array", 1),
    ]
    .iter()
    .enumerate()
    {
        let f = builder(scope, stat, 10 + kind as i32, *flen);
        let key = name_of(scope, fname);
        mu_t.set_with_attr(key.into(), f.into(), v8::PropertyAttribute::NONE);
    }

    let ro_f = ro_t.get_function(scope);
    let mu_f = mu_t.get_function(scope);
    // interface objects chain like their prototypes
    if let (Some(rof), Some(muf)) = (ro_f, mu_f) {
        muf.set_prototype(scope, rof.into());
        for (f, nm) in [(rof, "DOMMatrixReadOnly"), (muf, "DOMMatrix")] {
            let key = v8::String::new(scope, nm).unwrap();
            global.define_own_property(
                scope,
                key.into(),
                f.into(),
                v8::PropertyAttribute::DONT_ENUM,
            );
        }
        // Chrome's IDL key order interleaves constructor: before toString
        // on the RO prototype, before setMatrixValue on the mutable one.
        let protk = v8::String::new(scope, "prototype").unwrap();
        if let Some(p) = rof.get(scope, protk.into()) {
            if let Ok(p) = Local::<v8::Object>::try_from(p) {
                reorder_tail(scope, p, &[("constructor", true), ("toString", false)]);
            }
        }
        if let Some(p) = muf.get(scope, protk.into()) {
            if let Ok(p) = Local::<v8::Object>::try_from(p) {
                reorder_tail(
                    scope,
                    p,
                    &[("constructor", true), ("setMatrixValue", false)],
                );
            }
        }
        // WebKitCSSMatrix is the SAME constructor object under a legacy
        // alias - not a twin (truth wkcm.*).
        let key = v8::String::new(scope, "WebKitCSSMatrix").unwrap();
        global.define_own_property(
            scope,
            key.into(),
            muf.into(),
            v8::PropertyAttribute::DONT_ENUM,
        );
    }
    // DOMPointReadOnly.matrixTransform - dict-arg method the declarative
    // point interface cannot express; install onto the live prototype.
    let pk = v8::String::new(scope, "DOMPointReadOnly").unwrap();
    if let Some(pc) = global.get(scope, pk.into()) {
        if let Ok(pc) = Local::<v8::Function>::try_from(pc) {
            let protk = v8::String::new(scope, "prototype").unwrap();
            if let Some(proto) = pc.get(scope, protk.into()) {
                if let Ok(proto) = Local::<v8::Object>::try_from(proto) {
                    // Chrome order: x,y,z,w,matrixTransform,toJSON,
                    // constructor. Delete toJSON+constructor, append
                    // matrixTransform, then re-append the two in order.
                    let tjk = v8::String::new(scope, "toJSON").unwrap();
                    let tjv = proto.get(scope, tjk.into());
                    proto.delete(scope, tjk.into());
                    let ck = v8::String::new(scope, "constructor").unwrap();
                    let cv = proto.get(scope, ck.into());
                    proto.delete(scope, ck.into());
                    let mt = builder(scope, point_matrix_transform, 0, 0);
                    let mn = v8::String::new(scope, "matrixTransform").unwrap();
                    mt.set_class_name(mn);
                    if let Some(mtf) = mt.get_function(scope) {
                        let mk = v8::String::new(scope, "matrixTransform").unwrap();
                        proto.define_own_property(
                            scope,
                            mk.into(),
                            mtf.into(),
                            v8::PropertyAttribute::NONE,
                        );
                    }
                    if let Some(tjv) = tjv {
                        proto.define_own_property(
                            scope,
                            tjk.into(),
                            tjv,
                            v8::PropertyAttribute::NONE,
                        );
                    }
                    if let Some(cv) = cv {
                        proto.define_own_property(
                            scope,
                            ck.into(),
                            cv,
                            v8::PropertyAttribute::DONT_ENUM,
                        );
                    }
                }
            }
        }
    }
}
