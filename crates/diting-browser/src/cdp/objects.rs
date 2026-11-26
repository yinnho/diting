//! `objectId` as it arrives on the wire — the face's half of the handle
//! contract. The engine mints and holds handles; this module reads
//! Chrome's spellings of them (strict JSON first, then the lenient
//! number reader for what is no JSON at all) and asks the engine about
//! the realm-and-number pair it finds. Nothing here reaches into the
//! page: the engine's own spelling of an id never crosses this boundary.

use crate::session::HandleParts;

/// What Chrome's canonical spelling of a handle is — the strict path
/// accepts a parsed id only when it was spelled exactly this way.
fn chrome_oid(realm: i64, number: i64) -> String {
    format!("{{\"injectedScriptId\":{realm},\"id\":{number}}}")
}

/// Which of the four an id is ([`ObjectIdState`]), read the way Chrome

/// The two parts of an id and whether it was the strict spelling.
/// `Err(())` when the id is no id at all. A part reads `None` when it
/// was left empty — `"id":""` — which is a different thing from no part.
fn parts(spelled: &str) -> Result<(Option<i64>, Option<i64>, bool), ()> {
    // Strict first: the whole spelling as one JSON object.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(spelled) {
        let part = |field: &str| match value.get(field) {
            Some(serde_json::Value::String(empty)) if empty.is_empty() => Ok(None),
            Some(other) => other.as_i64().map(Some).ok_or(()),
            None => Err(()),
        };
        if let (Ok(realm), Ok(number)) = (part("injectedScriptId"), part("id")) {
            return Ok((realm, number, true));
        }
    }
    // No strict JSON: Chrome's number reader still takes each part by
    // itself — `"id":""2` reads an empty id, not no id.
    lenient_parts(spelled)
        .map(|(realm, number)| (realm, number, false))
        .ok_or(())
}

/// A wire id read once into the IR's [`HandleParts`] — for commands that
/// cross the session boundary carrying ids the thread will have to ask
/// the engine about. The spelling itself never leaves the face.
pub(super) fn handle_parts(object_id: &str) -> HandleParts {
    let spelled = object_id.trim_start();
    match parts(spelled) {
        Err(()) => HandleParts::NoId,
        Ok((realm, number, strict)) => {
            // The canonical check only matters once the realm matches and
            // the number exists — both decided on the session thread — so
            // it is reported for every strict spelling and ordered there.
            let canonical = match (strict, realm, number) {
                (true, Some(realm), Some(number)) => spelled == chrome_oid(realm, number),
                _ => true,
            };
            HandleParts::Parts {
                realm,
                number,
                canonical,
            }
        }
    }
}

/// The two parts of an id that is no JSON, read the way Chrome's number
/// reader takes them: each part on its own, a part that is no integer
/// making the whole no id, one left empty reading as none. `None` when
/// a part cannot be found at all.
fn lenient_parts(spelled: &str) -> Option<(Option<i64>, Option<i64>)> {
    // What follows the closing brace is no id at all.
    if !spelled.trim_end().ends_with('}') {
        return None;
    }
    fn part(text: &str) -> Option<Result<Option<i64>, ()>> {
        let text = text.trim_start().strip_prefix(':')?.trim_start();
        if let Some(inner) = text.strip_prefix('"') {
            let end = inner.find('"')?;
            return Some(if inner[..end].is_empty() {
                Ok(None)
            } else {
                Err(())
            });
        }
        // Chrome's number reader: a sign and digits — leading blanks
        // and zeroes aside — and the field's own end right after them.
        let bytes = text.as_bytes();
        let mut at = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
        let digits = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if at == digits || !matches!(bytes.get(at), None | Some(b',') | Some(b'}')) {
            return Some(Err(()));
        }
        Some(text[..at].parse::<i64>().map(Some).map_err(|_| ()))
    }
    let realm_at = spelled.find("\"injectedScriptId\"")?;
    let realm = part(&spelled[realm_at + "\"injectedScriptId\"".len()..])?.ok()?;
    let id_at = spelled[realm_at..].find("\"id\"")? + realm_at;
    let number = part(&spelled[id_at + "\"id\"".len()..])?.ok()?;
    Some((realm, number))
}
