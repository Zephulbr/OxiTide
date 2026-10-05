# Track 10.6 Tray and Global Hotkeys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make OxiTide behave like a resident desktop app: a system tray with show/hide + transport + quit, close-to-tray (setting-controlled), a working single-instance re-show, and user-defined global hotkeys registered through `tauri-plugin-global-shortcut` with conflict-safe settings capture.

**Architecture:** A new `tray.rs` owns the tray icon, menu, tooltip, and window toggle; a `tray` handle lives on `AppState` and the existing engine subscriber in `commands.rs::build_player` pushes tooltip updates. A new `hotkeys.rs` owns pure binding math (parse, action mapping, in-app skip list, volume step) plus registration; the plugin's global handler reads the live config and dispatches to the engine or tray. Config gains `ui.close_to_tray` (default true) and a `[hotkeys]` section (empty defaults). `set_close_to_tray` and `set_hotkey` commands persist config; `set_hotkey` re-registers and rolls back to the previous binding when the new one fails. The UI adds `hotkeys.ts` (accelerator capture/display helpers) and a Settings → System "Desktop / Global hotkeys" surface.

**Tech Stack:** Rust (Tauri 2.12 with the `tray-icon` feature; one new crate: `tauri-plugin-global-shortcut = "2"`, as spec §6.6 directs), Svelte 5 runes, TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-04-oxitide-track10-qol-design.md` (§5.1, §5.3, §6.6, §7, §8; decisions QoL-D3, QoL-D10; platform caveats §6.6 last bullet)

**Spec refinements (flagged for review):**

1. **No `love` hotkey action.** Spec §6.6 lists `play_pause`, `next`, `prev`, `volume_up`, `volume_down`, `show_hide`, `love`; 10.4 (scrobble love) is deferred 2026-10-05, so the action whitelist ships the six non-love actions and rejects `love`. Reviving 10.4 adds the seventh action then.
2. **`[hotkeys]` is a struct table** with named keys (`play_pause`, `next`, `prev`, `volume_up`, `volume_down`, `show_hide`) instead of a map — serde defaults, rollback, and TS typing stay simple, and the TOML shape is still action → accelerator.
3. **Tooltip updates ride the existing engine subscriber** (`build_player`) on `NowPlaying`/`State` events; no new event, no polling.
4. **Tray Quit saves window geometry first** (`save_window_state` + `app.exit(0)`), because with close-to-tray the normal `CloseRequested` path may not run on a tray-only exit.
5. **Left-click toggles the window**, the menu opens on right-click (`show_menu_on_left_click(false)`); Linux ignores that flag (menu on left click) — documented caveat, Track 8 exemption pattern.
6. **Volume hotkeys persist immediately** in 5% steps (matching the in-app arrow keys); config writes are atomic and a press is a discrete event.

## Global Constraints

- Windows-first; no CI. Every check is manual, run by the implementer.
- Gates after every Rust task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`. Gates after every UI task: `npm --prefix ui run check`, `npm --prefix ui test`.
- The only new dependency is `tauri-plugin-global-shortcut = "2"` plus tauri's existing `tray-icon` feature (spec §6.6). No other crates, no npm packages.
- Defaults are empty hotkeys and `close_to_tray = true` (QoL-D3, QoL-D10). Nothing autostarts, nothing registers a default global shortcut.
- Tray or hotkey registration failures surface a `tracing::warn!` and never block startup or window creation (spec §6.6 "Failures surface a notice and never block the app").
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- `set_hotkey` runs off the main thread (`#[tauri::command(async)]`): the plugin's register/unregister dispatch to the main thread and block, so a sync command would risk stalling it.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`accelerator` is one word, `enabled` unchanged). Keep `ui/src/lib/api.ts` in sync by hand.
- Svelte 5 runes only; components consume semantic tokens only; icons are inlined Lucide paths in `Icon.svelte`.
- Never log token values; hotkey accelerators are log-safe but config dumps are not (the existing redacting `Debug` rules stay).
- Do not edit `AGENTS.md` in this plan — the track completion note owns AGENTS.md updates (spec §10).
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

- **A user-bound global hotkey must not double-fire while the OxiTide window is focused.** Pinned by `focused_skip_covers_the_dom_table_but_not_custom_chords` (Task 3) and smoke step 7.
- **A conflicting or unparseable accelerator keeps the previous binding, surfaces an error, and never blocks startup.** Pinned by `parse_accepts_documented_forms_and_rejects_junk`, `set_writes_known_actions_and_rejects_unknown` (Task 3) and smoke steps 8–9.
- **Close with the setting on must hide without exiting and keep playback/session saving alive; tray Quit must run the full exit path (session flush, scheme restore).** Pinned by smoke steps 3–4 (no unit coverage for OS windows).
- **A second launch while the window is hidden must reveal, unminimize, and focus it.** Pinned by smoke step 5.
- **An empty or absent `[hotkeys]` table registers nothing and starts cleanly.** Pinned by `bindings_skip_empty_entries_and_keep_action_order` (Task 3) and `hotkeys_default_to_empty_and_round_trip` (Task 1).
- **A long track title must not overflow the tray tooltip.** Pinned by `tooltip_shortens_long_titles` (Task 2).

---

## File Structure

**Rust — `crates/oxitide-app`**

- Create `crates/oxitide-app/src/tray.rs` — `TrayAction`, `menu_action`, `tooltip`, `build`, `run`, `update_tooltip`, tests.
- Create `crates/oxitide-app/src/hotkeys.rs` — `HotkeyAction`, binding math, plugin handler, `register_all`, volume step, tests.
- Modify `crates/oxitide-app/src/config.rs` — `UiConfig.close_to_tray`, `HotkeysConfig`, `Config.hotkeys`, tests.
- Modify `crates/oxitide-app/src/state.rs` — `tray` field + accessors.
- Modify `crates/oxitide-app/src/commands.rs` — tooltip hook in `build_player`, `set_close_to_tray`, `set_hotkey`.
- Modify `crates/oxitide-app/src/main.rs` — modules, tray build, hotkey plugin + registration, close-to-tray, single-instance fix, `generate_handler!`.
- Modify `crates/oxitide-app/Cargo.toml` — tauri `tray-icon` feature, `tauri-plugin-global-shortcut`.

