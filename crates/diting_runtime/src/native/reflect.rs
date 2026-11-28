//! Native HTML reflected-attribute accessors: the mass migration of
//! gen/htmlreflect.gen.js's 237 measured reflectors. A reflector is pure
//! data — an attribute name, a conversion kind, and the measured enum/
//! default tables — so one pair of raw callbacks serves every member; the
//! per-member spec rides the callback `data` word as an index into
//! [`SPECS`]. The JS face (core.js P.reflector) delegates to the installer
//! here whenever the measured table covers the member and keeps its
//! closures for the rest.
//!
//! Attribute access goes through the element's OWN JS methods
//! (getAttribute/setAttribute/removeAttribute/hasAttribute) rather than a
//! host bridge: every side effect living in those wrappers — mutation
//! observer notification, custom-element reactions, the id/name registries,
//! stylesheet restarts, the dom crate's mutation stamps — then applies to
//! reflected writes exactly as it did when the reflectors were JS closures.
//! Chrome internally bypasses the public API here, but the engine's
//! pre-migration behavior is the compatibility bar.

use v8::{FunctionCallbackArguments, FunctionCallbackInfo, Local, PropertyAttribute, ReturnValue};

use super::reflect_table::SPECS;

/// Variants without a current table entry (BoolTrueFalse,
/// UnsignedLongDim) stay: the generator emits the full kind vocabulary
/// and a future measured member must not need an engine rebuild.
#[allow(dead_code)]
pub(crate) enum Kind {
    String,
    StringNte,
    BoolPresence,
    BoolTrueFalse,
    Enum,
    UnsignedLong,
    UnsignedLongDim,
    UlongDef,
    Long,
    Crossorigin,
    Url,
}

pub(crate) struct Spec {
    pub iface: &'static str,
    pub member: &'static str,
    pub attr: &'static str,
    pub kind: Kind,
    /// string default; enum missing-state value
    pub default: &'static str,
    /// enum invalid-state value
    pub invalid: &'static str,
    /// enum empty-string value (None: '' falls through to invalid)
    pub empty: Option<&'static str>,
    /// enum keyword map: ascii-lowercased attribute value -> reflected value
    pub map: &'static [(&'static str, &'static str)],
    /// ulong-def minimum
    pub min: u32,
    /// ulong-def numeric default
    pub udefault: u32,
    /// url kind: an ABSENT attribute reads document.URL (formAction)
    pub fallback_doc: bool,
    /// null/undefined set removes the attribute instead of stringifying
    pub null_removes: bool,
}

fn throw_type_error(scope: &mut v8::PinScope<'_, '_>, msg: &str) {
    let m = v8::String::new(scope, msg).unwrap();
    let e = v8::Exception::type_error(scope, m);
    // core.js fail() shape: the decorated message is an ENUMERABLE data
    // property (Blink sets it afterwards) — v8's own is non-enumerable.
    if let Ok(obj) = Local::<v8::Object>::try_from(e) {
        let key = v8::String::new(scope, "message").unwrap();
        obj.define_own_property(scope, key.into(), m.into(), PropertyAttribute::NONE);
    }
    scope.throw_exception(e);
}

fn illegal(scope: &mut v8::PinScope<'_, '_>) {
    throw_type_error(scope, "Illegal invocation");
}

// ---- attribute access through the element's own JS methods ----

fn call_method<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    this: Local<v8::Object>,
    name: &str,
    argv: &[Local<v8::Value>],
) -> Option<Local<'s, v8::Value>> {
    let key = v8::String::new(scope, name).unwrap();
    let f = this.get(scope, key.into())?;
    let f = Local::<v8::Function>::try_from(f).ok()?;
    f.call(scope, this.into(), argv)
}

fn get_attr(
    scope: &mut v8::PinScope<'_, '_>,
    this: Local<v8::Object>,
    attr: &str,
) -> Option<Option<String>> {
    let key = v8::String::new(scope, attr).unwrap();
    let v = call_method(scope, this, "getAttribute", &[key.into()])?;
    if v.is_null() || v.is_undefined() {
        Some(None)
    } else {
        let s = Local::<v8::String>::try_from(v).ok()?;
        Some(Some(s.to_rust_string_lossy(scope)))
    }
}

fn has_attr(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, attr: &str) -> bool {
    let key = v8::String::new(scope, attr).unwrap();
    call_method(scope, this, "hasAttribute", &[key.into()])
        .map(|v| v.boolean_value(scope))
        .unwrap_or(false)
}

fn set_attr(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, attr: &str, value: &str) {
    let k = v8::String::new(scope, attr).unwrap();
    let val = v8::String::new(scope, value).unwrap();
    call_method(scope, this, "setAttribute", &[k.into(), val.into()]);
}

