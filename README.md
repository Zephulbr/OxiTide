# OxiTide

An unofficial, native TIDAL desktop client with a bit-perfect audio engine. Tauri 2, Rust and Svelte 5.

[![CI](https://github.com/Zephulbr/OxiTide/actions/workflows/ci.yml/badge.svg)](https://github.com/Zephulbr/OxiTide/actions/workflows/ci.yml)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)
![Platforms: Windows, Linux](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-lightgrey.svg)

> [!IMPORTANT]
> **OxiTide is a third-party project. It is not affiliated with, endorsed by, or connected to TIDAL.**
> "TIDAL" is a trademark of its owner. A paid TIDAL subscription is required; OxiTide plays what
> your account is entitled to and nothing more. It is not a downloader, has no offline export, and
> does not circumvent DRM. Use it at your own risk and in line with TIDAL's terms.

<!-- Screenshots: add Atmosphere / Studio / Hi-Fi Rack captures under docs/images/ and link them here. -->

## Contents

- [Features](#features)
- [Install](#install)
- [Getting started](#getting-started)
- [Privacy and security](#privacy-and-security)
- [Platform status](#platform-status)
- [Build from source](#build-from-source)
- [Project layout](#project-layout)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [License](#license)

## Features

**Playback**
- Own Rust audio engine: WASAPI shared and exclusive output (bit-perfect), ASIO on Windows, ALSA on Linux.
- Gapless playback, queue with history, a streaming cache with prefetch, and session restore (queue and position survive a restart).
- Sound processing, all opt-in and off by default: 10-band EQ, ReplayGain, and crossfade. The UI says when a stage means playback is no longer bit-perfect.

**Library and discovery**
- Browse home, search, albums, artists (with credits and album info), playlists, mixes and radio.
- Edit your library: favorite tracks, albums and artists; create, rename and reorder playlists.
- Synced lyrics, listening history and stats, optional explicit / AI-content blocking.

**Interface**
- Three visual modes: **Atmosphere** (default, dynamic cover colour), **Studio** (dense and still, in warm, neutral grey or OLED black) and **Hi-Fi Rack** (a hardware-style skin with needle or spectrum meters). Dark and light themes.
- Mini player, native Windows 11 title bar with Snap Layouts, system tray with close-to-tray, and an accessibility pass (keyboard navigation, focus handling, reduced motion).
- Sleep timer, configurable global hotkeys and in-app keyboard shortcuts.

**Integrations**
- Scrobbling to Last.fm and ListenBrainz (independent toggles, offline retry queue).
- Windows media keys / SMTC, MPRIS on Linux, and Discord Rich Presence.
- **Phone remote**: an opt-in LAN web page (pair by QR code) to control playback, the queue, search and your library from a phone. Off by default.

## Install

Installers are published on the **Releases** page of the [GitHub](https://github.com/Zephulbr/OxiTide/releases)
repository (mirrored on [Codeberg](https://codeberg.org/Zephulbr/OxiTide)).

| Platform | Download | Notes |
|---|---|---|
| Windows 10/11 (x64) | `OxiTide_<version>_x64-setup.exe` (recommended) or the `.msi` | The `.exe` installs per user without admin rights. The `.msi` installs per machine. WebView2 is downloaded at install time if missing. |
| Linux (x86_64) | `.AppImage` or `.deb`; AUR recipe in `packaging/aur/` | See [Platform status](#platform-status). |

**Windows SmartScreen.** The installers are not code-signed yet, so Windows shows *"Windows protected your PC"*
on first run. Choose **More info → Run anyway** after verifying the download.

**Verify downloads.** Each release lists SHA-256 hashes in `SHA256SUMS.txt`:

```powershell
Get-FileHash .\OxiTide_<version>_x64-setup.exe -Algorithm SHA256
```

**Updates.** Windows installs and the Linux AppImage check for a new release shortly after start and
offer it in **Settings → System → Updates**; nothing is downloaded until you click. Updates are verified
against a signing key built into the app. Turn the startup check off in the same place. deb and AUR
installs update through the package manager.

## Getting started

1. Launch OxiTide and choose **Log in with TIDAL**. Your browser opens TIDAL's login page; there is no
   embedded web view.
2. After you sign in, the browser hands control back to the app through a `tidal://` link and OxiTide unlocks
   on its own. If the browser doesn't open the app, use **Browser didn't open the app?**: sign in, then paste
   the address of the page you land on back into OxiTide.
3. Pick an output device and quality under **Settings → Playback**. Exclusive mode and ASIO are for
   bit-perfect output; leave them off if you share the device with other apps.

Keyboard: `Space` play/pause, `←`/`→` seek, `Shift+←`/`Shift+→` previous/next, `↑`/`↓` volume, `M` mute,
`Ctrl+F` or `/` search, `Alt+←`/`Alt+→` back/forward, `Esc` closes the top-most panel. Global hotkeys are
user-defined in **Settings → System**.

## Privacy and security

OxiTide has no telemetry and no accounts of its own. What leaves your machine:

| Destination | When | Control |
|---|---|---|
| TIDAL (login, catalog, streams, images) | Always, to function | n/a |
| GitHub (update manifest) | Shortly after start and on "Check for updates" | Settings → System → Updates |
| Last.fm / ListenBrainz | Only if you connect an account | Settings → Connections |
| Discord (local IPC) | Only if enabled | Settings → Connections |
| LRCLIB (lyrics) | When you open lyrics | n/a |

- **Credentials.** Your session tokens are stored in the OS keyring (Windows Credential Manager, or the
  Secret Service on Linux; without one, the session lasts only until you quit, and the app tells you).
  Tokens are never logged.
- **No secrets in the repository.** The login uses OAuth PKCE, so there is no client secret. The TIDAL
  OAuth *client ID* in the source is a public identifier, not a credential. Last.fm keys are supplied
  by you (settings or `OXITIDE_LASTFM_API_KEY` / `OXITIDE_LASTFM_API_SECRET`).
- **Phone remote.** Off by default. When on, it listens only for devices on your local network
  (private and CGNAT ranges), requires QR pairing, and exposes a fixed allowlist of actions.
- **Cache.** Recently played tracks are cached on disk for fast replay and gapless transitions, up to a
  size you choose (**Settings → Storage**), and evicted automatically. It is a playback buffer, not a
  library or an offline mode; clear it any time or set it to clear on exit.
- **Data location.** Everything lives under `%APPDATA%\OxiTide\` (settings in `config\`, cache in
  `config\cache\`, logs in `logs\`); on Linux, under the XDG config directory.

Found a vulnerability? Please report it privately to the maintainer rather than opening a public issue.

## Platform status

| | Windows | Linux |
|---|---|---|
| Playback and library | Supported, primary target | Supported |
| Output | WASAPI shared/exclusive, ASIO | ALSA |
| Media controls | SMTC and media keys | MPRIS |
| Credential storage | Credential Manager | Secret Service |
| Packaging | NSIS, MSI | AppImage, deb, AUR recipe |
| Verification | Daily use and owner smoke tests | Built and tested under WSL2; real-hardware audio and desktop checks are still pending |

macOS is out of scope. ASIO output is merged but its hardware smoke test is still pending.

## Build from source

Prerequisites: Rust stable (pinned by `rust-toolchain.toml`), Node 22.22+ or 24.15+ (the UI tests need it), and on Linux the system packages in
[`docs/linux-build.md`](docs/linux-build.md).

```bash
npm --prefix ui install        # also installs the Tauri CLI

npm --prefix ui run app:dev    # development: starts Vite, then the app window
npm --prefix ui run app:build  # installers under target/release/bundle/

cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

> `cargo run -p oxitide-app` on its own fails with "localhost refused to connect": the debug build loads
> the Vite dev server, which must already be running. Use `app:dev`.

Continuous integration runs the same checks on Windows and Ubuntu 22.04 for every push and pull request.

## Project layout

| Path | What it is |
|---|---|
| `crates/oxitide-tidal` | Async TIDAL API client: PKCE login, token refresh, catalog, search, library. No UI dependencies. |
| `crates/oxitide-audio` | Playback engine: queue, decoding, cache, DSP, WASAPI / ASIO / ALSA output. |
| `crates/oxitide-app` | Tauri shell: commands, integrations, tray, phone-remote server, self-update. |
| `ui/` | Svelte 5 interface, including the phone-remote page (`ui/remote.html`). |
| `scripts/` | Icon generation, the performance harness, and release tooling. |
| `packaging/` | Linux packaging recipes (AUR). |
| `docs/` | Specs, plans, build and release guides. |

## Documentation

- [`docs/release.md`](docs/release.md): how releases are cut and updates are signed.
- [`docs/linux-build.md`](docs/linux-build.md): Linux build environment and packaging.
- [`docs/superpowers/`](docs/superpowers/): design specs, implementation plans, the roadmap, and smoke-test notes.
- [`AGENTS.md`](AGENTS.md): architecture notes and conventions for contributors and coding agents.
- [`ui/design/DESIGN.md`](ui/design/DESIGN.md): the visual system behind the three modes.

## Contributing

Issues and pull requests are welcome. Before opening a pull request, run the four checks above; they are
what CI runs. Larger features start as a short spec under `docs/superpowers/specs/`. Read `AGENTS.md` for
the conventions (Svelte 5 runes only, semantic design tokens, how Tauri commands are registered).

## Acknowledgements

Third-party attributions (vendored fonts, assets and Rust dependencies) are in
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## License

[AGPL-3.0-or-later](LICENSE).
