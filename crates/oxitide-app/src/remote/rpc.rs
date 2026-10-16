//! The remote's RPC surface (spec §6.3, RM-D5–RM-D7). `parse` turns a wire
//! call into a typed `RemoteCommand` and is the allowlist: a method it does
//! not know is unreachable. `execute` runs a command through the existing
//! `commands::*` functions, so filters and session handling apply unchanged.

use oxitide_audio::queue::{QueueItem, RepeatMode};
use oxitide_tidal::models::ArtistDiscography;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::commands::{self, PageTrackDto};
use crate::state::AppState;

/// Largest page a paged method may request.
pub const MAX_PAGE_LIMIT: u32 = 200;
/// Most track ids / queue items one playback or queue call may carry.
pub const MAX_PLAY_ITEMS: usize = 5000;
/// Responses above this size are refused (use the paged methods).
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Every method a paired phone may call; `parse` refuses anything else.
/// `get_queue_page`, `queue_jump`, and `get_remote_state` are remote-only
/// (no desktop command of that name).
pub const ALLOWLIST: &[&str] = &[
    // Transport
    "play_pause",
    "next",
    "prev",
    "seek",
    "set_volume",
    "set_shuffle",
    "set_repeat",
    "fade_and_pause",
    "retry",
    // Playback start
    "play_track_ids",
    "play_page_items",
    // Queue
    "queue_add",
    "queue_play_next",
    "queue_remove",
    "queue_reorder",
    "queue_clear",
    "get_queue_page",
    "queue_jump",
    // State
    "get_remote_state",
    // Discovery
    "search",
    "get_home",
    "get_explore",
    "get_for_you",
    "get_favorite_mixes",
    "get_mix",
    "get_track_radio",
    "get_artist_radio",
    "get_similar_artists",
    // Catalogue
    "get_album",
    "get_album_tracks",
    "get_album_info",
    "get_playlist",
    "get_playlist_tracks_page",
    "get_artist",
    "get_artist_page",
    "get_artist_top_tracks_page",
    "get_artist_albums_all",
    "get_track",
    "get_lyrics",
    "analyze_cover",
    // Library
    "get_favorite_tracks_page",
    "get_favorite_albums",
    "get_favorite_artists",
    "get_favorite_ids",
    "get_user_playlists",
    "get_collection_items",
    "get_history_page",
    // Writes
    "set_favorite_track",
    "set_favorite_album",
    "set_favorite_artist",
    "add_playlist_tracks",
];

/// The session seam's command type (RM-D12): one variant per allowlisted
/// method, parameters already validated.
#[derive(Debug, Clone)]
pub enum RemoteCommand {
    PlayPause,
    Next,
    Prev,
    Seek { position_ms: u64 },
    SetVolume { volume: f32 },
    SetShuffle { enabled: bool },
    SetRepeat { mode: RepeatMode },
    FadeAndPause { duration_ms: u64 },
    Retry,
    PlayTrackIds { ids: Vec<u64>, start: usize, shuffle: Option<bool> },
    PlayPageItems { items: Vec<PageTrackDto>, start: usize, shuffle: Option<bool> },
    QueueAdd { items: Vec<QueueItem> },
    QueuePlayNext { items: Vec<QueueItem> },
    QueueRemove { index: usize },
    QueueReorder { from: usize, to: usize },
    QueueClear,
    GetQueuePage { offset: usize, limit: usize },
    QueueJump { index: usize },
    GetRemoteState,
    Search { query: String },
    GetHome,
    GetExplore,
    GetForYou,
    GetFavoriteMixes,
    GetMix { id: String },
    GetTrackRadio { track_id: u64 },
    GetArtistRadio { artist_id: u64 },
    GetSimilarArtists { artist_id: u64 },
    GetAlbum { id: u64 },
    GetAlbumTracks { id: u64 },
    GetAlbumInfo { id: u64 },
    GetPlaylist { id: String },
    GetPlaylistTracksPage { id: String, offset: u32, limit: u32 },
    GetArtist { id: u64 },
    GetArtistPage { artist_id: u64 },
    GetArtistTopTracksPage { artist_id: u64, limit: u32, offset: u32 },
    GetArtistAlbumsAll { artist_id: u64, filter: ArtistDiscography },
    GetTrack { id: u64 },
    GetLyrics { track_id: u64, title: String, artist: String, album: Option<String>, duration_secs: u64 },
    AnalyzeCover { uuid: String, theme: crate::palette::Room },
    GetFavoriteTracksPage { offset: u32, limit: u32 },
    GetFavoriteAlbums,
    GetFavoriteArtists,
    GetFavoriteIds,
    GetUserPlaylists,
    GetCollectionItems { folder_id: Option<String>, order: Option<String> },
    GetHistoryPage { offset: usize, limit: usize },
    SetFavoriteTrack { track_id: u64, favorite: bool },
    SetFavoriteAlbum { album_id: u64, favorite: bool },
    SetFavoriteArtist { artist_id: u64, favorite: bool },
    AddPlaylistTracks { id: String, track_ids: Vec<u64>, allow_duplicates: bool },
}

