<script lang="ts">
  import { tick } from "svelte";
  import { api, type EditableProfile, type ProfileArt } from "../api";
  import { linkBrand } from "../artistLinks";
  import { errorMessage } from "../errors";
  import { profileEditUrl } from "../ownProfile";
  import {
    cleanUsername,
    draftFrom,
    draftProblem,
    hasChanges,
    missingSocials,
    normalizeLink,
    profileChanges,
    socialFor,
    USERNAME_RULE,
    usernameValid,
    type ProfileDraft,
    type SocialKind,
  } from "../profileEdit";
  import { profilePicture } from "../profilePicture.svelte";
  import BrandIcon from "./BrandIcon.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";
  import Modal from "./Modal.svelte";

  /// The account's own profile, edited in place: name, username, bio, links,
  /// picture, banner and logo, written to TIDAL (`update_my_profile` and the
  /// art commands). `picture` is the artist record's picture (uuid), the one
  /// a new picture replaces.
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

  /// Each art slot: a chosen file waits for Save; `remove` clears the
  /// current one. Saved in this order.
  const ARTS: ProfileArt[] = ["picture", "banner", "logo"];
  interface Slot {
    file: File | null;
    preview: string | null;
    remove: boolean;
  }
  const emptySlot = (): Slot => ({ file: null, preview: null, remove: false });
  /// One row per slot: `label` is shown (and names its file picker), `noun`
  /// names it in the buttons.
  const ART_ROWS: { art: ProfileArt; label: string; noun: string }[] = [
    { art: "picture", label: "Profile picture", noun: "picture" },
    { art: "banner", label: "Banner", noun: "banner" },
    { art: "logo", label: "Logo", noun: "logo" },
  ];
  let slots = $state<Record<ProfileArt, Slot>>({ picture: emptySlot(), banner: emptySlot(), logo: emptySlot() });
  let inputs = $state<Record<ProfileArt, HTMLInputElement | null>>({ picture: null, banner: null, logo: null });

  /// A username change asks first, as TIDAL's editor does: links to the
  /// profile elsewhere use it.
  let confirmingUsername = $state(false);

  const problem = $derived(original ? draftProblem(draft, original) : null);
  const usernameOk = $derived(usernameValid(draft.handle));
  const changes = $derived(original ? profileChanges(original, draft) : null);
  const dirty = $derived(
    (changes !== null && hasChanges(changes)) || ARTS.some((art) => slots[art].file !== null || slots[art].remove),
  );
  const socialsToAdd = $derived(missingSocials(draft.links));

  /// Whether TIDAL has this art now (a Remove applies to it).
  function hasArt(art: ProfileArt): boolean {
    if (!original) return false;
    if (art === "picture") return original.hasPicture;
    return (art === "banner" ? original.banner : original.logo) != null;
  }

  /// Whether the slot shows an image: a chosen file, or TIDAL's not removed.
  function showsArt(art: ProfileArt): boolean {
    return slots[art].file !== null || (hasArt(art) && !slots[art].remove);
  }

  /// The banner's or logo's thumbnail: the chosen file, else TIDAL's.
  function artSrc(art: ProfileArt): string | null {
    if (slots[art].preview) return slots[art].preview;
    if (art === "picture" || slots[art].remove || !original) return null;
    return (art === "banner" ? original.banner : original.logo)?.href ?? null;
  }

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
    const urls = ARTS.map((art) => slots[art].preview);
    return () => {
      for (const url of urls) if (url) URL.revokeObjectURL(url);
    };
  });

  function pick(art: ProfileArt, event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    slots[art] = { file, preview: URL.createObjectURL(file), remove: false };
    saveError = "";
  }

  function clearArt(art: ProfileArt) {
    slots[art] = { file: null, preview: null, remove: true };
    const input = inputs[art];
    if (input) input.value = "";
  }

  async function addLink(social: SocialKind | null) {
    draft.links = [...draft.links, { href: "", social }];
    await tick();
    document.getElementById(`profile-link-${draft.links.length - 1}`)?.focus();
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
      for (const art of ARTS) {
        const slot = slots[art];
        if (slot.file) {
          const url = await api.uploadMyProfileArt(art, await fileBase64(slot.file));
          // Shown at once; TIDAL's artist record takes a while to follow.
          if (art === "picture" && url) profilePicture.changed(artistId, url, picture);
        } else if (slot.remove) {
          await api.removeMyProfileArt(art);
          if (art === "picture") profilePicture.changed(artistId, null, picture);
        } else {
          continue;
        }
        slots[art] = emptySlot();
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
    <div class="confirm arrive">
      <p class="confirm-lead">Your username will be @{cleanUsername(draft.handle)}</p>
      <p class="muted">If you change your username later, be sure to update links to your profile on other platforms.</p>
      <p class="actions">
        <button type="button" class="btn ghost" onclick={() => (confirmingUsername = false)}>Cancel</button>
        <button type="button" class="btn primary" onclick={() => void commit()}>Save</button>
      </p>
    </div>
  {:else if loadError}
    <div class="sized">
      <p class="error" role="alert">{loadError}</p>
      <p class="actions"><button type="button" class="btn ghost" onclick={onclose}>Close</button></p>
    </div>
  {:else if !original}
    <div class="sized loading-box"><Loading label="Loading your profile…" /></div>
  {:else}
    {@const profile = original}
    <form class="arrive" onsubmit={submit}>
      <!-- Picture, banner and logo: one row each, thumbnail, name, tools.
           The banner and logo are uploaded to TIDAL but not shown in
           OxiTide's artist pages yet (they read an API without them). -->
      <div class="media">
        {#each ART_ROWS as { art, label, noun } (art)}
          {@const shown = showsArt(art)}
          {@const src = artSrc(art)}
          {@const action = `${shown ? "Change" : "Add"} ${noun}`}
          <div class="media-row">
            <span class="thumb {art}" class:empty={!shown}>
              {#if src}
                {#key src}<img class="art-in" {src} alt="" />{/key}
              {:else if art === "picture" && shown}
                <span class="art-in">
                  <Cover uuid={profilePicture.for(profile.artistId, picture)} size={160} radius={999} alt="" />
                </span>
              {:else}
                <Icon name={art === "picture" ? "user" : "image"} size={art === "picture" ? 22 : 16} />
              {/if}
            </span>
            <span class="media-text">
              <span class="media-label">{label}</span>
              <span class="muted hint">PNG or JPEG</span>
            </span>
            <span class="media-tools">
              <button
                type="button"
                class="tool-btn square"
                aria-label={action}
                title={action}
                onclick={() => inputs[art]?.click()}
                disabled={saving}><Icon name={shown ? "pencil" : "plus"} size={15} /></button
              >
              {#if shown}
                <button
                  type="button"
                  class="tool-btn square remove"
                  aria-label={`Remove ${noun}`}
                  title={`Remove ${noun}`}
                  onclick={() => clearArt(art)}
                  disabled={saving}><Icon name="trash-2" size={15} /></button
                >
              {/if}
            </span>
            <input
              bind:this={inputs[art]}
              class="file"
              type="file"
              accept="image/jpeg,image/png"
              aria-label={label}
              onchange={(e) => pick(art, e)}
            />
          </div>
        {/each}
      </div>

      <div class="field">
        <label for="profile-name">Name</label>
        <input id="profile-name" class="text" bind:value={draft.name} maxlength="100" autocomplete="off" />
      </div>
      <div class="field">
        <label for="profile-username">Username</label>
        <span class="box" class:invalid={!usernameOk}>
          <span class="at" aria-hidden="true">@</span>
          <input
            id="profile-username"
            bind:value={draft.handle}
            aria-invalid={!usernameOk}
            aria-describedby={usernameOk ? undefined : "profile-username-rule"}
            placeholder={USERNAME_RULE}
            maxlength="50"
            autocomplete="off"
            spellcheck="false"
          />
        </span>
        <!-- TIDAL's rule is the field's placeholder; it shows again, red,
             only while the typed name breaks it. -->
        {#if !usernameOk}
          <span id="profile-username-rule" class="hint rule-broken">{USERNAME_RULE}</span>
        {/if}
      </div>
      <div class="field">
        <label for="profile-bio">Bio</label>
        <textarea
          id="profile-bio"
          class="text"
          bind:value={draft.bio}
          rows="4"
          maxlength="1000"
          disabled={profile.bioId == null}
        ></textarea>
        {#if profile.bioId == null}
          <span class="muted hint">
            A first bio can only be added in
            <button
              type="button"
              class="inline-link"
              onclick={() => void api.openExternal(profileEditUrl(profile.artistId))}>TIDAL's editor</button
            >; after that it is edited here.
          </span>
        {/if}
      </div>

      <div class="field">
        <span id="profile-links-label" class="label">Links</span>
        {#if draft.links.length > 0}
          <ul class="links" aria-labelledby="profile-links-label">
            {#each draft.links as row, i (i)}
              {@const social = socialFor(row.social)}
              {@const brand = social?.brand ?? linkBrand("", normalizeLink(row.href))}
              {@const name = social?.label ?? `Link ${i + 1}`}
              <li class="arrive">
                <span class="box">
                  <span class="link-mark" aria-hidden="true">
                    {#if brand}<BrandIcon {brand} size={15} />{:else}<Icon name="globe" size={15} />{/if}
                  </span>
                  <input
                    id={`profile-link-${i}`}
                    bind:value={draft.links[i].href}
                    aria-label={name}
                    placeholder={social ? "Username or link" : "yoursite.com"}
                    spellcheck="false"
                  />
                </span>
                <button
                  type="button"
                  class="icon-btn"
                  aria-label={`Remove ${social ? social.label : `link ${i + 1}`}`}
                  title="Remove"
                  onclick={() => (draft.links = draft.links.filter((_, j) => j !== i))}
                  ><Icon name="x" size={15} /></button
                >
              </li>
            {/each}
          </ul>
        {/if}
        <!-- Add link, then one quick-add per social field the form lacks. -->
        <div class="link-adds">
          <button type="button" class="tool-btn add-link" onclick={() => void addLink(null)}>
            <Icon name="plus" size={15} /> Add link
          </button>
          {#each socialsToAdd as social (social.kind)}
            <button
              type="button"
              class="social-add"
              aria-label={`Add ${social.label}`}
              title={`Add ${social.label}`}
              onclick={() => void addLink(social.kind)}><BrandIcon brand={social.brand} size={17} /></button
            >
          {/each}
        </div>
      </div>

      <!-- The username rule already turns red under its field. -->
      {#if problem && problem !== USERNAME_RULE}
        <p class="error" role="alert">{problem}</p>
      {:else if saveError}
        <p class="error" role="alert">{saveError}</p>
      {/if}

      <p class="actions">
        <button type="button" class="btn ghost" onclick={onclose} disabled={saving}>Cancel</button>
        <button type="submit" class="btn primary" disabled={saving || !dirty || problem !== null}
          >{saving ? "Saving…" : "Save"}</button
        >
      </p>
    </form>
  {/if}
</Modal>

<style>
  form,
  .sized,
  .confirm {
    width: min(460px, 80vw);
  }
  form {
    display: grid;
    gap: 16px;
  }
  .field {
    display: grid;
    gap: 6px;
  }
  /* Field names: small and quiet, so the values carry the form. */
  .field > label,
  .field > .label {
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
  }
  .hint {
    font-size: 12px;
  }
  .file {
    display: none;
  }

  /* Text fields: the app's field recipe (app.css `.search-input`): hover
     lifts the fill, focus takes a soft accent wash and glow. `.box` is the
     same field around a prefix (the @, a link's logo) and its input. */
  .text,
  .box {
    width: 100%;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
    transition:
      background var(--dur-state) var(--ease-enter),
      border-color var(--dur-state) var(--ease-enter),
      box-shadow var(--dur-state) var(--ease-enter);
  }
  .text {
    padding: 9px 12px;
    font: inherit;
  }
  textarea.text {
    min-height: 96px;
    resize: vertical;
    line-height: 1.45;
  }
  .text:hover:not(:disabled),
  .box:hover {
    background: var(--surface-3);
  }
  .text:focus,
  .box:focus-within {
    outline: none;
    border-color: color-mix(in srgb, var(--accent) 45%, var(--line));
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-2));
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .text:disabled {
    color: var(--text-muted);
    cursor: not-allowed;
  }
  .box {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
  }
  .box input {
    flex: 1;
    min-width: 0;
    padding: 9px 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
  }
  .box input:focus-visible {
    outline: none;
  }
  .text::placeholder,
  .box input::placeholder {
    color: var(--text-muted);
  }
  .box.invalid,
  .box.invalid:focus-within {
    border-color: var(--danger);
    box-shadow: 0 0 10px color-mix(in srgb, var(--danger) 16%, transparent);
  }
  .at {
    flex: none;
    color: var(--text-muted);
    font-size: 1.15em;
    font-weight: 600;
    line-height: 1;
  }
  .rule-broken {
    color: var(--danger);
  }

  /* Picture, banner, logo: one panel of rows split by hairlines, the
     thumbnails in one column and the tools at the right edge. */
  .media {
    display: grid;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    background: color-mix(in srgb, var(--surface-2) 60%, transparent);
  }
  .media-row {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 56px;
    padding: 10px 12px;
  }
  .media-row + .media-row {
    border-top: 1px solid var(--line);
  }
  .thumb {
    display: grid;
    place-items: center;
    flex: none;
    width: 84px;
    height: 30px;
    overflow: hidden;
    border-radius: var(--radius-control);
    background: var(--surface-2);
    box-shadow: 0 0 0 1px var(--line);
    color: var(--text-muted);
  }
  /* TIDAL cuts a banner to 16:9 (and 1:1); the preview shows the wide cut. */
  .thumb.banner {
    height: auto;
    aspect-ratio: 16 / 9;
  }
  .thumb.picture {
    width: 56px;
    height: 56px;
    margin: 0 14px;
    border-radius: 50%;
  }
  .thumb.empty {
    box-shadow: none;
    border: 1px dashed var(--line);
    background: none;
  }
  .thumb.picture.empty {
    border: 0;
    box-shadow: 0 0 0 1px var(--line);
  }
  .thumb img,
  .thumb .art-in {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumb :global(.cover) {
    width: 100%;
    height: 100%;
  }
  .thumb.logo img {
    object-fit: contain;
  }
  /* A new image settles in rather than snapping. */
  .art-in {
    animation: art-in var(--dur-state) var(--ease-enter) backwards;
  }
  @keyframes art-in {
    from {
      opacity: 0;
      transform: scale(0.94);
    }
  }
  .media-text {
    display: grid;
    flex: 1;
    min-width: 0;
    gap: 2px;
  }
  .media-label {
    font-weight: 600;
  }
  .media-tools {
    display: flex;
    flex: none;
    gap: 6px;
  }

  /* Icon-only tool buttons: the shared `.tool-btn`, square. */
  .tool-btn.square {
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
  }
  .tool-btn.square:hover:not(:disabled) :global(svg) {
    color: var(--text);
  }
  .tool-btn.remove {
    transition:
      background var(--dur-state) var(--ease-enter),
      border-color var(--dur-state) var(--ease-enter),
      transform var(--dur-micro) var(--ease-enter);
  }
  .tool-btn.remove:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--danger) 45%, transparent);
    background: color-mix(in srgb, var(--danger) 10%, var(--surface-2));
  }
  .tool-btn.remove:hover:not(:disabled) :global(svg) {
    color: var(--danger);
  }

  /* Links: each a field with its site's logo inside, and an x beside. */
  .links {
    display: grid;
    gap: 8px;
    margin: 0 0 6px;
    padding: 0;
    list-style: none;
  }
  .links li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .links li.arrive {
    animation-duration: var(--dur-state);
  }
  .link-mark {
    display: grid;
    place-items: center;
    flex: none;
    color: var(--text-muted);
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
    background: var(--surface-3);
  }
  .link-adds {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .add-link {
    height: 32px;
    margin-right: 6px;
    padding: 0 12px;
    font-size: 13px;
  }
  /* Bare logos: color and a small lift answer hover, no box. */
  .social-add {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    transition:
      color var(--dur-state) var(--ease-enter),
      transform var(--dur-micro) var(--ease-enter);
  }
  .social-add:hover,
  .social-add:focus-visible {
    color: var(--text);
    transform: translateY(-1px) scale(1.08);
  }

  .inline-link {
    padding: 0;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
  .loading-box {
    display: grid;
    place-items: center;
    min-height: 160px;
  }
  .confirm p {
    margin: 0 0 12px;
  }
  .confirm .confirm-lead {
    font-size: 15px;
    font-weight: 600;
  }

  /* Cancel and Save: the app's dialog pair (Playlist rename), a quiet
     outline and the accent fill. */
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 4px 0 0;
  }
  .confirm .actions {
    margin-top: 16px;
  }
  .btn {
    height: 36px;
    padding: 0 18px;
    border-radius: var(--radius-control);
    font-weight: 600;
    transition:
      background var(--dur-state) var(--ease-enter),
      color var(--dur-state) var(--ease-enter),
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-micro) var(--ease-enter);
  }
  .btn.ghost {
    border: 1px solid var(--line);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .btn.ghost:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text);
  }
  .btn.primary {
    min-width: 84px;
    background: var(--accent);
    color: var(--on-accent);
  }
  .btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 86%, var(--text));
  }
</style>
