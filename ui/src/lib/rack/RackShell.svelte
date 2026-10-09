<script lang="ts">
  import { tick } from "svelte";
  import { appearance } from "../appearance.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import Icon from "../components/Icon.svelte";
  import PlaylistTree from "../components/PlaylistTree.svelte";
  import QueueRail from "../components/QueueRail.svelte";
  import SleepTimerControl from "../components/SleepTimerControl.svelte";
  import type { Favorites } from "../favorites.svelte";
  import { activeLine } from "../lyrics";
  import { lyrics } from "../lyrics.svelte";
  import type { Mini } from "../mini.svelte";
  import type { Nav, Route } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import type { QualityDisplay } from "../quality";
  import RoutedPage from "../RoutedPage.svelte";
  import type { SleepTimer } from "../sleepTimer.svelte";
  import type { Toasts } from "../toasts.svelte";
  import AmpUnit from "./AmpUnit.svelte";
  import DeckUnit from "./DeckUnit.svelte";
  import EqUnit from "./EqUnit.svelte";
  import { levels } from "./levels.svelte";
  import SpectrumMeter from "./meters/SpectrumMeter.svelte";
  import { LIBRARY_KEYS, libraryKeyOf, sourceOf, type Source } from "./sources";

  /// The Hi-Fi Rack shell (RK-D1, RK-D4): selector unit across the top, the
  /// rack column (deck, amp, EQ) on the left, the display unit on the right.
  /// Pages render unmodified inside the display; Now Playing expands the deck.
  let {
    nav,
    player,
    favorites,
    playlistActions,
    collection,
    collectionActions,
    toasts,
    qualityDisplay,
    mini,
    sleepTimer,
    error,
    onLogout,
    onQualityDisplay,
  }: {
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    collection: CollectionTree;
    collectionActions: CollectionActions;
    toasts: Toasts;
    qualityDisplay: QualityDisplay;
    mini: Mini;
    sleepTimer: SleepTimer;
    error: string;
    onLogout: () => void;
    onQualityDisplay: (mode: QualityDisplay) => void;
  } = $props();

  const SOURCES: { source: Source; label: string; route: Route }[] = [
    { source: "home", label: "Home", route: { name: "home" } },
    { source: "explore", label: "Explore", route: { name: "explore" } },
    { source: "library", label: "Library", route: { name: "albums" } },
    { source: "playlists", label: "Playlists", route: { name: "playlists" } },
  ];

  const expanded = $derived(nav.nowPlayingOpen);
  const source = $derived(sourceOf(nav.route));
  const libraryKey = $derived(libraryKeyOf(nav.route));
  let column: HTMLDivElement;

  // The compact column can scroll on short windows; the expanded layout must
  // start at the top or its units render cut off.
  $effect(() => {
    void expanded;
    if (column) column.scrollTop = 0;
  });

  async function search() {
    if (nav.nowPlayingOpen) nav.closeNowPlaying();
    if (nav.route.name !== "explore") nav.goto({ name: "explore" });
    await tick();
    document.getElementById("search-input")?.focus();
  }

  function go(route: Route) {
    if (nav.nowPlayingOpen) nav.closeNowPlaying();
    nav.goto(route);
  }

  // Expanded deck: lyrics on a VFD (previous / current / next line).
  $effect(() => {
    if (expanded) void lyrics.open(player.state.current);
  });
  const lines = $derived(lyrics.lyrics?.lines ?? []);
  const active = $derived(
    lyrics.lyrics?.synced ? activeLine(lines, player.pendingSeek ?? player.state.position_ms) : -1,
  );
  $effect(() => {
    if (expanded) return levels.use();
  });
</script>

