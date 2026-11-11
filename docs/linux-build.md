# Linux build environment — WSL2 Ubuntu 22.04

Track 8's pinned Linux build base is **WSL2 Ubuntu 22.04 LTS** (the WebKitGTK
4.1 floor and AppImage's oldest-supported-base guidance; Track 8 spec §4.7).
Builds must run on Linux: Tauri does not cross-compile Windows → Linux.
Every build and cargo gate can be run here; audio/desktop checks that need real
hardware ran on bare metal (CachyOS, `plans/notes/2026-10-23-linux-bare-metal-run.md`
and `plans/notes/2026-11-05-linux-v1-final-run.md`).

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
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
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
- Exclusive/bit-perfect output and hardware mixers cannot be verified here; use
  bare metal with a real DAC (see the run notes above).

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

Known costs (`plans/notes/2026-10-28-linux-home-lag.md`):

- **Backdrop filters.** WebKitGTK re-runs every `backdrop-filter` on each composite,
  even `blur(0px)`, so many small filtered elements are far more expensive than one
  large one.
- **Presented frames.** Every frame the page presents costs the app's GTK main thread
  (the `oxitide-app` thread, not the web process): one 40 px box turning at 60 fps held
  it at ~25% of a core, at 30 fps ~8%. `WEBKIT_DISABLE_DMABUF_RENDERER=1` and
  `GDK_BACKEND=x11` do not lower it, so continuous animations go through
  `ui/src/lib/frameGate.ts`. A CSS animation, even with a `steps()` timing, presents
  every display frame.

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

Artifacts (unsigned; `<version>` is the workspace `Cargo.toml` version):

- `target/release/bundle/deb/OxiTide_<version>_amd64.deb`
- `target/release/bundle/appimage/OxiTide_<version>_amd64.AppImage`

The commands below set `V` to that version first.

Inspect the deb metadata and desktop entry:

    V=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
    dpkg-deb -f target/release/bundle/deb/OxiTide_${V}_amd64.deb Depends
    rm -rf /tmp/oxitide-deb
    dpkg-deb -x target/release/bundle/deb/OxiTide_${V}_amd64.deb /tmp/oxitide-deb
    grep -E '^(Name|Exec|Icon|Categories|MimeType)=' /tmp/oxitide-deb/usr/share/applications/OxiTide.desktop

Expected: `Categories=AudioVideo;Audio;Music;`,
`MimeType=x-scheme-handler/tidal;`, `Name=OxiTide`, `Exec=oxitide-app`,
`Icon=oxitide-app`.

Inspect the AppImage entry without FUSE:

    ./target/release/bundle/appimage/OxiTide_${V}_amd64.AppImage --appimage-extract
    cat squashfs-root/usr/share/applications/OxiTide.desktop

Launch under WSLg:

    sudo apt-get install -y ./target/release/bundle/deb/OxiTide_${V}_amd64.deb
    oxitide-app

    chmod +x target/release/bundle/appimage/OxiTide_${V}_amd64.AppImage
    ./target/release/bundle/appimage/OxiTide_${V}_amd64.AppImage || \
      ./target/release/bundle/appimage/OxiTide_${V}_amd64.AppImage --appimage-extract-and-run

Fact: direct AppImage execution needs FUSE2 (`libfuse2` on Ubuntu 22.04) and
`/dev/fuse`; `--appimage-extract-and-run` bypasses FUSE. Record which path was
used in `plans/notes/2026-10-03-track8-packaging-smoke.md`.

AUR: build/install the `PKGBUILD` attached to a release (rendered from the
`packaging/aur/PKGBUILD.in` template; `node scripts/release/make-pkgbuild.mjs vX.Y.Z <dir>`
renders it locally for an existing tag) on Arch against `webkit2gtk-4.1`
(`makepkg -si`). The recipe sets `options=('!lto')`: under makepkg's default `lto`
the link fails (found and verified on `v0.6.0`, `plans/notes/2026-11-05-linux-v1-final-run.md`).

## Desktop behaviour worth knowing

Seen on bare metal (CachyOS, KDE Wayland, PipeWire; `plans/notes/2026-11-10-linux-v1-handoff-run.md`):

- **AppImage updates.** Only the AppImage updates itself (Settings → System → Updates); deb and
  AUR installs update through the package manager. Run the AppImage directly (AppRun sets
  `$APPIMAGE`, the file the updater replaces), not with `--appimage-extract-and-run`, from a
  folder the user can write (e.g. `~/Applications`). The file is replaced in place and keeps its
  name, so desktop entries pointing at it keep working.
- **Exclusive after PipeWire.** Switching from "System default" to the same DAC's "(direct
  hardware)" entry in exclusive mode plays nothing for ~5 s: PipeWire holds the card until
  WirePlumber suspends the idle sink, and the open waits for it (up to 7 s). Other switches move
  in place in well under a second.
- **WirePlumber after a direct open.** After OxiTide held a card directly, PipeWire can lose that
  card (no sink) until the device is replugged or WirePlumber restarts:
  `systemctl --user restart wireplumber`.
- **`sysdefault:` entries** ("ALSA default, may resample") go through `dmix` and alsa-lib's
  resampler, which can click on skip and seek. Prefer "System default" or the direct entry.

**Oldest base in a container.** A 22.04 distrobox (see the handoff run note) checks the library
floor with the host's Wayland and PipeWire. A bare 22.04 image has no ALSA `default` PCM. Add
`pcm.!default { type pulse }` and `ctl.!default { type pulse }` to
`/etc/alsa/conf.d/99-pulse-default.conf` (`libasound2-plugins`), as a stock 22.04 desktop's
PulseAudio does, then restart the app.
