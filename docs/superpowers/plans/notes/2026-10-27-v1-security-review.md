# v1.0 security, privacy and bug review

- **Date:** 2026-10-27
- **Branch:** `wasapi-0.26` (the review fixes are separate commits, cherry-pickable to `main`
  on their own: `59a0ce6`, `806cdb7`, `f6074d1`, `066a19a`; the wasapi bump is `735e550`)
- **Gate:** `2026-10-17-v1-release-gate.md` §3b
- **Gates after the changes:** `cargo clippy --workspace -- -D warnings` clean; `cargo test
  --workspace` 1153 passed; `npm --prefix ui run check` 0 errors; `npm --prefix ui test` 1245
  passed; `node --test scripts/release/release.test.mjs` passed. The two `#[ignore]`d WASAPI
  device tests pass on wasapi 0.26 (shared mode, default device).

## Fixed

- **Phone remote: string ids could steer the TIDAL API path** (`remote/rpc.rs`, `59a0ce6`).
  Playlist ids are spliced into paths (`playlists/{id}`, `…/items`), so a paired phone could send
  `../`, `?` or `#` and reach TIDAL endpoints off the RPC allowlist with the user's token. Ids
  from the phone (`get_playlist`, `get_playlist_tracks_page`, `add_playlist_tracks`, `get_mix`,
  `get_collection_items`) must be 1–64 ASCII letters, digits, `-` or `_`. Desktop commands are
  unchanged (the webview is trusted; its ids come from TIDAL).
- **Autoplay stalled after a pause during a fill** (`autoplay.rs`, `806cdb7`). A fill that found
  the user had moved on returned with `attempted` still set, so a resume on the same track and
  queue revision never filled and the drained batch was lost. The batch goes back to the pool and
  the attempt is forgotten.
- **Non-finite volume** (`queue.rs`, `f32::is_finite`, `f6074d1`). Defence in depth only: JSON
  cannot carry NaN and MPRIS already filters it; `clamp` would have let one through silently.
- **Dependency advisories** (lockfile only, `066a19a`): rustls 0.23.46 (RUSTSEC-2026-0346..0349),
  rustls-webpki 0.103.16 (-0382), webpki-roots / webpki-root-certs 1.0.11 (-0381, -0383), tokio
  1.54.1 (-0386, unsound `AsyncFd`), yoke-derive 0.8.4 (0.8.3 was yanked).
- **wasapi 0.24 → 0.26** (`735e550`, RUSTSEC-2026-0332, `WaveFormat::parse` out-of-bounds read).
  No code change. Source diff of the API OxiTide uses:
  - `is_supported_exclusive_with_quirks` no longer retries a 24-in-32 request as a plain
    `WAVEFORMATEX` (0.24 built a 32-bit header for it, 0.26's `to_waveformatex` refuses the
    ambiguous format). `exclusive_formats` always ends with the 32/32 candidate, which still gets
    that retry, so a DAC that only takes the plain header opens one candidate later with the same
    left-aligned samples. Watch the log for which candidate opens if a DAC misbehaves.
  - `write_to_device` (slice) is byte-identical; the render-client change is in the deque variant
    (unused). Removed functions are deprecated ones OxiTide never called; the rest is new API and
    import formatting.

## Smoke: wasapi 0.26 (owner, before tagging)

Run with `npm --prefix ui run app:dev` and a real account; the DAC used for the ASIO smoke is ideal.

- [ ] Shared mode: play, seek, pause past 30 s (`PAUSE_RELEASE`) and resume; volume changes apply.
- [ ] Exclusive, 16/44.1 track: opens, Exclusive + bit-perfect shown (Rack EXCL).
- [ ] Exclusive, 24/96 and 24/192 tracks (Max quality): reopen at the new rate, correct pitch,
      bit-perfect.
- [ ] Exclusive gapless album (same format): no reopen between tracks, no gap.
- [ ] Exclusive seek: no stale audio, no click/re-lock.
- [ ] Exclusive volume below 100 %: endpoint volume used (no software-volume notice on a DAC
      with hardware volume); back to 100 %.
- [ ] Exclusive fallback: select a device that refuses exclusive (e.g. Realtek with exclusive
      disabled in Windows Sound settings) with fallback = shared → plays with the notice; with
      fallback = stop → stops with the reason.
- [ ] Device switch while playing (WASAPI → WASAPI, WASAPI ↔ ASIO); unplug the DAC while playing →
      pause with a notice, replug + Play resumes.
- [ ] Change the Windows default device while playing on "System default" → follows to the new
      device.

## Accepted advisories (open at 1.0)

`cargo audit` (2026-10-27, after the updates): 3 vulnerabilities, 8 warnings, all transitive.

