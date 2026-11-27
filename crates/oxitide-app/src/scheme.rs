//! Windows custom-scheme ownership hygiene (spec §5.2). OxiTide registers
//! `tidal://` at startup so TIDAL's login redirect reaches it; because the
//! official TIDAL desktop app may own the same scheme, we snapshot the existing
//! HKCU registration before registering and restore it on exit.
//!
//! The guard is deliberately conservative: it never deletes a key it could not
//! first back up, and after a crash (the key already points at OxiTide) it
//! reuses the prior run's backup rather than discarding the official handler.
//!
//! The exit restore runs in-process through the registry API, never through
//! `reg.exe`: during a Windows sign-out or restart new processes cannot start
//! (a console child fails DLL initialisation with 0xc0000142), so a spawned
//! `reg.exe` popped an "Application Error" dialog and the restore never ran
//! (#15). The backup is therefore a registry copy, not a `.reg` file.

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Mutex;

/// The scheme's HKCU registration, relative to `HKEY_CURRENT_USER`.
#[cfg(windows)]
const SCHEME_KEY: &str = r"Software\Classes\tidal";
/// Where the official handler is copied while OxiTide owns the scheme.
#[cfg(windows)]
const BACKUP_KEY: &str = r"Software\OxiTide\tidal-scheme-backup";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum KeyOwner {
    /// No `tidal` handler under HKCU.
    Absent,
    /// The handler points at OxiTide itself.
    SelfOwned,
    /// The handler belongs to another app (the official TIDAL client).
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeAction {
    /// Copy the backed-up official registration back on exit.
    Restore,
    /// Delete OxiTide's registration (nothing existed before).
    Delete,
    /// Leave the key untouched (cannot safely back up or recover it).
    Leave,
}

/// Classify the scheme's registration from whether the key exists and its
/// `shell\open\command` default value (`None` when that is missing or
/// unreadable). A key without a readable command still belongs to someone
/// else: it is backed up, never deleted.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn classify(key_exists: bool, command: Option<&str>, exe_fragment: &str) -> KeyOwner {
    if !key_exists {
        return KeyOwner::Absent;
    }
    match command {
        Some(command)
            if command
                .to_ascii_lowercase()
                .contains(&exe_fragment.to_ascii_lowercase()) =>
        {
            KeyOwner::SelfOwned
        }
        _ => KeyOwner::Other,
    }
}

/// What to restore on exit given the owner seen at startup and whether a
/// backup from a prior run exists.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn decide(owner: KeyOwner, has_prior_backup: bool) -> SchemeAction {
    match owner {
        KeyOwner::Absent => SchemeAction::Delete,
        KeyOwner::SelfOwned if has_prior_backup => SchemeAction::Restore,
        KeyOwner::SelfOwned => SchemeAction::Leave,
        // `Other` is upgraded to `Restore` by the caller after a successful backup.
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

/// Thin wrappers over the Win32 registry API, all under `HKEY_CURRENT_USER`.
#[cfg(windows)]
mod registry {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCopyTreeW, RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegOpenKeyExW,
        HKEY, HKEY_CURRENT_USER, KEY_ALL_ACCESS, KEY_READ, REG_OPTION_NON_VOLATILE, RRF_NOEXPAND,
        RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    };

    fn check(code: WIN32_ERROR) -> Result<(), WIN32_ERROR> {
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(code)
        }
    }

    /// An open key, closed on drop.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful open/create and is closed once.
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn open(path: &str) -> Result<Key, WIN32_ERROR> {
        let mut key = HKEY::default();
        // SAFETY: `key` is a valid out-pointer; the path is a live HSTRING.
        check(unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                None,
                KEY_READ,
                &mut key,
            )
        })?;
        Ok(Key(key))
    }

    /// Whether the key exists. An open that fails for any other reason than
    /// "not found" counts as existing, so the caller never deletes it unseen.
    pub fn exists(path: &str) -> bool {
        match open(path) {
            Ok(_) => true,
            Err(code) => code != ERROR_FILE_NOT_FOUND,
        }
    }

    /// The key's default value as a string (`REG_SZ` or unexpanded `REG_EXPAND_SZ`).
    pub fn default_string(path: &str) -> Result<String, WIN32_ERROR> {
        let path = HSTRING::from(path);
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
        let mut size = 0u32;
        // SAFETY: a size query (no data buffer) with a valid out-pointer.
        check(unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &path,
                PCWSTR::null(),
                flags,
                None,
                None,
                Some(&mut size),
            )
        })?;
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        // SAFETY: `buf` holds `size` bytes, which `size` reports to the call.
        check(unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &path,
                PCWSTR::null(),
                flags,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        })?;
        buf.truncate(size as usize / 2);
        while buf.last() == Some(&0) {
            buf.pop();
        }
        Ok(String::from_utf16_lossy(&buf))
    }

    /// Delete the key and everything under it; a missing key is not an error.
    pub fn delete_tree(path: &str) -> Result<(), WIN32_ERROR> {
        // SAFETY: plain call with a live HSTRING path.
        match check(unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(path)) }) {
            Err(code) if code == ERROR_FILE_NOT_FOUND => Ok(()),
            other => other,
        }
    }

    /// Copy `src` (its values and subkeys) into `dest`, creating `dest`.
    pub fn copy_tree(src: &str, dest: &str) -> Result<(), WIN32_ERROR> {
        let src = open(src)?;
        let mut dest_key = HKEY::default();
        // SAFETY: `dest_key` is a valid out-pointer; the path is a live HSTRING.
        check(unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(dest),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                // `RegCopyTreeW` also copies security descriptors, which needs
                // `WRITE_DAC` on the destination; `KEY_WRITE` lacks it.
                KEY_ALL_ACCESS,
                None,
                &mut dest_key,
                None,
            )
        })?;
        let dest = Key(dest_key);
        // SAFETY: both handles are open; a null subkey copies `src` itself.
        check(unsafe { RegCopyTreeW(src.0, PCWSTR::null(), dest.0) })
    }

    /// Set a key's default string value, creating the key (tests only).
    #[cfg(test)]
    pub fn set_default_string(path: &str, value: &str) -> Result<(), WIN32_ERROR> {
        use windows::Win32::System::Registry::{RegSetKeyValueW, REG_SZ};
        let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `data` is a NUL-terminated UTF-16 buffer of the stated byte length.
        check(unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                PCWSTR::null(),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        })
    }

    /// Replace `dest` with a copy of `src`. A failed copy leaves `src` intact.
    pub fn replace_tree(src: &str, dest: &str) -> Result<(), WIN32_ERROR> {
        delete_tree(dest)?;
        copy_tree(src, dest)
    }
}

