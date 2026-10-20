import { invoke } from "@tauri-apps/api/core";
import type { HotkeyAction } from "./hotkeys";

/// The maximize button's box in CSS px; `right` is measured from the window's
/// right edge to the button's left edge (mirrors `titlebar::SnapTarget`).
export interface SnapTarget { right: number; top: number; width: number; height: number }
export interface ArtistRef { id: number; name: string }
export interface AlbumRef {
  id: number; title: string; cover?: string | null;
  /// `YYYY-MM-DD`; embedded only in playlist track payloads.
  releaseDate?: string;
}
export interface Track {
  id: number; title: string; duration: number;
  artists: ArtistRef[]; album?: AlbumRef | null;
  audioQuality?: string | null; trackNumber: number; volumeNumber: number;
  explicit: boolean;
  mediaMetadata?: { tags: string[] } | null;
  /// When the track joined the playlist / favorites (TIDAL timestamp).
  dateAdded?: string;
  /// TIDAL's global popularity, 0–100.
  popularity?: number;
}
export interface PlayCount { track_id: number; plays: number }
export interface AlbumDate { album_id: number; release_date: string | null }
export interface TrackFlag {
  track_id: number;
  explicit: boolean | null;
  ai: boolean | null;
}
export interface Album {
  id: number; title: string; artists: ArtistRef[]; cover?: string | null;
  numberOfTracks: number; duration: number; releaseDate?: string | null;
  audioQuality?: string | null;
  /// Release type: `ALBUM` / `EP` / `SINGLE`.
  type?: string | null;
  /// When it joined the favorites (favorite albums only).
  dateAdded?: string | null;
}
export interface AlbumInfo {
  releaseDate?: string | null;
  copyright?: string | null;
  barcodeId?: string | null;
  numberOfVolumes?: number | null;
  numberOfItems?: number | null;
}
export interface CreditContributor { name: string; artistId?: number | null }
export interface CreditRole { role: string; contributors: CreditContributor[] }
export interface TrackCredits { roles: CreditRole[] }
export interface Artist {
  id: number;
  name: string;
  picture?: string | null;
  /// Album cover shown when the artist has no picture (album sizes only).
  cover_fallback?: string | null;
  mixes?: ArtistMixes | null;
}
export interface ArtistMixes {
  ARTIST_MIX?: string | null;
  MASTER_ARTIST_MIX?: string | null;
}
export interface ArtistLink { site: string; url: string }
export interface BioSegment { text: string; artistId?: number | null }
export interface ArtistProfile {
  numberOfFollowers?: number | null;
  externalLinks: ArtistLink[];
  bio?: string | null;
  bioSegments?: BioSegment[] | null;
}
export type ArtistDiscography = "albums" | "eps_and_singles" | "compilations";
export interface Playlist {
  uuid: string; title: string; description?: string | null;
  numberOfTracks: number; duration: number; image?: string | null;
  squareImage?: string | null;
  created?: string | null;
  lastItemAddedAt?: string | null; lastUpdated?: string | null;
  publicPlaylist?: boolean | null;
  numberOfFollowers?: number | null;
  owned: boolean;
}
export interface PageItem {
  id: string; title: string; subtitle?: string | null; item_type: string; artwork?: string | null;
  /// Embedded track metadata when the feed payload carries it (Track 9.3).
  artists?: string[] | null;
  duration?: number | null;
  explicit?: boolean | null;
  audio_quality?: string | null;
  album_title?: string | null;
  /// Album release type (`ALBUM` / `EP` / `SINGLE`) when known.
  release_type?: string | null;
  /// Page links: the TIDAL page they open (`pages/…`), validated in Rust.
  api_path?: string | null;
  /// Artists with ids, so cards can link them; absent when none carry an id.
  artist_refs?: ArtistRef[] | null;
}
export interface PageSection { title: string; items: PageItem[]; kind: string }
/// One release activity from an artist the user follows (v2 feed).
export interface FeedActivity {
  id: number;
  title: string;
  subtitle: string;
  artwork?: string | null;
  explicit: boolean;
  hires: boolean;
  occurredAt: string;
  seen: boolean;
  /// `ALBUM` / `EP` / `SINGLE`; empty when TIDAL omits it.
  releaseType: string;
  /// Credited artists with ids (main artists when present).
  artists: ArtistRef[];
}
export interface SearchGroup<T> {
  items: T[];
  /// Every match TIDAL has for the type, beyond this page.
  totalNumberOfItems?: number;
}
/// TIDAL's single best match for a query (absent when it names none, or a video).
export type TopHit =
  | { kind: "artist"; item: Artist }
  | { kind: "album"; item: Album }
  | { kind: "track"; item: Track }
  | { kind: "playlist"; item: Playlist };
