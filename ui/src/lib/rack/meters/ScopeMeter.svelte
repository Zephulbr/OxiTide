<script lang="ts">
  import { goniometer, SCOPE_GAIN } from "../ballistics";
  import type { ScopeBuffer } from "../levels.svelte";

  /// Stereo audio monitor (DESIGN §5.9), after the McIntosh-style audio
  /// scopes: a square goniometer (mono = vertical line, left/right on the
  /// diagonals, width = how open the figure is) beside a dual-trace
  /// oscilloscope of the L and R waveforms. Traces leave a short phosphor
  /// afterglow. Fed by the tap's recent L/R sample pairs.
  let { buffer }: { buffer: ScopeBuffer } = $props();

  let gonio: HTMLCanvasElement;
  let wave: HTMLCanvasElement;

  /// Afterglow: how much of the previous image survives each animation frame.
  const PERSISTENCE = 0.72;

  $effect(() => {
    const g = gonio.getContext("2d");
    const w = wave.getContext("2d");
    if (!g || !w) return;
    let raf = 0;
    let seen = -1;
    let color = "";
    let colorAt = 0;
    const dpr = () => window.devicePixelRatio || 1;
    const fit = (c: HTMLCanvasElement) => {
      const box = c.getBoundingClientRect();
      c.width = Math.max(1, Math.round(box.width * dpr()));
      c.height = Math.max(1, Math.round(box.height * dpr()));
    };
    fit(gonio);
    fit(wave);
    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => {
            fit(gonio);
            fit(wave);
          });
    observer?.observe(gonio);
    observer?.observe(wave);

    const fade = (ctx: CanvasRenderingContext2D, c: HTMLCanvasElement) => {
      ctx.globalCompositeOperation = "destination-out";
      ctx.globalAlpha = 1 - PERSISTENCE;
      ctx.fillRect(0, 0, c.width, c.height);
      ctx.globalCompositeOperation = "source-over";
      ctx.globalAlpha = 1;
    };
    const stroke = (ctx: CanvasRenderingContext2D) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.3 * dpr();
      ctx.lineJoin = "round";
      ctx.shadowColor = color;
      ctx.shadowBlur = 4 * dpr();
      ctx.stroke();
      ctx.shadowBlur = 0;
    };

    const draw = (t: number) => {
      raf = requestAnimationFrame(draw);
      if (t - colorAt > 500 || !color) {
        color = getComputedStyle(gonio).color;
        colorAt = t;
      }
      fade(g, gonio);
      fade(w, wave);
      if (buffer.serial === seen) return;
      seen = buffer.serial;
      const s = buffer.samples;
      const pairs = s.length >> 1;
      if (pairs < 2) return;

      // Goniometer.
      const cx = gonio.width / 2;
      const cy = gonio.height / 2;
      const r = Math.min(cx, cy) * 0.9;
      g.beginPath();
      for (let i = 0; i < pairs; i++) {
        const [x, y] = goniometer(s[2 * i], s[2 * i + 1], SCOPE_GAIN);
        const px = cx + x * r;
        const py = cy - y * r;
        if (i === 0) g.moveTo(px, py);
        else g.lineTo(px, py);
      }
      stroke(g);

      // Dual-trace oscilloscope: L above, R below.
      const lanes: [number, number][] = [
        [wave.height * 0.27, 0],
        [wave.height * 0.73, 1],
      ];
      const amp = wave.height * 0.22;
      for (const [mid, ch] of lanes) {
        w.beginPath();
        for (let i = 0; i < pairs; i++) {
          const v = Math.max(-1, Math.min(1, s[2 * i + ch] * SCOPE_GAIN));
          const px = (i / (pairs - 1)) * wave.width;
          const py = mid - v * amp;
          if (i === 0) w.moveTo(px, py);
          else w.lineTo(px, py);
        }
        stroke(w);
      }
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      observer?.disconnect();
    };
  });
</script>

<div class="monitor">
  <div class="gonio">
    <svg class="grat" viewBox="0 0 100 100" aria-hidden="true">
      <circle cx="50" cy="50" r="45" />
      <circle cx="50" cy="50" r="22.5" class="minor" />
      <line x1="50" y1="5" x2="50" y2="95" />
      <line x1="5" y1="50" x2="95" y2="50" class="minor" />
      <line x1="18.2" y1="18.2" x2="81.8" y2="81.8" class="minor" />
      <line x1="81.8" y1="18.2" x2="18.2" y2="81.8" class="minor" />
      <text x="15" y="13">L</text>
      <text x="82" y="13">R</text>
      <text x="52.5" y="9">M</text>
      <text x="90" y="47">S</text>
    </svg>
    <canvas bind:this={gonio}></canvas>
  </div>
  <div class="wave">
    <div class="lanes" aria-hidden="true"><span>L</span><span>R</span></div>
    <canvas bind:this={wave}></canvas>
  </div>
</div>

<style>
  .monitor {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 12px;
    height: 100%;
  }
  .gonio,
  .wave {
    position: relative;
    min-height: 0;
    color: var(--rack-vfd);
  }
  .gonio {
    height: 100%;
    aspect-ratio: 1;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    color: var(--rack-vfd);
  }
  .grat {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    fill: none;
    stroke: currentColor;
    stroke-width: 0.5;
    opacity: 0.22;
  }
  .grat .minor {
    stroke-dasharray: 1.5 2;
  }
  .grat text {
    fill: currentColor;
    stroke: none;
    font: 700 6px var(--font-ui);
  }
  /* Graticule: centre lines for both lanes and a faint time grid. */
  .wave {
    border-left: 1px solid color-mix(in srgb, var(--rack-vfd) 15%, transparent);
    background:
      linear-gradient(color-mix(in srgb, var(--rack-vfd) 22%, transparent), color-mix(in srgb, var(--rack-vfd) 22%, transparent)) 0 27% / 100% 1px no-repeat,
      linear-gradient(color-mix(in srgb, var(--rack-vfd) 22%, transparent), color-mix(in srgb, var(--rack-vfd) 22%, transparent)) 0 73% / 100% 1px no-repeat,
      repeating-linear-gradient(90deg, color-mix(in srgb, var(--rack-vfd) 8%, transparent) 0 1px, transparent 1px 10%);
  }
  .lanes {
    position: absolute;
    left: 6px;
    top: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-around;
    font: 800 8px/1 var(--font-ui);
    opacity: 0.6;
  }
</style>
