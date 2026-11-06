use bytes::Bytes;
use cookie::{
    Cookie as RawCookie,
    time::{Duration, OffsetDateTime},
};
use http::{Uri, Version};

use super::{
    Cookie, CookieStore, Cookies, IntoCookie,
    store::{DEFAULT_PATH, Store, canonical_host, cookie_is_expired, domain_match, normalize_path},
};
use crate::{IntoUri, ext::UriExt, header::HeaderValue, sync::RwLock};

/// A good default `CookieStore` implementation.
///
/// This is the implementation used when simply calling `cookie_store(true)`.
/// This type is exposed to allow creating one and filling it with some
/// existing cookies more easily, before creating a [`crate::Client`].
#[derive(Debug, Default)]
pub struct Jar(RwLock<Store>);

macro_rules! into_uri {
    ($expr:expr) => {
        match $expr.into_uri() {
            Ok(u) => u,
            Err(_) => return,
        }
    };
}

impl Jar {
    /// Returns an unexpired cookie by name for an exact URI scope.
    ///
    /// The URI host is canonicalized and its path is used as the exact stored cookie path. Use
    /// [`matches`](Self::matches) to select every cookie that would apply to a request URI through
    /// RFC domain and path matching. When both storage scopes contain the name, the older cookie is
    /// returned.
    ///
    /// # Example
    /// ```
    /// use diting_wire::cookie::Jar;
    /// let jar = Jar::default();
    /// jar.add("foo=bar; Path=/foo; Domain=example.com", "http://example.com/foo");
    /// let cookie = jar.get("foo", "http://example.com/foo").unwrap();
    /// assert_eq!(cookie.value(), "bar");
    /// ```
    pub fn get<U: IntoUri>(&self, name: &str, uri: U) -> Option<Cookie<'static>> {
        let uri = uri.into_uri().ok()?;
        let host = canonical_host(uri.host()?)?;
        let now = OffsetDateTime::now_utc();
        let store = self.0.read();
        let cookie = store
            .cookies
            .get(&host)?
            .get(uri.path())?
            .entries(name)
            .filter(|entry| !cookie_is_expired(&entry.cookie, now))
            .min_by_key(|entry| entry.creation_index)?;

