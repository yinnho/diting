//! Ports a browser will not speak HTTP to. Chrome refuses a request to
//! any of them before it opens a socket (`net::ERR_UNSAFE_PORT`), because
//! another protocol answers there and a page's request could be read by it
//! as that protocol's input.
//!
//! The list is what Chrome 158 refused when asked about ports 1 through
//! 10100 (probe: `.local/diting-test-data/cdp11/port-probe.cjs`), not a
//! list from memory. Chrome's own list ends at 10080. A port it names that
//! is also a scheme's default is exempt for that scheme only (ftp 21);
//! nothing here speaks ftp.

use url::Url;

/// Sorted, so a lookup can bisect.
pub const RESTRICTED_PORTS: &[u16] = &[
    1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102,
    103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
    512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993,
    995, 1719, 1720, 1723, 2049, 3659, 4045, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668, 6669,
    6697, 10080,
];

/// The port `url` names, when a browser would refuse it. A URL that names
/// none uses its scheme's default (80, 443), which is never restricted.
///
/// `DITING_EXPLICITLY_ALLOWED_PORTS` — a comma-separated list, as Chrome's
/// `--explicitly-allowed-ports` — lifts the refusal for the ports on it.
pub fn restricted_port(url: &Url) -> Option<u16> {
    let port = url.port()?;
    if !RESTRICTED_PORTS.contains(&port) {
        return None;
    }
    let allowed = std::env::var("DITING_EXPLICITLY_ALLOWED_PORTS").unwrap_or_default();
    (!allowed_by(&allowed, port)).then_some(port)
}

fn allowed_by(list: &str, port: u16) -> bool {
    list.split(',')
        .filter_map(|entry| entry.trim().parse::<u16>().ok())
        .any(|allowed| allowed == port)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn the_table_is_sorted_and_free_of_repeats() {
        assert!(RESTRICTED_PORTS.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(RESTRICTED_PORTS.len(), 80);
    }

    #[test]
    fn a_restricted_port_is_named_and_an_ordinary_one_is_not() {
        assert_eq!(
            restricted_port(&url("http://example.com:6000/")),
            Some(6000)
        );
        assert_eq!(restricted_port(&url("https://example.com:25/")), Some(25));
        assert_eq!(restricted_port(&url("http://example.com:8080/")), None);
        assert_eq!(restricted_port(&url("http://example.com:10081/")), None);
        // No port named: the scheme's default, never restricted.
        assert_eq!(restricted_port(&url("http://example.com/")), None);
        assert_eq!(restricted_port(&url("https://example.com/")), None);
        // A port that is only a default elsewhere is still named as written.
        assert_eq!(restricted_port(&url("https://example.com:80/")), None);
        assert_eq!(restricted_port(&url("http://example.com:443/")), None);
    }

    #[test]
    fn the_allow_list_lifts_only_the_ports_on_it() {
        assert!(allowed_by("6000, 6666", 6000));
        assert!(allowed_by("6000,6666", 6666));
        assert!(!allowed_by("6000,6666", 25));
        assert!(!allowed_by("", 6000));
        assert!(!allowed_by("not-a-port,,", 6000));
    }

    fn kind_and_message(result: Result<(), crate::NetError>) -> (crate::NetFailure, String) {
        match result.expect_err("refused") {
            crate::NetError::Transport { kind, message } => (kind, message),
            crate::NetError::Network(message) => (crate::NetFailure::Other, message),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_refused_port_is_refused_whatever_the_private_network_switch_says() {
        use crate::client::validate_url;
        let (kind, message) =
            kind_and_message(validate_url(&url("http://example.com:6000/"), true));
        assert_eq!(kind, crate::NetFailure::UnsafePort);
        assert!(message.contains("net::ERR_UNSAFE_PORT"), "{message}");
        // An ordinary port is not touched by it.
        assert!(validate_url(&url("http://example.com:8080/"), false).is_ok());
    }

    #[test]
    fn a_url_refused_twice_keeps_the_private_network_words_and_the_ports_kind() {
        use crate::client::validate_url;
        // The switch is process-wide: hold it off for the assertions.
        let _guard = crate::PRIVATE_NET_ENV_LOCK.lock().unwrap();
        std::env::remove_var("DITING_ALLOW_PRIVATE_NETWORK");
        // Loopback is refused by the private-network policy; port 9 by the
        // browser's list. The operator still reads the opt-in hint, and the
        // kind says what a browser would have said.
        let (kind, message) = kind_and_message(validate_url(&url("http://127.0.0.1:9/"), false));
        assert_eq!(kind, crate::NetFailure::UnsafePort);
        assert!(message.contains("private/internal IP address"), "{message}");
        // The same address on an ordinary port is only the policy's.
        let (kind, _) = kind_and_message(validate_url(&url("http://127.0.0.1:8080/"), false));
        assert_eq!(kind, crate::NetFailure::Other);
    }

    #[test]
    fn only_web_urls_answer_to_the_port_list() {
        use crate::client::validate_url;
        // `file:` names no port; an unknown scheme keeps its own refusal.
        assert!(validate_url(&url("file:///tmp/x"), false).is_ok());
        let (kind, _) = kind_and_message(validate_url(&url("ftp://example.com:21/"), false));
        assert_eq!(kind, crate::NetFailure::Other);
    }
}
