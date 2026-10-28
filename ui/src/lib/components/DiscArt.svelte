<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import { frameDue } from "../frameGate";
  import Cover from "./Cover.svelte";

  let { uuid, playing }: { uuid: string | null | undefined; playing: boolean } = $props();
  let disc: HTMLDivElement | undefined;

  // A transform-only rAF loop; speed eases toward the target (~30°/s ≈
  // 12s/rev, a design value per spec §7.2) with a ~150ms time constant, so
  // pause settles in about 600ms instead of stopping dead. The loop is gated
  // on ambient motion: reduced-motion / Instrument never start it. Track 13
  // (EF-D4): once a paused disc has come to rest the loop ends instead of
  // ticking every frame; play starts it again from the same angle.
  let raf = 0;
  let last = 0;
  let angle = 0;
  let speed = 0;
  const tick = (now: number) => {
    if (!frameDue(now)) {
      raf = requestAnimationFrame(tick);
      return;
    }
    const dt = Math.min((now - last) / 1000, 0.1);
    last = now;
    const target = playing ? 30 : 0;
    speed += (target - speed) * Math.min(1, dt / 0.15);
    if (Math.abs(speed) < 0.001) speed = 0;
    angle = (angle + speed * dt) % 360;
    if (disc) disc.style.transform = `rotate(${angle}deg)`;
    raf = !playing && speed === 0 ? 0 : requestAnimationFrame(tick);
  };

  $effect(() => {
    if (!appearance.ambientMotion) {
      if (disc) disc.style.transform = "rotate(0deg)";
      return;
    }
    return () => {
      cancelAnimationFrame(raf);
      raf = 0;
    };
  });

  $effect(() => {
    if (!appearance.ambientMotion || !playing || raf !== 0) return;
    last = performance.now();
    raf = requestAnimationFrame(tick);
  });
</script>

<div class="disc-wrap">
  <div class="disc" bind:this={disc}>
    <Cover {uuid} size={640} radius={0} alt="" />
    <div class="grooves"></div>
    <div class="sheen"></div>
    <div class="label-ring"></div>
  </div>
</div>

<style>
  .disc-wrap {
    display: grid;
    place-items: center;
    width: min(100%, 55vh);
    margin: 0 auto;
  }
  .disc {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 50%;
    overflow: hidden;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    will-change: transform;
  }
  .grooves {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: repeating-radial-gradient(circle, rgba(0, 0, 0, 0.22) 0 1px, transparent 1px 4px);
  }
  .sheen {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: conic-gradient(from 200deg, transparent 0deg, rgba(255, 255, 255, 0.1) 40deg, transparent 100deg);
  }
  /* Tertiary ring: informational emphasis (spec §5.2). */
  .label-ring {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 24%;
    aspect-ratio: 1;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    border: 3px solid var(--dyn-tertiary);
    background: var(--bg);
  }
</style>