**UI**

- Create `ui/src/lib/hotkeys.ts` + `ui/src/lib/hotkeys.test.ts` — action table, accelerator capture and display.
- Modify `ui/src/lib/api.ts` — Config keys + two wrappers.
- Modify `ui/src/views/Settings.svelte` — System tab Desktop / Global hotkeys surface.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-05-track10-6-tray-hotkeys-smoke.md` — human GUI checklist.

---

### Task 1: Config keys — `close_to_tray` + `[hotkeys]`

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (Config `:9-20`, UiConfig `:120-142`, `Default` `:144-164`, struct literal `:354-388`, partial defaults `:408-438`)

**Interfaces:**
- Produces: `Config.hotkeys: HotkeysConfig`; `UiConfig.close_to_tray: bool` (default true); `HotkeysConfig { play_pause, next, prev, volume_up, volume_down, show_hide: String }` (all empty) — consumed by Tasks 2–3 and the UI.

- [ ] **Step 1: Write the failing tests**

Add to the config test module (after `content_round_trips_as_documented_toml`):

```rust
    #[test]
    fn close_to_tray_defaults_true_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(Config::default().ui.close_to_tray);
        let mut config = Config::default();
        config.ui.close_to_tray = false;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("close_to_tray = false"));
        assert!(!Config::load_from(&path).ui.close_to_tray);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn hotkeys_default_to_empty_and_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let fresh = Config::default();
        assert_eq!(fresh.hotkeys.play_pause, "");
        assert_eq!(fresh.hotkeys.show_hide, "");
        let mut config = Config::default();
        config.hotkeys.play_pause = "Ctrl+Alt+Space".into();
        config.hotkeys.volume_up = "VolumeUp".into();
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[hotkeys]"), "{text}");
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }
```

Extend `partial_file_fills_defaults` (next to the content assertions) with:

```rust
        assert!(config.ui.close_to_tray);
        assert_eq!(config.hotkeys.play_pause, "");
        assert_eq!(config.hotkeys.next, "");
        assert_eq!(config.hotkeys.prev, "");
        assert_eq!(config.hotkeys.volume_up, "");
        assert_eq!(config.hotkeys.volume_down, "");
        assert_eq!(config.hotkeys.show_hide, "");
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app hotkeys_default_to_empty`
Expected: FAIL to compile ("no field `hotkeys`" / "no field `close_to_tray`").

- [ ] **Step 3: Implement the config keys**

Add the field to `Config` (`config.rs:9-20`):

```rust
    pub history: HistoryConfig,
    pub content: ContentConfig,
    pub hotkeys: HotkeysConfig,
```

Add after `ContentConfig`:

```rust
/// Global hotkeys (spec §6.6): one accelerator per action, empty means
/// unbound. Values are parsed by `tauri-plugin-global-shortcut`; the UI keeps
/// them in the plugin's format (`"Ctrl+Alt+KeyP"`, `"MediaPlayPause"`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeysConfig {
    pub play_pause: String,
    pub next: String,
    pub prev: String,
    pub volume_up: String,
    pub volume_down: String,
    pub show_hide: String,
}
```

Add to `UiConfig` (after `oneko`):

```rust
    /// Close hides the window to the tray instead of exiting (QoL-D3).
    pub close_to_tray: bool,
```

and to `impl Default for UiConfig` (after `oneko: false`):

```rust
            close_to_tray: true,
```

Update the exhaustive `UiConfig` literal in `save_and_load_round_trip` (`:362-378`) with `close_to_tray: true,` and the `Config` literal (`:354-388`) with `hotkeys: HotkeysConfig::default(),`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app config`
Expected: PASS (all existing + 2 new).

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/config.rs
git commit -m "feat(app): add close-to-tray and hotkey config keys"
```

---

### Task 2: Tray icon, menu, tooltip, close-to-tray, single-instance fix

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml:13` (tauri features)
- Create: `crates/oxitide-app/src/tray.rs`
- Modify: `crates/oxitide-app/src/state.rs:49-50` (field), `:166-172` (accessors)
- Modify: `crates/oxitide-app/src/commands.rs:437-465` (tooltip hook in `build_player`)
- Modify: `crates/oxitide-app/src/main.rs:17-19` (module), `:82` (`pub(crate)`), `:211-215` (single instance), `:219-296` (setup tray), `:423-430` (close-to-tray)

**Interfaces:**
- Consumes: `Config.ui.close_to_tray` (Task 1), existing `state.player()`, `state.config()`.
- Produces: `TrayAction { ShowHide, PlayPause, Next, Prev, Quit }`; `menu_action(id) -> Option<TrayAction>`; `tooltip(Option<&QueueItem>) -> String`; `build(&AppHandle) -> tauri::Result<TrayIcon<R>>`; `run(&AppHandle, TrayAction)`; `update_tooltip(&AppHandle, Option<&QueueItem>)`; `AppState::set_tray/tray` — Tasks 3 (`hotkeys` show/hide) and the smoke checklist consume these.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/tray.rs` with only the test module (implementation in Step 3; until then the test compile fails — that is the expected failure):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, artist: &str) -> QueueItem {
        QueueItem {
            track_id: 1,
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            duration_secs: 200,
            cover: None,
        }
    }

    #[test]
    fn menu_action_maps_every_menu_id_and_ignores_others() {
        assert_eq!(menu_action("show_hide"), Some(TrayAction::ShowHide));
        assert_eq!(menu_action("play_pause"), Some(TrayAction::PlayPause));
        assert_eq!(menu_action("next"), Some(TrayAction::Next));
        assert_eq!(menu_action("prev"), Some(TrayAction::Prev));
        assert_eq!(menu_action("quit"), Some(TrayAction::Quit));
        assert_eq!(menu_action("some_other_menu"), None);
    }

    #[test]
    fn tooltip_falls_back_to_the_app_name() {
        assert_eq!(tooltip(None), "OxiTide");
        assert_eq!(tooltip(Some(&item("Song", "Artist"))), "Song - Artist");
    }

    /// Review Focus: a long title must not overflow the OS tooltip.
    #[test]
    fn tooltip_shortens_long_titles() {
        let long = "x".repeat(200);
        let text = tooltip(Some(&item(&long, "Artist")));
        assert!(text.chars().count() <= 120, "{text}");
        assert!(text.ends_with('…'), "{text}");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app tray`