fn remove_attr(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, attr: &str) {
    let k = v8::String::new(scope, attr).unwrap();
    call_method(scope, this, "removeAttribute", &[k.into()]);
}

/// ownerDocument string property (baseURI / URL) for the url kind.
fn doc_prop(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>, prop: &str) -> String {
    let odk = v8::String::new(scope, "ownerDocument").unwrap();
    let Some(doc) = this.get(scope, odk.into()) else {
        return String::new();
    };
    let Ok(doc) = Local::<v8::Object>::try_from(doc) else {
        return String::new();
    };
    let key = v8::String::new(scope, prop).unwrap();
    doc.get(scope, key.into())
        .and_then(|v| Local::<v8::String>::try_from(v).ok())
        .map(|s| s.to_rust_string_lossy(scope))
        .unwrap_or_default()
}

// ---- conversions: exact ports of the core.js reflector helpers ----

fn ascii_lower(v: &str) -> String {
    v.chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

/// HTML_WS_INT: leading whitespace [\t\n\f\r ]*, optional sign, digits.
/// None when the digits are missing.
fn parse_int_attr(v: &str) -> Option<f64> {
    let b = v.as_bytes();
    let mut i = 0;
    while i < b.len() && matches!(b[i], b'\t' | b'\n' | 0x0c | b'\r' | b' ') {
        i += 1;
    }
    let neg = if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        let n = b[i] == b'-';
        i += 1;
        n
    } else {
        false
    };
    let start = i;
    let mut n: f64 = 0.0;
    while i < b.len() && b[i].is_ascii_digit() {
        n = n * 10.0 + f64::from(b[i] - b'0');
        i += 1;
    }
    if i == start {
        return None;
    }
    Some(if neg { -n } else { n })
}

fn to_int32(n: f64) -> i32 {
    if !n.is_finite() {
        return 0;
    }
    let t = n.trunc() % 4294967296.0;
    let t = if t < 0.0 { t + 4294967296.0 } else { t };
    if t > 2147483647.0 {
        (t - 4294967296.0) as i64 as i32
    } else {
        t as i64 as i32
    }
}

fn to_uint32(n: f64) -> u32 {
    if !n.is_finite() {
        return 0;
    }
    let t = n.trunc() % 4294967296.0;
    let t = if t < 0.0 { t + 4294967296.0 } else { t };
    t as i64 as u32
}

// ---- this -> nid: the element wrapper carries a hidden own `_nid` — its
// absence means the accessor was invoked off-brand ----

fn nid_present(scope: &mut v8::PinScope<'_, '_>, this: Local<v8::Object>) -> bool {
    let key = v8::String::new(scope, "_nid").unwrap();
    this.get(scope, key.into())
        .map(|v| v.is_number())
        .unwrap_or(false)
}

fn spec_from_args(
    scope: &mut v8::PinScope<'_, '_>,
    args: &FunctionCallbackArguments,
) -> &'static Spec {
    let idx = args.data().int32_value(scope).unwrap_or(0) as usize;
    &SPECS[idx]
}

// ---- the accessor callbacks ----

