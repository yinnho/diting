//! URLPattern, native. The logic core is the vendored rust-urlpattern crate
//! (crates/diting_urlpattern, validated 741/741 against the measured Chrome
//! matrix before any wiring); this file is the WebIDL face: constructor
//! overload resolution, dictionary reading with Chrome's exact error
//! wording, the eight component getters, test/exec input processing and
//! the exec result object shape. Instances are runtime-only (never baked),
//! so state is a boxed compiled pattern behind a v8 External plus a
//! guaranteed finalizer; identity is the family tag in internal field 0.
//!
//! Known small compromise: the Weak handle itself is mem::forgotten so the
//! finalizer survives (dropping a Weak cancels its finalizer, and
//! re-adopting it inside the guaranteed finalizer would touch a possibly-
//! dying isolate). Cost: the ~48-byte WeakData per instance, never the
//! compiled pattern itself. Documented in the migration ledger.

use diting_urlpattern as up;
use v8::{FunctionCallbackArguments, FunctionCallbackInfo, Local, ReturnValue};

use super::matrix_conv::{illegal, throw_type};

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

const FAMILY: i32 = 0x0D07;
const NFIELDS: usize = 2;

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [
        ctor as Cb,
        comp_get as Cb,
        has_re_get as Cb,
        test_exec as Cb,
    ]
    .into_iter()
    .map(|f| v8::ExternalReference { function: f })
    .collect()
}

// ---- state ----------------------------------------------------------------

fn write_state(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, pat: up::UrlPattern) {
    let ptr = Box::into_raw(Box::new(pat));
    let ext = v8::External::new(scope, ptr as *mut std::ffi::c_void);
    this.set_internal_field(0, v8::Integer::new(scope, FAMILY).into());
    this.set_internal_field(1, ext.into());
    let fin = move || unsafe {
        drop(Box::from_raw(ptr));
    };
    // dropping a rusty-v8 Weak cancels its finalizer, so the handle must
    // never be dropped: ManuallyDrop keeps the finalizer armed for the
    // object lifetime. v8 reclaims the WeakData bookkeeping at context
    // teardown; the UrlPattern box itself is reclaimed by the finalizer.
    let weak = v8::Weak::with_guaranteed_finalizer(scope, this, Box::new(fin));
    let _pinned = std::mem::ManuallyDrop::new(weak);
}

fn read_state(
    scope: &mut v8::PinScope<'_, '_>,
    this: Local<v8::Object>,
) -> Option<&'static up::UrlPattern> {
    if this.internal_field_count() != NFIELDS {
        return None;
    }
    let tag: Local<v8::Value> = this.get_internal_field(scope, 0)?.try_into().ok()?;
    if tag.int32_value(scope)? != FAMILY {
        return None;
    }
    let ext: Local<v8::Value> = this.get_internal_field(scope, 1)?.try_into().ok()?;
    let ext = Local::<v8::External>::try_from(ext).ok()?;
    let ptr = ext.value() as *const up::UrlPattern;
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the pointer is written once at construction and the box is
    // dropped only by the GC finalizer, which cannot run while a live
    // reference reached it here.
    Some(unsafe { &*ptr })
}

// ---- small helpers --------------------------------------------------------

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

fn is_obj(v: Local<v8::Value>) -> bool {
    (v.is_object() && !v.is_null()) || v.is_function()
}

fn usv(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>, pre: &str) -> Option<String> {
    if v.is_symbol() {
        throw_type(scope, "Cannot convert a Symbol value to a string", pre);
        return None;
    }
    let s = v.to_string(scope)?;
    Some(s.to_rust_string_lossy(scope))
}

// ---- init dictionary reading ----------------------------------------------
// WebIDL dictionary member order is lexicographic: baseURL first. Returns
// the init plus the raw baseURL string separately (the crate's init wants
// a parsed Url, but the parse failure wording is ours to produce).

const INIT_KEYS: [&str; 9] = [
    "baseURL", "hash", "hostname", "password", "pathname", "port", "protocol", "search", "username",
];

fn read_init(
    scope: &mut v8::PinScope<'_, '_>,
    o: Local<v8::Object>,
    pre: &str,
) -> Option<(up::UrlPatternInit, Option<String>)> {
    let mut init = up::UrlPatternInit::default();
    let mut base_raw = None;
    for k in INIT_KEYS {
        let key = v8::String::new(scope, k).unwrap();
        let v = o.get(scope, key.into())?;
        if v.is_undefined() {
            continue;
        }
        let conv_pre = format!("{pre}: Failed to read the '{k}' property from 'URLPatternInit'");
        let s = usv(scope, v, &conv_pre)?;
        match k {
            "baseURL" => base_raw = Some(s),
            "hash" => init.hash = Some(s),
            "hostname" => init.hostname = Some(s),
            "password" => init.password = Some(s),
            "pathname" => init.pathname = Some(s),
            "port" => init.port = Some(s),
            "protocol" => init.protocol = Some(s),
            "search" => init.search = Some(s),
            "username" => init.username = Some(s),
            _ => unreachable!(),
        }
    }
    Some((init, base_raw))
}