Expected: FAIL to compile ("cannot find type `QueueItem`" / "cannot find function `menu_action`" etc.).

- [ ] **Step 3: Implement the tray module and wiring**

Enable the feature in `crates/oxitide-app/Cargo.toml`:

```toml
tauri = { version = "2", features = ["tray-icon"] }
```

Prepend the implementation to `crates/oxitide-app/src/tray.rs` (before the test module):

```rust
//! System tray (spec §6.6): show/hide, transport, quit, and a tooltip that
//! mirrors the current track. The tray is created once at startup and
//! survives player rebuilds; the engine subscriber pushes tooltip updates.

use oxitide_audio::engine::EngineCmd;
use oxitide_audio::queue::QueueItem;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowHide,
    PlayPause,
    Next,
    Prev,
    Quit,
}

/// Map a menu id to its action; ids from other menus are ignored.
pub fn menu_action(id: &str) -> Option<TrayAction> {
    match id {
        "show_hide" => Some(TrayAction::ShowHide),
        "play_pause" => Some(TrayAction::PlayPause),
        "next" => Some(TrayAction::Next),
        "prev" => Some(TrayAction::Prev),
        "quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

/// Tooltip text: the track when one is playing, else the app name. Windows
/// tooltips overflow past ~127 chars, so titles are capped at 120.
pub fn tooltip(item: Option<&QueueItem>) -> String {
    match item {
        Some(item) => {
            let text = format!("{} - {}", item.title, item.artist);
            if text.chars().count() > 120 {
                text.chars().take(119).collect::<String>() + "…"
            } else {
                text
            }
        }
        None => "OxiTide".to_string(),
    }
}

/// Create the tray icon and menu. Called once at startup, before login.
pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let show_hide = MenuItem::with_id(app, "show_hide", "Show / Hide", true, None::<&str>)?;
    let play_pause = MenuItem::with_id(app, "play_pause", "Play / Pause", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "Previous", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu =
        Menu::with_items(app, &[&show_hide, &play_pause, &next, &prev, &separator, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("OxiTide")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if let Some(action) = menu_action(event.id.as_ref()) {
                run(app, action);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                run(tray.app_handle(), TrayAction::ShowHide);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

/// Dispatch one tray action. Transport is a no-op without a player.
pub fn run(app: &AppHandle, action: TrayAction) {
    match action {
        TrayAction::ShowHide => toggle_window(app),
        TrayAction::Quit => {
            // With close-to-tray active the normal CloseRequested path is
            // bypassed by app.exit; save geometry explicitly so a tray-only
            // quit keeps the current window placement.
            crate::save_window_state(app);
            app.exit(0);
        }
        TrayAction::PlayPause | TrayAction::Next | TrayAction::Prev => {
            let Some(player) = app.state::<crate::state::AppState>().player() else {
                return;
            };
            let command = match action {
                TrayAction::PlayPause => EngineCmd::PlayPause,
                TrayAction::Next => EngineCmd::Next,
                _ => EngineCmd::Prev,
            };
            if let Err(e) = player.engine.handle(command) {
                tracing::warn!("tray transport failed: {e}");
            }
        }
    }
}

/// Show and focus the window when hidden/minimized, hide it when visible.
pub fn toggle_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Push the current track (or the app name) onto the tray tooltip. Called
/// from the engine event subscriber; a missing tray is a no-op.
pub fn update_tooltip(app: &AppHandle, item: Option<&QueueItem>) {
    let tray = app.state::<crate::state::AppState>().tray();
    if let Some(tray) = tray {
        if let Err(e) = tray.set_tooltip(Some(tooltip(item))) {
            tracing::debug!("tray tooltip update failed: {e}");
        }
    }
}
```

Add `mod tray;` to `crates/oxitide-app/src/main.rs` between `mod track_flags;` and `mod window;`.

In `state.rs`, add the field after `track_flags`:

```rust
    /// System tray handle; `None` when the tray could not be created.
    pub tray: Mutex<Option<tauri::tray::TrayIcon>>,
```

and accessors after `track_flags()`:

```rust
    pub fn set_tray(&self, tray: tauri::tray::TrayIcon) {
        *self.tray.lock().unwrap() = Some(tray);
    }

    pub fn tray(&self) -> Option<tauri::tray::TrayIcon> {
        self.tray.lock().unwrap().clone()
    }
```

In `commands.rs::build_player`, inside the first `engine.subscribe` closure, after the `EngineEvent::Error` block (`:446-450`), add:

```rust
        match &event {
            EngineEvent::NowPlaying(item) => {
                crate::tray::update_tooltip(&handle, item.as_ref());
            }
            EngineEvent::State(snapshot) => {
                crate::tray::update_tooltip(&handle, snapshot.current.as_ref());
            }
            _ => {}
        }
```

In `main.rs`, make the save helper reachable from the tray: change `fn save_window_state` (`:82`) to `pub(crate) fn save_window_state`.

