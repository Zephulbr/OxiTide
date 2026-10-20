<script lang="ts">
  import { api, type QueueItem } from "../api";
  import { formatDuration } from "../format";
  import type { Route } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import { copyText } from "../clipboard";
  import { menuFor } from "../menu.svelte";
  import { queueMenuItems } from "../menus";
  import { albumRoute, goToTargets } from "../queueLinks";
  import NameLink from "./NameLink.svelte";
  import QueueArtistLinks from "./QueueArtistLinks.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlayPauseGlyph from "./PlayPauseGlyph.svelte";

  /// "Continue listening" as the first tiles of Home's Jump back in grid: the
  /// current (or restored) track spanning two slots, and the next queued
  /// track in the third. Renders grid items directly (no wrapper), so it must
  /// sit inside the `.shortcuts` grid; nothing renders without a track.
  let {
    player,
    onerror,
    go,
  }: {
    player: Player;
    onerror?: (e: unknown) => void;
    /// Makes the current track's title and artist links (name-links spec).
    go?: (route: Route) => void;
  } = $props();

  async function openAlbum(item: QueueItem, open: (route: Route) => void) {
    const route = await albumRoute(item, api.getTrack);
    if (route) open(route);
  }

  const current = $derived(player.state.current);
  const playing = $derived(player.state.playing);
  const progress = $derived(
    player.state.duration_ms > 0
      ? Math.min(1, player.state.position_ms / player.state.duration_ms)
      : 0,
  );
  const queue = $derived(player.state.queue);
  const index = $derived(player.state.current_index ?? -1);
  const upNext = $derived(index >= 0 ? queue[index + 1] : undefined);

  function toggle() {
    void Promise.resolve(player.playPause()).catch((e) => onerror?.(e));
  }

  function playAt(at: number) {
    void api.playTracks(queue, at).catch((e) => onerror?.(e));
  }

  function playNext() {
    playAt(index + 1);
  }

  /// The queue rail's row menu, for the current track and the next one
  /// (neither has a "Play next": they already are, or are playing). Built at
  /// right-click time: `menuFor` keeps the closure it was mounted with, so
  /// the closures below read the live track and index.
  function menuItems(item: QueueItem | null | undefined, at: number) {
    if (!item) return [];
    return queueMenuItems({
      index: at,
      playNow: () => playAt(at),
      copyTitle: () => void copyText(item.title),
      remove: () => void Promise.resolve(player.remove(at)).catch((e) => onerror?.(e)),
      ...(go ? goToTargets(item, go, api.getTrack) : { goToArtists: [] }),
    });
  }
</script>

