# OxiTide — Track 9 UI Completion (minus Hi-Fi Rack)

- **Status:** Draft for review — design approved live with the owner (2026-10-05); plans pending
- **Date:** 2026-10-05
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 9; PM-D1–PM-D6);
  `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` §4.4, §11 (light-ready
  token contract); `2026-09-25-oxitide-2b-visual-convergence.md` §2, §4 (Rack sequencing);
  `2026-10-03-artist-page-design.md` + `2026-10-03-home-layout-card-language-design.md`
  (Track 9 slices already shipped); `2026-10-04-oxitide-track10-qol-design.md` §4 item 5,
  §6.5 (credits seam, feed limitation)
- **Process note:** scope, decisions, and architecture were brainstormed live with the
  owner on 2026-10-05. This document is the standalone source of truth for Track 9.
- **Scope change (owner, 2026-10-05):** **Hi-Fi Rack is excluded** — it gets its own
  design session and spec (convergence §2). **i18n graduates to its own track** — the
  roadmap explicitly allows this if it outgrows Track 9 (§3 Track 9), and ~600–800
  hardcoded strings across ~55 UI files do. This spec covers the remainder:
  light mode, track credits + album info, the bound polish bundle, and the a11y pass.

---

## 1. Purpose & Outcome

Track 9 closes out the visual/UX program so Track 2b can end: it makes OxiTide
themeable, completes the detail surfaces promised by the roadmap, fixes the known UI
defects from Tracks 2b/3/10, and gives accessibility an explicit, verifiable bar.

**Outcome:**

- A real **light theme** with full dynamic color, switchable per Dark / Light / System,
  with no boot flash and both visual modes (Atmosphere, Instrument) supported (§6.1).
- **Track credits** surface in Now Playing's new Info tab and the track menu, and the
  **album page** shows release facts and correct multi-volume numbering (§6.2).
- The **polish bundle** lands: feed cards gain metadata (instant play, gray-out,
  now-playing highlight, faster Play all), settings writes roll back on failure, the
  hotkey-recorder race is fixed, artist-page leftovers close, and small cosmetics are
  corrected (§6.3).
- The **a11y pass** verifies six acceptance criteria across every custom widget with new
  component-level tests plus human keyboard/NVDA/contrast smokes (§6.4).
- The two Track 9 slices already shipped (artist page, home rhythm) are verified and
  closed by 9.0, and stale UI smoke checklists are triaged so the track can actually
  close (§6.0).

**Success =** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
`npm --prefix ui run check`, and `npm --prefix ui test` stay green on every plan; the
9.0–9.4 smoke checklists pass with a real account; affected Track 3 scenarios re-pass
under PM-D1; Track 9 closes through the completion note in §10 with an explicit
verified/exempted split, handing i18n and Rack off as open tracks.

## 2. Scope

**In scope:**

- 9.0 Carry-in verification: artist-page and home-rhythm smokes/tuning + perf re-check;
  triage of stale UI smoke checklists.
- 9.1 Light mode: `Theme` config, `set_theme`, `data-theme` pipeline, pre-paint mirror,
  `themes.css` inversion (+ raw-color fixes), full light-room palette extraction with
  per-theme cache, Settings control, tests.
- 9.2 Credits + album info: API spike, DTOs/methods/commands, Now Playing Info tab,
  track context-menu entry + modal, album facts and multi-volume grouping.
- 9.3 Polish bundle: `PageItem` feed enrichment (gray-out, hover-play, now-playing
  highlight, Play-all latency), settings toggle rollback, hotkey-recorder race fix,
  artist-page leftovers, small cosmetics.
- 9.4 A11y pass: six acceptance criteria, the audited fix list, minimal component-test
  infra, human keyboard/NVDA/contrast smokes.
- Config, commands, DTOs, tests, smoke checklists, and roadmap/AGENTS.md updates for all
  of the above.

**Out of scope (non-goals):**

- **Hi-Fi Rack** (own design session + spec; only the interface contract matters here:
  `data-mode="rack"` will be a future override block, components never branch on mode).