        Some(Cookie::from(cookie.cookie.clone()))
    }

    /// Returns whether an unexpired cookie exists for an exact URI scope.
    ///
    /// This performs the same scope lookup as [`get`](Self::get) without cloning the cookie.
    pub fn contains<U: IntoUri>(&self, name: &str, uri: U) -> bool {
        let Ok(uri) = uri.into_uri() else {
            return false;
        };
        let Some(host) = uri.host().and_then(canonical_host) else {
            return false;
        };
        let now = OffsetDateTime::now_utc();

        self.0
            .read()
            .cookies
            .get(&host)
            .and_then(|path_map| path_map.get(uri.path()))
            .is_some_and(|cookie_map| {
                cookie_map
                    .entries(name)
                    .any(|entry| !cookie_is_expired(&entry.cookie, now))
            })
    }

    /// Returns all unexpired cookies in the jar.
    ///
    /// The returned cookies are owned snapshots with their effective stored `Path`. Host-only
    /// cookies keep their `Domain` attribute absent, so importing a snapshot into another jar does
    /// not broaden its domain scope. Snapshots are returned in creation order.
    ///
    /// # Example
    /// ```
    /// use diting_wire::cookie::Jar;
    /// let jar = Jar::default();
    /// jar.add("foo=bar; Domain=example.com", "http://example.com");
    /// for cookie in jar.get_all() {
    ///     println!("{}={}", cookie.name(), cookie.value());
    /// }
    /// ```
    pub fn get_all(&self) -> impl Iterator<Item = Cookie<'static>> {
        let now = OffsetDateTime::now_utc();
        let mut cookies = self
            .0
            .read()
            .cookies
            .values()
            .flat_map(|path_map| {
                path_map.values().flat_map(|cookie_map| {
                    cookie_map.values().filter_map(|entry| {
                        if cookie_is_expired(&entry.cookie, now) {
                            return None;
                        }

                        Some((entry.creation_index, Cookie::from(entry.cookie.clone())))
                    })
                })
            })
            .collect::<Vec<_>>();
        cookies.sort_unstable_by_key(|(creation_index, _)| *creation_index);
        cookies.into_iter().map(|(_, cookie)| cookie)
    }

    /// Returns the number of unexpired cookies in the jar.
    ///
    /// Expired cookies are excluded even if their internal entries have not yet been overwritten or
    /// cleared.
    pub fn len(&self) -> usize {
        let now = OffsetDateTime::now_utc();
        self.0
            .read()
            .cookies
            .values()
            .flat_map(|path_map| path_map.values())
            .flat_map(|cookie_map| cookie_map.values())
            .filter(|entry| !cookie_is_expired(&entry.cookie, now))
            .count()
    }

    /// Returns `true` when the jar has no unexpired cookies.
    pub fn is_empty(&self) -> bool {
        let now = OffsetDateTime::now_utc();
        !self
            .0
            .read()
            .cookies
            .values()
            .flat_map(|path_map| path_map.values())
            .flat_map(|cookie_map| cookie_map.values())
            .any(|entry| !cookie_is_expired(&entry.cookie, now))
    }

    /// Returns the unexpired cookies that apply to an HTTP or HTTPS request URI.
    ///
    /// Selection applies domain matching, path matching, host-only scope, the `Secure` attribute,
    /// and expiration rules from the [RFC 6265 retrieval model]. Returned cookies are owned
    /// snapshots that preserve host-only scope. Unsupported URI schemes do not match any cookies.
    ///
    /// # Example
    ///
    /// ```
    /// use diting_wire::cookie::Jar;
    ///
    /// let jar = Jar::default();
    /// jar.add("root=1; Domain=example.com; Path=/", "https://example.com/");
    /// jar.add("api=2; Domain=example.com; Path=/api", "https://example.com/api");
    ///
    /// let cookies = jar
    ///     .matches("https://www.example.com/api/users")
    ///     .collect::<Vec<_>>();
    /// assert_eq!(cookies.len(), 2);
    /// assert!(cookies.iter().any(|cookie| cookie.name() == "root"));
    /// assert!(cookies.iter().any(|cookie| cookie.name() == "api"));
    /// ```
    ///
    /// [RFC 6265 retrieval model]: https://www.rfc-editor.org/rfc/rfc6265.html#section-5.4
    pub fn matches<U: IntoUri>(&self, uri: U) -> impl Iterator<Item = Cookie<'static>> {
        let Ok(uri) = uri.into_uri() else {
            return Vec::new().into_iter();
        };
        let Some(host) = uri.host().and_then(canonical_host) else {
            return Vec::new().into_iter();
        };

        let now = OffsetDateTime::now_utc();
        let store = self.0.read();
        store
            .matching_cookies(&uri, &host, now)
            .map(|(_, _, entry)| Cookie::from(entry.cookie.clone()))
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Stores a cookie received from a URI.
    ///
    /// The URI supplies the host-only domain and default path when those attributes are absent.
    /// Cookies with an invalid URI or an invalid or mismatched `Domain` are ignored. A non-positive
    /// `Max-Age` removes an existing cookie in the same scope, as required by the
    /// [RFC 6265 storage model]. A positive `Max-Age` is stored as an absolute deadline. Insecure
    /// origins cannot set `Secure` cookies or overlay an existing `Secure` cookie, following the
    /// [RFC 6265bis storage model].
    ///
    /// # Example
    ///
    /// ```
    /// use diting_wire::cookie::Jar;
    /// use cookie::CookieBuilder;
    /// let jar = Jar::default();
    /// let cookie = CookieBuilder::new("foo", "bar")
    ///     .domain("example.com")
    ///     .path("/")
    ///     .build();
    /// jar.add(cookie, "http://example.com");
    /// ```
    ///
    /// [RFC 6265 storage model]: https://www.rfc-editor.org/rfc/rfc6265.html#section-5.3
    /// [RFC 6265bis storage model]: https://httpwg.org/http-extensions/draft-ietf-httpbis-rfc6265bis.html#section-5.7
    pub fn add<C, U>(&self, cookie: C, uri: U)
    where
        C: IntoCookie,
        U: IntoUri,
    {
        if let Some(cookie) = cookie.into_cookie() {
            let uri = into_uri!(uri);
            let mut cookie: RawCookie<'static> = cookie.into();
            let secure_origin = uri.is_https();

            if cookie.secure() == Some(true) && !secure_origin {
                return;
            }

            // If the request-uri contains no host component:
            let Some(host) = uri.host().and_then(canonical_host) else {
                return;
            };
            let host_only = cookie.domain().is_none();

            // If the canonicalized request-host does not domain-match the
            // domain-attribute:
            //    Ignore the cookie entirely and abort these steps.
            //
            // RFC 6265 §5.3 + §5.1.3:
            // https://datatracker.ietf.org/doc/html/rfc6265#section-5.3
            // https://datatracker.ietf.org/doc/html/rfc6265#section-5.1.3
            let domain = if let Some(raw_domain) = cookie.domain() {
                let Some(domain) = canonical_host(raw_domain) else {
                    return;
                };
                if !domain_match(&host, &domain) {
                    return;
                }

                cookie.set_domain(domain.to_string());
                domain
            } else {
                host
            };

            // Max-Age takes precedence over Expires and is relative to when the cookie is
            // received. Store its effective deadline so every read path applies the same
            // expiration decision. RFC 6265 sections 5.2.2 and 5.3:
            // https://www.rfc-editor.org/rfc/rfc6265.html#section-5.2.2
            // https://www.rfc-editor.org/rfc/rfc6265.html#section-5.3
            let now = OffsetDateTime::now_utc();
            let expired = match cookie.max_age() {
                Some(max_age) if max_age <= Duration::ZERO => true,
                Some(max_age) => {
                    let deadline = now.saturating_add(max_age);
                    cookie.set_max_age(None);
                    cookie.set_expires(deadline);
                    false
                }
                None => cookie
                    .expires_datetime()
                    .is_some_and(|deadline| deadline <= now),
            };

            // If the request-uri contains no path component or if the first character of the
            // path component of the request-uri is not a %x2F ("/") OR if the cookie's path-
            // attribute is missing or does not start with a %x2F ("/"):
            //    Let cookie-path be the default-path of the request-uri.
            // Otherwise:
            //    Let cookie-path be the substring of the request-uri's path from the first
            // character    up to, not including, the right-most %x2F ("/").
            //
            // RFC 6265 §5.2.4 + §5.1.4:
            // https://datatracker.ietf.org/doc/html/rfc6265#section-5.2.4
            // https://datatracker.ietf.org/doc/html/rfc6265#section-5.1.4
            let path = cookie
                .path()
                .filter(|path| path.starts_with(DEFAULT_PATH))
                .unwrap_or_else(|| normalize_path(uri.path()))
                .to_owned();

            let mut store = self.0.write();
            if !secure_origin
                && store.would_overlay_secure_cookie(cookie.name(), &domain, &path, now)
            {
                return;
            }

            if expired {
                store.remove_stored_cookie(&domain, &path, cookie.name(), host_only);
            } else {
                cookie.set_path(path.clone());
                store.insert_stored_cookie(domain, path, cookie);
            }
        }
    }

    /// Removes a cookie from an exact URI scope.
    ///
    /// The URI host and path identify the stored scope. Both host-only and `Domain` cookies with
    /// the same name in that scope are removed; other domains and paths are left unchanged.
    ///
    /// # Example
    /// ```
    /// use diting_wire::cookie::Jar;
    /// let jar = Jar::default();
    /// jar.add("foo=bar; Path=/foo; Domain=example.com", "http://example.com/foo");
    /// assert!(jar.get("foo", "http://example.com/foo").is_some());
    /// jar.remove("foo", "http://example.com/foo");
    /// assert!(jar.get("foo", "http://example.com/foo").is_none());
    /// ```
    pub fn remove<C, U>(&self, cookie: C, uri: U)
    where
        C: Into<RawCookie<'static>>,
        U: IntoUri,
    {
        let uri = into_uri!(uri);
        if let Some(host) = uri.host() {
            let Some(host) = canonical_host(host) else {
                return;
            };
            let cookie = cookie.into();
            let mut store = self.0.write();
            store.remove_stored_cookies(&host, uri.path(), cookie.name());
        }
    }

    /// Removes every cookie from the jar, leaving it empty.
    ///
    /// # Example
    /// ```
    /// use diting_wire::cookie::Jar;
    /// let jar = Jar::default();
    /// jar.add("foo=bar; Domain=example.com", "http://example.com");
    /// assert_eq!(jar.get_all().count(), 1);
    /// jar.clear();
    /// assert_eq!(jar.get_all().count(), 0);
    /// ```
    pub fn clear(&self) {
        *self.0.write() = Store::default();
    }
}

