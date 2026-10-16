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
  import { errorMessage } from "../lib/errors";
  import ArtistHero from "../lib/components/ArtistHero.svelte";
  import Carousel from "../lib/components/Carousel.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
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

  type SectionKey = "tracks" | "albums" | "eps" | "comps" | "similar";
  let sectionErrors = $state<Partial<Record<SectionKey, string>>>({});

  function setSectionError(key: SectionKey, error: unknown) {
    sectionErrors = { ...sectionErrors, [key]: errorMessage(error) };
  }

  function clearSectionError(key: SectionKey) {
    if (sectionErrors[key] === undefined) return;
    const next = { ...sectionErrors };
    delete next[key];
    sectionErrors = next;
  }

  /// Sections reveal in page order: one shows once it and every section above
  /// it have settled (loaded, empty or failed). They load in parallel and
  /// answer in any order, so showing each on arrival inserted shelves above
  /// ones already on screen and shoved them down.
  const SECTION_ORDER: SectionKey[] = ["tracks", "albums", "eps", "comps", "similar"];
  const unsettled = () => ({ tracks: false, albums: false, eps: false, comps: false, similar: false });
  let settled = $state<Record<SectionKey, boolean>>(unsettled());
  const shown = $derived.by(() => {
    const out = unsettled();
    for (const key of SECTION_ORDER) {
      if (!settled[key]) break;
      out[key] = true;
    }
    return out;
  });

  function loadSection<T>(key: SectionKey, run: () => Promise<T>, assign: (value: T) => void) {
    const token = loadToken;
    run()
      .then((value) => {
        if (token !== loadToken) return;
        clearSectionError(key);
        assign(value);
      })
      .catch((error) => {
        if (token !== loadToken) return;
        setSectionError(key, error);
      })
      .finally(() => {
        if (token === loadToken) settled[key] = true;
      });
  }

  function retrySection(key: SectionKey) {
    switch (key) {
      case "tracks":
        loadSection("tracks", () => api.getArtistTopTracks(id, 20), (list) => (topTracks = list));
        break;
      case "albums":
        loadSection("albums", () => api.getArtistAlbums(id, "albums", 50, 0), (list) => (albums = list));
        break;
      case "eps":
        loadSection(
          "eps",
          () => api.getArtistAlbums(id, "eps_and_singles", 50, 0),
          (list) => (eps = list),
        );
        break;
      case "comps":
        loadSection(
          "comps",
          () => api.getArtistAlbums(id, "compilations", 50, 0),
          (list) => (comps = list),
        );
        break;
      case "similar":
        loadSection("similar", () => api.getSimilarArtists(id), (list) => (similar = list));
        break;
    }
  }

  const view = createTrackView(() => topTracks);

  async function load() {
    const token = ++loadToken;
    loading = true;
    error = "";
    sectionErrors = {};
    settled = unsettled();
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
    loadSection("tracks", () => api.getArtistTopTracks(id, 20), (list) => (topTracks = list));
    loadSection("albums", () => api.getArtistAlbums(id, "albums", 50, 0), (list) => (albums = list));
    loadSection("eps", () => api.getArtistAlbums(id, "eps_and_singles", 50, 0), (list) => (eps = list));
    loadSection("comps", () => api.getArtistAlbums(id, "compilations", 50, 0), (list) => (comps = list));
    loadSection("similar", () => api.getSimilarArtists(id), (list) => (similar = list));
  }
  async function queueTop(startIndex: number, shuffle = false) {
    if (topTracks.length === 0) return;
    const prepared = await trackFlags.filterPlayable(topTracks, startIndex);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start, shuffle);
  }

  function playFrom(index: number) {
    // `view.items` is a prefix of `topTracks` until a filter/sort engages;
    // sourceIndex maps the visible row back to the queue's source order.
    const source = view.sourceIndex(index);
    if (source < 0) return;
    void queueTop(source);
  }

  async function share() {
    const copied = await copyText(shareUrl("artist", id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
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
  {@const mixId = artist.mixes?.ARTIST_MIX ?? null}
  <ArtistHero
    {artist}
    {profile}
    disabled={topTracks.length === 0}
    favorite={{
      active: favorites.hasArtist(artistId),
      onToggle: () => favorites.toggleArtist(artistId),
    }}
    onPlay={() => queueTop(0)}
    onShuffle={() => queueTop(0, true)}
    onOpenLink={openLink}
    onOpenArtist={(artistId) => nav.goto({ name: "artist", id: artistId })}
  >
    {#snippet actions()}
      <DetailAction
        icon="radio"
        label="Radio"
        title="Start artist radio"
        onclick={() => void playArtistRadio(artistId, toasts)}
      />
      <DetailAction icon="share-2" label="Share" title="Copy artist link" onclick={() => void share()} />
      {#if mixId}
        <DetailAction
          icon="disc-3"
          label="Artist mix"
          title="Open artist mix"
          onclick={() => nav.goto({ name: "mix", id: mixId })}
        />
      {/if}
    {/snippet}
  </ArtistHero>

  {#if shown.tracks && topTracks.length > 0}
    <section class="shelf">
      <div class="shelf-head">
        <h3>Top tracks</h3>
        <button class="more" onclick={() => nav.goto({ name: "artist-all", id, tab: "tracks" })}>
          View all
        </button>
      </div>
      <TrackTable
        {view}
        {nav}
        {favorites}
        {toasts}
        onPlay={playFrom}
        playingTrackId={player.state.current?.track_id ?? null}
        playbackRunning={player.state.playing}
        onAddToPlaylist={(track) => playlistActions.openPicker([track])}
      />
    </section>
  {:else if shown.tracks && sectionErrors.tracks}
    <section class="shelf">
      <div class="shelf-head"><h3>Top tracks</h3></div>
      <ErrorState message={sectionErrors.tracks} onRetry={() => retrySection("tracks")} />
    </section>
  {/if}

  {#if shown.albums && albums.length > 0}
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
  {:else if shown.albums && sectionErrors.albums}
    <section class="shelf">
      <div class="shelf-head"><h3>Albums</h3></div>
      <ErrorState message={sectionErrors.albums} onRetry={() => retrySection("albums")} />
    </section>
  {/if}

  {#if shown.eps && eps.length > 0}
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
  {:else if shown.eps && sectionErrors.eps}
    <section class="shelf">
      <div class="shelf-head"><h3>Singles & EPs</h3></div>
      <ErrorState message={sectionErrors.eps} onRetry={() => retrySection("eps")} />
    </section>
  {/if}

  {#if shown.comps && comps.length > 0}
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
  {:else if shown.comps && sectionErrors.comps}
    <section class="shelf">
      <div class="shelf-head"><h3>Appears on</h3></div>
      <ErrorState message={sectionErrors.comps} onRetry={() => retrySection("comps")} />
    </section>
  {/if}

  {#if shown.similar && similar.length > 0}
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
  {:else if shown.similar && sectionErrors.similar}
    <section class="shelf">
      <div class="shelf-head"><h3>Similar artists</h3></div>
      <ErrorState message={sectionErrors.similar} onRetry={() => retrySection("similar")} />
    </section>
  {/if}
{:else}
  <EmptyState message="Artist not found." />
{/if}

<style>
  .shelf {
    margin-bottom: 28px;
    /* Rises in as it is revealed (see shown). */
    animation: shelf-in var(--dur-view) var(--ease-enter) backwards;
  }
  @keyframes shelf-in {
    from {
      opacity: 0;
      transform: translateY(var(--rise));
    }
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
