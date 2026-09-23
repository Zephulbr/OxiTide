#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod player;
mod state;

use tauri::Manager;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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
