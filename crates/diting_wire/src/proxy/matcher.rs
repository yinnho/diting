//! Proxy matchers
//!
//! This module contains different matchers to configure rules for when a proxy
//! should be used, and if so, with what arguments.
//!
//! A [`Matcher`] can be constructed either using environment variables, or
//! a [`Matcher::builder()`].
//!
//! Once constructed, the `Matcher` can be asked if it intercepts a `Uri` by
//! calling [`Matcher::intercept()`].
//!
//! An [`Intercept`] includes the destination for the proxy, and any parsed
//! authentication to be used.

use std::net::IpAddr;
#[cfg(unix)]
use std::{path::Path, sync::Arc};

use bytes::Bytes;
use http::{
    HeaderMap, Uri,
    header::HeaderValue,
    uri::{Authority, Scheme},
};
use ipnet::IpNet;
use percent_encoding::percent_decode_str;

use self::builder::IntoValue;
use super::{Extra, Intercepted};
use crate::ext::UriExt;

/// A proxy matcher, usually built from environment variables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Matcher {
    http: Option<Intercept>,
    https: Option<Intercept>,
    no: NoProxy,
    #[cfg(unix)]
    unix: Option<Arc<Path>>,
}

/// A matched proxy,
///
/// This is returned by a matcher if a proxy should be used.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Intercept {
    uri: Uri,
    auth: Auth,
    extra: Extra,
}

/// A builder to create a [`Matcher`].
///
/// Construct with [`Matcher::builder()`].
#[derive(Default)]
pub struct Builder {
    pub(super) is_cgi: bool,
    pub(super) all: String,
    pub(super) http: String,
    pub(super) https: String,
    pub(super) no: String,
    #[cfg(unix)]
    pub(super) unix: Option<Arc<Path>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Auth {
    Empty,
    Basic(HeaderValue),
    Raw(Bytes, Bytes),
}

/// A filter for proxy matchers.
///
/// This type is based off the `NO_PROXY` rules used by curl.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct NoProxy {
    ips: IpMatcher,
    domains: DomainMatcher,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct DomainMatcher(Vec<String>);

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct IpMatcher(Vec<Ip>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Ip {
    Address(IpAddr),
    Network(IpNet),
}

// ===== impl Matcher =====

impl Matcher {
    /// Create a matcher from the environment or system.
    ///
    /// This checks the same environment variables as `from_env()`, and if not
    /// set, checks the system configuration for values for the OS.
    ///
    /// This constructor is always available, but if the `client-proxy-system`
    /// feature is enabled, it will check more configuration. Use this
    /// constructor if you want to allow users to optionally enable more, or
    /// use `from_env` if you do not want the values to change based on an
    /// enabled feature.
    pub fn from_system() -> Self {
        Builder::from_system().build(Extra::default())
    }

    /// Start a builder to configure a matcher.
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Check if the destination should be intercepted by a proxy.
    ///
    /// If the proxy rules match the destination, a new `Uri` will be returned
    /// to connect to.
    pub fn intercept(&self, dst: &Uri) -> Option<Intercepted> {
        // if unix sockets are configured, check them first
        #[cfg(unix)]
        if let Some(unix) = &self.unix {
            return Some(Intercepted::Unix(unix.clone()));
        }

        // TODO(perf): don't need to check `no` if below doesn't match...
        if self.no.contains(dst.host()?) {
            return None;
        }
        if dst.is_http() {
            return self.http.clone().map(Intercepted::Proxy);
        }

        if dst.is_https() {
            return self.https.clone().map(Intercepted::Proxy);
        }

        None
    }
}

// ===== impl Intercept =====

impl Intercept {
    #[inline]
    pub(crate) fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(crate) fn basic_auth(&self) -> Option<&HeaderValue> {
        if let Some(ref val) = self.extra.auth {
            return Some(val);
        }

        if let Auth::Basic(ref val) = self.auth {
            Some(val)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn custom_headers(&self) -> Option<&HeaderMap> {
        self.extra.misc.as_ref()
    }

    #[cfg(feature = "socks")]
    pub(crate) fn raw_auth(&self) -> Option<(Bytes, Bytes)> {
        if let Auth::Raw(ref u, ref p) = self.auth {
            Some((u.clone(), p.clone()))
        } else {
            None
        }
    }
}

// ===== impl Builder =====

impl Builder {
    fn from_env() -> Self {
        Builder {
            is_cgi: std::env::var_os("REQUEST_METHOD").is_some(),
            all: get_first_env(&["ALL_PROXY", "all_proxy"]),
            http: get_first_env(&["HTTP_PROXY", "http_proxy"]),
            https: get_first_env(&["HTTPS_PROXY", "https_proxy"]),
            no: get_first_env(&["NO_PROXY", "no_proxy"]),
            #[cfg(unix)]
            unix: None,
        }
    }

    fn from_system() -> Self {
        #[allow(unused_mut)]
        let mut builder = Self::from_env();



        builder
    }

    /// Set the target proxy for all destinations.
    pub fn all<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.all = val.into_value();
        self
    }

    /// Set the target proxy for HTTP destinations.
    pub fn http<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.http = val.into_value();
        self
    }

    /// Set the target proxy for HTTPS destinations.
    pub fn https<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.https = val.into_value();
        self
    }

    /// Set the "no" proxy filter.
    ///
    /// The rules are as follows:
    /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
    /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding
    ///   /size, for example "`192.168.1.0/24`").
    /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
    /// * Any other entry is considered a domain name (and may contain a leading dot, for example
    ///   `google.com` and `.google.com` are equivalent) and would match both that domain AND all
    ///   subdomains.
    ///
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would
    /// match (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URI `http://notgoogle.com/` would not match.
    pub fn no<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.no = val.into_value();
        self
    }

