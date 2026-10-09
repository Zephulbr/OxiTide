<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";

  /// Runs `load` (again whenever `key` changes) and renders loading, error
  /// with Retry, or the result.
  let {
    load,
    key = "",
    children,
  }: { load: () => Promise<T>; key?: unknown; children: Snippet<[T, () => void]> } = $props();

  let result = $state<{ value: T } | null>(null);
  let error = $state("");
  let token = 0;

  async function run() {
    const mine = ++token;
    error = "";
    try {
      const value = await load();
      if (mine === token) result = { value };
    } catch (e) {
      if (mine === token) error = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    void key;
    result = null;
    void run();
  });
</script>

{#if error}
  <div class="state">
    <Icon name="alert-triangle" size={22} />
    <p>{error}</p>
    <button class="pill" onclick={run}><Icon name="rotate-cw" size={16} /> Retry</button>
  </div>
{:else if result}
  {@render children(result.value, run)}
{:else}
  <div class="state muted" aria-busy="true">Loading…</div>
{/if}

<style>
  .state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 40px 0;
    text-align: center;
  }
</style>
