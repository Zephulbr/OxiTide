# OxiTide — Search discoverability (2026-11-17)

- **Status:** Implemented on `release/1.x` for the next 1.1.x release; owner check pending
  (`plans/notes/2026-11-17-search-discoverability-smoke.md`).
- **Owner:** V
- **Scope:** 1.x desktop app: the sidebar's Explore item, Home, Explore's search field and
  one Appearance setting. The phone remote (its own Search page) is out of scope. Local
  library search is V2 (`2026-10-17-oxitide-v2-modular-sources-design.md`).

## 1. Problem

Feedback: search is hard to find. It lives only on Explore, and the sidebar shows Explore
as a globe, so nothing says "search". `Ctrl+F` and `/` already open Explore with the field
focused, but no visible text mentions them.

## 2. Decisions

| # | Decision |
|---|---|
| SD-D1 | The sidebar's Explore item keeps its name and gets the `search` (magnifier) icon instead of `globe`. Its tooltip names the shortcut: "Explore and search (Ctrl+F)" (`nav.exploreHint`); the accessible name stays "Explore". No second sidebar item: two rows opening one page was rejected by the owner. |
| SD-D2 | Home gets a search field under its greeting. It is a launcher: Enter (or a recent search picked from its dropdown) records the query and opens `{ name: "explore", q }`, so the results, the type chips and Back behave as for a search typed on Explore. Typing alone does not navigate or search; Home never shows results. A blank Enter does nothing. |
| SD-D3 | One component, `lib/components/SearchField.svelte`, draws the field and its recent-searches dropdown (combobox, arrow keys, Escape, clear) for both pages. It takes the value, an input callback and a search callback; Explore passes its live search, Home its launcher. Explore's look and search behaviour are unchanged; one small difference: Enter now also closes the recent-searches list. |
| SD-D4 | The field is on by default and can be hidden: Settings → Appearance → Typography & layout → "Search bar on Home" (`home-search`, in the settings search index). The choice is UI-only, so it lives in localStorage (`oxitide.home-search`, `lib/homeSearch.svelte.ts`), like the track sort memory. |
| SD-D5 | `Ctrl+F` and `/` focus Home's field when Home shows it; elsewhere they open Explore and focus its field, as before. Both fields keep the `search-input` id (only one page is mounted at a time), so `focusSearch` finds either. |
| SD-D6 | Home's field is narrower than Explore's hero (40px tall, 560px max, left-aligned with the greeting) so the greeting stays the page's lead; it uses the same tokens, so every mode (Hi-Fi Rack included) styles it through its existing `.search-input` rules. |

## 3. Out of scope

- A search field in the window chrome or sidebar on every page (revisit for V2 with the
  per-source tabs, where the search scope also has to be decided).
- Live results on Home.
