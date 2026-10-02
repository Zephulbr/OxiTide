<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Playlist } from "../api";
  import type { PlaylistActions } from "../playlists.svelte";
  import { sortPlaylistsByRecency } from "../playlistSort";
  import Icon from "./Icon.svelte";
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
  let isPublic = $state(false);

  let sorted = $derived(sortPlaylistsByRecency(playlists));

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
    if (!trimmed) return;
    const navigateAfter = actions.fromPicker && actions.pendingTrackIds.length > 0;
    const created = await actions.createAndAdd(trimmed, description.trim(), isPublic);
    if (created) {
      name = "";
      description = "";
      isPublic = false;
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
    isPublic = false;
  }

  function autofocus(node: HTMLInputElement) {
    node.focus();
  }
</script>

{#if actions.open}
  <Modal title={actions.step === "create" ? "Create playlist" : "Add to playlist"} onclose={close}>
    {#if actions.step === "list"}
      <div class="list-step">
        <button class="new" disabled={actions.busy} onclick={() => actions.startCreate()}>
          <span class="new-icon"><Icon name="plus" size={16} /></span>
          New playlist…
        </button>
        <div class="sep" role="separator"></div>
        <div class="list">
          {#if loading}
            <p class="muted pad">Loading…</p>
          {:else if listError}
            <p class="error pad">{listError}</p>
            <button class="retry" onclick={loadPlaylists}>Retry</button>
          {:else}
            {#each sorted as p (p.uuid)}
              <button class="row" disabled={actions.busy} onclick={() => void actions.addTo(p)}>
                <span class="title">{p.title}</span>
                <span class="count">
                  {p.numberOfTracks} {p.numberOfTracks === 1 ? "track" : "tracks"}
                </span>
              </button>
            {/each}
            {#if playlists.length === 0}<p class="muted pad">No playlists yet</p>{/if}
          {/if}
        </div>
      </div>
    {:else if actions.step === "create"}
      <form
        class="create"
        onsubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <div class="create-body">
          <div class="cover-col">
            <div class="cover-placeholder"><Icon name="music" size={30} /></div>
            <button type="button" class="change-image" disabled title="Coming later">
              Change image
            </button>
          </div>
          <div class="fields">
            <input
              class="title-input"
              bind:value={name}
              placeholder="Title"
              aria-label="Playlist title"
              use:autofocus
            />
            <div class="desc-wrap">
              <textarea
                bind:value={description}
                placeholder="Write a description"
                aria-label="Playlist description"
                maxlength="500"
              ></textarea>
              <span class="counter">{description.length}/500 characters</span>
            </div>
            <div class="public-row">
              <span class="public-text">
                <strong>Make it public</strong>
                <span class="muted">Your playlist will be visible on your Profile and accessible by anyone.</span>
              </span>
              <button
                type="button"
                class="switch"
                class:on={isPublic}
                role="switch"
                aria-checked={isPublic}
                aria-label="Make it public"
                onclick={() => (isPublic = !isPublic)}
              >
                <span class="knob"></span>
              </button>
            </div>
          </div>
        </div>
        <p class="create-actions">
          <button
            type="submit"
            class="save"
            disabled={actions.busy || name.trim() === ""}
          >Save</button>
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
  .list-step {
    display: flex;
    flex-direction: column;
    min-width: min(320px, 76vw);
  }
  .new,
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .new:hover:not(:disabled),
  .row:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .new:disabled,
  .row:disabled {
    color: var(--text-muted);
    cursor: default;
  }
  .new-icon {
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  .sep {
    height: 1px;
    margin: 6px 0;
    background: var(--line);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: min(50vh, 420px);
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .row .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .row .count {
    flex: none;
    color: var(--text-muted);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .pad {
    margin: 0;
    padding: 9px 10px;
  }
  .retry {
    align-self: flex-start;
    margin: 0 10px 9px;
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
    color: var(--danger);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .create {
    min-width: min(460px, 76vw);
  }
  .create-body {
    display: flex;
    gap: 18px;
  }
  .cover-col {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 132px;
  }
  .cover-placeholder {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .change-image {
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text-muted);
    font-weight: 600;
  }
  .change-image:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .fields {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .title-input {
    width: 100%;
    padding: 14px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 15px;
    font-weight: 600;
  }
  .title-input::placeholder {
    color: var(--text-muted);
    font-weight: 400;
  }
  .desc-wrap {
    display: grid;
    gap: 6px;
  }
  .desc-wrap textarea {
    width: 100%;
    min-height: 132px;
    resize: vertical;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
  }
  .desc-wrap textarea::placeholder {
    color: var(--text-muted);
  }
  .counter {
    font-size: 12px;
    color: var(--text-muted);
  }
  .public-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
  }
  .public-text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .public-text strong {
    font-size: 14px;
  }
  .public-text .muted {
    font-size: 12px;
  }
  .switch {
    position: relative;
    flex: 0 0 auto;
    width: 42px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: var(--surface-3);
    cursor: pointer;
    transition: background var(--dur-micro) var(--ease-in);
  }
  .switch .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text-muted);
    transition:
      transform var(--dur-micro) var(--ease-in-out),
      background var(--dur-micro) var(--ease-in);
  }
  .switch.on {
    background: var(--accent);
  }
  .switch.on .knob {
    transform: translateX(18px);
    background: var(--on-accent);
  }
  .create-actions {
    display: flex;
    justify-content: flex-end;
    margin: 16px 0 0;
  }
  .save {
    padding: 9px 24px;
    border-radius: var(--radius-control);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
  }
  .save:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
