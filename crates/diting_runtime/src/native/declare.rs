//! The declaration layer: descriptors + the generic callbacks that serve
//! them. See the module doc in `mod.rs` for the laws.

use v8::{
    FunctionCallbackArguments, FunctionCallbackInfo, FunctionTemplate, Local, PropertyAttribute,
    ReturnValue,
};

/// Default of a slot; its variant also fixes the slot's WebIDL type
/// (`Num` = unrestricted double, `Str` = DOMString).
#[derive(Clone, Copy)]
pub(crate) enum Def {
    Num(f64),
    Str(&'static str),
}

/// A slot value as read from / written to an internal field.
#[derive(Clone, Debug)]
pub(crate) enum Val {
    Num(f64),
    Str(String),
}

impl Val {
    pub(crate) fn num(&self) -> f64 {
        match self { Val::Num(n) => *n, Val::Str(_) => f64::NAN }
    }
    pub(crate) fn str(&self) -> &str {
        match self { Val::Str(s) => s, Val::Num(_) => "" }
    }
}

/// One state slot: also the constructor argument and dictionary member of
/// the same name (default applies to `undefined`).
pub(crate) struct Slot {
    pub name: &'static str,
    pub default: Def,
}

pub(crate) enum Attr {
    /// Reads/writes internal field `1 + i`.
    Slot(usize),
    /// Computed from the slot values.
    Derived(fn(&[Val]) -> Val),
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
    /// Static dictionary factory name ("fromRect"), if the interface has one.
    pub factory: Option<&'static str>,
    /// Legacy numeric constants, defined on the constructor AND the
    /// prototype ({w:false, e:true, c:false}).
    pub consts: &'static [(&'static str, i32)],
    /// Prototype chains to the host's `Error.prototype` (DOMException).
    pub error_like: bool,
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
pub(crate) type Table = &'static [&'static Iface];

fn table() -> Table {
    super::IFACES
}

const MEMBER_BITS: i32 = 6;
fn enc(iface: usize, member: usize) -> i32 { ((iface as i32) << MEMBER_BITS) | member as i32 }
fn dec(d: i32) -> (usize, usize) { ((d >> MEMBER_BITS) as usize, (d & ((1 << MEMBER_BITS) - 1)) as usize) }

fn root_of(t: Table, mut i: usize) -> usize {
    while let Some(p) = t[i].parent { i = p; }
    i
}

fn throw_type_error(scope: &mut v8::PinScope<'_, '_>, msg: &str) {
    let m = v8::String::new(scope, msg).unwrap();
    let e = v8::Exception::type_error(scope, m);
    scope.throw_exception(e);
}

/// WebIDL conversion of one argument to the slot's type; `undefined` ->
/// `default` when given. A Symbol throws with Chrome's
/// "<prefix>: Cannot convert ..." wording.
fn conv(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>, def: Def, use_default: bool, prefix: &str) -> Option<Val> {
    if v.is_undefined() && use_default {
        return Some(match def { Def::Num(n) => Val::Num(n), Def::Str(s) => Val::Str(s.to_string()) });
    }
    match def {
        Def::Num(_) => {
            if v.is_symbol() {
                throw_type_error(scope, &format!("{prefix}: Cannot convert a Symbol value to a number"));
                return None;
            }
            v.number_value(scope).map(Val::Num)
        }
        Def::Str(_) => {
            if v.is_symbol() {
                throw_type_error(scope, &format!("{prefix}: Cannot convert a Symbol value to a string"));
                return None;
            }
            let s = v.to_string(scope)?;
            Some(Val::Str(s.to_rust_string_lossy(scope)))
        }
    }
}

/// Brand check: internal-field layout AND family tag must match.
fn read(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, root: &Iface) -> Option<Vec<Val>> {
    if this.internal_field_count() != 1 + root.slots.len() {
        return None;
    }
    let tag: Local<v8::Value> = this.get_internal_field(scope, 0)?.try_into().ok()?;
    if tag.int32_value(scope)? != root.family {
        return None;
    }
    let mut out = Vec::with_capacity(root.slots.len());
    for (i, s) in root.slots.iter().enumerate() {
        let f: Local<v8::Value> = this.get_internal_field(scope, i + 1)?.try_into().ok()?;
        out.push(match s.default {
            Def::Num(_) => Val::Num(f.number_value(scope)?),
            Def::Str(_) => Val::Str(f.to_string(scope)?.to_rust_string_lossy(scope)),
        });
    }
    Some(out)
}

fn to_js<'s>(scope: &mut v8::PinScope<'s, '_>, v: &Val) -> Local<'s, v8::Value> {
    match v {
        Val::Num(n) => v8::Number::new(scope, *n).into(),
        Val::Str(s) => v8::String::new(scope, s).unwrap().into(),
    }
}

fn write(scope: &mut v8::PinScope<'_, '_>, obj: Local<v8::Object>, family: i32, v: &[Val]) {
    let tag = v8::Integer::new(scope, family);
    obj.set_internal_field(0, tag.into());
    for (i, val) in v.iter().enumerate() {
        let js = to_js(scope, val);
        obj.set_internal_field(i + 1, js.into());
    }
}

