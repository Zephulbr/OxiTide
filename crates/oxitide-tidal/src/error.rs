use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Shown with reqwest's source chain: its own message for a transport
    /// failure is only "error sending request for url (…)", which hides
    /// whether the cause was a reset, DNS, TLS, or a timeout.
    #[error("http error: {}", display_chain(.0))]
    Http(#[from] reqwest::Error),
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

impl Error {
    /// Whether retrying the same request later could succeed: the network
    /// failed before a response arrived, or TIDAL answered with a rate limit
    /// or a gateway/server hiccup. Auth, permission, not-found, and shape
    /// errors are permanent for that request.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Http(e) => is_transport(e),
            Error::RateLimited => true,
            Error::Api { status, .. } => matches!(status, 500 | 502 | 503 | 504),
            _ => false,
        }
    }
}

/// A reqwest failure with no HTTP status: connect, send, body read, or
/// timeout. A stale pooled keep-alive connection lands here too.
pub(crate) fn is_transport(e: &reqwest::Error) -> bool {
    e.status().is_none() && (e.is_connect() || e.is_request() || e.is_body() || e.is_timeout())
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
mod tests {
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
        let e = Error::Http(e);
        assert!(e.is_transient());
        let text = e.to_string();
        // reqwest's own message ends at the URL; the cause must follow it.
        assert!(text.contains("/): "), "cause chain missing: {text}");
    }
}
