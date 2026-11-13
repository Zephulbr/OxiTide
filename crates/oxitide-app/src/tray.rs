//! System tray (spec §6.6): show/hide, transport, quit, and a tooltip that
//! mirrors the current track. The tray is created once at startup and
//! survives player rebuilds; the engine subscriber pushes tooltip updates.

use crate::shell_text::{self, ShellText};
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

/// Linux: the StatusNotifierItem `Title` is GLib's application name, which
/// defaults to the binary name (`oxitide-app`); tray-icon never sets one.
/// libappindicator has no tooltip, so desktops show this title in its place.
/// Called once at the start of `main`, before GTK starts.
#[cfg(target_os = "linux")]
pub fn set_application_name() {
    #[link(name = "glib-2.0")]
    extern "C" {
        fn g_set_application_name(name: *const std::ffi::c_char);
    }
    // SAFETY: a NUL-terminated static string; GLib copies it.
    unsafe { g_set_application_name(c"OxiTide".as_ptr()) }
}

/// The menu's items with the text each shows, kept so a language switch can
/// relabel them (`relabel`).
pub type TrayItems = Vec<(ShellText, MenuItem<tauri::Wry>)>;

/// Create the tray icon and menu. Called once at startup, before login.
pub fn build(app: &AppHandle) -> tauri::Result<(TrayIcon, TrayItems)> {
    let item = |id: &str, label: ShellText| {
        MenuItem::with_id(app, id, shell_text::text(label), true, None::<&str>)
    };
    let show_hide = item("show_hide", ShellText::TrayShowHide)?;
    let play_pause = item("play_pause", ShellText::TrayPlayPause)?;
    let next = item("next", ShellText::TrayNext)?;
    let prev = item("prev", ShellText::TrayPrevious)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = item("quit", ShellText::TrayQuit)?;
    let menu = Menu::with_items(
        app,
        &[&show_hide, &play_pause, &next, &prev, &separator, &quit],
    )?;
    let items = vec![
        (ShellText::TrayShowHide, show_hide.clone()),
        (ShellText::TrayPlayPause, play_pause.clone()),
        (ShellText::TrayNext, next.clone()),
        (ShellText::TrayPrevious, prev.clone()),
        (ShellText::TrayQuit, quit.clone()),
    ];
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
    Ok((builder.build(app)?, items))
}

/// Re-read every menu item's text in the current shell language.
pub fn relabel(items: &TrayItems) {
    for (label, item) in items {
        if let Err(e) = item.set_text(shell_text::text(*label)) {
            tracing::debug!("tray label update failed: {e}");
        }
    }
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
        crate::window::show_main(app);
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
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
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
