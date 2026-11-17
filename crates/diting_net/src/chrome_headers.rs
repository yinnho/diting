//! Chrome's HTTP/2 request-header order and the values that go with it.
//!
//! The wreq-util preset only seeds a plain `HeaderMap` (insertion order, no
//! real Chrome layout, `accept-encoding` compiled out) and wreq's auto-
//! decompression layer then appends its own `zstd,gzip,deflate,br` at the
//! very end. Header order is part of every HTTP client fingerprint (JA4H hashes
//! the ordered names), so each request gets Chrome's order via
//! `RequestBuilder::orig_headers`.
//!
//! Orders were measured on a real Chrome 155 against tls.peet.ws on
//! 2026-10-20, over HTTP/2 (pseudo-headers aside):
//!   * navigation:  sec-ch-ua*, upgrade-insecure-requests, user-agent, accept,
//!     sec-fetch-{site,mode,user,dest}, referer, accept-encoding,
//!     accept-language, cookie, priority.
//!   * subresource with page-set ("author") headers: Blink keeps author
//!     headers in a hash map, so their position among Chrome's own headers is
//!     fixed per name — measured here for the xhs set (authorization, x-t,
//!     x-b3-traceid, x-s-common, x-s, content-type) the publish endpoints use.
//!   * subresource without author headers: platform/UA/ch-ua first, then
//!     accept and the fetch-metadata triple.
//!
//! Headers a list does not name keep wreq's default placement (after the
//! named ones), which is where Chrome puts unknown author headers closest to.

use diting_wire::header::OrigHeaderMap;

use crate::client::ResourceType;

/// Derive `sec-ch-ua` and `sec-ch-ua-platform` from the User-Agent string so
/// the client hints stay consistent with the advertised UA. Anti-bot systems
/// (WeChat, etc.) flag mismatches like a macOS UA paired with a "Linux"
/// sec-ch-ua-platform or a version drift between UA and sec-ch-ua.
///
/// Returns `(sec_ch_ua_header, sec_ch_ua_platform_header)`.
pub fn derive_client_hints(ua: &str) -> (String, String) {
    // Major version: first \d+ after "Chrome/". A UA with no Chrome token
    // still has to agree with the fleet (chrome-target), not a leftover major.
    let version = ua
        .split("Chrome/")
        .nth(1)
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(diting_env::chrome_target::CHROME_MAJOR);

    let platform = if ua.contains("Macintosh") || ua.contains("Mac OS X") {
        "\"macOS\""
    } else if ua.contains("Windows") {
        "\"Windows\""
    } else if ua.contains("iPhone") || ua.contains("Android") {
        "\"Android\""
    } else {
        "\"Linux\""
    };

    (grease_brand_list(version), platform.to_string())
}

/// Chromium's UA-CH GREASE generator (user_agent_utils.cc), ported: the
/// grease brand, its version, and the shuffle order are all derived from
/// the major version — never random. Keeping the algorithm instead of a
/// per-major literal makes the header match the JS face
/// (bootstrap.js `__ditingBrandList`) for any major the fleet claims;
/// 145 and 155 were both hardcoded wrong before this.
/// 154 → `"Chromium";v="154", "Google Chrome";v="154", "Not A(Brand";v="99"`
/// (algorithm example; the fleet itself follows chrome-target).
fn grease_brand_list(version: u32) -> String {
    const CHARS: [&str; 11] = [" ", "(", ":", "-", ".", "/", ")", ";", "=", "?", "_"];
    const VERS: [&str; 3] = ["8", "99", "24"];
    const ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let seed = version as usize;
    let grease = format!("Not{}A{}Brand", CHARS[seed % 11], CHARS[(seed + 1) % 11]);
    let grease_ver = VERS[seed % 3];
    let vstr = version.to_string();
    let pre_shuffled = [
        (grease.as_str(), grease_ver),
        ("Chromium", vstr.as_str()),
        ("Google Chrome", vstr.as_str()),
    ];
    let order = ORDERS[seed % 6];
    let mut shuffled: [(&str, &str); 3] = [("", ""); 3];
    for (i, slot) in order.iter().enumerate() {
        shuffled[*slot] = pre_shuffled[i];
    }
    let parts: Vec<String> = shuffled
        .iter()
        .map(|(brand, ver)| format!("\"{brand}\";v=\"{ver}\""))
        .collect();
    parts.join(", ")
}

/// Chrome's literal for the request. Equals Chrome 145's value; the zstd
/// token arrived with Chrome 123.
pub const ACCEPT_ENCODING: &str = "gzip, deflate, br, zstd";

/// `Priority` request header. Top-level navigations are `u=0, i`; fetch/XHR
/// and scripts `u=1, i`; images `u=2, i`; stylesheets `u=0` (no `i`).
pub fn priority(rt: ResourceType) -> &'static str {
    match rt {
        ResourceType::Document => "u=0, i",
        ResourceType::Stylesheet => "u=0",
        ResourceType::Image => "u=2, i",
        ResourceType::Script | ResourceType::ModuleScript | ResourceType::Fetch => "u=1, i",
    }
}

const NAVIGATION: &[&str] = &[
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "sec-ch-ua-platform",
    "upgrade-insecure-requests",
    "user-agent",
    "accept",
    "sec-fetch-site",
    "sec-fetch-mode",
    "sec-fetch-user",
    "sec-fetch-dest",
    "referer",
    "accept-encoding",
    "accept-language",
    "cookie",
    "priority",
];

