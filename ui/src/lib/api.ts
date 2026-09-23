import { invoke } from "@tauri-apps/api/core";

export interface ArtistRef { id: number; name: string }
export interface AlbumRef { id: number; title: string; cover?: string | null }
export interface Track {
  id: number; title: string; duration: number;
  artists: ArtistRef[]; album?: AlbumRef | null;
  audioQuality?: string | null; trackNumber: number; volumeNumber: number;
  explicit: boolean;
  mediaMetadata?: { tags: string[] } | null;
}
export interface Album {
  id: number; title: string; artists: ArtistRef[]; cover?: string | null;
  numberOfTracks: number; duration: number; releaseDate?: string | null;
  audioQuality?: string | null;
}
export interface Artist { id: number; name: string; picture?: string | null }
export interface Playlist {
  uuid: string; title: string; description?: string | null;
  numberOfTracks: number; duration: number; image?: string | null;
}
export interface HomeItem { id: string; title: string; subtitle?: string | null; item_type: string }
export interface HomeSection { title: string; items: HomeItem[] }
export interface SearchGroup<T> { items: T[] }
export interface SearchResults {
  tracks: SearchGroup<Track>; albums: SearchGroup<Album>;
  artists: SearchGroup<Artist>; playlists: SearchGroup<Playlist>;
}
export interface SessionInfoDto { country_code: string; user_id: number }

export interface QueueItem {
  track_id: number; title: string; artist: string; album: string;
  duration_secs: number; cover?: string | null;
}
export type RepeatMode = "off" | "all" | "one";
export type Quality = "low" | "high" | "lossless" | "max";
export interface StreamMeta {
  quality?: Quality | null;
  codec?: string | null;
  sample_rate: number;
  bit_depth: number;
  channels: number;
  bitrate_kbps?: number | null;
}
export interface PlaybackState {
  playing: boolean; buffering: boolean; current?: QueueItem | null;
  position_ms: number; duration_ms: number; volume: number;
  shuffle: boolean; repeat: RepeatMode;
  preferred_quality: Quality;
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  bit_perfect_notice?: string | null;
  queue: QueueItem[];
  stream?: StreamMeta | null;
}
export interface OutputDevice { id: string; name: string; is_default: boolean }
export interface AudioStatus {
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  notice?: string | null;
}
export interface CacheStatus {
  dir: string;
  used_bytes: number;
  pending_bytes: number;
  max_bytes: number;
  usable: boolean;
  configured_dir: string;
}
export interface Config {
  audio: {
    output_device: string;
    exclusive: boolean;
    exclusive_fallback: "shared" | "warn_only";
    volume: number;
  };
  streaming: { quality: Quality; allow_fallback: boolean };
  ui: { quality_display: "compact" | "detailed" };
  cache: { dir: string; max_size_mb: number; clear_on_exit: boolean };
}
export type EngineEvent =
  | ({ type: "state" } & PlaybackState)
  | { type: "now_playing"; [k: string]: unknown }
  | { type: "position"; position_ms: number; duration_ms: number }
  | { type: "error"; code: string; message: string };

export function trackToQueueItem(t: Track): QueueItem {
  return {
    track_id: t.id,
    title: t.title,
    artist: t.artists.map((a) => a.name).join(", "),
    album: t.album?.title ?? "",
    duration_secs: t.duration,
    cover: t.album?.cover ?? null,
  };
}

export const api = {
  authBegin: () => invoke<{ url: string }>("auth_begin"),
  authComplete: (redirectUrl: string) => invoke<SessionInfoDto>("auth_complete", { redirectUrl }),
  authStatus: () => invoke<boolean>("auth_status"),
  authRestore: () => invoke<SessionInfoDto | null>("auth_restore"),
  authLogout: () => invoke<void>("auth_logout"),
  getHome: () => invoke<HomeSection[]>("get_home"),
  getAlbum: (id: number) => invoke<Album>("get_album", { id }),
  getAlbumTracks: (id: number) => invoke<Track[]>("get_album_tracks", { id }),
  getPlaylist: (id: string) => invoke<Playlist>("get_playlist", { id }),
  getPlaylistTracks: (id: string) => invoke<Track[]>("get_playlist_tracks", { id }),
  getArtist: (id: number) => invoke<Artist>("get_artist", { id }),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  getFavoriteTracks: () => invoke<Track[]>("get_favorite_tracks"),
  getFavoriteAlbums: () => invoke<Album[]>("get_favorite_albums"),
  getFavoriteArtists: () => invoke<Artist[]>("get_favorite_artists"),
  getUserPlaylists: () => invoke<Playlist[]>("get_user_playlists"),
  playTracks: (items: QueueItem[], start: number) => invoke<void>("play_tracks", { items, start }),
  playPause: () => invoke<void>("play_pause"),
  next: () => invoke<void>("next"),
  prev: () => invoke<void>("prev"),
  seek: (positionMs: number) => invoke<void>("seek", { positionMs }),
  setVolume: (volume: number, persist = false) => invoke<void>("set_volume", { volume, persist }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: RepeatMode) => invoke<void>("set_repeat", { mode }),
  queueReorder: (from: number, to: number) => invoke<void>("queue_reorder", { from, to }),
  queueRemove: (index: number) => invoke<void>("queue_remove", { index }),
  queueAdd: (items: QueueItem[]) => invoke<void>("queue_add", { items }),
  queueClear: () => invoke<void>("queue_clear"),
  getPlaybackState: () => invoke<PlaybackState>("get_playback_state"),
  getOutputDevices: () => invoke<OutputDevice[]>("get_output_devices"),
  setOutputDevice: (deviceId: string) => invoke<void>("set_output_device", { deviceId }),
  setPreferredQuality: (quality: Quality) => invoke<void>("set_preferred_quality", { quality }),
  setExclusive: (enabled: boolean) => invoke<void>("set_exclusive", { enabled }),
  setExclusiveFallback: (mode: "shared" | "warn_only") => invoke<void>("set_exclusive_fallback", { mode }),
  getAudioStatus: () => invoke<AudioStatus>("get_audio_status"),
  getConfig: () => invoke<Config>("get_config"),
  setAllowFallback: (enabled: boolean) => invoke<void>("set_allow_fallback", { enabled }),
  setQualityDisplay: (mode: "compact" | "detailed") =>
    invoke<void>("set_quality_display", { mode }),
  setCacheMaxSizeMb: (mb: number) => invoke<void>("set_cache_max_size_mb", { mb }),
  setCacheClearOnExit: (enabled: boolean) =>
    invoke<void>("set_cache_clear_on_exit", { enabled }),
  getCacheStatus: () => invoke<CacheStatus>("get_cache_status"),
  clearCache: () => invoke<void>("clear_cache"),
  getLogsDir: () => invoke<string>("get_logs_dir"),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
};
