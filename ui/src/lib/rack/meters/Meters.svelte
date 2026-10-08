<script lang="ts">
  import type { MeterStyle } from "../../api";
  import { levels } from "../levels.svelte";
  import BarMeter from "./BarMeter.svelte";
  import LedMeter from "./LedMeter.svelte";
  import NeedleMeter from "./NeedleMeter.svelte";
  import SpectrumMeter from "./SpectrumMeter.svelte";

  /// The amp's meter slot: one of four styles (RK-D6), all fed by the shared
  /// level feed, which runs only while this is mounted and visible.
  let { style }: { style: MeterStyle } = $props();

  $effect(() => levels.use());
  const snap = $derived(levels.snapshot);
</script>

<div class="meters" aria-hidden="true">
  {#if style === "needles"}
    <div class="pair">
      <NeedleMeter channel="L" position={snap.needle[0]} />
      <NeedleMeter channel="R" position={snap.needle[1]} />
    </div>
  {:else if style === "bars"}
    <div class="rk-vfd fill"><BarMeter snapshot={snap} /></div>
  {:else if style === "led"}
    <LedMeter snapshot={snap} />
  {:else}
    <div class="rk-vfd fill"><SpectrumMeter snapshot={snap} /></div>
  {/if}
</div>

<style>
  .meters {
    min-width: 0;
    height: 96px;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    height: 100%;
    align-items: center;
  }
  .fill {
    height: 100%;
    box-sizing: border-box;
    padding: 9px 12px 7px;
  }
</style>
