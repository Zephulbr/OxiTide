<script lang="ts">
  import { api, type Video } from "../lib/api";
  import { videoCoverUrl } from "../lib/artwork";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { formatDuration } from "../lib/format";

  let videos = $state<Video[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      videos = await api.getFavoriteVideos();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Videos</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if videos.length === 0}
  <EmptyState message="No videos yet." />
{:else}
  <ul class="videos">
    {#each videos as video (video.id)}
      <li>
        <span class="thumb">
          {#if video.image}
            <img src={videoCoverUrl(video.image, "480x270")} alt="" loading="lazy" />
          {:else}
            <span class="placeholder"><Icon name="play" size={18} /></span>
          {/if}
        </span>
        <span class="text">
          <span class="title">{video.title}</span>
          <span class="muted sub">
            {video.artists.map((a) => a.name).join(", ")} · {formatDuration(video.duration)}
          </span>
        </span>
      </li>
    {/each}
  </ul>
  <p class="muted note">Video playback isn't supported yet — these are your saved videos.</p>
{/if}

<style>
  .videos {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .videos li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .thumb {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 120px;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
  }
  .note {
    margin-top: 16px;
  }
</style>
