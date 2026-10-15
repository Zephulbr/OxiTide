<script lang="ts" generics="T extends string">
  import Icon, { type IconName } from "../Icon.svelte";

  /// A row of mutually exclusive choices. Keeps the `.segmented` + `.active`
  /// hooks the Rack skin restyles as light-bar keys. Arrow keys move focus
  /// between options and never reach the global transport shortcuts.
  let {
    value,
    options,
    label,
    onchange,
    disabled = false,
  }: {
    value: T;
    options: { value: T; label: string; icon?: IconName; hint?: string }[];
    label: string;
    onchange: (value: T) => void;
    disabled?: boolean;
  } = $props();

  function onKeydown(event: KeyboardEvent) {
    const keys = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const group = event.currentTarget as HTMLElement;
    const buttons = [...group.querySelectorAll<HTMLButtonElement>("button")];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (at < 0) return;
    event.preventDefault();
    event.stopPropagation();
    let next = at;
    if (event.key === "Home") next = 0;
    else if (event.key === "End") next = buttons.length - 1;
    else next = (at + (event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="segmented" role="group" aria-label={label} onkeydown={onKeydown}>
  {#each options as option (option.value)}
    <button
      type="button"
      class:active={option.value === value}
      aria-pressed={option.value === value}
      {disabled}
      title={option.hint}
      onclick={() => onchange(option.value)}
    >
      {#if option.icon}<Icon name={option.icon} size={14} />{/if}
      {option.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: calc(var(--radius-control) - 3px);
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
  }
  button:hover:not(.active) {
    background: var(--surface-2);
    color: var(--text);
  }
  button.active {
    background: color-mix(in srgb, var(--accent) 18%, var(--surface-3));
    color: var(--text);
    font-weight: 600;
  }
  button.active :global(svg) {
    color: var(--accent);
  }
</style>
