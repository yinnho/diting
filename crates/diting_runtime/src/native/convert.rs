//! WebIDL value conversions, Rust side — transliterated from the JS layer
//! (crates/diting_js/js/platform/core.js `conv`/`dict`), whose semantics
//! are pinned by the Chrome truth probes. Same wording, same ordering,
//! same getter side effects. Every function returns `None` with the
//! exception already pending on failure.
//!
//! Error shape law (chrome-error-truth.json, mirrored in core.js): the
//! TypeError is constructed with the UNDECORATED message; `message` is then
//! redefined as an ENUMERABLE data property carrying "<prefix>: <message>".
//! (captureStackTrace's frame trimming has no native analog — the
//! synchronous native callback is already the frame boundary.)

use v8::Local;

/// Throw a Chrome-shaped TypeError: `message` decorated as
/// "<prefix>: <message>" in an enumerable own property.
pub(crate) fn fail(scope: &mut v8::PinScope<'_, '_>, message: &str, prefix: Option<&str>) {
    let m = v8::String::new(scope, message).unwrap();
    let e = v8::Exception::type_error(scope, m);
    if let Some(prefix) = prefix {
        if let Ok(obj) = Local::<v8::Object>::try_from(e) {
            let decorated = if prefix.is_empty() {
                message.to_string()
            } else {
                format!("{prefix}: {message}")
            };
            let key = v8::String::new(scope, "message").unwrap();
            let val = v8::String::new(scope, &decorated).unwrap();
            // enumerable: Blink re-sets message after construction.
            obj.define_own_property(scope, key.into(), val.into(), v8::PropertyAttribute::NONE);
        }
    }
    scope.throw_exception(e);
}

pub(crate) fn illegal(scope: &mut v8::PinScope<'_, '_>) {
    fail(scope, "Illegal invocation", None);
}

fn symbol_guard(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    kind: &str,
    prefix: &str,
) -> bool {
    if v.is_symbol() {
        fail(
            scope,
            &format!("Cannot convert a Symbol value to a {kind}"),
            Some(prefix),
        );
        return true;
    }
    false
}

/// DOMString: Symbol throws; everything else is String(v).
pub(crate) fn dom_string(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    prefix: &str,
) -> Option<String> {
    if symbol_guard(scope, v, "string", prefix) {
        return None;
    }
    let s = v.to_string(scope)?;
    Some(s.to_rust_string_lossy(scope))
}

pub(crate) fn boolean(scope: &mut v8::PinScope<'_, '_>, v: Local<v8::Value>) -> bool {
    v.boolean_value(scope)
}

/// unsigned long: non-finite -> 0, then truncate and mod 2^32.
// allow: no consumer until the first family with an unsigned-long member
// lands (geometry is all-unrestricted-double); the conversion law is
// pinned here once for every future interface.
#[allow(dead_code)]
pub(crate) fn unsigned_long(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    prefix: &str,
) -> Option<f64> {
    if symbol_guard(scope, v, "number", prefix) {
        return None;
    }
    let n = v.number_value(scope)?;
    if !n.is_finite() {
        return Some(0.0);
    }
    let t = n.trunc();
    Some(t.rem_euclid(4294967296.0))
}

/// unrestricted double: NaN/Infinity pass through.
pub(crate) fn unrestricted_double(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    prefix: &str,
) -> Option<f64> {
    if symbol_guard(scope, v, "number", prefix) {
        return None;
    }
    v.number_value(scope)
}

/// restricted double: non-finite is a TypeError.
// allow: same — first consumer lands outside the geometry family.
#[allow(dead_code)]
pub(crate) fn double(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    prefix: &str,
) -> Option<f64> {
    let n = unrestricted_double(scope, v, prefix)?;
    if !n.is_finite() {
        fail(
            scope,
            "The provided double value is non-finite.",
            Some(prefix),
        );
        return None;
    }
    Some(n)
}

