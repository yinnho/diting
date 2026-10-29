//! The declaration layer: descriptors + the generic callbacks that serve
//! them. See the module doc in `mod.rs` for the laws.

use v8::{
    FunctionCallbackArguments, FunctionCallbackInfo, FunctionTemplate, Local, PropertyAttribute,
    ReturnValue,
};

/// One numeric state slot: also the constructor argument and dictionary
/// member of the same name (default applies to `undefined`).
pub(crate) struct Slot {
    pub name: &'static str,
    pub default: f64,
}

pub(crate) enum Attr {
    /// Reads/writes internal field `1 + i`.
    Slot(usize),
    /// Computed from the slot values.
    Derived(fn(&[f64]) -> f64),
}

pub(crate) struct Member {
    pub name: &'static str,
    pub attr: Attr,
    pub writable: bool,
}

pub(crate) struct Iface {
    pub name: &'static str,
    /// Index of the parent interface in the same table.
    pub parent: Option<usize>,
    /// Brand tag; a derived interface shares its root's family.
    pub family: i32,
    pub slots: &'static [Slot],
    /// Attributes declared on THIS interface's prototype.
    pub members: &'static [Member],
    /// toJSON on this prototype (lists the ROOT's members, in order).
    pub to_json: bool,
    /// Static dictionary factory name ("fromRect").
    pub factory: &'static str,
}

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [ctor as Cb, get as Cb, set as Cb, to_json as Cb, factory as Cb]
        .into_iter()
        .map(|f| v8::ExternalReference { function: f })
        .collect()
}

// The interface table is process-static, registered once at install; the
// callbacks resolve `data` against it. (A snapshot restore never runs
// `install`, so the table is also set from `tables()` lazily.)
static TABLE: std::sync::OnceLock<&'static [Iface]> = std::sync::OnceLock::new();

fn table() -> &'static [Iface] {
    TABLE.get().copied().unwrap_or(super::geometry::IFACES)
}

const MEMBER_BITS: i32 = 6;
fn enc(iface: usize, member: usize) -> i32 { ((iface as i32) << MEMBER_BITS) | member as i32 }
fn dec(d: i32) -> (usize, usize) { ((d >> MEMBER_BITS) as usize, (d & ((1 << MEMBER_BITS) - 1)) as usize) }

fn root_of(t: &[Iface], mut i: usize) -> usize {
    while let Some(p) = t[i].parent { i = p; }
    i
}

fn throw_type_error(scope: &mut v8::PinScope<'_, '_>, msg: &str) {
    let m = v8::String::new(scope, msg).unwrap();
    let e = v8::Exception::type_error(scope, m);
    scope.throw_exception(e);
}

/// WebIDL `unrestricted double`; `undefined` -> `default` when given.
/// A Symbol throws with Chrome's "<prefix>: Cannot convert ..." wording.
fn conv_double(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>, default: Option<f64>, prefix: &str) -> Option<f64> {
    if v.is_undefined() {
        if let Some(d) = default { return Some(d); }
    }
    if v.is_symbol() {
        throw_type_error(scope, &format!("{prefix}: Cannot convert a Symbol value to a number"));
        return None;
    }
    v.number_value(scope)
}

/// Brand check: internal-field layout AND family tag must match.
fn read(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, root: &Iface) -> Option<Vec<f64>> {
    if this.internal_field_count() != 1 + root.slots.len() {
        return None;
    }
    let tag: Local<v8::Value> = this.get_internal_field(scope, 0)?.try_into().ok()?;
    if tag.int32_value(scope)? != root.family {
        return None;
    }
    let mut out = Vec::with_capacity(root.slots.len());
    for i in 0..root.slots.len() {
        let f: Local<v8::Value> = this.get_internal_field(scope, i + 1)?.try_into().ok()?;
        out.push(f.number_value(scope)?);
    }
    Some(out)
}

fn write(scope: &mut v8::PinScope<'_, '_>, obj: Local<v8::Object>, family: i32, v: &[f64]) {
    let tag = v8::Integer::new(scope, family);
    obj.set_internal_field(0, tag.into());
    for (i, n) in v.iter().enumerate() {
        let num = v8::Number::new(scope, *n);
        obj.set_internal_field(i + 1, num.into());
    }
}

fn member_value(root: &Iface, m: &Member, v: &[f64]) -> f64 {
    let _ = root;
    match m.attr { Attr::Slot(i) => v[i], Attr::Derived(f) => f(v) }
}

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let t = table();
    let (ii, _) = dec(args.data().int32_value(scope).unwrap_or(0));
    let it = &t[ii];
    if !args.is_construct_call() {
        throw_type_error(scope, &format!("Failed to construct '{}': Please use the 'new' operator, this DOM object constructor cannot be called as a function.", it.name));
        return;
    }
    let prefix = format!("Failed to construct '{}'", it.name);
    let mut v = Vec::with_capacity(it.slots.len());
    for (i, s) in it.slots.iter().enumerate() {
        match conv_double(scope, args.get(i as i32), Some(s.default), &prefix) {
            Some(n) => v.push(n),
            None => return, // exception already pending
        }
    }
    write(scope, args.this(), it.family, &v);
}

unsafe extern "C" fn get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let t = table();
    let (ii, mi) = dec(args.data().int32_value(scope).unwrap_or(0));
    let root = &t[root_of(t, ii)];
    match read(scope, args.this(), root) {
        Some(v) => {
            let m = &t[ii].members[mi];
            let mut rv = ReturnValue::from_function_callback_info(info);
            rv.set(v8::Number::new(scope, member_value(root, m, &v)).into());
        }
        None => throw_type_error(scope, "Illegal invocation"),
    }
}

