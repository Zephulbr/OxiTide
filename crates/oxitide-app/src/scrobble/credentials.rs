use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const KEYRING_SERVICE: &str = "oxitide";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredCredential {
    pub username: String,
    pub secret: String,
}

impl std::fmt::Debug for StoredCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredCredential")
            .field("username", &self.username)
            .field("secret", &"<redacted>")
            .finish()
    }
}

pub trait CredentialStore: Send + Sync {
    fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String>;
    fn load(&self, service: &str) -> Result<Option<StoredCredential>, String>;
    fn clear(&self, service: &str) -> Result<(), String>;
}

pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    fn entry(service: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, service)
            .map_err(|e| format!("keyring unavailable: {e}"))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String> {
        let json = serde_json::to_string(credential).map_err(|e| e.to_string())?;
        Self::entry(service)?
            .set_password(&json)
            .map_err(|e| format!("keyring write failed: {e}"))
    }

    fn load(&self, service: &str) -> Result<Option<StoredCredential>, String> {
        match Self::entry(service)?.get_password() {
            Ok(json) => serde_json::from_str(&json).map(Some).map_err(|e| e.to_string()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keyring read failed: {e}")),
        }
    }

    fn clear(&self, service: &str) -> Result<(), String> {
        match Self::entry(service)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keyring delete failed: {e}")),
        }
    }
}

#[derive(Default)]
pub struct MemoryCredentialStore {
    inner: Mutex<HashMap<String, StoredCredential>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn save(&self, service: &str, credential: &StoredCredential) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap()
            .insert(service.to_string(), credential.clone());
        Ok(())
    }

    fn load(&self, service: &str) -> Result<Option<StoredCredential>, String> {
        Ok(self.inner.lock().unwrap().get(service).cloned())
    }

    fn clear(&self, service: &str) -> Result<(), String> {
        self.inner.lock().unwrap().remove(service);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryCredentialStore::default();
        assert!(store.load("lastfm").unwrap().is_none());
        let cred = StoredCredential { username: "v".into(), secret: "s".into() };
        store.save("lastfm", &cred).unwrap();
        assert_eq!(store.load("lastfm").unwrap(), Some(cred));
        store.clear("lastfm").unwrap();
        assert!(store.load("lastfm").unwrap().is_none());
    }

    #[test]
    fn debug_redacts_the_secret() {
        let cred = StoredCredential { username: "v".into(), secret: "super-secret".into() };
        let text = format!("{cred:?}");
        assert!(!text.contains("super-secret"), "{text}");
        assert!(text.contains("<redacted>"));
        assert!(text.contains("v"));
    }
}
