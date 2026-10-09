<script lang="ts">
  import { appearance } from "../../appearance.svelte";
  import { reelRadii } from "../deck";

  /// Cassette deck (DESIGN §5.9 slot): a tape behind a smoked window. The
  /// packs wind from the left reel to the right as the track plays (a calm,
  /// physical progress bar); hubs turn only while playing, the emptier reel
  /// faster, like real tape. A three-digit counter runs with the seconds.
  let {
    progress,
    playing,
    title,
    artist,
    elapsedMs,
  }: { progress: number; playing: boolean; title: string; artist: string; elapsedMs: number } = $props();

  const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
  const radii = $derived(reelRadii(progress));
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
      // Constant tape speed: angular speed is inversely proportional to the
      // pack radius (degrees per second at the hub).
      angles = [
        (angles[0] + (dt * 2600) / radii[0]) % 360,
        (angles[1] + (dt * 2600) / radii[1]) % 360,
      ];
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  const TEETH = [0, 60, 120, 180, 240, 300];
</script>

<div class="deck">
  <div class="rk-win window">
    <svg viewBox="0 0 300 118" aria-hidden="true">
      <!-- Shell -->
      <rect x="6" y="4" width="288" height="110" rx="8" class="shell" />
      <rect x="22" y="12" width="256" height="26" rx="3" class="label" />
      <text x="34" y="29" class="side">A</text>
      <text x="52" y="24" class="title">{clip(title, 44)}</text>
      <text x="52" y="33" class="artist">{clip(artist, 56)}</text>
      <!-- Tape path to the head gap -->
      <path d="M {94 - radii[0] * 0.2} {74 + radii[0]} L 132 108 L 168 108 L {206 + radii[1] * 0.2} {74 + radii[1]}" class="tape" />
      <!-- Reel windows -->
      {#each [94, 206] as cx, i (cx)}
        <circle cx={cx} cy="74" r="38" class="hole" />
        <circle cx={cx} cy="74" r={radii[i]} class="pack" />
        <g transform="rotate({angles[i]} {cx} 74)">
          <circle cx={cx} cy="74" r="11" class="hub" />
          {#each TEETH as a (a)}
            <rect x={cx - 1.5} y="63" width="3" height="5" class="tooth" transform="rotate({a} {cx} 74)" />
          {/each}
          <circle cx={cx} cy="74" r="4.5" class="spindle" />
        </g>
      {/each}
      <rect x="128" y="100" width="44" height="12" rx="2" class="gap" />
    </svg>
  </div>
  <div class="bar">
    <span class="rk-led" class:on={playing}>Play</span>
    <span class="rk-silk">Type II · CrO2</span>
    <span class="counter" aria-label={`Tape counter ${counter}`}>
      {#each counter.split("") as digit, i (i)}<span>{digit}</span>{/each}
    </span>
  </div>
</div>

<style>
  .deck {
    display: flex;
    flex-direction: column;
    gap: 6px;
    height: 100%;
  }
  .window {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    padding: 4px 10px;
  }
  svg {
    height: 100%;
    max-width: 100%;
  }
  .shell {
    fill: #141210;
    stroke: #2a2724;
    stroke-width: 1;
  }
  .label {
    fill: #d9d0c2;
    opacity: 0.88;
  }
  .side {
    font: 800 12px var(--font-ui);
    fill: #8a2f1c;
  }
  .title {
    font: 700 8px var(--font-ui);
    fill: #241d16;
  }
  .artist {
    font: 500 6.5px var(--font-ui);
    fill: #5a4f43;
  }
  .hole {
    fill: #080706;
    stroke: #2a2724;
  }
  .pack {
    fill: #3a2414;
    stroke: #1c120a;
    stroke-width: 0.6;
  }
  .hub {
    fill: #d8d2c8;
  }
  .tooth {
    fill: #080706;
  }
  .spindle {
    fill: #2a2724;
  }
  .tape {
    fill: none;
    stroke: #3a2414;
    stroke-width: 1.2;
  }
  .gap {
    fill: #080706;
    stroke: #2a2724;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 0 4px;
  }
  .counter {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 3px;
    background: #000;
    box-shadow: var(--rack-tray-shadow);
  }
  .counter span {
    width: 12px;
    padding: 2px 0;
    border-radius: 2px;
    text-align: center;
    font: 700 11px/1 var(--font-ui);
    font-variant-numeric: tabular-nums;
    color: #e8e2d8;
    background: linear-gradient(180deg, #2a2724, #111 45%, #111 55%, #2a2724);
  }
</style>
