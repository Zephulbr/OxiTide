import { SvelteMap } from "svelte/reactivity";
import { api, type Playlist, type Track } from "./api";
import { errorMessage } from "./errors";
import { invalidatePlaylistCovers } from "./playlistArt.svelte";
import type { Toasts } from "./toasts.svelte";

export interface PlaylistActions {
  readonly open: boolean;
  readonly step: Step;
  /// True when the picker was opened from a track/album surface (create
  /// navigates to the new playlist); false when opened from Library.
  readonly fromPicker: boolean;
  readonly pendingTrackIds: readonly number[];
  /// Phase-1 skips awaiting a decision (true = "Add both").
  readonly duplicates: readonly Track[];
  readonly choices: SvelteMap<number, boolean>;
  readonly busy: boolean;
  /// Bumped after a successful create or add so mounted lists (the playlist
  /// tree's counts, the Playlists grid) can refresh.
  readonly revision: number;
  /// The playlist that last received tracks, with a sequence number so an
  /// open Playlist view reloads even when the same playlist gets two adds.
  readonly lastAdded: { uuid: string; seq: number } | null;
  /// The pending add at the confirm step: where it goes, how many distinct
  /// tracks, and the collection they came from (when one was dropped).
  readonly confirming: { playlist: PlaylistRef; count: number; from: string | null } | null;
  /// The playlist an add in progress is going to (confirm and duplicates steps).
  readonly target: PlaylistRef | null;
  openPicker(tracks: Track[]): void;
  /// Add straight to `playlist` without the list step (a drag dropped on
  /// it). `tracks` may be a loader (a dropped album), whose failure is
  /// reported like the add's. With `confirm`, more than one track stops at
  /// the confirm step first (`from` names the dropped collection there).
  /// Phase 1 skips duplicates; anything skipped opens the picker at its
  /// duplicates step. `onOpen` puts an Open action on the success toast.
  addTracks(
    playlist: PlaylistRef,
    tracks: Track[] | (() => Promise<Track[]>),
    options?: AddOptions,
  ): Promise<void>;
  /// Runs the add waiting at the confirm step.
  confirmAdd(): Promise<void>;
  openCreate(): void;
  startCreate(): void;
  close(): void;
  reset(): void;
  addTo(playlist: Playlist): Promise<void>;
  createAndAdd(name: string, description: string, isPublic: boolean): Promise<Playlist | null>;
  setChoice(trackId: number, addBoth: boolean): void;
  setAllChoices(addBoth: boolean): void;
  confirmDuplicates(): Promise<void>;
}

type Step = "list" | "create" | "confirm" | "duplicates";

/// The parts of a playlist an add needs.
export type PlaylistRef = Pick<Playlist, "uuid" | "title">;

export interface AddOptions {
  onOpen?: () => void;
  /// Ask before adding more than one track (a drop that may have missed).
  confirm?: boolean;
  /// The dropped collection's name, for the confirm step.
  from?: string;
}

function dedupe(ids: number[]): number[] {
  return [...new Set(ids)];
}