- **zip 4.6.1** — RUSTSEC-2026-0367 / -0368 / -0369 (setuid bits kept, symlinks escaping the
  extraction directory, permissions applied through symlinks). Only path: `tauri-plugin-updater`
  2.13.1, the latest release (no newer plugin to take). Not reachable in practice:
  - every update archive is verified against the minisign public key in `tauri.conf.json` before
    it is opened, so a malicious archive needs the release signing key;
  - on Windows the updater runs the NSIS/MSI installer directly; the Linux AppImage is a file
    swap. All three advisories are about Unix extraction semantics.
  - **Action:** take a `tauri-plugin-updater` release that moves to zip ≥ 9 with a plain `cargo
    update` when one ships. If `cargo audit` is ever added to CI, ignore these three IDs in
    `.cargo/audit.toml` with a pointer to this note.
- **Linux desktop stack (warnings, not vulnerabilities):** `glib` 0.18.5 unsound
  `VariantStrIter` (RUSTSEC-2024-0429) and unmaintained `proc-macro-error` (RUSTSEC-2024-0370)
  come from Tauri's GTK 0.18 bindings (`tauri` → `muda`/`tray-icon` → `gtk`); unmaintained
  `zvariant` / `zbus_names` / `zvariant_derive` (RUSTSEC-2026-0370..0372) from `zbus` (MPRIS via
  `mpris-server`, `tauri-plugin-opener`, `tauri-plugin-single-instance`). They move when Tauri and
  zbus do; nothing to change here.
- **npm dev dependencies:** `npm audit` reports advisories in test tooling only (vitest's
  `tinypool`, `undici`); `npm audit --omit=dev` is clean, so nothing ships. Revisit with the next
  vitest major.

## Reviewed, no change needed

- **Phone remote:** LAN/CGNAT peers only at accept; `Host` must be the arrival IP or loopback
  `localhost` (DNS rebinding); `Origin` must match; POSTs need JSON + `X-Oxi-Remote: 1`; 64 KB
  bodies; pairing token 128-bit, single-use, 5 min, constant-time compare, in the QR fragment (never
  sent to the server in the URL); device tokens 256-bit, only SHA-256 stored; failure rate limit
  per IP and global; every `generate_handler!` command classified; phone covers limited to TIDAL
  images; `analyze_cover` takes only a cover UUID; page limits and item caps; static assets refuse
  traversal, dotfiles and Vite `@fs`.
- **Login:** PKCE; the deep-link path requires `tidal://login/auth` and a matching `state`; a
  rejected redirect restores the pending login.
- **Secrets:** TIDAL tokens and scrobble credentials in the OS keyring (memory-only fallback);
  redacting `Debug` impls; `get_config` blanks the Last.fm secret; stream errors use
  `without_url()`; `remote.json` holds hashes only; Copy diagnostics strips query strings,
  token/code/state values, bearer tokens and the home directory.
- **Webview:** strict CSP (no `unsafe-eval`/inline script in release), `opener` URL allowlist,
  `open_external` http(s) only; `{@html}` only for static icons and the Rust-generated QR SVG.
- **Engine inputs:** seek past the end ignored; queue index/reorder/insert bounds-checked; play
  start clamped; fade duration safe.
- **CI/release:** `contents: read` by default, `write` only in `publish`; no `${{ }}` expression
  injection in `run:`; signing key only in the build step; releases are drafts.
- **Privacy:** every destination is in `docs/privacy.md` (LRCLIB lyrics fallback, Discord IPC,
  Last.fm, ListenBrainz, GitHub update check).

## Known limits for the 1.0 release notes

- The phone remote is plain HTTP on the LAN: another device on the same network could capture a
  paired phone's cookie and get the remote's (allowlisted) control — playback, queue, favorites,
  adding to playlists. Off by default; revoke devices in Settings.
- Debug builds proxy the phone page's asset requests to the Vite dev server, so a LAN peer could
  read project source files while a dev build has the remote on. Release builds serve embedded
  assets only.
- The Windows build is unsigned (decided in the release gate).

## Follow-ups (not blocking)

- Optional hardening: pin third-party GitHub Actions (`dtolnay/rust-toolchain`,
  `Swatinem/rust-cache`) to commit SHAs; the release build job sees the signing key.
- Optional: add `cargo audit` to CI (with the zip ignores above).
- `cargo clippy --all-targets` reports 21 lints in test code (`len() == 0`, `field_reassign_with_default`,
  …); the gate is lib/bin only, so these are style debt.
- Autoplay: a panic inside a fill task would leave `in_flight` set for the session (no autoplay
  until restart). Unlikely; a drop guard would close it.
