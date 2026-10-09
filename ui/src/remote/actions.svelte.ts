import { SvelteSet } from "svelte/reactivity";
import type { Playlist } from "../lib/api";
import type { RemoteApi } from "./api";
import { fromTrack, toPageInput, toQueueItem, type RowTrack } from "./convert";
import { hrefFor } from "./routes";

export interface SheetItem {
  label: string;
  icon: "list-plus" | "plus" | "heart" | "disc-3" | "music" | "radio" | "x";
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
      this.notify(favorite ? "Added to your collection" : "Removed from your collection");
    } catch (e) {
      if (favorite) set.delete(id);
      else set.add(id);
      this.fail(e);
    }
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
      this.notify(rows.length === 1 ? "Added to queue" : `Added ${rows.length} tracks to queue`);
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

  /// The per-row menu (spec §6.5): queue, playlist, favorite, go to.
  openTrack(row: RowTrack) {
    void this.ensureFavorites();
    const items: SheetItem[] = [
      { label: "Add to queue", icon: "list-plus", run: () => this.enqueue([row]) },
      { label: "Add to playlist…", icon: "plus", run: () => this.pickPlaylist([row]) },
      {
        label: this.favTracks.has(row.id) ? "Remove from favorites" : "Add to favorites",
        icon: "heart",
        run: () => this.toggleTrack(row.id),
      },
      { label: "Track radio", icon: "radio", run: () => this.radio(row) },
    ];
    if (row.albumId !== null) {
      const albumId = row.albumId;
      items.push({ label: "Go to album", icon: "disc-3", run: () => go(hrefFor({ name: "album", id: albumId })) });
    }
    if (row.artistId !== null) {
      const artistId = row.artistId;
      items.push({ label: "Go to artist", icon: "music", run: () => go(hrefFor({ name: "artist", id: artistId })) });
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
      this.notify("You have no playlists of your own yet.");
      return;
    }
    this.sheet = {
      title: "Add to playlist",
      subtitle: rows.length === 1 ? rows[0].title : `${rows.length} tracks`,
      items: this.playlists.map((playlist) => ({
        label: playlist.title,
        icon: "plus",
        run: async () => {
          try {
            await this.api.addPlaylistTracks(playlist.uuid, rows.map((r) => r.id));
            this.notify(`Added to ${playlist.title}`);
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
