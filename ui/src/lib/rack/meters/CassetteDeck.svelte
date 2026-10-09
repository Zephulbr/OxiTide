<script lang="ts">
  import { appearance } from "../../appearance.svelte";
  import { reelRadii } from "../deck";

  /// Cassette deck (DESIGN §5.9 slot): a close-up through the deck's smoked
  /// door window — only the middle of the cassette is in view, filling the
  /// unit: the shell's clear window, both tape packs, the hubs. The packs wind
  /// left → right as the track plays (a physical progress bar); hubs turn only
  /// while playing, the emptier reel faster. The label edge and the shell's
  /// bottom run off the window. A three-digit counter runs with the seconds.
  let {
    progress,
    playing,
    title,
    artist,
    elapsedMs,
  }: { progress: number; playing: boolean; title: string; artist: string; elapsedMs: number } = $props();

  const HUB = 23;
  const FULL = 74;
  const LX = 112;
  const RX = 288;
  const CY = 66;
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
      angles = [(angles[0] + (dt * 5200) / radii[0]) % 360, (angles[1] + (dt * 5200) / radii[1]) % 360];
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  const TEETH = [0, 60, 120, 180, 240, 300];
  /// The shell, with its clear centre window and both hub holes cut out.
  const SHELL =
    "M0 0H400V132H0Z " +
    "M76 40 Q76 34 82 34 H318 Q324 34 324 40 V92 Q324 98 318 98 H82 Q76 98 76 92Z " +
    `M${LX - 26} ${CY}a26 26 0 1 0 52 0a26 26 0 1 0 -52 0Z ` +
    `M${RX - 26} ${CY}a26 26 0 1 0 52 0a26 26 0 1 0 -52 0Z`;
</script>

<div class="deck">
  <div class="door">
    <svg viewBox="0 0 400 132" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
      <defs>
        <radialGradient id="rk-pack" cx="50%" cy="50%" r="50%">
          <stop offset="0.3" stop-color="#2a1a0e" />
          <stop offset="0.92" stop-color="#46301c" />
          <stop offset="1" stop-color="#1c120a" />
        </radialGradient>
      </defs>
      <!-- Inside the cassette: the dark well, then the tape packs. -->
      <rect x="0" y="0" width="400" height="132" fill="#060505" />
      <circle cx={LX} cy={CY} r={radii[0]} fill="url(#rk-pack)" />
      <circle cx={RX} cy={CY} r={radii[1]} fill="url(#rk-pack)" />
      <!-- The shell over it, with the window and hub holes cut out. -->
      <path d={SHELL} fill-rule="evenodd" class="shell" />
      <path d="M76 40 Q76 34 82 34 H318 Q324 34 324 40 V92 Q324 98 318 98 H82 Q76 98 76 92Z" class="windowrim" />
      <!-- Label edge at the top and the shell's lower lip at the bottom. -->
      <rect x="20" y="-6" width="360" height="26" rx="3" class="label" />
      <text x="34" y="13" class="side">A</text>
      <text x="52" y="9" class="title">{clip(title, 52)}</text>
      <text x="52" y="17" class="artist">{clip(artist, 64)}</text>
      <rect x="150" y="112" width="100" height="20" rx="2" class="lip" />
      <!-- Hubs, turning. -->
      {#each [LX, RX] as cx, i (cx)}
        <g transform="rotate({angles[i]} {cx} {CY})">
          <circle cx={cx} cy={CY} r="19" class="hub" />
          {#each TEETH as a (a)}
            <rect x={cx - 2.5} y={CY - 19} width="5" height="8" class="tooth" transform="rotate({a} {cx} {CY})" />
          {/each}
          <circle cx={cx} cy={CY} r="7" class="spindle" />
        </g>
      {/each}
    </svg>
    <!-- The deck's door glass in front. -->
    <div class="glass"></div>
  </div>
  <div class="bar">
    <span class="rk-led" class:on={playing}>Play</span>
    <span class="rk-silk">Type II · CrO2 · Dolby B</span>
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
  .door {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: 5px;
    box-shadow: 0 0 0 1px #000, 0 1px 0 1px rgba(255, 255, 255, 0.07), inset 0 3px 8px rgba(0, 0, 0, 0.95);
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  /* Smoked door glass: darkens the cassette and carries the reflection. */
  .glass {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(166deg, rgba(255, 255, 255, 0.09) 0, rgba(255, 255, 255, 0.02) 32%, transparent 32.3%),
      linear-gradient(180deg, rgba(10, 8, 6, 0.35), rgba(10, 8, 6, 0.15) 40%, rgba(10, 8, 6, 0.45));
    box-shadow: inset 0 0 22px rgba(0, 0, 0, 0.75);
  }
  .shell {
    fill: #1b1917;
  }
  .windowrim {
    fill: rgba(255, 255, 255, 0.025);
    stroke: #34302c;
    stroke-width: 1.5;
  }
  .label {
    fill: #d9d0c2;
    opacity: 0.85;
  }
  .side {
    font: 800 11px var(--font-ui);
    fill: #8a2f1c;
  }
  .title {
    font: 700 7px var(--font-ui);
    fill: #241d16;
  }
  .artist {
    font: 500 6px var(--font-ui);
    fill: #5a4f43;
  }
  .lip {
    fill: #121110;
    stroke: #2a2724;
  }
  .hub {
    fill: #d8d2c8;
  }
  .tooth {
    fill: #0a0908;
  }
  .spindle {
    fill: #2a2724;
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