    // / Set the unix socket target proxy for all destinations.
    #[cfg(unix)]
    pub fn unix<S>(mut self, val: S) -> Self
    where
        S: super::uds::IntoUnixSocket,
    {
        self.unix = Some(val.unix_socket());
        self
    }

    /// Construct a [`Matcher`] using the configured values.
    pub(super) fn build(self, extra: Extra) -> Matcher {
        if self.is_cgi {
            return Matcher {
                http: None,
                https: None,
                no: NoProxy::empty(),
                #[cfg(unix)]
                unix: None,
            };
        }

        let mut all = parse_env_uri(&self.all);
        let mut http = parse_env_uri(&self.http);
        let mut https = parse_env_uri(&self.https);

        if let Some(http) = http.as_mut() {
            http.extra = extra.clone();
        }
        if let Some(https) = https.as_mut() {
            https.extra = extra.clone();
        }
        if http.is_none() || https.is_none() {
            if let Some(all) = all.as_mut() {
                all.extra = extra;
            }
        }

        Matcher {
            http: http.or_else(|| all.clone()),
            https: https.or(all),
            no: NoProxy::from_string(&self.no),
            #[cfg(unix)]
            unix: self.unix,
        }
    }
}

fn get_first_env(names: &[&str]) -> String {
    for name in names {
        if let Ok(val) = std::env::var(name) {
            return val;
        }
    }

    String::new()
}

fn parse_env_uri(val: &str) -> Option<Intercept> {
    let uri = val.parse::<Uri>().ok()?;
    let mut builder = Uri::builder();
    let mut is_httpish = false;
    let mut is_socks = false;
    let mut auth = Auth::Empty;

    builder = builder.scheme(match uri.scheme() {
        Some(s) => {
            if s == &Scheme::HTTP || s == &Scheme::HTTPS {
                is_httpish = true;
                s.clone()
            } else if matches!(s.as_str(), "socks4" | "socks4a" | "socks5" | "socks5h") {
                is_socks = true;
                s.clone()
            } else {
                // can't use this proxy scheme
                return None;
            }
        }
        // if no scheme provided, assume they meant 'http'
        None => {
            is_httpish = true;
            Scheme::HTTP
        }
    });

    let authority = {
        let authority = uri.authority()?;
        // default SOCKS port to 1080 if missing
        if is_socks && authority.port().is_none() {
            Authority::from_maybe_shared(Bytes::from(format!("{authority}:1080"))).ok()?
        } else {
            authority.clone()
        }
    };

    if let Some((userinfo, host_port)) = authority.as_str().rsplit_once('@') {
        let (user, pass) = match userinfo.split_once(':') {
            Some((user, pass)) => (user, Some(pass)),
            None => (userinfo, None),
        };

        let user = percent_decode_str(user).decode_utf8_lossy();
        let pass = pass.map(|pass| percent_decode_str(pass).decode_utf8_lossy());
        if is_httpish {
            auth = Auth::Basic(crate::util::basic_auth(&user, pass.as_deref()));
        } else {
            auth = Auth::Raw(
                Bytes::from(user.into_owned()),
                Bytes::from(pass.map_or_else(String::new, std::borrow::Cow::into_owned)),
            );
        }
        builder = builder.authority(host_port);
    } else {
        builder = builder.authority(authority);
    }

    // removing any path, but we MUST specify one or the builder errors
    builder = builder.path_and_query("/");

    Some(Intercept {
        auth,
        extra: Extra::default(),
        uri: builder.build().ok()?,
    })
}

