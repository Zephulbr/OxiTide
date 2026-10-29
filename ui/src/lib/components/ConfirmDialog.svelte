<script lang="ts">
  import Modal from "./Modal.svelte";
  let {
    title,
    message,
    confirmLabel = "Confirm",
    destructive = true,
    onconfirm,
    oncancel,
  }: {
    title: string;
    message: string;
    confirmLabel?: string;
    /// `false` draws the confirm in the accent instead of the danger colour.
    destructive?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<Modal {title} onclose={oncancel}>
  <p>{message}</p>
  <p class="actions">
    <button class="cancel" onclick={oncancel}>Cancel</button>
    <button class="confirm" class:safe={!destructive} onclick={onconfirm}>{confirmLabel}</button>
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
      background var(--dur-state) var(--ease-enter),
      border-color var(--dur-state) var(--ease-enter),
      box-shadow var(--dur-state) var(--ease-enter),
      transform var(--dur-micro) var(--ease-enter);
  }
  .confirm:hover {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    border-color: var(--danger);
    box-shadow: 0 0 9px color-mix(in srgb, var(--danger) 80%, transparent);
  }
  .confirm:active {
    transform: scale(0.97);
  }
  .confirm.safe {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    color: var(--accent);
  }
  .confirm.safe:hover {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border-color: var(--accent);
    box-shadow: 0 0 9px color-mix(in srgb, var(--accent) 80%, transparent);
  }
</style>
