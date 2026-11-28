// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Vendored upstream — byte-shape wins over house lints (crates/VENDORED.md).
#![allow(clippy::all)]
//! rust-urlpattern is an implementation of the
//! [URLPattern standard](https://wicg.github.io/urlpattern) for the Rust
//! programming language.
//!
//! For a usage example, see the [UrlPattern] documentation.

mod canonicalize_and_process;
mod component;
mod constructor_parser;
mod error;
mod matcher;
mod parser;
pub mod quirks;
mod regexp;
mod tokenizer;

pub use error::Error;
use url::Url;

use crate::canonicalize_and_process::is_special_scheme;
use crate::canonicalize_and_process::process_base_url;
use crate::canonicalize_and_process::special_scheme_default_port;
use crate::canonicalize_and_process::ProcessType;
use crate::component::Component;
use crate::regexp::RegExp;

/// Options to create a URL pattern.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct UrlPatternOptions {
    pub ignore_case: bool,
}

/// The structured input used to create a URL pattern.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct UrlPatternInit {
    pub protocol: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub hostname: Option<String>,
    pub port: Option<String>,
    pub pathname: Option<String>,
    pub search: Option<String>,
    pub hash: Option<String>,
    pub base_url: Option<Url>,
}

impl UrlPatternInit {
    pub fn parse_constructor_string<R: RegExp>(
        pattern: &str,
        base_url: Option<Url>,
    ) -> Result<UrlPatternInit, Error> {
        let mut init = constructor_parser::parse_constructor_string::<R>(pattern)?;
        if base_url.is_none() && init.protocol.is_none() {
            return Err(Error::BaseUrlRequired);
        }
        init.base_url = base_url;
        Ok(init)
    }

    // Ref: https://wicg.github.io/urlpattern/#process-a-urlpatterninit
    // TODO: use UrlPatternInit for arguments?
    #[allow(clippy::too_many_arguments)]
    fn process(
        &self,
        kind: ProcessType,
        protocol: Option<String>,
        username: Option<String>,
        password: Option<String>,
        hostname: Option<String>,
        port: Option<String>,
        pathname: Option<String>,
        search: Option<String>,
        hash: Option<String>,
    ) -> Result<UrlPatternInit, Error> {
        let mut result = UrlPatternInit {
            protocol,
            username,
            password,
            hostname,
            port,
            pathname,
            search,
            hash,
            base_url: None,
        };

        let base_url = if let Some(parsed_base_url) = &self.base_url {
            if self.protocol.is_none() {
                result.protocol = Some(process_base_url(parsed_base_url.scheme(), &kind));
            }

            if kind != ProcessType::Pattern
                && (self.protocol.is_none()
                    && self.hostname.is_none()
                    && self.port.is_none()
                    && self.username.is_none())
            {
                result.username = Some(process_base_url(parsed_base_url.username(), &kind));
            }

            if kind != ProcessType::Pattern
                && (self.protocol.is_none()
                    && self.hostname.is_none()
                    && self.port.is_none()
                    && self.username.is_none()
                    && self.password.is_none())
            {
                result.password = Some(process_base_url(
                    parsed_base_url.password().unwrap_or_default(),
                    &kind,
                ));
            }

            if self.protocol.is_none() && self.hostname.is_none() {
                result.hostname = Some(process_base_url(
                    parsed_base_url.host_str().unwrap_or_default(),
                    &kind,
                ));
            }

            if self.protocol.is_none() && self.hostname.is_none() && self.port.is_none() {
                result.port = Some(process_base_url(url::quirks::port(parsed_base_url), &kind));
            }

            if self.protocol.is_none()
                && self.hostname.is_none()
                && self.port.is_none()
                && self.pathname.is_none()
            {
                result.pathname = Some(process_base_url(
                    url::quirks::pathname(parsed_base_url),
                    &kind,
                ));
            }

            if self.protocol.is_none()
                && self.hostname.is_none()
                && self.port.is_none()
                && self.pathname.is_none()
                && self.search.is_none()
            {
                result.search = Some(process_base_url(
                    parsed_base_url.query().unwrap_or_default(),
                    &kind,
                ));
            }

            if self.protocol.is_none()
                && self.hostname.is_none()
                && self.port.is_none()
                && self.pathname.is_none()
                && self.search.is_none()
                && self.hash.is_none()
            {
                result.hash = Some(process_base_url(
                    parsed_base_url.fragment().unwrap_or_default(),
                    &kind,
                ));
            }

            Some(parsed_base_url)
        } else {
            None
        };

        if let Some(protocol) = &self.protocol {
            result.protocol = Some(canonicalize_and_process::process_protocol_init(
                protocol, &kind,
            )?);
        }
        if let Some(username) = &self.username {
            result.username = Some(canonicalize_and_process::process_username_init(
                username, &kind,
            )?);
        }
        if let Some(password) = &self.password {
            result.password = Some(canonicalize_and_process::process_password_init(
                password, &kind,
            )?);
        }
        if let Some(hostname) = &self.hostname {
            result.hostname = Some(canonicalize_and_process::process_hostname_init(
                hostname, &kind,
            )?);
        }
        if let Some(port) = &self.port {
            result.port = Some(canonicalize_and_process::process_port_init(
                port,
                result.protocol.as_deref(),
                &kind,
            )?);
        }
        if let Some(pathname) = &self.pathname {
            result.pathname = Some(pathname.clone());

            if let Some(base_url) = base_url {
                if !base_url.cannot_be_a_base() && !is_absolute_pathname(pathname, &kind) {
                    let baseurl_path = url::quirks::pathname(base_url);
                    let slash_index = baseurl_path.rfind('/');
                    if let Some(slash_index) = slash_index {
                        let new_pathname = baseurl_path[..=slash_index].to_string();
                        result.pathname =
                            Some(format!("{}{}", new_pathname, result.pathname.unwrap()));
                    }
                }
            }

            result.pathname = Some(canonicalize_and_process::process_pathname_init(
                &result.pathname.unwrap(),
                result.protocol.as_deref(),
                &kind,
            )?);
        }
        if let Some(search) = &self.search {
            result.search = Some(canonicalize_and_process::process_search_init(
                search, &kind,
            )?);
        }
        if let Some(hash) = &self.hash {
            result.hash = Some(canonicalize_and_process::process_hash_init(hash, &kind)?);
        }
        Ok(result)
    }
}