<div class="rack" class:expanded>
  <a class="skip-link" href="#main">Skip to content</a>

  <header class="rk-unit selector">
    <div class="brand"><span class="rk-name wordmark">OxiTide</span><span class="rk-silk">Network audio player</span></div>
    <div class="rk-tray history" role="group" aria-label="History">
      <button class="rk-key sm" disabled={!nav.canBack} onclick={() => nav.back()} aria-label="Back" title="Back (Alt+←)"><Icon name="chevron-left" size={12} /></button>
      <button class="rk-key sm" disabled={!nav.canForward} onclick={() => nav.forward()} aria-label="Forward" title="Forward (Alt+→)"><Icon name="chevron-right" size={12} /></button>
    </div>
    <nav class="sources" aria-label="Source">
      {#each SOURCES as s (s.source)}
        <button class="src" aria-current={source === s.source && !expanded ? "page" : undefined} onclick={() => go(s.route)}>
          <span class="lamp"></span><span class="rk-key">{s.label}</span>
        </button>
      {/each}
      <button class="src" onclick={search} title="Search (Ctrl+F)">
        <span class="lamp off"></span><span class="rk-key">Search</span>
      </button>
    </nav>
    <div class="right">
      <span class="rk-tray sleep" title="Sleep timer"><SleepTimerControl {sleepTimer} /></span>
      <button class="rk-key sm" onclick={() => void mini.enter()} title="Mini player">Mini</button>
      <button class="src setup" aria-current={source === "settings" && !expanded ? "page" : undefined} onclick={() => go({ name: "settings" })}>
        <span class="lamp"></span><span class="rk-key">Setup</span>
      </button>
      <div class="power" aria-hidden="true"><span class="dot"></span><span class="rk-silk">Power</span></div>
    </div>
  </header>

  <div class="column" bind:this={column}>
    <DeckUnit {player} {nav} {favorites} face={appearance.rack.deck_face} {expanded} />
    <AmpUnit {player} meters={appearance.rack.meters} {toasts} />
    {#if expanded}
      <section class="rk-unit spectrum" aria-label="Spectrum analyzer">
        <div class="rk-head"><h2 class="rk-name">Spectrum analyzer</h2><span class="rk-silk">28 band</span></div>
        <div class="rk-vfd specwin" aria-hidden="true"><SpectrumMeter snapshot={levels.snapshot} count={28} /></div>
      </section>
      <section class="rk-unit lyricsunit" aria-label="Lyrics">
        <div class="rk-head"><h2 class="rk-name">Lyrics</h2><span class="rk-silk">{lyrics.lyrics?.synced ? "Synced" : ""}</span></div>
        <div class="rk-vfd lyrics">
          {#if lines.length > 0 && active >= 0}
            {#each [-2, -1, 0, 1, 2] as offset (offset)}
              <div class="rk-vfd-text line" class:now={offset === 0} class:far={Math.abs(offset) === 2}>{lines[active + offset]?.text ?? ""}</div>
            {/each}
          {:else}
            <div class="rk-vfd-text sub line far">{lyrics.loading ? "Loading lyrics" : lines.length > 0 ? "" : "No synced lyrics"}</div>
            <div class="rk-vfd-text line now">{player.state.current?.title ?? ""}</div>
            <div class="rk-vfd-text sub line far">{player.state.current?.artist ?? ""}</div>
          {/if}
        </div>
      </section>
    {:else}
      {#if appearance.rack.eq_unit}<EqUnit {toasts} />{/if}
      <section class="rk-unit library" aria-label="Playlists">
        <div class="rk-head"><h2 class="rk-name">Playlists</h2>
          <button class="rk-key sm" onclick={() => go({ name: "playlists" })} title="All playlists">All</button>
        </div>
        <div class="rk-win tree">
          {#if collection.error}
            <p class="muted">{collection.error}</p>
          {:else}
            <PlaylistTree entries={collection.root} {collection} {nav} actions={collectionActions} />
          {/if}
        </div>
      </section>
    {/if}
  </div>

  {#if !expanded}
    <section class="rk-unit display" aria-label="Display">
      {#if libraryKey}
        <div class="rk-tray libkeys" role="group" aria-label="Library">
          {#each LIBRARY_KEYS as k (k.label)}
            <button class="rk-key sm" aria-current={libraryKey === k.label ? "page" : undefined} onclick={() => nav.goto(k.route)}>{k.label}</button>
          {/each}
        </div>
      {/if}
      <main class="main rack-main" id="main" tabindex="-1">
        {#if error}<p style="color:var(--danger)">{error}</p>{/if}
        <RoutedPage
          {nav}
          {player}
          {favorites}
          {playlistActions}
          {collection}
          {collectionActions}
          {toasts}
          {qualityDisplay}
          nowPlaying={false}
          {onLogout}
          {onQualityDisplay}
        />
      </main>
      {#if nav.queueOpen}
        <aside class="drawer right rk-unit" aria-label="Queue">
          <div class="rk-head"><h2 class="rk-name">Queue</h2>
            <button class="rk-key sm" onclick={() => nav.toggleQueue()} aria-label="Close queue" title="Close"><Icon name="x" size={12} /></button>
          </div>
          <div class="rk-win drawerwin"><QueueRail {player} /></div>
        </aside>
      {/if}
    </section>
  {:else if nav.queueOpen}
    <aside class="drawer right floating rk-unit" aria-label="Queue">
      <div class="rk-head"><h2 class="rk-name">Queue</h2>
        <button class="rk-key sm" onclick={() => nav.toggleQueue()} aria-label="Close queue" title="Close"><Icon name="x" size={12} /></button>
      </div>
      <div class="rk-win drawerwin"><QueueRail {player} /></div>
    </aside>
  {/if}
</div>

<style>
  .rack {
    --col: clamp(300px, 30vw, 392px);
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: var(--col) minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    gap: 6px;
    height: 100%;
    padding: 10px;
    box-sizing: border-box;
    background: var(--rack-room);
  }

  .selector {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 10px 22px;
    min-width: 0;
  }
  .brand {
    display: flex;
    align-items: baseline;
    gap: 12px;
    white-space: nowrap;
  }
  .wordmark {
    font-size: 20px;
    font-weight: 400;
    letter-spacing: 0.03em;
  }
  .sources {
    display: flex;
    gap: 10px;
    min-width: 0;
  }
  .src {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .src .rk-key {
    min-width: 76px;
  }
  .src .rk-key::after {
    display: none;
  }
  .lamp {
    width: 18px;
    height: 4px;
    border-radius: 2px;
    background: #2a1a08;
    box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.7);
    transition: background 350ms, box-shadow 350ms;
  }
  .lamp.off {
    visibility: hidden;
  }
  .src[aria-current] .lamp {
    background: var(--rack-lamp);
    box-shadow: 0 0 8px var(--rack-lamp-glow), 0 0 2px var(--rack-lamp);
    transition-duration: 80ms;
  }
  .src[aria-current] .rk-key {
    color: var(--rack-key-ink-on);
  }
  .src:active .rk-key {
    transform: perspective(200px) rotateX(-16deg);
    transform-origin: 50% 40%;
  }
  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  /* The shared sleep-timer button, seated in a tray as a key. */
  .sleep {
    color: var(--rack-key-ink);
  }
  .sleep :global(.moon) {
    width: 34px;
    height: 28px;
    border-radius: 3px;
    background: var(--rack-key);
    border-top: 1px solid var(--rack-key-edge);
    box-shadow: 0 0 0 1px #000, 0 2px 4px rgba(0, 0, 0, 0.6);
    color: var(--rack-key-ink);
  }
  .sleep :global(.moon:hover) {
    color: #b9ad9c;
  }
  .power {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--rack-lamp);
    box-shadow: 0 0 8px var(--rack-lamp-glow);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
  }
  /* Units keep their natural height; the Playlists unit takes the slack so
     the column and the display share a bottom edge. */
  .library {
    flex: 1 1 0;
    min-height: 150px;
    display: flex;
    flex-direction: column;
  }
  .library .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 6px;
    scrollbar-width: thin;
    scrollbar-color: #3a342e transparent;
  }

  .display {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 14px 6px 0 16px;
    overflow: hidden;
  }
  .libkeys {
    align-self: flex-start;
    flex-wrap: wrap;
    margin: 0 0 12px 6px;
  }
  .rack-main {
    flex: 1;
    min-height: 0;
    padding: 8px 14px 28px 6px;
  }

  .drawer {
    position: absolute;
    top: 8px;
    bottom: 8px;
    z-index: 7;
    width: min(340px, 80%);
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 12px;
    box-shadow: var(--rack-unit-shadow), 0 20px 40px rgba(0, 0, 0, 0.7);
    animation: rk-drawer var(--dur-view) var(--ease-in);
  }
  .drawerwin {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .drawerwin > :global(*) {
    flex: 1;
    min-height: 0;
  }
  .drawer.right {
    right: 8px;
  }
  .drawer.floating {
    position: fixed;
    top: 90px;
    bottom: 16px;
  }
  @keyframes rk-drawer {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  /* Expanded deck: the column spans the shell (DESIGN §2). */
  /* Deck on the left; amp, spectrum (fixed height) and lyrics (the slack) on
     the right, each at its natural size. */
  .rack.expanded .column {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(380px, 1fr);
    grid-template-rows: auto auto minmax(0, 1fr);
    overflow: hidden;
  }
  .rack.expanded .column > :global(.deck) {
    grid-row: 1 / 4;
  }
  .specwin {
    height: 150px;
    padding: 10px 12px 6px;
  }
  .lyricsunit {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .lyrics {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 10px;
    min-height: 140px;
    padding: 16px 18px;
  }
  .lyrics .line {
    white-space: normal;
    font-size: 13px;
    opacity: 0.45;
    text-shadow: none;
  }
  .lyrics .line.far {
    opacity: 0.2;
  }
  .lyrics .line.now {
    font-size: 17px;
    opacity: 1;
    text-shadow: 0 0 8px var(--rack-vfd), 0 0 1px var(--rack-vfd);
  }

  /* Window fit (spec §6.9). */
  @media (max-height: 760px) {
    .library {
      display: none;
    }
  }
  @media (max-width: 1000px) {
    .rack:not(.expanded) .column > :global(.eq) {
      display: none;
    }
    .brand .rk-silk,
    .power {
      display: none;
    }
  }
  @media (max-width: 860px) {
    .src .rk-key {
      min-width: 0;
    }
  }
</style>
