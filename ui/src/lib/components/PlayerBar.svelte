<script lang="ts">
  import { dspBadgeTitle } from "../dsp";
  import { dragSource } from "../dragDrop.svelte";
  import type { DragPayload } from "../dragPayload";
  import { formatDuration, sentenceCase } from "../format";
  import { menu, menuButton, menuFor, type MenuItem } from "../menu.svelte";
  import { later } from "../later.svelte";
  import { nowPlayingMenuItems } from "../menus";
  import { copyText } from "../clipboard";
  import { errorMessage } from "../errors";
  import { playTrackRadio } from "../radio";
  import { trackInfo } from "../trackInfo.svelte";
  import type { Toasts } from "../toasts.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import { goToTargets } from "../queueLinks";
  import { playFromLabel, playSource } from "../playSource.svelte";
  import { QUALITY_TIERS, qualityPlate, streamReadout, type QualityDisplay } from "../quality";
  import { api, type QueueItem, type RepeatMode, type Track } from "../api";
  import type { Favorites } from "../favorites.svelte";
  import type { Player } from "../player.svelte";
  import { sameRoute, type Nav, type Route } from "../nav.svelte";
  import { albumRoute } from "../queueLinks";
  import { locatePlaying } from "../locatePlaying.svelte";
  import { marquee } from "../marquee";
  import NameLink from "./NameLink.svelte";
  import QueueArtistLinks from "./QueueArtistLinks.svelte";
  import type { Mini } from "../mini.svelte";
  import { appearance } from "../appearance.svelte";
  import CatJam from "./CatJam.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlayPauseGlyph from "./PlayPauseGlyph.svelte";
  import SeekBar from "./SeekBar.svelte";
  import VolumeSlider from "./VolumeSlider.svelte";
  import SleepTimerControl from "./SleepTimerControl.svelte";
  import type { SleepTimer } from "../sleepTimer.svelte";

  let {
    player,
    qualityDisplay,
    favorites,
    nav,
    mini,
    sleepTimer,
    toasts,
    playlistActions,
    floating = false,
  }: {
    player: Player;
    qualityDisplay: QualityDisplay;
    favorites: Favorites;
    nav: Nav;
    mini: Mini;
    sleepTimer: SleepTimer;
    /// Track Radio and errors in the track menu; the entry is left out without.
    toasts?: Toasts;
    /// "Add to playlist" in the track menu; left out without.
    playlistActions?: PlaylistActions;
    /// The floating card (App's `data-player`); its seek tapers the wave.
    floating?: boolean;
  } = $props();

  const position = $derived(player.pendingSeek ?? player.state.position_ms);
  const duration = $derived(player.state.duration_ms);
  const plate = $derived(
    qualityPlate(player.state.stream, qualityDisplay, player.state.preferred_quality),
  );

  /// Name links leave Now Playing so the page they open is visible.
  function go(route: Route) {
    nav.closeNowPlaying();
    nav.goto(route);
  }

  async function openAlbum(item: QueueItem) {
    const route = await albumRoute(item, api.getTrack);
    if (route) go(route);
  }

  /// The "From …" link: opens its page, or, when that page is already open,
  /// shows the playing track in it.
  function openOrLocate(route: Route) {
    if (!sameRoute(nav.route, route)) {
      go(route);
      return;
    }
    nav.closeNowPlaying();
    locatePlaying.request();
  }

  async function openAlbumOrLocate(item: QueueItem) {
    const route = await albumRoute(item, api.getTrack);
    if (route) openOrLocate(route);
  }

  /// The "From …" line: what started the queue, when it queued this track;
  /// otherwise (added later, single plays, older sessions) the track's album.
  const from = $derived.by(() => {
    const current = player.state.current;
    if (!current) return null;
    const source = playSource.fromFor(current.track_id);
    if (source?.name) {
      const route = source.route;
      return {
        label: playFromLabel(source.kind),
        name: source.name,
        open: route ? () => openOrLocate(route) : undefined,
      };
    }
    if (!current.album) return null;
    return { label: playFromLabel("album"), name: current.album, open: () => void openAlbumOrLocate(current) };
  });

  // Click motion for the transport toggles and skips (AR-2): each press
  // remounts the icon with its animation; 0 = never pressed (no motion on
  // mount).
  let shuffleKicks = $state(0);
  let repeatKicks = $state(0);
  let prevKicks = $state(0);
  let nextKicks = $state(0);
  /// Unfavouriting breaks the heart: two halves split along a crack and fall.
  let heartBreaks = $state(0);
  let heartBreaking = $state(false);

  function toggleFavorite(trackId: number, wasFavorite: boolean) {
    if (wasFavorite) {
      heartBreaks += 1;
      heartBreaking = true;
    }
    void favorites.toggleTrack(trackId);
  }

  /// The playing track as a full `Track` (playlist picker, info modal); a
  /// queue item carries only its display fields.
  async function withTrack(item: QueueItem, use: (track: Track) => void) {
    try {
      use(await api.getTrack(item.track_id));
    } catch (e) {
      toasts?.show(errorMessage(e), { severity: "error" });
    }
  }

  /// Right-click (or Shift+F10) on the title, artist or "From" line. Built at
  /// open time, so it reads the live track and favorite state.
  function nowMenuItems(): MenuItem[] {
    const item = player.state.current;
    if (!item) return [];
    const favorited = favorites.hasTrack(item.track_id);
    const radioToasts = toasts;
    const picker = playlistActions;
    return nowPlayingMenuItems({
      favorited,
      toggleFavorite: () => toggleFavorite(item.track_id, favorited),
      startRadio: radioToasts ? () => void playTrackRadio(item.track_id, radioToasts, item.title) : undefined,
      addToPlaylist: picker ? () => void withTrack(item, (track) => picker.openPicker([track])) : undefined,
      later: { active: later.has("track", item.track_id), toggle: () => void later.toggle("track", item.track_id) },
      copyTitle: () => void copyText(item.title),
      trackInfo: () => void withTrack(item, (track) => trackInfo.open(track)),
      ...goToTargets(item, go, api.getTrack),
    });
  }

  /// The cover and the title block drag the playing track (to a playlist or
  /// the queue); read at drag time, like the menu.
  function dragCurrent(): DragPayload | null {
    const item = player.state.current;
    return item ? { kind: "track-id", id: item.track_id, title: item.title, cover: item.cover } : null;
  }

  let plateButton = $state<HTMLButtonElement | null>(null);

  const readout = $derived(streamReadout(player.state.stream));
  const outputLabel = $derived(
    player.state.bit_perfect
      ? "Bit-perfect"
      : player.state.output_mode === "exclusive"
        ? "Exclusive output"
        : "Shared output",
  );

  /// The plate's menu: the streaming tiers (picking one restarts the track at
  /// its position), the open stream (`nowPlaying` below), and a way to the
  /// output settings.
  function qualityItems(): MenuItem[] {
    const preferred = player.state.preferred_quality;
    return [
      ...[...QUALITY_TIERS].reverse().map(
        (tier): MenuItem => ({
          id: `quality-${tier.value}`,
          label: tier.label,
          detail: tier.hint,
          section: "Streaming quality",
          selected: tier.value === preferred,
          icon: tier.value === preferred ? "check" : undefined,
          run: () => {
            if (tier.value !== preferred) void api.setPreferredQuality(tier.value).catch(() => {});
          },
        }),
      ),
      {
        id: "quality-now",
        label: readout
          ? [readout.codec, outputLabel, readout.spec].filter(Boolean).join(", ")
          : "Nothing streaming yet",
        note: true,
        divider: true,
        content: nowPlaying,
        run: () => {},
      },
      {
        id: "quality-settings",
        label: "Output settings…",
        icon: "settings",
        divider: true,
        run: () => go({ name: "settings" }),
      },
    ];
  }

  /// Opens upward, its right edge on the plate's, so it stands clear of the
  /// window edge; pressing the plate again closes it (`menuButton`).
  function openQuality(button: HTMLElement) {
    const rect = button.getBoundingClientRect();
    menu.show(rect.right, rect.top - 8, qualityItems(), button, { placement: "above", align: "end" });
  }

  /// Hover light (AR-2): one glow per bar that rises from the bottom edge
  /// under the hovered control and glides between controls. It appears in
  /// place (no glide in from the last spot) and lingers briefly over gaps so
  /// crossing between neighbours does not blink it off. Written to the DOM
  /// directly: it is pointer feedback, not state.
  /// The light's drawn box (CSS `.floor::after`); each control scales it.
  const FLOOR_W = 200;
  const FLOOR_H = 50;

  /// The swell under a control grows with it: its width with the control's
  /// width, its height with the control's height (or width, for wide short
  /// ones like the quality plate). It stays low and wide — a gentle slope,
  /// well below the controls: an icon gets ~260 × 26px, play/pause
  /// ~380 × 32px, the plate the widest.
  function floorSize(controlWidth: number, controlHeight: number) {
    const width = Math.min(440, Math.max(240, controlWidth * 6 + 80));
    const height = Math.min(34, Math.max(24, controlHeight * 0.3 + 17, width * 0.075));
    return { width, height };
  }

  let barEl = $state<HTMLDivElement | null>(null);
  let floor = $state<HTMLSpanElement | null>(null);
  let dimTimer: ReturnType<typeof setTimeout> | undefined;

  function lightUnder(target: EventTarget | null): boolean {
    const control = target instanceof Element ? target.closest<HTMLElement>("[data-floor]") : null;
    if (!control || !barEl || !floor || !barEl.contains(control)) return false;
    clearTimeout(dimTimer);
    const box = control.getBoundingClientRect();
    const x = box.left + box.width / 2 - barEl.getBoundingClientRect().left;
    const { width, height } = floorSize(box.width, box.height);
    const lit = floor.classList.contains("on");
    if (!lit) floor.classList.add("snap");
    floor.style.setProperty("--floor-x", `${x}px`);
    floor.style.setProperty("--floor-sx", String(width / FLOOR_W));
    floor.style.setProperty("--floor-sy", String(height / FLOOR_H));
    if (!lit) {
      void floor.offsetWidth;
      floor.classList.remove("snap");
    }
    floor.classList.add("on");
    return true;
  }

  function dim(delay = 0) {
    clearTimeout(dimTimer);
    dimTimer = setTimeout(() => floor?.classList.remove("on"), delay);
  }

  function onPointerOver(event: PointerEvent) {
    if (!lightUnder(event.target)) dim(160);
  }

  function onFocusIn(event: FocusEvent) {
    if (event.target instanceof Element && event.target.matches(":focus-visible")) lightUnder(event.target);
  }

  // Delegated on the bar (a plain container, so not as template handlers).
  $effect(() => {
    const bar = barEl;
    if (!bar) return;
    const leave = () => dim();
    const blur = () => dim(160);
    bar.addEventListener("pointerover", onPointerOver);
    bar.addEventListener("pointerleave", leave);
    bar.addEventListener("focusin", onFocusIn);
    bar.addEventListener("focusout", blur);
    return () => {
      bar.removeEventListener("pointerover", onPointerOver);
      bar.removeEventListener("pointerleave", leave);
      bar.removeEventListener("focusin", onFocusIn);
      bar.removeEventListener("focusout", blur);
      clearTimeout(dimTimer);
    };
  });

  function nextRepeat(mode: RepeatMode): RepeatMode {
    if (mode === "off") return "all";
    if (mode === "all") return "one";
    return "off";
  }
