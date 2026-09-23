<script lang="ts">
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import type { Player } from "../player.svelte";
  import { formatDuration } from "../format";
  let { tracks, player }: { tracks: Track[]; player: Player } = $props();

  function play(index: number) {
    api.playTracks(tracks.map(trackToQueueItem), index);
  }
</script>

<table style="width:100%; border-collapse:collapse">
  <thead><tr><th style="text-align:left"><button onclick={() => play(0)}>Play all</button></th><th style="text-align:left">#</th><th style="text-align:left">Title</th><th style="text-align:right">Time</th></tr></thead>
  <tbody>
    {#each tracks as t, i (`${t.id}-${i}`)}
      <tr style="border-bottom:1px solid var(--line)">
        <td>
          <button onclick={() => play(i)} aria-label="Play">▶</button>
          <button
            onclick={() => api.queueAdd([trackToQueueItem(t)])}
            aria-label="Add to queue"
            title="Add to queue"
          >＋</button>
        </td>
        <td class="muted">{i + 1}</td>
        <td>{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></td>
        <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
      </tr>
    {/each}
    {#if tracks.length === 0}
      <tr><td colspan="4" class="muted">No tracks</td></tr>
    {/if}
  </tbody>
</table>
