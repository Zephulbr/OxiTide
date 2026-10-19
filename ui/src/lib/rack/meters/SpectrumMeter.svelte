<script lang="ts">
  import { poolBands } from "../ballistics";
  import type { LevelsSnapshot } from "../levels.svelte";

  /// VFD spectrum (DESIGN §5.9): `count` bars of stacked segments with falling
  /// caps; 20 in the amp, 28 in the expanded deck.
  let { snapshot, count = 20, scale = true }: { snapshot: LevelsSnapshot; count?: number; scale?: boolean } = $props();

  const bars = $derived(poolBands(snapshot.bands, count));
  const caps = $derived(poolBands(snapshot.caps, count));
</script>

<div class="spec">
  <div class="bars" style:grid-template-columns="repeat({count}, 1fr)">
    {#each bars as h, i (i)}
      <div class="bar" style:--h="{h * 100}%" style:--cap="{(caps[i] ?? 0) * 100}%"><i></i></div>
    {/each}
  </div>
  {#if scale}
    <div class="scale"><span>63</span><span>250</span><span>1k</span><span>4k</span><span>16k</span></div>
  {/if}
</div>

<style>
  .spec {
    display: flex;
    flex-direction: column;
    gap: 5px;
    height: 100%;
  }
  /* One glow for the whole group (Track 13): a filter per bar and a shadow
     per cap were re-rendered for every bar on every meter frame. */
  .bars {
    flex: 1;
    display: grid;
    gap: 3px;
    min-height: 0;
    filter: drop-shadow(0 0 3px var(--rack-vfd));
  }
  .bar {
    position: relative;
  }
  .bar::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--h);
    background: repeating-linear-gradient(0deg, var(--rack-vfd) 0 3px, transparent 3px 5px);
  }
  i {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    bottom: var(--cap);
    background: var(--rack-vfd);
  }
  .scale {
    display: flex;
    justify-content: space-between;
    font: 700 7.5px/1 var(--font-ui);
    opacity: 0.6;
  }
</style>