/// A failed call, sent as `"<code>: <message>"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: &'static str,
    pub message: String,
}

impl RpcError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    fn bad_params(message: impl Into<String>) -> Self {
        Self::new("bad_params", message)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// `POST /api/rpc` body.
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// `POST /api/rpc` response.
#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn ok(result: Value) -> Self {
        Self { ok: true, result: Some(result), error: None }
    }

    pub fn err(error: &RpcError) -> Self {
        Self { ok: false, result: None, error: Some(error.to_string()) }
    }
}

fn from_params<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    let params = if params.is_null() { Value::Object(Default::default()) } else { params };
    serde_json::from_value(params).map_err(|e| RpcError::bad_params(e.to_string()))
}

/// Deserialize camelCase params into the named snake_case bindings.
macro_rules! args {
    ($params:expr, { $($field:ident : $ty:ty),+ $(,)? }) => {{
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args { $($field: $ty),+ }
        let Args { $($field),+ } = from_params::<Args>($params)?;
        ($($field),+)
    }};
}

fn page_limit<T: Into<u64> + Copy>(limit: T) -> Result<T, RpcError> {
    if limit.into() > MAX_PAGE_LIMIT as u64 {
        return Err(RpcError::bad_params(format!("limit must be at most {MAX_PAGE_LIMIT}")));
    }
    Ok(limit)
}

fn bounded<T>(items: Vec<T>) -> Result<Vec<T>, RpcError> {
    if items.len() > MAX_PLAY_ITEMS {
        return Err(RpcError::bad_params(format!("at most {MAX_PLAY_ITEMS} items per call")));
    }
    Ok(items)
}

/// A TIDAL image id (dashed UUID): the only thing the phone may ask the PC to
/// fetch and analyse, so it cannot steer the request elsewhere.
fn is_cover_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid
            .chars()
            .enumerate()
            .all(|(i, ch)| if [8, 13, 18, 23].contains(&i) { ch == '-' } else { ch.is_ascii_hexdigit() })
}

/// Covers a phone sends ride into the desktop queue, the media overlay (which
/// fetches them), Discord presence (which publishes them), and history. Keep
/// only TIDAL image ids and TIDAL CDN URLs; anything else is dropped.
fn trusted_cover(cover: Option<String>) -> Option<String> {
    cover.filter(|c| is_cover_uuid(c) || c.starts_with("https://resources.tidal.com/"))
}

