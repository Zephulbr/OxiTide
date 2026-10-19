#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod album_dates;
mod artwork;
mod commands;
mod config;
mod discord;
mod history;
mod hotkeys;
mod lrc;
mod lyrics;
mod media_controls;
mod meter;
mod palette;
pub mod perf;
mod player;
mod remote;
mod scheme;
mod scrobble;
mod session;
mod state;
mod thumbbar;
mod titlebar;
mod track_flags;
mod tray;
mod updater;
mod webview_idle;
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
pub(crate) fn save_window_state(app: &tauri::AppHandle) {
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
    let _ = perf::install();
    init_logging();
    perf::mark("logging_ready");
    let config = config::Config::load();
    perf::mark("config_loaded");
    let state = AppState::default();
    let (token_store, keyring_status) = oxitide_tidal::token_store::select_token_store(
        &oxitide_tidal::token_store::SystemKeyringProbe,
    );
    if !keyring_status.persistent {
        tracing::warn!(
            reason = %keyring_status.reason.as_deref().unwrap_or("unknown"),
            "OS keyring unavailable; secrets will last only for this session"
        );
    }
    state.set_token_store(token_store);
    state.set_keyring_status(keyring_status.clone());
    state.set_cache(player::build_cache(&config));
    perf::mark("cache_ready");
    // One client (one connection pool and TLS setup) for the side services:
    // cover palettes, lyrics and scrobbling (Track 13). Clones share it.
    let services_http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    state.set_artwork(artwork::ArtworkService::new(
        services_http.clone(),
        "https://resources.tidal.com/images".into(),
        config::default_cache_dir()
            .unwrap_or_default()
            .join("palettes.json"),
    ));
    state.set_lyrics(lyrics::LyricsService::new(
        services_http.clone(),
        "https://lrclib.net",
        config::default_cache_dir()
            .unwrap_or_default()
            .join("lyrics.json"),
    ));
    let scrobble_http = services_http;
    let (lastfm_key, lastfm_secret) = config.scrobbling.lastfm_credentials();
    let scrobbler = scrobble::ScrobblerService::new(
        scrobble_http,
        "https://ws.audioscrobbler.com/2.0/",
        "https://api.listenbrainz.org/1/",
        scrobble::credentials::select_credential_store(keyring_status.persistent),
        lastfm_key,
        lastfm_secret,
        config.scrobbling.lastfm_enabled,
        config.scrobbling.listenbrainz_enabled,
    );
    state.set_scrobbler(scrobbler.clone());
    perf::mark("services_ready");
    state.set_config(config);
    let track_flags = std::sync::Arc::new(track_flags::TrackFlagsStore::start(
        track_flags::track_flags_path(),
    ));
    state.set_track_flags(track_flags.clone());
    if let Some(path) = session::session_path() {
        let saver = std::sync::Arc::new(session::SessionSaver::start(
            path,
            session::SESSION_DEBOUNCE,
        ));
        saver.set_flags(track_flags);
        state.set_session_saver(saver);
    }
    if let Some(path) = history::history_path() {
        let config = state.config();
        state.set_history(std::sync::Arc::new(history::HistoryStore::start(
            path,
            config.history.max_entries,
            config.history.enabled,
        )));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkeys::handle)
                .build(),
        )
        .manage(state)
        .setup(|app| {
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
            match tray::build(app.handle()) {
                Ok(icon) => app.state::<AppState>().set_tray(icon),
                Err(e) => tracing::warn!("system tray unavailable: {e}"),
            }
            let config = app.state::<AppState>().config();
            if let Err(e) = hotkeys::register_all(app.handle(), &config.hotkeys) {
                tracing::warn!("global hotkeys unavailable: {e}");
            }
            if let Some(scrobbler) = app.state::<AppState>().scrobbler() {
                let handle = app.handle().clone();
                scrobbler.set_status_sink(std::sync::Arc::new(move |status| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://status", status);
                }));
                let handle = app.handle().clone();
                scrobbler.set_auth_error_sink(std::sync::Arc::new(move |service| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://auth-error", service);
                }));
                let handle = app.handle().clone();
                scrobbler.set_scrobbled_sink(std::sync::Arc::new(move |track_id| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://scrobbled", track_id);
                }));
                scrobbler.start();
            }
            if let Some(window) = app.get_webview_window("main") {
                // Before the first show, so the system frame never flashes.
                titlebar::install(&window);
                webview_idle::install(&window);
                thumbbar::install(&window);
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
            perf::mark("window_shown");
            // Background startup: cache cleanup and scheme registration are
            // not needed for the first frame, so they run after the window is
            // shown. A quit before this finishes simply skips the scheme
            // restore (no snapshot remembered).
            let bg = app.handle().clone();
            let cache = app.state::<AppState>().cache();
            let spawned = std::thread::Builder::new()
                .name("oxitide-startup".into())
                .spawn(move || {
                    if let Some(cache) = cache {
                        cache.cleanup();
                        perf::mark("cache_cleanup_done");
                    }
                    scheme::remember(scheme::snapshot());
                    #[cfg(any(windows, target_os = "linux"))]
                    if let Err(e) = bg.deep_link().register_all() {
                        tracing::warn!("deep link registration failed: {e}");
                    }
                    scheme::arm_restore();
                    perf::mark("startup_background_done");
                });
            if let Err(e) = spawned {
                tracing::warn!("startup background thread failed: {e}");
            }
            // Phone remote (Track 11): only when enabled, after the window is
            // shown, off the critical path.
            if app.state::<AppState>().config().remote.enabled {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = remote::host::start(&handle).await;
                });
            }
            updater::spawn_startup_check(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_begin,
            commands::auth_complete,
            commands::auth_take_result,
            commands::auth_restore,
            commands::auth_status,
            commands::auth_logout,
            commands::get_keyring_status,
            commands::get_home,
            commands::get_album,
            commands::get_album_tracks,
            commands::get_album_info,
            commands::get_track,
            commands::get_track_credits,
            commands::get_playlist,
            commands::get_playlist_tracks,
            commands::get_artist,
            commands::search,
            commands::get_explore,
            commands::get_page,
            commands::get_feed,
            commands::get_favorite_mixes,
            commands::get_mix,
            commands::get_for_you,
            commands::get_track_radio,
            commands::get_artist_radio,
            commands::get_similar_artists,
            commands::get_artist_profile,
            commands::get_artist_top_tracks,
            commands::get_artist_top_tracks_page,
            commands::get_artist_albums,
            commands::get_artist_albums_all,
            commands::get_artist_page,
            commands::open_external,
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
            commands::set_playlist_public,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_playlist_tracks,
            commands::remove_playlist_items,
            commands::move_playlist_items,
            commands::retry,
            commands::play_tracks,
            commands::play_track_ids,
            commands::play_page_items,
            commands::play_pause,
            commands::next,
            commands::prev,
            commands::seek,
            commands::set_volume,
            commands::fade_and_pause,
            commands::get_history_page,
            commands::get_stats,
            commands::clear_history,
            commands::set_history_enabled,
            commands::set_history_max_entries,
            commands::get_track_flags,
            commands::get_play_counts,
            commands::get_album_release_dates,
            commands::set_block_explicit,
            commands::set_block_ai,
            commands::set_show_labels,
            commands::set_shuffle,
            commands::set_repeat,
            commands::queue_reorder,
            commands::queue_remove,
            commands::queue_add,
            commands::queue_play_next,
            commands::queue_clear,
            commands::apply_content_filters,
            commands::get_playback_state,
            commands::get_restored_session,
            commands::restore_session,
            commands::set_play_source,
            commands::clear_session,
            commands::get_config,
            commands::get_output_devices,
            commands::set_output_device,
            commands::set_preferred_quality,
            commands::set_exclusive,
            commands::set_exclusive_fallback,
            commands::set_dsp_settings,
            commands::automix_render,
            commands::get_audio_status,
            commands::get_cache_status,
            commands::clear_cache,
            commands::set_allow_fallback,
            commands::set_quality_display,
            commands::set_ui_mode,
            commands::set_theme,
            commands::set_typeface,
            commands::set_custom_typeface,
            commands::set_density,
            commands::set_cover_shape,
            commands::set_dynamic_color,
            commands::set_rack_settings,
            commands::set_level_meter,
            commands::set_queue_width,
            commands::set_mini_mode,
            commands::set_start_in_mini,
            commands::set_oneko,
            commands::set_hover_panels,
            commands::set_smtc,
            commands::set_discord,
            commands::set_close_to_tray,
            commands::get_update_support,
            commands::check_for_update,
            commands::install_update,
            commands::set_update_check_on_start,
            commands::set_hotkey,
            commands::set_hotkey_capture,
            commands::set_cache_max_size_mb,
            commands::set_cache_clear_on_exit,
            commands::get_custom_titlebar,
            commands::set_snap_target,
            commands::get_logs_dir,
            commands::open_logs_folder,
            commands::perf_mark,
            commands::perf_quit,
            commands::get_perf_snapshot,
            commands::perf_submit_ui_metrics,
            commands::perf_queue_build,
            commands::analyze_cover,
            commands::get_remote_status,
            commands::set_remote_enabled,
            commands::set_remote_settings,
            commands::begin_remote_pairing,
            commands::cancel_remote_pairing,
            commands::list_remote_devices,
            commands::rename_remote_device,
            commands::revoke_remote_device,
            commands::resolve_remote_pair_request,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OxiTide")
        .run(|app_handle, event| match event {
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
            tauri::RunEvent::Exit => run_exit_work(app_handle),
            _ => {}
        });
}

/// Shutdown persistence. Runs on `RunEvent::Exit`, and before a Windows update
/// install, whose installer ends the process without that event.
fn run_exit_work(app_handle: &tauri::AppHandle) {
    scheme::restore_remembered();
    remote::host::stop_now(app_handle);
    let state = app_handle.state::<AppState>();
    if let Some(saver) = state.session_saver() {
        saver.flush_now();
    }
    if let Some(history) = state.history() {
        history.finalize_now();
    }
    let config = state.config();
    if config.cache.clear_on_exit {
        if let Some(cache) = state.cache() {
            cache.clear();
        }
    }
}
