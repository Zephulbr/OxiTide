<script lang="ts">
  import type { FeedActivity } from "../api";
  import { groupActivities } from "../feed";
  import type { Nav } from "../nav.svelte";
  import Cover from "./Cover.svelte";

  /// Feed-only renderer: release activities grouped into date buckets, hanging
  /// off a vertical rail (one node per group label). Rows open the album.
  let { activities, nav }: { activities: FeedActivity[]; nav: Nav } = $props();

  const groups = $derived(groupActivities(activities, new Date()));
</script>

{#snippet rowInner(activity: FeedActivity)}
  <span class="thumb"><Cover uuid={activity.artwork} size={160} radius={8} alt="" /></span>
  <span class="meta">
    <span class="title-line">
      <span class="title">{activity.title}</span>
      {#if activity.explicit}<span class="badge" title="Explicit">E</span>{/if}
      {#if activity.hires}<span class="badge" title="Hi-Res Lossless">H</span>{/if}
    </span>
    <span class="sub">{activity.subtitle}</span>
  </span>
{/snippet}

<div class="timeline">
  {#each groups as group, gi (group.label)}
    <h3 class="group" class:first={gi === 0}>
      <span class="node"></span>
      <span class="group-title">{group.label}</span>
    </h3>
    <ul class="rows">
      {#each group.activities as activity, i (`${activity.id}-${i}`)}
        <li>
          <button class="row" onclick={() => nav.goto({ name: "album", id: activity.id })}>
            {@render rowInner(activity)}
          </button>
        </li>
      {/each}
    </ul>
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
    top: 10px;
    bottom: 10px;
    width: 1px;
    background: var(--line);
  }
  .group {
    display: grid;
    grid-template-columns: 24px 1fr;
    gap: 12px;
    align-items: center;
    margin: 32px 0 10px;
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-muted);
  }
  .group.first {
    margin-top: 0;
  }
  .group-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .node {
    justify-self: center;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0 0 0 36px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
    padding: 8px 12px 8px 0;
    border: 0;
    border-radius: var(--radius-card);
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .thumb {
    flex: 0 0 auto;
    width: 64px;
    overflow: hidden;
    border-radius: 10px;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .title-line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .title {
    font-size: 15px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex: 0 0 auto;
    padding: 1px 5px;
    border: 1px solid var(--line);
    border-radius: 5px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .sub {
    font-size: 13px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
