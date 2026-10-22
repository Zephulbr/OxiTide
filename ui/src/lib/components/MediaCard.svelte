<script lang="ts">
  import type { PageItem } from "../api";
  import { appearance } from "../appearance.svelte";
  import { cardTarget, getCardActions } from "../cardActions";
  import { dragSource } from "../dragDrop.svelte";
  import type { DragPayload } from "../dragPayload";
  import { coverTint } from "../coverTint";
  import { isFullAlbum, isPlayableTrack, shapeForItemType } from "../media";
  import { menuFor } from "../menu.svelte";
  import { routeForPageItem } from "../nav.svelte";
  import { PREFETCH_DWELL_MS, prefetchRoute } from "../viewData";
  import { playlistCovers } from "../playlistArt.svelte";
  import ArtistLinks from "./ArtistLinks.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import NameLink from "./NameLink.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";

  let {
    item,
    variant,
    thumb = 48,
    onclick,
    collage = false,
    playing = false,
    blocked = false,
    playButton = false,
    gapless = false,
  }: {
    item: PageItem;
    variant: "tile" | "carousel" | "mini" | "row";
    thumb?: number;
    onclick?: () => void;
    collage?: boolean;
    /// Review Focus 3: highlight matches by numeric id and never highlights a
    /// non-track card.
    playing?: boolean;
    /// Review Focus 2: gray-out is presentation only; the store fails open and
    /// the Rust backstop decides playback.
    blocked?: boolean;
    /// Show the hover play button on a card whose click starts playback even
    /// though it is not a track (Home's radio cards). Tracks always show it.
    playButton?: boolean;
    /// Collage covers with no seams between the four tiles (Home's rows).
    gapless?: boolean;
  } = $props();

  const shape = $derived(shapeForItemType(item.item_type));
  const fullAlbum = $derived(isFullAlbum(item));
  const playable = $derived(isPlayableTrack(item));
  const showPlay = $derived(playable || playButton);

  /// Collection cards (albums, playlists, mixes) split into separate targets:
  /// the cover and title open the page, the artists open theirs, the play
  /// button plays it, and a context menu offers the rest. Without a provider
  /// (or for action cards like Home's radios) the card stays one button.
  const actions = getCardActions();
  const target = $derived(actions && onclick && !playButton ? cardTarget(item) : null);
  const artists = $derived(item.artist_refs ?? []);

  /// The card's own cover colour. Until it arrives (or when the cover cannot be
  /// analysed) the hover tint is the accent. It is prefetched when the card
  /// scrolls into view, so it is normally ready before the first hover; a hover
  /// or focus that beats the prefetch asks again, jumping the queue.
  let tint = $state<string | null>(null);
  let el = $state<HTMLElement | null>(null);
  let visible = $state(false);

  /// What the tint is read from. Collage playlists carry no artwork of their
  /// own: they use the collage's first cover, once the collage store has it.
  const tintSource = $derived(
    collage ? (playlistCovers(item.id)?.[0] ?? item.artwork ?? null) : (item.artwork ?? null),
  );

  function warm(urgent: boolean) {
    if (tint !== null || !tintSource) return;
    void coverTint(tintSource, appearance.resolvedTheme, { urgent }).then((color) => {
      if (color) tint = color;
    });
  }

  $effect(() => {
    const node = el;
    if (!node || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        visible = true;
        observer.disconnect();
      },
      { rootMargin: "120px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  });

  // Prefetch once visible; re-runs when a collage's covers arrive late.
  $effect(() => {
    if (visible) warm(false);
  });

  /// Page prefetch (lib/viewData.ts): a pointer that rests on a card that opens
  /// a page warms that page's data, so the click usually lands on a cache hit.
  /// The dwell keeps a sweep across a carousel from firing a request per card;
  /// action cards (radios) and tracks open no page and never prefetch.
  let dwell: ReturnType<typeof setTimeout> | undefined;
  function intentStart() {
    warm(true);
    if (!onclick || playButton || playable) return;
    clearTimeout(dwell);
    dwell = setTimeout(() => prefetchRoute(routeForPageItem(item)), PREFETCH_DWELL_MS);
  }
  function intentEnd() {
    clearTimeout(dwell);
  }

  /// What dragging the card carries (drag-to-playlist spec §6.2): its
  /// collection, or a track card's track; artists, page links and radios
  /// carry nothing. Tracks are fetched only on drop.
  function dragPayload(): DragPayload | null {
    const collection = cardTarget(item);
    if (collection) return { kind: "collection", target: collection, cover: tintSource };
    if (playable) return { kind: "track-id", id: Number(item.id), title: item.title, cover: item.artwork };
    return null;
  }
  $effect(() => () => clearTimeout(dwell));
</script>

{#snippet artwork()}
  <span class="art-clip">
    {#if collage}
      <PlaylistCover
        owned
        playlist={item.id}
        image={item.artwork}
        size={variant === "row" ? 160 : 320}
        radius={0}
        {gapless}
        alt=""
      />
    {:else}
      <Cover uuid={item.artwork} size={variant === "row" ? 160 : 320} radius={0} alt="" />
    {/if}
  </span>
  {#if shape === "mix"}
    <span class="badge" aria-hidden="true"><Icon name="radio" size={12} /></span>
  {/if}
{/snippet}

{#snippet body()}
  <span class="art" style:--thumb="{thumb}px">
    {@render artwork()}
    {#if showPlay}
      <span class="play-glyph" aria-hidden="true"><Icon name="play" size={variant === "row" ? 14 : 18} /></span>
    {/if}
  </span>
  <span class="text">
    <span class="title">{item.title}</span>
    {#if item.subtitle}<span class="sub muted">{item.subtitle}</span>{/if}
  </span>
{/snippet}

{#if target && actions && onclick}
  {@const open = onclick}
  {@const collection = target}
  <div
    class="media {variant} shape-{shape} split"
    role="group"
    aria-label={item.title}
    class:full-album={fullAlbum}
    class:playing
    class:blocked
    style:--card-tint={tint ?? undefined}
    bind:this={el}
    onpointerenter={intentStart}
    onpointerleave={intentEnd}
    onfocusin={intentStart}
    use:menuFor={() => actions.menu(item, collection)}
    use:dragSource={dragPayload}
  >
    <span class="art" style:--thumb="{thumb}px">
      <!-- The cover is the card's tab stop; the title repeats its target, so
           it stays out of the tab order. -->
      <button type="button" class="cover-hit" aria-label={`Open ${item.title}`} onclick={open}>
        {@render artwork()}
      </button>
      <button
        type="button"
        class="play-glyph"
        aria-label={`Play ${item.title}`}
        title="Play"
        onclick={() => void actions.play(collection)}
      >
        <Icon name="play" size={variant === "row" ? 14 : 18} />
      </button>
    </span>
    <span class="text">
      <span class="title"><NameLink label={item.title} onOpen={open} focusable={false} /></span>
      {#if artists.length > 0}
        <span class="sub muted"><ArtistLinks {artists} onOpen={actions.openArtist} /></span>
      {:else if item.subtitle}
        <span class="sub muted">{item.subtitle}</span>
      {/if}
    </span>
  </div>
{:else if onclick}
  <button
    type="button"
    class="media {variant} shape-{shape}"
    class:full-album={fullAlbum}
    class:playing
    class:blocked
    style:--card-tint={tint ?? undefined}
    bind:this={el}
    onclick={onclick}
    onpointerenter={intentStart}
    onpointerleave={intentEnd}
    onfocus={intentStart}
    aria-label={showPlay ? `Play ${item.title}` : undefined}
    use:dragSource={dragPayload}
  >{@render body()}</button>
{:else}
  <div class="media {variant} shape-{shape} inert" class:full-album={fullAlbum} class:playing class:blocked>{@render body()}</div>
{/if}

<style>
  .media {
    /* The cover's colour; the accent until the palette arrives. */
    --card-tint: var(--accent);
    transition: --card-tint var(--dur-view) var(--ease-in-out);
    --art-radius: var(--radius-card);
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    min-width: 0;
    text-align: left;
    color: inherit;
    border-radius: var(--radius-card);
  }
  .shape-artist {
    --art-radius: 50%;
  }
  .shape-playlist {
    --art-radius: var(--radius-card-sharp);
  }
  /* Full-length albums read as record sleeves: square corners. EPs, singles
     and albums of unknown type keep the card radius. */
  .full-album {
    --art-radius: 0;
  }
  .tile,
  .carousel {
    padding: 12px;
  }
  .carousel {
    width: 210px;
    flex: 0 0 auto;
    scroll-snap-align: start;
  }
  /* Compact shelf card (Home's recently played): smaller art, tighter caption. */
  .mini {
    width: 148px;
    flex: 0 0 auto;
    gap: 6px;
    padding: 8px;
    scroll-snap-align: start;
  }
  .mini .title,
  .mini .sub {
    font-size: 12.5px;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 12px;
    padding: 8px;
    border-radius: var(--radius-control);
  }

  /* Hover tint. The cover's colour spreads out from the cover (from the left on
     rows, from the top on stacked cards) and fades before the far side, so it
     blends with whatever accent wash the card already sits on. Transform and
     opacity only. */
  .media::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    background: linear-gradient(
      var(--tint-angle, 180deg),
      color-mix(in srgb, var(--card-tint) 34%, transparent),
      color-mix(in srgb, var(--card-tint) 15%, transparent) 42%,
      transparent 82%
    );
    opacity: 0;
    transform: scale(1, 0.55);
    transform-origin: top;
    transition:
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-view) var(--ease-enter);
    pointer-events: none;
  }
  .row::before {
    --tint-angle: 90deg;
    transform: scale(0.55, 1);
    transform-origin: left;
  }
  .media:not(.inert):hover::before,
  .media:not(.inert):focus-visible::before,
  .media.split:focus-within::before {
    opacity: 1;
    transform: none;
  }
  .media.playing {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .media.inert {
    opacity: 0.9;
  }
  .media.playing .title {
    color: var(--accent);
  }
  .media.blocked .title,
  .media.blocked .sub {
    opacity: 0.45;
  }
  .media.blocked .title {
    text-decoration: line-through;
    text-decoration-color: var(--line);
  }
  .media.blocked .art-clip :global(img) {
    opacity: 0.55;
  }

  .art {
    position: relative;
    isolation: isolate;
    display: block;
    /* The whole cover (and its deeper shadow, and the play button) lifts. */
    transition: transform var(--dur-state) var(--ease-enter);
  }
  .media:not(.inert):hover .art,
  .media:not(.inert):focus-visible .art,
  .media.split:focus-within .art {
    transform: translateY(-3px);
  }
  .row .art {
    flex: 0 0 auto;
    width: var(--thumb);
  }
  .art-clip {
    display: block;
    overflow: hidden;
    aspect-ratio: 1;
    border-radius: var(--art-radius);
    background: var(--surface-2);
    /* A hairline plus a soft drop shadow seat every cover on the page. */
    box-shadow: 0 0 0 1px var(--line), var(--shadow-cover);
  }
  /* The lifted cover's deeper shadow fades in as its own layer, so only
     opacity animates. */
  .art::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: var(--art-radius);
    box-shadow: 0 10px 22px rgb(0 0 0 / 0.38);
    opacity: 0;
    transition: opacity var(--dur-state) var(--ease-enter);
  }
  .media:not(.inert):hover .art::before,
  .media:not(.inert):focus-visible .art::before,
  .media.split:focus-within .art::before {
    opacity: 1;
  }
  .art-clip :global(img) {
    transition: transform var(--dur-state) var(--ease-enter);
  }
  .media:not(.inert):hover .art-clip :global(img) {
    transform: scale(1.03);
  }
  .badge {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: var(--card-badge);
    height: var(--card-badge);
    border: 1px solid transparent;
    border-radius: 50%;
    color: var(--text);
    background:
      linear-gradient(var(--surface-float), var(--surface-float)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
  }
  /* Accent play button: rises from the cover's bottom-right corner. The small
     steady glow is the primary-transport allowance (AR-1). */
  .play-glyph {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 40%, transparent);
    opacity: 0;
    transform: translateY(8px);
    transition:
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-state) var(--ease-enter);
  }
  .row .play-glyph {
    right: 4px;
    bottom: 4px;
    width: 30px;
    height: 30px;
  }

  .media:hover .play-glyph,
  .media:focus-visible .play-glyph,
  .media.split:focus-within .play-glyph {
    opacity: 1;
    transform: none;
  }
  /* Split cards: the cover is its own button, and the play glyph is a real
     button over it rather than decoration. */
  .cover-hit {
    display: block;
    width: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--art-radius);
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .cover-hit:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  button.play-glyph {
    cursor: pointer;
  }
  .split button.play-glyph:hover {
    filter: brightness(1.08);
    transform: scale(1.06);
  }
  .split button.play-glyph:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }
  .split .text :global(.name-link:hover) {
    color: var(--text);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Row cards carry larger art than the type was sized for; a step up keeps
     the text from reading as a caption beside it. */
  .row .title {
    font-size: 15px;
  }
  .row .sub {
    font-size: 13px;
  }
</style>
