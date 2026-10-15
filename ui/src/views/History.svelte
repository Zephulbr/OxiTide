<script lang="ts">
  import { api, historyToQueueItem, type HistoryEntry, type Stats } from "../lib/api";
  import { clockTime, groupByDay, mergeHistoryEntries, relativeTime } from "../lib/historyTime";
  import { formatDuration, formatListenTime } from "../lib/format";
  import type { HistoryTab, Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import HistoryStats from "../lib/components/HistoryStats.svelte";
  import Loading from "../lib/components/Loading.svelte";

  let {
    nav,
    player,
    toasts,
    tab,
  }: { nav: Nav; player: Player; toasts: Toasts; tab: HistoryTab } = $props();

  const TABS: { id: HistoryTab; label: string }[] = [
    { id: "recent", label: "Recently played" },
    { id: "stats", label: "Stats" },
  ];
  const PAGE = 100;

  let entries = $state<HistoryEntry[]>([]);
  let total = $state(0);
  let recentLoading = $state(false);
  let recentError = $state("");

  let stats = $state<Stats | null>(null);
  let statsError = $state("");

  const groups = $derived(groupByDay(entries, Date.now()));

  async function loadRecent(reset: boolean) {
    if (recentLoading) return;
    recentLoading = true;
    recentError = "";
    try {
      const page = await api.getHistoryPage(reset ? 0 : entries.length, PAGE);
      entries = reset ? page.items : mergeHistoryEntries(entries, page.items);
      total = page.total;
    } catch (e) {
      recentError = String(e);
    } finally {
      recentLoading = false;
    }
  }

  async function loadStats() {
    statsError = "";
    try {
      stats = await api.getStats(new Date().getTimezoneOffset());
    } catch (e) {
      statsError = String(e);
    }
  }

  // Fetch when a tab becomes active: the view is not remounted on tab
  // switches, so load-once would leave the list and stats stale.
  let lastTab: HistoryTab | null = null;
  $effect(() => {
    if (tab === lastTab) return;
    lastTab = tab;
    if (tab === "recent") {
      void loadRecent(true);
    } else {
      void loadStats();
    }
  });

  async function playEntry(entry: HistoryEntry) {
    const index = entries.indexOf(entry);
    if (index < 0) return;
    // History records carry no flags: explicit fails open, AI is enriched by id.
    const items = entries.map((e) => ({ entry: e, id: e.track_id, explicit: undefined }));
    const prepared = await trackFlags.filterPlayable(items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api
      .playTracks(prepared.items.map((item) => historyToQueueItem(item.entry)), prepared.start)
      .catch((e) => {
        toasts.show(String(e), { severity: "error" });
      });
  }

  function selectTab(next: HistoryTab) {
    nav.replace({ name: "history", tab: next });
  }
</script>

<div class="view-fill">
  <header class="page-head">
    <h2>History</h2>
    {#if tab === "recent" && total > 0}<span class="muted count tnum">{total} {total === 1 ? "play" : "plays"}</span>{/if}
  </header>
  <nav class="tabs" aria-label="History sections">
    {#each TABS as entry (entry.id)}
      <button
        class="tab"
        class:active={tab === entry.id}
        aria-current={tab === entry.id ? "page" : undefined}
        onclick={() => selectTab(entry.id)}
      >{entry.label}</button>
    {/each}
  </nav>

  {#if tab === "recent"}
    <div class="tab-body">
      {#if recentError}
        <ErrorState message={recentError} onRetry={() => void loadRecent(true)} />
      {:else if recentLoading && entries.length === 0}
        <Loading />
      {:else if entries.length === 0}
        <EmptyState message="Nothing played yet." />
      {:else}
        {#each groups as group (group.entries[0].started_at)}
          {@const listened = group.entries.reduce((sum, e) => sum + e.listened_ms, 0)}
          <section class="day">
            <header class="dayhead">
              <h3>{group.label}</h3>
              <span class="muted tnum">
                {group.entries.length} {group.entries.length === 1 ? "play" : "plays"} · {formatListenTime(listened)}
              </span>
            </header>
            <div class="rows pane">
              {#each group.entries as entry, i (entry.started_at + "-" + entry.track_id + "-" + i)}
                {@const playing = player.state.current?.track_id === entry.track_id && !entry.completed}
                <button
                  class="row"
                  class:playing
                  class:partial={!playing && !entry.completed && entry.listened_ms < entry.duration_secs * 500}
                  onclick={() => playEntry(entry)}
                  title={`Play ${entry.title} (${relativeTime(entry.started_at, Date.now())})`}
                >
                  <span class="muted tnum when">{clockTime(entry.started_at)}</span>
                  <span class="thumb"><Cover uuid={entry.cover} size={160} radius={4} alt="" /></span>
                  <span class="meta">
                    <strong>{entry.title}</strong>
                    <span class="muted">{entry.artist}</span>
                  </span>
                  <span class="muted album">{entry.album}</span>
                  <span class="muted tnum len">{formatDuration(entry.duration_secs)}</span>
                </button>
              {/each}
            </div>
          </section>
        {/each}
        {#if entries.length < total}
          <p class="more">
            <button class="tool-btn" onclick={() => void loadRecent(false)} disabled={recentLoading}>Load more</button>
          </p>
        {/if}
      {/if}
    </div>
  {:else}
    <div class="tab-body">
      {#if statsError}
        <ErrorState message={statsError} onRetry={() => void loadStats()} />
      {:else if !stats}
        <Loading />
      {:else}
        <HistoryStats {stats} {nav} {toasts} />
      {/if}
    </div>
  {/if}
</div>

<style>
  .page-head {
    margin-bottom: 14px;
  }
  /* Same tabs as the artist catalogue (ArtistAll). */
  .tabs {
    display: flex;
    gap: 26px;
    margin: 0 0 8px;
    border-bottom: 1px solid var(--line);
  }
  .tab {
    position: relative;
    height: 38px;
    padding: 0 2px;
    color: var(--text-muted);
    font-size: 14px;
    font-weight: 600;
  }
  .tab:hover,
  .tab.active {
    color: var(--text);
  }
  .tab.active::after {
    content: "";
    position: absolute;
    right: 0;
    bottom: -1px;
    left: 0;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  .tab-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    /* .main drops its bottom padding for .view-fill pages, so this scroller
       clears the floating player bar itself (as TrackTable's fill mode does). */
    padding: 8px 4px calc(var(--player-height) + 24px) 0;
  }
  .dayhead {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin: 22px 0 8px;
    font-size: 12.5px;
  }
  .day:first-child .dayhead {
    margin-top: 8px;
  }
  .dayhead h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .dayhead::after {
    content: "";
    flex: 1;
    align-self: center;
    height: 1px;
    background: var(--line);
  }
  /* A play log: clock time in the gutter, then the track, album and length. */
  .row {
    display: grid;
    grid-template-columns: 56px 40px minmax(0, 1.3fr) minmax(0, 1fr) 48px;
    gap: 14px;
    align-items: center;
    width: 100%;
    padding: 6px 12px;
    text-align: left;
    border-radius: var(--radius-control);
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row.playing {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .row.playing strong {
    color: var(--accent);
  }
  .row.partial :is(strong, .meta .muted, .album) {
    opacity: 0.6;
  }
  .when {
    font-size: 12.5px;
    white-space: nowrap;
    text-align: right;
  }
  .thumb {
    width: 40px;
    height: 40px;
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .meta strong {
    font-weight: 600;
  }
  .meta strong,
  .meta .muted,
  .album {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta .muted,
  .album,
  .len {
    font-size: 12.5px;
  }
  .len {
    text-align: right;
  }
  @media (max-width: 1000px) {
    .row {
      grid-template-columns: 56px 40px minmax(0, 1fr) 48px;
    }
    .album {
      display: none;
    }
  }
  .more {
    text-align: center;
    margin: 20px 0;
  }
</style>