/// The allowlist: map a wire call to a command or refuse it.
pub fn parse(method: &str, params: Value) -> Result<RemoteCommand, RpcError> {
    use RemoteCommand as C;
    if !ALLOWLIST.contains(&method) {
        return Err(RpcError::new("unknown_method", method.to_string()));
    }
    Ok(match method {
        "play_pause" => C::PlayPause,
        "next" => C::Next,
        "prev" => C::Prev,
        "seek" => C::Seek { position_ms: args!(params, { position_ms: u64 }) },
        "set_volume" => {
            let volume = args!(params, { volume: f32 });
            if !volume.is_finite() {
                return Err(RpcError::bad_params("volume must be a number"));
            }
            C::SetVolume { volume: volume.clamp(0.0, 1.0) }
        }
        "set_shuffle" => C::SetShuffle { enabled: args!(params, { enabled: bool }) },
        "set_repeat" => C::SetRepeat { mode: args!(params, { mode: RepeatMode }) },
        "fade_and_pause" => C::FadeAndPause { duration_ms: args!(params, { duration_ms: u64 }) },
        "retry" => C::Retry,
        "play_track_ids" => {
            let (ids, start, shuffle) =
                args!(params, { ids: Vec<u64>, start: usize, shuffle: Option<bool> });
            C::PlayTrackIds { ids: bounded(ids)?, start, shuffle }
        }
        "play_page_items" => {
            let (items, start, shuffle) =
                args!(params, { items: Vec<PageTrackDto>, start: usize, shuffle: Option<bool> });
            let items = bounded(items)?
                .into_iter()
                .map(|item| PageTrackDto { cover: trusted_cover(item.cover), ..item })
                .collect();
            C::PlayPageItems { items, start, shuffle }
        }
        "queue_add" | "queue_play_next" => {
            let items = bounded(args!(params, { items: Vec<QueueItem> }))?
                .into_iter()
                .map(|item| QueueItem { cover: trusted_cover(item.cover), ..item })
                .collect();
            if method == "queue_add" {
                C::QueueAdd { items }
            } else {
                C::QueuePlayNext { items }
            }
        }
        "queue_remove" => C::QueueRemove { index: args!(params, { index: usize }) },
        "queue_reorder" => {
            let (from, to) = args!(params, { from: usize, to: usize });
            C::QueueReorder { from, to }
        }
        "queue_clear" => C::QueueClear,
        "get_queue_page" => {
            let (offset, limit) = args!(params, { offset: usize, limit: usize });
            C::GetQueuePage { offset, limit: page_limit(limit as u64)? as usize }
        }
        "queue_jump" => C::QueueJump { index: args!(params, { index: usize }) },
        "get_remote_state" => C::GetRemoteState,
        "search" => C::Search { query: args!(params, { query: String }) },
        "get_home" => C::GetHome,
        "get_explore" => C::GetExplore,
        "get_for_you" => C::GetForYou,
        "get_favorite_mixes" => C::GetFavoriteMixes,
        "get_mix" => C::GetMix { id: args!(params, { id: String }) },
        "get_track_radio" => C::GetTrackRadio { track_id: args!(params, { track_id: u64 }) },
        "get_artist_radio" => C::GetArtistRadio { artist_id: args!(params, { artist_id: u64 }) },
        "get_similar_artists" => {
            C::GetSimilarArtists { artist_id: args!(params, { artist_id: u64 }) }
        }
        "get_album" => C::GetAlbum { id: args!(params, { id: u64 }) },
        "get_album_tracks" => C::GetAlbumTracks { id: args!(params, { id: u64 }) },
        "get_album_info" => C::GetAlbumInfo { id: args!(params, { id: u64 }) },
        "get_playlist" => C::GetPlaylist { id: args!(params, { id: String }) },
        "get_playlist_tracks_page" => {
            let (id, offset, limit) = args!(params, { id: String, offset: u32, limit: u32 });
            C::GetPlaylistTracksPage { id, offset, limit: page_limit(limit)? }
        }
        "get_artist" => C::GetArtist { id: args!(params, { id: u64 }) },
        "get_artist_page" => C::GetArtistPage { artist_id: args!(params, { artist_id: u64 }) },
        "get_artist_top_tracks_page" => {
            let (artist_id, limit, offset) =
                args!(params, { artist_id: u64, limit: u32, offset: u32 });
            C::GetArtistTopTracksPage { artist_id, limit: page_limit(limit)?, offset }
        }
        "get_artist_albums_all" => {
            let (artist_id, filter) = args!(params, { artist_id: u64, filter: ArtistDiscography });
            C::GetArtistAlbumsAll { artist_id, filter }
        }
        "get_track" => C::GetTrack { id: args!(params, { id: u64 }) },
        "get_lyrics" => {
            let (track_id, title, artist, album, duration_secs) = args!(params, {
                track_id: u64, title: String, artist: String, album: Option<String>, duration_secs: u64,
            });
            C::GetLyrics { track_id, title, artist, album, duration_secs }
        }
        "analyze_cover" => {
            let (uuid, theme) = args!(params, { uuid: String, theme: crate::palette::Room });
            if !is_cover_uuid(&uuid) {
                return Err(RpcError::bad_params("not a cover id"));
            }
            C::AnalyzeCover { uuid, theme }
        }
        "get_favorite_tracks_page" => {
            let (offset, limit) = args!(params, { offset: u32, limit: u32 });
            C::GetFavoriteTracksPage { offset, limit: page_limit(limit)? }
        }
        "get_favorite_albums" => C::GetFavoriteAlbums,
        "get_favorite_artists" => C::GetFavoriteArtists,
        "get_favorite_ids" => C::GetFavoriteIds,
        "get_user_playlists" => C::GetUserPlaylists,
        "get_collection_items" => {
            let (folder_id, order) = args!(params, { folder_id: Option<String>, order: Option<String> });
            C::GetCollectionItems { folder_id, order }
        }
        "get_history_page" => {
            let (offset, limit) = args!(params, { offset: usize, limit: usize });
            C::GetHistoryPage { offset, limit: page_limit(limit as u64)? as usize }
        }
        "set_favorite_track" => {
            let (track_id, favorite) = args!(params, { track_id: u64, favorite: bool });
            C::SetFavoriteTrack { track_id, favorite }
        }
        "set_favorite_album" => {
            let (album_id, favorite) = args!(params, { album_id: u64, favorite: bool });
            C::SetFavoriteAlbum { album_id, favorite }
        }
        "set_favorite_artist" => {
            let (artist_id, favorite) = args!(params, { artist_id: u64, favorite: bool });
            C::SetFavoriteArtist { artist_id, favorite }
        }
        "add_playlist_tracks" => {
            let (id, track_ids, allow_duplicates) =
                args!(params, { id: String, track_ids: Vec<u64>, allow_duplicates: bool });
            C::AddPlaylistTracks { id, track_ids: bounded(track_ids)?, allow_duplicates }
        }
        _ => return Err(RpcError::new("unknown_method", method.to_string())),
    })
}

