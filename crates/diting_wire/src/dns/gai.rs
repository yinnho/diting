if_tokio_rt! {
    mod tokio;
}

if_compio_rt!(
    mod compio;
);

if_all_rt!(
    mod tokio;
);

use std::net::SocketAddr;

use crate::dns::SocketAddrs;

/// A resolver using blocking `getaddrinfo` calls in a threadpool.
#[derive(Clone, Default)]
pub struct GaiResolver {
    _priv: (),
}

/// An iterator of IP addresses returned from `getaddrinfo`.
pub struct GaiAddrs {
    inner: SocketAddrs,
}

impl Iterator for GaiAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
