import { api, trackToQueueItem, type Track } from "./api";
import type { Pager } from "./pager.svelte";
import { playSource, type PlayFrom } from "./playSource.svelte";
import { trackFlags } from "./trackFlags.svelte";
import type { TrackView } from "./trackView.svelte";

/// Plays started from a list that is still loading. The rows already loaded
/// play at once and the rest joins the queue once it has loaded
/// (`queueExtendContext`), so a click never waits for a whole collection
/// (issues #3 and #7). A play that does have to wait (a shuffle, a filtered
/// order) gives way to any desktop play made meanwhile (`claimPlay`).

export { claimPlay } from "./playClaims";

/// A play that started: its queue context and the track ids it queued.
export interface Started {
  context: number;
  ids: number[];
}

/// `"hidden"`: the content filters hide every track (the caller says so).
/// `"superseded"`: a newer play was claimed while this one waited.
export type StartResult = Started | "hidden" | "superseded";

export interface StartOptions {
  /// A fully shuffled context instead of one starting at `start`.
  shuffle?: boolean;
  /// From `claimPlay`; checked after every wait.
  isLatest?: () => boolean;
}

/// Play `list` from `start` with the content filters applied, credited to
/// `from` in the player bar's "From" line (none while a page is still loading
/// its name).
export async function startPlay(
  from: PlayFrom | null,
  list: readonly Track[],
  start: number,
  options: StartOptions = {},
): Promise<StartResult> {
  if (list.length === 0) return "hidden";
  const prepared = await trackFlags.filterPlayable(list, start);
  if (options.isLatest && !options.isLatest()) return "superseded";
  if (!prepared) return "hidden";
  const ids = prepared.items.map((track) => track.id);
  playSource.set(from, ids);
  const shuffle = options.shuffle ?? false;
  const context = await api.playTracks(
    prepared.items.map(trackToQueueItem),
    shuffle ? 0 : prepared.start,
    shuffle,
  );
  return { context, ids };
}

/// The tracks of `full` past `head`, the rows a play started from: the rest
/// of the list while it still starts with them (a playlist may repeat a
/// track), else every track not among them (it was refetched meanwhile).
export function restAfter(head: readonly Track[], full: readonly Track[]): Track[] {
  const prefix = head.length <= full.length && head.every((track, i) => full[i].id === track.id);
  if (prefix) return full.slice(head.length);
  const queued = new Set(head.map((track) => track.id));
  return full.filter((track) => !queued.has(track.id));
}

function sameSource(a: PlayFrom | null, b: PlayFrom): boolean {
  // By value: the store keeps its own proxied copy of `from`.
  return a !== null && a.kind === b.kind && a.name === b.name && JSON.stringify(a.route) === JSON.stringify(b.route);
}

/// Add `rest` to the context `started` began. The engine decides: it ignores
/// the rest once another play replaced that context, and only then is the
/// "From" credit left alone (a play that was clicked but never started does
/// not cancel it). Best effort: the rows already queued keep playing
/// whatever happens here.
export async function extendPlay(
  from: PlayFrom | null,
  started: Started,
  rest: readonly Track[],
): Promise<void> {
  try {
    if (rest.length === 0) return;
    const prepared = await trackFlags.filterPlayable(rest, 0);
    if (!prepared) return;
    const applied = await api.queueExtendContext(prepared.items.map(trackToQueueItem), started.context);
    if (applied && from !== null && sameSource(playSource.from, from)) {
      playSource.set(from, [...started.ids, ...prepared.items.map((track) => track.id)]);
    }
  } catch {
    // The rows already queued are playing; the rest is a bonus.
  }
}

/// Whether a play from `view` can start from the rows loaded so far: the list
/// is still loading and shown in its own order. A filter or sort needs the
/// whole list to know its order.
export function startsEarly(pager: Pager, view: Pick<TrackView, "engaged">): boolean {
  return !pager.done && !view.engaged;
}

/// Plays a page starts before its pager has the whole list (Tracks, a
/// playlist, an artist's All tracks): `play` starts the loaded rows and adds
/// the rest once loaded; `retry` is the list's Retry, which also finishes an
/// early play whose rest failed to load.
export function createEarlyPlays() {
  let unfinished: { pager: Pager; from: PlayFrom | null; head: Track[]; started: Started } | null = null;

  async function finish(pager: Pager, from: PlayFrom | null, head: Track[], started: Started) {
    await pager.ensureAll();
    // No error but not done: a Retry cleared it for its own load; wait on that.
    while (!pager.done && pager.error === "") await pager.ensureAll();
    if (!pager.done) {
      unfinished = { pager, from, head, started };
      return;
    }
    // The engine drops the rest if another play replaced this one meanwhile.
    await extendPlay(from, started, restAfter(head, pager.items));
  }

  return {
    /// Play `rows` (the pager's loaded rows, in order) from `start` now.
    /// Resolves once the play started; the rest joins in the background.
    async play(
      pager: Pager,
      from: PlayFrom | null,
      rows: readonly Track[],
      start: number,
      isLatest: () => boolean,
    ): Promise<StartResult> {
      // A copy: the pager appends to its live array as windows land.
      const head = [...rows];
      const started = await startPlay(from, head, start, { isLatest });
      if (typeof started !== "string") void finish(pager, from, head, started);
      return started;
    },
    retry(pager: Pager) {
      const pending = unfinished?.pager === pager ? unfinished : null;
      if (pending) {
        unfinished = null;
        void finish(pending.pager, pending.from, pending.head, pending.started);
      } else {
        void pager.loadMore();
      }
    },
  };
}
