<script lang="ts">
  import { api, type QueueItem } from "../api";
  import { formatDuration } from "../format";
  import { greeting } from "../homeData";
  import type { Route } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import { albumRoute } from "../queueLinks";
  import NameLink from "./NameLink.svelte";
  import QueueArtistLinks from "./QueueArtistLinks.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlayPauseGlyph from "./PlayPauseGlyph.svelte";

  /// Home's masthead: a time-of-day greeting and, when there is a current
  /// (or restored) track, a "Continue listening" card tinted by its cover with
  /// the next few queued tracks in an inset panel on the right.
  let {
    player,
    onerror,
    go,
  }: {
    player: Player;
    onerror?: (e: unknown) => void;
    /// Makes the current track's title, artist and album links (name-links spec).
    go?: (route: Route) => void;
  } = $props();

  async function openAlbum(item: QueueItem, open: (route: Route) => void) {
    const route = await albumRoute(item, api.getTrack);
    if (route) open(route);
  }

  const UP_NEXT = 4;

  const hello = greeting(new Date().getHours());
  const current = $derived(player.state.current);
  const playing = $derived(player.state.playing);
  const progress = $derived(
    player.state.duration_ms > 0
      ? Math.min(1, player.state.position_ms / player.state.duration_ms)
      : 0,
  );
  const queue = $derived(player.state.queue);
  const index = $derived(player.state.current_index ?? -1);
  const upNext = $derived(index >= 0 ? queue.slice(index + 1, index + 1 + UP_NEXT) : []);
  const more = $derived(index >= 0 ? Math.max(0, queue.length - index - 1 - upNext.length) : 0);

  function toggle() {
    void Promise.resolve(player.playPause()).catch((e) => onerror?.(e));
  }

  function jump(offset: number) {
    void api.playTracks(queue, index + 1 + offset).catch((e) => onerror?.(e));
  }
</script>