export interface SearchResults {
  tracks: SearchGroup<Track>; albums: SearchGroup<Album>;
  artists: SearchGroup<Artist>; playlists: SearchGroup<Playlist>;
  topHit?: TopHit | null;
}
export type SearchKind = "tracks" | "albums" | "artists" | "playlists";
export interface TrackPage { items: Track[]; total: number; offset: number; limit: number }
export interface HistoryEntry {
  track_id: number; title: string; artist: string; album: string;
  duration_secs: number; cover?: string | null;
  started_at: number; listened_ms: number; completed: boolean;
  /// Name-link ids; absent on plays recorded before them.
  artist_refs?: ArtistRef[];
  album_id?: number | null;
}
export interface HistoryPage { items: HistoryEntry[]; total: number; offset: number; limit: number }
/// `track_id`/`cover` come from the entry's newest play (ids resolve through it).
export interface StatsTopEntry {
  name: string; plays: number; listened_ms: number; track_id: number; cover?: string | null;
}
export interface StatsTopTrack {
  track_id: number; title: string; artist: string; album: string; cover?: string | null;
  plays: number; listened_ms: number;
}
export interface StatsDaily { date: string; plays: number; listened_ms: number }
export interface Stats {
  total_listened_ms: number; listened_ms_7d: number; listened_ms_30d: number;
  total_plays: number; plays_7d: number; plays_30d: number;
  top_artists: StatsTopEntry[]; top_tracks: StatsTopTrack[]; top_albums: StatsTopEntry[];
  daily: StatsDaily[];
  /// All-time plays by local hour (24) and weekday (7, Monday first).
  hourly: number[]; weekdays: number[];
  distinct_tracks: number; distinct_artists: number; distinct_albums: number;
  completed_plays: number;
  current_streak_days: number; longest_streak_days: number;
}
export interface SessionInfoDto { country_code: string; user_id: number }
export interface KeyringStatus { persistent: boolean; reason?: string | null }
export interface FavoriteIds { tracks: number[]; albums: number[]; artists: number[] }
export interface Mix {
  id: string; title: string; subTitle?: string | null;
  mixType?: string | null; image?: string | null;
}
export interface CollectionFolder { id: string; name: string; totalNumberOfItems: number }
export type CollectionEntry =
  | ({ kind: "folder" } & CollectionFolder)
  | ({ kind: "playlist" } & Playlist);
export interface MixDetail { id: string; title: string; artwork?: string | null; tracks: Track[] }
export interface LyricWord { time_ms: number; text: string }
export interface LyricLine { time_ms: number | null; text: string; words?: LyricWord[] }
export interface LyricsDto {
  track_id: number;
  source: "tidal" | "lrclib";
  synced: boolean;
  rtl: boolean;
  lines: LyricLine[];
}
export type ScrobbleServiceId = "lastfm" | "listenbrainz";
export interface ScrobbleServiceStatus {
  id: ScrobbleServiceId;
  enabled: boolean;
  connected: boolean;
  username: string | null;
  queued: number;
  needs_reconnect: boolean;
  last_error: string | null;
  configured: boolean;
}
export interface ScrobbleStatusDto {
  lastfm: ScrobbleServiceStatus;
  listenbrainz: ScrobbleServiceStatus;
}
export interface PerfMark { name: string; t_ms: number; unix_ms: number }
export interface PerfSnapshot {
  active: boolean;
  run_id: string;
  scenario: string;
  marks: PerfMark[];
  cache_hits: number;
  cache_misses: number;
  last_ttfs_ms: number | null;
  memory_mb: number | null;
}
export interface CoverPalette {
  primary: string;
  on_primary: string;
  secondary: string;
  tertiary: string;
  backdrop: string;
  ceiling_l: number;
}

