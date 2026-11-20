<script lang="ts">
  import { untrack } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import { useRemote } from "../context";
  import type { RowTrack } from "../convert";
  import { FILTER_MIN, filterRows, matchesThenRest } from "../filter";
  import type { TrackPaging } from "../paged.svelte";
  import FilterBox from "./FilterBox.svelte";
  import Sentinel from "./Sentinel.svelte";
  import TrackRow from "./TrackRow.svelte";
  import { t } from "../i18n.svelte";

  /// Play / Shuffle / Add-all header plus rows; `numbered` shows album track
  /// numbers. Paged lists (`paging`) load the next page as the end scrolls
  /// into view, and the header actions fetch the rest first. `filterable`
  /// adds a filter box (title or artist) once the list has `FILTER_MIN`
  /// tracks; a filter loads the whole list so it covers every track, and the
  /// header actions then work on the matches.
  let {
    tracks,
    numbered = false,
    paging = null,
    filterable = false,
  }: {
    tracks: RowTrack[];
    numbered?: boolean;
    paging?: TrackPaging | null;
    filterable?: boolean;
  } = $props();

  const { actions, store } = useRemote();
  const playingId = $derived(store.state?.current?.trackId ?? null);
  /// The PC plays at most this many tracks from one call.
  const PLAY_CAP = 5000;

  let loadingMore = $state(false);
  let preparing = $state(false);
  let filling = $state(false);
  let failed = $state(false);
  let query = $state("");

  const hasMore = $derived(!!paging && !paging.done);
  const filtered = $derived(query.trim() !== "");
  const shown = $derived(filterRows(tracks, query));
  /// Each row's place in the whole list: a stable key while the filter
  /// changes, so kept rows are not rebuilt (and do not replay their rise-in).
  const place = $derived(new Map(tracks.map((row, i) => [row, i])));
  const showFilter = $derived(filterable && (filtered || tracks.length + (paging?.left ?? 0) >= FILTER_MIN));

  /// One page load at a time: the scroll sentinel, the header actions and the
  /// filter all page through `paging.more`, which reads from the pager's next
  /// offset, so two overlapping calls would read the same page twice.
  let inflight: Promise<void> | null = null;
  function step(): Promise<void> {
    inflight ??= paging!.more().finally(() => (inflight = null));
    return inflight;
  }

  /// Load pages until `cap` tracks are in or the list is read to its end.
  /// Every page moves the pager on (a page of unavailable tracks brings no
  /// rows but is not the end), so this always finishes.
  async function fill(cap: number) {
    while (paging && !paging.done && tracks.length < cap) await step();
  }

  async function loadMore() {
    if (loadingMore || !hasMore) return;
    loadingMore = true;
    failed = false;
    try {
      await step();
    } catch (e) {
      failed = true;
      actions.fail(e);
    } finally {
      loadingMore = false;
    }
  }

  /// A filter covers the whole list, not just the pages scrolled so far.
  async function fillForFilter() {
    if (filling || !hasMore) return;
    filling = true;
    failed = false;
    try {
      await fill(Infinity);
    } catch (e) {
      failed = true;
      actions.fail(e);
    } finally {
      filling = false;
    }
  }

  $effect(() => {
    if (filtered) untrack(() => void fillForFilter());
  });

  /// Load the remaining pages (bounded by the play cap), then play. Filtered:
  /// Play queues the matches then the rest; Shuffle shuffles the matches.
  async function playAll(shuffle: boolean) {
    if (preparing) return;
    preparing = true;
    try {
      await fill(PLAY_CAP);
      const rows = !filtered ? tracks : shuffle ? shown : matchesThenRest(shown, tracks);
      await actions.play(rows, 0, shuffle);
    } catch (e) {
      actions.fail(e);
    } finally {
      preparing = false;
    }
  }

  /// A tapped row plays with the rest of the list after it, not only the
  /// pages scrolled so far: with shuffle on, those would shuffle just the
  /// newest favorites (issue #7). Loads the rest first, as Play does.
  async function playRow(i: number) {
    if (preparing) return;
    const row = shown[i];
    preparing = true;
    try {
      await fill(PLAY_CAP);
      // The fill can add matches above the tapped row, so find it again.
      const rows = filtered ? matchesThenRest(shown, tracks) : tracks;
      await actions.play(rows, Math.max(0, rows.indexOf(row)));
    } catch (e) {
      actions.fail(e);
    } finally {
      preparing = false;
    }
  }

  /// "Add all" means the whole list (or every match), not the pages scrolled
  /// so far: load the rest first, as Play does.
  async function addAll(add: (rows: RowTrack[]) => Promise<void>) {
    if (preparing) return;
    preparing = true;
    try {
      await fill(PLAY_CAP);
      await add(shown);
    } catch (e) {
      actions.fail(e);
    } finally {
      preparing = false;
    }
  }
</script>

{#if showFilter}<FilterBox bind:value={query} label={t("filter.tracks")} />{/if}

{#if shown.length > 0}
  <div class="actions">
    <button class="pill primary" disabled={preparing} onclick={() => playAll(false)}>
      <Icon name="play" size={18} /> {t("np.play")}
    </button>
    <button class="pill" disabled={preparing} onclick={() => playAll(true)}>
      <Icon name="shuffle" size={18} /> {t("np.shuffle")}
    </button>
    <button class="icon-btn" aria-label={t("list.queueAll")} disabled={preparing} onclick={() => addAll((rows) => actions.enqueue(rows))}>
      <Icon name="list-plus" size={20} />
    </button>
    <button class="icon-btn" aria-label={t("list.playlistAll")} disabled={preparing} onclick={() => addAll((rows) => actions.pickPlaylist(rows))}>
      <Icon name="plus" size={20} />
    </button>
    {#if preparing}<span class="muted note" role="status">{t("list.loadingAll")}</span>{/if}
  </div>
{/if}

{#each shown as row, i (`${row.id}-${place.get(row)}`)}
  <TrackRow
    track={row}
    index={i}
    number={numbered ? (place.get(row) ?? i) + 1 : null}
    active={row.id === playingId}
    onplay={() => playRow(i)}
    onmenu={() => actions.openTrack(row)}
  />
{/each}

{#if filtered}
  {#if filling}
    <div class="tail">
      <span class="spinner" aria-hidden="true"></span>
      <span class="muted" role="status">{t("filter.loadingAll")}</span>
    </div>
  {:else if failed && hasMore}
    <div class="tail"><button class="pill" onclick={fillForFilter}>{t("list.retry")}</button></div>
  {:else if shown.length === 0}
    <p class="muted">{t("filter.noMatch", { filter: query.trim() })}</p>
  {/if}
{:else if hasMore}
  <!-- Armed on `left`, which moves every page: a page of unavailable tracks adds no rows. -->
  {#if !failed}<Sentinel onvisible={loadMore} armed={paging!.left} />{/if}
  <div class="tail">
    {#if failed}
      <button class="pill" onclick={loadMore}>{t("list.retry")}</button>
    {:else}
      <span class="spinner" aria-hidden="true"></span>
      <span class="muted" role="status">{t("list.loadingMore", { left: paging!.left })}</span>
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
