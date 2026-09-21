<script lang="ts">
  import type { Nav } from "../nav.svelte";
  let { nav }: { nav: Nav } = $props();
</script>

<nav class="sidebar">
  <h1 style="font-size:16px">OxiTide</h1>
  <button onclick={() => nav.goto({ name: "home" })}>Home</button>
  <button onclick={() => nav.goto({ name: "search" })}>Search</button>
  <button onclick={() => nav.goto({ name: "library" })}>Library</button>
  <button onclick={() => nav.toggleQueue()}>Queue</button>
</nav>