unsafe extern "C" fn reflect_get(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let spec = spec_from_args(scope, &args);
    let mut rv = ReturnValue::from_function_callback_info(info);
    let this = args.this();
    if !nid_present(scope, this) {
        illegal(scope);
        return;
    }
    let attr = get_attr(scope, this, spec.attr).flatten();
    match spec.kind {
        Kind::String | Kind::StringNte => {
            let out = match &attr {
                None => spec.default,
                Some(v) => v.as_str(),
            };
            rv.set(v8::String::new(scope, out).unwrap().into());
        }
        Kind::BoolPresence => {
            let has = has_attr(scope, this, spec.attr);
            rv.set(v8::Boolean::new(scope, has).into());
        }
        Kind::BoolTrueFalse => {
            rv.set(v8::Boolean::new(scope, attr.as_deref() == Some("true")).into());
        }
        Kind::Enum => {
            let out: &str = match &attr {
                None => spec.default,
                Some(v) => {
                    let low = ascii_lower(v);
                    let keyed = spec
                        .map
                        .iter()
                        .find(|(k, _)| *k == low)
                        .map(|(_, val)| *val);
                    // empty-state value wins over the keyword map for '',
                    // then the map, then the invalid-state fallback
                    if low.is_empty() {
                        spec.empty.or(keyed).unwrap_or(spec.invalid)
                    } else {
                        keyed.unwrap_or(spec.invalid)
                    }
                }
            };
            rv.set(v8::String::new(scope, out).unwrap().into());
        }
        Kind::UnsignedLong | Kind::UnsignedLongDim => {
            let max = if matches!(spec.kind, Kind::UnsignedLong) {
                2147483647.0
            } else {
                4294967295.0
            };
            let out = match &attr {
                None => 0.0,
                Some(v) => match parse_int_attr(v) {
                    Some(n) if (0.0..=max).contains(&n) => n,
                    _ => 0.0,
                },
            };
            rv.set(v8::Number::new(scope, out).into());
        }
        Kind::UlongDef => {
            let out = match &attr {
                None => f64::from(spec.udefault),
                Some(v) => match parse_int_attr(v) {
                    Some(n) if n >= f64::from(spec.min) && n <= 2147483647.0 => n,
                    _ => f64::from(spec.udefault),
                },
            };
            rv.set(v8::Number::new(scope, out).into());
        }
        Kind::Long => {
            let out = match &attr {
                None => 0.0,
                Some(v) => match parse_int_attr(v) {
                    Some(n) if (-2147483648.0..=2147483647.0).contains(&n) => n,
                    _ => 0.0,
                },
            };
            rv.set(v8::Number::new(scope, out).into());
        }
        Kind::Crossorigin => match &attr {
            None => {
                rv.set(v8::null(scope).into());
            }
            Some(v) => {
                let out = if ascii_lower(v) == "use-credentials" {
                    "use-credentials"
                } else {
                    "anonymous"
                };
                rv.set(v8::String::new(scope, out).unwrap().into());
            }
        },
        Kind::Url => {
            let out = match &attr {
                None => {
                    if spec.fallback_doc {
                        doc_prop(scope, this, "URL")
                    } else {
                        spec.default.to_string()
                    }
                }
                Some(v) => {
                    let base = doc_prop(scope, this, "baseURI");
                    let resolved = url::Url::parse(&base)
                        .and_then(|b| b.join(v))
                        .or_else(|_| url::Url::parse(v));
                    match resolved {
                        Ok(u) => u.to_string(),
                        Err(_) => v.clone(),
                    }
                }
            };
            rv.set(v8::String::new(scope, &out).unwrap().into());
        }
    }
}

unsafe extern "C" fn reflect_set(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let spec = spec_from_args(scope, &args);
    let this = args.this();
    if !nid_present(scope, this) {
        illegal(scope);
        return;
    }
    let v = args.get(0);
    let prefix = format!(
        "Failed to set the '{}' property on '{}'",
        spec.member, spec.iface
    );
    macro_rules! dom_string {
        () => {{
            if v.is_symbol() {
                throw_type_error(
                    scope,
                    &format!("{prefix}: Cannot convert a Symbol value to a string"),
                );
                return;
            }
            let Some(s) = v.to_string(scope) else {
                return;
            };
            s.to_rust_string_lossy(scope)
        }};
    }
    macro_rules! number {
        () => {{
            if v.is_symbol() {
                throw_type_error(
                    scope,
                    &format!("{prefix}: Cannot convert a Symbol value to a number"),
                );
                return;
            }
            let Some(n) = v.number_value(scope) else {
                return;
            };
            n
        }};
    }
    match spec.kind {
        Kind::BoolPresence => {
            if v.boolean_value(scope) {
                set_attr(scope, this, spec.attr, "");
            } else {
                remove_attr(scope, this, spec.attr);
            }
        }
        Kind::BoolTrueFalse => {
            let out = if v.boolean_value(scope) {
                "true"
            } else {
                "false"
            };
            set_attr(scope, this, spec.attr, out);
        }
        Kind::UnsignedLong | Kind::UnsignedLongDim => {
            let n = number!();
            let u = to_uint32(n);
            let out = if u > 2147483647 { 0 } else { u };
            set_attr(scope, this, spec.attr, &out.to_string());
        }
        Kind::UlongDef => {
            let n = number!();
            let u = to_uint32(n);
            let out = if u > 2147483647 || u < spec.min {
                spec.udefault
            } else {
                u
            };
            set_attr(scope, this, spec.attr, &out.to_string());
        }
        Kind::Long => {
            let n = number!();
            set_attr(scope, this, spec.attr, &to_int32(n).to_string());
        }
        Kind::Crossorigin => {
            if v.is_null() || v.is_undefined() {
                remove_attr(scope, this, spec.attr);
                return;
            }
            let s = dom_string!();
            set_attr(scope, this, spec.attr, &s);
        }
        Kind::StringNte => {
            let s = if v.is_null() {
                String::new()
            } else {
                dom_string!()
            };
            set_attr(scope, this, spec.attr, &s);
        }
        _ => {
            // string / url / enum: the attribute lands AS GIVEN (the getter
            // normalizes); nullRemoves members treat null/undefined as
            // "no value".
            if spec.null_removes && (v.is_null() || v.is_undefined()) {
                remove_attr(scope, this, spec.attr);
                return;
            }
            let s = dom_string!();
            set_attr(scope, this, spec.attr, &s);
        }
    }
}

