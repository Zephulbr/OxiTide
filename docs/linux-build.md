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

## 4. Node 20

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
node --version   # v20.x
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
