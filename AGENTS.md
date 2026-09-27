# AGENTS.md

Unofficial TIDAL desktop client (Tauri 2 + Rust + Svelte 5). Windows-first, personal use. No CI — all checks are manual.

## Commands

Prereqs: Rust stable (`rust-toolchain.toml` pins it, with clippy/rustfmt), Node 20+, and a one-time `npm --prefix ui install` (also installs the Tauri CLI locally).

- Dev run: `npm --prefix ui run app:dev` (starts Vite on port 1420, then the Tauri window).
  - `cargo run -p oxitide-app` is a trap: the debug build loads `devUrl` (http://localhost:1420) and fails with "localhost refused to connect" unless Vite is already running.
- Package installer: `npm --prefix ui run app:build` (NSIS per-user + MSI; artifacts under `target/release/bundle/`). Unsigned; WebView2 downloads at install time.
- Rust: `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`. Single test: `cargo test -p oxitide-tidal <test_name>`.
- UI: `npm --prefix ui run check` (svelte-check); `npm --prefix ui test` (vitest). Single test: `npm --prefix ui test -- src/lib/format.test.ts`.

## Architecture

- `crates/oxitide-tidal` — pure async TIDAL API client (PKCE auth, token refresh, catalog/search/library/home). No Tauri/UI deps. All HTTP goes through `TidalClient::get` / `get_all_pages` in `client.rs`, which does single-flight refresh-on-401; new endpoints should reuse it.
- `crates/oxitide-app` — Tauri shell. New `#[tauri::command]` fns in `src/commands.rs` must also be listed in `generate_handler![...]` in `src/main.rs`, or the UI's `invoke` fails.
- `ui/` — Svelte 5 runes only (`$state`, `$props`, `$effect`); no Svelte 4 stores or `export let`. Routing is a hand-rolled `Route` union in `ui/src/lib/nav.svelte.ts`, switched in `App.svelte`.
- Playback is implemented: `crates/oxitide-audio` holds the engine (queue, decode, cache, WASAPI output); `crates/oxitide-app/src/player.rs` wires it and maps DTOs. Plans 1–9 (playback, favorites, playlists, UX foundations, data interaction) have shipped.
- Styling (Plan 10): design tokens live in `ui/src/styles/` — `tokens.css` (base + semantic token layers, motion, flags), `modes.css` (per-`data-mode` overrides), `fonts.css` (vendored Fraunces/Manrope/Inter). Components consume **semantic tokens only** (`--surface-*`, `--text`, `--accent`, …); never base or `--dyn-*` values directly. Appearance settings flow through `ui/src/lib/appearance.svelte.ts` (singleton `appearance`) → `data-mode`/`data-density` + CSS vars, persisted in the TOML config. Icons are inlined Lucide paths in `lib/components/Icon.svelte` — no unicode glyphs. Covers render through `lib/components/Cover.svelte` + `lib/artwork.ts` (`coverUrl` builds `resources.tidal.com` URLs from the dashed UUID).
- Modes are `atmosphere` (Plan 11) / `instrument` via `data-mode` on the root; components must not branch on mode — read flag booleans from `appearance` instead.

## IPC contract

- `invoke` sends camelCase args; Rust params stay snake_case (`redirect_url` ↔ `redirectUrl`).
- Payload field names are inconsistent and driven by serde attributes (`Track.audioQuality` is renamed, `HomeItem.item_type` is verbatim). Rust DTOs are the source of truth — keep `ui/src/lib/api.ts` interfaces in sync by hand.
- Auth is PKCE with a browser + paste-back redirect (no embedded webview); `auth_restore` silently refreshes on startup.

## Testing / security quirks

- HTTP tests use `wiremock` and must never hit the live TIDAL API.
- Tokens live in Windows Credential Manager via `KeyringStore` (service `oxitide`, user `tidal`); unit tests must use `MemoryStore`. Never log token values (`TokenSet` has a redacting `Debug` impl).
- GUI smoke checks require a human, a real account, and a browser login; record them as checklists in `docs/superpowers/plans/notes/`.

## Workflow

- Feature work is spec'd/planned as dated files in `docs/superpowers/specs/` and `docs/superpowers/plans/`; extend that series instead of inventing new structure. `.superpowers/` is gitignored brainstorm/session state.
- Windows-first: `keyring` is configured with the `windows-native` feature; the README's "Windows and Linux" wording is aspirational.
