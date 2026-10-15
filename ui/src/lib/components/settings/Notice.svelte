<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "../Icon.svelte";

  /// An inline callout inside a settings page. `info` explains, `warning`
  /// flags something to fix, `success` confirms.
  let {
    kind = "info",
    children,
  }: {
    kind?: "info" | "warning" | "success";
    children: Snippet;
  } = $props();

  const ICONS: Record<string, IconName> = {
    info: "info",
    warning: "alert-triangle",
    success: "circle-check",
  };
</script>

<div class="notice {kind}" role={kind === "warning" ? "alert" : "status"}>
  <Icon name={ICONS[kind]} size={16} />
  <div class="text">{@render children()}</div>
</div>

<style>
  .notice {
    --tone: var(--text-muted);
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin: 0 0 16px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--tone) 40%, transparent);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--tone) 9%, transparent);
    color: var(--text);
    font-size: 13px;
    line-height: 1.5;
  }
  .notice.info {
    --tone: var(--accent);
  }
  .notice.warning {
    --tone: var(--warning);
  }
  .notice.success {
    --tone: var(--success);
  }
  .notice :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--tone);
  }
  .text {
    min-width: 0;
  }
</style>
