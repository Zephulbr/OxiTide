<script lang="ts">
  let {
    message,
    action,
  }: { message: string; action?: { label: string; run(): void } } = $props();
</script>

<div class="empty-state">
  <h2 class="empty-title">{message}</h2>
  {#if action}<button onclick={action.run}>{action.label}</button>{/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  .empty-title {
    color: var(--text-muted);
    margin: 0;
  }
</style>
