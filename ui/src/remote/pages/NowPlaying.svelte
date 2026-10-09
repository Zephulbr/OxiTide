<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import type { Track } from "../../lib/api";
  import Art from "../components/Art.svelte";
  import Lyrics from "../components/Lyrics.svelte";
  import { useRemote } from "../context";
  import { art } from "../convert";
  import { clock, nextRepeat, qualityBadges, REMOTE_FADE_MS } from "../nowPlaying";
  import { hrefFor } from "../routes";

  const { store, api, actions } = useRemote();

  const playback = $derived(store.state);
  const current = $derived(playback?.current ?? null);
  const badges = $derived(playback ? qualityBadges(playback) : null);

  let now = $state(Date.now());
  let seeking = $state<number | null>(null);
  let volume = $state<number | null>(null);
  let showLyrics = $state(false);
  let details = $state<Track | null>(null);

  // Interpolate the position between the PC's 1 Hz updates.
  $effect(() => {
    if (!playback?.playing) return;
    const tick = setInterval(() => (now = Date.now()), 500);
    return () => clearInterval(tick);
  });

  // Ids for the artist/album links (queue items carry names only).
  $effect(() => {
    const id = current?.trackId;
    details = null;
    if (id === undefined) return;
    void api
      .getTrack(id)
      .then((track) => {
        if (store.state?.current?.trackId === id) details = track;
      })
      .catch(() => {});
  });

  $effect(() => {
    void actions.ensureFavorites();
  });

  const position = $derived(seeking ?? store.positionAt(now));
  const duration = $derived(playback?.durationMs ?? 0);
  const shownVolume = $derived(volume ?? playback?.volume ?? 1);
  const favorite = $derived(current ? actions.favTracks.has(current.trackId) : false);

  let volumeSent = 0;
  function onVolume(value: number) {
    volume = value;
    const at = Date.now();
    if (at - volumeSent > 150) {
      volumeSent = at;
      void api.setVolume(value).catch((e) => actions.fail(e));
    }
  }

  function commitVolume(value: number) {
    void api
      .setVolume(value)
      .catch((e) => actions.fail(e))
      .finally(() => (volume = null));
  }

  function commitSeek(value: number) {
    store.noteSeek(value);
    seeking = null;
    void api.seek(value).catch((e) => actions.fail(e));
  }

  function run(task: Promise<unknown>) {
    void task.catch((e) => actions.fail(e));
  }
</script>

