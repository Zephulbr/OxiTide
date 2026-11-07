# Customizable "My Collection" Implementation Plan

> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** let the owner order the sidebar's collection entries and drag a line; entries
below it fold under "Show more" (or the header chevron when all are below).

**Architecture:** UI-only. A persisted layout store, `Sidebar.svelte` rendering from it with
an edit mode, and the Rack's library tray reading the same order.

**Spec:** `docs/superpowers/specs/2026-11-07-oxitide-customizable-collection-design.md`

**Status (2026-11-07):** steps 1–5 done; `npm --prefix ui run check` and `npm --prefix ui test`
green. Step 6 (owner smoke) pending.

---

- [x] **1. Layout store** — `ui/src/lib/collectionLayout.svelte.ts`: keys and labels/icons,
  `normalizeLayout`, `toRows` / `fromRows` / `moveRow`, `createCollectionLayout(storage)`
  (`move`, `setOpen`, `reset`) and the `collectionLayout()` singleton. Tests:
  `collectionLayout.test.ts` (defaults, corrupt/partial saves, line moves, reset).
- [x] **2. Sidebar rendering** — visible entries, "Show more" only with entries on both
  sides, header chevron only when all are below, rail chevron only while something is
  hidden. The open state is saved.
- [x] **3. Edit mode** — header pencil (hover/focus) → Reset + Done; draggable rows and line
  (queue-rail geometry, settle without glide), Alt+↑/↓ with a live announcement, Esc ends a
  drag or the mode; collapsing to the rail ends it. Test: `Sidebar.test.ts` (three cases,
  rail, keyboard move, line drag, reset).
- [x] **4. Rack tray** — `orderedLibraryKeys(order)` in `rack/sources.ts`; `RackShell`
  renders the tray in that order (every key kept). Test in `rackHelpers.test.ts`.
- [x] **5. Docs** — `ui/design/DESIGN.md`, `ui/design/rack/DESIGN.md`, `docs/agents/ui.md`,
  roadmap revision line.
- [ ] **6. Owner smoke (Windows)** — in `npm --prefix ui run app:dev`:
  - default layout matches the old sidebar; "Show more" opens and closes and is remembered
    across a restart;
  - pencil appears on header hover and on Tab; Done/Esc return focus to it;
  - drag an entry across the line, and drag the line to the top and to the bottom (the
    three sketch cases render as in spec §2);
  - Alt+↑/↓ moves the focused row; Reset restores the default;
  - collapse to the rail (Ctrl+B) mid-edit: edit mode ends, rail shows only visible
    entries plus the chevron when something is hidden;
  - Rack mode: the Library tray follows the new order with every key.
