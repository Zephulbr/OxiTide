<script lang="ts">
  import { formatCount, formatListenTime } from "../format";
  import { barPercents, dayTick } from "../historyChart";
  import type { Stats } from "../api";

  let { stats }: { stats: Stats } = $props();

  const heights = $derived(barPercents(stats.daily.map((day) => day.plays)));
  const peak = $derived(Math.max(0, ...stats.daily.map((day) => day.plays)));
  const summary = $derived([
    { label: "All time", ms: stats.total_listened_ms, plays: stats.total_plays, days: 0 },
    { label: "Last 7 days", ms: stats.listened_ms_7d, plays: stats.plays_7d, days: 7 },
    { label: "Last 30 days", ms: stats.listened_ms_30d, plays: stats.plays_30d, days: 30 },
  ]);
  const tops = $derived([
    {
      title: "Top artists",
      rows: stats.top_artists.map((e) => ({ key: e.name, name: e.name, by: "", plays: e.plays })),
    },
    {
      title: "Top tracks",
      rows: stats.top_tracks.map((e) => ({ key: String(e.track_id), name: e.title, by: e.artist, plays: e.plays })),
    },
    {
      title: "Top albums",
      rows: stats.top_albums.map((e) => ({ key: e.name, name: e.name, by: "", plays: e.plays })),
    },
  ]);
</script>

{#if stats.total_plays === 0}
  <p class="muted">No plays yet.</p>
{:else}
  <div class="cards">
    {#each summary as card (card.label)}
      <div class="card lit pane">
        <span class="muted label">{card.label}</span>
        <strong class="tnum">{formatListenTime(card.ms)}</strong>
        <span class="muted tnum">
          {`${formatCount(card.plays)} ${card.plays === 1 ? "play" : "plays"}`}{card.days > 0
            ? ` · ${formatListenTime(card.ms / card.days)} a day`
            : ""}
        </span>
      </div>
    {/each}
  </div>

  <section class="panel lit pane">
    <header class="panel-head">
      <h3>Plays per day</h3>
      <span class="muted tnum">Last 30 days · peak {peak}</span>
    </header>
    <div class="chart" role="img" aria-label="Plays per day over the last 30 days">
      <div class="grid-lines" aria-hidden="true"><i></i><i></i><i></i></div>
      {#each stats.daily as day, i (day.date)}
        <div class="bar" title={`${dayTick(day.date)}: ${day.plays} ${day.plays === 1 ? "play" : "plays"}`}>
          <span class="fill" class:empty={day.plays === 0} style:height={`${heights[i]}%`}></span>
        </div>
      {/each}
    </div>
    {#if stats.daily.length > 0}
      <div class="axis muted tnum" aria-hidden="true">
        <span>{dayTick(stats.daily[0].date)}</span>
        <span>{dayTick(stats.daily[Math.floor(stats.daily.length / 2)].date)}</span>
        <span>Today</span>
      </div>
    {/if}
  </section>

  <div class="tops">
    {#each tops as list (list.title)}
      {@const max = Math.max(1, ...list.rows.map((row) => row.plays))}
      <section class="panel lit pane">
        <header class="panel-head"><h3>{list.title}</h3></header>
        <ol>
          {#each list.rows as row, i (row.key)}
            <li style:--share={row.plays / max}>
              <span class="rank tnum">{i + 1}</span>
              <span class="name">
                <span class="primary">{row.name}</span>
                {#if row.by}<span class="muted by">{row.by}</span>{/if}
              </span>
              <span class="muted tnum plays">{row.plays}</span>
            </li>
          {/each}
        </ol>
      </section>
    {/each}
  </div>
{/if}

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
    margin-bottom: 14px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px 18px;
    border-radius: var(--radius-card);
  }
  .label {
    font-size: 12.5px;
    font-weight: 600;
  }
  .card strong {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 640;
    letter-spacing: -0.01em;
  }
  .card .muted:last-child {
    font-size: 12.5px;
  }
  .panel {
    padding: 16px 18px 18px;
    border-radius: var(--radius-card);
  }
  .panel-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
    font-size: 12.5px;
  }
  .panel-head h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .chart {
    position: relative;
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 150px;
  }
  .grid-lines {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    pointer-events: none;
  }
  .grid-lines i {
    height: 1px;
    background: var(--line);
    opacity: 0.6;
  }
  .bar {
    position: relative;
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .fill {
    width: 100%;
    min-height: 3px;
    border-radius: 4px 4px 1px 1px;
    background: linear-gradient(180deg, var(--accent), color-mix(in srgb, var(--accent) 55%, transparent));
    transition: filter var(--dur-micro);
  }
  .bar:hover .fill:not(.empty) {
    filter: brightness(1.15);
  }
  .fill.empty {
    background: var(--line);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 11.5px;
  }
  .tops {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 14px;
    margin-top: 14px;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* Each row carries a faint bar for its share of the list's top count. */
  li {
    position: relative;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 6px 8px;
    border-radius: var(--radius-control);
    isolation: isolate;
  }
  li::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    width: calc(var(--share) * 100%);
    border-radius: inherit;
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }
  .rank {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    text-align: right;
  }
  li:first-child .rank {
    color: var(--accent);
  }
  .name {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .primary,
  .by {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .primary {
    flex: 0 1 auto;
    font-weight: 500;
  }
  .by {
    flex: 0 10 auto;
    font-size: 12.5px;
  }
  .plays {
    font-size: 12.5px;
  }
  @media (max-width: 760px) {
    .cards {
      grid-template-columns: 1fr;
    }
  }
</style>
