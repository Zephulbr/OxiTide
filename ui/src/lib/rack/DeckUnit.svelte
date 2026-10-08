<script lang="ts">
  import type { DeckFace, RepeatMode } from "../api";
  import { appearance } from "../appearance.svelte";
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import type { Favorites } from "../favorites.svelte";
  import type { Nav } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import { formatQuality } from "../quality";
  import { calendar, clock, currentIndex, formatLine, remainingMs, repeatAnnunciator, totalMs, upNext } from "./deck";
  import { levels } from "./levels.svelte";
  import BarMeter from "./meters/BarMeter.svelte";
  import SevenSeg from "./SevenSeg.svelte";
  import VfdSeek from "./VfdSeek.svelte";

  /// The deck (spec §6.1): a compact face per the Deck face setting, or the
  /// turntable when expanded (Now Playing in Rack). Every player-bar duty
  /// lives here: transport, seek, shuffle/repeat, favourite, queue.
  let {
    player,
    nav,
    favorites,
    face,
    expanded,
  }: { player: Player; nav: Nav; favorites: Favorites; face: DeckFace; expanded: boolean } = $props();

  const state = $derived(player.state);
  const current = $derived(state.current);
  const position = $derived(player.pendingSeek ?? state.position_ms);
  const index = $derived(currentIndex(state));
  const next = $derived(upNext(state, 3));
  const fav = $derived(current ? favorites.hasTrack(current.track_id) : false);
  const message = $derived(state.error || (state.bit_perfect_notice ?? ""));
  const progress = $derived(state.duration_ms > 0 ? Math.min(1, position / state.duration_ms) : 0);
  const spinning = $derived(state.playing && !appearance.reducedMotion);
  const quality = $derived(formatQuality(state.stream, "compact", state.preferred_quality));

  // The Display face's bar meter needs the level feed only while shown.
  $effect(() => {
    if (face === "display" && !expanded) return levels.use();
  });

  function nextRepeat(mode: RepeatMode): RepeatMode {
    return mode === "off" ? "all" : mode === "all" ? "one" : "off";
  }
</script>

