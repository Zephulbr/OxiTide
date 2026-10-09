<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { useRemote } from "../context";
  import type { RowTrack } from "../convert";
  import TrackRow from "./TrackRow.svelte";

  /// Play / Shuffle / Add-all header plus rows; `numbered` shows album track
  /// numbers. `more` loads the next page when the list is paged.
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
  let loadingMore = $state(false);

  async function loadMore() {
    if (!more || loadingMore) return;
    loadingMore = true;
    try {
      await more();
    } catch (e) {
      actions.fail(e);
    } finally {
      loadingMore = false;
    }
  }
</script>

{#if tracks.length > 0}
  <div class="actions">
    <button class="pill primary" onclick={() => actions.play(tracks, 0)}>
      <Icon name="play" size={18} /> Play
    </button>
    <button class="pill" onclick={() => actions.play(tracks, 0, true)}>
      <Icon name="shuffle" size={18} /> Shuffle
    </button>
    <button class="icon-btn" aria-label="Add all to queue" onclick={() => actions.enqueue(tracks)}>
      <Icon name="list-plus" size={20} />
    </button>
    <button class="icon-btn" aria-label="Add all to a playlist" onclick={() => actions.pickPlaylist(tracks)}>
      <Icon name="plus" size={20} />
    </button>
  </div>
  {#if total !== null && total > tracks.length}
    <p class="muted note">Play and Shuffle use the {tracks.length} tracks loaded so far.</p>
  {/if}
{/if}

{#each tracks as row, i (`${row.id}-${i}`)}
  <TrackRow
    track={row}
    number={numbered ? i + 1 : null}
    active={row.id === playingId}
    onplay={() => actions.play(tracks, i)}
    onmenu={() => actions.openTrack(row)}
  />
{/each}

{#if more && total !== null && total > tracks.length}
  <button class="pill more" disabled={loadingMore} onclick={loadMore}>
    {loadingMore ? "Loading…" : `Load more (${total - tracks.length} left)`}
  </button>
{/if}

<style>
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
  }
  .note {
    font-size: 12px;
    margin: 0 0 8px;
  }
  .more {
    width: 100%;
    margin-top: 12px;
  }
</style>