fn read_ignore_case(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>) -> bool {
    if !is_obj(v) {
        return false;
    }
    let Ok(obj) = Local::<v8::Object>::try_from(v) else {
        return false;
    };
    let key = v8::String::new(scope, "ignoreCase").unwrap();
    let Some(ic) = obj.get(scope, key.into()) else {
        return false;
    };
    ic.boolean_value(scope)
}

// ---- constructor ----------------------------------------------------------

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let pre = "Failed to construct 'URLPattern'";
    if !args.is_construct_call() {
        throw_type(scope, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", pre);
        return;
    }
    let input = {
        let v = args.get(0);
        if v.is_null() {
            v8::undefined(scope).into()
        } else {
            v
        }
    };
    let second = args.get(1);
    let third = args.get(2);
    let argc = args.length();
    let second_is_string =
        argc >= 3 || (!second.is_undefined() && !second.is_null() && !is_obj(second));
    let options = if second_is_string { third } else { second };

    let mut init = up::UrlPatternInit::default();
    if is_obj(input) {
        let obj = Local::<v8::Object>::try_from(input).unwrap();
        let Some((read, base_raw)) = read_init(scope, obj, pre) else {
            return;
        };
        init = read;
        if let Some(raw) = base_raw {
            match url::Url::parse(&raw) {
                Ok(u) => init.base_url = Some(u),
                Err(_) => {
                    throw_type(scope, &format!("Invalid baseURL '{raw}'."), pre);
                    return;
                }
            }
        }
        if second_is_string {
            let Some(b) = usv(scope, second, pre) else {
                return;
            };
            throw_type(scope, &format!("Invalid second argument baseURL '{b}' provided with a URLPatternInit input. Use the URLPatternInit.baseURL property instead."), pre);
            return;
        }
    } else if input.is_undefined() {
        // chrome treats an absent/undefined/null first argument as the
        // default empty dictionary — the init overload — so a string base
        // second argument is rejected exactly as with an object input
        // (pinned: new URLPattern(undefined, 'https://b.com/') throws)
        if second_is_string {
            let Some(b) = usv(scope, second, pre) else {
                return;
            };
            throw_type(scope, &format!("Invalid second argument baseURL '{b}' provided with a URLPatternInit input. Use the URLPatternInit.baseURL property instead."), pre);
            return;
        }
    } else {
        let s = match usv(scope, input, pre) {
            Some(s) => Some(s),
            None => return,
        };
        if let Some(s) = s {
            let base_raw = if second_is_string {
                match usv(scope, second, pre) {
                    Some(b) => Some(b),
                    None => return,
                }
            } else {
                None
            };
            // chrome validates the base string before the relative-input
            // check, and even when the constructor string is absolute
            // (pinned: '/x' + 'not a url' and 'https://x.com/ABC' + undefined
            // both throw Invalid baseURL)
            let base = match &base_raw {
                Some(raw) => match url::Url::parse(raw) {
                    Ok(u) => Some(u),
                    Err(_) => {
                        throw_type(scope, &format!("Invalid baseURL '{raw}'."), pre);
                        return;
                    }
                },
                None => None,
            };
            match up::UrlPatternInit::parse_constructor_string::<regex::Regex>(&s, base.clone()) {
                Ok(pi) => init = pi,
                Err(up::Error::BaseUrlRequired) => {
                    throw_type(scope, &format!("Relative constructor string '{s}' must have a base URL passed as the second argument."), pre);
                    return;
                }
                Err(e) => {
                    throw_type(scope, &format!("{e}"), pre);
                    return;
                }
            }
            // parse_constructor_string already threaded the base into the init
        }
    }
    if !options.is_undefined() && !options.is_null() && !is_obj(options) {
        throw_type(
            scope,
            "The provided value is not of type 'URLPatternOptions'.",
            pre,
        );
        return;
    }
    let ignore_case = if options.is_undefined() || options.is_null() {
        false
    } else {
        read_ignore_case(scope, options)
    };

    let opts = up::UrlPatternOptions { ignore_case };
    match up::UrlPattern::parse(init, opts) {
        Ok(p) => write_state(scope, args.this(), p),
        Err(e) => throw_type(scope, &format!("{e}"), pre),
    }
}

