<script lang="ts">
  import { bandLabel, EQ_BANDS_HZ, EQ_MAX_DB, formatDb, matchPreset, presetBands, withBand } from "../dsp";
  import { dspStore } from "../dspStore.svelte";
  import { errorMessage } from "../errors";
  import type { Toasts } from "../toasts.svelte";

  /// Graphic EQ unit (spec §6.3): dimple presets + ten faders bound to the
  /// shared DSP store. Moving a fader while EQ is off turns it on.
  let { toasts }: { toasts: Toasts } = $props();

  const PRESETS = [
    { id: "flat", label: "Flat" },
    { id: "bass", label: "Bass" },
    { id: "treble", label: "Treble" },
    { id: "vocal", label: "Vocal" },
    { id: "loudness", label: "Loudness" },
  ];

  $effect(() => {
    void dspStore.ensure();
  });

  const dsp = $derived(dspStore.settings);
  const bands = $derived(EQ_BANDS_HZ.map((_, i) => dsp?.eq.bands_db[i] ?? 0));
  const preset = $derived(matchPreset(bands));
  const report = (e: unknown) => toasts.show(errorMessage(e), { severity: "error" });

  function setBand(index: number, db: number) {
    if (!dsp) return;
    const eq = withBand(dsp.eq, index, db);
    dspStore.set({ ...dsp, eq: { ...eq, enabled: true } }, report);
  }
  function setPreset(id: string) {
    const next = presetBands(id);
    if (dsp && next) dspStore.set({ ...dsp, eq: { ...dsp.eq, bands_db: next, enabled: true } }, report);
  }

  function dbAt(slot: HTMLElement, clientY: number): number {
    const box = slot.getBoundingClientRect();
    return EQ_MAX_DB - ((clientY - box.top) / box.height) * 2 * EQ_MAX_DB;
  }
  function onPointerDown(event: PointerEvent, index: number) {
    const slot = event.currentTarget as HTMLElement;
    slot.setPointerCapture(event.pointerId);
    setBand(index, dbAt(slot, event.clientY));
  }
  function onPointerMove(event: PointerEvent, index: number) {
    const slot = event.currentTarget as HTMLElement;
    if (slot.hasPointerCapture(event.pointerId)) setBand(index, dbAt(slot, event.clientY));
  }
  function onKeydown(event: KeyboardEvent, index: number) {
    const step: Record<string, number> = { ArrowUp: 0.5, ArrowRight: 0.5, ArrowDown: -0.5, ArrowLeft: -0.5, PageUp: 3, PageDown: -3 };
    let next: number | null = null;
    if (event.key in step) next = bands[index] + step[event.key];
    else if (event.key === "Home") next = -EQ_MAX_DB;
    else if (event.key === "End") next = EQ_MAX_DB;
    if (next === null) return;
    event.preventDefault();
    setBand(index, next);
  }
  function onWheel(event: WheelEvent, index: number) {
    event.preventDefault();
    setBand(index, bands[index] - Math.sign(event.deltaY) * 0.5);
  }
</script>