export interface QueueItem {
  track_id: number; title: string; artist: string; album: string;
  duration_secs: number; cover?: string | null;
  /// Name-link ids; absent when the source carried names only (page-feed
  /// tracks, sessions saved before them). `artist` stays the display string.
  artist_refs?: ArtistRef[];
  album_id?: number | null;
}
export interface PageTrackInput {
  id: number;
  title?: string | null;
  artists?: string[] | null;
  album?: string | null;
  duration?: number | null;
  cover?: string | null;
  explicit?: boolean | null;
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
  /** ReplayGain applied to this track, in dB, while ReplayGain is on. */
  replay_gain_db?: number | null;
}
export interface PlaybackState {
  playing: boolean; buffering: boolean; current?: QueueItem | null;
  current_index?: number | null;
  position_ms: number; duration_ms: number; volume: number;
  shuffle: boolean; repeat: RepeatMode;
  preferred_quality: Quality;
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  bit_perfect_notice?: string | null;
  /** True while a DSP stage alters samples (bit_perfect is then false). */
  dsp_active: boolean;
  /** The stages altering samples, e.g. ["ReplayGain", "EQ"]. */
  dsp_stages: string[];
  /** Configured crossfade length in ms (0 = off). */
  crossfade_ms: number;
  /** The most recent crossfade's report (AutoMix A/B readout). */
  last_transition?: TransitionReport | null;
  /** Always present from `get_playback_state`; see `StateEvent` for events. */
  queue: QueueItem[];
  /** Revision of `queue`; moves whenever its items change. */
  queue_rev: number;
  stream?: StreamMeta | null;
}
/** A `state` event: `queue` is omitted when `queue_rev` did not move since the
 *  previous event, and the UI keeps the copy it has. */
export type StateEvent = Omit<PlaybackState, "queue"> & { queue?: QueueItem[] };
export interface RestoredSession {
  queue: QueueItem[];
  current_index: number;
  position_ms: number;
  shuffle: boolean;
  repeat: RepeatMode;
  play_source: string | null;
  stream?: StreamMeta | null;
}
export interface OutputDevice { id: string; name: string; is_default: boolean }
export interface AudioStatus {
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  notice?: string | null;
}
export type ReplayGainMode = "off" | "track" | "album";
export interface EqSettings {
  enabled: boolean;
  auto_preamp: boolean;
  preamp_db: number;
  bands_db: number[];
}
export interface ReplayGainSettings {
  mode: ReplayGainMode;
  preamp_db: number;
  untagged_db: number;
  prevent_clipping: boolean;
}
export interface CrossfadeSettings {
  duration_ms: number;
  skip_same_album: boolean;
  /** AutoMix: overlap audible audio, skipping silence at track edges. */
  trim_silence: boolean;
  /** AutoMix: overlap across sample-rate/channel changes by resampling. */
  resample: boolean;
  /** AutoMix: align the overlap beat-to-beat when tempos fit. */
  beat_match: boolean;
  /** Where the overlap sits and how the tracks trade places. */
  style: TransitionStyle;
}
/** Rust `dsp::TransitionStyle`. */
export type TransitionStyle = "classic" | "quick" | "blend" | "auto";
export type TransitionKind = "crossfade" | "beat_matched" | "fade_out" | "gapless";
/** What the most recent crossfade did (Rust `automix::TransitionReport`). */
export interface TransitionReport {
  kind: TransitionKind;
  overlap_ms: number;
  trimmed_out_ms: number;
  trimmed_in_ms: number;
  resampled: boolean;
  out_bpm?: number | null;
  in_bpm?: number | null;
  out_confidence?: number | null;
  in_confidence?: number | null;
  beats?: number | null;
  note?: string | null;
  style: TransitionStyle;
  /** Audible outgoing audio dropped after the overlap. */
  cut_out_ms: number;
  /** Incoming intro skipped beyond its leading silence. */
  cut_in_ms: number;
  /** Blend's level ride: dB the incoming track entered under its own level (0 = none). */
  ride_db?: number;
  /** Why Auto picked `style` (only when the setting is Auto). */
  auto_reason?: string | null;
}
/** A transition re-rendered by the AutoMix replay lab (`automix_render`). */
export interface TransitionReplay {
  report: TransitionReport;
  overlap_start_ms: number;
  overlap_end_ms: number;
  wav: Blob;
}
export interface DspSettings {
  eq: EqSettings;
  replay_gain: ReplayGainSettings;
  crossfade: CrossfadeSettings;
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
  ui: {
    quality_display: "compact" | "detailed";
    mode: "instrument" | "atmosphere" | "rack";
    theme: "dark" | "light" | "system";
    typeface: "auto" | "manrope" | "inter" | "system" | "custom";
    custom_typeface: string;
    density: "comfortable" | "compact";
    cover_shape: "disc" | "sleeve";
    dynamic_color: "auto" | "full" | "accent" | "off";
    queue_width: number;
    start_in_mini: boolean;
    oneko: boolean;
    hover_panels: boolean;
    close_to_tray: boolean;
    window_width: number;
    window_height: number;
    window_x?: number | null;
    window_y?: number | null;
    window_maximized: boolean;
  };
  integrations: {
    smtc: boolean;
    discord: boolean;
  };
  cache: { dir: string; max_size_mb: number; clear_on_exit: boolean };
  scrobbling: {
    lastfm_enabled: boolean;
    listenbrainz_enabled: boolean;
    lastfm_api_key: string;
    lastfm_api_secret: string;
  };
  history: { enabled: boolean; max_entries: number };
  content: { block_explicit: boolean; block_ai: boolean; show_labels: boolean };
  hotkeys: {
    play_pause: string;
    next: string;
    prev: string;
    volume_up: string;
    volume_down: string;
    show_hide: string;
  };
  dsp: DspSettings;
  rack: RackSettings;
  remote: RemoteSettings;
  updates: { check_on_start: boolean };
}
/** `get_update_support` (`updater.rs`). `reason` explains an unsupported install. */
export interface UpdateSupport {
  supported: boolean;
  reason: string | null;
  current_version: string;
  check_on_start: boolean;
}
/** A newer release (`check_for_update`, the `update_available` event). */
export interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
  date: string | null;
}
/** `update_progress` event while an update downloads. */
export interface UpdateProgress { downloaded: number; total: number | null }
export const UPDATE_AVAILABLE_EVENT = "update_available";
export const UPDATE_PROGRESS_EVENT = "update_progress";
/** Phone remote preferences (`[remote]`, Track 11). */
export interface RemoteSettings {
  enabled: boolean;
  port: number;
  /** QR address override; "" picks automatically. */
  interface: string;
  approve_new_devices: boolean;
}
export interface RemoteInterface { name: string; ip: string; virtual_adapter: boolean }
export interface RemotePairRequest { id: string; name: string; ip: string }
/** `get_remote_status` / the `remote_status` event (`remote/host.rs`). */
export interface RemoteStatus {
  enabled: boolean;
  listening: boolean;
  port: number | null;
  address: string | null;
  url: string | null;
  error: string | null;
  connected: number;
  interfaces: RemoteInterface[];
  settings: RemoteSettings;
  pending: RemotePairRequest[];
  pairing_expires_in_secs: number | null;
}
export interface RemotePairing { url: string; qr_svg: string; expires_in_secs: number }
export interface RemoteDevice {
  id: string;
  name: string;
  created_ms: number;
  last_seen_ms: number;
  connected: boolean;
}
export const REMOTE_STATUS_EVENT = "remote_status";
export const REMOTE_PAIR_REQUEST_EVENT = "remote_pair_request";
/** A favorite changed anywhere (desktop or a paired phone). */
export interface FavoriteChange { kind: "track" | "album" | "artist"; id: number; favorite: boolean }
export const FAVORITE_CHANGED_EVENT = "favorite_changed";