- **i18n** (own track; §10 records the handoff. New UI in 9.1–9.4 keeps strings
  component-local and untangled so later extraction stays mechanical).
- Undo for destructive operations; lyrics in mini/player bar; word-level lyric timing;
  user-supplied lyrics; offline lyric prefetch (`2026-09-29-oxitide-track4a-lyrics-design.md`
  §12); a For You page; radio seed switching; discovery feedback thumbs
  (`2026-09-30-oxitide-track4b-discovery-design.md` §11); oneko extras
  (`2026-09-29-oxitide-oneko-design.md` §11).
- Scrobble love (Track 10.4, deferred) and any scrobble-service behavior beyond the
  small "scrobbled" indicator in Now Playing.
- The S4 `advance_max_gap_ms` miss: owner-ruled WASAPI-ring behavior in the audio engine
  (`plans/notes/2026-10-02-track3-phase-d.md`), not a UI item; it stays Track 3 debt /
  Track 7 territory.
- DSP/casting/Atmos/ASIO (Track 7), downloads (PM-D2), new remote origins.

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| T9-D1 | **Track 9 continues as one umbrella spec minus Hi-Fi Rack and i18n**; i18n graduates to its own track per the roadmap allowance, Rack to its own session. | Brainstorm 2026-10-05; keeps each track closeable. |
| T9-D2 | **Light mode is orthogonal to mode**: a new `data-theme` attribute (`dark`/`light`), never a third `data-mode`; both Atmosphere and Instrument render in both themes. | `data-mode` selects effect personality and flags; light is a semantic-layer rewrite (foundation §4.1, §4.4). |
| T9-D3 | **Dynamic color works in light mode via full light-room extraction**: `palette.rs` becomes room-parameterized, `analyze_cover` takes the resolved theme, and the palette cache is keyed cover+theme. `Auto` keeps today's semantics (full in Atmosphere, off in Instrument) in both themes. | Preserves the signature feature; the contrast chain is re-derived and tested rather than assumed. |
| T9-D4 | **Theme options are Dark / Light / System**; default `dark` (no surprise flip on upgrade); System resolves via `matchMedia` and follows live OS changes. A pre-paint mirror prevents boot flash. | Cheap with the existing media-query pattern; explicit default keeps current look. |
| T9-D5 | **Credits surface = Now Playing "Info" tab + track context-menu "Track info"** (current track → tab; other rows → modal with the same panel). **Album info** extends the Album page with facts and multi-volume grouping. | Brainstorm 2026-10-05; the spine is the natural home, the menu covers non-playing rows. |
| T9-D6 | **Feed enrichment extends the page DTO** so Home/Explore track cards can gray out blocked tracks, play instantly, highlight the playing row, and skip the per-track metadata fetch on Play all. Flags ride the existing batched, fail-open `get_track_flags` path. | Fixes the Track 10 documented limitation (`track10-qol-design.md` §6.5) and the home spec's Play-all latency risk in one coherent change. |
| T9-D7 | **A11y closes on six explicit acceptance criteria** (keyboard traversal, focus-visible, SR roles/names/states, contrast both themes, ≥24px targets, route-change title/skip link), verified by minimal jsdom + @testing-library component tests plus human smokes. The per-cover composite-ceiling caveat is formally accepted, not hidden. | Roadmap requires explicit criteria "so the track can actually close"; component tests are the only durable evidence for focus/keyboard behavior. |
| T9-D8 | **Execution order 9.0 → 9.1 → 9.2 → 9.3 → 9.4** (features first, a11y sweeps last). 9.0's smokes are owner-run and may finish alongside 9.1; their defects merge before 9.3 is planned. | Light mode first makes later UI theme-aware from birth; a11y last sweeps final widgets once. |
| T9-D9 | **Smoke triage policy**: checklists for surfaces Track 9 touches are re-run; obsolete ones are retired with a one-line closure note; Track 10's pending owner smokes are triaged the same way if they surface UI defects. | The repo's open-checklist debt must not block closure, but touched surfaces must be verified. |
| T9-D10 | **PM-D1 applies to every plan**: each states its performance impact and re-runs the affected Track 3 scenarios under unchanged budgets. | Roadmap standing gate. |

