use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
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
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
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
        let e = Error::Api { status: 429, message: "slow down".into() };
        assert_eq!(e.to_string(), "api error 429: slow down");
    }
}
