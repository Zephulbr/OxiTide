<script lang="ts">
  let {
    layers = 10,
    height = 112,
    tint = "",
    tintStrength = 55,
  }: {
    layers?: number;
    height?: number;
    tint?: string;
    tintStrength?: number;
  } = $props();

  const fade = (end: number) =>
    `linear-gradient(to bottom, black 0%, black ${Math.max(end - 35, 0)}%, transparent ${end}%)`;

  const stack = $derived(
    Array.from({ length: layers }, (_, i) => {
      const factor = (i / Math.max(layers - 1, 1)) ** 2;
      const end = 100 - i * (65 / layers);
      const filter = `blur(calc(var(--glass-blur) * ${factor.toFixed(3)}))`;
      const mask = fade(end);
      return `backdrop-filter:${filter};-webkit-backdrop-filter:${filter};mask-image:${mask};-webkit-mask-image:${mask};`;
    }),
  );
</script>

<div class="edge-blur" style:height="{height}px" aria-hidden="true">
  {#each stack as style}
    <div class="layer" {style}></div>
  {/each}
  {#if tint}
    <div
      class="layer"
      style="background:linear-gradient(to bottom,
        color-mix(in srgb, {tint} {tintStrength}%, transparent),
        color-mix(in srgb, {tint} 0%, transparent));"
    ></div>
  {/if}
</div>

<style>
  .edge-blur {
    position: absolute;
    top: 0;
    right: 0;
    left: 0;
    pointer-events: none;
  }
  .layer {
    position: absolute;
    inset: 0;
  }
</style>