<section class="rk-unit deck" class:expanded aria-label={expanded ? "Turntable" : "Deck"}>
  <div class="rk-head">
    <h2 class="rk-name">{expanded ? "Direct drive deck" : face === "cd" ? "Compact disc player" : "Network transport"}</h2>
    <div class="headright">
      <span class="rk-silk">{expanded ? "33⅓" : `TIDAL · ${quality}`}</span>
      <button
        class="rk-key sm"
        onclick={() => nav.toggleNowPlaying()}
        aria-label={expanded ? "Close now playing" : "Open now playing"}
        title={expanded ? "Close now playing" : "Open now playing"}
        disabled={!current && !expanded}
      ><Icon name={expanded ? "chevron-down" : "chevron-up"} size={12} /></button>
    </div>
  </div>

  {#if expanded}
    <button class="turntable" onclick={() => nav.toggleNowPlaying()} aria-label="Close now playing">
      <span class="plinth">
        <span class="record">
          <span class="label" class:spin={spinning}>
            {#if current}<Cover uuid={current.cover} size={320} radius={999} alt="" />{/if}
          </span>
        </span>
      </span>
      <svg class="tonearm" viewBox="0 0 220 220" aria-hidden="true">
        <defs>
          <linearGradient id="rk-chrome" x1="0" x2="1">
            <stop offset="0" stop-color="#7d7871" /><stop offset=".5" stop-color="#eeeae4" /><stop offset="1" stop-color="#6e6a63" />
          </linearGradient>
        </defs>
        <circle cx="170" cy="30" r="17" fill="#1a1816" stroke="#5a554e" stroke-width="2" />
        <circle cx="170" cy="30" r="9" fill="url(#rk-chrome)" />
        <g transform="rotate({current ? progress * 22 : -14} 170 30)" class="arm">
          <rect x="178" y="22" width="18" height="16" rx="2" fill="#3a3632" />
          <path d="M170 30 L150 150 L128 182" stroke="url(#rk-chrome)" stroke-width="5" fill="none" stroke-linecap="round" />
          <rect x="113" y="176" width="26" height="15" rx="2" transform="rotate(32 126 183)" fill="#1d1b19" stroke="#6b665f" />
        </g>
      </svg>
    </button>
  {:else if face === "transport"}
    <div class="faceplate transport">
      <button class="rk-win art" onclick={() => nav.toggleNowPlaying()} aria-label="Open now playing" disabled={!current}>
        {#if current}<Cover uuid={current.cover} size={160} radius={2} alt={`${current.title} artwork`} />{/if}
      </button>
      <div class="rk-win nextup">
        <div class="nexthead">Up next</div>
        <ol>
          {#each next as item, i (item.track_id + ":" + i)}
            <li><span>{index + i + 2}</span><b>{item.title}</b><i>{clock(item.duration_secs * 1000)}</i></li>
          {:else}
            <li class="empty">End of queue</li>
          {/each}
        </ol>
      </div>
    </div>
  {:else if face === "cd"}
    <div class="faceplate cd">
      <div class="rk-tray tray">
        <div class="drawer"><span>Digital audio</span></div>
        <button class="rk-key eject" onclick={() => nav.toggleQueue()} aria-label="Queue" title="Queue" aria-pressed={nav.queueOpen}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><polygon points="12 4 21 15 3 15" fill="currentColor" /><rect x="3" y="18" width="18" height="2.6" fill="currentColor" /></svg>
        </button>
      </div>
      <div class="rk-vfd calendar" aria-hidden="true">
        <div class="cal">
          {#each calendar(state.queue.length, index) as cell (cell.n)}
            <span class={cell.state}>{cell.n}</span>
          {/each}
        </div>
        <div class="remain">
          <div class="rk-anno"><span class="on">REMAIN</span></div>
          <div class="big">-{clock(remainingMs({ ...state, position_ms: position }))}</div>
          <div class="rk-anno"><span class="on">{state.queue.length} TR</span><span class="on">{clock(totalMs(state.queue))}</span></div>
        </div>
      </div>
    </div>
  {/if}

  <div class="rk-vfd display" class:tall={face === "display" && !expanded}>
    {#if face === "display" && !expanded}
      <div class="vfdtop" aria-hidden="true">
        <div class="row">
          <div class="rk-anno">{#each formatLine(state.stream, state.output_mode) as tag (tag)}<span class="on">{tag}</span>{/each}</div>
          <div class="rk-anno"><span class="on">TIDAL {quality}</span></div>
        </div>
        <BarMeter snapshot={levels.snapshot} header={false} />
        <div class="rk-vfd-text sub nextline">
          <span class="dim">NEXT</span> {next[0]?.title ?? "—"} {#if next[0]}<span class="dim">{clock(next[0].duration_secs * 1000)}</span>{/if}
        </div>
      </div>
    {/if}
    <div class="row">
      <div class="titles">
        <div class="rk-vfd-text title">{current ? current.title : "No disc"}</div>
        <div class="rk-vfd-text sub" class:warn={message !== ""}>
          {message !== "" ? message : current ? `${current.artist}, ${current.album}` : "Choose something to play"}
        </div>
      </div>
      <SevenSeg value={clock(position)} />
    </div>
    <VfdSeek {player} />
    <div class="row annos" aria-hidden="true">
      <div class="rk-anno">
        <span class:on={state.playing}>PLAY</span>
        <span class:on={state.buffering}>BUF</span>
        <span class:on={state.repeat !== "off"}>{repeatAnnunciator(state.repeat) ?? "RPT"}</span>
        <span class:on={state.shuffle}>SHUF</span>
        <span class:on={state.crossfade_ms > 0}>XFADE</span>
      </div>
      <div class="rk-anno"><span>TRK</span><span class:on={index >= 0}>{String(index + 1).padStart(2, "0")}/{String(state.queue.length).padStart(2, "0")}</span></div>
    </div>
  </div>

  <div class="deckbar">
    <div class="rk-tray transport-keys" role="group" aria-label="Transport">
      <button class="rk-key" onclick={() => player.prev()} aria-label="Previous" disabled={state.queue.length === 0}><Icon name="skip-back-solid" size={14} /></button>
      <button class="rk-key" class:lit={state.playing} onclick={() => player.playPause()} aria-label={state.playing ? "Pause" : "Play"} disabled={!current}>
        <Icon name={state.playing ? "pause" : "play"} size={14} />
      </button>
      <button class="rk-key" onclick={() => player.next()} aria-label="Next" disabled={state.queue.length === 0}><Icon name="skip-forward-solid" size={14} /></button>
    </div>
    <div class="rk-tray" role="group" aria-label="Play mode">
      <button class="rk-key sm" aria-pressed={state.shuffle} onclick={() => player.setShuffle(!state.shuffle)} title="Shuffle the rest of the queue">Shuffle</button>
      <button
        class="rk-key sm"
        aria-pressed={state.repeat !== "off"}
        onclick={() => player.setRepeat(nextRepeat(state.repeat))}
        aria-label={state.repeat === "one" ? "Repeat one track" : state.repeat === "all" ? "Repeat all" : "Repeat off"}
      >{state.repeat === "one" ? "Repeat 1" : "Repeat"}</button>
      <button
        class="rk-key sm"
        aria-pressed={fav}
        disabled={!current}
        onclick={() => current && favorites.toggleTrack(current.track_id)}
        aria-label={fav ? "Unfavorite" : "Favorite"}
        title={fav ? "Unfavorite" : "Favorite"}
      ><Icon name="heart" size={12} /></button>
      <button class="rk-key sm" aria-pressed={nav.queueOpen} onclick={() => nav.toggleQueue()} title="Queue">Queue</button>
    </div>
  </div>
</section>

<style>
  .deck {
    --pl: clamp(104px, calc(100vh - 600px), 196px);
    display: flex;
    flex-direction: column;
  }
  .headright {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .headright .rk-key {
    padding: 0 8px 6px;
  }
  .faceplate {
    margin-bottom: 8px;
  }
  .transport {
    display: grid;
    grid-template-columns: var(--pl) minmax(0, 1fr);
    gap: 10px;
    height: var(--pl);
  }
  .art {
    padding: 6px;
    border: 0;
    cursor: pointer;
  }
  .art :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .nextup {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 9px 12px;
  }
  .nexthead {
    padding-bottom: 7px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    font: 600 10px/1 var(--font-ui);
    letter-spacing: 0.05em;
    color: var(--rack-silk-dim);
  }
  ol {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-evenly;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: baseline;
    font-size: 12px;
  }
  li span,
  li i {
    font-style: normal;
    font-size: 11px;
    color: var(--rack-screen-muted);
    font-variant-numeric: tabular-nums;
  }
  li b {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  li.empty {
    display: block;
    color: var(--rack-screen-muted);
  }

  .cd {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: var(--pl);
  }
  .tray {
    display: flex;
    height: 34px;
    box-sizing: border-box;
  }
  .drawer {
    flex: 1;
    position: relative;
    display: grid;
    place-items: center;
    border-radius: 2px;
    background: linear-gradient(180deg, #27251f, #161513 55%, #0e0d0c);
    border-top: 1px solid #3b3833;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
  }
  .drawer::before {
    content: "";
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 5px;
    height: 1px;
    background: #000;
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.05);
  }
  .drawer span {
    margin-top: -4px;
    font: 600 8.5px/1 var(--font-ui);
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--rack-silk-dim);
  }
  .eject {
    min-width: 44px;
    height: 100%;
  }
  .eject svg {
    width: 12px;
    height: 12px;
  }
  .calendar {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 16px;
    align-items: center;
  }
  .cal {
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 4px 2px;
  }
  .cal span {
    padding: 3px 0;
    border-radius: 2px;
    text-align: center;
    font: 800 10px/1 var(--font-ui);
    font-variant-numeric: tabular-nums;
    opacity: 0.1;
  }
  .cal .present {
    opacity: 0.8;
    text-shadow: 0 0 5px var(--rack-vfd);
  }
  .cal .current {
    opacity: 1;
    box-shadow: inset 0 0 0 1px var(--rack-vfd), 0 0 6px color-mix(in srgb, var(--rack-vfd) 35%, transparent);
  }
  .remain {
    display: grid;
    justify-items: end;
    gap: 4px;
  }
  .big {
    font: 700 20px/1 var(--font-ui);
    font-variant-numeric: tabular-nums;
    text-shadow: 0 0 8px var(--rack-vfd), 0 0 1px var(--rack-vfd);
  }

  .display .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .titles {
    min-width: 0;
  }
  .warn {
    color: var(--warning);
    text-shadow: none;
  }
  .annos {
    margin-top: 6px;
  }
  .display.tall {
    padding: 12px 14px 10px;
  }
  .display.tall .title {
    font-size: 16px;
  }
  .vfdtop {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 8px;
    height: calc(var(--pl) - 12px);
    padding-bottom: 10px;
    margin-bottom: 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--rack-vfd) 12%, transparent);
  }
  .nextline .dim {
    opacity: 0.55;
  }

  .deckbar {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    margin-top: 10px;
  }
  .transport-keys {
    flex: 1;
  }
  .transport-keys .rk-key {
    flex: 1;
  }

  /* Expanded deck: the turntable (DESIGN §5.10). */
  .deck.expanded {
    --pl: min(440px, 52vh);
    min-height: 0;
  }
  .turntable {
    position: relative;
    flex: 1;
    min-height: calc(var(--pl) + 10px);
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .plinth {
    position: relative;
    display: block;
    width: var(--pl);
    height: var(--pl);
    border-radius: 50%;
    background: radial-gradient(circle, #0b0a09 64%, #2d2a27 65%, #121110 69%, #3d3934 70%, #1a1816 72%);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.6);
  }
  .record {
    position: absolute;
    inset: 9px;
    border-radius: 50%;
    background:
      conic-gradient(from 20deg, rgba(255, 255, 255, 0) 0deg, rgba(255, 255, 255, 0.1) 30deg, rgba(255, 255, 255, 0) 70deg, rgba(255, 255, 255, 0) 190deg, rgba(255, 255, 255, 0.07) 215deg, rgba(255, 255, 255, 0) 250deg),
      repeating-radial-gradient(circle, #121212 0 1px, #070707 1px 2.2px);
  }
  .label {
    position: absolute;
    inset: 34%;
    overflow: hidden;
    border-radius: 50%;
    background: #222;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.6);
  }
  .label :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .label::after {
    content: "";
    position: absolute;
    inset: 46%;
    border-radius: 50%;
    background: #c9c3ba;
    box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.6);
  }
  .label.spin {
    animation: rk-spin 1.8s linear infinite;
  }
  @keyframes rk-spin {
    to {
      transform: rotate(360deg);
    }
  }
  .tonearm {
    position: absolute;
    left: 50%;
    top: 50%;
    width: calc(var(--pl) * 1.12);
    height: calc(var(--pl) * 1.12);
    transform: translate(-28%, -58%);
    pointer-events: none;
  }
  .arm {
    transition: transform 300ms;
  }
  @media (prefers-reduced-motion: reduce) {
    .arm {
      transition: none;
    }
  }
  /* Short windows: the face windows give way; VFD and keys stay (spec §6.9). */
  @media (max-height: 680px) {
    .deck:not(.expanded) .faceplate,
    .deck:not(.expanded) .vfdtop {
      display: none;
    }
  }
</style>
