<script lang="ts">
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Loading from "./Loading.svelte";
  import Modal from "./Modal.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";

  let { actions }: { actions: CollectionActions } = $props();

  let name = $state("");
  let folderName = $state("");

  $effect(() => {
    const target = actions.renameTarget;
    name = target
      ? target.kind === "playlist"
        ? target.playlist.title
        : target.folder.name
      : "";
  });

  $effect(() => {
    if (actions.createFolderTarget) folderName = "";
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
    title={actions.renameTarget.kind === "playlist" ? "Rename playlist" : "Rename folder"}
    onclose={() => actions.close()}
  >
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void actions.rename(name);
      }}
    >
      <label>
        Name
        <input bind:value={name} aria-label="Name" />
      </label>
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
        void actions.createFolder(folderName);
      }}
    >
      <label>
        Name
        <input bind:value={folderName} aria-label="Folder name" />
      </label>
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
        ? "Delete playlist"
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
    <ul class="folders">
      <li>
        <button onclick={() => void actions.move("root")}>Root</button>
      </li>
      {#each actions.folderOptions as option (option.folder.id)}
        <li style="padding-left:{option.depth * 14}px">
          <button onclick={() => void actions.move(option.folder.id)}>{option.folder.name}</button>
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
  input {
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
</style>
