import type { Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import type { MenuItem } from "./menu.svelte";

export interface TrackMenuContext {
  track: Track;
  index: number;
  trackCount: number;
  favorites: Favorites;
  play(): void | Promise<void>;
  addToQueue(): void;
  addToPlaylist(): void;
  playlist?: {
    onRemove(index: number): void;
    onMove(index: number, delta: -1 | 1): void;
    busy: boolean;
    disabled: boolean;
  };
}

/// Track row items. The playlist block is present only while the list is
/// editable; edge moves are disabled instead of hidden so the menu is stable.
export function trackMenuItems(ctx: TrackMenuContext): MenuItem[] {
  const items: MenuItem[] = [
    { id: "play", label: "Play", run: () => void ctx.play() },
    { id: "add-queue", label: "Add to queue", run: () => ctx.addToQueue() },
    { id: "add-playlist", label: "Add to playlist", run: () => ctx.addToPlaylist() },
    {
      id: "favorite",
      label: ctx.favorites.hasTrack(ctx.track.id) ? "Unfavorite" : "Favorite",
      run: () => void ctx.favorites.toggleTrack(ctx.track.id),
    },
  ];
  const playlist = ctx.playlist;
  if (playlist && !playlist.busy && !playlist.disabled) {
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
      { id: "remove", label: "Remove", run: () => playlist.onRemove(ctx.index) },
    );
  }
  return items;
}

export function queueMenuItems(ctx: {
  index: number;
  playNow(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "play-now", label: "Play now", run: () => ctx.playNow() },
    { id: "remove", label: "Remove", run: () => ctx.remove() },
  ];
}

export function libraryPlaylistMenuItems(ctx: {
  playlist: Playlist;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "open", label: "Open", run: () => ctx.open() },
    { id: "rename", label: "Rename", run: () => ctx.rename() },
    { id: "delete", label: "Delete", danger: true, run: () => ctx.remove() },
  ];
}
