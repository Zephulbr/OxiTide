import { SvelteMap } from "svelte/reactivity";
import { api, type Playlist, type Track } from "./api";
import type { Notice } from "./notice.svelte";

export interface PlaylistActions {
  readonly open: boolean;
  readonly step: "list" | "create" | "duplicates";
  /// True when the picker was opened from a track/album surface (create
  /// navigates to the new playlist); false when opened from Library.
  readonly fromPicker: boolean;
  readonly pendingTrackIds: readonly number[];
  /// Phase-1 skips awaiting a decision (true = "Add both").
  readonly duplicates: readonly Track[];
  readonly choices: SvelteMap<number, boolean>;
  readonly busy: boolean;
  /// Bumped after a successful create so mounted lists can refresh.
  readonly revision: number;
  openPicker(tracks: Track[]): void;
  openCreate(): void;
  startCreate(): void;
  close(): void;
  reset(): void;
  addTo(playlist: Playlist): Promise<void>;
  createAndAdd(name: string, description: string): Promise<Playlist | null>;
  setChoice(trackId: number, addBoth: boolean): void;
  setAllChoices(addBoth: boolean): void;
  confirmDuplicates(): Promise<void>;
}

type Step = "list" | "create" | "duplicates";

function dedupe(ids: number[]): number[] {
  return [...new Set(ids)];
}

/// The add-to-playlist picker and playlist creation flow. Track edits
/// (remove/move) are direct API calls in the views; creation, the two-phase
/// duplicate flow, and reset-on-logout live here.
export function createPlaylistActions(notice: Notice): PlaylistActions {
  let open = $state(false);
  let step = $state<Step>("list");
  let fromPicker = $state(false);
  let tracks = $state<Track[]>([]);
  let duplicates = $state<Track[]>([]);
  const choices = new SvelteMap<number, boolean>();
  let busy = $state(false);
  let revision = $state(0);
  let target = $state<Playlist | null>(null);

  function clearState() {
    step = "list";
    fromPicker = false;
    tracks = [];
    duplicates = [];
    choices.clear();
    target = null;
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

  async function addTo(playlist: Playlist) {
    if (busy) return;
    busy = true;
    target = playlist;
    const requested = dedupe(tracks.map((t) => t.id));
    try {
      const added = new Set(await api.addPlaylistTracks(playlist.uuid, requested, false));
      const skipped = requested.filter((id) => !added.has(id));
      if (skipped.length === 0) {
        notice.show(`${requested.length} added to ${playlist.title}`);
        close();
      } else {
        const skippedSet = new Set(skipped);
        duplicates = tracks.filter((t) => skippedSet.has(t.id));
        for (const t of duplicates) choices.set(t.id, false);
        step = "duplicates";
      }
    } catch (e) {
      notice.show(String(e));
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
      if (addBoth.length > 0) await api.addPlaylistTracks(target.uuid, addBoth, true);
      notice.show(
        `${addBoth.length} added, ${uniqueDuplicates.length - addBoth.length} kept one of`,
      );
      close();
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function createAndAdd(name: string, description: string): Promise<Playlist | null> {
    if (busy) return null;
    busy = true;
    try {
      const created = await api.createPlaylist(name, description);
      revision += 1;
      const ids = dedupe(tracks.map((t) => t.id));
      if (ids.length > 0) {
        try {
          await api.addPlaylistTracks(created.uuid, ids, false);
          notice.show(`Created ${created.title} with ${ids.length} tracks`);
        } catch (e) {
          notice.show(String(e));
        }
      } else {
        notice.show(`Created ${created.title}`);
      }
      close();
      return created;
    } catch (e) {
      notice.show(String(e));
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
    openPicker,
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
