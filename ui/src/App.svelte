<script lang="ts">
  import { tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import QueueRail from "./lib/components/QueueRail.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Album from "./views/Album.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Artist from "./views/Artist.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import Settings from "./views/Settings.svelte";
  import NowPlaying from "./views/NowPlaying.svelte";
  import PlaylistPicker from "./lib/components/PlaylistPicker.svelte";
  import ToastHost from "./lib/components/ToastHost.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import { createNav } from "./lib/nav.svelte";
  import { appearance, clampQueueWidth } from "./lib/appearance.svelte";
  import { errorMessage } from "./lib/errors";
  import { menu } from "./lib/menu.svelte";
  import { isShortcutHandled, matchShortcut, type ShortcutAction } from "./lib/shortcuts";
  import { createToasts } from "./lib/toasts.svelte";
  import { createPlayer } from "./lib/player.svelte";
  import { createFavorites } from "./lib/favorites.svelte";
  import { createPlaylistActions } from "./lib/playlists.svelte";
  import { clearPagerCache } from "./lib/pager.svelte";
  import { api, type QueueItem } from "./lib/api";
  import type { QualityDisplay } from "./lib/quality";

  interface ExpiredSession {
    queue: QueueItem[];
    index: number;
    position: number;
    playing: boolean;
  }

  const nav = createNav();
  const player = createPlayer();
  const toasts = createToasts();
  const favorites = createFavorites(toasts);
  const playlistActions = createPlaylistActions(toasts);
  let qualityDisplay = $state<QualityDisplay>("compact");
  let loggedIn = $state(false);
  let booting = $state(true);
  let error = $state("");
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

  $effect(() => {
    const media = window.matchMedia("(max-width: 1080px)");
    const sync = () => {
      if (!railTouched) sidebarRail = media.matches;
    };
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  });

  $effect(() => {
    const unlisten = listen("session_expired", () => {
      if (expired) return;
      const snapshot = player.state;
      const current = snapshot.current;
      expired = {
        queue: snapshot.queue,
        index: current
          ? Math.max(0, snapshot.queue.findIndex((item) => item.track_id === current.track_id))
          : 0,
        position: snapshot.position_ms,
        playing: snapshot.playing,
      };
      loggedIn = false;
      favorites.reset();
      playlistActions.reset();
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  $effect(() => {
    if (loggedIn && !playerReady) {
      playerReady = true;
      player.init();
    }
  });

  $effect(() => {
    if (loggedIn) untrack(() => favorites.hydrate());
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
    api
      .authRestore()
      .then((session) => {
        if (session) loggedIn = true;
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        booting = false;
      });
  });

  async function onLogin() {
    clearPagerCache();
    error = "";
    const restore = expired;
    expired = null;
    if (restore) {
      try {
        await player.restore(restore.queue, restore.index, restore.position, restore.playing);
      } catch (e) {
        error = String(e);
      }
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
    favorites.reset();
    playlistActions.reset();
    loggedIn = false;
  }

  /// Side buttons can fire both mousedown and auxclick for one press; ignore
  /// repeats inside this window so navigation happens exactly once.
  let sideButtonAt = 0;

  async function focusSearch() {
    if (nav.route.name !== "search") nav.goto({ name: "search" });
    await tick();
    document.getElementById("search-input")?.focus();
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
        if (menu.open) menu.close();
        else if (nav.nowPlayingOpen) nav.closeNowPlaying();
        else if (nav.queueOpen) nav.toggleQueue();
        break;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (!loggedIn) return;
    const overlayOpen = document.querySelector('[role="dialog"]') !== null;
    const action = matchShortcut(event, { overlayOpen });
    if (!action) {
      // A held Space is ignored as a toggle, but it must not scroll the page
      // once a track is loaded.
      if (event.key === " " && event.repeat && player.state.current) event.preventDefault();
      return;
    }
    if (action === "escape" && !menu.open && !nav.nowPlayingOpen && !nav.queueOpen) return;
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
  />
{:else}
  <div class="app" data-sidebar={sidebarRail ? "rail" : "full"}>
    <Sidebar {nav} rail={sidebarRail} onToggleRail={() => { railTouched = true; sidebarRail = !sidebarRail; }} onLogout={onLogout} />
    <main class="main">
      {#if error}<p style="color:var(--danger)">{error}</p>{/if}
      {#if nav.nowPlayingOpen}<NowPlaying {player} {nav} {qualityDisplay} />
      {:else if nav.route.name === "home"}<Home {nav} />
      {:else if nav.route.name === "search"}<Search {nav} {player} {favorites} {playlistActions} />
      {:else if nav.route.name === "library"}<Library {nav} {player} {favorites} {playlistActions} {toasts} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} {player} {favorites} {playlistActions} />
      {:else if nav.route.name === "playlist"}<Playlist {nav} id={nav.route.id} {player} {favorites} {playlistActions} {toasts} />
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {favorites} />
      {:else if nav.route.name === "settings"}<Settings {player} {toasts} onQualityDisplay={(mode) => (qualityDisplay = mode)} />{/if}
    </main>
    {#if nav.queueOpen}
      {#if queueOverlay}
        <div class="queue-backdrop" onclick={() => nav.toggleQueue()} aria-hidden="true"></div>
        <aside class="queue floating"><QueueRail {player} /></aside>
      {:else}
        <aside class="queue">
          <div class="queue-resize" onpointerdown={startQueueDrag} role="separator" aria-orientation="vertical" aria-label="Resize queue"></div>
          <QueueRail {player} />
        </aside>
      {/if}
    {/if}
    <PlayerBar {player} {qualityDisplay} {favorites} {nav} />
    <PlaylistPicker actions={playlistActions} onnavigate={(id) => nav.goto({ name: "playlist", id })} />
  </div>
{/if}

<svelte:window onkeydown={onKeydown} onmousedown={onSideButton} onauxclick={onSideButton} />
<ToastHost {toasts} />
<ContextMenu {menu} />
