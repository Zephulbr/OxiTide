<script lang="ts">
  import type { PageSection } from "../api";
  import { routeForPageItem, type Nav } from "../nav.svelte";
  import Cover from "./Cover.svelte";

  let {
    sections,
    nav,
  }: {
    sections: PageSection[];
    nav: Nav;
  } = $props();
</script>

{#each sections as section, si (`${section.title}-${si}`)}
  <h3>{section.title}</h3>
  <div class="card-grid">
    {#each section.items as item, i (item.id || `${item.title}-${i}`)}
      {@const route = routeForPageItem(item)}
      {#if route}
        <button class="card" onclick={() => nav.goto(route)}>
          <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
          <span class="card-title">{item.title}</span>
          {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
        </button>
      {:else}
        <div class="card inert">
          <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
          <span class="card-title">{item.title}</span>
          {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
        </div>
      {/if}
    {/each}
  </div>
{/each}

<style>
  .card-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .card-art {
    display: block;
    overflow: hidden;
    border-radius: var(--radius-card);
  }
  .card-art :global(img) {
    transition: transform var(--dur-state) var(--ease-in);
  }
  .card:hover .card-art :global(img) {
    transform: scale(1.03);
  }
  .card-title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card-sub {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card.inert {
    opacity: 0.9;
  }
</style>
