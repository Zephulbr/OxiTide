<script lang="ts">
  import type { Snippet } from "svelte";

  /// One setting: label + help text on the left, the control on the right.
  /// `stacked` drops the control onto its own line (sliders, wide editors).
  /// The row wraps by its own width, not the page's: when the control no
  /// longer fits beside the text it drops underneath, so the same row works
  /// in a wide single column, a half-width card, or a narrow window.
  /// `tag="label"` makes the whole row the click target of the single
  /// control inside it (toggle rows).
  let {
    label,
    description,
    stacked = false,
    disabled = false,
    tag = "div",
    children,
    below,
  }: {
    label: string;
    description?: string;
    stacked?: boolean;
    disabled?: boolean;
    tag?: "div" | "label";
    children?: Snippet;
    below?: Snippet;
  } = $props();
</script>

<svelte:element this={tag} class="row set-row" class:stacked class:disabled>
  <div class="text">
    <span class="label">{label}</span>
    {#if description}<span class="help">{description}</span>{/if}
  </div>
  {#if children}<div class="control">{@render children()}</div>{/if}
  {#if below}<div class="below">{@render below()}</div>{/if}
</svelte:element>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    align-content: center;
    align-items: center;
    gap: 8px 20px;
    padding: 13px 18px;
  }
  /* Rows come from separate component instances, so the sibling rule has to
     be global (the class is unique to this component). */
  :global(.set-row + .set-row) {
    border-top: 1px solid var(--line);
  }
  label.row {
    cursor: pointer;
  }
  label.row.disabled {
    cursor: default;
  }
  /* The text takes what the control leaves; below 240px it yields the line. */
  .text {
    display: flex;
    flex: 1 1 240px;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .label {
    font-weight: 550;
  }
  .help {
    max-width: 62ch;
    color: var(--text-muted);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .control {
    display: flex;
    flex: 0 1 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .below {
    flex: 1 1 100%;
    min-width: 0;
  }
  .row.stacked .control {
    flex: 1 1 100%;
  }
  .row.disabled .text {
    opacity: 0.5;
  }
</style>