Fix the single-instance callback (`:211-215`):

```rust
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
```

In `setup`, after `app.state::<AppState>().set_handle(...)` (`:234`), add:

```rust
            match tray::build(app.handle()) {
                Ok(icon) => app.state::<AppState>().set_tray(icon),
                Err(e) => tracing::warn!("system tray unavailable: {e}"),
            }
```

Replace the `CloseRequested` arm (`:423-430`) with:

```rust
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                save_window_state(app_handle);
                if app_handle.state::<AppState>().config().ui.close_to_tray {
                    api.prevent_close();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app tray`
Expected: PASS (3 tests).

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green. (First run compiles the `tray-icon` feature; that is expected.)

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/Cargo.toml crates/oxitide-app/src/tray.rs crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add the system tray, close-to-tray, and instance re-show"
```

---

### Task 3: Global hotkeys — plugin, registration, commands

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml:13-16` (plugin dependency)
- Create: `crates/oxitide-app/src/hotkeys.rs`
- Modify: `crates/oxitide-app/src/commands.rs` (two commands after `set_discord`, `:1930`)
- Modify: `crates/oxitide-app/src/main.rs:7-19` (module), `:210-220` (plugin), `:234-241` (startup registration), `:297-420` (`generate_handler!`)

**Interfaces:**
- Consumes: `HotkeysConfig` (Task 1), `AppState::player/tray` (Task 2), `crate::tray::{run, TrayAction}`.
- Produces: `HotkeyAction`; `ACTIONS`, `is_action`, `action_from_name`, `bindings`, `parse`, `set`, `app_shortcuts`, `should_skip_when_focused`, `action_for`, `handle`, `run`, `next_volume`, `window_focused`, `register_all`; commands `set_close_to_tray`, `set_hotkey` — the UI (Task 4–5) and the smoke checklist consume these.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/hotkeys.rs` with only the test module (implementation in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_documented_forms_and_rejects_junk() {
        assert!(parse("Ctrl+Alt+KeyP").is_ok());
        assert!(parse("MediaPlayPause").is_ok());
        assert!(parse("VolumeUp").is_ok());
        assert!(parse("  F12  ").is_ok());
        assert!(parse("").is_err());
        assert!(parse("Bogus+KeyP").is_err());
        assert!(parse("Ctrl+Shift").is_err());
    }

    #[test]
    fn bindings_skip_empty_entries_and_keep_action_order() {
        let mut config = HotkeysConfig::default();
        config.next = "Ctrl+Alt+KeyN".into();
        config.show_hide = "  ".into();
        config.play_pause = "F9".into();
        assert_eq!(bindings(&config), vec![("play_pause", "F9"), ("next", "Ctrl+Alt+KeyN")]);
    }

    #[test]
    fn set_writes_known_actions_and_rejects_unknown() {
        let mut config = HotkeysConfig::default();
        set(&mut config, "play_pause", " Ctrl+Alt+KeyP ").unwrap();
        assert_eq!(config.play_pause, "Ctrl+Alt+KeyP");
        assert!(set(&mut config, "love", "Ctrl+Alt+KeyL").is_err());
        assert!(config.next.is_empty());
    }

    #[test]
    fn action_for_matches_case_insensitively() {
        let mut config = HotkeysConfig::default();
        config.play_pause = "ctrl+alt+KeyP".into();
        assert_eq!(
            action_for(&config, &parse("CTRL+ALT+KeyP").unwrap()),
            Some(HotkeyAction::PlayPause)
        );
        assert_eq!(action_for(&config, &parse("F9").unwrap()), None);
    }

    /// Review Focus: while the window is focused the DOM owns its table; a
    /// custom chord must still reach the global handler.
    #[test]
    fn focused_skip_covers_the_dom_table_but_not_custom_chords() {
        assert!(should_skip_when_focused(&parse("Space").unwrap()));
        assert!(should_skip_when_focused(&parse("KeyM").unwrap()));
        assert!(should_skip_when_focused(&parse("Ctrl+KeyF").unwrap()));
        assert!(should_skip_when_focused(&parse("Shift+ArrowRight").unwrap()));
        assert!(!should_skip_when_focused(&parse("Ctrl+Alt+KeyP").unwrap()));
        assert!(!should_skip_when_focused(&parse("MediaPlayPause").unwrap()));
    }

    #[test]
    fn next_volume_clamps_to_zero_and_one() {
        assert!((next_volume(0.5, 0.05) - 0.55).abs() < 1e-6);
        assert!((next_volume(0.98, 0.05) - 1.0).abs() < 1e-6);
        assert!((next_volume(0.02, -0.05) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn action_list_matches_the_ui_contract() {
        assert_eq!(ACTIONS.len(), 6);
        assert!(is_action("volume_down"));
        assert!(!is_action("love"), "love ships with the deferred 10.4 slice");
        assert_eq!(action_from_name("show_hide"), Some(HotkeyAction::ShowHide));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app hotkeys`
Expected: FAIL to compile ("cannot find function `parse`" etc.).

- [ ] **Step 3: Implement the hotkey module and wiring**

Add the dependency in `crates/oxitide-app/Cargo.toml` (after `tauri-plugin-single-instance`):

```toml
tauri-plugin-global-shortcut = "2"
```

Prepend the implementation to `crates/oxitide-app/src/hotkeys.rs` (before the test module):

