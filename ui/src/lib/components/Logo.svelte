<script lang="ts">
  // The OxiTide mark: ink disc edged by a ring, wave clipped to the disc.
  // Colours follow --accent (dynamic colour / Rack lamp): ring = accent, wave =
  // accent hue-rotated -130deg (teal -> rust in the brand palette), disc = a
  // near-black tint of the accent. Browsers without relative colour syntax keep
  // the static brand colours. Detail steps down with size, mirroring
  // crates/oxitide-app/icons/svg/*.svg, but keeps both rings down to 22px
  // (sidebar/Rack header sizes); below that, one ring and a bolder wave.
  let { size = 24 }: { size?: number } = $props();

  const WAVE = "M12 82 C 38 58, 58 106, 86 82 C 106 64, 118 82, 132 68";
  const id = $props.id();

  const variant = $derived(
    size >= 56
      ? { ring: 3, inner: 3, wave: WAVE, stroke: 8 }
      : size >= 40
        ? { ring: 4, inner: 4, wave: WAVE, stroke: 10 }
        : size >= 22
          ? { ring: 6, inner: 5, wave: WAVE, stroke: 13 }
          : size >= 18
            ? { ring: 8, inner: 0, wave: "M18 84 C 46 56, 64 108, 122 76", stroke: 16 }
            : { ring: 10, inner: 0, wave: "M18 88 C 46 50, 64 116, 122 68", stroke: 24 },
  );
  // The ring's outer edge is the edge of the icon.
  const scale = $derived(504 / (50 + variant.ring / 2));
</script>

<svg
  class="logo"
  width={size}
  height={size}
  viewBox="0 0 1024 1024"
  aria-hidden="true"
  focusable="false"
>
  <defs><clipPath id="{id}-disc"><circle cx="70" cy="70" r="50" /></clipPath></defs>
  <g transform="translate(512 512) scale({scale}) translate(-70 -70)" fill="none" stroke-linecap="round">
    <circle class="ink" cx="70" cy="70" r="50" />
    {#if variant.inner}
      <circle class="ring" cx="70" cy="70" r="34" stroke-width={variant.inner} />
    {/if}
    <path class="wave" d={variant.wave} stroke-width={variant.stroke} clip-path="url(#{id}-disc)" />
    <circle class="ring" cx="70" cy="70" r="50" stroke-width={variant.ring} />
  </g>
</svg>

<style>
  .logo {
    flex: none;
    display: block;
  }
  .ink {
    fill: #1c2825;
  }
  .ring {
    stroke: #4f8c82;
  }
  .wave {
    stroke: #c4693e;
  }
  @supports (color: oklch(from red l c h)) {
    .ink {
      fill: oklch(from var(--accent) 0.26 calc(c * 0.2) h);
    }
    .ring {
      stroke: var(--accent);
    }
    .wave {
      stroke: oklch(from var(--accent) l c calc(h - 130));
    }
  }
</style>
