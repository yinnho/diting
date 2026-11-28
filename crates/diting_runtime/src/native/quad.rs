//! DOMQuad, native. State is four DOMPoint object references in internal
//! fields 1..=4 (object identities are stable: p1 returns the same object
//! every time — truth dq.ctor). Handwritten like matrix.rs; semantics from
//! geometry.impl.js (Chrome-truth pinned). Note: the legacy `bounds`
//! getter is gone from modern Chrome (truth dq.boundsGone) — only
//! getBounds() exists.

use v8::{FunctionCallbackArguments, FunctionCallbackInfo, Local, PropertyAttribute, ReturnValue};

const FAMILY: i32 = 0x0D04;
const NFIELDS: usize = 5;

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [ctor as Cb, get as Cb, method as Cb, stat as Cb]
        .into_iter()
        .map(|f| v8::ExternalReference { function: f })
        .collect()
}

fn illegal(scope: &mut v8::PinScope<'_, '_>) {
    super::convert::illegal(scope);
}

fn throw_type(scope: &mut v8::PinScope<'_, '_>, msg: &str, prefix: &str) {
    super::convert::fail(scope, msg, Some(prefix));
}

fn branded(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>) -> Option<()> {
    if this.internal_field_count() != NFIELDS {
        return None;
    }
    let tag: Local<v8::Value> = this.get_internal_field(scope, 0)?.try_into().ok()?;
    if tag.int32_value(scope)? != FAMILY {
        return None;
    }
    Some(())
}

fn point_at<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    this: Local<'s, v8::Object>,
    i: usize,
) -> Option<Local<'s, v8::Object>> {
    let v: Local<v8::Value> = this.get_internal_field(scope, i)?.try_into().ok()?;
    Local::<v8::Object>::try_from(v).ok()
}

/// Mint a DOMPoint through the global constructor (right prototype).
fn mint_point<'s>(scope: &mut v8::PinScope<'s, '_>, v: [f64; 4]) -> Option<Local<'s, v8::Object>> {
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, "DOMPoint").unwrap();
    let c = global.get(scope, k.into())?;
    let c = Local::<v8::Function>::try_from(c).ok()?;
    let argv: Vec<Local<v8::Value>> = v
        .iter()
        .map(|n| v8::Number::new(scope, *n).into())
        .collect();
    c.new_instance(scope, &argv)
}

/// Read one DOMPointInit dict argument: null/undefined -> defaults;
/// primitives fail; otherwise x/y/z/w members with defaults 0/0/0/1.
fn read_point_arg(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    ctx: &str,
) -> Option<[f64; 4]> {
    if v.is_undefined() || v.is_null() {
        return Some([0.0, 0.0, 0.0, 1.0]);
    }
    if !v.is_object() && !v.is_function() {
        throw_type(
            scope,
            "The provided value is not of type 'DOMPointInit'.",
            ctx,
        );
        return None;
    }
    let d = Local::<v8::Object>::try_from(v).ok()?;
    let mut out = [0.0, 0.0, 0.0, 1.0];
    for (i, k) in ["x", "y", "z", "w"].iter().enumerate() {
        let key = v8::String::new(scope, k).unwrap();
        let raw = d.get(scope, key.into())?;
        if raw.is_undefined() || raw.is_null() {
            continue;
        }
        if raw.is_symbol() {
            let tail = format!("{ctx}: Failed to read the '{k}' property from 'DOMPointInit'");
            super::convert::fail(
                scope,
                "Cannot convert a Symbol value to a number",
                Some(&tail),
            );
            return None;
        }
        out[i] = raw.number_value(scope)?;
    }
    Some(out)
}

fn read_rect_init(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    ctx: &str,
) -> Option<[f64; 4]> {
    if v.is_undefined() || v.is_null() {
        return Some([0.0, 0.0, 0.0, 0.0]);
    }
    if !v.is_object() && !v.is_function() {
        throw_type(
            scope,
            "The provided value is not of type 'DOMRectInit'.",
            ctx,
        );
        return None;
    }
    let d = Local::<v8::Object>::try_from(v).ok()?;
    let mut out = [0.0; 4];
    for (i, k) in ["x", "y", "width", "height"].iter().enumerate() {
        let key = v8::String::new(scope, k).unwrap();
        let raw = d.get(scope, key.into())?;
        if raw.is_undefined() || raw.is_null() {
            continue;
        }
        if raw.is_symbol() {
            let tail = format!("{ctx}: Failed to read the '{k}' property from 'DOMRectInit'");
            super::convert::fail(
                scope,
                "Cannot convert a Symbol value to a number",
                Some(&tail),
            );
            return None;
        }
        out[i] = raw.number_value(scope)?;
    }
    Some(out)
}

