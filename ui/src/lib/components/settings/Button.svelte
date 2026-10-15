<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon, { type IconName } from "../Icon.svelte";

  /// Settings button. Builds on the shared `.tool-btn` recipe (app.css) so it
  /// reads as the same object as every page toolbar, and so Rack skins it as a
  /// light-bar key; the variants only recolour it.
  let {
    variant = "secondary",
    icon,
    compact = true,
    children,
    class: extra = "",
    type = "button",
    ...rest
  }: {
    variant?: "secondary" | "primary" | "danger";
    icon?: IconName;
    compact?: boolean;
    children: Snippet;
  } & HTMLButtonAttributes = $props();
</script>

<button {type} class={["tool-btn", variant, compact && "compact", extra]} {...rest}>
  {#if icon}<Icon name={icon} size={15} />{/if}
  {@render children()}
</button>

<style>
  button.compact {
    padding: 6px 12px;
    font-size: 13px;
  }
  button.primary {
    border-color: transparent;
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
  }
  button.primary :global(svg) {
    color: inherit;
  }
  button.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 86%, var(--text));
  }
  button.danger {
    border-color: color-mix(in srgb, var(--danger) 45%, transparent);
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    color: var(--danger);
    font-weight: 600;
  }
  button.danger :global(svg) {
    color: inherit;
  }
  button.danger:hover:not(:disabled) {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 18%, transparent);
  }
  button:disabled {
    cursor: default;
  }
</style>
