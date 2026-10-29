//! The Chrome we claim to be - single source: `crates/diting/chrome-target.toml`.
//!
//! build.rs generates the constants below from that file. Every layer that needs a Chrome
//! version (UA string, Client Hints, TLS preset, tests) reads it from here; a Chrome version
//! literal anywhere else is a coherence bug that `tools/check_coherence.py` fails in CI.
//! Owner ruling 2026-10-29: follow Chrome stable, all layers one real version, macOS arm64.

include!(concat!(env!("OUT_DIR"), "/chrome_target.rs"));

macro_rules! chrome_ua {
    ($os:expr) => {
        concat!(
            "Mozilla/5.0 (",
            $os,
            ") AppleWebKit/537.36 (KHTML, like Gecko) Chrome/",
            env!("DITING_CHROME_MAJOR"),
            ".0.0.0 Safari/537.36"
        )
    };
}

/// Reduced UA strings (Chrome freezes everything after the major to `.0.0.0`).
pub const UA_MAC: &str = chrome_ua!("Macintosh; Intel Mac OS X 10_15_7");
pub const UA_WINDOWS: &str = chrome_ua!("Windows NT 10.0; Win64; x64");
pub const UA_LINUX: &str = chrome_ua!("X11; Linux x86_64");

/// The persona's own platform (what the product claims to be by default): macOS.
pub const UA_DEFAULT: &str = UA_MAC;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_is_internally_consistent() {
        assert!(CHROME_FULL_VERSION.starts_with(&format!("{CHROME_MAJOR}.")));
        assert_eq!(TLS_PROFILE_NAME, format!("chrome{CHROME_MAJOR}"));
        for ua in [UA_MAC, UA_WINDOWS, UA_LINUX] {
            assert!(ua.contains(&format!("Chrome/{CHROME_MAJOR}.0.0.0 ")), "{ua}");
        }
        assert_eq!(UA_DEFAULT, UA_MAC);
        assert_eq!((PLATFORM_OS, PLATFORM_ARCH), ("macOS", "arm64"));
    }
}
