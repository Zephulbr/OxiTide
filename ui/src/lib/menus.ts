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
  /// Opens the track info modal.
  trackInfo(): void;
  /// Opens the track's album; absent when it has none or it is the current
  /// page (name-links spec NL-D6).
  goToAlbum?: () => void;
  /// Artists to offer "Go to" entries for, already filtered (`menuArtists`).
  goToArtists?: { name: string; open(): void }[];
  /// True when the active content filters hide this track.
  blocked?: boolean;
  /// Save for Later state; absent where the surface does not offer it.
  later?: LaterToggle;
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
    ...laterItems(ctx.later),
    ...goToItems(ctx),
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

/// A track table's multi-row selection (right-click inside a selection of
/// two or more). Remove is present only while the playlist is editable.
export function selectionMenuItems(ctx: {
  count: number;
  playNext(): void;
  addToQueue(): void;
  /// Absent where the table has no playlist picker.
  addToPlaylist?: () => void;
  remove?: () => void;
}): MenuItem[] {
  const n = ctx.count;
  const items: MenuItem[] = [
    { id: "play-next", label: `Play ${n} next`, icon: "skip-forward", run: () => ctx.playNext() },
    { id: "add-queue", label: `Add ${n} to queue`, icon: "plus", run: () => ctx.addToQueue() },
  ];
  const addToPlaylist = ctx.addToPlaylist;
  if (addToPlaylist) {
    items.push({
      id: "add-playlist",
      label: `Add ${n} to playlist`,
      icon: "list-plus",
      divider: true,
      run: () => addToPlaylist(),
    });
  }
  const remove = ctx.remove;
  if (remove) {
    items.push({
      id: "remove",
      label: `Remove ${n} from playlist`,
      icon: "trash-2",
      danger: true,
      divider: true,
      run: () => remove(),
    });
  }
  return items;
}

interface GoToTargets {
  goToAlbum?: () => void;
  goToArtists?: { name: string; open(): void }[];
}

/// Whether the item is in Save for Later, and the toggle.
export interface LaterToggle {
  active: boolean;
  toggle(): void;
}

/// The Save for Later entry, labelled by state (spec SL-D4); none when the
/// surface has no toggle.
export function laterItems(later: LaterToggle | undefined): MenuItem[] {
  if (!later) return [];
  return [
    {
      id: "later",
      label: later.active ? "Remove from Save for later" : "Save for later",
      icon: later.active ? "bookmark-minus" : "bookmark",
      run: () => later.toggle(),
    },
  ];
}

/// The navigation group: album, then one "Go to artist" or a "Go to {name}"
/// per artist when there are several.
function goToItems(ctx: GoToTargets): MenuItem[] {
  const items: MenuItem[] = [];
  const goToAlbum = ctx.goToAlbum;
  if (goToAlbum) items.push({ id: "go-album", label: "Go to album", icon: "disc-3", run: () => goToAlbum() });
  const artists = ctx.goToArtists ?? [];
  artists.forEach((artist, i) => {
    items.push({
      id: artists.length === 1 ? "go-artist" : `go-artist-${i}`,
      label: artists.length === 1 ? "Go to artist" : `Go to ${artist.name}`,
      icon: "user",
      run: () => artist.open(),
    });
  });
  if (items.length > 0) items[0].divider = true;
  return items;
}

/// The player bar's playing track (right-click its title, artist or "From"
/// line): what a track row offers, minus playing it again or queueing it.
export function nowPlayingMenuItems(ctx: GoToTargets & {
  favorited: boolean;
  toggleFavorite(): void;
  /// Absent where the bar has no toasts to report a radio failure.
  startRadio?: () => void;
  /// Absent where the bar has no playlist picker.
  addToPlaylist?: () => void;
  later?: LaterToggle;
  copyTitle(): void;
  trackInfo(): void;
}): MenuItem[] {
  const items: MenuItem[] = [];
  const startRadio = ctx.startRadio;
  if (startRadio) items.push({ id: "radio", label: "Track Radio", icon: "radio", run: () => startRadio() });
  const addToPlaylist = ctx.addToPlaylist;
  if (addToPlaylist) {
    items.push({ id: "add-playlist", label: "Add to playlist", icon: "list-plus", run: () => addToPlaylist() });
  }
  items.push({
    id: "favorite",
    label: ctx.favorited ? "Unfavorite" : "Favorite",
    icon: ctx.favorited ? "heart-crack" : "heart",
    divider: items.length > 0,
    run: () => ctx.toggleFavorite(),
  });
  items.push(
    ...laterItems(ctx.later),
    ...goToItems(ctx),
    { id: "copy-title", label: "Copy title", icon: "copy", divider: true, run: () => ctx.copyTitle() },
    { id: "track-info", label: "Track info", icon: "info", run: () => ctx.trackInfo() },
  );
  return items;
}

