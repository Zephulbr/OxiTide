#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod player;
mod state;

use std::sync::OnceLock;

use tauri::Manager;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use state::AppState;

/// Keeps the non-blocking file writer's worker thread alive for the process.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// The daily-rotated file writer, or `None` when the logs directory cannot be
/// created (the app then logs to stderr only).
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

/// stderr plus a daily-rotated file log; `RUST_LOG` overrides the `info` default.
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

fn main() {
    init_logging();

    let config = config::Config::load();
    let state = AppState::default();
    state.set_cache(player::build_cache(&config));
    state.set_config(config);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
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
            commands::set_cache_max_size_mb,
            commands::set_cache_clear_on_exit,
            commands::get_logs_dir,
            commands::open_logs_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OxiTide")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                let config = state.config();
                if config.cache.clear_on_exit {
                    if let Some(cache) = state.cache() {
                        cache.clear();
                    }
                }
            }
        });
}
