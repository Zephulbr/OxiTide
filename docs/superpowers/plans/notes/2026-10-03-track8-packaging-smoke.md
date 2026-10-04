# Track 8 packaging smoke checklist (2026-10-03)

Human-in-the-loop checks for the packaging half of Track 8 (Plan 8.3,
`plans/2026-10-03-track8-3-linux-packaging.md`). Build environment: WSL2
Ubuntu 22.04 from Plan 8.1 (`docs/linux-build.md`), with WSLg for GUI windows.
Run every command from the repository root on the `track8-linux` branch/worktree.

**Result (2026-10-04): PASS** for the build/artifact/static rows; the two
launch rows are process-level only (window visibility pending). The build ran
from a Linux-side clone, not `/mnt/c`: `git clone --branch track8-linux
/mnt/c/Users/V/Documents/Opencode/OxiTide /root/oxitide-8.3`, checked out
`5c4ecfadce2cb48906d96e296efcb4e0e69fb7ca`, `git status --short` clean. A
dependency alignment was required first: the committed lockfiles had the
`tauri` crate at 2.12.0 while `@tauri-apps/api` was 2.11.1, which the Tauri CLI
version preflight rejects (`Found version mismatched Tauri packages`, exit 1 —
it also blocked the Windows `app:build`). Commit `5c4ecfa` (`chore(ui): align
Tauri JS packages with the 2.12 crate`) moved `@tauri-apps/api` 2.11.1 → 2.12.1
and `@tauri-apps/cli` 2.11.5 → 2.12.1. UI gates after the update:
`npm --prefix ui test` 506/506, `npm --prefix ui run check` 0 errors/0 warnings.
The acceptance script printed `PASS: deb + AppImage checks` and the launch rows
below are from that commit.

Record each row as PASS/FAIL with date, WSL distro/kernel, `rustc -V`, `node -v`,
`npm -v`, and evidence (command output, screenshot, log excerpt). A FAIL becomes
an issue; do not silently skip a row. The shared launch rows also appear in
`2026-10-03-track8-wslg-smoke.md` (Packaging launch) — record the detail here and
keep that file's checkbox in sync. Deferred items live in
`2026-10-03-track8-bare-metal-checklist.md` (Packaging and platform floor).

Expected artifact names (version from `crates/oxitide-app/tauri.conf.json`,
`architecture` segment `amd64`; substitute if either changes):

- `target/release/bundle/deb/OxiTide_0.2.0_amd64.deb`
- `target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage`

## Build environment (record)

- [x] **Host tools present:** `command -v xdg-mime` and
      `command -v update-desktop-database` both resolve. The AppImage bundler
      hard-copies `/usr/bin/xdg-mime` when `plugins.deep-link` is configured;
      missing `xdg-utils` fails the build. Record `xdg-utils` and
      `desktop-file-utils` versions (`dpkg-query -W`).
      PASS 2026-10-04: `xdg-utils 1.1.3-4.1ubuntu3~22.04.1`,
      `desktop-file-utils 0.26-1ubuntu3`.
- [x] **Environment lines recorded:** WSL distro/version (`lsb_release -ds`),
      kernel (`uname -r`), Rust (`rustc -V`), Node/npm (`node -v`, `npm -v`),
      Tauri CLI (`./ui/node_modules/.bin/tauri --version`, expected `2.11.5`).
      PASS: `Ubuntu 22.04.5 LTS`; kernel
      `6.18.40.1-microsoft-standard-WSL2`; `rustc 1.99.0 (b940084d7
      2026-09-28)`; node `v20.20.2`; npm `10.8.2`; Tauri CLI `tauri-cli 2.12.1`
      after the alignment (was expected `2.11.5` pre-alignment); webkit2gtk-4.1
      `2.50.4` (`tauri info`).

## Windows packaging preflight (post-JS-2.12 alignment, 2026-10-04)

After the `5c4ecfa` alignment, `ui\node_modules\.bin\tauri.cmd info` from this
worktree shows no version-mismatch error and the aligned set:

```
    - tauri 🦀: 2.12.0, (outdated, latest: 2.12.1)
    - @tauri-apps/api  ⱼₛ: 2.12.1
    - @tauri-apps/cli  ⱼₛ: 2.12.1
```

(`outdated, latest:` is informational; the mismatch preflight is clean.)
Windows bundle build not run in this track; release-time check.

## WSL2-verifiable now

- [x] **Build:** `npm --prefix ui run app:build` completes and reports
      `Finished 2 bundles at:` for deb + AppImage. Record the run date and
      build time.
      PASS 2026-10-04, from `/root/oxitide-8.3` at `5c4ecfa`; started
      `01:24:54 +03:00`, bundles written by `01:26`–`01:27` (≈2.5 min;
      dependencies were partly pre-compiled by an interrupted first attempt).
      Tail:

