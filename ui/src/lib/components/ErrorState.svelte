<script lang="ts">
  let { message, onRetry }: { message: string; onRetry?: () => void } = $props();
</script>

<div class="error-state">
  <p style="color:#ff6b6b">{message}</p>
  {#if onRetry}
    <button onclick={onRetry}>Retry</button>
  {/if}
</div>
