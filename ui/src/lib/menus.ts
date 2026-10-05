import type { CollectionFolder, Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import type { MenuItem } from "./menu.svelte";

export interface TrackMenuContext {
  track: Track;
  index: number;
  trackCount: number;
  favorites: Favorites;
  play(): void | Promise<void>;
  /// Starts a TIDAL track radio, replacing the queue.
  startRadio(): void;
  addToQueue(): void;
  addToPlaylist(): void;
  copyTitle(): void;
  /// True when the active content filters hide this track.
  blocked?: boolean;
  playlist?: {
    onRemove(index: number): void;
    onMove(index: number, delta: -1 | 1): void;
    busy: boolean;
    disabled: boolean;
    /// false omits move-up/move-down but keeps Remove (sorted/filtered views).
    canMove?: boolean;
  };
}

/// Track row items. The playlist block is present only while the list is
/// editable; edge moves are disabled instead of hidden so the menu is stable.
export function trackMenuItems(ctx: TrackMenuContext): MenuItem[] {
  const playbackBlocked = ctx.blocked === true;
  const items: MenuItem[] = [
    { id: "play", label: "Play", disabled: playbackBlocked, run: () => void ctx.play() },
    {
      id: "radio",
      label: "Track Radio",
      disabled: playbackBlocked,
      run: () => ctx.startRadio(),
    },
    {
      id: "add-queue",
      label: "Add to queue",
      disabled: playbackBlocked,
      run: () => ctx.addToQueue(),
    },
    { id: "add-playlist", label: "Add to playlist", run: () => ctx.addToPlaylist() },
    {
      id: "favorite",
      label: ctx.favorites.hasTrack(ctx.track.id) ? "Unfavorite" : "Favorite",
      run: () => void ctx.favorites.toggleTrack(ctx.track.id),
    },
    { id: "copy-title", label: "Copy title", run: () => ctx.copyTitle() },
  ];
  const playlist = ctx.playlist;
  if (playlist && !playlist.busy && !playlist.disabled) {
    if (playlist.canMove !== false) {
      items.push(
        {
          id: "move-up",
          label: "Move up",
          disabled: ctx.index === 0,
          run: () => playlist.onMove(ctx.index, -1),
        },
        {
          id: "move-down",
          label: "Move down",
          disabled: ctx.index === ctx.trackCount - 1,
          run: () => playlist.onMove(ctx.index, 1),
        },
      );
    }
    items.push({ id: "remove", label: "Remove", run: () => playlist.onRemove(ctx.index) });
  }
  return items;
}

export function queueMenuItems(ctx: {
  index: number;
  playNow(): void;
  copyTitle(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "play-now", label: "Play now", run: () => ctx.playNow() },
    { id: "copy-title", label: "Copy title", run: () => ctx.copyTitle() },
    { id: "remove", label: "Remove", run: () => ctx.remove() },
  ];
}

export function libraryPlaylistMenuItems(ctx: {
  playlist: Playlist;
  editable: boolean;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  const items: MenuItem[] = [{ id: "open", label: "Open", run: () => ctx.open() }];
  if (ctx.editable) {
    items.push(
      { id: "rename", label: "Rename", run: () => ctx.rename() },
      { id: "delete", label: "Delete", danger: true, run: () => ctx.remove() },
    );
  }
  return items;
}

export function collectionFolderMenuItems(ctx: {
  folder: CollectionFolder;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "open", label: "Open", run: () => ctx.open() },
    { id: "rename", label: "Rename", run: () => ctx.rename() },
    { id: "remove", label: "Remove", danger: true, run: () => ctx.remove() },
  ];
}

export function collectionPlaylistMenuItems(ctx: {
  playlist: Playlist;
  open(): void;
  rename(): void;
  remove(): void;
  move(): void;
}): MenuItem[] {
  const items: MenuItem[] = [
    { id: "open", label: "Open", run: () => ctx.open() },
    { id: "move", label: "Move to folder…", run: () => ctx.move() },
  ];
  if (ctx.playlist.owned) {
    items.push(
      { id: "rename", label: "Rename", run: () => ctx.rename() },
      { id: "delete", label: "Delete", danger: true, run: () => ctx.remove() },
    );
  } else {
    items.push({
      id: "remove",
      label: "Remove from collection",
      danger: true,
      run: () => ctx.remove(),
    });
  }
  return items;
}
