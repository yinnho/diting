//! How the bridge reads a command's parameters: the way Chrome reads them
//! (Chrome 158, measured with raw-CDP probes). Chrome reads every declared
//! field before it looks at any value, and a field of the wrong kind — a
//! `null` too, which is not a field left out — is `-32602` "Invalid
//! parameters" whatever else is wrong with the call. An integer is a JSON
//! number that is whole and fits an `int32_t`: `100.0` and `1e2` pass,
//! `100.5` and `1e30` do not. A field Chrome does not declare is ignored.
//!
//! So a command reads all of its fields first — the ones it uses and the
//! ones it only declares ([`kinds`]) — and only then looks at values, and
//! refuses what the engine cannot apply after that.
//!
//! Every reader here is also the one door for a `params` that is not an
//! object: a `null` is as good as left out, anything else is "Invalid
//! parameters". A command that reads no parameter at all takes any. Chrome
//! is uneven about that last case (158, measured): `Page.reload`,
//! `Network.enable` and `Target.getTargetInfo` refuse `[]` — as they do here,
//! they read a field — and `Page.enable`, `DOM.enable` and `Target.getTargets`
//! refuse it too, where here they take it; `Page.disable`, `Runtime.enable`
//! and `CSS.enable` take it. Chrome also adds to every such error a `data`
//! note of its own deserializer, with a byte position in its CBOR copy of
//! the frame: that is not copied.
use serde_json::Value;

use super::Failure;

/// A value Chrome refuses to read, in the words of its check.
pub(super) fn invalid(message: &str) -> Failure {
    Failure::Invalid(message.to_string())
}

pub(super) fn invalid_params() -> Failure {
    invalid("Invalid parameters")
}

/// A field Chrome declares as a string. A `null` is a wrong kind, not a
/// field left out.
pub(super) fn string<'a>(params: &'a Value, field: &str) -> Result<&'a str, Failure> {
    optional(params, field, Value::as_str)?.ok_or_else(invalid_params)
}

pub(super) fn optional_string<'a>(
    params: &'a Value,
    field: &str,
) -> Result<Option<&'a str>, Failure> {
    optional(params, field, Value::as_str)
}

pub(super) fn boolean(params: &Value, field: &str) -> Result<bool, Failure> {
    optional(params, field, Value::as_bool)?.ok_or_else(invalid_params)
}

pub(super) fn optional_boolean(params: &Value, field: &str) -> Result<Option<bool>, Failure> {
    optional(params, field, Value::as_bool)
}

pub(super) fn integer(params: &Value, field: &str) -> Result<i64, Failure> {
    optional(params, field, int32)?.ok_or_else(invalid_params)
}

pub(super) fn optional_integer(params: &Value, field: &str) -> Result<Option<i64>, Failure> {
    optional(params, field, int32)
}

pub(super) fn number(params: &Value, field: &str) -> Result<f64, Failure> {
    optional(params, field, Value::as_f64)?.ok_or_else(invalid_params)
}

pub(super) fn optional_number(params: &Value, field: &str) -> Result<Option<f64>, Failure> {
    optional(params, field, Value::as_f64)
}

/// An array of strings, whole: one entry of another kind and the field is.
pub(super) fn texts<'a>(params: &'a Value, field: &str) -> Result<Option<Vec<&'a str>>, Failure> {
    optional(params, field, |value| {
        value.as_array()?.iter().map(|item| item.as_str()).collect()
    })
}

/// A field Chrome declares as an object, whole, for whoever reads inside
/// it: the bridge does not declare what is in it.
pub(super) fn object<'a>(params: &'a Value, field: &str) -> Result<Option<&'a Value>, Failure> {
    optional(params, field, as_object)
}

fn as_object(value: &Value) -> Option<&Value> {
    value.is_object().then_some(value)
}

/// The kind of a field a command declares and the bridge does not read.
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Bool,
    Integer,
    Number,
    Text,
    /// An array of strings.
    Texts,
    /// An array of objects, whatever is in them.
    Objects,
}

/// Read every field in `fields` for its kind — and nothing else: none of
/// them is used.
pub(super) fn kinds(params: &Value, fields: &[(&str, Kind)]) -> Result<(), Failure> {
    if params.is_null() {
        return Ok(());
    }
    if !params.is_object() {
        return Err(invalid_params());
    }
    for (field, kind) in fields {
        let Some(value) = params.get(field) else {
            continue;
        };
        let is = match kind {
            Kind::Bool => value.is_boolean(),
            Kind::Integer => int32(value).is_some(),
            Kind::Number => value.is_number(),
            Kind::Text => value.is_string(),
            Kind::Texts => value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string)),
            Kind::Objects => value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_object)),
        };
        if !is {
            return Err(invalid_params());
        }
    }
    Ok(())
}

/// The field under `name`, read through `of`: `None` when it was left out.
/// No reader takes a `null` for a field, so one is a wrong kind.
pub(super) fn optional<'a, T>(
    params: &'a Value,
    field: &str,
    of: fn(&'a Value) -> Option<T>,
) -> Result<Option<T>, Failure> {
    if params.is_null() {
        return Ok(None);
    }
    if !params.is_object() {
        return Err(invalid_params());
    }
    match params.get(field) {
        None => Ok(None),
        Some(value) => of(value).map(Some).ok_or_else(invalid_params),
    }
}

/// A JSON number that is whole and fits an `int32_t`.
pub(super) fn int32(value: &Value) -> Option<i64> {
    value
        .as_f64()
        .filter(|number| {
            number.fract() == 0.0 && (-2_147_483_648.0..=2_147_483_647.0).contains(number)
        })
        .map(|number| number as i64)
}
