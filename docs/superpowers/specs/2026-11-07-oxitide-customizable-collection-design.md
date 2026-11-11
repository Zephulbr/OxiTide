# OxiTide — Customizable "My Collection"

- **Status:** Implemented 2026-11-07 (UI gates green); owner smoke passed 2026-11-11.
- **Date:** 2026-11-07
- **Owner:** V
- **References:** `ui/design/DESIGN.md` (Sidebar "My Collection") · `docs/agents/ui.md`
  (drag and drop) · plan `plans/2026-11-07-customizable-collection.md`
- **Process note:** the owner sketched three cases (some, all, none below the line) and
  settled four questions on 2026-11-07 (§4).

---

## 1. Purpose

The sidebar's collection split was fixed in code: Playlists, Albums, Tracks and Save for
Later always shown; Mixes & Radio, Profiles and Purchases under "Show more". The owner wants
to choose both the order and what folds away.

## 2. Behaviour

The collection is one ordered list with a line in it. Entries above the line always show;
entries below it fold away.

| Case | Collapsed | Expanded |
| --- | --- | --- |
| Some below | visible entries, then "Show more ⌄" | … "Show less ⌃", then the hidden entries |
| All below | the header "MY COLLECTION" with a chevron | header chevron open, every entry |
| None below | every entry, no toggle | (no toggle) |

- The header is a plain label unless every entry is below the line (CC-D1).
- One saved `open` flag drives both "Show more" and the all-below header chevron.
- **Edit mode:** a pencil in the header (shown on hover/focus, CC-D2) opens it. Every entry
  and the line become draggable rows; the line can be dragged like an entry. Reset restores
  the default; Done or Esc ends it. Alt+↑/↓ moves the focused row and announces the result.
  Entry clicks do not navigate while editing.
- **Rail:** the entries above the line, then a chevron only while something is below. No
  edit mode; collapsing the sidebar ends it.

## 3. Data

`lib/collectionLayout.svelte.ts`: `{ order: CollectionKey[], split: number, open: boolean }`
in localStorage `oxitide.collection-layout` (CC-D4). Keys are route names. The default is
today's split (four above). Loading drops unknown or repeated keys, clamps the split, and
appends any key the save lacks below the line, so a page added later never disappears.

## 4. Decisions

- **CC-D1:** the header is a toggle only when every entry is below the line, as in the
  sketch. The old collapse-all chevron is gone.
- **CC-D2:** the pencil shows on header hover or keyboard focus, like the playlist head's
  mini buttons.
- **CC-D3:** the Hi-Fi Rack's library tray follows the order but keeps every key (the rack has
  no "Show more").
- **CC-D4:** localStorage, like the playlist sort/filter prefs; no Rust, no command. It is
  per WebView profile and is lost if WebView data is cleared.
- **CC-D5:** the reorder is the sidebar's own pointer gesture reusing `lib/queueDrag.ts`
  geometry, like the queue rail's reorder. It moves sidebar rows, so it is not a
  `dragDrop` payload.

## 5. Out of scope

Hiding entries entirely, reordering Home/Explore/Feed/History/Settings, and touch drags
(edit mode is mouse and keyboard; touch presses are ignored like the queue rail's).
