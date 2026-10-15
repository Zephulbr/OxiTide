<script lang="ts">
  import type { FeedActivity } from "../api";
  import { groupActivities } from "../feed";
  import type { Nav } from "../nav.svelte";
  import Cover from "./Cover.svelte";

  /// Feed-only renderer: release activities grouped into date buckets, hanging
  /// off a vertical rail (one node per group). Each group is a grid of release
  /// covers; releases TIDAL reports unseen carry a "New" mark in the newest
  /// group only (this client never marks them seen). Cards open the album.
  let { activities, nav }: { activities: FeedActivity[]; nav: Nav } = $props();

  const groups = $derived(groupActivities(activities, new Date()));
</script>

<div class="timeline">
  {#each groups as group (group.label)}
    <section class="group">
      <h3 class="group-head">
        <span class="node"></span>
        <span class="group-title">{group.label}</span>
        <span class="muted n tnum">{group.activities.length}</span>
      </h3>
      <ul class="cards pane">
        {#each group.activities as activity, i (`${activity.id}-${i}`)}
          <li>
            <button class="card" onclick={() => nav.goto({ name: "album", id: activity.id })}>
              <span class="art">
                <Cover uuid={activity.artwork} size={320} radius={0} alt="" />
                {#if !activity.seen && group.label === "New updates"}<span class="new">New</span>{/if}
              </span>
              <span class="title-line">
                <span class="title">{activity.title}</span>
                {#if activity.explicit}<span class="badge" title="Explicit">E</span>{/if}
                {#if activity.hires}<span class="badge" title="Hi-Res Lossless">H</span>{/if}
              </span>
              <span class="sub">{activity.subtitle}</span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>

<style>
  .timeline {
    position: relative;
  }
  /* The rail: one continuous hairline through every group. */
  .timeline::before {
    content: "";
    position: absolute;
    left: 12px;
    top: 12px;
    bottom: 12px;
    width: 1px;
    background: var(--line);
  }
  .group + .group {
    margin-top: 28px;
  }
  .group-head {
    display: grid;
    grid-template-columns: 24px auto auto 1fr;
    gap: 12px;
    align-items: center;
    margin: 0 0 6px;
    font-size: 15px;
    font-weight: 600;
  }
  .group-head::after {
    content: "";
    height: 1px;
    background: var(--line);
  }
  .group-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    font-size: 12.5px;
    font-weight: 500;
  }
  .node {
    position: relative;
    justify-self: center;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 8px 16px;
    margin: 0;
    padding: 0 0 0 36px;
    list-style: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    min-width: 0;
    padding: 12px;
    border-radius: var(--radius-card);
    text-align: left;
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    position: relative;
    display: block;
    margin-bottom: 6px;
    overflow: hidden;
    aspect-ratio: 1;
    border-radius: var(--radius-card);
    background: var(--surface-2);
  }
  .art :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .art :global(img) {
    transition: transform var(--dur-state) var(--ease-enter);
  }
  .card:hover .art :global(img) {
    transform: scale(1.03);
  }
  .new {
    position: absolute;
    top: 8px;
    left: 8px;
    padding: 3px 8px;
    border-radius: 99px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }
  .title-line {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
  }
  /* Release names run long ("… (Live in Budapest)"): two lines, then clip. */
  .title {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-weight: 500;
    line-height: 1.3;
    overflow: hidden;
  }
  .badge {
    margin-top: 2px;
  }
  .badge {
    flex: 0 0 auto;
    padding: 0 4px;
    border: 1px solid var(--line);
    border-radius: 4px;
    font-size: 10px;
    font-weight: 700;
    line-height: 15px;
    color: var(--text-muted);
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
