<script lang="ts">
  import { tick } from "svelte";
  import { menu, type MenuItem } from "../menu.svelte";
  import { SORT_LABELS, type SortKey } from "../trackSort";
  import type { TrackView } from "../trackView.svelte";
  import Icon from "./Icon.svelte";

  /// The active sort as chips (MS-D7): click flips a level, × removes it,
  /// Alt+←/→ reorders, "Add sort" offers the table's unused keys.
  let { view, keys }: { view: TrackView; keys: readonly SortKey[] } = $props();

  let root = $state<HTMLDivElement | null>(null);

  const unused = $derived(keys.filter((key) => !view.levels.some((level) => level.key === key)));

  function openAdd(event: MouseEvent) {
    const opener = event.currentTarget as HTMLElement;
    const rect = opener.getBoundingClientRect();
    const first = view.levels.length === 0;
    const items: MenuItem[] = unused.map((key) => ({
      id: key,
      label: SORT_LABELS[key],
      section: first ? "Sort by" : "Then by",
      run: () => (first ? view.toggleSort(key) : view.addSort(key)),
    }));
    menu.show(rect.left, rect.bottom + 4, items, opener);
  }

  async function onChipKeydown(event: KeyboardEvent, index: number) {
    if (!event.altKey || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    event.preventDefault();
    const to = index + (event.key === "ArrowLeft" ? -1 : 1);
    if (to < 0 || to >= view.levels.length) return;
    view.moveSort(index, to);
    await tick();
    root?.querySelector<HTMLButtonElement>(`[data-level="${to}"]`)?.focus();
  }
</script>

<div class="sort-bar" bind:this={root} role="group" aria-label="Sort">
  {#if view.levels.length === 0}
    <button class="quiet" type="button" onclick={openAdd} disabled={view.preparing || unused.length === 0}>
      <Icon name="arrow-up-down" size={14} /> Sort
    </button>
  {:else}
    <span class="lead">Sorted by</span>
    {#each view.levels as level, i (level.key)}
      <span class="chip" class:primary={i === 0}>
        <button
          type="button"
          class="chip-main"
          data-level={i}
          disabled={view.preparing}
          title="Reverse order (Alt+←/→ to reorder)"
          aria-label={`${SORT_LABELS[level.key]}, ${level.dir === "asc" ? "ascending" : "descending"}, level ${i + 1} of ${view.levels.length}. Reverse order.`}
          onclick={() => view.flipSort(i)}
          onkeydown={(event) => onChipKeydown(event, i)}
        >
          {#if view.levels.length > 1}<span class="rank">{i + 1}</span>{/if}
          {SORT_LABELS[level.key]}
          <Icon name={level.dir === "asc" ? "arrow-up" : "arrow-down"} size={12} />
        </button>
        <button
          type="button"
          class="chip-remove"
          disabled={view.preparing}
          aria-label={`Remove ${SORT_LABELS[level.key]} sort`}
          title="Remove"
          onclick={() => view.removeSort(i)}
        ><Icon name="x" size={12} /></button>
      </span>
    {/each}
    {#if unused.length > 0}
      <button class="quiet" type="button" onclick={openAdd} disabled={view.preparing}>
        <Icon name="plus" size={14} /> Add sort
      </button>
    {/if}
    {#if view.levels.length > 1}
      <button class="quiet" type="button" onclick={() => view.clearSort()} disabled={view.preparing}>Clear</button>
    {/if}
  {/if}
</div>

<style>
  .sort-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 30px;
  }
  .lead {
    font-size: 12px;
    color: var(--text-muted);
    margin-right: 2px;
  }
  .quiet {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
  }
  .quiet:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 28px;
    border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--line));
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-2));
    color: var(--text);
    font-size: 12px;
  }
  .chip.primary {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--line));
    background: color-mix(in srgb, var(--accent) 16%, var(--surface-2));
  }
  .chip-main {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 100%;
    padding: 0 6px 0 10px;
    border: 0;
    background: transparent;
    color: inherit;
  }
  .chip-main :global(svg) {
    color: var(--accent);
  }
  .rank {
    display: inline-grid;
    place-items: center;
    min-width: 16px;
    height: 16px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 10px;
    font-weight: 700;
  }
  .chip-remove {
    display: grid;
    place-items: center;
    width: 24px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text-muted);
  }
  .chip-remove:hover:not(:disabled) {
    color: var(--text);
  }
</style>
