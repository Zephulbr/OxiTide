<script lang="ts">
  import { api } from "../api";
  import { copyText } from "../clipboard";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Icon from "./Icon.svelte";
  import Cover from "./Cover.svelte";
  import { appearance } from "../appearance.svelte";
  import { playSource } from "../playSource.svelte";
  import { rowHeightFor } from "../rowHeight";
  import VirtualList from "./VirtualList.svelte";
  import { menuFor } from "../menu.svelte";
  import { queueMenuItems } from "../menus";
  import type { DragParams } from "../drag";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();
  let confirmingClear = $state(false);

  /// Row height follows the density setting; mirrors --row-height via rowHeightFor.
  const queueRowHeight = $derived(rowHeightFor(appearance.density));

  const queueDrag: DragParams = {
    enabled: () => true,
    count: () => player.state.queue.length,
    rowHeight: () => queueRowHeight,
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
  <div style="margin:8px 0">
    <VirtualList
      total={player.state.queue.length}
      rowHeight={queueRowHeight}
      maxHeight="calc(100vh - 80px)"
      bottomPadding="calc(var(--player-height) + 12px)"
      drag={queueDrag}
      role="list"
    >
      {#snippet children(index)}
        {@const item = player.state.queue[index]}
        {@const playing = player.state.current?.track_id === item?.track_id}
        {#if item}
          <!-- A plain list item holding two sibling controls (play, remove):
               no interactive element nests inside another. -->
          <div
            role="listitem"
            draggable="true"
            data-drag-index={index}
            aria-posinset={index + 1}
            aria-setsize={player.state.queue.length}
            style="height:{queueRowHeight}px;box-sizing:border-box;display:flex;align-items:center;gap:8px;padding:4px;border-bottom:1px solid var(--line);{playing ? 'background:color-mix(in srgb, var(--accent) 12%, transparent);' : ''}"
          >
            <span class="muted handle" style="cursor:grab" title="Drag to reorder" aria-hidden="true"><Icon name="grip-vertical" size={14} /></span>
            <button
              type="button"
              class="row-main"
              aria-label={`${item.title} — ${item.artist}`}
              title="Play"
              use:menuFor={() =>
                queueMenuItems({
                  index,
                  playNow: () => jump(index),
                  copyTitle: () => void copyText(item.title),
                  remove: () => player.remove(index),
                })}
              onclick={() => jump(index)}
            >
              <span class="thumb-wrap"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
              <span class="row-title" class:playing>
                {item.title} <span class="muted">{item.artist}</span>
              </span>
            </button>
            <button onclick={() => player.remove(index)} aria-label={`Remove ${item.title}`} title="Remove"><Icon name="x" size={14} /></button>
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
        playSource.set(null);
        player.clear();
      }}
      oncancel={() => (confirmingClear = false)}
    />
  {/if}
</div>

<style>
  .row-main {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .thumb-wrap {
    flex: 0 0 auto;
    width: var(--thumb-size);
  }
  .row-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-title.playing {
    color: var(--accent);
  }
</style>
