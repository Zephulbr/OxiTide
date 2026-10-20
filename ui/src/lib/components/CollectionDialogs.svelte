<script lang="ts">
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Loading from "./Loading.svelte";
  import Modal from "./Modal.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import { DESCRIPTION_MAX, folderColorCss, folderMeta, type FolderColorId } from "../folderMeta.svelte";
  import FolderColorPicker from "./FolderColorPicker.svelte";
  import Icon from "./Icon.svelte";

  let { actions }: { actions: CollectionActions } = $props();

  const meta = folderMeta();
  let name = $state("");
  let description = $state("");
  let color = $state<FolderColorId | null>(null);
  let newColor = $state<FolderColorId | null>(null);
  let folderName = $state("");

  $effect(() => {
    const target = actions.renameTarget;
    name = target
      ? target.kind === "playlist"
        ? target.playlist.title
        : target.folder.name
      : "";
    description = target?.kind === "folder" ? meta.description(target.folder.id) : "";
    color = target?.kind === "folder" ? meta.color(target.folder.id) : null;
  });

  $effect(() => {
    if (actions.createFolderTarget) {
      folderName = "";
      newColor = null;
    }
  });

  function removeMessage(): string {
    const target = actions.removeTarget;
    if (!target) return "";
    if (target.kind === "folder") return `Remove "${target.name}"? Its playlists move back to All playlists.`;
    if (target.owned) return `Delete "${target.title}"? This cannot be undone.`;
    return `Remove "${target.title}" from your collection?`;
  }
</script>

{#if actions.renameTarget}
  <Modal
    title={actions.renameTarget.kind === "playlist" ? "Rename playlist" : "Edit folder"}
    onclose={() => actions.close()}
  >
    {@const isFolder = actions.renameTarget.kind === "folder"}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void actions.rename(name, isFolder ? { description, color } : undefined);
      }}
    >
      <label>
        Name
        <input bind:value={name} aria-label="Name" />
      </label>
      {#if isFolder}
        <div class="field">
          <span>Color</span>
          <FolderColorPicker bind:value={color} />
        </div>
        <label class="desc">
          Description
          <textarea
            bind:value={description}
            aria-label="Description"
            rows="3"
            maxlength={DESCRIPTION_MAX}
            placeholder="Optional"
          ></textarea>
          <span class="muted hint">Saved on this device only; TIDAL folders keep just a name.</span>
        </label>
      {/if}
      <p class="actions">
        <button type="button" onclick={() => actions.close()}>Cancel</button>
        <button type="submit" disabled={actions.busy || name.trim() === ""}>Save</button>
      </p>
    </form>
  </Modal>
{/if}

{#if actions.createFolderTarget}
  <Modal title="New folder" onclose={() => actions.close()}>
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void actions.createFolder(folderName, newColor);
      }}
    >
      <label>
        Name
        <input bind:value={folderName} aria-label="Folder name" />
      </label>
      <div class="field">
        <span>Color</span>
        <FolderColorPicker bind:value={newColor} />
      </div>
      <p class="actions">
        <button type="button" onclick={() => actions.close()}>Cancel</button>
        <button type="submit" disabled={actions.busy || folderName.trim() === ""}>Create</button>
      </p>
    </form>
  </Modal>
{/if}

{#if actions.removeTarget}
  <ConfirmDialog
    title={actions.removeTarget.kind === "folder"
      ? "Remove folder"
      : actions.removeTarget.owned
        ? "Delete playlist?"
        : "Remove from collection"}
    message={removeMessage()}
    onconfirm={() => void actions.confirmRemove()}
    oncancel={() => actions.close()}
  />
{/if}

{#if actions.moveTarget}
  <Modal title={`Move ${actions.moveTarget.title}`} onclose={() => actions.close()}>
    {#if actions.foldersLoading && actions.folderOptions.length === 0}
      <Loading label="Loading folders…" />
    {/if}
    {@const inFolder = actions.moveFrom !== "root"}
    <ul class="folders">
      <!-- Moving to TIDAL's top level ("root") takes the playlist out of its
           folder, so it is offered, as that, only when there is one. -->
      {#if inFolder}
        <li>
          <button class="out" onclick={() => void actions.move("root")}>
            <Icon name="folder-minus" size={16} /><span>Remove from folder</span>
          </button>
        </li>
      {/if}
      {#each actions.folderOptions as option (option.folder.id)}
        {@const hue = meta.hue(option.folder.id)}
        {@const current = option.folder.id === actions.moveFrom}
        <li style="padding-left:{option.depth * 14}px">
          <button disabled={current} onclick={() => void actions.move(option.folder.id)}>
            <span class="folder-icon" style:color={hue === null ? undefined : folderColorCss(hue)}>
              <Icon name="folder" size={16} />
            </span>
            <span>{option.folder.name}</span>
            {#if current}<span class="muted here">Current</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
    {#if actions.folderOptions.length === 0 && !actions.foldersLoading}
      <p class="muted">No folders yet.</p>
    {/if}
  </Modal>
{/if}

<style>
  label {
    display: grid;
    gap: 6px;
  }
  .desc,
  .field {
    margin-top: 12px;
  }
  .field {
    display: grid;
    gap: 6px;
  }
  textarea {
    resize: vertical;
    font: inherit;
  }
  .hint {
    font-size: 12px;
  }
  input,
  textarea {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 40vh;
    overflow: auto;
  }
  .folders button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: left;
    padding: 7px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .folders button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .folder-icon {
    display: contents;
  }
  /* The folder it is in now: shown for place, not a target. */
  .folders button:disabled {
    opacity: 1;
    cursor: default;
    background: none;
    color: var(--text-muted);
  }
  .here {
    margin-left: auto;
    font-size: 12px;
  }
  /* "Remove from folder" is the one action that is not a destination. */
  .folders .out {
    margin-bottom: 4px;
    border-bottom: 1px solid var(--line);
    border-radius: var(--radius-control) var(--radius-control) 0 0;
  }
</style>
