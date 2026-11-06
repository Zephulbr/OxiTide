<script lang="ts">
  import { api, type EditableProfile } from "../api";
  import { errorMessage } from "../errors";
  import { profileEditUrl } from "../ownProfile";
  import {
    cleanUsername,
    draftFrom,
    draftProblem,
    hasChanges,
    profileChanges,
    USERNAME_RULE,
    usernameValid,
    type ProfileDraft,
  } from "../profileEdit";
  import { profilePicture } from "../profilePicture.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  /// The account's own profile, edited in place: name, username, bio, links
  /// and picture, written to TIDAL (`update_my_profile` and the picture
  /// commands). `picture` is the artist record's picture (uuid), the one a
  /// new picture replaces.
  let {
    picture,
    onsaved,
    onclose,
  }: {
    picture: string | null;
    onsaved: () => void;
    onclose: () => void;
  } = $props();

  let original = $state<EditableProfile | null>(null);
  let draft = $state<ProfileDraft>({ name: "", handle: "", bio: "", links: [] });
  let loadError = $state("");
  let saveError = $state("");
  let saving = $state(false);

  /// A chosen picture waits for Save; `removePicture` clears the current one.
  let pickedFile = $state<File | null>(null);
  let pickedPreview = $state<string | null>(null);
  let removePicture = $state(false);
  let fileInput = $state<HTMLInputElement | null>(null);

  /// A username change asks first, as TIDAL's editor does: links to the
  /// profile elsewhere use it.
  let confirmingUsername = $state(false);

  const problem = $derived(original ? draftProblem(draft, original) : null);
  const usernameOk = $derived(usernameValid(draft.handle));
  const changes = $derived(original ? profileChanges(original, draft) : null);
  const dirty = $derived(
    (changes !== null && hasChanges(changes)) || pickedFile !== null || removePicture,
  );

  $effect(() => {
    let current = true;
    api
      .getEditableProfile()
      .then((profile) => {
        if (!current) return;
        original = profile;
        draft = draftFrom(profile);
      })
      .catch((e) => {
        if (current) loadError = errorMessage(e);
      });
    return () => {
      current = false;
    };
  });

  $effect(() => {
    const url = pickedPreview;
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  });

  function pick(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    pickedFile = file;
    pickedPreview = URL.createObjectURL(file);
    removePicture = false;
    saveError = "";
  }

  function clearPicture() {
    pickedFile = null;
    pickedPreview = null;
    removePicture = true;
    if (fileInput) fileInput.value = "";
  }

  function fileBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ""));
      reader.onerror = () => reject(reader.error ?? new Error("could not read the picture"));
      reader.readAsDataURL(file);
    });
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!original || !changes || problem || saving) return;
    if (!dirty) {
      onclose();
      return;
    }
    if (changes.edit.handle !== undefined) {
      confirmingUsername = true;
      return;
    }
    void commit();
  }

  async function commit() {
    if (!original || !changes) return;
    confirmingUsername = false;
    saving = true;
    saveError = "";
    const artistId = original.artistId;
    try {
      if (hasChanges(changes)) {
        original = await api.updateMyProfile(changes.edit, changes.bio);
        draft = draftFrom(original);
      }
      if (pickedFile) {
        const url = await api.uploadMyProfilePicture(await fileBase64(pickedFile));
        // Shown at once; TIDAL's artist record takes a while to follow.
        if (url) profilePicture.changed(artistId, url, picture);
        pickedFile = null;
      } else if (removePicture) {
        await api.removeMyProfilePicture();
        profilePicture.changed(artistId, null, picture);
        removePicture = false;
      }
      onsaved();
      onclose();
    } catch (e) {
      // What was already written stays written; the form shows the rest.
      saveError = errorMessage(e);
      onsaved();
    } finally {
      saving = false;
    }
  }
</script>

