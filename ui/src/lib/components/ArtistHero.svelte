<script lang="ts">
  import type { Snippet } from "svelte";
  import { api, type Artist, type ArtistProfile } from "../api";
  import { appearance } from "../appearance.svelte";
  import { isHttpUrl, linkLabel } from "../artistLinks";
  import { ARTIST_PICTURE_SIZE, coverUrl } from "../artwork";
  import { formatCount } from "../format";
  import BioText from "./BioText.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let {
    artist,
    profile,
    disabled = false,
    onPlay,
    onShuffle,
    onOpenLink,
    onOpenArtist,
    actions,
  }: {
    artist: Artist;
    profile: ArtistProfile | null;
    disabled?: boolean;
    onPlay: () => void;
    onShuffle: () => void;
    onOpenLink: (url: string) => void;
    onOpenArtist?: (id: number) => void;
    actions?: Snippet;
  } = $props();

  let tint = $state<{ primary: string; secondary: string; tertiary: string } | null>(null);
  let bioModal = $state(false);
  let canExpand = $state(false);
  let bioEl = $state<HTMLParagraphElement | null>(null);

  const fans = $derived(profile?.numberOfFollowers ?? null);
  const bioSegments = $derived(profile?.bioSegments ?? []);
  const links = $derived((profile?.externalLinks ?? []).filter((link) => isHttpUrl(link.url)));
  const backdrop = $derived(
    artist.picture ? coverUrl(artist.picture, ARTIST_PICTURE_SIZE) : "",
  );
  const heroStyle = $derived(
    tint
      ? `--hero-primary:${tint.primary};--hero-secondary:${tint.secondary};--hero-tertiary:${tint.tertiary}`
      : undefined,
  );

  // Local palette only: the hero must not repaint the app-wide --dyn-* mood.
  $effect(() => {
    const uuid = artist.picture;
    const theme = appearance.resolvedTheme;
    if (!uuid) {
      tint = null;
      return;
    }
    let current = true;
    void api
      .analyzeCover(uuid, theme)
      .then((palette) => {
        if (current) {
          tint = {
            primary: palette.primary,
            secondary: palette.secondary,
            tertiary: palette.tertiary,
          };
        }
      })
      .catch(() => {
        if (current) tint = null;
      });
    return () => {
      current = false;
    };
  });

  // The clamp hides text without any visual affordance; measure the real
  // overflow instead of guessing from a character count.
  $effect(() => {
    const node = bioEl;
    if (!node) {
      canExpand = false;
      return;
    }
    const measure = () => {
      canExpand = node.scrollHeight > node.clientHeight + 1;
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  });
</script>

<header class="artist-hero pane" style={heroStyle}>
  <div class="tint" aria-hidden="true"></div>
  {#if backdrop}
    <div class="photo" style="background-image:url({backdrop})" aria-hidden="true"></div>
  {/if}
  <div class="scrim" aria-hidden="true"></div>
  <div class="content">
    <span class="portrait">
      <Cover uuid={artist.picture} size={ARTIST_PICTURE_SIZE} radius={999} alt={artist.name} />
    </span>
    <div class="meta">
      <span class="kicker">Artist</span>
      <div class="name-row">
        <h2>{artist.name}</h2>
        {#if fans !== null}
          <p class="stats tnum">{formatCount(fans)} {fans === 1 ? "fan" : "fans"}</p>
        {/if}
      </div>
      <div class="controls">
        <div class="transport">
          <button class="btn-play" onclick={onPlay} {disabled}>
            <Icon name="play" size={20} /> Play
          </button>
          <button class="btn-shuffle" onclick={onShuffle} {disabled}>
            <Icon name="shuffle" size={20} /> Shuffle
          </button>
        </div>
        {#if actions}
          <div class="actions">{@render actions()}</div>
        {/if}
      </div>
      {#if bioSegments.some((segment) => segment.text.trim() !== "")}
        <p class="bio" bind:this={bioEl}><BioText segments={bioSegments} onArtist={onOpenArtist} /></p>
        {#if canExpand}
          <button class="more" aria-haspopup="dialog" onclick={() => (bioModal = true)}>
            Read more
          </button>
        {/if}
      {/if}
      {#if links.length > 0}
        <div class="socials">
          {#each links as link (link.url)}
            <button
              class="social"
              title={link.url}
              aria-label={`Open ${linkLabel(link.site, link.url)}`}
              onclick={() => onOpenLink(link.url)}
            >
              <Icon name="external-link" size={14} />
              <span>{linkLabel(link.site, link.url)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
  {#if bioModal && bioSegments.length > 0}
    <Modal title={`About ${artist.name}`} onclose={() => (bioModal = false)}>
      <div class="bio-full">
        <BioText
          segments={bioSegments}
          onArtist={(artistId) => {
            bioModal = false;
            onOpenArtist?.(artistId);
          }}
        />
      </div>
    </Modal>
  {/if}
</header>

<style>
  .artist-hero {
    --hero-primary: var(--surface-2);
    --hero-secondary: var(--surface-3);
    --hero-tertiary: var(--surface-1);
    position: relative;
    overflow: hidden;
    margin-bottom: 32px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sheet);
    background: linear-gradient(135deg, var(--surface-2), var(--surface-3) 55%, var(--surface-1));
  }
  /* Palette tint follows the dynamic-color flag: instrument resolves it to 0. */
  .tint {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      135deg,
      var(--hero-primary),
      var(--hero-secondary) 55%,
      var(--hero-tertiary)
    );
    opacity: var(--flag-dynamic-color);
  }
  .photo {
    position: absolute;
    inset: -12%;
    background-size: cover;
    background-position: center 30%;
    filter: blur(60px) saturate(1.2);
    transform: scale(1.1);
    opacity: calc(0.45 * var(--flag-backdrop-art));
  }
  /* The palette roles are tuned as *text* colours against the room (light in
     the dark theme), so painted as a background they can be as light as the
     hero's own text. The scrim pulls everything back toward the room colour —
     darker in the dark theme, paper-lighter in the light one — strongest behind
     the portrait and copy, lighter toward the right so the colour still shows. */
  .scrim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        90deg,
        color-mix(in oklab, var(--bg) 62%, transparent),
        color-mix(in oklab, var(--bg) 50%, transparent) 55%,
        color-mix(in oklab, var(--bg) 42%, transparent)
      ),
      linear-gradient(180deg, transparent 40%, color-mix(in oklab, var(--bg) 25%, transparent));
  }
  .content {
    position: relative;
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 36px 32px;
  }
  .portrait {
    flex: 0 0 auto;
    width: 260px;
  }
  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .kicker {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .name-row {
    display: flex;
    align-items: baseline;
    gap: 16px;
    flex-wrap: wrap;
  }
  h2 {
    margin: 0;
    font-size: clamp(38px, 4.4vw, 58px);
    line-height: 1.05;
  }
  .stats {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 16px;
    margin-top: 12px;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .transport > button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    height: 44px;
    padding: 0 22px;
    border-radius: var(--radius-control);
    font-weight: 600;
    transition:
      background var(--dur-micro) var(--ease-enter),
      color var(--dur-micro) var(--ease-enter);
  }
  .btn-play {
    background: var(--accent);
    color: var(--on-accent);
  }
  .btn-play:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn-play :global(svg) {
    fill: currentColor;
  }
  .btn-shuffle {
    background: color-mix(in srgb, var(--surface-float) 70%, transparent);
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .btn-shuffle:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }
  .transport > button:disabled {
    opacity: 0.5;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .bio {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    margin: 4px 0 0;
    max-width: 72ch;
    color: var(--text-muted);
    white-space: pre-line;
  }
  /* Modal reading layout: segments carry raw newlines, so pre-line keeps
     paragraph breaks while the roomier type does the rest. */
  .bio-full {
    max-width: 62ch;
    font-size: 16px;
    line-height: 1.7;
    color: var(--text);
    white-space: pre-line;
  }
  .more {
    align-self: flex-start;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .more:hover {
    color: var(--text);
  }
  .socials {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .social {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: var(--card-badge);
    background: color-mix(in srgb, var(--surface-float) 60%, transparent);
    backdrop-filter: blur(var(--glass-blur));
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 600;
  }
  .social:hover {
    color: var(--text);
    background: var(--surface-2);
  }
</style>
