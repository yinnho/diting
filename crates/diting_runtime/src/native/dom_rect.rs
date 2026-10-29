//! DOMRectReadOnly / DOMRect as native interfaces (geometry spec).
//! Shape contract is pinned against Chrome by tools/webplatform/chrome/
//! native-pilot-probe.js (the same probe runs against both).

use v8::{
    FunctionCallbackArguments, FunctionCallbackInfo, FunctionTemplate, Local, PropertyAttribute,
    ReturnValue,
};

/// Interface id stored in internal field 0 (the brand).
const TAG_RECT: i32 = 0x0D01;
/// Field layout: 0 tag, 1 x, 2 y, 3 width, 4 height.
const FIELDS: usize = 5;

// callback `data` selectors
const D_CTOR_RO: i32 = 1;
const D_CTOR_RECT: i32 = 2;

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [ctor as Cb, get as Cb, set as Cb, to_json as Cb, from_rect as Cb]
        .into_iter()
        .map(|f| v8::ExternalReference { function: f })
        .collect()
}

fn throw_type_error(scope: &mut v8::PinScope<'_, '_>, msg: &str) {
    let m = v8::String::new(scope, msg).unwrap();
    let e = v8::Exception::type_error(scope, m);
    scope.throw_exception(e);
}

/// WebIDL `unrestricted double`; `undefined` -> `default` when given
/// (dictionary / optional-argument members), else ToNumber (NaN).
/// A Symbol throws with Chrome's "<prefix>: Cannot convert ..." wording.
fn conv_double(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>, default: Option<f64>, prefix: &str) -> Option<f64> {
    if v.is_undefined() {
        if let Some(d) = default {
            return Some(d);
        }
    }
    if v.is_symbol() {
        throw_type_error(scope, &format!("{prefix}: Cannot convert a Symbol value to a number"));
        return None;
    }
    v.number_value(scope)
}

fn read(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>) -> Option<[f64; 4]> {
    if this.internal_field_count() != FIELDS {
        return None;
    }
    let tag = this.get_internal_field(scope, 0)?;
    let tag: Local<v8::Value> = tag.try_into().ok()?;
    if tag.int32_value(scope)? != TAG_RECT {
        return None;
    }
    let mut out = [0.0; 4];
    for (i, o) in out.iter_mut().enumerate() {
        let f: Local<v8::Value> = this.get_internal_field(scope, i + 1)?.try_into().ok()?;
        *o = f.number_value(scope)?;
    }
    Some(out)
}

fn write(scope: &mut v8::PinScope<'_, '_>, obj: Local<v8::Object>, v: [f64; 4]) {
    let tag = v8::Integer::new(scope, TAG_RECT);
    obj.set_internal_field(0, tag.into());
    for (i, n) in v.iter().enumerate() {
        let num = v8::Number::new(scope, *n);
        obj.set_internal_field(i + 1, num.into());
    }
}

// NaN-propagating min/max (spec: "min(y, y+height)" on doubles).
fn fmin(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) } }
fn fmax(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) } }

fn member(v: [f64; 4], idx: i32) -> f64 {
    let [x, y, w, h] = v;
    match idx {
        0 => x,
        1 => y,
        2 => w,
        3 => h,
        4 => fmin(y, y + h), // top
        5 => fmax(x, x + w), // right
        6 => fmax(y, y + h), // bottom
        _ => fmin(x, x + w), // left
    }
}

const MEMBERS: [&str; 8] = ["x", "y", "width", "height", "top", "right", "bottom", "left"];

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let name = if args.data().int32_value(scope).unwrap_or(0) == D_CTOR_RECT { "DOMRect" } else { "DOMRectReadOnly" };
    if !args.is_construct_call() {
        throw_type_error(scope, &format!("Failed to construct '{name}': Please use the 'new' operator, this DOM object constructor cannot be called as a function."));
        return;
    }
    let mut v = [0.0; 4];
    for (i, slot) in v.iter_mut().enumerate() {
        match conv_double(scope, args.get(i as i32), Some(0.0), &format!("Failed to construct '{name}'")) {
            Some(n) => *slot = n,
            None => return, // exception already pending
        }
    }
    write(scope, args.this(), v);
}

unsafe extern "C" fn get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let idx = args.data().int32_value(scope).unwrap_or(0);
    match read(scope, args.this()) {
        Some(v) => {
            let mut rv = ReturnValue::from_function_callback_info(info);
            rv.set(v8::Number::new(scope, member(v, idx)).into());
        }
        None => throw_type_error(scope, "Illegal invocation"),
    }
}

unsafe extern "C" fn set(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let idx = args.data().int32_value(scope).unwrap_or(0) as usize;
    let Some(mut v) = read(scope, args.this()) else {
        throw_type_error(scope, "Illegal invocation");
        return;
    };
    let prefix = format!("Failed to set the '{}' property on 'DOMRect'", MEMBERS[idx]);
    let Some(n) = conv_double(scope, args.get(0), None, &prefix) else { return };
    v[idx] = n;
    write(scope, args.this(), v);
}

unsafe extern "C" fn to_json(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let Some(v) = read(scope, args.this()) else {
        throw_type_error(scope, "Illegal invocation");
        return;
    };
    let obj = v8::Object::new(scope);
    for (i, name) in MEMBERS.iter().enumerate() {
        let k = v8::String::new(scope, name).unwrap();
        let n = v8::Number::new(scope, member(v, i as i32));
        obj.set(scope, k.into(), n.into());
    }
    ReturnValue::from_function_callback_info(info).set(obj.into());
}

