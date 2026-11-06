//! SOCKS4/5 CONNECT tunneling over tokio streams.
//!
//! Vendored from tokio-socks 0.5.3 (MIT, see LICENSE) and pruned to the two
//! faces this repository actually uses:
//!
//! - `Socks5Stream::connect` — dial a target through a socks5 proxy given
//!   the proxy's `host:port` (the diting WebSocket dial);
//! - the `*_with_socket` family — run the handshake over a caller-supplied
//!   socket (diting_wire's SOCKS connector, which owns its own dialing).
//!
//! Dropped on intake: BIND listeners, Tor RESOLVE/RESOLVE_PTR commands,
//! the futures-io / tokio dual-backend `io` module (sockets here are
//! tokio `AsyncRead + AsyncWrite`), and the stream-flavored proxy-address
//! resolution (`ToProxyAddrs` — `connect` resolves the proxy with std
//! `to_socket_addrs`, exactly what upstream's stream did on first poll).
//! thiserror was bumped 1 → 2 (the reason this crate was vendored at all:
//! upstream 0.5.3 is the last release pinning the thiserror 1 line).

use std::{
    borrow::Cow,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

pub mod tcp;

pub use tcp::{Socks4Stream, Socks5Stream};

pub type Result<T> = std::result::Result<T, Error>;

/// Error type of the SOCKS handshakes.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Failure caused by an IO error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// Failure due to invalid target address. It contains the detailed error
    /// message.
    #[error("Target address is invalid: {0}")]
    InvalidTargetAddress(&'static str),
    /// Proxy server unreachable.
    #[error("Proxy server unreachable")]
    ProxyServerUnreachable,
    /// Proxy server returns an invalid version number.
    #[error("Invalid response version")]
    InvalidResponseVersion,
    /// No acceptable auth methods
    #[error("No acceptable auth methods")]
    NoAcceptableAuthMethods,
    /// Unknown auth method
    #[error("Unknown auth method")]
    UnknownAuthMethod,
    /// General SOCKS server failure
    #[error("General SOCKS server failure")]
    GeneralSocksServerFailure,
    /// Connection not allowed by ruleset
    #[error("Connection not allowed by ruleset")]
    ConnectionNotAllowedByRuleset,
    /// Network unreachable
    #[error("Network unreachable")]
    NetworkUnreachable,
    /// Host unreachable
    #[error("Host unreachable")]
    HostUnreachable,
    /// Connection refused
    #[error("Connection refused")]
    ConnectionRefused,
    /// TTL expired
    #[error("TTL expired")]
    TtlExpired,
    /// Command not supported
    #[error("Command not supported")]
    CommandNotSupported,
    /// Address type not supported
    #[error("Address type not supported")]
    AddressTypeNotSupported,
    /// Unknown error
    #[error("Unknown error")]
    UnknownError,
    /// Invalid reserved byte
    #[error("Invalid reserved byte")]
    InvalidReservedByte,
    /// Unknown address type
    #[error("Unknown address type")]
    UnknownAddressType,
    /// Invalid authentication values. It contains the detailed error message.
    #[error("Invalid auth values: {0}")]
    InvalidAuthValues(&'static str),
    /// Password auth failure
    #[error("Password auth failure, code: {0}")]
    PasswordAuthFailure(u8),

    #[error("Authorization required")]
    AuthorizationRequired,

    #[error("Request rejected because SOCKS server cannot connect to identd on the client")]
    IdentdAuthFailure,

    #[error("Request rejected because the client program and identd report different user-ids")]
    InvalidUserIdAuthFailure,
}

/// A SOCKS connection target.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TargetAddr<'a> {
    /// Connect to an IP address.
    Ip(SocketAddr),

    /// Connect to a fully-qualified domain name.
    ///
    /// The domain name will be passed along to the proxy server and DNS lookup
    /// will happen there.
    Domain(Cow<'a, str>, u16),
}

impl fmt::Display for TargetAddr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetAddr::Ip(addr) => write!(f, "{}", addr),
            TargetAddr::Domain(domain, port) => write!(f, "{}:{}", domain, port),
        }
    }
}

impl TargetAddr<'_> {
    /// Creates owned `TargetAddr` by cloning. It is usually used to eliminate
    /// the lifetime bound.
    pub fn to_owned(&self) -> TargetAddr<'static> {
        match self {
            TargetAddr::Ip(addr) => TargetAddr::Ip(*addr),
            TargetAddr::Domain(domain, port) => {
                TargetAddr::Domain(String::from(domain.clone()).into(), *port)
            }
        }
    }
}

/// A trait for objects that can be converted to `TargetAddr`.
pub trait IntoTargetAddr<'a> {
    /// Converts the value of self to a `TargetAddr`.
    fn into_target_addr(self) -> Result<TargetAddr<'a>>;
}

macro_rules! trivial_impl_into_target_addr {
    ($t: ty) => {
        impl<'a> IntoTargetAddr<'a> for $t {
            fn into_target_addr(self) -> Result<TargetAddr<'a>> {
                Ok(TargetAddr::Ip(SocketAddr::from(self)))
            }
        }
    };
}

trivial_impl_into_target_addr!(SocketAddr);
trivial_impl_into_target_addr!((IpAddr, u16));
trivial_impl_into_target_addr!((Ipv4Addr, u16));
trivial_impl_into_target_addr!((Ipv6Addr, u16));
trivial_impl_into_target_addr!(SocketAddrV4);
trivial_impl_into_target_addr!(SocketAddrV6);

impl<'a> IntoTargetAddr<'a> for TargetAddr<'a> {
    fn into_target_addr(self) -> Result<TargetAddr<'a>> {
        Ok(self)
    }
}

impl<'a> IntoTargetAddr<'a> for (&'a str, u16) {
    fn into_target_addr(self) -> Result<TargetAddr<'a>> {
        // Try IP address first
        if let Ok(addr) = self.0.parse::<IpAddr>() {
            return (addr, self.1).into_target_addr();
        }

        // Treat as domain name
        if self.0.len() > 255 {
            return Err(Error::InvalidTargetAddress("overlong domain"));
        }

        Ok(TargetAddr::Domain(self.0.into(), self.1))
    }
}

impl<'a, T> IntoTargetAddr<'a> for &'a T
where
    T: IntoTargetAddr<'a> + Copy,
{
    fn into_target_addr(self) -> Result<TargetAddr<'a>> {
        (*self).into_target_addr()
    }
}

/// Authentication methods
#[derive(Debug)]
pub(crate) enum Authentication<'a> {
    Password {
        username: &'a str,
        password: &'a str,
    },
    None,
}

impl Authentication<'_> {
    fn id(&self) -> u8 {
        match self {
            Authentication::Password { .. } => 0x02,
            Authentication::None => 0x00,
        }
    }
}
