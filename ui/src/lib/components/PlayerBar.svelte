<script lang="ts">
  import { formatDuration } from "../format";
  import { formatQuality, type QualityDisplay } from "../quality";
  import type { RepeatMode } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  import type { Nav } from "../nav.svelte";
  import { appearance } from "../appearance.svelte";
  import { wavePath } from "../wave";
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
      player.seek(Math.round(clampRatio(e.clientX) * duration));
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
    {@const fav = favorites.hasTrack(current.track_id)}
    <div
      class="seek"
      class:dragging
      role="slider"
      tabindex="0"
      aria-label="Playback position"
      aria-valuemin={0}
      aria-valuemax={Math.max(duration, 1)}
      aria-valuenow={Math.round(position)}
      onpointerdown={seekPointer}
      onkeydown={seekKeydown}
    >
      {#if appearance.waveSeek}
        {@const pct = (position / Math.max(duration, 1)) * 100}
        {@const d = wavePath(current.track_id, 800, 14)}
        <div class="seek-wave">
          <svg viewBox="0 0 800 14" preserveAspectRatio="none" aria-hidden="true">
            <path class="wave-base" d={d} fill="none" stroke="var(--line)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
            <clipPath id="seek-played"><rect x="0" y="0" width="{pct}%" height="100%" /></clipPath>
            <path d={d} fill="none" stroke="var(--accent)" stroke-width="1.5" vector-effect="non-scaling-stroke" clip-path="url(#seek-played)" />
          </svg>
        </div>
      {:else}
        <div class="seek-line">
          <div class="seek-fill" style:width="{progress}%"></div>
          <div class="seek-thumb" style:left="{progress}%"></div>
        </div>
      {/if}
    </div>

    <div class="zone left">
      <button class="thumb" onclick={() => nav.toggleNowPlaying()} aria-label="Open now playing" title="Open now playing">
        <Cover uuid={current.cover} size={80} radius={6} alt={`${current.title} artwork`} />
        <span class="thumb-hint" aria-hidden="true"><Icon name="chevron-up" size={16} /></span>
      </button>

      <div class="now">
        <strong class="title">{current.title}</strong>
        <span class="muted artist">{current.artist}</span>
      </div>

      <button
        class="ibtn icon-heart"
        class:on={fav}
        onclick={() => favorites.toggleTrack(current.track_id)}
        aria-label={fav ? "Unfavorite" : "Favorite"}
        title={fav ? "Unfavorite" : "Favorite"}
      >{#key fav}<span class="heart-ico" class:pop={fav}><Icon name="heart" size={18} /></span>{/key}</button>

      <span class="muted tnum time">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
      {#if player.state.buffering}<span class="muted buffering">buffering…</span>{/if}
    </div>

    <div class="zone center">
      <button
        class="ibtn"
        class:active={player.state.shuffle}
        onclick={() => player.setShuffle(!player.state.shuffle)}
        aria-label="Shuffle"
        title="Shuffle the rest of the queue"
      ><Icon name="shuffle" size={18} /></button>
      <button class="ibtn" onclick={() => player.prev()} aria-label="Previous"><Icon name="skip-back" size={20} /></button>
      <button class="play" onclick={() => player.playPause()} aria-label="Play/Pause">
        <span class="play-ico" class:show={!player.state.playing}><Icon name="play" size={22} /></span>
        <span class="play-ico" class:show={player.state.playing}><Icon name="pause" size={22} /></span>
      </button>
      <button class="ibtn" onclick={() => player.next()} aria-label="Next"><Icon name="skip-forward" size={20} /></button>
      <button
        class="ibtn"
        class:active={player.state.repeat !== "off"}
        onclick={() => player.setRepeat(nextRepeat(player.state.repeat))}
        aria-label="Repeat"
        title={player.state.repeat === "one" ? "Repeat one" : player.state.repeat === "all" ? "Repeat all" : "Repeat off"}
      ><Icon name={player.state.repeat === "one" ? "repeat-1" : "repeat"} size={18} /></button>
    </div>

    <div class="zone right">
      <button
        class="ibtn"
        class:active={nav.queueOpen}
        onclick={() => nav.toggleQueue()}
        aria-label="Queue"
        title="Queue"
        aria-pressed={nav.queueOpen}
      ><Icon name="list-music" size={18} /></button>

      <label class="volume">
        <Icon name="volume-2" size={18} />
        <input
          type="range" min="0" max="1" step="0.01" value={player.state.volume}
          oninput={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value))}
          onchange={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value), true)}
          aria-label="Volume"
        />
      </label>

      <div class="badges">
        <span class="badge" class:max={maxQuality} title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
        {#if player.state.bit_perfect}<span class="badge bp">Bit-perfect</span>{/if}
      </div>
      {#if player.state.bit_perfect_notice}
        <span style="color:var(--warning)">{player.state.bit_perfect_notice}</span>
      {/if}
    </div>
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
  {#if player.state.error}
    <div class="player-error">
      <span style="color:var(--danger)">{player.state.error}</span>
      <button onclick={() => player.retry()} aria-label="Retry playback">Retry</button>
    </div>
  {/if}
</div>

<style>
  /* Three zones: track info left, transport dead-center, queue/volume/quality
     right. The 1fr/auto/1fr grid keeps the middle centered no matter how wide
     the sides get. */
  .player {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    transition: border-top-color var(--dur-micro);
  }
  /* The bar's top border reads as a separator through the seek; fade it out
     while the seek is interactive so only the progress shows. */
  .player:has(.seek:hover),
  .player:has(.seek:focus-visible),
  .player:has(.seek.dragging) {
    border-top-color: transparent;
  }
  .zone {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .zone.left { justify-self: start; }
  .zone.center { justify-self: center; gap: 6px; }
  .zone.right { justify-self: end; }

  /* Seek across the top edge: rests at the old hover thickness, grows a touch
     more (+ thumb for the line variant) on hover/focus/drag. */
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
    height: 4px;
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
  .seek:focus-visible .seek-line,
  .seek.dragging .seek-line {
    height: 6px;
  }
  .seek:hover .seek-thumb,
  .seek:focus-visible .seek-thumb,
  .seek.dragging .seek-thumb {
    transform: translate(-50%, -50%) scale(1);
  }
  /* Stylized wave (Atmosphere): full amplitude + opacity at rest; interactive
     grows it a touch and brightens the unplayed portion. */
  .seek-wave {
    width: 100%;
    height: 10px;
    transform-origin: top;
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .seek-wave svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .seek-wave .wave-base {
    transition: stroke var(--dur-micro);
  }
  .seek:hover .seek-wave,
  .seek:focus-visible .seek-wave,
  .seek.dragging .seek-wave {
    transform: scaleY(1.3);
  }
  .seek:hover .wave-base,
  .seek:focus-visible .wave-base,
  .seek.dragging .wave-base {
    stroke: var(--text-muted);
  }

  .thumb {
    position: relative;
    flex: 0 0 auto;
    width: 52px;
    height: 52px;
    padding: 0;
    border-radius: 6px;
    overflow: hidden;
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .thumb:hover {
    transform: scale(1.04);
  }
  /* Expand hint: the cover opens Now Playing, so say so on hover. */
  .thumb-hint {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    color: var(--text);
    opacity: 0;
    transition: opacity var(--dur-micro);
  }
  .thumb:hover .thumb-hint,
  .thumb:focus-visible .thumb-hint {
    opacity: 1;
  }

  .now {
    display: flex;
    flex-direction: column;
    min-width: 120px;
    max-width: 260px;
  }
  .title {
    font-size: 15px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    display: inline-grid;
    place-items: center;
  }

  /* Icon buttons: round hit area, hover halo + lift, deeper press than the
     global 0.98 for tactility. */
  .ibtn {
    position: relative;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    transition:
      color var(--dur-micro),
      background-color var(--dur-micro),
      transform var(--dur-micro) var(--ease-in);
  }
  .ibtn:hover {
    background: var(--surface-3);
    transform: translateY(-1px);
  }
  .player .ibtn:active {
    transform: scale(0.92);
  }
  /* On/off affordance for the toggles: accent color + a dot that scales in. */
  .ibtn::after {
    content: "";
    position: absolute;
    bottom: 0;
    left: 50%;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--accent);
    transform: translateX(-50%) scale(0);
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .ibtn.active {
    color: var(--accent);
  }
  .ibtn.active::after {
    transform: translateX(-50%) scale(1);
  }

  .icon-heart.on {
    color: var(--accent);
  }
  .icon-heart.on :global(svg) {
    fill: currentColor;
  }
  /* Favoriting pops the heart; the keyed remount replays the animation. */
  .heart-ico {
    display: inline-grid;
    place-items: center;
  }
  .heart-ico.pop {
    animation: heart-pop var(--dur-state) var(--ease-in);
  }
  @keyframes heart-pop {
    0% { transform: scale(1); }
    40% { transform: scale(1.35); }
    100% { transform: scale(1); }
  }

  .play {
    position: relative;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    transition:
      transform var(--dur-micro) var(--ease-in),
      box-shadow var(--dur-state) var(--ease-in);
  }
  .play:hover {
    transform: scale(1.06);
    box-shadow: 0 0 16px color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .player .play:active {
    transform: scale(0.94);
  }
  /* Play/pause swap: both icons stacked, the visible one scales + fades in. */
  .play-ico {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity var(--dur-state) var(--ease-in),
      transform var(--dur-state) var(--ease-in);
  }
  .play-ico.show {
    opacity: 1;
    transform: scale(1);
  }

  .time {
    font-size: 12px;
    white-space: nowrap;
  }
  .buffering {
    font-size: 12px;
    white-space: nowrap;
  }
  .badges {
    display: flex;
    gap: 6px;
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
  @media (max-width: 900px) {
    .badge.bp { display: none; }
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
    transition: color var(--dur-micro);
  }
  .volume:hover {
    color: var(--text);
  }
  .volume input[type="range"] {
    width: 80px;
    accent-color: var(--accent);
  }

  /* Transient errors float over the bar instead of reflowing the zones. */
  .player-error {
    position: absolute;
    right: 14px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    background: var(--surface-3);
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
  }
</style>