```rust
//! Global hotkeys (spec §6.6): user-defined accelerators registered through
//! `tauri-plugin-global-shortcut`. Registration is best-effort: a conflicting
//! accelerator surfaces to the caller and never blocks startup.

use oxitide_audio::engine::EngineCmd;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::config::HotkeysConfig;
use crate::state::AppState;

/// Volume step for the volume hotkeys; matches the in-app arrow-key step.
pub const VOLUME_STEP: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    PlayPause,
    Next,
    Prev,
    VolumeUp,
    VolumeDown,
    ShowHide,
}

/// Every action the UI can bind, in display order. `love` is absent while
/// 10.4 is deferred.
pub const ACTIONS: [&str; 6] =
    ["play_pause", "next", "prev", "volume_up", "volume_down", "show_hide"];

pub fn is_action(action: &str) -> bool {
    ACTIONS.contains(&action)
}

pub fn action_from_name(name: &str) -> Option<HotkeyAction> {
    match name {
        "play_pause" => Some(HotkeyAction::PlayPause),
        "next" => Some(HotkeyAction::Next),
        "prev" => Some(HotkeyAction::Prev),
        "volume_up" => Some(HotkeyAction::VolumeUp),
        "volume_down" => Some(HotkeyAction::VolumeDown),
        "show_hide" => Some(HotkeyAction::ShowHide),
        _ => None,
    }
}

/// Configured bindings with empty entries skipped, in action order.
pub fn bindings(config: &HotkeysConfig) -> Vec<(&'static str, &str)> {
    [
        ("play_pause", config.play_pause.as_str()),
        ("next", config.next.as_str()),
        ("prev", config.prev.as_str()),
        ("volume_up", config.volume_up.as_str()),
        ("volume_down", config.volume_down.as_str()),
        ("show_hide", config.show_hide.as_str()),
    ]
    .into_iter()
    .filter(|(_, accelerator)| !accelerator.trim().is_empty())
    .collect()
}

/// Parse an accelerator in the plugin's format (`"Ctrl+Alt+KeyP"`,
/// `"MediaPlayPause"`, `"VolumeUp"`).
pub fn parse(accelerator: &str) -> Result<Shortcut, String> {
    accelerator.trim().parse::<Shortcut>().map_err(|e| e.to_string())
}

/// Write one action's accelerator; unknown actions are rejected.
pub fn set(config: &mut HotkeysConfig, action: &str, accelerator: &str) -> Result<(), String> {
    let field = match action {
        "play_pause" => &mut config.play_pause,
        "next" => &mut config.next,
        "prev" => &mut config.prev,
        "volume_up" => &mut config.volume_up,
        "volume_down" => &mut config.volume_down,
        "show_hide" => &mut config.show_hide,
        _ => return Err(format!("unknown hotkey action: {action}")),
    };
    *field = accelerator.trim().to_string();
    Ok(())
}

/// The accelerators the DOM owns (`ui/src/lib/shortcuts.ts`). While the
/// window is focused the global handler ignores them so one press fires once.
pub fn app_shortcuts() -> Vec<Shortcut> {
    use Code::*;
    vec![
        Shortcut::new(None, Space),
        Shortcut::new(None, Slash),
        Shortcut::new(None, Escape),
        Shortcut::new(None, ArrowLeft),
        Shortcut::new(None, ArrowRight),
        Shortcut::new(None, ArrowUp),
        Shortcut::new(None, ArrowDown),
        Shortcut::new(None, KeyM),
        Shortcut::new(Some(Modifiers::SHIFT), ArrowLeft),
        Shortcut::new(Some(Modifiers::SHIFT), ArrowRight),
        Shortcut::new(Some(Modifiers::ALT), ArrowLeft),
        Shortcut::new(Some(Modifiers::ALT), ArrowRight),
        Shortcut::new(Some(Modifiers::CONTROL), KeyF),
    ]
}

pub fn should_skip_when_focused(shortcut: &Shortcut) -> bool {
    app_shortcuts()
        .iter()
        .any(|candidate| candidate.matches(shortcut.mods, shortcut.key))
}

/// The action bound to a triggered shortcut under the current config.
pub fn action_for(config: &HotkeysConfig, shortcut: &Shortcut) -> Option<HotkeyAction> {
    bindings(config).into_iter().find_map(|(name, accelerator)| {
        let parsed = parse(accelerator).ok()?;
        if parsed.matches(shortcut.mods, shortcut.key) {
            action_from_name(name)
        } else {
            None
        }
    })
}

/// Plugin handler: fires for every registered accelerator press.
pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    if window_focused(app) && should_skip_when_focused(shortcut) {
        return;
    }
    let config = app.state::<AppState>().config();
    if let Some(action) = action_for(&config.hotkeys, shortcut) {
        run(app, action);
    }
}

pub fn window_focused(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false)
}

/// Run one hotkey action; transport is a no-op without a player.
pub fn run(app: &AppHandle, action: HotkeyAction) {
    match action {
        HotkeyAction::ShowHide => crate::tray::run(app, crate::tray::TrayAction::ShowHide),
        HotkeyAction::PlayPause | HotkeyAction::Next | HotkeyAction::Prev => {
            let Some(player) = app.state::<AppState>().player() else {
                return;
            };
            let command = match action {
                HotkeyAction::PlayPause => EngineCmd::PlayPause,
                HotkeyAction::Next => EngineCmd::Next,
                _ => EngineCmd::Prev,
            };
            if let Err(e) = player.engine.handle(command) {
                tracing::warn!("global hotkey transport failed: {e}");
            }
        }
        HotkeyAction::VolumeUp => adjust_volume(app, VOLUME_STEP),
        HotkeyAction::VolumeDown => adjust_volume(app, -VOLUME_STEP),
    }
}

pub fn next_volume(current: f32, delta: f32) -> f32 {
    (current + delta).clamp(0.0, 1.0)
}

fn adjust_volume(app: &AppHandle, delta: f32) {
    let state = app.state::<AppState>();
    let Some(player) = state.player() else {
        return;
    };
    let volume = next_volume(player.engine.state().volume, delta);
    if let Err(e) = player.engine.handle(EngineCmd::SetVolume(volume)) {
        tracing::warn!("global hotkey volume failed: {e}");
        return;
    }
    let mut config = state.config();
    config.audio.volume = volume;
    if let Err(e) = config.save() {
        tracing::warn!("failed to persist hotkey volume: {e}");
    }
    state.set_config(config);
}

/// Register every configured accelerator, clearing the previous set first.
/// The returned error names the action whose binding failed; registrations
/// that already succeeded in this call stay active until the next call.
pub fn register_all(app: &AppHandle, config: &HotkeysConfig) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    if let Err(e) = shortcuts.unregister_all() {
        tracing::warn!("failed to clear global hotkeys: {e}");
    }
    for (name, accelerator) in bindings(config) {
        let shortcut = parse(accelerator).map_err(|e| format!("{name}: {e}"))?;
        shortcuts.register(shortcut).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}
```

