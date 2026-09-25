#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod player;
mod state;
mod window;

use std::sync::OnceLock;

use tauri::Manager;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use state::AppState;

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

fn main() {
    init_logging();

    let config = config::Config::load();
    let state = AppState::default();
    state.set_cache(player::build_cache(&config));
    state.set_config(config);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
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
        .invoke_handler(tauri::generate_handler![
            commands::auth_begin,
            commands::auth_complete,
            commands::auth_restore,
            commands::auth_status,
            commands::auth_logout,
            commands::get_home,
            commands::get_album,
            commands::get_album_tracks,
            commands::get_playlist,
            commands::get_playlist_tracks,
            commands::get_artist,
            commands::search,
            commands::get_favorite_tracks,
            commands::get_favorite_albums,
            commands::get_favorite_artists,
            commands::get_user_playlists,
            commands::get_favorite_tracks_page,
            commands::get_playlist_tracks_page,
            commands::set_favorite_track,
            commands::set_favorite_album,
            commands::set_favorite_artist,
            commands::get_favorite_ids,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_playlist_tracks,
            commands::remove_playlist_items,
            commands::move_playlist_items,
            commands::retry,
            commands::play_tracks,
            commands::play_pause,
            commands::next,
            commands::prev,
            commands::seek,
            commands::set_volume,
            commands::set_shuffle,
            commands::set_repeat,
            commands::queue_reorder,
            commands::queue_remove,
            commands::queue_add,
            commands::queue_clear,
            commands::get_playback_state,
            commands::get_config,
            commands::get_output_devices,
            commands::set_output_device,
            commands::set_preferred_quality,
            commands::set_exclusive,
            commands::set_exclusive_fallback,
            commands::get_audio_status,
            commands::get_cache_status,
            commands::clear_cache,
            commands::set_allow_fallback,
            commands::set_quality_display,
            commands::set_ui_mode,
            commands::set_typeface,
            commands::set_custom_typeface,
            commands::set_density,
            commands::set_cover_shape,
            commands::set_queue_width,
            commands::set_cache_max_size_mb,
            commands::set_cache_clear_on_exit,
            commands::get_logs_dir,
            commands::open_logs_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OxiTide")
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
}
