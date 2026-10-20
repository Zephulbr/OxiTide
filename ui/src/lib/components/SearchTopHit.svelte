<script lang="ts">
  import { api, trackToQueueItem, type ArtistRef, type TopHit, type Track } from "../api";
  import { getCardActions } from "../cardActions";
  import { isAlbumRelease } from "../media";
  import type { Nav, Route } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import type { Toasts } from "../toasts.svelte";
  import { trackFlags } from "../trackFlags.svelte";
  import ArtistLinks from "./ArtistLinks.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";

  let { hit, nav, toasts }: { hit: TopHit; nav: Nav; toasts: Toasts } = $props();

  const cardActions = getCardActions();

  /// What the card says and where it goes, per kind.
  const view = $derived.by(() => {
    switch (hit.kind) {
      case "artist":
        return {
          title: hit.item.name,
          label: "Artist",
          artists: [] as ArtistRef[],
          detail: "",
          route: { name: "artist", id: hit.item.id } as Route,
        };
      case "album": {
        const year = hit.item.releaseDate?.slice(0, 4) ?? "";
        const type = hit.item.type?.toUpperCase();
        return {
          title: hit.item.title,
          label: type === "EP" ? "EP" : type === "SINGLE" ? "Single" : "Album",
          artists: hit.item.artists,
          detail: /^\d{4}$/.test(year) ? year : "",
          route: { name: "album", id: hit.item.id } as Route,
        };
      }
      case "track":
        return {
          title: hit.item.title,
          label: "Track",
          artists: hit.item.artists,
          detail: hit.item.album?.title ?? "",
          route: hit.item.album ? ({ name: "album", id: hit.item.album.id } as Route) : null,
        };
      case "playlist":
        return {
          title: hit.item.title,
          label: "Playlist",
          artists: [] as ArtistRef[],
          detail: `${hit.item.numberOfTracks} ${hit.item.numberOfTracks === 1 ? "track" : "tracks"}`,
          route: { name: "playlist", id: hit.item.uuid } as Route,
        };
    }
  });

  async function playTracks(tracks: Track[]) {
    const prepared = await trackFlags.filterPlayable(tracks, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  /// An artist plays their top tracks (as on their page); an album or
  /// playlist plays through the shared card actions; a track plays alone.
  async function play() {
    try {
      if (hit.kind === "artist") await playTracks(await api.getArtistTopTracks(hit.item.id, 50));
      else if (hit.kind === "track") await playTracks([hit.item]);
      else if (hit.kind === "album") await cardActions?.play({ kind: "album", id: hit.item.id });
      else await cardActions?.play({ kind: "playlist", id: hit.item.uuid });
    } catch (e) {
      toasts.show(String(e), { severity: "error" });
    }
  }

  function open() {
    if (view.route) nav.goto(view.route);
  }
</script>

<!-- One card, several targets: the card opens the page (a stretched button
     under the rest), the artists open theirs, and Play plays. -->
<div class="top-hit" class:round={hit.kind === "artist"}>
  <button type="button" class="open" aria-label={`Open ${view.title}`} onclick={open} disabled={!view.route}></button>
  <span class="art-wrap">
    <span class="art" aria-hidden="true">
      {#if hit.kind === "artist"}
        <Cover uuid={hit.item.picture ?? hit.item.cover_fallback} size={320} radius={999} alt="" />
      {:else if hit.kind === "album"}
        <Cover uuid={hit.item.cover} size={320} radius={isAlbumRelease(hit.item.type) ? 0 : 6} alt="" />
      {:else if hit.kind === "track"}
        <Cover uuid={hit.item.album?.cover} size={320} radius={6} alt="" />
      {:else}
        <PlaylistCover owned={hit.item.owned} playlist={hit.item.uuid} image={hit.item.squareImage} size={320} radius={6} alt="" />
      {/if}
    </span>
    <button type="button" class="play" aria-label={`Play ${view.title}`} title="Play" onclick={play}>
      <Icon name="play" size={22} />
    </button>
  </span>
  <span class="text">
  <span class="title">{view.title}</span>
  <span class="meta">
    <span class="label">{view.label}</span>
    {#if view.artists.length > 0}
      <span class="artists"><ArtistLinks artists={view.artists} onOpen={(id) => nav.goto({ name: "artist", id })} /></span>
    {/if}
    {#if view.detail}<span class="detail">{view.detail}</span>{/if}
  </span>
  </span>
</div>

<style>
  /* No panel: the cover with the name and label beside it, as one group
     centred in the column next to the tracks. */
  .top-hit {
    position: relative;
    isolation: isolate;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 24px;
    height: 100%;
    min-width: 0;
    padding: 12px;
    border-radius: var(--radius-card);
  }
  .text {
    display: flex;
    flex: 0 1 auto;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
  }
  /* The whole card opens the page; links and Play sit above it. */
  .open {
    position: absolute;
    inset: 0;
    z-index: 0;
    border-radius: inherit;
    cursor: pointer;
  }
  .open:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .art-wrap {
    position: relative;
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    pointer-events: none;
  }
  .art {
    display: block;
    width: 152px;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--radius-card-sharp);
    box-shadow: 0 0 0 1px var(--line), var(--shadow-cover);
    transition: transform var(--dur-state) var(--ease-enter);
  }
  /* The cover lifts on hover, as the cards do. */
  .top-hit:hover .art,
  .top-hit:focus-within .art {
    transform: translateY(-3px);
  }
  .round .art {
    border-radius: 50%;
  }
  .title {
    display: -webkit-box;
    max-width: 100%;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    line-height: 1.1;
    pointer-events: none;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    max-width: 100%;
    color: color-mix(in srgb, var(--text) 62%, transparent);
    font-size: 13px;
  }
  .label {
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--text);
    font-size: 11.5px;
    font-weight: 600;
    pointer-events: none;
  }
  .artists {
    position: relative;
    z-index: 1;
    min-width: 0;
  }
  .detail {
    pointer-events: none;
  }
  /* Play sits on the centre of the cover, growing in on hover or keyboard
     focus (it lifts with the cover). */
  .play {
    position: absolute;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    color: var(--on-accent);
    background: var(--accent);
    box-shadow:
      0 0 14px color-mix(in srgb, var(--accent) 45%, transparent),
      0 4px 14px rgb(0 0 0 / 0.35);
    opacity: 0;
    transform: scale(0.85);
    pointer-events: auto;
    transition:
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-state) var(--ease-enter),
      filter var(--dur-micro) var(--ease-enter);
  }
  .top-hit:hover .play,
  .top-hit:focus-within .play {
    opacity: 1;
    transform: translateY(-3px);
  }
  .play:hover {
    filter: brightness(1.08);
  }
  .play:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .play,
    .top-hit:hover .play,
    .top-hit:hover .art,
    .top-hit:focus-within .play,
    .top-hit:focus-within .art {
      transform: none;
    }
  }
</style>
