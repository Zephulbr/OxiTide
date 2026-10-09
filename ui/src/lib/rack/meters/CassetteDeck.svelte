<script lang="ts">
  import { appearance } from "../../appearance.svelte";
  import { reelRadii } from "../deck";

  /// Cassette deck (DESIGN §5.9 slot), after a classic deck's door: a
  /// recessed, rounded window of smoky glass through which you see the
  /// cassette's clear window — translucent hubs turning, the tape packs behind
  /// them winding left → right as the track plays — with the deck's features
  /// printed underneath. Hubs turn only while playing, the emptier reel faster.
  let { progress, playing, elapsedMs }: { progress: number; playing: boolean; elapsedMs: number } = $props();

  const HUB = 14;
  const FULL = 50;
  const LX = 205;
  const RX = 395;
  const CY = 48;
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
      angles = [(angles[0] + (dt * 3000) / radii[0]) % 360, (angles[1] + (dt * 3000) / radii[1]) % 360];
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  const SPOKES = [0, 60, 120, 180, 240, 300];
  /// The deck door's window: a stadium (fully rounded ends).
  const DOOR = "M68 12 H532 A36 36 0 0 1 532 84 H68 A36 36 0 0 1 68 12 Z";
</script>

<div class="deck">
  <svg viewBox="0 0 600 112" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
    <defs>
      <linearGradient id="rk-smoke" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#3b3835" />
        <stop offset="0.5" stop-color="#2a2725" />
        <stop offset="1" stop-color="#1d1b19" />
      </linearGradient>
      <linearGradient id="rk-clear" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#6a6560" stop-opacity="0.55" />
        <stop offset="1" stop-color="#3a3632" stop-opacity="0.55" />
      </linearGradient>
      <linearGradient id="rk-glare" x1="0" y1="0" x2="0.35" y2="1">
        <stop offset="0" stop-color="#fff" stop-opacity="0.13" />
        <stop offset="0.42" stop-color="#fff" stop-opacity="0.03" />
        <stop offset="0.43" stop-color="#fff" stop-opacity="0" />
      </linearGradient>
      <clipPath id="rk-door"><path d={DOOR} /></clipPath>
    </defs>

    <!-- Door bezel: a raised ring, then the recessed window. -->
    <path d="M66 8 H534 A40 40 0 0 1 534 88 H66 A40 40 0 0 1 66 8 Z" class="bezel" />
    <path d={DOOR} fill="url(#rk-smoke)" />

    <g clip-path="url(#rk-door)">
      <!-- Cassette shell face behind the glass, and its clear window. -->
      <rect x="0" y="0" width="600" height="112" class="shell" />
      <rect x="120" y="22" width="360" height="52" rx="5" fill="url(#rk-clear)" class="clear" />
      <!-- Tape packs seen through the clear plastic. -->
      {#each [LX, RX] as cx, i (cx)}
        <circle cx={cx} cy={CY} r={radii[i]} class="pack" />
      {/each}
      <rect x="120" y="22" width="360" height="52" rx="5" class="clearfront" />
      <!-- Centre guide and its small sight hole. -->
      <rect x="276" y="36" width="48" height="24" rx="3" class="guide" />
      <circle cx="300" cy="33" r="4" class="guide" />
      <!-- Translucent hubs, turning. -->
      {#each [LX, RX] as cx, i (cx)}
        <circle cx={cx} cy={CY} r="20" class="hubring" />
        <g transform="rotate({angles[i]} {cx} {CY})">
          <circle cx={cx} cy={CY} r={HUB} class="hub" />
          {#each SPOKES as a (a)}
            <rect x={cx - 2} y={CY - HUB + 1} width="4" height="7" rx="1" class="spoke" transform="rotate({a} {cx} {CY})" />
          {/each}
          <circle cx={cx} cy={CY} r="5" class="bore" />
        </g>
      {/each}
      <!-- Glass reflection and the recess shadow. -->
      <rect x="0" y="0" width="600" height="112" class="reflect" />
    </g>
    <path d={DOOR} class="recess" />

    <text x="300" y="104" class="print" text-anchor="middle">AUTO REVERSE · STEREO · HI-FI · DOLBY B NR</text>
  </svg>
  <span class="rk-led playlamp" class:on={playing}>Play</span>
  <div class="counter" aria-label={`Tape counter ${counter}`}>
    {#each counter.split("") as digit, i (i)}<span>{digit}</span>{/each}
  </div>
</div>

<style>
  .deck {
    position: relative;
    height: 100%;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .bezel {
    fill: #121110;
    stroke: #3a3632;
    stroke-width: 1.2;
  }
  .shell {
    fill: #24211f;
  }
  .clear {
    stroke: rgba(255, 255, 255, 0.12);
    stroke-width: 1;
  }
  .pack {
    fill: rgba(28, 16, 8, 0.78);
    stroke: rgba(120, 80, 45, 0.45);
    stroke-width: 1;
  }
  .clearfront {
    fill: rgba(200, 195, 185, 0.06);
  }
  .guide {
    fill: rgba(255, 255, 255, 0.05);
    stroke: rgba(255, 255, 255, 0.14);
    stroke-width: 0.8;
  }
  .hubring {
    fill: none;
    stroke: rgba(220, 214, 204, 0.28);
    stroke-width: 1.5;
  }
  .hub {
    fill: rgba(215, 210, 200, 0.32);
    stroke: rgba(240, 236, 228, 0.55);
    stroke-width: 1;
  }
  .spoke {
    fill: rgba(240, 236, 228, 0.7);
  }
  .bore {
    fill: #141210;
    stroke: rgba(240, 236, 228, 0.4);
    stroke-width: 0.8;
  }
  .reflect {
    fill: url(#rk-glare);
  }
  /* Recessed window edge: dark inner lip, light lower catch. */
  .recess {
    fill: none;
    stroke: #000;
    stroke-width: 2.5;
    filter: drop-shadow(0 1px 0 rgba(255, 255, 255, 0.08));
  }
  .print {
    font: 700 7.5px var(--font-ui);
    letter-spacing: 0.14em;
    fill: var(--rack-silk);
  }
  .playlamp {
    position: absolute;
    left: 8px;
    bottom: 4px;
  }
  .counter {
    position: absolute;
    right: 8px;
    bottom: 2px;
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
</style>
