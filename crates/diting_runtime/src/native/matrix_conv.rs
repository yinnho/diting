#![allow(dead_code)] // some helpers serve matrix_methods callers only

/// Brand tag (0x0D01 rect, 0x0D02 point exist; matrix is the next family).
/// field 0 = tag, 1..=16 = m, 17 = is2D as 1.0/0.0.
pub(crate) const FAMILY: i32 = 0x0D03;
pub(crate) const NFIELDS: usize = 18;

use super::matmath as mm;
use v8::{FunctionCallbackArguments, Local};

pub(crate) fn num(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>, ctx: &str) -> Option<f64> {
    super::convert::unrestricted_double(scope, v, ctx)
}

pub(crate) fn num2str(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    ctx: &str,
) -> Option<String> {
    super::convert::dom_string(scope, v, ctx)
}

/// WebIDL arg with a declared default: undefined -> default.
pub(crate) fn arg_num(
    args: &FunctionCallbackArguments,
    i: i32,
    dflt: f64,
    scope: &mut v8::PinScope<'_, '_>,
    ctx: &str,
) -> Option<f64> {
    let v = args.get(i);
    if v.is_undefined() {
        Some(dflt)
    } else {
        num(scope, v, ctx)
    }
}

/// dictNum: undefined/null -> default; symbol -> error with the
/// "Failed to read the '<k>' property" chain; else Number(v).
pub(crate) fn dict_num(
    scope: &mut v8::PinScope<'_, '_>,
    d: Local<v8::Object>,
    k: &str,
    dflt: f64,
    dict: &str,
    ctx: &str,
) -> Option<Option<f64>> {
    let key = v8::String::new(scope, k).unwrap();
    let v = d.get(scope, key.into())?;
    if v.is_undefined() || v.is_null() {
        return Some(Some(dflt));
    }
    if v.is_symbol() {
        let tail = format!("{ctx}: Failed to read the '{k}' property from '{dict}'");
        super::convert::fail(
            scope,
            "Cannot convert a Symbol value to a number",
            Some(&tail),
        );
        return None;
    }
    v.number_value(scope).map(Some)
}

/// readMatrixInit — 3D-ness is VALUE-based against each member's
/// dictionary default; members are read twice exactly like the JS
/// (detection pass in M3D order, then the lexicographic ALIAS pass), so
/// getter side effects observe the identical sequence.
pub(crate) fn read_matrix_init(
    scope: &mut v8::PinScope<'_, '_>,
    d: Local<v8::Object>,
    ctx: &str,
) -> Option<(mm::M16, bool)> {
    const M3D: [(&str, usize, f64); 10] = [
        ("m13", 2, 0.0),
        ("m14", 3, 0.0),
        ("m23", 6, 0.0),
        ("m24", 7, 0.0),
        ("m31", 8, 0.0),
        ("m32", 9, 0.0),
        ("m33", 10, 1.0),
        ("m34", 11, 0.0),
        ("m43", 14, 0.0),
        ("m44", 15, 1.0),
    ];
    let mut has3d = false;
    for (k, _, dflt) in M3D {
        // detection pass: null counts as absent
        let key = v8::String::new(scope, k).unwrap();
        let v = d.get(scope, key.into())?;
        if !(v.is_undefined() || v.is_null()) {
            if v.is_symbol() {
                let tail = format!("{ctx}: Failed to read the '{k}' property from 'DOMMatrixInit'");
                super::convert::fail(
                    scope,
                    "Cannot convert a Symbol value to a number",
                    Some(&tail),
                );
                return None;
            }
            let n = v.number_value(scope)?;
            if n != dflt {
                has3d = true;
                break;
            }
        }
    }
    let is2d_key = v8::String::new(scope, "is2D").unwrap();
    let is2d_v = d.get(scope, is2d_key.into())?;
    let is2d = if !is2d_v.is_undefined() {
        let b = super::convert::boolean(scope, is2d_v);
        if b && has3d {
            throw_type(
                scope,
                "The is2D member is set to true but the input matrix is a 3d matrix.",
                ctx,
            );
            return None;
        }
        b
    } else {
        !has3d
    };
    const ALIAS: [(&str, usize); 22] = [
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
    ];
    let mut m = mm::id16();
    for (k, idx) in ALIAS {
        let key = v8::String::new(scope, k).unwrap();
        let v = d.get(scope, key.into())?;
        if !v.is_undefined() {
            let n = dict_num(scope, d, k, 0.0, "DOMMatrixInit", ctx)?;
            m[idx] = n.unwrap_or(0.0);
        }
    }
    Some((m, is2d))
}

