<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { useRemote } from "../context";
  import type { RowTrack } from "../convert";
  import Sentinel from "./Sentinel.svelte";
  import TrackRow from "./TrackRow.svelte";

  /// Play / Shuffle / Add-all header plus rows; `numbered` shows album track
  /// numbers. Paged lists (`more` + `total`) load the next page as the end
  /// scrolls into view, and Play/Shuffle fetch the rest first.
  let {
    tracks,
    numbered = false,
    more = null,
    total = null,
  }: {
    tracks: RowTrack[];
    numbered?: boolean;
    more?: (() => Promise<void>) | null;
    total?: number | null;
  } = $props();

  const { actions, store } = useRemote();
  const playingId = $derived(store.state?.current?.trackId ?? null);
  /// The PC plays at most this many tracks from one call.
  const PLAY_CAP = 5000;

  let loadingMore = $state(false);
  let preparing = $state(false);
  let failed = $state(false);

  const hasMore = $derived(!!more && total !== null && total > tracks.length);

  async function loadMore() {
    if (!more || loadingMore || !hasMore) return;
    loadingMore = true;
    failed = false;
    try {
      await more();
    } catch (e) {
      failed = true;
      actions.fail(e);
    } finally {
      loadingMore = false;
    }
  }

  /// Load the remaining pages (bounded by the play cap), then play.
  async function playAll(start: number, shuffle: boolean) {
    if (preparing) return;
    preparing = true;
    try {
      while (more && total !== null && tracks.length < Math.min(total, PLAY_CAP)) {
        const before = tracks.length;
        await more();
        if (tracks.length === before) break;
      }
      await actions.play(tracks, start, shuffle);
    } catch (e) {
      actions.fail(e);
    } finally {
      preparing = false;
    }
  }
</script>

{#if tracks.length > 0}
  <div class="actions">
    <button class="pill primary" disabled={preparing} onclick={() => playAll(0, false)}>
      <Icon name="play" size={18} /> Play
    </button>
    <button class="pill" disabled={preparing} onclick={() => playAll(0, true)}>
      <Icon name="shuffle" size={18} /> Shuffle
    </button>
    <button class="icon-btn" aria-label="Add all to queue" onclick={() => actions.enqueue(tracks)}>
      <Icon name="list-plus" size={20} />
    </button>
    <button class="icon-btn" aria-label="Add all to a playlist" onclick={() => actions.pickPlaylist(tracks)}>
      <Icon name="plus" size={20} />
    </button>
    {#if preparing}<span class="muted note" role="status">Loading the whole list…</span>{/if}
  </div>
{/if}

{#each tracks as row, i (`${row.id}-${i}`)}
  <TrackRow
    track={row}
    index={i}
    number={numbered ? i + 1 : null}
    active={row.id === playingId}
    onplay={() => actions.play(tracks, i)}
    onmenu={() => actions.openTrack(row)}
  />
{/each}

{#if hasMore}
  {#if !failed}<Sentinel onvisible={loadMore} armed={tracks.length} />{/if}
  <div class="tail">
    {#if failed}
      <button class="pill" onclick={loadMore}>Retry loading</button>
    {:else}
      <span class="spinner" aria-hidden="true"></span>
      <span class="muted" role="status">Loading more ({total! - tracks.length} left)…</span>
    {/if}
  </div>
{/if}

<style>
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
  }
  .note {
    font-size: 12px;
  }
  .tail {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 56px;
    font-size: 13px;
  }
  .spinner {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid var(--surface-3);
    border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