## 4. Current state and research summary

Recon on 2026-10-05 (working tree `948bcb4`, single `main`, no unmerged branches):

1. **Two Track 9 slices already shipped (2026-10-03) under the Track 9 label**, with owner
   smoke pending: artist-page redesign (`2026-10-03-artist-page-design.md`) and home
   rhythm/card language (`2026-10-03-home-layout-card-language-design.md`). The roadmap's
   "Proposed" status is stale; 9.0 closes these out.
2. **Light mode is token-ready but not light-ready.** Semantic tokens are consumed
   everywhere (whole-UI counts: `--text-muted` 86, `--accent` 70, `--line` 59, `--text`
   51, `--surface-*` 77), and only three hex literals exist outside token files. But all
   values are dark recipes, `app.css` hard-codes `color-scheme: dark`, `palette.rs`
   hard-codes the dark room (`BG`, `ROOM_AVG`, `TEXT`, `ON_ACCENT_DARK`, `SCRIM`,
   envelopes), the `@property` initial values are dark, and `PaletteDto` is keyed by
   cover only (`artwork.rs`, `CACHE_VERSION = 5`).
3. **Credits + album info are greenfield.** No credits code anywhere; `Track`/`Album`
   DTOs drop `volumeNumber` display and all album attributes. The public v2 `credits`
   relationship is confirmed to exist (`track10-qol-design.md` §4 item 5); album
   attributes via OpenAPI are unspiked. `TidalClient::track` exists
   (`crates/oxitide-tidal/src/catalog.rs`) but is not exposed as a command.
4. **A11y has a real baseline and concrete gaps.** 216 aria/role/tabindex matches, zero
   `svelte-ignore`, correct Modal focus/restore, correct Settings tab pattern, labelled
   sliders/carousels/toasts. Gaps: no focus trap or `inert` (Modal); class-only active
   items without `aria-activedescendant` (ContextMenu, SortSelect, Explore suggestions);
   Now Playing tabs missing roving/arrow/`aria-controls`/`tabpanel`; SeekBar missing
   `aria-valuetext` and using a 12px edge hit area; TrackTable missing `aria-sort`,
   keyboard row play, virtualized row semantics; QueueRail nested-interactive row;
   pointer-only splitter; label-less filter; shuffle/repeat without `aria-pressed`; no
   `document.title`/skip link; no focus restore for context menu/overlays.
5. **No component-test infra exists**: 59 pure-logic vitest files, zero components, no
   jsdom/testing-library/axe, no `test` block in `vite.config.ts`.
6. **Polish backlog (bound by owner, 2026-10-05):** feed gray-out/hover-play/Play-all
   latency (`track10-qol-design.md` §6.5 limitation; home spec §10–§11); settings
   optimistic toggles without rollback and the hotkey-recorder race
   (`plans/notes/2026-10-05-track10-completion.md` follow-ups); artist-page out-of-scope
   items (`2026-10-03-artist-page-design.md` §2, §7); playlists "Date" sort deviation
   (Plan 13), Purchases page card language (home spec §11), scrobbled indicator
   (`2026-09-29-oxitide-track5-scrobbling-design.md` §13).
7. **S4 is not a UI item** and is excluded (§2). Stale open checklists include Plan
   10/11-era smokes, Track 3 Phase B/C smoke lists, and the two 2026-10-03 Track 9
   slices; 9.0 triages them (T9-D9).

## 5. Architecture

### 5.1 Theme pipeline (`data-theme`)

- **Config:** `Theme { Dark, Light, System }` (serde lowercase, `default = dark`) added to
  `UiConfig` in `crates/oxitide-app/src/config.rs`; `set_theme` command mirrors
  `set_ui_mode` and is registered in `main.rs`; `Config.ui.theme` + `setTheme` land in
  `ui/src/lib/api.ts` by hand (IPC contract).
