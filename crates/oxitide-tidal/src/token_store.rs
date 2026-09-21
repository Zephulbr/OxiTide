use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const KEYRING_SERVICE: &str = "oxitide";
pub const KEYRING_USER: &str = "tidal";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

pub trait TokenStore: Send + Sync {
    fn save(&self, tokens: &TokenSet) -> Result<()>;
    fn load(&self) -> Result<Option<TokenSet>>;
    fn clear(&self) -> Result<()>;
}

pub struct KeyringStore;

impl KeyringStore {
    fn entry() -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| Error::Auth(format!("keyring unavailable: {e}")))
    }
}

impl TokenStore for KeyringStore {
    fn save(&self, tokens: &TokenSet) -> Result<()> {
        let json = serde_json::to_string(tokens)?;
        Self::entry()?
            .set_password(&json)
            .map_err(|e| Error::Auth(format!("keyring write failed: {e}")))
    }

    fn load(&self) -> Result<Option<TokenSet>> {
        match Self::entry()?.get_password() {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Auth(format!("keyring read failed: {e}"))),
        }
    }

    fn clear(&self) -> Result<()> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Auth(format!("keyring delete failed: {e}"))),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    inner: std::sync::Mutex<Option<TokenSet>>,
}

impl TokenStore for MemoryStore {
    fn save(&self, tokens: &TokenSet) -> Result<()> {
        *self.inner.lock().unwrap() = Some(tokens.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<TokenSet>> {
        Ok(self.inner.lock().unwrap().clone())
    }
    fn clear(&self) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TokenSet {
        TokenSet {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            token_type: "Bearer".into(),
            expires_at: Some(1_790_000_000),
        }
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::default();
        assert!(store.load().unwrap().is_none());
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), sample());
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn token_set_defaults_token_type_to_bearer() {
        let t: TokenSet = serde_json::from_str(r#"{"access_token":"x"}"#).unwrap();
        assert_eq!(t.token_type, "Bearer");
        assert!(t.refresh_token.is_none());
    }
}