// Ref: https://wicg.github.io/urlpattern/#is-an-absolute-pathname
fn is_absolute_pathname(input: &str, kind: &canonicalize_and_process::ProcessType) -> bool {
    if input.is_empty() {
        return false;
    }
    if input.starts_with('/') {
        return true;
    }
    if kind == &canonicalize_and_process::ProcessType::Url {
        return false;
    }
    // TODO: input code point length
    if input.len() < 2 {
        return false;
    }

    input.starts_with("\\/") || input.starts_with("{/")
}

// Ref: https://wicg.github.io/urlpattern/#urlpattern
/// A UrlPattern that can be matched against.
///
/// # Examples
///
/// ```
/// use urlpattern::UrlPattern;
/// use urlpattern::UrlPatternInit;
/// use urlpattern::UrlPatternMatchInput;
///
///# fn main() {
/// // Create the UrlPattern to match against.
/// let init = UrlPatternInit {
///   pathname: Some("/users/:id".to_owned()),
///   ..Default::default()
/// };
/// let pattern = <UrlPattern>::parse(init, Default::default()).unwrap();
///
/// // Match the pattern against a URL.
/// let url = "https://example.com/users/123".parse().unwrap();
/// let result = pattern.exec(UrlPatternMatchInput::Url(url)).unwrap().unwrap();
/// assert_eq!(result.pathname.groups.iter().find(|(k, _)| k == "id").unwrap().1.as_ref().unwrap(), "123");
///# }
/// ```
#[derive(Debug)]
pub struct UrlPattern<R: RegExp = regex::Regex> {
    protocol: Component<R>,
    username: Component<R>,
    password: Component<R>,
    hostname: Component<R>,
    port: Component<R>,
    pathname: Component<R>,
    search: Component<R>,
    hash: Component<R>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlPatternMatchInput {
    Init(UrlPatternInit),
    Url(Url),
}

// diting: liburlpattern wraps every component compile failure with the
// component name and the original pattern text (measured Chrome wording)
fn compile_component<R: RegExp>(
    name: &str,
    input: Option<&str>,
    encoding_callback: fn(&str) -> Result<String, Error>,
    options: parser::Options,
) -> Result<component::Component<R>, Error> {
    component::Component::compile(name, input, encoding_callback, options)
}

impl<R: RegExp> UrlPattern<R> {
    // Ref: https://wicg.github.io/urlpattern/#dom-urlpattern-urlpattern
    /// Parse a [UrlPatternInit] into a [UrlPattern].
    pub fn parse(init: UrlPatternInit, options: UrlPatternOptions) -> Result<Self, Error> {
        Self::parse_internal(init, true, options)
    }

