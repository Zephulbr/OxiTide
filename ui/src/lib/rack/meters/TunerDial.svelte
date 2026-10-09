<script lang="ts">
  import { appearance } from "../../appearance.svelte";
  import { clock, dialTicks } from "../deck";

  /// FM-tuner dial (DESIGN §5.9 slot): a backlit glass scale whose red pointer
  /// travels the track's timeline. The FM band printed above is decorative
  /// (the pointer's "frequency" is its position); the minute scale below is
  /// the real one. Almost static: the pointer glides at the track's pace.
  let {
    progress,
    durationMs,
    positionMs,
    playing,
    title,
    artist,
  }: { progress: number; durationMs: number; positionMs: number; playing: boolean; title: string; artist: string } = $props();

  const FM = [88, 90, 92, 94, 96, 98, 100, 102, 104, 106, 108];
  const p = $derived(Math.min(1, Math.max(0, Number.isFinite(progress) ? progress : 0)));
  const freq = $derived((87.5 + p * 20.5).toFixed(1));
  const ticks = $derived(dialTicks(durationMs));
</script>

<div class="tuner">
  <div class="dial" class:glide={!appearance.reducedMotion}>
    <div class="fm" aria-hidden="true">
      {#each FM as f, i (f)}
        <span style:left="{(i / (FM.length - 1)) * 100}%">{f}</span>
      {/each}
      <em>MHz</em>
    </div>
    <div class="scale" aria-hidden="true"></div>
    <div class="min" aria-hidden="true">
      {#each ticks as t (t.label)}
        <span style:left="{t.at * 100}%">{t.label}</span>
      {/each}
      <em>min</em>
    </div>
    <div class="pointer" style:left="{p * 100}%" aria-hidden="true"></div>
  </div>
  <div class="bar">
    <span class="rk-led" class:on={playing}>Stereo</span>
    <span class="rk-led" class:on={durationMs > 0}>Tuned</span>
    <div class="rk-vfd station">
      <span class="freq">{freq}</span><span class="mhz">MHz</span>
      <span class="name">{title}{artist ? ` · ${artist}` : ""}</span>
      <span class="time">{clock(positionMs)} / {clock(durationMs)}</span>
    </div>
  </div>
</div>

<style>
  .tuner {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
  }
  /* The dial glass, lit from behind by a warm lamp (the lamp colour). */
  .dial {
    position: relative;
    flex: 1;
    min-height: 0;
    border-radius: 4px;
    overflow: hidden;
    padding: 0 22px;
    background:
      linear-gradient(166deg, rgba(255, 255, 255, 0.06) 0, rgba(255, 255, 255, 0.015) 34%, transparent 34.3%),
      radial-gradient(120% 140% at 50% 50%, color-mix(in srgb, var(--rack-lamp) 22%, #0b0806), #070504 75%);
    box-shadow: 0 0 0 1px #000, 0 1px 0 1px rgba(255, 255, 255, 0.07), inset 0 3px 10px rgba(0, 0, 0, 0.9);
  }
  .fm,
  .min {
    position: absolute;
    left: 22px;
    right: 22px;
    font: 700 9px/1 var(--font-ui);
    letter-spacing: 0.04em;
    color: color-mix(in srgb, var(--rack-lamp) 70%, #fff);
  }
  .fm {
    top: 16%;
  }
  .min {
    bottom: 16%;
    opacity: 0.75;
  }
  .fm span,
  .min span {
    position: absolute;
    transform: translateX(-50%);
  }
  .fm em,
  .min em {
    position: absolute;
    right: -20px;
    font-style: normal;
    font-size: 7.5px;
    opacity: 0.7;
  }
  /* Tick rail across the middle. */
  .scale {
    position: absolute;
    left: 22px;
    right: 22px;
    top: 50%;
    height: 12px;
    transform: translateY(-50%);
    background:
      linear-gradient(color-mix(in srgb, var(--rack-lamp) 55%, transparent), color-mix(in srgb, var(--rack-lamp) 55%, transparent)) 0 50% / 100% 1px no-repeat,
      repeating-linear-gradient(90deg, color-mix(in srgb, var(--rack-lamp) 45%, transparent) 0 1px, transparent 1px 2%);
  }
  .pointer {
    position: absolute;
    top: 8%;
    bottom: 8%;
    width: 2px;
    margin-left: 21px;
    border-radius: 1px;
    background: #ff3b24;
    box-shadow: 0 0 6px rgba(255, 59, 36, 0.8);
  }
  .glide .pointer {
    transition: left 600ms linear;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .station {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 6px 10px;
    font: 700 11px/1 var(--font-ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    text-shadow: 0 0 6px var(--rack-vfd);
  }
  .freq {
    font-size: 14px;
    font-variant-numeric: tabular-nums;
  }
  .mhz {
    font-size: 8px;
    opacity: 0.7;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    opacity: 0.85;
  }
  .time {
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }
</style>