```
    Finished `release` profile [optimized] target(s) in 1m 48s
       Built application at: /root/oxitide-8.3/target/release/oxitide-app
    Bundling OxiTide_0.2.0_amd64.deb (/root/oxitide-8.3/target/release/bundle/deb/OxiTide_0.2.0_amd64.deb)
    Bundling OxiTide_0.2.0_amd64.AppImage (/root/oxitide-8.3/target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage)
    Finished 2 bundles at:
        /root/oxitide-8.3/target/release/bundle/deb/OxiTide_0.2.0_amd64.deb (9.57 MiB)
        /root/oxitide-8.3/target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage (83.50 MiB)
    (NPM_BUILD_EXIT=0)
```

- [x] **Artifacts exact paths:** `ls -l target/release/bundle/deb/` and
      `ls -l target/release/bundle/appimage/`; copy the actual filenames here
      (expected names above). Record sizes.
      PASS. Files (next to the unpacked `OxiTide_0.2.0_amd64/` and
      `OxiTide.AppDir/` directories):

```
-rw-r--r-- 1 root root 10032426 Oct  4 01:26 target/release/bundle/deb/OxiTide_0.2.0_amd64.deb
-rwxr-xr-x 1 root root 87558648 Oct  4 01:27 target/release/bundle/appimage/OxiTide_0.2.0_amd64.AppImage
```
- [x] **Artifact architecture:** `dpkg-deb -f ... Architecture` prints `amd64`.
      PASS.
- [x] **Deb Depends:** `dpkg-deb -f ... Depends` lists `libasound2`,
      `xdg-utils`, `desktop-file-utils`, plus the automatically appended
      `libwebkit2gtk-4.1-0` and `libgtk-3-0`. (On Ubuntu 24.04+ note that
      `libasound2` is transitional to `libasound2t64`.)
      PASS; `dpkg-deb -f ... Package Version Architecture Depends`:

```
Package: oxi-tide
Version: 0.2.0
Architecture: amd64
Depends: libasound2, xdg-utils, desktop-file-utils, libwebkit2gtk-4.1-0, libgtk-3-0
```
- [x] **Deb desktop entry:** `dpkg-deb -x` to `/tmp/oxitide-deb`; the
      `usr/share/applications/OxiTide.desktop` file contains exactly:
      `Categories=AudioVideo;Audio;Music;`,
      `MimeType=x-scheme-handler/tidal;`, `Name=OxiTide`,
      `Exec=oxitide-app`, `Icon=oxitide-app`; the matching PNGs exist under
      `usr/share/icons/hicolor/{32x32,128x128,256x256,512x512}/apps/oxitide-app.png`.
      Paste the `grep -E '^(Name|Exec|Icon|Categories|MimeType)='` output.
      PASS; full entry (all keys):

```
Name=OxiTide
Exec=oxitide-app
Icon=oxitide-app
Categories=AudioVideo;Audio;Music;
MimeType=x-scheme-handler/tidal;
StartupWMClass=oxitide-app
Terminal=false
Type=Application
```

      Icons present: `32x32`, `128x128`, `256x256@2`, `512x512` (no plain
      `256x256` directory; the acceptance check on `512x512` passes).
- [x] **AppImage desktop entry:** run the AppImage with `--appimage-extract`
      (no FUSE needed) and verify
      `squashfs-root/usr/share/applications/OxiTide.desktop` carries the same
      `MimeType` and `Categories` lines. Paste the output.
      PASS:

```
Categories=AudioVideo;Audio;Music;
MimeType=x-scheme-handler/tidal;
```
- [ ] **Deb launch (WSLg):** `sudo apt-get install -y ./target/release/bundle/deb/OxiTide_0.2.0_amd64.deb`
      then `oxitide-app`; the window opens in WSLg. Record whether this build
      predates Plans 8.1/8.2 (login notice/audio may be stubs).
      **Process-level result (2026-10-04):** process started and stayed alive
      20 s; window visibility pending a human check. `apt-get install -y
      ./target/...deb` → exit 0, `oxi-tide 0.2.0` installed to
      `/usr/bin/oxitide-app`; `timeout 20 oxitide-app` → exit `124` (still
      running when the timeout killed it; no crash). Log: the Plan 8.1
      session-only keyring warning (`org.freedesktop.secrets` absent) plus WSLg
      `Dropped Escape call` lines. Plans 8.1/8.2 are included in this build.
- [ ] **AppImage launch (WSLg):** `chmod +x`, probe `test -e /dev/fuse`, then
      run directly; if it fails with a FUSE/`libfuse.so.2` error, rerun with
      `--appimage-extract-and-run` and record which path worked.
      **Process-level result (2026-10-04):** process started and stayed alive
      20 s; window visibility pending a human check. `/dev/fuse` present; both
      `timeout 20 ./…AppImage` and `timeout 20 ./…AppImage
      --appimage-extract-and-run` → exit `124` (alive at timeout, no FUSE
      error). The extract-and-run path logged the expected
      `APPDIR`/`APPIMAGE` not-detected warning.
