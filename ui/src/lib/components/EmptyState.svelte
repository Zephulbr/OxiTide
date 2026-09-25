<script lang="ts">
  let {
    message,
    action,
  }: { message: string; action?: { label: string; run(): void } } = $props();
</script>

<div class="empty-state">
  <p class="muted">{message}</p>
  {#if action}<button onclick={action.run}>{action.label}</button>{/if}
</div>
