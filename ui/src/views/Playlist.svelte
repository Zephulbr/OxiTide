<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import DetailHeader from "../lib/components/DetailHeader.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import Modal from "../lib/components/Modal.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import TrackSearch from "../lib/components/TrackSearch.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { copyText } from "../lib/clipboard";
  import { formatTotalDuration } from "../lib/format";
  import { menu, type MenuItem } from "../lib/menu.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { trackFlags } from "../lib/trackFlags.svelte";
  import { invalidatePlaylistCovers } from "../lib/playlistArt.svelte";
  import { shareUrl } from "../lib/share";
  import { createTrackView, visibleIndexOf, warmTrackSearch } from "../lib/trackView.svelte";
  import { invalidate, load as loadCached, peek } from "../lib/viewCache";
  import { playlistSource } from "../lib/viewData";
  import type { CollectionTree } from "../lib/collection.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { errorMessage } from "../lib/errors";
  import type { Toasts } from "../lib/toasts.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    id,
    player,
    favorites,
    playlistActions,
    toasts,
    collection,
  }: {
    nav: Nav;
    id: string;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
    collection: CollectionTree;
  } = $props();

  // A cached visit renders on the first frame and revalidates in the
  // background (lib/viewCache.ts); the track list has its own pager cache.
  // svelte-ignore state_referenced_locally
  const source = playlistSource(id);
  const cached = peek(source);
  let playlist = $state<PlaylistT | null>(cached ?? null);
  let loading = $state(cached === undefined);
  let error = $state("");
  let renaming = $state(false);
  let renameTitle = $state("");
  let renameDescription = $state("");
  let confirmingDelete = $state(false);
  let busy = $state(false);
  let publicState = $state<boolean | null>(cached?.publicPlaylist ?? null);
  let publicBusy = $state(false);

  // `$derived` so navigating to another playlist rebuilds the pager; the new
  // key hydrates from the session cache when that playlist was seen before.
  const pager = $derived(
    createTrackPager(
      `playlist:${id}`,
      (offset, limit) => api.getPlaylistTracksPage(id, offset, limit),
      { dedupe: false, prefetchWindow: 100 },
    ),
  );

  // Rebuild the view when the pager is swapped for another playlist; reading
  // `pager` here makes that reactive (the closures alone would not).
  const view = $derived.by(() => {
    const p = pager;
    return createTrackView(() => p.items, async () => {
      await p.ensureAll();
      if (p.error) throw p.error;
    });
  });

  // "180 TRACKS (10:16:08) • 1 FAN" — server totals with loaded-item fallback.
  const stats = $derived.by(() => {
    if (!playlist) return "";
    const count = playlist.numberOfTracks || pager.items.length;
    const seconds =
      playlist.duration || pager.items.reduce((sum, t) => sum + t.duration, 0);
    const tracks = `${count} ${count === 1 ? "TRACK" : "TRACKS"}`;
    const parts = [seconds > 0 ? `${tracks} (${formatTotalDuration(seconds)})` : tracks];
    const followers = playlist.numberOfFollowers;
    if (followers != null) parts.push(`${followers} ${followers === 1 ? "FAN" : "FANS"}`);
    if (pager.unavailable > 0) parts.push(`${pager.unavailable} UNAVAILABLE`);
    return parts.join(" • ");
  });

  async function load() {
    if (peek(source) === undefined) loading = true;
    error = "";
    try {
      await loadCached(source, (loaded) => {
        playlist = loaded;
        publicState = loaded.publicPlaylist ?? null;
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    untrack(load);
  });

  $effect(() => {
    const current = pager;
    untrack(() => current.loadMore());
  });

  // Warm the filter/sort caches in idle chunks for large playlists (see
  // CollectionTracks); the first keystroke must not pay the whole build.
  $effect(() => {
    const list = pager.items;
    if (list.length < 500) return;
    return warmTrackSearch(list).cancel;
  });

  function startRename() {
    renaming = true;
    renameTitle = playlist?.title ?? "";
    renameDescription = playlist?.description ?? "";
  }

  async function saveRename() {
    const title = renameTitle.trim();
    if (!title) return;
    try {
      await api.renamePlaylist(id, title, renameDescription.trim());
      invalidate(source.key);
      if (playlist) {
        playlist.title = title;
        playlist.description = renameDescription.trim() || null;
      }
      renaming = false;
      toasts.show("Playlist renamed", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function togglePublic() {
    if (publicBusy || publicState === null) return;
    const next = !publicState;
    publicBusy = true;
    publicState = next;
    try {
      await api.setPlaylistPublic(id, next);
      invalidate(source.key);
      if (playlist) playlist.publicPlaylist = next;
      toasts.show(next ? "Playlist is now public" : "Playlist is now private", {
        severity: "success",
      });
    } catch (e) {
      publicState = !next;
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      publicBusy = false;
    }
  }

  async function share() {
    const copied = await copyText(shareUrl("playlist", id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
  }

  function openMore(event: MouseEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const items: MenuItem[] = [
      {
        id: "refresh",
        label: "Refresh",
        icon: "rotate-cw",
        run: () => {
          invalidatePlaylistCovers(id);
          invalidate(source.key);
          void pager.refresh();
        },
      },
    ];
    if (playlist?.owned) {
      items.push({
        id: "delete",
        label: "Delete playlist",
        icon: "trash-2",
        danger: true,
        divider: true,
        run: () => (confirmingDelete = true),
      });
    }
    menu.show(rect.left, rect.bottom + 4, items, event.currentTarget as HTMLElement);
  }

  async function confirmDelete() {
    confirmingDelete = false;
    try {
      await api.deletePlaylist(id);
      invalidate(source.key);
      invalidatePlaylistCovers(id);
      await collection.refresh();
      toasts.show("Playlist deleted", { severity: "success" });
      nav.goto({ name: "playlists" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function removeAt(index: number) {
    if (busy) return;
    const sourceIndex = view.sourceIndex(index);
    if (sourceIndex < 0) return;
    busy = true;
    try {
      await api.removePlaylistItems(id, [sourceIndex]);
      invalidatePlaylistCovers(id);
      await pager.refresh();
      toasts.show("Removed from playlist", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function moveTrack(from: number, to: number) {
    if (busy || from === to) return;
    busy = true;
    pager.moveItem(from, to);
    try {
      await api.movePlaylistItems(id, [from], to);
      invalidatePlaylistCovers(id);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      await pager.refresh();
    } finally {
      busy = false;
    }
  }

  async function playFrom(index: number) {
    const clicked = view.items[index];
    await pager.ensureAll();
    if (pager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(id);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(id);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    await pager.ensureAll();
    if (pager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(id);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else}
  <div class="view-fill">
    {#if playlist}
      {@const data = playlist}
      <DetailHeader
        kicker="Playlist"
        title={data.title}
        {stats}
        onPlay={playAll}
        onShuffle={shuffleAll}
        disabled={view.items.length === 0}
      >
        {#snippet cover()}
          <PlaylistCover
            owned={data.owned}
            playlist={id}
            image={data.squareImage}
            tracks={pager.items}
            size={640}
            radius={12}
            alt={`${data.title} cover`}
          />
        {/snippet}
        {#snippet tools()}
          <TrackSearch {view} />
        {/snippet}
        {#snippet actions()}
          {#if data.owned}
            <DetailAction
              icon="pencil"
              label="Edit"
              title="Edit playlist"
              onclick={startRename}
            />
            {#if publicState !== null}
              <DetailAction
                icon="globe"
                label={publicState ? "Public" : "Private"}
                active={publicState}
                title={publicState ? "Make private" : "Make public"}
                disabled={publicBusy}
                onclick={togglePublic}
              />
            {/if}
          {/if}
          <DetailAction icon="share-2" label="Share" title="Copy link" onclick={share} />
          <DetailAction icon="ellipsis" label="More" title="More actions" onclick={openMore} />
        {/snippet}
      </DetailHeader>
    {/if}
    {#if renaming}
      <Modal title="Edit playlist" onclose={() => (renaming = false)}>
        <div class="rename-form">
          <label>
            <span>Name</span>
            <input class="search-input" bind:value={renameTitle} aria-label="Playlist name" />
          </label>
          <label>
            <span>Description</span>
            <textarea bind:value={renameDescription} aria-label="Playlist description"></textarea>
          </label>
          <div class="rename-actions">
            <button class="ghost" onclick={() => (renaming = false)}>Cancel</button>
            <button class="primary" onclick={saveRename}>Save</button>
          </div>
        </div>
      </Modal>
    {/if}
    <TrackTable
      {view}
      {nav}
      {favorites}
      {toasts}
      fill
      showFilter={false}
      onPlay={playFrom}
      onEndReached={() => void pager.loadMore()}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      playingTrackId={player.state.current?.track_id ?? null}
      playbackRunning={player.state.playing}
      playlistActions={playlist?.owned
        ? { onRemove: removeAt, busy, disabled: !pager.done, onDrop: moveTrack }
        : undefined}
    />
    {#if pager.error}
      <ErrorState message={pager.error} onRetry={() => pager.loadMore()} />
    {/if}
  </div>
  {#if confirmingDelete}
    <ConfirmDialog
      title="Delete playlist?"
      message={`Delete "${playlist?.title ?? "this playlist"}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (confirmingDelete = false)}
    />
  {/if}
{/if}

<style>
  .rename-form {
    display: grid;
    gap: 12px;
    min-width: min(420px, 76vw);
  }
  .rename-form label {
    display: grid;
    gap: 6px;
    font-size: 13px;
    color: var(--text-muted);
  }
  .rename-form textarea {
    min-height: 96px;
    resize: vertical;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
  }
  .rename-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .rename-actions button {
    height: 38px;
    padding: 0 18px;
    border-radius: var(--radius-control);
    font-weight: 600;
  }
  .rename-actions .ghost {
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .rename-actions .ghost:hover {
    color: var(--text);
  }
  .rename-actions .primary {
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
