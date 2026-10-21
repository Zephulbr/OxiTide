<script lang="ts">
  import { api, type PageItem, type PageSection } from "../api";
  import { errorMessage } from "../errors";
  import { classifySection, isPlayableTrack, pageTrackInput } from "../media";
  import { routeForPageItem, type Nav, type SectionSource } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import type { Player } from "../player.svelte";
  import { trackFlags } from "../trackFlags.svelte";
  import type { Toasts } from "../toasts.svelte";
  import Carousel from "./Carousel.svelte";
  import Icon from "./Icon.svelte";
  import MediaCard from "./MediaCard.svelte";
  import PageLinkGrid from "./PageLinkGrid.svelte";

  let {
    sections,
    nav,
    toasts,
    source,
    player = null,
    uncapped = false,
    showTitle = true,
  }: {
    sections: PageSection[];
    nav: Nav;
    toasts: Toasts;
    source: SectionSource;
    player?: Player | null;
    uncapped?: boolean;
    showTitle?: boolean;
  } = $props();

  const TILE_CAP = 12;
  const LIST_CAP = 8;

  let busy = $state(false);

  const playingId = $derived(player?.state.current?.track_id ?? null);

  function asTrackLike(item: PageItem) {
    return { id: Number(item.id), explicit: item.explicit ?? undefined };
  }

  function isPlaying(item: PageItem): boolean {
    return isPlayableTrack(item) && Number(item.id) === playingId;
  }

  function isBlocked(item: PageItem): boolean {
    return isPlayableTrack(item) && trackFlags.blockReasons(asTrackLike(item)).length > 0;
  }

  function feedTracks(items: PageItem[]): PageItem[] {
    return items.filter(isPlayableTrack);
  }

  /// `title` names the section a "Play all" started (the player bar's "From"
  /// line); a single track has no source.
  function play(items: PageItem[], start: number, title = "") {
    playSource.set(
      title.trim()
        ? { kind: "section", name: title, route: { name: "section", source, title } }
        : null,
      items.map((item) => Number(item.id)),
    );
    return api.playPageItems(items.map(pageTrackInput), start);
  }

  function activate(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    if (route) return () => nav.goto(route);
    if (isPlayableTrack(item)) {
      return () => {
        void play([item], 0).catch((e) =>
          toasts.show(errorMessage(e), { severity: "error" }),
        );
      };
    }
    return undefined;
  }

  async function playAll(items: PageItem[], title: string) {
    if (busy) return;
    busy = true;
    try {
      await play(items, 0, title);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  /// Visible track items across every rendered section; the flags store
  /// batches them (≤20/request, fail-open) and is a no-op when the filters and
  /// labels are off.
  const visibleTracks = $derived.by(() => {
    const tracks: PageItem[] = [];
    for (const section of sections) {
      const treatment = classifySection(section.kind, section.items);
      const effective = uncapped && treatment === "carousel" ? "tiles" : treatment;
      const cap = effective === "list" ? LIST_CAP : TILE_CAP;
      const shown = uncapped ? section.items : section.items.slice(0, cap);
      // Carousels render every item, so flags must cover every item; the list
      // treatment renders the capped `shown`.
      const rendered = effective === "carousel" ? section.items : shown;
      tracks.push(...feedTracks(rendered));
    }
    return tracks;
  });

  $effect(() => {
    trackFlags.recordTracks(visibleTracks.map(asTrackLike));
    void trackFlags.ensure(visibleTracks.map((item) => Number(item.id)));
  });
</script>

{#each sections as section, si (`${section.title}-${si}`)}
  {@const treatment = classifySection(section.kind, section.items)}
  {@const effective = uncapped && treatment === "carousel" ? "tiles" : treatment}
  {@const cap = effective === "list" ? LIST_CAP : TILE_CAP}
  {@const shown = uncapped ? section.items : section.items.slice(0, cap)}
  {@const tracks = effective === "list" ? feedTracks(section.items) : []}
  {@const capped = !uncapped && section.items.length > cap}
  {#if effective === "links"}
    {#if showTitle && section.title.trim()}
      <div class="head"><h3>{section.title}</h3></div>
    {/if}
    <PageLinkGrid items={section.items} label={section.title || "Explore"} open={activate} />
  {:else if effective === "carousel"}
    <Carousel title={section.title}>
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        {@const onclick = activate(item)}
        <MediaCard
          {item}
          variant="carousel"
          {onclick}
          playing={isPlaying(item)}
          blocked={isBlocked(item)}
        />
      {/each}
    </Carousel>
  {:else}
    {#if showTitle || tracks.length > 0 || capped}
      <div class="head">
        {#if showTitle}<h3>{section.title}</h3>{/if}
        <div class="controls">
          {#if tracks.length > 0}
            <button
              type="button"
              class="control"
              onclick={() => playAll(tracks, section.title)}
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
    <div class="grid pane" class:list={effective === "list"}>
      {#each shown as item, i (item.id || `${item.title}-${i}`)}
        {@const onclick = activate(item)}
        <MediaCard
          {item}
          variant={effective === "list" ? "row" : "tile"}
          thumb={64}
          {onclick}
          playing={isPlaying(item)}
          blocked={isBlocked(item)}
        />
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
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 20px;
    margin-bottom: 24px;
  }
  .grid.list {
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 10px 20px;
  }
</style>
