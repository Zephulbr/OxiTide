<script lang="ts">
  import { untrack } from "svelte";
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import Icon from "./Icon.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    nav,
    rail,
    onToggleRail,
    onLogout,
    collection,
    playlistActions,
    actions,
  }: {
    nav: Nav;
    rail: boolean;
    onToggleRail: () => void;
    onLogout: () => void;
    collection: CollectionTree;
    playlistActions: PlaylistActions;
    actions: CollectionActions;
  } = $props();

  let collectionOpen = $state(true);
  let showMore = $state(false);

  let revisionSeen = untrack(() => playlistActions.revision);

  $effect(() => {
    const revision = playlistActions.revision;
    if (revision === revisionSeen) return;
    revisionSeen = revision;
    untrack(() => void collection.refresh());
  });

  const collectionItems: { route: Route; label: string; icon: "list-music" | "disc-3" | "music" }[] = [
    { route: { name: "playlists" }, label: "Playlists", icon: "list-music" },
    { route: { name: "albums" }, label: "Albums", icon: "disc-3" },
    { route: { name: "tracks" }, label: "Tracks", icon: "music" },
  ];

  const moreItems: {
    route: Route;
    label: string;
    icon: "disc-3" | "heart" | "copy";
  }[] = [
    { route: { name: "mixes" }, label: "Mixes & Radio", icon: "disc-3" },
    { route: { name: "profiles" }, label: "Profiles", icon: "heart" },
    { route: { name: "purchases" }, label: "Purchases", icon: "copy" },
  ];

  function isActive(name: string): boolean {
    if (name === "playlists") {
      return nav.route.name === "playlists" || nav.route.name === "folder";
    }
    return nav.route.name === name;
  }
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

  {#if !rail}
    <div class="history">
      <button disabled={!nav.canBack} onclick={() => nav.back()} title="Back (Alt+←)" aria-label="Back"><Icon name="chevron-left" /></button>
      <button disabled={!nav.canForward} onclick={() => nav.forward()} title="Forward (Alt+→)" aria-label="Forward"><Icon name="chevron-right" /></button>
    </div>
  {/if}

  <button class="nav-item" class:active={nav.route.name === "home"} aria-current={nav.route.name === "home" ? "page" : undefined} onclick={() => nav.goto({ name: "home" })} title="Home" aria-label="Home">
    <Icon name="home" />{#if !rail}<span>Home</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "explore"} aria-current={nav.route.name === "explore" ? "page" : undefined} onclick={() => nav.goto({ name: "explore" })} title="Explore" aria-label="Explore">
    <Icon name="globe" />{#if !rail}<span>Explore</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "feed"} aria-current={nav.route.name === "feed" ? "page" : undefined} onclick={() => nav.goto({ name: "feed" })} title="Feed" aria-label="Feed">
    <Icon name="bell" />{#if !rail}<span>Feed</span>{/if}
  </button>

  <div class="divider" role="separator"></div>

  {#if rail}
    <button class="nav-item" class:active={isActive("playlists")} aria-current={isActive("playlists") ? "page" : undefined} onclick={() => nav.goto({ name: "playlists" })} title="My Collection" aria-label="My Collection">
      <Icon name="library" />
    </button>
  {:else}
    <button class="section-head" onclick={() => (collectionOpen = !collectionOpen)} aria-expanded={collectionOpen}>
      <span>My Collection</span>
      <Icon name={collectionOpen ? "chevron-down" : "chevron-right"} size={14} />
    </button>
  {/if}

  {#if collectionOpen || rail}
    {#each collectionItems as item (item.label)}
      <button class="nav-item" class:active={isActive(item.route.name)} aria-current={isActive(item.route.name) ? "page" : undefined} onclick={() => nav.goto(item.route)} title={item.label} aria-label={item.label}>
        <Icon name={item.icon} />{#if !rail}<span>{item.label}</span>{/if}
      </button>
    {/each}
    {#if rail}
      <button class="nav-item" onclick={() => (showMore = !showMore)} aria-expanded={showMore} title="Show more" aria-label="Show more">
        <Icon name={showMore ? "chevron-up" : "chevron-down"} />
      </button>
    {:else}
      <button class="section-head sub" onclick={() => (showMore = !showMore)} aria-expanded={showMore}>
        <span>{showMore ? "Show less" : "Show more"}</span>
        <Icon name={showMore ? "chevron-up" : "chevron-down"} size={14} />
      </button>
    {/if}
    {#if showMore}
      {#each moreItems as item (item.label)}
        <button class="nav-item" class:active={isActive(item.route.name)} aria-current={isActive(item.route.name) ? "page" : undefined} onclick={() => nav.goto(item.route)} title={item.label} aria-label={item.label}>
          <Icon name={item.icon} />{#if !rail}<span>{item.label}</span>{/if}
        </button>
      {/each}
    {/if}
  {/if}

  <div class="divider" role="separator"></div>

  <div class="playlist-head">
    {#if !rail}<span class="playlist-title">Playlists</span>{/if}
    <button class="mini" onclick={() => playlistActions.openCreate()} title="New playlist" aria-label="New playlist">
      <Icon name="plus" size={14} />
    </button>
  </div>
  <div class="tree">
    {#if collection.error}
      <p class="muted tree-error">{collection.error}</p>
    {:else}
      <PlaylistTree entries={collection.root} {collection} {nav} {actions} {rail} />
    {/if}
  </div>

  <div class="spacer"></div>

  <button class="nav-item" class:active={nav.route.name === "settings"} aria-current={nav.route.name === "settings" ? "page" : undefined} onclick={() => nav.goto({ name: "settings" })} title="Settings" aria-label="Settings">
    <Icon name="settings" />{#if !rail}<span>Settings</span>{/if}
  </button>
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
    transition: opacity var(--dur-micro) var(--ease-in);
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
  .divider {
    height: 1px;
    background: var(--line);
    margin: 8px 4px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 7px 8px;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .section-head:hover {
    color: var(--text);
  }
  .section-head.sub {
    font-weight: 500;
    text-transform: none;
    letter-spacing: normal;
    padding-left: 32px;
  }
  .playlist-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 8px 3px;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .mini:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .tree {
    min-height: 0;
  }
  .tree-error {
    padding: 0 8px;
    font-size: 11.5px;
  }
  .spacer {
    flex: 1;
    min-height: 12px;
  }
  .rail .nav-item {
    justify-content: center;
    padding: 9px 0;
  }
  .rail .top {
    position: relative;
    justify-content: center;
  }
  .rail .collapse {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    color: var(--text-muted);
  }
  .rail .collapse:hover,
  .rail .collapse:focus-visible {
    opacity: 1;
    color: var(--text);
  }
  .rail .top:has(.collapse:hover, .collapse:focus-visible) .wordmark {
    opacity: 0;
  }
  .rail .playlist-head {
    justify-content: center;
    padding: 4px 0;
  }
  .rail .tree :global(.row) {
    justify-content: center;
  }
</style>
