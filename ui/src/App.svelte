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
  import { api } from "./lib/api";

  const nav = createNav();
  let loggedIn = $state(false);

  $effect(() => {
    api.authStatus().then((v) => (loggedIn = v));
  });

  function onLogin() { loggedIn = true; }
</script>

{#if !loggedIn}
  <Login onLogin={onLogin} />
{:else}
  <div class="app">
    <Sidebar {nav} />
    <main class="main">
      {#if nav.route.name === "home"}<Home {nav} />
      {:else if nav.route.name === "search"}<Search {nav} />
      {:else if nav.route.name === "library"}<Library {nav} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} />
      {:else if nav.route.name === "playlist"}<Playlist id={nav.route.id} />
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} />{/if}
    </main>
    {#if nav.queueOpen}<aside class="queue"><QueueRail /></aside>{/if}
    <PlayerBar />
  </div>
{/if}
