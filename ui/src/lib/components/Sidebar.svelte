<script lang="ts">
  import type { Nav } from "../nav.svelte";
  let { nav, onLogout }: { nav: Nav; onLogout: () => void } = $props();
</script>

<nav class="sidebar">
  <h1 style="font-size:16px">OxiTide</h1>
  <button onclick={() => nav.goto({ name: "home" })}>Home</button>
  <button onclick={() => nav.goto({ name: "search" })}>Search</button>
  <button onclick={() => nav.goto({ name: "library" })}>Library</button>
  <button onclick={() => nav.toggleQueue()}>Queue</button>
  <button onclick={() => nav.goto({ name: "settings" })}>Settings</button>
  <button onclick={onLogout}>Log out</button>
</nav>
