# OxiTide Plan 5 — Polish, Packaging & Paged Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first shareable OxiTide installer (0.2.0, NSIS per-user) and remove the remaining dead-end states: session expiry becomes a re-login that preserves the queue and position, network failures become retryable, crashes leave a log file, the window remembers its geometry, and Library/Playlist paint after one page instead of walking every page.

**Architecture:** `oxitide-app` gains a `window` module (pure on-screen clamp + capture/apply), file logging via `tracing-appender`, an `AppState` session-expiry signal (`fail_tidal` + engine `code: "auth"`) emitted to the UI on `session_expired`, and paged track commands. The engine gains `EngineCmd::Retry` and an auth error code. The UI gains `ErrorState`, a session-cached `createTrackPager` store, and the capture/restore flow in `App.svelte`.

**Tech Stack:** Rust 2021 (stable), Tauri 2.11, `tracing-subscriber 0.3` + `tracing-appender 0.2`, Svelte 5 runes + Vite + TypeScript, Vitest, wiremock.

**Spec:** `docs/superpowers/specs/2026-09-23-oxitide-plan5-polish-packaging-design.md`

## Global Constraints

- Rust edition 2021; toolchain pinned by `rust-toolchain.toml` (clippy + rustfmt). Node 20+. Tauri 2.x. Svelte 5 runes only (`$state`/`$props`/`$effect`; no stores, no `export let`).
- Windows-first. Config/app dir `%APPDATA%\OxiTide`; file `config.toml`; logs `%APPDATA%\OxiTide\logs`; default cache dir `%APPDATA%\OxiTide\cache`.
- `tracing-appender = "0.2"` is the only new dependency and only in `oxitide-app`. No new Tauri plugins: the logs folder opens through the already-installed `tauri-plugin-opener` (called from Rust, so the URL-only capability is unchanged); window state lives in `config.toml` via the existing `Config`/`directories` stack.
- Every new `#[tauri::command]` must also be added to `generate_handler![...]` in `crates/oxitide-app/src/main.rs`.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`position_ms` ↔ `positionMs`). Rust DTOs are the source of truth; update `ui/src/lib/api.ts` by hand.
- No token values are logged; the existing signed-URL redaction applies to file logs too. Unit tests use `MemoryStore`, never the real keyring.
- No live network in tests: HTTP tests use `wiremock`; audio tests use committed tone fixtures, `MapFetcher`, and `NullOutput`; window tests are pure functions.
- Version 0.2.0 lands in the root `Cargo.toml` and `crates/oxitide-app/tauri.conf.json` in one commit so `Cargo.lock` and the bundle agree.
- CI gates (all must pass at the end of every task that touches Rust/UI): `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`. One successful `npm --prefix ui run app:build` is required in Tasks 1 and 13.
- Conventional commits (`feat:`, `fix:`, `test:`, `chore:`, `docs:`). Commit at the end of every task. Never commit generated installers or secrets.

## Review Focus

Uncovered input classes / failure modes most likely to bite a real user; each is pinned to a test in its owning task:

1. **Session expires while the queue is paused** — re-login must restore the queue paused at the last position, not start playing. Owning task: 9 (`re-pauses a paused session`).
2. **Session expires with an empty queue** — restore must be a no-op, not call the engine with an empty queue. Owning task: 9 (`does nothing for an empty queue`).
3. **A full page of tracks the pager already has** — the offset must still advance so `ensureAll` terminates instead of looping forever. Owning task: 10 (`advances the offset when a full page repeats already-loaded tracks`).
4. **A page fetch fails inside `ensureAll`** — it must stop and surface the error, not spin. Owning task: 10 (`stops ensureAll on the first error instead of looping`).
5. **No monitors reported** (headless/odd RDP session) — the saved position must be ignored without panicking. Owning task: 3 (`no_monitors_rejects_every_position`).

## Plan-level decisions (deviations from the spec text, with rationale)

| # | Decision | Rationale |
|---|---|---|
| D1 | NSIS/WebView2 options go under `bundle.windows`: `bundle.windows.nsis.installMode = "currentUser"` and `bundle.windows.webviewInstallMode = { "type": "downloadBootstrapper" }` — not `bundle.nsis` as the spec's §5.1 shorthand says. | Tauri 2's `BundleConfig` has no `nsis` field; NSIS and WebView2 config live on `WindowsConfig` (`tauri-utils 2.9.3`). A literal `bundle.nsis` key fails config validation (`deny_unknown_fields`). |
| D2 | Window geometry is captured on `RunEvent::WindowEvent { event: CloseRequested }`, not `RunEvent::Exit`. | Tauri removes the window from the manager on `WindowEvent::Destroyed` (before `RunEvent::Exit`), so `get_webview_window("main")` is `None` at Exit and the spec's capture would silently no-op. |
| D3 | `visible_position(x, y, width, height, monitors)` takes the window size. | The 200×100 "corner visible" rule needs the window rect; the spec's signature omits it. |
| D4 | Geometry is stored in physical pixels (position and size). `apply` restores the size only once `window_x`/`window_y` exist; the first run keeps `tauri.conf.json`'s logical 1200×800. | `outer_position`/`inner_size` and monitor work areas are all physical; mixing units (or restoring the default 1200×800 as physical on a high-DPI display) would shrink the window. An off-screen saved position still restores the size, per the spec's error table. |
| D5 | `"visible": false` lands with the window-state task (3), not the packaging task (1). | `main.rs` must show the window after geometry is applied; shipping `visible:false` before that code exists would make the app windowless between tasks. |
| D6 | Daily-rotated logs are named `oxitide.YYYY-MM-DD.log` (`filename_prefix("oxitide")` + `filename_suffix("log")`). | `tracing-appender` inserts the date between prefix and suffix; rotation requires the dated name, so the spec's `oxitide.log` becomes the dated form. |
| D7 | `Engine::retry` seeks to the recorded position only when `position_track` matches the current track (else 0), mirroring `restart_current_at_position`. | Prevents seeking a freshly current track into a stale position if a failure lands between a track change and its first position update. |
| D8 | `window_x`/`window_y` use `#[serde(skip_serializing_if = "Option::is_none")]`. | `toml 1.x` serialization errors on `Option::None` ("unsupported none value"); without this, every `config.save()` (volume, quality, cache size) would start failing. |
| D9 | `player.retry()` clears the mirrored error locally after the command succeeds. | The engine's retry emits a `State` event, and the reducer only clears `error` on `now_playing`; without this the player bar would keep showing the old error after a successful retry. |

---

### Task 1: Packaging metadata, version 0.2.0, and the release checklist

**Milestone:** P5-1 (config + checklist; Task 13 re-runs the build on the final tree and performs the human install checks).

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/oxitide-app/tauri.conf.json`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Create: `docs/superpowers/plans/notes/plan5-release.md`
- Modify: `Cargo.lock` (regenerated by cargo)

**Interfaces:**
- Consumes: nothing.
- Produces: version `0.2.0` in both manifests; NSIS per-user + WebView2 download-bootstrapper bundle config; the release checklist Task 13 fills in.

- [ ] **Step 1: Bump the workspace version**

In `Cargo.toml`, change `[workspace.package]`:

```toml
[workspace.package]
version = "0.2.0"
edition = "2021"
license = "AGPL-3.0-or-later"
```

- [ ] **Step 2: Update `crates/oxitide-app/tauri.conf.json`**

Replace the whole file with:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "OxiTide",
  "version": "0.2.0",
  "identifier": "org.codeberg.zephulbr.oxitide",
  "build": {
    "frontendDist": "../../ui/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": { "script": "npm run dev", "cwd": "../../ui" },
    "beforeBuildCommand": { "script": "npm run build", "cwd": "../../ui" }
  },
  "app": {
    "windows": [
      { "title": "OxiTide", "width": 1200, "height": 800, "dragDropEnabled": false }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis"],
    "publisher": "OxiTide",
    "copyright": "AGPL-3.0-or-later",
    "category": "Music",
    "shortDescription": "Unofficial TIDAL desktop client",
    "longDescription": "OxiTide is an unofficial TIDAL desktop client for Windows with bit-perfect playback, a streaming cache, and a native Tauri UI.",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.png",
      "icons/icon.ico"
    ],
    "windows": {
      "webviewInstallMode": { "type": "downloadBootstrapper" },
      "nsis": { "installMode": "currentUser" }
    }
  }
}
```

Note the nesting (D1): `bundle.windows.nsis.installMode` and `bundle.windows.webviewInstallMode`. `"visible": false` is deliberately **not** added here; Task 3 adds it with the code that shows the window.

- [ ] **Step 3: Create the release checklist**

Create `docs/superpowers/plans/notes/plan5-release.md`:

````markdown
# Plan 5 Release Checklist — OxiTide 0.2.0

> Human checklist for the first shareable installer. Record results inline:
> `[x]` pass, `[ ]` fail/not run, with a one-line note after a failure.

## Version bump (both files in one commit)
- [ ] `Cargo.toml` → `[workspace.package] version = "0.2.0"`
- [ ] `crates/oxitide-app/tauri.conf.json` → `"version": "0.2.0"`
- [ ] `cargo test --workspace` regenerates `Cargo.lock`; commit it with both files.

## Build
```powershell
npm --prefix ui run app:build
```
Artifacts (unsigned):
- NSIS: `target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe` (per-user, no admin prompt)
- MSI: `target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi`

The WebView2 bootstrapper is downloaded at install time, so the installer needs
internet on a machine without WebView2 (Windows 10/11 normally ships it).

## Install / launch / upgrade / uninstall
- [ ] Run the NSIS setup as a normal user: no UAC/admin prompt; installs under `%LOCALAPPDATA%\OxiTide`.
- [ ] Launch from the Start Menu: the window opens at the saved size/position and the app works.
- [ ] SmartScreen warns "Windows protected your PC" (unsigned): More info → Run anyway is expected.
- [ ] Sign in and play a track; note the queue and position.
- [ ] Run the same setup again over the installed version: upgrade in place succeeds.
- [ ] `%APPDATA%\OxiTide\config.toml`, the cache directory, and the keyring entry survive the upgrade.
- [ ] Uninstall from Settings → Apps: the install directory is removed; `%APPDATA%\OxiTide` (config, cache, logs) is kept.
- [ ] The MSI target still builds (`target/release/bundle/msi/...`) even though NSIS is primary.

## Records
- Build commit:
- Build date:
- Notes:
````

- [ ] **Step 4: Update README and AGENTS.md**

In `README.md`, replace the build line in the Development block:

```markdown
# Build a packaged installer (NSIS per-user / MSI)
npm --prefix ui run app:build
# NSIS: target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe
# MSI:  target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi
```

Add below the code block:

```markdown
> The installer is unsigned (SmartScreen warns) and installs per user without
> admin rights. It downloads the WebView2 bootstrapper when WebView2 is missing.
```

In `AGENTS.md`, replace the packaging line:

