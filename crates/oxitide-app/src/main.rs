#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artwork;
mod commands;
mod config;
mod lrc;
mod lyrics;
mod media_controls;
mod palette;
mod player;
mod scheme;
mod scrobble;
mod state;
mod window;

use std::sync::OnceLock;

use oxitide_tidal::auth;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
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
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(writer),
            )
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
    let live = window::FullGeometry {
        width: config.ui.window_width,
        height: config.ui.window_height,
        x: config.ui.window_x,
        y: config.ui.window_y,
        maximized: config.ui.window_maximized,
    };
    let saved = window::geometry_to_save(state.pre_mini(), live);
    config.ui.window_width = saved.width;
    config.ui.window_height = saved.height;
    config.ui.window_x = saved.x;
    config.ui.window_y = saved.y;
    config.ui.window_maximized = saved.maximized;
    if let Err(e) = config.save() {
        tracing::warn!("failed to save window geometry: {e}");
    }
    state.set_config(config);
}

/// Complete a login from a `tidal://login/auth?code=…` URL. Ignores any other
/// deep link. Runs off the event thread; the result reaches the UI through both
/// the `auth://*` event and the buffered `auth_take_result`.
fn handle_deep_link(app: &tauri::AppHandle, raw: &str) {
    if auth::validate_redirect(raw).is_err() {
        tracing::debug!("ignoring non-login deep link: {raw}");
        return;
    }
    let app = app.clone();
    let url = raw.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match commands::finish_login(&app, &state, url).await {
            Ok(session) => {
                state.set_last_login(commands::LoginOutcome::Success(session.clone()));
                let _ = app.emit("auth://success", session);
            }
            Err(error) => {
                tracing::warn!("deep-link login failed: {error}");
                state.set_last_login(commands::LoginOutcome::Error(error.clone()));
                let _ = app.emit("auth://error", error);
            }
        }
    });
}

fn main() {
    init_logging();
    let config = config::Config::load();
    let state = AppState::default();
    state.set_cache(player::build_cache(&config));
    state.set_artwork(artwork::ArtworkService::new(
        reqwest::Client::new(),
        "https://resources.tidal.com/images".into(),
        config::default_cache_dir()
            .unwrap_or_default()
            .join("palettes.json"),
    ));
    state.set_lyrics(lyrics::LyricsService::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()),
        "https://lrclib.net",
        config::default_cache_dir()
            .unwrap_or_default()
            .join("lyrics.json"),
    ));
    let scrobble_http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let (lastfm_key, lastfm_secret) = config.scrobbling.lastfm_credentials();
    let scrobbler = scrobble::ScrobblerService::new(
        scrobble_http,
        "https://ws.audioscrobbler.com/2.0/",
        "https://api.listenbrainz.org/1/",
        std::sync::Arc::new(scrobble::credentials::KeyringCredentialStore),
        lastfm_key,
        lastfm_secret,
        config.scrobbling.lastfm_enabled,
        config.scrobbling.listenbrainz_enabled,
    );
    state.set_scrobbler(scrobbler.clone());
    state.set_config(config);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(|app| {
            scheme::remember(scheme::snapshot());
            #[cfg(any(windows, target_os = "linux"))]
            if let Err(e) = app.deep_link().register_all() {
                tracing::warn!("deep link registration failed: {e}");
            }
            let handle = app.handle().clone();
            app.deep_link().on_open_url({
                let handle = handle.clone();
                move |event| {
                    for url in event.urls() {
                        handle_deep_link(&handle, url.as_str());
                    }
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_deep_link(&handle, url.as_str());
                }
            }
            app.state::<AppState>().set_handle(app.handle().clone());
            if let Some(scrobbler) = app.state::<AppState>().scrobbler() {
                let handle = app.handle().clone();
                scrobbler.set_status_sink(std::sync::Arc::new(move |status| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://status", status);
                }));
                let handle = app.handle().clone();
                scrobbler.set_auth_error_sink(std::sync::Arc::new(move |service| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://auth-error", service);
                }));
                scrobbler.start();
            }
            if let Some(window) = app.get_webview_window("main") {
                let config = app.state::<AppState>().config();
                if let Err(e) = window::apply(&window, &config) {
                    tracing::warn!("failed to apply window geometry: {e}");
                }
                if config.ui.start_in_mini {
                    // MP-D11: the stash keeps the saved full geometry (including
                    // a saved maximized state) so exiting mini restores it.
                    let stash = window::FullGeometry {
                        width: config.ui.window_width,
                        height: config.ui.window_height,
                        x: config.ui.window_x,
                        y: config.ui.window_y,
                        maximized: config.ui.window_maximized,
                    };
                    app.state::<AppState>().set_pre_mini(stash);
                    if let Err(e) = window::enter_mini(&window, stash) {
                        tracing::warn!("failed to start in mini player: {e}");
                    }
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
            commands::auth_take_result,
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
            commands::get_explore,
            commands::get_feed,
            commands::get_favorite_mixes,
            commands::get_mix,
            commands::get_for_you,
            commands::get_track_radio,
            commands::get_artist_radio,
            commands::get_similar_artists,
            commands::get_lyrics,
            commands::get_scrobble_status,
            commands::lastfm_begin_auth,
            commands::lastfm_complete_auth,
            commands::lastfm_cancel_auth,
            commands::lastfm_disconnect,
            commands::listenbrainz_connect,
            commands::listenbrainz_disconnect,
            commands::set_scrobble_enabled,
            commands::get_collection_items,
            commands::create_folder,
            commands::rename_folder,
            commands::remove_collection_entry,
            commands::move_playlists_to_folder,
            commands::get_purchases,
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
            commands::set_dynamic_color,
            commands::set_queue_width,
            commands::set_mini_mode,
            commands::set_start_in_mini,
            commands::set_oneko,
            commands::set_smtc,
            commands::set_cache_max_size_mb,
            commands::set_cache_clear_on_exit,
            commands::get_logs_dir,
            commands::open_logs_folder,
            commands::analyze_cover,
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
                scheme::restore_remembered();
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