- **State:** `appearance.svelte.ts` gains `theme` (the setting) and `resolvedTheme`
  (`dark`/`light`): System resolves through
  `matchMedia("(prefers-color-scheme: dark)")` with a change listener (mirroring the
  reduced-motion pattern). `apply()` writes `root.dataset.theme = resolved`.
- **Boot flash:** the resolved **setting** (`dark|light|system`) is mirrored to
  `localStorage["oxitide.theme"]`; a tiny inline pre-paint script in `ui/index.html`
  reads it (absent → `dark`, matching the config default), resolves `system` via
  `matchMedia` synchronously, and sets `data-theme` + `style.colorScheme` before first
  render. `appearance.init()` reconciles from config and rewrites the mirror.
- **CSS layering:** new `ui/src/styles/themes.css` imported after `modes.css`; the
  `[data-theme="light"]` block overrides semantic values (surfaces, lines, text, danger/
  success/warning, `--surface-float`, `--edge-lit`, `--glare-alpha`, `--dyn-*` fallbacks,
  `color-scheme: light`), and compound selectors
  `[data-mode="atmosphere"][data-theme="light"]` / `[data-mode="instrument"][data-theme="light"]`
  carry mode-specific light treatments (light glass veil vs light solids; Grain opacity
  drops per foundation §4.4).
- **Token hygiene:** `--bg`, `--text`, `--text-muted` move into the semantic block (they
  behave semantically already); `--accent-brand`/`--on-accent-brand` become the declared
  `:root` values of `--dyn-primary`/`--dyn-on-primary` (single source for the static
  fallback, themed by the light block); the grain multiplier (`app.css`) and scrollbar
  thumb become tokens; raw-color
  sites fixed: `Backdrop.svelte` room gradient + scrim, `NowPlaying.svelte` glare,
  `Profiles.svelte` edge highlight, `app.css` `color-scheme`. `PerfHud` stays dark by
  design; masks/disc sheen stay.
- **Dynamic color:** `analyze_cover` gains a `theme` argument; `dynamicColor` re-requests
  the palette when `resolvedTheme` changes while dynamic color is on. `Auto` semantics
  unchanged.
- **Settings:** Appearance tab gets a Theme segmented control (Dark/Light/System) with
  helper copy; system-follow updates live.

### 5.2 Palette rooms (`palette.rs`)

- Extract the dark constants into a `Room` value: background, room average, text, ink,
  scrim floor, near-white guard, and the L/C envelopes for primary/secondary/tertiary.
- Add a warm **light room** (warm-paper counterpart to Warm Graphite, not pure white),
  following foundation §4.4's inversion notes: light scrim composite, dark-on-light
  glass tints, lower grain. Exact values are set in the plan with the same OKLCH
  workflow; accents target ≥4.5:1 against light composite ceilings by construction.
- `PaletteDto`/cache key becomes cover+theme; `CACHE_VERSION` is bumped; old caches
  invalidate cleanly. Light-room tests mirror the existing envelope/ceiling/backdrop/
  tertiary tests.
- Contrast caveat (foundation §5.3) is accepted symmetrically: on covers whose composite
  ceiling makes 4.5:1 impossible, vibrancy wins; this is recorded, tested where
  measurable, and stated in the acceptance criteria (AC-4).

### 5.3 Credits and album info backend

- **Spike first (9.2 task 0):** probe public v2 `tracks/{id}` credits relationship and
  `albums/{id}` attributes (label, copyright, UPC, volume/media counts) through the
  existing `get_openapi` seam; capture real payload shapes in a `plans/notes/` research
  note. Automated tests stay wiremock-only. If an attribute is unavailable or
  region-limited, the surface omits it; if credits are unavailable entirely, the Info
  tab ships with available metadata/album facts and credits are deferred as a documented
  follow-up.
- DTOs and methods: `TrackCredits` (role groups → contributors with optional artist ids)
  and `AlbumInfo`; `track_credits(id)` / `album_info(id)` in
  `crates/oxitide-tidal/src/catalog.rs` using the JSON:API parse pattern from
  `flags.rs`. Commands `get_track_credits` / `get_album_info` registered in `main.rs`;
  `api.ts` interfaces kept in sync.
