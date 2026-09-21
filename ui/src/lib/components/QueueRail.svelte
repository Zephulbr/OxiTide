<script lang="ts">
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();
</script>

<div style="padding:12px">
  <strong>Queue</strong>
  <p class="muted">{player.state.queue.length} track(s) queued. Wired up in Plan 2.</p>
</div>
