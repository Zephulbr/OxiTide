<script lang="ts">
  import { api } from "../api";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();
  let dragFrom = $state<number | null>(null);

  function jump(index: number) {
    const items = player.state.queue;
    if (index >= 0 && index < items.length) {
      api.playTracks(items, index);
    }
  }
</script>

<div style="padding:12px">
  <div style="display:flex;justify-content:space-between;align-items:center">
    <strong>Queue</strong>
    <button onclick={() => player.clear()}>Clear</button>
  </div>
  <ul style="list-style:none;margin:8px 0;padding:0">
    {#each player.state.queue as item, i (item.track_id + "-" + i)}
      <li
        draggable="true"
        ondragstart={() => (dragFrom = i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => { if (dragFrom !== null && dragFrom !== i) player.reorder(dragFrom, i); dragFrom = null; }}
        style="padding:4px;border-bottom:1px solid var(--line);{player.state.current?.track_id === item.track_id ? 'background:var(--panel);' : ''}"
      >
        <button onclick={() => jump(i)} style="all:unset;cursor:pointer">
          {item.title} <span class="muted">{item.artist}</span>
        </button>
        <button onclick={() => player.remove(i)} aria-label="Remove">✕</button>
      </li>
    {/each}
    {#if player.state.queue.length === 0}<li class="muted">Queue is empty</li>{/if}
  </ul>
</div>