```markdown
- Package installer: `npm --prefix ui run app:build` (NSIS per-user + MSI; artifacts under `target/release/bundle/`). Unsigned; WebView2 downloads at install time.
```

- [ ] **Step 5: Regenerate `Cargo.lock` and run the Rust gates**

Run:

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all green, and `git status` shows `Cargo.lock` modified (the three workspace package entries now say `0.2.0`).

- [ ] **Step 6: Build the installers**

Run (slow: release build + bundling):

```powershell
npm --prefix ui run app:build
```

Expected: exit 0. Verify both artifacts:

```powershell
Test-Path "target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe"
Test-Path "target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi"
```

Both must print `True`. If the config has a typo, `tauri build` fails immediately while parsing `tauri.conf.json` — fix and re-run.

- [ ] **Step 7: Commit**

```powershell
git add Cargo.toml Cargo.lock crates/oxitide-app/tauri.conf.json README.md AGENTS.md docs/superpowers/plans/notes/plan5-release.md
git commit -m "chore(release): package OxiTide 0.2.0"
```

---

### Task 2: File logging

**Milestone:** P5-2.

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml`
- Modify: `crates/oxitide-app/src/config.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 7 and 13):
  - `config::logs_dir() -> Option<PathBuf>`.
  - `get_logs_dir() -> String`, `open_logs_folder(app: tauri::AppHandle) -> Result<(), String>`.
  - `api.getLogsDir(): Promise<string>`, `api.openLogsFolder(): Promise<void>`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/oxitide-app/src/config.rs`:

```rust
    /// Review Focus: the logs directory must be `%APPDATA%\OxiTide\logs`, not
    /// the config directory (`ProjectDirs::config_dir()` appends `\config`).
    #[test]
    fn logs_dir_is_under_the_oxitide_app_dir() {
        let dir = logs_dir().expect("no project directories available");
        assert!(dir.ends_with("logs"), "unexpected logs dir: {}", dir.display());
        assert_eq!(
            dir.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()),
            Some("OxiTide"),
            "logs must live directly under the app dir: {}",
            dir.display()
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app logs_dir`
Expected: compile error — `logs_dir` not found.

- [ ] **Step 3: Implement `logs_dir`**

In `crates/oxitide-app/src/config.rs`, after `default_cache_dir`:

```rust
/// `%APPDATA%\OxiTide\logs` (the app root, not the config directory:
/// `ProjectDirs::config_dir()` appends `\config`).
pub fn logs_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("OxiTide").join("logs"))
}
```

Run: `cargo test -p oxitide-app logs_dir`
Expected: PASS.

- [ ] **Step 4: Add the dependency**

In `crates/oxitide-app/Cargo.toml` under `[dependencies]`:

```toml
tracing-appender = "0.2"
```

- [ ] **Step 5: Implement the logging init in `main.rs`**

At the top of `crates/oxitide-app/src/main.rs`, add imports:

```rust
use std::sync::OnceLock;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
```

Add above `fn main()`:

```rust
/// Keeps the non-blocking file writer's worker thread alive for the process.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// The daily-rotated file writer, or `None` when the logs directory cannot be
/// created (the app then logs to the console only).
fn file_writer() -> Option<tracing_appender::non_blocking::NonBlocking> {
    let dir = config::logs_dir()?;
    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("file logging disabled: cannot create {}", dir.display());
        return None;
    }
    let appender = match tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("oxitide")
        .filename_suffix("log")
        .max_log_files(5)
        .build(&dir)
    {
        Ok(appender) => appender,
        Err(e) => {
            eprintln!("file logging disabled: {e}");
            return None;
        }
    };
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let _ = LOG_GUARD.set(guard);
    Some(writer)
}

/// The console layer plus a daily-rotated file log; `RUST_LOG` overrides the
/// `info` default. The console layer keeps its previous destination (`fmt`
/// defaults to stdout).
fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer());
    match file_writer() {
        Some(writer) => registry
            .with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer))
            .init(),
        None => registry.init(),
    }
}
```

Replace the first statement of `main()`:

```rust
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
```

with:

```rust
    init_logging();
```

- [ ] **Step 6: Add the logs commands**

In `crates/oxitide-app/src/commands.rs`, after `set_cache_clear_on_exit`:

```rust
#[tauri::command]
pub fn get_logs_dir() -> String {
    crate::config::logs_dir()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default()
}

#[tauri::command]
pub fn open_logs_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = crate::config::logs_dir().ok_or_else(|| "no logs directory available".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
```

Register them in the `generate_handler![...]` list in `main.rs`:

```rust
            commands::get_logs_dir,
            commands::open_logs_folder,
```

- [ ] **Step 7: Add the UI surface**

In `ui/src/lib/api.ts`, add to the `api` object after `clearCache`:

```ts
  getLogsDir: () => invoke<string>("get_logs_dir"),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
```

In `ui/src/views/Settings.svelte`:

1. Add state next to `cacheStatus`:

```ts
  let logsDir = $state("");
```

2. Extend the initial load's `Promise.all`:

```ts
  $effect(() => {
    Promise.all([api.getConfig(), api.getOutputDevices(), api.getCacheStatus(), api.getLogsDir()])
      .then(([loaded, list, status, logs]) => {
        devices = list;
        cacheStatus = status;
        logsDir = logs;
```

(The rest of the `.then` body is unchanged.)

3. Add a Logs section just before the closing `{/if}` of the `{#if config}` block (after the cache section's `Directory:` line):

```svelte
  <h3>Logs</h3>
  <p class="muted">Directory: {logsDir || "unavailable"}</p>
  <p><button onclick={() => api.openLogsFolder().catch(apply)}>Open logs folder</button></p>
```

- [ ] **Step 8: Run the gates**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Manual spot check: run the app, confirm a file matching `%APPDATA%\OxiTide\logs\oxitide.*.log` appears and grows (repeated in Task 13's checklist).

- [ ] **Step 9: Commit**

```powershell
git add crates/oxitide-app/Cargo.toml crates/oxitide-app/src/config.rs crates/oxitide-app/src/main.rs crates/oxitide-app/src/commands.rs ui/src/lib/api.ts ui/src/views/Settings.svelte Cargo.lock
git commit -m "feat(app): write logs to a rotating file"
```

---

### Task 3: Window state

**Milestone:** P5-3.

**Files:**
- Modify: `crates/oxitide-app/src/config.rs`
- Create: `crates/oxitide-app/src/window.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/tauri.conf.json`
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes: `Config` (existing).
- Produces (used by Task 6's setup edit):
  - `UiConfig { window_width: u32, window_height: u32, window_x: Option<i32>, window_y: Option<i32>, window_maximized: bool }`.
  - `window::MonitorRect`, `window::visible_position(x, y, width, height, monitors) -> Option<(i32, i32)>`, `window::apply(&WebviewWindow, &Config) -> tauri::Result<()>`, `window::capture(&WebviewWindow, &mut Config)`.
  - `api.getConfig()`'s `Config.ui` gains the window fields.

- [ ] **Step 1: Write the failing config tests**

In `crates/oxitide-app/src/config.rs`, update the `save_and_load_round_trip` literal:

```rust
            ui: UiConfig {
                quality_display: QualityDisplay::Detailed,
                window_width: 1280,
                window_height: 720,
                window_x: Some(40),
                window_y: Some(60),
                window_maximized: true,
            },
```

Extend `partial_file_fills_defaults` with:

```rust
        assert_eq!(config.ui.window_width, 1200);
        assert_eq!(config.ui.window_height, 800);
        assert_eq!(config.ui.window_x, None);
        assert_eq!(config.ui.window_y, None);
        assert!(!config.ui.window_maximized);
```

Add a new test:

```rust
    /// Review Focus: `toml` serialization errors on `None` fields, so the
    /// window position must be omitted (not written as a null) before the
    /// first exit, and every other save must keep working.
    #[test]
    fn default_config_omits_window_position_before_the_first_save() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        Config::default().save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("window_x"), "window_x written before the first save:\n{text}");
        assert!(!text.contains("window_y"), "window_y written before the first save:\n{text}");
        assert!(text.contains("window_width = 1200"));
        assert!(text.contains("window_maximized = false"));
        let loaded = Config::load_from(&path);
        assert_eq!(loaded.ui.window_x, None);
        assert_eq!(loaded.ui.window_maximized, false);
        let _ = fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app config`
Expected: compile errors — no fields `window_width`, etc.

- [ ] **Step 3: Implement the config fields**

Replace `UiConfig` and its `Default` in `crates/oxitide-app/src/config.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub quality_display: QualityDisplay,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
    pub window_maximized: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            quality_display: QualityDisplay::Compact,
            window_width: 1200,
            window_height: 800,
            window_x: None,
            window_y: None,
            window_maximized: false,
        }
    }
}
```

Run: `cargo test -p oxitide-app config`
Expected: all config tests PASS, including the previously failing `new_sections_round_trip_as_documented_toml` (it calls `save_to` on a default config).

- [ ] **Step 4: Write the failing window tests**

Add the module declaration at the top of `crates/oxitide-app/src/main.rs` so the new file is compiled (without it the test filter would match nothing and "pass"):

```rust
mod window;
```

Create `crates/oxitide-app/src/window.rs` with only the test module and the `use super::*;` import:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> MonitorRect {
        MonitorRect { x, y, width, height }
    }

    #[test]
    fn a_fully_visible_position_is_restored() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(100, 100, 1200, 800, &monitors), Some((100, 100)));
    }

    #[test]
    fn an_offscreen_position_is_rejected() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(2500, 200, 1200, 800, &monitors), None);
        assert_eq!(visible_position(-1500, 200, 1200, 800, &monitors), None);
    }

    #[test]
    fn a_partially_visible_position_is_restored_when_enough_shows() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(1700, 900, 1200, 800, &monitors), Some((1700, 900)));
    }

    #[test]
    fn a_sliver_of_visibility_is_rejected() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(1900, 1060, 1200, 800, &monitors), None);
        assert_eq!(visible_position(-1150, -750, 1200, 800, &monitors), None);
    }

    /// Review Focus: no monitors reported must ignore the position, not panic.
    #[test]
    fn no_monitors_rejects_every_position() {
        assert_eq!(visible_position(0, 0, 1200, 800, &[]), None);
    }

    #[test]
    fn a_second_monitor_counts() {
        let monitors = [monitor(0, 0, 1920, 1080), monitor(1920, 0, 2560, 1440)];
        assert_eq!(visible_position(2000, 100, 1200, 800, &monitors), Some((2000, 100)));
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app window`
Expected: compile errors — `MonitorRect`, `visible_position` not defined.

- [ ] **Step 6: Implement the window module**

Prepend to `crates/oxitide-app/src/window.rs` (above the tests):

```rust
//! Window geometry persistence: capture size/position/maximized state into
//! `config.toml` and restore it with an on-screen check.

use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::config::Config;

/// A monitor's work area in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Minimum visible window area for a saved position to be restored.
const MIN_VISIBLE_WIDTH: i64 = 200;
const MIN_VISIBLE_HEIGHT: i64 = 100;

/// Returns `(x, y)` when at least a 200x100 corner of the window intersects a
/// monitor's work area, else `None`.
pub fn visible_position(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitors: &[MonitorRect],
) -> Option<(i32, i32)> {
    let visible = monitors.iter().any(|m| {
        let overlap_w =
            (x as i64 + width as i64).min(m.x as i64 + m.width as i64) - (x.max(m.x) as i64);
        let overlap_h =
            (y as i64 + height as i64).min(m.y as i64 + m.height as i64) - (y.max(m.y) as i64);
        overlap_w >= MIN_VISIBLE_WIDTH && overlap_h >= MIN_VISIBLE_HEIGHT
    });
    visible.then_some((x, y))
}

/// Apply the saved geometry. Maximized wins; a missing saved position leaves
/// the size from `tauri.conf.json` untouched (first run); an off-screen
/// position is ignored but the size is still restored.
pub fn apply(window: &WebviewWindow, config: &Config) -> tauri::Result<()> {
    let ui = &config.ui;
    if ui.window_maximized {
        return window.maximize();
    }
    if ui.window_x.is_none() && ui.window_y.is_none() {
        return Ok(());
    }
    window.set_size(PhysicalSize::new(ui.window_width, ui.window_height))?;
    if let (Some(x), Some(y)) = (ui.window_x, ui.window_y) {
        let monitors: Vec<MonitorRect> = window
            .available_monitors()?
            .iter()
            .map(|m| {
                let work = m.work_area();
                MonitorRect {
                    x: work.position.x,
                    y: work.position.y,
                    width: work.size.width,
                    height: work.size.height,
                }
            })
            .collect();
        if visible_position(x, y, ui.window_width, ui.window_height, &monitors).is_some() {
            window.set_position(PhysicalPosition::new(x, y))?;
        }
    }
    Ok(())
}

/// Read the window's current geometry into `config` (physical pixels).
pub fn capture(window: &WebviewWindow, config: &mut Config) {
    if let Ok(position) = window.outer_position() {
        config.ui.window_x = Some(position.x);
        config.ui.window_y = Some(position.y);
    }
    if let Ok(size) = window.inner_size() {
        config.ui.window_width = size.width;
        config.ui.window_height = size.height;
    }
    if let Ok(maximized) = window.is_maximized() {
        config.ui.window_maximized = maximized;
    }
}
```

- [ ] **Step 7: Run the window tests to verify they pass**

Run: `cargo test -p oxitide-app window`
Expected: all 6 tests PASS.

- [ ] **Step 8: Wire it into `main.rs`**

Add a save helper above `fn main()`:

```rust
/// Best-effort geometry save; called while the main window is closing.
fn save_window_state(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let mut config = state.config();
    window::capture(&window, &mut config);
    if let Err(e) = config.save() {
        tracing::warn!("failed to save window geometry: {e}");
    }
    state.set_config(config);
}
```

Add a `setup` hook to the builder, between `.manage(state)` and `.invoke_handler(...)`:

```rust
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let config = app.state::<AppState>().config();
                if let Err(e) = window::apply(&window, &config) {
                    tracing::warn!("failed to apply window geometry: {e}");
                }
                if let Err(e) = window.show() {
                    tracing::warn!("failed to show the main window: {e}");
                }
            }
            Ok(())
        })
