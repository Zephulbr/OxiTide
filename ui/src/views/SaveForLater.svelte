<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type LaterEntry, type LaterKind, type PageItem, type Track } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { errorMessage } from "../lib/errors";
  import { later } from "../lib/later.svelte";
  import { laterEntryKey, laterId, laterPageItem, LATER_CHIPS, MAX_LATER_PAGES } from "../lib/saveForLater";
  import { menuFor, type MenuItem } from "../lib/menu.svelte";
  import { playSource, type PlayFrom } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { createTrackView, visibleIndexOf } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
    kind,
  }: {
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
    /// The type chip; absent shows everything.
    kind?: LaterKind;
  } = $props();

  /// The player bar's "From" line for plays started here.
  const FROM: PlayFrom = { kind: "section", name: "Save for Later", route: { name: "later" } };

  // Spec SL-D6: the list is edited by the user, so every visit walks it fresh
  // (newest first) and pages render as they arrive.
  let entries = $state<LaterEntry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let videos = $state(0);
  let run = 0;

  async function load() {
    const mine = ++run;
    loading = true;
    error = "";
    entries = [];
    videos = 0;
    let cursor: string | null | undefined = null;
    try {
      for (let page = 0; page < MAX_LATER_PAGES; page += 1) {
        const result = await api.getSaveForLaterPage(cursor);
        if (mine !== run) return;
        entries = [...entries, ...result.items];
        videos += result.videos;
        // The first page is enough to draw; later ones append.
        loading = false;
        if (!result.cursor || result.cursor === cursor) break;
        cursor = result.cursor;
      }
    } catch (e) {
      if (mine === run) error = errorMessage(e);
    } finally {
      if (mine === run) loading = false;
    }
  }

  $effect(() => {
    untrack(load);
    return () => {
      run += 1;
    };
  });

  /// Removals made anywhere (a menu here or elsewhere) drop off at once.
  /// Before the saved ids hydrate, everything loaded is shown.
  const shown = $derived.by(() => {
    later.revision;
    if (!later.hydrated) return entries;
    return entries.filter((entry) => later.has(entry.kind, laterId(entry)));
  });

  const counts = $derived.by(() => {
    const out: Record<LaterKind, number> = { track: 0, album: 0, artist: 0, playlist: 0, mix: 0 };
    for (const entry of shown) out[entry.kind] += 1;
    return out;
  });

  /// Chips with nothing behind them are hidden; a route chip that emptied
  /// falls back to All.
  const chips = $derived(LATER_CHIPS.filter((chip) => counts[chip.kind] > 0));
  const active = $derived(kind && counts[kind] > 0 ? kind : undefined);
  const filtered = $derived(active ? shown.filter((entry) => entry.kind === active) : shown);

  const tracks = $derived(
    shown.flatMap((entry) => (entry.kind === "track" ? [entry.item] : [])),
  );
  const view = createTrackView(() => tracks, async () => {}, { memoryKey: "later" });

  function choose(next: LaterKind | undefined) {
    nav.replace(next ? { name: "later", kind: next } : { name: "later" });
  }

  async function playTracks(list: readonly Track[], start: number) {
    const prepared = await trackFlags.filterPlayable([...list], start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(FROM, prepared.items.map((t) => t.id));
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  function playFromTable(index: number) {
    const clicked = view.items[index];
    const start = visibleIndexOf(view.items, clicked, index);
    if (start >= 0) void playTracks(view.items, start);
  }

  /// A track card plays the saved tracks from that one.
  function playTrack(track: Track) {
    const start = tracks.indexOf(track);
    void playTracks(tracks, Math.max(0, start));
  }

  function open(entry: LaterEntry) {
    if (entry.kind === "track") playTrack(entry.item);
    else if (entry.kind === "album") nav.goto({ name: "album", id: entry.item.id });
    else if (entry.kind === "artist") nav.goto({ name: "artist", id: entry.item.id });
    else if (entry.kind === "mix") nav.goto({ name: "mix", id: entry.item.id });
    else nav.goto({ name: "playlist", id: entry.item.uuid });
  }

  /// Track and artist cards have no card menu of their own (collection cards
  /// do, with Save for later in it), so the page gives them one.
  function cellMenu(entry: LaterEntry): MenuItem[] {
    const remove: MenuItem = {
      id: "later",
      label: "Remove from Save for later",
      icon: "bookmark-minus",
      divider: true,
      run: () => void later.toggle(entry.kind, laterId(entry)),
    };
    if (entry.kind === "track") {
      const track = entry.item;
      const items: MenuItem[] = [
        { id: "play", label: "Play", icon: "play", run: () => playTrack(track) },
        {
          id: "add-playlist",
          label: "Add to playlist",
          icon: "list-plus",
          run: () => playlistActions.openPicker([track]),
        },
      ];
      if (track.album) {
        const albumId = track.album.id;
        items.push({ id: "go-album", label: "Go to album", icon: "disc-3", divider: true, run: () => nav.goto({ name: "album", id: albumId }) });
      }
      for (const [i, artist] of track.artists.entries()) {
        items.push({
          id: `go-artist-${i}`,
          label: track.artists.length === 1 ? "Go to artist" : `Go to ${artist.name}`,
          icon: "user",
          divider: i === 0 && !track.album,
          run: () => nav.goto({ name: "artist", id: artist.id }),
        });
      }
      return [...items, remove];
    }
    const artistId = entry.kind === "artist" ? entry.item.id : 0;
    return [
      { id: "open", label: "Open", icon: "external-link", run: () => nav.goto({ name: "artist", id: artistId }) },
      remove,
    ];
  }

  const total = $derived(shown.length);
</script>

<div class="later" class:view-fill={active === "track"}>
  <header class="page-head">
    <div class="title-block">
      <h2>Save for Later</h2>
      {#if !loading && !error && total > 0}
        <span class="summary tnum">{total} {total === 1 ? "item" : "items"}</span>
      {/if}
    </div>
  </header>
  {#if chips.length > 1}
    <div class="chips" role="group" aria-label="Show saved">
      <button type="button" class="chip" aria-pressed={active === undefined} onclick={() => choose(undefined)}>All</button>
      {#each chips as chip (chip.kind)}
        <button type="button" class="chip" aria-pressed={active === chip.kind} onclick={() => choose(chip.kind)}>
          {chip.label}
        </button>
      {/each}
    </div>
  {/if}
  {#if loading}
    <Loading />
  {:else if error && shown.length === 0}
    <ErrorState message={error} onRetry={load} />
  {:else if shown.length === 0}
    <EmptyState message="Nothing saved yet. Use Save for later in any track, album, artist, playlist or mix menu." />
  {:else if active === "track"}
    <TrackTable
      {view}
      {nav}
      {favorites}
      {toasts}
      fill
      onPlay={playFromTable}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      onAddManyToPlaylist={(ts) => playlistActions.openPicker(ts)}
      playingTrackId={player.state.current?.track_id ?? null}
      playbackRunning={player.state.playing}
    />
  {:else}
    <div class="grid pane" role="list">
      {#each filtered as entry (laterEntryKey(entry))}
        {@const item: PageItem = laterPageItem(entry)}
        {#if entry.kind === "track" || entry.kind === "artist"}
          <div class="cell" role="listitem" use:menuFor={() => cellMenu(entry)}>
            {#if entry.kind === "track"}<span class="type-badge" aria-hidden="true">Track</span>{/if}
            <MediaCard
              {item}
              variant="tile"
              playing={entry.kind === "track" && player.state.current?.track_id === entry.item.id}
              onclick={() => open(entry)}
            />
          </div>
        {:else}
          <div class="cell" role="listitem">
            <MediaCard
              {item}
              variant="tile"
              collage={entry.kind === "playlist" && entry.item.owned}
              onclick={() => open(entry)}
            />
          </div>
        {/if}
      {/each}
    </div>
  {/if}
  {#if !loading && error && shown.length > 0}
    <ErrorState message={error} onRetry={load} />
  {/if}
  {#if videos > 0}
    <p class="footnote muted">
      {videos} {videos === 1 ? "video" : "videos"} saved in TIDAL {videos === 1 ? "isn't" : "aren't"} shown here.
    </p>
  {/if}
</div>

<style>
  /* Collection Albums' insets and grid steps, so the two pages line up. */
  .later {
    container-type: inline-size;
    padding-inline: 24px;
  }
  .title-block {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title-block h2 {
    margin: 0;
  }
  .summary {
    color: color-mix(in srgb, var(--text) 62%, transparent);
    font-size: 14px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0 0 18px;
    padding: 2px;
  }
  .chip {
    height: 34px;
    padding: 0 18px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-1);
    color: var(--text-muted);
    font-size: 14px;
    transition:
      background var(--dur-state) var(--ease-enter),
      color var(--dur-state) var(--ease-enter);
  }
  .chip:hover,
  .chip:focus-visible {
    background: var(--surface-3);
    color: var(--text);
  }
  .chip[aria-pressed="true"] {
    border-color: transparent;
    background: var(--accent);
    color: var(--on-accent);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols, 8), minmax(0, 1fr));
    gap: 18px 14px;
  }
  @container (max-width: 1240px) {
    .grid { --cols: 7; }
  }
  @container (max-width: 1080px) {
    .grid { --cols: 6; }
  }
  @container (max-width: 920px) {
    .grid { --cols: 5; }
  }
  @container (max-width: 760px) {
    .grid { --cols: 4; }
  }
  @container (max-width: 600px) {
    .grid { --cols: 3; }
  }
  @container (max-width: 440px) {
    .grid { --cols: 2; }
  }
  .cell {
    position: relative;
    min-width: 0;
  }
  /* Track cards say what they are, as on TIDAL's page: the other kinds read
     from their shape (round artists, collage playlists). */
  .type-badge {
    position: absolute;
    z-index: 2;
    top: 16px;
    left: 16px;
    padding: 2px 7px;
    border-radius: var(--radius-control, 6px);
    background: var(--surface-3);
    color: var(--text);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    pointer-events: none;
  }
  .grid :global(.media.tile) {
    padding: 10px;
  }
  .grid :global(.media .title) {
    font-size: 15px;
    font-weight: 600;
  }
  .grid :global(.media .sub) {
    font-size: 13px;
  }
  .footnote {
    margin: 18px 0 0;
    font-size: 13px;
  }
</style>
