<script lang="ts">
  import { coverUrl } from "../artwork";
  import type { Player } from "../player.svelte";

  let { player }: { player: Player } = $props();

  // Two stacked layers: the new cover loads on the inactive one, then the
  // active flag flips and the opacity transition runs (spec §5.2 crossfade).
  let layers = $state<[string, string]>(["", ""]);
  let active = $state<0 | 1>(0);

  // Per-load token: latest-wins. A new track bumps the counter, so an
  // in-flight preload that resolves late is discarded rather than made
  // visible over the newer cover.
  let loadToken = 0;

  $effect(() => {
    const cover = player.state.current?.cover ?? null;
    const url = cover ? coverUrl(cover, 640) : "";
    if (url === layers[active]) return;
    const next = active === 0 ? 1 : 0;
    const token = (loadToken += 1);

    const apply = () => {
      if (token !== loadToken) return;
      layers[next] = url;
      active = next;
    };

    // No art → the static warm gradient room, applied immediately.
    if (!url) {
      apply();
      return;
    }

    // Preload the incoming image so `active` only flips once it is
    // decoded/loaded; otherwise a skip-storm fades the old layer to the
    // static gradient and pops the new image in un-faded (spec §5.2).
    const img = new Image();
    img.onload = apply;
    img.onerror = apply; // a failed decode falls back instead of getting stuck
    img.src = url;

    return () => {
      img.onload = null;
      img.onerror = null;
    };
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
    background: linear-gradient(160deg, var(--room-top), var(--bg)); /* no-art room (§5.2) */
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
    background: linear-gradient(180deg, var(--scrim-top), var(--scrim-bottom));
  }
</style>
