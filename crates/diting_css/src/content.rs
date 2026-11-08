//! Generated `content` + counters/quotes parsing (god-file ratchet split
//! from mod.rs, pure move).

use super::*;

/// Counter style names accepted in `counter(name, style)`/`counters(...)`;
/// unknown names fail closed to `Decimal` (css-counter-styles fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CounterStyle {
    #[default]
    Decimal,
    DecimalLeadingZero,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

/// Render one counter value. Roman is only defined for 1..=3999 and alpha
/// for >= 1; everything else (zero, negative, overflow) falls back to the
/// decimal rendering, matching Chrome's fallback posture.
pub fn format_counter_value(v: i64, style: CounterStyle) -> String {
    match style {
        CounterStyle::Decimal | CounterStyle::DecimalLeadingZero => {
            let s = v.to_string();
            if matches!(style, CounterStyle::DecimalLeadingZero) && (0..=9).contains(&v) {
                format!("0{}", s)
            } else {
                s
            }
        }
        CounterStyle::LowerAlpha | CounterStyle::UpperAlpha if v < 1 => v.to_string(),
        CounterStyle::LowerAlpha | CounterStyle::UpperAlpha => {
            let mut s = String::new();
            let mut n = v;
            while n > 0 {
                let rem = ((n - 1) % 26) as u8;
                let c = (b'a' + rem) as char;
                s.insert(
                    0,
                    if matches!(style, CounterStyle::UpperAlpha) {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    },
                );
                n = (n - 1) / 26;
            }
            s
        }
        CounterStyle::LowerRoman | CounterStyle::UpperRoman if !(1..=3999).contains(&v) => {
            v.to_string()
        }
        CounterStyle::LowerRoman | CounterStyle::UpperRoman => {
            const PAIRS: [(u16, &str); 13] = [
                (1000, "m"),
                (900, "cm"),
                (500, "d"),
                (400, "cd"),
                (100, "c"),
                (90, "xc"),
                (50, "l"),
                (40, "xl"),
                (10, "x"),
                (9, "ix"),
                (5, "v"),
                (4, "iv"),
                (1, "i"),
            ];
            let mut s = String::new();
            let mut n = v as u16;
            for (val, sym) in PAIRS {
                while n >= val {
                    s.push_str(sym);
                    n -= val;
                }
            }
            if matches!(style, CounterStyle::UpperRoman) {
                s.to_ascii_uppercase()
            } else {
                s
            }
        }
    }
}

/// `content` value on a generated-content pseudo, string forms only this
/// batch (quoted strings with CSS escapes; `attr(name)` as the unresolved
/// name; `none`/`normal` never reach here — they mean "no box").
#[derive(Debug, Clone, PartialEq)]
pub enum ContentValue {
    Str(String),
    Attr(String),
    /// `counter(name)` / `counter(name, style)`
    Counter {
        name: String,
        style: CounterStyle,
    },
    /// `counters(name, "sep")` / `counters(name, "sep", style)`
    Counters {
        name: String,
        sep: String,
        style: CounterStyle,
    },
    OpenQuote,
    CloseQuote,
    /// `no-open-quote`/`no-close-quote`: shifts the quote depth, renders "".
    NoQuote {
        close: bool,
    },
    /// A space-separated content list (`content: "§ " counter(x) ":"`).
    List(Vec<ContentValue>),
}

/// The two box-generating pseudos, each cascaded from its host element.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PseudoPair {
    pub before: Option<ComputedStyle>,
    pub after: Option<ComputedStyle>,
}

/// Parse a `content` value: a single part or a space-separated list of
/// quoted strings (CSS `\` escapes incl. `\e116` hex codepoints),
/// `attr(name)`, `counter(name[, style])`, `counters(name, "sep"[, style])`,
/// open-quote/close-quote/no-open-quote/no-close-quote, or none/normal
/// (None — no box). Unsupported function forms fail closed.
pub fn parse_content_value(value: &str) -> Option<ContentValue> {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    if lower == "none" || lower == "normal" || v.is_empty() {
        return None;
    }
    let parts = parse_content_list(v)?;
    match parts.len() {
        0 => None,
        1 => parts.into_iter().next(),
        _ => Some(ContentValue::List(parts)),
    }
}

/// Decode CSS string escapes: 1-6 hex digits (optionally ONE whitespace
/// terminator) → codepoint; any other char escapes itself (`\x` → x).
pub(crate) fn unescape_css_string(inner: &str) -> String {
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        let mut term: Option<char> = None;
        for h in chars.by_ref() {
            if hex.len() < 6 && h.is_ascii_hexdigit() {
                hex.push(h);
            } else {
                term = Some(h);
                break;
            }
        }
        if hex.is_empty() {
            if let Some(t) = term {
                out.push(t);
            }
            continue;
        }
        if let Some(cp) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            out.push(cp);
        }
        if let Some(t) = term {
            if !t.is_ascii_whitespace() {
                out.push(t);
            }
        }
    }
    out
}

