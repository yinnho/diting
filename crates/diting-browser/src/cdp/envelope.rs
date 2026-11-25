//! How the bridge reads a frame off the socket: the way Chrome reads it
//! (Chrome 158, measured with raw-CDP probes). A frame that is not JSON is
//! `-32700`, one that is no object is `-32600` `Message must be an object`.
//! Then its keys are checked in the order they were written, and the first
//! that is wrong answers; the rest are not looked at:
//!
//! | Key | What is wrong with it | Answer (`-32600`) |
//! |---|---|---|
//! | `id` | not a whole number that fits an `int32_t` | `Message must have integer 'id' property` |
//! | `sessionId` | not a string | `Message may have string 'sessionId' property` |
//! | `method` | not a string | `Message must have string 'method' property` |
//! | `params` | none of object, array, `null` | `Message may have object 'params' property` |
//! | any other | there | `Message has property other than 'id', 'method', 'sessionId', 'params'` |
//!
//! The answer carries the `id` when the walk had already read it: a wrong
//! key written before the `id` sends the answer back without one, and so
//! does a wrong `id`. An empty `method` passes the walk and is named once
//! every key was found right; an empty `sessionId` names the root level, as
//! none does.
//!
//! After the walk a frame with no `id` at all is answered `Message must
//! have integer 'id' property`, without one. It is not a notification here,
//! as it is not in Chrome: it is answered. An `id` comes back spelled as a
//! whole number (`2.0` as `2`).
//!
//! A frame that is not JSON is answered in this bridge's own words.
//! Chrome's are its parser's (`JSON: invalid token at position 0` and the
//! like), and no client reads them. Chrome's parser also refuses a key
//! written twice (`-32700`); this one keeps the last and answers the frame.

use std::fmt;

use serde::de::{Deserializer as _, MapAccess};
use serde_json::{json, Value};

use super::{params, Out};

/// A frame as read.
pub(super) struct Frame {
    pub(super) id: Value,
    pub(super) session: Option<String>,
    pub(super) method: String,
    /// `{}` when left out. A `null` or an array is passed on: a command
    /// that reads a parameter refuses it, one that reads none takes it.
    pub(super) params: Value,
}

const INVALID_REQUEST: i64 = -32600;

pub(super) fn read(text: &str) -> Result<Frame, Out> {
    let refused = |id: Option<&Value>, message: &str| Out::from_frames(vec![refusal(id, message)]);
    let entries = entries(text).map_err(|message| match message {
        Some(message) => refused(None, message),
        None => not_json(),
    })?;
    let mut id = None;
    let mut session = None;
    let mut method = None;
    let mut params = None;
    for (key, value) in entries {
        match key.as_str() {
            "id" => match params::int32(&value) {
                Some(number) => id = Some(Value::from(number)),
                None => {
                    return Err(refused(None, "Message must have integer 'id' property"));
                }
            },
            "sessionId" => {
                let Value::String(named) = value else {
                    return Err(refused(
                        id.as_ref(),
                        "Message may have string 'sessionId' property",
                    ));
                };
                session = (!named.is_empty()).then_some(named);
            }
            "method" => {
                if !value.is_string() {
                    return Err(refused(
                        id.as_ref(),
                        "Message must have string 'method' property",
                    ));
                }
                method = Some(value);
            }
            "params" => match value {
                Value::Object(_) | Value::Array(_) | Value::Null => params = Some(value),
                _ => {
                    return Err(refused(
                        id.as_ref(),
                        "Message may have object 'params' property",
                    ));
                }
            },
            _ => {
                return Err(refused(
                    id.as_ref(),
                    "Message has property other than 'id', 'method', 'sessionId', 'params'",
                ));
            }
        }
    }
    let Some(id) = id else {
        return Err(refused(None, "Message must have integer 'id' property"));
    };
    let Some(method) = method.and_then(|method| match method.as_str() {
        // An empty one is as good as none.
        Some("") | None => None,
        named => named.map(str::to_string),
    }) else {
        return Err(refused(
            Some(&id),
            "Message must have string 'method' property",
        ));
    };
    Ok(Frame {
        id,
        session,
        method,
        params: params.unwrap_or_else(|| json!({})),
    })
}

fn not_json() -> Out {
    Out::from_frames(vec![
        json!({ "error": { "code": -32700, "message": "Message is not valid JSON" } }).to_string(),
    ])
}

/// A `-32600`, with the `id` when the frame had a usable one — and with no
/// `sessionId`, as Chrome's.
fn refusal(id: Option<&Value>, message: &str) -> String {
    match id {
        Some(id) => json!({
            "id": id,
            "error": { "code": INVALID_REQUEST, "message": message },
        })
        .to_string(),
        None => json!({ "error": { "code": INVALID_REQUEST, "message": message } }).to_string(),
    }
}

/// The keys and values of the frame, in the order they were written: that
/// is the order they are checked in. `Err(None)` for what is not JSON,
/// `Err(Some(message))` for what is no object.
fn entries(text: &str) -> Result<Vec<(String, Value)>, Option<&'static str>> {
    let mut parser = serde_json::Deserializer::from_str(text);
    let entries = parser
        .deserialize_any(Written(Vec::new()))
        .map_err(|error| match error.classify() {
            serde_json::error::Category::Data => Some("Message must be an object"),
            _ => None,
        })?;
    parser.end().map_err(|_| None)?;
    Ok(entries.0)
}

/// A JSON object as a list of its entries. [`serde_json::Map`] would sort
/// them, or keep the order only under a feature some other crate decides.
struct Written(Vec<(String, Value)>);

impl<'de> serde::de::Visitor<'de> for Written {
    type Value = Written;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Written, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(Written(entries))
    }
}
