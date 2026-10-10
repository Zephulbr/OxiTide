<script lang="ts">
  import Loader from "../components/Loader.svelte";
  import Sections from "../components/Sections.svelte";
  import { useRemote } from "../context";

  let { source }: { source: "home" | "explore" | "foryou" } = $props();
  const { api } = useRemote();

  const TABS = [
    { id: "home", label: "Home" },
    { id: "foryou", label: "For You" },
    { id: "explore", label: "Explore" },
    { id: "mixes", label: "My Mixes" },
  ] as const;

  function load() {
    return source === "home" ? api.getHome() : source === "explore" ? api.getExplore() : api.getForYou();
  }
</script>

<div class="page">
  <nav class="chips" aria-label="Browse">
    {#each TABS as tab (tab.id)}
      <a href={`#/${tab.id}`} class:active={tab.id === source} aria-current={tab.id === source ? "page" : undefined}>
        {tab.label}
      </a>
    {/each}
  </nav>
  <Loader {load} key={source}>
    {#snippet children(sections)}
      <Sections {sections} />
    {/snippet}
  </Loader>
</div>

<style>
  .chips {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    margin: 0 calc(-1 * var(--remote-gutter)) 4px;
    padding: 4px var(--remote-gutter);
    scrollbar-width: none;
  }
  .chips a {
    flex: none;
    min-height: 36px;
    display: inline-flex;
    align-items: center;
    padding: 0 14px;
    border: 1px solid var(--line);
    border-radius: 999px;
    color: var(--text-muted);
    font-weight: 600;
    font-size: 14px;
  }
  .chips a.active {
    border-color: transparent;
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
