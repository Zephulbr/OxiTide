<script lang="ts">
  import type { Snippet } from "svelte";
  import { api, type Artist, type ArtistProfile } from "../api";
  import { appearance } from "../appearance.svelte";
  import { isHttpUrl, linkBrand, linkLabel } from "../artistLinks";
  import { artistArt, coverUrl } from "../artwork";
  import { formatCount } from "../format";
  import { heroScrim } from "../heroScrim";
  import BioText from "./BioText.svelte";
  import BrandIcon from "./BrandIcon.svelte";
  import Cover from "./Cover.svelte";
  import DetailAction from "./DetailAction.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let {
    artist,
    profile,
    disabled = false,
    favorite,
    onPlay,
    onShuffle,
    onOpenLink,
    onOpenArtist,
    actions,
  }: {
    artist: Artist;
    profile: ArtistProfile | null;
    disabled?: boolean;
    favorite?: { active: boolean; onToggle: () => void };
    onPlay: () => void;
    onShuffle: () => void;
    onOpenLink: (url: string) => void;
    onOpenArtist?: (id: number) => void;
    actions?: Snippet;
  } = $props();

  let tint = $state<{ primary: string; secondary: string; tertiary: string } | null>(null);
  let bioModal = $state(false);
  let linksModal = $state(false);

  function linkHost(url: string): string {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  }
  let canExpand = $state(false);
  let bioEl = $state<HTMLParagraphElement | null>(null);

  const fans = $derived(profile?.numberOfFollowers ?? null);
  const bioSegments = $derived(profile?.bioSegments ?? []);
  const links = $derived((profile?.externalLinks ?? []).filter((link) => isHttpUrl(link.url)));
  const art = $derived(artistArt(artist));
  const backdrop = $derived(art.uuid ? coverUrl(art.uuid, art.size) : "");
  const scrim = $derived(
    heroScrim(tint ? [tint.primary, tint.secondary, tint.tertiary] : null, appearance.resolvedTheme),
  );
  const heroStyle = $derived(
    (tint
      ? `--hero-primary:${tint.primary};--hero-secondary:${tint.secondary};--hero-tertiary:${tint.tertiary};`
      : "") + `--scrim:${scrim.toFixed(3)}`,
  );

  // Local palette only: the hero must not repaint the app-wide --dyn-* mood.
  $effect(() => {
    const uuid = art.uuid;
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
      <Cover uuid={art.uuid} size={art.size} radius={999} alt={artist.name} />
    </span>
    <div class="meta">
      <span class="kicker">Artist</span>
      <div class="name-row">
        <h2>{artist.name}</h2>
        {#if fans !== null}
          <p class="stats tnum">{formatCount(fans)} {fans === 1 ? "fan" : "fans"}</p>
        {/if}
      </div>
      {#if bioSegments.some((segment) => segment.text.trim() !== "")}
        <p class="bio" bind:this={bioEl}><BioText segments={bioSegments} onArtist={onOpenArtist} /></p>
        <!-- Always laid out; the measurement only toggles visibility. The
             overflow is measured a frame after a width change, so letting it
             add or remove the button would change the hero's height late and
             jump everything below (AR-9: width-driven layout must be CSS). -->
        <button
          class="more"
          class:unneeded={!canExpand}
          aria-haspopup="dialog"
          onclick={() => (bioModal = true)}
        >
          Read more
        </button>
      {/if}
      <!-- One bottom row: transport and Favorite, with the secondary actions
           (Links last) pushed to the far edge. It wraps in CSS only (AR-9). -->
      <div class="controls">
        <div class="transport">
          <button class="btn-play" onclick={onPlay} {disabled}>
            <Icon name="play" size={22} /> Play
          </button>
          <button class="btn-shuffle" onclick={onShuffle} {disabled}>
            <Icon name="shuffle" size={22} /> Shuffle
          </button>
          {#if favorite}
            <button
              class="btn-favorite"
              class:on={favorite.active}
              aria-pressed={favorite.active}
              aria-label={favorite.active ? "Unfavorite artist" : "Favorite artist"}
              title={favorite.active ? "Unfavorite artist" : "Favorite artist"}
              onclick={favorite.onToggle}
            >
              <Icon name="heart" size={22} />
            </button>
          {/if}
        </div>
        {#if actions || links.length > 0}
          <div class="actions">
            {@render actions?.()}
            {#if links.length > 0}
              <DetailAction
                icon="link"
                label="Links"
                title="Artist links"
                onclick={() => (linksModal = true)}
              />
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>
  {#if linksModal && links.length > 0}
    <Modal title={`${artist.name} elsewhere`} onclose={() => (linksModal = false)}>
      <ul class="links">
        {#each links as link (link.url)}
          {@const brand = linkBrand(link.site, link.url)}
          <li>
            <button
              class="link"
              title={link.url}
              onclick={() => {
                linksModal = false;
                onOpenLink(link.url);
              }}
            >
              <span class="logo">
                {#if brand}
                  <BrandIcon {brand} size={18} />
                {:else}
                  <Icon name="globe" size={18} />
                {/if}
              </span>
              <span class="link-text">
                <span class="link-label">{linkLabel(link.site, link.url)}</span>
                <span class="link-host">{linkHost(link.url)}</span>
              </span>
              <Icon name="external-link" size={14} />
            </button>
          </li>
        {/each}
      </ul>
    </Modal>
  {/if}
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
     hero's own text. The scrim pulls toward the room colour — darker in the
     dark theme, paper-lighter in the light one — light at the top, where the
     big name only needs 3:1, and deepest at the bottom behind the small bio
     and links. `--scrim` (lib/heroScrim.ts) scales it by how pale the palette
     is; with dynamic colour off the tint is not painted, so only the floor
     (for the blurred photo) remains. */
  .scrim {
    --s: max(0.3, calc(var(--scrim, 0.6) * var(--flag-dynamic-color)));
    position: absolute;
    inset: 0;
    background: linear-gradient(
      180deg,
      color-mix(in oklab, var(--bg) calc(var(--s) * 20%), transparent),
      color-mix(in oklab, var(--bg) calc(var(--s) * 50%), transparent) 45%,
      color-mix(in oklab, var(--bg) calc(var(--s) * 66%), transparent)
    );
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
    /* Small type on the lightest band of the scrim: stronger than muted. */
    color: color-mix(in oklab, var(--text) 82%, transparent);
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
    color: color-mix(in oklab, var(--text) 82%, transparent);
  }
  .controls {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px 16px;
    margin-top: 8px;
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
    gap: 10px;
    height: 52px;
    padding: 0 30px;
    border-radius: var(--radius-control);
    font-size: 16px;
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
  .transport > .btn-favorite {
    width: 52px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface-float) 70%, transparent);
    color: var(--text-muted);
  }
  .btn-favorite:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .btn-favorite.on {
    color: var(--accent);
  }
  .btn-favorite.on :global(svg) {
    fill: currentColor;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
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
  /* Keeps its space; hidden from pointer, Tab and screen readers. */
  .more.unneeded {
    visibility: hidden;
  }
  .links {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: min(360px, 80vw);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .link {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
  }
  .link:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .logo {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--text);
  }
  .link-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .link-label {
    color: var(--text);
    font-weight: 600;
  }
  .link-host {
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
