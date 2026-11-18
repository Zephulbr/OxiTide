import { SvelteSet } from "svelte/reactivity";
import type { FavoriteChange, Playlist } from "../lib/api";
import type { RemoteApi } from "./api";
import { fromTrack, toPageInput, toQueueItem, type RowTrack } from "./convert";
import { hrefFor } from "./routes";
import { t } from "./i18n.svelte";

export interface SheetItem {
  label: string;
  icon: "skip-forward" | "list-plus" | "plus" | "heart" | "disc-3" | "music" | "radio" | "x";
  run: () => void | Promise<void>;
}

export interface Sheet {
  title: string;
  subtitle?: string;
  items: SheetItem[];
}

const MAX_TRACKS = 5000;

/// Shared row actions for every page: play, queue, favorites, playlists,
/// the per-row sheet, and the toast line.
export class RemoteActions {
  toast = $state("");
  sheet = $state<Sheet | null>(null);
  readonly favTracks = new SvelteSet<number>();
  readonly favAlbums = new SvelteSet<number>();
  readonly favArtists = new SvelteSet<number>();
  private favoritesLoaded = false;
  private toastTimer: ReturnType<typeof setTimeout> | null = null;
  private playlists: Playlist[] | null = null;

  constructor(readonly api: RemoteApi) {}

  notify(text: string) {
    this.toast = text;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = ""), 3500);
  }

  fail(error: unknown) {
    this.notify(error instanceof Error ? error.message : String(error));
  }

  async ensureFavorites() {
    if (this.favoritesLoaded) return;
    this.favoritesLoaded = true;
    try {
      const ids = await this.api.getFavoriteIds();
      ids.tracks.forEach((id) => this.favTracks.add(id));
      ids.albums.forEach((id) => this.favAlbums.add(id));
      ids.artists.forEach((id) => this.favArtists.add(id));
    } catch {
      this.favoritesLoaded = false;
    }
  }

  private async toggle(set: SvelteSet<number>, id: number, write: (fav: boolean) => Promise<void>) {
    const favorite = !set.has(id);
    if (favorite) set.add(id);
    else set.delete(id);
    try {
      await write(favorite);
      this.notify(favorite ? t("toast.addedToCollection") : t("toast.removedFromCollection"));
    } catch (e) {
      if (favorite) set.delete(id);
      else set.add(id);
      this.fail(e);
    }
  }

  /// Apply a heart changed elsewhere (the PC, another phone, or our own echo).
  applyFavorite({ kind, id, favorite }: FavoriteChange) {
    const set = kind === "album" ? this.favAlbums : kind === "artist" ? this.favArtists : this.favTracks;
    if (favorite) set.add(id);
    else set.delete(id);
  }

  toggleTrack(id: number) {
    return this.toggle(this.favTracks, id, (f) => this.api.setFavoriteTrack(id, f));
  }

  toggleAlbum(id: number) {
    return this.toggle(this.favAlbums, id, (f) => this.api.setFavoriteAlbum(id, f));
  }

  toggleArtist(id: number) {
    return this.toggle(this.favArtists, id, (f) => this.api.setFavoriteArtist(id, f));
  }

  async play(rows: RowTrack[], start = 0, shuffle = false) {
    if (rows.length === 0) return;
    const window = rows.slice(0, MAX_TRACKS);
    try {
      await this.api.playPageItems(window.map(toPageInput), Math.min(start, window.length - 1), shuffle);
    } catch (e) {
      this.fail(e);
    }
  }

  async enqueue(rows: RowTrack[]) {
    try {
      await this.api.queueAdd(rows.slice(0, MAX_TRACKS).map(toQueueItem));
      this.notify(t("toast.queued", { count: rows.length }));
    } catch (e) {
      this.fail(e);
    }
  }

  /// Right after the current track, in order (`queue_play_next`).
  async playNext(rows: RowTrack[]) {
    try {
      await this.api.queuePlayNext(rows.slice(0, MAX_TRACKS).map(toQueueItem));
      this.notify(t("toast.playsNext", { count: rows.length }));
    } catch (e) {
      this.fail(e);
    }
  }

  async radio(row: RowTrack) {
    try {
      const tracks = await this.api.getTrackRadio(row.id);
      await this.play(tracks.map(fromTrack));
    } catch (e) {
      this.fail(e);
    }
  }

  /// The per-row menu (spec §6.5): play next, queue, playlist, favorite, go to.
  openTrack(row: RowTrack) {
    void this.ensureFavorites();
    const items: SheetItem[] = [
      { label: t("menu.playNext"), icon: "skip-forward", run: () => this.playNext([row]) },
      { label: t("menu.addToQueue"), icon: "list-plus", run: () => this.enqueue([row]) },
      { label: t("menu.addToPlaylist"), icon: "plus", run: () => this.pickPlaylist([row]) },
      {
        label: this.favTracks.has(row.id) ? t("np.unfavorite") : t("np.favorite"),
        icon: "heart",
        run: () => this.toggleTrack(row.id),
      },
      { label: t("menu.trackRadio"), icon: "radio", run: () => this.radio(row) },
    ];
    if (row.albumId !== null) {
      const albumId = row.albumId;
      items.push({ label: t("menu.goToAlbum"), icon: "disc-3", run: () => go(hrefFor({ name: "album", id: albumId })) });
    }
    if (row.artistId !== null) {
      const artistId = row.artistId;
      items.push({ label: t("menu.goToArtist"), icon: "music", run: () => go(hrefFor({ name: "artist", id: artistId })) });
    }
    this.sheet = { title: row.title, subtitle: row.artist, items };
  }

  /// Owned playlists, then "add" (duplicates skipped by the PC).
  async pickPlaylist(rows: RowTrack[]) {
    try {
      this.playlists ??= (await this.api.getUserPlaylists()).filter((p) => p.owned);
    } catch (e) {
      this.fail(e);
      return;
    }
    if (this.playlists.length === 0) {
      this.notify(t("toast.noPlaylists"));
      return;
    }
    this.sheet = {
      title: t("menu.addToPlaylistTitle"),
      subtitle: rows.length === 1 ? rows[0].title : t("common.trackCount", { count: rows.length }),
      items: this.playlists.map((playlist) => ({
        label: playlist.title,
        icon: "plus",
        run: async () => {
          try {
            await this.api.addPlaylistTracks(playlist.uuid, rows.map((r) => r.id));
            this.notify(t("toast.addedTo", { playlist: playlist.title }));
          } catch (e) {
            this.fail(e);
          }
        },
      })),
    };
  }

  async runSheet(item: SheetItem) {
    this.sheet = null;
    await item.run();
  }
}

function go(href: string) {
  location.hash = href.slice(1);
}
