# Search suggestions — manual smoke checklist

GUI smoke checks for the custom recent-searches dropdown that replaces
WebView2's native "Saved info" autofill flyout (2026-09-29). Requires a human,
a real account, and a browser login. Run with `npm --prefix ui run app:dev`.

## Native popup removed

- [ ] Focus the Explore search field and type/Enter a query: no Edge "Saved
      info" flyout with its own X ever appears above or below the input.
- [ ] Queries typed before this change (the old saved info) do not resurface.
- [ ] Right-clicking the input shows no browser autofill suggestion rows.

## Dropdown behavior

- [ ] Run a search with Enter, then refocus the field: our dropdown appears
      under the input with a muted "Recent searches" header and an X.
- [ ] With the field empty the dropdown lists recents, newest first, max 8.
- [ ] Typing filters the list case-insensitively on substrings ("blu" shows
      "Blur" and "Blurred lines"); no matches hides the dropdown.
- [ ] Clicking an entry fills the input, runs the search immediately, and the
      dropdown dismisses without the input losing focus.
- [ ] Debounced title searches alone (typing without Enter) do not add history;
      only Enter and suggestion clicks record.
- [ ] Repeat a query: it moves to the front, no duplicate row, newest casing
      wins.
- [ ] Run more than 8 distinct queries: the oldest falls off the list.
- [ ] The header X clears the list immediately; it stays cleared after an app
      restart.

## Keyboard and focus

- [ ] ArrowDown/ArrowUp move a highlighted row (wrapping at both ends) and
      open the dropdown if it was dismissed.
- [ ] Hovering a suggestion moves the highlight to that row; Enter then runs
      the hovered suggestion, not the typed query. Moving the pointer off the
      row (onto the header or away) clears the highlight — no stuck last row.
- [ ] Enter runs the highlighted suggestion; Enter with nothing highlighted
      runs the typed query as before.
- [ ] Escape dismisses the dropdown but keeps the query and results.
- [ ] Clicking elsewhere blurs and closes the dropdown; no row click is eaten
      by the blur (no first-click-misses).
- [ ] Switching away from Explore and back keeps the history (and the field
      resets, as before).

## Design match

- [ ] Panel fill, border, radius, glass blur, and shadow match the track
      context menu (right-click a track to compare side by side).
- [ ] Context menu: hovering rows moves the highlight there (no longer stuck
      on the first row); moving the pointer off a row — including onto a
      disabled row or out of the menu — clears it, and arrow keys continue
      from the hovered row.
- [ ] Both stay dark over a bright cover: play a track with bright artwork,
      open the dropdown and the context menu — neither turns light beige, and
      both carry a faint accent tint that follows the cover.
- [ ] Row hover/highlight matches our menu button highlight; search glyphs are
      muted, text ellipsizes instead of overflowing on long queries.
- [ ] Looks correct in both Atmosphere and Instrument modes and both cover
      shapes; no unicode glyphs anywhere in the panel.
- [ ] Persists across a full app restart (quit and relaunch, not just focus
      changes).
