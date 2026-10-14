<script lang="ts">
  import { art } from "../convert";

  /// The Atmosphere room behind every page: the current cover, heavily
  /// blurred and slowly drifting, under the shared scrim. Two layers
  /// crossfade on track change; the next image is preloaded first.
  let { cover }: { cover: string | null } = $props();

  let layers = $state<[string, string]>(["", ""]);
  let active = $state<0 | 1>(0);
  let token = 0;

  $effect(() => {
    const url = art(cover, 320);
    if (url === layers[active]) return;
    const next = active === 0 ? 1 : 0;
    const mine = ++token;
    const apply = () => {
      if (mine !== token) return;
      layers[next] = url;
      active = next;
    };
    if (!url) {
      apply();
      return;
    }
    const img = new Image();
    img.onload = apply;
    img.onerror = apply;
    img.src = url;
    return () => {
      img.onload = null;
      img.onerror = null;
    };
  });
</script>

<div class="backdrop" aria-hidden="true">
  {#each [0, 1] as i (i)}
    {#if layers[i]}
      <img class:active={active === i} src={layers[i]} alt="" draggable="false" />
    {/if}
  {/each}
  <div class="glow"></div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    pointer-events: none;
    background: linear-gradient(160deg, var(--room-top), var(--bg));
  }
  img {
    position: absolute;
    inset: -10%;
    width: 120%;
    height: 120%;
    object-fit: cover;
    filter: blur(56px) saturate(1.2);
    opacity: 0;
    transform: scale(1.15);
    transition: opacity var(--dur-mood) linear;
    animation: drift 38s var(--ease-in-out) infinite alternate;
  }
  img.active {
    opacity: 1;
  }
  /* A soft wash of the cover's accent at the top, then the shared scrim
     (structural floor: never below the Atmosphere scrim). */
  .glow {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(90% 55% at 50% 0%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 70%),
      linear-gradient(180deg, var(--scrim-top), var(--scrim-bottom));
  }
  @keyframes drift {
    from {
      transform: scale(1.15) translate3d(-3%, -2%, 0) rotate(-2deg);
    }
    to {
      transform: scale(1.25) translate3d(3%, 2%, 0) rotate(2deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    img {
      animation: none;
    }
  }
</style>