{#if !current}
  <div class="empty">
    <Icon name="disc-3" size={40} />
    <p>Nothing is playing on the PC.</p>
    <a class="pill primary" href="#/home">Browse</a>
    <a class="pill" href="#/search">Search</a>
  </div>
{:else}
  <section class="np" aria-label="Now playing">
    {#if showLyrics}
      <Lyrics track={current} positionMs={position} />
    {:else}
      <div class="cover">
        <Art src={art(current.cover, 640)} size="100%" eager />
      </div>
    {/if}

    <div class="meta">
      <div class="titles">
        <h1 class="ellipsis">{current.title}</h1>
        <p class="ellipsis">
          {#if details?.artists[0]}
            <a href={hrefFor({ name: "artist", id: details.artists[0].id })}>{current.artist}</a>
          {:else}
            <span>{current.artist}</span>
          {/if}
          {#if current.album}
            <span class="muted"> · </span>
            {#if details?.album}
              <a class="muted" href={hrefFor({ name: "album", id: details.album.id })}>{current.album}</a>
            {:else}
              <span class="muted">{current.album}</span>
            {/if}
          {/if}
        </p>
      </div>
      <button
        class="icon-btn fill"
        class:on={favorite}
        aria-pressed={favorite}
        aria-label={favorite ? "Remove from favorites" : "Add to favorites"}
        onclick={() => actions.toggleTrack(current.trackId)}
      >
        <Icon name="heart" size={24} />
      </button>
    </div>

    <div class="badges">
      {#if current.explicit}<span class="badge">E</span>{/if}
      {#if badges?.label}<span class="badge accent">{badges.label}</span>{/if}
      {#if badges?.detail}<span class="badge">{badges.detail}</span>{/if}
      {#if playback?.bitPerfect}<span class="badge accent">BIT-PERFECT</span>{/if}
      {#each playback?.dspStages ?? [] as stage (stage)}<span class="badge">{stage}</span>{/each}
    </div>

    <div class="seek">
      <input
        type="range"
        min="0"
        max={Math.max(duration, 1)}
        step="1000"
        value={position}
        aria-label="Seek"
        aria-valuetext={`${clock(position)} of ${clock(duration)}`}
        oninput={(e) => (seeking = Number((e.target as HTMLInputElement).value))}
        onchange={(e) => commitSeek(Number((e.target as HTMLInputElement).value))}
      />
      <div class="times muted">
        <span>{clock(position)}</span>
        <span>{clock(duration)}</span>
      </div>
    </div>

    <div class="transport">
      <button
        class="icon-btn"
        class:on={playback?.shuffle}
        aria-pressed={playback?.shuffle ?? false}
        aria-label="Shuffle"
        onclick={() => run(api.setShuffle(!playback?.shuffle))}
      >
        <Icon name="shuffle" size={22} />
      </button>
      <button class="icon-btn big" aria-label="Previous" onclick={() => run(api.prev())}>
        <Icon name="skip-back-solid" size={30} />
      </button>
      <button
        class="play"
        aria-label={playback?.playing ? "Pause" : "Play"}
        onclick={() => run(api.playPause())}
      >
        <Icon name={playback?.playing ? "pause" : "play"} size={32} />
      </button>
      <button class="icon-btn big" aria-label="Next" onclick={() => run(api.next())}>
        <Icon name="skip-forward-solid" size={30} />
      </button>
      <button
        class="icon-btn"
        class:on={playback?.repeat !== "off"}
        aria-label={`Repeat: ${playback?.repeat ?? "off"}`}
        onclick={() => run(api.setRepeat(nextRepeat(playback?.repeat ?? "off")))}
      >
        <Icon name={playback?.repeat === "one" ? "repeat-1" : "repeat"} size={22} />
      </button>
    </div>

    {#if store.playbackError}
      <div class="error" role="alert">
        <Icon name="alert-triangle" size={18} />
        <span>{store.playbackError}</span>
        <button class="pill" onclick={() => run(api.retry())}>Retry</button>
      </div>
    {/if}

    <div class="volume">
      <Icon name={shownVolume === 0 ? "volume-x" : shownVolume < 0.5 ? "volume-1" : "volume-2"} size={20} />
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={shownVolume}
        aria-label="Volume"
        oninput={(e) => onVolume(Number((e.target as HTMLInputElement).value))}
        onchange={(e) => commitVolume(Number((e.target as HTMLInputElement).value))}
      />
    </div>

    <div class="extras">
      <button
        class="icon-btn"
        aria-label="Pause with fade"
        title="Pause with fade"
        onclick={() => run(api.fadeAndPause(REMOTE_FADE_MS))}
        disabled={!playback?.playing}
      >
        <Icon name="moon" size={20} />
      </button>
      <button
        class="icon-btn"
        class:on={showLyrics}
        aria-pressed={showLyrics}
        aria-label="Lyrics"
        onclick={() => (showLyrics = !showLyrics)}
      >
        <Icon name="music" size={20} />
      </button>
      <button
        class="icon-btn"
        aria-label="More"
        onclick={() =>
          actions.openTrack({
            id: current.trackId,
            title: current.title,
            artist: current.artist,
            album: current.album,
            cover: current.cover ?? null,
            duration: current.durationSecs,
            explicit: current.explicit ?? false,
            artistId: details?.artists[0]?.id ?? null,
            albumId: details?.album?.id ?? null,
          })}
      >
        <Icon name="ellipsis" size={20} />
      </button>
      <a class="icon-btn" href="#/queue" aria-label="Queue">
        <Icon name="list-music" size={20} />
      </a>
    </div>
  </section>
{/if}

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 80px 24px;
    color: var(--text-muted);
    text-align: center;
  }
  .np {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px var(--remote-gutter) 24px;
    max-width: 560px;
    margin: 0 auto;
  }
  .cover {
    width: min(100%, 52vh);
    aspect-ratio: 1;
    margin: 0 auto;
  }
  .cover :global(.art) {
    width: 100% !important;
    height: 100% !important;
    border-radius: var(--radius-card);
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h1 {
    font-size: 24px;
  }
  .titles p {
    margin: 2px 0 0;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-height: 20px;
  }
  .seek input,
  .volume input {
    width: 100%;
    accent-color: var(--accent);
    height: 28px;
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .icon-btn.big {
    color: var(--text);
    width: 56px;
    height: 56px;
  }
  .play {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
  }
  .play :global(svg path),
  .play :global(svg rect) {
    fill: currentColor;
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-muted);
  }
  .extras {
    display: flex;
    justify-content: space-around;
  }
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    border-radius: var(--radius-control);
    color: var(--danger);
  }
  .error span {
    flex: 1;
  }
</style>
