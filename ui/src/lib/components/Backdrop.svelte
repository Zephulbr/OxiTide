<script lang="ts">
  import { coverUrl } from "../artwork";
  import type { Player } from "../player.svelte";

  let { player }: { player: Player } = $props();

  // Two stacked layers: the new cover loads on the inactive one, then the
  // active flag flips and the opacity transition runs (spec §5.2 crossfade).
  let layers = $state<[string, string]>(["", ""]);
  let active = $state<0 | 1>(0);

  $effect(() => {
    const cover = player.state.current?.cover ?? null;
    const url = cover ? coverUrl(cover, 640) : "";
    if (url === layers[active]) return;
    const next = active === 0 ? 1 : 0;
    layers[next] = url;
    active = next;
  });
</script>

<div class="backdrop" aria-hidden="true">
  {#each [0, 1] as i}
    {#if layers[i]}
      <img class:active={active === i} src={layers[i]} alt="" draggable="false" />
    {/if}
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: linear-gradient(160deg, #1d1712, var(--bg)); /* no-art room (§5.2) */
    opacity: var(--flag-backdrop-art);
  }
  .backdrop img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(80px) saturate(1.15);
    transform: scale(1.25); /* hide the blurred edge falloff */
    opacity: 0;
    transition: opacity var(--dur-mood) linear;
  }
  .backdrop img.active { opacity: 1; }
  /* Structural floor (§5.3): the scrim never drops below 55%. */
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(23, 18, 14, 0.55), rgba(23, 18, 14, 0.78));
  }
</style>
