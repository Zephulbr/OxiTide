<script lang="ts">
  import { api } from "../api";
  import { appearance } from "../appearance.svelte";
  import Cover from "../components/Cover.svelte";
  import { clock, currentIndex, upNext } from "./deck";
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

  /// Mirrors the default sidebar (search lives on Explore).
  const SOURCES: { source: Source; label: string; route: Route }[] = [
    { source: "home", label: "Home", route: { name: "home" } },
    { source: "explore", label: "Explore", route: { name: "explore" } },
    { source: "feed", label: "Feed", route: { name: "feed" } },
    { source: "history", label: "History", route: { name: "history", tab: "recent" } },
    { source: "library", label: "Library", route: { name: "playlists" } },
  ];
  const sleeping = $derived(sleepTimer.mode !== "off");

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
  /// Nine lyric lines around the current one, fading with distance.
  const LYRIC_OFFSETS = [-4, -3, -2, -1, 0, 1, 2, 3, 4];
  const queueIndex = $derived(currentIndex(player.state));
  const upcoming = $derived(upNext(player.state, 4));
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
    </nav>
    <!-- Utility keys: the same key-with-lamp as the sources, on one baseline. -->
    <div class="right">
      <div class="src util" class:lit={sleeping}>
        <span class="lamp"></span>
        <span class="sleep"><SleepTimerControl {sleepTimer} /></span>
      </div>
      <button class="src util" onclick={() => void mini.enter()} title="Mini player">
        <span class="lamp"></span><span class="rk-key">Mini</span>
      </button>
      <button class="src util" aria-current={source === "settings" && !expanded ? "page" : undefined} onclick={() => go({ name: "settings" })}>
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
            {#each LYRIC_OFFSETS as offset (offset)}
              <div class="rk-vfd-text line" class:now={offset === 0} style:--fade={1 - Math.abs(offset) * 0.17}>{lines[active + offset]?.text ?? ""}</div>
            {/each}
          {:else}
            <div class="rk-vfd-text sub line" style:--fade="0.5">{lyrics.loading ? "Loading lyrics" : lines.length > 0 ? "Waiting for the first line" : "No synced lyrics"}</div>
            <div class="rk-vfd-text line now">{player.state.current?.title ?? ""}</div>
            <div class="rk-vfd-text sub line" style:--fade="0.6">{player.state.current ? `${player.state.current.artist} · ${player.state.current.album}` : ""}</div>
          {/if}
        </div>
      </section>
      <section class="rk-unit changer" aria-label="Up next">
        <div class="rk-head"><h2 class="rk-name">Up next</h2>
          <span class="rk-silk">{queueIndex >= 0 ? `Track ${queueIndex + 1} of ${player.state.queue.length}` : ""}</span>
        </div>
        <ol class="slots">
          {#each upcoming as item, i (item.track_id + ":" + i)}
            <li>
              <button class="slot" onclick={() => api.playTracks(player.state.queue, queueIndex + 1 + i)} title={`Play ${item.title}`}>
                <span class="rk-win thumb"><Cover uuid={item.cover} size={80} radius={2} alt="" /></span>
                <span class="num">{queueIndex + 2 + i}</span>
                <span class="meta"><b>{item.title}</b><i>{item.artist}</i></span>
                <span class="dur">{clock(item.duration_secs * 1000)}</span>
              </button>
            </li>
          {:else}
            <li class="end">End of queue</li>
          {/each}
        </ol>
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
  /* Utility keys share the source keys' shape: lamp above, same key height,
     same baseline. The shared sleep-timer button is dressed as that key. */
  .src.util .rk-key {
    min-width: 58px;
  }
  .src.util.lit .lamp {
    background: var(--rack-lamp);
    box-shadow: 0 0 8px var(--rack-lamp-glow), 0 0 2px var(--rack-lamp);
  }
  .sleep {
    display: inline-flex;
    position: relative;
  }
  .sleep :global(.moon) {
    min-width: 58px;
    height: 34px;
    padding: 0 12px;
    border: 0;
    border-top: 1px solid var(--rack-key-edge);
    border-radius: 3px;
    background: var(--rack-key);
    box-shadow: 0 0 0 1px #000, 0 2px 4px rgba(0, 0, 0, 0.6);
    color: var(--rack-key-ink);
  }
  .sleep :global(.moon:hover) {
    color: #b9ad9c;
    background: var(--rack-key);
  }
  .src.util.lit .sleep :global(.moon) {
    color: var(--rack-key-ink-on);
  }
  /* Same two rows as a key (lamp, then a 34px label row) so it lines up. */
  .power {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    margin-left: 4px;
  }
  .power .rk-silk {
    height: 34px;
    display: flex;
    align-items: center;
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
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    overflow: hidden;
  }
  .rack.expanded .column > :global(.deck) {
    grid-row: 1 / 5;
  }
  /* The wide right column gets full-size meters. */
  .rack.expanded .column :global(.amp .meters) {
    height: 150px;
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
    gap: 12px;
    min-height: 140px;
    overflow: hidden;
    padding: 18px 22px;
  }
  .lyrics .line {
    white-space: normal;
    font-size: 13px;
    opacity: calc(var(--fade, 0.5) * 0.6);
    text-shadow: none;
    transition: opacity var(--dur-state);
  }
  .lyrics .line.now {
    font-size: 19px;
    opacity: 1;
    text-shadow: 0 0 8px var(--rack-vfd), 0 0 1px var(--rack-vfd);
  }

  /* Up next: a changer of the next tracks; a slot plays from there. */
  .slots {
    display: grid;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .slot {
    display: grid;
    grid-template-columns: 34px 22px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    width: 100%;
    padding: 4px 6px;
    border: 0;
    border-radius: 3px;
    background: none;
    color: var(--rack-screen-text);
    text-align: left;
    cursor: pointer;
  }
  .slot:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .thumb {
    display: block;
    width: 34px;
    height: 34px;
    padding: 2px;
    box-sizing: border-box;
  }
  .thumb :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .num,
  .dur {
    font: 700 10px/1 var(--font-ui);
    color: var(--rack-silk-dim);
    font-variant-numeric: tabular-nums;
  }
  .meta {
    display: grid;
    min-width: 0;
  }
  .meta b,
  .meta i {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .meta b {
    font-size: 12.5px;
    font-weight: 600;
  }
  .meta i {
    font-size: 11px;
    font-style: normal;
    color: var(--rack-screen-muted);
  }
  .end {
    padding: 6px;
    color: var(--rack-screen-muted);
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
