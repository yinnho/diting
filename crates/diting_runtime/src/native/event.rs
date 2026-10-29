//! Event / CustomEvent as native interfaces (M3 step C). Same laws as the
//! module doc in `mod.rs`, with one storage difference: the state lives in
//! a V8 *private slot* holding a fixed-layout array, not in internal
//! fields - because `class T extends Event` allocates a plain JS object
//! for `this` (no template reserved its fields), and the base constructor
//! must be able to brand THAT too. Chrome's answer to the same subclass
//! question is the C++ wrapper; a private slot is our wrapper.
//!
//! Layout of the state array (see the `S_*` indices):
//!   type, target, currentTarget, eventPhase, bubbles, cancelable,
//!   composed, defaultPrevented, stopped, immediateStopped, inPassive,
//!   timeStamp, isTrusted, dispatched, detail, path, pathDone
//!
//! The dispatch flag (`S_DISPATCHED`) is the DOM spec's one flag: it makes
//! re-entrant dispatchEvent throw InvalidStateError (event_target.rs) and
//! initEvent inert mid-dispatch.
//!
//! isTrusted is [LegacyUnforgeable] (Chrome truth 145): an OWN accessor
//! `{e:true, c:false, getter-only}` installed per instance - the single
//! own property any event object has (`Object.getOwnPropertyNames(new
//! Event('x'))` is exactly `["isTrusted"]`).

use v8::{FunctionCallbackArguments, FunctionCallbackInfo, FunctionTemplate, Local, PropertyAttribute, ReturnValue};

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [ctor as Cb, getter as Cb, setter as Cb, method as Cb, it_get as Cb, state as Cb]
        .into_iter()
        .map(|f| v8::ExternalReference { function: f })
        .collect()
}

// ---- state layout --------------------------------------------------------

pub(crate) const S_TYPE: u32 = 0;
pub(crate) const S_TARGET: u32 = 1;
pub(crate) const S_CURRENT: u32 = 2;
pub(crate) const S_PHASE: u32 = 3;
pub(crate) const S_BUBBLES: u32 = 4;
pub(crate) const S_CANCELABLE: u32 = 5;
pub(crate) const S_COMPOSED: u32 = 6;
pub(crate) const S_PREVENTED: u32 = 7;
pub(crate) const S_STOPPED: u32 = 8;
pub(crate) const S_IMMEDIATE: u32 = 9;
pub(crate) const S_PASSIVE: u32 = 10;
pub(crate) const S_STAMP: u32 = 11;
pub(crate) const S_TRUSTED: u32 = 12;
pub(crate) const S_DISPATCHED: u32 = 13;
pub(crate) const S_DETAIL: u32 = 14;
pub(crate) const S_PATH: u32 = 15;
pub(crate) const S_PATH_DONE: u32 = 16;
const S_LEN: u32 = 17;

/// The private-slot key under which an object carries its event state.
/// Presence of the slot IS the Event brand (unforgeable: page scripts can
/// neither create for_api symbols nor see them).
pub(crate) fn state_key<'a>(scope: &mut v8::PinScope<'a, '_>) -> Local<'a, v8::Private> {
    let n = s(scope, "diting#ev-state");
    v8::Private::for_api(scope, Some(n))
}

/// An object's event state array, or None when the object is not branded.
pub(crate) fn state_of<'a>(scope: &mut v8::PinScope<'a, '_>, obj: Local<'a, v8::Object>) -> Option<Local<'a, v8::Array>> {
    let key = state_key(scope);
    obj.get_private(scope, key).and_then(|v| Local::<v8::Array>::try_from(v).ok())
}

fn s<'a>(scope: &mut v8::PinScope<'a, '_>, text: &str) -> Local<'a, v8::String> {
    v8::String::new(scope, text).unwrap()
}

fn private<'a>(scope: &mut v8::PinScope<'a, '_>, name: &str) -> Local<'a, v8::Private> {
    let n = s(scope, name);
    v8::Private::for_api(scope, Some(n))
}

fn throw_type(scope: &mut v8::PinScope<'_, '_>, msg: &str) {
    let m = s(scope, msg);
    let e = v8::Exception::type_error(scope, m);
    scope.throw_exception(e);
}

