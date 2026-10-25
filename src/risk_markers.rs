//! Risk-control wall markers — the generic detection FRAMEWORK.
//!
//! Asset separation (2026-10-25): the engine detects walls; the needle
//! TABLE is operator-supplied business data. The public build ships an
//! EMPTY table — every function below answers "no wall" until a signal
//! file is loaded via `DITING_RISK_SIGNALS=<json>` (template in
//! `config/risk-signals.template.json`). Observation stays in core
//! (detect + surface, never bypass); site business (signing, publish,
//! needle tables) stays private.
//!
//! `har`, `captcha`, and `verdict` call the same functions so the three
//! faces cannot drift — the sharing is the tool value; the needles are
//! the business value.

use std::sync::OnceLock;

/// The loaded signal table. Needles are substring-matched: path needles
/// against the URL with `?query`/`#fragment` stripped, hosts and tokens
/// against the full lowercased URL (and hosts also against body HTML —
/// script-driven redirects embed them without the page URL moving),
/// body markers against retained bodies.
#[derive(Debug, Default, Clone)]
pub struct Signals {
    /// Challenge tag echoed on har/session rows (e.g. "punish").
    pub challenge_tag: String,
    /// Path-shaped needles — matched after stripping query/fragment.
    pub url_path_needles: Vec<String>,
    /// Host-shaped needles — unambiguous wall hosts.
    pub url_host_needles: Vec<String>,
    /// Token needles — broader than path/host, count even on clean URLs
    /// (the historical captcha-detector posture).
    pub url_token_needles: Vec<String>,
    /// Body markers for API/JSON payloads (a 200 whose fields say walled).
    pub body_markers: Vec<String>,
    /// Body-only needles for HTML interstitials (page-title shapes).
    pub body_title_markers: Vec<String>,
}

impl Signals {
    /// Parse the operator JSON document. Unknown fields ignored; a bad
    /// document keeps the empty table (warn).
    pub fn parse(raw: &str) -> Signals {
        let mut s = Signals::default();
        let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
            tracing::warn!("risk_signals: JSON did not parse; table stays empty");
            return s;
        };
        if let Some(tag) = v.get("challenge_tag").and_then(|t| t.as_str()) {
            s.challenge_tag = tag.to_string();
        }
        let take_arr = |key: &str, into: &mut Vec<String>| {
            if let Some(arr) = v.get(key).and_then(|a| a.as_array()) {
                for item in arr {
                    if let Some(n) = item.as_str() {
                        if !n.is_empty() {
                            into.push(n.to_string());
                        }
                    }
                }
            }
        };
        take_arr("url_path_needles", &mut s.url_path_needles);
        take_arr("url_host_needles", &mut s.url_host_needles);
        take_arr("url_token_needles", &mut s.url_token_needles);
        take_arr("body_markers", &mut s.body_markers);
        take_arr("body_title_markers", &mut s.body_title_markers);
        s
    }
}

static ENV_TABLE: OnceLock<Signals> = OnceLock::new();
#[cfg(test)]
static TEST_OVERRIDE: std::sync::Mutex<Option<Signals>> = std::sync::Mutex::new(None);

/// The active signal table: the test override when set (test binaries
/// install one shared demo table so parallel needle tests agree), else
/// `DITING_RISK_SIGNALS` (file path) read once; unset or unreadable
/// means the empty table.
pub fn table() -> Signals {
    #[cfg(test)]
    if let Some(t) = TEST_OVERRIDE.lock().unwrap().clone() {
        return t;
    }
    ENV_TABLE
        .get_or_init(|| {
            let path = std::env::var("DITING_RISK_SIGNALS").unwrap_or_default();
            if path.is_empty() {
                return Signals::default();
            }
            match std::fs::read_to_string(&path) {
                Ok(raw) => Signals::parse(&raw),
                Err(e) => {
                    tracing::warn!("risk_signals: cannot read {path}: {e}; table stays empty");
                    Signals::default()
                }
            }
        })
        .clone()
}

/// The demo table the test binary installs (neutral needles — the same
/// assertion shapes the old hardcoded platform cluster had).
#[cfg(test)]
pub(crate) fn demo_table() -> Signals {
    Signals::parse(
        r#"{
            "challenge_tag": "demo-wall",
            "url_path_needles": ["/demo/punish"],
            "url_host_needles": ["wall.example.net"],
            "url_token_needles": ["secmark"],
            "body_markers": ["DEMO_WALL_HIT", "walledata"],
            "body_title_markers": ["Demo Intercept"]
        }"#,
    )
}

/// Install the shared demo table for this test process. Idempotent;
/// every needle-asserting test calls it first so the parallel suite
/// sees one consistent table.
#[cfg(test)]
pub(crate) fn set_for_tests(s: Signals) {
    *TEST_OVERRIDE.lock().unwrap() = Some(s);
}

