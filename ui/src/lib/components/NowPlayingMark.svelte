<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import { frameDue, framesGated } from "../frameGate";

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

  /// One turn per 2 s, so motion reads at glyph size.
  const TURN_MS = 2000;
  const TURN_SPEED = 360000 / TURN_MS; // °/s
  /// 60 steps a turn: the angle changes 30 times a second (6° a step).
  /// Every changed frame costs a frame of the whole page (main thread and
  /// compositor), whatever the glyph's size: turned at 144 Hz, the mark held
  /// ~1.4 Gcycles/s; stepped, Chromium skips the frames in between and
  /// Atmosphere playback is ~0.36 (`plans/notes/2026-11-04-v1-perf-regressions-webview.md`).
  const STEPS = 60;
  /// Speed eases toward its target with this time constant (pause settles in
  /// about 600 ms instead of stopping dead).
  const EASE_S = 0.15;

  let disc: SVGGElement | undefined;
  let turn: HTMLSpanElement | undefined;
  // Outside the effect, so a pause/resume keeps the angle and the speed.
  let angle = 0;
  let speed = 0;
  let raf = 0;

  // --- Compositor turn (everywhere but Linux, where Web Animations exist) ---
  // While it turns, a stepped Web Animation rotates the wrapper on the
  // compositor: JS runs only while the speed eases (play/pause). At rest the
  // angle goes back into the SVG, drawn crisp.
  let anim: Animation | null = null;
  let base = 0; // the angle at the animation's time 0
  let target = 0;
  let last = 0;

  /// The angle on screen: the stepped value the animation shows.
  function shownAngle(): number {
    if (!anim) return angle;
    const t = Number(anim.currentTime ?? 0);
    const progress = (((t % TURN_MS) + TURN_MS) % TURN_MS) / TURN_MS;
    return (base + Math.floor(progress * STEPS) * (360 / STEPS)) % 360;
  }

  function startTurn() {
    if (anim || !turn || !disc) return;
    base = angle;
    disc.setAttribute("transform", "rotate(0 12 12)");
    anim = turn.animate(
      [{ transform: `rotate(${base}deg)` }, { transform: `rotate(${base + 360}deg)` }],
      { duration: TURN_MS, iterations: Infinity, easing: `steps(${STEPS}, end)` },
    );
    anim.playbackRate = speed / TURN_SPEED;
  }

  function endTurn() {
    if (anim) {
      angle = shownAngle();
      anim.cancel();
      anim = null;
    }
    disc?.setAttribute("transform", `rotate(${angle} 12 12)`);
  }

  function ease(now: number) {
    const dt = Math.min((now - last) / 1000, 0.1);
    last = now;
    speed += (target - speed) * Math.min(1, dt / EASE_S);
    if (Math.abs(target - speed) < 0.5) speed = target;
    if (anim) anim.playbackRate = speed / TURN_SPEED;
    if (speed !== target) {
      raf = requestAnimationFrame(ease);
      return;
    }
    raf = 0;
    if (speed === 0) endTurn();
  }

  function composited(spinning: boolean) {
    target = spinning ? TURN_SPEED : 0;
    if (!spinning && !anim) return;
    if (spinning) startTurn();
    if (speed !== target) {
      last = performance.now();
      raf = requestAnimationFrame(ease);
    }
    return () => {
      cancelAnimationFrame(raf);
      raf = 0;
    };
  }

  // --- Gated turn (Linux) ---
  // WebKitGTK presents every frame of an animation, stepped or not, so the
  // turn is drawn by a loop on `frameDue`'s shared 30 fps beat. The compositor
  // rotates the wrapper; at rest the angle goes back into the SVG.
  function show(moving: boolean) {
    if (!turn || !disc) return;
    if (moving) {
      disc.setAttribute("transform", "rotate(0 12 12)");
      turn.style.willChange = "transform";
      turn.style.transform = `rotate(${angle}deg)`;
    } else {
      turn.style.transform = "";
      turn.style.willChange = "";
      disc.setAttribute("transform", `rotate(${angle} 12 12)`);
    }
  }

  function gated(spinning: boolean) {
    if (!spinning && speed === 0) return;
    let at = performance.now();
    const tick = (now: number) => {
      if (!frameDue(now)) {
        raf = requestAnimationFrame(tick);
        return;
      }
      const dt = Math.min((now - at) / 1000, 0.1);
      at = now;
      speed += ((spinning ? TURN_SPEED : 0) - speed) * Math.min(1, dt / EASE_S);
      if (Math.abs(speed) < 0.01) speed = 0;
      angle = (angle + speed * dt) % 360;
      const moving = spinning || speed !== 0;
      show(moving);
      raf = moving ? requestAnimationFrame(tick) : 0;
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      raf = 0;
      show(false);
    };
  }

  // The turn runs only while the disc moves: it starts on play and ends once
  // a pause has eased it to a stop (paused is idle).
  $effect(() => {
    const spinning = playing;
    if (!appearance.ambientMotion) {
      anim?.cancel();
      anim = null;
      angle = 0;
      speed = 0;
      show(false);
      return;
    }
    // Without Web Animations (jsdom) the loop draws it, on every frame off Linux.
    const compositor = !framesGated && typeof turn?.animate === "function";
    return compositor ? composited(spinning) : gated(spinning);
  });
</script>

<span class="np-mark" bind:this={turn} style:width="{size}px" style:height="{size}px">
  <svg
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
</span>

<style>
  .np-mark {
    display: block;
    flex: 0 0 auto;
  }
  .np-mark svg {
    display: block;
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
