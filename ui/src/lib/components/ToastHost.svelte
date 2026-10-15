<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Toasts } from "../toasts.svelte";
  let { toasts }: { toasts: Toasts } = $props();

  function severityIcon(severity: string): "check" | "alert-triangle" | "info" {
    if (severity === "success") return "check";
    if (severity === "error") return "alert-triangle";
    return "info";
  }
</script>

{#if toasts.toasts.length > 0}
  <div class="toast-host" aria-live="polite">
    {#each toasts.toasts as toast (toast.id)}
      <div
        class="toast"
        class:error={toast.severity === "error"}
        class:success={toast.severity === "success"}
        role={toast.severity === "error" ? "alert" : undefined}
      >
        <span class="glyph" class:error={toast.severity === "error"} class:success={toast.severity === "success"}>
          <Icon name={severityIcon(toast.severity)} size={16} />
        </span>
        <span class="message">{toast.message}</span>
        {#if toast.action}
          <button
            onclick={() => {
              toast.action?.run();
              toasts.dismiss(toast.id);
            }}>{toast.action.label}</button>
        {/if}
        <button class="dismiss" onclick={() => toasts.dismiss(toast.id)} aria-label="Dismiss"><Icon name="x" size={14} /></button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-host {
    position: fixed;
    /* Below the custom caption band (window buttons), never over it. */
    top: calc(var(--titlebar-h) + 8px);
    right: 12px;
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: min(360px, 80vw);
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background:
      linear-gradient(var(--surface-3), var(--surface-3)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
    animation: toast-in var(--dur-view) cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  /* Severity lives on the glyph only — no stripes (spec §8/§14). */
  .glyph {
    display: inline-grid;
    place-items: center;
    color: var(--text-muted);
  }
  .glyph.error {
    color: var(--danger);
  }
  .glyph.success {
    color: var(--success);
  }
  .message {
    flex: 1;
    overflow-wrap: anywhere;
  }
  .dismiss {
    margin-left: auto;
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(-8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