</script>

<!-- The quality menu's "Now playing" note: file type and output path on one
     line, the stream's format under them. -->
{#snippet nowPlaying()}
  {#if readout}
    <span class="np">
      <span class="np-head">
        <span class="np-codec">{readout.codec ?? plate.tier}</span>
        <span
          class="np-out"
          class:lit={player.state.bit_perfect}
          title={player.state.dsp_active ? dspBadgeTitle(player.state.dsp_stages) : undefined}
        >
          {#if player.state.bit_perfect}<Icon name="check" size={11} />{/if}{outputLabel}{#if player.state.dsp_active}<span class="np-dsp"> · DSP</span>{/if}
        </span>
      </span>
      {#if readout.spec}<span class="np-spec tnum">{readout.spec}</span>{/if}
    </span>
  {:else}
    <span class="np-idle">Nothing streaming yet</span>
  {/if}
{/snippet}

<div class="player" bind:this={barEl}>
  {#if player.state.current}
    {@const current = player.state.current}
    {@const fav = favorites.hasTrack(current.track_id)}
    <span class="floor" bind:this={floor} aria-hidden="true"></span>
    <SeekBar {player} taper={floating} />

    <div class="zone left">
      <button
        class="thumb"
        onclick={() => nav.toggleNowPlaying()}
        aria-label="Open now playing"
        title="Open now playing"
        use:dragSource={dragCurrent}
      >
        <Cover uuid={current.cover} size={160} radius={6} alt={`${current.title} artwork`} />
        <span class="thumb-hint" aria-hidden="true"><Icon name="chevron-up" size={18} /></span>
      </button>

      <!-- Keyed on the track so a track change fades the new title in rather
           than snapping it (the backdrop crossfades behind it). -->
      <div class="now-cat" style:--cat-px={`${(32 * appearance.catjamScale) / 100}px`}>
        {#key current.track_id}
          <!-- Lines too long for the column scroll slowly while hovered
               (lib/marquee.ts); each line's first child is what moves. -->
          <div class="now" use:marquee use:menuFor={nowMenuItems} use:dragSource={dragCurrent}>
            <strong class="title" data-marquee><span><NameLink label={current.title} onOpen={() => void openAlbum(current)} /></span></strong>
            <span class="muted artist" data-marquee><span><QueueArtistLinks item={current} {go} /></span></span>
            {#if from}
              <span class="from" data-marquee><span><span class="from-label">{from.label}</span> <NameLink label={from.name} onOpen={from.open} /></span></span>
            {/if}
          </div>
        {/key}

        {#if appearance.catjam && !appearance.reducedMotion}
          <CatJam {player} coat={appearance.catjamCoat} scale={appearance.catjamScale} />
        {/if}
      </div>

      <button
        class="ibtn icon-heart"
        data-floor
        class:on={fav}
        onclick={() => toggleFavorite(current.track_id, fav)}
        aria-label={fav ? "Unfavorite" : "Favorite"}
        title={fav ? "Unfavorite" : "Favorite"}
      >
        {#key fav}<span class="heart-ico" class:pop={fav} class:mend={!fav && heartBreaking}><Icon name="heart" size={18} /></span>{/key}
        {#if heartBreaking}
          {#key heartBreaks}
            <span class="heart-break" aria-hidden="true">
              <span class="half left"><Icon name="heart" size={18} /></span>
              <span class="half right" onanimationend={() => (heartBreaking = false)}><Icon name="heart" size={18} /></span>
            </span>
          {/key}
        {/if}
      </button>

      <span class="muted tnum time">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
      {#if player.state.buffering}<span class="muted buffering" role="status">waiting for network…</span>{/if}
    </div>

    <div class="zone center">
      <button
        class="ibtn"
        data-floor
        class:active={player.state.shuffle}
        onclick={() => { shuffleKicks += 1; player.setShuffle(!player.state.shuffle); }}
        aria-label="Shuffle"
        aria-pressed={player.state.shuffle}
        title="Shuffle the rest of the queue"
      >{#key shuffleKicks}<span class="ico" class:kick-shuffle={shuffleKicks > 0}><Icon name="shuffle" size={18} /></span>{/key}</button>
      <button class="ibtn" data-floor onclick={() => { prevKicks += 1; player.prev(); }} aria-label="Previous" title="Previous">{#key prevKicks}<span class="ico" class:kick-prev={prevKicks > 0}><Icon name="skip-back-solid" size={22} /></span>{/key}</button>
      <button
        class="ibtn play"
        data-floor
        onclick={() => player.playPause()}
        aria-label={player.state.playing ? "Pause" : "Play"}
        title={player.state.playing ? "Pause" : "Play"}
      >
        <PlayPauseGlyph playing={player.state.playing} size={32} />
      </button>
      <button class="ibtn" data-floor onclick={() => { nextKicks += 1; player.next(); }} aria-label="Next" title="Next">{#key nextKicks}<span class="ico" class:kick-next={nextKicks > 0}><Icon name="skip-forward-solid" size={22} /></span>{/key}</button>
      <button
        class="ibtn"
        data-floor
        class:active={player.state.repeat !== "off"}
        onclick={() => { repeatKicks += 1; player.setRepeat(nextRepeat(player.state.repeat)); }}
        aria-label={player.state.repeat === "one"
          ? "Repeat one track"
          : player.state.repeat === "all"
            ? "Repeat all"
            : "Repeat off"}
        aria-pressed={player.state.repeat !== "off"}
        title={player.state.repeat === "one" ? "Repeat one" : player.state.repeat === "all" ? "Repeat all" : "Repeat off"}
      >{#key repeatKicks}<span class="ico" class:kick-repeat={repeatKicks > 0}><Icon name={player.state.repeat === "one" ? "repeat-1" : "repeat"} size={18} /></span>{/key}</button>
    </div>

    <div class="zone right">
      <button
        class="ibtn"
        data-floor
        class:active={nav.queueOpen}
        onclick={() => nav.toggleQueue()}
        aria-label="Queue"
        title="Queue (Q)"
        aria-pressed={nav.queueOpen}
      ><Icon name="list-music" size={18} /></button>

      <SleepTimerControl {sleepTimer} />

      <VolumeSlider {player} />

      <!-- One plate for the stream: tier, format (detailed mode), and the
           output path (BP = bit-perfect, or DSP). It opens the quality menu. -->
      <button
        class="plate"
        data-floor
        bind:this={plateButton}
        use:menuButton={openQuality}
        aria-haspopup="menu"
        aria-expanded={menu.open !== null && menu.open.opener === plateButton}
        aria-label={`Streaming quality: ${plate.tier}${plate.spec ? `, ${plate.spec}` : ""}${player.state.bit_perfect ? ", bit-perfect" : player.state.dsp_active ? ", DSP active" : ""}`}
        title={player.state.stream ? "Streaming quality" : "Preferred quality"}
      >
        <span class="tier">{plate.tier}</span>
        {#if plate.spec}<span class="seg spec tnum">{plate.spec}</span>{/if}
        {#if player.state.bit_perfect}
          <span class="seg bp" title="Bit Perfect"><Icon name="check" size={11} />BP</span>
        {:else if player.state.dsp_active}
          <span class="seg dsp" title={dspBadgeTitle(player.state.dsp_stages)}>DSP</span>
        {/if}
        <span class="chev" aria-hidden="true"><Icon name="chevron-up" size={12} /></span>
      </button>

      <button
        class="ibtn"
        data-floor
        onclick={() => void mini.enter()}
        aria-label="Mini player"
        title="Mini player"
      ><Icon name="minimize-2" size={18} /></button>

      {#if player.state.bit_perfect_notice && !player.state.error}
        <!-- Output notices can be a sentence long: an icon with the text as
             its tooltip keeps the right zone from reflowing. -->
        <span
          class="notice"
          role="img"
          aria-label={sentenceCase(player.state.bit_perfect_notice)}
          title={sentenceCase(player.state.bit_perfect_notice)}
        ><Icon name="alert-triangle" size={16} /></span>
      {/if}
    </div>
  {:else}
    <span class="idle muted"><Icon name="music" size={16} /> Nothing playing</span>
  {/if}
</div>

<style>
  /* Three zones: track info left, transport dead-center, queue/volume/quality
     right. The 1fr/auto/1fr grid keeps the middle centered no matter how wide
     the sides get. The box (position/height) comes from app.css so the bar
     floats over the content. */
  .player {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    padding: 0 16px;
  }
  .zone {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .zone.left { justify-self: start; }
  .zone.center { justify-self: center; gap: 10px; }
  .zone.right { justify-self: end; }

  /* Hover light (AR-1 site, AR-2 hover): accent light rising from the bottom
     edge. A faint wash runs along the whole edge and swells, on a bell curve,
     into a soft peak under the hovered control (--floor-x); one per bar,
     gliding between controls. Behind the controls (z -1, after the glass).
     The `on` and `snap` classes are written by the script.
     The shape comes from two smooth profiles multiplied: across, a Gaussian
     (the swell's gradient), up, an exponential fade (--floor-fade, shared
     with the edge wash). Their product's outline is a bell — flat along the
     edge, long gentle slopes, a rounded peak — with no hard edge anywhere,
     unlike a radial dome whose rim rises steeply from the edge. */
  .floor {
    --floor-fade: linear-gradient(
      to top,
      #000,
      rgb(0 0 0 / 0.62) 25%,
      rgb(0 0 0 / 0.32) 50%,
      rgb(0 0 0 / 0.12) 75%,
      transparent
    );
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    /* The floating bar's rounded box (app.css). */
    border-radius: inherit;
    pointer-events: none;
    opacity: 0;
    transition: opacity 1000ms var(--ease-in-out);
  }
  .floor::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 16px;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    mask-image: var(--floor-fade);
  }
  /* The swell: a 200 × 50 box scaled per control (--floor-sx/-sy, from its
     size) around its bottom centre, so the glide and the resize are one
     transform. The stops sample exp(-t²/2) for t = -3…3 at a 28% peak. */
  .floor::after {
    content: "";
    position: absolute;
    left: 0;
    bottom: 0;
    width: 200px;
    height: 50px;
    transform-origin: 50% 100%;
    transform: translateX(calc(var(--floor-x, 0px) - 100px))
      scale(var(--floor-sx, 1), var(--floor-sy, 1));
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--accent) 0.31%, transparent) 0%,
      color-mix(in srgb, var(--accent) 1.23%, transparent) 8.33%,
      color-mix(in srgb, var(--accent) 3.78%, transparent) 16.67%,
      color-mix(in srgb, var(--accent) 9.1%, transparent) 25%,
      color-mix(in srgb, var(--accent) 17%, transparent) 33.33%,
      color-mix(in srgb, var(--accent) 24.7%, transparent) 41.67%,
      color-mix(in srgb, var(--accent) 28%, transparent) 50%,
      color-mix(in srgb, var(--accent) 24.7%, transparent) 58.33%,
      color-mix(in srgb, var(--accent) 17%, transparent) 66.67%,
      color-mix(in srgb, var(--accent) 9.1%, transparent) 75%,
      color-mix(in srgb, var(--accent) 3.78%, transparent) 83.33%,
      color-mix(in srgb, var(--accent) 1.23%, transparent) 91.67%,
      color-mix(in srgb, var(--accent) 0.31%, transparent) 100%
    );
    mask-image: var(--floor-fade);
    transition: transform var(--dur-view) var(--ease-enter);
  }
  /* It comes up slowly (700 ms) and goes down more slowly still (1 s), so it
     reads as light warming and cooling, not a switch. */
  .floor:global(.on) {
    opacity: 1;
    transition: opacity 700ms var(--ease-in-out);
  }
  @media (prefers-reduced-motion: reduce) {
    .floor,
    .floor:global(.on) {
      transition-duration: var(--dur-state);
    }
  }
  .floor:global(.snap)::after {
    transition: none;
  }

  .thumb {
    position: relative;
    flex: 0 0 auto;
    width: 64px;
    height: 64px;
    padding: 0;
    border-radius: 6px;
    overflow: hidden;
    transition: transform var(--dur-micro) var(--ease-enter);
  }
  .thumb:hover {
    transform: scale(1.04);
  }
  /* The floating bar is 76px (app.css); the cover keeps the same 8px margin
     to its edges. */
  :global(.app[data-player="floating"]) .thumb {
    width: 60px;
    height: 60px;
  }
  /* Expand hint: the cover opens Now Playing, so say so on hover. */
  .thumb-hint {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    color: var(--text);
    opacity: 0;
    transition: opacity var(--dur-micro);
  }
  .thumb:hover .thumb-hint,
  .thumb:focus-visible .thumb-hint {
    opacity: 1;
  }

  .now {
    display: flex;
    flex-direction: column;
    min-width: 120px;
    max-width: 260px;
    animation: now-in var(--dur-view) var(--ease-enter) backwards;
  }
  /* Title + jamming cat. With the cat, the title's minimum width moves to
     this wrapper and the title hugs its text; the cat's auto margins split the
     leftover room, so it sits as far from the title as from the heart (the
     wrapper gap on its left matches the zone gap on its right). */
  .now-cat {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .now-cat:has(:global(.catjam)) {
    min-width: calc(120px + 12px + var(--cat-px));
  }
  .now-cat:has(:global(.catjam)) .now {
    min-width: 0;
  }
  .now-cat :global(.catjam) {
    margin-inline: auto;
  }
  @keyframes now-in {
    from {
      opacity: 0;
      transform: translateY(calc(var(--rise) * 0.4));
    }
  }
  .title {
    font-size: 15px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist :global(.name-link:hover) {
    color: var(--text);
  }
  /* "From album Blood Sugar Sex Magik": the eyebrow, then the name, which
     opens the album/playlist/… the queue came from. */
  .from {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .from-label {
    font-size: 9.5px;
    font-weight: 800;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    opacity: 0.8;
  }
  .from :global(.name-link:hover) {
    color: var(--text);
  }
  button {
    display: inline-grid;
    place-items: center;
  }

  /* Icon buttons: round hit area, resting a step below full text colour and
     coming up to it on hover (the floor light marks the target), deeper press
     than the global 0.98 for tactility. */
  .ibtn {
    position: relative;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: color-mix(in srgb, var(--text) 78%, transparent);
    transition:
      color var(--dur-micro),
      transform var(--dur-micro) var(--ease-enter);
  }
  .ibtn:hover {
    color: var(--text);
  }
  .player .ibtn:active {
    transform: scale(0.92);
  }
  /* The sleep timer and mute buttons come from shared components: in the bar
     they take the same light instead of their hover halo. */
  .player :global(.moon:hover),
  .player :global(.mute:hover) {
    background: transparent;
  }
  /* On/off affordance for the toggles: accent color + a dot that scales in. */
  .ibtn::after {
    content: "";
    position: absolute;
    bottom: 0;
    left: 50%;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--accent);
    transform: translateX(-50%) scale(0);
    transition: transform var(--dur-micro) var(--ease-enter);
  }
  .ibtn.active {
    color: var(--accent);
  }
  .ibtn.active::after {
    transform: translateX(-50%) scale(1);
  }

  .icon-heart.on {
    color: var(--accent);
  }
  .icon-heart.on :global(svg) {
    fill: currentColor;
  }
  /* Favoriting pops the heart; the keyed remount replays the animation. */
  .heart-ico {
    display: inline-grid;
    place-items: center;
  }
  .heart-ico.pop {
    animation: heart-pop var(--dur-state) var(--ease-enter);
  }
  @keyframes heart-pop {
    0% { transform: scale(1); }
    40% { transform: scale(1.35); }
    100% { transform: scale(1); }
  }
  /* Unfavoriting breaks the heart: the filled heart cracks (a short swell),
     splits along a jagged line, and the halves tip apart and fall away while
     the outline fades back in underneath. */
  .heart-ico.mend {
    animation: heart-mend 700ms var(--ease-in-out) both;
  }
  @keyframes heart-mend {
    0%, 55% { opacity: 0; }
    100% { opacity: 1; }
  }
  .heart-break {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    animation: heart-crack 700ms var(--ease-enter) both;
  }
  @keyframes heart-crack {
    0% { transform: scale(1); }
    15% { transform: scale(1.12); }
    30%, 100% { transform: scale(1); }
  }
  .half {
    grid-area: 1 / 1;
    display: inline-grid;
    place-items: center;
    color: var(--accent);
    transform-origin: 50% 85%;
  }
  .half :global(svg) {
    fill: currentColor;
  }
  .half.left {
    clip-path: polygon(0 0, 52% 0, 44% 28%, 56% 46%, 42% 64%, 50% 100%, 0 100%);
    animation: heart-half-left 700ms ease-in both;
  }
  .half.right {
    clip-path: polygon(52% 0, 100% 0, 100% 100%, 50% 100%, 42% 64%, 56% 46%, 44% 28%);
    animation: heart-half-right 700ms ease-in both;
  }
  @keyframes heart-half-left {
    0%, 25% { transform: none; opacity: 1; }
    100% { transform: translate(-5px, 9px) rotate(-28deg); opacity: 0; }
  }
  @keyframes heart-half-right {
    0%, 25% { transform: none; opacity: 1; }
    100% { transform: translate(5px, 9px) rotate(28deg); opacity: 0; }
  }

  /* Transport nudges, replayed by the keyed remount on each press: shuffle
     pushes its arrows ahead, repeat turns once round, skips slide out the way
     they go and come back in from the other side. */
  .ico {
    display: inline-grid;
    place-items: center;
  }
  /* Shuffle: the icon glides forward a touch with its two arrowheads (the
     first two paths) pushing further ahead, then everything settles back. */
  .ico.kick-shuffle {
    animation: kick-shuffle 520ms both;
  }
  .ico.kick-shuffle :global(path:nth-child(-n + 2)) {
    animation: kick-shuffle-head 520ms both;
  }
  @keyframes kick-shuffle {
    0% { transform: none; animation-timing-function: cubic-bezier(0.2, 0.7, 0.3, 1); }
    35% { transform: translateX(1.5px); animation-timing-function: cubic-bezier(0.4, 0, 0.2, 1); }
    100% { transform: none; }
  }
  @keyframes kick-shuffle-head {
    0% { transform: none; animation-timing-function: cubic-bezier(0.2, 0.7, 0.3, 1); }
    35% { transform: translateX(3px); animation-timing-function: cubic-bezier(0.4, 0, 0.2, 1); }
    100% { transform: none; }
  }
  .ico.kick-repeat {
    animation: kick-repeat 520ms var(--ease-in-out);
  }
  @keyframes kick-repeat {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  .ico.kick-next {
    animation: kick-next 280ms var(--ease-in-out);
  }
  @keyframes kick-next {
    0% { transform: none; opacity: 1; }
    45% { transform: translateX(7px); opacity: 0; }
    55% { transform: translateX(-7px); opacity: 0; }
    100% { transform: none; opacity: 1; }
  }
  .ico.kick-prev {
    animation: kick-prev 280ms var(--ease-in-out);
  }
  @keyframes kick-prev {
    0% { transform: none; opacity: 1; }
    45% { transform: translateX(-7px); opacity: 0; }
    55% { transform: translateX(7px); opacity: 0; }
    100% { transform: none; opacity: 1; }
  }
  /* Reduced motion: no movement; the broken heart just fades (its
     animationend still clears the state) and the outline is there at once. */
  @media (prefers-reduced-motion: reduce) {
    .ico.kick-shuffle,
    .ico.kick-shuffle :global(path),
    .ico.kick-repeat,
    .ico.kick-next,
    .ico.kick-prev,
    .heart-break,
    .heart-ico.mend {
      animation: none;
    }
    .half.left,
    .half.right {
      animation: heart-fade var(--dur-state) ease-out both;
    }
  }
  @keyframes heart-fade {
    from { opacity: 1; }
    to { opacity: 0; }
  }

  .ibtn.play {
    width: 50px;
    height: 50px;
    color: var(--accent);
  }

  .time {
    font-size: 12px;
    white-space: nowrap;
  }
  .buffering {
    font-size: 12px;
    white-space: nowrap;
  }
  /* Quality plate: one pill, hairline-split into tier · format · output path,
     with a chevron that says it opens a menu. */
  .plate {
    display: inline-flex;
    align-items: stretch;
    height: 26px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 13px;
    background: var(--surface-1);
    color: var(--text-muted);
    font-size: 11px;
    white-space: nowrap;
    transition:
      background-color var(--dur-micro),
      border-color var(--dur-micro),
      color var(--dur-micro);
  }
  .plate > span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 9px;
  }
  .plate > .seg {
    border-left: 1px solid var(--line);
  }
  .plate:hover,
  .plate[aria-expanded="true"] {
    background: var(--surface-3);
    border-color: color-mix(in srgb, var(--text) 18%, transparent);
    color: var(--text);
  }
  .tier,
  .bp,
  .dsp {
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text);
  }
  .plate .tier {
    color: var(--accent);
  }
  .bp :global(svg) {
    color: var(--accent);
    stroke-width: 3;
  }
  .plate > .chev {
    padding: 0 8px 0 0;
    margin-left: -3px;
    opacity: 0.55;
    transition: opacity var(--dur-micro);
  }
  .plate:hover > .chev,
  .plate[aria-expanded="true"] > .chev {
    opacity: 1;
  }
  @media (max-width: 900px) {
    .plate > .spec { display: none; }
  }

  /* Quality menu "Now playing" (rendered inside the menu): an eyebrow row —
     the file type in the accent on the left, the output path on the right
     (accent with a check when bit-perfect) — over the format in text colour. */
  .np {
    display: flex;
    flex-direction: column;
    gap: 9px;
    width: 100%;
    padding-block: 2px;
  }
  .np-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .np-codec {
    color: var(--accent);
  }
  .np-out {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text-muted);
  }
  .np-out.lit {
    color: var(--accent);
  }
  .np-out :global(svg) {
    stroke-width: 3;
  }
  .np-spec {
    color: var(--text);
    font-size: 13px;
  }
  .np-idle {
    color: var(--text-muted);
  }
  .notice {
    display: inline-grid;
    place-items: center;
    color: var(--warning);
    cursor: help;
  }
  /* Empty bar: a quiet centered line across all three zones. */
  .idle {
    grid-column: 1 / -1;
    justify-self: center;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
</style>
