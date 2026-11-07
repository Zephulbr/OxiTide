<script lang="ts">
  import { prefetchIntent } from "../viewData";
  import { tick, untrack } from "svelte";
  import {
    COLLECTION_ITEMS,
    collectionLayout,
    hiddenKeys,
    toRows,
    visibleKeys,
    type CollectionKey,
    type LayoutRow,
  } from "../collectionLayout.svelte";
  import { DRAG_THRESHOLD, dragTarget, rowShift } from "../queueDrag";
  import { appearance } from "../appearance.svelte";
  import { createHoverIntent } from "../hoverIntent.svelte";
  import { edgeFade } from "../edgeFade";
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menu, type MenuItem } from "../menu.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import { dragDrop, dropZone, type DropZone } from "../dragDrop.svelte";
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
  /// A drag that rests on the collapsed rail opens it, hover panels or not,
  /// until the drag ends (drag-to-playlist spec DD-D9).
  let dragPeek = $state(false);
  const peek = $derived((canPeek && (peekIntent.open || closing)) || (collapsed && dragPeek));
  const rail = $derived(collapsed && !peek);

  $effect(() => {
    if (dragDrop.active === null) dragPeek = false;
  });

  let scrollEl = $state<HTMLElement | null>(null);
  const RAIL_SPRING_MS = 400;
  const sidebarZone = $derived<DropZone>({
    key: "sidebar",
    scroller: () => scrollEl,
    spring: collapsed && !peek ? { ms: RAIL_SPRING_MS, open: () => (dragPeek = true) } : undefined,
  });

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

  // "My Collection": entries above the line always show; those below sit
  // under "Show more", or under the header's chevron when all are below.
  const layout = collectionLayout();
  const shown = $derived(visibleKeys(layout.prefs));
  const hidden = $derived(hiddenKeys(layout.prefs));
  const moreOpen = $derived(layout.prefs.open);
  const allHidden = $derived(shown.length === 0);

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

  function isActive(name: string): boolean {
    if (name === "playlists") {
      return nav.route.name === "playlists" || nav.route.name === "folder";
    }
    return nav.route.name === name;
  }

  // ---- Edit mode: drag entries (and the line) above or below the line ----

  let editing = $state(false);
  // The rail has no edit mode; collapsing the sidebar ends it.
  const edit = $derived(editing && !rail);
  $effect(() => {
    if (rail) editing = false;
  });
  const rows = $derived(toRows(layout.prefs));
  const lineIndex = $derived(rows.indexOf("line"));

  let editButton = $state<HTMLButtonElement | null>(null);
  let doneButton = $state<HTMLButtonElement | null>(null);
  let listEl = $state<HTMLElement | null>(null);
  let announce = $state("");

  async function startEditing() {
    editing = true;
    await tick();
    doneButton?.focus();
  }

  async function stopEditing() {
    endEditDrag();
    editing = false;
    announce = "";
    await tick();
    editButton?.focus();
  }

  function rowLabel(row: LayoutRow): string {
    return row === "line" ? "The line" : COLLECTION_ITEMS[row].label;
  }

  function rowState(index: number): string {
    if (index === lineIndex) return "entries below it are under Show more";
    return index < lineIndex ? "always shown" : "under Show more";
  }

  async function moveBy(index: number, delta: number) {
    const to = index + delta;
    if (to < 0 || to >= rows.length) return;
    const row = rows[index];
    layout.move(index, to);
    await tick();
    const now = rows.indexOf(row);
    announce = `${rowLabel(row)} moved to position ${now + 1} of ${rows.length}, ${rowState(now)}`;
    listEl?.querySelector<HTMLElement>(`[data-row="${row}"]`)?.focus();
  }

  function onRowKey(event: KeyboardEvent, index: number) {
    if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      void moveBy(index, event.key === "ArrowUp" ? -1 : 1);
    }
  }

  function onEditKey(event: KeyboardEvent) {
    if (!edit || event.key !== "Escape" || event.defaultPrevented) return;
    event.preventDefault();
    void stopEditing();
  }

  interface EditPress {
    index: number;
    y: number;
    x: number;
    /// Row pitch (height plus gap), and how far the row may travel each way.
    stride: number;
    minDy: number;
    maxDy: number;
    /// The pressed row's centre, relative to the list's top.
    center: number;
  }
  let editPress: EditPress | null = null;
  let editDrag = $state<{ from: number; to: number; dy: number; stride: number } | null>(null);
  let settling = $state(false);

  function onRowPointerDown(event: PointerEvent, index: number) {
    if (event.button !== 0 || event.pointerType === "touch" || editPress || !listEl) return;
    const row = event.currentTarget as HTMLElement;
    const slots = listEl.querySelectorAll<HTMLElement>("[data-slot]");
    const rect = row.getBoundingClientRect();
    const list = listEl.getBoundingClientRect();
    editPress = {
      index,
      x: event.clientX,
      y: event.clientY,
      stride: Math.max(1, slots.length > 1 ? slots[1].offsetTop - slots[0].offsetTop : rect.height),
      minDy: list.top - rect.top,
      maxDy: list.bottom - rect.bottom,
      center: rect.top - list.top + rect.height / 2,
    };
    window.addEventListener("pointermove", onEditMove);
    window.addEventListener("pointerup", onEditUp);
    window.addEventListener("pointercancel", endEditDrag);
    window.addEventListener("keydown", onEditDragKey, true);
  }

  function onEditMove(event: PointerEvent) {
    const press = editPress;
    if (!press) return;
    if (!editDrag) {
      if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_THRESHOLD) return;
      editDrag = { from: press.index, to: press.index, dy: 0, stride: press.stride };
    }
    const dy = Math.max(press.minDy, Math.min(press.maxDy, event.clientY - press.y));
    editDrag.dy = dy;
    editDrag.to = dragTarget(press.center + dy, press.stride, rows.length);
  }

  function onEditUp() {
    const done = editDrag;
    endEditDrag();
    if (!done || done.from === done.to) return;
    const row = rows[done.from];
    // Rows hold their shifted transforms; snap (no glide) into the new order.
    settling = true;
    requestAnimationFrame(() => requestAnimationFrame(() => (settling = false)));
    layout.move(done.from, done.to);
    announce = `${rowLabel(row)} moved to position ${done.to + 1} of ${rows.length}, ${rowState(done.to)}`;
  }

  function onEditDragKey(event: KeyboardEvent) {
    if (event.key !== "Escape" || !editDrag) return;
    event.preventDefault();
    event.stopPropagation();
    endEditDrag();
  }

  function endEditDrag() {
    editPress = null;
    editDrag = null;
    window.removeEventListener("pointermove", onEditMove);
    window.removeEventListener("pointerup", onEditUp);
    window.removeEventListener("pointercancel", endEditDrag);
    window.removeEventListener("keydown", onEditDragKey, true);
  }

  $effect(() => endEditDrag);

  function slotTransform(index: number): string | undefined {
    if (!editDrag) return undefined;
    if (index === editDrag.from) return `translateY(${editDrag.dy}px)`;
    const shift = rowShift(index, editDrag.from, editDrag.to);
    return shift === 0 ? undefined : `translateY(${shift * editDrag.stride}px)`;
  }