/// static fromRect(other = {}) - the dictionary is read once, in
/// declaration order (x, y, width, height), each member converted once.
unsafe extern "C" fn from_rect(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let which = args.data().int32_value(scope).unwrap_or(D_CTOR_RO);
    let ctor_name = if which == D_CTOR_RECT { "DOMRect" } else { "DOMRectReadOnly" };
    let prefix = format!("Failed to execute 'fromRect' on '{ctor_name}'");
    let mut v = [0.0; 4];
    let dict = args.get(0);
    if dict.is_undefined() || dict.is_null() {
        // dictionary default: all members 0
    } else if let Ok(d) = Local::<v8::Object>::try_from(dict) {
        // Web IDL: dictionary members are read in LEXICOGRAPHIC order
        // (height, width, x, y), each exactly once.
        for name in ["height", "width", "x", "y"] {
            let k = v8::String::new(scope, name).unwrap();
            let Some(got) = d.get(scope, k.into()) else { return };
            let Some(n) = conv_double(scope, got, Some(0.0), &prefix) else { return };
            let slot = MEMBERS[..4].iter().position(|m| *m == name).unwrap();
            v[slot] = n;
        }
    } else {
        throw_type_error(scope, &format!("{prefix}: The provided value is not of type 'DOMRectInit'."));
        return;
    }
    // Instantiate through the interface's own constructor so the result
    // carries the right prototype.
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, ctor_name).unwrap();
    let Some(c) = global.get(scope, k.into()) else { return };
    let Ok(c) = Local::<v8::Function>::try_from(c) else { return };
    let argv: Vec<Local<v8::Value>> = v.iter().map(|n| v8::Number::new(scope, *n).into()).collect();
    if let Some(o) = c.new_instance(scope, &argv) {
        ReturnValue::from_function_callback_info(info).set(o.into());
    }
}

fn builder<'s>(scope: &mut v8::PinScope<'s, '_, ()>, cb: Cb, data: i32, len: i32) -> Local<'s, FunctionTemplate> {
    let d = v8::Integer::new(scope, data);
    FunctionTemplate::builder_raw(cb).data(d.into()).length(len).build(scope)
}

fn name_of<'s>(scope: &mut v8::PinScope<'s, '_, ()>, s: &str) -> Local<'s, v8::String> {
    v8::String::new(scope, s).unwrap()
}

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    // DOMRectReadOnly
    let ro = builder(scope, ctor, D_CTOR_RO, 0);
    let ro_name = name_of(scope, "DOMRectReadOnly");
    ro.set_class_name(ro_name);
    ro.instance_template(scope).set_internal_field_count(FIELDS);
    ro.read_only_prototype();
    let ro_proto = ro.prototype_template(scope);
    let tag_key = v8::Symbol::get_to_string_tag(scope);
    let ro_tag = name_of(scope, "DOMRectReadOnly");
    ro_proto.set_with_attr(tag_key.into(), ro_tag.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM);
    for (i, name) in MEMBERS.iter().enumerate() {
        let g = builder(scope, get, i as i32, 0);
        let gname = name_of(scope, &format!("get {name}"));
        g.set_class_name(gname);
        let key = name_of(scope, name);
        ro_proto.set_accessor_property(key.into(), Some(g), None, PropertyAttribute::NONE);
    }
    let tj = builder(scope, to_json, 0, 0);
    let tj_name = name_of(scope, "toJSON");
    tj.set_class_name(tj_name);
    ro_proto.set_with_attr(tj_name.into(), tj.into(), PropertyAttribute::NONE);
    let fr = builder(scope, from_rect, D_CTOR_RO, 0);
    let fr_name = name_of(scope, "fromRect");
    fr.set_class_name(fr_name);
    ro.set_with_attr(fr_name.into(), fr.into(), PropertyAttribute::NONE);

    // DOMRect extends DOMRectReadOnly: x/y/width/height gain setters.
    let rect = builder(scope, ctor, D_CTOR_RECT, 0);
    let rect_name = name_of(scope, "DOMRect");
    rect.set_class_name(rect_name);
    rect.inherit(ro);
    rect.instance_template(scope).set_internal_field_count(FIELDS);
    rect.read_only_prototype();
    let rect_proto = rect.prototype_template(scope);
    rect_proto.set_with_attr(tag_key.into(), rect_name.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM);
    for (i, name) in MEMBERS[..4].iter().enumerate() {
        let g = builder(scope, get, i as i32, 0);
        let gname = name_of(scope, &format!("get {name}"));
        g.set_class_name(gname);
        let s = builder(scope, set, i as i32, 1);
        let sname = name_of(scope, &format!("set {name}"));
        s.set_class_name(sname);
        let key = name_of(scope, name);
        rect_proto.set_accessor_property(key.into(), Some(g), Some(s), PropertyAttribute::NONE);
    }
    let fr2 = builder(scope, from_rect, D_CTOR_RECT, 0);
    fr2.set_class_name(fr_name);
    rect.set_with_attr(fr_name.into(), fr2.into(), PropertyAttribute::NONE);

    let ro_fn = ro.get_function(scope).expect("template instantiates");
    let rect_fn = rect.get_function(scope).expect("template instantiates");
    // Interface objects chain like their prototypes: DOMRect.__proto__ is
    // DOMRectReadOnly (templates only chain the prototype objects).
    rect_fn.set_prototype(scope, ro_fn.into());
    for (name, f) in [("DOMRectReadOnly", ro_fn), ("DOMRect", rect_fn)] {
        let k = name_of(scope, name);
        global.define_own_property(scope, k.into(), f.into(), PropertyAttribute::DONT_ENUM);
    }
}
