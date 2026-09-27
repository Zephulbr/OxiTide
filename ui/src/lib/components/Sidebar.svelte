<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Nav } from "../nav.svelte";

  let {
    nav,
    rail,
    onToggleRail,
    onLogout,
  }: { nav: Nav; rail: boolean; onToggleRail: () => void; onLogout: () => void } = $props();
</script>

<nav class="sidebar" class:rail>
  <div class="top">
    <h1 class="wordmark">{rail ? "O" : "OxiTide"}</h1>
    <button
      class="collapse"
      onclick={onToggleRail}
      aria-label={rail ? "Expand sidebar" : "Collapse sidebar"}
      title={rail ? "Expand sidebar" : "Collapse sidebar"}
    ><Icon name={rail ? "panel-left-open" : "panel-left-close"} size={16} /></button>
  </div>

  <div class="history">
    <button disabled={!nav.canBack} onclick={() => nav.back()} title="Back (Alt+←)" aria-label="Back"><Icon name="chevron-left" /></button>
    <button disabled={!nav.canForward} onclick={() => nav.forward()} title="Forward (Alt+→)" aria-label="Forward"><Icon name="chevron-right" /></button>
  </div>

  <button class="nav-item" class:active={nav.route.name === "home"} onclick={() => nav.goto({ name: "home" })} title="Home" aria-label="Home">
    <Icon name="home" />{#if !rail}<span>Home</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "search"} onclick={() => nav.goto({ name: "search" })} title="Search" aria-label="Search">
    <Icon name="search" />{#if !rail}<span>Search</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "library"} onclick={() => nav.goto({ name: "library" })} title="Library" aria-label="Library">
    <Icon name="library" />{#if !rail}<span>Library</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.queueOpen} onclick={() => nav.toggleQueue()} title="Queue" aria-label="Queue">
    <Icon name="list-music" />{#if !rail}<span>Queue</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "settings"} onclick={() => nav.goto({ name: "settings" })} title="Settings" aria-label="Settings">
    <Icon name="settings" />{#if !rail}<span>Settings</span>{/if}
  </button>

  <div class="spacer"></div>

  <button class="nav-item" onclick={onLogout} title="Log out" aria-label="Log out">
    <Icon name="log-out" />{#if !rail}<span>Log out</span>{/if}
  </button>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .wordmark {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 640;
    margin: 0;
    letter-spacing: -0.02em;
  }
  .history {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
  }
  .nav-item:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav-item.active {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }
  .spacer {
    flex: 1;
  }
  .rail .nav-item {
    justify-content: center;
    padding: 9px 0;
  }
  .rail .top {
    justify-content: center;
  }
</style>
