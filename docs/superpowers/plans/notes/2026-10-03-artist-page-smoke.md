# Artist page smoke checklist (2026-10-03)

Human-in-the-loop checks for the Track 9 artist page. Run with
`npm --prefix ui run app:dev` and a real account.

- [ ] **Rich artist (e.g. AC/DC):** hero renders with the palette backdrop,
      blurred portrait, name, `1.1M fans`, Play/Shuffle enabled.
- [ ] **Sparse artist (e.g. The Notwist):** bio renders with Read more; no
      socials row appears; page is still clean with no empty headings.
- [ ] **Profile failure:** block `openapi.tidal.com` (or disconnect the
      network before opening) → hero still renders, no fans/bio/socials,
      Top tracks and shelves still load once the network returns.
- [ ] **Play vs Shuffle vs Radio:** Play queues the 10 top tracks in order and
      starts row 1; Shuffle queues them randomized; Radio replaces the queue
      with the artist radio (existing behavior).
- [ ] **Show all:** Top tracks shows 5 rows; Show all reveals 10; Show less
      collapses; playing from row 2 queues from that track.
- [ ] **Sections:** Albums / Singles & EPs / Appears on each render as a
      carousel and navigate to the album page; missing shelves are absent.
- [ ] **Similar artists:** circular cards; clicking navigates to that artist;
      back returns to the first artist with correct data.
- [ ] **Rapid navigation:** open artist A, immediately click similar artist B;
      B's page must show B's data only (no flashes of A's shelves).
- [ ] **Heart / Share / More:** heart toggles; More → Share copies
      `https://tidal.com/artist/{id}` and toasts; radio action unchanged.
- [ ] **Socials:** social chips open in the system browser; a chip with a
      non-web URL never renders.
- [ ] **Modes / motion:** atmosphere shows the backdrop; instrument shows the
      flat hero; reduced motion removes transitions.
- [ ] **Playing row:** the currently playing top track is highlighted.
- [ ] **Perf regression check:** with the app warm, run
      `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5`
      then `node scripts/perf/check.mjs`; budgets still pass.

## View all / artist catalog (2026-10-03 follow-up)

- [ ] Every shelf (Top tracks, Albums, Singles & EPs, Appears on) shows a
      View all link that opens the catalog on its tab.
- [ ] Tracks tab pages through the full top-tracks list; Play/Shuffle queue
      from there and the loaded-of-total count grows while scrolling.
- [ ] Albums / Singles & EPs / Appears on show every release (not just the
      first 50); album cards open the album page.
- [ ] Playlists tab lists user/editorial playlists; a card opens the playlist
      page and back returns to the same tab.
- [ ] Mixes tab opens the artist mix page; Artist Radio starts radio.
- [ ] Back from the catalog (e.g. after opening an album) returns to the tab
      you left; a second back reaches the artist page.
- [ ] Top tracks on the artist page has no Show all; its list scrolls in place
      (20 fetched).
- [ ] Catalog tabs show a sliding underline that animates between tabs; the
      Tracks tab list reaches the bottom of the window above the player bar.
- [ ] Album/playlist/mix cards on the catalog page are visibly larger than the
      old 150px grid.
