import type {
  Album,
  Artist,
  CoverPalette,
  ArtistDiscography,
  CollectionEntry,
  FavoriteIds,
  HistoryPage,
  LyricsDto,
  Mix,
  MixDetail,
  PageSection,
  PageTrackInput,
  Playlist,
  QueueItem,
  RemoteQueuePage,
  RemoteState,
  RepeatMode,
  SearchResults,
  Track,
  TrackPage,
} from "../lib/api";
import type { Call } from "./rpc";

/// Typed wrappers over the remote allowlist (`remote/rpc.rs`). Params are
/// camelCase like `invoke`.
export function remoteApi(call: Call) {
  return {
    getState: () => call<RemoteState>("get_remote_state"),
    playPause: () => call<void>("play_pause"),
    next: () => call<void>("next"),
    prev: () => call<void>("prev"),
    seek: (positionMs: number) => call<void>("seek", { positionMs: Math.round(positionMs) }),
    setVolume: (volume: number) => call<void>("set_volume", { volume }),
    setShuffle: (enabled: boolean) => call<void>("set_shuffle", { enabled }),
    setRepeat: (mode: RepeatMode) => call<void>("set_repeat", { mode }),
    fadeAndPause: (durationMs: number) => call<void>("fade_and_pause", { durationMs }),
    retry: () => call<void>("retry"),
    playPageItems: (items: PageTrackInput[], start: number, shuffle = false) =>
      call<void>("play_page_items", { items, start, shuffle }),
    playTrackIds: (ids: number[], start: number, shuffle = false) =>
      call<void>("play_track_ids", { ids, start, shuffle }),
    queueAdd: (items: QueueItem[]) => call<void>("queue_add", { items }),
    queueRemove: (index: number) => call<void>("queue_remove", { index }),
    queueReorder: (from: number, to: number) => call<void>("queue_reorder", { from, to }),
    queueClear: () => call<void>("queue_clear"),
    queueJump: (index: number) => call<void>("queue_jump", { index }),
    getQueuePage: (offset: number, limit: number) =>
      call<RemoteQueuePage>("get_queue_page", { offset, limit }),
    search: (query: string) => call<SearchResults>("search", { query }),
    getHome: () => call<PageSection[]>("get_home"),
    getExplore: () => call<PageSection[]>("get_explore"),
    getForYou: () => call<PageSection[]>("get_for_you"),
    getFavoriteMixes: () => call<Mix[]>("get_favorite_mixes"),
    getMix: (id: string) => call<MixDetail>("get_mix", { id }),
    getTrackRadio: (trackId: number) => call<Track[]>("get_track_radio", { trackId }),
    getArtistRadio: (artistId: number) => call<Track[]>("get_artist_radio", { artistId }),
    getSimilarArtists: (artistId: number) => call<Artist[]>("get_similar_artists", { artistId }),
    getAlbum: (id: number) => call<Album>("get_album", { id }),
    getAlbumTracks: (id: number) => call<Track[]>("get_album_tracks", { id }),
    getPlaylist: (id: string) => call<Playlist>("get_playlist", { id }),
    getPlaylistTracksPage: (id: string, offset: number, limit: number) =>
      call<TrackPage>("get_playlist_tracks_page", { id, offset, limit }),
    getArtist: (id: number) => call<Artist>("get_artist", { id }),
    getArtistTopTracksPage: (artistId: number, offset: number, limit: number) =>
      call<TrackPage>("get_artist_top_tracks_page", { artistId, offset, limit }),
    getArtistAlbumsAll: (artistId: number, filter: ArtistDiscography) =>
      call<Album[]>("get_artist_albums_all", { artistId, filter }),
    getTrack: (id: number) => call<Track>("get_track", { id }),
    getLyrics: (trackId: number, title: string, artist: string, album: string, durationSecs: number) =>
      call<LyricsDto | null>("get_lyrics", { trackId, title, artist, album, durationSecs }),
    analyzeCover: (uuid: string, theme: "dark" | "light") =>
      call<CoverPalette>("analyze_cover", { uuid, theme }),
    getFavoriteTracksPage: (offset: number, limit: number) =>
      call<TrackPage>("get_favorite_tracks_page", { offset, limit }),
    getFavoriteAlbums: () => call<Album[]>("get_favorite_albums"),
    getFavoriteArtists: () => call<Artist[]>("get_favorite_artists"),
    getFavoriteIds: () => call<FavoriteIds>("get_favorite_ids"),
    getUserPlaylists: () => call<Playlist[]>("get_user_playlists"),
    getCollectionItems: (folderId: string) =>
      call<CollectionEntry[]>("get_collection_items", { folderId, order: null }),
    getHistoryPage: (offset: number, limit: number) =>
      call<HistoryPage>("get_history_page", { offset, limit }),
    setFavoriteTrack: (trackId: number, favorite: boolean) =>
      call<void>("set_favorite_track", { trackId, favorite }),
    setFavoriteAlbum: (albumId: number, favorite: boolean) =>
      call<void>("set_favorite_album", { albumId, favorite }),
    setFavoriteArtist: (artistId: number, favorite: boolean) =>
      call<void>("set_favorite_artist", { artistId, favorite }),
    addPlaylistTracks: (id: string, trackIds: number[]) =>
      call<number[]>("add_playlist_tracks", { id, trackIds, allowDuplicates: false }),
  };
}

export type RemoteApi = ReturnType<typeof remoteApi>;