</script>

{#snippet entry(key: CollectionKey)}
  {@const item = COLLECTION_ITEMS[key]}
  {@const route = { name: key } as Route}
  <button class="nav-item" class:active={isActive(key)} aria-current={isActive(key) ? "page" : undefined} onclick={() => nav.goto(route)} use:prefetchIntent={route} title={item.label} aria-label={item.label}>
    <Icon name={item.icon} />{#if !rail}<span>{item.label}</span>{/if}
  </button>
{/snippet}

<nav
  class="sidebar"
  class:rail
  class:peek
  class:closing
  onpointerenter={onPointerEnter}
  onpointerleave={() => peekIntent.leave()}
  use:dropZone={sidebarZone}
>
  <div class="scroll edge-fade" bind:this={scrollEl} use:edgeFade={{ watchChildren: true }}>
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
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="collection-head" class:editing={edit} onkeydown={onEditKey}>
        {#if !edit && allHidden}
          <button class="section-head" onclick={() => layout.setOpen(!moreOpen)} aria-expanded={moreOpen}>
            <span>My Collection</span>
            <Icon name={moreOpen ? "chevron-down" : "chevron-right"} size={16} />
          </button>
        {:else}
          <div class="section-head static"><span>My Collection</span></div>
        {/if}
        <div class="head-actions" class:beside-chevron={!edit && allHidden}>
          {#if edit}
            <button class="mini" onclick={() => layout.reset()} title="Reset to default" aria-label="Reset My Collection to default">
              <Icon name="undo-2" size={16} />
            </button>
            <button class="mini done" bind:this={doneButton} onclick={stopEditing} title="Done" aria-label="Done editing My Collection">
              <Icon name="check" size={16} />
            </button>
          {:else}
            <button class="mini edit-toggle" bind:this={editButton} onclick={startEditing} title="Edit My Collection" aria-label="Edit My Collection">
              <Icon name="pencil" size={14} />
            </button>
          {/if}
        </div>
      </div>
    {/if}

    {#if edit}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="edit" onkeydown={onEditKey}>
        <p class="edit-hint" id="collection-edit-hint">Drag to arrange. Entries below the line stay under Show more.</p>
        <ul class="edit-list" class:settling class:dragging={editDrag !== null} bind:this={listEl} aria-label="Arrange My Collection">
          {#each rows as row, i (row)}
            <li class="slot" data-slot class:lifted={editDrag?.from === i} style:transform={slotTransform(i)}>
              {#if row === "line"}
                <button
                  class="edit-row line-row"
                  data-row={row}
                  aria-label="The line: entries below it are under Show more. Alt+Up or Alt+Down moves it"
                  aria-describedby="collection-edit-hint"
                  onpointerdown={(event) => onRowPointerDown(event, i)}
                  onkeydown={(event) => onRowKey(event, i)}
                >
                  <span class="rule"></span>
                  <span class="rule-label">Hidden below</span>
                  <span class="rule"></span>
                  <Icon name="grip-vertical" size={16} />
                </button>
              {:else}
                <button
                  class="edit-row"
                  class:below={i > lineIndex}
                  data-row={row}
                  aria-label={`${COLLECTION_ITEMS[row].label}, ${rowState(i)}. Alt+Up or Alt+Down moves it`}
                  aria-describedby="collection-edit-hint"
                  onpointerdown={(event) => onRowPointerDown(event, i)}
                  onkeydown={(event) => onRowKey(event, i)}
                >
                  <Icon name={COLLECTION_ITEMS[row].icon} />
                  <span class="edit-label">{COLLECTION_ITEMS[row].label}</span>
                  <Icon name="grip-vertical" size={16} />
                </button>
              {/if}
            </li>
          {/each}
        </ul>
        <div class="sr-only" aria-live="polite">{announce}</div>
      </div>
    {:else}
      {#each shown as key (key)}
        {@render entry(key)}
      {/each}
      {#if hidden.length > 0}
        {#if rail}
          <button class="nav-item" onclick={() => layout.setOpen(!moreOpen)} aria-expanded={moreOpen} title={moreOpen ? "Show less" : "Show more"} aria-label={moreOpen ? "Show less" : "Show more"}>
            <Icon name={moreOpen ? "chevron-up" : "chevron-down"} />
          </button>
        {:else if !allHidden}
          <button class="section-head sub" onclick={() => layout.setOpen(!moreOpen)} aria-expanded={moreOpen}>
            <span>{moreOpen ? "Show less" : "Show more"}</span>
            <Icon name={moreOpen ? "chevron-up" : "chevron-down"} size={16} />
          </button>
        {/if}
        {#if moreOpen}
          {#each hidden as key (key)}
            {@render entry(key)}
          {/each}
        {/if}
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
        <PlaylistTree entries={collection.root} {collection} {nav} {actions} {rail} {playing} {playlistActions} />
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
    /* The list edge fade (app.css): hidden past the player bar's top edge. */
    --fade-clear: var(--sidebar-clear, var(--player-height));
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
  /* My Collection's header: the label (a chevron toggle only when every entry
     is below the line) with the edit pencil, or Reset/Done while editing. */
  .collection-head {
    position: relative;
  }
  .section-head.static:hover {
    color: var(--text-muted);
  }
  .head-actions {
    position: absolute;
    top: 50%;
    /* Centred where the chevron would sit ((40px - 24px) / 2 from the edge),
       or just left of it when the chevron is there. */
    right: 5px;
    display: flex;
    gap: 2px;
    transform: translateY(-50%);
  }
  .head-actions.beside-chevron {
    right: 29px;
  }
  .edit-toggle {
    opacity: 0;
    transition: opacity var(--dur-micro) var(--ease-enter);
  }
  .collection-head:hover .edit-toggle,
  .edit-toggle:focus-visible {
    opacity: 1;
  }
  .mini.done {
    color: var(--accent);
  }
  .edit-hint {
    margin: 0 0 4px;
    padding: 0 9px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.4;
  }
  .edit-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
    user-select: none;
    touch-action: none;
  }
  .slot {
    position: relative;
    transition: transform var(--dur-micro) var(--ease-enter);
  }
  .edit-list.settling .slot,
  .slot.lifted {
    transition: none;
  }
  .slot.lifted {
    z-index: 1;
  }
  .edit-row {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    height: 36px;
    padding: 0 9px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
    cursor: grab;
  }
  .edit-row > :global(svg:first-child) {
    width: 20px;
    height: 20px;
    flex-shrink: 0;
  }
  .edit-row > :global(svg:last-child) {
    flex-shrink: 0;
    color: var(--text-muted);
    opacity: 0.7;
  }
  .edit-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .edit-row.below {
    opacity: 0.6;
  }
  .edit-row:hover,
  .edit-row:focus-visible {
    background: var(--surface-2);
    color: var(--text);
    opacity: 1;
  }
  .edit-list.dragging .edit-row {
    cursor: grabbing;
  }
  .edit-list.dragging .edit-row:hover {
    background: none;
  }
  .slot.lifted .edit-row,
  .edit-list.dragging .slot.lifted .edit-row:hover {
    background: var(--surface-2);
    color: var(--text);
    opacity: 1;
    box-shadow: var(--shadow-panel);
  }
  .line-row {
    gap: 8px;
    color: var(--accent);
  }
  .rule {
    flex: 1;
    height: 2px;
    border-radius: 1px;
    background: currentColor;
    opacity: 0.55;
  }
  .rule-label {
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .line-row:hover,
  .line-row:focus-visible,
  .slot.lifted .line-row {
    color: var(--accent);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  @media (prefers-reduced-motion: reduce) {
    .slot,
    .edit-toggle {
      transition: none;
    }
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
