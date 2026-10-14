//! Self-update (Track 12): checks the GitHub release manifest and installs a
//! newer build through `tauri-plugin-updater`. The webview never talks to the
//! plugin; it calls the thin commands in `commands.rs`, which land here.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::state::AppState;

/// Delay before the background check so it never competes with startup.
const STARTUP_CHECK_DELAY: Duration = Duration::from_secs(10);

/// What the UI needs to decide whether to offer updates at all.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateSupport {
    pub supported: bool,
    /// Why updates are unavailable (shown instead of the buttons).
    pub reason: Option<String>,
    pub current_version: String,
    pub check_on_start: bool,
}

/// A newer release found by a check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    /// RFC 3339 publication time from the manifest, when present.
    pub date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Windows installers always update; on Linux only an AppImage can replace
/// itself (deb/AUR installs belong to the package manager).
fn support_reason(windows: bool, linux: bool, appimage: bool) -> Option<&'static str> {
    if windows || (linux && appimage) {
        None
    } else if linux {
        Some("Updates are managed by your package manager for this install.")
    } else {
        Some("Updates are not available on this platform.")
    }
}

pub fn support(check_on_start: bool) -> UpdateSupport {
    let reason = support_reason(
        cfg!(windows),
        cfg!(target_os = "linux"),
        std::env::var_os("APPIMAGE").is_some(),
    );
    UpdateSupport {
        supported: reason.is_none(),
        reason: reason.map(str::to_string),
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        check_on_start,
    }
}

fn ensure_supported(state: &AppState) -> Result<(), String> {
    match support(state.config().updates.check_on_start).reason {
        None => Ok(()),
        Some(reason) => Err(format!("unsupported: {reason}")),
    }
}

/// Look for a newer release and remember it for `install`. `Ok(None)` means
/// this build is current.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let state = app.state::<AppState>();
    ensure_supported(&state)?;
    let updater = app.updater().map_err(|e| format!("update: {e}"))?;
    let found = updater.check().await.map_err(|e| format!("update: {e}"))?;
    let info = found.as_ref().map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().filter(|body| !body.trim().is_empty()),
        date: update.date.map(|date| date.to_string()),
    });
    *state.pending_update.lock().unwrap() = found;
    Ok(info)
}

/// Download and install the update `check` found, then hand over to the new
/// build. Windows' installer ends this process itself, so the same shutdown
/// work as a normal exit runs first; Linux restarts through the usual exit.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    ensure_supported(&state)?;
    let update = state
        .pending_update
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "no update is pending; check for updates first".to_string())?;
    let mut downloaded = 0u64;
    let progress_app = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update_progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| format!("update: {e}"))?;
    #[cfg(windows)]
    crate::run_exit_work(app);
    update.install(bytes).map_err(|e| format!("update: {e}"))?;
    #[cfg(not(windows))]
    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

/// Background check shortly after launch. Silent on failure; a found update is
/// announced with `update_available` and never downloaded unasked.
pub fn spawn_startup_check(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let state = app.state::<AppState>();
    let config = state.config();
    if !config.updates.check_on_start || support(true).reason.is_some() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_CHECK_DELAY).await;
        match check(&app).await {
            Ok(Some(info)) => {
                tracing::info!(version = %info.version, "update available");
                let _ = app.emit("update_available", info);
            }
            Ok(None) => tracing::info!("no update available"),
            Err(e) => tracing::info!("startup update check failed: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_always_supports_updates() {
        assert_eq!(support_reason(true, false, false), None);
    }

    #[test]
    fn linux_needs_an_appimage() {
        assert_eq!(support_reason(false, true, true), None);
        assert!(support_reason(false, true, false).unwrap().contains("package manager"));
    }

    #[test]
    fn other_platforms_are_unsupported() {
        assert!(support_reason(false, false, false).is_some());
    }

    #[test]
    fn support_reports_the_crate_version_and_setting() {
        let report = support(false);
        assert_eq!(report.current_version, env!("CARGO_PKG_VERSION"));
        assert!(!report.check_on_start);
        assert_eq!(report.supported, report.reason.is_none());
    }
}