- UI: `InfoPanel.svelte` (credits grouped by role, contributor → artist route when an id
  exists, album link, inline error + retry, clean empty state); Now Playing spine tab
  union extended to `"lyrics" | "next" | "info"`; track menu gains "Track info";
  Album page adds a facts row (release date, label, copyright, UPC) and multi-volume
  grouping/numbering.
- Caches: per-session `Map`s keyed by track/album id; lazy first-open fetch. No disk
  cache and no new origins.

### 5.4 Feed enrichment (`PageItem`)

- Extend the page DTO mapping (`crates/oxitide-tidal/src/pages.rs` + the UI `PageItem`
  type) so track items carry `{ title, artists, duration, explicit, audio_quality? }`
  where the page payload already includes them; items without metadata keep today's
  resolve path (graceful degradation).
- UI feed cards then: play instantly from embedded metadata, show the now-playing
  highlight by id, and gray out blocked tracks once flags land. Flags are batch-fetched
  (≤20/request) for visible track items through the existing `get_track_flags` store,
  asynchronous and fail-open. Play-all uses embedded metadata and skips the per-item
  fetch when available.
- Videos stay inert (PM-D6); no behavior change for non-track items.

### 5.5 Component-test infrastructure (9.4)

- Dev-only additions: `jsdom`, `@testing-library/svelte`, `@testing-library/user-event`.
  Existing pure tests stay on the node environment; component tests opt in per file
  (`// @vitest-environment jsdom`), so `vite.config.ts` needs no global change.
- Tauri coupling is handled by mocking `@tauri-apps/api/core` `invoke` and the
  `lib/api.ts` surface; interaction logic that can be pure (focus-order math, keyboard
  maps) is extracted into `lib/` and tested in the existing style.
- Tests target the T9-D7 criteria: Modal trap/restore, ContextMenu + SortSelect keyboard
  + activedescendant, Now Playing tabs, SeekBar valuetext, plus focus restoration.

## 6. Features

### 6.0 Carry-in verification (Plan 9.0)

- Owner runs the pending artist-page smoke (`plans/notes/2026-10-03-artist-page-smoke.md`)
  and home-rhythm smoke/tuning incl. its perf re-check
  (`plans/notes/2026-10-03-home-rhythm-smoke.md`).
- A triage note classifies every open UI smoke checklist: **re-run** (surfaces Track 9
  touches: home/artist/now-playing/settings/lists), **retire** (obsolete or superseded,
  one-line reason), or **leave** (non-UI). Track 10's pending smokes are included when
  they surface UI defects.
- Defects found are triaged into 9.1–9.4 or recorded as follow-ups with owner sign-off;
  9.2/9.3 scope is only expanded with that sign-off.

### 6.1 Light mode (Plan 9.1)

- Theme pipeline per §5.1: config/command/API, resolution, listener, pre-paint mirror,
  Settings control, live switching without restart.
- Inversion per §5.1–5.2: light semantic overrides for both modes, light-room palette
  extraction, per-theme cache, raw-color fixes, grain/glass adjustments.
- The light aesthetic is the warm-paper counterpart to Warm Graphite: warm off-white
  room, dark warm ink, inverted scrim/edge-lit/glare, lighter grain; visually verified
  against the foundation spec's contrast floors.
- Switching themes mid-playback with dynamic color on re-analyzes the current cover
  once; with dynamic color off, only tokens swap. No audio state is touched.
- Tests: config round-trip + pre-theme backfill (default dark); `resolveTheme`
  (System via mocked `matchMedia`, including change events); `apply()` writes
  `data-theme`; light-room palette cases; token-parity test (every semantic color token
  in `:root` has a light counterpart, modeled on `rowHeight.test.ts`).
- Smoke: both modes × both themes; live Dark→Light→System switches; restart in each
  setting with no flash; contrast and glass/scrim review.

### 6.2 Credits and album info (Plan 9.2)

