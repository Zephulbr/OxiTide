<script lang="ts">
  import { formatCount, formatTotalDuration } from "../format";
  import type { Stats } from "../api";

  let { stats }: { stats: Stats } = $props();
</script>

{#if stats.total_plays === 0}
  <p class="muted">No plays yet.</p>
{:else}
  <div class="cards">
    <div class="card">
      <span class="muted">All time</span>
      <strong class="tnum">{formatTotalDuration(stats.total_listened_ms / 1000)}</strong>
      <span class="muted">{formatCount(stats.total_plays)} plays</span>
    </div>
    <div class="card">
      <span class="muted">Last 7 days</span>
      <strong class="tnum">{formatTotalDuration(stats.listened_ms_7d / 1000)}</strong>
      <span class="muted">{formatCount(stats.plays_7d)} plays</span>
    </div>
    <div class="card">
      <span class="muted">Last 30 days</span>
      <strong class="tnum">{formatTotalDuration(stats.listened_ms_30d / 1000)}</strong>
      <span class="muted">{formatCount(stats.plays_30d)} plays</span>
    </div>
  </div>

  <div class="tops">
    <section>
      <h3>Top artists</h3>
      {#each stats.top_artists as entry (entry.name)}
        <p class="top">
          <span class="name">{entry.name}</span>
          <span class="muted tnum">{entry.plays}</span>
        </p>
      {/each}
    </section>
    <section>
      <h3>Top tracks</h3>
      {#each stats.top_tracks as entry (entry.track_id)}
        <p class="top">
          <span class="name">{entry.title} <span class="muted">{entry.artist}</span></span>
          <span class="muted tnum">{entry.plays}</span>
        </p>
      {/each}
    </section>
    <section>
      <h3>Top albums</h3>
      {#each stats.top_albums as entry (entry.name)}
        <p class="top">
          <span class="name">{entry.name}</span>
          <span class="muted tnum">{entry.plays}</span>
        </p>
      {/each}
    </section>
  </div>
{/if}

<style>
  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 150px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    background: var(--surface-1);
  }
  .card strong {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 640;
  }
  .tops {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 20px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    margin: 0;
    padding: 4px 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
