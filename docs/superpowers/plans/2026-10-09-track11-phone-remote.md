# Track 11 Phone Remote — Plans 11.1–11.5

**Goal:** Let a phone on the LAN control a running OxiTide through a QR-paired web page
(spec `docs/superpowers/specs/2026-10-09-oxitide-track11-phone-remote-design.md`).

**Status:** 11.1–11.5 implemented on branch `track11-phone-remote` (2026-10-09). 11.6
(human smoke on a real phone, PM-D1 re-runs with the remote enabled, AGENTS.md, completion
note) is open.

**Owner decisions (2026-10-09):** RM-D3…RM-D13 accepted as written; delivery 11.1–11.5 on a
branch; Windows firewall handled by Settings guidance only (no NSIS rule).

## What landed

| Plan | Commit | Content |
|---|---|---|
| 11.1–11.3 | `feat(remote): phone remote server, pairing, RPC and events` | `crates/oxitide-app/src/remote/`: `guard.rs` (LAN admission, Host/Origin, cookie, rate limiter, name sanitising), `pairing.rs` (token, approvals, `remote.json` device store), `rpc.rs` (allowlist + `RemoteCommand` + `execute` over `commands::*`), `events.rs` (hub, slim state, `queueRev`, throttles, SSE stream), `net.rs` (interface pick), `qr.rs`, `server.rs` (axum), `host.rs` (Tauri glue), `mod.rs` (`RemoteSession` seam, `RemoteService`); `[remote]` config; 8 commands; engine subscriber; session hooks |
| 11.2 (UI) | `feat(ui): Settings remote section …` | `lib/components/RemoteSettings.svelte` in Settings → Connections: toggle, address/port/interface, approval switch, pairing dialog with QR + countdown, approval prompt, device list with revoke, firewall/CDN guidance |
| 11.4–11.5 | `feat(remote): phone remote page` | `ui/remote.html` + `ui/src/remote/`: transport (`rpc.ts`), store (`store.svelte.ts`), queue cache, routes, pages (Now Playing + lyrics, Queue, Search, Home/For You/Explore, My Mixes, Library, Favorites, Playlists/folders, History, Album, Artist, Playlist, Mix), row-action sheet |

New crates: `axum` (http1/tokio/json only), `qrcode` (svg only), `sha2`, `getrandom`,
`futures-util`, `if-addrs`, `socket2`; workspace `tokio` gains `net`. `base64` reused from
the workspace.

## Spec refinements (flagged for review)

1. **Logout keeps the server running.** §6.1 lists logout among the stop triggers but also
   says the server runs while logged out. The remote stays up and phones get
   `session: loggedOut` + "Sign in on the PC", which also matches startup-while-logged-out.
2. **`get_artist` is allowlisted** (read-only catalogue) for the artist page header; the
   phone has no other source for the artist's name/picture.
3. **Host check uses the arrival address.** `Host` must be `localhost` (over loopback) or
   the IP literal the connection actually arrived on — by definition this machine's — with
   the bound port. No interface enumeration on the request path.
4. **IPv4 fallback only when IPv6 is unavailable.** On Windows a `0.0.0.0` bind can share a
   port another process holds dual-stack, so `AddrInUse`/`PermissionDenied` move to the next
   port instead.
5. **Assets: exact hits only.** Tauri's resolver falls back to `index.html` for unknown
   paths (and in dev reads a possibly stale `ui/dist`), so the server rejects HTML answers
   for non-HTML paths, checks the remote marker, never serves `index.html`, and in debug
   builds proxies the Vite dev server first.
6. **Approval surfacing:** no tray-notification plugin exists, so a pair request shows and
   focuses the main window when it is hidden/minimised (the user started pairing moments
   earlier); the prompt itself lives in Settings and the pairing dialog.
7. **Token lifecycle under approval:** the pairing token is reserved during the wait,
   burned on approve/deny, and released on timeout (so the same QR can be retried until it
   expires).
8. **Missing cookies are not rate-limited** (only wrong ones), so an unpaired phone loading
   the page cannot lock itself out of pairing.
9. **Playback from the phone uses `play_page_items`** with full metadata so long lists do not
   trigger per-track lookups on the PC; Play/Shuffle on paged lists use the rows loaded so
   far (the page says so).
10. **Main bundle:** the multi-page build moves the shared Svelte runtime and token CSS into
    a shared chunk (one extra `modulepreload` for the shell). Re-check S1/S2 in 11.6.

## Verification (2026-10-09, Windows)

- `cargo test --workspace` — green (remote: 58 tests incl. HTTP integration on an
  ephemeral port: pairing, replay, rate limit, approval deny, cookie, Host/Origin/header
  guards, oversize body, CSP, SSE snapshot → delta → revoke, disable closes stream,
  `not_logged_in`, port fallback, allowlist ↔ `generate_handler!` coverage).
- `cargo clippy --workspace -- -D warnings` — clean. (`--all-targets` shows pre-existing
  lints in other modules' tests, none in `remote/`.)
- `npm --prefix ui run check` — 0 errors; `npm --prefix ui test` — 92 files / 750 tests
  (remote store, transport, helpers, import-isolation guard, Settings section).
- `npm --prefix ui run build` — `dist/remote.html` emitted; remote chunks contain no Tauri
  code.
- Visual check of the phone page at 390×844 in dark and light against a mock backend
  (Now Playing, Queue windowing, Home shelves, Search + row sheet).
- 11.6 (2026-10-10): WSL2 Linux gates, release-build LAN probe, and the PM-D1 corpus
  (enabled + simulated phone vs. a disabled control) are recorded in
  `plans/notes/2026-10-10-track11-completion.md`; the remote shows no measurable cost, and
  seven pre-existing budget breaches reproduce with it disabled.
- **Not yet run:** the human smoke (`plans/notes/2026-10-09-track11-phone-remote-smoke.md`).
