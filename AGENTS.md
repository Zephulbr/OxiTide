# AGENTS.md

Unofficial TIDAL desktop client (Tauri 2 + Rust + Svelte 5), personal use. Windows comes first: Linux ships, but its batched bare-metal checks are deferred by the owner. Track status, sequencing and completion/smoke notes live in `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` and `docs/superpowers/plans/notes/`. CI (GitHub Actions) runs the Rust/UI gates; smoke checks, the perf harness, and releases' final sign-off stay manual.

## Commands

Prereqs: Rust stable (`rust-toolchain.toml` pins it, with clippy/rustfmt), Node `^22.22.2 || ^24.15 || >=26` (jsdom 30's range; Node 20 breaks the UI tests; CI uses 24), and a one-time `npm --prefix ui install` (also installs the Tauri CLI locally).

- Dev run: `npm --prefix ui run app:dev` (starts Vite on port 1420, then the Tauri window).
  - `cargo run -p oxitide-app` is a trap: the debug build loads `devUrl` (http://localhost:1420) and fails with "localhost refused to connect" unless Vite is already running.
- Package installer: `npm --prefix ui run app:build` (NSIS per-user + MSI; artifacts under `target/release/bundle/`). Unsigned; WebView2 downloads at install time.
- Rust: `cargo fmt --all` (CI runs `cargo fmt --all -- --check`, rustfmt defaults, no `rustfmt.toml`); `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`. Single test: `cargo test -p oxitide-tidal <test_name>`.
- UI: `npm --prefix ui run check` (svelte-check); `npm --prefix ui test` (vitest). Single test: `npm --prefix ui test -- src/lib/format.test.ts`.
- Linux (WSL2 Ubuntu 22.04): environment, apt prerequisites, and build/package commands live in `docs/linux-build.md`; run the same `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` gates there — they compile the `cfg(target_os = "linux")` code.

## Topic guides

Read the guide for the area you touch before changing it:

- `docs/agents/audio.md` — `crates/oxitide-audio`: network resilience and offline cache, ASIO, DSP/crossfade/AutoMix, autoplay.
- `docs/agents/app-shell.md` — `crates/oxitide-app`: efficiency rules (queue rev, paused idle, output ring/drain), phone remote, taskbar toolbar, persisted state files (session/history/flags/tray/hotkeys), output picker, Save for Later + collaborators, own-profile edits/uploads, distribution/updater.
- `docs/agents/ui.md` — `ui/`: styling, tokens and modes, Hi-Fi Rack, Home/Explore/search, page data cache, name links, queue dock glide, window chrome, drag and drop, CatJam (beat tracking, output lead).
- `docs/agents/perf.md` — the `scripts/perf/` harness, scenarios and budgets.

## Architecture

- `crates/oxitide-tidal` — pure async TIDAL API client (PKCE auth, token refresh, catalog/search/library/home). No Tauri/UI deps. All HTTP goes through `TidalClient` in `client.rs`: reads through `get` / `get_all_pages` (and the `get_v2` / `get_openapi` bases), writes through its `post_*` / `put_*` / `patch_openapi` / `delete_*` helpers. It does single-flight refresh-on-401 and retries GETs up to 3 times on transport failures and 502–504 (writes retry only when the connection never opened); new endpoints should reuse it. The one unauthenticated call is `put_upload` to a pre-signed artwork link. Build the `reqwest::Client` with `oxitide_tidal::http_client()` (timeouts, 30 s idle-pool retirement, TCP keepalive — idle keep-alive sockets go stale during long tracks).
- `crates/oxitide-audio` holds the engine (queue, decode, cache, output); `crates/oxitide-app/src/player.rs` wires it and maps DTOs.
- `crates/oxitide-app` — Tauri shell. New `#[tauri::command]` fns in `src/commands.rs` must also be listed in `generate_handler![...]` in `src/main.rs`, or the UI's `invoke` fails. A plain `#[tauri::command] pub fn` runs on the main thread; a test fails unless it is listed in `SYNC_COMMANDS` (commands.rs tests) with its reason. Blocking work that does not depend on call order goes in `#[tauri::command(async)]`; transport/queue commands and config writers stay sync (arrival order; settings read-modify-write the whole config). Every new command must also be classified in the phone remote's `ALLOWLIST` or `NOT_EXPOSED` (`remote/rpc.rs`; a test checks it).
- Player teardown: an engine's last reference can drop inside an async command (`auth_restore` rebuilds the player on every page reload). Anything an engine owns must be droppable there: `TidalResolver` shuts its runtime down with `shutdown_background`. `AppState` replaces or clears the player and drops the old one **after** releasing the lock; a panic under it poisons every later command (`plans/notes/2026-10-28-linux-home-lag.md`).
- `ui/` — Svelte 5 runes only (`$state`, `$props`, `$effect`); no Svelte 4 stores or `export let`. Routing is a hand-rolled `Route` union in `ui/src/lib/nav.svelte.ts`, switched in `App.svelte`.
- App icon: SVG sources in `crates/oxitide-app/icons/svg/` (size-specific variants of one mark; `ui/public/favicon.svg` is a copy of `icon-64.svg`). Never hand-edit the PNG/ICO files in `icons/` — run `node scripts/icons/build-icons.mjs` to regenerate them.

## Rules that apply everywhere

Details are in the topic guides; these bite in any task:

- Components consume **semantic tokens only** (`--surface-*`, `--text`, `--accent`, …), never base or `--dyn-*` values, and must not branch on mode (flags live on `appearance`). Icons are inlined Lucide paths — no unicode glyphs. Update `ui/design/DESIGN.md` / `ui/design/<mode>/DESIGN.md` with any visual change.
- Pages that load through `lib/viewCache.ts` must call their load inside `untrack` in the mount effect (else `effect_update_depth_exceeded`), and writes must invalidate what they change.
- Width-driven layout must be CSS (container/media queries), not a JS width count (it breaks the queue dock glide).
- Never use HTML5 DnD; all drag and drop goes through `lib/dragDrop.svelte.ts`.
- New `QueueItem` literals in Rust need `artist_refs` + `album_id`.
- Code that needs the queue on every state reads `queue_rev`; `state` events omit an unchanged queue.
- DSP identity settings must stay a no-op (exclusive `i32` output is sample-exact); a stage that alters samples clears `PlaybackState.bit_perfect` and lists itself in `dsp_stages`.
- Never fetch tempo or beat data, and never send track ids anywhere for it; beats come only from local analysis.
- The version lives only in the workspace `Cargo.toml`; `tauri.conf.json` and `ui/package.json` must not gain one.

## IPC contract

- `invoke` sends camelCase args; Rust params stay snake_case (`redirect_url` ↔ `redirectUrl`).
- Payload field names are inconsistent and driven by serde attributes (`Track.audioQuality` is renamed, `HomeItem.item_type` is verbatim). Rust DTOs are the source of truth — keep `ui/src/lib/api.ts` interfaces in sync by hand.
- The phone remote's RPC mirrors `invoke`: `{method, params}` with camelCase params mapped onto snake_case in `remote/rpc.rs`; errors are `"<code>: <message>"`. Its wire types (`RemoteState`, `RemoteQueuePage`, …) are camelCase and mirrored in `api.ts` like any DTO.
- Auth is PKCE with a browser + paste-back redirect (no embedded webview); `auth_restore` silently refreshes on startup.

## Testing / security quirks

- HTTP tests use `wiremock` and must never hit the live TIDAL API.
- Tokens live in Windows Credential Manager via `KeyringStore` (service `oxitide`, user `tidal`); unit tests must use `MemoryStore`. Never log token values (`TokenSet` has a redacting `Debug` impl). On Linux the backend is the Secret Service (a session-only fallback when none is running); note that keyring v3 silently falls back to its mock store if no platform feature is enabled.
- GUI smoke checks require a human, a real account, and a browser login; record them as checklists in `docs/superpowers/plans/notes/`.
- Component tests are jsdom per file (`// @vitest-environment jsdom`) via `@testing-library/svelte`; pure-logic tests stay on node. Modal focus handling lives in `lib/focusTrap.ts`, `lib/portal.ts` and `lib/dialogs.svelte.ts`; route titles in `lib/titles.ts` (with the skip link in `App.svelte`).

## Workflow

- Feature work is spec'd/planned as dated files in `docs/superpowers/specs/` and `docs/superpowers/plans/`; extend that series instead of inventing new structure. `.superpowers/` is gitignored brainstorm/session state.
- Legal/security surface: root `LICENSE` (AGPL-3.0-or-later) and `THIRD_PARTY_NOTICES.md`; the webview CSP lives in `crates/oxitide-app/tauri.conf.json` (`csp` + `devCsp`) — update the allowlist when adding a remote origin.
