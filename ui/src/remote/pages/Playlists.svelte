<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import Art from "../components/Art.svelte";
  import Loader from "../components/Loader.svelte";
  import { useRemote } from "../context";
  import { art } from "../convert";
  import { hrefFor } from "../routes";

  /// Playlists and folders, read-only (spec §2: no management from the phone).
  let { folder }: { folder: string } = $props();
  const { api } = useRemote();
</script>

<div class="page">
  <header class="page-head">
    {#if folder !== "root"}
      <button class="icon-btn" aria-label="Back" onclick={() => history.back()}>
        <Icon name="chevron-left" size={22} />
      </button>
    {/if}
    <h1>Playlists</h1>
  </header>
  <Loader load={() => api.getCollectionItems(folder)} key={folder}>
    {#snippet children(entries)}
      {#if entries.length === 0}
        <p class="muted">Nothing here yet.</p>
      {/if}
      <ul>
        {#each entries as entry (entry.kind === "folder" ? `f-${entry.id}` : `p-${entry.uuid}`)}
          <li>
            {#if entry.kind === "folder"}
              <a href={hrefFor({ name: "playlists", folder: entry.id })}>
                <span class="folder"><Icon name="folder" size={22} /></span>
                <span class="text">
                  <span class="title ellipsis">{entry.name}</span>
                  <span class="muted">{entry.totalNumberOfItems} items</span>
                </span>
              </a>
            {:else}
              <a href={hrefFor({ name: "playlist", id: entry.uuid })}>
                <Art src={art(entry.squareImage ?? entry.image, 160)} size={52} />
                <span class="text">
                  <span class="title ellipsis">{entry.title}</span>
                  <span class="muted">{entry.numberOfTracks} tracks</span>
                </span>
              </a>
            {/if}
          </li>
        {/each}
      </ul>
    {/snippet}
  </Loader>
</div>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  a {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 64px;
  }
  .folder {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: var(--radius-card-sharp);
    background: var(--surface-2);
    color: var(--accent);
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    font-size: 13px;
  }
  .title {
    font-size: 15px;
    font-weight: 600;
  }
</style>
