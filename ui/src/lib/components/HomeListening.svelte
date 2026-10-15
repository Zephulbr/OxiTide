<script lang="ts">
  import { api, type Stats, type StatsTopTrack } from "../api";
  import { errorMessage } from "../errors";
  import { formatCount, formatListenTime } from "../format";
  import { barPercents, dayTick } from "../historyChart";
  import { lastDays } from "../homeData";
  import type { Nav } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import type { Toasts } from "../toasts.svelte";

  /// "Your listening": a compact read of the local play history — this week's
  /// listening time with a 7-day bar strip, plus the top artists and tracks.
  let { stats, nav, toasts }: { stats: Stats; nav: Nav; toasts: Toasts } = $props();

  const days = $derived(lastDays(stats, 7));
  const heights = $derived(barPercents(days.map((day) => day.plays)));
  const artists = $derived(stats.top_artists.slice(0, 3));
  const tracks = $derived(stats.top_tracks.slice(0, 3));

  function play(track: StatsTopTrack) {
    playSource.set(null);
    void api
      .playPageItems([{ id: track.track_id, title: track.title, artists: [track.artist] }], 0)
      .catch((e) => toasts.show(errorMessage(e), { severity: "error" }));
  }
</script>

<section class="listening lit-tint pane" aria-label="Your listening">
  <div class="week">
    <span class="label muted">Last 7 days</span>
    <strong class="tnum">{formatListenTime(stats.listened_ms_7d)}</strong>
    <span class="muted tnum small">
      {formatCount(stats.plays_7d)} {stats.plays_7d === 1 ? "play" : "plays"}
    </span>
    <div class="bars" role="img" aria-label="Plays per day over the last 7 days">
      {#each days as day, i (day.date)}
        <span
          class="bar"
          title={`${dayTick(day.date)}: ${day.plays} ${day.plays === 1 ? "play" : "plays"}`}
        ><i class:empty={day.plays === 0} style:height={`${heights[i]}%`}></i></span>
      {/each}
    </div>
  </div>

  <div class="list">
    <h3>Top artists</h3>
    <ol>
      {#each artists as artist, i (artist.name)}
        <li>
          <span class="rank tnum">{i + 1}</span>
          <span class="name">{artist.name}</span>
          <span class="muted tnum small">{artist.plays}</span>
        </li>
      {/each}
    </ol>
  </div>

  <div class="list">
    <h3>Top tracks</h3>
    <ol>
      {#each tracks as track, i (track.track_id)}
        <li>
          <button type="button" class="row" onclick={() => play(track)} aria-label={`Play ${track.title}`}>
            <span class="rank tnum">{i + 1}</span>
            <span class="name">
              <span class="primary">{track.title}</span>
              <span class="muted by">{track.artist}</span>
            </span>
            <span class="muted tnum small">{track.plays}</span>
          </button>
        </li>
      {/each}
    </ol>
  </div>

  <button
    type="button"
    class="all"
    onclick={() => nav.goto({ name: "history", tab: "stats" })}
  >All stats</button>
</section>

<style>
  .listening {
    position: relative;
    display: grid;
    grid-template-columns: minmax(200px, 1fr) minmax(0, 1fr) minmax(0, 1.3fr);
    gap: 28px;
    padding: 20px 22px;
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-panel);
  }
  .week {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .label {
    font-size: 12.5px;
    font-weight: 600;
  }
  .week strong {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 640;
    letter-spacing: -0.01em;
  }
  .small {
    font-size: 12.5px;
  }
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    height: 44px;
    margin-top: 12px;
  }
  .bar {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .bar i {
    width: 100%;
    min-height: 3px;
    border-radius: 3px 3px 1px 1px;
    background: linear-gradient(180deg, var(--accent), color-mix(in srgb, var(--accent) 55%, transparent));
  }
  .bar i.empty {
    background: var(--line);
  }
  .list h3 {
    margin: 0 0 8px;
    font-size: 15px;
    font-weight: 600;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li,
  .row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    width: 100%;
    padding: 6px 8px;
    border-radius: var(--radius-control);
    text-align: left;
    color: inherit;
  }
  li:has(.row) {
    display: block;
    padding: 0;
  }
  .row:hover {
    background: var(--surface-2);
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
    font-weight: 500;
  }
  .primary,
  .by,
  li > .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .by {
    flex: 0 10 auto;
    font-size: 12.5px;
    font-weight: 400;
  }
  .all {
    position: absolute;
    top: 14px;
    right: 18px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .all:hover {
    color: var(--text);
  }
  @media (max-width: 900px) {
    .listening {
      grid-template-columns: 1fr;
      gap: 20px;
    }
  }
</style>
