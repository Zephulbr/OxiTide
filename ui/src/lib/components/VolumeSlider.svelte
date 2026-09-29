<script lang="ts">
  import type { Player } from "../player.svelte";
  import { volumeIcon } from "../player.svelte";
  import Icon from "./Icon.svelte";

  let {
    player,
    iconSize = 18,
    width = "146px",
  }: { player: Player; iconSize?: number; width?: string } = $props();

  const muted = $derived(player.state.volume === 0);
  const icon = $derived(volumeIcon(player.state.volume));
  const level = $derived(`${player.state.volume * 100}%`);
</script>

<div class="volume" style:width={width}>
  <button
    class="mute"
    class:on={muted}
    onclick={() => player.toggleMute()}
    aria-label={muted ? "Unmute" : "Mute"}
    title={muted ? "Unmute" : "Mute"}
    aria-pressed={muted}
  ><Icon name={icon} size={iconSize} /></button>
  <div class="track" style:--level={level}>
    <input
      type="range" min="0" max="1" step="0.01" value={player.state.volume}
      oninput={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value))}
      onchange={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value), true)}
      aria-label="Volume"
    />
  </div>
</div>

<style>
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .mute {
    flex: 0 0 auto;
    display: inline-grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: var(--text-muted);
    transition:
      color var(--dur-micro),
      background-color var(--dur-micro),
      transform var(--dur-micro) var(--ease-in);
  }
  .mute:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .mute:active {
    transform: scale(0.92);
  }
  .mute.on {
    color: var(--accent);
  }

  /* Pill track with a thumbless level fill: accent up to the value, `--line`
     after — the same track color as the seek bar. Fixed 8px; on hover it
     only picks up the play button's accent glow. */
  .track {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 8px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--accent) var(--level, 100%),
      var(--line) var(--level, 100%)
    );
    transition: box-shadow var(--dur-state) var(--ease-in);
  }
  .track:hover {
    box-shadow: 0 0 9px color-mix(in srgb, var(--accent) 80%, transparent);
  }

  /* The input is an invisible hit area over the track; the thumb stays hidden
     (keyboard focus shows the ring instead). */
  .track input {
    position: absolute;
    inset: -8px 0;
    width: 100%;
    height: auto;
    margin: 0;
    -webkit-appearance: none;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }
  .track input::-webkit-slider-runnable-track {
    background: transparent;
  }
  .track input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 0;
    height: 0;
  }
  .track input:focus-visible {
    outline: none;
  }
  .track:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
  }
</style>
