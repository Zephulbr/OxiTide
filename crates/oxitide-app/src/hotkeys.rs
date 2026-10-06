//! Global hotkeys (spec §6.6): user-defined accelerators registered through
//! `tauri-plugin-global-shortcut`. Registration is best-effort: a conflicting
//! accelerator surfaces to the caller and never blocks startup.

use oxitide_audio::engine::EngineCmd;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState,
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

/// Serializes hotkey config writes and re-registration across the Tauri
/// commands: a capture release can never interleave with a save's
/// registration, so the registration that runs last always reflects the
/// latest config (Track 10 follow-up; spec §6.3). Each command reads
/// `state.config()` inside the lock.
static MUTATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn with_mutation_lock<T>(run: impl FnOnce() -> T) -> T {
    let _guard = MUTATION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    run()
}

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

/// Write one action's accelerator; when `replace` names another action, that
/// action is cleared first so the new binding is not an in-app duplicate.
pub fn apply_binding(
    config: &mut HotkeysConfig,
    action: &str,
    accelerator: &str,
    replace: Option<&str>,
) -> Result<(), String> {
    if let Some(replace) = replace {
        if replace != action {
            set(config, replace, "")?;
        }
    }
    set(config, action, accelerator)
}

/// Valid parsed bindings and malformed persisted values, both in action order.
pub type ParsedBindings = (Vec<(&'static str, Shortcut)>, Vec<(&'static str, String)>);

/// Parsed configured bindings, split into valid shortcuts and malformed
/// persisted values (action, parse error). Entries keep action order.
pub fn parsed_bindings(config: &HotkeysConfig) -> ParsedBindings {
    let mut parsed = Vec::new();
    let mut malformed = Vec::new();
    for (name, accelerator) in bindings(config) {
        match parse(accelerator) {
            Ok(shortcut) => parsed.push((name, shortcut)),
            Err(error) => malformed.push((name, error)),
        }
    }
    (parsed, malformed)
}

/// The first two parsed bindings that resolve to the same chord, as
/// `(first action, duplicate action)`. Comparison uses `Shortcut::id()`, which
/// canonicalizes modifiers and key.
pub fn duplicate_binding(config: &HotkeysConfig) -> Option<(&'static str, &'static str)> {
    let (parsed, _) = parsed_bindings(config);
    for (index, (first, shortcut)) in parsed.iter().enumerate() {
        for (second, other) in parsed.iter().skip(index + 1) {
            if shortcut.id() == other.id() {
                return Some((*first, *second));
            }
        }
    }
    None
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

/// Plugin handler: fires for every registered accelerator press. Once a chord
/// is registered the global handler is the only delivery path; a registered
/// chord never reaches the webview.
pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let config = app.state::<AppState>().config();
    if let Some(action) = action_for(&config.hotkeys, shortcut) {
        run(app, action);
    }
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
    let mut config = state.config();
    let volume = next_volume(config.audio.volume, delta);
    if let Err(e) = player.engine.handle(EngineCmd::SetVolume(volume)) {
        tracing::warn!("global hotkey volume failed: {e}");
        return;
    }
    config.audio.volume = volume;
    if let Err(e) = config.save() {
        tracing::warn!("failed to persist hotkey volume: {e}");
    }
    state.set_config(config);
}

/// Register every configured accelerator, clearing the previous set first.
/// Duplicate chords and a failed registration name the offending action;
/// malformed persisted values are logged and skipped.
pub fn register_all(app: &AppHandle, config: &HotkeysConfig) -> Result<(), String> {
    if let Some((first, second)) = duplicate_binding(config) {
        return Err(format!("{second} duplicates the {first} binding"));
    }
    let shortcuts = app.global_shortcut();
    if let Err(e) = shortcuts.unregister_all() {
        tracing::warn!("failed to clear global hotkeys: {e}");
    }
    let (parsed, malformed) = parsed_bindings(config);
    for (name, error) in malformed {
        tracing::warn!("skipping malformed hotkey {name}: {error}");
    }
    for (name, shortcut) in parsed {
        shortcuts.register(shortcut).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}

/// Suspend every registered hotkey while capture is active, then restore the
/// configured set when it ends.
pub fn set_capture(app: &AppHandle, config: &HotkeysConfig, active: bool) -> Result<(), String> {
    if active {
        app.global_shortcut()
            .unregister_all()
            .map_err(|e| e.to_string())
    } else {
        register_all(app, config)
    }
}

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
    fn apply_binding_clears_the_replaced_action() {
        let mut config = HotkeysConfig::default();
        set(&mut config, "volume_up", "Ctrl+Alt+Space").unwrap();
        apply_binding(&mut config, "play_pause", "Ctrl+Alt+Space", Some("volume_up")).unwrap();
        assert_eq!(config.play_pause, "Ctrl+Alt+Space");
        assert_eq!(config.volume_up, "");
        assert!(duplicate_binding(&config).is_none());
    }

    #[test]
    fn apply_binding_ignores_a_replace_equal_to_the_action() {
        let mut config = HotkeysConfig::default();
        apply_binding(&mut config, "next", "F9", Some("next")).unwrap();
        assert_eq!(config.next, "F9");
    }

    #[test]
    fn apply_binding_rejects_an_unknown_replace() {
        let mut config = HotkeysConfig::default();
        assert!(apply_binding(&mut config, "next", "F9", Some("love")).is_err());
        assert_eq!(config.next, "");
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

    #[test]
    fn duplicate_binding_detects_same_chord_ignoring_case_and_format() {
        let mut config = HotkeysConfig::default();
        config.play_pause = "Ctrl+Alt+KeyP".into();
        config.next = "ctrl+alt+keyp".into();
        assert_eq!(duplicate_binding(&config), Some(("play_pause", "next")));
        let mut distinct = HotkeysConfig::default();
        distinct.play_pause = "Ctrl+Alt+KeyP".into();
        distinct.next = "Ctrl+Alt+KeyN".into();
        assert_eq!(duplicate_binding(&distinct), None);
        assert_eq!(duplicate_binding(&HotkeysConfig::default()), None);
    }

    #[test]
    fn parsed_bindings_split_malformed_entries_and_keep_later_valid_ones() {
        let mut config = HotkeysConfig::default();
        config.next = "Bogus+Key".into();
        config.prev = "F9".into();
        let (parsed, malformed) = parsed_bindings(&config);
        assert_eq!(parsed, vec![("prev", parse("F9").unwrap())]);
        assert_eq!(malformed.len(), 1);
        assert_eq!(malformed[0].0, "next");
        assert!(!malformed[0].1.is_empty());
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

    #[test]
    fn mutations_are_serialized() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let inside = Arc::new(AtomicBool::new(false));
        let overlapped = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let inside = inside.clone();
            let overlapped = overlapped.clone();
            handles.push(std::thread::spawn(move || {
                with_mutation_lock(|| {
                    if inside.swap(true, Ordering::SeqCst) {
                        overlapped.store(true, Ordering::SeqCst);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    inside.store(false, Ordering::SeqCst);
                });
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert!(!overlapped.load(Ordering::SeqCst));
    }
}