fn parse_content_list(v: &str) -> Option<Vec<ContentValue>> {
    let ch: Vec<char> = v.chars().collect();
    let mut i = 0usize;
    let mut out: Vec<ContentValue> = Vec::new();
    while i < ch.len() {
        let c = ch[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            let mut j = i + 1;
            let mut inner = String::new();
            loop {
                let d = *ch.get(j)?;
                if d == '\\' {
                    inner.push('\\');
                    inner.push(*ch.get(j + 1)?);
                    j += 2;
                    continue;
                }
                if d == quote {
                    break;
                }
                inner.push(d);
                j += 1;
            }
            out.push(ContentValue::Str(unescape_css_string(&inner)));
            i = j + 1;
            continue;
        }
        if c == '(' || c == ')' {
            return None;
        }
        // Identifier token: a bare keyword or the head of a function().
        let start = i;
        while i < ch.len() && !ch[i].is_whitespace() && ch[i] != '(' && ch[i] != ')' {
            i += 1;
        }
        let word: String = ch[start..i].iter().collect();
        if i < ch.len() && ch[i] == '(' {
            let arg_start = i + 1;
            let mut depth = 1i32;
            let mut j = arg_start;
            while j < ch.len() {
                if ch[j] == '(' {
                    depth += 1;
                } else if ch[j] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            if depth != 0 {
                return None;
            }
            let arg: String = ch[arg_start..j].iter().collect();
            out.push(parse_content_function(&word, &arg)?);
            i = j + 1;
            continue;
        }
        match word.to_ascii_lowercase().as_str() {
            "open-quote" => out.push(ContentValue::OpenQuote),
            "close-quote" => out.push(ContentValue::CloseQuote),
            "no-open-quote" => out.push(ContentValue::NoQuote { close: false }),
            "no-close-quote" => out.push(ContentValue::NoQuote { close: true }),
            _ => return None,
        }
    }
    Some(out)
}

fn parse_content_function(word: &str, arg: &str) -> Option<ContentValue> {
    let lw = word.to_ascii_lowercase();
    if lw == "attr" {
        let name = arg.trim();
        if !is_valid_counter_name(name) {
            return None;
        }
        return Some(ContentValue::Attr(name.to_string()));
    }
    if lw != "counter" && lw != "counters" {
        return None;
    }
    let parts = split_top_level_commas(arg);
    let name = parts.first()?.trim().to_string();
    if !is_valid_counter_name(&name) {
        return None;
    }
    let is_counter = lw == "counter";
    match parts.len() {
        1 if is_counter => Some(ContentValue::Counter {
            name,
            style: CounterStyle::Decimal,
        }),
        2 if is_counter => Some(ContentValue::Counter {
            name,
            style: parse_counter_style(parts[1].trim()),
        }),
        2 => Some(ContentValue::Counters {
            name,
            sep: parse_counter_separator(parts[1].trim())?,
            style: CounterStyle::Decimal,
        }),
        3 => Some(ContentValue::Counters {
            name,
            sep: parse_counter_separator(parts[1].trim())?,
            style: parse_counter_style(parts[2].trim()),
        }),
        _ => None,
    }
}

/// Quoted separator (`counters(x, ".")`) or a bare word (`counters(x, -)`).
fn parse_counter_separator(tok: &str) -> Option<String> {
    let bytes = tok.as_bytes();
    if bytes.first() == Some(&b'"') || bytes.first() == Some(&b'\'') {
        let quote = tok.chars().next()?;
        let inner = tok.strip_prefix(quote)?.strip_suffix(quote)?;
        return Some(unescape_css_string(inner));
    }
    if tok.is_empty() || tok.chars().any(char::is_whitespace) {
        return None;
    }
    Some(tok.to_string())
}

/// Known names map to their styles; anything else is a valid declaration
/// whose used style falls back to decimal (css-counter-styles posture).
fn parse_counter_style(tok: &str) -> CounterStyle {
    match tok.to_ascii_lowercase().as_str() {
        "decimal-leading-zero" => CounterStyle::DecimalLeadingZero,
        "lower-alpha" => CounterStyle::LowerAlpha,
        "upper-alpha" => CounterStyle::UpperAlpha,
        "lower-roman" => CounterStyle::LowerRoman,
        "upper-roman" => CounterStyle::UpperRoman,
        _ => CounterStyle::Decimal,
    }
}

/// <custom-ident> shape for counter and attr() names: non-empty, no
/// leading digit, alphanumeric/-/_ only.
fn is_valid_counter_name(s: &str) -> bool {
    !s.is_empty()
        && !s.chars().next().unwrap().is_ascii_digit()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// `counter-reset`/`counter-increment`: `none` → empty; a sequence of
/// `<name> [<int>]?` pairs (increment defaults to +1). Malformed → None
/// (the declaration is dropped).
pub fn parse_counter_modifiers(value: &str, is_increment: bool) -> Option<Vec<(String, i32)>> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if v.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let toks: Vec<&str> = v.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        let name = toks[i];
        if !is_valid_counter_name(name) {
            return None;
        }
        let delta = match toks.get(i + 1) {
            Some(t) if t.parse::<i32>().is_ok() => {
                i += 1;
                t.parse::<i32>().ok()?
            }
            _ if is_increment => 1,
            _ => 0,
        };
        out.push((name.to_string(), delta));
        i += 1;
    }
    Some(out)
}

/// `quotes: none` → Some(empty); `<open> <close> [...]` pairs → flattened
/// list. Unpaired trailing value → None (declaration dropped).
pub fn parse_quotes(value: &str) -> Option<Vec<String>> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if v.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let ch: Vec<char> = v.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < ch.len() {
        if ch[i].is_whitespace() {
            i += 1;
            continue;
        }
        let quote = ch[i];
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut j = i + 1;
        let mut inner = String::new();
        loop {
            let d = *ch.get(j)?;
            if d == '\\' {
                inner.push('\\');
                inner.push(*ch.get(j + 1)?);
                j += 2;
                continue;
            }
            if d == quote {
                break;
            }
            inner.push(d);
            j += 1;
        }
        out.push(unescape_css_string(&inner));
        i = j + 1;
    }
    if out.len() % 2 != 0 || out.is_empty() {
        return None;
    }
    Some(out)
}
