<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { useRemote } from "../context";
  import { art } from "../convert";
  import Art from "./Art.svelte";

  const { store, api, actions } = useRemote();
  const current = $derived(store.state?.current ?? null);
  const progress = $derived(
    store.state && store.state.durationMs > 0 ? store.state.positionMs / store.state.durationMs : 0,
  );
</script>

{#if current}
  <div class="mini">
    <span class="progress" style:transform={`scaleX(${progress})`} aria-hidden="true"></span>
    <a href="#/now" class="info" aria-label="Open Now Playing">
      <Art src={art(current.cover, 80)} size={44} />
      <span class="text">
        <span class="title ellipsis">{current.title}</span>
        <span class="muted ellipsis">{current.artist}</span>
      </span>
    </a>
    <button
      class="icon-btn"
      aria-label={store.state?.playing ? "Pause" : "Play"}
      onclick={() => api.playPause().catch((e) => actions.fail(e))}
    >
      <Icon name={store.state?.playing ? "pause" : "play"} size={24} />
    </button>
    <button class="icon-btn" aria-label="Next" onclick={() => api.next().catch((e) => actions.fail(e))}>
      <Icon name="skip-forward-solid" size={22} />
    </button>
  </div>
{/if}

<style>
  .mini {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    height: var(--remote-minibar);
    padding: 0 8px 0 10px;
    border-top: 1px solid var(--line);
    background: var(--surface-float);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    overflow: hidden;
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 2px;
    background: var(--accent);
    transform-origin: left;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    font-size: 13px;
  }
  .title {
    font-weight: 600;
    font-size: 14px;
  }
  .icon-btn {
    color: var(--text);
  }
</style>