<Modal title={confirmingUsername ? "Save username?" : "Edit profile"} {onclose}>
  {#if confirmingUsername}
    <div class="confirm">
      <p>Your username will be @{cleanUsername(draft.handle)}</p>
      <p class="muted">If you change your username later, be sure to update links to your profile on other platforms.</p>
      <p class="actions">
        <button type="button" onclick={() => (confirmingUsername = false)}>Cancel</button>
        <button type="button" onclick={() => void commit()}>Save</button>
      </p>
    </div>
  {:else if loadError}
    <p class="error" role="alert">{loadError}</p>
    <p class="actions"><button type="button" onclick={onclose}>Close</button></p>
  {:else if !original}
    <p class="muted">Loading your profile…</p>
  {:else}
    <form onsubmit={submit}>
      <div class="picture-row">
        <span class="portrait">
          {#if pickedPreview}
            <img src={pickedPreview} alt="" />
          {:else}
            <Cover
              uuid={removePicture ? null : profilePicture.for(original.artistId, picture)}
              size={320}
              radius={999}
              alt=""
            />
          {/if}
        </span>
        <div class="picture-actions">
          <button type="button" class="tool-btn" onclick={() => fileInput?.click()} disabled={saving}>
            <Icon name="pencil" size={14} /> Change picture
          </button>
          {#if (original.hasPicture && !removePicture) || pickedFile}
            <button type="button" class="tool-btn quiet" onclick={clearPicture} disabled={saving}>Remove</button>
          {/if}
          <span class="muted hint">JPEG or PNG; it is cropped to a square.</span>
          <input
            bind:this={fileInput}
            class="file"
            type="file"
            accept="image/jpeg,image/png"
            aria-label="Profile picture"
            onchange={pick}
          />
        </div>
      </div>

      <label>
        Name
        <input bind:value={draft.name} maxlength="100" autocomplete="off" />
      </label>
      <div class="field">
        <label for="profile-username">Username</label>
        <span class="handle">
          <span class="at" aria-hidden="true">@</span>
          <input
            id="profile-username"
            bind:value={draft.handle}
            class:invalid={!usernameOk}
            aria-invalid={!usernameOk}
            aria-describedby="profile-username-rule"
            maxlength="50"
            autocomplete="off"
            spellcheck="false"
          />
        </span>
        <span id="profile-username-rule" class="hint" class:muted={usernameOk} class:rule-broken={!usernameOk}>
          {USERNAME_RULE}
        </span>
      </div>
      <div class="field">
        <label for="profile-bio">Bio</label>
        <textarea
          id="profile-bio"
          bind:value={draft.bio}
          rows="4"
          maxlength="1000"
          disabled={original.bioId == null}
        ></textarea>
        {#if original.bioId == null}
          {@const artistId = original.artistId}
          <span class="muted hint">
            A first bio can only be added in
            <button type="button" class="inline-link" onclick={() => void api.openExternal(profileEditUrl(artistId))}
              >TIDAL's editor</button
            >; after that it is edited here.
          </span>
        {/if}
      </div>

      <div class="field">
        <span id="profile-links-label">Links</span>
        <ul class="links" aria-labelledby="profile-links-label">
          {#each draft.links as _, i (i)}
            <li>
              <input
                bind:value={draft.links[i]}
                aria-label={`Link ${i + 1}`}
                placeholder="instagram.com/you"
                spellcheck="false"
              />
              <button
                type="button"
                class="icon-btn"
                aria-label={`Remove link ${i + 1}`}
                onclick={() => (draft.links = draft.links.filter((_, j) => j !== i))}
              ><Icon name="x" size={14} /></button>
            </li>
          {/each}
        </ul>
        <button type="button" class="tool-btn quiet add" onclick={() => (draft.links = [...draft.links, ""])}>
          <Icon name="plus" size={14} /> Add link
        </button>
      </div>

      <!-- The username rule already turns red under its field. -->
      {#if problem && problem !== USERNAME_RULE}
        <p class="error" role="alert">{problem}</p>
      {:else if saveError}
        <p class="error" role="alert">{saveError}</p>
      {/if}

      <p class="actions">
        <button type="button" onclick={onclose} disabled={saving}>Cancel</button>
        <button type="submit" disabled={saving || problem !== null}>{saving ? "Saving…" : "Save"}</button>
      </p>
    </form>
  {/if}
</Modal>

<style>
  form {
    display: grid;
    gap: 14px;
    width: min(460px, 80vw);
  }
  label,
  .field {
    display: grid;
    gap: 6px;
  }
  input,
  textarea {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
  }
  textarea {
    resize: vertical;
  }
  textarea:disabled {
    color: var(--text-muted);
  }
  input::placeholder {
    color: var(--text-muted);
  }
  .hint {
    font-size: 12px;
  }
  .picture-row {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .portrait {
    flex: none;
    width: 88px;
    height: 88px;
    overflow: hidden;
    border-radius: 50%;
    box-shadow: 0 0 0 1px var(--line);
  }
  .portrait img,
  .portrait :global(.cover) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .picture-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .picture-actions .hint {
    flex-basis: 100%;
  }
  .file {
    display: none;
  }
  .handle {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .at {
    color: var(--text-muted);
  }
  .links {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .links li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .icon-btn:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .add {
    justify-self: start;
  }
  .tool-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .quiet {
    color: var(--text-muted);
  }
  .inline-link {
    padding: 0;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  input.invalid {
    border-color: var(--danger);
  }
  .rule-broken {
    color: var(--danger);
  }
  .confirm {
    width: min(460px, 80vw);
  }
  .confirm p {
    margin: 0 0 12px;
  }
  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 4px 0 0;
  }
</style>
