<script lang="ts">
  import {
    api,
    trackToQueueItem,
    type Album as AlbumT,
    type Artist as ArtistT,
    type ArtistProfile,
    type Track,
  } from "../lib/api";
  import { albumCard, artistCard } from "../lib/artistPage";
  import { copyText } from "../lib/clipboard";
  import ArtistHero from "../lib/components/ArtistHero.svelte";
  import Carousel from "../lib/components/Carousel.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { menu } from "../lib/menu.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { playArtistRadio } from "../lib/radio";
  import { shareUrl } from "../lib/share";
  import { createTrackView } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    id,
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    id: number;
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();

  let artist = $state<ArtistT | null>(null);
  let profile = $state<ArtistProfile | null>(null);
  let topTracks = $state<Track[]>([]);
  let albums = $state<AlbumT[]>([]);
  let eps = $state<AlbumT[]>([]);
  let comps = $state<AlbumT[]>([]);
  let similar = $state<ArtistT[]>([]);
  let loading = $state(true);
  let error = $state("");
  let loadToken = 0;

  const view = createTrackView(() => topTracks);

  async function load() {
    const token = ++loadToken;
    loading = true;
    error = "";
    topTracks = [];
    albums = [];
    eps = [];
    comps = [];
    similar = [];
    try {
      const [loadedArtist, loadedProfile] = await Promise.all([
        api.getArtist(id),
        api.getArtistProfile(id).catch(() => null),
      ]);
      if (token !== loadToken) return;
      artist = loadedArtist;
      profile = loadedProfile;
    } catch (e) {
      if (token !== loadToken) return;
      error = String(e);
      loading = false;
      return;
    }
    loading = false;
    // Sections load independently: one slow or failed shelf never blocks
    // the hero or its neighbours, and stale responses are dropped.
    void api.getArtistTopTracks(id, 20)
      .then((tracks) => {
        if (token === loadToken) topTracks = tracks;
      })
      .catch(() => {});
    for (const [filter, target] of [
      ["albums", "albums"],
      ["eps_and_singles", "eps"],
      ["compilations", "comps"],
    ] as const) {
      void api.getArtistAlbums(id, filter, 50, 0)
        .then((list) => {
          if (token !== loadToken) return;
          if (target === "albums") albums = list;
          else if (target === "eps") eps = list;
          else comps = list;
        })
        .catch(() => {});
    }
    void api.getSimilarArtists(id)
      .then((list) => {
        if (token === loadToken) similar = list;
      })
      .catch(() => {});
  }

  function queueTop(startIndex: number, shuffle = false) {
    if (topTracks.length === 0) return;
    playSource.set(null);
    void api.playTracks(topTracks.map(trackToQueueItem), startIndex, shuffle);
  }

  function playFrom(index: number) {
    // `view.items` is a prefix of `topTracks` until a filter/sort engages;
    // sourceIndex maps the visible row back to the queue's source order.
    const source = view.sourceIndex(index);
    if (source < 0) return;
    queueTop(source);
  }

  async function share() {
    const copied = await copyText(shareUrl("artist", id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
  }

  function openMenu(event: MouseEvent) {
    menu.show(event.clientX, event.clientY, [
      { id: "share", label: "Share", run: () => void share() },
    ]);
  }

  async function openLink(url: string) {
    try {
      await api.openExternal(url);
    } catch (e) {
      toasts.show(String(e), { severity: "error" });
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
  {@const artistId = artist.id}
  <ArtistHero
    {artist}
    {profile}
    disabled={topTracks.length === 0}
    onPlay={() => queueTop(0)}
    onShuffle={() => queueTop(0, true)}
    onOpenLink={openLink}
  >
    {#snippet actions()}
      <DetailAction
        icon="radio"
        label="Radio"
        title="Start artist radio"
        onclick={() => void playArtistRadio(artistId, toasts)}
      />
      <DetailAction
        icon="heart"
        label={favorites.hasArtist(artistId) ? "Favorited" : "Favorite"}
        active={favorites.hasArtist(artistId)}
        title={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
        onclick={() => favorites.toggleArtist(artistId)}
      />
      <DetailAction icon="ellipsis" label="More" title="More options" onclick={openMenu} />
    {/snippet}
  </ArtistHero>

  {#if topTracks.length > 0}
    <section class="shelf">
      <div class="shelf-head">
        <h3>Top tracks</h3>
        <button class="more" onclick={() => nav.goto({ name: "artist-all", id, tab: "tracks" })}>
          View all
        </button>
      </div>
      <TrackTable
        {view}
        {favorites}
        {toasts}
        onPlay={playFrom}
        playingTrackId={player.state.current?.track_id ?? null}
        onAddToPlaylist={(track) => playlistActions.openPicker([track])}
      />
    </section>
  {/if}

  {#if albums.length > 0}
    <section class="shelf">
      <Carousel title="Albums">
        {#snippet controls()}
          <button class="more" onclick={() => nav.goto({ name: "artist-all", id, tab: "albums" })}>
            View all
          </button>
        {/snippet}
        {#each albums as album (album.id)}
          <MediaCard
            variant="carousel"
            item={albumCard(album)}
            onclick={() => nav.goto({ name: "album", id: album.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}

  {#if eps.length > 0}
    <section class="shelf">
      <Carousel title="Singles & EPs">
        {#snippet controls()}
          <button class="more" onclick={() => nav.goto({ name: "artist-all", id, tab: "eps" })}>
            View all
          </button>
        {/snippet}
        {#each eps as album (album.id)}
          <MediaCard
            variant="carousel"
            item={albumCard(album)}
            onclick={() => nav.goto({ name: "album", id: album.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}

  {#if comps.length > 0}
    <section class="shelf">
      <Carousel title="Appears on">
        {#snippet controls()}
          <button class="more" onclick={() => nav.goto({ name: "artist-all", id, tab: "appears" })}>
            View all
          </button>
        {/snippet}
        {#each comps as album (album.id)}
          <MediaCard
            variant="carousel"
            item={albumCard(album)}
            onclick={() => nav.goto({ name: "album", id: album.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}

  {#if similar.length > 0}
    <section class="shelf">
      <Carousel title="Similar artists">
        {#each similar as s (s.id)}
          <MediaCard
            variant="carousel"
            item={artistCard(s)}
            onclick={() => nav.goto({ name: "artist", id: s.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}
{:else}
  <EmptyState message="Artist not found." />
{/if}

<style>
  .shelf {
    margin-bottom: 28px;
  }
  .shelf-head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .shelf-head h3 {
    flex: 1;
    min-width: 0;
  }
  .more {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .more:hover {
    color: var(--text);
  }
</style>
