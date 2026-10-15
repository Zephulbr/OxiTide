import type { CollectionFolder, Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import type { MenuItem } from "./menu.svelte";

export interface TrackMenuContext {
  track: Track;
  index: number;
  favorites: Favorites;
  play(): void | Promise<void>;
  /// Starts a TIDAL track radio, replacing the queue.
  startRadio(): void;
  /// Inserts the track right after the playing one.
  playNext(): void;
  addToQueue(): void;
  addToPlaylist(): void;
  copyTitle(): void;
  /// Opens the Info surface: the playing track routes to Now Playing's Info
  /// tab, any other row opens the modal (host decides).
  trackInfo(): void;
  /// True when the active content filters hide this track.
  blocked?: boolean;
  playlist?: {
    onRemove(index: number): void;
    busy: boolean;
    disabled: boolean;
  };
}

/// Track row items, grouped playback / library / utility with the playlist
/// Remove last. The playlist block is present only while the list is editable
/// (reordering is drag-and-drop only).
export function trackMenuItems(ctx: TrackMenuContext): MenuItem[] {
  const playbackBlocked = ctx.blocked === true;
  const favorited = ctx.favorites.hasTrack(ctx.track.id);
  const items: MenuItem[] = [
    {
      id: "play",
      label: "Play",
      icon: "play",
      disabled: playbackBlocked,
      run: () => void ctx.play(),
    },
    {
      id: "radio",
      label: "Track Radio",
      icon: "radio",
      disabled: playbackBlocked,
      run: () => ctx.startRadio(),
    },
    {
      id: "play-next",
      label: "Play next",
      icon: "skip-forward",
      disabled: playbackBlocked,
      run: () => ctx.playNext(),
    },
    {
      id: "add-queue",
      label: "Add to queue",
      icon: "plus",
      disabled: playbackBlocked,
      run: () => ctx.addToQueue(),
    },
    {
      id: "add-playlist",
      label: "Add to playlist",
      icon: "list-plus",
      divider: true,
      run: () => ctx.addToPlaylist(),
    },
    {
      id: "favorite",
      label: favorited ? "Unfavorite" : "Favorite",
      icon: favorited ? "heart-crack" : "heart",
      run: () => void ctx.favorites.toggleTrack(ctx.track.id),
    },
    {
      id: "copy-title",
      label: "Copy title",
      icon: "copy",
      divider: true,
      run: () => ctx.copyTitle(),
    },
    { id: "track-info", label: "Track info", icon: "info", run: () => ctx.trackInfo() },
  ];
  const playlist = ctx.playlist;
  if (playlist && !playlist.busy && !playlist.disabled) {
    items.push({
      id: "remove",
      label: "Remove",
      icon: "trash-2",
      danger: true,
      divider: true,
      run: () => playlist.onRemove(ctx.index),
    });
  }
  return items;
}

export function queueMenuItems(ctx: {
  index: number;
  playNow(): void;
  /// Move the row to right after the current track; absent when it is the
  /// current or already the next track.
  playNext?: () => void;
  copyTitle(): void;
  remove(): void;
}): MenuItem[] {
  const items: MenuItem[] = [
    { id: "play-now", label: "Play now", icon: "play", run: () => ctx.playNow() },
  ];
  const playNext = ctx.playNext;
  if (playNext) {
    items.push({ id: "play-next", label: "Play next", icon: "skip-forward", run: () => playNext() });
  }
  items.push(
    { id: "copy-title", label: "Copy title", icon: "copy", divider: true, run: () => ctx.copyTitle() },
    { id: "remove", label: "Remove", icon: "trash-2", danger: true, divider: true, run: () => ctx.remove() },
  );
  return items;
}

/// The artist-page overflow ("More") menu: Share always, the artist mix when
/// the parsed v1 payload exposes one (spec §6.3).
export function artistMoreItems(ctx: {
  share(): void;
  mixId?: string | null;
  openMix(id: string): void;
}): MenuItem[] {
  const items: MenuItem[] = [{ id: "share", label: "Share", icon: "share-2", run: () => ctx.share() }];
  if (ctx.mixId) {
    const mixId = ctx.mixId;
    items.push({ id: "artist-mix", label: "Artist mix", icon: "radio", run: () => ctx.openMix(mixId) });
  }
  return items;
}

export function libraryPlaylistMenuItems(ctx: {
  playlist: Playlist;
  editable: boolean;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  const items: MenuItem[] = [
    { id: "open", label: "Open", icon: "external-link", run: () => ctx.open() },
  ];
  if (ctx.editable) {
    items.push(
      { id: "rename", label: "Rename", icon: "pencil", run: () => ctx.rename() },
      { id: "delete", label: "Delete", icon: "trash-2", danger: true, divider: true, run: () => ctx.remove() },
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
    { id: "open", label: "Open", icon: "external-link", run: () => ctx.open() },
    { id: "rename", label: "Rename", icon: "pencil", run: () => ctx.rename() },
    { id: "remove", label: "Remove", icon: "trash-2", danger: true, divider: true, run: () => ctx.remove() },
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
    { id: "open", label: "Open", icon: "external-link", run: () => ctx.open() },
    { id: "move", label: "Move to folder…", icon: "folder", run: () => ctx.move() },
  ];
  if (ctx.playlist.owned) {
    items.push(
      { id: "rename", label: "Rename", icon: "pencil", run: () => ctx.rename() },
      { id: "delete", label: "Delete", icon: "trash-2", danger: true, divider: true, run: () => ctx.remove() },
    );
  } else {
    items.push({
      id: "remove",
      label: "Remove from collection",
      icon: "trash-2",
      danger: true,
      divider: true,
      run: () => ctx.remove(),
    });
  }
  return items;
}