<header class="masthead">
  <h2>{hello}</h2>
  {#if !current}<p class="muted">Pick something to start the night.</p>{/if}
</header>

{#if current}
  <section class="hero" class:has-next={upNext.length > 0} aria-label="Continue listening">
    <div class="art">
      <Cover uuid={current.cover} size={640} radius={14} alt="" />
    </div>
    <div class="meta">
      <span class="eyebrow">{playing ? "Now playing" : "Continue listening"}</span>
      {#if go}
        {@const open = go}
        <h3 class="title"><NameLink label={current.title} onOpen={() => void openAlbum(current, open)} /></h3>
        <p class="by">
          <span><QueueArtistLinks item={current} go={open} /></span>
          {#if current.album}<span class="muted"> · <NameLink label={current.album} onOpen={() => void openAlbum(current, open)} /></span>{/if}
        </p>
      {:else}
        <h3 class="title">{current.title}</h3>
        <p class="by">
          <span>{current.artist}</span>
          {#if current.album}<span class="muted"> · {current.album}</span>{/if}
        </p>
      {/if}
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
      <div class="actions">
        <button type="button" class="go" onclick={toggle}>
          <PlayPauseGlyph {playing} size={20} />
          <span>{playing ? "Pause" : "Resume"}</span>
        </button>
        <button
          type="button"
          class="ghost"
          onclick={() => void player.next()}
          aria-label="Next track"
          title="Next track"
        ><Icon name="skip-forward-solid" size={18} /></button>
      </div>
    </div>

    {#if upNext.length > 0}
      <aside class="next" aria-label="Up next">
        <header>
          <h4>Up next</h4>
          {#if more > 0}<span class="muted tnum">+{more} more</span>{/if}
        </header>
        <ol>
          {#each upNext as item, i (`${item.track_id}-${i}`)}
            <li>
              <button type="button" class="row" onclick={() => jump(i)} aria-label={`Play ${item.title}`}>
                <span class="thumb"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
                <span class="text">
                  <span class="name">{item.title}</span>
                  <span class="muted by">{item.artist}</span>
                </span>
                <span class="muted tnum dur">{formatDuration(item.duration_secs)}</span>
              </button>
            </li>
          {/each}
        </ol>
      </aside>
    {/if}
  </section>
{/if}

<style>
  .masthead {
    margin: 0 0 20px;
  }
  .masthead h2 {
    margin: 0;
    font-size: 40px;
  }
  .masthead p {
    margin: 4px 0 0;
  }
  .hero {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    gap: 32px;
    align-items: center;
    margin-bottom: 30px;
    padding: 26px;
    border: 1px solid transparent;
    border-radius: var(--radius-sheet);
    /* The cover tints the card: a stronger accent wash than the page surfaces,
       so the hero reads as its own object over the backdrop. */
    background:
      linear-gradient(
          115deg,
          color-mix(in srgb, var(--accent) 22%, var(--surface-1)),
          color-mix(in srgb, var(--accent) 7%, var(--surface-1)) 65%,
          var(--surface-1)
        )
        padding-box,
      linear-gradient(
          135deg,
          color-mix(in srgb, var(--accent) 38%, transparent),
          color-mix(in srgb, var(--accent) 16%, transparent)
        )
        border-box;
    /* Plain depth, not a glow: a dark drop shadow lifts it off the backdrop. */
    box-shadow: var(--shadow-panel), 0 18px 44px rgb(0 0 0 / 0.14);
  }
  .hero.has-next {
    grid-template-columns: 200px minmax(0, 1fr) minmax(300px, 36%);
  }
  .art {
    border-radius: 14px;
    /* The current track's cover may be lit (AR-3); halo kept under 35%. */
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent),
      0 10px 36px color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .art :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .meta {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
  }
  .eyebrow {
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .title {
    max-width: 100%;
    margin: 0;
    font-family: var(--font-display);
    font-size: 40px;
    font-weight: 640;
    letter-spacing: -0.02em;
    line-height: 1.1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .by {
    max-width: 100%;
    margin: 0;
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .by .muted :global(.name-link:hover) {
    color: var(--text);
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 12px;
    width: min(420px, 100%);
    margin-top: 10px;
  }
  .track {
    flex: 1;
    height: 4px;
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
    font-size: 12px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }
  .go {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 22px 0 16px;
    border-radius: 99px;
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
    /* Primary transport glow (AR-1): small, steady, accent-only. */
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 40%, transparent);
    transition: transform var(--dur-micro) var(--ease-enter);
  }
  .go:active {
    transform: scale(0.98);
  }
  .ghost {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 1px solid var(--line);
    border-radius: 50%;
    color: var(--text);
    background: var(--surface-2);
    transition: background var(--dur-state) var(--ease-enter);
  }
  .ghost:hover {
    background: var(--surface-3);
  }

  /* Up next sits directly on the card, set off by a hairline that runs the
     full card height (cancelling the hero's padding) and fades at both ends. */
  .next {
    position: relative;
    align-self: stretch;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
    margin: -26px -26px -26px 0;
    padding: 26px 26px 26px 32px;
  }
  .next::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 1px;
    background: linear-gradient(
      transparent,
      var(--line) 14%,
      var(--line) 86%,
      transparent
    );
  }
  .next header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0 8px 8px;
    font-size: 12.5px;
  }
  .next h4 {
    margin: 0;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  ol {
    display: flex;
    flex-direction: column;
    gap: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* A hairline under each track, starting at the text column. */
  li {
    position: relative;
  }
  li::after {
    content: "";
    position: absolute;
    right: 8px;
    bottom: 0;
    left: 60px;
    height: 1px;
    background: var(--line);
  }
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    width: 100%;
    padding: 6px 8px;
    border-radius: var(--radius-control);
    text-align: left;
    color: inherit;
    transition: background-color var(--dur-micro) var(--ease-enter);
  }
  .row:hover {
    background: var(--surface-2);
  }
  .thumb {
    display: block;
    width: 40px;
    height: 40px;
    overflow: hidden;
    border-radius: 6px;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name,
  .by {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-weight: 500;
  }
  .by,
  .dur {
    font-size: 12.5px;
  }
  @media (max-width: 1100px) {
    .hero.has-next {
      grid-template-columns: 200px minmax(0, 1fr);
    }
    .next {
      display: none;
    }
  }
  @media (max-width: 760px) {
    .hero,
    .hero.has-next {
      grid-template-columns: 112px minmax(0, 1fr);
      gap: 16px;
      padding: 16px;
    }
    .title {
      font-size: 26px;
    }
  }
</style>
