<script lang="ts">
  import { dspBadgeTitle } from "../dsp";
  import { formatDuration } from "../format";
  import { formatQuality, type QualityDisplay } from "../quality";
  import { api, type QueueItem, type RepeatMode } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  import type { Nav, Route } from "../nav.svelte";
  import { albumRoute } from "../queueLinks";
  import NameLink from "./NameLink.svelte";
  import QueueArtistLinks from "./QueueArtistLinks.svelte";
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

  /// Name links leave Now Playing so the page they open is visible.
  function go(route: Route) {
    nav.closeNowPlaying();
    nav.goto(route);
  }

  async function openAlbum(item: QueueItem) {
    const route = await albumRoute(item, api.getTrack);
    if (route) go(route);
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
    <SeekBar {player} />

    <div class="zone left">
      <button class="thumb" onclick={() => nav.toggleNowPlaying()} aria-label="Open now playing" title="Open now playing">
        <Cover uuid={current.cover} size={80} radius={6} alt={`${current.title} artwork`} />
        <span class="thumb-hint" aria-hidden="true"><Icon name="chevron-up" size={16} /></span>
      </button>

      <!-- Keyed on the track so a track change fades the new title in rather
           than snapping it (the backdrop crossfades behind it). -->
      {#key current.track_id}
        <div class="now">
          <strong class="title"><NameLink label={current.title} onOpen={() => void openAlbum(current)} /></strong>
          <span class="muted artist"><QueueArtistLinks item={current} {go} /></span>
        </div>
      {/key}

      <button
        class="ibtn icon-heart"
        class:on={fav}
        onclick={() => favorites.toggleTrack(current.track_id)}
        aria-label={fav ? "Unfavorite" : "Favorite"}
        title={fav ? "Unfavorite" : "Favorite"}
      >{#key fav}<span class="heart-ico" class:pop={fav}><Icon name="heart" size={18} /></span>{/key}</button>

      <span class="muted tnum time">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
      {#if player.state.buffering}<span class="muted buffering" role="status">waiting for network…</span>{/if}
    </div>

    <div class="zone center">
      <button
        class="ibtn"
        class:active={player.state.shuffle}
        onclick={() => player.setShuffle(!player.state.shuffle)}
        aria-label="Shuffle"
        aria-pressed={player.state.shuffle}
        title="Shuffle the rest of the queue"
      ><Icon name="shuffle" size={18} /></button>
      <button class="ibtn" onclick={() => player.prev()} aria-label="Previous" title="Previous"><Icon name="skip-back-solid" size={22} /></button>
      <button
        class="ibtn play"
        onclick={() => player.playPause()}
        aria-label={player.state.playing ? "Pause" : "Play"}
        title={player.state.playing ? "Pause" : "Play"}
      >
        <PlayPauseGlyph playing={player.state.playing} size={32} />
      </button>
      <button class="ibtn" onclick={() => player.next()} aria-label="Next" title="Next"><Icon name="skip-forward-solid" size={22} /></button>
      <button
        class="ibtn"
        class:active={player.state.repeat !== "off"}
        onclick={() => player.setRepeat(nextRepeat(player.state.repeat))}
        aria-label={player.state.repeat === "one"
          ? "Repeat one track"
          : player.state.repeat === "all"
            ? "Repeat all"
            : "Repeat off"}
        aria-pressed={player.state.repeat !== "off"}
        title={player.state.repeat === "one" ? "Repeat one" : player.state.repeat === "all" ? "Repeat all" : "Repeat off"}
      ><Icon name={player.state.repeat === "one" ? "repeat-1" : "repeat"} size={18} /></button>
    </div>

    <div class="zone right">
      <button
        class="ibtn"
        class:active={nav.queueOpen}
        onclick={() => nav.toggleQueue()}
        aria-label="Queue"
        title="Queue (Q)"
        aria-pressed={nav.queueOpen}
      ><Icon name="list-music" size={18} /></button>

      <SleepTimerControl {sleepTimer} />

      <VolumeSlider {player} />

      <div class="badges">
        <span class="badge" class:max={maxQuality} title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
        {#if player.state.bit_perfect}<span class="badge bp">Bit-perfect</span>{:else if player.state.dsp_active}<span class="badge" title={dspBadgeTitle(player.state.dsp_stages)}>DSP</span>{/if}
      </div>

      <button
        class="ibtn"
        onclick={() => void mini.enter()}
        aria-label="Mini player"
        title="Mini player"
      ><Icon name="minimize-2" size={18} /></button>

      {#if player.state.bit_perfect_notice && !player.state.error}
        <!-- Output notices can be a sentence long: an icon with the text as
             its tooltip keeps the right zone from reflowing. -->
        <span
          class="notice"
          role="img"
          aria-label={player.state.bit_perfect_notice}
          title={player.state.bit_perfect_notice}
        ><Icon name="alert-triangle" size={16} /></span>
      {/if}
    </div>
  {:else}
    <span class="idle muted"><Icon name="music" size={16} /> Nothing playing</span>
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
    transition: transform var(--dur-micro) var(--ease-enter);
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
    animation: now-in var(--dur-view) var(--ease-enter) backwards;
  }
  @keyframes now-in {
    from {
      opacity: 0;
      transform: translateY(calc(var(--rise) * 0.4));
    }
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
  .artist :global(.name-link:hover) {
    color: var(--text);
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
      transform var(--dur-micro) var(--ease-enter);
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
    transition: transform var(--dur-micro) var(--ease-enter);
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
    animation: heart-pop var(--dur-state) var(--ease-enter);
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
  .notice {
    display: inline-grid;
    place-items: center;
    color: var(--warning);
    cursor: help;
  }
  /* Empty bar: a quiet centered line across all three zones. */
  .idle {
    grid-column: 1 / -1;
    justify-self: center;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
</style>
