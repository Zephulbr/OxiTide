# AGENTS.md

Unofficial TIDAL desktop client (Tauri 2 + Rust + Svelte 5). Windows-first, personal use; Linux support (Track 8) shipped 2026-10-04 (WSL2-verified; bare-metal checks exempted — `docs/superpowers/plans/notes/2026-10-04-track8-completion.md`). Track 10 (daily-driver QoL: session restore, sleep timer, history/stats, content blocking, tray/hotkeys) shipped 2026-10-05 (owner smoke passed — `docs/superpowers/plans/notes/2026-10-05-track10-completion.md`); scrobble love (10.4) deferred. Track 7 DSP core (ReplayGain, EQ, crossfade) implemented 2026-10-07, owner smoke pending (`docs/superpowers/plans/notes/2026-10-07-track7-completion.md`). No CI — all checks are manual.

## Commands

Prereqs: Rust stable (`rust-toolchain.toml` pins it, with clippy/rustfmt), Node 20+, and a one-time `npm --prefix ui install` (also installs the Tauri CLI locally).

- Dev run: `npm --prefix ui run app:dev` (starts Vite on port 1420, then the Tauri window).
  - `cargo run -p oxitide-app` is a trap: the debug build loads `devUrl` (http://localhost:1420) and fails with "localhost refused to connect" unless Vite is already running.
- Package installer: `npm --prefix ui run app:build` (NSIS per-user + MSI; artifacts under `target/release/bundle/`). Unsigned; WebView2 downloads at install time.
- Rust: `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`. Single test: `cargo test -p oxitide-tidal <test_name>`.
- UI: `npm --prefix ui run check` (svelte-check); `npm --prefix ui test` (vitest). Single test: `npm --prefix ui test -- src/lib/format.test.ts`.
- Linux (WSL2 Ubuntu 22.04): environment, apt prerequisites, and build/package commands live in `docs/linux-build.md`; run the same `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` gates there — they compile the `cfg(target_os = "linux")` code.

## Architecture

- `crates/oxitide-tidal` — pure async TIDAL API client (PKCE auth, token refresh, catalog/search/library/home). No Tauri/UI deps. All HTTP goes through `TidalClient::get` / `get_all_pages` in `client.rs`, which does single-flight refresh-on-401; new endpoints should reuse it. Two non-v1 bases exist: internal `api.tidal.com/v2` (`get_v2`, used for writes/feed/folders) and public `openapi.tidal.com/v2` (`get_openapi`, JSON:API; playlist follower counts live only there).
- `crates/oxitide-app` — Tauri shell. New `#[tauri::command]` fns in `src/commands.rs` must also be listed in `generate_handler![...]` in `src/main.rs`, or the UI's `invoke` fails.
- `ui/` — Svelte 5 runes only (`$state`, `$props`, `$effect`); no Svelte 4 stores or `export let`. Routing is a hand-rolled `Route` union in `ui/src/lib/nav.svelte.ts`, switched in `App.svelte`.
- Playback is implemented: `crates/oxitide-audio` holds the engine (queue, decode, cache, WASAPI output); `crates/oxitide-app/src/player.rs` wires it and maps DTOs. Plans 1–14 (playback, favorites, playlists, UX/data interaction, visual system, mini-player, integrations) and post-MVP Tracks 1–6 have shipped; Track 3 (performance) is complete (Phases A–D, 2026-10-02); Track 8 (Linux) shipped 2026-10-04; Track 10 (QoL) merged 2026-10-05. Sequencing and future tracks live in `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md`.
- Styling (Plan 10): design tokens live in `ui/src/styles/` — `tokens.css` (base + semantic token layers, motion, flags), `modes.css` (per-`data-mode` overrides), `fonts.css` (vendored Fraunces/Manrope/Inter). Components consume **semantic tokens only** (`--surface-*`, `--text`, `--accent`, …); never base or `--dyn-*` values directly. Appearance settings flow through `ui/src/lib/appearance.svelte.ts` (singleton `appearance`) → `data-mode`/`data-density` + CSS vars, persisted in the TOML config. Icons are inlined Lucide paths in `lib/components/Icon.svelte` — no unicode glyphs. Covers render through `lib/components/Cover.svelte` + `lib/artwork.ts` (`coverUrl` builds `resources.tidal.com` URLs from the dashed UUID).
- Modes are `atmosphere` (default) / `instrument` via `data-mode` on the root. Dynamic color flows `analyze_cover` (Rust, `artwork.rs` + pure `palette.rs`, cached in `cache/palettes.json`) → `--dyn-*` `@property` tokens. JS flag booleans live on `appearance` (`discSpin`, `waveSeek`, …) — components must not branch on mode. Hi-Fi Rack is deferred to Track 9 (convergence doc §2).
- Sound processing (Track 7): `crates/oxitide-audio/src/dsp.rs` (pure: `DspSettings`, RBJ `f64` biquads, ReplayGain math, crossfade `Tail`) runs in the playback worker between the producer and `output.write`. Identity settings must stay a no-op so exclusive `i32` output remains sample-exact; any stage that alters samples clears `PlaybackState.bit_perfect` and lists itself in `dsp_stages` (never in the output notice). Settings apply live via `Engine::set_dsp` (version counter, no restart), persist as `[dsp]`, and reach the engine through `set_dsp_settings`. Crossfade holds back a FIFO of decoded audio and mixes it into the prefetched next stream on natural ends only.
- Track 10 state (all under `%APPDATA%\OxiTide\config\`): `session.rs` (queue/position/shuffle/repeat restore, `session.json`), `history.rs` (`history.jsonl` + History/stats), `track_flags.rs` (content-blocking flags, `cache/track_flags.json`), `tray.rs` (tray menu/tooltip/close-to-tray, `ui.close_to_tray`), and `hotkeys.rs` (user-defined global accelerators via `tauri-plugin-global-shortcut`, `[hotkeys]`, empty defaults; once a chord is registered the global handler is the only delivery path).

## IPC contract

- `invoke` sends camelCase args; Rust params stay snake_case (`redirect_url` ↔ `redirectUrl`).
- Payload field names are inconsistent and driven by serde attributes (`Track.audioQuality` is renamed, `HomeItem.item_type` is verbatim). Rust DTOs are the source of truth — keep `ui/src/lib/api.ts` interfaces in sync by hand.
- Auth is PKCE with a browser + paste-back redirect (no embedded webview); `auth_restore` silently refreshes on startup.

## Testing / security quirks

- HTTP tests use `wiremock` and must never hit the live TIDAL API.
- Tokens live in Windows Credential Manager via `KeyringStore` (service `oxitide`, user `tidal`); unit tests must use `MemoryStore`. Never log token values (`TokenSet` has a redacting `Debug` impl). Track 8 adds a Linux backend; note that keyring v3 silently falls back to its mock store if no platform feature is enabled.
- GUI smoke checks require a human, a real account, and a browser login; record them as checklists in `docs/superpowers/plans/notes/`.

## Performance harness

Track 3's measurement gate lives in `scripts/perf/` (manual; no CI):

- Windows-only exemption (Track 8, T8-D3): the scripted runner/driver (`run-scenario.ps1`, `drive.mjs`) speaks the WebView2 CDP endpoint and is not ported to WebKitGTK. On Linux the `/proc` metrics in `crates/oxitide-app/src/perf.rs` keep process-relative marks, memory, and playback rows measurable via `OXITIDE_PERF_SCENARIO` + `node scripts/perf/check.mjs`; frame/interaction budgets stay unmeasured until the deferred harness port. Authority: `docs/superpowers/plans/notes/2026-10-03-track8-perf-exemption.md`.
- `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario <S1..S10> [-Runs n] [-Manual] [-SkipCheck] [-Minutes n]` — launches the debug exe (Vite must be warm; the script starts it if port 1420 is free) and drives the scenario over the WebView2 CDP endpoint. Raw JSONL lands in the gitignored `.superpowers/perf/runs/`.
- `node scripts/perf/check.mjs` (or `npm --prefix ui run perf:check`) aggregates the corpus against `scripts/perf/budgets.json` (mirrors spec §4.2) and exits non-zero on any breach.
- Scenarios: S1 cold start, S2 warm start, S3-miss/S3-hit TTFS, S4 gapless advance, S5-instrument/S5-atmosphere scroll, S6 10k filter/sort/album, S7 10k queue build, S8 cache loop, S9 2 h soak, S10 manual stability. S4/S8/S9/S10 are human-in-the-loop (real account/audio); the rest are scripted.
- Conditions matter: single app, foreground, Vite warm, no deliberate background load. The current budgets and the S4 documented finding are recorded in `docs/superpowers/plans/notes/2026-10-02-track3-phase-d.md`.

## Workflow

- Feature work is spec'd/planned as dated files in `docs/superpowers/specs/` and `docs/superpowers/plans/`; extend that series instead of inventing new structure. `.superpowers/` is gitignored brainstorm/session state.
- Legal/security surface: root `LICENSE` (AGPL-3.0-or-later) and `THIRD_PARTY_NOTICES.md`; the webview CSP lives in `crates/oxitide-app/tauri.conf.json` (`csp` + `devCsp`) — update the allowlist when adding a remote origin.
- Windows-first: `keyring` uses `windows-native` on Windows; non-Windows builds use output/media/scheme stubs until Track 8 (Linux support).
