<script lang="ts">
  import { formatDuration } from "../format";
  import { formatQuality, type QualityDisplay } from "../quality";
  import type { RepeatMode } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  import type { Nav } from "../nav.svelte";
  import type { Mini } from "../mini.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlayPauseGlyph from "./PlayPauseGlyph.svelte";
  import SeekBar from "./SeekBar.svelte";
  import VolumeSlider from "./VolumeSlider.svelte";
  import SleepTimerControl from "./SleepTimerControl.svelte";
  import type { SleepTimer } from "../sleepTimer.svelte";

  let {
    player,
    qualityDisplay,
    favorites,
    nav,
    mini,
    sleepTimer,
  }: {
    player: Player;
    qualityDisplay: QualityDisplay;
    favorites: Favorites;
    nav: Nav;
    mini: Mini;
    sleepTimer: SleepTimer;
  } = $props();

  const position = $derived(player.pendingSeek ?? player.state.position_ms);
  const duration = $derived(player.state.duration_ms);
  const quality = $derived(
    formatQuality(player.state.stream, qualityDisplay, player.state.preferred_quality),
  );
  const maxQuality = $derived(
    formatQuality(player.state.stream, "compact", player.state.preferred_quality) === "MAX",
  );

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
    <SeekBar {player} />

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
      <button class="ibtn" onclick={() => player.prev()} aria-label="Previous"><Icon name="skip-back-solid" size={22} /></button>
      <button class="ibtn play" onclick={() => player.playPause()} aria-label="Play/Pause">
        <PlayPauseGlyph playing={player.state.playing} size={32} />
      </button>
      <button class="ibtn" onclick={() => player.next()} aria-label="Next"><Icon name="skip-forward-solid" size={22} /></button>
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

      <SleepTimerControl {sleepTimer} />

      <VolumeSlider {player} />

      <div class="badges">
        <span class="badge" class:max={maxQuality} title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
        {#if player.state.bit_perfect}<span class="badge bp">Bit-perfect</span>{/if}
      </div>

      <button
        class="ibtn"
        onclick={() => void mini.enter()}
        aria-label="Mini player"
        title="Mini player"
      ><Icon name="minimize-2" size={18} /></button>

      {#if player.state.bit_perfect_notice && !player.state.error}
        <span style="color:var(--warning)">{player.state.bit_perfect_notice}</span>
      {/if}
    </div>
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
</div>

<style>
  /* Three zones: track info left, transport dead-center, queue/volume/quality
     right. The 1fr/auto/1fr grid keeps the middle centered no matter how wide
     the sides get. The box (position/height) comes from app.css so the bar
     floats over the content. */
  .player {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    padding: 0 16px;
  }
  .zone {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .zone.left { justify-self: start; }
  .zone.center { justify-self: center; gap: 10px; }
  .zone.right { justify-self: end; }

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
    width: 50px;
    height: 50px;
    color: var(--accent);
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
</style>
