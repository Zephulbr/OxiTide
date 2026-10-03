use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const KEYRING_SERVICE: &str = "oxitide";
pub const KEYRING_USER: &str = "tidal";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenSet")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("token_type", &self.token_type)
            .field("expires_at", &self.expires_at)
            .finish()
    }
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

/// Whether the OS credential store can persist secrets this session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyringStatus {
    pub persistent: bool,
    pub reason: Option<String>,
}

impl KeyringStatus {
    pub fn persistent() -> Self {
        Self {
            persistent: true,
            reason: None,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            persistent: false,
            reason: Some(reason.into()),
        }
    }
}

const PROBE_SERVICE: &str = "oxitide-probe";
const PROBE_USER: &str = "probe";
const PROBE_SENTINEL: &str = "oxitide-probe-sentinel";

/// The minimal entry operations the sentinel probe performs. `keyring::Entry`
/// implements it in production; tests inject fakes so no OS store is touched.
trait ProbeEntry {
    fn write(&self, secret: &str) -> std::result::Result<(), String>;
    fn read(&self) -> std::result::Result<String, String>;
    fn delete(&self) -> std::result::Result<(), String>;
}

impl ProbeEntry for keyring::Entry {
    fn write(&self, secret: &str) -> std::result::Result<(), String> {
        self.set_password(secret).map_err(|e| e.to_string())
    }

    fn read(&self) -> std::result::Result<String, String> {
        self.get_password().map_err(|e| e.to_string())
    }

    fn delete(&self) -> std::result::Result<(), String> {
        self.delete_credential().map_err(|e| e.to_string())
    }
}

/// Write, read back, and delete a sentinel through `entry`. Any error or a
/// mismatched read-back means the store cannot be trusted to persist secrets.
fn probe_entry(entry: &dyn ProbeEntry) -> std::result::Result<(), String> {
    entry
        .write(PROBE_SENTINEL)
        .map_err(|e| format!("keyring write failed: {e}"))?;
    let read = entry
        .read()
        .map_err(|e| format!("keyring read failed: {e}"));
    let cleanup = entry
        .delete()
        .map_err(|e| format!("keyring cleanup failed: {e}"));
    let value = read?;
    cleanup?;
    if value == PROBE_SENTINEL {
        Ok(())
    } else {
        Err("the credential store did not return the sentinel".to_string())
    }
}

/// Availability check of the platform credential store. Implementations must
/// not log or expose secret material; the sentinel is non-sensitive.
pub trait KeyringProbe {
    fn probe(&self) -> std::result::Result<(), String>;
}

/// Probes the configured platform store (Credential Manager on Windows,
/// Secret Service on Linux).
pub struct SystemKeyringProbe;

impl KeyringProbe for SystemKeyringProbe {
    fn probe(&self) -> std::result::Result<(), String> {
        let entry =
            keyring::Entry::new(PROBE_SERVICE, PROBE_USER).map_err(|e| e.to_string())?;
        probe_entry(&entry)
    }
}

/// Select the process-wide token store after probing the platform credential
/// store: `KeyringStore` when persistence is available, otherwise a
/// session-only `MemoryStore`.
pub fn select_token_store(probe: &dyn KeyringProbe) -> (Arc<dyn TokenStore>, KeyringStatus) {
    match probe.probe() {
        Ok(()) => (Arc::new(KeyringStore), KeyringStatus::persistent()),
        Err(reason) => (
            Arc::new(MemoryStore::default()),
            KeyringStatus::unavailable(reason),
        ),
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

    #[test]
    fn debug_redacts_token_material() {
        let t = TokenSet {
            access_token: "super-secret-access".into(),
            refresh_token: Some("super-secret-refresh".into()),
            token_type: "Bearer".into(),
            expires_at: Some(1),
        };
        let s = format!("{t:?}");
        assert!(!s.contains("super-secret-access"));
        assert!(!s.contains("super-secret-refresh"));
        assert!(s.contains("<redacted>"));
    }

    #[test]
    fn sentinel_probe_rejects_a_store_that_loses_the_secret() {
        struct Forgetful;
        impl ProbeEntry for Forgetful {
            fn write(&self, _secret: &str) -> std::result::Result<(), String> {
                Ok(())
            }
            fn read(&self) -> std::result::Result<String, String> {
                Ok(String::new())
            }
            fn delete(&self) -> std::result::Result<(), String> {
                Ok(())
            }
        }
        assert!(probe_entry(&Forgetful).is_err());
    }

    #[test]
    fn sentinel_probe_writes_reads_then_deletes() {
        #[derive(Default)]
        struct Recording(std::sync::Mutex<Vec<&'static str>>);
        impl ProbeEntry for Recording {
            fn write(&self, _secret: &str) -> std::result::Result<(), String> {
                self.0.lock().unwrap().push("write");
                Ok(())
            }
            fn read(&self) -> std::result::Result<String, String> {
                self.0.lock().unwrap().push("read");
                Ok(PROBE_SENTINEL.to_string())
            }
            fn delete(&self) -> std::result::Result<(), String> {
                self.0.lock().unwrap().push("delete");
                Ok(())
            }
        }
        let recording = Recording::default();
        assert!(probe_entry(&recording).is_ok());
        assert_eq!(*recording.0.lock().unwrap(), ["write", "read", "delete"]);
    }

    struct FakeProbe(std::result::Result<(), String>);

    impl KeyringProbe for FakeProbe {
        fn probe(&self) -> std::result::Result<(), String> {
            self.0.clone()
        }
    }

    #[test]
    fn select_token_store_prefers_the_keyring_when_the_probe_succeeds() {
        let (_, status) = select_token_store(&FakeProbe(Ok(())));
        assert!(status.persistent);
        assert_eq!(status.reason, None);
    }

    #[test]
    fn select_token_store_falls_back_to_memory_when_the_probe_fails() {
        let (store, status) = select_token_store(&FakeProbe(Err("no secret service".into())));
        assert!(!status.persistent);
        assert_eq!(status.reason.as_deref(), Some("no secret service"));
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), sample());
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn fallback_store_is_session_only() {
        let (store, _) = select_token_store(&FakeProbe(Err("x".into())));
        store.save(&sample()).unwrap();
        let (fresh, _) = select_token_store(&FakeProbe(Err("x".into())));
        assert!(fresh.load().unwrap().is_none());
    }

    #[test]
    fn keyring_status_serializes_for_ipc() {
        let value = serde_json::to_value(KeyringStatus::unavailable("no bus")).unwrap();
        assert_eq!(value["persistent"], serde_json::json!(false));
        assert_eq!(value["reason"], serde_json::json!("no bus"));
        let ok = serde_json::to_value(KeyringStatus::persistent()).unwrap();
        assert_eq!(ok["persistent"], serde_json::json!(true));
        assert_eq!(ok["reason"], serde_json::Value::Null);
    }
}
