<script lang="ts">
  import { LED_STEPS, ledCount } from "../ballistics";
  import type { LevelsSnapshot } from "../levels.svelte";

  /// Discrete LED ladder on the faceplate (DESIGN §5.9); the peak LED stays
  /// lit through the hold.
  let { snapshot }: { snapshot: LevelsSnapshot } = $props();

  const LABELS = new Set([-40, -20, -10, -6, -3, 0, 3]);
</script>

<div class="led">
  {#each [0, 1] as ch (ch)}
    {@const lit = ledCount(snapshot.peakDb[ch])}
    {@const held = ledCount(snapshot.holdDb[ch]) - 1}
    <div class="ledrow">
      <b>{ch === 0 ? "L" : "R"}</b>
      <div class="line">
        {#each LED_STEPS as step, i (step)}
          <i class:hot={step > 0} class:on={i < lit || i === held}></i>
        {/each}
      </div>
    </div>
  {/each}
  <div class="scale">
    {#each LED_STEPS as step (step)}
      <span>{LABELS.has(step) ? (step > 0 ? `+${step}` : step) : ""}</span>
    {/each}
  </div>
</div>

<style>
  .led {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 9px;
    height: 100%;
    box-sizing: border-box;
    padding: 12px 12px 8px;
    border-radius: 4px;
    background: #050404;
    box-shadow: inset 0 0 0 2px #070605, inset 0 3px 10px rgba(0, 0, 0, 0.9), 0 1px 0 rgba(255, 255, 255, 0.08);
  }
  .ledrow {
    display: grid;
    grid-template-columns: 12px 1fr;
    gap: 8px;
    align-items: center;
  }
  b {
    font: 800 9.5px/1 var(--font-ui);
    color: var(--rack-silk-dim);
  }
  .line,
  .scale {
    display: grid;
    grid-template-columns: repeat(20, 1fr);
    gap: 3px;
  }
  i {
    height: 12px;
    border-radius: 2px;
    background: oklch(0.22 calc(var(--lamp-c) * 0.35) var(--lamp-h));
    box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.7), 0 1px 0 rgba(255, 255, 255, 0.04);
    transition: background 60ms;
  }
  i.hot {
    background: oklch(0.22 0.06 28);
  }
  i.on {
    background: var(--rack-lamp);
    box-shadow: 0 0 6px var(--rack-lamp-glow), inset 0 1px 0 rgba(255, 255, 255, 0.35);
    transition-duration: 0s;
  }
  i.hot.on {
    background: var(--rack-meter-red);
    box-shadow: 0 0 6px color-mix(in srgb, var(--rack-meter-red) 60%, transparent), inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }
  .scale {
    margin-left: 20px;
    font: 700 7px/1 var(--font-ui);
    color: var(--rack-silk-dim);
    text-align: center;
    white-space: nowrap;
  }
</style>