Add `mod hotkeys;` to `main.rs` between `mod history;` and `mod lrc;`.

In `commands.rs`, add the two commands after `set_discord` (`:1930`):

```rust
#[tauri::command]
pub fn set_close_to_tray(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.close_to_tray = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

/// Bind (or clear, with an empty accelerator) one global hotkey. Registration
/// failures restore the previous config and binding, then surface an error.
#[tauri::command(async)]
pub fn set_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    accelerator: String,
) -> Result<(), String> {
    if !crate::hotkeys::is_action(&action) {
        return Err(format!("unknown hotkey action: {action}"));
    }
    let accelerator = accelerator.trim().to_string();
    if !accelerator.is_empty() {
        crate::hotkeys::parse(&accelerator)?;
    }
    let previous = state.config();
    let mut next = previous.clone();
    crate::hotkeys::set(&mut next.hotkeys, &action, &accelerator)?;
    next.save()?;
    state.set_config(next.clone());
    if let Err(e) = crate::hotkeys::register_all(&app, &next.hotkeys) {
        let _ = previous.save();
        state.set_config(previous.clone());
        let _ = crate::hotkeys::register_all(&app, &previous.hotkeys);
        return Err(e);
    }
    Ok(())
}
```

In `main.rs`, add the plugin next to the single-instance plugin (`:211-216`):

```rust
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkeys::handle)
                .build(),
        )
```

In `setup`, after the tray build block from Task 2, add startup registration:

```rust
            let config = app.state::<AppState>().config();
            if let Err(e) = hotkeys::register_all(app.handle(), &config.hotkeys) {
                tracing::warn!("global hotkeys unavailable: {e}");
            }
```

Register both commands in `generate_handler!` next to `commands::set_smtc,` (`:409`):

```rust
            commands::set_close_to_tray,
            commands::set_hotkey,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app hotkeys`
Expected: PASS (7 tests).

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/Cargo.toml crates/oxitide-app/src/hotkeys.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add global hotkey registration and settings commands"
```

---

### Task 4: UI API surface and hotkey helpers

**Files:**
- Create: `ui/src/lib/hotkeys.ts`
- Create: `ui/src/lib/hotkeys.test.ts`
- Modify: `ui/src/lib/api.ts` (Config `:182-220`, wrappers after `setSmtc` `:405`)

**Interfaces:**
- Consumes: nothing from Rust at runtime (type mirror only).
- Produces: `HOTKEY_ACTIONS` / `HotkeyAction`; `acceleratorFromEvent(KeyLike) -> string | null`; `displayAccelerator(string) -> string`; `Config.ui.close_to_tray`; `Config.hotkeys`; `api.setCloseToTray/setHotkey` — Task 5 consumes all of these.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/hotkeys.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, displayAccelerator } from "./hotkeys";

function key(
  code: string,
  mods: Partial<{ ctrl: boolean; alt: boolean; shift: boolean; meta: boolean }> = {},
) {
  return {
    code,
    ctrlKey: mods.ctrl ?? false,
    altKey: mods.alt ?? false,
    shiftKey: mods.shift ?? false,
    metaKey: mods.meta ?? false,
  };
}

describe("acceleratorFromEvent", () => {
  it("builds modifier+key accelerators", () => {
    expect(acceleratorFromEvent(key("KeyP", { ctrl: true, alt: true }))).toBe("Ctrl+Alt+KeyP");
    expect(acceleratorFromEvent(key("Space", { ctrl: true }))).toBe("Ctrl+Space");
    expect(acceleratorFromEvent(key("MediaPlayPause"))).toBe("MediaPlayPause");
    expect(acceleratorFromEvent(key("F12"))).toBe("F12");
  });

  it("ignores lone modifier presses and unidentified keys", () => {
    expect(acceleratorFromEvent(key("ControlLeft", { ctrl: true }))).toBeNull();
    expect(acceleratorFromEvent(key("ShiftRight", { shift: true }))).toBeNull();
    expect(acceleratorFromEvent(key("Unidentified"))).toBeNull();
  });
});

describe("displayAccelerator", () => {
  it("prettifies stored tokens", () => {
    expect(displayAccelerator("Ctrl+Alt+KeyP")).toBe("Ctrl + Alt + P");
    expect(displayAccelerator("Shift+Digit1")).toBe("Shift + 1");
    expect(displayAccelerator("MediaPlayPause")).toBe("Media Play/Pause");
    expect(displayAccelerator("")).toBe("");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/hotkeys.test.ts`
Expected: FAIL — cannot resolve `./hotkeys`.

- [ ] **Step 3: Implement `ui/src/lib/hotkeys.ts`**