/// Queue rows. The "Go to" group is the keyboard path to the row's nested
/// name links (name-links spec NL-D10).
export function queueMenuItems(ctx: GoToTargets & {
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
  items.push(...goToItems(ctx));
  items.push(
    { id: "copy-title", label: "Copy title", icon: "copy", divider: true, run: () => ctx.copyTitle() },
    { id: "remove", label: "Remove", icon: "trash-2", danger: true, divider: true, run: () => ctx.remove() },
  );
  return items;
}

/// Album / playlist / mix cards: playback, then library, the artists, and
/// the link. Favorite is present for albums only (playlists and mixes have no
/// favorite toggle here).
export function collectionCardMenuItems(ctx: GoToTargets & {
  play(): void;
  shuffle(): void;
  playNext(): void;
  addToQueue(): void;
  addToPlaylist(): void;
  favorite?: { active: boolean; toggle(): void };
  later?: LaterToggle;
  copyLink(): void;
}): MenuItem[] {
  const items: MenuItem[] = [
    { id: "play", label: "Play", icon: "play", run: () => ctx.play() },
    { id: "shuffle", label: "Shuffle", icon: "shuffle", run: () => ctx.shuffle() },
    { id: "play-next", label: "Play next", icon: "skip-forward", run: () => ctx.playNext() },
    { id: "add-queue", label: "Add to queue", icon: "plus", run: () => ctx.addToQueue() },
    {
      id: "add-playlist",
      label: "Add to playlist",
      icon: "list-plus",
      divider: true,
      run: () => ctx.addToPlaylist(),
    },
  ];
  const favorite = ctx.favorite;
  if (favorite) {
    items.push({
      id: "favorite",
      label: favorite.active ? "Unfavorite" : "Favorite",
      icon: favorite.active ? "heart-crack" : "heart",
      run: () => favorite.toggle(),
    });
  }
  items.push(
    ...laterItems(ctx.later),
    ...goToItems(ctx),
    { id: "copy-link", label: "Copy link", icon: "link", divider: true, run: () => ctx.copyLink() },
  );
  return items;
}

/// History rows: play plus the "Go to" group, the keyboard path to the row's
/// nested name links (name-links spec NL-D10).
export function historyMenuItems(ctx: GoToTargets & { play(): void }): MenuItem[] {
  return [{ id: "play", label: "Play", icon: "play", run: () => ctx.play() }, ...goToItems(ctx)];
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
    // Name and the (local) description edit together.
    { id: "rename", label: "Edit folder…", icon: "pencil", run: () => ctx.rename() },
    { id: "remove", label: "Remove", icon: "trash-2", danger: true, divider: true, run: () => ctx.remove() },
  ];
}

export function collectionPlaylistMenuItems(ctx: {
  playlist: Playlist;
  open(): void;
  rename(): void;
  remove(): void;
  move(): void;
  later?: LaterToggle;
  /// Opens the Collaborators dialog; offered on owned playlists only (CO-D1).
  collaborators?: () => void;
}): MenuItem[] {
  const items: MenuItem[] = [
    { id: "open", label: "Open", icon: "external-link", run: () => ctx.open() },
    { id: "move", label: "Move to folder…", icon: "folder", run: () => ctx.move() },
    ...laterItems(ctx.later),
  ];
  const collaborators = ctx.collaborators;
  if (ctx.playlist.owned && collaborators) {
    items.push({ id: "collaborators", label: "Collaborators…", icon: "users", run: () => collaborators() });
  }
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