/// The add-to-playlist picker and playlist creation flow. Track edits
/// (remove/move) are direct API calls in the views; creation, the two-phase
/// duplicate flow, and reset-on-logout live here. Outcomes surface as toasts.
export function createPlaylistActions(toasts: Toasts): PlaylistActions {
  let open = $state(false);
  let step = $state<Step>("list");
  let fromPicker = $state(false);
  let tracks = $state<Track[]>([]);
  let duplicates = $state<Track[]>([]);
  const choices = new SvelteMap<number, boolean>();
  let busy = $state(false);
  let revision = $state(0);
  let lastAdded = $state<{ uuid: string; seq: number } | null>(null);
  let addSeq = 0;
  let target = $state<PlaylistRef | null>(null);
  // The confirm step's collection name and the success toast's Open action.
  let confirmFrom = $state<string | null>(null);
  let onOpenAfter: (() => void) | undefined;

  function clearState() {
    step = "list";
    fromPicker = false;
    tracks = [];
    duplicates = [];
    choices.clear();
    target = null;
    confirmFrom = null;
    onOpenAfter = undefined;
  }

  function openPicker(next: Track[]) {
    clearState();
    fromPicker = true;
    tracks = next;
    open = true;
  }

  function openCreate() {
    clearState();
    step = "create";
    open = true;
  }

  function startCreate() {
    step = "create";
  }

  function close() {
    open = false;
    clearState();
  }

  /// Mounted lists refresh their counts; an open view of the playlist reloads.
  function noteAdded(uuid: string) {
    invalidatePlaylistCovers(uuid);
    revision += 1;
    lastAdded = { uuid, seq: ++addSeq };
  }

  /// Phase 1 of an add of `tracks`: duplicates are skipped. Returns how many
  /// were requested when all landed; otherwise null, with the skipped tracks
  /// waiting at the duplicates step for `playlist`.
  async function addSkippingDuplicates(playlist: PlaylistRef): Promise<number | null> {
    target = playlist;
    const requested = dedupe(tracks.map((t) => t.id));
    const added = new Set(await api.addPlaylistTracks(playlist.uuid, requested, false));
    if (added.size > 0) noteAdded(playlist.uuid);
    else invalidatePlaylistCovers(playlist.uuid);
    const skipped = requested.filter((id) => !added.has(id));
    if (skipped.length === 0) return requested.length;
    const skippedSet = new Set(skipped);
    const seen = new Set<number>();
    duplicates = tracks.filter((t) => {
      if (!skippedSet.has(t.id) || seen.has(t.id)) return false;
      seen.add(t.id);
      return true;
    });
    for (const t of duplicates) choices.set(t.id, false);
    step = "duplicates";
    return null;
  }

  async function addTo(playlist: Playlist) {
    if (busy) return;
    busy = true;
    try {
      const count = await addSkippingDuplicates(playlist);
      if (count !== null) {
        toasts.show(`${count} added to ${playlist.title}`, { severity: "success" });
        close();
      }
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  /// Adds the pending `tracks` to `playlist` and reports it: a success toast
  /// (with Open), or the duplicates step left open for what was skipped.
  async function addPending(playlist: PlaylistRef) {
    try {
      const count = await addSkippingDuplicates(playlist);
      if (count === null) {
        open = true;
        return;
      }
      const message =
        count === 1 ? `Added "${tracks[0].title}" to ${playlist.title}` : `Added ${count} tracks to ${playlist.title}`;
      const onOpen = onOpenAfter;
      toasts.show(message, {
        severity: "success",
        action: onOpen ? { label: "Open", run: onOpen } : undefined,
      });
      close();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      close();
    }
  }

  async function addTracks(
    playlist: PlaylistRef,
    source: Track[] | (() => Promise<Track[]>),
    options: AddOptions = {},
  ) {
    if (open) return;
    if (busy) {
      toasts.show("Still adding the last tracks; try again in a moment.", { severity: "info" });
      return;
    }
    busy = true;
    clearState();
    try {
      const next = typeof source === "function" ? await source() : source;
      if (next.length === 0) return;
      tracks = next;
      onOpenAfter = options.onOpen;
      if (options.confirm && dedupe(next.map((t) => t.id)).length > 1) {
        target = playlist;
        confirmFrom = options.from ?? null;
        step = "confirm";
        open = true;
        return;
      }
      await addPending(playlist);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      clearState();
    } finally {
      busy = false;
    }
  }

  async function confirmAdd() {
    if (busy || step !== "confirm" || !target) return;
    busy = true;
    try {
      await addPending(target);
    } finally {
      busy = false;
    }
  }

  async function confirmDuplicates() {
    if (busy || !target) return;
    const addBoth = dedupe(
      duplicates.filter((t) => choices.get(t.id) === true).map((t) => t.id),
    );
    const uniqueDuplicates = dedupe(duplicates.map((t) => t.id));
    busy = true;
    try {
      if (addBoth.length > 0) {
        await api.addPlaylistTracks(target.uuid, addBoth, true);
        noteAdded(target.uuid);
      }
      const kept = uniqueDuplicates.length - addBoth.length;
      const copies = addBoth.length === 1 ? "1 copy" : `${addBoth.length} copies`;
      const message =
        addBoth.length === 0
          ? `Kept one of ${kept === 1 ? "the duplicate" : `${kept} duplicates`}`
          : kept === 0
            ? `Added ${copies}`
            : `Added ${copies}, kept one of ${kept}`;
      toasts.show(message, { severity: "info" });
      close();
    } catch (e) {
      toasts.show(`Some tracks were already added, but the duplicate update failed: ${errorMessage(e)}`, {
        severity: "error",
      });
    } finally {
      busy = false;
    }
  }

  async function createAndAdd(
    name: string,
    description: string,
    isPublic: boolean,
  ): Promise<Playlist | null> {
    if (busy) return null;
    busy = true;
    try {
      const created = await api.createPlaylist(name, description);
      revision += 1;
      if (isPublic) {
        try {
          await api.setPlaylistPublic(created.uuid, true);
        } catch (e) {
          toasts.show(
            `Created ${created.title}, but making it public failed: ${errorMessage(e)}`,
            { severity: "error" },
          );
        }
      }
      const ids = dedupe(tracks.map((t) => t.id));
      if (ids.length > 0) {
        const noun = ids.length === 1 ? "track" : "tracks";
        try {
          await api.addPlaylistTracks(created.uuid, ids, false);
          invalidatePlaylistCovers(created.uuid);
          toasts.show(`Created ${created.title} with ${ids.length} ${noun}`, {
            severity: "success",
          });
        } catch (e) {
          toasts.show(
            `Created ${created.title}, but adding ${ids.length} ${noun} failed: ${errorMessage(e)}`,
            { severity: "error" },
          );
        }
      } else {
        toasts.show(`Created ${created.title}`, { severity: "success" });
      }
      close();
      return created;
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      return null;
    } finally {
      busy = false;
    }
  }

  return {
    get open() {
      return open;
    },
    get step() {
      return step;
    },
    get fromPicker() {
      return fromPicker;
    },
    get pendingTrackIds() {
      return tracks.map((t) => t.id);
    },
    get duplicates() {
      return duplicates;
    },
    get choices() {
      return choices;
    },
    get busy() {
      return busy;
    },
    get revision() {
      return revision;
    },
    get lastAdded() {
      return lastAdded;
    },
    get target() {
      return target;
    },
    get confirming() {
      if (step !== "confirm" || !target) return null;
      return { playlist: target, count: dedupe(tracks.map((t) => t.id)).length, from: confirmFrom };
    },
    openPicker,
    addTracks,
    confirmAdd,
    openCreate,
    startCreate,
    close,
    reset: close,
    addTo,
    createAndAdd,
    setChoice: (trackId, addBoth) => {
      choices.set(trackId, addBoth);
    },
    setAllChoices: (addBoth) => {
      for (const t of duplicates) choices.set(t.id, addBoth);
    },
    confirmDuplicates,
  };
}