unsafe extern "C" fn set(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let t = table();
    let (ii, mi) = dec(args.data().int32_value(scope).unwrap_or(0));
    let root = &t[root_of(t, ii)];
    let Some(mut v) = read(scope, args.this(), root) else {
        throw_type_error(scope, "Illegal invocation");
        return;
    };
    let m = &t[ii].members[mi];
    let Attr::Slot(slot) = m.attr else { return };
    let prefix = format!("Failed to set the '{}' property on '{}'", m.name, t[ii].name);
    let Some(n) = conv_double(scope, args.get(0), None, &prefix) else { return };
    v[slot] = n;
    write(scope, args.this(), root.family, &v);
}

unsafe extern "C" fn to_json(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let t = table();
    let (ii, _) = dec(args.data().int32_value(scope).unwrap_or(0));
    let root = &t[root_of(t, ii)];
    let Some(v) = read(scope, args.this(), root) else {
        throw_type_error(scope, "Illegal invocation");
        return;
    };
    let obj = v8::Object::new(scope);
    for m in root.members {
        let k = v8::String::new(scope, m.name).unwrap();
        let n = v8::Number::new(scope, member_value(root, m, &v));
        obj.set(scope, k.into(), n.into());
    }
    ReturnValue::from_function_callback_info(info).set(obj.into());
}

/// static factory(other = {}) - the dictionary is read once, members in
/// LEXICOGRAPHIC order (Web IDL), each converted exactly once.
unsafe extern "C" fn factory(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let t = table();
    let (ii, _) = dec(args.data().int32_value(scope).unwrap_or(0));
    let it = &t[ii];
    let prefix = format!("Failed to execute '{}' on '{}'", it.factory, it.name);
    let mut v: Vec<f64> = it.slots.iter().map(|s| s.default).collect();
    let dict = args.get(0);
    if dict.is_undefined() || dict.is_null() {
        // dictionary default
    } else if let Ok(d) = Local::<v8::Object>::try_from(dict) {
        let mut order: Vec<usize> = (0..it.slots.len()).collect();
        order.sort_by_key(|&i| it.slots[i].name);
        for i in order {
            let k = v8::String::new(scope, it.slots[i].name).unwrap();
            let Some(got) = d.get(scope, k.into()) else { return };
            let Some(n) = conv_double(scope, got, Some(it.slots[i].default), &prefix) else { return };
            v[i] = n;
        }
    } else {
        let init = if it.name.starts_with("DOMRect") { "DOMRectInit" } else { "DOMPointInit" };
        throw_type_error(scope, &format!("{prefix}: The provided value is not of type '{init}'."));
        return;
    }
    // Instantiate through the interface's own constructor so the result
    // carries the right prototype.
    let global = scope.get_current_context().global(scope);
    let k = v8::String::new(scope, it.name).unwrap();
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

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>, ifaces: &'static [Iface]) {
    let _ = TABLE.set(ifaces);
    let tag_key = v8::Symbol::get_to_string_tag(scope);
    let mut templates: Vec<Local<FunctionTemplate>> = Vec::new();
    for (ii, it) in ifaces.iter().enumerate() {
        let t = builder(scope, ctor, enc(ii, 0), 0);
        let nm = name_of(scope, it.name);
        t.set_class_name(nm);
        if let Some(p) = it.parent { t.inherit(templates[p]); }
        t.instance_template(scope).set_internal_field_count(1 + it.slots.len());
        t.read_only_prototype();
        let proto = t.prototype_template(scope);
        proto.set_with_attr(tag_key.into(), nm.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM);
        for (mi, m) in it.members.iter().enumerate() {
            let g = builder(scope, get, enc(ii, mi), 0);
            let gname = name_of(scope, &format!("get {}", m.name));
            g.set_class_name(gname);
            let s = if m.writable {
                let s = builder(scope, set, enc(ii, mi), 1);
                let sname = name_of(scope, &format!("set {}", m.name));
                s.set_class_name(sname);
                Some(s)
            } else { None };
            let key = name_of(scope, m.name);
            proto.set_accessor_property(key.into(), Some(g), s, PropertyAttribute::NONE);
        }
        if it.to_json {
            let tj = builder(scope, to_json, enc(ii, 0), 0);
            let n = name_of(scope, "toJSON");
            tj.set_class_name(n);
            proto.set_with_attr(n.into(), tj.into(), PropertyAttribute::NONE);
        }
        let f = builder(scope, factory, enc(ii, 0), 0);
        let fname = name_of(scope, it.factory);
        f.set_class_name(fname);
        t.set_with_attr(fname.into(), f.into(), PropertyAttribute::NONE);
        templates.push(t);
    }
    let mut fns: Vec<Local<v8::Function>> = Vec::new();
    for (ii, it) in ifaces.iter().enumerate() {
        let f = templates[ii].get_function(scope).expect("template instantiates");
        // Interface objects chain like their prototypes (templates only
        // chain the prototype objects).
        if let Some(p) = it.parent { f.set_prototype(scope, fns[p].into()); }
        fns.push(f);
    }
    for (it, f) in ifaces.iter().zip(fns) {
        let k = name_of(scope, it.name);
        global.define_own_property(scope, k.into(), f.into(), PropertyAttribute::DONT_ENUM);
    }
}
