<script lang="ts">
  /// A native range input with a filled track. `origin="center"` fills from
  /// the middle (gains that can go both ways); `vertical` stands it up for the
  /// equalizer by rotating a horizontal input, which keeps native keyboard
  /// handling and works in every webview. `oninput` fires on every step,
  /// `oncommit` once on release (for settings that persist to disk).
  let {
    value,
    min,
    max,
    step = 1,
    label,
    format,
    oninput,
    oncommit,
    origin = "min",
    vertical = false,
    readout = true,
    disabled = false,
    width = 300,
  }: {
    value: number;
    min: number;
    max: number;
    step?: number;
    label: string;
    format: (value: number) => string;
    oninput?: (value: number) => void;
    oncommit?: (value: number) => void;
    origin?: "min" | "center";
    vertical?: boolean;
    readout?: boolean;
    disabled?: boolean;
    /// Track + readout width in px (horizontal only); shrinks in narrow rows.
    width?: number;
  } = $props();

  // The thumb, fill and readout follow the pointer even when the stored value
  // only changes on release; a new stored value takes over again.
  let live = $state(0);
  $effect.pre(() => {
    live = value;
  });

  const pct = (v: number) => ((Math.min(max, Math.max(min, v)) - min) / (max - min)) * 100;
  const fill = $derived.by(() => {
    const at = pct(live);
    if (origin === "center") return { from: Math.min(at, 50), to: Math.max(at, 50) };
    return { from: 0, to: at };
  });
</script>

<div class="slider" class:vertical style:width={vertical ? undefined : `min(${width}px, 100%)`}>
  <input
    type="range"
    {min}
    {max}
    {step}
    value={live}
    {disabled}
    aria-label={label}
    aria-valuetext={format(live)}
    style:--from="{fill.from}%"
    style:--to="{fill.to}%"
    oninput={(event) => {
      live = Number((event.target as HTMLInputElement).value);
      oninput?.(live);
    }}
    onchange={(event) => oncommit?.(Number((event.target as HTMLInputElement).value))}
  />
  {#if readout}<output>{format(live)}</output>{/if}
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  input {
    flex: 1 1 auto;
    min-width: 0;
    height: 24px;
    margin: 0;
    background: transparent;
    cursor: pointer;
    appearance: none;
    -webkit-appearance: none;
  }
  input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--text) 16%, transparent) 0 var(--from),
      var(--accent) var(--from) var(--to),
      color-mix(in srgb, var(--text) 16%, transparent) var(--to) 100%
    );
  }
  input::-webkit-slider-thumb {
    appearance: none;
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    border: 0;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.4);
    transition: box-shadow var(--dur-micro) var(--ease-enter);
  }
  input:hover::-webkit-slider-thumb {
    box-shadow:
      0 1px 3px rgb(0 0 0 / 0.4),
      0 0 0 4px color-mix(in srgb, var(--accent) 18%, transparent);
  }
  input:active::-webkit-slider-thumb {
    box-shadow:
      0 1px 3px rgb(0 0 0 / 0.4),
      0 0 0 6px color-mix(in srgb, var(--accent) 28%, transparent);
  }
  input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 4px;
    border-radius: 999px;
  }
  input:disabled {
    cursor: default;
    opacity: 0.4;
  }
  output {
    flex: none;
    min-width: 6ch;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  /* Vertical: a fixed-height box holding the input rotated a quarter turn. */
  .vertical {
    position: relative;
    justify-content: center;
    width: 28px;
    height: 132px;
  }
  .vertical input {
    position: absolute;
    top: 50%;
    left: 50%;
    flex: none;
    width: 132px;
    transform: translate(-50%, -50%) rotate(-90deg);
  }
</style>
