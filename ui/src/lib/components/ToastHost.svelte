<script lang="ts">
  import type { Toasts } from "../toasts.svelte";
  let { toasts }: { toasts: Toasts } = $props();
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
        <span class="message">{toast.message}</span>
        {#if toast.action}
          <button
            onclick={() => {
              toast.action?.run();
              toasts.dismiss(toast.id);
            }}>{toast.action.label}</button>
        {/if}
        <button class="dismiss" onclick={() => toasts.dismiss(toast.id)} aria-label="Dismiss">✕</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-host {
    position: fixed;
    top: 12px;
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
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
    box-shadow: 0 4px 12px rgb(0 0 0 / 0.35);
  }
  .toast.error {
    border-color: #ff6b6b;
    color: #ff6b6b;
    align-items: flex-start;
  }
  .toast.success {
    border-color: var(--accent);
  }
  .message {
    flex: 1;
    overflow-wrap: anywhere;
  }
  .dismiss {
    margin-left: auto;
  }
</style>
