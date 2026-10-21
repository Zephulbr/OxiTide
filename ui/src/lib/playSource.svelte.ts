import { api } from "./api";
import { parseRoute, type Route } from "./nav.svelte";
import { playlistRecency } from "./playlistRecency.svelte";

export type PlaylistRowState = "source" | "active" | "source-active" | "idle";

/// Sidebar row treatment: playing (source) and open (nav-active) are separate
/// signals, so a row can be both. The playing source keeps its accent label
/// and play glyph; the open highlight is never lost to it.
export function playlistRowState(isSource: boolean, isActive: boolean): PlaylistRowState {
  if (isSource) return isActive ? "source-active" : "source";
  if (isActive) return "active";
  return "idle";
}

export type PlayFromKind =
  | "album"
  | "playlist"
  | "mix"
  | "artist"
  | "favorites"
  | "history"
  | "search"
  | "radio"
  | "section";

/// What started the current queue, for the player bar's "From …" line: the
/// kind (its eyebrow word), the collection's name, and the page it opens
/// (null when there is none, e.g. a track radio).
export interface PlayFrom {
  kind: PlayFromKind;
  name: string;
  route: Route | null;
}

/// The persisted form: the source plus the track ids it queued, so tracks
/// added later (Add to queue, Play next, the phone remote) are not credited
/// to it.
export interface SavedPlayFrom extends PlayFrom {
  track_ids: number[];
}

const KINDS: readonly PlayFromKind[] = [
  "album",
  "playlist",
  "mix",
  "artist",
  "favorites",
  "history",
  "search",
  "radio",
  "section",
];

const KIND_LABELS: Record<PlayFromKind, string> = {
  album: "From album",
  playlist: "From playlist",
  mix: "From mix",
  artist: "From artist",
  favorites: "From",
  history: "From",
  search: "From search",
  radio: "From",
  section: "From",
};

/// The eyebrow before the source's name ("From album").
export function playFromLabel(kind: PlayFromKind): string {
  return KIND_LABELS[kind];
}

/// A saved source read back from the session, checked field by field.
export function parseSavedPlayFrom(value: unknown): SavedPlayFrom | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  if (!KINDS.includes(record.kind as PlayFromKind)) return null;
  if (typeof record.name !== "string") return null;
  const ids = Array.isArray(record.track_ids)
    ? record.track_ids.filter((id): id is number => typeof id === "number")
    : [];
  return {
    kind: record.kind as PlayFromKind,
    name: record.name,
    route: parseRoute(record.route),
    track_ids: ids,
  };
}

/// Sessions saved before the "From …" line carried only a playlist id: it
/// still marks the sidebar row, but credits no tracks.
export function legacyPlaylistSource(id: string | null): SavedPlayFrom | null {
  return id === null
    ? null
    : { kind: "playlist", name: "", route: { name: "playlist", id }, track_ids: [] };
}

/// What started the current queue. Every play path that replaces the queue
/// sets it (null when the origin has no page, e.g. a single track); the
/// queue's Clear action clears it. Queue jumps and transport controls leave it
/// alone — they keep the same queue. `onPlay` fires whenever a playlist
/// becomes the source (never on clearing), letting the recency store count
/// every listened-to playlist.
export function createPlaySource(
  onPlay?: (uuid: string) => void,
  persist?: (playlistId: string | null, from: SavedPlayFrom | null) => void,
) {
  let from = $state<PlayFrom | null>(null);
  let trackIds = $state.raw<ReadonlySet<number>>(new Set());

  const playlistId = $derived(
    from?.kind === "playlist" && from.route?.name === "playlist" ? from.route.id : null,
  );

  function snapshot(): SavedPlayFrom | null {
    return from === null ? null : { ...from, track_ids: [...trackIds] };
  }

  function apply(next: PlayFrom | null, ids: Iterable<number>) {
    from = next;
    trackIds = new Set(next === null ? [] : ids);
  }

  return {
    get playlistId() { return playlistId; },
    get from() { return from; },
    /// The source to credit for `trackId`: null when the track was not part
    /// of what the source queued.
    fromFor(trackId: number): PlayFrom | null {
      return from !== null && trackIds.has(trackId) ? from : null;
    },
    set(next: PlayFrom | null, ids: Iterable<number> = []) {
      apply(next, ids);
      persist?.(playlistId, snapshot());
      if (playlistId !== null) onPlay?.(playlistId);
    },
    snapshot,
    /// Re-apply a saved source without recording a new play (session restore),
    /// and persist it so the saver cannot clobber the marker on its next write.
    restore(saved: SavedPlayFrom | null) {
      apply(saved === null ? null : { kind: saved.kind, name: saved.name, route: saved.route }, saved?.track_ids ?? []);
      persist?.(playlistId, snapshot());
    },
  };
}

export type PlaySource = ReturnType<typeof createPlaySource>;

export const playSource = createPlaySource(
  (id) => playlistRecency().record(id),
  (id, from) => { void api.setPlaySource(id, from).catch(() => {}); },
);
