<script lang="ts">
  import { formatDuration } from "../format";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();

  let scrubbing = $state(false);
  let scrubValue = $state(0);

  const position = $derived(
    scrubbing ? scrubValue : player.state.position_ms,
  );
  const duration = $derived(player.state.duration_ms);

  function onSeekInput(e: Event) {
    scrubValue = Number((e.target as HTMLInputElement).value);
  }
  function onSeekCommit() {
    scrubbing = false;
    player.seek(scrubValue);
  }
</script>

<div class="player">
  {#if player.state.current}
    <div class="now">
      <strong>{player.state.current.title}</strong>
      <span class="muted">{player.state.current.artist}</span>
    </div>
    <button onclick={() => player.prev()} aria-label="Previous">⏮</button>
    <button onclick={() => player.playPause()} aria-label="Play/Pause">
      {player.state.playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => player.next()} aria-label="Next">⏭</button>
    <input
      type="range" min="0" max={Math.max(duration, 1)} value={position}
      oninput={onSeekInput}
      onpointerdown={() => (scrubbing = true)}
      onpointerup={onSeekCommit}
      onchange={onSeekCommit}
    />
    <span class="muted">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
    <input
      type="range" min="0" max="1" step="0.01" value={player.state.volume}
      oninput={(e) => player.setVolume(Number((e.target as HTMLInputElement).value))}
      aria-label="Volume"
    />
    {#if player.state.buffering}<span class="muted">buffering…</span>{/if}
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
  {#if player.state.error}<span style="color:#ff6b6b">{player.state.error}</span>{/if}
</div>
