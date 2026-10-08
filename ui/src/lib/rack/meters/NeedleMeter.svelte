<script lang="ts">
  import { needleAngle } from "../ballistics";

  /// Analog VU meter (DESIGN §5.9): −20…+3 VU scale, red over-zone, needle
  /// pivoting ±46° below the face. Decorative (aria-hidden by the parent).
  let { channel, position }: { channel: "L" | "R"; position: number } = $props();

  const PIV = { x: 100, y: 150 };
  const R = 112;
  const VU = [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3];
  const MAJOR = [-20, -10, -7, -5, -3, 0, 3];
  const posOf = (db: number) => (10 ** (db / 20) - 0.1) / (10 ** (3 / 20) - 0.1);
  const angOf = (p: number) => ((-46 + p * 92) * Math.PI) / 180;
  const pt = (a: number, r: number) => [PIV.x + Math.sin(a) * r, PIV.y - Math.cos(a) * r];
  const arc = (a0: number, a1: number, r: number) => {
    const [x0, y0] = pt(a0, r);
    const [x1, y1] = pt(a1, r);
    return `M${x0} ${y0} A${r} ${r} 0 0 1 ${x1} ${y1}`;
  };
  const zero = posOf(0);
  const ticks = VU.map((db) => {
    const a = angOf(posOf(db));
    const major = MAJOR.includes(db);
    const [x0, y0] = pt(a, R - (major ? 9 : 6));
    const [x1, y1] = pt(a, R + 1);
    const [tx, ty] = pt(a, R + 9);
    return { db, x0, y0, x1, y1, tx, ty, major, hot: db > 0, label: db > 0 ? `+${db}` : String(Math.abs(db)) };
  });
  const modulation = [0, 0.25, 0.5, 0.75, 1].map((p) => {
    const a = angOf(p * zero);
    const [x0, y0] = pt(a, R - 16);
    const [x1, y1] = pt(a, R - 20);
    return { x0, y0, x1, y1 };
  });
  const angle = $derived(needleAngle(position));
</script>

<div class="meter">
  <svg viewBox="0 0 200 129">
    <path d={arc(angOf(0), angOf(zero), R)} class="ink" stroke-width="1.6" fill="none" />
    <path d={arc(angOf(zero), angOf(1), R)} class="red" stroke-width="5" fill="none" />
    {#each ticks as t (t.db)}
      <line x1={t.x0} y1={t.y0} x2={t.x1} y2={t.y1} class={t.hot ? "red" : "ink"} stroke-width={t.major ? 1.6 : 1} />
      <text x={t.tx} y={t.ty} font-size="8.5" font-weight="700" text-anchor="middle" class={t.hot ? "red-fill" : "ink-fill"}>{t.label}</text>
    {/each}
    {#each modulation as m, i (i)}
      <line x1={m.x0} y1={m.y0} x2={m.x1} y2={m.y1} class="ink" stroke-width=".8" opacity=".6" />
    {/each}
    <text x="100" y="92" font-size="17" font-weight="700" text-anchor="middle" class="ink-fill">VU</text>
    <text x="22" y="118" font-size="9" font-weight="700" class="ink-fill" opacity=".75">{channel}</text>
    <line
      class="needle"
      x1={PIV.x}
      y1={PIV.y}
      x2={PIV.x}
      y2={PIV.y - R - 4}
      stroke-width="1.4"
      transform="rotate({angle} {PIV.x} {PIV.y})"
    />
    <rect x="0" y="124" width="200" height="5" fill="rgba(0,0,0,.5)" />
    <ellipse cx="100" cy="146" rx="26" ry="18" fill="#141210" />
  </svg>
  <div class="glass"></div>
</div>

<style>
  .meter {
    position: relative;
    aspect-ratio: 1.55;
    overflow: hidden;
    border-radius: 4px;
    background: var(--rack-meter-face);
    box-shadow: inset 0 0 0 2px #070605, inset 0 3px 10px rgba(0, 0, 0, 0.9), 0 1px 0 rgba(255, 255, 255, 0.08);
  }
  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    font-family: var(--font-ui);
  }
  .ink {
    stroke: var(--rack-meter-ink);
  }
  .red {
    stroke: var(--rack-meter-red);
  }
  .ink-fill {
    fill: var(--rack-meter-ink);
  }
  .red-fill {
    fill: var(--rack-meter-red);
  }
  .needle {
    stroke: var(--rack-needle);
  }
  .glass {
    position: absolute;
    inset: 0;
    pointer-events: none;
    mix-blend-mode: screen;
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.16), rgba(255, 255, 255, 0.02) 38%, transparent 39%),
      radial-gradient(90% 60% at 50% 120%, var(--rack-lamp-glow), transparent 70%);
  }
</style>
