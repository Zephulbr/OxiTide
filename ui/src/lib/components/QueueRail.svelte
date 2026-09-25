<script lang="ts">
  import { api } from "../api";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import VirtualList from "./VirtualList.svelte";
  import { menuFor } from "../menu.svelte";
  import { queueMenuItems } from "../menus";
  import type { DragParams } from "../drag";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();
  let confirmingClear = $state(false);

  /// Fixed row height; keep in sync with the row style below.
  const QUEUE_ROW_HEIGHT = 40;

  const queueDrag: DragParams = {
    enabled: () => true,
    count: () => player.state.queue.length,
    rowHeight: () => QUEUE_ROW_HEIGHT,
    onChange: () => {},
    onDrop: (from, to) => {
      if (from !== to) player.reorder(from, to);
    },
  };

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
    <button onclick={() => (confirmingClear = true)}>Clear</button>
  </div>
  <div role="list" style="margin:8px 0">
    <VirtualList
      total={player.state.queue.length}
      rowHeight={QUEUE_ROW_HEIGHT}
      maxHeight="calc(100vh - 220px)"
      drag={queueDrag}
    >
      {#snippet children(index)}
        {@const item = player.state.queue[index]}
        {#if item}
          <div
            role="listitem"
            draggable="true"
            data-drag-index={index}
            use:menuFor={() =>
              queueMenuItems({ index, playNow: () => jump(index), remove: () => player.remove(index) })}
            style="height:{QUEUE_ROW_HEIGHT}px;box-sizing:border-box;display:flex;align-items:center;padding:4px;border-bottom:1px solid var(--line);{player.state.current?.track_id === item.track_id ? 'background:var(--panel);' : ''}"
          >
            <span class="muted" style="cursor:grab;padding-right:6px" title="Drag to reorder" aria-hidden="true">⠿</span>
            <button onclick={() => jump(index)} style="all:unset;cursor:pointer;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
              {item.title} <span class="muted">{item.artist}</span>
            </button>
            <button onclick={() => player.remove(index)} aria-label="Remove">✕</button>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
  {#if player.state.queue.length === 0}<p class="muted">Queue is empty</p>{/if}
  {#if confirmingClear}
    <ConfirmDialog
      title="Clear queue"
      message="Remove all tracks from the queue?"
      onconfirm={() => {
        confirmingClear = false;
        player.clear();
      }}
      oncancel={() => (confirmingClear = false)}
    />
  {/if}
</div>
