<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import { frameDue } from "../frameGate";

  /// The "now playing" mark (queue current row, track tables, sidebar
  /// playlists): a tiny accent record — the Atmosphere disc motif at glyph
  /// size. It spins while playback runs and eases to a stop on pause, like
  /// Now Playing's DiscArt; without ambient motion (reduced motion,
  /// Instrument, Rack) it stays still.
  let {
    playing,
    size = 16,
    label = null,
  }: {
    playing: boolean;
    size?: number;
    /// Accessible name; null when the host already says "now playing".
    label?: string | null;
  } = $props();

  let disc: SVGGElement | undefined;
  // Outside the effect, so a pause/resume keeps the angle and the speed.
  let angle = 0;
  let speed = 0;

  // Same eased-speed loop as DiscArt, faster (one turn per 2 s) so motion
  // reads at glyph size. The loop runs only while the disc turns: it starts
  // on play and ends once a pause has eased it to a stop (paused is idle).
  $effect(() => {
    const spinning = playing;
    if (!appearance.ambientMotion) {
      angle = 0;
      speed = 0;
      disc?.setAttribute("transform", "rotate(0 12 12)");
      return;
    }
    if (!spinning && speed === 0) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      if (!frameDue(now)) {
        raf = requestAnimationFrame(tick);
        return;
      }
      const dt = Math.min((now - last) / 1000, 0.1);
      last = now;
      speed += ((spinning ? 180 : 0) - speed) * Math.min(1, dt / 0.15);
      if (Math.abs(speed) < 0.01) speed = 0;
      angle = (angle + speed * dt) % 360;
      disc?.setAttribute("transform", `rotate(${angle} 12 12)`);
      if (spinning || speed !== 0) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });
</script>

<svg
  class="np-mark"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  role={label ? "img" : undefined}
  aria-label={label ?? undefined}
  aria-hidden={label ? undefined : "true"}
>
  <g bind:this={disc}>
    <circle class="vinyl" cx="12" cy="12" r="11" />
    <circle class="groove" cx="12" cy="12" r="8" />
    <circle class="groove" cx="12" cy="12" r="5.6" />
    <!-- The sheen is the one asymmetric part: it is what makes the spin visible. -->
    <path class="sheen" d="M 12 2.6 A 9.4 9.4 0 0 1 20.1 7.3" />
    <circle class="label" cx="12" cy="12" r="3.2" />
  </g>
</svg>

<style>
  .np-mark {
    display: block;
    flex: 0 0 auto;
  }
  .vinyl {
    fill: var(--accent);
  }
  .groove {
    fill: none;
    stroke: color-mix(in srgb, var(--bg) 35%, transparent);
    stroke-width: 0.8;
  }
  .sheen {
    fill: none;
    /* A specular highlight is a material, light in every theme (DiscArt's
       sheen is the same white). */
    stroke: rgb(255 255 255 / 0.6);
    stroke-width: 1.6;
    stroke-linecap: round;
  }
  .label {
    fill: var(--bg);
  }
</style>
