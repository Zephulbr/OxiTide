<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    icon,
    label,
    active = false,
    fill = true,
    danger = false,
    disabled = false,
    title,
    onclick,
  }: {
    icon: IconName;
    label: string;
    active?: boolean;
    /// Fill the icon while active (the heart); off for outline-only marks
    /// such as the globe, which fill into a solid dot.
    fill?: boolean;
    danger?: boolean;
    disabled?: boolean;
    title?: string;
    onclick?: (event: MouseEvent) => void;
  } = $props();
</script>

<button
  class="action"
  class:on={active}
  class:fill
  class:danger
  {title}
  {disabled}
  {onclick}
>
  <Icon name={icon} size={20} />
  <span>{label}</span>
</button>

<style>
  .action {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    min-width: 62px;
    padding: 6px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    transition:
      color var(--dur-micro) var(--ease-enter),
      background var(--dur-micro) var(--ease-enter);
  }
  .action:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }
  .action.on {
    color: var(--accent);
  }
  .action.on.fill :global(svg) {
    fill: currentColor;
  }
  .action.danger:hover:not(:disabled) {
    color: var(--danger);
  }
</style>