- Task 0 spike per §5.3, then backend (DTOs/methods/commands), then UI:
  - **Now Playing Info tab** — credits by role; contributor names link to artist routes
    when ids exist; lazy fetch on first open with per-session cache; inline error +
    retry; explicit "no credits available" state. Built with the full ARIA tab pattern
    from birth (shown by 9.4's tests).
  - **Track menu "Track info"** — for the current track, opens Now Playing on the Info
    tab; for any other row, opens a modal (existing `Modal`) hosting the same panel.
  - **Album facts + volumes** — facts row uses `get_album_info` alongside the existing
    `Promise.all`, non-blocking and omitted when absent; multi-volume albums group
    tracks by volume and use disc-scoped numbering (today `volumeNumber` is parsed but
    discarded).
- Perf: no fetches on browse; one album-info call on album open; credit calls only on
  first Info open per track.

### 6.3 Polish bundle (Plan 9.3)

- **Feed enrichment** per §5.4: gray-out, instant hover-play, now-playing highlight,
  faster Play all. Flags ride the existing batched fail-open store.
- **Settings rollback:** `set_close_to_tray`, `set_smtc`, `set_oneko` (and the hotkey
  save if it shares the pattern) revert their control and toast when the invoke fails.
- **Hotkey-recorder race:** fix the capture-release/save re-registration ordering so
  saving during a capture cannot leave registrations stale; regression test.
- **Artist leftovers:** mixes action on the main artist page (from `artist.mixes`,
  already parsed), per-section retry/error rows instead of silent shelf failures, and
  bio links that navigate where the payload exposes structured links or open externally.
- **Small cosmetics:** playlist "Date" sort made honest (use a real added-date if the
  payload carries one, otherwise relabel to reflect service order); Purchases adopts
  `MediaCard`; a "scrobbled" indicator in Now Playing when a connected service has
  scrobbled the current play.

### 6.4 A11y pass (Plan 9.4)

**Acceptance criteria and verification:**

| # | Criterion | Verified by |
|---|---|---|
| AC-1 | Every interactive control is keyboard-reachable and operable; modal traps focus and restores it; menus/lists/tabs have full keyboard navigation. | Component tests (Modal, ContextMenu, SortSelect, tabs) + human keyboard-only script |
| AC-2 | Every control shows the 2px accent `:focus-visible` ring; no overlay/menu closes with focus lost. | Component focus-restore tests + human visual pass |
| AC-3 | Custom widgets expose correct roles/names/states: sliders `aria-valuetext`, toggles `aria-pressed`, tabs `aria-controls`/`tabpanel`, table `aria-sort` + virtualized row semantics, queue rows without nested interactives. | Component tests + human NVDA pass |
| AC-4 | Static tokens pass 4.5:1 and the dynamic palette chain is verified in both rooms (9.1); disabled/overlay text audited. The per-cover composite-ceiling caveat is accepted by owner ruling. | Rust palette tests + token audit + human spot check in both themes |
| AC-5 | Hit targets ≥24px per the foundation spec. | SeekBar edge fix + manual audit |
| AC-6 | Route changes update `document.title` and a skip link/landmark focus path exists. | Pure title-mapping test + human pass |

**Fix list (from the audit):** Modal focus trap + background `inert`; ContextMenu
`aria-activedescendant`/Home/End/Shift+F10/focus restore; SortSelect active option;
Now Playing tabs roving + arrows + panel wiring; SeekBar `aria-valuetext` +
Home/End/PageUp/PageDown + 24px edge target; VolumeSlider value text; TrackTable
`aria-sort`/keyboard row play and actions/virtualized semantics; VirtualList
set-size/position; QueueRail nested-interactive cleanup + row labels; queue splitter
focus/arrows/`aria-valuenow`; TrackFilter label; Explore search combobox pattern;
shuffle/repeat `aria-pressed`; mini queue sheet dialog semantics; focus restoration for
menus/overlays; `document.title` + skip link. TrackTable/VirtualList changes re-run
S5/S6/S7 (PM-D1).

## 7. Testing and verification

- **Rust (`cargo test --workspace`, wiremock only, `MemoryStore` for keyring):** theme
  serde round-trip + backfill; light-room palette envelopes/ceiling/backdrop/tertiary;
  credits/album-info parsing with wiremock fixtures; feed DTO enrichment mapping;
  hotkey re-registration ordering; config defaults for every new field.
- **UI (`npm --prefix ui test`, `npm --prefix ui run check`):** theme resolution/apply/
  mirror; dynamic-color theme re-request; token parity; InfoPanel states; page-card
  gray-out/highlight/latency paths; rollback handling; title mapping; new component
  tests for AC-1/AC-2/AC-3/AC-6.
- **Human GUI smoke checklists** in `plans/notes/` (real account): 9.0 carry-in runs;
  light mode both modes/themes + switches + restart; credits/album surfaces; feed
  blocking/hover-play; keyboard-only traversal, NVDA pass, contrast in both themes.
- Every plan runs TDD with per-task review; scripted gates are the merge bar.

## 8. Performance (PM-D1)

- Light mode adds only the pre-paint script to boot (~0 ms); palette extraction is
  per cover+theme, cached; theme switching re-analyzes one cover. 9.1 extends the S5
  pin to a light-theme variant and re-runs S1/S2/S5; budgets unchanged.
- 9.2 is lazy: no browse-time fetches; album info rides the existing album-open wave;
  credits load only on first Info open.
- 9.3 feed enrichment removes work (no per-item resolve on Play all) and adds one
  batched flags call for visible cards; re-run S5/S6/S7.
- 9.4 TrackTable/VirtualList semantic changes re-run S5/S6/S7; nothing lands on the
  position tick or first paint.

## 9. Risks and open items

- **Light-room contrast on extreme covers** — mitigated by parameterized, tested
  envelopes and explicitly accepted where the composite ceiling prevents 4.5:1 (AC-4).
- **Boot flash** — mitigated by the pre-paint mirror; verified by restart smoke in all
  three settings; residual first-frame risk if the mirror is stale while the OS theme
  changed (init reconciles immediately).
- **Credits/album attributes may be absent or region-limited** — resolved by the 9.2
  spike; graceful omission defined; if credits are unavailable the Info tab still ships
  with metadata/album facts and credits become a documented follow-up.
- **Page payloads may not carry track metadata for every card** — per-item graceful
  fallback to today's resolve path; 9.3 inspects real payloads first.
- **Component-test Tauri mocking** — bounded to 9.4; pure logic is extracted where
  possible; component tests are targeted, not a full suite.
- **Smoke findings expanding scope** — absorbed into 9.3 only with owner sign-off (T9-D9).
- **Track 10 owner smokes may surface UI defects** — triaged into 9.3/9.4 before those
  plans are written.

**Open items resolved before planning:** credits/album-info payload shapes (9.2 task 0),
page-payload metadata availability (9.3 task 0), playlist added-date availability (9.3).

## 10. Plan sequence, closure, and handoffs

Plans execute in order, each with TDD, the full gate suite, and its own smoke note:

1. **9.0 Carry-in verification** — run pending artist-page + home-rhythm smokes/tuning
   (owner), write the smoke-triage note, feed defects forward.
2. **9.1 Light mode** — theme pipeline, `themes.css` inversion, light-room palette
   extraction, Settings control, tests (+ S5 light pin).
3. **9.2 Credits and album info** — spike, DTOs/methods/commands, Info tab, track menu,
   album facts/volumes.
4. **9.3 Polish bundle** — feed enrichment, settings rollback, hotkey race, artist
   leftovers, cosmetics.
5. **9.4 A11y pass** — criteria, fixes, component-test infra, keyboard/NVDA/contrast
   smokes.

Track closure uses the Track 3/Track 8 completion-note pattern: a dated
`plans/notes/` note with the verified/exempted split (Linux GUI items follow the Track 8
exemption pattern), plus AGENTS.md updates for the theme state, new commands/DTOs, feed
enrichment, and the a11y test infrastructure, and a roadmap status refresh.

**Handoffs recorded here:** Hi-Fi Rack (own spec session; `data-mode="rack"` composes
with `data-theme`), i18n (own track; extraction-friendly strings maintained by this
track), Track 3 S4 debt (audio, not UI).
