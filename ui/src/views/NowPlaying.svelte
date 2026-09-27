<script lang="ts">
  import Cover from "../lib/components/Cover.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { formatDuration } from "../lib/format";
  import { formatQuality, type QualityDisplay } from "../lib/quality";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";

  let {
    player,
    nav,
    qualityDisplay,
  }: { player: Player; nav: Nav; qualityDisplay: QualityDisplay } = $props();

  const track = $derived(player.state.current);
  const quality = $derived(
    player.state.stream ? formatQuality(player.state.stream, "compact", player.state.preferred_quality) : "",
  );
  const qualityDetail = $derived(
    player.state.stream ? formatQuality(player.state.stream, "detailed", player.state.preferred_quality) : "",
  );
  const nextUp = $derived.by(() => {
    const current = track;
    if (!current) return [];
    const index = player.state.queue.findIndex((item) => item.track_id === current.track_id);
    if (index < 0) return [];
    return player.state.queue.slice(index + 1, index + 4);
  });
</script>

<div class="now-playing">
  <div class="art">
    {#if track}
      <button class="art-sleeve lit" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
        <Cover uuid={track.cover} size={640} radius={12} alt={`${track.title} artwork`} />
      </button>
    {:else}
      <div class="art-empty lit"><Icon name="music" size={64} /></div>
    {/if}
  </div>

  <div class="meta">
    {#if track}
      <span class="eyebrow">Now playing</span>
      <h2 class="title">{track.title}</h2>
      <p class="byline">{track.artist}{track.album ? ` — ${track.album}` : ""}</p>
      <div class="badges">
        {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
        {#if player.state.bit_perfect}<span class="badge">Bit-perfect</span>{/if}
        {#if qualityDetail}<span class="badge muted">{qualityDetail}</span>{/if}
      </div>

      <hr />

      <span class="eyebrow">Next up</span>
      {#if nextUp.length > 0}
        <ul class="next-up">
          {#each nextUp as item (item.track_id)}
            <li>
              <span class="thumb"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
              <span class="next-title">{item.title}</span>
              <span class="muted next-artist">{item.artist}</span>
              <span class="muted tnum">{formatDuration(item.duration_secs)}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">Queue is empty</p>
      {/if}

      <!-- Track 4: synced lyrics land here (spec §7.2). -->
      <div class="lyrics-slot"></div>
    {:else}
      <span class="eyebrow">Now playing</span>
      <p class="muted">Nothing playing</p>
    {/if}
  </div>
</div>

<style>
  .now-playing {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 420px);
    gap: 32px;
    height: 100%;
    align-items: center;
  }
  .art {
    display: grid;
    place-items: center;
  }
  .art-sleeve {
    display: block;
    width: min(100%, 55vh);
    margin: 0 auto;
  }
  .art-empty {
    width: min(60%, 320px);
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    border-radius: var(--radius-card);
    color: var(--text-muted);
  }
  .eyebrow {
    display: block;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-muted);
    margin-bottom: 8px;
  }
  .title {
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 640;
    letter-spacing: -0.02em;
    margin: 0 0 6px;
  }
  .byline {
    font-family: var(--font-display);
    font-style: italic;
    font-weight: 520;
    color: var(--text-muted);
    margin: 0 0 16px;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 16px;
  }
  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .badge.max {
    border-color: var(--dyn-tertiary);
    color: var(--dyn-tertiary);
  }
  hr {
    border: none;
    border-top: 1px solid var(--line);
    margin: 16px 0;
  }
  .next-up {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .next-up li {
    display: grid;
    grid-template-columns: var(--thumb-size) 1fr auto;
    grid-template-rows: auto auto;
    column-gap: 10px;
    align-items: center;
  }
  .thumb {
    grid-row: 1 / span 2;
    width: var(--thumb-size);
  }
  .next-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next-artist {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next-up .tnum {
    grid-row: 1 / span 2;
  }
  .lyrics-slot {
    margin-top: 24px;
  }
</style>
