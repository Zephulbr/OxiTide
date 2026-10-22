# Save for Later + Collaborators — owner smoke checklist

Spec: `docs/superpowers/specs/2026-10-22-oxitide-save-for-later-collaborators-design.md`
Plan: `docs/superpowers/plans/2026-10-22-save-for-later-collaborators.md`

Real account, Windows, `npm --prefix ui run app:dev` (or a release build). Tick each line.

## Save for Later

- [ ] The sidebar's My Collection lists **Save for Later** under Tracks. It opens the
      page, and the existing saved track ("i like the way you kiss me") is there.
- [ ] Save from each surface, then check that the menu label flips to "Remove from Save
      for later":
  - [ ] a track row;
  - [ ] the player bar title (right-click);
  - [ ] an album card;
  - [ ] a playlist card;
  - [ ] the album page header **Save**;
  - [ ] the artist page header **Save**;
  - [ ] the playlist page **More** menu;
  - [ ] a sidebar/tree playlist row;
  - [ ] a mix card (Home or Mixes & Radio);
  - [ ] the mix page header **Save**.
- [ ] The radio saved in the official app ("a tombstone") shows under the **Mixes &
      Radio** chip with its cover and opens the mix page. A mix saved here appears under
      Mixes & Radio in the official app.
- [ ] The "Saved for later" toast's **Open** goes to the page.
- [ ] The page lists the new items newest first. The chips appear only for kinds that
      are present, a chip filters, and Back/restart keeps the chip.
- [ ] The Tracks chip shows a track table that plays from the clicked row ("From Save for
      Later").
- [ ] A track card plays. Right-clicking a track or artist card offers **Remove from Save
      for later**, and the card disappears.
- [ ] Items saved here appear in the official TIDAL app's Save for Later, and items saved
      there appear here after reopening the page.
- [ ] If you have a saved video, the footnote counts it.
- [ ] Rack shell: the Library tray shows "Save for Later", and the page renders in the
      display.

## Collaborators (owner side)

- [ ] An owned playlist's More menu and its tree/Playlists-page menu offer
      **Collaborators…**. A playlist you don't own offers none.
- [ ] The dialog lists you as Owner (initials avatar).
- [ ] **Copy invitation link** copies `https://tidal.com/playlist/<uuid>/invite?code=…`.
      Copying again in the same session gives the same link.
- [ ] The link opens the official app or the web player's invite flow.
- [ ] **Reset link** asks first, then copies a new link. The old link no longer works.
- [ ] Rack shell: the dialog opens from the Rack tree.

## Task 1 spike (needs a second account)

- [ ] The second account opens the link and joins. Record here what the internal
      `user-playlists/{uuid}` `collaborators` array looks like (the field names only).
- [ ] The dialog lists the collaborator (name; a picture if TIDAL sends one).
- [ ] As the collaborator, check whether OxiTide's v1 add-to-playlist works on the joined
      playlist (expected: refused today, because `owned` is creator-only). Note whether
      the official v1 write endpoint accepts a collaborator; this decides spec §7.

## Linux

- Deferred with the batched bare-metal run.
