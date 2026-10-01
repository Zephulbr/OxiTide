//! Windows custom-scheme ownership hygiene (spec §5.2). OxiTide registers
//! `tidal://` at startup so TIDAL's login redirect reaches it; because the
//! official TIDAL desktop app may own the same scheme, we snapshot the existing
//! HKCU registration before registering and restore it on exit.
//!
//! The guard is deliberately conservative: it never deletes a key it could not
//! first back up, and after a crash (the key already points at OxiTide) it
//! reuses the prior run's backup rather than discarding the official handler.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Mutex;

#[cfg(windows)]
const KEY: &str = r"HKCU\Software\Classes\tidal";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOwner {
    /// No `tidal` handler under HKCU.
    Absent,
    /// The handler points at OxiTide itself.
    SelfOwned,
    /// The handler belongs to another app (the official TIDAL client).
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemeAction {
    /// Re-import a saved official registration on exit.
    Import(PathBuf),
    /// Delete OxiTide's registration (nothing existed before).
    Delete,
    /// Leave the key untouched (cannot safely back up or recover it).
    Leave,
}

/// Classify a `reg query` output for the scheme's command key.
pub fn owner_from_query(query_output: &str, exe_fragment: &str) -> KeyOwner {
    if query_output.trim().is_empty() {
        KeyOwner::Absent
    } else if query_output
        .to_ascii_lowercase()
        .contains(&exe_fragment.to_ascii_lowercase())
    {
        KeyOwner::SelfOwned
    } else {
        KeyOwner::Other
    }
}

/// What to restore on exit given the owner seen at startup and a backup file
/// from a prior run (if any).
pub fn decide(owner: KeyOwner, prior_backup: Option<PathBuf>) -> SchemeAction {
    match owner {
        KeyOwner::Absent => SchemeAction::Delete,
        KeyOwner::SelfOwned => prior_backup
            .map(SchemeAction::Import)
            .unwrap_or(SchemeAction::Leave),
        // `Other` is upgraded to `Import` by the caller after a successful export.
        KeyOwner::Other => SchemeAction::Leave,
    }
}

static BACKUP: Mutex<Option<SchemeAction>> = Mutex::new(None);
static RESTORE_ARMED: AtomicBool = AtomicBool::new(false);

/// Remember the snapshot taken at startup.
pub fn remember(action: SchemeAction) {
    *BACKUP.lock().unwrap() = Some(action);
}

/// Arm the exit restore. Called after `register_all` returns so a quit that
/// beats the background startup thread cannot restore a snapshot for a
/// registration that never ran (or is still in flight).
pub fn arm_restore() {
    RESTORE_ARMED.store(true, AtomicOrdering::SeqCst);
}

#[cfg(test)]
pub(crate) fn restore_is_armed() -> bool {
    RESTORE_ARMED.load(AtomicOrdering::SeqCst)
}

/// Restore the remembered handler on exit.
pub fn restore_remembered() {
    if !RESTORE_ARMED.load(AtomicOrdering::SeqCst) {
        return;
    }
    let guard = BACKUP.lock().unwrap();
    if let Some(action) = guard.as_ref() {
        restore(action);
    }
}

#[cfg(windows)]
fn backup_path() -> Option<PathBuf> {
    crate::config::config_path()?.parent().map(|dir| dir.join("tidal-scheme.reg"))
}

#[cfg(windows)]
fn detect_owner(exe_name: &str) -> KeyOwner {
    let command_key = format!("{KEY}\\shell\\open\\command");
    match std::process::Command::new("reg")
        .args(["query", &command_key, "/ve"])
        .output()
    {
        Ok(out) if out.status.success() => {
            owner_from_query(&String::from_utf8_lossy(&out.stdout), exe_name)
        }
        _ => KeyOwner::Absent,
    }
}

#[cfg(windows)]
fn export_key(path: &Path) -> bool {
    let file = path.to_string_lossy().to_string();
    std::process::Command::new("reg")
        .args(["export", KEY, &file, "/y"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Snapshot the current HKCU `tidal` registration before registering.
#[cfg(windows)]
pub fn snapshot() -> SchemeAction {
    let exe_name = match std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    {
        Some(name) => name,
        None => return SchemeAction::Leave,
    };
    let owner = detect_owner(&exe_name);
    if owner == KeyOwner::Other {
        // Back up the official handler; only trust it once the export succeeded.
        return match backup_path() {
            Some(path) if export_key(&path) => SchemeAction::Import(path),
            _ => SchemeAction::Leave,
        };
    }
    decide(owner, backup_path().filter(|p| p.exists()))
}

#[cfg(not(windows))]
pub fn snapshot() -> SchemeAction {
    SchemeAction::Leave
}

/// Perform the action: import a saved registration, or delete OxiTide's key.
/// A failed import keeps the backup file for manual recovery.
#[cfg(windows)]
fn restore(action: &SchemeAction) {
    match action {
        SchemeAction::Import(path) if path.exists() => {
            let file = path.to_string_lossy().to_string();
            let ok = std::process::Command::new("reg")
                .args(["import", &file])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                let _ = std::fs::remove_file(path);
            }
        }
        SchemeAction::Delete => {
            let _ = std::process::Command::new("reg")
                .args(["delete", KEY, "/f"])
                .status();
        }
        _ => {}
    }
}

#[cfg(not(windows))]
fn restore(_action: &SchemeAction) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_scheme_owner() {
        let self_text = r#"HKEY_CURRENT_USER\Software\Classes\tidal\shell\open\command
    (Default)    REG_SZ    "C:\Program Files\OxiTide\OxiTide.exe" "%1""#;
        assert_eq!(owner_from_query(self_text, "oxitide.exe"), KeyOwner::SelfOwned);
        let other = r#"(Default)    REG_SZ    "C:\Program Files\TIDAL\TIDAL.exe" "%1""#;
        assert_eq!(owner_from_query(other, "oxitide.exe"), KeyOwner::Other);
        assert_eq!(owner_from_query("", "oxitide.exe"), KeyOwner::Absent);
        assert_eq!(owner_from_query("   \n", "oxitide.exe"), KeyOwner::Absent);
    }

    /// Mutates process-global state: run in isolation or accept the terminal
    /// armed state.
    #[test]
    fn restore_stays_disarmed_until_registration_completes() {
        assert!(!restore_is_armed(), "restore must start disarmed");
        arm_restore();
        assert!(restore_is_armed());
    }

    #[test]
    fn decides_what_to_restore_on_exit() {
        let backup = PathBuf::from("C:/tmp/tidal-scheme.reg");
        assert_eq!(decide(KeyOwner::Absent, None), SchemeAction::Delete);
        assert_eq!(
            decide(KeyOwner::SelfOwned, Some(backup.clone())),
            SchemeAction::Import(backup)
        );
        assert_eq!(decide(KeyOwner::SelfOwned, None), SchemeAction::Leave);
        assert_eq!(decide(KeyOwner::Other, None), SchemeAction::Leave);
    }
}