#[cfg(windows)]
fn detect_owner(exe_name: &str) -> KeyOwner {
    let command = registry::default_string(&format!(r"{SCHEME_KEY}\shell\open\command")).ok();
    classify(registry::exists(SCHEME_KEY), command.as_deref(), exe_name)
}

/// Back the official handler up; a partial copy is discarded so a later run
/// never trusts it.
#[cfg(windows)]
fn back_up(scheme: &str, backup: &str) -> bool {
    match registry::replace_tree(scheme, backup) {
        Ok(()) => true,
        Err(code) => {
            tracing::warn!("tidal:// handler backup failed: {code:?}");
            let _ = registry::delete_tree(backup);
            false
        }
    }
}

/// Copy the backup back over the scheme key, then drop the backup. A failed
/// copy keeps the backup for the next run (it is retried after a restart).
#[cfg(windows)]
fn restore_backup(backup: &str, scheme: &str) -> bool {
    if !registry::exists(backup) {
        return false;
    }
    match registry::replace_tree(backup, scheme) {
        Ok(()) => {
            let _ = registry::delete_tree(backup);
            true
        }
        Err(code) => {
            tracing::warn!("tidal:// handler restore failed: {code:?}");
            false
        }
    }
}

/// The `.reg` file older builds exported the official handler to.
#[cfg(windows)]
fn legacy_backup_path() -> Option<std::path::PathBuf> {
    crate::config::config_path()?
        .parent()
        .map(|dir| dir.join("tidal-scheme.reg"))
}

/// Builds up to 1.1.10 restored the official handler with `reg import` on
/// exit, which fails during a sign-out or restart (#15) and leaves the
/// `.reg` file behind with OxiTide still owning the scheme. Import it once
/// here, at startup, where starting `reg.exe` works; the snapshot that
/// follows then backs the official handler up into the registry.
#[cfg(windows)]
fn migrate_legacy_backup(exe_name: &str) {
    use std::os::windows::process::CommandExt;
    // `reg.exe` is a console program: without it the GUI build flashes a console.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Some(path) = legacy_backup_path().filter(|p| p.exists()) else {
        return;
    };
    if detect_owner(exe_name) != KeyOwner::SelfOwned || registry::exists(BACKUP_KEY) {
        return;
    }
    let imported = std::process::Command::new("reg")
        .creation_flags(CREATE_NO_WINDOW)
        .arg("import")
        .arg(&path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if imported {
        let _ = std::fs::remove_file(&path);
    } else {
        tracing::warn!("could not import the saved tidal:// handler; kept {path:?}");
    }
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
    migrate_legacy_backup(&exe_name);
    let owner = detect_owner(&exe_name);
    if owner == KeyOwner::Other {
        // Back up the official handler; only trust it once the copy succeeded.
        return if back_up(SCHEME_KEY, BACKUP_KEY) {
            SchemeAction::Restore
        } else {
            SchemeAction::Leave
        };
    }
    decide(owner, registry::exists(BACKUP_KEY))
}

