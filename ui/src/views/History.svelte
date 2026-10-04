<script lang="ts">
  import { api, historyToQueueItem, type HistoryEntry, type Stats } from "../lib/api";
  import { groupByDay, mergeHistoryEntries, relativeTime } from "../lib/historyTime";
  import type { HistoryTab, Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";
  import { playSource } from "../lib/playSource.svelte";
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

  function playEntry(entry: HistoryEntry) {
    const index = entries.indexOf(entry);
    if (index < 0) return;
    playSource.set(null);
    void api.playTracks(entries.map(historyToQueueItem), index).catch((e) => {
      toasts.show(String(e), { severity: "error" });
    });
  }

  function selectTab(next: HistoryTab) {
    nav.replace({ name: "history", tab: next });
  }
</script>

<div class="view-fill">
  <h2>History</h2>
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
          <section class="day">
            <h3>{group.label}</h3>
            {#each group.entries as entry, i (entry.started_at + "-" + entry.track_id + "-" + i)}
              <button
                class="row"
                class:playing={player.state.current?.track_id === entry.track_id && !entry.completed}
                onclick={() => playEntry(entry)}
                title={`Play ${entry.title}`}
              >
                <span class="thumb"><Cover uuid={entry.cover} size={160} radius={4} alt="" /></span>
                <span class="meta">
                  <strong>{entry.title}</strong>
                  <span class="muted">{entry.artist}{entry.album ? ` — ${entry.album}` : ""}</span>
                </span>
                <span class="muted tnum when">{relativeTime(entry.started_at, Date.now())}</span>
              </button>
            {/each}
          </section>
        {/each}
        {#if entries.length < total}
          <p class="more">
            <button onclick={() => void loadRecent(false)} disabled={recentLoading}>Load more</button>
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
        <HistoryStats {stats} />
      {/if}
    </div>
  {/if}
</div>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--line);
    margin: 8px 0 16px;
  }
  .tab {
    margin-bottom: -1px;
    padding: 8px 12px;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
  }
  .tab:hover,
  .tab.active {
    color: var(--text);
  }
  .tab.active {
    border-bottom-color: var(--accent);
  }
  .tab-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 24px;
  }
  .day h3 {
    margin: 14px 0 6px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 4px;
    text-align: left;
    border-radius: var(--radius-control);
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row.playing strong {
    color: var(--accent);
  }
  .thumb {
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .meta strong,
  .meta .muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta .muted {
    font-size: 12px;
  }
  .when {
    font-size: 12px;
    white-space: nowrap;
  }
  .more {
    text-align: center;
    margin: 16px 0;
  }
</style>