/* Phone-remote wire types (camelCase; `remote/events.rs`). The phone page
   imports these type-only. */
export interface RemoteTrack {
  trackId: number;
  title: string;
  artist: string;
  album: string;
  cover?: string | null;
  durationSecs: number;
  explicit?: boolean | null;
  audioQuality?: Quality | null;
}
export interface RemoteStream {
  codec?: string | null;
  sampleRate: number;
  bitDepth: number;
  bitrateKbps?: number | null;
}
export interface RemoteState {
  loggedIn: boolean;
  playing: boolean;
  buffering: boolean;
  current: RemoteTrack | null;
  currentIndex: number | null;
  positionMs: number;
  durationMs: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  queueLen: number;
  queueRev: number;
  stream: RemoteStream | null;
  bitPerfect: boolean;
  dspStages: string[];
}
export interface RemoteQueuePage {
  items: QueueItem[];
  total: number;
  offset: number;
  limit: number;
  currentIndex: number | null;
  queueRev: number;
}
export type DeckFace = "transport" | "cd" | "display";
export type MeterStyle = "needles" | "bars" | "led" | "spectrum";
export type LampColor = "amber" | "blue" | "green" | "red" | "white" | "cover";
export type AnalyzerStyle = "auto" | "spectrum" | "recorder" | "tuner" | "eq";
/** Hi-Fi Rack options (`[rack]`, Track 9.5). */
export interface RackSettings {
  deck_face: DeckFace;
  meters: MeterStyle;
  analyzer: AnalyzerStyle;
  lamp: LampColor;
  eq_unit: boolean;
  /** Percent, 40–100. */
  lamp_brightness: number;
}
/** One `level_frame` event: linear peaks/RMS (1 = full scale), bands in dBFS. */
export interface LevelFrame {
  peak: [number, number];
  rms: [number, number];
  bands: number[];
}
export const LEVEL_FRAME_EVENT = "level_frame";
export type EngineEvent =
  | ({ type: "state" } & StateEvent)
  | { type: "now_playing"; [k: string]: unknown }
  | { type: "position"; position_ms: number; duration_ms: number }
  | { type: "error"; code: string; message: string };

