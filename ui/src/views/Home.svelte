<script lang="ts">
  import { api, type HomeSection } from "../lib/api";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let sections = $state<HomeSection[]>([]);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    loading = true;
    error = "";
    api
      .getHome()
      .then((s) => {
        sections = s;
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        loading = false;
      });
  });
</script>

<h2>Home</h2>
{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <p style="color:#ff6b6b">{error}</p>
{:else if sections.length === 0}
  <p class="muted">Nothing to show yet.</p>
{:else}
  {#each sections as section, si (`${section.title}-${si}`)}
    <h3>{section.title}</h3>
    <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:12px">
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        <button style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit">
          <div>{item.title}</div>
          {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
        </button>
      {/each}
    </div>
  {/each}
{/if}
