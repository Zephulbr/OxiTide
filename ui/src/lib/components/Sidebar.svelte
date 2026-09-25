<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Nav } from "../nav.svelte";
  let { nav, onLogout }: { nav: Nav; onLogout: () => void } = $props();
</script>

<nav class="sidebar">
  <h1 style="font-size:16px">OxiTide</h1>
  <div class="history">
    <button disabled={!nav.canBack} onclick={() => nav.back()} title="Back (Alt+←)" aria-label="Back"><Icon name="chevron-left" /></button>
    <button disabled={!nav.canForward} onclick={() => nav.forward()} title="Forward (Alt+→)" aria-label="Forward"><Icon name="chevron-right" /></button>
  </div>
  <button onclick={() => nav.goto({ name: "home" })}>Home</button>
  <button onclick={() => nav.goto({ name: "search" })}>Search</button>
  <button onclick={() => nav.goto({ name: "library" })}>Library</button>
  <button onclick={() => nav.toggleQueue()}>Queue</button>
  <button onclick={() => nav.goto({ name: "settings" })}>Settings</button>
  <button onclick={onLogout}>Log out</button>
</nav>

<style>
  .history {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
</style>