fn idx<'a>(scope: &mut v8::PinScope<'a, '_>, st: Local<'a, v8::Array>, i: u32) -> Local<'a, v8::Value> {
    match st.get_index(scope, i) { Some(v) => v, None => v8::undefined(scope).into() }
}

fn put<'a>(scope: &mut v8::PinScope<'a, '_>, st: Local<'a, v8::Array>, i: u32, v: Local<'a, v8::Value>) {
    st.set_index(scope, i, v);
}

fn flag<'a>(scope: &mut v8::PinScope<'a, '_>, st: Local<'a, v8::Array>, i: u32) -> bool {
    idx(scope, st, i).is_true()
}

fn b<'a>(scope: &mut v8::PinScope<'a, '_>, x: bool) -> Local<'a, v8::Value> {
    v8::Boolean::new(scope, x).into()
}

// ---- interface table -----------------------------------------------------

struct Member {
    name: &'static str,
    /// The prototype carries a setter too (returnValue, cancelBubble).
    set: bool,
}

struct Method { name: &'static str, len: i32 }

struct EventIface {
    name: &'static str,
    /// Init-dictionary type name in conversion errors ("EventInit").
    init_name: &'static str,
    /// Index of the parent in the same table.
    parent: Option<usize>,
    /// Legacy constants on the constructor AND prototype.
    consts: &'static [(&'static str, i32)],
    members: &'static [Member],
    methods: &'static [Method],
}

static EVENT: EventIface = EventIface {
    name: "Event",
    init_name: "EventInit",
    parent: None,
    consts: &[("NONE", 0), ("CAPTURING_PHASE", 1), ("AT_TARGET", 2), ("BUBBLING_PHASE", 3)],
    // Chrome order (shape.Event): attributes, constants, methods.
    members: &[
        Member { name: "type", set: false },
        Member { name: "target", set: false },
        Member { name: "currentTarget", set: false },
        Member { name: "eventPhase", set: false },
        Member { name: "bubbles", set: false },
        Member { name: "cancelable", set: false },
        Member { name: "defaultPrevented", set: false },
        Member { name: "composed", set: false },
        Member { name: "timeStamp", set: false },
        Member { name: "srcElement", set: false },
        Member { name: "returnValue", set: true },
        Member { name: "cancelBubble", set: true },
    ],
    methods: &[
        Method { name: "composedPath", len: 0 },
        Method { name: "initEvent", len: 1 },
        Method { name: "preventDefault", len: 0 },
        Method { name: "stopImmediatePropagation", len: 0 },
        Method { name: "stopPropagation", len: 0 },
    ],
};

static CUSTOM_EVENT: EventIface = EventIface {
    name: "CustomEvent",
    init_name: "CustomEventInit",
    parent: Some(0),
    consts: &[],
    members: &[Member { name: "detail", set: false }],
    methods: &[Method { name: "initCustomEvent", len: 1 }],
};

static TABLE: &[&EventIface] = &[&EVENT, &CUSTOM_EVENT];

const MEMBER_BITS: i32 = 6;
fn enc(iface: usize, member: usize) -> i32 { ((iface as i32) << MEMBER_BITS) | member as i32 }
fn dec(d: i32) -> (usize, usize) { ((d >> MEMBER_BITS) as usize, (d & ((1 << MEMBER_BITS) - 1)) as usize) }

