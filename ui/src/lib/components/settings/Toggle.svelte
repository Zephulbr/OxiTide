<script lang="ts">
  /// A switch built on a native checkbox: the input is invisible but fills the
  /// control, so clicks, Space and the accessibility tree all use it directly.
  let {
    checked,
    label,
    disabled = false,
    onchange,
  }: {
    checked: boolean;
    label: string;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  } = $props();
</script>

<span class="switch" class:on={checked} class:disabled>
  <input
    type="checkbox"
    {checked}
    {disabled}
    aria-label={label}
    onchange={(event) => onchange((event.target as HTMLInputElement).checked)}
  />
  <span class="track" aria-hidden="true"><span class="knob"></span></span>
</span>

<style>
  .switch {
    position: relative;
    display: inline-block;
    flex: none;
    width: 40px;
    height: 24px;
  }
  input {
    position: absolute;
    inset: 0;
    z-index: 1;
    width: 100%;
    height: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .track {
    position: absolute;
    inset: 0;
    border: 1px solid color-mix(in srgb, var(--text) 28%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 14%, transparent);
    transition:
      background-color var(--dur-state) var(--ease-enter),
      border-color var(--dur-state) var(--ease-enter);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text) 78%, transparent);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.3);
    transition:
      transform var(--dur-state) var(--ease-enter),
      background-color var(--dur-state) var(--ease-enter);
  }
  input:hover:not(:disabled) + .track {
    border-color: color-mix(in srgb, var(--text) 45%, transparent);
  }
  .on .track {
    border-color: transparent;
    background: var(--accent);
  }
  .on .knob {
    transform: translateX(16px);
    background: var(--on-accent);
  }
  input:focus-visible + .track {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .disabled {
    opacity: 0.4;
  }
  .disabled input {
    cursor: default;
  }
</style>