```

Replace the `.run(|app_handle, event| { ... })` closure with:

```rust
        .run(|app_handle, event| match event {
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { .. },
                ..
            } if label == "main" => {
                save_window_state(app_handle);
            }
            tauri::RunEvent::Exit => {
                let state = app_handle.state::<AppState>();
                let config = state.config();
                if config.cache.clear_on_exit {
                    if let Some(cache) = state.cache() {
                        cache.clear();
                    }
                }
            }
            _ => {}
        });
```

The window is captured on `CloseRequested` (D2) because Tauri removes it from the manager on `Destroyed`, before `RunEvent::Exit`.

- [ ] **Step 9: Hide the window until geometry is applied**

In `crates/oxitide-app/tauri.conf.json`, add `"visible": false` to the window entry (D5):

```json
    "windows": [
      { "title": "OxiTide", "width": 1200, "height": 800, "dragDropEnabled": false, "visible": false }
    ],
```

- [ ] **Step 10: Mirror the config fields in the UI**

In `ui/src/lib/api.ts`, replace the `ui` line in `interface Config`:

```ts
  ui: {
    quality_display: "compact" | "detailed";
    window_width: number;
    window_height: number;
    window_x?: number | null;
    window_y?: number | null;
    window_maximized: boolean;
  };
```

- [ ] **Step 11: Run the gates**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Manual spot check: run the app, resize/move the window, close it, confirm `window_width`, `window_height`, `window_x`, `window_y`, `window_maximized` appear in `%APPDATA%\OxiTide\config.toml`, then relaunch and confirm the geometry is restored.

- [ ] **Step 12: Commit**

```powershell
git add crates/oxitide-app/src/config.rs crates/oxitide-app/src/window.rs crates/oxitide-app/src/main.rs crates/oxitide-app/tauri.conf.json ui/src/lib/api.ts
git commit -m "feat(app): remember window geometry"
```

---

### Task 4: Paged favorite and playlist tracks in the Tidal client

**Milestone:** P5-6/P5-7 (client half).

**Files:**
- Modify: `crates/oxitide-tidal/src/library.rs`
- Modify: `crates/oxitide-tidal/src/catalog.rs`

**Interfaces:**
- Consumes: `ItemPage` (`total_number_of_items`), existing `TidalClient::get`.
- Produces (used by Task 6):
  - `TidalClient::favorite_tracks_page(&self, limit: u32, offset: u32) -> Result<(Vec<Track>, u32)>`.
  - `TidalClient::playlist_tracks_page(&self, id: &str, limit: u32, offset: u32) -> Result<(Vec<Track>, u32)>`.
  - The single-page `favorite_tracks` / `playlist_tracks` helpers are removed; `all_*` walkers stay.

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-tidal/src/library.rs`, replace the `favorite_tracks_pages_by_offset` test with:

```rust
    #[tokio::test]
    async fn favorite_tracks_page_returns_items_and_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "200")).and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"created": "2025-01-01T00:00:00.000+0000", "item": {"id": 7, "title": "Fav"}}],
                "totalNumberOfItems": 201, "limit": 100, "offset": 200
            })))
            .mount(&server).await;
        let (tracks, total) = client(&server).favorite_tracks_page(100, 200).await.unwrap();
        assert_eq!(tracks[0].id, 7);
        assert_eq!(total, 201);
    }
```

In `crates/oxitide-tidal/src/catalog.rs`, replace the `playlist_tracks_parses_page` test with:

```rust
    #[tokio::test]
    async fn playlist_tracks_page_returns_items_and_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid/tracks"))
            .and(query_param("limit", "100")).and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 2, "title": "P1", "artists": [{"id": 9, "name": "X"}]}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let (tracks, total) = client(&server).playlist_tracks_page("p-uuid", 100, 0).await.unwrap();
        assert_eq!(tracks[0].artists[0].name, "X");
        assert_eq!(total, 1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal page_returns_items_and_total`
Expected: compile errors — no method `favorite_tracks_page` / `playlist_tracks_page`.

- [ ] **Step 3: Implement the page methods**

In `crates/oxitide-tidal/src/library.rs`, replace `favorite_tracks` with:

```rust
    /// One page of favorite tracks plus the server-reported total.
    pub async fn favorite_tracks_page(&self, limit: u32, offset: u32) -> Result<(Vec<Track>, u32)> {
        let p: ItemPage<Favorited<Track>> = self
            .get(&format!("{}/tracks", self.user_base()), &page(limit, offset))
            .await?;
        Ok((p.items.into_iter().map(|f| f.item).collect(), p.total_number_of_items))
    }
```

In `crates/oxitide-tidal/src/catalog.rs`, replace `playlist_tracks` with:

```rust
    /// One page of playlist tracks plus the server-reported total.
    pub async fn playlist_tracks_page(
        &self,
        id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Track>, u32)> {
        let page: ItemPage<Track> =
            self.get(&format!("playlists/{id}/tracks"), &page_params(limit, offset)).await?;
        Ok((page.items, page.total_number_of_items))
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all tidal tests PASS (`all_favorite_tracks_fetches_every_page`, `album_tracks_*`, and the two new page tests).

- [ ] **Step 5: Run the workspace gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add crates/oxitide-tidal/src/library.rs crates/oxitide-tidal/src/catalog.rs
git commit -m "feat(tidal): add paged favorites and playlist tracks"
```

---

### Task 5: Engine retry and the auth error code

**Milestone:** P5-5 (engine half), P5-4 (engine half).

**Files:**
- Modify: `crates/oxitide-audio/src/queue.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`

**Interfaces:**
- Consumes: existing `Queue`, `PendingSeek`, `worker_main`.
- Produces (used by Tasks 6, 8):
  - `Queue::resume()`.
  - `EngineCmd::Retry`.
  - `Engine::retry(&self) -> Result<()>`.
  - `EngineEvent::Error { code: "auth", .. }` for `Error::Tidal(oxitide_tidal::Error::Unauthorized)` stream failures.

- [ ] **Step 1: Write the failing queue tests**

Add to the `#[cfg(test)] mod tests` in `crates/oxitide-audio/src/queue.rs`:

```rust
    #[test]
    fn resume_keeps_items_and_sets_playing() {
        let mut q = queue(2);
        q.stop();
        q.resume();
        assert!(q.is_playing());
        assert_eq!(q.items().len(), 2);
    }

    #[test]
    fn resume_without_a_current_track_is_a_noop() {
        let mut q = Queue::default();
        q.resume();
        assert!(!q.is_playing());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio queue`
Expected: compile error — no method `resume`.

- [ ] **Step 3: Implement `Queue::resume`**

In `crates/oxitide-audio/src/queue.rs`, after `stop`:

```rust
    /// Resume playback without touching the queue (e.g. after a retry).
    pub fn resume(&mut self) {
        if self.current.is_some() {
            self.playing = true;
        }
    }
```

- [ ] **Step 4: Write the failing engine tests**

In `crates/oxitide-audio/src/engine.rs`'s test module, add `AtomicUsize` to the test imports (next to `use std::collections::HashMap;`):

```rust
    use std::sync::atomic::AtomicUsize;
```

Add the resolvers next to `FailingResolver`:

```rust
    /// Fails the first resolve, then succeeds; simulates a transient network
    /// failure that a Retry recovers from.
    struct FlakyResolver {
        plan: StreamPlan,
        calls: AtomicUsize,
    }

    impl StreamResolver for FlakyResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(Error::Fetch("network down".into()))
            } else {
                Ok(self.plan.clone())
            }
        }
    }

    struct AuthFailingResolver;

    impl StreamResolver for AuthFailingResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Err(Error::Tidal(oxitide_tidal::Error::Unauthorized))
        }
    }
```

