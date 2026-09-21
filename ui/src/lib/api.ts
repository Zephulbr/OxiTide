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

export const api = {
  authBegin: () => invoke<{ url: string }>("auth_begin"),
  authComplete: (redirectUrl: string) => invoke<SessionInfoDto>("auth_complete", { redirectUrl }),
  authStatus: () => invoke<boolean>("auth_status"),
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
};
