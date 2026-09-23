<script lang="ts">
  import { formatDuration } from "../format";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const quality = $derived(player.state.preferred_quality);
</script>

<div class="player">
  {#if player.state.current}
    <div class="now">
      <strong>{player.state.current.title}</strong>
      <span class="muted">{player.state.current.artist}</span>
      <span class="badge" title="Streaming quality">{quality}</span>
      {#if player.state.bit_perfect}
        <span class="badge" title="Exclusive output at the source's native format">Bit-perfect</span>
      {/if}
    </div>
    <button onclick={() => player.prev()} aria-label="Previous">⏮</button>
    <button onclick={() => player.playPause()} aria-label="Play/Pause">
      {player.state.playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => player.next()} aria-label="Next">⏭</button>
    <input
      type="range" min="0" max={Math.max(duration, 1)} value={position}
      oninput={(e) => {
        dragging = true;
        dragValue = Number((e.target as HTMLInputElement).value);
      }}
      onchange={(e) => {
        dragging = false;
        player.seek(Number((e.target as HTMLInputElement).value));
      }}
      aria-label="Playback position"
    />
    <span class="muted">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
    <input
      type="range" min="0" max="1" step="0.01" value={player.state.volume}
      oninput={(e) => player.setVolume(Number((e.target as HTMLInputElement).value))}
      onchange={(e) => player.setVolume(Number((e.target as HTMLInputElement).value), true)}
      aria-label="Volume"
    />
    {#if player.state.buffering}<span class="muted">buffering…</span>{/if}
    {#if player.state.bit_perfect_notice}
      <span style="color:#e0a030">{player.state.bit_perfect_notice}</span>
    {/if}
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
  {#if player.state.error}<span style="color:#ff6b6b">{player.state.error}</span>{/if}
</div>
