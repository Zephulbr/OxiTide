<script lang="ts">
  // The OxiTide mark: ink disc, teal rings, rust wave. Brand colours are fixed
  // on purpose (not theme tokens) so the mark looks the same in every mode.
  // Detail steps down with size, mirroring crates/oxitide-app/icons/svg/*.svg:
  // two rings, one ring, one ring with a bolder wave, wave only.
  let { size = 24 }: { size?: number } = $props();

  const INK = "#1C2825";
  const TEAL = "#4F8C82";
  const RUST = "#C4693E";

  const variant = $derived(
    size >= 56
      ? { rings: [50, 34], ring: 3, wave: "M12 82 C 38 58, 58 106, 86 82 C 106 64, 118 82, 132 68", stroke: 8 }
      : size >= 40
        ? { rings: [50, 34], ring: 4, wave: "M12 82 C 38 58, 58 106, 86 82 C 106 64, 118 82, 132 68", stroke: 10 }
        : size >= 32
          ? { rings: [48], ring: 6, wave: "M12 82 C 38 58, 58 106, 86 82 C 106 64, 118 82, 132 68", stroke: 13 }
          : size >= 22
            ? { rings: [48], ring: 8, wave: "M18 84 C 46 56, 64 108, 122 76", stroke: 16 }
            : { rings: [], ring: 0, wave: "M18 88 C 46 50, 64 116, 122 68", stroke: 24 },
  );
</script>

<svg
  class="logo"
  width={size}
  height={size}
  viewBox="0 0 1024 1024"
  aria-hidden="true"
  focusable="false"
>
  <circle cx="512" cy="512" r="504" fill={INK} />
  <g transform="translate(137 137) scale(5.357)" fill="none" stroke-linecap="round">
    {#each variant.rings as r (r)}
      <circle cx="70" cy="70" {r} stroke={TEAL} stroke-width={variant.ring} />
    {/each}
    <path d={variant.wave} stroke={RUST} stroke-width={variant.stroke} />
  </g>
</svg>

<style>
  .logo {
    flex: none;
    display: block;
  }
</style>