// ---- getters / methods ----------------------------------------------------

const COMPONENTS: [&str; 8] = [
    "protocol", "username", "password", "hostname", "port", "pathname", "search", "hash",
];

unsafe extern "C" fn comp_get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let mut rv = ReturnValue::from_function_callback_info(info);
    let Some(p) = read_state(scope, args.this()) else {
        illegal(scope);
        return;
    };
    let idx = args.data().int32_value(scope).unwrap_or(0) as usize;
    let s = match idx {
        0 => p.protocol(),
        1 => p.username(),
        2 => p.password(),
        3 => p.hostname(),
        4 => p.port(),
        5 => p.pathname(),
        6 => p.search(),
        _ => p.hash(),
    };
    rv.set(v8::String::new(scope, s).unwrap().into());
}

unsafe extern "C" fn has_re_get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let mut rv = ReturnValue::from_function_callback_info(info);
    let Some(p) = read_state(scope, args.this()) else {
        illegal(scope);
        return;
    };
    rv.set(v8::Boolean::new(scope, p.has_regexp_groups()).into());
}

// ---- test / exec ----------------------------------------------------------

unsafe extern "C" fn test_exec(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let is_test = args.data().int32_value(scope).unwrap_or(0) != 0;
    let name = if is_test { "test" } else { "exec" };
    let pre_owned = format!("Failed to execute '{name}' on 'URLPattern'");
    let pre = pre_owned.as_str();
    let mut rv = ReturnValue::from_function_callback_info(info);
    let Some(p) = read_state(scope, args.this()) else {
        illegal(scope);
        return;
    };
    let input = {
        let v = args.get(0);
        if v.is_null() {
            v8::undefined(scope).into()
        } else {
            v
        }
    };
    let base_arg = args.get(1);

    let (mi, echo): (up::UrlPatternMatchInput, Vec<Local<v8::Value>>) = if is_obj(input) {
        let obj = Local::<v8::Object>::try_from(input).unwrap();
        let Some((mut init, base_raw)) = read_init(scope, obj, pre) else {
            return;
        };
        if !base_arg.is_undefined() {
            let Some(b) = usv(scope, base_arg, pre) else {
                return;
            };
            throw_type(scope, &format!("Invalid second argument baseURL '{b}' provided with a URLPatternInit input. Use the URLPatternInit.baseURL property instead."), pre);
            return;
        }
        if let Some(raw) = &base_raw {
            init.base_url = url::Url::parse(raw).ok();
        }
        // inputs echo: a shallow copy of the init dictionary as read
        let echo_obj = v8::Object::new(scope);
        for (k, val) in [
            ("baseURL", base_raw.as_deref()),
            ("hash", init.hash.as_deref()),
            ("hostname", init.hostname.as_deref()),
            ("password", init.password.as_deref()),
            ("pathname", init.pathname.as_deref()),
            ("port", init.port.as_deref()),
            ("protocol", init.protocol.as_deref()),
            ("search", init.search.as_deref()),
            ("username", init.username.as_deref()),
        ] {
            if let Some(v) = val {
                let key = v8::String::new(scope, k).unwrap();
                let sv = v8::String::new(scope, v).unwrap();
                echo_obj.set(scope, key.into(), sv.into());
            }
        }
        (up::UrlPatternMatchInput::Init(init), vec![echo_obj.into()])
    } else {
        let s = if input.is_undefined() {
            None
        } else {
            match usv(scope, input, pre) {
                Some(s) => Some(s),
                None => return,
            }
        };
        let b = if base_arg.is_undefined() {
            None
        } else {
            match usv(scope, base_arg, pre) {
                Some(b) => Some(b),
                None => return,
            }
        };
        match s {
            None => {
                // undefined/null input is the empty-init overload: a base
                // second argument is rejected, not ignored (pinned:
                // test(undefined, 'https://b.com/') throws)
                if !base_arg.is_undefined() {
                    let Some(b) = usv(scope, base_arg, pre) else {
                        return;
                    };
                    throw_type(scope, &format!("Invalid second argument baseURL '{b}' provided with a URLPatternInit input. Use the URLPatternInit.baseURL property instead."), pre);
                    return;
                }
                let empty = v8::Object::new(scope);
                (
                    up::UrlPatternMatchInput::Init(up::UrlPatternInit::default()),
                    vec![empty.into()],
                )
            }
            Some(s) => {
                let parsed = match &b {
                    None => url::Url::parse(&s),
                    Some(b) => match url::Url::parse(b) {
                        Ok(base) => base.join(&s),
                        Err(_) => {
                            if is_test {
                                rv.set(v8::Boolean::new(scope, false).into());
                            } else {
                                rv.set(v8::null(scope).into());
                            }
                            return;
                        }
                    },
                };
                match parsed {
                    Ok(u) => {
                        let mut echo: Vec<Local<v8::Value>> = Vec::new();
                        echo.push(v8::String::new(scope, &s).unwrap().into());
                        if let Some(b) = &b {
                            echo.push(v8::String::new(scope, b).unwrap().into());
                        }
                        (up::UrlPatternMatchInput::Url(u), echo)
                    }
                    Err(_) => {
                        if is_test {
                            rv.set(v8::Boolean::new(scope, false).into());
                        } else {
                            rv.set(v8::null(scope).into());
                        }
                        return;
                    }
                }
            }
        }
    };

    let result = p.exec(mi).ok().flatten();
    if is_test {
        rv.set(v8::Boolean::new(scope, result.is_some()).into());
        return;
    }
    let Some(res) = result else {
        rv.set(v8::null(scope).into());
        return;
    };
    // chrome pins lexicographic member order on the result dictionary,
    // inputs included in its alphabetical slot
    let out = v8::Object::new(scope);
    let comps = [
        ("hash", &res.hash),
        ("hostname", &res.hostname),
        ("password", &res.password),
        ("pathname", &res.pathname),
        ("port", &res.port),
        ("protocol", &res.protocol),
        ("search", &res.search),
        ("username", &res.username),
    ];
    let mut inputs_placed = false;
    for (cn, cr) in comps {
        if !inputs_placed && cn > "inputs" {
            let ik = v8::String::new(scope, "inputs").unwrap();
            let arr = v8::Array::new(scope, echo.len() as i32);
            for (i, e) in echo.iter().enumerate() {
                arr.set_index(scope, i as u32, *e);
            }
            out.set(scope, ik.into(), arr.into());
            inputs_placed = true;
        }
        let cobj = v8::Object::new(scope);
        let gobj = v8::Object::new(scope);
        for (gn, gv) in &cr.groups {
            let key = v8::String::new(scope, gn).unwrap();
            let val: Local<v8::Value> = match gv {
                Some(v) => v8::String::new(scope, v).unwrap().into(),
                None => v8::undefined(scope).into(),
            };
            gobj.set(scope, key.into(), val);
        }
        let gk = v8::String::new(scope, "groups").unwrap();
        cobj.set(scope, gk.into(), gobj.into());
        let ik = v8::String::new(scope, "input").unwrap();
        let iv = v8::String::new(scope, &cr.input).unwrap();
        cobj.set(scope, ik.into(), iv.into());
        let ck = v8::String::new(scope, cn).unwrap();
        out.set(scope, ck.into(), cobj.into());
    }
    rv.set(out.into());
}