fn json<T: Serialize>(result: Result<T, String>) -> Result<Value, String> {
    result.and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()))
}

/// Run a command against the desktop app. `GetRemoteState` and
/// `GetQueuePage` are answered by the event hub and never reach here.
pub async fn execute<R: Runtime>(app: &AppHandle<R>, command: RemoteCommand) -> Result<Value, String> {
    use RemoteCommand as C;
    let state = app.state::<AppState>();
    match command {
        C::PlayPause => json(commands::play_pause(state)),
        C::Next => json(commands::next(state)),
        C::Prev => json(commands::prev(state)),
        C::Seek { position_ms } => json(commands::seek(state, position_ms)),
        // The phone's volume is live only; it never rewrites config.toml.
        C::SetVolume { volume } => json(commands::set_volume(state, volume, false)),
        C::SetShuffle { enabled } => json(commands::set_shuffle(state, enabled)),
        C::SetRepeat { mode } => json(commands::set_repeat(state, mode)),
        C::FadeAndPause { duration_ms } => json(commands::fade_and_pause(state, duration_ms)),
        C::Retry => json(commands::retry(state)),
        C::PlayTrackIds { ids, start, shuffle } => {
            json(commands::play_track_ids(state, ids, start, shuffle).await)
        }
        C::PlayPageItems { items, start, shuffle } => {
            json(commands::play_page_items(state, items, start, shuffle).await)
        }
        C::QueueAdd { items } => json(commands::queue_add(state, items)),
        C::QueuePlayNext { items } => json(commands::queue_play_next(state, items)),
        C::QueueRemove { index } => json(commands::queue_remove(state, index)),
        C::QueueReorder { from, to } => json(commands::queue_reorder(state, from, to)),
        C::QueueClear => json(commands::queue_clear(state)),
        C::QueueJump { index } => {
            // The desktop rail's jump (`play_tracks(queue, index)`), with the
            // queue taken from the engine so the phone never uploads items.
            let player = state.player().ok_or_else(|| "not logged in".to_string())?;
            let queue = player.engine.state().queue;
            if index >= queue.len() {
                return Err("queue index out of range".to_string());
            }
            json(commands::play_tracks(state, queue, index, None))
        }
        C::GetQueuePage { .. } | C::GetRemoteState => {
            Err("answered by the event hub".to_string())
        }
        C::Search { query } => json(commands::search(state, query).await),
        C::GetHome => json(commands::get_home(state).await),
        C::GetExplore => json(commands::get_explore(state).await),
        C::GetForYou => json(commands::get_for_you(state).await),
        C::GetFavoriteMixes => json(commands::get_favorite_mixes(state).await),
        C::GetMix { id } => json(commands::get_mix(state, id).await),
        C::GetTrackRadio { track_id } => json(commands::get_track_radio(state, track_id).await),
        C::GetArtistRadio { artist_id } => json(commands::get_artist_radio(state, artist_id).await),
        C::GetSimilarArtists { artist_id } => {
            json(commands::get_similar_artists(state, artist_id).await)
        }
        C::GetAlbum { id } => json(commands::get_album(state, id).await),
        C::GetAlbumTracks { id } => json(commands::get_album_tracks(state, id).await),
        C::GetAlbumInfo { id } => json(commands::get_album_info(state, id).await),
        C::GetPlaylist { id } => json(commands::get_playlist(state, id).await),
        C::GetPlaylistTracksPage { id, offset, limit } => {
            json(commands::get_playlist_tracks_page(state, id, offset, limit).await)
        }
        C::GetArtist { id } => json(commands::get_artist(state, id).await),
        C::GetArtistPage { artist_id } => json(commands::get_artist_page(state, artist_id).await),
        C::GetArtistTopTracksPage { artist_id, limit, offset } => {
            json(commands::get_artist_top_tracks_page(state, artist_id, limit, offset).await)
        }
        C::GetArtistAlbumsAll { artist_id, filter } => {
            json(commands::get_artist_albums_all(state, artist_id, filter).await)
        }
        C::GetTrack { id } => json(commands::get_track(state, id).await),
        C::GetLyrics { track_id, title, artist, album, duration_secs } => json(
            commands::get_lyrics(state, track_id, title, artist, album, duration_secs).await,
        ),
        C::AnalyzeCover { uuid, theme } => json(commands::analyze_cover(state, uuid, theme).await),
        C::GetFavoriteTracksPage { offset, limit } => {
            json(commands::get_favorite_tracks_page(state, offset, limit).await)
        }
        C::GetFavoriteAlbums => json(commands::get_favorite_albums(state).await),
        C::GetFavoriteArtists => json(commands::get_favorite_artists(state).await),
        C::GetFavoriteIds => json(commands::get_favorite_ids(state).await),
        C::GetUserPlaylists => json(commands::get_user_playlists(state).await),
        C::GetCollectionItems { folder_id, order } => {
            json(commands::get_collection_items(state, folder_id, order).await)
        }
        C::GetHistoryPage { offset, limit } => json(commands::get_history_page(state, offset, limit)),
        C::SetFavoriteTrack { track_id, favorite } => {
            json(commands::set_favorite_track(state, track_id, favorite).await)
        }
        C::SetFavoriteAlbum { album_id, favorite } => {
            json(commands::set_favorite_album(state, album_id, favorite).await)
        }
        C::SetFavoriteArtist { artist_id, favorite } => {
            json(commands::set_favorite_artist(state, artist_id, favorite).await)
        }
        C::AddPlaylistTracks { id, track_ids, allow_duplicates } => {
            json(commands::add_playlist_tracks(state, id, track_ids, allow_duplicates).await)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Allowlisted methods with no `#[tauri::command]` of the same name.
    const REMOTE_ONLY: &[&str] = &["get_queue_page", "queue_jump", "get_remote_state"];

    /// Desktop commands deliberately unreachable from a phone (spec §6.3).
    /// A new `generate_handler!` entry must be added here or to `ALLOWLIST`,
    /// so exposure is always a reviewed change.
    const NOT_EXPOSED: &[&str] = &[
        "auth_begin",
        "auth_complete",
        "auth_take_result",
        "auth_restore",
        "auth_status",
        "auth_logout",
        "get_keyring_status",
        "get_track_credits",
        "get_playlist_tracks",
        "get_feed",
        "get_page",
        "get_artist_profile",
        "get_artist_top_tracks",
        "get_artist_albums",
        "open_external",
        "get_custom_titlebar",
        "set_snap_target",
        "get_scrobble_status",
        "lastfm_begin_auth",
        "lastfm_complete_auth",
        "lastfm_cancel_auth",
        "lastfm_disconnect",
        "listenbrainz_connect",
        "listenbrainz_disconnect",
        "set_scrobble_enabled",
        "create_folder",
        "rename_folder",
        "remove_collection_entry",
        "move_playlists_to_folder",
        "get_purchases",
        "get_favorite_tracks",
        "create_playlist",
        "set_playlist_public",
        "rename_playlist",
        "delete_playlist",
        "remove_playlist_items",
        "move_playlist_items",
        "play_tracks",
        "get_stats",
        "clear_history",
        "set_history_enabled",
        "set_history_max_entries",
        "get_track_flags",
        "set_block_explicit",
        "set_block_ai",
        "set_show_labels",
        "apply_content_filters",
        "get_playback_state",
        "get_restored_session",
        "restore_session",
        "set_play_source",
        "clear_session",
        "get_config",
        "get_output_devices",
        "set_output_device",
        "set_preferred_quality",
        "set_exclusive",
        "set_exclusive_fallback",
        "set_dsp_settings",
        // AutoMix replay lab: desktop A/B tool returning WAV bytes.
        "automix_render",
        "get_audio_status",
        "get_cache_status",
        "clear_cache",
        "set_allow_fallback",
        "set_quality_display",
        "set_ui_mode",
        "set_theme",
        "set_typeface",
        "set_custom_typeface",
        "set_density",
        "set_cover_shape",
        "set_dynamic_color",
        "set_rack_settings",
        "set_level_meter",
        "set_queue_width",
        "set_mini_mode",
        "set_start_in_mini",
        "set_oneko",
        "set_smtc",
        "set_discord",
        "set_close_to_tray",
        "set_hotkey",
        "set_hotkey_capture",
        "set_cache_max_size_mb",
        "set_cache_clear_on_exit",
        "get_logs_dir",
        "open_logs_folder",
        "perf_mark",
        "perf_quit",
        "get_perf_snapshot",
        "perf_submit_ui_metrics",
        "perf_queue_build",
        "get_remote_status",
        "set_remote_enabled",
        "set_remote_settings",
        "begin_remote_pairing",
        "cancel_remote_pairing",
        "list_remote_devices",
        "revoke_remote_device",
        "resolve_remote_pair_request",
        "rename_remote_device",
        "get_update_support",
        "check_for_update",
        "install_update",
        "set_update_check_on_start",
    ];

    /// The command names inside `generate_handler![...]` in `main.rs`.
    fn handler_commands() -> Vec<String> {
        let main = include_str!("../main.rs");
        let start = main.find("generate_handler![").expect("generate_handler! in main.rs");
        let body = &main[start..];
        let end = body.find(']').unwrap();
        body[..end]
            .split(',')
            .filter_map(|entry| entry.trim().strip_prefix("commands::"))
            .map(|name| name.trim().to_string())
            .collect()
    }

    /// Valid params for every allowlisted method.
    fn sample_params(method: &str) -> Value {
        match method {
            "seek" => json!({"positionMs": 1000}),
            "set_volume" => json!({"volume": 0.5}),
            "set_shuffle" => json!({"enabled": true}),
            "set_repeat" => json!({"mode": "all"}),
            "fade_and_pause" => json!({"durationMs": 3000}),
            "play_track_ids" => json!({"ids": [1, 2], "start": 0}),
            "play_page_items" => json!({"items": [{"id": 1}], "start": 0, "shuffle": true}),
            "queue_add" | "queue_play_next" => json!({"items": [{"track_id": 1, "title": "T"}]}),
            "queue_remove" | "queue_jump" => json!({"index": 2}),
            "queue_reorder" => json!({"from": 1, "to": 3}),
            "get_queue_page" | "get_history_page" | "get_favorite_tracks_page" => {
                json!({"offset": 0, "limit": 50})
            }
            "search" => json!({"query": "x"}),
            "get_mix" | "get_playlist" => json!({"id": "abc"}),
            "get_track_radio" => json!({"trackId": 1}),
            "get_artist_radio" | "get_similar_artists" | "get_artist_page" => json!({"artistId": 1}),
            "get_album" | "get_album_tracks" | "get_album_info" | "get_track" | "get_artist" => {
                json!({"id": 7})
            }
            "get_playlist_tracks_page" => json!({"id": "abc", "offset": 0, "limit": 100}),
            "get_artist_top_tracks_page" => json!({"artistId": 1, "limit": 20, "offset": 0}),
            "get_artist_albums_all" => json!({"artistId": 1, "filter": "eps_and_singles"}),
            "get_lyrics" => json!({
                "trackId": 1, "title": "T", "artist": "A", "album": null, "durationSecs": 200
            }),
            "get_collection_items" => json!({"folderId": "root"}),
            "analyze_cover" => json!({"uuid": "660f113b-e63f-4c2b-a021-33350ca6948b", "theme": "dark"}),
            "set_favorite_track" => json!({"trackId": 1, "favorite": true}),
            "set_favorite_album" => json!({"albumId": 1, "favorite": false}),
            "set_favorite_artist" => json!({"artistId": 1, "favorite": true}),
            "add_playlist_tracks" => json!({"id": "abc", "trackIds": [1], "allowDuplicates": false}),
            _ => Value::Null,
        }
    }

    #[test]
    fn every_allowlisted_method_parses() {
        for method in ALLOWLIST {
            let parsed = parse(method, sample_params(method));
            assert!(parsed.is_ok(), "{method}: {:?}", parsed.err());
        }
    }

    #[test]
    fn unknown_and_excluded_methods_are_denied() {
        for method in NOT_EXPOSED.iter().chain(["", "eval", "PLAY_PAUSE"].iter()) {
            let err = parse(method, json!({})).unwrap_err();
            assert_eq!(err.code, "unknown_method", "{method}");
        }
    }

    #[test]
    fn every_registered_command_is_classified() {
        let handlers = handler_commands();
        assert!(handlers.len() > 100, "parsed {} commands", handlers.len());
        for name in &handlers {
            let allowed = ALLOWLIST.contains(&name.as_str());
            let excluded = NOT_EXPOSED.contains(&name.as_str());
            assert!(
                allowed ^ excluded,
                "`{name}` must be in exactly one of ALLOWLIST / NOT_EXPOSED (new commands default to denied)"
            );
        }
        for name in ALLOWLIST.iter().filter(|m| !REMOTE_ONLY.contains(m)) {
            assert!(handlers.iter().any(|h| h == name), "`{name}` is not a registered command");
        }
        for name in REMOTE_ONLY {
            assert!(!handlers.iter().any(|h| h == name), "`{name}` must stay remote-only");
        }
    }

    #[test]
    fn params_map_from_camel_case() {
        match parse("get_lyrics", sample_params("get_lyrics")).unwrap() {
            RemoteCommand::GetLyrics { track_id, duration_secs, album, .. } => {
                assert_eq!((track_id, duration_secs, album), (1, 200, None));
            }
            other => panic!("unexpected {other:?}"),
        }
        let err = parse("seek", json!({"position_ms": 5})).unwrap_err();
        assert_eq!(err.code, "bad_params", "snake_case params are not the wire format");
        assert_eq!(parse("seek", Value::Null).unwrap_err().code, "bad_params");
    }

    #[test]
    fn limits_are_capped() {
        for method in ["get_queue_page", "get_history_page", "get_favorite_tracks_page"] {
            assert!(parse(method, json!({"offset": 0, "limit": 200})).is_ok());
            let err = parse(method, json!({"offset": 0, "limit": 201})).unwrap_err();
            assert_eq!(err.code, "bad_params", "{method}");
        }
        let ids: Vec<u64> = (0..5001).collect();
        let err = parse("play_track_ids", json!({"ids": ids, "start": 0})).unwrap_err();
        assert_eq!(err.code, "bad_params");
        let ids: Vec<u64> = (0..5000).collect();
        assert!(parse("play_track_ids", json!({"ids": ids, "start": 0})).is_ok());
    }

    #[test]
    fn cover_analysis_takes_only_tidal_image_ids() {
        for bad in [
            "",
            "../../etc/passwd",
            "http://evil.example/x.jpg",
            "660f113b-e63f-4c2b-a021-33350ca6948",
            "660f113b/e63f/4c2b/a021/33350ca6948b",
        ] {
            let err = parse("analyze_cover", json!({"uuid": bad, "theme": "dark"})).unwrap_err();
            assert_eq!(err.code, "bad_params", "{bad}");
        }
        let sepia = json!({"uuid": "660f113b-e63f-4c2b-a021-33350ca6948b", "theme": "sepia"});
        assert!(parse("analyze_cover", sepia).is_err());
    }

    #[test]
    fn phone_supplied_covers_are_limited_to_tidal_images() {
        let uuid = "660f113b-e63f-4c2b-a021-33350ca6948b";
        let cdn = "https://resources.tidal.com/images/a/b/320x320.jpg";
        let params = json!({"items": [
            {"track_id": 1, "title": "a", "cover": uuid},
            {"track_id": 2, "title": "b", "cover": cdn},
            {"track_id": 3, "title": "c", "cover": "http://192.168.1.1/admin"},
            {"track_id": 4, "title": "d", "cover": "https://evil.example/x.png"},
        ]});
        match parse("queue_add", params).unwrap() {
            RemoteCommand::QueueAdd { items } => {
                let covers: Vec<_> = items.iter().map(|i| i.cover.as_deref()).collect();
                assert_eq!(covers, vec![Some(uuid), Some(cdn), None, None]);
            }
            other => panic!("unexpected {other:?}"),
        }
        let params = json!({"items": [{"id": 1, "cover": "http://evil.example/x"}], "start": 0});
        match parse("play_page_items", params).unwrap() {
            RemoteCommand::PlayPageItems { items, .. } => assert_eq!(items[0].cover, None),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn volume_is_clamped_and_never_persisted() {
        match parse("set_volume", json!({"volume": 3.0})).unwrap() {
            RemoteCommand::SetVolume { volume } => assert_eq!(volume, 1.0),
            other => panic!("unexpected {other:?}"),
        }
        assert!(parse("set_volume", json!({"volume": "loud"})).is_err());
    }

    #[test]
    fn rpc_error_wire_format() {
        let error = RpcError::new("not_logged_in", "sign in on the PC");
        let body = serde_json::to_value(RpcResponse::err(&error)).unwrap();
        assert_eq!(body, json!({"ok": false, "error": "not_logged_in: sign in on the PC"}));
        let body = serde_json::to_value(RpcResponse::ok(json!([1]))).unwrap();
        assert_eq!(body, json!({"ok": true, "result": [1]}));
    }
}
