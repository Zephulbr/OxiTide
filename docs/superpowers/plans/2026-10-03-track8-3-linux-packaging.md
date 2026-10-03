# Track 8.3 Linux Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Track 8 Linux packaging slice: a `tauri.linux.conf.json` that bundles deb + AppImage with a correct `.desktop` entry (`MimeType=x-scheme-handler/tidal;`, Audio categories, AGPL license, non-Windows copy) without touching Windows bundling; the deb and AppImage built and launch-checked in WSL2 Ubuntu 22.04; an in-repo AUR source `PKGBUILD`; and the packaging half of `docs/linux-build.md`.

**Architecture:** The base `crates/oxitide-app/tauri.conf.json` stays Windows-only. A new `crates/oxitide-app/tauri.linux.conf.json` is merged by the Tauri CLI on Linux only (RFC 7396 JSON Merge Patch; arrays are replaced wholesale), so `bundle.targets` becomes `deb` + `appimage` on Linux while Windows keeps `msi` + `nsis`. The bundler already derives `MimeType=x-scheme-handler/tidal;` from the existing `plugins.deep-link.desktop.schemes = ["tidal"]` config, and AppImage reuses the deb data generation; a custom Handlebars template supplies the trailing semicolon the built-in template omits and otherwise matches the default entry byte-for-byte. The AUR PKGBUILD builds the binary with `tauri build --no-bundle` and installs a hand-written `.desktop` (also carrying the scheme handler) plus the repo's PNG icons; its install verification is deferred to an Arch host.

**Tech Stack:** Rust + Tauri 2 (CLI 2.11.5 pinned in `ui/package-lock.json`), Svelte/Vite frontend, WSL2 Ubuntu 22.04, `dpkg-deb`, linuxdeploy/AppImage, Arch `PKGBUILD` (`webkit2gtk-4.1`), Handlebars desktop template.

**Spec:** `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` (§4.4, §4.7, §6 Plan 8.3, §9)

## Global Constraints

