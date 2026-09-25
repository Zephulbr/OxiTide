<script lang="ts">
  let { label = "Loading…" }: { label?: string } = $props();
</script>

<span class="loading" role="status">
  <span class="spinner" aria-hidden="true"></span>
  {label}
</span>

<style>
  .loading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--line);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
