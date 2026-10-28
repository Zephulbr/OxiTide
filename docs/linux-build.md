# Linux build environment — WSL2 Ubuntu 22.04

Track 8's pinned Linux build base is **WSL2 Ubuntu 22.04 LTS** (the WebKitGTK
4.1 floor and AppImage's oldest-supported-base guidance; Track 8 spec §4.7).
Builds must run on Linux: Tauri does not cross-compile Windows → Linux.
Bare-metal audio/desktop smoke is deferred (spec §5.3), but every build and
cargo gate can be run here.

## 1. Install WSL2 + Ubuntu 22.04 (from Windows)

In an elevated Windows PowerShell:

```powershell
wsl --install -d Ubuntu-22.04
```

Reboot if prompted, create the Linux user, and confirm the distro:

```powershell
wsl -d Ubuntu-22.04 -- uname -a
```

All later commands run **inside the Ubuntu 22.04 shell**, from the repository
root.

## 2. System packages (Tauri v2 + audio + keyring)

```bash
sudo apt update
sudo apt install -y \
  build-essential curl wget file pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libasound2-plugins libdbus-1-dev \
  patchelf
```

- `build-essential`, `curl`, `wget`, `file`, `pkg-config`: general build tools.
- `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libxdo-dev`, `libssl-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`: Tauri v2 prerequisites for
  Ubuntu 22.04 (webkit2gtk 4.1 is the floor).
- `libasound2-dev`, `libasound2-plugins`: ALSA headers and the PulseAudio
  plugin that routes shared mode to WSLg's audio bridge (plan 8.2).
- `libdbus-1-dev`: keyring's `sync-secret-service` backend links libdbus.
- `patchelf`: AppImage bundling (plan 8.3).

## 3. Rust (per `rust-toolchain.toml`)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup show
```

`rust-toolchain.toml` pins `stable` with `clippy` + `rustfmt`; `rustup show`
run from the repo root installs and selects that toolchain.

## 4. Node 24

The UI tests need Node `^22.22.2 || ^24.15 || >=26` (jsdom 30's engines range); Node 20
fails with `webidl.util.markAsUncloneable is not a function`. CI uses Node 24.

```bash
curl -fsSL https://deb.nodesource.com/setup_24.x | sudo -E bash -
sudo apt install -y nodejs
node --version   # v24.x (>= 24.15)
```

Install the UI dependencies **from inside WSL** (rollup/esbuild native binaries
are platform-specific; a `node_modules` populated on Windows will not work):

```bash
rm -rf ui/node_modules
npm --prefix ui install
```

If the checkout lives on the Windows filesystem (`/mnt/c/...`), prefer a
Linux-side clone or worktree for speed and to avoid file-watching issues.

## 5. Build and test gates

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo build -p oxitide-app
npm --prefix ui run check
npm --prefix ui test
```

Run the app under WSLg:

```bash
npm --prefix ui run app:dev
```

## 6. WSLg notes

- WSLg supplies the Wayland/X11 display and a PulseAudio server to the distro;
  GUI windows and shared-mode audio work without extra configuration.
- Blank/black WebKitGTK window in a VM? Retry with
  `WEBKIT_DISABLE_COMPOSITING_MODE=1 npm --prefix ui run app:dev` (software
  compositing). This is a WSLg/VM rendering workaround, not a product setting.
- Exclusive/bit-perfect output and hardware mixers cannot be verified here; they
  are on the deferred bare-metal checklist (spec §5.3).

## Profiling WebKitGTK

WebKitGTK does not speak the Chromium DevTools protocol, so the perf harness's
CDP driver does not apply. To inspect a live page:

- **Inspector.** The dev build has Inspect Element (right-click).
- **Scripted access.** For scripted access, start it with the remote inspector:
  ```bash
  WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:9222 npm --prefix ui run app:dev
  ```
  `http://127.0.0.1:9222/` lists the page. Its WebSocket is
  `ws://127.0.0.1:9222/socket/1/1/WebPage`; wrap each protocol call
  (`Runtime.evaluate`, …) in `Target.sendMessageToTarget` for the target id from
  the first `Target.targetCreated` event. `Runtime.evaluate`'s `awaitPromise` does
  not resolve here; store async results on `window` and read them back.
