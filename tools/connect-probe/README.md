# oxitide-connect-probe

Throwaway spike tool for issue #11 (TIDAL Connect). It answers one question:
**does a TIDAL Connect receiver (e.g. a WiiM) accept a session started with
OxiTide's login?** Findings live in
`docs/superpowers/plans/notes/2026-11-23-tidal-connect-sender-spike.md`.

It is a standalone crate (its own `[workspace]` and `Cargo.lock`), outside the
root workspace and its CI gates. Windows only, like the test it was built for.

## What `cast` does

1. Reads OxiTide's saved login from Credential Manager (service `oxitide`, user
   `tidal`). **Read-only**: it never refreshes or writes the login; with less
   than 15 minutes left it stops and asks you to open OxiTide first.
2. Finds the device over mDNS (`_tidalconnect._tcp`), or takes `--device`.
3. Creates a three-track cloud queue on `connectqueue.tidal.com`.
4. Opens `wss://` to the device and verifies its certificate against TIDAL's
   Connect CA (name check off, as TIDAL's own controller does). **On failure it
   sends nothing** and logs the presented chain.
5. `startSession` → `loadCloudQueue` (access token only, no refresh token, as
   TIDAL desktop sends it) → play if needed → `pause` → `play` → `next` →
   `endSession`.
6. Deletes the cloud queue. Ctrl+C, Ctrl+Break and closing the console window
   still send `endSession` first (a receiver otherwise keeps the session), then
   delete the queue. `OXITIDE_PROBE_INTERRUPT_AFTER=<seconds>` fires the same
   interrupt on a timer, for testing that path.

Discovery waits up to 20 s (time for the first-run Windows Firewall prompt)
and finishes 3 s after the first device appears.

The log goes to `connect-probe-<time>.log` in the current folder (next to the
exe when double-clicked). Every line is scrubbed (token, user id, session ids,
JWTs, country code, also inside nested error JSON) and device messages are
logged through an allowlist, so it can be pasted into a public issue. It does
show the device's name and LAN address.

## Building the tester's exe

Build from a clean checkout of the committed branch, so the exe matches the
source exactly, and remap the build paths: panic messages embed dependency
source paths, which otherwise carry the builder's Windows user folder.

```powershell
$env:RUSTFLAGS = "-C target-feature=+crt-static " +
  "--remap-path-prefix=$env:USERPROFILE\.cargo\registry\src=/cargo/src " +
  "--remap-path-prefix=$env:USERPROFILE\.rustup=/rustup " +
  "--remap-path-prefix=$((git rev-parse --show-toplevel) -replace '/', '\')=/oxitide"
cargo build --release --locked   # target\release\oxitide-connect-probe.exe
```

`RUSTFLAGS` replaces the `rustflags` in `.cargo/config.toml`, hence the
repeated `+crt-static`. The static C runtime means the exe needs no Visual C++
redistributable (only Windows system DLLs). It is unsigned:
SmartScreen will warn on first run.

## Commands

```
oxitide-connect-probe                      # = cast, waits for Enter at the end (double-click)
oxitide-connect-probe cast [--device NAME|HOST:PORT] [--tracks ID,ID,ID] [--quality Q]
oxitide-connect-probe discover [--seconds N]
oxitide-connect-probe tls-check HOST:PORT  # certificate only, sends nothing
```

Developer-only: `bench-host [--port 9000]`, `mdns-selftest` (advertises and
finds a dummy service in-process) and `discover --service-type <type>` (checks
that multicast from other hosts arrives, e.g. `_googlecast._tcp.local.`).

## Local bench (no hardware)

TIDAL desktop ships a real Connect receiver, `tidal_connect_application.exe`
(`%LOCALAPPDATA%\TIDAL\app-*\resources\app.asar.unpacked\resources\win\`). It
holds a TIDAL-issued certificate and listens on `:2019` (TLS, controllers) and
`:9000` (plain, its host). TIDAL desktop must be closed (it runs its own copy).

```powershell
$exe = Get-ChildItem "$env:LOCALAPPDATA\TIDAL" -Recurse -Filter tidal_connect_application.exe | Select-Object -First 1 -ExpandProperty FullName
Start-Process $exe -WindowStyle Hidden            # receiver
cargo run -- bench-host                           # stand-in host (another terminal)
cargo run -- cast --device 127.0.0.1:2019         # the probe
```

The binary crashes at the end of some sessions; restart it between runs, and
stop it when done (it keeps the ports otherwise). It is TIDAL's proprietary
software: use it for local testing only, never ship it.