```ts
/// Global-hotkey helpers. The action ids mirror `HotkeysConfig` in Rust and
/// the `set_hotkey` command's whitelist; the accelerator format is
/// `tauri-plugin-global-shortcut`'s (`"Ctrl+Alt+KeyP"`, `"MediaPlayPause"`).

export const HOTKEY_ACTIONS = [
  { id: "play_pause", label: "Play / pause" },
  { id: "next", label: "Next track" },
  { id: "prev", label: "Previous track" },
  { id: "volume_up", label: "Volume up" },
  { id: "volume_down", label: "Volume down" },
  { id: "show_hide", label: "Show / hide window" },
] as const;

export type HotkeyAction = (typeof HOTKEY_ACTIONS)[number]["id"];

export interface KeyLike {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/// Accelerator for a captured keydown, or null for lone modifier presses and
/// unidentified keys so the capture keeps waiting.
export function acceleratorFromEvent(event: KeyLike): string | null {
  if (!event.code || event.code === "Unidentified" || MODIFIER_CODES.has(event.code)) {
    return null;
  }
  const tokens: string[] = [];
  if (event.ctrlKey) tokens.push("Ctrl");
  if (event.altKey) tokens.push("Alt");
  if (event.shiftKey) tokens.push("Shift");
  if (event.metaKey) tokens.push("Super");
  tokens.push(event.code);
  return tokens.join("+");
}

const KEY_LABELS: Record<string, string> = {
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  ArrowDown: "Down",
  Escape: "Esc",
  MediaPlayPause: "Media Play/Pause",
  MediaTrackNext: "Media Next",
  MediaTrackPrevious: "Media Previous",
};

/// Human-readable form of a stored accelerator.
export function displayAccelerator(accelerator: string): string {
  if (!accelerator) return "";
  return accelerator
    .split("+")
    .map((token) => {
      if (/^Key[A-Z]$/.test(token)) return token.slice(3);
      if (/^Digit\d$/.test(token)) return token.slice(5);
      return KEY_LABELS[token] ?? token;
    })
    .join(" + ");
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/hotkeys.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Extend `ui/src/lib/api.ts`**

Add at the top, next to the tauri import:

```ts
import type { HotkeyAction } from "./hotkeys";
```

Extend `Config.ui` (after `oneko: boolean;`):

```ts
    close_to_tray: boolean;
```

and add a new section after `content`:

```ts
  hotkeys: {
    play_pause: string;
    next: string;
    prev: string;
    volume_up: string;
    volume_down: string;
    show_hide: string;
  };
```

Add the wrappers after `setSmtc`:

```ts
  setCloseToTray: (enabled: boolean) => invoke<void>("set_close_to_tray", { enabled }),
  setHotkey: (action: HotkeyAction, accelerator: string) =>
    invoke<void>("set_hotkey", { action, accelerator }),
```

- [ ] **Step 6: Verify**

Run: `npm --prefix ui test`, `npm --prefix ui run check`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add ui/src/lib/hotkeys.ts ui/src/lib/hotkeys.test.ts ui/src/lib/api.ts
git commit -m "feat(ui): add hotkey capture helpers and the desktop API surface"
```

---

### Task 5: Settings → System — Desktop and Global hotkeys

**Files:**
- Modify: `ui/src/views/Settings.svelte` (imports `:1-31`, state `:58`, handlers near `setSmtc` `:311-315`, panel `:846-865`, styles `:869-1005`)

**Interfaces:**
- Consumes: `HOTKEY_ACTIONS`, `acceleratorFromEvent`, `displayAccelerator`, `HotkeyAction` (Task 4); `api.setCloseToTray/setHotkey` (Task 4); the existing `apply(e)` toast helper.
- Produces: the Desktop + Global hotkeys settings surface (spec §6.6, §7 UI tests).

- [ ] **Step 1: Add imports and state**

Add to the imports:

```ts
  import {
    HOTKEY_ACTIONS,
    acceleratorFromEvent,
    displayAccelerator,
    type HotkeyAction,
  } from "../lib/hotkeys";
```

Add next to `confirmingBlockExplicit`:

```ts
  let recordingAction = $state<HotkeyAction | null>(null);
```

- [ ] **Step 2: Add the capture effect and handlers**

Add the effect next to the `customDraft` sync effect (`:178-180`):

```ts
  // While recording, a capture-phase listener owns the keyboard: the app's
  // window-level shortcut handler never sees the press.
  $effect(() => {
    if (!recordingAction) return;
    const handler = (event: KeyboardEvent) => onHotkeyKeydown(event);
    window.addEventListener("keydown", handler, true);
    document.getElementById(`hotkey-${recordingAction}`)?.focus();
    return () => window.removeEventListener("keydown", handler, true);
  });
```

Add the handlers next to `setSmtc`:

```ts
  function setCloseToTray(enabled: boolean) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, close_to_tray: enabled } };
    api.setCloseToTray(enabled).catch(apply);
  }

  function onHotkeyKeydown(event: KeyboardEvent) {
    if (!recordingAction) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      recordingAction = null;
      return;
    }
    if (event.key === "Backspace" || event.key === "Delete") {
      void saveHotkey(recordingAction, "");
      return;
    }
    const accelerator = acceleratorFromEvent(event);
    if (!accelerator) return;
    const action = recordingAction;
    recordingAction = null;
    void saveHotkey(action, accelerator);
  }

  /// Optimistic update; the Rust command rolls back its own config on a
  /// registration conflict, so a failure restores the shown binding too.
  async function saveHotkey(action: HotkeyAction, accelerator: string) {
    if (!config) return;
    const previous = config.hotkeys[action];
    config = { ...config, hotkeys: { ...config.hotkeys, [action]: accelerator } };
    try {
      await api.setHotkey(action, accelerator);
    } catch (e) {
      config = { ...config, hotkeys: { ...config.hotkeys, [action]: previous } };
      apply(e);
    }
  }
```

- [ ] **Step 3: Add the panel markup**

In the System panel, after the Account block's closing `</p>` (after the logout button, `:855`) and before `<h3>Keyboard shortcuts</h3>`, insert:

