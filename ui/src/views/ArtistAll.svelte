<script lang="ts">
  import { untrack } from "svelte";
  import {
    api,
    trackToQueueItem,
    type Album,
    type Artist,
    type PageItem,
    type Track,
  } from "../lib/api";
  import { albumCard, playlistsFromSections } from "../lib/artistPage";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { routeForPageItem, type ArtistTab, type Nav } from "../lib/nav.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { playSource, type PlayFrom } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { playArtistRadio } from "../lib/radio";
  import { createTrackView, visibleIndexOf } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    id,
    tab,
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    id: number;
    tab: ArtistTab;
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();

  const TABS: { id: ArtistTab; label: string }[] = [
    { id: "tracks", label: "Tracks" },
    { id: "albums", label: "Albums" },
    { id: "eps", label: "Singles & EPs" },
    { id: "appears", label: "Appears on" },
    { id: "playlists", label: "Playlists" },
    { id: "mixes", label: "Mixes" },
  ];

  type AlbumTab = "albums" | "eps" | "appears";
  const FILTERS: Record<AlbumTab, "albums" | "eps_and_singles" | "compilations"> = {
    albums: "albums",
    eps: "eps_and_singles",
    appears: "compilations",
  };

  let artist = $state<Artist | null>(null);
  let artistError = $state("");
  /// The player bar's "From artist" line for plays started here.
  const origin = $derived<PlayFrom | null>(
    artist ? { kind: "artist", name: artist.name, route: { name: "artist", id } } : null,
  );
  let albumLists = $state<Record<AlbumTab, Album[] | null>>({
    albums: null,
    eps: null,
    appears: null,
  });
  let albumErrors = $state<Record<AlbumTab, string>>({ albums: "", eps: "", appears: "" });
  let playlists = $state<PageItem[] | null>(null);
  let playlistsError = $state("");

  // `{#key id}` in App resets this store when the artist changes, so the
  // initial id is captured once on purpose.
  const pager = untrack(() =>
    createTrackPager(`artist:${id}:tracks`, (offset, limit) =>
      api.getArtistTopTracksPage(id, offset, limit),
    ),
  );
  const view = untrack(() =>
    createTrackView(
      () => pager.items,
      async () => {
        await pager.ensureAll();
        if (pager.error) throw pager.error;
      },
      { memoryKey: `artist-all:${id}` },
    ),
  );

  const activeAlbumKind = $derived<AlbumTab | null>(
    tab === "albums" || tab === "eps" || tab === "appears" ? tab : null,
  );

  let tabsEl = $state<HTMLElement | null>(null);
  let indicator = $state({ left: 0, width: 0 });

  // Slide the underline to the active tab; observing the buttons keeps the
  // line aligned through resize and font swaps.
  $effect(() => {
    const row = tabsEl;
    const active = row?.querySelector<HTMLButtonElement>(`button[data-tab="${tab}"]`);
    if (!row || !active) return;
    const update = () => {
      indicator = { left: active.offsetLeft, width: active.offsetWidth };
    };
    update();
    const observer = new ResizeObserver(update);
    for (const button of row.querySelectorAll("button")) observer.observe(button);
    return () => observer.disconnect();
  });

  async function loadArtist() {
    artistError = "";
    try {
      artist = await api.getArtist(id);
    } catch (e) {
      artistError = String(e);
    }
  }

  async function loadAlbums(kind: AlbumTab) {
    if (albumLists[kind] !== null || albumErrors[kind] !== "") return;
    try {
      const list = await api.getArtistAlbumsAll(id, FILTERS[kind]);
      albumLists = { ...albumLists, [kind]: list };
    } catch (e) {
      albumErrors = { ...albumErrors, [kind]: String(e) };
    }
  }

  async function retryAlbums(kind: AlbumTab) {
    albumErrors = { ...albumErrors, [kind]: "" };
    await loadAlbums(kind);
  }

  async function loadPlaylists() {
    if (playlists !== null || playlistsError !== "") return;
    try {
      playlists = playlistsFromSections(await api.getArtistPage(id));
    } catch (e) {
      playlistsError = String(e);
    }
  }

  async function retryPlaylists() {
    playlistsError = "";
    await loadPlaylists();
  }

  $effect(() => {
    void loadArtist();
  });

  $effect(() => {
    if (tab !== "tracks") return;
    untrack(() => pager.loadMore());
  });

  $effect(() => {
    if (activeAlbumKind === null) return;
    void loadAlbums(activeAlbumKind);
  });

  $effect(() => {
    if (tab !== "playlists") return;
    void loadPlaylists();
  });

  async function playFrom(index: number) {
    const clicked = view.items[index];
    await pager.ensureAll();
    if (pager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(origin, prepared.items.map((t) => t.id));
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll(shuffle = false) {
    await pager.ensureAll();
    if (pager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(origin, prepared.items.map((t) => t.id));
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, shuffle);
  }
</script>

<div class="view-fill">
  <div class="catalog-head">
    <span class="kicker">Artist</span>
    <h2>{artist?.name ?? "Artist"}</h2>
  </div>

  <nav class="tabs" aria-label="Artist sections" bind:this={tabsEl}>
    {#each TABS as entry (entry.id)}
      <button
        class:active={tab === entry.id}
        data-tab={entry.id}
        aria-current={tab === entry.id ? "page" : undefined}
        onclick={() => nav.replace({ name: "artist-all", id, tab: entry.id })}
      >{entry.label}</button>
    {/each}
    {#if indicator.width > 0}
      <span
        class="tab-underline"
        style:left="{indicator.left}px"
        style:width="{indicator.width}px"
      ></span>
    {/if}
  </nav>

{#if tab === "tracks"}
  <div class="list-head">
    <p class="muted tnum count">
      {#if pager.total > 0}{pager.total} tracks{:else}tracks{/if}
    </p>
    <button
      class="list-action"
      onclick={() => playAll(true)}
      disabled={view.items.length === 0}
      aria-label="Shuffle"
      title="Shuffle"
    ><Icon name="shuffle" size={16} /></button>
    <button
      class="list-action"
      onclick={() => playAll()}
      disabled={view.items.length === 0}
      aria-label="Play all"
      title="Play all"
    ><Icon name="play" size={16} /></button>
  </div>
  {#if pager.error}
    <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
  {:else if pager.items.length === 0 && pager.loading}
    <Loading />
  {:else if pager.items.length === 0 && pager.done}
    <EmptyState message="No tracks." />
  {:else}
    <TrackTable
      {view}
      kind="artist"
      {nav}
      {favorites}
      {toasts}
      fill
      onPlay={playFrom}
      onEndReached={() => void pager.loadMore()}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      onAddManyToPlaylist={(ts) => playlistActions.openPicker(ts)}
      playingTrackId={player.state.current?.track_id ?? null}
      playbackRunning={player.state.playing}
    />
  {/if}
{:else if activeAlbumKind}
  {@const kind = activeAlbumKind}
  {@const list = albumLists[kind]}
  <div class="tab-body">
    {#if albumErrors[kind]}
      <ErrorState message={albumErrors[kind]} onRetry={() => retryAlbums(kind)} />
    {:else if list === null}
      <Loading />
    {:else if list.length === 0}
      <EmptyState message="No releases." />
    {:else}
      <p class="muted tnum count">{list.length} releases</p>
      <div class="grid">
        {#each list as release (release.id)}
          <MediaCard
            variant="tile"
            item={albumCard(release)}
            onclick={() => nav.goto({ name: "album", id: release.id })}
          />
        {/each}
      </div>
    {/if}
  </div>
{:else if tab === "playlists"}
  <div class="tab-body">
    {#if playlistsError}
      <ErrorState message={playlistsError} onRetry={retryPlaylists} />
    {:else if playlists === null}
      <Loading />
    {:else if playlists.length === 0}
      <EmptyState message="No playlists with this artist." />
    {:else}
      <p class="muted tnum count">{playlists.length} playlists</p>
      <div class="grid">
        {#each playlists as entry (entry.id)}
          {@const route = routeForPageItem(entry)}
          <MediaCard
            variant="tile"
            item={entry}
            onclick={route ? () => nav.goto(route) : undefined}
          />
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <div class="tab-body">
    {#if artistError}
      <ErrorState message={artistError} onRetry={loadArtist} />
    {:else if artist === null}
      <Loading />
    {:else}
      <div class="grid">
        {#if artist.mixes?.ARTIST_MIX}
          {@const mixId = artist.mixes.ARTIST_MIX}
          <MediaCard
            variant="tile"
            item={{
              id: mixId,
              title: `${artist.name} Mix`,
              subtitle: "Artist mix",
              item_type: "MIX",
              artwork: artist.picture ?? artist.cover_fallback,
            }}
            onclick={() => nav.goto({ name: "mix", id: mixId })}
          />
        {/if}
        <button class="radio-tile" onclick={() => void playArtistRadio(id, toasts, artist?.name)}>
          <span class="radio-icon" aria-hidden="true"><Icon name="radio" size={30} /></span>
          <span class="radio-title">Artist Radio</span>
          <span class="radio-sub muted">Endless station seeded on {artist.name}</span>
        </button>
      </div>
    {/if}
  </div>
{/if}
</div>

<style>
  .catalog-head {
    margin-bottom: 12px;
  }
  .kicker {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  /* Not a .page-head, so it takes the page-title size itself (app.css). */
  .catalog-head h2 {
    margin: 0;
    font-size: var(--page-title-size, 32px);
    line-height: 1.1;
  }
  .tabs {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 26px;
    margin-bottom: 16px;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    position: relative;
    height: 38px;
    padding: 0 2px;
    color: var(--text-muted);
    font-size: 14px;
    font-weight: 600;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.active {
    color: var(--text);
  }
  .tab-underline {
    position: absolute;
    bottom: -1px;
    left: 0;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
    transition:
      left var(--dur-state) var(--ease-in-out),
      width var(--dur-state) var(--ease-in-out);
  }
  .tab-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 24px;
  }
  .list-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
  }
  .list-head .count {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .list-action {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .list-action:hover:not(:disabled) {
    color: var(--text);
  }
  .list-action:disabled {
    opacity: 0.5;
  }
  .count {
    margin: 0 0 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(192px, 1fr));
    gap: 16px 20px;
  }
  .radio-tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
    text-align: left;
  }
  .radio-tile:hover {
    background: var(--surface-2);
  }
  .radio-icon {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 1;
    border-radius: var(--radius-card);
    background: linear-gradient(135deg, var(--surface-2), var(--surface-3));
    color: var(--text-muted);
  }
  .radio-title {
    font-weight: 500;
    font-size: 15px;
  }
  .radio-sub {
    font-size: 13px;
  }
</style>
