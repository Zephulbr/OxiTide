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
export interface PlaybackState {
  playing: boolean; buffering: boolean; current?: QueueItem | null;
  position_ms: number; duration_ms: number; volume: number;
  shuffle: boolean; repeat: RepeatMode;
  preferred_quality: "low" | "high" | "lossless" | "max";
  queue: QueueItem[];
}
export interface OutputDevice { id: string; name: string; is_default: boolean }
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
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: RepeatMode) => invoke<void>("set_repeat", { mode }),
  queueReorder: (from: number, to: number) => invoke<void>("queue_reorder", { from, to }),
  queueRemove: (index: number) => invoke<void>("queue_remove", { index }),
  queueClear: () => invoke<void>("queue_clear"),
  getPlaybackState: () => invoke<PlaybackState>("get_playback_state"),
  getOutputDevices: () => invoke<OutputDevice[]>("get_output_devices"),
  setOutputDevice: (deviceId: string) => invoke<void>("set_output_device", { deviceId }),
  setPreferredQuality: (quality: "low" | "high" | "lossless" | "max") => invoke<void>("set_preferred_quality", { quality }),
};
