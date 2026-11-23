use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Shown with reqwest's source chain: its own message for a transport
    /// failure is only "error sending request for url (…)", which hides
    /// whether the cause was a reset, DNS, TLS, or a timeout.
    #[error("http error: {}", display_chain(.0))]
    Http(#[source] reqwest::Error),
    /// The secure connection itself failed: a reply that was not TLS, or a
    /// certificate no trusted root vouches for. On a working network that
    /// is something in the path (antivirus HTTPS scanning, a proxy, a VPN, a
    /// captive portal); retrying at once does not help, so it is never
    /// retried per request. The UI matches the `tls error:` prefix
    /// (`ui/src/lib/errors.ts`). Holds reqwest's message with its causes.
    #[error("tls error: {0}")]
    Tls(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("rate limited")]
    RateLimited,
    #[error("api error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("task join error: {0}")]
    Task(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// A TLS failure gets its own variant; every other reqwest error stays `Http`.
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if is_tls(&e) {
            Error::Tls(display_chain(&e))
        } else {
            Error::Http(e)
        }
    }
}

impl Error {
    /// Whether retrying the same request later could succeed: the network
    /// failed before a response arrived, or TIDAL answered with a rate limit
    /// or a gateway/server hiccup. Auth, permission, not-found, and shape
    /// errors are permanent for that request.
    ///
    /// A TLS failure counts: it is a condition of the network, not of the
    /// request, so callers that wait for the network (offline start, cached
    /// playback, resume) treat it like an outage. Only the immediate
    /// per-request retry skips it ([`is_transport`]).
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Http(e) => is_transport(e),
            Error::Tls(_) => true,
            Error::RateLimited => true,
            Error::Api { status, .. } => matches!(status, 500 | 502 | 503 | 504),
            _ => false,
        }
    }
}

/// A reqwest failure with no HTTP status that an immediate retry may get
/// past: connect, send, body read, or timeout. A stale pooled keep-alive
/// connection lands here too. TLS failures are left out ([`is_tls`]).
pub(crate) fn is_transport(e: &reqwest::Error) -> bool {
    e.status().is_none()
        && (e.is_connect() || e.is_request() || e.is_body() || e.is_timeout())
        && !is_tls(e)
}

/// Whether `e`'s cause is a TLS failure (`rustls::Error`): the handshake got
/// something that is not TLS (`InvalidContentType`), a certificate was
/// refused (`UnknownIssuer`, …), or the peer sent a fatal alert.
///
/// rustls reaches reqwest wrapped in an `io::Error`, whose `source()` skips
/// the error it wraps, so each `io::Error` in the chain is opened with
/// `get_ref` as well.
pub(crate) fn is_tls(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut next = Some(e);
    while let Some(cause) = next {
        if cause.downcast_ref::<rustls::Error>().is_some() {
            return true;
        }
        if let Some(inner) = cause
            .downcast_ref::<std::io::Error>()
            .and_then(|io| io.get_ref())
        {
            if is_tls(inner) {
                return true;
            }
        }
        next = cause.source();
    }
    false
}

/// `e` followed by each distinct `source()` message, joined by ": ".
pub(crate) fn display_chain(e: &reqwest::Error) -> String {
    let mut out = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        let text = cause.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        source = cause.source();
    }
    out
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn rate_limited_has_stable_message() {
        assert_eq!(Error::RateLimited.to_string(), "rate limited");
    }

    #[test]
    fn api_error_includes_status_and_message() {
        let e = Error::Api {
            status: 429,
            message: "slow down".into(),
        };
        assert_eq!(e.to_string(), "api error 429: slow down");
    }

    #[test]
    fn rate_limits_and_gateway_errors_are_transient() {
        assert!(Error::RateLimited.is_transient());
        for status in [500, 502, 503, 504] {
            assert!(Error::Api {
                status,
                message: String::new()
            }
            .is_transient());
        }
    }

    #[test]
    fn auth_and_lookup_errors_are_not_transient() {
        assert!(!Error::Unauthorized.is_transient());
        assert!(!Error::Forbidden.is_transient());
        assert!(!Error::NotFound("x".into()).is_transient());
        assert!(!Error::Api {
            status: 400,
            message: String::new()
        }
        .is_transient());
    }

    #[tokio::test]
    async fn connect_failure_is_transient_and_shows_its_cause() {
        // Bind then drop a listener so the port is closed.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let e = reqwest::Client::new()
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .await
            .unwrap_err();
        let e = Error::from(e);
        assert!(matches!(e, Error::Http(_)), "{e:?}");
        assert!(e.is_transient());
        let text = e.to_string();
        // reqwest's own message ends at the URL; the cause must follow it.
        assert!(text.contains("/): "), "cause chain missing: {text}");
    }

    /// A local server that answers every connection with plain HTTP, as the
    /// #10 reporter's network did; returns its port and a connection count.
    pub(crate) fn plain_text_server() -> (u16, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let connections = std::sync::Arc::new(AtomicUsize::new(0));
        let counted = connections.clone();
        std::thread::spawn(move || {
            for mut socket in listener.incoming().flatten() {
                counted.fetch_add(1, Ordering::SeqCst);
                // Take the ClientHello first, as a middlebox does; closing
                // with it unread resets the connection (os error 10053)
                // before the client sees any reply.
                let mut hello = [0u8; 4096];
                let _ = socket.read(&mut hello);
                let _ =
                    socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 7\r\n\r\nblocked");
                let _ = socket.shutdown(std::net::Shutdown::Write);
                // Drain until the client hangs up, so nothing unread is left.
                while matches!(socket.read(&mut hello), Ok(n) if n > 0) {}
            }
        });
        (port, connections)
    }

    /// Issue #10: TLS met with a plain-text reply is rustls'
    /// `InvalidContentType`. It becomes `Tls`: transient for callers that
    /// wait for the network, but not a transport error to retry at once.
    #[tokio::test]
    async fn a_reply_that_is_not_tls_is_a_tls_error() {
        let (port, _) = plain_text_server();
        let e = reqwest::Client::new()
            .get(format!("https://127.0.0.1:{port}/"))
            .send()
            .await
            .unwrap_err();
        assert!(is_tls(&e), "not classified as TLS: {}", display_chain(&e));
        assert!(!is_transport(&e));
        let e = Error::from(e);
        assert!(matches!(e, Error::Tls(_)), "{e:?}");
        assert!(e.is_transient());
        let text = e.to_string();
        assert!(text.starts_with("tls error: "), "{text}");
        assert!(text.contains("InvalidContentType"), "{text}");
    }

    #[test]
    fn a_rustls_error_inside_an_io_error_is_found() {
        let rustls = rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer);
        let io = std::io::Error::new(std::io::ErrorKind::InvalidData, rustls);
        assert!(is_tls(&io));
        let plain = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        assert!(!is_tls(&plain));
    }
}
