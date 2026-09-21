<script lang="ts">
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
  import { createNav } from "./lib/nav.svelte";
  import { createPlayer } from "./lib/player.svelte";
  import { api } from "./lib/api";

  const nav = createNav();
  const player = createPlayer();
  let loggedIn = $state(false);
  let booting = $state(true);
  let error = $state("");
  let playerReady = $state(false);

  $effect(() => {
    if (loggedIn && !playerReady) {
      playerReady = true;
      player.init();
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

  function onLogin() {
    error = "";
    loggedIn = true;
  }

  async function onLogout() {
    try {
      await api.authLogout();
    } catch (e) {
      error = String(e);
    }
    loggedIn = false;
  }
</script>

{#if booting}
  <p class="muted" style="padding:24px">Loading…</p>
{:else if !loggedIn}
  {#if error}<p style="color:#ff6b6b;padding:0 24px">{error}</p>{/if}
  <Login onLogin={onLogin} />
{:else}
  <div class="app">
    <Sidebar {nav} onLogout={onLogout} />
    <main class="main">
      {#if nav.route.name === "home"}<Home {nav} />
      {:else if nav.route.name === "search"}<Search {nav} />
      {:else if nav.route.name === "library"}<Library {nav} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} />
      {:else if nav.route.name === "playlist"}<Playlist id={nav.route.id} />
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} />{/if}
    </main>
    {#if nav.queueOpen}<aside class="queue"><QueueRail {player} /></aside>{/if}
    <PlayerBar {player} />
  </div>
{/if}