pub(crate) fn read_point_init(
    scope: &mut v8::PinScope<'_, '_>,
    d: Local<v8::Object>,
    ctx: &str,
) -> Option<[f64; 4]> {
    let mut out = [0.0, 0.0, 0.0, 1.0];
    for (i, k) in ["x", "y", "z", "w"].iter().enumerate() {
        let dflt = if *k == "w" { 1.0 } else { 0.0 };
        let n = dict_num(scope, d, k, dflt, "DOMPointInit", ctx)?;
        out[i] = n.unwrap_or(dflt);
    }
    Some(out)
}

/// asDict: null/undefined -> empty object semantics; primitives fail.
pub(crate) fn as_dict<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    v: Local<'s, v8::Value>,
    dict: &str,
    ctx: &str,
) -> Option<Option<Local<'s, v8::Object>>> {
    if v.is_null() || v.is_undefined() {
        return Some(None);
    }
    if !v.is_object() && !v.is_function() {
        throw_type(
            scope,
            &format!("The provided value is not of type '{dict}'."),
            ctx,
        );
        return None;
    }
    Local::<v8::Object>::try_from(v).ok().map(Some)
}

/// Instantiate `name` through the global constructor with no args, then
/// overwrite the matrix state — constructor-signature differences never
/// leak into method code.
pub(crate) fn mint_matrix<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    cls: &str,
    m: &mm::M16,
    is2d: bool,
) -> Option<Local<'s, v8::Object>> {
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, cls).unwrap();
    let c = global.get(scope, k.into())?;
    let c = Local::<v8::Function>::try_from(c).ok()?;
    let o = c.new_instance(scope, &[])?;
    write_state(scope, o, m, is2d);
    Some(o)
}

pub(crate) fn mint_point<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    v: [f64; 4],
) -> Option<Local<'s, v8::Object>> {
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, "DOMPoint").unwrap();
    let c = global.get(scope, k.into())?;
    let c = Local::<v8::Function>::try_from(c).ok()?;
    let argv: Vec<Local<v8::Value>> = v
        .iter()
        .map(|n| v8::Number::new(scope, *n).into())
        .collect();
    let o = c.new_instance(scope, &argv)?;
    Some(o)
}

/// Throw a DOMException SyntaxError with the decorated enumerable message
/// (mirrors domFail in the JS).
pub(crate) fn dom_fail(scope: &mut v8::PinScope<'_, '_>, msg: &str, ctx: &str) {
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, "DOMException").unwrap();
    let Some(cv) = global.get(scope, k.into()) else {
        return;
    };
    let Ok(c) = Local::<v8::Function>::try_from(cv) else {
        return;
    };
    let m1 = v8::String::new(scope, msg).unwrap();
    let m2 = v8::String::new(scope, "SyntaxError").unwrap();
    let argv: [Local<v8::Value>; 2] = [m1.into(), m2.into()];
    let Some(e) = c.new_instance(scope, &argv) else {
        return;
    };
    let decorated = format!("{ctx}: {msg}");
    let key = v8::String::new(scope, "message").unwrap();
    let val = v8::String::new(scope, &decorated).unwrap();
    e.define_own_property(scope, key.into(), val.into(), v8::PropertyAttribute::NONE);
    scope.throw_exception(e.into());
}