#[cfg(not(windows))]
pub fn snapshot() -> SchemeAction {
    SchemeAction::Leave
}

/// Perform the action: copy the saved registration back, or delete OxiTide's
/// key. In-process only: this runs during a Windows sign-out, when no new
/// process can start.
#[cfg(windows)]
fn restore(action: &SchemeAction) {
    match action {
        SchemeAction::Restore => {
            restore_backup(BACKUP_KEY, SCHEME_KEY);
        }
        SchemeAction::Delete => {
            let _ = registry::delete_tree(SCHEME_KEY);
        }
        SchemeAction::Leave => {}
    }
}

#[cfg(not(windows))]
fn restore(_action: &SchemeAction) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_scheme_owner() {
        let self_command = r#""C:\Program Files\OxiTide\OxiTide.exe" "%1""#;
        assert_eq!(
            classify(true, Some(self_command), "oxitide.exe"),
            KeyOwner::SelfOwned
        );
        let other = r#""C:\Program Files\TIDAL\TIDAL.exe" "%1""#;
        assert_eq!(classify(true, Some(other), "oxitide.exe"), KeyOwner::Other);
        // A key with no readable command is someone else's: back it up, never delete it.
        assert_eq!(classify(true, None, "oxitide.exe"), KeyOwner::Other);
        assert_eq!(classify(true, Some(""), "oxitide.exe"), KeyOwner::Other);
        assert_eq!(classify(false, None, "oxitide.exe"), KeyOwner::Absent);
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
        assert_eq!(decide(KeyOwner::Absent, false), SchemeAction::Delete);
        assert_eq!(decide(KeyOwner::Absent, true), SchemeAction::Delete);
        assert_eq!(decide(KeyOwner::SelfOwned, true), SchemeAction::Restore);
        assert_eq!(decide(KeyOwner::SelfOwned, false), SchemeAction::Leave);
        assert_eq!(decide(KeyOwner::Other, false), SchemeAction::Leave);
    }

    /// The backup round trip on a scratch HKCU key (never the real `tidal` one).
    #[cfg(windows)]
    #[test]
    fn backs_up_and_restores_a_handler_in_process() {
        /// Deletes the scratch tree even when an assertion fails.
        struct Scratch(String);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = registry::delete_tree(&self.0);
            }
        }

        let root = Scratch(format!(
            r"Software\OxiTide-test-scheme-{}",
            std::process::id()
        ));
        let scheme = format!(r"{}\tidal", root.0);
        let backup = format!(r"{}\backup", root.0);
        let command = format!(r"{scheme}\shell\open\command");
        let icon = format!(r"{scheme}\DefaultIcon");

        registry::set_default_string(&command, r#""C:\TIDAL\TIDAL.exe" "%1""#).unwrap();
        assert!(back_up(&scheme, &backup));
        // OxiTide takes the scheme over, adding a subkey the official one lacks...
        registry::set_default_string(&command, r#""C:\OxiTide\OxiTide.exe" "%1""#).unwrap();
        registry::set_default_string(&icon, r"C:\OxiTide\OxiTide.exe,0").unwrap();
        assert_eq!(
            classify(
                registry::exists(&scheme),
                registry::default_string(&command).ok().as_deref(),
                "oxitide.exe"
            ),
            KeyOwner::SelfOwned
        );
        // ...and gives the official handler back on exit.
        assert!(restore_backup(&backup, &scheme));
        assert_eq!(
            registry::default_string(&command).unwrap(),
            r#""C:\TIDAL\TIDAL.exe" "%1""#
        );
        assert!(!registry::exists(&icon), "the restore replaces, not merges");
        assert!(!registry::exists(&backup), "a used backup is dropped");
        assert!(!restore_backup(&backup, &scheme), "no backup, no restore");

        // `Delete` on exit removes the whole registration.
        registry::delete_tree(&scheme).unwrap();
        assert!(!registry::exists(&scheme));
        assert_eq!(
            classify(registry::exists(&scheme), None, "oxitide.exe"),
            KeyOwner::Absent
        );
    }
}
