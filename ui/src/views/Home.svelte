<script lang="ts">
  import { api, type HomeSection } from "../lib/api";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let sections = $state<HomeSection[]>([]);
  let error = $state("");

  $effect(() => {
    api.getHome().then((s) => (sections = s)).catch((e) => (error = String(e)));
  });
</script>

<h2>Home</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
{#each sections as section (section.title)}
  <h3>{section.title}</h3>
  <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:12px">
    {#each section.items as item (item.id)}
      <button style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit">
        <div>{item.title}</div>
        {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
      </button>
    {/each}
  </div>
{:else}
  <p class="muted">Loading…</p>
{/each}
