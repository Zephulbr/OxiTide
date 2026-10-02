<script lang="ts">
  import { api, type PageItem, type PageSection } from "../api";
  import { errorMessage } from "../errors";
  import { classifySection, isPlayableTrack, playableTrackIds } from "../media";
  import { routeForPageItem, type Nav } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import type { Toasts } from "../toasts.svelte";
  import Carousel from "./Carousel.svelte";
  import Icon from "./Icon.svelte";
  import MediaCard from "./MediaCard.svelte";

  let {
    sections,
    nav,
    toasts,
    source,
    uncapped = false,
    showTitle = true,
  }: {
    sections: PageSection[];
    nav: Nav;
    toasts: Toasts;
    source: "home" | "explore" | "forYou";
    uncapped?: boolean;
    showTitle?: boolean;
  } = $props();

  const TILE_CAP = 12;
  const LIST_CAP = 8;

  let busy = $state(false);

  function play(ids: number[]) {
    playSource.set(null);
    return api.playTrackIds(ids, 0);
  }

  function activate(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    if (route) return () => nav.goto(route);
    if (isPlayableTrack(item)) {
      return () => {
        void play([Number(item.id)]).catch((e) =>
          toasts.show(errorMessage(e), { severity: "error" }),
        );
      };
    }
    return undefined;
  }

  async function playAll(ids: number[]) {
    if (busy) return;
    busy = true;
    try {
      await play(ids);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }
</script>

{#each sections as section, si (`${section.title}-${si}`)}
  {@const treatment = classifySection(section.kind, section.items)}
  {@const effective = uncapped && treatment === "carousel" ? "tiles" : treatment}
  {@const cap = effective === "list" ? LIST_CAP : TILE_CAP}
  {@const shown = uncapped ? section.items : section.items.slice(0, cap)}
  {@const ids = effective === "list" ? playableTrackIds(section.items) : []}
  {@const capped = !uncapped && section.items.length > cap}
  {#if effective === "carousel"}
    <Carousel title={section.title}>
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        {@const onclick = activate(item)}
        <MediaCard {item} variant="carousel" {onclick} />
      {/each}
    </Carousel>
  {:else}
    {#if showTitle || ids.length > 0 || capped}
      <div class="head">
        {#if showTitle}<h3>{section.title}</h3>{/if}
        <div class="controls">
          {#if ids.length > 0}
            <button
              type="button"
              class="control"
              onclick={() => playAll(ids)}
              disabled={busy}
              aria-label="Play all"
              title="Play all"
            ><Icon name="play" size={16} /></button>
          {/if}
          {#if capped}
            <button
              type="button"
              class="view-all"
              onclick={() => nav.goto({ name: "section", source, title: section.title })}
            >View all</button>
          {/if}
        </div>
      </div>
    {/if}
    <div class="grid" class:list={effective === "list"}>
      {#each shown as item, i (item.id || `${item.title}-${i}`)}
        {@const onclick = activate(item)}
        <MediaCard {item} variant={effective === "list" ? "row" : "tile"} {onclick} />
      {/each}
    </div>
  {/if}
{/each}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .head h3 {
    flex: 1;
    min-width: 0;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
  }
  .control {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: 50%;
    color: var(--text-muted);
  }
  .control:hover:not(:disabled) {
    color: var(--text);
  }
  .control:disabled {
    opacity: 0.4;
  }
  .view-all {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .view-all:hover {
    color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .grid.list {
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 8px 16px;
  }
</style>