/// Subresource carrying page-set headers; `accept` explicitly set by the page
/// (axios-style) sits right after user-agent.
const AUTHORED_ACCEPT_SET: &[&str] = &[
    "content-length",
    "sec-ch-ua-platform",
    "authorization",
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "x-t",
    "x-b3-traceid",
    "x-s-common",
    "user-agent",
    "accept",
    "content-type",
    "x-s",
    "origin",
    "sec-fetch-site",
    "sec-fetch-mode",
    "sec-fetch-user",
    "sec-fetch-dest",
    "referer",
    "accept-encoding",
    "accept-language",
    "cookie",
    "priority",
];

/// Same, with `accept` left to the browser default: it lands after x-s.
const AUTHORED_ACCEPT_DEFAULT: &[&str] = &[
    "content-length",
    "sec-ch-ua-platform",
    "authorization",
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "x-t",
    "x-b3-traceid",
    "x-s-common",
    "user-agent",
    "content-type",
    "x-s",
    "accept",
    "origin",
    "sec-fetch-site",
    "sec-fetch-mode",
    "sec-fetch-user",
    "sec-fetch-dest",
    "referer",
    "accept-encoding",
    "accept-language",
    "cookie",
    "priority",
];

/// Subresource with no page-set headers (plain script/image/css loads, bare
/// `fetch(url)`).
const PLAIN_SUBRESOURCE: &[&str] = &[
    "content-length",
    "sec-ch-ua-platform",
    "user-agent",
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "content-type",
    "accept",
    "origin",
    "sec-fetch-site",
    "sec-fetch-mode",
    "sec-fetch-dest",
    "referer",
    "accept-encoding",
    "accept-language",
    "cookie",
    "priority",
];

fn order_names(navigation: bool, authored: bool, authored_accept: bool) -> &'static [&'static str] {
    if navigation {
        NAVIGATION
    } else if authored {
        if authored_accept {
            AUTHORED_ACCEPT_SET
        } else {
            AUTHORED_ACCEPT_DEFAULT
        }
    } else {
        PLAIN_SUBRESOURCE
    }
}

/// True when the page set headers beyond the ones a browser adds itself —
/// the case where Blink's author-header hash map shapes the wire order.
pub fn has_author_headers<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    names.into_iter().any(|n| {
        !matches!(
            n.to_ascii_lowercase().as_str(),
            "accept"
                | "accept-language"
                | "accept-encoding"
                | "referer"
                | "origin"
                | "cookie"
                | "user-agent"
                | "content-length"
                | "priority"
                | "content-type"
                | "sec-ch-ua"
                | "sec-ch-ua-mobile"
                | "sec-ch-ua-platform"
                | "sec-fetch-dest"
                | "sec-fetch-mode"
                | "sec-fetch-site"
                | "sec-fetch-user"
                | "upgrade-insecure-requests"
        )
    })
}

/// The `OrigHeaderMap` for one request.
pub fn header_order(navigation: bool, authored: bool, authored_accept: bool) -> OrigHeaderMap {
    let mut map = OrigHeaderMap::new();
    for name in order_names(navigation, authored, authored_accept) {
        map.insert(*name);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(m: &OrigHeaderMap) -> Vec<String> {
        m.iter().map(|(n, _)| n.as_str().to_string()).collect()
    }

    #[test]
    fn navigation_order_is_chromes() {
        let n = names(&header_order(true, false, false));
        let pos = |s: &str| n.iter().position(|x| x == s).unwrap();
        assert!(pos("sec-ch-ua") < pos("upgrade-insecure-requests"));
        assert!(pos("user-agent") < pos("accept"));
        assert!(pos("sec-fetch-site") < pos("sec-fetch-mode"));
        assert!(pos("sec-fetch-user") < pos("sec-fetch-dest"));
        assert!(pos("accept-encoding") < pos("accept-language"));
        assert_eq!(n.last().unwrap(), "priority");
    }

    #[test]
    fn xhs_publish_render_order_matches_measured_chrome() {
        // Measured on Chrome 155: content-length, sec-ch-ua-platform,
        // authorization, sec-ch-ua, sec-ch-ua-mobile, x-t, x-b3-traceid,
        // x-s-common, user-agent, accept, content-type, x-s, origin,
        // sec-fetch-site/mode/dest, referer, accept-encoding,
        // accept-language, priority.
        let n = names(&header_order(false, true, true));
        let want = [
            "content-length",
            "sec-ch-ua-platform",
            "authorization",
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "x-t",
            "x-b3-traceid",
            "x-s-common",
            "user-agent",
            "accept",
            "content-type",
            "x-s",
            "origin",
            "sec-fetch-site",
            "sec-fetch-mode",
        ];
        assert_eq!(&n[..want.len()], &want);
    }

    #[test]
    fn author_header_detection() {
        assert!(has_author_headers(["x-s", "accept"]));
        assert!(has_author_headers(["Authorization"]));
        assert!(!has_author_headers(["accept", "Content-Type", "Referer"]));
    }

    #[test]
    fn priority_by_resource_class() {
        assert_eq!(priority(ResourceType::Document), "u=0, i");
        assert_eq!(priority(ResourceType::Fetch), "u=1, i");
        assert_eq!(priority(ResourceType::Image), "u=2, i");
        assert_eq!(priority(ResourceType::Stylesheet), "u=0");
    }
}
