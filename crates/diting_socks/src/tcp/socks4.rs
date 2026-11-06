//! SOCKS4 client — pruned to the `connect_with_socket` face diting_wire's
//! connector uses (it dials the proxy itself).
//!
//! Vendored from tokio-socks 0.5.3; the license lives at the crate root.

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::{Error, IntoTargetAddr, Result, TargetAddr};

#[repr(u8)]
#[derive(Clone, Copy)]
enum CommandV4 {
    Connect = 0x01,
}

/// A SOCKS4 client over a caller-supplied socket.
#[derive(Debug)]
pub struct Socks4Stream<S> {
    socket: S,
}

impl<S> Socks4Stream<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    /// Connects to a target server through a SOCKS4 proxy given a socket to it.
    pub async fn connect_with_socket<'t, T>(socket: S, target: T) -> Result<Socks4Stream<S>>
    where
        T: IntoTargetAddr<'t>,
    {
        let sock =
            Socks4Connector::new(CommandV4::Connect, target.into_target_addr()?)
                .execute_with_socket(socket)
                .await?;

        Ok(sock)
    }

    /// Consumes the `Socks4Stream`, returning the inner socket.
    pub fn into_inner(self) -> S {
        self.socket
    }
}

/// Runs the SOCKS4 request/reply exchange over an already-connected socket.
struct Socks4Connector<'t> {
    command: CommandV4,
    target: TargetAddr<'t>,
    buf: [u8; 513],
    ptr: usize,
    len: usize,
}

impl<'t> Socks4Connector<'t> {
    fn new(command: CommandV4, target: TargetAddr<'t>) -> Self {
        Socks4Connector {
            command,
            target,
            buf: [0; 513],
            ptr: 0,
            len: 0,
        }
    }

    pub async fn execute_with_socket<S>(mut self, mut socket: S) -> Result<Socks4Stream<S>>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // Send request address that should be proxied
        self.prepare_send_request()?;
        socket.write_all(&self.buf[self.ptr..self.len]).await?;

        self.receive_reply(&mut socket).await?;

        Ok(Socks4Stream { socket })
    }

    fn prepare_send_request(&mut self) -> Result<()> {
        self.ptr = 0;
        self.buf[..2].copy_from_slice(&[0x04, self.command as u8]);
        match &self.target {
            TargetAddr::Ip(SocketAddr::V4(addr)) => {
                self.buf[2..4].copy_from_slice(&addr.port().to_be_bytes());
                self.buf[4..8].copy_from_slice(&addr.ip().octets());
                self.len = 8;
            },
            TargetAddr::Ip(SocketAddr::V6(_)) => {
                return Err(Error::AddressTypeNotSupported);
            },
            TargetAddr::Domain(domain, port) => {
                // SOCKS4a: bogus IP 0.0.0.1 marks the trailing bytes as a
                // domain for the proxy to resolve.
                self.buf[2..4].copy_from_slice(&port.to_be_bytes());
                self.buf[4..8].copy_from_slice(&[0, 0, 0, 1]);
                self.len = 8;
                let domain = domain.as_bytes();
                let domain_len = domain.len();
                self.buf[self.len..self.len + domain_len].copy_from_slice(domain);
                self.len += domain_len;
            },
        }
        self.buf[self.len] = 0; // null terminator
        self.len += 1;
        Ok(())
    }

    // Consumes and validates the 8-byte reply. The bound address it carries is
    // dead surface on the pruned face (diting_wire only wants the socket), so
    // the bytes are read and dropped.
    async fn receive_reply<S>(&mut self, mut tcp: S) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // https://www.openssh.com/txt/socks4.protocol
        // +----+----+----+----+----+----+----+----+
        // | VN | CD | DSTPORT |      DSTIP        |
        // +----+----+----+----+----+----+----+----+
        // # of bytes:	   1    1      2              4
        //
        // VN is the version of the reply code and should be 0. CD is the result
        // code with one of the following values:
        // 90: request granted
        // 91: request rejected or failed
        // 92: request rejected because SOCKS server cannot connect to
        // identd on the client
        // 93: request rejected because the client program and identd
        // report different user-ids

        self.ptr = 0;
        self.len = 8;
        self.ptr += tcp.read_exact(&mut self.buf[self.ptr..self.len]).await?;
        if self.buf[0] != 0 {
            return Err(Error::InvalidResponseVersion);
        }

        match self.buf[1] {
            0x5A => {},                                           // request granted
            0x5B => return Err(Error::GeneralSocksServerFailure), // connection rejected/failed
            0x5C => return Err(Error::IdentdAuthFailure),         // cannot connect to identd on the client
            0x5D => return Err(Error::InvalidUserIdAuthFailure),  // different user-ids
            _ => return Err(Error::UnknownError),
        }

        Ok(())
    }
}
