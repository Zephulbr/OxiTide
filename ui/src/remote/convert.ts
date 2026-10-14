import type {
  Album,
  Artist,
  HistoryEntry,
  Mix,
  PageItem,
  PageTrackInput,
  Playlist,
  QueueItem,
  Track,
} from "../lib/api";
import { ARTIST_PICTURE_SIZE, coverUrl, type CoverSize } from "../lib/artwork";
import { isPlayableTrack, pageTrackInput } from "../lib/media";
import type { RemoteRoute } from "./routes";
import { routeForItem } from "./routes";

/// One track row, whatever the source (catalogue, feed, history, queue).
export interface RowTrack {
  id: number;
  title: string;
  artist: string;
  album: string;
  cover: string | null;
  duration: number;
  explicit: boolean;
  artistId: number | null;
  albumId: number | null;
  /// Feed rows carry partial metadata; play them through their own payload.
  input?: PageTrackInput;
}

export function fromTrack(t: Track): RowTrack {
  return {
    id: t.id,
    title: t.title,
    artist: t.artists.map((a) => a.name).join(", "),
    album: t.album?.title ?? "",
    cover: t.album?.cover ?? null,
    duration: t.duration,
    explicit: t.explicit,
    artistId: t.artists[0]?.id ?? null,
    albumId: t.album?.id ?? null,
  };
}

export function fromPageItem(item: PageItem): RowTrack {
  return {
    id: Number(item.id),
    title: item.title,
    artist: item.artists?.join(", ") || item.subtitle || "",
    album: item.album_title ?? "",
    cover: item.artwork ?? null,
    duration: item.duration ?? 0,
    explicit: item.explicit ?? false,
    artistId: null,
    albumId: null,
    input: pageTrackInput(item),
  };
}

export function fromHistory(entry: HistoryEntry): RowTrack {
  return {
    id: entry.track_id,
    title: entry.title,
    artist: entry.artist,
    album: entry.album,
    cover: entry.cover ?? null,
    duration: entry.duration_secs,
    explicit: false,
    artistId: null,
    albumId: null,
  };
}

export function fromQueueItem(item: QueueItem): RowTrack {
  return {
    id: item.track_id,
    title: item.title,
    artist: item.artist,
    album: item.album,
    cover: item.cover ?? null,
    duration: item.duration_secs,
    explicit: false,
    artistId: null,
    albumId: null,
  };
}

/// The `play_page_items` payload: complete metadata skips the per-track
/// lookup on the PC.
export function toPageInput(row: RowTrack): PageTrackInput {
  if (row.input) return row.input;
  return {
    id: row.id,
    title: row.title || null,
    artists: row.artist ? [row.artist] : null,
    album: row.album || null,
    duration: row.duration || null,
    cover: row.cover,
    explicit: row.explicit,
  };
}

export function toQueueItem(row: RowTrack): QueueItem {
  return {
    track_id: row.id,
    title: row.title,
    artist: row.artist,
    album: row.album,
    duration_secs: row.duration,
    cover: row.cover,
  };
}

export function art(uuid: string | null | undefined, size: CoverSize = 320): string {
  return uuid ? coverUrl(uuid, size) : "";
}

/// A shelf tile.
export interface Tile {
  key: string;
  title: string;
  subtitle: string;
  image: string;
  round: boolean;
  route: RemoteRoute | null;
  /// A playable feed track (no route): tapping plays it.
  track: RowTrack | null;
}

export function tileForItem(item: PageItem, index: number): Tile {
  const type = item.item_type.toUpperCase();
  const round = type.includes("ARTIST");
  return {
    key: `${item.item_type}-${item.id || index}`,
    title: item.title,
    subtitle: item.subtitle ?? "",
    image: art(item.artwork, round ? ARTIST_PICTURE_SIZE : 320),
    round,
    route: routeForItem(item),
    track: isPlayableTrack(item) ? fromPageItem(item) : null,
  };
}

export function albumTile(album: Album): Tile {
  const year = album.releaseDate?.slice(0, 4) ?? "";
  return {
    key: `album-${album.id}`,
    title: album.title,
    subtitle: [album.artists.map((a) => a.name).join(", "), year].filter(Boolean).join(" · "),
    image: art(album.cover),
    round: false,
    route: { name: "album", id: album.id },
    track: null,
  };
}

export function artistTile(artist: Artist): Tile {
  return {
    key: `artist-${artist.id}`,
    title: artist.name,
    subtitle: "Artist",
    image: art(artist.picture, ARTIST_PICTURE_SIZE),
    round: true,
    route: { name: "artist", id: artist.id },
    track: null,
  };
}

export function playlistTile(playlist: Playlist): Tile {
  return {
    key: `playlist-${playlist.uuid}`,
    title: playlist.title,
    subtitle: `${playlist.numberOfTracks} tracks`,
    image: art(playlist.squareImage ?? playlist.image),
    round: false,
    route: { name: "playlist", id: playlist.uuid },
    track: null,
  };
}

export function mixTile(mix: Mix): Tile {
  return {
    key: `mix-${mix.id}`,
    title: mix.title,
    subtitle: mix.subTitle ?? "Mix",
    image: art(mix.image),
    round: false,
    route: { name: "mix", id: mix.id },
    track: null,
  };
}
