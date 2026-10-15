<script lang="ts">
  /// A numeric field with a unit suffix. Commits on blur/Enter, clamped and
  /// rounded to whole numbers, and writes the clamped value back so the field
  /// never shows something that was not saved.
  let {
    value,
    min,
    max,
    unit = "",
    label,
    onchange,
    width = 110,
  }: {
    value: number;
    min: number;
    max?: number;
    unit?: string;
    label: string;
    onchange: (value: number) => void;
    width?: number;
  } = $props();

  function commit(event: Event) {
    const input = event.target as HTMLInputElement;
    const parsed = Number(input.value);
    const upper = max ?? Number.POSITIVE_INFINITY;
    const next = Number.isFinite(parsed) ? Math.min(upper, Math.max(min, Math.round(parsed))) : value;
    input.value = String(next);
    if (next !== value) onchange(next);
  }
</script>

<label class="number">
  <input
    class="search-input"
    type="number"
    {min}
    {max}
    {value}
    aria-label={label}
    style:width="{width}px"
    onchange={commit}
    onkeydown={(event) => {
      if (event.key === "Enter") (event.target as HTMLInputElement).blur();
    }}
  />
  {#if unit}<span class="unit">{unit}</span>{/if}
</label>

<style>
  .number {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  input {
    padding: 7px 10px;
    font-size: 13.5px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    -moz-appearance: textfield;
    appearance: textfield;
  }
  input::-webkit-inner-spin-button,
  input::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  .unit {
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
