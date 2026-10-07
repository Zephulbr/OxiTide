<script lang="ts">
  import { untrack } from "svelte";
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menu, type MenuItem } from "../menu.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import { playlistView } from "../playlistView.svelte";
  import { playlistViewMenuItems } from "../playlistViewMenu";
  import Icon from "./Icon.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    nav,
    rail,
    onToggleRail,
    collection,
    playlistActions,
    actions,
  }: {
    nav: Nav;
    rail: boolean;
    onToggleRail: () => void;
    collection: CollectionTree;
    playlistActions: PlaylistActions;
    actions: CollectionActions;
  } = $props();

  let collectionOpen = $state(true);
  let showMore = $state(false);

  const view = playlistView();

  function showAtButton(event: MouseEvent, items: MenuItem[]) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu.show(rect.left, rect.bottom + 4, items, event.currentTarget as HTMLElement);
  }

  function openViewMenu(event: MouseEvent) {
    showAtButton(
      event,
      playlistViewMenuItems(view.prefs, {
        setSort: (key) => view.setSort(key),
        setFilter: (filter) => view.setFilter(filter),
      }),
    );
  }

  function openCreateMenu(event: MouseEvent) {
    showAtButton(event, [
      {
        id: "new-playlist",
        label: "New Playlist",
        run: () => playlistActions.openCreate(),
      },
      {
        id: "new-folder",
        label: "New Folder",
        run: () => actions.openCreateFolder("root"),
      },
    ]);
  }

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
  <div class="scroll">
    <div class="top">
      <h1 class="wordmark">{rail ? "O" : "OxiTide"}</h1>
      <div class="top-actions">
        <button
          class="collapse"
          onclick={onToggleRail}
          aria-label={rail ? "Expand sidebar" : "Collapse sidebar"}
          title={rail ? "Expand sidebar" : "Collapse sidebar"}
        ><Icon name={rail ? "panel-left-open" : "panel-left-close"} size={18} /></button>
      </div>
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
    <button class="nav-item" class:active={nav.route.name === "history"} aria-current={nav.route.name === "history" ? "page" : undefined} onclick={() => nav.goto({ name: "history", tab: "recent" })} title="History" aria-label="History">
      <Icon name="history" />{#if !rail}<span>History</span>{/if}
    </button>
    <button class="nav-item" class:active={nav.route.name === "settings"} aria-current={nav.route.name === "settings" ? "page" : undefined} onclick={() => nav.goto({ name: "settings" })} title="Settings" aria-label="Settings">
      <Icon name="settings" />{#if !rail}<span>Settings</span>{/if}
    </button>

    <div class="divider" role="separator"></div>

    {#if rail}
      <button class="nav-item" class:active={isActive("playlists")} aria-current={isActive("playlists") ? "page" : undefined} onclick={() => nav.goto({ name: "playlists" })} title="My Collection" aria-label="My Collection">
        <Icon name="library" />
      </button>
    {:else}
      <button class="section-head" onclick={() => (collectionOpen = !collectionOpen)} aria-expanded={collectionOpen}>
        <span>My Collection</span>
        <Icon name={collectionOpen ? "chevron-down" : "chevron-right"} size={16} />
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
          <Icon name={showMore ? "chevron-up" : "chevron-down"} size={16} />
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

    {#if !rail}
      <div class="playlist-head">
        <span class="playlist-title">Playlists</span>
        <button
          class="mini"
          onclick={openCreateMenu}
          title="New playlist or folder"
          aria-label="New playlist or folder"
        >
          <Icon name="plus" size={16} />
        </button>
        <button
          class="mini"
          onclick={openViewMenu}
          title="Sort & filter playlists"
          aria-label="Sort and filter playlists"
        >
          <Icon name="sliders-horizontal" size={16} />
        </button>
      </div>
    {/if}
    <div class="tree">
      {#if collection.error}
        <p class="muted tree-error">{collection.error}</p>
      {:else}
        <PlaylistTree entries={collection.root} {collection} {nav} {actions} {rail} />
      {/if}
    </div>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: calc(var(--font-body-size) + 1px);
  }
  /* Nav + playlist tree share one scroll area; the tree must not shrink as a
     flex item or its rows spill out of the box (the old footer bug). */
  .scroll {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    /* Bleed over the sidebar padding so scrolled rows span edge to edge,
       while the inner padding keeps the 12px gutters. The bottom clearance
       lets the playlist tree scroll clear of the floating player bar. */
    margin: -12px -12px 0;
    padding: 12px 12px calc(var(--player-height) + 12px);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .top-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .wordmark {
    font-family: var(--font-display);
    font-size: 22px;
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
    gap: 11px;
    width: 100%;
    padding: 8px 9px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
  }
  .nav-item :global(svg) {
    width: 20px;
    height: 20px;
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
    padding: 8px 9px;
    color: var(--text-muted);
    font-size: 12.5px;
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
    gap: 2px;
    padding: 8px 9px 3px;
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .playlist-title {
    flex: 1;
    min-width: 0;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .mini:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .tree {
    flex-shrink: 0;
  }
  .tree-error {
    padding: 0 9px;
    font-size: 12.5px;
  }
  .rail .nav-item {
    justify-content: center;
    padding: 10px 0;
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
  .rail .tree :global(.row) {
    justify-content: center;
  }
</style>
