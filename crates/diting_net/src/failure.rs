//! Why a request got no response, as a value. The engine used to record
//! only free text for this, so a face that had to tell a refused
//! connection from a refused CORS check read the text; the kind is known
//! where the failure happens, and travels from there.

use std::error::Error;

/// How a request failed. What a face calls it is the face's own business;
/// this is what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetFailure {
    /// Nothing was listening: the connection was refused.
    ConnectionRefused,
    /// The host name did not resolve.
    NameNotResolved,
    /// The request ran out of time.
    TimedOut,
    /// The peer reset an established connection.
    ConnectionReset,
    /// `Network.setBlockedURLs` held it back before it left.
    BlockedByInspector,
    /// The response named no `Access-Control-Allow-Origin` at all.
    CorsMissingAllowOrigin,
    /// The response named another origin than the page's.
    CorsAllowOriginMismatch(String),
    /// The port is one a browser refuses to speak HTTP to, so no socket
    /// was opened.
    UnsafePort,
    /// Any other failure: the free-text reason is all there is.
    Other,
}

impl NetFailure {
    /// The refusal of a CORS check for want of an acceptable
    /// `Access-Control-Allow-Origin`: `allowed` is what the response sent,
    /// empty when it sent none.
    pub fn cors_allow_origin(allowed: &str) -> Self {
        match allowed {
            "" => NetFailure::CorsMissingAllowOrigin,
            other => NetFailure::CorsAllowOriginMismatch(other.to_string()),
        }
    }
}

/// What the transport's error says of itself, read off its typed source
/// chain (`is_dns`, `is_timeout`, the `io::Error` kind underneath) — not
/// off its message.
pub fn classify_transport(error: &diting_wire::Error) -> NetFailure {
    if error.is_dns() {
        NetFailure::NameNotResolved
    } else if error.is_timeout() {
        NetFailure::TimedOut
    } else if io_kind(error) == Some(std::io::ErrorKind::ConnectionRefused) {
        NetFailure::ConnectionRefused
    } else if error.is_connection_reset() {
        NetFailure::ConnectionReset
    } else {
        NetFailure::Other
    }
}

/// The kind of the innermost `io::Error` in `error`'s source chain.
fn io_kind(error: &(dyn Error + 'static)) -> Option<std::io::ErrorKind> {
    let mut found = None;
    let mut source = error.source();
    while let Some(err) = source {
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            found = Some(io.kind());
        }
        source = err.source();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn failure_of(url: &str) -> NetFailure {
        let error = diting_wire::Client::new()
            .get(url)
            .send()
            .await
            .expect_err("nothing answers there");
        classify_transport(&error)
    }

    #[tokio::test]
    async fn a_closed_port_is_a_refused_connection() {
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        assert_eq!(
            failure_of(&format!("http://127.0.0.1:{port}/")).await,
            NetFailure::ConnectionRefused
        );
    }

    #[tokio::test]
    async fn a_name_that_cannot_resolve_is_a_lookup_failure() {
        // `.invalid` never resolves (RFC 2606).
        assert_eq!(
            failure_of("http://nonexistent-host.invalid/").await,
            NetFailure::NameNotResolved
        );
    }
}
