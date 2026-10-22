<script lang="ts">
  import { prefetchIntent } from "../viewData";
  import { untrack } from "svelte";
  import { appearance } from "../appearance.svelte";
  import { createHoverIntent } from "../hoverIntent.svelte";
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menu, type MenuItem } from "../menu.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import { playlistView } from "../playlistView.svelte";
  import { playlistViewMenuItems } from "../playlistViewMenu";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    nav,
    rail: collapsed,
    hoverExpand = false,
    onToggleRail,
    collection,
    playlistActions,
    actions,
    playing = false,
  }: {
    nav: Nav;
    /// Collapsed to the icon rail (the user's choice or a narrow window).
    rail: boolean;
    /// ui.hover_panels: hovering the collapsed rail expands it as an overlay.
    hoverExpand?: boolean;
    /// Playback is running (spins the playing playlist's mark).
    playing?: boolean;
    onToggleRail: () => void;
    collection: CollectionTree;
    playlistActions: PlaylistActions;
    actions: CollectionActions;
  } = $props();

  // Hover-expand: the rail opens to full width over the page (its grid column
  // stays rail-wide, so nothing reflows) and plays a short reverse on close.
  // A context menu opened from it holds it open until the menu closes.
  const PEEK_OUT_MS = 140;
  const peekIntent = createHoverIntent({ isHeld: () => menu.open !== null });
  const canPeek = $derived(collapsed && hoverExpand);
  let closing = $state(false);
  const peek = $derived(canPeek && (peekIntent.open || closing));
  const rail = $derived(collapsed && !peek);

  $effect(() => {
    if (!canPeek) untrack(() => peekIntent.reset());
  });

  // Pre-DOM, so the close starts in the same update that drops the open
  // state; never a rail frame (its collapse button over the logo) in between.
  let wasOpen = false;
  $effect.pre(() => {
    const open = peekIntent.open;
    const animate = wasOpen && !open && canPeek && !appearance.reducedMotion;
    wasOpen = open;
    if (!animate) return;
    closing = true;
    const timer = setTimeout(() => (closing = false), PEEK_OUT_MS);
    return () => {
      clearTimeout(timer);
      closing = false;
    };
  });

  function onPointerEnter() {
    // Back in mid-close: reopen at once rather than snapping to the rail first.
    if (canPeek) peekIntent.enter(closing);
  }

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
        icon: "list-plus",
        run: () => playlistActions.openCreate(),
      },
      {
        id: "new-folder",
        label: "New Folder",
        icon: "folder-plus",
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

<nav
  class="sidebar"
  class:rail
  class:peek
  class:closing
  onpointerenter={onPointerEnter}
  onpointerleave={() => peekIntent.leave()}
>
  <div class="scroll">
    <div class="top">
      <h1 class="wordmark" aria-label="OxiTide">
        <Logo size={28} />{#if !rail}OxiTide{/if}
      </h1>
      <div class="top-actions">
        <!-- Labelled from the collapsed state: on a hover-expanded rail it pins
             the sidebar open. -->
        <button
          class="collapse"
          onclick={onToggleRail}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          title={collapsed ? "Expand sidebar (Ctrl+B)" : "Collapse sidebar (Ctrl+B)"}
        ><Icon name={collapsed ? "panel-left-open" : "panel-left-close"} size={18} /></button>
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
    <button class="nav-item" class:active={nav.route.name === "feed"} aria-current={nav.route.name === "feed" ? "page" : undefined} onclick={() => nav.goto({ name: "feed" })} use:prefetchIntent={{ name: "feed" }} title="Feed" aria-label="Feed">
      <Icon name="bell" />{#if !rail}<span>Feed</span>{/if}
    </button>
    <button class="nav-item" class:active={nav.route.name === "history"} aria-current={nav.route.name === "history" ? "page" : undefined} onclick={() => nav.goto({ name: "history", tab: "stats" })} title="History" aria-label="History">
      <Icon name="history" />{#if !rail}<span>History</span>{/if}
    </button>
    <button class="nav-item" class:active={nav.route.name === "settings"} aria-current={nav.route.name === "settings" ? "page" : undefined} onclick={() => nav.goto({ name: "settings" })} title="Settings" aria-label="Settings">
      <Icon name="settings" />{#if !rail}<span>Settings</span>{/if}
    </button>

    <div class="divider" role="separator"></div>

    {#if rail}
      <button class="nav-item" class:active={isActive("playlists")} aria-current={isActive("playlists") ? "page" : undefined} onclick={() => nav.goto({ name: "playlists" })} use:prefetchIntent={{ name: "playlists" }} title="My Collection" aria-label="My Collection">
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
        <button class="nav-item" class:active={isActive(item.route.name)} aria-current={isActive(item.route.name) ? "page" : undefined} onclick={() => nav.goto(item.route)} use:prefetchIntent={item.route} title={item.label} aria-label={item.label}>
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
          <button class="nav-item" class:active={isActive(item.route.name)} aria-current={isActive(item.route.name) ? "page" : undefined} onclick={() => nav.goto(item.route)} use:prefetchIntent={item.route} title={item.label} aria-label={item.label}>
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
        <PlaylistTree entries={collection.root} {collection} {nav} {actions} {rail} {playing} />
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
       lets the playlist tree scroll clear of the floating player bar
       (none when the bar floats over the main column only, app.css). */
    margin: -12px -12px 0;
    padding: 12px 12px calc(var(--sidebar-clear, var(--player-height)) + 12px);
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
    display: flex;
    align-items: center;
    gap: 9px;
    /* The wordmark is brand lettering: always Manrope, whatever the
       Typeface pick. */
    font-family: "Manrope", var(--font-ui);
    font-size: 21px;
    font-weight: 800;
    margin: 0;
    /* As tall as the mark, with or without the name beside it, so the mark
       never moves vertically as the sidebar collapses or expands. */
    line-height: 28px;
    /* The rail's centred spot for the mark ((40px - 28px) / 2), so it never
       moves as the sidebar collapses, expands or peeks. */
    padding-left: 6px;
    letter-spacing: 0.01em;
    transition: opacity var(--dur-micro) var(--ease-enter);
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
  .rail .wordmark {
    padding-left: 0;
  }
  .rail .collapse {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    color: var(--text-muted);
    /* Hides at once: the button lands here visible when the sidebar collapses
       (Ctrl+B, the button, a hover close), and app.css's opacity fade would
       show it over the logo. Only revealing it fades. */
    transition: none;
  }
  .rail .collapse:hover,
  .rail .collapse:focus-visible {
    opacity: 1;
    color: var(--text);
    transition:
      opacity var(--dur-micro),
      color var(--dur-micro);
  }
  .rail .top:has(.collapse:hover, .collapse:focus-visible) .wordmark {
    opacity: 0;
  }
  .rail .tree :global(.row) {
    justify-content: center;
  }
  /* Hover-expanded rail: wider than its grid track, so it overlays the page.
     It stops at the player bar (like the floating queue) instead of covering
     the bar's left end. The rail itself widens from its 64px (app.css) to
     full width; its contents are laid out once at the full width and clipped
     by the moving edge (the shell's overflow: hidden), so only the shell's
     own box changes per frame and labels never wrap mid-glide. */
  .sidebar.peek {
    position: relative;
    z-index: 15;
    width: var(--sidebar-expanded-width);
    height: calc(100% - var(--player-height));
    align-self: start;
    box-shadow: 12px 0 32px rgba(0, 0, 0, 0.45);
    animation: peek-in var(--dur-state) var(--ease-enter);
  }
  .sidebar.peek.closing {
    animation: peek-out 140ms var(--ease-exit) forwards;
  }
  .sidebar.peek .scroll {
    /* The stretched width of an expanded sidebar's scroll area (less the
       shell's 1px border), held while the shell is narrower. */
    width: calc(var(--sidebar-expanded-width) - 1px);
    padding-bottom: 12px;
  }
  @keyframes peek-in {
    from {
      width: 64px;
    }
  }
  @keyframes peek-out {
    to {
      width: 64px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sidebar.peek,
    .sidebar.peek.closing {
      animation: none;
    }
  }
</style>