/// Mint a DOMRect through the global constructor.
fn mint_rect<'s>(scope: &mut v8::PinScope<'s, '_>, v: [f64; 4]) -> Option<Local<'s, v8::Object>> {
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, "DOMRect").unwrap();
    let c = global.get(scope, k.into())?;
    let c = Local::<v8::Function>::try_from(c).ok()?;
    let argv: Vec<Local<v8::Value>> = v
        .iter()
        .map(|n| v8::Number::new(scope, *n).into())
        .collect();
    c.new_instance(scope, &argv)
}

/// Mint a DOMQuad with the given four point objects already in fields.
fn mint_quad<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    pts: [Local<'s, v8::Object>; 4],
) -> Option<Local<'s, v8::Object>> {
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, "DOMQuad").unwrap();
    let c = global.get(scope, k.into())?;
    let c = Local::<v8::Function>::try_from(c).ok()?;
    let q = c.new_instance(scope, &[])?;
    for (i, p) in pts.iter().enumerate() {
        q.set_internal_field(i + 1, (*p).into());
    }
    Some(q)
}

fn num_prop(scope: &mut v8::PinScope<'_, '_>, obj: Local<v8::Object>, name: &str) -> Option<f64> {
    let k = v8::String::new(scope, name).unwrap();
    let v = obj.get(scope, k.into())?;
    v.number_value(scope)
}

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    if !args.is_construct_call() {
        throw_type(scope, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", "Failed to construct 'DOMQuad'");
        return;
    }
    let ctx = "Failed to construct 'DOMQuad'";
    let tag = v8::Integer::new(scope, FAMILY);
    args.this().set_internal_field(0, tag.into());
    for k in 0..4 {
        let Some(p) = read_point_arg(scope, args.get(k), ctx) else {
            return;
        };
        let Some(pt) = mint_point(scope, p) else {
            return;
        };
        args.this().set_internal_field(k as usize + 1, pt.into());
    }
}

unsafe extern "C" fn get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let idx = args.data().int32_value(scope).unwrap_or(0) as usize;
    if branded(scope, args.this()).is_none() {
        illegal(scope);
        return;
    }
    let Some(p) = point_at(scope, args.this(), idx + 1) else {
        illegal(scope);
        return;
    };
    let mut rv = ReturnValue::from_function_callback_info(info);
    rv.set(p.into());
}

