# Search discoverability — owner smoke

**Status:** Open. Code on `release/1.x` (after `v1.1.0`); for the next 1.1.x release's gate.

- Spec: `docs/superpowers/specs/2026-11-17-oxitide-search-discoverability-design.md`.
- Local gates, 2026-11-17: svelte-check 0 errors / 0 warnings; vitest all green (incl. new
  `SearchField.test.ts`, `homeSearch.test.ts`, `Home.test.ts` cases);
  `node scripts/i18n/check.mjs` 0 errors (210 warnings, unchanged; 4 new messages drafted
  in all 23 languages as `machine`).

## Checks (dev build, a real account)

- [ ] Sidebar: Explore shows the magnifier; hovering it reads "Explore and search (Ctrl+F)";
      the collapsed rail shows the magnifier too.
- [ ] Home: the field sits under the greeting; type a query, Enter → Explore with results;
      Back returns to Home.
- [ ] Home: focus the empty field → recent searches drop down over "Jump back in" (not under
      it); pick one → Explore with its results.
- [ ] `Ctrl+F` and `/` on Home focus Home's field; on any other page they open Explore with
      its field focused.
- [ ] Settings → Appearance → "Search bar on Home" off: Home has no field, `Ctrl+F` on Home
      opens Explore; survives a restart; settings search "search bar" finds the row.
- [ ] Explore unchanged: live search while typing, chips, type chips, recent searches
      (Enter now also closes the recent list).
- [ ] Hi-Fi Rack mode and one contrast theme: Home's field looks right.
- [ ] One translated language (e.g. German): tooltip, setting label and help text fit.
