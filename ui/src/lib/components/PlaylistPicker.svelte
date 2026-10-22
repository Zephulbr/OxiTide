<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Playlist } from "../api";
  import type { PlaylistActions } from "../playlists.svelte";
  import { sortPlaylistsByRecency } from "../playlistSort";
  import Cover from "./Cover.svelte";
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

<!-- Keep one / Add both, for one duplicate (`label` names it) or all of them.
     Neither half is pressed when the rows are mixed. -->
{#snippet segmented(keep: boolean, both: boolean, onKeep: () => void, onBoth: () => void, label?: string)}
  <span class="seg" role="group" aria-label={label ? `${label}: keep one or add both` : "All duplicates"}>
    <button class:on={keep} aria-pressed={keep} onclick={onKeep}>Keep one</button>
    <button class:on={both} aria-pressed={both} onclick={onBoth}>Add both</button>
  </span>
{/snippet}

{#if actions.open}
  <Modal title={actions.step === "create" ? "Create playlist" : "Add to playlist"} onclose={close}>
    {#if actions.step === "confirm" && actions.confirming}
      <!-- A drop of more than one track asks first (drag-to-playlist DD-D15). -->
      {@const c = actions.confirming}
      <div class="confirm">
        <p class="lead">Add {c.count} tracks?</p>
        <dl class="route">
          {#if c.from}
            <div class="leg">
              <dt>From</dt>
              <dd title={c.from}>{c.from}</dd>
            </div>
          {/if}
          <div class="leg">
            <dt>To</dt>
            <dd title={c.playlist.title}>{c.playlist.title}</dd>
          </div>
        </dl>
        <p class="footer">
          <button class="cancel" onclick={close}>Cancel</button>
          <button class="save" disabled={actions.busy} onclick={() => void actions.confirmAdd()}>
            {actions.busy ? "Adding…" : `Add ${c.count} tracks`}
          </button>
        </p>
      </div>
    {:else if actions.step === "list"}
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
      {@const dupes = actions.duplicates}
      {@const copies = dupes.filter((t) => choice(t.id)).length}
      <div class="dupes-step">
        <p class="lead">
          {dupes.length === 1 ? "1 track is" : `${dupes.length} tracks are`} already in
          {#if actions.target}<span class="name" title={actions.target.title}>{actions.target.title}</span>{:else}this playlist{/if}
        </p>
        <p class="muted hint">Keep the copy that's there, or add another.</p>
        <div class="bulk">
          <span class="muted">All</span>
          {@render segmented(copies === 0, copies === dupes.length, () => actions.setAllChoices(false), () => actions.setAllChoices(true))}
        </div>
        <ul class="dupes">
          {#each dupes as t (t.id)}
            <li class="dupe">
              <span class="dupe-cover"><Cover uuid={t.album?.cover} size={80} radius={4} /></span>
              <span class="dupe-text">
                <span class="dupe-title" title={t.title}>{t.title}</span>
                <span class="muted dupe-artist">{t.artists.map((a) => a.name).join(", ")}</span>
              </span>
              {@render segmented(!choice(t.id), choice(t.id), () => actions.setChoice(t.id, false), () => actions.setChoice(t.id, true), t.title)}
            </li>
          {/each}
        </ul>
        <p class="footer">
          <button class="cancel" onclick={close}>Skip duplicates</button>
          <button class="save" disabled={actions.busy || copies === 0} onclick={() => void actions.confirmDuplicates()}>
            {copies === 0 ? "Add copies" : copies === 1 ? "Add 1 copy" : `Add ${copies} copies`}
          </button>
        </p>
      </div>
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
  .error {
    color: var(--danger);
  }
  /* Confirm and duplicates steps: a short lead, then the details, then a
     footer of real buttons (quiet secondary, accent primary). Long names
     wrap anywhere and clamp to two lines; the full name is on hover. */
  .confirm,
  .dupes-step {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(420px, 80vw);
  }
  .dupes-step {
    width: min(500px, 84vw);
  }
  .lead {
    margin: 0;
    font-size: 16px;
    font-weight: 650;
    line-height: 1.35;
  }
  .lead .name,
  .route dd {
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .lead .name {
    color: var(--accent);
  }
  .hint {
    margin: -6px 0 0;
    font-size: 13px;
  }
  .route {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 4px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
  }
  .leg {
    display: grid;
    grid-template-columns: 44px 1fr;
    align-items: baseline;
    gap: 10px;
    padding: 8px 10px;
  }
  .leg + .leg {
    border-top: 1px solid var(--line);
  }
  .route dt {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .route dd {
    margin: 0;
    min-width: 0;
    color: var(--text);
    font-weight: 600;
    line-height: 1.35;
  }
  .footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 4px 0 0;
  }
  .footer .save {
    min-width: 96px;
  }
  .bulk {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    font-size: 13px;
  }
  .dupes {
    max-height: min(46vh, 420px);
    margin: 0;
    padding: 0 4px 0 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    list-style: none;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
  }
  .dupe {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 2px;
  }
  .dupe + .dupe {
    border-top: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .dupe-cover {
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
  }
  .dupe-text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 1px;
  }
  .dupe-title,
  .dupe-artist {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dupe-title {
    font-weight: 600;
  }
  .dupe-artist {
    font-size: 13px;
  }
  /* Keep one / Add both: a two-part switch; the chosen half is filled. */
  .seg {
    flex: 0 0 auto;
    display: inline-flex;
    padding: 2px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-2);
  }
  .seg button {
    padding: 5px 11px;
    border-radius: 999px;
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
    transition:
      background var(--dur-micro) var(--ease-enter),
      color var(--dur-micro) var(--ease-enter);
  }
  .seg button:hover:not(.on) {
    color: var(--text);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--on-accent);
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
    transition: background var(--dur-micro) var(--ease-enter);
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
      background var(--dur-micro) var(--ease-enter);
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
  .cancel {
    padding: 9px 16px;
    border: 1px solid color-mix(in srgb, var(--text) 22%, transparent);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
    transition: background var(--dur-micro) var(--ease-enter);
  }
  .cancel:hover {
    background: var(--surface-3);
  }
  .save {
    padding: 9px 24px;
    border-radius: var(--radius-control);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
    transition: filter var(--dur-micro) var(--ease-enter);
  }
  .save:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .save:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