- **Per-thread CPU.** Read it from `/proc/<pid>/task/*/stat`. A busy
  `ThreadedCompositor` thread with an idle main thread means compositing cost,
  not script or layout.

Known cost: WebKitGTK re-runs every `backdrop-filter` on each composite, even
`blur(0px)`, so many small filtered elements are far more expensive than one
large one (`plans/notes/2026-10-28-linux-home-lag.md`).

## 7. Keyring / Secret Service

WSL2 Ubuntu 22.04 has no Secret Service provider by default. OxiTide probes for
one at startup and falls back to a session-only store: login works, but nothing
persists across restarts, and the UI shows a notice. That is the expected
default here and is covered by the Track 8.1 smoke checklist.

To exercise the persistent path:

```bash
sudo apt install -y gnome-keyring
dbus-run-session -- bash -lc \
  'echo -n "" | gnome-keyring-daemon --unlock --components=secrets; npm --prefix ui run app:dev'
```

The daemon unlocks the login keyring with an empty password for the disposable
WSL environment (never do this on a real desktop). With it running, a login
survives a restart and the notice is absent.

## Packaging

`crates/oxitide-app/tauri.linux.conf.json` (merged only on Linux) sets the bundle
targets to `deb` + `AppImage`, sets `license = "AGPL-3.0-or-later"`, replaces the
base long description (which says "for Windows"), adds `libasound2`, `xdg-utils`,
and `desktop-file-utils` to the deb `Depends` (Tauri appends
`libwebkit2gtk-4.1-0` and `libgtk-3-0` automatically), and points at
`linux/oxitide.desktop.hbs` (app-dir-relative, resolved after the CLI chdirs to
the app directory). That template terminates the `MimeType=x-scheme-handler/tidal;`
line that the bundler derives from `plugins.deep-link.desktop.schemes` (Tauri's
built-in template omits the trailing semicolon). Windows bundling is untouched.

Install `xdg-utils` on the build host before bundling: with deep links
configured, the AppImage bundler copies `/usr/bin/xdg-mime` into the AppDir and
fails without it. Install `desktop-file-utils` for `desktop-file-validate`,
`update-desktop-database` (runtime deep-link registration), and deb inspection.

    sudo apt-get install -y xdg-utils desktop-file-utils

From the repository root (the CLI resolves `desktopTemplate` from the app
directory after chdir, not from this CWD):

    npm --prefix ui run app:build

Artifacts (unsigned; version from the workspace `Cargo.toml`):

- `target/release/bundle/deb/OxiTide_0.2.0_amd64.deb`
- `target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage`

Inspect the deb metadata and desktop entry:

    dpkg-deb -f target/release/bundle/deb/OxiTide_0.2.0_amd64.deb Depends
    rm -rf /tmp/oxitide-deb
    dpkg-deb -x target/release/bundle/deb/OxiTide_0.2.0_amd64.deb /tmp/oxitide-deb
    grep -E '^(Name|Exec|Icon|Categories|MimeType)=' /tmp/oxitide-deb/usr/share/applications/OxiTide.desktop

Expected: `Categories=AudioVideo;Audio;Music;`,
`MimeType=x-scheme-handler/tidal;`, `Name=OxiTide`, `Exec=oxitide-app`,
`Icon=oxitide-app`.

Inspect the AppImage entry without FUSE:

    ./target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage --appimage-extract
    cat squashfs-root/usr/share/applications/OxiTide.desktop

Launch under WSLg:

    sudo apt-get install -y ./target/release/bundle/deb/OxiTide_0.2.0_amd64.deb
    oxitide-app

    chmod +x target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage
    ./target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage || \
      ./target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage --appimage-extract-and-run

Fact: direct AppImage execution needs FUSE2 (`libfuse2` on Ubuntu 22.04) and
`/dev/fuse`; `--appimage-extract-and-run` bypasses FUSE. Record which path was
used in `plans/notes/2026-10-03-track8-packaging-smoke.md`.

AUR: build/install the `PKGBUILD` attached to a release (rendered from the
`packaging/aur/PKGBUILD.in` template; `node scripts/release/make-pkgbuild.mjs vX.Y.Z <dir>`
renders it locally for an existing tag) on Arch against `webkit2gtk-4.1`
(`makepkg -si`); verification is deferred to the bare-metal checklist.
