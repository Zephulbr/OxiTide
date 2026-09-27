<script lang="ts">
  let { message, onRetry }: { message: string; onRetry?: () => void } = $props();
</script>

<div class="error-state">
  <p style="color:var(--danger)">{message}</p>
  {#if onRetry}
    <button onclick={onRetry}>Retry</button>
  {/if}
</div>
