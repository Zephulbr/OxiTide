<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { formatDuration } from "../../lib/format";
  import { art, type RowTrack } from "../convert";
  import Art from "./Art.svelte";

  let {
    track,
    active = false,
    index = 0,
    number = null,
    onplay,
    onmenu,
  }: {
    track: RowTrack;
    active?: boolean;
    /// Position in the list: the first rows rise in one after another.
    index?: number;
    /// Album track number instead of artwork.
    number?: number | null;
    onplay: () => void;
    onmenu: () => void;
  } = $props();
</script>

<div class="row" class:active style:--delay={`${index < 16 ? index * 24 : 0}ms`}>
  <button class="main" onclick={onplay} aria-label={`Play ${track.title} by ${track.artist}`}>
    {#if number !== null}
      <span class="num muted">{number}</span>
    {:else}
      <Art src={art(track.cover, 80)} size={44} />
    {/if}
    <span class="text">
      <span class="title ellipsis">{track.title}</span>
      <span class="sub muted ellipsis">
        {#if track.explicit}<span class="badge">E</span>{/if}
        {track.artist}
      </span>
    </span>
    {#if track.duration > 0}
      <span class="time muted">{formatDuration(track.duration)}</span>
    {/if}
  </button>
  <button class="icon-btn" onclick={onmenu} aria-label={`More for ${track.title}`}>
    <Icon name="ellipsis" size={20} />
  </button>
</div>

<style>
  .row {
    animation: row-in var(--dur-view) var(--ease-enter) both;
    animation-delay: var(--delay, 0ms);
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 60px;
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
  .main:active {
    opacity: 0.7;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    text-align: left;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title {
    font-weight: 600;
  }
  .active .title {
    color: var(--accent);
  }
  .sub {
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .badge {
    font-size: 9px;
    padding: 0 4px;
  }
  .num {
    width: 44px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .time {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
</style>
