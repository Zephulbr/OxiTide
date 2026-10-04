#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use oxitide_audio::queue::{QueueItem, RepeatMode};
use serde::{Deserialize, Serialize};

pub const SESSION_VERSION: u32 = 1;

/// One queue item as persisted. `explicit`/`ai` are reserved for the Track 10.5
/// content filters; they are `None` until that plan populates them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionItem {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u32,
    pub cover: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<bool>,
}

impl From<&QueueItem> for SessionItem {
    fn from(item: &QueueItem) -> Self {
        Self {
            track_id: item.track_id,
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: item.album.clone(),
            duration_secs: item.duration_secs,
            cover: item.cover.clone(),
            explicit: None,
            ai: None,
        }
    }
}

impl From<SessionItem> for QueueItem {
    fn from(item: SessionItem) -> Self {
        Self {
            track_id: item.track_id,
            title: item.title,
            artist: item.artist,
            album: item.album,
            duration_secs: item.duration_secs,
            cover: item.cover,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub version: u32,
    pub queue: Vec<SessionItem>,
    pub current_index: Option<usize>,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub play_source: Option<String>,
    pub saved_at: u64,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            version: SESSION_VERSION,
            queue: Vec::new(),
            current_index: None,
            position_ms: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
            play_source: None,
            saved_at: 0,
        }
    }
}

/// `%APPDATA%\OxiTide\session.json`, beside `config.toml`.
pub fn session_path() -> Option<PathBuf> {
    crate::config::config_path().map(|path| path.with_file_name("session.json"))
}

/// Read `path`; a missing file yields `None`, and a malformed or
/// future-version file is renamed `.bak` (so it cannot brick startup) and
/// yields `None`.
pub fn load(path: &Path) -> Option<Session> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Session>(&text) {
        Ok(session) if session.version == SESSION_VERSION => Some(session),
        Ok(session) => {
            tracing::warn!("unsupported session version {}; backing up", session.version);
            backup(path);
            None
        }
        Err(e) => {
            tracing::warn!(
                "malformed session at {} ({:?} at line {}, column {})",
                path.display(),
                e.classify(),
                e.line(),
                e.column()
            );
            backup(path);
            None
        }
    }
}

/// Atomic write (temp file + rename), the `config.rs` pattern.
pub fn save(path: &Path, session: &Session) -> Result<(), String> {
    let text = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove the session file; a missing file is not an error.
pub fn clear(path: &Path) {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("failed to clear session: {e}");
        }
    }
}

fn backup(path: &Path) {
    let backup = path.with_extension("json.bak");
    let _ = fs::rename(path, &backup);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-session-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn item(id: u64) -> SessionItem {
        SessionItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 3,
            cover: None,
            explicit: None,
            ai: None,
        }
    }

    fn session() -> Session {
        Session {
            version: SESSION_VERSION,
            queue: vec![item(1), item(2), item(3)],
            current_index: Some(1),
            position_ms: 4200,
            shuffle: true,
            repeat: RepeatMode::All,
            play_source: Some("pl-1".into()),
            saved_at: 1_700_000_000,
        }
    }

    #[test]
    fn round_trip_preserves_everything_including_duplicates() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let mut original = session();
        original.queue = vec![item(1), item(2), item(1), item(3)];
        save(&path, &original).unwrap();
        assert_eq!(load(&path), Some(original));
        assert!(!dir.join("session.json.tmp").exists(), "temp file left behind");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_file_is_none() {
        let dir = temp_dir();
        assert_eq!(load(&dir.join("session.json")), None);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: a corrupt file must never reach startup.
    #[test]
    fn malformed_session_is_backed_up_and_ignored() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        fs::write(&path, "not json at all").unwrap();
        assert_eq!(load(&path), None);
        assert!(!path.exists());
        assert!(dir.join("session.json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: files written by a future OxiTide must not be trusted.
    #[test]
    fn future_version_is_backed_up_and_ignored() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        fs::write(&path, r#"{"version":99,"queue":[],"current_index":null,"position_ms":0,"shuffle":false,"repeat":"off","play_source":null,"saved_at":0}"#).unwrap();
        assert_eq!(load(&path), None);
        assert!(dir.join("session.json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_removes_the_file_and_tolerates_absence() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        save(&path, &session()).unwrap();
        clear(&path);
        assert!(!path.exists());
        clear(&path); // no panic on a missing file
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unwritable_path_returns_an_error_not_a_panic() {
        let dir = temp_dir();
        let blocker = dir.join("blocked");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("session.json");
        assert!(save(&path, &session()).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
