//! HTTP header types
//!
//! This module provides [`HeaderName`], [`HeaderMap`], [`OrigHeaderMap`], [`HeaderCaseName`], and a
//! number of types used for interacting with `HeaderMap`. These types allow representing both
//! HTTP/1 and HTTP/2 headers.

use bytes::Bytes;
use diting_wire_proto::ext::OnPreserveHeaderCallback;
pub use http::header::*;

/// Trait for types that can be converted into an [`HeaderCaseName`] (case-preserved header).
///
/// This trait is sealed, so only known types can implement it.
/// Supported types:
/// - `&'static str`
/// - `String`
/// - `Bytes`
/// - `HeaderName`
/// - `&HeaderName`
/// - `HeaderCaseName`
/// - `&HeaderCaseName`
pub trait IntoHeaderCaseName: sealed::Sealed {
    /// Converts the type into an [`HeaderCaseName`].
    fn into_header_case_name(self) -> HeaderCaseName;
}

/// A map from header names to their original casing as received in an HTTP message.
///
/// [`OrigHeaderMap`] not only preserves the original case of each header name as it appeared
/// in the request or response, but also maintains the insertion order of headers. This makes
/// it suitable for use cases where the order of headers matters, such as HTTP/1.x message
/// serialization, proxying, or reproducing requests/responses exactly as received.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrigHeaderMap(HeaderMap<HeaderCaseName>);

// ===== impl OrigHeaderMap =====

impl OrigHeaderMap {
    /// Creates a new, empty [`OrigHeaderMap`].
    #[inline]
    pub fn new() -> Self {
        Self(HeaderMap::default())
    }

    /// Creates an empty [`OrigHeaderMap`] with the specified capacity.
    #[inline]
    pub fn with_capacity(size: usize) -> Self {
        Self(HeaderMap::with_capacity(size))
    }

    /// Insert a new header name into the collection.
    ///
    /// If the map did not previously have this key present, then `false` is
    /// returned.
    ///
    /// If the map did have this key present, the new value is pushed to the end
    /// of the list of values currently associated with the key. The key is not
    /// updated, though; this matters for types that can be `==` without being
    /// identical.
    #[inline]
    pub fn insert<N>(&mut self, orig: N) -> bool
    where
        N: IntoHeaderCaseName,
    {
        let header_case_name = orig.into_header_case_name();
        match &header_case_name.inner {
            Repr::Cased(bytes) => HeaderName::from_bytes(bytes)
                .map(|header_name| self.0.append(header_name, header_case_name))
                .unwrap_or(false),
            Repr::Standard(header_name) => self.0.append(header_name.clone(), header_case_name),
        }
    }

    /// Extends the map with all entries from another [`OrigHeaderMap`], preserving order.
    #[inline]
    pub fn extend(&mut self, iter: OrigHeaderMap) {
        self.0.extend(iter.0);
    }

    /// Returns the number of headers stored in the map.
    ///
    /// This number represents the total number of **values** stored in the map.
    /// This number can be greater than or equal to the number of **keys**
    /// stored given that a single key may have more than one associated value.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the map contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over all header names and their original spellings, in insertion order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&HeaderName, &HeaderCaseName)> {
        self.0.iter()
    }
}

impl OnPreserveHeaderCallback for OrigHeaderMap {
    fn call(&self, headers: &mut HeaderMap) {
        if headers.len() <= 1 || self.0.is_empty() {
            return;
        }

        // Create a new header map to store the sorted headers
        let mut sorted_headers = HeaderMap::with_capacity(headers.keys_len());

        // First insert headers in the specified order
        for name in self.0.keys() {
            for value in headers.get_all(name) {
                sorted_headers.append(name.clone(), value.clone());
            }
            headers.remove(name);
        }

        // Then insert any remaining headers that were not ordered
        let mut prev_name: Option<HeaderName> = None;
        for (name, value) in headers.drain() {
            match (name, &prev_name) {
                (Some(name), _) => {
                    prev_name.replace(name.clone());
                    sorted_headers.insert(name, value);
                }
                (None, Some(prev_name)) => {
                    sorted_headers.append(prev_name, value);
                }
                _ => {}
            }
        }

        std::mem::swap(headers, &mut sorted_headers);
    }

    fn call_visit(
        &self,
        headers: &mut HeaderMap,
        dst: &mut dyn FnMut(&dyn AsRef<[u8]>, &http::HeaderValue),
    ) {
        // First, sort headers according to the order defined in this map
        for (name, case_name) in self.iter() {
            for value in headers.get_all(name) {
                dst(case_name, value);
            }

            headers.remove(name);
        }

        // After processing all ordered headers, append any remaining headers
        let mut prev_name: Option<HeaderCaseName> = None;
        for (name, value) in headers.drain() {
            match (name, &prev_name) {
                (Some(name), _) => {
                    dst(&name, &value);
                    prev_name.replace(name.into_header_case_name());
                }
                (None, Some(prev_name)) => {
                    dst(prev_name, &value);
                }
                _ => (),
            };
        }
    }
}

impl<'a> IntoIterator for &'a OrigHeaderMap {
    type Item = (&'a HeaderName, &'a HeaderCaseName);
    type IntoIter = <&'a HeaderMap<HeaderCaseName> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for OrigHeaderMap {
    type Item = (Option<HeaderName>, HeaderCaseName);
    type IntoIter = <HeaderMap<HeaderCaseName> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl_request_config_value!(OrigHeaderMap);

/// An HTTP header name with both normalized and original casing.
///
/// While HTTP headers are case-insensitive, this type stores both
/// the canonical [`HeaderName`] and the original casing as received,
/// useful for preserving header order and formatting in proxies,
/// debugging, or exact HTTP message reproduction.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct HeaderCaseName {
    inner: Repr,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum Repr {
    Cased(Bytes),
    Standard(HeaderName),
}

impl AsRef<[u8]> for HeaderCaseName {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match &self.inner {
            Repr::Standard(name) => name.as_ref(),
            Repr::Cased(orig) => orig.as_ref(),
        }
    }
}

impl IntoHeaderCaseName for &'static str {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        Bytes::from_static(self.as_bytes()).into_header_case_name()
    }
}

impl IntoHeaderCaseName for String {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        Bytes::from(self).into_header_case_name()
    }
}

impl IntoHeaderCaseName for Bytes {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        HeaderCaseName {
            inner: Repr::Cased(self),
        }
    }
}

impl IntoHeaderCaseName for &HeaderName {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        HeaderCaseName {
            inner: Repr::Standard(self.clone()),
        }
    }
}

impl IntoHeaderCaseName for HeaderName {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        HeaderCaseName {
            inner: Repr::Standard(self),
        }
    }
}

impl IntoHeaderCaseName for HeaderCaseName {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        self
    }
}

impl IntoHeaderCaseName for &HeaderCaseName {
    #[inline]
    fn into_header_case_name(self) -> HeaderCaseName {
        self.clone()
    }
}

mod sealed {

    use bytes::Bytes;
    use http::HeaderName;

    use crate::header::HeaderCaseName;

    pub trait Sealed {}

    impl Sealed for &'static str {}
    impl Sealed for String {}
    impl Sealed for Bytes {}
    impl Sealed for &HeaderName {}
    impl Sealed for HeaderName {}
    impl Sealed for &HeaderCaseName {}
    impl Sealed for HeaderCaseName {}
}
