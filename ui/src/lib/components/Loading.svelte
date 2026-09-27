<script lang="ts">
  let { label = "Loading…" }: { label?: string } = $props();
</script>

<span class="loading" role="status">
  <span class="disc" aria-hidden="true"></span>
  {label}
</span>

<style>
  .loading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
  }
  /* Disc spinner (spec §8/§10.2): a small record with an accent tick. */
  .disc {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--line);
    position: relative;
    animation: spin 1.2s linear infinite;
  }
  .disc::after {
    content: "";
    position: absolute;
    left: calc(50% - 1px);
    top: 1px;
    width: 2px;
    height: 5px;
    background: var(--accent);
    border-radius: 1px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
