<script lang="ts">
  import { DB_STEP, EQ_BANDS_HZ, EQ_MAX_DB, bandLabel, formatDb, formatDbShort } from "../../dsp";
  import Slider from "./Slider.svelte";

  /// The ten-band graphite-style fader bank. Each fader fills from 0 dB.
  let {
    bands,
    onband,
    disabled = false,
  }: {
    bands: number[];
    onband: (index: number, db: number) => void;
    disabled?: boolean;
  } = $props();
</script>

<div class="bank" role="group" aria-label="Equalizer bands">
  {#each EQ_BANDS_HZ as hz, index (hz)}
    {@const db = bands[index] ?? 0}
    <div class="band">
      <span class="gain" class:zero={db === 0}>{formatDbShort(db)}</span>
      <Slider
        vertical
        {disabled}
        readout={false}
        origin="center"
        min={-EQ_MAX_DB}
        max={EQ_MAX_DB}
        step={DB_STEP}
        value={db}
        label={`${bandLabel(hz)} Hz`}
        format={formatDb}
        oninput={(value) => onband(index, value)}
      />
      <span class="hz">{bandLabel(hz)}</span>
    </div>
  {/each}
</div>

<style>
  .bank {
    display: grid;
    grid-template-columns: repeat(10, minmax(0, 1fr));
    width: 100%;
    gap: 4px;
    padding: 4px 0 2px;
  }
  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .gain {
    min-height: 1.3em;
    color: var(--text);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .gain.zero {
    color: var(--text-muted);
    font-weight: 500;
  }
  .hz {
    color: var(--text-muted);
    font-size: 11.5px;
    letter-spacing: 0.02em;
  }
</style>