/// Constructor init parsing: undefined/null -> identity; string ->
/// transform list; sequence -> 6/16 elements; object with numeric m11 ->
/// dictionary copy; anything else -> stringified then parsed.
pub(crate) fn parse_init(
    scope: &mut v8::PinScope<'_, '_>,
    init: Local<v8::Value>,
    ctx: &str,
) -> Option<(mm::M16, bool)> {
    if init.is_undefined() || init.is_null() {
        return Some((mm::id16(), true));
    }
    if init.is_string() {
        let s = init.to_string(scope)?.to_rust_string_lossy(scope);
        return match mm::parse_transform_list(&s) {
            Ok(p) => Some((p.m, p.is2d)),
            Err(mm::ParseErr::Resolvable) => {
                dom_fail(scope, "Values must be resolvable at parse time", ctx);
                None
            }
            Err(mm::ParseErr::Failed) => {
                dom_fail(scope, &format!("Failed to parse '{s}'."), ctx);
                None
            }
        };
    }
    // sequence leg: object/function with a callable @@iterator
    if init.is_object() || init.is_function() {
        if let Ok(obj) = Local::<v8::Object>::try_from(init) {
            let sym = v8::Symbol::get_iterator(scope);
            if let Some(it) = obj.get(scope, sym.into()) {
                if it.is_function() {
                    let items = super::convert::sequence(scope, init, ctx)?;
                    if items.len() != 6 && items.len() != 16 {
                        throw_type(scope, "The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix.", ctx);
                        return None;
                    }
                    let mut m = mm::id16();
                    if items.len() == 6 {
                        const SLOTS6: [usize; 6] = [0, 1, 4, 5, 12, 13];
                        for (k, slot) in SLOTS6.iter().enumerate() {
                            m[*slot] = num(scope, items[k], ctx)?;
                        }
                        return Some((m, true));
                    }
                    for k in 0..16 {
                        m[k] = num(scope, items[k], ctx)?;
                    }
                    return Some((m, false));
                }
            }
            // copy-ctor leg: object with a numeric m11
            let k11 = v8::String::new(scope, "m11").unwrap();
            if let Some(m11) = obj.get(scope, k11.into()) {
                if m11.is_number() {
                    return read_matrix_init(scope, obj, ctx);
                }
            }
        }
    }
    let s = num2str(scope, init, ctx)?;
    match mm::parse_transform_list(&s) {
        Ok(p) => Some((p.m, p.is2d)),
        Err(mm::ParseErr::Resolvable) => {
            dom_fail(scope, "Values must be resolvable at parse time", ctx);
            None
        }
        Err(mm::ParseErr::Failed) => {
            dom_fail(scope, &format!("Failed to parse '{s}'."), ctx);
            None
        }
    }
}

pub(crate) fn throw_type(scope: &mut v8::PinScope<'_, '_>, msg: &str, prefix: &str) {
    super::convert::fail(scope, msg, Some(prefix));
}

pub(crate) fn illegal(scope: &mut v8::PinScope<'_, '_>) {
    super::convert::illegal(scope);
}

pub(crate) fn read_state(
    scope: &mut v8::PinScope<'_, '_>,
    this: Local<v8::Object>,
) -> Option<(mm::M16, bool)> {
    if this.internal_field_count() != NFIELDS {
        return None;
    }
    let tag: Local<v8::Value> = this.get_internal_field(scope, 0)?.try_into().ok()?;
    if tag.int32_value(scope)? != FAMILY {
        return None;
    }
    let mut m = [0.0; 16];
    for (i, slot) in m.iter_mut().enumerate() {
        let f: Local<v8::Value> = this.get_internal_field(scope, i + 1)?.try_into().ok()?;
        *slot = f.number_value(scope)?;
    }
    let f: Local<v8::Value> = this.get_internal_field(scope, 17)?.try_into().ok()?;
    Some((m, f.number_value(scope)? != 0.0))
}

pub(crate) fn write_state(
    scope: &mut v8::PinScope<'_, '_>,
    obj: Local<v8::Object>,
    m: &mm::M16,
    is2d: bool,
) {
    let tag = v8::Integer::new(scope, FAMILY);
    obj.set_internal_field(0, tag.into());
    for (i, v) in m.iter().enumerate() {
        let n = v8::Number::new(scope, *v);
        obj.set_internal_field(i + 1, n.into());
    }
    let f = v8::Number::new(scope, if is2d { 1.0 } else { 0.0 });
    obj.set_internal_field(17, f.into());
}
