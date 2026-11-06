<script lang="ts">
  import FavoriteHeart, { type HeartFx } from "./FavoriteHeart.svelte";
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
    heartFx = null,
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
    /// A favorite toggle's last change; with `icon="heart"` the heart is
    /// FavoriteHeart (it beats on hover, pops on favorite, breaks on
    /// unfavorite).
    heartFx?: HeartFx | null;
  } = $props();
</script>

<button
  class="action"
  class:on={active}
  class:fill
  class:danger
  class:heart={icon === "heart"}
  {title}
  {disabled}
  {onclick}
>
  {#if icon === "heart"}
    <FavoriteHeart size={20} fx={heartFx} />
  {:else}
    <Icon name={icon} size={20} />
  {/if}
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
  .action.on,
  .action.heart:hover:not(:disabled) {
    color: var(--accent);
  }
  .action.on.fill :global(svg) {
    fill: currentColor;
  }
  .action.danger:hover:not(:disabled) {
    color: var(--danger);
  }
</style>
