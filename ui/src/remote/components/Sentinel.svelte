<script lang="ts">
  /// Calls `onvisible` whenever the end of a list comes within `margin` of the
  /// viewport (infinite scroll). The caller guards against overlapping loads.
  /// `armed` changes (e.g. the row count) re-observe, so a page too short to
  /// push the end out of range still triggers the next load.
  let {
    onvisible,
    margin = "800px",
    armed = 0,
  }: { onvisible: () => void; margin?: string; armed?: unknown } = $props();

  let node = $state<HTMLDivElement | null>(null);

  $effect(() => {
    void armed;
    if (!node || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) onvisible();
      },
      { rootMargin: `0px 0px ${margin} 0px` },
    );
    observer.observe(node);
    return () => observer.disconnect();
  });
</script>

<div bind:this={node} class="sentinel" aria-hidden="true"></div>

<style>
  .sentinel {
    height: 1px;
  }
</style>
