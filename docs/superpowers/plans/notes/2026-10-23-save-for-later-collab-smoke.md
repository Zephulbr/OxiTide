# Save for Later + Collaborators — owner smoke checklist

Spec: `docs/superpowers/specs/2026-10-22-oxitide-save-for-later-collaborators-design.md`
Plan: `docs/superpowers/plans/2026-10-22-save-for-later-collaborators.md`

Real account, Windows, `npm --prefix ui run app:dev` (or a release build). Tick each line.

## Save for Later

- [x] The sidebar's My Collection lists **Save for Later** under Tracks. It opens the
      page, and the existing saved track ("i like the way you kiss me") is there.
- [x] Save from each surface, then check that the menu label flips to "Remove from Save
      for later":
  - [x] a track row;
  - [x] the player bar title (right-click);
  - [x] an album card;
  - [x] a playlist card;
  - [x] the album page header **Save**;
  - [x] the artist page header **Save**;
  - [x] the playlist page **More** menu;
  - [x] a sidebar/tree playlist row;
  - [x] a mix card (Home or Mixes & Radio);
  - [x] the mix page header **Save**.
- [x] The radio saved in the official app ("a tombstone") shows under the **Mixes &
      Radio** chip with its cover and opens the mix page. A mix saved here appears under
      Mixes & Radio in the official app.
- [x] The "Saved for later" toast's **Open** goes to the page.
- [x] The page lists the new items newest first. The chips appear only for kinds that
      are present, a chip filters, and Back/restart keeps the chip.
- [x] The Tracks chip shows a track table that plays from the clicked row ("From Save for
      Later").
- [x] A track card plays. Right-clicking a track or artist card offers **Remove from Save
      for later**, and the card disappears.
- [x] Items saved here appear in the official TIDAL app's Save for Later, and items saved
      there appear here after reopening the page.
- [x] If you have a saved video, the footnote counts it.
- [x] Rack shell: the Library tray shows "Save for Later", and the page renders in the
      display.

## Collaborators (owner side)

- [x] An owned playlist's More menu and its tree/Playlists-page menu offer
      **Collaborators…**. A playlist you don't own offers none.
- [x] The dialog lists you as Owner (initials avatar).
- [x] **Copy invitation link** copies `https://tidal.com/playlist/<uuid>/invite?code=…`.
      Copying again in the same session gives the same link.
- [x] The link opens the official app or the web player's invite flow.
- [x] **Reset link** asks first, then copies a new link. The old link no longer works.
- [x] Rack shell: the dialog opens from the Rack tree.

## Task 1 spike (needs a second account)

- [x] The second account opens the link and joins. Record here what the internal
      `user-playlists/{uuid}` `collaborators` array looks like (the field names only).
- [x] The dialog lists the collaborator (name; a picture if TIDAL sends one).
- [x] As the collaborator, check whether OxiTide's v1 add-to-playlist works on the joined
      playlist (expected: refused today, because `owned` is creator-only). Note whether
      the official v1 write endpoint accepts a collaborator; this decides spec §7.

## Linux

- Deferred with the batched bare-metal run.

**Closed 2026-10-26 (owner):** the remaining rows were marked done by the owner, not ticked row by row.