// ---- constructor ---------------------------------------------------------

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    // The ctor's data word is the bare table index (install), not an
    // enc(iface, member) word like the accessors/methods.
    let ii = args.data().int32_value(scope).unwrap_or(0) as usize;
    let it = TABLE[ii];
    if !args.is_construct_call() {
        throw_type(scope, &format!("Failed to construct '{}': Please use the 'new' operator, this DOM object constructor cannot be called as a function.", it.name));
        return;
    }
    if args.length() < 1 {
        throw_type(scope, &format!("Failed to construct '{}': 1 argument required, but only {} present.", it.name, args.length()));
        return;
    }
    let a0 = args.get(0);
    if a0.is_symbol() {
        throw_type(scope, &format!("Failed to construct '{}': Cannot convert a Symbol value to a string", it.name));
        return;
    }
    let Some(ty) = a0.to_string(scope) else { return };
    // EventInit dictionary: members are read once, in lexicographic order
    // (bubbles, cancelable, composed[, detail]); null/undefined = defaults;
    // a non-object throws with the dictionary's type name.
    let mut bubbles = false;
    let mut cancelable = false;
    let mut composed = false;
    let mut detail: Local<v8::Value> = v8::null(scope).into();
    let d = args.get(1);
    if !(d.is_undefined() || d.is_null()) {
        if let Ok(dict) = Local::<v8::Object>::try_from(d) {
            let kb = s(scope, "bubbles");
            bubbles = dict.get(scope, kb.into()).map(|v| v.boolean_value(scope)).unwrap_or(false);
            let kc = s(scope, "cancelable");
            cancelable = dict.get(scope, kc.into()).map(|v| v.boolean_value(scope)).unwrap_or(false);
            let kp = s(scope, "composed");
            composed = dict.get(scope, kp.into()).map(|v| v.boolean_value(scope)).unwrap_or(false);
            if it.parent.is_some() {
                let kd = s(scope, "detail");
                // `any detail = null`: undefined reads back as null.
                let dv = match dict.get(scope, kd.into()) { Some(v) => v, None => v8::undefined(scope).into() };
                if !dv.is_undefined() { detail = dv; }
            }
        } else {
            throw_type(scope, &format!("Failed to construct '{}': The provided value is not of type '{}'.", it.name, it.init_name));
            return;
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0);
    let null: Local<v8::Value> = v8::null(scope).into();
    let zero: Local<v8::Value> = v8::Integer::new(scope, 0).into();
    let stamp: Local<v8::Value> = v8::Number::new(scope, now).into();
    let f_false: Local<v8::Value> = b(scope, false);
    let fb: Local<v8::Value> = b(scope, bubbles);
    let fc: Local<v8::Value> = b(scope, cancelable);
    let fp: Local<v8::Value> = b(scope, composed);
    let empty_path: Local<v8::Value> = v8::Array::new(scope, 0).into();
    let st = v8::Array::new(scope, S_LEN as i32);
    put(scope, st, S_TYPE, ty.into());
    put(scope, st, S_TARGET, null);
    put(scope, st, S_CURRENT, null);
    put(scope, st, S_PHASE, zero);
    put(scope, st, S_BUBBLES, fb);
    put(scope, st, S_CANCELABLE, fc);
    put(scope, st, S_COMPOSED, fp);
    put(scope, st, S_PREVENTED, f_false);
    put(scope, st, S_STOPPED, f_false);
    put(scope, st, S_IMMEDIATE, f_false);
    put(scope, st, S_PASSIVE, f_false);
    put(scope, st, S_STAMP, stamp);
    put(scope, st, S_TRUSTED, f_false);
    put(scope, st, S_DISPATCHED, f_false);
    put(scope, st, S_DETAIL, detail);
    put(scope, st, S_PATH, empty_path);
    put(scope, st, S_PATH_DONE, f_false);
    let this = args.this();
    let key = state_key(scope);
    this.set_private(scope, key, st.into());
    install_is_trusted(scope, this);
}

/// The [LegacyUnforgeable] isTrusted: an own getter-only accessor
/// `{e:true, c:false}` on every instance - including instances of page
/// subclasses, whose `this` the base constructor just branded.
fn install_is_trusted(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>) {
    let global = scope.get_current_context().global(scope);
    let dpk = private(scope, "diting#ev-dp");
    let Some(dp) = global.get_private(scope, dpk).and_then(|v| Local::<v8::Function>::try_from(v).ok()) else { return };
    let dk = private(scope, "diting#ev-itdesc");
    let Some(desc) = global.get_private(scope, dk).and_then(|v| Local::<v8::Object>::try_from(v).ok()) else { return };
    let name = s(scope, "isTrusted");
    let u = v8::undefined(scope);
    dp.call(scope, u.into(), &[this.into(), name.into(), desc.into()]);
}

// ---- prototype accessors -------------------------------------------------