Add the tests:

```rust
    #[test]
    fn retry_after_a_stream_error_restarts_the_current_track() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(FlakyResolver { plan, calls: AtomicUsize::new(0) }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })));
        assert!(!engine.state().playing, "a stream error must stop the queue");

        engine.retry().unwrap();

        wait_for(|| output.samples().len() > 0);
        assert!(engine.state().playing, "retry did not resume playback");
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(1));
    }

    #[test]
    fn retry_resumes_at_the_recorded_position() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() > 0);

        engine.handle(EngineCmd::Seek(1000)).unwrap();
        wait_for(|| engine.state().position_ms >= 1000);
        // Simulate the stop a stream error performs, keeping the position.
        engine.inner.queue.lock().unwrap().stop();
        let before = output.samples().len();

        engine.retry().unwrap();

        wait_for(|| output.samples().len() > before);
        assert!(engine.state().playing, "retry did not resume playback");
        assert!(
            engine.state().position_ms >= 1000,
            "retry restarted from the beginning: {}",
            engine.state().position_ms
        );
    }

    #[test]
    fn retry_without_a_current_item_is_a_noop() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        let before = engine.inner.generation.load(Ordering::SeqCst);

        engine.handle(EngineCmd::Retry).unwrap();

        assert_eq!(engine.inner.generation.load(Ordering::SeqCst), before);
        assert!(output.samples().is_empty());
        assert!(!engine.state().playing);
    }

    #[test]
    fn unauthorized_stream_error_is_coded_auth() {
        let output = NullOutput::new();
        let engine = build_engine(
            Arc::new(AuthFailingResolver),
            Arc::new(crate::fetch::MapFetcher::default()),
            Box::new(output),
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));

        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })));
        let code = seen
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                EngineEvent::Error { code, .. } => Some(code.clone()),
                _ => None,
            })
            .expect("no error event");
        assert_eq!(code, "auth");
    }
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio engine`
Expected: compile errors — no variant `Retry`, no method `retry`.

- [ ] **Step 6: Implement `EngineCmd::Retry` and `Engine::retry`**

In `crates/oxitide-audio/src/engine.rs`, add the variant to `EngineCmd`:

```rust
    Clear,
    /// Restart the current track at its last position (after an error).
    Retry,
```

Add the method next to `restart_current_at_position`:

```rust
    /// Restart the current track at its last position. Recovers a stream
    /// error, a device-loss pause, or a queue stopped at the end of a track.
    /// A no-op without a current item.
    pub fn retry(&self) -> Result<()> {
        let item = {
            let q = self.inner.queue.lock().unwrap();
            q.current().cloned()
        };
        if let Some(item) = item {
            self.inner.queue.lock().unwrap().resume();
            self.inner.paused.store(false, Ordering::SeqCst);
            // Only trust `position_ms` when it belongs to the current track.
            let position = if self.inner.position_track.load(Ordering::SeqCst) == item.track_id {
                self.inner.position_ms.load(Ordering::SeqCst)
            } else {
                0
            };
            *self.inner.seek_target.lock().unwrap() = Some(PendingSeek {
                track_id: item.track_id,
                position_ms: position,
                auto_start: true,
            });
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
            self.inner.notify_worker();
            self.emit(EngineEvent::State(self.state()));
        }
        Ok(())
    }
```

Add the `handle` arm after `EngineCmd::Clear`:

```rust
            EngineCmd::Retry => {
                self.retry()?;
            }
```

- [ ] **Step 7: Implement the auth error code**

In `worker_main`'s `SessionOutcome::StreamError(e)` arm, replace the emit line:

```rust
                emit(&inner, EngineEvent::Error { code: "playback".into(), message: e.to_string() });
```

with:

```rust
                let code = match &e {
                    Error::Tidal(oxitide_tidal::Error::Unauthorized) => "auth",
                    _ => "playback",
                };
                emit(&inner, EngineEvent::Error { code: code.into(), message: e.to_string() });
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio`
Expected: all audio tests PASS, including the four new ones.

- [ ] **Step 9: Run the workspace gates and commit**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
git add crates/oxitide-audio/src/queue.rs crates/oxitide-audio/src/engine.rs
git commit -m "feat(audio): retry playback and flag auth errors"
```

---

### Task 6: Session-expiry signal, new commands, and player replacement

**Milestone:** P5-4 (app half), P5-5/P5-6 (command surface).

**Files:**
- Modify: `crates/oxitide-app/src/state.rs`
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes: Task 4 (`favorite_tracks_page`, `playlist_tracks_page`), Task 5 (`Engine::retry`, `code: "auth"`).
- Produces (used by Tasks 7–12):
  - `AppState::{set_handle, session_expired, notify_session_expired, clear_session_expired, fail_tidal}`.
  - `TrackPageDto { items, total, offset, limit }`.
  - Commands: `get_favorite_tracks_page { offset, limit }`, `get_playlist_tracks_page { id, offset, limit }`, `retry`.
  - `session_expired` Tauri event.
  - `api.getFavoriteTracksPage`, `api.getPlaylistTracksPage`, `api.retry`, `api.TrackPage`.

- [ ] **Step 1: Write the failing state tests**

Add to the `#[cfg(test)] mod tests` in `crates/oxitide-app/src/state.rs`:

```rust
    #[test]
    fn fail_tidal_signals_only_unauthorized() {
        let state = AppState::default();
        assert_eq!(state.fail_tidal(Error::Unauthorized), "unauthorized");
        assert!(state.session_expired());

        let other = AppState::default();
        assert_eq!(other.fail_tidal(Error::RateLimited), "rate limited");
        assert!(!other.session_expired());
    }

    #[test]
    fn notify_session_expired_is_idempotent_without_a_handle() {
        let state = AppState::default();
        state.notify_session_expired();
        state.notify_session_expired();
        assert!(state.session_expired());
    }

    #[test]
    fn logout_clears_the_expired_flag() {
        let state = AppState::default();
        state.notify_session_expired();
        state.logout();
        assert!(!state.session_expired());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app state`
Expected: compile errors — no method `fail_tidal`, `session_expired`, `notify_session_expired`.

- [ ] **Step 3: Implement the state signal**

In `crates/oxitide-app/src/state.rs`, add imports:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

use oxitide_tidal::Error;
use tauri::{AppHandle, Emitter};
```

Add fields to `AppState`:

```rust
    pub cache: Mutex<Option<Arc<DiskCache>>>,
    session_expired: AtomicBool,
    handle: Mutex<Option<AppHandle>>,
```

(`#[derive(Default)]` still works: `AtomicBool` defaults to `false`, `Mutex<Option<_>>` to `None`.)

Add methods:

```rust
    /// Remember the app handle so `notify_session_expired` can reach the UI.
    pub fn set_handle(&self, handle: AppHandle) {
        *self.handle.lock().unwrap() = Some(handle);
    }

    pub fn session_expired(&self) -> bool {
        self.session_expired.load(Ordering::SeqCst)
    }

    /// Flag the session as expired and tell the UI to show the login screen.
    /// Idempotent: the first signal wins.
    pub fn notify_session_expired(&self) {
        if self.session_expired.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::warn!("TIDAL session expired; waiting for a new login");
        if let Some(handle) = self.handle.lock().unwrap().as_ref() {
            let _ = handle.emit("session_expired", ());
        }
    }

    pub fn clear_session_expired(&self) {
        self.session_expired.store(false, Ordering::SeqCst);
    }

    /// Map a TIDAL client error to a command error string, signalling the UI
    /// when the session is no longer valid.
    pub fn fail_tidal(&self, error: Error) -> String {
        if matches!(error, Error::Unauthorized) {
            self.notify_session_expired();
        }
        error.to_string()
    }
```

Add `self.clear_session_expired();` to `logout()`:

```rust
    pub fn logout(&self) {
        *self.client.write().unwrap() = None;
        *self.player.write().unwrap() = None;
        self.clear_session_expired();
    }
```

Run: `cargo test -p oxitide-app state`
Expected: all state tests PASS.

- [ ] **Step 4: Add the DTO and commands**

In `crates/oxitide-app/src/commands.rs`, change the import line to include the `Manager` trait:

```rust
use tauri::{Manager, State};
```

Add the DTO after `CacheStatusDto`:

```rust
#[derive(Serialize)]
pub struct TrackPageDto {
    pub items: Vec<Track>,
    pub total: u32,
    pub offset: u32,
    pub limit: u32,
}
```

Add the commands after `get_user_playlists`:

```rust
#[tauri::command]
pub async fn get_favorite_tracks_page(
    state: State<'_, AppState>,
    offset: u32,
    limit: u32,
) -> Result<TrackPageDto, String> {
    let (items, total) = require_client(&state)?
        .favorite_tracks_page(limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(TrackPageDto { items, total, offset, limit })
}

#[tauri::command]
pub async fn get_playlist_tracks_page(
    state: State<'_, AppState>,
    id: String,
    offset: u32,
    limit: u32,
) -> Result<TrackPageDto, String> {
    let (items, total) = require_client(&state)?
        .playlist_tracks_page(&id, limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(TrackPageDto { items, total, offset, limit })
}
```

Add the retry command after `queue_clear`:

```rust
#[tauri::command]
pub fn retry(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.retry().map_err(|e| e.to_string())
}
```

Add a DTO test to the test module:

```rust
    #[test]
    fn track_page_dto_serializes_fields() {
        let dto = TrackPageDto { items: Vec::new(), total: 7, offset: 100, limit: 50 };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["total"], 7);
        assert_eq!(value["offset"], 100);
        assert_eq!(value["limit"], 50);
        assert!(value["items"].is_array());
    }
```

- [ ] **Step 5: Route every Tidal command error through `fail_tidal`**

In `crates/oxitide-app/src/commands.rs`, replace `.map_err(|e| e.to_string())` with `.map_err(|e| state.fail_tidal(e))` in exactly these commands: `get_home`, `get_album`, `get_album_tracks`, `get_playlist`, `get_playlist_tracks`, `get_artist`, `search`, `get_favorite_tracks`, `get_favorite_albums`, `get_favorite_artists`, `get_user_playlists` (the two new page commands already use it). The resulting functions:

```rust
#[tauri::command]
pub async fn get_home(state: State<'_, AppState>) -> Result<Vec<HomeSection>, String> {
    require_client(&state)?.home_page().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_album(state: State<'_, AppState>, id: u64) -> Result<Album, String> {
    require_client(&state)?.album(id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_album_tracks(state: State<'_, AppState>, id: u64) -> Result<Vec<Track>, String> {
    require_client(&state)?.album_tracks_all(id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_playlist(state: State<'_, AppState>, id: String) -> Result<Playlist, String> {
    require_client(&state)?.playlist(&id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_playlist_tracks(state: State<'_, AppState>, id: String) -> Result<Vec<Track>, String> {
    require_client(&state)?.playlist_tracks_all(&id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist(state: State<'_, AppState>, id: u64) -> Result<Artist, String> {
    require_client(&state)?.artist(id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<SearchResults, String> {
    require_client(&state)?
        .search(&query, &["tracks", "albums", "artists", "playlists"], 20, 0)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    require_client(&state)?.all_favorite_tracks().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    require_client(&state)?.all_favorite_albums().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    require_client(&state)?.all_favorite_artists().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_user_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    require_client(&state)?.all_user_playlists().await.map_err(|e| state.fail_tidal(e))
}
```

