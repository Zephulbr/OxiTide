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
    <button onclick={oncancel}>Cancel</button>
    <button onclick={onconfirm}>Confirm</button>
  </p>
</Modal>

<style>
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
