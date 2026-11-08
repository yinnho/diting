//! URI conversion utilities.
//!
//! This module provides the [`IntoUri`] trait, allowing various types
//! (such as `&str`, `String`, `Vec<u8>`, etc.) to be fallibly converted into an [`http::Uri`].
//! The conversion is based on `TryFrom<T> for Uri` and ensures the resulting URI is valid and
//! contains a host.
//!
//! Internally, the trait is sealed to prevent

use bytes::Bytes;
use http::Uri;

use crate::{Error, Result};

/// Converts a value into a [`Uri`] with error handling.
///
/// This trait is implemented for common types such as [`Uri`], [`String`], [`&str`], and byte
/// slices, as well as any type that can be fallibly converted into a [`Uri`] via [`TryFrom`].
pub trait IntoUri: IntoUriSealed {}

impl IntoUri for Uri {}
impl IntoUri for &Uri {}
impl IntoUri for &str {}
impl IntoUri for String {}
impl IntoUri for &String {}
impl IntoUri for Vec<u8> {}
impl IntoUri for &[u8] {}
impl IntoUri for url::Url {}
impl IntoUri for &url::Url {}

pub trait IntoUriSealed {
    // Besides parsing as a valid `Uri`.
    fn into_uri(self) -> Result<Uri>;
}

impl IntoUriSealed for &[u8] {
    fn into_uri(self) -> Result<Uri> {
        Uri::try_from(self)
            .or_else(|_| internal::parse(internal::Kind::Bytes(self)))
            .and_then(IntoUriSealed::into_uri)
    }
}

impl IntoUriSealed for Vec<u8> {
    fn into_uri(self) -> Result<Uri> {
        let bytes = Bytes::from(self);
        Uri::from_maybe_shared(bytes.clone())
            .or_else(|_| internal::parse(internal::Kind::Bytes(&bytes)))
            .and_then(IntoUriSealed::into_uri)
    }
}

impl IntoUriSealed for &str {
    fn into_uri(self) -> Result<Uri> {
        Uri::try_from(self)
            .or_else(|_| internal::parse(internal::Kind::Str(self)))
            .and_then(IntoUriSealed::into_uri)
    }
}

impl IntoUriSealed for String {
    #[inline]
    fn into_uri(self) -> Result<Uri> {
        self.into_bytes().into_uri()
    }
}

impl IntoUriSealed for &String {
    #[inline]
    fn into_uri(self) -> Result<Uri> {
        IntoUriSealed::into_uri(self.as_str())
    }
}

// A parsed `Url` serializes to valid ASCII (punycode host, percent-encoded
// path), so the plain `Uri` parse never needs the fallback.
impl IntoUriSealed for &url::Url {
    fn into_uri(self) -> Result<Uri> {
        IntoUriSealed::into_uri(self.as_str())
    }
}

impl IntoUriSealed for url::Url {
    #[inline]
    fn into_uri(self) -> Result<Uri> {
        IntoUriSealed::into_uri(&self)
    }
}

impl IntoUriSealed for Uri {
    fn into_uri(self) -> Result<Uri> {
        match (self.scheme(), self.authority()) {
            (Some(_), Some(_)) => Ok(self),
            _ => Err(Error::uri_bad_scheme(self)),
        }
    }
}

impl IntoUriSealed for &Uri {
    fn into_uri(self) -> Result<Uri> {
        match (self.scheme(), self.authority()) {
            (Some(_), Some(_)) => Ok(self.clone()),
            _ => Err(Error::uri_bad_scheme(self.clone())),
        }
    }
}

mod internal {
    use http::Uri;
    use url::Url;

    use crate::{Error, Result};

    pub(super) enum Kind<'a> {
        Bytes(&'a [u8]),
        Str(&'a str),
    }

    pub(super) fn parse(s: Kind) -> Result<Uri> {
        let s = match s {
            Kind::Bytes(bytes) => std::str::from_utf8(bytes).map_err(Error::decode),
            Kind::Str(s) => Ok(s),
        }?;

        Url::parse(s)
            .map(String::from)
            .map_err(Error::builder)
            .and_then(|s| Uri::try_from(s).map_err(Error::builder))
    }
}
