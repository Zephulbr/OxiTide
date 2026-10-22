<script lang="ts">
  import { api, type PlaylistMember } from "../api";
  import { copyText } from "../clipboard";
  import { collaboratorsDialog, initials } from "../collaborators.svelte";
  import { errorMessage } from "../errors";
  import type { Toasts } from "../toasts.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  /// The Collaborators dialog (spec 2026-10-22, §6.3): copy the playlist's
  /// invitation link and see who is on it. Mounted once in App.
  let { toasts }: { toasts: Toasts } = $props();

  const target = $derived(collaboratorsDialog.target);
  let members = $state<PlaylistMember[]>([]);
  let membersLoading = $state(false);
  let membersError = $state("");
  let copying = $state(false);
  let confirmingReset = $state(false);
  let hasLink = $state(false);

  // Each open loads its playlist's members; a stale answer is dropped.
  let run = 0;
  $effect(() => {
    const current = target;
    confirmingReset = false;
    copying = false;
    members = [];
    membersError = "";
    if (!current) return;
    hasLink = collaboratorsDialog.hasLink(current.uuid);
    const mine = ++run;
    membersLoading = true;
    api
      .playlistMembers(current.uuid)
      .then((list) => {
        if (mine === run) members = list;
      })
      .catch((e) => {
        if (mine === run) membersError = errorMessage(e);
      })
      .finally(() => {
        if (mine === run) membersLoading = false;
      });
  });

  async function copy(renew: boolean) {
    const current = target;
    if (!current || copying) return;
    copying = true;
    confirmingReset = false;
    try {
      const invite = await api.playlistInvite(current.uuid, renew);
      collaboratorsDialog.markLink(current.uuid);
      if (target?.uuid === current.uuid) hasLink = true;
      const copied = await copyText(invite.url);
      if (!copied) toasts.show("Couldn't copy the invitation link", { severity: "error" });
      else toasts.show(renew ? "New invitation link copied. The old one no longer works." : "Invitation link copied", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      copying = false;
    }
  }
</script>

{#if target}
  <Modal title="Collaborators" onclose={() => collaboratorsDialog.close()}>
    <div class="collab">
      <p class="playlist muted">{target.title}</p>
      <button type="button" class="row invite" onclick={() => void copy(false)} disabled={copying} aria-busy={copying}>
        <span class="well" aria-hidden="true">
          {#if copying}<span class="spinner"></span>{:else}<Icon name="copy" size={18} />{/if}
        </span>
        <span class="text">
          <span class="name">Copy invitation link</span>
          <span class="sub muted">Invite others by sharing the link</span>
        </span>
      </button>

      {#if membersLoading}
        <p class="status muted">Loading members…</p>
      {:else if membersError}
        <p class="status muted">Couldn't load the members: {membersError}</p>
      {:else}
        <ul class="members" aria-label="Members">
          {#each members as member (member.id)}
            <li class="row">
              <span class="avatar" aria-hidden="true">
                {#if member.picture}
                  <Cover uuid={member.picture} size={80} radius={40} alt="" />
                {:else}
                  {initials(member.name)}
                {/if}
              </span>
              <span class="text">
                <span class="name">{member.name}</span>
                {#if member.owner}<span class="sub muted">Owner</span>{/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      <footer>
        {#if confirmingReset}
          <span class="note">The current link will stop working.</span>
          <span class="actions">
            <button type="button" class="text-button" onclick={() => (confirmingReset = false)}>Cancel</button>
            <button type="button" class="text-button danger" onclick={() => void copy(true)}>Reset link</button>
          </span>
        {:else}
          <span class="note muted">Links work for 7 days. People who join keep access.</span>
          {#if hasLink}
            <button type="button" class="text-button" onclick={() => (confirmingReset = true)} disabled={copying}>
              Reset link
            </button>
          {/if}
        {/if}
      </footer>
    </div>
  </Modal>
{/if}

<style>
  .collab {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: min(440px, 80vw);
  }
  .playlist {
    margin: -8px 0 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px;
    border-radius: var(--radius-control);
    color: var(--text);
    text-align: left;
  }
  .invite {
    width: 100%;
    transition: background var(--dur-state) var(--ease-enter);
  }
  .invite:hover:not(:disabled),
  .invite:focus-visible {
    background: var(--surface-2);
  }
  .well,
  .avatar {
    display: grid;
    flex: none;
    place-items: center;
    width: 40px;
    height: 40px;
    overflow: hidden;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--text);
  }
  .avatar {
    font-size: 14px;
    font-weight: 600;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .sub {
    font-size: 13px;
  }
  .members {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .status {
    margin: 8px;
    font-size: 13px;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 8px;
    padding: 10px 8px 0;
    border-top: 1px solid var(--line);
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 4px;
  }
  .text-button {
    flex: none;
    padding: 4px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
  }
  .text-button:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }
  .text-button.danger {
    color: var(--danger);
  }
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--line);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: collab-spin 0.8s linear infinite;
  }
  @keyframes collab-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