unsafe extern "C" fn method(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let which = args.data().int32_value(scope).unwrap_or(0);
    if branded(scope, args.this()).is_none() {
        illegal(scope);
        return;
    }
    let mut rv = ReturnValue::from_function_callback_info(info);
    match which {
        // getBounds
        0 => {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for i in 1..=4 {
                let Some(p) = point_at(scope, args.this(), i) else {
                    illegal(scope);
                    return;
                };
                let Some(x) = num_prop(scope, p, "x") else {
                    return;
                };
                let Some(y) = num_prop(scope, p, "y") else {
                    return;
                };
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
            let Some(r) = mint_rect(scope, [min_x, min_y, max_x - min_x, max_y - min_y]) else {
                return;
            };
            rv.set(r.into());
        }
        // toJSON: {p1:{w,x,y,z}, ...} in key order p1..p4, point keys w,x,y,z
        _ => {
            let out = v8::Object::new(scope);
            for i in 1..=4 {
                let Some(p) = point_at(scope, args.this(), i) else {
                    illegal(scope);
                    return;
                };
                let pj = v8::Object::new(scope);
                for k in ["w", "x", "y", "z"] {
                    let key = v8::String::new(scope, k).unwrap();
                    let Some(v) = p.get(scope, key.into()) else {
                        return;
                    };
                    pj.set(scope, key.into(), v);
                }
                let pk = v8::String::new(scope, &format!("p{i}")).unwrap();
                out.set(scope, pk.into(), pj.into());
            }
            rv.set(out.into());
        }
    }
}

unsafe extern "C" fn stat(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let which = args.data().int32_value(scope).unwrap_or(0);
    let mut rv = ReturnValue::from_function_callback_info(info);
    match which {
        // fromRect
        0 => {
            let ctx = "Failed to execute 'fromRect' on 'DOMQuad'";
            let Some([x, y, w, h]) = read_rect_init(scope, args.get(0), ctx) else {
                return;
            };
            let mut pts: Vec<Local<v8::Object>> = Vec::new();
            for p in [
                [x, y, 0.0, 1.0],
                [x + w, y, 0.0, 1.0],
                [x + w, y + h, 0.0, 1.0],
                [x, y + h, 0.0, 1.0],
            ] {
                let Some(pt) = mint_point(scope, p) else {
                    return;
                };
                pts.push(pt);
            }
            let Some(q) = mint_quad(scope, [pts[0], pts[1], pts[2], pts[3]]) else {
                return;
            };
            rv.set(q.into());
        }
        // fromQuad: read p1..p4 dict members, each a DOMPointInit
        _ => {
            let ctx = "Failed to execute 'fromQuad' on 'DOMQuad'";
            let init = args.get(0);
            if !init.is_undefined() && !init.is_null() && !init.is_object() && !init.is_function() {
                throw_type(
                    scope,
                    "The provided value is not of type 'DOMQuadInit'.",
                    ctx,
                );
                return;
            }
            let mut pts: Vec<Local<v8::Object>> = Vec::new();
            for k in ["p1", "p2", "p3", "p4"] {
                let member = if let Ok(d) = Local::<v8::Object>::try_from(init) {
                    if init.is_undefined() || init.is_null() {
                        v8::undefined(scope).into()
                    } else {
                        let key = v8::String::new(scope, k).unwrap();
                        let Some(v) = d.get(scope, key.into()) else {
                            return;
                        };
                        v
                    }
                } else {
                    v8::undefined(scope).into()
                };
                let Some(p) = read_point_arg(scope, member, ctx) else {
                    return;
                };
                let Some(pt) = mint_point(scope, p) else {
                    return;
                };
                pts.push(pt);
            }
            let Some(q) = mint_quad(scope, [pts[0], pts[1], pts[2], pts[3]]) else {
                return;
            };
            rv.set(q.into());
        }
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

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    let tag_key = v8::Symbol::get_to_string_tag(scope);
    let t = builder(scope, ctor, 0, 0);
    let nm = v8::String::new(scope, "DOMQuad").unwrap();
    t.set_class_name(nm);
    t.instance_template(scope).set_internal_field_count(NFIELDS);
    t.read_only_prototype();
    let proto = t.prototype_template(scope);
    proto.set_with_attr(
        tag_key.into(),
        nm.into(),
        PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM,
    );
    for (i, pname) in ["p1", "p2", "p3", "p4"].iter().enumerate() {
        let g = builder(scope, get, i as i32, 0);
        let gn = v8::String::new(scope, &format!("get {pname}")).unwrap();
        g.set_class_name(gn);
        let key = v8::String::new(scope, pname).unwrap();
        proto.set_accessor_property(key.into(), Some(g), None, PropertyAttribute::NONE);
    }
    {
        let f = builder(scope, method, 0, 0);
        let key = v8::String::new(scope, "getBounds").unwrap();
        proto.set_with_attr(key.into(), f.into(), PropertyAttribute::NONE);
    }
    {
        let f = builder(scope, method, 1, 0);
        let key = v8::String::new(scope, "toJSON").unwrap();
        proto.set_with_attr(key.into(), f.into(), PropertyAttribute::NONE);
    }
    for (which, fname) in [(0, "fromRect"), (1, "fromQuad")] {
        let f = builder(scope, stat, which, 0);
        let key = v8::String::new(scope, fname).unwrap();
        t.set_with_attr(key.into(), f.into(), PropertyAttribute::NONE);
    }
    if let Some(f) = t.get_function(scope) {
        let key = v8::String::new(scope, "DOMQuad").unwrap();
        global.define_own_property(scope, key.into(), f.into(), PropertyAttribute::DONT_ENUM);
    }
}
