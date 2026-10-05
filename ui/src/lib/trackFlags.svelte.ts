import { SvelteMap } from "svelte/reactivity";
import { api } from "./api";

export interface TrackFlagInfo {
  explicit: boolean | null;
  ai: boolean | null;
  /// True once a lookup for this id has completed, even if the API returned
  /// no attributes; prevents re-requesting permanently unknown ids.
  fetched: boolean;
}

/// Anything with an id and, when known from the Track DTO, an explicit flag.
export interface TrackLike {
  id: number;
  explicit?: boolean;
}

export interface Playable<T> {
  items: T[];
  start: number;
}

export interface TrackFlagsStore {
  readonly blockExplicit: boolean;
  readonly blockAi: boolean;
  readonly showLabels: boolean;
  init(): Promise<void>;
  setContentFlags(blockExplicit: boolean, blockAi: boolean): void;
  setShowLabels(show: boolean): void;
  recordTracks(tracks: readonly TrackLike[]): void;
  ensure(ids: readonly number[]): Promise<void>;
  isBlocked(track: TrackLike): boolean;
  blockReasons(track: TrackLike): ("explicit" | "ai")[];
  labels(track: TrackLike): ("explicit" | "ai")[];
  filterPlayable<T extends TrackLike>(
    tracks: readonly T[],
    start: number,
  ): Promise<Playable<T> | null>;
  reset(): void;
}

/// Index of the original start track in the kept list, or of the next kept
/// track, clamped to the last kept item.
export function remapStart(ids: readonly number[], kept: readonly number[], start: number): number {
  if (kept.length === 0 || start >= ids.length) return 0;
  const exact = kept.indexOf(ids[start]);
  if (exact >= 0) return exact;
  const before = ids.slice(0, start).filter((id) => kept.includes(id)).length;
  return Math.min(before, kept.length - 1);
}

export function createTrackFlags(): TrackFlagsStore {
  let blockExplicit = $state(false);
  let blockAi = $state(false);
  let showLabels = $state(true);
  const flags = new SvelteMap<number, TrackFlagInfo>();
  const pending = new Set<number>();
  let flushing: Promise<void> | null = null;

  function known(id: number): boolean {
    return flags.get(id)?.fetched === true;
  }

  async function init(): Promise<void> {
    try {
      const config = await api.getConfig();
      blockExplicit = config.content.block_explicit;
      blockAi = config.content.block_ai;
      showLabels = config.content.show_labels;
    } catch {
      // Defaults stay off; enforcement then matches a fresh config.
    }
  }

  function setContentFlags(explicit: boolean, ai: boolean) {
    blockExplicit = explicit;
    blockAi = ai;
  }

  function setShowLabels(show: boolean) {
    showLabels = show;
  }

  function recordTracks(tracks: readonly TrackLike[]) {
    for (const track of tracks) {
      if (track.explicit === undefined) continue;
      const entry = flags.get(track.id);
      if (entry?.explicit === track.explicit) continue;
      flags.set(track.id, {
        explicit: track.explicit,
        ai: entry?.ai ?? null,
        fetched: entry?.fetched ?? false,
      });
    }
  }

  async function ensure(ids: readonly number[]): Promise<void> {
    // Enrich for blocking or for tagging; labels-off and no filter is a no-op.
    if (!blockAi && !showLabels) return;
    for (const id of ids) if (!known(id)) pending.add(id);
    if (pending.size === 0) {
      await flushing;
      return;
    }
    if (!flushing) {
      flushing = (async () => {
        // Let same-tick callers join this batch before it is snapshotted.
        await Promise.resolve();
        const batch = [...pending];
        pending.clear();
        try {
          const rows = await api.getTrackFlags(batch);
          for (const row of rows) {
            const entry = flags.get(row.track_id);
            flags.set(row.track_id, {
              explicit: row.explicit ?? entry?.explicit ?? null,
              ai: row.ai ?? entry?.ai ?? null,
              fetched: true,
            });
          }
        } catch {
          // Fail open: unknown flags never block playback.
        } finally {
          flushing = null;
        }
      })();
    }
    await flushing;
    if (pending.size > 0) await ensure([]);
  }

  function blockReasons(track: TrackLike): ("explicit" | "ai")[] {
    const entry = flags.get(track.id);
    const reasons: ("explicit" | "ai")[] = [];
    if (blockExplicit && entry?.explicit === true) reasons.push("explicit");
    if (blockAi && entry?.ai === true) reasons.push("ai");
    return reasons;
  }

  function isBlocked(track: TrackLike): boolean {
    return blockReasons(track).length > 0;
  }

  /// Known flags for tagging, independent of the active filters. The DTO's
  /// explicit value covers rows before enrichment lands.
  function labels(track: TrackLike): ("explicit" | "ai")[] {
    const entry = flags.get(track.id);
    const explicit = entry?.explicit ?? track.explicit ?? null;
    const reasons: ("explicit" | "ai")[] = [];
    if (explicit === true) reasons.push("explicit");
    if (entry?.ai === true) reasons.push("ai");
    return reasons;
  }

  async function filterPlayable<T extends TrackLike>(
    tracks: readonly T[],
    start: number,
  ): Promise<Playable<T> | null> {
    recordTracks(tracks);
    if (blockAi) await ensure(tracks.map((track) => track.id));
    const items = tracks.filter((track) => !isBlocked(track));
    if (items.length === 0) return null;
    const kept = items.map((track) => track.id);
    return { items, start: remapStart(tracks.map((track) => track.id), kept, start) };
  }

  // The toggles mirror global config, not account state, so they survive a
  // logout reset; only the per-account flag cache is dropped.
  function reset() {
    flags.clear();
    pending.clear();
    flushing = null;
  }

  return {
    get blockExplicit() {
      return blockExplicit;
    },
    get blockAi() {
      return blockAi;
    },
    get showLabels() {
      return showLabels;
    },
    init,
    setContentFlags,
    setShowLabels,
    recordTracks,
    ensure,
    isBlocked,
    blockReasons,
    labels,
    filterPlayable,
    reset,
  };
}

/// One app-wide instance; views and TrackTable consume it directly.
export const trackFlags = createTrackFlags();
