<script lang="ts">
  import { formatDuration } from "../format";
  import { formatQuality, type QualityDisplay } from "../quality";
  import type { RepeatMode } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  import type { Nav } from "../nav.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let {
    player,
    qualityDisplay,
    favorites,
    nav,
  }: { player: Player; qualityDisplay: QualityDisplay; favorites: Favorites; nav: Nav } = $props();

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const progress = $derived((position / Math.max(duration, 1)) * 100);
  const quality = $derived(
    formatQuality(player.state.stream, qualityDisplay, player.state.preferred_quality),
  );
  const maxQuality = $derived(
    formatQuality(player.state.stream, "compact", player.state.preferred_quality) === "MAX",
  );

  function seekPointer(event: PointerEvent) {
    const el = event.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const clampRatio = (clientX: number) => Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    dragging = true;
    dragValue = clampRatio(event.clientX) * duration;
    el.setPointerCapture(event.pointerId);
    const onMove = (e: PointerEvent) => {
      dragValue = clampRatio(e.clientX) * duration;
    };
    const onUp = (e: PointerEvent) => {
      dragging = false;
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", onUp);
      player.seek(clampRatio(e.clientX) * duration);
    };
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", onUp);
  }

  function seekKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowLeft") player.seek(Math.max(0, position - 5000));
    else if (event.key === "ArrowRight") {
      const limit = duration > 0 ? duration : position + 5000;
      player.seek(Math.min(limit, position + 5000));
    } else return;
    event.preventDefault();
  }

  function nextRepeat(mode: RepeatMode): RepeatMode {
    if (mode === "off") return "all";
    if (mode === "all") return "one";
    return "off";
  }
</script>

<div class="player">
  {#if player.state.current}
    {@const current = player.state.current}
    <div
      class="seek"
      role="slider"
      tabindex="0"
      aria-label="Playback position"
      aria-valuemin={0}
      aria-valuemax={Math.max(duration, 1)}
      aria-valuenow={Math.round(position)}
      onpointerdown={seekPointer}
      onkeydown={seekKeydown}
    >
      <div class="seek-line">
        <div class="seek-fill" style:width="{progress}%"></div>
        <div class="seek-thumb" style:left="{progress}%"></div>
      </div>
    </div>

    <button class="thumb" onclick={() => nav.toggleNowPlaying()} aria-label="Open now playing" title="Open now playing">
      <Cover uuid={current.cover} size={80} radius={6} alt={`${current.title} artwork`} />
    </button>

    <div class="now">
      <strong class="title">{current.title}</strong>
      <span class="muted artist">{current.artist}</span>
    </div>

    <button
      class="icon-heart"
      class:on={favorites.hasTrack(current.track_id)}
      onclick={() => favorites.toggleTrack(current.track_id)}
      aria-label={favorites.hasTrack(current.track_id) ? "Unfavorite" : "Favorite"}
      title={favorites.hasTrack(current.track_id) ? "Unfavorite" : "Favorite"}
    ><Icon name="heart" size={16} /></button>

    <div class="transport">
      <button onclick={() => player.prev()} aria-label="Previous"><Icon name="skip-back" /></button>
      <button class="play" onclick={() => player.playPause()} aria-label="Play/Pause">
        <Icon name={player.state.playing ? "pause" : "play"} size={20} />
      </button>
      <button onclick={() => player.next()} aria-label="Next"><Icon name="skip-forward" /></button>
      <button
        class:active={player.state.shuffle}
        onclick={() => player.setShuffle(!player.state.shuffle)}
        aria-label="Shuffle"
        title="Shuffle the rest of the queue"
      ><Icon name="shuffle" size={16} /></button>
      <button
        class:active={player.state.repeat !== "off"}
        onclick={() => player.setRepeat(nextRepeat(player.state.repeat))}
        aria-label="Repeat"
        title={player.state.repeat === "one" ? "Repeat one" : player.state.repeat === "all" ? "Repeat all" : "Repeat off"}
      ><Icon name={player.state.repeat === "one" ? "repeat-1" : "repeat"} size={16} /></button>
    </div>

    <span class="muted tnum time">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>

    <div class="badges">
      <span class="badge" class:max={maxQuality} title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
      {#if player.state.bit_perfect}<span class="badge">Bit-perfect</span>{/if}
    </div>

    <label class="volume">
      <Icon name="volume-2" size={16} />
      <input
        type="range" min="0" max="1" step="0.01" value={player.state.volume}
        oninput={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value))}
        onchange={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value), true)}
        aria-label="Volume"
      />
    </label>

    {#if player.state.buffering}<span class="muted">buffering…</span>{/if}
    {#if player.state.bit_perfect_notice}
      <span style="color:var(--warning)">{player.state.bit_perfect_notice}</span>
    {/if}
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
  {#if player.state.error}
    <span style="color:var(--danger)">{player.state.error}</span>
    <button onclick={() => player.retry()} aria-label="Retry playback">Retry</button>
  {/if}
</div>

<style>
  .player {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 14px;
  }
  /* Hairline seek across the top edge: thin at rest, grows + thumb on hover. */
  .seek {
    position: absolute;
    left: 0;
    right: 0;
    top: -1px;
    height: 12px;
    cursor: pointer;
    display: flex;
    align-items: flex-start;
  }
  .seek-line {
    position: relative;
    width: 100%;
    height: 2px;
    margin-top: 0;
    background: var(--line);
    transition: height var(--dur-micro) var(--ease-in);
  }
  .seek-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent);
  }
  .seek-thumb {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    transform: translate(-50%, -50%) scale(0);
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .seek:hover .seek-line,
  .seek:focus-visible .seek-line {
    height: 4px;
  }
  .seek:hover .seek-thumb,
  .seek:focus-visible .seek-thumb {
    transform: translate(-50%, -50%) scale(1);
  }

  .thumb {
    flex: 0 0 auto;
    width: 48px;
    height: 48px;
    padding: 0;
    border-radius: 6px;
    overflow: hidden;
  }
  .now {
    display: flex;
    flex-direction: column;
    min-width: 120px;
    max-width: 260px;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    display: inline-grid;
    place-items: center;
  }
  .icon-heart.on {
    color: var(--accent);
  }
  .icon-heart.on :global(svg) {
    fill: currentColor;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .play {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
  }
  .transport .active {
    color: var(--accent);
  }
  .time {
    font-size: 11.5px;
    white-space: nowrap;
  }
  .badges {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .badge.max {
    border-color: var(--dyn-tertiary);
    color: var(--dyn-tertiary);
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
  }
  .volume input[type="range"] {
    width: 90px;
    accent-color: var(--accent);
  }
</style>
