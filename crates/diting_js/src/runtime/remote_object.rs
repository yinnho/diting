//! Chrome RemoteObject wire shapes: number spelling, value boxing, and
//! the info_from_json/meta constructors. Pure associated fns — no state.

use super::RemoteObjectInfo;

impl super::JsRuntime {
    /// Chrome spells a JS number via Number→String: integral doubles lose
    /// the fraction ("2", not "2.0"). v8_to_json boxes every JS number as an
    /// f64 serde Number, whose Display keeps float-ness, and JS clients only
    /// normalize that away for `value` (JSON.parse), never for the
    /// `description` string (obscura#541 probe follow-up, same class as the
    /// #576 integer coordinates).
    pub(super) fn chrome_number_string(n: &serde_json::Number) -> String {
        match n.as_f64() {
            Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < i64::MAX as f64 => {
                format!("{}", f as i64)
            }
            _ => n.to_string(),
        }
    }

    /// Integral f64 numbers serialize without the trailing ".0" so
    /// `returnByValue` payloads match Chrome's wire form for non-JS clients
    /// too (serde_json would print 2.0).
    pub(super) fn chrome_number_value(n: &serde_json::Number) -> serde_json::Value {
        if let Some(f) = n.as_f64() {
            if f.is_finite() && f.fract() == 0.0 && f.abs() < i64::MAX as f64 {
                return serde_json::Value::Number(serde_json::Number::from(f as i64));
            }
        }
        serde_json::Value::Number(n.clone())
    }

    pub(super) fn info_from_json(value: &serde_json::Value) -> RemoteObjectInfo {
        match value {
            serde_json::Value::Null => RemoteObjectInfo {
                js_type: "object".into(),
                subtype: Some("null".into()),
                class_name: String::new(),
                description: "null".into(),
                object_id: None,
                value: Some(serde_json::Value::Null),
            },
            serde_json::Value::Bool(b) => RemoteObjectInfo {
                js_type: "boolean".into(),
                subtype: None,
                class_name: String::new(),
                description: b.to_string(),
                object_id: None,
                value: Some(value.clone()),
            },
            serde_json::Value::Number(n) => RemoteObjectInfo {
                js_type: "number".into(),
                subtype: None,
                class_name: String::new(),
                description: Self::chrome_number_string(n),
                object_id: None,
                value: Some(Self::chrome_number_value(n)),
            },
            serde_json::Value::String(s) => RemoteObjectInfo {
                js_type: "string".into(),
                subtype: None,
                class_name: String::new(),
                description: s.clone(),
                object_id: None,
                value: Some(value.clone()),
            },
            serde_json::Value::Array(arr) => RemoteObjectInfo {
                js_type: "object".into(),
                subtype: Some("array".into()),
                class_name: "Array".into(),
                description: format!("Array({})", arr.len()),
                object_id: None,
                value: Some(value.clone()),
            },
            serde_json::Value::Object(_) => RemoteObjectInfo {
                js_type: "object".into(),
                subtype: None,
                class_name: "Object".into(),
                description: "Object".into(),
                object_id: None,
                value: Some(value.clone()),
            },
        }
    }

    /// Chrome's RemoteObject for JS `undefined` is `{type:"undefined"}` —
    /// no value, no subtype (obscura#779).
    pub(super) fn undefined_info() -> RemoteObjectInfo {
        RemoteObjectInfo {
            js_type: "undefined".into(),
            subtype: None,
            class_name: String::new(),
            description: String::new(),
            object_id: None,
            value: None,
        }
    }

    pub(super) fn info_from_meta(
        meta: &serde_json::Value,
        object_id: Option<String>,
    ) -> RemoteObjectInfo {
        let js_type = meta
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("undefined")
            .to_string();
        let subtype = meta
            .get("subtype")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let class_name = meta
            .get("className")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = meta
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Chrome's RemoteObject for a primitive carries the real JSON value
        // (which meta_extract_js put in `value` for number/boolean/string),
        // and a handle (objectId) only for objects and functions — a number
        // or `undefined` result is never something the client can call into
        // (obscura#779).
        let value = match js_type.as_str() {
            "number" | "boolean" | "string" => meta.get("value").cloned(),
            _ => None,
        };
        let object_id = match js_type.as_str() {
            "object" | "function" => object_id,
            _ => None,
        };

        RemoteObjectInfo {
            js_type,
            subtype,
            class_name,
            description,
            object_id,
            value,
        }
    }
}
