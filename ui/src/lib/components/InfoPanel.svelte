<script lang="ts">
  import { formatDuration } from "../format";
  import { info } from "../info.svelte";
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import { trackQualityLabel } from "../quality";
  import ErrorState from "./ErrorState.svelte";

  let {
    trackId,
    nav,
    onNavigate,
  }: { trackId: number; nav: Nav; onNavigate?: () => void } = $props();

  $effect(() => {
    void info.open(trackId);
  });

  const track = $derived(info.trackId === trackId ? info.track : null);
  const credits = $derived(info.trackId === trackId ? info.credits : null);
  const loading = $derived(info.trackId !== trackId || info.loading);
  const error = $derived(info.trackId === trackId ? info.error : "");
  const creditsError = $derived(info.trackId === trackId ? info.creditsError : "");
  const quality = $derived(track ? trackQualityLabel(track.audioQuality) : "");

  function go(route: Route) {
    onNavigate?.();
    nav.goto(route);
  }
</script>

{#if loading}
  <p class="muted pad">Loading track info…</p>
{:else if error}
  <ErrorState message={error} onRetry={() => info.retry()} />
{:else if track}
  <div class="info-panel">
    <dl class="facts">
      <div>
        <dt>Album</dt>
        <dd>
          {#if track.album}
            {@const album = track.album}
            <button onclick={() => go({ name: "album", id: album.id })}>{album.title}</button>
          {:else}
            <span class="muted">—</span>
          {/if}
        </dd>
      </div>
      <div>
        <dt>Artists</dt>
        <dd>
          {#each track.artists as artist (artist.id)}
            <button onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
          {/each}
        </dd>
      </div>
      <div>
        <dt>Duration</dt>
        <dd>{formatDuration(track.duration)}</dd>
      </div>
      {#if quality}
        <div>
          <dt>Quality</dt>
          <dd>{quality}</dd>
        </div>
      {/if}
    </dl>

    <section class="credits" aria-label="Credits">
      {#if creditsError}
        <p class="error">{creditsError}</p>
        <button class="retry" onclick={() => info.retry()}>Retry</button>
      {:else if credits && credits.roles.length > 0}
        {#each credits.roles as role (role.role)}
          <div class="role">
            <h3>{role.role}</h3>
            <p>
              {#each role.contributors as contributor, i (contributor.name + i)}
                {#if i > 0}<span class="sep">·</span>{/if}{#if contributor.artistId !== null && contributor.artistId !== undefined}<button onclick={() => go({ name: "artist", id: contributor.artistId! })}>{contributor.name}</button>{:else}<span>{contributor.name}</span>{/if}
              {/each}
            </p>
          </div>
        {/each}
      {:else if credits}
        <p class="muted">No credits available for this track.</p>
      {/if}
    </section>
  </div>
{:else}
  <p class="muted pad">Track not found.</p>
{/if}

<style>
  .info-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
  }
  .pad {
    padding: 8px 0;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
  }
  .facts div {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    gap: 8px;
    align-items: baseline;
  }
  .facts dt {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .facts dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .credits {
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 1px solid var(--line);
    padding-top: 14px;
  }
  .role h3 {
    margin: 0 0 2px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .role p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.5;
  }
  .sep {
    color: var(--text-muted);
    padding: 0 6px;
  }
  .retry {
    align-self: flex-start;
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
</style>
