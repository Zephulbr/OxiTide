<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import type { Track } from "../../lib/api";
  import { fade } from "svelte/transition";
  import CoverStage from "../components/CoverStage.svelte";
  import Lyrics from "../components/Lyrics.svelte";
  import { useRemote } from "../context";
  import { motion } from "../motion";
  import { clock, nextRepeat, qualityBadges, REMOTE_FADE_MS } from "../nowPlaying";
  import { hrefFor } from "../routes";

  const { store, api, actions } = useRemote();

  const playback = $derived(store.state);
  const current = $derived(playback?.current ?? null);
  const badges = $derived(playback ? qualityBadges(playback) : null);

  let now = $state(Date.now());
  let seeking = $state<number | null>(null);
  /// Volume being dragged on the cover (shown before the PC confirms).
  let volume = $state<number | null>(null);
  /// Level to restore when unmuting from the speaker button.
  let unmuteTo = 0.5;
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

  // Live updates while swiping are throttled; the final value always goes.
  let volumeSent = 0;
  function onVolume(value: number, final: boolean) {
    volume = value;
    store.noteVolume(value);
    const at = Date.now();
    if (final || at - volumeSent > 120) {
      volumeSent = at;
      void api
        .setVolume(value)
        .catch((e) => actions.fail(e))
        .finally(() => {
          if (final) volume = null;
        });
    }
  }

  function toggleMute() {
    if (shownVolume > 0) {
      unmuteTo = shownVolume;
      onVolume(0, true);
    } else {
      onVolume(unmuteTo || 0.5, true);
    }
  }

  // One-time hint for the swipe (it replaces the old volume bar).
  $effect(() => {
    if (!current) return;
    try {
      if (localStorage.getItem("oxi.volumeHint")) return;
      localStorage.setItem("oxi.volumeHint", "1");
    } catch {
      return;
    }
    actions.notify("Tip: swipe up or down on the artwork to change the volume");
  });

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
      <CoverStage
        cover={current.cover ?? null}
        playing={playback?.playing ?? false}
        volume={shownVolume}
        onvolume={onVolume}
      />
    {/if}

    <div class="meta">
      {#key current.trackId}
      <div class="titles" in:fade={{ duration: motion(240) }}>
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
      {/key}
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

    <div class="extras">
      <button
        class="icon-btn level"
        aria-label={shownVolume === 0 ? "Unmute" : `Mute (volume ${Math.round(shownVolume * 100)}%)`}
        title="Swipe the artwork to change the volume"
        onclick={toggleMute}
      >
        <Icon name={shownVolume === 0 ? "volume-x" : shownVolume < 0.5 ? "volume-1" : "volume-2"} size={20} />
        <span class="pct">{Math.round(shownVolume * 100)}</span>
      </button>
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
  .seek input {
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
  .level {
    width: auto;
    min-width: var(--remote-touch);
    padding: 0 10px;
    gap: 4px;
    border-radius: 999px;
    display: inline-flex;
  }
  .pct {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .play {
    transition: transform var(--dur-micro) var(--ease-enter), box-shadow var(--dur-state) var(--ease-enter);
    box-shadow: 0 10px 24px color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .play:active {
    transform: scale(0.92);
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
