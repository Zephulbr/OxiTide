import { getContext, setContext } from "svelte";
import { api, trackToQueueItem, type PageItem, type Track } from "./api";
import { copyText } from "./clipboard";
import { errorMessage } from "./errors";
import type { Favorites } from "./favorites.svelte";
import type { MenuItem } from "./menu.svelte";
import { collectionCardMenuItems } from "./menus";
import { routeForPageItem, type Nav } from "./nav.svelte";
import { playSource } from "./playSource.svelte";
import type { PlaylistActions } from "./playlists.svelte";
import { shareUrl } from "./share";
import type { Toasts } from "./toasts.svelte";
import { trackFlags } from "./trackFlags.svelte";

/// What a collection card (album, playlist, mix) opens and plays.
export type CardTarget =
  | { kind: "album"; id: number }
  | { kind: "playlist"; id: string }
  | { kind: "mix"; id: string };

/// The card's collection, from the same routing rule its click uses. Tracks,
/// artists and page links are not collections.
export function cardTarget(item: PageItem): CardTarget | null {
  const route = routeForPageItem(item);
  if (route?.name === "album") return { kind: "album", id: route.id };
  if (route?.name === "playlist") return { kind: "playlist", id: route.id };
  if (route?.name === "mix") return { kind: "mix", id: route.id };
  return null;
}

export interface CardActions {
  play(target: CardTarget, shuffle?: boolean): Promise<void>;
  openArtist(id: number): void;
  menu(item: PageItem, target: CardTarget): MenuItem[];
}

export interface CardDeps {
  nav: Nav;
  toasts: Toasts;
  favorites: Favorites;
  playlistActions: PlaylistActions;
}

async function loadTracks(target: CardTarget): Promise<Track[]> {
  if (target.kind === "album") return api.getAlbumTracks(target.id);
  if (target.kind === "playlist") return api.getPlaylistTracks(target.id);
  return (await api.getMix(target.id)).tracks;
}

/// Card play/queue/menu behaviour, shared by every page through context so
/// cards stay self-contained (no per-view wiring).
export function createCardActions({ nav, toasts, favorites, playlistActions }: CardDeps): CardActions {
  const HIDDEN = "Everything here is hidden by your content filters.";

  /// The collection's playable tracks, or null after telling the user why not.
  async function playable(target: CardTarget): Promise<Track[] | null> {
    try {
      const prepared = await trackFlags.filterPlayable(await loadTracks(target), 0);
      if (!prepared) {
        toasts.show(HIDDEN, { severity: "info" });
        return null;
      }
      return prepared.items;
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      return null;
    }
  }

  async function play(target: CardTarget, shuffle = false) {
    const tracks = await playable(target);
    if (!tracks) return;
    // Only playlists mark the sidebar source; every other origin clears it.
    playSource.set(target.kind === "playlist" ? target.id : null);
    try {
      await api.playTracks(tracks.map(trackToQueueItem), 0, shuffle);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function queue(target: CardTarget, title: string, next: boolean) {
    const tracks = await playable(target);
    if (!tracks) return;
    const items = tracks.map(trackToQueueItem);
    try {
      await (next ? api.queuePlayNext(items) : api.queueAdd(items));
      toasts.show(next ? `"${title}" plays next` : `Added "${title}" to the queue`);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function addToPlaylist(target: CardTarget) {
    try {
      const tracks = await loadTracks(target);
      if (tracks.length > 0) playlistActions.openPicker(tracks);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function copyLink(target: CardTarget) {
    const copied = await copyText(shareUrl(target.kind, target.id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
  }

  function openArtist(id: number) {
    nav.goto({ name: "artist", id });
  }

  function menu(item: PageItem, target: CardTarget): MenuItem[] {
    const album = target.kind === "album" ? target.id : null;
    return collectionCardMenuItems({
      play: () => void play(target),
      shuffle: () => void play(target, true),
      playNext: () => void queue(target, item.title, true),
      addToQueue: () => void queue(target, item.title, false),
      addToPlaylist: () => void addToPlaylist(target),
      favorite:
        album === null
          ? undefined
          : { active: favorites.hasAlbum(album), toggle: () => void favorites.toggleAlbum(album) },
      goToArtists: (item.artist_refs ?? []).map((artist) => ({
        name: artist.name,
        open: () => openArtist(artist.id),
      })),
      copyLink: () => void copyLink(target),
    });
  }

  return { play, openArtist, menu };
}

const KEY = Symbol("card-actions");

/// Provided once per routed page; cards outside a provider keep the plain
/// single-click behaviour.
export function setCardActions(actions: CardActions) {
  setContext(KEY, actions);
}

export function getCardActions(): CardActions | undefined {
  return getContext<CardActions | undefined>(KEY);
}
