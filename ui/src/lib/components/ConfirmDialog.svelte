<script lang="ts">
  import Modal from "./Modal.svelte";
  let {
    title,
    message,
    onconfirm,
    oncancel,
  }: { title: string; message: string; onconfirm: () => void; oncancel: () => void } = $props();
</script>

<Modal {title} onclose={oncancel}>
  <p>{message}</p>
  <p class="actions">
    <button class="cancel" onclick={oncancel}>Cancel</button>
    <button class="confirm" onclick={onconfirm}>Confirm</button>
  </p>
</Modal>

<style>
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 16px 0 0;
  }
  .actions button {
    padding: 8px 16px;
    border-radius: var(--radius-control);
    font-weight: 600;
  }
  .cancel {
    border: 1px solid var(--line);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .cancel:hover {
    color: var(--text);
  }
  /* Destructive confirm, matching the Settings danger-button treatment. */
  .confirm {
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    color: var(--danger);
    transition:
      background var(--dur-state) var(--ease-in),
      border-color var(--dur-state) var(--ease-in),
      box-shadow var(--dur-state) var(--ease-in),
      transform var(--dur-micro) var(--ease-in);
  }
  .confirm:hover {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    border-color: var(--danger);
    box-shadow: 0 0 9px color-mix(in srgb, var(--danger) 80%, transparent);
  }
  .confirm:active {
    transform: scale(0.97);
  }
</style>
