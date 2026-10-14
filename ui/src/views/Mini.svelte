<script lang="ts">
  import type { RepeatMode } from "../lib/api";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Mini } from "../lib/mini.svelte";
  import type { Player } from "../lib/player.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import PlayPauseGlyph from "../lib/components/PlayPauseGlyph.svelte";
  import { focusTrap } from "../lib/focusTrap";
  import QueueRail from "../lib/components/QueueRail.svelte";
  import SeekBar from "../lib/components/SeekBar.svelte";
  import VolumeSlider from "../lib/components/VolumeSlider.svelte";

  let {
    player,
    favorites,
    mini,
  }: { player: Player; favorites: Favorites; mini: Mini } = $props();

  function nextRepeat(mode: RepeatMode): RepeatMode {
    if (mode === "off") return "all";
    if (mode === "all") return "one";
    return "off";
  }
</script>

<div class="mini">
  {#if player.state.current}
    {@const current = player.state.current}
    {@const fav = favorites.hasTrack(current.track_id)}
    <div class="art">
      <Cover uuid={current.cover} size={320} radius={10} alt={`${current.title} artwork`} />
    </div>
    <div class="meta">
      <strong class="title">{current.title}</strong>
      <span class="muted artist">{current.artist}</span>
    </div>
    <SeekBar {player} variant="inline" />
    <div class="transport">
      <button class="ibtn" onclick={() => player.prev()} aria-label="Previous">
        <Icon name="skip-back-solid" size={20} />
      </button>
      <button class="ibtn play" onclick={() => player.playPause()} aria-label="Play/Pause">
        <PlayPauseGlyph playing={player.state.playing} size={28} />
      </button>
      <button class="ibtn" onclick={() => player.next()} aria-label="Next">
        <Icon name="skip-forward-solid" size={20} />
      </button>
    </div>
    <div class="secondary">
      <button
        class="ibtn icon-heart"
        class:on={fav}
        onclick={() => favorites.toggleTrack(current.track_id)}
        aria-label={fav ? "Unfavorite" : "Favorite"}
        title={fav ? "Unfavorite" : "Favorite"}
      ><Icon name="heart" size={18} /></button>
      <button
        class="ibtn"
        class:active={player.state.shuffle}
        onclick={() => player.setShuffle(!player.state.shuffle)}
        aria-label="Shuffle"
        aria-pressed={player.state.shuffle}
        title="Shuffle the rest of the queue"
      ><Icon name="shuffle" size={18} /></button>
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
        title={player.state.repeat === "one"
          ? "Repeat one"
          : player.state.repeat === "all"
            ? "Repeat all"
            : "Repeat off"}
      ><Icon name={player.state.repeat === "one" ? "repeat-1" : "repeat"} size={18} /></button>
      <button
        class="ibtn"
        class:active={mini.queueOpen}
        onclick={() => mini.toggleQueue()}
        aria-label="Queue"
        title="Queue"
        aria-pressed={mini.queueOpen}
      ><Icon name="list-music" size={18} /></button>
      <button
        class="ibtn"
        onclick={() => void mini.exit()}
        aria-label="Restore full window"
        title="Restore full window"
      ><Icon name="maximize-2" size={18} /></button>
    </div>
    <VolumeSlider {player} iconSize={16} width="calc(5 * 30px + 4 * 4px)" />
  {:else}
    <div class="empty">
      <Icon name="music" size={40} />
      <p class="muted">Nothing playing</p>
    </div>
  {/if}

  {#if mini.queueOpen}
    <div class="sheet-backdrop" onclick={() => mini.toggleQueue()} aria-hidden="true"></div>
    <!-- Escape is routed by App.currentEscapeTarget ("mini-queue"); the trap
         restores focus to the Queue button on close. -->
    <div
      class="queue-sheet"
      role="dialog"
      aria-modal="true"
      aria-label="Queue"
      tabindex="-1"
      use:focusTrap
    >
      <QueueRail {player} />
    </div>
  {/if}
</div>

<style>
  .mini {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    height: 100%;
    /* The top gutter grows to the caption band under custom chrome. */
    padding: max(14px, var(--titlebar-h)) 14px 14px;
    position: relative;
    z-index: 1;
    overflow: auto;
  }
  /* The cover is fluid: it gives up width before the controls run out of room. */
  .art {
    flex: 0 0 auto;
    width: min(100%, 190px, 42vh);
    border-radius: var(--radius-card);
    overflow: hidden;
  }
  .meta {
    width: 100%;
    min-width: 0;
    text-align: center;
  }
  .title {
    display: block;
    font-family: var(--font-display);
    font-size: 16px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist {
    display: block;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }
  .secondary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 4px;
    width: 100%;
  }
  .empty {
    margin: auto;
    display: grid;
    place-items: center;
    gap: 8px;
    color: var(--text-muted);
  }
  button {
    display: inline-grid;
    place-items: center;
  }
  .ibtn {
    position: relative;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: var(--text-muted);
    transition:
      color var(--dur-micro),
      background-color var(--dur-micro),
      transform var(--dur-micro) var(--ease-in);
  }
  .ibtn:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .ibtn:active {
    transform: scale(0.92);
  }
  .ibtn.active {
    color: var(--accent);
  }
  .icon-heart.on {
    color: var(--accent);
  }
  .icon-heart.on :global(svg) {
    fill: currentColor;
  }
  .play {
    width: 46px;
    height: 46px;
    color: var(--accent);
  }
  .play:hover {
    color: var(--accent);
  }
  /* Queue sheet (MP-D5): slides up over the mini layout, never leaves mini. */
  .sheet-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    z-index: 10;
  }
  .queue-sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 11;
    max-height: 85vh;
    overflow: auto;
    background: var(--surface-1);
    backdrop-filter: blur(var(--glass-blur));
    border-top: 1px solid var(--line);
    border-radius: var(--radius-sheet) var(--radius-sheet) 0 0;
    animation: sheet-in var(--dur-view) var(--ease-in) both;
  }
  @keyframes sheet-in {
    from {
      transform: translateY(100%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .queue-sheet {
      animation-name: sheet-fade;
    }
    @keyframes sheet-fade {
      from {
        opacity: 0;
      }
    }
  }
</style>