/// sequence<T> as a Vec of raw values: the caller converts items. The
/// conversion-of-container semantics live here (@@iterator required,
/// iteration exceptions propagate); per-item conversion stays with the
/// caller because T varies.
pub(crate) fn sequence<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    v: Local<v8::Value>,
    prefix: &str,
) -> Option<Vec<Local<'s, v8::Value>>> {
    let obj = if (v.is_object() || v.is_function()) && !v.is_null() {
        Local::<v8::Object>::try_from(v).ok()
    } else {
        None
    };
    let Some(obj) = obj else {
        fail(
            scope,
            "The provided value cannot be converted to a sequence.",
            Some(prefix),
        );
        return None;
    };
    let sym = v8::Symbol::get_iterator(scope);
    let Some(iter_fn) = obj.get(scope, sym.into()) else {
        return None; // getter threw — propagates
    };
    if !iter_fn.is_function() {
        fail(
            scope,
            "The provided value cannot be converted to a sequence.",
            Some(prefix),
        );
        return None;
    }
    let iter_fn = Local::<v8::Function>::try_from(iter_fn).ok()?;
    let recv: Local<v8::Value> = obj.into();
    let iter = iter_fn.call(scope, recv, &[])?;
    let iter = Local::<v8::Object>::try_from(iter).ok()?;
    let next_key = v8::String::new(scope, "next").unwrap();
    let next = iter.get(scope, next_key.into())?;
    let next = Local::<v8::Function>::try_from(next).ok()?;
    let value_key = v8::String::new(scope, "value").unwrap();
    let done_key = v8::String::new(scope, "done").unwrap();
    let iter_recv: Local<v8::Value> = iter.into();
    let mut out = Vec::new();
    loop {
        let step = next.call(scope, iter_recv, &[])?;
        let step = Local::<v8::Object>::try_from(step).ok()?;
        let done = step.get(scope, done_key.into())?;
        if done.boolean_value(scope) {
            break;
        }
        let item = step.get(scope, value_key.into())?;
        out.push(item);
    }
    Some(out)
}

/// One dictionary member: read property `name` as `kind`.
// allow: generic dict reader — DOMMatrix reads its init dict by hand
// (the double-read detection pass doesn't fit the generic shape); the
// generic path is for the simpler families that follow.
#[allow(dead_code)]
pub(crate) struct DictMember {
    pub name: &'static str,
    pub kind: DictKind,
    pub required: bool,
}

// allow: see DictMember.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum DictKind {
    Double,
    UnrestrictedDouble,
    DomString,
    UnsignedLong,
    Boolean,
}

// allow: see DictMember.
#[allow(dead_code)]
pub(crate) enum DictVal {
    Num(f64),
    Str(String),
    Bool(bool),
    /// Member absent and no default ran — WebIDL "not present".
    Absent,
}

// allow: see DictMember.
#[allow(dead_code)]
/// WebIDL dictionary expansion. Members are read in the GIVEN order (the
/// caller encodes Chrome's read order); reads go through plain property
/// access, so getter side effects and exceptions propagate exactly like
/// the spec (and the JS `dict()` this mirrors).
pub(crate) fn dict(
    scope: &mut v8::PinScope<'_, '_>,
    v: Local<v8::Value>,
    dict_name: &str,
    members: &[DictMember],
    prefix: &str,
) -> Option<Vec<DictVal>> {
    let mut out = Vec::with_capacity(members.len());
    if v.is_undefined() || v.is_null() {
        // The empty dictionary: every member absent.
        for m in members {
            if m.required {
                fail(scope, &format!("Failed to read the '{}' property from '{dict_name}': Required member is undefined.", m.name), Some(prefix));
                return None;
            }
            out.push(DictVal::Absent);
        }
        return Some(out);
    }
    let Ok(obj) = Local::<v8::Object>::try_from(v) else {
        fail(
            scope,
            &format!("The provided value is not of type '{dict_name}'."),
            Some(prefix),
        );
        return None;
    };
    for m in members {
        let key = v8::String::new(scope, m.name).unwrap();
        let raw = obj.get(scope, key.into())?;
        if raw.is_undefined() {
            if m.required {
                fail(scope, &format!("Failed to read the '{}' property from '{dict_name}': Required member is undefined.", m.name), Some(prefix));
                return None;
            }
            out.push(DictVal::Absent);
            continue;
        }
        let tail = format!(
            "{prefix}: Failed to read the '{}' property from '{dict_name}'",
            m.name
        );
        let val = match m.kind {
            DictKind::Double => double(scope, raw, &tail).map(DictVal::Num),
            DictKind::UnrestrictedDouble => {
                unrestricted_double(scope, raw, &tail).map(DictVal::Num)
            }
            DictKind::DomString => dom_string(scope, raw, &tail).map(DictVal::Str),
            DictKind::UnsignedLong => unsigned_long(scope, raw, &tail).map(DictVal::Num),
            DictKind::Boolean => Some(DictVal::Bool(boolean(scope, raw))),
        };
        {
            let val = val?;
            out.push(val)
        }
    }
    Some(out)
}
