use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("tidal error: {0}")]
    Tidal(#[from] oxitide_tidal::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
    #[error("mpd parse error: {0}")]
    Mpd(String),
    #[error("fetch error: {0}")]
    Fetch(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("output error: {0}")]
    Output(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_error_has_stable_message() {
        assert_eq!(Error::Manifest("bad".into()).to_string(), "manifest error: bad");
    }

    #[test]
    fn tap_error_includes_message() {
        assert_eq!(Error::Decode("boom".into()).to_string(), "decode error: boom");
    }
}
