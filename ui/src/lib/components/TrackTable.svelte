<script lang="ts">
  import type { Track } from "../api";
  import { formatDuration } from "../format";
  let { tracks }: { tracks: Track[] } = $props();
</script>

<table style="width:100%; border-collapse:collapse">
  <thead><tr><th style="text-align:left">#</th><th style="text-align:left">Title</th><th style="text-align:right">Time</th></tr></thead>
  <tbody>
    {#each tracks as t, i (t.id)}
      <tr style="border-bottom:1px solid var(--line)">
        <td class="muted">{i + 1}</td>
        <td>{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></td>
        <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
      </tr>
    {/each}
    {#if tracks.length === 0}
      <tr><td colspan="3" class="muted">No tracks</td></tr>
    {/if}
  </tbody>
</table>
