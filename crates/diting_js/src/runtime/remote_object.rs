//! Chrome RemoteObject wire shapes: number spelling, value boxing, the
//! info_from_json/meta constructors, and an Error's stack as Chrome shows
//! it. Pure associated fns — no state.

/// Rich evaluate result: typed remote-object metadata plus optional JSON
/// `value`. HTTP/session callers usually read `value`; `object_id` /
/// `subtype` / `class_name` support DOM-handle style follow-ups without a
/// separate wire protocol.
#[cfg_attr(not(test), allow(dead_code))] // tests cross-check the type metadata
#[derive(Debug, Clone)]
pub struct RemoteObjectInfo {
    pub js_type: String,
    pub subtype: Option<String>,
    pub class_name: String,
    /// What Chrome writes beside a result ("Object" / "Array(3)" / the
    /// source of a function …). An Error's is its message line and then
    /// the frames of its stack.
    pub description: String,
    pub object_id: Option<String>,
    pub value: Option<serde_json::Value>,
    /// `unserializableValue`: a number JSON has no spelling for (`NaN`,
    /// `Infinity`, `-0`), or a bigint spelled with its `n`. Set by
    /// [`Self::info_from_meta`] only — a value read through JSON has
    /// already lost what it would say.
    pub unserializable_value: Option<String>,
}

/// One frame of an error's stack as Chrome writes it after `at `: `inner
/// (<anonymous>:1:38)`, `pageThrows (<script>:1:30)`. [`page_frames`]
/// builds these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    /// The frame whole: the function's name when it has one, and where it
    /// is.
    pub text: String,
    /// The script the frame is in, and its 1-based line and column. A
    /// builtin without a position (`new Promise (<anonymous>)`) has none of
    /// the three: empty and 0.
    pub script: String,
    pub line: u32,
    pub column: u32,
}

/// The page's own frames of a stack V8 wrote in this engine, innermost
/// first — the engine's own left out, the way Chrome shows a stack. V8
/// writes the first line as the message; the rest are frames.
///
/// - Code an engine script compiled by `eval` — the script an eval ran, a
///   `callFunctionOn` declaration — is shown as Chrome shows it, a script of
///   its own: Chrome compiles both as scripts, so V8 wraps none of their
///   frames in an `eval at` origin and names no function of theirs `eval`.
///   Here they were compiled by a function of one of the engine's wrapper
///   scripts (`<eval-remote>`, `<callFnRemote>` …), and V8 says so in both
///   ways — that origin is what tells them apart.
/// - Code the page compiled itself, and the page's `<script>`s, stay as V8
///   wrote them.
/// - The engine's own scripts are left out: the wrappers its Rust side runs
///   and the event loop and DOM it writes in JS, which V8 names
///   `<anonymous>`. A builtin without a position stays (`new Promise
///   (<anonymous>)`), as it does in Chrome — except the `eval` that runs an
///   evaluated script.
pub fn page_frames(stack: &str) -> Vec<StackFrame> {
    stack.lines().skip(1).filter_map(frame).collect()
}

/// `place` is `script:line:column`, 1-based both.
fn position(place: &str) -> Option<(String, u32, u32)> {
    let mut parts = place.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    Some((parts.next()?.to_string(), line, column))
}

/// Whether a frame of that script is the page's. The engine runs its own
/// scripts and the page's inline ones under names in angle brackets
/// (`<anonymous>`, `<eval-remote>`), and the page's inline ones as
/// `<script>`; a page's external script is named by its URL.
fn page_script(script: &str) -> bool {
    script == "<script>" || !script.starts_with('<')
}

/// A frame's function name without the `async ` V8 puts in front of it.
fn split_async(text: &str) -> (&str, &str) {
    text.strip_prefix("async ")
        .map_or(("", text), |name| ("async ", name))
}

/// The name to show for a function of a script the engine compiled, from
/// what V8 calls it: `None` when Chrome would show none.
///
/// V8 names a frame by the function's declared or inferred name, and a
/// nameless one `eval` when the frame is of evaluated code
/// (`call-site-info.cc`, `GetFunctionName`) — in Chrome, whose script is
/// not evaluated, that one has no name, or the method it was called as
/// (`Object.eval [as then]` → `Object.then`). A name other than `eval` is
/// what Chrome shows too, as V8 wrote it.
fn compiled_name(name: &str) -> Option<String> {
    let (base, method) = match name.split_once(" [as ") {
        Some((base, method)) => (base, Some(method.trim_end_matches(']'))),
        None => (name, None),
    };
    // `TypeName.name`; what precedes the name is its dot.
    let (owner, last) = match base.rsplit_once('.') {
        Some((owner, last)) => (format!("{owner}."), last),
        None => (String::new(), base),
    };
    if last != "eval" {
        return Some(name.to_string());
    }
    match (method, owner.is_empty()) {
        (Some(method), _) => Some(format!("{owner}{method}")),
        (None, true) => None,
        (None, false) => Some(format!("{owner}<anonymous>")),
    }
}

