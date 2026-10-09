<script lang="ts">
  import Icon from "../lib/components/Icon.svelte";
  import { RemoteActions } from "./actions.svelte";
  import { remoteApi } from "./api";
  import MiniBar from "./components/MiniBar.svelte";
  import Sheet from "./components/Sheet.svelte";
  import TabBar from "./components/TabBar.svelte";
  import { provideRemote } from "./context";
  import Album from "./pages/Album.svelte";
  import Artist from "./pages/Artist.svelte";
  import Favorites from "./pages/Favorites.svelte";
  import Feed from "./pages/Feed.svelte";
  import History from "./pages/History.svelte";
  import Library from "./pages/Library.svelte";
  import Mix from "./pages/Mix.svelte";
  import Mixes from "./pages/Mixes.svelte";
  import NowPlaying from "./pages/NowPlaying.svelte";
  import Playlist from "./pages/Playlist.svelte";
  import Playlists from "./pages/Playlists.svelte";
  import Queue from "./pages/Queue.svelte";
  import Search from "./pages/Search.svelte";
  import { parseHash, tabFor } from "./routes";
  import { createClient, guessDeviceName, pairingToken } from "./rpc";
  import { RemoteStore } from "./store.svelte";

  const client = createClient();
  const api = remoteApi(client.call);
  const store = new RemoteStore({ call: client.call });
  const actions = new RemoteActions(api);
  provideRemote({ api, store, actions });

  let route = $state(parseHash(location.hash));
  let pairing = $state<{ token: string; error: string } | null>(null);

  async function pair(token: string) {
    pairing = { token, error: "" };
    try {
      await client.pair(token, guessDeviceName(navigator.userAgent));
      pairing = null;
      store.start();
    } catch (e) {
      pairing = { token, error: e instanceof Error ? e.message : String(e) };
    }
  }

  $effect(() => {
    const token = pairingToken(location.hash);
    if (token) {
      // Never leave the pairing code in the address bar or history.
      history.replaceState(null, "", `${location.pathname}#/now`);
      route = { name: "now" };
      void pair(token);
    } else {
      store.start();
    }
    const onHash = () => {
      route = parseHash(location.hash);
      actions.sheet = null;
      window.scrollTo(0, 0);
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") store.wake();
    };
    window.addEventListener("hashchange", onHash);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("hashchange", onHash);
      document.removeEventListener("visibilitychange", onVisible);
      store.stop();
    };
  });

  const ready = $derived(store.state !== null && store.phase !== "unpaired");
  const signedOut = $derived(store.state !== null && !store.state.loggedIn);
</script>

{#if pairing}
  <div class="screen">
    <Icon name="disc-3" size={48} />
    {#if pairing.error}
      <h1>Pairing failed</h1>
      <p class="muted">{pairing.error}</p>
      <button class="pill primary" onclick={() => pairing && pair(pairing.token)}>Try again</button>
      <p class="muted small">Or show a new QR code in OxiTide on your PC and scan it again.</p>
    {:else}
      <h1>Pairing…</h1>
      <p class="muted">If OxiTide asks, approve this phone on your PC.</p>
    {/if}
  </div>
{:else if store.phase === "unpaired"}
  <div class="screen">
    <Icon name="disc-3" size={48} />
    <h1>Pair this phone</h1>
    <p class="muted">
      On your PC, open OxiTide → Settings → Connections → <strong>Pair a phone</strong>, then scan
      the QR code with this phone's camera.
    </p>
  </div>
{:else if !ready}
  <div class="screen" aria-busy="true">
    <Icon name="disc-3" size={48} />
    <h1>Connecting to OxiTide…</h1>
    {#if store.phase === "connecting" && store.state === null}
      <p class="muted small">
        Make sure OxiTide is running on your PC with the phone remote on, and that this phone is
        on the same Wi-Fi.
      </p>
    {/if}
  </div>
{:else if signedOut}
  <div class="screen">
    <Icon name="log-out" size={48} />
    <h1>Sign in on the PC</h1>
    <p class="muted">{store.session}</p>
  </div>
{:else}
  <div class="shell">
    {#if store.phase === "reconnecting"}
      <div class="banner" role="status">Reconnecting to your PC…</div>
    {:else if store.session}
      <div class="banner" role="status">{store.session}</div>
    {/if}
    <main>
      {#if route.name === "now"}
        <NowPlaying />
      {:else if route.name === "queue"}
        <Queue />
      {:else if route.name === "search"}
        <Search />
      {:else if route.name === "home" || route.name === "explore" || route.name === "foryou"}
        <Feed source={route.name} />
      {:else if route.name === "mixes"}
        <Mixes />
      {:else if route.name === "library"}
        <Library />
      {:else if route.name === "history"}
        <History />
      {:else if route.name === "favorites"}
        <Favorites kind={route.kind} />
      {:else if route.name === "playlists"}
        <Playlists folder={route.folder} />
      {:else if route.name === "album"}
        <Album id={route.id} />
      {:else if route.name === "artist"}
        <Artist id={route.id} />
      {:else if route.name === "playlist"}
        <Playlist id={route.id} />
      {:else if route.name === "mix"}
        <Mix id={route.id} />
      {/if}
    </main>
    <footer class="dock">
      {#if route.name !== "now"}<MiniBar />{/if}
      <TabBar active={tabFor(route)} />
    </footer>
    <Sheet />
    {#if actions.toast}
      <div class="toast" role="status">{actions.toast}</div>
    {/if}
  </div>
{/if}

<style>
  .screen {
    min-height: 100vh;
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 32px;
    text-align: center;
    color: var(--text);
  }
  .screen :global(svg) {
    color: var(--accent);
  }
  .screen h1 {
    font-size: 26px;
  }
  .screen p {
    margin: 0;
    max-width: 34ch;
  }
  .small {
    font-size: 13px;
  }
  main {
    padding-top: env(safe-area-inset-top);
    padding-bottom: calc(var(--remote-tabbar) + var(--remote-minibar) + env(safe-area-inset-bottom) + 8px);
  }
  .dock {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 20;
  }
  .banner {
    position: sticky;
    top: 0;
    z-index: 30;
    padding: calc(6px + env(safe-area-inset-top)) 16px 6px;
    background: color-mix(in srgb, var(--warning) 22%, var(--bg));
    color: var(--text);
    font-size: 13px;
    text-align: center;
  }
  .toast {
    position: fixed;
    left: 16px;
    right: 16px;
    bottom: calc(var(--remote-tabbar) + var(--remote-minibar) + env(safe-area-inset-bottom) + 12px);
    z-index: 50;
    padding: 12px 16px;
    border-radius: var(--radius-control);
    border: 1px solid var(--line);
    background: var(--surface-float);
    backdrop-filter: blur(var(--overlay-blur));
    -webkit-backdrop-filter: blur(var(--overlay-blur));
    text-align: center;
  }
</style>