unsafe extern "C" fn getter(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let (ii, mi) = dec(args.data().int32_value(scope).unwrap_or(0));
    let Some(st) = state_of(scope, args.this()) else { throw_type(scope, "Illegal invocation"); return };
    let name = TABLE[ii].members[mi].name;
    let mut rv = ReturnValue::from_function_callback_info(info);
    let prevented = flag(scope, st, S_PREVENTED);
    let stopped = flag(scope, st, S_STOPPED);
    let out: Local<v8::Value> = match name {
        "type" => idx(scope, st, S_TYPE),
        "target" | "srcElement" => idx(scope, st, S_TARGET),
        "currentTarget" => idx(scope, st, S_CURRENT),
        "eventPhase" => idx(scope, st, S_PHASE),
        "bubbles" => idx(scope, st, S_BUBBLES),
        "cancelable" => idx(scope, st, S_CANCELABLE),
        "composed" => idx(scope, st, S_COMPOSED),
        "defaultPrevented" => idx(scope, st, S_PREVENTED),
        "timeStamp" => idx(scope, st, S_STAMP),
        "detail" => idx(scope, st, S_DETAIL),
        "returnValue" => b(scope, !prevented),
        "cancelBubble" => b(scope, stopped),
        _ => v8::undefined(scope).into(),
    };
    rv.set(out);
}

unsafe extern "C" fn setter(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let (ii, mi) = dec(args.data().int32_value(scope).unwrap_or(0));
    let Some(st) = state_of(scope, args.this()) else { throw_type(scope, "Illegal invocation"); return };
    match TABLE[ii].members[mi].name {
        // Legacy pair: setting returnValue to a falsy value cancels (if
        // cancelable); setting it true does nothing.
        "returnValue" => {
            let can = flag(scope, st, S_CANCELABLE);
            let passive = flag(scope, st, S_PASSIVE);
            if !args.get(0).boolean_value(scope) && can && !passive {
                let t = b(scope, true);
                put(scope, st, S_PREVENTED, t);
            }
        }
        // Legacy IE alias: truthy stopPropagation; setting it false does
        // NOT clear the flag.
        "cancelBubble" => {
            if args.get(0).boolean_value(scope) {
                let t = b(scope, true);
                put(scope, st, S_STOPPED, t);
            }
        }
        _ => {}
    }
}

// ---- prototype methods ---------------------------------------------------

unsafe extern "C" fn method(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let (ii, mi) = dec(args.data().int32_value(scope).unwrap_or(0));
    let it = TABLE[ii];
    let m = &it.methods[mi];
    let Some(st) = state_of(scope, args.this()) else { throw_type(scope, "Illegal invocation"); return };
    let prefix = format!("Failed to execute '{}' on '{}'", m.name, it.name);
    let mut rv = ReturnValue::from_function_callback_info(info);
    match m.name {
        "composedPath" => {
            let path: Local<v8::Array> = idx(scope, st, S_PATH).try_into().ok().unwrap_or_else(|| v8::Array::new(scope, 0));
            if path.length() > 0 {
                rv.set(path.into());
                return;
            }
            // Outside a (native) dispatch the path is either fresh
            // (compute the ancestor walk, like Chrome does mid-dispatch on
            // a legacy-dispatched event) or already cleared (a dispatch
            // happened before - Chrome returns [] forever after).
            if flag(scope, st, S_PATH_DONE) {
                rv.set(v8::Array::new(scope, 0).into());
                return;
            }
            let target = idx(scope, st, S_TARGET);
            let mut nodes: Vec<Local<v8::Value>> = Vec::new();
            let mut cur = target;
            for _ in 0..4096 {
                let Ok(o) = Local::<v8::Object>::try_from(cur) else { break };
                nodes.push(cur);
                let pk = s(scope, "parentNode");
                cur = o.get(scope, pk.into()).unwrap_or_else(|| v8::null(scope).into());
                if cur.is_null() || cur.is_undefined() { break; }
            }
            if nodes.is_empty() {
                // Fresh event (target never set): Chrome's path is [] — no
                // window appended, no walk.
                let fresh: Local<v8::Value> = v8::Array::new(scope, 0).into();
                rv.set(fresh);
                return;
            }
            let global = scope.get_current_context().global(scope);
            let wk = s(scope, "window");
            let win = global.get(scope, wk.into()).unwrap_or_else(|| v8::undefined(scope).into());
            let last_is_win = nodes.last().map(|n| n.strict_equals(win)).unwrap_or(false);
            if !win.is_undefined() && !win.is_null() && !last_is_win { nodes.push(win); }
            let arr = v8::Array::new_with_elements(scope, &nodes);
            rv.set(arr.into());
        }
        "initEvent" | "initCustomEvent" => {
            if args.length() < 1 {
                throw_type(scope, &format!("{prefix}: 1 argument required, but only {} present.", args.length()));
                return;
            }
            // The dispatch flag makes the legacy initializers inert.
            if flag(scope, st, S_DISPATCHED) { return; }
            let a0 = args.get(0);
            if a0.is_symbol() {
                throw_type(scope, &format!("{prefix}: Cannot convert a Symbol value to a string"));
                return;
            }
            let Some(ty) = a0.to_string(scope) else { return };
            let bv = args.get(1).boolean_value(scope);
            let cv = args.get(2).boolean_value(scope);
            let fb = b(scope, bv);
            let fc = b(scope, cv);
            let f_false = b(scope, false);
            put(scope, st, S_TYPE, ty.into());
            put(scope, st, S_BUBBLES, fb);
            put(scope, st, S_CANCELABLE, fc);
            if m.name == "initCustomEvent" {
                let d = args.get(3);
                // `any detail = null`: undefined reads back as null.
                let dv: Local<v8::Value> = if d.is_undefined() { v8::null(scope).into() } else { d };
                put(scope, st, S_DETAIL, dv);
            }
            put(scope, st, S_PREVENTED, f_false);
            put(scope, st, S_STOPPED, f_false);
            put(scope, st, S_IMMEDIATE, f_false);
        }
        "preventDefault" => {
            let can = flag(scope, st, S_CANCELABLE);
            let passive = flag(scope, st, S_PASSIVE);
            if can && !passive {
                let t = b(scope, true);
                put(scope, st, S_PREVENTED, t);
            }
        }
        "stopPropagation" => {
            let t = b(scope, true);
            put(scope, st, S_STOPPED, t);
        }
        "stopImmediatePropagation" => {
            let t = b(scope, true);
            put(scope, st, S_STOPPED, t);
            put(scope, st, S_IMMEDIATE, t);
        }
        _ => {}
    }
}

