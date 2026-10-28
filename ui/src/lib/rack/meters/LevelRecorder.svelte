<script lang="ts">
  import { frameDue } from "../../frameGate";
  import { RECORDER_INTERVAL_MS, RECORDER_POINTS, recorderFraction } from "../ballistics";
  import type { RecorderHistory } from "../levels.svelte";

  /// Chart-recorder strip (DESIGN §5.9): L and R programme level over the last
  /// minute, drifting slowly left. Calm by design — one point every 250 ms,
  /// smooth sub-step scrolling in between, and it stands still while paused.
  let { history }: { history: RecorderHistory } = $props();

  const GRID = [-20, -10, 0];
  /// Shortest step between redraws while the paper glides (~30 Hz).
  const DRAW_STEP_MS = 1000 / 30 - 1;
  const LABELS = [3, 0, -10, -20];

  let canvas: HTMLCanvasElement;

  $effect(() => {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    let raf = 0;
    let idle: ReturnType<typeof setTimeout> | null = null;
    let color = "";
    let colorAt = 0;
    let drawnAt = -Infinity;
    /// What the canvas shows; a frame with the same inputs is skipped.
    let drawn = { serial: -1, offset: -1, color: "" };
    const dpr = () => window.devicePixelRatio || 1;
    const fit = () => {
      const box = canvas.getBoundingClientRect();
      canvas.width = Math.max(1, Math.round(box.width * dpr()));
      canvas.height = Math.max(1, Math.round(box.height * dpr()));
      drawn = { serial: -1, offset: -1, color: "" }; // resizing clears the canvas
    };
    fit();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(fit);
    observer?.observe(canvas);

    const trace = (values: readonly number[], offset: number, alpha: number, dash: number[]) => {
      const w = canvas.width;
      const h = canvas.height;
      const dx = w / (RECORDER_POINTS - 1);
      const n = values.length;
      if (n < 2) return;
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const x = w - (n - 1 - i + offset) * dx;
        const y = h - recorderFraction(values[i]) * h;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.globalAlpha = alpha;
      ctx.setLineDash(dash.map((d) => d * dpr()));
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.4 * dpr();
      ctx.lineJoin = "round";
      ctx.shadowColor = color;
      ctx.shadowBlur = 3 * dpr();
      ctx.stroke();
      ctx.shadowBlur = 0;
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    };

    const draw = (t: number) => {
      // Track 13: the paper crosses its width once a minute (a few px a
      // second), so ~30 redraws a second glide as smoothly as one per frame.
      if (!frameDue(t, drawnAt, DRAW_STEP_MS)) {
        raf = requestAnimationFrame(draw);
        return;
      }
      drawnAt = t;
      if (t - colorAt > 500 || !color) {
        color = getComputedStyle(canvas).color;
        colorAt = t;
      }
      // Glide between points while new ones keep arriving; freeze when they
      // stop (paused), so the paper does not run on silence.
      const since = performance.now() - history.lastAt;
      const offset = since < RECORDER_INTERVAL_MS * 2 ? Math.min(1, since / RECORDER_INTERVAL_MS) : 0;
      const frozen = offset === 0 && drawn.offset === 0 && drawn.serial === history.serial;
      if (!frozen || drawn.color !== color) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        trace(history.right, offset, 0.45, [4, 3]);
        trace(history.left, offset, 1, []);
        drawn = { serial: history.serial, offset, color };
      }
      // Track 13 (EF-D4): a frozen strip is checked once per recorder
      // interval instead of every frame; a new point resumes the glide.
      if (frozen) {
        idle = setTimeout(() => {
          idle = null;
          raf = requestAnimationFrame(draw);
        }, RECORDER_INTERVAL_MS);
      } else {
        raf = requestAnimationFrame(draw);
      }
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      if (idle !== null) clearTimeout(idle);
      observer?.disconnect();
    };
  });

  const pct = (db: number) => `${(1 - recorderFraction(db)) * 100}%`;
</script>

<div class="recorder">
  <div class="paper">
    <div class="over" style:height={pct(0)} aria-hidden="true"></div>
    {#each GRID as db (db)}
      <div class="grid" class:zero={db === 0} style:top={pct(db)} aria-hidden="true"></div>
    {/each}
    <canvas bind:this={canvas}></canvas>
    <div class="legend" aria-hidden="true"><span class="l">L</span><span class="r">R</span></div>
  </div>
  <div class="dbscale" aria-hidden="true">
    {#each LABELS as db (db)}
      <span style:top={pct(db)}>{db > 0 ? `+${db}` : db}</span>
    {/each}
  </div>
  <div class="time" aria-hidden="true"><span>−60 s</span><span>−30 s</span><span>now</span></div>
</div>

<style>
  .recorder {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 26px;
    grid-template-rows: minmax(0, 1fr) auto;
    gap: 4px 6px;
    height: 100%;
    color: var(--rack-vfd);
  }
  .paper {
    position: relative;
    min-height: 0;
    overflow: hidden;
    /* Faint time divisions every 10 s. */
    background: repeating-linear-gradient(90deg, transparent 0 calc(100% / 6 - 1px), color-mix(in srgb, var(--rack-vfd) 8%, transparent) calc(100% / 6 - 1px) calc(100% / 6));
  }
  .over {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    background: color-mix(in srgb, var(--rack-meter-red) 7%, transparent);
  }
  .grid {
    position: absolute;
    left: 0;
    right: 0;
    height: 1px;
    background: color-mix(in srgb, var(--rack-vfd) 12%, transparent);
  }
  .grid.zero {
    background: color-mix(in srgb, var(--rack-meter-red) 45%, transparent);
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    color: var(--rack-vfd);
  }
  .legend {
    position: absolute;
    top: 4px;
    left: 6px;
    display: flex;
    gap: 10px;
    font: 800 8px/1 var(--font-ui);
    letter-spacing: 0.08em;
  }
  .legend span::before {
    content: "";
    display: inline-block;
    width: 12px;
    height: 0;
    margin-right: 4px;
    vertical-align: middle;
    border-top: 1.5px solid currentColor;
  }
  .legend .r {
    opacity: 0.6;
  }
  .legend .r::before {
    border-top-style: dashed;
  }
  .dbscale {
    position: relative;
    font: 700 7.5px/1 var(--font-ui);
    opacity: 0.6;
  }
  .dbscale span {
    position: absolute;
    left: 0;
    transform: translateY(-50%);
    font-variant-numeric: tabular-nums;
  }
  .time {
    display: flex;
    justify-content: space-between;
    font: 700 7.5px/1 var(--font-ui);
    opacity: 0.6;
  }
</style>
