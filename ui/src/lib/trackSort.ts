import type { Track } from "./api";
import { parseTidalTime } from "./playlistSort";

/// Multi-level track sorting (spec 2026-10-19-oxitide-multi-sort-design.md).
export type SortKey =
  | "title"
  | "artist"
  | "album"
  | "duration"
  | "quality"
  | "added"
  | "released"
  | "popularity"
  | "plays";
export type SortDirection = "asc" | "desc";
export interface SortLevel {
  key: SortKey;
  dir: SortDirection;
}

/// Which track table a list renders in; decides the keys it offers (MS-D11).
export type TableKind = "favorites" | "playlist" | "album" | "artist" | "mix" | "search";

/// At most this many levels; adding past it replaces the last (MS-D5).
export const MAX_LEVELS = 3;

export const SORT_KEYS: readonly SortKey[] = [
  "title",
  "artist",
  "album",
  "duration",
  "quality",
  "added",
  "released",
  "popularity",
  "plays",
];

export const SORT_LABELS: Record<SortKey, string> = {
  title: "Title",
  artist: "Artist",
  album: "Album",
  duration: "Time",
  quality: "Quality",
  added: "Date added",
  released: "Release date",
  popularity: "Popularity",
  plays: "Plays",
};

/// Text, time and quality ascend; dates and counts start newest/most (MS-D8).
export const DEFAULT_DIR: Record<SortKey, SortDirection> = {
  title: "asc",
  artist: "asc",
  album: "asc",
  duration: "asc",
  quality: "asc",
  added: "desc",
  released: "desc",
  popularity: "desc",
  plays: "desc",
};

/// Keys whose values need a fetch before the list can sort by them.
export const DATA_KEYS: readonly SortKey[] = ["released", "plays"];

export function needsData(levels: readonly SortLevel[]): boolean {
  return levels.some((level) => DATA_KEYS.includes(level.key));
}

/// Keys a table offers: only where its rows carry the data and rows can
/// differ (an album page shares one album and one release date; only
/// favorites and playlists know when a track was added).
export function keysFor(kind: TableKind): readonly SortKey[] {
  switch (kind) {
    case "favorites":
    case "playlist":
      return SORT_KEYS;
    case "album":
      return SORT_KEYS.filter((key) => key !== "album" && key !== "released" && key !== "added");
    case "artist":
    case "mix":
    case "search":
      return SORT_KEYS.filter((key) => key !== "added");
  }
}

/// Header click (MS-D6): a new key becomes the only level; the sole level
/// cycles default direction → flipped → off; a key already among several
/// levels flips in place so the other levels survive.
export function setPrimary(levels: readonly SortLevel[], key: SortKey): SortLevel[] {
  const at = levels.findIndex((level) => level.key === key);
  if (at < 0) return [{ key, dir: DEFAULT_DIR[key] }];
  if (levels.length === 1) {
    return levels[0].dir === DEFAULT_DIR[key] ? [{ key, dir: flip(levels[0].dir) }] : [];
  }
  return flipLevel(levels, at);
}

/// "+" / Shift+click: append as the next level (an active key flips instead);
/// past MAX_LEVELS the last level is replaced.
export function addLevel(levels: readonly SortLevel[], key: SortKey): SortLevel[] {
  const at = levels.findIndex((level) => level.key === key);
  if (at >= 0) return flipLevel(levels, at);
  const next = { key, dir: DEFAULT_DIR[key] };
  return levels.length >= MAX_LEVELS
    ? [...levels.slice(0, MAX_LEVELS - 1), next]
    : [...levels, next];
}

export function flipLevel(levels: readonly SortLevel[], index: number): SortLevel[] {
  return levels.map((level, i) => (i === index ? { key: level.key, dir: flip(level.dir) } : level));
}

export function removeLevel(levels: readonly SortLevel[], index: number): SortLevel[] {
  return levels.filter((_, i) => i !== index);
}

export function moveLevel(levels: readonly SortLevel[], from: number, to: number): SortLevel[] {
  if (from < 0 || from >= levels.length || to < 0 || to >= levels.length || from === to) {
    return [...levels];
  }
  const next = [...levels];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

function flip(dir: SortDirection): SortDirection {
  return dir === "asc" ? "desc" : "asc";
}

/// Data fetched for the `plays` / `released` keys (lib/sortData.svelte.ts).
export interface SortContext {
  readonly plays: ReadonlyMap<number, number>;
  /// Album id → `YYYY-MM-DD`; `null` when TIDAL has no date.
  readonly releaseDates: ReadonlyMap<number, string | null>;
}

export const EMPTY_CONTEXT: SortContext = { plays: new Map(), releaseDates: new Map() };

const QUALITY_RANK: Record<string, number> = {
  LOW: 0,
  HIGH: 1,
  LOSSLESS: 2,
  HI_RES: 3,
  HI_RES_LOSSLESS: 3,
};

export function qualityRank(track: Track): number | undefined {
  if (track.mediaMetadata?.tags.includes("HIRES_LOSSLESS")) return 3;
  return track.audioQuality ? QUALITY_RANK[track.audioQuality] : undefined;
}

export function releaseDateOf(track: Track, context: SortContext): string | undefined {
  const album = track.album;
  if (!album) return undefined;
  return album.releaseDate ?? context.releaseDates.get(album.id) ?? undefined;
}

type Value = string | number | undefined;

/// The value a key sorts by; `undefined` is "missing" and always sorts last.
export function sortValue(track: Track, key: SortKey, context: SortContext): Value {
  switch (key) {
    case "title":
      return track.title;
    case "artist":
      return track.artists[0]?.name;
    case "album":
      return track.album?.title;
    case "duration":
      return track.duration;
    case "quality":
      return qualityRank(track);
    case "added": {
      const time = parseTidalTime(track.dateAdded);
      return time === 0 ? undefined : time;
    }
    case "released":
      return releaseDateOf(track, context);
    case "popularity":
      return track.popularity ?? undefined;
    case "plays":
      return context.plays.get(track.id);
  }
}

const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

function compareValues(a: Value, b: Value): number {
  if (typeof a === "string" && typeof b === "string") return collator.compare(a, b);
  return (a as number) - (b as number);
}

/// Sort `list` by `levels` (stable copy). Missing values sit last in either
/// direction (MS-D9). With an Album level, rows of one album keep album
/// order (disc, then track number) before falling back to source order.
export function sortTracks(
  list: readonly Track[],
  levels: readonly SortLevel[],
  context: SortContext = EMPTY_CONTEXT,
): Track[] {
  if (levels.length === 0) return [...list];
  const albumOrder = levels.some((level) => level.key === "album");
  const factors = levels.map((level) => (level.dir === "asc" ? 1 : -1));
  return list
    .map((track, index) => ({
      track,
      index,
      values: levels.map((level) => sortValue(track, level.key, context)),
    }))
    .sort((a, b) => {
      for (let i = 0; i < levels.length; i++) {
        const left = a.values[i];
        const right = b.values[i];
        if (left === undefined || right === undefined) {
          if (left === right) continue;
          return left === undefined ? 1 : -1;
        }
        const cmp = compareValues(left, right);
        if (cmp !== 0) return cmp * factors[i];
      }
      if (albumOrder) {
        const cmp =
          (a.track.album?.id ?? 0) - (b.track.album?.id ?? 0) ||
          a.track.volumeNumber - b.track.volumeNumber ||
          a.track.trackNumber - b.track.trackNumber;
        if (cmp !== 0) return cmp;
      }
      return a.index - b.index;
    })
    .map((entry) => entry.track);
}