- [ ] **Step 6: Shut down the old engine when replacing the player**

Add the helper above `build_player`:

```rust
/// Replace the running player: release the old engine's worker and output
/// first, and clear the expired-session flag now that a new login exists.
fn replace_player(
    app: &tauri::AppHandle,
    state: &AppState,
    client: TidalClient,
    config: &Config,
    cache: Arc<DiskCache>,
) -> Result<Player, String> {
    if let Some(old) = state.player() {
        old.engine.shutdown();
    }
    state.clear_session_expired();
    build_player(app, client, config, cache)
}
```

In `auth_complete`, replace:

```rust
    state.set_player(build_player(&app, client, &config, cache)?);
```

with:

```rust
    state.set_player(replace_player(&app, &state, client, &config, cache)?);
```

In `auth_restore`, replace:

```rust
            state.set_player(build_player(&app, client, &config, cache)?);
```

with:

```rust
            state.set_player(replace_player(&app, &state, client, &config, cache)?);
```

- [ ] **Step 7: Signal session expiry from engine auth errors**

In `build_player`, replace the subscribe block:

```rust
    let handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
```

with:

```rust
    let handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        if let EngineEvent::Error { code, .. } = &event {
            if code == "auth" {
                handle.state::<AppState>().notify_session_expired();
            }
        }
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
```

- [ ] **Step 8: Register the commands and set the state handle in `main.rs`**

In `crates/oxitide-app/src/main.rs`, add to `generate_handler![...]` (next to the other commands):

```rust
            commands::get_favorite_tracks_page,
            commands::get_playlist_tracks_page,
            commands::retry,
```

Update the `setup` hook (from Task 3) to remember the handle:

```rust
        .setup(|app| {
            app.state::<AppState>().set_handle(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                let config = app.state::<AppState>().config();
                if let Err(e) = window::apply(&window, &config) {
                    tracing::warn!("failed to apply window geometry: {e}");
                }
                if let Err(e) = window.show() {
                    tracing::warn!("failed to show the main window: {e}");
                }
            }
            Ok(())
        })
```

- [ ] **Step 9: Add the UI API surface**

In `ui/src/lib/api.ts`, add the `TrackPage` interface after `SearchResults`:

```ts
export interface TrackPage { items: Track[]; total: number; offset: number; limit: number }
```

Add to the `api` object after `getUserPlaylists`:

```ts
  getFavoriteTracksPage: (offset: number, limit: number) =>
    invoke<TrackPage>("get_favorite_tracks_page", { offset, limit }),
  getPlaylistTracksPage: (id: string, offset: number, limit: number) =>
    invoke<TrackPage>("get_playlist_tracks_page", { id, offset, limit }),
```

Add after `playPause`:

```ts
  retry: () => invoke<void>("retry"),
```

- [ ] **Step 10: Run the gates**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green.

- [ ] **Step 11: Commit**

```powershell
git add crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts
git commit -m "feat(app): recover from an expired session"
```

---

### Task 7: ErrorState and view load extraction

**Milestone:** P5-5 (views half).

**Files:**
- Create: `ui/src/lib/components/ErrorState.svelte`
- Modify: `ui/src/views/Home.svelte`
- Modify: `ui/src/views/Album.svelte`
- Modify: `ui/src/views/Artist.svelte`
- Modify: `ui/src/views/Search.svelte`
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: existing `api` fetches.
- Produces (used by Tasks 11, 12): `ErrorState` with `{ message: string, onRetry?: () => void }`.

- [ ] **Step 1: Create the component**

Create `ui/src/lib/components/ErrorState.svelte`:

```svelte
<script lang="ts">
  let { message, onRetry }: { message: string; onRetry?: () => void } = $props();
</script>

<div class="error-state">
  <p style="color:#ff6b6b">{message}</p>
  {#if onRetry}
    <button onclick={onRetry}>Retry</button>
  {/if}
</div>
```

- [ ] **Step 2: Refactor Home.svelte**

Replace the whole file with:

```svelte
<script lang="ts">
  import { api, type HomeSection } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let sections = $state<HomeSection[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      sections = await api.getHome();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

<h2>Home</h2>
{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <p class="muted">Nothing to show yet.</p>
{:else}
  {#each sections as section, si (`${section.title}-${si}`)}
    <h3>{section.title}</h3>
    <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:12px">
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        <button style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit">
          <div>{item.title}</div>
          {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
        </button>
      {/each}
    </div>
  {/each}
{/if}
```

- [ ] **Step 3: Refactor Album.svelte**

Replace the whole file with:

```svelte
<script lang="ts">
  import { api, type Album as AlbumT, type Track } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Player } from "../lib/player.svelte";
  let { id, player }: { id: number; player: Player } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      [album, tracks] = await Promise.all([api.getAlbum(id), api.getAlbumTracks(id)]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  {#if album}
    <h2>{album.title}</h2>
    <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.numberOfTracks} tracks</p>
  {/if}
  <TrackTable {tracks} {player} />
{/if}
```

- [ ] **Step 4: Refactor Artist.svelte**

Replace the whole file with:

```svelte
<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  let { id }: { id: number } = $props();
  let artist = $state<ArtistT | null>(null);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      artist = await api.getArtist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
  <h2>{artist.name}</h2>
  <p class="muted">Artist</p>
{/if}
```

- [ ] **Step 5: Refactor Search.svelte**

Replace the `<script>` block with:

```svelte
<script lang="ts">
  import { api, type SearchResults } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  let { nav, player }: { nav: Nav; player: Player } = $props();
  let query = $state("");
  let results = $state<SearchResults | null>(null);
  let loading = $state(false);
  let error = $state("");

  async function load() {
    if (query.trim().length === 0) return;
    loading = true;
    error = "";
    try {
      results = await api.search(query);
    } catch (e) {
      error = String(e);
      results = null;
    } finally {
      loading = false;
    }
  }
</script>
```

In the template, change the input handler from `run()` to `load()` and the error branch:

```svelte
<input bind:value={query} onkeydown={(e) => e.key === "Enter" && load()} placeholder="Search TIDAL" />
{#if loading}
  <p class="muted">Searching…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if results}
```

The Tracks/Albums/Artists/Playlists sections stay exactly as they are.

- [ ] **Step 6: Refactor Settings.svelte**

Add the import next to the other imports:

```ts
  import ErrorState from "../lib/components/ErrorState.svelte";
```

Replace the initial-load `$effect` (as extended in Task 2) with a `load()` function:

```ts
  async function load() {
    error = "";
    try {
      const [loaded, list, status, logs] = await Promise.all([
        api.getConfig(),
        api.getOutputDevices(),
        api.getCacheStatus(),
        api.getLogsDir(),
      ]);
      devices = list;
      cacheStatus = status;
      logsDir = logs;
      // The stored device may be gone; fall back to the system default and
      // persist that so the next start does not retry it.
      let next = loaded;
      if (next.audio.output_device && !list.some((d) => d.id === next.audio.output_device)) {
        next = { ...next, audio: { ...next.audio, output_device: "" } };
        api.setOutputDevice("").catch(apply);
      }
      config = next;
    } catch (e) {
      apply(e);
    }
  }

  $effect(() => {
    load();
  });
```

Replace the template's opening:

```svelte
<h2>Settings</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
{#if config}
```

with:

```svelte
<h2>Settings</h2>
{#if !config}
  {#if error}
    <ErrorState message={error} onRetry={load} />
  {:else}
    <p class="muted">Loading…</p>
  {/if}
{:else}
  {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
```

Everything from `<h3>Audio output</h3>` down to the existing final `{/if}` stays as it is (the final `{/if}` now closes the `{:else}` branch).

- [ ] **Step 7: Run the UI gates**

```powershell
npm --prefix ui test
npm --prefix ui run check
```

Expected: 13 existing tests still pass; svelte-check reports 0 errors. Manual spot check (repeated in Task 13): stop the network, open Home/Album/Artist/Search/Settings, confirm the error with Retry appears; restore the network and press Retry.

- [ ] **Step 8: Commit**

```powershell
git add ui/src/lib/components/ErrorState.svelte ui/src/views/Home.svelte ui/src/views/Album.svelte ui/src/views/Artist.svelte ui/src/views/Search.svelte ui/src/views/Settings.svelte
git commit -m "feat(ui): add a retryable error state to data views"
```

---

### Task 8: Player retry UI

**Milestone:** P5-5 (playback UI half).

**Files:**
- Modify: `ui/src/lib/player.svelte.ts`
- Modify: `ui/src/lib/components/PlayerBar.svelte`

**Interfaces:**
- Consumes: Task 6 (`api.retry`).
- Produces: `player.retry(): Promise<void>`.

- [ ] **Step 1: Add `retry` to the player store**

In `ui/src/lib/player.svelte.ts`, add after `seek`:

```ts
  async function retry() {
    await api.retry();
    // The engine emits a State event, which does not clear the mirrored
    // error; clear it here once the retry command was accepted.
    state = { ...state, error: "" };
  }
```

Add it to the returned object after `playPause`:

```ts
    retry,
```

- [ ] **Step 2: Add the Retry button to the player bar**

In `ui/src/lib/components/PlayerBar.svelte`, replace:

```svelte
  {#if player.state.error}<span style="color:#ff6b6b">{player.state.error}</span>{/if}
```

with:

```svelte
  {#if player.state.error}
    <span style="color:#ff6b6b">{player.state.error}</span>
    <button onclick={() => player.retry()} aria-label="Retry playback">Retry</button>
  {/if}
```

- [ ] **Step 3: Run the UI gates**

```powershell
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Manual spot check (repeated in Task 13): with a track playing, disable the network, wait for the player-bar error, re-enable the network, press Retry — playback resumes near the last position.

- [ ] **Step 4: Commit**

```powershell
git add ui/src/lib/player.svelte.ts ui/src/lib/components/PlayerBar.svelte
git commit -m "feat(ui): retry the current track from the player bar"
```

---

### Task 9: Session expiry capture and restore

**Milestone:** P5-4 (UI half).

**Files:**
- Modify: `ui/src/views/Login.svelte`
- Modify: `ui/src/lib/player.svelte.ts`
- Modify: `ui/src/App.svelte`
- Create: `ui/src/lib/player.restore.test.ts`

**Interfaces:**
- Consumes: Task 6 (`session_expired` event, rebuilt engine on `auth_complete`).
- Produces: `player.restore(queue, index, positionMs, wasPlaying): Promise<void>`; `Login` gains `message?: string`.

- [ ] **Step 1: Write the failing restore tests**

Create `ui/src/lib/player.restore.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  playTracks: vi.fn(),
  seek: vi.fn(),
  playPause: vi.fn(),
  getPlaybackState: vi.fn(),
}));

