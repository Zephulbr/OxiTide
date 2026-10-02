<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    icon,
    label,
    active = false,
    danger = false,
    disabled = false,
    title,
    onclick,
  }: {
    icon: IconName;
    label: string;
    active?: boolean;
    danger?: boolean;
    disabled?: boolean;
    title?: string;
    onclick?: (event: MouseEvent) => void;
  } = $props();
</script>

<button
  class="action"
  class:on={active}
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
      color var(--dur-micro) var(--ease-in),
      background var(--dur-micro) var(--ease-in);
  }
  .action:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }
  .action.on {
    color: var(--accent);
  }
  .action.on :global(svg) {
    fill: currentColor;
  }
  .action.danger:hover:not(:disabled) {
    color: var(--danger);
  }
</style>
