<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Playlist } from "../api";
  import type { PlaylistActions } from "../playlists.svelte";
  import Modal from "./Modal.svelte";

  let {
    actions,
    onnavigate,
  }: { actions: PlaylistActions; onnavigate: (id: string) => void } = $props();

  let playlists = $state<Playlist[]>([]);
  let loading = $state(false);
  let listError = $state("");
  let name = $state("");
  let description = $state("");
  let formError = $state("");

  $effect(() => {
    if (!actions.open || actions.step !== "list") return;
    untrack(() => void loadPlaylists());
  });

  async function loadPlaylists() {
    loading = true;
    listError = "";
    try {
      playlists = await api.getUserPlaylists();
    } catch (e) {
      listError = String(e);
    } finally {
      loading = false;
    }
  }

  async function create() {
    const trimmed = name.trim();
    if (!trimmed) {
      formError = "A name is required";
      return;
    }
    formError = "";
    const navigateAfter = actions.fromPicker && actions.pendingTrackIds.length > 0;
    const created = await actions.createAndAdd(trimmed, description.trim());
    if (created) {
      name = "";
      description = "";
      if (navigateAfter) onnavigate(created.uuid);
    }
  }

  function choice(trackId: number): boolean {
    return actions.choices.get(trackId) ?? false;
  }

  function close() {
    actions.close();
    name = "";
    description = "";
    formError = "";
  }

  function autofocus(node: HTMLInputElement) {
    node.focus();
  }
</script>

{#if actions.open}
  <Modal title="Add to playlist" onclose={close}>
    {#if actions.step === "list"}
      {#if loading}
        <p class="muted">Loading…</p>
      {:else if listError}
        <p class="error">{listError}</p>
        <button onclick={loadPlaylists}>Retry</button>
      {:else}
        <ul>
          {#each playlists as p (p.uuid)}
            <li>
              <button disabled={actions.busy} onclick={() => void actions.addTo(p)}>
                {p.title} <span class="muted">{p.numberOfTracks} tracks</span>
              </button>
            </li>
          {/each}
          {#if playlists.length === 0}<li class="muted">No playlists yet</li>{/if}
        </ul>
        <button disabled={actions.busy} onclick={() => actions.startCreate()}>New playlist…</button>
      {/if}
    {:else if actions.step === "create"}
      <form
        onsubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <p><label>Name <input bind:value={name} use:autofocus /></label></p>
        <p><label>Description <input bind:value={description} /></label></p>
        {#if formError}<p class="error">{formError}</p>{/if}
        <p class="actions">
          <button type="button" onclick={close}>Cancel</button>
          <button type="submit" disabled={actions.busy}>Create</button>
        </p>
      </form>
    {:else}
      <p class="muted">Some tracks are already in this playlist. Keep one, or add another copy.</p>
      <div class="dupes">
        {#each actions.duplicates as t (t.id)}
          <div class="dupe">
            <div class="card">
              <strong>{t.title}</strong>
              <span class="muted">{t.artists.map((a) => a.name).join(", ")}</span>
              <span class="muted">In playlist</span>
            </div>
            <div class="card">
              <strong>{t.title}</strong>
              <span class="muted">{t.artists.map((a) => a.name).join(", ")}</span>
              <span class="muted">Adding</span>
            </div>
            <div class="choice">
              <button class:on={!choice(t.id)} onclick={() => actions.setChoice(t.id, false)}>
                Keep one
              </button>
              <button class:on={choice(t.id)} onclick={() => actions.setChoice(t.id, true)}>
                Add both
              </button>
            </div>
          </div>
        {/each}
      </div>
      <p class="actions">
        <button onclick={() => actions.setAllChoices(false)}>Keep one of each</button>
        <button onclick={() => actions.setAllChoices(true)}>Add all duplicates</button>
      </p>
      <p class="actions">
        <button onclick={close}>Close (keep one of each)</button>
        <button disabled={actions.busy} onclick={() => void actions.confirmDuplicates()}>
          Confirm
        </button>
      </p>
    {/if}
  </Modal>
{/if}

<style>
  ul {
    padding-left: 18px;
  }
  .dupes {
    max-height: 45vh;
    overflow: auto;
  }
  .dupe {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 8px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .choice {
    display: flex;
    gap: 4px;
  }
  .choice .on {
    color: var(--accent);
  }
  .error {
    color: #ff6b6b;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