/// URL-shape challenge for the network log face: strip query/fragment
/// before matching path needles (a mere `?needle=x` query name must not
/// trip the path form); hosts match the full lowercased URL.
pub fn ali_url_challenge_kind(url: &str) -> Option<String> {
    let t = table();
    if t.challenge_tag.is_empty() {
        return None;
    }
    let bare = url.split(['?', '#']).next().unwrap_or(url);
    let path_l = bare.to_ascii_lowercase();
    if t.url_path_needles.iter().any(|n| path_l.contains(n.as_str())) {
        return Some(t.challenge_tag.clone());
    }
    let full_l = url.to_ascii_lowercase();
    if t.url_host_needles.iter().any(|n| full_l.contains(n.as_str())) {
        return Some(t.challenge_tag.clone());
    }
    None
}

/// Captcha/verdict-style URL hit: substring on the full URL (query
/// counts), plus the broader token needles the captcha face uses.
pub fn ali_url_looks_walled(url: &str) -> bool {
    let t = table();
    let lower = url.to_ascii_lowercase();
    if t.url_path_needles.iter().any(|n| lower.contains(n.as_str())) {
        return true;
    }
    if t.url_host_needles.iter().any(|n| lower.contains(n.as_str())) {
        return true;
    }
    if t.url_token_needles.iter().any(|n| lower.contains(n.as_str())) {
        return true;
    }
    false
}

/// Body/HTML looks like a walled payload or in-place interstitial page.
pub fn ali_body_looks_walled(body: &str) -> bool {
    let t = table();
    if t.body_markers.iter().any(|m| body.contains(m.as_str())) {
        return true;
    }
    if t.body_title_markers.iter().any(|m| body.contains(m.as_str())) {
        return true;
    }
    // Script-driven redirects embed hosts in HTML without the page URL
    // changing yet.
    let lower = body.to_ascii_lowercase();
    if t.url_host_needles.iter().any(|n| lower.contains(n.as_str())) {
        return true;
    }
    if t.url_path_needles.iter().any(|n| lower.contains(n.as_str())) {
        return true;
    }
    false
}

/// Body-only challenge tag for har (API JSON) — same markers, no HTML
/// titles (har retained bodies are API JSON; titles are a captcha/HTML
/// concern).
pub fn ali_body_challenge_kind(body: &str) -> Option<String> {
    let t = table();
    if !t.challenge_tag.is_empty() && t.body_markers.iter().any(|m| body.contains(m.as_str())) {
        return Some(t.challenge_tag.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The injected table drives every face — the same assertion shapes
    /// the old hardcoded cluster had, against neutral needles.
    #[test]
    fn injected_table_drives_all_faces() {
        set_for_tests(Signals::parse(
            r#"{
                "challenge_tag": "demo-wall",
                "url_path_needles": ["/demo/punish"],
                "url_host_needles": ["wall.example.net"],
                "url_token_needles": ["secmark"],
                "body_markers": ["DEMO_WALL_HIT", "walledata"],
                "body_title_markers": ["Demo Intercept"]
            }"#,
        ));
        // Path form trips; query-only name does not (har convention).
        assert_eq!(
            ali_url_challenge_kind("https://api.example.com/x/1.0//demo/punish"),
            Some("demo-wall".to_string())
        );
        assert_eq!(
            ali_url_challenge_kind("https://api.example.com/x/?/demo/punish=x"),
            None
        );
        assert_eq!(
            ali_url_challenge_kind("https://wall.example.net/auth?secmark=abc"),
            Some("demo-wall".to_string())
        );
        // Captcha posture: query counts + token needles.
        assert!(ali_url_looks_walled("https://wall.example.net/x"));
        assert!(ali_url_looks_walled("https://a.example.com/y?secmark=1"));
        assert!(!ali_url_looks_walled("https://api.example.com/clean/1.0/"));
        // Bodies.
        assert!(ali_body_looks_walled(r#"{"ret":["DEMO_WALL_HIT"]}"#));
        assert_eq!(ali_body_challenge_kind(r#"{"walledata":"z"}"#), Some("demo-wall".to_string()));
        assert!(ali_body_looks_walled("<title>Demo Intercept</title>"));
        assert!(ali_body_looks_walled("redirect to wall.example.net soon"));
        assert_eq!(ali_body_challenge_kind(r#"{"ok":true}"#), None);
    }

    /// The parse contract: empty/unknown docs produce the empty table
    /// that detects nothing.
    #[test]
    fn empty_document_parses_to_empty_table() {
        let s = Signals::parse("{}");
        assert!(s.challenge_tag.is_empty() && s.url_path_needles.is_empty());
        let bad = Signals::parse("not json");
        assert!(bad.url_host_needles.is_empty());
    }
}
