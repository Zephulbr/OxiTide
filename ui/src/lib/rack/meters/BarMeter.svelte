<script lang="ts">
  import { peakFraction } from "../ballistics";
  import type { LevelsSnapshot } from "../levels.svelte";

  /// VFD peak bars (DESIGN §5.9): −40…+3 dBFS, red past 0 dBFS, hold tick.
  let { snapshot, header = true }: { snapshot: LevelsSnapshot; header?: boolean } = $props();

  const TICKS = [-40, -30, -20, -10, -6, -3, 0, 3];
  const hold = $derived(Math.max(snapshot.holdDb[0], snapshot.holdDb[1]));
  const readout = $derived(`${hold > 0 ? "+" : ""}${Math.max(-40, hold).toFixed(1)} dB`);
</script>

<div class="bars">
  {#if header}
    <div class="row">
      <div class="rk-anno"><span class="on">PROGRAM</span><span class="on">HOLD</span></div>
      <div class="rk-anno"><span class="on">{readout}</span></div>
    </div>
  {/if}
  {#each [0, 1] as ch (ch)}
    <div class="dbar">
      <b>{ch === 0 ? "L" : "R"}</b>
      <div class="track" style:--f={peakFraction(snapshot.peakDb[ch])} style:--pk={peakFraction(snapshot.holdDb[ch])}>
        <i class="fill"></i><i class="pk"></i>
      </div>
    </div>
  {/each}
  <div class="scale">
    {#each TICKS as d (d)}
      <span style:left="{peakFraction(d) * 100}%">{d > 0 ? `+${d}` : d}</span>
    {/each}
  </div>
</div>

<style>
  .bars {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 6px;
    height: 100%;
  }
  .row {
    display: flex;
    justify-content: space-between;
  }
  .dbar {
    display: grid;
    grid-template-columns: 12px 1fr;
    gap: 8px;
    align-items: center;
  }
  b {
    font: 800 9.5px/1 var(--font-ui);
    text-shadow: 0 0 5px var(--rack-vfd);
  }
  .track {
    position: relative;
    height: 11px;
  }
  .track::before,
  .fill {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, var(--rack-vfd) 0 93%, var(--rack-meter-red) 93%);
    mask: repeating-linear-gradient(90deg, #000 0 4px, transparent 4px 6px);
  }
  .track::before {
    opacity: 0.1;
  }
  .fill {
    clip-path: inset(0 calc((1 - var(--f)) * 100%) 0 0);
    filter: drop-shadow(0 0 3px var(--rack-vfd));
  }
  .pk {
    position: absolute;
    top: -1px;
    bottom: -1px;
    width: 3px;
    left: calc(var(--pk) * 100% - 3px);
    background: var(--rack-vfd);
    box-shadow: 0 0 6px var(--rack-vfd);
  }
  .scale {
    position: relative;
    height: 8px;
    margin-left: 20px;
    font: 700 7.5px/1 var(--font-ui);
    opacity: 0.6;
  }
  .scale span {
    position: absolute;
    transform: translateX(-50%);
  }
  .scale span:first-child {
    transform: none;
  }
  .scale span:last-child {
    transform: translateX(-100%);
  }
</style>
