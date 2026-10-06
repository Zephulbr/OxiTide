<script lang="ts">
  import type { PageItem } from "../api";
  import { isPlayableTrack, shapeForItemType } from "../media";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";

  let {
    item,
    variant,
    thumb = 48,
    onclick,
    collage = false,
    playing = false,
    blocked = false,
  }: {
    item: PageItem;
    variant: "tile" | "carousel" | "row";
    thumb?: number;
    onclick?: () => void;
    collage?: boolean;
    /// Review Focus 3: highlight matches by numeric id and never highlights a
    /// non-track card.
    playing?: boolean;
    /// Review Focus 2: gray-out is presentation only; the store fails open and
    /// the Rust backstop decides playback.
    blocked?: boolean;
  } = $props();

  const shape = $derived(shapeForItemType(item.item_type));
  const playable = $derived(isPlayableTrack(item));
</script>

{#snippet body()}
  <span class="art" style:--thumb="{thumb}px">
    <span class="art-clip">
      {#if collage}
        <PlaylistCover
          owned
          playlist={item.id}
          image={item.artwork}
          size={variant === "row" ? 160 : 320}
          radius={0}
          alt=""
        />
      {:else}
        <Cover uuid={item.artwork} size={variant === "row" ? 160 : 320} radius={0} alt="" />
      {/if}
    </span>
    {#if shape === "mix"}
      <span class="badge" aria-hidden="true"><Icon name="radio" size={12} /></span>
    {/if}
    {#if playable}
      <span class="play-glyph" aria-hidden="true"><Icon name="play" size={14} /></span>
    {/if}
  </span>
  <span class="text">
    <span class="title">{item.title}</span>
    {#if item.subtitle}<span class="sub muted">{item.subtitle}</span>{/if}
  </span>
{/snippet}

{#if onclick}
  <button
    type="button"
    class="media {variant} shape-{shape}"
    class:playing
    class:blocked
    onclick={onclick}
    aria-label={playable ? `Play ${item.title}` : undefined}
  >{@render body()}</button>
{:else}
  <div class="media {variant} shape-{shape} inert" class:playing class:blocked>{@render body()}</div>
{/if}

<style>
  .media {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    min-width: 0;
    text-align: left;
    color: inherit;
    border-radius: var(--radius-card);
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
  .row {
    flex-direction: row;
    align-items: center;
    gap: 12px;
    padding: 8px;
    border-radius: var(--radius-control);
  }
  .media:not(.inert):hover {
    background: var(--surface-2);
  }
  .media.inert {
    opacity: 0.9;
  }
  .media.playing {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
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
  }
  .row .art {
    flex: 0 0 auto;
    width: var(--thumb);
  }
  .art-clip {
    display: block;
    overflow: hidden;
    aspect-ratio: 1;
    border-radius: var(--radius-card);
    background: var(--surface-2);
  }
  .shape-artist .art-clip {
    border-radius: 50%;
  }
  .shape-playlist .art-clip {
    border-radius: var(--radius-card-sharp);
  }
  .shape-playlist .art::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    transform: translate(3px, 3px);
    border: 1px solid var(--line);
    border-radius: var(--radius-card-sharp);
    background: var(--surface-3);
  }
  .row.shape-playlist .art::after {
    transform: translate(2px, 2px);
  }
  .art-clip :global(img) {
    transition: transform var(--dur-state) var(--ease-in);
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
  .play-glyph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius-card);
    color: var(--text);
    background: color-mix(in srgb, var(--surface-float) 65%, transparent);
    opacity: 0;
    transition: opacity var(--dur-state) var(--ease-in);
  }
  .shape-artist .play-glyph {
    border-radius: 50%;
  }
  .shape-playlist .play-glyph {
    border-radius: var(--radius-card-sharp);
  }
  .media:hover .play-glyph,
  .media:focus-visible .play-glyph {
    opacity: 1;
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
</style>
