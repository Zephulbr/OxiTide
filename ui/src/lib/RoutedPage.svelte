<script lang="ts">
  import Home from "../views/Home.svelte";
  import Album from "../views/Album.svelte";
  import Playlist from "../views/Playlist.svelte";
  import Artist from "../views/Artist.svelte";
  import ArtistAll from "../views/ArtistAll.svelte";
  import Explore from "../views/Explore.svelte";
  import Feed from "../views/Feed.svelte";
  import History from "../views/History.svelte";
  import CollectionAlbums from "../views/CollectionAlbums.svelte";
  import CollectionTracks from "../views/CollectionTracks.svelte";
  import Profiles from "../views/Profiles.svelte";
  import Playlists from "../views/Playlists.svelte";
  import Mixes from "../views/Mixes.svelte";
  import Mix from "../views/Mix.svelte";
  import Purchases from "../views/Purchases.svelte";
  import Settings from "../views/Settings.svelte";
  import NowPlaying from "../views/NowPlaying.svelte";
  import Section from "../views/Section.svelte";
  import TidalPage from "../views/TidalPage.svelte";
  import type { Nav } from "./nav.svelte";
  import type { Player } from "./player.svelte";
  import type { Favorites } from "./favorites.svelte";
  import type { PlaylistActions } from "./playlists.svelte";
  import type { CollectionTree } from "./collection.svelte";
  import type { CollectionActions } from "./collectionActions.svelte";
  import type { Toasts } from "./toasts.svelte";
  import type { QualityDisplay } from "./quality";

  /// The routed page, shared by the default shell and the Hi-Fi Rack shell.
  /// `nowPlaying` is false in Rack, whose deck expands instead (RK-D4).
  let {
    nav,
    player,
    favorites,
    playlistActions,
    collection,
    collectionActions,
    toasts,
    qualityDisplay,
    nowPlaying = true,
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
    nowPlaying?: boolean;
    onLogout: () => void;
    onQualityDisplay: (mode: QualityDisplay) => void;
  } = $props();

  // Route identity for the page-enter animation. ArtistAll tab switches are
  // deliberately excluded: the tab underline is that surface's only motion.
  const pageKey = $derived.by(() => {
    const route = nav.route;
    if (route.name === "section") return `section:${route.source}:${route.title}`;
    if (route.name === "page") return `page:${route.path}`;
    if ("id" in route) return `${route.name}:${route.id}`;
    return route.name;
  });
</script>

{#key pageKey}
  <div class="page">
    {#if nowPlaying && nav.nowPlayingOpen}<NowPlaying {player} {nav} {qualityDisplay} />
    {:else if nav.route.name === "home"}<Home {nav} {player} {toasts} />
    {:else if nav.route.name === "explore"}<Explore {nav} {player} {favorites} {playlistActions} {toasts} />
    {:else if nav.route.name === "feed"}<Feed {nav} />
    {:else if nav.route.name === "history"}<History {nav} {player} {toasts} tab={nav.route.tab} />
    {:else if nav.route.name === "albums"}<CollectionAlbums {nav} />
    {:else if nav.route.name === "tracks"}<CollectionTracks {nav} {player} {favorites} {playlistActions} {toasts} />
    {:else if nav.route.name === "profiles"}<Profiles {nav} />
    {:else if nav.route.name === "mixes"}<Mixes {nav} {player} {toasts} />
    {:else if nav.route.name === "purchases"}<Purchases {nav} />
    {:else if nav.route.name === "playlists"}<Playlists {nav} {playlistActions} actions={collectionActions} />
    {:else if nav.route.name === "folder"}<Playlists {nav} {playlistActions} actions={collectionActions} folderId={nav.route.id} folderLabel={nav.route.label ?? ""} />
    {:else if nav.route.name === "section"}<Section source={nav.route.source} title={nav.route.title} {nav} {player} {toasts} />
    {:else if nav.route.name === "page"}<TidalPage path={nav.route.path} title={nav.route.title} {nav} {player} {toasts} />
    {:else if nav.route.name === "album"}<Album id={nav.route.id} {nav} {player} {favorites} {playlistActions} {toasts} />
    {:else if nav.route.name === "mix"}<Mix id={nav.route.id} {nav} {player} {favorites} {playlistActions} {toasts} />
    {:else if nav.route.name === "playlist"}<Playlist {nav} id={nav.route.id} {player} {favorites} {playlistActions} {toasts} {collection} />
    {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {nav} {player} {favorites} {playlistActions} {toasts} />
    {:else if nav.route.name === "artist-all"}{#key nav.route.id}<ArtistAll id={nav.route.id} tab={nav.route.tab} {nav} {player} {favorites} {playlistActions} {toasts} />{/key}
    {:else if nav.route.name === "settings"}<Settings {player} {toasts} {onLogout} {onQualityDisplay} />{/if}
  </div>
{/key}