{#if current}
<div class="continue">
  <section
    class="resume"
    aria-label={playing ? "Now playing" : "Continue listening"}
    use:menuFor={() => menuItems(current, index)}
  >
    <div class="art">
      <Cover uuid={current.cover} size={160} radius={6} alt="" />
      <!-- The cover's play glyph, as on Home's other cards (MediaCard). -->
      <button
        type="button"
        class="play-glyph"
        onclick={toggle}
        aria-label={playing ? "Pause" : "Resume"}
        title={playing ? "Pause" : "Resume"}
      ><PlayPauseGlyph {playing} size={14} /></button>
    </div>
    <div class="meta">
      <p class="line">
        {#if go}
          {@const open = go}
          <span class="title"><NameLink label={current.title} onOpen={() => void openAlbum(current, open)} /></span>
          <span class="by muted"><QueueArtistLinks item={current} go={open} /></span>
        {:else}
          <span class="title">{current.title}</span>
          <span class="by muted">{current.artist}</span>
        {/if}
      </p>
      <div class="progress">
        <div
          class="track"
          role="progressbar"
          aria-label="Track progress"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progress * 100)}
        ><span style:width="{progress * 100}%"></span></div>
        <span class="time muted tnum">
          {formatDuration(player.state.position_ms / 1000)} / {formatDuration(
            (player.state.duration_ms || current.duration_secs * 1000) / 1000,
          )}
        </span>
      </div>
    </div>
  </section>
  {#if upNext}
    <button
      type="button"
      class="next"
      onclick={playNext}
      aria-label={`Play next: ${upNext.title}`}
      use:menuFor={() => menuItems(upNext, index + 1)}
    >
      <span class="next-thumb">
        <Cover uuid={upNext.cover} size={160} radius={6} alt="" />
        <span class="play-glyph" aria-hidden="true"><Icon name="play" size={12} /></span>
      </span>
      <span class="next-text">
        <span class="eyebrow">Up next</span>
        <span class="next-title">{upNext.title}</span>
        <span class="muted">{upNext.artist}</span>
      </span>
    </button>
  {/if}
</div>
{/if}

<style>
  /* No card, like the shortcut rows beside them. The 8px vertical padding
     (a row card's own) keeps the 88px art level with theirs. */
  /* One grid item in its own column (Home's `.continuing` grid, up to
     640px): the resume tile, then up next right after it. */
  .continue {
    position: relative;
    display: flex;
    align-items: center;
    gap: 20px;
    min-width: 0;
    container: continue / inline-size;
  }
  .resume,
  .next {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 8px 0;
  }
  .resume {
    flex: 0 1 auto;
    gap: 16px;
  }
  .art,
  .next-thumb {
    position: relative;
    flex: none;
    width: 88px;
    height: 88px;
    overflow: hidden;
    border-radius: var(--radius-control);
  }
  /* The current track's cover may be lit (AR-3); halo kept under 35%. */
  .art {
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent),
      0 4px 14px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .art :global(.cover),
  .next-thumb :global(.cover) {
    width: 100%;
    height: 100%;
  }

  /* Title over artist over progress, filling the cover's height. */
  .meta {
    display: flex;
    flex: 0 1 auto;
    max-width: 300px;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
  }
  .line {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    min-width: 0;
    line-height: 1.25;
    white-space: nowrap;
  }
  .title,
  .by {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .by {
    font-size: 13.5px;
  }
  .by :global(.name-link:hover) {
    color: var(--text);
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  /* A set width, not `flex: 1`: the tile sizes to its content, and an empty
     bar has no content width, so a flexible one collapsed to nothing. */
  .track {
    flex: 0 1 210px;
    width: 210px;
    height: 3px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }
  .track span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .time {
    flex: none;
    white-space: nowrap;
    font-size: 11.5px;
  }

  /* The cover's play glyph: an accent disc centred on the cover that grows
     in on hover or keyboard focus. */
  .play-glyph {
    position: absolute;
    inset: 0;
    margin: auto;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 40%, transparent);
    opacity: 0;
    transform: scale(0.8);
    transition:
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-state) var(--ease-enter);
  }
  .next-thumb .play-glyph {
    width: 24px;
    height: 24px;
  }
  .resume:hover .play-glyph,
  .resume:focus-within .play-glyph,
  .next:hover .play-glyph,
  .next:focus-visible .play-glyph {
    opacity: 1;
    transform: none;
  }
  button.play-glyph {
    cursor: pointer;
  }
  button.play-glyph:hover {
    filter: brightness(1.08);
    transform: scale(1.06);
  }
  button.play-glyph:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }

  .next {
    padding: 8px;
    border-radius: var(--radius-control);
    color: inherit;
    text-align: left;
    transition: background-color var(--dur-micro) var(--ease-enter);
  }
  .next:hover {
    background: var(--surface-2);
  }
  .next-thumb {
    width: 48px;
    height: 48px;
    border-radius: 6px;
  }
  .next-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    max-width: 150px;
    font-size: 12.5px;
    line-height: 1.3;
  }
  .next-text > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .eyebrow {
    margin-bottom: 2px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .next-title {
    font-size: 13.5px;
    font-weight: 600;
  }

  /* CSS, not a measured count, so the queue glide sees the final layout:
     up next drops out when the column is too narrow for both. */
  @container continue (width < 600px) {
    .next {
      display: none;
    }
  }

  /* A hairline in the middle of the grid gap (16px) setting the item off
     from the shortcuts, fading at both ends. Gone when the item has the row
     to itself (Home's `.continuing` grid below 896px). */
  .continue::after {
    content: "";
    position: absolute;
    top: 12px;
    bottom: 12px;
    right: -8px;
    width: 1px;
    background: linear-gradient(transparent, var(--line) 20%, var(--line) 80%, transparent);
    pointer-events: none;
  }
  @container shortcuts (width < 896px) {
    .continue::after {
      display: none;
    }
  }
</style>
