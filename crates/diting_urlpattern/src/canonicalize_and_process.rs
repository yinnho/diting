// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// NOTE to all: the code in this crate sometimes slightly diverges from the
// precise wording of the spec, because rust-url does not expose all the
// routines exactly as the spec wants. The end behaviour should be identical.

use crate::Error;

// https://wicg.github.io/urlpattern/#canon-encoding-callbacks

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-protocol
pub fn canonicalize_protocol(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    // diting: liburlpattern validates the scheme shape directly — rust-url's
    // parser error wording/behavior is not Chrome's (measured)
    let valid = {
        let mut cs = value.chars();
        match cs.next() {
            Some(c) if c.is_ascii_alphabetic() => {
                cs.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
            }
            _ => false,
        }
    };
    if !valid {
        return Err(Error::Message(format!("Invalid protocol '{value}'.")));
    }
    Ok(value.to_ascii_lowercase())
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-username
pub fn canonicalize_username(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let mut url = url::Url::parse("http://dummy.test").unwrap();
    // Note: unwrap is safe, because this is a HTTP url that supports username.
    url.set_username(value).unwrap();
    Ok(url.username().to_string())
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-password
pub fn canonicalize_password(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let mut url = url::Url::parse("http://dummy.test").unwrap();
    // Note: unwrap is safe, because this is a HTTP url that supports password.
    url.set_password(Some(value)).unwrap();
    Ok(url.password().unwrap().to_string())
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-hostname
pub fn canonicalize_hostname(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    // diting: the host parser stops at / ? # \ (state override) — the
    // canonicalizer keeps only that prefix (measured: 'a/b' -> 'a' is a VALID
    // hostname pattern). Validation is the dummy round-trip: a failed set
    // leaves the dummy host in place, which is what Chrome detects.
    let cut = value
        .split(|c| matches!(c, '/' | '?' | '#' | '\\'))
        .next()
        .unwrap_or(value);
    let mut url = url::Url::parse("http://dummy.test").unwrap();
    let set_ok = url.set_host(Some(cut)).is_ok();
    let host = url::quirks::hostname(&url).to_string();
    if cut.is_empty() || ((!set_ok || host == "dummy.test") && cut != "dummy.test") {
        return Err(Error::Message(format!("Invalid hostname '{value}'.")));
    }
    Ok(host)
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-an-ipv6-hostname
pub fn canonicalize_ipv6_hostname(value: &str) -> Result<String, Error> {
    let valid_ipv6 = value
        .chars()
        .all(|c| c.is_ascii_hexdigit() || matches!(c, '[' | ']' | ':'));
    if !valid_ipv6 {
        Err(Error::Message(format!("Invalid IPv6 hostname '{value}'.")))
    } else {
        Ok(value.to_ascii_lowercase())
    }
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-port
pub fn canonicalize_port(value: &str, mut protocol: Option<&str>) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if let Some("") = protocol {
        protocol = None;
    }
    // diting: liburlpattern ports parseInt semantics — the leading digit run
    // is the whole value ('1.5' -> 1 is a VALID port pattern, measured), then
    // the u16 range check, then re-serialization strips leading zeros
    let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
    let port: u64 = if digits.is_empty() {
        u64::MAX
    } else {
        digits.parse().unwrap_or(u64::MAX)
    };
    if port > 65535 {
        return Err(Error::Message(format!("Invalid port '{value}'.")));
    }
    // a special scheme's default port strips to empty (the url round-trip
    // did this implicitly before)
    let default = match protocol {
        Some("http") | Some("ws") => Some(80),
        Some("https") | Some("wss") => Some(443),
        Some("ftp") => Some(21),
        _ => None,
    };
    if default == Some(port) {
        return Ok(String::new());
    }
    Ok((port as u16).to_string())
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-pathname
pub fn canonicalize_pathname(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let leading_slash = value.starts_with('/');
    let modified_value = if !leading_slash {
        format!("/-{value}")
    } else {
        value.to_string()
    };
    let mut url = url::Url::parse("http://dummy.test").unwrap();
    url.set_path(&modified_value);
    let mut pathname = url::quirks::pathname(&url);
    if !leading_slash {
        pathname = &pathname[2..];
    }
    Ok(pathname.to_string())
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-an-opaque-pathname
pub fn canonicalize_an_opaque_pathname(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    // diting: rust-url's opaque-path setter encodes '/' as %2F, but the spec's
    // opaque-path canonicalization is the C0-control percent-encode set alone
    // (C0 controls, DEL and everything above 0x7E) — Chrome keeps '/' literal
    // and matches it (measured: 'foo://bar/:p' x 'foo://bar/baz'). Encode
    // directly instead of round-tripping through a data: URL.
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if (c as u32) < 0x20 || (c as u32) > 0x7E {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).as_bytes() {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-search
pub fn canonicalize_search(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let mut url = url::Url::parse("http://dummy.test").unwrap();
    url.set_query(Some(value));
    Ok(url.query().unwrap_or("").to_string())
}

// Ref: https://wicg.github.io/urlpattern/#canonicalize-a-search
pub fn canonicalize_hash(value: &str) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let mut url = url::Url::parse("http://dummy.test").unwrap();
    url.set_fragment(Some(value));
    Ok(url.fragment().unwrap_or("").to_string())
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProcessType {
    Pattern,
    Url,
}

// Ref: https://wicg.github.io/urlpattern/#process-protocol-for-init
pub fn process_protocol_init(value: &str, kind: &ProcessType) -> Result<String, Error> {
    let stripped_value = value.strip_suffix(':').unwrap_or(value);
    if kind == &ProcessType::Pattern {
        Ok(stripped_value.to_string())
    } else {
        canonicalize_protocol(stripped_value)
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-username-for-init
pub fn process_username_init(value: &str, kind: &ProcessType) -> Result<String, Error> {
    if kind == &ProcessType::Pattern {
        Ok(value.to_string())
    } else {
        canonicalize_username(value)
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-password-for-init
pub fn process_password_init(value: &str, kind: &ProcessType) -> Result<String, Error> {
    if kind == &ProcessType::Pattern {
        Ok(value.to_string())
    } else {
        canonicalize_password(value)
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-hostname-for-init
pub fn process_hostname_init(value: &str, kind: &ProcessType) -> Result<String, Error> {
    if kind == &ProcessType::Pattern {
        Ok(value.to_string())
    } else {
        canonicalize_hostname(value)
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-port-for-init
pub fn process_port_init(
    port_value: &str,
    protocol_value: Option<&str>,
    kind: &ProcessType,
) -> Result<String, Error> {
    if kind == &ProcessType::Pattern {
        Ok(port_value.to_string())
    } else {
        canonicalize_port(port_value, protocol_value)
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-pathname-for-init
pub fn process_pathname_init(
    pathname_value: &str,
    protocol_value: Option<&str>,
    kind: &ProcessType,
) -> Result<String, Error> {
    if kind == &ProcessType::Pattern {
        Ok(pathname_value.to_string())
    } else {
        match protocol_value {
            Some(protocol) if protocol.is_empty() || is_special_scheme(protocol) => {
                canonicalize_pathname(pathname_value)
            }
            _ => canonicalize_an_opaque_pathname(pathname_value),
        }
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-search-for-init
pub fn process_search_init(value: &str, kind: &ProcessType) -> Result<String, Error> {
    let stripped_value = if value.starts_with('?') {
        value.get(1..).unwrap()
    } else {
        value
    };
    if kind == &ProcessType::Pattern {
        Ok(stripped_value.to_string())
    } else {
        canonicalize_search(stripped_value)
    }
}

// Ref: https://wicg.github.io/urlpattern/#process-hash-for-init
pub fn process_hash_init(value: &str, kind: &ProcessType) -> Result<String, Error> {
    let stripped_value = if value.starts_with('#') {
        value.get(1..).unwrap()
    } else {
        value
    };
    if kind == &ProcessType::Pattern {
        Ok(stripped_value.to_string())
    } else {
        canonicalize_hash(stripped_value)
    }
}

pub fn is_special_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https" | "ws" | "wss" | "ftp" | "file")
}

pub fn special_scheme_default_port(scheme: &str) -> Option<&'static str> {
    match scheme {
        "http" => Some("80"),
        "https" => Some("443"),
        "ws" => Some("80"),
        "wss" => Some("443"),
        "ftp" => Some("21"),
        "file" => None,
        _ => None,
    }
}

// Ref: https://urlpattern.spec.whatwg.org/#process-a-base-url-string
pub fn process_base_url(input: &str, kind: &ProcessType) -> String {
    if kind != &ProcessType::Pattern {
        input.to_string()
    } else {
        escape_pattern_string(input)
    }
}

// Ref: https://wicg.github.io/urlpattern/#escape-a-pattern-string
pub fn escape_pattern_string(input: &str) -> String {
    assert!(input.is_ascii());
    let mut result = String::new();
    for char in input.chars() {
        if matches!(char, '+' | '*' | '?' | ':' | '{' | '}' | '(' | ')' | '\\') {
            result.push('\\');
        }
        result.push(char);
    }
    result
}
