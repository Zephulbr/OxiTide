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
