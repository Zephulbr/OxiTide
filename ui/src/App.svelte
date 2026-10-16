<script lang="ts">
  import { tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import QueueRail from "./lib/components/QueueRail.svelte";
  import { fade } from "svelte/transition";
  import {
    commitOpenGlide,
    GLIDE_MS,
    holdPage,
    prepareOpenGlide,
    queueDock,
    queueFloat,
    releasePageHold,
    startPendingGlide,
  } from "./lib/queueTransition";
  import Backdrop from "./lib/components/Backdrop.svelte";
  import Login from "./views/Login.svelte";
  import Mini from "./views/Mini.svelte";
  import RoutedPage from "./lib/RoutedPage.svelte";
  import RackShell from "./lib/rack/RackShell.svelte";
  import RackMini from "./lib/rack/RackMini.svelte";
  import { createLamp } from "./lib/rack/lamp.svelte";
  import PlaylistPicker from "./lib/components/PlaylistPicker.svelte";
  import TrackInfoHost from "./lib/components/TrackInfoHost.svelte";
  import CollectionDialogs from "./lib/components/CollectionDialogs.svelte";
  import ToastHost from "./lib/components/ToastHost.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import { dialogs } from "./lib/dialogs.svelte";
  import Neko from "./lib/components/Neko.svelte";
  import PerfHud from "./lib/components/PerfHud.svelte";
  import WindowControls from "./lib/components/WindowControls.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isDragTarget } from "./lib/titlebar";
  import { createNav, type Route } from "./lib/nav.svelte";
  import {
    appearance,
    clampQueueWidth,
    QUEUE_WIDTH_MAX,
    QUEUE_WIDTH_MIN,
  } from "./lib/appearance.svelte";
  import { routeTitle } from "./lib/titles";
  import { perf } from "./lib/perf.svelte";
  import { perfHooks } from "./lib/perfHooks";
  import { makeSyntheticTracks } from "./lib/perfFixture";
  import { errorMessage } from "./lib/errors";
  import { menu } from "./lib/menu.svelte";
  import { escapeTarget, isShortcutHandled, matchShortcut, type ShortcutAction } from "./lib/shortcuts";
  import { createToasts } from "./lib/toasts.svelte";
  import { createMini } from "./lib/mini.svelte";
  import { createPlayer } from "./lib/player.svelte";
  import { createSleepTimer } from "./lib/sleepTimer.svelte";
  import { createAuthGate } from "./lib/authGate";
  import { createDynamicColor } from "./lib/dynamicColor.svelte";
  import { createFavorites } from "./lib/favorites.svelte";
  import { createPlaylistActions } from "./lib/playlists.svelte";
  import { createCollectionTree } from "./lib/collection.svelte";
  import { createCollectionActions } from "./lib/collectionActions.svelte";
  import { clearPagerCache } from "./lib/pager.svelte";
  import { clearPlaylistCovers } from "./lib/playlistArt.svelte";
  import { playSource } from "./lib/playSource.svelte";
  import { scrobbled } from "./lib/scrobbled.svelte";
  import { trackFlags } from "./lib/trackFlags.svelte";
  import { api, FAVORITE_CHANGED_EVENT, SCROBBLE_AUTH_ERROR_EVENT, SCROBBLE_SCROBBLED_EVENT, UPDATE_AVAILABLE_EVENT, trackToQueueItem, type FavoriteChange, type QueueItem, type RepeatMode, type StreamMeta, type UpdateInfo } from "./lib/api";
  import type { QualityDisplay } from "./lib/quality";

  interface ExpiredSession {
    queue: QueueItem[];
    index: number;
    position: number;
    shuffle: boolean;
    repeat: RepeatMode;
    playSource: string | null;
    stream: StreamMeta | null;
  }

  const nav = createNav(window.localStorage);
  /// Name links in the shell chrome (queue rail) leave Now Playing so the page
  /// they open is visible.
  function goFromChrome(route: Route) {
    nav.closeNowPlaying();
    nav.goto(route);
  }
  const toasts = createToasts();
  const player = createPlayer(toasts);
  const sleepTimer = createSleepTimer((fadeMs) => {
    api.fadeAndPause(fadeMs).catch((e) => toasts.show(errorMessage(e), { severity: "error" }));
  });
  createDynamicColor(player);
  createLamp(player);
  const mini = createMini(toasts);
  const favorites = createFavorites(toasts);
  const playlistActions = createPlaylistActions(toasts);
  const collection = createCollectionTree(toasts);
  const collectionActions = createCollectionActions(collection, toasts);

  // Custom window chrome (Windows): Rust decides; `data-chrome` turns on the
  // caption band (--titlebar-h) for every shell.
  let customChrome = $state(false);
  api
    .getCustomTitlebar()
    .then((custom) => {
      customChrome = custom;
      if (custom) document.documentElement.dataset.chrome = "custom";
    })
    .catch(() => {});

  let qualityDisplay = $state<QualityDisplay>("compact");
  let loggedIn = $state(false);
  let booting = $state(true);
  let error = $state("");
  let authError = $state("");
  let playerReady = $state(false);
  let expired = $state<ExpiredSession | null>(null);
  let sidebarRail = $state(false);
  let railTouched = $state(false);
  let queueOverlay = $state(false);

  $effect(() => {
    const media = window.matchMedia("(max-width: 760px)");
    const sync = () => (queueOverlay = media.matches);
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  });

  function startQueueDrag(event: PointerEvent) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = appearance.queueWidth;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    let width = startWidth;
    const onMove = (e: PointerEvent) => {
      // The handle sits on the rail's left edge: dragging left widens.
      // Preview live without touching the config (writes happen once, on release).
      width = clampQueueWidth(startWidth + (startX - e.clientX));
      document.documentElement.style.setProperty("--queue-width", `${width}px`);
    };
    const onUp = () => {
      target.removeEventListener("pointermove", onMove);
      target.removeEventListener("pointerup", onUp);
      appearance.setQueueWidth(width).catch(() => {});
    };
    target.addEventListener("pointermove", onMove);
    target.addEventListener("pointerup", onUp);
  }

  /// Keyboard resize for the queue splitter (a slider over the rail width).
  /// Left/Right follow the handle, which sits on the rail's left edge, so
  /// ArrowLeft widens; Up/Down follow the value. Shift takes bigger steps.
  function onQueueResizeKeydown(event: KeyboardEvent) {
    const step = event.shiftKey ? 48 : 16;
    let width: number | null = null;
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") width = appearance.queueWidth + step;
    else if (event.key === "ArrowRight" || event.key === "ArrowDown") width = appearance.queueWidth - step;
    else if (event.key === "Home") width = QUEUE_WIDTH_MIN;
    else if (event.key === "End") width = QUEUE_WIDTH_MAX;
    if (width === null) return;
    event.preventDefault();
    void appearance.setQueueWidth(width).catch(() => {});
  }

  $effect(() => {
    document.title = routeTitle(nav.route, nav.nowPlayingOpen);
  });

  // Restore focus to whatever opened Now Playing / the floating queue overlay
  // once it closes (both swap content out from under the focused element).
  let restoreTarget: HTMLElement | null = null;
  $effect(() => {
    const open = nav.nowPlayingOpen || (queueOverlay && nav.queueOpen);
    if (open) {
      const active = document.activeElement;
      if (restoreTarget === null && active instanceof HTMLElement && active !== document.body) {
        restoreTarget = active;
      }
    } else if (restoreTarget !== null) {
      const target = restoreTarget;
      restoreTarget = null;
      if (target.isConnected) target.focus();
    }
  });

  $effect(() => {
    const media = window.matchMedia("(max-width: 1080px)");
    const sync = () => {
      if (!railTouched) sidebarRail = media.matches;
    };
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  });

  // Hearts toggled on a paired phone (Track 11) — and echoes of our own
  // toggles, which the store ignores while they are in flight.
  $effect(() => {
    const unlisten = listen<FavoriteChange>(FAVORITE_CHANGED_EVENT, (event) => {
      const { kind, id, favorite } = event.payload;
      favorites.applyExternal(kind, id, favorite);
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  $effect(() => {
    const unlisten = listen("session_expired", () => {
      if (expired) return;
      const snapshot = player.state;
      const current = snapshot.current;
      expired = {
        queue: snapshot.queue,
        index:
          snapshot.current_index ??
          (current
            ? Math.max(0, snapshot.queue.findIndex((item) => item.track_id === current.track_id))
            : 0),
        position: snapshot.position_ms,
        shuffle: snapshot.shuffle,
        repeat: snapshot.repeat,
        playSource: playSource.playlistId,
        stream: snapshot.stream,
      };
      // The route and its persisted history stay put: after re-login the same
      // account resumes where it was. Multi-account is a non-goal (PM-D6), so a
      // different account inheriting the route is not a case handled here.
      loggedIn = false;
      favorites.reset();
      playlistActions.reset();
      collection.reset();
      collectionActions.reset();
      trackFlags.reset();
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  $effect(() => {
    const unlisten = listen<string>(SCROBBLE_AUTH_ERROR_EVENT, (event) => {
      const name = event.payload === "lastfm" ? "Last.fm" : "ListenBrainz";
      toasts.show(`${name} scrobbling needs reconnecting.`, { severity: "error" });
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  $effect(() => {
    const unlisten = listen<UpdateInfo>(UPDATE_AVAILABLE_EVENT, (event) => {
      toasts.show(`OxiTide ${event.payload.version} is available.`, {
        action: { label: "Open Settings", run: () => nav.goto({ name: "settings" }) },
      });
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  $effect(() => {
    const unlisten = listen<number>(SCROBBLE_SCROBBLED_EVENT, (event) => {
      scrobbled.mark(event.payload);
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  // The badge only describes the current play; a track change clears it even
  // while the Now Playing overlay is closed.
  $effect(() => {
    const current = player.state.current?.track_id ?? null;
    if (current !== scrobbled.trackId) scrobbled.clear();
  });

  $effect(() => {
    if (loggedIn && !playerReady) {
      playerReady = true;
      player.init();
    }
  });

  // The sleep timer rides the same 500 ms position ticks as the seek bar;
  // end-of-track mode fires once the last five seconds begin.
  $effect(() => {
    sleepTimer.observe(
      player.state.position_ms,
      player.state.duration_ms,
      player.state.playing,
      player.state.queue.length,
      player.state.crossfade_ms,
    );
  });

  $effect(() => {
    if (loggedIn) untrack(() => favorites.hydrate());
  });

  $effect(() => {
    if (loggedIn) untrack(() => collection.ensureRoot());
  });

  $effect(() => {
    if (loggedIn) {
      api
        .getConfig()
        .then((config) => (qualityDisplay = config.ui.quality_display))
        .catch(() => {});
    }
  });

  $effect(() => {
    void appearance.init();
  });

  $effect(() => {
    void trackFlags.init();
  });

  $effect(() => {
    void perf.init();
  });

  $effect(() => {
    if (!booting) perf.mark("ui_ready");
  });

  $effect(() => {
    if (!perf.active) return;
    const bridge = {
      goto: (name: string) => nav.goto({ name } as never),
      scrollTracks: (px: number) =>
        document.querySelector<HTMLElement>("[data-perf-scroll]")?.scrollBy({ top: px }),
      setFilter: (text: string) => {
        const input = document.querySelector<HTMLInputElement>("[data-perf-filter]");
        if (!input) return false;
        input.value = text;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        return true;
      },
      loadSyntheticTracks: (count: number) => {
        if (!perfHooks.loadSyntheticTracks) return false;
        perfHooks.loadSyntheticTracks(count);
        return true;
      },
      measureFilter: async (text: string) => {
        const input = document.querySelector<HTMLInputElement>("[data-perf-filter]");
        if (!input) return false;
        const started = performance.now();
        input.value = text;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        perf.recordInteraction("filter", performance.now() - started);
        return true;
      },
      measureSort: async (key: "title" | "duration") => {
        if (!perfHooks.toggleViewSort) return false;
        const started = performance.now();
        perfHooks.toggleViewSort(key);
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        perf.recordInteraction("sort", performance.now() - started);
        return true;
      },
      buildSyntheticQueue: async (count: number) => {
        const items = makeSyntheticTracks(count).map(trackToQueueItem);
        const started = performance.now();
        await api.perfQueueBuild(items);
        perf.recordInteraction("queue", performance.now() - started);
        return true;
      },
      gotoAlbum: (id: number) => {
        nav.goto({ name: "album", id });
        return true;
      },
      albumHookReady: () => typeof perfHooks.measureAlbumOpen === "function",
      measureAlbumOpen: (count: number) => {
        if (!perfHooks.measureAlbumOpen) return false;
        perfHooks.measureAlbumOpen(count);
        return true;
      },
      setMode: (mode: "instrument" | "atmosphere") => appearance.setMode(mode),
      setTheme: (theme: "dark" | "light" | "system") => appearance.setTheme(theme),
    };
    (window as unknown as { __perf?: typeof bridge }).__perf = bridge;
    return () => {
      delete (window as unknown as { __perf?: unknown }).__perf;
    };
  });

  $effect(() => {
    void mini.init();
  });

  // The login view needs room: leave mini whenever it shows without a session
  // (logout, session expiry, and a start_in_mini cold boot all land here).
  $effect(() => {
    if (!booting && !loggedIn && mini.active) void mini.exit();
  });

  $effect(() => {
    api
      .authRestore()
      .then(async (session) => {
        if (session) {
          await restoreSavedSession();
          loggedIn = true;
        }
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        booting = false;
      });
  });

  $effect(() => {
    const gate = createAuthGate({
      listen,
      takeResult: api.authTakeResult,
      onLogin,
      onError: (message) => (authError = message),
    });
    return () => gate.dispose();
  });

  // Docked queue opening: snapshot the page before the rail enters the DOM,
  // then hold + glide it right after the update (lib/queueTransition.ts).
  // Closing is handled by the rail's outro start, when it is still laid out.
  let queueWasOpen = false;
  $effect.pre(() => {
    if (nav.queueOpen && !queueWasOpen && !queueOverlay) {
      prepareOpenGlide(document.getElementById("main"), { glide: !appearance.reducedMotion });
    }
  });
  $effect(() => {
    const open = nav.queueOpen;
    if (open && !queueWasOpen && !queueOverlay) {
      commitOpenGlide(document.getElementById("main"), appearance.queueWidth);
    }
    queueWasOpen = open;
  });

  async function restoreSavedSession() {
    try {
      const session = await api.getRestoredSession();
      if (!session) return;
      if (session.queue.length === 0) {
        playSource.restore(null);
        return;
      }
      await player.restore(
        session.queue,
        session.current_index,
        session.position_ms,
        session.shuffle,
        session.repeat,
        session.stream ?? null,
      );
      playSource.restore(session.play_source);
    } catch (e) {
      error = String(e);
    }
  }

  async function onLogin() {
    authError = "";
    clearPagerCache();
    clearPlaylistCovers();
    error = "";
    const restore = expired;
    expired = null;
    if (restore) {
      try {
        await player.restore(
          restore.queue,
          restore.index,
          restore.position,
          restore.shuffle,
          restore.repeat,
          restore.stream,
        );
        playSource.restore(restore.playSource);
      } catch (e) {
        error = String(e);
      }
    } else {
      await restoreSavedSession();
    }
    loggedIn = true;
  }

  async function onLogout() {
    try {
      await api.authLogout();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
    clearPagerCache();
    clearPlaylistCovers();
    favorites.reset();
    playlistActions.reset();
    collection.reset();
    collectionActions.reset();
    trackFlags.reset();
    nav.reset();
    loggedIn = false;
  }

  /// Side buttons can fire both mousedown and auxclick for one press; ignore
  /// repeats inside this window so navigation happens exactly once.
  let sideButtonAt = 0;

  async function focusSearch() {
    if (nav.route.name !== "explore") nav.goto({ name: "explore" });
    await tick();
    document.getElementById("search-input")?.focus();
  }

  function currentEscapeTarget() {
    return escapeTarget({
      menuOpen: menu.open !== null,
      miniActive: mini.active,
      miniQueueOpen: mini.queueOpen,
      nowPlayingOpen: nav.nowPlayingOpen,
      queueOpen: nav.queueOpen,
    });
  }

  function runShortcut(action: ShortcutAction) {
    const state = player.state;
    switch (action) {
      case "play-pause":
        if (state.current) player.playPause();
        break;
      case "seek-back":
        if (state.current) player.seek(Math.max(0, state.position_ms - 5000));
        break;
      case "seek-forward": {
        if (!state.current) break;
        const limit = state.duration_ms > 0 ? state.duration_ms : state.position_ms + 5000;
        player.seek(Math.min(limit, state.position_ms + 5000));
        break;
      }
      case "prev-track":
        if (state.queue.length > 0) player.prev();
        break;
      case "next-track":
        if (state.queue.length > 0) player.next();
        break;
      case "volume-up":
        player.setVolume(Math.min(1, state.volume + 0.05));
        break;
      case "volume-down":
        player.setVolume(Math.max(0, state.volume - 0.05));
        break;
      case "toggle-mute":
        player.toggleMute();
        break;
      case "focus-search":
        void focusSearch();
        break;
      case "history-back":
        nav.back();
        break;
      case "history-forward":
        nav.forward();
        break;
      case "escape":
        switch (currentEscapeTarget()) {
          case "menu":
            menu.close();
            break;
          case "mini-queue":
            mini.toggleQueue();
            break;
          case "exit-mini":
            void mini.exit();
            break;
          case "now-playing":
            nav.closeNowPlaying();
            break;
          case "queue":
            nav.toggleQueue();
            break;
        }
        break;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (!loggedIn) return;
    // A component already handled this key (slider, tabs, menu, row focus):
    // acting on it globally too would double-seek or double-step.
    if (event.defaultPrevented) return;
    const overlayOpen = dialogs.count > 0;
    const action = matchShortcut(event, { overlayOpen });
    if (!action) {
      // A held Space is ignored as a toggle, but it must not scroll the page
      // once a track is loaded.
      if (event.key === " " && event.repeat && player.state.current) event.preventDefault();
      return;
    }
    if (action === "escape" && currentEscapeTarget() === null) return;
    if (
      !isShortcutHandled(action, {
        hasCurrent: player.state.current !== null,
        hasQueue: player.state.queue.length > 0,
      })
    ) {
      return;
    }
    event.preventDefault();
    runShortcut(action);
  }

  /// The top band is the title bar under custom chrome: a press on anything
  /// that is not a control drags the window, a double-click maximizes (not in
  /// the mini player, which has no maximize).
  function onTitlebarPress(event: MouseEvent) {
    if (!customChrome || event.button !== 0) return;
    if (!isDragTarget(event.target, event.clientY)) return;
    event.preventDefault();
    const win = getCurrentWindow();
    if (event.detail === 2) {
      if (!(loggedIn && mini.active)) void win.toggleMaximize();
    } else {
      void win.startDragging();
    }
  }

  function onSideButton(event: MouseEvent) {
    if (!loggedIn) return;
    if (event.button !== 3 && event.button !== 4) return;
    event.preventDefault();
    const now = Date.now();
    if (now - sideButtonAt < 50) return;
    sideButtonAt = now;
    if (event.button === 3) nav.back();
    else nav.forward();
  }
</script>

{#if booting}
  <p class="muted" style="padding:24px">Loading…</p>
{:else if !loggedIn}
  {#if error}<p style="color:var(--danger);padding:0 24px">{error}</p>{/if}
  <Login
    onLogin={onLogin}
    message={expired ? "Session expired — sign in to continue" : ""}
    {authError}
  />
{:else}
  <Backdrop {player} />
  {#if mini.active}
    <div class="mini-layer" inert={dialogs.count > 0}>
      <!-- Rack has its own mini player (9.5c); same sanctioned branch (RK-D1). -->
      {#if appearance.rackShell}
        <RackMini {player} {favorites} {mini} />
      {:else}
        <Mini {player} {favorites} {mini} />
      {/if}
    </div>
  {:else if appearance.rackShell}
    <!-- Hi-Fi Rack: the one sanctioned shell branch (Rack spec RK-D1). -->
    <div class="rack-layer" inert={dialogs.count > 0}>
      <RackShell
        {nav}
        {player}
        {favorites}
        {playlistActions}
        {collection}
        {collectionActions}
        {toasts}
        {qualityDisplay}
        {mini}
        {sleepTimer}
        {error}
        {onLogout}
        onQualityDisplay={(mode) => (qualityDisplay = mode)}
      />
      <TrackInfoHost {nav} />
      <PlaylistPicker actions={playlistActions} onnavigate={(id) => nav.goto({ name: "playlist", id })} />
      <CollectionDialogs actions={collectionActions} />
    </div>
  {:else}
  <div class="app" data-sidebar={sidebarRail ? "rail" : "full"} data-now-playing={nav.nowPlayingOpen} inert={dialogs.count > 0}>
    <a class="skip-link" href="#main">Skip to content</a>
    <Sidebar
      {nav}
      rail={sidebarRail}
      onToggleRail={() => { railTouched = true; sidebarRail = !sidebarRail; }}
      {collection}
      {playlistActions}
      actions={collectionActions}
      playing={player.state.playing}
    />
    <main class="main" id="main" tabindex="-1">
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
        scroller={() => document.getElementById("main")}
        {onLogout}
        onQualityDisplay={(mode) => (qualityDisplay = mode)}
      />
    </main>
    {#if nav.queueOpen}
      {#if queueOverlay}
        <div
          class="queue-backdrop"
          onclick={() => nav.toggleQueue()}
          aria-hidden="true"
          transition:fade|global={{ duration: appearance.reducedMotion ? 120 : GLIDE_MS }}
        ></div>
        <aside class="queue floating" transition:queueFloat|global={{ reduced: appearance.reducedMotion }}>
          <QueueRail {player} go={(route) => { nav.toggleQueue(); goFromChrome(route); }} />
        </aside>
      {:else}
        <aside
          class="queue"
          transition:queueDock|global={{ reduced: appearance.reducedMotion }}
          onintrostart={startPendingGlide}
          onoutrostart={(e) =>
            holdPage(document.getElementById("main"), e.currentTarget, 0, {
              glide: !appearance.reducedMotion,
            })}
          onintroend={releasePageHold}
          onoutroend={releasePageHold}
        >
          <div
            class="queue-resize"
            onpointerdown={startQueueDrag}
            onkeydown={onQueueResizeKeydown}
            tabindex="0"
            role="slider"
            aria-label="Queue width"
            aria-valuemin={QUEUE_WIDTH_MIN}
            aria-valuemax={QUEUE_WIDTH_MAX}
            aria-valuenow={appearance.queueWidth}
            aria-valuetext={`${appearance.queueWidth} pixels`}
          ></div>
          <QueueRail {player} go={goFromChrome} />
        </aside>
      {/if}
    {/if}
    <PlayerBar {player} {qualityDisplay} {favorites} {nav} {mini} {sleepTimer} />
    <TrackInfoHost {nav} />
    <PlaylistPicker actions={playlistActions} onnavigate={(id) => nav.goto({ name: "playlist", id })} />
    <CollectionDialogs actions={collectionActions} />
  </div>
  {/if}
  <div class="grain" aria-hidden="true"></div>
  {#if appearance.oneko && !appearance.reducedMotion}<Neko />{/if}
{/if}

<svelte:window
  onkeydown={onKeydown}
  onmousedown={(e) => {
    onSideButton(e);
    onTitlebarPress(e);
  }}
  onauxclick={onSideButton}
/>
<ToastHost {toasts} />
{#if customChrome}<WindowControls maximizable={!(loggedIn && mini.active)} />{/if}
<ContextMenu {menu} />
<PerfHud />