fn frame(line: &str) -> Option<StackFrame> {
    let rest = line.trim().strip_prefix("at ")?;
    // `name (eval at <who called eval> (<their script:line:column>), place)`.
    if let Some((head, body)) = rest.split_once(" (eval at ") {
        let body = body.strip_suffix(')')?;
        let (origin, place) = body.rsplit_once(", ")?;
        let (script, line, column) = position(place)?;
        let (async_, name) = split_async(head);
        // The innermost bracketed name is the script of whoever called
        // `eval`: one of the engine's wrappers, or the page's code.
        let caller = origin.rsplit_once(" (").map_or(origin, |(_, base)| base);
        let caller_script = caller.rsplit_once(':').map_or(caller, |(script, _)| script);
        let mut text = rest.to_string();
        if !page_script(caller_script) {
            // The engine's trampoline compiled it — unless the origin says
            // the caller was itself evaluated, which is the page's own `eval`
            // inside what the engine compiled.
            let nested = origin.contains("(eval at ");
            text = match compiled_name(name) {
                Some(name) => format!("{async_}{name} ({place})"),
                None if nested && name == "eval" => format!("eval ({place})"),
                None => format!("{async_}{place}"),
            };
        }
        return Some(StackFrame {
            text,
            script,
            line,
            column,
        });
    }
    // `name (script:line:column)`, `script:line:column` — or a builtin. An
    // `async ` is in front of the name, or of the place when there is none.
    let (_, plain) = split_async(rest);
    let (name, tail) = match plain.split_once(" (") {
        Some((name, tail)) => (name, tail.strip_suffix(')')?),
        None => ("", plain),
    };
    let Some((script, line, column)) = position(tail) else {
        let builtin = tail == "<anonymous>" && name != "eval";
        return builtin.then(|| StackFrame {
            text: rest.to_string(),
            script: String::new(),
            line: 0,
            column: 0,
        });
    };
    page_script(&script).then(|| StackFrame {
        text: rest.to_string(),
        script,
        line,
        column,
    })
}

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
                unserializable_value: None,
                value: Some(serde_json::Value::Null),
            },
            serde_json::Value::Bool(b) => RemoteObjectInfo {
                js_type: "boolean".into(),
                subtype: None,
                class_name: String::new(),
                description: b.to_string(),
                object_id: None,
                unserializable_value: None,
                value: Some(value.clone()),
            },
            serde_json::Value::Number(n) => RemoteObjectInfo {
                js_type: "number".into(),
                subtype: None,
                class_name: String::new(),
                description: Self::chrome_number_string(n),
                object_id: None,
                unserializable_value: None,
                value: Some(Self::chrome_number_value(n)),
            },
            serde_json::Value::String(s) => RemoteObjectInfo {
                js_type: "string".into(),
                subtype: None,
                class_name: String::new(),
                description: s.clone(),
                object_id: None,
                unserializable_value: None,
                value: Some(value.clone()),
            },
            serde_json::Value::Array(arr) => RemoteObjectInfo {
                js_type: "object".into(),
                subtype: Some("array".into()),
                class_name: "Array".into(),
                description: format!("Array({})", arr.len()),
                object_id: None,
                unserializable_value: None,
                value: Some(value.clone()),
            },
            serde_json::Value::Object(_) => RemoteObjectInfo {
                js_type: "object".into(),
                subtype: None,
                class_name: "Object".into(),
                description: "Object".into(),
                object_id: None,
                unserializable_value: None,
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
            unserializable_value: None,
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
        let mut description = meta
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if subtype.as_deref() == Some("error") {
            description = stack_description(
                &description,
                meta.get("stack").and_then(|v| v.as_str()).unwrap_or(""),
            );
        }

        let unserializable_value = meta
            .get("unserializableValue")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Chrome's RemoteObject for a primitive carries the real JSON value
        // (which meta_extract_js put in `value` for number/boolean/string),
        // and `null` comes as a value too. A handle (objectId) is given only
        // to an object, a function or a symbol — a number or `undefined`
        // result is never something the client can call into (obscura#779),
        // and neither is `null`.
        let value = match (js_type.as_str(), subtype.as_deref()) {
            ("number" | "boolean" | "string", _) => meta.get("value").cloned(),
            ("object", Some("null")) => Some(serde_json::Value::Null),
            _ => None,
        };
        let object_id = match (js_type.as_str(), subtype.as_deref()) {
            ("object", Some("null")) => None,
            ("object" | "function" | "symbol", _) => object_id,
            _ => None,
        };

        RemoteObjectInfo {
            js_type,
            subtype,
            class_name,
            description,
            object_id,
            value,
            unserializable_value,
        }
    }
}

/// An Error's description as Chrome writes it: its message line, then the
/// page's own frames of its stack. `message` is `name: message` as the
/// error says it now; a stack that is not V8's contributes frames, and
/// nothing else.
fn stack_description(message: &str, stack: &str) -> String {
    let mut description = message.to_string();
    for frame in page_frames(stack) {
        description.push_str("\n    at ");
        description.push_str(&frame.text);
    }
    description
}
