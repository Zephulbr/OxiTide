<script lang="ts">
  /// SVG 7-segment readout for digits and colons (DESIGN §4). Unlit segments
  /// stay faintly visible, like a real VFD. Decorative: the caller exposes the
  /// value as text elsewhere.
  let { value, height = 22 }: { value: string; height?: number } = $props();

  const SEG: Record<string, string> = {
    "0": "abcdef", "1": "bc", "2": "abged", "3": "abgcd", "4": "fgbc",
    "5": "afgcd", "6": "afgedc", "7": "abc", "8": "abcdefg", "9": "abcfgd", "-": "g", " ": "",
  };
  const POLY: Record<string, string> = {
    a: "3,1 17,1 15,4 5,4", b: "18,2 18,16 15,15 15,5", c: "18,20 18,34 15,31 15,21",
    d: "3,35 17,35 15,32 5,32", e: "2,20 2,34 5,31 5,21", f: "2,2 2,16 5,15 5,5",
    g: "3,18 5,16.5 15,16.5 17,18 15,19.5 5,19.5",
  };

  const glyphs = $derived.by(() => {
    let x = 2;
    return [...value].map((ch) => {
      const at = x;
      x += ch === ":" ? 8 : 22;
      return { ch, x: at };
    });
  });
  const width = $derived(glyphs.reduce((w, g) => w + (g.ch === ":" ? 8 : 22), 4));
</script>

<svg class="seg" viewBox="0 0 {width} 36" style:height="{height}px" aria-hidden="true">
  {#each glyphs as g, i (i)}
    {#if g.ch === ":"}
      <circle cx={g.x + 3.5} cy="12" r="1.7" class="on" />
      <circle cx={g.x + 2.1} cy="25" r="1.7" class="on" />
    {:else}
      {#each Object.entries(POLY) as [k, p] (k)}
        <polygon transform="translate({g.x} 0) skewX(-6)" points={p} class={(SEG[g.ch] ?? "").includes(k) ? "on" : "off"} />
      {/each}
    {/if}
  {/each}
</svg>

<style>
  .seg {
    display: block;
    filter: drop-shadow(0 0 3px var(--rack-vfd));
  }
  .on {
    fill: var(--rack-vfd);
  }
  .off {
    fill: var(--rack-vfd-ghost);
  }
</style>