- Windows-first. Windows gates must stay green after every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`. No CI exists; all checks are manual.
- Do not edit `crates/oxitide-app/tauri.conf.json` or any Windows bundling setting. The only new config is the Linux platform file; platform configs are read only on their platform.
- All Linux build, extraction, and launch work happens in the WSL2 Ubuntu 22.04 environment from Plan 8.1. Tauri cannot cross-compile; there are no Windows-side bundle builds in this plan.
- Artifacts are unsigned. No signing, no updater, no rpm/Flatpak, no code changes expected in `crates/**` or `ui/**`.
- The AppImage bundler copies `/usr/bin/xdg-mime` whenever deep-link protocols are configured (Tauri CLI 2.11.5, `tauri-bundler/src/bundle/linux/appimage/linuxdeploy.rs`), so the WSL2 build host must have `xdg-utils`. The app's `register_all()` (`crates/oxitide-app/src/main.rs:252`) also needs `xdg-mime` and `update-desktop-database` at runtime (`desktop-file-utils`).
- Runtime registration of `tidal://` via `register_all()` is the AppImage path; the `.desktop` `MimeType` is the installed-deb/AUR path. Both must remain present.
- Tauri CLI 2.11.5 chdirs to the app directory (`crates/tauri-cli/src/build.rs` `setup()` → `set_current_dir(dirs.tauri)`) before bundling, and the bundler reads `bundle.linux.deb.desktopTemplate` CWD-relative; `desktopTemplate` is therefore app-dir-relative (`linux/oxitide.desktop.hbs`), not repo-root-relative.
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green verification instead.
- AUR verification (`makepkg`, `pacman`, `namcap`) is not possible in WSL2 Ubuntu; record it in the smoke note as deferred, never fake it.

## Review Focus

1. **`.desktop` scheme registration (user-visible: `tidal://` does nothing).** The entry must contain exactly `MimeType=x-scheme-handler/tidal;` and `Categories=AudioVideo;Audio;Music;`. Tauri's built-in desktop template emits `MimeType=x-scheme-handler/tidal` with **no trailing semicolon** in 2.11.5, so the custom template is load-bearing. Pinned by Task 1's template assertion and Task 2's `grep -qx` checks on the extracted deb and AppImage (plus Task 3 on the AUR `.desktop`).
2. **AppImage build fails on a bare WSL2 host: `xdg-mime binary not found`.** With deep links configured, the bundler hard-copies `/usr/bin/xdg-mime`; `xdg-utils` is not in the spec's apt list. Pinned by Task 2's `command -v xdg-mime` preflight and the recorded apt install.
3. **`desktopTemplate` path resolution.** CLI 2.11.5 chdirs to the app directory (`crates/oxitide-app`) before bundling and hands `bundle.linux.deb.desktopTemplate` to the bundler unresolved, so the path must be app-dir-relative (`linux/oxitide.desktop.hbs`); a wrong path fails the build with `Failed to setup custom handlebar template`. Pinned by Task 1's template assertion, Task 2's build step, and the extracted-entry check.
4. **AppImage FUSE on WSL2.** Direct AppImage execution mounts through FUSE2; WSL2 may lack `/dev/fuse` or `libfuse.so.2`. Pinned by Task 2's `test -e /dev/fuse` probe and the documented `--appimage-extract-and-run` fallback; extraction-based verification works without FUSE.
5. **AUR source pin / install layout drift.** The repo has no git tags, so the recipe must pin a commit, and `Exec=oxitide` must match the installed binary path (the deb binary is `oxitide-app`, the AUR rename is `oxitide`). Pinned by Task 3's `git cat-file -e` and `grep -qx 'Exec=oxitide'` checks; install verification is deferred.

---

## File Structure

**Linux bundle config (authored on Windows, read only on Linux)**

- Create `crates/oxitide-app/tauri.linux.conf.json` — Linux bundle targets, license, long description, deb `Depends`, desktop template path.
- Create `crates/oxitide-app/linux/oxitide.desktop.hbs` — desktop entry template (scheme handler with trailing `;`).

**WSL2 build (Task 2)**

- No new repo scripts. Artifacts land in `target/release/bundle/deb/` and `target/release/bundle/appimage/` (gitignored); results are recorded in the smoke note.

**AUR (Task 3)**

- Create `packaging/aur/PKGBUILD` — source build, commit-pinned.
- Create `packaging/aur/oxitide.desktop` — installed desktop entry with the scheme handler.

**Docs (Tasks 3–4, plus the smoke note)**

- Modify (or create) `docs/linux-build.md` — add a `## Packaging` section; merges with Plan 8.1's environment doc.
- Modify `docs/superpowers/plans/notes/2026-10-03-track8-packaging-smoke.md` — record results, artifact names, and deferred items (created at plan-writing time; filled while executing).

---

### Task 1: Linux bundle config and desktop template

**Files:**
- Create: `crates/oxitide-app/tauri.linux.conf.json`
- Create: `crates/oxitide-app/linux/oxitide.desktop.hbs`

**Interfaces:**
- Consumes: base `crates/oxitide-app/tauri.conf.json` (`productName` `OxiTide`, identifier `org.codeberg.zephulbr.oxitide`, `plugins.deep-link.desktop.schemes = ["tidal"]`, `category: Music`, PNG icons); the workspace license `AGPL-3.0-or-later` in `Cargo.toml`.
- Produces: the merged Linux bundle configuration used by `tauri build`; `bundle.linux.deb.desktopTemplate` consumed by the deb **and** AppImage bundlers (AppImage reuses `debian::generate_data`); desktop variables available to the template are `categories`, `comment`, `exec`, `icon`, `name`, `mime_type`, `long_description` (verified against Tauri 2.11.5 `freedesktop/mod.rs`).

- [ ] **Step 1: Write the failing check (Windows PowerShell, from the repo root)**

Save and run this check as a scratch command (not a repo file). It asserts the new files and their required content; it must fail now because neither file exists.

```powershell
$conf = 'crates/oxitide-app/tauri.linux.conf.json'
$tpl  = 'crates/oxitide-app/linux/oxitide.desktop.hbs'
if (-not (Test-Path $conf)) { throw "missing $conf" }
$json = Get-Content $conf -Raw | ConvertFrom-Json
if (($json.bundle.targets -join ',') -ne 'deb,appimage') { throw 'targets must be deb,appimage' }
if ($json.bundle.license -ne 'AGPL-3.0-or-later') { throw 'license must be AGPL-3.0-or-later' }
if ($json.bundle.longDescription -match 'Windows') { throw 'longDescription still says Windows' }
if (($json.bundle.linux.deb.depends -notcontains 'libasound2') -or
    ($json.bundle.linux.deb.depends -notcontains 'xdg-utils') -or
    ($json.bundle.linux.deb.depends -notcontains 'desktop-file-utils')) { throw 'deb depends incomplete' }
if ($json.bundle.linux.deb.desktopTemplate -ne 'linux/oxitide.desktop.hbs') { throw 'desktopTemplate path' }
if (-not (Test-Path $tpl)) { throw "missing $tpl" }
$tplText = Get-Content $tpl -Raw
if ($tplText -notmatch [regex]::Escape('MimeType={{mime_type}};')) { throw 'template lacks trailing-semicolon MimeType' }
"OK: linux bundle config"
```

- [ ] **Step 2: Verify it fails for the right reason (Windows)**

Run the check. Expected: `Exception: missing crates/oxitide-app/tauri.linux.conf.json` — the file genuinely does not exist yet; that is the red state.

- [ ] **Step 3: Implement**

Create `crates/oxitide-app/tauri.linux.conf.json` (strict JSON; the CLI validates the merged result against the schema at `https://schema.tauri.app/config/2`):

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "bundle": {
    "targets": ["deb", "appimage"],
    "license": "AGPL-3.0-or-later",
    "longDescription": "OxiTide is an unofficial TIDAL desktop client with bit-perfect playback, a streaming cache, and a native Tauri UI.",
    "linux": {
      "deb": {
        "depends": ["libasound2", "xdg-utils", "desktop-file-utils"],
        "desktopTemplate": "linux/oxitide.desktop.hbs"
      }
    }
  }
}
```

Notes for the implementer:

- The Tauri CLI appends `libwebkit2gtk-4.1-0` and `libgtk-3-0` to `Depends` automatically on Linux (verified in CLI 2.11.5 `tauri_config_to_bundle_settings`), so they are not listed here. `libasound2` is the package name on Ubuntu 22.04 / Debian 12; on Ubuntu 24.04+ it is superseded by `libasound2t64` with a transitional `libasound2` — accepted baseline behavior, note it in the smoke note.
- No `fileAssociations` entry is needed: the bundler builds `MimeType` from `plugins.deep-link.desktop.schemes` (already `["tidal"]` in the base config) plus any file associations.
- `bundle.linux.appimage.bundleMediaFramework` stays at its default `false`: OxiTide's audio is native (ALSA) and the webview renders no HTML media.

Create `crates/oxitide-app/linux/oxitide.desktop.hbs` (this mirrors Tauri 2.11.5's `freedesktop/main.desktop` but terminates the MIME list; do not change the variable names):

```
[Desktop Entry]
Categories={{categories}}
{{#if comment}}
Comment={{comment}}
{{/if}}
Exec={{exec}}
StartupWMClass={{exec}}
Icon={{icon}}
Name={{name}}
Terminal=false
Type=Application
{{#if mime_type}}
MimeType={{mime_type}};
{{/if}}
```

- [ ] **Step 4: Verify it passes (Windows)**

Run the Step 1 check from the repo root. Expected: `OK: linux bundle config`.

Prove Windows bundling is untouched:

```powershell
git diff --exit-code -- crates/oxitide-app/tauri.conf.json
git status --short
```

Expected: the diff exits `0` (no output); `git status` lists only the two new files. Then run the Windows gates:

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

Expected: all pass; the Linux config is not read on Windows because the CLI looks up `tauri.linux.conf.json` only for the Linux target.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/tauri.linux.conf.json crates/oxitide-app/linux/oxitide.desktop.hbs
git commit -m "chore(packaging): add the Linux deb/AppImage bundle config"
```

---

### Task 2: Build deb + AppImage in WSL2 and verify under WSLg

**Files:**
- Modify: `docs/superpowers/plans/notes/2026-10-03-track8-packaging-smoke.md` (record results)
- Artifacts (not committed; gitignored): `target/release/bundle/deb/OxiTide_0.2.0_amd64.deb`, `target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage`

**Interfaces:**
- Consumes: Task 1's `tauri.linux.conf.json` + template; the Plan 8.1 WSL2 Ubuntu 22.04 environment (`docs/linux-build.md`); `ui/package-lock.json` (Tauri CLI 2.11.5).
- Produces: the two bundles and recorded launch evidence. Version strings come from `crates/oxitide-app/tauri.conf.json` (`0.2.0`); the arch segment is `amd64`. If a later change bumps the version, substitute it in every path.

- [ ] **Step 1: Write/execute the failing verification (WSL2 Ubuntu 22.04, repo root)**

This is the exact acceptance check for the slice. Run it before building; it must fail at the first missing prerequisite or artifact.

```bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
DEB=target/release/bundle/deb/OxiTide_0.2.0_amd64.deb
APP=target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage

command -v xdg-mime >/dev/null || { echo "FAIL: xdg-mime missing (sudo apt-get install -y xdg-utils)"; exit 1; }
command -v update-desktop-database >/dev/null || { echo "FAIL: update-desktop-database missing (sudo apt-get install -y desktop-file-utils)"; exit 1; }
test -f "$DEB" || { echo "FAIL: $DEB missing"; exit 1; }
test -f "$APP" || { echo "FAIL: $APP missing"; exit 1; }

rm -rf /tmp/oxitide-deb && dpkg-deb -x "$DEB" /tmp/oxitide-deb
DESK=/tmp/oxitide-deb/usr/share/applications/OxiTide.desktop
test -f "$DESK" || { echo "FAIL: $DESK missing"; exit 1; }
grep -qx 'MimeType=x-scheme-handler/tidal;' "$DESK" || { echo "FAIL: MimeType line"; exit 1; }
grep -qx 'Categories=AudioVideo;Audio;Music;' "$DESK" || { echo "FAIL: Categories line"; exit 1; }
grep -qx 'Exec=oxitide-app' "$DESK" || { echo "FAIL: Exec line"; exit 1; }
grep -qx 'Name=OxiTide' "$DESK" || { echo "FAIL: Name line"; exit 1; }
grep -qx 'Icon=oxitide-app' "$DESK" || { echo "FAIL: Icon line"; exit 1; }
test -f /tmp/oxitide-deb/usr/share/icons/hicolor/512x512/apps/oxitide-app.png || { echo "FAIL: 512 icon"; exit 1; }
dpkg-deb -f "$DEB" Depends | grep -q 'libasound2' || { echo "FAIL: libasound2 dep"; exit 1; }
dpkg-deb -f "$DEB" Depends | grep -q 'libwebkit2gtk-4.1-0' || { echo "FAIL: webkit dep"; exit 1; }
dpkg-deb -f "$DEB" Architecture | grep -qx 'amd64' || { echo "FAIL: arch"; exit 1; }

rm -rf /tmp/oxitide-appimage && mkdir -p /tmp/oxitide-appimage && cd /tmp/oxitide-appimage
"$ROOT/$APP" --appimage-extract >/dev/null
APPDESK=squashfs-root/usr/share/applications/OxiTide.desktop
test -f "$APPDESK" || { echo "FAIL: AppImage desktop file missing"; exit 1; }
grep -qx 'MimeType=x-scheme-handler/tidal;' "$APPDESK" || { echo "FAIL: AppImage MimeType"; exit 1; }
grep -qx 'Categories=AudioVideo;Audio;Music;' "$APPDESK" || { echo "FAIL: AppImage Categories"; exit 1; }
echo "PASS: deb + AppImage checks"
```

- [ ] **Step 2: Verify it fails for the right reason (WSL2)**

Run Step 1. Expected: either `FAIL: xdg-mime missing` (install it, then rerun) or `FAIL: target/release/bundle/deb/OxiTide_0.2.0_amd64.deb missing` — the artifacts genuinely do not exist yet. If `xdg-mime` was missing, install both host tools first:

```bash
sudo apt-get update && sudo apt-get install -y xdg-utils desktop-file-utils
```

- [ ] **Step 3: Build the bundles (WSL2, repo root)**

```bash
cd "$(git rev-parse --show-toplevel)"
npm --prefix ui run app:build
```

This runs Vite's production build via `beforeBuildCommand` and then `tauri build` with the merged Linux config, producing `deb` and `appimage`. Expected tail: `Finished 2 bundles at:` followed by the deb and AppImage paths. First Linux release build is long; the AppImage step downloads linuxdeploy/AppRun into the Tauri tools cache on first use, so allow network access.

If the build fails with `Failed to setup custom handlebar template`, the `desktopTemplate` path is wrong: the CLI chdirs to the app directory before bundling, so it must be app-dir-relative (`linux/oxitide.desktop.hbs`), not repo-root-relative.

- [ ] **Step 4: Verify it passes and launch under WSLg (WSL2)**

Rerun the Step 1 script. Expected: `PASS: deb + AppImage checks`.

Record the metadata that the script checks piecemeal:

```bash
dpkg-deb -f target/release/bundle/deb/OxiTide_0.2.0_amd64.deb Package Version Architecture Depends
ls -l target/release/bundle/deb/ target/release/bundle/appimage/
```

Launch checks (human, WSLg window; record PASS/FAIL with a screenshot or log excerpt in the smoke note):

```bash
sudo apt-get install -y ./target/release/bundle/deb/OxiTide_0.2.0_amd64.deb
oxitide-app
```

Expected: the OxiTide window opens in WSLg. (Login/audio behavior depends on Plans 8.1/8.2 having landed; GUI launch is what this task verifies.) Close it before continuing.

```bash
chmod +x target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage
test -e /dev/fuse && echo "FUSE present" || echo "FUSE absent"
./target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage \
  || ./target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage --appimage-extract-and-run
```

Fact: direct execution of a type-2 AppImage needs FUSE2 (`libfuse2` on Ubuntu 22.04) and `/dev/fuse`; `--appimage-extract-and-run` is the upstream-supported path when FUSE is unavailable. Record which invocation worked. (Desktop-entry verification in Step 1 does not need FUSE.)

Update the smoke note: fill the build-environment line, the exact artifact paths from `ls`, the `dpkg-deb -f` output, and the two launch results. Cross-reference `notes/2026-10-03-track8-wslg-smoke.md` for the shared launch rows; do not duplicate them beyond the pointer.

- [ ] **Step 5: Commit**

If the only change is the recorded smoke results:

```bash
git add docs/superpowers/plans/notes/2026-10-03-track8-packaging-smoke.md
git commit -m "docs: record the Linux packaging smoke results"
```

If the run exposed a config fix, commit that separately first (`chore(packaging): ...`) with the exact change and evidence.

---

### Task 3: AUR source recipe

**Files:**
- Create: `packaging/aur/PKGBUILD`
- Create: `packaging/aur/oxitide.desktop`
- Modify: `docs/superpowers/plans/notes/2026-10-03-track8-packaging-smoke.md` (record deferred AUR checks)

**Interfaces:**
- Consumes: remote `https://codeberg.org/Zephulbr/OxiTide.git` (read with `git remote get-url origin`); repo files present at the pinned commit: `LICENSE`, `THIRD_PARTY_NOTICES.md`, `crates/oxitide-app/icons/{32x32,128x128,128x128@2x,icon}.png`, `packaging/aur/oxitide.desktop`; UI lockfile for `npm ci`.
- Produces: `oxitide` package build (`/usr/bin/oxitide`, hicolor icons, `/usr/share/applications/oxitide.desktop` with `MimeType=x-scheme-handler/tidal;`), installable on Arch with `webkit2gtk-4.1`. There are no git tags in the repo, so the source is commit-pinned; the install run is deferred.

- [ ] **Step 1: Write/execute the failing check (WSL2 or Git Bash, repo root)**

```bash
set -euo pipefail
test -f packaging/aur/PKGBUILD || { echo "FAIL: PKGBUILD missing"; exit 1; }
test -f packaging/aur/oxitide.desktop || { echo "FAIL: desktop file missing"; exit 1; }
bash -n packaging/aur/PKGBUILD
grep -q "^pkgver=0.2.0$" packaging/aur/PKGBUILD || { echo "FAIL: pkgver"; exit 1; }
grep -q "^license=('AGPL-3.0-or-later')$" packaging/aur/PKGBUILD || { echo "FAIL: license"; exit 1; }
grep -q "'webkit2gtk-4.1' 'gtk3' 'alsa-lib' 'librsvg' 'hicolor-icon-theme' 'xdg-utils' 'desktop-file-utils'" packaging/aur/PKGBUILD || { echo "FAIL: depends"; exit 1; }
grep -q "^makedepends=('rust' 'nodejs' 'npm' 'pkgconf' 'git')$" packaging/aur/PKGBUILD || { echo "FAIL: makedepends"; exit 1; }
grep -qx 'MimeType=x-scheme-handler/tidal;' packaging/aur/oxitide.desktop || { echo "FAIL: desktop MimeType"; exit 1; }
grep -qx 'Exec=oxitide' packaging/aur/oxitide.desktop || { echo "FAIL: desktop Exec"; exit 1; }
echo "OK: AUR recipe static checks"
```

- [ ] **Step 2: Verify it fails for the right reason**

Run Step 1. Expected: `FAIL: PKGBUILD missing` — neither file exists yet.

- [ ] **Step 3: Implement**

Create `packaging/aur/oxitide.desktop`:

```desktop
[Desktop Entry]
Type=Application
Name=OxiTide
GenericName=Music Player
Comment=Unofficial TIDAL desktop client
Exec=oxitide
Icon=oxitide
Terminal=false
Categories=AudioVideo;Audio;Music;
StartupWMClass=oxitide
MimeType=x-scheme-handler/tidal;
Keywords=tidal;music;streaming;
```

Create `packaging/aur/PKGBUILD`. Substitute `_commit` with the value of `git rev-parse HEAD` at execution time before writing (the literal below is HEAD at plan-writing time; Step 4 re-pins it to this recipe's commit, which is what must survive):

```sh
# Maintainer: OxiTide project <https://codeberg.org/Zephulbr/OxiTide/issues>
pkgname=oxitide
pkgver=0.2.0
pkgrel=1
pkgdesc="Unofficial TIDAL desktop client"
arch=('x86_64')
url="https://codeberg.org/Zephulbr/OxiTide"
license=('AGPL-3.0-or-later')
depends=('webkit2gtk-4.1' 'gtk3' 'alsa-lib' 'librsvg' 'hicolor-icon-theme' 'xdg-utils' 'desktop-file-utils')
makedepends=('rust' 'nodejs' 'npm' 'pkgconf' 'git')
_commit=5eb328434f87456a6db8afabc8a59ce0d0fffc02
source=("$pkgname::git+$url.git#commit=$_commit")
sha256sums=('SKIP')

prepare() {
  cd "$pkgname"
  npm --prefix ui ci
  cargo fetch --locked
}

build() {
  cd "$pkgname"
  export CARGO_NET_OFFLINE=true
  ./ui/node_modules/.bin/tauri build --no-bundle
}

package() {
  cd "$pkgname"
  install -Dm755 target/release/oxitide-app "$pkgdir/usr/bin/oxitide"
  install -Dm644 packaging/aur/oxitide.desktop "$pkgdir/usr/share/applications/oxitide.desktop"
  install -Dm644 crates/oxitide-app/icons/32x32.png "$pkgdir/usr/share/icons/hicolor/32x32/apps/oxitide.png"
  install -Dm644 crates/oxitide-app/icons/128x128.png "$pkgdir/usr/share/icons/hicolor/128x128/apps/oxitide.png"
  install -Dm644 crates/oxitide-app/icons/128x128@2x.png "$pkgdir/usr/share/icons/hicolor/256x256/apps/oxitide.png"
  install -Dm644 crates/oxitide-app/icons/icon.png "$pkgdir/usr/share/icons/hicolor/512x512/apps/oxitide.png"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 THIRD_PARTY_NOTICES.md "$pkgdir/usr/share/licenses/$pkgname/THIRD_PARTY_NOTICES.md"
}
```

Rationale for the non-obvious lines:

- `--no-bundle` skips Tauri's bundlers entirely; `package()` owns the install layout, which is what an AUR source package should do.
- `CARGO_NET_OFFLINE=true` after `cargo fetch --locked` keeps the build reproducible from the fetched lockfile state; `npm ci` uses `ui/package-lock.json`.
- `xdg-utils` and `desktop-file-utils` are runtime deps of `register_all()` (`main.rs:252`): the deep-link plugin shells out to `xdg-mime` and `update-desktop-database` (verified in `plugins-workspace/plugins/deep-link/src/lib.rs`).
- The binary is installed as `/usr/bin/oxitide` and the desktop entry's `Exec=oxitide` matches it; the deb keeps `oxitide-app` because Tauri generates that entry itself.

- [ ] **Step 4: Verify it passes, then pin the source to this commit**

Run the Step 1 script. Expected: `OK: AUR recipe static checks`. If `desktop-file-utils` is installed (Task 2), also run:

```bash
desktop-file-validate packaging/aur/oxitide.desktop
```

Expected: no output (valid). Then commit the recipe (commit A) and re-pin `_commit` to it, because the pinned tree must contain `packaging/aur/oxitide.desktop` for `package()`:

```bash
git add packaging/aur/PKGBUILD packaging/aur/oxitide.desktop
git commit -m "chore(packaging): add the AUR source recipe"
A="$(git rev-parse HEAD)"
sed -i "s/^_commit=.*/_commit=$A/" packaging/aur/PKGBUILD
grep -q "^_commit=$A$" packaging/aur/PKGBUILD
git cat-file -e "$A:packaging/aur/oxitide.desktop"
git cat-file -e "$A:crates/oxitide-app/icons/icon.png"
git cat-file -e "$A:LICENSE"
echo "OK: source pinned to $A"
```

Expected: `OK: source pinned to <commit A>` with no `git cat-file` errors. (`makepkg --printsrcinfo` and `namcap` are Arch-only; they are recorded as deferred in the smoke note.)

- [ ] **Step 5: Commit the re-pin and the smoke note**

```bash
git add packaging/aur/PKGBUILD docs/superpowers/plans/notes/2026-10-03-track8-packaging-smoke.md
git commit -m "chore(packaging): pin the AUR source to the packaging commit"
```

Update the smoke note's deferred section before this commit: Arch `makepkg -si`/`namcap`/`xdg-mime query default x-scheme-handler/tidal` and the oldest-base launch checks stay unchecked with their exact commands, cross-referenced to `notes/2026-10-03-track8-bare-metal-checklist.md` (packaging rows).

---

### Task 4: Document packaging in `docs/linux-build.md`

**Files:**
- Modify (or create): `docs/linux-build.md` — append a `## Packaging` section