vi.mock("./api", () => ({ api: mocks }));

import { createPlayer, emptyState } from "./player.svelte";

const queue = [
  { track_id: 1, title: "A", artist: "X", album: "", duration_secs: 10, cover: null },
  { track_id: 2, title: "B", artist: "Y", album: "", duration_secs: 10, cover: null },
];

function order(...calls: { mock: { invocationCallOrder: number[] } }[]): number[] {
  return calls.map((fn) => fn.mock.invocationCallOrder[0]);
}

beforeEach(() => {
  mocks.playTracks.mockReset().mockResolvedValue(undefined);
  mocks.seek.mockReset().mockResolvedValue(undefined);
  mocks.playPause.mockReset().mockResolvedValue(undefined);
  mocks.getPlaybackState.mockReset().mockResolvedValue({
    ...emptyState(),
    playing: true,
    current: queue[1],
    queue,
  });
});

describe("player.restore", () => {
  it("resumes a playing session in order", async () => {
    const player = createPlayer();
    await player.restore(queue, 1, 1500, true);

    expect(mocks.playTracks).toHaveBeenCalledWith(queue, 1);
    expect(mocks.seek).toHaveBeenCalledWith(1500);
    expect(mocks.playPause).not.toHaveBeenCalled();
    const calls = order(mocks.playTracks, mocks.seek);
    expect(calls[0]).toBeLessThan(calls[1]);
    expect(mocks.getPlaybackState).toHaveBeenCalled();
  });

  it("re-pauses a paused session", async () => {
    const player = createPlayer();
    await player.restore(queue, 0, 900, false);

    expect(mocks.playPause).toHaveBeenCalledTimes(1);
    const calls = order(mocks.playTracks, mocks.seek, mocks.playPause);
    expect(calls).toEqual([...calls].sort((a, b) => a - b));
  });

  it("does nothing for an empty queue", async () => {
    const player = createPlayer();
    await player.restore([], 0, 0, true);

    expect(mocks.playTracks).not.toHaveBeenCalled();
    expect(mocks.seek).not.toHaveBeenCalled();
    expect(mocks.playPause).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/player.restore.test.ts`
Expected: FAIL — `player.restore is not a function`.

- [ ] **Step 3: Implement `player.restore`**

In `ui/src/lib/player.svelte.ts`, add after `retry`:

```ts
  /// Rebuild the queue after a re-login: start the context, seek to the last
  /// position, and pause again when the session was paused. `play_context`
  /// always starts playback, hence the extra `playPause` for paused sessions.
  async function restore(
    queue: QueueItem[],
    index: number,
    positionMs: number,
    wasPlaying: boolean,
  ) {
    if (queue.length === 0) return;
    await api.playTracks(queue, index);
    await seek(positionMs);
    if (!wasPlaying) await api.playPause();
    const snapshot = await api.getPlaybackState();
    state = reduce(state, { type: "state", ...snapshot });
  }
```

Add it to the returned object:

```ts
    restore,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/player.restore.test.ts`
Expected: 3 tests PASS.

- [ ] **Step 5: Give Login an optional message**

In `ui/src/views/Login.svelte`, change the props:

```ts
  let { onLogin, message = "" }: { onLogin: () => void; message?: string } = $props();
```

Change the heading:

```svelte
  <h1>{message || "Sign in to TIDAL"}</h1>
```

- [ ] **Step 6: Wire the capture/restore flow in App.svelte**

Replace the whole file with:

```svelte
<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import QueueRail from "./lib/components/QueueRail.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Album from "./views/Album.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Artist from "./views/Artist.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import Settings from "./views/Settings.svelte";
  import { createNav } from "./lib/nav.svelte";
  import { createPlayer } from "./lib/player.svelte";
  import { api, type QueueItem } from "./lib/api";
  import type { QualityDisplay } from "./lib/quality";

  interface ExpiredSession {
    queue: QueueItem[];
    index: number;
    position: number;
    playing: boolean;
  }

  const nav = createNav();
  const player = createPlayer();
  let qualityDisplay = $state<QualityDisplay>("compact");
  let loggedIn = $state(false);
  let booting = $state(true);
  let error = $state("");
  let playerReady = $state(false);
  let expired = $state<ExpiredSession | null>(null);

  $effect(() => {
    const unlisten = listen("session_expired", () => {
      if (expired) return;
      const snapshot = player.state;
      const current = snapshot.current;
      expired = {
        queue: snapshot.queue,
        index: current
          ? Math.max(0, snapshot.queue.findIndex((item) => item.track_id === current.track_id))
          : 0,
        position: snapshot.position_ms,
        playing: snapshot.playing,
      };
      loggedIn = false;
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  $effect(() => {
    if (loggedIn && !playerReady) {
      playerReady = true;
      player.init();
    }
  });

  $effect(() => {
    if (loggedIn) {
      api
        .getConfig()
        .then((config) => (qualityDisplay = config.ui.quality_display))
        .catch(() => {});
    }
  });

  $effect(() => {
    api
      .authRestore()
      .then((session) => {
        if (session) loggedIn = true;
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        booting = false;
      });
  });

  async function onLogin() {
    error = "";
    const restore = expired;
    expired = null;
    if (restore) {
      try {
        await player.restore(restore.queue, restore.index, restore.position, restore.playing);
      } catch (e) {
        error = String(e);
      }
    }
    loggedIn = true;
  }

  async function onLogout() {
    try {
      await api.authLogout();
    } catch (e) {
      error = String(e);
    }
    loggedIn = false;
  }
</script>

{#if booting}
  <p class="muted" style="padding:24px">Loading…</p>
{:else if !loggedIn}
  {#if error}<p style="color:#ff6b6b;padding:0 24px">{error}</p>{/if}
  <Login
    onLogin={onLogin}
    message={expired ? "Session expired — sign in to continue" : ""}
  />
{:else}
  <div class="app">
    <Sidebar {nav} onLogout={onLogout} />
    <main class="main">
      {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
      {#if nav.route.name === "home"}<Home {nav} />
      {:else if nav.route.name === "search"}<Search {nav} {player} />
      {:else if nav.route.name === "library"}<Library {nav} {player} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} {player} />
      {:else if nav.route.name === "playlist"}<Playlist id={nav.route.id} {player} />
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} />
      {:else if nav.route.name === "settings"}<Settings {player} onQualityDisplay={(mode) => (qualityDisplay = mode)} />{/if}
    </main>
    {#if nav.queueOpen}<aside class="queue"><QueueRail {player} /></aside>{/if}
    <PlayerBar {player} {qualityDisplay} />
  </div>
{/if}
```

- [ ] **Step 7: Run the UI gates**

```powershell
npm --prefix ui test
npm --prefix ui run check
```

Expected: 16 tests pass (13 existing + 3 restore), svelte-check 0 errors.

- [ ] **Step 8: Commit**

```powershell
git add ui/src/views/Login.svelte ui/src/lib/player.svelte.ts ui/src/App.svelte ui/src/lib/player.restore.test.ts
git commit -m "feat(ui): restore the queue after a session expiry"
```

---

### Task 10: Session-cached track pager store

**Milestone:** P5-6/P5-7 (store half).

**Files:**
- Create: `ui/src/lib/pager.svelte.ts`
- Create: `ui/src/lib/pager.test.ts`

**Interfaces:**
- Consumes: Task 6 (`TrackPage`).
- Produces (used by Tasks 11, 12):
  - `createTrackPager(key: string, load: (offset: number, limit: number) => Promise<TrackPage>, pageSize = 100, dedupe = true) -> Pager`.
  - `Pager { items: Track[], total: number, loading: boolean, error: string, done: boolean, loadMore(), ensureAll(), refresh() }`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/pager.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import type { Track, TrackPage } from "./api";
import { createTrackPager } from "./pager.svelte";

function track(id: number): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

function page(items: Track[], offset: number, limit: number, total: number): TrackPage {
  return { items, offset, limit, total };
}

describe("track pager", () => {
  it("appends pages, dedupes by id, and stops on a short page", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2), track(3)], 0, limit, 5)
        : page([track(3), track(4)], offset, limit, 5),
    );
    const pager = createTrackPager("append", load, 3);

    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3]);
    expect(pager.total).toBe(5);
    expect(pager.done).toBe(false);

    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
  });

  it("keeps legitimate duplicate tracks within one page", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(3), track(3)], offset, limit, 2),
    );
    const pager = createTrackPager("in-page-dupes", load, 2);

    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([3, 3]);
    expect(pager.done).toBe(true);
  });

  it("marks done when the offset reaches the total", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("offset-done", load, 2);

    await pager.loadMore();
    expect(pager.done).toBe(true);
    await pager.loadMore();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("loads every page with ensureAll", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page(offset < 4 ? [track(offset + 1), track(offset + 2)] : [], offset, limit, 4),
    );
    const pager = createTrackPager("ensure-all", load, 2);

    await pager.ensureAll();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
    expect(load.mock.calls.map(([offset]) => offset)).toEqual([0, 2]);
  });

  it("hydrates a new pager from the session cache and continues at the next offset", async () => {
    const loadA = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 4),
    );
    const a = createTrackPager("hydrate", loadA, 2);
    await a.loadMore();

    const loadB = vi.fn(async (offset: number, limit: number) =>
      page([track(3), track(4)], offset, limit, 4),
    );
    const b = createTrackPager("hydrate", loadB, 2);
    expect(b.items.map((t) => t.id)).toEqual([1, 2]);

    await b.loadMore();
    expect(loadB).toHaveBeenCalledWith(2, 2);
    expect(b.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(b.done).toBe(true);
  });

  it("refresh clears the cache and reloads from offset 0", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page(offset === 0 ? [track(1)] : [track(2)], offset, limit, 2),
    );
    const pager = createTrackPager("refresh", load, 1);
    await pager.loadMore();
    await pager.loadMore();
    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);

    load.mockClear();
    await pager.refresh();

    expect(load).toHaveBeenCalledTimes(1);
    expect(load).toHaveBeenCalledWith(0, 1);
    expect(pager.items.map((t) => t.id)).toEqual([1]);
    expect(pager.done).toBe(false);
  });

  it("keeps loaded items on error and retries the same offset", async () => {
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 2) throw new Error("boom");
      return page([track(calls)], offset, limit, 3);
    });
    const pager = createTrackPager("error-retry", load, 1);

    await pager.loadMore();
    await pager.loadMore();
    expect(pager.error).toContain("boom");
    expect(pager.items.map((t) => t.id)).toEqual([1]);

    await pager.loadMore();
    expect(pager.error).toBe("");
    expect(load).toHaveBeenLastCalledWith(1, 1);
    expect(pager.items.map((t) => t.id)).toEqual([1, 3]);
  });

  /// Review Focus: an error inside ensureAll must stop the loop, not spin.
  it("stops ensureAll on the first error instead of looping", async () => {
    const load = vi.fn(async () => {
      throw new Error("offline");
    });
    const pager = createTrackPager("error-stop", load, 2);

    await pager.ensureAll();

    expect(load).toHaveBeenCalledTimes(1);
    expect(pager.error).toContain("offline");
  });

  /// Review Focus: a full page of already-loaded tracks must still advance the
  /// offset, or ensureAll never terminates.
  it("advances the offset when a full page repeats already-loaded tracks", async () => {
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset < 4) return page([track(1), track(2)], offset, limit, 10);
      return page([], offset, limit, 10);
    });
    const pager = createTrackPager("duplicate-page", load, 2);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);
    expect(pager.done).toBe(true);
    expect(load).toHaveBeenCalledTimes(3);
  });

  /// Review Focus: `ensureAll` must await an in-flight page, not spin on the
  /// `loading` short-circuit and starve the event loop.
  it("ensureAll waits for an in-flight page instead of spinning", async () => {
    let resolveFirst: (page: TrackPage) => void = () => {};
    const first = new Promise<TrackPage>((resolve) => {
      resolveFirst = resolve;
    });
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) return first;
      return page(offset < 4 ? [track(offset + 1), track(offset + 2)] : [], offset, limit, 4);
    });
    const pager = createTrackPager("inflight", load, 2);

    const pending = pager.loadMore();
    const all = pager.ensureAll();
    // Resolve on a macrotask: a microtask-spinning `ensureAll` starves this
    // timer and the test times out, which is exactly the freeze being pinned.
    setTimeout(() => resolveFirst(page([track(1), track(2)], 0, 2, 4)), 0);

    await pending;
    await all;
    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4]);
    expect(pager.done).toBe(true);
  });

  /// Review Focus: `refresh` must discard an in-flight page rather than let it
  /// republish into the cache the refresh cleared.
  it("refresh discards an in-flight page instead of mixing it in", async () => {
    let resolveFirst: (page: TrackPage) => void = () => {};
    const first = new Promise<TrackPage>((resolve) => {
      resolveFirst = resolve;
    });
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) return first;
      return page([track(9)], offset, limit, 1);
    });
    const pager = createTrackPager("refresh-inflight", load, 1);

    const pending = pager.loadMore();
    const refreshing = pager.refresh();
    resolveFirst(page([track(1)], 0, 1, 1));

    await pending;
    await refreshing;
    expect(pager.items.map((t) => t.id)).toEqual([9]);
    expect(pager.done).toBe(true);
  });

  /// Smoke finding (2026-09-24): TIDAL returns short pages mid-list, so a
  /// short page must not end the walk while the reported total says more
  /// items remain (mirrors `TidalClient::get_all_pages`).
  it("continues past a short page while the total says more remain", async () => {
    const load = vi.fn(async (offset: number, limit: number) => {
      if (offset === 0) return page([track(1), track(2)], offset, limit, 10);
      if (offset === 2) return page([track(3), track(4), track(5)], offset, limit, 10);
      if (offset === 5) return page([track(6), track(7), track(8)], offset, limit, 10);
      return page([track(9), track(10)], offset, limit, 10);
    });
    const pager = createTrackPager("short-page", load, 3);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(pager.done).toBe(true);
  });

  it("stops on an empty page when the total overstates the list", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0 ? page([track(1), track(2)], offset, limit, 10) : page([], offset, limit, 10),
    );
    const pager = createTrackPager("stale-total", load, 2);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2]);
    expect(pager.done).toBe(true);
    expect(load).toHaveBeenCalledTimes(2);
  });

  /// Smoke finding (2026-09-24): playlists may legitimately repeat a track;
  /// with dedupe off the queue must match the playlist exactly.
  it("keeps duplicate tracks across pages when dedupe is disabled", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      offset === 0
        ? page([track(1), track(2)], offset, limit, 4)
        : page([track(1), track(3)], offset, limit, 4),
    );
    const pager = createTrackPager("no-dedupe", load, 2, false);

    await pager.ensureAll();

    expect(pager.items.map((t) => t.id)).toEqual([1, 2, 1, 3]);
    expect(pager.done).toBe(true);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: FAIL — cannot resolve `./pager.svelte`.

- [ ] **Step 3: Implement the pager store**

Create `ui/src/lib/pager.svelte.ts`:

```ts
import type { Track, TrackPage } from "./api";

export interface Pager {
  readonly items: Track[];
  readonly total: number;
  readonly loading: boolean;
  readonly error: string;
  readonly done: boolean;
  loadMore(): Promise<void>;
  ensureAll(): Promise<void>;
  refresh(): Promise<void>;
}

interface CacheEntry {
  items: Track[];
  total: number;
  nextOffset: number;
  done: boolean;
}

/// Session cache: revisiting a view within one app run hydrates from here
/// instead of refetching. `refresh()` clears the key's entry.
const cache = new Map<string, CacheEntry>();

/// A scroll pager for a track list. `key` is `"favorites"` or
/// `"playlist:{uuid}"`; `load` fetches one page at `offset`. `dedupe` drops
/// ids already loaded (favorites can't repeat); playlists pass `false` so a
/// track that appears twice in the playlist stays twice in the queue.
export function createTrackPager(
  key: string,
  load: (offset: number, limit: number) => Promise<TrackPage>,
  pageSize = 100,
  dedupe = true,
): Pager {
  const cached = cache.get(key);
  let items = $state<Track[]>(cached ? [...cached.items] : []);
  let total = $state(cached?.total ?? 0);
  let done = $state(cached?.done ?? false);
  let loading = $state(false);
  let error = $state("");
  let nextOffset = cached?.nextOffset ?? 0;
  // The in-flight page request, shared by concurrent `loadMore` callers so
  // `ensureAll` awaits real progress instead of spinning on a resolved promise.
  let inflight: Promise<void> | null = null;

  function publish() {
    cache.set(key, { items: [...items], total, nextOffset, done });
  }

  function loadMore(): Promise<void> {
    if (inflight) return inflight;
    if (done) return Promise.resolve();
    inflight = (async () => {
      loading = true;
      error = "";
      const offset = nextOffset;
      try {
        const page = await load(offset, pageSize);
        const received = page.items.length;
        if (dedupe) {
          const seen = new Set(items.map((t) => t.id));
          // Dedupe only against already-loaded pages (offset overlap);
          // legitimate duplicate tracks within one page must survive.
          const fresh = page.items.filter((t) => !seen.has(t.id));
          items = [...items, ...fresh];
        } else {
          items = [...items, ...page.items];
        }
        const reportedTotal = page.total;
        // Advance by the raw page length so a page of duplicates cannot stall.
        nextOffset = offset + received;
        // Trust the reported total: TIDAL returns short pages mid-list, so a
        // short page only ends the walk when no total is reported (mirrors
        // `TidalClient::get_all_pages`); an empty page always ends it.
        done =
          received === 0 ||
          (reportedTotal > 0 ? nextOffset >= reportedTotal : received < pageSize);
        total = reportedTotal;
        publish();
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
        inflight = null;
      }
    })();
    return inflight;
  }

  async function ensureAll() {
    while (!done && error === "") {
      await loadMore();
    }
  }

  async function refresh() {
    // Let an in-flight page finish first so it cannot republish into the
    // cache after the reset.
    if (inflight) await inflight;
    cache.delete(key);
    items = [];
    total = 0;
    nextOffset = 0;
    done = false;
    error = "";
    await loadMore();
  }

  return {
    get items() { return items; },
    get total() { return total; },
    get loading() { return loading; },
    get error() { return error; },
    get done() { return done; },
    loadMore,
    ensureAll,
    refresh,
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: all 15 pager tests PASS.

- [ ] **Step 5: Run the UI gates and commit**

```powershell
npm --prefix ui test
npm --prefix ui run check
git add ui/src/lib/pager.svelte.ts ui/src/lib/pager.test.ts
git commit -m "feat(ui): add a session-cached track pager"
```

---

### Task 11: Page favorite tracks in the Library

**Milestone:** P5-6.

**Files:**
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/views/Library.svelte`

**Interfaces:**
- Consumes: Tasks 6, 7, 10 (`api.getFavoriteTracksPage`, `ErrorState`, `createTrackPager`).
- Produces: `TrackTable` optional `onPlay(index)` / `onPlayAll()` overrides with a "Loading all…" busy state.

- [ ] **Step 1: Add the TrackTable overrides**

Replace `ui/src/lib/components/TrackTable.svelte` with:

```svelte
<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { formatDuration } from "../format";
  import type { Player } from "../player.svelte";
  let {
    tracks,
    player,
    onPlay,
    onPlayAll,
  }: {
    tracks: Track[];
    player: Player;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
  } = $props();

  let loadingAll = $state(false);

  async function play(index: number) {
    if (onPlay) {
      loadingAll = true;
      try {
        await onPlay(index);
      } finally {
        loadingAll = false;
      }
      return;
    }
    api.playTracks(tracks.map(trackToQueueItem), index);
  }

  async function playAll() {
    if (onPlayAll) {
      loadingAll = true;
      try {
        await onPlayAll();
      } finally {
        loadingAll = false;
      }
      return;
    }
    play(0);
  }
</script>

<table style="width:100%; border-collapse:collapse">
  <thead><tr><th style="text-align:left"><button onclick={playAll} disabled={loadingAll}>{loadingAll ? "Loading all…" : "Play all"}</button></th><th style="text-align:left">#</th><th style="text-align:left">Title</th><th style="text-align:right">Time</th></tr></thead>
  <tbody>
    {#each tracks as t, i (`${t.id}-${i}`)}
      <tr style="border-bottom:1px solid var(--line)">
        <td>
          <button onclick={() => play(i)} disabled={loadingAll} aria-label="Play">▶</button>
          <button
            onclick={() => api.queueAdd([trackToQueueItem(t)])}
            aria-label="Add to queue"
            title="Add to queue"
          >＋</button>
        </td>
        <td class="muted">{i + 1}</td>
        <td>{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></td>
        <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
      </tr>
    {/each}
    {#if tracks.length === 0}
      <tr><td colspan="4" class="muted">No tracks</td></tr>
    {/if}
  </tbody>
</table>
```

Add the `formatDuration` import at the top (it was missing from the original snippet):

```ts
  import { formatDuration } from "../format";
```

- [ ] **Step 2: Page the Library**

Replace `ui/src/views/Library.svelte` with:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Album, type Playlist } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  let { nav, player }: { nav: Nav; player: Player } = $props();

  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);
  let error = $state("");
  let sentinel = $state<HTMLElement | null>(null);

  const favorites = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  async function load() {
    loading = true;
    error = "";
    try {
      [albums, playlists] = await Promise.all([api.getFavoriteAlbums(), api.getUserPlaylists()]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  // Eager first page: the sentinel alone would leave the section empty when it
  // starts below the fold. `untrack` keeps this a one-shot mount action.
  $effect(() => {
    untrack(() => favorites.loadMore());
  });

  $effect(() => {
    if (!sentinel || favorites.done) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) favorites.loadMore();
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  async function playFrom(index: number) {
    await favorites.ensureAll();
    if (favorites.error) return;
    api.playTracks(favorites.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await favorites.ensureAll();
    if (favorites.error) return;
    api.playTracks(favorites.items.map(trackToQueueItem), 0);
  }
</script>

<h2>Library</h2>
{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  <h3>Playlists</h3>
  <ul>
    {#each playlists as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
    {#if playlists.length === 0}<li class="muted">No playlists</li>{/if}
  </ul>
  <h3>Albums</h3>
  <ul>
    {#each albums as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
    {#if albums.length === 0}<li class="muted">No albums</li>{/if}
  </ul>
  <h3>Favorite tracks</h3>
  <p>
    <button onclick={() => favorites.refresh()}>Refresh</button>
    <span class="muted">{favorites.items.length} of {favorites.total}</span>
  </p>
  <TrackTable tracks={favorites.items} {player} onPlay={playFrom} onPlayAll={playAll} />
  {#if favorites.error}
    <ErrorState message={favorites.error} onRetry={() => favorites.loadMore()} />
  {/if}
  {#if favorites.loading}<p class="muted">Loading…</p>{/if}
  {#if !favorites.done && !favorites.error}
    <div bind:this={sentinel} style="height:1px"></div>
  {/if}
{/if}
```

- [ ] **Step 3: Run the UI gates**

```powershell
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Manual spot check (repeated in Task 13): open Library with a large favorites list — the first page paints immediately, scrolling appends, Play all shows "Loading all…" and queues the full list, and leaving and returning to Library does not refetch (session cache).

- [ ] **Step 4: Commit**

```powershell
git add ui/src/lib/components/TrackTable.svelte ui/src/views/Library.svelte
git commit -m "feat(ui): page favorite tracks in the library"
```

---

### Task 12: Page playlist tracks

**Milestone:** P5-7.

**Files:**
- Modify: `ui/src/views/Playlist.svelte`

**Interfaces:**
- Consumes: Tasks 6, 7, 10, 11 (`api.getPlaylistTracksPage`, `ErrorState`, `createTrackPager`, `TrackTable` overrides).
- Produces: the paged Playlist view.

- [ ] **Step 1: Replace Playlist.svelte**

Replace `ui/src/views/Playlist.svelte` with:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Player } from "../lib/player.svelte";
  let { id, player }: { id: string; player: Player } = $props();

  let playlist = $state<PlaylistT | null>(null);
  let loading = $state(true);
  let error = $state("");
  let sentinel = $state<HTMLElement | null>(null);

  // `$derived` so navigating to another playlist rebuilds the pager; the new
  // key hydrates from the session cache when that playlist was seen before.
  // `dedupe = false`: a playlist may legitimately repeat a track, and the
  // queue must match the playlist exactly.
  const pager = $derived(
    createTrackPager(
      `playlist:${id}`,
      (offset, limit) => api.getPlaylistTracksPage(id, offset, limit),
      100,
      false,
    ),
  );

  async function load() {
    loading = true;
    error = "";
    try {
      playlist = await api.getPlaylist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  // Eager first page: the sentinel alone would leave the list empty when it
  // starts below the fold. `untrack` keeps this a one-shot mount action.
  $effect(() => {
    untrack(() => pager.loadMore());
  });

  $effect(() => {
    if (!sentinel || pager.done) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) pager.loadMore();
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  async function playFrom(index: number) {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), 0);
  }
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  {#if playlist}
    <h2>{playlist.title}</h2>
    <p class="muted">{playlist.numberOfTracks} tracks</p>
  {/if}
  <p>
    <button onclick={() => pager.refresh()}>Refresh</button>
    <span class="muted">{pager.items.length} of {pager.total}</span>
  </p>
  <TrackTable tracks={pager.items} {player} onPlay={playFrom} onPlayAll={playAll} />
  {#if pager.error}
    <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
  {/if}
  {#if pager.loading}<p class="muted">Loading…</p>{/if}
  {#if !pager.done && !pager.error}
    <div bind:this={sentinel} style="height:1px"></div>
  {/if}
{/if}
```

The `Track` type import is not needed in this file; the import list above already contains only what the code uses.

- [ ] **Step 2: Run the UI gates**

```powershell
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Manual spot check (repeated in Task 13): open a large playlist, scroll to append, Play all queues the whole list, navigating between two playlists loads the right tracks, and revisiting a playlist within the session does not refetch.

- [ ] **Step 3: Commit**

```powershell
git add ui/src/views/Playlist.svelte
git commit -m "feat(ui): page playlist tracks"
```

---

### Task 13: Verification

**Milestone:** P5-8.

**Files:**
- Create: `docs/superpowers/plans/notes/plan5-smoke.md`
- Modify: `docs/superpowers/plans/notes/plan5-release.md` (fill in results)
- Modify (only if a smoke run surfaces a real bug): whatever file the bug lives in

**Interfaces:**
- Consumes: the shipped Plan 5 build.
- Produces: recorded automated results, a human app checklist, and the filled-in release checklist.

- [ ] **Step 1: Run the automated gates and record exact results**

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Note the counts (Rust tests per crate, UI tests per file). If anything fails, fix it first via `superpowers:systematic-debugging` (failing test first) and commit the fix separately.

- [ ] **Step 2: Build the final installers**

```powershell
npm --prefix ui run app:build
Test-Path "target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe"
Test-Path "target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi"
```

Expected: exit 0 and `True` for both paths.

- [ ] **Step 3: Run the Plan 5 human checklist**

Start the app with a real account and a real audio device:

```powershell
npm --prefix ui run app:dev
```

Have the human run every item below and report pass/fail:

```
- [ ] Install → launch → upgrade-in-place → uninstall (see plan5-release.md).
- [ ] Session expiry: while a track plays, revoke the session (TIDAL account → "Log out of all devices"; if unavailable, delete the keyring entry and wait for the access token to expire), then click Library. Expect "Session expired — sign in to continue"; after login the queue and position resume (paused if it was paused).
- [ ] Network off while browsing: the view shows the error with Retry; restore the network and press Retry — the view loads.
- [ ] Network off mid-stream: the player bar shows the error with Retry; restore the network and press Retry — playback restarts near its last position.
- [ ] Window geometry survives a restart; a maximized window reopens maximized.
- [ ] A saved position on a disconnected monitor is ignored: the window opens at the default position with its saved size (edit config.toml to an off-screen position to simulate).
- [ ] %APPDATA%\OxiTide\logs\oxitide.<date>.log exists and grows; Settings → Logs shows the directory and Open logs folder opens Explorer there.
- [ ] Library with a large favorites list paints after the first page, appends while scrolling, and "Play all" shows "Loading all…" then queues the full list.
- [ ] Playlist paging behaves the same; revisiting Library/Playlist within a session does not refetch (session cache).
```

- [ ] **Step 4: Record the results**

Create `docs/superpowers/plans/notes/plan5-smoke.md`:

```markdown
# Plan 5 Smoke Checklist

> Requires a human, a logged-in TIDAL account, an audio device, and the built
> installer. Run with `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at <commit>)
- cargo test --workspace: <counts>
- cargo clippy --workspace -- -D warnings: <clean/warnings>
- npm --prefix ui test: <counts>
- npm --prefix ui run check: <errors/warnings>
- npm --prefix ui run app:build: <artifact paths>

## Manual checks
<the checklist with [x]/[ ] and one note per failure>
```

Fill in the install/upgrade/uninstall results in `docs/superpowers/plans/notes/plan5-release.md` (the `[ ]` boxes and the Records section) using the artifacts built in Step 2.

- [ ] **Step 5: Fix any real bugs**

For each failure, use `superpowers:systematic-debugging`: capture the symptom, write a failing test (a Rust unit test where possible, otherwise a documented manual repro), fix, re-run the gate(s), and commit each fix as its own `fix(...)` commit.

- [ ] **Step 6: Commit**

```powershell
git add docs/superpowers/plans/notes/plan5-smoke.md docs/superpowers/plans/notes/plan5-release.md
git commit -m "test: record plan 5 verification results"
```

If Step 5 produced fixes, commit those first (one commit per fix).

---

## Self-Review

**Spec coverage:**

| Spec section | Task(s) |
|---|---|
| §5.1 packaging (version 0.2.0, bundle metadata, NSIS currentUser, WebView2 bootstrapper, visible:false, release checklist, README/AGENTS) | 1, 3 |
| §5.2 auth expiry detection (`session_expired`, `fail_tidal`, engine `code: "auth"`, engine replacement) | 5, 6 |
| §5.2 UI capture/restore (`Login` message, `player.restore`, App listener) | 9 |
| §5.3 ErrorState + view `load()` extraction | 7 |
| §5.3 playback retry (`EngineCmd::Retry`, `Engine::retry`, `retry` command, player bar button) | 5, 6, 8 |
| §5.4 file logging (`logs_dir`, daily rotation, 5 files, `RUST_LOG`, logs commands, Settings row) | 2 |
| §5.5 window state (config fields, `window.rs`, apply/show, capture, clamp) | 3 |
| §5.6 paged favorite tracks (page endpoints, `TrackPageDto`, commands, pager store, Library sentinel/Refresh/ensure-all) | 4, 6, 10, 11 |
| §5.6/§5.7 paged playlists | 4, 6, 10, 12 |
| §6 config keys | 2 (`logs_dir`), 3 (`[ui]` window fields) |
| §7 IPC surface (5 new commands + `session_expired` event + `code: "auth"`) | 2, 5, 6 |
| §8 UI changes | 7, 8, 9, 11, 12 |
| §9 error handling table | 2, 3, 5, 6, 7, 9, 10 |
| §10 testing strategy | every task |
| §11 milestones P5-1…P5-8 | 1, 2, 3, 5+6+9, 7+8, 4+10+11, 4+10+12, 13 |

**Deferred scope untouched:** code signing, auto-updater, release hosting/CI, keyboard shortcuts/media keys, list virtualization, the Plan 4 review's deferred minor findings, and Phase 2/3/4 features.

**Placeholder scan:** every step carries exact commands and full code; no TBD/TODO. The only human-dependent values are test counts and smoke results, which Task 13 records.

**Type consistency:** `TrackPageDto`/`TrackPage` (Task 6) match the pager's `load` signature (Task 10) and the `api.getFavoriteTracksPage`/`getPlaylistTracksPage` calls (Tasks 11–12); `ErrorState`'s `{ message, onRetry }` (Task 7) is used unchanged in Tasks 11–12; `player.restore(queue, index, positionMs, wasPlaying)` (Task 9) matches the App.svelte call; `window::visible_position(x, y, width, height, monitors)` (Task 3) matches its tests; `AppState::fail_tidal` (Task 6) is applied to the same command list it enumerates.

**Review Focus → owning tests:** all five items are pinned (Tasks 9, 9, 10, 10, 3).

