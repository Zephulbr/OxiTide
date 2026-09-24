<script lang="ts">
  import { untrack } from "svelte";
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
  import PlaylistPicker from "./lib/components/PlaylistPicker.svelte";
  import { createNav } from "./lib/nav.svelte";
  import { createPlayer } from "./lib/player.svelte";
  import { createFavorites } from "./lib/favorites.svelte";
  import { createNotice } from "./lib/notice.svelte";
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
  const favorites = createFavorites();
  const notice = createNotice();
  const playlistActions = createPlaylistActions(notice);
  let qualityDisplay = $state<QualityDisplay>("compact");
  let loggedIn = $state(false);
  let booting = $state(true);
  let error = $state("");
  let playerReady = $state(false);
  let expired = $state<ExpiredSession | null>(null);

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
    if (!favorites.error) return;
    const timer = setTimeout(() => favorites.dismissError(), 6000);
    return () => clearTimeout(timer);
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
      error = String(e);
    }
    clearPagerCache();
    favorites.reset();
    playlistActions.reset();
    loggedIn = false;
  }
</script>

{#if booting}
  <p class="muted" style="padding:24px">Loading…</p>
{:else if !loggedIn}
  {#if error}<p style="color:#ff6b6b;padding:0 24px">{error}</p>{/if}
  <Login
    onLogin={onLogin}
    message={expired ? "Session expired — sign in to continue" : ""}
  />
{:else}
  <div class="app">
    <Sidebar {nav} onLogout={onLogout} />
    <main class="main">
      {#if favorites.error}
        <div class="notice">
          <span>{favorites.error}</span>
          <button onclick={() => favorites.dismissError()} aria-label="Dismiss">✕</button>
        </div>
      {/if}
      {#if notice.message}
        <div class="notice">
          <span>{notice.message}</span>
          <button onclick={() => notice.dismiss()} aria-label="Dismiss">✕</button>
        </div>
      {/if}
      {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
      {#if nav.route.name === "home"}<Home {nav} />
      {:else if nav.route.name === "search"}<Search {nav} {player} {favorites} {playlistActions} />
      {:else if nav.route.name === "library"}<Library {nav} {player} {favorites} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} {player} {favorites} {playlistActions} />
      {:else if nav.route.name === "playlist"}<Playlist id={nav.route.id} {player} {favorites} />
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {favorites} />
      {:else if nav.route.name === "settings"}<Settings {player} onQualityDisplay={(mode) => (qualityDisplay = mode)} />{/if}
    </main>
    {#if nav.queueOpen}<aside class="queue"><QueueRail {player} /></aside>{/if}
    <PlayerBar {player} {qualityDisplay} {favorites} />
    <PlaylistPicker actions={playlistActions} onnavigate={(id) => nav.goto({ name: "playlist", id })} />
  </div>
{/if}

<style>
  .notice {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 0 0 12px;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
    color: #ff6b6b;
  }
  .notice button {
    margin-left: auto;
  }
</style>