// ---- the installer: __diting_reflect_one(proto, member, iface) ----

/// Called from core.js P.reflector during the bake for every member whose
/// spec the measured table covers. Builds the accessor pair with the spec
/// index as callback data and seats it through Object.defineProperty — the
/// only path to the exact accessor descriptor (enumerable, configurable);
/// v8's SetAccessor neither controls enumerability nor materializes the
/// getter/setter function shapes the truth pins. Returns false when the
/// table does not cover the member so the JS face keeps its own closure.
unsafe extern "C" fn reflect_install_one(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let mut rv = ReturnValue::from_function_callback_info(info);
    macro_rules! fail {
        () => {{
            rv.set(v8::Boolean::new(scope, false).into());
            return;
        }};
    }
    let Ok(proto) = Local::<v8::Object>::try_from(args.get(0)) else {
        fail!();
    };
    let member = args.get(1).to_rust_string_lossy(scope);
    let iface = args.get(2).to_rust_string_lossy(scope);
    // A/B mass measurement: DITING_REFLECT_OFF in the bake environment
    // disables the native arm so the blob diff isolates exactly the
    // native-vs-closure reflector mass (same JS, both arms).
    if std::env::var("DITING_REFLECT_OFF").is_ok() {
        fail!();
    }
    let Some(idx) = SPECS
        .iter()
        .position(|s| s.iface == iface && s.member == member)
    else {
        fail!();
    };
    let data = v8::Integer::new(scope, idx as i32);
    let g_t = v8::FunctionTemplate::builder_raw(reflect_get)
        .data(data.into())
        .length(0)
        .build(scope);
    let gname = v8::String::new(scope, &format!("get {member}")).unwrap();
    g_t.set_class_name(gname);
    let s_t = v8::FunctionTemplate::builder_raw(reflect_set)
        .data(data.into())
        .length(1)
        .build(scope);
    let sname = v8::String::new(scope, &format!("set {member}")).unwrap();
    s_t.set_class_name(sname);
    let Some(g) = g_t.get_function(scope) else {
        fail!();
    };
    let Some(s) = s_t.get_function(scope) else {
        fail!();
    };
    let desc = v8::Object::new(scope);
    let gk = v8::String::new(scope, "get").unwrap();
    desc.set(scope, gk.into(), g.into());
    let sk = v8::String::new(scope, "set").unwrap();
    desc.set(scope, sk.into(), s.into());
    let ek = v8::String::new(scope, "enumerable").unwrap();
    desc.set(scope, ek.into(), v8::Boolean::new(scope, true).into());
    let ck = v8::String::new(scope, "configurable").unwrap();
    desc.set(scope, ck.into(), v8::Boolean::new(scope, true).into());
    let global = scope.get_current_context().global(scope);
    let obj_key = v8::String::new(scope, "Object").unwrap();
    let dp_key = v8::String::new(scope, "defineProperty").unwrap();
    let define = global
        .get(scope, obj_key.into())
        .and_then(|o| Local::<v8::Object>::try_from(o).ok())
        .and_then(|o| o.get(scope, dp_key.into()))
        .and_then(|f| Local::<v8::Function>::try_from(f).ok());
    let Some(define) = define else {
        fail!();
    };
    let name_key = v8::String::new(scope, &member).unwrap();
    let argv = [proto.into(), name_key.into(), desc.into()];
    if define.call(scope, global.into(), &argv).is_some() {
        rv.set(v8::Boolean::new(scope, true).into());
    } else {
        rv.set(v8::Boolean::new(scope, false).into());
    }
}

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [
        reflect_get as Cb,
        reflect_set as Cb,
        reflect_install_one as Cb,
    ]
    .into_iter()
    .map(|f| v8::ExternalReference { function: f })
    .collect()
}

/// Install the `__diting_reflect_one` global — runs in install_faces ahead
/// of the platform bundle, like `__diting_spawn`.
pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    let f = v8::Function::new_raw(scope, reflect_install_one).unwrap();
    let name = v8::String::new(scope, "__diting_reflect_one").unwrap();
    // hidden like the spawn entry: op names never become enumerable window
    // properties (the fingerprint face)
    global.define_own_property(scope, name.into(), f.into(), PropertyAttribute::DONT_ENUM);
}