impl NoProxy {
    fn empty() -> NoProxy {
        NoProxy {
            ips: IpMatcher(Vec::new()),
            domains: DomainMatcher(Vec::new()),
        }
    }

    /// Returns a new no-proxy configuration based on a `no_proxy` string (or `None` if no variables
    /// are set)
    /// The rules are as follows:
    /// * The environment variable `NO_PROXY` is checked, if it is not set, `no_proxy` is checked
    /// * If neither environment variable is set, `None` is returned
    /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
    /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding
    ///   /size, for example "`192.168.1.0/24`").
    /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
    /// * Any other entry is considered a domain name (and may contain a leading dot, for example
    ///   `google.com` and `.google.com` are equivalent) and would match both that domain AND all
    ///   subdomains.
    ///
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would
    /// match (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URI `http://notgoogle.com/` would not match.
    pub fn from_string(no_proxy_list: &str) -> Self {
        let mut ips = Vec::new();
        let mut domains = Vec::new();
        let parts = no_proxy_list.split(',').map(str::trim);
        for part in parts {
            match part.parse::<IpNet>() {
                // If we can parse an IP net or address, then use it, otherwise, assume it is a
                // domain
                Ok(ip) => ips.push(Ip::Network(ip)),
                Err(_) => match part.parse::<IpAddr>() {
                    Ok(addr) => ips.push(Ip::Address(addr)),
                    Err(_) => {
                        if !part.trim().is_empty() {
                            domains.push(part.to_owned())
                        }
                    }
                },
            }
        }
        NoProxy {
            ips: IpMatcher(ips),
            domains: DomainMatcher(domains),
        }
    }

    /// Return true if this matches the host (domain or IP).
    pub fn contains(&self, host: &str) -> bool {
        let host = crate::util::strip_ipv6_brackets(host);
        match host.parse::<IpAddr>() {
            // If we can parse an IP addr, then use it, otherwise, assume it is a domain
            Ok(ip) => self.ips.contains(ip),
            Err(_) => self.domains.contains(host),
        }
    }
}

impl IpMatcher {
    fn contains(&self, addr: IpAddr) -> bool {
        for ip in &self.0 {
            match ip {
                Ip::Address(address) => {
                    if &addr == address {
                        return true;
                    }
                }
                Ip::Network(net) => {
                    if net.contains(&addr) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl DomainMatcher {
    // The following links may be useful to understand the origin of these rules:
    // * https://curl.se/libcurl/c/CURLOPT_NOPROXY.html
    // * https://github.com/curl/curl/issues/1208
    fn contains(&self, domain: &str) -> bool {
        let domain_len = domain.len();
        for d in &self.0 {
            if d.eq_ignore_ascii_case(domain)
                || d.strip_prefix('.')
                    .is_some_and(|s| s.eq_ignore_ascii_case(domain))
            {
                return true;
            } else if domain
                .get(domain_len.saturating_sub(d.len())..)
                .is_some_and(|s| s.eq_ignore_ascii_case(d))
            {
                if d.starts_with('.') {
                    // If the first character of d is a dot, that means the first character of
                    // domain must also be a dot, so we are looking at a
                    // subdomain of d and that matches
                    return true;
                } else if domain
                    .as_bytes()
                    .get(domain_len.saturating_sub(d.len() + 1))
                    == Some(&b'.')
                {
                    // Given that d is a prefix of domain, if the prior character in domain is a dot
                    // then that means we must be matching a subdomain of d, and that matches
                    return true;
                }
            } else if d == "*" {
                return true;
            }
        }
        false
    }
}

mod builder {
    /// A type that can used as a `Builder` value.
    ///
    /// Private and sealed, only visible in docs.
    pub trait IntoValue {
        #[doc(hidden)]
        fn into_value(self) -> String;
    }

    impl IntoValue for String {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self
        }
    }

    impl IntoValue for &String {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self.into()
        }
    }

    impl IntoValue for &str {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self.into()
        }
    }
}