fn member_value(m: &Member, v: &[Val]) -> Val {
    match m.attr { Attr::Slot(i) => v[i].clone(), Attr::Derived(f) => f(v) }
}

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let t = table();
    let (ii, _) = dec(args.data().int32_value(scope).unwrap_or(0));
    let it = t[ii];
    if !args.is_construct_call() {
        throw_type_error(scope, &format!("Failed to construct '{}': Please use the 'new' operator, this DOM object constructor cannot be called as a function.", it.name));
        return;
    }
    let prefix = format!("Failed to construct '{}'", it.name);
    let mut v = Vec::with_capacity(it.slots.len());
    for (i, s) in it.slots.iter().enumerate() {
        match conv(scope, args.get(i as i32), s.default, true, &prefix) {
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
    let root = t[root_of(t, ii)];
    match read(scope, args.this(), root) {
        Some(v) => {
            let m = &t[ii].members[mi];
            let mut rv = ReturnValue::from_function_callback_info(info);
            let out = to_js(scope, &member_value(m, &v));
            rv.set(out);
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
    let root = t[root_of(t, ii)];
    let Some(mut v) = read(scope, args.this(), root) else {
        throw_type_error(scope, "Illegal invocation");
        return;
    };
    let m = &t[ii].members[mi];
    let Attr::Slot(slot) = m.attr else { return };
    let prefix = format!("Failed to set the '{}' property on '{}'", m.name, t[ii].name);
    let Some(n) = conv(scope, args.get(0), root.slots[slot].default, false, &prefix) else { return };
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
    let root = t[root_of(t, ii)];
    let Some(v) = read(scope, args.this(), root) else {
        throw_type_error(scope, "Illegal invocation");
        return;
    };
    let obj = v8::Object::new(scope);
    for m in root.members {
        let k = v8::String::new(scope, m.name).unwrap();
        let n = to_js(scope, &member_value(m, &v));
        obj.set(scope, k.into(), n);
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
    let it = t[ii];
    let Some(fname) = it.factory else { return };
    let prefix = format!("Failed to execute '{}' on '{}'", fname, it.name);
    let mut v: Vec<Val> = it.slots.iter().map(|s| match s.default { Def::Num(n) => Val::Num(n), Def::Str(t) => Val::Str(t.to_string()) }).collect();
    let dict = args.get(0);
    if dict.is_undefined() || dict.is_null() {
        // dictionary default
    } else if let Ok(d) = Local::<v8::Object>::try_from(dict) {
        let mut order: Vec<usize> = (0..it.slots.len()).collect();
        order.sort_by_key(|&i| it.slots[i].name);
        for i in order {
            let k = v8::String::new(scope, it.slots[i].name).unwrap();
            let Some(got) = d.get(scope, k.into()) else { return };
            let Some(n) = conv(scope, got, it.slots[i].default, true, &prefix) else { return };
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
    let argv: Vec<Local<v8::Value>> = v.iter().map(|n| to_js(scope, n)).collect();
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

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>, ifaces: Table) {
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
        if let Some(fn_name) = it.factory {
            let f = builder(scope, factory, enc(ii, 0), 0);
            let fname = name_of(scope, fn_name);
            f.set_class_name(fname);
            t.set_with_attr(fname.into(), f.into(), PropertyAttribute::NONE);
        }
        for (cname, cval) in it.consts {
            let k = name_of(scope, cname);
            let v = v8::Integer::new(scope, *cval);
            t.set_with_attr(k.into(), v.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_DELETE);
            proto.set_with_attr(k.into(), v.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_DELETE);
        }
        templates.push(t);
    }
    let mut fns: Vec<Local<v8::Function>> = Vec::new();
    for (ii, it) in ifaces.iter().enumerate() {
        let f = templates[ii].get_function(scope).expect("template instantiates");
        // Interface objects chain like their prototypes (templates only
        // chain the prototype objects).
        if let Some(p) = it.parent { f.set_prototype(scope, fns[p].into()); }
        if it.error_like {
            // Web IDL: an exception interface's prototype chains to
            // Error.prototype, so `e instanceof Error` and Error's toString.
            let ek = name_of(scope, "Error");
            let pk = name_of(scope, "prototype");
            let err = global.get(scope, ek.into()).and_then(|e| Local::<v8::Object>::try_from(e).ok());
            let proto = f.get(scope, pk.into()).and_then(|p| Local::<v8::Object>::try_from(p).ok());
            if let (Some(err), Some(proto)) = (err, proto) {
                if let Some(ep) = err.get(scope, pk.into()) { proto.set_prototype(scope, ep); }
            }
        }
        fns.push(f);
    }
    for (it, f) in ifaces.iter().zip(fns) {
        let k = name_of(scope, it.name);
        global.define_own_property(scope, k.into(), f.into(), PropertyAttribute::DONT_ENUM);
    }
}