    pub(crate) fn parse_internal(
        init: UrlPatternInit,
        report_regex_errors: bool,
        options: UrlPatternOptions,
    ) -> Result<Self, Error> {
        let mut processed_init = init.process(
            ProcessType::Pattern,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )?;

        //  If processedInit["protocol"] is a special scheme and processedInit["port"] is its corresponding default port
        if let Some(protocol) = &processed_init.protocol {
            if is_special_scheme(protocol) {
                let default_port = special_scheme_default_port(protocol);
                if default_port == processed_init.port.as_deref() {
                    processed_init.port = Some(String::new())
                }
            }
        }

        let protocol = compile_component(
            "protocol",
            processed_init.protocol.as_deref(),
            canonicalize_and_process::canonicalize_protocol,
            parser::Options::default(),
        )?
        .optionally_transpose_regex_error(report_regex_errors)?;

        let hostname_is_ipv6 = processed_init
            .hostname
            .as_deref()
            .map(hostname_pattern_is_ipv6_address)
            .unwrap_or(false);

        let hostname = if hostname_is_ipv6 {
            compile_component(
                "hostname",
                processed_init.hostname.as_deref(),
                canonicalize_and_process::canonicalize_ipv6_hostname,
                parser::Options::hostname(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?
        } else {
            compile_component(
                "hostname",
                processed_init.hostname.as_deref(),
                canonicalize_and_process::canonicalize_hostname,
                parser::Options::hostname(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?
        };

        let compile_options = parser::Options {
            ignore_case: options.ignore_case,
            ..Default::default()
        };

        let pathname = if protocol.protocol_component_matches_special_scheme() {
            compile_component(
                "pathname",
                processed_init.pathname.as_deref(),
                canonicalize_and_process::canonicalize_pathname,
                parser::Options {
                    ignore_case: options.ignore_case,
                    ..parser::Options::pathname()
                },
            )?
            .optionally_transpose_regex_error(report_regex_errors)?
        } else {
            compile_component(
                "pathname",
                processed_init.pathname.as_deref(),
                canonicalize_and_process::canonicalize_an_opaque_pathname,
                compile_options.clone(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?
        };

        Ok(UrlPattern {
            protocol,
            username: compile_component(
                "username",
                processed_init.username.as_deref(),
                canonicalize_and_process::canonicalize_username,
                parser::Options::default(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?,
            password: compile_component(
                "password",
                processed_init.password.as_deref(),
                canonicalize_and_process::canonicalize_password,
                parser::Options::default(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?,
            hostname,
            port: compile_component(
                "port",
                processed_init.port.as_deref(),
                |port| canonicalize_and_process::canonicalize_port(port, None),
                parser::Options::default(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?,
            pathname,
            search: compile_component(
                "search",
                processed_init.search.as_deref(),
                canonicalize_and_process::canonicalize_search,
                compile_options.clone(),
            )?
            .optionally_transpose_regex_error(report_regex_errors)?,
            hash: compile_component(
                "hash",
                processed_init.hash.as_deref(),
                canonicalize_and_process::canonicalize_hash,
                compile_options,
            )?
            .optionally_transpose_regex_error(report_regex_errors)?,
        })
    }

    /// The pattern used to match against the protocol of the URL.
    pub fn protocol(&self) -> &str {
        &self.protocol.pattern_string
    }

    /// The pattern used to match against the username of the URL.
    pub fn username(&self) -> &str {
        &self.username.pattern_string
    }

    /// The pattern used to match against the password of the URL.
    pub fn password(&self) -> &str {
        &self.password.pattern_string
    }

    /// The pattern used to match against the hostname of the URL.
    pub fn hostname(&self) -> &str {
        &self.hostname.pattern_string
    }

    /// The pattern used to match against the port of the URL.
    pub fn port(&self) -> &str {
        &self.port.pattern_string
    }

    /// The pattern used to match against the pathname of the URL.
    pub fn pathname(&self) -> &str {
        &self.pathname.pattern_string
    }

    /// The pattern used to match against the search string of the URL.
    pub fn search(&self) -> &str {
        &self.search.pattern_string
    }

    /// The pattern used to match against the hash fragment of the URL.
    pub fn hash(&self) -> &str {
        &self.hash.pattern_string
    }

    /// Returns whether the URLPattern contains one or more groups which uses regular expression matching.
    pub fn has_regexp_groups(&self) -> bool {
        self.protocol.has_regexp_group
            || self.username.has_regexp_group
            || self.password.has_regexp_group
            || self.hostname.has_regexp_group
            || self.port.has_regexp_group
            || self.pathname.has_regexp_group
            || self.search.has_regexp_group
            || self.hash.has_regexp_group
    }

    // Ref: https://wicg.github.io/urlpattern/#dom-urlpattern-test
    /// Test if a given [UrlPatternInput] (with optional base url), matches the
    /// pattern.
    pub fn test(&self, input: UrlPatternMatchInput) -> Result<bool, Error> {
        self.matches(input).map(|res| res.is_some())
    }

    // Ref: https://wicg.github.io/urlpattern/#dom-urlpattern-exec
    /// Execute the pattern against a [UrlPatternInput] (with optional base url),
    /// returning a [UrlPatternResult] if the pattern matches. If the pattern
    /// doesn't match, returns `None`.
    pub fn exec(&self, input: UrlPatternMatchInput) -> Result<Option<UrlPatternResult>, Error> {
        self.matches(input)
    }

    // Ref: https://wicg.github.io/urlpattern/#match
    fn matches(&self, input: UrlPatternMatchInput) -> Result<Option<UrlPatternResult>, Error> {
        let input = match quirks::parse_match_input(input) {
            Some(input) => input,
            None => return Ok(None),
        };

        let protocol_exec_result = self.protocol.matcher.matches(&input.protocol);
        let username_exec_result = self.username.matcher.matches(&input.username);
        let password_exec_result = self.password.matcher.matches(&input.password);
        let hostname_exec_result = self.hostname.matcher.matches(&input.hostname);
        let port_exec_result = self.port.matcher.matches(&input.port);
        let pathname_exec_result = self.pathname.matcher.matches(&input.pathname);
        let search_exec_result = self.search.matcher.matches(&input.search);
        let hash_exec_result = self.hash.matcher.matches(&input.hash);

        match (
            protocol_exec_result,
            username_exec_result,
            password_exec_result,
            hostname_exec_result,
            port_exec_result,
            pathname_exec_result,
            search_exec_result,
            hash_exec_result,
        ) {
            (
                Some(protocol_exec_result),
                Some(username_exec_result),
                Some(password_exec_result),
                Some(hostname_exec_result),
                Some(port_exec_result),
                Some(pathname_exec_result),
                Some(search_exec_result),
                Some(hash_exec_result),
            ) => Ok(Some(UrlPatternResult {
                protocol: self
                    .protocol
                    .create_match_result(input.protocol.clone(), protocol_exec_result),
                username: self
                    .username
                    .create_match_result(input.username.clone(), username_exec_result),
                password: self
                    .password
                    .create_match_result(input.password.clone(), password_exec_result),
                hostname: self
                    .hostname
                    .create_match_result(input.hostname.clone(), hostname_exec_result),
                port: self
                    .port
                    .create_match_result(input.port.clone(), port_exec_result),
                pathname: self
                    .pathname
                    .create_match_result(input.pathname.clone(), pathname_exec_result),
                search: self
                    .search
                    .create_match_result(input.search.clone(), search_exec_result),
                hash: self
                    .hash
                    .create_match_result(input.hash.clone(), hash_exec_result),
            })),
            _ => Ok(None),
        }
    }
}

// Ref: https://wicg.github.io/urlpattern/#hostname-pattern-is-an-ipv6-address
fn hostname_pattern_is_ipv6_address(input: &str) -> bool {
    // TODO: code point length
    if input.len() < 2 {
        return false;
    }

    input.starts_with('[') || input.starts_with("{[") || input.starts_with("\\[")
}

// Ref: https://wicg.github.io/urlpattern/#dictdef-urlpatternresult
/// A result of a URL pattern match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlPatternResult {
    pub protocol: UrlPatternComponentResult,
    pub username: UrlPatternComponentResult,
    pub password: UrlPatternComponentResult,
    pub hostname: UrlPatternComponentResult,
    pub port: UrlPatternComponentResult,
    pub pathname: UrlPatternComponentResult,
    pub search: UrlPatternComponentResult,
    pub hash: UrlPatternComponentResult,
}

// Ref: https://wicg.github.io/urlpattern/#dictdef-urlpatterncomponentresult
/// A result of a URL pattern match on a single component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlPatternComponentResult {
    /// The matched input for this component.
    pub input: String,
    /// The values for all named groups in the pattern, in pattern order.
    pub groups: Vec<(String, Option<String>)>,
}