- [x] **AUR recipe static checks (Task 3 owns these):** `bash -n packaging/aur/PKGBUILD`,
      `grep -qx 'MimeType=x-scheme-handler/tidal;' packaging/aur/oxitide.desktop`,
      `grep -qx 'Exec=oxitide' packaging/aur/oxitide.desktop`,
      `desktop-file-validate packaging/aur/oxitide.desktop` (no output), and
      `_commit` resolves to a commit containing all seven source files installed
      by `package()`: `packaging/aur/oxitide.desktop`,
      `crates/oxitide-app/icons/{32x32,128x128,128x128@2x,icon}.png`,
      `LICENSE`, and `THIRD_PARTY_NOTICES.md` (`git cat-file -e`).
      PASS 2026-10-04 (Task 3): the full Step 1 script printed
      `OK: AUR recipe static checks`; `desktop-file-validate` produced no output
      (exit 0); `_commit=31b27a8d532b977637d5418c003a5fd47f7784f0` (commit A,
      the recipe commit). Pin evidence (re-verified 2026-10-04 in the
      whole-branch review): `git cat-file -e 31b27a8…:<path>` succeeded for all
      seven installed source files — `packaging/aur/oxitide.desktop`,
      `crates/oxitide-app/icons/32x32.png`,
      `crates/oxitide-app/icons/128x128.png`,
      `crates/oxitide-app/icons/128x128@2x.png`,
      `crates/oxitide-app/icons/icon.png`, `LICENSE`, and
      `THIRD_PARTY_NOTICES.md`; committed blobs are LF-only.
- [ ] **`tidal://` best effort (WSLg):** if `xdg-open tidal://...` reaches the
      app, record it; full registration is deferred.
      Partial 2026-10-04: the AppImage run's runtime `register_all()` wrote
      `/root/.local/share/applications/oxitide-app-handler.desktop`
      (`Exec="…/OxiTide_0.2.0_amd64.AppImage" %u`) and `xdg-mime query default
      x-scheme-handler/tidal` now resolves to `oxitide-app-handler.desktop`.
      An `xdg-open` round-trip was not attempted (deferred; the installed deb
      also ships `/usr/share/applications/OxiTide.desktop` with the scheme
      handler).

## Deferred (not verifiable in WSL2)

- [ ] **AUR install on Arch:** in a clean chroot (`makechrootpkg` or a fresh
      VM), `makepkg --printsrcinfo`, then `makepkg -si`. Confirm `/usr/bin/oxitide`
      exists, hicolor icons install, the `.desktop` lands with the scheme
      handler, and `namcap packaging/aur/PKGBUILD` is clean. Then
      `xdg-mime query default x-scheme-handler/tidal` names `oxitide.desktop`
      and `xdg-open 'tidal://...'` launches/focuses the app.
      **Pin reachable (2026-10-04):** `main` was pushed to Codeberg
      (`3e78bf2..ccd70f2`), and the pinned commit
      `31b27a8d532b977637d5418c003a5fd47f7784f0` is an ancestor of
      `origin/main` (`git merge-base --is-ancestor` exit 0), so an AUR user's
      `makepkg` source fetch resolves. The AUR install itself stays deferred to
      the Arch host; no re-pin was needed (fast-forward integration). Cross-ref
      `2026-10-03-track8-bare-metal-checklist.md` (Packaging and platform floor,
      AUR install row).
- [ ] **Oldest-base launch:** run the deb and the AppImage built here on a
      bare-metal Ubuntu 22.04 (non-WSL) host; confirm no
      `GLIBC_x.y not found`, the window renders, and audio plays once Plan 8.2
      is in. Also run the AppImage on a current distro. Cross-ref
      `2026-10-03-track8-bare-metal-checklist.md` (Packaging and platform floor,
      AppImage oldest-base launch and WebKitGTK 4.1 floor rows).
- [ ] **Real-DE desktop entry:** install the deb on a real desktop session and
      verify the app appears under Audio/Video with the OxiTide icon and that
      `tidal://` opens it (cross-ref bare-metal checklist).

Close-out (2026-10-04): the build/artifact/static rows above are PASS; the two
launch rows are process-level only (window visibility pending a human check),
and the `tidal://` round-trip is partial (registration resolves). The AUR
static checks were completed by Task 3 (recipe commit `31b27a8`, evidence
above). The shared launch rows also live in
`2026-10-03-track8-wslg-smoke.md` (Packaging launch); this note carries the
detail — keep that file's checkbox in sync. The deferred rows below remain
open — they are tracked in
`2026-10-03-track8-bare-metal-checklist.md` (Packaging and platform floor) and
Track 8 closure still requires them (spec §5.4).
