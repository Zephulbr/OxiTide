# Track 9 - UI completion (completion note)

- **Date:** 2026-10-13
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §10; Hi-Fi Rack spec
  `2026-10-08-oxitide-track9-5-hifi-rack-design.md`
- **Closed by:** owner, 2026-10-13. The owner marked the 9.2, 9.3 and 9.5 smokes done and asked to close
  the track entirely. Windows is the priority; the Linux bare-metal run is deferred (see below).
- **Merged:** everything is on `main` (9.5c merged 2026-10-13 as `2465750`).

## Plans and their smoke notes

| Plan | Scope | Smoke note | State |
|---|---|---|---|
| 9.0 | Carry-in verification and smoke triage | `2026-10-06-track9-carry-in-triage.md` | Done (zero code defects) |
| 9.1 | Light mode (`data-theme`, palette rooms, Dark/Light/System) | `DONE-2026-10-05-track9-1-light-mode-smoke.md` | Passed |
| 9.2 | Track credits, Info tab, album facts, multi-volume albums | `DONE-2026-10-06-track9-2-smoke.md` | Closed 2026-10-13 |
| 9.3 | Polish bundle (feed enrichment, settings rollback, hotkey race, artist leftovers, cosmetics) | `DONE-2026-10-06-track9-3-smoke.md` | Closed 2026-10-13 |
| 9.4 | A11y pass (AC-1 to AC-6, component-test infra) | `DONE-2026-10-07-track9-4-smoke.md` | Passed 2026-10-08, PM-D1 re-runs green |
| 9.5 | Hi-Fi Rack, third UI mode | `DONE-2026-10-08-track9-5-rack-smoke.md` + `2026-10-09-track9-5-hifi-rack-completion.md` | Closed 2026-10-13 |
| 9.5c | Narrow fold, Rack mini player, queue changer, DSEG7 segment font | `2026-10-10-track9-5c-rack-completion.md` | Closed 2026-10-13 |

## Verified

- **Scripted gates (2026-10-13, merged `main`):** `cargo test --workspace` green, `cargo clippy --workspace
  -- -D warnings` clean, `npm --prefix ui run check` 0 errors / 0 warnings, `npm --prefix ui test` 769 passed.
- 9.1 and 9.4 had full owner passes, including the keyboard, NVDA and contrast scripts for 9.4.
- 9.2, 9.3, 9.5 and 9.5c were closed by owner decision on 2026-10-13; their checklists were **not ticked row by
  row**. They have scripted-gate coverage and iterative owner review (Rack went through nine design and seven
  polish rounds), not a recorded line-by-line pass.

## Exempted / accepted

- **Per-cover composite-ceiling contrast caveat** (AC-4, owner ruling): on covers whose composite ceiling makes
  4.5:1 impossible, vibrancy wins.
- Mini queue sheet uses `aria-modal` without `inert` (documented in the 9.4 note).
- **PM-D1 perf re-runs not recorded for 9.3 and 9.5.** 9.3's S5/S6/S7 re-run was BLOCKED on 2026-10-06 (busy
  machine, port 1420 held); the Rack S1/S5 re-runs were never run. The 9.1 `S5-light` `memory_max_mb`
  681 vs 675 adjudication and the S1/S2 quiet re-run also remain open. Evidence so far attributes these to the
  environment (WebView2 process growth, app-owned memory inside the Phase D range). Run on a quiet machine when
  convenient; Track 11's note already lists seven pre-existing PM-D1 breaches to triage or renegotiate.
- Rack is always dark (no light/silver variant), by owner decision.

## Linux bare-metal run (deferred; Windows first)

The owner will run these later, in one batch, on real Linux hardware. None of them gates Track 9.

- **Track 8 resume lists** (exempted at its closure): `2026-10-03-track8-bare-metal-checklist.md`,
  `2026-10-03-track8-packaging-smoke.md`, `2026-10-03-track8-wslg-smoke.md`, `2026-10-03-track8-1-keyring-smoke.md`,
  `2026-10-03-track8-2-alsa-smoke.md`, `2026-10-03-track8-4-linux-desktop-smoke.md`.
- **Track 11:** the "Linux build: server reachable, `ufw` hint shown" item in
  `2026-10-09-track11-phone-remote-smoke.md`.
- **Track 9 surfaces on WebKitGTK** (never run there): light mode and the pre-paint theme mirror, the Info tab and
  album facts, feed gray-out, the a11y focus/keyboard behaviours, and Hi-Fi Rack (shell, fold, mini player,
  changer, DSEG7 font rendering, meters).
- **Linux cargo gates for code merged since Track 8:** `cargo test --workspace` and
  `cargo clippy --workspace -- -D warnings` in WSL2 or on the hardware. New cross-platform code that Linux
  compiles for the first time: `routed_output.rs`, `output::effective_mode`/`asio_device_key`, and the
  engine change (all ASIO files are `cfg(windows)`). The custom title bar is Windows-only (`get_custom_titlebar`
  returns false on Linux, which keeps the system frame) - check that on a real compositor.
- Perf: the Linux harness is exempt (Track 8, T8-D3); `/proc` metrics only.

## Handoffs and open items

- **i18n** is its own track and has no spec yet (about 600-800 hardcoded strings across about 55 UI files).
- **Credits pagination:** the Info tab renders the first 20 credits only (cursor-paginated); follow-up only if
  it proves visible. Credits and playlist dates were probed under `countryCode=GR` only.
- **Explore serves no track rows**, so the 9.3 Explore feed integration is inert until the page changes.
- Track 3 S4 `advance_max_gap_ms` stays audio-engine debt, not UI.
- Not in Track 9 by spec: undo for destructive ops, lyrics in the mini player, word-level lyric timing,
  user-supplied lyrics, offline lyric prefetch, a For You page, radio seed switching, discovery feedback
  thumbs, oneko extras.