impl CookieStore for Jar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, uri: &Uri) {
        let cookies = cookie_headers
            .map(Cookie::parse)
            .filter_map(Result::ok)
            .map(|cookie| RawCookie::from(cookie).into_owned());

        for cookie in cookies {
            self.add(cookie, uri);
        }
    }

    fn cookies(&self, uri: &Uri, version: Version) -> Cookies {
        let host = match uri.host() {
            Some(host) => match canonical_host(host) {
                Some(host) => host,
                None => return Cookies::Empty,
            },
            None => return Cookies::Empty,
        };

        let now = OffsetDateTime::now_utc();
        let store = self.0.read();
        let mut matches = store.matching_cookies(uri, &host, now).collect::<Vec<_>>();

        // Chromium sorts cookies selected for a request by longest path first, then oldest
        // creation time. Cookie names and domains do not participate in the comparison.
        // RFC 6265 section 5.4: https://www.rfc-editor.org/rfc/rfc6265.html#section-5.4
        // Chromium: https://chromium.googlesource.com/chromium/src/+/main/net/cookies/cookie_monster.cc
        matches.sort_unstable_by(|(_, left_path, left), (_, right_path, right)| {
            right_path
                .len()
                .cmp(&left_path.len())
                .then_with(|| left.creation_index.cmp(&right.creation_index))
        });

        let iter = matches.into_iter().map(|(_, _, entry)| &entry.cookie);

        if matches!(version, Version::HTTP_2 | Version::HTTP_3) {
            let cookies = iter
                .map(|cookie| {
                    let name = cookie.name();
                    let value = cookie.value();

                    let mut cookie_str = String::with_capacity(name.len() + 1 + value.len());
                    cookie_str.push_str(name);
                    cookie_str.push('=');
                    cookie_str.push_str(value);

                    HeaderValue::from_maybe_shared(Bytes::from(cookie_str))
                })
                .filter_map(Result::ok)
                .collect();

            Cookies::Uncompressed(cookies)
        } else {
            let cookies = iter.fold(String::new(), |mut cookies, cookie| {
                if !cookies.is_empty() {
                    cookies.push_str("; ");
                }
                cookies.push_str(cookie.name());
                cookies.push('=');
                cookies.push_str(cookie.value());
                cookies
            });

            if cookies.is_empty() {
                return Cookies::Empty;
            }

            HeaderValue::from_maybe_shared(Bytes::from(cookies))
                .map(Cookies::Compressed)
                .unwrap_or(Cookies::Empty)
        }
    }
}
