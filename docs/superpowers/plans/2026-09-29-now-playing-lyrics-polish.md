# OxiTide — Now Playing polish + karaoke lyrics

- **Status:** Approved (owner, 2026-09-29)
- **Date:** 2026-09-29
- **Owner:** V
- **Parents:** `docs/superpowers/specs/2026-09-29-oxitide-track4a-lyrics-design.md` (lyrics surface) ·
  `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (§7.2 Now Playing)
- **Scope note:** polish pass, not a feature spec — pure UI restyle of the expanded
  Now Playing view. No Rust, no IPC, no store/logic changes.

## 1. Goal

Push the expanded Now Playing view toward the Apple-Music karaoke feel (owner
reference screenshot): lyrics get big, bold lines where the active line is fully
bright, upcoming lines dim, already-sung lines dim further, and far-away lines pick
up a slight depth blur. Around that, polish the view's motion: art crossfade on
track change, a gentler exit, art shadows/hover, calmer tab morph, spine-tab
transitions, details spacing.

## 2. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| NP-D1 | **Full karaoke + depth blur:** ~30px, weight 700 lines; active 100% opacity, upcoming 50%, past 30%, lines ≥4 away get `blur(2px)`. | Closest to the Apple Music reference; owner picked it. |
| NP-D2 | **Past dimmer than upcoming.** | Clear sense of progress. |
| NP-D3 | **View-wide polish**, not lyrics-only: entrance/exit, morph, art presentation, details. | Owner choice. |
| NP-D4 | **Plan doc + smoke note** process (no separate spec). | Polish-pass ceremony per AGENTS.md workflow. |
| NP-D5 | **Unsynced lyrics stay plain:** no dim tiers, no blur — a uniform readable list. | Karaoke states are meaningless without timing; spec §6 keeps unsynced as plain scrollable text. |
| NP-D6 | **Active line rests at 35% from top** (was 40%). | Matches the reference placement. |

## 3. Changes

### 3.1 `ui/src/lib/components/LyricsPanel.svelte`

- Typography: `font-size: calc(var(--font-body-size) + 16px)` (30px comfortable /
  29px compact), `font-weight: 700`, `line-height: 1.35`, padding `12px 0`.
  Family follows `--font-ui` (Manrope/Inter per mode — no mode branching, spec L-D9).
- Per-line state classes derived from the existing `active` index:
  `active` → opacity 1 + `color: var(--text)`; upcoming (`i > active`, or synced but
  not started) → opacity 0.5; `past` (`i < active`) → opacity 0.3;
  `far` (`|i − active| ≥ 4`) → `filter: blur(2px)`.
- Transitions on opacity/filter/color over `var(--dur-view)` + `var(--ease-in-out)`;
  tokens already collapse under reduced motion (tokens.css §12).
- Hover on tappable (timed) lines brightens toward 0.85 (click-to-seek affordance).
- Scroll anchor 40% → 35%; `::before`/`::after` spacers adjust to match.
- Untouched: `activeLine` pure helper, click-to-seek, jump-to-current, manual-scroll
  pause, RTL, reduced-motion instant positioning.

### 3.2 `ui/src/views/NowPlaying.svelte`

- Art crossfade on track change: `{#key track.track_id}` inside a grid-stacked
  container; outgoing/incoming crossfade (~240/200ms). DiscArt's spin loop restarts
  on remount — acceptable (new disc anyway).
- Exit (`npOut`): add `translateY(10px)` + `scale(0.99)` to the fade, gated on
  `appearance.reducedMotion`.
- Sleeve art gets `box-shadow: 0 24px 64px rgba(0,0,0,.45)` (parity with the disc's
  existing shadow); both shapes get a `scale(1.015)` hover lift — the art is the
  close button, so it should signal interactivity. Disc rotation lives on the inner
  `.disc`, no transform conflict.
- Entrance: art scale-in softened to 0.95 + slight rise.
- Morph: `--np-glide` 460ms → 520ms; spine tab color/border animates on
  `var(--dur-state)`.
- Lyrics-mode details block: title 31px → 34px, stage gap 20px → 24px.

### 3.3 Docs

- This plan file.
- `docs/superpowers/plans/notes/2026-09-29-now-playing-polish-smoke.md` — human GUI
  checklist (see §5).

## 3.4 Round 2 (owner review of round 1, same day)

Owner asked for: lyrics block shifted left (nothing else moving), active line
vertically centered, word-by-word karaoke fill in the accent color, and ~10%
larger lyrics text. Confirmed interactively before implementation.

| # | Decision | Rationale |
|---|---|---|
| R2-D1 | **Word timings: enhanced-LRC parsing in Rust + UI estimation fallback.** `LyricLine` gains `words: Vec<LyricWord>` (serde default → old caches load); `lineWords()` estimates by word length when a source has no stamps. | Correct timing when providers ship word stamps; fixes enhanced tags rendering as literal text today; estimation covers standard LRC (the common case). |
| R2-D2 | **Cumulative accent trail** — a word flips to `--accent` when sung and stays accent until the line ends. | Classic karaoke read; the line then dims as a whole when it becomes past. |
| R2-D3 | **Discrete word flip** blended over `--dur-micro`, not a gradient wipe. | Cheap, reads well at the interpolated clock; wipe remains a later upgrade if wanted. |
| R2-D4 | **48px left shift via `transform` on the lyrics block only** (`.lyrics-wrap`). | Transform-only: no reflow, no wrapping change, no other element moves (fly transitions live one level up, no conflict). |
| R2-D5 | **True vertical centering:** scroll target centers the active line's box at 50%; spacer pseudo-elements 50%/50%. | Owner asked for centered, not the 35%-from-top reference placement. |
| R2-D6 | **Interpolated clock** in the panel: position events arrive every 500ms (`engine.rs`), so the panel advances a local rAF clock from the last event (`pendingSeek`-aware, freezes on pause, resets on seek). Line highlight, scroll follow, and word fill share it. | Word fill lands on time without engine/IPC cadence changes; same pattern as DiscArt's spin loop. |
| R2-D7 | **~10% larger lyrics text** via `calc((var(--font-body-size) + 16px) * 1.1)` on `.line` only. | Nothing else derives from the value, so no other UI moves. |

Files: `crates/oxitide-app/src/lrc.rs` (+5 parser tests), `ui/src/lib/api.ts`,
`ui/src/lib/lyrics.ts` (+7 tests), `ui/src/lib/components/LyricsPanel.svelte`.
Automated gates all green (cargo test/clippy, svelte-check, vitest); the round-2
items were added to the smoke checklist.

## 3.5 Round 3 (owner review, 2026-09-30)

Owner found the Lyrics-tab switch abrupt: **Next Up emptied too fast and the
incoming lyrics revealed the whole list at once mid-song**. Choreography
change, not a slowdown — `--np-glide` stays 520ms (owner call).

| # | Decision | Rationale |
|---|---|---|
| R3-D1 | Outgoing Meta column (details/hr/list) exits over 300ms `cubicInOut` (was 160–200ms `cubicOut`), travel −16px → −10px (tuned to 420ms in Round 5). | Matches the glide instead of clearing in a blink. |
| R3-D2 | Lyrics slot enters at 80ms delay (was 180ms), 320ms, y:8 (was y:16); its exit 280ms/y:12 (was 200ms/y:24). | Exit and entry overlap mid-glide: one crossfade, no empty beat. |
| R3-D3 | Lyrics lines stagger in on mount by distance from the line the song is on (24ms per line, capped 140ms, `--dur-state` CSS fade via inline `--line-delay` — tightened from `--dur-view` after smoke). | Opening mid-song settles around the current line instead of populating top and bottom at once; also softens track-change remounts. |

Files: `ui/src/views/NowPlaying.svelte`, `ui/src/lib/components/LyricsPanel.svelte`.
Reduced motion unchanged: `dur()`/`appearance.reducedMotion` gate the slot, the
media query kills the line entrance.

## 3.6 Round 4 (owner review, 2026-09-30)

Round 3's entrance still moved: lines re-wrapped as the column resized
mid-glide and the centered line snapped. Root cause: the panel mounted at t=0
while the `.meta` width kept animating for 520ms, so lines laid out against
the starting width and were re-centered only at the next line change.

| # | Decision | Rationale |
|---|---|---|
| R4-D1 | **Mount the panel as the glide settles** (`lyricsSettled`; JS `GLIDE_MS` = 460ms of the 520ms `--np-glide` — the ease-in-out tail is flat, within a few px of final). A flip back within 160ms (cached track change) mounts at once; initial mount with lyrics already shown settles at once; reduced motion settles instantly. | Lines lay out at effectively final width; Round 3's choreography and the composition are untouched. Accepted trade-off: a brief (~0.15s) empty column before the cascade. |
| R4-D2 | `liveMs` seeds from `pendingSeek ?? position_ms` (untracked) instead of 0. | The first centering uses the real active line. |
| R4-D3 | `overflow-anchor: none` on `.lyrics`. | Keeps Chrome scroll anchoring from nudging `scrollTop` when content reflows. |

Files: `ui/src/views/NowPlaying.svelte`, `ui/src/lib/components/LyricsPanel.svelte`.

## 3.7 Round 5 (owner review, 2026-09-30)

Next Up disappeared too fast and jumped upward on the Lyrics press. Two
causes: (a) the incoming `.lyrics-slot` was a flexing (`flex: 1`) child of the
centered `.meta` column, so while the outgoing details/list were still in the
DOM for their out transitions, the slot absorbed the free space and shoved
them from center to the top; (b) the 300ms exit ended well before the lyrics
cascade at 460ms.

| # | Decision | Rationale |
|---|---|---|
| R5-D1 | `.lyrics-slot` becomes `position: absolute; inset: 0` (`.meta` gains `position: relative`); no `flex: 1`. | The slot is out of flow, so the outgoing Next Up holds its centered position through the exit — and the reverse swap mounts its content centered from frame 1 instead of being displaced by the fading slot. |
| R5-D2 | Outgoing exit 300ms → **420ms** `cubicInOut`, y −10 kept, for `.meta-details`, `hr`, `.meta-list`. | Ends just before the lyrics cascade (460ms): calmer settle, no empty-column beat. The 10px drift was never the jump; the layout displacement was. |

Files: `ui/src/views/NowPlaying.svelte`.

## 4. Explicitly untouched

- `activeLine` / `lyrics.ts` / `lyrics.svelte.ts` → `lyrics.test.ts` and
  `lyricsStore.test.ts` pass unmodified.
- Player engine, queue, backdrop, mini player, player bar, `DiscArt.svelte`.
- No new appearance flags; no mode branching; semantic tokens only.

## 5. Verification

1. `npm --prefix ui run check` — clean.
2. `npm --prefix ui test` — existing suites stay green.
3. Human smoke (`npm --prefix ui run app:dev`, real account) per the notes
   checklist: karaoke look on a synced track; past/upcoming dim tiers; far blur
   (GPU cost is the watch item — the knob is a single value); click-to-seek;
   manual-scroll + jump button; unsynced track renders plain; track-change art
   crossfade (disc angle reset is expected); tab morph; entrance/close motion;
   reduced motion; compact density; Instrument mode; narrow window; both cover
   shapes.

## 6. Risks

- `filter: blur()` on text animates once per line change — minor GPU cost; smoke
  validates, single-value knob if it misbehaves.
- Keyed art remount resets the disc rotation angle to 0° on track change —
  visually fine (new cover); noted in smoke.