export const AUTH_SUCCESS_EVENT = "auth://success";
export const AUTH_ERROR_EVENT = "auth://error";
export const SCROBBLE_STATUS_EVENT = "scrobble://status";
export const SCROBBLE_AUTH_ERROR_EVENT = "scrobble://auth-error";
export const SCROBBLE_SCROBBLED_EVENT = "scrobble://scrobbled";

export function trackToQueueItem(t: Track): QueueItem {
  return {
    track_id: t.id,
    title: t.title,
    artist: t.artists.map((a) => a.name).join(", "),
    album: t.album?.title ?? "",
    duration_secs: t.duration,
    cover: t.album?.cover ?? null,
    artist_refs: t.artists.map((a) => ({ id: a.id, name: a.name })),
    album_id: t.album?.id ?? null,
  };
}

export function historyToQueueItem(entry: HistoryEntry): QueueItem {
  return {
    track_id: entry.track_id,
    title: entry.title,
    artist: entry.artist,
    album: entry.album,
    duration_secs: entry.duration_secs,
    cover: entry.cover ?? null,
    artist_refs: entry.artist_refs,
    album_id: entry.album_id ?? null,
  };
}

/// Split an `automix_render` payload: `[u32 LE header length][JSON][WAV]`.
export function parseReplay(buffer: ArrayBuffer): TransitionReplay {
  const length = new DataView(buffer).getUint32(0, true);
  const header = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, 4, length)));
  return {
    report: header.report,
    overlap_start_ms: header.overlap_start_ms,
    overlap_end_ms: header.overlap_end_ms,
    wav: new Blob([new Uint8Array(buffer, 4 + length)], { type: "audio/wav" }),
  };
}

