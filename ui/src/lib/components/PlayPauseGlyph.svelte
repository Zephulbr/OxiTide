<script lang="ts">
  let { playing, size = 24 }: { playing: boolean; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <path
    class="glyph"
    class:playing={playing}
    d={playing
      ? "M7.5 3.5 L9.5 3.5 L9.5 20.5 L7.5 20.5 Z M14.5 3.5 L16.5 3.5 L16.5 20.5 L14.5 20.5 Z"
      : "M7 3.5 L21.7224 11.95 L21.7224 12.05 L7 20.5 Z M21.7224 11.95 L21.7224 11.95 L21.7224 12.05 L21.7224 12.05 Z"}
  />
</svg>

<style>
  .glyph {
    fill: currentColor;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linejoin: round;
    d: path("M7 3.5 L21.7224 11.95 L21.7224 12.05 L7 20.5 Z M21.7224 11.95 L21.7224 11.95 L21.7224 12.05 L21.7224 12.05 Z");
    transition: d var(--dur-state) var(--ease-enter);
  }
  .glyph.playing {
    d: path("M7.5 3.5 L9.5 3.5 L9.5 20.5 L7.5 20.5 Z M14.5 3.5 L16.5 3.5 L16.5 20.5 L14.5 20.5 Z");
  }
</style>
