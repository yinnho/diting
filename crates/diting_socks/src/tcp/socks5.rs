//! SOCKS5 client — the two faces this repo uses: plain `connect` (resolve
//! the proxy ourselves, dial, handshake) and `*_with_socket` (handshake over
//! a caller-supplied socket, diting_wire's connector).
//!
//! Vendored from tokio-socks 0.5.3; the license lives at the crate root.

use std::{
    borrow::Cow,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    net::ToSocketAddrs as _,
};

use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
};

use crate::{Authentication, Error, IntoTargetAddr, Result, TargetAddr};

#[repr(u8)]
#[derive(Clone, Copy)]
enum Command {
    Connect = 0x01,
}

/// A SOCKS5 client.
#[derive(Debug)]
pub struct Socks5Stream<S> {
    socket: S,
    target: TargetAddr<'static>,
}

impl Socks5Stream<TcpStream> {
    /// Connects to a target server through a SOCKS5 proxy given the proxy's
    /// `host:port`. The proxy is resolved with std `to_socket_addrs` (the
    /// upstream stream machinery did the same on first poll) and addrs are
    /// tried in order until one dials.
    pub async fn connect<'t>(proxy: &str, target: impl IntoTargetAddr<'t>) -> Result<Self> {
        let addrs = proxy.to_socket_addrs()?;
        let mut last = None;
        for addr in addrs {
            match TcpStream::connect(addr).await {
                Ok(tcp) => {
                    return SocksConnector::new(
                        Authentication::None,
                        Command::Connect,
                        target.into_target_addr()?,
                    )
                    .execute_with_socket(tcp)
                    .await;
                },
                Err(e) => last = Some(e),
            }
        }
        Err(match last {
            Some(e) => e.into(),
            None => Error::ProxyServerUnreachable,
        })
    }
}

impl<S> Socks5Stream<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    /// Connects to a target server through a SOCKS5 proxy given a socket to it.
    pub async fn connect_with_socket<'t, T>(socket: S, target: T) -> Result<Socks5Stream<S>>
    where
        T: IntoTargetAddr<'t>,
    {
        Self::execute_command_with_socket(socket, target, Authentication::None).await
    }

    /// Connects to a target server through a SOCKS5 proxy using given username,
    /// password and a socket to the proxy.
    pub async fn connect_with_password_and_socket<'a, 't, T>(
        socket: S,
        target: T,
        username: &'a str,
        password: &'a str,
    ) -> Result<Socks5Stream<S>>
    where
        T: IntoTargetAddr<'t>,
    {
        Self::execute_command_with_socket(
            socket,
            target,
            Authentication::Password { username, password },
        )
        .await
    }

    async fn execute_command_with_socket<'a, 't, T>(
        socket: S,
        target: T,
        auth: Authentication<'a>,
    ) -> Result<Socks5Stream<S>>
    where
        T: IntoTargetAddr<'t>,
    {
        Self::validate_auth(&auth)?;

        SocksConnector::new(auth, Command::Connect, target.into_target_addr()?)
            .execute_with_socket(socket)
            .await
    }

    fn validate_auth(auth: &Authentication<'_>) -> Result<()> {
        match auth {
            Authentication::Password { username, password } => {
                let username_len = username.len();
                if !(1..=255).contains(&username_len) {
                    Err(Error::InvalidAuthValues("username length should between 1 to 255"))?
                }
                let password_len = password.len();
                if !(1..=255).contains(&password_len) {
                    Err(Error::InvalidAuthValues("password length should between 1 to 255"))?
                }
            },
            Authentication::None => {},
        }
        Ok(())
    }

    /// Consumes the `Socks5Stream`, returning the inner socket.
    pub fn into_inner(self) -> S {
        self.socket
    }

    /// Returns the target address that the proxy server connects to.
    pub fn target_addr(&self) -> TargetAddr<'_> {
        match &self.target {
            TargetAddr::Ip(addr) => TargetAddr::Ip(*addr),
            TargetAddr::Domain(domain, port) => {
                TargetAddr::Domain(Cow::Owned(domain.clone().into_owned()), *port)
            },
        }
    }
}

