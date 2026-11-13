<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import type { SearchResults } from "../../lib/api";
  import Shelf from "../components/Shelf.svelte";
  import TrackRow from "../components/TrackRow.svelte";
  import { useRemote } from "../context";
  import { albumTile, artistTile, fromTrack, playlistTile } from "../convert";
  import { t } from "../i18n.svelte";

  const { api, actions, store } = useRemote();

  // Kept across visits so Back returns to the same results.
  let query = $state(sessionStorage.getItem("oxi.search") ?? "");
  let results = $state<SearchResults | null>(null);
  let error = $state("");
  let busy = $state(false);
  let input = $state<HTMLInputElement | null>(null);
  let token = 0;

  $effect(() => {
    const q = query.trim();
    sessionStorage.setItem("oxi.search", query);
    if (!q) {
      results = null;
      error = "";
      return;
    }
    const mine = ++token;
    const timer = setTimeout(async () => {
      busy = true;
      try {
        const found = await api.search(q);
        if (mine === token) {
          results = found;
          error = "";
        }
      } catch (e) {
        if (mine === token) error = e instanceof Error ? e.message : String(e);
      } finally {
        if (mine === token) busy = false;
      }
    }, 350);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    if (!query) input?.focus();
  });

  const tracks = $derived(results?.tracks.items.map(fromTrack) ?? []);
  const playingId = $derived(store.state?.current?.trackId ?? null);
</script>

<div class="page">
  <form class="box" role="search" onsubmit={(e) => e.preventDefault()}>
    <Icon name="search" size={18} />
    <input
      bind:this={input}
      bind:value={query}
      type="search"
      placeholder={t("search.placeholder")}
      aria-label={t("search.label")}
      enterkeyhint="search"
      autocomplete="off"
    />
    {#if query}
      <button type="button" class="icon-btn" aria-label={t("search.clear")} onclick={() => (query = "")}>
        <Icon name="x" size={18} />
      </button>
    {/if}
  </form>

  {#if error}
    <p class="muted">{error}</p>
  {:else if busy && !results}
    <p class="muted">{t("search.searching")}</p>
  {:else if results}
    {#if tracks.length > 0}
      <h2 class="section-title">{t("library.tracks")}</h2>
      {#each tracks as row, i (row.id)}
        <TrackRow
          track={row}
          index={i}
          active={row.id === playingId}
          onplay={() => actions.play(tracks, i)}
          onmenu={() => actions.openTrack(row)}
        />
      {/each}
    {/if}
    {#if results.artists.items.length > 0}
      <Shelf title={t("library.artists")} tiles={results.artists.items.map(artistTile)} />
    {/if}
    {#if results.albums.items.length > 0}
      <Shelf title={t("library.albums")} tiles={results.albums.items.map(albumTile)} />
    {/if}
    {#if results.playlists.items.length > 0}
      <Shelf title={t("library.playlists")} tiles={results.playlists.items.map(playlistTile)} />
    {/if}
    {#if tracks.length + results.artists.items.length + results.albums.items.length + results.playlists.items.length === 0}
      <p class="muted">{t("search.noResults", { query: query.trim() })}</p>
    {/if}
  {/if}
</div>

<style>
  .box {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px 0 14px;
    margin-bottom: 8px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-float);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    color: var(--text-muted);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 46px;
    border: none;
    background: none;
    color: var(--text);
    font-size: 16px;
    outline: none;
  }
  input::-webkit-search-cancel-button {
    display: none;
  }
</style>
