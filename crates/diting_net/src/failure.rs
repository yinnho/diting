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
    /// The peer took the request and closed without a word.
    EmptyResponse,
    /// The body ended short of the length the response declared.
    ContentLengthMismatch,
    /// The body ended inside a chunked encoding.
    IncompleteChunkedEncoding,
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
    } else if error.is_incomplete_body() {
        NetFailure::ContentLengthMismatch
    } else if error.is_incomplete_chunks() {
        NetFailure::IncompleteChunkedEncoding
    } else if error.is_incomplete_message() {
        NetFailure::EmptyResponse
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

    /// A server that does what its path says once it has the request:
    /// `/empty` closes, `/reset` resets, `/shortfin` and `/shortrst` send
    /// 3 of a declared 100 body bytes and then close or reset, and
    /// `/chunkcut` ends inside a chunked body.
    async fn misbehaving_server() -> u16 {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    let line = request.lines().next().unwrap_or("").to_string();
                    let reset = |sock: &tokio::net::TcpStream| {
                        // SO_LINGER 0 turns the close into a reset. The deprecation is
                        // about blocking the thread on drop; these sockets are test-local.
                        #[allow(deprecated)]
                        let _ = sock.set_linger(Some(std::time::Duration::from_secs(0)));
                    };
                    if line.contains("/reset") {
                        reset(&sock);
                    } else if line.contains("/shortfin") || line.contains("/shortrst") {
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\nabc")
                            .await;
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        if line.contains("/shortrst") {
                            reset(&sock);
                        }
                    } else if line.contains("/chunkcut") {
                        let _ = sock
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabc\r\n",
                            )
                            .await;
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    // `/empty` and anything else: drop the socket unanswered.
                });
            }
        });
        port
    }

    /// How a request to `path` failed, whether on the request or on its body.
    async fn how_it_failed(port: u16, path: &str) -> NetFailure {
        let sent = diting_wire::Client::new()
            .get(format!("http://127.0.0.1:{port}/{path}"))
            .send()
            .await;
        let error = match sent {
            Err(error) => error,
            Ok(response) => response.bytes().await.expect_err("the body ends short"),
        };
        classify_transport(&error)
    }

    #[tokio::test]
    async fn a_connection_that_closes_unanswered_is_an_empty_response() {
        let port = misbehaving_server().await;
        assert_eq!(
            how_it_failed(port, "empty").await,
            NetFailure::EmptyResponse
        );
    }

    #[tokio::test]
    async fn a_reset_connection_is_a_reset_before_or_during_the_body() {
        let port = misbehaving_server().await;
        assert_eq!(
            how_it_failed(port, "reset").await,
            NetFailure::ConnectionReset
        );
        assert_eq!(
            how_it_failed(port, "shortrst").await,
            NetFailure::ConnectionReset
        );
    }

    #[tokio::test]
    async fn a_body_that_ends_short_says_how_its_length_was_declared() {
        let port = misbehaving_server().await;
        assert_eq!(
            how_it_failed(port, "shortfin").await,
            NetFailure::ContentLengthMismatch
        );
        assert_eq!(
            how_it_failed(port, "chunkcut").await,
            NetFailure::IncompleteChunkedEncoding
        );
    }
}
