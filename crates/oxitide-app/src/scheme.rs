//! Windows custom-scheme ownership hygiene (spec §5.2). OxiTide registers
//! `tidal://` at startup so TIDAL's login redirect reaches it; because the
//! official TIDAL desktop app may own the same scheme, we snapshot the existing
//! HKCU registration before registering and restore it on exit.

use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(windows)]
const KEY: &str = r"HKCU\Software\Classes\tidal";

/// True when `reg query` output points the scheme at OxiTide itself
/// (case-insensitive), meaning there is nothing of the official app to preserve.
pub fn points_to_self(reg_output: &str, exe_fragment: &str) -> bool {
    reg_output
        .to_ascii_lowercase()
        .contains(&exe_fragment.to_ascii_lowercase())
}

static BACKUP: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

/// Remember the pre-registration snapshot (or `None` when there was none).
pub fn remember(backup: Option<PathBuf>) {
    *BACKUP.lock().unwrap() = Some(backup);
}

/// Restore the remembered handler on exit.
pub fn restore_remembered() {
    let guard = BACKUP.lock().unwrap();
    if let Some(backup) = guard.as_ref() {
        restore(backup);
    }
}

#[cfg(windows)]
fn backup_path() -> Option<PathBuf> {
    crate::config::config_path()?.parent().map(|dir| dir.join("tidal-scheme.reg"))
}

#[cfg(windows)]
fn scheme_points_to_self(exe_name: &str) -> bool {
    let command_key = format!("{KEY}\\shell\\open\\command");
    let mut text = String::new();
    for args in [["query", KEY, "/ve"], ["query", command_key.as_str(), "/ve"]] {
        if let Ok(out) = std::process::Command::new("reg").args(args).output() {
            text.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }
    points_to_self(&text, exe_name)
}

/// Snapshot the current HKCU `tidal` registration before registering. Returns
/// `None` when the key is absent (exit then deletes OxiTide's key and falls back
/// to an HKLM registration) or already points at OxiTide.
#[cfg(windows)]
pub fn snapshot() -> Option<PathBuf> {
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))?;
    if scheme_points_to_self(&exe_name) {
        return None;
    }
    let path = backup_path()?;
    let file = path.to_string_lossy().to_string();
    let ok = std::process::Command::new("reg")
        .args(["export", KEY, &file, "/y"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then_some(path)
}

#[cfg(not(windows))]
pub fn snapshot() -> Option<PathBuf> {
    None
}

/// Import the snapshot, or delete OxiTide's key when there was none.
#[cfg(windows)]
fn restore(backup: &Option<PathBuf>) {
    match backup {
        Some(path) if path.exists() => {
            let file = path.to_string_lossy().to_string();
            let _ = std::process::Command::new("reg")
                .args(["import", &file])
                .status();
            let _ = std::fs::remove_file(path);
        }
        _ => {
            let _ = std::process::Command::new("reg")
                .args(["delete", KEY, "/f"])
                .status();
        }
    }
}

#[cfg(not(windows))]
fn restore(_backup: &Option<PathBuf>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_self_registration_case_insensitively() {
        let text = r#"HKEY_CURRENT_USER\Software\Classes\tidal\shell\open\command
    (Default)    REG_SZ    "C:\Program Files\OxiTide\OxiTide.exe" "%1""#;
        assert!(points_to_self(text, "oxitide.exe"));
        assert!(!points_to_self(text, "tidal.exe"));
    }
}