export const api = {
  authBegin: (mode: "deep" | "paste" = "deep") =>
    invoke<{ url: string }>("auth_begin", { mode }),
  authComplete: (redirectUrl: string) => invoke<SessionInfoDto>("auth_complete", { redirectUrl }),
  authTakeResult: () => invoke<SessionInfoDto | null>("auth_take_result"),
  authStatus: () => invoke<boolean>("auth_status"),
  authRestore: () => invoke<SessionInfoDto | null>("auth_restore"),
  authLogout: () => invoke<void>("auth_logout"),
  getKeyringStatus: () => invoke<KeyringStatus>("get_keyring_status"),
  getHome: () => invoke<PageSection[]>("get_home"),
  getExplore: () => invoke<PageSection[]>("get_explore"),
  getPage: (path: string) => invoke<PageSection[]>("get_page", { path }),
  getFeed: () => invoke<FeedActivity[]>("get_feed"),
  getFavoriteMixes: () => invoke<Mix[]>("get_favorite_mixes"),
  getMix: (id: string) => invoke<MixDetail>("get_mix", { id }),
  getForYou: () => invoke<PageSection[]>("get_for_you"),
  getTrackRadio: (trackId: number) => invoke<Track[]>("get_track_radio", { trackId }),
  getArtistRadio: (artistId: number) => invoke<Track[]>("get_artist_radio", { artistId }),
  getSimilarArtists: (artistId: number) => invoke<Artist[]>("get_similar_artists", { artistId }),
  getArtistProfile: (id: number) => invoke<ArtistProfile>("get_artist_profile", { id }),
  getArtistTopTracks: (artistId: number, limit: number) =>
    invoke<Track[]>("get_artist_top_tracks", { artistId, limit }),
  getArtistTopTracksPage: (artistId: number, offset: number, limit: number) =>
    invoke<TrackPage>("get_artist_top_tracks_page", { artistId, offset, limit }),
  getArtistAlbums: (artistId: number, filter: ArtistDiscography, limit: number, offset: number) =>
    invoke<Album[]>("get_artist_albums", { artistId, filter, limit, offset }),
  getArtistAlbumsAll: (artistId: number, filter: ArtistDiscography) =>
    invoke<Album[]>("get_artist_albums_all", { artistId, filter }),
  getArtistPage: (artistId: number) => invoke<PageSection[]>("get_artist_page", { artistId }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  getLyrics: (trackId: number, title: string, artist: string, album: string, durationSecs: number) =>
    invoke<LyricsDto | null>("get_lyrics", { trackId, title, artist, album, durationSecs }),
  getScrobbleStatus: () => invoke<ScrobbleStatusDto>("get_scrobble_status"),
  lastfmBeginAuth: () => invoke<string>("lastfm_begin_auth"),
  lastfmCompleteAuth: () => invoke<ScrobbleStatusDto>("lastfm_complete_auth"),
  lastfmCancelAuth: () => invoke<void>("lastfm_cancel_auth"),
  lastfmDisconnect: () => invoke<void>("lastfm_disconnect"),
  listenbrainzConnect: (token: string) =>
    invoke<ScrobbleStatusDto>("listenbrainz_connect", { token }),
  listenbrainzDisconnect: () => invoke<void>("listenbrainz_disconnect"),
  setScrobbleEnabled: (service: ScrobbleServiceId, enabled: boolean) =>
    invoke<void>("set_scrobble_enabled", { service, enabled }),
  getCollectionItems: (folderId = "root", order?: string) =>
    invoke<CollectionEntry[]>("get_collection_items", { folderId, order: order ?? null }),
  createFolder: (name: string, parentId = "root") =>
    invoke<CollectionFolder>("create_folder", { name, parentId }),
  renameFolder: (id: string, name: string) => invoke<void>("rename_folder", { id, name }),
  removeCollectionEntry: (kind: "folder" | "playlist", id: string) =>
    invoke<void>("remove_collection_entry", { kind, id }),
  movePlaylistsToFolder: (ids: string[], folderId: string) =>
    invoke<void>("move_playlists_to_folder", { ids, folderId }),
  getPurchases: () => invoke<PageItem[]>("get_purchases"),
  getAlbum: (id: number) => invoke<Album>("get_album", { id }),
  getAlbumTracks: (id: number) => invoke<Track[]>("get_album_tracks", { id }),
  getAlbumInfo: (id: number) => invoke<AlbumInfo>("get_album_info", { id }),
  getTrack: (id: number) => invoke<Track>("get_track", { id }),
  getTrackCredits: (id: number) => invoke<TrackCredits>("get_track_credits", { id }),
  getPlaylist: (id: string) => invoke<Playlist>("get_playlist", { id }),
  getPlaylistTracks: (id: string) => invoke<Track[]>("get_playlist_tracks", { id }),
  getArtist: (id: number) => invoke<Artist>("get_artist", { id }),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  /// One 50-item page of a single result type; the other groups are empty.
  searchType: (query: string, kind: SearchKind, offset: number) =>
    invoke<SearchResults>("search_type", { query, kind, offset }),
  getFavoriteTracks: () => invoke<Track[]>("get_favorite_tracks"),
  getFavoriteAlbums: () => invoke<Album[]>("get_favorite_albums"),
  getFavoriteArtists: () => invoke<Artist[]>("get_favorite_artists"),
  getUserPlaylists: () => invoke<Playlist[]>("get_user_playlists"),
  createPlaylist: (name: string, description: string) =>
    invoke<Playlist>("create_playlist", { name, description }),
  setPlaylistPublic: (id: string, isPublic: boolean) =>
    invoke<void>("set_playlist_public", { id, isPublic }),
  renamePlaylist: (id: string, title: string, description: string) =>
    invoke<void>("rename_playlist", { id, title, description }),
  deletePlaylist: (id: string) => invoke<void>("delete_playlist", { id }),
  addPlaylistTracks: (id: string, trackIds: number[], allowDuplicates: boolean) =>
    invoke<number[]>("add_playlist_tracks", { id, trackIds, allowDuplicates }),
  removePlaylistItems: (id: string, indices: number[]) =>
    invoke<void>("remove_playlist_items", { id, indices }),
  movePlaylistItems: (id: string, indices: number[], toIndex: number) =>
    invoke<void>("move_playlist_items", { id, indices, toIndex }),
  getFavoriteTracksPage: (offset: number, limit: number) =>
    invoke<TrackPage>("get_favorite_tracks_page", { offset, limit }),
  getPlaylistTracksPage: (id: string, offset: number, limit: number) =>
    invoke<TrackPage>("get_playlist_tracks_page", { id, offset, limit }),
  getFavoriteIds: () => invoke<FavoriteIds>("get_favorite_ids"),
  setFavoriteTrack: (trackId: number, favorite: boolean) =>
    invoke<void>("set_favorite_track", { trackId, favorite }),
  setFavoriteAlbum: (albumId: number, favorite: boolean) =>
    invoke<void>("set_favorite_album", { albumId, favorite }),
  setFavoriteArtist: (artistId: number, favorite: boolean) =>
    invoke<void>("set_favorite_artist", { artistId, favorite }),
  playTracks: (items: QueueItem[], start: number, shuffle = false) =>
    invoke<void>("play_tracks", { items, start, shuffle }),
  playTrackIds: (ids: number[], start: number, shuffle = false) =>
    invoke<void>("play_track_ids", { ids, start, shuffle }),
  playPageItems: (items: PageTrackInput[], start: number, shuffle = false) =>
    invoke<void>("play_page_items", { items, start, shuffle }),
  playPause: () => invoke<void>("play_pause"),
  retry: () => invoke<void>("retry"),
  next: () => invoke<void>("next"),
  prev: () => invoke<void>("prev"),
  seek: (positionMs: number) => invoke<void>("seek", { positionMs }),
  setVolume: (volume: number, persist = false) => invoke<void>("set_volume", { volume, persist }),
  fadeAndPause: (durationMs: number) => invoke<void>("fade_and_pause", { durationMs }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: RepeatMode) => invoke<void>("set_repeat", { mode }),
  queueReorder: (from: number, to: number) => invoke<void>("queue_reorder", { from, to }),
  queueRemove: (index: number) => invoke<void>("queue_remove", { index }),
  queueAdd: (items: QueueItem[]) => invoke<void>("queue_add", { items }),
  queuePlayNext: (items: QueueItem[]) => invoke<void>("queue_play_next", { items }),
  queueClear: () => invoke<void>("queue_clear"),
  getPlaybackState: () => invoke<PlaybackState>("get_playback_state"),
  getRestoredSession: () => invoke<RestoredSession | null>("get_restored_session"),
  restoreSession: (
    items: QueueItem[],
    start: number,
    positionMs: number,
    shuffle: boolean,
    repeat: RepeatMode,
  ) => invoke<void>("restore_session", { items, start, positionMs, shuffle, repeat }),
  setPlaySource: (id: string | null) => invoke<void>("set_play_source", { id }),
  getHistoryPage: (offset: number, limit: number) =>
    invoke<HistoryPage>("get_history_page", { offset, limit }),
  getStats: (offsetMinutes: number) => invoke<Stats>("get_stats", { offsetMinutes }),
  clearHistory: () => invoke<void>("clear_history"),
  setHistoryEnabled: (enabled: boolean) => invoke<void>("set_history_enabled", { enabled }),
  setHistoryMaxEntries: (maxEntries: number) =>
    invoke<void>("set_history_max_entries", { maxEntries }),
  getTrackFlags: (ids: number[]) => invoke<TrackFlag[]>("get_track_flags", { ids }),
  getPlayCounts: (ids: number[]) => invoke<PlayCount[]>("get_play_counts", { ids }),
  getAlbumReleaseDates: (ids: number[]) =>
    invoke<AlbumDate[]>("get_album_release_dates", { ids }),
  setBlockExplicit: (enabled: boolean) => invoke<void>("set_block_explicit", { enabled }),
  setBlockAi: (enabled: boolean) => invoke<void>("set_block_ai", { enabled }),
  setShowLabels: (enabled: boolean) => invoke<void>("set_show_labels", { enabled }),
  applyContentFilters: (scope: "explicit" | "ai") =>
    invoke<void>("apply_content_filters", { scope }),
  getOutputDevices: () => invoke<OutputDevice[]>("get_output_devices"),
  setOutputDevice: (deviceId: string) => invoke<void>("set_output_device", { deviceId }),
  setPreferredQuality: (quality: Quality) => invoke<void>("set_preferred_quality", { quality }),
  setExclusive: (enabled: boolean) => invoke<void>("set_exclusive", { enabled }),
  setExclusiveFallback: (mode: "shared" | "warn_only") => invoke<void>("set_exclusive_fallback", { mode }),
  getAudioStatus: () => invoke<AudioStatus>("get_audio_status"),
  setDspSettings: (settings: DspSettings) =>
    invoke<DspSettings>("set_dsp_settings", { settings }),
  automixRender: async (style: TransitionStyle) =>
    parseReplay(await invoke<ArrayBuffer>("automix_render", { style })),
  getPerfSnapshot: () => invoke<PerfSnapshot>("get_perf_snapshot"),
  perfMark: (name: string) => invoke<void>("perf_mark", { name }),
  perfSubmitUiMetrics: (metrics: Record<string, unknown>) =>
    invoke<void>("perf_submit_ui_metrics", { metrics }),
  perfQueueBuild: (items: QueueItem[]) => invoke<void>("perf_queue_build", { items, start: 0 }),
  getConfig: () => invoke<Config>("get_config"),
  setAllowFallback: (enabled: boolean) => invoke<void>("set_allow_fallback", { enabled }),
  setQualityDisplay: (mode: "compact" | "detailed") =>
    invoke<void>("set_quality_display", { mode }),
  setUiMode: (mode: "instrument" | "atmosphere" | "rack") => invoke<void>("set_ui_mode", { mode }),
  setRackSettings: (settings: RackSettings) =>
    invoke<RackSettings>("set_rack_settings", { settings }),
  setLevelMeter: (enabled: boolean) => invoke<void>("set_level_meter", { enabled }),
  setTheme: (theme: "dark" | "light" | "system") => invoke<void>("set_theme", { theme }),
  setTypeface: (typeface: "auto" | "manrope" | "inter" | "system" | "custom") =>
    invoke<void>("set_typeface", { typeface }),
  setCustomTypeface: (name: string) => invoke<void>("set_custom_typeface", { name }),
  setDensity: (density: "comfortable" | "compact") => invoke<void>("set_density", { density }),
  setCoverShape: (shape: "disc" | "sleeve") => invoke<void>("set_cover_shape", { shape }),
  setDynamicColor: (mode: "auto" | "full" | "accent" | "off") =>
    invoke<void>("set_dynamic_color", { mode }),
  setQueueWidth: (px: number) => invoke<void>("set_queue_width", { px }),
  setMiniMode: (active: boolean) => invoke<void>("set_mini_mode", { active }),
  setStartInMini: (enabled: boolean) => invoke<void>("set_start_in_mini", { enabled }),
  setOneko: (enabled: boolean) => invoke<void>("set_oneko", { enabled }),
  setHoverPanels: (enabled: boolean) => invoke<void>("set_hover_panels", { enabled }),
  setSmtc: (enabled: boolean) => invoke<void>("set_smtc", { enabled }),
  setCloseToTray: (enabled: boolean) => invoke<void>("set_close_to_tray", { enabled }),
  setHotkey: (action: HotkeyAction, accelerator: string, replace: HotkeyAction | null = null) =>
    invoke<void>("set_hotkey", { action, accelerator, replace }),
  setHotkeyCapture: (active: boolean) => invoke<void>("set_hotkey_capture", { active }),
  setDiscord: (enabled: boolean) => invoke<void>("set_discord", { enabled }),
  setCacheMaxSizeMb: (mb: number) => invoke<void>("set_cache_max_size_mb", { mb }),
  setCacheClearOnExit: (enabled: boolean) =>
    invoke<void>("set_cache_clear_on_exit", { enabled }),
  getCacheStatus: () => invoke<CacheStatus>("get_cache_status"),
  clearCache: () => invoke<void>("clear_cache"),
  getCustomTitlebar: () => invoke<boolean>("get_custom_titlebar"),
  setSnapTarget: (target: SnapTarget | null) => invoke<void>("set_snap_target", { target }),
  getLogsDir: () => invoke<string>("get_logs_dir"),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  analyzeCover: (uuid: string, theme: "dark" | "light") =>
    invoke<CoverPalette>("analyze_cover", { uuid, theme }),
  getUpdateSupport: () => invoke<UpdateSupport>("get_update_support"),
  checkForUpdate: () => invoke<UpdateInfo | null>("check_for_update"),
  installUpdate: () => invoke<void>("install_update"),
  setUpdateCheckOnStart: (enabled: boolean) =>
    invoke<void>("set_update_check_on_start", { enabled }),
  getRemoteStatus: () => invoke<RemoteStatus>("get_remote_status"),
  setRemoteEnabled: (enabled: boolean) => invoke<RemoteStatus>("set_remote_enabled", { enabled }),
  setRemoteSettings: (port: number, iface: string, approveNewDevices: boolean) =>
    invoke<RemoteStatus>("set_remote_settings", {
      port,
      interface: iface,
      approveNewDevices,
    }),
  beginRemotePairing: () => invoke<RemotePairing>("begin_remote_pairing"),
  cancelRemotePairing: () => invoke<void>("cancel_remote_pairing"),
  listRemoteDevices: () => invoke<RemoteDevice[]>("list_remote_devices"),
  renameRemoteDevice: (id: string, name: string) =>
    invoke<void>("rename_remote_device", { id, name }),
  revokeRemoteDevice: (id: string) => invoke<void>("revoke_remote_device", { id }),
  resolveRemotePairRequest: (id: string, approve: boolean) =>
    invoke<boolean>("resolve_remote_pair_request", { id, approve }),
};