**Interfaces:**
- Consumes: Task 1's config, Task 2's verified commands/artifact paths, Task 3's AUR location.
- Produces: the packaging section Plan 8.1 extends (or that extends 8.1's environment section); no code.

- [ ] **Step 1: Write/execute the failing check (WSL2 or Windows)**

```bash
grep -q '^## Packaging$' docs/linux-build.md || { echo "FAIL: Packaging section missing"; exit 1; }
grep -q 'xdg-utils' docs/linux-build.md || { echo "FAIL: xdg-utils note missing"; exit 1; }
grep -q 'OxiTide_0.2.0_amd64.AppImage' docs/linux-build.md || { echo "FAIL: artifact path missing"; exit 1; }
echo "OK: packaging docs"
```

- [ ] **Step 2: Verify it fails for the right reason**

Run Step 1. Expected: `FAIL: Packaging section missing` (the file does not exist yet, or exists without the section if Plan 8.1 has landed).

- [ ] **Step 3: Implement**

If `docs/linux-build.md` does not exist yet (Plan 8.1 not executed), create it with exactly this content; if it exists, append from `## Packaging` onward and leave 8.1's content untouched:

```markdown
# OxiTide Linux build

> This file is completed by Plan 8.1 (WSL2 Ubuntu 22.04 environment) and
> Plan 8.3 (packaging). The sections below are merged in whichever order the
> plans land.

## Packaging

`crates/oxitide-app/tauri.linux.conf.json` (merged only on Linux) sets the bundle
targets to `deb` + `AppImage`, sets `license = "AGPL-3.0-or-later"`, replaces the
base long description (which says "for Windows"), adds `libasound2`, `xdg-utils`,
and `desktop-file-utils` to the deb `Depends` (Tauri appends
`libwebkit2gtk-4.1-0` and `libgtk-3-0` automatically), and points at
`crates/oxitide-app/linux/oxitide.desktop.hbs`. That template terminates the
`MimeType=x-scheme-handler/tidal;` line that the bundler derives from
`plugins.deep-link.desktop.schemes` (Tauri's built-in template omits the
trailing semicolon). Windows bundling is untouched.

Install `xdg-utils` on the build host before bundling: with deep links
configured, the AppImage bundler copies `/usr/bin/xdg-mime` into the AppDir and
fails without it. Install `desktop-file-utils` for `desktop-file-validate`,
`update-desktop-database` (runtime deep-link registration), and deb inspection.

    sudo apt-get install -y xdg-utils desktop-file-utils

From the repository root (the CLI resolves `desktopTemplate` from the app
directory after chdir, not from this CWD):

    npm --prefix ui run app:build

Artifacts (unsigned; version from `crates/oxitide-app/tauri.conf.json`):

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

AUR: build/install `packaging/aur/PKGBUILD` on Arch against `webkit2gtk-4.1`
(`makepkg -si`); verification is deferred to the bare-metal checklist.
```

- [ ] **Step 4: Verify it passes**

Run the Step 1 script. Expected: `OK: packaging docs`. Check the file has no unrelated changes:

```bash
git diff --stat -- docs/linux-build.md
```

Expected: only additions under `## Packaging`.

- [ ] **Step 5: Commit**

```bash
git add docs/linux-build.md
git commit -m "docs: add Linux packaging instructions"
```
