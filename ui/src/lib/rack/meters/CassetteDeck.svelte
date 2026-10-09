<script lang="ts">
  import { appearance } from "../../appearance.svelte";
  import { reelRadii } from "../deck";

  /// Cassette deck (DESIGN §5.9 slot): a close-up of a cassette through the
  /// deck's door glass, drawn to the slot's wide shape — the label strip with
  /// the title along the top, the shell's wide clear window showing both
  /// glossy tape packs, the turning hubs, screws along the bottom. The packs
  /// wind left → right as the track plays (a physical progress bar); hubs turn
  /// only while playing, the emptier reel faster.
  let {
    progress,
    playing,
    title,
    artist,
    elapsedMs,
  }: { progress: number; playing: boolean; title: string; artist: string; elapsedMs: number } = $props();

  const HUB = 17;
  const FULL = 80;
  const LX = 210;
  const RX = 390;
  const CY = 56;
  const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
  const radii = $derived(reelRadii(progress, HUB, FULL));
  const counter = $derived(String(Math.floor(elapsedMs / 1000) % 1000).padStart(3, "0"));
  const moving = $derived(playing && !appearance.reducedMotion);

  let angles = $state([0, 0]);
  $effect(() => {
    if (!moving) return;
    let raf = 0;
    let last: number | null = null;
    const step = (t: number) => {
      const dt = last === null ? 0 : Math.min(0.1, (t - last) / 1000);
      last = t;
      // Constant tape speed: hub turn rate is inversely proportional to the
      // pack radius (degrees per second at the hub).
      angles = [(angles[0] + (dt * 4200) / radii[0]) % 360, (angles[1] + (dt * 4200) / radii[1]) % 360];
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  const TEETH = [0, 60, 120, 180, 240, 300];
  /// Tape wound on a pack shows faint winding rings.
  const rings = (r: number) => Array.from({ length: Math.max(0, Math.floor((r - HUB) / 7)) }, (_, i) => HUB + 4 + i * 7);
  const WINDOW = "M128 24 Q128 18 134 18 H466 Q472 18 472 24 V86 Q472 92 466 92 H134 Q128 92 128 86Z";
</script>

<div class="deck">
  <div class="door">
    <svg viewBox="0 0 600 110" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <defs>
        <radialGradient id="rk-pack" cx="50%" cy="50%" r="50%">
          <stop offset="0.2" stop-color="#3a2513" />
          <stop offset="0.7" stop-color="#6b4626" />
          <stop offset="0.92" stop-color="#8f6034" />
          <stop offset="1" stop-color="#3a2513" />
        </radialGradient>
        <linearGradient id="rk-plastic" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#2a2622" />
          <stop offset="1" stop-color="#14110f" />
        </linearGradient>
        <clipPath id="rk-window"><path d={WINDOW} /></clipPath>
      </defs>

      <!-- Shell body. -->
      <rect x="0" y="0" width="600" height="110" class="shell" />

      <!-- Clear window: the well behind, the packs, the hubs. -->
      <g clip-path="url(#rk-window)">
        <rect x="128" y="18" width="344" height="74" fill="url(#rk-plastic)" />
        {#each [LX, RX] as cx, i (cx)}
          <circle cx={cx} cy={CY} r={radii[i]} fill="url(#rk-pack)" />
          {#each rings(radii[i]) as r (r)}
            <circle cx={cx} cy={CY} r={r} class="ring" />
          {/each}
          <g transform="rotate({angles[i]} {cx} {CY})">
            <circle cx={cx} cy={CY} r={HUB} class="hub" />
            <circle cx={cx} cy={CY} r="10" class="bore" />
            {#each TEETH as a (a)}
              <rect x={cx - 1.8} y={CY - 10} width="3.6" height="5" class="hub" transform="rotate({a} {cx} {CY})" />
            {/each}
          </g>
        {/each}
        <!-- Window plastic sheen. -->
        <rect x="128" y="18" width="344" height="74" class="sheen" />
      </g>
      <path d={WINDOW} class="rim" />

      <!-- Label strip along the top edge. -->
      <rect x="18" y="-4" width="564" height="18" rx="2" class="label" />
      <text x="30" y="10" class="side">A</text>
      <text x="48" y="10" class="title">{clip(title, 70)}{artist ? ` — ${clip(artist, 40)}` : ""}</text>
      <text x="570" y="10" class="type" text-anchor="end">C-90</text>
      <!-- Printed shell marks and screws. -->
      <text x="40" y="58" class="print">NR</text>
      <text x="560" y="58" class="print" text-anchor="end">HIGH (CrO2)</text>
      {#each [24, 300, 576] as x (x)}
        <circle cx={x} cy="101" r="3.4" class="screw" />
        <line x1={x - 2.2} y1="101" x2={x + 2.2} y2="101" class="slot" />
      {/each}
    </svg>
    <!-- The deck's door glass in front. -->
    <div class="glass"></div>
    <div class="counter" aria-label={`Tape counter ${counter}`}>
      {#each counter.split("") as digit, i (i)}<span>{digit}</span>{/each}
    </div>
    <span class="rk-led playlamp" class:on={playing}>Play</span>
  </div>
</div>

<style>
  .deck {
    height: 100%;
  }
  .door {
    position: relative;
    height: 100%;
    overflow: hidden;
    border-radius: 5px;
    /* The shell's colour: on a wider slot the cassette is centred and the
       extra width reads as more shell, never as a crop of the label. */
    background: #1e1c1a;
    box-shadow: 0 0 0 1px #000, 0 1px 0 1px rgba(255, 255, 255, 0.07), inset 0 3px 8px rgba(0, 0, 0, 0.95);
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  /* Door glass: a reflection and a light smoke, darker at the edges. */
  .glass {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(166deg, rgba(255, 255, 255, 0.08) 0, rgba(255, 255, 255, 0.02) 30%, transparent 30.3%),
      rgba(10, 8, 6, 0.12);
    box-shadow: inset 0 0 26px rgba(0, 0, 0, 0.8);
  }
  .shell {
    fill: #1e1c1a;
  }
  .ring {
    fill: none;
    stroke: rgba(0, 0, 0, 0.22);
    stroke-width: 0.6;
  }
  .hub {
    fill: #e2dcd2;
  }
  .bore {
    fill: #0c0b0a;
  }
  .sheen {
    fill: url(#rk-plastic);
    opacity: 0.18;
  }
  .rim {
    fill: none;
    stroke: #3a3632;
    stroke-width: 1.6;
  }
  .label {
    fill: #dcd3c4;
  }
  .side {
    font: 800 10px var(--font-ui);
    fill: #8a2f1c;
  }
  .title {
    font: 700 8px var(--font-ui);
    fill: #2a2119;
  }
  .type {
    font: 800 8px var(--font-ui);
    fill: #8a2f1c;
  }
  .print {
    font: 700 8px var(--font-ui);
    letter-spacing: 0.08em;
    fill: #5d5750;
  }
  .screw {
    fill: #2c2926;
    stroke: #0d0c0b;
  }
  .slot {
    stroke: #0d0c0b;
    stroke-width: 1;
  }
  /* The deck's own counter and lamp, on the bezel corners. */
  .counter {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 3px;
    background: #000;
    box-shadow: var(--rack-tray-shadow);
  }
  .counter span {
    width: 11px;
    padding: 2px 0;
    border-radius: 2px;
    text-align: center;
    font: 700 10px/1 var(--font-ui);
    font-variant-numeric: tabular-nums;
    color: #e8e2d8;
    background: linear-gradient(180deg, #2a2724, #111 45%, #111 55%, #2a2724);
  }
  .playlamp {
    position: absolute;
    left: 10px;
    bottom: 10px;
  }
</style>
