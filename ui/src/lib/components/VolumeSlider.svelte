<script lang="ts">
  import type { Player } from "../player.svelte";
  import { shownVolume, volumeIcon } from "../player.svelte";
  import { FIXED_VOLUME_HINT as FIXED_HINT } from "../volume";
  import Icon from "./Icon.svelte";

  let {
    player,
    iconSize = 18,
    width = "146px",
  }: { player: Player; iconSize?: number; width?: string } = $props();

  /// Exclusive output plays at 100% and leaves the level to the device: the
  /// slider shows full and locks, with a lock in place of the mute button.
  const fixed = $derived(player.state.volume_fixed);
  const volume = $derived(shownVolume(player.state));
  const muted = $derived(volume === 0);
  const icon = $derived(fixed ? "lock" : volumeIcon(volume));
  const level = $derived(`${volume * 100}%`);</script>

<div class="volume" class:fixed style:width={width} title={fixed ? FIXED_HINT : undefined}>
  <button
    class="mute"
    class:on={muted}
    data-floor
    disabled={fixed}
    onclick={() => player.toggleMute()}
    aria-label={fixed ? FIXED_HINT : muted ? "Unmute" : "Mute"}
    title={fixed ? FIXED_HINT : muted ? "Unmute" : "Mute"}
    aria-pressed={fixed ? undefined : muted}
  ><Icon name={icon} size={iconSize} /></button>
  <div class="track" style:--level={level}>
    <input
      type="range" min="0" max="1" step="0.01" value={volume} data-perf-volume
      disabled={fixed}
      oninput={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value))}
      onchange={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value), true)}
      aria-label="Volume"
      aria-valuetext={fixed ? "100%, fixed in exclusive mode" : `${Math.round(volume * 100)}%`}
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
      transform var(--dur-micro) var(--ease-enter);
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
  .mute:disabled {
    cursor: default;
  }
  .mute:disabled:hover {
    background: none;
    color: var(--text-muted);
  }
  .mute:disabled:active {
    transform: none;
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
    transition: box-shadow var(--dur-state) var(--ease-enter);
  }
  .track:hover {
    box-shadow: 0 0 9px color-mix(in srgb, var(--accent) 80%, transparent);
  }
  /* Fixed: a full but muted fill, no glow, no pointer. */
  .fixed .track {
    background: var(--text-muted);
    opacity: 0.5;
  }
  .fixed .track:hover {
    box-shadow: none;
  }
  .fixed .track input {
    cursor: default;
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

  /* Held at 100% (ASIO): the level reads full, dimmed, and takes no input. */
  .fixed {
    opacity: 0.55;
  }
  .fixed .mute,
  .fixed .track input {
    cursor: not-allowed;
  }
  .fixed .mute:hover {
    background: none;
    color: var(--text-muted);
  }
  .fixed .track:hover {
    box-shadow: none;
  }
</style>