// ---- install --------------------------------------------------------------

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    let t = builder(scope, ctor, 0, 0);
    let nm = v8::String::new(scope, "URLPattern").unwrap();
    t.set_class_name(nm);
    t.instance_template(scope).set_internal_field_count(NFIELDS);
    t.read_only_prototype();
    let proto = t.prototype_template(scope);
    let tag_key = v8::Symbol::get_to_string_tag(scope);
    proto.set_with_attr(
        tag_key.into(),
        nm.into(),
        v8::PropertyAttribute::READ_ONLY | v8::PropertyAttribute::DONT_ENUM,
    );
    for (i, cn) in COMPONENTS.iter().enumerate() {
        let g = builder(scope, comp_get, i as i32, 0);
        let gn = v8::String::new(scope, &format!("get {cn}")).unwrap();
        g.set_class_name(gn);
        let key = v8::String::new(scope, cn).unwrap();
        proto.set_accessor_property(key.into(), Some(g), None, v8::PropertyAttribute::NONE);
    }
    {
        let g = builder(scope, has_re_get, 0, 0);
        let gn = v8::String::new(scope, "get hasRegExpGroups").unwrap();
        g.set_class_name(gn);
        let key = v8::String::new(scope, "hasRegExpGroups").unwrap();
        proto.set_accessor_property(key.into(), Some(g), None, v8::PropertyAttribute::NONE);
    }
    for (name, data) in [("exec", 0), ("test", 1)] {
        let f = builder(scope, test_exec, data, 0);
        let key = v8::String::new(scope, name).unwrap();
        proto.set_with_attr(key.into(), f.into(), v8::PropertyAttribute::NONE);
    }
    if let Some(f) = t.get_function(scope) {
        let key = v8::String::new(scope, "URLPattern").unwrap();
        global.define_own_property(
            scope,
            key.into(),
            f.into(),
            v8::PropertyAttribute::DONT_ENUM,
        );
    }
}