/// Runs the SOCKS5 method-negotiation + CONNECT exchange over an
/// already-connected socket.
struct SocksConnector<'a, 't> {
    auth: Authentication<'a>,
    command: Command,
    target: TargetAddr<'t>,
    buf: [u8; 513],
    ptr: usize,
    len: usize,
}

impl<'a, 't> SocksConnector<'a, 't> {
    fn new(auth: Authentication<'a>, command: Command, target: TargetAddr<'t>) -> Self {
        SocksConnector {
            auth,
            command,
            target,
            buf: [0; 513],
            ptr: 0,
            len: 0,
        }
    }

    pub async fn execute_with_socket<S>(mut self, mut socket: S) -> Result<Socks5Stream<S>>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        self.authenticate(&mut socket).await?;

        // Send request address that should be proxied
        self.prepare_send_request();
        socket.write_all(&self.buf[self.ptr..self.len]).await?;

        let target = self.receive_reply(&mut socket).await?;

        Ok(Socks5Stream { socket, target })
    }

    fn prepare_send_method_selection(&mut self) {
        self.ptr = 0;
        self.buf[0] = 0x05;
        match self.auth {
            Authentication::None => {
                self.buf[1..3].copy_from_slice(&[1, 0x00]);
                self.len = 3;
            },
            Authentication::Password { .. } => {
                self.buf[1..4].copy_from_slice(&[2, 0x00, 0x02]);
                self.len = 4;
            },
        }
    }

    fn prepare_recv_method_selection(&mut self) {
        self.ptr = 0;
        self.len = 2;
    }

    fn prepare_send_password_auth(&mut self) {
        if let Authentication::Password { username, password } = self.auth {
            self.ptr = 0;
            self.buf[0] = 0x01;
            let username_bytes = username.as_bytes();
            let username_len = username_bytes.len();
            self.buf[1] = username_len as u8;
            self.buf[2..(2 + username_len)].copy_from_slice(username_bytes);
            let password_bytes = password.as_bytes();
            let password_len = password_bytes.len();
            self.len = 3 + username_len + password_len;
            self.buf[2 + username_len] = password_len as u8;
            self.buf[(3 + username_len)..self.len].copy_from_slice(password_bytes);
        } else {
            unreachable!("password auth prepared without password credentials")
        }
    }

    fn prepare_recv_password_auth(&mut self) {
        self.ptr = 0;
        self.len = 2;
    }

    fn prepare_send_request(&mut self) {
        self.ptr = 0;
        self.buf[..3].copy_from_slice(&[0x05, self.command as u8, 0x00]);
        match &self.target {
            TargetAddr::Ip(SocketAddr::V4(addr)) => {
                self.buf[3] = 0x01;
                self.buf[4..8].copy_from_slice(&addr.ip().octets());
                self.buf[8..10].copy_from_slice(&addr.port().to_be_bytes());
                self.len = 10;
            },
            TargetAddr::Ip(SocketAddr::V6(addr)) => {
                self.buf[3] = 0x04;
                self.buf[4..20].copy_from_slice(&addr.ip().octets());
                self.buf[20..22].copy_from_slice(&addr.port().to_be_bytes());
                self.len = 22;
            },
            TargetAddr::Domain(domain, port) => {
                self.buf[3] = 0x03;
                let domain = domain.as_bytes();
                let len = domain.len();
                self.buf[4] = len as u8;
                self.buf[5..5 + len].copy_from_slice(domain);
                self.buf[(5 + len)..(7 + len)].copy_from_slice(&port.to_be_bytes());
                self.len = 7 + len;
            },
        }
    }

    fn prepare_recv_reply(&mut self) {
        self.ptr = 0;
        self.len = 4;
    }

    async fn password_authentication_protocol<S>(
        &mut self,
        tcp: &mut S,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        if let Authentication::None = self.auth {
            return Err(Error::AuthorizationRequired);
        }

        self.prepare_send_password_auth();
        tcp.write_all(&self.buf[self.ptr..self.len]).await?;

        self.prepare_recv_password_auth();
        tcp.read_exact(&mut self.buf[self.ptr..self.len]).await?;

        if self.buf[0] != 0x01 {
            return Err(Error::InvalidResponseVersion);
        }
        if self.buf[1] != 0x00 {
            return Err(Error::PasswordAuthFailure(self.buf[1]));
        }

        Ok(())
    }

    async fn authenticate<S>(&mut self, tcp: &mut S) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // Write request to connect/authenticate
        self.prepare_send_method_selection();
        tcp.write_all(&self.buf[self.ptr..self.len]).await?;

        // Receive authentication method
        self.prepare_recv_method_selection();
        tcp.read_exact(&mut self.buf[self.ptr..self.len]).await?;
        if self.buf[0] != 0x05 {
            return Err(Error::InvalidResponseVersion);
        }
        match self.buf[1] {
            0x00 => {
                // No auth
            },
            0x02 => {
                self.password_authentication_protocol(tcp).await?;
            },
            0xff => {
                return Err(Error::NoAcceptableAuthMethods);
            },
            m if m != self.auth.id() => return Err(Error::UnknownAuthMethod),
            _ => unreachable!("unhandled SOCKS5 auth method reply"),
        }

        Ok(())
    }

    async fn receive_reply<S>(&mut self, tcp: &mut S) -> Result<TargetAddr<'static>>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        self.prepare_recv_reply();
        self.ptr += tcp.read_exact(&mut self.buf[self.ptr..self.len]).await?;
        if self.buf[0] != 0x05 {
            return Err(Error::InvalidResponseVersion);
        }
        if self.buf[2] != 0x00 {
            return Err(Error::InvalidReservedByte);
        }

        match self.buf[1] {
            0x00 => {}, // succeeded
            0x01 => Err(Error::GeneralSocksServerFailure)?,
            0x02 => Err(Error::ConnectionNotAllowedByRuleset)?,
            0x03 => Err(Error::NetworkUnreachable)?,
            0x04 => Err(Error::HostUnreachable)?,
            0x05 => Err(Error::ConnectionRefused)?,
            0x06 => Err(Error::TtlExpired)?,
            0x07 => Err(Error::CommandNotSupported)?,
            0x08 => Err(Error::AddressTypeNotSupported)?,
            _ => Err(Error::UnknownAuthMethod)?,
        }

        match self.buf[3] {
            // IPv4
            0x01 => {
                self.len = 10;
            },
            // IPv6
            0x04 => {
                self.len = 22;
            },
            // Domain
            0x03 => {
                self.len = 5;
                self.ptr += tcp.read_exact(&mut self.buf[self.ptr..self.len]).await?;
                self.len += self.buf[4] as usize + 2;
            },
            _ => Err(Error::UnknownAddressType)?,
        }

        self.ptr += tcp.read_exact(&mut self.buf[self.ptr..self.len]).await?;
        let target: TargetAddr<'static> = match self.buf[3] {
            // IPv4
            0x01 => {
                let mut ip = [0; 4];
                ip[..].copy_from_slice(&self.buf[4..8]);
                let ip = Ipv4Addr::from(ip);
                let port = u16::from_be_bytes([self.buf[8], self.buf[9]]);
                TargetAddr::Ip(SocketAddr::new(ip.into(), port))
            },
            // IPv6
            0x04 => {
                let mut ip = [0; 16];
                ip[..].copy_from_slice(&self.buf[4..20]);
                let ip = Ipv6Addr::from(ip);
                let port = u16::from_be_bytes([self.buf[20], self.buf[21]]);
                TargetAddr::Ip(SocketAddr::new(ip.into(), port))
            },
            // Domain
            0x03 => {
                let domain_bytes = self.buf[5..(self.len - 2)].to_vec();
                let domain = String::from_utf8(domain_bytes)
                    .map_err(|_| Error::InvalidTargetAddress("not a valid UTF-8 string"))?;
                let port = u16::from_be_bytes([self.buf[self.len - 2], self.buf[self.len - 1]]);
                TargetAddr::Domain(domain.into(), port)
            },
            _ => unreachable!("address type checked above"),
        };

        Ok(target)
    }
}
