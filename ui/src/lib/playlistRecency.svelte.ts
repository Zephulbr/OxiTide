export const RECENCY_KEY = "oxitide.playlist-recency";

const MAX_ENTRIES = 500;

type RecencyStorage = Pick<Storage, "getItem" | "setItem">;

export interface PlaylistRecency {
  /// Last-played epoch ms, or 0 when the playlist has never been played.
  rank(uuid: string): number;
  record(uuid: string): void;
}

function load(storage: RecencyStorage): Record<string, number> {
  const raw = storage.getItem(RECENCY_KEY);
  if (raw === null) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return {};
    const times: Record<string, number> = {};
    for (const [uuid, value] of Object.entries(parsed)) {
      if (typeof value === "number" && Number.isFinite(value)) times[uuid] = value;
    }
    return times;
  } catch {
    return {};
  }
}

/// Persisted last-played times for playlists. The factory takes its storage
/// and clock so tests can pass fakes; `playlistRecency()` binds the app-wide
/// singleton to localStorage.
export function createPlaylistRecency(
  storage: RecencyStorage,
  now: () => number = Date.now,
): PlaylistRecency {
  let times = $state<Record<string, number>>(load(storage));

  function persist() {
    storage.setItem(RECENCY_KEY, JSON.stringify(times));
  }

  function record(uuid: string) {
    times = { ...times, [uuid]: now() };
    const uuids = Object.keys(times);
    if (uuids.length > MAX_ENTRIES) {
      const dropped = uuids
        .sort((a, b) => times[a] - times[b])
        .slice(0, uuids.length - MAX_ENTRIES);
      const trimmed = { ...times };
      for (const key of dropped) delete trimmed[key];
      times = trimmed;
    }
    persist();
  }

  return {
    rank: (uuid) => times[uuid] ?? 0,
    record,
  };
}

let singleton: PlaylistRecency | null = null;

export function playlistRecency(): PlaylistRecency {
  singleton ??= createPlaylistRecency(window.localStorage);
  return singleton;
}
