# Track 8 packaging smoke checklist (2026-10-03)

Human-in-the-loop checks for the packaging half of Track 8 (Plan 8.3,
`plans/2026-10-03-track8-3-linux-packaging.md`). Build environment: WSL2
Ubuntu 22.04 from Plan 8.1 (`docs/linux-build.md`), with WSLg for GUI windows.
Run every command from the repository root on the `track8-linux` branch/worktree.

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

- [ ] **Host tools present:** `command -v xdg-mime` and
      `command -v update-desktop-database` both resolve. The AppImage bundler
      hard-copies `/usr/bin/xdg-mime` when `plugins.deep-link` is configured;
      missing `xdg-utils` fails the build. Record `xdg-utils` and
      `desktop-file-utils` versions (`dpkg-query -W`).
- [ ] **Environment lines recorded:** WSL distro/version (`lsb_release -ds`),
      kernel (`uname -r`), Rust (`rustc -V`), Node/npm (`node -v`, `npm -v`),
      Tauri CLI (`./ui/node_modules/.bin/tauri --version`, expected `2.11.5`).

## WSL2-verifiable now

- [ ] **Build:** `npm --prefix ui run app:build` completes and reports
      `Finished 2 bundles at:` for deb + AppImage. Record the run date and
      build time.
- [ ] **Artifacts exact paths:** `ls -l target/release/bundle/deb/` and
      `ls -l target/release/bundle/appimage/`; copy the actual filenames here
      (expected names above). Record sizes.
- [ ] **Artifact architecture:** `dpkg-deb -f ... Architecture` prints `amd64`.
- [ ] **Deb Depends:** `dpkg-deb -f ... Depends` lists `libasound2`,
      `xdg-utils`, `desktop-file-utils`, plus the automatically appended
      `libwebkit2gtk-4.1-0` and `libgtk-3-0`. (On Ubuntu 24.04+ note that
      `libasound2` is transitional to `libasound2t64`.)
- [ ] **Deb desktop entry:** `dpkg-deb -x` to `/tmp/oxitide-deb`; the
      `usr/share/applications/OxiTide.desktop` file contains exactly:
      `Categories=AudioVideo;Audio;Music;`,
      `MimeType=x-scheme-handler/tidal;`, `Name=OxiTide`,
      `Exec=oxitide-app`, `Icon=oxitide-app`; the matching PNGs exist under
      `usr/share/icons/hicolor/{32x32,128x128,256x256,512x512}/apps/oxitide-app.png`.
      Paste the `grep -E '^(Name|Exec|Icon|Categories|MimeType)='` output.
- [ ] **AppImage desktop entry:** run the AppImage with `--appimage-extract`
      (no FUSE needed) and verify
      `squashfs-root/usr/share/applications/OxiTide.desktop` carries the same
      `MimeType` and `Categories` lines. Paste the output.
- [ ] **Deb launch (WSLg):** `sudo apt-get install -y ./target/release/bundle/deb/OxiTide_0.2.0_amd64.deb`
      then `oxitide-app`; the window opens in WSLg. Record whether this build
      predates Plans 8.1/8.2 (login notice/audio may be stubs).
- [ ] **AppImage launch (WSLg):** `chmod +x`, probe `test -e /dev/fuse`, then
      run directly; if it fails with a FUSE/`libfuse.so.2` error, rerun with
      `--appimage-extract-and-run` and record which path worked.
- [ ] **AUR recipe static checks:** `bash -n packaging/aur/PKGBUILD`,
      `grep -qx 'MimeType=x-scheme-handler/tidal;' packaging/aur/oxitide.desktop`,
      `grep -qx 'Exec=oxitide' packaging/aur/oxitide.desktop`,
      `desktop-file-validate packaging/aur/oxitide.desktop` (no output), and
      `_commit` resolves to a commit containing `packaging/aur/oxitide.desktop`,
      `LICENSE`, and `crates/oxitide-app/icons/icon.png` (`git cat-file -e`).
- [ ] **`tidal://` best effort (WSLg):** if `xdg-open tidal://...` reaches the
      app, record it; full registration is deferred.

## Deferred (not verifiable in WSL2)

- [ ] **AUR install on Arch:** in a clean chroot (`makechrootpkg` or a fresh
      VM), `makepkg --printsrcinfo`, then `makepkg -si`. Confirm `/usr/bin/oxitide`
      exists, hicolor icons install, the `.desktop` lands with the scheme
      handler, and `namcap packaging/aur/PKGBUILD` is clean. Then
      `xdg-mime query default x-scheme-handler/tidal` names `oxitide.desktop`
      and `xdg-open 'tidal://...'` launches/focuses the app.
- [ ] **Oldest-base launch:** run the deb and the AppImage built here on a
      bare-metal Ubuntu 22.04 (non-WSL) host; confirm no
      `GLIBC_x.y not found`, the window renders, and audio plays once Plan 8.2
      is in. Also run the AppImage on a current distro.
- [ ] **Real-DE desktop entry:** install the deb on a real desktop session and
      verify the app appears under Audio/Video with the OxiTide icon and that
      `tidal://` opens it (cross-ref bare-metal checklist).

Close-out: when the WSL2-verifiable rows are PASS (or explicitly recorded as
exempt), note the result here; Track 8 closure still requires the deferred rows
in `2026-10-03-track8-bare-metal-checklist.md` (spec §5.4).