// ---- the unforgeable isTrusted getter ------------------------------------

unsafe extern "C" fn it_get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let Some(st) = state_of(scope, args.this()) else { throw_type(scope, "Illegal invocation"); return };
    let mut rv = ReturnValue::from_function_callback_info(info);
    rv.set(idx(scope, st, S_TRUSTED));
}

// ---- the engine-internal state facade ------------------------------------

/// `__diting_event_state(obj, key[, value])` - the ONLY way engine JS
/// touches event internals (target/currentTarget/phase pins in the legacy
/// dispatchers, offset writes, propagation-flag reads). Branded objects
/// read/write the state array; anything else falls back to plain property
/// semantics so the legacy dispatchers keep tolerating hand-rolled event
/// objects. The global is captured and deleted by bootstrap.js at bake
/// time - page scripts never see it.
unsafe extern "C" fn state(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let Ok(obj) = Local::<v8::Object>::try_from(args.get(0)) else { return };
    let Some(key) = args.get(1).to_string(scope) else { return };
    let val = args.get(2);
    let Some(st) = state_of(scope, obj) else {
        // unbranded: legacy plain-object behavior
        if val.is_undefined() {
            let mut rv = ReturnValue::from_function_callback_info(info);
            rv.set(obj.get(scope, key.into()).unwrap_or_else(|| v8::undefined(scope).into()));
        } else {
            obj.set(scope, key.into(), val);
        }
        return;
    };
    let i = match key.to_rust_string_lossy(scope).as_str() {
        "type" => S_TYPE, "target" => S_TARGET, "currentTarget" => S_CURRENT,
        "eventPhase" => S_PHASE, "bubbles" => S_BUBBLES, "cancelable" => S_CANCELABLE,
        "composed" => S_COMPOSED, "defaultPrevented" => S_PREVENTED, "stopped" => S_STOPPED,
        "immediateStopped" => S_IMMEDIATE, "inPassive" => S_PASSIVE, "timeStamp" => S_STAMP,
        "isTrusted" => S_TRUSTED, "dispatched" => S_DISPATCHED, "detail" => S_DETAIL,
        "path" => S_PATH, "pathDone" => S_PATH_DONE,
        _ => return,
    };
    if val.is_undefined() {
        let mut rv = ReturnValue::from_function_callback_info(info);
        rv.set(idx(scope, st, i));
    } else {
        put(scope, st, i, val);
    }
}