<section class="rk-unit eq" aria-label="Graphic equalizer">
  <div class="rk-head">
    <h2 class="rk-name">Graphic equalizer</h2>
    {#if preset === "custom"}<span class="rk-silk">Custom</span>{/if}
  </div>
  <div class="presets" role="radiogroup" aria-label="EQ preset">
    {#each PRESETS as p (p.id)}
      <button class="preset" role="radio" aria-checked={preset === p.id} disabled={!dsp} onclick={() => setPreset(p.id)}>
        <span class="dimple"></span>{p.label}
      </button>
    {/each}
  </div>
  <div class="faders">
    {#each EQ_BANDS_HZ as hz, i (hz)}
      <div class="fader">
        <div
          class="slot"
          role="slider"
          tabindex="0"
          aria-label="{bandLabel(hz)} Hz"
          aria-orientation="vertical"
          aria-valuemin={-EQ_MAX_DB}
          aria-valuemax={EQ_MAX_DB}
          aria-valuenow={bands[i]}
          aria-valuetext={formatDb(bands[i])}
          style:--g={(EQ_MAX_DB - bands[i]) / (2 * EQ_MAX_DB)}
          onpointerdown={(e) => onPointerDown(e, i)}
          onpointermove={(e) => onPointerMove(e, i)}
          onkeydown={(e) => onKeydown(e, i)}
          onwheel={(e) => onWheel(e, i)}
        >
          <span class="ladder"></span>
          <span class="cap" class:off={!dsp?.eq.enabled}></span>
        </div>
        <span class="hz">{bandLabel(hz)}</span>
      </div>
    {/each}
  </div>
</section>

<style>
  .eq {
    display: flex;
    flex-direction: column;
  }
  .presets {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    margin: 0 4px 8px;
  }
  .preset {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--rack-silk-dim);
    font: 600 10px/1 var(--font-ui);
    letter-spacing: 0.04em;
    cursor: pointer;
  }
  .preset[aria-checked="true"] {
    color: var(--rack-silk);
  }
  .dimple {
    position: relative;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: linear-gradient(145deg, #262524, #121110);
    box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.7), -1px -1px 2px rgba(255, 255, 255, 0.08);
    transition: box-shadow 200ms, transform 200ms;
  }
  .preset[aria-checked="true"] .dimple {
    transform: scale(1.08);
    box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.7), -1px -1px 2px rgba(255, 255, 255, 0.08), inset 2px 2px 3px rgba(0, 0, 0, 0.8), inset -1px -1px 2px rgba(255, 255, 255, 0.06);
  }
  .preset[aria-checked="true"] .dimple::after {
    content: "";
    position: absolute;
    inset: 4.5px;
    border-radius: 50%;
    background: var(--rack-lamp);
    box-shadow: 0 0 5px var(--rack-lamp-glow);
  }
  .faders {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 4px;
    justify-items: center;
    align-content: center;
    min-height: 96px;
  }
  .fader {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
  }
  .slot {
    position: relative;
    width: 5px;
    height: 72px;
    border-radius: 2px;
    background: #000;
    cursor: ns-resize;
    touch-action: none;
  }
  .ladder {
    position: absolute;
    inset: 0 -10px;
    pointer-events: none;
    background:
      linear-gradient(var(--rack-silk-dim), var(--rack-silk-dim)) left top / 5px 1px no-repeat,
      linear-gradient(var(--rack-silk-dim), var(--rack-silk-dim)) right top / 5px 1px no-repeat,
      linear-gradient(var(--rack-silk-dim), var(--rack-silk-dim)) left bottom / 5px 1px no-repeat,
      linear-gradient(var(--rack-silk-dim), var(--rack-silk-dim)) right bottom / 5px 1px no-repeat;
  }
  .ladder::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 1px;
    background: var(--rack-silk-dim);
    opacity: 0.9;
  }
  .cap {
    position: absolute;
    left: 50%;
    top: calc(var(--g) * 100% - 6.5px);
    width: 24px;
    height: 13px;
    margin-left: -12px;
    border-radius: 2px;
    z-index: 1;
    border: 1px solid #0a0a0a;
    background: linear-gradient(to bottom, #3a3a3a 0%, #575757 38%, #1a1a1a 44%, var(--rack-lamp) 47%, var(--rack-lamp) 53%, #1a1a1a 56%, #575757 62%, #3a3a3a 100%);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.75), 0 4px 10px rgba(0, 0, 0, 0.45), inset 0 1px 1px rgba(255, 255, 255, 0.18);
  }
  .cap.off {
    background: linear-gradient(to bottom, #3a3a3a 0%, #575757 38%, #1a1a1a 44%, #3a3633 47%, #3a3633 53%, #1a1a1a 56%, #575757 62%, #3a3a3a 100%);
  }
  .slot:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 6px;
  }
  .hz {
    font: 700 8.5px/1 var(--font-ui);
    letter-spacing: 0.04em;
    color: var(--rack-silk-dim);
  }
</style>
