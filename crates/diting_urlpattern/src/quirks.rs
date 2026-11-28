//! This module contains functions required to integrate this library into
//! browsers. If you are not building a browser, you can ignore this module.

// diting: the mirror type system that used to live here (quirks::UrlPattern,
// StringOrInit, process_construct_pattern_input, parse_pattern,
// process_match_input, EcmaRegexp — the browser-integration layer for
// host-compiled regexps) had zero consumers after the native migration and
// was removed. What stays is the match-input normalization the main matcher
// actually runs through.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchInput {
    pub protocol: String,
    pub username: String,
    pub password: String,
    pub hostname: String,
    pub port: String,
    pub pathname: String,
    pub search: String,
    pub hash: String,
}

pub fn parse_match_input(input: crate::UrlPatternMatchInput) -> Option<MatchInput> {
    let mut i = MatchInput::default();
    match input {
        crate::UrlPatternMatchInput::Init(init) => {
            if let Ok(apply_result) = init.process(
                crate::canonicalize_and_process::ProcessType::Url,
                Some(i.protocol),
                Some(i.username),
                Some(i.password),
                Some(i.hostname),
                Some(i.port),
                Some(i.pathname),
                Some(i.search),
                Some(i.hash),
            ) {
                i.protocol = apply_result.protocol.unwrap();
                i.username = apply_result.username.unwrap();
                i.password = apply_result.password.unwrap();
                i.hostname = apply_result.hostname.unwrap();
                i.port = apply_result.port.unwrap();
                i.pathname = apply_result.pathname.unwrap();
                i.search = apply_result.search.unwrap();
                i.hash = apply_result.hash.unwrap();
            } else {
                return None;
            }
        }
        crate::UrlPatternMatchInput::Url(url) => {
            i.protocol = url.scheme().to_string();
            i.username = url.username().to_string();
            i.password = url.password().unwrap_or_default().to_string();
            i.hostname = url.host_str().unwrap_or_default().to_string();
            i.port = url::quirks::port(&url).to_string();
            i.pathname = url::quirks::pathname(&url).to_string();
            i.search = url.query().unwrap_or_default().to_string();
            i.hash = url.fragment().unwrap_or_default().to_string();
        }
    }

    Some(i)
}
