<script lang="ts">
  import { poolBands, waterfallIntensity } from "../ballistics";
  import type { LevelsSnapshot } from "../levels.svelte";

  /// VFD waterfall (DESIGN §5.9): the spectrum's recent history scrolling
  /// downward — newest row on top, each band a segmented cell whose
  /// brightness is its level. Same data as the bars, read over time, so the
  /// analyzer never repeats the amp's spectrum.
  let { snapshot, count = 28, scale = true }: { snapshot: LevelsSnapshot; count?: number; scale?: boolean } = $props();

  const ROW_MS = 45;
  const ROW_H = 3;
  const ROW_GAP = 2;
  const COL_GAP = 3;

  let canvas: HTMLCanvasElement;

  $effect(() => {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    let color = "";
    let colorAt = 0;
    let last = 0;
    let raf = 0;
    const dpr = () => window.devicePixelRatio || 1;

    const fit = () => {
      const box = canvas.getBoundingClientRect();
      canvas.width = Math.max(1, Math.round(box.width * dpr()));
      canvas.height = Math.max(1, Math.round(box.height * dpr()));
    };
    fit();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(fit);
    observer?.observe(canvas);

    const draw = (t: number) => {
      raf = requestAnimationFrame(draw);
      if (t - last < ROW_MS) return;
      last = t;
      // The lamp colour glides on cover changes; re-read it now and then.
      if (t - colorAt > 500 || !color) {
        color = getComputedStyle(canvas).color;
        colorAt = t;
      }
      const s = dpr();
      const pitch = (ROW_H + ROW_GAP) * s;
      const w = canvas.width;
      const h = canvas.height;
      // Scroll the history down one row, then paint the newest row on top.
      ctx.globalAlpha = 1;
      ctx.drawImage(canvas, 0, 0, w, h - pitch, 0, pitch, w, h - pitch);
      ctx.clearRect(0, 0, w, pitch);
      const bands = poolBands(snapshot.raw, count);
      const gap = COL_GAP * s;
      const cw = (w - gap * (count - 1)) / count;
      ctx.fillStyle = color;
      for (let i = 0; i < count; i++) {
        ctx.globalAlpha = waterfallIntensity(bands[i] ?? 0);
        ctx.fillRect(i * (cw + gap), 0, cw, ROW_H * s);
      }
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      observer?.disconnect();
    };
  });
</script>

<div class="fall">
  <canvas bind:this={canvas}></canvas>
  {#if scale}
    <div class="scale"><span>63</span><span>250</span><span>1k</span><span>4k</span><span>16k</span></div>
  {/if}
</div>

<style>
  .fall {
    display: flex;
    flex-direction: column;
    gap: 5px;
    height: 100%;
  }
  canvas {
    flex: 1;
    min-height: 0;
    width: 100%;
    color: var(--rack-vfd);
    filter: drop-shadow(0 0 1px var(--rack-vfd));
    /* Older rows fade toward the bottom, like phosphor decaying. */
    mask: linear-gradient(180deg, #000 0%, #000 35%, rgba(0, 0, 0, 0.25) 100%);
  }
  .scale {
    display: flex;
    justify-content: space-between;
    font: 700 7.5px/1 var(--font-ui);
    opacity: 0.6;
  }
</style>
