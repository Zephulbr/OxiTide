<script lang="ts">
  import { formatDuration } from "../format";
  import { formatQuality, type QualityDisplay } from "../quality";
  import type { RepeatMode } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  let { player, qualityDisplay, favorites }: { player: Player; qualityDisplay: QualityDisplay; favorites: Favorites } = $props();

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const quality = $derived(
    formatQuality(player.state.stream, qualityDisplay, player.state.preferred_quality),
  );

  function nextRepeat(mode: RepeatMode): RepeatMode {
    if (mode === "off") return "all";
    if (mode === "all") return "one";
    return "off";
  }
</script>

<div class="player">
  {#if player.state.current}
    {@const trackId = player.state.current.track_id}
    <div class="now">
      <strong>{player.state.current.title}</strong>
      <span class="muted">{player.state.current.artist}</span>
      <span class="badge" title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
      {#if player.state.bit_perfect}
        <span class="badge" title="Exclusive output at the source's native format">Bit-perfect</span>
      {/if}
    </div>
    <button
      class:on={favorites.hasTrack(trackId)}
      onclick={() => favorites.toggleTrack(trackId)}
      aria-label={favorites.hasTrack(trackId) ? "Unfavorite" : "Favorite"}
      title={favorites.hasTrack(trackId) ? "Unfavorite" : "Favorite"}
    >{favorites.hasTrack(trackId) ? "♥" : "♡"}</button>
    <button onclick={() => player.prev()} aria-label="Previous">⏮</button>
    <button onclick={() => player.playPause()} aria-label="Play/Pause">
      {player.state.playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => player.next()} aria-label="Next">⏭</button>
    <button
      class:active={player.state.shuffle}
      onclick={() => player.setShuffle(!player.state.shuffle)}
      aria-label="Shuffle"
      title="Shuffle the rest of the queue"
    >🔀</button>
    <button
      class:active={player.state.repeat !== "off"}
      onclick={() => player.setRepeat(nextRepeat(player.state.repeat))}
      aria-label="Repeat"
      title={player.state.repeat === "one" ? "Repeat one" : player.state.repeat === "all" ? "Repeat all" : "Repeat off"}
    >{player.state.repeat === "one" ? "🔂" : "🔁"}</button>
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
  {#if player.state.error}
    <span style="color:#ff6b6b">{player.state.error}</span>
    <button onclick={() => player.retry()} aria-label="Retry playback">Retry</button>
  {/if}
</div>

<style>
  .active { color: var(--accent); }
  .on { color: var(--accent); }
</style>