```svelte
      <h3>Desktop</h3>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.ui.close_to_tray}
              onchange={(e) => setCloseToTray((e.target as HTMLInputElement).checked)}
            />
            <span>Close to tray</span>
          </label>
          <span class="muted">
            Closing the window hides it and keeps playback running; Quit lives in
            the tray menu.
          </span>
        </div>
      </div>

      <h3>Global hotkeys</h3>
      <p class="muted">
        Work while OxiTide is in the background. Record a shortcut, or press
        Backspace/Delete while recording to clear it. A shortcut that is already
        taken shows a message and keeps the old binding.
      </p>
      {#each HOTKEY_ACTIONS as entry (entry.id)}
        <div class="hotkey-row">
          <span class="hotkey-label">{entry.label}</span>
          <kbd>{displayAccelerator(config.hotkeys[entry.id]) || "Not set"}</kbd>
          <button
            id={`hotkey-${entry.id}`}
            class:recording={recordingAction === entry.id}
            onclick={() =>
              (recordingAction = recordingAction === entry.id ? null : entry.id)}
          >{recordingAction === entry.id ? "Press keys…" : "Record"}</button>
          {#if config.hotkeys[entry.id]}
            <button onclick={() => void saveHotkey(entry.id, "")}>Clear</button>
          {/if}
        </div>
      {/each}
```

- [ ] **Step 4: Add the styles**

Append to the component `<style>` block:

```css
  .hotkey-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 6px 0;
  }
  .hotkey-label {
    min-width: 150px;
  }
  .hotkey-row kbd {
    min-width: 140px;
    color: var(--text-muted);
  }
  .hotkey-row button.recording {
    border: 1px solid var(--accent);
    color: var(--accent);
  }
```

- [ ] **Step 5: Verify**

Run: `npm --prefix ui run check`, `npm --prefix ui test`
Expected: no errors, all tests pass. Manual (record in the smoke checklist): each control live-applies — close-to-tray toggles, recording shows the chord, Escape cancels, Backspace clears, a conflict toasts and keeps the old binding, and all picks survive a restart.

- [ ] **Step 6: Commit**

```bash
git add ui/src/views/Settings.svelte
git commit -m "feat(ui): add desktop and global-hotkey settings"
```

---

### Task 6: Smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-05-track10-6-tray-hotkeys-smoke.md`

- [ ] **Step 1: Write the checklist**

```markdown
# Track 10.6 tray and global hotkeys — GUI smoke checklist

Human, real account, Windows. Record pass/fail per step; steps 3–5 and 7–11 need
a track loaded unless stated. Linux caveats (tray needs libappindicator; Wayland
global shortcuts need a portal; left-click menu behavior) follow the Track 8
exemption pattern and are checked in the track completion note.

1. The tray icon appears when the app starts; hovering it shows "OxiTide".
2. Play a track: within a second the tooltip reads "Title - Artist". Tray
   Play/Pause, Next, and Previous control playback. End the queue: the tooltip
   returns to "OxiTide".
3. Press the window X with "Close to tray" on (default): the window hides, the
   app keeps playing, and the tray icon stays. Left-clicking the tray icon
   re-shows the window; the tray menu's Show / Hide also toggles it.
4. Tray menu → Quit exits fully. Relaunching restores the paused session and
   the window geometry from step 3.
5. Launch a second instance while the window is hidden: the window shows,
   unminimizes, and focuses; no second tray icon appears.
6. Settings → System: "Close to tray" is checked. Turn it off, press X: the app
   exits. Turn it on again: X hides.
7. Record `Ctrl+Alt+Space` for Play / pause; the row shows "Ctrl + Alt + Space"
   and playback toggles from another focused app. With the OxiTide window
   focused, pressing Space still toggles exactly once (no double-fire).
8. Try to record `Ctrl+Alt+Space` for Next: a toast appears and Play / pause
   keeps its binding; restarting keeps it too.
9. Record a chord another app already owns (e.g. an OS-reserved one): a toast
   appears and the previous binding stays.
10. Clear a hotkey (Backspace/Delete while recording, or the Clear button):
    the row shows "Not set" and the accelerator no longer fires after restart.
11. Volume up/down hotkeys move the volume in 5% steps and the level survives a
    restart (`config.audio.volume`).
12. `%APPDATA%\OxiTide\config\config.toml` contains `close_to_tray = true|false`
    under `[ui]` and a `[hotkeys]` section with the recorded accelerators.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-05-track10-6-tray-hotkeys-smoke.md
git commit -m "docs: add the Track 10.6 tray and hotkeys smoke checklist"
```

---

## Self-Review Notes

- **Spec coverage:** tray feature + menu Show/Hide, Play/Pause, Next, Previous, separator, Quit (§6.6) → Task 2; tooltip mirrors NowPlaying (§6.6) → Task 2 (`update_tooltip` on `NowPlaying`/`State`); close-to-tray default true + setting-off restores exit (§6.6, QoL-D3) → Tasks 1, 2, 5; single-instance show+unminimize+focus (§6.6) → Task 2; global hotkeys via `tauri-plugin-global-shortcut`, empty defaults, startup + save registration, unregister-old-first, conflict keeps previous binding, focused-skip of the in-app table, media keys stay on SMTC (§6.6, QoL-D10) → Tasks 3–5; config `[hotkeys]` + `ui.close_to_tray` (§5.3) → Task 1; Linux caveats documented (tray/libappindicator, Wayland portal) → smoke note + Task spec refinements; PM-D1 impact is nil (no polling, one tray/tooltip update per track, bounded 5% config writes) → noted in Task 3.
- **Deferred scope untouched:** no `love` action (rejected by `is_action`), no 10.4 module work; `AGENTS.md` updates left to the track completion note.
- **Type consistency:** `HotkeyAction` ids match `ACTIONS` and `HOTKEY_ACTIONS`; `bindings` order matches `ACTIONS`; `set`/`action_from_name` cover the same six names; Task 4's `Config.hotkeys` keys match `HotkeysConfig` fields exactly.