// ---- install -------------------------------------------------------------

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    let mut templates: Vec<Local<FunctionTemplate>> = Vec::new();
    for (ii, it) in TABLE.iter().enumerate() {
        let t = FunctionTemplate::builder_raw(ctor).data(v8::Integer::new(scope, ii as i32).into()).length(1).build(scope);
        let name = s(scope, it.name);
        t.set_class_name(name);
        if let Some(p) = it.parent { t.inherit(templates[p]); }
        t.read_only_prototype();
        let proto = t.prototype_template(scope);
        proto.set_with_attr(v8::Symbol::get_to_string_tag(scope).into(), name.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM);
        for (mi, m) in it.members.iter().enumerate() {
            let g = FunctionTemplate::builder_raw(getter).data(v8::Integer::new(scope, enc(ii, mi)).into()).length(0).build(scope);
            g.set_class_name(s(scope, &format!("get {}", m.name)));
            let st = if m.set {
                let f = FunctionTemplate::builder_raw(setter).data(v8::Integer::new(scope, enc(ii, mi)).into()).length(1).build(scope);
                f.set_class_name(s(scope, &format!("set {}", m.name)));
                Some(f)
            } else { None };
            proto.set_accessor_property(s(scope, m.name).into(), Some(g), st, PropertyAttribute::NONE);
        }
        for (cname, cval) in it.consts {
            let k = s(scope, cname);
            let v = v8::Integer::new(scope, *cval);
            t.set_with_attr(k.into(), v.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_DELETE);
            proto.set_with_attr(k.into(), v.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_DELETE);
        }
        for (mi, m) in it.methods.iter().enumerate() {
            let f = FunctionTemplate::builder_raw(method).data(v8::Integer::new(scope, enc(ii, mi)).into()).length(m.len).build(scope);
            f.set_class_name(s(scope, m.name));
            proto.set_with_attr(s(scope, m.name).into(), f.into(), PropertyAttribute::NONE);
        }
        templates.push(t);
    }
    let mut fns: Vec<Local<v8::Function>> = Vec::new();
    for (ii, it) in TABLE.iter().enumerate() {
        let f = templates[ii].get_function(scope).expect("template instantiates");
        if let Some(p) = it.parent { f.set_prototype(scope, fns[p].into()); }
        fns.push(f);
    }
    for (it, f) in TABLE.iter().zip(fns) {
        let gname = s(scope, it.name);
        global.define_own_property(scope, gname.into(), f.into(), PropertyAttribute::DONT_ENUM);
    }
    // The shared isTrusted getter + descriptor, and the intrinsic
    // Object.defineProperty used to stamp it per instance.
    let itg = FunctionTemplate::builder_raw(it_get).length(0).build(scope);
    itg.set_class_name(s(scope, "get isTrusted"));
    let itgf = itg.get_function(scope).expect("isTrusted getter instantiates");
    let desc = v8::Object::new(scope);
    let k_get = s(scope, "get");
    desc.set(scope, k_get.into(), itgf.into());
    let k_enum = s(scope, "enumerable");
    let v_true = b(scope, true);
    desc.set(scope, k_enum.into(), v_true);
    let k_conf = s(scope, "configurable");
    let v_false = b(scope, false);
    desc.set(scope, k_conf.into(), v_false);
    let obj_k = s(scope, "Object");
    let obj = global.get(scope, obj_k.into()).and_then(|o| Local::<v8::Object>::try_from(o).ok());
    let dp_k = s(scope, "defineProperty");
    let dp = obj.and_then(|o| o.get(scope, dp_k.into())).and_then(|f| Local::<v8::Function>::try_from(f).ok());
    if let Some(dp) = dp {
        let dp_p = private(scope, "diting#ev-dp");
        global.set_private(scope, dp_p, dp.into());
        let itdesc_p = private(scope, "diting#ev-itdesc");
        global.set_private(scope, itdesc_p, desc.into());
    }
    // engine-internal facade (bootstrap.js captures and deletes the global)
    let sf = v8::Function::new_raw(scope, state as Cb).unwrap();
    let name = s(scope, "__diting_event_state");
    sf.set_name(name);
    global.set(scope, name.into(), sf.into());
}
